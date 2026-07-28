//! Frame-local A10 strike drop + detonation logs for GW shadow parity.
//!
//! Under coupled dual-tick, GW sole-ticks jet transport, pending drops, and
//! missile fall; host applies create_object / damage without dual flight.

use super::{ObjectId, Team};
use glam::Vec3;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct A10DropEvent {
    pub team: Team,
    pub target: Vec3,
    pub producer: ObjectId,
}

#[derive(Debug, Clone)]
pub struct A10DetonateEvent {
    pub missile: ObjectId,
    pub producer: Option<ObjectId>,
    pub team: Team,
    pub pos: Vec3,
}

thread_local! {
    static DROPS: RefCell<Vec<A10DropEvent>> = RefCell::new(Vec::new());
    static DETS: RefCell<Vec<A10DetonateEvent>> = RefCell::new(Vec::new());
}

pub fn record_drop(ev: A10DropEvent) {
    DROPS.with(|l| l.borrow_mut().push(ev));
}

pub fn record_detonate(ev: A10DetonateEvent) {
    DETS.with(|l| l.borrow_mut().push(ev));
}

pub fn drain_drops() -> Vec<A10DropEvent> {
    DROPS.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

pub fn drain_dets() -> Vec<A10DetonateEvent> {
    DETS.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

pub fn clear() {
    DROPS.with(|l| l.borrow_mut().clear());
    DETS.with(|l| l.borrow_mut().clear());
}
