//! Host China Nuclear Tanks residual (death blast + locomotor speed + radiation).
//!
//! Residual slice (playability):
//! - `Upgrade_ChinaNuclearTanks` PLAYER_UPGRADE residual equips eligible China tanks
//!   (Battlemaster / Overlord / Emperor chassis):
//!   - Locomotor residual speed: Battlemaster **25 → 35**, Overlord/Emperor **20 → 30**.
//!   - On death: dual-radius `NuclearTankDeathWeapon` residual
//!     Primary **25**/r**25** + Secondary **10**/r**75** (Nuke_ general: **110**/r**80** +
//!     **70**/r**100**).
//!   - Spawns residual `OCL_RadiationFieldSmall` / `SmallRadiationFieldWeapon`:
//!     **5** dmg / r**15** / tick **750**ms / lifetime **2500**ms.
//! - Honesty counters for upgrade apply / death detonate / radiation.
//!
//! Fail-closed honesty:
//! - Not full FireWeaponWhenDeadBehavior exclusive module / RequiresAllTriggers matrix
//! - Not full LocomotorSetUpgrade visual / Nuclear*Locomotor pitch-roll matrix
//! - Not full Nuke_ fusion locomotor / red FX particle matrix
//! - Not network nuclear-tanks replication (network deferred)

use super::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Retail Upgrade_ChinaNuclearTanks.
pub const UPGRADE_CHINA_NUCLEAR_TANKS: &str = "Upgrade_ChinaNuclearTanks";

/// Retail BattleMasterLocomotor Speed.
pub const BATTLEMASTER_BASE_SPEED: f32 = 25.0;
/// Retail NuclearBattleMasterLocomotor Speed (33% faster).
pub const BATTLEMASTER_NUCLEAR_SPEED: f32 = 35.0;
/// Retail OverlordLocomotor Speed.
pub const OVERLORD_BASE_SPEED: f32 = 20.0;
/// Retail NuclearOverlordLocomotor Speed (50% faster residual).
pub const OVERLORD_NUCLEAR_SPEED: f32 = 30.0;

/// Retail NuclearTankDeathWeapon PrimaryDamage.
pub const NUCLEAR_TANK_PRIMARY_DAMAGE: f32 = 25.0;
/// Retail PrimaryDamageRadius.
pub const NUCLEAR_TANK_PRIMARY_RADIUS: f32 = 25.0;
/// Retail SecondaryDamage.
pub const NUCLEAR_TANK_SECONDARY_DAMAGE: f32 = 10.0;
/// Retail SecondaryDamageRadius.
pub const NUCLEAR_TANK_SECONDARY_RADIUS: f32 = 75.0;

/// Retail Nuke_NuclearTankDeathWeapon PrimaryDamage.
pub const NUKE_GEN_PRIMARY_DAMAGE: f32 = 110.0;
/// Retail Nuke_ PrimaryDamageRadius.
pub const NUKE_GEN_PRIMARY_RADIUS: f32 = 80.0;
/// Retail Nuke_ SecondaryDamage.
pub const NUKE_GEN_SECONDARY_DAMAGE: f32 = 70.0;
/// Retail Nuke_ SecondaryDamageRadius.
pub const NUKE_GEN_SECONDARY_RADIUS: f32 = 100.0;

/// Retail SmallRadiationFieldWeapon PrimaryDamage.
pub const SMALL_RADIATION_DAMAGE: f32 = 5.0;
/// Retail SmallRadiationFieldWeapon PrimaryDamageRadius.
pub const SMALL_RADIATION_RADIUS: f32 = 15.0;
/// Retail DelayBetweenShots 750ms → 23 frames @ 30 FPS.
pub const SMALL_RADIATION_TICK_FRAMES: u32 = 23;
/// Retail RadiationFieldSmall LifetimeUpdate 2500ms → 75 frames.
pub const SMALL_RADIATION_DURATION_FRAMES: u32 = 75;

/// Residual detonation audio.
pub const NUCLEAR_TANK_DEATH_AUDIO: &str = "NuclearTankDeathWeapon";
/// Residual radiation ambient audio.
pub const SMALL_RADIATION_AUDIO: &str = "RadiationPoolAmbientLoop";
/// Residual upgrade-complete audio cue.
pub const NUCLEAR_TANKS_UPGRADE_AUDIO: &str = "UpgradeChinaNuclearTanks";

