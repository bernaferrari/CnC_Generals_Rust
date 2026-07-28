//! Frame-local EMP Pulse drop + detonation logs for GW shadow parity.
//!
//! Under coupled dual-tick, GW sole-ticks transport/bomb/spheroid residual and
//! records spawn/detonate/expire intents so host applies without dual flight.

use super::{ObjectId, Team};
use glam::Vec3;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct EmpPulseDropEvent {
    pub team: Team,
    pub target: Vec3,
    pub producer: ObjectId,
    pub player_id: u32,
    pub caster_id: u32,
}

#[derive(Debug, Clone)]
pub struct EmpPulseDetonateEvent {
    pub bomb: ObjectId,
    pub producer: Option<ObjectId>,
    pub team: Team,
    pub pos: Vec3,
}

#[derive(Debug, Clone)]
pub struct EmpPulseSpheroidExpireEvent {
    pub id: ObjectId,
}

thread_local! {
    static DROPS: RefCell<Vec<EmpPulseDropEvent>> = RefCell::new(Vec::new());
    static DETS: RefCell<Vec<EmpPulseDetonateEvent>> = RefCell::new(Vec::new());
    static SPH: RefCell<Vec<EmpPulseSpheroidExpireEvent>> = RefCell::new(Vec::new());
}

pub fn record_drop(ev: EmpPulseDropEvent) {
    DROPS.with(|l| l.borrow_mut().push(ev));
}

pub fn record_detonate(ev: EmpPulseDetonateEvent) {
    DETS.with(|l| l.borrow_mut().push(ev));
}

pub fn record_spheroid_expire(ev: EmpPulseSpheroidExpireEvent) {
    SPH.with(|l| l.borrow_mut().push(ev));
}

pub fn drain_drops() -> Vec<EmpPulseDropEvent> {
    DROPS.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

pub fn drain_dets() -> Vec<EmpPulseDetonateEvent> {
    DETS.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

pub fn drain_spheroid_expires() -> Vec<EmpPulseSpheroidExpireEvent> {
    SPH.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

pub fn clear() {
    DROPS.with(|l| l.borrow_mut().clear());
    DETS.with(|l| l.borrow_mut().clear());
    SPH.with(|l| l.borrow_mut().clear());
}
