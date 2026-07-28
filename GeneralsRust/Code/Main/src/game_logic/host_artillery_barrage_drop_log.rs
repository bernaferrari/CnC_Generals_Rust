//! Frame-local ArtilleryBarrage drop + detonation logs for GW shadow parity.

use super::{ObjectId, Team};
use glam::Vec3;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct ArtilleryDropEvent {
    pub team: Team,
    pub target: Vec3,
    pub producer: ObjectId,
}

#[derive(Debug, Clone)]
pub struct ArtilleryDetonateEvent {
    pub shell: ObjectId,
    pub producer: Option<ObjectId>,
    pub team: Team,
    pub pos: Vec3,
}

thread_local! {
    static DROPS: RefCell<Vec<ArtilleryDropEvent>> = RefCell::new(Vec::new());
    static DETS: RefCell<Vec<ArtilleryDetonateEvent>> = RefCell::new(Vec::new());
}

pub fn record_drop(ev: ArtilleryDropEvent) {
    DROPS.with(|l| l.borrow_mut().push(ev));
}
pub fn record_detonate(ev: ArtilleryDetonateEvent) {
    DETS.with(|l| l.borrow_mut().push(ev));
}
pub fn drain_drops() -> Vec<ArtilleryDropEvent> {
    DROPS.with(|l| std::mem::take(&mut *l.borrow_mut()))
}
pub fn drain_dets() -> Vec<ArtilleryDetonateEvent> {
    DETS.with(|l| std::mem::take(&mut *l.borrow_mut()))
}
pub fn clear() {
    DROPS.with(|l| l.borrow_mut().clear());
    DETS.with(|l| l.borrow_mut().clear());
}
