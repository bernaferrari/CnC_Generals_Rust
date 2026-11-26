//! Object creation debris plans, disposition flags, and per-world LOD state.

use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Debris disposition flags - how debris should behave when spawned
/// Matches C++ enum DebrisDisposition (ObjectCreationList.cpp:673-684)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DebrisDisposition(u32);

impl DebrisDisposition {
    pub const LIKE_EXISTING: u32 = 0x00000001; // Use same orientation as source
    pub const ON_GROUND_ALIGNED: u32 = 0x00000002; // Place on ground, random orientation
    pub const SEND_IT_FLYING: u32 = 0x00000004; // Launch with random forces
    pub const SEND_IT_UP: u32 = 0x00000008; // Launch upward
    pub const SEND_IT_OUT: u32 = 0x00000010; // Push outward horizontally
    pub const RANDOM_FORCE: u32 = 0x00000020; // Apply custom random force
    pub const FLOATING: u32 = 0x00000040; // Enable floating (water)
    pub const INHERIT_VELOCITY: u32 = 0x00000080; // Inherit source object velocity
    pub const WHIRLING: u32 = 0x00000100; // Random spin rates

    pub fn new(flags: u32) -> Self {
        Self(flags)
    }

    pub fn has(&self, flag: u32) -> bool {
        (self.0 & flag) != 0
    }

    pub fn set(&mut self, flag: u32) {
        self.0 |= flag;
    }
}

/// Fallback for the retail OCL_CreateDamagedBarrel entry.
pub const DAMAGED_BARREL_DEBRIS_MODEL: &str = "PMBarrel01_D1";
pub const DAMAGED_BARREL_MIN_FORCE: f32 = 5.0;
pub const DAMAGED_BARREL_MAX_FORCE: f32 = 7.0;
pub const DAMAGED_BARREL_SPIN_RATE: f32 = 180.0;
pub const DAMAGED_BARREL_MIN_PITCH_DEG: f32 = 75.0;
pub const DAMAGED_BARREL_MAX_PITCH_DEG: f32 = 90.0;

/// Fallback for GenericDebris on the CreateObjectDie path.
pub const GENERIC_TANK_DEBRIS_TEMPLATE: &str = "GenericDebris";
pub const GENERIC_TANK_DEBRIS_MIN_FORCE: f32 = 8.0;
pub const GENERIC_TANK_DEBRIS_MAX_FORCE: f32 = 14.0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DebrisPlan {
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
    /// C++ ExtraFriction (already per-frame).
    pub extra_friction: f32,
    /// C++ ExtraBounciness (stamped; C++ stores as ground-stiffness modifier).
    pub extra_bounciness: f32,
    /// C++ BounceSound AudioEventRTS name. Empty = silent land.
    pub bounce_sound: String,
}

impl DebrisPlan {
    pub fn damaged_barrel() -> Self {
        Self {
            model_or_template: DAMAGED_BARREL_DEBRIS_MODEL.into(),
            count: 1,
            disposition: DebrisDisposition::RANDOM_FORCE,
            offset: Vec3::ZERO,
            mass: 2.0,
            min_force: DAMAGED_BARREL_MIN_FORCE,
            max_force: DAMAGED_BARREL_MAX_FORCE,
            spin_rate_deg: DAMAGED_BARREL_SPIN_RATE,
            min_pitch_deg: DAMAGED_BARREL_MIN_PITCH_DEG,
            max_pitch_deg: DAMAGED_BARREL_MAX_PITCH_DEG,
            extra_friction: 0.0,
            extra_bounciness: 0.0,
            bounce_sound: String::new(),
        }
    }

    pub fn generic_tank_debris() -> Self {
        Self {
            model_or_template: GENERIC_TANK_DEBRIS_TEMPLATE.into(),
            count: 3,
            disposition: DebrisDisposition::SEND_IT_FLYING
                | DebrisDisposition::RANDOM_FORCE
                | DebrisDisposition::INHERIT_VELOCITY,
            offset: Vec3::ZERO,
            mass: 5.0,
            min_force: GENERIC_TANK_DEBRIS_MIN_FORCE,
            max_force: GENERIC_TANK_DEBRIS_MAX_FORCE,
            spin_rate_deg: 120.0,
            min_pitch_deg: 45.0,
            max_pitch_deg: 80.0,
            extra_friction: 0.0,
            extra_bounciness: 0.0,
            bounce_sound: String::new(),
        }
    }
}

/// C++ ObjectCreationList.cpp:1101 SEND_IT_FLYING|SEND_IT_UP|RANDOM_FORCE
/// is the only path that setAllowBouncing(true).
pub fn disposition_enables_bounce(disposition: u32) -> bool {
    disposition
        & (DebrisDisposition::SEND_IT_FLYING
            | DebrisDisposition::SEND_IT_UP
            | DebrisDisposition::RANDOM_FORCE)
        != 0
}

