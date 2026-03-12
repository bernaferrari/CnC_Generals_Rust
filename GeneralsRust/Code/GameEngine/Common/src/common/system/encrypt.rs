//! Encryption and Decryption Utilities
//!
//! Provides encryption and decryption functionality for game data.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum EncryptError {
    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),
    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),
    #[error("Invalid key: {0}")]
    InvalidKey(String),
}

/// Basic XOR encryption for simple obfuscation
pub fn xor_encrypt(data: &[u8], key: &[u8]) -> Vec<u8> {
    if key.is_empty() {
        return data.to_vec();
    }

    data.iter()
        .enumerate()
        .map(|(i, &b)| b ^ key[i % key.len()])
        .collect()
}

/// Basic XOR decryption (same as encryption for XOR)
pub fn xor_decrypt(data: &[u8], key: &[u8]) -> Vec<u8> {
    xor_encrypt(data, key)
}

/// Max password length for legacy EncryptString.
pub const MAX_ENCRYPTED_STRING: usize = 8;

/// Legacy Westwood Online password obfuscation.
pub fn encrypt_string(input: &str) -> String {
    const BASE_STRING: &[u8; 64] =
        b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789./";

    let bytes = input.as_bytes();
    let length = bytes.len().min(MAX_ENCRYPTED_STRING);
    let mut temp_buffer = [0u8; MAX_ENCRYPTED_STRING];
    let mut source = [0u8; MAX_ENCRYPTED_STRING + 1];
    source[..length].copy_from_slice(&bytes[..length]);

    for up in 0..length {
        // Match C++ exactly: DnCnt starts at Length and reads the implicit NUL at `source[length]`.
        let dn = length - up;
        let src = source[up];
        let shift = src & 0x01;
        let other = source[dn];
        let shifted = src.wrapping_shl(shift as u32);
        let value = if (src & 0x01) != 0 {
            shifted & other
        } else {
            shifted ^ other
        };
        temp_buffer[up] = value;
    }

    let mut output = String::with_capacity(MAX_ENCRYPTED_STRING);
    for idx in 0..MAX_ENCRYPTED_STRING {
        let mapped = BASE_STRING[(temp_buffer[idx] & 0x3F) as usize];
        output.push(mapped as char);
    }
    output
}

/// Compatibility encryption path used by systems that previously called a richer API.
/// For parity with existing game data usage, this currently delegates to XOR obfuscation.
pub fn advanced_encrypt(data: &[u8], key: &[u8]) -> Result<Vec<u8>, EncryptError> {
    if key.is_empty() {
        return Err(EncryptError::InvalidKey(
            "Advanced encryption key cannot be empty".to_string(),
        ));
    }
    Ok(xor_encrypt(data, key))
}

/// Compatibility decryption path used by systems that previously called a richer API.
/// XOR is symmetric, so this delegates to the same transform as `advanced_encrypt`.
pub fn advanced_decrypt(data: &[u8], key: &[u8]) -> Result<Vec<u8>, EncryptError> {
    if key.is_empty() {
        return Err(EncryptError::InvalidKey(
            "Advanced decryption key cannot be empty".to_string(),
        ));
    }
    Ok(xor_decrypt(data, key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_string_matches_legacy_reference_values() {
        assert_eq!(encrypt_string("abcd"), "agcgaaaa");
        assert_eq!(encrypt_string("password"), "WaIMMsbf");
        assert_eq!(encrypt_string("abc12345"), "axeIaGxI");
    }

    #[test]
    fn advanced_encrypt_round_trip() {
        let data = b"general rust parity";
        let key = b"zh";
        let encrypted = advanced_encrypt(data, key).expect("encryption should succeed");
        assert_ne!(encrypted, data);
        let decrypted = advanced_decrypt(&encrypted, key).expect("decryption should succeed");
        assert_eq!(decrypted, data);
    }

    #[test]
    fn advanced_encrypt_rejects_empty_key() {
        let err = advanced_encrypt(b"abc", b"").expect_err("empty key should fail");
        assert!(matches!(err, EncryptError::InvalidKey(_)));
    }
}
