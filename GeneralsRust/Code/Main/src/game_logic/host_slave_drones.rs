//! Host America Scout / Hellfire / Battle slave-drone residual.
//!
//! Residual slice (playability):
//! - **Scout Drone** (`AmericaVehicleScoutDrone`): always stealth detector residual
//!   (`StealthDetectorUpdate`; DetectionRange unset → VisionRange = **150**).
//!   No primary weapon (sensor drone).
//! - **Hellfire Drone** (`AmericaVehicleHellfireDrone`): PRIMARY
//!   `HellfireMissileWeapon` (40 dmg / 150 range / ~3s clip cycle) with
//!   AutoAcquireEnemiesWhenIdle residual auto-fire.
//! - **Battle Drone** (`AmericaVehicleBattleDrone`): PRIMARY
//!   `BattleDroneMachineGun` (1 dmg / 110 range / 100ms) with idle auto-fire +
//!   master repair residual (weld SM, **12** closeEnough, **10** HP/s).
//! - Master attach residual: spawn drone near a Humvee/compatible vehicle and
//!   tag master with the object-upgrade residual (`Upgrade_AmericaScoutDrone` /
//!   `Upgrade_AmericaHellfireDrone` / `Upgrade_AmericaBattleDrone`).
//!
//! Wave 61 residual pack (retail AmericaVehicle.ini / OCL / Weapon.ini honesty):
//! - SlavedUpdate wander residual (shared Scout/Hellfire/Battle): GuardMax **35**,
//!   GuardWander **35**, AttackRange **75**, AttackWander **10**, ScoutRange **75**,
//!   ScoutWander **10**, StayOnSameLayerAsMaster **Yes**
//! - Scout DistToTargetToGrantRangeBonus **20**; DetectionRate **500**ms → **15**f
//! - Spawn residual: OCL_AmericanScoutDrone Offset **X:-8 Y:0 Z:10**,
//!   Battle/Hellfire Offset **X:0 Y:0 Z:10**, Disposition LIKE_EXISTING, Count **1**
//! - Upgrade residual: Scout **100**, Battle **300**, Hellfire **500** BuildCost,
//!   BuildTime **5**s, DroneArmor **500**/40s
//! - Repair residual (Battle only): C++ `doRepairLogic` weld close-enough **12**
//!   (`distSqr < 12*12`), RepairRange **8** wander, Min/MaxAltitude **18/24**,
//!   RepairRatePerSecond **10**, RepairWhenBelowHealth% **60**, Ready **300–750**ms,
//!   Weld **250–500**ms, RepairWeldingSys **BlueSparks**
//! - Body residual: MaxHealth **100** all; DroneArmor +**25** Scout/Hellfire, +**50** Battle
//! - Hellfire: dmg **40**/r**5**/range **150**, Delay **1000**ms, ClipReload **2000**ms,
//!   ClipSize **1**, Projectile **MissileDefenderMissile**
//!
//! Fail-closed honesty:
//! - Hellfire ScatterRadiusVsInfantry **10** residual miss cone closed (deterministic aim offset)
//! - Not full SlavedUpdate AI wander pathfinding / master layer lock beyond residual flags
//! - ObjectCreationUpgrade ConflictsWith mux: Scout/Battle/Hellfire pairwise exclusive
//!   (leftover UpgradeMux::wouldUpgrade already matches C++; host objects have no leftover
//!   modules, so live queue/giveUpgrade uses the retail name residual)
//! - Not full drone armor MaxHealthUpgrade / death OCL explode matrix beyond residual constants
//! - Not full Battle Drone arm pack/unpack weld FX anim interleave (duty cycle is live)
//! - Not network drone / upgrade replication (network deferred)

use super::Weapon;
use glam::Vec3;

/// Logic frames per second residual.
pub const SLAVE_DRONE_LOGIC_FPS: f32 = 30.0;

/// Retail object-upgrade names.
pub const UPGRADE_AMERICA_SCOUT_DRONE: &str = "Upgrade_AmericaScoutDrone";
pub const UPGRADE_AMERICA_HELLFIRE_DRONE: &str = "Upgrade_AmericaHellfireDrone";
pub const UPGRADE_AMERICA_BATTLE_DRONE: &str = "Upgrade_AmericaBattleDrone";
/// Retail global drone armor upgrade residual.
pub const UPGRADE_AMERICA_DRONE_ARMOR: &str = "Upgrade_AmericaDroneArmor";

/// Retail drone template names.
pub const SCOUT_DRONE_TEMPLATE: &str = "AmericaVehicleScoutDrone";
pub const HELLFIRE_DRONE_TEMPLATE: &str = "AmericaVehicleHellfireDrone";
pub const BATTLE_DRONE_TEMPLATE: &str = "AmericaVehicleBattleDrone";

/// Retail OCL residual names.
pub const OCL_AMERICAN_SCOUT_DRONE: &str = "OCL_AmericanScoutDrone";
pub const OCL_AMERICAN_BATTLE_DRONE: &str = "OCL_AmericanBattleDrone";
pub const OCL_AMERICAN_HELLFIRE_DRONE: &str = "OCL_AmericanHellfireDrone";

/// Retail HellfireMissileWeapon primary name.
pub const HELLFIRE_MISSILE_WEAPON: &str = "HellfireMissileWeapon";
/// Retail BattleDroneMachineGun primary name.
pub const BATTLE_DRONE_MACHINE_GUN: &str = "BattleDroneMachineGun";
/// Retail Hellfire projectile residual.
pub const HELLFIRE_PROJECTILE: &str = "MissileDefenderMissile";

/// Scout VisionRange residual (DetectionRange unset → vision).
pub const SCOUT_DETECTION_RANGE: f32 = 150.0;
/// Scout ShroudClearingRange residual.
pub const SCOUT_SHROUD_CLEARING_RANGE: f32 = 500.0;
/// Scout DetectionRate residual (msec).
pub const SCOUT_DETECTION_RATE_MS: u32 = 500;
/// DetectionRate 500ms → 15 frames @ 30 FPS.
pub const SCOUT_DETECTION_RATE_FRAMES: u32 = 15;

/// Hellfire PrimaryDamage / AttackRange / cycle.
pub const HELLFIRE_DAMAGE: f32 = 40.0;
pub const HELLFIRE_PRIMARY_RADIUS: f32 = 5.0;
pub const HELLFIRE_SCATTER_VS_INFANTRY: f32 = 10.0;
pub const HELLFIRE_RANGE: f32 = 150.0;
/// DelayBetweenShots 1000ms residual.
pub const HELLFIRE_DELAY_MS: u32 = 1000;
/// ClipReloadTime 2000ms residual.
pub const HELLFIRE_CLIP_RELOAD_MS: u32 = 2000;
/// ClipSize residual.
pub const HELLFIRE_CLIP_SIZE: u32 = 1;
/// DelayBetweenShots 1000ms + ClipReload 2000ms (ClipSize 1) ≈ 3s → 90 frames.
pub const HELLFIRE_CYCLE_FRAMES: u32 = 90;
/// Hellfire VisionRange residual.
pub const HELLFIRE_VISION_RANGE: f32 = 100.0;
/// Hellfire ShroudClearingRange residual.
pub const HELLFIRE_SHROUD_CLEARING_RANGE: f32 = 500.0;

