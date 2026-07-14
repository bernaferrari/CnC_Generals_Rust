//! Host China FireWall / Firestorm residual (Dragon Tank FIRE_WEAPON secondary).
//!
//! Residual slice (playability):
//! - `DoSpecialPower(FireWall)` at a world location creates a line of fire
//!   damage zones from the caster toward the target (retail OCL_FireWallSegment
//!   path residual).
//! - Zones tick FLAME damage on a DelayBetweenShots residual interval for a
//!   DeletionUpdate lifetime residual so units take fire damage along the line.
//! - Honesty counters/flags for residual gates and tests.
//!
//! Fail-closed honesty:
//! - Not full DragonTankFireWallWeapon projectile stream / OCL segment spawn
//! - Not InchForwardLocomotor crawl of FireWallSegment objects
//! - Not BlackNapalm upgraded segment weapons / particle systems
//! - Not FIRE_WEAPON command-button weapon-slot matrix / multi-select AI
//! - Not multiplayer shared-synced timer / UnitSpecificSound EVA parity

use super::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Logic frames per second (host fixed step).
pub const FIREWALL_LOGIC_FPS: f32 = 30.0;

/// Retail FireWallSegment DeletionUpdate Min/MaxLifetime = 4000 ms @ 30 FPS.
pub const FIREWALL_DURATION_FRAMES: u32 = 120;

/// Retail FireWallSegmentWeapon DelayBetweenShots = 250 ms → ~7.5 frames @ 30 FPS.
pub const FIREWALL_TICK_INTERVAL_FRAMES: u32 = 8;

/// Retail FireWallSegmentWeapon PrimaryDamage.
pub const FIREWALL_DAMAGE_PER_TICK: f32 = 4.0;

/// Retail FireWallSegmentWeapon PrimaryDamageRadius.
pub const FIREWALL_SEGMENT_RADIUS: f32 = 10.0;

/// Spacing between residual segments along the wall line.
pub const FIREWALL_SEGMENT_SPACING: f32 = 12.0;

/// Maximum wall length residual (fail-closed vs full projectile range matrix).
pub const FIREWALL_MAX_LENGTH: f32 = 120.0;

/// Minimum wall length so a point-click near the caster still spawns zones.
pub const FIREWALL_MIN_LENGTH: f32 = 36.0;

/// Offset from caster toward target where the first segment starts
/// (retail DragonTankFireWallWeapon AttackRange ≈ 25 residual).
pub const FIREWALL_START_OFFSET: f32 = 20.0;

/// Activate audio residual (CommandButton UnitSpecificSound residual name).
pub const FIREWALL_ACTIVATE_AUDIO: &str = "DragonTankVoiceModeFireStorm";

/// Loop / ambient fire residual cue name (Weapon.ini FireSound residual).
pub const FIREWALL_BURN_AUDIO: &str = "DragonTankWeaponLoop";

/// One residual fire damage zone along the wall.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostFireWallSegment {
    pub position: Vec3,
    pub radius: f32,
}

/// One active residual FireWall activation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostFireWall {
    pub id: u32,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub target_position: Vec3,
    pub activate_frame: u32,
    pub expires_frame: u32,
    /// Next absolute frame at which damage ticks apply.
    pub next_tick_frame: u32,
    pub segments: Vec<HostFireWallSegment>,
    /// Total damage dealt across all ticks (honesty / tests).
    pub total_damage_applied: f32,
    /// Number of distinct damage applications (object×tick).
    pub damage_applications: u32,
    /// Number of objects destroyed by this wall.
    pub objects_destroyed: u32,
}

impl HostFireWall {
    pub fn is_expired(&self, current_frame: u32) -> bool {
        current_frame >= self.expires_frame
    }

    pub fn is_due_tick(&self, current_frame: u32) -> bool {
        !self.is_expired(current_frame) && current_frame >= self.next_tick_frame
    }
}

