//! Frame-local host heal / absolute-HP log for GameWorld shadow parity.
//!
//! Complements `host_damage_log` for HP increases and absolute health writes
//! (battle-drone repair, construction finish, composite armor, etc.).

use super::ObjectId;
use crate::game_logic::host_damage_log::OwnerHealthChange;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostHealEvent {
    pub target: ObjectId,
    /// Absolute health after the host write.
    pub health: f32,
    owner_health_change: OwnerHealthChange,
}

impl HostHealEvent {
    pub(crate) fn owner_health_already_applied(&self) -> bool {
        self.owner_health_change == OwnerHealthChange::Applied
    }
}

/// Queue an absolute health effect not yet applied to the owner.
pub fn record(target: ObjectId, health: f32) {
    record_health_event(target, health, OwnerHealthChange::Pending);
}

/// Observe a completed owner write without replaying it at host admission.
pub(crate) fn record_applied(target: ObjectId, health: f32) {
    record_health_event(target, health, OwnerHealthChange::Applied);
}

fn record_health_event(target: ObjectId, health: f32, owner_health_change: OwnerHealthChange) {
    if !health.is_finite() || health < 0.0 {
        return;
    }
    crate::game_logic::host_health_log::record_heal(HostHealEvent {
        target,
        health,
        owner_health_change,
    });
}

/// Snapshot pending absolute-health events in their host-recorded order.
pub fn snapshot() -> Vec<HostHealEvent> {
    crate::game_logic::host_health_log::snapshot_heal()
}

pub fn has_pending(object: ObjectId) -> bool {
    crate::game_logic::host_health_log::has_heal(object)
}

/// Drain only absolute-health records, leaving pending damage records untouched.
pub fn drain() -> Vec<HostHealEvent> {
    crate::game_logic::host_health_log::drain_heal()
}

pub fn len() -> usize {
    crate::game_logic::host_health_log::len_heal()
}

pub fn clear() {
    crate::game_logic::host_health_log::clear_heal();
}

/// Take events from the most recent non-empty `drain()` (PresentationFrame sole consumer).
pub fn take_last_drain() -> Vec<HostHealEvent> {
    crate::game_logic::host_health_log::take_last_heal()
}

/// Non-destructive peek (tests).
pub fn last_drain_snapshot() -> Vec<HostHealEvent> {
    crate::game_logic::host_health_log::snapshot_last_heal()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completed_observations_and_pending_heals_preserve_admission_and_order() {
        clear();
        record_applied(ObjectId(1), 90.0);
        record(ObjectId(2), 70.0);
        record_applied(ObjectId(3), f32::NAN);
        record(ObjectId(4), -1.0);
        let events = drain();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].target, ObjectId(1));
        assert_eq!(events[0].health, 90.0);
        assert!(events[0].owner_health_already_applied());
        assert_eq!(events[1].target, ObjectId(2));
        assert_eq!(events[1].health, 70.0);
        assert!(!events[1].owner_health_already_applied());
        assert_eq!(last_drain_snapshot(), events);
        assert!(drain().is_empty());
        clear();
    }

    #[test]
    fn record_and_drain() {
        clear();
        record(ObjectId(1), 50.0);
        assert_eq!(drain().len(), 1);
        assert!(drain().is_empty());
        assert_eq!(last_drain_snapshot().len(), 1);
    }
}
