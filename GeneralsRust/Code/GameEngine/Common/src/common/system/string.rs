// FILE: string.rs /////////////////////////////////////////////////////////////
// String utilities and wrapper types
///////////////////////////////////////////////////////////////////////////////

use std::fmt;

/// WSYS String wrapper
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WSysString {
    data: String,
}

impl WSysString {
    pub fn new(s: &str) -> Self {
        Self {
            data: s.to_string(),
        }
    }

    pub fn empty() -> Self {
        Self {
            data: String::new(),
        }
    }

    pub fn make_upper_case(&mut self) {
        self.data = self.data.to_uppercase();
    }

    pub fn make_lower_case(&mut self) {
        self.data = self.data.to_lowercase();
    }

    pub fn length(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn format(&mut self, _fmt: &str, args: fmt::Arguments) -> Result<(), fmt::Error> {
        self.data = format!("{}", args);
        Ok(())
    }

    pub fn set(&mut self, s: &str) {
        self.data = s.to_string();
    }

    pub fn get(&self) -> &str {
        &self.data
    }

    pub fn as_str(&self) -> &str {
        &self.data
    }
}

impl Default for WSysString {
    fn default() -> Self {
        Self::empty()
    }
}

impl From<&str> for WSysString {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for WSysString {
    fn from(s: String) -> Self {
        Self { data: s }
    }
}

impl fmt::Display for WSysString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.data)
    }
}

impl PartialEq<str> for WSysString {
    fn eq(&self, other: &str) -> bool {
        self.data == other
    }
}

impl PartialEq<&str> for WSysString {
    fn eq(&self, other: &&str) -> bool {
        self.data == *other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wsys_string() {
        let mut s = WSysString::new("Hello World");
        assert_eq!(s.length(), 11);
        assert!(!s.is_empty());

        s.make_upper_case();
        assert_eq!(s.get(), "HELLO WORLD");

        s.make_lower_case();
        assert_eq!(s.get(), "hello world");

        s.set("Test");
        assert_eq!(s, "Test");

        let empty = WSysString::empty();
        assert!(empty.is_empty());
    }
}