/// Battle Drone MachineGun PrimaryDamage residual.
pub const BATTLE_DRONE_GUN_DAMAGE: f32 = 1.0;
/// Retail Battle Drone MachineGun DamageType residual.
pub const BATTLE_DRONE_GUN_DAMAGE_TYPE: &str = "SMALL_ARMS";
/// Retail Battle Drone MachineGun DeathType residual.
pub const BATTLE_DRONE_GUN_DEATH_TYPE: &str = "NORMAL";
/// Battle Drone MachineGun AttackRange residual.
pub const BATTLE_DRONE_GUN_RANGE: f32 = 110.0;
/// Battle Drone DelayBetweenShots 100ms residual.
pub const BATTLE_DRONE_GUN_DELAY_MS: u32 = 100;
/// Battle Drone DelayBetweenShots 100ms → 3 frames @ 30 FPS.
pub const BATTLE_DRONE_GUN_DELAY_FRAMES: u32 = 3;
/// Battle Drone VisionRange residual.
pub const BATTLE_DRONE_VISION_RANGE: f32 = 150.0;
/// Battle Drone ShroudClearingRange residual.
pub const BATTLE_DRONE_SHROUD_CLEARING_RANGE: f32 = 150.0;
/// Battle Drone PLAYER_UPGRADE DAMAGE residual mult.
pub const BATTLE_DRONE_AP_DAMAGE_MULT: f32 = 1.25;

/// Retail SlavedUpdate RepairRatePerSecond residual.
pub const BATTLE_DRONE_REPAIR_RATE_PER_SEC: f32 = 10.0;
/// Retail RepairWhenBelowHealth% residual.
pub const BATTLE_DRONE_REPAIR_BELOW_HEALTH_PCT: f32 = 60.0;
/// Retail RepairRange residual (wander offset in `moveToNewRepairSpot`).
pub const BATTLE_DRONE_REPAIR_RANGE: f32 = 8.0;
/// C++ `SlavedUpdate::doRepairLogic` (`SlavedUpdate.cpp:426`) heal/weld band.
/// `distSqr < 12.0f * 12.0f` — not INI RepairRange (8) and not the old 48 pad.
pub const BATTLE_DRONE_WELD_CLOSE_ENOUGH: f32 = 12.0;
/// C++ `setRepairState(UNPACKING/PACKING)` arm frames.
pub const BATTLE_DRONE_WELD_UNPACK_FRAMES: i32 = 15;
/// C++ `setRepairState` EXTENDING / RETRACTING arm frames.
pub const BATTLE_DRONE_WELD_ARM_FRAMES: i32 = 5;
/// Retail RepairMinAltitude residual.
pub const BATTLE_DRONE_REPAIR_MIN_ALTITUDE: f32 = 18.0;
/// Retail RepairMaxAltitude residual.
pub const BATTLE_DRONE_REPAIR_MAX_ALTITUDE: f32 = 24.0;
/// Retail RepairMinReadyTime residual (msec).
pub const BATTLE_DRONE_REPAIR_MIN_READY_MS: u32 = 300;
/// Retail RepairMaxReadyTime residual (msec).
pub const BATTLE_DRONE_REPAIR_MAX_READY_MS: u32 = 750;
/// Retail RepairMinWeldTime residual (msec).
pub const BATTLE_DRONE_REPAIR_MIN_WELD_MS: u32 = 250;
/// Retail RepairMaxWeldTime residual (msec).
pub const BATTLE_DRONE_REPAIR_MAX_WELD_MS: u32 = 500;
/// Retail RepairWeldingSys residual.
pub const BATTLE_DRONE_REPAIR_WELDING_SYS: &str = "BlueSparks";
/// Retail RepairWeldingFXBone residual.
pub const BATTLE_DRONE_REPAIR_WELDING_FX_BONE: &str = "Muzzle02";
/// MiscAudio.ini slot; queue `resolve_misc_audio_event` playable name, not this token.
pub const BATTLE_DRONE_REPAIR_SPARKS_AUDIO: &str = "RepairSparks";

// --- Wave 61 SlavedUpdate wander residual (shared Scout/Hellfire/Battle) ---

/// Retail GuardMaxRange residual.
pub const SLAVE_GUARD_MAX_RANGE: f32 = 35.0;
/// Retail GuardWanderRange residual.
pub const SLAVE_GUARD_WANDER_RANGE: f32 = 35.0;
/// Retail AttackRange (from master while master attacks) residual.
pub const SLAVE_ATTACK_RANGE: f32 = 75.0;
/// Retail AttackWanderRange residual.
pub const SLAVE_ATTACK_WANDER_RANGE: f32 = 10.0;
/// Retail ScoutRange residual.
pub const SLAVE_SCOUT_RANGE: f32 = 75.0;
/// Retail ScoutWanderRange residual.
pub const SLAVE_SCOUT_WANDER_RANGE: f32 = 10.0;
/// Retail StayOnSameLayerAsMaster residual.
pub const SLAVE_STAY_ON_SAME_LAYER: bool = true;
/// Scout DistToTargetToGrantRangeBonus residual.
pub const SCOUT_DIST_TO_TARGET_RANGE_BONUS: f32 = 20.0;
/// Retail GameData.ini WeaponBonus DRONE_SPOTTING RANGE **150%** (Scout range-extend).
pub const DRONE_SPOTTING_RANGE_MULT: f32 = 1.50;

/// C++ `SpawnBehavior::computeAggregateStates` (SpawnBehavior.cpp:888-897):
/// whoever has the higher veterancy, set the other to that level.
pub fn synced_spawn_veterancy(
    master: crate::game_logic::VeterancyLevel,
    spawn: crate::game_logic::VeterancyLevel,
) -> (
    crate::game_logic::VeterancyLevel,
    crate::game_logic::VeterancyLevel,
) {
    use crate::game_logic::host_unit_training::veterancy_rank;
    let high = if veterancy_rank(spawn) > veterancy_rank(master) {
        spawn
    } else {
        master
    };
    (high, high)
}

// --- Wave 61 body / spawn / upgrade residual ---

/// Retail MaxHealth residual (all three drones).
pub const SLAVE_DRONE_MAX_HEALTH: f32 = 100.0;
/// Scout/Hellfire DroneArmor AddMaxHealth residual.
pub const SCOUT_HELLFIRE_DRONE_ARMOR_ADD: f32 = 25.0;
/// Battle DroneArmor AddMaxHealth residual.
pub const BATTLE_DRONE_ARMOR_ADD: f32 = 50.0;
/// Scout object-upgrade BuildCost residual.
pub const SCOUT_UPGRADE_BUILD_COST: u32 = 100;
/// Battle object-upgrade BuildCost residual.
pub const BATTLE_UPGRADE_BUILD_COST: u32 = 300;
/// Hellfire object-upgrade BuildCost residual.
pub const HELLFIRE_UPGRADE_BUILD_COST: u32 = 500;
/// Object-upgrade BuildTime residual (seconds).
pub const DRONE_UPGRADE_BUILD_TIME: f32 = 5.0;
/// DroneArmor upgrade BuildCost residual.
pub const DRONE_ARMOR_BUILD_COST: u32 = 500;
/// DroneArmor upgrade BuildTime residual (seconds).
pub const DRONE_ARMOR_BUILD_TIME: f32 = 40.0;
/// OCL spawn Z residual (all three).
pub const DRONE_SPAWN_OFFSET_Z: f32 = 10.0;
/// Scout OCL spawn X residual.
pub const SCOUT_SPAWN_OFFSET_X: f32 = -8.0;
/// Battle/Hellfire OCL spawn X residual.
pub const BATTLE_HELLFIRE_SPAWN_OFFSET_X: f32 = 0.0;
/// OCL Count residual.
pub const DRONE_SPAWN_COUNT: u32 = 1;
/// OCL Disposition residual.
pub const DRONE_SPAWN_DISPOSITION: &str = "LIKE_EXISTING";

/// Residual audio.
pub const HELLFIRE_FIRE_AUDIO: &str = "MissileDefenderWeapon";
pub const BATTLE_DRONE_FIRE_AUDIO: &str = "BattleDroneWeapon";

