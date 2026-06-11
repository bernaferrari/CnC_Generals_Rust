//! AIUpdateInterface – central bridge between AI state machines and game objects.
//!
//! Ported from C++ AIUpdate.h / AIUpdate.cpp (5280 lines).
//! This file contains:
//!   - AIUpdateModuleData: INI-parsed module data
//!   - AIUpdateInterface: runtime AI state + ~30 bridge methods for pathfinding,
//!     locomotion, turret control, waypoints, collision, and state management
//!   - AIUpdateInterfaceModule: thin module wrapper for the module system

use std::any::Any;
use std::sync::{Arc, Mutex};

use game_engine::common::ini::{FieldParse, INIError, INILoadType, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

use std::collections::HashMap;

use crate::ai::turret::TurretAI;
use crate::common::{
    AsciiString, Bool, Coord3D, ICoord2D, Int, LocomotorSetType, ObjectID, Real, UnsignedInt,
    WhichTurretType, INVALID_ID, LOGICFRAMES_PER_SECOND, WEAPONSLOT_COUNT,
};
use crate::helpers::TheGameLogic;
use crate::object::registry::OBJECT_REGISTRY;
use crate::weapon::WeaponSlotType;

// ---------------------------------------------------------------------------
// Constants from C++ AIUpdate.h
// ---------------------------------------------------------------------------

/// Matches C++ FAST_AS_POSSIBLE in AIUpdate.h.
pub const AI_FAST_AS_POSSIBLE: Real = 999_999.0;

/// Maximum waypoints in the planning-mode queue (C++ MAX_WAYPOINTS = 16).
pub const MAX_WAYPOINTS: usize = 16;

/// Maximum turrets per unit (C++ MAX_TURRETS = 2).
pub const MAX_TURRETS: usize = 2;

// ---------------------------------------------------------------------------
// Auto-acquire bit flags (C++ AutoAcquireStates)
// ---------------------------------------------------------------------------

pub const AUTO_ACQUIRE_IDLE: u32 = 0x01;
pub const AUTO_ACQUIRE_IDLE_STEALTHED: u32 = 0x02;
pub const AUTO_ACQUIRE_IDLE_NO: u32 = 0x04;
pub const AUTO_ACQUIRE_IDLE_NOT_WHILE_ATTACKING: u32 = 0x08;
pub const AUTO_ACQUIRE_IDLE_ATTACK_BUILDINGS: u32 = 0x10;

const AUTO_ACQUIRE_ENEMIES_NAMES: &[&str] = &[
    "YES",
    "STEALTHED",
    "NO",
    "NOTWHILEATTACKING",
    "ATTACK_BUILDINGS",
];

const WEAPON_SLOT_NAMES: &[&str] = &["PRIMARY", "SECONDARY", "TERTIARY"];

// ---------------------------------------------------------------------------
// Locomotor goal type (C++ LocoGoalType in AIUpdate.h, saved in xfer)
// ---------------------------------------------------------------------------

/// Locomotor goal type – written to save files so numeric values must not change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum LocoGoalType {
    None = 0,
    PositionOnPath = 1,
    PositionExplicit = 2,
    Angle = 3,
}

impl Default for LocoGoalType {
    fn default() -> Self {
        Self::None
    }
}

// ---------------------------------------------------------------------------
// Guard target type (C++ GuardTargetType)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum GuardTargetType {
    Location = 0,
    Object = 1,
    Area = 2,
    None_ = 3,
}

impl Default for GuardTargetType {
    fn default() -> Self {
        Self::None_
    }
}

// ---------------------------------------------------------------------------
// Guard mode (C++ GuardMode)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum GuardMode {
    Normal = 0,
    GuardNormal = 1,
}

impl Default for GuardMode {
    fn default() -> Self {
        Self::Normal
    }
}

#[derive(Debug, Clone)]
pub struct TurretAIData {
    pub turn_rate: Real,
    pub pitch_rate: Real,
    pub natural_turret_angle: Real,
    pub natural_turret_pitch: Real,
    pub turret_fire_angle_sweep: [Real; WEAPONSLOT_COUNT],
    pub turret_sweep_speed_modifier: [Real; WEAPONSLOT_COUNT],
    pub fire_pitch: Real,
    pub min_pitch: Real,
    pub ground_unit_pitch: Real,
    pub turret_weapon_slots: u32,
    pub min_idle_scan_angle: Real,
    pub max_idle_scan_angle: Real,
    pub min_idle_scan_interval: UnsignedInt,
    pub max_idle_scan_interval: UnsignedInt,
    pub recenter_time: UnsignedInt,
    pub initially_disabled: Bool,
    pub fires_while_turning: Bool,
    pub allows_pitch: Bool,
    pub inter_turret_delay: UnsignedInt,
}

impl Default for TurretAIData {
    fn default() -> Self {
        Self {
            turn_rate: 0.01,
            pitch_rate: 0.01,
            natural_turret_angle: 0.0,
            natural_turret_pitch: 0.0,
            turret_fire_angle_sweep: [0.0; WEAPONSLOT_COUNT],
            turret_sweep_speed_modifier: [1.0; WEAPONSLOT_COUNT],
            fire_pitch: 0.0,
            min_pitch: 0.0,
            ground_unit_pitch: 0.0,
            turret_weapon_slots: 0,
            min_idle_scan_angle: 0.0,
            max_idle_scan_angle: 0.0,
            min_idle_scan_interval: 9_999_999,
            max_idle_scan_interval: 9_999_999,
            recenter_time: LOGICFRAMES_PER_SECOND * 2,
            initially_disabled: false,
            fires_while_turning: false,
            allows_pitch: false,
            inter_turret_delay: 0,
        }
    }
}

impl TurretAIData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, TURRET_AI_FIELDS)
    }

    pub fn apply_to(&self, turret: &mut TurretAI) {
        turret.set_turn_rate(self.turn_rate);
        turret.set_pitch_rate(self.pitch_rate);
        turret.set_natural_angle(self.natural_turret_angle);
        turret.set_natural_pitch(self.natural_turret_pitch);
        turret.set_fire_pitch(self.fire_pitch);
        turret.set_min_pitch(self.min_pitch);
        turret.set_ground_unit_pitch(self.ground_unit_pitch);
        turret.set_turret_weapon_slots_mask(self.turret_weapon_slots);
        turret.set_idle_scan_angle_range(self.min_idle_scan_angle, self.max_idle_scan_angle);
        turret
            .set_idle_scan_interval_range(self.min_idle_scan_interval, self.max_idle_scan_interval);
        turret.set_recenter_time(self.recenter_time);
        turret.set_initially_disabled(self.initially_disabled);
        turret.set_turret_enabled(!self.initially_disabled);
        turret.set_fires_while_turning(self.fires_while_turning);
        turret.set_allows_pitch(self.allows_pitch);
        turret.set_inter_turret_delay(self.inter_turret_delay);

        for (index, sweep) in self.turret_fire_angle_sweep.iter().enumerate() {
            let slot = match index {
                0 => WeaponSlotType::Primary,
                1 => WeaponSlotType::Secondary,
                _ => WeaponSlotType::Tertiary,
            };
            turret.set_turret_fire_angle_sweep_for_weapon_slot(slot, *sweep);
        }
        for (index, modifier) in self.turret_sweep_speed_modifier.iter().enumerate() {
            let slot = match index {
                0 => WeaponSlotType::Primary,
                1 => WeaponSlotType::Secondary,
                _ => WeaponSlotType::Tertiary,
            };
            turret.set_turret_sweep_speed_modifier_for_weapon_slot(slot, *modifier);
        }
    }
}

/// AIUpdateModuleData (matches C++ AIUpdateModuleData defaults and fields).
#[derive(Debug, Clone)]
pub struct AIUpdateModuleData {
    module_tag_name_key: NameKeyType,
    auto_acquire_enemies_when_idle: u32,
    mood_attack_check_rate: UnsignedInt,
    surrender_duration: UnsignedInt,
    forbid_player_commands: Bool,
    turrets_linked: Bool,
    turret_primary: Option<TurretAIData>,
    turret_secondary: Option<TurretAIData>,
    locomotor_sets: HashMap<LocomotorSetType, Vec<AsciiString>>,
}

impl Default for AIUpdateModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            auto_acquire_enemies_when_idle: 0,
            mood_attack_check_rate: LOGICFRAMES_PER_SECOND * 2,
            surrender_duration: LOGICFRAMES_PER_SECOND * 120,
            forbid_player_commands: false,
            turrets_linked: false,
            turret_primary: None,
            turret_secondary: None,
            locomotor_sets: HashMap::new(),
        }
    }
}

