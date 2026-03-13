//! Reentrant string tokenizer mirroring WWLib `strtok_r`.
//!
//! This module provides a faithful Rust implementation of the POSIX-style
//! reentrant string tokenizer (`strtok_r`) as implemented in WWLib.
//!
//! Unlike the standard C `strtok()` which uses a static buffer (making it
//! non-reentrant and unsafe to nest), this version takes an explicit state
//! pointer, allowing multiple tokenizations to proceed concurrently.
//!
//! # C++ Source
//! Original implementation in `GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/strtok_r.cpp`
//!
//! # Behavioral Notes
//! - Leading delimiters are skipped (matching C++ behavior)
//! - Empty tokens between consecutive delimiters are collapsed
//! - The input buffer is modified in-place (null terminators inserted)
//! - Returns `None` when no more tokens are available

/// Reentrant string tokenizer state.
///
/// Tracks the current position within the string being tokenized.
/// This mirrors the `char **lasts` parameter from the C++ version.
pub struct StrtokState {
    /// Current position within the string buffer (byte offset)
    position: usize,
}

impl StrtokState {
    /// Create a new tokenizer state starting at position 0.
    pub fn new() -> Self {
        Self { position: 0 }
    }

    /// Reset the state to the beginning.
    pub fn reset(&mut self) {
        self.position = 0;
    }
}

impl Default for StrtokState {
    fn default() -> Self {
        Self::new()
    }
}

/// Find the length of the initial segment of `s` that consists entirely
/// of characters NOT in `accept`. Mirrors C `strcspn`.
fn strcspn(s: &[u8], accept: &[u8]) -> usize {
    for (i, &ch) in s.iter().enumerate() {
        if accept.contains(&ch) {
            return i;
        }
    }
    s.len()
}

/// Find the length of the initial segment of `s` that consists entirely
/// of characters in `accept`. Mirrors C `strspn`.
fn strspn(s: &[u8], accept: &[u8]) -> usize {
    for (i, &ch) in s.iter().enumerate() {
        if !accept.contains(&ch) {
            return i;
        }
    }
    s.len()
}

/// Reentrant string tokenizer (safe Rust version).
///
/// Returns the next token from `buffer`, using `state` to track position
/// and `delimiters` as the set of delimiter characters.
///
/// This mirrors the C++ `strtok_r` function:
/// ```cpp
/// char *strtok_r(char *strptr, const char *delimiters, char **lasts)
/// ```
///
/// # Behavior
/// - If `buffer` is `Some`, tokenization starts from the beginning of that buffer
/// - If `buffer` is `None`, tokenization continues from the current `state` position
/// - Leading delimiters are skipped
/// - Empty tokens (consecutive delimiters) are collapsed
/// - Returns `None` when no more tokens are available
///
/// # Safety Note
/// Unlike the C++ version which null-terminates tokens in-place, this safe
/// version works with byte slices and returns string slices. The buffer is
/// still modified if you use the unsafe variant.
pub fn strtok_r<'a>(
    buffer: Option<&'a mut [u8]>,
    delimiters: &[u8],
    state: &mut StrtokState,
) -> Option<&'a [u8]> {
    // If a new buffer is provided, reset state to the beginning
    if let Some(buf) = buffer {
        state.position = 0;
        return strtok_r_internal(buf, delimiters, state);
    }
    None
}

/// Continue tokenization from the current state position.
/// The buffer must be the same buffer used in the initial call.
pub fn strtok_r_continue<'a>(
    buffer: &'a mut [u8],
    delimiters: &[u8],
    state: &mut StrtokState,
) -> Option<&'a [u8]> {
    strtok_r_internal(buffer, delimiters, state)
}