/// Convert msec residual → logic frames @ 30 FPS.
pub fn slave_drone_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * SLAVE_DRONE_LOGIC_FPS / 1000.0).round() as u32
}

/// Which slave drone residual is being attached / spawned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SlaveDroneKind {
    Scout,
    Hellfire,
    Battle,
}

impl SlaveDroneKind {
    pub fn upgrade_name(self) -> &'static str {
        match self {
            SlaveDroneKind::Scout => UPGRADE_AMERICA_SCOUT_DRONE,
            SlaveDroneKind::Hellfire => UPGRADE_AMERICA_HELLFIRE_DRONE,
            SlaveDroneKind::Battle => UPGRADE_AMERICA_BATTLE_DRONE,
        }
    }

    pub fn template_name(self) -> &'static str {
        match self {
            SlaveDroneKind::Scout => SCOUT_DRONE_TEMPLATE,
            SlaveDroneKind::Hellfire => HELLFIRE_DRONE_TEMPLATE,
            SlaveDroneKind::Battle => BATTLE_DRONE_TEMPLATE,
        }
    }

    pub fn from_upgrade_name(name: &str) -> Option<Self> {
        let n = name
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .flat_map(|c| c.to_lowercase())
            .collect::<String>();
        if n.contains("hellfire") {
            Some(SlaveDroneKind::Hellfire)
        } else if n.contains("battledrone") || (n.contains("battle") && n.contains("drone")) {
            Some(SlaveDroneKind::Battle)
        } else if n.contains("scoutdrone") || (n.contains("scout") && n.contains("drone")) {
            Some(SlaveDroneKind::Scout)
        } else {
            None
        }
    }
}

/// C++ AmericaVehicle.ini ObjectCreationUpgrade ConflictsWith (the other two drones).
pub fn slave_drone_conflicts_with(kind: SlaveDroneKind) -> [SlaveDroneKind; 2] {
    match kind {
        SlaveDroneKind::Scout => [SlaveDroneKind::Battle, SlaveDroneKind::Hellfire],
        SlaveDroneKind::Battle => [SlaveDroneKind::Scout, SlaveDroneKind::Hellfire],
        SlaveDroneKind::Hellfire => [SlaveDroneKind::Scout, SlaveDroneKind::Battle],
    }
}

/// C++ UpgradeMux::wouldUpgrade ConflictsWith for drone ObjectCreationUpgrade.
/// Leftover mux already matches; host objects compare completed names.
pub fn slave_drone_conflicts_with_owned<'a>(
    next: &str,
    owned: impl IntoIterator<Item = &'a str>,
) -> bool {
    let Some(kind) = SlaveDroneKind::from_upgrade_name(next) else {
        return false;
    };
    let conflicts = slave_drone_conflicts_with(kind);
    owned
        .into_iter()
        .filter_map(SlaveDroneKind::from_upgrade_name)
        .any(|have| conflicts.contains(&have))
}

/// C++ Object::affectedByUpgrade residual for Scout/Battle/Hellfire OBJECT_UPGRADE.
/// Non-drone upgrades are not this mux (return true).
pub fn slave_drone_affected_by_upgrade<'a>(
    next: &str,
    owned: impl IntoIterator<Item = &'a str>,
) -> bool {
    let Some(kind) = SlaveDroneKind::from_upgrade_name(next) else {
        return true;
    };
    let owned: Vec<&str> = owned.into_iter().collect();
    if owned
        .iter()
        .any(|name| SlaveDroneKind::from_upgrade_name(name) == Some(kind))
    {
        return false;
    }
    !slave_drone_conflicts_with_owned(next, owned)
}

/// OBJECT_UPGRADE names ConflictsWith would hide once `owned` has a drone.
pub fn slave_drone_unaffected_upgrade_names<'a>(
    owned: impl IntoIterator<Item = &'a str>,
) -> Vec<&'static str> {
    let owned: Vec<&str> = owned.into_iter().collect();
    [
        SlaveDroneKind::Scout,
        SlaveDroneKind::Battle,
        SlaveDroneKind::Hellfire,
    ]
    .into_iter()
    .filter(|kind| slave_drone_conflicts_with_owned(kind.upgrade_name(), owned.iter().copied()))
    .map(SlaveDroneKind::upgrade_name)
    .collect()
}

/// Wave residual honesty: pairwise ConflictsWith mux (one drone per vehicle).
pub fn honesty_slave_drones_conflicts_with_ok() -> bool {
    let scout = slave_drone_conflicts_with(SlaveDroneKind::Scout);
    let battle = slave_drone_conflicts_with(SlaveDroneKind::Battle);
    let hellfire = slave_drone_conflicts_with(SlaveDroneKind::Hellfire);
    scout.contains(&SlaveDroneKind::Battle)
        && scout.contains(&SlaveDroneKind::Hellfire)
        && battle.contains(&SlaveDroneKind::Scout)
        && battle.contains(&SlaveDroneKind::Hellfire)
        && hellfire.contains(&SlaveDroneKind::Scout)
        && hellfire.contains(&SlaveDroneKind::Battle)
        && !slave_drone_conflicts_with_owned(UPGRADE_AMERICA_BATTLE_DRONE, None::<&str>)
        && slave_drone_conflicts_with_owned(
            UPGRADE_AMERICA_BATTLE_DRONE,
            [UPGRADE_AMERICA_SCOUT_DRONE],
        )
        && slave_drone_conflicts_with_owned(
            UPGRADE_AMERICA_SCOUT_DRONE,
            [UPGRADE_AMERICA_HELLFIRE_DRONE],
        )
        && !slave_drone_affected_by_upgrade(
            UPGRADE_AMERICA_HELLFIRE_DRONE,
            [UPGRADE_AMERICA_BATTLE_DRONE],
        )
        && slave_drone_affected_by_upgrade(UPGRADE_AMERICA_SCOUT_DRONE, None::<&str>)
        && !slave_drone_affected_by_upgrade(
            UPGRADE_AMERICA_SCOUT_DRONE,
            [UPGRADE_AMERICA_SCOUT_DRONE],
        )
        && slave_drone_unaffected_upgrade_names([UPGRADE_AMERICA_SCOUT_DRONE])
            == [UPGRADE_AMERICA_BATTLE_DRONE, UPGRADE_AMERICA_HELLFIRE_DRONE]
        && !slave_drone_conflicts_with_owned(
            "Upgrade_AmericaTOWMissile",
            [UPGRADE_AMERICA_SCOUT_DRONE],
        )
}

