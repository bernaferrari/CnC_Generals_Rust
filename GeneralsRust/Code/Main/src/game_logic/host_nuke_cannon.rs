//! Host China Nuke Cannon primary residual (area shell + medium radiation field).
//!
//! Residual slice (playability):
//! - PRIMARY `NukeCannonGun` area residual:
//!   PrimaryDamage **400** / radius **50** + SecondaryDamage **20** / radius **60**.
//! - AttackRange **350**, MinimumAttackRange **150**, Delay **10000**ms (300 frames).
//! - Impact also spawns residual `OCL_RadiationFieldMedium` /
//!   `MediumRadiationFieldWeapon`: 15 dmg / radius 50 / 750ms ticks / 30s lifetime.
//! - Neutron Shell secondary remains `host_neutron_shell` residual (not re-opened).
//!
//! Fail-closed honesty:
//! - Not full NukeCannonShell DumbProjectileBehavior lob path
//! - Not full DeployStyleAIUpdate unpack / pack animation matrix
//! - Not full ScatterRadiusVsInfantry random miss matrix
//! - Not network nuke-cannon / radiation replication (network deferred)

use super::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};

// Re-export neutron helpers used by GameLogic residual gates.
pub use crate::game_logic::host_neutron_shell::{
    is_nuke_cannon_template, should_apply_neutron_blast, NUKE_CANNON_NEUTRON_WEAPON,
    NUKE_CANNON_PRIMARY_WEAPON, UPGRADE_CHINA_NEUTRON_SHELLS,
};

/// Retail NukeCannonGun PrimaryDamage.
pub const NUKE_CANNON_PRIMARY_DAMAGE: f32 = 400.0;
/// Retail NukeCannonGun PrimaryDamageRadius.
pub const NUKE_CANNON_PRIMARY_RADIUS: f32 = 50.0;
/// Retail NukeCannonGun SecondaryDamage.
pub const NUKE_CANNON_SECONDARY_DAMAGE: f32 = 20.0;
/// Retail NukeCannonGun SecondaryDamageRadius.
pub const NUKE_CANNON_SECONDARY_RADIUS: f32 = 60.0;
/// Retail AttackRange.
pub const NUKE_CANNON_ATTACK_RANGE: f32 = 350.0;
/// Retail MinimumAttackRange.
pub const NUKE_CANNON_MIN_RANGE: f32 = 150.0;
/// Retail DelayBetweenShots 10000ms → 300 frames @ 30 FPS.
pub const NUKE_CANNON_DELAY_FRAMES: u32 = 300;

/// Retail MediumRadiationFieldWeapon PrimaryDamage.
pub const MEDIUM_RADIATION_DAMAGE_PER_TICK: f32 = 15.0;
/// Retail MediumRadiationFieldWeapon PrimaryDamageRadius.
pub const MEDIUM_RADIATION_RADIUS: f32 = 50.0;
/// Retail DelayBetweenShots 750ms → ~23 frames @ 30 FPS.
pub const MEDIUM_RADIATION_TICK_INTERVAL_FRAMES: u32 = 23;
/// Retail LifetimeUpdate Min/MaxLifetime 30000ms → 900 frames @ 30 FPS.
pub const MEDIUM_RADIATION_DURATION_FRAMES: u32 = 900;

/// Residual fire / detonation audio.
pub const NUKE_CANNON_FIRE_AUDIO: &str = "NukeCannonWeapon";
/// Residual radiation ambient.
pub const MEDIUM_RADIATION_AUDIO: &str = "RadiationPoolAmbientLoop";

/// Whether residual primary fire should apply Nuke Cannon area + radiation.
///
/// Slot 0 = primary shell residual (not neutron secondary).
pub fn should_apply_nuke_cannon_primary(is_nuke_cannon: bool, fired_slot: u8) -> bool {
    is_nuke_cannon && fired_slot == 0
}

/// Area damage at distance (max of primary/secondary rings).
pub fn nuke_cannon_primary_damage_at(distance: f32) -> f32 {
    if distance <= NUKE_CANNON_PRIMARY_RADIUS {
        NUKE_CANNON_PRIMARY_DAMAGE
    } else if distance <= NUKE_CANNON_SECONDARY_RADIUS {
        NUKE_CANNON_SECONDARY_DAMAGE
    } else {
        0.0
    }
}

