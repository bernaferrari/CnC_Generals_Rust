use crate::buff::BufferSource;
use crate::straw::{Straw, StrawBase};
use crate::wwfile::{FileInterface, FileRights};
use std::cell::RefCell;
use std::rc::Rc;

pub struct BufferStraw<'a> {
    base: StrawBase<'a>,
    buffer: BufferSource<'a>,
    index: usize,
}

impl<'a> BufferStraw<'a> {
    pub fn new(buffer: BufferSource<'a>) -> Self {
        Self {
            base: StrawBase::new(),
            buffer,
            index: 0,
        }
    }
}

impl<'a> Straw<'a> for BufferStraw<'a> {
    fn base(&self) -> &StrawBase<'a> {
        &self.base
    }

    fn base_mut(&mut self) -> &mut StrawBase<'a> {
        &mut self.base
    }

    fn get(&mut self, buffer: &mut [u8]) -> i32 {
        let source = self.buffer.as_slice();
        let len = buffer.len().min(source.len().saturating_sub(self.index));
        buffer[..len].copy_from_slice(&source[self.index..self.index + len]);
        self.index += len;
        len as i32
    }
}

pub struct FileStraw<'a> {
    base: StrawBase<'a>,
    file: Option<Rc<RefCell<dyn FileInterface>>>,
    has_opened: bool,
}

impl<'a> FileStraw<'a> {
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

impl<'a> Straw<'a> for FileStraw<'a> {
    fn base(&self) -> &StrawBase<'a> {
        &self.base
    }

    fn base_mut(&mut self) -> &mut StrawBase<'a> {
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

impl Drop for FileStraw<'_> {
    fn drop(&mut self) {
        if self.valid_file() && self.has_opened {
            if let Some(file_ref) = self.file.take() {
                let _ = file_ref.borrow_mut().close();
            }
            self.has_opened = false;
        }
    }
}
