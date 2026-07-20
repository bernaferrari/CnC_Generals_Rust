//! Frame-local host BodyDamageType residual for GameWorld SetBodyDamage parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostBodyDamageEvent {
    pub object: ObjectId,
    /// 0 pristine .. 3 rubble.
    pub body_damage_state: u8,
}

thread_local! {
    static LOG: RefCell<Vec<HostBodyDamageEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, body_damage_state: u8) {
    LOG.with(|log| {
        log.borrow_mut().push(HostBodyDamageEvent {
            object,
            body_damage_state: body_damage_state.min(3),
        });
    });
}

pub fn drain() -> Vec<HostBodyDamageEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