/// Max splash radius residual.
pub fn nuke_cannon_splash_radius() -> f32 {
    NUKE_CANNON_SECONDARY_RADIUS
}

/// Legal residual shell splash target.
pub fn is_legal_nuke_cannon_splash_target(
    is_alive: bool,
    is_self: bool,
    under_construction: bool,
    is_combat_kind: bool,
) -> bool {
    is_alive && !is_self && !under_construction && is_combat_kind
}

/// One active residual MediumRadiationField from Nuke Cannon primary impact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostMediumRadiationZone {
    pub id: u32,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub position: Vec3,
    pub radius: f32,
    pub damage_per_tick: f32,
    pub activate_frame: u32,
    pub expires_frame: u32,
    pub next_tick_frame: u32,
    pub total_damage_applied: f32,
    pub damage_applications: u32,
    pub objects_destroyed: u32,
}

impl HostMediumRadiationZone {
    pub fn is_expired(&self, current_frame: u32) -> bool {
        current_frame >= self.expires_frame
    }

    pub fn is_due_tick(&self, current_frame: u32) -> bool {
        !self.is_expired(current_frame) && current_frame >= self.next_tick_frame
    }
}

/// Damage application plan for a single victim this tick.
#[derive(Debug, Clone, Copy)]
pub struct HostMediumRadiationDamageHit {
    pub target_id: ObjectId,
    pub damage: f32,
    pub zone_id: u32,
}

/// Result of resolving one radiation zone's damage tick.
#[derive(Debug, Clone)]
pub struct HostMediumRadiationTickPlan {
    pub zone_id: u32,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub hits: Vec<HostMediumRadiationDamageHit>,
}

/// Host residual registry for Nuke Cannon primary area + medium radiation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostNukeCannonRegistry {
    next_id: u32,
    active: Vec<HostMediumRadiationZone>,
    /// Primary shell area residual blasts.
    pub primary_blasts: u32,
    /// Units hit by primary area residual.
    pub units_hit: u32,
    /// Medium radiation zones spawned.
    pub radiation_zones_spawned: u32,
    pub radiation_expirations: u32,
    pub radiation_total_damage: f32,
    pub radiation_damage_applications: u32,
    pub radiation_objects_destroyed: u32,
}

