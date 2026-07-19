//! Frame-local host turret log for GameWorld SetTurret parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostTurretEvent {
    pub object: ObjectId,
    pub angle_deg: f32,
    pub pitch_deg: f32,
    pub holding: bool,
    pub idle_scanning: bool,
}

thread_local! {
    static LOG: RefCell<Vec<HostTurretEvent>> = RefCell::new(Vec::new());
}

pub fn record(
    object: ObjectId,
    angle_deg: f32,
    pitch_deg: f32,
    holding: bool,
    idle_scanning: bool,
) {
    LOG.with(|log| {
        log.borrow_mut().push(HostTurretEvent {
            object,
            angle_deg,
            pitch_deg,
            holding,
            idle_scanning,
        });
    });
}

pub fn drain() -> Vec<HostTurretEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
