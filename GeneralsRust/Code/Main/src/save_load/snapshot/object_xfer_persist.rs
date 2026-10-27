//! Persist leftover Object / Weapon / TurretAI / Stealth / OpenContain / AIUpdate clocks.
//!
//! C++ `Object::xfer` writes vision/shroud ranges, DISABLED_HELD,
//! `m_singleUseCommandUsed`, `m_indicatorColor`, `m_weaponBonusCondition`,
//! `m_smcUntil`, `m_isReceivingDifficultyBonus`, v9 `m_safeOcclusionFrame`,
//! and `m_healthBoxOffset`. `Weapon::xfer` writes `m_whenPreAttackFinished`,
//! `m_maxShotCount`, `m_scatterTargetsUnused`, and `m_lastFireFrame`.
//! `TurretAI::xfer` v2 writes angle/pitch/target/hold/enabled/state.
//! `StealthUpdate::xfer` v2 writes `m_framesGranted`. `OpenContain::xfer` v2
//! writes `m_whichExitPath`. `AIUpdateInterface::xfer` writes
//! `m_isRecruitable`, `m_nextEnemyScanTime`, `m_ignoreCollisionsUntil`,
//! `m_ignoreObstacleID`, `m_finalPosition`/`m_doFinalPosition`,
//! `m_canPathThroughUnits`, and `m_lastCommandSource`. Leftover xfer
//! already matches those tables. Live stores the same residual on host
//! `Object` / `GameLogic` scan maps but ObjectSnapshot never wrote
//! cheer/`SPECIAL_CHEERING`, mid-clip scatter unused, HORDE/ENTHUSIASTIC/
//! SUBLIMINAL bits, `safe_occlusion_frame`, `health_box_offset`,
//! `last_fire_frame`, per-object `is_recruitable`, guard/hunt scan clocks,
//! `ignore_collisions_until_frame`, `do_final_position`/`final_position`,
//! `ignored_obstacle_id`, `can_path_through_units`, or `last_command_source`
//! — load snapped the Angry Mob bar to the nexus, recloaked a frame early,
//! unlocked script-locked recruits, re-scanned every guard/hunt unit,
//! re-blocked mid-bump movers, left mid-settle units short of the leftover
//! cell, collided enter/dozer units with the building they should ignore,
//! wedged mid-exit units in the factory hull, and re-armed CommandButtonHunt
//! after a player/script cancel.
//!
//! Append a tagged suffix after the historical v9 contain/producer payload
//! so older decoders ignore the extra bytes. No WorldSnapshot version bump.
//! Restore writes clocks/flags only; it never re-runs create/apply.

use crate::game_logic::HUNT_CMD_FROM_AI;
use crate::game_logic::object::TurretSubState;
use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use serde::{Deserialize, Serialize};

const OXOB_MAGIC: &[u8; 4] = b"OXOB";
const OXOB_VERSION: u32 = 17;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ObjectXferPersistPayload {
    objects: Vec<ObjectXferPersist>,
    script_status: Vec<ObjectScriptStatusPersist>,
    structures: Vec<StructureTopplePersist>,
    damage_info: Vec<ObjectDamageInfoPersist>,
    transport: Vec<TransportMovementPersist>,
    body_history: Vec<body_damage_history_persist::BodyDamageHistoryPersist>,
}

/// V16 retains its exact positional Body-history records.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ObjectXferPersistPayloadV16 {
    objects: Vec<ObjectXferPersist>,
    script_status: Vec<ObjectScriptStatusPersist>,
    structures: Vec<StructureTopplePersist>,
    damage_info: Vec<ObjectDamageInfoPersist>,
    transport: Vec<TransportMovementPersist>,
    body_history: Vec<body_damage_history_persist::BodyDamageHistoryPersistV16>,
}

/// v15 retains its exact transport/body-type positional bincode layout.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ObjectXferPersistPayloadV15 {
    objects: Vec<ObjectXferPersist>,
    script_status: Vec<ObjectScriptStatusPersist>,
    structures: Vec<StructureTopplePersist>,
    damage_info: Vec<ObjectDamageInfoPersist>,
    transport: Vec<TransportMovementPersist>,
}

#[path = "body_damage_history_persist.rs"]
mod body_damage_history_persist;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ObjectXferPersistPayloadV14 {
    objects: Vec<ObjectXferPersist>,
    script_status: Vec<ObjectScriptStatusPersist>,
    structures: Vec<StructureTopplePersist>,
    damage_info: Vec<ObjectDamageInfoPersist>,
}

#[path = "transport_movement_persist.rs"]
mod transport_movement_persist;
use transport_movement_persist::TransportMovementPersist;

/// v13 added canonical topple poses, before body damage-type continuation.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ObjectXferPersistPayloadV13 {
    objects: Vec<ObjectXferPersist>,
    script_status: Vec<ObjectScriptStatusPersist>,
    structures: Vec<StructureTopplePersist>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ObjectDamageInfoPersist {
    object_id: u32,
    last_damage_type: Option<crate::game_logic::combat::DamageType>,
}

/// v12 added only targetable/masked. Its positional layout is retained.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ObjectXferPersistPayloadV12 {
    objects: Vec<ObjectXferPersist>,
    script_status: Vec<ObjectScriptStatusPersist>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StructureTopplePersist {
    object_id: u32,
    matrix: [f32; 16],
    runtime: crate::game_logic::host_structure_topple::HostStructureToppleData,
}

