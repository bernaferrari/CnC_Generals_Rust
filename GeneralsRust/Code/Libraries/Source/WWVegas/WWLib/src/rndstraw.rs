use crate::random::{Random3Class, RandomGenerator};
use crate::sha::ShaEngine;
use crate::straw::{Straw, StrawBase};

pub struct RandomStraw {
    base: StrawBase,
    seed_bits: usize,
    current: usize,
    random: [Random3Class; 32],
}

impl RandomStraw {
    pub fn new() -> Self {
        let mut instance = Self {
            base: StrawBase::new(),
            seed_bits: 0,
            current: 0,
            random: [
                Random3Class::new(0, 0),
                Random3Class::new(0, 0),
                Random3Class::new(0, 0),
                Random3Class::new(0, 0),
                Random3Class::new(0, 0),
                Random3Class::new(0, 0),
                Random3Class::new(0, 0),
                Random3Class::new(0, 0),
                Random3Class::new(0, 0),
                Random3Class::new(0, 0),
                Random3Class::new(0, 0),
                Random3Class::new(0, 0),
                Random3Class::new(0, 0),
                Random3Class::new(0, 0),
                Random3Class::new(0, 0),
                Random3Class::new(0, 0),
                Random3Class::new(0, 0),
                Random3Class::new(0, 0),
                Random3Class::new(0, 0),
                Random3Class::new(0, 0),
                Random3Class::new(0, 0),
                Random3Class::new(0, 0),
                Random3Class::new(0, 0),
                Random3Class::new(0, 0),
                Random3Class::new(0, 0),
                Random3Class::new(0, 0),
                Random3Class::new(0, 0),
                Random3Class::new(0, 0),
                Random3Class::new(0, 0),
                Random3Class::new(0, 0),
                Random3Class::new(0, 0),
                Random3Class::new(0, 0),
            ],
        };
        instance.reset();
        instance
    }

    pub fn reset(&mut self) {
        self.seed_bits = 0;
        self.current = 0;
        for generator in &mut self.random {
            generator.set_state(0, 0);
        }
    }

    pub fn seed_bits_needed(&self) -> usize {
        let total_bits = self.random_bytes_len() * 8;
        if self.seed_bits < total_bits {
            total_bits - self.seed_bits
        } else {
            0
        }
    }

    pub fn seed_bit(&mut self, seed: i32) {
        let mut bytes = self.random_bytes();
        let index = (self.seed_bits / 8) % bytes.len();
        let mask = 1u8 << (self.seed_bits & 7);
        if seed & 0x01 != 0 {
            bytes[index] ^= mask;
        }
        self.seed_bits += 1;
        self.set_random_bytes(&bytes);
        if self.seed_bits == bytes.len() * 8 {
            self.scramble_seed();
        }
    }

    pub fn seed_byte(&mut self, mut seed: i8) {
        for _ in 0..8 {
            self.seed_bit(seed as i32);
            seed >>= 1;
        }
    }

    pub fn seed_short(&mut self, mut seed: i16) {
        for _ in 0..(i16::BITS) {
            self.seed_bit(seed as i32);
            seed >>= 1;
        }
    }

    pub fn seed_long(&mut self, mut seed: i32) {
        for _ in 0..(i32::BITS) {
            self.seed_bit(seed as i32);
            seed >>= 1;
        }
    }

    fn random_bytes_len(&self) -> usize {
        self.random.len() * 8
    }

    fn random_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.random_bytes_len());
        for generator in &self.random {
            let (seed, index) = generator.state();
            bytes.extend_from_slice(&seed.to_le_bytes());
            bytes.extend_from_slice(&index.to_le_bytes());
        }
        bytes
    }

    fn set_random_bytes(&mut self, bytes: &[u8]) {
        for (i, generator) in self.random.iter_mut().enumerate() {
            let offset = i * 8;
            let seed = i32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
            let index = i32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap());
            generator.set_state(seed, index);
        }
    }

    fn scramble_seed(&mut self) {
        let mut bytes = self.random_bytes();
        let mut sha = ShaEngine::new();
        for index in 0..bytes.len() {
            sha.update(&bytes);
            let digest = sha.finalize();
            let remaining = bytes.len() - index;
            let tocopy = digest.len().min(remaining);
            bytes[index..index + tocopy].copy_from_slice(&digest[..tocopy]);
        }
        self.set_random_bytes(&bytes);
    }
}

impl Default for RandomStraw {
    fn default() -> Self {
        Self::new()
    }
}

impl Straw for RandomStraw {
    fn base(&self) -> &StrawBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut StrawBase {
        &mut self.base
    }

    fn get(&mut self, buffer: &mut [u8]) -> i32 {
        if buffer.is_empty() {
            return Straw::get(self, buffer);
        }

        let mut total = 0usize;
        let count = buffer.len();
        while total < count {
            buffer[total] = (self.random[self.current].next() & 0xFF) as u8;
            self.current = (self.current + 1) % self.random.len();
            total += 1;
        }
        total as i32
    }
}
