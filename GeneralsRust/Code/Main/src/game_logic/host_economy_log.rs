//! Frame-local host economy log for GameWorld shadow parity.
//!
//! Player cash mutations record post-change absolute supplies (and power when
//! known). End-of-tick economy authority applies SetSupplies mutations then
//! writebacks host players.

use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostEconomyEvent {
    pub player_id: u32,
    /// Absolute supplies after the host mutation.
    pub supplies: u32,
    /// Absolute power_available after the host mutation (best-effort).
    pub power_available: i32,
}

thread_local! {
    static LOG: RefCell<Vec<HostEconomyEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostEconomyEvent>> = RefCell::new(Vec::new());
}

pub fn record(player_id: u32, supplies: u32, power_available: i32) {
    LOG.with(|log| {
        log.borrow_mut().push(HostEconomyEvent {
            player_id,
            supplies,
            power_available,
        });
    });
}

pub fn drain() -> Vec<HostEconomyEvent> {
    let v = LOG.with(|log| std::mem::take(&mut *log.borrow_mut()));
    // Keep last non-empty batch for PresentationFrame after shadow session.
    if !v.is_empty() {
        LAST_DRAIN.with(|last| *last.borrow_mut() = v.clone());
    }
    v
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
    LAST_DRAIN.with(|last| last.borrow_mut().clear());
}

pub fn len() -> usize {
    LOG.with(|log| log.borrow().len())
}

/// Take events from the most recent non-empty `drain()` (PresentationFrame sole consumer).
pub fn take_last_drain() -> Vec<HostEconomyEvent> {
    LAST_DRAIN.with(|last| std::mem::take(&mut *last.borrow_mut()))
}

/// Non-destructive peek (tests).
pub fn last_drain_snapshot() -> Vec<HostEconomyEvent> {
    LAST_DRAIN.with(|last| last.borrow().clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn economy_log_drain_and_last_snapshot() {
        clear();
        record(0, 1000, 5);
        record(1, 500, -2);
        assert_eq!(len(), 2);
        let v = drain();
        assert_eq!(v.len(), 2);
        assert!(drain().is_empty());
        assert_eq!(last_drain_snapshot().len(), 2);
        assert_eq!(last_drain_snapshot()[0].supplies, 1000);
    }
}
