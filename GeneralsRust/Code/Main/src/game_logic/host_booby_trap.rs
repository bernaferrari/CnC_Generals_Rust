//! Host GLA Rebel BoobyTrap residual (plant on structure → detonate on capture/death).
//!
//! Residual slice (playability):
//! - `Upgrade_GLAInfantryRebelBoobyTrapAttack` PLAYER_UPGRADE residual unlocks
//!   `SpecialAbilityBoobyTrap` for Rebel infantry.
//! - Plant residual: walk to enemy/ally/neutral structure within StartAbilityRange **5**
//!   → mark structure `OBJECT_STATUS_BOOBY_TRAPPED` (host residual flag).
//! - Reload residual: **7500** ms (**225** frames).
//! - Detonate residual when:
//!   - Enemy (or non-ally) captures / completes capture of the structure, or
//!   - Structure dies, or
//!   - Residual explicit trigger (enter / special-ability on trapped structure).
//! - Detonation: `BoobyTrapDetonationWeapon` dual-radius residual
//!   Primary **200** / (r**5** + geometry) + Secondary **50** / (r**15** + geometry).
//! - Allies of planter do not trigger detonation (C++ checkAndDetonateBoobyTrap).
//!
//! Wave 68 residual pack (retail Weapon.ini / SpecialPower.ini / Upgrade.ini /
//! WeaponObjects.ini / GLAInfantry.ini honesty):
//! - Weapon: Primary **200**/r**5**, Secondary **50**/r**15**, DamageType EXPLOSION,
//!   DeathType EXPLODED; GeometryBasedDamageWeapon/FX residual
//! - Ability: StartAbilityRange **5**, ReloadTime **7500**ms → **225**f,
//!   MaxSpecialObjects **100**, SpecialObjectsPersistent **Yes**
//! - Upgrade: BuildCost **1000**, BuildTime **30**s → **900**f
//! - Object: Vision/Shroud **25**, MaxHealth **1**, KindOf BOOBY_TRAP NO_COLLIDE MINE,
//!   Geometry CYLINDER **8**/ **8**, StealthDelay **0**, InnateStealth **Yes**
//!
//! Fail-closed honesty:
//! - BoobyTrap SpecialObject spawn + MaxSpecialObjects residual closed
//!   (StickyBomb bone matrix / stealth GPU / geometry partition fail-closed)
//! - Not full geometry-based partition iterate FROM_BOUNDINGSPHERE_3D matrix
//! - Not network booby-trap replication (network deferred)

use super::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Logic frames per second (host fixed step).
pub const BOOBY_LOGIC_FPS: f32 = 30.0;

/// Retail Upgrade_GLAInfantryRebelBoobyTrapAttack.
pub const UPGRADE_GLA_REBEL_BOOBY_TRAP: &str = "Upgrade_GLAInfantryRebelBoobyTrapAttack";
/// Retail SpecialAbilityBoobyTrap.
pub const SPECIAL_ABILITY_BOOBY_TRAP: &str = "SpecialAbilityBoobyTrap";
/// Retail SpecialObject name.
pub const BOOBY_TRAP_OBJECT: &str = "BoobyTrap";
/// Retail BoobyTrapDetonationWeapon name residual.
pub const BOOBY_DETONATION_WEAPON: &str = "BoobyTrapDetonationWeapon";

