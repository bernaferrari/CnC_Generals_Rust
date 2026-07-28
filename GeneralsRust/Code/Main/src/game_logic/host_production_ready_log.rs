//! Wave 614: GameWorld production-ready residual log.
//! Wave 735: ready events also carry GW spawn pose + rally so host sole-tick
//! complete/spawn applies GameWorld pose authority (not host-only recompute).
//! Wave 736: unit ready events may carry a pre-spawned GameWorld entity raw id;
//! host production spawn binds the new ObjectId to that entity (GW entity-first).
//!
//! Under PRODUCTION_AUTHORITY sole-tick, `writeback_production_to_host` records
//! producers whose queue head is finished (progress complete + exit delay clear).
//! Host `host_collect_production_completions` drains this log so GameWorld is the
//! sole authority for *which* producers are ready, *where* units exit, and (for
//! units) the shadow entity that already exists; host still try_complete +
//! allocates ObjectId on spawn, then binds rather than double-spawning.
//!
//! Fail-closed: empty drain is valid (no completions this frame).
//! `playable_claim` stays false.

use crate::game_logic::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct HostProductionReadyEvent {
    pub producer: ObjectId,
    pub template_name: String,
    pub is_upgrade: bool,
    /// Wave 735: GameWorld producer exit/spawn pose residual (`[x,y,z]`).
    pub spawn_pos: Option<[f32; 3]>,
    /// Wave 735: GameWorld rally point residual when set on the producer entity.
    pub rally: Option<[f32; 3]>,
    /// Wave 736: pre-spawned GameWorld entity raw id for unit completions.
    /// Host binds the allocated ObjectId to this entity (no second Spawn).
    pub gw_entity_raw: Option<u32>,
}

thread_local! {
    static LOG: RefCell<Vec<HostProductionReadyEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostProductionReadyEvent>> = RefCell::new(Vec::new());
    /// Wave 736: FIFO of GW entity raw ids to bind on the next host production spawns.
    static PENDING_BIND: RefCell<std::collections::VecDeque<u32>> = RefCell::new(std::collections::VecDeque::new());
}

/// Record a ready producer (no pose — tests / upgrade-only residual).
pub fn record(producer: ObjectId, template_name: impl Into<String>, is_upgrade: bool) {
    record_with_pose(producer, template_name, is_upgrade, None, None, None);
}

/// Wave 735/736: record ready producer with GameWorld spawn pose, rally, optional entity.
pub fn record_with_pose(
    producer: ObjectId,
    template_name: impl Into<String>,
    is_upgrade: bool,
    spawn_pos: Option<[f32; 3]>,
    rally: Option<[f32; 3]>,
    gw_entity_raw: Option<u32>,
) {
    LOG.with(|log| {
        log.borrow_mut().push(HostProductionReadyEvent {
            producer,
            template_name: template_name.into(),
            is_upgrade,
            spawn_pos,
            rally,
            gw_entity_raw,
        });
    });
}

pub fn drain() -> Vec<HostProductionReadyEvent> {
    LOG.with(|log| {
        let events = std::mem::take(&mut *log.borrow_mut());
        LAST_DRAIN.with(|last| *last.borrow_mut() = events.clone());
        events
    })
}

pub fn snapshot() -> Vec<HostProductionReadyEvent> {
    LOG.with(|log| log.borrow().clone())
}

pub fn take_last_drain() -> Vec<HostProductionReadyEvent> {
    LAST_DRAIN.with(|last| std::mem::take(&mut *last.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
    LAST_DRAIN.with(|last| last.borrow_mut().clear());
    PENDING_BIND.with(|q| q.borrow_mut().clear());
}

/// Wave 736: queue a pre-spawned GW entity for the next host production ObjectId bind.
pub fn push_pending_bind(gw_entity_raw: u32) {
    PENDING_BIND.with(|q| q.borrow_mut().push_back(gw_entity_raw));
}

/// Wave 736: take next pending GW entity bind for host production spawn.
pub fn pop_pending_bind() -> Option<u32> {
    PENDING_BIND.with(|q| q.borrow_mut().pop_front())
}

pub fn pending_bind_len() -> usize {
    PENDING_BIND.with(|q| q.borrow().len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_drain_roundtrip() {
        clear();
        record(ObjectId(7), "USA_Ranger", false);
        record(ObjectId(8), "Upgrade_Flashbang", true);
        let d = drain();
        assert_eq!(d.len(), 2);
        assert_eq!(d[0].producer.0, 7);
        assert!(d[0].spawn_pos.is_none());
        assert!(d[0].gw_entity_raw.is_none());
        assert!(d[1].is_upgrade);
        assert!(drain().is_empty());
        clear();
    }

    #[test]
    fn record_with_pose_carries_spawn_rally_and_entity() {
        clear();
        record_with_pose(
            ObjectId(3),
            "USA_Ranger",
            false,
            Some([10.0, 1.0, 20.0]),
            Some([40.0, 1.0, 50.0]),
            Some(99),
        );
        let d = drain();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].spawn_pos, Some([10.0, 1.0, 20.0]));
        assert_eq!(d[0].rally, Some([40.0, 1.0, 50.0]));
        assert_eq!(d[0].gw_entity_raw, Some(99));
        clear();
    }

    #[test]
    fn pending_bind_fifo() {
        clear();
        push_pending_bind(11);
        push_pending_bind(22);
        assert_eq!(pop_pending_bind(), Some(11));
        assert_eq!(pop_pending_bind(), Some(22));
        assert_eq!(pop_pending_bind(), None);
        clear();
    }
}
