//! String system for Command & Conquer Generals Zero Hour
//!
//! Strings use owned standard-library storage and retain a terminating NUL
//! for C-string interoperation.

use base_types::*;
use std::cmp::Ordering;
use std::ffi::CStr;
use std::fmt;

/// Maximum length of any string in characters
pub const MAX_STRING_LEN: usize = 32767;

/// Maximum length of format buffer
pub const MAX_FORMAT_BUF_LEN: usize = 2048;

/// An owned byte string with a trailing C terminator.
#[derive(Clone)]
pub struct AsciiString {
    data: Vec<u8>,
}

impl AsciiString {
    /// The empty string constant
    pub const THE_EMPTY_STRING: AsciiString = AsciiString { data: Vec::new() };

    /// Create a new empty AsciiString.
    pub fn new() -> Self {
        Self::THE_EMPTY_STRING
    }

    /// Create an AsciiString from a &str
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Result<Self, String> {
        Self::from_bytes(s.as_bytes())
    }

    /// Create a string from bytes, using the first NUL as the C terminator.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let capacity = bytes.len() + 1;
        if capacity > MAX_STRING_LEN {
            return Err(format!("String capacity {} exceeds maximum {}", capacity, MAX_STRING_LEN));
        }
        let visible = bytes.split(|b| *b == 0).next().unwrap_or(&[]);
        let mut data = Vec::with_capacity(visible.len() + 1);
        data.extend_from_slice(visible);
        data.push(0);
        Ok(Self { data })
    }

    /// Create an AsciiString from a borrowed C string, or empty for a null input.
    pub fn from_c_str(s: Option<&CStr>) -> Result<Self, String> {
        let Some(s) = s else {
            return Ok(Self::THE_EMPTY_STRING);
        };
        Self::from_bytes(s.to_bytes())
    }

    /// Get the length of the string in characters
    pub fn get_length(&self) -> usize {
        self.as_bytes().len()
    }

    /// Check if the string is empty
    pub fn is_empty(&self) -> Bool {
        self.get_length() == 0
    }

    /// Clear the string (make it empty)
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// Get a const pointer to the string data
    pub fn str(&self) -> *const libc::c_char {
        if self.data.is_empty() {
            c"".as_ptr()
        } else {
            self.data.as_ptr().cast()
        }
    }

    /// Borrow the NUL-terminated bytes as a C string.
    pub fn as_c_str(&self) -> &CStr {
        if self.data.is_empty() {
            c""
        } else {
            CStr::from_bytes_with_nul(&self.data).expect("string terminator invariant")
        }
    }

    /// Return the exact bytes before the first NUL.
    pub fn as_bytes(&self) -> &[u8] {
        self.data.strip_suffix(&[0]).unwrap_or(&[])
    }

    /// Decode as UTF-8, replacing invalid byte sequences.
    pub fn as_str(&self) -> std::borrow::Cow<'_, str> {
        String::from_utf8_lossy(self.as_bytes())
    }

    /// Decode the exact bytes as UTF-8 without replacement.
    pub fn try_as_str(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(self.as_bytes())
    }

    /// Get a byte at the specified index, represented as a character.
    pub fn get_char_at(&self, index: usize) -> Option<char> {
        self.as_bytes().get(index).copied().map(char::from)
    }

    /// Set the string from another AsciiString
    pub fn set(&mut self, other: &AsciiString) {
        self.data.clone_from(&other.data);
    }

    /// Set the string from a &str
    pub fn set_str(&mut self, s: &str) -> Result<(), String> {
        let capacity = s.len() + 1;
        if capacity > MAX_STRING_LEN {
            return Err(format!("String capacity {} exceeds maximum {}", capacity, MAX_STRING_LEN));
        }
        let visible = s.split('\0').next().unwrap_or("");
        self.data.clear();
        self.data.extend_from_slice(visible.as_bytes());
        self.data.push(0);
        Ok(())
    }

    /// Concatenate another AsciiString to this one
    pub fn concat(&mut self, other: &AsciiString) -> Result<(), String> {
        self.concat_bytes(other.as_bytes())
    }

    /// Concatenate a &str to this string
    pub fn concat_str(&mut self, s: &str) -> Result<(), String> {
        let capacity = self.get_length() + s.len() + 1;
        if capacity > MAX_STRING_LEN {
            return Err(format!("String capacity {} exceeds maximum {}", capacity, MAX_STRING_LEN));
        }
        self.concat_bytes(s.split('\0').next().unwrap_or("").as_bytes())
    }

    fn concat_bytes(&mut self, s: &[u8]) -> Result<(), String> {
        if s.is_empty() {
            return Ok(());
        }
        let capacity = self.get_length() + s.len() + 1;
        if capacity > MAX_STRING_LEN {
            return Err(format!("String capacity {} exceeds maximum {}", capacity, MAX_STRING_LEN));
        }
        if !self.data.is_empty() {
            self.data.pop();
        }
        self.data.extend_from_slice(s);
        self.data.push(0);
        Ok(())
    }

    /// Concatenate a single character to this string
    pub fn concat_char(&mut self, c: char) -> Result<(), String> {
        let mut bytes = [0; 4];
        self.concat_str(c.encode_utf8(&mut bytes))
    }

    /// Remove leading and trailing whitespace
    pub fn trim(&mut self) -> Result<(), String> {
        let bytes = self.as_bytes();
        let start = bytes.iter().position(|b| !b.is_ascii_whitespace()).unwrap_or(bytes.len());
        let end = bytes.iter().rposition(|b| !b.is_ascii_whitespace()).map_or(start, |i| i + 1);
        self.data.copy_within(start..end, 0);
        self.data.truncate(end - start);
        self.data.push(0);
        Ok(())
    }

    /// Convert the string to lowercase
    pub fn to_lower(&mut self) -> Result<(), String> {
        self.data.make_ascii_lowercase();
        Ok(())
    }

    /// Remove the last character from the string
    pub fn remove_last_char(&mut self) {
        if !self.data.is_empty() {
            self.data.pop();
            self.data.pop();
            self.data.push(0);
        }
    }

    /// Format the string using sprintf-style formatting
    pub fn format(format_str: &str, args: &[&dyn fmt::Display]) -> Result<Self, String> {
        let mut formatted = String::new();

        // Simple format implementation (could be enhanced)
        let mut arg_iter = args.iter();
        let mut chars = format_str.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '%' {
                if let Some(next_ch) = chars.peek() {
                    match next_ch {
                        '%' => {
                            formatted.push('%');
                            chars.next();
                        }
                        's' | 'd' | 'f' => {
                            if let Some(arg) = arg_iter.next() {
                                formatted.push_str(&format!("{}", arg));
                            } else {
                                return Err("Not enough arguments for format string".to_string());
                            }
                            chars.next();
                        }
                        _ => {
                            formatted.push(ch);
                        }
                    }
                } else {
                    formatted.push(ch);
                }
            } else {
                formatted.push(ch);
            }
        }

        if formatted.len() > MAX_FORMAT_BUF_LEN {
            return Err(format!(
                "Formatted string too long: {} > {}",
                formatted.len(),
                MAX_FORMAT_BUF_LEN
            ));
        }

        Self::from_str(&formatted)
    }

    /// Find the first occurrence of a substring
    pub fn find(&self, substring: &str) -> Option<usize> {
        let needle = substring.as_bytes();
        if needle.is_empty() { Some(0) } else { self.as_bytes().windows(needle.len()).position(|w| w == needle) }
    }

    /// Find the last occurrence of a substring
    pub fn reverse_find(&self, substring: &str) -> Option<usize> {
        let needle = substring.as_bytes();
        if needle.is_empty() { Some(self.get_length()) } else { self.as_bytes().windows(needle.len()).rposition(|w| w == needle) }
    }

    /// Extract a substring
    pub fn substr(&self, start: usize, length: usize) -> Result<Self, String> {
        let bytes = self.as_bytes();
        if start >= bytes.len() {
            return Ok(Self::THE_EMPTY_STRING);
        }
        let end = start.saturating_add(length).min(bytes.len());
        Self::from_bytes(&bytes[start..end])
    }

    /// Compare two strings
    pub fn compare(&self, other: &AsciiString) -> Ordering {
        self.as_bytes().cmp(other.as_bytes())
    }

    /// Compare with a &str
    pub fn compare_str(&self, other: &str) -> Ordering {
        self.as_bytes().cmp(other.as_bytes())
    }

    /// Check if the string starts with a prefix
    pub fn starts_with(&self, prefix: &str) -> bool {
        self.as_bytes().starts_with(prefix.as_bytes())
    }

    /// Check if the string ends with a suffix
    pub fn ends_with(&self, suffix: &str) -> bool {
        self.as_bytes().ends_with(suffix.as_bytes())
    }

    /// Replace all occurrences of a substring
    pub fn replace(&mut self, from: &str, to: &str) -> Result<(), String> {
        let from = from.as_bytes();
        if from.is_empty() {
            if let Ok(text) = std::str::from_utf8(self.as_bytes()) {
                let result = text.replace("", to);
                return self.set_str(&result);
            }
            return Err("Cannot replace an empty string in invalid UTF-8".to_string());
        }
        let to = to.split('\0').next().unwrap_or("").as_bytes();
        let mut result = Vec::new();
        let mut remaining = self.as_bytes();
        while let Some(index) = remaining.windows(from.len()).position(|w| w == from) {
            result.extend_from_slice(&remaining[..index]);
            result.extend_from_slice(to);
            remaining = &remaining[index + from.len()..];
        }
        result.extend_from_slice(remaining);
        *self = Self::from_bytes(&result)?;
        Ok(())
    }

    /// Convert to UnicodeString
    #[cfg(feature = "unicode")]
    pub fn to_unicode(&self) -> UnicodeString {
        UnicodeString::from_ascii(self)
    }
}

