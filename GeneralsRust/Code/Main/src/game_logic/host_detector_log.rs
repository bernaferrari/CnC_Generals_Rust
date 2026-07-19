//! Frame-local host detector log for GameWorld SetDetector parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostDetectorEvent {
    pub object: ObjectId,
    pub is_detector: bool,
    pub detection_range: f32,
    pub detection_rate_frames: u32,
}

thread_local! {
    static LOG: RefCell<Vec<HostDetectorEvent>> = RefCell::new(Vec::new());
}

pub fn record(
    object: ObjectId,
    is_detector: bool,
    detection_range: f32,
    detection_rate_frames: u32,
) {
    LOG.with(|log| {
        log.borrow_mut().push(HostDetectorEvent {
            object,
            is_detector,
            detection_range: detection_range.max(0.0),
            detection_rate_frames,
        });
    });
}

pub fn drain() -> Vec<HostDetectorEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
