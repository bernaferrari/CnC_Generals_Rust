use crate::blowfish::MAX_KEY_LENGTH;
use crate::blowpipe::{BlowPipe, CryptControl as BlowControl};
use crate::pipe::{Pipe, PipeBase};
use crate::pk::PKey;
use crate::rndstraw::RandomStraw;
use crate::straw::Straw;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CryptControl {
    Encrypt,
    Decrypt,
}

pub struct PKPipe {
    base: PipeBase,
    is_getting_key: bool,
    rand: Rc<RefCell<RandomStraw>>,
    bf: Rc<RefCell<BlowPipe>>,
    control: CryptControl,
    cipher_key: Option<Rc<PKey>>,
    buffer: [u8; 256],
    counter: usize,
    bytes_left: usize,
}

impl PKPipe {
    pub fn new(control: CryptControl, rnd: Rc<RefCell<RandomStraw>>) -> Self {
        let bf = Rc::new(RefCell::new(BlowPipe::new(match control {
            CryptControl::Encrypt => BlowControl::Encrypt,
            CryptControl::Decrypt => BlowControl::Decrypt,
        })));
        let mut base = PipeBase::new();
        base.set_chain_to(Some(bf.clone()));
        Self {
            base,
            is_getting_key: true,
            rand: rnd,
            bf,
            control,
            cipher_key: None,
            buffer: [0u8; 256],
            counter: 0,
            bytes_left: 0,
        }
    }

    pub fn put_to(&mut self, pipe: Option<Rc<RefCell<dyn Pipe>>>) {
        let mut bf_guard = self.bf.borrow_mut();
        if let Some(current) = bf_guard.base().chain_to() {
            current.borrow_mut().base_mut().set_chain_from(None);
            current.borrow_mut().flush();
        }

        if let Some(new_to) = pipe.clone() {
            if let Some(existing_from) = new_to.borrow().base().chain_from() {
                if let Some(existing) = existing_from.upgrade() {
                    existing.borrow_mut().base_mut().set_chain_to(None);
                }
            }
            // Trait-object back-link cannot be formed from concrete `BlowPipe` handle here.
            // Keep forward chain valid; back-link is optional for this ported path.
            new_to.borrow_mut().base_mut().set_chain_from(None);
        }

        bf_guard.base_mut().set_chain_to(pipe);
        self.base.set_chain_to(Some(self.bf.clone()));
    }

    pub fn key(&mut self, key: Option<Rc<PKey>>) {
        if key.is_none() {
            let _ = self.flush();
            self.is_getting_key = false;
        }
        self.cipher_key = key;

        if self.cipher_key.is_some() {
            self.is_getting_key = true;
            if self.control == CryptControl::Decrypt {
                self.counter = self.encrypted_key_length();
                self.bytes_left = self.counter;
            }
        }
    }

    fn encrypted_key_length(&self) -> usize {
        self.cipher_key
            .as_ref()
            .map(|key| key.block_count(MAX_KEY_LENGTH) * key.crypt_block_size())
            .unwrap_or(0)
    }

    fn plain_key_length(&self) -> usize {
        self.cipher_key
            .as_ref()
            .map(|key| key.block_count(MAX_KEY_LENGTH) * key.plain_block_size())
            .unwrap_or(0)
    }
}

impl Pipe for PKPipe {
    fn base(&self) -> &PipeBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut PipeBase {
        &mut self.base
    }

    fn put(&mut self, source: &[u8]) -> i32 {
        if source.is_empty() || self.cipher_key.is_none() {
            return Pipe::put(self, source);
        }

        let mut total = 0i32;
        let mut offset = 0usize;
        let mut remaining = source.len();

        if self.is_getting_key {
            if self.control == CryptControl::Encrypt {
                let mut key_buffer = [0u8; 256];
                key_buffer[..MAX_KEY_LENGTH].fill(0);
                let _ = self
                    .rand
                    .borrow_mut()
                    .get(&mut key_buffer[..MAX_KEY_LENGTH]);

                let plain_key_len = self.plain_key_length();
                if plain_key_len > 0 {
                    let cipher = self.cipher_key.as_ref().unwrap();
                    let did_put =
                        cipher.encrypt_into(&key_buffer[..plain_key_len], &mut self.buffer);
                    let encrypted = self.buffer[..did_put].to_vec();
                    total += Pipe::put(self, &encrypted);
                    self.bf.borrow_mut().key(&key_buffer[..MAX_KEY_LENGTH]);
                    self.is_getting_key = false;
                }
            } else {
                let to_copy = remaining.min(self.bytes_left);
                let start = self.counter - self.bytes_left;
                self.buffer[start..start + to_copy]
                    .copy_from_slice(&source[offset..offset + to_copy]);
                remaining -= to_copy;
                offset += to_copy;
                self.bytes_left -= to_copy;

                if self.bytes_left == 0 {
                    let mut key_buffer = [0u8; 256];
                    let cipher = self.cipher_key.as_ref().unwrap();
                    let _ = cipher.decrypt_into(&self.buffer[..self.counter], &mut key_buffer);
                    self.bf.borrow_mut().key(&key_buffer[..MAX_KEY_LENGTH]);
                    self.is_getting_key = false;
                }
            }
        }

        if remaining > 0 {
            total += Pipe::put(self, &source[offset..offset + remaining]);
        }

        total
    }
}