impl AIUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, AI_UPDATE_FIELDS)
    }

    pub fn auto_acquire_enemies_when_idle(&self) -> u32 {
        self.auto_acquire_enemies_when_idle
    }

    pub fn set_auto_acquire_enemies_when_idle(&mut self, value: u32) {
        self.auto_acquire_enemies_when_idle = value;
    }

    pub fn mood_attack_check_rate(&self) -> UnsignedInt {
        self.mood_attack_check_rate
    }

    pub fn set_mood_attack_check_rate(&mut self, value: UnsignedInt) {
        self.mood_attack_check_rate = value;
    }

    pub fn surrender_duration_frames(&self) -> UnsignedInt {
        self.surrender_duration
    }

    pub fn set_surrender_duration_frames(&mut self, value: UnsignedInt) {
        self.surrender_duration = value;
    }

    pub fn forbid_player_commands(&self) -> Bool {
        self.forbid_player_commands
    }

    pub fn set_forbid_player_commands(&mut self, value: Bool) {
        self.forbid_player_commands = value;
    }

    pub fn turrets_linked(&self) -> Bool {
        self.turrets_linked
    }

    pub fn set_turrets_linked(&mut self, value: Bool) {
        self.turrets_linked = value;
    }

    pub fn turret_primary(&self) -> Option<&TurretAIData> {
        self.turret_primary.as_ref()
    }

    pub fn set_turret_primary(&mut self, turret: TurretAIData) {
        self.turret_primary = Some(turret);
    }

    pub fn turret_secondary(&self) -> Option<&TurretAIData> {
        self.turret_secondary.as_ref()
    }

    pub fn set_turret_secondary(&mut self, turret: TurretAIData) {
        self.turret_secondary = Some(turret);
    }

    pub fn locomotor_sets(&self) -> &HashMap<LocomotorSetType, Vec<AsciiString>> {
        &self.locomotor_sets
    }

    pub fn has_locomotor_set(&self, set: LocomotorSetType) -> bool {
        self.locomotor_sets
            .get(&set)
            .map(|entries| !entries.is_empty())
            .unwrap_or(false)
    }

    pub fn add_locomotor_set_entry(&mut self, set: LocomotorSetType, locomotor: AsciiString) {
        self.locomotor_sets.entry(set).or_default().push(locomotor);
    }

    pub fn set_locomotor_set_entries(&mut self, set: LocomotorSetType, entries: Vec<AsciiString>) {
        self.locomotor_sets.insert(set, entries);
    }
}

impl ModuleData for AIUpdateModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.module_tag_name_key = key;
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_tag_name_key
    }

    fn is_ai_module_data(&self) -> bool {
        true
    }
}

impl Snapshotable for AIUpdateModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: u8 = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn parse_auto_acquire_field(
    _ini: &mut INI,
    data: &mut AIUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.auto_acquire_enemies_when_idle =
        INI::parse_bit_string_32(tokens, AUTO_ACQUIRE_ENEMIES_NAMES)?;
    Ok(())
}

fn parse_duration_field(
    setter: &mut dyn FnMut(UnsignedInt),
    tokens: &[&str],
) -> Result<(), INIError> {
    let Some(token) = tokens.first().copied() else {
        return Err(INIError::InvalidData);
    };
    setter(INI::parse_duration_unsigned_int(token)?);
    Ok(())
}

fn parse_bool_field(setter: &mut dyn FnMut(Bool), tokens: &[&str]) -> Result<(), INIError> {
    let Some(token) = tokens.first().copied() else {
        return Err(INIError::InvalidData);
    };
    setter(INI::parse_bool(token)?);
    Ok(())
}

const AI_UPDATE_FIELDS: &[FieldParse<AIUpdateModuleData>] = &[
    FieldParse {
        token: "Turret",
        parse: parse_turret_field,
    },
    FieldParse {
        token: "AltTurret",
        parse: parse_alt_turret_field,
    },
    FieldParse {
        token: "AutoAcquireEnemiesWhenIdle",
        parse: parse_auto_acquire_field,
    },
    FieldParse {
        token: "Locomotor",
        parse: parse_locomotor_set_field,
    },
    FieldParse {
        token: "MoodAttackCheckRate",
        parse: |_, data, tokens| {
            parse_duration_field(&mut |value| data.mood_attack_check_rate = value, tokens)
        },
    },
    FieldParse {
        token: "SurrenderDuration",
        parse: |_, data, tokens| {
            parse_duration_field(&mut |value| data.surrender_duration = value, tokens)
        },
    },
    FieldParse {
        token: "ForbidPlayerCommands",
        parse: |_, data, tokens| {
            parse_bool_field(&mut |value| data.forbid_player_commands = value, tokens)
        },
    },
    FieldParse {
        token: "TurretsLinked",
        parse: |_, data, tokens| parse_bool_field(&mut |value| data.turrets_linked = value, tokens),
    },
];

fn parse_turret_field(
    ini: &mut INI,
    data: &mut AIUpdateModuleData,
    _tokens: &[&str],
) -> Result<(), INIError> {
    if data.turret_primary.is_some() {
        return Err(INIError::InvalidData);
    }
    let mut turret = TurretAIData::default();
    turret.parse_from_ini(ini)?;
    data.turret_primary = Some(turret);
    Ok(())
}

fn parse_alt_turret_field(
    ini: &mut INI,
    data: &mut AIUpdateModuleData,
    _tokens: &[&str],
) -> Result<(), INIError> {
    if data.turret_secondary.is_some() {
        return Err(INIError::InvalidData);
    }
    let mut turret = TurretAIData::default();
    turret.parse_from_ini(ini)?;
    data.turret_secondary = Some(turret);
    Ok(())
}

fn parse_turret_sweep(
    _ini: &mut INI,
    data: &mut TurretAIData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.len() < 2 {
        return Err(INIError::InvalidData);
    }
    let slot = INI::parse_index_list(tokens[0], WEAPON_SLOT_NAMES)? as usize;
    if slot >= WEAPONSLOT_COUNT {
        return Err(INIError::InvalidData);
    }
    data.turret_fire_angle_sweep[slot] = INI::parse_angle_real(tokens[1])?;
    Ok(())
}

fn parse_turret_sweep_speed(
    _ini: &mut INI,
    data: &mut TurretAIData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.len() < 2 {
        return Err(INIError::InvalidData);
    }
    let slot = INI::parse_index_list(tokens[0], WEAPON_SLOT_NAMES)? as usize;
    if slot >= WEAPONSLOT_COUNT {
        return Err(INIError::InvalidData);
    }
    data.turret_sweep_speed_modifier[slot] = INI::parse_real(tokens[1])?;
    Ok(())
}

fn parse_controlled_weapon_slots(
    _ini: &mut INI,
    data: &mut TurretAIData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let mut mask = 0u32;
    for token in tokens {
        let slot = INI::parse_index_list(token, WEAPON_SLOT_NAMES)?;
        mask |= 1u32 << slot;
    }
    data.turret_weapon_slots = mask;
    Ok(())
}

