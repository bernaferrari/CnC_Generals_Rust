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
    AsciiString, Bool, Coord3D, FormationID, ICoord2D, Int, KindOf, LocomotorSetType, ObjectID,
    ObjectStatusMaskType, ObjectStatusTypes, Real, UnsignedInt, Vec3D, WhichTurretType, INVALID_ID,
    LOGICFRAMES_PER_SECOND, WEAPONSLOT_COUNT,
};
use crate::helpers::TheGameLogic;
use crate::locomotor::LOCOMOTOR_STORE;
use crate::locomotor::{ActivePath, BodyDamageType as LocoBodyDamageType, Locomotor};
use crate::locomotor::{SURFACE_AIR, SURFACE_GROUND};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::{CrushSquishTestType, Object};
use crate::path::PATHFIND_CELL_SIZE_F;
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

#[derive(Clone)]
struct CollisionObjectSnapshot {
    id: ObjectID,
    position: Coord3D,
    direction: (Real, Real),
    velocity: Vec3D,
    has_physics: bool,
    is_infantry: bool,
    is_vehicle: bool,
    is_dozer: bool,
    moving: bool,
    ground: bool,
    dead: bool,
    path_destination: Option<Coord3D>,
    frames_blocked: u32,
    formation_id: FormationID,
}

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
    cur_locomotor_template: AsciiString,
    cur_locomotor_surfaces: u32,
    cur_locomotor: Option<Locomotor>,
    cur_locomotor_speed: Real,
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

    // Rust adapter state: C++ reaches the owner through getObject().
    owner_object_id: ObjectID,

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
        Self::new_for_object(module_data, INVALID_ID)
    }

    pub fn new_for_object(module_data: Arc<AIUpdateModuleData>, owner_object_id: ObjectID) -> Self {
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
            cur_locomotor_template: AsciiString::new(),
            cur_locomotor_surfaces: 0,
            cur_locomotor: None,
            cur_locomotor_speed: 0.0,
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
            owner_object_id,
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
            // C++ calls pathfinder->findSafePath() here. Until that bridge is
            // available, preserve the observable contract that a safe-path
            // request resolves to an active movement path instead of completing
            // pathless.
            self.compute_safe_path();
            return;
        }

        if self.is_approach_path && !self.is_doing_ground_movement() {
            self.is_approach_path = false;
        }
        if self.is_approach_path {
            self.compute_approach_path(self.requested_destination);
            self.is_approach_path = true;
            return;
        }

        if self.is_attack_path {
            let victim_id = self.requested_victim_id;
            let victim = if victim_id == INVALID_ID {
                None
            } else {
                OBJECT_REGISTRY.get_object(victim_id)
            };
            if self.compute_attack_path(victim.as_ref()) {
                self.is_attack_path = true;
                return;
            }
            self.is_attack_path = false;
            if let Some(victim) = victim {
                if let Ok(victim_guard) = victim.read() {
                    self.requested_destination = *victim_guard.get_position();
                    self.ignore_obstacle(victim_guard.get_id());
                }
            }
        }

        self.compute_path(self.requested_destination);
        self.waiting_for_path = self.queue_for_path_frame > TheGameLogic::get_frame();
        if !self.waiting_for_path {
            self.wake_up_now();
        }
    }

    fn compute_attack_path(&mut self, victim: Option<&Arc<std::sync::RwLock<Object>>>) -> bool {
        let now = TheGameLogic::get_frame();
        if self.path_timestamp_is_recent(now) && self.path.is_some() && self.is_blocked_and_stuck {
            self.set_ignore_collision_time(LOGICFRAMES_PER_SECOND * 2);
            self.blocked_frames = 0;
            self.is_blocked = false;
            self.is_blocked_and_stuck = false;
            return true;
        }

        let Some(owner_arc) = OBJECT_REGISTRY.get_object(self.owner_object_id) else {
            return false;
        };
        let land_bound = (self.current_locomotor_set_surfaces() & SURFACE_AIR) == 0;
        let target_pos = victim
            .and_then(|victim| victim.read().ok().map(|guard| *guard.get_position()))
            .unwrap_or(self.requested_destination);

        let attack_in_range_and_visible = {
            let Ok(owner_guard) = owner_arc.read() else {
                return false;
            };
            let Some((weapon, _slot)) = owner_guard.get_current_weapon() else {
                return false;
            };
            let in_range = if let Some(victim) = victim {
                if let Ok(victim_guard) = victim.read() {
                    weapon.is_within_attack_range(
                        owner_guard.get_id(),
                        Some(victim_guard.get_id()),
                        None,
                    )
                } else {
                    false
                }
            } else {
                weapon.is_within_attack_range(owner_guard.get_id(), None, Some(&target_pos))
            };
            if !in_range {
                false
            } else {
                let view_blocked = if self.is_doing_ground_movement() {
                    crate::ai::THE_AI
                        .read()
                        .ok()
                        .and_then(|ai| ai.pathfinder())
                        .and_then(|pathfinder| {
                            pathfinder.read().ok().map(|pf| {
                                if let Some(victim) = victim {
                                    victim.read().ok().map_or(false, |victim_guard| {
                                        if victim_guard.is_significantly_above_terrain() {
                                            false
                                        } else {
                                            pf.is_attack_view_blocked_by_obstacle(
                                                &owner_guard,
                                                owner_guard.get_position(),
                                                Some(&victim_guard),
                                                &target_pos,
                                            )
                                        }
                                    })
                                } else {
                                    pf.is_attack_view_blocked_by_obstacle(
                                        &owner_guard,
                                        owner_guard.get_position(),
                                        None,
                                        &target_pos,
                                    )
                                }
                            })
                        })
                        .unwrap_or(false)
                } else {
                    false
                };
                !view_blocked
            }
        };
        if attack_in_range_and_visible {
            self.destroy_path();
            return true;
        }

        let (is_contact_weapon, owner_pos, owner_above_terrain) = {
            let Ok(owner_guard) = owner_arc.read() else {
                return false;
            };
            let Some((weapon, _slot)) = owner_guard.get_current_weapon() else {
                return false;
            };
            (
                weapon.is_contact_weapon(),
                *owner_guard.get_position(),
                owner_guard.is_above_terrain(),
            )
        };

        if is_contact_weapon {
            self.destroy_path();
            let ok = self.compute_path(target_pos);
            let Some(path) = self.path.as_mut() else {
                return false;
            };
            let Some(last_node) = path.last_mut() else {
                return false;
            };
            let target_dx = target_pos.x - last_node.x;
            let target_dy = target_pos.y - last_node.y;
            if target_dx * target_dx + target_dy * target_dy
                < (PATHFIND_CELL_SIZE_F * 3.0) * (PATHFIND_CELL_SIZE_F * 3.0)
            {
                *last_node = target_pos;
            }
            let owner_dx = owner_pos.x - last_node.x;
            let owner_dy = owner_pos.y - last_node.y;
            if owner_dx * owner_dx + owner_dy * owner_dy
                < PATHFIND_CELL_SIZE_F * PATHFIND_CELL_SIZE_F
            {
                self.destroy_path();
                return false;
            }
            return ok;
        }

        if owner_above_terrain && !land_bound {
            if self.path.as_ref().is_some_and(|path| {
                path.len() == 2
                    && path.last().is_some_and(|last| {
                        let dx = target_pos.x - last.x;
                        let dy = target_pos.y - last.y;
                        dx * dx + dy * dy < 0.25
                    })
            }) {
                return true;
            }
            self.destroy_path();
            let mut start = owner_pos;
            start.z = target_pos.z;
            self.path = Some(vec![start, target_pos]);
            self.path_timestamp = TheGameLogic::get_frame();
            self.blocked_frames = 0;
            self.is_blocked_and_stuck = false;
            self.set_locomotor_goal_position_on_path();
            return true;
        }

        self.destroy_path();
        if !self.compute_path(target_pos) {
            return false;
        }

        let Some(goal) = self.path.as_ref().and_then(|path| path.last()).copied() else {
            return false;
        };
        let goal_in_range = {
            let Ok(owner_guard) = owner_arc.read() else {
                return false;
            };
            let Some((weapon, _slot)) = owner_guard.get_current_weapon() else {
                return false;
            };
            weapon.is_source_object_with_goal_position_within_attack_range(
                owner_guard.get_id(),
                &goal,
                victim.and_then(|victim| victim.read().ok().map(|guard| guard.get_id())),
                Some(&target_pos),
            )
        };
        if !goal_in_range {
            let dx = goal.x - owner_pos.x;
            let dy = goal.y - owner_pos.y;
            if (dx * dx + dy * dy).sqrt() < 3.0 * PATHFIND_CELL_SIZE_F {
                self.destroy_path();
                return self.compute_approach_path(owner_pos);
            }
            self.destroy_path();
            return false;
        }

        self.path_timestamp = TheGameLogic::get_frame();
        self.blocked_frames = 0;
        self.is_blocked_and_stuck = false;
        true
    }

    /// C++ AIUpdateInterface::computePath – computes a path to destination,
    /// returns false if no path found.  Lines ~AIUpdate.cpp:440.
    pub fn compute_path(&mut self, destination: Coord3D) -> bool {
        self.requested_destination = destination;
        if !self.is_blocked_and_stuck {
            self.destroy_path();
        }

        if self.can_compute_quick_path() {
            return self.compute_quick_path(destination);
        }

        self.retry_path = false;

        let start = self.final_position;
        if start == Coord3D::ZERO {
            self.path_timestamp = TheGameLogic::get_frame();
            self.blocked_frames = 0;
            self.is_blocked_and_stuck = false;
            return false;
        }
        let surfaces = self.current_locomotor_set_surfaces();
        if surfaces == 0 {
            self.path_timestamp = TheGameLogic::get_frame();
            self.blocked_frames = 0;
            self.is_blocked_and_stuck = false;
            return false;
        }

        let Some(pathfinder) = crate::ai::THE_AI.read().ok().and_then(|ai| ai.pathfinder()) else {
            self.path_timestamp = TheGameLogic::get_frame();
            self.blocked_frames = 0;
            self.is_blocked_and_stuck = false;
            return false;
        };
        let Ok(pathfinder) = pathfinder.read() else {
            self.path_timestamp = TheGameLogic::get_frame();
            self.blocked_frames = 0;
            self.is_blocked_and_stuck = false;
            return false;
        };
        let ignore_obstacle_id = if self.ignore_obstacle_id == INVALID_ID {
            None
        } else {
            Some(self.ignore_obstacle_id)
        };

        if !self.is_final_goal
            && pathfinder.is_line_passable_for_surfaces(
                &start,
                &destination,
                surfaces,
                ignore_obstacle_id,
            )
        {
            drop(pathfinder);
            return self.compute_quick_path(destination);
        }

        let request = crate::ai::pathfind_complete::PathRequest {
            object_id: INVALID_ID,
            from: start,
            to: destination,
            surfaces,
            is_crusher: false,
            unit_radius: 0.0,
            allow_partial: false,
            move_allies: self.can_path_through_units,
            ignore_obstacle_id,
            is_human: false,
        };
        let mut path_result = pathfinder.find_path_result(request.clone());
        if !path_result.success && self.path.is_none() {
            path_result = pathfinder.find_closest_path_result(request);
            self.retry_path = true;
        }
        drop(pathfinder);

        if path_result.success {
            self.destroy_path();
            self.path = Some(path_result.waypoints);
            self.queue_for_path_frame = 0;
            self.set_locomotor_goal_position_on_path();
            self.is_blocked = false;
        } else if self.path.is_some() && self.is_blocked_and_stuck {
            self.destroy_path();
            self.set_queue_for_path_time(LOGICFRAMES_PER_SECOND);
            self.final_position = start;
            self.set_locomotor_goal_none();
            self.is_blocked = false;
        }

        self.path_timestamp = TheGameLogic::get_frame();
        self.blocked_frames = 0;
        self.is_blocked_and_stuck = false;
        self.path.is_some()
    }

    fn compute_approach_path(&mut self, destination: Coord3D) -> bool {
        self.destroy_path();

        let start = self.final_position;
        if start == Coord3D::ZERO {
            self.path_timestamp = TheGameLogic::get_frame();
            self.blocked_frames = 0;
            self.is_blocked_and_stuck = false;
            return false;
        }
        let surfaces = self.current_locomotor_set_surfaces();
        if surfaces == 0 {
            self.path_timestamp = TheGameLogic::get_frame();
            self.blocked_frames = 0;
            self.is_blocked_and_stuck = false;
            return false;
        }

        let Some(pathfinder) = crate::ai::THE_AI.read().ok().and_then(|ai| ai.pathfinder()) else {
            self.path_timestamp = TheGameLogic::get_frame();
            self.blocked_frames = 0;
            self.is_blocked_and_stuck = false;
            return false;
        };
        let Ok(pathfinder) = pathfinder.read() else {
            self.path_timestamp = TheGameLogic::get_frame();
            self.blocked_frames = 0;
            self.is_blocked_and_stuck = false;
            return false;
        };
        let ignore_obstacle_id = if self.ignore_obstacle_id == INVALID_ID {
            None
        } else {
            Some(self.ignore_obstacle_id)
        };
        let request = crate::ai::pathfind_complete::PathRequest {
            object_id: INVALID_ID,
            from: start,
            to: destination,
            surfaces,
            is_crusher: false,
            unit_radius: 0.0,
            allow_partial: false,
            move_allies: false,
            ignore_obstacle_id,
            is_human: false,
        };
        let path_result = pathfinder.find_closest_path_result(request);
        drop(pathfinder);

        if path_result.success {
            self.path = Some(path_result.waypoints);
            self.queue_for_path_frame = 0;
            self.set_locomotor_goal_position_on_path();
            self.is_blocked = false;
        }

        self.path_timestamp = TheGameLogic::get_frame();
        self.blocked_frames = 0;
        self.is_blocked_and_stuck = false;
        path_result.success
    }

    fn compute_safe_path(&mut self) -> bool {
        let start = self.final_position;
        if start == Coord3D::ZERO {
            return false;
        }

        let fallback_repulsor = Coord3D::new(-1000.0, -1000.0, start.z);
        let repulsor1 = self
            .repulsor_position(self.repulsor1)
            .unwrap_or(fallback_repulsor);
        let repulsor2 = self.repulsor_position(self.repulsor2).unwrap_or(repulsor1);
        let repel_from = (repulsor1 + repulsor2) * 0.5;
        let mut away_x = start.x - repel_from.x;
        let mut away_y = start.y - repel_from.y;
        let len_sq = away_x * away_x + away_y * away_y;
        if len_sq <= f32::EPSILON {
            away_x = 1.0;
            away_y = 0.0;
        } else {
            let inv_len = len_sq.sqrt().recip();
            away_x *= inv_len;
            away_y *= inv_len;
        }

        let safe_distance = 64.0;
        let destination = Coord3D::new(
            start.x + away_x * safe_distance,
            start.y + away_y * safe_distance,
            start.z,
        );

        self.path = Some(vec![start, destination]);
        self.path_timestamp = TheGameLogic::get_frame();
        self.blocked_frames = 0;
        self.is_blocked = false;
        self.is_blocked_and_stuck = false;
        self.queue_for_path_frame = 0;
        self.set_locomotor_goal_position_on_path();
        true
    }

    fn repulsor_position(&self, id: ObjectID) -> Option<Coord3D> {
        if id == INVALID_ID {
            return None;
        }
        let object = TheGameLogic::find_object_by_id(id)?;
        let guard = object.read().ok()?;
        Some(*guard.get_position())
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
        if self.path_timestamp_is_recent(now) {
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
        if self.path_timestamp_is_recent(now) {
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
        if self.path_timestamp_is_recent(now) {
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
        if self.path_timestamp_is_recent(now) {
            self.set_queue_for_path_time(LOGICFRAMES_PER_SECOND * 2);
            return;
        }
        self.set_queue_for_path_time(0);
    }

    /// C++ AIUpdateInterface::isPathAvailable – checks if a path exists
    /// between current position and destination.
    pub fn is_path_available(&self, destination: Coord3D) -> bool {
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
        if self.final_position == Coord3D::ZERO || destination == Coord3D::ZERO {
            return false;
        }

        let surfaces = self.current_locomotor_set_surfaces();
        if surfaces == 0 {
            return false;
        }

        let Some(pathfinder) = crate::ai::THE_AI.read().ok().and_then(|ai| ai.pathfinder()) else {
            return false;
        };
        let Ok(pathfinder) = pathfinder.read() else {
            return false;
        };
        pathfinder
            .find_path(&self.final_position, &destination, surfaces, false)
            .is_some()
    }

    /// C++ AIUpdateInterface::canComputeQuickPath – airborne units can skip
    /// queued pathfinding and build a direct path immediately.
    pub fn can_compute_quick_path(&self) -> bool {
        let set_surfaces = self.current_locomotor_set_surfaces();
        set_surfaces != 0 && (set_surfaces & SURFACE_AIR) != 0 && !self.is_doing_ground_movement()
    }

    fn current_locomotor_set_surfaces(&self) -> u32 {
        let Some(entries) = self
            .module_data
            .locomotor_sets()
            .get(&self.cur_locomotor_set)
        else {
            return self.cur_locomotor_surfaces;
        };

        let surfaces = entries
            .iter()
            .filter_map(|entry| LOCOMOTOR_STORE.get_template(entry.as_str()))
            .fold(0, |surfaces, template| surfaces | template.surfaces);

        if surfaces != 0 {
            surfaces
        } else {
            self.cur_locomotor_surfaces
        }
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

    fn path_timestamp_is_recent(&self, now: UnsignedInt) -> bool {
        now < 3 || self.path_timestamp > now - 3
    }

    // -----------------------------------------------------------------------
    // GROUP B – Locomotor bridge
    // -----------------------------------------------------------------------

    /// C++ AIUpdateInterface::doLocomotor – execute locomotor movement along path.
    /// Returns UpdateSleepTime hint.  Ported from AIUpdate.cpp (approx line 2000+).
    pub fn do_locomotor(&mut self) -> UnsignedInt {
        self.choose_locomotor_from_current_set();

        if self.is_blocked {
            self.blocked_frames = self.blocked_frames.saturating_add(1);
        } else {
            self.blocked_frames = 0;
        }
        self.is_blocked = false;
        let blocked = self.blocked_frames > 0;

        if self.cur_locomotor_tag != 0 {
            match self.locomotor_goal_type {
                LocoGoalType::None => {}
                LocoGoalType::PositionOnPath => {
                    if self.path.is_none() && self.waiting_for_path {
                        return u32::MAX;
                    }
                    self.locomotor_move_along_path(blocked);
                }
                LocoGoalType::PositionExplicit => {
                    self.locomotor_move_towards(self.locomotor_goal_data, blocked);
                }
                LocoGoalType::Angle => {
                    self.locomotor_rotate_towards(self.locomotor_goal_data.x);
                }
            }

            if !blocked && self.blocked_frames > 1 {
                self.blocked_frames = 1;
            }
            self.cur_max_blocked_speed = AI_FAST_AS_POSSIBLE;
        }

        0
    }

    fn locomotor_move_along_path(&mut self, blocked: bool) {
        if !self.is_doing_ground_movement() {
            if let Some(goal_pos) = self.path.as_ref().and_then(|path| path.last()).copied() {
                self.locomotor_move_towards(goal_pos, blocked);
            }
            return;
        }

        let Some(path) = self.path.clone() else {
            return;
        };
        let Some((current_pos, current_angle, body_condition)) = self.owner_locomotor_inputs()
        else {
            return;
        };
        let current_speed = self.cur_locomotor_speed;
        let mut speed = {
            let Some(locomotor) = self.cur_locomotor.as_ref() else {
                return;
            };
            self.clamped_desired_speed(locomotor, body_condition)
        };
        speed = self.apply_blocked_speed_limit(speed, blocked);

        let Some(locomotor) = self.cur_locomotor.as_mut() else {
            return;
        };
        let current_frame = TheGameLogic::get_frame();
        let path_changed = locomotor
            .active_path
            .as_ref()
            .map(|active_path| active_path.waypoints != path)
            .unwrap_or(true);
        if path_changed {
            locomotor.active_path = Some(ActivePath::new(path, current_frame));
        }
        let delta_time = 1.0 / LOGICFRAMES_PER_SECOND as Real;
        if let Some((new_pos, new_angle, new_speed)) = locomotor.update_path_following(
            current_pos,
            current_angle,
            current_speed,
            body_condition,
            speed,
            current_frame,
            delta_time,
        ) {
            self.apply_locomotor_result(current_pos, current_angle, new_pos, new_angle, new_speed);
        }
        self.do_final_position = false;
    }

    fn locomotor_move_towards(&mut self, goal_pos: Coord3D, blocked: bool) {
        let Some((current_pos, current_angle, body_condition)) = self.owner_locomotor_inputs()
        else {
            return;
        };
        let current_speed = self.cur_locomotor_speed;
        let mut speed = {
            let Some(locomotor) = self.cur_locomotor.as_ref() else {
                return;
            };
            self.clamped_desired_speed(locomotor, body_condition)
        };
        speed = self.apply_blocked_speed_limit(speed, blocked);

        let Some(locomotor) = self.cur_locomotor.as_mut() else {
            return;
        };
        let delta_time = 1.0 / LOGICFRAMES_PER_SECOND as Real;
        let (new_pos, new_angle, new_speed) = locomotor.move_towards(
            current_pos,
            current_angle,
            current_speed,
            goal_pos,
            speed,
            body_condition,
            delta_time,
        );
        self.apply_locomotor_result(current_pos, current_angle, new_pos, new_angle, new_speed);
        self.do_final_position = false;
    }

    fn locomotor_rotate_towards(&mut self, angle: Real) {
        let Some((current_pos, current_angle, body_condition)) = self.owner_locomotor_inputs()
        else {
            return;
        };
        let current_speed = self.cur_locomotor_speed;
        let Some(locomotor) = self.cur_locomotor.as_mut() else {
            return;
        };
        let delta_time = 1.0 / LOGICFRAMES_PER_SECOND as Real;
        let (new_pos, new_angle, new_speed) = locomotor.loco_update_move_towards_angle(
            current_pos,
            current_angle,
            angle,
            current_speed,
            body_condition,
            delta_time,
        );
        self.apply_locomotor_result(current_pos, current_angle, new_pos, new_angle, new_speed);
        self.do_final_position = false;
    }

    fn owner_locomotor_inputs(&self) -> Option<(Coord3D, Real, LocoBodyDamageType)> {
        OBJECT_REGISTRY.with_object(self.owner_object_id, |owner| {
            (
                *owner.get_position(),
                owner.get_orientation(),
                Self::owner_body_damage_type(owner),
            )
        })
    }

    fn owner_body_damage_type(owner: &Object) -> LocoBodyDamageType {
        owner
            .get_body_module()
            .and_then(|body| body.lock().ok().map(|guard| guard.get_damage_state()))
            .map(|state| match state {
                crate::common::BodyDamageType::Pristine => LocoBodyDamageType::Pristine,
                crate::common::BodyDamageType::Damaged => LocoBodyDamageType::Damaged,
                crate::common::BodyDamageType::ReallyDamaged => LocoBodyDamageType::ReallyDamaged,
                crate::common::BodyDamageType::Rubble => LocoBodyDamageType::Rubble,
            })
            .unwrap_or(LocoBodyDamageType::Pristine)
    }

    fn clamped_desired_speed(&self, locomotor: &Locomotor, condition: LocoBodyDamageType) -> Real {
        let max_speed = locomotor.get_max_speed_for_condition(condition);
        if self.desired_speed == AI_FAST_AS_POSSIBLE || self.desired_speed > max_speed {
            max_speed
        } else {
            self.desired_speed
        }
    }

    fn apply_blocked_speed_limit(&mut self, speed: Real, blocked: bool) -> Real {
        if blocked && speed > self.cur_max_blocked_speed {
            let mut speed = self.cur_max_blocked_speed;
            if self.bump_speed_limit > speed {
                self.bump_speed_limit = speed;
            }
            self.bump_speed_limit *= 0.95;
            speed = self.bump_speed_limit;
            speed
        } else {
            if self.bump_speed_limit < AI_FAST_AS_POSSIBLE {
                if self.bump_speed_limit < speed * 0.2 {
                    self.bump_speed_limit = speed * 0.2;
                }
                self.bump_speed_limit *= 1.05;
            }
            speed.min(self.bump_speed_limit)
        }
    }

    fn apply_locomotor_result(
        &mut self,
        old_pos: Coord3D,
        old_angle: Real,
        new_pos: Coord3D,
        new_angle: Real,
        new_speed: Real,
    ) {
        self.cur_locomotor_speed = new_speed;
        let Some(owner) = OBJECT_REGISTRY.get_object(self.owner_object_id) else {
            return;
        };
        let Ok(mut owner) = owner.write() else {
            return;
        };
        let _ = owner.set_position(&new_pos);
        let _ = owner.set_orientation(new_angle);
        if let Some(physics) = owner.get_physics() {
            if let Ok(mut physics) = physics.lock() {
                let delta_time = 1.0 / LOGICFRAMES_PER_SECOND as Real;
                let velocity = (new_pos - old_pos) / delta_time;
                physics.set_velocity(&velocity);

                let mut yaw_delta = new_angle - old_angle;
                let two_pi = std::f32::consts::PI * 2.0;
                while yaw_delta > std::f32::consts::PI {
                    yaw_delta -= two_pi;
                }
                while yaw_delta < -std::f32::consts::PI {
                    yaw_delta += two_pi;
                }
                physics.set_yaw_rate(yaw_delta / delta_time);
                physics.set_turning(if yaw_delta > 0.0 {
                    1
                } else if yaw_delta < 0.0 {
                    -1
                } else {
                    0
                });
            }
        }

        if let Some(locomotor) = self.cur_locomotor.as_ref() {
            let airborne = owner.get_height_above_terrain()
                > locomotor.template.airborne_targeting_height as Real;
            owner.set_status(
                ObjectStatusMaskType::from_status(ObjectStatusTypes::AirborneTarget),
                airborne,
            );
        }
    }

    /// C++ AIUpdateInterface::setLocomotorGoalPositionOnPath – sets the
    /// locomotor movement target from the current path.
    pub fn set_locomotor_goal_position_on_path(&mut self) {
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
        self.locomotor_goal_data.x = angle;
    }

    /// C++ AIUpdateInterface::setLocomotorGoalNone
    pub fn set_locomotor_goal_none(&mut self) {
        self.locomotor_goal_type = LocoGoalType::None;
    }

    /// C++ AIUpdateInterface::getCurLocomotorSpeed – current speed for AI decisions.
    /// Ported from AIUpdate.cpp:774.
    pub fn get_cur_locomotor_speed(&self) -> Real {
        if self.cur_locomotor_tag == 0 {
            return 0.0;
        }

        LOCOMOTOR_STORE
            .get_template(self.cur_locomotor_template.as_str())
            .map(|template| template.max_speed)
            .unwrap_or(0.0)
    }

    fn choose_locomotor_for_owner_position(
        &self,
        entries: &[AsciiString],
    ) -> Option<(usize, AsciiString, u32)> {
        if self.owner_object_id == INVALID_ID {
            return None;
        }

        let Some((position, is_crusher)) = OBJECT_REGISTRY
            .with_object(self.owner_object_id, |owner| {
                (owner.get_position().clone(), owner.get_crusher_level() > 0)
            })
        else {
            return None;
        };

        let pathfinder_arc = crate::ai::THE_AI
            .read()
            .ok()
            .and_then(|ai| ai.pathfinder())?;
        let pathfinder = pathfinder_arc.read().ok()?;
        let ignore_obstacle_id = if self.ignore_obstacle_id == INVALID_ID {
            None
        } else {
            Some(self.ignore_obstacle_id)
        };

        entries.iter().enumerate().find_map(|(index, entry)| {
            let template = LOCOMOTOR_STORE.get_template(entry.as_str())?;
            pathfinder
                .valid_movement_position_for_surfaces(
                    template.surfaces,
                    is_crusher,
                    &position,
                    ignore_obstacle_id,
                )
                .then(|| (index, entry.clone(), template.surfaces))
        })
    }

    fn choose_ground_locomotor(entries: &[AsciiString]) -> Option<(usize, AsciiString, u32)> {
        entries.iter().enumerate().find_map(|(index, entry)| {
            let template = LOCOMOTOR_STORE.get_template(entry.as_str())?;
            ((template.surfaces & SURFACE_GROUND) != 0)
                .then(|| (index, entry.clone(), template.surfaces))
        })
    }

    /// C++ AIUpdateInterface::chooseGoodLocomotorFromCurrentSet – selects the
    /// best locomotor from the current set for the object's current position.
    /// Ported from AIUpdate.cpp:833.
    pub fn choose_locomotor_from_current_set(&mut self) {
        let previous_locomotor_tag = self.cur_locomotor_tag;
        let previous_locomotor_template = self.cur_locomotor_template.clone();
        let previous_locomotor_surfaces = self.cur_locomotor_surfaces;
        let entries = self
            .module_data
            .locomotor_sets()
            .get(&self.cur_locomotor_set)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let selected_entry = self
            .choose_locomotor_for_owner_position(entries)
            .or_else(|| {
                (previous_locomotor_tag == 0)
                    .then(|| Self::choose_ground_locomotor(entries))
                    .flatten()
            });

        if let Some((index, entry, surfaces)) = selected_entry {
            self.cur_locomotor_tag = (index + 1) as UnsignedInt;
            self.cur_locomotor_template = entry.clone();
            self.cur_locomotor_surfaces = surfaces;
        } else if previous_locomotor_tag != 0 {
            self.cur_locomotor_tag = previous_locomotor_tag;
            self.cur_locomotor_template = previous_locomotor_template;
            self.cur_locomotor_surfaces = previous_locomotor_surfaces;
        } else {
            self.cur_locomotor_tag = 0;
            self.cur_locomotor_template.clear();
            self.cur_locomotor_surfaces = 0;
        }
        self.sync_cur_locomotor_state();
    }

    fn sync_cur_locomotor_state(&mut self) {
        if self.cur_locomotor_tag == 0 || self.cur_locomotor_template.is_empty() {
            self.cur_locomotor = None;
            self.cur_locomotor_speed = 0.0;
            return;
        }

        let template_name = self.cur_locomotor_template.as_str();
        let is_current = self
            .cur_locomotor
            .as_ref()
            .map(|locomotor| locomotor.get_template_name() == template_name)
            .unwrap_or(false);
        if !is_current {
            self.cur_locomotor = LOCOMOTOR_STORE.create_locomotor(template_name);
            self.cur_locomotor_speed = 0.0;
        }
    }

    /// C++ AIUpdateInterface::isDoingGroundMovement – true if moving along ground.
    pub fn is_doing_ground_movement(&self) -> bool {
        if self.current_locomotor_set_surfaces() == SURFACE_AIR {
            return false;
        }
        if self.cur_locomotor_tag == 0 {
            return false;
        }
        if (self.cur_locomotor_surfaces & SURFACE_AIR) != 0 {
            return false;
        }
        (self.cur_locomotor_surfaces & SURFACE_GROUND) != 0
    }

    // -----------------------------------------------------------------------
    // GROUP C – Physics / Collision
    // -----------------------------------------------------------------------

    /// C++ AIUpdateInterface::processCollision – returns true if the physics
    /// collide should apply the force.  Determines blocking and stuck state.
    /// Ported from AIUpdate.cpp:1410.
    pub fn process_collision(&mut self, other_id: ObjectID) -> bool {
        if self.ignore_collisions_until > TheGameLogic::get_frame() || self.can_path_through_units {
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

    fn normalize_relative_angle(mut angle: Real) -> Real {
        while angle > std::f32::consts::PI {
            angle -= std::f32::consts::PI * 2.0;
        }
        while angle < -std::f32::consts::PI {
            angle += std::f32::consts::PI * 2.0;
        }
        angle
    }

    fn relative_angle(direction: (Real, Real), vector_to_target: (Real, Real)) -> Real {
        let facing_angle = direction.1.atan2(direction.0);
        let target_angle = vector_to_target.1.atan2(vector_to_target.0);
        Self::normalize_relative_angle(target_angle - facing_angle)
    }

    fn physics_velocity(obj: &Object) -> (Vec3D, bool) {
        obj.get_physics()
            .and_then(|physics| physics.lock().ok().map(|physics| physics.get_velocity()))
            .map(|velocity| (velocity, true))
            .unwrap_or((Vec3D::ZERO, false))
    }

    fn self_collision_snapshot(&self) -> Option<CollisionObjectSnapshot> {
        let obj_arc = OBJECT_REGISTRY.get_object(self.owner_object_id)?;
        let obj = obj_arc.read().ok()?;
        let (velocity, has_physics) = Self::physics_velocity(&obj);
        Some(CollisionObjectSnapshot {
            id: obj.get_id(),
            position: *obj.get_position(),
            direction: obj.get_unit_direction_vector_2d(),
            velocity,
            has_physics,
            is_infantry: obj.is_kind_of(KindOf::Infantry),
            is_vehicle: obj.is_kind_of(KindOf::Vehicle),
            is_dozer: obj.is_kind_of(KindOf::Dozer),
            moving: self.is_moving,
            ground: self.is_doing_ground_movement(),
            dead: self.is_ai_dead,
            path_destination: self.path.as_ref().and_then(|path| path.last().copied()),
            frames_blocked: self.blocked_frames,
            formation_id: obj.get_formation_id(),
        })
    }

    fn other_collision_snapshot(other_id: ObjectID) -> Option<CollisionObjectSnapshot> {
        let obj_arc = OBJECT_REGISTRY.get_object(other_id)?;
        let (
            id,
            position,
            direction,
            velocity,
            has_physics,
            is_infantry,
            is_vehicle,
            is_dozer,
            formation_id,
            ai,
        ) = {
            let obj = obj_arc.read().ok()?;
            let (velocity, has_physics) = Self::physics_velocity(&obj);
            (
                obj.get_id(),
                *obj.get_position(),
                obj.get_unit_direction_vector_2d(),
                velocity,
                has_physics,
                obj.is_kind_of(KindOf::Infantry),
                obj.is_kind_of(KindOf::Vehicle),
                obj.is_kind_of(KindOf::Dozer),
                obj.get_formation_id(),
                obj.get_ai_update_interface()?,
            )
        };
        let ai = ai.lock().ok()?;
        Some(CollisionObjectSnapshot {
            id,
            position,
            direction,
            velocity,
            has_physics,
            is_infantry,
            is_vehicle,
            is_dozer,
            moving: ai.is_moving(),
            ground: ai.is_doing_ground_movement(),
            dead: ai.is_ai_in_dead_state(),
            path_destination: ai.get_path_destination(),
            frames_blocked: ai.get_num_frames_blocked(),
            formation_id,
        })
    }

    fn can_crush_or_squish(&self, other_id: ObjectID) -> bool {
        OBJECT_REGISTRY
            .with_object(self.owner_object_id, |owner| {
                OBJECT_REGISTRY
                    .with_object(other_id, |other| {
                        owner.can_crush_or_squish(other, CrushSquishTestType::TestCrushOrSquish)
                    })
                    .unwrap_or(false)
            })
            .unwrap_or(false)
    }

    fn has_higher_path_priority(
        &self,
        owner: &CollisionObjectSnapshot,
        other: &CollisionObjectSnapshot,
    ) -> bool {
        if owner.is_dozer && !other.is_dozer {
            return true;
        }
        if !owner.is_dozer && other.is_dozer {
            return false;
        }
        if owner.is_vehicle && other.is_infantry {
            return true;
        }
        if owner.is_infantry && other.is_vehicle {
            return false;
        }

        let dot = owner.direction.0 * other.direction.0 + owner.direction.1 * other.direction.1;
        if dot <= 0.0 {
            return owner.id < other.id;
        }

        let combined = (
            owner.direction.0 + other.direction.0,
            owner.direction.1 + other.direction.1,
        );
        let vector_to_other = (
            other.position.x - owner.position.x,
            other.position.y - owner.position.y,
        );
        let dot_product = combined.0 * vector_to_other.0 + combined.1 * vector_to_other.1;
        if dot_product > 0.0 {
            return false;
        }
        if dot_product < 0.0 {
            return true;
        }
        owner.id < other.id
    }

    /// C++ AIUpdateInterface::blockedBy – returns true if we are blocked by
    /// the other object.  Ported from AIUpdate.cpp:1272.
    pub fn blocked_by(&self, other_id: ObjectID) -> bool {
        if !self.is_moving {
            return false;
        }
        if self.is_approach_path {
            return false;
        }
        let Some(owner) = self.self_collision_snapshot() else {
            return false;
        };
        let Some(other) = Self::other_collision_snapshot(other_id) else {
            return false;
        };

        if let Some(goal) = owner.path_destination {
            let dx = (goal.x - owner.position.x).abs();
            let dy = (goal.y - owner.position.y).abs();
            if dx < PATHFIND_CELL_SIZE_F && dy < PATHFIND_CELL_SIZE_F {
                return false;
            }
        }

        if self.can_crush_or_squish(other_id) {
            return false;
        }
        if !other.ground {
            return false;
        }

        let dx = owner.position.x - other.position.x;
        let dy = owner.position.y - other.position.y;
        let cur_dist_sqr = dx * dx + dy * dy;

        let dot_dir = owner.direction.0 * other.direction.0 + owner.direction.1 * other.direction.1;
        if owner.is_infantry && other.is_infantry {
            return false;
        }

        let same_cell = PATHFIND_CELL_SIZE_F * PATHFIND_CELL_SIZE_F * 0.0001;
        if cur_dist_sqr < same_cell {
            return self.has_higher_path_priority(&owner, &other);
        }

        if owner.frames_blocked > LOGICFRAMES_PER_SECOND && dot_dir <= 0.0 {
            return false;
        }

        let vector_to_other = (
            other.position.x - owner.position.x,
            other.position.y - owner.position.y,
        );
        let collision_angle = Self::relative_angle(owner.direction, vector_to_other);
        let other_angle =
            Self::relative_angle(other.direction, (-vector_to_other.0, -vector_to_other.1));
        if collision_angle > std::f32::consts::PI / 2.0
            || collision_angle < -std::f32::consts::PI / 2.0
        {
            return false;
        }

        let mut angle_limit = std::f32::consts::PI / 4.0;
        if !other.moving {
            angle_limit *= 0.75;
        }
        if collision_angle > angle_limit || collision_angle < -angle_limit {
            if dot_dir <= 0.0 {
                return false;
            }
            if other.moving && (other_angle > angle_limit || other_angle < -angle_limit) {
                let adjusted_dx = dx + owner.direction.0 - other.direction.0;
                let adjusted_dy = dy + owner.direction.1 - other.direction.1;
                if cur_dist_sqr > adjusted_dx * adjusted_dx + adjusted_dy * adjusted_dy {
                    if self.has_higher_path_priority(&owner, &other) {
                        return false;
                    }
                } else {
                    return false;
                }
            } else {
                return false;
            }
        }

        !other.dead
    }

    /// C++ AIUpdateInterface::calculateMaxBlockedSpeed – max speed we can have
    /// and not run into the blocking unit.  Ported from AIUpdate.cpp:1234.
    pub fn calculate_max_blocked_speed(&self, other_id: ObjectID) -> Real {
        let Some(owner) = self.self_collision_snapshot() else {
            return self.cur_max_blocked_speed;
        };
        let Some(other) = Self::other_collision_snapshot(other_id) else {
            return self.cur_max_blocked_speed;
        };

        let mut vector_to_other = (
            other.position.x - owner.position.x,
            other.position.y - owner.position.y,
        );
        let length =
            (vector_to_other.0 * vector_to_other.0 + vector_to_other.1 * vector_to_other.1).sqrt();
        if length > 0.0 {
            vector_to_other.0 /= length;
            vector_to_other.1 /= length;
        }

        let dot_product =
            vector_to_other.0 * other.direction.0 + vector_to_other.1 * other.direction.1;
        if dot_product < 0.0 {
            return 0.0;
        }
        if !other.has_physics {
            return self.cur_max_blocked_speed;
        }

        let mut other_velocity = other.velocity;
        other_velocity.z = 0.0;
        let away_speed = other_velocity.length() * dot_product;
        let towards_dot =
            vector_to_other.0 * owner.direction.0 + vector_to_other.1 * owner.direction.1;
        if towards_dot <= 0.0 {
            return self.cur_max_blocked_speed;
        }

        let mut max_speed = away_speed / towards_dot;
        if other.formation_id != FormationID::NONE && owner.formation_id == other.formation_id {
            max_speed *= 0.55;
        }
        max_speed.min(self.cur_max_blocked_speed)
    }

    /// C++ AIUpdateInterface::needToRotate – returns true if we need to rotate
    /// to point in our path's direction.  Ported from AIUpdate.cpp:1380.
    pub fn need_to_rotate(&self) -> bool {
        if self.waiting_for_path {
            return true;
        }
        let Some(path) = self.path.as_ref() else {
            return false;
        };
        let Some((position, orientation)) = OBJECT_REGISTRY
            .with_object(self.owner_object_id, |owner| {
                (*owner.get_position(), owner.get_orientation())
            })
        else {
            return false;
        };
        let Some(path_point) = path
            .iter()
            .copied()
            .find(|point| {
                let dx = point.x - position.x;
                let dy = point.y - position.y;
                dx * dx + dy * dy > f32::EPSILON
            })
            .or_else(|| path.last().copied())
        else {
            return false;
        };
        let delta = path_point - position;
        if delta.length_squared() < f32::EPSILON {
            return false;
        }
        let desired_angle = delta.y.atan2(delta.x);
        let delta_angle = Self::normalize_relative_angle(desired_angle - orientation);
        delta_angle.abs() > (std::f32::consts::PI / 30.0)
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
        if self.is_in_update || self.owner_object_id == INVALID_ID {
            return;
        }
        TheGameLogic::set_wake_frame(self.owner_object_id, crate::modules::UPDATE_SLEEP_NONE);
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
        self.ignore_collisions_until = if frames != 0 {
            TheGameLogic::get_frame().saturating_add(frames)
        } else {
            0
        };
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
        self.cur_locomotor_template.clear();
        self.cur_locomotor_surfaces = 0;
        self.cur_locomotor = None;
        self.cur_locomotor_speed = 0.0;
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
    use crate::ai::pathfind_astar::PathfindCellType;
    use crate::locomotor::SURFACE_WATER;
    use crate::object::Object;
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;
    use std::sync::RwLock;

    struct RegisteredObjectCleanup(ObjectID);

    impl Drop for RegisteredObjectCleanup {
        fn drop(&mut self) {
            OBJECT_REGISTRY.unregister_object(self.0);
        }
    }

    struct PathfinderCellCleanup(Coord3D);

    impl Drop for PathfinderCellCleanup {
        fn drop(&mut self) {
            let Some(pathfinder) = crate::ai::THE_AI.read().ok().and_then(|ai| ai.pathfinder())
            else {
                return;
            };
            if let Ok(mut pathfinder) = pathfinder.write() {
                pathfinder.set_cell_type_for_test(&self.0, PathfindCellType::Clear);
            };
        }
    }

    fn ai_update() -> AIUpdateInterface {
        AIUpdateInterface::new(Arc::new(AIUpdateModuleData::default()))
    }

    fn ai_update_with_locomotors() -> AIUpdateInterface {
        let mut data = AIUpdateModuleData::default();
        data.add_locomotor_set_entry(LocomotorSetType::Normal, "Wheeled".into());
        data.add_locomotor_set_entry(LocomotorSetType::NormalUpgraded, "Tracked".into());
        AIUpdateInterface::new(Arc::new(data))
    }

    fn ai_update_with_air_locomotor() -> AIUpdateInterface {
        let mut data = AIUpdateModuleData::default();
        data.add_locomotor_set_entry(LocomotorSetType::Supersonic, "Thrust".into());
        AIUpdateInterface::new(Arc::new(data))
    }

    #[test]
    fn new_for_object_records_owner_for_wake_up_now_like_cpp() {
        let ai = AIUpdateInterface::new_for_object(Arc::new(AIUpdateModuleData::default()), 42);

        assert_eq!(ai.owner_object_id, 42);
    }

    #[test]
    fn wake_up_now_noops_while_in_update_like_cpp() {
        let mut ai = AIUpdateInterface::new_for_object(Arc::new(AIUpdateModuleData::default()), 42);
        ai.is_in_update = true;

        ai.wake_up_now();

        assert_eq!(ai.owner_object_id, 42);
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

    #[derive(Debug, Default)]
    struct CollisionAi {
        moving: bool,
        ground: bool,
        dead: bool,
        path_destination: Option<Coord3D>,
        frames_blocked: u32,
    }

    impl crate::modules::AIUpdateInterface for CollisionAi {
        fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }

        fn is_moving(&self) -> bool {
            self.moving
        }

        fn is_idle(&self) -> bool {
            !self.moving
        }

        fn set_movement_target(&mut self, _target: &Coord3D) -> Result<(), String> {
            Ok(())
        }

        fn is_doing_ground_movement(&self) -> bool {
            self.ground
        }

        fn is_ai_in_dead_state(&self) -> bool {
            self.dead
        }

        fn get_path_destination(&self) -> Option<Coord3D> {
            self.path_destination
        }

        fn get_num_frames_blocked(&self) -> u32 {
            self.frames_blocked
        }
    }

    fn register_collision_object(
        id: ObjectID,
        position: Coord3D,
        orientation: Real,
        ai: Option<CollisionAi>,
    ) -> (Arc<RwLock<Object>>, RegisteredObjectCleanup) {
        let object = Arc::new(RwLock::new(Object::new_test(id, 100.0)));
        {
            let mut object_guard = object.write().unwrap();
            object_guard.set_position(&position).unwrap();
            object_guard.set_orientation(orientation).unwrap();
            if let Some(ai) = ai {
                let ai: Arc<Mutex<dyn crate::modules::AIUpdateInterface>> =
                    Arc::new(Mutex::new(ai));
                object_guard.set_ai_update_interface(Some(ai));
            }
        }
        OBJECT_REGISTRY.register_object(id, &object);
        (object, RegisteredObjectCleanup(id))
    }

    fn add_primary_weapon(object: &mut Object, range: Real) {
        let mut weapon_template =
            crate::weapon::WeaponTemplate::new(format!("AIUpdateTestWeapon{}", object.get_id()));
        weapon_template.attack_range = range;
        weapon_template.minimum_attack_range = 0.0;

        let mut template_set = crate::weapon::WeaponTemplateSet::new();
        template_set.set_weapon_template(WeaponSlotType::Primary, Arc::new(weapon_template));
        object.weapon_set.add_weapon_template_set(template_set);
        object
            .weapon_set
            .update_weapon_set(object.get_id(), &crate::weapon::WeaponSetFlags::new())
            .unwrap();
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
        assert_eq!(
            ai.ignore_collisions_until,
            TheGameLogic::get_frame().saturating_add(LOGICFRAMES_PER_SECOND * 2)
        );
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
    fn set_locomotor_goal_position_on_path_without_path_still_sets_position_on_path_like_cpp() {
        let mut ai = ai_update();
        ai.path = None;
        ai.locomotor_goal_data = Coord3D::new(4.0, 5.0, 6.0);

        ai.set_locomotor_goal_position_on_path();

        assert_eq!(ai.get_locomotor_goal_type(), LocoGoalType::PositionOnPath);
        assert_eq!(ai.locomotor_goal_data, Coord3D::ZERO);
    }

    #[test]
    fn set_locomotor_goal_orientation_preserves_existing_y_z_like_cpp() {
        let mut ai = ai_update();
        ai.locomotor_goal_data = Coord3D::new(1.0, 2.0, 3.0);

        ai.set_locomotor_goal_orientation(45.0);

        assert_eq!(ai.get_locomotor_goal_type(), LocoGoalType::Angle);
        assert_eq!(ai.locomotor_goal_data, Coord3D::new(45.0, 2.0, 3.0));
    }

    #[test]
    fn set_locomotor_goal_none_preserves_goal_data_like_cpp() {
        let mut ai = ai_update();
        ai.locomotor_goal_data = Coord3D::new(7.0, 8.0, 9.0);

        ai.set_locomotor_goal_none();

        assert_eq!(ai.get_locomotor_goal_type(), LocoGoalType::None);
        assert_eq!(ai.locomotor_goal_data, Coord3D::new(7.0, 8.0, 9.0));
    }

    #[test]
    fn compute_and_destroy_path_update_cpp_state() {
        let mut ai = ai_update_with_locomotors();
        assert!(ai.choose_locomotor_set(LocomotorSetType::Normal));
        ai.set_final_position(Coord3D::new(1.0, 2.0, 0.0));
        ai.is_blocked = true;
        ai.is_blocked_and_stuck = true;
        ai.retry_path = true;
        ai.set_queue_for_path_time(LOGICFRAMES_PER_SECOND);

        assert!(ai.compute_path(Coord3D::new(8.0, 9.0, 0.0)));
        assert!(ai.get_path().is_some());
        assert_eq!(ai.get_path_timestamp(), TheGameLogic::get_frame());
        assert!(!ai.is_blocked);
        assert!(!ai.is_blocked_and_stuck);
        assert!(!ai.retry_path);
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
    fn compute_path_destroys_stale_non_stuck_path_before_failure_like_cpp() {
        let mut ai = ai_update();
        ai.path = Some(vec![
            Coord3D::new(2.0, 3.0, 0.0),
            Coord3D::new(4.0, 5.0, 0.0),
        ]);
        ai.waiting_for_path = true;
        ai.is_attack_path = true;
        ai.retry_path = true;

        assert!(!ai.compute_path(Coord3D::new(8.0, 9.0, 0.0)));

        assert!(ai.get_path().is_none());
        assert!(!ai.is_waiting_for_path());
        assert!(!ai.is_attack_path);
        assert!(!ai.retry_path);
        assert_eq!(ai.get_locomotor_goal_type(), LocoGoalType::None);
    }

    #[test]
    fn compute_path_failure_records_attempt_and_clears_stuck_state_like_cpp() {
        let mut ai = ai_update();
        ai.blocked_frames = 42;
        ai.is_blocked = true;
        ai.is_blocked_and_stuck = true;

        assert!(!ai.compute_path(Coord3D::new(8.0, 9.0, 0.0)));

        assert_eq!(ai.get_path_timestamp(), TheGameLogic::get_frame());
        assert_eq!(ai.blocked_frames, 0);
        assert!(ai.is_blocked);
        assert!(!ai.is_blocked_and_stuck);
        assert!(ai.get_path().is_none());
    }

    #[test]
    fn request_path_computes_airborne_quick_path_like_cpp() {
        let mut ai = ai_update();
        ai.set_final_position(Coord3D::new(4.0, 5.0, 1.0));
        ai.cur_locomotor_set = LocomotorSetType::Supersonic;
        ai.cur_locomotor_tag = 1;
        ai.cur_locomotor_surfaces = SURFACE_AIR;
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
        ai.cur_locomotor_surfaces = SURFACE_AIR;
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
    fn quick_path_requires_air_surface_like_cpp() {
        let mut ai = ai_update();
        ai.cur_locomotor_set = LocomotorSetType::Supersonic;
        ai.cur_locomotor_tag = 1;
        ai.cur_locomotor_surfaces = 0;
        assert!(!ai.can_compute_quick_path());

        ai.cur_locomotor_surfaces = SURFACE_AIR;
        assert!(ai.can_compute_quick_path());
    }

    #[test]
    fn quick_path_uses_locomotor_set_surfaces_like_cpp() {
        let mut ground_ai = ai_update_with_locomotors();
        assert!(ground_ai.choose_locomotor_set(LocomotorSetType::Normal));
        ground_ai.cur_locomotor_surfaces = SURFACE_AIR;
        assert!(!ground_ai.can_compute_quick_path());

        let mut air_ai = ai_update_with_air_locomotor();
        assert!(air_ai.choose_locomotor_set(LocomotorSetType::Supersonic));
        assert!(air_ai.can_compute_quick_path());
    }

    #[test]
    fn is_doing_ground_movement_uses_current_locomotor_surfaces_like_cpp() {
        let mut ai = ai_update();

        ai.cur_locomotor_surfaces = SURFACE_GROUND;
        assert!(!ai.is_doing_ground_movement());

        ai.cur_locomotor_tag = 1;
        assert!(ai.is_doing_ground_movement());

        ai.cur_locomotor_surfaces = SURFACE_AIR;
        assert!(!ai.is_doing_ground_movement());

        ai.cur_locomotor_surfaces = SURFACE_GROUND | SURFACE_AIR;
        assert!(!ai.is_doing_ground_movement());
    }

    #[test]
    fn is_doing_ground_movement_air_only_set_wins_over_stale_current_locomotor_like_cpp() {
        let mut ai = ai_update_with_air_locomotor();
        ai.cur_locomotor_set = LocomotorSetType::Supersonic;
        ai.cur_locomotor_tag = 1;
        ai.cur_locomotor_surfaces = SURFACE_GROUND;

        assert!(!ai.is_doing_ground_movement());
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
        assert!(!ai.is_path_available(Coord3D::new(99.0, 99.0, 0.0)));

        let mut ai = ai_update_with_locomotors();
        assert!(ai.choose_locomotor_set(LocomotorSetType::Normal));
        ai.set_final_position(Coord3D::new(1.0, 2.0, 0.0));
        assert!(ai.is_path_available(Coord3D::new(99.0, 99.0, 0.0)));
        assert!(!ai.is_path_available(Coord3D::new(20_000.0, 20_000.0, 0.0)));
    }

    #[test]
    fn blocked_by_detects_live_ground_object_ahead_like_cpp() {
        let owner_id = 7_301;
        let other_id = 7_302;
        let (_owner, _owner_cleanup) =
            register_collision_object(owner_id, Coord3D::ZERO, 0.0, None);
        let (_other, _other_cleanup) = register_collision_object(
            other_id,
            Coord3D::new(20.0, 0.0, 0.0),
            0.0,
            Some(CollisionAi {
                ground: true,
                ..CollisionAi::default()
            }),
        );
        let mut ai =
            AIUpdateInterface::new_for_object(Arc::new(AIUpdateModuleData::default()), owner_id);
        ai.cur_locomotor_tag = 1;
        ai.cur_locomotor_surfaces = SURFACE_GROUND;
        ai.is_moving = true;
        ai.path = Some(vec![Coord3D::new(100.0, 0.0, 0.0)]);

        assert!(ai.blocked_by(other_id));
    }

    #[test]
    fn blocked_by_ignores_near_final_goal_like_cpp() {
        let owner_id = 7_303;
        let other_id = 7_304;
        let (_owner, _owner_cleanup) =
            register_collision_object(owner_id, Coord3D::ZERO, 0.0, None);
        let (_other, _other_cleanup) = register_collision_object(
            other_id,
            Coord3D::new(2.0, 0.0, 0.0),
            0.0,
            Some(CollisionAi {
                ground: true,
                ..CollisionAi::default()
            }),
        );
        let mut ai =
            AIUpdateInterface::new_for_object(Arc::new(AIUpdateModuleData::default()), owner_id);
        ai.cur_locomotor_tag = 1;
        ai.cur_locomotor_surfaces = SURFACE_GROUND;
        ai.is_moving = true;
        ai.path = Some(vec![Coord3D::new(5.0, 5.0, 0.0)]);

        assert!(!ai.blocked_by(other_id));
    }

    #[test]
    fn calculate_max_blocked_speed_returns_zero_when_other_moves_toward_owner_like_cpp() {
        let owner_id = 7_305;
        let other_id = 7_306;
        let (_owner, _owner_cleanup) =
            register_collision_object(owner_id, Coord3D::ZERO, 0.0, None);
        let (_other, _other_cleanup) = register_collision_object(
            other_id,
            Coord3D::new(20.0, 0.0, 0.0),
            std::f32::consts::PI,
            Some(CollisionAi {
                ground: true,
                ..CollisionAi::default()
            }),
        );
        let mut ai =
            AIUpdateInterface::new_for_object(Arc::new(AIUpdateModuleData::default()), owner_id);
        ai.cur_locomotor_tag = 1;
        ai.cur_locomotor_surfaces = SURFACE_GROUND;
        ai.cur_max_blocked_speed = 12.0;

        assert_eq!(ai.calculate_max_blocked_speed(other_id), 0.0);
    }

    #[test]
    fn process_collision_marks_stationary_blocker_stuck_like_cpp() {
        let owner_id = 7_307;
        let other_id = 7_308;
        let (_owner, _owner_cleanup) =
            register_collision_object(owner_id, Coord3D::ZERO, 0.0, None);
        let (_other, _other_cleanup) = register_collision_object(
            other_id,
            Coord3D::new(20.0, 0.0, 0.0),
            std::f32::consts::PI,
            Some(CollisionAi {
                ground: true,
                ..CollisionAi::default()
            }),
        );
        let mut ai =
            AIUpdateInterface::new_for_object(Arc::new(AIUpdateModuleData::default()), owner_id);
        ai.cur_locomotor_tag = 1;
        ai.cur_locomotor_surfaces = SURFACE_GROUND;
        ai.cur_max_blocked_speed = AI_FAST_AS_POSSIBLE;
        ai.is_moving = true;
        ai.path = Some(vec![Coord3D::new(100.0, 0.0, 0.0)]);

        assert!(!ai.process_collision(other_id));
        assert!(ai.is_blocked());
        assert_eq!(ai.get_blocked_frames(), 1);
        assert_eq!(ai.cur_max_blocked_speed, 0.0);
        assert!(ai.is_blocked_and_stuck());
    }

    #[test]
    fn need_to_rotate_uses_owner_facing_and_path_direction_like_cpp() {
        let owner_id = 7_309;
        let (_owner, _owner_cleanup) =
            register_collision_object(owner_id, Coord3D::ZERO, 0.0, None);
        let mut ai =
            AIUpdateInterface::new_for_object(Arc::new(AIUpdateModuleData::default()), owner_id);

        ai.path = Some(vec![Coord3D::new(10.0, 0.0, 0.0)]);
        assert!(!ai.need_to_rotate());

        ai.path = Some(vec![Coord3D::new(0.0, 10.0, 0.0)]);
        assert!(ai.need_to_rotate());

        ai.waiting_for_path = true;
        ai.path = None;
        assert!(ai.need_to_rotate());
    }

    #[test]
    fn do_pathfind_approach_builds_current_bridge_path() {
        let mut ai = ai_update_with_locomotors();
        assert!(ai.choose_locomotor_set(LocomotorSetType::Normal));
        ai.set_final_position(Coord3D::new(3.0, 4.0, 0.0));

        ai.request_approach_path(Coord3D::new(12.0, 16.0, 0.0));
        ai.do_pathfind();

        assert!(!ai.is_waiting_for_path());
        assert!(ai.is_approach_path);
        assert_eq!(ai.get_locomotor_goal_type(), LocoGoalType::PositionOnPath);
        let path = ai.get_path().as_ref().unwrap();
        assert_eq!(path.first().copied(), Some(Coord3D::new(3.0, 4.0, 0.0)));
        assert_eq!(path.last().copied(), Some(Coord3D::new(12.0, 16.0, 0.0)));
    }

    #[test]
    fn do_pathfind_attack_fallback_clears_attack_flag_like_cpp() {
        let mut ai = ai_update_with_locomotors();
        assert!(ai.choose_locomotor_set(LocomotorSetType::Normal));
        ai.set_final_position(Coord3D::new(6.0, 7.0, 0.0));

        ai.request_attack_path(123, Coord3D::new(18.0, 21.0, 0.0));
        ai.do_pathfind();

        assert!(!ai.is_waiting_for_path());
        assert!(!ai.is_attack_path);
        assert_eq!(ai.get_locomotor_goal_type(), LocoGoalType::PositionOnPath);
        let path = ai.get_path().as_ref().unwrap();
        assert_eq!(path.first().copied(), Some(Coord3D::new(6.0, 7.0, 0.0)));
        assert_eq!(path.last().copied(), Some(Coord3D::new(18.0, 21.0, 0.0)));
    }

    #[test]
    fn do_pathfind_attack_in_range_finishes_without_move_path_like_cpp() {
        let owner_id = 7_401;
        let victim_id = 7_402;
        let (owner, _owner_cleanup) = register_collision_object(owner_id, Coord3D::ZERO, 0.0, None);
        add_primary_weapon(&mut owner.write().unwrap(), 80.0);
        let (_victim, _victim_cleanup) =
            register_collision_object(victim_id, Coord3D::new(20.0, 0.0, 0.0), 0.0, None);
        let mut ai =
            AIUpdateInterface::new_for_object(Arc::new(AIUpdateModuleData::default()), owner_id);
        ai.cur_locomotor_tag = 1;
        ai.cur_locomotor_surfaces = SURFACE_GROUND;
        ai.path = Some(vec![Coord3D::ZERO, Coord3D::new(4.0, 0.0, 0.0)]);
        ai.path_timestamp = 1;

        let mut logic = crate::system::game_logic::lock_game_logic().unwrap();
        logic.set_current_frame(10);
        drop(logic);

        ai.request_attack_path(victim_id, Coord3D::new(20.0, 0.0, 0.0));
        ai.do_pathfind();

        assert!(!ai.is_waiting_for_path());
        assert!(ai.is_attack_path);
        assert!(ai.get_path().is_none());
        assert_eq!(ai.get_locomotor_goal_type(), LocoGoalType::None);

        let mut logic = crate::system::game_logic::lock_game_logic().unwrap();
        logic.set_current_frame(0);
    }

    #[test]
    fn do_pathfind_attack_reuses_recent_stuck_path_like_cpp() {
        let owner_id = 7_403;
        let victim_id = 7_404;
        let (owner, _owner_cleanup) = register_collision_object(owner_id, Coord3D::ZERO, 0.0, None);
        add_primary_weapon(&mut owner.write().unwrap(), 5.0);
        let (_victim, _victim_cleanup) =
            register_collision_object(victim_id, Coord3D::new(90.0, 0.0, 0.0), 0.0, None);
        let mut logic = crate::system::game_logic::lock_game_logic().unwrap();
        logic.set_current_frame(400);
        drop(logic);

        let mut ai =
            AIUpdateInterface::new_for_object(Arc::new(AIUpdateModuleData::default()), owner_id);
        ai.cur_locomotor_tag = 1;
        ai.cur_locomotor_surfaces = SURFACE_GROUND;
        ai.path = Some(vec![Coord3D::ZERO, Coord3D::new(12.0, 0.0, 0.0)]);
        ai.path_timestamp = 399;
        ai.is_blocked = true;
        ai.is_blocked_and_stuck = true;
        ai.blocked_frames = 8;

        ai.request_attack_path(victim_id, Coord3D::new(90.0, 0.0, 0.0));
        ai.do_pathfind();

        assert!(ai.is_attack_path);
        assert!(ai.get_path().is_some());
        assert_eq!(ai.get_blocked_frames(), 0);
        assert!(!ai.is_blocked());
        assert!(!ai.is_blocked_and_stuck());
        assert_eq!(ai.ignore_collisions_until, 400 + LOGICFRAMES_PER_SECOND * 2);

        let mut logic = crate::system::game_logic::lock_game_logic().unwrap();
        logic.set_current_frame(0);
    }

    #[test]
    fn do_pathfind_safe_path_builds_escape_path_like_cpp_contract() {
        let mut ai = ai_update();
        ai.set_final_position(Coord3D::new(10.0, 10.0, 0.0));

        ai.request_safe_path(777);
        ai.do_pathfind();

        assert!(!ai.is_waiting_for_path());
        assert!(ai.is_safe_path);
        assert_eq!(ai.get_locomotor_goal_type(), LocoGoalType::PositionOnPath);
        let path = ai.get_path().as_ref().expect("safe path should be set");
        assert_eq!(path[0], Coord3D::new(10.0, 10.0, 0.0));
        assert_ne!(path[1], Coord3D::new(10.0, 10.0, 0.0));
        assert_eq!(path[1].z, 0.0);
    }

    #[test]
    fn do_locomotor_without_current_locomotor_preserves_blocked_speed_like_cpp() {
        let mut ai = ai_update();
        ai.cur_max_blocked_speed = 12.0;
        ai.is_blocked = true;

        assert_eq!(ai.do_locomotor(), 0);
        assert_eq!(ai.blocked_frames, 1);
        assert!(!ai.is_blocked);
        assert_eq!(ai.cur_max_blocked_speed, 12.0);
    }

    #[test]
    fn do_locomotor_with_current_locomotor_resets_blocked_speed_like_cpp() {
        let mut ai = ai_update_with_locomotors();
        assert!(ai.choose_locomotor_set(LocomotorSetType::Normal));
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
    fn ignore_collision_time_uses_absolute_expiry_frame_like_cpp() {
        let mut logic = crate::system::game_logic::lock_game_logic().unwrap();
        logic.set_current_frame(100);
        drop(logic);

        let mut ai = ai_update();
        ai.set_ignore_collision_time(LOGICFRAMES_PER_SECOND * 2);

        assert_eq!(ai.ignore_collisions_until, 100 + LOGICFRAMES_PER_SECOND * 2);

        let mut logic = crate::system::game_logic::lock_game_logic().unwrap();
        logic.set_current_frame(u64::from(ai.ignore_collisions_until));
        drop(logic);

        assert!(!ai.process_collision(42));
        assert!(!ai.can_path_through_units);

        let mut logic = crate::system::game_logic::lock_game_logic().unwrap();
        logic.set_current_frame(0);
    }

    #[test]
    fn do_locomotor_waits_forever_for_pending_path_with_current_locomotor_like_cpp() {
        let mut ai = ai_update_with_locomotors();
        assert!(ai.choose_locomotor_set(LocomotorSetType::Normal));
        ai.locomotor_goal_type = LocoGoalType::PositionOnPath;
        ai.waiting_for_path = true;
        ai.path = None;

        assert_eq!(ai.do_locomotor(), u32::MAX);
        assert_eq!(ai.get_locomotor_goal_type(), LocoGoalType::PositionOnPath);
    }

    #[test]
    fn do_locomotor_without_current_locomotor_does_not_wait_for_pending_path_like_cpp() {
        let mut ai = ai_update();
        ai.locomotor_goal_type = LocoGoalType::PositionOnPath;
        ai.waiting_for_path = true;
        ai.path = None;

        assert_eq!(ai.do_locomotor(), 0);
        assert_eq!(ai.get_locomotor_goal_type(), LocoGoalType::PositionOnPath);
    }

    #[test]
    fn do_locomotor_explicit_goal_moves_owner_with_locomotor_like_cpp() {
        let owner_id = 7_501;
        let (owner, _owner_cleanup) = register_collision_object(owner_id, Coord3D::ZERO, 0.0, None);
        let mut data = AIUpdateModuleData::default();
        data.add_locomotor_set_entry(LocomotorSetType::Normal, "Wheeled".into());
        let mut ai = AIUpdateInterface::new_for_object(Arc::new(data), owner_id);
        assert!(ai.choose_locomotor_set(LocomotorSetType::Normal));

        ai.set_locomotor_goal_position_explicit(Coord3D::new(40.0, 0.0, 0.0));
        assert_eq!(ai.do_locomotor(), 0);

        let position = *owner.read().unwrap().get_position();
        assert!(position.x > 0.0);
        assert!(position.y.abs() < 0.01);
        assert!(ai.cur_locomotor_speed > 0.0);
        assert!(!ai.do_final_position);
    }

    #[test]
    fn do_locomotor_path_goal_moves_toward_next_path_point_like_cpp() {
        let owner_id = 7_502;
        let (owner, _owner_cleanup) = register_collision_object(owner_id, Coord3D::ZERO, 0.0, None);
        let mut data = AIUpdateModuleData::default();
        data.add_locomotor_set_entry(LocomotorSetType::Normal, "Wheeled".into());
        let mut ai = AIUpdateInterface::new_for_object(Arc::new(data), owner_id);
        assert!(ai.choose_locomotor_set(LocomotorSetType::Normal));
        ai.path = Some(vec![
            Coord3D::ZERO,
            Coord3D::new(30.0, 0.0, 0.0),
            Coord3D::new(60.0, 0.0, 0.0),
        ]);
        ai.set_locomotor_goal_position_on_path();

        assert_eq!(ai.do_locomotor(), 0);

        let position = *owner.read().unwrap().get_position();
        assert!(position.x > 0.0);
        assert!(position.x < 30.0);
        assert!(ai.cur_locomotor_speed > 0.0);
    }

    #[test]
    fn do_locomotor_angle_goal_rotates_owner_with_locomotor_like_cpp() {
        let owner_id = 7_503;
        let (owner, _owner_cleanup) = register_collision_object(owner_id, Coord3D::ZERO, 0.0, None);
        let mut data = AIUpdateModuleData::default();
        data.add_locomotor_set_entry(LocomotorSetType::Normal, "Wheeled".into());
        let mut ai = AIUpdateInterface::new_for_object(Arc::new(data), owner_id);
        assert!(ai.choose_locomotor_set(LocomotorSetType::Normal));

        ai.set_locomotor_goal_orientation(std::f32::consts::FRAC_PI_2);
        assert_eq!(ai.do_locomotor(), 0);

        let orientation = owner.read().unwrap().get_orientation();
        assert!(orientation > 0.0);
        assert!(orientation < std::f32::consts::FRAC_PI_2);
        assert!(!ai.do_final_position);
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
        assert_eq!(ai.cur_locomotor_template.as_str(), "Wheeled");
        assert_eq!(ai.cur_locomotor_surfaces, SURFACE_GROUND);
        assert_eq!(ai.get_cur_locomotor_speed(), 15.0);
    }

    #[test]
    fn choose_locomotor_preserves_previous_when_no_current_cell_locomotor_like_cpp() {
        let mut ai = ai_update();
        ai.cur_locomotor_set = LocomotorSetType::Wander;
        ai.cur_locomotor_tag = 77;
        ai.cur_locomotor_template = "Wheeled".into();
        ai.cur_locomotor_surfaces = SURFACE_GROUND;

        ai.choose_locomotor_from_current_set();

        assert_eq!(ai.cur_locomotor_tag, 77);
        assert_eq!(ai.cur_locomotor_template.as_str(), "Wheeled");
        assert_eq!(ai.cur_locomotor_surfaces, SURFACE_GROUND);
        assert_eq!(ai.get_cur_locomotor_speed(), 15.0);
    }

    #[test]
    fn choose_locomotor_falls_back_to_ground_when_no_current_cell_locomotor_like_cpp() {
        let mut data = AIUpdateModuleData::default();
        data.add_locomotor_set_entry(LocomotorSetType::Normal, "Thrust".into());
        data.add_locomotor_set_entry(LocomotorSetType::Normal, "Wheeled".into());
        let mut ai = AIUpdateInterface::new(Arc::new(data));
        ai.cur_locomotor_set = LocomotorSetType::Normal;

        ai.choose_locomotor_from_current_set();

        assert_eq!(ai.cur_locomotor_tag, 2);
        assert_eq!(ai.cur_locomotor_template.as_str(), "Wheeled");
        assert_eq!(ai.cur_locomotor_surfaces, SURFACE_GROUND);
    }

    #[test]
    fn choose_locomotor_uses_pathfinder_valid_cell_like_cpp() {
        let object_id = 7_201;
        let position = Coord3D::new(48.0, 48.0, 0.0);
        let object = Arc::new(RwLock::new(Object::new_test(object_id, 100.0)));
        object.write().unwrap().set_position(&position).unwrap();
        OBJECT_REGISTRY.register_object(object_id, &object);
        let _object_cleanup = RegisteredObjectCleanup(object_id);

        let pathfinder = crate::ai::THE_AI
            .read()
            .ok()
            .and_then(|ai| ai.pathfinder())
            .unwrap();
        pathfinder
            .write()
            .unwrap()
            .set_cell_type_for_test(&position, PathfindCellType::Water);
        let _cell_cleanup = PathfinderCellCleanup(position);

        let mut data = AIUpdateModuleData::default();
        data.add_locomotor_set_entry(LocomotorSetType::Normal, "Wheeled".into());
        data.add_locomotor_set_entry(LocomotorSetType::Normal, "Hover".into());
        let mut ai = AIUpdateInterface::new_for_object(Arc::new(data), object_id);
        ai.cur_locomotor_set = LocomotorSetType::Normal;

        ai.choose_locomotor_from_current_set();

        assert_eq!(ai.cur_locomotor_tag, 2);
        assert_eq!(ai.cur_locomotor_template.as_str(), "Hover");
        assert_eq!(ai.cur_locomotor_surfaces, SURFACE_GROUND | SURFACE_WATER);
    }

    #[test]
    fn do_locomotor_reselects_current_locomotor_like_cpp() {
        let mut ai = ai_update_with_locomotors();
        assert!(ai.choose_locomotor_set_explicit(LocomotorSetType::Normal));
        assert_eq!(ai.cur_locomotor_tag, 0);

        ai.do_locomotor();

        assert_ne!(ai.cur_locomotor_tag, 0);
        assert_eq!(ai.cur_locomotor_template.as_str(), "Wheeled");
        assert_eq!(ai.cur_locomotor_surfaces, SURFACE_GROUND);
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
