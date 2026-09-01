//! Host LocomotorStore bootstrap for template → movement speed binding.
//!
//! # Why host Movement defaults to 10 u/s
//!
//! `Object::new` uses `Movement::default()` (max_speed 10, accel 5). Full
//! Locomotor.ini population happens when the engine loads BIG archives into
//! Common's `ini_locomotor` store. Headless unit tests and many host probes
//! never open archives, so rangers stayed at the host default and golden
//! skirmish had to lift them toward retail BasicHumanLocomotor (20).
//!
//! This module is the reliable host-side fill path (mirrors `weapon_bootstrap`):
//! - Prefer loading extracted / shipped `Data/INI/Locomotor.ini` when present
//! - Always seed a small set of golden-unit locomotors if still missing
//! - Bind SET_NORMAL locomotor names on known host unit templates
//!
//! Fail-closed residual:
//! - Not a full multi-locomotor set / surface-type matrix / SET_PANIC upgrades
//! - Host binds only primary SET_NORMAL speed/accel/turn for create_object
//! - Common store stores per-frame values (C++ parity); host Movement is
//!   dist/sec and rads/sec, so we convert when resolving for Object.movement

use game_engine::common::ini::ini_locomotor::{
    LocomotorAppearance as SourceLocomotorAppearance,
    LocomotorBehaviorZ as SourceLocomotorBehaviorZ, LocomotorTemplate, get_locomotor_store,
    get_locomotor_store_mut, load_locomotors_from_str, parse_locomotor_template_definition,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

/// Retail SET_NORMAL locomotor names used by host golden / skirmish unit templates.
pub const BASIC_HUMAN_LOCOMOTOR: &str = "BasicHumanLocomotor";
pub const REDGUARD_LOCOMOTOR: &str = "RedguardLocomotor";
pub const HUMVEE_LOCOMOTOR: &str = "HumveeLocomotor";
pub const CRUSADER_LOCOMOTOR: &str = "CrusaderLocomotor";
pub const SCORPION_LOCOMOTOR: &str = "ScorpionLocomotor";
pub const BATTLE_MASTER_LOCOMOTOR: &str = "BattleMasterLocomotor";
pub const TECHNICAL_LOCOMOTOR: &str = "TechnicalLocomotor";

// --- Wave 81 common-unit locomotor residual deepen ---
/// Retail Pathfinder / Colonel Burton ground residual.
pub const COLONEL_BURTON_GROUND_LOCOMOTOR: &str = "ColonelBurtonGroundLocomotor";
/// Retail AmericaVehicleTomahawk residual.
pub const TOMAHAWK_LOCOMOTOR: &str = "TomahawkLocomotor";
/// Retail GLAVehicleSCUDLauncher residual.
pub const SCUD_LAUNCHER_LOCOMOTOR: &str = "ScudLauncherLocomotor";
/// Retail GLAVehicleQuadCannon residual.
pub const QUAD_CANNON_LOCOMOTOR: &str = "QuadCannonLocomotor";
/// Retail AmericaJetRaptor residual.
pub const RAPTOR_JET_LOCOMOTOR: &str = "RaptorJetLocomotor";

// --- Wave 92 common-unit locomotor residual expand ---
/// Retail ChinaTankOverlord residual.
pub const OVERLORD_LOCOMOTOR: &str = "OverlordLocomotor";
/// Retail GLATankMarauder residual.
pub const MARAUDER_LOCOMOTOR: &str = "MarauderLocomotor";
/// Retail ChinaTankDragon residual.
pub const DRAGON_LOCOMOTOR: &str = "DragonLocomotor";
/// Retail AmericaVehicleComanche residual.
pub const COMANCHE_LOCOMOTOR: &str = "ComancheLocomotor";
/// Retail ChinaJetMIG residual.
pub const MIG_LOCOMOTOR: &str = "MIGLocomotor";
/// Retail GLAVehicleRocketBuggy residual.
pub const ROCKET_BUGGY_LOCOMOTOR: &str = "RocketBuggyLocomotor";
/// Retail GLAVehicleBattleBus residual.
pub const BATTLE_BUS_LOCOMOTOR: &str = "BattleBusLocomotor";
/// Retail SupplyTruck residual.
pub const SUPPLY_TRUCK_LOCOMOTOR: &str = "SupplyTruckLocomotor";
/// Retail AmericaVehicleAvenger residual.
pub const AVENGER_LOCOMOTOR: &str = "AvengerLocomotor";
/// Retail ChinaTankGattling residual.
pub const GATTLING_TANK_LOCOMOTOR: &str = "GattlingTankLocomotor";
/// Retail ChinaVehicleInfernoCannon residual.
pub const INFERNO_LOCOMOTOR: &str = "InfernoLocomotor";
/// Retail AmericaVehicleDozer residual.
pub const AMERICA_DOZER_LOCOMOTOR: &str = "AmericaVehicleDozerLocomotor";
/// Retail ChinaVehicleHelix residual.
pub const HELIX_LOCOMOTOR: &str = "HelixLocomotor";

// --- Wave 103 common-unit locomotor residual expand ---
/// Retail GLAVehicleBombTruck residual.
pub const BOMB_TRUCK_LOCOMOTOR: &str = "BombTruckLocomotor";
/// Retail ChinaVehicleTroopCrawler residual.
pub const TROOP_CRAWLER_LOCOMOTOR: &str = "TroopCrawlerLocomotor";
/// Retail GLAVehicleRadarVan residual.
pub const RADAR_VAN_LOCOMOTOR: &str = "RadarVanLocomotor";
/// Retail GLAVehicleToxinTruck residual.
pub const TOXIN_TRUCK_LOCOMOTOR: &str = "ToxinTruckLocomotor";
/// Retail AmericaVehicleChinook residual.
pub const CHINOOK_LOCOMOTOR: &str = "ChinookLocomotor";
/// Retail AmericaJetA10Thunderbolt residual.
pub const A10_THUNDERBOLT_LOCOMOTOR: &str = "A10ThunderboltLocomotor";
/// Retail AmericaJetB52 residual.
pub const B52_LOCOMOTOR: &str = "B52Locomotor";
/// Retail GLAVehicleCombatBike residual.
pub const COMBAT_BIKE_GROUND_LOCOMOTOR: &str = "CombatBikeGroundLocomotor";
/// Retail cliff member of GLAVehicleCombatBike's SET_NORMAL row.
pub const COMBAT_BIKE_CLIFF_LOCOMOTOR: &str = "CombatBikeCliffLocomotor";
/// Retail SET_SLUGGISH ground member of GLAVehicleCombatBike.
pub const COMBAT_BIKE_TERRORIST_GROUND_LOCOMOTOR: &str = "CombatBikeTerroristGroundLocomotor";
/// Retail SET_SLUGGISH cliff member of GLAVehicleCombatBike.
pub const COMBAT_BIKE_TERRORIST_CLIFF_LOCOMOTOR: &str = "CombatBikeTerroristCliffLocomotor";
/// Retail SET_TAXIING residual for jets / chinooks on the ground.
pub const AIRPLANE_TAXIING_LOCOMOTOR: &str = "AirplaneTaxiingLocomotor";
/// Retail Raptor / Stealth taxiing template.
pub const RAPTOR_TAXIING_LOCOMOTOR: &str = "RaptorTaxiingLocomotor";
/// Retail SET_SUPERSONIC residual for Raptor / Stealth attack dash.
pub const RAPTOR_SUPERSONIC_LOCOMOTOR: &str = "RaptorSupersonicLocomotor";
/// Retail SET_SUPERSONIC residual for Aurora attack dash.
pub const AURORA_SUPERSONIC_LOCOMOTOR: &str = "AuroraSupersonicLocomotor";
/// Retail SET_SUPERSONIC residual for MIG attack dash.
pub const MIG_SUPERSONIC_LOCOMOTOR: &str = "MIGSupersonicLocomotor";

/// Retail AmericaVehiclePOWTruck residual.
pub const POW_TRUCK_LOCOMOTOR: &str = "POWTruckLocomotor";
/// Retail Nuke_ChinaTankBattleMaster residual.
pub const NUCLEAR_BATTLE_MASTER_LOCOMOTOR: &str = "NuclearBattleMasterLocomotor";
/// Retail GLAInfantryJarmenKell residual.
pub const JARMEN_KELL_LOCOMOTOR: &str = "JarmenKellLocomotor";
/// Retail ChinaInfantryBlackLotus residual.
pub const BLACK_LOTUS_LOCOMOTOR: &str = "BlackLotusLocomotor";
/// Retail GLAInfantrySaboteur residual.
pub const SABOTEUR_GROUND_LOCOMOTOR: &str = "SaboteurGroundLocomotor";
/// Retail AmericaInfantryMissileDefender residual.
pub const MISSILE_DEFENDER_LOCOMOTOR: &str = "MissileDefenderLocomotor";
/// Retail GLAInfantryAngryMobNexus SET_NORMAL residual (Speed 18).
pub const ANGRY_MOB_NEXUS_LOCOMOTOR: &str = "AngryMobNexusLocomotor";
/// Retail cliff climber used on SET_NORMAL rows for Burton / Pathfinder / Saboteur.
pub const HUMAN_CLIFF_LOCOMOTOR: &str = "HumanCliffLocomotor";
/// Retail AmericaVehicleSpyDrone SET_NORMAL residual.
pub const SPY_DRONE_LOCOMOTOR: &str = "SpyDroneLocomotor";

/// Wave 81/92/103 residual seed table: (name, Speed, Acceleration, TurnRate deg).
/// Values match retail Locomotor.ini for common host units.
pub const HOST_LOCOMOTOR_SEED_RESIDUAL_TABLE: &[(&str, f32, f32, f32)] = &[
    (BASIC_HUMAN_LOCOMOTOR, 20.0, 100.0, 500.0),
    (REDGUARD_LOCOMOTOR, 25.0, 100.0, 500.0),
    (HUMVEE_LOCOMOTOR, 60.0, 1000.0, 180.0),
    (CRUSADER_LOCOMOTOR, 30.0, 1000.0, 180.0),
    (SCORPION_LOCOMOTOR, 40.0, 1000.0, 180.0),
    (BATTLE_MASTER_LOCOMOTOR, 25.0, 1000.0, 180.0),
    (TECHNICAL_LOCOMOTOR, 90.0, 100.0, 180.0),
    // Wave 81 deepen:
    (COLONEL_BURTON_GROUND_LOCOMOTOR, 30.0, 100.0, 500.0),
    (TOMAHAWK_LOCOMOTOR, 30.0, 1000.0, 180.0),
    (SCUD_LAUNCHER_LOCOMOTOR, 20.0, 160.0, 50.0),
    (QUAD_CANNON_LOCOMOTOR, 40.0, 1000.0, 180.0),
    (RAPTOR_JET_LOCOMOTOR, 175.0, 120.0, 120.0),
    // Wave 92 expand:
    (OVERLORD_LOCOMOTOR, 20.0, 15.0, 60.0),
    (MARAUDER_LOCOMOTOR, 40.0, 1000.0, 180.0),
    (DRAGON_LOCOMOTOR, 30.0, 1000.0, 180.0),
    (COMANCHE_LOCOMOTOR, 120.0, 60.0, 180.0),
    (MIG_LOCOMOTOR, 160.0, 110.0, 120.0),
    (ROCKET_BUGGY_LOCOMOTOR, 90.0, 90.0, 180.0),
    (BATTLE_BUS_LOCOMOTOR, 70.0, 1000.0, 90.0),
    (SUPPLY_TRUCK_LOCOMOTOR, 40.0, 240.0, 90.0),
    (AVENGER_LOCOMOTOR, 30.0, 1000.0, 180.0),
    (GATTLING_TANK_LOCOMOTOR, 40.0, 1000.0, 180.0),
    (INFERNO_LOCOMOTOR, 30.0, 1000.0, 120.0),
    (AMERICA_DOZER_LOCOMOTOR, 30.0, 30.0, 90.0),
    (HELIX_LOCOMOTOR, 75.0, 60.0, 180.0),
    // Wave 103 expand:
    (BOMB_TRUCK_LOCOMOTOR, 50.0, 1000.0, 90.0),
    (TROOP_CRAWLER_LOCOMOTOR, 40.0, 1000.0, 120.0),
    (RADAR_VAN_LOCOMOTOR, 40.0, 50.0, 180.0),
    (TOXIN_TRUCK_LOCOMOTOR, 30.0, 100.0, 180.0),
    (CHINOOK_LOCOMOTOR, 150.0, 60.0, 180.0),
    (A10_THUNDERBOLT_LOCOMOTOR, 120.0, 80.0, 200.0),
    (B52_LOCOMOTOR, 125.0, 60.0, 25.0),
    (COMBAT_BIKE_GROUND_LOCOMOTOR, 120.0, 90.0, 120.0),
    (POW_TRUCK_LOCOMOTOR, 60.0, 120.0, 120.0),
    (NUCLEAR_BATTLE_MASTER_LOCOMOTOR, 35.0, 1000.0, 180.0),
    (JARMEN_KELL_LOCOMOTOR, 30.0, 100.0, 500.0),
    (BLACK_LOTUS_LOCOMOTOR, 30.0, 100.0, 500.0),
    (SABOTEUR_GROUND_LOCOMOTOR, 30.0, 100.0, 500.0),
    (MISSILE_DEFENDER_LOCOMOTOR, 20.0, 100.0, 500.0),
    (ANGRY_MOB_NEXUS_LOCOMOTOR, 18.0, 100.0, 500.0),
    (SPY_DRONE_LOCOMOTOR, 75.0, 100.0, 180.0),
];

/// Logic FPS used by C++ Locomotor.ini unit conversion (Speed / 30 → dist/frame).
const LOGIC_FPS: f32 = 30.0;

static BOOTSTRAP_ATTEMPTED: AtomicBool = AtomicBool::new(false);
static SEED_COMPLETE: AtomicBool = AtomicBool::new(false);

/// Host-facing movement stats resolved from a LocomotorTemplate (dist/sec, rads/sec).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostMovementStats {
    pub max_speed: f32,
    pub acceleration: f32,
    pub turn_rate: f32,
}

