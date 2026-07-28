//! Frame-local DaisyCutter/MOAB drop + detonation logs for GW shadow parity.
//!
//! Under coupled dual-tick, GW sole-ticks transport/bomb flight and records
//! spawn/detonate intents here so host can create bombs and apply damage
//! without dual-ticking flight.

use super::{ObjectId, Team};
use crate::game_logic::host_daisy_cutter_flight::DaisyFlightPayloadTier;
use glam::Vec3;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct DaisyDropEvent {
    pub team: Team,
    pub target: Vec3,
    pub producer: ObjectId,
    pub tier: DaisyFlightPayloadTier,
}

#[derive(Debug, Clone)]
pub struct DaisyDetonateEvent {
    pub bomb: ObjectId,
    pub producer: Option<ObjectId>,
    pub team: Team,
    pub pos: Vec3,
    pub tier: DaisyFlightPayloadTier,
}

thread_local! {
    static DROPS: RefCell<Vec<DaisyDropEvent>> = RefCell::new(Vec::new());
    static DETS: RefCell<Vec<DaisyDetonateEvent>> = RefCell::new(Vec::new());
}

pub fn record_drop(ev: DaisyDropEvent) {
    DROPS.with(|l| l.borrow_mut().push(ev));
}

pub fn record_detonate(ev: DaisyDetonateEvent) {
    DETS.with(|l| l.borrow_mut().push(ev));
}

pub fn drain_drops() -> Vec<DaisyDropEvent> {
    DROPS.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

pub fn drain_dets() -> Vec<DaisyDetonateEvent> {
    DETS.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

pub fn clear() {
    DROPS.with(|l| l.borrow_mut().clear());
    DETS.with(|l| l.borrow_mut().clear());
}
