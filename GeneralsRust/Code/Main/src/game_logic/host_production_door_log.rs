//! Frame-local host production door residual for GameWorld SetProductionDoor parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostProductionDoorEvent {
    pub producer: ObjectId,
    /// 0 idle, 1 opening, 2 wait open, 3 wait close, 4 closing.
    pub production_door_phase: u8,
    pub production_door_phase_end_frame: u32,
    pub production_door_hold_open: bool,
}

thread_local! {
    static LOG: RefCell<Vec<HostProductionDoorEvent>> = RefCell::new(Vec::new());
}

pub fn record(
    producer: ObjectId,
    production_door_phase: u8,
    production_door_phase_end_frame: u32,
    production_door_hold_open: bool,
) {
    LOG.with(|log| {
        log.borrow_mut().push(HostProductionDoorEvent {
            producer,
            production_door_phase,
            production_door_phase_end_frame,
            production_door_hold_open,
        });
    });
}

pub fn drain() -> Vec<HostProductionDoorEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
