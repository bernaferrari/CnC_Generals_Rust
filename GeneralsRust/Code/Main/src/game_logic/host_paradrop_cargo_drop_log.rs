//! Frame-local Paradrop cargo-plane drop + parachute ground logs for GW shadow parity.

use super::{ObjectId, Team};
use glam::Vec3;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct ParadropCargoDropEvent {
    pub team: Team,
    pub target: Vec3,
    pub producer: ObjectId,
}

#[derive(Debug, Clone)]
pub struct ParadropParachuteGroundEvent {
    pub id: ObjectId,
}

thread_local! {
    static DROPS: RefCell<Vec<ParadropCargoDropEvent>> = RefCell::new(Vec::new());
    static GROUND: RefCell<Vec<ParadropParachuteGroundEvent>> = RefCell::new(Vec::new());
}

pub fn record_drop(ev: ParadropCargoDropEvent) {
    DROPS.with(|l| l.borrow_mut().push(ev));
}
pub fn record_ground(ev: ParadropParachuteGroundEvent) {
    GROUND.with(|l| l.borrow_mut().push(ev));
}
pub fn drain_drops() -> Vec<ParadropCargoDropEvent> {
    DROPS.with(|l| std::mem::take(&mut *l.borrow_mut()))
}
pub fn drain_ground() -> Vec<ParadropParachuteGroundEvent> {
    GROUND.with(|l| std::mem::take(&mut *l.borrow_mut()))
}
pub fn clear() {
    DROPS.with(|l| l.borrow_mut().clear());
    GROUND.with(|l| l.borrow_mut().clear());
}