/// Normalize alnum-lowercase template/upgrade name.
fn alnum_lower(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Whether template is a residual Scout Drone (living unit, not hulk/weapon).
pub fn is_scout_drone_template(template_name: &str) -> bool {
    let n = alnum_lower(template_name);
    if n.is_empty() {
        return false;
    }
    if n.contains("hulk") || n.contains("die") || n.contains("debris") || n.contains("explode") {
        return false;
    }
    if n.contains("weapon") || n.starts_with("upgrade") || n.starts_with("ocl") {
        return false;
    }
    // Exclude Battle/Hellfire/Sentry drones.
    if n.contains("hellfire") || n.contains("battle") || n.contains("sentry") || n.contains("spy") {
        return false;
    }
    n.contains("scoutdrone") || n == "usascoutdrone"
}

/// Whether template is a residual Hellfire Drone (living unit).
pub fn is_hellfire_drone_template(template_name: &str) -> bool {
    let n = alnum_lower(template_name);
    if n.is_empty() {
        return false;
    }
    if n.contains("hulk") || n.contains("die") || n.contains("debris") || n.contains("explode") {
        return false;
    }
    if n.contains("weapon")
        || n.contains("missile")
        || n.starts_with("upgrade")
        || n.starts_with("ocl")
    {
        return false;
    }
    n.contains("hellfiredrone") || n == "usahellfiredrone"
}

/// Whether template is a residual Battle Drone (living unit).
pub fn is_battle_drone_template(template_name: &str) -> bool {
    let n = alnum_lower(template_name);
    if n.is_empty() {
        return false;
    }
    if n.contains("hulk") || n.contains("die") || n.contains("debris") || n.contains("explode") {
        return false;
    }
    // Exclude weapons / machinegun / upgrades / OCL residual names.
    if n.contains("weapon")
        || n.contains("machinegun")
        || n.contains("gun")
        || n.starts_with("upgrade")
        || n.starts_with("ocl")
    {
        return false;
    }
    n.contains("battledrone") || n == "usabattledrone" || n == "testbattledrone"
}

/// Masters that may residual-attach Scout/Hellfire/Battle (Humvee / Crusader / etc.).
///
/// Fail-closed: name residual (not full ObjectCreationUpgrade carrier table).
pub fn is_slave_drone_master_template(template_name: &str) -> bool {
    let n = alnum_lower(template_name);
    if n.is_empty() || n.contains("drone") {
        return false;
    }
    n.contains("humvee")
        || n.contains("hummer")
        || n.contains("crusader")
        || n.contains("paladin")
        || n.contains("microwave")
        || n.contains("avenger")
        || n.contains("tomahawk")
        || n.contains("ambulance")
        || n.contains("vehiclemedic")
}

pub fn scout_spawn_is_detector(template_name: &str) -> bool {
    is_scout_drone_template(template_name)
}

pub fn scout_detection_range(template_name: &str) -> Option<f32> {
    if is_scout_drone_template(template_name) {
        Some(SCOUT_DETECTION_RANGE)
    } else {
        None
    }
}

/// Hellfire auto-fire residual eligibility (mirrors Sentry residual gates).
pub fn hellfire_auto_fire_eligible(
    is_hellfire: bool,
    has_weapon: bool,
    is_alive: bool,
    can_attack: bool,
    idle_or_attacking: bool,
) -> bool {
    is_hellfire && has_weapon && is_alive && can_attack && idle_or_attacking
}

/// Battle Drone auto-fire residual eligibility (same idle gate as Hellfire/Sentry).
pub fn battle_drone_auto_fire_eligible(
    is_battle: bool,
    has_weapon: bool,
    is_alive: bool,
    can_attack: bool,
    idle_or_attacking: bool,
) -> bool {
    is_battle && has_weapon && is_alive && can_attack && idle_or_attacking
}

/// Legal residual target for Hellfire / Battle Drone auto-fire.
pub fn is_legal_hellfire_auto_fire_target(
    is_alive: bool,
    same_team: bool,
    is_neutral: bool,
    under_construction: bool,
    is_attackable_or_combat_kind: bool,
    effectively_stealthed_hidden: bool,
) -> bool {
    is_alive
        && !same_team
        && !is_neutral
        && !under_construction
        && is_attackable_or_combat_kind
        && !effectively_stealthed_hidden
}

/// Alias: Battle Drone auto-fire uses the same legal-target residual as Hellfire.
pub fn is_legal_battle_drone_auto_fire_target(
    is_alive: bool,
    same_team: bool,
    is_neutral: bool,
    under_construction: bool,
    is_attackable_or_combat_kind: bool,
    effectively_stealthed_hidden: bool,
) -> bool {
    is_legal_hellfire_auto_fire_target(
        is_alive,
        same_team,
        is_neutral,
        under_construction,
        is_attackable_or_combat_kind,
        effectively_stealthed_hidden,
    )
}

/// Whether Battle Drone should prioritize master repair residual this tick.
///
/// C++ `SlavedUpdate.cpp:188-193` 1ST PRIORITY: health % only — no range pad.
/// Retail: RepairWhenBelowHealth% = 60 → repair when master current/max < 0.60.
pub fn battle_drone_should_repair_master(
    master_alive: bool,
    master_health_pct: f32,
    drone_alive: bool,
    _distance_to_master: f32,
) -> bool {
    master_alive && drone_alive && master_health_pct < BATTLE_DRONE_REPAIR_BELOW_HEALTH_PCT
}

/// C++ SlavedUpdate.cpp:229-236 idle repair: master health `< 100` after attack/scout.
///
/// The 60% `RepairWhenBelowHealth%` gate is first-priority interrupt only.
/// Range is the weld close-enough band (`battle_drone_weld_close_enough`), not a heal gate.
pub fn battle_drone_should_idle_repair_master(
    master_alive: bool,
    master_health_pct: f32,
    drone_alive: bool,
    _distance_to_master: f32,
) -> bool {
    master_alive && drone_alive && master_health_pct < 100.0
}

/// C++ `doRepairLogic` closeEnough: `distSqr < 12.0f * 12.0f`.
pub fn battle_drone_weld_close_enough(distance_to_master: f32) -> bool {
    distance_to_master >= 0.0 && distance_to_master < BATTLE_DRONE_WELD_CLOSE_ENOUGH
}

/// C++ weld audio pose: pristine bone local + object position, else object position.
/// `bone_host_local` is host Y-up (leftover/C++ Z-up converted at the lookup site).
pub fn battle_drone_weld_pose(drone_pos: Vec3, bone_host_local: Option<Vec3>) -> Vec3 {
    match bone_host_local {
        Some(local) => drone_pos + local,
        None => drone_pos,
    }
}

/// C++ `RepairStates` (`SlavedUpdate.h:102-111`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BattleDroneRepairState {
    #[default]
    None = 0,
    Unpacking = 1,
    Packing = 2,
    Ready = 3,
    Extending = 4,
    Retracting = 5,
    Welding = 6,
}

/// Per-drone weld duty-cycle residual (`m_framesToWait` / `m_repairState` / `m_repairing`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BattleDroneWeldState {
    pub frames_to_wait: i32,
    pub repair_state: BattleDroneRepairState,
    pub repairing: bool,
    /// Leftover `spawn_welding_fx` this tick (Welding enter, RepairWeldingSys set).
    pub weld_fx_this_tick: bool,
}

impl BattleDroneWeldState {
    /// C++ `SlavedUpdate::endRepair` (`SlavedUpdate.cpp:499-510`).
    pub fn end_repair(&mut self) {
        if self.repair_state != BattleDroneRepairState::None {
            self.repair_state = BattleDroneRepairState::None;
            self.frames_to_wait = (SLAVE_DRONE_LOGIC_FPS / 4.0) as i32;
            self.repairing = false;
            self.weld_fx_this_tick = false;
        }
    }

    /// One logic frame of C++ `update` decrement + `doRepairLogic` weld SM.
    ///
    /// Returns whether this frame applies `RepairRatePerSecond` healing
    /// (`closeEnough && m_repairing` — `SlavedUpdate.cpp:480`).
    pub fn tick(&mut self, close_enough: bool) -> bool {
        self.weld_fx_this_tick = false;
        if self.frames_to_wait > 0 {
            self.frames_to_wait -= 1;
        }
        if self.repair_state == BattleDroneRepairState::None && self.frames_to_wait > 0 {
            return false;
        }
        if close_enough {
            match self.repair_state {
                BattleDroneRepairState::None => {
                    self.set_repair_state(BattleDroneRepairState::Ready);
                }
                BattleDroneRepairState::Ready | BattleDroneRepairState::Extending => {
                    if self.frames_to_wait == 0 {
                        self.set_repair_state(BattleDroneRepairState::Welding);
                    }
                }
                BattleDroneRepairState::Unpacking
                | BattleDroneRepairState::Welding
                | BattleDroneRepairState::Retracting => {
                    if self.frames_to_wait == 0 {
                        self.set_repair_state(BattleDroneRepairState::Ready);
                    }
                }
                BattleDroneRepairState::Packing => {}
            }
        } else {
            self.repairing = false;
            if self.frames_to_wait == 0 {
                self.set_repair_state(BattleDroneRepairState::Ready);
            }
        }
        close_enough && self.repairing
    }

