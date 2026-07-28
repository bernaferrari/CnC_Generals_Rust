//! Frame-local CarpetBomb drop + detonation logs for GW shadow parity.

use super::{ObjectId, Team};
use glam::Vec3;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct CarpetBombDropEvent {
    pub team: Team,
    pub target: Vec3,
    pub producer: ObjectId,
}

#[derive(Debug, Clone)]
pub struct CarpetBombDetonateEvent {
    pub bomb: ObjectId,
    pub producer: Option<ObjectId>,
    pub team: Team,
    pub pos: Vec3,
}

thread_local! {
    static DROPS: RefCell<Vec<CarpetBombDropEvent>> = RefCell::new(Vec::new());
    static DETS: RefCell<Vec<CarpetBombDetonateEvent>> = RefCell::new(Vec::new());
}

pub fn record_drop(ev: CarpetBombDropEvent) {
    DROPS.with(|l| l.borrow_mut().push(ev));
}
pub fn record_detonate(ev: CarpetBombDetonateEvent) {
    DETS.with(|l| l.borrow_mut().push(ev));
}
pub fn drain_drops() -> Vec<CarpetBombDropEvent> {
    DROPS.with(|l| std::mem::take(&mut *l.borrow_mut()))
}
pub fn drain_dets() -> Vec<CarpetBombDetonateEvent> {
    DETS.with(|l| std::mem::take(&mut *l.borrow_mut()))
}
pub fn clear() {
    DROPS.with(|l| l.borrow_mut().clear());
    DETS.with(|l| l.borrow_mut().clear());
}