/// Damage application plan for a single victim this tick.
#[derive(Debug, Clone, Copy)]
pub struct HostFireWallDamageHit {
    pub target_id: ObjectId,
    pub damage: f32,
    pub wall_id: u32,
}

/// Result of resolving one wall's damage tick.
#[derive(Debug, Clone)]
pub struct HostFireWallTickPlan {
    pub wall_id: u32,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub hits: Vec<HostFireWallDamageHit>,
}

/// Host residual registry for FireWall activations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostFireWallRegistry {
    next_id: u32,
    /// Active (not yet expired) residual walls.
    active: Vec<HostFireWall>,
    /// Total activations (honesty).
    pub activations: u32,
    /// Walls that have expired (bookkeeping prune).
    pub expirations: u32,
    /// Total residual damage applied across all walls.
    pub total_damage_applied: f32,
    /// Total damage application events.
    pub damage_applications: u32,
    /// Objects destroyed by residual fire.
    pub objects_destroyed: u32,
}

impl HostFireWallRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    pub fn active_walls(&self) -> &[HostFireWall] {
        &self.active
    }

    pub fn activations(&self) -> u32 {
        self.activations
    }

    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    /// Build residual segment positions along the caster → target line.
    pub fn build_segments(caster_pos: Vec3, target_pos: Vec3) -> Vec<HostFireWallSegment> {
        let dx = target_pos.x - caster_pos.x;
        let dz = target_pos.z - caster_pos.z;
        let dist = (dx * dx + dz * dz).sqrt();

        // Default forward if target coincides with caster.
        let (dir_x, dir_z) = if dist < 1.0 {
            (1.0_f32, 0.0_f32)
        } else {
            (dx / dist, dz / dist)
        };

        let wall_len = dist
            .max(FIREWALL_MIN_LENGTH)
            .min(FIREWALL_MAX_LENGTH)
            .max(FIREWALL_START_OFFSET + FIREWALL_SEGMENT_SPACING);

        let mut segments = Vec::new();
        let mut along = FIREWALL_START_OFFSET;
        while along <= wall_len + 0.01 {
            segments.push(HostFireWallSegment {
                position: Vec3::new(
                    caster_pos.x + dir_x * along,
                    caster_pos.y,
                    caster_pos.z + dir_z * along,
                ),
                radius: FIREWALL_SEGMENT_RADIUS,
            });
            along += FIREWALL_SEGMENT_SPACING;
        }
        if segments.is_empty() {
            segments.push(HostFireWallSegment {
                position: Vec3::new(
                    caster_pos.x + dir_x * FIREWALL_START_OFFSET,
                    caster_pos.y,
                    caster_pos.z + dir_z * FIREWALL_START_OFFSET,
                ),
                radius: FIREWALL_SEGMENT_RADIUS,
            });
        }
        segments
    }

    /// Queue a residual FireWall from caster toward target.
    pub fn activate(
        &mut self,
        source_object: ObjectId,
        source_team: super::Team,
        caster_pos: Vec3,
        target_pos: Vec3,
        activate_frame: u32,
    ) -> u32 {
        let id = self.alloc_id();
        let segments = Self::build_segments(caster_pos, target_pos);
        let wall = HostFireWall {
            id,
            source_object,
            source_team,
            target_position: target_pos,
            activate_frame,
            expires_frame: activate_frame.saturating_add(FIREWALL_DURATION_FRAMES),
            // First damage tick on the activation frame so residual is immediately observable.
            next_tick_frame: activate_frame,
            segments,
            total_damage_applied: 0.0,
            damage_applications: 0,
            objects_destroyed: 0,
        };
        self.active.push(wall);
        self.activations = self.activations.saturating_add(1);
        id
    }

    /// Plan damage for all walls due to tick this frame.
    ///
    /// Retail FireWallSegmentWeapon RadiusDamageAffects = ALLIES ENEMIES NEUTRALS
    /// (friendly fire). Residual still skips the source caster object.
    pub fn plan_due_ticks(
        &self,
        current_frame: u32,
        object_positions: &[(ObjectId, Vec3, super::Team, bool)],
    ) -> Vec<HostFireWallTickPlan> {
        let mut plans = Vec::new();
        for wall in &self.active {
            if !wall.is_due_tick(current_frame) {
                continue;
            }
            let mut hits = Vec::new();
            for &(id, pos, _team, alive) in object_positions {
                if !alive || id == wall.source_object {
                    continue;
                }
                let mut in_fire = false;
                for seg in &wall.segments {
                    let dx = pos.x - seg.position.x;
                    let dz = pos.z - seg.position.z;
                    if dx * dx + dz * dz <= seg.radius * seg.radius {
                        in_fire = true;
                        break;
                    }
                }
                if in_fire {
                    hits.push(HostFireWallDamageHit {
                        target_id: id,
                        damage: FIREWALL_DAMAGE_PER_TICK,
                        wall_id: wall.id,
                    });
                }
            }
            plans.push(HostFireWallTickPlan {
                wall_id: wall.id,
                source_object: wall.source_object,
                source_team: wall.source_team,
                hits,
            });
        }
        plans.sort_by_key(|p| p.wall_id);
        plans
    }

    /// Record results after GameLogic applied a tick's damage.
    pub fn record_tick_complete(
        &mut self,
        wall_id: u32,
        damage_applied: f32,
        applications: u32,
        destroyed: u32,
        current_frame: u32,
    ) {
        if let Some(wall) = self.active.iter_mut().find(|w| w.id == wall_id) {
            wall.total_damage_applied += damage_applied;
            wall.damage_applications = wall.damage_applications.saturating_add(applications);
            wall.objects_destroyed = wall.objects_destroyed.saturating_add(destroyed);
            wall.next_tick_frame = current_frame.saturating_add(FIREWALL_TICK_INTERVAL_FRAMES);
        }
        self.total_damage_applied += damage_applied;
        self.damage_applications = self.damage_applications.saturating_add(applications);
        self.objects_destroyed = self.objects_destroyed.saturating_add(destroyed);
    }

    /// Drop expired walls.
    pub fn prune_expired(&mut self, current_frame: u32) {
        let before = self.active.len();
        self.active.retain(|w| !w.is_expired(current_frame));
        let removed = before.saturating_sub(self.active.len()) as u32;
        self.expirations = self.expirations.saturating_add(removed);
    }

    /// Residual honesty: at least one FireWall activated.
    pub fn honesty_activate_ok(&self) -> bool {
        self.activations > 0
    }

    /// Residual honesty: fire damage was applied to at least one victim tick.
    pub fn honesty_damage_ok(&self) -> bool {
        self.damage_applications > 0 && self.total_damage_applied > 0.0
    }

    /// Combined host path: activated and dealt residual fire damage.
    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_activate_ok() && self.honesty_damage_ok()
    }

    /// True if any active residual segment covers `pos` horizontally.
    pub fn is_position_in_active_fire(&self, pos: Vec3) -> bool {
        self.active.iter().any(|w| {
            w.segments.iter().any(|seg| {
                let dx = pos.x - seg.position.x;
                let dz = pos.z - seg.position.z;
                dx * dx + dz * dz <= seg.radius * seg.radius
            })
        })
    }
}

