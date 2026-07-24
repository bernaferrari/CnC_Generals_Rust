//! Host ObjectCreationList CreateDebris residual (disposition force matrix).
//!
//! C++: `GenericObjectCreationNugget` CreateDebris path applies DebrisDisposition
//! bits (LIKE_EXISTING, ON_GROUND_ALIGNED, SEND_IT_FLYING/UP/OUT, RANDOM_FORCE,
//! FLOATING, INHERIT_VELOCITY, WHIRLING) then physics force/spin.
//!
//! Residual playability slice:
//! - Disposition flag peel + name table (shared with Wave 107 honesty constants)
//! - Force magnitude / pitch → initial velocity residual
//! - Count + offset spawn plan
//! - Wire from CreateObjectDie GenericDebris peels
//!
//! Fail-closed: not full particle system attach / mass→friction matrix / W3D
//! debris model LOD / full PhysicsBehavior whirling torque.

use glam::Vec3;
use serde::{Deserialize, Serialize};

// Re-export disposition bits from wave107 pack for single source of truth.
pub use crate::game_logic::host_fx_ocl_particle_audio_residual_wave107::{
    DEBRIS_DISPOSITION_NAMES_RESIDUAL, DEBRIS_FLOATING, DEBRIS_INHERIT_VELOCITY,
    DEBRIS_LIKE_EXISTING, DEBRIS_ON_GROUND_ALIGNED, DEBRIS_RANDOM_FORCE, DEBRIS_SEND_IT_FLYING,
    DEBRIS_SEND_IT_OUT, DEBRIS_SEND_IT_UP, DEBRIS_WHIRLING,
};

/// Retail OCL_CreateDamagedBarrel peel residual.
pub const DAMAGED_BARREL_DEBRIS_MODEL: &str = "PMBarrel01_D1";
pub const DAMAGED_BARREL_MIN_FORCE: f32 = 5.0;
pub const DAMAGED_BARREL_MAX_FORCE: f32 = 7.0;
pub const DAMAGED_BARREL_SPIN_RATE: f32 = 180.0;
pub const DAMAGED_BARREL_MIN_PITCH_DEG: f32 = 75.0;
pub const DAMAGED_BARREL_MAX_PITCH_DEG: f32 = 90.0;

/// Generic tank debris residual peel (CreateObjectDie common path).
pub const GENERIC_TANK_DEBRIS_TEMPLATE: &str = "GenericDebris";
pub const GENERIC_TANK_DEBRIS_MIN_FORCE: f32 = 8.0;
pub const GENERIC_TANK_DEBRIS_MAX_FORCE: f32 = 14.0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HostOclCreateDebrisPlan {
    pub model_or_template: String,
    pub count: u32,
    pub disposition: u32,
    pub offset: Vec3,
    pub mass: f32,
    pub min_force: f32,
    pub max_force: f32,
    pub spin_rate_deg: f32,
    pub min_pitch_deg: f32,
    pub max_pitch_deg: f32,
}

impl HostOclCreateDebrisPlan {
    pub fn damaged_barrel() -> Self {
        Self {
            model_or_template: DAMAGED_BARREL_DEBRIS_MODEL.into(),
            count: 1,
            disposition: DEBRIS_RANDOM_FORCE,
            offset: Vec3::ZERO,
            mass: 2.0,
            min_force: DAMAGED_BARREL_MIN_FORCE,
            max_force: DAMAGED_BARREL_MAX_FORCE,
            spin_rate_deg: DAMAGED_BARREL_SPIN_RATE,
            min_pitch_deg: DAMAGED_BARREL_MIN_PITCH_DEG,
            max_pitch_deg: DAMAGED_BARREL_MAX_PITCH_DEG,
        }
    }

    pub fn generic_tank_debris() -> Self {
        Self {
            model_or_template: GENERIC_TANK_DEBRIS_TEMPLATE.into(),
            count: 3,
            disposition: DEBRIS_SEND_IT_FLYING | DEBRIS_RANDOM_FORCE | DEBRIS_INHERIT_VELOCITY,
            offset: Vec3::ZERO,
            mass: 5.0,
            min_force: GENERIC_TANK_DEBRIS_MIN_FORCE,
            max_force: GENERIC_TANK_DEBRIS_MAX_FORCE,
            spin_rate_deg: 120.0,
            min_pitch_deg: 45.0,
            max_pitch_deg: 80.0,
        }
    }
}

/// Parse disposition name list ("A B C") into bit flags.
pub fn parse_disposition_names(names: &str) -> u32 {
    let mut bits = 0u32;
    for tok in names.split(|c: char| c.is_whitespace() || c == '|' || c == ',') {
        let t = tok.trim();
        if t.is_empty() {
            continue;
        }
        let u = t.to_ascii_uppercase();
        if let Some(i) = DEBRIS_DISPOSITION_NAMES_RESIDUAL
            .iter()
            .position(|n| n.eq_ignore_ascii_case(&u))
        {
            bits |= 1u32 << i;
        }
    }
    bits
}