// v11 retains its exact positional bincode layout. Older saves had no host
// representation of these two original Object status values.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ObjectXferPersistPayloadV11 {
    objects: Vec<ObjectXferPersist>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ObjectScriptStatusPersist {
    object_id: u32,
    script_targetable: bool,
    masked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ObjectXferPersistPayloadV1 {
    objects: Vec<ObjectXferPersistV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ObjectXferPersistPayloadV2 {
    objects: Vec<ObjectXferPersistV2>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ObjectXferPersistPayloadV3 {
    objects: Vec<ObjectXferPersistV3>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ObjectXferPersistPayloadV4 {
    objects: Vec<ObjectXferPersistV4>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ObjectXferPersist {
    object_id: u32,
    disabled_held: bool,
    single_use_command_used: bool,
    ai_attitude: i8,
    custom_indicator_color: Option<u32>,
    vision_range: f32,
    shroud_clearing_range: f32,
    shroud_range: f32,
    pre_attack_ready_at: f32,
    pre_attack_target: Option<u32>,
    consecutive_shot_target: Option<u32>,
    max_shots_to_fire: i32,
    turret_angle_deg: f32,
    turret_pitch_deg: f32,
    turret_idle_scan_next_frame: u32,
    turret_idle_scanning: bool,
    turret_idle_scan_desired_angle_deg: f32,
    turret_idle_scan_index: u32,
    turret_holding: bool,
    turret_hold_until_frame: u32,
    turret_idle_recentering: bool,
    turret_mood_target: bool,
    turret_target_id: Option<u32>,
    turret_force_attacking: bool,
    turret_enabled: bool,
    turret_substate: u8,
    turret_rotating: bool,
    temporary_stealth_expires_frame: u32,
    weapon_bonus_solo: u8,
    which_exit_path: u8,
    cheer_timer: f32,
    special_cheering: bool,
    weapon_scatter_targets_unused: [Vec<i32>; 3],
    weapon_scatter_targets_inited: [bool; 3],
    weapon_bonus_horde: bool,
    weapon_bonus_enthusiastic: bool,
    weapon_bonus_subliminal: bool,
    safe_occlusion_frame: u32,
    health_box_offset: [f32; 3],
    last_fire_frame: u32,
    is_recruitable: bool,
    guard_next_enemy_scan: Option<u32>,
    hunt_next_enemy_scan: Option<u32>,
    ignore_collisions_until_frame: u32,
    do_final_position: bool,
    final_position: [f32; 3],
    ignored_obstacle_id: Option<u32>,
    can_path_through_units: bool,
    last_command_source: u32,
    /// C++ `m_upgradedLocomotors`.
    locomotor_upgrade: bool,
    /// C++ `m_curLocomotorSet` token (`SET_NORMAL`, `SET_TAXIING`, ...).
    cur_locomotor_set: Option<String>,
    /// False on upgrades from versions that did not store Locomotor::xfer scalars.
    has_locomotor_motion: bool,
    /// Main's effective braking in host sec²; preserve explicit saved values.
    braking: f32,
    /// C++ `Locomotor::m_maxLift`.
    max_lift: f32,
    /// C++ `Locomotor::m_brakingFactor`.
    braking_factor: f32,
    /// False on saves that did not store close-enough distance or preferred height.
    has_locomotor_arrival: bool,
    /// C++ `Locomotor::m_closeEnoughDist` (None = leave the create-time value).
    close_enough_dist: Option<f32>,
    /// C++ close-enough-is-3D flag.
    close_enough_dist_3d: bool,
    /// C++ `Locomotor::m_donutTimer`.
    donut_timer: u32,
    /// C++ `Locomotor::m_preferredHeight`.
    preferred_height: f32,
    /// C++ `Locomotor::m_preferredHeightDamping`.
    preferred_height_damping: f32,
    /// False on saves that did not store `m_flags`, maintain position, or wander offset.
    has_locomotor_pose: bool,
    /// C++ `Locomotor::m_flags` (`LocoFlag` bit order).
    loco_flags: u32,
    /// C++ `Locomotor::m_maintainPos` (None = invalid / unset).
    maintain_pos: Option<[f32; 3]>,
    /// C++ `Locomotor::m_angleOffset`.
    wander_angle_offset: f32,
    /// C++ `Locomotor::m_offsetIncrement`.
    wander_offset_increment: f32,
    /// False on saves that did not store the active locomotor template name.
    has_locomotor_template: bool,
    /// C++ `Locomotor::getTemplateName`, reloaded by `LocomotorSet::xfer`.
    cur_locomotor_name: Option<String>,
    /// C++ `LocomotorSet::m_validLocomotorSurfaces`.
    valid_locomotor_surfaces: u32,
    /// C++ `LocomotorSet::m_downhillOnly`.
    downhill_only: bool,
    /// C++ doQuickExit temporary-state frame. Absent before OXOB version 11.
    #[serde(default)]
    quick_exit_until: Option<u32>,
}

/// OXOB version 10: locomotor template, no quick-exit deadline.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ObjectXferPersistV10 {
    object_id: u32,
    disabled_held: bool,
    single_use_command_used: bool,
    ai_attitude: i8,
    custom_indicator_color: Option<u32>,
    vision_range: f32,
    shroud_clearing_range: f32,
    shroud_range: f32,
    pre_attack_ready_at: f32,
    pre_attack_target: Option<u32>,
    consecutive_shot_target: Option<u32>,
    max_shots_to_fire: i32,
    turret_angle_deg: f32,
    turret_pitch_deg: f32,
    turret_idle_scan_next_frame: u32,
    turret_idle_scanning: bool,
    turret_idle_scan_desired_angle_deg: f32,
    turret_idle_scan_index: u32,
    turret_holding: bool,
    turret_hold_until_frame: u32,
    turret_idle_recentering: bool,
    turret_mood_target: bool,
    turret_target_id: Option<u32>,
    turret_force_attacking: bool,
    turret_enabled: bool,
    turret_substate: u8,
    turret_rotating: bool,
    temporary_stealth_expires_frame: u32,
    weapon_bonus_solo: u8,
    which_exit_path: u8,
    cheer_timer: f32,
    special_cheering: bool,
    weapon_scatter_targets_unused: [Vec<i32>; 3],
    weapon_scatter_targets_inited: [bool; 3],
    weapon_bonus_horde: bool,
    weapon_bonus_enthusiastic: bool,
    weapon_bonus_subliminal: bool,
    safe_occlusion_frame: u32,
    health_box_offset: [f32; 3],
    last_fire_frame: u32,
    is_recruitable: bool,
    guard_next_enemy_scan: Option<u32>,
    hunt_next_enemy_scan: Option<u32>,
    ignore_collisions_until_frame: u32,
    do_final_position: bool,
    final_position: [f32; 3],
    ignored_obstacle_id: Option<u32>,
    can_path_through_units: bool,
    last_command_source: u32,
    locomotor_upgrade: bool,
    cur_locomotor_set: Option<String>,
    has_locomotor_motion: bool,
    braking: f32,
    max_lift: f32,
    braking_factor: f32,
    has_locomotor_arrival: bool,
    close_enough_dist: Option<f32>,
    close_enough_dist_3d: bool,
    donut_timer: u32,
    preferred_height: f32,
    preferred_height_damping: f32,
    has_locomotor_pose: bool,
    loco_flags: u32,
    maintain_pos: Option<[f32; 3]>,
    wander_angle_offset: f32,
    wander_offset_increment: f32,
    has_locomotor_template: bool,
    cur_locomotor_name: Option<String>,
    valid_locomotor_surfaces: u32,
    downhill_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ObjectXferPersistPayloadV10 {
    objects: Vec<ObjectXferPersistV10>,
}

impl From<ObjectXferPersistV10> for ObjectXferPersist {
    fn from(v: ObjectXferPersistV10) -> Self {
        Self {
            object_id: v.object_id,
            disabled_held: v.disabled_held,
            single_use_command_used: v.single_use_command_used,
            ai_attitude: v.ai_attitude,
            custom_indicator_color: v.custom_indicator_color,
            vision_range: v.vision_range,
            shroud_clearing_range: v.shroud_clearing_range,
            shroud_range: v.shroud_range,
            pre_attack_ready_at: v.pre_attack_ready_at,
            pre_attack_target: v.pre_attack_target,
            consecutive_shot_target: v.consecutive_shot_target,
            max_shots_to_fire: v.max_shots_to_fire,
            turret_angle_deg: v.turret_angle_deg,
            turret_pitch_deg: v.turret_pitch_deg,
            turret_idle_scan_next_frame: v.turret_idle_scan_next_frame,
            turret_idle_scanning: v.turret_idle_scanning,
            turret_idle_scan_desired_angle_deg: v.turret_idle_scan_desired_angle_deg,
            turret_idle_scan_index: v.turret_idle_scan_index,
            turret_holding: v.turret_holding,
            turret_hold_until_frame: v.turret_hold_until_frame,
            turret_idle_recentering: v.turret_idle_recentering,
            turret_mood_target: v.turret_mood_target,
            turret_target_id: v.turret_target_id,
            turret_force_attacking: v.turret_force_attacking,
            turret_enabled: v.turret_enabled,
            turret_substate: v.turret_substate,
            turret_rotating: v.turret_rotating,
            temporary_stealth_expires_frame: v.temporary_stealth_expires_frame,
            weapon_bonus_solo: v.weapon_bonus_solo,
            which_exit_path: v.which_exit_path,
            cheer_timer: v.cheer_timer,
            special_cheering: v.special_cheering,
            weapon_scatter_targets_unused: v.weapon_scatter_targets_unused,
            weapon_scatter_targets_inited: v.weapon_scatter_targets_inited,
            weapon_bonus_horde: v.weapon_bonus_horde,
            weapon_bonus_enthusiastic: v.weapon_bonus_enthusiastic,
            weapon_bonus_subliminal: v.weapon_bonus_subliminal,
            safe_occlusion_frame: v.safe_occlusion_frame,
            health_box_offset: v.health_box_offset,
            last_fire_frame: v.last_fire_frame,
            is_recruitable: v.is_recruitable,
            guard_next_enemy_scan: v.guard_next_enemy_scan,
            hunt_next_enemy_scan: v.hunt_next_enemy_scan,
            ignore_collisions_until_frame: v.ignore_collisions_until_frame,
            do_final_position: v.do_final_position,
            final_position: v.final_position,
            ignored_obstacle_id: v.ignored_obstacle_id,
            can_path_through_units: v.can_path_through_units,
            last_command_source: v.last_command_source,
            locomotor_upgrade: v.locomotor_upgrade,
            cur_locomotor_set: v.cur_locomotor_set,
            has_locomotor_motion: v.has_locomotor_motion,
            braking: v.braking,
            max_lift: v.max_lift,
            braking_factor: v.braking_factor,
            has_locomotor_arrival: v.has_locomotor_arrival,
            close_enough_dist: v.close_enough_dist,
            close_enough_dist_3d: v.close_enough_dist_3d,
            donut_timer: v.donut_timer,
            preferred_height: v.preferred_height,
            preferred_height_damping: v.preferred_height_damping,
            has_locomotor_pose: v.has_locomotor_pose,
            loco_flags: v.loco_flags,
            maintain_pos: v.maintain_pos,
            wander_angle_offset: v.wander_angle_offset,
            wander_offset_increment: v.wander_offset_increment,
            has_locomotor_template: v.has_locomotor_template,
            cur_locomotor_name: v.cur_locomotor_name,
            valid_locomotor_surfaces: v.valid_locomotor_surfaces,
            downhill_only: v.downhill_only,
            quick_exit_until: None,
        }
    }
}

/// OXOB version 6: locomotor token, no braking scalars.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ObjectXferPersistV6 {
    object_id: u32,
    disabled_held: bool,
    single_use_command_used: bool,
    ai_attitude: i8,
    custom_indicator_color: Option<u32>,
    vision_range: f32,
    shroud_clearing_range: f32,
    shroud_range: f32,
    pre_attack_ready_at: f32,
    pre_attack_target: Option<u32>,
    consecutive_shot_target: Option<u32>,
    max_shots_to_fire: i32,
    turret_angle_deg: f32,
    turret_pitch_deg: f32,
    turret_idle_scan_next_frame: u32,
    turret_idle_scanning: bool,
    turret_idle_scan_desired_angle_deg: f32,
    turret_idle_scan_index: u32,
    turret_holding: bool,
    turret_hold_until_frame: u32,
    turret_idle_recentering: bool,
    turret_mood_target: bool,
    turret_target_id: Option<u32>,
    turret_force_attacking: bool,
    turret_enabled: bool,
    turret_substate: u8,
    turret_rotating: bool,
    temporary_stealth_expires_frame: u32,
    weapon_bonus_solo: u8,
    which_exit_path: u8,
    cheer_timer: f32,
    special_cheering: bool,
    weapon_scatter_targets_unused: [Vec<i32>; 3],
    weapon_scatter_targets_inited: [bool; 3],
    weapon_bonus_horde: bool,
    weapon_bonus_enthusiastic: bool,
    weapon_bonus_subliminal: bool,
    safe_occlusion_frame: u32,
    health_box_offset: [f32; 3],
    last_fire_frame: u32,
    is_recruitable: bool,
    guard_next_enemy_scan: Option<u32>,
    hunt_next_enemy_scan: Option<u32>,
    ignore_collisions_until_frame: u32,
    do_final_position: bool,
    final_position: [f32; 3],
    ignored_obstacle_id: Option<u32>,
    can_path_through_units: bool,
    last_command_source: u32,
    locomotor_upgrade: bool,
    cur_locomotor_set: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ObjectXferPersistPayloadV6 {
    objects: Vec<ObjectXferPersistV6>,
}

/// OXOB version 7: braking scalars, no close-enough distance.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ObjectXferPersistV7 {
    object_id: u32,
    disabled_held: bool,
    single_use_command_used: bool,
    ai_attitude: i8,
    custom_indicator_color: Option<u32>,
    vision_range: f32,
    shroud_clearing_range: f32,
    shroud_range: f32,
    pre_attack_ready_at: f32,
    pre_attack_target: Option<u32>,
    consecutive_shot_target: Option<u32>,
    max_shots_to_fire: i32,
    turret_angle_deg: f32,
    turret_pitch_deg: f32,
    turret_idle_scan_next_frame: u32,
    turret_idle_scanning: bool,
    turret_idle_scan_desired_angle_deg: f32,
    turret_idle_scan_index: u32,
    turret_holding: bool,
    turret_hold_until_frame: u32,
    turret_idle_recentering: bool,
    turret_mood_target: bool,
    turret_target_id: Option<u32>,
    turret_force_attacking: bool,
    turret_enabled: bool,
    turret_substate: u8,
    turret_rotating: bool,
    temporary_stealth_expires_frame: u32,
    weapon_bonus_solo: u8,
    which_exit_path: u8,
    cheer_timer: f32,
    special_cheering: bool,
    weapon_scatter_targets_unused: [Vec<i32>; 3],
    weapon_scatter_targets_inited: [bool; 3],
    weapon_bonus_horde: bool,
    weapon_bonus_enthusiastic: bool,
    weapon_bonus_subliminal: bool,
    safe_occlusion_frame: u32,
    health_box_offset: [f32; 3],
    last_fire_frame: u32,
    is_recruitable: bool,
    guard_next_enemy_scan: Option<u32>,
    hunt_next_enemy_scan: Option<u32>,
    ignore_collisions_until_frame: u32,
    do_final_position: bool,
    final_position: [f32; 3],
    ignored_obstacle_id: Option<u32>,
    can_path_through_units: bool,
    last_command_source: u32,
    locomotor_upgrade: bool,
    cur_locomotor_set: Option<String>,
    has_locomotor_motion: bool,
    braking: f32,
    max_lift: f32,
    braking_factor: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ObjectXferPersistPayloadV7 {
    objects: Vec<ObjectXferPersistV7>,
}

/// OXOB version 8: stopping distance, no flag word.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ObjectXferPersistV8 {
    object_id: u32,
    disabled_held: bool,
    single_use_command_used: bool,
    ai_attitude: i8,
    custom_indicator_color: Option<u32>,
    vision_range: f32,
    shroud_clearing_range: f32,
    shroud_range: f32,
    pre_attack_ready_at: f32,
    pre_attack_target: Option<u32>,
    consecutive_shot_target: Option<u32>,
    max_shots_to_fire: i32,
    turret_angle_deg: f32,
    turret_pitch_deg: f32,
    turret_idle_scan_next_frame: u32,
    turret_idle_scanning: bool,
    turret_idle_scan_desired_angle_deg: f32,
    turret_idle_scan_index: u32,
    turret_holding: bool,
    turret_hold_until_frame: u32,
    turret_idle_recentering: bool,
    turret_mood_target: bool,
    turret_target_id: Option<u32>,
    turret_force_attacking: bool,
    turret_enabled: bool,
    turret_substate: u8,
    turret_rotating: bool,
    temporary_stealth_expires_frame: u32,
    weapon_bonus_solo: u8,
    which_exit_path: u8,
    cheer_timer: f32,
    special_cheering: bool,
    weapon_scatter_targets_unused: [Vec<i32>; 3],
    weapon_scatter_targets_inited: [bool; 3],
    weapon_bonus_horde: bool,
    weapon_bonus_enthusiastic: bool,
    weapon_bonus_subliminal: bool,
    safe_occlusion_frame: u32,
    health_box_offset: [f32; 3],
    last_fire_frame: u32,
    is_recruitable: bool,
    guard_next_enemy_scan: Option<u32>,
    hunt_next_enemy_scan: Option<u32>,
    ignore_collisions_until_frame: u32,
    do_final_position: bool,
    final_position: [f32; 3],
    ignored_obstacle_id: Option<u32>,
    can_path_through_units: bool,
    last_command_source: u32,
    locomotor_upgrade: bool,
    cur_locomotor_set: Option<String>,
    has_locomotor_motion: bool,
    braking: f32,
    max_lift: f32,
    braking_factor: f32,
    has_locomotor_arrival: bool,
    close_enough_dist: Option<f32>,
    close_enough_dist_3d: bool,
    donut_timer: u32,
    preferred_height: f32,
    preferred_height_damping: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ObjectXferPersistPayloadV8 {
    objects: Vec<ObjectXferPersistV8>,
}

/// OXOB version 9: flag word and wander pose, no surface mask.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ObjectXferPersistV9 {
    object_id: u32,
    disabled_held: bool,
    single_use_command_used: bool,
    ai_attitude: i8,
    custom_indicator_color: Option<u32>,
    vision_range: f32,
    shroud_clearing_range: f32,
    shroud_range: f32,
    pre_attack_ready_at: f32,
    pre_attack_target: Option<u32>,
    consecutive_shot_target: Option<u32>,
    max_shots_to_fire: i32,
    turret_angle_deg: f32,
    turret_pitch_deg: f32,
    turret_idle_scan_next_frame: u32,
    turret_idle_scanning: bool,
    turret_idle_scan_desired_angle_deg: f32,
    turret_idle_scan_index: u32,
    turret_holding: bool,
    turret_hold_until_frame: u32,
    turret_idle_recentering: bool,
    turret_mood_target: bool,
    turret_target_id: Option<u32>,
    turret_force_attacking: bool,
    turret_enabled: bool,
    turret_substate: u8,
    turret_rotating: bool,
    temporary_stealth_expires_frame: u32,
    weapon_bonus_solo: u8,
    which_exit_path: u8,
    cheer_timer: f32,
    special_cheering: bool,
    weapon_scatter_targets_unused: [Vec<i32>; 3],
    weapon_scatter_targets_inited: [bool; 3],
    weapon_bonus_horde: bool,
    weapon_bonus_enthusiastic: bool,
    weapon_bonus_subliminal: bool,
    safe_occlusion_frame: u32,
    health_box_offset: [f32; 3],
    last_fire_frame: u32,
    is_recruitable: bool,
    guard_next_enemy_scan: Option<u32>,
    hunt_next_enemy_scan: Option<u32>,
    ignore_collisions_until_frame: u32,
    do_final_position: bool,
    final_position: [f32; 3],
    ignored_obstacle_id: Option<u32>,
    can_path_through_units: bool,
    last_command_source: u32,
    locomotor_upgrade: bool,
    cur_locomotor_set: Option<String>,
    has_locomotor_motion: bool,
    braking: f32,
    max_lift: f32,
    braking_factor: f32,
    has_locomotor_arrival: bool,
    close_enough_dist: Option<f32>,
    close_enough_dist_3d: bool,
    donut_timer: u32,
    preferred_height: f32,
    preferred_height_damping: f32,
    has_locomotor_pose: bool,
    loco_flags: u32,
    maintain_pos: Option<[f32; 3]>,
    wander_angle_offset: f32,
    wander_offset_increment: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ObjectXferPersistPayloadV9 {
    objects: Vec<ObjectXferPersistV9>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ObjectXferPersistV1 {
    object_id: u32,
    disabled_held: bool,
    single_use_command_used: bool,
    ai_attitude: i8,
    custom_indicator_color: Option<u32>,
    vision_range: f32,
    shroud_clearing_range: f32,
    shroud_range: f32,
    pre_attack_ready_at: f32,
    pre_attack_target: Option<u32>,
    consecutive_shot_target: Option<u32>,
    max_shots_to_fire: i32,
    turret_angle_deg: f32,
    turret_pitch_deg: f32,
    turret_idle_scan_next_frame: u32,
    turret_idle_scanning: bool,
    turret_idle_scan_desired_angle_deg: f32,
    turret_idle_scan_index: u32,
    turret_holding: bool,
    turret_hold_until_frame: u32,
    turret_idle_recentering: bool,
    turret_mood_target: bool,
    turret_target_id: Option<u32>,
    turret_force_attacking: bool,
    turret_enabled: bool,
    turret_substate: u8,
    turret_rotating: bool,
    temporary_stealth_expires_frame: u32,
    weapon_bonus_solo: u8,
    which_exit_path: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ObjectXferPersistV2 {
    object_id: u32,
    disabled_held: bool,
    single_use_command_used: bool,
    ai_attitude: i8,
    custom_indicator_color: Option<u32>,
    vision_range: f32,
    shroud_clearing_range: f32,
    shroud_range: f32,
    pre_attack_ready_at: f32,
    pre_attack_target: Option<u32>,
    consecutive_shot_target: Option<u32>,
    max_shots_to_fire: i32,
    turret_angle_deg: f32,
    turret_pitch_deg: f32,
    turret_idle_scan_next_frame: u32,
    turret_idle_scanning: bool,
    turret_idle_scan_desired_angle_deg: f32,
    turret_idle_scan_index: u32,
    turret_holding: bool,
    turret_hold_until_frame: u32,
    turret_idle_recentering: bool,
    turret_mood_target: bool,
    turret_target_id: Option<u32>,
    turret_force_attacking: bool,
    turret_enabled: bool,
    turret_substate: u8,
    turret_rotating: bool,
    temporary_stealth_expires_frame: u32,
    weapon_bonus_solo: u8,
    which_exit_path: u8,
    cheer_timer: f32,
    special_cheering: bool,
    weapon_scatter_targets_unused: [Vec<i32>; 3],
    weapon_scatter_targets_inited: [bool; 3],
    weapon_bonus_horde: bool,
    weapon_bonus_enthusiastic: bool,
    weapon_bonus_subliminal: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ObjectXferPersistV3 {
    object_id: u32,
    disabled_held: bool,
    single_use_command_used: bool,
    ai_attitude: i8,
    custom_indicator_color: Option<u32>,
    vision_range: f32,
    shroud_clearing_range: f32,
    shroud_range: f32,
    pre_attack_ready_at: f32,
    pre_attack_target: Option<u32>,
    consecutive_shot_target: Option<u32>,
    max_shots_to_fire: i32,
    turret_angle_deg: f32,
    turret_pitch_deg: f32,
    turret_idle_scan_next_frame: u32,
    turret_idle_scanning: bool,
    turret_idle_scan_desired_angle_deg: f32,
    turret_idle_scan_index: u32,
    turret_holding: bool,
    turret_hold_until_frame: u32,
    turret_idle_recentering: bool,
    turret_mood_target: bool,
    turret_target_id: Option<u32>,
    turret_force_attacking: bool,
    turret_enabled: bool,
    turret_substate: u8,
    turret_rotating: bool,
    temporary_stealth_expires_frame: u32,
    weapon_bonus_solo: u8,
    which_exit_path: u8,
    cheer_timer: f32,
    special_cheering: bool,
    weapon_scatter_targets_unused: [Vec<i32>; 3],
    weapon_scatter_targets_inited: [bool; 3],
    weapon_bonus_horde: bool,
    weapon_bonus_enthusiastic: bool,
    weapon_bonus_subliminal: bool,
    safe_occlusion_frame: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ObjectXferPersistV4 {
    object_id: u32,
    disabled_held: bool,
    single_use_command_used: bool,
    ai_attitude: i8,
    custom_indicator_color: Option<u32>,
    vision_range: f32,
    shroud_clearing_range: f32,
    shroud_range: f32,
    pre_attack_ready_at: f32,
    pre_attack_target: Option<u32>,
    consecutive_shot_target: Option<u32>,
    max_shots_to_fire: i32,
    turret_angle_deg: f32,
    turret_pitch_deg: f32,
    turret_idle_scan_next_frame: u32,
    turret_idle_scanning: bool,
    turret_idle_scan_desired_angle_deg: f32,
    turret_idle_scan_index: u32,
    turret_holding: bool,
    turret_hold_until_frame: u32,
    turret_idle_recentering: bool,
    turret_mood_target: bool,
    turret_target_id: Option<u32>,
    turret_force_attacking: bool,
    turret_enabled: bool,
    turret_substate: u8,
    turret_rotating: bool,
    temporary_stealth_expires_frame: u32,
    weapon_bonus_solo: u8,
    which_exit_path: u8,
    cheer_timer: f32,
    special_cheering: bool,
    weapon_scatter_targets_unused: [Vec<i32>; 3],
    weapon_scatter_targets_inited: [bool; 3],
    weapon_bonus_horde: bool,
    weapon_bonus_enthusiastic: bool,
    weapon_bonus_subliminal: bool,
    safe_occlusion_frame: u32,
    health_box_offset: [f32; 3],
    last_fire_frame: u32,
    is_recruitable: bool,
    guard_next_enemy_scan: Option<u32>,
    hunt_next_enemy_scan: Option<u32>,
    ignore_collisions_until_frame: u32,
}

/// OXOB version 5: command source, no locomotor upgrade or set.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ObjectXferPersistV5 {
    object_id: u32,
    disabled_held: bool,
    single_use_command_used: bool,
    ai_attitude: i8,
    custom_indicator_color: Option<u32>,
    vision_range: f32,
    shroud_clearing_range: f32,
    shroud_range: f32,
    pre_attack_ready_at: f32,
    pre_attack_target: Option<u32>,
    consecutive_shot_target: Option<u32>,
    max_shots_to_fire: i32,
    turret_angle_deg: f32,
    turret_pitch_deg: f32,
    turret_idle_scan_next_frame: u32,
    turret_idle_scanning: bool,
    turret_idle_scan_desired_angle_deg: f32,
    turret_idle_scan_index: u32,
    turret_holding: bool,
    turret_hold_until_frame: u32,
    turret_idle_recentering: bool,
    turret_mood_target: bool,
    turret_target_id: Option<u32>,
    turret_force_attacking: bool,
    turret_enabled: bool,
    turret_substate: u8,
    turret_rotating: bool,
    temporary_stealth_expires_frame: u32,
    weapon_bonus_solo: u8,
    which_exit_path: u8,
    cheer_timer: f32,
    special_cheering: bool,
    weapon_scatter_targets_unused: [Vec<i32>; 3],
    weapon_scatter_targets_inited: [bool; 3],
    weapon_bonus_horde: bool,
    weapon_bonus_enthusiastic: bool,
    weapon_bonus_subliminal: bool,
    safe_occlusion_frame: u32,
    health_box_offset: [f32; 3],
    last_fire_frame: u32,
    is_recruitable: bool,
    guard_next_enemy_scan: Option<u32>,
    hunt_next_enemy_scan: Option<u32>,
    ignore_collisions_until_frame: u32,
    do_final_position: bool,
    final_position: [f32; 3],
    ignored_obstacle_id: Option<u32>,
    can_path_through_units: bool,
    last_command_source: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ObjectXferPersistPayloadV5 {
    objects: Vec<ObjectXferPersistV5>,
}

impl From<ObjectXferPersistV1> for ObjectXferPersist {
    fn from(v1: ObjectXferPersistV1) -> Self {
        Self {
            object_id: v1.object_id,
            disabled_held: v1.disabled_held,
            single_use_command_used: v1.single_use_command_used,
            ai_attitude: v1.ai_attitude,
            custom_indicator_color: v1.custom_indicator_color,
            vision_range: v1.vision_range,
            shroud_clearing_range: v1.shroud_clearing_range,
            shroud_range: v1.shroud_range,
            pre_attack_ready_at: v1.pre_attack_ready_at,
            pre_attack_target: v1.pre_attack_target,
            consecutive_shot_target: v1.consecutive_shot_target,
            max_shots_to_fire: v1.max_shots_to_fire,
            turret_angle_deg: v1.turret_angle_deg,
            turret_pitch_deg: v1.turret_pitch_deg,
            turret_idle_scan_next_frame: v1.turret_idle_scan_next_frame,
            turret_idle_scanning: v1.turret_idle_scanning,
            turret_idle_scan_desired_angle_deg: v1.turret_idle_scan_desired_angle_deg,
            turret_idle_scan_index: v1.turret_idle_scan_index,
            turret_holding: v1.turret_holding,
            turret_hold_until_frame: v1.turret_hold_until_frame,
            turret_idle_recentering: v1.turret_idle_recentering,
            turret_mood_target: v1.turret_mood_target,
            turret_target_id: v1.turret_target_id,
            turret_force_attacking: v1.turret_force_attacking,
            turret_enabled: v1.turret_enabled,
            turret_substate: v1.turret_substate,
            turret_rotating: v1.turret_rotating,
            temporary_stealth_expires_frame: v1.temporary_stealth_expires_frame,
            weapon_bonus_solo: v1.weapon_bonus_solo,
            which_exit_path: v1.which_exit_path,
            cheer_timer: 0.0,
            special_cheering: false,
            weapon_scatter_targets_unused: Default::default(),
            weapon_scatter_targets_inited: [false; 3],
            weapon_bonus_horde: false,
            weapon_bonus_enthusiastic: false,
            weapon_bonus_subliminal: false,
            safe_occlusion_frame: 0,
            health_box_offset: [0.0; 3],
            last_fire_frame: 0,
            is_recruitable: true,
            guard_next_enemy_scan: None,
            quick_exit_until: None,
            hunt_next_enemy_scan: None,
            ignore_collisions_until_frame: 0,
            do_final_position: false,
            final_position: [0.0; 3],
            ignored_obstacle_id: None,
            can_path_through_units: false,
            last_command_source: HUNT_CMD_FROM_AI,
            locomotor_upgrade: false,
            cur_locomotor_set: None,
            has_locomotor_motion: false,
            braking: 0.0,
            max_lift: 0.0,
            braking_factor: 0.0,
            has_locomotor_arrival: false,
            close_enough_dist: None,
            close_enough_dist_3d: false,
            donut_timer: 0,
            preferred_height: 0.0,
            preferred_height_damping: 0.0,
            has_locomotor_pose: false,
            loco_flags: 0,
            maintain_pos: None,
            wander_angle_offset: 0.0,
            wander_offset_increment: 0.0,
            has_locomotor_template: false,
            cur_locomotor_name: None,
            valid_locomotor_surfaces: 0,
            downhill_only: false,
        }
    }
}

impl From<ObjectXferPersistV2> for ObjectXferPersist {
    fn from(v2: ObjectXferPersistV2) -> Self {
        Self {
            object_id: v2.object_id,
            disabled_held: v2.disabled_held,
            single_use_command_used: v2.single_use_command_used,
            ai_attitude: v2.ai_attitude,
            custom_indicator_color: v2.custom_indicator_color,
            vision_range: v2.vision_range,
            shroud_clearing_range: v2.shroud_clearing_range,
            shroud_range: v2.shroud_range,
            pre_attack_ready_at: v2.pre_attack_ready_at,
            pre_attack_target: v2.pre_attack_target,
            consecutive_shot_target: v2.consecutive_shot_target,
            max_shots_to_fire: v2.max_shots_to_fire,
            turret_angle_deg: v2.turret_angle_deg,
            turret_pitch_deg: v2.turret_pitch_deg,
            turret_idle_scan_next_frame: v2.turret_idle_scan_next_frame,
            turret_idle_scanning: v2.turret_idle_scanning,
            turret_idle_scan_desired_angle_deg: v2.turret_idle_scan_desired_angle_deg,
            turret_idle_scan_index: v2.turret_idle_scan_index,
            turret_holding: v2.turret_holding,
            turret_hold_until_frame: v2.turret_hold_until_frame,
            turret_idle_recentering: v2.turret_idle_recentering,
            turret_mood_target: v2.turret_mood_target,
            turret_target_id: v2.turret_target_id,
            turret_force_attacking: v2.turret_force_attacking,
            turret_enabled: v2.turret_enabled,
            turret_substate: v2.turret_substate,
            turret_rotating: v2.turret_rotating,
            temporary_stealth_expires_frame: v2.temporary_stealth_expires_frame,
            weapon_bonus_solo: v2.weapon_bonus_solo,
            which_exit_path: v2.which_exit_path,
            cheer_timer: v2.cheer_timer,
            special_cheering: v2.special_cheering,
            weapon_scatter_targets_unused: v2.weapon_scatter_targets_unused,
            weapon_scatter_targets_inited: v2.weapon_scatter_targets_inited,
            weapon_bonus_horde: v2.weapon_bonus_horde,
            weapon_bonus_enthusiastic: v2.weapon_bonus_enthusiastic,
            weapon_bonus_subliminal: v2.weapon_bonus_subliminal,
            safe_occlusion_frame: 0,
            health_box_offset: [0.0; 3],
            last_fire_frame: 0,
            is_recruitable: true,
            guard_next_enemy_scan: None,
            quick_exit_until: None,
            hunt_next_enemy_scan: None,
            ignore_collisions_until_frame: 0,
            do_final_position: false,
            final_position: [0.0; 3],
            ignored_obstacle_id: None,
            can_path_through_units: false,
            last_command_source: HUNT_CMD_FROM_AI,
            locomotor_upgrade: false,
            cur_locomotor_set: None,
            has_locomotor_motion: false,
            braking: 0.0,
            max_lift: 0.0,
            braking_factor: 0.0,
            has_locomotor_arrival: false,
            close_enough_dist: None,
            close_enough_dist_3d: false,
            donut_timer: 0,
            preferred_height: 0.0,
            preferred_height_damping: 0.0,
            has_locomotor_pose: false,
            loco_flags: 0,
            maintain_pos: None,
            wander_angle_offset: 0.0,
            wander_offset_increment: 0.0,
            has_locomotor_template: false,
            cur_locomotor_name: None,
            valid_locomotor_surfaces: 0,
            downhill_only: false,
        }
    }
}

impl From<ObjectXferPersistV3> for ObjectXferPersist {
    fn from(v3: ObjectXferPersistV3) -> Self {
        Self {
            object_id: v3.object_id,
            disabled_held: v3.disabled_held,
            single_use_command_used: v3.single_use_command_used,
            ai_attitude: v3.ai_attitude,
            custom_indicator_color: v3.custom_indicator_color,
            vision_range: v3.vision_range,
            shroud_clearing_range: v3.shroud_clearing_range,
            shroud_range: v3.shroud_range,
            pre_attack_ready_at: v3.pre_attack_ready_at,
            pre_attack_target: v3.pre_attack_target,
            consecutive_shot_target: v3.consecutive_shot_target,
            max_shots_to_fire: v3.max_shots_to_fire,
            turret_angle_deg: v3.turret_angle_deg,
            turret_pitch_deg: v3.turret_pitch_deg,
            turret_idle_scan_next_frame: v3.turret_idle_scan_next_frame,
            turret_idle_scanning: v3.turret_idle_scanning,
            turret_idle_scan_desired_angle_deg: v3.turret_idle_scan_desired_angle_deg,
            turret_idle_scan_index: v3.turret_idle_scan_index,
            turret_holding: v3.turret_holding,
            turret_hold_until_frame: v3.turret_hold_until_frame,
            turret_idle_recentering: v3.turret_idle_recentering,
            turret_mood_target: v3.turret_mood_target,
            turret_target_id: v3.turret_target_id,
            turret_force_attacking: v3.turret_force_attacking,
            turret_enabled: v3.turret_enabled,
            turret_substate: v3.turret_substate,
            turret_rotating: v3.turret_rotating,
            temporary_stealth_expires_frame: v3.temporary_stealth_expires_frame,
            weapon_bonus_solo: v3.weapon_bonus_solo,
            which_exit_path: v3.which_exit_path,
            cheer_timer: v3.cheer_timer,
            special_cheering: v3.special_cheering,
            weapon_scatter_targets_unused: v3.weapon_scatter_targets_unused,
            weapon_scatter_targets_inited: v3.weapon_scatter_targets_inited,
            weapon_bonus_horde: v3.weapon_bonus_horde,
            weapon_bonus_enthusiastic: v3.weapon_bonus_enthusiastic,
            weapon_bonus_subliminal: v3.weapon_bonus_subliminal,
            safe_occlusion_frame: v3.safe_occlusion_frame,
            health_box_offset: [0.0; 3],
            last_fire_frame: 0,
            is_recruitable: true,
            guard_next_enemy_scan: None,
            quick_exit_until: None,
            hunt_next_enemy_scan: None,
            ignore_collisions_until_frame: 0,
            do_final_position: false,
            final_position: [0.0; 3],
            ignored_obstacle_id: None,
            can_path_through_units: false,
            last_command_source: HUNT_CMD_FROM_AI,
            locomotor_upgrade: false,
            cur_locomotor_set: None,
            has_locomotor_motion: false,
            braking: 0.0,
            max_lift: 0.0,
            braking_factor: 0.0,
            has_locomotor_arrival: false,
            close_enough_dist: None,
            close_enough_dist_3d: false,
            donut_timer: 0,
            preferred_height: 0.0,
            preferred_height_damping: 0.0,
            has_locomotor_pose: false,
            loco_flags: 0,
            maintain_pos: None,
            wander_angle_offset: 0.0,
            wander_offset_increment: 0.0,
            has_locomotor_template: false,
            cur_locomotor_name: None,
            valid_locomotor_surfaces: 0,
            downhill_only: false,
        }
    }
}

impl From<ObjectXferPersistV4> for ObjectXferPersist {
    fn from(v4: ObjectXferPersistV4) -> Self {
        Self {
            object_id: v4.object_id,
            disabled_held: v4.disabled_held,
            single_use_command_used: v4.single_use_command_used,
            ai_attitude: v4.ai_attitude,
            custom_indicator_color: v4.custom_indicator_color,
            vision_range: v4.vision_range,
            shroud_clearing_range: v4.shroud_clearing_range,
            shroud_range: v4.shroud_range,
            pre_attack_ready_at: v4.pre_attack_ready_at,
            pre_attack_target: v4.pre_attack_target,
            consecutive_shot_target: v4.consecutive_shot_target,
            max_shots_to_fire: v4.max_shots_to_fire,
            turret_angle_deg: v4.turret_angle_deg,
            turret_pitch_deg: v4.turret_pitch_deg,
            turret_idle_scan_next_frame: v4.turret_idle_scan_next_frame,
            turret_idle_scanning: v4.turret_idle_scanning,
            turret_idle_scan_desired_angle_deg: v4.turret_idle_scan_desired_angle_deg,
            turret_idle_scan_index: v4.turret_idle_scan_index,
            turret_holding: v4.turret_holding,
            turret_hold_until_frame: v4.turret_hold_until_frame,
            turret_idle_recentering: v4.turret_idle_recentering,
            turret_mood_target: v4.turret_mood_target,
            turret_target_id: v4.turret_target_id,
            turret_force_attacking: v4.turret_force_attacking,
            turret_enabled: v4.turret_enabled,
            turret_substate: v4.turret_substate,
            turret_rotating: v4.turret_rotating,
            temporary_stealth_expires_frame: v4.temporary_stealth_expires_frame,
            weapon_bonus_solo: v4.weapon_bonus_solo,
            which_exit_path: v4.which_exit_path,
            cheer_timer: v4.cheer_timer,
            special_cheering: v4.special_cheering,
            weapon_scatter_targets_unused: v4.weapon_scatter_targets_unused,
            weapon_scatter_targets_inited: v4.weapon_scatter_targets_inited,
            weapon_bonus_horde: v4.weapon_bonus_horde,
            weapon_bonus_enthusiastic: v4.weapon_bonus_enthusiastic,
            weapon_bonus_subliminal: v4.weapon_bonus_subliminal,
            safe_occlusion_frame: v4.safe_occlusion_frame,
            health_box_offset: v4.health_box_offset,
            last_fire_frame: v4.last_fire_frame,
            is_recruitable: v4.is_recruitable,
            guard_next_enemy_scan: v4.guard_next_enemy_scan,
            quick_exit_until: None,
            hunt_next_enemy_scan: v4.hunt_next_enemy_scan,
            ignore_collisions_until_frame: v4.ignore_collisions_until_frame,
            do_final_position: false,
            final_position: [0.0; 3],
            ignored_obstacle_id: None,
            can_path_through_units: false,
            last_command_source: HUNT_CMD_FROM_AI,
            locomotor_upgrade: false,
            cur_locomotor_set: None,
            has_locomotor_motion: false,
            braking: 0.0,
            max_lift: 0.0,
            braking_factor: 0.0,
            has_locomotor_arrival: false,
            close_enough_dist: None,
            close_enough_dist_3d: false,
            donut_timer: 0,
            preferred_height: 0.0,
            preferred_height_damping: 0.0,
            has_locomotor_pose: false,
            loco_flags: 0,
            maintain_pos: None,
            wander_angle_offset: 0.0,
            wander_offset_increment: 0.0,
            has_locomotor_template: false,
            cur_locomotor_name: None,
            valid_locomotor_surfaces: 0,
            downhill_only: false,
        }
    }
}

impl From<ObjectXferPersistV5> for ObjectXferPersist {
    fn from(v5: ObjectXferPersistV5) -> Self {
        Self {
            object_id: v5.object_id,
            disabled_held: v5.disabled_held,
            single_use_command_used: v5.single_use_command_used,
            ai_attitude: v5.ai_attitude,
            custom_indicator_color: v5.custom_indicator_color,
            vision_range: v5.vision_range,
            shroud_clearing_range: v5.shroud_clearing_range,
            shroud_range: v5.shroud_range,
            pre_attack_ready_at: v5.pre_attack_ready_at,
            pre_attack_target: v5.pre_attack_target,
            consecutive_shot_target: v5.consecutive_shot_target,
            max_shots_to_fire: v5.max_shots_to_fire,
            turret_angle_deg: v5.turret_angle_deg,
            turret_pitch_deg: v5.turret_pitch_deg,
            turret_idle_scan_next_frame: v5.turret_idle_scan_next_frame,
            turret_idle_scanning: v5.turret_idle_scanning,
            turret_idle_scan_desired_angle_deg: v5.turret_idle_scan_desired_angle_deg,
            turret_idle_scan_index: v5.turret_idle_scan_index,
            turret_holding: v5.turret_holding,
            turret_hold_until_frame: v5.turret_hold_until_frame,
            turret_idle_recentering: v5.turret_idle_recentering,
            turret_mood_target: v5.turret_mood_target,
            turret_target_id: v5.turret_target_id,
            turret_force_attacking: v5.turret_force_attacking,
            turret_enabled: v5.turret_enabled,
            turret_substate: v5.turret_substate,
            turret_rotating: v5.turret_rotating,
            temporary_stealth_expires_frame: v5.temporary_stealth_expires_frame,
            weapon_bonus_solo: v5.weapon_bonus_solo,
            which_exit_path: v5.which_exit_path,
            cheer_timer: v5.cheer_timer,
            special_cheering: v5.special_cheering,
            weapon_scatter_targets_unused: v5.weapon_scatter_targets_unused,
            weapon_scatter_targets_inited: v5.weapon_scatter_targets_inited,
            weapon_bonus_horde: v5.weapon_bonus_horde,
            weapon_bonus_enthusiastic: v5.weapon_bonus_enthusiastic,
            weapon_bonus_subliminal: v5.weapon_bonus_subliminal,
            safe_occlusion_frame: v5.safe_occlusion_frame,
            health_box_offset: v5.health_box_offset,
            last_fire_frame: v5.last_fire_frame,
            is_recruitable: v5.is_recruitable,
            guard_next_enemy_scan: v5.guard_next_enemy_scan,
            quick_exit_until: None,
            hunt_next_enemy_scan: v5.hunt_next_enemy_scan,
            ignore_collisions_until_frame: v5.ignore_collisions_until_frame,
            do_final_position: v5.do_final_position,
            final_position: v5.final_position,
            ignored_obstacle_id: v5.ignored_obstacle_id,
            can_path_through_units: v5.can_path_through_units,
            last_command_source: v5.last_command_source,
            locomotor_upgrade: false,
            cur_locomotor_set: None,
            has_locomotor_motion: false,
            braking: 0.0,
            max_lift: 0.0,
            braking_factor: 0.0,
            has_locomotor_arrival: false,
            close_enough_dist: None,
            close_enough_dist_3d: false,
            donut_timer: 0,
            preferred_height: 0.0,
            preferred_height_damping: 0.0,
            has_locomotor_pose: false,
            loco_flags: 0,
            maintain_pos: None,
            wander_angle_offset: 0.0,
            wander_offset_increment: 0.0,
            has_locomotor_template: false,
            cur_locomotor_name: None,
            valid_locomotor_surfaces: 0,
            downhill_only: false,
        }
    }
}

impl From<ObjectXferPersistV6> for ObjectXferPersist {
    fn from(v6: ObjectXferPersistV6) -> Self {
        Self {
            object_id: v6.object_id,
            disabled_held: v6.disabled_held,
            single_use_command_used: v6.single_use_command_used,
            ai_attitude: v6.ai_attitude,
            custom_indicator_color: v6.custom_indicator_color,
            vision_range: v6.vision_range,
            shroud_clearing_range: v6.shroud_clearing_range,
            shroud_range: v6.shroud_range,
            pre_attack_ready_at: v6.pre_attack_ready_at,
            pre_attack_target: v6.pre_attack_target,
            consecutive_shot_target: v6.consecutive_shot_target,
            max_shots_to_fire: v6.max_shots_to_fire,
            turret_angle_deg: v6.turret_angle_deg,
            turret_pitch_deg: v6.turret_pitch_deg,
            turret_idle_scan_next_frame: v6.turret_idle_scan_next_frame,
            turret_idle_scanning: v6.turret_idle_scanning,
            turret_idle_scan_desired_angle_deg: v6.turret_idle_scan_desired_angle_deg,
            turret_idle_scan_index: v6.turret_idle_scan_index,
            turret_holding: v6.turret_holding,
            turret_hold_until_frame: v6.turret_hold_until_frame,
            turret_idle_recentering: v6.turret_idle_recentering,
            turret_mood_target: v6.turret_mood_target,
            turret_target_id: v6.turret_target_id,
            turret_force_attacking: v6.turret_force_attacking,
            turret_enabled: v6.turret_enabled,
            turret_substate: v6.turret_substate,
            turret_rotating: v6.turret_rotating,
            temporary_stealth_expires_frame: v6.temporary_stealth_expires_frame,
            weapon_bonus_solo: v6.weapon_bonus_solo,
            which_exit_path: v6.which_exit_path,
            cheer_timer: v6.cheer_timer,
            special_cheering: v6.special_cheering,
            weapon_scatter_targets_unused: v6.weapon_scatter_targets_unused,
            weapon_scatter_targets_inited: v6.weapon_scatter_targets_inited,
            weapon_bonus_horde: v6.weapon_bonus_horde,
            weapon_bonus_enthusiastic: v6.weapon_bonus_enthusiastic,
            weapon_bonus_subliminal: v6.weapon_bonus_subliminal,
            safe_occlusion_frame: v6.safe_occlusion_frame,
            health_box_offset: v6.health_box_offset,
            last_fire_frame: v6.last_fire_frame,
            is_recruitable: v6.is_recruitable,
            guard_next_enemy_scan: v6.guard_next_enemy_scan,
            quick_exit_until: None,
            hunt_next_enemy_scan: v6.hunt_next_enemy_scan,
            ignore_collisions_until_frame: v6.ignore_collisions_until_frame,
            do_final_position: v6.do_final_position,
            final_position: v6.final_position,
            ignored_obstacle_id: v6.ignored_obstacle_id,
            can_path_through_units: v6.can_path_through_units,
            last_command_source: v6.last_command_source,
            locomotor_upgrade: v6.locomotor_upgrade,
            cur_locomotor_set: v6.cur_locomotor_set,
            has_locomotor_motion: false,
            braking: 0.0,
            max_lift: 0.0,
            braking_factor: 0.0,
            has_locomotor_arrival: false,
            close_enough_dist: None,
            close_enough_dist_3d: false,
            donut_timer: 0,
            preferred_height: 0.0,
            preferred_height_damping: 0.0,
            has_locomotor_pose: false,
            loco_flags: 0,
            maintain_pos: None,
            wander_angle_offset: 0.0,
            wander_offset_increment: 0.0,
            has_locomotor_template: false,
            cur_locomotor_name: None,
            valid_locomotor_surfaces: 0,
            downhill_only: false,
        }
    }
}

impl From<ObjectXferPersistV7> for ObjectXferPersist {
    fn from(v7: ObjectXferPersistV7) -> Self {
        Self {
            object_id: v7.object_id,
            disabled_held: v7.disabled_held,
            single_use_command_used: v7.single_use_command_used,
            ai_attitude: v7.ai_attitude,
            custom_indicator_color: v7.custom_indicator_color,
            vision_range: v7.vision_range,
            shroud_clearing_range: v7.shroud_clearing_range,
            shroud_range: v7.shroud_range,
            pre_attack_ready_at: v7.pre_attack_ready_at,
            pre_attack_target: v7.pre_attack_target,
            consecutive_shot_target: v7.consecutive_shot_target,
            max_shots_to_fire: v7.max_shots_to_fire,
            turret_angle_deg: v7.turret_angle_deg,
            turret_pitch_deg: v7.turret_pitch_deg,
            turret_idle_scan_next_frame: v7.turret_idle_scan_next_frame,
            turret_idle_scanning: v7.turret_idle_scanning,
            turret_idle_scan_desired_angle_deg: v7.turret_idle_scan_desired_angle_deg,
            turret_idle_scan_index: v7.turret_idle_scan_index,
            turret_holding: v7.turret_holding,
            turret_hold_until_frame: v7.turret_hold_until_frame,
            turret_idle_recentering: v7.turret_idle_recentering,
            turret_mood_target: v7.turret_mood_target,
            turret_target_id: v7.turret_target_id,
            turret_force_attacking: v7.turret_force_attacking,
            turret_enabled: v7.turret_enabled,
            turret_substate: v7.turret_substate,
            turret_rotating: v7.turret_rotating,
            temporary_stealth_expires_frame: v7.temporary_stealth_expires_frame,
            weapon_bonus_solo: v7.weapon_bonus_solo,
            which_exit_path: v7.which_exit_path,
            cheer_timer: v7.cheer_timer,
            special_cheering: v7.special_cheering,
            weapon_scatter_targets_unused: v7.weapon_scatter_targets_unused,
            weapon_scatter_targets_inited: v7.weapon_scatter_targets_inited,
            weapon_bonus_horde: v7.weapon_bonus_horde,
            weapon_bonus_enthusiastic: v7.weapon_bonus_enthusiastic,
            weapon_bonus_subliminal: v7.weapon_bonus_subliminal,
            safe_occlusion_frame: v7.safe_occlusion_frame,
            health_box_offset: v7.health_box_offset,
            last_fire_frame: v7.last_fire_frame,
            is_recruitable: v7.is_recruitable,
            guard_next_enemy_scan: v7.guard_next_enemy_scan,
            quick_exit_until: None,
            hunt_next_enemy_scan: v7.hunt_next_enemy_scan,
            ignore_collisions_until_frame: v7.ignore_collisions_until_frame,
            do_final_position: v7.do_final_position,
            final_position: v7.final_position,
            ignored_obstacle_id: v7.ignored_obstacle_id,
            can_path_through_units: v7.can_path_through_units,
            last_command_source: v7.last_command_source,
            locomotor_upgrade: v7.locomotor_upgrade,
            cur_locomotor_set: v7.cur_locomotor_set,
            has_locomotor_motion: v7.has_locomotor_motion,
            braking: v7.braking,
            max_lift: v7.max_lift,
            braking_factor: v7.braking_factor,
            has_locomotor_arrival: false,
            close_enough_dist: None,
            close_enough_dist_3d: false,
            donut_timer: 0,
            preferred_height: 0.0,
            preferred_height_damping: 0.0,
            has_locomotor_pose: false,
            loco_flags: 0,
            maintain_pos: None,
            wander_angle_offset: 0.0,
            wander_offset_increment: 0.0,
            has_locomotor_template: false,
            cur_locomotor_name: None,
            valid_locomotor_surfaces: 0,
            downhill_only: false,
        }
    }
}

impl From<ObjectXferPersistV8> for ObjectXferPersist {
    fn from(v8: ObjectXferPersistV8) -> Self {
        Self {
            object_id: v8.object_id,
            disabled_held: v8.disabled_held,
            single_use_command_used: v8.single_use_command_used,
            ai_attitude: v8.ai_attitude,
            custom_indicator_color: v8.custom_indicator_color,
            vision_range: v8.vision_range,
            shroud_clearing_range: v8.shroud_clearing_range,
            shroud_range: v8.shroud_range,
            pre_attack_ready_at: v8.pre_attack_ready_at,
            pre_attack_target: v8.pre_attack_target,
            consecutive_shot_target: v8.consecutive_shot_target,
            max_shots_to_fire: v8.max_shots_to_fire,
            turret_angle_deg: v8.turret_angle_deg,
            turret_pitch_deg: v8.turret_pitch_deg,
            turret_idle_scan_next_frame: v8.turret_idle_scan_next_frame,
            turret_idle_scanning: v8.turret_idle_scanning,
            turret_idle_scan_desired_angle_deg: v8.turret_idle_scan_desired_angle_deg,
            turret_idle_scan_index: v8.turret_idle_scan_index,
            turret_holding: v8.turret_holding,
            turret_hold_until_frame: v8.turret_hold_until_frame,
            turret_idle_recentering: v8.turret_idle_recentering,
            turret_mood_target: v8.turret_mood_target,
            turret_target_id: v8.turret_target_id,
            turret_force_attacking: v8.turret_force_attacking,
            turret_enabled: v8.turret_enabled,
            turret_substate: v8.turret_substate,
            turret_rotating: v8.turret_rotating,
            temporary_stealth_expires_frame: v8.temporary_stealth_expires_frame,
            weapon_bonus_solo: v8.weapon_bonus_solo,
            which_exit_path: v8.which_exit_path,
            cheer_timer: v8.cheer_timer,
            special_cheering: v8.special_cheering,
            weapon_scatter_targets_unused: v8.weapon_scatter_targets_unused,
            weapon_scatter_targets_inited: v8.weapon_scatter_targets_inited,
            weapon_bonus_horde: v8.weapon_bonus_horde,
            weapon_bonus_enthusiastic: v8.weapon_bonus_enthusiastic,
            weapon_bonus_subliminal: v8.weapon_bonus_subliminal,
            safe_occlusion_frame: v8.safe_occlusion_frame,
            health_box_offset: v8.health_box_offset,
            last_fire_frame: v8.last_fire_frame,
            is_recruitable: v8.is_recruitable,
            guard_next_enemy_scan: v8.guard_next_enemy_scan,
            quick_exit_until: None,
            hunt_next_enemy_scan: v8.hunt_next_enemy_scan,
            ignore_collisions_until_frame: v8.ignore_collisions_until_frame,
            do_final_position: v8.do_final_position,
            final_position: v8.final_position,
            ignored_obstacle_id: v8.ignored_obstacle_id,
            can_path_through_units: v8.can_path_through_units,
            last_command_source: v8.last_command_source,
            locomotor_upgrade: v8.locomotor_upgrade,
            cur_locomotor_set: v8.cur_locomotor_set,
            has_locomotor_motion: v8.has_locomotor_motion,
            braking: v8.braking,
            max_lift: v8.max_lift,
            braking_factor: v8.braking_factor,
            has_locomotor_arrival: v8.has_locomotor_arrival,
            close_enough_dist: v8.close_enough_dist,
            close_enough_dist_3d: v8.close_enough_dist_3d,
            donut_timer: v8.donut_timer,
            preferred_height: v8.preferred_height,
            preferred_height_damping: v8.preferred_height_damping,
            has_locomotor_pose: false,
            loco_flags: 0,
            maintain_pos: None,
            wander_angle_offset: 0.0,
            wander_offset_increment: 0.0,
            has_locomotor_template: false,
            cur_locomotor_name: None,
            valid_locomotor_surfaces: 0,
            downhill_only: false,
        }
    }
}

impl From<ObjectXferPersistV9> for ObjectXferPersist {
    fn from(v9: ObjectXferPersistV9) -> Self {
        Self {
            object_id: v9.object_id,
            disabled_held: v9.disabled_held,
            single_use_command_used: v9.single_use_command_used,
            ai_attitude: v9.ai_attitude,
            custom_indicator_color: v9.custom_indicator_color,
            vision_range: v9.vision_range,
            shroud_clearing_range: v9.shroud_clearing_range,
            shroud_range: v9.shroud_range,
            pre_attack_ready_at: v9.pre_attack_ready_at,
            pre_attack_target: v9.pre_attack_target,
            consecutive_shot_target: v9.consecutive_shot_target,
            max_shots_to_fire: v9.max_shots_to_fire,
            turret_angle_deg: v9.turret_angle_deg,
            turret_pitch_deg: v9.turret_pitch_deg,
            turret_idle_scan_next_frame: v9.turret_idle_scan_next_frame,
            turret_idle_scanning: v9.turret_idle_scanning,
            turret_idle_scan_desired_angle_deg: v9.turret_idle_scan_desired_angle_deg,
            turret_idle_scan_index: v9.turret_idle_scan_index,
            turret_holding: v9.turret_holding,
            turret_hold_until_frame: v9.turret_hold_until_frame,
            turret_idle_recentering: v9.turret_idle_recentering,
            turret_mood_target: v9.turret_mood_target,
            turret_target_id: v9.turret_target_id,
            turret_force_attacking: v9.turret_force_attacking,
            turret_enabled: v9.turret_enabled,
            turret_substate: v9.turret_substate,
            turret_rotating: v9.turret_rotating,
            temporary_stealth_expires_frame: v9.temporary_stealth_expires_frame,
            weapon_bonus_solo: v9.weapon_bonus_solo,
            which_exit_path: v9.which_exit_path,
            cheer_timer: v9.cheer_timer,
            special_cheering: v9.special_cheering,
            weapon_scatter_targets_unused: v9.weapon_scatter_targets_unused,
            weapon_scatter_targets_inited: v9.weapon_scatter_targets_inited,
            weapon_bonus_horde: v9.weapon_bonus_horde,
            weapon_bonus_enthusiastic: v9.weapon_bonus_enthusiastic,
            weapon_bonus_subliminal: v9.weapon_bonus_subliminal,
            safe_occlusion_frame: v9.safe_occlusion_frame,
            health_box_offset: v9.health_box_offset,
            last_fire_frame: v9.last_fire_frame,
            is_recruitable: v9.is_recruitable,
            guard_next_enemy_scan: v9.guard_next_enemy_scan,
            quick_exit_until: None,
            hunt_next_enemy_scan: v9.hunt_next_enemy_scan,
            ignore_collisions_until_frame: v9.ignore_collisions_until_frame,
            do_final_position: v9.do_final_position,
            final_position: v9.final_position,
            ignored_obstacle_id: v9.ignored_obstacle_id,
            can_path_through_units: v9.can_path_through_units,
            last_command_source: v9.last_command_source,
            locomotor_upgrade: v9.locomotor_upgrade,
            cur_locomotor_set: v9.cur_locomotor_set,
            has_locomotor_motion: v9.has_locomotor_motion,
            braking: v9.braking,
            max_lift: v9.max_lift,
            braking_factor: v9.braking_factor,
            has_locomotor_arrival: v9.has_locomotor_arrival,
            close_enough_dist: v9.close_enough_dist,
            close_enough_dist_3d: v9.close_enough_dist_3d,
            donut_timer: v9.donut_timer,
            preferred_height: v9.preferred_height,
            preferred_height_damping: v9.preferred_height_damping,
            has_locomotor_pose: v9.has_locomotor_pose,
            loco_flags: v9.loco_flags,
            maintain_pos: v9.maintain_pos,
            wander_angle_offset: v9.wander_angle_offset,
            wander_offset_increment: v9.wander_offset_increment,
            has_locomotor_template: false,
            cur_locomotor_name: None,
            valid_locomotor_surfaces: 0,
            downhill_only: false,
        }
    }
}

pub fn append_to_lifecycle_tail(bytes: &mut Vec<u8>, game_logic: &GameLogic) {
    let payload = capture(game_logic);
    if payload.objects.is_empty() {
        return;
    }
    let Ok(encoded) = bincode_legacy::serialize(&payload) else {
        return;
    };
    bytes.extend_from_slice(OXOB_MAGIC);
    append_u32(bytes, OXOB_VERSION);
    append_u32(bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
}

/// Inspect the historical XP-only receipt before any receiving-world mutation.
pub(super) fn saved_kill_experience_receipts(
    bytes: &[u8],
) -> SaveLoadResult<std::collections::HashSet<ObjectId>> {
    let Some(mut rest) = find_oxob_suffix(bytes) else {
        return Ok(Default::default());
    };
    let version = take_u32(&mut rest)?;
    if version < OXOB_VERSION {
        return Ok(Default::default());
    }
    if version != OXOB_VERSION {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown OXOB suffix version {version}"
        )));
    }
    let length = take_u32(&mut rest)? as usize;
    let bytes = rest
        .get(..length)
        .ok_or_else(|| SaveLoadError::Corrupted("OXOB payload truncated".into()))?;
    let payload: ObjectXferPersistPayload = bincode_legacy::deserialize(bytes)
        .map_err(|error| SaveLoadError::Corrupted(format!("OXOB payload decode: {error}")))?;
    Ok(payload
        .body_history
        .into_iter()
        .filter(|record| record.kill_experience_awarded)
        .map(|record| ObjectId(record.object_id))
        .collect())
}

pub fn apply_from_lifecycle_tail(bytes: &[u8], game_logic: &mut GameLogic) -> SaveLoadResult<()> {
    let Some(suffix) = find_oxob_suffix(bytes) else {
        reset_object_xfer(game_logic);
        return Ok(());
    };
    let mut rest = suffix;
    let version = take_u32(&mut rest)?;
    if version != 1
        && version != 2
        && version != 3
        && version != 4
        && version != 5
        && version != 6
        && version != 7
        && version != 8
        && version != 9
        && version != 10
        && version != 11
        && version != 12
        && version != 13
        && version != 14
        && version != 15
        && version != 16
        && version != OXOB_VERSION
    {
        return Err(SaveLoadError::Corrupted(format!(
            "unknown OXOB suffix version {version}"
        )));
    }
    let payload_len = take_u32(&mut rest)? as usize;
    if rest.len() < payload_len {
        return Err(SaveLoadError::Corrupted(
            "OXOB payload truncated".to_string(),
        ));
    }
    let encoded = &rest[..payload_len];
    let payload = if version == 1 {
        let old: ObjectXferPersistPayloadV1 = bincode_legacy::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("OXOB payload decode: {err}")))?;
        ObjectXferPersistPayload {
            objects: old
                .objects
                .into_iter()
                .map(ObjectXferPersist::from)
                .collect(),
            script_status: Vec::new(),
            structures: Vec::new(),
            damage_info: Vec::new(),
            transport: Vec::new(),
            body_history: Vec::new(),
        }
    } else if version == 2 {
        let old: ObjectXferPersistPayloadV2 = bincode_legacy::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("OXOB payload decode: {err}")))?;
        ObjectXferPersistPayload {
            objects: old
                .objects
                .into_iter()
                .map(ObjectXferPersist::from)
                .collect(),
            script_status: Vec::new(),
            structures: Vec::new(),
            damage_info: Vec::new(),
            transport: Vec::new(),
            body_history: Vec::new(),
        }
    } else if version == 3 {
        let old: ObjectXferPersistPayloadV3 = bincode_legacy::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("OXOB payload decode: {err}")))?;
        ObjectXferPersistPayload {
            objects: old
                .objects
                .into_iter()
                .map(ObjectXferPersist::from)
                .collect(),
            script_status: Vec::new(),
            structures: Vec::new(),
            damage_info: Vec::new(),
            transport: Vec::new(),
            body_history: Vec::new(),
        }
    } else if version == 4 {
        let old: ObjectXferPersistPayloadV4 = bincode_legacy::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("OXOB payload decode: {err}")))?;
        ObjectXferPersistPayload {
            objects: old
                .objects
                .into_iter()
                .map(ObjectXferPersist::from)
                .collect(),
            script_status: Vec::new(),
            structures: Vec::new(),
            damage_info: Vec::new(),
            transport: Vec::new(),
            body_history: Vec::new(),
        }
    } else if version == 5 {
        let old: ObjectXferPersistPayloadV5 = bincode_legacy::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("OXOB payload decode: {err}")))?;
        ObjectXferPersistPayload {
            objects: old
                .objects
                .into_iter()
                .map(ObjectXferPersist::from)
                .collect(),
            script_status: Vec::new(),
            structures: Vec::new(),
            damage_info: Vec::new(),
            transport: Vec::new(),
            body_history: Vec::new(),
        }
    } else if version == 6 {
        let old: ObjectXferPersistPayloadV6 = bincode_legacy::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("OXOB payload decode: {err}")))?;
        ObjectXferPersistPayload {
            objects: old
                .objects
                .into_iter()
                .map(ObjectXferPersist::from)
                .collect(),
            script_status: Vec::new(),
            structures: Vec::new(),
            damage_info: Vec::new(),
            transport: Vec::new(),
            body_history: Vec::new(),
        }
    } else if version == 7 {
        let old: ObjectXferPersistPayloadV7 = bincode_legacy::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("OXOB payload decode: {err}")))?;
        ObjectXferPersistPayload {
            objects: old
                .objects
                .into_iter()
                .map(ObjectXferPersist::from)
                .collect(),
            script_status: Vec::new(),
            structures: Vec::new(),
            damage_info: Vec::new(),
            transport: Vec::new(),
            body_history: Vec::new(),
        }
    } else if version == 8 {
        let old: ObjectXferPersistPayloadV8 = bincode_legacy::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("OXOB payload decode: {err}")))?;
        ObjectXferPersistPayload {
            objects: old
                .objects
                .into_iter()
                .map(ObjectXferPersist::from)
                .collect(),
            script_status: Vec::new(),
            structures: Vec::new(),
            damage_info: Vec::new(),
            transport: Vec::new(),
            body_history: Vec::new(),
        }
    } else if version == 9 {
        let old: ObjectXferPersistPayloadV9 = bincode_legacy::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("OXOB payload decode: {err}")))?;
        ObjectXferPersistPayload {
            objects: old
                .objects
                .into_iter()
                .map(ObjectXferPersist::from)
                .collect(),
            script_status: Vec::new(),
            structures: Vec::new(),
            damage_info: Vec::new(),
            transport: Vec::new(),
            body_history: Vec::new(),
        }
    } else if version == 10 {
        let old: ObjectXferPersistPayloadV10 = bincode_legacy::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("OXOB payload decode: {err}")))?;
        ObjectXferPersistPayload {
            objects: old
                .objects
                .into_iter()
                .map(ObjectXferPersist::from)
                .collect(),
            script_status: Vec::new(),
            structures: Vec::new(),
            damage_info: Vec::new(),
            transport: Vec::new(),
            body_history: Vec::new(),
        }
    } else if version == 11 {
        let old: ObjectXferPersistPayloadV11 = bincode_legacy::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("OXOB payload decode: {err}")))?;
        ObjectXferPersistPayload {
            objects: old.objects,
            script_status: Vec::new(),
            structures: Vec::new(),
            damage_info: Vec::new(),
            transport: Vec::new(),
            body_history: Vec::new(),
        }
    } else if version == 12 {
        let old: ObjectXferPersistPayloadV12 = bincode_legacy::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("OXOB payload decode: {err}")))?;
        ObjectXferPersistPayload {
            objects: old.objects,
            script_status: old.script_status,
            structures: Vec::new(),
            damage_info: Vec::new(),
            transport: Vec::new(),
            body_history: Vec::new(),
        }
    } else if version == 13 {
        let old: ObjectXferPersistPayloadV13 = bincode_legacy::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("OXOB payload decode: {err}")))?;
        ObjectXferPersistPayload {
            objects: old.objects,
            script_status: old.script_status,
            structures: old.structures,
            damage_info: Vec::new(),
            transport: Vec::new(),
            body_history: Vec::new(),
        }
    } else if version == 14 {
        let old: ObjectXferPersistPayloadV14 = bincode_legacy::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("OXOB payload decode: {err}")))?;
        ObjectXferPersistPayload {
            objects: old.objects,
            script_status: old.script_status,
            structures: old.structures,
            damage_info: old.damage_info,
            transport: Vec::new(),
            body_history: Vec::new(),
        }
    } else if version == 15 {
        let old: ObjectXferPersistPayloadV15 = bincode_legacy::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("OXOB payload decode: {err}")))?;
        ObjectXferPersistPayload {
            objects: old.objects,
            script_status: old.script_status,
            structures: old.structures,
            damage_info: old.damage_info,
            transport: old.transport,
            body_history: Vec::new(),
        }
    } else if version == 16 {
        let old: ObjectXferPersistPayloadV16 = bincode_legacy::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("OXOB payload decode: {err}")))?;
        ObjectXferPersistPayload {
            objects: old.objects,
            script_status: old.script_status,
            structures: old.structures,
            damage_info: old.damage_info,
            transport: old.transport,
            body_history: body_damage_history_persist::migrate_v16(game_logic, old.body_history)?,
        }
    } else {
        bincode_legacy::deserialize(encoded)
            .map_err(|err| SaveLoadError::Corrupted(format!("OXOB payload decode: {err}")))?
    };
    body_damage_history_persist::validate(game_logic, &payload.body_history)?;
    if version >= 16 && payload.body_history.len() != game_logic.host_objects().registered_len() {
        return Err(SaveLoadError::Corrupted(
            "incomplete saved body history owners".into(),
        ));
    }
    transport_movement_persist::validate(game_logic, &payload.transport)?;
    if version >= 15 && payload.transport.len() != game_logic.host_objects().registered_len() {
        return Err(SaveLoadError::Corrupted(
            "incomplete saved transport movement owners".into(),
        ));
    }
    validate_structure_records(game_logic, &payload.structures)?;
    let mut damage_ids = std::collections::HashSet::new();
    for entry in &payload.damage_info {
        if !damage_ids.insert(entry.object_id)
            || game_logic.host_object(ObjectId(entry.object_id)).is_none()
        {
            return Err(SaveLoadError::Corrupted(
                "invalid saved body damage-info owner".into(),
            ));
        }
    }
    if version >= 14 && damage_ids.len() != game_logic.host_objects().registered_len() {
        return Err(SaveLoadError::Corrupted(
            "incomplete saved body damage-info owners".into(),
        ));
    }
    if game_logic.locomotor_definitions.is_some() {
        for entry in &payload.objects {
            if entry.has_locomotor_template {
                if let Some(name) = entry.cur_locomotor_name.as_deref() {
                    if game_logic.resolve_receiving_locomotor(name).is_none() {
                        return Err(SaveLoadError::Corrupted(format!(
                            "saved object {} references an unavailable receiving locomotor {name}",
                            entry.object_id
                        )));
                    }
                }
            }
        }
    }
    // Decode the complete suffix before changing the receiving objects. A bad
    // record must not reset their existing clocks or script state.
    reset_object_xfer(game_logic);
    apply_payload(game_logic, payload);
    Ok(())
}