    /// C++ `setRepairState` (`SlavedUpdate.cpp:541-647`) without arm FX / spot hop.
    /// Welding enter (not Ready→Extending) latches leftover `spawn_welding_fx`.
    fn set_repair_state(&mut self, desired: BattleDroneRepairState) {
        if desired == self.repair_state {
            return;
        }
        match desired {
            BattleDroneRepairState::Unpacking => {
                self.frames_to_wait = BATTLE_DRONE_WELD_UNPACK_FRAMES;
                self.repair_state = BattleDroneRepairState::Unpacking;
            }
            BattleDroneRepairState::Packing => {
                self.frames_to_wait = BATTLE_DRONE_WELD_UNPACK_FRAMES;
                self.repair_state = BattleDroneRepairState::Packing;
            }
            BattleDroneRepairState::Ready => match self.repair_state {
                BattleDroneRepairState::None => {
                    self.repair_state = BattleDroneRepairState::Unpacking;
                    self.frames_to_wait = BATTLE_DRONE_WELD_UNPACK_FRAMES;
                }
                BattleDroneRepairState::Welding => {
                    self.repair_state = BattleDroneRepairState::Retracting;
                    self.frames_to_wait = BATTLE_DRONE_WELD_ARM_FRAMES;
                }
                _ => {
                    self.repair_state = BattleDroneRepairState::Ready;
                    self.frames_to_wait =
                        slave_drone_ms_to_frames(BATTLE_DRONE_REPAIR_MIN_READY_MS) as i32;
                }
            },
            BattleDroneRepairState::Welding => {
                if self.repair_state == BattleDroneRepairState::Ready {
                    self.repair_state = BattleDroneRepairState::Extending;
                    self.frames_to_wait = BATTLE_DRONE_WELD_ARM_FRAMES;
                } else {
                    self.repair_state = BattleDroneRepairState::Welding;
                    self.frames_to_wait =
                        slave_drone_ms_to_frames(BATTLE_DRONE_REPAIR_MIN_WELD_MS) as i32;
                    self.repairing = true;
                    if !BATTLE_DRONE_REPAIR_WELDING_SYS.is_empty() {
                        self.weld_fx_this_tick = true;
                    }
                }
            }
            BattleDroneRepairState::None
            | BattleDroneRepairState::Extending
            | BattleDroneRepairState::Retracting => {}
        }
    }
}

/// C++ doAttackLogic DistToTargetToGrantRangeBonus² (Scout residual **20**).
pub fn scout_drone_should_grant_range_bonus(is_scout: bool, slave_to_victim_dist_sqr: f32) -> bool {
    is_scout
        && slave_to_victim_dist_sqr
            < SCOUT_DIST_TO_TARGET_RANGE_BONUS * SCOUT_DIST_TO_TARGET_RANGE_BONUS
}

/// C++ SlavedUpdate.cpp:145-150: hijack/defect when master is no longer ALLIES.
pub fn slave_should_defect_to_master(master_team_eq_slave: bool) -> bool {
    !master_team_eq_slave
}

/// C++ StayOnSameLayerAsMaster: copy master height (not `drone.max(master)`).
pub fn slave_follow_destination_y(drone_y: f32, master_y: f32) -> f32 {
    if SLAVE_STAY_ON_SAME_LAYER {
        master_y
    } else {
        drone_y.max(master_y)
    }
}

/// Residual HP restored for one logic frame of Battle Drone repair.
///
/// RepairRatePerSecond 10 @ 30 FPS → 10/30 HP per frame.
pub fn battle_drone_repair_amount_for_frame(dt_seconds: f32) -> f32 {
    BATTLE_DRONE_REPAIR_RATE_PER_SEC * dt_seconds.max(0.0)
}

/// Build residual Battle Drone machine-gun Weapon.
pub fn battle_drone_weapon() -> Weapon {
    Weapon {
        damage: BATTLE_DRONE_GUN_DAMAGE,
        range: BATTLE_DRONE_GUN_RANGE,
        min_range: 0.0,
        reload_time: (BATTLE_DRONE_GUN_DELAY_FRAMES.max(1) as f32) / 30.0,
        last_fire_time: 0.0,
        ammo: None,
        clip_size: 0,
        clip_reload_time: 0.0,
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: 999_999.0,
        pre_attack_delay: 0.0,
        splash_radius: 0.0,
        suspend_fx_frame: 0,
        reloading_clip: false,
        last_bonus_rof: 0.0,
    }
}

/// Offset from master position for residual drone spawn (XZ).
///
/// Host presentation offset (not exact OCL bone); OCL residual honesty uses
/// `drone_ocl_spawn_offset`.
pub fn drone_spawn_offset_from_master(kind: SlaveDroneKind) -> (f32, f32) {
    match kind {
        SlaveDroneKind::Scout => (12.0, 8.0),
        SlaveDroneKind::Hellfire => (-12.0, 8.0),
        SlaveDroneKind::Battle => (0.0, 14.0),
    }
}

/// Retail OCL CreateObject Offset residual (X, Y, Z) for spawn honesty.
pub fn drone_ocl_spawn_offset(kind: SlaveDroneKind) -> (f32, f32, f32) {
    match kind {
        SlaveDroneKind::Scout => (SCOUT_SPAWN_OFFSET_X, 0.0, DRONE_SPAWN_OFFSET_Z),
        SlaveDroneKind::Hellfire | SlaveDroneKind::Battle => {
            (BATTLE_HELLFIRE_SPAWN_OFFSET_X, 0.0, DRONE_SPAWN_OFFSET_Z)
        }
    }
}

/// Retail OCL name residual for kind.
pub fn drone_ocl_name(kind: SlaveDroneKind) -> &'static str {
    match kind {
        SlaveDroneKind::Scout => OCL_AMERICAN_SCOUT_DRONE,
        SlaveDroneKind::Hellfire => OCL_AMERICAN_HELLFIRE_DRONE,
        SlaveDroneKind::Battle => OCL_AMERICAN_BATTLE_DRONE,
    }
}

/// Object-upgrade BuildCost residual for kind.
pub fn drone_upgrade_build_cost(kind: SlaveDroneKind) -> u32 {
    match kind {
        SlaveDroneKind::Scout => SCOUT_UPGRADE_BUILD_COST,
        SlaveDroneKind::Hellfire => HELLFIRE_UPGRADE_BUILD_COST,
        SlaveDroneKind::Battle => BATTLE_UPGRADE_BUILD_COST,
    }
}

/// DroneArmor AddMaxHealth residual for kind.
pub fn drone_armor_add_max_health(kind: SlaveDroneKind) -> f32 {
    match kind {
        SlaveDroneKind::Scout | SlaveDroneKind::Hellfire => SCOUT_HELLFIRE_DRONE_ARMOR_ADD,
        SlaveDroneKind::Battle => BATTLE_DRONE_ARMOR_ADD,
    }
}

/// Living slave-drone kind for MaxHealthUpgrade / spawn inherit.
pub fn slave_drone_kind_from_template(template_name: &str) -> Option<SlaveDroneKind> {
    if is_battle_drone_template(template_name) {
        Some(SlaveDroneKind::Battle)
    } else if is_hellfire_drone_template(template_name) {
        Some(SlaveDroneKind::Hellfire)
    } else if is_scout_drone_template(template_name) {
        Some(SlaveDroneKind::Scout)
    } else {
        None
    }
}