/// Locomotor constants consumed by C++ `Drawable::calcPhysicsXform*`.
///
/// These values intentionally remain in the source `Locomotor.ini` units.
/// Unlike [`HostMovementStats`], they are not converted to per-second host
/// units: the GameClient visual calculation runs at the 30 Hz logic cadence
/// and applies the authored spring/damping coefficients directly.  Keeping
/// this as a separate value object lets the eventual per-visible-pass
/// collector carry exact authored constants without silently treating
/// movement-unit conversions as visual inputs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostLocomotorVisualPhysics {
    pub accel_pitch_limit: f32,
    pub decel_pitch_limit: f32,
    pub bounce_kick: f32,
    pub pitch_stiffness: f32,
    pub roll_stiffness: f32,
    pub pitch_damping: f32,
    pub roll_damping: f32,
    pub pitch_by_z_vel_coef: f32,
    pub thrust_roll: f32,
    pub wobble_rate: f32,
    pub min_wobble: f32,
    pub max_wobble: f32,
    pub forward_vel_coef: f32,
    pub lateral_vel_coef: f32,
    pub forward_accel_coef: f32,
    pub lateral_accel_coef: f32,
    pub uniform_axial_damping: f32,
    pub has_suspension: bool,
    pub maximum_wheel_extension: f32,
    pub maximum_wheel_compression: f32,
    pub wheel_turn_angle: f32,
    pub rudder_correction_degree: f32,
    pub rudder_correction_rate: f32,
    pub elevator_correction_degree: f32,
    pub elevator_correction_rate: f32,
}

/// Complete active-host projection of one exact Locomotor.ini template.
/// `HostMovementStats` deliberately predates the physics representation and
/// remains the light-weight spawn API; RiderChange needs this fuller record
/// because `AIUpdateInterface::chooseLocomotorSet` changes more than speed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostLocomotorBinding {
    pub movement: HostMovementStats,
    /// Exact authored inputs for the future GameClient physics-visual pass.
    pub visual_physics: HostLocomotorVisualPhysics,
    pub max_speed_damaged: f32,
    pub acceleration_damaged: f32,
    pub turn_rate_damaged: f32,
    pub braking: f32,
    pub min_speed: f32,
    pub min_turn_speed: f32,
    pub behavior_z: crate::game_logic::LocomotorBehaviorZ,
    pub appearance: crate::game_logic::LocomotorAppearance,
    pub extra_2d_friction: f32,
    pub apply_2d_friction_when_airborne: bool,
    /// C++ LocomotorTemplate::m_allowMotiveForceWhileAirborne.
    pub allow_motive_force_while_airborne: bool,
    /// C++ LocomotorTemplate::m_locomotorWorksWhenDead.
    pub locomotor_works_when_dead: bool,
    /// C++ LocomotorTemplate::m_airborneTargetingHeight (INT_MAX if omitted).
    pub airborne_targeting_height: i32,
    pub can_move_backward: bool,
    pub downhill_only: bool,
    pub max_lift: f32,
    pub max_lift_damaged: f32,
    pub speed_limit_z: f32,
    pub preferred_height: f32,
    pub preferred_height_damping: f32,
    pub circling_radius: f32,
    /// C++ LocomotorTemplate::m_maxThrustAngle (radians).
    pub max_thrust_angle: f32,
    pub turn_pivot_offset: f32,
    /// C++ LocomotorTemplate::m_wanderWidthFactor (0 = weave off).
    pub wander_width_factor: f32,
    /// C++ LocomotorTemplate::m_wanderLengthFactor (phase increment divisor).
    pub wander_length_factor: f32,
    pub stick_to_ground: bool,
    /// C++ LocomotorTemplate::m_closeEnoughDist (default 1.0).
    pub close_enough_dist: f32,
    /// Leftover `FLAG_CLOSE_ENOUGH_3D` / C++ `m_isCloseEnoughDist3D`.
    pub close_enough_dist_3d: bool,
    /// Leftover `parse_duration_real` of Common `SlideIntoPlaceTime` (msec → frames).
    /// C++ `m_ultraAccurateSlideIntoPlaceFactor` (Locomotor.cpp:474).
    pub ultra_accurate_slide_factor: f32,
    pub locomotor_surfaces: u32,
}

/// A complete authored C++ Locomotor set: every member resolves live and the
/// members' surface masks are pairwise disjoint, so the member for a position
/// is the unique surface intersection (C++ `LocomotorSet::findLocomotor`,
/// Locomotor.cpp:2773-2782).  Members may differ in behavior: retail
/// GLAVehicleCombatBike authors SET_SLUGGISH with ground Speed 90 vs cliff
/// Speed 60 dist/sec (INIZH Locomotor.ini), and C++ accepts that set verbatim
/// (`AIUpdateInterface::chooseLocomotorSetExplicit`, AIUpdate.cpp:819-833).
#[derive(Debug, Clone, PartialEq)]
pub struct HostCompleteLocomotorSet {
    /// First source declaration (the ground member in retail authoring),
    /// retained as the canonical active name for single-locomotor consumers.
    pub representative_name: String,
    /// Every exact authored locomotor name in source order.
    pub locomotor_names: Vec<String>,
    /// Union of the members' non-overlapping surface masks.
    pub locomotor_surfaces: u32,
}

/// Initialize / seed the Common LocomotorStore for host create_object binding.
/// Safe to call repeatedly.
///
/// Returns how many templates were added by this call (seed + filesystem load).
pub fn ensure_host_locomotor_store() -> usize {
    // create_object binds movement per placement. After start_new_game the
    // store is already filled; do not re-seed residual tables.
    if SEED_COMPLETE.load(Ordering::Relaxed) {
        return 0;
    }
    let mut added = 0usize;

    // Prefer real INI data when extracted game data is on disk (once).
    if !BOOTSTRAP_ATTEMPTED.swap(true, Ordering::Relaxed) {
        added += try_load_locomotor_ini_from_disk();
    }

    // RiderChange must validate the complete SET_NORMAL / SET_SLUGGISH
    // surface rows even in headless tests where the extracted Locomotor.ini
    // is unavailable.  Seed both retail Combat Bike members with every
    // currently-live physics field before the older speed-only generic
    // table fills the ground member.
    added += seed_exact_combat_bike_normal_locomotors();
    added += seed_exact_combat_bike_sluggish_locomotors();
    added += seed_exact_aircraft_set_switch_locomotors();
    added += seed_exact_human_cliff_locomotor();
    added += seed_exact_spy_drone_locomotor();

    // Always fill missing golden / Wave 81 common-unit locomotors.
    // (INI load may have BasicHuman but omit some residual names.)
    added += seed_known_host_locomotors();
    // Retail Chinook/Helix author SlideIntoPlaceTime=100 even when the
    // extracted Locomotor.ini is missing. Patch after generic seed so a
    // prior speed-only insert still gets the leftover duration.
    added += ensure_chinook_helix_slide_into_place();

    SEED_COMPLETE.store(true, Ordering::Relaxed);
    added
}

pub fn locomotor_name_for_unit(template_name: &str) -> Option<&'static str> {
    if crate::game_logic::host_angry_mob::is_angry_mob_nexus_template(template_name) {
        return Some(ANGRY_MOB_NEXUS_LOCOMOTOR);
    }
    if template_name.contains("AngryMob") {
        return Some(BASIC_HUMAN_LOCOMOTOR);
    }
    match template_name {
        // USA infantry (AmericaInfantryRanger → BasicHumanLocomotor)
        "USA_Ranger" | "GoldenRanger" | "AmericaInfantryRanger" => Some(BASIC_HUMAN_LOCOMOTOR),
        // Pathfinder / Colonel Burton share ground locomotor residual (Wave 81).
        "USA_Pathfinder"
        | "AmericaInfantryPathfinder"
        | "AirF_AmericaInfantryPathfinder"
        | "SupW_AmericaInfantryPathfinder"
        | "Lazr_AmericaInfantryPathfinder"
        | "USA_ColonelBurton"
        | "AmericaInfantryColonelBurton" => Some(COLONEL_BURTON_GROUND_LOCOMOTOR),
        // GLA infantry (GLAInfantryRebel → BasicHumanLocomotor)
        "GLA_Soldier" | "GLA_Rebel" | "GLAInfantryRebel" => Some(BASIC_HUMAN_LOCOMOTOR),
        // China infantry (ChinaInfantryRedguard → RedguardLocomotor @ 25)
        "China_RedGuard" | "China_Soldier" | "ChinaInfantryRedguard" => Some(REDGUARD_LOCOMOTOR),
        // USA vehicles
        "USA_Humvee" | "AmericaVehicleHumvee" => Some(HUMVEE_LOCOMOTOR),
        "USA_Crusader" | "USA_CrusaderTank" | "AmericaTankCrusader" => Some(CRUSADER_LOCOMOTOR),
        "USA_Tomahawk" | "AmericaVehicleTomahawk" => Some(TOMAHAWK_LOCOMOTOR),
        "USA_Avenger" | "AmericaVehicleAvenger" | "AmericaTankAvenger" => Some(AVENGER_LOCOMOTOR),
        "USA_Dozer" | "AmericaVehicleDozer" | "AmericaDozer" => Some(AMERICA_DOZER_LOCOMOTOR),
        // GLA vehicles
        "GLA_Technical" | "GLAVehicleTechnical" => Some(TECHNICAL_LOCOMOTOR),
        "GLA_Scorpion" | "GLA_ScorpionTank" | "GLATankScorpion" => Some(SCORPION_LOCOMOTOR),
        "GLA_ScudLauncher" | "GLAVehicleSCUDLauncher" | "GLAVehicleScudLauncher" => {
            Some(SCUD_LAUNCHER_LOCOMOTOR)
        }
        "GLA_QuadCannon" | "GLAVehicleQuadCannon" => Some(QUAD_CANNON_LOCOMOTOR),
        "GLA_Marauder" | "GLATankMarauder" => Some(MARAUDER_LOCOMOTOR),
        "GLA_RocketBuggy" | "GLAVehicleRocketBuggy" => Some(ROCKET_BUGGY_LOCOMOTOR),
        "GLA_BattleBus" | "GLAVehicleBattleBus" => Some(BATTLE_BUS_LOCOMOTOR),
        // China vehicles
        "China_BattleTank" | "ChinaTankBattleMaster" => Some(BATTLE_MASTER_LOCOMOTOR),
        "China_Overlord" | "ChinaTankOverlord" => Some(OVERLORD_LOCOMOTOR),
        "China_Dragon" | "ChinaTankDragon" => Some(DRAGON_LOCOMOTOR),
        "China_Gattling" | "ChinaTankGattling" => Some(GATTLING_TANK_LOCOMOTOR),
        "China_Inferno" | "ChinaVehicleInfernoCannon" => Some(INFERNO_LOCOMOTOR),
        // USA aircraft residual
        "USA_Raptor" | "AmericaJetRaptor" => Some(RAPTOR_JET_LOCOMOTOR),
        "USA_Comanche" | "AmericaVehicleComanche" => Some(COMANCHE_LOCOMOTOR),
        // China aircraft residual
        "China_MIG" | "ChinaJetMIG" | "ChinaJetMiG" => Some(MIG_LOCOMOTOR),
        "China_Helix" | "ChinaVehicleHelix" => Some(HELIX_LOCOMOTOR),
        // Supply trucks (all factions share residual)
        "AmericaVehicleSupplyTruck" | "ChinaVehicleSupplyTruck" | "GLAVehicleSupplyTruck" => {
            Some(SUPPLY_TRUCK_LOCOMOTOR)
        }
        // Wave 103 expand unit → SET_NORMAL residual
        "GLA_BombTruck" | "GLAVehicleBombTruck" => Some(BOMB_TRUCK_LOCOMOTOR),
        "China_TroopCrawler" | "ChinaVehicleTroopCrawler" => Some(TROOP_CRAWLER_LOCOMOTOR),
        "GLA_RadarVan" | "GLAVehicleRadarVan" => Some(RADAR_VAN_LOCOMOTOR),
        "GLA_ToxinTractor" | "GLAVehicleToxinTruck" => Some(TOXIN_TRUCK_LOCOMOTOR),
        "USA_Chinook" | "AmericaVehicleChinook" => Some(CHINOOK_LOCOMOTOR),
        "USA_A10" | "AmericaJetA10Thunderbolt" => Some(A10_THUNDERBOLT_LOCOMOTOR),
        "USA_B52" | "AmericaJetB52" => Some(B52_LOCOMOTOR),
        "GLA_CombatBike" | "GLAVehicleCombatBike" => Some(COMBAT_BIKE_GROUND_LOCOMOTOR),
        "USA_POWTruck" | "AmericaVehiclePOWTruck" => Some(POW_TRUCK_LOCOMOTOR),
        "Nuke_ChinaTankBattleMaster" | "China_NuclearBattleMaster" => {
            Some(NUCLEAR_BATTLE_MASTER_LOCOMOTOR)
        }
        "GLA_JarmenKell" | "GLAInfantryJarmenKell" => Some(JARMEN_KELL_LOCOMOTOR),
        "China_BlackLotus" | "ChinaInfantryBlackLotus" => Some(BLACK_LOTUS_LOCOMOTOR),
        "GLA_Saboteur" | "GLAInfantrySaboteur" => Some(SABOTEUR_GROUND_LOCOMOTOR),
        "USA_MissileDefender" | "AmericaInfantryMissileDefender" => {
            Some(MISSILE_DEFENDER_LOCOMOTOR)
        }
        "USA_SpyDrone" | "AmericaVehicleSpyDrone" => Some(SPY_DRONE_LOCOMOTOR),
        _ => None,
    }
}