impl Default for AsciiString {
    fn default() -> Self {
        Self::new()
    }
}


impl fmt::Display for AsciiString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl fmt::Debug for AsciiString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AsciiString(\"{}\")", self.as_str())
    }
}

impl PartialEq for AsciiString {
    fn eq(&self, other: &Self) -> bool {
        self.compare(other) == Ordering::Equal
    }
}

impl Eq for AsciiString {}

impl PartialOrd for AsciiString {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for AsciiString {
    fn cmp(&self, other: &Self) -> Ordering {
        self.compare(other)
    }
}

impl From<&str> for AsciiString {
    fn from(s: &str) -> Self {
        Self::from_str(s).unwrap_or(Self::THE_EMPTY_STRING)
    }
}

impl From<String> for AsciiString {
    fn from(s: String) -> Self {
        Self::from_str(&s).unwrap_or(Self::THE_EMPTY_STRING)
    }
}

impl std::ops::Add for AsciiString {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        let mut result = self;
        let _ = result.concat(&other); // Ignore error for operator
        result
    }
}

impl std::ops::Add<&str> for AsciiString {
    type Output = Self;

    fn add(self, other: &str) -> Self {
        let mut result = self;
        let _ = result.concat_str(other); // Ignore error for operator
        result
    }
}

impl std::ops::AddAssign<&AsciiString> for AsciiString {
    fn add_assign(&mut self, other: &Self) {
        let _ = self.concat(other); // Ignore error for operator
    }
}

