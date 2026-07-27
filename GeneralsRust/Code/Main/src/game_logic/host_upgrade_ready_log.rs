//! Wave 624: GameWorld upgrade-complete ready residual log.
//!
//! Under DAMAGE_AUTHORITY / economy-adjacent upgrade channel, when
//! `writeback_completed_upgrades_to_host` first observes a completed upgrade
//! on a shadow player, it records here. Host drains and runs
//! `apply_host_upgrade_complete` so GameWorld decides completion while host
//! owns unit unlocks, EVA, radar, and status-bit residuals.
//!
//! Fail-closed: empty drain is valid (no new completions this frame).

use crate::game_logic::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct HostUpgradeReadyEvent {
    pub player_id: u32,
    pub upgrade_name: String,
    /// Optional producer/source host object when known (0 = unknown).
    pub source_object: ObjectId,
}

thread_local! {
    static LOG: RefCell<Vec<HostUpgradeReadyEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostUpgradeReadyEvent>> = RefCell::new(Vec::new());
}

pub fn record(player_id: u32, upgrade_name: impl Into<String>, source_object: ObjectId) {
    LOG.with(|log| {
        log.borrow_mut().push(HostUpgradeReadyEvent {
            player_id,
            upgrade_name: upgrade_name.into(),
            source_object,
        });
    });
}

pub fn drain() -> Vec<HostUpgradeReadyEvent> {
    LOG.with(|log| {
        let events = std::mem::take(&mut *log.borrow_mut());
        LAST_DRAIN.with(|last| *last.borrow_mut() = events.clone());
        events
    })
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
    LAST_DRAIN.with(|last| last.borrow_mut().clear());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_drain_roundtrip() {
        clear();
        record(1, "Upgrade_AmericaAdvancedTraining", ObjectId(0));
        let d = drain();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].player_id, 1);
        assert!(d[0].upgrade_name.contains("AdvancedTraining"));
        assert!(drain().is_empty());
        clear();
    }
}
