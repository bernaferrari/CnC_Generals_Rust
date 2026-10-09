//! Ordered, per-GameLogic transport for host HP mutations and observations.
//!
//! The driving GameLogic owns this value. Producers borrow it explicitly; this
//! module contains no thread-local or other ambient mutable state.

use super::ObjectId;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum HostHealthEvent {
    Damage(super::host_damage_log::HostDamageEvent),
    Heal(super::host_heal_log::HostHealEvent),
}

/// Ordered receipt for one owner's health handoff. New events extend the
/// unapplied suffix; admitting them never replays the completed prefix.
#[derive(Debug, Default)]
pub(crate) struct HostHealthBatch {
    events: Vec<HostHealthEvent>,
    shadow_applied_prefix: usize,
}

impl HostHealthBatch {
    pub(crate) fn events(&self) -> &[HostHealthEvent] {
        &self.events
    }

    pub(crate) fn shadow_applied_events(&self) -> &[HostHealthEvent] {
        &self.events[..self.shadow_applied_prefix]
    }

    pub(crate) fn pending_shadow_events(&self) -> &[HostHealthEvent] {
        &self.events[self.shadow_applied_prefix..]
    }

    pub(crate) fn mark_shadow_applied(&mut self) {
        self.shadow_applied_prefix = self.events.len();
    }

    fn clear_kind(&mut self, damage: bool) {
        self.shadow_applied_prefix = self
            .shadow_applied_events()
            .iter()
            .filter(|event| matches!(event, HostHealthEvent::Damage(_)) != damage)
            .count();
        self.events
            .retain(|event| matches!(event, HostHealthEvent::Damage(_)) != damage);
    }
}

/// Host-to-shadow event state for one GameLogic instance.
///
/// Pending events and presentation receipts are transient frame state and are
/// intentionally not part of GameLogic's Xfer representation.
#[derive(Debug, Default)]
pub struct HostHealthEvents {
    events: Vec<HostHealthEvent>,
    last_damage: Vec<super::host_damage_log::HostDamageEvent>,
    last_heal: Vec<super::host_heal_log::HostHealEvent>,
    cumulative_damage: f32,
    cumulative_kills: u32,
    early_ordered_batch: Option<HostHealthBatch>,
}

impl HostHealthEvents {
    /// Queue damage whose health effect still needs admission by the authority.
    /// Untyped callers use C++ DAMAGE_EXPLOSION ordinal 0.
    pub(crate) fn record_damage(
        &mut self,
        target: ObjectId,
        amount: f32,
        source: Option<ObjectId>,
        destroyed: bool,
    ) {
        self.record_damage_typed(target, amount, source, destroyed, 0);
    }

    pub(crate) fn record_damage_typed(
        &mut self,
        target: ObjectId,
        amount: f32,
        source: Option<ObjectId>,
        destroyed: bool,
        damage_type_ordinal: u32,
    ) {
        self.push_damage(super::host_damage_log::HostDamageEvent::pending(
            target,
            amount,
            source,
            destroyed,
            damage_type_ordinal,
        ));
    }

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
    pub(crate) fn record_heal(&mut self, target: ObjectId, health: f32) {
        self.push_heal(super::host_heal_log::HostHealEvent::pending(target, health));
    }

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

    /// Finish a host-only boundary, including an earlier eager handoff that
    /// has not reached its session boundary. Earlier records precede new ones.
    pub(crate) fn drain_for_host_boundary(&mut self) -> Vec<HostHealthEvent> {
        let mut drained = self
            .take_early_batch()
            .map(|batch| batch.events)
            .unwrap_or_default();
        drained.extend(std::mem::take(&mut self.events));
        self.cache_completed(&drained);
        drained
    }

