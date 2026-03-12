//! Game discovery service

/// Game discovery
pub struct GameDiscovery;

impl GameDiscovery {
    /// Create new discovery service
    pub fn new() -> Self {
        Self
    }
}

impl Default for GameDiscovery {
    fn default() -> Self {
        Self::new()
    }
}