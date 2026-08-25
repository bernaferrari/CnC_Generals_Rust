//! Wave 958: host_object dual-read seal (tests + residual).
//!
//! Host [`Object`] type, split across focused impl modules. Public paths stay
//! `crate::game_logic::object::Object` (and the small enums/helpers that lived
//! in the original `object.rs`).

// Restricted re-exports so impl submodules can `use super::*;` without
// dumping game_logic's public surface back through `pub use object::*;`.
pub(in crate::game_logic::object) use super::*;
pub(in crate::game_logic::object) use crate::command_system::SpecialPowerType;
pub(in crate::game_logic::object) use glam::{Mat4, Vec3};
pub(in crate::game_logic::object) use serde::{Deserialize, Serialize};
pub(in crate::game_logic::object) use std::collections::{HashMap, HashSet};

/// C++ TurretAI state residual.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TurretSubState {
    #[default]
    Idle,
    IdleScan,
    Aim,
    Fire,
    Hold,
    Recenter,
}

impl TurretSubState {
    #[inline]
    pub fn ordinal(self) -> u8 {
        match self {
            TurretSubState::Idle => 0,
            TurretSubState::IdleScan => 1,
            TurretSubState::Aim => 2,
            TurretSubState::Fire => 3,
            TurretSubState::Hold => 4,
            TurretSubState::Recenter => 5,
        }
    }

    #[inline]
    pub fn from_ordinal(v: u8) -> Self {
        match v {
            1 => TurretSubState::IdleScan,
            2 => TurretSubState::Aim,
            3 => TurretSubState::Fire,
            4 => TurretSubState::Hold,
            5 => TurretSubState::Recenter,
            _ => TurretSubState::Idle,
        }
    }
}

/// C++ AttackStateMachine substate residual.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AttackSubState {
    /// C++ AIM_AT_TARGET (default on enter).
    #[default]
    AimAtTarget,
    /// C++ FIRE_WEAPON.
    FireWeapon,
    /// C++ APPROACH_TARGET.
    ApproachTarget,
    /// C++ CHASE_TARGET (pursue residual collapses to approach when not fleeing).
    ChaseTarget,
}

/// C++ `AIUpdateInterface::LocoGoalType` (AIUpdate.h). Saved numeric values
/// must not change.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LocoGoalType {
    #[default]
    None = 0,
    PositionOnPath = 1,
    PositionExplicit = 2,
    Angle = 3,
}

/// C++ `AIFaceState::update` relative-angle success (`0.035` rad ≈ 2°).
pub const FACE_REL_THRESH_RAD: f32 = 0.035;

/// The durable marker for one actually accepted WeaponSet discharge.
///
/// This is deliberately separate from `last_fire_*` / `fire_intent_count`:
/// those fields also represent GameWorld AI presentation writeback, whereas a
/// W3D recoil/muzzle cue must name the concrete slot and its barrel *before*
/// that Weapon advances. A zero sequence is the only unseen sentinel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WeaponDischargeMarker {
    pub sequence: u64,
    pub weapon_slot: u8,
    pub fired_barrel: u8,
    pub logic_frame: u32,
}

impl WeaponDischargeMarker {
    #[inline]
    pub const fn unseen() -> Self {
        Self {
            sequence: 0,
            weapon_slot: 0,
            fired_barrel: 0,
            logic_frame: 0,
        }
    }

    #[inline]
    pub const fn is_seen(self) -> bool {
        self.sequence != 0
    }
}

impl AttackSubState {
    pub fn to_ordinal(self) -> u8 {
        match self {
            AttackSubState::AimAtTarget => 0,
            AttackSubState::FireWeapon => 1,
            AttackSubState::ApproachTarget => 2,
            AttackSubState::ChaseTarget => 3,
        }
    }

    pub fn from_ordinal(v: u8) -> Self {
        match v {
            1 => AttackSubState::FireWeapon,
            2 => AttackSubState::ApproachTarget,
            3 => AttackSubState::ChaseTarget,
            _ => AttackSubState::AimAtTarget,
        }
    }
}

fn default_one_f32() -> f32 {
    1.0
}

fn default_terrain_decal_none() -> u8 {
    8
}

/// C++ `LAYER_GROUND` (`PathfindLayerEnum::Ground = 1`).
fn default_pathfind_layer_ground() -> u8 {
    1
}

/// C++ `Drawable::xfer` overlay icon slot (name + keepTillFrame + Anim2D).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct DrawableOverlayIcon {
    pub name: String,
    pub keep_till_frame: u32,
    #[serde(default)]
    pub template_name: String,
    #[serde(default)]
    pub anim_frame: u32,
}

fn default_pitch_roll_yaw_factor() -> f32 {
    2.0
}

/// C++ DEFAULT_TURN_RATE residual (radians/frame).
pub(crate) fn default_turret_turn_rate() -> f32 {
    0.01
}

mod turret_spawn;
pub(crate) use turret_spawn::{
    TurretSpawnSpec, turret_deg_per_sec_to_rad_per_frame, turret_ms_to_frames,
    turret_spawn_for_template,
};

/// C++ default recenter wait residual (2 * LOGICFRAMES_PER_SECOND).
pub(crate) fn default_turret_recenter_frames() -> u32 {
    60
}

pub(crate) fn default_mood_attack_check_rate() -> u32 {
    // C++ typical mood check rate residual (~1s @ 30fps).
    30
}

pub(crate) fn default_vision_range() -> f32 {
    // C++ Object.cpp:270 copies ThingTemplate::m_visionRange (default 0).
    0.0
}

fn default_true_for_auto_acquire() -> bool {
    true
}

fn default_max_shots() -> i32 {
    -1
}

fn default_experience_scalar() -> f32 {
    1.0
}

fn default_braking() -> f32 {
    // C++ LocomotorTemplate::m_braking = BIGNUM (Locomotor.cpp:270).
    99999.0
}

fn default_donut_timer() -> u32 {
    // C++ ctor seeds now+2.5s; MAX means start_move / first wheels apply stamps it.
    u32::MAX
}

fn default_airborne_targeting_height() -> i32 {
    // C++ LocomotorTemplate::m_airborneTargetingHeight = INT_MAX (Locomotor.cpp:314).
    i32::MAX
}

pub(crate) fn actual_speed_is_zero(o: &Object) -> bool {
    o.movement.velocity.x.abs() < 1e-4 && o.movement.velocity.z.abs() < 1e-4
}

/// C++ calcSlowDownDist residual (host units).
/// C++ AIStates isSamePosition residual (2D, dist/10 tolerance).
pub fn is_same_position_residual(
    our_pos: glam::Vec3,
    prev_target: glam::Vec3,
    cur_target: glam::Vec3,
) -> bool {
    let dx = cur_target.x - prev_target.x;
    let dz = cur_target.z - prev_target.z;
    let to_x = cur_target.x - our_pos.x;
    let to_z = cur_target.z - our_pos.z;
    const TOLERANCE_FACTOR: f32 = 1.0 / 100.0;
    let tolerance_sqr = (to_x * to_x + to_z * to_z) * TOLERANCE_FACTOR;
    dx * dx + dz * dz <= tolerance_sqr
}

pub fn calc_slow_down_dist(cur_speed: f32, desired_speed: f32, max_braking: f32) -> f32 {
    let delta = cur_speed - desired_speed;
    if delta <= 0.0 {
        return 0.0;
    }
    let braking = max_braking.abs().max(1e-6);
    let dist = (delta * delta / braking) * 0.5;
    const FUDGE: f32 = 1.05;
    dist * FUDGE
}

/// Object type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectType {
    Infantry,
    Vehicle,
    Aircraft,
    Building,
    Supply,
    Projectile,
    Neutral,
}

/// C++ PhysicsTurningType residual.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(i8)]
pub enum PhysicsTurningType {
    TurnNegative = -1,
    #[default]
    TurnNone = 0,
    TurnPositive = 1,
}

impl PhysicsTurningType {
    pub fn to_ordinal(self) -> i8 {
        match self {
            PhysicsTurningType::TurnNegative => -1,
            PhysicsTurningType::TurnNone => 0,
            PhysicsTurningType::TurnPositive => 1,
        }
    }
    pub fn from_ordinal(v: i8) -> Self {
        match v {
            -1 => PhysicsTurningType::TurnNegative,
            1 => PhysicsTurningType::TurnPositive,
            _ => PhysicsTurningType::TurnNone,
        }
    }
}

/// C++ `LocomotorBehaviorZ` (`Locomotor.h:68-78`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum LocomotorBehaviorZ {
    #[default]
    NoZMotiveForce = 0,
    SeaLevel = 1,
    SurfaceRelativeHeight = 2,
    AbsoluteHeight = 3,
    FixedSurfaceRelativeHeight = 4,
    FixedAbsoluteHeight = 5,
    RelativeToGroundAndBuildings = 6,
    SmoothRelativeToHighestLayer = 7,
}

impl LocomotorBehaviorZ {
    pub fn to_ordinal(self) -> u8 {
        self as u8
    }
    pub fn from_ordinal(v: u8) -> Self {
        match v {
            1 => LocomotorBehaviorZ::SeaLevel,
            2 => LocomotorBehaviorZ::SurfaceRelativeHeight,
            3 => LocomotorBehaviorZ::AbsoluteHeight,
            4 => LocomotorBehaviorZ::FixedSurfaceRelativeHeight,
            5 => LocomotorBehaviorZ::FixedAbsoluteHeight,
            6 => LocomotorBehaviorZ::RelativeToGroundAndBuildings,
            7 => LocomotorBehaviorZ::SmoothRelativeToHighestLayer,
            _ => LocomotorBehaviorZ::NoZMotiveForce,
        }
    }
}

/// C++ LocomotorAppearance residual (subset used by host update_movement).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum LocomotorAppearance {
    #[default]
    Other = 0,
    LegsTwo = 1,
    WheelsFour = 2,
    Treads = 3,
    Hover = 4,
    Wings = 5,
    Thrust = 6,
    Motorcycle = 7,
    Climber = 8,
}

impl LocomotorAppearance {
    pub fn to_ordinal(self) -> u8 {
        match self {
            LocomotorAppearance::Other => 0,
            LocomotorAppearance::LegsTwo => 1,
            LocomotorAppearance::WheelsFour => 2,
            LocomotorAppearance::Treads => 3,
            LocomotorAppearance::Hover => 4,
            LocomotorAppearance::Wings => 5,
            LocomotorAppearance::Thrust => 6,
            LocomotorAppearance::Motorcycle => 7,
            LocomotorAppearance::Climber => 8,
        }
    }
    pub fn from_ordinal(v: u8) -> Self {
        match v {
            1 => LocomotorAppearance::LegsTwo,
            2 => LocomotorAppearance::WheelsFour,
            3 => LocomotorAppearance::Treads,
            4 => LocomotorAppearance::Hover,
            5 => LocomotorAppearance::Wings,
            6 => LocomotorAppearance::Thrust,
            7 => LocomotorAppearance::Motorcycle,
            8 => LocomotorAppearance::Climber,
            _ => LocomotorAppearance::Other,
        }
    }
}