const TURRET_AI_FIELDS: &[FieldParse<TurretAIData>] = &[
    FieldParse {
        token: "TurretTurnRate",
        parse: |_, data, tokens| {
            let token = tokens.first().ok_or(INIError::InvalidData)?;
            data.turn_rate = INI::parse_angular_velocity_real(token)?;
            Ok(())
        },
    },
    FieldParse {
        token: "TurretPitchRate",
        parse: |_, data, tokens| {
            let token = tokens.first().ok_or(INIError::InvalidData)?;
            data.pitch_rate = INI::parse_angular_velocity_real(token)?;
            Ok(())
        },
    },
    FieldParse {
        token: "NaturalTurretAngle",
        parse: |_, data, tokens| {
            let token = tokens.first().ok_or(INIError::InvalidData)?;
            data.natural_turret_angle = INI::parse_angle_real(token)?;
            Ok(())
        },
    },
    FieldParse {
        token: "NaturalTurretPitch",
        parse: |_, data, tokens| {
            let token = tokens.first().ok_or(INIError::InvalidData)?;
            data.natural_turret_pitch = INI::parse_angle_real(token)?;
            Ok(())
        },
    },
    FieldParse {
        token: "FirePitch",
        parse: |_, data, tokens| {
            let token = tokens.first().ok_or(INIError::InvalidData)?;
            data.fire_pitch = INI::parse_angle_real(token)?;
            Ok(())
        },
    },
    FieldParse {
        token: "MinPhysicalPitch",
        parse: |_, data, tokens| {
            let token = tokens.first().ok_or(INIError::InvalidData)?;
            data.min_pitch = INI::parse_angle_real(token)?;
            Ok(())
        },
    },
    FieldParse {
        token: "GroundUnitPitch",
        parse: |_, data, tokens| {
            let token = tokens.first().ok_or(INIError::InvalidData)?;
            data.ground_unit_pitch = INI::parse_angle_real(token)?;
            Ok(())
        },
    },
    FieldParse {
        token: "TurretFireAngleSweep",
        parse: parse_turret_sweep,
    },
    FieldParse {
        token: "TurretSweepSpeedModifier",
        parse: parse_turret_sweep_speed,
    },
    FieldParse {
        token: "ControlledWeaponSlots",
        parse: parse_controlled_weapon_slots,
    },
    FieldParse {
        token: "AllowsPitch",
        parse: |_, data, tokens| {
            let token = tokens.first().ok_or(INIError::InvalidData)?;
            data.allows_pitch = INI::parse_bool(token)?;
            Ok(())
        },
    },
    FieldParse {
        token: "InterTurretDelay",
        parse: |_, data, tokens| {
            let token = tokens.first().ok_or(INIError::InvalidData)?;
            data.inter_turret_delay = INI::parse_duration_unsigned_int(token)?;
            Ok(())
        },
    },
    FieldParse {
        token: "MinIdleScanAngle",
        parse: |_, data, tokens| {
            let token = tokens.first().ok_or(INIError::InvalidData)?;
            data.min_idle_scan_angle = INI::parse_angle_real(token)?;
            Ok(())
        },
    },
    FieldParse {
        token: "MaxIdleScanAngle",
        parse: |_, data, tokens| {
            let token = tokens.first().ok_or(INIError::InvalidData)?;
            data.max_idle_scan_angle = INI::parse_angle_real(token)?;
            Ok(())
        },
    },
    FieldParse {
        token: "MinIdleScanInterval",
        parse: |_, data, tokens| {
            let token = tokens.first().ok_or(INIError::InvalidData)?;
            data.min_idle_scan_interval = INI::parse_duration_unsigned_int(token)?;
            Ok(())
        },
    },
    FieldParse {
        token: "MaxIdleScanInterval",
        parse: |_, data, tokens| {
            let token = tokens.first().ok_or(INIError::InvalidData)?;
            data.max_idle_scan_interval = INI::parse_duration_unsigned_int(token)?;
            Ok(())
        },
    },
    FieldParse {
        token: "RecenterTime",
        parse: |_, data, tokens| {
            let token = tokens.first().ok_or(INIError::InvalidData)?;
            data.recenter_time = INI::parse_duration_unsigned_int(token)?;
            Ok(())
        },
    },
    FieldParse {
        token: "InitiallyDisabled",
        parse: |_, data, tokens| {
            let token = tokens.first().ok_or(INIError::InvalidData)?;
            data.initially_disabled = INI::parse_bool(token)?;
            Ok(())
        },
    },
    FieldParse {
        token: "FiresWhileTurning",
        parse: |_, data, tokens| {
            let token = tokens.first().ok_or(INIError::InvalidData)?;
            data.fires_while_turning = INI::parse_bool(token)?;
            Ok(())
        },
    },
];

fn parse_locomotor_set_field(
    ini: &mut INI,
    data: &mut AIUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.len() < 2 {
        return Err(INIError::InvalidData);
    }

    let set = match tokens[0] {
        "SET_NORMAL" => LocomotorSetType::Normal,
        "SET_NORMAL_UPGRADED" => LocomotorSetType::NormalUpgraded,
        "SET_FREEFALL" => LocomotorSetType::Freefall,
        "SET_WANDER" => LocomotorSetType::Wander,
        "SET_PANIC" => LocomotorSetType::Panic,
        "SET_TAXIING" => LocomotorSetType::Taxiing,
        "SET_SUPERSONIC" => LocomotorSetType::Supersonic,
        "SET_SLUGGISH" => LocomotorSetType::Sluggish,
        _ => return Err(INIError::InvalidData),
    };

    let entry = data.locomotor_sets.entry(set).or_default();
    if !entry.is_empty() && ini.get_load_type() != INILoadType::CreateOverrides {
        return Err(INIError::InvalidData);
    }
    entry.clear();

    for token in tokens.iter().skip(1) {
        if token.is_empty() || token.eq_ignore_ascii_case("None") {
            continue;
        }
        entry.push(AsciiString::from(*token));
    }

    if entry.is_empty() {
        return Err(INIError::InvalidData);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// AIUpdateInterface – runtime AI state machine + bridge methods
// Ported from C++ AIUpdate.h (AIUpdateInterface class, ~775 lines) and
// AIUpdate.cpp (5280 lines).  State layout mirrors C++ member order.
// ---------------------------------------------------------------------------

pub struct AIUpdateInterface {
    // Waypoint tracking
    prior_waypoint_id: UnsignedInt,
    current_waypoint_id: UnsignedInt,

    // AI scanning
    next_enemy_scan_time: UnsignedInt,
    current_victim_id: ObjectID,

    // Speed / command
    desired_speed: Real,
    last_command_source: u8,

    // Guard
    guard_mode: GuardMode,
    guard_target_type: [GuardTargetType; 2],
    location_to_guard: Coord3D,
    object_to_guard: ObjectID,

    // Attack info
    attack_info_tag: UnsignedInt,

    // Planning-mode waypoint queue
    waypoint_queue: [Coord3D; MAX_WAYPOINTS],
    waypoint_count: usize,
    waypoint_index: usize,
    completed_waypoint_id: UnsignedInt,

    // Pathfinding
    path: Option<Vec<Coord3D>>,
    requested_victim_id: ObjectID,
    requested_destination: Coord3D,
    requested_destination2: Coord3D,
    path_timestamp: UnsignedInt,
    ignore_obstacle_id: ObjectID,
    path_extra_distance: Real,
    pathfind_goal_cell: ICoord2D,
    pathfind_cur_cell: ICoord2D,
    blocked_frames: UnsignedInt,
    cur_max_blocked_speed: Real,
    bump_speed_limit: Real,
    ignore_collisions_until: UnsignedInt,
    queue_for_path_frame: UnsignedInt,
    final_position: Coord3D,
    repulsor1: ObjectID,
    repulsor2: ObjectID,
    next_goal_path_index: Int,
    move_out_of_way1: ObjectID,
    move_out_of_way2: ObjectID,

    // Locomotor
    locomotor_set_tag: UnsignedInt,
    cur_locomotor_tag: UnsignedInt,
    cur_locomotor_set: LocomotorSetType,
    locomotor_goal_type: LocoGoalType,
    locomotor_goal_data: Coord3D,

    // Turret AI handles
    turret_ai: [Option<Arc<std::sync::Mutex<TurretAI>>>; MAX_TURRETS],
    turret_sync_flag: WhichTurretType,

    // Attitude / mood
    attitude: UnsignedInt,
    next_mood_check_time: UnsignedInt,

    // Misc state
    crate_created: ObjectID,
    tmp_int: Int,

    // Boolean flags
    do_final_position: bool,
    waiting_for_path: bool,
    is_attack_path: bool,
    is_final_goal: bool,
    is_approach_path: bool,
    is_safe_path: bool,
    movement_complete: bool,
    is_moving: bool,
    is_blocked: bool,
    is_blocked_and_stuck: bool,
    upgraded_locomotors: bool,
    can_path_through_units: bool,
    randomly_offset_mood_check: bool,
    is_ai_dead: bool,
    is_recruitable: bool,
    executing_waypoint_queue: bool,
    retry_path: bool,
    is_in_update: bool,
    fix_loco_in_post_process: bool,
    allowed_to_chase: bool,

    // Module data reference
    module_data: Arc<AIUpdateModuleData>,
}

impl std::fmt::Debug for AIUpdateInterface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AIUpdateInterface")
            .field("prior_waypoint_id", &self.prior_waypoint_id)
            .field("current_waypoint_id", &self.current_waypoint_id)
            .field("current_victim_id", &self.current_victim_id)
            .field("desired_speed", &self.desired_speed)
            .field("cur_locomotor_set", &self.cur_locomotor_set)
            .field("locomotor_goal_type", &self.locomotor_goal_type)
            .field("is_moving", &self.is_moving)
            .field("is_ai_dead", &self.is_ai_dead)
            .field("waiting_for_path", &self.waiting_for_path)
            .field("is_blocked", &self.is_blocked)
            .field("path", &self.path.is_some())
            .finish()
    }
}

