use crate::blowfish::MAX_KEY_LENGTH;
use crate::blwstraw::{BlowStraw, CryptControl as BlowControl};
use crate::pk::PKey;
use crate::rndstraw::RandomStraw;
use crate::straw::{Straw, StrawBase};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CryptControl {
    Encrypt,
    Decrypt,
}

pub struct PKStraw {
    base: StrawBase,
    is_getting_key: bool,
    rand: Rc<RefCell<RandomStraw>>,
    bf: Rc<RefCell<BlowStraw>>,
    control: CryptControl,
    cipher_key: Option<Rc<PKey>>,
    buffer: [u8; 256],
    counter: usize,
    bytes_left: usize,
}

impl PKStraw {
    pub fn new(control: CryptControl, rnd: Rc<RefCell<RandomStraw>>) -> Self {
        let bf = Rc::new(RefCell::new(BlowStraw::new(match control {
            CryptControl::Encrypt => BlowControl::Encrypt,
            CryptControl::Decrypt => BlowControl::Decrypt,
        })));
        let mut base = StrawBase::new();
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

    pub fn get_from(&mut self, straw: Option<Rc<RefCell<dyn Straw>>>) {
        let mut bf_guard = self.bf.borrow_mut();
        if let Some(current) = bf_guard.base().chain_to() {
            current.borrow_mut().base_mut().set_chain_from(None);
        }

        if let Some(new_from) = straw.clone() {
            if let Some(existing_from) = new_from.borrow().base().chain_from() {
                if let Some(existing) = existing_from.upgrade() {
                    existing.borrow_mut().base_mut().set_chain_to(None);
                }
            }
            // Trait-object back-link cannot be formed from concrete `BlowStraw` handle here.
            // Keep forward chain valid; back-link is optional for this ported path.
            new_from.borrow_mut().base_mut().set_chain_from(None);
        }

        bf_guard.base_mut().set_chain_to(straw);
        self.base.set_chain_to(Some(self.bf.clone()));
    }

    pub fn key(&mut self, key: Option<Rc<PKey>>) {
        self.cipher_key = key;
        if self.cipher_key.is_some() {
            self.is_getting_key = true;
        }
        self.counter = 0;
        self.bytes_left = 0;
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

impl Straw for PKStraw {
    fn base(&self) -> &StrawBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut StrawBase {
        &mut self.base
    }

    fn get(&mut self, buffer: &mut [u8]) -> i32 {
        if buffer.is_empty() || self.cipher_key.is_none() {
            return Straw::get(self, buffer);
        }

        let mut total = 0usize;
        let mut remaining = buffer.len();
        let mut offset = 0usize;

        if self.is_getting_key {
            if self.control == CryptControl::Decrypt {
                let mut cbuffer = vec![0u8; self.encrypted_key_length()];
                let got = Straw::get(self, &mut cbuffer);
                if got as usize != cbuffer.len() {
                    return 0;
                }
                let cipher = self.cipher_key.as_ref().unwrap();
                let mut plain = [0u8; 256];
                let _ = cipher.decrypt_into(&cbuffer, &mut plain);
                self.bf.borrow_mut().key(&plain[..MAX_KEY_LENGTH]);
            } else {
                let mut key_buffer = [0u8; 256];
                key_buffer.fill(0);
                let _ = self
                    .rand
                    .borrow_mut()
                    .get(&mut key_buffer[..MAX_KEY_LENGTH]);

                let plain_len = self.plain_key_length();
                let cipher = self.cipher_key.as_ref().unwrap();
                self.counter = cipher.encrypt_into(&key_buffer[..plain_len], &mut self.buffer);
                self.bytes_left = self.counter;
                self.bf.borrow_mut().key(&key_buffer[..MAX_KEY_LENGTH]);
            }
            self.is_getting_key = false;
        }

        if self.bytes_left > 0 {
            let tocopy = remaining.min(self.bytes_left);
            let start = self.counter - self.bytes_left;
            buffer[offset..offset + tocopy].copy_from_slice(&self.buffer[start..start + tocopy]);
            offset += tocopy;
            remaining -= tocopy;
            self.bytes_left -= tocopy;
            total += tocopy;
        }

        if remaining > 0 {
            let got = Straw::get(self, &mut buffer[offset..offset + remaining]);
            if got > 0 {
                total += got as usize;
            }
        }

        total as i32
    }
}
