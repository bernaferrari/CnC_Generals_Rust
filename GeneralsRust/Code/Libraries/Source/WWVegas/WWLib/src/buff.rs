//! Buffer container (ported from WWLib buff.h).

use std::borrow::Cow;

/// Writable owned storage or an exclusive, write-through borrow.
/// Cloning snapshots the bytes into independent owned storage, including for
/// borrowed buffers; only the original borrowed buffer writes through.
#[derive(Debug)]
pub struct Buffer<'a> {
    data: Option<BufferData<'a>>,
}

#[derive(Debug)]
enum BufferData<'a> {
    Owned(Vec<u8>),
    Borrowed(&'a mut [u8]),
}

impl<'a> Buffer<'a> {
    pub fn new() -> Self {
        Self { data: None }
    }

    pub fn with_size(size: i32) -> Self {
        if size <= 0 {
            Self::new()
        } else {
            Self { data: Some(BufferData::Owned(vec![0; size as usize])) }
        }
    }

    /// Take ownership of existing writable bytes.
    pub fn from_vec(data: Vec<u8>) -> Self {
        assert!(data.len() <= i32::MAX as usize);
        Self { data: Some(BufferData::Owned(data)) }
    }

    /// Borrow writable storage for the lifetime of this buffer.
    pub fn from_slice(data: &'a mut [u8]) -> Self {
        assert!(data.len() <= i32::MAX as usize);
        Self { data: Some(BufferData::Borrowed(data)) }
    }

    pub fn reset(&mut self) {
        self.data = None;
    }

    pub fn get_size(&self) -> i32 {
        self.as_slice().map_or(0, |data| data.len() as i32)
    }

    pub fn is_valid(&self) -> bool {
        self.data.is_some()
    }

    pub fn as_slice(&self) -> Option<&[u8]> {
        match &self.data {
            Some(BufferData::Owned(data)) => Some(data),
            Some(BufferData::Borrowed(data)) => Some(data),
            None => None,
        }
    }

    pub fn as_mut_slice(&mut self) -> Option<&mut [u8]> {
        match &mut self.data {
            Some(BufferData::Owned(data)) => Some(data),
            Some(BufferData::Borrowed(data)) => Some(data),
            None => None,
        }
    }
}

impl Clone for Buffer<'_> {
    fn clone(&self) -> Self {
        Self { data: self.as_slice().map(|data| BufferData::Owned(data.to_vec())) }
    }
}

impl Default for Buffer<'_> {
    fn default() -> Self {
        Self::new()
    }
}

/// Read-only storage for a buffer straw.
#[derive(Debug, Clone)]
pub struct BufferSource<'a> {
    data: Cow<'a, [u8]>,
}

impl<'a> BufferSource<'a> {
    pub fn from_slice(data: &'a [u8]) -> Self {
        assert!(data.len() <= i32::MAX as usize);
        Self { data: Cow::Borrowed(data) }
    }

    pub fn from_vec(data: Vec<u8>) -> Self {
        assert!(data.len() <= i32::MAX as usize);
        Self { data: Cow::Owned(data) }
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }
}

#[cfg(test)]
mod tests {
    use super::{Buffer, BufferSource};
    use crate::b64pipe::{Base64Pipe, CodeControl as PipeControl};
    use crate::b64straw::{Base64Straw, CodeControl as StrawControl};
    use crate::pipe::{Pipe, put_to};
    use crate::straw::{Straw, get_from};
    use crate::xpipe::BufferPipe;
    use crate::xstraw::BufferStraw;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn owned_buffers_flow_through_pipe_and_straw_chains() {
        let sink = Rc::new(RefCell::new(BufferPipe::new(Buffer::with_size(4))));
        let encoder: Rc<RefCell<dyn Pipe<'_> + '_>> =
            Rc::new(RefCell::new(Base64Pipe::new(PipeControl::Encode)));
        put_to(&encoder, Some(sink.clone()));
        assert_eq!(encoder.borrow_mut().put(b"cat"), 4);
        let encoded = sink.borrow().buffer().as_slice().unwrap().to_vec();
        assert_eq!(encoded, b"Y2F0");

        let source = Rc::new(RefCell::new(BufferStraw::new(BufferSource::from_vec(encoded))));
        let decoder: Rc<RefCell<dyn Straw<'_> + '_>> =
            Rc::new(RefCell::new(Base64Straw::new(StrawControl::Decode)));
        get_from(&decoder, Some(source));
        let mut decoded = [0; 4];
        assert_eq!(decoder.borrow_mut().get(&mut decoded), 3);
        assert_eq!(&decoded[..3], b"cat");
    }

