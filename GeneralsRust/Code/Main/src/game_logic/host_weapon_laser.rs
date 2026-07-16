//! Host Weapon.ini LaserName residual beams.
//!
//! C++ `Weapon::createLaser` / LaserUpdate path creates a laser Thing from
//! `LaserName` between shooter and target. Host residual freezes a short-lived
//! beam descriptor for PresentationFrame / laser_segment_upload.
//!
//! Fail-closed: not full ThingFactory laser object, bone attach, or WGPU
//! W3DLaserDraw texture sample parity.

use super::ObjectId;
use serde::{Deserialize, Serialize};

/// Retail-ish lifetime for a combat laser residual (frames @ 30 Hz).
/// PointDefenseLaserBeam LifetimeUpdate is ~95ms → ~3f; keep a slightly longer
/// observe window so presentation can freeze the beam mid-frame.
pub const WEAPON_LASER_LIFETIME_FRAMES: u32 = 6;

/// Host residual weapon laser beam (LaserName template).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResidualWeaponLaser {
    pub laser_name: String,
    /// C++ Weapon.ini LaserBoneName residual (muzzle/bone attach).
    pub laser_bone_name: String,
    pub from_id: ObjectId,
    pub to_id: Option<ObjectId>,
    pub from_x: f32,
    pub from_y: f32,
    pub from_z: f32,
    pub to_x: f32,
    pub to_y: f32,
    pub to_z: f32,
    pub expires_frame: u32,
    pub scroll_offset: f32,
}

impl ResidualWeaponLaser {
    pub fn new(
        laser_name: impl Into<String>,
        from_id: ObjectId,
        to_id: Option<ObjectId>,
        from: (f32, f32, f32),
        to: (f32, f32, f32),
        start_frame: u32,
    ) -> Self {
        Self::with_bone(laser_name, "", from_id, to_id, from, to, start_frame)
    }

    pub fn with_bone(
        laser_name: impl Into<String>,
        laser_bone_name: impl Into<String>,
        from_id: ObjectId,
        to_id: Option<ObjectId>,
        from: (f32, f32, f32),
        to: (f32, f32, f32),
        start_frame: u32,
    ) -> Self {
        Self {
            laser_name: laser_name.into(),
            laser_bone_name: laser_bone_name.into(),
            from_id,
            to_id,
            from_x: from.0,
            from_y: from.1,
            from_z: from.2,
            to_x: to.0,
            to_y: to.1,
            to_z: to.2,
            expires_frame: start_frame.saturating_add(WEAPON_LASER_LIFETIME_FRAMES.max(1)),
            scroll_offset: 0.0,
        }
    }

    pub fn is_active_at(&self, frame: u32) -> bool {
        frame < self.expires_frame
    }

    pub fn from_pos(&self) -> (f32, f32, f32) {
        (self.from_x, self.from_y, self.from_z)
    }

    pub fn to_pos(&self) -> (f32, f32, f32) {
        (self.to_x, self.to_y, self.to_z)
    }
}

/// Advance residual scroll and drop expired beams.
pub fn update_weapon_lasers(lasers: &mut Vec<ResidualWeaponLaser>, frame: u32) {
    for l in lasers.iter_mut() {
        // W3DLaserDraw ScrollRate residual-ish: advance slowly.
        l.scroll_offset = l.scroll_offset + 0.05;
    }
    lasers.retain(|l| l.is_active_at(frame));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weapon_laser_expires_and_retains_name() {
        let l = ResidualWeaponLaser::new(
            "PointDefenseLaserBeam",
            ObjectId(1),
            Some(ObjectId(2)),
            (0.0, 0.0, 0.0),
            (10.0, 0.0, 0.0),
            100,
        );
        assert_eq!(l.laser_name, "PointDefenseLaserBeam");
        assert!(l.laser_bone_name.is_empty());
        let l2 = ResidualWeaponLaser::with_bone(
            "PointDefenseLaserBeam",
            "LASER",
            ObjectId(1),
            Some(ObjectId(2)),
            (0.0, 0.0, 0.0),
            (10.0, 0.0, 0.0),
            100,
        );
        assert_eq!(l2.laser_bone_name, "LASER");
        assert!(l.is_active_at(100));
        assert!(l.is_active_at(105));
        assert!(!l.is_active_at(106));
        let mut v = vec![l];
        update_weapon_lasers(&mut v, 106);
        assert!(v.is_empty());
    }
}
