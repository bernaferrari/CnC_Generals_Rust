//! Absolute-health event payload for the Main host/shadow bridge.
//!
//! Pending storage lives on `HostHealthEvents` in `host_health_log`; this
//! module intentionally contains no ambient state.

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
    pub(crate) fn pending(target: ObjectId, health: f32) -> Self {
        Self {
            target,
            health,
            owner_health_change: OwnerHealthChange::Pending,
        }
    }

    pub(crate) fn applied(target: ObjectId, health: f32) -> Self {
        Self {
            target,
            health,
            owner_health_change: OwnerHealthChange::Applied,
        }
    }

    pub(crate) fn owner_health_already_applied(&self) -> bool {
        self.owner_health_change == OwnerHealthChange::Applied
    }
}

#[cfg(test)]
mod owned_queue_tests {
    use super::*;
    use crate::game_logic::host_health_log::HostHealthEvents;

    #[test]
    fn completed_and_pending_heals_preserve_validation_and_admission() {
        let mut events = HostHealthEvents::default();
        events.record_applied_heal(ObjectId(1), 90.0);
        events.record_heal(ObjectId(2), 70.0);
        events.record_applied_heal(ObjectId(3), f32::NAN);
        events.record_heal(ObjectId(4), -1.0);
        let drained = events.drain_heal();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].target, ObjectId(1));
        assert_eq!(drained[0].health, 90.0);
        assert!(drained[0].owner_health_already_applied());
        assert_eq!(drained[1].target, ObjectId(2));
        assert_eq!(drained[1].health, 70.0);
        assert!(!drained[1].owner_health_already_applied());
        assert_eq!(events.snapshot_last_heal(), drained);
    }

    #[test]
    fn empty_drain_retains_previous_nonempty_presentation_batch() {
        let mut events = HostHealthEvents::default();
        events.record_heal(ObjectId(1), 50.0);
        let first = events.drain_heal();
        assert_eq!(first.len(), 1);
        assert!(events.drain_heal().is_empty());
        assert_eq!(events.snapshot_last_heal(), first);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::HostHealthEvents;

    #[test]
    fn completed_observations_and_pending_heals_preserve_admission_and_order() {
        let mut events = HostHealthEvents::default();
        events.clear_heal();
        events.record_applied_heal(ObjectId(1), 90.0);
        events.record_heal(ObjectId(2), 70.0);
        events.record_applied_heal(ObjectId(3), f32::NAN);
        events.record_heal(ObjectId(4), -1.0);
        let drained = events.drain_heal();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].target, ObjectId(1));
        assert_eq!(drained[0].health, 90.0);
        assert!(drained[0].owner_health_already_applied());
        assert_eq!(drained[1].target, ObjectId(2));
        assert_eq!(drained[1].health, 70.0);
        assert!(!drained[1].owner_health_already_applied());
        assert_eq!(events.snapshot_last_heal(), drained);
        assert!(events.drain_heal().is_empty());
        events.clear_heal();
    }

    #[test]
    fn record_and_drain() {
        let mut events = HostHealthEvents::default();
        events.clear_heal();
        events.record_heal(ObjectId(1), 50.0);
        assert_eq!(events.drain_heal().len(), 1);
        assert!(events.drain_heal().is_empty());
        assert_eq!(events.snapshot_last_heal().len(), 1);
    }
}
