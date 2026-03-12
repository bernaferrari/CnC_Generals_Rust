//! MD5 Message-Digest Algorithm Implementation
//!
//! This is a Rust implementation of the RSA Data Security, Inc. MD5 Message-Digest Algorithm
//! as used in the Command & Conquer Generals WWLib library.
//!
//! # License
//!
//! Derived from the RSA Data Security, Inc. MD5 Message-Digest Algorithm.
//! Original copyright (C) 1991-2, RSA Data Security, Inc. Created 1991.
//!
//! License to copy and use this software is granted provided that it
//! is identified as "derived from the RSA Data Security, Inc. MD5 Message-Digest
//! Algorithm" in all material mentioning or referencing the derived work.
//!
//! # Examples
//!
//! ## One-shot hashing
//!
//! ```
//! use wwlib_rust::md5::Md5;
//!
//! let digest = Md5::hash(b"hello world");
//! assert_eq!(
//!     digest,
//!     [0x5e, 0xb6, 0x3b, 0xbb, 0xe0, 0x1e, 0xee, 0xd0,
//!      0x93, 0xcb, 0x22, 0xbb, 0x8f, 0x5a, 0xcd, 0xc3]
//! );
//! ```
//!
//! ## Streaming interface
//!
//! ```
//! use wwlib_rust::md5::Md5;
//!
//! let mut hasher = Md5::new();
//! hasher.update(b"hello ");
//! hasher.update(b"world");
//! let digest = hasher.finalize();
//!
//! assert_eq!(
//!     digest,
//!     [0x5e, 0xb6, 0x3b, 0xbb, 0xe0, 0x1e, 0xee, 0xd0,
//!      0x93, 0xcb, 0x22, 0xbb, 0x8f, 0x5a, 0xcd, 0xc3]
//! );
//! ```
//!
//! # Security Note
//!
//! MD5 is cryptographically broken and unsuitable for further use in security contexts.
//! This implementation is provided for compatibility with legacy systems and should only
//! be used for checksums and non-cryptographic purposes.

use std::convert::TryInto;

/// MD5 digest length in bytes
pub const MD5_DIGEST_LENGTH: usize = 16;

/// MD5 block size in bytes
const BLOCK_SIZE: usize = 64;

/// MD5 context structure for streaming hash computation
#[derive(Clone)]
pub struct Md5 {
    /// Current hash state (A, B, C, D)
    state: [u32; 4],
    /// Number of bits processed (low 32 bits, high 32 bits)
    count: [u32; 2],
    /// Input buffer for partial blocks
    buffer: [u8; BLOCK_SIZE],
}

impl Md5 {
    /// Create a new MD5 hasher instance
    ///
    /// # Examples
    ///
    /// ```
    /// use wwlib_rust::md5::Md5;
    ///
    /// let hasher = Md5::new();
    /// ```
    pub fn new() -> Self {
        Self {
            // MD5 magic initialization constants
            state: [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476],
            count: [0, 0],
            buffer: [0; BLOCK_SIZE],
        }
    }

    /// Update the MD5 context with input data
    ///
    /// This method can be called multiple times to process data in chunks.
    ///
    /// # Arguments
    ///
    /// * `input` - The input data to hash
    ///
    /// # Examples
    ///
    /// ```
    /// use wwlib_rust::md5::Md5;
    ///
    /// let mut hasher = Md5::new();
    /// hasher.update(b"hello ");
    /// hasher.update(b"world");
    /// ```
    pub fn update(&mut self, input: &[u8]) {
        let input_len = input.len();

        // Compute number of bytes mod 64
        let index = ((self.count[0] >> 3) & 0x3F) as usize;

        // Update number of bits
        let bit_len = (input_len as u32) << 3;
        if self.count[0].wrapping_add(bit_len) < self.count[0] {
            self.count[1] = self.count[1].wrapping_add(1);
        }
        self.count[0] = self.count[0].wrapping_add(bit_len);
        self.count[1] = self.count[1].wrapping_add((input_len as u32) >> 29);

        let part_len = BLOCK_SIZE - index;

        // Transform as many times as possible
        if input_len >= part_len {
            // Fill buffer and process first block
            self.buffer[index..].copy_from_slice(&input[..part_len]);
            let buffer_copy = self.buffer;
            self.md5_transform(&buffer_copy);

            // Process complete 64-byte blocks
            let mut i = part_len;
            while i + 63 < input_len {
                let block: [u8; BLOCK_SIZE] = input[i..i + BLOCK_SIZE].try_into().unwrap();
                self.md5_transform(&block);
                i += BLOCK_SIZE;
            }

            // Buffer remaining input
            let remaining = input_len - i;
            if remaining > 0 {
                self.buffer[..remaining].copy_from_slice(&input[i..]);
            }
        } else {
            // Buffer input
            self.buffer[index..index + input_len].copy_from_slice(input);
        }
    }

