// Packet encryption/decryption for C++ Generals compatibility
//
// The C++ implementation uses a simple XOR encryption with a rotating mask.
// This is NOT secure encryption - it's obfuscation to prevent casual packet inspection.

/// Encrypt a buffer using the C++ XOR algorithm
///
/// C++ implementation:
/// ```c++
/// static inline void encryptBuf(unsigned char *buf, Int len) {
///     UnsignedInt mask = 0x0000Fade;
///     UnsignedInt *uintPtr = (UnsignedInt *)(buf);
///     for (int i=0; i<len/4; i++) {
///         *uintPtr = (*uintPtr) ^ mask;
///         *uintPtr = htonl(*uintPtr);
///         uintPtr++;
///         mask += 0x00000321;
///     }
/// }
/// ```
///
/// # Arguments
/// * `buf` - Buffer to encrypt (must be mutable)
/// * `len` - Number of bytes to encrypt (only multiples of 4 are encrypted)
pub fn encrypt_buffer(buf: &mut [u8], len: usize) {
    let mut mask: u32 = 0x0000_Fade;

    // Only encrypt whole 4-byte words
    let num_words = len / 4;

    for i in 0..num_words {
        let offset = i * 4;

        // Read 4 bytes as u32 (little-endian on x86)
        let mut value = u32::from_le_bytes([
            buf[offset],
            buf[offset + 1],
            buf[offset + 2],
            buf[offset + 3],
        ]);

        // XOR with mask
        value ^= mask;

        // Convert to network byte order (big-endian)
        value = value.to_be();

        // Write back
        let bytes = value.to_le_bytes();
        buf[offset..offset + 4].copy_from_slice(&bytes);

        // Increment mask
        mask = mask.wrapping_add(0x0000_0321);
    }
}

/// Decrypt a buffer using the C++ XOR algorithm
///
/// C++ implementation:
/// ```c++
/// static inline void decryptBuf(unsigned char *buf, Int len) {
///     UnsignedInt mask = 0x0000Fade;
///     UnsignedInt *uintPtr = (UnsignedInt *)(buf);
///     for (int i=0; i<len/4; i++) {
///         *uintPtr = htonl(*uintPtr);
///         *uintPtr = (*uintPtr) ^ mask;
///         uintPtr++;
///         mask += 0x00000321;
///     }
/// }
/// ```
///
/// # Arguments
/// * `buf` - Buffer to decrypt (must be mutable)
/// * `len` - Number of bytes to decrypt (only multiples of 4 are decrypted)
pub fn decrypt_buffer(buf: &mut [u8], len: usize) {
    let mut mask: u32 = 0x0000_Fade;

    // Only decrypt whole 4-byte words
    let num_words = len / 4;

    for i in 0..num_words {
        let offset = i * 4;

        // Read 4 bytes as u32 (little-endian on x86)
        let mut value = u32::from_le_bytes([
            buf[offset],
            buf[offset + 1],
            buf[offset + 2],
            buf[offset + 3],
        ]);

        // Convert from network byte order (big-endian) to host order
        value = u32::from_be(value);

        // XOR with mask
        value ^= mask;

        // Write back
        let bytes = value.to_le_bytes();
        buf[offset..offset + 4].copy_from_slice(&bytes);

        // Increment mask
        mask = mask.wrapping_add(0x0000_0321);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let original = b"Hello, World! This is a test message for encryption.";
        let mut encrypted = original.to_vec();

        // Encrypt
        let len = encrypted.len();
        encrypt_buffer(&mut encrypted, len);

        // Should be different after encryption
        assert_ne!(&encrypted[..], &original[..]);

        // Decrypt
        let len = encrypted.len();
        decrypt_buffer(&mut encrypted, len);

        // Should match original after decrypt
        assert_eq!(&encrypted[..], &original[..]);
    }

    #[test]
    fn test_encrypt_empty() {
        let mut buf = [];
        encrypt_buffer(&mut buf, 0);
        // Should not panic
    }

    #[test]
    fn test_decrypt_empty() {
        let mut buf = [];
        decrypt_buffer(&mut buf, 0);
        // Should not panic
    }

    #[test]
    fn test_encrypt_partial_word() {
        // Only 3 bytes - should not encrypt the partial word
        let original = [0x01, 0x02, 0x03];
        let mut buf = original;
        let len = buf.len();
        encrypt_buffer(&mut buf, len);

        // Should remain unchanged (no complete 4-byte word)
        assert_eq!(&buf[..], &original[..]);
    }

    #[test]
    fn test_encrypt_exact_word() {
        let mut buf = [0x00, 0x00, 0x00, 0x00];
        encrypt_buffer(&mut buf, 4);

        // XOR with 0x0000Fade, then htonl
        let expected_value = 0x0000_Fade_u32.to_be();
        let expected_bytes = expected_value.to_le_bytes();

        assert_eq!(&buf[..], &expected_bytes[..]);
    }

    #[test]
    fn test_mask_increments() {
        let mut buf = [0u8; 8]; // Two 4-byte words
        encrypt_buffer(&mut buf, 8);

        // First word should be encrypted with mask 0x0000Fade
        // Second word should be encrypted with mask 0x0000Fade + 0x321 = 0x0000FDEF

        // Decrypt and verify masks were applied
        decrypt_buffer(&mut buf, 8);
        assert_eq!(&buf[..], &[0u8; 8][..]);
    }

    #[test]
    fn test_encrypt_deterministic() {
        let data = b"Test message for encryption!";

        let mut buf1 = data.to_vec();
        let mut buf2 = data.to_vec();

        let len1 = buf1.len();
        encrypt_buffer(&mut buf1, len1);
        let len2 = buf2.len();
        encrypt_buffer(&mut buf2, len2);

        assert_eq!(buf1, buf2);
    }
}
