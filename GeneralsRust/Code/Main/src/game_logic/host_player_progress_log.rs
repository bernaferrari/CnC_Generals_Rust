//! Frame-local host player rank/skill/science/bounty log for GameWorld parity.

use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostPlayerProgressEvent {
    pub player_id: u32,
    pub rank_level: u32,
    pub skill_points: i32,
    pub science_purchase_points: i32,
    pub cash_bounty_percent: f32,
}

thread_local! {
    static LOG: RefCell<Vec<HostPlayerProgressEvent>> = RefCell::new(Vec::new());
}

pub fn record(
    player_id: u32,
    rank_level: u32,
    skill_points: i32,
    science_purchase_points: i32,
    cash_bounty_percent: f32,
) {
    LOG.with(|log| {
        log.borrow_mut().push(HostPlayerProgressEvent {
            player_id,
            rank_level,
            skill_points,
            science_purchase_points,
            cash_bounty_percent,
        });
    });
}

pub fn drain() -> Vec<HostPlayerProgressEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
