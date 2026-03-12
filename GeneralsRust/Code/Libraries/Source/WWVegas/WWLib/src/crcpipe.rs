use crate::crc::CrcEngine;
use crate::pipe::{Pipe, PipeBase};

/// CRCPipe - calculates CRC on data passing through.
pub struct CrcPipe {
    base: PipeBase,
    crc: CrcEngine,
}

impl CrcPipe {
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

impl Pipe for CrcPipe {
    fn base(&self) -> &PipeBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut PipeBase {
        &mut self.base
    }

    fn put(&mut self, source: &[u8]) -> i32 {
        if !source.is_empty() {
            self.crc.update_buffer(source);
        }
        Pipe::put(self, source)
    }
}
