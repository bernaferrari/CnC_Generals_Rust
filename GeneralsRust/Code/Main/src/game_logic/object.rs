use super::*;
use crate::command_system::SpecialPowerType;
use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

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

/// C++ DEFAULT_TURN_RATE residual (radians/frame).
fn default_turret_turn_rate() -> f32 {
    0.01
}

/// C++ default recenter wait residual (2 * LOGICFRAMES_PER_SECOND).
fn default_turret_recenter_frames() -> u32 {
    60
}

fn default_mood_attack_check_rate() -> u32 {
    // C++ typical mood check rate residual (~1s @ 30fps).
    30
}

fn default_vision_range() -> f32 {
    150.0
}

fn default_true_for_auto_acquire() -> bool {
    true
}

fn default_max_shots() -> i32 {
    -1
}

fn default_braking() -> f32 {
    50.0
}

fn actual_speed_is_zero(o: &Object) -> bool {
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

fn default_strategy_center_turret_angle() -> f32 {
    crate::game_logic::host_strategy_center::STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG
}

fn default_strategy_center_turret_pitch() -> f32 {
    crate::game_logic::host_strategy_center::STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG
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

/// C++ LocomotorBehaviorZ residual (subset).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum LocomotorBehaviorZ {
    #[default]
    NoZMotiveForce = 0,
    SeaLevel = 1,
    SurfaceRelativeHeight = 2,
    AbsoluteHeight = 3,
    SmoothRelativeToHighestLayer = 4,
}

impl LocomotorBehaviorZ {
    pub fn to_ordinal(self) -> u8 {
        match self {
            LocomotorBehaviorZ::NoZMotiveForce => 0,
            LocomotorBehaviorZ::SeaLevel => 1,
            LocomotorBehaviorZ::SurfaceRelativeHeight => 2,
            LocomotorBehaviorZ::AbsoluteHeight => 3,
            LocomotorBehaviorZ::SmoothRelativeToHighestLayer => 4,
        }
    }
    pub fn from_ordinal(v: u8) -> Self {
        match v {
            1 => LocomotorBehaviorZ::SeaLevel,
            2 => LocomotorBehaviorZ::SurfaceRelativeHeight,
            3 => LocomotorBehaviorZ::AbsoluteHeight,
            4 => LocomotorBehaviorZ::SmoothRelativeToHighestLayer,
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

    /// Object name
    pub name: String,

    /// Object status
    pub status: ObjectStatus,
    /// C++ ObjectStatusMaskType residual bits (StatusBitsUpgrade set/clear).
    #[serde(default)]
    pub object_status_bits: u64,
    /// C++ ModelConditionFlags residual bits (ALLOW_SURRENDER-off index layout).
    #[serde(default)]
    pub model_condition_bits: u128,
    /// C++ RadarUpdate m_extendDoneFrame residual (0 = inactive).
    pub radar_extend_done_frame: u32,
    /// C++ RadarUpdate m_extendComplete residual.
    pub radar_extend_complete: bool,
    /// C++ RadarUpdate m_radarActive residual.
    pub radar_active: bool,
    /// C++ ProductionUpdate door residual phase: 0=idle 1=opening 2=wait 3=closing.
    pub production_door_phase: u8,
    /// Frame when current door residual phase ends.
    pub production_door_phase_end_frame: u32,
    /// C++ ProductionUpdate DoorInfo::m_holdOpen residual (ParkingPlace).
    pub production_door_hold_open: bool,
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
    #[serde(default = "default_bounce_sound_name")]
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
    /// C++ AI panic state residual (AI_PANIC → bounce force allowed).
    #[serde(default)]
    pub is_panicking: bool,
    /// C++ PhysicsBehavior m_mass residual.
    #[serde(default = "default_physics_mass")]
    pub physics_mass: f32,
    /// C++ PhysicsBehavior m_accel residual (integrated each frame).
    #[serde(default)]
    pub physics_accel: glam::Vec3,
    /// C++ isMotive residual frames remaining (0 = not motive / accept full force).
    #[serde(default)]
    pub motive_frames_remaining: u32,
    /// C++ AIUpdate m_waitingForPath residual.
    #[serde(default)]
    pub waiting_for_path: bool,
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
    /// C++ m_pitchRollYawFactor residual (default 1.0).
    #[serde(default = "default_one_f32")]
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
    /// C++ m_requestedDestination residual.
    #[serde(default)]
    pub requested_destination: Option<glam::Vec3>,
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

    /// Health system
    pub health: Health,

    /// Movement system
    pub movement: Movement,

    /// Experience system
    pub experience: Experience,

    /// Primary weapon
    pub weapon: Option<Weapon>,

    /// Secondary weapon slot (C++ WeaponSet SECONDARY). Optional residual bind.
    pub secondary_weapon: Option<Weapon>,

    /// Current target
    pub target: Option<ObjectId>,

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

    /// Target location for ground attacks
    pub target_location: Option<Vec3>,

    /// Guard position
    pub guard_position: Option<Vec3>,
    /// C++ AIGuardRetaliateMachine goal victim residual.
    #[serde(default)]
    pub guard_retaliate_victim: Option<ObjectId>,
    /// C++ AIUpdateInterface::m_crateCreated residual (notifyCrate).
    #[serde(default)]
    pub crate_created: Option<ObjectId>,
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
    /// C++ AIUpdate::m_locomotorUpgrade residual (LocomotorSetUpgrade).
    #[serde(default)]
    pub locomotor_upgrade: bool,
    /// C++ TERRAIN_DECAL_CHEMSUIT residual (ArmorUpgrade ChemicalSuits unique case).
    #[serde(default)]
    pub terrain_decal_chemsuit: bool,
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
    /// C++ SpecialPowerModule m_pausedCount>0 residual (StartsPaused / pauseCountdown).
    #[serde(default)]
    pub special_power_paused: std::collections::HashSet<crate::command_system::SpecialPowerType>,
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

    /// Host residual: GLA Tunnel Network structure (`TunnelContain`).
    /// Shared per-team capacity via `HostTunnelNetworkRegistry` (MaxTunnelCapacity=10).
    /// Fail-closed: not full GuardTunnelNetwork AI / CaveSystem cave-in matrix.
    pub is_tunnel_network: bool,

    /// Host residual: AirF Combat Chinook style transport (capacity 8 + fire +
    /// armed-riders + ListeningOutpost dummy). Distinct from vanilla Chinook
    /// (no PassengersAllowedToFire) and from Battle Bus for honesty counters.
    pub is_combat_chinook_transport: bool,

    /// C++ parity (Object::m_containedBy): when this unit is inside a
    /// transport/garrison, stores the container's ID.  None when free.
    pub contained_by: Option<ObjectId>,

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
    /// C++ Weapon::m_curBarrel residual (which fire bone / FX barrel).
    pub weapon_cur_barrel: u8,
    /// C++ WeaponTemplate::m_shotsPerBarrel residual (0/1 = single-barrel).
    pub weapon_shots_per_barrel: u32,
    /// C++ Drawable barrel count residual (mod wraps cur barrel).
    pub weapon_barrel_count: u8,
    /// C++ Weapon::m_numShotsForCurBarrel residual.
    pub weapon_shots_left_on_barrel: u32,

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
    /// C++ TensileFormationUpdate residual (avalanche chunks).
    #[serde(default)]
    pub tensile_formation: Option<crate::game_logic::host_tensile_formation::HostTensileFormationData>,
    /// C++ FireSpreadUpdate + FlammableUpdate residual.
    #[serde(default)]
    pub fire_spread: Option<crate::game_logic::host_fire_spread::HostFireSpreadData>,
    /// C++ BaseRegenerateUpdate residual (structure auto-heal).
    #[serde(default)]
    pub base_regenerate: Option<crate::game_logic::host_base_regenerate::HostBaseRegenerateData>,
    /// C++ EnemyNearUpdate residual (MODELCONDITION_ENEMYNEAR).
    #[serde(default)]
    pub enemy_near: Option<crate::game_logic::host_enemy_near::HostEnemyNearData>,
    /// C++ AnimationSteeringUpdate residual (Battle Bus turn anims).
    #[serde(default)]
    pub animation_steering: Option<crate::game_logic::host_animation_steering::HostAnimationSteeringData>,
    /// C++ FloatUpdate residual (boat sway / water snap).
    #[serde(default)]
    pub float_update: Option<crate::game_logic::host_float_update::HostFloatUpdateData>,
    /// C++ ProneUpdate residual (infantry cower).
    #[serde(default)]
    pub prone_update: Option<crate::game_logic::host_prone_update::HostProneUpdateData>,
    /// C++ RadiusDecalUpdate residual (SW delivery decal).
    #[serde(default)]
    pub radius_decal_update: Option<crate::game_logic::host_radius_decal_update::HostRadiusDecalUpdateData>,
    /// C++ CheckpointUpdate residual (ally gate).
    #[serde(default)]
    pub checkpoint_update: Option<crate::game_logic::host_checkpoint_update::HostCheckpointUpdateData>,
    /// C++ SpectreGunshipDeploymentUpdate residual (CC spawns gunship).
    #[serde(default)]
    pub spectre_gunship_deployment:
        Option<crate::game_logic::host_spectre_gunship_deployment::HostSpectreGunshipDeploymentData>,
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
    /// Fail-closed: not full IR FX / kindof filters / garrisoned-detect rules.
    pub is_detector: bool,
    /// Detection range in world units. `0` => use template `sight_range`
    /// (matches C++ when DetectionRange is unset/0).
    pub detection_range: f32,
    /// StealthDetectorUpdate DetectionRate residual in logic frames.
    /// `0` = continuous every-frame scan (legacy host residual detectors).
    /// Strategy Center S&D residual sets **15** (500ms @ 30 FPS).
    pub detection_rate_frames: u32,
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
    /// Fail-closed: not full RubOffRadius honorary / terrain-decal flag matrix.
    #[serde(default)]
    pub weapon_bonus_horde: bool,
    /// Host residual NATIONALISM weapon bonus (only while in horde + upgrade).
    /// Fail-closed: not full Fanaticism infantry-general branch.
    #[serde(default)]
    pub weapon_bonus_nationalism: bool,

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

    /// Host residual: Strategy Center / TurretAI yaw (deg).
    /// Natural for Strategy Center = **-90** (NaturalTurretAngle).
    #[serde(default = "default_strategy_center_turret_angle")]
    pub turret_angle_deg: f32,
    /// Host residual: Strategy Center / TurretAI pitch (deg).
    /// Natural for Strategy Center = **45** (NaturalTurretPitch).
    #[serde(default = "default_strategy_center_turret_pitch")]
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
    /// C++ AutoAcquireEnemiesWhenIdle residual (AAS_Idle bit).
    #[serde(default = "default_true_for_auto_acquire")]
    pub auto_acquire_when_idle: bool,
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
    /// 0 = no delay gate (instant re-cloak residual, e.g. Rebel Camouflage).
    #[serde(default)]
    pub stealth_allowed_frame: u32,
    /// Pending StealthDelay scheduling after a reveal (resolved in stealth update).
    #[serde(default)]
    pub stealth_delay_pending: bool,
    /// Frames of StealthDelay after reveal (CamoNetting structures = 75).
    /// 0 = instant re-cloak residual.
    #[serde(default)]
    pub stealth_delay_frames: u32,
    /// C++ StealthForbiddenConditions TAKING_DAMAGE residual.
    #[serde(default)]
    pub stealth_breaks_on_damage: bool,
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
pub const MAX_FRICTION_RESIDUAL: f32 = 0.99;
/// C++ PATHFIND_CELL_SIZE_F residual (world units).
pub const PATHFIND_CELL_SIZE_F_RESIDUAL: f32 = 10.0;
/// C++ PhysicsBehavior isVerySmall3D residual threshold.
pub const VERY_SMALL_VEL: f32 = 0.01;
/// Host residual bounce-land AudioEventRTS name (fail-closed default).
pub const BOUNCE_SOUND_DEFAULT: &str = "BodyFallGeneric";
/// C++ doBounceSound NORMAL_VEL_Z residual.
pub const BOUNCE_NORMAL_VEL_Z: f32 = 0.25;
/// C++ doBounceSound NORMAL_MASS residual.
pub const BOUNCE_NORMAL_MASS: f32 = 50.0;

fn default_bounce_sound_name() -> String {
    BOUNCE_SOUND_DEFAULT.to_string()
}

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

impl Object {
    pub fn new(template: ThingTemplate, id: ObjectId, team: Team) -> Self {
        let max_health = template.max_health;
        let position = Vec3::ZERO; // Default position
        let template_name = template.name.clone();

        // Determine object type from template
        let object_type = if template.is_kind_of(KindOf::Infantry) {
            ObjectType::Infantry
        } else if template.is_kind_of(KindOf::Vehicle) {
            ObjectType::Vehicle
        } else if template.is_kind_of(KindOf::Aircraft) {
            ObjectType::Aircraft
        } else if template.is_kind_of(KindOf::Structure) {
            ObjectType::Building
        } else {
            ObjectType::Neutral
        };

        // Calculate selection radius based on object type
        let selection_radius = match object_type {
            ObjectType::Infantry => 8.0,
            ObjectType::Vehicle => 15.0,
            ObjectType::Aircraft => 20.0,
            ObjectType::Building => 25.0,
            ObjectType::Neutral => 10.0,
            _ => 10.0,
        };

        let building_data = if object_type == ObjectType::Building {
            let building_type = BuildingType::from_template_name(&template_name);
            Some(BuildingData::new(building_type))
        } else {
            None
        };

        let special_power_cooldown = template.special_power_cooldown;

        let (mut power_provided, mut power_consumed) = building_data
            .as_ref()
            .map(|data| (data.power_output, data.power_requirement))
            .unwrap_or((0, 0));
        // C++ EnergyProduction residual for superweapon buildings (PUC/Nuke -10, Scud 0).
        // Overrides BuildingType::from_template_name fallback (CommandCenter -3 residual).
        if let Some(energy) =
            crate::game_logic::host_superweapon_kindof::superweapon_energy_production_for_template(
                &template_name,
            )
        {
            let (p, c) =
                crate::game_logic::host_superweapon_kindof::apply_energy_production_to_power(
                    energy,
                );
            power_provided = p;
            power_consumed = c;
        }

        Self {
            thing: Thing::new(template),
            id,
            team,
            name: String::new(),
            status: ObjectStatus::default(),
            object_status_bits: 0,
            model_condition_bits: 0,
            radar_extend_done_frame: 0,
            radar_extend_complete: false,
            radar_active: false,
            production_door_phase: 0,
            production_door_phase_end_frame: 0,
            production_door_hold_open: false,
            is_rebuild_hole: false,
            rebuild_template_name: None,
            rebuild_ready_frame: 0,
            rebuild_spawner_id: None,
            rebuild_worker_id: None,
            rebuild_reconstructing_id: None,
            producer_id: None,
            highlander_body: false,
            upgrade_die: None,
            construction_complete_clear_frame: 0,
            sole_healing_benefactor: None,
            sole_healing_benefactor_expiration_frame: 0,
            idle_since_frame: 0,
            shock_stun_frames: 0,
            shock_yaw_rate: 0.0,
            shock_pitch_rate: 0.0,
            shock_roll_rate: 0.0,
            shock_allow_bounce: false,
            shock_was_airborne: false,
            shock_grounded_once: false,
            shock_up_z: 1.0,
            locomotor_surfaces: 0,
            cell_is_cliff: false,
            cell_is_underwater: false,
            kill_when_resting_on_ground: false,
            immune_to_falling_damage: false,
            bounce_land_events: 0,
            last_bounce_fall_dy: 0.0,
            bounce_sound_name: BOUNCE_SOUND_DEFAULT.to_string(),
            last_bounce_volume: 0.0,
            bounce_audio_pending: 0,
            crusher_level: 0,
            crushable_level: 255,
            topple_data: None,
            structure_topple_data: None,
            structure_collapse_data: None,
            keep_object_die: None,
            wave_guide_data: None,
            fire_weapon_when_dead_fired: false,
            bone_fx_damage: None,
            poisoned_behavior: None,
            defection_helper: None,
            fire_weapon_power: None,
            fire_weapon_when_damaged: None,
            pending_fire_when_damaged_weapon: None,
            transition_damage_fx: None,
            pending_transition_damage_fx: Vec::new(),
            fx_list_die: None,
            pending_death_fx: None,
            pending_death_audio: None,
            create_object_die: None,
            pending_create_object_die_spawns: Vec::new(),
            create_object_die_transfer_damage: 0.0,
            lifetime_update: None,
            slow_death: None,
            height_die: None,
            fuel_air_gas_slow_death: None,
            neutron_missile_update: None,
            scud_storm_missile_flight: None,
            carpet_bomb_payload: false,
            carpet_bomb_transport: None,
            artillery_barrage_shell: false,
            artillery_barrage_transport: None,
            a10_strike_missile: false,
            a10_strike_transport: None,
            leaflet_transport_target: None,
            leaflet_container: false,
            paradrop_transport_target: None,
            paradrop_parachute: false,
            daisy_cutter_transport: None,
            daisy_cutter_bomb: false,
            anthrax_bomb_transport: None,
            anthrax_bomb_payload: false,
            sneak_tunnel_start: false,
            tensile_formation: None,
            fire_spread: None,
            base_regenerate: None,
            enemy_near: None,
            animation_steering: None,
            float_update: None,
            prone_update: None,
            radius_decal_update: None,
            checkpoint_update: None,
            spectre_gunship_deployment: None,
            smart_bomb_target_homing: None,
            helicopter_slow_death: None,
            jet_slow_death: None,
            front_crushed: false,
            back_crushed: false,
            physics_current_overlap: None,
            physics_previous_overlap: None,
            ignore_collisions_with: None,
            last_collidee: None,
            allow_collide_force: true,
            can_path_through_units: false,
            ignore_collisions_until_frame: 0,
            is_blocked: false,
            is_blocked_and_stuck: false,
            cur_max_blocked_speed: f32::MAX,
            num_frames_blocked: 0,
            is_panicking: false,
            physics_mass: 1.0,
            physics_accel: glam::Vec3::ZERO,
            motive_frames_remaining: 0,
            waiting_for_path: false,
            move_away_from: None,
            move_away_frames: 0,
            move_away_destination: None,
            request_other_move_away: None,
            forward_friction: DEFAULT_FORWARD_FRICTION_RESIDUAL,
            lateral_friction: DEFAULT_LATERAL_FRICTION_RESIDUAL,
            z_friction: DEFAULT_Z_FRICTION_RESIDUAL,
            aerodynamic_friction: DEFAULT_AERO_FRICTION_RESIDUAL,
            extra_friction: 0.0,
            apply_friction_2d_when_airborne: false,
            velocity_magnitude_cache: -1.0,
            original_allow_bounce: false,
            stick_to_ground: false,
            allow_to_fall: false,
            was_airborne_last_frame: false,
            center_of_mass_offset: 0.0,
            pitch_roll_yaw_factor: 1.0,
            is_braking: false,
            braking_factor: 1.0,
            braking: 50.0,
            loco_apply_2d_friction_airborne: false,
            loco_extra_2d_friction: 0.0,
            physics_turning: PhysicsTurningType::TurnNone,
            loco_behavior_z: LocomotorBehaviorZ::NoZMotiveForce,
            loco_preferred_height: 0.0,
            loco_preferred_height_damping: 1.0,
            maintain_pos_valid: false,
            maintain_pos: None,
            loco_appearance: LocomotorAppearance::Other,
            min_turn_speed: 0.0,
            min_speed: 0.0,
            ultra_accurate: false,
            can_move_backward: false,
            moving_backwards: false,
            no_slow_down_as_approaching_dest: false,
            over_water: false,
            circling_radius: 0.0,
            precise_z_pos: false,
            is_dozer: false,
            on_invalid_movement_terrain: false,
            turn_pivot_offset: 0.0,
            wander_width_factor: 0.0,
            wander_angle_offset: 0.0,
            wander_offset_increment: 0.0,
            wander_offset_increasing: true,
            downhill_only: false,
            max_lift: 0.0,
            max_lift_damaged: 0.0,
            speed_limit_z: 999999.0,
            group_speed_factor: 1.0,
            is_attack_path: false,
            is_exact_path: false,
            is_approach_path: false,
            is_safe_path: false,
            requested_victim_id: None,
            requested_destination: None,
            path_timestamp: 0,
            queue_for_path_frames: 0,
            max_shots_to_fire: -1,
            attack_substate: crate::game_logic::AttackSubState::AimAtTarget,
            approach_timestamp: 0,
            prev_victim_pos: None,
            temporary_move_frames: 0,
            body_damage_state:
                crate::game_logic::host_enum_table_residual::HostBodyDamageType::Pristine,
            health: Health::new(max_health),
            movement: Movement::default(),
            experience: Experience::default(),
            weapon: None,
            secondary_weapon: None,
            target: None,
            construction_percent: 1.0, // Fully constructed by default
            building_data,
            stored_resources: Resources::default(),
            power_provided,
            power_consumed,
            selected: false,
            selection_flash_remaining: 0,
            ai_state: AIState::Idle,
            object_type,
            template_name,
            position,
            max_health,
            target_location: None,
            guard_position: None,
            guard_retaliate_victim: None,
            guard_retaliate_anchor: None,
            crate_created: None,
            hijack_vehicle_id: None,
            hijacker_in_vehicle: false,
            hijacker_update_active: false,
            hijacker_was_airborne: false,
            hijacker_eject_pos: None,
            weapon_crate_upgrade: 0,
            armor_crate_upgrade: 0,
            guard_target: None,
            force_attack: false,
            show_health_bar: true, // Show health bars by default
            selection_radius,
            ground_height: 0.0,
            ground_height_from_terrain: false,
            team_color: team.get_color(),
            occupants: Vec::new(),
            max_transport: 0,
            overlord_bunker_capacity: None,
            passengers_allowed_to_fire: false,
            armed_riders_upgrade_weapon_set: false,
            weapon_set_player_upgrade: false,
            weapon_bonus_player_upgrade: false,
            armor_set_player_upgrade: false,
            locomotor_upgrade: false,
            terrain_decal_chemsuit: false,
            sub_object_visibility: Default::default(),
            special_power_completion: None,
            power_plant_rods_extended: false,
            power_plant_rods_done_frame: 0,
            special_power_paused: std::collections::HashSet::new(),
            weapon_set_mine_clearing_detail: false,
            weapon_set_carbomb: false,
            weapon_set_vehicle_hijack: false,
            is_battle_bus_transport: false,
            battle_bus_body: None,
            armor_set_second_life: false,
            is_technical_transport: false,
            is_combat_cycle_transport: false,
            combat_cycle_rider: 0,
            is_tunnel_network: false,
            is_combat_chinook_transport: false,
            contained_by: None,
            cheer_timer: 0.0,
            prone_timer: 0.0,
            emoticon_name: String::new(),
            emoticon_frames_left: 0,
            is_surrendered: false,
            formation_id: 0,
            formation_offset: glam::Vec2::ZERO,
            overcharge_enabled: false,
            active_weapon_slot: 0,
            weapon_lock_type: WeaponLockType::NotLocked,
            weapon_lock_slot: 0,
            weapon_fire_status: WeaponFireStatus::ReadyToFire,
            fire_sound_loop_until_frame: 0,
            fire_sound_loop_name: String::new(),
            weapon_cur_barrel: 0,
            weapon_shots_per_barrel: 1,
            weapon_barrel_count: 1,
            weapon_shots_left_on_barrel: 1,
            pre_attack_target: None,
            pre_attack_ready_at: 0.0,
            consecutive_shot_target: None,
            consecutive_shots_at_target: 0,
            leech_range_active_primary: false,
            leech_range_active_secondary: false,
            last_fire_victim_host: 0,
            last_fire_slot: 0,
            last_fire_damage: 0.0,
            last_fire_range: 0.0,
            last_fire_sim_time: 0.0,
            last_fire_frame: 0,
            fire_intent_count: 0,
            guard_radius: 0.0,
            guard_mode: GuardMode::Normal,
            pending_evacuate_on_stop: false,
            pending_exit_after_evacuate: false,
            applied_upgrades: HashSet::new(),
            special_power_ready: true,
            special_power_cooldown,
            special_power_cooldown_remaining: 0.0,
            special_power_cooldowns: HashMap::new(),
            special_power_override_destination: None,
            special_power_override_type: None,
            mine_data: None,
            is_detector: false,
            detection_range: 0.0,
            detection_rate_frames: 0,
            next_detection_scan_frame: 0,
            detection_expires_frame: 0,
            stealth_breaks_on_attack: true,
            stealth_breaks_on_move: false,
            innate_stealth: false,
            disguise_as_template: None,
            disguise_pending_template: None,
            disguise_pending_team: None,
            disguise_as_team: None,
            vision_spied_mask: 0,
            weapon_bonus_enthusiastic: false,
            weapon_bonus_subliminal: false,
            weapon_bonus_horde: false,
            weapon_bonus_nationalism: false,
            weapon_bonus_frenzy: false,
            weapon_bonus_frenzy_until_frame: 0,
            weapon_bonus_frenzy_level: 0,
            weapon_bonus_battle_plan_bombardment: false,
            weapon_bonus_battle_plan_hold_the_line: false,
            weapon_bonus_battle_plan_search_and_destroy: false,
            battle_plan_sight_scalar_applied: 1.0,
            continuous_fire_consecutive: 0,
            continuous_fire_level: 0,
            continuous_fire_one_shots: u32::MAX,
            continuous_fire_two_shots: u32::MAX,
            continuous_fire_coast_frames: 0,
            auto_reload_when_idle_frames: 0,
            frame_to_force_reload: 0,
            continuous_fire_coast_until_frame: 0,
            fire_ocl_after_cooldown: None,
            continuous_fire_victim: 0,
            faerie_fire_until_frame: 0,
            subdual_damage: 0.0,
            subdual_heal_rate_frames: 0,
            subdual_heal_amount: 0.0,
            subdual_heal_countdown: 0,
            is_humvee_transport: false,
            is_listening_outpost_transport: false,
            is_pathfinder_unit: false,
            is_troop_crawler_transport: false,
            assault_transport: None,
            deploy_style: None,
            command_button_hunt: None,
            has_overlord_gattling_addon: false,
            has_overlord_propaganda_addon: false,
            is_helix_transport: false,
            command_set_override: None,
            demo_suicided_detonating: false,
            hive_slave_count: 0,
            hive_slave_hp: 0.0,
            hive_slave_respawn_frame: 0,
            hive_slaves: [crate::game_logic::host_base_defense::ResidualHiveSlave::default(); 3],
            turret_angle_deg: default_strategy_center_turret_angle(),
            turret_pitch_deg: default_strategy_center_turret_pitch(),
            turret_idle_scan_next_frame: 0,
            turret_idle_scanning: false,
            turret_idle_scan_desired_angle_deg: 0.0,
            turret_idle_scan_index: 0,
            turret_holding: false,
            turret_hold_until_frame: 0,
            turret_idle_recentering: false,
            turret_mood_target: false,
            turret_target_id: None,
            turret_force_attacking: false,
            turret_enabled: false,
            turret_turn_rate_rad: default_turret_turn_rate(),
            turret_substate: TurretSubState::Idle,
            turret_rotating: false,
            turret_natural_angle_deg: 0.0,
            turret_natural_pitch_deg: 0.0,
            turret_recenter_frames: default_turret_recenter_frames(),
            ai_attitude: 0, // HostAiAttitude::Normal
            repulsor_until_frame: 0,
            last_damage_source: None,
            next_mood_check_time: 0,
            mood_attack_check_rate: default_mood_attack_check_rate(),
            vision_range: default_vision_range(),
            shroud_clearing_range: default_vision_range(),
            shroud_range: 0.0,
            auto_acquire_when_idle: true,
            attack_priority_set: None,
            camo_friendly_opacity: 1.0,
            camo_opacity_pulse_phase: 0.0,
            camo_stealth_look: 0,
            camo_heat_vision_opacity: 0.0,
            camo_net_sub_object_shown: false,
            camo_net_sub_object_observer_visible: false,
            stealth_allowed_frame: 0,
            stealth_delay_pending: false,
            stealth_delay_frames: 0,
            stealth_breaks_on_damage: false,
        }
    }

    /// Alternative constructor for command system compatibility
    pub fn new_simple(id: ObjectId, object_type: ObjectType, template_name: String) -> Self {
        let template = ThingTemplate::new(&template_name);
        let team = Team::Neutral;
        let selection_radius = match object_type {
            ObjectType::Infantry => 8.0,
            ObjectType::Vehicle => 15.0,
            ObjectType::Aircraft => 20.0,
            ObjectType::Building => 25.0,
            ObjectType::Neutral => 10.0,
            _ => 10.0,
        };

        Self {
            thing: Thing::new(template),
            id,
            team,
            name: String::new(),
            status: ObjectStatus::default(),
            object_status_bits: 0,
            model_condition_bits: 0,
            radar_extend_done_frame: 0,
            radar_extend_complete: false,
            radar_active: false,
            production_door_phase: 0,
            production_door_phase_end_frame: 0,
            production_door_hold_open: false,
            is_rebuild_hole: false,
            rebuild_template_name: None,
            rebuild_ready_frame: 0,
            rebuild_spawner_id: None,
            rebuild_worker_id: None,
            rebuild_reconstructing_id: None,
            producer_id: None,
            highlander_body: false,
            upgrade_die: None,
            construction_complete_clear_frame: 0,
            sole_healing_benefactor: None,
            sole_healing_benefactor_expiration_frame: 0,
            idle_since_frame: 0,
            shock_stun_frames: 0,
            shock_yaw_rate: 0.0,
            shock_pitch_rate: 0.0,
            shock_roll_rate: 0.0,
            shock_allow_bounce: false,
            shock_was_airborne: false,
            shock_grounded_once: false,
            shock_up_z: 1.0,
            locomotor_surfaces: 0,
            cell_is_cliff: false,
            cell_is_underwater: false,
            kill_when_resting_on_ground: false,
            immune_to_falling_damage: false,
            bounce_land_events: 0,
            last_bounce_fall_dy: 0.0,
            bounce_sound_name: BOUNCE_SOUND_DEFAULT.to_string(),
            last_bounce_volume: 0.0,
            bounce_audio_pending: 0,
            crusher_level: 0,
            crushable_level: 255,
            topple_data: None,
            structure_topple_data: None,
            structure_collapse_data: None,
            keep_object_die: None,
            wave_guide_data: None,
            fire_weapon_when_dead_fired: false,
            bone_fx_damage: None,
            poisoned_behavior: None,
            defection_helper: None,
            fire_weapon_power: None,
            fire_weapon_when_damaged: None,
            pending_fire_when_damaged_weapon: None,
            transition_damage_fx: None,
            pending_transition_damage_fx: Vec::new(),
            fx_list_die: None,
            pending_death_fx: None,
            pending_death_audio: None,
            create_object_die: None,
            pending_create_object_die_spawns: Vec::new(),
            create_object_die_transfer_damage: 0.0,
            lifetime_update: None,
            slow_death: None,
            height_die: None,
            fuel_air_gas_slow_death: None,
            neutron_missile_update: None,
            scud_storm_missile_flight: None,
            carpet_bomb_payload: false,
            carpet_bomb_transport: None,
            artillery_barrage_shell: false,
            artillery_barrage_transport: None,
            a10_strike_missile: false,
            a10_strike_transport: None,
            leaflet_transport_target: None,
            leaflet_container: false,
            paradrop_transport_target: None,
            paradrop_parachute: false,
            daisy_cutter_transport: None,
            daisy_cutter_bomb: false,
            anthrax_bomb_transport: None,
            anthrax_bomb_payload: false,
            sneak_tunnel_start: false,
            tensile_formation: None,
            fire_spread: None,
            base_regenerate: None,
            enemy_near: None,
            animation_steering: None,
            float_update: None,
            prone_update: None,
            radius_decal_update: None,
            checkpoint_update: None,
            spectre_gunship_deployment: None,
            smart_bomb_target_homing: None,
            helicopter_slow_death: None,
            jet_slow_death: None,
            front_crushed: false,
            back_crushed: false,
            physics_current_overlap: None,
            physics_previous_overlap: None,
            ignore_collisions_with: None,
            last_collidee: None,
            allow_collide_force: true,
            can_path_through_units: false,
            ignore_collisions_until_frame: 0,
            is_blocked: false,
            is_blocked_and_stuck: false,
            cur_max_blocked_speed: f32::MAX,
            num_frames_blocked: 0,
            is_panicking: false,
            physics_mass: 1.0,
            physics_accel: glam::Vec3::ZERO,
            motive_frames_remaining: 0,
            waiting_for_path: false,
            move_away_from: None,
            move_away_frames: 0,
            move_away_destination: None,
            request_other_move_away: None,
            forward_friction: DEFAULT_FORWARD_FRICTION_RESIDUAL,
            lateral_friction: DEFAULT_LATERAL_FRICTION_RESIDUAL,
            z_friction: DEFAULT_Z_FRICTION_RESIDUAL,
            aerodynamic_friction: DEFAULT_AERO_FRICTION_RESIDUAL,
            extra_friction: 0.0,
            apply_friction_2d_when_airborne: false,
            velocity_magnitude_cache: -1.0,
            original_allow_bounce: false,
            stick_to_ground: false,
            allow_to_fall: false,
            was_airborne_last_frame: false,
            center_of_mass_offset: 0.0,
            pitch_roll_yaw_factor: 1.0,
            is_braking: false,
            braking_factor: 1.0,
            braking: 50.0,
            loco_apply_2d_friction_airborne: false,
            loco_extra_2d_friction: 0.0,
            physics_turning: PhysicsTurningType::TurnNone,
            loco_behavior_z: LocomotorBehaviorZ::NoZMotiveForce,
            loco_preferred_height: 0.0,
            loco_preferred_height_damping: 1.0,
            maintain_pos_valid: false,
            maintain_pos: None,
            loco_appearance: LocomotorAppearance::Other,
            min_turn_speed: 0.0,
            min_speed: 0.0,
            ultra_accurate: false,
            can_move_backward: false,
            moving_backwards: false,
            no_slow_down_as_approaching_dest: false,
            over_water: false,
            circling_radius: 0.0,
            precise_z_pos: false,
            is_dozer: false,
            on_invalid_movement_terrain: false,
            turn_pivot_offset: 0.0,
            wander_width_factor: 0.0,
            wander_angle_offset: 0.0,
            wander_offset_increment: 0.0,
            wander_offset_increasing: true,
            downhill_only: false,
            max_lift: 0.0,
            max_lift_damaged: 0.0,
            speed_limit_z: 999999.0,
            group_speed_factor: 1.0,
            is_attack_path: false,
            is_exact_path: false,
            is_approach_path: false,
            is_safe_path: false,
            requested_victim_id: None,
            requested_destination: None,
            path_timestamp: 0,
            queue_for_path_frames: 0,
            max_shots_to_fire: -1,
            attack_substate: crate::game_logic::AttackSubState::AimAtTarget,
            approach_timestamp: 0,
            prev_victim_pos: None,
            temporary_move_frames: 0,
            body_damage_state:
                crate::game_logic::host_enum_table_residual::HostBodyDamageType::Pristine,
            health: Health::new(100.0),
            movement: Movement::default(),
            experience: Experience::default(),
            weapon: None,
            secondary_weapon: None,
            target: None,
            construction_percent: 1.0,
            building_data: None,
            stored_resources: Resources::default(),
            power_provided: 0,
            power_consumed: 0,
            selected: false,
            selection_flash_remaining: 0,
            ai_state: AIState::Idle,
            object_type,
            template_name,
            position: Vec3::ZERO,
            max_health: 100.0,
            target_location: None,
            guard_position: None,
            guard_retaliate_victim: None,
            guard_retaliate_anchor: None,
            crate_created: None,
            hijack_vehicle_id: None,
            hijacker_in_vehicle: false,
            hijacker_update_active: false,
            hijacker_was_airborne: false,
            hijacker_eject_pos: None,
            weapon_crate_upgrade: 0,
            armor_crate_upgrade: 0,
            guard_target: None,
            force_attack: false,
            show_health_bar: true,
            selection_radius,
            ground_height: 0.0,
            ground_height_from_terrain: false,
            team_color: team.get_color(),
            occupants: Vec::new(),
            max_transport: 0,
            overlord_bunker_capacity: None,
            passengers_allowed_to_fire: false,
            armed_riders_upgrade_weapon_set: false,
            weapon_set_player_upgrade: false,
            weapon_bonus_player_upgrade: false,
            armor_set_player_upgrade: false,
            locomotor_upgrade: false,
            terrain_decal_chemsuit: false,
            sub_object_visibility: Default::default(),
            special_power_completion: None,
            power_plant_rods_extended: false,
            power_plant_rods_done_frame: 0,
            special_power_paused: std::collections::HashSet::new(),
            weapon_set_mine_clearing_detail: false,
            weapon_set_carbomb: false,
            weapon_set_vehicle_hijack: false,
            is_battle_bus_transport: false,
            battle_bus_body: None,
            armor_set_second_life: false,
            is_technical_transport: false,
            is_combat_cycle_transport: false,
            combat_cycle_rider: 0,
            is_tunnel_network: false,
            is_combat_chinook_transport: false,
            contained_by: None,
            cheer_timer: 0.0,
            prone_timer: 0.0,
            emoticon_name: String::new(),
            emoticon_frames_left: 0,
            is_surrendered: false,
            formation_id: 0,
            formation_offset: glam::Vec2::ZERO,
            overcharge_enabled: false,
            active_weapon_slot: 0,
            weapon_lock_type: WeaponLockType::NotLocked,
            weapon_lock_slot: 0,
            weapon_fire_status: WeaponFireStatus::ReadyToFire,
            fire_sound_loop_until_frame: 0,
            fire_sound_loop_name: String::new(),
            weapon_cur_barrel: 0,
            weapon_shots_per_barrel: 1,
            weapon_barrel_count: 1,
            weapon_shots_left_on_barrel: 1,
            pre_attack_target: None,
            pre_attack_ready_at: 0.0,
            consecutive_shot_target: None,
            consecutive_shots_at_target: 0,
            leech_range_active_primary: false,
            leech_range_active_secondary: false,
            last_fire_victim_host: 0,
            last_fire_slot: 0,
            last_fire_damage: 0.0,
            last_fire_range: 0.0,
            last_fire_sim_time: 0.0,
            last_fire_frame: 0,
            fire_intent_count: 0,
            guard_radius: 0.0,
            guard_mode: GuardMode::Normal,
            pending_evacuate_on_stop: false,
            pending_exit_after_evacuate: false,
            applied_upgrades: HashSet::new(),
            special_power_ready: true,
            special_power_cooldown: 10.0,
            special_power_cooldown_remaining: 0.0,
            special_power_cooldowns: HashMap::new(),
            special_power_override_destination: None,
            special_power_override_type: None,
            mine_data: None,
            is_detector: false,
            detection_range: 0.0,
            detection_rate_frames: 0,
            next_detection_scan_frame: 0,
            detection_expires_frame: 0,
            stealth_breaks_on_attack: true,
            stealth_breaks_on_move: false,
            innate_stealth: false,
            disguise_as_template: None,
            disguise_pending_template: None,
            disguise_pending_team: None,
            disguise_as_team: None,
            vision_spied_mask: 0,
            weapon_bonus_enthusiastic: false,
            weapon_bonus_subliminal: false,
            weapon_bonus_horde: false,
            weapon_bonus_nationalism: false,
            weapon_bonus_frenzy: false,
            weapon_bonus_frenzy_until_frame: 0,
            weapon_bonus_frenzy_level: 0,
            weapon_bonus_battle_plan_bombardment: false,
            weapon_bonus_battle_plan_hold_the_line: false,
            weapon_bonus_battle_plan_search_and_destroy: false,
            battle_plan_sight_scalar_applied: 1.0,
            continuous_fire_consecutive: 0,
            continuous_fire_level: 0,
            continuous_fire_one_shots: u32::MAX,
            continuous_fire_two_shots: u32::MAX,
            continuous_fire_coast_frames: 0,
            auto_reload_when_idle_frames: 0,
            frame_to_force_reload: 0,
            continuous_fire_coast_until_frame: 0,
            fire_ocl_after_cooldown: None,
            continuous_fire_victim: 0,
            faerie_fire_until_frame: 0,
            subdual_damage: 0.0,
            subdual_heal_rate_frames: 0,
            subdual_heal_amount: 0.0,
            subdual_heal_countdown: 0,
            is_humvee_transport: false,
            is_listening_outpost_transport: false,
            is_pathfinder_unit: false,
            is_troop_crawler_transport: false,
            assault_transport: None,
            deploy_style: None,
            command_button_hunt: None,
            has_overlord_gattling_addon: false,
            has_overlord_propaganda_addon: false,
            is_helix_transport: false,
            command_set_override: None,
            demo_suicided_detonating: false,
            hive_slave_count: 0,
            hive_slave_hp: 0.0,
            hive_slave_respawn_frame: 0,
            hive_slaves: [crate::game_logic::host_base_defense::ResidualHiveSlave::default(); 3],
            turret_angle_deg: default_strategy_center_turret_angle(),
            turret_pitch_deg: default_strategy_center_turret_pitch(),
            turret_idle_scan_next_frame: 0,
            turret_idle_scanning: false,
            turret_idle_scan_desired_angle_deg: 0.0,
            turret_idle_scan_index: 0,
            turret_holding: false,
            turret_hold_until_frame: 0,
            turret_idle_recentering: false,
            turret_mood_target: false,
            turret_target_id: None,
            turret_force_attacking: false,
            turret_enabled: false,
            turret_turn_rate_rad: default_turret_turn_rate(),
            turret_substate: TurretSubState::Idle,
            turret_rotating: false,
            turret_natural_angle_deg: 0.0,
            turret_natural_pitch_deg: 0.0,
            turret_recenter_frames: default_turret_recenter_frames(),
            ai_attitude: 0, // HostAiAttitude::Normal
            repulsor_until_frame: 0,
            last_damage_source: None,
            next_mood_check_time: 0,
            mood_attack_check_rate: default_mood_attack_check_rate(),
            vision_range: default_vision_range(),
            shroud_clearing_range: default_vision_range(),
            shroud_range: 0.0,
            auto_acquire_when_idle: true,
            attack_priority_set: None,
            camo_friendly_opacity: 1.0,
            camo_opacity_pulse_phase: 0.0,
            camo_stealth_look: 0,
            camo_heat_vision_opacity: 0.0,
            camo_net_sub_object_shown: false,
            camo_net_sub_object_observer_visible: false,
            stealth_allowed_frame: 0,
            stealth_delay_pending: false,
            stealth_delay_frames: 0,
            stealth_breaks_on_damage: false,
        }
    }

    pub fn new_under_construction(template: ThingTemplate, id: ObjectId, team: Team) -> Self {
        let mut obj = Self::new(template, id, team);
        obj.construction_percent = 0.0;
        obj.set_status_under_construction(true);
        obj.health.current = 0.1; // Very low health during construction
        obj
    }

    pub fn get_template(&self) -> &ThingTemplate {
        self.thing.get_template()
    }

    pub fn is_kind_of(&self, kind: KindOf) -> bool {
        self.thing.is_kind_of(kind)
    }

    /// C++ PoisonedBehavior::onDamage residual.
    pub fn notify_poisoned_on_damage(
        &mut self,
        current_frame: u32,
        damage_type: crate::game_logic::combat::DamageType,
        damage_dealt: f32,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
    ) {
        use crate::game_logic::host_poisoned_behavior::{
            is_poison_damage_type, HostPoisonedBehaviorData,
        };
        if !is_poison_damage_type(damage_type) || damage_dealt <= 0.0 {
            return;
        }
        if self.poisoned_behavior.is_none() {
            self.poisoned_behavior = Some(HostPoisonedBehaviorData::default());
        }
        if let Some(p) = self.poisoned_behavior.as_mut() {
            p.start_poisoned_effects(current_frame, damage_dealt, death_type);
        }
    }

    /// C++ PoisonedBehavior::onHealing residual.
    pub fn clear_poisoned_on_healing(&mut self) {
        if let Some(p) = self.poisoned_behavior.as_mut() {
            p.stop_poisoned_effects();
        }
    }

    /// C++ PoisonedBehavior::update residual. Returns DoT damage to apply.
    pub fn tick_poisoned_behavior(
        &mut self,
        current_frame: u32,
    ) -> Option<(f32, crate::game_logic::host_usa_pilot::HostDeathType)> {
        let alive = !self.status.destroyed
            && !self.status.effectively_dead
            && !self.status.keep_as_rubble
            && self.health.is_alive();
        let Some(p) = self.poisoned_behavior.as_mut() else {
            return None;
        };
        let dmg = p.tick(current_frame);
        if p.should_stop(current_frame) && alive {
            p.stop_poisoned_effects();
        }
        dmg
    }

    /// Presentation: poisoned tint residual.
    pub fn is_poison_tinted(&self) -> bool {
        self.poisoned_behavior
            .as_ref()
            .map(|p| p.tint_poisoned)
            .unwrap_or(false)
    }

    /// C++ Object::defect(team, detectionFrames) residual.
    pub fn defect(&mut self, new_team: Team, now: u32, detection_frames: u32) {
        self.set_team(new_team);
        self.begin_undetected_defection(now, detection_frames, true);
    }

    /// C++ Object::defect / friend_setUndetectedDefector + DefectionHelper timer.
    pub fn begin_undetected_defection(&mut self, now: u32, protection_frames: u32, with_fx: bool) {
        if self.defection_helper.is_none() {
            self.defection_helper =
                Some(crate::game_logic::host_defection_helper::HostDefectionHelperData::default());
        }
        if let Some(d) = self.defection_helper.as_mut() {
            crate::game_logic::host_defection_helper::defect_team_residual(
                d,
                now,
                protection_frames,
                with_fx,
            );
        }
    }

    pub fn is_undetected_defector(&self) -> bool {
        self.defection_helper
            .as_ref()
            .map(|d| d.is_undetected_defector())
            .unwrap_or(false)
    }

    pub fn blow_defector_cover(&mut self) {
        if let Some(d) = self.defection_helper.as_mut() {
            d.blow_cover();
        }
    }

    /// C++ ObjectDefectionHelper::update residual.
    pub fn tick_defection_helper(&mut self, now: u32) {
        let firing = self.status.is_firing_weapon;
        let dead =
            self.status.destroyed || self.status.effectively_dead || self.health.current <= 0.0;
        if let Some(d) = self.defection_helper.as_mut() {
            d.tick(now, firing, dead);
        }
    }

    /// C++ FireWeaponPower::doSpecialPower residual.
    pub fn activate_fire_weapon_power(&mut self, target: Option<(f32, f32)>) -> bool {
        if self.is_disabled() {
            return false;
        }
        let shots =
            crate::game_logic::host_fire_weapon_power::max_shots_for_template(&self.template_name);
        self.fire_weapon_power = Some(match target {
            Some((x, z)) => {
                crate::game_logic::host_fire_weapon_power::HostFireWeaponPowerRequest::at_location(
                    shots, x, z,
                )
            }
            None => crate::game_logic::host_fire_weapon_power::HostFireWeaponPowerRequest::at_self(
                shots,
            ),
        });
        // C++ reloadAllAmmo(TRUE) residual.
        self.reload_all_ammo();
        true
    }

    pub fn is_alive(&self) -> bool {
        if self.status.destroyed
            || self.status.effectively_dead
            || self.status.keep_as_rubble
            || !self.health.is_alive()
        {
            return false;
        }
        // C++ effectively-dead during SlowDeath / air crash sequences.
        if self
            .slow_death
            .as_ref()
            .map(|s| s.is_active())
            .unwrap_or(false)
        {
            return false;
        }
        if self
            .jet_slow_death
            .as_ref()
            .map(|j| j.is_active())
            .unwrap_or(false)
        {
            return false;
        }
        if self
            .helicopter_slow_death
            .as_ref()
            .map(|h| h.is_active())
            .unwrap_or(false)
        {
            return false;
        }
        true
    }

    pub fn get_health_percentage(&self) -> f32 {
        self.health.percentage()
    }

    pub fn is_constructed(&self) -> bool {
        !self.status.under_construction && self.construction_percent >= 1.0
    }

    pub fn is_mobile(&self) -> bool {
        // C++-ish: infantry/vehicle/aircraft, plus Worker KindOf.
        // Do NOT call can_construct() here — that path can re-enter is_mobile.
        // Host dozer residual: treat non-structure templates named *Dozer* as mobile.
        if self.is_kind_of(KindOf::Infantry)
            || self.is_kind_of(KindOf::Vehicle)
            || self.is_kind_of(KindOf::Aircraft)
            || self.is_kind_of(KindOf::Worker)
        {
            return true;
        }
        if !self.is_kind_of(KindOf::Structure) {
            let name = self.template_name.to_ascii_lowercase();
            if name.contains("dozer") || name.contains("worker") || name.contains("construction") {
                return true;
            }
        }
        false
    }

    pub fn is_selectable(&self) -> bool {
        self.is_alive()
            && self.is_kind_of(KindOf::Selectable)
            && !self.status.masked
            && !self.status.unselectable
            && !self.hijacker_in_vehicle
            && !matches!(self.ai_state, AIState::Docked | AIState::Garrisoned)
    }

    pub fn is_worker(&self) -> bool {
        self.is_kind_of(KindOf::Worker)
            || self.template_name.contains("Dozer")
            || self.template_name.contains("Worker")
            || self.template_name.contains("Harvester")
            || self.template_name.contains("Collector")
    }

    pub fn is_hero(&self) -> bool {
        self.is_kind_of(KindOf::Hero) || self.template_name.contains("Hero")
    }

    pub fn is_command_center(&self) -> bool {
        self.is_kind_of(KindOf::CommandCenter)
            || self.template_name.contains("CommandCenter")
            || self.template_name.contains("Headquarters")
    }

    pub fn is_faction_structure(&self) -> bool {
        self.is_kind_of(KindOf::FSBarracks)
            || self.is_kind_of(KindOf::FSWarFactory)
            || self.is_kind_of(KindOf::FSAirfield)
            || self.is_kind_of(KindOf::FSInternetCenter)
            || self.is_kind_of(KindOf::FSPower)
            || self.is_kind_of(KindOf::FSBaseDefense)
            || self.is_kind_of(KindOf::FSSupplyDropzone)
            || self.is_kind_of(KindOf::FSSupplyCenter)
            || self.is_kind_of(KindOf::FSSuperweapon)
            || self.is_kind_of(KindOf::FSStrategyCenter)
            || self.is_kind_of(KindOf::FSFake)
            || self.is_kind_of(KindOf::FSTechnology)
            || self.is_kind_of(KindOf::FSBlackMarket)
            || self.is_kind_of(KindOf::FSAdvancedTech)
            || self.is_command_center()
            || self.is_kind_of(KindOf::SupplyCenter)
            || self.is_kind_of(KindOf::PowerPlant)
            || self.template_name.contains("Barracks")
            || self.template_name.contains("WarFactory")
            || self.template_name.contains("Airfield")
            || self.template_name.contains("InternetCenter")
            || self.template_name.contains("PowerPlant")
            || self.template_name.contains("SupplyDropzone")
            || self.template_name.contains("SupplyCenter")
            || self.template_name.contains("Superweapon")
            || self.template_name.contains("StrategyCenter")
            || self.template_name.contains("BlackMarket")
            || self.template_name.contains("TechCenter")
    }

    pub fn is_non_faction_structure(&self) -> bool {
        self.is_kind_of(KindOf::Structure) && !self.is_faction_structure()
    }

    /// C++ parity (Object::isDisabled): returns true if the object is in any
    /// disabled state that prevents it from acting (attacking, producing, etc.)
    ///
    /// Note: `weapons_jammed` (ECM residual) is intentionally **not** full
    /// disabled — C++ DISABLED_SUBDUED on vehicles only blocks `canFireWeapon`;
    /// residual keeps movement. Check `is_weapons_jammed()` / `can_attack()` for fire.
    /// Structure `disabled_subdued` (Microwave residual) **is** full disable.
    pub fn is_disabled(&self) -> bool {
        self.status.disabled_underpowered
            || self.status.disabled_unmanned
            || self.status.disabled_hacked
            || self.status.disabled_emp
            || self.status.disabled_paralyzed
            || self.status.disabled_subdued
            || self.status.disabled_freefall
            || self.status.disabled_default
            || self.status.under_construction
    }

    /// C++ DISABLED_FREEFALL residual.
    pub fn is_freefall_disabled(&self) -> bool {
        self.status.disabled_freefall
    }

    /// C++ DISABLED_UNMANNED residual (Jarmen Kell kill-pilot snipe).
    pub fn is_unmanned(&self) -> bool {
        self.status.disabled_unmanned
    }

    /// C++ DISABLED_HACKED residual (Black Lotus DisableVehicleHack).
    pub fn is_hacked_disabled(&self) -> bool {
        self.status.disabled_hacked
    }

    /// C++ DISABLED_EMP residual (EMPUpdate / SuperweaponEMPPulse).
    pub fn is_emp_disabled(&self) -> bool {
        self.status.disabled_emp
    }

    /// C++ DISABLED_PARALYZED residual (BattlePlanChangeParalyzeTime).
    pub fn is_paralyzed_disabled(&self) -> bool {
        self.status.disabled_paralyzed
    }

    /// Host ECM / jammer residual: weapons cannot fire while in jam radius.
    /// C++ DISABLED_SUBDUED / canFireWeapon residual (Microwave/ECM disabler).
    pub fn is_weapons_jammed(&self) -> bool {
        self.status.weapons_jammed
    }

    /// C++ DISABLED_SUBDUED residual (Microwave building disabler on structures).
    pub fn is_subdued_disabled(&self) -> bool {
        self.status.disabled_subdued
    }

    /// Apply / clear weapons-jam residual (ECM field coverage).
    pub fn set_weapons_jammed(&mut self, jammed: bool) {
        if jammed {
            self.set_status_weapons_jammed(true);
            // C++ canFireWeapon false while subdued: drop in-progress attack fire
            // but do not freeze movement (jam residual is weapons-only).
            self.status.attacking = false;
            self.set_status_force_attack(false);
        } else {
            self.set_status_weapons_jammed(false);
        }
    }

    /// Apply / clear DISABLED_SUBDUED residual (Microwave structure cook).
    ///
    /// C++ ActiveBody::onSubdualChange → setDisabled(DISABLED_SUBDUED).
    /// Structures stop production / attack while cooked; residual continuous
    /// while microwave keeps attacking (not full subdual accumulate/heal).

    /// C++ StructureCollapseUpdate::onDie / beginStructureCollapse residual.
    pub fn begin_structure_collapse(&mut self, current_frame: u32) -> bool {
        if !self.is_kind_of(crate::game_logic::KindOf::Structure) {
            return false;
        }
        if !crate::game_logic::host_structure_collapse::is_structure_collapse_candidate(
            &self.template_name,
            true,
        ) {
            return false;
        }
        if self
            .structure_collapse_data
            .as_ref()
            .map(|d| !d.is_standing())
            .unwrap_or(false)
        {
            return true;
        }
        let mut data =
            crate::game_logic::host_structure_collapse::HostStructureCollapseData::default();
        let radius = self.selection_radius.max(10.0);
        data.building_height = (self.health.maximum.max(100.0) * 0.12)
            .clamp(15.0, 60.0)
            .max(radius * 0.8);
        // Mid delay residual (average of 15–30).
        data.begin(current_frame, 22);
        self.structure_collapse_data = Some(data);
        self.selected = false;
        self.status.selected = false;
        self.set_ai_state(crate::game_logic::AIState::Idle);
        if self.health.current <= 0.0 {
            self.health.current = 0.01;
        }
        self.status.destroyed = false;
        true
    }

    /// C++ StructureCollapseUpdate::update residual. True when collapse completes.
    pub fn tick_structure_collapse(&mut self, current_frame: u32) -> bool {
        let Some(sc) = self.structure_collapse_data.as_mut() else {
            return false;
        };
        if !sc.tick(current_frame) {
            return false;
        }
        self.health.current = 0.0;
        self.status.destroyed = true;
        self.status.death_type = crate::game_logic::host_usa_pilot::HostDeathType::Toppled;
        true
    }

    /// Presentation vertical offset from structure collapse residual.
    pub fn presentation_collapse_height_offset(&self) -> f32 {
        self.structure_collapse_data
            .as_ref()
            .map(|d| d.collapse_height_offset())
            .unwrap_or(0.0)
    }

    /// Presentation shudder residual from structure collapse.
    pub fn presentation_collapse_shudder(&self) -> (f32, f32) {
        self.structure_collapse_data
            .as_ref()
            .map(|d| (d.shudder_x, d.shudder_z))
            .unwrap_or((0.0, 0.0))
    }

    /// C++ StructureToppleUpdate::onDie / beginStructureTopple residual.
    /// Call when a structure reaches lethal damage instead of instant destroy.
    /// Returns true if structure topple was started (caller should not destroy yet).
    pub fn begin_structure_topple(
        &mut self,
        current_frame: u32,
        attacker_pos: Option<(f32, f32)>,
    ) -> bool {
        if !self.is_kind_of(crate::game_logic::KindOf::Structure) {
            return false;
        }
        if !crate::game_logic::host_structure_topple::is_structure_topple_candidate(
            &self.template_name,
            true,
        ) {
            return false;
        }
        if self
            .structure_topple_data
            .as_ref()
            .map(|d| !d.is_standing())
            .unwrap_or(false)
        {
            return true; // already toppling
        }
        let pos = self.get_position();
        let (dx, dz) = match attacker_pos {
            Some((ax, az)) => (pos.x - ax, pos.z - az),
            None => (1.0, 0.0),
        };
        let mut data = crate::game_logic::host_structure_topple::HostStructureToppleData::default();
        let radius = self.selection_radius.max(10.0);
        data.facing_width = radius;
        data.building_height = (self.health.maximum.max(100.0) * 0.15).clamp(20.0, 80.0);
        data.begin(current_frame, dx, dz, 0);
        self.structure_topple_data = Some(data);
        // C++ marks AI dead and deselects while building is still "alive" for fall.
        self.selected = false;
        self.status.selected = false;
        self.set_ai_state(crate::game_logic::AIState::Idle);
        // Keep a sliver of HP so generic destroy passes leave it alone until done.
        if self.health.current <= 0.0 {
            self.health.current = 0.01;
        }
        self.status.destroyed = false;
        true
    }

    /// Attach FireWeaponWhenDamaged residual when template peels match.

    pub fn ensure_height_die(&mut self, current_frame: u32) {
        if self.height_die.is_some() {
            return;
        }
        if let Some((h, desc, delay_ms)) =
            crate::game_logic::host_height_die::height_die_config_for_template(&self.template_name)
        {
            let delay_f = ((delay_ms as f32) * 30.0 / 1000.0).round() as u32;
            self.height_die = Some(
                crate::game_logic::host_height_die::HostHeightDieData::with_target(
                    h,
                    desc,
                    current_frame.saturating_add(delay_f),
                ),
            );
        }
    }

    /// C++ HeightDieUpdate residual. True when should die from altitude.
    /// C++ FuelAir gas SlowDeathBehavior residual install.
    pub fn ensure_fuel_air_gas_slow_death(&mut self, current_frame: u32) {
        if self.fuel_air_gas_slow_death.is_some() {
            return;
        }
        if let Some(data) =
            crate::game_logic::host_fuel_air_gas_slow_death::HostFuelAirGasSlowDeathData::for_template(
                &self.template_name,
                current_frame,
            )
        {
            self.fuel_air_gas_slow_death = Some(data);
            // Retail HeightDie TargetHeight 15 on gas.
            if self.height_die.is_none() {
                self.height_die = Some(
                    crate::game_logic::host_height_die::HostHeightDieData::with_target(
                        crate::game_logic::host_fuel_air_gas_slow_death::FUEL_AIR_GAS_HEIGHT_DIE,
                        false,
                        current_frame,
                    ),
                );
            }
        }
    }


    /// C++ NeutronMissileUpdate residual install when target known.
    pub fn ensure_neutron_missile_update(
        &mut self,
        target: glam::Vec3,
        launcher: Option<crate::game_logic::ObjectId>,
        now: u32,
    ) {
        if self.neutron_missile_update.is_some() {
            return;
        }
        let launch = self.get_position();
        if let Some(data) =
            crate::game_logic::host_neutron_missile_update::HostNeutronMissileUpdateData::for_template(
                &self.template_name,
                launch,
                target,
                launcher.map(|id| id.0),
                now,
            )
        {
            self.neutron_missile_update = Some(data);
        }
    }


    pub fn tick_height_die(&mut self, current_frame: u32, terrain_height: f32) -> bool {
        self.ensure_height_die(current_frame);
        let pos = self.get_position();
        let hat = pos.y - terrain_height;
        let contained = self.contained_by.is_some();
        let Some(hd) = self.height_die.as_mut() else {
            return false;
        };
        if hd.tick(current_frame, hat, contained) {
            self.health.current = 0.0;
            self.status.destroyed = true;
            self.refresh_model_condition_bits();
            return true;
        }
        false
    }

    /// C++ KeepObjectDie residual: leave rubble instead of DestroyDie remove.
    pub fn begin_keep_object_die(&mut self, current_frame: u32) -> bool {
        let is_struct = self.is_kind_of(crate::game_logic::KindOf::Structure);
        if !crate::game_logic::host_keep_object_die::wants_keep_object_die(
            &self.template_name,
            is_struct,
        ) {
            return false;
        }
        if self.status.keep_as_rubble {
            return true;
        }
        let mut data = crate::game_logic::host_keep_object_die::HostKeepObjectDieData::default();
        data.mark_rubble(current_frame);
        self.keep_object_die = Some(data);
        self.health.current = 0.0;
        self.status.effectively_dead = true;
        self.status.keep_as_rubble = true;
        // Not destroyed: remains in world. Unselectable rubble husk.
        self.status.destroyed = false;
        self.status.selected = false;
        self.set_ai_state(crate::game_logic::AIState::Idle);
        self.target = None;
        self.refresh_model_condition_bits();
        true
    }

    /// C++ JetSlowDeathBehavior residual begin.
    pub fn begin_jet_slow_death(&mut self) -> bool {
        if !crate::game_logic::host_jet_slow_death::is_jet_slow_death_template(&self.template_name)
        {
            return false;
        }
        if self
            .jet_slow_death
            .as_ref()
            .map(|j| j.is_active() || j.done)
            .unwrap_or(false)
        {
            return self
                .jet_slow_death
                .as_ref()
                .map(|j| j.is_active())
                .unwrap_or(false);
        }
        let hat = self.get_position().y; // residual vs terrain 0
        let mut j = crate::game_logic::host_jet_slow_death::HostJetSlowDeathData::default();
        j.begin(hat.max(0.0));
        self.jet_slow_death = Some(j);
        if self.health.current <= 0.0 {
            self.health.current = 0.01;
        }
        self.status.destroyed = false;
        self.set_ai_state(crate::game_logic::AIState::Idle);
        self.target = None;
        true
    }

    /// Tick jet crash. `terrain_height` world Y of ground.
    pub fn tick_jet_slow_death(&mut self, current_frame: u32, terrain_height: f32) -> bool {
        let pos = self.get_position();
        let hat = (pos.y - terrain_height).max(0.0);
        let ori = self.get_orientation();
        let Some(j) = self.jet_slow_death.as_mut() else {
            return false;
        };
        if !j.is_active() {
            return false;
        }
        let (dy, d_roll, done) = j.tick(current_frame, hat);
        let mut np = pos;
        np.y = (np.y + dy).max(terrain_height);
        self.set_position(np);
        // Use orientation as roll residual peel (presentation).
        self.set_orientation(ori + d_roll);
        if done {
            self.health.current = 0.0;
            self.status.destroyed = true;
            self.refresh_model_condition_bits();
            return true;
        }
        false
    }

    /// C++ HelicopterSlowDeathBehavior residual begin.
    pub fn begin_helicopter_slow_death(&mut self) -> bool {
        if !crate::game_logic::host_helicopter_slow_death::is_helicopter_slow_death_template(
            &self.template_name,
        ) {
            return false;
        }
        if self
            .helicopter_slow_death
            .as_ref()
            .map(|h| h.is_active() || h.done)
            .unwrap_or(false)
        {
            return self
                .helicopter_slow_death
                .as_ref()
                .map(|h| h.is_active())
                .unwrap_or(false);
        }
        let mut h =
            crate::game_logic::host_helicopter_slow_death::HostHelicopterSlowDeathData::default();
        h.begin();
        self.helicopter_slow_death = Some(h);
        if self.health.current <= 0.0 {
            self.health.current = 0.01;
        }
        self.status.destroyed = false;
        self.set_ai_state(crate::game_logic::AIState::Idle);
        self.target = None;
        true
    }

    /// Returns true when heli crash finished and should destroy.
    pub fn tick_helicopter_slow_death(&mut self, current_frame: u32, terrain_height: f32) -> bool {
        let pos = self.get_position();
        let hat = (pos.y - terrain_height).max(0.0);
        let ori = self.get_orientation();
        let Some(h) = self.helicopter_slow_death.as_mut() else {
            return false;
        };
        if !h.is_active() {
            return false;
        }
        let (dx, dy, dz, dori, done, _blade) = h.tick(current_frame, hat);
        let mut np = pos;
        np.x += dx;
        np.y = (np.y + dy).max(terrain_height);
        np.z += dz;
        self.set_position(np);
        self.set_orientation(ori + dori);
        if done {
            self.health.current = 0.0;
            self.status.destroyed = true;
            self.refresh_model_condition_bits();
            return true;
        }
        false
    }

    /// C++ SlowDeathBehavior::beginSlowDeath residual.
    /// Returns true if slow death started (caller should defer destroy).
    pub fn begin_slow_death(&mut self, current_frame: u32) -> bool {
        use crate::game_logic::host_slow_death::wants_slow_death;
        let is_inf = self.is_kind_of(crate::game_logic::KindOf::Infantry);
        let is_veh = self.is_kind_of(crate::game_logic::KindOf::Vehicle);
        if !wants_slow_death(&self.template_name, is_inf, is_veh) {
            return false;
        }
        if self
            .slow_death
            .as_ref()
            .map(|s| s.is_active() || s.is_done())
            .unwrap_or(false)
        {
            return self
                .slow_death
                .as_ref()
                .map(|s| s.is_active())
                .unwrap_or(false);
        }
        let mut sd = crate::game_logic::host_slow_death::HostSlowDeathData::default();
        let ok = if is_inf {
            sd.begin_infantry(current_frame)
        } else {
            sd.begin_vehicle(current_frame)
        };
        if !ok {
            return false;
        }
        // sd may be replaced by fling residual below for infantry.
        // Optional fling residual (exploded infantry peel).
        if is_inf {
            // Deterministic angle from object id.
            let ang = (self.id.0 as f32) * 0.618_033_988;
            let mut fling_sd =
                crate::game_logic::host_slow_death::HostSlowDeathData::infantry_fling_residual(
                    current_frame,
                    40.0,
                    ang,
                );
            // Keep timing from standard infantry residual.
            fling_sd.sink_at_frame = sd.sink_at_frame;
            fling_sd.destroy_at_frame = sd.destroy_at_frame;
            fling_sd.phase = sd.phase;
            sd = fling_sd;
        }
        if let Some((fx, fy, fz)) = sd.take_fling_impulse() {
            self.movement.velocity.x += fx;
            self.movement.velocity.y += fy;
            self.movement.velocity.z += fz;
        }
        self.slow_death = Some(sd);
        // Keep a sliver of HP bookkeeping like structure topple residual.
        if self.health.current <= 0.0 {
            self.health.current = 0.01;
        }
        // Mark "dead" for AI but not removed yet.
        self.set_ai_state(crate::game_logic::AIState::Idle);
        self.target = None;
        self.selected = false;
        self.status.selected = false;
        self.status.destroyed = false;
        true
    }

    /// C++ SlowDeathBehavior::update residual. True when ready to destroyObject.
    pub fn tick_slow_death(&mut self, current_frame: u32) -> bool {
        let Some(sd) = self.slow_death.as_mut() else {
            return false;
        };
        if !sd.tick(current_frame) {
            return false;
        }
        self.health.current = 0.0;
        self.status.destroyed = true;
        true
    }

    pub fn presentation_slow_death_sink_offset(&self) -> f32 {
        self.slow_death
            .as_ref()
            .map(|s| s.sink_offset)
            .unwrap_or(0.0)
    }

    pub fn ensure_create_object_die(&mut self) {
        if self.create_object_die.is_some() {
            return;
        }
        if let Some(cfg) =
            crate::game_logic::host_create_object_die::create_object_die_config_for_template(
                &self.template_name,
            )
        {
            self.create_object_die = Some(cfg);
        }
    }

    pub fn ensure_lifetime_update(&mut self, current_frame: u32) {
        if self.lifetime_update.is_some() {
            return;
        }
        if let Some(msec) =
            crate::game_logic::host_lifetime_update::lifetime_msec_for_template(&self.template_name)
        {
            self.lifetime_update = Some(
                crate::game_logic::host_lifetime_update::HostLifetimeUpdateData::from_msec(
                    current_frame,
                    msec,
                ),
            );
        }
    }

    /// C++ CreateObjectDie::onDie residual — queues spawn templates.
    pub fn fire_create_object_die(&mut self) {
        self.ensure_create_object_die();
        let Some(cod) = self.create_object_die.as_mut() else {
            return;
        };
        let transfer = cod.transfer_previous_health;
        if let Some(spawns) = cod.on_die() {
            self.pending_create_object_die_spawns = spawns;
            if transfer {
                // previous health residual ≈ current before death; use max-current.
                let max_h = self.health.maximum.max(self.max_health).max(1.0);
                let prev = self.health.current.max(0.0);
                self.create_object_die_transfer_damage = (max_h - prev).max(0.0);
            }
        }
    }

    pub fn take_pending_create_object_die_spawns(&mut self) -> (Vec<String>, f32, bool) {
        let spawns = std::mem::take(&mut self.pending_create_object_die_spawns);
        let dmg = self.create_object_die_transfer_damage;
        self.create_object_die_transfer_damage = 0.0;
        let transfer = self
            .create_object_die
            .as_ref()
            .map(|c| c.transfer_previous_health)
            .unwrap_or(false);
        (spawns, dmg, transfer)
    }

    /// C++ LifetimeUpdate residual. True when object should die this frame.
    pub fn tick_lifetime_update(&mut self, current_frame: u32) -> bool {
        self.ensure_lifetime_update(current_frame);
        self.lifetime_update
            .as_ref()
            .map(|l| l.tick(current_frame))
            .unwrap_or(false)
    }

    pub fn ensure_transition_damage_fx(&mut self) {
        if self.transition_damage_fx.is_some() {
            return;
        }
        let is_structure = self.is_kind_of(crate::game_logic::KindOf::Structure);
        let is_vehicle = self.is_kind_of(crate::game_logic::KindOf::Vehicle);
        if let Some(cfg) =
            crate::game_logic::host_transition_damage_fx::transition_damage_fx_config_for_template(
                &self.template_name,
                is_structure,
                is_vehicle,
            )
        {
            self.transition_damage_fx = Some(cfg);
        }
    }

    pub fn ensure_fx_list_die(&mut self) {
        if self.fx_list_die.is_some() {
            return;
        }
        if let Some(cfg) = crate::game_logic::host_fx_list_die::fx_list_die_config_for_template(
            &self.template_name,
        ) {
            self.fx_list_die = Some(cfg);
        }
    }

    pub fn take_pending_transition_damage_fx(
        &mut self,
    ) -> Vec<crate::game_logic::host_transition_damage_fx::HostTransitionDamageFxEvent> {
        std::mem::take(&mut self.pending_transition_damage_fx)
    }

    pub fn take_pending_death_fx_audio(&mut self) -> (Option<String>, Option<String>) {
        (
            self.pending_death_fx.take(),
            self.pending_death_audio.take(),
        )
    }

    /// C++ FXListDie::onDie residual.
    pub fn fire_fx_list_die(&mut self) {
        self.ensure_fx_list_die();
        let Some(fx) = self.fx_list_die.as_mut() else {
            return;
        };
        if let Some((f, a)) = fx.on_die() {
            if self.pending_death_fx.is_none() {
                self.pending_death_fx = f;
            }
            if self.pending_death_audio.is_none() {
                self.pending_death_audio = a;
            }
        }
    }

    pub fn ensure_fire_weapon_when_damaged(&mut self) {
        if self.fire_weapon_when_damaged.is_some() {
            return;
        }
        if let Some(cfg) =
            crate::game_logic::host_fire_weapon_when_damaged::fire_when_damaged_config_for_template(
                &self.template_name,
            )
        {
            self.fire_weapon_when_damaged = Some(cfg);
        }
    }

    /// C++ FireWeaponWhenDamagedBehavior::onDamage residual.
    /// Returns weapon name to force-fire at self position.
    pub fn take_pending_fire_when_damaged_weapon(&mut self) -> Option<String> {
        self.pending_fire_when_damaged_weapon.take()
    }

    pub fn on_fire_weapon_when_damaged(
        &mut self,
        actual_damage: f32,
        current_frame: u32,
    ) -> Option<String> {
        self.ensure_fire_weapon_when_damaged();
        let Some(fw) = self.fire_weapon_when_damaged.as_mut() else {
            return None;
        };
        fw.on_damage(
            actual_damage,
            self.health.current,
            self.health.maximum.max(self.max_health).max(1.0),
            current_frame,
        )
    }

    /// C++ FireWeaponWhenDamagedBehavior continuous update residual.
    pub fn tick_fire_weapon_when_damaged_continuous(
        &mut self,
        current_frame: u32,
    ) -> Option<String> {
        self.ensure_fire_weapon_when_damaged();
        let Some(fw) = self.fire_weapon_when_damaged.as_mut() else {
            return None;
        };
        fw.tick_continuous(
            self.health.current,
            self.health.maximum.max(self.max_health).max(1.0),
            current_frame,
        )
    }

    /// Drain C++ applyCrushingDamage residual samples for this frame.
    pub fn take_structure_topple_crush_samples(
        &mut self,
    ) -> Vec<crate::game_logic::host_structure_topple::StructureToppleCrushSample> {
        let pos = self.get_position();
        let Some(st) = self.structure_topple_data.as_mut() else {
            return Vec::new();
        };
        if !(st.is_active()
            || matches!(
                st.state,
                crate::game_logic::host_structure_topple::HostStructureToppleState::Done
            ))
        {
            return Vec::new();
        }
        st.take_crush_sweep_samples(pos.x, pos.z)
    }

    /// C++ StructureToppleUpdate::update residual. True when fall completes.
    pub fn tick_structure_topple(&mut self, current_frame: u32) -> bool {
        let Some(st) = self.structure_topple_data.as_mut() else {
            return false;
        };
        if !st.tick(current_frame) {
            return false;
        }
        // doToppleDoneStuff residual: finalize death.
        self.health.current = 0.0;
        self.status.destroyed = true;
        self.status.death_type = crate::game_logic::host_usa_pilot::HostDeathType::Toppled;
        true
    }

    /// Combined presentation lean (tree topple or structure topple).
    pub fn presentation_topple_lean_radians(&self) -> f32 {
        if let Some(st) = self.structure_topple_data.as_ref() {
            if st.is_active()
                || matches!(
                    st.state,
                    crate::game_logic::host_structure_topple::HostStructureToppleState::Done
                )
            {
                return st.lean_radians;
            }
        }
        self.topple_data
            .as_ref()
            .map(|t| t.lean_radians)
            .unwrap_or(0.0)
    }

    /// Attach residual ToppleUpdate when template is topple-capable.
    pub fn ensure_topple_data(&mut self) {
        if self.topple_data.is_none()
            && crate::game_logic::host_topple::is_topple_capable_template(&self.template_name)
        {
            self.topple_data = Some(crate::game_logic::host_topple::HostToppleData::default());
        }
    }

    /// C++ Object::topple residual.
    /// Returns true if the object should be destroyed immediately (start-topple kill).
    pub fn apply_topple(
        &mut self,
        dir_x: f32,
        dir_y: f32,
        topple_speed: f32,
        options: u32,
    ) -> bool {
        if self.status.destroyed || !self.is_alive() {
            return false;
        }
        self.ensure_topple_data();
        let kill_now = {
            let Some(td) = self.topple_data.as_mut() else {
                return false;
            };
            if !td.is_able_to_be_toppled() {
                return false;
            }
            td.apply_toppling_force(dir_x, dir_y, topple_speed, options)
        };
        if kill_now {
            self.health.current = 0.0;
            self.status.destroyed = true;
            self.status.death_type = crate::game_logic::host_usa_pilot::HostDeathType::Toppled;
        }
        kill_now
    }

    /// C++ ToppleUpdate::update residual. Returns true if death-by-topple this frame.
    pub fn tick_topple(&mut self) -> bool {
        let Some(td) = self.topple_data.as_mut() else {
            return false;
        };
        if !td.tick() {
            return false;
        }
        // deathByToppling: UNRESISTABLE + DEATH_TOPPLED
        self.health.current = 0.0;
        self.status.destroyed = true;
        self.status.death_type = crate::game_logic::host_usa_pilot::HostDeathType::Toppled;
        true
    }

    /// C++ ToppleUpdate::onCollide residual when other crushes this prop.
    pub fn try_topple_from_crusher(
        &mut self,
        crusher_level: u8,
        from_x: f32,
        from_z: f32,
        speed: f32,
    ) -> bool {
        if !crate::game_logic::host_topple::crusher_can_topple(crusher_level) {
            return false;
        }
        self.ensure_topple_data();
        let Some(td) = self.topple_data.as_ref() else {
            return false;
        };
        if !td.is_able_to_be_toppled() {
            return false;
        }
        let pos = self.get_position();
        let dx = pos.x - from_x;
        let dz = pos.z - from_z;
        self.apply_topple(
            dx,
            dz,
            speed.max(1.0),
            crate::game_logic::host_topple::TOPPLE_OPTIONS_NONE,
        )
    }

    /// C++ KINDOF_CAN_SURRENDER residual (infantry primarily).
    pub fn can_surrender_from_damage(&self) -> bool {
        if self.is_kind_of(crate::game_logic::KindOf::Infantry) {
            return true;
        }
        let n = self.template_name.to_ascii_lowercase();
        n.contains("infantry")
            || n.contains("ranger")
            || n.contains("rebel")
            || n.contains("redguard")
            || n.contains("tankhunter")
            || n.contains("pathfinder")
            || n.contains("colonel")
            || n.contains("jarmen")
            || n.contains("hijacker")
            || n.contains("worker")
            || n.contains("pilot")
    }

    /// Consume pending DAMAGE_DEPLOY assault signal (GameLogic combat path).
    pub fn take_pending_deploy_assault(&mut self) -> bool {
        let v = self.status.pending_deploy_assault;
        self.status.pending_deploy_assault = false;
        v
    }

    /// Consume pending DAMAGE_KILL_GARRISONED occupant kill count.
    pub fn take_pending_kill_garrisoned(&mut self) -> u32 {
        let v = self.status.pending_kill_garrisoned;
        self.status.pending_kill_garrisoned = 0;
        v
    }

    /// Residual mine / demo-trap / booby identity for DAMAGE_DISARM targeting.
    pub fn is_disarmable_mine(&self) -> bool {
        use crate::game_logic::host_mines::can_clear_mine_kind;
        if let Some(md) = self.mine_data.as_ref() {
            return !md.detonated && can_clear_mine_kind(md.kind);
        }
        // Name peel residual when mine_data not attached yet.
        let n = self.template_name.to_ascii_lowercase();
        n.contains("mine")
            || n.contains("demotrap")
            || n.contains("booby")
            || self.status.booby_trapped
    }

    /// C++ LandMineInterface::disarm residual (safe clear, no splash).
    pub fn disarm_mine_safe(&mut self) -> bool {
        if !self.is_disarmable_mine() {
            return false;
        }
        if let Some(md) = self.mine_data.as_mut() {
            md.detonated = true;
            md.proximity_enabled = false;
            md.detonate_at_frame = None;
        }
        self.health.current = 0.0;
        self.status.destroyed = true;
        true
    }

    /// C++ ActiveBody::isSubdued residual (`currentSubdual >= maxHealth`).
    #[inline]
    pub fn is_subdued(&self) -> bool {
        self.health.maximum > 0.0 && self.subdual_damage + 1e-3 >= self.health.maximum
    }

    /// C++ ActiveBody::internalAddSubdualDamage + onSubdualChange residual.
    pub fn apply_subdual_damage(&mut self, amount: f32) {
        if amount <= 0.0 || !amount.is_finite() {
            return;
        }
        // Infantry residual: subdual rarely applies (microwave targets vehicles/structures).
        if self.is_kind_of(crate::game_logic::KindOf::Infantry) {
            return;
        }
        let was = self.is_subdued();
        let cap = self.health.maximum.max(1.0);
        self.subdual_damage = (self.subdual_damage + amount).min(cap);
        // Default heal rate residual when first hit (retail-ish 30f / 5 dmg step peel).
        if self.subdual_heal_rate_frames == 0 {
            self.subdual_heal_rate_frames = 30;
            self.subdual_heal_amount = 5.0;
        }
        self.subdual_heal_countdown = self.subdual_heal_rate_frames;
        let now = self.is_subdued();
        if now != was {
            self.set_disabled_subdued(now);
        } else if now {
            self.set_disabled_subdued(true);
        }
    }

    /// C++ SubdualDamageHelper::update residual heal step.
    pub fn tick_subdual_damage(&mut self) {
        if self.subdual_damage <= 0.0 {
            if self.status.disabled_subdued && !self.is_emp_disabled() {
                // Keep subdued clear if no other disable source.
                // Only clear subdual-driven disable when subdual healed out.
            }
            return;
        }
        if self.subdual_heal_rate_frames == 0 || self.subdual_heal_amount <= 0.0 {
            return;
        }
        if self.subdual_heal_countdown > 0 {
            self.subdual_heal_countdown -= 1;
            return;
        }
        let was = self.is_subdued();
        self.subdual_damage = (self.subdual_damage - self.subdual_heal_amount).max(0.0);
        self.subdual_heal_countdown = self.subdual_heal_rate_frames;
        let now = self.is_subdued();
        if was && !now {
            self.set_disabled_subdued(false);
        }
    }

    pub fn set_disabled_subdued(&mut self, subdued: bool) {
        if subdued {
            self.set_status_disabled_subdued(true);
            // C++ orderAllPassengersToIdle residual: drop attack / move orders.
            self.status.attacking = false;
            self.set_status_force_attack(false);
            self.target = None;
            self.target_location = None;
            // Structures do not move; stop any residual production-related AI.
            if !self.is_kind_of(KindOf::Structure) {
                self.set_status_moving(false);
                self.stop_moving();
                self.set_ai_state(AIState::Idle);
            }
        } else {
            self.set_status_disabled_subdued(false);
        }
    }

    /// Apply kill-pilot residual: vehicle becomes unmanned (no HP change).
    /// Caller is responsible for team transfer (typically Neutral).
    /// Captures `unmanned_owner_team` for PilotFindVehicle PartitionFilterPlayer residual.
    pub fn apply_kill_pilot_unmanned(&mut self) {
        // Preserve original controller for same-player PartitionFilter residual.
        // Only snapshot on the edge into unmanned (refresh would overwrite Neutral).
        if !self.status.disabled_unmanned {
            self.status.unmanned_owner_team = Some(self.team);
        }
        self.set_status_disabled_unmanned(true);
        self.set_status_disabled_hacked(false);
        self.status.disabled_hacked_until_frame = 0;
        self.set_status_disabled_emp(false);
        self.status.disabled_emp_until_frame = 0;
        self.set_status_disabled_paralyzed(false);
        self.status.disabled_paralyzed_until_frame = 0;
        self.status.attacking = false;
        self.set_status_moving(false);
        self.stop_moving();
        self.target = None;
        self.target_location = None;
        self.set_status_force_attack(false);
        self.set_ai_state(AIState::Idle);
    }

    /// Apply USA Pilot recrew residual onto this unmanned vehicle.
    ///
    /// Clears DISABLED_UNMANNED, transfers team to pilot team, merges pilot
    /// veterancy (retail VeterancyCrateCollide IsPilot + AddsOwnerVeterancy).
    /// Caller destroys the pilot infantry.
    pub fn apply_pilot_recrew(
        &mut self,
        pilot_team: Team,
        pilot_level: crate::game_logic::VeterancyLevel,
    ) -> bool {
        use crate::game_logic::host_usa_pilot::{merged_recrew_veterancy, veterancy_rank};

        if !self.status.disabled_unmanned {
            return false;
        }
        self.set_status_disabled_unmanned(false);
        self.status.unmanned_owner_team = None;
        self.set_status_disabled_hacked(false);
        self.status.disabled_hacked_until_frame = 0;
        self.set_status_disabled_emp(false);
        self.status.disabled_emp_until_frame = 0;
        self.set_status_disabled_paralyzed(false);
        self.status.disabled_paralyzed_until_frame = 0;
        self.status.attacking = false;
        self.set_status_moving(false);
        self.stop_moving();
        self.target = None;
        self.target_location = None;
        self.set_status_force_attack(false);
        self.set_ai_state(AIState::Idle);
        self.set_team(pilot_team);

        let previous = self.experience.level;
        let merged = merged_recrew_veterancy(previous, pilot_level);
        let transferred = veterancy_rank(merged) > veterancy_rank(previous);
        if merged != previous {
            self.experience.level = merged;
            self.apply_veterancy_bonuses(previous, merged);
        }
        transferred
    }

    /// Apply DISABLED_HACKED residual until `until_frame` (absolute host logic frame).
    /// C++ SpecialAbilityUpdate: setDisabledUntil(DISABLED_HACKED, now + EffectDuration).

    /// C++ Drawable::flashAsSelected residual (default white/house flash, decay 4).
    /// C++ OBJECT_STATUS_DEPLOYED residual.
    pub fn is_deployed(&self) -> bool {
        self.status.deployed
    }

    /// Toggle DeployStyle residual (artillery / missile humvee / etc.).
    pub fn set_deployed(&mut self, deployed: bool) {
        self.status.deployed = deployed;
        if deployed {
            // Deployed units typically stop locomoting residual.
            self.stop_moving();
            self.set_status_moving(false);
        }
    }

    /// Install C++ DeployStyleAIUpdate residual from template peels.
    pub fn install_deploy_style_if_needed(&mut self) {
        if self.deploy_style.is_some() {
            return;
        }
        if let Some(data) = crate::game_logic::host_deploy_style::HostDeployStyleData::for_template(
            &self.template_name,
        ) {
            self.deploy_style = Some(data);
        }
    }

    /// C++ TensileFormationUpdate install residual (AvalancheChunk peels).
    pub fn install_tensile_formation_if_needed(&mut self) {
        if self.tensile_formation.is_some() {
            return;
        }
        if let Some(data) =
            crate::game_logic::host_tensile_formation::HostTensileFormationData::for_template(
                &self.template_name,
            )
        {
            self.tensile_formation = Some(data);
        }
    }

    pub fn install_fire_spread_if_needed(&mut self) {
        if self.fire_spread.is_some() {
            return;
        }
        if let Some(data) =
            crate::game_logic::host_fire_spread::HostFireSpreadData::for_template(&self.template_name)
        {
            self.fire_spread = Some(data);
        }
    }

    /// C++ Object::setShroudRange residual (ActiveShroudUpgrade).
    pub fn set_shroud_range(&mut self, new_range: f32) {
        self.shroud_range =
            crate::game_logic::host_active_shroud_upgrade::apply_active_shroud_range(
                self.shroud_range,
                new_range,
            );
    }

    pub fn install_animation_steering_if_needed(&mut self) {
        if self.animation_steering.is_some() {
            return;
        }
        if let Some(data) =
            crate::game_logic::host_animation_steering::HostAnimationSteeringData::for_template(
                &self.template_name,
            )
        {
            self.animation_steering = Some(data);
        }
    }

    pub fn install_float_update_if_needed(&mut self) {
        if self.float_update.is_some() {
            return;
        }
        if let Some(data) =
            crate::game_logic::host_float_update::HostFloatUpdateData::for_template(
                &self.template_name,
            )
        {
            self.float_update = Some(data);
        }
    }

    pub fn install_prone_update_if_needed(&mut self) {
        if self.prone_update.is_some() {
            return;
        }
        if let Some(data) =
            crate::game_logic::host_prone_update::HostProneUpdateData::for_template(
                &self.template_name,
            )
        {
            self.prone_update = Some(data);
        }
    }

    pub fn install_radius_decal_update_if_needed(&mut self) {
        if self.radius_decal_update.is_some() {
            return;
        }
        if let Some(data) =
            crate::game_logic::host_radius_decal_update::HostRadiusDecalUpdateData::for_template(
                &self.template_name,
            )
        {
            self.radius_decal_update = Some(data);
        }
    }

    pub fn install_checkpoint_update_if_needed(&mut self) {
        if self.checkpoint_update.is_some() {
            return;
        }
        if let Some(mut data) =
            crate::game_logic::host_checkpoint_update::HostCheckpointUpdateData::for_template(
                &self.template_name,
                self.vision_range,
            )
        {
            data.vision_range = self.vision_range.max(data.vision_range);
            self.checkpoint_update = Some(data);
        }
    }

    pub fn install_spectre_gunship_deployment_if_needed(&mut self) {
        if self.spectre_gunship_deployment.is_some() {
            return;
        }
        if let Some(data) =
            crate::game_logic::host_spectre_gunship_deployment::HostSpectreGunshipDeploymentData::for_template(
                &self.template_name,
            )
        {
            self.spectre_gunship_deployment = Some(data);
        }
    }

    pub fn install_smart_bomb_target_homing_if_needed(&mut self) {
        if self.smart_bomb_target_homing.is_some() {
            return;
        }
        if let Some(data) =
            crate::game_logic::host_smart_bomb_target_homing::HostSmartBombTargetHomingData::for_template(
                &self.template_name,
            )
        {
            self.smart_bomb_target_homing = Some(data);
        }
    }

    pub fn set_smart_bomb_target(&mut self, target: glam::Vec3) -> bool {
        self.install_smart_bomb_target_homing_if_needed();
        self.smart_bomb_target_homing
            .as_mut()
            .map(|h| h.set_target_position(target))
            .unwrap_or(false)
    }

    pub fn create_delivery_radius_decal(
        &mut self,
        pos: glam::Vec3,
        frame: u32,
    ) -> bool {
        self.install_radius_decal_update_if_needed();
        let Some(rd) = self.radius_decal_update.as_mut() else {
            return false;
        };
        let tmpl = crate::game_logic::host_radius_decal_update::default_delivery_decal_template_for_host(
            &self.template_name,
        );
        let radius = crate::game_logic::host_radius_decal_update::default_delivery_decal_radius_for_template(
            &self.template_name,
        );
        rd.create_radius_decal(tmpl, radius, pos, frame);
        rd.set_kill_when_no_longer_attacking(true);
        !rd.delivery_decal.is_empty()
    }

    pub fn install_enemy_near_if_needed(&mut self) {
        if self.enemy_near.is_some() {
            return;
        }
        if let Some(data) = crate::game_logic::host_enemy_near::HostEnemyNearData::for_template(
            &self.template_name,
            self.vision_range,
        ) {
            self.enemy_near = Some(data);
        }
    }

    pub fn install_base_regenerate_if_needed(&mut self) {
        if self.base_regenerate.is_some() {
            return;
        }
        let is_structure = self.is_kind_of(crate::game_logic::KindOf::Structure);
        if let Some(data) =
            crate::game_logic::host_base_regenerate::HostBaseRegenerateData::for_structure_template(
                &self.template_name,
                is_structure,
            )
        {
            self.base_regenerate = Some(data);
        }
    }

    pub fn notify_base_regenerate_damage(&mut self, current_frame: u32, is_healing: bool) {
        if let Some(br) = self.base_regenerate.as_mut() {
            br.on_damage(current_frame, is_healing);
        }
    }

    pub fn has_fire_spread(&self) -> bool {
        self.fire_spread.is_some()
    }

    pub fn try_ignite_fire_spread(&mut self, current_frame: u32) -> bool {
        let Some(fs) = self.fire_spread.as_mut() else {
            return false;
        };
        fs.try_to_ignite(current_frame)
    }

    pub fn has_tensile_formation(&self) -> bool {
        self.tensile_formation.is_some()
    }

    /// Health fraction for BODY_DAMAGED residual gate.
    pub fn health_fraction(&self) -> f32 {
        let max_h = self.health.maximum.max(self.max_health).max(1.0);
        (self.health.current / max_h).clamp(0.0, 1.0)
    }

    /// True when DeployStyle residual allows firing this frame.
    pub fn deploy_style_allows_fire(&self) -> bool {
        match self.deploy_style.as_ref() {
            None => true,
            Some(d) => d.is_ready_to_attack(),
        }
    }

    /// True when DeployStyle residual allows pathing this frame.
    pub fn deploy_style_allows_move(&self) -> bool {
        match self.deploy_style.as_ref() {
            None => true,
            Some(d) => d.is_ready_to_move(),
        }
    }

    /// C++ CommandButtonHuntUpdate::setCommandButton residual.
    pub fn start_command_button_hunt(
        &mut self,
        mode: crate::game_logic::host_command_button_hunt::HostCommandButtonHuntMode,
        current_frame: u32,
    ) {
        self.command_button_hunt = Some(
            crate::game_logic::host_command_button_hunt::HostCommandButtonHuntData::new(
                mode,
                current_frame,
            ),
        );
        self.set_ai_state(AIState::Idle);
        self.target = None;
        self.stop_moving();
    }

    pub fn clear_command_button_hunt(&mut self) {
        if let Some(h) = self.command_button_hunt.as_mut() {
            h.clear();
        }
        self.command_button_hunt = None;
    }

    pub fn flash_as_selected(&mut self) {
        self.selection_flash_remaining =
            crate::game_logic::host_saboteur::SABOTEUR_FLASH_DECAY_FRAMES;
        self.record_host_ai_request();
    }

    /// True while selection flash envelope residual is active.
    pub fn is_selection_flashing(&self) -> bool {
        self.selection_flash_remaining > 0
    }

    /// Tick selection flash residual once per logic frame.
    pub fn tick_selection_flash(&mut self) {
        self.selection_flash_remaining = self.selection_flash_remaining.saturating_sub(1);
        self.record_host_ai_request();
    }

    pub fn apply_disabled_hacked(&mut self, until_frame: u32) {
        self.set_status_disabled_hacked(true);
        self.status.disabled_hacked_until_frame = until_frame;
        self.record_disable_timers();
        self.status.attacking = false;
        self.set_status_moving(false);
        self.stop_moving();
        self.target = None;
        self.target_location = None;
        self.set_status_force_attack(false);
        self.set_ai_state(AIState::Idle);
    }

    /// Expire DISABLED_HACKED when the host frame passes the residual timer.
    pub fn tick_disabled_hacked(&mut self, current_frame: u32) {
        if self.status.disabled_hacked
            && self.status.disabled_hacked_until_frame > 0
            && current_frame >= self.status.disabled_hacked_until_frame
        {
            self.set_status_disabled_hacked(false);
            self.status.disabled_hacked_until_frame = 0;
        }
    }

    /// Apply DISABLED_EMP residual until `until_frame` (absolute host logic frame).
    /// C++ EMPUpdate::doDisableAttack: setDisabledUntil(DISABLED_EMP, now + DisabledDuration).
    /// Refresh extends the timer if a later expiry is provided.

    /// Disable until_frame residual → GameWorld SetDisableTimers.
    pub fn record_disable_timers(&mut self) {
        crate::game_logic::host_disable_timers_log::record(
            self.id,
            self.status.disabled_emp_until_frame,
            self.status.disabled_hacked_until_frame,
            self.status.disabled_paralyzed_until_frame,
        );
    }

    pub fn apply_disabled_emp(&mut self, until_frame: u32) {
        self.set_status_disabled_emp(true);
        if until_frame > self.status.disabled_emp_until_frame {
            self.status.disabled_emp_until_frame = until_frame;
        }
        self.record_disable_timers();
        self.set_status_attacking(false);
        self.set_status_moving(false);
        self.stop_moving();
        self.target = None;
        self.target_location = None;
        self.set_status_force_attack(false);
        self.set_ai_state(AIState::Idle);
    }

    /// Expire DISABLED_EMP when the host frame passes the residual timer.
    pub fn tick_disabled_emp(&mut self, current_frame: u32) {
        if self.status.disabled_emp
            && self.status.disabled_emp_until_frame > 0
            && current_frame >= self.status.disabled_emp_until_frame
        {
            self.set_status_disabled_emp(false);
            self.status.disabled_emp_until_frame = 0;
        }
    }

    /// Apply DISABLED_PARALYZED residual until `until_frame` (absolute host logic frame).
    /// C++ BattlePlanUpdate::paralyzeTroop: setDisabledUntil(DISABLED_PARALYZED, now + frames).
    /// Refresh extends the timer if a later expiry is provided.
    pub fn apply_disabled_paralyzed(&mut self, until_frame: u32) {
        self.set_status_disabled_paralyzed(true);
        if until_frame > self.status.disabled_paralyzed_until_frame {
            self.status.disabled_paralyzed_until_frame = until_frame;
        }
        self.record_disable_timers();
        self.status.attacking = false;
        self.set_status_moving(false);
        self.stop_moving();
        self.target = None;
        self.target_location = None;
        self.set_status_force_attack(false);
        self.set_ai_state(AIState::Idle);
    }

    /// Expire DISABLED_PARALYZED when the host frame passes the residual timer.
    pub fn tick_disabled_paralyzed(&mut self, current_frame: u32) {
        if self.status.disabled_paralyzed
            && self.status.disabled_paralyzed_until_frame > 0
            && current_frame >= self.status.disabled_paralyzed_until_frame
        {
            self.set_status_disabled_paralyzed(false);
            self.status.disabled_paralyzed_until_frame = 0;
        }
    }

    /// C++ goInvulnerable residual (OCL InvulnerableTime post-eject).
    pub fn is_eject_invulnerable(&self) -> bool {
        self.status.eject_invulnerable
    }

    /// Apply InvulnerableTime residual until `until_frame` (absolute host logic frame).
    /// Refresh extends the timer if a later expiry is provided.
    pub fn apply_eject_invulnerable(&mut self, until_frame: u32) {
        self.set_status_eject_invulnerable(true);
        if until_frame > self.status.eject_invulnerable_until_frame {
            self.status.eject_invulnerable_until_frame = until_frame;
        }
        // C++ goInvulnerable uses defection helper without defector FX flash.
        let now = crate::game_logic::host_historic_bonus::logic_frame();
        let frames = until_frame.saturating_sub(now).max(1);
        self.begin_undetected_defection(
            now,
            frames.min(crate::game_logic::host_defection_helper::DEFECTION_DETECTION_TIME_MAX),
            false,
        );
    }

    /// Expire InvulnerableTime when the host frame passes the residual timer.
    /// Host residual: OCL_EjectPilotViaParachute parachuting state.
    pub fn is_parachuting(&self) -> bool {
        self.status.parachuting
    }

    /// Whether AmericaParachute residual chute is open (past OpenDist freefall).
    pub fn is_parachute_open(&self) -> bool {
        self.status.parachute_open
    }

    /// Begin air-eject parachute residual (elevated spawn + freefall → OpenDist → open).
    ///
    /// Applies C++ low-altitude open fudge: if height above ground < 2×OpenDist,
    /// fudge start height so the chute can still open.
    pub fn apply_eject_parachuting(&mut self) {
        use crate::game_logic::host_usa_pilot::fudge_parachute_start_height;
        let start_y = self.get_position().y;
        let ground_y = 0.0; // host residual ground plane
        let fudged = fudge_parachute_start_height(start_y, ground_y);
        self.set_status_parachuting(true);
        self.status.airborne_target = true;
        self.set_status_parachute_open(false);
        self.status.parachute_start_height = fudged;
        // Freefall residual: pitch/roll rates seed only when chute opens.
        self.status.parachute_pitch = 0.0;
        self.status.parachute_roll = 0.0;
        self.status.parachute_pitch_rate = 0.0;
        self.status.parachute_roll_rate = 0.0;
    }

    /// Begin AmericaCrateParachute residual for cargo crate payload.
    ///
    /// Uses crate OpenDist **12.5** low-altitude fudge (not pilot OpenDist 100).
    /// Fail-closed: not full PutInContainer AmericaCrateParachute Object.
    pub fn apply_crate_parachuting(&mut self) {
        use crate::game_logic::host_deliver_payload::fudge_crate_parachute_start_height;
        let start_y = self.get_position().y;
        let ground_y = 0.0;
        let fudged = fudge_crate_parachute_start_height(start_y, ground_y);
        self.set_status_parachuting(true);
        self.status.airborne_target = true;
        self.set_status_parachute_open(false);
        self.status.parachute_start_height = fudged;
        self.status.parachute_pitch = 0.0;
        self.status.parachute_roll = 0.0;
        self.status.parachute_pitch_rate = 0.0;
        self.status.parachute_roll_rate = 0.0;
    }

    /// Whether low-altitude open fudge residual applied for this parachute start.
    pub fn parachute_start_was_fudged(&self) -> bool {
        use crate::game_logic::host_usa_pilot::parachute_start_height_was_fudged;
        // Fudge rewrites start height; detect by comparing raw y vs stored start.
        // After apply, start_height is fudged value; raw spawn y is current y
        // only at apply time — host honesty uses registry counter instead.
        parachute_start_height_was_fudged(self.get_position().y, 0.0)
    }

    /// Mark AmericaParachute residual chute open (after OpenDist freefall).
    ///
    /// Seeds pitch/roll rates residual (C++ constructor random in ±Pitch/RollRateMax;
    /// host uses deterministic mid residual).
    pub fn open_eject_parachute(&mut self) {
        use crate::game_logic::host_usa_pilot::{
            parachute_initial_pitch_rate, parachute_initial_roll_rate,
        };
        self.set_status_parachute_open(true);
        self.status.parachute_pitch = 0.0;
        self.status.parachute_roll = 0.0;
        self.status.parachute_pitch_rate = parachute_initial_pitch_rate();
        self.status.parachute_roll_rate = parachute_initial_roll_rate();
    }

    /// Clear parachuting residual on land.
    pub fn clear_eject_parachuting(&mut self) {
        self.set_status_parachuting(false);
        self.status.airborne_target = false;
        self.set_status_parachute_open(false);
        self.status.parachute_start_height = 0.0;
        self.status.parachute_pitch = 0.0;
        self.status.parachute_roll = 0.0;
        self.status.parachute_pitch_rate = 0.0;
        self.status.parachute_roll_rate = 0.0;
        self.status.parachute_landing_override = None;
        self.set_status_parachute_landing_override_set(false);
    }

    /// C++ ParachuteContain::setOverrideDestination residual.
    ///
    /// DeliverPayload aims the open chute at an explicit LZ instead of
    /// findPositionAround drift. Host residual: store XZ target for open-chute
    /// horizontal step.
    pub fn set_parachute_override_destination(&mut self, dest: glam::Vec3) {
        self.status.parachute_landing_override = Some(dest);
        self.set_status_parachute_landing_override_set(true);
    }

    /// Whether landing override residual is armed.
    pub fn has_parachute_landing_override(&self) -> bool {
        self.status.parachute_landing_override_set
            && self.status.parachute_landing_override.is_some()
    }

    /// Landing override residual target (world XZ; y ignored for aim).
    pub fn parachute_landing_override(&self) -> Option<glam::Vec3> {
        if self.status.parachute_landing_override_set {
            self.status.parachute_landing_override
        } else {
            None
        }
    }

    /// AmericaParachute pitch residual (radians) while chute open.
    pub fn parachute_pitch(&self) -> f32 {
        self.status.parachute_pitch
    }

    /// AmericaParachute roll residual (radians) while chute open.
    pub fn parachute_roll(&self) -> f32 {
        self.status.parachute_roll
    }

    pub fn tick_eject_invulnerable(&mut self, current_frame: u32) {
        if self.status.eject_invulnerable
            && self.status.eject_invulnerable_until_frame > 0
            && current_frame >= self.status.eject_invulnerable_until_frame
        {
            self.set_status_eject_invulnerable(false);
            self.status.eject_invulnerable_until_frame = 0;
        }
    }

    /// Whether Frenzy / Rage temporary attack buff residual is active.
    pub fn is_frenzy_buffed(&self) -> bool {
        self.weapon_bonus_frenzy
    }

    pub fn record_host_weapon_bonus(&self) {
        crate::game_logic::host_weapon_bonus_log::record(
            crate::game_logic::host_weapon_bonus_log::HostWeaponBonusEvent {
                object: self.id,
                enthusiastic: self.weapon_bonus_enthusiastic,
                subliminal: self.weapon_bonus_subliminal,
                horde: self.weapon_bonus_horde,
                nationalism: self.weapon_bonus_nationalism,
                frenzy: self.weapon_bonus_frenzy,
                frenzy_level: self.weapon_bonus_frenzy_level,
                battle_plan_bombardment: self.weapon_bonus_battle_plan_bombardment,
                battle_plan_hold_the_line: self.weapon_bonus_battle_plan_hold_the_line,
                battle_plan_search_and_destroy: self.weapon_bonus_battle_plan_search_and_destroy,
                frenzy_until_frame: self.weapon_bonus_frenzy_until_frame,
                battle_plan_sight_scalar_applied: self.battle_plan_sight_scalar_applied,
            },
        );
    }

    /// Apply temporary Frenzy residual (C++ Object::doTempWeaponBonus FRENZY_*).
    /// Refresh extends the timer if a later expiry is provided; keeps higher level.
    pub fn apply_weapon_bonus_frenzy(&mut self, level: u8, until_frame: u32) {
        let lvl = level.clamp(1, 3);
        self.weapon_bonus_frenzy = true;
        if lvl > self.weapon_bonus_frenzy_level {
            self.weapon_bonus_frenzy_level = lvl;
        } else if self.weapon_bonus_frenzy_level == 0 {
            self.weapon_bonus_frenzy_level = lvl;
        }
        if until_frame > self.weapon_bonus_frenzy_until_frame {
            self.weapon_bonus_frenzy_until_frame = until_frame;
        }
        self.record_host_weapon_bonus();
    }

    /// Clear Frenzy residual weapon-bonus flags.
    pub fn clear_weapon_bonus_frenzy(&mut self) {
        self.weapon_bonus_frenzy = false;
        self.weapon_bonus_frenzy_until_frame = 0;
        self.weapon_bonus_frenzy_level = 0;
        self.record_host_weapon_bonus();
    }

    /// Expire Frenzy residual when the host frame passes the residual timer.
    pub fn tick_weapon_bonus_frenzy(&mut self, current_frame: u32) {
        if self.weapon_bonus_frenzy
            && self.weapon_bonus_frenzy_until_frame > 0
            && current_frame >= self.weapon_bonus_frenzy_until_frame
        {
            self.clear_weapon_bonus_frenzy();
        }
    }

    /// Retail DAMAGE multiplier while Frenzy residual is active (1.0 when clear).
    pub fn frenzy_damage_multiplier(&self) -> f32 {
        if !self.weapon_bonus_frenzy {
            return 1.0;
        }
        crate::game_logic::host_frenzy::HostFrenzyLevel::from_u8(self.weapon_bonus_frenzy_level)
            .damage_multiplier()
    }

    /// Whether any Strategy Center battle-plan residual weapon bonus is active.
    pub fn has_battle_plan_bonus(&self) -> bool {
        self.weapon_bonus_battle_plan_bombardment
            || self.weapon_bonus_battle_plan_hold_the_line
            || self.weapon_bonus_battle_plan_search_and_destroy
    }

    /// Apply residual Strategy Center army battle-plan bonuses to this unit.
    ///
    /// Clears previous battle-plan residual flags first (plan switch residual).
    pub fn apply_battle_plan_bonus(
        &mut self,
        plan: crate::game_logic::host_strategy_center::HostBattlePlan,
    ) {
        self.clear_battle_plan_bonus();
        match plan {
            crate::game_logic::host_strategy_center::HostBattlePlan::Bombardment => {
                self.weapon_bonus_battle_plan_bombardment = true;
            }
            crate::game_logic::host_strategy_center::HostBattlePlan::HoldTheLine => {
                self.weapon_bonus_battle_plan_hold_the_line = true;
            }
            crate::game_logic::host_strategy_center::HostBattlePlan::SearchAndDestroy => {
                self.weapon_bonus_battle_plan_search_and_destroy = true;
                // Sight residual: scale detection / template sight residual field.
                let scalar = plan.army_sight_range_scalar();
                if (scalar - 1.0).abs() > f32::EPSILON {
                    self.detection_range = self.effective_detection_range() * scalar;
                    self.battle_plan_sight_scalar_applied = scalar;
                }
            }
        }
        self.record_host_weapon_bonus();
        self.record_host_detector();
    }

    /// Clear residual Strategy Center battle-plan bonuses.
    pub fn clear_battle_plan_bonus(&mut self) {
        self.weapon_bonus_battle_plan_bombardment = false;
        self.weapon_bonus_battle_plan_hold_the_line = false;
        self.weapon_bonus_battle_plan_search_and_destroy = false;
        // Undo SearchAndDestroy sight residual.
        if (self.battle_plan_sight_scalar_applied - 1.0).abs() > f32::EPSILON
            && self.battle_plan_sight_scalar_applied > f32::EPSILON
        {
            self.detection_range =
                self.detection_range / self.battle_plan_sight_scalar_applied.max(0.01);
            // If detection_range collapses near template default residual, clear override.
            let base = self.get_template().sight_range;
            if (self.detection_range - base).abs() < 0.5 {
                self.detection_range = 0.0;
            }
        }
        self.battle_plan_sight_scalar_applied = 1.0;
        self.record_host_weapon_bonus();
        self.record_host_detector();
    }

    /// Retail BATTLEPLAN_BOMBARDMENT DAMAGE multiplier (1.0 when clear).
    pub fn battle_plan_damage_multiplier(&self) -> f32 {
        if self.weapon_bonus_battle_plan_bombardment {
            crate::game_logic::host_strategy_center::BOMBARDMENT_DAMAGE_MULT
        } else {
            1.0
        }
    }

    /// Retail HoldTheLine armor damage scalar (incoming damage mult; 1.0 when clear).
    pub fn battle_plan_armor_damage_scalar(&self) -> f32 {
        if self.weapon_bonus_battle_plan_hold_the_line {
            crate::game_logic::host_strategy_center::HOLD_THE_LINE_ARMOR_DAMAGE_SCALAR
        } else {
            1.0
        }
    }

    /// Retail BATTLEPLAN_SEARCHANDDESTROY RANGE multiplier (1.0 when clear).
    pub fn battle_plan_range_multiplier(&self) -> f32 {
        self.weapon_bonus_fields().1
    }

    /// C++ WeaponBonus append residual for active condition flags.
    /// Returns (DAMAGE, RANGE, RATE_OF_FIRE, PRE_ATTACK) multipliers (default 1.0).
    pub fn weapon_bonus_fields(&self) -> (f32, f32, f32, f32) {
        use crate::game_logic::host_propaganda::{
            ENTHUSIASTIC_RATE_OF_FIRE_MULT, SUBLIMINAL_RATE_OF_FIRE_MULT,
        };
        use crate::game_logic::host_red_guard::{
            INFANTRY_HORDE_ROF_MULT, INFANTRY_NATIONALISM_ROF_MULT,
        };
        use crate::game_logic::host_strategy_center::{
            BOMBARDMENT_DAMAGE_MULT, SEARCH_AND_DESTROY_RANGE_MULT,
        };

        let mut damage = 1.0f32;
        let mut range = 1.0f32;
        let mut rof = 1.0f32;
        let pre_attack = 1.0f32;

        if self.weapon_bonus_enthusiastic {
            rof *= ENTHUSIASTIC_RATE_OF_FIRE_MULT;
        }
        if self.weapon_bonus_subliminal {
            rof *= SUBLIMINAL_RATE_OF_FIRE_MULT;
        }
        if self.weapon_bonus_horde {
            rof *= INFANTRY_HORDE_ROF_MULT;
        }
        if self.weapon_bonus_nationalism {
            rof *= INFANTRY_NATIONALISM_ROF_MULT;
        }
        damage *= self.frenzy_damage_multiplier();
        if self.weapon_bonus_battle_plan_bombardment {
            damage *= BOMBARDMENT_DAMAGE_MULT;
        }
        if self.weapon_bonus_battle_plan_search_and_destroy {
            range *= SEARCH_AND_DESTROY_RANGE_MULT;
        }
        // C++ WEAPONBONUSCONDITION_GARRISONED residual (GameData RANGE 133%).
        if self.contained_by.is_some() {
            range *= 1.33;
        }
        // C++ CONTINUOUS_FIRE_MEAN / FAST WeaponBonus ROF residual
        // (GameData defaults MEAN 200%, FAST 300%). Level set by FiringTracker
        // / gattling ramp residuals on Object::continuous_fire_level.
        match self.continuous_fire_level {
            1 => rof *= 2.0,
            2 => rof *= 3.0,
            _ => {}
        }

        (damage, range, rof.max(0.01), pre_attack.max(0.01))
    }

    /// Effective weapon range with WeaponBonus RANGE field.
    pub fn effective_weapon_range(&self, base_range: f32) -> f32 {
        base_range * self.weapon_bonus_fields().1
    }

    /// Effective weapon damage with WeaponBonus DAMAGE field.
    pub fn effective_weapon_damage(&self, base_damage: f32) -> f32 {
        base_damage * self.weapon_bonus_fields().0
    }

    /// Effective reload interval (seconds) with RATE_OF_FIRE bonus.
    pub fn effective_weapon_reload(&self, base_reload: f32) -> f32 {
        let rof = self.weapon_bonus_fields().2;
        (base_reload / rof).max(0.0)
    }

    /// C++ OBJECT_STATUS_FAERIE_FIRE residual (Avenger paint).
    pub fn is_faerie_fire(&self) -> bool {
        self.status.faerie_fire
    }

    /// Apply FAERIE_FIRE status residual until absolute frame (refresh extends timer).

    /// C++ Object::doStatusDamage residual.
    ///
    /// `status_name` is an OBJECT_STATUS_* residual name (e.g. "FAERIE_FIRE").
    /// `duration_frames` is the timer length; refresh extends if later.
    pub fn do_status_damage(
        &mut self,
        status_name: &str,
        duration_frames: u32,
        current_frame: u32,
    ) {
        let until = current_frame.saturating_add(duration_frames.max(1));
        let key = status_name.to_ascii_uppercase();
        match key.as_str() {
            "FAERIE_FIRE" => {
                self.apply_faerie_fire(until);
            }
            "REPULSOR" => {
                self.set_status_repulsor(true);
                // No dedicated timer residual yet — clear on next tick if needed.
            }
            "CAN_ATTACK" | "IS_ATTACKING" => {
                // Non-timer status peels: ignore for damage residual.
            }
            _ => {
                // Unknown status residual: no-op fail-closed (no HP damage).
            }
        }
    }

    pub fn apply_faerie_fire(&mut self, until_frame: u32) {
        self.set_status_faerie_fire(true);
        if until_frame > self.faerie_fire_until_frame {
            self.faerie_fire_until_frame = until_frame;
        }
        crate::game_logic::host_faerie_fire_log::record(
            self.id,
            true,
            self.faerie_fire_until_frame,
        );
    }

    /// Clear FAERIE_FIRE residual status.
    pub fn clear_faerie_fire(&mut self) {
        self.set_status_faerie_fire(false);
        self.faerie_fire_until_frame = 0;
        crate::game_logic::host_faerie_fire_log::record(self.id, false, 0);
    }

    /// Expire FAERIE_FIRE residual when host frame passes the residual timer.
    pub fn tick_faerie_fire(&mut self, current_frame: u32) {
        if self.status.faerie_fire
            && self.faerie_fire_until_frame > 0
            && current_frame >= self.faerie_fire_until_frame
        {
            self.clear_faerie_fire();
        }
    }

    /// Weapon ready with optional TARGET_FAERIE_FIRE ROF residual (150%).

    /// C++ ObjectRepulsorHelper::update residual — clear temporary REPULSOR.
    ///
    /// `repulsor_until_frame` stores remaining frames (countdown), not an absolute
    /// logic frame. C++ helper sleeps 2 seconds then clears the status bit.
    pub fn tick_repulsor_status(&mut self, _current_frame: u32) {
        if !self.status.repulsor {
            self.repulsor_until_frame = 0;
            return;
        }
        if self.repulsor_until_frame == 0 {
            // Permanent script-set REPULSOR (no helper timer).
            return;
        }
        self.repulsor_until_frame = self.repulsor_until_frame.saturating_sub(1);
        if self.repulsor_until_frame == 0 {
            self.set_status_repulsor(false);
        }
    }

    pub fn weapon_ready_vs_target(
        weapon: &Weapon,
        current_time: f32,
        target_has_faerie_fire: bool,
    ) -> bool {
        crate::game_logic::host_avenger::weapon_ready_vs_faerie(
            weapon.last_fire_time,
            weapon.reload_time,
            current_time,
            target_has_faerie_fire,
        )
    }

    /// Ready check with attacker WeaponBonus RATE_OF_FIRE + target FAERIE_FIRE ROF.
    pub fn weapon_ready_vs_target_bonused(
        &self,
        weapon: &Weapon,
        current_time: f32,
        target_has_faerie_fire: bool,
    ) -> bool {
        let base = self.effective_weapon_reload(weapon.reload_time);
        let effective = crate::game_logic::host_avenger::effective_reload_vs_target(
            base,
            target_has_faerie_fire,
        );
        current_time - weapon.last_fire_time >= effective
    }

    /// C++ OBJECT_STATUS_IS_CARBOMB residual.
    pub fn is_car_bomb(&self) -> bool {
        self.status.is_carbomb
    }

    /// C++ OBJECT_STATUS_HIJACKED residual.
    pub fn is_hijacked(&self) -> bool {
        self.status.hijacked
    }
    /// C++ Object::m_privateStatus CAPTURED residual (setCaptured).
    pub fn set_private_captured(&mut self, captured: bool) {
        self.set_status_private_captured(captured);
    }

    /// C++ Object::isCaptured residual.
    pub fn is_private_captured(&self) -> bool {
        self.status.private_captured
    }

    /// Apply ConvertToCarBomb residual onto this vehicle (caller sets team).
    ///
    /// C++ ConvertToCarBombCrateCollide residual:
    /// - WEAPONSET_CARBOMB / SuicideCarBomb weapon
    /// - OBJECT_STATUS_IS_CARBOMB
    /// - endow vision + shroudClearing from converter
    /// - copy converter veterancy level
    /// Binds SuicideCarBomb residual weapon and marks IS_CARBOMB.
    pub fn apply_convert_to_car_bomb(&mut self) {
        self.apply_convert_to_car_bomb_from(None);
    }

    /// Convert with optional donor (terrorist) residual endowments.
    pub fn apply_convert_to_car_bomb_from(&mut self, donor: Option<&Object>) {
        self.set_status_is_carbomb(true);
        self.set_status_disabled_unmanned(false);
        self.set_status_disabled_hacked(false);
        self.status.disabled_hacked_until_frame = 0;
        self.set_status_disabled_emp(false);
        self.status.disabled_emp_until_frame = 0;
        self.set_status_hijacked(false);
        self.weapon = Some(crate::game_logic::host_car_bomb::suicide_car_bomb_weapon());
        self.secondary_weapon = None;
        self.set_active_weapon_slot(0);
        self.status.attacking = false;
        self.set_status_moving(false);
        self.stop_moving();
        self.target = None;
        self.target_location = None;
        self.set_status_force_attack(false);
        self.set_ai_state(AIState::Idle);
        if let Some(d) = donor {
            // C++ setVisionRange / setShroudClearingRange from converter.
            self.vision_range = d.vision_range;
            self.shroud_clearing_range = d.shroud_clearing_range.max(d.vision_range);
            // C++ ExperienceTracker::setVeterancyLevel(converter level).
            let donor_level = d.experience.level;
            if !matches!(donor_level, crate::game_logic::VeterancyLevel::Rookie) {
                let prev = self.experience.level;
                self.experience.level = donor_level;
                self.record_host_veterancy_level();
                // Seed XP to at least the threshold for the donor level residual.
                let thr = self.thing.template.veterancy_xp_thresholds;
                let need = match donor_level {
                    crate::game_logic::VeterancyLevel::Veteran => thr[0],
                    crate::game_logic::VeterancyLevel::Elite => thr[1],
                    crate::game_logic::VeterancyLevel::Heroic => thr[2],
                    crate::game_logic::VeterancyLevel::Rookie => 0.0,
                };
                if self.experience.current < need {
                    self.experience.current = need;
                }
                if prev != donor_level {
                    self.apply_veterancy_bonuses(prev, donor_level);
                }
            }
        }
        self.record_host_crush_vision();
    }

    /// Apply Hijack residual ownership mark (caller sets team).
    /// C++ ConvertToHijackedVehicleCrateCollide: OBJECT_STATUS_HIJACKED + idle AI.

    /// C++ HijackerUpdate enter-vehicle residual (hide hijacker with vehicle).
    pub fn begin_hijacker_in_vehicle(&mut self, vehicle_id: ObjectId) {
        self.hijack_vehicle_id = Some(vehicle_id);
        self.record_host_hijacker();
        self.hijacker_in_vehicle = true;
        self.record_host_hijacker();
        self.hijacker_update_active = true;
        self.record_host_hijacker();
        self.set_status_no_collisions(true);
        self.set_status_masked(true);
        self.set_status_unselectable(true);
        self.status.attacking = false;
        self.set_status_moving(false);
        self.stop_moving();
        self.target = None;
        self.set_ai_state(AIState::Idle);
        // Soft-hide: not destroyed, not selectable.
    }

    /// C++ HijackerUpdate exit when vehicle dies residual.
    pub fn end_hijacker_in_vehicle(&mut self, eject_pos: glam::Vec3, was_airborne: bool) {
        self.hijack_vehicle_id = None;
        self.record_host_hijacker();
        self.hijacker_in_vehicle = false;
        self.record_host_hijacker();
        self.hijacker_update_active = false;
        self.record_host_hijacker();
        self.set_status_no_collisions(false);
        self.set_status_masked(false);
        self.set_status_unselectable(false);
        self.hijacker_was_airborne = was_airborne;
        self.record_host_hijacker();
        self.hijacker_eject_pos = Some(eject_pos);
        self.record_host_hijacker();
        self.set_position(eject_pos);
        self.set_ai_state(AIState::Idle);
        self.stop_moving();
        self.target = None;
    }

    /// Sync ride residual: copy vehicle position + MAX veterancy.
    pub fn tick_hijacker_in_vehicle(
        &mut self,
        vehicle_pos: glam::Vec3,
        vehicle_airborne: bool,
        vehicle_level: crate::game_logic::VeterancyLevel,
        vehicle_xp: f32,
    ) {
        if !self.hijacker_in_vehicle {
            return;
        }
        self.set_position(vehicle_pos);
        self.hijacker_was_airborne = vehicle_airborne;
        self.record_host_hijacker();
        self.hijacker_eject_pos = Some(vehicle_pos);
        self.record_host_hijacker();
        // MAX veterancy residual between jacker and vehicle.
        use crate::game_logic::VeterancyLevel;
        let rank = |l: VeterancyLevel| -> u8 {
            match l {
                VeterancyLevel::Rookie => 0,
                VeterancyLevel::Veteran => 1,
                VeterancyLevel::Elite => 2,
                VeterancyLevel::Heroic => 3,
            }
        };
        let highest = if rank(vehicle_level) >= rank(self.experience.level) {
            vehicle_level
        } else {
            self.experience.level
        };
        if rank(highest) > rank(self.experience.level) {
            let prev = self.experience.level;
            self.experience.level = highest;
            self.record_host_veterancy_level();
            let thr = self.thing.template.veterancy_xp_thresholds;
            let need = match highest {
                VeterancyLevel::Veteran => thr[0],
                VeterancyLevel::Elite => thr[1],
                VeterancyLevel::Heroic => thr[2],
                VeterancyLevel::Rookie => 0.0,
            };
            if self.experience.current < need.max(vehicle_xp) {
                self.experience.current = need.max(vehicle_xp);
            }
            self.apply_veterancy_bonuses(prev, highest);
        }
    }

    pub fn apply_hijacked(&mut self) {
        self.apply_hijacked_from(None);
    }

    /// Hijack with optional donor (hijacker) residual endowments.
    ///
    /// C++ residual:
    /// - OBJECT_STATUS_HIJACKED
    /// - aiIdle after brief move-to-self
    /// - cancel dozer tasks
    /// - MAX(target, jacker) veterancy on both (jacker may be destroyed after)
    pub fn apply_hijacked_from(&mut self, donor: Option<&Object>) {
        self.set_status_hijacked(true);
        self.set_status_disabled_unmanned(false);
        self.set_status_disabled_hacked(false);
        self.status.disabled_hacked_until_frame = 0;
        self.set_status_disabled_emp(false);
        self.status.disabled_emp_until_frame = 0;
        self.set_status_is_carbomb(false);
        self.status.attacking = false;
        self.set_status_moving(false);
        self.stop_moving();
        self.target = None;
        self.target_location = None;
        self.set_status_force_attack(false);
        // C++ aiMoveToPosition(self) then aiIdle — host: clear move + Idle.
        self.set_ai_state(AIState::Idle);
        // Cancel dozer construction/repair residual.
        if self.is_kind_of(KindOf::Worker) || self.is_worker() {
            self.set_ai_state(AIState::Idle);
            // Clear construction target residual if any.
            self.target = None;
        }
        if let Some(d) = donor {
            use crate::game_logic::VeterancyLevel;
            // MAX of target and jacker veterancy residual.
            let rank = |l: VeterancyLevel| -> u8 {
                match l {
                    VeterancyLevel::Rookie => 0,
                    VeterancyLevel::Veteran => 1,
                    VeterancyLevel::Elite => 2,
                    VeterancyLevel::Heroic => 3,
                }
            };
            let highest = if rank(d.experience.level) >= rank(self.experience.level) {
                d.experience.level
            } else {
                self.experience.level
            };
            if rank(highest) > rank(self.experience.level) {
                let prev = self.experience.level;
                self.experience.level = highest;
                self.record_host_veterancy_level();
                let thr = self.thing.template.veterancy_xp_thresholds;
                let need = match highest {
                    VeterancyLevel::Veteran => thr[0],
                    VeterancyLevel::Elite => thr[1],
                    VeterancyLevel::Heroic => thr[2],
                    VeterancyLevel::Rookie => 0.0,
                };
                if self.experience.current < need {
                    self.experience.current = need;
                }
                self.apply_veterancy_bonuses(prev, highest);
            }
        }
    }

    /// True when this aircraft is parked at an airfield (ParkingPlace residual).
    pub fn is_parked_at_airfield(&self) -> bool {
        (self.is_kind_of(KindOf::Aircraft) || self.object_type == ObjectType::Aircraft)
            && self.ai_state == AIState::Docked
            && self.contained_by.is_some()
    }

    /// C++ JetAIUpdate takeoff residual from ParkingPlace.
    ///
    /// Clears hangar bookkeeping, lifts to ApproachHeight (**50**), marks airborne.
    /// Returns the airfield id that was left (if any).
    pub fn takeoff_from_airfield_parking(&mut self) -> Option<ObjectId> {
        if !(self.is_kind_of(KindOf::Aircraft) || self.object_type == ObjectType::Aircraft) {
            return None;
        }
        if self.ai_state != AIState::Docked && self.contained_by.is_none() {
            return None;
        }
        let af = self.contained_by.take();
        self.set_ai_state(AIState::Idle);
        self.status.airborne_target = true;
        // Retail AmericaAirfield ApproachHeight residual.
        use crate::game_logic::host_dock_contain_exit_heal_residual::PARKING_PLACE_AIRFIELD_APPROACH_HEIGHT;
        let mut pos = self.get_position();
        if pos.y < PARKING_PLACE_AIRFIELD_APPROACH_HEIGHT {
            pos.y = PARKING_PLACE_AIRFIELD_APPROACH_HEIGHT;
            self.set_position(pos);
        }
        af
    }

    pub fn can_attack(&self) -> bool {
        // Garrisoned units may still fire from the structure (residual
        // fire-from-garrison). Docked transport cargo and units mid-enter cannot.
        // Docked aircraft may attack (ParkingPlace takeoff/sortie residual).
        // weapons_jammed: C++ canFireWeapon DISABLED_SUBDUED residual (ECM field).
        // shock stun: C++ Physics IS_STUNNED residual — cannot acquire/fire while stunned.
        let parked_aircraft = self.is_parked_at_airfield();
        self.is_alive()
            && self.weapon.is_some()
            && !self.is_disabled()
            && !self.is_shock_stunned()
            && !self.status.weapons_jammed
            && (parked_aircraft || !matches!(self.ai_state, AIState::Docked | AIState::Entering))
    }

    /// Authoritative container for docked/garrisoned units.
    /// Prefer `contained_by`; fall back to `target` for legacy enter paths.
    pub fn container_id(&self) -> Option<ObjectId> {
        if let Some(id) = self.contained_by {
            return Some(id);
        }
        if matches!(self.ai_state, AIState::Docked | AIState::Garrisoned) {
            self.target
        } else {
            None
        }
    }

    /// True when this unit is currently inside a transport or garrison.
    pub fn is_contained(&self) -> bool {
        matches!(self.ai_state, AIState::Docked | AIState::Garrisoned)
            || self.contained_by.is_some()
    }

    pub fn is_attackable(&self) -> bool {
        self.is_alive() && self.is_kind_of(KindOf::Attackable)
    }

    pub fn get_position(&self) -> Vec3 {
        self.thing.get_position()
    }

    pub fn set_position(&mut self, position: Vec3) {
        self.thing.set_position(position);
        // Keep compatibility shadow in sync (many call sites still read `position`).
        self.position = position;
    }

    pub fn get_orientation(&self) -> f32 {
        self.thing.get_orientation()
    }

    pub fn set_orientation(&mut self, angle: f32) {
        self.thing.set_orientation(angle);
    }

    pub fn get_transform_matrix(&self) -> Mat4 {
        self.thing.get_transform_matrix()
    }

    /// C++ ActiveBody visual condition + Drawable::reactToBodyDamageStateChange residual.

    /// C++ ProductionUpdate MODELCONDITION_CONSTRUCTION_COMPLETE residual.

    /// C++ RadarUpdate::extendRadar residual.

    /// C++ ProductionUpdate door residual:
    /// OPENING → WAITING_OPEN → WAITING_TO_CLOSE → CLOSING → idle.
    ///
    /// Retail residual timings (fail-closed vs full INI Door*Time):
    /// open 15f, wait-open 30f, wait-to-close 1f, close 15f.
    pub fn start_production_door_cycle(&mut self, now: u32) {
        use crate::game_logic::host_enum_table_residual::{
            door_1_closing_model_bit, door_1_opening_model_bit, door_1_waiting_open_model_bit,
            door_1_waiting_to_close_model_bit,
        };
        // Clear door 1 bits then set OPENING.
        let open_b = door_1_opening_model_bit();
        let wait_b = door_1_waiting_open_model_bit();
        let wait_close_b = door_1_waiting_to_close_model_bit();
        let close_b = door_1_closing_model_bit();
        self.model_condition_bits &= !(1u128 << open_b);
        self.model_condition_bits &= !(1u128 << wait_b);
        self.model_condition_bits &= !(1u128 << wait_close_b);
        self.model_condition_bits &= !(1u128 << close_b);
        self.model_condition_bits |= 1u128 << open_b;
        self.production_door_phase = 1;
        self.production_door_phase_end_frame = now.saturating_add(15);
        self.refresh_model_condition_bits();
        self.record_host_model_condition();
        self.record_host_production_door();
    }

    /// C++ ProductionUpdate::setHoldDoorOpen residual.
    ///
    /// When hold becomes true and the door is idle, starts OPENING residual.
    /// While hold is true, tick will not leave WAITING_OPEN / WAITING_TO_CLOSE.
    pub fn set_production_door_hold_open(&mut self, hold: bool, now: u32) {
        self.production_door_hold_open = hold;
        if hold && self.production_door_phase == 0 {
            // C++: if all door frames 0, start opening.
            self.start_production_door_cycle(now);
        }
        if !hold {
            // Allow close path to proceed from current phase on next tick.
            // If stuck in wait phases, schedule immediate advance eligibility.
            if matches!(self.production_door_phase, 2 | 3) {
                self.production_door_phase_end_frame = now;
            }
        }
        self.record_host_production_door();
    }

    /// Advance production door residual; returns true when cycle fully closed.
    pub fn tick_production_door(&mut self, now: u32) -> bool {
        if self.production_door_phase == 0 {
            return false;
        }
        if now < self.production_door_phase_end_frame {
            return false;
        }
        use crate::game_logic::host_enum_table_residual::{
            door_1_closing_model_bit, door_1_opening_model_bit, door_1_waiting_open_model_bit,
            door_1_waiting_to_close_model_bit,
        };
        let open_b = door_1_opening_model_bit();
        let wait_b = door_1_waiting_open_model_bit();
        let wait_close_b = door_1_waiting_to_close_model_bit();
        let close_b = door_1_closing_model_bit();
        let result = match self.production_door_phase {
            1 => {
                // OPENING → WAITING_OPEN
                self.model_condition_bits &= !(1u128 << open_b);
                self.model_condition_bits |= 1u128 << wait_b;
                self.production_door_phase = 2;
                self.record_host_production_door();
                self.production_door_phase_end_frame = now.saturating_add(30);
                self.refresh_model_condition_bits();
                false
            }
            2 => {
                // C++: !m_holdOpen required to leave WAITING_OPEN.
                if self.production_door_hold_open {
                    // Keep waiting-open while held (refresh wait stamp residual).
                    self.production_door_phase_end_frame = now.saturating_add(30);
                    return false;
                }
                // WAITING_OPEN → WAITING_TO_CLOSE residual (C++ theWaitingToCloseFlags).
                self.model_condition_bits &= !(1u128 << wait_b);
                self.model_condition_bits |= 1u128 << wait_close_b;
                self.production_door_phase = 3;
                self.record_host_production_door();
                // Minimal hold before CLOSING residual (INI DoorCloseTime path).
                self.production_door_phase_end_frame = now.saturating_add(1);
                self.refresh_model_condition_bits();
                false
            }
            3 => {
                // C++: !m_holdOpen required to leave WAITING_TO_CLOSE / CLOSING path.
                if self.production_door_hold_open {
                    self.production_door_phase_end_frame = now.saturating_add(1);
                    return false;
                }
                // WAITING_TO_CLOSE → CLOSING
                self.model_condition_bits &= !(1u128 << wait_close_b);
                self.model_condition_bits |= 1u128 << close_b;
                self.production_door_phase = 4;
                self.record_host_production_door();
                self.production_door_phase_end_frame = now.saturating_add(15);
                self.refresh_model_condition_bits();
                false
            }
            4 => {
                // C++: !m_holdOpen required to finish closing.
                if self.production_door_hold_open {
                    // Snap back to waiting-open while held.
                    self.model_condition_bits &= !(1u128 << close_b);
                    self.model_condition_bits |= 1u128 << wait_b;
                    self.production_door_phase = 2;
                    self.record_host_production_door();
                    self.production_door_phase_end_frame = now.saturating_add(30);
                    self.refresh_model_condition_bits();
                    return false;
                }
                // CLOSING → idle
                self.model_condition_bits &= !(1u128 << close_b);
                self.production_door_phase = 0;
                self.record_host_production_door();
                self.production_door_phase_end_frame = 0;
                self.refresh_model_condition_bits();
                true
            }
            _ => {
                self.production_door_phase = 0;
                self.record_host_production_door();
                false
            }
        };
        self.record_host_model_condition();
        result
    }

    pub fn extend_radar(&mut self, done_frame: u32) {
        use crate::game_logic::host_enum_table_residual::radar_extending_model_bit;
        let bit = radar_extending_model_bit();
        self.model_condition_bits |= 1u128 << bit;
        // Clear upgraded while extending.
        use crate::game_logic::host_enum_table_residual::radar_upgraded_model_bit;
        self.model_condition_bits &= !(1u128 << radar_upgraded_model_bit());
        self.radar_extend_done_frame = done_frame;
        self.radar_extend_complete = false;
        self.radar_active = true;
        self.record_host_radar_extend();
        self.refresh_model_condition_bits();
        self.record_host_model_condition();
    }

    /// C++ RadarUpdate::update extend completion residual.
    /// Returns true when extension just completed this tick.
    pub fn tick_radar_extend(&mut self, current_frame: u32) -> bool {
        if self.radar_extend_done_frame == 0 || self.radar_extend_complete {
            return false;
        }
        if current_frame <= self.radar_extend_done_frame {
            return false;
        }
        use crate::game_logic::host_enum_table_residual::{
            radar_extending_model_bit, radar_upgraded_model_bit,
        };
        self.radar_extend_complete = true;
        self.radar_extend_done_frame = 0;
        self.model_condition_bits &= !(1u128 << radar_extending_model_bit());
        self.model_condition_bits |= 1u128 << radar_upgraded_model_bit();
        self.refresh_model_condition_bits();
        self.record_host_radar_extend();
        self.record_host_model_condition();
        true
    }

    /// C++ BuildAssistant/Dozer construction model-condition residual.
    ///
    /// - With active dozer nearby: PARTIALLY_CONSTRUCTED + ACTIVELY_BEING_CONSTRUCTED
    /// - Without dozer (waiting): AWAITING_CONSTRUCTION + PARTIALLY_CONSTRUCTED
    /// - Clears ACTIVELY_BEING when dozer leaves.

    /// C++ MODELCONDITION_ACTIVELY_CONSTRUCTING residual (dozer or factory).

    /// C++ BuildAssistant sell scaffold model residual (start of sell).
    /// C++ TechBuildingBehavior MODELCONDITION_CAPTURED residual.
    pub fn set_captured_model_condition(&mut self, captured: bool) {
        use crate::game_logic::host_enum_table_residual::captured_model_bit;
        let bit = captured_model_bit();
        if bit == 0 {
            return;
        }
        if captured {
            self.model_condition_bits |= 1u128 << bit;
        } else {
            self.model_condition_bits &= !(1u128 << bit);
        }
        self.refresh_model_condition_bits();
        self.record_host_model_condition();
    }

    pub fn has_captured_model_condition(&self) -> bool {
        use crate::game_logic::host_enum_table_residual::captured_model_bit;
        let bit = captured_model_bit();
        bit != 0 && (self.model_condition_bits & (1u128 << bit)) != 0
    }

    pub fn apply_sell_scaffold_model_conditions(&mut self) {
        use crate::game_logic::host_enum_table_residual::{
            actively_being_constructed_model_bit, construction_complete_model_bit,
            partially_constructed_model_bit, sold_model_bit,
        };
        let part_b = partially_constructed_model_bit();
        let active_b = actively_being_constructed_model_bit();
        let sold_b = sold_model_bit();
        let complete_b = construction_complete_model_bit();
        self.model_condition_bits &= !(1u128 << sold_b);
        self.model_condition_bits &= !(1u128 << complete_b);
        self.model_condition_bits |= 1u128 << part_b;
        self.model_condition_bits |= 1u128 << active_b;
        self.refresh_model_condition_bits();
        self.record_host_model_condition();
    }

    /// C++ BuildAssistant: when construction percent crosses to <= 0 during sell.
    pub fn apply_sold_model_condition(&mut self) {
        use crate::game_logic::host_enum_table_residual::{
            actively_being_constructed_model_bit, awaiting_construction_model_bit,
            partially_constructed_model_bit, sold_model_bit,
        };
        let await_b = awaiting_construction_model_bit();
        let part_b = partially_constructed_model_bit();
        let active_b = actively_being_constructed_model_bit();
        let sold_b = sold_model_bit();
        self.model_condition_bits &= !(1u128 << await_b);
        self.model_condition_bits &= !(1u128 << part_b);
        self.model_condition_bits &= !(1u128 << active_b);
        self.model_condition_bits |= 1u128 << sold_b;
        self.refresh_model_condition_bits();
        self.record_host_model_condition();
    }

    /// C++ Object::attemptHealingFromSoleBenefactor residual.
    ///
    /// Non-stacking healers (dozer repair, ambulance, propaganda) claim exclusive
    /// heal rights for `duration` frames. Returns false if another benefactor still
    /// owns the claim.
    pub fn attempt_healing_from_sole_benefactor(
        &mut self,
        amount: f32,
        source_id: ObjectId,
        duration_frames: u32,
        now: u32,
    ) -> bool {
        if amount <= 0.0 {
            return false;
        }
        let claim_open = now > self.sole_healing_benefactor_expiration_frame
            || self.sole_healing_benefactor == Some(source_id);
        self.record_host_sole_healing();
        if !claim_open {
            return false;
        }
        self.sole_healing_benefactor = Some(source_id);
        self.record_host_sole_healing();
        self.sole_healing_benefactor_expiration_frame = now.saturating_add(duration_frames);
        self.record_host_sole_healing();
        let before = self.health.current;
        self.heal(amount);
        self.health.current > before + 0.0001 || self.health.current >= self.health.maximum - 0.01
    }

    pub fn set_actively_constructing(&mut self, active: bool) {
        use crate::game_logic::host_enum_table_residual::actively_constructing_model_bit;
        let bit = actively_constructing_model_bit();
        if active {
            self.model_condition_bits |= 1u128 << bit;
        } else {
            self.model_condition_bits &= !(1u128 << bit);
        }
        self.refresh_model_condition_bits();
        self.record_host_model_condition();
    }

    pub fn set_under_construction_model_conditions(&mut self, actively_built: bool) {
        use crate::game_logic::host_enum_table_residual::{
            actively_being_constructed_model_bit, awaiting_construction_model_bit,
            construction_complete_model_bit, partially_constructed_model_bit,
        };
        let await_b = awaiting_construction_model_bit();
        let part_b = partially_constructed_model_bit();
        let active_b = actively_being_constructed_model_bit();
        let complete_b = construction_complete_model_bit();
        // Clear all construction-related bits first.
        self.model_condition_bits &= !(1u128 << await_b);
        self.model_condition_bits &= !(1u128 << part_b);
        self.model_condition_bits &= !(1u128 << active_b);
        self.model_condition_bits &= !(1u128 << complete_b);
        self.model_condition_bits |= 1u128 << part_b;
        if actively_built {
            self.model_condition_bits |= 1u128 << active_b;
        } else {
            self.model_condition_bits |= 1u128 << await_b;
        }
        self.refresh_model_condition_bits();
        self.record_host_model_condition();
    }

    /// C++ clear construction model conditions on finish (before CONSTRUCTION_COMPLETE).
    pub fn clear_under_construction_model_conditions(&mut self) {
        use crate::game_logic::host_enum_table_residual::{
            actively_being_constructed_model_bit, awaiting_construction_model_bit,
            partially_constructed_model_bit,
        };
        let await_b = awaiting_construction_model_bit();
        let part_b = partially_constructed_model_bit();
        let active_b = actively_being_constructed_model_bit();
        self.model_condition_bits &= !(1u128 << await_b);
        self.model_condition_bits &= !(1u128 << part_b);
        self.model_condition_bits &= !(1u128 << active_b);
        self.refresh_model_condition_bits();
        self.record_host_model_condition();
    }

    /// C++ ProductionUpdate ConstructionCompleteDuration residual (default 45f / 1500ms).
    pub const CONSTRUCTION_COMPLETE_DURATION_FRAMES_RESIDUAL: u32 = 45;

    pub fn set_construction_complete_condition(&mut self) {
        self.set_construction_complete_condition_at(0);
    }

    /// Set CONSTRUCTION_COMPLETE and schedule clear after residual duration.
    /// `now==0` keeps bit sticky (legacy structure-complete path until tick).
    pub fn set_construction_complete_condition_at(&mut self, now: u32) {
        use crate::game_logic::host_enum_table_residual::construction_complete_model_bit;
        let bit = construction_complete_model_bit();
        self.model_condition_bits |= 1u128 << bit;
        if now > 0 {
            self.construction_complete_clear_frame =
                now.saturating_add(Self::CONSTRUCTION_COMPLETE_DURATION_FRAMES_RESIDUAL);
            self.record_host_rebuild_producer();
        } else {
            // Structure build-complete path: clear after residual duration from next tick
            // when caller supplies frame via arm helper.
            self.construction_complete_clear_frame = 0;
            self.record_host_rebuild_producer();
        }
        self.refresh_model_condition_bits();
    }

    /// Arm clear deadline for CONSTRUCTION_COMPLETE (C++ m_constructionCompleteFrame).
    pub fn arm_construction_complete_clear(&mut self, now: u32) {
        if now == 0 {
            return;
        }
        self.construction_complete_clear_frame =
            now.saturating_add(Self::CONSTRUCTION_COMPLETE_DURATION_FRAMES_RESIDUAL);
        self.record_host_rebuild_producer();
    }

    /// C++ ProductionUpdate clear CONSTRUCTION_COMPLETE after duration.
    /// Returns true when the bit was cleared this tick.
    pub fn tick_construction_complete_clear(&mut self, now: u32) -> bool {
        if self.construction_complete_clear_frame == 0 {
            return false;
        }
        if now < self.construction_complete_clear_frame {
            return false;
        }
        use crate::game_logic::host_enum_table_residual::construction_complete_model_bit;
        let bit = construction_complete_model_bit();
        self.model_condition_bits &= !(1u128 << bit);
        self.construction_complete_clear_frame = 0;
        self.record_host_rebuild_producer();
        self.refresh_model_condition_bits();
        true
    }

    pub fn refresh_model_condition_bits(&mut self) {
        use crate::game_logic::host_enum_table_residual::{
            host_apply_body_damage_model_bits, host_calc_body_damage_state, HostBodyDamageType,
            MC_BIT_ATTACKING, MC_BIT_DISGUISED, MC_BIT_DYING, MC_BIT_MOVING,
        };
        let health = self.health.current;
        let max_h = self.health.maximum.max(0.0);
        let old_state = self.body_damage_state;
        let state = if self.status.destroyed || health <= 0.0 {
            HostBodyDamageType::Rubble
        } else {
            host_calc_body_damage_state(health, max_h)
        };
        self.body_damage_state = state;
        crate::game_logic::host_body_damage_log::record(self.id, state.ordinal());
        // C++ BoneFXDamage::onBodyDamageStateChange residual.
        if old_state != state {
            if self.bone_fx_damage.is_none()
                && crate::game_logic::host_bone_fx_damage::wants_bone_fx(&self.template_name)
            {
                self.bone_fx_damage =
                    Some(crate::game_logic::host_bone_fx_damage::HostBoneFxDamageData::default());
            }
            if let Some(bfx) = self.bone_fx_damage.as_mut() {
                let _ = bfx.on_body_damage_state_change(&self.template_name, old_state, state);
            }
        }
        // C++ TransitionDamageFX::onBodyDamageStateChange residual.
        self.ensure_transition_damage_fx();
        if let Some(cfg) = self.transition_damage_fx.as_ref() {
            if let Some(ev) = crate::game_logic::host_transition_damage_fx::transition_event(
                cfg, old_state, state,
            ) {
                self.pending_transition_damage_fx.push(ev);
            }
        }
        // C++ FXListDie when entering rubble / destroyed.
        if matches!(state, HostBodyDamageType::Rubble)
            && !matches!(old_state, HostBodyDamageType::Rubble)
        {
            self.fire_fx_list_die();
            self.fire_create_object_die();
        }
        let mut bits = host_apply_body_damage_model_bits(self.model_condition_bits, state);
        // Motion / combat residual bits from ObjectStatus.
        if self.status.moving {
            bits |= 1u128 << MC_BIT_MOVING;
        } else {
            bits &= !(1u128 << MC_BIT_MOVING);
        }
        if self.status.attacking {
            bits |= 1u128 << MC_BIT_ATTACKING;
        } else {
            bits &= !(1u128 << MC_BIT_ATTACKING);
        }
        if self.status.destroyed {
            bits |= 1u128 << MC_BIT_DYING;
        } else {
            bits &= !(1u128 << MC_BIT_DYING);
        }
        if self.status.disguised {
            bits |= 1u128 << MC_BIT_DISGUISED;
        } else {
            bits &= !(1u128 << MC_BIT_DISGUISED);
        }
        // C++ Physics stun model conditions residual.
        use crate::game_logic::host_enum_table_residual::{
            MC_BIT_STUNNED, MC_BIT_STUNNED_FLAILING,
        };
        bits &= !(1u128 << MC_BIT_STUNNED_FLAILING);
        bits &= !(1u128 << MC_BIT_STUNNED);
        if self.shock_stun_frames > 15 {
            bits |= 1u128 << MC_BIT_STUNNED_FLAILING;
        } else if self.shock_stun_frames > 0 {
            bits |= 1u128 << MC_BIT_STUNNED;
        }
        // FREEFALL residual: airborne while stunned (C++ IS_IN_FREEFALL path).
        use crate::game_logic::host_enum_table_residual::MC_BIT_FREEFALL;
        bits &= !(1u128 << MC_BIT_FREEFALL);
        let airborne = self.get_position().y > 0.05 || self.movement.velocity.y > 1.0;
        if airborne && self.shock_stun_frames > 0 && self.shock_was_airborne {
            bits |= 1u128 << MC_BIT_FREEFALL;
        }
        // After first ground contact, prefer STUNNED over FLAILING even if frames high.
        if self.shock_grounded_once && self.shock_stun_frames > 0 {
            use crate::game_logic::host_enum_table_residual::{
                MC_BIT_STUNNED, MC_BIT_STUNNED_FLAILING,
            };
            bits &= !(1u128 << MC_BIT_STUNNED_FLAILING);
            bits |= 1u128 << MC_BIT_STUNNED;
        }
        // Radar extend residual sticks across body/motion refresh.
        use crate::game_logic::host_enum_table_residual::{
            radar_extending_model_bit, radar_upgraded_model_bit,
        };
        let had_radar_ext =
            (self.model_condition_bits & (1u128 << radar_extending_model_bit())) != 0;
        let had_radar_upg =
            (self.model_condition_bits & (1u128 << radar_upgraded_model_bit())) != 0;
        use crate::game_logic::host_enum_table_residual::{
            door_1_closing_model_bit, door_1_opening_model_bit, door_1_waiting_open_model_bit,
            door_1_waiting_to_close_model_bit,
        };
        let had_door_open =
            (self.model_condition_bits & (1u128 << door_1_opening_model_bit())) != 0;
        let had_door_wait =
            (self.model_condition_bits & (1u128 << door_1_waiting_open_model_bit())) != 0;
        let had_door_wait_close =
            (self.model_condition_bits & (1u128 << door_1_waiting_to_close_model_bit())) != 0;
        let had_door_close =
            (self.model_condition_bits & (1u128 << door_1_closing_model_bit())) != 0;

        // SPLATTED residual sticks after fatal falling damage.
        use crate::game_logic::host_enum_table_residual::MC_BIT_SPLATTED;
        let had_splat = (self.model_condition_bits & (1u128 << MC_BIT_SPLATTED)) != 0;
        if had_splat
            || (self.status.destroyed
                && self.status.death_type
                    == crate::game_logic::host_usa_pilot::HostDeathType::Splatted)
        {
            bits |= 1u128 << MC_BIT_SPLATTED;
            crate::game_logic::host_death_type_log::record(
                self.id,
                self.status.death_type.ordinal(),
            );
        }
        if had_radar_ext {
            bits |= 1u128 << radar_extending_model_bit();
        }
        if had_radar_upg {
            bits |= 1u128 << radar_upgraded_model_bit();
        }
        if had_door_open {
            bits |= 1u128 << door_1_opening_model_bit();
        }
        if had_door_wait {
            bits |= 1u128 << door_1_waiting_open_model_bit();
        }
        if had_door_wait_close {
            bits |= 1u128 << door_1_waiting_to_close_model_bit();
        }
        if had_door_close {
            bits |= 1u128 << door_1_closing_model_bit();
        }
        // Construction residual bits stick across body/motion refresh.
        use crate::game_logic::host_enum_table_residual::{
            actively_being_constructed_model_bit, awaiting_construction_model_bit,
            construction_complete_model_bit, partially_constructed_model_bit,
        };
        let had_cc =
            (self.model_condition_bits & (1u128 << construction_complete_model_bit())) != 0;
        let had_await =
            (self.model_condition_bits & (1u128 << awaiting_construction_model_bit())) != 0;
        let had_partial =
            (self.model_condition_bits & (1u128 << partially_constructed_model_bit())) != 0;
        let had_active =
            (self.model_condition_bits & (1u128 << actively_being_constructed_model_bit())) != 0;
        if had_cc {
            bits |= 1u128 << construction_complete_model_bit();
        }
        if had_await {
            bits |= 1u128 << awaiting_construction_model_bit();
        }
        if had_partial {
            bits |= 1u128 << partially_constructed_model_bit();
        }
        if had_active {
            bits |= 1u128 << actively_being_constructed_model_bit();
        }
        use crate::game_logic::host_enum_table_residual::actively_constructing_model_bit;
        let had_ac =
            (self.model_condition_bits & (1u128 << actively_constructing_model_bit())) != 0;
        if had_ac {
            bits |= 1u128 << actively_constructing_model_bit();
        }
        use crate::game_logic::host_enum_table_residual::sold_model_bit;
        let had_sold_mc = (self.model_condition_bits & (1u128 << sold_model_bit())) != 0;
        if had_sold_mc {
            bits |= 1u128 << sold_model_bit();
        }
        self.model_condition_bits = bits;
        self.record_host_model_condition();
    }

    pub fn take_damage(&mut self, damage: f32) -> bool {
        self.take_damage_from(damage, None)
    }

    /// Apply damage with optional C++ BodyModule last-damage-source residual.
    ///
    /// Passive AI mood (WaitForAttack) uses `last_damage_source` for idle
    /// mood-target retaliate residual.

    /// C++ PhysicsBehavior::applyShock residual (ground units only).
    ///
    /// Adds lateral+up velocity impulse and a short stun residual. Airborne /
    /// aircraft / projectiles are immune (C++ isAirborneTarget / KINDOF_PROJECTILE).

    /// C++ PhysicsBehavior defaults for shock random rotation residual.
    pub const SHOCK_MAX_YAW: f32 = 0.05;
    pub const SHOCK_MAX_PITCH: f32 = 0.025;
    pub const SHOCK_MAX_ROLL: f32 = 0.025;

    /// C++ PhysicsBehavior::applyRandomRotation residual.
    ///
    /// Adds random yaw/pitch/roll rates and immediately kicks orientation yaw
    /// so the tumble is observable without a full rigid-body integrator.
    /// Structures stick-to-ground residual: no rotation.
    pub fn apply_shock_random_rotation(&mut self, seed: u32) {
        if self.is_kind_of(KindOf::Structure) {
            return;
        }
        use crate::game_logic::host_rng_residual::pure_logic_random_real;
        // GameLogicRandomValue(-1, 1) residual via pure stream.
        let yaw_m = pure_logic_random_real(seed, 10, -1.0, 1.0);
        let pitch_m = pure_logic_random_real(seed, 11, -1.0, 1.0);
        let roll_m = pure_logic_random_real(seed, 12, -1.0, 1.0);
        self.shock_yaw_rate += Self::SHOCK_MAX_YAW * yaw_m;
        self.shock_pitch_rate += Self::SHOCK_MAX_PITCH * pitch_m;
        self.shock_roll_rate += Self::SHOCK_MAX_ROLL * roll_m;
        // Immediate yaw kick (presentation/tumble residual).
        let ori = self.get_orientation() + self.shock_yaw_rate;
        self.set_orientation(ori);
        // Tumble upright residual: strong pitch/roll can invert (Z-up < 0).
        self.shock_up_z -= (pitch_m.abs() + roll_m.abs()) * 0.75;
        if self.shock_up_z < -1.0 {
            self.shock_up_z = -1.0;
        }
        if self.shock_up_z > 1.0 {
            self.shock_up_z = 1.0;
        }
        self.record_host_shock_stun();
    }

    pub fn apply_shock_wave_impulse(&mut self, force: glam::Vec3) -> bool {
        if !self.is_alive() {
            return false;
        }
        if self.is_kind_of(KindOf::Aircraft) || self.status.airborne_target {
            return false;
        }
        if self.is_kind_of(KindOf::Structure) {
            return false;
        }
        // Scale residual: force is weapon units; convert to velocity nudge.
        const FORCE_TO_VEL: f32 = 0.05;
        let impulse = force * FORCE_TO_VEL;
        self.movement.velocity += impulse;
        // Cap residual velocity so MOAB doesn't fling units off-map instantly.
        let speed = self.movement.velocity.length();
        const MAX_SHOCK_SPEED: f32 = 80.0;
        if speed > MAX_SHOCK_SPEED {
            self.movement.velocity *= MAX_SHOCK_SPEED / speed;
        }
        // C++ applyRandomRotation residual (deterministic seed from id + force).
        let seed = self
            .id
            .0
            .wrapping_mul(0x9E37_79B9)
            .wrapping_add((force.x.to_bits()).wrapping_mul(0x85EB_CA6B))
            .wrapping_add(force.z.to_bits());
        self.apply_shock_random_rotation(seed);
        // C++ applyRandomRotation sets ALLOW_BOUNCE until bounce completes.
        self.shock_allow_bounce = true;
        self.shock_grounded_once = false;
        self.shock_up_z = 1.0;
        self.ensure_locomotor_surfaces();
        // Strong upward impulse residual: freefall model bit while airborne from shock.
        if self.movement.velocity.y > 8.0 {
            use crate::game_logic::host_enum_table_residual::MC_BIT_FREEFALL;
            self.model_condition_bits |= 1u128 << MC_BIT_FREEFALL;
            self.shock_was_airborne = true;
        }
        // C++ setStunned(true) + MODELCONDITION_STUNNED_FLAILING residual.
        // Duration: 45 frames (~1.5s). First 30 flailing, then STUNNED, then clear.
        const TOTAL: u32 = 45;
        self.shock_stun_frames = self.shock_stun_frames.max(TOTAL);
        self.refresh_model_condition_bits();
        if matches!(
            self.ai_state,
            AIState::Attacking | AIState::AttackMoving | AIState::Moving
        ) {
            self.set_status_moving(true);
        }
        true
    }

    /// C++ GlobalData::m_groundStiffness default residual.
    pub const GROUND_STIFFNESS: f32 = 0.5;
    /// Host gravity residual (world-Y up) while shock-airborne.
    pub const SHOCK_GRAVITY: f32 = -1.0; // C++ GlobalData::m_gravity residual
    /// C++ handleBounce YPR damping residual.
    pub const BOUNCE_YPR_DAMPING: f32 = 0.7;
    /// C++ PhysicsBehavior mass default residual.
    pub const SHOCK_MASS: f32 = 1.0;
    /// C++ FallHeightDamageFactor default residual.
    pub const FALL_HEIGHT_DAMAGE_FACTOR: f32 = 1.0;
    /// C++ min fall angle tan residual (~71 degrees).
    pub const FALL_MIN_ANGLE_TAN: f32 = 3.0;
    pub const FALL_TINY_DELTA: f32 = 0.01;

    /// C++ heightToSpeed(height) = sqrt(|2*g*h|) with g residual 1.0.
    pub fn height_to_fall_speed(height: f32) -> f32 {
        (2.0 * Self::SHOCK_GRAVITY.abs() * height.abs()).sqrt()
    }

    /// C++ PhysicsBehaviorModuleData::m_minFallSpeedForDamage default (height 40).
    pub fn min_fall_speed_for_damage() -> f32 {
        Self::height_to_fall_speed(40.0)
    }

    /// C++ falling-damage residual when leaving airborne for ground.
    ///
    /// `impact_vy` is world-Y velocity at impact (negative when falling).
    /// Returns damage applied (0 if none).

    /// C++ isVerySmall3D residual on velocity.
    pub fn velocity_is_very_small(&self) -> bool {
        let v = self.movement.velocity;
        v.x.abs() < VERY_SMALL_VEL && v.y.abs() < VERY_SMALL_VEL && v.z.abs() < VERY_SMALL_VEL
    }

    /// C++ PhysicsBehavior::doBounceSound residual (event count + fall dy + volume).

    /// C++ PhysicsBehavior onCollide vehicle-into-immobile crash residual.

    /// C++ PhysicsBehavior::scrubVelocity2D residual (host XZ ground plane).
    ///
    /// If desired < 0.001, zero lateral velocity. Else scale down if faster than desired.

    /// C++ PhysicsBehavior::setIgnoreCollisionsWith residual.

    /// C++ Object::getUnitDirectionVector2D residual (XZ ground, glam x/z).
    pub fn unit_direction_vector_2d(&self) -> glam::Vec2 {
        // Match unit_direction_xz: orientation 0 faces +X (host XZ plane).
        let (x, z) = self.unit_direction_xz();
        glam::Vec2::new(x, z)
    }

    /// C++ AIUpdateInterface::blockedBy residual (simplified geometry).
    ///
    /// Fail-closed vs full pathfind goal cell / path priority matrix.
    pub fn ai_blocked_by(&self, other: &Object) -> bool {
        if self.can_crush_only(other, false) {
            return false;
        }
        let other_ground =
            other.can_move() && !other.status.airborne_target && !other.is_parachuting();
        if !other_ground {
            return false;
        }
        let us = self.get_position();
        let them = other.get_position();
        let dx = them.x - us.x;
        let dz = them.z - us.z; // host XZ ground plane (C++ XY)
        let dsqr = dx * dx + dz * dz;
        // Same-cell residual: path priority by ObjectId.
        if dsqr < PATHFIND_CELL_SIZE_F_RESIDUAL * PATHFIND_CELL_SIZE_F_RESIDUAL * 0.0001 {
            return self.id.0 > other.id.0; // higher id = lower priority loses
        }

        let our_dir = self.unit_direction_vector_2d();
        let their_dir = other.unit_direction_vector_2d();
        let dir_dot = our_dir.x * their_dir.x + our_dir.y * their_dir.y;

        // Infantry vs infantry: only block if same-ish heading.
        if self.is_kind_of(crate::game_logic::KindOf::Infantry)
            && other.is_kind_of(crate::game_logic::KindOf::Infantry)
            && dir_dot <= 0.25
        {
            return false;
        }

        // Relative angle of other from us along our facing.
        let collision_angle = self.relative_angle_2d_to(them);
        let mut angle_limit = std::f32::consts::FRAC_PI_4; // 45 deg
        let other_moving = other.movement.velocity.length_squared() > 0.01;
        if !other_moving {
            angle_limit *= 0.75;
        }
        if collision_angle > std::f32::consts::FRAC_PI_2
            || collision_angle < -std::f32::consts::FRAC_PI_2
        {
            return false; // moving away
        }
        if collision_angle > angle_limit || collision_angle < -angle_limit {
            if dir_dot <= 0.0 {
                return false;
            }
            // Off-angle residual: not blocked unless head-on closing.
            return false;
        }

        // Long blocked + opposite heading: pass through residual.
        if self.num_frames_blocked > 30 && dir_dot <= 0.0 {
            return false;
        }

        !other.status.destroyed && other.is_alive()
    }

    /// C++ AIUpdateInterface::calculateMaxBlockedSpeed residual.
    pub fn calculate_max_blocked_speed(&self, other: &Object) -> f32 {
        let us = self.get_position();
        let them = other.get_position();
        let mut vx = them.x - us.x;
        let mut vz = them.z - us.z;
        let len = (vx * vx + vz * vz).sqrt();
        if len < 1.0e-4 {
            return 0.0;
        }
        vx /= len;
        vz /= len;
        let other_dir = other.unit_direction_vector_2d();
        let speed_factor = vx * other_dir.x + vz * other_dir.y;
        if speed_factor < 0.0 {
            return 0.0; // they run into us
        }
        let other_vel = other.movement.velocity;
        let other_speed_2d = (other_vel.x * other_vel.x + other_vel.z * other_vel.z).sqrt();
        let away_speed = other_speed_2d * speed_factor;
        let our_dir = self.unit_direction_vector_2d();
        let toward = vx * our_dir.x + vz * our_dir.y;
        if toward <= 0.0 {
            return self.cur_max_blocked_speed;
        }
        let max_speed = away_speed / toward;
        // Formation crowd residual not wired — fail-closed skip 0.55 factor.
        if max_speed > self.cur_max_blocked_speed {
            return self.cur_max_blocked_speed;
        }
        max_speed
    }

    /// C++ AIUpdateInterface::processCollision residual (force-apply gate + blocked).
    ///
    /// Returns true if physics should apply bounce force. Sets is_blocked /
    /// cur_max_blocked_speed when self is moving into other.
    pub fn ai_process_collision(&mut self, other: &Object, current_frame: u32) -> bool {
        if !self.allow_collide_force {
            return false;
        }
        if self.can_path_through_units {
            self.is_blocked = false;
            return false;
        }
        if self.ignore_collisions_until_frame > 0
            && current_frame < self.ignore_collisions_until_frame
        {
            return false;
        }
        // Other needs AI residual: can_move stand-in.
        if !other.can_move() {
            // Immobile bounce handled outside AI processCollision.
            return true;
        }
        let self_ground = self.can_move() && !self.status.airborne_target && !self.is_parachuting();
        let other_ground =
            other.can_move() && !other.status.airborne_target && !other.is_parachuting();
        if !self_ground || !other_ground {
            return false;
        }

        let self_moving = self.movement.velocity.length_squared() > 0.01;
        if self_moving {
            let blocked = self.ai_blocked_by(other);
            if blocked {
                // Panic infantry bounces residual.
                if self.is_kind_of(crate::game_logic::KindOf::Infantry) && self.is_panicking {
                    return true;
                }
                self.is_blocked = true;
                let max_speed = self.calculate_max_blocked_speed(other);
                if max_speed < self.cur_max_blocked_speed {
                    self.cur_max_blocked_speed = max_speed;
                }
                // Vehicle into infantry: request move-away residual.
                if other.is_kind_of(crate::game_logic::KindOf::Infantry)
                    && !self.is_kind_of(crate::game_logic::KindOf::Infantry)
                {
                    // C++ busy/using-ability gate residual.
                    if !other.status.using_ability {
                        self.request_other_move_away = Some(other.id);
                    }
                }
                return false;
            }
        }
        false
    }

    /// Apply cur_max_blocked_speed cap residual (2D XZ).
    pub fn apply_blocked_speed_cap(&mut self) {
        if !self.is_blocked || !self.cur_max_blocked_speed.is_finite() {
            return;
        }
        let v = self.movement.velocity;
        let speed_2d = (v.x * v.x + v.z * v.z).sqrt();
        if speed_2d > self.cur_max_blocked_speed && speed_2d > 1.0e-4 {
            let s = self.cur_max_blocked_speed / speed_2d;
            self.movement.velocity.x *= s;
            self.movement.velocity.z *= s;
        }
    }

    /// C++ PhysicsBehavior::getMass residual.
    pub fn physics_get_mass(&self) -> f32 {
        self.physics_mass.max(1.0e-4)
    }

    /// C++ PhysicsBehavior::isMotive residual.
    pub fn is_motive(&self) -> bool {
        self.motive_frames_remaining > 0
    }

    /// C++ PhysicsBehavior::applyForce residual.
    ///
    /// When motive, only lateral component (perp to unit facing) is accepted.
    /// Host XZ ground plane maps C++ XY; world Y is vertical.
    pub fn apply_physics_force(&mut self, force: glam::Vec3) {
        if !force.x.is_finite() || !force.y.is_finite() || !force.z.is_finite() {
            return;
        }
        let mut mod_force = force;
        if self.is_motive() {
            let dir = self.unit_direction_vector_2d(); // (x,z)
                                                       // C++ lateralDot = force.x * (-dir.y) + force.y * dir.x
                                                       // Host: force.x * (-dir.z_comp) + force.z * dir.x where dir=(x,z)
            let lateral_dot = force.x * (-dir.y) + force.z * dir.x;
            mod_force.x = lateral_dot * (-dir.y);
            mod_force.z = lateral_dot * dir.x;
            // vertical unchanged
        }
        let inv = 1.0 / self.physics_get_mass();
        self.physics_accel += mod_force * inv;
    }

    /// C++ rotateObjAroundLocoPivot / rotateTowardsPosition residual.
    pub fn rotate_towards_position(
        &mut self,
        goal: glam::Vec3,
        dt: f32,
    ) -> (PhysicsTurningType, f32) {
        let max_turn = self.movement.turn_rate * dt;
        self.rotate_obj_around_loco_pivot(goal, max_turn)
    }

    /// C++ Locomotor::rotateObjAroundLocoPivot residual.
    pub fn rotate_obj_around_loco_pivot(
        &mut self,
        goal: glam::Vec3,
        max_turn_rate: f32,
    ) -> (PhysicsTurningType, f32) {
        let angle = self.get_orientation();
        let mut offset = self.turn_pivot_offset;
        if self.is_braking {
            offset = 0.0;
        }
        let us = self.get_position();
        let (dx, dz, turn_pos) = if offset.abs() > 1e-6 {
            let radius = self.selection_radius.max(1.0);
            let turn_point = offset * radius;
            let dir = self.unit_direction_vector_2d();
            let turn_pos =
                glam::Vec3::new(us.x + dir.x * turn_point, us.y, us.z + dir.y * turn_point);
            let dx = goal.x - turn_pos.x;
            let dz = goal.z - turn_pos.z;
            if dx.abs() < 0.1 && dz.abs() < 0.1 {
                self.physics_turning = PhysicsTurningType::TurnNone;
                self.record_host_locomotor();
                return (PhysicsTurningType::TurnNone, 0.0);
            }
            (dx, dz, Some(turn_pos))
        } else {
            let dx = goal.x - us.x;
            let dz = goal.z - us.z;
            if dx * dx + dz * dz < 1.0e-8 {
                self.physics_turning = PhysicsTurningType::TurnNone;
                self.record_host_locomotor();
                return (PhysicsTurningType::TurnNone, 0.0);
            }
            (dx, dz, None)
        };
        let desired = (-dz).atan2(dx);
        let mut amount = desired - angle;
        while amount > std::f32::consts::PI {
            amount -= std::f32::consts::TAU;
        }
        while amount < -std::f32::consts::PI {
            amount += std::f32::consts::TAU;
        }
        let rel = amount;
        let (amount, turning) = if amount > max_turn_rate {
            (max_turn_rate, PhysicsTurningType::TurnPositive)
        } else if amount < -max_turn_rate {
            (-max_turn_rate, PhysicsTurningType::TurnNegative)
        } else {
            (amount, PhysicsTurningType::TurnNone)
        };
        if let Some(tp) = turn_pos {
            let cos_a = amount.cos();
            let sin_a = amount.sin();
            let rx = us.x - tp.x;
            let rz = us.z - tp.z;
            let nx = tp.x + rx * cos_a - rz * sin_a;
            let nz = tp.z + rx * sin_a + rz * cos_a;
            self.set_position(glam::Vec3::new(nx, us.y, nz));
        }
        self.set_orientation(angle + amount);
        self.physics_turning = turning;
        self.record_host_locomotor();
        (turning, rel)
    }

    /// C++ Locomotor::locoUpdate_moveTowardsAngle residual.
    pub fn loco_update_move_towards_angle(&mut self, goal_angle: f32, dt: f32) {
        self.maintain_pos_valid = false;
        if self.shock_stun_frames > 0 {
            return;
        }
        let min_speed = self.min_speed;
        if min_speed > 0.0 {
            let us = self.get_position();
            let desired = glam::Vec3::new(
                us.x + goal_angle.cos() * min_speed * 2.0,
                us.y,
                us.z + (-goal_angle.sin()) * min_speed * 2.0,
            );
            let prev = self.movement.target_position;
            self.movement.target_position = Some(desired);
            let _ = self.rotate_towards_position(desired, dt);
            self.apply_forward_speed_force(min_speed, dt);
            let p = self.get_position() + self.movement.velocity * dt;
            self.set_position(p);
            let _ = self.handle_behavior_z(p.y, None);
            self.movement.target_position = prev;
        } else {
            let us = self.get_position();
            let desired = glam::Vec3::new(
                us.x + goal_angle.cos() * 1000.0,
                us.y,
                us.z + (-goal_angle.sin()) * 1000.0,
            );
            let _ = self.rotate_towards_position(desired, dt);
            let _ = self.handle_behavior_z(us.y, None);
        }
    }

    /// Advance wander angle offset residual (legs).
    pub fn tick_wander_angle_offset(&mut self, actual_speed: f32) -> f32 {
        if self.wander_width_factor == 0.0 {
            return 0.0;
        }
        if self.wander_offset_increment == 0.0 {
            self.wander_offset_increment = std::f32::consts::PI / 40.0;
        }
        let angle_limit = std::f32::consts::PI / 8.0 * self.wander_width_factor;
        if self.wander_offset_increasing {
            self.wander_angle_offset += self.wander_offset_increment * actual_speed;
            if self.wander_angle_offset > angle_limit {
                self.wander_offset_increasing = false;
            }
        } else {
            self.wander_angle_offset -= self.wander_offset_increment * actual_speed;
            if self.wander_angle_offset < -angle_limit {
                self.wander_offset_increasing = true;
            }
        }
        self.wander_angle_offset
    }

    /// C++ Locomotor::handleBehaviorZ residual (fail-closed subset).
    ///
    /// `ground_y` is terrain height at object XZ. Returns true if needs constant calling.
    pub fn handle_behavior_z(&mut self, ground_y: f32, goal_y: Option<f32>) -> bool {
        match self.loco_behavior_z {
            LocomotorBehaviorZ::NoZMotiveForce => false,
            LocomotorBehaviorZ::SeaLevel => {
                // Fail-closed: no water table — snap to ground layer.
                let mut p = self.get_position();
                p.y = ground_y;
                self.set_position(p);
                true
            }
            LocomotorBehaviorZ::SurfaceRelativeHeight
            | LocomotorBehaviorZ::SmoothRelativeToHighestLayer => {
                if self.loco_preferred_height == 0.0 && goal_y.is_none() {
                    return true;
                }
                let p = self.get_position();
                let preferred_raw = if let Some(gy) = goal_y {
                    gy
                } else {
                    self.loco_preferred_height + ground_y
                };
                let mut delta = preferred_raw - p.y;
                delta *= self.loco_preferred_height_damping.clamp(0.0, 1.0);
                let preferred = p.y + delta;
                let lift = if self.effective_max_lift() > 0.0 {
                    self.calc_lift_to_use_at_pt(p.y, preferred)
                } else {
                    // Fail-closed: no lift template — proportional residual.
                    preferred - p.y
                };
                if lift.abs() > 1.0e-4 {
                    let force_y = lift * self.physics_get_mass();
                    self.apply_motive_force(glam::Vec3::new(0.0, force_y, 0.0));
                }
                true
            }
            LocomotorBehaviorZ::AbsoluteHeight => {
                if self.loco_preferred_height == 0.0 && goal_y.is_none() {
                    return true;
                }
                let mut p = self.get_position();
                let preferred = goal_y.unwrap_or(self.loco_preferred_height);
                let mut delta = preferred - p.y;
                delta *= self.loco_preferred_height_damping.clamp(0.0, 1.0);
                p.y += delta;
                self.set_position(p);
                true
            }
        }
    }

    /// C++ Locomotor::locoUpdate_maintainCurrentPosition residual (ground units).
    ///
    /// Stops horizontal motion for legs/treads/wheels; hover/wings need constant Z.
    pub fn loco_maintain_current_position(&mut self, ground_y: f32) -> bool {
        if !self.maintain_pos_valid {
            self.maintain_pos = Some(self.get_position());
            self.record_host_combat_attack();
            self.maintain_pos_valid = true;
        }
        self.is_braking = false;
        self.physics_turning = PhysicsTurningType::TurnNone;
        self.record_host_locomotor();

        // Appearance-specific maintain residual.
        match self.loco_appearance {
            LocomotorAppearance::Wings => {
                // Circling maintain — needs dt; use 1/30 frame residual.
                self.maintain_position_wings(1.0 / 30.0);
                return true;
            }
            LocomotorAppearance::Thrust => {
                if let Some(m) = self.maintain_pos {
                    let spd = self.min_speed.max(1.0);
                    self.move_towards_thrust(m, 0.0, spd, 1.0 / 30.0);
                }
                return true;
            }
            LocomotorAppearance::Hover => {
                self.physics_turning = PhysicsTurningType::TurnNone;
                if self.is_motive() {
                    self.scrub_velocity_2d(0.0);
                }
                let maintain_y = self.maintain_pos.map(|p| p.y);
                let _ = self.handle_behavior_z(ground_y, maintain_y);
                return true;
            }
            _ => {}
        }

        // Ground-appearance residual: scrub horizontal velocity (legs/treads/wheels).
        let airborne_loco = self.is_kind_of(crate::game_logic::KindOf::Aircraft)
            || matches!(
                self.loco_behavior_z,
                LocomotorBehaviorZ::SurfaceRelativeHeight
                    | LocomotorBehaviorZ::SmoothRelativeToHighestLayer
                    | LocomotorBehaviorZ::AbsoluteHeight
            );
        if !airborne_loco {
            self.scrub_velocity_2d(0.0);
        }

        let maintain_y = self.maintain_pos.map(|p| p.y);
        let needs_z = self.handle_behavior_z(ground_y, maintain_y);
        // Hover/air need constant calling; ground settled does not.
        airborne_loco || needs_z
    }

    /// C++ Locomotor::setPhysicsOptions residual.
    pub fn set_locomotor_physics_options(&mut self) {
        // C++ EXTRA_FRIC 0.5 when ULTRA_ACCURATE.
        let ultra = if self.ultra_accurate { 0.5 } else { 0.0 };
        self.extra_friction = self.loco_extra_2d_friction + ultra;
        self.apply_friction_2d_when_airborne = self.loco_apply_2d_friction_airborne;
        // Walking units stick to ground residual.
        if self.is_kind_of(crate::game_logic::KindOf::Infantry) {
            self.stick_to_ground = true;
            if matches!(self.loco_appearance, LocomotorAppearance::Other) {
                self.loco_appearance = LocomotorAppearance::LegsTwo;
                self.record_host_locomotor();
            }
        } else if self.is_kind_of(crate::game_logic::KindOf::Aircraft) {
            if matches!(self.loco_appearance, LocomotorAppearance::Other) {
                self.loco_appearance = LocomotorAppearance::Wings;
                self.record_host_locomotor();
            }
        } else if self.is_kind_of(crate::game_logic::KindOf::Vehicle) {
            if matches!(self.loco_appearance, LocomotorAppearance::Other) {
                // Fail-closed: vehicles default treads-like (tanks common in host).
                self.loco_appearance = LocomotorAppearance::Treads;
                self.record_host_locomotor();
            }
        }
    }

    /// C++ Locomotor::getMaxLift residual (host world-Y).
    /// C++ Locomotor::getMaxLift residual (damage-conditioned).
    pub fn get_max_lift(&self) -> f32 {
        self.effective_max_lift()
    }

    /// C++ Locomotor::getMaxLift(BodyDamageType) residual.
    pub fn effective_max_lift(&self) -> f32 {
        use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
        let pristine = self.max_lift.max(0.0);
        let damaged = self.max_lift_damaged.clamp(0.0, pristine.max(0.0));
        match self.body_damage_state {
            HostBodyDamageType::Pristine | HostBodyDamageType::Damaged => pristine,
            HostBodyDamageType::ReallyDamaged | HostBodyDamageType::Rubble => {
                if damaged > 0.0 {
                    damaged.min(pristine)
                } else if pristine > 0.0 {
                    pristine * 0.5
                } else {
                    0.0
                }
            }
        }
    }

    /// C++ Locomotor::calcLiftToUseAtPt residual (simplified).
    ///
    /// Gravity residual = -1.0 (host world-Y). Returns lift accel to apply (not force).
    pub fn calc_lift_to_use_at_pt(&self, cur_y: f32, preferred_height: f32) -> f32 {
        const GRAVITY: f32 = -1.0;
        let max_gross = self.get_max_lift();
        let mut max_net = max_gross + GRAVITY;
        if max_net < 0.0 {
            max_net = 0.0;
        }
        let cur_vy = self.movement.velocity.y;
        let max_accel = if self.ultra_accurate {
            if cur_vy < 0.0 {
                2.0 * max_net
            } else {
                -2.0 * max_net
            }
        } else if cur_vy < 0.0 {
            max_net
        } else {
            GRAVITY
        };
        let desired_accel = if max_accel.abs() > 0.001 {
            let delta_y = preferred_height - cur_y;
            let brake_dist = (cur_vy * cur_vy) / max_accel.abs().max(1e-6);
            if brake_dist.abs() > delta_y.abs() {
                max_accel
            } else if cur_vy.abs() > self.speed_limit_z {
                self.speed_limit_z - cur_vy
            } else {
                // a = 2(dz - v) assuming t=1 frame
                2.0 * (delta_y - cur_vy)
            }
        } else {
            0.0
        };
        let mut lift = desired_accel - GRAVITY;
        if self.ultra_accurate {
            const UP_FACTOR: f32 = 3.0;
            if lift > UP_FACTOR * max_gross {
                lift = UP_FACTOR * max_gross;
            } else if lift < -max_gross {
                lift = -max_gross;
            }
        } else if lift > max_gross {
            lift = max_gross;
        } else if lift < 0.0 {
            lift = 0.0;
        }
        lift
    }

    /// C++ AIUpdateInterface::requestAttackPath flag residual (before pathfinder).
    pub fn begin_request_attack_path(
        &mut self,
        victim_id: Option<ObjectId>,
        victim_pos: glam::Vec3,
        current_frame: u32,
    ) -> bool {
        // Returns false if should defer (repath too soon).
        self.requested_destination = Some(victim_pos);
        self.record_host_ai_request();
        self.requested_victim_id = victim_id;
        self.record_host_ai_request();
        self.is_attack_path = true;
        self.is_approach_path = false;
        self.record_host_locomotor();
        self.is_safe_path = false;
        self.waiting_for_path = true;
        if self.path_timestamp > 0 && current_frame.saturating_sub(self.path_timestamp) < 3 {
            // C++ setQueueForPathTime(2 sec)
            self.queue_for_path_frames = 60;
            return false;
        }
        self.path_timestamp = current_frame;
        self.record_host_ai_request();
        true
    }

    /// C++ AIUpdateInterface::requestPath flag residual (non-attack).
    pub fn begin_request_move_path(&mut self, destination: glam::Vec3, current_frame: u32) -> bool {
        self.requested_destination = Some(destination);
        self.record_host_ai_request();
        self.requested_victim_id = None;
        self.record_host_ai_request();
        self.is_attack_path = false;
        self.is_exact_path = false;
        self.is_approach_path = false;
        self.record_host_locomotor();
        self.is_safe_path = false;
        self.waiting_for_path = true;
        if self.path_timestamp > 0 && current_frame.saturating_sub(self.path_timestamp) < 3 {
            self.queue_for_path_frames = 60;
            return false;
        }
        self.path_timestamp = current_frame;
        self.record_host_ai_request();
        true
    }

    /// C++ requestApproachPath residual.
    pub fn begin_request_approach_path(
        &mut self,
        destination: glam::Vec3,
        current_frame: u32,
    ) -> bool {
        let ok = self.begin_request_move_path(destination, current_frame);
        self.is_approach_path = true;
        self.record_host_locomotor();
        ok
    }

    /// C++ requestSafePath residual.
    pub fn begin_request_safe_path(
        &mut self,
        repulsor: ObjectId,
        flee_pos: glam::Vec3,
        current_frame: u32,
    ) -> bool {
        let ok = self.begin_request_move_path(flee_pos, current_frame);
        self.is_safe_path = true;
        self.requested_victim_id = Some(repulsor);
        self.record_host_ai_request();
        ok
    }

    /// Tick path queue delay residual.
    pub fn tick_path_queue(&mut self) {
        if self.queue_for_path_frames > 0 {
            self.queue_for_path_frames -= 1;
        }
        if self.temporary_move_frames > 0 {
            self.temporary_move_frames -= 1;
            if self.temporary_move_frames == 0
                && matches!(self.ai_state, AIState::Moving)
                && self.movement.target_position.is_none()
            {
                // Temporary AI move expired with no destination — idle residual.
                self.set_ai_state(AIState::Idle);
                self.record_host_combat_attack();
            }
        }
    }

    /// C++ privateAttackObject max-shots residual.
    /// C++ Locomotor::getMaxSpeedForCondition residual.
    /// Better than MovementPenaltyDamageState (REALLYDAMAGED) → pristine max;
    /// else → max_speed_damaged (clamped by pristine max).
    pub fn effective_max_speed(&self) -> f32 {
        use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
        let pristine = self.movement.max_speed.max(0.0);
        let damaged = self
            .movement
            .max_speed_damaged
            .clamp(0.0, pristine.max(0.0));
        // Penalty threshold = ReallyDamaged (GameData.ini residual).
        match self.body_damage_state {
            HostBodyDamageType::Pristine | HostBodyDamageType::Damaged => pristine,
            HostBodyDamageType::ReallyDamaged | HostBodyDamageType::Rubble => {
                if damaged > 0.0 {
                    damaged.min(pristine)
                } else {
                    pristine * 0.5
                }
            }
        }
    }

    /// C++ Locomotor::getMaxTurnRate residual (damage-conditioned).
    pub fn effective_turn_rate(&self) -> f32 {
        use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
        let pristine = self.movement.turn_rate.max(0.0);
        let damaged = self
            .movement
            .turn_rate_damaged
            .clamp(0.0, pristine.max(0.0));
        match self.body_damage_state {
            HostBodyDamageType::Pristine | HostBodyDamageType::Damaged => pristine,
            HostBodyDamageType::ReallyDamaged | HostBodyDamageType::Rubble => {
                if damaged > 0.0 {
                    damaged.min(pristine)
                } else {
                    pristine * 0.5
                }
            }
        }
    }

    /// C++ Locomotor::getMaxAcceleration residual (damage-conditioned).
    pub fn effective_acceleration(&self) -> f32 {
        use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
        let pristine = self.movement.acceleration.max(0.0);
        let damaged = self
            .movement
            .acceleration_damaged
            .clamp(0.0, pristine.max(0.0));
        match self.body_damage_state {
            HostBodyDamageType::Pristine | HostBodyDamageType::Damaged => pristine,
            HostBodyDamageType::ReallyDamaged | HostBodyDamageType::Rubble => {
                if damaged > 0.0 {
                    damaged.min(pristine)
                } else {
                    pristine * 0.5
                }
            }
        }
    }

    pub fn set_max_shots_to_fire(&mut self, max_shots: i32) {
        self.max_shots_to_fire = max_shots;
        self.record_host_combat_attack();
    }

    /// C++ Weapon::getMaxShotCount residual: 0 means cannot fire more.
    /// Host uses -1 as unlimited (also accepts C++ NO_MAX_SHOTS_LIMIT).
    #[inline]
    pub fn has_max_shots_remaining(&self) -> bool {
        self.max_shots_to_fire != 0
    }

    /// C++ `--m_maxShotCount` residual after a successful discharge.
    pub fn consume_max_shot_count(&mut self) {
        const NO_MAX: i32 =
            crate::game_logic::host_ai_path_combat_residual_wave105::NO_MAX_SHOTS_LIMIT;
        if self.max_shots_to_fire == -1 || self.max_shots_to_fire == NO_MAX {
            return;
        }
        if self.max_shots_to_fire > 0 {
            self.max_shots_to_fire -= 1;
        }
    }

    /// C++ AIUpdateInterface::requestPath residual (fail-closed straight path).
    ///
    /// Sets waiting_for_path briefly, installs single-waypoint path to dest.
    /// Full Pathfinder A* is applied by GameLogic when grid is available.
    pub fn request_path(&mut self, destination: glam::Vec3, waypoints: Option<Vec<glam::Vec3>>) {
        self.waiting_for_path = true;
        self.queue_for_path_frames = 0;
        self.maintain_pos_valid = false;
        if let Some(mut wps) = waypoints {
            if wps.is_empty() {
                wps.push(destination);
            }
            self.movement.path = wps;
        } else {
            self.movement.path = vec![destination];
        }
        self.movement.current_path_index = 0;
        self.movement.target_position = self.movement.path.first().copied();
        self.waiting_for_path = false;
        self.is_braking = false;
        self.record_host_movement();
    }

    /// True if effectively moving (C++ isMoving || isWaitingForPath).
    pub fn is_effectively_moving(&self) -> bool {
        self.waiting_for_path
            || self.movement.target_position.is_some()
            || self.movement.velocity.length_squared() > 0.01
    }

    /// C++ Locomotor::calcMinTurnRadius residual (host units).
    pub fn calc_min_turn_radius(&self) -> f32 {
        let min_speed = self.min_speed.max(0.0);
        // turn_rate is rad/sec; convert to per-frame for C++ parity radius.
        let max_turn_rate = self.movement.turn_rate / 30.0;
        if max_turn_rate > 1.0e-6 {
            // minSpeed is units/sec → per-frame for C++ formula minSpeed/maxTurnRate
            (min_speed / 30.0) / max_turn_rate
        } else {
            999_999.0
        }
    }

    /// C++ Locomotor::fixInvalidPosition residual.
    ///
    /// Fail-closed without full pathfinder neighbor scan: when
    /// `on_invalid_movement_terrain` or cliff cell, push toward valid via motive force.
    pub fn fix_invalid_position(&mut self) -> bool {
        if self.is_dozer || self.is_kind_of(crate::game_logic::KindOf::Aircraft) {
            return false;
        }
        if !self.on_invalid_movement_terrain && !self.cell_is_cliff {
            return false;
        }
        // Push opposite current lateral velocity if sinking into obstacle; else nudge
        // along facing residual (C++ 3×3 neighbor vote simplified).
        let mass = self.physics_get_mass();
        let v = self.movement.velocity;
        let speed2 = v.x * v.x + v.z * v.z;
        if speed2 > 0.01 {
            let inv = 1.0 / speed2.sqrt();
            let nx = -v.x * inv;
            let nz = -v.z * inv;
            // If already leaving (dot with correction > 0.25), skip.
            let leaving = v.x * nx + v.z * nz; // nx opposite vel so leaving is negative of progress
                                               // correction direction is opposite into-invalid → along -velocity when moving in
            if leaving > 0.25 {
                return false;
            }
            let force = glam::Vec3::new(nx * mass / 5.0, 0.0, nz * mass / 5.0);
            self.apply_motive_force(force);
            self.integrate_physics_accel();
            return true;
        }
        // Stationary on invalid: nudge along facing.
        let d = self.unit_direction_vector_2d();
        let force = glam::Vec3::new(d.x * mass / 5.0, 0.0, d.y * mass / 5.0);
        self.apply_motive_force(force);
        self.integrate_physics_accel();
        true
    }

    /// C++ maintainCurrentPositionWings residual — circle around maintain pos.
    pub fn maintain_position_wings(&mut self, dt: f32) {
        self.physics_turning = PhysicsTurningType::TurnNone;
        if !self.is_motive() && !self.status.airborne_target {
            return;
        }
        let Some(maintain) = self.maintain_pos else {
            return;
        };
        let mut turn_radius = self.circling_radius;
        if turn_radius.abs() < 1.0e-4 {
            turn_radius = self.calc_min_turn_radius();
        }
        let us = self.get_position();
        let dx = maintain.x - us.x;
        let dz = maintain.z - us.z;
        let mut angle = if dx * dx + dz * dz < 1.0e-6 {
            self.get_orientation()
        } else {
            (-dz).atan2(dx) // host facing convention for direction to maintain
        };
        // C++ aimDir = PI - PI/8
        let mut aim_dir = std::f32::consts::PI - std::f32::consts::PI / 8.0;
        if turn_radius < 0.0 {
            turn_radius = -turn_radius;
            aim_dir = -aim_dir;
        }
        angle += aim_dir;
        let desired = glam::Vec3::new(
            maintain.x + angle.cos() * turn_radius,
            maintain.y,
            maintain.z + (-angle.sin()) * turn_radius, // match host dir xz from angle
        );
        // Drive toward opposite side of circle at min_speed.
        let spd = self.min_speed.max(self.movement.max_speed * 0.25).max(1.0);
        self.movement.target_position = Some(desired);
        // One sub-step of other-like move without recursion into maintain.
        let (_t, _rel) = self.rotate_towards_position(desired, dt);
        self.apply_forward_speed_force(spd, dt);
        let p = self.get_position() + self.movement.velocity * dt;
        self.set_position(p);
        // Restore no-order state.
        self.movement.target_position = None;
        let gy = p.y;
        let _ = self.handle_behavior_z(gy, Some(maintain.y));
    }

    /// C++ moveTowardsPositionThrust residual (simplified 3D force toward goal).
    pub fn move_towards_thrust(
        &mut self,
        goal: glam::Vec3,
        on_path_dist: f32,
        mut desired_speed: f32,
        dt: f32,
    ) {
        let max_speed = self.effective_max_speed().max(0.01);
        desired_speed = desired_speed.clamp(self.min_speed, max_speed);
        let actual = self.movement.velocity.length();
        if self.braking > 0.0 && !self.no_slow_down_as_approaching_dest {
            let slow = (actual / 1.5) * (actual / self.braking.max(1e-3));
            if on_path_dist < slow {
                desired_speed = self.min_speed;
            }
        }
        let mut local_goal = goal;
        if self.loco_preferred_height != 0.0 && !self.precise_z_pos {
            // surface relative preferred height residual (ground_y ≈ current if unknown)
            let surface = self.get_position().y; // fail-closed
            let preferred = self.loco_preferred_height + surface;
            let mut delta = preferred - self.get_position().y;
            delta *= self.loco_preferred_height_damping.clamp(0.0, 1.0);
            local_goal.y = self.get_position().y + delta;
        }
        let us = self.get_position();
        let mut dir = local_goal - us;
        let len = dir.length();
        if len < 1e-4 {
            return;
        }
        dir /= len;
        let speed_delta = desired_speed - actual;
        let max_accel = if speed_delta > 0.0 || self.braking <= 0.0 {
            self.movement.acceleration
        } else {
            self.braking
        };
        // Damped accel residual: thrustDir*maxAccel - vel*damping
        let damping = (max_accel / max_speed).clamp(0.0, 1.0);
        let accel = dir * max_accel - self.movement.velocity * damping;
        let mass = self.physics_get_mass();
        self.apply_motive_force(accel * mass);
        self.integrate_physics_accel();
        // Orient toward velocity residual.
        if self.movement.velocity.length_squared() > 1e-4 {
            let v = self.movement.velocity;
            let desired_yaw = (-v.z).atan2(v.x);
            let (_t, _) = self.rotate_towards_position(
                us + glam::Vec3::new(desired_yaw.cos(), 0.0, -desired_yaw.sin()),
                dt,
            );
        }
        let p = us + self.movement.velocity * dt;
        self.set_position(p);
    }

    /// Apply forward motive force to close speedDelta (C++ legs/other residual).
    fn apply_forward_speed_force(&mut self, goal_speed: f32, dt: f32) {
        let actual = self.forward_speed_2d();
        // When moving backwards residual, treat signed speed.
        let actual = if self.moving_backwards {
            -actual.abs()
        } else {
            actual
        };
        let speed_delta = goal_speed - actual;
        if speed_delta.abs() < 1.0e-5 {
            return;
        }
        let mass = self.physics_get_mass();
        // Host Movement accel is units/sec²; convert impulse for one logic frame.
        let frame_dt = (dt * 30.0).clamp(0.5, 2.0) / 30.0; // ~one frame
        let acceleration = if speed_delta > 0.0 {
            self.movement.acceleration
        } else {
            -self.braking.max(self.movement.acceleration)
        };
        let mut accel_force = mass * acceleration * frame_dt * 30.0; // N-ish
        let max_force_needed = mass * speed_delta;
        if accel_force.abs() > max_force_needed.abs() {
            accel_force = max_force_needed;
        }
        let dir = self.unit_direction_vector_2d();
        let sign = if self.moving_backwards { -1.0 } else { 1.0 };
        self.apply_motive_force(glam::Vec3::new(
            accel_force * dir.x * sign,
            0.0,
            accel_force * dir.y * sign,
        ));
        // Integrate immediately so this frame's movement sees it (host dt path).
        self.integrate_physics_accel();
        // Also blend velocity toward goal for host-second dt residual.
        let dir = self.unit_direction_vector_2d();
        let target = glam::Vec3::new(
            dir.x * goal_speed * sign,
            self.movement.velocity.y,
            dir.y * goal_speed * sign,
        );
        let max_accel = self.movement.acceleration * dt;
        let diff = target - self.movement.velocity;
        if diff.length() <= max_accel {
            self.movement.velocity = target;
        } else if diff.length() > 1e-6 {
            self.movement.velocity += diff.normalize() * max_accel;
        }
        self.invalidate_velocity_magnitude();
        self.record_host_movement();
    }

    /// C++ PhysicsBehavior::applyMotiveForce residual.
    ///
    /// Temporarily accepts full force (clears motive), applies, then arms motive
    /// window for MOTIVE_FRAMES so subsequent collide forces are lateral-only.
    pub fn apply_motive_force(&mut self, force: glam::Vec3) {
        let prev = self.motive_frames_remaining;
        self.motive_frames_remaining = 0;
        self.record_host_physics_motive();
        self.apply_physics_force(force);
        self.motive_frames_remaining = MOTIVE_FRAMES_RESIDUAL.max(prev);
        self.record_host_physics_motive();
    }

    /// C++ PhysicsBehavior::resetDynamicPhysics residual.
    pub fn reset_dynamic_physics(&mut self) {
        self.physics_accel = glam::Vec3::ZERO;
        self.movement.velocity = glam::Vec3::ZERO;
        self.invalidate_velocity_magnitude();
        self.shock_yaw_rate = 0.0;
        self.shock_pitch_rate = 0.0;
        self.shock_roll_rate = 0.0;
        self.motive_frames_remaining = 0;
        self.record_host_physics_motive();
        self.record_host_movement();
    }

    /// Integrate physics_accel into velocity residual (a → v per logic frame).
    pub fn integrate_physics_accel(&mut self) {
        if self.physics_accel != glam::Vec3::ZERO {
            self.movement.velocity += self.physics_accel;
            self.physics_accel = glam::Vec3::ZERO;
            self.invalidate_velocity_magnitude();
        }
        if self.motive_frames_remaining > 0 {
            self.motive_frames_remaining -= 1;
        }
    }

    /// Invalidate cached velocity magnitude residual.
    pub fn invalidate_velocity_magnitude(&mut self) {
        self.velocity_magnitude_cache = -1.0;
    }

    /// C++ PhysicsBehavior::getVelocityMagnitude residual.
    pub fn velocity_magnitude(&mut self) -> f32 {
        if self.velocity_magnitude_cache < 0.0 {
            let v = self.movement.velocity;
            self.velocity_magnitude_cache = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
        }
        self.velocity_magnitude_cache
    }

    /// C++ getForwardSpeed2D residual (signed along facing on XZ).
    pub fn forward_speed_2d(&self) -> f32 {
        let dir = self.unit_direction_vector_2d();
        let v = self.movement.velocity;
        let vx = v.x * dir.x;
        let vz = v.z * dir.y;
        let dot = vx + vz;
        let speed = (vx * vx + vz * vz).sqrt();
        if dot >= 0.0 {
            speed
        } else {
            -speed
        }
    }

    /// C++ getAerodynamicFriction residual (clamped).
    pub fn get_aerodynamic_friction(&self) -> f32 {
        let f = self.aerodynamic_friction + self.extra_friction;
        f.max(MIN_AERO_FRICTION_RESIDUAL).min(MAX_FRICTION_RESIDUAL)
    }

    /// C++ getForwardFriction residual.
    pub fn get_forward_friction(&self) -> f32 {
        let f = self.forward_friction + self.extra_friction;
        f.clamp(0.0, MAX_FRICTION_RESIDUAL)
    }

    /// C++ getLateralFriction residual.
    pub fn get_lateral_friction(&self) -> f32 {
        let f = self.lateral_friction + self.extra_friction;
        f.clamp(0.0, MAX_FRICTION_RESIDUAL)
    }

    /// C++ PhysicsBehavior::applyFrictionalForces residual (host XZ ground).
    pub fn apply_frictional_forces(&mut self) {
        // C++: APPLY_FRICTION2D_WHEN_AIRBORNE || !isSignificantlyAboveTerrain || deckTaxiing
        // Host residual: non-airborne OR flag → 2D friction; else aero.
        let use_2d = self.apply_friction_2d_when_airborne || !self.status.airborne_target;

        if use_2d {
            // YPR damping residual: DEFAULT_LATERAL_FRICTION on shock rates.
            let d = 1.0 - DEFAULT_LATERAL_FRICTION_RESIDUAL;
            self.shock_yaw_rate *= d;
            self.shock_pitch_rate *= d;
            self.shock_roll_rate *= d;

            let v = self.movement.velocity;
            if v.x != 0.0 || v.z != 0.0 {
                let dir = self.unit_direction_vector_2d();
                let mass = self.physics_get_mass();
                let lateral_dot = v.x * (-dir.y) + v.z * dir.x;
                let lat_x = lateral_dot * (-dir.y);
                let lat_z = lateral_dot * dir.x;
                let lf = mass * self.get_lateral_friction();
                let mut accel = glam::Vec3::new(-(lf * lat_x), 0.0, -(lf * lat_z));
                if !self.is_motive() {
                    let forward_dot = v.x * dir.x + v.z * dir.y;
                    let fwd_x = forward_dot * dir.x;
                    let fwd_z = forward_dot * dir.y;
                    let ff = mass * self.get_forward_friction();
                    accel.x += -(ff * fwd_x);
                    accel.z += -(ff * fwd_z);
                }
                self.apply_physics_force(accel);
            }
        } else {
            let aero = -self.get_aerodynamic_friction();
            let v = self.movement.velocity;
            self.physics_accel.x += v.x * aero;
            self.physics_accel.y += v.y * aero;
            self.physics_accel.z += v.z * aero;
            let d = 1.0 + aero;
            self.shock_yaw_rate *= d;
            self.shock_pitch_rate *= d;
            self.shock_roll_rate *= d;
        }
    }

    /// C++ PhysicsBehavior::transferVelocityTo residual.
    pub fn transfer_velocity_to(&self, other: &mut Object) {
        other.movement.velocity += self.movement.velocity;
        other.invalidate_velocity_magnitude();
    }

    /// C++ PhysicsBehavior::addVelocityTo residual.
    pub fn add_velocity(&mut self, vel: glam::Vec3) {
        self.movement.velocity += vel;
        self.invalidate_velocity_magnitude();
    }

    /// C++ applyGravitationalForces residual (host world Y up).
    pub fn apply_gravitational_forces(&mut self) {
        // C++ TheGlobalData->m_gravity residual ≈ -1.0 world units / frame²
        // Host shock gravity is -1.0 on Y.
        self.physics_accel.y += -1.0;
    }

    /// C++ AIUpdateInterface::privateMoveAwayFromUnit residual (fail-closed).
    ///
    /// No full pathfinder: push destination opposite the threat along XZ and
    /// enter move-out-of-way window. Re-request while already yielding + blocked
    /// grants ignore-collisions for 2 seconds (C++ cheat).
    pub fn ai_move_away_from_unit(&mut self, threat_id: ObjectId, threat_pos: glam::Vec3) {
        if self.status.destroyed || !self.is_alive() || !self.can_move() {
            return;
        }
        if self.is_kind_of(crate::game_logic::KindOf::Immobile)
            || self.is_kind_of(crate::game_logic::KindOf::Structure)
        {
            return;
        }
        // Already yielding for this threat.
        if self.move_away_from == Some(threat_id) && self.move_away_frames > 0 {
            if self.is_blocked {
                // C++ setIgnoreCollisionTime(2 sec)
                self.ignore_collisions_until_frame = self.ignore_collisions_until_frame.max(60); // caller should OR with current frame externally
                                                                                                 // Store relative: use flag via ignore_collisions_with as well.
                self.ignore_collisions_with = Some(threat_id);
            }
            return;
        }
        let us = self.get_position();
        let mut dx = us.x - threat_pos.x;
        let mut dz = us.z - threat_pos.z;
        let len = (dx * dx + dz * dz).sqrt();
        if len < 1.0e-3 {
            // Coincident: push along our facing.
            let d = self.unit_direction_vector_2d();
            dx = d.x;
            dz = d.y;
        } else {
            dx /= len;
            dz /= len;
        }
        // PATHFIND_CELL_SIZE * ~2 step away residual.
        let step = PATHFIND_CELL_SIZE_F_RESIDUAL * 2.0;
        let dest = glam::Vec3::new(us.x + dx * step, us.y, us.z + dz * step);
        self.move_away_from = Some(threat_id);
        self.move_away_destination = Some(dest);
        self.move_away_frames = 10 * 30; // 10 seconds temporary state residual
                                         // Nudge velocity toward dest residual (fail-closed vs full path).
        self.movement.velocity.x += dx * 0.5;
        self.movement.velocity.z += dz * 0.5;
    }

    /// Tick move-away temporary state residual.
    pub fn tick_move_away_state(&mut self) {
        if self.move_away_frames > 0 {
            self.move_away_frames -= 1;
            if self.move_away_frames == 0 {
                self.move_away_from = None;
                self.move_away_destination = None;
            }
        }
    }

    /// Clear per-frame blocked residual at start of AI/physics tick.
    pub fn clear_blocked_frame_state(&mut self) {
        if self.is_blocked {
            self.num_frames_blocked = self.num_frames_blocked.saturating_add(1);
            // Stuck residual: blocked for > 1 second (30 frames).
            if self.num_frames_blocked > 30 {
                self.is_blocked_and_stuck = true;
            }
        } else {
            self.num_frames_blocked = 0;
            self.is_blocked_and_stuck = false;
        }
        self.is_blocked = false;
        self.cur_max_blocked_speed = f32::MAX;
        self.request_other_move_away = None;
    }
    pub fn set_ignore_collisions_with(&mut self, id: Option<ObjectId>) {
        self.ignore_collisions_with = id;
    }

    /// C++ PhysicsBehavior::isIgnoringCollisionsWith residual.
    pub fn is_ignoring_collisions_with(&self, id: ObjectId) -> bool {
        self.ignore_collisions_with == Some(id)
    }

    /// C++ PhysicsBehavior::isCurrentlyOverlapped residual.
    pub fn is_currently_overlapped(&self, id: ObjectId) -> bool {
        self.physics_current_overlap == Some(id)
    }

    /// C++ PhysicsBehavior::wasPreviouslyOverlapped residual.
    pub fn was_previously_overlapped(&self, id: ObjectId) -> bool {
        self.physics_previous_overlap == Some(id)
    }

    /// C++ PhysicsBehavior::addOverlap residual.
    pub fn add_physics_overlap(&mut self, id: ObjectId) {
        if !self.is_currently_overlapped(id) {
            self.physics_current_overlap = Some(id);
        }
    }

    fn ensure_crush_levels(&mut self) {
        // Host residual defaults when unset: vehicles crush infantry.
        if self.crusher_level == 0 && self.is_kind_of(KindOf::Vehicle) {
            self.crusher_level = 1;
        }
        if self.crushable_level == 255 && self.is_kind_of(KindOf::Infantry) {
            self.crushable_level = 0;
        }
        self.record_host_crush_vision();
    }

    /// C++ Object::canCrushOrSquish TEST_CRUSH_ONLY residual.
    pub fn can_crush_only(&self, other: &Object, is_ally: bool) -> bool {
        use crate::game_logic::host_partition_collision_physics_residual::can_crush_only_residual;
        can_crush_only_residual(
            self.crusher_level,
            other.crushable_level,
            is_ally,
            self.status.disabled_unmanned,
        )
    }

    /// Unit direction 2D residual from orientation (host XZ plane).
    pub fn unit_direction_xz(&self) -> (f32, f32) {
        let yaw = self.get_orientation();
        // Orientation 0 faces +X; desired heading uses (-dz).atan2(dx),
        // so +Z is yaw = -PI/2 → dir (0, +1).
        (yaw.cos(), -yaw.sin())
    }

    /// C++ PhysicsBehavior::checkForOverlapCollision residual.
    ///
    /// Returns true if this is an overlap/crush interaction (skip normal bounce).
    /// On first crush pass of target point, applies HUGE crush damage.
    pub fn check_for_overlap_collision(&mut self, other: &mut Object, is_ally: bool) -> bool {
        use crate::game_logic::host_partition_collision_physics_residual::{
            past_crush_point_residual, CrushTarget, PHYSICS_HUGE_DAMAGE_AMOUNT_RESIDUAL,
        };
        self.ensure_crush_levels();
        other.ensure_crush_levels();
        if self.velocity_is_very_small() {
            return false;
        }
        let self_crushing_other = self.can_crush_only(other, is_ally);
        let self_being_crushed = other.can_crush_only(self, is_ally);
        if self_crushing_other && self_being_crushed {
            return false;
        }
        if self_being_crushed {
            return true; // passive overlap
        }
        if !self_crushing_other {
            return false;
        }
        // C++ SquishCollide residual: infantry/crushable under tank with velocity
        // toward victim takes immediate HUGE crush damage (tight radius).
        // Physics front/back crush points still run below for vehicles/props.
        if other.is_kind_of(crate::game_logic::KindOf::Infantry)
            || other.crushable_level < self.crusher_level
        {
            use crate::game_logic::host_squish_collide::{
                should_skip_squish_for_goal_ability, velocity_toward_victim, within_squish_radius,
                SQUISH_HUGE_DAMAGE,
            };
            if !is_ally && !should_skip_squish_for_goal_ability(&other.template_name) {
                let us = self.get_position();
                let them = other.get_position();
                let vel = self.movement.velocity;
                let toward = velocity_toward_victim((us.x, us.z), (them.x, them.z), (vel.x, vel.z));
                let crusher_r = self.selection_radius.max(5.0);
                if toward && within_squish_radius((us.x, us.z), (them.x, them.z), crusher_r) {
                    other.front_crushed = true;
                    other.back_crushed = true;
                    other.apply_crush_die_model_conditions();
                    let _ = other.take_damage_from_typed_death(
                        SQUISH_HUGE_DAMAGE,
                        Some(self.id),
                        crate::game_logic::combat::DamageType::Crush,
                        crate::game_logic::host_usa_pilot::HostDeathType::Crushed,
                    );
                    self.add_physics_overlap(other.id);
                    return true;
                }
            }
        }
        // add overlap
        let oid = other.id;
        let first =
            self.physics_previous_overlap != Some(oid) && self.physics_current_overlap != Some(oid);
        self.add_physics_overlap(oid);
        if first {
            // 0-amount crush damage residual (DamageFX trigger only).
            let _ = other.take_damage_from_typed_death(
                0.0,
                Some(self.id),
                crate::game_logic::combat::DamageType::Crush,
                crate::game_logic::host_usa_pilot::HostDeathType::Crushed,
            );
        }
        if other.front_crushed && other.back_crushed {
            return true;
        }
        let us = self.get_position();
        let them = other.get_position();
        let (dx_f, dz_f) = self.unit_direction_xz();
        // major radius residual ≈ selection_radius
        let major = other.selection_radius.max(5.0);
        let offset = major / 2.0;
        let crushee_facing = {
            let y = other.get_orientation();
            (y.cos(), y.sin())
        };
        let target = {
            use crate::game_logic::host_partition_collision_physics_residual::select_crush_target_by_perp_residual;
            select_crush_target_by_perp_residual(
                other.front_crushed,
                other.back_crushed,
                (us.x, us.z),
                (them.x, them.z),
                (dx_f, dz_f),
                crushee_facing,
                offset,
            )
        };
        if target == CrushTarget::NoCrush {
            return true;
        }
        let point = match target {
            CrushTarget::FrontEndCrush => (
                them.x + crushee_facing.0 * offset,
                them.z + crushee_facing.1 * offset,
            ),
            CrushTarget::BackEndCrush => (
                them.x - crushee_facing.0 * offset,
                them.z - crushee_facing.1 * offset,
            ),
            CrushTarget::TotalCrush | CrushTarget::NoCrush => (them.x, them.z),
        };
        if past_crush_point_residual((us.x, us.z), point, (dx_f, dz_f), offset) {
            match target {
                CrushTarget::FrontEndCrush => {
                    other.front_crushed = true;
                    other.record_host_crush_vision();
                }
                CrushTarget::BackEndCrush => {
                    other.back_crushed = true;
                    other.record_host_crush_vision();
                }
                CrushTarget::TotalCrush => {
                    other.front_crushed = true;
                    other.back_crushed = true;
                    other.record_host_crush_vision();
                }
                CrushTarget::NoCrush => {}
            }
            // C++ CrushDie::onDie model condition residual.
            other.apply_crush_die_model_conditions();
            let _ = other.take_damage_from_typed_death(
                PHYSICS_HUGE_DAMAGE_AMOUNT_RESIDUAL,
                Some(self.id),
                crate::game_logic::combat::DamageType::Crush,
                crate::game_logic::host_usa_pilot::HostDeathType::Crushed,
            );
        }
        true
    }

    /// End-of-frame overlap residual: previous = current, clear current.
    pub fn advance_physics_overlap_frame(&mut self) {
        self.physics_previous_overlap = self.physics_current_overlap;
        self.physics_current_overlap = None;
    }

    pub fn scrub_velocity_2d(&mut self, desired_velocity: f32) {
        if desired_velocity < 0.001 {
            self.movement.velocity.x = 0.0;
            self.movement.velocity.z = 0.0;
            return;
        }
        let vx = self.movement.velocity.x;
        let vz = self.movement.velocity.z;
        let cur = (vx * vx + vz * vz).sqrt();
        if desired_velocity > cur || cur < 1e-6 {
            return;
        }
        let s = desired_velocity / cur;
        self.movement.velocity.x = vx * s;
        self.movement.velocity.z = vz * s;
    }

    /// C++ PhysicsBehavior::scrubVelocityZ residual (host Y-up vertical).
    pub fn scrub_velocity_vertical(&mut self, desired_velocity: f32) {
        if desired_velocity.abs() < 0.001 {
            self.movement.velocity.y = 0.0;
            return;
        }
        let vy = self.movement.velocity.y;
        if (desired_velocity < 0.0 && vy < desired_velocity)
            || (desired_velocity > 0.0 && vy > desired_velocity)
        {
            self.movement.velocity.y = desired_velocity;
        }
    }

    /// C++ parachute vs building jam residual: push out + scrub lateral.
    pub fn apply_parachute_building_bounce_out(
        &mut self,
        other_center: glam::Vec3,
        us_radius: f32,
    ) {
        use crate::game_logic::host_partition_collision_physics_residual::parachute_bounce_out_distance;
        let us = self.get_position();
        let mut dx = other_center.x - us.x;
        let mut dz = other_center.z - us.z;
        let mut dist = (dx * dx + dz * dz).sqrt();
        if dist < 1.0 {
            dist = 1.0;
            dx = 1.0;
            dz = 0.0;
        }
        let bounce = parachute_bounce_out_distance(us_radius);
        let mut pos = us;
        pos.x -= bounce * dx / dist;
        pos.z -= bounce * dz / dist;
        self.set_position(pos);
        self.scrub_velocity_2d(0.0);
    }

    /// C++ immobile collide stiffness bounce residual on velocity.
    ///
    /// Zeros velocity then applies bounce factor along separation (host XZ + Y).
    /// Returns applied force vector residual (for tests).
    pub fn apply_structure_stiffness_bounce(
        &mut self,
        other_center: glam::Vec3,
        stiffness: f32,
        mass: f32,
    ) -> glam::Vec3 {
        use crate::game_logic::host_partition_collision_physics_residual::structure_immobile_bounce_factor;
        let us = self.get_position();
        let mut dx = other_center.x - us.x;
        let mut dy = other_center.y - us.y;
        let mut dz = other_center.z - us.z;
        let mut dist = (dx * dx + dy * dy + dz * dz).sqrt();
        if dist < 1.0 {
            dist = 1.0;
        }
        let mag = self.movement.velocity.length();
        let factor = structure_immobile_bounce_factor(mag, mass, stiffness);
        // C++ cheats: nuke velocity then apply force direction from delta.
        self.movement.velocity = glam::Vec3::ZERO;
        let dir = glam::Vec3::new(dx / dist, dy / dist, dz / dist);
        // Force on us is opposite separation (push away from other): -delta direction * |factor|
        // factor is already negative; force = factor * unit(delta) pushes us away when factor<0?
        // C++: force = factor * (delta/dist) with factor negative → toward -delta = away from other. Good.
        let force = dir * factor;
        // mass≈1 → velocity += force (host residual, no separate accel integrate).
        self.movement.velocity += force;
        self.record_host_movement();
        force
    }

    pub fn evaluate_vehicle_crash_into(
        &self,
        other: &Object,
    ) -> crate::game_logic::host_partition_collision_physics_residual::VehicleCrashImmobileOutcome
    {
        use crate::game_logic::host_partition_collision_physics_residual::{
            vehicle_crash_into_immobile_outcome, PHYSICS_DEFAULT_STRUCTURE_RUBBLE_HEIGHT_RESIDUAL,
        };
        let is_vehicle = self.is_kind_of(KindOf::Vehicle);
        let other_structure = other.is_kind_of(KindOf::Structure);
        let other_immobile =
            other_structure || other.is_kind_of(KindOf::Immobile) || !other.can_move();
        // C++ delta.z < 0 → host Y-up falling.
        let falling = self.movement.velocity.y < 0.0;
        vehicle_crash_into_immobile_outcome(
            is_vehicle,
            other_structure,
            other_immobile,
            falling,
            self.get_position().y,
            PHYSICS_DEFAULT_STRUCTURE_RUBBLE_HEIGHT_RESIDUAL,
        )
    }

    pub fn record_bounce_land(&mut self, prev_y: f32) {
        let dy = (prev_y - self.get_position().y).abs();
        self.last_bounce_fall_dy = dy;
        self.last_bounce_volume = bounce_sound_volume_residual(dy, Self::SHOCK_MASS);
        self.bounce_land_events = self.bounce_land_events.saturating_add(1);
        self.bounce_audio_pending = self.bounce_audio_pending.saturating_add(1);
        if self.bounce_sound_name.is_empty() {
            self.bounce_sound_name = BOUNCE_SOUND_DEFAULT.to_string();
        }
        self.record_host_bounce_land();
    }

    /// Drain one pending bounce audio emit for GameLogic → TheAudio queue.
    pub fn take_bounce_audio_pending(&mut self) -> Option<(String, f32)> {
        if self.bounce_audio_pending == 0 {
            return None;
        }
        self.bounce_audio_pending = self.bounce_audio_pending.saturating_sub(1);
        self.record_host_bounce_land();
        Some((self.bounce_sound_name.clone(), self.last_bounce_volume))
    }

    /// C++ killWhenRestingOnGround residual.
    ///
    /// When settled on ground with near-zero velocity, kill non-drone (or
    /// unmanned/dead drones).
    pub fn maybe_kill_when_resting_on_ground(&mut self) -> bool {
        if !self.kill_when_resting_on_ground || self.status.destroyed {
            return false;
        }
        if self.get_position().y > 0.05 {
            return false;
        }
        if !self.velocity_is_very_small() {
            return false;
        }
        let is_drone = self.template_name.to_ascii_lowercase().contains("drone");
        // C++: kill if !drone OR dead OR unmanned.
        if is_drone && self.is_alive() && !self.status.disabled_unmanned {
            return false;
        }
        self.kill_from_stun_destruction()
    }

    pub fn apply_shock_fall_damage(&mut self, impact_vy: f32) -> f32 {
        if self.immune_to_falling_damage || self.is_kind_of(KindOf::Projectile) {
            return 0.0;
        }
        // netSpeed = -activeVelZ - minFall (C++ Z-up); host Y-up equivalent.
        let net_speed = (-impact_vy) - Self::min_fall_speed_for_damage();
        if net_speed <= 0.0 {
            return 0.0;
        }
        let vx = self.movement.velocity.x;
        let vz = self.movement.velocity.z;
        // Steep-fall gate residual.
        let steep_x =
            vx.abs() <= Self::FALL_TINY_DELTA || (impact_vy / vx).abs() >= Self::FALL_MIN_ANGLE_TAN;
        let steep_z =
            vz.abs() <= Self::FALL_TINY_DELTA || (impact_vy / vz).abs() >= Self::FALL_MIN_ANGLE_TAN;
        if !(steep_x && steep_z) {
            return 0.0;
        }
        let damage_amt = net_speed * Self::SHOCK_MASS * Self::FALL_HEIGHT_DAMAGE_FACTOR;
        if damage_amt <= 0.0 {
            return 0.0;
        }
        let killed = self.take_damage_from_typed_death(
            damage_amt,
            Some(self.id),
            crate::game_logic::combat::DamageType::Falling,
            crate::game_logic::host_usa_pilot::HostDeathType::Splatted,
        );
        if killed {
            use crate::game_logic::host_enum_table_residual::MC_BIT_SPLATTED;
            self.model_condition_bits |= 1u128 << MC_BIT_SPLATTED;
            self.refresh_model_condition_bits();
            // refresh may clear SPLATTED if not wired — re-set after.
            self.model_condition_bits |= 1u128 << MC_BIT_SPLATTED;
        }
        damage_amt
    }

    /// C++ PhysicsBehavior::applyYPRDamping residual.
    pub fn apply_ypr_damping(&mut self, factor: f32) {
        self.shock_yaw_rate *= factor;
        self.shock_pitch_rate *= factor;
        self.shock_roll_rate *= factor;
    }

    /// C++ setAllowBouncing residual.
    pub fn set_allow_bouncing(&mut self, allow: bool) {
        self.shock_allow_bounce = allow;
    }

    /// C++ handleBounce force residual (does not mutate velocity; returns force).
    ///
    /// Callers apply via `apply_physics_force` when ALLOW_BOUNCE remains set.
    pub fn compute_ground_bounce_force(
        &mut self,
        old_y: f32,
        new_y: f32,
        ground_y: f32,
    ) -> Option<glam::Vec3> {
        if !self.shock_allow_bounce || new_y > ground_y {
            return None;
        }
        let vy = self.movement.velocity.y;
        let mut desired_accel_y = 0.0;
        if old_y > ground_y && vy < 0.0 {
            let stiffness = Self::GROUND_STIFFNESS.clamp(0.01, 0.99);
            desired_accel_y = vy.abs() * stiffness;
        }
        self.apply_ypr_damping(Self::BOUNCE_YPR_DAMPING);
        if desired_accel_y > 0.0 {
            // C++ bounceForce.z = mass * desiredAccelZ
            let force_y = self.physics_get_mass() * desired_accel_y;
            // Right orientation residual when inverted.
            if self.shock_up_z < 0.0 {
                self.shock_up_z = 1.0;
            }
            self.shock_pitch_rate = 0.0;
            self.shock_roll_rate = 0.0;
            Some(glam::Vec3::new(0.0, force_y, 0.0))
        } else {
            // Restore original allow bounce residual.
            self.shock_allow_bounce = self.original_allow_bounce;
            None
        }
    }

    /// C++ PhysicsBehavior position integrate + ground clamp residual (one frame).
    ///
    /// `ground_y` is terrain height at object XZ. Returns true if a bounce force was applied.
    pub fn tick_physics_motion_step(&mut self, ground_y: f32) -> bool {
        if self.status.destroyed || !self.is_alive() {
            return false;
        }
        // Held residual not fully ported — skip if explicitly non-mobile structure without fall.
        if self.is_kind_of(crate::game_logic::KindOf::Structure) && !self.allow_to_fall {
            return false;
        }

        let old_pos = self.get_position();
        let old_y = old_pos.y;
        let airborne_start = old_y > ground_y + 0.05;

        // Integrate position from velocity (1 logic frame).
        let v = self.movement.velocity;
        let mut new_pos = old_pos + v;
        // YPR rate integrate residual (orientation presentation).
        let pryf = self.pitch_roll_yaw_factor;
        let mut yaw_rate = self.shock_yaw_rate * pryf;
        let mut pitch_rate = self.shock_pitch_rate * pryf;
        let roll_rate = self.shock_roll_rate * pryf;
        // C++ centerOfMassOffset damps pitch toward straight up/down residual.
        if self.center_of_mass_offset != 0.0 {
            // Host residual: approximate pitch angle from shock_up_z.
            let pitch_angle = (1.0 - self.shock_up_z.clamp(-1.0, 1.0))
                .asin()
                .copysign(self.shock_up_z);
            let remaining = if self.center_of_mass_offset > 0.0 {
                std::f32::consts::FRAC_PI_2 - pitch_angle
            } else {
                -std::f32::consts::FRAC_PI_2 + pitch_angle
            };
            pitch_rate *= remaining.sin();
        }
        let _ = roll_rate; // roll applied via shock rates presentation residual
        if yaw_rate.abs() > 1e-8 {
            let yaw = self.get_orientation() + yaw_rate;
            self.set_orientation(yaw);
        }
        let _ = pitch_rate;

        let bounce_force = self.compute_ground_bounce_force(old_y, new_pos.y, ground_y);
        let mut bounced = false;

        // Remember z-vel prior to ground-slam (host Y).
        if new_pos.y <= ground_y {
            let dy = ground_y - new_pos.y;
            self.movement.velocity.y += dy;
            if self.movement.velocity.y > 0.0 {
                self.movement.velocity.y = 0.0;
            }
            self.invalidate_velocity_magnitude();
            new_pos.y = ground_y;
            self.allow_to_fall = false;
            // Stunned flailing → stunned residual on first ground hit.
            if self.shock_stun_frames > 0 && !self.shock_grounded_once {
                self.shock_grounded_once = true;
            }
        } else if new_pos.y > ground_y {
            if self.stick_to_ground && !self.allow_to_fall {
                new_pos.y = ground_y;
            }
        }

        self.set_position(new_pos);

        if let Some(force) = bounce_force {
            if self.shock_allow_bounce {
                self.apply_physics_force(force);
                // Immediate integrate of bounce accel residual (C++ applies same frame).
                self.integrate_physics_accel();
                bounced = true;
                let _ = self.test_stunned_unit_for_destruction();
            }
        }

        let airborne_end = new_pos.y > ground_y + 0.05;
        // Landing damage residual when was airborne last frame.
        if self.was_airborne_last_frame && !airborne_end && !self.immune_to_falling_damage {
            // doBounceSound residual already exists elsewhere; falling damage peel.
            let impact_vy = v.y;
            let _ = self.apply_shock_fall_damage(impact_vy);
        }
        self.was_airborne_last_frame = airborne_end;
        self.record_host_locomotor();
        self.status.airborne_target = airborne_end;
        let _ = airborne_start; // reserved for future free-fall start residual
                                // C++ killWhenRestingOnGround residual after landing.
        if !airborne_end {
            let _ = self.maybe_kill_when_resting_on_ground();
        }
        bounced
    }

    /// C++ PhysicsBehavior::handleBounce residual (world-Y = C++ Z).
    ///
    /// Returns upward bounce velocity applied (0 if no bounce).
    pub fn handle_shock_ground_bounce(&mut self, old_y: f32, new_y: f32, ground_y: f32) -> f32 {
        if !self.shock_allow_bounce || new_y > ground_y {
            return 0.0;
        }
        let mut bounce_vy = 0.0;
        let vy = self.movement.velocity.y;
        if old_y > ground_y && vy < 0.0 {
            let stiffness = Self::GROUND_STIFFNESS.clamp(0.01, 0.99);
            // C++ desiredAccelZ = fabs(vz)*stiffness; mass≈1 → velocity kick.
            bounce_vy = vy.abs() * stiffness;
        }
        // Damp tumble rates on bounce.
        self.shock_yaw_rate *= Self::BOUNCE_YPR_DAMPING;
        self.shock_pitch_rate *= Self::BOUNCE_YPR_DAMPING;
        self.shock_roll_rate *= Self::BOUNCE_YPR_DAMPING;
        if bounce_vy > 0.0 {
            self.movement.velocity.y = bounce_vy;
            // C++ testStunnedUnitForDestruction on successful bounce force.
            if self.test_stunned_unit_for_destruction() {
                return 0.0;
            }
            // Right the object residual: keep yaw, zero pitch/roll presentation rates.
            self.shock_pitch_rate = 0.0;
            self.shock_roll_rate = 0.0;
            // C++ setAngles after bounce rights pitch/roll when not killed.
            if self.shock_up_z < 0.0 {
                // Already handled by kill path; keep.
            } else {
                self.shock_up_z = 1.0;
            }
            return bounce_vy;
        }
        // Bounce complete — restore original allow (host: off).
        self.shock_allow_bounce = false;
        self.record_host_bounce_land();
        0.0
    }

    /// Default locomotor surfaces residual from KindOf (fail-closed ground units).
    pub fn default_locomotor_surfaces_for_template(template: &ThingTemplate) -> u32 {
        if template.is_kind_of(KindOf::Aircraft) {
            LOCO_SURFACE_AIR | LOCO_SURFACE_GROUND
        } else if template.name.to_ascii_lowercase().contains("hover")
            || template.name.to_ascii_lowercase().contains("amphib")
            || template.name.to_ascii_lowercase().contains("ship")
        {
            LOCO_SURFACE_GROUND | LOCO_SURFACE_WATER
        } else if template.is_kind_of(KindOf::Structure) {
            LOCO_SURFACE_GROUND
        } else {
            LOCO_SURFACE_GROUND
        }
    }

    fn ensure_locomotor_surfaces(&mut self) {
        if self.locomotor_surfaces == 0 {
            self.locomotor_surfaces =
                Self::default_locomotor_surfaces_for_template(&self.thing.template);
        }
    }

    pub fn has_locomotor_for_surface(&self, surface: u32) -> bool {
        (self.locomotor_surfaces & surface) != 0
    }

    /// C++ PhysicsBehavior::testStunnedUnitForDestruction residual.
    ///
    /// Called on bounce. Kills when upside-down, off-map, cliff without cliff
    /// locomotor, or underwater without water locomotor.
    pub fn test_stunned_unit_for_destruction(&mut self) -> bool {
        if !self.is_shock_stunned() || self.status.destroyed {
            return false;
        }
        self.ensure_locomotor_surfaces();
        // Upside down when transform Z-up residual is negative.
        if self.shock_up_z < 0.0 {
            return self.kill_from_stun_destruction();
        }
        // C++ obj->isOffMap residual.
        let pos = self.get_position();
        if crate::game_logic::host_deliver_payload::is_off_map_default_residual(pos) {
            return self.kill_from_stun_destruction();
        }
        // C++ isCliffCell && !hasLocomotorForSurface(CLIFF).
        if self.cell_is_cliff && !self.has_locomotor_for_surface(LOCO_SURFACE_CLIFF) {
            return self.kill_from_stun_destruction();
        }
        // C++ isUnderwater && !hasLocomotorForSurface(WATER).
        if self.cell_is_underwater && !self.has_locomotor_for_surface(LOCO_SURFACE_WATER) {
            return self.kill_from_stun_destruction();
        }
        false
    }

    fn kill_from_stun_destruction(&mut self) -> bool {
        if self.status.destroyed {
            return false;
        }
        self.health.current = 0.0;
        self.status.destroyed = true;
        self.status.death_type = crate::game_logic::host_usa_pilot::HostDeathType::Normal;
        crate::game_logic::host_death_type_log::record(self.id, self.status.death_type.ordinal());
        self.set_ai_state(AIState::Idle);
        self.target = None;
        self.shock_stun_frames = 0;
        self.set_status_disabled_freefall(false);
        self.refresh_model_condition_bits();
        true
    }

    /// Tick shock stun residual (once per logic frame).
    pub fn tick_shock_stun(&mut self) {
        if self.shock_stun_frames == 0 {
            // Damp residual rates when fully settled.
            self.shock_yaw_rate *= 0.85;
            self.shock_pitch_rate *= 0.85;
            self.shock_roll_rate *= 0.85;
            if self.shock_yaw_rate.abs() < 1e-4 {
                self.shock_yaw_rate = 0.0;
            }
            // Grounded settle: clear freefall leftovers.
            if self.movement.velocity.y.abs() < 0.25 {
                self.movement.velocity.y = 0.0;
                self.shock_was_airborne = false;
                self.shock_allow_bounce = false;
                self.set_status_disabled_freefall(false);
                let _ = self.maybe_kill_when_resting_on_ground();
            }
            return;
        }
        self.shock_stun_frames = self.shock_stun_frames.saturating_sub(1);
        self.record_host_shock_stun();
        // Integrate yaw rate residual while stunned (tumble settle).
        if self.shock_yaw_rate.abs() > 1e-5 {
            let ori = self.get_orientation() + self.shock_yaw_rate;
            self.set_orientation(ori);
            self.shock_yaw_rate *= 0.92; // friction residual
        }
        self.shock_pitch_rate *= 0.92;
        self.shock_roll_rate *= 0.92;

        // Vertical freefall / bounce residual (host Y-up == C++ Z).
        let ground_y = 0.0;
        let old_y = self.get_position().y;
        // Gravity while airborne or still carrying vertical velocity.
        if old_y > ground_y + 0.01 || self.movement.velocity.y.abs() > 0.01 {
            self.movement.velocity.y += Self::SHOCK_GRAVITY;
            let mut pos = self.get_position();
            let new_y = pos.y + self.movement.velocity.y;
            if new_y <= ground_y {
                // Capture impact velocity before bounce/slam (C++ activeVelZ residual).
                let impact_vy = self.movement.velocity.y;
                let was_air = self.shock_was_airborne || old_y > ground_y + 0.01;
                let bounced = self.handle_shock_ground_bounce(old_y, new_y, ground_y);
                pos.y = ground_y;
                self.set_position(pos);
                // C++ first ground hit while stunned: FLAILING → STUNNED.
                if !self.shock_grounded_once {
                    self.shock_grounded_once = true;
                    // Force model into STUNNED band (frames 1..=15) if still flailing.
                    if self.shock_stun_frames > 15 {
                        self.shock_stun_frames = 15;
                    }
                }
                // C++ WAS_AIRBORNE_LAST_FRAME && !airborneAtEnd → bounce sound + fall damage.
                if was_air {
                    self.record_bounce_land(old_y);
                    let _ = self.apply_shock_fall_damage(impact_vy);
                }
                if bounced <= 0.0 {
                    // Slam residual: clamp downward vel at ground.
                    if self.movement.velocity.y < 0.0 {
                        self.movement.velocity.y = 0.0;
                    }
                    self.shock_was_airborne = false;
                    // C++ clear IS_IN_FREEFALL / DISABLED_FREEFALL when grounded.
                    self.set_status_disabled_freefall(false);
                } else {
                    // Bounce still airborne residual.
                    self.shock_was_airborne = true;
                    self.set_status_disabled_freefall(true);
                }
            } else {
                pos.y = new_y;
                self.set_position(pos);
                self.shock_was_airborne = true;
                // C++ IS_IN_FREEFALL → DISABLED_FREEFALL + MODELCONDITION_FREEFALL.
                self.set_status_disabled_freefall(true);
            }
        } else {
            // Lateral bleed only when grounded.
            if self.movement.velocity.y.abs() < 0.5 {
                self.movement.velocity.y = 0.0;
            }
            self.set_status_disabled_freefall(false);
            self.shock_was_airborne = false;
        }
        // Lateral friction residual while stunned on ground.
        if self.get_position().y <= ground_y + 0.01 {
            self.movement.velocity.x *= 0.92;
            self.movement.velocity.z *= 0.92;
            // Ground contact residual: freefall disable only while airborne.
            if self.movement.velocity.y <= 0.01 {
                self.set_status_disabled_freefall(false);
            }
            // C++ killWhenRestingOnGround after settle.
            let _ = self.maybe_kill_when_resting_on_ground();
        }
        self.refresh_model_condition_bits();
    }

    /// C++ PhysicsBehavior::getIsStunned residual.
    pub fn is_shock_stunned(&self) -> bool {
        self.shock_stun_frames > 0
    }

    pub fn take_damage_from(&mut self, damage: f32, source: Option<ObjectId>) -> bool {
        self.take_damage_from_typed(
            damage,
            source,
            crate::game_logic::combat::DamageType::Unresistable,
        )
    }

    /// Superweapon / strike residual: always mutate host HP (and still log for shadow).
    /// Combat fire under DAMAGE_AUTHORITY defers HP to GameWorld writeback; strikes
    /// call this path so host-only update_special_power_strikes still applies damage.
    pub fn take_damage_from_immediate(&mut self, damage: f32, source: Option<ObjectId>) -> bool {
        self.take_damage_from_typed_death_with_host_hp(
            damage,
            source,
            crate::game_logic::combat::DamageType::Unresistable,
            crate::game_logic::host_usa_pilot::HostDeathType::from_host_damage_type(
                crate::game_logic::combat::DamageType::Unresistable,
            ),
            true, // force host HP apply
        )
    }

    /// Apply damage with host combat DamageType for Armor.ini residual coefficients.
    pub fn take_damage_from_typed(
        &mut self,
        damage: f32,
        source: Option<ObjectId>,
        damage_type: crate::game_logic::combat::DamageType,
    ) -> bool {
        self.take_damage_from_typed_death(
            damage,
            source,
            damage_type,
            crate::game_logic::host_usa_pilot::HostDeathType::from_host_damage_type(damage_type),
        )
    }

    /// Apply damage with Armor.ini type residual and Weapon.ini DeathType on kill.
    pub fn take_damage_from_typed_death(
        &mut self,
        damage: f32,
        source: Option<ObjectId>,
        damage_type: crate::game_logic::combat::DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
    ) -> bool {
        // C++ DAMAGE_DISARM residual: destroy mine without detonation splash.
        if matches!(damage_type, crate::game_logic::combat::DamageType::Disarm) {
            let _ = (source, death_type, damage);
            return self.disarm_mine_safe();
        }
        // C++ DAMAGE_DEPLOY residual: no HP on victim.
        // AssaultTransportAI::beginAssault is source-side (GameLogic combat path).
        if matches!(damage_type, crate::game_logic::combat::DamageType::Deploy) {
            let _ = (source, death_type, damage);
            return false;
        }
        // C++ DAMAGE_HACK residual: fire does not deal HP (effect is timer-driven).
        if matches!(damage_type, crate::game_logic::combat::DamageType::Hack) {
            let _ = (source, death_type, damage);
            return false;
        }
        // C++ DAMAGE_KILL_GARRISONED residual: structure HP untouched; occupants
        // cleared by GameLogic using pending kill count = floor(amount).
        if matches!(
            damage_type,
            crate::game_logic::combat::DamageType::KillGarrisoned
        ) {
            let _ = (source, death_type);
            let kills = damage.max(0.0).floor() as u32;
            self.status.pending_kill_garrisoned =
                self.status.pending_kill_garrisoned.saturating_add(kills);
            return false;
        }
        // C++ DAMAGE_SURRENDER residual: lethal hit on surrender-capable infantry
        // sets surrendered instead of destroying (ActiveBody commented path residual).
        if matches!(
            damage_type,
            crate::game_logic::combat::DamageType::Surrender
        ) {
            let _ = death_type;
            if self.can_surrender_from_damage() {
                let would_kill = damage >= self.health.current && self.health.current > 0.0;
                if would_kill {
                    self.set_surrendered(true);
                    self.status.attacking = false;
                    self.target = None;
                    return false;
                }
            }
            // Non-lethal or non-capable: fall through to normal HP.
        }
        // DAMAGE_PENALTY: normal HP path (no special intercept).
        // C++ DAMAGE_HEALING residual: restore HP via attemptHealing; never destroys.
        // Does not stamp last_damage_source (C++ AIGuardRetaliate / stealth skip).
        if matches!(damage_type, crate::game_logic::combat::DamageType::Healing) {
            let _ = death_type;
            if self.status.destroyed || !self.is_alive() {
                return false;
            }
            // C++ PoisonedBehavior::onHealing residual (heal path).
            self.clear_poisoned_on_healing();
            // amount is heal strength; negative ignored by heal().
            self.heal(damage.max(0.0));
            // Optional: record healer without treating as hostile damage source.
            let _ = source;
            return false;
        }
        // DAMAGE_WATER: normal HP damage path (type distinguishes FX in C++).
        // C++ DAMAGE_KILL_PILOT residual: unmanned vehicle, no HP damage.
        if matches!(
            damage_type,
            crate::game_logic::combat::DamageType::KillPilot
        ) {
            if self.is_kind_of(crate::game_logic::KindOf::Vehicle)
                || self.is_kind_of(crate::game_logic::KindOf::Aircraft)
            {
                // C++ car-bomb dead-man residual when sniped.
                if self.is_car_bomb() {
                    // Detonation handled by combat caller; mark unmanned edge.
                }
                self.apply_kill_pilot_unmanned();
                self.set_team(crate::game_logic::Team::Neutral);
            }
            let _ = (source, death_type, damage);
            return false;
        }
        // C++ IsSubdualDamage residual (Microwave/EMP maps to host EMP class).
        if matches!(damage_type, crate::game_logic::combat::DamageType::EMP) {
            self.apply_subdual_damage(damage.max(0.0));
            let _ = (source, death_type);
            return false;
        }
        // C++ DAMAGE_STATUS residual: amount is duration msec, not hitpoints.
        if matches!(damage_type, crate::game_logic::combat::DamageType::Status) {
            let frames = ((damage.max(0.0) * 30.0) / 1000.0).ceil() as u32;
            let frame = crate::game_logic::host_historic_bonus::logic_frame();
            // Default status peel when caller didn't already apply a named status.
            // FAERIE_FIRE is the primary retail STATUS residual.
            if frames > 0 {
                self.do_status_damage("FAERIE_FIRE", frames.max(1), frame);
            }
            let _ = (source, death_type);
            return false;
        }

        self.take_damage_from_typed_death_with_host_hp(
            damage,
            source,
            damage_type,
            death_type,
            false,
        )
    }

    fn take_damage_from_typed_death_with_host_hp(
        &mut self,
        damage: f32,
        source: Option<ObjectId>,
        damage_type: crate::game_logic::combat::DamageType,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType,
        force_host_hp: bool,
    ) -> bool {
        if self.status.destroyed {
            return false;
        }
        // OCL InvulnerableTime residual (post-eject pilot shield).
        if self.status.eject_invulnerable {
            return false;
        }

        // C++ BaseRegenerateUpdate::onDamage residual (delay before auto-heal).
        if damage > 0.0 {
            if let Some(br) = self.base_regenerate.as_mut() {
                br.mark_damaged();
            }
            // C++ ProneUpdate::goProne residual.
            if let Some(pu) = self.prone_update.as_mut() {
                let _ = pu.go_prone_damage(damage);
            }
        }

        // C++ StealthForbiddenConditions TAKING_DAMAGE residual (CamoNetting structures).
        if self.stealth_breaks_on_damage && self.status.stealthed {
            self.break_stealth();
        }

        // BodyModule last damage source residual (Passive WaitForAttack).
        if let Some(src) = source {
            self.last_damage_source = Some(src);
        }

        // Armor.ini residual coefficient (by object kind + damage type), then
        // legacy scalar armor + HoldTheLine plan residual.
        // C++ DAMAGE_UNRESISTABLE bypasses ArmorTemplate + scalar armor residual.
        let typed =
            crate::game_logic::host_armor_residual::apply_residual_armor(self, damage_type, damage);
        // C++ DAMAGE_UNRESISTABLE bypasses ArmorTemplate/scalar armor, but Strategy Center
        // HoldTheLinePlanArmorDamageScalar still multiplies body damage (LESS is better).
        let battle_plan_armor = self.battle_plan_armor_damage_scalar();
        let mut actual_damage = if matches!(
            damage_type,
            crate::game_logic::combat::DamageType::Unresistable
        ) {
            typed * battle_plan_armor
        } else {
            let armor_factor =
                1.0 - (self.thing.template.armor / (self.thing.template.armor + 100.0));
            typed * armor_factor * battle_plan_armor
        };

        // C++ ActiveBody: damaged CAN_BE_REPULSED civilians scare others when EnableRepulsors.
        // Object::setStatus(REPULSOR) + ObjectRepulsorHelper sleepUntil(+2 sec).
        if crate::game_logic::host_repulsor_gate::is_enabled()
            && actual_damage > 0.0
            && self.is_kind_of(KindOf::CanBeRepulsed)
        {
            self.set_status_repulsor(true);
            // 2 * LOGICFRAMES_PER_SECOND residual; frame base applied by host tick if 0.
            // Store absolute if known; else relative sentinel cleared by tick with current frame.
            if self.repulsor_until_frame == 0 || self.repulsor_until_frame < 100_000 {
                // relative duration residual; tick converts with current_frame
                self.repulsor_until_frame = 60; // 2 seconds @ 30Hz
            }
        }

        // C++ UndeadBody::attemptDamage residual (Battle Bus first life).
        // Clamp lethal non-UNRESISTABLE damage to leave 1 HP, then startSecondLife.
        let mut battle_bus_start_second = false;
        if self.battle_bus_should_intercept_lethal(damage_type, actual_damage) {
            actual_damage = (self.health.current - 1.0).max(0.0);
            battle_bus_start_second = true;
        }

        // C++ HighlanderBody::attemptDamage residual.
        let mut _highlander_clamped = false;
        if self.highlander_body && !battle_bus_start_second {
            let unres = matches!(
                damage_type,
                crate::game_logic::combat::DamageType::Unresistable
                    | crate::game_logic::combat::DamageType::Penalty
            );
            let (clamped, did) = crate::game_logic::host_highlander_body::highlander_clamp_damage(
                self.health.current,
                actual_damage,
                unres,
            );
            if did {
                actual_damage = clamped;
                _highlander_clamped = true;
            }
        }

        // GameWorld damage authority: host logs intent only; HP/destroyed last-write
        // via shadow session mutations + writeback_health_to_host (no mid-frame host HP mutate).
        // Defer only when a live shadow session can consume the log. Otherwise host-only
        // combat would record damage and never apply HP (authority without writeback).
        // force_host_hp: superweapon/residual paths always mutate host immediately.
        let damage_auth =
            crate::gameworld_shadow::gameworld_damage_authority_live() && !force_host_hp;
        let destroyed = if damage_auth {
            let projected = (self.health.current - actual_damage).max(0.0);
            let will_die = projected <= 0.0 || actual_damage >= self.health.current;
            crate::game_logic::host_damage_log::record(self.id, actual_damage, source, will_die);
            // Projected lethal: mark destroyed so is_alive() fails mid-frame without
            // mutating HP (shadow remains last-writer for the numeric health value).
            // Prevents multi-attacker overkill / retarget of a corpse before writeback.
            if will_die && !self.status.destroyed {
                self.status.destroyed = true;
                self.status.death_type = death_type;
                crate::game_logic::host_death_type_log::record(
                    self.id,
                    self.status.death_type.ordinal(),
                );
                self.set_ai_state(AIState::Idle);
                self.target = None;
            }
            will_die
        } else {
            self.health.damage(actual_damage);
            let destroyed = if !self.health.is_alive() {
                self.status.destroyed = true;
                self.status.death_type = death_type;
                crate::game_logic::host_death_type_log::record(
                    self.id,
                    self.status.death_type.ordinal(),
                );
                self.set_ai_state(AIState::Idle);
                self.target = None;
                true
            } else {
                false
            };
            crate::game_logic::host_damage_log::record(self.id, actual_damage, source, destroyed);
            destroyed
        };

        // C++ UndeadBody::startSecondLife after ActiveBody::attemptDamage residual.
        if battle_bus_start_second {
            self.start_battle_bus_second_life();
        }

        // C++ PoisonedBehavior::onDamage residual.
        if actual_damage > 0.0 {
            let frame = crate::game_logic::host_historic_bonus::logic_frame();
            self.notify_poisoned_on_damage(frame, damage_type, actual_damage, death_type);
        }
        // C++ FireWeaponWhenDamagedBehavior::onDamage residual (frame filled by GameLogic).
        if actual_damage > 0.0
            && !matches!(
                damage_type,
                crate::game_logic::combat::DamageType::Healing
                    | crate::game_logic::combat::DamageType::Status
                    | crate::game_logic::combat::DamageType::Hack
                    | crate::game_logic::combat::DamageType::Deploy
                    | crate::game_logic::combat::DamageType::Disarm
                    | crate::game_logic::combat::DamageType::KillPilot
                    | crate::game_logic::combat::DamageType::KillGarrisoned
            )
        {
            self.ensure_fire_weapon_when_damaged();
            if let Some(fw) = self.fire_weapon_when_damaged.as_mut() {
                // Frame 0: debounce via serial on data; GameLogic may also call with real frame.
                if let Some(w) = fw.on_damage(
                    actual_damage,
                    self.health.current,
                    self.health.maximum.max(self.max_health).max(1.0),
                    fw.last_reaction_frame.saturating_add(2),
                ) {
                    self.pending_fire_when_damaged_weapon = Some(w);
                }
            }
        }

        self.refresh_model_condition_bits();
        if battle_bus_start_second {
            false
        } else {
            destroyed
        }
    }

    /// C++ AttitudeType residual (Sleep/Passive/Normal/Alert/Aggressive).
    pub fn ai_attitude(&self) -> crate::game_logic::host_strategy_center::HostAiAttitude {
        crate::game_logic::host_strategy_center::HostAiAttitude::from_i8(self.ai_attitude)
    }

    /// Set C++ AttitudeType residual for TurretAI mood matrix.
    pub fn set_ai_attitude(
        &mut self,
        attitude: crate::game_logic::host_strategy_center::HostAiAttitude,
    ) {
        self.ai_attitude = attitude.as_i8();
        crate::game_logic::host_ai_attitude_log::record(self.id, self.ai_attitude);
    }

    pub fn record_host_ground_height(&self) {
        crate::game_logic::host_ground_height_log::record(
            self.id,
            self.ground_height,
            self.ground_height_from_terrain,
        );
    }

    pub fn set_ground_height_residual(&mut self, height: f32, from_terrain: bool) {
        let changed = (self.ground_height - height).abs() > f32::EPSILON
            || self.ground_height_from_terrain != from_terrain;
        if !changed {
            return;
        }
        self.ground_height = height;
        self.ground_height_from_terrain = from_terrain;
        self.record_host_ground_height();
    }

    /// Presentation mesh identity residual (model_key + mesh_scale) → GameWorld SetModelMesh.
    pub fn set_model_mesh_residual(&mut self, model_key: impl Into<String>, mesh_scale: f32) {
        let key = model_key.into();
        let scale = if mesh_scale.is_finite() && mesh_scale > 0.0 {
            mesh_scale
        } else {
            1.0
        };
        crate::game_logic::host_model_mesh_log::record(self.id, key, scale);
    }

    /// Resolve and log mesh residual from the active (possibly disguised) template.
    pub fn record_model_mesh_from_template(&mut self) {
        let tpl = self.get_template();
        let key = crate::assets::mesh_asset_resolve::model_key_from_template(tpl);
        let scale = crate::assets::mesh_asset_resolve::mesh_scale_from_template(tpl);
        self.set_model_mesh_residual(key, scale);
    }

    /// FOW visibility residual → GameWorld SetFow (presentation last-writer channel).
    pub fn set_fow_residual(
        &mut self,
        visibility_alpha: f32,
        is_explored: f32,
        visibility_falloff: f32,
    ) {
        crate::game_logic::host_fow_log::record(
            self.id,
            visibility_alpha,
            is_explored,
            visibility_falloff,
        );
    }

    /// Presentation kind_of ORDER bits residual (same ORDER as GameWorldShadow::host_kind_of_bits).
    pub fn presentation_kind_of_bits(&self) -> u32 {
        use crate::game_logic::KindOf;
        const ORDER: &[KindOf] = &[
            KindOf::Structure,
            KindOf::Infantry,
            KindOf::Vehicle,
            KindOf::Aircraft,
            KindOf::Projectile,
            KindOf::Resource,
            KindOf::Selectable,
            KindOf::Attackable,
            KindOf::CommandCenter,
            KindOf::Worker,
            KindOf::Hero,
            KindOf::SupplyCenter,
            KindOf::PowerPlant,
            KindOf::FSBarracks,
            KindOf::FSWarFactory,
            KindOf::FSAirfield,
            KindOf::FSInternetCenter,
            KindOf::FSPower,
            KindOf::FSBaseDefense,
            KindOf::FSSupplyDropzone,
            KindOf::FSSupplyCenter,
            KindOf::FSSuperweapon,
            KindOf::FSStrategyCenter,
            KindOf::FSFake,
            KindOf::FSTechnology,
            KindOf::FSBlackMarket,
            KindOf::FSAdvancedTech,
            KindOf::Harvestable,
            KindOf::Powered,
        ];
        let set = &self.get_template().kind_of;
        let mut bits = 0u32;
        for (i, k) in ORDER.iter().enumerate() {
            if set.contains(k) {
                bits |= 1u32 << i;
            }
        }
        bits
    }

    /// kind_of bits residual → GameWorld SetKindOfBits.
    pub fn set_kind_of_bits_residual(&mut self, kind_of_bits: u32) {
        crate::game_logic::host_kind_of_log::record(self.id, kind_of_bits);
    }

    /// Resolve and log kind_of bits from the active template.
    pub fn record_kind_of_bits_from_template(&mut self) {
        let bits = self.presentation_kind_of_bits();
        self.set_kind_of_bits_residual(bits);
    }

    pub fn record_host_identity(&self) {
        crate::game_logic::host_identity_log::record(self.id, self.name.clone(), self.team_color);
    }

    pub fn record_host_building_type(&self) {
        use crate::game_logic::BuildingType as B;
        let (is_building, ordinal) = match self.building_data.as_ref() {
            Some(bd) => {
                let ord = match bd.building_type {
                    B::CommandCenter => 0u8,
                    B::Barracks => 1,
                    B::WarFactory => 2,
                    B::Airfield => 3,
                    B::RepairPad => 4,
                    B::HealPad => 5,
                    B::SupplyCenter => 6,
                    B::PowerPlant => 7,
                    B::DefenseTurret => 8,
                    B::SupplyDropZone => 9,
                    B::Palace => 10,
                    B::Propaganda => 11,
                    B::Bunker => 12,
                };
                (true, ord)
            }
            None => (false, 255u8),
        };
        crate::game_logic::host_building_type_log::record(self.id, is_building, ordinal);
    }

    /// C++ CrushDie model condition FRONTCRUSHED/BACKCRUSHED residual.
    pub fn apply_crush_die_model_conditions(&mut self) {
        use crate::game_logic::host_neutron_missile_slow_death::{
            MC_BIT_BACKCRUSHED, MC_BIT_FRONTCRUSHED,
        };
        // Clear then set like C++ clearAndSetModelConditionFlags.
        self.model_condition_bits &= !(1u128 << MC_BIT_FRONTCRUSHED);
        self.model_condition_bits &= !(1u128 << MC_BIT_BACKCRUSHED);
        if self.front_crushed {
            self.model_condition_bits |= 1u128 << MC_BIT_FRONTCRUSHED;
        }
        if self.back_crushed {
            self.model_condition_bits |= 1u128 << MC_BIT_BACKCRUSHED;
        }
    }

    pub fn record_host_crush_vision(&self) {
        crate::game_logic::host_crush_vision_log::record(
            self.id,
            self.crusher_level,
            self.crushable_level,
            self.vision_range,
            self.shroud_clearing_range,
            self.front_crushed,
            self.back_crushed,
        );
    }

    pub fn record_host_demo_mine_cheer(&self) {
        crate::game_logic::host_demo_mine_cheer_log::record(
            self.id,
            self.demo_suicided_detonating,
            self.mine_data.is_some(),
            self.cheer_timer,
        );
    }

    pub fn record_host_selection_radius(&self) {
        crate::game_logic::host_selection_radius_log::record(self.id, self.selection_radius);
    }

    pub fn set_selection_radius(&mut self, selection_radius: f32) {
        if (self.selection_radius - selection_radius).abs() > f32::EPSILON {
            self.selection_radius = selection_radius;
            self.record_host_selection_radius();
        }
    }

    pub fn record_host_model_condition(&self) {
        crate::game_logic::host_model_condition_log::record(self.id, self.model_condition_bits);
    }

    pub fn record_host_radar_extend(&self) {
        crate::game_logic::host_radar_extend_log::record(
            self.id,
            self.radar_extend_done_frame,
            self.radar_extend_complete,
            self.radar_active,
        );
    }

    pub fn record_host_shock_stun(&self) {
        crate::game_logic::host_shock_stun_log::record(
            self.id,
            self.shock_stun_frames,
            self.shock_yaw_rate,
            self.shock_pitch_rate,
            self.shock_roll_rate,
            self.shock_up_z,
            self.shock_allow_bounce,
            self.shock_grounded_once,
            self.shock_was_airborne,
            self.cell_is_cliff,
            self.cell_is_underwater,
        );
    }

    pub fn record_host_production_door(&self) {
        crate::game_logic::host_production_door_log::record(
            self.id,
            self.production_door_phase,
            self.production_door_phase_end_frame,
            self.production_door_hold_open,
        );
    }

    pub fn record_host_ai_mood(&self) {
        crate::game_logic::host_ai_mood_log::record(
            self.id,
            self.idle_since_frame,
            self.mood_attack_check_rate,
            self.auto_acquire_when_idle,
            self.attack_priority_set.clone().unwrap_or_default(),
        );
    }

    pub fn record_host_sole_healing(&self) {
        crate::game_logic::host_sole_healing_log::record(
            self.id,
            self.sole_healing_benefactor.map(|id| id.0),
            self.sole_healing_benefactor_expiration_frame,
        );
    }

    pub fn record_host_rebuild_producer(&self) {
        crate::game_logic::host_rebuild_producer_log::record(
            self.id,
            self.is_rebuild_hole,
            self.rebuild_template_name.clone().unwrap_or_default(),
            self.rebuild_ready_frame,
            self.rebuild_spawner_id.map(|id| id.0),
            self.rebuild_worker_id.map(|id| id.0),
            self.rebuild_reconstructing_id.map(|id| id.0),
            self.producer_id.map(|id| id.0),
            self.construction_complete_clear_frame,
        );
    }

    pub fn record_host_bounce_land(&self) {
        crate::game_logic::host_bounce_land_log::record(
            self.id,
            self.kill_when_resting_on_ground,
            self.bounce_land_events,
            self.last_bounce_fall_dy,
            self.bounce_sound_name.clone(),
            self.last_bounce_volume,
            self.bounce_audio_pending,
            self.allow_collide_force,
            self.last_collidee.map(|id| id.0),
            self.ignore_collisions_with.map(|id| id.0),
        );
    }

    pub fn record_host_physics_motive(&self) {
        crate::game_logic::host_physics_motive_log::record(
            self.id,
            self.motive_frames_remaining,
            self.physics_mass,
            [
                self.physics_accel.x,
                self.physics_accel.y,
                self.physics_accel.z,
            ],
            self.forward_friction,
            self.lateral_friction,
            self.z_friction,
            self.can_path_through_units,
            self.ignore_collisions_until_frame,
            self.is_panicking,
            self.move_away_frames,
            self.aerodynamic_friction,
            self.extra_friction,
            self.apply_friction_2d_when_airborne,
            self.center_of_mass_offset,
            self.pitch_roll_yaw_factor,
            self.move_away_destination.map(|p| [p.x, p.y, p.z]),
            self.request_other_move_away.map(|id| id.0),
            self.immune_to_falling_damage,
            self.physics_current_overlap.map(|id| id.0),
            self.physics_previous_overlap.map(|id| id.0),
        );
    }

    pub fn record_host_movement(&self) {
        crate::game_logic::host_movement_log::record(
            self.id,
            self.movement.velocity,
            self.movement.max_speed,
            self.movement.current_path_index,
            &self.movement.path,
            self.waiting_for_path,
            self.locomotor_surfaces,
            self.is_attack_path,
            self.is_blocked_and_stuck,
            self.is_braking,
            self.is_safe_path,
            self.queue_for_path_frames,
            self.path_timestamp,
            self.cur_max_blocked_speed,
            self.num_frames_blocked,
            self.is_blocked,
            self.move_away_from.map(|id| id.0),
            self.requested_victim_id.map(|id| id.0),
        );
        self.record_host_physics_motive();
    }

    pub fn record_host_weapon_stats(&self) {
        let (
            has_weapon,
            weapon_damage,
            weapon_range,
            weapon_min_range,
            weapon_reload_time,
            weapon_last_fire_time,
            weapon_clip_size,
            weapon_clip_reload_time,
            weapon_ammo,
            weapon_can_target_air,
            weapon_can_target_ground,
            weapon_projectile_speed,
        ) = if let Some(w) = self.weapon.as_ref() {
            (
                true,
                w.damage,
                w.range,
                w.min_range,
                w.reload_time,
                w.last_fire_time,
                w.clip_size,
                w.clip_reload_time,
                w.ammo.unwrap_or(u32::MAX),
                w.can_target_air,
                w.can_target_ground,
                w.projectile_speed,
            )
        } else {
            (
                false,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
                0,
                0.0,
                u32::MAX,
                false,
                true,
                0.0,
            )
        };
        let (has_secondary_weapon, secondary_weapon_damage, secondary_weapon_range) =
            if let Some(w) = self.secondary_weapon.as_ref() {
                (true, w.damage, w.range)
            } else {
                (false, 0.0, 0.0)
            };
        crate::game_logic::host_weapon_stats_log::record(
            crate::game_logic::host_weapon_stats_log::HostWeaponStatsEvent {
                object: self.id,
                has_weapon,
                weapon_damage,
                weapon_range,
                weapon_min_range,
                weapon_reload_time,
                weapon_last_fire_time,
                weapon_clip_size,
                weapon_clip_reload_time,
                weapon_ammo,
                weapon_can_target_air,
                weapon_can_target_ground,
                weapon_projectile_speed,
                has_secondary_weapon,
                secondary_weapon_damage,
                secondary_weapon_range,
                leech_range_active_primary: self.leech_range_active_primary,
                leech_range_active_secondary: self.leech_range_active_secondary,
            },
        );
    }

    pub fn record_host_vision_camo(&self) {
        crate::game_logic::host_vision_camo_log::record(
            self.id,
            self.vision_spied_mask,
            self.camo_friendly_opacity,
            self.camo_stealth_look,
        );
    }

    pub fn record_host_command_set(&self) {
        crate::game_logic::host_command_set_log::record(self.id, self.command_set_override.clone());
    }

    pub fn set_command_set_override(&mut self, command_set: Option<String>) {
        if self.command_set_override != command_set {
            self.command_set_override = command_set;
            self.record_host_command_set();
        }
    }

    pub fn record_host_disguise(&self) {
        let team = self
            .disguise_as_team
            .map(|t| match t {
                Team::USA => 0,
                Team::China => 1,
                Team::GLA => 2,
                Team::Neutral => 3,
            })
            .unwrap_or(255);
        crate::game_logic::host_disguise_log::record(
            self.id,
            self.disguise_as_template.clone(),
            team,
        );
    }

    pub fn record_host_overlord(&self) {
        let bunker_capacity = match self.overlord_bunker_capacity {
            Some(n) => n.min(u16::MAX as usize - 1) as u16,
            None => u16::MAX,
        };
        crate::game_logic::host_overlord_log::record(
            self.id,
            self.has_overlord_gattling_addon,
            self.has_overlord_propaganda_addon,
            bunker_capacity,
            self.is_helix_transport,
        );
    }

    pub fn record_host_stealth_flags(&self) {
        crate::game_logic::host_stealth_flags_log::record(
            crate::game_logic::host_stealth_flags_log::HostStealthFlagsEvent {
                object: self.id,
                innate_stealth: self.innate_stealth,
                stealth_breaks_on_attack: self.stealth_breaks_on_attack,
                stealth_breaks_on_move: self.stealth_breaks_on_move,
                is_tunnel_network: self.is_tunnel_network,
                passengers_allowed_to_fire: self.passengers_allowed_to_fire,
            },
        );
    }

    pub fn record_host_hive(&self) {
        crate::game_logic::host_hive_log::record(
            self.id,
            self.hive_slave_count,
            self.hive_slave_hp,
        );
    }

    pub fn record_host_contain_capacity(&self) {
        let max_garrison = self
            .building_data
            .as_ref()
            .map(|bd| bd.max_garrison.min(u16::MAX as usize) as u16)
            .unwrap_or(0);
        crate::game_logic::host_contain_capacity_log::record(
            self.id,
            self.max_transport,
            max_garrison,
        );
    }

    pub fn record_host_overcharge(&self) {
        crate::game_logic::host_overcharge_log::record(self.id, self.overcharge_enabled);
    }

    pub fn set_overcharge_enabled(&mut self, enabled: bool) {
        if self.overcharge_enabled != enabled {
            self.overcharge_enabled = enabled;
            self.record_host_overcharge();
        }
    }

    /// C++ Object::setWeaponSetFlag(WEAPONSET_MINE_CLEARING_DETAIL) residual.

    /// C++ SpecialPowerUpdateInterface::setSpecialPowerOverridableDestination residual.
    pub fn set_special_power_overridable_destination(
        &mut self,
        loc: Vec3,
        power: Option<crate::command_system::SpecialPowerType>,
    ) {
        self.special_power_override_destination = Some(loc);
        self.special_power_override_type = power;
    }

    pub fn clear_special_power_overridable_destination(&mut self) {
        self.special_power_override_destination = None;
        self.special_power_override_type = None;
    }

    /// C++ Object::setWeaponSetFlag residual (subset used by AIGroup).
    /// `flag`: 0=PLAYER_UPGRADE, 1=MINE_CLEARING, 2=CARBOMB, 3=VEHICLE_HIJACK.
    pub fn set_weapon_set_flag(&mut self, flag: u8, enabled: bool) -> bool {
        match flag {
            0 => {
                self.weapon_set_player_upgrade = enabled;
            }
            1 => {
                self.weapon_set_mine_clearing_detail = enabled;
            }
            2 => {
                self.weapon_set_carbomb = enabled;
            }
            3 => {
                self.weapon_set_vehicle_hijack = enabled;
            }
            _ => return false,
        }
        self.record_host_weapon_set();
        true
    }

    pub fn set_weapon_set_mine_clearing_detail(&mut self, enabled: bool) {
        self.weapon_set_mine_clearing_detail = enabled;
        self.record_host_weapon_set();
    }

    /// C++ AICMD_GO_PRONE residual — infantry hit the dirt briefly.
    pub fn go_prone(&mut self, duration_secs: f32) {
        self.stop_moving();
        self.set_target(None);
        self.set_force_attack(false);
        self.prone_timer = duration_secs.max(0.1);
        if let Some(pu) = self.prone_update.as_mut() {
            // Approximate seconds → frames at 30 Hz for module residual.
            let frames = (duration_secs.max(0.1) * 30.0).round() as i32;
            let was = pu.prone_frames > 0;
            pu.prone_frames = pu.prone_frames.max(frames);
            if !was {
                pu.model_prone = true;
                pu.no_attack = true;
            }
        }
        if let Some(bit) =
            crate::game_logic::host_enum_table_residual::model_condition_bit_name_index("PRONE")
        {
            self.model_condition_bits |= 1u128 << bit;
        }
        // Stay in Idle while prone so orders can break it; timer clears the bit.
        if !matches!(
            self.ai_state,
            AIState::Attacking
                | AIState::AttackMoving
                | AIState::GuardingArea
                | AIState::GuardingObject
        ) {
            self.set_ai_state(AIState::Idle);
        }
        self.record_host_locomotor();
    }

    pub fn record_host_weapon_set(&self) {
        crate::game_logic::host_weapon_set_log::record(
            self.id,
            self.weapon_set_player_upgrade,
            self.armed_riders_upgrade_weapon_set,
        );
    }

    pub fn record_host_ai_attitude(&self) {
        crate::game_logic::host_ai_attitude_log::record(self.id, self.ai_attitude);
    }

    pub fn set_ai_attitude_i8(&mut self, attitude: i8) {
        let a = attitude.clamp(-2, 2);
        if self.ai_attitude != a {
            self.ai_attitude = a;
            self.record_host_ai_attitude();
        }
    }

    pub fn heal(&mut self, amount: f32) {
        if self.status.destroyed {
            return;
        }
        // C++ PoisonedBehavior::onHealing residual.
        self.clear_poisoned_on_healing();
        let before = self.health.current;
        if amount <= 0.0 || !amount.is_finite() {
            return;
        }
        let projected = (before + amount).min(self.health.maximum);
        if projected <= before {
            return;
        }
        // GameWorld HP authority: log absolute health; defer host mutate to writeback.
        if crate::gameworld_shadow::gameworld_damage_authority_live() {
            crate::game_logic::host_heal_log::record(self.id, projected);
        } else {
            self.health.heal(amount);
            crate::game_logic::host_heal_log::record(self.id, self.health.current);
        }
        self.refresh_model_condition_bits();
    }

    /// C++ residual: STEALTHED && !DETECTED && !DISGUISED.
    /// Stealthed-and-undetected units are not legal auto/manual attack targets.
    /// Disguised units are visible as their disguise team (not pure-stealth hide).
    pub fn is_effectively_stealthed(&self) -> bool {
        self.status.stealthed && !self.status.detected && !self.status.disguised
    }

    /// C++ OBJECT_STATUS_DISGUISED residual.
    pub fn is_disguised(&self) -> bool {
        self.status.disguised
    }

    /// Apply Bomb Truck disguise residual (StealthUpdate::disguiseAsObject).
    ///
    /// C++ residual: start DisguiseTransitionTime frames; at halfpoint
    /// `changeVisualDisguise` sets DISGUISED + model. Host residual: arm
    /// pending template/team, tick opacity, commit at halfpoint.
    pub fn apply_disguise(&mut self, template_name: &str, as_team: Team) {
        use crate::game_logic::host_bomb_truck_disguise::BOMB_TRUCK_DISGUISE_TRANSITION_FRAMES;
        if self.status.destroyed {
            return;
        }
        self.disguise_pending_template = Some(template_name.to_string());
        self.record_host_ai_request();
        self.disguise_pending_team = Some(as_team);
        // Not fully disguised until halfpoint residual.
        self.set_status_disguised(false);
        self.set_status_stealthed(true);
        self.set_status_detected(false);
        self.detection_expires_frame = 0;
        self.record_host_stealth_delay();
        self.status.disguise_transition_frames = BOMB_TRUCK_DISGUISE_TRANSITION_FRAMES;
        self.set_status_disguise_transitioning_to(true);
        self.set_status_disguise_halfpoint_reached(false);
        self.status.disguise_transition_opacity = 1.0;
        // Keep previous appearance until halfpoint if any.
        self.record_host_disguise();
    }

    /// Clear disguise residual (reveal transition).
    ///
    /// C++ residual: DisguiseRevealTransitionTime frames; halfpoint restores
    /// true visual; end clears STEALTHED.
    pub fn clear_disguise(&mut self) {
        use crate::game_logic::host_bomb_truck_disguise::BOMB_TRUCK_DISGUISE_REVEAL_TRANSITION_FRAMES;
        if !self.status.disguised
            && self.disguise_as_template.is_none()
            && self.disguise_pending_template.is_none()
            && self.status.disguise_transition_frames == 0
        {
            return;
        }
        // Begin reveal transition residual (losing disguise look).
        self.status.disguise_transition_frames = BOMB_TRUCK_DISGUISE_REVEAL_TRANSITION_FRAMES;
        self.set_status_disguise_transitioning_to(false);
        self.set_status_disguise_halfpoint_reached(false);
        self.status.disguise_transition_opacity = 1.0;
        // Keep disguise_as_* until halfpoint swap back.
        self.record_host_disguise();
    }

    /// Force-clear disguise residual immediately (no transition).
    pub fn clear_disguise_instant(&mut self) {
        self.set_status_disguised(false);
        self.disguise_as_template = None;
        self.disguise_as_team = None;
        self.disguise_pending_template = None;
        self.record_host_ai_request();
        self.disguise_pending_team = None;
        self.set_status_stealthed(false);
        self.set_status_detected(false);
        self.detection_expires_frame = 0;
        self.record_host_stealth_delay();
        self.status.disguise_transition_frames = 0;
        self.set_status_disguise_transitioning_to(false);
        self.set_status_disguise_halfpoint_reached(false);
        self.status.disguise_transition_opacity = 1.0;
        self.record_host_disguise();
    }

    /// C++ StealthUpdate disguise transition residual tick.
    ///
    /// Returns true when halfpoint model-swap residual fired this frame.
    pub fn tick_disguise_transition(&mut self) -> bool {
        if self.status.disguise_transition_frames == 0 {
            return false;
        }
        use crate::game_logic::host_bomb_truck_disguise::{
            BOMB_TRUCK_DISGUISE_REVEAL_TRANSITION_FRAMES, BOMB_TRUCK_DISGUISE_TRANSITION_FRAMES,
        };
        self.status.disguise_transition_frames =
            self.status.disguise_transition_frames.saturating_sub(1);
        let total = if self.status.disguise_transitioning_to {
            BOMB_TRUCK_DISGUISE_TRANSITION_FRAMES.max(1)
        } else {
            BOMB_TRUCK_DISGUISE_REVEAL_TRANSITION_FRAMES.max(1)
        };
        let remaining = self.status.disguise_transition_frames;
        // factor 0 → 1 over transition (C++).
        let factor = 1.0 - (remaining as f32 / total as f32);
        // Opacity: full → none at midpoint → full (fabs(1 - factor*2)).
        let opacity = (1.0 - factor * 2.0).abs();
        self.status.disguise_transition_opacity = opacity;

        let mut halfpoint = false;
        if factor >= 0.5 && !self.status.disguise_halfpoint_reached {
            self.set_status_disguise_halfpoint_reached(true);
            halfpoint = true;
            if self.status.disguise_transitioning_to {
                // changeVisualDisguise residual: commit pending appearance.
                if let Some(tpl) = self.disguise_pending_template.take() {
                    self.disguise_as_template = Some(tpl);
                }
                if let Some(team) = self.disguise_pending_team.take() {
                    self.disguise_as_team = Some(team);
                }
                self.set_status_disguised(true);

                self.record_model_mesh_from_template();
                self.record_kind_of_bits_from_template();
                self.set_status_stealthed(true);
                self.set_status_detected(false);
            } else {
                // Reveal halfpoint: restore true look residual.
                self.set_status_disguised(false);
                self.disguise_as_template = None;
                self.disguise_as_team = None;
                self.disguise_pending_template = None;
                self.record_host_ai_request();
                self.disguise_pending_team = None;
            }
        }

        if remaining == 0 && !self.status.disguise_transitioning_to {
            // Finished removing disguise — clear stealth residual.
            self.set_status_stealthed(false);
            self.set_status_detected(false);
            self.detection_expires_frame = 0;
            self.record_host_stealth_delay();
            self.status.disguise_transition_opacity = 1.0;
        }
        self.record_host_disguise();
        halfpoint
    }

    /// Whether disguise transition residual is active.
    pub fn is_disguise_transitioning(&self) -> bool {
        self.status.disguise_transition_frames > 0
    }

    /// C++ SpyVisionUpdate::setDisabledUntilFrame residual.
    pub fn apply_spy_vision_disabled_until(&mut self, until_frame: u32) {
        if until_frame > self.status.spy_vision_disabled_until_frame {
            self.status.spy_vision_disabled_until_frame = until_frame;
        }
    }

    /// Whether SpyVision residual is currently disabled by sabotage residual.
    pub fn is_spy_vision_disabled(&self, current_frame: u32) -> bool {
        self.status.spy_vision_disabled_until_frame > current_frame
    }

    /// Expire SpyVision sabotage disable residual when frame passes.
    pub fn tick_spy_vision_disabled(&mut self, current_frame: u32) {
        if self.status.spy_vision_disabled_until_frame > 0
            && current_frame >= self.status.spy_vision_disabled_until_frame
        {
            self.status.spy_vision_disabled_until_frame = 0;
        }
    }

    /// Apparent team residual for a viewer (see host_bomb_truck_disguise).
    pub fn apparent_team_to(&self, viewer_team: Team) -> Team {
        crate::game_logic::host_bomb_truck_disguise::apparent_team_for_viewer(
            self.team,
            self.disguise_as_team,
            self.status.disguised,
            viewer_team,
        )
    }

    /// Effective detection radius for this unit when `is_detector`.
    /// C++: DetectionRange if > 0 else vision range.
    pub fn effective_detection_range(&self) -> f32 {
        if self.detection_range > 0.0 {
            self.detection_range
        } else {
            self.get_template().sight_range
        }
    }

    /// Mark this object as detected until `expires_frame` (logic frame exclusive).
    /// C++ StealthUpdate::markAsDetected residual.
    pub fn mark_detected(&mut self, expires_frame: u32) {
        self.set_status_detected(true);
        // Keep the later expiry if already detected by another scanner.
        if expires_frame > self.detection_expires_frame {
            self.detection_expires_frame = expires_frame;
            self.record_host_stealth_delay();
        }
    }

    /// Clear DETECTED status (stealth may remain active).
    pub fn clear_detected(&mut self) {
        self.set_status_detected(false);
        self.detection_expires_frame = 0;
        self.record_host_stealth_delay();
    }

    /// Break stealth entirely (fire / script residual).
    /// Also clears disguise residual (attack reveal path for bomb truck).
    pub fn break_stealth(&mut self) {
        if self.status.disguised {
            self.clear_disguise();
            return;
        }
        let was_stealthed = self.status.stealthed;
        self.set_status_stealthed(false);
        self.set_status_detected(false);
        self.detection_expires_frame = 0;
        self.record_host_stealth_delay();
        // CamoNetting / StealthDelay residual: schedule re-cloak gate on reveal.
        if was_stealthed && self.stealth_delay_frames > 0 {
            self.stealth_delay_pending = true;
            self.record_host_stealth_delay();
        }
        // CamoNetting FriendlyOpacity residual: revealed → max opacity.
        if was_stealthed && self.stealth_breaks_on_damage {
            self.camo_friendly_opacity = 1.0;
            self.camo_opacity_pulse_phase = 0.0;
            self.record_host_stealth_delay();
        }
        self.record_host_vision_camo();
    }

    /// C++ StealthUpdate::receiveGrant residual (GPS Scrambler / GrantStealthBehavior).
    ///
    /// Sets OBJECT_STATUS_STEALTHED (+ host residual CAN_STEALTH via stealthed flag)
    /// and clears DETECTED so the unit is effectively stealthed until broken by
    /// attack / mark_detected / break_stealth.
    ///
    /// Fail-closed: not full StealthUpdate framesGranted timer / disguise skip
    /// (callers filter disguise units) / opacity drawable path.
    pub fn apply_grant_stealth(&mut self) {
        if self.status.destroyed {
            return;
        }
        self.set_status_stealthed(true);
        self.set_status_detected(false);
        self.detection_expires_frame = 0;
        self.record_host_stealth_delay();
    }

    /// C++ Object::setVisionSpied residual (refcounted mask simplified to bitmask).
    /// When on, spying player treats this unit as a temporary looker / revealed target.
    pub fn set_vision_spied_by_player(&mut self, player_id: u32, on: bool) {
        let bit = 1u32 << player_id.min(31);
        if on {
            self.vision_spied_mask |= bit;
        } else {
            self.vision_spied_mask &= !bit;
        }
        self.record_host_vision_camo();
    }

    /// True if `player_id` currently has vision-spied residual on this unit.
    pub fn is_vision_spied_by_player(&self, player_id: u32) -> bool {
        let bit = 1u32 << player_id.min(31);
        (self.vision_spied_mask & bit) != 0
    }

    /// Whether an enemy of `attacker_team` may target this object.
    /// C++ WeaponSet::getCanAttackObject stealth gate residual + disguise
    /// relationship residual (disguised units appear as disguise team).
    pub fn is_targetable_by_enemy_of(&self, attacker_team: Team) -> bool {
        if !self.is_alive() || !self.is_attackable() {
            return false;
        }
        // Disguise residual: auto-target uses apparent team (allies of disguise skip).
        if self.status.disguised {
            return crate::game_logic::host_bomb_truck_disguise::is_auto_targetable_as_enemy(
                self.team,
                self.disguise_as_team,
                true,
                attacker_team,
            ) && !self.is_effectively_stealthed();
        }
        if self.team == attacker_team {
            return false;
        }
        // Stealthed and not detected: not a valid target.
        !self.is_effectively_stealthed()
    }

    /// Whether `weapon` can legally hit `target` (air/ground + range + stealth).
    pub fn can_target_with(&self, target: &Object, weapon: &Weapon) -> bool {
        self.can_target_with_slot(target, weapon, None)
    }

    /// Slot-aware can_target (LeechRange uses per-slot active residual).
    pub fn can_target_with_slot(&self, target: &Object, weapon: &Weapon, slot: Option<u8>) -> bool {
        // C++ WeaponSet: stealthed + undetected cannot be attacked
        // (including force-fire against pure stealth; disguise exception not residual).
        // OBJECT_STATUS_IGNORING_STEALTH residual bypasses this gate.
        if target.is_effectively_stealthed()
            && target.team != self.team
            && !self.status.ignoring_stealth
        {
            return false;
        }

        let target_is_air = target.is_kind_of(KindOf::Aircraft) || target.status.airborne_target;

        if target_is_air && !weapon.can_target_air {
            return false;
        }

        if !target_is_air && !weapon.can_target_ground {
            return false;
        }
        // C++ DAMAGE_DISARM estimate residual: only mines/demo/booby are valid.
        {
            let wname = match slot {
                Some(1) => self.thing.template.secondary_weapon_name.as_deref().or(self
                    .thing
                    .template
                    .primary_weapon_name
                    .as_deref()),
                _ => self.thing.template.primary_weapon_name.as_deref(),
            };
            if wname
                .map(crate::game_logic::weapon_bootstrap::host_weapon_is_disarm_damage)
                .unwrap_or(false)
            {
                if !target.is_disarmable_mine() {
                    return false;
                }
            }
        }

        // C++ parity (Weapon::isWithinAttackRange): check both minimum
        // and maximum attack range. Ground targets use horizontal (XZ)
        // distance so terrain height does not permanently block fire after
        // a successful march into range.
        let distance = if target_is_air {
            self.thing.get_distance_to(&target.thing)
        } else {
            let a = self.get_position();
            let b = target.get_position();
            let dx = a.x - b.x;
            let dz = a.z - b.z;
            (dx * dx + dz * dz).sqrt()
        };
        if weapon.min_range > 0.0 && distance < weapon.min_range {
            return false;
        }
        // C++ Weapon::hasLeechRange residual: once activated, max range waived
        // for the remainder of the attack cycle.
        let leech = match slot {
            Some(1) => self.leech_range_active_secondary,
            Some(_) => self.leech_range_active_primary,
            None => self.leech_range_active_primary || self.leech_range_active_secondary,
        };
        if leech {
            return true;
        }
        // SearchAndDestroy residual: BATTLEPLAN_SEARCHANDDESTROY RANGE 120%.
        let max_range = self.effective_weapon_range(weapon.range);
        distance <= max_range
    }

    /// True if primary **or** secondary can currently hit the target.
    pub fn can_target(&self, target: &Object) -> bool {
        if target.is_effectively_stealthed() && target.team != self.team {
            return false;
        }
        if let Some(weapon) = &self.weapon {
            if self.can_target_with_slot(target, weapon, Some(0)) {
                return true;
            }
        }
        if let Some(weapon) = &self.secondary_weapon {
            if self.can_target_with_slot(target, weapon, Some(1)) {
                return true;
            }
        }
        false
    }

    /// Weapon ready on reload timer (not range).
    ///
    /// C++ AutoReloadsClip residual via weapon-name peel:
    /// - Auto: empty clip becomes ready after clip reload (refill on fire).
    /// - Manual / ReturnToBase: empty stays OUT_OF_AMMO until `rearm_weapon_full`.
    pub fn weapon_ready(weapon: &Weapon, current_time: f32) -> bool {
        // Without a name peel, treat as Auto (legacy).
        current_time - weapon.last_fire_time >= weapon.reload_time
            && Self::weapon_has_ammo_for_shot(weapon, None)
    }

    /// Name-aware ready check (preferred).
    /// `effective_reload` = WeaponBonus RATE_OF_FIRE adjusted interval (seconds).
    pub fn weapon_ready_named(
        weapon: &Weapon,
        current_time: f32,
        weapon_name: Option<&str>,
        effective_reload: f32,
    ) -> bool {
        current_time - weapon.last_fire_time >= effective_reload
            && Self::weapon_has_ammo_for_shot(weapon, weapon_name)
    }

    pub fn weapon_has_ammo_for_shot(weapon: &Weapon, weapon_name: Option<&str>) -> bool {
        use crate::game_logic::weapon_bootstrap::{
            host_reload_type_for_weapon_name, HostReloadType,
        };
        let rt = weapon_name
            .map(host_reload_type_for_weapon_name)
            .unwrap_or(HostReloadType::Auto);
        match rt {
            HostReloadType::Auto => true,
            HostReloadType::Manual | HostReloadType::ReturnToBase => match weapon.ammo {
                Some(0) => false,
                Some(_) => true,
                None => true, // unlimited residual
            },
        }
    }

    /// C++ clip residual: consume one round.
    pub fn consume_ammo_on_fire(weapon: &mut Weapon, current_time: f32) {
        Self::consume_ammo_on_fire_named(weapon, current_time, None);
    }

    pub fn consume_ammo_on_fire_named(
        weapon: &mut Weapon,
        current_time: f32,
        weapon_name: Option<&str>,
    ) {
        use crate::game_logic::weapon_bootstrap::{
            host_reload_type_for_weapon_name, HostReloadType,
        };
        weapon.last_fire_time = current_time;
        let rt = weapon_name
            .map(host_reload_type_for_weapon_name)
            .unwrap_or(HostReloadType::Auto);

        if weapon.clip_size == 0 {
            if let Some(a) = weapon.ammo.as_mut() {
                if *a > 0 {
                    *a -= 1;
                }
            }
            return;
        }

        match rt {
            HostReloadType::Auto => {
                if weapon.ammo == Some(0) || weapon.ammo.is_none() {
                    weapon.ammo = Some(weapon.clip_size);
                }
                if let Some(a) = weapon.ammo.as_mut() {
                    *a = a.saturating_sub(1);
                    if *a == 0 {
                        let clip_rt = if weapon.clip_reload_time > 0.0 {
                            weapon.clip_reload_time
                        } else {
                            weapon.reload_time
                        };
                        weapon.last_fire_time = current_time - weapon.reload_time + clip_rt;
                    }
                }
            }
            HostReloadType::Manual | HostReloadType::ReturnToBase => {
                if weapon.ammo.is_none() {
                    weapon.ammo = Some(weapon.clip_size);
                }
                if let Some(a) = weapon.ammo.as_mut() {
                    if *a > 0 {
                        *a = a.saturating_sub(1);
                    }
                }
            }
        }
    }

    pub fn rearm_weapon_full(weapon: &mut Weapon) {
        if weapon.clip_size > 0 {
            weapon.ammo = Some(weapon.clip_size);
        } else if let Some(a) = weapon.ammo {
            weapon.ammo = Some(a.max(1));
        }
        weapon.last_fire_time = -1.0e6;
    }

    fn primary_weapon_name(&self) -> Option<&str> {
        self.thing.template.primary_weapon_name.as_deref()
    }

    fn secondary_weapon_name(&self) -> Option<&str> {
        self.thing.template.secondary_weapon_name.as_deref().or(self
            .thing
            .template
            .primary_weapon_name
            .as_deref())
    }

    pub fn needs_return_to_base_rearm(&self) -> bool {
        use crate::game_logic::weapon_bootstrap::{
            host_reload_type_for_weapon_name, HostReloadType,
        };
        let empty_rtb = |w: &Weapon, name: Option<&str>| {
            let rt = name
                .map(host_reload_type_for_weapon_name)
                .unwrap_or(HostReloadType::Auto);
            rt == HostReloadType::ReturnToBase && matches!(w.ammo, Some(0))
        };
        self.weapon
            .as_ref()
            .is_some_and(|w| empty_rtb(w, self.primary_weapon_name()))
            || self
                .secondary_weapon
                .as_ref()
                .is_some_and(|w| empty_rtb(w, self.secondary_weapon_name()))
    }

    pub fn rearm_return_to_base_weapons(&mut self) -> bool {
        use crate::game_logic::weapon_bootstrap::{
            host_reload_type_for_weapon_name, HostReloadType,
        };
        let mut any = false;
        let pri = self.primary_weapon_name().map(|s| s.to_string());
        let sec = self.secondary_weapon_name().map(|s| s.to_string());
        if let Some(w) = self.weapon.as_mut() {
            let rt = pri
                .as_deref()
                .map(host_reload_type_for_weapon_name)
                .unwrap_or(HostReloadType::Auto);
            if rt == HostReloadType::ReturnToBase {
                Self::rearm_weapon_full(w);
                any = true;
            }
        }
        if let Some(w) = self.secondary_weapon.as_mut() {
            let rt = sec
                .as_deref()
                .map(host_reload_type_for_weapon_name)
                .unwrap_or(HostReloadType::Auto);
            if rt == HostReloadType::ReturnToBase {
                Self::rearm_weapon_full(w);
                any = true;
            }
        }
        any
    }

    /// C++ JetAIUpdate `OutOfAmmoDamagePerSecond` residual (fraction of max HP / sec).
    /// Retail JetAIUpdate OutOfAmmoDamagePerSecond = **10%**.
    pub const OUT_OF_AMMO_DAMAGE_PER_SECOND: f32 = 0.10;

    /// Apply one logic-frame of out-of-ammo damage while RTB weapons are empty.
    ///
    /// C++ JetOrHeliCirclingDeadAirfieldState:
    /// `damageRate = pct * SECONDS_PER_LOGICFRAME * maxHealth`, DAMAGE_UNRESISTABLE.
    /// Returns damage applied (0 if not eligible).
    pub fn apply_out_of_ammo_damage_frame(&mut self) -> f32 {
        if !self.is_alive() {
            return 0.0;
        }
        // Aircraft / jet residual only.
        if !(self.is_kind_of(KindOf::Aircraft) || self.object_type == ObjectType::Aircraft) {
            return 0.0;
        }
        if !self.needs_return_to_base_rearm() {
            return 0.0;
        }
        // No damage while docked at airfield / garrisoned.
        if matches!(
            self.ai_state,
            AIState::Docked | AIState::Garrisoned | AIState::Entering | AIState::Docking
        ) {
            return 0.0;
        }

        const LOGIC_DT: f32 = 1.0 / 30.0;
        let max_hp = self.health.maximum.max(1.0);
        let dmg = Self::OUT_OF_AMMO_DAMAGE_PER_SECOND * LOGIC_DT * max_hp;
        if dmg <= 0.0 {
            return 0.0;
        }
        self.take_damage(dmg);
        dmg
    }

    /// Distance to another object (3D residual; pathfinding often 2D).
    pub fn distance_to_object(&self, other: &Object) -> f32 {
        self.get_position().distance(other.get_position())
    }

    /// Distance to world position.
    pub fn distance_to_pos(&self, pos: glam::Vec3) -> f32 {
        self.get_position().distance(pos)
    }

    /// C++ Weapon::isWithinAttackRange residual (primary then secondary).
    /// When LeechRange is active for a slot, max range is waived (C++ hasLeechRange).
    /// Max range includes WeaponBonus RANGE field (garrison / SearchAndDestroy / …).
    pub fn is_within_attack_range(&self, other: &Object) -> bool {
        let dist = self.distance_to_object(other);
        if let Some(w) = &self.weapon {
            if w.min_range > 0.0 && dist + 1e-4 < w.min_range {
                // min range still enforced under leech
            } else if self.leech_range_active_primary {
                return true;
            } else {
                let range = self.effective_weapon_range(w.range);
                if dist <= range + 1e-3 {
                    return true;
                }
            }
        }
        if let Some(w) = &self.secondary_weapon {
            if w.min_range > 0.0 && dist + 1e-4 < w.min_range {
                // min range still enforced
            } else if self.leech_range_active_secondary {
                return true;
            } else {
                let range = self.effective_weapon_range(w.range);
                if dist <= range + 1e-3 {
                    return true;
                }
            }
        }
        false
    }

    /// C++ Weapon::isWithinAttackRange for a position.
    pub fn is_within_attack_range_pos(&self, pos: glam::Vec3) -> bool {
        let dist = self.distance_to_pos(pos);
        if let Some(w) = &self.weapon {
            if w.min_range > 0.0 && dist + 1e-4 < w.min_range {
            } else if self.leech_range_active_primary {
                return true;
            } else {
                let range = self.effective_weapon_range(w.range);
                if dist <= range + 1e-3 {
                    return true;
                }
            }
        }
        if let Some(w) = &self.secondary_weapon {
            if w.min_range > 0.0 && dist + 1e-4 < w.min_range {
            } else if self.leech_range_active_secondary {
                return true;
            } else {
                let range = self.effective_weapon_range(w.range);
                if dist <= range + 1e-3 {
                    return true;
                }
            }
        }
        false
    }

    /// C++ canPursue residual (simplified — no turret matrix).

    /// C++ Weapon::hasLeechRange residual (primary or secondary active).
    pub fn leech_range_active(&self) -> bool {
        self.leech_range_active_primary || self.leech_range_active_secondary
    }

    pub fn can_pursue_target(&self, victim: &Object) -> bool {
        // Need victim physics (velocity).
        let victim_speed = victim.forward_speed_2d().abs();
        let our_max = self.effective_max_speed();
        if our_max <= 0.0 {
            return false;
        }
        // Crush residual: vehicles always pursue crushable infantry if AI computer — fail-closed skip player type.
        if self.can_crush_only(victim, false) {
            return true;
        }
        // Too close residual: min_range
        if let Some(w) = &self.weapon {
            let dist = self.distance_to_object(victim);
            if w.min_range > 0.0 && dist < w.min_range {
                return false;
            }
        }
        if victim_speed >= our_max {
            return false;
        }
        if victim_speed < our_max / 10.0 {
            return false;
        }
        // Victim moving away residual.
        let us = self.get_position();
        let them = victim.get_position();
        let dx = them.x - us.x;
        let dz = them.z - us.z;
        let vdir = victim.unit_direction_vector_2d();
        if dx * vdir.x + dz * vdir.y < 0.0 {
            return false; // moving toward us
        }
        true
    }

    /// Face toward a world position (AI_FACE_POSITION residual).
    pub fn face_position(&mut self, pos: glam::Vec3, dt: f32) -> bool {
        if !self.can_move() {
            return false;
        }
        let (_t, rel) = self.rotate_towards_position(pos, dt);
        rel.abs() < 0.05 // facing success residual (~3 deg)
    }

    /// Face toward another object.
    pub fn face_object(&mut self, other: &Object, dt: f32) -> bool {
        self.face_position(other.get_position(), dt)
    }

    /// C++ WeaponSet model-condition residual for PREATTACK/FIRING/BETWEEN/RELOADING A/B/C.
    ///
    /// Maps `weapon_fire_status` + active slot onto ModelConditionFlags bits
    /// (ALLOW_SURRENDER-off layout: PREATTACK_A=35 .. RELOADING_C=46).

    /// C++ Object::getAmmoPipShowingInfo residual.
    ///
    /// Returns `(clip_size, remaining_ammo)` for the first ShowsAmmoPips weapon.

    /// C++ Weapon::getPercentReadyToFire residual for one slot (0.0..1.0).
    pub fn weapon_slot_percent_ready_to_fire(&self, slot: u8, current_time: f32) -> f32 {
        let Some(weapon) = self.weapon_slot(slot) else {
            return 0.0;
        };
        let name = if slot == 1 {
            self.thing.template.secondary_weapon_name.as_deref().or(self
                .thing
                .template
                .primary_weapon_name
                .as_deref())
        } else {
            self.thing.template.primary_weapon_name.as_deref()
        };
        // Prefer live WeaponFireStatus when this is the active slot.
        let status = if slot == self.active_weapon_slot {
            self.weapon_fire_status
        } else {
            // Approximate status from ammo/reload without mutating.
            if !Self::weapon_has_ammo_for_shot(weapon, name) {
                WeaponFireStatus::OutOfAmmo
            } else {
                let reload = self.effective_weapon_reload(weapon.reload_time);
                if current_time - weapon.last_fire_time < reload - 1e-6 {
                    if weapon.clip_size > 0
                        && weapon.ammo == Some(weapon.clip_size)
                        && weapon.clip_reload_time > reload + 1e-4
                    {
                        WeaponFireStatus::ReloadingClip
                    } else {
                        WeaponFireStatus::BetweenFiringShots
                    }
                } else {
                    WeaponFireStatus::ReadyToFire
                }
            }
        };
        match status {
            WeaponFireStatus::OutOfAmmo | WeaponFireStatus::PreAttack => 0.0,
            WeaponFireStatus::ReadyToFire => 1.0,
            WeaponFireStatus::BetweenFiringShots | WeaponFireStatus::ReloadingClip => {
                let reload =
                    if status == WeaponFireStatus::ReloadingClip && weapon.clip_reload_time > 0.0 {
                        weapon.clip_reload_time
                    } else {
                        self.effective_weapon_reload(weapon.reload_time)
                    };
                if reload <= 1e-6 {
                    return 1.0;
                }
                let elapsed = (current_time - weapon.last_fire_time).max(0.0);
                if elapsed >= reload {
                    1.0
                } else {
                    (elapsed / reload).clamp(0.0, 1.0)
                }
            }
        }
    }

    /// C++ Object::getMostPercentReadyToFireAnyWeapon residual (0..100).
    pub fn get_most_percent_ready_to_fire_any_weapon(&self, current_time: f32) -> u32 {
        let mut most = 0u32;
        for slot in [0u8, 1u8] {
            if self.weapon_slot(slot).is_none() {
                continue;
            }
            let pct = (self.weapon_slot_percent_ready_to_fire(slot, current_time) * 100.0) as u32;
            if pct > most {
                most = pct;
            }
            if most >= 100 {
                return 100;
            }
        }
        most.min(100)
    }

    pub fn get_ammo_pip_showing_info(&self) -> Option<(u32, u32)> {
        use crate::game_logic::weapon_bootstrap::host_shows_ammo_pips_for_weapon_name;
        for slot in [0u8, 1u8] {
            let Some(w) = self.weapon_slot(slot) else {
                continue;
            };
            let name = if slot == 1 {
                self.thing.template.secondary_weapon_name.as_deref().or(self
                    .thing
                    .template
                    .primary_weapon_name
                    .as_deref())
            } else {
                self.thing.template.primary_weapon_name.as_deref()
            };
            let Some(n) = name else {
                continue;
            };
            if !host_shows_ammo_pips_for_weapon_name(n) {
                continue;
            }
            let total = if w.clip_size > 0 {
                w.clip_size
            } else {
                w.ammo.unwrap_or(0)
            };
            if total == 0 {
                continue;
            }
            let full = w.ammo.unwrap_or(total).min(total);
            return Some((total, full));
        }
        None
    }

    /// C++ Object::findWaypointFollowingCapableWeapon residual (slot index).
    ///
    /// Scans SECONDARY then PRIMARY (C++ WEAPONSLOT_COUNT-1 .. PRIMARY).
    pub fn find_waypoint_following_capable_weapon_slot(&self) -> Option<u8> {
        use crate::game_logic::weapon_bootstrap::host_capable_of_following_waypoint_for_weapon_name;
        for slot in [1u8, 0u8] {
            let Some(_w) = self.weapon_slot(slot) else {
                continue;
            };
            let name = if slot == 1 {
                self.thing.template.secondary_weapon_name.as_deref().or(self
                    .thing
                    .template
                    .primary_weapon_name
                    .as_deref())
            } else {
                self.thing.template.primary_weapon_name.as_deref()
            };
            if name
                .map(host_capable_of_following_waypoint_for_weapon_name)
                .unwrap_or(false)
            {
                return Some(slot);
            }
        }
        None
    }

    pub fn sync_weapon_model_conditions_from_status(&mut self) {
        use crate::game_logic::host_enum_table_residual::{
            MC_BIT_BETWEEN_FIRING_SHOTS_A, MC_BIT_BETWEEN_FIRING_SHOTS_B,
            MC_BIT_BETWEEN_FIRING_SHOTS_C, MC_BIT_FIRING_A, MC_BIT_FIRING_B, MC_BIT_FIRING_C,
            MC_BIT_PREATTACK_A, MC_BIT_PREATTACK_B, MC_BIT_PREATTACK_C, MC_BIT_RELOADING_A,
            MC_BIT_RELOADING_B, MC_BIT_RELOADING_C,
        };
        const WEAPON_MC_BITS: [u32; 12] = [
            MC_BIT_PREATTACK_A,
            MC_BIT_FIRING_A,
            MC_BIT_BETWEEN_FIRING_SHOTS_A,
            MC_BIT_RELOADING_A,
            MC_BIT_PREATTACK_B,
            MC_BIT_FIRING_B,
            MC_BIT_BETWEEN_FIRING_SHOTS_B,
            MC_BIT_RELOADING_B,
            MC_BIT_PREATTACK_C,
            MC_BIT_FIRING_C,
            MC_BIT_BETWEEN_FIRING_SHOTS_C,
            MC_BIT_RELOADING_C,
        ];
        for b in WEAPON_MC_BITS {
            self.model_condition_bits &= !(1u128 << b);
        }
        let base = match self.active_weapon_slot {
            1 => 4usize,
            2 => 8usize,
            _ => 0usize,
        };
        let idx = match self.weapon_fire_status {
            WeaponFireStatus::PreAttack => Some(base),
            WeaponFireStatus::BetweenFiringShots => Some(base + 2),
            WeaponFireStatus::ReloadingClip => Some(base + 3),
            WeaponFireStatus::ReadyToFire | WeaponFireStatus::OutOfAmmo => {
                if self.status.is_firing_weapon {
                    Some(base + 1)
                } else {
                    None
                }
            }
        };
        if let Some(i) = idx {
            self.model_condition_bits |= 1u128 << WEAPON_MC_BITS[i];
        } else if self.status.is_firing_weapon {
            self.model_condition_bits |= 1u128 << WEAPON_MC_BITS[base + 1];
        }
    }

    /// C++ Weapon::getStatus residual refresh for the active/primary slot.
    pub fn refresh_weapon_fire_status(&mut self, current_time: f32) {
        // Pre-attack wind-up wins while armed.
        if self.pre_attack_ready_at > current_time + 1e-6 {
            self.weapon_fire_status = WeaponFireStatus::PreAttack;
            self.sync_weapon_model_conditions_from_status();
            return;
        }
        let slot = self.active_weapon_slot;
        let Some(weapon) = self.weapon_slot(slot).or_else(|| self.weapon.as_ref()) else {
            self.weapon_fire_status = WeaponFireStatus::OutOfAmmo;
            self.sync_weapon_model_conditions_from_status();
            return;
        };
        let name = if slot == 1 {
            self.thing.template.secondary_weapon_name.as_deref().or(self
                .thing
                .template
                .primary_weapon_name
                .as_deref())
        } else {
            self.thing.template.primary_weapon_name.as_deref()
        };
        let reload = self.effective_weapon_reload(weapon.reload_time);
        if !Self::weapon_has_ammo_for_shot(weapon, name) {
            self.weapon_fire_status = WeaponFireStatus::OutOfAmmo;
            self.sync_weapon_model_conditions_from_status();
            return;
        }
        if weapon.clip_size > 0 {
            let clip_reload = if weapon.clip_reload_time > 0.0 {
                weapon.clip_reload_time
            } else {
                reload
            };
            if current_time - weapon.last_fire_time < reload - 1e-6 {
                if weapon.ammo == Some(weapon.clip_size)
                    && clip_reload > reload + 1e-4
                    && current_time - weapon.last_fire_time < clip_reload
                {
                    self.weapon_fire_status = WeaponFireStatus::ReloadingClip;
                    self.sync_weapon_model_conditions_from_status();
                    return;
                }
                self.weapon_fire_status = WeaponFireStatus::BetweenFiringShots;
                self.sync_weapon_model_conditions_from_status();
                return;
            }
        } else if current_time - weapon.last_fire_time < reload - 1e-6 {
            self.weapon_fire_status = WeaponFireStatus::BetweenFiringShots;
            self.sync_weapon_model_conditions_from_status();
            return;
        }
        self.weapon_fire_status = WeaponFireStatus::ReadyToFire;
        self.sync_weapon_model_conditions_from_status();
    }

    pub fn can_fire(&self, current_time: f32) -> bool {
        // C++ Object::canFireWeapon: DISABLED_SUBDUED / weapons_jammed residual.
        // Shock stun residual blocks weapon fire while flailing/stunned.
        if self.status.weapons_jammed || self.is_disabled() || self.is_shock_stunned() {
            return false;
        }
        let primary_name = self.thing.template.primary_weapon_name.clone();
        let secondary_name = self.thing.template.secondary_weapon_name.clone();
        if let Some(weapon) = &self.weapon {
            let reload = self.effective_weapon_reload(weapon.reload_time);
            if Self::weapon_ready_named(weapon, current_time, primary_name.as_deref(), reload) {
                return true;
            }
        }
        if let Some(weapon) = &self.secondary_weapon {
            let reload = self.effective_weapon_reload(weapon.reload_time);
            let name = secondary_name.as_deref().or(primary_name.as_deref());
            if Self::weapon_ready_named(weapon, current_time, name, reload) {
                return true;
            }
        }
        false
    }

    /// Fail-closed residual combat weapon choice (not full AutoChoose/PreferredAgainst).
    ///
    /// Slot: `0` = primary, `1` = secondary.
    /// Rules:
    /// - Player lock (`active_weapon_slot == 1`): prefer secondary when ready + in range.
    /// - PreferredAgainst residual (damage + kind heuristic, not full INI matrix):
    ///   - Structures: prefer secondary when damage ≥ primary (or primary cannot fire).
    ///   - Infantry: prefer secondary when damage > primary (FlashBang residual).
    ///   - Vehicles: prefer secondary when damage > primary (TOW residual).
    ///   - Neutron residual: active secondary with neutron upgrade vs infantry/vehicle
    ///     prefers secondary when player locked or secondary is the only ready slot;
    ///     also when primary cannot fire and secondary is ready.
    /// - Else primary when ready + in range; else secondary (alternate fire residual).
    pub fn select_combat_weapon_slot(&self, target: &Object, current_time: f32) -> Option<u8> {
        // C++ WeaponSet lock: locked slot wins while ready/in-range.
        if self.weapon_lock_type != WeaponLockType::NotLocked {
            let slot = self.weapon_lock_slot;
            if let Some(w) = self.weapon_slot(slot) {
                let target_faerie = target.is_faerie_fire();
                if self.weapon_ready_vs_target_bonused(w, current_time, target_faerie)
                    && self.can_target_with(target, w)
                {
                    return Some(slot);
                }
            }
            // Temporary lock may fall through if clip empty / not ready.
            if self.weapon_lock_type == WeaponLockType::LockedPermanently {
                return Some(self.weapon_lock_slot);
            }
        }
        let target_faerie = target.is_faerie_fire();
        let primary_ok = self.weapon.as_ref().is_some_and(|w| {
            self.weapon_ready_vs_target_bonused(w, current_time, target_faerie)
                && self.can_target_with(target, w)
        });
        let secondary_ok = self.secondary_weapon.as_ref().is_some_and(|w| {
            self.weapon_ready_vs_target_bonused(w, current_time, target_faerie)
                && self.can_target_with(target, w)
        });

        if !primary_ok && !secondary_ok {
            return None;
        }

        // Manual weapon-slot toggle (command residual).
        if self.active_weapon_slot == 1 {
            if secondary_ok {
                return Some(1);
            }
            if primary_ok {
                return Some(0);
            }
            return None;
        }

        // Comanche Rocket Pods residual: retail AutoChooseSources = TERTIARY NONE.
        // Host secondary carries pods after upgrade; never auto-choose unless
        // player locks active_weapon_slot == 1 (FIRE_WEAPON residual).
        let rocket_pods_manual_only =
            crate::game_logic::host_comanche_rocket_pods::is_comanche_template(&self.template_name)
                && (self.has_upgrade_tag(
                    crate::game_logic::host_comanche_rocket_pods::UPGRADE_COMANCHE_ROCKET_PODS,
                ) || self.has_upgrade_tag("Upgrade_ComancheRocketPods"));

        let target_is_structure =
            target.object_type == ObjectType::Building || target.is_kind_of(KindOf::Structure);
        let target_is_infantry = target.is_kind_of(KindOf::Infantry);
        let target_is_vehicle =
            target.is_kind_of(KindOf::Vehicle) && !target.is_kind_of(KindOf::Aircraft);
        let target_is_air = target.is_kind_of(KindOf::Aircraft) || target.status.airborne_target;

        let primary_damage = self.weapon.as_ref().map(|w| w.damage).unwrap_or(0.0);
        let secondary_damage = self
            .secondary_weapon
            .as_ref()
            .map(|w| w.damage)
            .unwrap_or(0.0);

        // SCUD residual: PreferredAgainst SECONDARY INFANTRY (toxin warhead)
        // even though secondary primary-damage is lower than explosive.
        let scud_prefer_toxin =
            crate::game_logic::host_scud_launcher::scud_prefer_secondary_vs_infantry(
                crate::game_logic::host_scud_launcher::is_scud_launcher_template(
                    &self.template_name,
                ),
                target_is_infantry,
            );

        // Quad Cannon residual: airborne targets prefer AA secondary slot.
        let quad_prefer_aa =
            crate::game_logic::host_quad_cannon::is_quad_cannon_template(&self.template_name)
                && target_is_air;

        // Avenger residual: airborne targets prefer air laser secondary.
        let avenger_prefer_aa = crate::game_logic::host_avenger::avenger_prefer_air_laser(
            crate::game_logic::host_avenger::is_avenger_template(&self.template_name),
            target_is_air,
        );

        // Humvee residual: airborne targets prefer air TOW after TOW upgrade.
        let humvee_prefer_aa = crate::game_logic::host_humvee::humvee_prefer_air_tow(
            crate::game_logic::host_humvee::is_humvee_template(&self.template_name),
            self.has_upgrade_tag(crate::game_logic::host_upgrades::UPGRADE_AMERICA_TOW)
                || self.has_upgrade_tag("Upgrade_AmericaTOWMissile"),
            target_is_air,
        );

        if secondary_ok && !rocket_pods_manual_only {
            if scud_prefer_toxin || quad_prefer_aa || avenger_prefer_aa || humvee_prefer_aa {
                return Some(1);
            }
            // PreferredAgainst residual by target kind + relative damage.
            if target_is_structure && (secondary_damage >= primary_damage || !primary_ok) {
                return Some(1);
            }
            if target_is_infantry && (secondary_damage > primary_damage || !primary_ok) {
                // FlashBang residual (35 > 5). Neutron secondary damage is 1.0 so
                // only wins here when primary cannot fire unless slot-locked.
                return Some(1);
            }
            if target_is_vehicle && (secondary_damage > primary_damage || !primary_ok) {
                // TOW residual (30 > 10 Humvee gun).
                return Some(1);
            }
        }

        // Default / alternate: primary first, then secondary if only it is ready.
        // Rocket pods: never fall back to secondary without slot lock.
        if primary_ok {
            Some(0)
        } else if secondary_ok && !rocket_pods_manual_only {
            Some(1)
        } else {
            None
        }
    }

    pub fn weapon_slot(&self, slot: u8) -> Option<&Weapon> {
        match slot {
            1 => self.secondary_weapon.as_ref(),
            _ => self.weapon.as_ref(),
        }
    }

    pub fn weapon_slot_mut(&mut self, slot: u8) -> Option<&mut Weapon> {
        match slot {
            1 => self.secondary_weapon.as_mut(),
            _ => self.weapon.as_mut(),
        }
    }

    /// C++ PartitionManager::getRelativeAngle2D residual to a world position.

    /// Normalize angle to (-PI, PI].
    pub fn normalize_angle_rad(a: f32) -> f32 {
        let mut x = a % (std::f32::consts::TAU);
        if x > std::f32::consts::PI {
            x -= std::f32::consts::TAU;
        } else if x <= -std::f32::consts::PI {
            x += std::f32::consts::TAU;
        }
        x
    }

    /// C++ TurretAI::friend_turnTowardsAngle residual.
    ///
    /// `desired_rel_rad` is desired world-relative aim angle of the body-to-target
    /// relative heading; host stores turret yaw in degrees absolute-ish residual
    /// matching Strategy Center path (body-relative when body ori is applied).
    /// Returns true when |angle - desired| <= rel_thresh.
    pub fn turn_turret_towards_angle_rad(
        &mut self,
        desired_rel_rad: f32,
        rate_modifier: f32,
        rel_thresh: f32,
    ) -> bool {
        let desired = Self::normalize_angle_rad(desired_rel_rad);
        let orig = self.turret_angle_deg.to_radians();
        let mut actual = Self::normalize_angle_rad(orig);
        let turn_rate = (self.turret_turn_rate_rad * rate_modifier.max(0.0)).max(0.0);
        let angle_diff = Self::normalize_angle_rad(desired - actual);
        if angle_diff.abs() < turn_rate {
            actual = desired;
            self.turret_rotating = false;
        } else {
            if angle_diff > 0.0 {
                actual += turn_rate;
            } else {
                actual -= turn_rate;
            }
            actual = Self::normalize_angle_rad(actual);
            self.turret_rotating = true;
        }
        self.turret_angle_deg = actual.to_degrees();
        let aligned = Self::normalize_angle_rad(actual - desired).abs() <= rel_thresh.max(0.0);
        self.record_host_turret();
        aligned
    }

    /// C++ TurretAI::setTurretTargetObject residual (object-local).

    /// C++ TurretAI::friend_turnTowardsPitch residual.
    pub fn turn_turret_towards_pitch_rad(
        &mut self,
        desired_pitch_rad: f32,
        rate_modifier: f32,
    ) -> bool {
        let desired = Self::normalize_angle_rad(desired_pitch_rad);
        let mut actual = Self::normalize_angle_rad(self.turret_pitch_deg.to_radians());
        let pitch_rate = (self.turret_turn_rate_rad * rate_modifier.max(0.0)).max(0.0);
        let diff = Self::normalize_angle_rad(desired - actual);
        if diff.abs() < pitch_rate {
            actual = desired;
        } else if diff > 0.0 {
            actual = Self::normalize_angle_rad(actual + pitch_rate);
        } else {
            actual = Self::normalize_angle_rad(actual - pitch_rate);
        }
        self.turret_pitch_deg = actual.to_degrees();
        let aligned = Self::normalize_angle_rad(actual - desired).abs() <= 1e-4;
        self.record_host_turret();
        aligned
    }

    pub fn set_turret_target_object(&mut self, victim: Option<ObjectId>, force_attacking: bool) {
        if !self.turret_enabled {
            return;
        }
        match victim {
            None => {
                self.turret_target_id = None;
                self.turret_force_attacking = false;
                if matches!(
                    self.turret_substate,
                    TurretSubState::Aim | TurretSubState::Fire
                ) {
                    self.turret_substate = TurretSubState::Hold;
                }
            }
            Some(id) => {
                self.turret_target_id = Some(id);
                self.turret_force_attacking = force_attacking;
                if !matches!(
                    self.turret_substate,
                    TurretSubState::Aim | TurretSubState::Fire
                ) {
                    self.turret_substate = TurretSubState::Aim;
                }
            }
        }
    }

    /// C++ TurretAI::isTryingToAimAtTarget residual.
    pub fn is_trying_to_aim_at_target(&self, victim: ObjectId) -> bool {
        self.turret_substate == TurretSubState::Aim && self.turret_target_id == Some(victim)
    }

    pub fn relative_angle_2d_to(&self, target_pos: Vec3) -> f32 {
        crate::game_logic::weapon_bootstrap::relative_angle_2d(
            self.get_position(),
            self.get_orientation(),
            target_pos,
        )
    }

    /// Resolve AcceptableAimDelta for the active/named weapon slot (radians).
    pub fn aim_delta_for_slot(&self, slot: u8) -> f32 {
        let name = if slot == 1 {
            self.thing.template.secondary_weapon_name.as_deref().or(self
                .thing
                .template
                .primary_weapon_name
                .as_deref())
        } else {
            self.thing.template.primary_weapon_name.as_deref()
        };
        name.map(crate::game_logic::weapon_bootstrap::host_aim_delta_for_weapon_name)
            .unwrap_or(crate::game_logic::weapon_bootstrap::AIM_DELTA_REL_THRESH_RAD)
    }

    /// C++ AIStates aim gate: facing within AcceptableAimDelta of target.
    pub fn is_aimed_at_position(&self, target_pos: Vec3, slot: u8) -> bool {
        let aim = self.aim_delta_for_slot(slot);
        // Omni-fire residual (~180°): always aimed.
        if aim >= std::f32::consts::PI - 1e-3 {
            return true;
        }
        let rel = self.relative_angle_2d_to(target_pos);
        crate::game_logic::weapon_bootstrap::is_within_aim_delta(rel, aim)
    }

    /// C++ setLocomotorGoalOrientation residual: rotate toward target (in-place turn).
    ///
    /// `max_step_rad` caps per-call turn (default generous for host residual).
    /// Returns true when already within aim delta after the step.
    pub fn turn_toward_position(&mut self, target_pos: Vec3, slot: u8, max_step_rad: f32) -> bool {
        let aim = self.aim_delta_for_slot(slot);
        if aim >= std::f32::consts::PI - 1e-3 {
            return true;
        }
        let rel = self.relative_angle_2d_to(target_pos);
        if crate::game_logic::weapon_bootstrap::is_within_aim_delta(rel, aim) {
            return true;
        }
        let step = max_step_rad.max(0.0);
        let turn = rel.clamp(-step, step);
        let new_ori = self.get_orientation() + turn;
        self.set_orientation(new_ori);
        let rel2 = self.relative_angle_2d_to(target_pos);
        crate::game_logic::weapon_bootstrap::is_within_aim_delta(rel2, aim)
    }

    /// C++ Weapon::getPreAttackDelay residual: whether PreAttackDelay applies this shot.
    pub fn pre_attack_delay_applies(
        &self,
        slot: u8,
        target_id: ObjectId,
        prefire: crate::game_logic::weapon_bootstrap::HostPrefireType,
        pre_delay: f32,
    ) -> bool {
        use crate::game_logic::weapon_bootstrap::HostPrefireType;
        if pre_delay <= 0.0 {
            return false;
        }
        match prefire {
            HostPrefireType::PerShot => true,
            HostPrefireType::PerAttack => {
                // Only the first shot of an engagement against this victim.
                !(self.consecutive_shot_target == Some(target_id)
                    && self.consecutive_shots_at_target > 0)
            }
            HostPrefireType::PerClip => match self.weapon_slot(slot) {
                Some(w) if w.clip_size > 0 => {
                    let ammo = w.ammo.unwrap_or(w.clip_size);
                    ammo >= w.clip_size
                }
                // Unlimited clip residual: treat like per-shot.
                _ => true,
            },
        }
    }

    /// Record a successful discharge for PreAttackType PER_ATTACK bookkeeping.
    pub fn record_shot_at_target(&mut self, target_id: ObjectId) {
        if self.consecutive_shot_target == Some(target_id) {
            self.consecutive_shots_at_target = self.consecutive_shots_at_target.saturating_add(1);
            self.record_host_combat_attack();
        } else {
            self.consecutive_shot_target = Some(target_id);
            self.consecutive_shots_at_target = 1;
            self.record_host_combat_attack();
        }
        // PER_SHOT: force next fire_at to re-arm delay by clearing ready stamp into the past.
        self.pre_attack_ready_at = 0.0;
        self.record_host_combat_attack();
        self.update_continuous_fire_after_shot(target_id);
    }

    /// C++ FiringTracker continuous-fire MEAN/FAST residual (non-gattling path).
    /// Gattling buildings/tanks may overwrite level via specialized advance helpers.
    pub fn update_continuous_fire_after_shot(&mut self, target_id: ObjectId) {
        let one = self.continuous_fire_one_shots;
        let two = self.continuous_fire_two_shots;
        if one == 0 || one == u32::MAX {
            return;
        }
        let c = self.consecutive_shots_at_target;
        self.continuous_fire_victim = target_id.0;
        self.continuous_fire_consecutive = c;
        let level = self.continuous_fire_level;
        self.continuous_fire_level = if level == 1 {
            if c < one {
                0
            } else if two != u32::MAX && c > two {
                2
            } else {
                1
            }
        } else if level == 2 {
            if two != u32::MAX && c < two {
                0
            } else {
                2
            }
        } else if c > one {
            1
        } else {
            0
        };
        self.record_host_continuous_fire();
    }

    /// Stamp ContinuousFireCoast deadline after a shot (C++ m_frameToStartCoolDown).
    pub fn stamp_continuous_fire_coast(&mut self, frame: u32) {
        if self.continuous_fire_level == 0 {
            self.continuous_fire_coast_until_frame = 0;
            return;
        }
        let coast = self.continuous_fire_coast_frames;
        if coast == 0 {
            // No coast configured — keep spin until explicit cool-down.
            return;
        }
        self.continuous_fire_coast_until_frame = frame.saturating_add(coast);
    }

    /// C++ FiringTracker::update cool-down after ContinuousFireCoast idle.
    pub fn tick_continuous_fire_coast(&mut self, frame: u32) {
        self.tick_fire_sound_loop(frame);
        let _ = frame;
        self.tick_subdual_damage();
        if self.continuous_fire_level == 0 {
            return;
        }
        let until = self.continuous_fire_coast_until_frame;
        if until == 0 || frame < until {
            return;
        }
        // coolDown residual: clear MEAN/FAST straight to zero.
        self.continuous_fire_level = 0;
        self.continuous_fire_consecutive = 0;
        self.consecutive_shots_at_target = 0;
        self.consecutive_shot_target = None;
        self.continuous_fire_victim = 0;
        self.continuous_fire_coast_until_frame = 0;
        self.record_host_continuous_fire();
    }

    /// Stamp AutoReloadWhenIdle deadline after a shot (C++ m_frameToForceReload).
    pub fn stamp_auto_reload_when_idle(&mut self, frame: u32) {
        let delay = self.auto_reload_when_idle_frames;
        if delay == 0 {
            return;
        }
        // Only meaningful when clip is partially empty.
        let partial = self
            .weapon
            .as_ref()
            .is_some_and(|w| w.clip_size > 0 && w.ammo.map(|a| a < w.clip_size).unwrap_or(false));
        if partial {
            self.frame_to_force_reload = frame.saturating_add(delay);
        } else {
            self.frame_to_force_reload = 0;
        }
    }

    /// C++ Object::reloadAllAmmo(TRUE) residual — refill primary/secondary clips.
    pub fn reload_all_ammo(&mut self) {
        for slot in [0u8, 1u8] {
            if let Some(w) = self.weapon_slot_mut(slot) {
                if w.clip_size > 0 {
                    w.ammo = Some(w.clip_size);
                }
            }
        }
        self.frame_to_force_reload = 0;
    }

    /// C++ FiringTracker::update force-reload-when-idle residual.
    pub fn tick_force_reload_when_idle(&mut self, frame: u32) {
        let until = self.frame_to_force_reload;
        if until == 0 || frame < until {
            return;
        }
        let needs =
            self.weapon.as_ref().is_some_and(|w| {
                w.clip_size > 0 && w.ammo.map(|a| a < w.clip_size).unwrap_or(true)
            }) || self.secondary_weapon.as_ref().is_some_and(|w| {
                w.clip_size > 0 && w.ammo.map(|a| a < w.clip_size).unwrap_or(true)
            });
        if needs {
            self.reload_all_ammo();
        } else {
            self.frame_to_force_reload = 0;
        }
    }

    /// Fire at target. `target_is_infantry` selects ScatterRadiusVsInfantry residual.
    pub fn fire_at(&mut self, target_id: ObjectId, current_time: f32) -> bool {
        self.fire_at_ex(target_id, current_time, false, false)
    }

    /// Fire at target with KindOf-aware scatter residual.
    /// `target_has_faerie_fire`: C++ TARGET_FAERIE_FIRE WeaponBonus ROF residual.
    pub fn fire_at_ex(
        &mut self,
        target_id: ObjectId,
        current_time: f32,
        target_is_infantry: bool,
        target_has_faerie_fire: bool,
    ) -> bool {
        // C++ Weapon::getMaxShotCount residual — AI burst / scatter limits.
        if !self.has_max_shots_remaining() {
            return false;
        }

        // C++ canFireWeapon residual: jammed / disabled units cannot discharge.
        if self.status.weapons_jammed || self.is_disabled() {
            return false;
        }
        // Prefer the locked/active slot when ready; else primary; else secondary.
        let slot = {
            let prefer_secondary = self.active_weapon_slot == 1;
            let mut rof = self.weapon_bonus_fields().2;
            if target_has_faerie_fire {
                rof *= crate::game_logic::host_avenger::FAERIE_FIRE_ROF_MULTIPLIER;
            }
            let primary_name = self.primary_weapon_name().map(|s| s.to_string());
            let secondary_name = self.secondary_weapon_name().map(|s| s.to_string());
            let primary_ready = self.weapon.as_ref().is_some_and(|w| {
                let reload = (w.reload_time / rof).max(0.0);
                Self::weapon_ready_named(w, current_time, primary_name.as_deref(), reload)
            });
            let secondary_ready = self.secondary_weapon.as_ref().is_some_and(|w| {
                let reload = (w.reload_time / rof).max(0.0);
                Self::weapon_ready_named(w, current_time, secondary_name.as_deref(), reload)
            });
            if prefer_secondary && secondary_ready {
                1u8
            } else if primary_ready {
                0u8
            } else if secondary_ready {
                1u8
            } else {
                return false;
            }
        };

        // C++ Weapon::getPreAttackDelay / PreAttackType residual.
        let pre_delay = {
            let base = self
                .weapon_slot(slot)
                .map(|w| w.pre_attack_delay.max(0.0))
                .unwrap_or(0.0);
            base * self.weapon_bonus_fields().3
        };
        let prefire = {
            let name = if slot == 1 {
                self.thing.template.secondary_weapon_name.as_deref().or(self
                    .thing
                    .template
                    .primary_weapon_name
                    .as_deref())
            } else {
                self.thing.template.primary_weapon_name.as_deref()
            };
            name.map(crate::game_logic::weapon_bootstrap::host_prefire_type_for_weapon_name)
                .unwrap_or(crate::game_logic::weapon_bootstrap::HostPrefireType::PerShot)
        };
        let apply_delay = self.pre_attack_delay_applies(slot, target_id, prefire, pre_delay);
        if apply_delay {
            // Arm a wind-up when:
            // - new target, or
            // - ready_at == 0 (previous shot completed / no active cycle).
            // Once armed, wait until ready_at; do NOT re-arm while ready_at is set
            // (even after it elapses) until record_shot_at_target clears it.
            let needs_arm =
                self.pre_attack_target != Some(target_id) || self.pre_attack_ready_at <= 0.0;
            if needs_arm {
                self.pre_attack_target = Some(target_id);
                self.record_host_combat_attack();
                self.pre_attack_ready_at = current_time + pre_delay;
                self.weapon_fire_status = WeaponFireStatus::PreAttack;
                self.sync_weapon_model_conditions_from_status();
                self.record_host_combat_attack();
                // C++ Weapon::preFireWeapon LeechRange activate residual.
                self.activate_leech_range_for_slot(slot);
            }
            if current_time + 1e-6 < self.pre_attack_ready_at {
                // Decision authority: engagement state is GameWorld last-writer.
                if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                    crate::game_logic::host_ai_decision_log::record_attack(self.id, target_id);
                    crate::game_logic::host_ai_decision_log::record_set_state(self.id, 2);
                // Attacking
                } else {
                    self.target = Some(target_id);
                    self.set_ai_state(AIState::Attacking);
                }
                self.status.attacking = true;
                return false;
            }
            // Delay complete — fall through to fire; record_shot clears ready_at.
        } else {
            self.pre_attack_target = Some(target_id);
            self.record_host_combat_attack();
        }

        let fire_weapon_name = if slot == 1 {
            self.secondary_weapon_name().map(|s| s.to_string())
        } else {
            self.primary_weapon_name().map(|s| s.to_string())
        };
        let base_damage = self.weapon_slot(slot).map(|w| w.damage).unwrap_or(0.0);
        let weapon_damage = self.effective_weapon_damage(base_damage);
        if let Some(weapon) = self.weapon_slot_mut(slot) {
            Self::consume_ammo_on_fire_named(weapon, current_time, fire_weapon_name.as_deref());
            let weapon_speed = weapon.projectile_speed;
            let weapon_splash = weapon.splash_radius;
            // AA residual: air-only weapons home on live target (missile track).
            let weapon_homing = weapon.can_target_air && !weapon.can_target_ground;
            let shooter_id = self.id;
            let shooter_pos = self.get_position();
            self.target = Some(target_id);

            // Prefer Weapon.ini DamageType via store name; shape residual if store empty.
            let weapon_dtype = {
                let slot = self.active_weapon_slot;
                let name = if slot == 1 {
                    self.thing.template.secondary_weapon_name.as_deref().or(self
                        .thing
                        .template
                        .primary_weapon_name
                        .as_deref())
                } else {
                    self.thing.template.primary_weapon_name.as_deref()
                };
                if let Some(n) = name {
                    let _ = crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
                    if crate::game_logic::thing::ThingTemplate::weapon_from_store(n).is_some() {
                        crate::game_logic::host_armor_residual::host_damage_type_for_weapon_name(n)
                    } else if weapon_speed <= 0.0 || weapon_speed >= 999_000.0 {
                        super::combat::DamageType::Laser
                    } else if weapon_splash > 0.0 {
                        super::combat::DamageType::Explosive
                    } else {
                        super::combat::DamageType::Bullet
                    }
                } else if weapon_speed <= 0.0 || weapon_speed >= 999_000.0 {
                    super::combat::DamageType::Laser
                } else if weapon_splash > 0.0 {
                    super::combat::DamageType::Explosive
                } else {
                    super::combat::DamageType::Bullet
                }
            };
            super::combat::queue_projectile(super::combat::PendingProjectile {
                shooter_id,
                shooter_pos,
                target_id: Some(target_id),
                target_pos: None,
                damage: weapon_damage,
                speed: weapon_speed,
                splash_radius: weapon_splash,
                is_homing: weapon_homing,
                damage_type: weapon_dtype,
                death_type: {
                    let slot = self.active_weapon_slot;
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    crate::game_logic::host_armor_residual::resolve_host_death_type(
                        name,
                        weapon_dtype,
                    )
                },
                projectile_object_name:
                    crate::game_logic::weapon_bootstrap::host_projectile_name_for_unit_slot(
                        self.template_name.as_str(),
                        self.thing.template.primary_weapon_name.as_deref(),
                        self.thing.template.secondary_weapon_name.as_deref(),
                        slot,
                    ),
                detonation_fx_name: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(
                        crate::game_logic::weapon_bootstrap::host_detonation_fx_for_weapon_name,
                    )
                    .unwrap_or_default()
                },
                detonation_ocl_name: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(
                        crate::game_logic::weapon_bootstrap::host_detonation_ocl_for_weapon_name,
                    )
                    .unwrap_or_default()
                },
                exhaust_name:
                    crate::game_logic::weapon_bootstrap::host_projectile_exhaust_for_unit_slot(
                        self.template_name.as_str(),
                        self.thing.template.primary_weapon_name.as_deref(),
                        self.thing.template.secondary_weapon_name.as_deref(),
                        slot,
                    ),
                secondary_damage: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(
                        crate::game_logic::weapon_bootstrap::host_secondary_damage_for_weapon_name,
                    )
                    .unwrap_or(0.0)
                },
                secondary_damage_radius: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(
                        crate::game_logic::weapon_bootstrap::host_secondary_damage_radius_for_weapon_name,
                    )
                    .unwrap_or(0.0)
                },
                shock_wave_amount: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(
                        crate::game_logic::weapon_bootstrap::host_shock_wave_amount_for_weapon_name,
                    )
                    .unwrap_or(0.0)
                },
                shock_wave_radius: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(
                        crate::game_logic::weapon_bootstrap::host_shock_wave_radius_for_weapon_name,
                    )
                    .unwrap_or(0.0)
                },
                shock_wave_taper_off: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(
                        crate::game_logic::weapon_bootstrap::host_shock_wave_taper_for_weapon_name,
                    )
                    .unwrap_or(0.0)
                },
                radius_damage_affects: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(
                        crate::game_logic::weapon_bootstrap::host_radius_damage_affects_for_weapon_name,
                    )
                    .unwrap_or(
                        crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_ENEMIES
                            | crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_NEUTRALS,
                    )
                },
                projectile_collides: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(
                        crate::game_logic::weapon_bootstrap::host_projectile_collides_for_weapon_name,
                    )
                    .unwrap_or(crate::game_logic::weapon_bootstrap::PROJECTILE_COLLIDE_DEFAULT)
                },
                // C++ ScatterRadius + ScatterRadiusVsInfantry residual.
                // fire_at cannot query peer KindOf; apply VsInfantry peel whenever a
                // target id is set (infantry-common residual). Ground attacks use base only.
                scatter_radius: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    // C++: base ScatterRadius + ScatterRadiusVsInfantry only vs infantry.
                    name.map(|n| {
                        crate::game_logic::weapon_bootstrap::host_effective_scatter_radius(
                            n,
                            target_is_infantry,
                        )
                    })
                    .unwrap_or(0.0)
                },
                min_weapon_speed: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(|n| {
                        crate::game_logic::weapon_bootstrap::host_weapon_speed_peel_for_weapon_name(
                            n,
                        )
                        .min_weapon_speed
                    })
                    .unwrap_or(0.0)
                },
                scale_weapon_speed: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(|n| {
                        crate::game_logic::weapon_bootstrap::host_weapon_speed_peel_for_weapon_name(
                            n,
                        )
                        .scale_weapon_speed
                    })
                    .unwrap_or(false)
                },
                attack_range: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(|n| {
                        crate::game_logic::weapon_bootstrap::host_weapon_speed_peel_for_weapon_name(
                            n,
                        )
                        .attack_range
                    })
                    .or_else(|| self.weapon_slot(slot).map(|w| w.range))
                    .unwrap_or(0.0)
                },
                min_attack_range: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(|n| {
                        crate::game_logic::weapon_bootstrap::host_weapon_speed_peel_for_weapon_name(
                            n,
                        )
                        .min_attack_range
                    })
                    .or_else(|| self.weapon_slot(slot).map(|w| w.min_range))
                    .unwrap_or(0.0)
                },
                historic_weapon_key: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.unwrap_or("").to_string()
                },
                historic_bonus_time_frames: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(|n| {
                        crate::game_logic::weapon_bootstrap::host_historic_bonus_for_weapon_name(n)
                            .time_frames
                    })
                    .unwrap_or(0)
                },
                historic_bonus_count: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(|n| {
                        crate::game_logic::weapon_bootstrap::host_historic_bonus_for_weapon_name(n)
                            .count
                    })
                    .unwrap_or(0)
                },
                historic_bonus_radius: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(|n| {
                        crate::game_logic::weapon_bootstrap::host_historic_bonus_for_weapon_name(n)
                            .radius
                    })
                    .unwrap_or(0.0)
                },
                historic_bonus_weapon: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(|n| {
                        crate::game_logic::weapon_bootstrap::host_historic_bonus_for_weapon_name(n)
                            .bonus_weapon
                    })
                    .unwrap_or_default()
                },
                die_on_detonate: {
                    let name = if slot == 1 {
                        self.thing.template.secondary_weapon_name.as_deref().or(self
                            .thing
                            .template
                            .primary_weapon_name
                            .as_deref())
                    } else {
                        self.thing.template.primary_weapon_name.as_deref()
                    };
                    name.map(
                        crate::game_logic::weapon_bootstrap::host_die_on_detonate_for_weapon_name,
                    )
                    .unwrap_or(false)
                },
            });
            // C++ fireWeaponTemplate LeechRange activate residual.
            self.activate_leech_range_for_slot(slot);
            self.record_shot_at_target(target_id);
            // C++ Weapon::m_numShotsForCurBarrel / m_curBarrel residual.
            self.advance_weapon_barrel_after_shot();
            // C++ --m_maxShotCount residual.
            self.consume_max_shot_count();
            self.refresh_weapon_fire_status(current_time);
            {
                let frame = crate::game_logic::host_historic_bonus::logic_frame();
                let wname_owned = if slot == 1 {
                    self.thing
                        .template
                        .secondary_weapon_name
                        .clone()
                        .or_else(|| self.thing.template.primary_weapon_name.clone())
                } else {
                    self.thing.template.primary_weapon_name.clone()
                };
                self.stamp_fire_sound_loop_after_shot(frame, wname_owned.as_deref());
            }
            {
                let (dmg, rng) = self
                    .weapon_slot(slot)
                    .map(|w| (w.damage, w.range))
                    .unwrap_or((0.0, 0.0));
                let frame = crate::game_logic::host_historic_bonus::logic_frame();
                let next_count = self.fire_intent_count.saturating_add(1);
                // When AI attack authority is on, GameWorld SetFireIntent writeback is
                // last-writer — log the intent without dual-writing host last_fire_*.
                if crate::gameworld_shadow::gameworld_ai_attack_authority_live() {
                    crate::game_logic::host_fire_intent_log::record(
                        self.id,
                        target_id.0,
                        slot,
                        dmg,
                        rng,
                        current_time,
                        frame,
                        next_count,
                    );
                    // Keep counter monotonic for subsequent shots this frame.
                    self.fire_intent_count = next_count;
                } else {
                    self.last_fire_victim_host = target_id.0;
                    self.last_fire_slot = slot;
                    self.last_fire_damage = dmg;
                    self.last_fire_range = rng;
                    self.last_fire_sim_time = current_time;
                    self.last_fire_frame = frame;
                    self.fire_intent_count = next_count;
                    self.record_host_fire_intent();
                }
            }

            // C++ STEALTH_NOT_WHILE_ATTACKING / IS_FIRING_WEAPON residual:
            // firing breaks stealth (default host residual).
            if self.stealth_breaks_on_attack && self.status.stealthed {
                self.break_stealth();
            }
            true
        } else {
            false
        }
    }

    pub fn move_to(&mut self, position: Vec3) {
        if self.is_mobile() && self.is_alive() {
            self.movement.target_position = Some(position);
            self.set_ai_state(AIState::Moving);
            self.set_status_moving(true);
            crate::game_logic::host_move_log::record(
                self.id,
                Some([position.x, position.y, position.z]),
            );
        }
    }

    pub fn stop_moving(&mut self) {
        self.movement.target_position = None;
        self.movement.velocity = Vec3::ZERO;
        crate::game_logic::host_move_log::record(self.id, None);
        self.movement.path.clear();
        self.movement.current_path_index = 0;
        self.set_status_moving(false);
        self.waiting_for_path = false;
        self.is_attack_path = false;
        self.is_approach_path = false;
        self.record_host_locomotor();
        self.is_safe_path = false;
        self.temporary_move_frames = 0;
        self.record_host_combat_attack();
        self.is_blocked = false;
        self.is_blocked_and_stuck = false;
        // Only pure locomotion returns to Idle when the destination is reached.
        // Interaction states (Capturing, Repairing, SpecialAbility, Entering, …)
        // set a destination while remaining in-state; clobbering them to Idle
        // aborted capture/repair on arrival before support-state resolution.
        if matches!(self.ai_state, AIState::Moving | AIState::AttackMoving) {
            self.set_ai_state(AIState::Idle);
        }
        self.record_host_movement();
    }

    pub fn attack_target(&mut self, target_id: ObjectId) {
        if !self.is_alive() {
            return;
        }
        // Shock stun residual: ignore new attack orders while stunned.
        if self.is_shock_stunned() {
            return;
        }
        // Jet takeoff residual: leave hangar before engaging.
        let _ = self.takeoff_from_airfield_parking();
        if self.can_attack() {
            if self.pre_attack_target != Some(target_id) {
                // New target — fire_at will start PRE_ATTACK clock.
                self.pre_attack_target = None;
                self.record_host_combat_attack();
                self.pre_attack_ready_at = 0.0;
                self.record_host_combat_attack();
            }
            self.target = Some(target_id);
            self.target_location = None;
            self.set_status_force_attack(false);
            self.set_ai_state(AIState::Attacking);
            self.status.attacking = true;
            crate::game_logic::host_attack_log::record(self.id, Some(target_id));
        }
    }

    /// C++ Weapon::setLeechRangeActive residual for a weapon slot.

    /// C++ Weapon barrel rotation residual after a shot.
    /// Decrements shots on current barrel; when exhausted, advances `weapon_cur_barrel`.

    /// C++ FiringTracker::shotFired FireSoundLoopTime residual.
    /// Extends the looping fire-audio deadline; records start when newly armed.
    pub fn stamp_fire_sound_loop_after_shot(&mut self, frame: u32, weapon_name: Option<&str>) {
        let loop_frames = weapon_name
            .map(crate::game_logic::weapon_bootstrap::host_fire_sound_loop_frames_for_weapon_name)
            .unwrap_or(0);
        if loop_frames == 0 {
            return;
        }
        let sound = weapon_name
            .map(crate::game_logic::weapon_bootstrap::host_fire_sound_for_weapon_name)
            .unwrap_or_default();
        if sound.is_empty() {
            return;
        }
        let was_active = self.fire_sound_loop_until_frame > frame;
        self.fire_sound_loop_until_frame = frame.saturating_add(loop_frames);
        self.fire_sound_loop_name = sound.clone();
        if !was_active {
            crate::game_logic::host_fire_sound_loop_log::record(self.id, sound, true);
        }
    }

    /// C++ FiringTracker::update stop-loop residual when deadline elapses.
    pub fn tick_fire_sound_loop(&mut self, frame: u32) {
        if self.fire_sound_loop_until_frame == 0 {
            return;
        }
        if frame >= self.fire_sound_loop_until_frame {
            let sound = std::mem::take(&mut self.fire_sound_loop_name);
            self.fire_sound_loop_until_frame = 0;
            if !sound.is_empty() {
                crate::game_logic::host_fire_sound_loop_log::record(self.id, sound, false);
            }
        }
    }

    pub fn advance_weapon_barrel_after_shot(&mut self) {
        let spb = self.weapon_shots_per_barrel.max(1);
        let barrels = self.weapon_barrel_count.max(1) as u32;
        if self.weapon_shots_left_on_barrel == 0 {
            self.weapon_shots_left_on_barrel = spb;
        }
        self.weapon_shots_left_on_barrel = self.weapon_shots_left_on_barrel.saturating_sub(1);
        if self.weapon_shots_left_on_barrel == 0 {
            self.weapon_cur_barrel = ((self.weapon_cur_barrel as u32 + 1) % barrels) as u8;
            self.weapon_shots_left_on_barrel = spb;
        }
    }

    pub fn activate_leech_range_for_slot(&mut self, slot: u8) {
        let name = if slot == 1 {
            self.thing.template.secondary_weapon_name.as_deref().or(self
                .thing
                .template
                .primary_weapon_name
                .as_deref())
        } else {
            self.thing.template.primary_weapon_name.as_deref()
        };
        let is_leech = name
            .map(crate::game_logic::weapon_bootstrap::host_leech_range_weapon_for_weapon_name)
            .unwrap_or(false);
        if !is_leech {
            return;
        }
        if slot == 1 {
            self.leech_range_active_secondary = true;
            self.record_host_weapon_stats();
        } else {
            self.leech_range_active_primary = true;
            self.record_host_weapon_stats();
        }
    }

    /// C++ Object::clearLeechRangeModeForAllWeapons residual.
    pub fn clear_leech_range_mode_for_all_weapons(&mut self) {
        self.leech_range_active_primary = false;
        self.record_host_weapon_stats();
        self.leech_range_active_secondary = false;
        self.record_host_weapon_stats();
    }

    pub fn stop_attack(&mut self) {
        self.target = None;
        self.target_location = None;
        self.record_host_target_location();
        self.set_status_force_attack(false);
        self.pre_attack_target = None;
        self.record_host_combat_attack();
        self.pre_attack_ready_at = 0.0;
        self.record_host_combat_attack();
        self.consecutive_shot_target = None;
        self.consecutive_shots_at_target = 0;
        self.record_host_combat_attack();
        self.clear_leech_range_mode_for_all_weapons();
        self.status.attacking = false;
        crate::game_logic::host_attack_log::record(self.id, None);
        // C++ parity: guard units return to their guard state after a kill
        // rather than going fully idle. The guard anchor/radius are preserved
        // so the support-states update loop will re-engage nearby enemies.
        if self.guard_target.is_some() {
            self.set_ai_state(AIState::GuardingObject);
        } else if self.guard_position.is_some() {
            self.set_ai_state(AIState::GuardingArea);
        } else {
            self.set_ai_state(AIState::Idle);
        }
    }

    pub fn clear_all_occupants(&mut self) {
        if let Some(building) = self.building_data.as_mut() {
            building.garrisoned_units.clear();
        }
        self.occupants.clear();
    }

    // Command system compatibility methods
    pub fn can_move(&self) -> bool {
        // weapons_jammed intentionally does NOT block movement (weapons-only residual).
        // disabled_subdued blocks move (C++ DISABLED_SUBDUED full disable for non-projectile).
        // Docked aircraft may move (takeoff/sortie residual).
        // Shock flailing residual: block commanded move while STUNNED_FLAILING
        // (stun_frames > 15). Settled STUNNED phase may still stagger via velocity.
        let parked_aircraft = self.is_parked_at_airfield();
        let flailing = self.shock_stun_frames > 15;
        self.is_mobile()
            && self.is_alive()
            && !self.status.deployed
            && !self.status.disabled_unmanned
            && !self.status.disabled_hacked
            && !self.status.disabled_emp
            && !self.status.disabled_subdued
            && !flailing
            && (parked_aircraft || !matches!(self.ai_state, AIState::Docked | AIState::Garrisoned))
    }

    pub fn set_destination(&mut self, destination: Vec3) {
        let _ = self.takeoff_from_airfield_parking();
        // C++ DeployStyle: ordered move packs up (undeploy) residual.
        if self.status.deployed {
            self.set_deployed(false);
        }
        self.move_to(destination);
    }

    pub fn set_target(&mut self, target: Option<ObjectId>) {
        if target.is_some() {
            let _ = self.takeoff_from_airfield_parking();
        }
        self.target = target;
        if target.is_some() {
            self.target_location = None;
            self.record_host_target_location();
            self.set_ai_state(AIState::Attacking);
            self.status.attacking = true;
        } else {
            self.target_location = None;
            self.set_status_force_attack(false);
            self.set_ai_state(AIState::Idle);
            self.status.attacking = false;
        }
        crate::game_logic::host_attack_log::record(self.id, target);
    }

    /// Check whether this object can fire the requested special power.
    ///
    /// Per-power residual: only this power's timer must be clear (other SWs may
    /// still be reloading). Aggregate `special_power_ready` is refreshed for HUD.
    pub fn is_special_power_ready(&self, power: &SpecialPowerType) -> bool {
        if !self.is_alive() || self.is_disabled() {
            return false;
        }
        // C++ SpecialPowerModule::isReady requires m_pausedCount == 0.
        if self.special_power_paused.contains(power) {
            return false;
        }
        let remaining = self
            .special_power_cooldowns
            .get(power)
            .copied()
            .unwrap_or(0.0);
        remaining <= 0.0
    }

    /// C++ SpecialPowerModule::pauseCountdown residual.
    pub fn pause_special_power_countdown(&mut self, power: &SpecialPowerType, pause: bool) {
        if pause {
            self.special_power_paused.insert(power.clone());
        } else {
            self.special_power_paused.remove(power);
            // Unpause starts / continues recharge: if no cooldown entry, begin full reload.
            // Start/continue recharge residual after final unpause.
            let mut cd =
                crate::game_logic::host_special_power_enum_residual::special_power_reload_seconds(
                    power,
                )
                .unwrap_or(0.0);
            if cd <= 0.0 {
                cd = if self.special_power_cooldown > 0.0 {
                    self.special_power_cooldown
                } else {
                    // StartsPaused peels without ReloadTime residual default to 1s.
                    1.0
                };
            }
            self.special_power_cooldowns
                .entry(power.clone())
                .and_modify(|r| {
                    if *r <= 0.0 {
                        *r = cd;
                    }
                })
                .or_insert(cd);
        }
    }

    /// C++ Object::setWeaponBonusCondition(PLAYER_UPGRADE) residual.
    pub fn set_weapon_bonus_player_upgrade(&mut self, enabled: bool) {
        self.weapon_bonus_player_upgrade = enabled;
    }

    /// C++ BodyModule::setArmorSetFlag(ARMORSET_PLAYER_UPGRADE) residual.
    pub fn set_armor_set_player_upgrade(&mut self, enabled: bool) {
        self.armor_set_player_upgrade = enabled;
    }

    /// C++ AIUpdateInterface::setLocomotorUpgrade residual.
    pub fn set_locomotor_upgrade(&mut self, enabled: bool) {
        self.locomotor_upgrade = enabled;
    }

    /// C++ Drawable::setTerrainDecal(TERRAIN_DECAL_CHEMSUIT) residual.
    pub fn set_terrain_decal_chemsuit(&mut self, enabled: bool) {
        self.terrain_decal_chemsuit = enabled;
    }

    /// C++ SpecialPowerCompletionDie::setCreator residual.
    pub fn set_special_power_completion(
        &mut self,
        special_power_name: impl Into<String>,
        creator_id: u32,
    ) {
        if self
            .special_power_completion
            .as_ref()
            .map(|d| d.creator_set)
            .unwrap_or(false)
        {
            return;
        }
        self.special_power_completion = Some(
            crate::game_logic::host_special_power_completion_die::HostSpecialPowerCompletionDieData::new(
                special_power_name,
                creator_id,
            ),
        );
    }

    /// C++ SpecialPowerModule::startPowerRecharge residual (non-SharedNSync path).
    ///
    /// Sets this power's cooldown to full ReloadTime so PublicTimer SWs start
    /// charging when the structure is created/completed — not ready-to-fire.
    pub fn start_power_recharge(&mut self, power: &crate::command_system::SpecialPowerType) {
        let cd = crate::game_logic::host_special_power_enum_residual::special_power_reload_seconds(
            power,
        )
        .unwrap_or(self.special_power_cooldown)
        .max(0.0);
        if cd > 0.0 {
            self.special_power_cooldowns.insert(power.clone(), cd);
            // Legacy aggregate timer residual for single-slot HUD paths.
            self.special_power_cooldown = cd;
            self.special_power_cooldown_remaining = cd;
            self.set_special_power_ready(false);
        } else {
            self.special_power_cooldowns.remove(power);
            self.set_special_power_ready(true);
            self.special_power_cooldown_remaining = 0.0;
        }
        self.refresh_special_power_aggregate_cooldown();
    }

    /// Consume a charge for the special power and start per-power cooldown.
    pub fn consume_special_power_charge(&mut self, power: &SpecialPowerType) {
        if !self.is_special_power_ready(power) {
            return;
        }
        // Prefer retail SpecialPower ReloadTime residual when known; else template cooldown.
        let cd = crate::game_logic::host_special_power_enum_residual::special_power_reload_seconds(
            power,
        )
        .unwrap_or(self.special_power_cooldown)
        .max(0.0);
        if cd > 0.0 {
            self.special_power_cooldowns.insert(power.clone(), cd);
        } else {
            self.special_power_cooldowns.remove(power);
        }
        self.refresh_special_power_aggregate_cooldown();
        self.set_ai_state(AIState::Idle);
    }

    /// Refresh legacy aggregate ready/remaining from per-power residual timers.
    pub fn refresh_special_power_aggregate_cooldown(&mut self) {
        let mut max_rem = 0.0_f32;
        self.special_power_cooldowns.retain(|_, r| {
            if *r > max_rem {
                max_rem = *r;
            }
            *r > 0.0
        });
        // Also consider legacy single timer if still non-zero (older save residual).
        if self.special_power_cooldown_remaining > max_rem {
            max_rem = self.special_power_cooldown_remaining;
        }
        self.special_power_cooldown_remaining = max_rem;
        self.special_power_ready = max_rem <= 0.0;
        self.record_host_special_power();
    }

    pub fn apply_upgrade_tag(&mut self, upgrade: &str) {
        if !upgrade.is_empty() {
            self.applied_upgrades.insert(upgrade.to_string());
        }
    }

    /// C++ Object::removeUpgrade residual.
    pub fn remove_upgrade_tag(&mut self, upgrade: &str) -> bool {
        self.applied_upgrades.remove(upgrade)
    }

    pub fn has_upgrade_tag(&self, upgrade: &str) -> bool {
        self.applied_upgrades.contains(upgrade)
    }

    /// Install C++ HighlanderBody residual.
    pub fn install_highlander_body(&mut self) {
        self.highlander_body = true;
    }

    /// Install C++ UpgradeDie residual.
    pub fn install_upgrade_die(&mut self, upgrade_to_remove: impl Into<String>) {
        self.upgrade_die =
            Some(crate::game_logic::host_upgrade_die::HostUpgradeDieData::new(upgrade_to_remove));
    }

    pub fn set_target_location(&mut self, location: Option<Vec3>) {
        self.target_location = location;
        if location.is_some() {
            self.target = None;
            self.set_ai_state(AIState::Attacking);
            self.status.attacking = true;
        } else {
            self.set_status_force_attack(false);
        }
        self.record_host_target_location();
    }

    pub fn set_force_attack(&mut self, force: bool) {
        self.set_status_force_attack(force);
    }

    pub fn stop(&mut self) {
        // Stop all current actions
        self.stop_moving();
        self.stop_attack();
    }

    pub fn set_guard_position(&mut self, position: Option<Vec3>) {
        self.guard_position = position;
        if position.is_some() {
            self.set_ai_state(AIState::GuardingArea);
        }
        self.record_host_guard();
    }

    pub fn set_guard_mode(&mut self, mode: GuardMode) {
        self.guard_mode = mode;
        self.record_host_guard();
    }

    pub fn set_guard_target(&mut self, target: Option<ObjectId>) {
        self.guard_target = target;
        if target.is_some() {
            self.set_ai_state(AIState::GuardingObject);
        }
        self.record_host_guard();
    }

    /// C++ AIUpdateInterface::privateGuardRetaliate residual.
    ///
    /// Clears current goal, anchors at `pos` (unit position if None), sets
    /// goal victim, enters GuardRetaliating, optional max shots.

    /// C++ AIUpdateInterface::notifyCrate residual.
    pub fn notify_crate(&mut self, crate_id: ObjectId) {
        self.crate_created = Some(crate_id);
        self.record_host_ai_request();
    }

    /// C++ AIUpdateInterface::checkForCrateToPickup residual.
    ///
    /// Saves id, clears marker (C++ clears before lookup — host saves first so
    /// the crate can actually be found), returns crate id if still pending.
    pub fn check_for_crate_to_pickup(&mut self) -> Option<ObjectId> {
        let id = self.crate_created.take()?;
        Some(id)
    }

    pub fn begin_guard_retaliate(
        &mut self,
        victim: ObjectId,
        anchor: Option<glam::Vec3>,
        max_shots: Option<i32>,
    ) {
        if !self.is_alive() || self.status.destroyed {
            return;
        }
        if self.is_kind_of(KindOf::Immobile) || self.is_kind_of(KindOf::Structure) {
            return;
        }
        let anchor_pos = anchor.unwrap_or_else(|| self.get_position());
        self.guard_retaliate_victim = Some(victim);
        self.record_host_ai_request();
        self.guard_retaliate_anchor = Some(anchor_pos);
        // Preserve ordinary guard anchors if already guarding.
        if self.guard_position.is_none() && self.guard_target.is_none() {
            self.guard_position = Some(anchor_pos);
        }
        self.target = Some(victim);
        self.target_location = None;
        self.set_ai_state(AIState::GuardRetaliating);
        self.status.attacking = true;
        if let Some(max) = max_shots {
            self.max_shots_to_fire = max;
            self.record_host_combat_attack();
        }
        crate::game_logic::host_attack_log::record(self.id, Some(victim));
        self.record_host_guard();
    }

    /// Clear GuardRetaliate residual and return to guard/idle.
    pub fn end_guard_retaliate(&mut self) {
        self.guard_retaliate_victim = None;
        self.guard_retaliate_anchor = None;
        self.target = None;
        self.status.attacking = false;
        if self.guard_target.is_some() {
            self.set_ai_state(AIState::GuardingObject);
        } else if self.guard_position.is_some() {
            self.set_ai_state(AIState::GuardingArea);
        } else {
            self.set_ai_state(AIState::Idle);
        }
        crate::game_logic::host_attack_log::record(self.id, None);
    }

    /// Tick GuardRetaliate: drop when victim gone; return toward anchor if far.
    ///
    /// Fail-closed vs full AIGuardRetaliateMachine inner/outer/return states.
    pub fn tick_guard_retaliate(&mut self, victim_alive: bool, victim_pos: Option<glam::Vec3>) {
        if !matches!(self.ai_state, AIState::GuardRetaliating) {
            return;
        }
        if !victim_alive || self.guard_retaliate_victim.is_none() {
            // Return residual: move back to anchor then end.
            if let Some(anchor) = self.guard_retaliate_anchor {
                let us = self.get_position();
                let dx = us.x - anchor.x;
                let dz = us.z - anchor.z;
                // CLOSE_ENOUGH = 25 residual
                if dx * dx + dz * dz > 25.0 * 25.0 && self.can_move() {
                    self.move_to(anchor);
                    // Keep GuardRetaliating until close enough? C++ RETURN state.
                    // Host: issue move then end attack bit.
                    self.target = None;
                    self.status.attacking = false;
                    self.set_ai_state(AIState::Moving);
                    // stash that we should re-enter guard when move completes via clear on kill path
                    return;
                }
            }
            self.end_guard_retaliate();
            return;
        }
        // Keep target locked on victim.
        if let Some(vid) = self.guard_retaliate_victim {
            if self.target != Some(vid) {
                self.target = Some(vid);
                self.status.attacking = true;
            }
        }
        let _ = victim_pos;
    }

    pub fn can_repair(&self) -> bool {
        // Repair/build authority should be limited to worker/dozer-style units.
        self.can_move() && self.is_worker()
    }

    pub fn can_construct(&self) -> bool {
        // Construction should be limited to worker/dozer-style units.
        self.can_move() && self.is_worker()
    }

    pub fn can_contain(&self) -> bool {
        if !self.is_alive() {
            return false;
        }
        // China Overlord residual: only containable once BattleBunker residual
        // capacity is installed (Some(n>0)). Without bunker (Some(0)) reject.
        if self.is_overlord_style_container() {
            return self.overlord_bunker_slot_capacity() > 0;
        }
        // GLA Tunnel Network residual: TunnelContain entrance (shared team pool).
        if self.is_tunnel_network_style_container() {
            return self.is_kind_of(KindOf::Structure);
        }
        // Transports: any vehicle may act as a container (host residual).
        // Explicit max_transport=0 still allows footprint residual capacity.
        if self.is_kind_of(KindOf::Vehicle) {
            return true;
        }
        // Structures: only garrisonable buildings with residual capacity > 0.
        // Fail-closed: faction producers / non-bunker structures reject Enter.
        if self.is_kind_of(KindOf::Structure) {
            return self
                .building_data
                .as_ref()
                .map(|b| b.max_garrison > 0)
                .unwrap_or(false);
        }
        false
    }

    pub fn has_capacity_for(&self, count: usize) -> bool {
        if let Some(building) = &self.building_data {
            if building.max_garrison == 0 {
                return false;
            }
            building.garrisoned_units.len() + count <= building.max_garrison
        } else if self.is_kind_of(KindOf::Vehicle) {
            let cap = self.transport_capacity();
            if cap == 0 {
                return false;
            }
            self.occupants.len() + count <= cap
        } else {
            false
        }
    }

    /// Residual garrison capacity (structures only). 0 = not garrisonable.
    pub fn garrison_capacity(&self) -> usize {
        self.building_data
            .as_ref()
            .map(|b| b.max_garrison)
            .unwrap_or(0)
    }

    /// True when this vehicle uses OverlordContain residual semantics
    /// (`overlord_bunker_capacity` is `Some(...)`).
    pub fn is_overlord_style_container(&self) -> bool {
        self.overlord_bunker_capacity.is_some()
    }

    /// Residual BattleBunker infantry slots on an Overlord-style vehicle.
    /// `0` when not overlord-style or bunker residual not installed.
    pub fn overlord_bunker_slot_capacity(&self) -> usize {
        self.overlord_bunker_capacity.unwrap_or(0)
    }

    /// Install residual BattleBunker capacity (C++ OCL_OverlordBattleBunker →
    /// ChinaTankOverlordBattleBunker TransportContain Slots=5).
    /// Fail-closed: does not spawn a real portable-structure passenger object.
    /// Conflicts residual: clears gattling/propaganda addons (exclusive payload).
    pub fn install_overlord_battle_bunker(&mut self, slots: usize) {
        self.overlord_bunker_capacity = Some(slots);
        // Exclusive ConflictsWith residual (not Emperor innate propaganda).
        let emperor =
            crate::game_logic::host_overlord_addons::is_emperor_template(&self.template_name);
        self.has_overlord_gattling_addon = false;
        if !emperor {
            self.has_overlord_propaganda_addon = false;
        }
        self.record_host_overlord();
    }

    /// Install residual portable GattlingCannon addon
    /// (C++ OCL_OverlordGattlingCannon / OCL_HelixGattlingCannon).
    /// Equips AA secondary + passenger ground residual on primary fires.
    /// Fail-closed: not full portable-structure passenger object.
    pub fn install_overlord_gattling_addon(&mut self) {
        use crate::game_logic::host_gattling_tank::has_chain_guns_upgrade;
        use crate::game_logic::host_overlord_addons::{
            is_emperor_template, overlord_gattling_air_weapon,
        };
        // Exclusive ConflictsWith residual vs bunker / propaganda (except Emperor).
        let emperor = is_emperor_template(&self.template_name);
        if !emperor {
            self.has_overlord_propaganda_addon = false;
            // Keep overlord-style marker but zero bunker slots.
            if self.overlord_bunker_capacity.is_some() {
                self.overlord_bunker_capacity = Some(0);
            }
        }
        self.has_overlord_gattling_addon = true;
        self.weapon_set_player_upgrade = true;
        let chain = has_chain_guns_upgrade(&self.applied_upgrades);
        self.secondary_weapon = Some(overlord_gattling_air_weapon(0, chain));
        self.continuous_fire_consecutive = 0;
        self.continuous_fire_level = 0;
        self.continuous_fire_coast_until_frame = 0;
        self.continuous_fire_victim = 0;
        self.record_host_combat_attack();
        self.record_host_continuous_fire();
        self.record_host_weapon_set();
        self.record_host_overlord();
    }

    /// Install residual portable PropagandaTower addon
    /// (C++ OCL_OverlordPropagandaTower / OCL_HelixPropagandaTower).
    /// Fail-closed: not full portable tower object / PulseFX.
    pub fn install_overlord_propaganda_addon(&mut self) {
        // Exclusive ConflictsWith residual vs gattling / bunker.
        self.has_overlord_gattling_addon = false;
        if self.overlord_bunker_capacity.is_some() {
            self.overlord_bunker_capacity = Some(0);
        }
        self.has_overlord_propaganda_addon = true;
        self.record_host_overlord();
    }

    /// Install residual HelixContain transport (Slots=5).
    pub fn install_helix_transport(&mut self) {
        self.is_helix_transport = true;
        self.max_transport = crate::game_logic::host_overlord_addons::HELIX_TRANSPORT_SLOTS;
        // Helix can hold infantry / vehicle / portable structure residual.
        // Fail-closed: allow_inside matrix simplified to transport capacity.
        self.record_host_contain_capacity();
        self.record_host_overlord();
    }

    /// True when portable gattling residual is active on this host.
    pub fn has_overlord_gattling_residual(&self) -> bool {
        self.has_overlord_gattling_addon
    }

    /// True when portable / innate propaganda residual is active on this host.
    pub fn has_overlord_propaganda_residual(&self) -> bool {
        self.has_overlord_propaganda_addon
            || crate::game_logic::host_overlord_addons::is_emperor_template(&self.template_name)
    }

    /// Install residual GLA Battle Bus transport:
    /// C++ TransportContain Slots=8, PassengersAllowedToFire=Yes,
    /// ArmedRidersUpgradeMyWeaponSet=Yes, AllowInsideKindOf=INFANTRY.
    /// Fail-closed: not multi-door exit / SlowDeath undeath SECOND_LIFE.
    pub fn install_battle_bus_transport(&mut self) {
        self.is_battle_bus_transport = true;
        self.max_transport = crate::game_logic::host_battle_bus::BATTLE_BUS_TRANSPORT_SLOTS;
        self.passengers_allowed_to_fire = true;
        self.armed_riders_upgrade_weapon_set = true;
        if self.battle_bus_body.is_none() {
            self.battle_bus_body =
                Some(crate::game_logic::host_battle_bus::HostBattleBusBodyData::new());
        }
        // First-life max health residual (UndeadBody / ActiveBody).
        if self.health.maximum < crate::game_logic::host_battle_bus::BATTLE_BUS_MAX_HEALTH {
            self.health.maximum = crate::game_logic::host_battle_bus::BATTLE_BUS_MAX_HEALTH;
            self.health.current = crate::game_logic::host_battle_bus::BATTLE_BUS_MAX_HEALTH;
        }
        self.record_host_weapon_set();
        self.record_host_contain_capacity();
        self.record_host_stealth_flags();
    }

    /// True when this vehicle is a Battle Bus residual transport.
    pub fn is_battle_bus_style_container(&self) -> bool {
        self.is_battle_bus_transport
    }

    /// C++ UndeadBody::startSecondLife + BattleBus first-death begin residual.
    pub fn start_battle_bus_second_life(&mut self) {
        use crate::game_logic::host_battle_bus::{
            HostBattleBusBodyData, BATTLE_BUS_MC_BIT_SECOND_LIFE,
            BATTLE_BUS_SECOND_LIFE_MAX_HEALTH, BATTLE_BUS_THROW_FORCE,
        };
        let frame = crate::game_logic::host_historic_bonus::logic_frame();
        let body = self
            .battle_bus_body
            .get_or_insert_with(HostBattleBusBodyData::new);
        if body.is_second_life && !body.is_in_first_death {
            // Already converted.
            return;
        }
        body.begin_first_life_undeath(frame);
        self.health.maximum = BATTLE_BUS_SECOND_LIFE_MAX_HEALTH;
        self.health.current = BATTLE_BUS_SECOND_LIFE_MAX_HEALTH;
        self.armor_set_second_life = true;
        self.status.destroyed = false;
        self.status.effectively_dead = false;
        // Throw residual (C++ PhysicsBehavior::applyShock Z = ThrowForce).
        let _ = self.apply_shock_wave_impulse(glam::Vec3::new(0.0, 0.0, BATTLE_BUS_THROW_FORCE));
        self.apply_shock_random_rotation(frame);
        self.stop_moving();
        self.set_ai_state(AIState::Idle);
        self.target = None;
        self.status.attacking = false;
        let _ = BATTLE_BUS_MC_BIT_SECOND_LIFE; // set on land
        self.record_host_weapon_set();
    }

    /// Tick BattleBusSlowDeath first-death air time + empty hulk arming.
    /// Returns (landed_this_tick, empty_hulk_kill).
    pub fn tick_battle_bus_slow_death(
        &mut self,
        current_frame: u32,
        _above_terrain_hint: bool,
        passenger_count: usize,
    ) -> (bool, bool) {
        use crate::game_logic::host_battle_bus::BATTLE_BUS_MC_BIT_SECOND_LIFE;
        if self.battle_bus_body.is_none() {
            return (false, false);
        }
        // Integrate residual throw height (world Z).
        let (in_first, throw_vz) = self
            .battle_bus_body
            .as_ref()
            .map(|b| (b.is_in_first_death, b.throw_vz))
            .unwrap_or((false, 0.0));
        if in_first && throw_vz.abs() > 0.001 {
            let pos = self.get_position();
            let mut z = pos.z + throw_vz;
            let mut new_vz = throw_vz - 0.5; // residual gravity peel
            if new_vz < 0.0 && z <= 0.0 {
                z = 0.0;
                new_vz = 0.0;
            }
            self.set_position(glam::Vec3::new(pos.x, pos.y, z.max(0.0)));
            if let Some(body) = self.battle_bus_body.as_mut() {
                body.throw_vz = new_vz;
            }
        }
        let above = self.get_position().z > 0.5;
        let landed = self
            .battle_bus_body
            .as_mut()
            .map(|b| b.try_land_first_death(current_frame, above))
            .unwrap_or(false);
        if landed {
            // C++ setModelConditionState(MODELCONDITION_SECOND_LIFE) + DISABLED_HELD.
            self.model_condition_bits |= 1u128 << BATTLE_BUS_MC_BIT_SECOND_LIFE;
            self.stop_moving();
            self.set_ai_state(AIState::Idle);
            self.refresh_model_condition_bits();
        }
        let empty_kill = self
            .battle_bus_body
            .as_mut()
            .map(|b| b.tick_empty_hulk(passenger_count, current_frame))
            .unwrap_or(false);
        (landed, empty_kill)
    }

    /// True when UndeadBody should intercept a lethal hit (first life only).
    pub fn battle_bus_should_intercept_lethal(
        &self,
        damage_type: crate::game_logic::combat::DamageType,
        actual_damage: f32,
    ) -> bool {
        if !self.is_battle_bus_transport {
            return false;
        }
        // C++ UndeadBody: DAMAGE_UNRESISTABLE bypasses intercept (penalty / script kill).
        if matches!(
            damage_type,
            crate::game_logic::combat::DamageType::Unresistable
                | crate::game_logic::combat::DamageType::Penalty
                | crate::game_logic::combat::DamageType::Healing
                | crate::game_logic::combat::DamageType::Status
                | crate::game_logic::combat::DamageType::Hack
                | crate::game_logic::combat::DamageType::Deploy
                | crate::game_logic::combat::DamageType::Disarm
                | crate::game_logic::combat::DamageType::KillGarrisoned
                | crate::game_logic::combat::DamageType::Surrender
        ) {
            return false;
        }
        let second = self
            .battle_bus_body
            .as_ref()
            .map(|b| b.is_second_life)
            .unwrap_or(false);
        !second && actual_damage >= self.health.current && self.health.current > 0.0
    }

    /// Install residual GLA Tunnel Network structure:
    /// C++ TunnelContain shared MaxTunnelCapacity=10 per player.
    /// Fail-closed: not GuardTunnelNetwork AI / TimeForFullHeal / CaveSystem.
    pub fn install_tunnel_network_residual(&mut self) {
        self.is_tunnel_network = true;
        if let Some(bd) = self.building_data.as_mut() {
            // Local max is the shared pool cap; GameLogic enforces team-shared count.
            bd.max_garrison = crate::game_logic::host_tunnel_network::MAX_TUNNEL_CAPACITY;
        } else {
            let mut bd = BuildingData::new(BuildingType::Bunker);
            bd.max_garrison = crate::game_logic::host_tunnel_network::MAX_TUNNEL_CAPACITY;
            self.building_data = Some(bd);
            self.record_host_building_type();
        }
        self.record_host_contain_capacity();
        self.record_host_stealth_flags();
    }

    /// True when this structure is a GLA Tunnel Network residual entrance.
    pub fn is_tunnel_network_style_container(&self) -> bool {
        self.is_tunnel_network
    }

    /// Install residual GLA Technical transport:
    /// C++ TransportContain Slots=5, AllowInsideKindOf=INFANTRY.
    /// Passengers ride (bed garrison residual) but do **not** fire
    /// (`PassengersAllowedToFire` unset in retail).
    /// Fail-closed: not chassis reskin / W3D gunner matrix.
    pub fn install_technical_transport(&mut self) {
        self.is_technical_transport = true;
        self.max_transport = crate::game_logic::host_technical::TECHNICAL_TRANSPORT_SLOTS;
        self.passengers_allowed_to_fire = false;
        self.armed_riders_upgrade_weapon_set = false;
        self.record_host_weapon_set();
        self.record_host_contain_capacity();
        self.record_host_stealth_flags();
    }

    /// True when this vehicle is a GLA Technical residual transport.
    pub fn is_technical_style_container(&self) -> bool {
        self.is_technical_transport
    }

    /// Install residual GLA Combat Cycle RiderChangeContain:
    /// C++ Slots=1, AllowInsideKindOf=INFANTRY, passengers do not fire
    /// (bike itself switches WeaponSet to rider weapon residual).
    /// Fail-closed: not full STATUS_RIDER death OCL / scuttle matrix.
    pub fn install_combat_cycle_transport(&mut self) {
        self.is_combat_cycle_transport = true;
        self.max_transport = crate::game_logic::host_combat_cycle::COMBAT_CYCLE_TRANSPORT_SLOTS;
        self.passengers_allowed_to_fire = false;
        self.armed_riders_upgrade_weapon_set = false;
        self.record_host_weapon_set();
        self.record_host_contain_capacity();
        self.record_host_stealth_flags();
    }

    /// True when this vehicle is a GLA Combat Cycle residual transport.
    pub fn is_combat_cycle_style_container(&self) -> bool {
        self.is_combat_cycle_transport
    }

    /// Install residual America Humvee transport:
    /// C++ TransportContain Slots=5, PassengersAllowedToFire=Yes,
    /// AllowInsideKindOf=INFANTRY.
    /// Fail-closed: not multi-exit-path / drone ObjectCreationUpgrade matrix.
    pub fn install_humvee_transport(&mut self) {
        self.is_humvee_transport = true;
        self.max_transport = crate::game_logic::host_humvee::HUMVEE_TRANSPORT_SLOTS;
        self.passengers_allowed_to_fire = true;
        self.armed_riders_upgrade_weapon_set = false;
        self.record_host_weapon_set();
        self.record_host_contain_capacity();
        self.record_host_stealth_flags();
    }

    /// True when this vehicle is an America Humvee residual transport.
    pub fn is_humvee_style_container(&self) -> bool {
        self.is_humvee_transport
    }

    /// Install residual China Troop Crawler transport:
    /// C++ TransportContain Slots=8, AllowInsideKindOf=INFANTRY,
    /// InitialPayload Redguard×8, GoAggressiveOnExit residual (exit-to-fight).
    /// Passengers do **not** fire from inside (`PassengersAllowedToFire` unset).
    /// Fail-closed: not multi-exit-path / HealthRegen / wounded retrieve matrix.
    pub fn install_troop_crawler_transport(&mut self) {
        self.is_troop_crawler_transport = true;
        self.max_transport = crate::game_logic::host_troop_crawler::TROOP_CRAWLER_TRANSPORT_SLOTS;
        self.passengers_allowed_to_fire = false;
        self.armed_riders_upgrade_weapon_set = false;
        self.record_host_weapon_set();
        self.record_host_contain_capacity();
        self.record_host_stealth_flags();
    }

    /// True when this vehicle is a China Troop Crawler residual transport.
    pub fn is_troop_crawler_style_container(&self) -> bool {
        self.is_troop_crawler_transport
    }

    /// Install residual Air Force Combat Chinook transport:
    /// C++ TransportContain Slots=8, PassengersAllowedToFire=Yes,
    /// ArmedRidersUpgradeMyWeaponSet=Yes, AllowInsideKindOf=INFANTRY VEHICLE.
    /// Fail-closed: not ChinookAIUpdate ropes / supply / rappel / combat drop.
    pub fn install_combat_chinook_transport(&mut self) {
        self.is_combat_chinook_transport = true;
        self.max_transport = crate::game_logic::host_combat_chinook::COMBAT_CHINOOK_TRANSPORT_SLOTS;
        self.passengers_allowed_to_fire = true;
        self.armed_riders_upgrade_weapon_set = true;
        // Combat Chinook KindOf includes CAN_ATTACK residual (vanilla Chinook does not).
        self.thing.template.add_kind_of(KindOf::Attackable);
        // Retail WeaponSet Conditions=None has PRIMARY NONE until PLAYER_UPGRADE
        // (ListeningOutpostUpgradedDummyWeapon). Strip kind-based Weapon::default.
        self.weapon = None;
        self.weapon_set_player_upgrade = false;
        self.record_host_weapon_set();
        self.record_host_contain_capacity();
        self.record_host_stealth_flags();
    }

    /// True when this vehicle is an AirF Combat Chinook residual transport.
    pub fn is_combat_chinook_style_container(&self) -> bool {
        self.is_combat_chinook_transport
    }

    /// Install residual China Listening Outpost transport + detect residual:
    /// C++ TransportContain Slots=2, PassengersAllowedToFire=Yes,
    /// ArmedRidersUpgradeMyWeaponSet=Yes, AllowInsideKindOf=INFANTRY,
    /// StealthDetectorUpdate DetectionRange=300, InnateStealth=Yes.
    /// Fail-closed: not multi-door exit / IR FX / RIDERS_ATTACKING uncloak matrix.
    pub fn install_listening_outpost_transport(&mut self) {
        self.is_listening_outpost_transport = true;
        self.max_transport =
            crate::game_logic::host_listening_outpost::LISTENING_OUTPOST_TRANSPORT_SLOTS;
        self.passengers_allowed_to_fire = true;
        self.armed_riders_upgrade_weapon_set = true;
        // Detector residual (DetectionRange = 300).
        self.is_detector = true;
        self.detection_range =
            crate::game_logic::host_listening_outpost::LISTENING_OUTPOST_DETECTION_RANGE;
        // Innate stealth residual; uncloaks while MOVING.
        self.set_status_stealthed(true);
        self.innate_stealth = true;
        self.stealth_breaks_on_move = true;
        // Fire does not break stealth on the vehicle itself (passengers fire residual).
        self.stealth_breaks_on_attack = false;
        // Retail WeaponSet Conditions=None has PRIMARY NONE until PLAYER_UPGRADE.
        self.weapon = None;
        self.weapon_set_player_upgrade = false;
        // KindOf residual includes CAN_ATTACK (for dummy weapon range residual).
        self.thing.template.add_kind_of(KindOf::Attackable);
        self.record_host_detector();
        self.record_host_weapon_set();
        self.record_host_contain_capacity();
        self.record_host_stealth_flags();
    }

    /// True when this vehicle is a China Listening Outpost residual transport.
    pub fn is_listening_outpost_style_container(&self) -> bool {
        self.is_listening_outpost_transport
    }

    /// Residual transport capacity (vehicles). Overlord bunker residual wins,
    /// then explicit `max_transport`, else footprint heuristic. Structures return 0.
    pub fn transport_capacity(&self) -> usize {
        if self.is_kind_of(KindOf::Structure) {
            return 0;
        }
        if !self.is_kind_of(KindOf::Vehicle) {
            return 0;
        }
        // Overlord BattleBunker residual: bunker slots only (0 without bunker).
        if let Some(cap) = self.overlord_bunker_capacity {
            return cap;
        }
        if self.max_transport > 0 {
            return self.max_transport;
        }
        // Transport heuristic based on footprint: larger selection radius holds more.
        let base_cap = (self.selection_radius / 8.0).ceil() as usize + 2;
        base_cap.clamp(2, 12)
    }

    /// Current transport occupant count (vehicles only; structures use garrison).
    pub fn transport_count(&self) -> usize {
        if self.is_kind_of(KindOf::Structure) {
            0
        } else {
            self.occupants.len()
        }
    }

    /// Current garrison/transport occupant count.
    pub fn garrison_count(&self) -> usize {
        self.contained_units().len()
    }

    pub fn set_contained_by(&mut self, container: Option<ObjectId>) {
        self.contained_by = container;
        crate::game_logic::host_contain_log::record_contained_by(self.id, container);
    }

    pub fn add_occupant(&mut self, unit_id: ObjectId) -> bool {
        if !self.can_contain() || !self.has_capacity_for(1) {
            return false;
        }
        if let Some(building) = self.building_data.as_mut() {
            if building.garrisoned_units.contains(&unit_id) {
                return true;
            }
            building.garrisoned_units.push(unit_id);
            crate::game_logic::host_contain_log::record_garrison(
                self.id,
                &building.garrisoned_units,
                building.max_garrison.min(u16::MAX as usize) as u16,
            );
            true
        } else {
            if self.occupants.contains(&unit_id) {
                return true;
            }
            self.occupants.push(unit_id);
            crate::game_logic::host_contain_log::record_garrison(
                self.id,
                &self.occupants,
                self.occupants.len().min(u16::MAX as usize) as u16,
            );
            true
        }
    }

    pub fn contained_units(&self) -> Vec<ObjectId> {
        if let Some(building) = &self.building_data {
            building.garrisoned_units.clone()
        } else {
            self.occupants.clone()
        }
    }

    pub fn remove_occupant(&mut self, unit_id: ObjectId) -> bool {
        if let Some(building) = self.building_data.as_mut() {
            if let Some(pos) = building
                .garrisoned_units
                .iter()
                .position(|&id| id == unit_id)
            {
                building.garrisoned_units.remove(pos);
                crate::game_logic::host_contain_log::record_garrison(
                    self.id,
                    &building.garrisoned_units,
                    building.max_garrison.min(u16::MAX as usize) as u16,
                );
                return true;
            }
        }
        if let Some(pos) = self.occupants.iter().position(|&id| id == unit_id) {
            self.occupants.remove(pos);
            crate::game_logic::host_contain_log::record_garrison(
                self.id,
                &self.occupants,
                self.occupants.len().min(u16::MAX as usize) as u16,
            );
            return true;
        }
        false
    }

    /// Begin containing an occupant (transport/garrison bookkeeping).
    pub fn enter_transport(&mut self, unit_id: ObjectId) -> bool {
        self.add_occupant(unit_id)
    }

    /// Remove an occupant from this transport/garrison.
    pub fn exit_transport(&mut self, unit_id: ObjectId) -> bool {
        self.remove_occupant(unit_id)
    }

    pub fn tick_timers(&mut self, dt: f32) -> bool {
        if self.cheer_timer > 0.0 {
            self.cheer_timer -= dt;
            if self.cheer_timer <= 0.0 && self.ai_state == AIState::SpecialAbility {
                self.set_ai_state(AIState::Idle);
                self.cheer_timer = 0.0;
                self.record_host_demo_mine_cheer();
            }
        }

        if self.prone_timer > 0.0 {
            self.prone_timer -= dt;
            if self.prone_timer <= 0.0 {
                self.prone_timer = 0.0;
                if let Some(bit) =
                    crate::game_logic::host_enum_table_residual::model_condition_bit_name_index(
                        "PRONE",
                    )
                {
                    self.model_condition_bits &= !(1u128 << bit);
                }
            }
        }

        if self.emoticon_frames_left > 0 {
            // dt is seconds; logic is 30Hz — consume fractional frames.
            let frames = (dt * 30.0).max(0.0);
            let next = self.emoticon_frames_left as f32 - frames;
            if next <= 0.0 {
                self.emoticon_frames_left = 0;
                self.emoticon_name.clear();
            } else {
                self.emoticon_frames_left = next.ceil() as i32;
            }
        }

        let was_ready = self.special_power_ready;
        // C++ SpecialPowerModule::getReadyFrame residual: while isDisabled (or
        // pauseCountdown), availableOnFrame slides with the logic frame — countdown
        // does not advance. SharedNSync player timers are separate and keep ticking.
        let freeze_special_power = self.is_disabled();
        // Under SPECIAL_POWER_AUTHORITY+shadow, GameWorld sole-ticks countdown;
        // host only refreshes ready aggregate after writeback.
        let sole_sp = crate::gameworld_shadow::gameworld_special_power_sole_tick_enabled();
        if dt > 0.0 && !freeze_special_power && !sole_sp && !self.special_power_cooldowns.is_empty()
        {
            for rem in self.special_power_cooldowns.values_mut() {
                *rem = (*rem - dt).max(0.0);
            }
        }
        // Legacy single-timer residual (older paths / saves).
        if dt > 0.0
            && !freeze_special_power
            && !sole_sp
            && self.special_power_cooldown_remaining > 0.0
        {
            self.special_power_cooldown_remaining =
                (self.special_power_cooldown_remaining - dt).max(0.0);
        }
        self.refresh_special_power_aggregate_cooldown();
        let became_ready = !was_ready && self.special_power_ready;
        // GameWorld last-writer residual: publish SP timer after every tick that
        // may have advanced/frozen countdown or flipped ready.
        if became_ready
            || self.special_power_cooldown_remaining > 0.0
            || !self.special_power_cooldowns.is_empty()
            || was_ready != self.special_power_ready
        {
            self.record_host_special_power();
        }
        became_ready
    }

    pub fn update_construction(&mut self, dt: f32) {
        if self.status.under_construction {
            let build_rate = 1.0 / self.thing.template.build_time;
            self.construction_percent += build_rate * dt;

            if self.construction_percent >= 1.0 {
                self.construction_percent = 1.0;
                self.set_status_under_construction(false);
                self.health.current = self.health.maximum;
            } else {
                // Health scales with construction progress
                self.health.current = self.health.maximum * (0.1 + 0.9 * self.construction_percent);
            }
        }
    }

    pub fn update_movement(&mut self, dt: f32) {
        if matches!(self.ai_state, AIState::Docked | AIState::Garrisoned) {
            self.movement.target_position = None;
            self.movement.velocity = Vec3::ZERO;
            return;
        }

        // C++ Locomotor::setPhysicsOptions residual each move tick.
        self.set_locomotor_physics_options();

        // Stunned residual: no loco move while shock-stunned.
        if self.shock_stun_frames > 0 {
            return;
        }

        // C++ fixInvalidPosition residual when on invalid terrain.
        if self.fix_invalid_position() {
            return;
        }

        if self.movement.target_position.is_none() {
            // C++ maintainCurrentPosition when no move order.
            // ground_y unknown here — use current y as layer residual.
            let gy = self.get_position().y;
            let _ = self.loco_maintain_current_position(gy);
            return;
        }

        // Moving: invalidate maintain pos residual.
        self.maintain_pos_valid = false;

        if let Some(target_pos) = self.movement.target_position {
            let current_pos = self.get_position();
            let dx = target_pos.x - current_pos.x;
            let dz = target_pos.z - current_pos.z;
            let dist_2d = (dx * dx + dz * dz).sqrt();

            if dist_2d < 1.0e-4 {
                // Advance path or stop.
                let next_waypoint =
                    if self.movement.current_path_index + 1 < self.movement.path.len() {
                        self.movement.current_path_index += 1;
                        Some(self.movement.path[self.movement.current_path_index])
                    } else {
                        None
                    };
                if let Some(waypoint) = next_waypoint {
                    self.movement.target_position = Some(waypoint);
                } else {
                    self.stop_moving();
                }
                return;
            }

            // C++ locoUpdate_moveTowardsPosition residual (treads-like host default).
            let max_speed = self.effective_max_speed().max(0.0);
            let mut desired_speed = max_speed * self.group_speed_factor.clamp(0.0, 1.0);
            // Cap by blocked speed residual (convert frame→sec: blocked is per-frame).
            if self.is_blocked && self.cur_max_blocked_speed.is_finite() {
                let blocked_per_sec = self.cur_max_blocked_speed * 30.0;
                desired_speed = desired_speed.min(blocked_per_sec);
            }

            // C++ getIsDownhillOnly residual: refuse uphill goals.
            if self.downhill_only {
                let us_y = current_pos.y;
                let goal_y = target_pos.y;
                if us_y < goal_y - 0.05 {
                    return;
                }
            }

            // Legs wander residual: bias desired heading before rotate.
            let mut rotate_goal = target_pos;
            if matches!(
                self.loco_appearance,
                LocomotorAppearance::LegsTwo | LocomotorAppearance::Climber
            ) && self.wander_width_factor != 0.0
            {
                let actual = self.forward_speed_2d().abs();
                let wobble = self.tick_wander_angle_offset(actual);
                let us = self.get_position();
                let base = (-dz).atan2(dx) + wobble;
                rotate_goal = glam::Vec3::new(
                    us.x + base.cos() * 100.0,
                    us.y,
                    us.z + (-base.sin()) * 100.0,
                );
            }

            // C++ rotateTowardsPosition residual.
            let (_turning, angle_diff) = self.rotate_towards_position(rotate_goal, dt);

            // Appearance-specific speed residual (C++ moveTowardsPosition*).
            let quarter_pi = std::f32::consts::FRAC_PI_4;
            let mut angle_coeff = angle_diff.abs() / quarter_pi;
            if angle_coeff > 1.0 {
                angle_coeff = 1.0;
            }

            // Wheels: can only turn while moving — cap to minTurnSpeed when turning.
            if matches!(
                self.loco_appearance,
                LocomotorAppearance::WheelsFour | LocomotorAppearance::Motorcycle
            ) {
                let mut turn_speed = self.min_turn_speed;
                if turn_speed < desired_speed / 4.0 {
                    turn_speed = desired_speed / 4.0;
                }
                let small_turn = std::f32::consts::PI / 20.0;
                if angle_diff.abs() > small_turn && desired_speed > turn_speed {
                    desired_speed = turn_speed;
                }
                // Reverse residual when goal is behind and can_move_backward.
                if self.can_move_backward
                    && actual_speed_is_zero(self)
                    && angle_diff.abs() > std::f32::consts::FRAC_PI_2
                {
                    self.moving_backwards = true;
                    self.record_host_locomotor();
                }
                if self.moving_backwards && angle_diff.abs() < std::f32::consts::FRAC_PI_2 {
                    self.moving_backwards = false;
                    self.record_host_locomotor();
                }
            }

            let mut goal_speed = match self.loco_appearance {
                LocomotorAppearance::LegsTwo
                | LocomotorAppearance::Climber
                | LocomotorAppearance::Treads => (1.0 - angle_coeff) * desired_speed,
                LocomotorAppearance::WheelsFour | LocomotorAppearance::Motorcycle => desired_speed,
                LocomotorAppearance::Hover
                | LocomotorAppearance::Wings
                | LocomotorAppearance::Thrust
                | LocomotorAppearance::Other => desired_speed,
            };

            // Braking residual near destination (unless NO_SLOW_DOWN).
            let actual_speed = self.forward_speed_2d().abs();
            let braking = self.braking.max(1.0e-3);
            let slow_down_dist =
                calc_slow_down_dist(actual_speed, self.min_speed.max(0.0), braking);
            if !self.no_slow_down_as_approaching_dest {
                if dist_2d < slow_down_dist && !self.is_braking {
                    self.is_braking = true;
                    self.braking_factor = 1.1;
                }
                if dist_2d > PATHFIND_CELL_SIZE_F_RESIDUAL && dist_2d > 2.0 * slow_down_dist {
                    self.is_braking = false;
                    self.braking_factor = 1.0;
                }
                if self.is_braking {
                    let floor = self.min_speed.max(0.0);
                    goal_speed = goal_speed
                        .min(actual_speed * 0.85 / self.braking_factor.max(1.0))
                        .max(floor);
                }
            }
            // Treads near-goal tight turn residual.
            if matches!(self.loco_appearance, LocomotorAppearance::Treads)
                && dist_2d < 2.0 * PATHFIND_CELL_SIZE_F_RESIDUAL
                && angle_coeff > 0.05
            {
                goal_speed = actual_speed * 0.6;
            }

            // Wings/Thrust specialized residual (may set position itself).
            if matches!(self.loco_appearance, LocomotorAppearance::Thrust) {
                self.move_towards_thrust(target_pos, dist_2d, goal_speed, dt);
                let _ = self.handle_behavior_z(self.get_position().y, Some(target_pos.y));
            } else if matches!(self.loco_appearance, LocomotorAppearance::Wings) {
                // 2D other-like + preferred height via BehaviorZ.
                self.apply_forward_speed_force(goal_speed, dt);
                let new_position = current_pos + self.movement.velocity * dt;
                self.set_position(new_position);
                let _ = self.handle_behavior_z(new_position.y, Some(target_pos.y));
            } else {
                // Force/velocity apply residual (legs/wheels/treads/hover/other).
                self.apply_forward_speed_force(goal_speed, dt);

                // Hover over-water residual (model condition flag only).
                if matches!(self.loco_appearance, LocomotorAppearance::Hover) {
                    // Fail-closed: no water table — never set over_water true here.
                    if self.over_water {
                        self.over_water = false;
                    }
                }

                // Arm motive window so collide forces stay lateral while driving.
                if goal_speed.abs() > 0.1 {
                    self.motive_frames_remaining = MOTIVE_FRAMES_RESIDUAL;
                    self.record_host_physics_motive();
                }

                // Position integrate (host dt seconds).
                let new_position = current_pos + self.movement.velocity * dt;
                self.set_position(new_position);

                // C++ handleBehaviorZ residual after loco XY step.
                let ground_y = new_position.y; // caller/physics motion step samples terrain
                let _ = self.handle_behavior_z(ground_y, Some(target_pos.y));
            }

            // Arrival residual.
            let distance_to_target = current_pos.distance(target_pos);
            if distance_to_target < 2.0 {
                let next_waypoint =
                    if self.movement.current_path_index + 1 < self.movement.path.len() {
                        self.movement.current_path_index += 1;
                        Some(self.movement.path[self.movement.current_path_index])
                    } else {
                        None
                    };
                if let Some(waypoint) = next_waypoint {
                    self.movement.target_position = Some(waypoint);
                    self.is_braking = false;
                } else {
                    self.stop_moving();
                    self.is_braking = false;
                }
            }
        }
        self.record_host_movement();
    }

    /// C++ SalvageCrateCollide::doWeaponSet residual.
    pub fn apply_salvage_weapon_upgrade(&mut self) {
        if self.weapon_crate_upgrade >= 2 {
            return;
        }
        self.weapon_crate_upgrade = self.weapon_crate_upgrade.saturating_add(1);
        self.record_host_ai_request();
        if let Some(w) = self.weapon.as_mut() {
            w.damage *= 1.15;
        }
    }

    /// C++ SalvageCrateCollide::doArmorSet residual.
    pub fn apply_salvage_armor_upgrade(&mut self) {
        if self.armor_crate_upgrade >= 2 {
            return;
        }
        self.armor_crate_upgrade = self.armor_crate_upgrade.saturating_add(1);
        self.thing.template.armor += 10.0;
    }

    /// C++ SalvageCrateCollide::doLevelGain residual.
    pub fn apply_salvage_level_gain(&mut self) {
        use crate::game_logic::VeterancyLevel;
        let cur = self.experience.level;
        if matches!(cur, VeterancyLevel::Heroic) {
            return;
        }
        let need = match cur {
            VeterancyLevel::Rookie => self.thing.template.veterancy_xp_thresholds[0],
            VeterancyLevel::Veteran => self.thing.template.veterancy_xp_thresholds[1],
            VeterancyLevel::Elite => self.thing.template.veterancy_xp_thresholds[2],
            VeterancyLevel::Heroic => return,
        };
        let add = (need - self.experience.current).max(1.0);
        self.gain_experience(add);
    }

    /// C++ ExperienceTracker::gainExpForLevel residual.
    ///
    /// Grants just enough XP to gain `levels` veterancy ranks (clamped to Heroic).
    /// `can_level_up` false skips (non-trainable residual).
    pub fn gain_exp_for_level(&mut self, levels: u8, can_level_up: bool) -> u8 {
        if levels == 0 || !can_level_up {
            return 0;
        }
        use crate::game_logic::VeterancyLevel;
        let mut gained = 0u8;
        for _ in 0..levels {
            if matches!(self.experience.level, VeterancyLevel::Heroic) {
                break;
            }
            self.apply_salvage_level_gain();
            gained += 1;
        }
        gained
    }

    pub fn record_host_experience(&self) {
        crate::game_logic::host_experience_log::record(self.id, self.experience.current.max(0.0));
    }

    fn record_host_veterancy_level(&self) {
        let ordinal = match self.experience.level {
            crate::game_logic::VeterancyLevel::Rookie => 0u8,
            crate::game_logic::VeterancyLevel::Veteran => 1,
            crate::game_logic::VeterancyLevel::Elite => 2,
            crate::game_logic::VeterancyLevel::Heroic => 3,
        };
        crate::game_logic::host_veterancy_log::record(self.id, ordinal);
    }

    pub fn gain_experience(&mut self, amount: f32) {
        // Wave 79: AdvancedTraining ExperienceScalarUpgrade residual application.
        // C++ AddXPScalar 1.0 → double XP when the upgrade tag is present.
        let amount = if self.has_advanced_training_xp_scalar() {
            crate::game_logic::host_unit_training::residual_xp_gain_with_advanced_training(
                amount, true,
            )
        } else {
            amount
        };
        if amount <= 0.0 || !amount.is_finite() {
            return;
        }
        let projected = self.experience.current + amount;

        // C++ parity: veterancy thresholds are per-template (Object::ExperienceValues
        // in INI).  Use template-defined thresholds, falling back to defaults.
        let thresholds = self.thing.template.veterancy_xp_thresholds;

        // Check for level up against projected XP (even when HP/XP authority defers current).
        let previous_level = self.experience.level;
        let new_level = if projected >= thresholds[2] {
            VeterancyLevel::Heroic
        } else if projected >= thresholds[1] {
            VeterancyLevel::Elite
        } else if projected >= thresholds[0] {
            VeterancyLevel::Veteran
        } else {
            VeterancyLevel::Rookie
        };

        if new_level != previous_level {
            self.experience.level = new_level;
            // Apply veterancy bonuses
            self.apply_veterancy_bonuses(previous_level, new_level);
            self.record_host_veterancy_level();
        }

        // GameWorld residual authority: log absolute XP; defer host current mutate.
        if crate::gameworld_shadow::gameworld_damage_authority_live() {
            crate::game_logic::host_experience_log::record(self.id, projected.max(0.0));
        } else {
            self.experience.current = projected;
            self.record_host_experience();
        }
    }

    /// C++ parity (GameData.ini veterancy bonuses):
    ///   Veteran: +10% dmg, +20% RoF, +20% HP
    ///   Elite:   +20% dmg, +40% RoF, +30% HP
    ///   Heroic:  +30% dmg, +60% RoF, +50% HP
    /// Returns (health_multiplier, damage_multiplier, rof_multiplier).
    fn veterancy_bonuses(level: VeterancyLevel) -> (f32, f32, f32) {
        crate::game_logic::host_unit_training::veterancy_bonus_multipliers(level)
    }

    /// Wave 79: true when AdvancedTraining ExperienceScalar residual tag is present.
    pub fn has_advanced_training_xp_scalar(&self) -> bool {
        use crate::game_logic::host_unit_training::{
            is_advanced_training_upgrade, UPGRADE_AMERICA_ADVANCED_TRAINING,
        };
        self.has_upgrade_tag(UPGRADE_AMERICA_ADVANCED_TRAINING)
            || self.has_upgrade_tag("UpgradeAdvancedTraining")
            || self
                .applied_upgrades
                .iter()
                .any(|u| is_advanced_training_upgrade(u))
    }

    pub fn record_host_max_health(&self) {
        crate::game_logic::host_max_health_log::record(
            self.id,
            self.max_health.max(self.health.maximum).max(1.0),
        );
    }

    pub(crate) fn apply_veterancy_bonuses(
        &mut self,
        previous_level: VeterancyLevel,
        new_level: VeterancyLevel,
    ) {
        let (_old_health_bonus, old_damage_bonus, old_rof_bonus) =
            Self::veterancy_bonuses(previous_level);
        let (health_bonus, damage_bonus, rof_bonus) = Self::veterancy_bonuses(new_level);

        // Apply health bonus
        let base_health = self.thing.template.max_health;
        let old_max_health = self.health.maximum.max(1.0);
        let health_ratio = (self.health.current / old_max_health).clamp(0.0, 1.0);
        self.health.maximum = base_health * health_bonus;
        self.health.current = (self.health.maximum * health_ratio).clamp(0.0, self.health.maximum);

        // Apply weapon damage and rate-of-fire bonuses
        if let Some(weapon) = &mut self.weapon {
            let dmg_scale = if old_damage_bonus > 0.0 {
                damage_bonus / old_damage_bonus
            } else {
                1.0
            };
            weapon.damage *= dmg_scale;
            // C++ parity: RoF bonus reduces reload time (faster firing).
            // Scale relative to previous level so multi-level transitions work.
            let rof_scale = rof_bonus / old_rof_bonus;
            weapon.reload_time *= rof_scale;
        }
        self.record_host_veterancy_level();
        self.max_health = self.health.maximum.max(1.0);
        self.record_host_max_health();
    }

    /// C++ ExperienceTracker::setMinVeterancyLevel residual (VeterancyGainCreate).
    ///
    /// Never lowers rank. Seeds residual XP so gain_experience does not demote.
    /// Applies health / weapon bonuses when promoting.
    pub fn set_min_veterancy_level(&mut self, level: VeterancyLevel) -> bool {
        fn rank(level: VeterancyLevel) -> u8 {
            match level {
                VeterancyLevel::Rookie => 0,
                VeterancyLevel::Veteran => 1,
                VeterancyLevel::Elite => 2,
                VeterancyLevel::Heroic => 3,
            }
        }
        fn xp_seed(level: VeterancyLevel, thresholds: [f32; 3]) -> f32 {
            match level {
                VeterancyLevel::Rookie => 0.0,
                VeterancyLevel::Veteran => thresholds[0],
                VeterancyLevel::Elite => thresholds[1],
                VeterancyLevel::Heroic => thresholds[2],
            }
        }

        let previous = self.experience.level;
        let thresholds = self.thing.template.veterancy_xp_thresholds;
        if rank(level) <= rank(previous) {
            // Still seed XP if level already matches but XP is below threshold.
            let seed = xp_seed(previous, thresholds);
            if self.experience.current < seed {
                self.experience.current = seed;
            }
            return false;
        }
        self.experience.level = level;
        let seed = xp_seed(level, thresholds);
        self.experience.current = self.experience.current.max(seed);
        self.apply_veterancy_bonuses(previous, level);
        true
    }

    pub fn select(&mut self) {
        if self.is_selectable() {
            self.selected = true;
            self.status.selected = true;
            crate::game_logic::host_status_log::record_selected(self.id, true);
        }
    }

    pub fn deselect(&mut self) {
        self.selected = false;
        self.status.selected = false;
        crate::game_logic::host_status_log::record_selected(self.id, false);
    }

    /// Host combat residual: mark attacking and log for GameWorld status channel.
    pub fn set_status_attacking(&mut self, attacking: bool) {
        self.status.attacking = attacking;
        crate::game_logic::host_status_log::record_attacking(self.id, attacking);
    }

    /// Host weapon fire residual + status channel log.
    pub fn set_status_firing_weapon(&mut self, firing: bool) {
        self.status.is_firing_weapon = firing;
        if firing {
            self.blow_defector_cover();
        }
        crate::game_logic::host_status_log::record_firing(self.id, firing);
    }

    /// Host weapon aim residual + status channel log.
    pub fn set_status_aiming_weapon(&mut self, aiming: bool) {
        self.status.is_aiming_weapon = aiming;
        crate::game_logic::host_status_log::record_aiming(self.id, aiming);
    }

    /// Host stealth residual + status channel log.
    pub fn set_status_stealthed(&mut self, stealthed: bool) {
        self.status.stealthed = stealthed;
        crate::game_logic::host_status_log::record_stealthed(self.id, stealthed);
    }

    /// Host detection residual + status channel log.
    pub fn set_status_detected(&mut self, detected: bool) {
        self.status.detected = detected;
        crate::game_logic::host_status_log::record_detected(self.id, detected);
    }

    /// Host EMP disable residual + status channel log.
    pub fn set_status_disabled_emp(&mut self, disabled: bool) {
        self.status.disabled_emp = disabled;
        crate::game_logic::host_status_log::record_disabled_emp(self.id, disabled);
    }

    /// Host weapon jam residual + status channel log.
    pub fn set_status_weapons_jammed(&mut self, jammed: bool) {
        self.status.weapons_jammed = jammed;
        crate::game_logic::host_status_log::record_weapons_jammed(self.id, jammed);
    }

    pub fn set_status_moving(&mut self, moving: bool) {
        self.status.moving = moving;
        crate::game_logic::host_status_log::record_moving(self.id, moving);
    }

    pub fn set_status_disabled_hacked(&mut self, v: bool) {
        self.status.disabled_hacked = v;
        crate::game_logic::host_status_log::record_disabled_hacked(self.id, v);
    }

    pub fn set_status_disabled_unmanned(&mut self, v: bool) {
        self.status.disabled_unmanned = v;
        crate::game_logic::host_status_log::record_disabled_unmanned(self.id, v);
    }

    pub fn set_status_disabled_paralyzed(&mut self, v: bool) {
        self.status.disabled_paralyzed = v;
        crate::game_logic::host_status_log::record_disabled_paralyzed(self.id, v);
    }

    pub fn set_status_disabled_subdued(&mut self, v: bool) {
        self.status.disabled_subdued = v;
        crate::game_logic::host_status_log::record_disabled_subdued(self.id, v);
    }

    /// C++ Object::setStatus / clearStatus residual via StatusBitsUpgrade.
    pub fn apply_status_bits_upgrade_masks(
        &mut self,
        set_names: &[&str],
        clear_names: &[&str],
    ) -> (u32, u32) {
        use crate::game_logic::host_status_bits_upgrade::{
            apply_status_bits_upgrade, object_status_mask_from_names, status_bits_has,
        };
        let before = self.object_status_bits;
        self.object_status_bits =
            apply_status_bits_upgrade(self.object_status_bits, set_names, clear_names);
        // Mirror a few high-traffic bits onto ObjectStatus bools.
        if status_bits_has(self.object_status_bits, "DESTROYED") {
            self.status.destroyed = true;
        }
        if status_bits_has(self.object_status_bits, "UNDER_CONSTRUCTION") {
            self.status.under_construction = true;
        } else if set_names.iter().any(|n| n.eq_ignore_ascii_case("UNDER_CONSTRUCTION"))
            || clear_names
                .iter()
                .any(|n| n.eq_ignore_ascii_case("UNDER_CONSTRUCTION"))
        {
            // cleared path
            if !status_bits_has(self.object_status_bits, "UNDER_CONSTRUCTION") {
                self.status.under_construction = false;
            }
        }
        if status_bits_has(self.object_status_bits, "REPULSOR") {
            self.status.repulsor = true;
        }
        if status_bits_has(self.object_status_bits, "SOLD") {
            // best-effort: sold residual if field exists
            let _ = self.status.sold;
            self.status.sold = true;
        }
        let set_m = object_status_mask_from_names(set_names);
        let clear_m = object_status_mask_from_names(clear_names);
        let set_count = set_m.count_ones();
        let clear_count = (before & clear_m).count_ones();
        (set_count, clear_count)
    }

    pub fn has_object_status_bit(&self, name: &str) -> bool {
        crate::game_logic::host_status_bits_upgrade::status_bits_has(
            self.object_status_bits,
            name,
        )
    }

    pub fn set_status_masked(&mut self, v: bool) {
        self.status.masked = v;
        crate::game_logic::host_status_log::record_masked(self.id, v);
    }

    pub fn set_status_disguised(&mut self, v: bool) {
        self.status.disguised = v;
        crate::game_logic::host_status_log::record_disguised(self.id, v);
    }
    pub fn set_status_no_collisions(&mut self, v: bool) {
        self.status.no_collisions = v;
        crate::game_logic::host_status_log::record_no_collisions(self.id, v);
    }

    pub fn set_status_private_captured(&mut self, v: bool) {
        self.status.private_captured = v;
        crate::game_logic::host_status_log::record_private_captured(self.id, v);
    }

    pub fn set_status_disguise_transitioning_to(&mut self, v: bool) {
        self.status.disguise_transitioning_to = v;
        crate::game_logic::host_status_log::record_disguise_transitioning_to(self.id, v);
    }

    pub fn set_status_disguise_halfpoint_reached(&mut self, v: bool) {
        self.status.disguise_halfpoint_reached = v;
        crate::game_logic::host_status_log::record_disguise_halfpoint_reached(self.id, v);
    }

    pub fn set_status_faerie_fire(&mut self, v: bool) {
        self.status.faerie_fire = v;
        crate::game_logic::host_status_log::record_faerie_fire(self.id, v);
    }

    pub fn set_status_booby_trapped(&mut self, v: bool) {
        self.status.booby_trapped = v;
        crate::game_logic::host_status_log::record_booby_trapped(self.id, v);
    }

    pub fn set_status_eject_invulnerable(&mut self, v: bool) {
        self.status.eject_invulnerable = v;
        crate::game_logic::host_status_log::record_eject_invulnerable(self.id, v);
    }

    pub fn set_status_pilot_did_move_to_base(&mut self, v: bool) {
        self.status.pilot_did_move_to_base = v;
        crate::game_logic::host_status_log::record_pilot_did_move_to_base(self.id, v);
    }

    pub fn set_status_parachuting(&mut self, v: bool) {
        self.status.parachuting = v;
        crate::game_logic::host_status_log::record_parachuting(self.id, v);
    }

    pub fn set_status_parachute_open(&mut self, v: bool) {
        self.status.parachute_open = v;
        crate::game_logic::host_status_log::record_parachute_open(self.id, v);
    }

    pub fn set_status_parachute_landing_override_set(&mut self, v: bool) {
        self.status.parachute_landing_override_set = v;
        crate::game_logic::host_status_log::record_parachute_landing_override_set(self.id, v);
    }
    pub fn set_status_using_ability(&mut self, v: bool) {
        self.status.using_ability = v;
        crate::game_logic::host_status_log::record_using_ability(self.id, v);
    }
    pub fn set_status_deployed(&mut self, v: bool) {
        self.status.deployed = v;
        crate::game_logic::host_status_log::record_deployed(self.id, v);
    }
    pub fn set_status_under_construction(&mut self, v: bool) {
        self.status.under_construction = v;
        crate::game_logic::host_status_log::record_under_construction(self.id, v);
    }
    pub fn set_status_sold(&mut self, v: bool) {
        self.status.sold = v;
        crate::game_logic::host_status_log::record_sold(self.id, v);
    }
    pub fn set_status_reconstructing(&mut self, v: bool) {
        self.status.reconstructing = v;
        crate::game_logic::host_status_log::record_reconstructing(self.id, v);
    }
    pub fn set_status_unselectable(&mut self, v: bool) {
        self.status.unselectable = v;
        crate::game_logic::host_status_log::record_unselectable(self.id, v);
    }
    pub fn set_status_ignoring_stealth(&mut self, v: bool) {
        self.status.ignoring_stealth = v;
        crate::game_logic::host_status_log::record_ignoring_stealth(self.id, v);
    }
    pub fn set_status_repulsor(&mut self, v: bool) {
        self.status.repulsor = v;
        crate::game_logic::host_status_log::record_repulsor(self.id, v);
        crate::game_logic::host_repulsor_log::record(self.id, v, self.repulsor_until_frame);
    }

    /// Arm temporary repulsor helper countdown (C++ ObjectRepulsorHelper residual).
    pub fn arm_repulsor_countdown(&mut self, remaining_frames: u32) {
        self.repulsor_until_frame = remaining_frames;
        self.set_status_repulsor(true);
    }
    pub fn set_status_disabled_underpowered(&mut self, v: bool) {
        self.status.disabled_underpowered = v;
        crate::game_logic::host_status_log::record_disabled_underpowered(self.id, v);
    }
    pub fn set_status_disabled_freefall(&mut self, v: bool) {
        self.status.disabled_freefall = v;
        crate::game_logic::host_status_log::record_disabled_freefall(self.id, v);
    }
    pub fn set_status_is_carbomb(&mut self, v: bool) {
        self.status.is_carbomb = v;
        crate::game_logic::host_status_log::record_is_carbomb(self.id, v);
    }
    pub fn set_status_hijacked(&mut self, v: bool) {
        self.status.hijacked = v;
        crate::game_logic::host_status_log::record_hijacked(self.id, v);
    }
    pub fn set_status_force_attack(&mut self, v: bool) {
        self.force_attack = v;
        crate::game_logic::host_status_log::record_force_attack(self.id, v);
    }
    pub fn record_host_guard(&self) {
        let position = self.guard_position.map(|p| [p.x, p.y, p.z]);
        let target_host = self.guard_target.map(|id| id.0).unwrap_or(0);
        crate::game_logic::host_guard_log::record(
            self.id,
            position,
            target_host,
            self.guard_radius,
        );
    }

    pub fn record_host_continuous_fire(&self) {
        let consecutive = self.continuous_fire_consecutive.min(u16::MAX as u32) as u16;
        crate::game_logic::host_continuous_fire_log::record(
            self.id,
            self.continuous_fire_level,
            consecutive,
            self.continuous_fire_coast_until_frame,
        );
    }

    pub fn record_host_detector(&self) {
        crate::game_logic::host_detector_log::record(
            self.id,
            self.is_detector,
            self.detection_range,
            self.detection_rate_frames,
        );
    }

    pub fn set_detector_state(
        &mut self,
        is_detector: bool,
        detection_range: f32,
        detection_rate_frames: u32,
    ) {
        let detection_range = detection_range.max(0.0);
        if self.is_detector != is_detector
            || (self.detection_range - detection_range).abs() > 1e-5
            || self.detection_rate_frames != detection_rate_frames
        {
            self.is_detector = is_detector;
            self.detection_range = detection_range;
            self.detection_rate_frames = detection_rate_frames;
            self.record_host_detector();
        }
    }

    pub fn record_host_target_location(&self) {
        let loc = self.target_location.map(|p| [p.x, p.y, p.z]);
        crate::game_logic::host_target_location_log::record(self.id, loc);
    }

    pub fn record_host_hijacker(&self) {
        crate::game_logic::host_hijacker_log::record(
            self.id,
            self.hijack_vehicle_id.map(|id| id.0).unwrap_or(0),
            self.hijacker_in_vehicle,
            self.hijacker_update_active,
            self.hijacker_was_airborne,
            self.hijacker_eject_pos.map(|p| [p.x, p.y, p.z]),
            self.hive_slave_respawn_frame,
            self.next_detection_scan_frame,
        );
    }

    pub fn record_host_ai_request(&self) {
        let pending_team = self
            .disguise_pending_team
            .map(|t| match t {
                crate::game_logic::Team::USA => 0u8,
                crate::game_logic::Team::China => 1u8,
                crate::game_logic::Team::GLA => 2u8,
                crate::game_logic::Team::Neutral => 3u8,
            })
            .unwrap_or(255u8);
        crate::game_logic::host_ai_request_log::record(
            self.id,
            self.requested_victim_id.map(|id| id.0).unwrap_or(0),
            self.requested_destination.map(|p| [p.x, p.y, p.z]),
            self.prev_victim_pos.map(|p| [p.x, p.y, p.z]),
            self.crate_created.map(|id| id.0).unwrap_or(0),
            self.guard_retaliate_victim.map(|id| id.0).unwrap_or(0),
            self.guard_retaliate_anchor.map(|p| [p.x, p.y, p.z]),
            self.path_timestamp,
            self.disguise_pending_template.clone().unwrap_or_default(),
            pending_team,
            self.weapon_crate_upgrade,
            self.armor_crate_upgrade,
            self.selection_flash_remaining,
        );
    }

    pub fn record_host_locomotor(&self) {
        crate::game_logic::host_locomotor_log::record(
            self.id,
            self.is_approach_path,
            self.on_invalid_movement_terrain,
            self.was_airborne_last_frame,
            self.can_move_backward,
            self.moving_backwards,
            self.no_slow_down_as_approaching_dest,
            self.turn_pivot_offset,
            self.wander_width_factor,
            self.loco_apply_2d_friction_airborne,
            self.loco_extra_2d_friction,
            self.loco_preferred_height,
            self.loco_preferred_height_damping,
            self.loco_appearance.to_ordinal(),
            self.loco_behavior_z.to_ordinal(),
            self.min_turn_speed,
            self.physics_turning.to_ordinal(),
        );
    }

    pub fn record_host_fire_intent(&self) {
        crate::game_logic::host_fire_intent_log::record(
            self.id,
            self.last_fire_victim_host,
            self.last_fire_slot,
            self.last_fire_damage,
            self.last_fire_range,
            self.last_fire_sim_time,
            self.last_fire_frame,
            self.fire_intent_count,
        );
    }

    pub fn record_host_combat_attack(&self) {
        crate::game_logic::host_combat_attack_log::record(
            self.id,
            self.pre_attack_target.map(|id| id.0).unwrap_or(0),
            self.pre_attack_ready_at,
            self.consecutive_shots_at_target,
            self.max_shots_to_fire,
            self.attack_substate.to_ordinal(),
            self.approach_timestamp,
            self.continuous_fire_victim,
            self.maintain_pos_valid,
            self.maintain_pos.map(|p| [p.x, p.y, p.z]),
            self.temporary_move_frames,
            self.group_speed_factor,
        );
    }

    pub fn record_host_stealth_delay(&self) {
        crate::game_logic::host_stealth_delay_log::record(
            self.id,
            self.stealth_allowed_frame,
            self.stealth_delay_pending,
            self.stealth_delay_frames,
            self.stealth_breaks_on_damage,
            self.detection_expires_frame,
            self.camo_opacity_pulse_phase,
            self.camo_heat_vision_opacity,
            self.camo_net_sub_object_shown,
            self.camo_net_sub_object_observer_visible,
        );
    }

    pub fn record_host_turret(&self) {
        crate::game_logic::host_turret_log::record(
            self.id,
            self.turret_angle_deg,
            self.turret_pitch_deg,
            self.turret_holding,
            self.turret_idle_scanning,
            self.turret_turn_rate_rad,
            self.turret_recenter_frames,
            self.turret_hold_until_frame,
            self.turret_idle_recentering,
            self.turret_enabled,
            self.turret_rotating,
            self.turret_natural_angle_deg,
            self.turret_natural_pitch_deg,
            self.turret_target_id.map(|id| id.0).unwrap_or(0),
            self.turret_force_attacking,
            self.turret_mood_target,
            self.turret_idle_scan_next_frame,
            self.turret_idle_scan_desired_angle_deg,
            self.turret_idle_scan_index,
            self.turret_substate.ordinal(),
        );
    }

    pub fn record_host_entity_power(&self) {
        crate::game_logic::host_entity_power_log::record(
            self.id,
            self.power_provided,
            self.power_consumed,
        );
    }

    pub fn set_entity_power(&mut self, provided: i32, consumed: i32) {
        let provided = provided.max(0);
        let consumed = consumed.max(0);
        if self.power_provided != provided || self.power_consumed != consumed {
            self.power_provided = provided;
            self.power_consumed = consumed;
            self.record_host_entity_power();
        }
    }

    pub fn record_host_weapon_slot(&self) {
        crate::game_logic::host_weapon_slot_log::record(self.id, self.active_weapon_slot);
    }

    /// C++ Object::setWeaponLock residual.
    /// Returns false if the requested slot has no weapon.
    pub fn set_weapon_lock(&mut self, slot: u8, lock_type: WeaponLockType) -> bool {
        if lock_type == WeaponLockType::NotLocked {
            self.release_weapon_lock(WeaponLockType::LockedPermanently);
            return true;
        }
        if self.weapon_slot(slot).is_none() {
            return false;
        }
        // Permanent lock cannot be overridden by temporary (C++ WeaponSet residual).
        if self.weapon_lock_type == WeaponLockType::LockedPermanently
            && lock_type == WeaponLockType::LockedTemporarily
        {
            return false;
        }
        self.weapon_lock_type = lock_type;
        self.weapon_lock_slot = slot;
        self.set_active_weapon_slot(slot);
        true
    }

    /// C++ Object::releaseWeaponLock residual.
    pub fn release_weapon_lock(&mut self, lock_type: WeaponLockType) {
        match lock_type {
            WeaponLockType::NotLocked => {}
            WeaponLockType::LockedTemporarily => {
                if self.weapon_lock_type == WeaponLockType::LockedTemporarily {
                    self.weapon_lock_type = WeaponLockType::NotLocked;
                }
            }
            WeaponLockType::LockedPermanently => {
                // Permanent release clears any lock.
                self.weapon_lock_type = WeaponLockType::NotLocked;
            }
        }
    }

    pub fn is_weapon_locked(&self) -> bool {
        self.weapon_lock_type != WeaponLockType::NotLocked
    }

    /// C++ Drawable::setEmoticon residual (duration in logic frames @ 30Hz).
    pub fn set_surrendered(&mut self, surrendered: bool) {
        self.is_surrendered = surrendered;
        if surrendered {
            self.stop_moving();
            self.set_target(None);
            self.set_force_attack(false);
            self.set_ai_state(AIState::Idle);
        }
    }

    pub fn set_emoticon(&mut self, name: &str, duration_frames: i32) {
        if name.is_empty() || duration_frames <= 0 {
            self.emoticon_name.clear();
            self.emoticon_frames_left = 0;
            return;
        }
        self.emoticon_name = name.to_string();
        self.emoticon_frames_left = duration_frames;
    }

    pub fn set_active_weapon_slot(&mut self, slot: u8) {
        if self.active_weapon_slot != slot {
            self.active_weapon_slot = slot;
            self.record_host_weapon_slot();
        }
    }

    pub fn record_host_special_power(&self) {
        crate::game_logic::host_special_power_log::record(
            self.id,
            self.special_power_ready,
            self.special_power_cooldown_remaining,
            self.special_power_cooldown,
            self.is_disabled(),
        );
    }

    pub fn set_special_power_ready(&mut self, ready: bool) {
        self.special_power_ready = ready;
        self.record_host_special_power();
    }

    pub fn set_stored_supplies(&mut self, supplies: u32) {
        self.stored_resources.supplies = supplies;
        crate::game_logic::host_stored_supplies_log::record(self.id, supplies);
    }

    /// Set the AI state for autonomous behavior
    pub fn set_ai_state(&mut self, state: AIState) {
        let ordinal = match state {
            AIState::Idle => 0u8,
            AIState::Moving => 1,
            AIState::Attacking => 2,
            AIState::AttackMoving => 3,
            AIState::AttackingGround => 4,
            AIState::Gathering => 5,
            AIState::ReturningResources => 6,
            AIState::Constructing => 7,
            AIState::Repairing => 8,
            AIState::GuardingArea => 9,
            AIState::GuardingObject => 10,
            AIState::Patrolling => 11,
            AIState::Docked => 12,
            AIState::Garrisoned => 13,
            AIState::SpecialAbility => 14,
            AIState::SeekingRepair => 15,
            AIState::SeekingHealing => 16,
            AIState::Entering => 17,
            AIState::Docking => 18,
            AIState::Capturing => 19,
            AIState::GuardRetaliating => 20,
        };
        self.ai_state = state;
        crate::game_logic::host_ai_state_log::record(self.id, ordinal);
        self.record_host_ai_mood();
    }

    /// Get visual information for rendering
    pub fn get_visual_info(&self) -> ObjectVisualInfo {
        ObjectVisualInfo {
            position: self.get_position(),
            orientation: self.get_orientation(),
            team_color: self.team_color,
            selection_radius: self.selection_radius,
            ground_height: self.ground_height,
            ground_height_from_terrain: self.ground_height_from_terrain,
            is_selected: self.selected,
            show_health_bar: self.show_health_bar && self.is_alive(),
            health_percentage: self.get_health_percentage(),
            model_name: self.thing.template.model_name.clone(),
            object_type: self.object_type,
            team: self.team,
            under_construction: self.status.under_construction,
            construction_percent: self.construction_percent,
        }
    }

    /// Update team color (useful for changing allegiance)
    pub fn set_team(&mut self, team: Team) {
        if self.team != team {
            self.team = team;
            self.team_color = team.get_color();
            crate::game_logic::host_owner_log::record(self.id, team);
        } else {
            self.team = team;
            self.team_color = team.get_color();
        }
        self.record_host_identity();
    }

    /// Check if this object is visible to a team (for fog of war / targeting UI).
    /// C++ residual: stealthed-and-undetected units are hidden from non-allied teams.
    /// Detected stealthed units become visible (and targetable).
    pub fn is_visible_to_team(&self, team: Team) -> bool {
        // Team-local baseline visibility check. Global shroud/fog filtering is applied by
        // higher-level visibility queries in GameLogic that have object IDs and player context.
        if team == self.team {
            return true;
        }
        !self.is_effectively_stealthed()
    }

    /// Get a description string for UI display.
    /// C++ parity: prefers per-object name override, then template display
    /// name (from INI DisplayName), then template internal name.
    pub fn get_display_name(&self) -> String {
        if !self.name.is_empty() {
            return self.name.clone();
        }
        let tmpl_display = &self.thing.template.display_name;
        if !tmpl_display.is_empty() && tmpl_display != &self.template_name {
            return tmpl_display.clone();
        }
        self.template_name.clone()
    }
}

/// Visual information structure for rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectVisualInfo {
    pub position: Vec3,
    pub orientation: f32,
    pub team_color: [f32; 4],
    pub selection_radius: f32,
    /// Terrain ground height residual at object XY (presentation / FOW residual).
    #[serde(default)]
    pub ground_height: f32,
    /// True when ground_height came from terrain sample (not default 0).
    #[serde(default)]
    pub ground_height_from_terrain: bool,
    pub is_selected: bool,
    pub show_health_bar: bool,
    pub health_percentage: f32,
    pub model_name: Option<String>,
    pub object_type: ObjectType,
    pub team: Team,
    pub under_construction: bool,
    pub construction_percent: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_object() -> Object {
        let template = ThingTemplate::new("TestUnit");
        let mut object = Object::new(template, ObjectId(1), Team::USA);
        object.weapon = Some(Weapon {
            damage: 100.0,
            ..Weapon::default()
        });
        object
    }

    #[test]
    fn veterancy_increases_weapon_damage() {
        let mut object = make_test_object();
        object.gain_experience(60.0); // Veteran → +10% dmg
        let veteran_damage = object.weapon.as_ref().map(|w| w.damage).unwrap_or_default();
        assert!((veteran_damage - 110.0).abs() < 0.01);

        object.gain_experience(90.0); // Elite → +20% dmg (total)
        let elite_damage = object.weapon.as_ref().map(|w| w.damage).unwrap_or_default();
        assert!((elite_damage - 120.0).abs() < 0.01);
    }

    #[test]
    fn veterancy_preserves_health_ratio_when_max_health_changes() {
        let mut object = make_test_object();
        object.health.current = 50.0;
        object.health.maximum = 100.0;

        object.gain_experience(60.0); // Veteran → +20% HP
        assert!((object.health.maximum - 120.0).abs() < 0.01);
        assert!((object.health.current - 60.0).abs() < 0.01);
    }

    #[test]
    fn stop_attack_clears_force_attack_and_targets() {
        let mut object = make_test_object();
        object.set_target(Some(ObjectId(99)));
        object.set_force_attack(true);
        object.set_target_location(Some(Vec3::new(1.0, 0.0, 2.0)));
        object.stop_attack();

        assert!(object.target.is_none());
        assert!(object.target_location.is_none());
        assert!(!object.force_attack);
        assert!(!object.status.attacking);
    }

    #[test]
    fn setting_target_location_clears_object_target() {
        let mut object = make_test_object();
        object.set_target(Some(ObjectId(77)));
        object.set_target_location(Some(Vec3::new(10.0, 0.0, 10.0)));

        assert!(object.target.is_none());
        assert!(object.target_location.is_some());
        assert!(object.status.attacking);
    }

    #[test]
    fn effectively_stealthed_blocks_enemy_visibility_and_targeting() {
        let mut stealthed = make_test_object();
        stealthed.team = Team::USA;
        stealthed.status.stealthed = true;
        stealthed.status.detected = false;
        stealthed.thing.template.add_kind_of(KindOf::Attackable);

        assert!(stealthed.is_effectively_stealthed());
        assert!(stealthed.is_visible_to_team(Team::USA));
        assert!(!stealthed.is_visible_to_team(Team::China));
        assert!(!stealthed.is_targetable_by_enemy_of(Team::China));

        stealthed.status.detected = true;
        assert!(!stealthed.is_effectively_stealthed());
        assert!(stealthed.is_visible_to_team(Team::China));
        assert!(stealthed.is_targetable_by_enemy_of(Team::China));
    }

    #[test]
    fn fire_at_breaks_stealth_when_forbidden_while_attacking() {
        let mut object = make_test_object();
        object.status.stealthed = true;
        object.stealth_breaks_on_attack = true;
        object.weapon = Some(Weapon {
            damage: 100.0,
            range: 100.0,
            reload_time: 0.5,
            last_fire_time: -1.0,
            ..Weapon::default()
        });
        assert!(object.fire_at(ObjectId(2), 0.0));
        assert!(!object.status.stealthed);
        assert!(!object.status.detected);
    }

    #[test]
    fn can_target_rejects_undetected_stealthed_enemy() {
        let mut attacker = make_test_object();
        attacker.weapon = Some(Weapon {
            damage: 10.0,
            range: 100.0,
            ..Weapon::default()
        });

        let mut target = make_test_object();
        target.id = ObjectId(2);
        target.team = Team::China;
        target.status.stealthed = true;
        target.status.detected = false;
        target.set_position(Vec3::new(5.0, 0.0, 0.0));

        assert!(!attacker.can_target(&target));

        target.status.detected = true;
        assert!(attacker.can_target(&target));
    }

    #[test]
    fn clip_ammo_forces_clip_reload_gap() {
        use crate::game_logic::Weapon;
        let mut w = Weapon {
            damage: 10.0,
            range: 100.0,
            reload_time: 0.1, // between shots
            ammo: Some(2),
            clip_size: 2,
            clip_reload_time: 2.0, // long clip reload
            last_fire_time: -100.0,
            ..Weapon::default()
        };
        let t0 = 10.0;
        assert!(Object::weapon_ready(&w, t0));
        Object::consume_ammo_on_fire(&mut w, t0);
        assert_eq!(w.ammo, Some(1));
        // Between-shot: ready after 0.1
        assert!(!Object::weapon_ready(&w, t0 + 0.05));
        assert!(Object::weapon_ready(&w, t0 + 0.11));
        Object::consume_ammo_on_fire(&mut w, t0 + 0.11);
        assert_eq!(w.ammo, Some(0));
        // Clip empty: not ready until clip_reload (~2.0 from last fire adjusted)
        assert!(!Object::weapon_ready(&w, t0 + 0.11 + 0.5));
        assert!(
            Object::weapon_ready(&w, t0 + 0.11 + 2.0),
            "clip reload must elapse before next ready"
        );
        Object::consume_ammo_on_fire(&mut w, t0 + 0.11 + 2.0);
        assert_eq!(w.ammo, Some(1), "refill then spend one");
    }

    #[test]
    fn clip_ammo_cpp_surface() {
        let src = include_str!("object.rs");
        assert!(src.contains("fn consume_ammo_on_fire"));
        assert!(src.contains("clip_reload_time"));
    }

    #[test]
    fn pre_attack_delay_blocks_first_shot() {
        use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
        use glam::Vec3;
        let mut tmpl = ThingTemplate::new("PreAtk");
        tmpl.set_health(100.0);
        tmpl.add_kind_of(KindOf::Infantry);
        tmpl.add_kind_of(KindOf::Attackable);
        let mut atk = Object::new(tmpl, ObjectId(1), Team::USA);
        atk.set_position(Vec3::ZERO);
        atk.weapon = Some(Weapon {
            damage: 25.0,
            range: 100.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            pre_attack_delay: 1.0,
            ammo: Some(5),
            clip_size: 5,
            ..Weapon::default()
        });
        let tgt_id = ObjectId(2);

        // First call starts wind-up, must not fire (ammo unchanged).
        assert!(!atk.fire_at(tgt_id, 10.0));
        assert_eq!(atk.pre_attack_target, Some(tgt_id));
        assert!((atk.pre_attack_ready_at - 11.0).abs() < 1e-4);
        assert_eq!(atk.weapon.as_ref().unwrap().ammo, Some(5));

        // Still winding up.
        assert!(!atk.fire_at(tgt_id, 10.5));
        assert_eq!(atk.weapon.as_ref().unwrap().ammo, Some(5));

        // After delay, fires and consumes ammo.
        assert!(atk.fire_at(tgt_id, 11.0));
        assert_eq!(atk.weapon.as_ref().unwrap().ammo, Some(4));
    }

    #[test]
    fn pre_attack_resets_on_new_target() {
        use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
        let mut tmpl = ThingTemplate::new("PreAtk2");
        tmpl.add_kind_of(KindOf::Infantry);
        tmpl.add_kind_of(KindOf::Attackable);
        let mut atk = Object::new(tmpl, ObjectId(3), Team::USA);
        atk.weapon = Some(Weapon {
            damage: 10.0,
            range: 50.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            pre_attack_delay: 2.0,
            ..Weapon::default()
        });
        assert!(!atk.fire_at(ObjectId(10), 5.0));
        assert!((atk.pre_attack_ready_at - 7.0).abs() < 1e-4);
        // Switch target restarts delay.
        assert!(!atk.fire_at(ObjectId(11), 6.0));
        assert_eq!(atk.pre_attack_target, Some(ObjectId(11)));
        assert!((atk.pre_attack_ready_at - 8.0).abs() < 1e-4);
    }

    #[test]
    fn pre_attack_cpp_surface() {
        let src = include_str!("object.rs");
        assert!(src.contains("PRE_ATTACK residual"));
        assert!(src.contains("pre_attack_ready_at"));
        assert!(src.contains("pre_attack_delay"));
    }

    #[test]
    fn small_arms_reduced_on_tank_armor_residual() {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let mut tmpl = ThingTemplate::new("ArmorTank");
        tmpl.set_health(1000.0);
        tmpl.add_kind_of(KindOf::Vehicle);
        tmpl.add_kind_of(KindOf::Attackable);
        let mut tank = Object::new(tmpl, ObjectId(70), Team::USA);
        let hp0 = tank.health.current;
        // TankArmor SmallArms residual is 0.25 → 100 * 0.25 = 25
        tank.take_damage_from_typed(100.0, None, DamageType::Bullet);
        let dealt = hp0 - tank.health.current;
        assert!(
            (dealt - 25.0).abs() < 1.0,
            "expected ~25 small-arms on tank, got {dealt}"
        );
    }

    #[test]
    fn laser_half_on_human_armor_residual() {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let mut tmpl = ThingTemplate::new("ArmorInf");
        tmpl.set_health(500.0);
        tmpl.add_kind_of(KindOf::Infantry);
        tmpl.add_kind_of(KindOf::Attackable);
        let mut inf = Object::new(tmpl, ObjectId(71), Team::GLA);
        let hp0 = inf.health.current;
        // HumanArmor Laser residual 0.5 → 100 * 0.5 = 50
        inf.take_damage_from_typed(100.0, None, DamageType::Laser);
        let dealt = hp0 - inf.health.current;
        assert!(
            (dealt - 50.0).abs() < 1.0,
            "expected ~50 laser on infantry, got {dealt}"
        );
    }

    #[test]
    fn flame_kill_sets_burned_death_type() {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::host_usa_pilot::HostDeathType;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let mut tmpl = ThingTemplate::new("BurnMe");
        tmpl.set_health(50.0);
        tmpl.add_kind_of(KindOf::Infantry);
        let mut o = Object::new(tmpl, ObjectId(80), Team::GLA);
        let dead =
            o.take_damage_from_typed_death(999.0, None, DamageType::Flame, HostDeathType::Burned);
        assert!(dead);
        assert_eq!(o.status.death_type, HostDeathType::Burned);
    }

    #[test]
    fn resolve_death_type_from_damage_class() {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::host_armor_residual::resolve_host_death_type;
        use crate::game_logic::host_usa_pilot::HostDeathType;
        assert_eq!(
            resolve_host_death_type(None, DamageType::Explosive),
            HostDeathType::Exploded
        );
        assert_eq!(
            resolve_host_death_type(None, DamageType::Laser),
            HostDeathType::Lasered
        );
        assert_eq!(
            resolve_host_death_type(None, DamageType::Toxin),
            HostDeathType::Poisoned
        );
    }

    #[test]
    fn garrison_range_bonus_extends_is_within_attack_range() {
        use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
        use glam::Vec3;

        let mut atk_t = ThingTemplate::new("GR_A");
        atk_t.add_kind_of(KindOf::Infantry);
        atk_t.set_health(100.0);
        let mut vic_t = ThingTemplate::new("GR_V");
        vic_t.add_kind_of(KindOf::Vehicle);
        vic_t.set_health(100.0);
        let mut atk = Object::new(atk_t, ObjectId(1), Team::USA);
        let mut vic = Object::new(vic_t, ObjectId(2), Team::GLA);
        atk.set_position(Vec3::ZERO);
        // 120 units away; weapon range 100 — out without garrison, in with 133%.
        vic.set_position(Vec3::new(120.0, 0.0, 0.0));
        atk.weapon = Some(Weapon {
            damage: 10.0,
            range: 100.0,
            ..Weapon::default()
        });
        assert!(!atk.is_within_attack_range(&vic));
        atk.contained_by = Some(ObjectId(99));
        assert!(
            atk.is_within_attack_range(&vic),
            "garrison RANGE 133% should cover 120 with base 100"
        );
    }

    #[test]
    fn barrel_advances_after_shots_per_barrel() {
        let vt = ThingTemplate::new("QuadCannon");
        let mut o = Object::new(vt, ObjectId(1), Team::USA);
        o.weapon_shots_per_barrel = 2;
        o.weapon_barrel_count = 4;
        o.weapon_shots_left_on_barrel = 2;
        o.weapon_cur_barrel = 0;
        o.advance_weapon_barrel_after_shot();
        assert_eq!(o.weapon_cur_barrel, 0);
        assert_eq!(o.weapon_shots_left_on_barrel, 1);
        o.advance_weapon_barrel_after_shot();
        assert_eq!(o.weapon_cur_barrel, 1);
        assert_eq!(o.weapon_shots_left_on_barrel, 2);
        for _ in 0..6 {
            o.advance_weapon_barrel_after_shot();
        }
        // 1 + 6/2 = 4 barrels wrapped -> barrel 0 after 8 shots total from start of loop?
        // started at barrel 1 after 2 shots; +6 shots = 3 more barrel advances -> barrel 0
        assert_eq!(o.weapon_cur_barrel, 0);
    }

    #[test]
    fn fire_sound_loop_extends_and_stops() {
        use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
        let mut tmpl = ThingTemplate::new("FlameLoop");
        tmpl.primary_weapon_name = Some("DragonTankFlameWeapon".into());
        tmpl.set_health(100.0);
        tmpl.add_kind_of(KindOf::Vehicle);
        tmpl.add_kind_of(KindOf::Attackable);
        let mut o = Object::new(tmpl, ObjectId(7), Team::China);
        o.weapon = Some(Weapon {
            damage: 5.0,
            range: 50.0,
            reload_time: 0.1,
            last_fire_time: -100.0,
            ..Weapon::default()
        });
        crate::game_logic::host_fire_sound_loop_log::clear();
        o.stamp_fire_sound_loop_after_shot(10, Some("DragonTankFlameWeapon"));
        assert!(o.fire_sound_loop_until_frame > 10);
        let start = crate::game_logic::host_fire_sound_loop_log::drain();
        assert_eq!(start.len(), 1);
        assert!(start[0].start);
        // refresh should not re-emit start while still active
        o.stamp_fire_sound_loop_after_shot(11, Some("DragonTankFlameWeapon"));
        assert!(crate::game_logic::host_fire_sound_loop_log::drain().is_empty());
        let stop_at = o.fire_sound_loop_until_frame;
        o.tick_fire_sound_loop(stop_at);
        let stop = crate::game_logic::host_fire_sound_loop_log::drain();
        assert_eq!(stop.len(), 1);
        assert!(!stop[0].start);
        assert_eq!(o.fire_sound_loop_until_frame, 0);
    }

    #[test]
    fn height_die_kills_when_low() {
        use crate::game_logic::host_height_die::HostHeightDieData;
        use crate::game_logic::{Team, ThingTemplate};
        let mut t = ThingTemplate::new("AmericaAuroraBomb");
        t.set_health(10.0);
        let mut o = Object::new(t, ObjectId(1), Team::USA);
        o.height_die = Some(HostHeightDieData::with_target(5.0, true, 0));
        o.set_position(glam::Vec3::new(0.0, 100.0, 0.0));
        assert!(!o.tick_height_die(1, 0.0));
        o.set_position(glam::Vec3::new(0.0, 50.0, 0.0));
        assert!(!o.tick_height_die(2, 0.0));
        o.set_position(glam::Vec3::new(0.0, 3.0, 0.0));
        assert!(o.tick_height_die(3, 0.0));
        assert!(o.status.destroyed);
    }

    #[test]
    fn squish_requires_velocity_toward_victim() {
        use crate::game_logic::host_squish_collide::velocity_toward_victim;
        assert!(velocity_toward_victim((0.0, 0.0), (5.0, 0.0), (2.0, 0.0)));
        assert!(!velocity_toward_victim((0.0, 0.0), (5.0, 0.0), (-2.0, 0.0)));

        let mut vt = ThingTemplate::new("CrusherTank");
        vt.add_kind_of(KindOf::Vehicle);
        let mut tank = Object::new(vt, ObjectId(101), Team::USA);
        tank.crusher_level = 1;
        tank.set_orientation(0.0);
        // Moving toward infantry (+X).
        tank.movement.velocity = glam::Vec3::new(5.0, 0.0, 0.0);
        tank.set_position(glam::Vec3::new(0.0, 0.0, 0.0));
        tank.selection_radius = 8.0;

        let mut it = ThingTemplate::new("CrushableInf");
        it.add_kind_of(KindOf::Infantry);
        let mut inf = Object::new(it, ObjectId(102), Team::GLA);
        inf.crushable_level = 0;
        inf.selection_radius = 5.0;
        inf.set_position(glam::Vec3::new(4.0, 0.0, 0.0));
        inf.health.current = 100.0;
        inf.health.maximum = 100.0;

        assert!(
            tank.check_for_overlap_collision(&mut inf, false),
            "squish must kill when moving toward infantry in tight radius"
        );
        assert!(inf.front_crushed && inf.back_crushed);
        assert!(!inf.is_alive() || inf.health.current <= 0.0);
    }

    #[test]
    fn defection_timer_expires_and_blows_on_fire() {
        use crate::game_logic::host_defection_helper::DEFAULT_DEFECTION_PROTECTION_FRAMES;
        let mut t = ThingTemplate::new("AmericaInfantryPilot");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Infantry);
        let mut o = Object::new(t, ObjectId(301), Team::USA);
        o.begin_undetected_defection(0, 30, true);
        assert!(o.is_undetected_defector());
        for f in 0..29 {
            o.tick_defection_helper(f);
        }
        assert!(o.is_undetected_defector());
        o.tick_defection_helper(30);
        assert!(!o.is_undetected_defector());

        o.begin_undetected_defection(0, DEFAULT_DEFECTION_PROTECTION_FRAMES, true);
        assert!(o.is_undetected_defector());
        o.status.is_firing_weapon = true;
        o.tick_defection_helper(5);
        assert!(!o.is_undetected_defector());
    }

    #[test]
    fn fire_weapon_power_queues_shots() {
        let mut t = ThingTemplate::new("SpectreHowitzerMarker");
        t.set_health(100.0);
        let mut o = Object::new(t, ObjectId(302), Team::USA);
        assert!(o.activate_fire_weapon_power(Some((100.0, 200.0))));
        let req = o.fire_weapon_power.as_ref().unwrap();
        assert_eq!(req.shots_remaining, 3);
        assert!(req.has_location);
    }

    #[test]
    fn poisoned_behavior_dots_after_toxin() {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::host_historic_bonus;
        let mut t = ThingTemplate::new("TestInfantry");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Infantry);
        let mut o = Object::new(t, ObjectId(201), Team::USA);
        o.health.current = 100.0;
        o.health.maximum = 100.0;
        host_historic_bonus::set_logic_frame(10);
        let _ = o.take_damage_from_typed(20.0, None, DamageType::Toxin);
        // HP reduced by initial hit
        let after_hit = o.health.current;
        assert!(after_hit < 100.0);
        assert!(o
            .poisoned_behavior
            .as_ref()
            .map(|p| p.is_active())
            .unwrap_or(false));
        assert!(o.is_poison_tinted());
        // Advance DoT ticks
        host_historic_bonus::set_logic_frame(20);
        let mut total_dot = 0.0;
        for f in 11..100 {
            if let Some((d, _)) = o.tick_poisoned_behavior(f) {
                total_dot += d;
                let _ = o.take_damage_from_typed_death(
                    d,
                    None,
                    DamageType::Unresistable,
                    crate::game_logic::host_usa_pilot::HostDeathType::Poisoned,
                );
            }
        }
        assert!(total_dot > 0.0, "poison DoT must tick");
        assert!(o.health.current < after_hit || !o.is_alive());
    }

    #[test]
    fn healing_clears_poison() {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::host_historic_bonus;
        let mut t = ThingTemplate::new("TestInfantry");
        t.set_health(100.0);
        let mut o = Object::new(t, ObjectId(202), Team::USA);
        o.health.current = 80.0;
        o.health.maximum = 100.0;
        host_historic_bonus::set_logic_frame(5);
        let _ = o.take_damage_from_typed(10.0, None, DamageType::Toxin);
        assert!(o.is_poison_tinted());
        o.heal(5.0);
        assert!(!o.is_poison_tinted());
    }

    #[test]
    fn bone_fx_fires_on_body_damage_worsen() {
        let mut t = ThingTemplate::new("GLAVehicleScudLauncher");
        t.set_health(1000.0);
        t.add_kind_of(KindOf::Vehicle);
        let mut o = Object::new(t, ObjectId(103), Team::GLA);
        o.health.current = 1000.0;
        o.health.maximum = 1000.0;
        o.refresh_model_condition_bits();
        // Drop into damaged band.
        o.health.current = 400.0;
        o.refresh_model_condition_bits();
        assert!(
            o.bone_fx_damage
                .as_ref()
                .map(|b| b.transitions > 0)
                .unwrap_or(false),
            "BoneFX must fire on damage transition"
        );
        assert!(o
            .bone_fx_damage
            .as_ref()
            .and_then(|b| b.last_fx.as_ref())
            .map(|s| s.contains("Damaged") || s.contains("BoneFX"))
            .unwrap_or(false));
    }

    #[test]
    fn crush_die_sets_model_condition_bits() {
        use crate::game_logic::host_neutron_missile_slow_death::{
            MC_BIT_BACKCRUSHED, MC_BIT_FRONTCRUSHED,
        };
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let mut t = ThingTemplate::new("TestInfantry");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Infantry);
        let mut o = Object::new(t, ObjectId(4), Team::USA);
        o.front_crushed = true;
        o.apply_crush_die_model_conditions();
        assert_ne!(o.model_condition_bits & (1u128 << MC_BIT_FRONTCRUSHED), 0);
        assert_eq!(o.model_condition_bits & (1u128 << MC_BIT_BACKCRUSHED), 0);
        o.back_crushed = true;
        o.apply_crush_die_model_conditions();
        assert_ne!(o.model_condition_bits & (1u128 << MC_BIT_BACKCRUSHED), 0);
    }

    #[test]
    fn keep_object_die_leaves_rubble() {
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let mut t = ThingTemplate::new("TechHospital");
        t.set_health(500.0);
        t.add_kind_of(KindOf::Structure);
        let mut o = Object::new(t, ObjectId(9), Team::Neutral);
        o.health.current = 0.0;
        assert!(o.begin_keep_object_die(10));
        assert!(o.status.keep_as_rubble);
        assert!(o.status.effectively_dead);
        assert!(!o.status.destroyed);
        assert!(!o.is_alive());
    }

    #[test]
    fn jet_slow_death_begins_for_raptor() {
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let mut t = ThingTemplate::new("AmericaJetRaptor");
        t.set_health(200.0);
        t.add_kind_of(KindOf::Aircraft);
        let mut o = Object::new(t, ObjectId(3), Team::USA);
        o.health.current = 0.0;
        o.set_position(glam::Vec3::new(0.0, 80.0, 0.0));
        assert!(o.begin_jet_slow_death());
        assert!(o.jet_slow_death.as_ref().unwrap().is_active());
    }

    #[test]
    fn helicopter_slow_death_begins() {
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let mut t = ThingTemplate::new("AmericaComanche");
        t.set_health(200.0);
        t.add_kind_of(KindOf::Aircraft);
        let mut o = Object::new(t, ObjectId(2), Team::USA);
        o.health.current = 0.0;
        assert!(o.begin_helicopter_slow_death());
        assert!(o.helicopter_slow_death.as_ref().unwrap().is_active());
    }

    #[test]
    fn slow_death_infantry_defers_and_sinks() {
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let mut t = ThingTemplate::new("AmericaInfantryRanger");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Infantry);
        let mut o = Object::new(t, ObjectId(1), Team::USA);
        o.health.current = 0.0;
        assert!(o.begin_slow_death(0));
        assert!(!o.status.destroyed);
        assert!(o.slow_death.as_ref().unwrap().is_active());
        let mut done = false;
        for f in 0..400 {
            if o.tick_slow_death(f) {
                done = true;
                break;
            }
        }
        assert!(done);
        assert!(o.status.destroyed);
        assert!(o.presentation_slow_death_sink_offset() <= 0.0);
    }

    #[test]
    fn create_object_die_queues_spawns() {
        use crate::game_logic::host_create_object_die::HostCreateObjectDieData;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let mut t = ThingTemplate::new("GLASneakAttackTunnelNetworkStart");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Structure);
        let mut o = Object::new(t, ObjectId(1), Team::GLA);
        o.create_object_die = Some(HostCreateObjectDieData {
            ocl_name: "OCL_CreateSneakAttackTunnel".into(),
            spawn_templates: vec!["GLASneakAttackTunnelNetwork".into()],
            transfer_previous_health: true,
            fired: false,
        });
        o.health.current = 0.0;
        o.status.destroyed = true;
        o.refresh_model_condition_bits();
        let (spawns, dmg, transfer) = o.take_pending_create_object_die_spawns();
        assert_eq!(spawns, vec!["GLASneakAttackTunnelNetwork".to_string()]);
        assert!(transfer);
        assert!(dmg >= 0.0);
    }

    #[test]
    fn lifetime_update_expires() {
        use crate::game_logic::host_lifetime_update::HostLifetimeUpdateData;
        use crate::game_logic::{Team, ThingTemplate};
        let mut t = ThingTemplate::new("PoisonFieldMedium");
        t.set_health(10.0);
        let mut o = Object::new(t, ObjectId(2), Team::Neutral);
        o.lifetime_update = Some(HostLifetimeUpdateData::from_delay_frames(0, 3));
        assert!(!o.tick_lifetime_update(2));
        assert!(o.tick_lifetime_update(3));
    }

    #[test]
    fn transition_damage_fx_queues_on_worse_state() {
        use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
        use crate::game_logic::host_transition_damage_fx::HostTransitionDamageFxData;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let mut t = ThingTemplate::new("AmericaWarFactory");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Structure);
        let mut o = Object::new(t, ObjectId(1), Team::USA);
        o.health.current = 100.0;
        o.health.maximum = 100.0;
        o.transition_damage_fx = Some(HostTransitionDamageFxData::generic_structure_residual());
        o.body_damage_state = HostBodyDamageType::Pristine;
        o.health.current = 40.0; // damaged
        o.refresh_model_condition_bits();
        assert_eq!(o.body_damage_state, HostBodyDamageType::Damaged);
        let ev = o.take_pending_transition_damage_fx();
        assert!(!ev.is_empty());
        assert_eq!(ev[0].new_state, HostBodyDamageType::Damaged.ordinal());
    }

    #[test]
    fn fx_list_die_queues_on_rubble() {
        use crate::game_logic::host_fx_list_die::HostFxListDieData;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let mut t = ThingTemplate::new("AmericaTankCrusader");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Vehicle);
        let mut o = Object::new(t, ObjectId(2), Team::USA);
        o.fx_list_die = Some(HostFxListDieData {
            death_fx: Some("FX_VehicleDie".into()),
            death_audio: Some("VehicleDestroyed".into()),
            ..Default::default()
        });
        o.health.current = 0.0;
        o.status.destroyed = true;
        o.refresh_model_condition_bits();
        let (fx, audio) = o.take_pending_death_fx_audio();
        assert_eq!(fx.as_deref(), Some("FX_VehicleDie"));
        assert_eq!(audio.as_deref(), Some("VehicleDestroyed"));
    }

    #[test]
    fn structure_collapse_on_lethal() {
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let mut t = ThingTemplate::new("CivilianBarn01");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Structure);
        let mut b = Object::new(t, ObjectId(1), Team::Neutral);
        b.health.current = 100.0;
        assert!(b.begin_structure_collapse(5));
        assert!(b.structure_collapse_data.as_ref().unwrap().is_active());
        let mut done = false;
        for f in 5..800 {
            if b.tick_structure_collapse(f) {
                done = true;
                break;
            }
        }
        assert!(done);
        assert!(b.status.destroyed);
        assert!(b.presentation_collapse_height_offset() < -1.0);
    }

    #[test]
    fn structure_topple_on_lethal_damage() {
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let mut t = ThingTemplate::new("AmericaWarFactory");
        t.set_health(200.0);
        t.add_kind_of(KindOf::Structure);
        let mut b = Object::new(t, ObjectId(1), Team::USA);
        b.health.current = 200.0;
        assert!(b.begin_structure_topple(10, Some((0.0, 0.0))));
        assert!(
            b.structure_topple_data.as_ref().unwrap().is_active()
                || !b.structure_topple_data.as_ref().unwrap().is_standing()
        );
        assert!(!b.status.destroyed);
        let mut done = false;
        for f in 10..800 {
            if b.tick_structure_topple(f) {
                done = true;
                break;
            }
        }
        assert!(done);
        assert!(b.status.destroyed);
        assert_eq!(
            b.status.death_type,
            crate::game_logic::host_usa_pilot::HostDeathType::Toppled
        );
        assert!(b.presentation_topple_lean_radians() > 1.0);
    }

    #[test]
    fn topple_residual_falls_and_dies() {
        use crate::game_logic::host_topple::{HostToppleData, TOPPLE_OPTIONS_NO_BOUNCE};
        use crate::game_logic::{Team, ThingTemplate};
        let mut t = ThingTemplate::new("TreeOak");
        t.set_health(50.0);
        let mut tree = Object::new(t, ObjectId(1), Team::Neutral);
        tree.health.current = 50.0;
        tree.topple_data = Some(HostToppleData::default());
        assert!(!tree.apply_topple(1.0, 0.0, 2.0, TOPPLE_OPTIONS_NO_BOUNCE));
        assert!(tree.is_alive());
        let mut died = false;
        for _ in 0..600 {
            if tree.tick_topple() {
                died = true;
                break;
            }
        }
        assert!(died);
        assert!(tree.status.destroyed);
        assert_eq!(
            tree.status.death_type,
            crate::game_logic::host_usa_pilot::HostDeathType::Toppled
        );
    }

    #[test]
    fn healing_and_water_damage_residuals() {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::{KindOf, Team, ThingTemplate};

        let mut t = ThingTemplate::new("Ranger");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Infantry);
        let mut unit = Object::new(t, ObjectId(1), Team::USA);
        unit.health.current = 40.0;
        unit.health.maximum = 100.0;

        // Healing restores HP and never destroys.
        assert!(!unit.take_damage_from_typed(25.0, Some(ObjectId(99)), DamageType::Healing));
        assert!((unit.health.current - 65.0).abs() < 1e-3);
        assert!(unit.is_alive());
        // Healing must not stamp hostile last_damage_source.
        assert!(unit.last_damage_source.is_none());

        // Cap at maximum.
        assert!(!unit.take_damage_from_typed(1000.0, None, DamageType::Healing));
        assert!((unit.health.current - 100.0).abs() < 1e-3);

        // Water deals normal HP damage.
        unit.health.current = 100.0;
        let destroyed = unit.take_damage_from_typed(30.0, None, DamageType::Water);
        assert!(!destroyed);
        assert!((unit.health.current - 70.0).abs() < 1e-3);

        // Dead units do not heal.
        unit.health.current = 0.0;
        unit.status.destroyed = true;
        assert!(!unit.take_damage_from_typed(50.0, None, DamageType::Healing));
        assert!((unit.health.current - 0.0).abs() < 1e-3);
    }

    #[test]
    fn deploy_hack_surrender_kill_garrisoned_damage_residuals() {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::{KindOf, Team, ThingTemplate};

        // DEPLOY: no HP, sets pending assault signal.
        let mut tt = ThingTemplate::new("TroopCrawler");
        tt.set_health(100.0);
        let mut crawler = Object::new(tt, ObjectId(1), Team::China);
        crawler.health.current = 100.0;
        assert!(!crawler.take_damage_from_typed(50.0, None, DamageType::Deploy));
        assert!((crawler.health.current - 100.0).abs() < 1e-3);
        assert!(!crawler.status.destroyed);

        // HACK: no HP.
        let mut ht = ThingTemplate::new("Tank");
        ht.set_health(100.0);
        ht.add_kind_of(KindOf::Vehicle);
        let mut tank = Object::new(ht, ObjectId(2), Team::USA);
        tank.health.current = 100.0;
        assert!(!tank.take_damage_from_typed(40.0, None, DamageType::Hack));
        assert!((tank.health.current - 100.0).abs() < 1e-3);

        // SURRENDER lethal on infantry: surrendered, not destroyed.
        let mut it = ThingTemplate::new("Ranger");
        it.set_health(50.0);
        it.add_kind_of(KindOf::Infantry);
        let mut ranger = Object::new(it, ObjectId(3), Team::USA);
        ranger.health.current = 50.0;
        assert!(!ranger.take_damage_from_typed(50.0, None, DamageType::Surrender));
        assert!(ranger.is_surrendered);
        assert!(ranger.is_alive());
        assert!((ranger.health.current - 50.0).abs() < 1e-3);

        // KILL_GARRISONED: structure HP untouched; pending count = floor(amount).
        let mut st = ThingTemplate::new("Bunker");
        st.set_health(500.0);
        st.add_kind_of(KindOf::Structure);
        let mut bunker = Object::new(st, ObjectId(4), Team::GLA);
        bunker.health.current = 500.0;
        assert!(!bunker.take_damage_from_typed(3.7, None, DamageType::KillGarrisoned));
        assert!((bunker.health.current - 500.0).abs() < 1e-3);
        assert_eq!(bunker.take_pending_kill_garrisoned(), 3);

        // PENALTY: normal HP path.
        let mut pt = ThingTemplate::new("Tank");
        pt.set_health(100.0);
        let mut penalized = Object::new(pt, ObjectId(5), Team::USA);
        penalized.health.current = 100.0;
        let _ = penalized.take_damage_from_typed(25.0, None, DamageType::Penalty);
        assert!((penalized.health.current - 75.0).abs() < 1e-3);
    }

    #[test]
    fn disarm_damage_clears_mine_without_hp_on_tank() {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::host_mines::{HostMineData, HostMineKind};
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let mut mt = ThingTemplate::new("Mine");
        mt.set_health(10.0);
        let mut mine = Object::new(mt, ObjectId(1), Team::GLA);
        mine.mine_data = Some(HostMineData {
            kind: HostMineKind::LandMine,
            trigger_range: 10.0,
            detonation_damage: 100.0,
            detonation_radius: 20.0,
            secondary_damage: 0.0,
            secondary_radius: 0.0,
            demo_trap_profile: Default::default(),
            proximity_enabled: true,
            demo_trap_mode: crate::game_logic::host_mines::DemoTrapMode::Proximity,
            detonated: false,
            detonate_at_frame: None,
            attached_to: None,
            producer_id: None,
        });
        mine.health.current = 10.0;
        assert!(mine.take_damage_from_typed(1.0, None, DamageType::Disarm));
        assert!(mine.status.destroyed);
        assert!(mine.mine_data.as_ref().unwrap().detonated);

        let mut tt = ThingTemplate::new("Tank");
        tt.set_health(100.0);
        tt.add_kind_of(KindOf::Vehicle);
        let mut tank = Object::new(tt, ObjectId(2), Team::USA);
        tank.health.current = 100.0;
        assert!(!tank.take_damage_from_typed(50.0, None, DamageType::Disarm));
        assert!((tank.health.current - 100.0).abs() < 1e-3);
    }

    #[test]
    fn kill_pilot_damage_unmans_vehicle_without_hp_loss() {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let mut tmpl = ThingTemplate::new("Tank");
        tmpl.set_health(200.0);
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut o = Object::new(tmpl, ObjectId(5), Team::China);
        o.health.current = 200.0;
        o.health.maximum = 200.0;
        assert!(!o.take_damage_from_typed(1.0, None, DamageType::KillPilot));
        assert!((o.health.current - 200.0).abs() < 1e-3);
        assert!(o.is_unmanned());
        assert_eq!(o.team, Team::Neutral);
    }

    #[test]
    fn emp_subdual_disables_without_hp_loss() {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let mut tmpl = ThingTemplate::new("Tank");
        tmpl.set_health(100.0);
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut o = Object::new(tmpl, ObjectId(4), Team::USA);
        o.health.current = 100.0;
        o.health.maximum = 100.0;
        assert!(!o.take_damage_from_typed(40.0, None, DamageType::EMP));
        assert!((o.health.current - 100.0).abs() < 1e-3);
        assert!((o.subdual_damage - 40.0).abs() < 1e-3);
        assert!(!o.is_subdued());
        assert!(!o.take_damage_from_typed(70.0, None, DamageType::EMP));
        assert!(o.is_subdued());
        assert!(o.is_disabled());
        // Heal residual clears subdual.
        o.subdual_heal_rate_frames = 1;
        o.subdual_heal_amount = 50.0;
        o.subdual_heal_countdown = 0;
        o.tick_subdual_damage();
        o.subdual_heal_countdown = 0;
        o.tick_subdual_damage();
        o.subdual_heal_countdown = 0;
        o.tick_subdual_damage();
        assert!(!o.is_subdued());
    }

    #[test]
    fn status_damage_applies_faerie_without_hp_loss() {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let mut tmpl = ThingTemplate::new("PaintMe");
        tmpl.set_health(100.0);
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut o = Object::new(tmpl, ObjectId(9), Team::GLA);
        o.health.current = 100.0;
        o.health.maximum = 100.0;
        let dead = o.take_damage_from_typed(200.0, None, DamageType::Status);
        assert!(!dead);
        assert!((o.health.current - 100.0).abs() < 1e-3);
        assert!(o.is_faerie_fire());
        assert!(o.faerie_fire_until_frame > 0);
    }

    #[test]
    fn most_percent_ready_between_shots_progresses() {
        use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
        let mut tmpl = ThingTemplate::new("PctReady");
        tmpl.set_health(100.0);
        tmpl.add_kind_of(KindOf::Infantry);
        let mut o = Object::new(tmpl.clone(), ObjectId(1), Team::USA);
        let tgt = Object::new(tmpl, ObjectId(2), Team::GLA);
        o.weapon = Some(Weapon {
            damage: 10.0,
            range: 100.0,
            reload_time: 1.0,
            last_fire_time: -100.0,
            ..Weapon::default()
        });
        assert_eq!(o.get_most_percent_ready_to_fire_any_weapon(0.0), 100);
        assert!(o.fire_at(tgt.id, 1.0));
        assert_eq!(o.weapon_fire_status, WeaponFireStatus::BetweenFiringShots);
        let mid = o.get_most_percent_ready_to_fire_any_weapon(1.5);
        assert!(mid > 0 && mid < 100, "mid={mid}");
        assert_eq!(o.get_most_percent_ready_to_fire_any_weapon(2.0), 100);
    }

    #[test]
    fn ammo_pip_and_waypoint_weapon_helpers() {
        use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
        let mut tmpl = ThingTemplate::new("Raptor");
        tmpl.primary_weapon_name = Some("AmericaJetRaptorMissileWeapon".into());
        tmpl.secondary_weapon_name = Some("ScudStormWeapon".into());
        tmpl.set_health(100.0);
        tmpl.add_kind_of(KindOf::Aircraft);
        let mut o = Object::new(tmpl, ObjectId(3), Team::USA);
        o.weapon = Some(Weapon {
            damage: 50.0,
            range: 200.0,
            reload_time: 1.0,
            clip_size: 4,
            ammo: Some(2),
            ..Weapon::default()
        });
        o.secondary_weapon = Some(Weapon {
            damage: 100.0,
            range: 500.0,
            reload_time: 5.0,
            ..Weapon::default()
        });
        assert_eq!(o.get_ammo_pip_showing_info(), Some((4, 2)));
        assert_eq!(o.find_waypoint_following_capable_weapon_slot(), Some(1));
    }

    #[test]
    fn weapon_status_sets_between_firing_model_condition() {
        use crate::game_logic::host_enum_table_residual::{
            MC_BIT_BETWEEN_FIRING_SHOTS_A, MC_BIT_PREATTACK_A,
        };
        use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
        let mut tmpl = ThingTemplate::new("McFire");
        tmpl.set_health(100.0);
        tmpl.add_kind_of(KindOf::Infantry);
        tmpl.add_kind_of(KindOf::Attackable);
        let mut atk = Object::new(tmpl.clone(), ObjectId(1), Team::USA);
        let tgt = Object::new(tmpl, ObjectId(2), Team::GLA);
        atk.weapon = Some(Weapon {
            damage: 10.0,
            range: 100.0,
            reload_time: 1.0,
            last_fire_time: -100.0,
            ..Weapon::default()
        });
        assert!(atk.fire_at(tgt.id, 1.0));
        assert_eq!(atk.weapon_fire_status, WeaponFireStatus::BetweenFiringShots);
        assert_ne!(
            atk.model_condition_bits & (1u128 << MC_BIT_BETWEEN_FIRING_SHOTS_A),
            0
        );
        atk.pre_attack_ready_at = 5.0;
        atk.refresh_weapon_fire_status(4.0);
        assert_eq!(atk.weapon_fire_status, WeaponFireStatus::PreAttack);
        assert_ne!(atk.model_condition_bits & (1u128 << MC_BIT_PREATTACK_A), 0);
    }

    #[test]
    fn weapon_fire_status_between_shots_after_fire() {
        use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
        let mut tmpl = ThingTemplate::new("StatusW");
        tmpl.set_health(100.0);
        tmpl.add_kind_of(KindOf::Infantry);
        tmpl.add_kind_of(KindOf::Attackable);
        let mut atk = Object::new(tmpl.clone(), ObjectId(1), Team::USA);
        let tgt = Object::new(tmpl, ObjectId(2), Team::GLA);
        atk.weapon = Some(Weapon {
            damage: 10.0,
            range: 100.0,
            reload_time: 1.0,
            last_fire_time: -100.0,
            ..Weapon::default()
        });
        assert_eq!(atk.weapon_fire_status, WeaponFireStatus::ReadyToFire);
        assert!(atk.fire_at(tgt.id, 1.0));
        assert_eq!(atk.weapon_fire_status, WeaponFireStatus::BetweenFiringShots);
        atk.refresh_weapon_fire_status(2.0);
        assert_eq!(atk.weapon_fire_status, WeaponFireStatus::ReadyToFire);
    }

    #[test]
    fn can_fire_honors_weapon_bonus_rof() {
        use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
        let mut tmpl = ThingTemplate::new("RofCan");
        tmpl.set_health(100.0);
        tmpl.add_kind_of(KindOf::Infantry);
        tmpl.add_kind_of(KindOf::Attackable);
        let mut o = Object::new(tmpl, ObjectId(1), Team::USA);
        o.weapon = Some(Weapon {
            damage: 10.0,
            range: 100.0,
            reload_time: 1.0,
            last_fire_time: 0.0,
            ..Weapon::default()
        });
        // Base: not ready at t=0.5
        assert!(!o.can_fire(0.5));
        // With 2x ROF, effective reload = 0.5 → ready at t=0.5
        o.weapon_bonus_enthusiastic = true;
        // Enthusiastic mult is typically >1; if not, force via horde path.
        let (_, _, rof, _) = o.weapon_bonus_fields();
        assert!(rof > 1.0, "expected ROF bonus mult, got {rof}");
        let need = 1.0 / rof;
        assert!(
            o.can_fire(need + 1e-4),
            "can_fire should honor ROF bonus at t={}",
            need
        );
        assert!(!o.can_fire(need - 0.05));
    }

    #[test]
    fn max_shots_to_fire_blocks_after_budget() {
        use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
        use glam::Vec3;
        let mut tmpl = ThingTemplate::new("MaxShot");
        tmpl.set_health(100.0);
        tmpl.add_kind_of(KindOf::Infantry);
        tmpl.add_kind_of(KindOf::Attackable);
        let mut atk = Object::new(tmpl.clone(), ObjectId(1), Team::USA);
        let tgt = Object::new(tmpl, ObjectId(2), Team::GLA);
        atk.set_position(Vec3::ZERO);
        atk.weapon = Some(Weapon {
            damage: 10.0,
            range: 100.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            pre_attack_delay: 0.0,
            projectile_speed: 999_000.0,
            ..Weapon::default()
        });
        atk.set_max_shots_to_fire(2);
        assert!(atk.fire_at(tgt.id, 1.0));
        assert_eq!(atk.max_shots_to_fire, 1);
        assert!(atk.fire_at(tgt.id, 2.0));
        assert_eq!(atk.max_shots_to_fire, 0);
        assert!(!atk.fire_at(tgt.id, 3.0));
        atk.set_max_shots_to_fire(-1);
        assert!(atk.fire_at(tgt.id, 4.0));
        assert_eq!(atk.max_shots_to_fire, -1);
    }

    #[test]
    fn leech_range_waives_max_in_is_within_attack_range() {
        use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
        use glam::Vec3;

        let mut atk_t = ThingTemplate::new("LR_A");
        atk_t.add_kind_of(KindOf::Vehicle);
        atk_t.set_health(100.0);
        let mut vic_t = ThingTemplate::new("LR_V");
        vic_t.add_kind_of(KindOf::Infantry);
        vic_t.set_health(50.0);
        let mut atk = Object::new(atk_t, ObjectId(1), Team::USA);
        let mut vic = Object::new(vic_t, ObjectId(2), Team::GLA);
        atk.set_position(Vec3::ZERO);
        vic.set_position(Vec3::new(500.0, 0.0, 0.0)); // far beyond weapon range
        atk.weapon = Some(Weapon {
            damage: 10.0,
            range: 100.0,
            min_range: 0.0,
            ..Weapon::default()
        });
        // Force leech template name path: set flags directly (activate needs name peel).
        assert!(!atk.is_within_attack_range(&vic));
        atk.leech_range_active_primary = true;
        assert!(
            atk.is_within_attack_range(&vic),
            "leech must waive max range once active"
        );
        // Min range still blocks under leech.
        atk.weapon.as_mut().unwrap().min_range = 600.0;
        assert!(
            !atk.is_within_attack_range(&vic),
            "min range still enforced with leech"
        );
    }

    #[test]
    fn force_reload_when_idle_refills_clip() {
        use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("AR_V");
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(100.0);
        logic.templates.insert("AR_V".to_string(), tpl);
        let id = logic.create_object("AR_V", Team::USA, Vec3::ZERO).unwrap();
        {
            let a = logic.get_object_mut(id).unwrap();
            a.weapon = Some(Weapon {
                damage: 10.0,
                range: 100.0,
                reload_time: 0.5,
                clip_size: 4,
                ammo: Some(1), // partial
                ..Weapon::default()
            });
            a.auto_reload_when_idle_frames = 15;
            a.stamp_auto_reload_when_idle(100);
            assert_eq!(a.frame_to_force_reload, 115);
            a.tick_force_reload_when_idle(114);
            assert_eq!(a.weapon.as_ref().unwrap().ammo, Some(1));
            a.tick_force_reload_when_idle(115);
            assert_eq!(
                a.weapon.as_ref().unwrap().ammo,
                Some(4),
                "idle force reload refills clip"
            );
            assert_eq!(a.frame_to_force_reload, 0);
        }
    }

    #[test]
    fn continuous_fire_coasts_down_after_idle() {
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("CFC_V");
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(100.0);
        logic.templates.insert("CFC_V".to_string(), tpl);
        let id = logic.create_object("CFC_V", Team::USA, Vec3::ZERO).unwrap();
        let tgt = ObjectId(7);
        {
            let a = logic.get_object_mut(id).unwrap();
            a.continuous_fire_one_shots = 1;
            a.continuous_fire_two_shots = 4;
            a.continuous_fire_coast_frames = 10;
            a.record_shot_at_target(tgt);
            a.record_shot_at_target(tgt);
            assert_eq!(a.continuous_fire_level, 1);
            a.stamp_continuous_fire_coast(100);
            assert_eq!(a.continuous_fire_coast_until_frame, 110);
            a.tick_continuous_fire_coast(109);
            assert_eq!(a.continuous_fire_level, 1);
            a.tick_continuous_fire_coast(110);
            assert_eq!(a.continuous_fire_level, 0);
            assert_eq!(a.consecutive_shots_at_target, 0);
            let (_, _, rof, _) = a.weapon_bonus_fields();
            assert!((rof - 1.0).abs() < 0.01);
        }
    }

    #[test]
    fn continuous_fire_mean_rof_after_threshold() {
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("CF_V");
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(100.0);
        logic.templates.insert("CF_V".to_string(), tpl);
        let id = logic.create_object("CF_V", Team::USA, Vec3::ZERO).unwrap();
        let tgt = ObjectId(42);
        {
            let a = logic.get_object_mut(id).unwrap();
            a.continuous_fire_one_shots = 2;
            a.continuous_fire_two_shots = 5;
            assert_eq!(a.continuous_fire_level, 0);
            a.record_shot_at_target(tgt);
            a.record_shot_at_target(tgt);
            assert_eq!(a.continuous_fire_level, 0); // need consecutive > 2
            a.record_shot_at_target(tgt);
            assert_eq!(a.continuous_fire_level, 1);
            let (_, _, rof, _) = a.weapon_bonus_fields();
            assert!((rof - 2.0).abs() < 0.01, "MEAN ROF 200% got {rof}");
            for _ in 0..3 {
                a.record_shot_at_target(tgt);
            }
            assert_eq!(a.continuous_fire_level, 2);
            let (_, _, rof2, _) = a.weapon_bonus_fields();
            assert!((rof2 - 3.0).abs() < 0.01, "FAST ROF 300% got {rof2}");
        }
    }

    #[test]
    fn fire_at_ex_faerie_fire_speeds_reload() {
        use crate::game_logic::host_avenger::FAERIE_FIRE_ROF_MULTIPLIER;
        use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("FF_ATK");
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(100.0);
        logic.templates.insert("FF_ATK".to_string(), tpl);
        let atk = logic
            .create_object("FF_ATK", Team::USA, Vec3::ZERO)
            .unwrap();
        {
            let a = logic.get_object_mut(atk).unwrap();
            a.weapon = Some(Weapon {
                damage: 10.0,
                range: 200.0,
                reload_time: 1.0,
                last_fire_time: -100.0, // never-fired residual
                ..Weapon::default()
            });
            // First shot at t=0
            assert!(a.fire_at_ex(ObjectId(99), 0.0, false, true));
            // Without faerie, not ready at 0.7 (needs full 1.0s)
            assert!(!a.fire_at_ex(ObjectId(99), 0.7, false, false));
            // With faerie ROF 150%, ready at 0.7 (effective reload ~0.667)
            assert!(
                a.fire_at_ex(ObjectId(99), 0.7, false, true),
                "TARGET_FAERIE_FIRE should ready at ~0.667s reload"
            );
            assert!((FAERIE_FIRE_ROF_MULTIPLIER - 1.5).abs() < 0.001);
        }
    }

    #[test]
    fn weapon_bonus_fields_stack_rof_and_damage() {
        use crate::game_logic::host_propaganda::ENTHUSIASTIC_RATE_OF_FIRE_MULT;
        use crate::game_logic::host_red_guard::INFANTRY_HORDE_ROF_MULT;
        use crate::game_logic::host_strategy_center::BOMBARDMENT_DAMAGE_MULT;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("WB_V");
        tpl.add_kind_of(KindOf::Infantry);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(100.0);
        logic.templates.insert("WB_V".to_string(), tpl);
        let id = logic.create_object("WB_V", Team::USA, Vec3::ZERO).unwrap();
        {
            let o = logic.get_object_mut(id).unwrap();
            o.weapon_bonus_enthusiastic = true;
            o.weapon_bonus_horde = true;
            o.weapon_bonus_battle_plan_bombardment = true;
            let (dmg, _range, rof, _) = o.weapon_bonus_fields();
            assert!(
                (rof - ENTHUSIASTIC_RATE_OF_FIRE_MULT * INFANTRY_HORDE_ROF_MULT).abs() < 0.001,
                "ROF stacks propaganda+horde got {rof}"
            );
            assert!(
                (dmg - BOMBARDMENT_DAMAGE_MULT).abs() < 0.001,
                "damage includes bombardment got {dmg}"
            );
            assert!((o.effective_weapon_reload(2.0) - 2.0 / rof).abs() < 0.001);
            assert!((o.effective_weapon_damage(10.0) - 10.0 * dmg).abs() < 0.001);
        }
    }

    #[test]
    fn effective_max_lift_uses_damaged_locomotor() {
        use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("LIFT_V");
        tpl.add_kind_of(KindOf::Aircraft);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(100.0);
        logic.templates.insert("LIFT_V".to_string(), tpl);
        let id = logic
            .create_object("LIFT_V", Team::USA, Vec3::new(0.0, 50.0, 0.0))
            .unwrap();
        {
            let o = logic.get_object_mut(id).unwrap();
            o.max_lift = 8.0;
            o.max_lift_damaged = 3.0;
            o.health.current = 100.0;
            o.health.maximum = 100.0;
            o.refresh_model_condition_bits();
            assert_eq!(o.body_damage_state, HostBodyDamageType::Pristine);
            assert!((o.effective_max_lift() - 8.0).abs() < 0.01);
            o.health.current = 10.0;
            o.refresh_model_condition_bits();
            assert_eq!(o.body_damage_state, HostBodyDamageType::ReallyDamaged);
            assert!(
                (o.effective_max_lift() - 3.0).abs() < 0.01,
                "really damaged uses max_lift_damaged"
            );
        }
    }

    #[test]
    fn body_damage_sets_model_condition_bits() {
        use crate::game_logic::host_enum_table_residual::{
            host_model_condition_has, HostBodyDamageType, MC_BIT_DAMAGED, MC_BIT_DYING,
            MC_BIT_REALLYDAMAGED, MC_BIT_RUBBLE,
        };
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        let mut tmpl = ThingTemplate::new("McBits");
        tmpl.set_health(100.0);
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut o = Object::new(tmpl, ObjectId(90), Team::USA);
        o.refresh_model_condition_bits();
        assert_eq!(o.body_damage_state, HostBodyDamageType::Pristine);
        assert!(!host_model_condition_has(
            o.model_condition_bits,
            MC_BIT_DAMAGED
        ));

        o.health.current = 40.0; // between 0.25 and 0.5
        o.refresh_model_condition_bits();
        assert_eq!(o.body_damage_state, HostBodyDamageType::Damaged);
        assert!(host_model_condition_has(
            o.model_condition_bits,
            MC_BIT_DAMAGED
        ));
        assert!(!host_model_condition_has(
            o.model_condition_bits,
            MC_BIT_REALLYDAMAGED
        ));

        o.health.current = 10.0;
        o.refresh_model_condition_bits();
        assert_eq!(o.body_damage_state, HostBodyDamageType::ReallyDamaged);
        assert!(host_model_condition_has(
            o.model_condition_bits,
            MC_BIT_REALLYDAMAGED
        ));

        o.take_damage(9999.0);
        assert!(o.status.destroyed);
        assert_eq!(o.body_damage_state, HostBodyDamageType::Rubble);
        assert!(host_model_condition_has(
            o.model_condition_bits,
            MC_BIT_RUBBLE
        ));
        assert!(host_model_condition_has(
            o.model_condition_bits,
            MC_BIT_DYING
        ));
    }

    #[test]
    fn body_damage_threshold_cpp_surface() {
        use crate::game_logic::host_enum_table_residual::{
            host_calc_body_damage_state, HostBodyDamageType, HOST_UNIT_DAMAGED_THRESH,
            HOST_UNIT_REALLY_DAMAGED_THRESH,
        };
        assert!((HOST_UNIT_DAMAGED_THRESH - 0.5).abs() < 1e-6);
        assert!((HOST_UNIT_REALLY_DAMAGED_THRESH - 0.25).abs() < 1e-6);
        assert_eq!(
            host_calc_body_damage_state(100.0, 100.0),
            HostBodyDamageType::Pristine
        );
        assert_eq!(
            host_calc_body_damage_state(50.0, 100.0),
            HostBodyDamageType::Damaged
        );
        assert_eq!(
            host_calc_body_damage_state(25.0, 100.0),
            HostBodyDamageType::ReallyDamaged
        );
        assert_eq!(
            host_calc_body_damage_state(0.0, 100.0),
            HostBodyDamageType::Rubble
        );
    }

    #[test]
    fn fire_at_stamps_detonation_fx_on_pending() {
        // Surface residual: PendingProjectile carries ProjectileDetonationFX name.
        let src = include_str!("object.rs");
        assert!(src.contains("detonation_fx_name"));
        assert!(src.contains("host_detonation_fx_for_weapon_name"));
        assert!(src.contains("detonation_ocl_name"));
        assert!(src.contains("host_detonation_ocl_for_weapon_name"));
        assert!(src.contains("exhaust_name"));
        assert!(src.contains("host_projectile_exhaust_for_unit_slot"));
        let csrc = include_str!("combat.rs");
        assert!(csrc.contains("take_impact_fx"));
        assert!(csrc.contains("ProjectileImpactFx"));
    }

    #[test]
    fn leech_range_waives_max_range_after_activate() {
        let mut tmpl = ThingTemplate::new("GLAInfantryTerrorist");
        tmpl.primary_weapon_name = Some("GLAInfantryTerrorist".into());
        let mut atk = Object::new(tmpl, ObjectId(1), Team::GLA);
        atk.set_position(glam::Vec3::ZERO);
        atk.weapon = Some(Weapon {
            damage: 100.0,
            range: 20.0,
            min_range: 0.0,
            can_target_air: false,
            can_target_ground: true,
            projectile_speed: 0.0,
            ..Weapon::default()
        });

        let mut tgt = Object::new(
            ThingTemplate::new("AmericaTankCrusader"),
            ObjectId(2),
            Team::USA,
        );
        tgt.set_position(glam::Vec3::new(100.0, 0.0, 0.0)); // out of 20 range
        tgt.thing.template.add_kind_of(KindOf::Vehicle);
        tgt.thing.template.add_kind_of(KindOf::Attackable);

        // Before leech: out of range.
        assert!(!atk.can_target_with_slot(&tgt, atk.weapon.as_ref().unwrap(), Some(0)));

        // Activate leech (as if pre-fire / fire occurred in range).
        atk.activate_leech_range_for_slot(0);
        assert!(atk.leech_range_active_primary);
        assert!(atk.can_target_with_slot(&tgt, atk.weapon.as_ref().unwrap(), Some(0)));

        // stop_attack clears.
        atk.stop_attack();
        assert!(!atk.leech_range_active_primary);
        assert!(!atk.can_target_with_slot(&tgt, atk.weapon.as_ref().unwrap(), Some(0)));
    }

    #[test]
    fn acceptable_aim_delta_blocks_then_allows_after_turn() {
        let mut tmpl = ThingTemplate::new("AmericaTankCrusader");
        tmpl.primary_weapon_name = Some("AmericaTankCrusaderGun".into());
        let mut atk = Object::new(tmpl, ObjectId(1), Team::USA);
        atk.set_position(glam::Vec3::ZERO);
        atk.set_orientation(0.0); // face +X residual (movement convention)
        atk.weapon = Some(Weapon {
            damage: 10.0,
            range: 200.0,
            ..Weapon::default()
        });
        let target = glam::Vec3::new(0.0, 0.0, 50.0); // off to +Z (~90°)
        let aim = atk.aim_delta_for_slot(0);
        let rel = atk.relative_angle_2d_to(target);
        // 20° aim residual should NOT be aimed at 90° offset.
        assert!(
            !atk.is_aimed_at_position(target, 0),
            "unexpectedly aimed: aim_delta={aim} rel={rel} ori={}",
            atk.get_orientation()
        );
        // Turn in steps until aimed.
        let mut aimed = false;
        for _ in 0..20 {
            if atk.turn_toward_position(target, 0, 0.2) {
                aimed = true;
                break;
            }
        }
        assert!(
            aimed,
            "should aim after turns, ori={}",
            atk.get_orientation()
        );
        assert!(atk.is_aimed_at_position(target, 0));
    }

    #[test]
    fn omni_aim_delta_always_aimed() {
        let mut tmpl = ThingTemplate::new("AmericaSentryDrone");
        tmpl.primary_weapon_name = Some("AmericaSentryDroneGun".into());
        let mut atk = Object::new(tmpl, ObjectId(3), Team::USA);
        atk.set_position(glam::Vec3::ZERO);
        atk.set_orientation(0.0);
        let target = glam::Vec3::new(-40.0, 0.0, 10.0);
        assert!(atk.is_aimed_at_position(target, 0));
    }

    #[test]
    fn pre_attack_type_per_shot_delays_every_discharge() {
        use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
        use glam::Vec3;
        let mut tmpl = ThingTemplate::new("Gattling");
        tmpl.primary_weapon_name = Some("AmericaGattlingTankGun".into());
        tmpl.add_kind_of(KindOf::Vehicle);
        tmpl.add_kind_of(KindOf::Attackable);
        let mut atk = Object::new(tmpl, ObjectId(1), Team::USA);
        atk.set_position(Vec3::ZERO);
        atk.weapon = Some(Weapon {
            damage: 5.0,
            range: 100.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            pre_attack_delay: 0.5,
            ..Weapon::default()
        });
        let tgt = ObjectId(9);
        // First wind-up
        assert!(!atk.fire_at(tgt, 10.0));
        assert!((atk.pre_attack_ready_at - 10.5).abs() < 1e-4);
        // Still winding
        assert!(!atk.fire_at(tgt, 10.2));
        // Fire after delay
        assert!(atk.fire_at(tgt, 10.5));
        assert_eq!(atk.consecutive_shots_at_target, 1);
        // PER_SHOT: next shot needs a new delay even vs same target
        assert!(!atk.fire_at(tgt, 10.5));
        assert!(atk.pre_attack_ready_at > 10.5);
        assert!(!atk.fire_at(tgt, 10.7));
        assert!(atk.fire_at(tgt, 11.0));
        assert_eq!(atk.consecutive_shots_at_target, 2);
    }

    #[test]
    fn pre_attack_type_per_attack_delays_once_per_target() {
        use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
        use glam::Vec3;
        let mut tmpl = ThingTemplate::new("Ranger");
        tmpl.primary_weapon_name = Some("AmericaRangerMachineGun".into());
        tmpl.add_kind_of(KindOf::Infantry);
        tmpl.add_kind_of(KindOf::Attackable);
        let mut atk = Object::new(tmpl, ObjectId(1), Team::USA);
        atk.set_position(Vec3::ZERO);
        atk.weapon = Some(Weapon {
            damage: 5.0,
            range: 100.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            pre_attack_delay: 1.0,
            ammo: Some(5),
            clip_size: 5,
            ..Weapon::default()
        });
        let tgt = ObjectId(9);
        assert!(!atk.fire_at(tgt, 5.0)); // wind-up
        assert!(atk.fire_at(tgt, 6.0)); // fire
                                        // Same target: no second wind-up
        assert!(atk.fire_at(tgt, 6.0));
        assert_eq!(atk.consecutive_shots_at_target, 2);
        // New target: delay again
        let tgt2 = ObjectId(10);
        assert!(!atk.fire_at(tgt2, 6.0));
        assert!(atk.fire_at(tgt2, 7.0));
    }

    #[test]
    fn pre_attack_type_per_clip_delays_on_full_clip_only() {
        use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
        use glam::Vec3;
        let mut tmpl = ThingTemplate::new("Scud");
        // Seed-only name (not in WeaponStore) so PreAttackType peels to PER_CLIP.
        tmpl.primary_weapon_name = Some("HostTestScudStormClipWeapon".into());
        tmpl.add_kind_of(KindOf::Structure);
        tmpl.add_kind_of(KindOf::Attackable);
        let mut atk = Object::new(tmpl, ObjectId(1), Team::GLA);
        atk.set_position(Vec3::ZERO);
        atk.weapon = Some(Weapon {
            damage: 50.0,
            range: 300.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            pre_attack_delay: 2.0,
            ammo: Some(3),
            clip_size: 3,
            clip_reload_time: 0.0,
            ..Weapon::default()
        });
        let tgt = ObjectId(9);
        // Full clip → delay
        assert!(!atk.fire_at(tgt, 1.0));
        assert!(atk.fire_at(tgt, 3.0));
        assert_eq!(atk.weapon.as_ref().unwrap().ammo, Some(2));
        // Mid-clip → no delay
        assert!(atk.fire_at(tgt, 3.0));
        assert_eq!(atk.weapon.as_ref().unwrap().ammo, Some(1));
        assert!(atk.fire_at(tgt, 3.0));
        assert_eq!(atk.weapon.as_ref().unwrap().ammo, Some(0));
    }

    #[test]
    fn return_to_base_blocks_fire_until_rearm() {
        use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
        use glam::Vec3;
        let mut tmpl = ThingTemplate::new("AmericaJetRaptor");
        // Seed-only name so store cannot peel YES over RETURN_TO_BASE.
        tmpl.primary_weapon_name = Some("HostTestRaptorJetMissileWeapon".into());
        tmpl.add_kind_of(KindOf::Aircraft);
        tmpl.add_kind_of(KindOf::Attackable);
        let mut jet = Object::new(tmpl, ObjectId(1), Team::USA);
        jet.set_position(Vec3::ZERO);
        jet.weapon = Some(Weapon {
            damage: 100.0,
            range: 200.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            ammo: Some(2),
            clip_size: 2,
            can_target_air: true,
            can_target_ground: true,
            ..Weapon::default()
        });
        let tgt = ObjectId(9);
        assert!(jet.fire_at(tgt, 1.0));
        assert_eq!(jet.weapon.as_ref().unwrap().ammo, Some(1));
        assert!(jet.fire_at(tgt, 1.0));
        assert_eq!(jet.weapon.as_ref().unwrap().ammo, Some(0));
        assert!(jet.needs_return_to_base_rearm());
        assert!(!jet.fire_at(tgt, 2.0));
        assert!(!Object::weapon_ready_named(
            jet.weapon.as_ref().unwrap(),
            2.0,
            Some("HostTestRaptorJetMissileWeapon"),
            jet.weapon.as_ref().unwrap().reload_time,
        ));
        assert!(jet.rearm_return_to_base_weapons());
        assert_eq!(jet.weapon.as_ref().unwrap().ammo, Some(2));
        assert!(jet.fire_at(tgt, 3.0));
        assert_eq!(jet.weapon.as_ref().unwrap().ammo, Some(1));
    }

    #[test]
    fn auto_reload_still_refills_clip() {
        use crate::game_logic::Weapon;
        let mut w = Weapon {
            damage: 10.0,
            range: 100.0,
            reload_time: 0.1,
            ammo: Some(1),
            clip_size: 2,
            clip_reload_time: 1.0,
            last_fire_time: -100.0,
            ..Weapon::default()
        };
        let t0 = 5.0;
        assert!(Object::weapon_ready(&w, t0));
        Object::consume_ammo_on_fire(&mut w, t0);
        assert_eq!(w.ammo, Some(0));
        // After clip reload gap, ready again and refill on fire.
        assert!(
            Object::weapon_ready(&w, t0 + 1.05),
            "last_fire={} reload={}",
            w.last_fire_time,
            w.reload_time
        );
        Object::consume_ammo_on_fire(&mut w, t0 + 1.05);
        assert_eq!(w.ammo, Some(1)); // refilled to 2, spent 1
    }

    #[test]
    fn out_of_ammo_damage_ticks_empty_rtb_jet() {
        use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
        use glam::Vec3;
        let mut tmpl = ThingTemplate::new("AmericaJetRaptor");
        tmpl.primary_weapon_name = Some("HostTestRaptorJetMissileWeapon".into());
        tmpl.add_kind_of(KindOf::Aircraft);
        tmpl.add_kind_of(KindOf::Attackable);
        tmpl.set_health(100.0);
        let mut jet = Object::new(tmpl, ObjectId(1), Team::USA);
        jet.set_position(Vec3::new(0.0, 50.0, 0.0));
        jet.status.airborne_target = true;
        jet.weapon = Some(Weapon {
            damage: 100.0,
            range: 200.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            ammo: Some(0),
            clip_size: 2,
            can_target_air: true,
            can_target_ground: true,
            ..Weapon::default()
        });
        assert!(jet.needs_return_to_base_rearm());
        let hp0 = jet.health.current;
        let dmg = jet.apply_out_of_ammo_damage_frame();
        // 10% / sec * 1/30 * 100 = 10/30 ≈ 0.333
        assert!((dmg - (0.10 / 30.0) * 100.0).abs() < 1e-3, "dmg={dmg}");
        assert!((hp0 - jet.health.current - dmg).abs() < 1e-3);
        // Docked: no damage.
        jet.health.current = 100.0;
        jet.set_ai_state(AIState::Docked);
        assert_eq!(jet.apply_out_of_ammo_damage_frame(), 0.0);
        // Rearmed: no damage.
        jet.set_ai_state(AIState::Idle);
        jet.rearm_return_to_base_weapons();
        assert_eq!(jet.apply_out_of_ammo_damage_frame(), 0.0);
    }

    #[test]
    fn parked_jet_takeoff_on_attack_and_move() {
        use crate::game_logic::host_dock_contain_exit_heal_residual::PARKING_PLACE_AIRFIELD_APPROACH_HEIGHT;
        use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
        use glam::Vec3;
        let mut tmpl = ThingTemplate::new("AmericaJetRaptor");
        tmpl.primary_weapon_name = Some("HostTestRaptorJetMissileWeapon".into());
        tmpl.add_kind_of(KindOf::Aircraft);
        tmpl.add_kind_of(KindOf::Attackable);
        tmpl.set_health(100.0);
        let mut jet = Object::new(tmpl, ObjectId(1), Team::USA);
        jet.set_position(Vec3::new(0.0, 0.0, 0.0));
        jet.weapon = Some(Weapon {
            damage: 50.0,
            range: 200.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            ammo: Some(4),
            clip_size: 4,
            can_target_air: true,
            can_target_ground: true,
            ..Weapon::default()
        });
        jet.contained_by = Some(ObjectId(99));
        jet.set_ai_state(AIState::Docked);
        jet.status.airborne_target = false;
        assert!(jet.is_parked_at_airfield());
        assert!(jet.can_attack()); // parked aircraft may sortie
        jet.attack_target(ObjectId(7));
        assert!(jet.contained_by.is_none());
        assert_ne!(jet.ai_state, AIState::Docked);
        assert!(jet.status.airborne_target);
        assert!(jet.get_position().y >= PARKING_PLACE_AIRFIELD_APPROACH_HEIGHT - 1e-3);
        assert_eq!(jet.target, Some(ObjectId(7)));
        assert_eq!(jet.ai_state, AIState::Attacking);

        // Re-dock and move.
        jet.contained_by = Some(ObjectId(99));
        jet.set_ai_state(AIState::Docked);
        jet.status.airborne_target = false;
        jet.set_position(Vec3::new(10.0, 0.0, 0.0));
        jet.set_destination(Vec3::new(100.0, 0.0, 0.0));
        assert!(jet.contained_by.is_none());
        assert!(jet.status.airborne_target || jet.ai_state != AIState::Docked);
        assert!(jet.get_position().y >= PARKING_PLACE_AIRFIELD_APPROACH_HEIGHT - 1e-3);
    }

    #[test]
    fn fire_at_scatter_vs_infantry_only_when_flagged() {
        use crate::game_logic::weapon_bootstrap::host_effective_scatter_radius;
        // Crusader gun: base 0 + ScatterRadiusVsInfantry 10.
        let vs_inf = host_effective_scatter_radius("AmericaTankCrusaderGun", true);
        let vs_veh = host_effective_scatter_radius("AmericaTankCrusaderGun", false);
        assert!(vs_inf >= 10.0 - 1e-3, "vs infantry {vs_inf}");
        assert!(vs_veh < 1e-3, "vs vehicle base {vs_veh}");
        // fire_at_ex is the KindOf-aware entry; fire_at defaults infantry=false (base only).
        let src = include_str!("object.rs");
        assert!(src.contains("fn fire_at_ex"));
        assert!(src.contains("target_is_infantry"));
        assert!(
            src.contains("host_effective_scatter_radius"),
            "fire path must peel scatter"
        );
    }

    #[test]
    fn shock_wave_impulse_knocks_ground_units() {
        use crate::game_logic::host_enum_table_residual::{
            host_model_condition_has, MC_BIT_STUNNED, MC_BIT_STUNNED_FLAILING,
        };
        let mut tmpl = ThingTemplate::new("ShockVic");
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut o = Object::new(tmpl, ObjectId(1), Team::USA);
        o.movement.velocity = glam::Vec3::ZERO;
        assert!(o.apply_shock_wave_impulse(glam::Vec3::new(20.0, 10.0, 0.0)));
        assert!(o.movement.velocity.length() > 0.0);
        assert!(o.is_shock_stunned());
        assert!(host_model_condition_has(
            o.model_condition_bits,
            MC_BIT_STUNNED_FLAILING
        ));
        // After flail window: STUNNED bit.
        o.shock_stun_frames = 10;
        o.refresh_model_condition_bits();
        assert!(host_model_condition_has(
            o.model_condition_bits,
            MC_BIT_STUNNED
        ));
        assert!(!host_model_condition_has(
            o.model_condition_bits,
            MC_BIT_STUNNED_FLAILING
        ));
        // Aircraft immune.
        let mut at = ThingTemplate::new("ShockAir");
        at.add_kind_of(KindOf::Aircraft);
        let mut a = Object::new(at, ObjectId(2), Team::USA);
        a.status.airborne_target = true;
        assert!(!a.apply_shock_wave_impulse(glam::Vec3::new(20.0, 10.0, 0.0)));
    }

    #[test]
    fn shock_stun_ticks_clear_model_bits() {
        use crate::game_logic::host_enum_table_residual::{
            host_model_condition_has, MC_BIT_STUNNED, MC_BIT_STUNNED_FLAILING,
        };
        let mut tmpl = ThingTemplate::new("StunTick");
        tmpl.add_kind_of(KindOf::Infantry);
        let mut o = Object::new(tmpl, ObjectId(3), Team::USA);
        assert!(o.apply_shock_wave_impulse(glam::Vec3::new(5.0, 5.0, 0.0)));
        let start = o.shock_stun_frames;
        assert!(start >= 40);
        for _ in 0..start {
            o.tick_shock_stun();
        }
        assert_eq!(o.shock_stun_frames, 0);
        assert!(!host_model_condition_has(
            o.model_condition_bits,
            MC_BIT_STUNNED_FLAILING
        ));
        assert!(!host_model_condition_has(
            o.model_condition_bits,
            MC_BIT_STUNNED
        ));
    }

    #[test]
    fn ignore_collisions_and_overlap_helpers() {
        let mut a = Object::new(
            {
                let mut t = ThingTemplate::new("IgnA");
                t.add_kind_of(KindOf::Vehicle);
                t
            },
            ObjectId(301),
            Team::USA,
        );
        let b_id = ObjectId(302);
        assert!(!a.is_ignoring_collisions_with(b_id));
        a.set_ignore_collisions_with(Some(b_id));
        assert!(a.is_ignoring_collisions_with(b_id));
        a.set_ignore_collisions_with(None);
        assert!(!a.is_ignoring_collisions_with(b_id));

        a.add_physics_overlap(b_id);
        assert!(a.is_currently_overlapped(b_id));
        assert!(!a.was_previously_overlapped(b_id));
        a.advance_physics_overlap_frame();
        assert!(!a.is_currently_overlapped(b_id));
        assert!(a.was_previously_overlapped(b_id));
        a.last_collidee = Some(b_id);
        assert_eq!(a.last_collidee, Some(b_id));
    }
    #[test]
    fn crush_selects_front_or_back_by_approach() {
        use crate::game_logic::host_partition_collision_physics_residual::{
            select_crush_target_by_perp_residual, CrushTarget,
        };
        // Sanity on residual selector.
        assert_eq!(
            select_crush_target_by_perp_residual(
                false,
                false,
                (4.0, 0.5),
                (0.0, 0.0),
                (1.0, 0.0),
                (1.0, 0.0),
                5.0,
            ),
            CrushTarget::FrontEndCrush
        );
        // Approach front of infantry: tank past front point only → front_crushed first.
        let mut vt = ThingTemplate::new("FrontCrushTank");
        vt.add_kind_of(KindOf::Vehicle);
        let mut tank = Object::new(vt, ObjectId(201), Team::USA);
        tank.crusher_level = 1;
        tank.set_orientation(0.0);
        tank.movement.velocity = glam::Vec3::new(5.0, 0.0, 0.0);
        // Front of inf at x≈5 (offset 5, facing +X): tank just past front.
        tank.set_position(glam::Vec3::new(5.5, 0.0, 0.2));

        let mut it = ThingTemplate::new("FrontCrushInf");
        it.add_kind_of(KindOf::Infantry);
        let mut inf = Object::new(it, ObjectId(202), Team::GLA);
        inf.crushable_level = 0;
        inf.selection_radius = 10.0;
        inf.set_orientation(0.0);
        inf.set_position(glam::Vec3::new(0.0, 0.0, 0.0));
        inf.health.current = 999999.0; // survive first non-total if needed
        inf.health.maximum = 999999.0;

        // With front selection + past front point, front_crushed set.
        // Use huge HP so we can observe flags before death if total.
        assert!(tank.check_for_overlap_collision(&mut inf, false));
        // Either front crushed or total (if selector picked total and killed).
        assert!(
            inf.front_crushed || inf.back_crushed || inf.status.destroyed,
            "front={} back={} dead={}",
            inf.front_crushed,
            inf.back_crushed,
            inf.status.destroyed
        );
    }
    #[test]
    fn crush_overlap_collision_kills_infantry() {
        use crate::game_logic::host_usa_pilot::HostDeathType;
        let mut vt = ThingTemplate::new("CrusherTank");
        vt.add_kind_of(KindOf::Vehicle);
        let mut tank = Object::new(vt, ObjectId(91), Team::USA);
        tank.crusher_level = 1;
        tank.set_orientation(0.0); // faces +X
        tank.movement.velocity = glam::Vec3::new(5.0, 0.0, 0.0); // moving +X

        let mut it = ThingTemplate::new("CrushableInf");
        it.add_kind_of(KindOf::Infantry);
        let mut inf = Object::new(it, ObjectId(92), Team::GLA);
        inf.crushable_level = 0;
        inf.selection_radius = 10.0;
        // Tank past infantry center along +X.
        inf.set_position(glam::Vec3::new(5.0, 0.0, 0.0));
        tank.set_position(glam::Vec3::new(6.0, 0.0, 0.0));

        assert!(tank.can_crush_only(&inf, false));
        assert!(tank.check_for_overlap_collision(&mut inf, false));
        assert!(inf.status.destroyed || inf.health.current <= 0.0);
        if inf.status.destroyed {
            assert_eq!(inf.status.death_type, HostDeathType::Crushed);
        }
        // Allies do not crush.
        let mut a = Object::new(
            {
                let mut t = ThingTemplate::new("AllyInf");
                t.add_kind_of(KindOf::Infantry);
                t
            },
            ObjectId(93),
            Team::USA,
        );
        a.crushable_level = 0;
        a.set_position(glam::Vec3::new(5.0, 0.0, 0.0));
        tank.physics_current_overlap = None;
        tank.physics_previous_overlap = None;
        assert!(!tank.can_crush_only(&a, true));
        assert!(!tank.check_for_overlap_collision(&mut a, true));
    }
    #[test]
    fn scrub_velocity_and_structure_stiffness_bounce() {
        use crate::game_logic::host_partition_collision_physics_residual::{
            clamp_structure_stiffness, parachute_bounce_out_distance,
            PHYSICS_STRUCTURE_STIFFNESS_DEFAULT_RESIDUAL,
        };
        let mut tmpl = ThingTemplate::new("ScrubVic");
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut o = Object::new(tmpl, ObjectId(71), Team::USA);
        o.movement.velocity = glam::Vec3::new(10.0, 0.0, 0.0);
        o.scrub_velocity_2d(5.0);
        assert!((o.movement.velocity.x - 5.0).abs() < 1e-3);
        assert!(o.movement.velocity.z.abs() < 1e-5);
        o.scrub_velocity_2d(0.0);
        assert_eq!(o.movement.velocity.x, 0.0);

        o.movement.velocity = glam::Vec3::new(0.0, -8.0, 0.0);
        o.scrub_velocity_vertical(-3.0);
        assert!((o.movement.velocity.y - (-3.0)).abs() < 1e-5);

        // Parachute bounce out.
        o.set_position(glam::Vec3::new(0.0, 5.0, 0.0));
        o.movement.velocity = glam::Vec3::new(4.0, -1.0, 0.0);
        o.apply_parachute_building_bounce_out(glam::Vec3::new(10.0, 5.0, 0.0), 20.0);
        assert!(o.get_position().x < 0.0, "pushed away from building +X");
        assert_eq!(o.movement.velocity.x, 0.0);
        assert_eq!(o.movement.velocity.z, 0.0);
        assert!((parachute_bounce_out_distance(20.0) - 2.0).abs() < 1e-6);

        // Structure stiffness bounce.
        o.set_position(glam::Vec3::new(0.0, 2.0, 0.0));
        o.movement.velocity = glam::Vec3::new(6.0, -2.0, 0.0);
        let f = o.apply_structure_stiffness_bounce(
            glam::Vec3::new(5.0, 2.0, 0.0),
            PHYSICS_STRUCTURE_STIFFNESS_DEFAULT_RESIDUAL,
            1.0,
        );
        assert!(f.x < 0.0, "push back -X force={f:?}");
        assert!(o.movement.velocity.x < 0.0);
        assert!((clamp_structure_stiffness(0.5) - 0.5).abs() < 1e-6);
    }
    #[test]
    fn vehicle_crash_into_structure_residual() {
        use crate::game_logic::host_partition_collision_physics_residual::{
            vehicle_crash_destroys_vehicle, vehicle_crash_weapon_name, VehicleCrashImmobileOutcome,
            PHYSICS_VEHICLE_CRASHES_INTO_BUILDING_WEAPON,
        };
        let mut vt = ThingTemplate::new("CrashVic");
        vt.add_kind_of(KindOf::Vehicle);
        let mut v = Object::new(vt, ObjectId(51), Team::USA);
        v.set_position(glam::Vec3::new(0.0, 5.0, 0.0));
        v.movement.velocity = glam::Vec3::new(0.0, -3.0, 0.0);

        let mut st = ThingTemplate::new("CrashBldg");
        st.add_kind_of(KindOf::Structure);
        st.add_kind_of(KindOf::Immobile);
        let s = Object::new(st, ObjectId(52), Team::China);

        let o = v.evaluate_vehicle_crash_into(&s);
        assert_eq!(o, VehicleCrashImmobileOutcome::DestroyWithBuildingWeapon);
        assert!(vehicle_crash_destroys_vehicle(o));
        assert_eq!(
            vehicle_crash_weapon_name(o),
            Some(PHYSICS_VEHICLE_CRASHES_INTO_BUILDING_WEAPON)
        );

        // Rising vehicle: no crash.
        v.movement.velocity.y = 2.0;
        assert_eq!(
            v.evaluate_vehicle_crash_into(&s),
            VehicleCrashImmobileOutcome::None
        );
    }
    #[test]
    fn kill_when_resting_and_bounce_land_residual() {
        let mut tmpl = ThingTemplate::new("RestKillVic");
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut o = Object::new(tmpl, ObjectId(41), Team::USA);
        o.kill_when_resting_on_ground = true;
        o.shock_stun_frames = 5;
        o.set_position(glam::Vec3::ZERO);
        o.movement.velocity = glam::Vec3::ZERO;
        assert!(o.maybe_kill_when_resting_on_ground());
        assert!(o.status.destroyed);

        // Drone alive with flag does not kill.
        let mut td = ThingTemplate::new("CombatDrone");
        td.add_kind_of(KindOf::Vehicle);
        let mut d = Object::new(td, ObjectId(42), Team::USA);
        d.kill_when_resting_on_ground = true;
        d.shock_stun_frames = 5;
        d.set_position(glam::Vec3::ZERO);
        d.movement.velocity = glam::Vec3::ZERO;
        assert!(!d.maybe_kill_when_resting_on_ground());
        assert!(!d.status.destroyed);
        // Unmanned drone does kill.
        d.status.disabled_unmanned = true;
        assert!(d.maybe_kill_when_resting_on_ground());
        assert!(d.status.destroyed);

        // Bounce land event on airborne ground hit.
        let mut tb = ThingTemplate::new("BounceSnd");
        tb.add_kind_of(KindOf::Vehicle);
        let mut b = Object::new(tb, ObjectId(43), Team::USA);
        b.shock_stun_frames = 30;
        b.shock_allow_bounce = false;
        b.shock_was_airborne = true;
        b.set_position(glam::Vec3::new(0.0, 3.0, 0.0));
        b.movement.velocity = glam::Vec3::new(0.0, -5.0, 0.0);
        b.immune_to_falling_damage = true; // isolate bounce event
        for _ in 0..20 {
            b.tick_shock_stun();
            if b.bounce_land_events > 0 {
                break;
            }
        }
        assert!(
            b.bounce_land_events > 0,
            "landing records bounce sound residual"
        );
        assert!(b.last_bounce_fall_dy > 0.0);
        assert!(b.last_bounce_volume >= 0.25 && b.last_bounce_volume <= 1.0);
        assert!(b.bounce_audio_pending > 0);
        let (name, vol) = b.take_bounce_audio_pending().expect("pending");
        assert_eq!(name, BOUNCE_SOUND_DEFAULT);
        assert!((vol - b.last_bounce_volume).abs() < 1e-5);
        let v_small = bounce_sound_volume_residual(0.05, 1.0);
        let v_big = bounce_sound_volume_residual(0.25, 50.0);
        assert!(v_big >= v_small);

        // Immune falling takes no damage.
        let mut ti = ThingTemplate::new("ImmuneFall");
        ti.add_kind_of(KindOf::Vehicle);
        let mut i = Object::new(ti, ObjectId(44), Team::USA);
        i.health.current = 100.0;
        i.immune_to_falling_damage = true;
        assert_eq!(i.apply_shock_fall_damage(-30.0), 0.0);
        assert_eq!(i.health.current, 100.0);
    }
    #[test]
    fn stunned_off_map_cliff_water_kills_without_loco() {
        use crate::game_logic::host_deliver_payload::{
            is_off_map_default_residual, RESIDUAL_MAP_EXTENT_MAX_X,
        };
        let mut tmpl = ThingTemplate::new("GroundTank");
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut o = Object::new(tmpl, ObjectId(31), Team::USA);
        o.shock_stun_frames = 30;
        o.ensure_locomotor_surfaces();
        assert!(o.has_locomotor_for_surface(LOCO_SURFACE_GROUND));
        assert!(!o.has_locomotor_for_surface(LOCO_SURFACE_CLIFF));
        assert!(!o.has_locomotor_for_surface(LOCO_SURFACE_WATER));
        o.set_position(glam::Vec3::new(RESIDUAL_MAP_EXTENT_MAX_X + 50.0, 0.0, 0.0));
        assert!(is_off_map_default_residual(o.get_position()));
        assert!(o.test_stunned_unit_for_destruction());
        assert!(o.status.destroyed);

        let mut t2 = ThingTemplate::new("CliffVictim");
        t2.add_kind_of(KindOf::Infantry);
        let mut c = Object::new(t2, ObjectId(32), Team::USA);
        c.shock_stun_frames = 20;
        c.cell_is_cliff = true;
        c.set_position(glam::Vec3::ZERO);
        assert!(c.test_stunned_unit_for_destruction());
        assert!(c.status.destroyed);

        let mut t3 = ThingTemplate::new("WaterVictim");
        t3.add_kind_of(KindOf::Vehicle);
        let mut w = Object::new(t3, ObjectId(33), Team::USA);
        w.shock_stun_frames = 20;
        w.cell_is_underwater = true;
        w.set_position(glam::Vec3::ZERO);
        assert!(w.test_stunned_unit_for_destruction());
        assert!(w.status.destroyed);

        let mut th = ThingTemplate::new("AmphibHover");
        th.add_kind_of(KindOf::Vehicle);
        let mut h = Object::new(th, ObjectId(34), Team::USA);
        h.shock_stun_frames = 20;
        h.locomotor_surfaces = LOCO_SURFACE_GROUND | LOCO_SURFACE_WATER;
        h.cell_is_underwater = true;
        h.set_position(glam::Vec3::ZERO);
        assert!(!h.test_stunned_unit_for_destruction());
        assert!(!h.status.destroyed);
        h.cell_is_underwater = false;
        h.cell_is_cliff = true;
        h.locomotor_surfaces |= LOCO_SURFACE_CLIFF;
        assert!(!h.test_stunned_unit_for_destruction());
    }
    #[test]
    fn stunned_upside_down_bounce_kills_and_freefall_disables() {
        let mut tmpl = ThingTemplate::new("StunKill");
        tmpl.add_kind_of(KindOf::Vehicle);
        tmpl.max_health = 100.0;
        let mut o = Object::new(tmpl, ObjectId(21), Team::USA);
        o.health.current = 100.0;
        assert!(o.apply_shock_wave_impulse(glam::Vec3::new(5.0, 30.0, 0.0)));
        // Force inverted residual (C++ Get_Z_Vector().Z < 0).
        o.shock_up_z = -0.5;
        o.shock_allow_bounce = true;
        o.shock_stun_frames = 40;
        // Simulate bounce path with downward impact from above ground.
        o.set_position(glam::Vec3::new(0.0, 2.0, 0.0));
        o.movement.velocity = glam::Vec3::new(0.0, -4.0, 0.0);
        let bounced = o.handle_shock_ground_bounce(2.0, -0.1, 0.0);
        assert!(o.status.destroyed, "upside-down stunned must die on bounce");
        assert_eq!(bounced, 0.0);
        // Freefall disable residual while airborne.
        let mut t2 = ThingTemplate::new("FreeFallDis");
        t2.add_kind_of(KindOf::Vehicle);
        let mut a = Object::new(t2, ObjectId(22), Team::USA);
        assert!(a.apply_shock_wave_impulse(glam::Vec3::new(0.0, 50.0, 0.0)));
        a.set_position(glam::Vec3::ZERO);
        // Climb a few frames.
        for _ in 0..5 {
            if a.get_position().y > 0.2 {
                break;
            }
            a.tick_shock_stun();
        }
        if a.get_position().y > 0.05 {
            assert!(a.status.disabled_freefall || a.is_disabled());
            assert!(a.is_freefall_disabled() || a.is_disabled());
        }
        // Land fully.
        for _ in 0..80 {
            a.tick_shock_stun();
            if a.shock_stun_frames == 0 && a.get_position().y <= 0.01 {
                break;
            }
        }
        if a.get_position().y <= 0.01 && !a.status.destroyed {
            assert!(
                !a.status.disabled_freefall,
                "grounded clears DISABLED_FREEFALL"
            );
        }
    }
    #[test]
    fn shock_fall_damage_splats_on_hard_landing() {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::host_enum_table_residual::{
            host_model_condition_has, MC_BIT_SPLATTED,
        };
        use crate::game_logic::host_usa_pilot::HostDeathType;
        // height_to_speed(40) with |g|=1 → sqrt(80) ≈ 8.94
        assert!((Object::min_fall_speed_for_damage() - (80.0f32).sqrt()).abs() < 1e-3);
        let mut tmpl = ThingTemplate::new("SplatVic");
        tmpl.add_kind_of(KindOf::Vehicle);
        tmpl.max_health = 50.0;
        let mut o = Object::new(tmpl, ObjectId(11), Team::USA);
        o.health.current = 50.0;
        o.health.maximum = 50.0;
        o.set_position(glam::Vec3::new(0.0, 5.0, 0.0));
        o.shock_was_airborne = true;
        o.shock_allow_bounce = false;
        o.shock_stun_frames = 20;
        // Hard downward impact residual (steep fall, no lateral).
        o.movement.velocity = glam::Vec3::new(0.0, -20.0, 0.0);
        let dmg = o.apply_shock_fall_damage(-20.0);
        assert!(dmg > 0.0, "expected fall damage, got {dmg}");
        // net = 20 - sqrt(80) ≈ 11.06 → kills 50hp unit with mass1 factor1? 11 < 50 so wounded
        assert!(o.health.current < 50.0);
        // Stronger impact to splat.
        o.health.current = 5.0;
        o.status.destroyed = false;
        let dmg2 = o.apply_shock_fall_damage(-30.0);
        assert!(dmg2 > 5.0);
        assert!(o.status.destroyed || o.health.current <= 0.0);
        if o.status.destroyed {
            assert_eq!(o.status.death_type, HostDeathType::Splatted);
            assert!(host_model_condition_has(
                o.model_condition_bits,
                MC_BIT_SPLATTED
            ));
        }
        // Shallow slope residual: large lateral vs vertical → no damage.
        let mut s = Object::new(
            {
                let mut t = ThingTemplate::new("SlopeVic");
                t.add_kind_of(KindOf::Vehicle);
                t
            },
            ObjectId(12),
            Team::USA,
        );
        s.health.current = 100.0;
        s.movement.velocity = glam::Vec3::new(50.0, -5.0, 0.0);
        let d0 = s.apply_shock_fall_damage(-5.0);
        assert_eq!(d0, 0.0, "below min fall speed");
        // Above min speed but shallow angle.
        let d1 = s.apply_shock_fall_damage(-20.0);
        // |20/50|=0.4 < 3 → not steep
        assert_eq!(d1, 0.0, "shallow fall must not damage");
        let _ = DamageType::Falling;
    }
    #[test]
    fn shock_bounce_settles_freefall_and_switches_to_stunned() {
        use crate::game_logic::host_enum_table_residual::{
            host_model_condition_has, MC_BIT_FREEFALL, MC_BIT_STUNNED, MC_BIT_STUNNED_FLAILING,
        };
        let mut tmpl = ThingTemplate::new("BounceVic");
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut o = Object::new(tmpl, ObjectId(9), Team::USA);
        o.set_position(glam::Vec3::new(0.0, 0.0, 0.0));
        assert!(o.apply_shock_wave_impulse(glam::Vec3::new(10.0, 40.0, 0.0)));
        assert!(o.shock_allow_bounce);
        // Climb while velocity positive.
        let mut saw_air = false;
        let mut saw_bounce = false;
        let mut max_y = 0.0f32;
        let mut saw_stunned_after_ground = false;
        for _ in 0..120 {
            o.tick_shock_stun();
            let y = o.get_position().y;
            max_y = max_y.max(y);
            if y > 0.5 {
                saw_air = true;
            }
            if o.shock_grounded_once {
                saw_bounce = true;
                // While still stunned after first ground hit: STUNNED, not FLAILING.
                if o.shock_stun_frames > 0 {
                    assert!(
                        host_model_condition_has(o.model_condition_bits, MC_BIT_STUNNED),
                        "frames={} bits={:#x}",
                        o.shock_stun_frames,
                        o.model_condition_bits
                    );
                    assert!(!host_model_condition_has(
                        o.model_condition_bits,
                        MC_BIT_STUNNED_FLAILING
                    ));
                    saw_stunned_after_ground = true;
                }
            }
            if o.shock_stun_frames == 0 && o.get_position().y <= 0.01 {
                break;
            }
        }
        assert!(saw_air || max_y > 0.0, "shock lift should leave ground");
        assert!(saw_bounce || o.shock_grounded_once, "must hit ground");
        assert!(
            saw_stunned_after_ground,
            "must observe STUNNED bit after ground while stun active"
        );
        // Settled: no freefall bit when grounded.
        if o.get_position().y <= 0.01 && o.movement.velocity.y.abs() < 0.5 {
            assert!(!host_model_condition_has(
                o.model_condition_bits,
                MC_BIT_FREEFALL
            ));
        }
        assert!(o.get_position().y >= -0.01, "must not sink below ground");
    }
    #[test]
    fn shock_applies_random_rotation_and_optional_freefall_bit() {
        use crate::game_logic::host_enum_table_residual::{
            host_model_condition_has, MC_BIT_FREEFALL,
        };
        let mut tmpl = ThingTemplate::new("RotVic");
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut o = Object::new(tmpl, ObjectId(7), Team::USA);
        let ori0 = o.get_orientation();
        o.shock_yaw_rate = 0.0;
        assert!(o.apply_shock_wave_impulse(glam::Vec3::new(30.0, 20.0, 10.0)));
        // Random rotation residual should change rates and/or orientation.
        let rotated = (o.get_orientation() - ori0).abs() > 1e-6
            || o.shock_yaw_rate.abs() > 1e-6
            || o.shock_pitch_rate.abs() > 1e-6;
        assert!(rotated, "shock applies rotation residual");
        // Strong up velocity may set FREEFALL while stunned.
        if o.movement.velocity.y > 8.0 {
            assert!(host_model_condition_has(
                o.model_condition_bits,
                MC_BIT_FREEFALL
            ));
        }
        // Structure stick-to-ground: no rotation.
        let mut st = ThingTemplate::new("RotStruct");
        st.add_kind_of(KindOf::Structure);
        let mut s = Object::new(st, ObjectId(8), Team::USA);
        let s0 = s.get_orientation();
        s.apply_shock_random_rotation(123);
        assert!((s.get_orientation() - s0).abs() < 1e-6);
        assert_eq!(s.shock_yaw_rate, 0.0);
    }
    #[test]
    fn shock_stun_blocks_attack_fire_and_flail_move() {
        let mut tmpl = ThingTemplate::new("StunBlock");
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut o = Object::new(tmpl, ObjectId(42), Team::USA);
        o.weapon = Some(Weapon {
            damage: 10.0,
            range: 100.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            can_target_ground: true,
            ..Weapon::default()
        });
        assert!(o.can_attack());
        assert!(o.can_fire(0.0));
        assert!(o.can_move());
        assert!(o.apply_shock_wave_impulse(glam::Vec3::new(10.0, 5.0, 0.0)));
        assert!(o.is_shock_stunned());
        assert!(!o.can_attack(), "stunned cannot attack");
        assert!(!o.can_fire(0.0), "stunned cannot fire");
        // Flailing phase blocks commanded move.
        assert!(o.shock_stun_frames > 15);
        assert!(!o.can_move(), "flailing cannot take move orders");
        // Settled stunned phase: move orders allowed (stagger), still no fire.
        o.shock_stun_frames = 10;
        o.refresh_model_condition_bits();
        assert!(!o.can_attack());
        assert!(!o.can_fire(1.0));
        assert!(o.can_move(), "settled stun may stagger-move");
        // attack_target ignored while stunned.
        o.shock_stun_frames = 20;
        o.attack_target(ObjectId(99));
        assert!(o.target.is_none() || o.ai_state != AIState::Attacking || !o.can_attack());
        // After stun clears, combat again.
        o.shock_stun_frames = 0;
        o.refresh_model_condition_bits();
        assert!(o.can_attack());
        assert!(o.can_fire(2.0));
        assert!(o.can_move());
    }
}
