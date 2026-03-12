use crate::int::{generate_prime, BigInt};
use crate::straw::Straw;

#[derive(Clone, Debug)]
pub struct PKey {
    modulus: BigInt,
    exponent: BigInt,
    bit_precision: u32,
}

impl PKey {
    pub fn new() -> Self {
        Self {
            modulus: BigInt::zero(),
            exponent: BigInt::zero(),
            bit_precision: 0,
        }
    }

    pub fn from_der(exponent: &[u8], modulus: &[u8]) -> Self {
        let mut key = Self::new();
        if let Some(modulus_val) = BigInt::der_decode(modulus) {
            key.modulus = modulus_val;
            key.bit_precision = key.modulus.bit_count().saturating_sub(1);
        }
        if let Some(exp_val) = BigInt::der_decode(exponent) {
            key.exponent = exp_val;
        }
        key
    }

    pub fn encrypt_into(&self, source: &[u8], dest: &mut [u8]) -> usize {
        let plain_size = self.plain_block_size();
        let crypt_size = self.crypt_block_size();
        if plain_size == 0 || crypt_size == 0 {
            return 0;
        }

        let mut total = 0usize;
        let mut offset = 0usize;
        while source.len() - offset >= plain_size {
            if total + crypt_size > dest.len() {
                break;
            }
            let block = &source[offset..offset + plain_size];
            let temp = BigInt::from_le_bytes(block).exp_mod(&self.exponent, &self.modulus);
            let out = temp.to_le_bytes_fixed(crypt_size);
            dest[total..total + crypt_size].copy_from_slice(&out);
            offset += plain_size;
            total += crypt_size;
        }
        total
    }

    pub fn decrypt_into(&self, source: &[u8], dest: &mut [u8]) -> usize {
        let plain_size = self.plain_block_size();
        let crypt_size = self.crypt_block_size();
        if plain_size == 0 || crypt_size == 0 {
            return 0;
        }

        let mut total = 0usize;
        let mut offset = 0usize;
        while source.len() - offset >= crypt_size {
            if total + plain_size > dest.len() {
                break;
            }
            let block = &source[offset..offset + crypt_size];
            let temp = BigInt::from_le_bytes(block).exp_mod(&self.exponent, &self.modulus);
            let out = temp.to_le_bytes_fixed(plain_size);
            dest[total..total + plain_size].copy_from_slice(&out);
            offset += crypt_size;
            total += plain_size;
        }
        total
    }

    pub fn generate(random: &mut dyn Straw, bits: u32, fastkey: &mut PKey, slowkey: &mut PKey) {
        loop {
            let p = generate_prime(random, bits);
            let q = generate_prime(random, bits);
            let e = PKey::fast_exponent();
            let n = p.clone() * q.clone();
            let pqmin = (p - BigInt::from(1u64)) * (q - BigInt::from(1u64));
            let d = e.inverse_mod(&pqmin);

            fastkey.exponent = e.clone();
            fastkey.modulus = n.clone();
            fastkey.bit_precision = n.bit_count().saturating_sub(1);

            slowkey.exponent = d;
            slowkey.modulus = n;
            slowkey.bit_precision = fastkey.bit_precision;

            let plain_size = fastkey.plain_block_size();
            if plain_size == 0 || plain_size > 256 {
                break;
            }

            let mut before = vec![0u8; plain_size];
            let mut produced = 0usize;
            while produced < before.len() {
                let got = random.get(&mut before[produced..]);
                if got <= 0 {
                    break;
                }
                produced += got as usize;
            }
            if produced != before.len() {
                continue;
            }
            let mut after = vec![0u8; fastkey.crypt_block_size()];
            let encrypted = fastkey.encrypt_into(&before, &mut after);
            let mut decrypted = vec![0u8; plain_size];
            let _ = slowkey.decrypt_into(&after[..encrypted], &mut decrypted);

            if before == decrypted {
                break;
            }
        }
    }

    pub fn plain_block_size(&self) -> usize {
        if self.bit_precision == 0 {
            0
        } else {
            ((self.bit_precision - 1) / 8) as usize
        }
    }

    pub fn crypt_block_size(&self) -> usize {
        self.plain_block_size() + 1
    }

    pub fn block_count(&self, plaintext_length: usize) -> usize {
        let plain_size = self.plain_block_size();
        if plaintext_length == 0 || plain_size == 0 {
            0
        } else {
            ((plaintext_length - 1) / plain_size) + 1
        }
    }

    pub fn encode_modulus(&self, buffer: &mut [u8]) -> usize {
        let encoded = self.modulus.der_encode();
        if buffer.len() < encoded.len() {
            return 0;
        }
        buffer[..encoded.len()].copy_from_slice(&encoded);
        encoded.len()
    }

    pub fn encode_exponent(&self, buffer: &mut [u8]) -> usize {
        let encoded = self.exponent.der_encode();
        if buffer.len() < encoded.len() {
            return 0;
        }
        buffer[..encoded.len()].copy_from_slice(&encoded);
        encoded.len()
    }

    pub fn decode_modulus(&mut self, buffer: &[u8]) {
        if let Some(decoded) = BigInt::der_decode(buffer) {
            self.modulus = decoded;
            self.bit_precision = self.modulus.bit_count().saturating_sub(1);
        }
    }

    pub fn decode_exponent(&mut self, buffer: &[u8]) {
        if let Some(decoded) = BigInt::der_decode(buffer) {
            self.exponent = decoded;
        }
    }

    pub fn fast_exponent() -> BigInt {
        BigInt::from(65537u64)
    }
}

impl Default for PKey {
    fn default() -> Self {
        Self::new()
    }
}