    /// Finalize the MD5 computation and return the digest
    ///
    /// This consumes the hasher and returns the final 16-byte MD5 digest.
    /// The hasher cannot be used after calling this method.
    ///
    /// # Returns
    ///
    /// A 16-byte array containing the MD5 digest
    ///
    /// # Examples
    ///
    /// ```
    /// use wwlib_rust::md5::Md5;
    ///
    /// let mut hasher = Md5::new();
    /// hasher.update(b"hello world");
    /// let digest = hasher.finalize();
    /// ```
    pub fn finalize(mut self) -> [u8; MD5_DIGEST_LENGTH] {
        // Save number of bits
        let bits = encode_u32_array(&self.count, 8);

        // Pad out to 56 mod 64
        let index = ((self.count[0] >> 3) & 0x3f) as usize;
        let pad_len = if index < 56 { 56 - index } else { 120 - index };
        self.update(&PADDING[..pad_len]);

        // Append length (before padding)
        self.update(&bits);

        // Store state in digest
        let encoded = encode_u32_array(&self.state, MD5_DIGEST_LENGTH);
        let mut digest = [0u8; MD5_DIGEST_LENGTH];
        digest.copy_from_slice(&encoded);
        digest
    }

    /// Compute MD5 hash of input data in one operation
    ///
    /// This is a convenience method that creates a new hasher, processes the input,
    /// and returns the final digest.
    ///
    /// # Arguments
    ///
    /// * `input` - The data to hash
    ///
    /// # Returns
    ///
    /// A 16-byte array containing the MD5 digest
    ///
    /// # Examples
    ///
    /// ```
    /// use wwlib_rust::md5::Md5;
    ///
    /// let digest = Md5::hash(b"hello world");
    /// ```
    pub fn hash(input: &[u8]) -> [u8; MD5_DIGEST_LENGTH] {
        let mut hasher = Self::new();
        hasher.update(input);
        hasher.finalize()
    }