impl AIUpdateInterface {
    pub fn new(module_data: Arc<AIUpdateModuleData>) -> Self {
        Self {
            prior_waypoint_id: 0xfacade,
            current_waypoint_id: 0xfacade,
            next_enemy_scan_time: 0,
            current_victim_id: INVALID_ID,
            desired_speed: AI_FAST_AS_POSSIBLE,
            last_command_source: 0,
            guard_mode: GuardMode::Normal,
            guard_target_type: [GuardTargetType::None_; 2],
            location_to_guard: Coord3D::ZERO,
            object_to_guard: INVALID_ID,
            attack_info_tag: 0,
            waypoint_queue: [Coord3D::ZERO; MAX_WAYPOINTS],
            waypoint_count: 0,
            waypoint_index: 0,
            completed_waypoint_id: 0,
            path: None,
            requested_victim_id: INVALID_ID,
            requested_destination: Coord3D::ZERO,
            requested_destination2: Coord3D::ZERO,
            path_timestamp: 0,
            ignore_obstacle_id: INVALID_ID,
            path_extra_distance: 0.0,
            pathfind_goal_cell: ICoord2D::new(-1, -1),
            pathfind_cur_cell: ICoord2D::new(-1, -1),
            blocked_frames: 0,
            cur_max_blocked_speed: 0.0,
            bump_speed_limit: AI_FAST_AS_POSSIBLE,
            ignore_collisions_until: 0,
            queue_for_path_frame: 0,
            final_position: Coord3D::ZERO,
            repulsor1: INVALID_ID,
            repulsor2: INVALID_ID,
            next_goal_path_index: -1,
            move_out_of_way1: INVALID_ID,
            move_out_of_way2: INVALID_ID,
            locomotor_set_tag: 0,
            cur_locomotor_tag: 0,
            cur_locomotor_set: LocomotorSetType::Invalid,
            locomotor_goal_type: LocoGoalType::None,
            locomotor_goal_data: Coord3D::ZERO,
            turret_ai: [None, None],
            turret_sync_flag: WhichTurretType::Invalid,
            attitude: 0,
            next_mood_check_time: 0,
            crate_created: INVALID_ID,
            tmp_int: 0,
            do_final_position: false,
            waiting_for_path: false,
            is_attack_path: false,
            is_final_goal: false,
            is_approach_path: false,
            is_safe_path: false,
            movement_complete: false,
            is_moving: false,
            is_blocked: false,
            is_blocked_and_stuck: false,
            upgraded_locomotors: false,
            can_path_through_units: false,
            randomly_offset_mood_check: false,
            is_ai_dead: false,
            is_recruitable: true,
            executing_waypoint_queue: false,
            retry_path: false,
            is_in_update: false,
            fix_loco_in_post_process: false,
            allowed_to_chase: true,
            module_data,
        }
    }

    // -----------------------------------------------------------------------
    // GROUP A – Pathfinding bridge
    // -----------------------------------------------------------------------

    /// C++ AIUpdateInterface::doPathfind – called by the pathfinder when it
    /// processes the pathfind queue.  Full logic flow ported from AIUpdate.cpp:378.
    pub fn do_pathfind(&mut self) {
        if !self.waiting_for_path {
            return;
        }
        self.waiting_for_path = false;

        if self.is_safe_path {
            self.destroy_path();
            // PARITY_TODO: call pathfinder->findSafePath() once pathfinder bridge is ported
            return;
        }

        if self.is_approach_path && !self.is_doing_ground_movement() {
            self.is_approach_path = false;
        }
        if self.is_approach_path {
            self.destroy_path();
            // PARITY_TODO: call pathfinder->findClosestPath() once pathfinder bridge is ported.
            // Until then, use the same path bridge as regular movement so approach
            // requests do not complete pathless.
            self.compute_path(self.requested_destination);
            self.is_approach_path = true;
            return;
        }

        if self.is_attack_path {
            // PARITY_TODO: computeAttackPath() once attack-path logic is ported.
            // C++ clears m_isAttackPath when attack-path computation fails, then falls
            // back to a normal path toward the requested destination.
            self.is_attack_path = false;
        }

        self.compute_path(self.requested_destination);
        self.waiting_for_path = self.queue_for_path_frame > TheGameLogic::get_frame();
        if !self.waiting_for_path {
            self.wake_up_now();
        }
    }

    /// C++ AIUpdateInterface::computePath – computes a path to destination,
    /// returns false if no path found.  Lines ~AIUpdate.cpp:440.
    pub fn compute_path(&mut self, destination: Coord3D) -> bool {
        self.requested_destination = destination;
        if self.can_compute_quick_path() {
            return self.compute_quick_path(destination);
        }

        // PARITY_TODO: delegate to pathfinder->findPath() once pathfinder bridge is ported.
        // For now, create a simple two-point path as placeholder.
        let start = self.final_position;
        if start == Coord3D::ZERO {
            return false;
        }
        self.path = Some(vec![start, destination]);
        self.path_timestamp = TheGameLogic::get_frame();
        self.blocked_frames = 0;
        self.is_blocked = false;
        self.is_blocked_and_stuck = false;
        self.queue_for_path_frame = 0;
        self.set_locomotor_goal_position_on_path();
        true
    }

    /// C++ AIUpdateInterface::destroyPath – destroy the current path, setting it to NULL.
    pub fn destroy_path(&mut self) {
        self.path = None;
        self.waiting_for_path = false;
        self.is_attack_path = false;
        self.set_locomotor_goal_none();
    }

    /// C++ AIUpdateInterface::requestPath – queues a request to pathfind to destination.
    /// Ported from AIUpdate.cpp:461.
    pub fn request_path(&mut self, destination: Coord3D, is_final_goal: bool) {
        self.requested_destination = destination;
        self.is_final_goal = is_final_goal;
        self.is_attack_path = false;
        self.requested_victim_id = INVALID_ID;
        self.is_approach_path = false;
        self.is_safe_path = false;

        if self.can_compute_quick_path() {
            self.compute_quick_path(destination);
            return;
        }

        self.waiting_for_path = true;

        let now = TheGameLogic::get_frame();
        if self.path_timestamp > now.saturating_sub(3) {
            self.set_queue_for_path_time(LOGICFRAMES_PER_SECOND);
            if self.path.is_some() && self.is_blocked_and_stuck {
                self.set_ignore_collision_time(LOGICFRAMES_PER_SECOND * 2);
                self.blocked_frames = 0;
                self.is_blocked = false;
                self.is_blocked_and_stuck = false;
            }
            return;
        }
        self.set_queue_for_path_time(0);
    }

    /// C++ AIUpdateInterface::requestAttackPath.
    pub fn request_attack_path(&mut self, victim_id: ObjectID, victim_pos: Coord3D) {
        self.requested_destination = victim_pos;
        self.requested_victim_id = victim_id;
        self.is_attack_path = true;
        self.is_approach_path = false;
        self.is_safe_path = false;
        self.waiting_for_path = true;

        let now = TheGameLogic::get_frame();
        if self.path_timestamp > now.saturating_sub(3) {
            self.set_queue_for_path_time(LOGICFRAMES_PER_SECOND * 2);
            self.set_locomotor_goal_none();
            return;
        }
        self.set_queue_for_path_time(0);
    }

    /// C++ AIUpdateInterface::requestApproachPath.
    pub fn request_approach_path(&mut self, destination: Coord3D) {
        self.requested_destination = destination;
        self.is_final_goal = true;
        self.is_attack_path = false;
        self.requested_victim_id = INVALID_ID;
        self.is_approach_path = true;
        self.is_safe_path = false;
        self.waiting_for_path = true;

        let now = TheGameLogic::get_frame();
        if self.path_timestamp > now.saturating_sub(3) {
            self.set_queue_for_path_time(LOGICFRAMES_PER_SECOND * 2);
            return;
        }
        self.set_queue_for_path_time(0);
    }

    /// C++ AIUpdateInterface::requestSafePath.
    pub fn request_safe_path(&mut self, repulsor: ObjectID) {
        if repulsor != self.repulsor1 {
            self.repulsor2 = self.repulsor1;
        }
        self.repulsor1 = repulsor;
        self.is_final_goal = false;
        self.is_attack_path = false;
        self.requested_victim_id = INVALID_ID;
        self.is_approach_path = false;
        self.is_safe_path = true;
        self.waiting_for_path = true;

        let now = TheGameLogic::get_frame();
        if self.path_timestamp > now.saturating_sub(3) {
            self.set_queue_for_path_time(LOGICFRAMES_PER_SECOND * 2);
            return;
        }
        self.set_queue_for_path_time(0);
    }