/// Retail BoobyTrapDetonationWeapon PrimaryDamage.
pub const BOOBY_PRIMARY_DAMAGE: f32 = 200.0;
/// Retail PrimaryDamageRadius (added past bounding circle).
pub const BOOBY_PRIMARY_RADIUS: f32 = 5.0;
/// Retail SecondaryDamage.
pub const BOOBY_SECONDARY_DAMAGE: f32 = 50.0;
/// Retail SecondaryDamageRadius (added past bounding circle).
pub const BOOBY_SECONDARY_RADIUS: f32 = 15.0;
/// Retail DamageType residual.
pub const BOOBY_DAMAGE_TYPE: &str = "EXPLOSION";
/// Retail DeathType residual.
pub const BOOBY_DEATH_TYPE: &str = "EXPLODED";
/// Retail StartAbilityRange.
pub const BOOBY_START_ABILITY_RANGE: f32 = 5.0;
/// Retail SpecialPower ReloadTime residual (msec).
pub const BOOBY_RELOAD_MS: u32 = 7500;
/// Retail SpecialPower ReloadTime 7500ms → 225 frames @ 30 FPS.
pub const BOOBY_RELOAD_FRAMES: u32 = 225;
/// C++ BOOBY_TRAP_SCAN_RANGE residual honesty (Object VisionRange).
pub const BOOBY_TRAP_SCAN_RANGE: f32 = 25.0;
/// Retail ShroudClearingRange residual.
pub const BOOBY_TRAP_SHROUD_CLEARING_RANGE: f32 = 25.0;
/// Retail MaxSpecialObjects residual.
pub const BOOBY_MAX_SPECIAL_OBJECTS: u32 = 100;
/// Retail SpecialObjectsPersistent residual.
pub const BOOBY_SPECIAL_OBJECTS_PERSISTENT: bool = true;
/// Retail SpecialObjectsPersistWhenOwnerDies residual.
pub const BOOBY_SPECIAL_OBJECTS_PERSIST_WHEN_OWNER_DIES: bool = true;
/// Retail PreparationTime residual (msec).
pub const BOOBY_PREPARATION_TIME_MS: u32 = 0;

/// Residual plant audio (StickyBombCreated / InitiateSound residual).
pub const BOOBY_TRAP_INSTALL_AUDIO: &str = "BoobyTrapInstall";
/// Residual detonation audio / FX residual cue.
pub const BOOBY_TRAP_DETONATE_AUDIO: &str = "FX_BoobyTrapExplosion";
/// Retail GeometryBasedDamageWeapon residual.
pub const BOOBY_GEOMETRY_DAMAGE_WEAPON: &str = "BoobyTrapDetonationWeapon";
/// Retail GeometryBasedDamageFX residual.
pub const BOOBY_GEOMETRY_DAMAGE_FX: &str = "FX_BoobyTrapExplosion";

/// Retail Upgrade BuildCost residual.
pub const BOOBY_UPGRADE_BUILD_COST: u32 = 1000;
/// Retail Upgrade BuildTime residual (seconds).
pub const BOOBY_UPGRADE_BUILD_TIME_SEC: f32 = 30.0;
/// Upgrade BuildTime 30s → 900 frames @ 30 FPS.
pub const BOOBY_UPGRADE_BUILD_TIME_FRAMES: u32 = 900;
/// Retail ResearchSound residual.
pub const BOOBY_UPGRADE_RESEARCH_SOUND: &str = "RebelVoiceUpgradeBoobyTrap";
/// Retail InitiateSound residual on SpecialAbility.
pub const BOOBY_INITIATE_SOUND: &str = "RebelVoiceBoobyTrapInstall";

/// Retail BoobyTrap Object MaxHealth residual.
pub const BOOBY_TRAP_MAX_HEALTH: f32 = 1.0;
/// Retail KindOf residual tokens.
pub const BOOBY_TRAP_KIND_OF: &str = "BOOBY_TRAP NO_COLLIDE MINE";
/// Retail Geometry major radius residual.
pub const BOOBY_TRAP_GEOMETRY_MAJOR_RADIUS: f32 = 8.0;
/// Retail Geometry height residual.
pub const BOOBY_TRAP_GEOMETRY_HEIGHT: f32 = 8.0;
/// Retail StealthDelay residual (msec).
pub const BOOBY_TRAP_STEALTH_DELAY_MS: u32 = 0;
/// Retail InnateStealth residual.
pub const BOOBY_TRAP_INNATE_STEALTH: bool = true;
/// Retail FriendlyOpacityMin residual (percent).
pub const BOOBY_TRAP_FRIENDLY_OPACITY_MIN_PERCENT: f32 = 50.0;
/// Retail Physics Mass residual.
pub const BOOBY_TRAP_PHYSICS_MASS: f32 = 5.0;
/// Retail SpecialPower Enum residual.
pub const BOOBY_SPECIAL_ENUM: &str = "SPECIAL_BOOBY_TRAP";