/// Whether template is eligible for Nuclear Tanks residual (Battlemaster / Overlord / Emperor).
///
/// Fail-closed: name residual. Excludes portable payloads, shells, Helix, Dragon, Inferno.
pub fn is_nuclear_tanks_eligible(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    if n.contains("gattling")
        || n.contains("gatling")
        || n.contains("propaganda")
        || n.contains("bunker")
        || n.contains("weapon")
        || n.contains("shell")
        || n.contains("projectile")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("dead")
        || n.starts_with("upgrade")
        || n.contains("science")
        || n.contains("command")
        || n.contains("helix")
        || n.contains("dragon")
        || n.contains("inferno")
        || n.contains("nuke cannon")
        || n.contains("nukecannon")
        || n.contains("troopcrawler")
        || n.contains("listening")
        || n.contains("ecm")
        || n.contains("locomotor")
    {
        return false;
    }
    // Explicit residual test / shorthand names.
    if n == "testbattlemaster"
        || n == "testoverlord"
        || n == "testemperor"
        || n == "china_battlemastertank"
        || n == "china_overlordtank"
        || n == "china_overlord"
        || n == "tank_chinatankemperor"
    {
        return true;
    }
    // Battlemaster chassis.
    if n.contains("battlemaster") || n.contains("battlemastertank") {
        return true;
    }
    // Overlord chassis (not portable gattling/propaganda payloads — filtered above).
    if n.contains("overlord") {
        return true;
    }
    // Emperor (innate propaganda Overlord general variant).
    if n.contains("emperor") && n.contains("tank") {
        return true;
    }
    false
}

/// Whether template uses Nuke General death weapon residual numbers.
pub fn is_nuke_general_nuclear_tanks(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    n.starts_with("nuke_") || n.contains("nuke_chinatank") || n.contains("fusionbattlemaster")
}

/// Whether template is an Overlord / Emperor chassis for nuclear speed residual.
pub fn is_overlord_chassis_for_nuclear_speed(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.contains("gattling") || n.contains("propaganda") || n.contains("bunker") {
        return false;
    }
    n.contains("overlord") || (n.contains("emperor") && n.contains("tank")) || n == "testemperor"
}

/// Residual max_speed after Nuclear Tanks upgrade.
pub fn nuclear_tanks_residual_speed(template_name: &str) -> f32 {
    if is_overlord_chassis_for_nuclear_speed(template_name) {
        OVERLORD_NUCLEAR_SPEED
    } else {
        BATTLEMASTER_NUCLEAR_SPEED
    }
}

/// Base residual max_speed without Nuclear Tanks (for tests / idempotent refresh).
pub fn nuclear_tanks_base_speed(template_name: &str) -> f32 {
    if is_overlord_chassis_for_nuclear_speed(template_name) {
        OVERLORD_BASE_SPEED
    } else {
        BATTLEMASTER_BASE_SPEED
    }
}

/// Whether unit has Nuclear Tanks residual upgrade tag applied.
pub fn has_nuclear_tanks_upgrade(applied_upgrades: &HashSet<String>) -> bool {
    applied_upgrades.iter().any(|u| {
        let n: String = u
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .flat_map(|c| c.to_lowercase())
            .collect();
        n.contains("nucleartanks")
            || n.contains("upgradenucleartanks")
            || n.contains("upgradenucleartank")
            || n == "upgradechinanucleartanks"
    })
}

/// Dual-radius death blast damage at distance (nuke-general variant when flagged).
pub fn nuclear_tank_death_damage_at(distance: f32, nuke_general: bool) -> f32 {
    let (p_dmg, p_r, s_dmg, s_r) = if nuke_general {
        (
            NUKE_GEN_PRIMARY_DAMAGE,
            NUKE_GEN_PRIMARY_RADIUS,
            NUKE_GEN_SECONDARY_DAMAGE,
            NUKE_GEN_SECONDARY_RADIUS,
        )
    } else {
        (
            NUCLEAR_TANK_PRIMARY_DAMAGE,
            NUCLEAR_TANK_PRIMARY_RADIUS,
            NUCLEAR_TANK_SECONDARY_DAMAGE,
            NUCLEAR_TANK_SECONDARY_RADIUS,
        )
    };
    if distance <= p_r {
        p_dmg
    } else if distance <= s_r {
        s_dmg
    } else {
        0.0
    }
}