    /// C++ AIUpdateInterface::isPathAvailable – checks if a path exists
    /// between current position and destination.
    pub fn is_path_available(&self, destination: Coord3D) -> bool {
        // PARITY_TODO: delegate to pathfinder->clientSafeQuickDoesPathExist once
        // the pathfinder bridge is ported. Until then, answer against the same
        // information compute_path can actually use instead of treating any
        // unrelated current path as proof that this destination is reachable.
        if let Some(path) = self.path.as_ref() {
            if let Some(close_node) = path.last() {
                let dx = destination.x - close_node.x;
                let dy = destination.y - close_node.y;
                let dz = destination.z - close_node.z;
                if dx * dx + dy * dy + dz * dz < 0.25 {
                    return true;
                }
            }
        }
        self.final_position != Coord3D::ZERO && destination != Coord3D::ZERO
    }

    /// C++ AIUpdateInterface::canComputeQuickPath – airborne units can skip
    /// queued pathfinding and build a direct path immediately.
    pub fn can_compute_quick_path(&self) -> bool {
        self.cur_locomotor_tag != 0 && !self.is_doing_ground_movement()
    }

    /// C++ AIUpdateInterface::computeQuickPath – build a direct two-node path
    /// for airborne/non-ground movement.
    pub fn compute_quick_path(&mut self, destination: Coord3D) -> bool {
        if let Some(path) = self.path.as_ref() {
            if let Some(close_node) = path.last() {
                let dx = destination.x - close_node.x;
                let dy = destination.y - close_node.y;
                let dz = destination.z - close_node.z;
                if dx * dx + dy * dy + dz * dz < 0.25 {
                    return true;
                }
            }
        }

        let mut start = self.final_position;
        if start == Coord3D::ZERO {
            return false;
        }
        start.z = destination.z;

        self.destroy_path();
        self.path = Some(vec![start, destination]);
        self.path_timestamp = TheGameLogic::get_frame();
        self.blocked_frames = 0;
        self.is_blocked = false;
        self.is_blocked_and_stuck = false;
        self.queue_for_path_frame = 0;
        self.waiting_for_path = false;
        self.set_locomotor_goal_position_on_path();
        true
    }

    // -----------------------------------------------------------------------
    // GROUP B – Locomotor bridge
    // -----------------------------------------------------------------------

    /// C++ AIUpdateInterface::doLocomotor – execute locomotor movement along path.
    /// Returns UpdateSleepTime hint.  Ported from AIUpdate.cpp (approx line 2000+).
    pub fn do_locomotor(&mut self) -> UnsignedInt {
        if self.is_blocked {
            self.blocked_frames = self.blocked_frames.saturating_add(1);
        } else {
            self.blocked_frames = 0;
        }
        self.is_blocked = false;
        let blocked = self.blocked_frames > 0;

        match self.locomotor_goal_type {
            LocoGoalType::None => {}
            LocoGoalType::PositionOnPath => {
                if self.path.is_none() && self.waiting_for_path {
                    return u32::MAX;
                }
                // PARITY_TODO: move along path using cur_locomotor once locomotor bridge is ported
            }
            LocoGoalType::PositionExplicit => {
                // PARITY_TODO: move to explicit position
            }
            LocoGoalType::Angle => {
                // PARITY_TODO: rotate to angle
            }
        }

        if !blocked && self.blocked_frames > 1 {
            self.blocked_frames = 1;
        }
        self.cur_max_blocked_speed = AI_FAST_AS_POSSIBLE;

        0
    }

    /// C++ AIUpdateInterface::setLocomotorGoalPositionOnPath – sets the
    /// locomotor movement target from the current path.
    pub fn set_locomotor_goal_position_on_path(&mut self) {
        if self.path.is_none() {
            self.locomotor_goal_type = LocoGoalType::None;
            self.locomotor_goal_data = Coord3D::ZERO;
            return;
        }
        self.locomotor_goal_type = LocoGoalType::PositionOnPath;
        self.locomotor_goal_data = Coord3D::ZERO;
    }

    /// C++ AIUpdateInterface::setLocomotorGoalExplicit
    pub fn set_locomotor_goal_position_explicit(&mut self, new_pos: Coord3D) {
        self.locomotor_goal_type = LocoGoalType::PositionExplicit;
        self.locomotor_goal_data = new_pos;
    }

    /// C++ AIUpdateInterface::setLocomotorGoalOrientation
    pub fn set_locomotor_goal_orientation(&mut self, angle: Real) {
        self.locomotor_goal_type = LocoGoalType::Angle;
        self.locomotor_goal_data = Coord3D::new(angle, 0.0, 0.0);
    }

    /// C++ AIUpdateInterface::setLocomotorGoalNone
    pub fn set_locomotor_goal_none(&mut self) {
        self.locomotor_goal_type = LocoGoalType::None;
        self.locomotor_goal_data = Coord3D::ZERO;
    }

    /// C++ AIUpdateInterface::getCurLocomotorSpeed – current speed for AI decisions.
    /// Ported from AIUpdate.cpp:774.
    pub fn get_cur_locomotor_speed(&self) -> Real {
        // PARITY_TODO: query cur_locomotor->getMaxSpeedForCondition(damageState)
        if self.cur_locomotor_tag != 0 {
            return AI_FAST_AS_POSSIBLE;
        }
        0.0
    }

    /// C++ AIUpdateInterface::chooseGoodLocomotorFromCurrentSet – selects the
    /// best locomotor from the current set for the object's current position.
    /// Ported from AIUpdate.cpp:833.
    pub fn choose_locomotor_from_current_set(&mut self) {
        // PARITY_TODO: delegate to pathfinder->chooseBestLocomotorForPosition()
        // once surface-aware locomotor templates are wired. Until then, keep
        // the C++ success/failure contract by selecting an available template
        // from the current set instead of inventing one. If no replacement is
        // available, keep the previous locomotor like C++ does when physics has
        // slid the object into an invalid cell.
        let previous_locomotor_tag = self.cur_locomotor_tag;
        self.cur_locomotor_tag = if self
            .module_data
            .locomotor_sets()
            .get(&self.cur_locomotor_set)
            .map(|entries| !entries.is_empty())
            .unwrap_or(false)
        {
            1
        } else if previous_locomotor_tag != 0 {
            previous_locomotor_tag
        } else {
            0
        };
    }

    /// C++ AIUpdateInterface::isDoingGroundMovement – true if moving along ground.
    pub fn is_doing_ground_movement(&self) -> bool {
        // PARITY_TODO: check cur_locomotor surface type
        self.cur_locomotor_set != LocomotorSetType::Freefall
            && self.cur_locomotor_set != LocomotorSetType::Supersonic
    }

    // -----------------------------------------------------------------------
    // GROUP C – Physics / Collision
    // -----------------------------------------------------------------------

    /// C++ AIUpdateInterface::processCollision – returns true if the physics
    /// collide should apply the force.  Determines blocking and stuck state.
    /// Ported from AIUpdate.cpp:1410.
    pub fn process_collision(&mut self, other_id: ObjectID) -> bool {
        if self.ignore_collisions_until > 0 || self.can_path_through_units {
            return false;
        }

        if !self.is_doing_ground_movement() {
            return false;
        }

        let self_moving = self.is_moving;
        if self_moving {
            let blocked = self.blocked_by(other_id);
            if blocked {
                self.is_blocked = true;
                let max_speed = self.calculate_max_blocked_speed(other_id);
                if max_speed < self.cur_max_blocked_speed {
                    self.cur_max_blocked_speed = max_speed;
                }
                if self.blocked_frames == 0 {
                    self.blocked_frames = 1;
                }
                if !self.need_to_rotate() {
                    self.is_blocked_and_stuck = true;
                }
            }
        }

        false
    }

    /// C++ AIUpdateInterface::blockedBy – returns true if we are blocked by
    /// the other object.  Ported from AIUpdate.cpp:1272.
    pub fn blocked_by(&self, _other_id: ObjectID) -> bool {
        if !self.is_moving {
            return false;
        }
        if self.is_approach_path {
            return false;
        }
        // PARITY_TODO: full angle/distance/infantry check once Object position access is wired
        false
    }

    /// C++ AIUpdateInterface::calculateMaxBlockedSpeed – max speed we can have
    /// and not run into the blocking unit.  Ported from AIUpdate.cpp:1234.
    pub fn calculate_max_blocked_speed(&self, _other_id: ObjectID) -> Real {
        // PARITY_TODO: full vector math once Object position access is wired
        self.cur_max_blocked_speed
    }