fn capture(game_logic: &GameLogic) -> ObjectXferPersistPayload {
    // C++ Object::xfer always writes vision/shroud. Restore constructs via
    // new_with_logic_frame, which zeros those ranges, so every host object
    // must be captured — residual-only would drop hijack/S&D/shroud overrides.
    let mut objects = Vec::new();
    for (id, object) in game_logic.host_objects().registered() {
        objects.push(ObjectXferPersist {
            object_id: id.0,
            disabled_held: object.status.disabled_held,
            single_use_command_used: object.single_use_command_used,
            ai_attitude: object.ai_attitude,
            custom_indicator_color: object.custom_indicator_color,
            vision_range: object.vision_range,
            shroud_clearing_range: object.shroud_clearing_range,
            shroud_range: object.shroud_range,
            pre_attack_ready_at: object.pre_attack_ready_at,
            pre_attack_target: object.pre_attack_target.map(|id| id.0),
            consecutive_shot_target: object.consecutive_shot_target.map(|id| id.0),
            max_shots_to_fire: object.max_shots_to_fire,
            turret_angle_deg: object.turret_angle_deg,
            turret_pitch_deg: object.turret_pitch_deg,
            turret_idle_scan_next_frame: object.turret_idle_scan_next_frame,
            turret_idle_scanning: object.turret_idle_scanning,
            turret_idle_scan_desired_angle_deg: object.turret_idle_scan_desired_angle_deg,
            turret_idle_scan_index: object.turret_idle_scan_index,
            turret_holding: object.turret_holding,
            turret_hold_until_frame: object.turret_hold_until_frame,
            turret_idle_recentering: object.turret_idle_recentering,
            turret_mood_target: object.turret_mood_target,
            turret_target_id: object.turret_target_id.map(|id| id.0),
            turret_force_attacking: object.turret_force_attacking,
            turret_enabled: object.turret_enabled,
            turret_substate: object.turret_substate.ordinal(),
            turret_rotating: object.turret_rotating,
            temporary_stealth_expires_frame: object.temporary_stealth_expires_frame,
            weapon_bonus_solo: object.weapon_bonus_solo,
            which_exit_path: object.which_exit_path,
            cheer_timer: object.cheer_timer,
            special_cheering: object_has_special_cheering(object),
            weapon_scatter_targets_unused: object.weapon_scatter_targets_unused.clone(),
            weapon_scatter_targets_inited: object.weapon_scatter_targets_inited,
            weapon_bonus_horde: object.weapon_bonus_horde,
            weapon_bonus_enthusiastic: object.weapon_bonus_enthusiastic,
            weapon_bonus_subliminal: object.weapon_bonus_subliminal,
            safe_occlusion_frame: object.safe_occlusion_frame,
            health_box_offset: object.health_box_offset,
            last_fire_frame: object.last_fire_frame,
            is_recruitable: object.is_recruitable,
            guard_next_enemy_scan: object.unit_ai_runtime.guard_scan_deadline(),
            quick_exit_until: object.unit_ai_runtime.quick_exit_deadline(),
            hunt_next_enemy_scan: object.unit_ai_runtime.hunt_scan_deadline(),
            ignore_collisions_until_frame: object.ignore_collisions_until_frame,
            do_final_position: object.do_final_position,
            final_position: object.final_position.to_array(),
            ignored_obstacle_id: object.ignored_obstacle_id.map(|id| id.0),
            can_path_through_units: object.can_path_through_units,
            last_command_source: object.last_command_source,
            locomotor_upgrade: object.locomotor_upgrade,
            cur_locomotor_set: object.jet_ai.cur_locomotor_set.clone(),
            has_locomotor_motion: true,
            braking: object.braking,
            max_lift: object.max_lift,
            braking_factor: object.braking_factor,
            has_locomotor_arrival: true,
            close_enough_dist: object.close_enough_dist,
            close_enough_dist_3d: object.close_enough_dist_3d,
            donut_timer: object.donut_timer,
            preferred_height: object.loco_preferred_height,
            preferred_height_damping: object.loco_preferred_height_damping,
            has_locomotor_pose: true,
            loco_flags: pack_loco_flags(object),
            maintain_pos: object.maintain_pos.map(|p| [p.x, p.y, p.z]),
            wander_angle_offset: object.wander_angle_offset,
            wander_offset_increment: object.wander_offset_increment,
            has_locomotor_template: true,
            cur_locomotor_name: object.cur_locomotor_name.clone(),
            valid_locomotor_surfaces: object.locomotor_surfaces,
            downhill_only: object.downhill_only,
        });
    }
    let script_status = game_logic
        .host_objects()
        .registered()
        .map(|(id, object)| ObjectScriptStatusPersist {
            object_id: id.0,
            script_targetable: object.script_targetable,
            masked: object.status.masked,
        })
        .collect();
    let structures = game_logic
        .host_objects()
        .registered()
        .filter_map(|(id, object)| {
            object
                .structure_topple_data
                .as_ref()
                .map(|runtime| StructureTopplePersist {
                    object_id: id.0,
                    matrix: object.get_transform_matrix().to_cols_array(),
                    runtime: runtime.clone(),
                })
        })
        .collect();
    let mut damage_info: Vec<_> = game_logic
        .host_objects()
        .registered()
        .map(|(id, object)| ObjectDamageInfoPersist {
            object_id: id.0,
            last_damage_type: object.last_damage_info_type,
        })
        .collect();
    damage_info.sort_by_key(|entry| entry.object_id);
    ObjectXferPersistPayload {
        objects,
        script_status,
        structures,
        damage_info,
        transport: transport_movement_persist::capture(game_logic),
        body_history: body_damage_history_persist::capture(game_logic),
    }
}

