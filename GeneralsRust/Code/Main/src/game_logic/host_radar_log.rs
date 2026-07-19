//! Frame-local host player radar log for GameWorld SetPlayerRadar parity.

use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostRadarEvent {
    pub player_id: u32,
    pub radar_count: i32,
    pub radar_disabled: bool,
}

thread_local! {
    static LOG: RefCell<Vec<HostRadarEvent>> = RefCell::new(Vec::new());
}

pub fn record(player_id: u32, radar_count: i32, radar_disabled: bool) {
    LOG.with(|log| {
        log.borrow_mut().push(HostRadarEvent {
            player_id,
            radar_count,
            radar_disabled,
        });
    });
}

pub fn drain() -> Vec<HostRadarEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
