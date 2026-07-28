//! Wave 679: host production spawn ObjectId ready residual log.
//!
//! `host_spawn_production_unit` still allocates host ObjectIds via `create_object`.
//! Successful production spawns record here; host drains and applies door/notify/
//! exit/path residual so the spawn ID flows through a drainable ready channel
//! before presentation side effects.
//!
//! Fail-closed: empty drain is valid (no production spawns this frame).
//! Not full GameWorld spawn-ID authority / playable_claim.

use crate::game_logic::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, PartialEq)]
pub struct HostProductionSpawnReadyEvent {
    pub unit: ObjectId,
    pub producer: ObjectId,
    pub template: String,
    pub spawn_pos: [f32; 3],
    pub rally: Option<[f32; 3]>,
}

thread_local! {
    static LOG: RefCell<Vec<HostProductionSpawnReadyEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostProductionSpawnReadyEvent>> = RefCell::new(Vec::new());
}

pub fn record(
    unit: ObjectId,
    producer: ObjectId,
    template: String,
    spawn_pos: [f32; 3],
    rally: Option<[f32; 3]>,
) {
    LOG.with(|log| {
        log.borrow_mut().push(HostProductionSpawnReadyEvent {
            unit,
            producer,
            template,
            spawn_pos,
            rally,
        });
    });
}

pub fn drain() -> Vec<HostProductionSpawnReadyEvent> {
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
        record(
            ObjectId(679),
            ObjectId(1),
            "Ranger".into(),
            [1.0, 0.0, 2.0],
            Some([3.0, 0.0, 4.0]),
        );
        let d = drain();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].unit.0, 679);
        assert_eq!(d[0].producer.0, 1);
        assert_eq!(d[0].template, "Ranger");
        assert_eq!(d[0].spawn_pos, [1.0, 0.0, 2.0]);
        assert_eq!(d[0].rally, Some([3.0, 0.0, 4.0]));
        assert!(drain().is_empty());
        clear();
    }
}