/// Convert residual milliseconds to logic frames @ 30 FPS.
pub fn booby_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) / (1000.0 / BOOBY_LOGIC_FPS)).round() as u32
}

/// Active residual plant on a structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostBoobyTrapPlant {
    pub structure_id: ObjectId,
    pub planter_id: ObjectId,
    pub planter_team: super::Team,
    pub plant_frame: u32,
    /// Residual geometry radius used at detonation (selection_radius residual).
    pub geometry_radius: f32,
    /// C++ SpecialObject BoobyTrap Thing id residual.
    #[serde(default)]
    pub charge_object_id: Option<ObjectId>,
}

/// Host residual honesty registry for BoobyTrap plant / detonate.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostBoobyTrapRegistry {
    /// structure_id → plant residual.
    plants: HashMap<u32, HostBoobyTrapPlant>,
    /// Last plant frame per rebel (reload residual).
    last_plant_frame: HashMap<u32, u32>,
    pub plants_total: u32,
    pub detonations_total: u32,
    pub units_hit_total: u32,
    pub capture_triggers: u32,
    pub death_triggers: u32,
    pub upgrades_applied: u32,
}

impl HostBoobyTrapRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn active_count(&self) -> usize {
        self.plants.len()
    }

    pub fn plant(&self, structure_id: ObjectId) -> Option<&HostBoobyTrapPlant> {
        self.plants.get(&structure_id.0)
    }

    pub fn is_booby_trapped(&self, structure_id: ObjectId) -> bool {
        self.plants.contains_key(&structure_id.0)
    }

    pub fn record_upgrade_applied(&mut self, count: u32) {
        self.upgrades_applied = self.upgrades_applied.saturating_add(count);
    }

    /// Whether rebel may plant (reload residual).
    pub fn plant_ready(&self, planter_id: ObjectId, current_frame: u32) -> bool {
        match self.last_plant_frame.get(&planter_id.0) {
            Some(&last) => current_frame.saturating_sub(last) >= BOOBY_RELOAD_FRAMES,
            None => true,
        }
    }

    /// Install residual plant. Returns previous plant if structure was already trapped.
    pub fn install(
        &mut self,
        structure_id: ObjectId,
        planter_id: ObjectId,
        planter_team: super::Team,
        plant_frame: u32,
        geometry_radius: f32,
        charge_object_id: Option<ObjectId>,
    ) -> Option<HostBoobyTrapPlant> {
        let prev = self.plants.remove(&structure_id.0);
        self.plants.insert(
            structure_id.0,
            HostBoobyTrapPlant {
                structure_id,
                planter_id,
                planter_team,
                plant_frame,
                geometry_radius: geometry_radius.max(1.0),
                charge_object_id,
            },
        );
        self.last_plant_frame.insert(planter_id.0, plant_frame);
        self.plants_total = self.plants_total.saturating_add(1);
        prev
    }

    /// Active SpecialObject count for MaxSpecialObjects residual.
    pub fn active_special_objects_for_planter(&self, planter_id: ObjectId) -> u32 {
        self.plants
            .values()
            .filter(|p| p.planter_id == planter_id && p.charge_object_id.is_some())
            .count() as u32
    }

    /// Whether planter may place another BoobyTrap (MaxSpecialObjects residual).
    pub fn can_place_special_object(&self, planter_id: ObjectId) -> bool {
        self.active_special_objects_for_planter(planter_id) < BOOBY_MAX_SPECIAL_OBJECTS
    }

    /// Bind charge object id after spawn residual.
    pub fn set_charge_object(&mut self, structure_id: ObjectId, charge_id: ObjectId) {
        if let Some(p) = self.plants.get_mut(&structure_id.0) {
            p.charge_object_id = Some(charge_id);
        }
    }

    /// Take plant for detonation (clears residual).
    pub fn take_plant(&mut self, structure_id: ObjectId) -> Option<HostBoobyTrapPlant> {
        self.plants.remove(&structure_id.0)
    }

    pub fn forget_structure(&mut self, structure_id: ObjectId) {
        self.plants.remove(&structure_id.0);
    }

    pub fn record_detonation(&mut self, units_hit: u32, via_capture: bool, via_death: bool) {
        self.detonations_total = self.detonations_total.saturating_add(1);
        self.units_hit_total = self.units_hit_total.saturating_add(units_hit);
        if via_capture {
            self.capture_triggers = self.capture_triggers.saturating_add(1);
        }
        if via_death {
            self.death_triggers = self.death_triggers.saturating_add(1);
        }
    }

    pub fn honesty_plant_ok(&self) -> bool {
        self.plants_total > 0
    }

    pub fn honesty_detonate_ok(&self) -> bool {
        self.detonations_total > 0 && self.units_hit_total > 0
    }

    pub fn honesty_upgrade_ok(&self) -> bool {
        self.upgrades_applied > 0
    }

    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_plant_ok() || self.honesty_detonate_ok() || self.honesty_upgrade_ok()
    }
}