/// Deterministic force magnitude for an item in a fallback debris plan.
pub fn force_magnitude_for_index(min_f: f32, max_f: f32, index: u32) -> f32 {
    if max_f <= min_f {
        return min_f.max(0.0);
    }
    let t = ((index.wrapping_mul(2654435761)) & 0xffff) as f32 / 65535.0;
    min_f + (max_f - min_f) * t
}

/// Deterministic pitch in degrees for an item in a fallback debris plan.
pub fn pitch_deg_for_index(min_p: f32, max_p: f32, index: u32) -> f32 {
    if max_p <= min_p {
        return min_p;
    }
    let t = ((index.wrapping_mul(2246822519)) & 0xffff) as f32 / 65535.0;
    min_p + (max_p - min_p) * t
}

/// Resolve the initial velocity for a fallback debris plan.
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
    if disposition & DebrisDisposition::INHERIT_VELOCITY != 0 {
        vel += inherit_vel;
    }

    let mag = force_magnitude_for_index(min_force, max_force, index);
    let pitch = pitch_deg_for_index(min_pitch_deg, max_pitch_deg, index).to_radians();

    // Spread items around a full circle by index.
    let yaw = (index as f32 * 2.399_963) % std::f32::consts::TAU;

    if disposition & DebrisDisposition::SEND_IT_UP != 0 {
        vel.y += mag;
    }
    if disposition
        & (DebrisDisposition::SEND_IT_FLYING
            | DebrisDisposition::RANDOM_FORCE
            | DebrisDisposition::SEND_IT_OUT)
        != 0
    {
        let horiz = mag * pitch.cos();
        let vert = mag * pitch.sin();
        vel.x += horiz * yaw.cos();
        vel.z += horiz * yaw.sin();
        vel.y += vert;
    } else if disposition & DebrisDisposition::FLOATING != 0 {
        vel.y += mag * 0.15;
    }

    // LIKE_EXISTING / ON_GROUND_ALIGNED: no extra force (pose only).
    vel
}

/// Convert spin from degrees per second to radians per logic frame.
pub fn spin_rate_rad_per_frame(spin_rate_deg: f32) -> f32 {
    spin_rate_deg.to_radians() / crate::time::LOGIC_FRAMES_PER_SECOND
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DebrisLodState {
    /// Receiving-session copy of GameLODManager's dynamic debris mask.
    /// The generation counter is session-owned and advances once per parsed
    /// CreateDebris item, after its authored model selection.
    #[serde(default)]
    pub dynamic_skip_mask: u32,
    #[serde(default)]
    pub debris_generations: u32,
}

impl DebrisLodState {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn clear(&mut self) {
        let dynamic_skip_mask = self.dynamic_skip_mask;
        *self = Self {
            dynamic_skip_mask,
            ..Self::default()
        };
    }
    pub fn admit_dynamic_skip_mask(&mut self, mask: u32) {
        self.dynamic_skip_mask = mask;
        self.debris_generations = 0;
    }
    pub fn admit_runtime_state(&mut self, mask: u32, generations: u32) {
        self.dynamic_skip_mask = mask;
        self.debris_generations = generations;
    }
    /// C++ GameLODManager::isDebrisSkipped; called for each selected model.
    pub fn is_debris_skipped(&mut self) -> bool {
        self.debris_generations = self.debris_generations.wrapping_add(1);
        (self.debris_generations & self.dynamic_skip_mask) != self.dynamic_skip_mask
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debris_force_and_bounce_follow_disposition() {
        let upward = debris_initial_velocity(
            DebrisDisposition::SEND_IT_UP,
            Vec3::ZERO,
            0,
            10.0,
            10.0,
            90.0,
            90.0,
        );
        assert_eq!(upward, Vec3::new(0.0, 10.0, 0.0));

        let flying = debris_initial_velocity(
            DebrisDisposition::SEND_IT_FLYING | DebrisDisposition::RANDOM_FORCE,
            Vec3::new(1.0, 0.0, 0.0),
            3,
            5.0,
            7.0,
            75.0,
            90.0,
        );
        assert!(flying.length() > 1.0);

        let tank = DebrisPlan::generic_tank_debris();
        assert_eq!(tank.count, 3);
        assert!(DebrisDisposition::new(tank.disposition).has(DebrisDisposition::SEND_IT_FLYING));
        assert!(disposition_enables_bounce(tank.disposition));
        assert!(disposition_enables_bounce(DebrisDisposition::RANDOM_FORCE));
        assert!(!disposition_enables_bounce(
            DebrisDisposition::ON_GROUND_ALIGNED
        ));

        let barrel = DebrisPlan::damaged_barrel();
        assert_eq!(barrel.disposition, DebrisDisposition::RANDOM_FORCE);
        assert_eq!(barrel.min_force, 5.0);
        assert_eq!(spin_rate_rad_per_frame(180.0), std::f32::consts::PI / 30.0);
    }
}
