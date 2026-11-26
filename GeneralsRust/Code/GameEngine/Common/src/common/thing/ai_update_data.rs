//! Authored AI and turret settings from AIUpdate.cpp and TurretAI.cpp.

use super::module::{ModuleData, NameKeyType};
use crate::common::ascii_string::AsciiString;
use crate::common::game_common::LOGICFRAMES_PER_SECOND;
use crate::common::ini::{FieldParse, INI, INIError, INILoadType};
use crate::common::system::{Snapshotable, Xfer};
use std::{any::Any, collections::HashMap};

const WEAPONSLOT_COUNT: usize = 3;

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

/// Locomotor set selection (`LocomotorSetType` in C++).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[repr(i32)]
pub enum LocomotorSetType {
    Invalid = -1,
    Normal = 0,
    NormalUpgraded = 1,
    Freefall = 2,
    Wander = 3,
    Panic = 4,
    Taxiing = 5,
    Supersonic = 6,
    Sluggish = 7,
}

impl LocomotorSetType {
    pub(super) fn from_ini_name(name: &str) -> Option<Self> {
        match name {
            "SET_NORMAL" => Some(Self::Normal),
            "SET_NORMAL_UPGRADED" => Some(Self::NormalUpgraded),
            "SET_FREEFALL" => Some(Self::Freefall),
            "SET_WANDER" => Some(Self::Wander),
            "SET_PANIC" => Some(Self::Panic),
            "SET_TAXIING" => Some(Self::Taxiing),
            "SET_SUPERSONIC" => Some(Self::Supersonic),
            "SET_SLUGGISH" => Some(Self::Sluggish),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TurretAIData {
    pub turn_rate: f32,
    pub pitch_rate: f32,
    pub natural_turret_angle: f32,
    pub natural_turret_pitch: f32,
    pub turret_fire_angle_sweep: [f32; WEAPONSLOT_COUNT],
    pub turret_sweep_speed_modifier: [f32; WEAPONSLOT_COUNT],
    pub fire_pitch: f32,
    pub min_pitch: f32,
    pub ground_unit_pitch: f32,
    pub turret_weapon_slots: u32,
    pub min_idle_scan_angle: f32,
    pub max_idle_scan_angle: f32,
    pub min_idle_scan_interval: u32,
    pub max_idle_scan_interval: u32,
    pub recenter_time: u32,
    pub initially_disabled: bool,
    pub fires_while_turning: bool,
    pub allows_pitch: bool,
    pub inter_turret_delay: u32,
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
}

/// AIUpdateModuleData (matches C++ AIUpdateModuleData defaults and fields).
#[derive(Debug, Clone)]
pub struct AIUpdateModuleData {
    module_tag_name_key: NameKeyType,
    auto_acquire_enemies_when_idle: u32,
    mood_attack_check_rate: u32,
    surrender_duration: u32,
    forbid_player_commands: bool,
    turrets_linked: bool,
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

    pub fn mood_attack_check_rate(&self) -> u32 {
        self.mood_attack_check_rate
    }

    pub fn set_mood_attack_check_rate(&mut self, value: u32) {
        self.mood_attack_check_rate = value;
    }

    pub fn surrender_duration_frames(&self) -> u32 {
        self.surrender_duration
    }

    pub fn set_surrender_duration_frames(&mut self, value: u32) {
        self.surrender_duration = value;
    }

    pub fn forbid_player_commands(&self) -> bool {
        self.forbid_player_commands
    }

    pub fn set_forbid_player_commands(&mut self, value: bool) {
        self.forbid_player_commands = value;
    }

    pub fn turrets_linked(&self) -> bool {
        self.turrets_linked
    }

    pub fn set_turrets_linked(&mut self, value: bool) {
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

    fn ai_update_data(&self) -> Option<&AIUpdateModuleData> {
        Some(self)
    }

    fn ai_update_data_mut(&mut self) -> Option<&mut AIUpdateModuleData> {
        Some(self)
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

fn parse_duration_field(setter: &mut dyn FnMut(u32), tokens: &[&str]) -> Result<(), INIError> {
    let Some(token) = tokens.first().copied() else {
        return Err(INIError::InvalidData);
    };
    setter(INI::parse_duration_unsigned_int(token)?);
    Ok(())
}

fn parse_bool_field(setter: &mut dyn FnMut(bool), tokens: &[&str]) -> Result<(), INIError> {
    let Some(token) = tokens.first().copied() else {
        return Err(INIError::InvalidData);
    };
    setter(INI::parse_bool(token)?);
    Ok(())
}

pub const AI_UPDATE_FIELDS: &[FieldParse<AIUpdateModuleData>] = &[
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

    let set = LocomotorSetType::from_ini_name(tokens[0]).ok_or(INIError::InvalidData)?;

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
