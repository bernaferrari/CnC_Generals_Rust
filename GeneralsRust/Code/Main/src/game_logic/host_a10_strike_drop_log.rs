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
    /// World spawn at jet WeaponA bone (not y=90 above impact).
    pub spawn: Vec3,
    pub producer: ObjectId,
}

#[derive(Debug, Clone)]
pub struct A10DetonateEvent {
    pub missile: ObjectId,
    pub producer: Option<ObjectId>,
    pub team: Team,
    pub pos: Vec3,
}

#[derive(Debug, Clone)]
pub struct A10VulcanEvent {
    pub jet: ObjectId,
    pub producer: Option<ObjectId>,
    pub team: Team,
    pub pos: Vec3,
}

#[derive(Debug, Clone)]
pub struct A10DiveStartEvent {
    pub jet: ObjectId,
    pub pos: Vec3,
}

thread_local! {
    static DROPS: RefCell<Vec<A10DropEvent>> = RefCell::new(Vec::new());
    static DETS: RefCell<Vec<A10DetonateEvent>> = RefCell::new(Vec::new());
    static VULCANS: RefCell<Vec<A10VulcanEvent>> = RefCell::new(Vec::new());
    static DIVE_STARTS: RefCell<Vec<A10DiveStartEvent>> = RefCell::new(Vec::new());
}

pub fn record_drop(ev: A10DropEvent) {
    DROPS.with(|l| l.borrow_mut().push(ev));
}

pub fn record_detonate(ev: A10DetonateEvent) {
    DETS.with(|l| l.borrow_mut().push(ev));
}

pub fn record_vulcan(ev: A10VulcanEvent) {
    VULCANS.with(|l| l.borrow_mut().push(ev));
}

pub fn record_dive_start(ev: A10DiveStartEvent) {
    DIVE_STARTS.with(|l| l.borrow_mut().push(ev));
}

pub fn drain_drops() -> Vec<A10DropEvent> {
    DROPS.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

pub fn drain_dets() -> Vec<A10DetonateEvent> {
    DETS.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

pub fn drain_vulcans() -> Vec<A10VulcanEvent> {
    VULCANS.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

pub fn drain_dive_starts() -> Vec<A10DiveStartEvent> {
    DIVE_STARTS.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

pub fn clear() {
    DROPS.with(|l| l.borrow_mut().clear());
    DETS.with(|l| l.borrow_mut().clear());
    VULCANS.with(|l| l.borrow_mut().clear());
    DIVE_STARTS.with(|l| l.borrow_mut().clear());
}

/// Residual A10ThunderboltMissileWeapon flight speed (wu / logic frame).
pub const A10_MISSILE_SPEED_PER_FRAME: f32 = 24.0;

/// Residual WeaponA01..06 pylon offsets in host Y-up model space.
pub fn a10_weapon_a_local(slot_1based: u32) -> Vec3 {
    let i = slot_1based.saturating_sub(1);
    let side = if i % 2 == 0 { -8.0 } else { 8.0 };
    let along = (i / 2) as f32 * 6.0 - 6.0;
    Vec3::new(side, -4.0, along)
}

/// World position of VisibleDropBone `WeaponA` slot (1-based).
pub fn a10_weapon_a_world_pos(jet_pos: Vec3, yaw: f32, slot_1based: u32) -> Vec3 {
    let local = a10_weapon_a_local(slot_1based);
    let (sin, cos) = yaw.sin_cos();
    Vec3::new(
        jet_pos.x + local.x * cos - local.z * sin,
        jet_pos.y + local.y,
        jet_pos.z + local.x * sin + local.z * cos,
    )
}

/// C++ `projectileFireAtObjectOrPosition(NULL, targetPos)` residual velocity.
pub fn a10_missile_fire_velocity(from: Vec3, target: Vec3, inherit: Vec3) -> Vec3 {
    let mut aim = target;
    aim.y = 0.0;
    let to = aim - from;
    let dir = to.normalize_or_zero();
    let dir = if dir.length_squared() < 1e-8 {
        Vec3::new(0.0, -1.0, 0.0)
    } else {
        dir
    };
    let mut v = dir * A10_MISSILE_SPEED_PER_FRAME + inherit;
    // The warhead aims at the ground plane (aim.y = 0); a climbing jet must
    // not drag it upward past the surface (C++ projectile status always
    // descends to the aim position — HeightDie on ground contact).
    v.y = v.y.min(0.0);
    v
}
