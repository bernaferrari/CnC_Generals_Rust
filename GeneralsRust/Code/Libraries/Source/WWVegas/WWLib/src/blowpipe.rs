use crate::blowfish::{BlowfishEngine, BLOCK_SIZE};
use crate::pipe::{Pipe, PipeBase};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CryptControl {
    Encrypt,
    Decrypt,
}

/// BlowPipe - performs Blowfish encryption/decryption on stream data.
pub struct BlowPipe {
    base: PipeBase,
    engine: Option<BlowfishEngine>,
    buffer: [u8; BLOCK_SIZE],
    counter: usize,
    control: CryptControl,
}

impl BlowPipe {
    pub fn new(control: CryptControl) -> Self {
        Self {
            base: PipeBase::new(),
            engine: None,
            buffer: [0u8; BLOCK_SIZE],
            counter: 0,
            control,
        }
    }

    pub fn key(&mut self, key: &[u8]) {
        let mut engine = self.engine.take().unwrap_or_else(BlowfishEngine::new);
        let _ = engine.submit_key(key);
        self.engine = Some(engine);
    }

    fn process_block(&mut self, input: &[u8], output: &mut [u8]) {
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
            output[..input.len()].copy_from_slice(input);
        }
    }
}

impl Pipe for BlowPipe {
    fn base(&self) -> &PipeBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut PipeBase {
        &mut self.base
    }

    fn put(&mut self, source: &[u8]) -> i32 {
        if source.is_empty() {
            return Pipe::put(self, source);
        }

        if self.engine.is_none() {
            return Pipe::put(self, source);
        }

        let mut total = 0i32;
        let mut offset = 0usize;
        let mut slen = source.len();

        if self.counter > 0 {
            let sublen = (BLOCK_SIZE - self.counter).min(slen);
            self.buffer[self.counter..self.counter + sublen]
                .copy_from_slice(&source[offset..offset + sublen]);
            self.counter += sublen;
            offset += sublen;
            slen -= sublen;

            if self.counter == BLOCK_SIZE {
                let mut out = [0u8; BLOCK_SIZE];
                let block = self.buffer;
                self.process_block(&block, &mut out);
                total += Pipe::put(self, &out);
                self.counter = 0;
            }
        }

        while slen >= BLOCK_SIZE {
            let block = &source[offset..offset + BLOCK_SIZE];
            let mut out = [0u8; BLOCK_SIZE];
            self.process_block(block, &mut out);
            total += Pipe::put(self, &out);
            offset += BLOCK_SIZE;
            slen -= BLOCK_SIZE;
        }

        if slen > 0 {
            self.buffer[..slen].copy_from_slice(&source[offset..offset + slen]);
            self.counter = slen;
        }

        total
    }

    fn flush(&mut self) -> i32 {
        let mut total = 0i32;
        if self.counter > 0 && self.engine.is_some() {
            let pending = self.buffer[..self.counter].to_vec();
            total += Pipe::put(self, &pending);
        }
        self.counter = 0;
        total + Pipe::flush(self)
    }
}