/// Game Object - the main entity class for all game units, buildings, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Object {
    /// Base Thing functionality
    pub thing: Thing,

    /// Unique identifier
    pub id: ObjectId,

    /// Team ownership
    pub team: Team,

    /// Controlling player.  `team` is faction/template identity and is not a
    /// unique owner in same-faction skirmishes.
    #[serde(default)]
    pub owner_player_id: Option<u32>,
    /// C++ Team instance name (`Team::getName`) for script team overrides.
    #[serde(default)]
    pub team_instance_name: String,

    /// Object name
    pub name: String,

    /// Object status
    pub status: ObjectStatus,
    /// C++ ObjectStatusMaskType residual bits (StatusBitsUpgrade set/clear).
    #[serde(default)]
    pub object_status_bits: u64,
    /// C++ `OBJECT_STATUS_SCRIPT_UNSELLABLE` / `Object::isScriptUnsellable`.
    #[serde(default)]
    pub script_unsellable: bool,
    /// C++ `Object::m_singleUseCommandUsed` — whole command strip Restricted.
    #[serde(default)]
    pub single_use_command_used: bool,
    /// C++ `OBJECT_STATUS_SCRIPT_UNSTEALTHED` — `NAMED/TEAM_SET_STEALTH_ENABLED`.
    #[serde(default)]
    pub script_unstealthed: bool,
    /// C++ `OBJECT_STATUS_SCRIPT_TARGETABLE` / map `objectTargetable` /
    /// leftover_sa `Player Targetable`.
    #[serde(default)]
    pub script_targetable: bool,


    /// C++ ActiveBody::m_indestructible (map objectIndestructible / UNIT INDESTRUCTIBLE).
    #[serde(default)]
    pub indestructible: bool,

    /// Wave 754: EjectPilotDie onDie already fired (death-start residual).
    #[serde(default)]
    pub eject_pilot_die_applied: bool,
    /// C++ ModelConditionFlags residual bits (ALLOW_SURRENDER-off index layout).
    #[serde(default)]
    pub model_condition_bits: u128,
    /// C++ `TheKey_objectWeather` (`Object.cpp:3595-3605`): 0 follow map,
    /// 1 force clear `MODELCONDITION_SNOW`, 2 force set.
    #[serde(default)]
    pub object_weather: i32,
    /// C++ `Object::m_safeOcclusionFrame`. RTS3DScene only adds a score-unit
    /// to potential occludees when this frame is in the past (`W3DScene.cpp:474`).
    #[serde(default)]
    pub safe_occlusion_frame: u32,


    /// C++ RadarUpdate m_extendDoneFrame residual (0 = inactive).
    pub radar_extend_done_frame: u32,
    /// C++ RadarUpdate m_extendComplete residual.
    pub radar_extend_complete: bool,
    /// C++ RadarUpdate m_radarActive residual.
    pub radar_active: bool,
    /// C++ ProductionUpdate door residual phase for DOOR_1: 0=idle 1=opening 2=wait 4=closing.
    pub production_door_phase: u8,
    /// Frame when current door residual phase ends.
    pub production_door_phase_end_frame: u32,
    /// C++ ProductionUpdate DoorInfo::m_holdOpen for DOOR_1 (legacy / save alias).
    pub production_door_hold_open: bool,
    /// C++ DoorInfo m_doors[DOOR_COUNT_MAX].m_holdOpen — one bit per hangar stall.
    #[serde(default)]
    pub production_door_hold_opens: [bool; 4],
    /// C++ DoorInfo m_doors[DOOR_COUNT_MAX] phases (0 idle, 1 opening, 2 wait, 4 closing).
    #[serde(default)]
    pub production_door_phases: [u8; 4],
    /// Per-door phase end frames matching `production_door_phases`.
    #[serde(default)]
    pub production_door_phase_end_frames: [u32; 4],
    /// Reserved ExitDoorType index (0=DOOR_1 .. 3=DOOR_4) for the current cycle.
    #[serde(default)]
    pub production_door_active_index: u8,
    /// C++ RebuildHoleBehavior residual: this object is a rebuild hole.
    pub is_rebuild_hole: bool,
    /// Template name to reconstruct (C++ m_rebuildTemplate).
    pub rebuild_template_name: Option<String>,
    /// Absolute frame when hole may spawn reconstruction (worker delay residual).
    pub rebuild_ready_frame: u32,
    /// Original structure that spawned this hole.
    pub rebuild_spawner_id: Option<ObjectId>,
    /// C++ RebuildHoleBehavior m_workerID residual.
    pub rebuild_worker_id: Option<ObjectId>,
    /// C++ RebuildHoleBehavior m_reconstructingID residual.
    pub rebuild_reconstructing_id: Option<ObjectId>,
    /// C++ Object::m_producerID residual (hole is producer of reconstructing building).
    pub producer_id: Option<ObjectId>,
    /// C++ Object::m_builderID residual — exclusive dozer/worker building this
    /// structure. DozerAIUpdate refuses a second builder when this is set
    /// (DozerAIUpdate.cpp:305).
    #[serde(default)]
    pub builder_id: Option<ObjectId>,
    /// C++ `DozerAIUpdate::m_task[DOZER_TASK_BUILD]` — kept while REPAIR runs
    /// so idle `isBuildMostImportant` can resume the scaffold (DozerAIUpdate.cpp:1948).
    #[serde(default)]
    pub dozer_task_build_target: Option<ObjectId>,
    /// C++ `m_task[DOZER_TASK_BUILD].m_taskOrderFrame`.
    #[serde(default)]
    pub dozer_task_build_order_frame: u32,
    /// C++ `DozerAIUpdate::m_task[DOZER_TASK_REPAIR]`.
    #[serde(default)]
    pub dozer_task_repair_target: Option<ObjectId>,
    /// C++ `m_task[DOZER_TASK_REPAIR].m_taskOrderFrame`.
    #[serde(default)]
    pub dozer_task_repair_order_frame: u32,
    /// C++ `m_dockPoint[DOZER_TASK_BUILD][DOZER_DOCK_POINT_ACTION]` (DozerAIUpdate.cpp:1990-1991).
    /// Seeded by `findGoodBuildOrRepairPosition` (half major radius). Build HP
    /// starts only when idle at this point, not the structure centre.
    #[serde(default)]
    pub dozer_dock_action: Option<Vec3>,



    /// C++ SupplyTruckAIUpdate/WorkerAIUpdate::m_preferredDock.
    ///
    /// AIPlayer deliberately issues its collector dock command as
    /// `CMD_FROM_PLAYER`, which makes the chosen SupplyCenter persist across
    /// gather/return cycles instead of letting ResourceManager choose a
    /// different nearby center.
    #[serde(default)]
    pub preferred_dock_id: Option<ObjectId>,
    /// C++ one-shot SpawnBehavior state for SupplyCenter/Stash starter collectors.
    ///
    /// The spawned unit can die, but `OneShot = Yes` must not become eligible
    /// again merely because no live child remains.
    #[serde(default)]
    pub supply_center_spawn_behavior_fired: bool,
    /// C++ `SupplyTruckAIUpdate::m_stateMachine` compact state.
    #[serde(default)]
    pub supply_truck_state: SupplyTruckState,
    /// C++ `m_forceWantingState`; consumed exactly once after the authored
    /// SupplyCenter exit completes.
    #[serde(default)]
    pub supply_truck_force_pending: bool,
    /// Earliest frame at which the current warehouse/center dock may act.
    #[serde(default)]
    pub supply_truck_next_dock_action_frame: u32,
    /// C++ `DockUpdate::m_activeDocker` — only this docker may enter/act.
    #[serde(default)]
    pub dock_active_docker: Option<ObjectId>,
    /// C++ `RailedTransportAIUpdate::m_inTransit`.
    #[serde(default)]
    pub railed_in_transit: bool,
    /// C++ `RailedTransportAIUpdate::m_waypointDataLoaded`.
    #[serde(default)]
    pub railed_waypoint_data_loaded: bool,
    /// C++ `RailedTransportAIUpdate::m_currentPath` (`INVALID_PATH` = -1).
    #[serde(default = "crate::game_logic::default_railed_current_path")]
    pub railed_current_path: i32,
    /// C++ `m_path[0..m_numPaths]` Start/End waypoint IDs.
    #[serde(default)]
    pub railed_paths: Vec<(u32, u32)>,

    /// C++ `Drawable::updateDrawableSupplyStatus` current boxes.
    #[serde(default)]
    pub drawable_supply_boxes: u32,
    /// C++ startingBoxes argument to `updateDrawableSupplyStatus`.
    #[serde(default)]
    pub drawable_supply_max_boxes: u32,
    /// C++ `RepairDockUpdate::m_lastRepair`.
    #[serde(default)]
    pub repair_dock_last_id: Option<ObjectId>,
    /// C++ `RepairDockUpdate::m_healthToAddPerFrame` stored as HP/sec.
    #[serde(default)]
    pub repair_dock_health_per_sec: f32,
    /// Absolute frame when a GrantTemporaryStealth residual expires (0 = none).
    #[serde(default)]
    pub temporary_stealth_expires_frame: u32,

    /// C++ HighlanderBody residual (cannot die from normal damage).
    pub highlander_body: bool,
    /// C++ UpgradeDie residual (free producer upgrade on death).
    pub upgrade_die: Option<crate::game_logic::host_upgrade_die::HostUpgradeDieData>,
    /// C++ ProductionUpdate m_constructionCompleteFrame residual.
    /// Absolute frame when CONSTRUCTION_COMPLETE bit should clear (0 = inactive).
    pub construction_complete_clear_frame: u32,
    /// C++ Object::m_soleHealingBenefactorID residual.
    pub sole_healing_benefactor: Option<ObjectId>,
    /// C++ Object::m_soleHealingBenefactorExpirationFrame residual.
    pub sole_healing_benefactor_expiration_frame: u32,
    /// C++ DozerPrimaryIdleState m_idleTooLongTimestamp residual.
    pub idle_since_frame: u32,
    /// C++ PhysicsBehavior IS_STUNNED residual frames remaining (0 = clear).
    #[serde(default)]
    pub shock_stun_frames: u32,
    /// C++ PhysicsBehavior m_yawRate residual from shock random rotation.
    #[serde(default)]
    pub shock_yaw_rate: f32,
    /// C++ PhysicsBehavior m_pitchRate residual from shock random rotation.
    #[serde(default)]
    pub shock_pitch_rate: f32,
    /// C++ PhysicsBehavior m_rollRate residual from shock random rotation.
    #[serde(default)]
    pub shock_roll_rate: f32,
    /// C++ PhysicsBehavior ALLOW_BOUNCE residual (enabled by applyRandomRotation).
    #[serde(default)]
    pub shock_allow_bounce: bool,
    /// C++ WAS_AIRBORNE_LAST_FRAME residual during shock freefall.
    #[serde(default)]
    pub shock_was_airborne: bool,
    /// First ground contact while stunned: STUNNED_FLAILING → STUNNED residual.
    #[serde(default)]
    pub shock_grounded_once: bool,
    /// C++ transform Z-up residual (1 upright, <0 inverted / splat candidate).
    #[serde(default = "default_shock_up_z")]
    pub shock_up_z: f32,
    /// C++ LocomotorSurfaceTypeMask residual (default by KindOf).
    #[serde(default)]
    pub locomotor_surfaces: u32,
    /// Host TerrainLogic::isCliffCell residual for stun destruction (set by world).
    #[serde(default)]
    pub cell_is_cliff: bool,
    /// Host TerrainLogic::isUnderwater residual for stun destruction (set by world).
    #[serde(default)]
    pub cell_is_underwater: bool,
    /// C++ PhysicsBehaviorModuleData::m_killWhenRestingOnGround residual.
    #[serde(default)]
    pub kill_when_resting_on_ground: bool,
    /// C++ PhysicsBehaviorModuleData::m_minFallSpeedForDamage residual.
    #[serde(default = "default_min_fall_speed_for_damage")]
    pub min_fall_speed_for_damage: f32,
    /// C++ PhysicsBehaviorModuleData::m_fallHeightDamageFactor residual.
    #[serde(default = "default_fall_height_damage_factor")]
    pub fall_height_damage_factor: f32,
    /// C++ PhysicsBehavior::update ground onCollide(NULL) pending residual.
    #[serde(default)]
    pub pending_ground_collide: bool,
    /// C++ IMMUNE_TO_FALLING_DAMAGE residual (projectiles / special).
    #[serde(default)]
    pub immune_to_falling_damage: bool,
    /// Host residual: bounce-land audio events (doBounceSound count).
    #[serde(default)]
    pub bounce_land_events: u32,
    /// Last bounce vertical displacement residual for volume (prevY - y).
    #[serde(default)]
    pub last_bounce_fall_dy: f32,
    /// C++ PhysicsBehavior bounce AudioEventRTS name residual.
    /// Empty unless OCL/SlowDeath authored BounceSound (doBounceSound no-ops).
    #[serde(default)]
    pub bounce_sound_name: String,
    /// Last computed bounce volume residual [0.25, 1.0] (MuLaw path).
    #[serde(default)]
    pub last_bounce_volume: f32,
    pub bounce_audio_pending: u32,
    /// C++ ThingTemplate CrusherLevel residual.
    #[serde(default)]
    pub crusher_level: u8,
    /// C++ ThingTemplate CrushableLevel residual (default 255 = uncrushable).
    #[serde(default = "default_crushable_level")]
    pub crushable_level: u8,
    /// C++ `findModule("SquishCollide")` residual (Object.cpp:1133).
    #[serde(default)]
    pub has_squish_collide: bool,
    /// C++ BodyModule front crushed residual.
    #[serde(default)]
    pub front_crushed: bool,
    /// C++ BodyModule back crushed residual.
    #[serde(default)]
    pub back_crushed: bool,
    /// C++ PhysicsBehavior m_currentOverlap residual.
    #[serde(default)]
    pub physics_current_overlap: Option<ObjectId>,
    /// C++ PhysicsBehavior m_previousOverlap residual.
    #[serde(default)]
    pub physics_previous_overlap: Option<ObjectId>,
    /// C++ PhysicsBehavior m_ignoreCollisionsWith residual.
    #[serde(default)]
    pub ignore_collisions_with: Option<ObjectId>,
    /// C++ PhysicsBehavior m_lastCollidee residual.
    #[serde(default)]
    pub last_collidee: Option<ObjectId>,
    /// C++ PhysicsBehaviorModuleData m_allowCollideForce residual (default true).
    #[serde(default = "default_true")]
    pub allow_collide_force: bool,
    /// C++ AIUpdate m_canPathThroughUnits residual.
    #[serde(default)]
    pub can_path_through_units: bool,
    /// C++ AIUpdate m_ignoreCollisionsUntil frame residual (0 = inactive).
    #[serde(default)]
    pub ignore_collisions_until_frame: u32,
    /// C++ AIUpdate m_isBlocked residual.
    #[serde(default)]
    pub is_blocked: bool,
    /// C++ AIUpdate m_isBlockedAndStuck residual.
    #[serde(default)]
    pub is_blocked_and_stuck: bool,
    /// C++ AIUpdate m_curMaxBlockedSpeed residual (world units / frame).
    #[serde(default = "default_max_f32")]
    pub cur_max_blocked_speed: f32,
    /// C++ AIUpdate getNumFramesBlocked residual.
    #[serde(default)]
    pub num_frames_blocked: u32,
    /// C++ AIUpdate m_bumpSpeedLimit residual (host dist/sec, FAST_AS_POSSIBLE = MAX).
    #[serde(default = "default_max_f32")]
    pub bump_speed_limit: f32,
    /// C++ LocomotorSet member names for the current SET_* (surface-switched).
    #[serde(default)]
    pub locomotor_set_names: Vec<String>,
    /// C++ AIUpdate m_curLocomotor template name.
    #[serde(default)]
    pub cur_locomotor_name: Option<String>,
    /// C++ AI panic state residual (AI_PANIC → bounce force allowed).
    #[serde(default)]
    pub is_panicking: bool,
    /// C++ PhysicsBehavior m_mass residual.
    #[serde(default = "default_physics_mass")]
    pub physics_mass: f32,
    /// C++ OpenContain::getContainedItemsMass cache (sum of rider getMass).
    #[serde(default)]
    pub contained_items_mass: f32,
    /// C++ PhysicsBehaviorModuleData::m_shockResistance residual.
    #[serde(default)]
    pub shock_resistance: f32,
    /// C++ PhysicsBehavior m_accel residual (integrated each frame).
    #[serde(default)]
    pub physics_accel: glam::Vec3,
    /// C++ isMotive residual frames remaining (0 = not motive / accept full force).
    #[serde(default)]
    pub motive_frames_remaining: u32,
    /// C++ AIUpdate m_waitingForPath residual.
    #[serde(default)]
    pub waiting_for_path: bool,
    /// C++ AIUpdate `m_doFinalPosition` — leftover NONE-goal settle.
    #[serde(default)]
    pub do_final_position: bool,
    /// C++ AIUpdate `m_finalPosition` (host Y-up).
    #[serde(default)]
    pub final_position: glam::Vec3,
    /// C++ AIUpdateInterface::m_ignoreObstacleID residual.
    #[serde(default)]
    pub ignored_obstacle_id: Option<ObjectId>,
    /// C++ m_moveOutOfWay1 residual (object id we're yielding for).
    #[serde(default)]
    pub move_away_from: Option<ObjectId>,
    /// C++ AI_MOVE_OUT_OF_THE_WAY temporary state frames remaining.
    #[serde(default)]
    pub move_away_frames: u32,
    /// Desired yield position residual from aiMoveAwayFromUnit.
    #[serde(default)]
    pub move_away_destination: Option<glam::Vec3>,
    /// When set by processCollision, GameLogic should call ai_move_away on this id.
    #[serde(default)]
    pub request_other_move_away: Option<ObjectId>,
    /// C++ PhysicsBehaviorModuleData m_forwardFriction residual (per frame).
    #[serde(default = "default_forward_friction")]
    pub forward_friction: f32,
    /// C++ m_lateralFriction residual (per frame).
    #[serde(default = "default_lateral_friction")]
    pub lateral_friction: f32,
    /// C++ m_ZFriction residual (per frame).
    #[serde(default = "default_z_friction")]
    pub z_friction: f32,
    /// C++ m_aerodynamicFriction residual (per frame).
    #[serde(default)]
    pub aerodynamic_friction: f32,
    /// C++ m_extraFriction residual.
    #[serde(default)]
    pub extra_friction: f32,
    /// C++ m_extraBounciness residual (OCL CreateDebris / SlowDeath).
    #[serde(default)]
    pub extra_bounciness: f32,
    /// C++ APPLY_FRICTION2D_WHEN_AIRBORNE flag residual.
    #[serde(default)]
    pub apply_friction_2d_when_airborne: bool,
    /// Cached velocity magnitude residual (negative = invalid).
    #[serde(default = "default_invalid_vel_mag")]
    pub velocity_magnitude_cache: f32,
    /// C++ m_originalAllowBounce residual.
    #[serde(default)]
    pub original_allow_bounce: bool,
    /// C++ STICK_TO_GROUND flag residual.
    #[serde(default)]
    pub stick_to_ground: bool,
    /// C++ ALLOW_TO_FALL flag residual.
    #[serde(default)]
    pub allow_to_fall: bool,
    /// C++ WAS_AIRBORNE_LAST_FRAME residual (general physics, not only shock).
    #[serde(default)]
    pub was_airborne_last_frame: bool,
    /// C++ PhysicsBehaviorModuleData m_centerOfMassOffset residual.
    #[serde(default)]
    pub center_of_mass_offset: f32,
    /// C++ m_pitchRollYawFactor residual (default 2.0).
    #[serde(default = "default_pitch_roll_yaw_factor")]
    pub pitch_roll_yaw_factor: f32,
    /// C++ Locomotor IS_BRAKING flag residual.
    #[serde(default)]
    pub is_braking: bool,
    /// C++ Locomotor m_brakingFactor residual.
    #[serde(default = "default_one_f32")]
    pub braking_factor: f32,
    /// C++ Locomotor braking deceleration residual (units/sec², host Movement space).
    #[serde(default = "default_braking")]
    pub braking: f32,
    /// C++ Locomotor APPLY_2D_FRICTION_WHEN_AIRBORNE residual.
    #[serde(default)]
    pub loco_apply_2d_friction_airborne: bool,
    /// C++ LocomotorTemplate::m_allowMotiveForceWhileAirborne residual.
    #[serde(default)]
    pub allow_motive_force_while_airborne: bool,
    /// C++ LocomotorTemplate::m_locomotorWorksWhenDead residual.
    #[serde(default)]
    pub locomotor_works_when_dead: bool,
    /// C++ LocomotorTemplate::m_airborneTargetingHeight residual (INT_MAX if omitted).
    #[serde(default = "default_airborne_targeting_height")]
    pub airborne_targeting_height: i32,
    /// C++ Locomotor extra2DFriction residual (added to physics extra_friction).
    #[serde(default)]
    pub loco_extra_2d_friction: f32,
    /// C++ PhysicsBehavior m_turning residual.
    #[serde(default)]
    pub physics_turning: PhysicsTurningType,
    /// C++ Locomotor m_behaviorZ residual.
    #[serde(default)]
    pub loco_behavior_z: LocomotorBehaviorZ,
    /// C++ Locomotor m_preferredHeight residual (world Y).
    #[serde(default)]
    pub loco_preferred_height: f32,
    /// C++ preferredHeightDamping residual (0..1).
    #[serde(default = "default_one_f32")]
    pub loco_preferred_height_damping: f32,
    /// C++ MAINTAIN_POS_IS_VALID + m_maintainPos residual.
    #[serde(default)]
    pub maintain_pos_valid: bool,
    #[serde(default)]
    pub maintain_pos: Option<glam::Vec3>,
    /// C++ Locomotor appearance residual.
    #[serde(default)]
    pub loco_appearance: LocomotorAppearance,
    /// C++ m_minTurnSpeed residual (host units/sec).
    #[serde(default)]
    pub min_turn_speed: f32,
    /// C++ m_minSpeed residual (host units/sec).
    #[serde(default)]
    pub min_speed: f32,
    /// C++ `AIUpdateInterface::m_locomotorGoalType`.
    #[serde(default)]
    pub locomotor_goal_type: LocoGoalType,
    /// C++ `m_locomotorGoalData.x` when the goal is ANGLE.
    #[serde(default)]
    pub locomotor_goal_angle: f32,
    /// C++ `AIFaceState::m_canTurnInPlace` (`minSpeed == 0`).
    #[serde(default)]
    pub face_can_turn_in_place: bool,
    /// Host persist: FACE is still marching toward the goal.
    #[serde(default)]
    pub face_active: bool,
    /// C++ Face-position machine goal (`m_obj == false`).
    #[serde(default)]
    pub face_goal_pos: Option<glam::Vec3>,
    /// Logic frame last leftover-marched by Face (prevents double apply).
    #[serde(default)]
    pub face_loco_frame: u32,

    /// C++ LocomotorTemplate `m_ultraAccurateSlideIntoPlaceFactor`.
    #[serde(default)]
    pub ultra_accurate_slide_factor: f32,
    /// C++ ULTRA_ACCURATE flag residual.
    #[serde(default)]
    pub ultra_accurate: bool,
    /// C++ canMoveBackward residual (wheeled).
    #[serde(default)]
    pub can_move_backward: bool,
    /// C++ MOVING_BACKWARDS residual.
    #[serde(default)]
    pub moving_backwards: bool,
    /// C++ NO_SLOW_DOWN_AS_APPROACHING_DEST residual.
    #[serde(default)]
    pub no_slow_down_as_approaching_dest: bool,
    /// C++ OVER_WATER model condition residual (hover).
    #[serde(default)]
    pub over_water: bool,
    /// C++ LocomotorTemplate m_circlingRadius residual (0 = use min turn radius).
    #[serde(default)]
    pub circling_radius: f32,
    /// C++ PRECISE_Z_POS flag residual.
    #[serde(default)]
    pub precise_z_pos: bool,
    /// C++ `Locomotor::ALLOW_INVALID_POSITION` (Locomotor.h:398).
    #[serde(default)]
    pub allow_invalid_position: bool,
    /// C++ LocomotorTemplate::m_maxThrustAngle residual (radians, parseAngleReal).
    #[serde(default)]
    pub max_thrust_angle: f32,
    /// C++ KINDOF_DOZER residual (skip fixInvalidPosition).
    #[serde(default)]
    pub is_dozer: bool,
    /// Host residual: position is on invalid pathfind cell (set by world).
    #[serde(default)]
    pub on_invalid_movement_terrain: bool,
    /// C++ m_turnPivotOffset residual (-1 rear, 0 center, 1 front).
    #[serde(default)]
    pub turn_pivot_offset: f32,
    /// C++ m_wanderWidthFactor residual (0 = off).
    #[serde(default)]
    pub wander_width_factor: f32,
    /// C++ m_angleOffset residual for wander.
    #[serde(default)]
    pub wander_angle_offset: f32,
    /// C++ m_offsetIncrement residual.
    #[serde(default)]
    pub wander_offset_increment: f32,
    /// C++ OFFSET_INCREASING flag residual.
    #[serde(default)]
    pub wander_offset_increasing: bool,
    /// C++ Locomotor downhill-only residual (ski / sled).
    #[serde(default)]
    pub downhill_only: bool,
    /// C++ Locomotor FLAG_CLIMBING residual (`Locomotor.cpp:1711-1716`).
    #[serde(default)]
    pub is_climbing: bool,
    /// C++ Locomotor `m_donutTimer` residual (logic frame).
    #[serde(default = "default_donut_timer")]
    pub donut_timer: u32,
    /// C++ m_lift residual (world-Y up accel capacity).
    #[serde(default)]
    pub max_lift: f32,
    /// C++ LocomotorTemplate::m_liftDamaged residual.
    pub max_lift_damaged: f32,
    /// C++ m_speedLimitZ residual (vertical speed limit).
    #[serde(default)]
    pub speed_limit_z: f32,
    /// C++ group move speed factor residual (1.0 = full).
    #[serde(default = "default_one_f32")]
    pub group_speed_factor: f32,
    /// C++ AIUpdate m_isAttackPath residual.
    #[serde(default)]
    pub is_attack_path: bool,
    /// C++ exact waypoint path residual (no pathfind smoothing).
    pub is_exact_path: bool,
    /// C++ m_isApproachPath residual.
    #[serde(default)]
    pub is_approach_path: bool,
    /// C++ m_isSafePath residual.
    #[serde(default)]
    pub is_safe_path: bool,
    /// C++ m_requestedVictimID residual.
    #[serde(default)]
    pub requested_victim_id: Option<ObjectId>,
    /// C++ `AIUpdateInterface::m_repulsor2` — prior safe-path threat.
    #[serde(default)]
    pub safe_path_repulsor2: Option<ObjectId>,

    /// C++ m_requestedDestination residual.
    #[serde(default)]
    pub requested_destination: Option<glam::Vec3>,
    /// C++ `AIAttackMoveToState::m_retryCount` (ATTACK_RETRY_COUNT=5).
    #[serde(default)]
    pub attack_move_retry_count: i32,
    /// C++ `AIAttackMoveToState::m_frameToSleepUntil`.
    #[serde(default)]
    pub attack_move_sleep_until: u32,
    /// C++ AIUpdateInterface::m_completedWaypoint pathLabel1/2/3.
    #[serde(default)]
    pub completed_waypoint_labels: Vec<String>,
    /// Labels of the waypoint path currently being followed; committed on last hop.
    #[serde(default)]
    pub pending_waypoint_labels: Vec<String>,
    /// C++ m_pathTimestamp residual (frame of last path request).
    #[serde(default)]
    pub path_timestamp: u32,
    /// C++ queue-for-path delay frames remaining (0 = idle).
    #[serde(default)]
    pub queue_for_path_frames: u32,
    /// C++ Weapon maxShotCount residual (-1 = unlimited).
    #[serde(default = "default_max_shots")]
    pub max_shots_to_fire: i32,
    /// C++ AttackStateMachine current substate residual.
    #[serde(default)]
    pub attack_substate: crate::game_logic::AttackSubState,
    /// C++ AIAttackApproachTargetState m_approachTimestamp residual.
    #[serde(default)]
    pub approach_timestamp: u32,
    /// C++ m_prevVictimPos residual (attack approach).
    #[serde(default)]
    pub prev_victim_pos: Option<glam::Vec3>,
    /// C++ temporary move-to frames remaining (AI_MOVE_TO temporary state).
    #[serde(default)]
    pub temporary_move_frames: u32,
    /// C++ BodyDamageType residual (drives DAMAGED/REALLYDAMAGED/RUBBLE bits).
    #[serde(default)]
    pub body_damage_state: crate::game_logic::host_enum_table_residual::HostBodyDamageType,
    /// C++ `AIInternalMoveToState::m_ambientPlayingHandle` event name.
    #[serde(default)]
    pub move_loop_audio: Option<String>,
    /// C++ `Drawable::m_ambientSound` event name currently playing.
    #[serde(default)]
    pub ambient_audio: Option<String>,
    /// C++ `Drawable::m_ambientSoundEnabledFromScript` (ctor true).
    #[serde(default = "default_true")]
    pub ambient_sound_enabled_from_script: bool,


    /// Health system
    pub health: Health,

    /// Movement system
    pub movement: Movement,

    /// Experience system
    pub experience: Experience,
    /// C++ ExperienceTracker::m_experienceSink — forward kill XP to this object.
    #[serde(default)]
    pub experience_sink: Option<ObjectId>,
    /// C++ ExperienceTracker::m_experienceScalar (ctor 1.0).
    #[serde(default = "default_experience_scalar")]
    pub experience_scalar: f32,


    /// Primary weapon
    pub weapon: Option<Weapon>,

    /// Exact `MINE_CLEARING_DETAIL` replacement for the PRIMARY slot.
    /// Separate storage preserves the ordinary primary's reload/ammo state
    /// while the C++ detail flag is active. Older snapshots deserialize this
    /// absent field as unavailable rather than inventing a mine-clear weapon.
    #[serde(default)]
    pub mine_clearing_primary_weapon: Option<Weapon>,

    /// Secondary weapon slot (C++ WeaponSet SECONDARY). Optional residual bind.
    pub secondary_weapon: Option<Weapon>,

    /// Tertiary weapon slot (C++ WeaponSet TERTIARY).
    ///
    /// A missing value is intentionally distinct from PRIMARY: callers must
    /// never turn an unavailable/manual tertiary command into a primary shot.
    #[serde(default)]
    pub tertiary_weapon: Option<Weapon>,

    /// Current target
    pub target: Option<ObjectId>,

    /// Active C++ `SpecialAbilityUpdate` capture phase. The target remains in
    /// `target`; this stores only the authored unpack/preparation/pack timer
    /// so save/load cannot collapse a live channel into an instant transfer.
    #[serde(default)]
    pub capture_channel: Option<CaptureChannelState>,
    /// Persistent C++ `SpecialAbilityUpdate` channel for
    /// `SPECIAL_HACKER_DISABLE_BUILDING`.  This is separate from capture and
    /// generic pending abilities because HDB repeatedly refreshes its target
    /// through `PersistentPrepTime` and starts its recharge at preparation.
    #[serde(default)]
    pub hacker_disable_channel: Option<HackerDisableChannelState>,
    /// C++ `SpecialAbilityUpdate::m_animFrames` while planting C4/TNT.
    #[serde(default)]
    pub charge_plant_unpack_remaining_seconds: Option<f32>,


    /// Construction progress (0.0 to 1.0)
    pub construction_percent: f32,

    /// Building-specific data (present for structures)
    pub building_data: Option<BuildingData>,

    /// Resource storage for buildings
    pub stored_resources: Resources,

    /// Power provided/consumed
    pub power_provided: i32,
    pub power_consumed: i32,

    /// Selection state
    pub selected: bool,
    /// C++ Drawable selection flash envelope residual (frames remaining).
    pub selection_flash_remaining: u32,
    /// C++ `flashAsSelected(&myHouseColor)` envelope RGB. `None` is white default.
    #[serde(default)]
    pub selection_flash_color: Option<[f32; 3]>,
    /// C++ Drawable::m_flashCount (`NAMED/TEAM FLASH` / `FLASH_WHITE`).
    #[serde(default)]
    pub flash_count: i32,
    /// C++ Drawable::m_flashColor packed RGB (`RGBColor::getAsInt`).
    #[serde(default)]
    pub flash_color: u32,


    /// AI state for autonomous behavior
    pub ai_state: AIState,

    // Command system compatibility fields
    /// Object type identifier
    pub object_type: ObjectType,

    /// Template name for identification
    pub template_name: String,

    /// Current position (shadow of thing.position for compatibility)
    pub position: Vec3,

    /// Maximum health
    pub max_health: f32,

    /// C++ `ActiveBody::m_initialHealth` (INI `InitialHealth`). Distinct from
    /// current max. `setMaxHealth` overwrites this; `setInitialHealth` does not.
    #[serde(default)]
    pub initial_health: f32,

    /// Target location for ground attacks
    pub target_location: Option<Vec3>,

    /// Guard position
    pub guard_position: Option<Vec3>,
    /// C++ AIGuardMachine::m_areaToGuard trigger name residual.
    #[serde(default)]
    pub guard_area_trigger: Option<String>,
    /// C++ AIGuardRetaliateMachine goal victim residual.
    #[serde(default)]
    pub guard_retaliate_victim: Option<ObjectId>,
    /// C++ AIUpdateInterface::m_crateCreated residual (notifyCrate).
    #[serde(default)]
    pub crate_created: Option<ObjectId>,
    /// C++ AI_HUNT parent-machine residual. Host Patrolling is AI_HUNT.
    #[serde(default)]
    pub hunting: bool,
    /// C++ HijackerUpdate::m_targetID residual (vehicle being driven).
    #[serde(default)]
    pub hijack_vehicle_id: Option<ObjectId>,
    /// C++ HijackerUpdate::m_isInVehicle residual.
    #[serde(default)]
    pub hijacker_in_vehicle: bool,
    /// C++ HijackerUpdate::m_update residual.
    #[serde(default)]
    pub hijacker_update_active: bool,
    /// C++ HijackerUpdate::m_wasTargetAirborne residual.
    #[serde(default)]
    pub hijacker_was_airborne: bool,
    /// C++ HijackerUpdate::m_ejectPos residual.
    #[serde(default)]
    pub hijacker_eject_pos: Option<glam::Vec3>,
    /// C++ Drawable::setDrawableHidden residual (hijack ride-hide / contain).
    #[serde(default)]
    pub drawable_hidden: bool,

    /// C++ WEAPONSET_CRATEUPGRADE_ONE/TWO residual (0/1/2).
    #[serde(default)]
    pub weapon_crate_upgrade: u8,
    /// C++ ARMORSET_CRATE_UPGRADE_ONE/TWO residual (0/1/2).
    #[serde(default)]
    pub armor_crate_upgrade: u8,
    /// C++ setGoalPositionClipped anchor for GuardRetaliate return residual.
    #[serde(default)]
    pub guard_retaliate_anchor: Option<Vec3>,

    /// Guard target
    pub guard_target: Option<ObjectId>,
    /// C++ AIGuardInner / Outer / AttackAggressor residual (0 none, 1 inner, 2 outer, 3 aggressor).
    #[serde(default)]
    pub guard_chase_phase: u8,
    /// C++ ExitConditions::m_attackGiveUpFrame.
    #[serde(default)]
    pub guard_chase_give_up_frame: u32,


    /// Force attack mode
    pub force_attack: bool,

    /// Visual properties for rendering
    pub show_health_bar: bool,
    pub selection_radius: f32,
    /// Terrain ground height residual at object XY (presentation / FOW residual).
    #[serde(default)]
    pub ground_height: f32,
    /// True when ground_height came from terrain sample (not default 0).
    #[serde(default)]
    pub ground_height_from_terrain: bool,
    pub team_color: [f32; 4],

    /// Tracked occupants for transports/garrisons
    pub occupants: Vec<ObjectId>,
    /// C++ OpenContain::m_playerEnteredMask — last rider's controlling player name.
    /// One-frame pulse: stamped on enter, cleared by OpenContain::update next frame.
    #[serde(default)]
    pub player_who_entered: String,


    /// Residual transport slot capacity (vehicles).
    /// `0` = use footprint heuristic (existing host residual default).
    /// Explicit value (e.g. Humvee/Chinook slots) hard-caps occupants.
    /// Fail-closed: not multi-door / air-transport path parity.
    pub max_transport: usize,

    /// Host residual: China Overlord / BattleBunker infantry capacity.
    ///
    /// C++ OverlordContain holds one PORTABLE_STRUCTURE (BattleBunker), then
    /// redirects infantry contain queries into the bunker's TransportContain
    /// (INI `Slots = 5`). Host residual collapses that redirect into a single
    /// capacity on the tank:
    /// - `None` — not an overlord-style container (normal vehicle residual)
    /// - `Some(0)` — overlord-style without BattleBunker residual (reject enter)
    /// - `Some(n)` — BattleBunker residual active with `n` infantry slots
    ///
    /// Fail-closed: not full OverlordContain redirect / portable-structure spawn /
    /// GattlingCannon / PropagandaTower payload matrix.
    pub overlord_bunker_capacity: Option<usize>,

    /// Host residual: C++ OpenContain `m_passengersAllowedToFire`.
    /// When true, Docked infantry may residual-fire from the container origin
    /// (GLA Battle Bus / Humvee-style fire-from-transport).
    /// Fail-closed: not full garrison weapon-bone positions.
    pub passengers_allowed_to_fire: bool,

    /// Host residual: C++ TransportContain `m_armedRidersUpgradeWeaponSet`.
    /// When true, bus sets `weapon_set_player_upgrade` while any armed infantry
    /// rider is loaded (Battle Bus PLAYER_UPGRADE weapon set residual).
    pub armed_riders_upgrade_weapon_set: bool,

    /// Host residual: C++ WEAPONSET_PLAYER_UPGRADE flag on this object.
    /// Battle Bus uses this when armed riders are present.
    pub weapon_set_player_upgrade: bool,
    /// C++ WEAPONBONUSCONDITION_PLAYER_UPGRADE residual (WeaponBonusUpgrade).
    #[serde(default)]
    pub weapon_bonus_player_upgrade: bool,
    /// C++ ARMORSET_PLAYER_UPGRADE residual (ArmorUpgrade).
    #[serde(default)]
    pub armor_set_player_upgrade: bool,
    /// C++ ARMORSET_VETERAN residual.
    #[serde(default)]
    pub armor_set_veteran: bool,
    /// C++ ARMORSET_ELITE residual.
    #[serde(default)]
    pub armor_set_elite: bool,
    /// C++ ARMORSET_HERO residual.
    #[serde(default)]
    pub armor_set_hero: bool,
    /// C++ WEAPONSET_VETERAN residual (`Object::onVeterancyLevelChanged`).
    #[serde(default)]
    pub weapon_set_veteran: bool,
    /// C++ WEAPONSET_ELITE residual.
    #[serde(default)]
    pub weapon_set_elite: bool,
    /// C++ WEAPONSET_HERO residual.
    #[serde(default)]
    pub weapon_set_hero: bool,
    /// C++ WEAPONBONUSCONDITION_VETERAN residual.
    #[serde(default)]
    pub weapon_bonus_veteran: bool,
    /// C++ WEAPONBONUSCONDITION_ELITE residual.
    #[serde(default)]
    pub weapon_bonus_elite: bool,
    /// C++ WEAPONBONUSCONDITION_HERO residual.
    #[serde(default)]
    pub weapon_bonus_hero: bool,
    /// C++ AIUpdate::m_locomotorUpgrade residual (LocomotorSetUpgrade).
    #[serde(default)]
    pub locomotor_upgrade: bool,
    /// C++ TERRAIN_DECAL_CHEMSUIT residual (ArmorUpgrade ChemicalSuits unique case).
    #[serde(default)]
    pub terrain_decal_chemsuit: bool,
    /// C++ Drawable terrain-decal type residual (HordeUpdate rings).
    #[serde(default = "default_terrain_decal_none")]
    pub terrain_decal_type: u8,
    /// C++ Drawable::setTerrainDecalSize residual (vehicles: 3.5 * majorRadius).
    #[serde(default)]
    pub terrain_decal_size: f32,
    /// C++ Drawable::setTerrainDecalFadeTarget residual.
    #[serde(default)]
    pub terrain_decal_fade_target: f32,
    #[serde(default)]
    pub terrain_decal_fade_rate: f32,
    /// C++ `Drawable::m_decalOpacity` residual.
    #[serde(default)]
    pub terrain_decal_opacity: f32,

    /// C++ SubObjectsUpgrade show/hide residual (Bombload / BombWing peels).
    #[serde(default)]
    pub sub_object_visibility: crate::game_logic::host_sub_objects_upgrade::HostSubObjectVisibility,
    /// C++ SpecialPowerCompletionDie residual (notify script on death).
    #[serde(default)]
    pub special_power_completion: Option<
        crate::game_logic::host_special_power_completion_die::HostSpecialPowerCompletionDieData,
    >,
    /// C++ PowerPlantUpdate m_extended residual.
    #[serde(default)]
    pub power_plant_rods_extended: bool,
    /// Absolute frame when POWER_PLANT_UPGRADING → UPGRADED (0 = idle).
    #[serde(default)]
    pub power_plant_rods_done_frame: u32,
    /// C++ SpecialPowerModule `m_pausedCount` residual (StartsPaused / pauseCountdown).
    /// Refcount, not a set: two pause() calls need two unpauses.
    #[serde(default)]
    pub special_power_paused: std::collections::HashMap<crate::command_system::SpecialPowerType, u32>,
    /// C++ WEAPONSET_MINE_CLEARING_DETAIL residual (DozerAI / AIGroup::setMineClearingDetail).
    #[serde(default)]
    pub weapon_set_mine_clearing_detail: bool,
    /// C++ WEAPONSET_CARBOMB residual.
    #[serde(default)]
    pub weapon_set_carbomb: bool,
    /// C++ WEAPONSET_VEHICLE_HIJACK residual.
    #[serde(default)]
    pub weapon_set_vehicle_hijack: bool,

    /// Host residual: Battle Bus style transport (capacity 8 + fire + armed-riders).
    /// Distinct from generic Humvee transport residual for honesty counters.
    pub is_battle_bus_transport: bool,
    /// C++ UndeadBody + BattleBusSlowDeathBehavior residual.
    pub battle_bus_body: Option<crate::game_logic::host_battle_bus::HostBattleBusBodyData>,
    /// C++ BodyModule ARMORSET_SECOND_LIFE residual.
    pub armor_set_second_life: bool,

    /// Host residual: GLA Technical transport (capacity 5, infantry only, no passenger fire).
    /// Fail-closed: not chassis reskin / salvage W3D gunner swap matrix.
    pub is_technical_transport: bool,

    /// Host residual: GLA Combat Cycle / Combat Bike RiderChangeContain (capacity 1).
    /// Rider weapon switch residual; passengers do not fire from bed (bike fires).
    /// Fail-closed: not full STATUS_RIDER death OCL / scuttle / stealth matrix.
    pub is_combat_cycle_transport: bool,

    /// Host residual: active Combat Cycle rider class (0=none … 7=saboteur).
    /// Mirrors RiderChangeContain WEAPON_RIDER* residual selection.
    pub combat_cycle_rider: u8,

    /// Active parsed `RiderChangeContain::RiderN` ordinal.  This is separate
    /// from the older Combat Cycle weapon residual so generic normal Enter
    /// never has to infer a rider from its template spelling.
    #[serde(default)]
    pub rider_change_active_slot: Option<u8>,
    /// The exact active RiderN model-condition mask, retained to clear the
    /// previous rider before applying a replacement or scuttling on exit.
    #[serde(default)]
    pub rider_change_model_condition_mask: u128,
    /// The exact active RiderN ObjectStatus mask.
    #[serde(default)]
    pub rider_change_object_status_mask: u64,
    /// Frozen active WeaponSet and locomotor set tokens.  The full authored
    /// roster remains on ThingTemplate; these fields record only runtime
    /// selection needed to undo/inspect the physical transaction.
    #[serde(default)]
    pub rider_change_weapon_set: Option<String>,
    #[serde(default)]
    pub rider_change_locomotor_set: Option<String>,
    /// Exact active primary locomotor selected from the parsed RiderN set.
    /// This is distinct from the template's default locomotor so a snapshot
    /// can show whether the authoritative RiderChange transaction actually
    /// bound the authored SET_NORMAL row.
    #[serde(default)]
    pub rider_change_locomotor_name: Option<String>,
    /// C++ RiderChangeContain m_scuttledOnFrame (zero = not scuttled).
    #[serde(default)]
    pub rider_change_scuttled_on_frame: u32,

    /// Host residual: GLA Tunnel Network structure (`TunnelContain`).
    /// Shared per-team capacity via `HostTunnelNetworkRegistry` (MaxTunnelCapacity=10).
    /// Fail-closed: not full GuardTunnelNetwork AI / CaveSystem cave-in matrix.
    pub is_tunnel_network: bool,

    /// C++ CaveContain CaveIndex + CaveSystem registration.
    #[serde(default)]
    pub cave_index: i32,
    /// Host residual: this object is a CaveContain entrance.
    #[serde(default)]
    pub is_cave_contain: bool,

    /// Host residual: AirF Combat Chinook style transport (capacity 8 + fire +
    /// armed-riders + ListeningOutpost dummy). Distinct from vanilla Chinook
    /// (no PassengersAllowedToFire) and from Battle Bus for honesty counters.
    pub is_combat_chinook_transport: bool,
    /// C++ ChinookAIUpdate residual (flight status / auto-land / evac / combat drop).
    #[serde(default)]
    pub chinook_ai: Option<crate::game_logic::host_combat_chinook::HostChinookAI>,


    /// C++ parity (Object::m_containedBy): when this unit is inside a
    /// transport/garrison, stores the container's ID.  None when free.
    pub contained_by: Option<ObjectId>,
    /// C++ `AIUpdateInterface::m_isRecruitable` (default true).
    #[serde(default = "default_true")]
    pub is_recruitable: bool,


    /// Optional short-lived cheer/animation timer
    pub cheer_timer: f32,
    /// C++ AICMD_GO_PRONE residual duration (seconds).
    #[serde(default)]
    pub prone_timer: f32,
    /// C++ Drawable::setEmoticon residual — icon name (empty = none).
    #[serde(default)]
    pub emoticon_name: String,
    /// Remaining logic frames for emoticon (C++ duration frames).
    #[serde(default)]
    pub emoticon_frames_left: i32,
    /// C++ Object custom indicator color residual (`NAMED_CUSTOM_COLOR`).
    /// Packed GameMakeColor `(a<<24)|(r<<16)|(g<<8)|b`. None = house color.
    #[serde(default)]
    pub custom_indicator_color: Option<u32>,
    /// C++ Locomotor::m_closeEnoughDist residual (`SET_STOPPING_DISTANCE`).
    #[serde(default)]
    pub close_enough_dist: Option<f32>,
    /// Leftover `FLAG_CLOSE_ENOUGH_3D` / C++ `Locomotor::isCloseEnoughDist3D`.
    #[serde(default)]
    pub close_enough_dist_3d: bool,
    /// C++ AIUpdateInterface::setSurrendered residual.
    #[serde(default)]
    pub is_surrendered: bool,

    /// C++ Object::m_formationID residual (0 = NO_FORMATION_ID).
    pub formation_id: u32,
    /// C++ Object::m_formationOffset residual (host XZ → Vec2 x/y).
    pub formation_offset: glam::Vec2,

    /// Toggleable weapon/overcharge state flags
    pub overcharge_enabled: bool,
    pub active_weapon_slot: u8,
    /// C++ WeaponSet lock residual.
    #[serde(default)]
    pub weapon_lock_type: WeaponLockType,
    /// Slot held by the lock (PRIMARY=0, SECONDARY=1, TERTIARY=2).
    #[serde(default)]
    pub weapon_lock_slot: u8,
    /// C++ Weapon::m_status residual (active slot).
    pub weapon_fire_status: WeaponFireStatus,
    /// C++ FiringTracker::m_frameToStopLoopingSound residual.
    #[serde(default)]
    pub fire_sound_loop_until_frame: u32,
    /// Active looping FireSound name while until_frame is live.
    #[serde(default)]
    pub fire_sound_loop_name: String,
    /// C++ `WeaponSet` owns a distinct `Weapon` instance per PRIMARY,
    /// SECONDARY, and TERTIARY slot.  Keep each slot's mutable barrel cursor
    /// independent rather than letting a secondary shot rotate PRIMARY's
    /// muzzle/FX state.
    #[serde(default)]
    pub weapon_barrel_states: [WeaponBarrelState; 3],
    /// C++ `Weapon::m_scatterTargetsUnused` per WeaponSet slot. Rebuilt on
    /// clip reload; exhausted mid-clip falls back to ScatterRadius.
    #[serde(default)]
    pub weapon_scatter_targets_unused: [Vec<i32>; 3],
    /// Whether this slot has rebuilt its unused scatter table this clip.
    #[serde(default)]
    pub weapon_scatter_targets_inited: [bool; 3],


    /// C++ Weapon PRE_ATTACK residual: target being wound up against.
    #[serde(default)]
    pub pre_attack_target: Option<ObjectId>,
    /// Absolute sim time when pre-attack delay elapses (ready to discharge).
    #[serde(default)]
    pub pre_attack_ready_at: f32,
    /// C++ Object consecutive-shot residual for PreAttackType PER_ATTACK.
    #[serde(default)]
    pub consecutive_shot_target: Option<ObjectId>,
    #[serde(default)]
    pub consecutive_shots_at_target: u32,
    /// C++ Weapon::m_leechWeaponRangeActive residual (primary).
    #[serde(default)]
    pub leech_range_active_primary: bool,
    /// C++ Weapon::m_leechWeaponRangeActive residual (secondary).
    #[serde(default)]
    pub leech_range_active_secondary: bool,
    /// Host residual: last successful fire_at victim (host object id, 0 = none).
    #[serde(default)]
    pub last_fire_victim_host: u32,
    /// Host residual: weapon slot used on last successful fire_at.
    #[serde(default)]
    pub last_fire_slot: u8,
    /// Host residual: damage snapshot on last successful fire_at.
    #[serde(default)]
    pub last_fire_damage: f32,
    /// Host residual: range snapshot on last successful fire_at.
    #[serde(default)]
    pub last_fire_range: f32,
    /// Host residual: sim time of last successful fire_at.
    #[serde(default)]
    pub last_fire_sim_time: f32,
    /// Host residual: logic frame of last successful fire_at.
    #[serde(default)]
    pub last_fire_frame: u32,
    /// Host residual: cumulative successful fire_at discharges this match.
    #[serde(default)]
    pub fire_intent_count: u32,

    /// Last real accepted WeaponSet discharge. This survives save/load as a
    /// v4 ObjectSnapshot tail; it must never be reconstructed from AI intent.
    #[serde(default)]
    pub last_weapon_discharge_sequence: u64,
    #[serde(default)]
    pub last_weapon_discharge_slot: u8,
    #[serde(default)]
    pub last_weapon_discharge_barrel: u8,
    #[serde(default)]
    pub last_weapon_discharge_frame: u32,
    #[serde(skip)]
    pub visual_object_generation: u64,
    #[serde(skip)]
    pub visual_draw_state_revision: u64,
    #[serde(skip)]
    pub pending_weapon_visual_capture: Option<crate::game_logic::PendingWeaponVisualDispatchCapture>,

    /// Stored guard radius for pathing/AI persistence
    pub guard_radius: f32,

    /// C++ GuardMode residual (Normal / WithoutPursuit / FlyingUnitsOnly).
    pub guard_mode: GuardMode,

    /// C++ AICMD_MOVE_TO_POSITION_AND_EVACUATE residual — unload on path complete.
    #[serde(default)]
    pub pending_evacuate_on_stop: bool,
    /// C++ AICMD_MOVE_TO_POSITION_AND_EVACUATE_AND_EXIT residual — destroy transport after unload.
    #[serde(default)]
    pub pending_exit_after_evacuate: bool,
    /// C++ AIExitState::update — riders already ordered out stream one per
    /// ExitDelay with no hull-stop requirement (hq-ksb4t). Distinct from
    /// move-to-and-evacuate, which waits for arrival.
    #[serde(default)]
    pub pending_stream_exit: bool,

    /// C++ TransportContain::m_frameExitNotBusy — next logic frame a rider may exit.
    #[serde(default)]
    pub frame_exit_not_busy: u32,
    /// C++ OpenContain::m_whichExitPath (1-based ExitStart/End cycle).
    #[serde(default)]
    pub which_exit_path: u8,
    /// C++ `Object::m_layer` — bridge/deck vs ground. Copied onto riders at exit.
    #[serde(default = "default_pathfind_layer_ground")]
    pub pathfind_layer: u8,
    /// C++ `OpenContain::m_doorCloseCountdown` — frames until DOOR_1_CLOSING.
    #[serde(default)]
    pub door_close_countdown: u32,


    /// Applied upgrades keyed by upgrade template/tag name.
    pub applied_upgrades: HashSet<String>,

    /// Special power availability/cooldown state.
    ///
    /// Legacy aggregate residual (HUD/presentation): ready when **all** tracked
    /// per-power cooldowns are clear, remaining = max remaining among them.
    pub special_power_ready: bool,
    pub special_power_cooldown: f32,
    pub special_power_cooldown_remaining: f32,
    /// Per-power residual cooldown remaining (seconds). Independent timers so
    /// A10 vs SpySatellite do not share one charge (C++ SpecialPowerModule style).
    #[serde(default)]
    pub special_power_cooldowns: HashMap<crate::command_system::SpecialPowerType, f32>,
    /// C++ SpecialPowerUpdateInterface overridable destination residual.
    #[serde(default)]
    pub special_power_override_destination: Option<Vec3>,
    /// Which power currently accepts destination override (None = any/active).
    #[serde(default)]
    pub special_power_override_type: Option<crate::command_system::SpecialPowerType>,

    /// Host residual mine / demo-trap / timed demo-charge state.
    /// `None` for ordinary units/structures. Fail-closed: not full C++
    /// MinefieldBehavior / DemoTrapUpdate / StickyBombUpdate modules.
    /// C++ ToppleUpdate residual (trees / crushable props).
    #[serde(default)]
    pub topple_data: Option<crate::game_logic::host_topple::HostToppleData>,
    /// C++ StructureToppleUpdate residual (buildings fall after HP death).
    #[serde(default)]
    pub structure_topple_data:
        Option<crate::game_logic::host_structure_topple::HostStructureToppleData>,
    /// C++ StructureCollapseUpdate residual (civilian buildings sink on death).
    #[serde(default)]
    pub structure_collapse_data:
        Option<crate::game_logic::host_structure_collapse::HostStructureCollapseData>,
    /// C++ KeepObjectDie residual (leave rubble).
    #[serde(default)]
    pub keep_object_die: Option<crate::game_logic::host_keep_object_die::HostKeepObjectDieData>,
    /// C++ WaveGuideUpdate residual.
    #[serde(default)]
    pub wave_guide_data: Option<crate::game_logic::host_wave_guide::HostWaveGuideData>,
    /// C++ FireWeaponWhenDead residual once-fired flag.
    #[serde(default)]
    pub fire_weapon_when_dead_fired: bool,
    /// C++ BoneFXDamage residual.
    #[serde(default)]
    pub bone_fx_damage: Option<crate::game_logic::host_bone_fx_damage::HostBoneFxDamageData>,
    /// C++ PoisonedBehavior residual.
    #[serde(default)]
    pub poisoned_behavior:
        Option<crate::game_logic::host_poisoned_behavior::HostPoisonedBehaviorData>,
    /// C++ ObjectDefectionHelper residual.
    #[serde(default)]
    pub defection_helper: Option<crate::game_logic::host_defection_helper::HostDefectionHelperData>,
    /// C++ FireWeaponPower residual pending attack.
    #[serde(default)]
    pub fire_weapon_power:
        Option<crate::game_logic::host_fire_weapon_power::HostFireWeaponPowerRequest>,
    /// C++ FireWeaponWhenDamagedBehavior residual.
    #[serde(default)]
    pub fire_weapon_when_damaged:
        Option<crate::game_logic::host_fire_weapon_when_damaged::HostFireWeaponWhenDamagedData>,
    /// Object-owned source-keyed C++ temporary Weapon allocations for the
    /// parsed FireWeaponWhenDamaged/Dead behavior modules. Live damage/death
    /// execution is in `world_combat/temporary_weapon_fire.rs`.
    #[serde(default)]
    pub temporary_weapon_runtime:
        crate::game_logic::host_temporary_weapon_behavior::TemporaryWeaponRuntimeBundle,
    /// Pending reaction weapon name from last onDamage residual (drained by GameLogic).
    #[serde(default)]
    pub pending_fire_when_damaged_weapon: Option<String>,
    /// C++ TransitionDamageFX residual.
    #[serde(default)]
    pub transition_damage_fx:
        Option<crate::game_logic::host_transition_damage_fx::HostTransitionDamageFxData>,
    /// Pending transition FX events (drained by GameLogic / presentation).
    #[serde(default)]
    pub pending_transition_damage_fx:
        Vec<crate::game_logic::host_transition_damage_fx::HostTransitionDamageFxEvent>,
    /// C++ FXListDie residual.
    #[serde(default)]
    pub fx_list_die: Option<crate::game_logic::host_fx_list_die::HostFxListDieData>,
    /// Pending death FX name residual.
    #[serde(default)]
    pub pending_death_fx: Option<String>,
    /// Pending death audio residual.
    #[serde(default)]
    pub pending_death_audio: Option<String>,
    /// C++ `TheAudio->removeAudioEvent` for SoundDeathLoop on ground hit.
    #[serde(default)]
    pub pending_death_audio_stop: bool,
    /// C++ CreateObjectDie residual.
    #[serde(default)]
    pub create_object_die:
        Option<crate::game_logic::host_create_object_die::HostCreateObjectDieData>,
    /// Pending spawn templates from CreateObjectDie (drained by GameLogic).
    #[serde(default)]
    pub pending_create_object_die_spawns: Vec<String>,
    /// C++ TransferPreviousHealth residual snapshot (max - previous health).
    #[serde(default)]
    pub create_object_die_transfer_damage: f32,
    /// C++ TransferPreviousHealth subdual residual.
    #[serde(default)]
    pub create_object_die_transfer_subdual: f32,
    /// C++ TransferPreviousHealth last damage source residual.
    #[serde(default)]
    pub create_object_die_transfer_source: Option<ObjectId>,
    /// C++ InstantDeathBehavior residual burst weapon name.
    #[serde(default)]
    pub pending_instant_death_weapon: Option<String>,
    /// C++ CrushDie residual.
    #[serde(default)]
    pub crush_die: Option<crate::game_logic::host_crush_die::HostCrushDieData>,
    /// C++ ActiveBody getPreviousHealth residual.
    #[serde(default)]
    pub previous_health: f32,
    /// C++ LifetimeUpdate residual.
    #[serde(default)]
    pub lifetime_update: Option<crate::game_logic::host_lifetime_update::HostLifetimeUpdateData>,
    /// C++ SlowDeathBehavior residual.
    #[serde(default)]
    pub slow_death: Option<crate::game_logic::host_slow_death::HostSlowDeathData>,
    /// C++ HeightDieUpdate residual.
    #[serde(default)]
    pub height_die: Option<crate::game_logic::host_height_die::HostHeightDieData>,
    /// C++ SlowDeathBehavior residual on FuelAir gas clouds.
    #[serde(default)]
    pub fuel_air_gas_slow_death:
        Option<crate::game_logic::host_fuel_air_gas_slow_death::HostFuelAirGasSlowDeathData>,
    /// C++ NeutronMissileUpdate residual flight.
    #[serde(default)]
    pub neutron_missile_update:
        Option<crate::game_logic::host_neutron_missile_update::HostNeutronMissileUpdateData>,
    /// C++ MissileLauncherBuildingUpdate door state (Scud Storm / Nuke silo).
    #[serde(default)]
    pub missile_launcher_building: Option<
        crate::game_logic::host_missile_launcher_building_update::HostMissileLauncherBuildingUpdateData,
    >,
    /// C++ ScudStormMissile MissileAIUpdate ballistic residual.
    #[serde(default)]
    pub scud_storm_missile_flight:
        Option<crate::game_logic::host_scud_storm_missile_flight::HostScudStormMissileFlightData>,
    /// C++ CarpetBomb payload HeightDie residual.
    #[serde(default)]
    pub carpet_bomb_payload: bool,
    /// C++ AmericaJetB52 carpet transport residual.
    #[serde(default)]
    pub carpet_bomb_transport:
        Option<crate::game_logic::host_carpet_bomb_flight::HostCarpetBombFlightData>,
    /// C++ ChinaArtilleryBarrageShell HeightDie residual.
    #[serde(default)]
    pub artillery_barrage_shell: bool,
    /// C++ ChinaArtilleryCannon transport residual.
    #[serde(default)]
    pub artillery_barrage_transport:
        Option<crate::game_logic::host_artillery_barrage_flight::HostArtilleryBarrageFlightData>,
    /// C++ A10ThunderboltMissile HeightDie residual.
    #[serde(default)]
    pub a10_strike_missile: bool,
    /// C++ AmericaJetA10Thunderbolt transport residual.
    #[serde(default)]
    pub a10_strike_transport:
        Option<crate::game_logic::host_a10_strike_flight::HostA10StrikeFlightData>,
    /// C++ Leaflet AmericaJetB52 transport residual target.
    #[serde(default)]
    pub leaflet_transport_target: Option<glam::Vec3>,
    /// C++ LeafletContainer payload residual (fall then disable).
    #[serde(default)]
    pub leaflet_container: bool,
    /// C++ AmericaJetCargoPlane paradrop transport residual target.
    #[serde(default)]
    pub paradrop_transport_target: Option<glam::Vec3>,
    /// C++ AmericaParachute container residual (fall then infantry land).
    #[serde(default)]
    pub paradrop_parachute: bool,
    /// C++ DaisyCutter AmericaJetB52 transport residual.
    #[serde(default)]
    pub daisy_cutter_transport:
        Option<crate::game_logic::host_daisy_cutter_flight::HostDaisyCutterFlightData>,
    /// C++ DaisyCutterBomb HeightDie residual.
    #[serde(default)]
    pub daisy_cutter_bomb: bool,
    /// C++ AnthraxBomb GLAJetCargoPlane transport residual.
    #[serde(default)]
    pub anthrax_bomb_transport:
        Option<crate::game_logic::host_anthrax_bomb_flight::HostAnthraxBombFlightData>,
    /// C++ AnthraxBomb payload HeightDie residual.
    #[serde(default)]
    pub anthrax_bomb_payload: bool,
    /// C++ GLASneakAttackTunnelNetworkStart residual marker.
    #[serde(default)]
    pub sneak_tunnel_start: bool,
    /// C++ ClusterMines ChinaJetCargoPlane transport residual.
    #[serde(default)]
    pub cluster_mines_transport:
        Option<crate::game_logic::host_cluster_mines_flight::HostClusterMinesFlightData>,
    /// C++ ClusterMinesBomb HeightDie residual.
    #[serde(default)]
    pub cluster_mines_bomb: bool,
    /// C++ EMPPulse ChinaJetCargoPlane transport residual.
    #[serde(default)]
    pub emp_pulse_transport:
        Option<crate::game_logic::host_emp_pulse_flight::HostEmpPulseFlightData>,
    /// C++ EMPPulseBomb HeightDie residual.
    #[serde(default)]
    pub emp_pulse_bomb: bool,
    /// C++ EMPPulseEffectSpheroid residual object.
    #[serde(default)]
    pub emp_pulse_spheroid: bool,
    /// Absolute frame when EMPPulseEffectSpheroid Lifetime residual expires.
    #[serde(default)]
    pub emp_pulse_spheroid_expires_frame: Option<u32>,
    /// C++ ParticleUplinkCannonTrailRemnant residual object.
    #[serde(default)]
    pub particle_trail_remnant: bool,
    /// Absolute frame when TrailRemnant DeletionUpdate residual expires.
    #[serde(default)]
    pub particle_trail_remnant_expires_frame: Option<u32>,
    /// C++ NukeRadiationFieldWeapon residual object.
    #[serde(default)]
    pub nuke_radiation_field: bool,
    /// Absolute frame when NukeRadiationFieldWeapon Lifetime residual expires.
    #[serde(default)]
    pub nuke_radiation_field_expires_frame: Option<u32>,
    /// C++ PoisonFieldAnthraxBomb residual object.
    #[serde(default)]
    pub anthrax_toxin_field: bool,
    /// Absolute frame when PoisonFieldAnthraxBomb Lifetime residual expires.
    #[serde(default)]
    pub anthrax_toxin_field_expires_frame: Option<u32>,
    /// C++ SpectreHowitzerShell residual projectile object.
    #[serde(default)]
    pub spectre_howitzer_shell: bool,
    /// Absolute frame when SpectreHowitzerShell HeightDie residual expires.
    #[serde(default)]
    pub spectre_howitzer_shell_expires_frame: Option<u32>,
    /// C++ ParticleUplinkCannon_OrbitalLaser residual object.
    #[serde(default)]
    pub particle_orbital_laser: bool,
    /// Absolute frame when OrbitalLaser residual expires.
    #[serde(default)]
    pub particle_orbital_laser_expires_frame: Option<u32>,
    /// C++ Medium/Intense ConnectorLaser residual object.
    #[serde(default)]
    pub particle_connector_laser: bool,
    /// Absolute frame when connector laser residual expires.
    #[serde(default)]
    pub particle_connector_laser_expires_frame: Option<u32>,
    /// C++ PointDefenseLaserBeam residual object.
    #[serde(default)]
    pub point_defense_laser_beam: bool,
    /// Absolute frame when PointDefenseLaserBeam Lifetime residual expires.
    #[serde(default)]
    pub point_defense_laser_beam_expires_frame: Option<u32>,
    /// C++ MissileDefender SpecialAbilityUpdate SpecialObject = LaserBeam residual.
    #[serde(default)]
    pub missile_defender_laser_beam: bool,
    /// Absolute frame when MD LaserBeam residual expires (prep window).
    #[serde(default)]
    pub missile_defender_laser_beam_expires_frame: Option<u32>,
    /// C++ BoobyTrap SpecialObject residual (GLA Rebel plant).
    #[serde(default)]
    pub booby_trap_special: bool,
    /// Structure this BoobyTrap SpecialObject is stuck to.
    #[serde(default)]
    pub booby_trap_attached_to: Option<ObjectId>,
    /// C++ CountermeasureFlare SpecialObject residual.
    #[serde(default)]
    pub countermeasure_flare: bool,
    /// Absolute frame when CountermeasureFlare Lifetime residual expires.
    #[serde(default)]
    pub countermeasure_flare_expires_frame: Option<u32>,
    /// C++ AngryMob SpawnBehavior member residual.
    #[serde(default)]
    pub angry_mob_member: bool,
    /// Nexus owner for AngryMob member residual.
    #[serde(default)]
    pub angry_mob_nexus_id: Option<ObjectId>,
    /// C++ Weapon.ini LaserName laser beam SpecialObject residual.
    #[serde(default)]
    pub weapon_laser_beam: bool,
    /// Absolute frame when weapon laser beam Lifetime residual expires.
    #[serde(default)]
    pub weapon_laser_beam_expires_frame: Option<u32>,
    /// C++ ComancheRocketPodRocket projectile residual.
    #[serde(default)]
    pub comanche_rocket_pod_projectile: bool,
    /// Absolute frame when rocket pod projectile residual expires/impacts.
    #[serde(default)]
    pub comanche_rocket_pod_projectile_expires_frame: Option<u32>,
    /// C++ StealthJetMissile projectile residual.
    #[serde(default)]
    pub stealth_jet_missile_projectile: bool,
    #[serde(default)]
    pub stealth_jet_missile_aim: Option<[f32; 3]>,
    #[serde(default)]
    pub stealth_jet_missile_intended: Option<u32>,
    #[serde(default)]
    pub stealth_jet_missile_travelled: f32,
    #[serde(default)]
    pub stealth_jet_missile_fuel_expires_frame: Option<u32>,
    #[serde(default)]
    pub stealth_jet_missile_ignition_frame: Option<u32>,
    /// C++ NapalmBomb SpecialObject residual (Helix drop).
    #[serde(default)]
    pub helix_napalm_bomb_projectile: bool,
    /// C++ SCUDMissile projectile residual (SCUD Launcher gun).
    #[serde(default)]
    pub scud_launcher_missile_projectile: bool,
    /// C++ TomahawkMissile projectile residual.
    #[serde(default)]
    pub tomahawk_missile_projectile: bool,
    /// C++ AuroraBomb SpecialObject residual (dive bomb).
    #[serde(default)]
    pub aurora_bomb_projectile: bool,
    /// C++ RocketBuggyMissile projectile residual.
    #[serde(default)]
    pub rocket_buggy_missile_projectile: bool,
    /// C++ NeutronCannonShell DumbProjectile residual.
    #[serde(default)]
    pub neutron_cannon_shell_projectile: bool,
    /// C++ NukeCannonShell DumbProjectile residual.
    #[serde(default)]
    pub nuke_cannon_shell_projectile: bool,
    /// C++ GenericTankShell DumbProjectile residual (Crusader/Paladin).
    #[serde(default)]
    pub usa_tank_shell_projectile: bool,
    /// USA tank shell launch origin residual.
    #[serde(default)]
    pub usa_tank_shell_from: Option<[f32; 3]>,
    /// USA tank shell aim residual.
    #[serde(default)]
    pub usa_tank_shell_aim: Option<[f32; 3]>,
    /// USA tank shell launch frame residual.
    #[serde(default)]
    pub usa_tank_shell_launch_frame: Option<u32>,
    /// USA tank shell flight frames residual.
    #[serde(default)]
    pub usa_tank_shell_flight_frames: u32,
    /// Weapon speed residual used for this shell flight.
    #[serde(default)]
    pub usa_tank_shell_weapon_speed: f32,
    /// Intended target id residual for USA tank shell.
    #[serde(default)]
    pub usa_tank_shell_intended: Option<u32>,
    /// C++ BattleMasterTankShell DumbProjectile residual.
    #[serde(default)]
    pub battlemaster_shell_projectile: bool,
    /// Battlemaster shell launch origin residual.
    #[serde(default)]
    pub battlemaster_shell_from: Option<[f32; 3]>,
    /// Battlemaster shell aim residual.
    #[serde(default)]
    pub battlemaster_shell_aim: Option<[f32; 3]>,
    /// Battlemaster shell launch frame residual.
    #[serde(default)]
    pub battlemaster_shell_launch_frame: Option<u32>,
    /// Battlemaster shell flight frames residual.
    #[serde(default)]
    pub battlemaster_shell_flight_frames: u32,
    /// Intended target id residual for Battlemaster shell.
    #[serde(default)]
    pub battlemaster_shell_intended: Option<u32>,
    /// C++ OverlordTankShell DumbProjectile residual.
    #[serde(default)]
    pub overlord_shell_projectile: bool,
    #[serde(default)]
    pub overlord_shell_from: Option<[f32; 3]>,
    #[serde(default)]
    pub overlord_shell_aim: Option<[f32; 3]>,
    #[serde(default)]
    pub overlord_shell_launch_frame: Option<u32>,
    #[serde(default)]
    pub overlord_shell_flight_frames: u32,
    #[serde(default)]
    pub overlord_shell_intended: Option<u32>,
    /// C++ InfernoTankShell DumbProjectile residual.
    #[serde(default)]
    pub inferno_shell_projectile: bool,
    #[serde(default)]
    pub inferno_shell_from: Option<[f32; 3]>,
    #[serde(default)]
    pub inferno_shell_aim: Option<[f32; 3]>,
    #[serde(default)]
    pub inferno_shell_launch_frame: Option<u32>,
    #[serde(default)]
    pub inferno_shell_flight_frames: u32,
    #[serde(default)]
    pub inferno_shell_intended: Option<u32>,
    /// BlackNapalm upgraded shell residual.
    #[serde(default)]
    pub inferno_shell_upgraded: bool,
    /// C++ MarauderTankShell DumbProjectile residual.
    #[serde(default)]
    pub marauder_shell_projectile: bool,
    #[serde(default)]
    pub marauder_shell_from: Option<[f32; 3]>,
    #[serde(default)]
    pub marauder_shell_aim: Option<[f32; 3]>,
    #[serde(default)]
    pub marauder_shell_launch_frame: Option<u32>,
    #[serde(default)]
    pub marauder_shell_flight_frames: u32,
    #[serde(default)]
    pub marauder_shell_intended: Option<u32>,
    #[serde(default)]
    pub marauder_shell_weapon_speed: f32,
    /// C++ Fire Base GenericTankShell lob residual.
    #[serde(default)]
    pub fire_base_shell_projectile: bool,
    #[serde(default)]
    pub fire_base_shell_from: Option<[f32; 3]>,
    #[serde(default)]
    pub fire_base_shell_aim: Option<[f32; 3]>,
    #[serde(default)]
    pub fire_base_shell_launch_frame: Option<u32>,
    #[serde(default)]
    pub fire_base_shell_flight_frames: u32,
    #[serde(default)]
    pub fire_base_shell_intended: Option<u32>,
    /// C++ RaptorJetMissile projectile residual.
    #[serde(default)]
    pub raptor_missile_projectile: bool,
    #[serde(default)]
    pub raptor_missile_aim: Option<[f32; 3]>,
    #[serde(default)]
    pub raptor_missile_intended: Option<u32>,
    #[serde(default)]
    pub raptor_missile_travelled: f32,
    #[serde(default)]
    pub raptor_missile_fuel_expires_frame: Option<u32>,
    #[serde(default)]
    pub raptor_missile_ignition_frame: Option<u32>,
    /// C++ NapalmMissile / MiG projectile residual.
    #[serde(default)]
    pub mig_missile_projectile: bool,
    #[serde(default)]
    pub mig_missile_aim: Option<[f32; 3]>,
    #[serde(default)]
    pub mig_missile_intended: Option<u32>,
    #[serde(default)]
    pub mig_missile_travelled: f32,
    #[serde(default)]
    pub mig_missile_fuel_expires_frame: Option<u32>,
    #[serde(default)]
    pub mig_missile_ignition_frame: Option<u32>,
    /// C++ RangerFlashBangGrenade DumbProjectile residual.
    #[serde(default)]
    pub flashbang_grenade_projectile: bool,
    #[serde(default)]
    pub flashbang_grenade_from: Option<[f32; 3]>,
    #[serde(default)]
    pub flashbang_grenade_aim: Option<[f32; 3]>,
    #[serde(default)]
    pub flashbang_grenade_launch_frame: Option<u32>,
    #[serde(default)]
    pub flashbang_grenade_flight_frames: u32,
    #[serde(default)]
    pub flashbang_grenade_intended: Option<u32>,
    /// Host residual: HumveeMissile / PatriotMissile TOW projectile in flight.
    #[serde(default)]
    pub humvee_tow_projectile: bool,
    /// Air TOW (PatriotMissile seek) vs ground TOW (HumveeMissile non-seek).
    #[serde(default)]
    pub humvee_tow_air: bool,
    #[serde(default)]
    pub humvee_tow_aim: Option<[f32; 3]>,
    #[serde(default)]
    pub humvee_tow_intended: Option<u32>,
    #[serde(default)]
    pub humvee_tow_travelled: f32,
    #[serde(default)]
    pub humvee_tow_fuel_expires_frame: Option<u32>,
    #[serde(default)]
    pub humvee_tow_ignition_frame: Option<u32>,
    /// Host residual: DragonTankFlameProjectile in flight.
    #[serde(default)]
    pub dragon_flame_projectile: bool,
    #[serde(default)]
    pub dragon_flame_aim: Option<[f32; 3]>,
    #[serde(default)]
    pub dragon_flame_intended: Option<u32>,
    #[serde(default)]
    pub dragon_flame_travelled: f32,
    #[serde(default)]
    pub dragon_flame_fuel_expires_frame: Option<u32>,
    #[serde(default)]
    pub dragon_flame_ignition_frame: Option<u32>,
    /// Shooter id for projectile stream residual (u32 ObjectId).
    #[serde(default)]
    pub dragon_flame_shooter: Option<u32>,
    /// Host residual: ToxinTruckStreamProjectile in flight.
    #[serde(default)]
    pub toxin_stream_projectile: bool,
    #[serde(default)]
    pub toxin_stream_aim: Option<[f32; 3]>,
    #[serde(default)]
    pub toxin_stream_intended: Option<u32>,
    #[serde(default)]
    pub toxin_stream_travelled: f32,
    #[serde(default)]
    pub toxin_stream_fuel_expires_frame: Option<u32>,
    #[serde(default)]
    pub toxin_stream_ignition_frame: Option<u32>,
    #[serde(default)]
    pub toxin_stream_shooter: Option<u32>,
    /// Host residual: TechnicalRPGMissile in flight.
    #[serde(default)]
    pub technical_rpg_missile_projectile: bool,
    #[serde(default)]
    pub technical_rpg_missile_aim: Option<[f32; 3]>,
    #[serde(default)]
    pub technical_rpg_missile_intended: Option<u32>,
    #[serde(default)]
    pub technical_rpg_missile_travelled: f32,
    #[serde(default)]
    pub technical_rpg_missile_fuel_expires_frame: Option<u32>,
    #[serde(default)]
    pub technical_rpg_missile_ignition_frame: Option<u32>,
    /// Host residual: Technical cannon GenericTankShell in flight.
    #[serde(default)]
    pub technical_cannon_shell_projectile: bool,
    #[serde(default)]
    pub technical_cannon_shell_from: Option<[f32; 3]>,
    #[serde(default)]
    pub technical_cannon_shell_aim: Option<[f32; 3]>,
    #[serde(default)]
    pub technical_cannon_shell_launch_frame: Option<u32>,
    #[serde(default)]
    pub technical_cannon_shell_flight_frames: u32,
    #[serde(default)]
    pub technical_cannon_shell_intended: Option<u32>,
    /// Host residual: projectile has been ECM-jammed (lost lock / scatter).
    #[serde(default)]
    pub ecm_missile_jammed: bool,
    /// Host residual: CleanupStreamProjectile in flight.
    #[serde(default)]
    pub cleanup_stream_projectile: bool,
    #[serde(default)]
    pub cleanup_stream_aim: Option<[f32; 3]>,
    #[serde(default)]
    pub cleanup_stream_intended: Option<u32>,
    #[serde(default)]
    pub cleanup_stream_travelled: f32,
    #[serde(default)]
    pub cleanup_stream_fuel_expires_frame: Option<u32>,
    #[serde(default)]
    pub cleanup_stream_ignition_frame: Option<u32>,
    #[serde(default)]
    pub cleanup_stream_shooter: Option<u32>,
    #[serde(default)]
    pub cleanup_stream_player_id: u32,
    /// Host residual: Angry Mob rock/molotov projectile in flight.
    #[serde(default)]
    pub angry_mob_projectile: bool,
    /// 0 = rock, 1 = molotov.
    #[serde(default)]
    pub angry_mob_projectile_kind: u8,
    #[serde(default)]
    pub angry_mob_projectile_from: Option<[f32; 3]>,
    #[serde(default)]
    pub angry_mob_projectile_aim: Option<[f32; 3]>,
    #[serde(default)]
    pub angry_mob_projectile_launch_frame: Option<u32>,
    #[serde(default)]
    pub angry_mob_projectile_flight_frames: u32,
    #[serde(default)]
    pub angry_mob_projectile_intended: Option<u32>,
    /// Host residual: FireFieldSmall OCL object from Inferno shell impact.
    #[serde(default)]
    pub inferno_fire_field: bool,
    #[serde(default)]
    pub inferno_fire_field_upgraded: bool,
    #[serde(default)]
    pub inferno_fire_field_expires_frame: Option<u32>,
    #[serde(default)]
    pub inferno_fire_field_zone_id: Option<u32>,
    /// Nuke shell launch origin residual.
    #[serde(default)]
    pub nuke_shell_from: Option<[f32; 3]>,
    /// Nuke shell aim residual.
    #[serde(default)]
    pub nuke_shell_aim: Option<[f32; 3]>,
    /// Nuke shell launch frame residual.
    #[serde(default)]
    pub nuke_shell_launch_frame: Option<u32>,
    /// Nuke shell flight frames residual.
    #[serde(default)]
    pub nuke_shell_flight_frames: u32,
    /// C++ TunnelDefenderMissile / RPG projectile residual.
    #[serde(default)]
    pub rpg_trooper_missile_projectile: bool,
    /// C++ TankHunterMissile projectile residual.
    #[serde(default)]
    pub tank_hunter_missile_projectile: bool,
    /// C++ MissileDefenderMissile projectile residual.
    #[serde(default)]
    pub missile_defender_missile_projectile: bool,
    /// Aim point for MissileDefender missile residual.
    #[serde(default)]
    pub missile_defender_missile_aim: Option<[f32; 3]>,
    /// Intended target id residual.
    #[serde(default)]
    pub missile_defender_missile_intended: Option<u32>,
    /// Distance travelled this MissileDefender missile flight residual.
    #[serde(default)]
    pub missile_defender_missile_travelled: f32,
    /// Absolute frame when MissileDefender missile FuelLifetime residual expires.
    #[serde(default)]
    pub missile_defender_missile_fuel_expires_frame: Option<u32>,
    /// Whether this MD missile was fired from laser-guided secondary residual.
    #[serde(default)]
    pub missile_defender_missile_laser_slot: bool,
    /// C++ ScorpionTankShell DumbProjectile residual.
    #[serde(default)]
    pub scorpion_shell_projectile: bool,
    /// Shell launch origin residual.
    #[serde(default)]
    pub scorpion_shell_from: Option<[f32; 3]>,
    /// Shell aim residual.
    #[serde(default)]
    pub scorpion_shell_aim: Option<[f32; 3]>,
    /// Shell launch frame residual.
    #[serde(default)]
    pub scorpion_shell_launch_frame: Option<u32>,
    /// Shell flight frames residual.
    #[serde(default)]
    pub scorpion_shell_flight_frames: u32,
    /// Weapon slot residual for scorpion shell (0=gun).
    #[serde(default)]
    pub scorpion_shell_slot: u8,
    /// C++ ScorpionMissile projectile residual.
    #[serde(default)]
    pub scorpion_missile_projectile: bool,
    /// Aim for ScorpionMissile residual.
    #[serde(default)]
    pub scorpion_missile_aim: Option<[f32; 3]>,
    /// Intended target id residual.
    #[serde(default)]
    pub scorpion_missile_intended: Option<u32>,
    /// Distance travelled residual.
    #[serde(default)]
    pub scorpion_missile_travelled: f32,
    /// Fuel expires frame residual.
    #[serde(default)]
    pub scorpion_missile_fuel_expires_frame: Option<u32>,
    /// Weapon slot residual for scorpion missile.
    #[serde(default)]
    pub scorpion_missile_slot: u8,
    /// Aim point for TankHunter missile residual.
    #[serde(default)]
    pub tank_hunter_missile_aim: Option<[f32; 3]>,
    /// Intended target id residual.
    #[serde(default)]
    pub tank_hunter_missile_intended: Option<u32>,
    /// Distance travelled this TankHunter missile flight residual.
    #[serde(default)]
    pub tank_hunter_missile_travelled: f32,
    /// Absolute frame when TankHunter missile FuelLifetime residual expires.
    #[serde(default)]
    pub tank_hunter_missile_fuel_expires_frame: Option<u32>,
    /// Aim point for RPG missile residual.
    #[serde(default)]
    pub rpg_trooper_missile_aim: Option<[f32; 3]>,
    /// Intended target id residual.
    #[serde(default)]
    pub rpg_trooper_missile_intended: Option<u32>,
    /// Distance travelled this RPG missile flight residual.
    #[serde(default)]
    pub rpg_trooper_missile_travelled: f32,
    /// Absolute frame when RPG missile FuelLifetime residual expires.
    #[serde(default)]
    pub rpg_trooper_missile_fuel_expires_frame: Option<u32>,
    /// Bezier flight start residual.
    #[serde(default)]
    pub neutron_shell_from: Option<[f32; 3]>,
    /// Bezier flight aim residual.
    #[serde(default)]
    pub neutron_shell_aim: Option<[f32; 3]>,
    /// Absolute frame when neutron shell was launched.
    #[serde(default)]
    pub neutron_shell_launch_frame: Option<u32>,
    /// Total flight frames residual for Bezier t.
    #[serde(default)]
    pub neutron_shell_flight_frames: u32,
    /// Aim point for RocketBuggyMissile residual.
    #[serde(default)]
    pub rocket_buggy_missile_aim: Option<[f32; 3]>,
    /// Intended target id for RocketBuggyMissile residual (primary hit).
    #[serde(default)]
    pub rocket_buggy_missile_intended: Option<u32>,
    /// Distance travelled this RocketBuggyMissile flight residual.
    #[serde(default)]
    pub rocket_buggy_missile_travelled: f32,
    /// Absolute frame when RocketBuggyMissile FuelLifetime residual expires.
    #[serde(default)]
    pub rocket_buggy_missile_fuel_expires_frame: Option<u32>,
    /// Aim point for AuroraBomb guided drop residual.
    #[serde(default)]
    pub aurora_bomb_aim: Option<[f32; 3]>,
    /// Host aurora mission id linked to this projectile residual.
    #[serde(default)]
    pub aurora_bomb_mission_id: Option<u32>,
    /// Aim point for TomahawkMissile lob residual.
    #[serde(default)]
    pub tomahawk_missile_aim: Option<[f32; 3]>,
    /// Distance travelled this Tomahawk flight residual.
    #[serde(default)]
    pub tomahawk_missile_travelled: f32,
    /// Absolute frame when Tomahawk FuelLifetime residual expires.
    #[serde(default)]
    pub tomahawk_missile_fuel_expires_frame: Option<u32>,
    /// SCUDMissile toxin warhead residual (secondary / anthrax slot).
    #[serde(default)]
    pub scud_launcher_missile_toxin: bool,
    /// Aim point for SCUDMissile lob residual.
    #[serde(default)]
    pub scud_launcher_missile_aim: Option<[f32; 3]>,
    /// Distance travelled this flight (DistanceToTravelBeforeTurning residual).
    #[serde(default)]
    pub scud_launcher_missile_travelled: f32,
    /// Absolute frame when FuelLifetime residual expires.
    #[serde(default)]
    pub scud_launcher_missile_fuel_expires_frame: Option<u32>,
    /// Absolute frame when StealthJetMissile KillSelfDelay residual expires.
    #[serde(default)]
    pub stealth_jet_missile_expires_frame: Option<u32>,
    /// C++ JetAIUpdate ClipReload airfield rearm ready frame residual.
    #[serde(default)]
    pub airfield_rearm_ready_frame: Option<u32>,
    /// C++ JetAIUpdate `m_producerLocation` — last airfield/spawn position.
    #[serde(default)]
    pub jet_producer_location: Option<[f32; 3]>,
    /// C++ `JetOrHeliCirclingDeadAirfieldState` — hover at last airfield.
    #[serde(default)]
    pub jet_circling_dead_airfield: bool,
    /// C++ CirclingDeadAirfield `m_checkAirfield` next findSuitableAirfield frame.
    #[serde(default)]
    pub jet_circling_airfield_check_frame: u32,
    /// C++ `JetOrHeliReloadAmmoState::m_reloadTime` (logic frames).
    #[serde(default)]
    pub airfield_rearm_duration_frames: u32,
    /// A player-issued `ReturnToBase` is distinct from an empty
    /// ReturnToBase-reload clip.  While it is live, the authoritative
    /// airfield path keeps the C++ ParkingPlace reservation and completes the
    /// landing even if there is no weapon clip to rearm.
    #[serde(default)]
    pub return_to_base_requested: bool,
    /// C++ `JetAIUpdate` live residual (lockon / sneaky / idle RTB / takeoff).
    #[serde(default)]
    pub jet_ai: HostJetAi,

    /// Exact index of the C++ `ParkingPlaceBehavior::m_spaces` entry reserved
    /// for this aircraft.  The producer id names the owning airfield; this
    /// index is the persistent parking reservation and is intentionally not a
    /// generic `garrisoned_units` membership bit.
    #[serde(default)]
    pub airfield_parking_space_index: Option<u32>,
    /// C++ Frenzy_InvisibleMarker DeletionUpdate residual.
    #[serde(default)]
    pub frenzy_invisible_marker: bool,
    /// C++ Ambush CreateObject FadeIn residual (STEALTHED until FadeTime).
    #[serde(default)]
    pub ambush_fade_in: bool,
    /// C++ GPSScrambler_InvisibleMarker residual.
    #[serde(default)]
    pub gps_scrambler_marker: bool,
    /// C++ RepairVehiclesInArea_InvisibleMarker residual.
    #[serde(default)]
    pub emergency_repair_marker: bool,
    /// C++ SpySatellitePing residual object.
    #[serde(default)]
    pub spy_satellite_ping: bool,
    /// Absolute frame when SpySatellitePing DeletionUpdate residual expires.
    #[serde(default)]
    pub spy_satellite_ping_expires_frame: Option<u32>,
    /// C++ RadarVanPing residual object.
    #[serde(default)]
    pub radar_van_ping: bool,
    /// C++ FireWallSegment residual object.
    #[serde(default)]
    pub firewall_segment: bool,
    /// Absolute frame when FireWallSegment DeletionUpdate residual expires.
    #[serde(default)]
    pub firewall_segment_expires_frame: Option<u32>,
    /// Host residual: wall id for InchForward crawl direction lookup.
    #[serde(default)]
    pub firewall_segment_wall_id: Option<u32>,
    /// Host residual: InchForward crawl direction XZ.
    #[serde(default)]
    pub firewall_segment_dir: Option<[f32; 2]>,
    /// Absolute frame when RadarVanPing DeletionUpdate residual expires.
    #[serde(default)]
    pub radar_van_ping_expires_frame: Option<u32>,
    /// C++ TensileFormationUpdate residual (avalanche chunks).
    #[serde(default)]
    pub tensile_formation:
        Option<crate::game_logic::host_tensile_formation::HostTensileFormationData>,
    /// C++ FireSpreadUpdate + FlammableUpdate residual.
    #[serde(default)]
    pub fire_spread: Option<crate::game_logic::host_fire_spread::HostFireSpreadData>,
    /// C++ BaseRegenerateUpdate residual (structure auto-heal).
    #[serde(default)]
    pub base_regenerate: Option<crate::game_logic::host_base_regenerate::HostBaseRegenerateData>,
    /// C++ ModuleTag_DefaultAutoHealBehavior residual (trainable self-heal).
    #[serde(default)]
    pub default_auto_heal: Option<crate::game_logic::host_heal::HostDefaultAutoHealData>,
    /// C++ EnemyNearUpdate residual (MODELCONDITION_ENEMYNEAR).
    #[serde(default)]
    pub enemy_near: Option<crate::game_logic::host_enemy_near::HostEnemyNearData>,

    /// C++ AnimationSteeringUpdate residual (Battle Bus turn anims).
    #[serde(default)]
    pub animation_steering:
        Option<crate::game_logic::host_animation_steering::HostAnimationSteeringData>,
    /// C++ FloatUpdate residual (boat sway / water snap).
    #[serde(default)]
    pub float_update: Option<crate::game_logic::host_float_update::HostFloatUpdateData>,
    /// C++ ProneUpdate residual (infantry cower).
    #[serde(default)]
    pub prone_update: Option<crate::game_logic::host_prone_update::HostProneUpdateData>,
    /// C++ RadiusDecalUpdate residual (SW delivery decal).
    #[serde(default)]
    pub radius_decal_update:
        Option<crate::game_logic::host_radius_decal_update::HostRadiusDecalUpdateData>,
    /// C++ CheckpointUpdate residual (ally gate).
    #[serde(default)]
    pub checkpoint_update:
        Option<crate::game_logic::host_checkpoint_update::HostCheckpointUpdateData>,
    /// C++ SpectreGunshipDeploymentUpdate residual (CC spawns gunship).
    #[serde(default)]
    pub spectre_gunship_deployment: Option<
        crate::game_logic::host_spectre_gunship_deployment::HostSpectreGunshipDeploymentData,
    >,
    /// C++ SpectreGunshipUpdate residual (insertion / doors / afterburner / depart).
    #[serde(default)]
    pub spectre_gunship_update: Option<
        crate::game_logic::host_spectre_gunship_update::HostSpectreGunshipUpdateData,
    >,
    /// C++ SmartBombTargetHomingUpdate residual (MOAB course fudge).
    #[serde(default)]
    pub smart_bomb_target_homing:
        Option<crate::game_logic::host_smart_bomb_target_homing::HostSmartBombTargetHomingData>,
    /// C++ HelicopterSlowDeathBehavior residual.
    #[serde(default)]
    pub helicopter_slow_death:
        Option<crate::game_logic::host_helicopter_slow_death::HostHelicopterSlowDeathData>,
    /// C++ JetSlowDeathBehavior residual.
    #[serde(default)]
    pub jet_slow_death: Option<crate::game_logic::host_jet_slow_death::HostJetSlowDeathData>,
    pub mine_data: Option<crate::game_logic::host_mines::HostMineData>,

    /// Host residual: unit can detect stealthed enemies (C++ StealthDetectorUpdate).
    pub is_detector: bool,
    /// Detection range in world units. `0` => use template `sight_range`
    /// (matches C++ when DetectionRange is unset/0).
    pub detection_range: f32,
    /// StealthDetectorUpdate DetectionRate residual in logic frames.
    /// `0` = continuous every-frame scan (legacy host residual detectors).
    /// Strategy Center S&D residual sets **15** (500ms @ 30 FPS).
    pub detection_rate_frames: u32,
    /// C++ `StealthDetectorUpdateModuleData::m_extraDetectKindof` (ExtraRequiredKindOf).
    /// Empty (0) is C++ ctor `KINDOFMASK_NONE` — accept every kind.
    #[serde(default)]
    pub extra_detect_kindof: u128,
    /// C++ `StealthDetectorUpdateModuleData::m_extraDetectKindofNot` (ExtraForbiddenKindOf).
    #[serde(default)]
    pub extra_detect_kindof_not: u128,
    /// Absolute frame when the next DetectionRate residual scan may fire.
    /// `0` means scan is due immediately (setSDEnabled → UPDATE_SLEEP_NONE).
    pub next_detection_scan_frame: u32,
    /// Logic frame when OBJECT_STATUS_DETECTED expires (0 = no timer).
    /// C++ StealthUpdate::m_detectionExpiresFrame residual.
    pub detection_expires_frame: u32,
    /// C++ STEALTH_NOT_WHILE_ATTACKING residual: firing breaks stealth.
    /// Default true for host residual honesty.
    pub stealth_breaks_on_attack: bool,
    /// C++ StealthForbiddenConditions MOVING residual (Pathfinder): uncloak while moving.
    /// Fail-closed: not full StealthUpdate condition matrix.
    pub stealth_breaks_on_move: bool,
    /// C++ InnateStealth residual: re-cloak when forbidden conditions clear.
    pub innate_stealth: bool,

    /// C++ StealthUpdate disguise residual (Bomb Truck DisguisesAsTeam).
    /// Template the unit is currently disguised as (None when not disguised).
    #[serde(default)]
    pub disguise_as_template: Option<String>,
    /// Pending disguise template while transition residual runs (pre-halfpoint).
    #[serde(default)]
    pub disguise_pending_template: Option<String>,
    /// Pending disguise team while transition residual runs.
    #[serde(default)]
    pub disguise_pending_team: Option<Team>,
    /// Team residual the unit appears as to non-allied viewers while disguised.
    #[serde(default)]
    pub disguise_as_team: Option<Team>,

    /// Host residual: bitmask of player indices currently vision-spying this unit
    /// (C++ Object::m_visionSpiedBy / setVisionSpied for CIA Intelligence SpyVision).
    /// Fail-closed: not full looking_mask partition maintenance.
    pub vision_spied_mask: u32,

    /// Host residual weapon-bonus flags from PropagandaTowerBehavior.
    /// C++ WEAPONBONUSCONDITION_ENTHUSIASTIC / SUBLIMINAL (rate-of-fire buff near speaker tower).
    /// Fail-closed: not full WeaponBonusConditionFlags matrix / ROF multiplier application.
    pub weapon_bonus_enthusiastic: bool,
    pub weapon_bonus_subliminal: bool,

    /// Host residual HORDE weapon bonus (C++ WEAPONBONUSCONDITION_HORDE via HordeUpdate).
    /// RubOffRadius honorary + leftover terrain-decal type/size/fade.
    #[serde(default)]
    pub weapon_bonus_horde: bool,
    /// Host residual NATIONALISM weapon bonus (player Upgrade_Nationalism).
    #[serde(default)]
    pub weapon_bonus_nationalism: bool,
    /// C++ WEAPONBONUSCONDITION_FANATICISM — only while NATIONALISM is set.
    #[serde(default)]
    pub weapon_bonus_fanaticism: bool,
    /// C++ HordeUpdate::m_lastHordeRefreshFrame.
    #[serde(default)]
    pub last_horde_refresh_frame: u32,
    /// C++ constructor first-wake + infantry UPDATE_SLEEP(UpdateRate).
    #[serde(default)]
    pub horde_next_wake_frame: u32,
    /// True after the first HordeUpdate wake on this object.
    #[serde(default)]
    pub horde_wake_initialized: bool,


    /// Host residual Frenzy / Rage temporary attack buff
    /// (C++ WEAPONBONUSCONDITION_FRENZY_ONE/TWO/THREE via doTempWeaponBonus).
    /// Fail-closed: not full WeaponBonusConditionFlags matrix / TempWeaponBonusHelper Xfer.
    pub weapon_bonus_frenzy: bool,
    /// Absolute host logic frame when Frenzy residual expires (0 = none).
    pub weapon_bonus_frenzy_until_frame: u32,
    /// Residual Frenzy tier 1..=3 (maps to FRENZY_ONE/TWO/THREE damage mult).
    pub weapon_bonus_frenzy_level: u8,

    /// Host residual USA Strategy Center battle-plan weapon bonuses
    /// (C++ WEAPONBONUSCONDITION_BATTLEPLAN_* via Player::applyBattlePlanBonuses).
    /// Fail-closed: not full KindOf multi-mask / projectile inheritance matrix.
    #[serde(default)]
    pub weapon_bonus_battle_plan_bombardment: bool,
    #[serde(default)]
    pub weapon_bonus_battle_plan_hold_the_line: bool,
    #[serde(default)]
    pub weapon_bonus_battle_plan_search_and_destroy: bool,
    /// C++ WEAPONBONUSCONDITION_DRONE_SPOTTING residual (Scout drone range-extend).
    #[serde(default)]
    pub weapon_bonus_drone_spotting: bool,
    /// C++ WEAPONBONUSCONDITION_SOLO_HUMAN_*/SOLO_AI_* residual
    /// (`Player::friend_applyDifficultyBonusesForObject`). 0 = unset;
    /// 16..=21 match C++ discriminants (ALLOW_DEMORALIZE off).
    #[serde(default)]
    pub weapon_bonus_solo: u8,
    /// C++ `Object::m_isReceivingDifficultyBonus` (Object.cpp:4410 v5).
    /// Distinct from `weapon_bonus_solo` bits so load can restore the latch
    /// without re-running `friend_applyDifficultyBonusesForObject`.
    #[serde(default)]
    pub is_receiving_difficulty_bonus: bool,


    /// Residual sight-range scale currently applied for SearchAndDestroy (1.0 = none).
    #[serde(default = "default_one_f32")]
    pub battle_plan_sight_scalar_applied: f32,
    /// Host residual continuous-fire ramp (Gattling Tank FiringTracker residual).
    /// Consecutive shots at current victim for ContinuousFireOne/Two thresholds.
    /// Fail-closed: not full model-condition CONTINUOUS_FIRE_* animation matrix.
    #[serde(default)]
    pub continuous_fire_consecutive: u32,
    /// 0=base/slow, 1=mean (200% RoF), 2=fast (300% RoF).
    #[serde(default)]
    pub continuous_fire_level: u8,
    /// C++ WeaponTemplate::m_continuousFireOneShotsNeeded residual (u32::MAX = off).
    pub continuous_fire_one_shots: u32,
    /// C++ WeaponTemplate::m_continuousFireTwoShotsNeeded residual.
    pub continuous_fire_two_shots: u32,
    /// C++ ContinuousFireCoast residual (logic frames; 0 = no auto cool-down timer).
    pub continuous_fire_coast_frames: u32,
    /// C++ AutoReloadWhenIdle residual (logic frames; 0 = disabled).
    pub auto_reload_when_idle_frames: u32,
    /// C++ FiringTracker::m_frameToForceReload residual (0 = none).
    pub frame_to_force_reload: u32,
    /// Absolute host frame until which coast keeps spin-up (0 = none).
    #[serde(default)]
    pub continuous_fire_coast_until_frame: u32,
    /// C++ FireOCLAfterWeaponCooldownUpdate residual (toxin spray secondary).
    pub fire_ocl_after_cooldown:
        Option<crate::game_logic::host_toxin_tractor::HostFireOclAfterCooldownData>,
    /// Last continuous-fire victim object id bits (0 = none/ground).
    #[serde(default)]
    pub continuous_fire_victim: u32,

    /// Absolute host logic frame when FAERIE_FIRE residual expires (0 = none).
    /// C++ StatusDamageHelper m_frameToHeal residual (Avenger paint).
    #[serde(default)]
    pub faerie_fire_until_frame: u32,
    /// C++ ActiveBody m_currentSubdualDamage residual.
    #[serde(default)]
    pub subdual_damage: f32,
    /// C++ ActiveBodyModuleData::m_subdualDamageCap. 0 = canBeSubdued false.
    #[serde(default)]
    pub subdual_damage_cap: f32,
    /// C++ SubdualDamageHealRate residual (frames between heal steps; 0 = no auto-heal).
    #[serde(default)]
    pub subdual_heal_rate_frames: u32,
    /// C++ SubdualDamageHealAmount residual.
    #[serde(default)]
    pub subdual_heal_amount: f32,
    /// Countdown to next subdual heal step.
    #[serde(default)]
    pub subdual_heal_countdown: u32,

    /// Host residual: America Humvee TransportContain (Slots=5 + passengers fire).
    #[serde(default)]
    pub is_humvee_transport: bool,

    /// Host residual: China Listening Outpost TransportContain (Slots=2 + fire +
    /// armed-riders dummy + stealth detector 300 + InnateStealth).
    /// Fail-closed: not multi-door exit / IR FX / RIDERS_ATTACKING uncloak matrix.
    #[serde(default)]
    pub is_listening_outpost_transport: bool,

    /// Host residual: America Pathfinder unit class (StealthDetector + InnateStealth).
    /// Cached at spawn so stealth ticks avoid template-name scans on dense maps.
    #[serde(default)]
    pub is_pathfinder_unit: bool,

    /// Host residual: China Troop Crawler TransportContain (Slots=8 + assault deploy).
    /// Passengers exit to fight (do not fire from inside). Fail-closed vs full
    /// AssaultTransportAIUpdate wounded-retrieve / multi-exit path matrix.
    #[serde(default)]
    pub is_troop_crawler_transport: bool,
    /// C++ AssaultTransportAIUpdate residual state (designated target + members).
    pub assault_transport: Option<crate::game_logic::host_troop_crawler::HostAssaultTransportState>,
    /// C++ DeployStyleAIUpdate pack/unpack residual.
    pub deploy_style: Option<crate::game_logic::host_deploy_style::HostDeployStyleData>,
    /// C++ CommandButtonHuntUpdate residual (special-button hunt).
    pub command_button_hunt:
        Option<crate::game_logic::host_command_button_hunt::HostCommandButtonHuntData>,
    /// C++ AIUpdateInterface::getLastCommandSource residual.
    /// CommandButtonHuntUpdate quits unless this is CMD_FROM_AI.
    #[serde(default = "crate::game_logic::host_command_button_hunt::default_last_command_source")]
    pub last_command_source: u32,


    /// Host residual: Overlord / Helix portable GattlingCannon addon installed
    /// (`Upgrade_ChinaOverlordGattlingCannon` / Helix equivalent). Equips AA
    /// secondary + passenger ground gattling residual on primary fire.
    /// Fail-closed: not full portable-structure passenger object spawn.
    #[serde(default)]
    pub has_overlord_gattling_addon: bool,

    /// Host residual: Overlord / Helix portable PropagandaTower addon installed
    /// (`Upgrade_ChinaOverlordPropagandaTower` / Helix equivalent). Emperor tanks
    /// spawn with this true (innate PropagandaTowerBehavior AffectsSelf).
    /// Fail-closed: not full portable tower object / PulseFX.
    #[serde(default)]
    pub has_overlord_propaganda_addon: bool,

    /// C++ OverlordContain/HelixContain onBodyDamageStateChange setDamageState
    /// on the portable addon. Live addons are flags, so this holds the mirrored
    /// DAMAGED/REALLYDAMAGED visual state (never RUBBLE).
    #[serde(default)]
    pub overlord_addon_body_damage_state:
        crate::game_logic::host_enum_table_residual::HostBodyDamageType,

    /// Spawned portable payload object (gattling / speaker / bunker).
    #[serde(default)]
    pub overlord_portable_occupant: Option<ObjectId>,

    /// Host residual: HelixContain transport (Slots=5, infantry/vehicle/portable).
    /// Fail-closed: not multi-exit / napalm bomb special ability matrix.
    #[serde(default)]
    pub is_helix_transport: bool,

    /// Host residual: C++ Object::m_commandSetStringOverride (CommandSetUpgrade).
    /// Demo SuicideBomb residual swaps to `*CommandSetUpgrade` including
    /// `Demo_Command_TertiarySuicide`. Fail-closed: not full control-bar matrix.
    #[serde(default)]
    pub command_set_override: Option<String>,

    /// Host residual: intentional SUICIDED death already applied PlusFire blast.
    /// Suppresses Demo_DestroyedWeapon double-fire on process_destroy_list.
    #[serde(default)]
    pub demo_suicided_detonating: bool,

    /// Host residual: HiveStructureBody / SpawnBehavior slave count (Stinger Site).
    /// 0 for non-hive units. Mirror of alive residual roster slots.
    #[serde(default)]
    pub hive_slave_count: u8,
    /// Host residual: active residual slave HP (first alive mirror).
    #[serde(default)]
    pub hive_slave_hp: f32,
    /// Absolute host frame when next residual slave respawns (0 = none).
    #[serde(default)]
    pub hive_slave_respawn_frame: u32,
    /// Host residual: physical SpawnBehavior slave roster (getClosestSlave).
    /// Fail-closed: not full soldier Object / AI / W3D bone attach.
    #[serde(default)]
    pub hive_slaves: [crate::game_logic::host_base_defense::ResidualHiveSlave; 3],

    /// Host residual: TurretAI yaw (deg). Defaults **0**; Strategy Center
    /// authors NaturalTurretAngle **-90**.
    #[serde(default)]
    pub turret_angle_deg: f32,
    /// Host residual: TurretAI pitch (deg). Defaults **0**; Strategy Center
    /// authors NaturalTurretPitch **45**.
    #[serde(default)]
    pub turret_pitch_deg: f32,
    /// TurretAI idle-scan residual: absolute frame when next idle scan may start.
    /// 0 = not scheduled (or just completed without reschedule).
    #[serde(default)]
    pub turret_idle_scan_next_frame: u32,
    /// TurretAI idle-scan residual: true while rotating toward desired angle.
    #[serde(default)]
    pub turret_idle_scanning: bool,
    /// TurretAI idle-scan residual: desired absolute yaw while scanning.
    #[serde(default)]
    pub turret_idle_scan_desired_angle_deg: f32,
    /// TurretAI idle-scan residual: deterministic scan index (interval/offset seed).
    #[serde(default)]
    pub turret_idle_scan_index: u32,
    /// TurretAI HoldTurret residual: true while holding after idle-scan complete.
    #[serde(default)]
    pub turret_holding: bool,
    /// TurretAI HoldTurret residual: absolute frame when hold ends (0 = none).
    #[serde(default)]
    pub turret_hold_until_frame: u32,
    /// TurretAI idle-recenter residual: true while recentering after Hold (not pack).
    #[serde(default)]
    pub turret_idle_recentering: bool,
    /// TurretAI idle mood-target residual: target was set by friend_checkForIdleMoodTarget.
    /// Cleared when mood target leaves range / dies (C++ m_targetWasSetByIdleMood).
    #[serde(default)]
    pub turret_mood_target: bool,
    /// C++ TurretAI goal object residual.
    #[serde(default)]
    pub turret_target_id: Option<ObjectId>,
    /// C++ TurretAI m_target forceAttacking residual.
    #[serde(default)]
    pub turret_force_attacking: bool,
    /// C++ TurretAI enabled residual (false until unit has a turret slot).
    #[serde(default)]
    pub turret_enabled: bool,
    /// C++ TurretAIData::m_turnRate residual (radians per logic frame).
    #[serde(default = "default_turret_turn_rate")]
    pub turret_turn_rate_rad: f32,
    /// C++ TurretAI state machine residual.
    #[serde(default)]
    pub turret_substate: TurretSubState,
    /// C++ MODELCONDITION_TURRET_ROTATE residual.
    #[serde(default)]
    pub turret_rotating: bool,
    /// C++ TurretAIData NaturalTurretAngle residual (deg).
    #[serde(default)]
    pub turret_natural_angle_deg: f32,
    /// C++ TurretAIData NaturalTurretPitch residual (deg).
    #[serde(default)]
    pub turret_natural_pitch_deg: f32,
    /// C++ TurretAIData::m_recenterTime residual (logic frames).
    #[serde(default = "default_turret_recenter_frames")]
    pub turret_recenter_frames: u32,

    /// C++ AIUpdateInterface AttitudeType residual (AI_SLEEP..AI_AGGRESSIVE).
    /// Host residual for TurretAI mood matrix Sleep/Passive gates.
    /// Ordinals: -2=Sleep, -1=Passive, 0=Normal, 1=Alert, 2=Aggressive.
    #[serde(default)]
    pub ai_attitude: i8,
    /// C++ ObjectRepulsorHelper residual: frames remaining until REPULSOR clears.
    /// 0 while inactive or for permanent script-set repulsor (no auto-clear).
    #[serde(default)]
    pub repulsor_until_frame: u32,
    /// C++ BodyModule last damage source residual (Passive WaitForAttack).
    /// Set when damage is applied with a known attacker id.
    #[serde(default)]
    pub last_damage_source: Option<ObjectId>,
    /// C++ BodyModule `getLastDamageTimestamp` residual (TunnelContain nemesis window).
    #[serde(default)]
    pub last_damage_timestamp: Option<u32>,
    /// C++ ActiveBody `m_lastHealingTimestamp` (attemptHealing stamps lastDamageInfo).
    #[serde(default)]
    pub last_healing_timestamp: Option<u32>,
    /// C++ ActiveBody `m_lastDamageFXDone` (doDamageFX per-type throttle).
    #[serde(default)]
    pub last_damage_fx_done: Option<crate::game_logic::combat::DamageType>,
    /// C++ `BodyModule::getLastDamageInfo()->in.m_damageType`.
    #[serde(default)]
    pub last_damage_info_type: Option<crate::game_logic::combat::DamageType>,
    /// C++ ActiveBody `m_nextDamageFXTime` (logic frame; throttle gate).
    #[serde(default)]
    pub next_damage_fx_time: u32,
    /// Last-damage source was VEHICLE/INFANTRY/faction structure (same-frame preference).
    #[serde(default)]
    pub last_damage_source_preferred: bool,
    /// C++ ActiveBody scoreTheKill once per death (splash/residual + direct-fire).
    #[serde(default)]
    pub kill_experience_awarded: bool,
    /// C++ Object `m_healthBoxOffset` (SpawnBehavior averages spawn positions).
    #[serde(default)]
    pub health_box_offset: [f32; 3],
    /// C++ InactiveBody templates (FireField / PoisonField / RadiationField).
    #[serde(default)]
    pub uses_inactive_body: bool,
    /// C++ InactiveBody `m_dieCalled` (UNRESISTABLE onDie once).
    #[serde(default)]
    pub inactive_body_die_called: bool,
    /// C++ AIUpdateInterface::m_nextMoodCheckTime residual.
    #[serde(default)]
    pub next_mood_check_time: u32,
    /// C++ m_moodAttackCheckRate residual (logic frames between mood checks).
    #[serde(default = "default_mood_attack_check_rate")]
    pub mood_attack_check_rate: u32,
    /// C++ vision range residual for mood acquire (world units).
    #[serde(default = "default_vision_range")]
    pub vision_range: f32,
    /// C++ Object::m_shroudClearingRange residual (CarBomb endow path).
    #[serde(default = "default_vision_range")]
    pub shroud_clearing_range: f32,
    /// C++ Object::m_shroudRange residual (active enemy fogging radius).
    #[serde(default)]
    pub shroud_range: f32,
    /// C++ ThingTemplate::friend_getBuildCost residual for value-map stamp.
    #[serde(default)]
    pub partition_cash_value: u32,
    /// C++ ThingTemplate::getThreatValue residual for threat-map stamp.
    #[serde(default)]
    pub partition_threat_value: u32,
    /// Last live doValueAffect/doThreatAffect payload (C++ SightingInfo).
    #[serde(default)]
    pub partition_last_affect:
        Option<crate::game_logic::partition_manager::HostPartitionAffectStamp>,
    /// Last doShroudReveal looker (C++ Object::m_partitionLastLook).
    #[serde(default)]
    pub partition_last_look:
        Option<crate::game_logic::partition_manager::HostPartitionLookStamp>,


    /// C++ AutoAcquireEnemiesWhenIdle residual (AAS_Idle bit).
    #[serde(default)]
    pub auto_acquire_when_idle: bool,
    /// C++ `AIUpdateModuleData::m_autoAcquireEnemiesWhenIdle` bitfield.
    #[serde(default)]
    pub auto_acquire_idle_bits: u32,
    /// C++ `AIUpdateModuleData::m_forbidPlayerCommands` residual.
    #[serde(default)]
    pub forbid_player_commands: bool,

    /// C++ AIUpdateInterface attack priority set name residual.
    #[serde(default)]
    pub attack_priority_set: Option<String>,

    /// CamoNetting StealthUpdate FriendlyOpacity residual (0.5 cloaked / 1.0 revealed).
    /// Fail-closed: not full drawable sub-object camo net mesh visual.
    #[serde(default = "default_one_f32")]
    pub camo_friendly_opacity: f32,
    /// StealthUpdate pulse phase residual (radians) while cloaked.
    #[serde(default)]
    pub camo_opacity_pulse_phase: f32,
    /// CamoNetting StealthLook residual (host of Drawable::setStealthLook).
    /// C++ `StealthLookType` / `HostCamoStealthLook` ordinals:
    /// 0=None, 1=VisibleFriendly, 2=DisguisedEnemy, 3=VisibleDetected,
    /// 4=VisibleFriendlyDetected, 5=Invisible.
    /// Fail-closed: not full W3D heat-vision second material pass GPU.
    #[serde(default)]
    pub camo_stealth_look: u8,
    /// Heat-vision second material pass opacity residual (0 or 1 host residual).
    #[serde(default)]
    pub camo_heat_vision_opacity: f32,
    /// CamoNetting sub-object net mesh residual shown (Upgrade_GLACamoNetting applied).
    /// Fail-closed: not full W3D SubObjectsUpgrade / mesh GPU draw.
    #[serde(default)]
    pub camo_net_sub_object_shown: bool,
    /// CamoNetting sub-object residual observer-visible (StealthLook ≠ Invisible).
    #[serde(default)]
    pub camo_net_sub_object_observer_visible: bool,

    /// C++ StealthUpdate StealthDelay residual: earliest frame allowed to re-cloak.
    /// 0 = no delay gate (instant re-cloak residual, e.g. Pathfinder StealthDelay 0).
    #[serde(default)]
    pub stealth_allowed_frame: u32,
    /// Pending StealthDelay scheduling after a reveal (resolved in stealth update).
    #[serde(default)]
    pub stealth_delay_pending: bool,
    /// Frames of StealthDelay after reveal (Camo Rebel / CamoNetting = 75, Sentry = 60).
    /// 0 = instant re-cloak residual.
    #[serde(default)]
    pub stealth_delay_frames: u32,
    /// C++ StealthForbiddenConditions TAKING_DAMAGE residual.
    #[serde(default)]
    pub stealth_breaks_on_damage: bool,
    /// C++ `Drawable::fadeIn` / `fadeOut` residual (0 none, 1 in, 2 out).
    #[serde(default)]
    pub drawable_fade_mode: u8,
    /// Logic frame when the current Drawable fade started.
    #[serde(default)]
    pub drawable_fade_start_frame: u32,
    /// C++ `m_timeToFade` residual (logic frames).
    #[serde(default)]
    pub drawable_fade_frames: u32,
    /// C++ `Drawable::m_explicitOpacity` residual (script/fade channel).
    #[serde(default = "default_one_f32")]
    pub drawable_explicit_opacity: f32,
    /// C++ `Drawable::m_instanceScale` residual (script / EMP pulse scale).
    #[serde(default = "default_one_f32")]
    pub drawable_instance_scale: f32,
    /// C++ `Drawable::m_tintStatus` residual.
    #[serde(default)]
    pub drawable_tint_status: u32,
    /// C++ `Drawable::m_prevTintStatus` residual.
    #[serde(default)]
    pub drawable_prev_tint_status: u32,
    /// C++ `Drawable::m_expirationDate` residual (0 = never).
    #[serde(default)]
    pub drawable_expiration_date: u32,
    /// C++ `DrawableLocoInfo` pitch/roll/yaw residual.
    #[serde(default)]
    pub drawable_loco_pitch: f32,
    #[serde(default)]
    pub drawable_loco_pitch_rate: f32,
    #[serde(default)]
    pub drawable_loco_roll: f32,
    #[serde(default)]
    pub drawable_loco_roll_rate: f32,
    #[serde(default)]
    pub drawable_loco_yaw: f32,
    #[serde(default)]
    pub drawable_loco_accel_pitch: f32,
    #[serde(default)]
    pub drawable_loco_accel_pitch_rate: f32,
    #[serde(default)]
    pub drawable_loco_accel_roll: f32,
    #[serde(default)]
    pub drawable_loco_accel_roll_rate: f32,
    /// C++ overlay icon keepTillFrame + Anim2D snapshot residual.
    #[serde(default)]
    pub drawable_overlay_icons: Vec<DrawableOverlayIcon>,

}