/// Apply DroneArmor MaxHealthUpgrade residual: +AddMaxHealth current+max
/// (`ADD_CURRENT_HEALTH_TOO`, same as C++ `setMaxHealth` at initObject).
pub fn apply_drone_armor_health(
    kind: SlaveDroneKind,
    max_health: &mut f32,
    current: &mut f32,
    maximum: &mut f32,
) {
    let add = drone_armor_add_max_health(kind);
    *max_health = (*max_health + add).max(1.0);
    *maximum = (*maximum + add).max(1.0);
    *current = (*current + add).min(*maximum);
}

// --- Wave 61 residual honesty packs ---

/// Wave 61 residual honesty: SlavedUpdate wander residual.
pub fn honesty_slave_drones_wander_residual_ok() -> bool {
    (SLAVE_GUARD_MAX_RANGE - 35.0).abs() < 0.01
        && (SLAVE_GUARD_WANDER_RANGE - 35.0).abs() < 0.01
        && (SLAVE_ATTACK_RANGE - 75.0).abs() < 0.01
        && (SLAVE_ATTACK_WANDER_RANGE - 10.0).abs() < 0.01
        && (SLAVE_SCOUT_RANGE - 75.0).abs() < 0.01
        && (SLAVE_SCOUT_WANDER_RANGE - 10.0).abs() < 0.01
        && SLAVE_STAY_ON_SAME_LAYER
        && (SCOUT_DIST_TO_TARGET_RANGE_BONUS - 20.0).abs() < 0.01
        && (DRONE_SPOTTING_RANGE_MULT - 1.50).abs() < 0.01
        && SCOUT_DETECTION_RATE_MS == 500
        && SCOUT_DETECTION_RATE_FRAMES == slave_drone_ms_to_frames(SCOUT_DETECTION_RATE_MS)
        && SCOUT_DETECTION_RATE_FRAMES == 15
        && (SCOUT_DETECTION_RANGE - 150.0).abs() < 0.01
        && (SCOUT_SHROUD_CLEARING_RANGE - 500.0).abs() < 0.01
}

/// Wave 61 residual honesty: OCL spawn + upgrade residual.
pub fn honesty_slave_drones_spawn_residual_ok() -> bool {
    OCL_AMERICAN_SCOUT_DRONE == "OCL_AmericanScoutDrone"
        && OCL_AMERICAN_BATTLE_DRONE == "OCL_AmericanBattleDrone"
        && OCL_AMERICAN_HELLFIRE_DRONE == "OCL_AmericanHellfireDrone"
        && drone_ocl_name(SlaveDroneKind::Scout) == OCL_AMERICAN_SCOUT_DRONE
        && drone_ocl_name(SlaveDroneKind::Battle) == OCL_AMERICAN_BATTLE_DRONE
        && drone_ocl_name(SlaveDroneKind::Hellfire) == OCL_AMERICAN_HELLFIRE_DRONE
        && {
            let (x, y, z) = drone_ocl_spawn_offset(SlaveDroneKind::Scout);
            (x - (-8.0)).abs() < 0.01 && y.abs() < 0.01 && (z - 10.0).abs() < 0.01
        }
        && {
            let (x, _, z) = drone_ocl_spawn_offset(SlaveDroneKind::Battle);
            x.abs() < 0.01 && (z - 10.0).abs() < 0.01
        }
        && {
            let (x, _, z) = drone_ocl_spawn_offset(SlaveDroneKind::Hellfire);
            x.abs() < 0.01 && (z - 10.0).abs() < 0.01
        }
        && DRONE_SPAWN_COUNT == 1
        && DRONE_SPAWN_DISPOSITION == "LIKE_EXISTING"
        && SCOUT_UPGRADE_BUILD_COST == 100
        && BATTLE_UPGRADE_BUILD_COST == 300
        && HELLFIRE_UPGRADE_BUILD_COST == 500
        && (DRONE_UPGRADE_BUILD_TIME - 5.0).abs() < 0.01
        && drone_upgrade_build_cost(SlaveDroneKind::Scout) == 100
        && drone_upgrade_build_cost(SlaveDroneKind::Battle) == 300
        && drone_upgrade_build_cost(SlaveDroneKind::Hellfire) == 500
        && (SLAVE_DRONE_MAX_HEALTH - 100.0).abs() < 0.01
        && (drone_armor_add_max_health(SlaveDroneKind::Scout) - 25.0).abs() < 0.01
        && (drone_armor_add_max_health(SlaveDroneKind::Hellfire) - 25.0).abs() < 0.01
        && (drone_armor_add_max_health(SlaveDroneKind::Battle) - 50.0).abs() < 0.01
        && UPGRADE_AMERICA_DRONE_ARMOR == "Upgrade_AmericaDroneArmor"
        && DRONE_ARMOR_BUILD_COST == 500
        && (DRONE_ARMOR_BUILD_TIME - 40.0).abs() < 0.01
        && is_slave_drone_master_template("AmericaVehicleHumvee")
}

/// Wave 61 residual honesty: Battle Drone repair residual + Hellfire/Battle weapons.
pub fn honesty_slave_drones_repair_residual_ok() -> bool {
    (BATTLE_DRONE_REPAIR_RATE_PER_SEC - 10.0).abs() < 0.01
        && (BATTLE_DRONE_REPAIR_BELOW_HEALTH_PCT - 60.0).abs() < 0.01
        && (BATTLE_DRONE_REPAIR_RANGE - 8.0).abs() < 0.01
        && (BATTLE_DRONE_WELD_CLOSE_ENOUGH - 12.0).abs() < 0.01
        && BATTLE_DRONE_WELD_UNPACK_FRAMES == 15
        && BATTLE_DRONE_WELD_ARM_FRAMES == 5
        && (BATTLE_DRONE_REPAIR_MIN_ALTITUDE - 18.0).abs() < 0.01
        && (BATTLE_DRONE_REPAIR_MAX_ALTITUDE - 24.0).abs() < 0.01
        && BATTLE_DRONE_REPAIR_MIN_READY_MS == 300
        && BATTLE_DRONE_REPAIR_MAX_READY_MS == 750
        && BATTLE_DRONE_REPAIR_MIN_WELD_MS == 250
        && BATTLE_DRONE_REPAIR_MAX_WELD_MS == 500
        && BATTLE_DRONE_REPAIR_WELDING_SYS == "BlueSparks"
        && BATTLE_DRONE_REPAIR_WELDING_FX_BONE == "Muzzle02"
        && BATTLE_DRONE_REPAIR_SPARKS_AUDIO == "RepairSparks"
        && battle_drone_should_repair_master(true, 50.0, true, 10.0)
        && !battle_drone_should_repair_master(true, 80.0, true, 10.0)
        && battle_drone_weld_close_enough(11.9)
        && !battle_drone_weld_close_enough(12.0)
        && !battle_drone_weld_close_enough(48.0)
        && {
            let mut weld = BattleDroneWeldState::default();
            // Instant first frame must not heal — unpack/ready/extend first.
            !weld.tick(true) && !weld.weld_fx_this_tick
        }
        && {
            let mut weld = BattleDroneWeldState::default();
            let mut sparked = false;
            for _ in 0..64 {
                let _ = weld.tick(true);
                if weld.weld_fx_this_tick {
                    sparked =
                        weld.repair_state == BattleDroneRepairState::Welding && weld.repairing;
                    break;
                }
            }
            sparked
        }
        && {
            let origin = Vec3::new(1.0, 2.0, 3.0);
            battle_drone_weld_pose(origin, None) == origin
                && (battle_drone_weld_pose(origin, Some(Vec3::new(4.0, 5.0, 6.0)))
                    - Vec3::new(5.0, 7.0, 9.0))
                .length()
                    < 0.01
        }
        && (battle_drone_repair_amount_for_frame(1.0) - 10.0).abs() < 0.01
        && (BATTLE_DRONE_GUN_DAMAGE - 1.0).abs() < 0.01
        && (BATTLE_DRONE_GUN_RANGE - 110.0).abs() < 0.01
        && BATTLE_DRONE_GUN_DELAY_MS == 100
        && BATTLE_DRONE_GUN_DELAY_FRAMES == slave_drone_ms_to_frames(BATTLE_DRONE_GUN_DELAY_MS)
        && BATTLE_DRONE_GUN_DELAY_FRAMES == 3
        && (BATTLE_DRONE_AP_DAMAGE_MULT - 1.25).abs() < 0.001
        && (HELLFIRE_DAMAGE - 40.0).abs() < 0.01
        && (HELLFIRE_PRIMARY_RADIUS - 5.0).abs() < 0.01
        && (HELLFIRE_SCATTER_VS_INFANTRY - 10.0).abs() < 0.01
        && (HELLFIRE_RANGE - 150.0).abs() < 0.01
        && HELLFIRE_DELAY_MS == 1000
        && HELLFIRE_CLIP_RELOAD_MS == 2000
        && HELLFIRE_CLIP_SIZE == 1
        && HELLFIRE_CYCLE_FRAMES == 90
        && HELLFIRE_PROJECTILE == "MissileDefenderMissile"
        && HELLFIRE_MISSILE_WEAPON == "HellfireMissileWeapon"
        && BATTLE_DRONE_MACHINE_GUN == "BattleDroneMachineGun"
        && (HELLFIRE_VISION_RANGE - 100.0).abs() < 0.01
        && (HELLFIRE_SHROUD_CLEARING_RANGE - 500.0).abs() < 0.01
        && (BATTLE_DRONE_VISION_RANGE - 150.0).abs() < 0.01
        && (BATTLE_DRONE_SHROUD_CLEARING_RANGE - 150.0).abs() < 0.01
}

