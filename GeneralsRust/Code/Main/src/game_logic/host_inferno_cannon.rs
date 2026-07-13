//! Host China Inferno Cannon residual fire zone (FireFieldSmall).
//!
//! Residual slice (playability):
//! - Inferno Cannon attack impact spawns a residual fire damage zone at the
//!   impact point (retail InfernoTankShell FireWeaponWhenDeadBehavior →
//!   SmallFireFieldCreationWeapon → OCL_FireFieldSmall → FireFieldSmall).
//! - Zones tick FLAME damage on DelayBetweenShots residual interval for a
//!   DeletionUpdate lifetime residual so units take fire DoT after the shell.
//! - Honesty counters/flags for residual gates and tests.
//!
//! Fail-closed honesty:
//! - Not full InfernoTankShell DumbProjectileBehavior bezier lob path
//! - Not full FireWeaponWhenDeadBehavior / OCL_FireFieldSmall object spawn
//! - Not BlackNapalm / InfernoCannonGunUpgraded FireFieldUpgradedSmall path
//! - Not HistoricBonus FirestormSmallCreationWeapon multi-shell matrix
//! - Not multiplayer shared-synced fire field / particle bone attach parity

use super::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Logic frames per second (host fixed step).
pub const INFERNO_LOGIC_FPS: f32 = 30.0;

/// Retail FireFieldSmall DeletionUpdate Min/MaxLifetime = 2500 ms @ 30 FPS.
pub const INFERNO_FIRE_DURATION_FRAMES: u32 = 75;

/// Retail SmallFireFieldWeapon DelayBetweenShots = 250 ms → ~7.5 frames @ 30 FPS.
pub const INFERNO_FIRE_TICK_INTERVAL_FRAMES: u32 = 8;

/// Retail SmallFireFieldWeapon PrimaryDamage.
pub const INFERNO_FIRE_DAMAGE_PER_TICK: f32 = 5.0;

/// Retail SmallFireFieldWeapon PrimaryDamageRadius.
pub const INFERNO_FIRE_RADIUS: f32 = 30.0;

/// Retail InfernoCannonGun AttackRange residual.
pub const INFERNO_CANNON_ATTACK_RANGE: f32 = 300.0;

/// Retail InfernoCannonGun MinimumAttackRange residual.
pub const INFERNO_CANNON_MIN_RANGE: f32 = 50.0;

/// Retail InfernoCannonGun DelayBetweenShots 4000 ms → 120 frames @ 30 FPS.
pub const INFERNO_CANNON_DELAY_FRAMES: u32 = 120;

/// Retail InfernoCannonGun PrimaryDamage (shell impact residual).
pub const INFERNO_CANNON_SHELL_DAMAGE: f32 = 30.0;

/// Retail InfernoCannonGun PrimaryDamageRadius.
pub const INFERNO_CANNON_SHELL_RADIUS: f32 = 15.0;

/// Retail primary weapon template name.
pub const INFERNO_CANNON_PRIMARY_WEAPON: &str = "InfernoCannonGun";

/// Retail upgraded primary (BlackNapalm) — not fully modeled; name residual only.
pub const INFERNO_CANNON_UPGRADED_WEAPON: &str = "InfernoCannonGunUpgraded";

/// Fire / detonation audio residual.
pub const INFERNO_CANNON_FIRE_AUDIO: &str = "InfernoCannonWeapon";

/// Ambient fire residual cue name (particle / field residual).
pub const INFERNO_FIRE_BURN_AUDIO: &str = "InfernoCannonFire";

/// Whether template is a residual Inferno Cannon that spawns fire zones.
///
/// Fail-closed: name residual (not full INI WeaponSet / Artillery style matrix).
/// Excludes projectile shells (`InfernoTankShell`).
pub fn is_inferno_cannon_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n == "testinfernocannon" || n == "test_inferno_cannon" {
        return true;
    }
    // Projectile / shell / fire-field objects are not the cannon vehicle.
    if n.contains("shell")
        || n.contains("projectile")
        || n.contains("firefield")
        || n.contains("fire_field")
    {
        return false;
    }
    n.contains("infernocannon") || n.contains("inferno_cannon") || n.contains("vehicleinferno")
}

/// One active residual Inferno fire damage zone (FireFieldSmall residual).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostInfernoFireZone {
    pub id: u32,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub position: Vec3,
    pub radius: f32,
    pub damage_per_tick: f32,
    pub activate_frame: u32,
    pub expires_frame: u32,
    /// Next absolute frame at which damage ticks apply.
    pub next_tick_frame: u32,
    /// Upgraded BlackNapalm residual (higher damage when true).
    pub upgraded: bool,
    /// Total damage dealt across all ticks (honesty / tests).
    pub total_damage_applied: f32,
    /// Number of distinct damage applications (object×tick).
    pub damage_applications: u32,
    /// Number of objects destroyed by this zone.
    pub objects_destroyed: u32,
}

impl HostInfernoFireZone {
    pub fn is_expired(&self, current_frame: u32) -> bool {
        current_frame >= self.expires_frame
    }

