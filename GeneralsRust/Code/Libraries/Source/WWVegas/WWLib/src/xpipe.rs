use crate::buff::Buffer;
use crate::pipe::{Pipe, PipeBase};
use crate::wwfile::{FileInterface, FileRights};
use std::cell::RefCell;
use std::rc::Rc;

pub struct BufferPipe {
    base: PipeBase,
    buffer: Buffer,
    index: i32,
}

impl BufferPipe {
    pub fn new(buffer: Buffer) -> Self {
        Self {
            base: PipeBase::new(),
            buffer,
            index: 0,
        }
    }

    fn is_valid(&self) -> bool {
        self.buffer.is_valid()
    }
}

impl Pipe for BufferPipe {
    fn base(&self) -> &PipeBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut PipeBase {
        &mut self.base
    }

    fn put(&mut self, source: &[u8]) -> i32 {
        if !self.is_valid() || source.is_empty() {
            return 0;
        }

        let mut len = source.len() as i32;
        if self.buffer.get_size() != 0 {
            let theoretical_max = self.buffer.get_size() - self.index;
            if len > theoretical_max {
                len = theoretical_max;
            }
        }

        if len > 0 {
            if let Some(dest) = self.buffer.as_mut_slice() {
                let start = self.index as usize;
                dest[start..start + len as usize].copy_from_slice(&source[..len as usize]);
            }
        }

        self.index += len;
        len
    }
}

pub struct FilePipe {
    base: PipeBase,
    file: Option<Rc<RefCell<dyn FileInterface>>>,
    has_opened: bool,
}

impl FilePipe {
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

impl Pipe for FilePipe {
    fn base(&self) -> &PipeBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut PipeBase {
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

impl Drop for FilePipe {
    fn drop(&mut self) {
        if self.valid_file() && self.has_opened {
            if let Some(file_ref) = self.file.take() {
                let _ = file_ref.borrow_mut().close();
            }
            self.has_opened = false;
        }
    }
}