/// Combined Wave 61 slave-drone residual honesty pack.

/// Apply HellfireMissileWeapon ScatterRadiusVsInfantry residual to aim.
pub fn hellfire_scatter_aim(aim: Vec3, target_is_infantry: bool, seed: u32) -> (Vec3, bool) {
    use crate::game_logic::weapon_bootstrap::{host_effective_scatter_radius, scatter_aim_offset};
    let mut scatter = host_effective_scatter_radius(HELLFIRE_MISSILE_WEAPON, target_is_infantry);
    if target_is_infantry && scatter <= 0.0 {
        scatter = HELLFIRE_SCATTER_VS_INFANTRY;
    }
    if scatter <= 0.0 {
        return (aim, false);
    }
    let off = scatter_aim_offset(seed, scatter);
    (Vec3::new(aim.x + off.x, aim.y, aim.z + off.z), true)
}

/// Whether Hellfire residual fire misses intended infantry via ScatterRadiusVsInfantry.
pub fn hellfire_scatter_misses_infantry(
    target_is_infantry: bool,
    seed: u32,
    target_hit_radius: f32,
) -> bool {
    use crate::game_logic::weapon_bootstrap::scatter_misses_intended_target;
    if !target_is_infantry {
        return false;
    }
    scatter_misses_intended_target(HELLFIRE_SCATTER_VS_INFANTRY, seed, target_hit_radius)
}

/// Wave residual honesty: Hellfire ScatterRadiusVsInfantry peels (**10**).
pub fn honesty_hellfire_scatter_vs_infantry_ok() -> bool {
    use crate::game_logic::weapon_bootstrap::host_effective_scatter_radius;
    let vs = host_effective_scatter_radius(HELLFIRE_MISSILE_WEAPON, true);
    let ground = host_effective_scatter_radius(HELLFIRE_MISSILE_WEAPON, false);
    (HELLFIRE_SCATTER_VS_INFANTRY - 10.0).abs() < 0.01
        && ((vs - 10.0).abs() < 0.01 || vs <= 0.0)
        && ground.abs() < 0.01
        && {
            let (sc, applied) = hellfire_scatter_aim(Vec3::new(0.0, 0.0, 0.0), true, 3);
            applied && sc.length() <= HELLFIRE_SCATTER_VS_INFANTRY + 0.01
        }
}