    /// C++ AIUpdateInterface::needToRotate – returns true if we need to rotate
    /// to point in our path's direction.  Ported from AIUpdate.cpp:1380.
    pub fn need_to_rotate(&self) -> bool {
        if self.waiting_for_path {
            return true;
        }
        if self.path.is_none() {
            return false;
        }
        // PARITY_TODO: compute angle delta from path direction once path math is ported
        false
    }

    // -----------------------------------------------------------------------
    // GROUP D – Turret control
    // -----------------------------------------------------------------------

    /// C++ AIUpdateInterface::setTurretTargetObject
    pub fn set_turret_target_object(&mut self, tur: WhichTurretType, target_id: ObjectID) {
        let idx = match tur {
            WhichTurretType::Main => 0,
            WhichTurretType::Alt => 1,
            _ => return,
        };
        if let Some(ref turret) = self.turret_ai[idx] {
            if let Ok(mut guard) = turret.lock() {
                let target = if target_id == INVALID_ID {
                    None
                } else {
                    OBJECT_REGISTRY.get_object(target_id)
                };
                guard.set_current_target(target);
            }
        }
    }

    /// C++ AIUpdateInterface::setTurretTargetPosition
    pub fn set_turret_target_position(&mut self, tur: WhichTurretType, pos: Coord3D) {
        let idx = match tur {
            WhichTurretType::Main => 0,
            WhichTurretType::Alt => 1,
            _ => return,
        };
        if let Some(ref turret) = self.turret_ai[idx] {
            if let Ok(mut guard) = turret.lock() {
                guard.set_target_position(Some(pos));
            }
        }
    }

    /// C++ AIUpdateInterface::setTurretEnabled
    pub fn set_turret_enabled(&mut self, tur: WhichTurretType, enabled: bool) {
        let idx = match tur {
            WhichTurretType::Main => 0,
            WhichTurretType::Alt => 1,
            _ => return,
        };
        if let Some(ref turret) = self.turret_ai[idx] {
            if let Ok(mut guard) = turret.lock() {
                guard.set_turret_enabled(enabled);
            }
        }
    }

    /// C++ AIUpdateInterface::recenterTurret
    pub fn recenter_turret(&mut self, tur: WhichTurretType) {
        let idx = match tur {
            WhichTurretType::Main => 0,
            WhichTurretType::Alt => 1,
            _ => return,
        };
        if let Some(ref turret) = self.turret_ai[idx] {
            if let Ok(mut guard) = turret.lock() {
                guard.recenter_turret();
            }
        }
    }

    /// C++ AIUpdateInterface::isTurretInNaturalPosition
    pub fn is_turret_in_natural_position(&self, tur: WhichTurretType) -> bool {
        let idx = match tur {
            WhichTurretType::Main => 0,
            WhichTurretType::Alt => 1,
            _ => return false,
        };
        if let Some(ref turret) = self.turret_ai[idx] {
            if let Ok(guard) = turret.lock() {
                let cur = guard.get_turret_angle();
                let nat = guard.get_natural_angle();
                return (cur - nat).abs() < 0.01;
            }
        }
        false
    }

    /// C++ AIUpdateInterface::getTurretRotAndPitch – returns (angle, pitch) or
    /// None if the turret doesn't exist.
    pub fn get_turret_rot_and_pitch(&self, tur: WhichTurretType) -> Option<(Real, Real)> {
        let idx = match tur {
            WhichTurretType::Main => 0,
            WhichTurretType::Alt => 1,
            _ => return None,
        };
        if let Some(ref turret) = self.turret_ai[idx] {
            if let Ok(guard) = turret.lock() {
                return Some((guard.get_turret_angle(), guard.get_turret_pitch()));
            }
        }
        None
    }

    // -----------------------------------------------------------------------
    // GROUP E – Waypoint management
    // -----------------------------------------------------------------------

    /// C++ AIUpdateInterface::queueWaypoint – add waypoint to end of move list.
    /// Returns true if success, false if queue was full.
    /// Ported from AIUpdate.cpp:1139.
    pub fn queue_waypoint(&mut self, pos: Coord3D) -> bool {
        if self.waypoint_count < MAX_WAYPOINTS {
            self.waypoint_queue[self.waypoint_count] = pos;
            self.waypoint_count += 1;
            return true;
        }
        false
    }

    /// C++ AIUpdateInterface::executeWaypointQueue – start moving along queued waypoints.
    /// Ported from AIUpdate.cpp:1153.
    pub fn execute_waypoint_queue(&mut self) -> bool {
        if self.is_ai_dead {
            return false;
        }
        if self.waypoint_count > 0 {
            self.waypoint_index = 0;
            self.executing_waypoint_queue = true;
            return true;
        }
        false
    }

    /// C++ AIUpdateInterface::clearWaypointQueue – reset the waypoint queue to empty.
    pub fn clear_waypoint_queue(&mut self) {
        self.waypoint_count = 0;
        self.executing_waypoint_queue = false;
    }

    // -----------------------------------------------------------------------
    // GROUP F – State management
    // -----------------------------------------------------------------------

    /// C++ AIUpdateInterface::friend_notifyStateMachineChanged
    pub fn friend_notify_state_machine_changed(&mut self) {
        self.wake_up_now();
    }

    /// C++ AIUpdateInterface::friend_startingMove
    pub fn friend_starting_move(&mut self) {
        self.is_moving = true;
        self.movement_complete = false;
        self.is_blocked = false;
        self.is_blocked_and_stuck = false;
        self.blocked_frames = 0;
        self.cur_max_blocked_speed = AI_FAST_AS_POSSIBLE;
    }

    /// C++ AIUpdateInterface::friend_endingMove
    pub fn friend_ending_move(&mut self) {
        self.is_moving = false;
        self.movement_complete = true;
    }

    /// C++ AIUpdateInterface::markAsDead – marks AI as dead and wakes up.
    /// Ported from AIUpdate.cpp:1176.
    pub fn mark_as_dead(&mut self) {
        self.is_ai_dead = true;
        self.wake_up_now();
    }

    /// C++ AIUpdateInterface::setQueueForPathTime – sets the frame at which
    /// we should re-queue for a pathfind.  Must NOT be set directly.
    /// Ported from AIUpdate.cpp:936.
    pub fn set_queue_for_path_time(&mut self, frames: UnsignedInt) {
        self.queue_for_path_frame = if frames != 0 {
            TheGameLogic::get_frame().saturating_add(frames)
        } else {
            0
        };
    }

    /// C++ AIUpdateInterface::wakeUpNow – wake the AI update immediately.
    /// Ported from AIUpdate.cpp:956.
    pub fn wake_up_now(&mut self) {
        // PARITY_TODO: setWakeFrame(getObject(), UPDATE_SLEEP_NONE) once
        // the wake-frame system is wired through
    }

    // -----------------------------------------------------------------------
    // Accessors
    // -----------------------------------------------------------------------

    pub fn is_ai_dead(&self) -> bool {
        self.is_ai_dead
    }

    pub fn is_moving(&self) -> bool {
        self.is_moving
    }

    pub fn is_waiting_for_path(&self) -> bool {
        self.waiting_for_path || self.queue_for_path_frame > TheGameLogic::get_frame()
    }

    pub fn is_blocked(&self) -> bool {
        self.is_blocked
    }

    pub fn is_blocked_and_stuck(&self) -> bool {
        self.is_blocked_and_stuck
    }

    pub fn get_desired_speed(&self) -> Real {
        self.desired_speed
    }

    pub fn set_desired_speed(&mut self, speed: Real) {
        self.desired_speed = speed;
    }

    pub fn get_path(&self) -> &Option<Vec<Coord3D>> {
        &self.path
    }

    pub fn get_cur_locomotor_set(&self) -> LocomotorSetType {
        self.cur_locomotor_set
    }

    pub fn get_locomotor_goal_type(&self) -> LocoGoalType {
        self.locomotor_goal_type
    }

    pub fn get_blocked_frames(&self) -> UnsignedInt {
        self.blocked_frames
    }

    pub fn get_queue_for_path_frame(&self) -> UnsignedInt {
        self.queue_for_path_frame
    }

    pub fn get_path_timestamp(&self) -> UnsignedInt {
        self.path_timestamp
    }

    pub fn get_current_victim_id(&self) -> ObjectID {
        self.current_victim_id
    }

    pub fn set_current_victim(&mut self, victim_id: ObjectID) {
        self.current_victim_id = victim_id;
    }

