use crate::game_logic::{AIState, BuildingType, GameLogic, KindOf, Object, ObjectId, Team};
use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::f32::consts::TAU;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

const DOUBLE_CLICK_THRESHOLD: Duration = Duration::from_millis(250);

fn screen_to_world(screen: Vec2, viewport_size: Vec2, world_min: Vec3, world_max: Vec3) -> Vec3 {
    let viewport_width = viewport_size.x.max(1.0);
    let viewport_height = viewport_size.y.max(1.0);
    let normalized_x = (screen.x / viewport_width).clamp(0.0, 1.0);
    let normalized_y = (screen.y / viewport_height).clamp(0.0, 1.0);
    let world_width = (world_max.x - world_min.x).max(1.0);
    let world_height = (world_max.z - world_min.z).max(1.0);

    Vec3::new(
        world_min.x + normalized_x * world_width,
        0.0,
        world_min.z + normalized_y * world_height,
    )
}

fn default_max_shots_cmd() -> i32 {
    -1
}

/// All possible command types that can be issued in the game
/// Based on MSG_* types from MessageStream.h starting at MSG_BEGIN_NETWORK_MESSAGES = 1000
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CommandType {
    // Selection commands
    CreateSelectedGroup {
        create_new: bool,
        units: Vec<ObjectId>,
    },
    DestroySelectedGroup {
        team_id: u32,
    },
    RemoveFromSelectedGroup {
        units: Vec<ObjectId>,
    },

    // Movement commands
    Move {
        destination: Vec3,
    }, // Basic move command
    MoveTo {
        destination: Vec3,
        waypoints: Vec<Vec3>,
    },
    AttackMoveTo {
        destination: Vec3,
        /// C++ maxShotsToFire residual (-1 / NO_MAX = unlimited).
        #[serde(default = "default_max_shots_cmd")]
        max_shots: i32,
    },
    ForceMoveTo {
        destination: Vec3,
    },
    AddWaypoint {
        destination: Vec3,
    },

    // Combat commands
    Attack {
        target_id: ObjectId,
    }, // Basic attack command
    AttackObject {
        target_id: ObjectId,
    },
    ForceAttackObject {
        target_id: ObjectId,
    },
    ForceAttackGround {
        location: Vec3,
    },
    /// C++ AIGroup::groupAttackPosition residual (None loc = attack own position).
    AttackPosition {
        location: Option<Vec3>,
        max_shots: i32,
    },
    Stop,
    Guard {
        target: GuardTarget,
        /// C++ GuardMode residual (default Normal).
        #[serde(default)]
        mode: crate::game_logic::GuardMode,
    },
    /// Patrol residual: units wander and auto-engage nearby enemies.
    Patrol,
    /// C++ AttitudeType residual hotkeys / strip.
    AttitudeSleep,
    AttitudePassive,
    AttitudeNormal,
    AttitudeAggressive,
    Scatter,
    /// C++ AIGroup::groupTightenToPosition residual — all units path to same point.
    TightenToPosition {
        destination: glam::Vec3,
    },
    /// C++ AIGroup::groupAttackTeam residual.
    AttackTeam {
        /// Team discriminant: 0=GLA, 1=USA, 2=China (Neutral ignored).
        team: u8,
        max_shots: i32,
    },
    /// C++ AIGroup::groupOverrideSpecialPowerDestination residual.
    OverrideSpecialPowerDestination {
        location: glam::Vec3,
    },
    /// C++ AIGroup::setWeaponSetFlag residual.
    /// flag: 0=PLAYER_UPGRADE, 1=MINE_CLEARING, 2=CARBOMB, 3=VEHICLE_HIJACK.
    SetWeaponSetFlag {
        flag: u8,
        enabled: bool,
    },
    /// C++ AIGroup::groupFollowWaypointPath residual (explicit path points).
    FollowWaypointPath {
        waypoints: Vec<glam::Vec3>,
        exact: bool,
        /// C++ groupFollowWaypointPathAsTeam residual.
        #[serde(default)]
        as_team: bool,
    },
    /// C++ AIAttackFollowWaypointPathState residual.
    AttackFollowWaypointPath {
        waypoints: Vec<glam::Vec3>,
        exact: bool,
        #[serde(default)]
        as_team: bool,
    },
    /// C++ AIGroup::groupDoCommandButtonUsingWaypoints residual.
    DoCommandButtonUsingWaypoints {
        button: String,
        waypoints: Vec<glam::Vec3>,
    },
    /// C++ AIGroup::groupSurrender residual (test-key path).
    Surrender {
        surrendered: bool,
    },
    /// C++ AIGroup::groupDoCommandButton residual — dispatch by button name.
    DoCommandButton {
        button: String,
    },
    /// C++ AIGroup::groupDoCommandButtonAtPosition residual.
    DoCommandButtonAtPosition {
        button: String,
        location: glam::Vec3,
    },
    /// C++ AIGroup::groupDoCommandButtonAtObject residual.
    DoCommandButtonAtObject {
        button: String,
        target: crate::game_logic::ObjectId,
    },
    /// C++ AIGroup::groupExecuteRailedTransport residual.
    ExecuteRailedTransport,

    Deploy,
    Gather {
        target_id: ObjectId,
    },

    // Building and construction
    Build {
        template_name: String,
        location: Vec3,
    }, // Basic build command
    DozerConstruct {
        template_name: String,
        location: Vec3,
        /// Build facing residual (radians about Y).
        orientation: f32,
    },
    DozerConstructLine {
        template_name: String,
        start: Vec3,
        end: Vec3,
    },
    DozerCancelConstruct {
        object_id: ObjectId,
    },
    ResumeConstruction {
        target_id: ObjectId,
    },
    Sell {
        object_id: ObjectId,
    },

    // Unit production
    QueueUnitCreate {
        template_name: String,
        quantity: u32,
    },
    CancelUnitCreate {
        template_name: String,
    },

    // Special abilities
    DoSpecialPower {
        power_type: SpecialPowerType,
        target: PowerTarget,
    },
    DoWeapon {
        weapon_slot: WeaponSlot,
        target: WeaponTarget,
    },

    // Transport and container
    Enter {
        target_id: ObjectId,
    },
    Exit,
    Evacuate,
    /// C++ AIGroup::groupMoveToAndEvacuate — path container then unload.
    MoveToAndEvacuate {
        destination: glam::Vec3,
        /// When true: AICMD_MOVE_TO_POSITION_AND_EVACUATE_AND_EXIT (transport self-destruct residual).
        and_exit: bool,
    },
    /// China Hacker field HackInternet residual (start cash interval).
    HackInternet,
    /// Aircraft return-to-base / dock nearest airfield residual.
    ReturnToBase,
    /// Harvester return cargo to nearest SupplyCenter residual.
    ReturnSupplies,
    /// Dozer/Worker path to nearest enemy mine and clear residual.
    ClearMines,
    /// C++ AIGroup::setMineClearingDetail residual.
    SetMineClearingDetail {
        enabled: bool,
    },
    /// C++ AIGroup::groupGoProne residual.
    GoProne,
    /// C++ AIGroup::setWeaponLockForGroup residual.
    SetWeaponLock {
        slot: u8,
        /// 0=NotLocked, 1=Temporary, 2=Permanent (WeaponLockType).
        lock_type: u8,
    },
    /// C++ AIGroup::releaseWeaponLockForGroup residual.
    ReleaseWeaponLock {
        /// 1=Temporary, 2=Permanent.
        lock_type: u8,
    },
    /// C++ AIGroup::groupSetEmoticon residual.
    SetEmoticon {
        name: String,
        /// Duration in logic frames.
        duration_frames: i32,
    },
    /// C++ AIGroup::groupAttackArea residual — attack enemies inside radius around point.
    AttackArea {
        center: glam::Vec3,
        radius: f32,
    },
    Dock {
        target_id: ObjectId,
    },
    CombatDrop {
        target: DropTarget,
    },

    // Utility commands
    Repair {
        target_id: ObjectId,
    },
    GetRepaired {
        target_id: ObjectId,
    },
    GetHealed {
        target_id: ObjectId,
    },
    SetRallyPoint {
        location: Vec3,
    },

    // Economy and resources
    PurchaseScience {
        science_name: String,
    },
    QueueUpgrade {
        upgrade_name: String,
    },
    CancelUpgrade {
        upgrade_name: String,
    },

    // Special unit abilities
    Hijack {
        target_id: ObjectId,
    },
    Sabotage {
        target_id: ObjectId,
    },
    ConvertToCarbomb {
        target_id: ObjectId,
    },
    CaptureBuilding {
        target_id: ObjectId,
    },
    SnipeVehicle {
        target_id: ObjectId,
    },
    /// Colonel Burton residual: plant timed demo charge on structure/vehicle.
    PlantTimedDemoCharge {
        target_id: ObjectId,
    },
    /// Colonel Burton residual: plant remote demo charge on structure/vehicle
    /// (SPECIAL_REMOTE_CHARGES — no auto-timer until DetonateRemoteDemoCharges).
    PlantRemoteDemoCharge {
        target_id: ObjectId,
    },
    /// Colonel Burton residual: detonate all remote demo charges planted by
    /// the selected unit(s) (SPECIAL_REMOTE_CHARGES no-target path).
    DetonateRemoteDemoCharges,
    /// Demo General residual: intentional SUICIDED detonation
    /// (`Demo_Command_TertiarySuicide` FIRE_WEAPON tertiary after
    /// `Demo_Upgrade_SuicideBomb` CommandSetUpgrade residual).
    DemoTertiarySuicide,
    /// Black Lotus residual: steal cash from enemy supply/cash building.
    StealCashHack {
        target_id: ObjectId,
    },
    /// Black Lotus residual: disable enemy ground vehicle (DISABLED_HACKED).
    DisableVehicleHack {
        target_id: ObjectId,
    },
    /// China Hacker residual: disable enemy structure (DISABLED_HACKED).
    /// SpecialAbilityHackerDisableBuilding.
    HackerDisableBuilding {
        target_id: ObjectId,
    },
    /// GLA Bomb Truck residual: disguise as target vehicle
    /// (SpecialAbilityDisguiseAsVehicle / StealthUpdate::disguiseAsTemplate).
    DisguiseAsVehicle {
        target_id: ObjectId,
    },
    /// GLA Rebel residual: plant BoobyTrap on structure (SpecialAbilityBoobyTrap).
    PlantBoobyTrap {
        target_id: ObjectId,
    },
    SwitchWeapons,
    ToggleOvercharge,

    // Formation and group commands
    CreateFormation,
    Cheer,

    // Network/multiplayer commands
    PlaceBeacon {
        location: Vec3,
        text: String,
    },
    RemoveBeacon,
    ViewLastRadarEvent,
    ViewRadarAt {
        position: Vec3,
    },
    ViewCommandCenter,

    // Invalid command placeholder
    Invalid,
}

/// Target types for guard command
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GuardTarget {
    Position(Vec3),
    Object(ObjectId),
}

/// Target types for special powers
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PowerTarget {
    Location(Vec3),
    Object(ObjectId),
    None,
}

/// Target types for weapon commands
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WeaponTarget {
    Location(Vec3),
    Object(ObjectId),
}

/// Target types for combat drops
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DropTarget {
    Location(Vec3),
    Object(ObjectId),
}