    /// Internal MD5 transformation function
    ///
    /// Processes a single 64-byte block and updates the internal state.
    fn md5_transform(&mut self, block: &[u8; BLOCK_SIZE]) {
        let mut a = self.state[0];
        let mut b = self.state[1];
        let mut c = self.state[2];
        let mut d = self.state[3];

        // Decode block into 32-bit words
        let x = decode_u8_array(block, BLOCK_SIZE);

        // Round 1
        ff(&mut a, b, c, d, x[0], S11, 0xd76aa478); // 1
        ff(&mut d, a, b, c, x[1], S12, 0xe8c7b756); // 2
        ff(&mut c, d, a, b, x[2], S13, 0x242070db); // 3
        ff(&mut b, c, d, a, x[3], S14, 0xc1bdceee); // 4
        ff(&mut a, b, c, d, x[4], S11, 0xf57c0faf); // 5
        ff(&mut d, a, b, c, x[5], S12, 0x4787c62a); // 6
        ff(&mut c, d, a, b, x[6], S13, 0xa8304613); // 7
        ff(&mut b, c, d, a, x[7], S14, 0xfd469501); // 8
        ff(&mut a, b, c, d, x[8], S11, 0x698098d8); // 9
        ff(&mut d, a, b, c, x[9], S12, 0x8b44f7af); // 10
        ff(&mut c, d, a, b, x[10], S13, 0xffff5bb1); // 11
        ff(&mut b, c, d, a, x[11], S14, 0x895cd7be); // 12
        ff(&mut a, b, c, d, x[12], S11, 0x6b901122); // 13
        ff(&mut d, a, b, c, x[13], S12, 0xfd987193); // 14
        ff(&mut c, d, a, b, x[14], S13, 0xa679438e); // 15
        ff(&mut b, c, d, a, x[15], S14, 0x49b40821); // 16

        // Round 2
        gg(&mut a, b, c, d, x[1], S21, 0xf61e2562); // 17
        gg(&mut d, a, b, c, x[6], S22, 0xc040b340); // 18
        gg(&mut c, d, a, b, x[11], S23, 0x265e5a51); // 19
        gg(&mut b, c, d, a, x[0], S24, 0xe9b6c7aa); // 20
        gg(&mut a, b, c, d, x[5], S21, 0xd62f105d); // 21
        gg(&mut d, a, b, c, x[10], S22, 0x02441453); // 22
        gg(&mut c, d, a, b, x[15], S23, 0xd8a1e681); // 23
        gg(&mut b, c, d, a, x[4], S24, 0xe7d3fbc8); // 24
        gg(&mut a, b, c, d, x[9], S21, 0x21e1cde6); // 25
        gg(&mut d, a, b, c, x[14], S22, 0xc33707d6); // 26
        gg(&mut c, d, a, b, x[3], S23, 0xf4d50d87); // 27
        gg(&mut b, c, d, a, x[8], S24, 0x455a14ed); // 28
        gg(&mut a, b, c, d, x[13], S21, 0xa9e3e905); // 29
        gg(&mut d, a, b, c, x[2], S22, 0xfcefa3f8); // 30
        gg(&mut c, d, a, b, x[7], S23, 0x676f02d9); // 31
        gg(&mut b, c, d, a, x[12], S24, 0x8d2a4c8a); // 32

        // Round 3
        hh(&mut a, b, c, d, x[5], S31, 0xfffa3942); // 33
        hh(&mut d, a, b, c, x[8], S32, 0x8771f681); // 34
        hh(&mut c, d, a, b, x[11], S33, 0x6d9d6122); // 35
        hh(&mut b, c, d, a, x[14], S34, 0xfde5380c); // 36
        hh(&mut a, b, c, d, x[1], S31, 0xa4beea44); // 37
        hh(&mut d, a, b, c, x[4], S32, 0x4bdecfa9); // 38
        hh(&mut c, d, a, b, x[7], S33, 0xf6bb4b60); // 39
        hh(&mut b, c, d, a, x[10], S34, 0xbebfbc70); // 40
        hh(&mut a, b, c, d, x[13], S31, 0x289b7ec6); // 41
        hh(&mut d, a, b, c, x[0], S32, 0xeaa127fa); // 42
        hh(&mut c, d, a, b, x[3], S33, 0xd4ef3085); // 43
        hh(&mut b, c, d, a, x[6], S34, 0x04881d05); // 44
        hh(&mut a, b, c, d, x[9], S31, 0xd9d4d039); // 45
        hh(&mut d, a, b, c, x[12], S32, 0xe6db99e5); // 46
        hh(&mut c, d, a, b, x[15], S33, 0x1fa27cf8); // 47
        hh(&mut b, c, d, a, x[2], S34, 0xc4ac5665); // 48

        // Round 4
        ii(&mut a, b, c, d, x[0], S41, 0xf4292244); // 49
        ii(&mut d, a, b, c, x[7], S42, 0x432aff97); // 50
        ii(&mut c, d, a, b, x[14], S43, 0xab9423a7); // 51
        ii(&mut b, c, d, a, x[5], S44, 0xfc93a039); // 52
        ii(&mut a, b, c, d, x[12], S41, 0x655b59c3); // 53
        ii(&mut d, a, b, c, x[3], S42, 0x8f0ccc92); // 54
        ii(&mut c, d, a, b, x[10], S43, 0xffeff47d); // 55
        ii(&mut b, c, d, a, x[1], S44, 0x85845dd1); // 56
        ii(&mut a, b, c, d, x[8], S41, 0x6fa87e4f); // 57
        ii(&mut d, a, b, c, x[15], S42, 0xfe2ce6e0); // 58
        ii(&mut c, d, a, b, x[6], S43, 0xa3014314); // 59
        ii(&mut b, c, d, a, x[13], S44, 0x4e0811a1); // 60
        ii(&mut a, b, c, d, x[4], S41, 0xf7537e82); // 61
        ii(&mut d, a, b, c, x[11], S42, 0xbd3af235); // 62
        ii(&mut c, d, a, b, x[2], S43, 0x2ad7d2bb); // 63
        ii(&mut b, c, d, a, x[9], S44, 0xeb86d391); // 64

        // Add this chunk's hash to result so far
        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
    }
}

impl Default for Md5 {
    fn default() -> Self {
        Self::new()
    }
}

// Constants for MD5Transform routine
const S11: u32 = 7;
const S12: u32 = 12;
const S13: u32 = 17;
const S14: u32 = 22;
const S21: u32 = 5;
const S22: u32 = 9;
const S23: u32 = 14;
const S24: u32 = 20;
const S31: u32 = 4;
const S32: u32 = 11;
const S33: u32 = 16;
const S34: u32 = 23;
const S41: u32 = 6;
const S42: u32 = 10;
const S43: u32 = 15;
const S44: u32 = 21;

// Padding array for MD5
static PADDING: [u8; 64] = [
    0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0,
];

// MD5 basic functions
#[inline]
fn f(x: u32, y: u32, z: u32) -> u32 {
    (x & y) | ((!x) & z)
}