fn validate_structure_records(
    game_logic: &GameLogic,
    records: &[StructureTopplePersist],
) -> SaveLoadResult<()> {
    let mut ids = std::collections::HashSet::new();
    for entry in records {
        let data = &entry.runtime;
        let object = game_logic
            .host_object(ObjectId(entry.object_id))
            .ok_or_else(|| {
                SaveLoadError::Corrupted("saved StructureTopple owner is absent".into())
            })?;
        let values = [
            data.dir_x,
            data.dir_y,
            data.topple_velocity,
            data.accumulated_angle,
            data.structural_integrity,
            data.structural_decay,
            data.lean_radians,
            data.last_crushed_location,
            data.building_height,
            data.facing_width,
            data.crush_damage,
            data.crush_radius,
            data.major_radius,
            data.minor_radius,
            data.orientation,
            data.building_x,
            data.building_y,
            data.building_z,
            data.delay_burst_x,
            data.delay_burst_y,
            data.delay_burst_z,
            data.last_polled_angle,
        ];
        if !ids.insert(entry.object_id)
            || !object.thing().template.authored_structure_topple
            || object
                .thing()
                .template
                .structure_topple_definition
                .as_ref()
                .is_some_and(Result::is_err)
            || !entry.matrix.iter().all(|value| value.is_finite())
            || !values.iter().all(|value| value.is_finite())
            || !data.angle_fx.iter().all(|(angle, _)| angle.is_finite())
            || !data.pending_fx.iter().all(|effect| {
                [effect.x, effect.y, effect.z]
                    .iter()
                    .all(|value| value.is_finite())
            })
        {
            return Err(SaveLoadError::Corrupted(
                "invalid saved StructureTopple state or receiving module".into(),
            ));
        }
    }
    Ok(())
}

