//! Frame-local host special-power log for GameWorld SetSpecialPower parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostSpecialPowerEvent {
    pub object: ObjectId,
    pub ready: bool,
    /// Seconds remaining on aggregate object SP timer residual.
    pub cooldown_remaining: f32,
    /// Full cooldown duration residual (seconds).
    pub cooldown: f32,
    /// C++ isDisabled / pauseCountdown: countdown must not advance.
    pub frozen: bool,
}

thread_local! {
    static LOG: RefCell<Vec<HostSpecialPowerEvent>> = RefCell::new(Vec::new());
}

pub fn record(object: ObjectId, ready: bool, cooldown_remaining: f32, cooldown: f32, frozen: bool) {
    LOG.with(|log| {
        log.borrow_mut().push(HostSpecialPowerEvent {
            object,
            ready,
            cooldown_remaining: cooldown_remaining.max(0.0),
            cooldown: cooldown.max(0.0),
            frozen,
        });
    });
}

pub fn drain() -> Vec<HostSpecialPowerEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
