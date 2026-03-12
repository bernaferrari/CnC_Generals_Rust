//! Security validation

/// Security validator
pub struct SecurityValidator;

impl SecurityValidator {
    /// Create new validator
    pub fn new() -> Self {
        Self
    }
}

impl Default for SecurityValidator {
    fn default() -> Self {
        Self::new()
    }
}