/// C++ `WeaponStatus` (WeaponStatus.h) residual for the active weapon slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum WeaponFireStatus {
    ReadyToFire = 0,
    OutOfAmmo = 1,
    BetweenFiringShots = 2,
    ReloadingClip = 3,
    PreAttack = 4,
}

impl Default for WeaponFireStatus {
    fn default() -> Self {
        Self::ReadyToFire
    }
}

/// C++ WeaponLockType (WeaponSet.h) residual.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum WeaponLockType {
    #[default]
    NotLocked = 0,
    /// Locked until clip empty / attack state exits.
    LockedTemporarily = 1,
    /// Locked until explicitly unlocked or lock changes.
    LockedPermanently = 2,
}

/// C++ `GuardMode` (GameCommon.h) residual for AIGroup::groupGuard*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum GuardMode {
    /// GUARDMODE_NORMAL — may pursue outside the guard area.
    #[default]
    Normal = 0,
    /// GUARDMODE_GUARD_WITHOUT_PURSUIT — no pursuit out of guard area.
    WithoutPursuit = 1,
    /// GUARDMODE_GUARD_FLYING_UNITS_ONLY — ignore non-flyers.
    FlyingUnitsOnly = 2,
}

/// C++ AIGuardRetaliate AttackAggressor residual (`guard_chase_phase`).
pub const GUARD_CHASE_PHASE_RETALIATE: u8 = 4;

