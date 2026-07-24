//! Host ObjectCreationList ApplyRandomForce nugget residual.
//!
//! C++: `ApplyRandomForceNugget::create(primary)` applies a random force + spin
//! rates to the primary object's PhysicsBehavior (used before CreateDebris on
//! air deaths — e.g. `OCL_TechnicalAirDeathStart`).
//!
//! Residual playability slice:
//! - Min/Max force magnitude + pitch → impulse on movement.velocity
//! - SpinRate → orientation nudge residual
//! - Template/OCL peels for common air-death OCLs
//!
//! Fail-closed: not full PhysicsBehavior yaw/roll/pitch rate matrix /
//! GameLogicRandomValueReal stream (uses deterministic index salt).

use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Retail OCL_TechnicalAirDeathStart residual peels.
pub const TECHNICAL_AIR_DEATH_MIN_FORCE: f32 = 60.0;
pub const TECHNICAL_AIR_DEATH_MAX_FORCE: f32 = 100.0;
pub const TECHNICAL_AIR_DEATH_MIN_PITCH_DEG: f32 = 70.0;
pub const TECHNICAL_AIR_DEATH_MAX_PITCH_DEG: f32 = 90.0;
pub const TECHNICAL_AIR_DEATH_SPIN_RATE_DEG: f32 = 120.0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HostOclApplyRandomForcePlan {
    pub ocl_name: String,
    pub min_force: f32,
    pub max_force: f32,
    pub min_pitch_deg: f32,
    pub max_pitch_deg: f32,
    pub spin_rate_deg: f32,
}

impl HostOclApplyRandomForcePlan {
    pub fn technical_air_death() -> Self {
        Self {
            ocl_name: "OCL_TechnicalAirDeathStart".into(),
            min_force: TECHNICAL_AIR_DEATH_MIN_FORCE,
            max_force: TECHNICAL_AIR_DEATH_MAX_FORCE,
            min_pitch_deg: TECHNICAL_AIR_DEATH_MIN_PITCH_DEG,
            max_pitch_deg: TECHNICAL_AIR_DEATH_MAX_PITCH_DEG,
            spin_rate_deg: TECHNICAL_AIR_DEATH_SPIN_RATE_DEG,
        }
    }

    pub fn generic_air_death(ocl: &str) -> Self {
        Self {
            ocl_name: ocl.into(),
            min_force: 40.0,
            max_force: 80.0,
            min_pitch_deg: 60.0,
            max_pitch_deg: 90.0,
            spin_rate_deg: 90.0,
        }
    }
}

/// Peel plan from dying template / OCL name.
pub fn apply_random_force_plan_for(name: &str) -> Option<HostOclApplyRandomForcePlan> {
    let n = name.to_ascii_lowercase();
    if n.contains("technical") && (n.contains("airdeath") || n.contains("air_death") || n.contains("death")) {
        return Some(HostOclApplyRandomForcePlan::technical_air_death());
    }
    if n.contains("airdeath") || n.contains("air_death") {
        return Some(HostOclApplyRandomForcePlan::generic_air_death(name));
    }
    // Humvee / vehicle air death OCLs residual.
    if n.contains("humvee") && n.contains("death") {
        return Some(HostOclApplyRandomForcePlan::generic_air_death("OCL_HumveeAirDeathStart"));
    }
    if n.contains("quadcannon") && n.contains("death") {
        return Some(HostOclApplyRandomForcePlan::generic_air_death(
            "OCL_QuadCannonAirDeathStart",
        ));
    }
    None
}

/// Deterministic force residual (mass-independent impulse units).
pub fn calc_random_force(plan: &HostOclApplyRandomForcePlan, salt: u32) -> Vec3 {
    let t = ((salt.wrapping_mul(2654435761)) & 0xffff) as f32 / 65535.0;
    let mag = plan.min_force + (plan.max_force - plan.min_force) * t;
    let tp = ((salt.wrapping_mul(2246822519)) & 0xffff) as f32 / 65535.0;
    let pitch = (plan.min_pitch_deg + (plan.max_pitch_deg - plan.min_pitch_deg) * tp).to_radians();
    let yaw = (salt as f32 * 1.618_033_9) % std::f32::consts::TAU;
    let horiz = mag * pitch.cos();
    let vert = mag * pitch.sin();
    Vec3::new(horiz * yaw.cos(), vert, horiz * yaw.sin())
}

/// Spin residual rad applied once to orientation.
pub fn spin_nudge_rad(plan: &HostOclApplyRandomForcePlan, salt: u32) -> f32 {
    let t = ((salt.wrapping_mul(3266489917)) & 0xffff) as f32 / 65535.0;
    let signed = t * 2.0 - 1.0;
    signed * plan.spin_rate_deg.to_radians()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostOclApplyRandomForceRegistry {
    pub applied: u32,
    pub last_force_mag: f32,
}

impl HostOclApplyRandomForceRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn record(&mut self, force: Vec3) {
        self.applied = self.applied.saturating_add(1);
        self.last_force_mag = force.length();
    }
    pub fn honesty_host_path_ok(&self) -> bool {
        self.applied > 0
    }
}

pub fn honesty_ocl_apply_random_force_residual_ok() -> bool {
    let p = HostOclApplyRandomForcePlan::technical_air_death();
    (p.min_force - 60.0).abs() < 0.1
        && (p.max_force - 100.0).abs() < 0.1
        && apply_random_force_plan_for("OCL_TechnicalAirDeathStart").is_some()
        && {
            let f = calc_random_force(&p, 7);
            f.length() >= 60.0 - 0.1 && f.y > 0.0
        }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn technical_force_upward() {
        assert!(honesty_ocl_apply_random_force_residual_ok());
        let p = HostOclApplyRandomForcePlan::technical_air_death();
        let f = calc_random_force(&p, 1);
        assert!(f.y > 50.0, "pitch 70-90 should be mostly upward, y={}", f.y);
    }
}
