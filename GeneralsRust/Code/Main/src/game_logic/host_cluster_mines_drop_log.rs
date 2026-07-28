//! Frame-local ClusterMines drop + detonation logs for GW shadow parity.
//!
//! Under coupled dual-tick, GW sole-ticks transport/bomb flight and records
//! spawn/detonate intents here so host can create bombs and place minefields
//! without dual-ticking flight.

use super::{ObjectId, Team};
use glam::Vec3;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct ClusterMinesDropEvent {
    pub team: Team,
    pub target: Vec3,
    pub producer: ObjectId,
}

#[derive(Debug, Clone)]
pub struct ClusterMinesDetonateEvent {
    pub bomb: ObjectId,
    pub producer: Option<ObjectId>,
    pub team: Team,
    pub pos: Vec3,
}

thread_local! {
    static DROPS: RefCell<Vec<ClusterMinesDropEvent>> = RefCell::new(Vec::new());
    static DETS: RefCell<Vec<ClusterMinesDetonateEvent>> = RefCell::new(Vec::new());
}

pub fn record_drop(ev: ClusterMinesDropEvent) {
    DROPS.with(|l| l.borrow_mut().push(ev));
}

pub fn record_detonate(ev: ClusterMinesDetonateEvent) {
    DETS.with(|l| l.borrow_mut().push(ev));
}

pub fn drain_drops() -> Vec<ClusterMinesDropEvent> {
    DROPS.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

pub fn drain_dets() -> Vec<ClusterMinesDetonateEvent> {
    DETS.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

pub fn clear() {
    DROPS.with(|l| l.borrow_mut().clear());
    DETS.with(|l| l.borrow_mut().clear());
}
