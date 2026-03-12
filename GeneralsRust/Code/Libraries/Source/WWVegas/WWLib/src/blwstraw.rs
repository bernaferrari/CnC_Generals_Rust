use crate::blowfish::BlowfishEngine;
use crate::straw::{Straw, StrawBase};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CryptControl {
    Encrypt,
    Decrypt,
}

pub struct BlowStraw {
    base: StrawBase,
    engine: Option<BlowfishEngine>,
    buffer: [u8; 8],
    counter: usize,
    control: CryptControl,
}

impl BlowStraw {
    pub fn new(control: CryptControl) -> Self {
        Self {
            base: StrawBase::new(),
            engine: None,
            buffer: [0u8; 8],
            counter: 0,
            control,
        }
    }

    pub fn key(&mut self, key: &[u8]) {
        let mut engine = self.engine.take().unwrap_or_else(BlowfishEngine::new);
        let _ = engine.submit_key(key);
        self.engine = Some(engine);
    }

    fn process_block(&self, input: &[u8; 8], output: &mut [u8; 8]) {
        if let Some(engine) = &self.engine {
            match self.control {
                CryptControl::Decrypt => {
                    let _ = engine.decrypt(input, output);
                }
                CryptControl::Encrypt => {
                    let _ = engine.encrypt(input, output);
                }
            }
        } else {
            *output = *input;
        }
    }
}

impl Straw for BlowStraw {
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
        if self.engine.is_none() {
            return Straw::get(self, buffer);
        }

        let mut total = 0usize;
        let mut remaining = buffer.len();
        let mut offset = 0usize;

        while remaining > 0 {
            if self.counter > 0 {
                let tocopy = remaining.min(self.counter);
                let start = self.buffer.len() - self.counter;
                buffer[offset..offset + tocopy]
                    .copy_from_slice(&self.buffer[start..start + tocopy]);
                self.counter -= tocopy;
                offset += tocopy;
                remaining -= tocopy;
            }
            if remaining == 0 {
                break;
            }

            let mut block = [0u8; 8];
            let got = Straw::get(self, &mut block);
            if got <= 0 {
                break;
            }
            let got = got as usize;
            if got == block.len() {
                let mut out = [0u8; 8];
                self.process_block(&block, &mut out);
                self.buffer = out;
            } else {
                self.buffer[block.len() - got..].copy_from_slice(&block[..got]);
            }
            self.counter = got;
        }

        total = offset;
        total as i32
    }
}
