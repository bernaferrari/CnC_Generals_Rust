//! Network Command Bridge (stubbed when network feature is disabled).

use crate::commands::Command;

/// Statistics for network command translation
#[derive(Debug, Clone, Default)]
pub struct BridgeStatistics {
    pub total_translated: u64,
    pub total_rejected: u64,
    pub frame_sync_errors: u64,
    pub invalid_payloads: u64,
}

/// Stubbed network command translation bridge
pub struct NetworkCommandBridge {
    current_frame: u32,
    stats: BridgeStatistics,
}

impl NetworkCommandBridge {
    pub fn new() -> Self {
        Self {
            current_frame: 0,
            stats: BridgeStatistics::default(),
        }
    }

    pub fn set_current_frame(&mut self, frame: u32) {
        self.current_frame = frame;
    }

    pub fn translate<T>(_net_cmd: &T) -> Result<Command, String> {
        Err("network feature disabled".to_string())
    }

    pub fn validate_frame_sync<T>(&self, _net_cmd: &T) -> Result<(), String> {
        Ok(())
    }

    pub fn queue_network_command<T>(&mut self, _net_cmd: T) -> Result<(), String> {
        self.stats.total_rejected += 1;
        self.stats.invalid_payloads += 1;
        Err("network feature disabled".to_string())
    }

    pub fn get_statistics(&self) -> &BridgeStatistics {
        &self.stats
    }

    pub fn reset_statistics(&mut self) {
        self.stats = BridgeStatistics::default();
    }
}

impl Default for NetworkCommandBridge {
    fn default() -> Self {
        Self::new()
    }
}
