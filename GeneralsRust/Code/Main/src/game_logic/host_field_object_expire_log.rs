//! Frame-local field/beam/shell lifetime expire logs for GW shadow parity.
//!
//! Covers NukeRadiationField / AnthraxToxinField / InfernoFireField object
//! residuals plus Wave 806 Spectre/flare/laser beams Wave 808 particle lasers, and Wave 809 firewall segment /
//! radar-van ping, and Wave 817 money-crate deletion lifetimes.

use super::{ObjectId, Team};
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldObjectKind {
    NukeRadiation,
    AnthraxToxin,
    InfernoFire,
    SpectreHowitzerShell,
    CountermeasureFlare,
    PointDefenseLaserBeam,
    WeaponLaserBeam,
    ParticleTrailRemnant,
    ParticleOrbitalLaser,
    ParticleConnectorLaser,
    FirewallSegment,
    RadarVanPing,
    MoneyCrate,
}

#[derive(Debug, Clone)]
pub struct FieldObjectExpireEvent {
    pub id: ObjectId,
    pub team: Option<Team>,
    pub kind: FieldObjectKind,
    /// Countermeasure flare producer for note_flare_expired residual.
    pub producer: Option<ObjectId>,
}

thread_local! {
    static EXPIRES: RefCell<Vec<FieldObjectExpireEvent>> = RefCell::new(Vec::new());
}

pub fn record(ev: FieldObjectExpireEvent) {
    EXPIRES.with(|l| l.borrow_mut().push(ev));
}

pub fn drain() -> Vec<FieldObjectExpireEvent> {
    EXPIRES.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

pub fn clear() {
    EXPIRES.with(|l| l.borrow_mut().clear());
}
