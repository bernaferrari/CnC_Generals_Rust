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
//!   master repair residual (`RepairRatePerSecond` **10** when master HP < **60%**).
//! - Master attach residual: spawn drone near a Humvee/compatible vehicle and
//!   tag master with the object-upgrade residual (`Upgrade_AmericaScoutDrone` /
//!   `Upgrade_AmericaHellfireDrone` / `Upgrade_AmericaBattleDrone`).
//!
//! Fail-closed honesty:
//! - Not full SlavedUpdate guard/scout wander ranges / master layer lock
//! - Not full ObjectCreationUpgrade ConflictsWith / ProductionUpdate queue UI
//! - Not full drone armor MaxHealthUpgrade / death OCL explode matrix
//! - Not full Battle Drone arm pack/unpack weld FX / RepairMinAltitude matrix
//! - Not network drone / upgrade replication (network deferred)

use super::Weapon;

/// Retail object-upgrade names.
pub const UPGRADE_AMERICA_SCOUT_DRONE: &str = "Upgrade_AmericaScoutDrone";
pub const UPGRADE_AMERICA_HELLFIRE_DRONE: &str = "Upgrade_AmericaHellfireDrone";
pub const UPGRADE_AMERICA_BATTLE_DRONE: &str = "Upgrade_AmericaBattleDrone";

/// Retail drone template names.
pub const SCOUT_DRONE_TEMPLATE: &str = "AmericaVehicleScoutDrone";
pub const HELLFIRE_DRONE_TEMPLATE: &str = "AmericaVehicleHellfireDrone";
pub const BATTLE_DRONE_TEMPLATE: &str = "AmericaVehicleBattleDrone";

/// Retail HellfireMissileWeapon primary name.
pub const HELLFIRE_MISSILE_WEAPON: &str = "HellfireMissileWeapon";
/// Retail BattleDroneMachineGun primary name.
pub const BATTLE_DRONE_MACHINE_GUN: &str = "BattleDroneMachineGun";

/// Scout VisionRange residual (DetectionRange unset → vision).
pub const SCOUT_DETECTION_RANGE: f32 = 150.0;

/// Hellfire PrimaryDamage / AttackRange / cycle.
pub const HELLFIRE_DAMAGE: f32 = 40.0;
pub const HELLFIRE_RANGE: f32 = 150.0;
/// DelayBetweenShots 1000ms + ClipReload 2000ms (ClipSize 1) ≈ 3s → 90 frames.
pub const HELLFIRE_CYCLE_FRAMES: u32 = 90;

/// Battle Drone MachineGun PrimaryDamage residual.
pub const BATTLE_DRONE_GUN_DAMAGE: f32 = 1.0;
/// Battle Drone MachineGun AttackRange residual.
pub const BATTLE_DRONE_GUN_RANGE: f32 = 110.0;
/// Battle Drone DelayBetweenShots 100ms → 3 frames @ 30 FPS.
pub const BATTLE_DRONE_GUN_DELAY_FRAMES: u32 = 3;
/// Retail SlavedUpdate RepairRatePerSecond residual.
pub const BATTLE_DRONE_REPAIR_RATE_PER_SEC: f32 = 10.0;
/// Retail RepairWhenBelowHealth% residual.
pub const BATTLE_DRONE_REPAIR_BELOW_HEALTH_PCT: f32 = 60.0;
/// Retail RepairRange residual (how close to master for repair).
pub const BATTLE_DRONE_REPAIR_RANGE: f32 = 8.0;

/// Residual audio.
pub const HELLFIRE_FIRE_AUDIO: &str = "MissileDefenderWeapon";
pub const BATTLE_DRONE_FIRE_AUDIO: &str = "BattleDroneWeapon";

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
    if n.contains("weapon") || n.contains("missile") || n.starts_with("upgrade") || n.starts_with("ocl")
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
/// Retail: RepairWhenBelowHealth% = 60 → repair when master current/max < 0.60.
pub fn battle_drone_should_repair_master(
    master_alive: bool,
    master_health_pct: f32,
    drone_alive: bool,
    distance_to_master: f32,
) -> bool {
    master_alive
        && drone_alive
        && master_health_pct < BATTLE_DRONE_REPAIR_BELOW_HEALTH_PCT
        && distance_to_master <= BATTLE_DRONE_REPAIR_RANGE + 40.0 // host residual pad
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
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: 999_999.0,
        pre_attack_delay: 0.0,
    }
}

/// Offset from master position for residual drone spawn (XZ).
pub fn drone_spawn_offset_from_master(kind: SlaveDroneKind) -> (f32, f32) {
    match kind {
        SlaveDroneKind::Scout => (12.0, 8.0),
        SlaveDroneKind::Hellfire => (-12.0, 8.0),
        SlaveDroneKind::Battle => (0.0, 14.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!((battle_drone_repair_amount_for_frame(1.0) - 10.0).abs() < 0.01);
        assert!((battle_drone_repair_amount_for_frame(1.0 / 30.0) - 10.0 / 30.0).abs() < 0.001);
        assert!(battle_drone_auto_fire_eligible(true, true, true, true, true));
        assert!(!battle_drone_auto_fire_eligible(true, false, true, true, true));
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
    }
}