    pub fn is_due_tick(&self, current_frame: u32) -> bool {
        !self.is_expired(current_frame) && current_frame >= self.next_tick_frame
    }
}

/// Damage application plan for a single victim this tick.
#[derive(Debug, Clone, Copy)]
pub struct HostInfernoFireDamageHit {
    pub target_id: ObjectId,
    pub damage: f32,
    pub zone_id: u32,
}

/// Result of resolving one zone's damage tick.
#[derive(Debug, Clone)]
pub struct HostInfernoFireTickPlan {
    pub zone_id: u32,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub hits: Vec<HostInfernoFireDamageHit>,
}

/// Host residual registry for Inferno Cannon fire zones.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostInfernoFireZoneRegistry {
    next_id: u32,
    /// Active (not yet expired) residual fire zones.
    active: Vec<HostInfernoFireZone>,
    /// Total fire zones spawned (honesty).
    pub zones_spawned: u32,
    /// Zones that have expired (bookkeeping prune).
    pub expirations: u32,
    /// Total residual damage applied across all zones.
    pub total_damage_applied: f32,
    /// Total damage application events.
    pub damage_applications: u32,
    /// Objects destroyed by residual fire.
    pub objects_destroyed: u32,
}

impl HostInfernoFireZoneRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    pub fn active_zones(&self) -> &[HostInfernoFireZone] {
        &self.active
    }

    pub fn zones_spawned(&self) -> u32 {
        self.zones_spawned
    }

    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    /// Spawn a residual FireFieldSmall at impact from an Inferno Cannon shell.
    ///
    /// Retail path: InfernoTankShell death → SmallFireFieldCreationWeapon →
    /// OCL_FireFieldSmall → FireFieldSmall with SmallFireFieldWeapon.
    pub fn spawn_zone(
        &mut self,
        source_object: ObjectId,
        source_team: super::Team,
        impact_pos: Vec3,
        activate_frame: u32,
        upgraded: bool,
    ) -> u32 {
        let id = self.alloc_id();
        let (damage, radius) = if upgraded {
            // Retail SmallFireFieldWeaponUpgraded: 7.5 dmg / 30 radius.
            (7.5_f32, INFERNO_FIRE_RADIUS)
        } else {
            (INFERNO_FIRE_DAMAGE_PER_TICK, INFERNO_FIRE_RADIUS)
        };
        let zone = HostInfernoFireZone {
            id,
            source_object,
            source_team,
            position: impact_pos,
            radius,
            damage_per_tick: damage,
            activate_frame,
            expires_frame: activate_frame.saturating_add(INFERNO_FIRE_DURATION_FRAMES),
            // First damage tick on the activation frame so residual is immediately observable.
            next_tick_frame: activate_frame,
            upgraded,
            total_damage_applied: 0.0,
            damage_applications: 0,
            objects_destroyed: 0,
        };
        self.active.push(zone);
        self.zones_spawned = self.zones_spawned.saturating_add(1);
        id
    }

    /// Plan damage for all zones due to tick this frame.
    ///
    /// Retail SmallFireFieldWeapon RadiusDamageAffects = ALLIES ENEMIES NEUTRALS
    /// (friendly fire). Residual still skips the source Inferno Cannon object.
    pub fn plan_due_ticks(
        &self,
        current_frame: u32,
        object_positions: &[(ObjectId, Vec3, super::Team, bool)],
    ) -> Vec<HostInfernoFireTickPlan> {
        let mut plans = Vec::new();
        for zone in &self.active {
            if !zone.is_due_tick(current_frame) {
                continue;
            }
            let mut hits = Vec::new();
            let r2 = zone.radius * zone.radius;
            for &(id, pos, _team, alive) in object_positions {
                if !alive || id == zone.source_object {
                    continue;
                }
                let dx = pos.x - zone.position.x;
                let dz = pos.z - zone.position.z;
                if dx * dx + dz * dz <= r2 {
                    hits.push(HostInfernoFireDamageHit {
                        target_id: id,
                        damage: zone.damage_per_tick,
                        zone_id: zone.id,
                    });
                }
            }
            plans.push(HostInfernoFireTickPlan {
                zone_id: zone.id,
                source_object: zone.source_object,
                source_team: zone.source_team,
                hits,
            });
        }
        plans.sort_by_key(|p| p.zone_id);
        plans
    }

    /// Record results after GameLogic applied a tick's damage.
    pub fn record_tick_complete(
        &mut self,
        zone_id: u32,
        damage_applied: f32,
        applications: u32,
        destroyed: u32,
        current_frame: u32,
    ) {
        if let Some(zone) = self.active.iter_mut().find(|z| z.id == zone_id) {
            zone.total_damage_applied += damage_applied;
            zone.damage_applications = zone.damage_applications.saturating_add(applications);
            zone.objects_destroyed = zone.objects_destroyed.saturating_add(destroyed);
            zone.next_tick_frame =
                current_frame.saturating_add(INFERNO_FIRE_TICK_INTERVAL_FRAMES);
        }
        self.total_damage_applied += damage_applied;
        self.damage_applications = self.damage_applications.saturating_add(applications);
        self.objects_destroyed = self.objects_destroyed.saturating_add(destroyed);
    }

    /// Drop expired zones.
    pub fn prune_expired(&mut self, current_frame: u32) {
        let before = self.active.len();
        self.active.retain(|z| !z.is_expired(current_frame));
        let removed = before.saturating_sub(self.active.len()) as u32;
        self.expirations = self.expirations.saturating_add(removed);
    }

    /// Residual honesty: at least one fire zone spawned.
    pub fn honesty_spawn_ok(&self) -> bool {
        self.zones_spawned > 0
    }

    /// Residual honesty: fire damage was applied to at least one victim tick.
    pub fn honesty_damage_ok(&self) -> bool {
        self.damage_applications > 0 && self.total_damage_applied > 0.0
    }

    /// Combined host path: spawned a zone and dealt residual fire damage.
    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_spawn_ok() && self.honesty_damage_ok()
    }

    /// True if any active residual zone covers `pos` horizontally.
    pub fn is_position_in_active_fire(&self, pos: Vec3) -> bool {
        self.active.iter().any(|z| {
            let dx = pos.x - z.position.x;
            let dz = pos.z - z.position.z;
            dx * dx + dz * dz <= z.radius * z.radius
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::Team;

    #[test]
    fn inferno_cannon_name_matrix() {
        assert!(is_inferno_cannon_template("ChinaVehicleInfernoCannon"));
        assert!(is_inferno_cannon_template("China_InfernoCannon"));
        assert!(is_inferno_cannon_template("Nuke_ChinaVehicleInfernoCannon"));
        assert!(is_inferno_cannon_template("TestInfernoCannon"));
        assert!(!is_inferno_cannon_template("ChinaTankBattleMaster"));
        assert!(!is_inferno_cannon_template("USA_Ranger"));
        assert!(!is_inferno_cannon_template("InfernoTankShell"));
        assert!(!is_inferno_cannon_template("FireFieldSmall"));
    }

    #[test]
    fn spawn_and_tick_damages_enemy_in_radius() {
        let mut reg = HostInfernoFireZoneRegistry::new();
        let id = reg.spawn_zone(
            ObjectId(1),
            Team::China,
            Vec3::new(100.0, 0.0, 0.0),
            0,
            false,
        );
        assert!(reg.honesty_spawn_ok());
        assert!(!reg.honesty_damage_ok());
        assert_eq!(reg.active_count(), 1);
        assert_eq!(reg.active_zones()[0].id, id);

        let impact = reg.active_zones()[0].position;
        let objects = vec![
            (ObjectId(1), Vec3::new(0.0, 0.0, 0.0), Team::China, true),
            (ObjectId(2), impact, Team::GLA, true),
            (ObjectId(3), Vec3::new(0.0, 0.0, 500.0), Team::GLA, true),
        ];
        let plans = reg.plan_due_ticks(0, &objects);
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].hits.len(), 1);
        assert_eq!(plans[0].hits[0].target_id, ObjectId(2));
        assert!((plans[0].hits[0].damage - INFERNO_FIRE_DAMAGE_PER_TICK).abs() < 0.01);

        reg.record_tick_complete(id, INFERNO_FIRE_DAMAGE_PER_TICK, 1, 0, 0);
        assert!(reg.honesty_damage_ok());
        assert!(reg.honesty_host_path_ok());
        assert_eq!(
            reg.active_zones()[0].next_tick_frame,
            INFERNO_FIRE_TICK_INTERVAL_FRAMES
        );

        // Not due again until interval elapses.
        assert!(reg.plan_due_ticks(1, &objects).is_empty());
        assert!(!reg
            .plan_due_ticks(INFERNO_FIRE_TICK_INTERVAL_FRAMES, &objects)
            .is_empty());
    }

    #[test]
    fn prune_expired_after_duration() {
        let mut reg = HostInfernoFireZoneRegistry::new();
        reg.spawn_zone(
            ObjectId(1),
            Team::China,
            Vec3::new(50.0, 0.0, 0.0),
            10,
            false,
        );
        reg.prune_expired(10 + INFERNO_FIRE_DURATION_FRAMES - 1);
        assert_eq!(reg.active_count(), 1);
        reg.prune_expired(10 + INFERNO_FIRE_DURATION_FRAMES);
        assert_eq!(reg.active_count(), 0);
        assert_eq!(reg.expirations, 1);
    }

    #[test]
    fn upgraded_zone_deals_higher_damage() {
        let mut reg = HostInfernoFireZoneRegistry::new();
        reg.spawn_zone(
            ObjectId(1),
            Team::China,
            Vec3::new(0.0, 0.0, 0.0),
            0,
            true,
        );
        assert!((reg.active_zones()[0].damage_per_tick - 7.5).abs() < 0.01);
        let objects = vec![(ObjectId(2), Vec3::ZERO, Team::USA, true)];
        let plans = reg.plan_due_ticks(0, &objects);
        assert_eq!(plans[0].hits.len(), 1);
        assert!((plans[0].hits[0].damage - 7.5).abs() < 0.01);
    }
}