#[inline]
fn g(x: u32, y: u32, z: u32) -> u32 {
    (x & z) | (y & (!z))
}

#[inline]
fn h(x: u32, y: u32, z: u32) -> u32 {
    x ^ y ^ z
}

#[inline]
fn i(x: u32, y: u32, z: u32) -> u32 {
    y ^ (x | (!z))
}

// Rotate left n bits
#[inline]
fn rotate_left(x: u32, n: u32) -> u32 {
    (x << n) | (x >> (32 - n))
}

// FF, GG, HH, and II transformations for rounds 1, 2, 3, and 4
#[inline]
fn ff(a: &mut u32, b: u32, c: u32, d: u32, x: u32, s: u32, ac: u32) {
    *a = a.wrapping_add(f(b, c, d)).wrapping_add(x).wrapping_add(ac);
    *a = rotate_left(*a, s).wrapping_add(b);
}

#[inline]
fn gg(a: &mut u32, b: u32, c: u32, d: u32, x: u32, s: u32, ac: u32) {
    *a = a.wrapping_add(g(b, c, d)).wrapping_add(x).wrapping_add(ac);
    *a = rotate_left(*a, s).wrapping_add(b);
}

#[inline]
fn hh(a: &mut u32, b: u32, c: u32, d: u32, x: u32, s: u32, ac: u32) {
    *a = a.wrapping_add(h(b, c, d)).wrapping_add(x).wrapping_add(ac);
    *a = rotate_left(*a, s).wrapping_add(b);
}

#[inline]
fn ii(a: &mut u32, b: u32, c: u32, d: u32, x: u32, s: u32, ac: u32) {
    *a = a.wrapping_add(i(b, c, d)).wrapping_add(x).wrapping_add(ac);
    *a = rotate_left(*a, s).wrapping_add(b);
}

/// Encode u32 array into u8 array (little-endian)
fn encode_u32_array(input: &[u32], output_len: usize) -> Vec<u8> {
    let mut output = Vec::with_capacity(output_len);

    for i in 0..(output_len / 4) {
        let value = input[i];
        output.push((value & 0xff) as u8);
        output.push(((value >> 8) & 0xff) as u8);
        output.push(((value >> 16) & 0xff) as u8);
        output.push(((value >> 24) & 0xff) as u8);
    }

    output
}

/// Decode u8 array into u32 array (little-endian)
fn decode_u8_array(input: &[u8], input_len: usize) -> Vec<u32> {
    let mut output = Vec::with_capacity(input_len / 4);

    for i in (0..input_len).step_by(4) {
        let value = (input[i] as u32)
            | ((input[i + 1] as u32) << 8)
            | ((input[i + 2] as u32) << 16)
            | ((input[i + 3] as u32) << 24);
        output.push(value);
    }

    output
}