/// Whether template is a residual Rebel that can plant BoobyTrap after upgrade.
///
/// Fail-closed: name residual (not full CommandSet / UnpauseSpecialPowerUpgrade matrix).
pub fn is_booby_trap_planter_template(template_name: &str) -> bool {
    crate::game_logic::host_gla_rebel::is_gla_rebel_template(template_name)
}

/// Whether unit has BoobyTrap residual upgrade tag.
pub fn has_booby_trap_upgrade(applied_upgrades: &std::collections::HashSet<String>) -> bool {
    applied_upgrades.iter().any(|u| {
        let n: String = u
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .flat_map(|c| c.to_lowercase())
            .collect();
        n.contains("boobytrap")
            || n.contains("rebelbooby")
            || n == "upgradeglainfantryrebelboobytrapattack"
    })
}

/// Dual-radius detonation damage at distance (geometry radius already added to rings).
pub fn booby_trap_damage_at(distance: f32, geometry_radius: f32) -> f32 {
    let primary_r = BOOBY_PRIMARY_RADIUS + geometry_radius;
    let secondary_r = BOOBY_SECONDARY_RADIUS + geometry_radius;
    if distance <= primary_r {
        BOOBY_PRIMARY_DAMAGE
    } else if distance <= secondary_r {
        BOOBY_SECONDARY_DAMAGE
    } else {
        0.0
    }
}

/// Outer splash radius including geometry residual.
pub fn booby_trap_splash_radius(geometry_radius: f32) -> f32 {
    BOOBY_SECONDARY_RADIUS + geometry_radius
}

/// 2D distance residual.
pub fn distance_2d(ax: f32, az: f32, bx: f32, bz: f32) -> f32 {
    let dx = ax - bx;
    let dz = az - bz;
    (dx * dx + dz * dz).sqrt()
}

/// Legal residual detonation victim (alive combat kind; not structure self when dead-path).
pub fn is_legal_booby_victim(
    is_alive: bool,
    is_structure_self: bool,
    under_construction: bool,
    combat_kind: bool,
) -> bool {
    is_alive && !is_structure_self && !under_construction && combat_kind
}

/// Whether trigger unit is an ally of planter (should NOT detonate).
pub fn is_planter_ally(planter_team: super::Team, trigger_team: super::Team) -> bool {
    planter_team == trigger_team
}

