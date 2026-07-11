//! String class for WW3D
//!
//! Minimal implementation for compilation compatibility

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StringClass {
    data: String,
}

impl StringClass {
    pub fn new() -> Self {
        Self {
            data: String::new(),
        }
    }

    /// Construct from a string slice (C++ StringClass parity name; not `std::str::FromStr`).
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        Self {
            data: s.to_string(),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.data
    }
}

impl Default for StringClass {
    fn default() -> Self {
        Self::new()
    }
}

impl From<&str> for StringClass {
    fn from(s: &str) -> Self {
        Self::from_str(s)
    }
}

impl From<String> for StringClass {
    fn from(s: String) -> Self {
        Self { data: s }
    }
}

impl std::borrow::Borrow<str> for StringClass {
    fn borrow(&self) -> &str {
        &self.data
    }
}

impl std::fmt::Display for StringClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.data)
    }
}
