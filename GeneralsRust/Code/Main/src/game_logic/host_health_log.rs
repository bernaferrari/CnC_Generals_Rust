//! Ordered, immutable receipts of completed per-GameLogic body operations.
//!
//! The driving GameLogic owns this value. Producers borrow it explicitly; this
//! module contains no thread-local or other ambient mutable state.

use super::ObjectId;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum HostHealthEvent {
    Damage(super::host_damage_log::HostDamageEvent),
    Heal(super::host_heal_log::HostHealEvent),
}

/// Host-to-shadow event state for one GameLogic instance.
///
/// Completed presentation receipts are transient frame state and are
/// intentionally not part of GameLogic's Xfer representation.
#[derive(Debug, Default)]
pub struct HostHealthEvents {
    events: Vec<HostHealthEvent>,
    last_damage: Vec<super::host_damage_log::HostDamageEvent>,
    last_heal: Vec<super::host_heal_log::HostHealEvent>,
    cumulative_damage: f32,
    cumulative_kills: u32,
}

impl HostHealthEvents {
    /// Observe damage already committed by Object's C++-ordered body operation.
    pub(crate) fn record_applied_damage(
        &mut self,
        target: ObjectId,
        amount: f32,
        source: Option<ObjectId>,
        destroyed: bool,
        damage_type_ordinal: u32,
        body: super::host_damage_log::BodyHealthObservation,
    ) {
        self.push_damage(super::host_damage_log::HostDamageEvent::applied(
            target,
            amount,
            source,
            destroyed,
            damage_type_ordinal,
            body,
        ));
    }

    fn push_damage(&mut self, event: super::host_damage_log::HostDamageEvent) {
        if event.amount <= 0.0 && !event.destroyed {
            return;
        }
        self.cumulative_damage += event.amount.max(0.0);
        if event.destroyed {
            self.cumulative_kills = self.cumulative_kills.saturating_add(1);
        }
        self.events.push(HostHealthEvent::Damage(event));
    }

    /// Queue an absolute health effect not yet applied to this owner.

    /// Observe a completed owner absolute-health write.
    pub(crate) fn record_applied_heal(&mut self, target: ObjectId, health: f32) {
        self.push_heal(super::host_heal_log::HostHealEvent::applied(target, health));
    }

    fn push_heal(&mut self, event: super::host_heal_log::HostHealEvent) {
        if !event.health.is_finite() || event.health < 0.0 {
            return;
        }
        self.events.push(HostHealthEvent::Heal(event));
    }

    pub(crate) fn snapshot_ordered(&self) -> Vec<HostHealthEvent> {
        self.events.clone()
    }

    /// Drain all records in the exact order their owner operations emitted them.
    pub(crate) fn drain_ordered(&mut self) -> Vec<HostHealthEvent> {
        let drained = std::mem::take(&mut self.events);
        self.cache_completed(&drained);
        drained
    }

    /// Freeze completed owner observations at the actual frame boundary.
    pub(crate) fn drain_for_host_boundary(&mut self) -> Vec<HostHealthEvent> {
        let drained = std::mem::take(&mut self.events);
        self.cache_completed(&drained);
        drained
    }

    /// Preserve the eager receipt and append later events for this same
    /// boundary. The prefix records shadow admission, not owner HP writes.

    fn cache_completed(&mut self, drained: &[HostHealthEvent]) {
        let mut damage = Vec::new();
        let mut heal = Vec::new();
        for event in drained {
            match event {
                HostHealthEvent::Damage(event) => damage.push(*event),
                HostHealthEvent::Heal(event) => heal.push(*event),
            }
        }
        if !damage.is_empty() {
            self.last_damage = damage;
        }
        if !heal.is_empty() {
            self.last_heal = heal;
        }
    }

    pub(crate) fn snapshot_damage(&self) -> Vec<super::host_damage_log::HostDamageEvent> {
        self.events
            .iter()
            .filter_map(|event| match event {
                HostHealthEvent::Damage(event) => Some(*event),
                HostHealthEvent::Heal(_) => None,
            })
            .collect()
    }

    pub(crate) fn snapshot_heal(&self) -> Vec<super::host_heal_log::HostHealEvent> {
        self.events
            .iter()
            .filter_map(|event| match event {
                HostHealthEvent::Damage(_) => None,
                HostHealthEvent::Heal(event) => Some(*event),
            })
            .collect()
    }

    pub(crate) fn has_damage(&self, object: ObjectId) -> bool {
        self.events
            .iter()
            .any(|event| matches!(event, HostHealthEvent::Damage(event) if event.target == object))
    }

    pub(crate) fn has_heal(&self, object: ObjectId) -> bool {
        self.events
            .iter()
            .any(|event| matches!(event, HostHealthEvent::Heal(event) if event.target == object))
    }

    pub(crate) fn len_damage(&self) -> usize {
        self.events
            .iter()
            .filter(|event| matches!(event, HostHealthEvent::Damage(_)))
            .count()
    }

    pub(crate) fn len_heal(&self) -> usize {
        self.events
            .iter()
            .filter(|event| matches!(event, HostHealthEvent::Heal(_)))
            .count()
    }