/// Whether plant reload residual is ready.
pub fn booby_trap_ready(current_frame: u32, last_plant_frame: Option<u32>) -> bool {
    match last_plant_frame {
        Some(last) => current_frame.saturating_sub(last) >= BOOBY_RELOAD_FRAMES,
        None => true,
    }
}

/// Residual plant range check (StartAbilityRange + radii pad).
pub fn in_plant_range(
    planter_pos: Vec3,
    target_pos: Vec3,
    planter_radius: f32,
    target_radius: f32,
) -> bool {
    let dist = distance_2d(planter_pos.x, planter_pos.z, target_pos.x, target_pos.z);
    dist <= BOOBY_START_ABILITY_RANGE + planter_radius + target_radius
}

// --- Wave 68 residual honesty packs ---

pub fn honesty_booby_trap_weapon_residual_ok() -> bool {
    BOOBY_DETONATION_WEAPON == "BoobyTrapDetonationWeapon"
        && (BOOBY_PRIMARY_DAMAGE - 200.0).abs() < 0.01
        && (BOOBY_PRIMARY_RADIUS - 5.0).abs() < 0.01
        && (BOOBY_SECONDARY_DAMAGE - 50.0).abs() < 0.01
        && (BOOBY_SECONDARY_RADIUS - 15.0).abs() < 0.01
        && BOOBY_DAMAGE_TYPE == "EXPLOSION"
        && BOOBY_DEATH_TYPE == "EXPLODED"
        && BOOBY_GEOMETRY_DAMAGE_WEAPON == "BoobyTrapDetonationWeapon"
        && BOOBY_GEOMETRY_DAMAGE_FX == "FX_BoobyTrapExplosion"
        && BOOBY_TRAP_DETONATE_AUDIO == "FX_BoobyTrapExplosion"
}

pub fn honesty_booby_trap_ability_residual_ok() -> bool {
    SPECIAL_ABILITY_BOOBY_TRAP == "SpecialAbilityBoobyTrap"
        && BOOBY_TRAP_OBJECT == "BoobyTrap"
        && (BOOBY_START_ABILITY_RANGE - 5.0).abs() < 0.01
        && BOOBY_RELOAD_MS == 7500
        && BOOBY_RELOAD_FRAMES == booby_ms_to_frames(BOOBY_RELOAD_MS)
        && BOOBY_MAX_SPECIAL_OBJECTS == 100
        && BOOBY_SPECIAL_OBJECTS_PERSISTENT
        && BOOBY_SPECIAL_OBJECTS_PERSIST_WHEN_OWNER_DIES
        && BOOBY_PREPARATION_TIME_MS == 0
        && BOOBY_SPECIAL_ENUM == "SPECIAL_BOOBY_TRAP"
        && BOOBY_INITIATE_SOUND == "RebelVoiceBoobyTrapInstall"
        && BOOBY_TRAP_INSTALL_AUDIO == "BoobyTrapInstall"
}

pub fn honesty_booby_trap_upgrade_residual_ok() -> bool {
    UPGRADE_GLA_REBEL_BOOBY_TRAP == "Upgrade_GLAInfantryRebelBoobyTrapAttack"
        && BOOBY_UPGRADE_BUILD_COST == 1000
        && (BOOBY_UPGRADE_BUILD_TIME_SEC - 30.0).abs() < 0.01
        && BOOBY_UPGRADE_BUILD_TIME_FRAMES
            == (BOOBY_UPGRADE_BUILD_TIME_SEC * BOOBY_LOGIC_FPS).round() as u32
        && BOOBY_UPGRADE_RESEARCH_SOUND == "RebelVoiceUpgradeBoobyTrap"
}

