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
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}

pub fn len() -> usize {
    LOG.with(|log| log.borrow().len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn economy_log_drain_order() {
        clear();
        record(0, 100, 10);
        record(1, 50, 0);
        let v = drain();
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].supplies, 100);
        assert_eq!(v[1].player_id, 1);
        assert!(drain().is_empty());
    }
}
