//! Frame-local host damage log for GameWorld shadow parity.
//!
//! `Object::take_damage_from` records actual HP damage applied (post-armor).
//! GameLogic/engine drains the log after a host tick and feeds `GameWorldShadow`.
//!
//! Thread-local avoids threading `&mut GameLogic` through every Object method
//! (borrow-first at the drain boundary; no Arc on the world).

use super::ObjectId;
use std::cell::{Cell, RefCell};

/// One damage application observed on the host authority.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostDamageEvent {
    pub target: ObjectId,
    /// Actual HP removed after armor/battle-plan scalars.
    pub amount: f32,
    pub source: Option<ObjectId>,
    pub destroyed: bool,
}

thread_local! {
    static LOG: RefCell<Vec<HostDamageEvent>> = const { RefCell::new(Vec::new()) };
    static LAST_DRAIN: RefCell<Vec<HostDamageEvent>> = const { RefCell::new(Vec::new()) };
    static CUM_DAMAGE: Cell<f32> = const { Cell::new(0.0) };
    static CUM_KILLS: Cell<u32> = const { Cell::new(0) };
}

/// Record a damage event (called from Object::take_damage_from).
pub fn record(target: ObjectId, amount: f32, source: Option<ObjectId>, destroyed: bool) {
    if amount <= 0.0 && !destroyed {
        return;
    }
    CUM_DAMAGE.set(CUM_DAMAGE.get() + amount.max(0.0));
    if destroyed {
        CUM_KILLS.set(CUM_KILLS.get().saturating_add(1));
    }
    LOG.with(|log| {
        log.borrow_mut().push(HostDamageEvent {
            target,
            amount,
            source,
            destroyed,
        });
    });
}

/// Drain all events since last drain (order preserved).
pub fn drain() -> Vec<HostDamageEvent> {
    let v = LOG.with(|log| std::mem::take(&mut *log.borrow_mut()));
    // Keep last non-empty batch for PresentationFrame after shadow session.
    if !v.is_empty() {
        LAST_DRAIN.with(|last| *last.borrow_mut() = v.clone());
    }
    v
}

/// Peek count without draining (tests).
pub fn len() -> usize {
    LOG.with(|log| log.borrow().len())
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
    LOG.with(|log| log.borrow_mut().clear());
    LAST_DRAIN.with(|last| last.borrow_mut().clear());
    reset_cumulative();
}

/// Take events from the most recent non-empty `drain()` (PresentationFrame sole consumer).
pub fn take_last_drain() -> Vec<HostDamageEvent> {
    LAST_DRAIN.with(|last| std::mem::take(&mut *last.borrow_mut()))
}

/// Non-destructive peek (tests).
pub fn last_drain_snapshot() -> Vec<HostDamageEvent> {
    LAST_DRAIN.with(|last| last.borrow().clone())
}

#[cfg(test)]
mod tests {
    use super::*;

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