fn reset_object_xfer(game_logic: &mut GameLogic) {
    transport_movement_persist::reset(game_logic);
    body_damage_history_persist::apply(game_logic, Vec::new());
    let ids: Vec<ObjectId> = game_logic
        .host_objects()
        .registered_keys()
        .copied()
        .collect();
    for id in ids {
        let Some(object) = game_logic.host_objects_mut().get_mut(&id) else {
            continue;
        };
        object.script_targetable = false;
        object.status.masked = false;
        object.structure_topple_data = None;
        object.last_damage_info_type = None;
        object.status.disabled_held = false;
        object.single_use_command_used = false;
        object.ai_attitude = 0;
        object.custom_indicator_color = None;
        object.pre_attack_ready_at = 0.0;
        object.pre_attack_target = None;
        object.consecutive_shot_target = None;
        object.max_shots_to_fire = -1;
        object.turret_idle_scan_next_frame = 0;
        object.turret_idle_scanning = false;
        object.turret_holding = false;
        object.turret_hold_until_frame = 0;
        object.turret_idle_recentering = false;
        object.turret_mood_target = false;
        object.turret_target_id = None;
        object.turret_force_attacking = false;
        object.turret_substate = TurretSubState::Idle;
        object.turret_rotating = false;
        object.temporary_stealth_expires_frame = 0;
        object.weapon_bonus_solo = 0;
        object.is_receiving_difficulty_bonus = false;
        object.which_exit_path = 0;
        object.cheer_timer = 0.0;
        set_object_special_cheering(object, false);
        object.weapon_scatter_targets_unused = Default::default();
        object.weapon_scatter_targets_inited = [false; 3];
        object.weapon_bonus_horde = false;
        object.weapon_bonus_enthusiastic = false;
        object.weapon_bonus_subliminal = false;
        object.health_box_offset = [0.0; 3];
        object.last_fire_frame = 0;
        object.is_recruitable = true;
        object.ignore_collisions_until_frame = 0;
        object.do_final_position = false;
        object.final_position = glam::Vec3::ZERO;
        object.ignored_obstacle_id = None;
        object.can_path_through_units = false;
        object.last_command_source = HUNT_CMD_FROM_AI;
        object.locomotor_upgrade = false;
        object.jet_ai.cur_locomotor_set = None;
    }
}

