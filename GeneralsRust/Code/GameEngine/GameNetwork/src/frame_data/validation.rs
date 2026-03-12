//! Frame data validation

/// Frame validator
pub struct FrameValidator;

impl FrameValidator {
    /// Create new validator
    pub fn new() -> Self {
        Self
    }
}

impl Default for FrameValidator {
    fn default() -> Self {
        Self::new()
    }
}
