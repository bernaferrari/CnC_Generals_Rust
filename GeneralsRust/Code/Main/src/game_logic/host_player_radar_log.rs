//! Frame-local player radar provider count logs for GW shadow parity.

use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct PlayerRadarEvent {
    pub player_id: u32,
    pub radar_count: i32,
    pub had_radar: bool,
    pub has_radar: bool,
}

thread_local! {
    static EVENTS: RefCell<Vec<PlayerRadarEvent>> = RefCell::new(Vec::new());
}

pub fn record(ev: PlayerRadarEvent) {
    EVENTS.with(|l| l.borrow_mut().push(ev));
}

pub fn drain() -> Vec<PlayerRadarEvent> {
    EVENTS.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

pub fn clear() {
    EVENTS.with(|l| l.borrow_mut().clear());
}