/// Authored SET_NORMAL members in declaration order for known multi-surface units.
/// C++ `chooseGoodLocomotorFromCurrentSet` picks one by cell surface.
pub fn locomotor_set_names_for_unit(template_name: &str) -> Vec<String> {
    let names: &[&str] = match template_name {
        "GLA_CombatBike" | "GLAVehicleCombatBike" => {
            &[COMBAT_BIKE_GROUND_LOCOMOTOR, COMBAT_BIKE_CLIFF_LOCOMOTOR]
        }
        "USA_ColonelBurton"
        | "AmericaInfantryColonelBurton"
        | "USA_Pathfinder"
        | "AmericaInfantryPathfinder"
        | "Lazr_AmericaInfantryPathfinder" => {
            &[COLONEL_BURTON_GROUND_LOCOMOTOR, HUMAN_CLIFF_LOCOMOTOR]
        }
        "GLA_Saboteur" | "GLAInfantrySaboteur" => {
            &[SABOTEUR_GROUND_LOCOMOTOR, HUMAN_CLIFF_LOCOMOTOR]
        }
        _ => return Vec::new(),
    };
    names.iter().map(|n| (*n).to_string()).collect()
}

/// Wave 81 residual honesty: common-unit locomotor seed residual table.
///
/// Ensures golden + Wave 81 deepen names are present with retail Speed residual.
/// Fail-closed: not full multi-surface / SET_PANIC / pitch-roll matrix.
pub fn honesty_locomotor_residual_table_wave81() -> bool {
    let _ = ensure_host_locomotor_store();
    if HOST_LOCOMOTOR_SEED_RESIDUAL_TABLE.len() < 12 {
        return false;
    }
    // Core + Wave 81 names present in table.
    let names_ok = [
        BASIC_HUMAN_LOCOMOTOR,
        COLONEL_BURTON_GROUND_LOCOMOTOR,
        TOMAHAWK_LOCOMOTOR,
        SCUD_LAUNCHER_LOCOMOTOR,
        QUAD_CANNON_LOCOMOTOR,
        RAPTOR_JET_LOCOMOTOR,
        TECHNICAL_LOCOMOTOR,
        HUMVEE_LOCOMOTOR,
    ]
    .iter()
    .all(|n| {
        HOST_LOCOMOTOR_SEED_RESIDUAL_TABLE
            .iter()
            .any(|(name, ..)| name == n)
    });
    if !names_ok {
        return false;
    }
    // Seeded / loaded templates resolve retail speeds.
    let speed_ok = |name: &str, expected: f32| {
        movement_from_store(name)
            .map(|m| (m.max_speed - expected).abs() < 0.5)
            .unwrap_or(false)
    };
    names_ok
        && speed_ok(BASIC_HUMAN_LOCOMOTOR, 20.0)
        && speed_ok(COLONEL_BURTON_GROUND_LOCOMOTOR, 30.0)
        && speed_ok(TOMAHAWK_LOCOMOTOR, 30.0)
        && speed_ok(SCUD_LAUNCHER_LOCOMOTOR, 20.0)
        && speed_ok(QUAD_CANNON_LOCOMOTOR, 40.0)
        && speed_ok(RAPTOR_JET_LOCOMOTOR, 175.0)
        && locomotor_name_for_unit("AmericaInfantryPathfinder")
            == Some(COLONEL_BURTON_GROUND_LOCOMOTOR)
        && locomotor_name_for_unit("AmericaVehicleTomahawk") == Some(TOMAHAWK_LOCOMOTOR)
        && locomotor_name_for_unit("GLAVehicleQuadCannon") == Some(QUAD_CANNON_LOCOMOTOR)
        && locomotor_name_for_unit("AmericaJetRaptor") == Some(RAPTOR_JET_LOCOMOTOR)
}

/// Wave 92 residual honesty: expand common-unit locomotor residual names.
///
/// Adds Overlord / Marauder / Dragon / Comanche / MIG / RocketBuggy / BattleBus /
/// SupplyTruck / Avenger / GattlingTank / Inferno / Dozer / Helix residual.
/// Fail-closed: not full multi-surface / SET_PANIC / pitch-roll matrix.
pub fn honesty_locomotor_residual_expand_wave92() -> bool {
    let _ = ensure_host_locomotor_store();
    // Wave 81 base + Wave 92 expand (≥ 25 rows).
    if HOST_LOCOMOTOR_SEED_RESIDUAL_TABLE.len() < 25 {
        return false;
    }
    let wave92_names = [
        OVERLORD_LOCOMOTOR,
        MARAUDER_LOCOMOTOR,
        DRAGON_LOCOMOTOR,
        COMANCHE_LOCOMOTOR,
        MIG_LOCOMOTOR,
        ROCKET_BUGGY_LOCOMOTOR,
        BATTLE_BUS_LOCOMOTOR,
        SUPPLY_TRUCK_LOCOMOTOR,
        AVENGER_LOCOMOTOR,
        GATTLING_TANK_LOCOMOTOR,
        INFERNO_LOCOMOTOR,
        AMERICA_DOZER_LOCOMOTOR,
        HELIX_LOCOMOTOR,
    ];
    let names_ok = wave92_names.iter().all(|n| {
        HOST_LOCOMOTOR_SEED_RESIDUAL_TABLE
            .iter()
            .any(|(name, ..)| name == n)
    });
    if !names_ok {
        return false;
    }
    let speed_ok = |name: &str, expected: f32| {
        movement_from_store(name)
            .map(|m| (m.max_speed - expected).abs() < 0.5)
            .unwrap_or(false)
    };
    names_ok
        && speed_ok(OVERLORD_LOCOMOTOR, 20.0)
        && speed_ok(MARAUDER_LOCOMOTOR, 40.0)
        && speed_ok(DRAGON_LOCOMOTOR, 30.0)
        && speed_ok(COMANCHE_LOCOMOTOR, 120.0)
        && speed_ok(MIG_LOCOMOTOR, 160.0)
        && speed_ok(ROCKET_BUGGY_LOCOMOTOR, 90.0)
        && speed_ok(BATTLE_BUS_LOCOMOTOR, 70.0)
        && speed_ok(SUPPLY_TRUCK_LOCOMOTOR, 40.0)
        && speed_ok(AVENGER_LOCOMOTOR, 30.0)
        && speed_ok(GATTLING_TANK_LOCOMOTOR, 40.0)
        && speed_ok(INFERNO_LOCOMOTOR, 30.0)
        && speed_ok(AMERICA_DOZER_LOCOMOTOR, 30.0)
        && speed_ok(HELIX_LOCOMOTOR, 75.0)
        && locomotor_name_for_unit("ChinaTankOverlord") == Some(OVERLORD_LOCOMOTOR)
        && locomotor_name_for_unit("GLATankMarauder") == Some(MARAUDER_LOCOMOTOR)
        && locomotor_name_for_unit("ChinaTankDragon") == Some(DRAGON_LOCOMOTOR)
        && locomotor_name_for_unit("AmericaVehicleComanche") == Some(COMANCHE_LOCOMOTOR)
        && locomotor_name_for_unit("ChinaJetMIG") == Some(MIG_LOCOMOTOR)
        && locomotor_name_for_unit("GLAVehicleRocketBuggy") == Some(ROCKET_BUGGY_LOCOMOTOR)
        && locomotor_name_for_unit("AmericaTankAvenger") == Some(AVENGER_LOCOMOTOR)
        && locomotor_name_for_unit("ChinaVehicleHelix") == Some(HELIX_LOCOMOTOR)
        && honesty_locomotor_residual_table_wave81()
}

/// Wave 103 residual honesty: expand more common-unit locomotor residual names.
///
/// Adds BombTruck / TroopCrawler / RadarVan / ToxinTruck / Chinook / A10 / B52 /
/// CombatBike / POWTruck / NuclearBattleMaster / JarmenKell / BlackLotus /
/// Saboteur / MissileDefender residual.
/// Fail-closed: not full multi-surface / SET_PANIC / pitch-roll matrix.
pub fn honesty_locomotor_residual_expand_wave103() -> bool {
    let _ = ensure_host_locomotor_store();
    if !honesty_locomotor_residual_expand_wave92() {
        return false;
    }
    // Wave 81+92+103 (≥ 39 rows).
    if HOST_LOCOMOTOR_SEED_RESIDUAL_TABLE.len() < 39 {
        return false;
    }
    let wave103_names = [
        BOMB_TRUCK_LOCOMOTOR,
        TROOP_CRAWLER_LOCOMOTOR,
        RADAR_VAN_LOCOMOTOR,
        TOXIN_TRUCK_LOCOMOTOR,
        CHINOOK_LOCOMOTOR,
        A10_THUNDERBOLT_LOCOMOTOR,
        B52_LOCOMOTOR,
        COMBAT_BIKE_GROUND_LOCOMOTOR,
        POW_TRUCK_LOCOMOTOR,
        NUCLEAR_BATTLE_MASTER_LOCOMOTOR,
        JARMEN_KELL_LOCOMOTOR,
        BLACK_LOTUS_LOCOMOTOR,
        SABOTEUR_GROUND_LOCOMOTOR,
        MISSILE_DEFENDER_LOCOMOTOR,
    ];
    let names_ok = wave103_names.iter().all(|n| {
        HOST_LOCOMOTOR_SEED_RESIDUAL_TABLE
            .iter()
            .any(|(name, ..)| name == n)
    });
    if !names_ok {
        return false;
    }
    let speed_ok = |name: &str, expected: f32| {
        movement_from_store(name)
            .map(|m| (m.max_speed - expected).abs() < 0.5)
            .unwrap_or(false)
    };
    names_ok
        && speed_ok(BOMB_TRUCK_LOCOMOTOR, 50.0)
        && speed_ok(TROOP_CRAWLER_LOCOMOTOR, 40.0)
        && speed_ok(RADAR_VAN_LOCOMOTOR, 40.0)
        && speed_ok(TOXIN_TRUCK_LOCOMOTOR, 30.0)
        && speed_ok(CHINOOK_LOCOMOTOR, 150.0)
        && speed_ok(A10_THUNDERBOLT_LOCOMOTOR, 120.0)
        && speed_ok(B52_LOCOMOTOR, 125.0)
        && speed_ok(COMBAT_BIKE_GROUND_LOCOMOTOR, 120.0)
        && speed_ok(POW_TRUCK_LOCOMOTOR, 60.0)
        && speed_ok(NUCLEAR_BATTLE_MASTER_LOCOMOTOR, 35.0)
        && speed_ok(JARMEN_KELL_LOCOMOTOR, 30.0)
        && speed_ok(BLACK_LOTUS_LOCOMOTOR, 30.0)
        && speed_ok(SABOTEUR_GROUND_LOCOMOTOR, 30.0)
        && speed_ok(MISSILE_DEFENDER_LOCOMOTOR, 20.0)
        && locomotor_name_for_unit("GLAVehicleBombTruck") == Some(BOMB_TRUCK_LOCOMOTOR)
        && locomotor_name_for_unit("ChinaVehicleTroopCrawler") == Some(TROOP_CRAWLER_LOCOMOTOR)
        && locomotor_name_for_unit("GLAVehicleRadarVan") == Some(RADAR_VAN_LOCOMOTOR)
        && locomotor_name_for_unit("GLAVehicleToxinTruck") == Some(TOXIN_TRUCK_LOCOMOTOR)
        && locomotor_name_for_unit("AmericaVehicleChinook") == Some(CHINOOK_LOCOMOTOR)
        && locomotor_name_for_unit("AmericaJetA10Thunderbolt") == Some(A10_THUNDERBOLT_LOCOMOTOR)
        && locomotor_name_for_unit("AmericaJetB52") == Some(B52_LOCOMOTOR)
        && locomotor_name_for_unit("GLAVehicleCombatBike") == Some(COMBAT_BIKE_GROUND_LOCOMOTOR)
        && locomotor_name_for_unit("GLAInfantryJarmenKell") == Some(JARMEN_KELL_LOCOMOTOR)
        && locomotor_name_for_unit("ChinaInfantryBlackLotus") == Some(BLACK_LOTUS_LOCOMOTOR)
        && locomotor_name_for_unit("AmericaInfantryMissileDefender")
            == Some(MISSILE_DEFENDER_LOCOMOTOR)
}

