use crate::buff::Buffer;
use crate::pipe::{Pipe, PipeBase};
use crate::wwfile::{FileInterface, FileRights};
use std::cell::RefCell;
use std::rc::Rc;

pub struct BufferPipe<'a> {
    base: PipeBase<'a>,
    buffer: Buffer<'a>,
    index: usize,
}

impl<'a> BufferPipe<'a> {
    pub fn new(buffer: Buffer<'a>) -> Self {
        Self {
            base: PipeBase::new(),
            buffer,
            index: 0,
        }
    }

    pub fn buffer(&self) -> &Buffer<'a> {
        &self.buffer
    }

    pub fn into_buffer(self) -> Buffer<'a> {
        self.buffer
    }
}

impl<'a> Pipe<'a> for BufferPipe<'a> {
    fn base(&self) -> &PipeBase<'a> {
        &self.base
    }

    fn base_mut(&mut self) -> &mut PipeBase<'a> {
        &mut self.base
    }

    fn put(&mut self, source: &[u8]) -> i32 {
        let Some(dest) = self.buffer.as_mut_slice() else {
            return 0;
        };
        let len = source.len().min(dest.len().saturating_sub(self.index));
        dest[self.index..self.index + len].copy_from_slice(&source[..len]);
        self.index += len;
        len as i32
    }
}

pub struct FilePipe<'a> {
    base: PipeBase<'a>,
    file: Option<Rc<RefCell<dyn FileInterface>>>,
    has_opened: bool,
}

impl<'a> FilePipe<'a> {
    pub fn new(file: Rc<RefCell<dyn FileInterface>>) -> Self {
        Self {
            base: PipeBase::new(),
            file: Some(file),
            has_opened: false,
        }
    }

    fn valid_file(&self) -> bool {
        self.file.is_some()
    }
}

impl<'a> Pipe<'a> for FilePipe<'a> {
    fn base(&self) -> &PipeBase<'a> {
        &self.base
    }

    fn base_mut(&mut self) -> &mut PipeBase<'a> {
        &mut self.base
    }

    fn put(&mut self, source: &[u8]) -> i32 {
        if !self.valid_file() || source.is_empty() {
            return 0;
        }

        let file_ref = self.file.as_ref().unwrap();
        let mut file = file_ref.borrow_mut();
        if !file.is_open() {
            self.has_opened = true;
            let _ = file.open(FileRights::Write);
        }
        match file.write(source) {
            Ok(written) => written as i32,
            Err(_) => 0,
        }
    }

    fn end(&mut self) -> i32 {
        let total = Pipe::end(self);
        if self.valid_file() && self.has_opened {
            if let Some(file_ref) = self.file.take() {
                let _ = file_ref.borrow_mut().close();
            }
            self.has_opened = false;
        }
        total
    }
}

impl Drop for FilePipe<'_> {
    fn drop(&mut self) {
        if self.valid_file() && self.has_opened {
            if let Some(file_ref) = self.file.take() {
                let _ = file_ref.borrow_mut().close();
            }
            self.has_opened = false;
        }
    }
}
