//! Anti-cheat system

/// Anti-cheat service
pub struct AntiCheatService;

impl AntiCheatService {
    /// Create new anti-cheat service
    pub fn new() -> Self {
        Self
    }
}

impl Default for AntiCheatService {
    fn default() -> Self {
        Self::new()
    }
}