/// Resolve host Movement stats from the Locomotor catalog by template name.
/// Ensures the store is bootstrapped first. Returns None if missing or unusable.
pub fn resolve_host_movement(locomotor_name: &str) -> Option<HostMovementStats> {
    let _ = ensure_host_locomotor_store();
    movement_from_store(locomotor_name)
}

/// Resolve every Locomotor.ini field Main currently has a live Object field
/// for.  Callers must use this for stateful swaps instead of copying the
/// template's normal movement or reusing a rider-name heuristic.
pub fn resolve_host_locomotor_binding(locomotor_name: &str) -> Option<HostLocomotorBinding> {
    let _ = ensure_host_locomotor_store();
    if locomotor_name == CHINOOK_LOCOMOTOR || locomotor_name == HELIX_LOCOMOTOR {
        let _ = ensure_chinook_helix_slide_into_place();
    }
    let store = get_locomotor_store();
    let template = store.find_template(locomotor_name)?;
    if template.max_speed <= 0.0 {
        return None;
    }
    host_locomotor_binding_from_template(template)
}

/// Resolve a C++ `Locomotor = SET_* ...` row as a complete authored set:
/// every member must resolve live with pairwise-disjoint surfaces.  C++ jams
/// the whole row into the AI's LocomotorSet verbatim
/// (`AIUpdateInterface::chooseLocomotorSetExplicit`, AIUpdate.cpp:819-833)
/// and picks the active member by terrain surface later
/// (`chooseGoodLocomotorFromCurrentSet`, AIUpdate.cpp:828-853, through
/// `LocomotorSet::findLocomotor`, Locomotor.cpp:2773-2782), so members may
/// differ in behavior (retail Combat Bike SET_SLUGGISH: 90 ground / 60 cliff).
pub fn resolve_complete_host_locomotor_set(
    locomotor_names: &[String],
) -> Option<HostCompleteLocomotorSet> {
    let first_name = locomotor_names.first()?.trim();
    if first_name.is_empty() || first_name.eq_ignore_ascii_case("none") {
        return None;
    }
    let mut canonical_names = Vec::with_capacity(locomotor_names.len());
    let mut surfaces = 0u32;
    for raw_name in locomotor_names {
        let name = raw_name.trim();
        if name.is_empty()
            || name.eq_ignore_ascii_case("none")
            || canonical_names
                .iter()
                .any(|prior: &String| prior.eq_ignore_ascii_case(name))
        {
            return None;
        }
        let binding = resolve_host_locomotor_binding(name)?;
        if binding.locomotor_surfaces == 0 || (surfaces & binding.locomotor_surfaces) != 0 {
            return None;
        }
        surfaces |= binding.locomotor_surfaces;
        canonical_names.push(name.to_string());
    }
    Some(HostCompleteLocomotorSet {
        representative_name: first_name.to_string(),
        locomotor_names: canonical_names,
        locomotor_surfaces: surfaces,
    })
}

/// C++ `AIUpdateInterface::chooseGoodLocomotorFromCurrentSet`
/// (AIUpdate.cpp:828-853): the first set member whose surfaces intersect the
/// position surface mask; when none intersects, the GROUND member
/// (AIUpdate.cpp:849-852); otherwise the first authored member.
pub fn choose_host_locomotor_set_member_for_surfaces(
    locomotor_names: &[String],
    acceptable: u32,
) -> Option<(String, HostLocomotorBinding)> {
    let mut candidates = [
        choose_best_locomotor_name_for_surfaces(locomotor_names, acceptable),
        choose_best_locomotor_name_for_surfaces(
            locomotor_names,
            crate::game_logic::object::LOCO_SURFACE_GROUND,
        ),
        locomotor_names.first().map(|name| name.trim().to_string()),
    ];
    for name in candidates.iter_mut().flatten() {
        if name.is_empty() || name.eq_ignore_ascii_case("none") {
            continue;
        }
        if let Some(binding) = resolve_host_locomotor_binding(name) {
            return Some((std::mem::take(name), binding));
        }
    }
    None
}

/// C++ `Pathfinder::validLocomotorSurfacesForCellType` (AIPathfind.cpp:4734-4757).
pub fn valid_locomotor_surfaces_for_cell_type(
    ty: gamelogic::ai::pathfind_astar::PathfindCellType,
) -> u32 {
    use crate::game_logic::object::{
        LOCO_SURFACE_AIR, LOCO_SURFACE_CLIFF, LOCO_SURFACE_GROUND, LOCO_SURFACE_RUBBLE,
        LOCO_SURFACE_WATER,
    };
    use gamelogic::ai::pathfind_astar::PathfindCellType;
    match ty {
        PathfindCellType::Obstacle
        | PathfindCellType::Impassable
        | PathfindCellType::BridgeImpassable => LOCO_SURFACE_AIR,
        PathfindCellType::Clear => LOCO_SURFACE_GROUND | LOCO_SURFACE_AIR,
        PathfindCellType::Water => LOCO_SURFACE_WATER | LOCO_SURFACE_AIR,
        PathfindCellType::Rubble => LOCO_SURFACE_RUBBLE | LOCO_SURFACE_AIR,
        PathfindCellType::Cliff => LOCO_SURFACE_CLIFF | LOCO_SURFACE_AIR,
    }
}

/// C++ `LocomotorSet::findLocomotor` — first member whose surfaces intersect.
pub fn choose_best_locomotor_name_for_surfaces(
    locomotor_names: &[String],
    acceptable: u32,
) -> Option<String> {
    for raw in locomotor_names {
        let name = raw.trim();
        if name.is_empty() || name.eq_ignore_ascii_case("none") {
            continue;
        }
        let Some(binding) = resolve_host_locomotor_binding(name) else {
            continue;
        };
        if (binding.locomotor_surfaces & acceptable) != 0 {
            return Some(name.to_string());
        }
    }
    None
}

/// Canonical SET_* names that resolve, including heterogeneous members.
pub fn resolve_host_locomotor_set_names(locomotor_names: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(locomotor_names.len());
    for raw in locomotor_names {
        let name = raw.trim();
        if name.is_empty() || name.eq_ignore_ascii_case("none") {
            continue;
        }
        if out
            .iter()
            .any(|prior: &String| prior.eq_ignore_ascii_case(name))
        {
            continue;
        }
        if resolve_host_locomotor_binding(name).is_some() {
            out.push(name.to_string());
        }
    }
    out
}

/// Convert a Common LocomotorTemplate (per-frame) into host Movement units (per-sec).
///
/// C++ Locomotor.ini: Speed is dist/sec, stored as Speed/30 dist/frame.
/// Host `Movement.max_speed` is dist/sec (used with dt seconds).
pub fn movement_from_store(name: &str) -> Option<HostMovementStats> {
    let store = get_locomotor_store();
    let t = store.find_template(name)?;
    if t.max_speed <= 0.0 {
        return None;
    }
    Some(host_stats_from_template(t))
}

fn host_stats_from_template(t: &LocomotorTemplate) -> HostMovementStats {
    // Store: dist/frame, dist/frame², rads/frame → host: dist/sec, dist/sec², rads/sec.
    let max_speed = t.max_speed * LOGIC_FPS;
    let acceleration = if t.acceleration > 0.0 {
        t.acceleration * LOGIC_FPS * LOGIC_FPS
    } else {
        // Fallback: snappy enough not to crawl; retail infantry uses 100.
        100.0
    };
    let turn_rate = if t.max_turn_rate > 0.0 {
        t.max_turn_rate * LOGIC_FPS
    } else {
        std::f32::consts::PI
    };
    HostMovementStats {
        max_speed,
        acceleration,
        turn_rate,
    }
}

fn host_locomotor_binding_from_template(t: &LocomotorTemplate) -> Option<HostLocomotorBinding> {
    const FPS: f32 = LOGIC_FPS;
    let movement = host_stats_from_template(t);
    let acceleration = |value: f32| {
        if value > 0.0 {
            value * FPS * FPS
        } else {
            movement.acceleration
        }
    };
    let turn_rate = |value: f32| {
        if value > 0.0 {
            value * FPS
        } else {
            movement.turn_rate
        }
    };
    let surface_mask = t.surfaces.0;
    // A valid C++ Locomotor set can contain distinct ground and cliff
    // templates.  This binding is one selected template, so its own source
    // surface mask—not the object's generic KindOf fallback—is authoritative.
    if surface_mask == 0 {
        return None;
    }
    let behavior_z = match t.behavior_z {
        SourceLocomotorBehaviorZ::NoZMotiveForce => {
            crate::game_logic::LocomotorBehaviorZ::NoZMotiveForce
        }
        SourceLocomotorBehaviorZ::SeaLevel => crate::game_logic::LocomotorBehaviorZ::SeaLevel,
        SourceLocomotorBehaviorZ::SurfaceRelativeHeight => {
            crate::game_logic::LocomotorBehaviorZ::SurfaceRelativeHeight
        }
        SourceLocomotorBehaviorZ::AbsoluteHeight => {
            crate::game_logic::LocomotorBehaviorZ::AbsoluteHeight
        }
        SourceLocomotorBehaviorZ::SmoothRelativeToHighestLayer => {
            crate::game_logic::LocomotorBehaviorZ::SmoothRelativeToHighestLayer
        }
        SourceLocomotorBehaviorZ::FixedSurfaceRelativeHeight => {
            crate::game_logic::LocomotorBehaviorZ::FixedSurfaceRelativeHeight
        }
        SourceLocomotorBehaviorZ::FixedAbsoluteHeight => {
            crate::game_logic::LocomotorBehaviorZ::FixedAbsoluteHeight
        }
        SourceLocomotorBehaviorZ::RelativeToGroundAndBuildings => {
            crate::game_logic::LocomotorBehaviorZ::RelativeToGroundAndBuildings
        }
    };
    let leftover = gamelogic::locomotor::ini_bridge::from_common_ini_template(t);

    Some(HostLocomotorBinding {
        movement,
        visual_physics: HostLocomotorVisualPhysics {
            accel_pitch_limit: t.accel_pitch_limit,
            decel_pitch_limit: t.decel_pitch_limit,
            bounce_kick: t.bounce_kick,
            pitch_stiffness: t.pitch_stiffness,
            roll_stiffness: t.roll_stiffness,
            pitch_damping: t.pitch_damping,
            roll_damping: t.roll_damping,
            pitch_by_z_vel_coef: t.pitch_by_z_vel_coef,
            thrust_roll: t.thrust_roll,
            wobble_rate: t.wobble_rate,
            min_wobble: t.min_wobble,
            max_wobble: t.max_wobble,
            forward_vel_coef: t.forward_vel_coef,
            lateral_vel_coef: t.lateral_vel_coef,
            forward_accel_coef: t.forward_accel_coef,
            lateral_accel_coef: t.lateral_accel_coef,
            uniform_axial_damping: t.uniform_axial_damping,
            has_suspension: t.has_suspension,
            maximum_wheel_extension: t.maximum_wheel_extension,
            maximum_wheel_compression: t.maximum_wheel_compression,
            wheel_turn_angle: t.wheel_turn_angle,
            rudder_correction_degree: t.rudder_correction_degree,
            rudder_correction_rate: t.rudder_correction_rate,
            elevator_correction_degree: t.elevator_correction_degree,
            elevator_correction_rate: t.elevator_correction_rate,
        },
        max_speed_damaged: if t.max_speed_damaged > 0.0 {
            t.max_speed_damaged * FPS
        } else {
            movement.max_speed
        },
        acceleration_damaged: acceleration(t.acceleration_damaged),
        turn_rate_damaged: turn_rate(t.max_turn_rate_damaged),
        braking: {
            // C++ omitted Braking stays BIGNUM (Locomotor.cpp:270). Common
            // store uses 0.0 for that case; convert authored values only.
            const LOCO_BIGNUM: f32 = 99999.0;
            if t.braking <= 0.0 || t.braking >= LOCO_BIGNUM {
                LOCO_BIGNUM
            } else {
                t.braking * FPS * FPS
            }
        },
        min_speed: t.min_speed,
        min_turn_speed: t.min_turn_speed,
        behavior_z,
        appearance: match t.appearance {
            SourceLocomotorAppearance::LegsTWO => crate::game_logic::LocomotorAppearance::LegsTwo,
            SourceLocomotorAppearance::WheelsFOUR => {
                crate::game_logic::LocomotorAppearance::WheelsFour
            }
            SourceLocomotorAppearance::Treads => crate::game_logic::LocomotorAppearance::Treads,
            SourceLocomotorAppearance::Hover => crate::game_logic::LocomotorAppearance::Hover,
            SourceLocomotorAppearance::Wings => crate::game_logic::LocomotorAppearance::Wings,
            SourceLocomotorAppearance::Thrust => crate::game_logic::LocomotorAppearance::Thrust,
            SourceLocomotorAppearance::Climber => crate::game_logic::LocomotorAppearance::Climber,
            SourceLocomotorAppearance::Other => crate::game_logic::LocomotorAppearance::Other,
            SourceLocomotorAppearance::Motorcycle => {
                crate::game_logic::LocomotorAppearance::Motorcycle
            }
        },
        extra_2d_friction: t.extra_2d_friction,
        apply_2d_friction_when_airborne: t.apply_2d_friction_when_airborne,
        allow_motive_force_while_airborne: t.allow_motive_force_while_airborne,
        locomotor_works_when_dead: t.locomotor_works_when_dead,
        airborne_targeting_height: if t.airborne_targeting_height == 0 {
            i32::MAX
        } else {
            t.airborne_targeting_height
        },
        can_move_backward: t.can_move_backward,
        downhill_only: t.downhill_only,
        // Common already stored Lift/LiftDamaged as parseAccelerationReal (/900).
        // handleBehaviorZ / calcLiftToUseAtPt are dt-less per-frame Euler, so
        // do not inflate back to per-sec² (C++ Locomotor.cpp:428-437).
        max_lift: t.lift,
        max_lift_damaged: if t.lift_damaged >= 0.0 {
            t.lift_damaged
        } else {
            t.lift
        },
        // Common stores SpeedLimitZ as raw dist/sec; leftover ini_bridge
        // applies parseVelocityReal (÷30). Omitted BIGNUM stays unconverted.
        speed_limit_z: if t.speed_limit_z >= 1_000_000.0 {
            999_999.0
        } else {
            t.speed_limit_z / FPS
        },
        preferred_height: t.preferred_height,
        preferred_height_damping: t.preferred_height_damping,
        circling_radius: t.circling_radius,
        // Common INI store keeps authored degrees; C++ parseAngleReal → radians.
        max_thrust_angle: t.max_thrust_angle * std::f32::consts::PI / 180.0,
        turn_pivot_offset: t.turn_pivot_offset,
        wander_width_factor: t.wander_width_factor,
        wander_length_factor: if t.wander_length_factor.abs() < 1.0e-6 {
            1.0
        } else {
            t.wander_length_factor
        },
        stick_to_ground: t.stick_to_ground,
        close_enough_dist: if t.close_enough_dist.is_finite() && t.close_enough_dist >= 0.0 {
            t.close_enough_dist
        } else {
            1.0
        },
        // Common stores SlideIntoPlaceTime as raw msec; leftover ini_bridge
        // parse_duration_real converts msec → frames (100 msec → 3).
        close_enough_dist_3d: leftover.is_close_enough_dist_3d,
        ultra_accurate_slide_factor: leftover.ultra_accurate_slide_factor,
        locomotor_surfaces: surface_mask,
    })
}

