//! Host Weapon.ini LaserName residual beams.
//!
//! C++ `Weapon::createLaser` / LaserUpdate path creates a laser Thing from
//! `LaserName` between shooter and target. Host residual freezes a short-lived
//! beam descriptor for PresentationFrame / laser_segment_upload.
//!
//! Fail-closed: ThingFactory laser object residual closed for combat LaserName
//! (AvengerLaserBeam etc.); not full bone attach matrix or WGPU W3DLaserDraw
//! texture sample parity.

use super::ObjectId;
use serde::{Deserialize, Serialize};

/// Retail-ish lifetime for a combat laser residual (frames @ 30 Hz).
/// PointDefenseLaserBeam LifetimeUpdate is ~95ms → ~3f; keep a slightly longer
/// observe window so presentation can freeze the beam mid-frame.
pub const WEAPON_LASER_LIFETIME_FRAMES: u32 = 6;
/// Retail AvengerLaserBeam LifetimeUpdate 205ms residual.
pub const AVENGER_WEAPON_LASER_LIFETIME_FRAMES: u32 = 7;
/// Default MaxHealth residual for laser beam Things.
pub const WEAPON_LASER_BEAM_MAX_HEALTH: f32 = 1.0;

/// Lifetime frames for a LaserName template residual.
pub fn laser_beam_lifetime_frames(laser_name: &str) -> u32 {
    let n = laser_name.to_ascii_lowercase();
    if n.contains("avenger") {
        AVENGER_WEAPON_LASER_LIFETIME_FRAMES
    } else if n.contains("pointdefense") || n.contains("point_defense") {
        3 // PointDefenseLaserBeam 95ms residual
    } else {
        WEAPON_LASER_LIFETIME_FRAMES
    }
}

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
        Self::with_bone_lifetime(
            laser_name,
            laser_bone_name,
            from_id,
            to_id,
            from,
            to,
            start_frame,
            WEAPON_LASER_LIFETIME_FRAMES,
        )
    }

    /// C++ special-object LaserUpdate lives until `killSpecialObjects`.
    pub fn with_bone_lifetime(
        laser_name: impl Into<String>,
        laser_bone_name: impl Into<String>,
        from_id: ObjectId,
        to_id: Option<ObjectId>,
        from: (f32, f32, f32),
        to: (f32, f32, f32),
        start_frame: u32,
        lifetime_frames: u32,
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
            expires_frame: start_frame.saturating_add(lifetime_frames.max(1)),
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

    /// C++ `LaserUpdate::initLaser` — rewrite both endpoints without respawning.
    pub fn retarget(&mut self, from: (f32, f32, f32), to: (f32, f32, f32)) {
        self.from_x = from.0;
        self.from_y = from.1;
        self.from_z = from.2;
        self.to_x = to.0;
        self.to_y = to.1;
        self.to_z = to.2;
    }

    /// Keep a special-ability residual alive across continuePreparation frames.
    pub fn keep_alive(&mut self, frame: u32, extra_frames: u32) {
        let until = frame.saturating_add(extra_frames.max(1));
        if self.expires_frame < until {
            self.expires_frame = until;
        }
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

/// C++ `SpecialAbilityUpdate::initLaser` endpoints: caster attach-bone world
/// pose (caller supplies bone world, else object origin) and target geometry center.
pub fn special_ability_laser_endpoints(
    caster_pos: glam::Vec3,
    target_pos: glam::Vec3,
    target_geom_height: f32,
    target_selection_radius: f32,
    target_geom_authored: bool,
) -> (glam::Vec3, glam::Vec3) {
    let lift = if target_geom_authored {
        target_geom_height * 0.5
    } else {
        target_selection_radius.max(5.0) * 0.5
    };
    (
        caster_pos,
        glam::Vec3::new(target_pos.x, target_pos.y + lift, target_pos.z),
    )
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

    #[test]
    fn special_ability_laser_end_uses_geometry_center() {
        let caster = glam::Vec3::new(1.0, 2.0, 3.0);
        let target = glam::Vec3::new(10.0, 4.0, 8.0);
        let (from, to) = special_ability_laser_endpoints(caster, target, 20.0, 6.0, true);
        assert_eq!(from, caster);
        assert!((to.x - 10.0).abs() < 1e-5);
        assert!((to.y - 14.0).abs() < 1e-5);
        assert!((to.z - 8.0).abs() < 1e-5);
        let mut laser = ResidualWeaponLaser::new(
            "BinaryDataStream",
            ObjectId(1),
            Some(ObjectId(2)),
            (from.x, from.y, from.z),
            (to.x, to.y, to.z),
            10,
        );
        laser.retarget((0.0, 0.0, 0.0), (5.0, 6.0, 7.0));
        assert_eq!(laser.to_pos(), (5.0, 6.0, 7.0));
        laser.keep_alive(10, 45);
        assert!(laser.is_active_at(54));
        assert!(!laser.is_active_at(55));
    }
}