/// Special power types
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SpecialPowerType {
    Airstrike,
    /// China Artillery Barrage residual (SPECIAL_ARTILLERY_BARRAGE /
    /// SuperweaponArtilleryBarrage family). Delayed multi-shell scatter area
    /// damage (`ArtilleryBarrageDamageWeapon` residual within WeaponErrorRadius).
    /// Fail-closed: not full ChinaArtilleryCannon OCL DeliverPayload /
    /// random scatter draw / science-tier FormationSize 12/24/36 path.
    Artillery,
    /// Carpet Bomb residual (SPECIAL_CARPET_BOMB / SuperweaponCarpetBomb family).
    /// Delayed bomber approach then multi-point line/area explosive damage
    /// (`CarpetBombWeapon` residual along drop line).
    /// Fail-closed: not full B52/ChinaJet OCL DeliverPayload / DropVariance /
    /// staggered DropDelay / science-tier upgrade path.
    CarpetBomb,
    /// China Airforce Early Carpet Bomb residual (EARLY_SPECIAL_CHINA_CARPET_BOMB /
    /// Early_SuperweaponChinaCarpetBomb). China payload matrix + Early science gate.
    EarlyChinaCarpetBomb,
    /// USA Airforce Carpet Bomb residual (AIRF_SPECIAL_CARPET_BOMB /
    /// AirF_SuperweaponCarpetBomb). AirForce payload matrix (12 bombs / 130ms delay).
    AirForceCarpetBomb,
    ClusterMines,
    /// USA Superweapon Crate Drop residual (SPECIAL_CRATE_DROP / SuperweaponCrateDrop).
    /// Spawns residual 200DollarCrate × 10 near target.
    /// Fail-closed vs full AmericaJetCargoPlane OCL DeliverPayload Object.
    CrateDrop,
    DaisyCutter,
    EmergencyRepair,
    /// Early Emergency Repair residual (EARLY_SPECIAL_REPAIR_VEHICLES).
    EarlyEmergencyRepair,
    FuelAirBomb,
    Healing,
    IonCannon,
    NapalmStrike,
    NuclearMissile,
    /// GLA Black Market Nuke residual (SPECIAL_BLACK_MARKET_NUKE /
    /// SuperweaponBlackMarketNuke). Host maps to NuclearMissile radiation residual.
    /// Fail-closed vs full BlackMarketNuke OCL / smaller dirty yield matrix.
    BlackMarketNuke,
    /// GLA Detonate Dirty Nuke residual (SPECIAL_DETONATE_DIRTY_NUKE /
    /// SuperweaponDetonateDirtyNuke). Short-reload host NuclearMissile residual.
    DetonateDirtyNuke,
    /// USA America Airborne / SuperweaponParadropAmerica residual.
    Paradrop,
    /// Infantry General Paradrop residual (Infa_SuperweaponInfantryParadrop).
    /// Host maps to Paradrop residual path with same ranger payload fail-closed.
    InfantryParadrop,
    /// Tank General Tank Paradrop residual (Tank_SuperweaponTankParadrop).
    /// Host maps to Paradrop residual path (fail-closed infantry payload).
    TankParadrop,
    /// GLA Rebel Ambush / SuperweaponRebelAmbush residual (SPECIAL_AMBUSH).
    /// Spawns infantry near target after fade delay — fail-closed vs full OCL.
    Ambush,
    /// GLA Terror Cell residual (SPECIAL_TERROR_CELL / SuperweaponTerrorCell).
    /// Host maps to Ambush infantry spawn residual (fail-closed vs full OCL cell).
    TerrorCell,
    ParticleCannon,
    RadarScan,
    ScudStorm,
    /// USA Spy Satellite Scan residual — temporary FOW reveal at location.
    SpySatellite,
    /// USA CIA Intelligence residual (SpyVision / setUnitsVisionSpied).
    /// Temporarily reveals enemy units (vision-spied + FOW + DETECTED residual).
    /// Fail-closed: not full SpyVisionUpdate module / kindof filter / sabotage path.
    CiaIntelligence,
    /// USA Communications Download residual (SPECIAL_COMMUNICATIONS_DOWNLOAD).
    /// Host maps to CIA Intelligence SpyVision residual (shared activate audio).
    /// Fail-closed vs full Pathfinder module duration matrix.
    CommunicationsDownload,
    SpyDrone,
    SuperweaponCountermeasures,
    /// China Dragon Tank FireWall / Firestorm residual (FIRE_WEAPON secondary path).
    /// Creates a line of fire damage zones toward the target location.
    FireWall,
    /// GLA Anthrax Bomb residual (SPECIAL_ANTHRAX_BOMB / SuperweaponAnthraxBomb).
    /// Delayed plane-drop blast + residual toxin field ticks.
    /// Fail-closed: not full OCL jet cargo / PoisonField object / gamma upgrade.
    AnthraxBomb,
    /// USA Spectre Gunship residual (SPECIAL_SPECTRE_GUNSHIP / SuperweaponSpectreGunship).
    /// Delayed orbit insertion at target + periodic howitzer residual damage ticks
    /// in AttackAreaRadius for OrbitTime.
    /// Fail-closed: not full SpectreGunshipUpdate OCL aircraft / gattling strafe /
    /// howitzer projectile / decal / contain gunner path.
    SpectreGunship,
    /// China EMP Pulse residual (SPECIAL_EMP_PULSE / SuperweaponEMPPulse).
    /// Temporarily disables vehicles/structures in radius (DISABLED_EMP).
    /// Fail-closed: not full OCL EMPPulseBomb / EMPPulseEffectSpheroid drawable path.
    EmpPulse,
    /// China Frenzy ("Rage") residual (SPECIAL_FRENZY / SuperweaponFrenzy).
    /// Temporary ally attack buff in radius (WEAPONBONUSCONDITION_FRENZY_*).
    /// Fail-closed: not full OCL Frenzy_InvisibleMarker / FrenzyCloud particle path.
    Frenzy,
    /// China Airforce Early Frenzy residual (EARLY_SPECIAL_FRENZY /
    /// Early_SuperweaponFrenzy). Same residual levels; Early science gate.
    EarlyFrenzy,
    /// USA Strategy Center Bombardment battle plan residual
    /// (`SpecialAbilityChangeBattlePlans` OPTION_ONE / PLANSTATUS_BOMBARDMENT).
    /// Army-wide DAMAGE 120% residual for legal members.
    /// Fail-closed: not full BattlePlanUpdate pack/unpack / turret enable matrix.
    BattlePlanBombardment,
    /// USA Strategy Center HoldTheLine battle plan residual
    /// (`SpecialAbilityChangeBattlePlans` OPTION_TWO / PLANSTATUS_HOLDTHELINE).
    /// Army armor damage scalar 0.9 + Strategy Center max-health 2.0 residual.
    /// Fail-closed: not full paralyze / door animation matrix.
    BattlePlanHoldTheLine,
    /// USA Strategy Center SearchAndDestroy battle plan residual
    /// (`SpecialAbilityChangeBattlePlans` OPTION_THREE / PLANSTATUS_SEARCHANDDESTROY).
    /// Army RANGE 120% + sight 1.2 residual; center detect residual.
    /// Fail-closed: not full StealthDetectorUpdate module / vision object path.
    BattlePlanSearchAndDestroy,
    /// GLA GPS Scrambler residual (SPECIAL_GPS_SCRAMBLER / SuperweaponGPSScrambler).
    /// Grants temporary STEALTHED to ally vehicles/infantry in radius (GrantStealth).
    /// Fail-closed: not full OCL GPSScrambler_InvisibleMarker grow-radius pulse path.
    GpsScrambler,
    /// USA Leaflet Drop residual (SPECIAL_LEAFLET_DROP / SuperweaponLeafletDrop).
    /// Delayed disable of enemy infantry/vehicles in radius (DISABLED_EMP residual).
    /// Fail-closed: not full OCL B52 / LeafletContainer / LeafletFX particle path.
    LeafletDrop,
    /// USA Airforce Early Leaflet Drop residual (EARLY_SPECIAL_LEAFLET_DROP /
    /// Early_SuperweaponLeafletDrop). Same delay/radius/reload; Early science gate.
    EarlyLeafletDrop,
    /// GLA Sneak Attack residual (SPECIAL_SNEAK_ATTACK / SuperweaponSneakAttack).
    /// Delayed tunnel structure spawn at target + residual shockwave damage.
    /// Fail-closed: not full OCL Start animation / multi-shockwave / TunnelContain path.
    SneakAttack,
    /// USA Superweapon General Cruise Missile residual
    /// (SUPR_SPECIAL_CRUISE_MISSILE / SupW_CruiseMissile / SUPERWEAPON_CruiseMissile).
    /// Delayed loft-to-target strike with `MOABDetonationWeapon` area damage.
    /// Fail-closed: not full NeutronMissileUpdate loft / door animation /
    /// OCL FireWeapon projectile path / MOABFlameWeapon secondary.
    CruiseMissile,
    /// USA Ambulance Cleanup Area residual
    /// (`SpecialAbilityAmbulanceCleanupArea` / CleanupAreaPower).
    /// Clears residual toxin/radiation fields and mines at a world location.
    /// Fail-closed: not full CleanupHazardUpdate projectile stream / scan loop /
    /// HazardousMaterialArmor object stack / rubble pathfind clear.
    CleanupArea,
    /// China Helix NapalmBomb residual (`SpecialAbilityHelixNapalmBomb` /
    /// SPECIAL_HELIX_NAPALM_BOMB). Instant NapalmBomb blast + FirestormSmall DoT
    /// at target location (requires Upgrade_HelixNapalmBomb residual unlock).
    /// Fail-closed: not full SpecialObject NapalmBomb fall / HeightDieUpdate /
    /// FirestormDynamicGeometryInfoUpdate expand animation.
    HelixNapalmBomb,
    /// Nuke General Helix NukeBomb residual (`Nuke_SpecialAbilityHelixNukeBomb`).
    /// Same SPECIAL_HELIX_NAPALM_BOMB enum; host maps to Helix napalm residual path
    /// with Nuke_Upgrade_HelixNukeBomb unlock residual.
    HelixNukeBomb,
    /// China SuperweaponCashHack residual (SPECIAL_CASH_HACK / SuperweaponCashHack).
    /// Instant steal from richest enemy player by SCIENCE_CashHack1/2/3 amount
    /// (1000/2000/4000). Fail-closed vs full CashHackSpecialPower victim clamp /
    /// floating text / multiplayer academy path.
    CashHack,
    /// USA Airforce DaisyCutter residual (AIRF_SPECIAL_DAISY_CUTTER).
    AirForceDaisyCutter,
    /// USA Airforce A10 residual (AIRF_SPECIAL_A10_THUNDERBOLT_STRIKE).
    AirForceAirstrike,
    /// USA Airforce Spectre residual (AIRF_SPECIAL_SPECTRE_GUNSHIP).
    AirForceSpectreGunship,
    /// Superweapon General PUC residual (SUPW_SPECIAL_PARTICLE_UPLINK_CANNON).
    SuperweaponParticleCannon,
    /// Nuke General NeutronMissile residual (NUKE_SPECIAL_NEUTRON_MISSILE).
    NukeNeutronMissile,
    /// Superweapon General NeutronMissile residual (SUPW path alias).
    SuperweaponNeutronMissile,
    /// Nuke General China CarpetBomb residual.
    NukeChinaCarpetBomb,
    /// Stealth General GPS Scrambler residual (Slth_SuperweaponGPSScrambler).
    StealthGpsScrambler,
    /// Launch Baikonur Rocket residual (SPECIAL_LAUNCH_BAIKONUR_ROCKET).
    BaikonurRocket,
    /// Nuke General NukeDrop residual (NUKE_SPECIAL_CLUSTER_MINES).
    NukeDrop,
    /// Battleship Bombardment residual (SPECIAL_BATTLESHIP_BOMBARDMENT).
    BattleshipBombardment,
    /// Laser General Particle Uplink residual (LAZR_SPECIAL_PARTICLE_UPLINK_CANNON).
    /// Host maps to ParticleCannon residual path.
    LaserCannon,
    /// USA Missile Defender laser guided special residual
    /// (SPECIAL_MISSILE_DEFENDER_LASER_GUIDED_MISSILES).
    /// Locks secondary laser weapon + attack target (StartAbilityRange 200).
    MissileDefenderLaserGuided,
    /// China Tank Hunter TNT special residual (SPECIAL_TANKHUNTER_TNT_ATTACK).
    /// Plants sticky timed charge (StartAbilityRange 5, Reload 7500ms).
    TankHunterTnt,
    /// Laser General Howitzer laser guided residual
    /// (shares SPECIAL_MISSILE_DEFENDER_LASER_GUIDED_MISSILES enum).
    /// Host maps to secondary laser-lock attack residual.
    LaserGuidedHowitzer,
    /// Demo Rebel timed charges residual (SPECIAL_TIMED_CHARGES / Reload 30000ms).
    DemoRebelTimedCharges,
    /// Demo Kell timed charges residual (SPECIAL_TIMED_CHARGES).
    DemoKellTimedCharges,
    /// Demo Kell sticky charges residual (SPECIAL_TIMED_CHARGES).
    DemoKellStickyCharges,
    /// Demo Kell remote charges residual (SPECIAL_REMOTE_CHARGES).
    DemoKellRemoteCharges,
    /// Battle Bus demo trap rollout residual (SPECIAL_TIMED_CHARGES / Reload 7500ms).
    BattleBusDemoTrapRollout,
    /// Colonel Burton timed charges residual (SPECIAL_TIMED_CHARGES).
    BurtonTimedCharges,
    /// Colonel Burton remote charges residual (SPECIAL_REMOTE_CHARGES).
    BurtonRemoteCharges,
    /// China Hacker disable building residual (SPECIAL_HACKER_DISABLE_BUILDING).
    HackerDisableBuilding,
    /// Black Lotus disable vehicle residual (SPECIAL_BLACKLOTUS_DISABLE_VEHICLE_HACK).
    BlackLotusDisableVehicle,
    /// Black Lotus steal cash residual (SPECIAL_BLACKLOTUS_STEAL_CASH_HACK).
    BlackLotusStealCash,
    /// Black Lotus capture building residual (SPECIAL_BLACKLOTUS_CAPTURE_BUILDING).
    BlackLotusCaptureBuilding,
    /// Microwave Tank disable building residual (SPECIAL_MICROWAVE_DISABLE_BUILDING).
    MicrowaveDisableBuilding,
    /// Ranger capture building residual (SPECIAL_INFANTRY_CAPTURE_BUILDING).
    RangerCaptureBuilding,
    /// Red Guard capture building residual (SPECIAL_INFANTRY_CAPTURE_BUILDING).
    RedGuardCaptureBuilding,
    /// Rebel capture building residual (SPECIAL_INFANTRY_CAPTURE_BUILDING).
    RebelCaptureBuilding,
    /// Bomb Truck disguise residual (SPECIAL_DISGUISE_AS_VEHICLE).
    DisguiseAsVehiclePower,
    Invalid,
}

/// Weapon slot identifiers
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WeaponSlot {
    Primary,
    Secondary,
    Tertiary,
    AntiAir,
    Slot(u32),
}

/// Command evaluation results
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CommandResult {
    Success,
    InvalidTarget,
    OutOfRange,
    InsufficientResources,
    InvalidCommand,
    UnitBusy,
    TargetDestroyed,
    RequiresLineOfSight,
    InvalidLocation,
    CannotAttackTarget,
    CannotMoveToLocation,
    BuildingBlocked,
}

/// Command evaluation mode
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CommandEvaluateType {
    DoCommand,
    DoHint,
    EvaluateOnly,
}

/// A complete game command with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameCommand {
    pub command_type: CommandType,
    pub player_id: u32,
    pub command_id: u32,
    pub timestamp: SystemTime,
    pub selected_units: Vec<ObjectId>,
    pub modifier_keys: ModifierKeys,
}

/// Mouse/keyboard modifier keys
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct ModifierKeys {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

/// Information needed for command creation from mouse input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MouseCommandContext {
    pub world_position: Vec3,
    pub target_object: Option<ObjectId>,
    pub screen_position: Vec2,
    pub viewport_size: Option<Vec2>,
    pub world_min: Option<Vec3>,
    pub world_max: Option<Vec3>,
    pub mouse_button: MouseButton,
    pub modifier_keys: ModifierKeys,
    pub is_drag: bool,
    pub drag_start: Option<Vec2>,
    pub drag_end: Option<Vec2>,
    pub drag_start_world: Option<Vec3>,
    pub drag_end_world: Option<Vec3>,
}

/// Mouse button types
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Command system state for tracking mode
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CommandMode {
    Normal,
    ForceAttack,
    ForceMove,
    Waypoint,
    BuildMode { template_name: String },
    SpecialPower { power_type: SpecialPowerType },
}

/// Main command system that handles all RTS commands
pub struct CommandSystem {
    /// Current command mode (force attack, build mode, etc.)
    pub current_mode: CommandMode,

    /// Commands waiting to be processed
    command_queue: VecDeque<GameCommand>,

    /// Current command ID counter
    next_command_id: u32,

    /// Mouse drag tracking
    mouse_drag_start: Option<Vec2>,
    mouse_down_time: Option<Instant>,

    /// Command history for undo/replay
    command_history: Vec<GameCommand>,

    /// Player-specific command settings
    player_settings: HashMap<u32, PlayerCommandSettings>,
}

/// Per-player command settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerCommandSettings {
    pub auto_attack: bool,
    pub smart_select: bool,
    pub formation_move: bool,
    pub waypoint_mode: bool,
}

impl Default for PlayerCommandSettings {
    fn default() -> Self {
        Self {
            auto_attack: false,
            smart_select: true,
            formation_move: true,
            waypoint_mode: false,
        }
    }
}

impl Default for CommandSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandSystem {
    /// Create a new command system
    pub fn new() -> Self {
        Self {
            current_mode: CommandMode::Normal,
            command_queue: VecDeque::new(),
            next_command_id: 1,
            mouse_drag_start: None,
            mouse_down_time: None,
            command_history: Vec::new(),
            player_settings: HashMap::new(),
        }
    }

    /// Get (or lazily create) mutable command settings for a player
    fn player_settings_mut(&mut self, player_id: u32) -> &mut PlayerCommandSettings {
        self.player_settings.entry(player_id).or_default()
    }

    /// Read-only view of a player's settings (creating default when missing).
    pub fn player_settings(&mut self, player_id: u32) -> PlayerCommandSettings {
        self.player_settings_mut(player_id).clone()
    }

    /// Enable or disable waypoint mode for a player.
    pub fn set_waypoint_mode_for_player(&mut self, player_id: u32, enabled: bool) {
        self.player_settings_mut(player_id).waypoint_mode = enabled;
    }

    /// Toggle auto-attack preference and return the new value.
    pub fn toggle_auto_attack(&mut self, player_id: u32) -> bool {
        let settings = self.player_settings_mut(player_id);
        settings.auto_attack = !settings.auto_attack;
        settings.auto_attack
    }

    /// Toggle whether moves should preserve formation and return the new value.
    pub fn toggle_formation_move(&mut self, player_id: u32) -> bool {
        let settings = self.player_settings_mut(player_id);
        settings.formation_move = !settings.formation_move;
        settings.formation_move
    }

    /// Toggle whether selection should attempt smart grouping and return the new value.
    pub fn toggle_smart_select(&mut self, player_id: u32) -> bool {
        let settings = self.player_settings_mut(player_id);
        settings.smart_select = !settings.smart_select;
        settings.smart_select
    }

    /// Select units matching predicate for the player and queue the selection command.
    pub fn select_units_by_predicate<F>(
        &mut self,
        player_id: u32,
        modifier_keys: ModifierKeys,
        game_logic: &GameLogic,
        mut predicate: F,
    ) -> bool
    where
        F: FnMut(&Object) -> bool,
    {
        let player = match game_logic.get_player(player_id) {
            Some(player) => player,
            None => return false,
        };

        let mut units = Vec::new();
        for (&id, obj) in game_logic.get_objects().iter() {
            if obj.team == player.team && obj.is_selectable() && predicate(obj) {
                units.push(id);
            }
        }

        if units.is_empty() {
            return false;
        }

        let command = self.create_command(
            CommandType::CreateSelectedGroup {
                create_new: !modifier_keys.shift,
                units: units.clone(),
            },
            &units,
            player_id,
            modifier_keys,
        );
        self.queue_command(command);
        true
    }