/// Apply every live Object field this binding owns, including damaged
/// speed/accel, INI Braking, and unique legs-wander phase
/// (C++ Locomotor.cpp:647-649).
pub fn apply_host_locomotor_binding(
    object: &mut crate::game_logic::object::Object,
    binding: &HostLocomotorBinding,
) {
    object.movement.max_speed = binding.movement.max_speed;
    object.movement.max_speed_damaged = binding.max_speed_damaged;
    object.movement.acceleration = binding.movement.acceleration;
    object.movement.acceleration_damaged = binding.acceleration_damaged;
    object.movement.turn_rate = binding.movement.turn_rate;
    object.movement.turn_rate_damaged = binding.turn_rate_damaged;
    object.braking = binding.braking;
    object.min_speed = binding.min_speed;
    object.min_turn_speed = binding.min_turn_speed;
    object.loco_behavior_z = binding.behavior_z;
    object.loco_appearance = binding.appearance;
    object.loco_extra_2d_friction = binding.extra_2d_friction;
    object.loco_apply_2d_friction_airborne = binding.apply_2d_friction_when_airborne;
    object.allow_motive_force_while_airborne = binding.allow_motive_force_while_airborne;
    object.locomotor_works_when_dead = binding.locomotor_works_when_dead;
    object.airborne_targeting_height = binding.airborne_targeting_height;
    object.can_move_backward = binding.can_move_backward;
    object.downhill_only = binding.downhill_only;
    object.max_lift = binding.max_lift;
    object.max_lift_damaged = binding.max_lift_damaged;
    object.speed_limit_z = binding.speed_limit_z;
    object.loco_preferred_height = binding.preferred_height;
    object.loco_preferred_height_damping = binding.preferred_height_damping;
    object.circling_radius = binding.circling_radius;
    object.max_thrust_angle = binding.max_thrust_angle;
    object.turn_pivot_offset = binding.turn_pivot_offset;
    object.stick_to_ground = binding.stick_to_ground;
    object.locomotor_surfaces = binding.locomotor_surfaces;
    if object.close_enough_dist.is_none() {
        object.close_enough_dist = Some(binding.close_enough_dist);
    }
    object.close_enough_dist_3d = binding.close_enough_dist_3d;
    object.wander_width_factor = binding.wander_width_factor;
    object.ultra_accurate_slide_factor = binding.ultra_accurate_slide_factor;
    seed_wander_phase(object, binding.wander_length_factor);
    object.set_locomotor_physics_options();
    object.record_host_locomotor();
    object.record_host_movement();
}

/// C++ Locomotor ctor: random angle offset + increment / wanderLengthFactor.
fn seed_wander_phase(object: &mut crate::game_logic::object::Object, wander_length_factor: f32) {
    use crate::game_logic::host_rng_residual::{pure_logic_random_int, pure_logic_random_real};
    let seed = object.id.0;
    let pi = std::f32::consts::PI;
    object.wander_angle_offset = pure_logic_random_real(seed, 20, -pi / 6.0, pi / 6.0);
    let wander_len = if wander_length_factor.abs() < 1.0e-6 {
        1.0
    } else {
        wander_length_factor
    };
    object.wander_offset_increment =
        (pi / 40.0) * (pure_logic_random_real(seed, 21, 0.8, 1.2) / wander_len);
    object.wander_offset_increasing = pure_logic_random_int(seed, 22, 0, 1) != 0;
}

fn same_host_locomotor_behavior(left: &HostLocomotorBinding, right: &HostLocomotorBinding) -> bool {
    left.movement == right.movement
        && left.visual_physics == right.visual_physics
        && left.max_speed_damaged == right.max_speed_damaged
        && left.acceleration_damaged == right.acceleration_damaged
        && left.turn_rate_damaged == right.turn_rate_damaged
        && left.braking == right.braking
        && left.min_speed == right.min_speed
        && left.min_turn_speed == right.min_turn_speed
        && left.behavior_z == right.behavior_z
        && left.appearance == right.appearance
        && left.extra_2d_friction == right.extra_2d_friction
        && left.apply_2d_friction_when_airborne == right.apply_2d_friction_when_airborne
        && left.allow_motive_force_while_airborne == right.allow_motive_force_while_airborne
        && left.locomotor_works_when_dead == right.locomotor_works_when_dead
        && left.airborne_targeting_height == right.airborne_targeting_height
        && left.can_move_backward == right.can_move_backward
        && left.downhill_only == right.downhill_only
        && left.max_lift == right.max_lift
        && left.max_lift_damaged == right.max_lift_damaged
        && left.speed_limit_z == right.speed_limit_z
        && left.preferred_height == right.preferred_height
        && left.preferred_height_damping == right.preferred_height_damping
        && left.circling_radius == right.circling_radius
        && left.max_thrust_angle == right.max_thrust_angle
        && left.turn_pivot_offset == right.turn_pivot_offset
        && left.wander_width_factor == right.wander_width_factor
        && left.wander_length_factor == right.wander_length_factor
        && left.stick_to_ground == right.stick_to_ground
        && left.close_enough_dist == right.close_enough_dist
        && left.close_enough_dist_3d == right.close_enough_dist_3d
        && left.ultra_accurate_slide_factor == right.ultra_accurate_slide_factor
}

fn store_has(name: &str) -> bool {
    get_locomotor_store().find_template(name).is_some()
}

fn try_load_locomotor_ini_from_disk() -> usize {
    let mut total = 0usize;
    for path in locomotor_ini_candidate_paths() {
        if !path.is_file() {
            continue;
        }
        match std::fs::read_to_string(&path) {
            Ok(content) => match load_locomotors_from_str(&content) {
                Ok(n) if n > 0 => {
                    log::info!(
                        "Host LocomotorStore: loaded {} locomotor templates from {}",
                        n,
                        path.display()
                    );
                    total += n;
                    // One full Locomotor.ini is enough; further copies are overrides.
                    break;
                }
                Ok(_) => {}
                Err(e) => {
                    log::debug!(
                        "Host LocomotorStore: parse failed for {}: {e}",
                        path.display()
                    );
                }
            },
            Err(e) => {
                log::debug!("Host LocomotorStore: cannot read {}: {e}", path.display());
            }
        }
    }
    total
}

fn locomotor_ini_candidate_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    let relative = [
        "windows_game/extracted_big_files/INIZH/Data/INI/Locomotor.ini",
        "windows_game/extracted_big_files_v2/INI/Locomotor.ini",
        "Data/INI/Locomotor.ini",
        "Data/INI/Default/Locomotor.ini",
        "INIZH/Data/INI/Locomotor.ini",
    ];
    for r in relative {
        paths.push(PathBuf::from(r));
    }

    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        let mut dir = PathBuf::from(manifest);
        for _ in 0..6 {
            for r in relative {
                paths.push(dir.join(r));
            }
            if !dir.pop() {
                break;
            }
        }
    }

    let mut seen = std::collections::HashSet::new();
    paths.retain(|p| seen.insert(p.clone()));
    paths
}

/// Seed minimal retail-ish stats for host golden units when INI is unavailable.
///
/// Values match retail Locomotor.ini Speed / Acceleration / TurnRate (dist/sec).
/// Wave 81: uses [`HOST_LOCOMOTOR_SEED_RESIDUAL_TABLE`] (golden + common deepen).
fn seed_known_host_locomotors() -> usize {
    let mut added = 0usize;
    for &(name, speed, accel, turn_deg) in HOST_LOCOMOTOR_SEED_RESIDUAL_TABLE {
        if store_has(name) {
            continue;
        }
        let mut props = HashMap::new();
        props.insert("Speed".to_string(), format!("{}", speed));
        props.insert("Acceleration".to_string(), format!("{}", accel));
        props.insert("TurnRate".to_string(), format!("{}", turn_deg));
        // Air residual locomotors; others GROUND (host seed surfaces residual only).
        let surfaces = if name == RAPTOR_JET_LOCOMOTOR
            || name == COMANCHE_LOCOMOTOR
            || name == MIG_LOCOMOTOR
            || name == HELIX_LOCOMOTOR
            || name == CHINOOK_LOCOMOTOR
            || name == A10_THUNDERBOLT_LOCOMOTOR
            || name == B52_LOCOMOTOR
            || name == SPY_DRONE_LOCOMOTOR
        {
            "AIR"
        } else {
            "GROUND"
        };
        props.insert("Surfaces".to_string(), surfaces.to_string());
        if matches!(
            name,
            BASIC_HUMAN_LOCOMOTOR
                | REDGUARD_LOCOMOTOR
                | COLONEL_BURTON_GROUND_LOCOMOTOR
                | JARMEN_KELL_LOCOMOTOR
                | BLACK_LOTUS_LOCOMOTOR
                | SABOTEUR_GROUND_LOCOMOTOR
                | MISSILE_DEFENDER_LOCOMOTOR
        ) {
            // Retail infantry locos: TWO_LEGS + wander weave + SpeedDamaged ~half.
            props.insert("Appearance".to_string(), "TWO_LEGS".to_string());
            props.insert("WanderWidthFactor".to_string(), "1.0".to_string());
            props.insert("WanderLengthFactor".to_string(), "1.0".to_string());
            props.insert("SpeedDamaged".to_string(), format!("{}", speed * 0.5));
            props.insert(
                "AccelerationDamaged".to_string(),
                format!("{}", accel * 0.5),
            );
        }
        if name == CHINOOK_LOCOMOTOR || name == HELIX_LOCOMOTOR {
            // Retail ChinookLocomotor / HelixLocomotor: HOVER + 100 msec slide.
            props.insert("Appearance".to_string(), "HOVER".to_string());
            props.insert("SlideIntoPlaceTime".to_string(), "100".to_string());
        }

        match parse_locomotor_template_definition(name, &props) {
            Ok(template) => match get_locomotor_store_mut().add_template(template) {
                Ok(()) => {
                    log::debug!("Host LocomotorStore: seeded locomotor {}", name);
                    added += 1;
                }
                Err(e) => {
                    log::warn!("Host LocomotorStore: failed to add {}: {e}", name);
                }
            },
            Err(e) => {
                log::warn!("Host LocomotorStore: failed to seed {}: {e}", name);
            }
        }
    }
    if added > 0 {
        log::info!(
            "Host LocomotorStore: seeded {} known golden-unit locomotors (INI data unavailable or incomplete)",
            added
        );
    }
    added
}

