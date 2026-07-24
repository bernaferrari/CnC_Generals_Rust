//! Host SmartBombTargetHomingUpdate residual (falling bomb course fudge).
//!
//! C++: `SmartBombTargetHomingUpdate::update`
//! - Requires `SetTargetPosition` then only steers while significantly above terrain
//! - `pos.xz = lerp(current, target, 1 - scalar)` with scalar default **0.99**
//! - Height axis unchanged (C++ Z / host Y)
//!
//! Retail peel (`WeaponObjects.ini` MOAB):
//! - `CourseCorrectionScalar = 0.99` (1=no homing, 0=snap)
//!
//! Fail-closed: not full isSignificantlyAboveTerrain geometry peel beyond height
//! threshold residual / random wake bias.

use glam::Vec3;
use serde::{Deserialize, Serialize};

/// C++ module default CourseCorrectionScalar.
pub const SMART_BOMB_DEFAULT_COURSE_SCALAR: f32 = 0.99;
/// Retail MOAB peel.
pub const MOAB_COURSE_CORRECTION_SCALAR: f32 = 0.99;
/// Host residual: height above terrain required to steer (world Y).
pub const SMART_BOMB_SIGNIFICANTLY_ABOVE_TERRAIN: f32 = 5.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostSmartBombTargetHomingData {
    pub course_correction_scalar: f32,
    pub target_received: bool,
    pub target: Vec3,
}

impl Default for HostSmartBombTargetHomingData {
    fn default() -> Self {
        Self {
            course_correction_scalar: SMART_BOMB_DEFAULT_COURSE_SCALAR,
            target_received: false,
            target: Vec3::ZERO,
        }
    }
}

impl HostSmartBombTargetHomingData {
    pub fn with_scalar(scalar: f32) -> Self {
        Self {
            course_correction_scalar: scalar.clamp(0.0, 1.0),
            ..Self::default()
        }
    }

    pub fn for_template(template_name: &str) -> Option<Self> {
        if is_smart_bomb_homing_template(template_name) {
            Some(Self::with_scalar(MOAB_COURSE_CORRECTION_SCALAR))
        } else {
            None
        }
    }

    /// C++ SetTargetPosition — reject zero-length coord residual.
    pub fn set_target_position(&mut self, target: Vec3) -> bool {
        if target.length() <= 0.0 {
            return false;
        }
        self.target = target;
        self.target_received = true;
        true
    }

    /// One frame course correction. Returns new position if steered.
    pub fn tick(
        &self,
        current_pos: Vec3,
        height_above_terrain: f32,
    ) -> Option<Vec3> {
        if !self.target_received {
            return None;
        }
        if height_above_terrain < SMART_BOMB_SIGNIFICANTLY_ABOVE_TERRAIN {
            return None;
        }
        let status = self.course_correction_scalar.clamp(0.0, 1.0);
        let target_c = 1.0 - status;
        Some(Vec3::new(
            self.target.x * target_c + current_pos.x * status,
            current_pos.y, // keep altitude (C++ keeps Z)
            self.target.z * target_c + current_pos.z * status,
        ))
    }
}

pub fn is_smart_bomb_homing_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("moab")
        || n.contains("daisycutter")
        || n.contains("daisy_cutter")
        || n.contains("smartbomb")
        || n.contains("fuelairbomb")
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostSmartBombTargetHomingRegistry {
    pub installed: u32,
    pub targets_set: u32,
    pub steers: u32,
}

impl HostSmartBombTargetHomingRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn record_install(&mut self) {
        self.installed = self.installed.saturating_add(1);
    }
    pub fn record_target(&mut self) {
        self.targets_set = self.targets_set.saturating_add(1);
    }
    pub fn record_steer(&mut self) {
        self.steers = self.steers.saturating_add(1);
    }
    pub fn honesty_host_path_ok(&self) -> bool {
        self.installed > 0 || self.steers > 0 || self.targets_set > 0
    }
}

pub fn honesty_smart_bomb_target_homing_residual_ok() -> bool {
    (SMART_BOMB_DEFAULT_COURSE_SCALAR - 0.99).abs() < 1.0e-6
        && (MOAB_COURSE_CORRECTION_SCALAR - 0.99).abs() < 1.0e-6
        && is_smart_bomb_homing_template("MOAB")
        && is_smart_bomb_homing_template("AmericaMOAB")
        && !is_smart_bomb_homing_template("AmericaTankCrusader")
        && {
            let mut d = HostSmartBombTargetHomingData::with_scalar(0.99);
            let ok = d.set_target_position(Vec3::new(100.0, 50.0, 0.0));
            let p0 = Vec3::new(0.0, 80.0, 0.0);
            match d.tick(p0, 80.0) {
                Some(p1) => {
                    ok && (p1.x - 1.0).abs() < 0.01 && (p1.y - 80.0).abs() < 1.0e-5
                }
                None => false,
            }
        }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn residual_pack_and_lerp() {
        // honesty uses assert! in non-test in old pattern — rewrite honesty without assert
        let mut d = HostSmartBombTargetHomingData::with_scalar(0.99);
        assert!(d.set_target_position(Vec3::new(100.0, 0.0, 0.0)));
        assert!(!d.set_target_position(Vec3::ZERO));
        let p = d
            .tick(Vec3::new(0.0, 40.0, 0.0), 40.0)
            .expect("above terrain");
        assert!((p.x - 1.0).abs() < 0.01);
        assert!((p.y - 40.0).abs() < 1e-5);
        assert!(d.tick(Vec3::new(0.0, 1.0, 0.0), 1.0).is_none());
        assert!(is_smart_bomb_homing_template("MOAB"));
        assert!((SMART_BOMB_DEFAULT_COURSE_SCALAR - 0.99).abs() < 1e-6);
    }
}
