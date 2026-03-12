//! Authentication services

/// Authentication provider
pub struct AuthenticationProvider;

impl AuthenticationProvider {
    /// Create new provider
    pub fn new() -> Self {
        Self
    }
}

impl Default for AuthenticationProvider {
    fn default() -> Self {
        Self::new()
    }
}