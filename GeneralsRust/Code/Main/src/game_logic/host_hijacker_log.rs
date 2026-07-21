//! Frame-local host hijacker residual for GameWorld SetHijacker parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostHijackerEvent {
    pub object: ObjectId,
    pub hijack_vehicle_host: u32,
    pub hijacker_in_vehicle: bool,
    pub hijacker_update_active: bool,
    pub hijacker_was_airborne: bool,
    pub hijacker_eject_pos: Option<[f32; 3]>,
    pub hive_slave_respawn_frame: u32,
    pub next_detection_scan_frame: u32,
}

thread_local! {
    static LOG: RefCell<Vec<HostHijackerEvent>> = RefCell::new(Vec::new());
}

pub fn record(
    object: ObjectId,
    hijack_vehicle_host: u32,
    hijacker_in_vehicle: bool,
    hijacker_update_active: bool,
    hijacker_was_airborne: bool,
    hijacker_eject_pos: Option<[f32; 3]>,
    hive_slave_respawn_frame: u32,
    next_detection_scan_frame: u32,
) {
    LOG.with(|log| {
        log.borrow_mut().push(HostHijackerEvent {
            object,
            hijack_vehicle_host,
            hijacker_in_vehicle,
            hijacker_update_active,
            hijacker_was_airborne,
            hijacker_eject_pos,
            hive_slave_respawn_frame,
            next_detection_scan_frame,
        });
    });
}

pub fn drain() -> Vec<HostHijackerEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
