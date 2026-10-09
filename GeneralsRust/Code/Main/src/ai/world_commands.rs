//! Synchronous host execution adapters for strategic AI.
//!
//! Keep command side effects on the driving GameLogic and apply them immediately, as
//! C++ AIPlayer does. These adapters do not buffer decisions or select an active world.
use super::*;

impl GameLogic {
    /// Drain the world-owned TeamFactory's pre-destruction notifications before AI phases.
    pub(super) fn take_ai_team_destroy_notifications(&mut self) -> Vec<(u32, String)> {
        match self.team_factory.lock() {
            Ok(mut factory) => factory.take_host_pre_team_destroy_requests(),
            Err(poisoned) => poisoned.into_inner().take_host_pre_team_destroy_requests(),
        }
    }
}