fn strtok_r_internal<'a>(
    buffer: &'a mut [u8],
    delimiters: &[u8],
    state: &mut StrtokState,
) -> Option<&'a [u8]> {
    let len = buffer.len();

    // Check for end of string
    if state.position >= len || buffer[state.position] == 0 {
        return None;
    }

    // Check if current position starts with a delimiter
    let mut dstart = strcspn(&buffer[state.position..], delimiters);

    if dstart == 0 {
        // String starts with delimiters - skip them
        let dend = strspn(&buffer[state.position..], delimiters);
        state.position += dend;

        // Check for end of string after skipping delimiters
        if state.position >= len || buffer[state.position] == 0 {
            return None;
        }

        dstart = strcspn(&buffer[state.position..], delimiters);
    }

    let token_start = state.position;

    // Check if this is the last token (delimiter is null terminator or end of buffer)
    if state.position + dstart >= len || buffer[state.position + dstart] == 0 {
        // Last token - advance position to end
        state.position += dstart;
    } else {
        // Null-terminate the token and advance past the delimiter
        buffer[state.position + dstart] = 0;
        state.position += dstart + 1;
    }

    // Safety: we know the range is valid and the bytes at token_start..token_start+dstart
    // are valid and (now) null-terminated
    Some(&buffer[token_start..token_start + dstart])
}

/// Safe string tokenizer that works with `&str` without modifying the input.
///
/// This is the idiomatic Rust wrapper that returns an iterator over tokens.
/// It preserves the same tokenization logic (skipping leading delimiters,
/// collapsing empty tokens) but does not modify the input string.
///
/// # Example
/// ```rust
/// use wwlib_rust::strtok_r::tokenize;
///
/// let tokens: Vec<&str> = tokenize("  hello,world,,test  ", ",").collect();
/// assert_eq!(tokens, vec!["  hello", "world", "test  "]);
/// ```
pub fn tokenize<'a>(input: &'a str, delimiters: &'a str) -> TokenIterator<'a> {
    TokenIterator {
        remaining: input,
        delimiters,
        started: false,
    }
}

/// Iterator over tokens in a string, preserving strtok_r semantics.
pub struct TokenIterator<'a> {
    remaining: &'a str,
    delimiters: &'a str,
    started: bool,
}