    /// Drain only damage, leaving heals in their original relative order.
    pub(crate) fn drain_damage(&mut self) -> Vec<super::host_damage_log::HostDamageEvent> {
        let mut selected = Vec::new();
        let mut keep = Vec::with_capacity(self.events.len());
        for event in self.events.drain(..) {
            match event {
                HostHealthEvent::Damage(event) => selected.push(event),
                other => keep.push(other),
            }
        }
        self.events = keep;
        if !selected.is_empty() {
            self.last_damage = selected.clone();
        }
        selected
    }

    /// Drain only heals, leaving damage in its original relative order.
    pub(crate) fn drain_heal(&mut self) -> Vec<super::host_heal_log::HostHealEvent> {
        let mut selected = Vec::new();
        let mut keep = Vec::with_capacity(self.events.len());
        for event in self.events.drain(..) {
            match event {
                HostHealthEvent::Heal(event) => selected.push(event),
                other => keep.push(other),
            }
        }
        self.events = keep;
        if !selected.is_empty() {
            self.last_heal = selected.clone();
        }
        selected
    }

    pub(crate) fn clear_damage(&mut self) {
        self.events
            .retain(|event| !matches!(event, HostHealthEvent::Damage(_)));
        self.last_damage.clear();
        self.reset_cumulative();
    }

    pub(crate) fn clear_heal(&mut self) {
        self.events
            .retain(|event| !matches!(event, HostHealthEvent::Heal(_)));
        self.last_heal.clear();
    }

    /// Clear one owner's event and receipt state; used by that owner's reset.
    pub(crate) fn clear(&mut self) {
        self.events.clear();
        self.last_damage.clear();
        self.last_heal.clear();
        self.reset_cumulative();
    }

    pub(crate) fn take_last_damage(&mut self) -> Vec<super::host_damage_log::HostDamageEvent> {
        std::mem::take(&mut self.last_damage)
    }

    pub(crate) fn snapshot_last_damage(&self) -> Vec<super::host_damage_log::HostDamageEvent> {
        self.last_damage.clone()
    }

    pub(crate) fn take_last_heal(&mut self) -> Vec<super::host_heal_log::HostHealEvent> {
        std::mem::take(&mut self.last_heal)
    }

    pub(crate) fn snapshot_last_heal(&self) -> Vec<super::host_heal_log::HostHealEvent> {
        self.last_heal.clone()
    }

    pub(crate) fn cumulative_totals(&self) -> (f32, u32) {
        (self.cumulative_damage, self.cumulative_kills)
    }

    pub(crate) fn reset_cumulative(&mut self) {
        self.cumulative_damage = 0.0;
        self.cumulative_kills = 0;
    }
}

#[cfg(test)]
mod completed_receipt_tests {
    use super::*;
    use crate::game_logic::{Object, Team, ThingTemplate};

    fn body() -> Object {
        let mut template = ThingTemplate::new("CompletedReceiptBody");
        template.set_health(100.0);
        Object::new(template, ObjectId(7), Team::USA)
    }

    #[test]
    fn real_body_receipts_preserve_operation_order_and_frozen_result() {
        let mut object = body();
        let mut events = HostHealthEvents::default();
        assert!(!object.take_damage(20.0, &mut events));
        object.heal(5.0, &mut events);
        assert_eq!(object.health.current, 85.0);
        let ordered = events.drain_for_host_boundary();
        assert_eq!(ordered.len(), 2);
        let HostHealthEvent::Damage(damage) = ordered[0] else {
            panic!("damage first");
        };
        let HostHealthEvent::Heal(heal) = ordered[1] else {
            panic!("heal second");
        };
        assert_eq!(damage.amount, 20.0);
        assert_eq!(
            damage.body_observation(),
            crate::game_logic::host_damage_log::BodyHealthObservation {
                current: 80.0,
                maximum: 100.0
            }
        );
        assert_eq!(heal.health, 85.0);
        assert_eq!(events.cumulative_totals(), (20.0, 0));
        assert!(events.drain_for_host_boundary().is_empty());
        assert_eq!(object.health.current, 85.0);
        assert_eq!(events.snapshot_last_damage(), vec![damage]);
        assert_eq!(events.snapshot_last_heal(), vec![heal]);
    }

    #[test]
    fn same_id_receivers_and_invalid_observations_are_isolated() {
        let mut first = body();
        let mut second = body();
        let mut a = HostHealthEvents::default();
        let mut b = HostHealthEvents::default();
        first.take_damage(20.0, &mut a);
        second.take_damage(5.0, &mut b);
        let frozen = a.snapshot_ordered();
        b.drain_for_host_boundary();
        b.clear();
        assert_eq!(a.snapshot_ordered(), frozen);
        a.record_applied_damage(
            ObjectId(7),
            f32::NAN,
            None,
            false,
            0,
            crate::game_logic::host_damage_log::BodyHealthObservation {
                current: first.health.current,
                maximum: first.health.maximum,
            },
        );
        a.record_applied_heal(ObjectId(7), -1.0);
        assert_eq!(a.snapshot_ordered(), frozen);
        assert_eq!(first.health.current, 80.0);
        assert_eq!(second.health.current, 95.0);
        assert_eq!(a.cumulative_totals(), (20.0, 0));
    }
}