impl std::ops::AddAssign<&str> for AsciiString {
    fn add_assign(&mut self, other: &str) {
        let _ = self.concat_str(other); // Ignore error for operator
    }
}

#[cfg(feature = "unicode")]
/// An owned, NUL-terminated UTF-16 string.
#[derive(Clone)]
pub struct UnicodeString {
    data: Vec<u16>,
}

#[cfg(feature = "unicode")]
impl UnicodeString {
    /// The empty string constant
    pub const THE_EMPTY_STRING: UnicodeString = UnicodeString { data: Vec::new() };

    /// Create a new empty UnicodeString.
    pub fn new() -> Self {
        Self::THE_EMPTY_STRING
    }

    /// Translate each byte like the original C++ implementation.
    ///
    /// The original Windows target uses signed `char` and 16-bit `WideChar`;
    /// consequently bytes 0x80..=0xff sign-extend to 0xff80..=0xffff.
    pub fn from_ascii(ascii: &AsciiString) -> Self {
        let mut data = Vec::with_capacity(ascii.get_length() + 1);
        data.extend(ascii.as_bytes().iter().map(|&byte| (byte as i8 as i16) as u16));
        data.push(0);
        Self { data }
    }

    /// Create a UnicodeString from a &str
    pub fn from_str(s: &str) -> Result<Self, String> {
        let capacity = s.encode_utf16().count() + 1;
        if capacity > MAX_STRING_LEN {
            return Err(format!("String capacity {} exceeds maximum {}", capacity, MAX_STRING_LEN));
        }
        let mut data = Vec::with_capacity(capacity);
        data.extend(s.encode_utf16().take_while(|unit| *unit != 0));
        data.push(0);
        Ok(Self { data })
    }

    /// Set the string contents from a &str.
    pub fn set_str(&mut self, s: &str) -> Result<(), String> {
        if s.is_empty() {
            self.clear();
            return Ok(());
        }

        let mut new_value = UnicodeString::from_str(s)?;
        std::mem::swap(self, &mut new_value);
        Ok(())
    }

    /// Get the length of the string in characters
    pub fn get_length(&self) -> usize {
        self.data.len().saturating_sub(1)
    }

    /// Check if the string is empty
    pub fn is_empty(&self) -> Bool {
        self.get_length() == 0
    }

    /// Clear the string
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// Convert to an owned UTF-8 string, replacing invalid UTF-16 sequences.
    pub fn as_str(&self) -> String {
        String::from_utf16_lossy(self.data.strip_suffix(&[0]).unwrap_or(&[]))
    }