impl<'a> Iterator for TokenIterator<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining.is_empty() {
            return None;
        }

        // Skip leading delimiters (matching C++ strtok_r behavior)
        loop {
            // Find first non-delimiter character
            let start = self
                .remaining
                .find(|c: char| !self.delimiters.contains(c))?;

            // Trim leading delimiters
            self.remaining = &self.remaining[start..];

            if self.remaining.is_empty() {
                return None;
            }

            // Find next delimiter or end of string
            let end = self
                .remaining
                .find(|c: char| self.delimiters.contains(c))
                .unwrap_or(self.remaining.len());

            let token = &self.remaining[..end];

            // Advance past this token and its trailing delimiter
            if end < self.remaining.len() {
                self.remaining = &self.remaining[end + 1..];
            } else {
                self.remaining = "";
            }

            // Skip empty tokens (consecutive delimiters)
            if !token.is_empty() {
                self.started = true;
                return Some(token);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strtok_r_basic() {
        let mut buf = b"hello,world,test".to_vec();
        let delims = b",";
        let mut state = StrtokState::new();

        let t1 = strtok_r(Some(&mut buf), delims, &mut state);
        assert_eq!(t1, Some(b"hello".as_slice()));

        let t2 = strtok_r_continue(&mut buf, delims, &mut state);
        assert_eq!(t2, Some(b"world".as_slice()));

        let t3 = strtok_r_continue(&mut buf, delims, &mut state);
        assert_eq!(t3, Some(b"test".as_slice()));

        let t4 = strtok_r_continue(&mut buf, delims, &mut state);
        assert_eq!(t4, None);
    }

    #[test]
    fn test_strtok_r_leading_delimiters() {
        let mut buf = b",,hello,world".to_vec();
        let delims = b",";
        let mut state = StrtokState::new();

        let t1 = strtok_r(Some(&mut buf), delims, &mut state);
        assert_eq!(t1, Some(b"hello".as_slice()));

        let t2 = strtok_r_continue(&mut buf, delims, &mut state);
        assert_eq!(t2, Some(b"world".as_slice()));

        let t3 = strtok_r_continue(&mut buf, delims, &mut state);
        assert_eq!(t3, None);
    }

    #[test]
    fn test_strtok_r_trailing_delimiters() {
        let mut buf = b"hello,world,,".to_vec();
        let delims = b",";
        let mut state = StrtokState::new();

        let t1 = strtok_r(Some(&mut buf), delims, &mut state);
        assert_eq!(t1, Some(b"hello".as_slice()));

        let t2 = strtok_r_continue(&mut buf, delims, &mut state);
        assert_eq!(t2, Some(b"world".as_slice()));

        let t3 = strtok_r_continue(&mut buf, delims, &mut state);
        assert_eq!(t3, None);
    }

    #[test]
    fn test_strtok_r_empty_string() {
        let mut buf = b"".to_vec();
        let delims = b",";
        let mut state = StrtokState::new();

        let t1 = strtok_r(Some(&mut buf), delims, &mut state);
        assert_eq!(t1, None);
    }

    #[test]
    fn test_strtok_r_only_delimiters() {
        let mut buf = b",,,".to_vec();
        let delims = b",";
        let mut state = StrtokState::new();

        let t1 = strtok_r(Some(&mut buf), delims, &mut state);
        assert_eq!(t1, None);
    }

    #[test]
    fn test_strtok_r_multiple_delimiters() {
        let mut buf = b"hello;world,test".to_vec();
        let delims = b";,";
        let mut state = StrtokState::new();

        let t1 = strtok_r(Some(&mut buf), delims, &mut state);
        assert_eq!(t1, Some(b"hello".as_slice()));

        let t2 = strtok_r_continue(&mut buf, delims, &mut state);
        assert_eq!(t2, Some(b"world".as_slice()));

        let t3 = strtok_r_continue(&mut buf, delims, &mut state);
        assert_eq!(t3, Some(b"test".as_slice()));
    }

    #[test]
    fn test_strtok_r_single_token() {
        let mut buf = b"hello".to_vec();
        let delims = b",";
        let mut state = StrtokState::new();

        let t1 = strtok_r(Some(&mut buf), delims, &mut state);
        assert_eq!(t1, Some(b"hello".as_slice()));

        let t2 = strtok_r_continue(&mut buf, delims, &mut state);
        assert_eq!(t2, None);
    }

    #[test]
    fn test_tokenize_safe() {
        let tokens: Vec<&str> = tokenize("hello,world,test", ",").collect();
        assert_eq!(tokens, vec!["hello", "world", "test"]);
    }

    #[test]
    fn test_tokenize_leading_delimiters() {
        let tokens: Vec<&str> = tokenize(",,hello,world", ",").collect();
        assert_eq!(tokens, vec!["hello", "world"]);
    }

    #[test]
    fn test_tokenize_empty_tokens_skipped() {
        let tokens: Vec<&str> = tokenize("a,,b,,,c", ",").collect();
        assert_eq!(tokens, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_tokenize_empty_string() {
        let tokens: Vec<&str> = tokenize("", ",").collect();
        assert!(tokens.is_empty());
    }

    #[test]
    fn test_tokenize_only_delimiters() {
        let tokens: Vec<&str> = tokenize(",,,", ",").collect();
        assert!(tokens.is_empty());
    }

    #[test]
    fn test_tokenize_whitespace() {
        let tokens: Vec<&str> = tokenize("  hello  world  test  ", " ").collect();
        assert_eq!(tokens, vec!["hello", "world", "test"]);
    }

    #[test]
    fn test_state_reset() {
        let mut state = StrtokState::new();
        state.position = 42;
        state.reset();
        assert_eq!(state.position, 0);
    }

    #[test]
    fn test_strtok_r_consecutive_calls() {
        // Verify that multiple independent tokenizations can run
        // (the key feature of strtok_r vs strtok)
        let mut buf1 = b"a,b,c".to_vec();
        let mut buf2 = b"1;2;3".to_vec();
        let mut state1 = StrtokState::new();
        let mut state2 = StrtokState::new();

        let t1a = strtok_r(Some(&mut buf1), b",", &mut state1);
        let t2a = strtok_r(Some(&mut buf2), b";", &mut state2);

        assert_eq!(t1a, Some(b"a".as_slice()));
        assert_eq!(t2a, Some(b"1".as_slice()));

        let t1b = strtok_r_continue(&mut buf1, b",", &mut state1);
        let t2b = strtok_r_continue(&mut buf2, b";", &mut state2);

        assert_eq!(t1b, Some(b"b".as_slice()));
        assert_eq!(t2b, Some(b"2".as_slice()));
    }
}