/// C++ ChinookLocomotor / HelixLocomotor `SlideIntoPlaceTime = 100` (msec).
/// Common stores raw msec; leftover `parse_duration_real` → 3 frames.
fn ensure_chinook_helix_slide_into_place() -> usize {
    const RETAIL_SLIDE_MSEC: f32 = 100.0;
    let mut patched = 0usize;
    for name in [CHINOOK_LOCOMOTOR, HELIX_LOCOMOTOR] {
        if !store_has(name) {
            let (speed, accel, turn) = match name {
                CHINOOK_LOCOMOTOR => (150.0, 60.0, 180.0),
                _ => (75.0, 60.0, 180.0),
            };
            let mut props = HashMap::new();
            props.insert("Speed".to_string(), format!("{}", speed));
            props.insert("Acceleration".to_string(), format!("{}", accel));
            props.insert("TurnRate".to_string(), format!("{}", turn));
            props.insert("Surfaces".to_string(), "AIR".to_string());
            props.insert("Appearance".to_string(), "HOVER".to_string());
            props.insert(
                "SlideIntoPlaceTime".to_string(),
                format!("{}", RETAIL_SLIDE_MSEC),
            );
            match parse_locomotor_template_definition(name, &props) {
                Ok(template) => match get_locomotor_store_mut().add_template(template) {
                    Ok(()) => patched += 1,
                    Err(e) => log::warn!(
                        "Host LocomotorStore: cannot seed {name} SlideIntoPlaceTime: {e}"
                    ),
                },
                Err(e) => {
                    log::warn!("Host LocomotorStore: cannot parse {name} SlideIntoPlaceTime: {e}")
                }
            }
            continue;
        }
        if let Some(template) = get_locomotor_store_mut().find_template_mut(name) {
            let mut changed = false;
            if template.ultra_accurate_slide_into_place_factor <= 0.0 {
                template.ultra_accurate_slide_into_place_factor = RETAIL_SLIDE_MSEC;
                changed = true;
            }
            if template.appearance == SourceLocomotorAppearance::Other {
                template.appearance = SourceLocomotorAppearance::Hover;
                changed = true;
            }
            if changed {
                patched += 1;
            }
        }
    }
    patched
}

fn seed_exact_combat_bike_normal_locomotors() -> usize {
    let mut added = 0usize;
    for (name, surfaces) in [
        (COMBAT_BIKE_GROUND_LOCOMOTOR, "GROUND RUBBLE"),
        (COMBAT_BIKE_CLIFF_LOCOMOTOR, "CLIFF"),
    ] {
        if store_has(name) {
            continue;
        }
        let mut props = HashMap::new();
        props.insert("Surfaces".to_string(), surfaces.to_string());
        props.insert("Speed".to_string(), "120".to_string());
        props.insert("SpeedDamaged".to_string(), "90".to_string());
        props.insert("TurnRate".to_string(), "120".to_string());
        props.insert("TurnRateDamaged".to_string(), "120".to_string());
        props.insert("Acceleration".to_string(), "90".to_string());
        props.insert("AccelerationDamaged".to_string(), "80".to_string());
        props.insert("Braking".to_string(), "60".to_string());
        props.insert("MinTurnSpeed".to_string(), "20".to_string());
        props.insert("TurnPivotOffset".to_string(), "-0.60".to_string());
        props.insert("ZAxisBehavior".to_string(), "NO_Z_MOTIVE_FORCE".to_string());
        props.insert("Appearance".to_string(), "MOTORCYCLE".to_string());
        props.insert("UniformAxialDamping".to_string(), "0.9".to_string());
        props.insert("CanMoveBackwards".to_string(), "No".to_string());
        props.insert("HasSuspension".to_string(), "Yes".to_string());
        match parse_locomotor_template_definition(name, &props) {
            Ok(template) => match get_locomotor_store_mut().add_template(template) {
                Ok(()) => added += 1,
                Err(error) => log::warn!(
                    "Host LocomotorStore: cannot seed Combat Bike locomotor {name}: {error}"
                ),
            },
            Err(error) => log::warn!(
                "Host LocomotorStore: cannot parse Combat Bike locomotor {name}: {error}"
            ),
        }
    }
    added
}

fn seed_exact_human_cliff_locomotor() -> usize {
    if store_has(HUMAN_CLIFF_LOCOMOTOR) {
        return 0;
    }
    let mut props = HashMap::new();
    props.insert("Surfaces".to_string(), "CLIFF".to_string());
    props.insert("Speed".to_string(), "30".to_string());
    props.insert("SpeedDamaged".to_string(), "20".to_string());
    props.insert("TurnRate".to_string(), "500".to_string());
    props.insert("TurnRateDamaged".to_string(), "500".to_string());
    props.insert("Acceleration".to_string(), "100".to_string());
    props.insert("AccelerationDamaged".to_string(), "80".to_string());
    props.insert("Braking".to_string(), "50".to_string());
    props.insert("ZAxisBehavior".to_string(), "NO_Z_MOTIVE_FORCE".to_string());
    props.insert("Appearance".to_string(), "CLIMBER".to_string());
    match parse_locomotor_template_definition(HUMAN_CLIFF_LOCOMOTOR, &props) {
        Ok(template) => match get_locomotor_store_mut().add_template(template) {
            Ok(()) => 1,
            Err(error) => {
                log::warn!("Host LocomotorStore: cannot seed {HUMAN_CLIFF_LOCOMOTOR}: {error}");
                0
            }
        },
        Err(error) => {
            log::warn!("Host LocomotorStore: cannot parse {HUMAN_CLIFF_LOCOMOTOR}: {error}");
            0
        }
    }
}

fn seed_exact_combat_bike_sluggish_locomotors() -> usize {
    let mut added = 0usize;
    // Retail INIZH Locomotor.ini values.  SET_SLUGGISH is heterogeneous:
    // ground ";25% slower" Speed 90 vs cliff Speed 60.
    for (name, surfaces, speed, speed_damaged, accel, accel_damaged) in [
        (
            COMBAT_BIKE_TERRORIST_GROUND_LOCOMOTOR,
            "GROUND RUBBLE",
            "90",
            "68",
            "68",
            "60",
        ),
        (
            COMBAT_BIKE_TERRORIST_CLIFF_LOCOMOTOR,
            "CLIFF",
            "60",
            "45",
            "50",
            "40",
        ),
    ] {
        if store_has(name) {
            continue;
        }
        let mut props = HashMap::new();
        props.insert("Surfaces".to_string(), surfaces.to_string());
        props.insert("Speed".to_string(), speed.to_string());
        props.insert("SpeedDamaged".to_string(), speed_damaged.to_string());
        props.insert("TurnRate".to_string(), "120".to_string());
        props.insert("TurnRateDamaged".to_string(), "120".to_string());
        props.insert("Acceleration".to_string(), accel.to_string());
        props.insert("AccelerationDamaged".to_string(), accel_damaged.to_string());
        props.insert("Braking".to_string(), "60".to_string());
        props.insert("MinTurnSpeed".to_string(), "20".to_string());
        props.insert("TurnPivotOffset".to_string(), "-0.60".to_string());
        props.insert("ZAxisBehavior".to_string(), "NO_Z_MOTIVE_FORCE".to_string());
        props.insert("Appearance".to_string(), "MOTORCYCLE".to_string());
        props.insert("UniformAxialDamping".to_string(), "0.9".to_string());
        props.insert("CanMoveBackwards".to_string(), "No".to_string());
        props.insert("HasSuspension".to_string(), "Yes".to_string());
        match parse_locomotor_template_definition(name, &props) {
            Ok(template) => match get_locomotor_store_mut().add_template(template) {
                Ok(()) => added += 1,
                Err(error) => log::warn!(
                    "Host LocomotorStore: cannot seed sluggish Combat Bike locomotor {name}: {error}"
                ),
            },
            Err(error) => log::warn!(
                "Host LocomotorStore: cannot parse sluggish Combat Bike locomotor {name}: {error}"
            ),
        }
    }
    added
}

fn seed_exact_aircraft_set_switch_locomotors() -> usize {
    let mut added = 0usize;
    for (name, surfaces, appearance, speed, accel, turn, braking) in [
        (
            AIRPLANE_TAXIING_LOCOMOTOR,
            "GROUND",
            "WHEELS",
            "25",
            "40",
            "90",
            Some("50"),
        ),
        (
            RAPTOR_TAXIING_LOCOMOTOR,
            "GROUND",
            "WHEELS",
            "25",
            "40",
            "90",
            Some("50"),
        ),
        (
            RAPTOR_SUPERSONIC_LOCOMOTOR,
            "AIR",
            "WINGS",
            "250",
            "180",
            "90",
            None,
        ),
        (
            AURORA_SUPERSONIC_LOCOMOTOR,
            "AIR",
            "WINGS",
            "320",
            "200",
            "80",
            None,
        ),
        (
            MIG_SUPERSONIC_LOCOMOTOR,
            "AIR",
            "WINGS",
            "230",
            "160",
            "90",
            None,
        ),
    ] {
        if store_has(name) {
            continue;
        }
        let mut props = HashMap::new();
        props.insert("Surfaces".to_string(), surfaces.to_string());
        props.insert("Appearance".to_string(), appearance.to_string());
        props.insert("Speed".to_string(), speed.to_string());
        props.insert("SpeedDamaged".to_string(), speed.to_string());
        props.insert("Acceleration".to_string(), accel.to_string());
        props.insert("AccelerationDamaged".to_string(), accel.to_string());
        props.insert("TurnRate".to_string(), turn.to_string());
        props.insert("TurnRateDamaged".to_string(), turn.to_string());
        if let Some(brake) = braking {
            props.insert("Braking".to_string(), brake.to_string());
        }
        if surfaces == "GROUND" {
            props.insert("MinSpeed".to_string(), "0".to_string());
            props.insert("ZAxisBehavior".to_string(), "NO_Z_MOTIVE_FORCE".to_string());
        }
        if surfaces == "AIR" {
            props.insert("AllowAirborneMotiveForce".to_string(), "Yes".to_string());
        }
        match parse_locomotor_template_definition(name, &props) {
            Ok(template) => match get_locomotor_store_mut().add_template(template) {
                Ok(()) => added += 1,
                Err(error) => log::warn!(
                    "Host LocomotorStore: cannot seed aircraft set locomotor {name}: {error}"
                ),
            },
            Err(error) => log::warn!(
                "Host LocomotorStore: cannot parse aircraft set locomotor {name}: {error}"
            ),
        }
    }
    added
}

/// Seed SpyDroneLocomotor even after the generic store bootstrap completed.
pub fn ensure_spy_drone_locomotor() -> usize {
    let _ = ensure_host_locomotor_store();
    seed_exact_spy_drone_locomotor()
}

fn seed_exact_spy_drone_locomotor() -> usize {
    if store_has(SPY_DRONE_LOCOMOTOR) {
        return 0;
    }
    let mut props = HashMap::new();
    props.insert("Surfaces".to_string(), "AIR".to_string());
    props.insert("Appearance".to_string(), "HOVER".to_string());
    props.insert("Speed".to_string(), "75".to_string());
    props.insert("SpeedDamaged".to_string(), "75".to_string());
    props.insert("Acceleration".to_string(), "100".to_string());
    props.insert("AccelerationDamaged".to_string(), "100".to_string());
    props.insert("TurnRate".to_string(), "180".to_string());
    props.insert("TurnRateDamaged".to_string(), "180".to_string());
    props.insert("MinSpeed".to_string(), "0".to_string());
    props.insert("Braking".to_string(), "50".to_string());
    props.insert("PreferredHeight".to_string(), "40".to_string());
    props.insert("PreferredHeightDamping".to_string(), "0.8".to_string());
    props.insert(
        "ZAxisBehavior".to_string(),
        "SURFACE_RELATIVE_HEIGHT".to_string(),
    );
    props.insert("AllowAirborneMotiveForce".to_string(), "Yes".to_string());
    props.insert("StickToGround".to_string(), "No".to_string());
    match parse_locomotor_template_definition(SPY_DRONE_LOCOMOTOR, &props) {
        Ok(template) => match get_locomotor_store_mut().add_template(template) {
            Ok(()) => 1,
            Err(error) => {
                log::warn!("Host LocomotorStore: cannot seed SpyDroneLocomotor: {error}");
                0
            }
        },
        Err(error) => {
            log::warn!("Host LocomotorStore: cannot parse SpyDroneLocomotor: {error}");
            0
        }
    }
}

