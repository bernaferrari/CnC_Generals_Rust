//! Frame-local AnthraxBomb drop + detonation logs for GW shadow parity.
//!
//! Under coupled dual-tick, GW sole-ticks transport/bomb flight and records
//! spawn/detonate intents here so host can create bombs and apply damage
//! without dual-ticking flight.

use super::{ObjectId, Team};
use crate::game_logic::host_anthrax_bomb_flight::AnthraxBombPayloadTier;
use glam::Vec3;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct AnthraxDropEvent {
    pub team: Team,
    pub target: Vec3,
    /// C++ contained payload exits at the transport pose, not over the click.
    pub plane_pos: Vec3,
    pub producer: ObjectId,
    pub tier: AnthraxBombPayloadTier,
}

#[derive(Debug, Clone)]
pub struct AnthraxDetonateEvent {
    pub bomb: ObjectId,
    pub producer: Option<ObjectId>,
    pub team: Team,
    pub pos: Vec3,
}

thread_local! {
    static DROPS: RefCell<Vec<AnthraxDropEvent>> = RefCell::new(Vec::new());
    static DETS: RefCell<Vec<AnthraxDetonateEvent>> = RefCell::new(Vec::new());
}

pub fn record_drop(ev: AnthraxDropEvent) {
    DROPS.with(|l| l.borrow_mut().push(ev));
}

pub fn record_detonate(ev: AnthraxDetonateEvent) {
    DETS.with(|l| l.borrow_mut().push(ev));
}

pub fn drain_drops() -> Vec<AnthraxDropEvent> {
    DROPS.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

pub fn drain_dets() -> Vec<AnthraxDetonateEvent> {
    DETS.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

pub fn clear() {
    DROPS.with(|l| l.borrow_mut().clear());
    DETS.with(|l| l.borrow_mut().clear());
}