/// Convert MD5 digest to hexadecimal string
pub fn digest_to_hex(digest: &[u8; MD5_DIGEST_LENGTH]) -> String {
    digest.iter().map(|b| format!("{:02x}", b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_md5_empty_string() {
        let digest = Md5::hash(b"");
        let expected = [
            0xd4, 0x1d, 0x8c, 0xd9, 0x8f, 0x00, 0xb2, 0x04, 0xe9, 0x80, 0x09, 0x98, 0xec, 0xf8,
            0x42, 0x7e,
        ];
        assert_eq!(digest, expected);
    }

    #[test]
    fn test_md5_single_char() {
        let digest = Md5::hash(b"a");
        let expected = [
            0x0c, 0xc1, 0x75, 0xb9, 0xc0, 0xf1, 0xb6, 0xa8, 0x31, 0xc3, 0x99, 0xe2, 0x69, 0x77,
            0x26, 0x61,
        ];
        assert_eq!(digest, expected);
    }

    #[test]
    fn test_md5_abc() {
        let digest = Md5::hash(b"abc");
        let expected = [
            0x90, 0x01, 0x50, 0x98, 0x3c, 0xd2, 0x4f, 0xb0, 0xd6, 0x96, 0x3f, 0x7d, 0x28, 0xe1,
            0x7f, 0x72,
        ];
        assert_eq!(digest, expected);
    }

    #[test]
    fn test_md5_hello_world() {
        let digest = Md5::hash(b"hello world");
        let expected = [
            0x5e, 0xb6, 0x3b, 0xbb, 0xe0, 0x1e, 0xee, 0xd0, 0x93, 0xcb, 0x22, 0xbb, 0x8f, 0x5a,
            0xcd, 0xc3,
        ];
        assert_eq!(digest, expected);
    }

    #[test]
    fn test_md5_message_digest() {
        let digest = Md5::hash(b"message digest");
        let expected = [
            0xf9, 0x6b, 0x69, 0x7d, 0x7c, 0xb7, 0x93, 0x8d, 0x52, 0x5a, 0x2f, 0x31, 0xaa, 0xf1,
            0x61, 0xd0,
        ];
        assert_eq!(digest, expected);
    }

    #[test]
    fn test_md5_alphabet() {
        let digest = Md5::hash(b"abcdefghijklmnopqrstuvwxyz");
        let expected = [
            0xc3, 0xfc, 0xd3, 0xd7, 0x61, 0x92, 0xe4, 0x00, 0x7d, 0xfb, 0x49, 0x6c, 0xca, 0x67,
            0xe1, 0x3b,
        ];
        assert_eq!(digest, expected);
    }

    #[test]
    fn test_md5_alphanumeric() {
        let digest = Md5::hash(b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789");
        let expected = [
            0xd1, 0x74, 0xab, 0x98, 0xd2, 0x77, 0xd9, 0xf5, 0xa5, 0x61, 0x1c, 0x2c, 0x9f, 0x41,
            0x9d, 0x9f,
        ];
        assert_eq!(digest, expected);
    }

    #[test]
    fn test_md5_long_sequence() {
        // 80 '1' characters
        let input = "1".repeat(80);
        let digest = Md5::hash(input.as_bytes());
        let expected = [
            0x74, 0x78, 0xba, 0x18, 0x75, 0xf1, 0x75, 0x11, 0xc1, 0x27, 0x40, 0x43, 0x13, 0x36,
            0xa0, 0x9e,
        ];
        assert_eq!(digest, expected);
    }

    #[test]
    fn test_md5_streaming_interface() {
        let mut hasher = Md5::new();
        hasher.update(b"hello ");
        hasher.update(b"world");
        let digest = hasher.finalize();

        let expected = [
            0x5e, 0xb6, 0x3b, 0xbb, 0xe0, 0x1e, 0xee, 0xd0, 0x93, 0xcb, 0x22, 0xbb, 0x8f, 0x5a,
            0xcd, 0xc3,
        ];
        assert_eq!(digest, expected);
    }

    #[test]
    fn test_md5_streaming_vs_oneshot() {
        let data = b"The quick brown fox jumps over the lazy dog";

        // One-shot
        let digest1 = Md5::hash(data);

        // Streaming
        let mut hasher = Md5::new();
        hasher.update(data);
        let digest2 = hasher.finalize();

        assert_eq!(digest1, digest2);
    }

    #[test]
    fn test_md5_streaming_chunks() {
        let data = b"The quick brown fox jumps over the lazy dog";

        // One-shot
        let digest1 = Md5::hash(data);

        // Streaming in chunks
        let mut hasher = Md5::new();
        for chunk in data.chunks(7) {
            hasher.update(chunk);
        }
        let digest2 = hasher.finalize();

        assert_eq!(digest1, digest2);
    }

    #[test]
    fn test_digest_to_hex() {
        let digest = Md5::hash(b"hello world");
        let hex = digest_to_hex(&digest);
        assert_eq!(hex, "5eb63bbbe01eeed093cb22bb8f5acdc3");
    }

    #[test]
    fn test_md5_large_input() {
        // Test with 1000 byte input to ensure block processing works correctly
        let input = vec![0x42; 1000];
        let digest = Md5::hash(&input);

        // Verify we get a consistent result
        let digest2 = Md5::hash(&input);
        assert_eq!(digest, digest2);

        // Verify streaming gives same result
        let mut hasher = Md5::new();
        hasher.update(&input);
        let digest3 = hasher.finalize();
        assert_eq!(digest, digest3);
    }

    #[test]
    fn test_md5_boundary_conditions() {
        // Test various input sizes around block boundaries
        for size in [0, 1, 55, 56, 63, 64, 65, 119, 120, 128] {
            let input = vec![0x5A; size];

            let digest1 = Md5::hash(&input);

            let mut hasher = Md5::new();
            hasher.update(&input);
            let digest2 = hasher.finalize();

            assert_eq!(digest1, digest2, "Failed at size {}", size);
        }
    }

    #[test]
    fn test_md5_clone() {
        let mut hasher1 = Md5::new();
        hasher1.update(b"hello ");

        let mut hasher2 = hasher1.clone();

        hasher1.update(b"world");
        hasher2.update(b"world");

        let digest1 = hasher1.finalize();
        let digest2 = hasher2.finalize();

        assert_eq!(digest1, digest2);
    }
}