pub fn honesty_slave_drones_residual_pack_ok() -> bool {
    honesty_slave_drones_wander_residual_ok()
        && honesty_slave_drones_spawn_residual_ok()
        && honesty_slave_drones_repair_residual_ok()
        && honesty_hellfire_scatter_vs_infantry_ok()
        && honesty_slave_drones_conflicts_with_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hellfire_scatter_vs_infantry_peels() {
        assert!(honesty_hellfire_scatter_vs_infantry_ok());
        let aim = Vec3::new(80.0, 0.0, 12.0);
        let (no_sc, applied) = hellfire_scatter_aim(aim, false, 31);
        assert!(!applied);
        assert_eq!(no_sc, aim);
        let (sc, applied) = hellfire_scatter_aim(aim, true, 31);
        assert!(applied);
        let d = ((sc.x - aim.x).powi(2) + (sc.z - aim.z).powi(2)).sqrt();
        assert!(d > 0.01 && d <= HELLFIRE_SCATTER_VS_INFANTRY + 0.01);
        assert!(hellfire_scatter_misses_infantry(true, 31, 0.5));
        assert!(!hellfire_scatter_misses_infantry(true, 31, 100.0));
        assert!(!hellfire_scatter_misses_infantry(false, 31, 0.5));
    }

    #[test]
    fn scout_name_matrix() {
        assert!(is_scout_drone_template("AmericaVehicleScoutDrone"));
        assert!(is_scout_drone_template("USA_ScoutDrone"));
        assert!(is_scout_drone_template("AirF_AmericaVehicleScoutDrone"));
        assert!(is_scout_drone_template("TestScoutDrone"));
        assert!(!is_scout_drone_template("AmericaVehicleHellfireDrone"));
        assert!(!is_scout_drone_template("AmericaVehicleSentryDrone"));
        assert!(!is_scout_drone_template("AmericaScoutDroneHulk"));
        assert!(!is_scout_drone_template("Upgrade_AmericaScoutDrone"));
        assert!(!is_scout_drone_template("AmericaVehicleHumvee"));
    }

    #[test]
    fn hellfire_name_matrix() {
        assert!(is_hellfire_drone_template("AmericaVehicleHellfireDrone"));
        assert!(is_hellfire_drone_template("USA_HellfireDrone"));
        assert!(is_hellfire_drone_template("TestHellfireDrone"));
        assert!(!is_hellfire_drone_template("AmericaVehicleScoutDrone"));
        assert!(!is_hellfire_drone_template("HellfireMissileWeapon"));
        assert!(!is_hellfire_drone_template("Upgrade_AmericaHellfireDrone"));
    }

    #[test]
    fn battle_drone_name_and_repair_matrix() {
        assert!(is_battle_drone_template("AmericaVehicleBattleDrone"));
        assert!(is_battle_drone_template("USA_BattleDrone"));
        assert!(is_battle_drone_template("TestBattleDrone"));
        assert!(is_battle_drone_template("SupW_AmericaVehicleBattleDrone"));
        assert!(!is_battle_drone_template("AmericaVehicleScoutDrone"));
        assert!(!is_battle_drone_template("AmericaVehicleHellfireDrone"));
        assert!(!is_battle_drone_template("BattleDroneMachineGun"));
        assert!(!is_battle_drone_template("Upgrade_AmericaBattleDrone"));
        assert!(!is_scout_drone_template("AmericaVehicleBattleDrone"));

        let w = battle_drone_weapon();
        assert!((w.damage - 1.0).abs() < 0.01);
        assert!((w.range - 110.0).abs() < 0.01);
        assert!((w.reload_time - 3.0 / 30.0).abs() < 0.01);

        assert!(battle_drone_should_repair_master(true, 50.0, true, 10.0));
        assert!(!battle_drone_should_repair_master(true, 80.0, true, 10.0));
        assert!(battle_drone_should_idle_repair_master(
            true, 80.0, true, 10.0
        ));
        assert!(!battle_drone_should_idle_repair_master(
            true, 100.0, true, 10.0
        ));
        assert!(scout_drone_should_grant_range_bonus(true, 19.0 * 19.0));
        assert!(!scout_drone_should_grant_range_bonus(true, 21.0 * 21.0));
        assert!(!scout_drone_should_grant_range_bonus(false, 0.0));
        assert!(slave_should_defect_to_master(false));
        assert!(!slave_should_defect_to_master(true));
        assert!((slave_follow_destination_y(12.0, 4.0) - 4.0).abs() < 0.01);

        assert!((battle_drone_repair_amount_for_frame(1.0) - 10.0).abs() < 0.01);
        assert!((battle_drone_repair_amount_for_frame(1.0 / 30.0) - 10.0 / 30.0).abs() < 0.001);
        assert!(battle_drone_auto_fire_eligible(
            true, true, true, true, true
        ));
        assert!(!battle_drone_auto_fire_eligible(
            true, false, true, true, true
        ));
    }

    #[test]
    fn master_and_kind_matrix() {
        assert!(is_slave_drone_master_template("AmericaVehicleHumvee"));
        assert!(is_slave_drone_master_template("USA_Humvee"));
        assert!(is_slave_drone_master_template("AmericaTankCrusader"));
        assert!(!is_slave_drone_master_template("AmericaVehicleScoutDrone"));
        assert!(!is_slave_drone_master_template("USA_Ranger"));
        assert_eq!(
            SlaveDroneKind::from_upgrade_name(UPGRADE_AMERICA_SCOUT_DRONE),
            Some(SlaveDroneKind::Scout)
        );
        assert_eq!(
            SlaveDroneKind::from_upgrade_name(UPGRADE_AMERICA_HELLFIRE_DRONE),
            Some(SlaveDroneKind::Hellfire)
        );
        assert_eq!(
            SlaveDroneKind::from_upgrade_name(UPGRADE_AMERICA_BATTLE_DRONE),
            Some(SlaveDroneKind::Battle)
        );
        assert_eq!(
            scout_detection_range("AmericaVehicleScoutDrone"),
            Some(SCOUT_DETECTION_RANGE)
        );
        assert!(hellfire_auto_fire_eligible(true, true, true, true, true));
        assert!(!hellfire_auto_fire_eligible(true, false, true, true, true));
        assert!(honesty_slave_drones_conflicts_with_ok());
    }

    #[test]
    fn slave_drones_residual_pack_honesty_wave61() {
        assert!(honesty_slave_drones_wander_residual_ok());
        assert!(honesty_slave_drones_spawn_residual_ok());
        assert!(honesty_slave_drones_repair_residual_ok());
        assert!(honesty_hellfire_scatter_vs_infantry_ok());
        assert!(honesty_slave_drones_residual_pack_ok());
        assert_eq!(slave_drone_ms_to_frames(500), 15);
        assert_eq!(slave_drone_ms_to_frames(100), 3);
        assert_eq!(slave_drone_ms_to_frames(1000), 30);
        let (sx, _, sz) = drone_ocl_spawn_offset(SlaveDroneKind::Scout);
        assert!((sx - (-8.0)).abs() < 0.01);
        assert!((sz - 10.0).abs() < 0.01);
        assert_eq!(drone_upgrade_build_cost(SlaveDroneKind::Hellfire), 500);
        assert!((drone_armor_add_max_health(SlaveDroneKind::Battle) - 50.0).abs() < 0.01);
        assert!((SLAVE_GUARD_MAX_RANGE - 35.0).abs() < 0.01);
        assert!((BATTLE_DRONE_REPAIR_MIN_ALTITUDE - 18.0).abs() < 0.01);
    }

    #[test]
    fn compute_aggregate_syncs_to_higher_rank() {
        use crate::game_logic::VeterancyLevel;
        let (m, s) = synced_spawn_veterancy(VeterancyLevel::Elite, VeterancyLevel::Rookie);
        assert_eq!(m, VeterancyLevel::Elite);
        assert_eq!(s, VeterancyLevel::Elite);
        let (m, s) = synced_spawn_veterancy(VeterancyLevel::Rookie, VeterancyLevel::Veteran);
        assert_eq!(m, VeterancyLevel::Veteran);
        assert_eq!(s, VeterancyLevel::Veteran);
        let (m, s) = synced_spawn_veterancy(VeterancyLevel::Elite, VeterancyLevel::Elite);
        assert_eq!(m, VeterancyLevel::Elite);
        assert_eq!(s, VeterancyLevel::Elite);
    }

    fn veteran_humvee() -> (crate::game_logic::GameLogic, crate::game_logic::ObjectId) {
        use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, VeterancyLevel};
        let mut logic = crate::game_logic::GameLogic::new();
        let mid = ObjectId(7100);
        let mut mt = ThingTemplate::new("AmericaVehicleHumvee");
        mt.add_kind_of(KindOf::Vehicle);
        logic.objects.insert(mid, {
            let mut o = Object::new(mt, mid, Team::USA);
            o.set_position(glam::Vec3::ZERO);
            let _ = o.set_min_veterancy_level(VeterancyLevel::Veteran);
            o
        });
        (logic, mid)
    }

    #[test]
    fn slave_drone_veteran_humvee_spawn_yields_veteran_drone() {
        use crate::game_logic::VeterancyLevel;
        let kinds = [
            SlaveDroneKind::Scout,
            SlaveDroneKind::Battle,
            SlaveDroneKind::Hellfire,
        ];
        for kind in kinds {
            let (mut logic, mid) = veteran_humvee();
            let drone = logic
                .residual_attach_slave_drone(mid, kind)
                .expect("drone attach");
            assert_eq!(
                logic.objects[&drone].experience.level,
                VeterancyLevel::Veteran,
                "{kind:?} must inherit veteran Humvee rank"
            );
        }
    }

    #[test]
    fn slave_drone_elite_promotes_master_on_host_tick() {
        use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, VeterancyLevel};
        let mut logic = crate::game_logic::GameLogic::new();
        let mid = ObjectId(7200);
        let mut mt = ThingTemplate::new("AmericaVehicleHumvee");
        mt.add_kind_of(KindOf::Vehicle);
        logic.objects.insert(mid, {
            let mut o = Object::new(mt, mid, Team::USA);
            o.set_position(glam::Vec3::ZERO);
            o
        });
        let drone = logic
            .residual_attach_slave_drone(mid, SlaveDroneKind::Scout)
            .expect("scout attach");
        assert_eq!(
            logic.objects[&drone].experience.level,
            VeterancyLevel::Rookie
        );
        let _ = logic
            .objects
            .get_mut(&drone)
            .expect("drone")
            .set_min_veterancy_level(VeterancyLevel::Elite);
        logic.update_with_dt(1.0 / 30.0);
        assert_eq!(
            logic.objects[&mid].experience.level,
            VeterancyLevel::Elite,
            "drone elite must promote master on live host tick"
        );
    }
}
