//! Wave 627: GameWorld production-door phase-change ready residual log.
//!
//! When `writeback_production_door_to_host` observes a phase change, it records
//! here. Host drains and applies door model-condition residual so GameWorld
//! owns phase/end-frame while host owns visual bits.
//!
//! Fail-closed: empty drain is valid (no door phase changes this frame).

use crate::game_logic::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct HostProductionDoorReadyEvent {
    pub producer: ObjectId,
    pub previous_phase: u8,
    pub new_phase: u8,
    pub phase_end_frame: u32,
    pub hold_open: bool,
}

thread_local! {
    static LOG: RefCell<Vec<HostProductionDoorReadyEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostProductionDoorReadyEvent>> = RefCell::new(Vec::new());
}

pub fn record(
    producer: ObjectId,
    previous_phase: u8,
    new_phase: u8,
    phase_end_frame: u32,
    hold_open: bool,
) {
    LOG.with(|log| {
        log.borrow_mut().push(HostProductionDoorReadyEvent {
            producer,
            previous_phase,
            new_phase,
            phase_end_frame,
            hold_open,
        });
    });
}

pub fn drain() -> Vec<HostProductionDoorReadyEvent> {
    LOG.with(|log| {
        let events = std::mem::take(&mut *log.borrow_mut());
        LAST_DRAIN.with(|last| *last.borrow_mut() = events.clone());
        events
    })
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
    LAST_DRAIN.with(|last| last.borrow_mut().clear());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_drain_roundtrip() {
        clear();
        record(ObjectId(4), 1, 2, 100, false);
        let d = drain();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].producer.0, 4);
        assert_eq!(d[0].previous_phase, 1);
        assert_eq!(d[0].new_phase, 2);
        assert!(drain().is_empty());
        clear();
    }
}