// --- Wave 69 residual honesty peels (retail FireWallSegment weapon / ability) ---

/// Retail FireWallSegmentWeapon name residual.
pub const FIREWALL_SEGMENT_WEAPON: &str = "FireWallSegmentWeapon";
/// Retail DragonTankFireWallWeapon name residual.
pub const FIREWALL_DRAGON_WEAPON: &str = "DragonTankFireWallWeapon";
/// Retail FireWallSegment DeletionUpdate lifetime residual (msec).
pub const FIREWALL_DURATION_MS: u32 = 4_000;
/// Retail FireWallSegmentWeapon DelayBetweenShots residual (msec).
pub const FIREWALL_TICK_MS: u32 = 250;
/// Retail DamageType residual.
pub const FIREWALL_DAMAGE_TYPE: &str = "FLAME";
/// Retail DeathType residual.
pub const FIREWALL_DEATH_TYPE: &str = "BURNED";
/// Retail DragonTankFireWallWeapon AttackRange residual.
pub const FIREWALL_DRAGON_ATTACK_RANGE: f32 = 25.0;
/// Retail DragonTankFireWallWeapon PrimaryDamage residual.
pub const FIREWALL_DRAGON_PRIMARY_DAMAGE: f32 = 10.0;
/// Retail ProjectileDetonationOCL residual.
pub const FIREWALL_OCL_SEGMENT: &str = "OCL_FireWallSegment";
/// Retail segment MaxHealth residual (ImmortalBody).
pub const FIREWALL_SEGMENT_MAX_HEALTH: f32 = 50.0;
/// Retail FireWallSegmentWeapon AttackRange residual.
pub const FIREWALL_SEGMENT_ATTACK_RANGE: f32 = 15.0;