    pub fn get_ignore_obstacle_id(&self) -> ObjectID {
        self.ignore_obstacle_id
    }

    pub fn ignore_obstacle(&mut self, id: ObjectID) {
        self.ignore_obstacle_id = id;
    }

    pub fn set_ignore_collision_time(&mut self, frames: UnsignedInt) {
        self.ignore_collisions_until = frames;
    }

    pub fn can_path_through_units(&self) -> bool {
        self.can_path_through_units
    }

    pub fn set_can_path_through_units(&mut self, can_path: bool) {
        self.can_path_through_units = can_path;
        if can_path {
            self.is_blocked_and_stuck = false;
        }
    }

    pub fn is_recruitable(&self) -> bool {
        self.is_recruitable
    }

    pub fn set_is_recruitable(&mut self, val: bool) {
        self.is_recruitable = val;
    }

    pub fn get_final_position(&self) -> Coord3D {
        self.final_position
    }

    pub fn set_final_position(&mut self, pos: Coord3D) {
        self.final_position = pos;
        self.do_final_position = false;
    }

    pub fn get_module_data(&self) -> &AIUpdateModuleData {
        &self.module_data
    }

    pub fn are_turrets_linked(&self) -> bool {
        self.module_data.turrets_linked()
    }

    pub fn can_auto_acquire(&self) -> bool {
        self.module_data.auto_acquire_enemies_when_idle() != 0
    }

    pub fn set_locomotor_upgrade(&mut self, set: bool) {
        self.upgraded_locomotors = set;
        if matches!(
            self.cur_locomotor_set,
            LocomotorSetType::Normal | LocomotorSetType::NormalUpgraded
        ) {
            self.choose_locomotor_set(LocomotorSetType::Normal);
        }
    }

    pub fn notify_crate(&mut self, id: ObjectID) {
        self.crate_created = id;
    }

    pub fn get_crate_id(&self) -> ObjectID {
        self.crate_created
    }

    pub fn choose_locomotor_set(&mut self, wst: LocomotorSetType) -> bool {
        let mut actual_set = wst;
        if wst == LocomotorSetType::Normal && self.upgraded_locomotors {
            actual_set = LocomotorSetType::NormalUpgraded;
        }
        if actual_set == self.cur_locomotor_set {
            return true;
        }
        if self.choose_locomotor_set_explicit(actual_set) {
            self.choose_locomotor_from_current_set();
            return true;
        }
        false
    }

    fn choose_locomotor_set_explicit(&mut self, wst: LocomotorSetType) -> bool {
        let Some(entries) = self.module_data.locomotor_sets().get(&wst) else {
            return false;
        };
        if entries.is_empty() {
            return false;
        }

        self.locomotor_set_tag = entries.len() as UnsignedInt;
        self.cur_locomotor_tag = 0;
        self.cur_locomotor_set = wst;
        true
    }
}

/// Module wrapper for AIUpdateInterface to satisfy module creation parity.
#[derive(Debug)]
pub struct AIUpdateInterfaceModule {
    module_name_key: NameKeyType,
    data: Arc<AIUpdateModuleData>,
    runtime_ai: Option<Arc<Mutex<dyn crate::modules::AIUpdateInterface>>>,
}

impl AIUpdateInterfaceModule {
    pub fn new(module_name_key: NameKeyType, data: Arc<AIUpdateModuleData>) -> Self {
        Self {
            module_name_key,
            data,
            runtime_ai: None,
        }
    }

    pub fn set_runtime_ai(
        &mut self,
        runtime_ai: Arc<Mutex<dyn crate::modules::AIUpdateInterface>>,
    ) {
        self.runtime_ai = Some(runtime_ai);
    }
}

impl Module for AIUpdateInterfaceModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }
}

