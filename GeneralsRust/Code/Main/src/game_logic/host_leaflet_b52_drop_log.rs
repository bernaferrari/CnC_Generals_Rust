//! Frame-local Leaflet B52 drop + container ground logs for GW shadow parity.

use super::{ObjectId, Team};
use glam::Vec3;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct LeafletB52DropEvent {
    pub team: Team,
    pub target: Vec3,
    pub producer: ObjectId,
}

#[derive(Debug, Clone)]
pub struct LeafletContainerGroundEvent {
    pub id: ObjectId,
    pub pos: Vec3,
}

thread_local! {
    static DROPS: RefCell<Vec<LeafletB52DropEvent>> = RefCell::new(Vec::new());
    static GROUND: RefCell<Vec<LeafletContainerGroundEvent>> = RefCell::new(Vec::new());
}

pub fn record_drop(ev: LeafletB52DropEvent) { DROPS.with(|l| l.borrow_mut().push(ev)); }
pub fn record_ground(ev: LeafletContainerGroundEvent) { GROUND.with(|l| l.borrow_mut().push(ev)); }
pub fn drain_drops() -> Vec<LeafletB52DropEvent> { DROPS.with(|l| std::mem::take(&mut *l.borrow_mut())) }
pub fn drain_ground() -> Vec<LeafletContainerGroundEvent> { GROUND.with(|l| std::mem::take(&mut *l.borrow_mut())) }
pub fn clear() {
    DROPS.with(|l| l.borrow_mut().clear());
    GROUND.with(|l| l.borrow_mut().clear());
}
