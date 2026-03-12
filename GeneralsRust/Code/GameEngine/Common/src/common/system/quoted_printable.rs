////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// FILE: quoted_printable.rs //////////////////////////////////////////////////
// Author: Matt Campbell, February 2002
// Description: Quoted-printable encode/decode
///////////////////////////////////////////////////////////////////////////////

/// Magic character used for encoding non-alphanumeric characters.
const MAGIC_CHAR: u8 = b'_';

/// Maximum destination bytes used by legacy C++ helpers.
const MAX_BUFFER_SIZE: usize = 1024;

/// Convert an integer (0-15) to its ASCII hex digit representation.
fn int_to_hex_digit(num: u8) -> Option<u8> {
    match num {
        0..=9 => Some(b'0' + num),
        10..=15 => Some(b'A' + (num - 10)),
        _ => None,
    }
}

/// Convert an ASCII hex digit to its integer value.
fn hex_digit_to_int(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => 0,
    }
}

/// Check if a character is alphanumeric.
fn is_alnum(c: u8) -> bool {
    c.is_ascii_alphanumeric()
}

fn push_encoded_byte(out: &mut Vec<u8>, byte: u8, max_len: usize) -> bool {
    if is_alnum(byte) {
        if out.len() + 1 > max_len {
            return false;
        }
        out.push(byte);
    } else {
        if out.len() + 3 > max_len {
            return false;
        }
        out.push(MAGIC_CHAR);
        out.push(int_to_hex_digit(byte >> 4).unwrap_or(b'0'));
        out.push(int_to_hex_digit(byte & 0x0f).unwrap_or(b'0'));
    }
    true
}

fn decode_quoted_printable_bytes(original: &str, max_len: usize) -> Vec<u8> {
    let mut result = Vec::with_capacity(original.len().min(max_len));
    let bytes = original.as_bytes();

    let mut i = 0usize;
    while i < bytes.len() && result.len() < max_len {
        if bytes[i] == MAGIC_CHAR {
            if i + 1 >= bytes.len() {
                break;
            }
            let mut value = hex_digit_to_int(bytes[i + 1]);
            i += 1;
            if i + 1 < bytes.len() {
                value = (value << 4) | hex_digit_to_int(bytes[i + 1]);
                i += 1;
            }
            result.push(value);
        } else {
            result.push(bytes[i]);
        }
        i += 1;
    }

    result
}

/// Convert Unicode string to quoted-printable ASCII string.
pub fn unicode_string_to_quoted_printable(original: &str) -> String {
    let mut result = Vec::with_capacity(MAX_BUFFER_SIZE);
    let max_output = MAX_BUFFER_SIZE - 1;

    for unit in original.encode_utf16() {
        let [lo, hi] = unit.to_le_bytes();
        if !push_encoded_byte(&mut result, lo, max_output) {
            break;
        }
        if !push_encoded_byte(&mut result, hi, max_output) {
            break;
        }
    }

    String::from_utf8_lossy(&result).into_owned()
}

/// Convert ASCII string to quoted-printable ASCII string.
pub fn ascii_string_to_quoted_printable(original: &str) -> String {
    let mut result = Vec::with_capacity(MAX_BUFFER_SIZE);
    let max_output = MAX_BUFFER_SIZE - 1;

    for &byte in original.as_bytes() {
        if !push_encoded_byte(&mut result, byte, max_output) {
            break;
        }
    }

    String::from_utf8_lossy(&result).into_owned()
}

/// Convert ASCII quoted-printable string to Unicode string.
pub fn quoted_printable_to_unicode_string(original: &str) -> String {
    let mut decoded = decode_quoted_printable_bytes(original, (MAX_BUFFER_SIZE * 2) - 1);

    // Legacy C++ helper appends one byte on even-length payloads before final terminator.
    if decoded.len() % 2 == 0 {
        decoded.push(0);
    }
    decoded.push(0);

    let mut units = Vec::with_capacity(decoded.len() / 2);
    for pair in decoded.chunks_exact(2) {
        let unit = u16::from_le_bytes([pair[0], pair[1]]);
        if unit == 0 {
            break;
        }
        units.push(unit);
    }

    String::from_utf16_lossy(&units)
}

/// Convert ASCII quoted-printable string to ASCII string.
pub fn quoted_printable_to_ascii_string(original: &str) -> String {
    let result = decode_quoted_printable_bytes(original, MAX_BUFFER_SIZE - 1);
    String::from_utf8_lossy(&result).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_int_to_hex_digit() {
        assert_eq!(int_to_hex_digit(0), Some(b'0'));
        assert_eq!(int_to_hex_digit(9), Some(b'9'));
        assert_eq!(int_to_hex_digit(10), Some(b'A'));
        assert_eq!(int_to_hex_digit(15), Some(b'F'));
        assert_eq!(int_to_hex_digit(16), None);
    }

    #[test]
    fn test_hex_digit_to_int() {
        assert_eq!(hex_digit_to_int(b'0'), 0);
        assert_eq!(hex_digit_to_int(b'9'), 9);
        assert_eq!(hex_digit_to_int(b'A'), 10);
        assert_eq!(hex_digit_to_int(b'F'), 15);
        assert_eq!(hex_digit_to_int(b'a'), 10);
        assert_eq!(hex_digit_to_int(b'f'), 15);
    }

    #[test]
    fn test_ascii_quoted_printable_roundtrip() {
        let original = "Hello World! @#$";
        let encoded = ascii_string_to_quoted_printable(original);
        let decoded = quoted_printable_to_ascii_string(&encoded);

        // Alphanumeric characters remain unchanged.
        assert!(encoded.contains("Hello"));
        assert!(encoded.contains("World"));
        // Non-alphanumeric characters are escaped.
        assert!(encoded.contains("_"));
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_unicode_quoted_printable_roundtrip() {
        let original = "Hello Ω";
        let encoded = unicode_string_to_quoted_printable(original);
        let decoded = quoted_printable_to_unicode_string(&encoded);

        // ASCII bytes remain visible in encoded output.
        assert!(encoded.contains("Hello"));
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_special_characters_encoding() {
        let original = "!@#$%";
        let encoded = ascii_string_to_quoted_printable(original);
        let decoded = quoted_printable_to_ascii_string(&encoded);

        // Encoding uses the magic marker.
        assert!(encoded.contains("_"));
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_empty_string() {
        let original = "";
        let encoded = ascii_string_to_quoted_printable(original);
        let decoded = quoted_printable_to_ascii_string(&encoded);

        assert_eq!(encoded, "");
        assert_eq!(decoded, "");
    }

    #[test]
    fn test_alphanumeric_unchanged() {
        let original = "abc123XYZ";
        let encoded = ascii_string_to_quoted_printable(original);

        // Alphanumeric characters should not be encoded.
        assert_eq!(encoded, original);

        let decoded = quoted_printable_to_ascii_string(&encoded);
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_ascii_decode_trailing_magic_matches_legacy() {
        assert_eq!(quoted_printable_to_ascii_string("Test_"), "Test");
    }

    #[test]
    fn test_ascii_decode_single_nibble_matches_legacy() {
        assert_eq!(quoted_printable_to_ascii_string("_A"), "\n");
    }
}