    /// Preserve the eager receipt and append later events for this same
    /// boundary. The prefix records shadow admission, not owner HP writes.
    pub(crate) fn drain_for_shadow_boundary(&mut self) -> HostHealthBatch {
        let mut batch = self.take_early_batch().unwrap_or_default();
        batch.events.extend(std::mem::take(&mut self.events));
        self.cache_completed(&batch.events);
        batch
    }

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
        self.clear_early_kind(true);
        self.reset_cumulative();
    }

    pub(crate) fn clear_heal(&mut self) {
        self.events
            .retain(|event| !matches!(event, HostHealthEvent::Heal(_)));
        self.last_heal.clear();
        self.clear_early_kind(false);
    }

    fn clear_early_kind(&mut self, damage: bool) {
        if let Some(batch) = &mut self.early_ordered_batch {
            batch.clear_kind(damage);
            if batch.events.is_empty() {
                self.early_ordered_batch = None;
            }
        }
    }

    /// Clear one owner's event and receipt state; used by that owner's reset.
    pub(crate) fn clear(&mut self) {
        self.events.clear();
        self.last_damage.clear();
        self.last_heal.clear();
        self.early_ordered_batch = None;
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

    pub(crate) fn set_early_batch(&mut self, batch: HostHealthBatch) {
        self.early_ordered_batch = (!batch.events.is_empty()).then_some(batch);
    }

    pub(crate) fn take_early_batch(&mut self) -> Option<HostHealthBatch> {
        self.early_ordered_batch.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damage_and_heal_share_one_append_ordered_drain() {
        let mut events = HostHealthEvents::default();
        events.record_heal(ObjectId(1), 80.0);
        events.record_damage(ObjectId(1), 10.0, None, false);
        let drained = events.drain_ordered();
        assert!(matches!(
            drained.as_slice(),
            [HostHealthEvent::Heal(_), HostHealthEvent::Damage(_)]
        ));
        assert_eq!(events.snapshot_last_damage().len(), 1);
        assert_eq!(events.snapshot_last_heal().len(), 1);
    }

    #[test]
    fn damage_and_heal_share_order_in_both_directions() {
        let mut events = HostHealthEvents::default();
        events.record_damage(ObjectId(1), 10.0, None, false);
        events.record_heal(ObjectId(1), 80.0);
        assert!(matches!(
            events.drain_ordered().as_slice(),
            [HostHealthEvent::Damage(_), HostHealthEvent::Heal(_)]
        ));
    }

    #[test]
    fn typed_drains_leave_the_other_kind_queued() {
        let mut events = HostHealthEvents::default();
        events.record_damage(ObjectId(1), 5.0, None, false);
        events.record_heal(ObjectId(1), 90.0);
        assert_eq!(events.drain_damage().len(), 1);
        assert_eq!(events.snapshot_ordered().len(), 1);
        assert_eq!(events.drain_heal().len(), 1);
        assert!(events.snapshot_ordered().is_empty());
    }

    #[test]
    fn queues_and_early_batches_are_independent() {
        let mut a = HostHealthEvents::default();
        let mut b = HostHealthEvents::default();
        a.record_damage(ObjectId(7), 4.0, None, false);
        let mut batch = a.drain_for_shadow_boundary();
        batch.mark_shadow_applied();
        a.set_early_batch(batch);
        b.record_heal(ObjectId(7), 80.0);
        assert!(a.snapshot_ordered().is_empty());
        assert_eq!(b.snapshot_heal().len(), 1);
        assert_eq!(
            a.take_early_batch().unwrap().shadow_applied_events().len(),
            1
        );
        assert!(b.take_early_batch().is_none());
        b.clear();
        assert!(a.snapshot_last_damage().len() == 1);
    }

    #[test]
    fn health_handoff_retains_the_applied_prefix_before_later_events() {
        let mut events = HostHealthEvents::default();
        events.record_damage(ObjectId(1), 20.0, None, false);
        let mut batch = events.drain_for_shadow_boundary();
        batch.mark_shadow_applied();
        events.set_early_batch(batch);
        events.record_heal(ObjectId(1), 90.0);
        events.record_damage(ObjectId(1), 10.0, None, false);
        let batch = events.drain_for_shadow_boundary();
        assert_eq!(batch.shadow_applied_events().len(), 1);
        assert!(matches!(
            batch.pending_shadow_events(),
            [HostHealthEvent::Heal(_), HostHealthEvent::Damage(_)]
        ));
        assert_eq!(events.snapshot_last_damage().len(), 2);
        assert_eq!(events.snapshot_last_heal().len(), 1);
    }

    #[test]
    fn health_handoff_partial_clear_preserves_the_opposite_admission_state() {
        for clear_damage in [true, false] {
            let mut events = HostHealthEvents::default();
            events.record_damage(ObjectId(1), 10.0, None, false);
            events.record_heal(ObjectId(1), 90.0);
            let mut batch = events.drain_for_shadow_boundary();
            batch.mark_shadow_applied();
            events.set_early_batch(batch);
            events.record_damage(ObjectId(1), 5.0, None, false);
            events.record_heal(ObjectId(1), 80.0);
            let batch = events.drain_for_shadow_boundary();
            events.set_early_batch(batch);
            if clear_damage {
                events.clear_damage();
            } else {
                events.clear_heal();
            }
            let batch = events.drain_for_shadow_boundary();
            assert_eq!(batch.events().len(), 2);
            assert_eq!(batch.shadow_applied_events().len(), 1);
            assert_eq!(batch.pending_shadow_events().len(), 1);
            assert!(
                batch
                    .events()
                    .iter()
                    .all(|event| matches!(event, HostHealthEvent::Damage(_)) != clear_damage)
            );
        }
    }

    #[test]
    fn health_event_fallback_partial_clear_retires_only_its_kind_from_an_early_receipt() {
        for clear_damage in [true, false] {
            let mut events = HostHealthEvents::default();
            events.record_damage(ObjectId(1), 10.0, None, false);
            events.record_heal(ObjectId(1), 90.0);
            let batch = events.drain_for_shadow_boundary();
            events.set_early_batch(batch);
            if clear_damage {
                events.clear_damage();
            } else {
                events.clear_heal();
            }
            let batch = events.drain_for_host_boundary();
            assert_eq!(batch.len(), 1);
            assert_eq!(
                matches!(batch[0], HostHealthEvent::Damage(_)),
                !clear_damage
            );
            assert!(events.take_early_batch().is_none());
        }
    }
}
