use crate::buff::Buffer;
use crate::straw::{Straw, StrawBase};
use crate::wwfile::{FileInterface, FileRights};
use std::cell::RefCell;
use std::rc::Rc;

pub struct BufferStraw {
    base: StrawBase,
    buffer: Buffer,
    index: i32,
}

impl BufferStraw {
    pub fn new(buffer: Buffer) -> Self {
        Self {
            base: StrawBase::new(),
            buffer,
            index: 0,
        }
    }

    fn is_valid(&self) -> bool {
        self.buffer.is_valid()
    }
}

impl Straw for BufferStraw {
    fn base(&self) -> &StrawBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut StrawBase {
        &mut self.base
    }

    fn get(&mut self, buffer: &mut [u8]) -> i32 {
        if !self.is_valid() || buffer.is_empty() {
            return 0;
        }

        let mut len = buffer.len() as i32;
        if self.buffer.get_size() != 0 {
            let theoretical_max = self.buffer.get_size() - self.index;
            if len > theoretical_max {
                len = theoretical_max;
            }
        }

        if len > 0 {
            if let Some(src) = self.buffer.as_slice() {
                let start = self.index as usize;
                buffer[..len as usize].copy_from_slice(&src[start..start + len as usize]);
            }
        }

        self.index += len;
        len
    }
}

pub struct FileStraw {
    base: StrawBase,
    file: Option<Rc<RefCell<dyn FileInterface>>>,
    has_opened: bool,
}

impl FileStraw {
    pub fn new(file: Rc<RefCell<dyn FileInterface>>) -> Self {
        Self {
            base: StrawBase::new(),
            file: Some(file),
            has_opened: false,
        }
    }

    fn valid_file(&self) -> bool {
        self.file.is_some()
    }
}

impl Straw for FileStraw {
    fn base(&self) -> &StrawBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut StrawBase {
        &mut self.base
    }

    fn get(&mut self, buffer: &mut [u8]) -> i32 {
        if !self.valid_file() || buffer.is_empty() {
            return 0;
        }

        let file_ref = self.file.as_ref().unwrap();
        let mut file = file_ref.borrow_mut();
        if !file.is_open() {
            self.has_opened = true;
            if !file.is_available(false) {
                return 0;
            }
            if file.open(FileRights::Read).is_err() {
                return 0;
            }
        }

        match file.read(buffer) {
            Ok(read) => read as i32,
            Err(_) => 0,
        }
    }
}

impl Drop for FileStraw {
    fn drop(&mut self) {
        if self.valid_file() && self.has_opened {
            if let Some(file_ref) = self.file.take() {
                let _ = file_ref.borrow_mut().close();
            }
            self.has_opened = false;
        }
    }
}