/// AI behavior states
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AIState {
    Idle,
    Moving,
    Attacking,
    AttackMoving,
    AttackingGround,
    Gathering,
    ReturningResources,
    Constructing,
    Repairing,
    GuardingArea,
    GuardingObject,
    /// C++ AI_GUARD_RETALIATE residual — attack aggressor with guard restrictions.
    GuardRetaliating,
    Patrolling,
    Docked,
    Garrisoned,
    SpecialAbility,
    SeekingRepair,
    SeekingHealing,
    Entering,
    Docking,
    Capturing,
    /// C++ `AI_FACE_OBJECT`.
    FacingObject,
    /// C++ `AI_FACE_POSITION`.
    FacingPosition,
}

/// C++ `SpecialAbilityUpdate::PackingState` subset for capture abilities.
/// `Unpacking` remains distinct from `Preparing`: the SpecialPower charge is
/// not triggered until preparation begins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CaptureChannelPhase {
    #[default]
    Unpacking,
    Preparing,
    Packing,
}

/// Persistent capture channel timer in seconds. Object INI stores these
/// values in milliseconds; seconds match the host simulation `dt` without
/// fixed-tick truncation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CaptureChannelState {
    pub phase: CaptureChannelPhase,
    pub remaining_seconds: f32,
}

impl CaptureChannelState {
    pub fn new(phase: CaptureChannelPhase, duration_ms: u32) -> Self {
        Self {
            phase,
            remaining_seconds: duration_ms as f32 / 1_000.0,
        }
    }
}

