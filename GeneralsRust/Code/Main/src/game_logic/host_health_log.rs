//! One ordered, frame-local transport for host HP mutations and observations.
//!
//! Damage and absolute-health records share a single insertion-ordered Vec.
//! Typed modules remain the producer/consumer API; this module owns the only
//! pending queue and exposes a merged drain for coupled shadow admission.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum HostHealthEvent {
    Damage(super::host_damage_log::HostDamageEvent),
    Heal(super::host_heal_log::HostHealEvent),
}

thread_local! {
    static EVENTS: RefCell<Vec<HostHealthEvent>> = const { RefCell::new(Vec::new()) };
    // Per-type last-drain views are presentation caches, never pending authority.
    static LAST_DAMAGE: RefCell<Vec<super::host_damage_log::HostDamageEvent>> = const { RefCell::new(Vec::new()) };
    static LAST_HEAL: RefCell<Vec<super::host_heal_log::HostHealEvent>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn record_damage(event: super::host_damage_log::HostDamageEvent) {
    EVENTS.with(|events| events.borrow_mut().push(HostHealthEvent::Damage(event)));
}

pub(crate) fn record_heal(event: super::host_heal_log::HostHealEvent) {
    EVENTS.with(|events| events.borrow_mut().push(HostHealthEvent::Heal(event)));
}

pub(crate) fn snapshot_ordered() -> Vec<HostHealthEvent> {
    EVENTS.with(|events| events.borrow().clone())
}

/// Drain all HP records in the order their owner operations emitted them.
pub(crate) fn drain_ordered() -> Vec<HostHealthEvent> {
    let drained = EVENTS.with(|events| std::mem::take(&mut *events.borrow_mut()));
    let mut damage = Vec::new();
    let mut heal = Vec::new();
    for event in &drained {
        match event {
            HostHealthEvent::Damage(event) => damage.push(*event),
            HostHealthEvent::Heal(event) => heal.push(*event),
        }
    }
    if !damage.is_empty() {
        LAST_DAMAGE.with(|last| *last.borrow_mut() = damage);
    }
    if !heal.is_empty() {
        LAST_HEAL.with(|last| *last.borrow_mut() = heal);
    }
    drained
}

pub(crate) fn snapshot_damage() -> Vec<super::host_damage_log::HostDamageEvent> {
    EVENTS.with(|events| {
        events
            .borrow()
            .iter()
            .filter_map(|event| match event {
                HostHealthEvent::Damage(event) => Some(*event),
                HostHealthEvent::Heal(_) => None,
            })
            .collect()
    })
}

pub(crate) fn snapshot_heal() -> Vec<super::host_heal_log::HostHealEvent> {
    EVENTS.with(|events| {
        events
            .borrow()
            .iter()
            .filter_map(|event| match event {
                HostHealthEvent::Damage(_) => None,
                HostHealthEvent::Heal(event) => Some(*event),
            })
            .collect()
    })
}

pub(crate) fn has_damage(object: ObjectId) -> bool {
    EVENTS.with(|events| {
        events
            .borrow()
            .iter()
            .any(|event| matches!(event, HostHealthEvent::Damage(event) if event.target == object))
    })
}

pub(crate) fn has_heal(object: ObjectId) -> bool {
    EVENTS.with(|events| {
        events
            .borrow()
            .iter()
            .any(|event| matches!(event, HostHealthEvent::Heal(event) if event.target == object))
    })
}

pub(crate) fn len_damage() -> usize {
    EVENTS.with(|events| {
        events
            .borrow()
            .iter()
            .filter(|event| matches!(event, HostHealthEvent::Damage(_)))
            .count()
    })
}

pub(crate) fn len_heal() -> usize {
    EVENTS.with(|events| {
        events
            .borrow()
            .iter()
            .filter(|event| matches!(event, HostHealthEvent::Heal(_)))
            .count()
    })
}

pub(crate) fn drain_damage() -> Vec<super::host_damage_log::HostDamageEvent> {
    let mut selected = Vec::new();
    EVENTS.with(|events| {
        let mut events = events.borrow_mut();
        let mut keep = Vec::with_capacity(events.len());
        for event in events.drain(..) {
            match event {
                HostHealthEvent::Damage(event) => selected.push(event),
                other => keep.push(other),
            }
        }
        *events = keep;
    });
    if !selected.is_empty() {
        LAST_DAMAGE.with(|last| *last.borrow_mut() = selected.clone());
    }
    selected
}

pub(crate) fn drain_heal() -> Vec<super::host_heal_log::HostHealEvent> {
    let mut selected = Vec::new();
    EVENTS.with(|events| {
        let mut events = events.borrow_mut();
        let mut keep = Vec::with_capacity(events.len());
        for event in events.drain(..) {
            match event {
                HostHealthEvent::Heal(event) => selected.push(event),
                other => keep.push(other),
            }
        }
        *events = keep;
    });
    if !selected.is_empty() {
        LAST_HEAL.with(|last| *last.borrow_mut() = selected.clone());
    }
    selected
}

pub(crate) fn clear_damage() {
    EVENTS.with(|events| {
        events
            .borrow_mut()
            .retain(|event| !matches!(event, HostHealthEvent::Damage(_)))
    });
    LAST_DAMAGE.with(|last| last.borrow_mut().clear());
}

pub(crate) fn clear_heal() {
    EVENTS.with(|events| {
        events
            .borrow_mut()
            .retain(|event| !matches!(event, HostHealthEvent::Heal(_)))
    });
    LAST_HEAL.with(|last| last.borrow_mut().clear());
}

pub(crate) fn take_last_damage() -> Vec<super::host_damage_log::HostDamageEvent> {
    LAST_DAMAGE.with(|last| std::mem::take(&mut *last.borrow_mut()))
}

pub(crate) fn snapshot_last_damage() -> Vec<super::host_damage_log::HostDamageEvent> {
    LAST_DAMAGE.with(|last| last.borrow().clone())
}

pub(crate) fn take_last_heal() -> Vec<super::host_heal_log::HostHealEvent> {
    LAST_HEAL.with(|last| std::mem::take(&mut *last.borrow_mut()))
}

pub(crate) fn snapshot_last_heal() -> Vec<super::host_heal_log::HostHealEvent> {
    LAST_HEAL.with(|last| last.borrow().clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{host_damage_log, host_heal_log};

    #[test]
    fn damage_and_heal_share_one_append_ordered_drain() {
        host_damage_log::clear();
        host_heal_log::clear();
        host_heal_log::record(ObjectId(1), 80.0);
        host_damage_log::record(ObjectId(1), 10.0, None, false);
        let events = drain_ordered();
        assert!(matches!(
            events.as_slice(),
            [HostHealthEvent::Heal(_), HostHealthEvent::Damage(_)]
        ));
        assert_eq!(host_damage_log::last_drain_snapshot().len(), 1);
        assert_eq!(host_heal_log::last_drain_snapshot().len(), 1);
    }

    #[test]
    fn damage_and_heal_share_order_in_both_directions() {
        use crate::game_logic::{host_damage_log, host_heal_log};

        host_damage_log::clear();
        host_heal_log::clear();
        host_damage_log::record(ObjectId(1), 10.0, None, false);
        host_heal_log::record(ObjectId(1), 80.0);
        assert!(matches!(
            drain_ordered().as_slice(),
            [HostHealthEvent::Damage(_), HostHealthEvent::Heal(_)]
        ));
    }

    #[test]
    fn typed_drains_leave_the_other_kind_queued() {
        host_damage_log::clear();
        host_heal_log::clear();
        host_damage_log::record(ObjectId(1), 5.0, None, false);
        host_heal_log::record(ObjectId(1), 90.0);
        assert_eq!(host_damage_log::drain().len(), 1);
        assert_eq!(snapshot_ordered().len(), 1);
        assert_eq!(host_heal_log::drain().len(), 1);
        assert!(snapshot_ordered().is_empty());
    }
}
