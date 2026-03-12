use crate::pipe::{Pipe, PipeBase};
use crate::sha::ShaEngine;

pub struct SHAPipe {
    base: PipeBase,
    sha: ShaEngine,
}

impl SHAPipe {
    pub fn new() -> Self {
        Self {
            base: PipeBase::new(),
            sha: ShaEngine::new(),
        }
    }

    pub fn result(&mut self, out: &mut [u8]) -> usize {
        if out.len() < crate::sha::SHA1_DIGEST_SIZE {
            return 0;
        }
        let digest = self.sha.finalize();
        out[..crate::sha::SHA1_DIGEST_SIZE].copy_from_slice(&digest);
        crate::sha::SHA1_DIGEST_SIZE
    }
}

impl Default for SHAPipe {
    fn default() -> Self {
        Self::new()
    }
}

impl Pipe for SHAPipe {
    fn base(&self) -> &PipeBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut PipeBase {
        &mut self.base
    }

    fn put(&mut self, source: &[u8]) -> i32 {
        if !source.is_empty() {
            self.sha.update(source);
        }
        Pipe::put(self, source)
    }
}