pub fn honesty_booby_trap_object_residual_ok() -> bool {
    (BOOBY_TRAP_SCAN_RANGE - 25.0).abs() < 0.01
        && (BOOBY_TRAP_SHROUD_CLEARING_RANGE - 25.0).abs() < 0.01
        && (BOOBY_TRAP_MAX_HEALTH - 1.0).abs() < 0.01
        && BOOBY_TRAP_KIND_OF == "BOOBY_TRAP NO_COLLIDE MINE"
        && (BOOBY_TRAP_GEOMETRY_MAJOR_RADIUS - 8.0).abs() < 0.01
        && (BOOBY_TRAP_GEOMETRY_HEIGHT - 8.0).abs() < 0.01
        && BOOBY_TRAP_STEALTH_DELAY_MS == 0
        && BOOBY_TRAP_INNATE_STEALTH
        && (BOOBY_TRAP_FRIENDLY_OPACITY_MIN_PERCENT - 50.0).abs() < 0.01
        && (BOOBY_TRAP_PHYSICS_MASS - 5.0).abs() < 0.01
}

pub fn honesty_booby_trap_residual_pack_ok() -> bool {
    honesty_booby_trap_weapon_residual_ok()
        && honesty_booby_trap_ability_residual_ok()
        && honesty_booby_trap_upgrade_residual_ok()
        && honesty_booby_trap_object_residual_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::Team;
    use std::collections::HashSet;

    #[test]
    fn residual_gate_planter_and_upgrade() {
        assert!(is_booby_trap_planter_template("GLAInfantryRebel"));
        assert!(is_booby_trap_planter_template("Demo_GLAInfantryRebel"));
        assert!(!is_booby_trap_planter_template("GLAInfantryTerrorist"));
        let mut tags = HashSet::new();
        tags.insert(UPGRADE_GLA_REBEL_BOOBY_TRAP.to_string());
        assert!(has_booby_trap_upgrade(&tags));
        assert!(!has_booby_trap_upgrade(&HashSet::new()));
    }

    #[test]
    fn residual_damage_and_reload() {
        assert!((booby_trap_damage_at(0.0, 10.0) - 200.0).abs() < 0.01);
        // primary_r = 5+10=15, secondary_r=15+10=25
        assert!((booby_trap_damage_at(20.0, 10.0) - 50.0).abs() < 0.01);
        assert!((booby_trap_damage_at(30.0, 10.0)).abs() < 0.01);
        assert!(booby_trap_ready(0, None));
        assert!(!booby_trap_ready(100, Some(0)));
        assert!(booby_trap_ready(225, Some(0)));
        assert!(is_planter_ally(Team::GLA, Team::GLA));
        assert!(!is_planter_ally(Team::GLA, Team::USA));
    }

    #[test]
    fn residual_registry_plant_and_detonate() {
        let mut reg = HostBoobyTrapRegistry::new();
        reg.install(ObjectId(10), ObjectId(1), Team::GLA, 5, 12.0, Some(ObjectId(99)));
        assert!(reg.can_place_special_object(ObjectId(1))); // 1 < 100
        assert_eq!(reg.active_special_objects_for_planter(ObjectId(1)), 1);
        assert!(reg.is_booby_trapped(ObjectId(10)));
        assert!(reg.honesty_plant_ok());
        let plant = reg.take_plant(ObjectId(10)).expect("plant");
        assert_eq!(plant.planter_team, Team::GLA);
        assert!((plant.geometry_radius - 12.0).abs() < 0.01);
        reg.record_detonation(3, true, false);
        assert!(reg.honesty_detonate_ok());
        assert_eq!(reg.capture_triggers, 1);
    }

    #[test]
    fn booby_trap_residual_pack_honesty() {
        assert_eq!(booby_ms_to_frames(7500), 225);
        assert_eq!(booby_ms_to_frames(0), 0);
        assert!(honesty_booby_trap_weapon_residual_ok());
        assert!(honesty_booby_trap_ability_residual_ok());
        assert!(honesty_booby_trap_upgrade_residual_ok());
        assert!(honesty_booby_trap_object_residual_ok());
        assert!(honesty_booby_trap_residual_pack_ok());
    }
}
