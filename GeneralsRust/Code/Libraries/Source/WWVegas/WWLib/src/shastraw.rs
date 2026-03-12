use crate::sha::ShaEngine;
use crate::straw::{Straw, StrawBase};

pub struct SHAStraw {
    base: StrawBase,
    disabled: bool,
    sha: ShaEngine,
}

impl SHAStraw {
    pub fn new() -> Self {
        Self {
            base: StrawBase::new(),
            disabled: false,
            sha: ShaEngine::new(),
        }
    }

    pub fn disable(&mut self) {
        self.disabled = true;
    }

    pub fn enable(&mut self) {
        self.disabled = false;
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

impl Default for SHAStraw {
    fn default() -> Self {
        Self::new()
    }
}

impl Straw for SHAStraw {
    fn base(&self) -> &StrawBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut StrawBase {
        &mut self.base
    }

    fn get(&mut self, buffer: &mut [u8]) -> i32 {
        if buffer.is_empty() {
            return 0;
        }
        let got = Straw::get(self, buffer);
        if got > 0 && !self.disabled {
            self.sha.update(&buffer[..got as usize]);
        }
        got
    }
}
