//! Wave 631: GameWorld economy writeback ready residual log.
//!
//! When `writeback_economy_to_host` changes player economy fields, it records
//! here. Host drains and applies presentation/UI residual via host_economy_log
//! so GameWorld owns the economy last-write while host owns presentation
//! bookkeeping.
//!
//! Fail-closed: empty drain is valid (no economy changes this frame).

use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct HostEconomyReadyEvent {
    pub player_id: u32,
    pub previous_supplies: u32,
    pub supplies: u32,
    pub previous_power: i32,
    pub power_available: i32,
    pub previous_radar_count: i32,
    pub radar_count: i32,
    pub previous_radar_disabled: bool,
    pub radar_disabled: bool,
    pub previous_alive: bool,
    pub is_alive: bool,
    pub supplies_changed: bool,
    pub power_changed: bool,
    pub radar_changed: bool,
    pub alive_changed: bool,
}

thread_local! {
    static LOG: RefCell<Vec<HostEconomyReadyEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostEconomyReadyEvent>> = RefCell::new(Vec::new());
}

pub fn record(ev: HostEconomyReadyEvent) {
    LOG.with(|log| log.borrow_mut().push(ev));
}

pub fn drain() -> Vec<HostEconomyReadyEvent> {
    LOG.with(|log| {
        let events = std::mem::take(&mut *log.borrow_mut());
        LAST_DRAIN.with(|last| *last.borrow_mut() = events.clone());
        events
    })
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
    LAST_DRAIN.with(|last| last.borrow_mut().clear());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_drain_roundtrip() {
        clear();
        record(HostEconomyReadyEvent {
            player_id: 1,
            previous_supplies: 1000,
            supplies: 900,
            previous_power: 5,
            power_available: 3,
            previous_radar_count: 1,
            radar_count: 1,
            previous_radar_disabled: false,
            radar_disabled: false,
            previous_alive: true,
            is_alive: true,
            supplies_changed: true,
            power_changed: true,
            radar_changed: false,
            alive_changed: false,
        });
        let d = drain();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].player_id, 1);
        assert_eq!(d[0].supplies, 900);
        assert!(d[0].supplies_changed);
        assert!(drain().is_empty());
        clear();
    }
}
