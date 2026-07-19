//! Frame-local host stealth/tunnel/passenger-fire flag log for GameWorld SetStealthFlags parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostStealthFlagsEvent {
    pub object: ObjectId,
    pub innate_stealth: bool,
    pub stealth_breaks_on_attack: bool,
    pub stealth_breaks_on_move: bool,
    pub is_tunnel_network: bool,
    pub passengers_allowed_to_fire: bool,
}

thread_local! {
    static LOG: RefCell<Vec<HostStealthFlagsEvent>> = RefCell::new(Vec::new());
}

pub fn record(ev: HostStealthFlagsEvent) {
    LOG.with(|log| log.borrow_mut().push(ev));
}

pub fn drain() -> Vec<HostStealthFlagsEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
