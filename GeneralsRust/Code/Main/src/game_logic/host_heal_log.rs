//! Frame-local host heal / absolute-HP log for GameWorld shadow parity.
//!
//! Complements `host_damage_log` for HP increases and absolute health writes
//! (battle-drone repair, construction finish, composite armor, etc.).

use super::ObjectId;
use crate::game_logic::host_damage_log::OwnerHealthChange;
use std::cell::RefCell;

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

thread_local! {
    static LOG: RefCell<Vec<HostHealEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostHealEvent>> = RefCell::new(Vec::new());
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
    LOG.with(|log| {
        log.borrow_mut().push(HostHealEvent {
            target,
            health,
            owner_health_change,
        });
    });
}

pub fn snapshot() -> Vec<HostHealEvent> {
    LOG.with(|log| log.borrow().clone())
}

pub fn has_pending(object: ObjectId) -> bool {
    LOG.with(|log| log.borrow().iter().any(|e| e.target == object))
}

pub fn drain() -> Vec<HostHealEvent> {
    let v = LOG.with(|log| std::mem::take(&mut *log.borrow_mut()));
    // Keep last non-empty batch for PresentationFrame after shadow session.
    if !v.is_empty() {
        LAST_DRAIN.with(|last| *last.borrow_mut() = v.clone());
    }
    v
}

pub fn len() -> usize {
    LOG.with(|log| log.borrow().len())
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
    LAST_DRAIN.with(|last| last.borrow_mut().clear());
}

/// Take events from the most recent non-empty `drain()` (PresentationFrame sole consumer).
pub fn take_last_drain() -> Vec<HostHealEvent> {
    LAST_DRAIN.with(|last| std::mem::take(&mut *last.borrow_mut()))
}

/// Non-destructive peek (tests).
pub fn last_drain_snapshot() -> Vec<HostHealEvent> {
    LAST_DRAIN.with(|last| last.borrow().clone())
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
