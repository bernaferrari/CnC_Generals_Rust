//! WWString - String wrapper class for WW3D compatibility

use std::fmt;

/// String class wrapper for WW3D compatibility
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StringClass {
    inner: String,
}

impl StringClass {
    /// Create a new StringClass from a string slice
    pub fn new(s: &str) -> Self {
        Self {
            inner: s.to_string(),
        }
    }

    /// Create an empty StringClass
    pub fn empty() -> Self {
        Self {
            inner: String::new(),
        }
    }

    /// Get the string as a slice
    pub fn as_str(&self) -> &str {
        &self.inner
    }

    /// Get the length of the string
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Check if the string is empty
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Convert to String
    pub fn to_string(&self) -> String {
        self.inner.clone()
    }
}

impl From<&str> for StringClass {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for StringClass {
    fn from(s: String) -> Self {
        Self { inner: s }
    }
}

impl fmt::Display for StringClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.inner)
    }
}

// Implement Deref for easier use
impl std::ops::Deref for StringClass {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