/// C++ `SpecialAbilityUpdate::PackingState` subset for Hacker Disable
/// Building.  It deliberately does not reuse `CaptureChannelPhase`: the two
/// special powers have distinct authority and HDB's preparation can persist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum HackerDisableChannelPhase {
    #[default]
    Unpacking,
    Preparing,
    Packing,
    /// The click has passed C++ ActionManager authority, but the update has
    /// not reached its authored `StartAbilityRange` yet.  Keep this explicit
    /// so a save cannot turn a still-approaching Hacker into an instant
    /// unpack/preparation channel on restore.
    Approaching,
}

/// Save-safe source/target state for one active Hacker Disable Building
/// SpecialAbilityUpdate.  Durations are integrated in seconds, while source
/// INI metadata remains in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HackerDisableChannelState {
    pub target_id: ObjectId,
    pub phase: HackerDisableChannelPhase,
    pub remaining_seconds: f32,
}

impl HackerDisableChannelState {
    pub fn new(target_id: ObjectId, phase: HackerDisableChannelPhase, duration_ms: u32) -> Self {
        Self {
            target_id,
            phase,
            remaining_seconds: duration_ms as f32 / 1_000.0,
        }
    }
}

fn default_shock_up_z() -> f32 {
    1.0
}

/// C++ LOCOMOTORSURFACE_* residual bits (LocomotorSet.h).
pub const LOCO_SURFACE_GROUND: u32 = 1 << 0;
pub const LOCO_SURFACE_WATER: u32 = 1 << 1;
pub const LOCO_SURFACE_CLIFF: u32 = 1 << 2;
pub const LOCO_SURFACE_AIR: u32 = 1 << 3;
pub const LOCO_SURFACE_RUBBLE: u32 = 1 << 4;
/// C++ PhysicsBehavior default friction residuals (per-frame).
/// C++ MOTIVE_FRAMES = LOGICFRAMES_PER_SECOND/3 residual.
pub const MOTIVE_FRAMES_RESIDUAL: u32 = 10;
/// C++ AIAttackApproachTargetState::MIN_RECOMPUTE_TIME residual.
pub const MIN_RECOMPUTE_TIME_RESIDUAL: u32 = 10;
pub const DEFAULT_FORWARD_FRICTION_RESIDUAL: f32 = 0.15;
pub const DEFAULT_LATERAL_FRICTION_RESIDUAL: f32 = 0.15;
pub const DEFAULT_Z_FRICTION_RESIDUAL: f32 = 0.8;
pub const DEFAULT_AERO_FRICTION_RESIDUAL: f32 = 0.0;
pub const MIN_AERO_FRICTION_RESIDUAL: f32 = 0.0;
/// C++ MIN_NON_AERO_FRICTION residual.
pub const MIN_NON_AERO_FRICTION_RESIDUAL: f32 = 0.01;
pub const MAX_FRICTION_RESIDUAL: f32 = 0.99;
/// C++ PATHFIND_CELL_SIZE_F residual (world units).
pub const PATHFIND_CELL_SIZE_F_RESIDUAL: f32 = 10.0;
/// C++ PhysicsBehavior isVerySmall3D residual threshold.
pub const VERY_SMALL_VEL: f32 = 0.01;
/// Authored BounceSound event name used by OCL debris / tests. Not a default.
pub const BOUNCE_SOUND_DEFAULT: &str = "BodyFallGeneric";
/// C++ doBounceSound NORMAL_VEL_Z residual.
pub const BOUNCE_NORMAL_VEL_Z: f32 = 0.25;
/// C++ doBounceSound NORMAL_MASS residual.
pub const BOUNCE_NORMAL_MASS: f32 = 50.0;

