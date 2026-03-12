//! Game filtering for matchmaking

/// Game filter
pub struct GameFilter;

impl GameFilter {
    /// Create new filter
    pub fn new() -> Self {
        Self
    }
}

impl Default for GameFilter {
    fn default() -> Self {
        Self::new()
    }
}