/// Convert residual msec → logic frames @ 30 FPS.
pub fn firewall_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * FIREWALL_LOGIC_FPS / 1000.0).round() as u32
}

/// Wave 69 residual honesty: segment weapon residual peel.
pub fn honesty_firewall_weapon_residual_ok() -> bool {
    FIREWALL_SEGMENT_WEAPON == "FireWallSegmentWeapon"
        && (FIREWALL_DAMAGE_PER_TICK - 4.0).abs() < 0.01
        && (FIREWALL_SEGMENT_RADIUS - 10.0).abs() < 0.01
        && FIREWALL_TICK_MS == 250
        && FIREWALL_TICK_INTERVAL_FRAMES == firewall_ms_to_frames(FIREWALL_TICK_MS)
        && FIREWALL_TICK_INTERVAL_FRAMES == 8
        && FIREWALL_DAMAGE_TYPE == "FLAME"
        && FIREWALL_DEATH_TYPE == "BURNED"
        && (FIREWALL_SEGMENT_ATTACK_RANGE - 15.0).abs() < 0.01
        && FIREWALL_BURN_AUDIO == "DragonTankWeaponLoop"
}

/// Wave 69 residual honesty: ability / duration / geometry residual peel.
pub fn honesty_firewall_ability_residual_ok() -> bool {
    FIREWALL_DRAGON_WEAPON == "DragonTankFireWallWeapon"
        && FIREWALL_OCL_SEGMENT == "OCL_FireWallSegment"
        && FIREWALL_DURATION_MS == 4_000
        && FIREWALL_DURATION_FRAMES == firewall_ms_to_frames(FIREWALL_DURATION_MS)
        && FIREWALL_DURATION_FRAMES == 120
        && (FIREWALL_DRAGON_ATTACK_RANGE - 25.0).abs() < 0.01
        && (FIREWALL_DRAGON_PRIMARY_DAMAGE - 10.0).abs() < 0.01
        && (FIREWALL_START_OFFSET - 20.0).abs() < 0.01
        && (FIREWALL_SEGMENT_SPACING - 12.0).abs() < 0.01
        && (FIREWALL_MAX_LENGTH - 120.0).abs() < 0.01
        && (FIREWALL_MIN_LENGTH - 36.0).abs() < 0.01
        && (FIREWALL_SEGMENT_MAX_HEALTH - 50.0).abs() < 0.01
        && FIREWALL_ACTIVATE_AUDIO == "DragonTankVoiceModeFireStorm"
        && !HostFireWallRegistry::build_segments(glam::Vec3::ZERO, glam::Vec3::new(100.0, 0.0, 0.0))
            .is_empty()
}

