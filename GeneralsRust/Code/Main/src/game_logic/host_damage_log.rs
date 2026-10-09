//! Frame-local host damage log for GameWorld shadow parity.
//!
//! `Object::take_damage_from` records actual HP damage applied (post-armor).
//! GameLogic/engine drains the log after a host tick and feeds `GameWorldShadow`.
//!
//! Completed observations copy the exact owner's body at the operation. The
//! frame-local handoff remains thread-local until world event ownership is migrated.

use super::ObjectId;
use std::cell::Cell;

/// Whether the producer has already changed its object's health.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::game_logic) enum OwnerHealthChange {
    Pending,
    Applied,
}

/// Immutable result of a completed body operation, never another mutable body.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BodyHealthObservation {
    pub current: f32,
    pub maximum: f32,
}

/// One damage application observed on the host authority.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostDamageEvent {
    pub target: ObjectId,
    /// Actual HP removed after armor/battle-plan scalars.
    pub amount: f32,
    pub source: Option<ObjectId>,
    pub destroyed: bool,
    /// C++ `DamageType` ordinal (`DAMAGE_EXPLOSION` = 0).
    pub damage_type_ordinal: u32,
    owner_body: Option<BodyHealthObservation>,
}

impl HostDamageEvent {
    pub(crate) fn owner_health_already_applied(&self) -> bool {
        self.owner_body.is_some()
    }

    pub(crate) fn body_observation(&self) -> Option<BodyHealthObservation> {
        self.owner_body
    }
}

thread_local! {
    static CUM_DAMAGE: Cell<f32> = const { Cell::new(0.0) };
    static CUM_KILLS: Cell<u32> = const { Cell::new(0) };
}

/// Queue damage whose health effect still needs admission by the authority.
/// Untyped callers default to C++ `DAMAGE_EXPLOSION` (ordinal 0).
pub fn record(target: ObjectId, amount: f32, source: Option<ObjectId>, destroyed: bool) {
    record_typed(target, amount, source, destroyed, 0);
}

/// Record a damage event with the C++ `DamageType` ordinal.
pub fn record_typed(
    target: ObjectId,
    amount: f32,
    source: Option<ObjectId>,
    destroyed: bool,
    damage_type_ordinal: u32,
) {
    record_damage_event(target, amount, source, destroyed, damage_type_ordinal, None);
}

/// Observe damage already committed by Object's C++-ordered body operation.
/// Shadow consumers still receive the damage, but host fallback admission must
/// not subtract it from that same object again.
pub(crate) fn record_applied(
    target: ObjectId,
    amount: f32,
    source: Option<ObjectId>,
    destroyed: bool,
    damage_type_ordinal: u32,
    body: BodyHealthObservation,
) {
    record_damage_event(
        target,
        amount,
        source,
        destroyed,
        damage_type_ordinal,
        Some(body),
    );
}

fn record_damage_event(
    target: ObjectId,
    amount: f32,
    source: Option<ObjectId>,
    destroyed: bool,
    damage_type_ordinal: u32,
    owner_body: Option<BodyHealthObservation>,
) {
    if amount <= 0.0 && !destroyed {
        return;
    }
    CUM_DAMAGE.set(CUM_DAMAGE.get() + amount.max(0.0));
    if destroyed {
        CUM_KILLS.set(CUM_KILLS.get().saturating_add(1));
    }
    crate::game_logic::host_health_log::record_damage(HostDamageEvent {
        target,
        amount,
        source,
        destroyed,
        damage_type_ordinal,
        owner_body,
    });
}

/// Snapshot pending damage events in the order damage producers recorded them.
/// Heal events remain available through `host_heal_log::snapshot`.
pub fn snapshot() -> Vec<HostDamageEvent> {
    crate::game_logic::host_health_log::snapshot_damage()
}

pub fn has_pending(object: ObjectId) -> bool {
    crate::game_logic::host_health_log::has_damage(object)
}

/// Drain only damage records, leaving pending heal records untouched.
pub fn drain() -> Vec<HostDamageEvent> {
    crate::game_logic::host_health_log::drain_damage()
}

/// Peek count without draining (tests).
pub fn len() -> usize {
    crate::game_logic::host_health_log::len_damage()
}

/// Match-scoped cumulative totals (survives drain; reset via `clear` / `reset_cumulative`).
pub fn cumulative_totals() -> (f32, u32) {
    (CUM_DAMAGE.get(), CUM_KILLS.get())
}

/// Reset cumulative match counters (new skirmish residual).
pub fn reset_cumulative() {
    CUM_DAMAGE.set(0.0);
    CUM_KILLS.set(0);
}

/// Clear without returning (test isolation).
pub fn clear() {
    crate::game_logic::host_health_log::clear_damage();
    reset_cumulative();
}

/// Take events from the most recent non-empty `drain()` (PresentationFrame sole consumer).
pub fn take_last_drain() -> Vec<HostDamageEvent> {
    crate::game_logic::host_health_log::take_last_damage()
}

/// Non-destructive peek (tests).
pub fn last_drain_snapshot() -> Vec<HostDamageEvent> {
    crate::game_logic::host_health_log::snapshot_last_damage()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applied_observations_and_pending_damage_keep_their_admission_kind() {
        clear();
        let body = BodyHealthObservation {
            current: 80.0,
            maximum: 100.0,
        };
        record_applied(ObjectId(1), 20.0, None, false, 2, body);
        record_typed(ObjectId(2), 30.0, Some(ObjectId(1)), true, 5);
        let events = drain();
        assert_eq!(events.len(), 2);
        assert!(events[0].owner_health_already_applied());
        assert_eq!(events[0].body_observation(), Some(body));
        assert!(!events[1].owner_health_already_applied());
        assert_eq!(events[1].body_observation(), None);
        assert_eq!(events[0].damage_type_ordinal, 2);
        assert_eq!(events[1].damage_type_ordinal, 5);
        assert_eq!(last_drain_snapshot(), events);
        assert_eq!(cumulative_totals(), (50.0, 1));
        clear();
    }

    #[test]
    fn record_and_drain_preserves_order() {
        clear();
        record(ObjectId(1), 10.0, Some(ObjectId(2)), false);
        record(ObjectId(3), 5.0, None, true);
        assert_eq!(len(), 2);
        let v = drain();
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].target, ObjectId(1));
        assert_eq!(v[1].destroyed, true);
        assert!(drain().is_empty());
        assert_eq!(last_drain_snapshot().len(), 2);
        let (d, k) = cumulative_totals();
        assert!((d - 15.0).abs() < f32::EPSILON);
        assert_eq!(k, 1);
    }
}
