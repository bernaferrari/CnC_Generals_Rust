//! Wave 620: GameWorld rebuild-hole ready residual log.
//! Wave 740: ready events may carry pre-spawned GameWorld entity raw ids for the
//! worker + reconstructing structure (entity-first under construction sole-tick).
//!
//! Under CONSTRUCTION_AUTHORITY sole-tick, `writeback_rebuild_producer_to_host`
//! records rebuild holes whose ready frame has been reached and that are not
//! already reconstructing. Host `update_rebuild_holes` drains this log so
//! GameWorld decides readiness; host binds ObjectIds to pre-spawned entities
//! when raws are present (Wave 740).
//!
//! Fail-closed: empty drain is valid (no rebuild starts this frame).
//! `playable_claim` stays false.

use crate::game_logic::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct HostRebuildReadyEvent {
    pub hole: ObjectId,
    pub ready_frame: u32,
    /// Wave 740: pre-spawned worker entity raw id (construction sole-tick).
    pub worker_entity_raw: Option<u32>,
    /// Wave 740: pre-spawned reconstructing structure entity raw id.
    pub rebuild_entity_raw: Option<u32>,
    /// Wave 740: GW hole pose residual for spawn.
    pub spawn_pos: Option<[f32; 3]>,
    /// Wave 740: hole orientation residual.
    pub orientation: f32,
    /// Wave 740: rebuild template name residual.
    pub rebuild_template: String,
}

thread_local! {
    static LOG: RefCell<Vec<HostRebuildReadyEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostRebuildReadyEvent>> = RefCell::new(Vec::new());
}

pub fn record(hole: ObjectId, ready_frame: u32) {
    record_with_entities(hole, ready_frame, None, None, None, 0.0, String::new());
}

/// Wave 740: record ready hole with optional pre-spawned GW entities + pose.
pub fn record_with_entities(
    hole: ObjectId,
    ready_frame: u32,
    worker_entity_raw: Option<u32>,
    rebuild_entity_raw: Option<u32>,
    spawn_pos: Option<[f32; 3]>,
    orientation: f32,
    rebuild_template: impl Into<String>,
) {
    LOG.with(|log| {
        log.borrow_mut().push(HostRebuildReadyEvent {
            hole,
            ready_frame,
            worker_entity_raw,
            rebuild_entity_raw,
            spawn_pos,
            orientation,
            rebuild_template: rebuild_template.into(),
        });
    });
}

pub fn drain() -> Vec<HostRebuildReadyEvent> {
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
        record(ObjectId(11), 90);
        let d = drain();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].hole.0, 11);
        assert_eq!(d[0].ready_frame, 90);
        assert!(d[0].worker_entity_raw.is_none());
        assert!(drain().is_empty());
        clear();
    }

    #[test]
    fn record_with_entities_carries_raws() {
        clear();
        record_with_entities(
            ObjectId(2),
            30,
            Some(100),
            Some(101),
            Some([1.0, 0.0, 2.0]),
            0.5,
            "GLATunnelNetwork",
        );
        let d = drain();
        assert_eq!(d[0].worker_entity_raw, Some(100));
        assert_eq!(d[0].rebuild_entity_raw, Some(101));
        assert_eq!(d[0].rebuild_template, "GLATunnelNetwork");
        clear();
    }
}