/// Deterministic residual force magnitude from min/max + index salt.
pub fn force_magnitude_for_index(min_f: f32, max_f: f32, index: u32) -> f32 {
    if max_f <= min_f {
        return min_f.max(0.0);
    }
    let t = ((index.wrapping_mul(2654435761)) & 0xffff) as f32 / 65535.0;
    min_f + (max_f - min_f) * t
}

/// Deterministic pitch degrees residual.
pub fn pitch_deg_for_index(min_p: f32, max_p: f32, index: u32) -> f32 {
    if max_p <= min_p {
        return min_p;
    }
    let t = ((index.wrapping_mul(2246822519)) & 0xffff) as f32 / 65535.0;
    min_p + (max_p - min_p) * t
}

/// Build initial velocity residual from disposition + force peels.
/// Host Y-up: pitch from horizontal (90 = straight up).
pub fn debris_initial_velocity(
    disposition: u32,
    inherit_vel: Vec3,
    index: u32,
    min_force: f32,
    max_force: f32,
    min_pitch_deg: f32,
    max_pitch_deg: f32,
) -> Vec3 {
    let mut vel = Vec3::ZERO;
    if disposition & DEBRIS_INHERIT_VELOCITY != 0 {
        vel += inherit_vel;
    }

    let mag = force_magnitude_for_index(min_force, max_force, index);
    let pitch = pitch_deg_for_index(min_pitch_deg, max_pitch_deg, index).to_radians();

    // Azimuth residual from index (full circle).
    let yaw = (index as f32 * 2.399_963) % std::f32::consts::TAU;

    if disposition & DEBRIS_SEND_IT_UP != 0 {
        vel.y += mag;
    }
    if disposition & (DEBRIS_SEND_IT_FLYING | DEBRIS_RANDOM_FORCE | DEBRIS_SEND_IT_OUT) != 0 {
        let horiz = mag * pitch.cos();
        let vert = mag * pitch.sin();
        vel.x += horiz * yaw.cos();
        vel.z += horiz * yaw.sin();
        vel.y += vert;
    } else if disposition & DEBRIS_FLOATING != 0 {
        vel.y += mag * 0.15;
    }

    // LIKE_EXISTING / ON_GROUND_ALIGNED: no extra force (pose only).
    vel
}

/// Spin rate rad/frame residual from deg/sec peel @ 30 FPS.
pub fn spin_rate_rad_per_frame(spin_rate_deg: f32) -> f32 {
    spin_rate_deg.to_radians() / 30.0
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostOclCreateDebrisRegistry {
    pub plans: u32,
    pub debris_spawned: u32,
    pub flying_forces: u32,
    pub last_disposition: u32,
}

impl HostOclCreateDebrisRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn record_plan(&mut self, disposition: u32) {
        self.plans = self.plans.saturating_add(1);
        self.last_disposition = disposition;
    }
    pub fn record_spawn(&mut self, disposition: u32) {
        self.debris_spawned = self.debris_spawned.saturating_add(1);
        if disposition
            & (DEBRIS_SEND_IT_FLYING | DEBRIS_SEND_IT_UP | DEBRIS_SEND_IT_OUT | DEBRIS_RANDOM_FORCE)
            != 0
        {
            self.flying_forces = self.flying_forces.saturating_add(1);
        }
    }
    pub fn honesty_host_path_ok(&self) -> bool {
        self.plans > 0 || self.debris_spawned > 0
    }
}

pub fn honesty_ocl_create_debris_residual_ok() -> bool {
    parse_disposition_names("RANDOM_FORCE") == DEBRIS_RANDOM_FORCE
        && parse_disposition_names("SEND_IT_FLYING INHERIT_VELOCITY")
            == (DEBRIS_SEND_IT_FLYING | DEBRIS_INHERIT_VELOCITY)
        && {
            let v = debris_initial_velocity(
                DEBRIS_SEND_IT_UP,
                Vec3::ZERO,
                0,
                10.0,
                10.0,
                90.0,
                90.0,
            );
            v.y >= 9.0
        }
        && {
            let p = HostOclCreateDebrisPlan::damaged_barrel();
            p.disposition == DEBRIS_RANDOM_FORCE && (p.min_force - 5.0).abs() < 1e-5
        }
        && spin_rate_rad_per_frame(180.0) > 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disposition_parse_and_force() {
        assert!(honesty_ocl_create_debris_residual_ok());
        let v = debris_initial_velocity(
            DEBRIS_SEND_IT_FLYING | DEBRIS_RANDOM_FORCE,
            Vec3::new(1.0, 0.0, 0.0),
            3,
            5.0,
            7.0,
            75.0,
            90.0,
        );
        assert!(v.length() > 1.0);
        let tank = HostOclCreateDebrisPlan::generic_tank_debris();
        assert_eq!(tank.count, 3);
        assert!(tank.disposition & DEBRIS_SEND_IT_FLYING != 0);
    }
}
