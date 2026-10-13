//! Absolute-health event payload for the Main host/shadow bridge.
//!
//! Completed receipts live on `HostHealthEvents` in `host_health_log`; this
//! module intentionally contains no ambient state.

use super::ObjectId;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostHealEvent {
    pub target: ObjectId,
    /// Absolute health after the host write.
    pub health: f32,
}

impl HostHealEvent {
    pub(crate) fn applied(target: ObjectId, health: f32) -> Self {
        Self { target, health }
    }
}