impl Snapshotable for AIUpdateInterfaceModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: u8 = 4;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;
        if let Some(runtime_ai) = &self.runtime_ai {
            let mut guard = runtime_ai
                .lock()
                .map_err(|_| "AIUpdate runtime lock poisoned during crc".to_string())?;
            let _ = guard.xfer_ai_update_state(xfer)?;
        }
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: u8 = 4;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;
        if let Some(runtime_ai) = &self.runtime_ai {
            let mut guard = runtime_ai
                .lock()
                .map_err(|_| "AIUpdate runtime lock poisoned during xfer".to_string())?;
            let _ = guard.xfer_ai_update_state(xfer)?;
        }
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;

    fn ai_update() -> AIUpdateInterface {
        AIUpdateInterface::new(Arc::new(AIUpdateModuleData::default()))
    }

    fn ai_update_with_locomotors() -> AIUpdateInterface {
        let mut data = AIUpdateModuleData::default();
        data.add_locomotor_set_entry(LocomotorSetType::Normal, "BasicLoco".into());
        data.add_locomotor_set_entry(LocomotorSetType::NormalUpgraded, "UpgradeLoco".into());
        AIUpdateInterface::new(Arc::new(data))
    }

    #[derive(Debug)]
    struct RecordingAi {
        calls: Arc<Mutex<u32>>,
    }

    impl crate::modules::AIUpdateInterface for RecordingAi {
        fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }

        fn xfer_ai_update_state(&mut self, _xfer: &mut dyn Xfer) -> Result<bool, String> {
            *self.calls.lock().unwrap() += 1;
            Ok(true)
        }

        fn is_moving(&self) -> bool {
            false
        }

        fn is_idle(&self) -> bool {
            true
        }

        fn set_movement_target(&mut self, _target: &Coord3D) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn ai_update_module_xfer_forwards_to_runtime_ai() {
        let data = Arc::new(AIUpdateModuleData::default());
        let mut module = AIUpdateInterfaceModule::new(1, data);
        let calls = Arc::new(Mutex::new(0));
        let runtime_ai = Arc::new(Mutex::new(RecordingAi {
            calls: Arc::clone(&calls),
        }));
        module.set_runtime_ai(runtime_ai);

        let writer = Cursor::new(Vec::new());
        let mut xfer = XferSave::new(writer, 1);
        module
            .xfer(&mut xfer)
            .expect("AIUpdate module xfer should forward to runtime AI");

        assert_eq!(*calls.lock().unwrap(), 1);
    }

    #[test]
    fn set_queue_for_path_time_stores_absolute_frame() {
        let mut ai = ai_update();
        let now = TheGameLogic::get_frame();

        ai.set_queue_for_path_time(LOGICFRAMES_PER_SECOND);

        assert_eq!(
            ai.get_queue_for_path_frame(),
            now.saturating_add(LOGICFRAMES_PER_SECOND)
        );
        assert!(ai.is_waiting_for_path());
    }

    #[test]
    fn request_path_uses_cpp_repath_delay_when_path_is_fresh() {
        let mut ai = ai_update();
        ai.path_timestamp = TheGameLogic::get_frame();
        ai.path = Some(vec![
            Coord3D::new(1.0, 0.0, 0.0),
            Coord3D::new(2.0, 0.0, 0.0),
        ]);
        ai.is_blocked = true;
        ai.is_blocked_and_stuck = true;

        ai.request_path(Coord3D::new(64.0, 32.0, 0.0), true);

        assert_eq!(ai.requested_destination, Coord3D::new(64.0, 32.0, 0.0));
        assert!(ai.is_final_goal);
        assert!(!ai.is_attack_path);
        assert_eq!(
            ai.get_queue_for_path_frame(),
            TheGameLogic::get_frame().saturating_add(LOGICFRAMES_PER_SECOND)
        );
        assert_eq!(ai.blocked_frames, 0);
        assert!(!ai.is_blocked);
        assert!(!ai.is_blocked_and_stuck);
        assert_eq!(ai.ignore_collisions_until, LOGICFRAMES_PER_SECOND * 2);
    }

    #[test]
    fn request_attack_approach_and_safe_paths_match_cpp_flags() {
        let mut ai = ai_update();

        ai.request_attack_path(123, Coord3D::new(10.0, 20.0, 0.0));
        assert_eq!(ai.requested_victim_id, 123);
        assert_eq!(ai.requested_destination, Coord3D::new(10.0, 20.0, 0.0));
        assert!(ai.is_attack_path);
        assert!(!ai.is_approach_path);
        assert!(!ai.is_safe_path);
        assert!(ai.is_waiting_for_path());

        ai.request_approach_path(Coord3D::new(30.0, 40.0, 0.0));
        assert_eq!(ai.requested_victim_id, INVALID_ID);
        assert!(ai.is_final_goal);
        assert!(!ai.is_attack_path);
        assert!(ai.is_approach_path);
        assert!(!ai.is_safe_path);

        ai.request_safe_path(456);
        assert_eq!(ai.repulsor1, 456);
        assert_eq!(ai.requested_victim_id, INVALID_ID);
        assert!(!ai.is_final_goal);
        assert!(!ai.is_attack_path);
        assert!(!ai.is_approach_path);
        assert!(ai.is_safe_path);
    }

    #[test]
    fn compute_and_destroy_path_update_cpp_state() {
        let mut ai = ai_update();
        ai.set_final_position(Coord3D::new(1.0, 2.0, 0.0));
        ai.is_blocked = true;
        ai.is_blocked_and_stuck = true;
        ai.set_queue_for_path_time(LOGICFRAMES_PER_SECOND);

        assert!(ai.compute_path(Coord3D::new(8.0, 9.0, 0.0)));
        assert!(ai.get_path().is_some());
        assert_eq!(ai.get_path_timestamp(), TheGameLogic::get_frame());
        assert!(!ai.is_blocked);
        assert!(!ai.is_blocked_and_stuck);
        assert_eq!(ai.get_queue_for_path_frame(), 0);
        assert_eq!(ai.get_locomotor_goal_type(), LocoGoalType::PositionOnPath);
        assert_eq!(ai.locomotor_goal_data, Coord3D::ZERO);

        ai.is_attack_path = true;
        ai.waiting_for_path = true;
        ai.set_locomotor_goal_position_explicit(Coord3D::new(2.0, 3.0, 0.0));
        ai.destroy_path();

        assert!(ai.get_path().is_none());
        assert!(!ai.is_waiting_for_path());
        assert!(!ai.is_attack_path);
        assert_eq!(ai.get_locomotor_goal_type(), LocoGoalType::None);
    }

    #[test]
    fn request_path_computes_airborne_quick_path_like_cpp() {
        let mut ai = ai_update();
        ai.set_final_position(Coord3D::new(4.0, 5.0, 1.0));
        ai.cur_locomotor_set = LocomotorSetType::Supersonic;
        ai.cur_locomotor_tag = 1;
        ai.is_blocked = true;
        ai.is_blocked_and_stuck = true;

        ai.request_path(Coord3D::new(20.0, 30.0, 9.0), true);

        assert!(!ai.is_waiting_for_path());
        assert_eq!(ai.get_queue_for_path_frame(), 0);
        assert!(!ai.is_blocked);
        assert!(!ai.is_blocked_and_stuck);
        assert_eq!(ai.get_locomotor_goal_type(), LocoGoalType::PositionOnPath);
        assert_eq!(
            ai.get_path().as_ref().unwrap().as_slice(),
            &[Coord3D::new(4.0, 5.0, 9.0), Coord3D::new(20.0, 30.0, 9.0)]
        );
    }

    #[test]
    fn quick_path_reuses_close_existing_destination_like_cpp() {
        let mut ai = ai_update();
        ai.set_final_position(Coord3D::new(1.0, 2.0, 0.0));
        ai.cur_locomotor_set = LocomotorSetType::Supersonic;
        ai.cur_locomotor_tag = 1;
        ai.path = Some(vec![
            Coord3D::new(1.0, 2.0, 3.0),
            Coord3D::new(9.0, 10.0, 11.0),
        ]);
        let old_timestamp = ai.get_path_timestamp();

        assert!(ai.compute_quick_path(Coord3D::new(9.2, 10.1, 11.0)));

        assert_eq!(ai.get_path().as_ref().unwrap().len(), 2);
        assert_eq!(ai.get_path_timestamp(), old_timestamp);
    }

    #[test]
    fn is_path_available_checks_requested_destination_not_any_path() {
        let mut ai = ai_update();
        ai.path = Some(vec![
            Coord3D::new(1.0, 2.0, 0.0),
            Coord3D::new(10.0, 10.0, 0.0),
        ]);

        assert!(ai.is_path_available(Coord3D::new(10.1, 10.1, 0.0)));
        assert!(!ai.is_path_available(Coord3D::new(99.0, 99.0, 0.0)));

        ai.set_final_position(Coord3D::new(1.0, 2.0, 0.0));
        assert!(ai.is_path_available(Coord3D::new(99.0, 99.0, 0.0)));
    }

    #[test]
    fn do_pathfind_approach_builds_current_bridge_path() {
        let mut ai = ai_update();
        ai.set_final_position(Coord3D::new(3.0, 4.0, 0.0));

        ai.request_approach_path(Coord3D::new(12.0, 16.0, 0.0));
        ai.do_pathfind();

        assert!(!ai.is_waiting_for_path());
        assert!(ai.is_approach_path);
        assert_eq!(ai.get_locomotor_goal_type(), LocoGoalType::PositionOnPath);
        assert_eq!(
            ai.get_path().as_ref().unwrap().as_slice(),
            &[Coord3D::new(3.0, 4.0, 0.0), Coord3D::new(12.0, 16.0, 0.0)]
        );
    }

    #[test]
    fn do_pathfind_attack_fallback_clears_attack_flag_like_cpp() {
        let mut ai = ai_update();
        ai.set_final_position(Coord3D::new(6.0, 7.0, 0.0));

        ai.request_attack_path(123, Coord3D::new(18.0, 21.0, 0.0));
        ai.do_pathfind();

        assert!(!ai.is_waiting_for_path());
        assert!(!ai.is_attack_path);
        assert_eq!(ai.get_locomotor_goal_type(), LocoGoalType::PositionOnPath);
        assert_eq!(
            ai.get_path().as_ref().unwrap().as_slice(),
            &[Coord3D::new(6.0, 7.0, 0.0), Coord3D::new(18.0, 21.0, 0.0)]
        );
    }

    #[test]
    fn do_locomotor_updates_blocked_state_like_cpp() {
        let mut ai = ai_update();
        ai.cur_max_blocked_speed = 12.0;
        ai.is_blocked = true;

        assert_eq!(ai.do_locomotor(), 0);
        assert_eq!(ai.blocked_frames, 1);
        assert!(!ai.is_blocked);
        assert_eq!(ai.cur_max_blocked_speed, AI_FAST_AS_POSSIBLE);

        assert_eq!(ai.do_locomotor(), 0);
        assert_eq!(ai.blocked_frames, 0);
    }

    #[test]
    fn do_locomotor_waits_forever_for_pending_path() {
        let mut ai = ai_update();
        ai.locomotor_goal_type = LocoGoalType::PositionOnPath;
        ai.waiting_for_path = true;
        ai.path = None;

        assert_eq!(ai.do_locomotor(), u32::MAX);
        assert_eq!(ai.get_locomotor_goal_type(), LocoGoalType::PositionOnPath);
    }

    #[test]
    fn choose_locomotor_set_fails_without_ini_set_like_cpp() {
        let mut ai = ai_update_with_locomotors();

        assert!(!ai.choose_locomotor_set(LocomotorSetType::Wander));
        assert_eq!(ai.get_cur_locomotor_set(), LocomotorSetType::Invalid);
        assert_eq!(ai.cur_locomotor_tag, 0);
    }

    #[test]
    fn choose_locomotor_set_selects_available_template_like_cpp() {
        let mut ai = ai_update_with_locomotors();

        assert!(ai.choose_locomotor_set(LocomotorSetType::Normal));
        assert_eq!(ai.get_cur_locomotor_set(), LocomotorSetType::Normal);
        assert_eq!(ai.locomotor_set_tag, 1);
        assert_ne!(ai.cur_locomotor_tag, 0);
        assert_eq!(ai.get_cur_locomotor_speed(), AI_FAST_AS_POSSIBLE);
    }

    #[test]
    fn choose_locomotor_preserves_previous_when_no_current_cell_locomotor_like_cpp() {
        let mut ai = ai_update();
        ai.cur_locomotor_set = LocomotorSetType::Wander;
        ai.cur_locomotor_tag = 77;

        ai.choose_locomotor_from_current_set();

        assert_eq!(ai.cur_locomotor_tag, 77);
        assert_eq!(ai.get_cur_locomotor_speed(), AI_FAST_AS_POSSIBLE);
    }

    #[test]
    fn set_locomotor_upgrade_reselects_normal_set_like_cpp() {
        let mut ai = ai_update_with_locomotors();

        assert!(ai.choose_locomotor_set(LocomotorSetType::Normal));
        ai.set_locomotor_upgrade(true);

        assert_eq!(ai.get_cur_locomotor_set(), LocomotorSetType::NormalUpgraded);
        assert_ne!(ai.cur_locomotor_tag, 0);
    }
}