    /// Build a command that selects all objects matching the double-clicked target
    fn create_select_similar_command(
        &mut self,
        target_id: ObjectId,
        player_id: u32,
        modifier_keys: ModifierKeys,
        game_logic: &GameLogic,
    ) -> Option<GameCommand> {
        let target = game_logic.get_object(target_id)?;
        let player = game_logic.get_player(player_id)?;

        // Only allow selecting similar units that belong to the same team
        if target.team != player.team {
            return None;
        }

        let template_name = target.template_name.clone();
        let object_type = target.object_type;
        let mut units: Vec<ObjectId> = game_logic
            .get_objects()
            .iter()
            .filter_map(|(&id, obj)| {
                if obj.team == target.team
                    && obj.is_selectable()
                    && (obj.template_name == template_name
                        || (modifier_keys.alt && obj.object_type == object_type))
                {
                    Some(id)
                } else {
                    None
                }
            })
            .collect();

        if units.is_empty() {
            return None;
        }

        if !units.contains(&target_id) {
            units.push(target_id);
        }

        let command_units = units.clone();
        Some(self.create_command(
            CommandType::CreateSelectedGroup {
                create_new: true,
                units: command_units,
            },
            units.as_slice(),
            player_id,
            modifier_keys,
        ))
    }

    /// Set the current command mode
    pub fn set_mode(&mut self, mode: CommandMode) {
        self.current_mode = mode.clone();
        log::debug!("Command mode changed to: {:?}", mode);
    }

    /// Process mouse input and create appropriate commands
    pub fn process_mouse_input(
        &mut self,
        context: &MouseCommandContext,
        selected_units: &[ObjectId],
        player_id: u32,
        game_logic: &GameLogic,
    ) -> Option<GameCommand> {
        if context.is_drag {
            self.mouse_drag_start = context.drag_start.or(Some(context.screen_position));
        } else {
            self.mouse_drag_start = None;
        }

        match context.mouse_button {
            MouseButton::Left => {
                self.process_left_click(context, selected_units, player_id, game_logic)
            }
            MouseButton::Right => {
                self.process_right_click(context, selected_units, player_id, game_logic)
            }
            MouseButton::Middle => {
                self.process_middle_click(context, selected_units, player_id, game_logic)
            }
        }
    }

    /// Process left mouse click
    fn process_left_click(
        &mut self,
        context: &MouseCommandContext,
        selected_units: &[ObjectId],
        player_id: u32,
        game_logic: &GameLogic,
    ) -> Option<GameCommand> {
        match &self.current_mode {
            CommandMode::Normal => {
                let now = Instant::now();
                let is_double_click = self
                    .mouse_down_time
                    .map(|last| now.duration_since(last) <= DOUBLE_CLICK_THRESHOLD)
                    .unwrap_or(false)
                    && self.player_settings_mut(player_id).smart_select
                    && !context.is_drag;
                self.mouse_down_time = Some(now);

                if is_double_click {
                    if let Some(target_id) = context.target_object {
                        if let Some(command) = self.create_select_similar_command(
                            target_id,
                            player_id,
                            context.modifier_keys,
                            game_logic,
                        ) {
                            return Some(command);
                        }
                    }
                }

                if context.is_drag {
                    // Area selection
                    Some(self.create_selection_command(context, player_id, game_logic))
                } else if let Some(target_id) = context.target_object {
                    // Select single unit
                    let create_new = !context.modifier_keys.shift;
                    Some(self.create_command(
                        CommandType::CreateSelectedGroup {
                            create_new,
                            units: vec![target_id],
                        },
                        selected_units,
                        player_id,
                        context.modifier_keys,
                    ))
                } else {
                    None
                }
            }
            CommandMode::BuildMode { template_name } => {
                // Place building
                Some(self.create_command(
                    CommandType::DozerConstruct {
                        template_name: template_name.clone(),
                        location: context.world_position,
                        orientation: 0.0,
                    },
                    selected_units,
                    player_id,
                    context.modifier_keys,
                ))
            }
            CommandMode::SpecialPower { power_type } => {
                // Use special power
                let target = if let Some(target_id) = context.target_object {
                    PowerTarget::Object(target_id)
                } else {
                    PowerTarget::Location(context.world_position)
                };

                Some(self.create_command(
                    CommandType::DoSpecialPower {
                        power_type: power_type.clone(),
                        target,
                    },
                    selected_units,
                    player_id,
                    context.modifier_keys,
                ))
            }
            _ => None,
        }
    }

    /// Process right mouse click - creates movement and attack commands
    fn process_right_click(
        &mut self,
        context: &MouseCommandContext,
        selected_units: &[ObjectId],
        player_id: u32,
        game_logic: &GameLogic,
    ) -> Option<GameCommand> {
        if selected_units.is_empty() {
            return None;
        }

        let (mut waypoint_mode, auto_attack) = {
            let settings = self.player_settings_mut(player_id);
            (settings.waypoint_mode, settings.auto_attack)
        };

        if context.modifier_keys.alt {
            waypoint_mode = true;
        }

        // C++ TheInGameUI force modes residual:
        // Ctrl = ForceAttack, Alt = Waypoints (prefer over sticky current_mode).
        let mode = if context.modifier_keys.ctrl {
            CommandMode::ForceAttack
        } else if waypoint_mode {
            CommandMode::Waypoint
        } else {
            self.current_mode.clone()
        };

        let mut command_type = match &mode {
            CommandMode::ForceAttack => {
                if let Some(target_id) = context.target_object {
                    CommandType::ForceAttackObject { target_id }
                } else {
                    CommandType::ForceAttackGround {
                        location: context.world_position,
                    }
                }
            }
            CommandMode::ForceMove => CommandType::ForceMoveTo {
                destination: context.world_position,
            },
            CommandMode::Waypoint => CommandType::AddWaypoint {
                destination: context.world_position,
            },
            _ => {
                // Context-sensitive command
                self.determine_context_command(context, selected_units, game_logic)
            }
        };

        if auto_attack {
            if let CommandType::MoveTo { destination, .. } = command_type {
                command_type = CommandType::AttackMoveTo {
                    destination,
                    max_shots: -1,
                };
            }
        }

        Some(self.create_command(
            command_type,
            selected_units,
            player_id,
            context.modifier_keys,
        ))
    }

    /// Process middle mouse click
    fn process_middle_click(
        &mut self,
        _context: &MouseCommandContext,
        _selected_units: &[ObjectId],
        _player_id: u32,
        _game_logic: &GameLogic,
    ) -> Option<GameCommand> {
        // Middle click typically used for camera controls
        None
    }

    /// Determine context-sensitive command based on target and selected units
    fn determine_context_command(
        &self,
        context: &MouseCommandContext,
        selected_units: &[ObjectId],
        game_logic: &GameLogic,
    ) -> CommandType {
        if let Some(target_id) = context.target_object {
            if let Some(target_obj) = game_logic.get_object(target_id) {
                // Check if target is a resource/harvestable and selected units can gather
                if self.can_gather_from_target(selected_units, target_obj, game_logic) {
                    return CommandType::Gather { target_id };
                }

                // C++ capture residual: capture-capable infantry/heroes → neutral
                // (or unowned tech) structure preferred over attack when applicable.
                if self.can_capture_building(selected_units, target_obj, game_logic) {
                    // Prefer capture on Neutral structures; enemy still defaults to attack
                    // unless no attacker can fire (unarmed infantry with capture upgrade).
                    let prefer_capture = target_obj.team == Team::Neutral
                        || !self.can_attack_target(selected_units, target_obj, game_logic);
                    if prefer_capture {
                        return CommandType::CaptureBuilding { target_id };
                    }
                }

                // Check if target is enemy - attack
                if self.can_attack_target(selected_units, target_obj, game_logic) {
                    return CommandType::AttackObject { target_id };
                }

                // C++ MSG_RESUME_CONSTRUCTION residual: dozer → unfinished ally structure.
                if self.can_resume_construction(selected_units, target_obj, game_logic) {
                    return CommandType::ResumeConstruction { target_id };
                }

                // Check if target is repairable
                if self.can_repair_target(selected_units, target_obj, game_logic) {
                    return CommandType::Repair { target_id };
                }

                // Check if target is enterable
                if self.can_enter_target(selected_units, target_obj, game_logic) {
                    return CommandType::Enter { target_id };
                }

                // Check if target provides healing/repair services
                if self.can_get_serviced_at_target(selected_units, target_obj, game_logic) {
                    let target_building_type = target_obj
                        .building_data
                        .as_ref()
                        .map(|b| b.building_type)
                        .unwrap_or(BuildingType::CommandCenter);
                    if target_building_type == BuildingType::HealPad
                        || target_obj.is_medical_facility()
                    {
                        return CommandType::GetHealed { target_id };
                    } else {
                        return CommandType::GetRepaired { target_id };
                    }
                }
            }
        }

        // Default to move command (Ctrl ForceAttack handled before context path).
        CommandType::MoveTo {
            destination: context.world_position,
            waypoints: Vec::new(),
        }
    }

    /// Create area selection command from drag
    fn create_selection_command(
        &mut self,
        context: &MouseCommandContext,
        player_id: u32,
        game_logic: &GameLogic,
    ) -> GameCommand {
        let player = match game_logic.get_player(player_id) {
            Some(player) => player,
            None => {
                return self.create_command(
                    CommandType::CreateSelectedGroup {
                        create_new: !context.modifier_keys.shift,
                        units: Vec::new(),
                    },
                    &[],
                    player_id,
                    context.modifier_keys,
                );
            }
        };

        let drag_start = context.drag_start.unwrap_or(context.screen_position);
        let drag_end = context.drag_end.unwrap_or(context.screen_position);
        let viewport_size = context.viewport_size.unwrap_or(Vec2::new(800.0, 600.0));
        let world_min = context.world_min.unwrap_or(Vec3::new(-400.0, 0.0, -300.0));
        let world_max = context.world_max.unwrap_or(Vec3::new(400.0, 0.0, 300.0));
        let drag_start_world = context
            .drag_start_world
            .unwrap_or_else(|| screen_to_world(drag_start, viewport_size, world_min, world_max));
        let drag_end_world = context
            .drag_end_world
            .unwrap_or_else(|| screen_to_world(drag_end, viewport_size, world_min, world_max));

        let min_x = drag_start_world.x.min(drag_end_world.x);
        let max_x = drag_start_world.x.max(drag_end_world.x);
        let min_z = drag_start_world.z.min(drag_end_world.z);
        let max_z = drag_start_world.z.max(drag_end_world.z);

        let mut units = Vec::new();
        for (&id, obj) in game_logic.get_objects().iter() {
            if obj.team != player.team || !obj.is_selectable() {
                continue;
            }

            let obj_pos = obj.get_position();
            if obj_pos.x >= min_x && obj_pos.x <= max_x && obj_pos.z >= min_z && obj_pos.z <= max_z
            {
                units.push(id);
            }
        }

        self.create_command(
            CommandType::CreateSelectedGroup {
                create_new: !context.modifier_keys.shift,
                units,
            },
            &[],
            player_id,
            context.modifier_keys,
        )
    }

    /// Create a game command with metadata
    fn create_command(
        &mut self,
        command_type: CommandType,
        selected_units: &[ObjectId],
        player_id: u32,
        modifier_keys: ModifierKeys,
    ) -> GameCommand {
        let command = GameCommand {
            command_type,
            player_id,
            command_id: self.next_command_id,
            timestamp: SystemTime::now(),
            selected_units: selected_units.to_vec(),
            modifier_keys,
        };

        self.next_command_id += 1;
        command
    }

    /// Queue command for execution
    pub fn queue_command(&mut self, command: GameCommand) {
        log::debug!("Queuing command: {:?}", command.command_type);
        self.command_queue.push_back(command);
    }

    /// Create and queue a command immediately (used by keyboard shortcuts).
    pub fn queue_immediate_command(
        &mut self,
        command_type: CommandType,
        selected_units: &[ObjectId],
        player_id: u32,
        modifier_keys: ModifierKeys,
    ) {
        let command = self.create_command(command_type, selected_units, player_id, modifier_keys);
        self.queue_command(command);
    }

    /// Process all queued commands
    pub fn process_commands(&mut self, game_logic: &mut GameLogic) -> Vec<CommandResult> {
        let mut results = Vec::new();

        while let Some(command) = self.command_queue.pop_front() {
            let result = self.execute_command(&command, game_logic);
            results.push(result);

            // Add to history for replay/undo
            self.command_history.push(command);
        }

        results
    }

    /// Execute a single command
    pub fn execute_command(
        &self,
        command: &GameCommand,
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        let mut executor =
            crate::command_executor::CommandExecutor::new(game_logic, command.player_id);
        match executor.execute_command(command.clone()) {
            Ok(result) => result,
            Err(err) => {
                log::warn!(
                    "Failed to execute command {:?} for player {}: {}",
                    command.command_type,
                    command.player_id,
                    err
                );
                CommandResult::InvalidCommand
            }
        }
    }

    /// Execute move command - core RTS functionality
    fn execute_move_command(
        &self,
        units: &[ObjectId],
        destination: Vec3,
        _waypoints: &[Vec3],
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        // Residual pathfind parity with CommandExecutor::execute_move.
        let mut all_success = true;
        for &unit_id in units {
            let can = game_logic
                .get_object(unit_id)
                .map(|u| u.can_move())
                .unwrap_or(false);
            if !can {
                all_success = false;
                continue;
            }
            if let Some(unit) = game_logic.get_object_mut(unit_id) {
                unit.stop_attack();
            }
            if !game_logic.assign_unit_path(unit_id, destination, &[]) {
                if let Some(unit) = game_logic.get_object_mut(unit_id) {
                    unit.set_destination(destination);
                    unit.set_ai_state(AIState::Moving);
                }
                all_success = false;
                continue;
            }
            if let Some(unit) = game_logic.get_object_mut(unit_id) {
                unit.set_ai_state(AIState::Moving);
            }
            log::debug!("Unit {} moving to {:?}", unit_id.0, destination);
        }
        if all_success {
            CommandResult::Success
        } else {
            CommandResult::InvalidTarget
        }
    }

    /// Execute attack command - core RTS functionality
    fn execute_attack_command(
        &self,
        units: &[ObjectId],
        target_id: ObjectId,
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        // Check if target exists and is attackable
        if let Some(target) = game_logic.get_object(target_id) {
            if target.is_dead() {
                return CommandResult::TargetDestroyed;
            }
        } else {
            return CommandResult::InvalidTarget;
        }

        let mut all_success = true;

        for &unit_id in units {
            if let Some(unit) = game_logic.get_object_mut(unit_id) {
                if unit.can_attack() {
                    unit.set_target(Some(target_id));
                    unit.set_ai_state(AIState::Attacking);
                    log::debug!("Unit {} attacking target {}", unit_id.0, target_id.0);
                } else {
                    all_success = false;
                }
            } else {
                all_success = false;
            }
        }

        if all_success {
            CommandResult::Success
        } else {
            CommandResult::InvalidTarget
        }
    }

    /// Execute attack-move command
    fn execute_attack_move_command(
        &self,
        units: &[ObjectId],
        destination: Vec3,
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        // Residual pathfind parity with CommandExecutor::execute_attack_move.
        let mut all_success = true;
        for &unit_id in units {
            let ok = game_logic
                .get_object(unit_id)
                .map(|u| u.can_move() && u.can_attack())
                .unwrap_or(false);
            if !ok {
                all_success = false;
                continue;
            }
            if let Some(unit) = game_logic.get_object_mut(unit_id) {
                unit.stop_attack();
            }
            if !game_logic.assign_unit_path(unit_id, destination, &[]) {
                if let Some(unit) = game_logic.get_object_mut(unit_id) {
                    unit.set_destination(destination);
                    unit.set_ai_state(AIState::AttackMoving);
                }
                all_success = false;
                continue;
            }
            if let Some(unit) = game_logic.get_object_mut(unit_id) {
                unit.set_ai_state(AIState::AttackMoving);
            }
            log::debug!("Unit {} attack-moving to {:?}", unit_id.0, destination);
        }
        if all_success {
            CommandResult::Success
        } else {
            CommandResult::InvalidTarget
        }
    }

