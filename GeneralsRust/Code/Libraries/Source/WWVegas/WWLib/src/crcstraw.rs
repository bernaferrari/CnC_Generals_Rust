use crate::crc::CrcEngine;
use crate::straw::{Straw, StrawBase};

/// CRCStraw - calculates CRC on data passing through.
pub struct CrcStraw<'a> {
    base: StrawBase<'a>,
    crc: CrcEngine,
}

impl<'a> CrcStraw<'a> {
    pub fn new() -> Self {
        Self {
            base: StrawBase::new(),
            crc: CrcEngine::new(),
        }
    }

    pub fn result(&self) -> i32 {
        self.crc.value()
    }
}

impl<'a> Straw<'a> for CrcStraw<'a> {
    fn base(&self) -> &StrawBase<'a> {
        &self.base
    }

    fn base_mut(&mut self) -> &mut StrawBase<'a> {
        &mut self.base
    }

    fn get(&mut self, buffer: &mut [u8]) -> i32 {
        if buffer.is_empty() {
            return 0;
        }
        let count = Straw::get(self, buffer);
        if count > 0 {
            self.crc.update_buffer(&buffer[..count as usize]);
        }
        count
    }
}