fn apply_payload(game_logic: &mut GameLogic, payload: ObjectXferPersistPayload) {
    transport_movement_persist::apply(game_logic, payload.transport);
    body_damage_history_persist::apply(game_logic, payload.body_history);
    // Original Object::xfer restores raw status; it does not replay mask or
    // script commands. The receiving world already owns these exact objects.
    for entry in payload.damage_info {
        if let Some(object) = game_logic
            .host_objects_mut()
            .get_mut(&ObjectId(entry.object_id))
        {
            object.last_damage_info_type = entry.last_damage_type;
        }
    }
    for entry in payload.structures {
        if let Some(object) = game_logic
            .host_objects_mut()
            .get_mut(&ObjectId(entry.object_id))
        {
            object.restore_owned_structure_topple(
                entry.runtime,
                glam::Mat4::from_cols_array(&entry.matrix),
            );
        }
    }
    for entry in payload.script_status {
        if let Some(object) = game_logic
            .host_objects_mut()
            .get_mut(&ObjectId(entry.object_id))
        {
            object.script_targetable = entry.script_targetable;
            object.status.masked = entry.masked;
        }
    }
    for entry in payload.objects {
        let id = ObjectId(entry.object_id);
        let selected_locomotor = entry
            .has_locomotor_template
            .then(|| entry.cur_locomotor_name.as_deref())
            .flatten()
            .and_then(|name| game_logic.resolve_receiving_locomotor(name));
        if let Some(object) = game_logic.host_objects_mut().get_mut(&id) {
            object.status.disabled_held = entry.disabled_held;
            object.single_use_command_used = entry.single_use_command_used;
            object.ai_attitude = entry.ai_attitude;
            object.custom_indicator_color = entry.custom_indicator_color;
            object.vision_range = entry.vision_range;
            object.shroud_clearing_range = entry.shroud_clearing_range;
            object.shroud_range = entry.shroud_range;
            object.pre_attack_ready_at = entry.pre_attack_ready_at;
            object.pre_attack_target = entry.pre_attack_target.map(ObjectId);
            object.consecutive_shot_target = entry.consecutive_shot_target.map(ObjectId);
            object.max_shots_to_fire = entry.max_shots_to_fire;
            object.turret_angle_deg = entry.turret_angle_deg;
            object.turret_pitch_deg = entry.turret_pitch_deg;
            object.turret_idle_scan_next_frame = entry.turret_idle_scan_next_frame;
            object.turret_idle_scanning = entry.turret_idle_scanning;
            object.turret_idle_scan_desired_angle_deg = entry.turret_idle_scan_desired_angle_deg;
            object.turret_idle_scan_index = entry.turret_idle_scan_index;
            object.turret_holding = entry.turret_holding;
            object.turret_hold_until_frame = entry.turret_hold_until_frame;
            object.turret_idle_recentering = entry.turret_idle_recentering;
            object.turret_mood_target = entry.turret_mood_target;
            object.turret_target_id = entry.turret_target_id.map(ObjectId);
            object.turret_force_attacking = entry.turret_force_attacking;
            object.turret_enabled = entry.turret_enabled;
            object.turret_substate = TurretSubState::from_ordinal(entry.turret_substate);
            object.turret_rotating = entry.turret_rotating;
            object.temporary_stealth_expires_frame = entry.temporary_stealth_expires_frame;
            object.weapon_bonus_solo = entry.weapon_bonus_solo;
            object.is_receiving_difficulty_bonus = entry.weapon_bonus_solo != 0;
            object.which_exit_path = entry.which_exit_path;
            object.cheer_timer = entry.cheer_timer;
            set_object_special_cheering(object, entry.special_cheering);
            object.weapon_scatter_targets_unused = entry.weapon_scatter_targets_unused;
            object.weapon_scatter_targets_inited = entry.weapon_scatter_targets_inited;
            object.weapon_bonus_horde = entry.weapon_bonus_horde;
            object.weapon_bonus_enthusiastic = entry.weapon_bonus_enthusiastic;
            object.weapon_bonus_subliminal = entry.weapon_bonus_subliminal;
            object.safe_occlusion_frame = entry.safe_occlusion_frame;
            object.health_box_offset = entry.health_box_offset;
            object.last_fire_frame = entry.last_fire_frame;
            object.is_recruitable = entry.is_recruitable;
            object.ignore_collisions_until_frame = entry.ignore_collisions_until_frame;
            object.do_final_position = entry.do_final_position;
            object.final_position = glam::Vec3::from_array(entry.final_position);
            object.ignored_obstacle_id = entry.ignored_obstacle_id.map(ObjectId);
            object.can_path_through_units = entry.can_path_through_units;
            object.last_command_source = entry.last_command_source;
            if entry.has_locomotor_template {
                object.selected_locomotor_definition = None;
                object.cur_locomotor_name = entry.cur_locomotor_name;
                // Definition facts come from the receiving world; transferred
                // motion, arrival, and pose remain runtime state below.
                if let Some(binding) = selected_locomotor {
                    // Older guard/panic module capsules already restored their
                    // explicit width projection. Keep it through this definition
                    // reconstruction; v11 later restores the canonical row.
                    let saved_width = (object.panic_runtime.is_some()
                        || object.unit_ai_runtime.guard_phase().is_some())
                    .then_some(object.wander_width_factor);
                    crate::game_logic::locomotor_bootstrap::apply_host_locomotor_template_facts(
                        object, &binding,
                    );
                    if let Some(width) = saved_width {
                        object.wander_width_factor = width;
                    }
                }
            }
            object.restore_saved_locomotor(entry.locomotor_upgrade, entry.cur_locomotor_set);
            if entry.has_locomotor_motion {
                object.braking = entry.braking;
                object.max_lift = entry.max_lift;
                object.braking_factor = entry.braking_factor;
            }
            if entry.has_locomotor_arrival {
                object.close_enough_dist = entry.close_enough_dist;
                object.close_enough_dist_3d = entry.close_enough_dist_3d;
                object.donut_timer = entry.donut_timer;
                object.loco_preferred_height = entry.preferred_height;
                object.loco_preferred_height_damping = entry.preferred_height_damping;
            }
            if entry.has_locomotor_pose {
                apply_loco_flags(object, entry.loco_flags);
                object.maintain_pos = entry.maintain_pos.map(glam::Vec3::from_array);
                object.wander_angle_offset = entry.wander_angle_offset;
                object.wander_offset_increment = entry.wander_offset_increment;
            }
            if entry.has_locomotor_template {
                object.locomotor_surfaces = entry.valid_locomotor_surfaces;
                object.downhill_only = entry.downhill_only;
            }
            let locomotor_body_saved = entry.has_locomotor_template
                || entry.has_locomotor_motion
                || entry.has_locomotor_pose
                || entry.has_locomotor_arrival;
            if !locomotor_body_saved {
                if let Some(token) = object.jet_ai.cur_locomotor_set.clone() {
                    if let Some(kind) = crate::game_logic::host_upgrade_module_residuals::locomotor_set_kind_from_token(&token)
                    {
                        object.jet_ai.cur_locomotor_set = None;
                        crate::game_logic::host_upgrade_module_residuals::apply_locomotor_set_kind(
                            object,
                            kind,
                        );
                        object.jet_ai.cur_locomotor_set = Some(token);
                    }
                }
            }
        }
    }
}

fn pack_loco_flags(object: &crate::game_logic::Object) -> u32 {
    let mut flags = 0u32;
    let mut set = |bit: u32, on: bool| {
        if on {
            flags |= 1 << bit;
        }
    };
    // C++ LocoFlag order (Locomotor.h:397-408).
    set(0, object.is_braking);
    set(1, object.allow_invalid_position);
    set(2, object.maintain_pos_valid);
    set(3, object.precise_z_pos);
    set(4, object.no_slow_down_as_approaching_dest);
    set(5, object.over_water);
    set(6, object.ultra_accurate);
    set(7, object.moving_backwards);
    set(8, object.doing_three_point_turn);
    set(9, object.is_climbing);
    set(10, object.close_enough_dist_3d);
    set(11, object.wander_offset_increasing);
    flags
}

fn apply_loco_flags(object: &mut crate::game_logic::Object, flags: u32) {
    let on = |bit: u32| flags & (1 << bit) != 0;
    object.is_braking = on(0);
    object.allow_invalid_position = on(1);
    object.maintain_pos_valid = on(2);
    object.precise_z_pos = on(3);
    object.no_slow_down_as_approaching_dest = on(4);
    object.over_water = on(5);
    object.ultra_accurate = on(6);
    object.moving_backwards = on(7);
    object.doing_three_point_turn = on(8);
    object.is_climbing = on(9);
    object.close_enough_dist_3d = on(10);
    object.wander_offset_increasing = on(11);
}

fn special_cheering_mask() -> u128 {
    1u128 << crate::game_logic::host_enum_table_residual::special_cheering_model_bit()
}

fn object_has_special_cheering(object: &crate::game_logic::Object) -> bool {
    object.model_condition_bits & special_cheering_mask() != 0
}

fn set_object_special_cheering(object: &mut crate::game_logic::Object, on: bool) {
    let mask = special_cheering_mask();
    if on {
        object.model_condition_bits |= mask;
    } else {
        object.model_condition_bits &= !mask;
    }
}