fn default_crushable_level() -> u8 {
    255
}

fn default_true() -> bool {
    true
}

fn default_max_f32() -> f32 {
    f32::MAX
}

fn default_physics_mass() -> f32 {
    1.0
}

fn default_forward_friction() -> f32 {
    DEFAULT_FORWARD_FRICTION_RESIDUAL
}
fn default_lateral_friction() -> f32 {
    DEFAULT_LATERAL_FRICTION_RESIDUAL
}
fn default_z_friction() -> f32 {
    DEFAULT_Z_FRICTION_RESIDUAL
}
fn default_invalid_vel_mag() -> f32 {
    -1.0
}

fn default_min_fall_speed_for_damage() -> f32 {
    Object::min_fall_speed_for_damage()
}
fn default_fall_height_damage_factor() -> f32 {
    1.0
}

/// C++ MuLaw residual used by doBounceSound volume adjust.
pub fn bounce_mulaw(x: f32, max_x: f32, mu: f32) -> f32 {
    let max_x = max_x.max(1e-6);
    let ax = (x.abs() / max_x).min(1.0);
    let s = if x >= 0.0 { 1.0 } else { -1.0 };
    s * (1.0 + mu * ax).ln() / (1.0 + mu).ln()
}

/// C++ NormalizeToRange residual.
pub fn bounce_normalize_to_range(v: f32, a: f32, b: f32, c: f32, d: f32) -> f32 {
    if (b - a).abs() < 1e-9 {
        return c;
    }
    let t = ((v - a) / (b - a)).clamp(0.0, 1.0);
    c + t * (d - c)
}