    #[test]
    fn borrowed_bytes_flow_through_multiple_pipe_and_straw_stages() {
        let mut bytes = [0u8; 3];
        {
            let sink = Rc::new(RefCell::new(BufferPipe::new(Buffer::from_slice(&mut bytes))));
            let decoder: Rc<RefCell<dyn Pipe<'_> + '_>> =
                Rc::new(RefCell::new(Base64Pipe::new(PipeControl::Decode)));
            let encoder: Rc<RefCell<dyn Pipe<'_> + '_>> =
                Rc::new(RefCell::new(Base64Pipe::new(PipeControl::Encode)));
            put_to(&decoder, Some(sink));
            put_to(&encoder, Some(decoder));
            assert_eq!(encoder.borrow_mut().put(b"c"), 0);
            assert_eq!(encoder.borrow_mut().put(b"at"), 3);
        }
        assert_eq!(&bytes, b"cat");

        let mut result = [0u8; 4];
        {
            let source = Rc::new(RefCell::new(BufferStraw::new(BufferSource::from_slice(&bytes))));
            let encoder: Rc<RefCell<dyn Straw<'_> + '_>> =
                Rc::new(RefCell::new(Base64Straw::new(StrawControl::Encode)));
            let decoder: Rc<RefCell<dyn Straw<'_> + '_>> =
                Rc::new(RefCell::new(Base64Straw::new(StrawControl::Decode)));
            get_from(&encoder, Some(source));
            get_from(&decoder, Some(encoder));
            assert_eq!(decoder.borrow_mut().get(&mut result), 3);
        }
        assert_eq!(&result[..3], b"cat");
    }

    #[test]
    fn borrowed_pipe_chain_truncates_in_stream_order() {
        let mut bytes = [0u8; 2];
        {
            let sink = Rc::new(RefCell::new(BufferPipe::new(Buffer::from_slice(&mut bytes))));
            let encoder: Rc<RefCell<dyn Pipe<'_> + '_>> =
                Rc::new(RefCell::new(Base64Pipe::new(PipeControl::Encode)));
            put_to(&encoder, Some(sink));
            assert_eq!(encoder.borrow_mut().put(b"cat"), 2);
        }
        assert_eq!(&bytes, b"Y2");
    }

    #[test]
    fn borrowed_storage_writes_through_and_clones_are_isolated() {
        let mut bytes = [0; 3];
        {
            let mut buffer = Buffer::from_slice(&mut bytes);
            let mut copy = buffer.clone();
            copy.as_mut_slice().unwrap()[0] = 9;
            assert_eq!(buffer.as_slice().unwrap(), &[0, 0, 0]);
            let mut pipe = BufferPipe::new(buffer);
            assert_eq!(pipe.put(b"abcd"), 3);
            buffer = pipe.into_buffer();
            assert_eq!(buffer.as_slice().unwrap(), b"abc");
        }
        assert_eq!(&bytes, b"abc");
        let mut straw = BufferStraw::new(BufferSource::from_slice(&bytes));
        let mut result = [0; 4];
        assert_eq!(straw.get(&mut result), 3);
        assert_eq!(&result[..3], b"abc");
    }
}
