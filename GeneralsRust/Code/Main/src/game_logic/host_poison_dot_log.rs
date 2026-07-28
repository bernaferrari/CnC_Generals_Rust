//! Frame-local PoisonedBehavior DoT log for GameWorld shadow parity.
//!
//! Under coupled dual-tick, GW sole-ticks poison intervals and records DoT here
//! so host can apply UNRESISTABLE damage without dual-ticking the timer.

use super::ObjectId;
use crate::game_logic::host_usa_pilot::HostDeathType;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostPoisonDotEvent {
    pub object: ObjectId,
    pub amount: f32,
    pub death_type: HostDeathType,
}

thread_local! {
    static LOG: RefCell<Vec<HostPoisonDotEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, amount: f32, death_type: HostDeathType) {
    LOG.with(|log| {
        log.borrow_mut().push(HostPoisonDotEvent {
            object,
            amount,
            death_type,
        })
    });
}

pub fn has_pending(object: ObjectId) -> bool {
    LOG.with(|log| log.borrow().iter().any(|e| e.object == object))
}

pub fn drain() -> Vec<HostPoisonDotEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