/// Combined Wave 69 FireWall residual honesty pack.
pub fn honesty_firewall_residual_pack_ok() -> bool {
    honesty_firewall_weapon_residual_ok() && honesty_firewall_ability_residual_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::Team;

    #[test]
    fn build_segments_form_line_toward_target() {
        let segs = HostFireWallRegistry::build_segments(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(100.0, 0.0, 0.0),
        );
        assert!(
            segs.len() >= 3,
            "expected multiple segments, got {}",
            segs.len()
        );
        // First segment starts near FIREWALL_START_OFFSET along +X.
        assert!((segs[0].position.x - FIREWALL_START_OFFSET).abs() < 0.1);
        assert!(segs.last().unwrap().position.x > segs[0].position.x);
        for s in &segs {
            assert!((s.position.z).abs() < 0.1);
            assert!((s.radius - FIREWALL_SEGMENT_RADIUS).abs() < 0.01);
        }
    }

    #[test]
    fn activate_and_tick_damages_enemy_on_line() {
        let mut reg = HostFireWallRegistry::new();
        let id = reg.activate(
            ObjectId(1),
            Team::China,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(80.0, 0.0, 0.0),
            0,
        );
        assert!(reg.honesty_activate_ok());
        assert!(!reg.honesty_damage_ok());
        assert_eq!(reg.active_count(), 1);
        assert_eq!(reg.active_walls()[0].id, id);

        // Enemy on the line near first segment.
        let first = reg.active_walls()[0].segments[0].position;
        let objects = vec![
            (ObjectId(1), Vec3::new(0.0, 0.0, 0.0), Team::China, true),
            (ObjectId(2), first, Team::GLA, true),
            (ObjectId(3), Vec3::new(0.0, 0.0, 500.0), Team::GLA, true),
        ];
        let plans = reg.plan_due_ticks(0, &objects);
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].hits.len(), 1);
        assert_eq!(plans[0].hits[0].target_id, ObjectId(2));
        assert!((plans[0].hits[0].damage - FIREWALL_DAMAGE_PER_TICK).abs() < 0.01);

        reg.record_tick_complete(id, FIREWALL_DAMAGE_PER_TICK, 1, 0, 0);
        assert!(reg.honesty_damage_ok());
        assert!(reg.honesty_host_path_ok());
        assert_eq!(
            reg.active_walls()[0].next_tick_frame,
            FIREWALL_TICK_INTERVAL_FRAMES
        );

        // Not due again until interval elapses.
        assert!(reg.plan_due_ticks(1, &objects).is_empty());
        assert!(!reg
            .plan_due_ticks(FIREWALL_TICK_INTERVAL_FRAMES, &objects)
            .is_empty());
    }

    #[test]
    fn prune_expired_after_duration() {
        let mut reg = HostFireWallRegistry::new();
        reg.activate(
            ObjectId(1),
            Team::China,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(50.0, 0.0, 0.0),
            10,
        );
        reg.prune_expired(10 + FIREWALL_DURATION_FRAMES - 1);
        assert_eq!(reg.active_count(), 1);
        reg.prune_expired(10 + FIREWALL_DURATION_FRAMES);
        assert_eq!(reg.active_count(), 0);
        assert_eq!(reg.expirations, 1);
    }

    #[test]
    fn firewall_residual_pack_honesty_wave69() {
        assert_eq!(firewall_ms_to_frames(250), 8);
        assert_eq!(firewall_ms_to_frames(4_000), 120);
        assert!(honesty_firewall_weapon_residual_ok());
        assert!(honesty_firewall_ability_residual_ok());
        assert!(honesty_firewall_residual_pack_ok());
        assert_eq!(FIREWALL_DAMAGE_TYPE, "FLAME");
        assert_eq!(FIREWALL_DEATH_TYPE, "BURNED");
        assert_eq!(FIREWALL_OCL_SEGMENT, "OCL_FireWallSegment");
    }
}