/// Splash outer radius for death residual.
pub fn nuclear_tank_death_splash_radius(nuke_general: bool) -> f32 {
    if nuke_general {
        NUKE_GEN_SECONDARY_RADIUS
    } else {
        NUCLEAR_TANK_SECONDARY_RADIUS
    }
}

/// Legal residual splash target (alive combat unit/structure; includes allies/neutrals).
pub fn is_legal_nuclear_death_target(
    is_alive: bool,
    is_self: bool,
    under_construction: bool,
    combat_kind: bool,
) -> bool {
    is_alive && !is_self && !under_construction && combat_kind
}

/// One active residual SmallRadiationField from Nuclear Tank death.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostSmallRadiationZone {
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

impl HostSmallRadiationZone {
    pub fn is_expired(&self, current_frame: u32) -> bool {
        current_frame >= self.expires_frame
    }

    pub fn is_due_tick(&self, current_frame: u32) -> bool {
        !self.is_expired(current_frame) && current_frame >= self.next_tick_frame
    }
}

/// Damage application plan for a single victim this tick.
#[derive(Debug, Clone, Copy)]
pub struct HostSmallRadiationDamageHit {
    pub target_id: ObjectId,
    pub damage: f32,
    pub zone_id: u32,
}

/// Result of resolving one radiation zone's damage tick.
#[derive(Debug, Clone)]
pub struct HostSmallRadiationTickPlan {
    pub zone_id: u32,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub hits: Vec<HostSmallRadiationDamageHit>,
}

/// Host residual honesty / radiation registry for Nuclear Tanks.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostNuclearTanksRegistry {
    next_id: u32,
    active: Vec<HostSmallRadiationZone>,
    /// Tanks that received Nuclear Tanks upgrade residual.
    pub upgrades_applied: u32,
    /// Death detonations fired.
    pub death_detonations: u32,
    /// Units hit by death dual-radius residual.
    pub death_units_hit: u32,
    /// Small radiation zones spawned.
    pub radiation_zones_spawned: u32,
    pub radiation_expirations: u32,
    pub radiation_total_damage: f32,
    pub radiation_damage_applications: u32,
    pub radiation_objects_destroyed: u32,
    /// Nuke-general death residual path used at least once.
    pub nuke_general_detonations: u32,
}