impl HostNukeCannonRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    pub fn active_zones(&self) -> &[HostMediumRadiationZone] {
        &self.active
    }

    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    pub fn record_primary_blast(&mut self, units_hit: u32) {
        self.primary_blasts = self.primary_blasts.saturating_add(1);
        self.units_hit = self.units_hit.saturating_add(units_hit);
    }

    /// Spawn residual MediumRadiationField at Nuke Cannon primary impact.
    pub fn spawn_radiation_zone(
        &mut self,
        source_object: ObjectId,
        source_team: super::Team,
        impact_pos: Vec3,
        activate_frame: u32,
    ) -> u32 {
        let id = self.alloc_id();
        let zone = HostMediumRadiationZone {
            id,
            source_object,
            source_team,
            position: impact_pos,
            radius: MEDIUM_RADIATION_RADIUS,
            damage_per_tick: MEDIUM_RADIATION_DAMAGE_PER_TICK,
            activate_frame,
            expires_frame: activate_frame.saturating_add(MEDIUM_RADIATION_DURATION_FRAMES),
            next_tick_frame: activate_frame,
            total_damage_applied: 0.0,
            damage_applications: 0,
            objects_destroyed: 0,
        };
        self.active.push(zone);
        self.radiation_zones_spawned = self.radiation_zones_spawned.saturating_add(1);
        id
    }

    pub fn plan_due_ticks(
        &self,
        current_frame: u32,
        object_positions: &[(ObjectId, Vec3, super::Team, bool)],
    ) -> Vec<HostMediumRadiationTickPlan> {
        let mut plans = Vec::new();
        for zone in &self.active {
            if !zone.is_due_tick(current_frame) {
                continue;
            }
            let mut hits = Vec::new();
            for &(id, pos, _team, alive) in object_positions {
                if !alive || id == zone.source_object {
                    continue;
                }
                let dx = zone.position.x - pos.x;
                let dz = zone.position.z - pos.z;
                let dist = (dx * dx + dz * dz).sqrt();
                if dist <= zone.radius {
                    hits.push(HostMediumRadiationDamageHit {
                        target_id: id,
                        damage: zone.damage_per_tick,
                        zone_id: zone.id,
                    });
                }
            }
            plans.push(HostMediumRadiationTickPlan {
                zone_id: zone.id,
                source_object: zone.source_object,
                source_team: zone.source_team,
                hits,
            });
        }
        plans.sort_by_key(|p| p.zone_id);
        plans
    }

    pub fn record_tick_complete(
        &mut self,
        zone_id: u32,
        total_damage: f32,
        applications: u32,
        objects_destroyed: u32,
        current_frame: u32,
    ) {
        if let Some(zone) = self.active.iter_mut().find(|z| z.id == zone_id) {
            zone.total_damage_applied += total_damage;
            zone.damage_applications += applications;
            zone.objects_destroyed += objects_destroyed;
            zone.next_tick_frame =
                current_frame.saturating_add(MEDIUM_RADIATION_TICK_INTERVAL_FRAMES);
            self.radiation_total_damage += total_damage;
            self.radiation_damage_applications = self
                .radiation_damage_applications
                .saturating_add(applications);
            self.radiation_objects_destroyed = self
                .radiation_objects_destroyed
                .saturating_add(objects_destroyed);
        }
    }

    pub fn prune_expired(&mut self, current_frame: u32) {
        let before = self.active.len();
        self.active.retain(|z| !z.is_expired(current_frame));
        self.radiation_expirations = self
            .radiation_expirations
            .saturating_add((before.saturating_sub(self.active.len())) as u32);
    }

    pub fn honesty_primary_ok(&self) -> bool {
        self.primary_blasts > 0 && self.units_hit > 0
    }

    pub fn honesty_radiation_ok(&self) -> bool {
        self.radiation_zones_spawned > 0
    }

    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_primary_ok() || self.honesty_radiation_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::Team;

    #[test]
    fn primary_gate_and_damage_rings() {
        assert!(should_apply_nuke_cannon_primary(true, 0));
        assert!(!should_apply_nuke_cannon_primary(true, 1));
        assert!(!should_apply_nuke_cannon_primary(false, 0));

        assert!(
            (nuke_cannon_primary_damage_at(0.0) - NUKE_CANNON_PRIMARY_DAMAGE).abs() < 0.01
        );
        assert!(
            (nuke_cannon_primary_damage_at(50.0) - NUKE_CANNON_PRIMARY_DAMAGE).abs() < 0.01
        );
        assert!(
            (nuke_cannon_primary_damage_at(55.0) - NUKE_CANNON_SECONDARY_DAMAGE).abs() < 0.01
        );
        assert!((nuke_cannon_primary_damage_at(61.0)).abs() < 0.01);
        assert!((nuke_cannon_splash_radius() - NUKE_CANNON_SECONDARY_RADIUS).abs() < 0.01);
    }

    #[test]
    fn radiation_registry_spawn_and_tick() {
        let mut reg = HostNukeCannonRegistry::new();
        assert!(!reg.honesty_host_path_ok());
        reg.record_primary_blast(2);
        assert!(reg.honesty_primary_ok());

        let id = reg.spawn_radiation_zone(
            ObjectId(1),
            Team::China,
            Vec3::new(0.0, 0.0, 0.0),
            0,
        );
        assert_eq!(id, 0);
        assert!(reg.honesty_radiation_ok());
        assert_eq!(reg.active_count(), 1);

        let positions = vec![
            (ObjectId(1), Vec3::ZERO, Team::China, true),
            (ObjectId(2), Vec3::new(10.0, 0.0, 0.0), Team::USA, true),
            (ObjectId(3), Vec3::new(200.0, 0.0, 0.0), Team::USA, true),
        ];
        let plans = reg.plan_due_ticks(0, &positions);
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].hits.len(), 1);
        assert_eq!(plans[0].hits[0].target_id, ObjectId(2));
        assert!((plans[0].hits[0].damage - MEDIUM_RADIATION_DAMAGE_PER_TICK).abs() < 0.01);

        reg.record_tick_complete(0, 15.0, 1, 0, 0);
        reg.prune_expired(MEDIUM_RADIATION_DURATION_FRAMES);
        assert_eq!(reg.active_count(), 0);
    }

    #[test]
    fn reexports_nuke_cannon_template() {
        assert!(is_nuke_cannon_template("ChinaVehicleNukeCannon"));
        assert!(!is_nuke_cannon_template("NukeCannonShell"));
    }
}