    /// Execute force attack command
    fn execute_force_attack_command(
        &self,
        units: &[ObjectId],
        target_id: ObjectId,
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        // Force attack doesn't check relationships - attack anything
        let mut all_success = true;

        for &unit_id in units {
            if let Some(unit) = game_logic.get_object_mut(unit_id) {
                if unit.can_attack() {
                    unit.set_target(Some(target_id));
                    unit.set_ai_state(AIState::Attacking);
                    unit.set_force_attack(true);
                    log::debug!("Unit {} force-attacking target {}", unit_id.0, target_id.0);
                } else {
                    all_success = false;
                }
            } else {
                all_success = false;
            }
        }

        if all_success {
            CommandResult::Success
        } else {
            CommandResult::InvalidTarget
        }
    }

    /// Execute force attack ground command
    fn execute_force_attack_ground_command(
        &self,
        units: &[ObjectId],
        location: Vec3,
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        let mut all_success = true;

        for &unit_id in units {
            if let Some(unit) = game_logic.get_object_mut(unit_id) {
                if unit.can_attack() {
                    unit.set_target_location(Some(location));
                    unit.set_ai_state(AIState::AttackingGround);
                    log::debug!(
                        "Unit {} force-attacking ground at {:?}",
                        unit_id.0,
                        location
                    );
                } else {
                    all_success = false;
                }
            } else {
                all_success = false;
            }
        }

        if all_success {
            CommandResult::Success
        } else {
            CommandResult::InvalidTarget
        }
    }

    /// Execute stop command
    fn execute_stop_command(
        &self,
        units: &[ObjectId],
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        for &unit_id in units {
            if let Some(unit) = game_logic.get_object_mut(unit_id) {
                unit.stop();
                unit.set_ai_state(AIState::Idle);
                log::debug!("Unit {} stopped", unit_id.0);
            }
        }
        CommandResult::Success
    }

    /// Execute scatter command by pushing units away from their current positions.
    fn execute_scatter_command(
        &self,
        units: &[ObjectId],
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        if units.is_empty() {
            return CommandResult::InvalidCommand;
        }

        const BASE_DISTANCE: f32 = 25.0;
        const DISTANCE_VARIATION: f32 = 10.0;

        // Compute scatter goals first (no mut borrow during path assign).
        let mut goals: Vec<(ObjectId, Vec3)> = Vec::new();
        for (index, &unit_id) in units.iter().enumerate() {
            let Some(unit) = game_logic.get_object(unit_id) else {
                continue;
            };
            if !unit.can_move() {
                continue;
            }
            let origin = unit.get_position();
            let angle = ((unit_id.0 as usize + index) as f32 * 0.318_309_87) % TAU;
            let distance = BASE_DISTANCE + (index as f32 % DISTANCE_VARIATION).abs();
            let offset = Vec3::new(angle.cos(), 0.0, angle.sin()) * distance;
            goals.push((unit_id, origin + offset));
        }
        for (unit_id, destination) in goals {
            if !game_logic.assign_unit_path(unit_id, destination, &[]) {
                if let Some(unit) = game_logic.get_object_mut(unit_id) {
                    unit.set_destination(destination);
                }
            }
            if let Some(unit) = game_logic.get_object_mut(unit_id) {
                unit.set_ai_state(AIState::Moving);
                log::debug!("Unit {} scattering toward {:?}", unit_id.0, destination);
            }
        }

        CommandResult::Success
    }

    /// Arrange selected units into a grid formation centered around their centroid.
    fn execute_create_formation_command(
        &self,
        units: &[ObjectId],
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        if units.is_empty() {
            return CommandResult::InvalidCommand;
        }

        let mut movable_units = Vec::new();
        for &unit_id in units {
            if let Some(unit) = game_logic.get_object(unit_id) {
                if unit.can_move() {
                    movable_units.push((unit_id, unit.get_position()));
                }
            }
        }

        if movable_units.is_empty() {
            return CommandResult::InvalidCommand;
        }

        let mut centroid = Vec3::ZERO;
        for (_, position) in &movable_units {
            centroid += *position;
        }
        centroid /= movable_units.len() as f32;

        let columns = (movable_units.len() as f32).sqrt().ceil() as usize;
        let rows = movable_units.len().div_ceil(columns);
        let spacing = 20.0;

        for (index, (unit_id, _)) in movable_units.iter().enumerate() {
            let row = (index / columns) as f32;
            let column = (index % columns) as f32;
            let offset_x = (column - (columns as f32 - 1.0) * 0.5) * spacing;
            let offset_z = (row - (rows as f32 - 1.0) * 0.5) * spacing;
            let destination = centroid + Vec3::new(offset_x, 0.0, offset_z);

            if !game_logic.assign_unit_path(*unit_id, destination, &[]) {
                if let Some(unit) = game_logic.get_object_mut(*unit_id) {
                    unit.set_destination(destination);
                }
            }
            if let Some(unit) = game_logic.get_object_mut(*unit_id) {
                unit.set_ai_state(AIState::Moving);
                log::debug!("Unit {} forming up at {:?}", unit_id.0, destination);
            }
        }

        CommandResult::Success
    }