fn find_oxob_suffix(bytes: &[u8]) -> Option<&[u8]> {
    bytes
        .windows(4)
        .rposition(|window| window == OXOB_MAGIC)
        .map(|idx| &bytes[idx + 4..])
}

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn take_u32(rest: &mut &[u8]) -> SaveLoadResult<u32> {
    if rest.len() < 4 {
        return Err(SaveLoadError::Corrupted("OXOB u32 truncated".to_string()));
    }
    let (head, tail) = rest.split_at(4);
    *rest = tail;
    Ok(u32::from_le_bytes([head[0], head[1], head[2], head[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v11_object_payload_keeps_original_layout_and_defaults_new_script_flags() {
        let mut source = GameLogic::new();
        let template = crate::game_logic::ThingTemplate::new("OldScriptStatusOwner");
        source.templates.insert(template.name.clone(), template);
        let id = source
            .create_object(
                "OldScriptStatusOwner",
                crate::game_logic::Team::USA,
                glam::Vec3::ZERO,
            )
            .unwrap();
        let object = source.host_object_mut(id).unwrap();
        object.single_use_command_used = true;
        object.script_targetable = true;
        object.status.masked = true;
        let current = capture(&source);
        let old = ObjectXferPersistPayloadV11 {
            objects: current.objects,
        };
        let encoded = bincode_legacy::serialize(&old).unwrap();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(OXOB_MAGIC);
        append_u32(&mut bytes, 11);
        append_u32(&mut bytes, encoded.len() as u32);
        bytes.extend_from_slice(&encoded);
        apply_from_lifecycle_tail(&bytes, &mut source).unwrap();
        let loaded = source.host_object(id).unwrap();
        assert!(loaded.single_use_command_used);
        assert!(!loaded.script_targetable);
        assert!(!loaded.status.masked);
    }

    #[test]
    fn v12_object_payload_retains_script_status_without_structure_records() {
        let mut source = GameLogic::new();
        source.templates.insert(
            "V12ScriptOwner".into(),
            crate::game_logic::ThingTemplate::new("V12ScriptOwner"),
        );
        let id = source
            .create_object(
                "V12ScriptOwner",
                crate::game_logic::Team::USA,
                glam::Vec3::ZERO,
            )
            .unwrap();
        source.host_object_mut(id).unwrap().script_targetable = true;
        source.host_object_mut(id).unwrap().status.masked = true;
        let current = capture(&source);
        let old = ObjectXferPersistPayloadV12 {
            objects: current.objects,
            script_status: current.script_status,
        };
        let encoded = bincode_legacy::serialize(&old).unwrap();
        let mut bytes = OXOB_MAGIC.to_vec();
        append_u32(&mut bytes, 12);
        append_u32(&mut bytes, encoded.len() as u32);
        bytes.extend_from_slice(&encoded);
        apply_from_lifecycle_tail(&bytes, &mut source).unwrap();
        let object = source.host_object(id).unwrap();
        assert!(object.script_targetable && object.status.masked);
        assert!(object.structure_topple_data.is_none());
    }

    #[test]
    fn exact_v13_reader_preserves_topple_pose_and_uses_absent_damage_history_default() {
        let mut source = GameLogic::new();
        let mut template = crate::game_logic::ThingTemplate::new("V13ToppleOwner");
        template.add_kind_of(crate::game_logic::KindOf::Structure);
        template.authored_structure_topple = true;
        source.templates.insert(template.name.clone(), template);
        let id = source
            .create_object(
                "V13ToppleOwner",
                crate::game_logic::Team::USA,
                glam::Vec3::new(2.0, 0.0, 7.0),
            )
            .unwrap();
        let object = source.host_object_mut(id).unwrap();
        assert!(object.prepare_structure_topple_with_direction(
            13,
            None,
            Some((0.6, 0.8)),
            7,
            None,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        ));
        object.script_targetable = true;
        object.status.masked = true;
        object.last_damage_info_type = Some(crate::game_logic::combat::DamageType::Flame);
        let matrix = object.get_transform_matrix();
        let current = capture(&source);
        let old = ObjectXferPersistPayloadV13 {
            objects: current.objects,
            script_status: current.script_status,
            structures: current.structures,
        };
        let encoded = bincode_legacy::serialize(&old).unwrap();
        let mut bytes = OXOB_MAGIC.to_vec();
        append_u32(&mut bytes, 13);
        append_u32(&mut bytes, encoded.len() as u32);
        bytes.extend_from_slice(&encoded);
        apply_from_lifecycle_tail(&bytes, &mut source).unwrap();
        let object = source.host_object(id).unwrap();
        assert!(object.script_targetable && object.status.masked);
        assert_eq!(object.get_transform_matrix(), matrix);
        assert_eq!(
            object
                .structure_topple_data
                .as_ref()
                .unwrap()
                .topple_start_frame,
            20
        );
        assert_eq!(
            object.last_damage_info_type, None,
            "v13 did not save this original body field"
        );
    }

    #[test]
    fn malformed_body_history_owner_preserves_receiving_history_and_pose() {
        let mut receiving = GameLogic::new();
        receiving.templates.insert(
            "BodyHistoryOwner".into(),
            crate::game_logic::ThingTemplate::new("BodyHistoryOwner"),
        );
        let id = receiving
            .create_object(
                "BodyHistoryOwner",
                crate::game_logic::Team::USA,
                glam::Vec3::new(3.0, 0.0, 9.0),
            )
            .unwrap();
        receiving.host_object_mut(id).unwrap().last_damage_info_type =
            Some(crate::game_logic::combat::DamageType::Flame);
        let matrix = receiving.host_object(id).unwrap().get_transform_matrix();
        let mut payload = capture(&receiving);
        payload.damage_info.push(ObjectDamageInfoPersist {
            object_id: id.0,
            last_damage_type: None,
        });
        let encoded = bincode_legacy::serialize(&payload).unwrap();
        let mut bytes = OXOB_MAGIC.to_vec();
        append_u32(&mut bytes, OXOB_VERSION);
        append_u32(&mut bytes, encoded.len() as u32);
        bytes.extend_from_slice(&encoded);
        assert!(apply_from_lifecycle_tail(&bytes, &mut receiving).is_err());
        assert_eq!(
            receiving.host_object(id).unwrap().last_damage_info_type,
            Some(crate::game_logic::combat::DamageType::Flame)
        );
        assert_eq!(
            receiving.host_object(id).unwrap().get_transform_matrix(),
            matrix
        );
    }

    #[test]
    fn malformed_structure_matrix_rejects_before_receiving_state_changes() {
        let mut receiving = GameLogic::new();
        let mut template = crate::game_logic::ThingTemplate::new("ReceivingTopple");
        template.add_kind_of(crate::game_logic::KindOf::Structure);
        template.authored_structure_topple = true;
        receiving.templates.insert(template.name.clone(), template);
        let id = receiving
            .create_object(
                "ReceivingTopple",
                crate::game_logic::Team::USA,
                glam::Vec3::new(2.0, 0.0, 7.0),
            )
            .unwrap();
        assert!(
            receiving
                .host_object_mut(id)
                .unwrap()
                .begin_structure_topple(
                    0,
                    None,
                    &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new())
                )
        );
        receiving.host_object_mut(id).unwrap().script_targetable = true;
        let before = receiving.host_object(id).unwrap().get_transform_matrix();
        let mut payload = capture(&receiving);
        assert_eq!(payload.structures.len(), 1);
        payload.structures[0].matrix[0] = f32::NAN;
        let encoded = bincode_legacy::serialize(&payload).unwrap();
        let mut bytes = OXOB_MAGIC.to_vec();
        append_u32(&mut bytes, OXOB_VERSION);
        append_u32(&mut bytes, encoded.len() as u32);
        bytes.extend_from_slice(&encoded);
        assert!(apply_from_lifecycle_tail(&bytes, &mut receiving).is_err());
        let object = receiving.host_object(id).unwrap();
        assert!(object.script_targetable);
        assert!(object.structure_topple_data.is_some());
        assert_eq!(object.get_transform_matrix(), before);
    }

    #[test]
    fn malformed_object_payload_preserves_receiving_script_flags_and_clocks() {
        let mut receiving = GameLogic::new();
        let template = crate::game_logic::ThingTemplate::new("ReceivingScriptStatusOwner");
        receiving.templates.insert(template.name.clone(), template);
        let id = receiving
            .create_object(
                "ReceivingScriptStatusOwner",
                crate::game_logic::Team::USA,
                glam::Vec3::ZERO,
            )
            .unwrap();
        let object = receiving.host_object_mut(id).unwrap();
        object.script_targetable = true;
        object.status.masked = true;
        object.single_use_command_used = true;
        object.pre_attack_ready_at = 19.0;
        object.last_fire_frame = 223;

        // Unknown version, truncated declared bytes, and invalid serialized
        // data all fail during candidate decoding, before any object reset.
        for (version, declared_len, payload) in [
            (OXOB_VERSION + 1, 0, Vec::new()),
            (OXOB_VERSION, 2, vec![0]),
            (OXOB_VERSION, 0, Vec::new()),
        ] {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(OXOB_MAGIC);
            append_u32(&mut bytes, version);
            append_u32(&mut bytes, declared_len);
            bytes.extend_from_slice(&payload);
            assert!(apply_from_lifecycle_tail(&bytes, &mut receiving).is_err());
            let object = receiving.host_object(id).unwrap();
            assert!(object.script_targetable);
            assert!(object.status.masked);
            assert!(object.single_use_command_used);
            assert_eq!(object.pre_attack_ready_at, 19.0);
            assert_eq!(object.last_fire_frame, 223);
        }
    }
    use crate::game_logic::object::TurretSubState;
    use crate::game_logic::{Player, Team, ThingTemplate};
    use glam::Vec3;

    #[test]
    fn snapshot_round_trips_object_xfer_clocks() {
        let mut source = GameLogic::new();
        source.templates.insert(
            "AmericaInfantryRanger".to_string(),
            ThingTemplate::new("AmericaInfantryRanger"),
        );
        source.admit_receiving_module_content(crate::assets::ReceivingObjectContent::from_sources([
            ("native-clock-owner.ini".into(), "Object AmericaInfantryRanger\nKindOf = INFANTRY SELECTABLE\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = AIUpdateInterface ActualClockOwner\nEnd\nEnd\n".into()),
        ]).unwrap()).unwrap();
        source.add_player(Player::new(0, Team::USA, "USA", true));
        let id = source
            .create_object(
                "AmericaInfantryRanger",
                Team::USA,
                Vec3::new(10.0, 0.0, 8.0),
            )
            .expect("unit");
        {
            let object = source.host_object_mut(id).expect("unit obj");
            object.status.disabled_held = true;
            object.single_use_command_used = true;
            object.ai_attitude = -1;
            object.custom_indicator_color = Some(0xFFFF_0000);
            object.vision_range = 220.0;
            object.shroud_clearing_range = 240.0;
            object.shroud_range = 80.0;
            object.pre_attack_ready_at = 12.5;
            object.pre_attack_target = Some(ObjectId(9));
            object.consecutive_shot_target = Some(ObjectId(9));
            object.max_shots_to_fire = 3;
            object.turret_angle_deg = 45.0;
            object.turret_substate = TurretSubState::Aim;
            object.turret_target_id = Some(ObjectId(9));
            object.temporary_stealth_expires_frame = 600;
            object.weapon_bonus_solo = 16;
            object.which_exit_path = 2;
            object.cheer_timer = 1.5;
            object.model_condition_bits |=
                1u128 << crate::game_logic::host_enum_table_residual::special_cheering_model_bit();
            object.weapon_scatter_targets_unused = [vec![2, 0], vec![1], Vec::new()];
            object.weapon_scatter_targets_inited = [true, true, false];
            object.weapon_bonus_horde = true;
            object.weapon_bonus_enthusiastic = true;
            object.weapon_bonus_subliminal = true;
            object.safe_occlusion_frame = 12_345;
            object.health_box_offset = [12.5, -3.0, 4.25];
            object.last_fire_frame = 4_200;
            object.is_recruitable = false;
            object.ignore_collisions_until_frame = 88;
        }
        source
            .unit_ai_runtime_mut(id)
            .expect("unit")
            .set_guard_scan_deadline(Some(310));
        source
            .unit_ai_runtime_mut(id)
            .expect("unit")
            .set_hunt_scan_deadline(Some(640));
        source
            .unit_ai_runtime_mut(id)
            .expect("unit")
            .set_quick_exit_deadline(Some(930));

        let builder = super::super::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        assert!(
            find_oxob_suffix(&snapshot.lifecycle_tail).is_some(),
            "OXOB suffix must be appended to lifecycle tail"
        );

        let mut restored = GameLogic::new();
        restored.add_player(Player::new(0, Team::USA, "USA", true));
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        let loaded = restored.host_object(id).expect("restored unit");
        assert!(loaded.status.disabled_held);
        assert!(loaded.single_use_command_used);
        assert_eq!(loaded.ai_attitude, -1);
        assert_eq!(loaded.custom_indicator_color, Some(0xFFFF_0000));
        assert_eq!(loaded.vision_range, 220.0);
        assert_eq!(loaded.shroud_clearing_range, 240.0);
        assert_eq!(loaded.shroud_range, 80.0);
        assert!((loaded.pre_attack_ready_at - 12.5).abs() < 1e-4);
        assert_eq!(loaded.pre_attack_target, Some(ObjectId(9)));
        assert_eq!(loaded.consecutive_shot_target, Some(ObjectId(9)));
        assert_eq!(loaded.max_shots_to_fire, 3);
        assert!((loaded.turret_angle_deg - 45.0).abs() < 1e-4);
        assert_eq!(loaded.turret_substate, TurretSubState::Aim);
        assert_eq!(loaded.turret_target_id, Some(ObjectId(9)));
        assert_eq!(loaded.temporary_stealth_expires_frame, 600);
        assert_eq!(loaded.weapon_bonus_solo, 16);
        assert!(
            loaded.is_receiving_difficulty_bonus,
            "m_isReceivingDifficultyBonus latch follows restored SOLO bits"
        );
        assert_eq!(loaded.which_exit_path, 2);
        assert!((loaded.cheer_timer - 1.5).abs() < 1e-4);
        assert!(
            object_has_special_cheering(&loaded),
            "SPECIAL_CHEERING must survive load"
        );
        assert_eq!(
            loaded.weapon_scatter_targets_unused,
            [vec![2, 0], vec![1], Vec::new()]
        );
        assert_eq!(loaded.weapon_scatter_targets_inited, [true, true, false]);
        assert!(loaded.weapon_bonus_horde);
        assert!(loaded.weapon_bonus_enthusiastic);
        assert!(loaded.weapon_bonus_subliminal);
        assert_eq!(loaded.safe_occlusion_frame, 12_345);
        assert_eq!(loaded.health_box_offset, [12.5, -3.0, 4.25]);
        assert_eq!(loaded.last_fire_frame, 4_200);
        assert!(
            !loaded.is_recruitable,
            "m_isRecruitable=false must survive load"
        );
        assert_eq!(loaded.ignore_collisions_until_frame, 88);
        assert_eq!(
            restored
                .unit_ai_runtime(id)
                .and_then(|runtime| runtime.guard_scan_deadline()),
            Some(310)
        );
        assert_eq!(
            restored
                .unit_ai_runtime(id)
                .and_then(|runtime| runtime.hunt_scan_deadline()),
            Some(640)
        );
        assert_eq!(
            restored
                .unit_ai_runtime(id)
                .and_then(|runtime| runtime.quick_exit_deadline()),
            Some(930),
            "quick-exit deadline must survive SnapshotBuilder/OXOB restore"
        );
    }

    #[test]
    fn absent_suffix_clears_stale_object_xfer() {
        let mut logic = GameLogic::new();
        logic.templates.insert(
            "AmericaInfantryRanger".to_string(),
            ThingTemplate::new("AmericaInfantryRanger"),
        );
        logic.add_player(Player::new(0, Team::USA, "USA", true));
        let id = logic
            .create_object("AmericaInfantryRanger", Team::USA, Vec3::ZERO)
            .expect("unit");
        {
            let object = logic.host_object_mut(id).expect("unit");
            object.status.disabled_held = true;
            object.single_use_command_used = true;
            object.custom_indicator_color = Some(0xFF00_FF00);
            object.pre_attack_ready_at = 4.0;
            object.pre_attack_target = Some(ObjectId(2));
            object.consecutive_shot_target = Some(ObjectId(2));
            object.max_shots_to_fire = 1;
            object.weapon_bonus_solo = 18;
            object.which_exit_path = 3;
            object.cheer_timer = 2.0;
            object.model_condition_bits |=
                1u128 << crate::game_logic::host_enum_table_residual::special_cheering_model_bit();
            object.weapon_scatter_targets_unused = [vec![3], Vec::new(), Vec::new()];
            object.weapon_scatter_targets_inited = [true, false, false];
            object.weapon_bonus_horde = true;
            object.weapon_bonus_enthusiastic = true;
            object.weapon_bonus_subliminal = true;
            object.health_box_offset = [1.0, 2.0, 3.0];
            object.last_fire_frame = 99;
            object.is_recruitable = false;
            object.ignore_collisions_until_frame = 44;
        }
        logic
            .unit_ai_runtime_mut(id)
            .expect("unit")
            .set_guard_scan_deadline(Some(12));
        logic
            .unit_ai_runtime_mut(id)
            .expect("unit")
            .set_hunt_scan_deadline(Some(24));
        logic
            .unit_ai_runtime_mut(id)
            .expect("unit")
            .set_quick_exit_deadline(Some(930));
        apply_from_lifecycle_tail(b"no-magic-here", &mut logic).expect("apply");
        let object = logic.host_object(id).expect("unit");
        assert!(!object.status.disabled_held);
        assert!(!object.single_use_command_used);
        assert!(!object.is_receiving_difficulty_bonus);
        assert_eq!(object.custom_indicator_color, None);
        assert_eq!(object.pre_attack_ready_at, 0.0);
        assert_eq!(object.pre_attack_target, None);
        assert_eq!(object.consecutive_shot_target, None);
        assert_eq!(object.max_shots_to_fire, -1);
        assert_eq!(object.weapon_bonus_solo, 0);
        assert_eq!(object.which_exit_path, 0);
        assert_eq!(object.cheer_timer, 0.0);
        assert!(!object_has_special_cheering(&object));
        assert!(
            object
                .weapon_scatter_targets_unused
                .iter()
                .all(Vec::is_empty)
        );
        assert_eq!(object.weapon_scatter_targets_inited, [false, false, false]);
        assert!(!object.weapon_bonus_horde);
        assert!(!object.weapon_bonus_enthusiastic);
        assert!(!object.weapon_bonus_subliminal);
        assert_eq!(object.health_box_offset, [0.0, 0.0, 0.0]);
        assert_eq!(object.last_fire_frame, 0);
        assert!(
            object.is_recruitable,
            "absent OXOB resets m_isRecruitable to leftover default true"
        );
        assert_eq!(object.ignore_collisions_until_frame, 0);
        assert!(
            logic
                .host_objects()
                .registered_values()
                .all(|object| object.unit_ai_runtime.guard_scan_deadline() == Some(12))
        );
        assert!(
            logic
                .host_objects()
                .registered_values()
                .all(|object| object.unit_ai_runtime.hunt_scan_deadline() == Some(24))
        );
        assert!(
            logic
                .host_objects()
                .registered_values()
                .all(|object| object.unit_ai_runtime.quick_exit_deadline() == Some(930))
        );
    }

    #[test]
    fn oxob_round_trips_indicator_vision_single_use_preattack() {
        let mut source = GameLogic::new();
        source.templates.insert(
            "AmericaInfantryRanger".to_string(),
            ThingTemplate::new("AmericaInfantryRanger"),
        );
        source.add_player(Player::new(0, Team::USA, "USA", true));
        let id = source
            .create_object("AmericaInfantryRanger", Team::USA, Vec3::ZERO)
            .expect("unit");
        {
            let object = source.host_object_mut(id).expect("unit");
            object.single_use_command_used = true;
            object.custom_indicator_color = Some(0xFFFF_0000);
            object.vision_range = 220.0;
            object.shroud_clearing_range = 240.0;
            object.shroud_range = 80.0;
            object.pre_attack_ready_at = 12.5;
            object.pre_attack_target = Some(ObjectId(9));
            object.consecutive_shot_target = Some(ObjectId(9));
            object.max_shots_to_fire = 3;
        }

        let mut bytes = Vec::new();
        append_to_lifecycle_tail(&mut bytes, &source);
        assert!(
            find_oxob_suffix(&bytes).is_some(),
            "OXOB suffix must encode indicator/vision/single-use/pre-attack"
        );

        let mut dest = GameLogic::new();
        dest.templates = source.templates.clone();
        dest.add_player(Player::new(0, Team::USA, "USA", true));
        let dest_id = dest
            .create_object("AmericaInfantryRanger", Team::USA, Vec3::ZERO)
            .expect("dest unit");
        assert_eq!(dest_id, id);
        dest.host_object_mut(dest_id)
            .unwrap()
            .restore_base_ai_continuation(
                source
                    .host_object(id)
                    .unwrap()
                    .capture_base_ai_continuation(),
            )
            .unwrap();
        apply_from_lifecycle_tail(&bytes, &mut dest).expect("apply");

        let loaded = dest.host_object(dest_id).expect("loaded");
        assert!(loaded.single_use_command_used);
        assert_eq!(loaded.custom_indicator_color, Some(0xFFFF_0000));
        assert_eq!(loaded.vision_range, 220.0);
        assert_eq!(loaded.shroud_clearing_range, 240.0);
        assert_eq!(loaded.shroud_range, 80.0);
        assert!((loaded.pre_attack_ready_at - 12.5).abs() < 1e-4);
        assert_eq!(loaded.pre_attack_target, Some(ObjectId(9)));
        assert_eq!(loaded.consecutive_shot_target, Some(ObjectId(9)));
        assert_eq!(loaded.max_shots_to_fire, 3);
    }

    #[test]
    fn oxob_round_trips_cheer_scatter_horde_bonus() {
        let mut source = GameLogic::new();
        source.templates.insert(
            "AmericaInfantryRanger".to_string(),
            ThingTemplate::new("AmericaInfantryRanger"),
        );
        source.add_player(Player::new(0, Team::USA, "USA", true));
        let id = source
            .create_object("AmericaInfantryRanger", Team::USA, Vec3::ZERO)
            .expect("unit");
        {
            let object = source.host_object_mut(id).expect("unit");
            object.cheer_timer = 2.25;
            object.model_condition_bits |=
                1u128 << crate::game_logic::host_enum_table_residual::special_cheering_model_bit();
            object.weapon_scatter_targets_unused = [vec![4, 1, 0], Vec::new(), vec![2]];
            object.weapon_scatter_targets_inited = [true, false, true];
            object.weapon_bonus_horde = true;
            object.weapon_bonus_enthusiastic = true;
            object.weapon_bonus_subliminal = true;
        }

        let mut bytes = Vec::new();
        append_to_lifecycle_tail(&mut bytes, &source);
        assert!(find_oxob_suffix(&bytes).is_some());

        let mut dest = GameLogic::new();
        dest.templates = source.templates.clone();
        dest.add_player(Player::new(0, Team::USA, "USA", true));
        let dest_id = dest
            .create_object("AmericaInfantryRanger", Team::USA, Vec3::ZERO)
            .expect("dest unit");
        apply_from_lifecycle_tail(&bytes, &mut dest).expect("apply");

        let loaded = dest.host_object(dest_id).expect("loaded");
        assert!((loaded.cheer_timer - 2.25).abs() < 1e-4);
        assert!(object_has_special_cheering(&loaded));
        assert_eq!(
            loaded.weapon_scatter_targets_unused,
            [vec![4, 1, 0], Vec::new(), vec![2]]
        );
        assert_eq!(loaded.weapon_scatter_targets_inited, [true, false, true]);
        assert!(loaded.weapon_bonus_horde);
        assert!(loaded.weapon_bonus_enthusiastic);
        assert!(loaded.weapon_bonus_subliminal);
    }

    #[test]
    fn oxob_round_trips_safe_occlusion_frame() {
        let mut source = GameLogic::new();
        source.templates.insert(
            "AmericaInfantryRanger".to_string(),
            ThingTemplate::new("AmericaInfantryRanger"),
        );
        source.add_player(Player::new(0, Team::USA, "USA", true));
        let id = source
            .create_object("AmericaInfantryRanger", Team::USA, Vec3::ZERO)
            .expect("unit");
        {
            let object = source.host_object_mut(id).expect("unit");
            // Garrison/tunnel exit stamp (now + delay) or HUGE_FRAME leftover.
            object.safe_occlusion_frame = 30_090;
        }

        let mut bytes = Vec::new();
        append_to_lifecycle_tail(&mut bytes, &source);
        assert!(find_oxob_suffix(&bytes).is_some());

        let mut dest = GameLogic::new();
        dest.templates = source.templates.clone();
        dest.add_player(Player::new(0, Team::USA, "USA", true));
        let dest_id = dest
            .create_object("AmericaInfantryRanger", Team::USA, Vec3::ZERO)
            .expect("dest unit");
        {
            let object = dest.host_object_mut(dest_id).expect("dest");
            object.safe_occlusion_frame = 90;
        }
        apply_from_lifecycle_tail(&bytes, &mut dest).expect("apply");

        let loaded = dest.host_object(dest_id).expect("loaded");
        assert_eq!(
            loaded.safe_occlusion_frame, 30_090,
            "m_safeOcclusionFrame must survive load"
        );
    }

    #[test]
    fn absent_suffix_keeps_template_safe_occlusion_frame() {
        let mut logic = GameLogic::new();
        logic.templates.insert(
            "AmericaInfantryRanger".to_string(),
            ThingTemplate::new("AmericaInfantryRanger"),
        );
        logic.add_player(Player::new(0, Team::USA, "USA", true));
        let id = logic
            .create_object("AmericaInfantryRanger", Team::USA, Vec3::ZERO)
            .expect("unit");
        {
            let object = logic.host_object_mut(id).expect("unit");
            object.safe_occlusion_frame = 777;
        }
        apply_from_lifecycle_tail(b"no-magic-here", &mut logic).expect("apply");
        let object = logic.host_object(id).expect("unit");
        assert_eq!(
            object.safe_occlusion_frame, 777,
            "absent OXOB must not wipe template/garrison occlusion clock"
        );
    }

    #[test]
    fn oxob_round_trips_health_box_last_fire_recruit_scan_ignore() {
        let mut source = GameLogic::new();
        source.templates.insert(
            "AmericaInfantryRanger".to_string(),
            ThingTemplate::new("AmericaInfantryRanger"),
        );
        source.add_player(Player::new(0, Team::USA, "USA", true));
        let id = source
            .create_object("AmericaInfantryRanger", Team::USA, Vec3::ZERO)
            .expect("unit");
        {
            let object = source.host_object_mut(id).expect("unit");
            object.health_box_offset = [8.0, 20.0, -1.5];
            object.last_fire_frame = 77;
            object.is_recruitable = false;
            object.ignore_collisions_until_frame = 150;
            object.locomotor_upgrade = true;
            object.jet_ai.cur_locomotor_set = Some("SET_NORMAL_UPGRADED".to_string());
            object.braking = 40.0;
            object.max_lift = 9.5;
            object.braking_factor = 0.25;
            object.close_enough_dist = Some(25.0);
            object.close_enough_dist_3d = true;
            object.donut_timer = 80;
            object.loco_preferred_height = 12.0;
            object.loco_preferred_height_damping = 0.4;
            object.is_braking = true;
            object.is_climbing = true;
            object.doing_three_point_turn = true;
            object.cur_locomotor_name = Some("WorkerShoes".to_string());
            object.locomotor_surfaces = 0x11;
            object.downhill_only = true;
            object.maintain_pos_valid = true;
            object.maintain_pos = Some(glam::Vec3::new(3.0, 1.0, 4.0));
            object.wander_angle_offset = 0.7;
            object.wander_offset_increment = 0.15;
            object.wander_offset_increasing = true;
        }
        source
            .unit_ai_runtime_mut(id)
            .expect("unit")
            .set_guard_scan_deadline(Some(90));
        source
            .unit_ai_runtime_mut(id)
            .expect("unit")
            .set_hunt_scan_deadline(Some(120));

        let mut bytes = Vec::new();
        append_to_lifecycle_tail(&mut bytes, &source);
        assert!(find_oxob_suffix(&bytes).is_some());

        let mut dest = GameLogic::new();
        dest.templates = source.templates.clone();
        dest.add_player(Player::new(0, Team::USA, "USA", true));
        let dest_id = dest
            .create_object("AmericaInfantryRanger", Team::USA, Vec3::ZERO)
            .expect("dest unit");
        {
            let object = dest.host_object_mut(dest_id).expect("dest");
            object.health_box_offset = [99.0, 99.0, 99.0];
            object.last_fire_frame = 1;
            object.is_recruitable = true;
            object.ignore_collisions_until_frame = 3;
            object.locomotor_surfaces = 0x40;
            object.cur_locomotor_name = Some("CreateTime".to_string());
        }
        dest.unit_ai_runtime_mut(dest_id)
            .expect("unit")
            .set_guard_scan_deadline(Some(1));
        dest.unit_ai_runtime_mut(dest_id)
            .expect("unit")
            .set_hunt_scan_deadline(Some(2));
        apply_from_lifecycle_tail(&bytes, &mut dest).expect("apply");

        let loaded = dest.host_object(dest_id).expect("loaded");
        assert_eq!(
            loaded.health_box_offset,
            [8.0, 20.0, -1.5],
            "m_healthBoxOffset must survive load"
        );
        assert_eq!(
            loaded.last_fire_frame, 77,
            "Weapon::m_lastFireFrame must survive load"
        );
        assert!(
            !loaded.is_recruitable,
            "AIUpdateInterface::m_isRecruitable=false must survive load"
        );
        assert_eq!(
            loaded.ignore_collisions_until_frame, 150,
            "m_ignoreCollisionsUntil must survive load"
        );
        assert!(loaded.locomotor_upgrade);
        assert_eq!(
            loaded.jet_ai.cur_locomotor_set.as_deref(),
            Some("SET_NORMAL_UPGRADED")
        );
        assert!((loaded.braking - 40.0).abs() < 0.001);
        assert!((loaded.max_lift - 9.5).abs() < 0.001);
        assert!((loaded.braking_factor - 0.25).abs() < 0.001);
        assert_eq!(loaded.close_enough_dist, Some(25.0));
        assert!(loaded.close_enough_dist_3d);
        assert_eq!(loaded.donut_timer, 80);
        assert!((loaded.loco_preferred_height - 12.0).abs() < 0.001);
        assert!((loaded.loco_preferred_height_damping - 0.4).abs() < 0.001);
        assert!(loaded.is_braking);
        assert!(loaded.is_climbing);
        assert!(loaded.doing_three_point_turn);
        assert_eq!(loaded.cur_locomotor_name.as_deref(), Some("WorkerShoes"));
        assert_eq!(loaded.locomotor_surfaces, 0x11);
        assert!(loaded.downhill_only);
        assert!(loaded.maintain_pos_valid);
        assert_eq!(loaded.maintain_pos, Some(glam::Vec3::new(3.0, 1.0, 4.0)));
        assert!((loaded.wander_angle_offset - 0.7).abs() < 0.001);
        assert!((loaded.wander_offset_increment - 0.15).abs() < 0.001);
        assert!(loaded.wander_offset_increasing);
        assert_eq!(
            dest.unit_ai_runtime(dest_id)
                .and_then(|runtime| runtime.guard_scan_deadline()),
            Some(90),
            "AIGuardIdleState scan clock must survive load"
        );
        assert_eq!(
            dest.unit_ai_runtime(dest_id)
                .and_then(|runtime| runtime.hunt_scan_deadline()),
            Some(120),
            "AIHuntState scan clock must survive load"
        );
    }

    #[test]
    fn oxob_v5_suffix_clears_the_locomotor_upgrade() {
        let mut logic = GameLogic::new();
        logic.templates.insert(
            "AmericaInfantryRanger".to_string(),
            ThingTemplate::new("AmericaInfantryRanger"),
        );
        logic.add_player(Player::new(0, Team::USA, "USA", true));
        let id = logic
            .create_object("AmericaInfantryRanger", Team::USA, Vec3::ZERO)
            .expect("unit");
        {
            let object = logic.host_object_mut(id).expect("unit");
            object.locomotor_upgrade = true;
            object.jet_ai.cur_locomotor_set = Some("SET_TAXIING".to_string());
        }
        let v5 = ObjectXferPersistPayloadV5 {
            objects: vec![ObjectXferPersistV5 {
                object_id: id.0,
                ..Default::default()
            }],
        };
        let encoded = bincode_legacy::serialize(&v5).expect("v5 encode");
        let mut bytes = Vec::new();
        bytes.extend_from_slice(OXOB_MAGIC);
        append_u32(&mut bytes, 5);
        append_u32(&mut bytes, encoded.len() as u32);
        bytes.extend_from_slice(&encoded);
        apply_from_lifecycle_tail(&bytes, &mut logic).expect("apply v5");
        let loaded = logic.host_object(id).expect("unit");
        assert!(!loaded.locomotor_upgrade);
        assert!(loaded.jet_ai.cur_locomotor_set.is_none());
    }

    #[test]
    fn oxob_v6_suffix_keeps_create_time_braking() {
        let mut logic = GameLogic::new();
        logic.templates.insert(
            "AmericaInfantryRanger".to_string(),
            ThingTemplate::new("AmericaInfantryRanger"),
        );
        logic.add_player(Player::new(0, Team::USA, "USA", true));
        let id = logic
            .create_object("AmericaInfantryRanger", Team::USA, Vec3::ZERO)
            .expect("unit");
        {
            let object = logic.host_object_mut(id).expect("unit");
            object.braking = 40.0;
            object.max_lift = 9.5;
            object.braking_factor = 0.25;
            object.locomotor_upgrade = true;
            object.jet_ai.cur_locomotor_set = Some("SET_TAXIING".to_string());
        }
        let v6 = ObjectXferPersistPayloadV6 {
            objects: vec![ObjectXferPersistV6 {
                object_id: id.0,
                locomotor_upgrade: true,
                cur_locomotor_set: Some("SET_TAXIING".to_string()),
                ..Default::default()
            }],
        };
        let encoded = bincode_legacy::serialize(&v6).expect("v6 encode");
        let mut bytes = Vec::new();
        bytes.extend_from_slice(OXOB_MAGIC);
        append_u32(&mut bytes, 6);
        append_u32(&mut bytes, encoded.len() as u32);
        bytes.extend_from_slice(&encoded);
        apply_from_lifecycle_tail(&bytes, &mut logic).expect("apply v6");
        let loaded = logic.host_object(id).expect("unit");
        assert!(loaded.locomotor_upgrade);
        assert_eq!(
            loaded.jet_ai.cur_locomotor_set.as_deref(),
            Some("SET_TAXIING")
        );
        assert!((loaded.braking - 40.0).abs() < 0.001);
        assert!((loaded.max_lift - 9.5).abs() < 0.001);
        assert!((loaded.braking_factor - 0.25).abs() < 0.001);
    }

    #[test]
    fn oxob_v7_suffix_keeps_create_time_stopping_distance() {
        let mut logic = GameLogic::new();
        logic.templates.insert(
            "AmericaInfantryRanger".to_string(),
            ThingTemplate::new("AmericaInfantryRanger"),
        );
        logic.add_player(Player::new(0, Team::USA, "USA", true));
        let id = logic
            .create_object("AmericaInfantryRanger", Team::USA, Vec3::ZERO)
            .expect("unit");
        {
            let object = logic.host_object_mut(id).expect("unit");
            object.close_enough_dist = Some(25.0);
            object.close_enough_dist_3d = true;
            object.donut_timer = 80;
            object.loco_preferred_height = 12.0;
            object.loco_preferred_height_damping = 0.4;
            object.braking = 40.0;
        }
        let v7 = ObjectXferPersistPayloadV7 {
            objects: vec![ObjectXferPersistV7 {
                object_id: id.0,
                has_locomotor_motion: true,
                braking: 40.0,
                ..Default::default()
            }],
        };
        let encoded = bincode_legacy::serialize(&v7).expect("v7 encode");
        let mut bytes = Vec::new();
        bytes.extend_from_slice(OXOB_MAGIC);
        append_u32(&mut bytes, 7);
        append_u32(&mut bytes, encoded.len() as u32);
        bytes.extend_from_slice(&encoded);
        apply_from_lifecycle_tail(&bytes, &mut logic).expect("apply v7");
        let loaded = logic.host_object(id).expect("unit");
        assert!((loaded.braking - 40.0).abs() < 0.001);
        assert_eq!(loaded.close_enough_dist, Some(25.0));
        assert!(loaded.close_enough_dist_3d);
        assert_eq!(loaded.donut_timer, 80);
        assert!((loaded.loco_preferred_height - 12.0).abs() < 0.001);
        assert!((loaded.loco_preferred_height_damping - 0.4).abs() < 0.001);
    }

    #[test]
    fn oxob_v8_suffix_keeps_create_time_climb_and_wander() {
        let mut logic = GameLogic::new();
        logic.templates.insert(
            "AmericaInfantryRanger".to_string(),
            ThingTemplate::new("AmericaInfantryRanger"),
        );
        logic.add_player(Player::new(0, Team::USA, "USA", true));
        let id = logic
            .create_object("AmericaInfantryRanger", Team::USA, Vec3::ZERO)
            .expect("unit");
        {
            let object = logic.host_object_mut(id).expect("unit");
            object.is_climbing = true;
            object.wander_angle_offset = 0.7;
            object.maintain_pos = Some(glam::Vec3::new(3.0, 1.0, 4.0));
            object.close_enough_dist = Some(25.0);
        }
        let v8 = ObjectXferPersistPayloadV8 {
            objects: vec![ObjectXferPersistV8 {
                object_id: id.0,
                has_locomotor_arrival: true,
                close_enough_dist: Some(25.0),
                ..Default::default()
            }],
        };
        let encoded = bincode_legacy::serialize(&v8).expect("v8 encode");
        let mut bytes = Vec::new();
        bytes.extend_from_slice(OXOB_MAGIC);
        append_u32(&mut bytes, 8);
        append_u32(&mut bytes, encoded.len() as u32);
        bytes.extend_from_slice(&encoded);
        apply_from_lifecycle_tail(&bytes, &mut logic).expect("apply v8");
        let loaded = logic.host_object(id).expect("unit");
        assert_eq!(loaded.close_enough_dist, Some(25.0));
        assert!(loaded.is_climbing);
        assert!((loaded.wander_angle_offset - 0.7).abs() < 0.001);
        assert_eq!(loaded.maintain_pos, Some(glam::Vec3::new(3.0, 1.0, 4.0)));
    }

    #[test]
    fn oxob_v9_suffix_keeps_create_time_locomotor_name() {
        let mut logic = GameLogic::new();
        logic.templates.insert(
            "AmericaInfantryRanger".to_string(),
            ThingTemplate::new("AmericaInfantryRanger"),
        );
        logic.add_player(Player::new(0, Team::USA, "USA", true));
        let id = logic
            .create_object("AmericaInfantryRanger", Team::USA, Vec3::ZERO)
            .expect("unit");
        {
            let object = logic.host_object_mut(id).expect("unit");
            object.cur_locomotor_name = Some("KeepMe".to_string());
            object.locomotor_surfaces = 0x40;
            object.is_climbing = true;
        }
        let v9 = ObjectXferPersistPayloadV9 {
            objects: vec![ObjectXferPersistV9 {
                object_id: id.0,
                has_locomotor_pose: true,
                loco_flags: 1 << 9,
                ..Default::default()
            }],
        };
        let encoded = bincode_legacy::serialize(&v9).expect("v9 encode");
        let mut bytes = Vec::new();
        bytes.extend_from_slice(OXOB_MAGIC);
        append_u32(&mut bytes, 9);
        append_u32(&mut bytes, encoded.len() as u32);
        bytes.extend_from_slice(&encoded);
        apply_from_lifecycle_tail(&bytes, &mut logic).expect("apply v9");
        let loaded = logic.host_object(id).expect("unit");
        assert!(loaded.is_climbing);
        assert_eq!(loaded.cur_locomotor_name.as_deref(), Some("KeepMe"));
        assert_eq!(loaded.locomotor_surfaces, 0x40);
    }

    #[test]
    fn oxob_v3_suffix_defaults_new_leftover_fields() {
        let v3 = ObjectXferPersistPayloadV3 {
            objects: vec![ObjectXferPersistV3 {
                object_id: 1,
                disabled_held: false,
                single_use_command_used: false,
                ai_attitude: 0,
                custom_indicator_color: None,
                vision_range: 150.0,
                shroud_clearing_range: 150.0,
                shroud_range: 0.0,
                pre_attack_ready_at: 0.0,
                pre_attack_target: None,
                consecutive_shot_target: None,
                max_shots_to_fire: -1,
                turret_angle_deg: 0.0,
                turret_pitch_deg: 0.0,
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
                turret_enabled: true,
                turret_substate: 0,
                turret_rotating: false,
                temporary_stealth_expires_frame: 0,
                weapon_bonus_solo: 0,
                which_exit_path: 0,
                cheer_timer: 0.0,
                special_cheering: false,
                weapon_scatter_targets_unused: Default::default(),
                weapon_scatter_targets_inited: [false; 3],
                weapon_bonus_horde: false,
                weapon_bonus_enthusiastic: false,
                weapon_bonus_subliminal: false,
                safe_occlusion_frame: 12,
            }],
        };
        let encoded = bincode_legacy::serialize(&v3).expect("v3 encode");
        let mut bytes = Vec::new();
        bytes.extend_from_slice(OXOB_MAGIC);
        append_u32(&mut bytes, 3);
        append_u32(&mut bytes, encoded.len() as u32);
        bytes.extend_from_slice(&encoded);

        let mut dest = GameLogic::new();
        dest.templates.insert(
            "AmericaInfantryRanger".to_string(),
            ThingTemplate::new("AmericaInfantryRanger"),
        );
        dest.add_player(Player::new(0, Team::USA, "USA", true));
        let dest_id = dest
            .create_object("AmericaInfantryRanger", Team::USA, Vec3::ZERO)
            .expect("dest unit");
        assert_eq!(dest_id.0, 1);
        {
            let object = dest.host_object_mut(dest_id).expect("dest");
            object.health_box_offset = [5.0, 5.0, 5.0];
            object.last_fire_frame = 9;
            object.is_recruitable = false;
            object.ignore_collisions_until_frame = 11;
        }
        dest.unit_ai_runtime_mut(dest_id)
            .expect("unit")
            .set_guard_scan_deadline(Some(4));
        dest.unit_ai_runtime_mut(dest_id)
            .expect("unit")
            .set_hunt_scan_deadline(Some(5));
        apply_from_lifecycle_tail(&bytes, &mut dest).expect("apply v3");

        let loaded = dest.host_object(dest_id).expect("loaded");
        assert_eq!(loaded.safe_occlusion_frame, 12);
        assert_eq!(loaded.health_box_offset, [0.0, 0.0, 0.0]);
        assert_eq!(loaded.last_fire_frame, 0);
        assert!(loaded.is_recruitable);
        assert_eq!(loaded.ignore_collisions_until_frame, 0);
        assert!(
            dest.host_objects()
                .registered_values()
                .all(|object| object.unit_ai_runtime.guard_scan_deadline() == Some(4))
        );
        assert!(
            dest.host_objects()
                .registered_values()
                .all(|object| object.unit_ai_runtime.hunt_scan_deadline() == Some(5))
        );
    }
}

#[cfg(test)]
#[path = "locomotor_restore_tests.rs"]
mod locomotor_restore_tests;

/// Current OO deadline fields are immutable projections. Validate only after
/// native14 has been installed and all physical/definition tails have finished.
pub(super) fn validate_base_ai_projection_after_restore(
    bytes: &[u8],
    game_logic: &GameLogic,
) -> SaveLoadResult<()> {
    let Some(mut rest) = find_oxob_suffix(bytes) else {
        return Ok(());
    };
    let version = take_u32(&mut rest)?;
    if version != OXOB_VERSION {
        return Err(SaveLoadError::Corrupted(
            "native14 requires current OO projection layout".into(),
        ));
    }
    let length = take_u32(&mut rest)? as usize;
    let encoded = rest
        .get(..length)
        .ok_or_else(|| SaveLoadError::Corrupted("truncated OO projection".into()))?;
    let payload: ObjectXferPersistPayload = bincode_legacy::deserialize(encoded)
        .map_err(|e| SaveLoadError::Corrupted(format!("OO projection decode: {e}")))?;
    for entry in &payload.objects {
        let runtime = game_logic
            .unit_ai_runtime(ObjectId(entry.object_id))
            .ok_or_else(|| SaveLoadError::Corrupted("missing OO projection owner".into()))?;
        if runtime.guard_scan_deadline() != entry.guard_next_enemy_scan
            || runtime.hunt_scan_deadline() != entry.hunt_next_enemy_scan
            || runtime.quick_exit_deadline() != entry.quick_exit_until
        {
            return Err(SaveLoadError::Corrupted(
                "OO deadline projection disagrees with native14 owner".into(),
            ));
        }
    }
    transport_movement_persist::validate_base_ai_projection_after_restore(
        game_logic,
        &payload.transport,
    )
}

pub(super) fn validate_base_ai_projection_before_restore(
    bytes: &[u8],
    snapshot: &super::WorldSnapshot,
) -> SaveLoadResult<()> {
    let Some(mut rest) = find_oxob_suffix(bytes) else {
        return Ok(());
    };
    let version = take_u32(&mut rest)?;
    if version != OXOB_VERSION {
        return Err(SaveLoadError::Corrupted(
            "native14 requires current OO projection layout".into(),
        ));
    }
    let length = take_u32(&mut rest)? as usize;
    let encoded = rest
        .get(..length)
        .ok_or_else(|| SaveLoadError::Corrupted("truncated OO projection".into()))?;
    let payload: ObjectXferPersistPayload = bincode_legacy::deserialize(encoded)
        .map_err(|e| SaveLoadError::Corrupted(format!("OO projection decode: {e}")))?;
    for entry in &payload.objects {
        let saved = snapshot
            .objects
            .get(&ObjectId(entry.object_id))
            .ok_or_else(|| SaveLoadError::Corrupted("missing detached OO owner".into()))?;
        let control = match saved.modules.get("ReceivingBaseAI") {
            Some(super::ModuleSnapshot::BaseAi(owner)) => owner.continuation(),
            _ => {
                return Err(SaveLoadError::Corrupted(
                    "missing detached BaseAI capsule".into(),
                ));
            }
        };
        let expected = control.map_or((None, None, None), |control| control.deadline_projection());
        if expected
            != (
                entry.guard_next_enemy_scan,
                entry.hunt_next_enemy_scan,
                entry.quick_exit_until,
            )
        {
            return Err(SaveLoadError::Corrupted(
                "detached OO deadline projection disagrees with native14 owner".into(),
            ));
        }
    }
    transport_movement_persist::validate_base_ai_projection_before_restore(
        &payload.transport,
        snapshot,
    )
}