/// Test helper: force re-bootstrap attempt (does not clear existing templates).
#[cfg(test)]
pub fn reset_bootstrap_attempt_flag_for_tests() {
    BOOTSTRAP_ATTEMPTED.store(false, Ordering::Relaxed);
    SEED_COMPLETE.store(false, Ordering::Relaxed);
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;

    #[test]
    fn bootstrap_seeds_basic_human_at_retail_speed() {
        ensure_host_locomotor_store();
        assert!(store_has(BASIC_HUMAN_LOCOMOTOR));
        let m = movement_from_store(BASIC_HUMAN_LOCOMOTOR).expect("movement");
        assert!(
            (m.max_speed - 20.0).abs() < 0.05,
            "retail BasicHumanLocomotor Speed is 20 u/s, got {}",
            m.max_speed
        );
        assert!(
            (m.acceleration - 100.0).abs() < 0.5,
            "retail Acceleration is 100, got {}",
            m.acceleration
        );
    }

    #[test]
    fn create_object_usa_ranger_binds_retail_infantry_speed() {
        ensure_host_locomotor_store();

        let mut logic = crate::game_logic::GameLogic::new();
        let mut ranger = ThingTemplate::new("USA_Ranger");
        ranger
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Attackable)
            .add_kind_of(KindOf::Selectable)
            .set_health(120.0)
            .set_locomotor_name(BASIC_HUMAN_LOCOMOTOR);
        logic.templates.insert("USA_Ranger".to_string(), ranger);

        let id = logic
            .create_object("USA_Ranger", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("create USA_Ranger");
        let obj = logic.objects.get(&id).expect("object");
        // Catalog path must replace Movement::default() (10) with retail ~20.
        assert!(
            (obj.movement.max_speed - 10.0).abs() > 0.01,
            "expected catalog speed, still host default 10: {}",
            obj.movement.max_speed
        );
        assert!(
            (obj.movement.max_speed - 20.0).abs() < 0.05,
            "expected BasicHumanLocomotor 20 u/s, got {}",
            obj.movement.max_speed
        );
        assert!(
            (obj.movement.acceleration - 100.0).abs() < 0.5,
            "expected retail accel 100, got {}",
            obj.movement.acceleration
        );
        assert!(
            (obj.movement.max_speed_damaged - 10.0).abs() < 0.05
                || (obj.movement.max_speed_damaged - 20.0).abs() < 0.05,
            "SpeedDamaged must come from INI/seed, not Movement::default 5: {}",
            obj.movement.max_speed_damaged
        );
        assert!(
            (obj.braking - 50.0).abs() > 0.5,
            "INI/omitted Braking must not stay hardcoded 50, got {}",
            obj.braking
        );
        assert!(
            obj.wander_width_factor > 0.0,
            "BasicHuman wander weave must be bound"
        );
        assert!(
            obj.wander_offset_increment > 0.0,
            "wander phase increment must be unique-seeded"
        );
        assert_eq!(
            obj.loco_appearance,
            crate::game_logic::LocomotorAppearance::LegsTwo
        );
    }

    #[test]
    fn infantry_template_resolves_movement_from_locomotor_path() {
        ensure_host_locomotor_store();
        let mut t = ThingTemplate::new("USA_Ranger");
        t.add_kind_of(KindOf::Infantry)
            .set_locomotor_name(BASIC_HUMAN_LOCOMOTOR);
        let m = t.resolve_movement().expect("locomotor path");
        assert!((m.max_speed - 20.0).abs() < 0.05);
    }

    #[test]
    fn host_binding_retains_exact_drawable_visual_locomotor_constants() {
        let mut properties = HashMap::new();
        properties.insert("Speed".to_string(), "30".to_string());
        properties.insert("Surfaces".to_string(), "GROUND".to_string());
        properties.insert("AccelerationPitchLimit".to_string(), "0.11".to_string());
        properties.insert("DecelerationPitchLimit".to_string(), "0.22".to_string());
        properties.insert("BounceAmount".to_string(), "0.33".to_string());
        properties.insert("PitchStiffness".to_string(), "0.44".to_string());
        properties.insert("RollStiffness".to_string(), "0.55".to_string());
        properties.insert("PitchDamping".to_string(), "0.66".to_string());
        properties.insert("RollDamping".to_string(), "0.77".to_string());
        properties.insert(
            "PitchInDirectionOfZVelFactor".to_string(),
            "0.88".to_string(),
        );
        properties.insert("ThrustRoll".to_string(), "0.99".to_string());
        properties.insert("ThrustWobbleRate".to_string(), "1.11".to_string());
        properties.insert("ThrustMinWobble".to_string(), "1.22".to_string());
        properties.insert("ThrustMaxWobble".to_string(), "1.33".to_string());
        properties.insert("ForwardVelocityPitchFactor".to_string(), "1.44".to_string());
        properties.insert("LateralVelocityRollFactor".to_string(), "1.55".to_string());
        properties.insert(
            "ForwardAccelerationPitchFactor".to_string(),
            "1.66".to_string(),
        );
        properties.insert(
            "LateralAccelerationRollFactor".to_string(),
            "1.77".to_string(),
        );
        properties.insert("UniformAxialDamping".to_string(), "1.88".to_string());
        properties.insert("HasSuspension".to_string(), "Yes".to_string());
        properties.insert("MaximumWheelExtension".to_string(), "-1.99".to_string());
        properties.insert("MaximumWheelCompression".to_string(), "2.11".to_string());
        properties.insert("FrontWheelTurnAngle".to_string(), "2.22".to_string());
        properties.insert("RudderCorrectionDegree".to_string(), "2.33".to_string());
        properties.insert("RudderCorrectionRate".to_string(), "2.44".to_string());
        properties.insert("ElevatorCorrectionDegree".to_string(), "2.55".to_string());
        properties.insert("ElevatorCorrectionRate".to_string(), "2.66".to_string());

        let template = parse_locomotor_template_definition("VisualProbe", &properties)
            .expect("parse visual locomotor source");
        let binding =
            host_locomotor_binding_from_template(&template).expect("visual locomotor binding");
        let visual = binding.visual_physics;

        assert_eq!(visual.accel_pitch_limit, 0.11);
        assert_eq!(visual.decel_pitch_limit, 0.22);
        assert_eq!(visual.bounce_kick, 0.33);
        assert_eq!(visual.pitch_stiffness, 0.44);
        assert_eq!(visual.roll_stiffness, 0.55);
        assert_eq!(visual.pitch_damping, 0.66);
        assert_eq!(visual.roll_damping, 0.77);
        assert_eq!(visual.pitch_by_z_vel_coef, 0.88);
        assert_eq!(visual.thrust_roll, 0.99);
        assert_eq!(visual.wobble_rate, 1.11);
        assert_eq!(visual.min_wobble, 1.22);
        assert_eq!(visual.max_wobble, 1.33);
        assert_eq!(visual.forward_vel_coef, 1.44);
        assert_eq!(visual.lateral_vel_coef, 1.55);
        assert_eq!(visual.forward_accel_coef, 1.66);
        assert_eq!(visual.lateral_accel_coef, 1.77);
        assert_eq!(visual.uniform_axial_damping, 1.88);
        assert!(visual.has_suspension);
        assert_eq!(visual.maximum_wheel_extension, -1.99);
        assert_eq!(visual.maximum_wheel_compression, 2.11);
        assert_eq!(visual.wheel_turn_angle, 2.22);
        assert_eq!(visual.rudder_correction_degree, 2.33);
        assert_eq!(visual.rudder_correction_rate, 2.44);
        assert_eq!(visual.elevator_correction_degree, 2.55);
        assert_eq!(visual.elevator_correction_rate, 2.66);

        let mut changed = binding;
        changed.visual_physics.pitch_damping += 1.0;
        assert!(!same_host_locomotor_behavior(&binding, &changed));
    }

    #[test]
    fn omitted_airborne_targeting_height_binds_int_max() {
        let mut properties = HashMap::new();
        properties.insert("Speed".to_string(), "20".to_string());
        properties.insert("Surfaces".to_string(), "GROUND".to_string());
        let omitted = parse_locomotor_template_definition("GroundNoHeight", &properties)
            .expect("parse ground locomotor");
        let omitted_binding =
            host_locomotor_binding_from_template(&omitted).expect("bind omitted height");
        assert_eq!(omitted_binding.airborne_targeting_height, i32::MAX);

        properties.insert("AirborneTargetingHeight".to_string(), "30".to_string());
        let authored = parse_locomotor_template_definition("AirHeightLoco", &properties)
            .expect("parse authored height");
        let authored_binding =
            host_locomotor_binding_from_template(&authored).expect("bind authored height");
        assert_eq!(authored_binding.airborne_targeting_height, 30);
    }

    #[test]
    fn slide_into_place_time_binds_leftover_duration_real() {
        let mut properties = HashMap::new();
        properties.insert("Speed".to_string(), "150".to_string());
        properties.insert("Surfaces".to_string(), "AIR".to_string());
        properties.insert("Appearance".to_string(), "HOVER".to_string());
        properties.insert("SlideIntoPlaceTime".to_string(), "100".to_string());
        let authored = parse_locomotor_template_definition("ChinookSlide", &properties)
            .expect("parse SlideIntoPlaceTime");
        assert!(
            (authored.ultra_accurate_slide_into_place_factor - 100.0).abs() < 1e-6,
            "Common stores raw msec, got {}",
            authored.ultra_accurate_slide_into_place_factor
        );
        let leftover = gamelogic::locomotor::ini_bridge::from_common_ini_template(&authored);
        let binding =
            host_locomotor_binding_from_template(&authored).expect("bind SlideIntoPlaceTime");
        assert!(
            (binding.ultra_accurate_slide_factor - leftover.ultra_accurate_slide_factor).abs()
                < 1e-6
        );
        assert!(
            (binding.ultra_accurate_slide_factor - 3.0).abs() < 1e-6,
            "leftover parse_duration_real(100 msec) is 3 frames, got {}",
            binding.ultra_accurate_slide_factor
        );

        let mut tmpl = ThingTemplate::new("SlideProbe");
        tmpl.add_kind_of(KindOf::Aircraft);
        let mut obj = Object::new(tmpl, ObjectId(1), Team::USA);
        apply_host_locomotor_binding(&mut obj, &binding);
        assert!(
            (obj.ultra_accurate_slide_factor - 3.0).abs() < 1e-6,
            "apply must stamp leftover slide factor, got {}",
            obj.ultra_accurate_slide_factor
        );

        let omitted = parse_locomotor_template_definition("NoSlide", &{
            let mut p = HashMap::new();
            p.insert("Speed".to_string(), "20".to_string());
            p.insert("Surfaces".to_string(), "GROUND".to_string());
            p
        })
        .expect("parse omitted SlideIntoPlaceTime");
        let omitted_binding =
            host_locomotor_binding_from_template(&omitted).expect("bind omitted slide");
        assert_eq!(omitted_binding.ultra_accurate_slide_factor, 0.0);

        let mut changed = binding;
        changed.ultra_accurate_slide_factor += 1.0;
        assert!(!same_host_locomotor_behavior(&binding, &changed));
    }

    #[test]
    fn locomotor_works_when_dead_binds_and_applies() {
        let mut properties = HashMap::new();
        properties.insert("Speed".to_string(), "80".to_string());
        properties.insert("Surfaces".to_string(), "AIR".to_string());
        properties.insert("LocomotorWorksWhenDead".to_string(), "Yes".to_string());
        let authored = parse_locomotor_template_definition("DeadJetLoco", &properties)
            .expect("parse LocomotorWorksWhenDead");
        assert!(authored.locomotor_works_when_dead);
        let binding =
            host_locomotor_binding_from_template(&authored).expect("bind works-when-dead");
        assert!(binding.locomotor_works_when_dead);

        let mut tmpl = ThingTemplate::new("DeadJetProbe");
        tmpl.add_kind_of(KindOf::Aircraft);
        let mut obj = Object::new(tmpl, ObjectId(2), Team::USA);
        assert!(!obj.locomotor_works_when_dead);
        apply_host_locomotor_binding(&mut obj, &binding);
        assert!(obj.locomotor_works_when_dead);
        obj.health.current = 0.0;
        assert!(!obj.is_alive());
        assert!(
            !obj.host_skip_dead_locomotor(),
            "authored dead locos keep motive"
        );

        let omitted = parse_locomotor_template_definition("GroundDeadNo", &{
            let mut p = HashMap::new();
            p.insert("Speed".to_string(), "20".to_string());
            p.insert("Surfaces".to_string(), "GROUND".to_string());
            p
        })
        .expect("parse omitted works-when-dead");
        let omitted_binding = host_locomotor_binding_from_template(&omitted).expect("bind omitted");
        assert!(!omitted_binding.locomotor_works_when_dead);
        let mut changed = binding;
        changed.locomotor_works_when_dead = false;
        assert!(!same_host_locomotor_behavior(&binding, &changed));
    }

    #[test]
    fn close_enough_dist_3d_binds_and_arrival_is_leftover_2d_unless_flag() {
        let mut properties = HashMap::new();
        properties.insert("Speed".to_string(), "20".to_string());
        properties.insert("Surfaces".to_string(), "GROUND".to_string());
        properties.insert("CloseEnoughDist3D".to_string(), "Yes".to_string());
        let authored = parse_locomotor_template_definition("ScudDive", &properties)
            .expect("parse CloseEnoughDist3D");
        let leftover = gamelogic::locomotor::ini_bridge::from_common_ini_template(&authored);
        assert!(leftover.is_close_enough_dist_3d);
        let binding = host_locomotor_binding_from_template(&authored).expect("bind 3d");
        assert!(binding.close_enough_dist_3d);

        let mut tmpl = ThingTemplate::new("GroundSlope");
        tmpl.add_kind_of(KindOf::Infantry);
        let mut stamped = Object::new(tmpl, ObjectId(3), Team::USA);
        apply_host_locomotor_binding(&mut stamped, &binding);
        assert!(stamped.close_enough_dist_3d);

        let omitted = parse_locomotor_template_definition("Ground2d", &{
            let mut p = HashMap::new();
            p.insert("Speed".to_string(), "20".to_string());
            p.insert("Surfaces".to_string(), "GROUND".to_string());
            p
        })
        .expect("parse omitted CloseEnoughDist3D");
        let omitted_binding =
            host_locomotor_binding_from_template(&omitted).expect("bind omitted 3d");
        assert!(!omitted_binding.close_enough_dist_3d);
        let mut changed = binding;
        changed.close_enough_dist_3d = false;
        assert!(!same_host_locomotor_behavior(&binding, &changed));

        // Slope: 2D=1, 3D~10. Leftover ground uses 2D so plants; flag uses 3D.
        let mut ground = Object::new(ThingTemplate::new("SlopeScout"), ObjectId(4), Team::USA);
        apply_host_locomotor_binding(&mut ground, &omitted_binding);
        ground.set_position(Vec3::new(0.0, 10.0, 0.0));
        ground.close_enough_dist = Some(2.0);
        ground.movement.target_position = Some(Vec3::new(1.0, 0.0, 0.0));
        ground.movement.max_speed = 0.0;
        ground.movement.velocity = Vec3::ZERO;
        assert!(
            (ground
                .host_locomotor_distance_to_goal(ground.get_position(), Vec3::new(1.0, 0.0, 0.0))
                - 1.0)
                .abs()
                < 1e-4
        );
        ground.update_movement(1.0 / 30.0);
        assert!(
            ground.movement.target_position.is_none(),
            "ground leftover arrival is 2D, 1wu < 2"
        );

        let mut dive = Object::new(ThingTemplate::new("ScudDiveObj"), ObjectId(5), Team::USA);
        apply_host_locomotor_binding(&mut dive, &binding);
        dive.set_position(Vec3::new(0.0, 10.0, 0.0));
        dive.close_enough_dist = Some(2.0);
        dive.movement.target_position = Some(Vec3::new(1.0, 0.0, 0.0));
        dive.movement.max_speed = 0.0;
        dive.movement.velocity = Vec3::ZERO;
        let d3 =
            dive.host_locomotor_distance_to_goal(dive.get_position(), Vec3::new(1.0, 0.0, 0.0));
        assert!(d3 > 9.0, "flag uses 3D hypotenuse, got {d3}");
        dive.update_movement(1.0 / 30.0);
        assert!(
            dive.movement.target_position.is_some(),
            "CloseEnoughDist3D must not plant on 2D when 3D > arrive"
        );

        let mut proj_tmpl = ThingTemplate::new("Missile");
        proj_tmpl.add_kind_of(KindOf::Projectile);
        let mut proj = Object::new(proj_tmpl, ObjectId(6), Team::USA);
        apply_host_locomotor_binding(&mut proj, &omitted_binding);
        assert!(!proj.close_enough_dist_3d);
        proj.set_position(Vec3::new(0.0, 10.0, 0.0));
        let pd =
            proj.host_locomotor_distance_to_goal(proj.get_position(), Vec3::new(1.0, 0.0, 0.0));
        assert!(pd > 9.0, "KINDOF_PROJECTILE leftover-uses 3D, got {pd}");
    }

    #[test]
    fn chinook_helix_seed_binds_slide_into_place_time() {
        let _ = ensure_chinook_helix_slide_into_place();
        for name in [CHINOOK_LOCOMOTOR, HELIX_LOCOMOTOR] {
            let binding = resolve_host_locomotor_binding(name)
                .unwrap_or_else(|| panic!("{name} must resolve"));
            assert!(
                (binding.ultra_accurate_slide_factor - 3.0).abs() < 1e-6,
                "{name} SlideIntoPlaceTime 100 msec → 3 frames, got {}",
                binding.ultra_accurate_slide_factor
            );
            assert_eq!(
                binding.appearance,
                crate::game_logic::LocomotorAppearance::Hover,
                "{name} retail appearance is HOVER"
            );
        }

        let mut logic = crate::game_logic::GameLogic::new();
        let mut chinook = ThingTemplate::new("AmericaVehicleChinook");
        chinook
            .add_kind_of(KindOf::Aircraft)
            .set_locomotor_name(CHINOOK_LOCOMOTOR);
        logic
            .templates
            .insert("AmericaVehicleChinook".to_string(), chinook);
        let id = logic
            .create_object("AmericaVehicleChinook", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("create Chinook");
        let obj = logic.objects.get(&id).expect("chinook");
        assert!(
            (obj.ultra_accurate_slide_factor - 3.0).abs() < 1e-6,
            "create_object must stamp Chinook slide factor, got {}",
            obj.ultra_accurate_slide_factor
        );
    }

    #[test]
    fn lift_and_speed_limit_z_bind_per_frame_units() {
        let mut properties = HashMap::new();
        properties.insert("Speed".to_string(), "20".to_string());
        properties.insert("Surfaces".to_string(), "AIR".to_string());
        properties.insert("Lift".to_string(), "90".to_string());
        properties.insert("LiftDamaged".to_string(), "45".to_string());
        properties.insert("SpeedLimitZ".to_string(), "30".to_string());
        let authored = parse_locomotor_template_definition("HoverLift", &properties)
            .expect("parse hover lift locomotor");
        assert!(
            (authored.lift - 0.1).abs() < 1e-6,
            "Common stores Lift/900, got {}",
            authored.lift
        );
        let binding = host_locomotor_binding_from_template(&authored).expect("bind hover lift");
        assert!(
            (binding.max_lift - 0.1).abs() < 1e-6,
            "live lift must stay per-frame², not *900, got {}",
            binding.max_lift
        );
        assert!(
            (binding.max_lift_damaged - 0.05).abs() < 1e-6,
            "live liftDamaged must stay per-frame², got {}",
            binding.max_lift_damaged
        );
        assert!(
            (binding.speed_limit_z - 1.0).abs() < 1e-6,
            "SpeedLimitZ must be parseVelocityReal (÷30), got {}",
            binding.speed_limit_z
        );

        let omitted = parse_locomotor_template_definition("NoZLimit", &{
            let mut p = HashMap::new();
            p.insert("Speed".to_string(), "20".to_string());
            p.insert("Surfaces".to_string(), "GROUND".to_string());
            p
        })
        .expect("parse omitted SpeedLimitZ");
        let omitted_binding =
            host_locomotor_binding_from_template(&omitted).expect("bind omitted z limit");
        assert!(
            (omitted_binding.speed_limit_z - 999_999.0).abs() < 1e-3,
            "omitted SpeedLimitZ stays BIGNUM, got {}",
            omitted_binding.speed_limit_z
        );
    }

    #[test]
    fn locomotor_name_for_known_units() {
        assert_eq!(
            locomotor_name_for_unit("USA_Ranger"),
            Some(BASIC_HUMAN_LOCOMOTOR)
        );
        assert_eq!(
            locomotor_name_for_unit("USA_Humvee"),
            Some(HUMVEE_LOCOMOTOR)
        );
        assert_eq!(
            locomotor_name_for_unit("USA_Dozer"),
            Some(AMERICA_DOZER_LOCOMOTOR)
        );
        assert_eq!(
            locomotor_name_for_unit("China_Soldier"),
            Some(REDGUARD_LOCOMOTOR)
        );
        assert_eq!(
            locomotor_name_for_unit("AmericaInfantryPathfinder"),
            Some(COLONEL_BURTON_GROUND_LOCOMOTOR)
        );
        assert_eq!(
            locomotor_name_for_unit("AmericaVehicleTomahawk"),
            Some(TOMAHAWK_LOCOMOTOR)
        );
    }

    #[test]
    fn locomotor_residual_table_wave81_honesty() {
        assert!(honesty_locomotor_residual_table_wave81());
        assert!(HOST_LOCOMOTOR_SEED_RESIDUAL_TABLE.len() >= 12);
        assert!(store_has(COLONEL_BURTON_GROUND_LOCOMOTOR));
        assert!(store_has(RAPTOR_JET_LOCOMOTOR));
    }

    #[test]
    fn locomotor_residual_expand_wave92_honesty() {
        assert!(honesty_locomotor_residual_expand_wave92());
        assert!(HOST_LOCOMOTOR_SEED_RESIDUAL_TABLE.len() >= 25);
        assert!(store_has(OVERLORD_LOCOMOTOR));
        assert!(store_has(COMANCHE_LOCOMOTOR));
        assert!(store_has(MIG_LOCOMOTOR));
        assert!(store_has(HELIX_LOCOMOTOR));
        assert_eq!(
            locomotor_name_for_unit("ChinaTankOverlord"),
            Some(OVERLORD_LOCOMOTOR)
        );
        assert_eq!(
            locomotor_name_for_unit("AmericaVehicleComanche"),
            Some(COMANCHE_LOCOMOTOR)
        );
    }

    #[test]
    fn locomotor_residual_expand_pack_honesty_wave103() {
        assert!(honesty_locomotor_residual_expand_wave103());
        assert!(HOST_LOCOMOTOR_SEED_RESIDUAL_TABLE.len() >= 39);
        assert!(store_has(BOMB_TRUCK_LOCOMOTOR));
        assert!(store_has(CHINOOK_LOCOMOTOR));
        assert!(store_has(COMBAT_BIKE_GROUND_LOCOMOTOR));
        assert!(store_has(JARMEN_KELL_LOCOMOTOR));
        assert_eq!(
            locomotor_name_for_unit("GLAVehicleBombTruck"),
            Some(BOMB_TRUCK_LOCOMOTOR)
        );
        assert_eq!(
            locomotor_name_for_unit("AmericaVehicleChinook"),
            Some(CHINOOK_LOCOMOTOR)
        );
    }

    #[test]
    fn create_object_humvee_binds_vehicle_speed_when_catalog_present() {
        ensure_host_locomotor_store();
        let mut logic = crate::game_logic::GameLogic::new();
        let mut humvee = ThingTemplate::new("USA_Humvee");
        humvee
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Attackable)
            .set_locomotor_name(HUMVEE_LOCOMOTOR);
        logic.templates.insert("USA_Humvee".to_string(), humvee);

        let id = logic
            .create_object("USA_Humvee", Team::USA, Vec3::ZERO)
            .expect("create");
        let obj = logic.objects.get(&id).expect("object");
        assert!(
            (obj.movement.max_speed - 60.0).abs() < 0.1,
            "HumveeLocomotor Speed is 60, got {}",
            obj.movement.max_speed
        );
    }

    #[test]
    fn locomotor_set_names_for_unit_lists_cliff_members() {
        ensure_host_locomotor_store();
        let bike = locomotor_set_names_for_unit("GLAVehicleCombatBike");
        assert_eq!(
            bike,
            vec![
                COMBAT_BIKE_GROUND_LOCOMOTOR.to_string(),
                COMBAT_BIKE_CLIFF_LOCOMOTOR.to_string()
            ]
        );
        let burton = locomotor_set_names_for_unit("AmericaInfantryColonelBurton");
        assert_eq!(
            burton,
            vec![
                COLONEL_BURTON_GROUND_LOCOMOTOR.to_string(),
                HUMAN_CLIFF_LOCOMOTOR.to_string()
            ]
        );
        let cliff = resolve_host_locomotor_binding(HUMAN_CLIFF_LOCOMOTOR)
            .expect("HumanCliffLocomotor seed");
        assert_eq!(
            cliff.locomotor_surfaces & crate::game_logic::object::LOCO_SURFACE_CLIFF,
            crate::game_logic::object::LOCO_SURFACE_CLIFF
        );
        assert_eq!(
            cliff.appearance,
            crate::game_logic::LocomotorAppearance::Climber
        );
    }
}