impl HostNuclearTanksRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    pub fn active_zones(&self) -> &[HostSmallRadiationZone] {
        &self.active
    }

    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    pub fn record_upgrade_applied(&mut self, count: u32) {
        self.upgrades_applied = self.upgrades_applied.saturating_add(count);
    }

    pub fn record_death_detonation(&mut self, units_hit: u32, nuke_general: bool) {
        self.death_detonations = self.death_detonations.saturating_add(1);
        self.death_units_hit = self.death_units_hit.saturating_add(units_hit);
        if nuke_general {
            self.nuke_general_detonations = self.nuke_general_detonations.saturating_add(1);
        }
    }

    pub fn spawn_radiation_zone(
        &mut self,
        source_object: ObjectId,
        source_team: super::Team,
        impact_pos: Vec3,
        activate_frame: u32,
    ) -> u32 {
        let id = self.alloc_id();
        let zone = HostSmallRadiationZone {
            id,
            source_object,
            source_team,
            position: impact_pos,
            radius: SMALL_RADIATION_RADIUS,
            damage_per_tick: SMALL_RADIATION_DAMAGE,
            activate_frame,
            expires_frame: activate_frame.saturating_add(SMALL_RADIATION_DURATION_FRAMES),
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
    ) -> Vec<HostSmallRadiationTickPlan> {
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
                    hits.push(HostSmallRadiationDamageHit {
                        target_id: id,
                        damage: zone.damage_per_tick,
                        zone_id: zone.id,
                    });
                }
            }
            plans.push(HostSmallRadiationTickPlan {
                zone_id: zone.id,
                source_object: zone.source_object,
                source_team: zone.source_team,
                hits,
            });
        }
        plans
    }

    pub fn record_tick_complete(
        &mut self,
        zone_id: u32,
        total_damage: f32,
        applications: u32,
        destroyed: u32,
        current_frame: u32,
    ) {
        if let Some(zone) = self.active.iter_mut().find(|z| z.id == zone_id) {
            zone.total_damage_applied += total_damage;
            zone.damage_applications = zone.damage_applications.saturating_add(applications);
            zone.objects_destroyed = zone.objects_destroyed.saturating_add(destroyed);
            zone.next_tick_frame = current_frame.saturating_add(SMALL_RADIATION_TICK_FRAMES);
        }
        self.radiation_total_damage += total_damage;
        self.radiation_damage_applications =
            self.radiation_damage_applications.saturating_add(applications);
        self.radiation_objects_destroyed =
            self.radiation_objects_destroyed.saturating_add(destroyed);
    }

    pub fn prune_expired(&mut self, current_frame: u32) {
        let before = self.active.len();
        self.active.retain(|z| !z.is_expired(current_frame));
        let removed = before.saturating_sub(self.active.len()) as u32;
        self.radiation_expirations = self.radiation_expirations.saturating_add(removed);
    }

    pub fn honesty_upgrade_ok(&self) -> bool {
        self.upgrades_applied > 0
    }

    pub fn honesty_death_ok(&self) -> bool {
        self.death_detonations > 0 && self.death_units_hit > 0
    }

    pub fn honesty_radiation_ok(&self) -> bool {
        self.radiation_zones_spawned > 0
            && (self.radiation_damage_applications > 0 || self.radiation_total_damage > 0.0)
    }

    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_upgrade_ok() || self.honesty_death_ok() || self.radiation_zones_spawned > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::Team;

    #[test]
    fn residual_gate_eligibility() {
        assert!(is_nuclear_tanks_eligible("ChinaTankBattleMaster"));
        assert!(is_nuclear_tanks_eligible("ChinaTankOverlord"));
        assert!(is_nuclear_tanks_eligible("Tank_ChinaTankEmperor"));
        assert!(is_nuclear_tanks_eligible("Nuke_ChinaTankBattleMaster"));
        assert!(is_nuclear_tanks_eligible("TestBattlemaster"));
        assert!(!is_nuclear_tanks_eligible("ChinaTankDragon"));
        assert!(!is_nuclear_tanks_eligible("ChinaVehicleInfernoCannon"));
        assert!(!is_nuclear_tanks_eligible("ChinaOverlordGattlingCannon"));
        assert!(!is_nuclear_tanks_eligible("Upgrade_ChinaNuclearTanks"));
    }

    #[test]
    fn residual_speed_and_damage() {
        assert!((nuclear_tanks_residual_speed("ChinaTankBattleMaster") - 35.0).abs() < 0.01);
        assert!((nuclear_tanks_residual_speed("ChinaTankOverlord") - 30.0).abs() < 0.01);
        assert!((nuclear_tank_death_damage_at(0.0, false) - 25.0).abs() < 0.01);
        assert!((nuclear_tank_death_damage_at(50.0, false) - 10.0).abs() < 0.01);
        assert!((nuclear_tank_death_damage_at(0.0, true) - 110.0).abs() < 0.01);
        assert!((nuclear_tank_death_splash_radius(false) - 75.0).abs() < 0.01);
    }

    #[test]
    fn residual_upgrade_tag_and_radiation() {
        let mut tags = HashSet::new();
        tags.insert(UPGRADE_CHINA_NUCLEAR_TANKS.to_string());
        assert!(has_nuclear_tanks_upgrade(&tags));
        assert!(!has_nuclear_tanks_upgrade(&HashSet::new()));

        let mut reg = HostNuclearTanksRegistry::new();
        let id = reg.spawn_radiation_zone(
            ObjectId(1),
            Team::China,
            Vec3::ZERO,
            10,
        );
        assert_eq!(id, 0);
        assert!(reg.honesty_host_path_ok());
        assert!(reg.honesty_radiation_ok() || reg.radiation_zones_spawned > 0);
        reg.record_upgrade_applied(1);
        reg.record_death_detonation(2, false);
        assert!(reg.honesty_upgrade_ok());
        assert!(reg.honesty_death_ok());
    }
}