    fn execute_view_command_center(
        &self,
        player_id: u32,
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        let player = match game_logic.get_player(player_id) {
            Some(player) => player,
            None => return CommandResult::InvalidCommand,
        };

        if let Some(position) = game_logic.command_center_position(player.team) {
            game_logic.request_camera_focus(position);
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// Execute guard command
    fn execute_guard_command(
        &self,
        units: &[ObjectId],
        target: &GuardTarget,
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        for &unit_id in units {
            if let Some(unit) = game_logic.get_object_mut(unit_id) {
                match target {
                    GuardTarget::Position(pos) => {
                        unit.set_guard_position(Some(*pos));
                        unit.set_ai_state(AIState::GuardingArea);
                        log::debug!("Unit {} guarding position {:?}", unit_id.0, pos);
                    }
                    GuardTarget::Object(target_id) => {
                        unit.set_guard_target(Some(*target_id));
                        unit.set_ai_state(AIState::GuardingObject);
                        log::debug!("Unit {} guarding object {}", unit_id.0, target_id.0);
                    }
                }
            }
        }
        CommandResult::Success
    }

    /// Execute construction command
    fn execute_construct_command(
        &self,
        units: &[ObjectId],
        template_name: &str,
        location: Vec3,
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        let (build_cost, is_structure) = match game_logic.get_templates().get(template_name) {
            Some(t) => (
                t.build_cost,
                t.is_kind_of(crate::game_logic::KindOf::Structure),
            ),
            None => return CommandResult::InvalidCommand,
        };

        if !is_structure {
            return CommandResult::InvalidCommand;
        }

        // Find a constructor unit
        for &unit_id in units {
            let team = match game_logic.get_object(unit_id) {
                Some(unit) if unit.can_construct() => unit.team,
                Some(_) => continue,
                None => continue,
            };

            {
                let Some(player) = game_logic.get_player_mut_by_team(team) else {
                    continue;
                };

                if !player.spend_resources(&build_cost) {
                    return CommandResult::InvalidCommand;
                }
            }

            let created =
                game_logic.create_object_under_construction(template_name, team, location);
            if created.is_none() {
                if let Some(player) = game_logic.get_player_mut_by_team(team) {
                    player.resources.supplies = player
                        .resources
                        .supplies
                        .saturating_add(build_cost.supplies);
                }
                return CommandResult::InvalidCommand;
            }

            if !game_logic.assign_unit_path(unit_id, location, &[]) {
                if let Some(unit) = game_logic.get_object_mut(unit_id) {
                    unit.set_destination(location);
                }
            }
            if let Some(unit) = game_logic.get_object_mut(unit_id) {
                unit.set_ai_state(AIState::Constructing);
            }

            log::debug!(
                "Unit {} constructing {} at {:?}",
                unit_id.0,
                template_name,
                location
            );
            return CommandResult::Success;
        }
        CommandResult::InvalidCommand
    }

    /// Execute selection command
    fn execute_selection_command(
        &self,
        player_id: u32,
        create_new: bool,
        units: &[ObjectId],
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        if let Some(player) = game_logic.get_player_mut(player_id) {
            if create_new {
                player.selected_objects.clear();
            }

            for &unit_id in units {
                if !player.selected_objects.contains(&unit_id) {
                    player.selected_objects.push(unit_id);
                }
            }

            log::debug!(
                "Player {} selected {} units",
                player_id,
                player.selected_objects.len()
            );
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// Validate if selected units can capture target structure residual.
    fn can_capture_building(
        &self,
        units: &[ObjectId],
        target: &Object,
        game_logic: &GameLogic,
    ) -> bool {
        use crate::game_logic::host_hero_abilities::{
            can_capture_without_upgrade, is_black_lotus_template,
        };
        if !target.is_kind_of(crate::game_logic::KindOf::Structure)
            || !target.is_alive()
            || target.status.under_construction
            || target.status.sold
        {
            return false;
        }
        for &unit_id in units {
            let Some(unit) = game_logic.get_object(unit_id) else {
                continue;
            };
            if !unit.is_alive() || !unit.can_move() || unit.team == target.team {
                continue;
            }
            let is_lotus = is_black_lotus_template(&unit.template_name);
            let capture_ability = can_capture_without_upgrade(unit.is_hero(), is_lotus)
                || (unit.is_kind_of(crate::game_logic::KindOf::Infantry)
                    && game_logic.team_has_completed_capture_upgrade(unit.team));
            if capture_ability {
                return true;
            }
        }
        false
    }

    /// Validate if selected units can attack target
    fn can_attack_target(
        &self,
        units: &[ObjectId],
        target: &Object,
        game_logic: &GameLogic,
    ) -> bool {
        for &unit_id in units {
            if let Some(unit) = game_logic.get_object(unit_id) {
                if unit.can_attack() && unit.team != target.team && !target.is_dead() {
                    return true;
                }
            }
        }
        false
    }

    /// Validate if selected units can gather from a resource target
    fn can_gather_from_target(
        &self,
        units: &[ObjectId],
        target: &Object,
        game_logic: &GameLogic,
    ) -> bool {
        if !target.is_alive() {
            return false;
        }
        let target_is_resource = target.is_kind_of(KindOf::Harvestable)
            || target.is_kind_of(KindOf::Resource)
            || target.object_type == crate::game_logic::ObjectType::Supply;
        if !target_is_resource {
            return false;
        }
        // Check if any selected unit is a worker/harvester on the same team
        for &unit_id in units {
            if let Some(unit) = game_logic.get_object(unit_id) {
                if unit.is_worker()
                    && unit.team == target.team
                    && unit.is_alive()
                    && unit.can_move()
                {
                    return true;
                }
            }
        }
        false
    }

    /// Validate if selected dozers can resume construction on unfinished structure.
    fn can_resume_construction(
        &self,
        units: &[ObjectId],
        target: &Object,
        game_logic: &GameLogic,
    ) -> bool {
        if !target.is_kind_of(crate::game_logic::KindOf::Structure)
            || !target.is_alive()
            || !target.status.under_construction
        {
            return false;
        }
        for &unit_id in units {
            if let Some(unit) = game_logic.get_object(unit_id) {
                // Dozer / worker residual (can_repair covers builders).
                if unit.can_repair()
                    && unit.is_alive()
                    && unit.can_move()
                    && (unit.team == target.team || target.team == Team::Neutral)
                {
                    return true;
                }
            }
        }
        false
    }

    /// Validate if selected units can repair target
    fn can_repair_target(
        &self,
        units: &[ObjectId],
        target: &Object,
        game_logic: &GameLogic,
    ) -> bool {
        // Host residual: dozer/worker → damaged ally/neutral structure (not under construction).
        // Fail-closed: not full ActionManager canRepairObject edge matrix.
        if !target.is_kind_of(crate::game_logic::KindOf::Structure)
            || !target.is_alive()
            || target.status.under_construction
            || !target.is_damaged()
        {
            return false;
        }
        for &unit_id in units {
            if let Some(unit) = game_logic.get_object(unit_id) {
                if unit.can_repair() && (unit.team == target.team || target.team == Team::Neutral) {
                    return true;
                }
            }
        }
        false
    }

    /// Validate if selected units can enter target
    fn can_enter_target(
        &self,
        units: &[ObjectId],
        target: &Object,
        game_logic: &GameLogic,
    ) -> bool {
        if !target.can_contain() || !target.is_alive() || target.status.under_construction {
            return false;
        }

        let target_has_occupants = !target.contained_units().is_empty();
        // Structures + Overlord BattleBunker residual: infantry/heroes only.
        let infantry_only = target.is_kind_of(KindOf::Structure)
            || (target.is_overlord_style_container() && target.overlord_bunker_slot_capacity() > 0);

        for &unit_id in units {
            let Some(unit) = game_logic.get_object(unit_id) else {
                continue;
            };

            if unit_id == target.id
                || !unit.is_alive()
                || unit.status.under_construction
                || !unit.can_move()
                || unit.is_kind_of(KindOf::Structure)
            {
                continue;
            }

            if infantry_only && !unit.is_kind_of(KindOf::Infantry) && !unit.is_hero() {
                continue;
            }

            let target_contains_unit = target.contained_units().contains(&unit_id);
            let target_has_space = target.has_capacity_for(1);
            if !target_contains_unit && !target_has_space {
                continue;
            }

            if target.team != unit.team
                && target.team != Team::Neutral
                && (target.is_faction_structure() || target_has_occupants)
            {
                continue;
            }

            return true;
        }

        false
    }

    /// Validate if selected units can get services at target
    fn can_get_serviced_at_target(
        &self,
        units: &[ObjectId],
        target: &Object,
        game_logic: &GameLogic,
    ) -> bool {
        if !target.is_alive() || target.status.under_construction {
            return false;
        }

        let target_building_type = target
            .building_data
            .as_ref()
            .map(|b| b.building_type)
            .unwrap_or(BuildingType::CommandCenter);

        for &unit_id in units {
            if let Some(unit) = game_logic.get_object(unit_id) {
                if unit.team != target.team
                    || !unit.is_alive()
                    || !unit.can_move()
                    || !(unit.is_damaged() || unit.is_injured())
                {
                    continue;
                }

                let can_use_service = match target_building_type {
                    BuildingType::HealPad => unit.is_kind_of(KindOf::Infantry),
                    // USA RepairPad + China WarFactory RepairDock residual.
                    BuildingType::RepairPad | BuildingType::WarFactory => {
                        unit.is_kind_of(KindOf::Vehicle) && !unit.is_kind_of(KindOf::Aircraft)
                    }
                    BuildingType::Airfield => unit.is_kind_of(KindOf::Aircraft),
                    _ => false,
                };

                if can_use_service {
                    return true;
                }
            }
        }
        false
    }

    /// Get current selected units for a player
    pub fn get_selected_units(&self, player_id: u32, game_logic: &GameLogic) -> Vec<ObjectId> {
        if let Some(player) = game_logic.get_player(player_id) {
            player.selected_objects.clone()
        } else {
            Vec::new()
        }
    }

    /// Clear command queue
    pub fn clear_queue(&mut self) {
        self.command_queue.clear();
    }

    /// Get command history
    pub fn get_command_history(&self) -> &[GameCommand] {
        &self.command_history
    }
}

/// Global command system instance
static COMMAND_SYSTEM: OnceLock<Mutex<CommandSystem>> = OnceLock::new();

/// Initialize the global command system
pub fn init_command_system() {
    let _ = COMMAND_SYSTEM.get_or_init(|| {
        log::info!("Command system initialized");
        Mutex::new(CommandSystem::new())
    });
}

/// Get the global command system instance
pub fn get_command_system() -> &'static Mutex<CommandSystem> {
    COMMAND_SYSTEM.get_or_init(|| {
        log::info!("Command system initialized");
        Mutex::new(CommandSystem::new())
    })
}

// Extension methods for Object to support command system
pub trait CommandableObject {
    fn can_move(&self) -> bool;
    fn can_attack(&self) -> bool;
    fn can_construct(&self) -> bool;
    fn can_repair(&self) -> bool;
    fn can_contain(&self) -> bool;
    fn is_damaged(&self) -> bool;
    fn is_injured(&self) -> bool;
    fn is_dead(&self) -> bool;
    fn is_medical_facility(&self) -> bool;
    fn provides_repair(&self) -> bool;
    fn provides_healing(&self) -> bool;
    fn has_capacity_for(&self, other: &Object) -> bool;
    fn set_destination(&mut self, destination: Vec3);
    fn set_target(&mut self, target: Option<ObjectId>);
    fn set_target_location(&mut self, location: Option<Vec3>);
    fn set_guard_position(&mut self, position: Option<Vec3>);
    fn set_guard_target(&mut self, target: Option<ObjectId>);
    fn set_force_attack(&mut self, force: bool);
    fn stop(&mut self);
}

impl CommandableObject for Object {
    fn can_move(&self) -> bool {
        // Check if object has mobility
        matches!(
            self.object_type,
            crate::game_logic::ObjectType::Vehicle
                | crate::game_logic::ObjectType::Infantry
                | crate::game_logic::ObjectType::Aircraft
        )
    }

    fn can_attack(&self) -> bool {
        // Check if object has weapons
        self.health.current > 0.0
            && !matches!(self.object_type, crate::game_logic::ObjectType::Supply)
    }

    fn can_construct(&self) -> bool {
        self.can_move()
            && (self.is_kind_of(crate::game_logic::KindOf::Worker)
                || self.template_name.contains("Dozer")
                || self.template_name.contains("Worker")
                || self.template_name.contains("Harvester")
                || self.template_name.contains("Collector"))
    }

    fn can_repair(&self) -> bool {
        self.can_construct() // Dozers can repair
    }

    fn can_contain(&self) -> bool {
        Object::can_contain(self)
    }

    fn is_damaged(&self) -> bool {
        self.health.current < self.max_health && self.health.current > 0.0
    }

    fn is_injured(&self) -> bool {
        self.is_damaged() // Same as damaged for now
    }

    fn is_dead(&self) -> bool {
        self.health.current <= 0.0
    }

    fn is_medical_facility(&self) -> bool {
        self.building_data
            .as_ref()
            .map(|b| b.building_type == BuildingType::HealPad)
            .unwrap_or_else(|| {
                let lower = self.template_name.to_ascii_lowercase();
                lower.contains("hospital") || lower.contains("heal") || lower.contains("medic")
            })
    }

    fn provides_repair(&self) -> bool {
        self.building_data
            .as_ref()
            .map(|b| {
                matches!(
                    b.building_type,
                    // RepairPad + Airfield + WarFactory (China RepairDock residual).
                    BuildingType::RepairPad | BuildingType::Airfield | BuildingType::WarFactory
                )
            })
            .unwrap_or_else(|| {
                matches!(self.object_type, crate::game_logic::ObjectType::Building)
                    && (self.template_name.contains("Repair")
                        || self.template_name.contains("Service")
                        || self.template_name.contains("Airfield")
                        || self.template_name.contains("WarFactory")
                        || self.template_name.contains("War Factory"))
            })
    }

    fn provides_healing(&self) -> bool {
        self.is_medical_facility()
    }

    fn has_capacity_for(&self, _other: &Object) -> bool {
        Object::has_capacity_for(self, 1)
    }

    fn set_destination(&mut self, destination: Vec3) {
        Object::set_destination(self, destination);
    }

    fn set_target(&mut self, target: Option<ObjectId>) {
        Object::set_target(self, target);
    }

    fn set_target_location(&mut self, location: Option<Vec3>) {
        Object::set_target_location(self, location);
    }

    fn set_guard_position(&mut self, position: Option<Vec3>) {
        Object::set_guard_position(self, position);
    }

    fn set_guard_target(&mut self, target: Option<ObjectId>) {
        Object::set_guard_target(self, target);
    }

    fn set_force_attack(&mut self, force: bool) {
        Object::set_force_attack(self, force);
    }

    fn stop(&mut self) {
        Object::stop(self);
    }
}

/// Map retail/host ControlBar command button names to [`CommandType`].
///
/// Fail-closed residual: upgrade/cancel/stop/scatter core only — not full
/// CommandSet INI matrix / context-sensitive ControlBar.cpp.
pub fn command_type_from_button_name(name: &str) -> Option<CommandType> {
    let n = name.trim();
    if n.is_empty() {
        return None;
    }
    let lower: String = n
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect();
    // Drop optional "command" prefix residual.
    let key = lower.strip_prefix("command").unwrap_or(lower.as_str());

    match key {
        "stop" => Some(CommandType::Stop),
        "scatter" => Some(CommandType::Scatter),
        "tighten" | "tightentoposition" | "gatherclick" => Some(CommandType::TightenToPosition {
            destination: glam::Vec3::ZERO,
        }),
        "attackteam" => Some(CommandType::AttackTeam {
            team: 0,
            max_shots: -1,
        }),
        "overridespecialpowerdestination" | "overridespdest" => {
            Some(CommandType::OverrideSpecialPowerDestination {
                location: glam::Vec3::ZERO,
            })
        }
        "setweaponsetflag" | "weaponsetflag" => Some(CommandType::SetWeaponSetFlag {
            flag: 0,
            enabled: true,
        }),
        "followwaypointpath" | "followwaypoints" => Some(CommandType::FollowWaypointPath {
            waypoints: Vec::new(),
            exact: false,
            as_team: false,
        }),
        "followwaypointpathexact" => Some(CommandType::FollowWaypointPath {
            waypoints: Vec::new(),
            exact: true,
            as_team: false,
        }),
        "followwaypointpathasteam" | "followwaypointsasteam" => {
            Some(CommandType::FollowWaypointPath {
                waypoints: Vec::new(),
                exact: false,
                as_team: true,
            })
        }
        "followwaypointpathasteamexact" => Some(CommandType::FollowWaypointPath {
            waypoints: Vec::new(),
            exact: true,
            as_team: true,
        }),
        "attackfollowwaypointpath" | "attackfollowwaypoints" => {
            Some(CommandType::AttackFollowWaypointPath {
                waypoints: Vec::new(),
                exact: false,
                as_team: false,
            })
        }
        "attackfollowwaypointpathasteam" => Some(CommandType::AttackFollowWaypointPath {
            waypoints: Vec::new(),
            exact: false,
            as_team: true,
        }),
        "attackfollowwaypointpathexact" => Some(CommandType::AttackFollowWaypointPath {
            waypoints: Vec::new(),
            exact: true,
            as_team: false,
        }),
        "surrender" => Some(CommandType::Surrender { surrendered: true }),
        "unsurrender" => Some(CommandType::Surrender { surrendered: false }),
        "docommandbutton" => Some(CommandType::DoCommandButton {
            button: String::new(),
        }),
        "executerailedtransport" | "railedtransport" => Some(CommandType::ExecuteRailedTransport),
        "deploy" => Some(CommandType::Deploy),
        "cheer" | "allcheer" | "groupcheer" => Some(CommandType::Cheer),
        "createformation" | "formation" => Some(CommandType::CreateFormation),
        "viewcommandcenter" | "centerbase" => Some(CommandType::ViewCommandCenter),
        "viewlastradarevent" | "gotoradarevent" => Some(CommandType::ViewLastRadarEvent),
        "placebeacon" | "beacon" => Some(CommandType::PlaceBeacon {
            location: glam::Vec3::ZERO, // filled by map click
            text: String::new(),
        }),
        "removebeacon" | "deletebeacon" => Some(CommandType::RemoveBeacon),
        "attackmove" | "attackmoveto" => Some(CommandType::AttackMoveTo {
            destination: glam::Vec3::ZERO, // filled by dispatch from cursor/world,
            max_shots: -1,
        }),
        "setrallypoint" => Some(CommandType::SetRallyPoint {
            location: glam::Vec3::ZERO, // filled by dispatch
        }),
        "guard" | "guardarea" | "guardposition" => Some(CommandType::Guard {
            target: GuardTarget::Position(glam::Vec3::ZERO),
            mode: crate::game_logic::GuardMode::Normal,
        }),
        "guardwithoutpursuit" | "guard_without_pursuit" => Some(CommandType::Guard {
            target: GuardTarget::Position(glam::Vec3::ZERO),
            mode: crate::game_logic::GuardMode::WithoutPursuit,
        }),
        "guardflying" | "guardflyingunits" | "guardflyingunitsonly" | "guard_flying_units_only" => {
            Some(CommandType::Guard {
                target: GuardTarget::Position(glam::Vec3::ZERO),
                mode: crate::game_logic::GuardMode::FlyingUnitsOnly,
            })
        }
        "patrol" | "hunt" => Some(CommandType::Patrol),
        "attackposition" => Some(CommandType::AttackPosition {
            location: Some(glam::Vec3::ZERO),
            max_shots: -1,
        }),
        "attackself" | "attackownposition" => Some(CommandType::AttackPosition {
            location: None,
            max_shots: -1,
        }),
        "attitudesleep" | "sleep" | "holdfire" => Some(CommandType::AttitudeSleep),
        "attitudepassive" | "passive" | "defend" => Some(CommandType::AttitudePassive),
        "attitudenormal" | "normal" | "normalstance" => Some(CommandType::AttitudeNormal),
        "attitudeaggressive" | "aggressive" | "attackmoveaggression" => {
            Some(CommandType::AttitudeAggressive)
        }
        "evacuate" | "structureexit" => Some(CommandType::Evacuate),
        "movetoandevacuate" | "moveevacuate" => Some(CommandType::MoveToAndEvacuate {
            destination: glam::Vec3::ZERO,
            and_exit: false,
        }),
        "movetoandevacuateandexit" | "moveevacuateexit" => Some(CommandType::MoveToAndEvacuate {
            destination: glam::Vec3::ZERO,
            and_exit: true,
        }),
        "repair" => Some(CommandType::Repair {
            target_id: ObjectId(0),
        }),
        "exit" => Some(CommandType::Exit),
        "sell" => Some(CommandType::Sell {
            object_id: crate::game_logic::ObjectId(0), // filled by dispatch
        }),
        // GeneralsExperience science purchase residual (name filled by UI/hotkey).
        "purchasescience" | "buyscience" => Some(CommandType::PurchaseScience {
            science_name: String::new(),
        }),
        "switchweapons" | "switchweapon" | "toggleweapon" => Some(CommandType::SwitchWeapons),
        "combatdrop" | "rappell" | "rappel" => Some(CommandType::CombatDrop {
            target: crate::command_system::DropTarget::Location(glam::Vec3::ZERO),
        }),
        "hackinternet" | "internet" | "starthacking" => Some(CommandType::HackInternet),
        "returntobase" | "rtb" | "land" => Some(CommandType::ReturnToBase),
        "resumeconstruction" | "resume" => Some(CommandType::ResumeConstruction {
            target_id: crate::game_logic::ObjectId(0),
        }),
        "returnsupplies" | "returncargo" | "forcesupplyreturn" => Some(CommandType::ReturnSupplies),
        "cleanuparea" | "detox" | "clearhazards" => Some(CommandType::DoSpecialPower {
            power_type: SpecialPowerType::CleanupArea,
            target: PowerTarget::None,
        }),
        "clearmines" | "disarmmines" => Some(CommandType::ClearMines),
        "setmineclearingdetail" | "mineclearingdetail" | "mineclearing" => {
            Some(CommandType::SetMineClearingDetail { enabled: true })
        }
        "clearmineclearingdetail" | "nomineclearing" => {
            Some(CommandType::SetMineClearingDetail { enabled: false })
        }
        "goprone" | "prone" | "hitthedirt" => Some(CommandType::GoProne),
        "setweaponlock" | "weaponlock" | "lockweapon" => Some(CommandType::SetWeaponLock {
            slot: 1,      // default secondary lock residual
            lock_type: 2, // permanent
        }),
        "releaseweaponlock" | "unlockweapon" => {
            Some(CommandType::ReleaseWeaponLock { lock_type: 2 })
        }
        "setemoticon" | "emoticon" => Some(CommandType::SetEmoticon {
            name: "Emoticon_Smile".into(),
            duration_frames: 90, // 3s @ 30Hz
        }),
        "attackarea" => Some(CommandType::AttackArea {
            center: glam::Vec3::ZERO,
            radius: 150.0,
        }),
        // Generic ControlBar SW button residual — power type resolved at arm time.
        "specialpower" | "dospecialpower" => Some(CommandType::DoSpecialPower {
            power_type: SpecialPowerType::ParticleCannon, // placeholder; engine resolves
            target: PowerTarget::None,
        }),
        // USA Strategy Center battle plans residual (immediate, no map click).
        "initiatebattleplanbombardment" | "battleplanbombardment" => {
            Some(CommandType::DoSpecialPower {
                power_type: SpecialPowerType::BattlePlanBombardment,
                target: PowerTarget::None,
            })
        }
        "initiatebattleplanholdtheline" | "battleplanholdtheline" => {
            Some(CommandType::DoSpecialPower {
                power_type: SpecialPowerType::BattlePlanHoldTheLine,
                target: PowerTarget::None,
            })
        }
        "initiatebattleplansearchanddestroy" | "battleplansearchanddestroy" => {
            Some(CommandType::DoSpecialPower {
                power_type: SpecialPowerType::BattlePlanSearchAndDestroy,
                target: PowerTarget::None,
            })
        }
        // Named superweapon / intel residual button names (map-click arm).
        "spysatellitescan" | "spysatellite" => Some(CommandType::DoSpecialPower {
            power_type: SpecialPowerType::SpySatellite,
            target: PowerTarget::None,
        }),
        "ciaintelligence" | "ciaintel" => Some(CommandType::DoSpecialPower {
            power_type: SpecialPowerType::CiaIntelligence,
            target: PowerTarget::None,
        }),
        "spydrone" => Some(CommandType::DoSpecialPower {
            power_type: SpecialPowerType::SpyDrone,
            target: PowerTarget::None,
        }),
        "particlecannon" | "fireparticlecannon" => Some(CommandType::DoSpecialPower {
            power_type: SpecialPowerType::ParticleCannon,
            target: PowerTarget::None,
        }),
        "nuclearmissile" | "launchnuclearmissile" => Some(CommandType::DoSpecialPower {
            power_type: SpecialPowerType::NuclearMissile,
            target: PowerTarget::None,
        }),
        "scudstorm" | "launchesudstorm" | "launchscudstorm" => Some(CommandType::DoSpecialPower {
            power_type: SpecialPowerType::ScudStorm,
            target: PowerTarget::None,
        }),
        "carpetbomb" | "chinacarpetbomb" | "americacarpetbomb" => {
            Some(CommandType::DoSpecialPower {
                power_type: SpecialPowerType::CarpetBomb,
                target: PowerTarget::None,
            })
        }
        "artillerybarrage" | "artillery" => Some(CommandType::DoSpecialPower {
            power_type: SpecialPowerType::Artillery,
            target: PowerTarget::None,
        }),
        "emergencyrepair" | "repairvehicles" => Some(CommandType::DoSpecialPower {
            power_type: SpecialPowerType::EmergencyRepair,
            target: PowerTarget::None,
        }),
        "airstrike" | "spectreairstrike" => Some(CommandType::DoSpecialPower {
            power_type: SpecialPowerType::Airstrike,
            target: PowerTarget::None,
        }),
        "ambush" | "rebelambush" => Some(CommandType::DoSpecialPower {
            power_type: SpecialPowerType::Ambush,
            target: PowerTarget::None,
        }),
        "sneakattack" => Some(CommandType::DoSpecialPower {
            power_type: SpecialPowerType::SneakAttack,
            target: PowerTarget::None,
        }),
        "leafletdrop" | "leaflet" => Some(CommandType::DoSpecialPower {
            power_type: SpecialPowerType::LeafletDrop,
            target: PowerTarget::None,
        }),
        "gpsscrambler" | "gps" => Some(CommandType::DoSpecialPower {
            power_type: SpecialPowerType::GpsScrambler,
            target: PowerTarget::None,
        }),
        "spectregunship" | "spectre" => Some(CommandType::DoSpecialPower {
            power_type: SpecialPowerType::SpectreGunship,
            target: PowerTarget::None,
        }),
        "anthraxbomb" | "anthrax" => Some(CommandType::DoSpecialPower {
            power_type: SpecialPowerType::AnthraxBomb,
            target: PowerTarget::None,
        }),
        "cancelupgrade" => Some(CommandType::CancelUpgrade {
            upgrade_name: String::new(),
        }),
        "cancelunit" | "cancelunitcreate" => Some(CommandType::CancelUnitCreate {
            template_name: String::new(),
        }),
        "cancelconstruction" => Some(CommandType::DozerCancelConstruct {
            object_id: crate::game_logic::ObjectId(0),
        }),
        // Unit special-ability residual (target filled by map click).
        "hijack" | "hijackvehicle" => Some(CommandType::Hijack {
            target_id: crate::game_logic::ObjectId(0),
        }),
        "sabotage" | "sabotagebuilding" => Some(CommandType::Sabotage {
            target_id: crate::game_logic::ObjectId(0),
        }),
        "capturebuilding"
        | "rangercapturebuilding"
        | "redguardcapturebuilding"
        | "rebelcapturebuilding"
        | "blacklotuscapturebuilding" => Some(CommandType::CaptureBuilding {
            target_id: crate::game_logic::ObjectId(0),
        }),
        "snipevehicle" | "jarmenkellsnipe" | "snipe" => Some(CommandType::SnipeVehicle {
            target_id: crate::game_logic::ObjectId(0),
        }),
        "planttimeddemocharge" | "timeddemocharge" | "planttimedcharge" => {
            Some(CommandType::PlantTimedDemoCharge {
                target_id: crate::game_logic::ObjectId(0),
            })
        }
        "plantremotedemocharge" | "remotedemocharge" | "plantremotecharge" => {
            Some(CommandType::PlantRemoteDemoCharge {
                target_id: crate::game_logic::ObjectId(0),
            })
        }
        "detonateremotedemocharges" | "detonateremotecharges" => {
            Some(CommandType::DetonateRemoteDemoCharges)
        }
        "stealcashhack" | "blacklotusstealcash" => Some(CommandType::StealCashHack {
            target_id: crate::game_logic::ObjectId(0),
        }),
        "disablevehiclehack" | "blacklotusdisablevehicle" => {
            Some(CommandType::DisableVehicleHack {
                target_id: crate::game_logic::ObjectId(0),
            })
        }
        "hackerdisablebuilding" | "disablebuilding" => Some(CommandType::HackerDisableBuilding {
            target_id: crate::game_logic::ObjectId(0),
        }),
        "disguiseasvehicle" | "disguise" => Some(CommandType::DisguiseAsVehicle {
            target_id: crate::game_logic::ObjectId(0),
        }),
        "plantboobytrap" | "boobytrap" => Some(CommandType::PlantBoobyTrap {
            target_id: crate::game_logic::ObjectId(0),
        }),
        "converttocarbomb" | "carbomb" => Some(CommandType::ConvertToCarbomb {
            target_id: crate::game_logic::ObjectId(0),
        }),
        "demotertiarysuicide" | "suicidebomb" | "tertiarysuicide" => {
            Some(CommandType::DemoTertiarySuicide)
        }
        "toggleovercharge" | "overcharge" => Some(CommandType::ToggleOvercharge),
        _ => {
            // Command_UpgradeAmericaX / Command_Upgrade_GLA… → Upgrade_AmericaX
            if let Some(rest) = key.strip_prefix("upgrade") {
                if rest.is_empty() {
                    return None;
                }
                // Rebuild from original casing after Command_/Upgrade prefix.
                let stripped = n
                    .trim()
                    .trim_start_matches("Command_")
                    .trim_start_matches("command_")
                    .trim_start_matches("Command")
                    .trim_start_matches("command");
                let body = stripped
                    .strip_prefix("Upgrade_")
                    .or_else(|| stripped.strip_prefix("Upgrade"))
                    .or_else(|| stripped.strip_prefix("upgrade_"))
                    .or_else(|| stripped.strip_prefix("upgrade"))
                    .unwrap_or(rest)
                    .trim_start_matches('_');
                let upgrade_name = format!("Upgrade_{body}");
                Some(CommandType::QueueUpgrade { upgrade_name })
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{GameLogic, Object, ObjectType};
    use game_engine::common::global_data::with_global_data_restored;

    #[test]
    fn test_command_creation() {
        let mut system = CommandSystem::new();
        let context = MouseCommandContext {
            world_position: Vec3::new(100.0, 0.0, 100.0),
            target_object: None,
            screen_position: Vec2::new(400.0, 300.0),
            viewport_size: None,
            world_min: None,
            world_max: None,
            mouse_button: MouseButton::Right,
            modifier_keys: ModifierKeys::default(),
            is_drag: false,
            drag_start: None,
            drag_end: None,
            drag_start_world: None,
            drag_end_world: None,
        };

        let game_logic = GameLogic::new();
        let selected_units = vec![ObjectId(1)];

        if let Some(command) = system.process_mouse_input(&context, &selected_units, 0, &game_logic)
        {
            match command.command_type {
                CommandType::MoveTo { destination, .. } => {
                    assert_eq!(destination, Vec3::new(100.0, 0.0, 100.0));
                }
                _ => panic!("Expected MoveTo command"),
            }
        } else {
            panic!("Expected command to be created");
        }
    }

    #[test]
    fn test_command_execution() {
        use crate::game_logic::{KindOf, Team, ThingTemplate};

        let system = CommandSystem::new();
        let mut game_logic = GameLogic::new();

        // Create test object using a minimal thing template
        let mut template = ThingTemplate::new("TestUnit");
        template.add_kind_of(KindOf::Vehicle);
        template.set_health(100.0);

        let mut obj = Object::new(template, ObjectId(1), Team::USA);
        obj.position = Vec3::new(0.0, 0.0, 0.0);
        game_logic.add_object(obj);

        let command = GameCommand {
            command_type: CommandType::MoveTo {
                destination: Vec3::new(50.0, 0.0, 50.0),
                waypoints: Vec::new(),
            },
            player_id: 0,
            command_id: 1,
            timestamp: SystemTime::now(),
            selected_units: vec![ObjectId(1)],
            modifier_keys: ModifierKeys::default(),
        };

        let result = system.execute_command(&command, &mut game_logic);
        assert_eq!(result, CommandResult::Success);
    }

    #[test]
    fn right_click_heal_pad_issues_get_healed() {
        use crate::game_logic::{KindOf, Team, ThingTemplate};

        let mut system = CommandSystem::new();
        let mut game_logic = GameLogic::new();

        let mut infantry_template = ThingTemplate::new("TestInfantry");
        infantry_template
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        let mut infantry = Object::new(infantry_template, ObjectId(1), Team::USA);
        // Damage authority freezes mid-frame HP on take_damage; set current directly
        // so is_damaged() is observable without a shadow writeback session.
        infantry.health.current = (infantry.health.maximum - 25.0).max(1.0);
        game_logic.add_object(infantry);

        let mut heal_pad_template = ThingTemplate::new("TestHealPad");
        heal_pad_template
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .set_health(900.0);
        let heal_pad = Object::new(heal_pad_template, ObjectId(2), Team::USA);
        game_logic.add_object(heal_pad);

        let context = MouseCommandContext {
            world_position: Vec3::new(0.0, 0.0, 0.0),
            target_object: Some(ObjectId(2)),
            screen_position: Vec2::new(0.0, 0.0),
            viewport_size: None,
            world_min: None,
            world_max: None,
            mouse_button: MouseButton::Right,
            modifier_keys: ModifierKeys::default(),
            is_drag: false,
            drag_start: None,
            drag_end: None,
            drag_start_world: None,
            drag_end_world: None,
        };

        let command = system
            .process_mouse_input(&context, &[ObjectId(1)], 0, &game_logic)
            .expect("right click should generate a command");
        assert!(
            matches!(
                command.command_type,
                CommandType::GetHealed {
                    target_id: ObjectId(2)
                }
            ),
            "heal pad target should issue GetHealed"
        );
    }

    #[test]
    fn right_click_repair_pad_issues_get_repaired() {
        use crate::game_logic::{KindOf, Team, ThingTemplate};

        let mut system = CommandSystem::new();
        let mut game_logic = GameLogic::new();

        let mut vehicle_template = ThingTemplate::new("TestTank");
        vehicle_template
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .set_health(250.0);
        let mut vehicle = Object::new(vehicle_template, ObjectId(10), Team::USA);
        // Damage authority freezes mid-frame HP on take_damage; set current directly
        // so is_damaged() is observable without a shadow writeback session.
        vehicle.health.current = (vehicle.health.maximum - 30.0).max(1.0);
        game_logic.add_object(vehicle);

        let mut repair_pad_template = ThingTemplate::new("TestRepairPad");
        repair_pad_template
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .set_health(1000.0);
        let repair_pad = Object::new(repair_pad_template, ObjectId(11), Team::USA);
        game_logic.add_object(repair_pad);

        let context = MouseCommandContext {
            world_position: Vec3::new(0.0, 0.0, 0.0),
            target_object: Some(ObjectId(11)),
            screen_position: Vec2::new(0.0, 0.0),
            viewport_size: None,
            world_min: None,
            world_max: None,
            mouse_button: MouseButton::Right,
            modifier_keys: ModifierKeys::default(),
            is_drag: false,
            drag_start: None,
            drag_end: None,
            drag_start_world: None,
            drag_end_world: None,
        };

        let command = system
            .process_mouse_input(&context, &[ObjectId(10)], 0, &game_logic)
            .expect("right click should generate a command");
        assert!(
            matches!(
                command.command_type,
                CommandType::GetRepaired {
                    target_id: ObjectId(11)
                }
            ),
            "repair pad target should issue GetRepaired"
        );
    }

    #[test]
    fn drag_selection_prefers_world_drag_bounds_when_provided() {
        use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

        let mut system = CommandSystem::new();
        let mut game_logic = GameLogic::new();
        game_logic.add_player(Player::new(0, Team::USA, "TestPlayer", true));

        let mut template = ThingTemplate::new("TestUnit");
        template
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);

        let mut near = Object::new(template.clone(), ObjectId(31), Team::USA);
        near.set_position(Vec3::new(10.0, 0.0, 10.0));
        game_logic.add_object(near);

        let mut far = Object::new(template, ObjectId(32), Team::USA);
        far.set_position(Vec3::new(120.0, 0.0, 120.0));
        game_logic.add_object(far);

        let context = MouseCommandContext {
            world_position: Vec3::new(0.0, 0.0, 0.0),
            target_object: None,
            screen_position: Vec2::new(0.0, 0.0),
            viewport_size: Some(Vec2::new(1024.0, 768.0)),
            world_min: Some(Vec3::new(-256.0, 0.0, -256.0)),
            world_max: Some(Vec3::new(256.0, 0.0, 256.0)),
            mouse_button: MouseButton::Left,
            modifier_keys: ModifierKeys::default(),
            is_drag: true,
            drag_start: Some(Vec2::new(999.0, 999.0)),
            drag_end: Some(Vec2::new(1000.0, 1000.0)),
            drag_start_world: Some(Vec3::new(0.0, 0.0, 0.0)),
            drag_end_world: Some(Vec3::new(50.0, 0.0, 50.0)),
        };

        let command = system
            .process_mouse_input(&context, &[], 0, &game_logic)
            .expect("drag selection should produce command");

        match command.command_type {
            CommandType::CreateSelectedGroup { units, .. } => {
                assert!(units.contains(&ObjectId(31)));
                assert!(!units.contains(&ObjectId(32)));
            }
            other => panic!("expected drag CreateSelectedGroup command, got {other:?}"),
        }
    }

    #[test]
    fn queue_upgrade_deducts_once_per_team_and_prevents_duplicate_queue() {
        use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

        let system = CommandSystem::new();
        let mut game_logic = GameLogic::new();
        let mut player = Player::new(0, Team::USA, "USA", true);
        player.resources.supplies = 5000;
        game_logic.add_player(player);

        let mut template = ThingTemplate::new("AmericaSupplyCenter");
        template
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);

        let producer_a = Object::new(template.clone(), ObjectId(201), Team::USA);
        let producer_b = Object::new(template, ObjectId(202), Team::USA);
        game_logic.add_object(producer_a);
        game_logic.add_object(producer_b);

        let queue_command = GameCommand {
            command_type: CommandType::QueueUpgrade {
                upgrade_name: "Upgrade_AmericaSupplyLines".to_string(),
            },
            player_id: 0,
            command_id: 1,
            timestamp: SystemTime::now(),
            selected_units: vec![ObjectId(201), ObjectId(202)],
            modifier_keys: ModifierKeys::default(),
        };

        let first_result = system.execute_command(&queue_command, &mut game_logic);
        assert_eq!(first_result, CommandResult::Success);

        let player_after_first = game_logic.get_player(0).expect("player should exist");
        assert_eq!(
            player_after_first.effective_supplies(), 4200,
            "upgrade cost should be charged once per team, not per selected unit (retail SupplyLines=800)"
        );
        assert!(player_after_first
            .queued_upgrades
            .contains("Upgrade_AmericaSupplyLines"));

        let second_result = system.execute_command(&queue_command, &mut game_logic);
        assert_eq!(second_result, CommandResult::InvalidCommand);
    }

    #[test]
    fn queue_upgrade_identity_matches_ini_name_variants() {
        use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

        let system = CommandSystem::new();
        let mut game_logic = GameLogic::new();
        let mut player = Player::new(0, Team::USA, "USA", true);
        player.resources.supplies = 5000;
        game_logic.add_player(player);

        let mut template = ThingTemplate::new("AmericaSupplyCenter");
        template
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        game_logic.add_object(Object::new(template, ObjectId(251), Team::USA));

        let queue_command = GameCommand {
            command_type: CommandType::QueueUpgrade {
                upgrade_name: "Upgrade_AmericaSupplyLines".to_string(),
            },
            player_id: 0,
            command_id: 30,
            timestamp: SystemTime::now(),
            selected_units: vec![ObjectId(251)],
            modifier_keys: ModifierKeys::default(),
        };
        assert_eq!(
            system.execute_command(&queue_command, &mut game_logic),
            CommandResult::Success
        );

        let variant_command = GameCommand {
            command_type: CommandType::QueueUpgrade {
                upgrade_name: "upgradeamericasupplylines".to_string(),
            },
            player_id: 0,
            command_id: 31,
            timestamp: SystemTime::now(),
            selected_units: vec![ObjectId(251)],
            modifier_keys: ModifierKeys::default(),
        };
        assert_eq!(
            system.execute_command(&variant_command, &mut game_logic),
            CommandResult::InvalidCommand,
            "same upgrade should not be charged twice when naming style differs"
        );

        let cancel_variant = GameCommand {
            command_type: CommandType::CancelUpgrade {
                upgrade_name: "UPGRADE_AMERICA_SUPPLY_LINES".to_string(),
            },
            player_id: 0,
            command_id: 32,
            timestamp: SystemTime::now(),
            selected_units: vec![ObjectId(251)],
            modifier_keys: ModifierKeys::default(),
        };
        assert_eq!(
            system.execute_command(&cancel_variant, &mut game_logic),
            CommandResult::Success,
            "cancel should find the queued upgrade by normalized INI identity"
        );

        let player = game_logic.get_player(0).expect("player should exist");
        assert_eq!(player.effective_supplies(), 5000);
        assert!(player.queued_upgrades.is_empty());
    }

    #[test]
    fn purchase_science_identity_matches_command_name_variants() {
        use crate::game_logic::{Player, Team};

        let system = CommandSystem::new();
        let mut game_logic = GameLogic::new();
        let mut player = Player::new(0, Team::USA, "USA", true);
        player.resources.supplies = 3000;
        // C++ residual: science purchase points, not supplies.
        player.science_purchase_points = 2;
        player.unlocked_sciences.insert("SCIENCE_AMERICA".into());
        player.unlocked_sciences.insert("SCIENCE_Rank1".into());
        game_logic.add_player(player);

        let purchase_command = GameCommand {
            command_type: CommandType::PurchaseScience {
                science_name: "A10Strike1".to_string(),
            },
            player_id: 0,
            command_id: 40,
            timestamp: SystemTime::now(),
            selected_units: Vec::new(),
            modifier_keys: ModifierKeys::default(),
        };
        assert_eq!(
            system.execute_command(&purchase_command, &mut game_logic),
            CommandResult::Success
        );

        let variant_command = GameCommand {
            command_type: CommandType::PurchaseScience {
                science_name: "a10_strike_1".to_string(),
            },
            player_id: 0,
            command_id: 41,
            timestamp: SystemTime::now(),
            selected_units: Vec::new(),
            modifier_keys: ModifierKeys::default(),
        };
        assert_eq!(
            system.execute_command(&variant_command, &mut game_logic),
            CommandResult::InvalidCommand,
            "same science should not be charged twice when naming style differs"
        );

        let player = game_logic.get_player(0).expect("player should exist");
        assert_eq!(
            player.effective_supplies(),
            3000,
            "science purchase must not spend supplies residual"
        );
        assert_eq!(
            player.science_purchase_points, 1,
            "one point spent residual"
        );
        assert!(
            player.has_unlocked_science("SCIENCE_A10ThunderboltMissileStrike1"),
            "canonical A10 science residual"
        );
    }

    #[test]
    fn sell_refunds_queued_production() {
        use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

        with_global_data_restored(|| {
            game_engine::common::global_data::write().sell_percentage = 0.5;

            let system = CommandSystem::new();
            let mut game_logic = GameLogic::new();
            let mut player = Player::new(0, Team::USA, "USA", true);
            player.resources.supplies = 1_000;
            game_logic.add_player(player);

            let mut barracks = ThingTemplate::new("TestBarracks");
            barracks
                .add_kind_of(KindOf::Structure)
                .add_kind_of(KindOf::Selectable)
                .set_health(1_000.0)
                .set_cost(1_000, -1);
            game_logic
                .templates
                .insert("TestBarracks".to_string(), barracks);

            let mut infantry = ThingTemplate::new("TestInfantry");
            infantry
                .add_kind_of(KindOf::Infantry)
                .add_kind_of(KindOf::Selectable)
                .set_health(100.0)
                .set_cost(100, 0);
            game_logic
                .templates
                .insert("TestInfantry".to_string(), infantry);

            let barracks_id = game_logic
                .create_object("TestBarracks", Team::USA, Vec3::ZERO)
                .expect("barracks should be created");

            let queue_command = GameCommand {
                command_type: CommandType::QueueUnitCreate {
                    template_name: "TestInfantry".to_string(),
                    quantity: 1,
                },
                player_id: 0,
                command_id: 50,
                timestamp: SystemTime::now(),
                selected_units: vec![barracks_id],
                modifier_keys: ModifierKeys::default(),
            };
            assert_eq!(
                system.execute_command(&queue_command, &mut game_logic),
                CommandResult::Success
            );
            assert_eq!(
                game_logic.get_player(0).unwrap().effective_supplies(),
                900,
                "queued unit should charge before selling"
            );

            let sell_command = GameCommand {
                command_type: CommandType::Sell {
                    object_id: barracks_id,
                },
                player_id: 0,
                command_id: 51,
                timestamp: SystemTime::now(),
                selected_units: vec![barracks_id],
                modifier_keys: ModifierKeys::default(),
            };
            assert_eq!(
                system.execute_command(&sell_command, &mut game_logic),
                CommandResult::Success
            );

            // C++ BuildAssistant::sellObject cancels production at sell start;
            // structure refund deposits when sell finishes (~90 frames).
            assert_eq!(
                game_logic.get_player(0).unwrap().effective_supplies(),
                1_000,
                "sell start should refund queued production immediately"
            );
            assert!(
                game_logic
                    .find_object(barracks_id)
                    .map(|object| object.status.sold)
                    .unwrap_or(false),
                "sell start should mark structure sold residual"
            );
            assert!(
                game_logic
                    .find_object(barracks_id)
                    .and_then(|object| object.building_data.as_ref())
                    .map(|building| building.production_queue.is_empty())
                    .unwrap_or(true),
                "sell should drain queued production at sell start"
            );

            // Advance multi-frame sell residual to completion.
            for step in 1..=200u64 {
                game_logic.set_current_frame(step);
                game_logic.update_sell_list();
                game_logic.process_destroy_list();
                if game_logic.find_object(barracks_id).is_none() {
                    break;
                }
            }
            assert!(
                game_logic.find_object(barracks_id).is_none(),
                "sell finish should destroy structure"
            );
            assert_eq!(
                game_logic.get_player(0).unwrap().effective_supplies(),
                1_500,
                "selling should refund both the structure sell value and queued production"
            );
        });
    }

    #[test]
    fn sell_refund_uses_global_sell_percentage() {
        use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

        with_global_data_restored(|| {
            game_engine::common::global_data::write().sell_percentage = 0.25;

            let system = CommandSystem::new();
            let mut game_logic = GameLogic::new();
            let mut player = Player::new(0, Team::USA, "USA", true);
            player.resources.supplies = 0;
            game_logic.add_player(player);

            let mut barracks = ThingTemplate::new("TestBarracks");
            barracks
                .add_kind_of(KindOf::Structure)
                .add_kind_of(KindOf::Selectable)
                .set_health(1_000.0)
                .set_cost(1_000, -1);
            game_logic
                .templates
                .insert("TestBarracks".to_string(), barracks);

            let barracks_id = game_logic
                .create_object("TestBarracks", Team::USA, Vec3::ZERO)
                .expect("barracks should be created");

            // Re-assert sell percentage immediately before sell so the production
            // path is proven to consume the live GlobalData value under isolation.
            assert!(
                (game_engine::common::global_data::read().sell_percentage - 0.25).abs()
                    < f32::EPSILON,
                "test isolation must preserve configured SellPercentage"
            );

            let sell_command = GameCommand {
                command_type: CommandType::Sell {
                    object_id: barracks_id,
                },
                player_id: 0,
                command_id: 52,
                timestamp: SystemTime::now(),
                selected_units: vec![barracks_id],
                modifier_keys: ModifierKeys::default(),
            };
            assert_eq!(
                system.execute_command(&sell_command, &mut game_logic),
                CommandResult::Success
            );

            // Structure refund deposits at sell finish (C++ BuildAssistant::update).
            for step in 1..=200u64 {
                game_logic.set_current_frame(step);
                game_logic.update_sell_list();
                game_logic.process_destroy_list();
                if game_logic.find_object(barracks_id).is_none() {
                    break;
                }
            }
            assert!(
                game_logic.find_object(barracks_id).is_none(),
                "sell finish should destroy structure"
            );
            assert_eq!(
                game_logic.get_player(0).unwrap().effective_supplies(),
                250,
                "sell refund should use GlobalData SellPercentage (effective under economy auth)"
            );
        });
    }

    #[test]
    fn cancel_construction_refunds_full_build_cost() {
        use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

        let system = CommandSystem::new();
        let mut game_logic = GameLogic::new();
        let mut player = Player::new(0, Team::USA, "USA", true);
        player.resources.supplies = 0;
        game_logic.add_player(player);

        let mut barracks = ThingTemplate::new("TestBarracks");
        barracks
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .set_health(1_000.0)
            .set_cost(1_000, -1);
        game_logic
            .templates
            .insert("TestBarracks".to_string(), barracks);

        let barracks_id = game_logic
            .create_object_under_construction("TestBarracks", Team::USA, Vec3::ZERO)
            .expect("under-construction barracks should be created");

        let cancel_command = GameCommand {
            command_type: CommandType::DozerCancelConstruct {
                object_id: barracks_id,
            },
            player_id: 0,
            command_id: 60,
            timestamp: SystemTime::now(),
            selected_units: vec![],
            modifier_keys: ModifierKeys::default(),
        };

        assert_eq!(
            system.execute_command(&cancel_command, &mut game_logic),
            CommandResult::Success
        );
        game_logic.update();

        assert!(
            game_logic.get_object(barracks_id).is_none(),
            "cancelled construction should be destroyed"
        );
        assert_eq!(
            game_logic.get_player(0).unwrap().effective_supplies(),
            1_000,
            "C++ dozer cancel refunds the full build cost"
        );
    }

    #[test]
    fn cancel_construction_rejects_enemy_structure() {
        use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

        let system = CommandSystem::new();
        let mut game_logic = GameLogic::new();
        let mut usa = Player::new(0, Team::USA, "USA", true);
        usa.resources.supplies = 0;
        game_logic.add_player(usa);
        let mut gla = Player::new(2, Team::GLA, "GLA", false);
        gla.resources.supplies = 0;
        game_logic.add_player(gla);

        let mut barracks = ThingTemplate::new("TestBarracks");
        barracks
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .set_health(1_000.0)
            .set_cost(1_000, -1);
        game_logic
            .templates
            .insert("TestBarracks".to_string(), barracks);

        let barracks_id = game_logic
            .create_object_under_construction("TestBarracks", Team::USA, Vec3::ZERO)
            .expect("under-construction barracks should be created");

        let cancel_command = GameCommand {
            command_type: CommandType::DozerCancelConstruct {
                object_id: barracks_id,
            },
            player_id: 2,
            command_id: 61,
            timestamp: SystemTime::now(),
            selected_units: vec![],
            modifier_keys: ModifierKeys::default(),
        };

        assert_eq!(
            system.execute_command(&cancel_command, &mut game_logic),
            CommandResult::InvalidTarget
        );
        game_logic.update();

        assert!(
            game_logic.get_object(barracks_id).is_some(),
            "enemy cancel command must not destroy the target"
        );
        assert_eq!(
            game_logic.get_player(2).unwrap().effective_supplies(),
            0,
            "enemy cancel command must not refund the issuing player"
        );
    }

    #[test]
    fn right_click_ctrl_force_attacks_object_residual() {
        use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

        let mut logic = GameLogic::new();
        logic.add_player(Player::new(0, Team::USA, "USA", true));

        let mut ranger_t = ThingTemplate::new("AmericaInfantryRanger");
        ranger_t
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        logic
            .templates
            .insert("AmericaInfantryRanger".into(), ranger_t);
        let mut rebel_t = ThingTemplate::new("GLAInfantryRebel");
        rebel_t
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        logic.templates.insert("GLAInfantryRebel".into(), rebel_t);

        let attacker = logic
            .create_object(
                "AmericaInfantryRanger",
                Team::USA,
                glam::Vec3::new(0.0, 0.0, 0.0),
            )
            .expect("attacker");
        let target = logic
            .create_object(
                "GLAInfantryRebel",
                Team::GLA,
                glam::Vec3::new(50.0, 0.0, 0.0),
            )
            .expect("target");

        let ctx = MouseCommandContext {
            world_position: glam::Vec3::new(50.0, 0.0, 0.0),
            target_object: Some(target),
            screen_position: glam::Vec2::ZERO,
            viewport_size: None,
            world_min: None,
            world_max: None,
            mouse_button: MouseButton::Right,
            modifier_keys: ModifierKeys {
                ctrl: true,
                shift: false,
                alt: false,
            },
            is_drag: false,
            drag_start: None,
            drag_end: None,
            drag_start_world: None,
            drag_end_world: None,
        };
        let mut sys = CommandSystem::new();
        let cmd = sys
            .process_mouse_input(&ctx, &[attacker], 0, &logic)
            .expect("ctrl RMB should produce command");
        match cmd.command_type {
            CommandType::ForceAttackObject { target_id } => assert_eq!(target_id, target),
            other => panic!("expected ForceAttackObject, got {other:?}"),
        }
    }

    #[test]
    fn right_click_ctrl_force_attacks_ground_residual() {
        use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

        let mut logic = GameLogic::new();
        logic.add_player(Player::new(0, Team::USA, "USA", true));

        let mut ranger_t = ThingTemplate::new("AmericaInfantryRanger");
        ranger_t
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        logic
            .templates
            .insert("AmericaInfantryRanger".into(), ranger_t);

        let attacker = logic
            .create_object(
                "AmericaInfantryRanger",
                Team::USA,
                glam::Vec3::new(0.0, 0.0, 0.0),
            )
            .expect("attacker");

        let loc = glam::Vec3::new(80.0, 0.0, 40.0);
        let ctx = MouseCommandContext {
            world_position: loc,
            target_object: None,
            screen_position: glam::Vec2::ZERO,
            viewport_size: None,
            world_min: None,
            world_max: None,
            mouse_button: MouseButton::Right,
            modifier_keys: ModifierKeys {
                ctrl: true,
                shift: false,
                alt: false,
            },
            is_drag: false,
            drag_start: None,
            drag_end: None,
            drag_start_world: None,
            drag_end_world: None,
        };
        let mut sys = CommandSystem::new();
        let cmd = sys
            .process_mouse_input(&ctx, &[attacker], 0, &logic)
            .expect("ctrl RMB ground should produce command");
        match cmd.command_type {
            CommandType::ForceAttackGround { location } => {
                assert!((location - loc).length() < 0.1);
            }
            other => panic!("expected ForceAttackGround, got {other:?}"),
        }
    }

    fn right_click_damaged_vehicle_get_repaired_context_residual() {
        use crate::game_logic::{
            buildings::{BuildingData, BuildingType},
            KindOf, Player, Team, ThingTemplate,
        };

        let mut logic = GameLogic::new();
        let mut player = Player::new(0, Team::USA, "USA", true);
        logic.add_player(player);

        let mut tank_t = ThingTemplate::new("AmericaTankCrusader");
        tank_t
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .set_health(500.0);
        logic.templates.insert("AmericaTankCrusader".into(), tank_t);
        let mut wf_t = ThingTemplate::new("AmericaWarFactory");
        wf_t.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .set_health(2000.0);
        logic.templates.insert("AmericaWarFactory".into(), wf_t);

        let tank = logic
            .create_object(
                "AmericaTankCrusader",
                Team::USA,
                glam::Vec3::new(0.0, 0.0, 0.0),
            )
            .expect("tank");
        let wf = logic
            .create_object(
                "AmericaWarFactory",
                Team::USA,
                glam::Vec3::new(40.0, 0.0, 0.0),
            )
            .expect("wf");
        if let Some(o) = logic.get_object_mut(tank) {
            o.health.current = 100.0; // damaged residual
        }
        if let Some(o) = logic.get_object_mut(wf) {
            o.building_data = Some(BuildingData::new(BuildingType::WarFactory));
        }

        let ctx = MouseCommandContext {
            world_position: glam::Vec3::new(40.0, 0.0, 0.0),
            target_object: Some(wf),
            screen_position: glam::Vec2::ZERO,
            viewport_size: None,
            world_min: None,
            world_max: None,
            mouse_button: MouseButton::Right,
            modifier_keys: ModifierKeys::default(),
            is_drag: false,
            drag_start: None,
            drag_end: None,
            drag_start_world: None,
            drag_end_world: None,
        };
        let mut sys = CommandSystem::new();
        let cmd = sys
            .process_mouse_input(&ctx, &[tank], 0, &logic)
            .expect("context command");
        match cmd.command_type {
            CommandType::GetRepaired { target_id } => assert_eq!(target_id, wf),
            other => panic!("expected GetRepaired, got {other:?}"),
        }
    }

    #[test]
    fn command_type_from_button_name_view_and_formation_residual() {
        use crate::command_system::{command_type_from_button_name, CommandType};
        assert!(matches!(
            command_type_from_button_name("Command_CreateFormation"),
            Some(CommandType::CreateFormation)
        ));
        assert!(matches!(
            command_type_from_button_name("Command_ViewCommandCenter"),
            Some(CommandType::ViewCommandCenter)
        ));
        assert!(matches!(
            command_type_from_button_name("Command_ViewLastRadarEvent"),
            Some(CommandType::ViewLastRadarEvent)
        ));
        assert!(matches!(
            command_type_from_button_name("Command_PlaceBeacon"),
            Some(CommandType::PlaceBeacon { .. })
        ));
        assert!(matches!(
            command_type_from_button_name("Command_RemoveBeacon"),
            Some(CommandType::RemoveBeacon)
        ));
        assert!(matches!(
            command_type_from_button_name("Command_Cheer"),
            Some(CommandType::Cheer)
        ));
        assert!(matches!(
            command_type_from_button_name("Command_Deploy"),
            Some(CommandType::Deploy)
        ));
    }

    fn special_power_button_maps_and_structure_resolves_puc_residual() {
        use crate::command_system::{command_type_from_button_name, CommandType, SpecialPowerType};
        use crate::game_logic::host_superweapon_kindof::special_power_for_superweapon_structure;
        assert!(matches!(
            command_type_from_button_name("Command_SpecialPower"),
            Some(CommandType::DoSpecialPower { .. })
        ));
        assert_eq!(
            special_power_for_superweapon_structure("AmericaParticleCannonUplink"),
            Some(SpecialPowerType::ParticleCannon)
        );
        assert_eq!(
            special_power_for_superweapon_structure("GLAScudStorm"),
            Some(SpecialPowerType::ScudStorm)
        );
        assert_eq!(
            special_power_for_superweapon_structure("ChinaNuclearMissile"),
            Some(SpecialPowerType::NuclearMissile)
        );
    }

    fn command_type_from_button_name_upgrade_and_cancel_residual() {
        let q = command_type_from_button_name("Command_UpgradeAmericaRangerFlashBangGrenade")
            .expect("upgrade");
        match q {
            CommandType::QueueUpgrade { upgrade_name } => {
                assert_eq!(upgrade_name, "Upgrade_AmericaRangerFlashBangGrenade");
            }
            other => panic!("expected QueueUpgrade, got {other:?}"),
        }
        let c = command_type_from_button_name("Command_CancelUpgrade").expect("cancel");
        assert!(matches!(
            c,
            CommandType::CancelUpgrade { upgrade_name } if upgrade_name.is_empty()
        ));
        assert!(matches!(
            command_type_from_button_name("Command_Stop"),
            Some(CommandType::Stop)
        ));
        assert!(matches!(
            command_type_from_button_name("Command_AttackMove"),
            Some(CommandType::AttackMoveTo { .. })
        ));
        assert!(matches!(
            command_type_from_button_name("Command_SetRallyPoint"),
            Some(CommandType::SetRallyPoint { .. })
        ));
        assert!(matches!(
            command_type_from_button_name("Command_Evacuate"),
            Some(CommandType::Evacuate)
        ));
        assert!(matches!(
            command_type_from_button_name("Command_Sell"),
            Some(CommandType::Sell { .. })
        ));
        assert!(matches!(
            command_type_from_button_name("Command_SpecialPower"),
            Some(CommandType::DoSpecialPower { .. })
        ));
    }

    #[test]
    fn queue_upgrade_refuses_when_production_queue_full_residual() {
        use crate::game_logic::buildings::{
            BuildingData, BuildingType, ProductionItem, ProductionKind,
            DEFAULT_PRODUCTION_QUEUE_LIMIT,
        };
        use crate::game_logic::host_upgrades::UPGRADE_AMERICA_FLASHBANG;
        use crate::game_logic::{KindOf, Player, Resources, Team, ThingTemplate};

        let mut logic = crate::game_logic::GameLogic::new();
        let mut player = Player::new(0, Team::USA, "USA", true);
        player.resources.supplies = 50_000;
        logic.add_player(player);
        let mut bar = ThingTemplate::new("TestBarracks");
        bar.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::FSBarracks)
            .set_health(1000.0);
        logic.templates.insert("TestBarracks".into(), bar);
        let bid = logic
            .create_object("TestBarracks", Team::USA, glam::Vec3::ZERO)
            .expect("barracks");
        if let Some(o) = logic.get_object_mut(bid) {
            let mut bd = BuildingData::new(BuildingType::Barracks);
            for i in 0..DEFAULT_PRODUCTION_QUEUE_LIMIT {
                bd.production_queue.push(ProductionItem {
                    template_name: format!("Filler{i}"),
                    progress: 0.0,
                    total_time: 10.0,
                    cost: Resources {
                        supplies: 0,
                        power: 0,
                    },
                    quantity_total: 1,
                    quantity_produced: 0,
                    kind: ProductionKind::Unit,
                });
            }
            o.building_data = Some(bd);
        }
        let money_before = logic
            .get_player(0)
            .map(|p| p.effective_supplies())
            .unwrap_or(0);
        logic.queue_command(GameCommand {
            command_type: CommandType::QueueUpgrade {
                upgrade_name: UPGRADE_AMERICA_FLASHBANG.to_string(),
            },
            player_id: 0,
            command_id: 1,
            timestamp: std::time::SystemTime::now(),
            selected_units: vec![bid],
            modifier_keys: ModifierKeys::default(),
        });
        logic.process_commands();
        let money_after = logic
            .get_player(0)
            .map(|p| p.effective_supplies())
            .unwrap_or(0);
        assert_eq!(
            money_before, money_after,
            "queue-full upgrade must not charge residual"
        );
        assert!(
            !logic
                .get_player(0)
                .map(|p| p.has_queued_upgrade(UPGRADE_AMERICA_FLASHBANG))
                .unwrap_or(true),
            "must not queue upgrade when production queue full"
        );
    }

    #[test]
    fn cancel_upgrade_empty_name_cancels_production_head_residual() {
        use crate::command_system::{CommandType, GameCommand};
        use crate::game_logic::host_upgrades::UPGRADE_AMERICA_FLASHBANG;
        use crate::game_logic::{
            buildings::{BuildingData, BuildingType},
            KindOf, Player, Team, ThingTemplate,
        };

        let mut logic = crate::game_logic::GameLogic::new();
        let mut player = Player::new(0, Team::USA, "USA", true);
        player.resources.supplies = 5000;
        logic.add_player(player);
        let mut bar = ThingTemplate::new("TestBarracks");
        bar.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::FSBarracks)
            .set_health(1000.0);
        logic.templates.insert("TestBarracks".into(), bar);
        let bid = logic
            .create_object("TestBarracks", Team::USA, glam::Vec3::ZERO)
            .expect("barracks");
        if let Some(o) = logic.get_object_mut(bid) {
            o.building_data = Some(BuildingData::new(BuildingType::Barracks));
        }

        // Queue via command path so player + building both hold residual.
        logic.queue_command(GameCommand {
            command_type: CommandType::QueueUpgrade {
                upgrade_name: UPGRADE_AMERICA_FLASHBANG.to_string(),
            },
            player_id: 0,
            command_id: 1,
            timestamp: std::time::SystemTime::now(),
            selected_units: vec![bid],
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
        logic.process_commands();
        assert!(logic
            .get_player(0)
            .map(|p| p.has_queued_upgrade(UPGRADE_AMERICA_FLASHBANG))
            .unwrap_or(false));
        let money_after_queue = logic
            .get_player(0)
            .map(|p| p.effective_supplies())
            .unwrap_or(0);

        // Empty name CancelUpgrade → head residual.
        logic.queue_command(GameCommand {
            command_type: CommandType::CancelUpgrade {
                upgrade_name: String::new(),
            },
            player_id: 0,
            command_id: 2,
            timestamp: std::time::SystemTime::now(),
            selected_units: vec![bid],
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
        logic.process_commands();

        assert!(
            !logic
                .get_player(0)
                .map(|p| p.has_queued_upgrade(UPGRADE_AMERICA_FLASHBANG))
                .unwrap_or(true),
            "queued upgrade cleared"
        );
        let q_empty = logic
            .get_object(bid)
            .and_then(|o| o.building_data.as_ref())
            .map(|b| b.production_queue.is_empty())
            .unwrap_or(false);
        assert!(q_empty, "building PRODUCTION_UPGRADE head removed");
        let money_after = logic
            .get_player(0)
            .map(|p| p.effective_supplies())
            .unwrap_or(0);
        assert!(
            money_after > money_after_queue,
            "cancel refunds residual cost: before={money_after_queue} after={money_after}"
        );
    }

    #[test]
    fn cancel_upgrade_refunds_only_when_upgrade_is_queued() {
        use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

        let system = CommandSystem::new();
        let mut game_logic = GameLogic::new();
        let mut player = Player::new(0, Team::USA, "USA", true);
        player.resources.supplies = 3000;
        game_logic.add_player(player);

        let mut template = ThingTemplate::new("AmericaSupplyCenter");
        template
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        let producer = Object::new(template, ObjectId(301), Team::USA);
        game_logic.add_object(producer);

        let queue_command = GameCommand {
            command_type: CommandType::QueueUpgrade {
                upgrade_name: "Upgrade_AmericaSupplyLines".to_string(),
            },
            player_id: 0,
            command_id: 10,
            timestamp: SystemTime::now(),
            selected_units: vec![ObjectId(301)],
            modifier_keys: ModifierKeys::default(),
        };
        assert_eq!(
            system.execute_command(&queue_command, &mut game_logic),
            CommandResult::Success
        );

        let cancel_command = GameCommand {
            command_type: CommandType::CancelUpgrade {
                upgrade_name: "Upgrade_AmericaSupplyLines".to_string(),
            },
            player_id: 0,
            command_id: 11,
            timestamp: SystemTime::now(),
            selected_units: vec![ObjectId(301)],
            modifier_keys: ModifierKeys::default(),
        };
        assert_eq!(
            system.execute_command(&cancel_command, &mut game_logic),
            CommandResult::Success
        );

        let player_after_cancel = game_logic.get_player(0).expect("player should exist");
        assert_eq!(
            player_after_cancel.effective_supplies(),
            3000,
            "cancel should refund the queued upgrade cost"
        );
        assert!(!player_after_cancel
            .queued_upgrades
            .contains("Upgrade_AmericaSupplyLines"));

        assert_eq!(
            system.execute_command(&cancel_command, &mut game_logic),
            CommandResult::InvalidCommand,
            "cancelling a non-queued upgrade should not issue another refund"
        );
    }

    #[test]
    fn queue_upgrade_requires_constructed_building_source() {
        use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

        let system = CommandSystem::new();
        let mut game_logic = GameLogic::new();
        let mut player = Player::new(0, Team::USA, "USA", true);
        player.resources.supplies = 3000;
        game_logic.add_player(player);

        let mut unit_template = ThingTemplate::new("TestUnit");
        unit_template
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        game_logic.add_object(Object::new(unit_template, ObjectId(351), Team::USA));

        let command = GameCommand {
            command_type: CommandType::QueueUpgrade {
                upgrade_name: "Upgrade_AmericaSupplyLines".to_string(),
            },
            player_id: 0,
            command_id: 12,
            timestamp: SystemTime::now(),
            selected_units: vec![ObjectId(351)],
            modifier_keys: ModifierKeys::default(),
        };

        assert_eq!(
            system.execute_command(&command, &mut game_logic),
            CommandResult::InvalidCommand
        );
        let player_after = game_logic.get_player(0).expect("player should exist");
        assert_eq!(
            player_after.effective_supplies(),
            3000,
            "non-producing units must not charge upgrade resources"
        );
        assert!(player_after.queued_upgrades.is_empty());
    }

    #[test]
    fn queued_upgrade_completes_during_simulation_update() {
        use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

        let system = CommandSystem::new();
        let mut game_logic = GameLogic::new();
        let mut player = Player::new(0, Team::USA, "USA", true);
        player.resources.supplies = 3000;
        game_logic.add_player(player);

        let mut template = ThingTemplate::new("AmericaSupplyCenter");
        template
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        let producer = Object::new(template, ObjectId(401), Team::USA);
        game_logic.add_object(producer);

        let command = GameCommand {
            command_type: CommandType::QueueUpgrade {
                upgrade_name: "Upgrade_AmericaSupplyLines".to_string(),
            },
            player_id: 0,
            command_id: 20,
            timestamp: SystemTime::now(),
            selected_units: vec![ObjectId(401)],
            modifier_keys: ModifierKeys::default(),
        };
        assert_eq!(
            system.execute_command(&command, &mut game_logic),
            CommandResult::Success
        );

        let player_after_queue = game_logic.get_player(0).expect("player should exist");
        assert!(player_after_queue
            .queued_upgrades
            .contains("Upgrade_AmericaSupplyLines"));
        assert!(!player_after_queue
            .unlocked_sciences
            .contains("Upgrade_AmericaSupplyLines"));

        game_logic.update();

        let player_after_update = game_logic
            .get_player(0)
            .expect("player should exist after update");
        assert!(!player_after_update
            .queued_upgrades
            .contains("Upgrade_AmericaSupplyLines"));
        assert!(player_after_update
            .unlocked_sciences
            .contains("Upgrade_AmericaSupplyLines"));
        assert_eq!(
            system.execute_command(&command, &mut game_logic),
            CommandResult::InvalidCommand,
            "completed upgrades should not be queued or charged again"
        );
    }

    #[test]
    fn command_system_residual_locomotion_pathfinds() {
        let src = include_str!("command_system.rs");
        let move_i = src.find("fn execute_move_command").expect("move");
        let w = &src[move_i..move_i + 800];
        assert!(
            w.contains("CommandExecutor") || w.contains("assign_unit_path"),
            "residual move must pathfind via executor or assign_unit_path"
        );
        let am_i = src.find("fn execute_attack_move_command").expect("am");
        let w = &src[am_i..am_i + 800];
        assert!(
            w.contains("CommandExecutor") || w.contains("assign_unit_path"),
            "residual attack-move must pathfind"
        );
        let sc_i = src.find("fn execute_scatter_command").expect("sc");
        let w = &src[sc_i..sc_i + 1600];
        assert!(
            w.contains("assign_unit_path"),
            "residual scatter must assign_unit_path"
        );
    }

    #[test]
    fn resume_construction_context_residual() {
        let src = include_str!("command_system.rs");
        assert!(
            src.contains("fn can_resume_construction")
                && src.contains("CommandType::ResumeConstruction"),
            "context path must offer ResumeConstruction for unfinished structures"
        );
        let start = src
            .find("fn determine_context_command")
            .expect("determine_context_command");
        let body = &src[start..start + 2200];
        assert!(
            body.contains("can_resume_construction"),
            "determine_context_command must call can_resume_construction"
        );
    }

    #[test]
    fn capture_building_context_residual() {
        let src = include_str!("command_system.rs");
        assert!(
            src.contains("fn can_capture_building") && src.contains("CommandType::CaptureBuilding"),
            "context path must offer CaptureBuilding residual"
        );
        let start = src
            .find("fn determine_context_command")
            .expect("determine_context_command");
        let body = &src[start..start + 2800];
        assert!(
            body.contains("can_capture_building"),
            "determine_context_command must call can_capture_building"
        );
    }

    #[test]
    fn unit_ability_button_name_map_residual() {
        use crate::command_system::{command_type_from_button_name, CommandType};
        assert!(matches!(
            command_type_from_button_name("Command_Hijack"),
            Some(CommandType::Hijack { .. })
        ));
        assert!(matches!(
            command_type_from_button_name("Command_SnipeVehicle"),
            Some(CommandType::SnipeVehicle { .. })
        ));
        assert!(matches!(
            command_type_from_button_name("Command_CaptureBuilding"),
            Some(CommandType::CaptureBuilding { .. })
        ));
        assert!(matches!(
            command_type_from_button_name("Command_PlantTimedDemoCharge"),
            Some(CommandType::PlantTimedDemoCharge { .. })
        ));
        assert!(matches!(
            command_type_from_button_name("Command_BlackLotusStealCash")
                .or_else(|| command_type_from_button_name("Command_StealCashHack")),
            Some(CommandType::StealCashHack { .. })
        ));
    }
}
