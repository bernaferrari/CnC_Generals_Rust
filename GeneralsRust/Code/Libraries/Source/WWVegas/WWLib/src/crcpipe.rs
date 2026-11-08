use crate::crc::CrcEngine;
use crate::pipe::{Pipe, PipeBase};

/// CRCPipe - calculates CRC on data passing through.
pub struct CrcPipe<'a> {
    base: PipeBase<'a>,
    crc: CrcEngine,
}

impl<'a> CrcPipe<'a> {
    pub fn new() -> Self {
        Self {
            base: PipeBase::new(),
            crc: CrcEngine::new(),
        }
    }

    pub fn result(&self) -> i32 {
        self.crc.value()
    }
}

impl<'a> Pipe<'a> for CrcPipe<'a> {
    fn base(&self) -> &PipeBase<'a> {
        &self.base
    }

    fn base_mut(&mut self) -> &mut PipeBase<'a> {
        &mut self.base
    }

    fn put(&mut self, source: &[u8]) -> i32 {
        if !source.is_empty() {
            self.crc.update_buffer(source);
        }
        Pipe::put(self, source)
    }
}