/// C++ doBounceSound volume residual from fall dy and mass.
pub fn bounce_sound_volume_residual(fall_dy: f32, mass: f32) -> f32 {
    let mut vel = fall_dy.abs();
    if vel > BOUNCE_NORMAL_VEL_Z {
        vel = BOUNCE_NORMAL_VEL_Z;
    }
    let mut m = mass.abs();
    if m > BOUNCE_NORMAL_MASS {
        m = BOUNCE_NORMAL_MASS;
    }
    let mut vol = bounce_normalize_to_range(
        bounce_mulaw(vel, BOUNCE_NORMAL_VEL_Z, 500.0),
        -1.0,
        1.0,
        0.25,
        1.0,
    );
    vol *= bounce_normalize_to_range(
        bounce_mulaw(m, BOUNCE_NORMAL_MASS, 500.0),
        -1.0,
        1.0,
        0.25,
        1.0,
    );
    vol.clamp(0.25, 1.0)
}

mod attack;
mod barrels;
mod bonuses;
mod construct;
mod damage;
mod death;
#[cfg(test)]
mod entity_inventory_audit;
mod entity_lifecycle_apply;
#[cfg(test)]
mod entity_lifecycle_audit;
mod entity_lifecycle_envelope;
mod entity_lifecycle_flight;
mod entity_lifecycle_inventory;
mod entity_lifecycle_projectiles;
mod entity_lifecycle_residuals;
mod entity_lifecycle_tags;
mod install;
mod jets;
mod orders;
mod physics;
mod physics_motion;
mod pose;
mod record;
mod rtb;
mod status_bits;
mod stealth;
mod update;
pub mod visual;
mod weapons;