    /// Narrow each UTF-16 code unit to the low byte, matching C++ `char`.
    /// A zero low byte is skipped, as concatenating a C NUL adds no character.
    pub fn to_ascii(&self) -> AsciiString {
        let units = self.data.strip_suffix(&[0]).unwrap_or(&[]);
        let mut data = Vec::with_capacity(units.len() + 1);
        data.extend(units.iter().map(|&unit| unit as u8).filter(|&byte| byte != 0));
        data.push(0);
        AsciiString { data }
    }
}


#[cfg(feature = "unicode")]
impl fmt::Display for UnicodeString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(feature = "unicode")]
impl fmt::Debug for UnicodeString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UnicodeString(\"{}\")", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ascii_string_creation() {
        let empty = AsciiString::new();
        assert!(empty.is_empty());
        assert_eq!(empty.get_length(), 0);

        let s = AsciiString::from_str("Hello World").unwrap();
        assert!(!s.is_empty());
        assert_eq!(s.get_length(), 11);
        assert_eq!(s.as_str(), "Hello World");
    }

    #[test]
    fn ascii_clone_is_independent() {
        let original = AsciiString::from_str("Original").unwrap();
        let mut changed = original.clone();
        changed.set_str("Modified").unwrap();
        assert_eq!(original.as_str(), "Original");
        assert_eq!(changed.as_str(), "Modified");
    }

    #[test]
    fn bytes_c_terminator_and_byte_truncation() {
        let source = CStr::from_bytes_with_nul(b"\xff\xc3\xa9\0").unwrap();
        let mut value = AsciiString::from_c_str(Some(source)).unwrap();
        let original = value.clone();
        assert_eq!(value.get_length(), 3);
        assert_eq!(value.as_c_str().to_bytes_with_nul(), b"\xff\xc3\xa9\0");
        assert_eq!(value.substr(1, 1).unwrap().as_bytes(), b"\xc3");
        value.remove_last_char();
        assert_eq!(value.as_bytes(), b"\xff\xc3");
        assert_eq!(original.as_bytes(), b"\xff\xc3\xa9");
        assert_eq!(AsciiString::from_str("a\0b").unwrap().as_c_str().to_bytes_with_nul(), b"a\0");
        assert_eq!(AsciiString::new().as_c_str().to_bytes_with_nul(), b"\0");
    }

    #[test]
    fn byte_capacity_includes_terminator_and_failed_mutation_is_atomic() {
        let accepted = "é".repeat((MAX_STRING_LEN - 1) / 2);
        let mut value = AsciiString::from_str(&accepted).unwrap();
        assert_eq!(value.get_length(), MAX_STRING_LEN - 1);
        assert!(value.concat_str("x").is_err());
        assert_eq!(value.as_str(), accepted);
        assert!(value.set_str(&"x".repeat(MAX_STRING_LEN)).is_err());
        assert_eq!(value.get_length(), MAX_STRING_LEN - 1);
    }

    #[cfg(feature = "unicode")]
    #[test]
    fn utf16_limits_nuls_and_clones_use_code_units() {
        let accepted = "😀".repeat((MAX_STRING_LEN - 1) / 2);
        let mut value = UnicodeString::from_str(&accepted).unwrap();
        let original = value.clone();
        assert_eq!(value.get_length(), MAX_STRING_LEN - 1);
        assert!(value.set_str(&(accepted.clone() + "x")).is_err());
        assert_eq!(value.as_str(), accepted);
        value.set_str("😀\0hidden").unwrap();
        assert_eq!(value.get_length(), 2);
        assert_eq!(value.as_str(), "😀");
        assert_eq!(original.get_length(), MAX_STRING_LEN - 1);
    }

    #[test]
    fn test_ascii_string_concatenation() {
        let mut s = AsciiString::from_str("Hello").unwrap();
        let _ = s.concat_str(" World");
        assert_eq!(s.as_str(), "Hello World");
        assert_eq!(s.get_length(), 11);
    }

    #[test]
    fn test_ascii_string_trim() {
        let mut s = AsciiString::from_str("  Hello World  ").unwrap();
        let _ = s.trim();
        assert_eq!(s.as_str(), "Hello World");
    }

    #[test]
    fn test_ascii_string_case_conversion() {
        let mut s = AsciiString::from_str("Hello World").unwrap();
        let _ = s.to_lower();
        assert_eq!(s.as_str(), "hello world");
    }

    #[test]
    fn test_ascii_string_substring() {
        let s = AsciiString::from_str("Hello World").unwrap();
        let substr = s.substr(6, 5).unwrap();
        assert_eq!(substr.as_str(), "World");
    }

    #[test]
    fn test_ascii_string_find() {
        let s = AsciiString::from_str("Hello World").unwrap();
        assert_eq!(s.find("World"), Some(6));
        assert_eq!(s.find("Universe"), None);
    }

    #[test]
    fn test_ascii_string_comparison() {
        let s1 = AsciiString::from_str("Apple").unwrap();
        let s2 = AsciiString::from_str("Banana").unwrap();
        let s3 = AsciiString::from_str("Apple").unwrap();

        assert!(s1 < s2);
        assert_eq!(s1, s3);
        assert!(s1.compare(&s2) == Ordering::Less);
    }

    #[test]
    fn test_ascii_string_format() {
        let formatted = AsciiString::format("Hello %s!", &[&"World"]).unwrap();
        assert_eq!(formatted.as_str(), "Hello World!");
    }

    #[test]
    fn test_ascii_string_operators() {
        let s1 = AsciiString::from_str("Hello").unwrap();
        let s2 = AsciiString::from_str(" World").unwrap();

        let combined = s1 + s2;
        assert_eq!(combined.as_str(), "Hello World");

        let mut s3 = AsciiString::from_str("Hello").unwrap();
        s3 += " World";
        assert_eq!(s3.as_str(), "Hello World");
    }

    #[cfg(feature = "unicode")]
    #[test]
    fn test_unicode_string_creation() {
        let empty = UnicodeString::new();
        assert!(empty.is_empty());

        let s = UnicodeString::from_str("Hello World").unwrap();
        assert!(!s.is_empty());
        assert_eq!(s.get_length(), 11);
    }

    #[cfg(feature = "unicode")]
    #[test]
    fn unicode_clone_survives_mutation_and_drop_order() {
        let original = UnicodeString::from_str("original").unwrap();
        let mut changed = original.clone();
        let survivor = original.clone();
        changed.set_str("changed").unwrap();
        drop(original);
        assert_eq!(survivor.as_str(), "original");
        assert_eq!(changed.as_str(), "changed");
        drop(changed);
        assert_eq!(survivor.as_str(), "original");

        let survivor = {
            let source = UnicodeString::from_str("retained").unwrap();
            source.clone()
        };
        assert_eq!(survivor.as_str(), "retained");
    }

    #[test]
    fn ascii_many_clones_remain_valid() {
        let original = AsciiString::from_str("retained").unwrap();
        let copies: Vec<_> = (0..u16::MAX).map(|_| original.clone()).collect();
        assert_eq!(copies.last().unwrap().as_str(), "retained");
        drop(copies);
        assert_eq!(original.as_str(), "retained");
    }

    #[cfg(feature = "unicode")]
    #[test]
    fn test_ascii_to_unicode_conversion() {
        let ascii = AsciiString::from_str("Hello World").unwrap();
        let unicode = ascii.to_unicode();
        let back_to_ascii = unicode.to_ascii();

        assert_eq!(ascii.as_str(), back_to_ascii.as_str());
    }

    #[cfg(feature = "unicode")]
    #[test]
    fn cpp_signed_byte_widening_and_code_unit_narrowing() {
        let bytes = AsciiString::from_bytes(b"A\x7f\x80\xff").unwrap();
        assert!(bytes.try_as_str().is_err());
        let wide = bytes.to_unicode();
        assert_eq!(wide.get_length(), 4);
        assert_eq!(wide.as_str(), "A\u{7f}\u{ff80}\u{ffff}");
        assert_eq!(wide.to_ascii().as_bytes(), b"A\x7f\x80\xff");

        // U+0100 and the low surrogate of 😀 narrow to NUL; C++ concat
        // leaves the destination unchanged for each of those code units.
        let wide = UnicodeString::from_str("A\u{100}😀€Z").unwrap();
        assert_eq!(wide.to_ascii().as_c_str().to_bytes_with_nul(), b"A=\xacZ\0");
        let wide = UnicodeString::from_str("A\0B").unwrap();
        assert_eq!(wide.to_ascii().as_bytes(), b"A");
    }

    #[cfg(feature = "unicode")]
    #[test]
    fn repeated_unicode_conversion_produces_independent_owned_text() {
        let mut value = UnicodeString::from_str("A😀Z").unwrap();
        let first = value.as_str();
        let second = value.as_str();
        assert_eq!(value.get_length(), 4);
        value.set_str("updated").unwrap();
        assert_eq!(first, "A😀Z");
        assert_eq!(second, "A😀Z");
        assert_eq!(value.to_ascii().as_str(), "updated");
    }
}
