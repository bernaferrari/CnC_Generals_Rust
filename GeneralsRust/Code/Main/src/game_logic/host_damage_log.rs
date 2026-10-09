//! Damage-event payloads and owner health observations for the Main host/shadow bridge.
//!
//! Pending storage and per-match counters live on `HostHealthEvents` in
//! `host_health_log`; this module intentionally contains no ambient state.

use super::ObjectId;

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
    pub(crate) fn pending(
        target: ObjectId,
        amount: f32,
        source: Option<ObjectId>,
        destroyed: bool,
        damage_type_ordinal: u32,
    ) -> Self {
        Self {
            target,
            amount,
            source,
            destroyed,
            damage_type_ordinal,
            owner_body: None,
        }
    }

    pub(crate) fn applied(
        target: ObjectId,
        amount: f32,
        source: Option<ObjectId>,
        destroyed: bool,
        damage_type_ordinal: u32,
        body: BodyHealthObservation,
    ) -> Self {
        Self {
            target,
            amount,
            source,
            destroyed,
            damage_type_ordinal,
            owner_body: Some(body),
        }
    }

    pub(crate) fn owner_health_already_applied(&self) -> bool {
        self.owner_body.is_some()
    }

    pub(crate) fn body_observation(&self) -> Option<BodyHealthObservation> {
        self.owner_body
    }
}

#[cfg(test)]
mod owned_queue_tests {
    use super::*;
    use crate::game_logic::host_health_log::HostHealthEvents;

    #[test]
    fn applied_observations_and_pending_damage_keep_their_admission_kind() {
        let mut events = HostHealthEvents::default();
        let body = BodyHealthObservation {
            current: 80.0,
            maximum: 100.0,
        };
        events.record_applied_damage(ObjectId(1), 20.0, None, false, 2, body);
        events.record_damage_typed(ObjectId(2), 30.0, Some(ObjectId(1)), true, 5);
        let drained = events.drain_damage();
        assert_eq!(drained.len(), 2);
        assert!(drained[0].owner_health_already_applied());
        assert_eq!(drained[0].body_observation(), Some(body));
        assert!(!drained[1].owner_health_already_applied());
        assert_eq!(drained[1].body_observation(), None);
        assert_eq!(drained[0].damage_type_ordinal, 2);
        assert_eq!(drained[1].damage_type_ordinal, 5);
        assert_eq!(events.snapshot_last_damage(), drained);
        assert_eq!(events.cumulative_totals(), (50.0, 1));
    }

    #[test]
    fn damage_record_filters_and_cumulative_totals_are_instance_owned() {
        let mut a = HostHealthEvents::default();
        let mut b = HostHealthEvents::default();
        a.record_damage(ObjectId(1), 10.0, Some(ObjectId(2)), false);
        a.record_damage(ObjectId(3), 5.0, None, true);
        a.record_damage(ObjectId(4), 0.0, None, false);
        b.record_damage(ObjectId(1), 7.0, None, false);
        assert_eq!(a.len_damage(), 2);
        assert_eq!(b.len_damage(), 1);
        assert_eq!(a.drain_damage()[0].target, ObjectId(1));
        assert_eq!(a.snapshot_last_damage().len(), 2);
        assert!(a.drain_damage().is_empty());
        assert_eq!(a.cumulative_totals(), (15.0, 1));
        assert_eq!(b.cumulative_totals(), (7.0, 0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::HostHealthEvents;

    #[test]
    fn applied_observations_and_pending_damage_keep_their_admission_kind() {
        let mut events = HostHealthEvents::default();
        events.clear_damage();
        let body = BodyHealthObservation {
            current: 80.0,
            maximum: 100.0,
        };
        events.record_applied_damage(ObjectId(1), 20.0, None, false, 2, body);
        events.record_damage_typed(ObjectId(2), 30.0, Some(ObjectId(1)), true, 5);
        let drained = events.drain_damage();
        assert_eq!(drained.len(), 2);
        assert!(drained[0].owner_health_already_applied());
        assert_eq!(drained[0].body_observation(), Some(body));
        assert!(!drained[1].owner_health_already_applied());
        assert_eq!(drained[1].body_observation(), None);
        assert_eq!(drained[0].damage_type_ordinal, 2);
        assert_eq!(drained[1].damage_type_ordinal, 5);
        assert_eq!(events.snapshot_last_damage(), drained);
        assert_eq!(events.cumulative_totals(), (50.0, 1));
        events.clear_damage();
    }

    #[test]
    fn record_and_drain_preserves_order() {
        let mut events = HostHealthEvents::default();
        events.clear_damage();
        events.record_damage(ObjectId(1), 10.0, Some(ObjectId(2)), false);
        events.record_damage(ObjectId(3), 5.0, None, true);
        assert_eq!(events.len_damage(), 2);
        let v = events.drain_damage();
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].target, ObjectId(1));
        assert_eq!(v[1].destroyed, true);
        assert!(events.drain_damage().is_empty());
        assert_eq!(events.snapshot_last_damage().len(), 2);
        let (d, k) = events.cumulative_totals();
        assert!((d - 15.0).abs() < f32::EPSILON);
        assert_eq!(k, 1);
    }
}