pub use entity_lifecycle_envelope::{
    decode_lifecycle_snapshot_block, encode_lifecycle_snapshot_block,
};
pub use entity_lifecycle_tags::INVENTORY_TAGS;

pub use barrels::WeaponBarrelState;
pub use damage::{prime_live_damage_context, set_pending_damage_status_type};
pub use status_bits::{
    drain_mask_deselects, leftover_object_is_hero, leftover_object_is_kind_of_hero,
    leftover_object_script_targetable,
};

pub use jets::{
    HostJetAi, HostJetPendingResume, JET_AFTERBURNER_SOUND, JET_AFTERBURNER_SOUND_STOP,
    JET_LOCKON_TICK_SOUND, JET_RTB_PHASE_APPROACH, JET_RTB_PHASE_LANDING, JET_RTB_PHASE_TAXI,
    JET_WHEEL_SCREECH_SOUND, JET_WHEEL_SCREECH_Z_SLOP, JetAiTickAction,
    STEALTH_FIGHTER_LOCKON_CURSOR, STEALTH_FIGHTER_LOCKON_TIME_FRAMES,
};
#[cfg(test)]
pub use stealth::reset_drawable_tint_envelopes;
pub use stealth::{
    DRAWABLE_FADE_IN, DRAWABLE_FADE_NONE, DRAWABLE_FADE_OUT, DrawableTintEnvelopePersist,
    MATERIAL_PASS_OPACITY_FADE_SCALAR, SOUND_STEALTH_OFF, SOUND_STEALTH_ON,
    STEALTH_UPDATE_PULSE_PHASE_RATE, TINT_DISABLED_ATTACK_FRAMES, TINT_DISABLED_COLOR,
    TINT_FRENZY_COLOR, TINT_FRENZY_COLOR_INFANTRY, TINT_SUBDUAL_ATTACK_FRAMES, TINT_SUBDUAL_COLOR,
    VERY_TRANSPARENT_MATERIAL_PASS_OPACITY, capture_drawable_tint_envelope,
    drawable_disabled_dark_tint, drawable_explicit_fade_opacity, drawable_status_tint_rgb,
    friendly_stealth_pulse_opacity, is_live_stealth_black_market, order_idle_enemies_on_reveal,
    restore_drawable_tint_envelope, sample_drawable_status_tint,
    stealth_second_material_pass_opacity, stealth_update_pulse_opacity,
};
pub use visual::ObjectVisualInfo;

#[cfg(test)]
mod tests;

/// Concatenated live `object/*.rs` sources (excluding tests) for residual scans.
pub const OBJECT_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("turret_spawn.rs"),
    include_str!("attack.rs"),
    include_str!("bonuses.rs"),
    include_str!("construct.rs"),
    include_str!("damage.rs"),
    include_str!("death.rs"),
    include_str!("install.rs"),
    include_str!("jets.rs"),
    include_str!("orders.rs"),
    include_str!("physics.rs"),
    include_str!("physics_motion.rs"),
    include_str!("pose.rs"),
    include_str!("record.rs"),
    include_str!("rtb.rs"),
    include_str!("status_bits.rs"),
    include_str!("stealth.rs"),
    include_str!("update.rs"),
    include_str!("visual.rs"),
    include_str!("weapons.rs"),
);
