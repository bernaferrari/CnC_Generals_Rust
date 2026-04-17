//! AIUpdateInterface module data + module wrapper for module system parity.
//!
//! This captures the AIUpdateModuleData fields that influence per-unit AI behavior in C++,
//! including surrender duration and auto-acquire flags.

use std::any::Any;
use std::sync::Arc;

use game_engine::common::ini::{FieldParse, INIError, INILoadType, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

use std::collections::HashMap;

use crate::ai::turret::TurretAI;
use crate::common::{
    AsciiString, Bool, LocomotorSetType, Real, UnsignedInt, LOGICFRAMES_PER_SECOND,
    WEAPONSLOT_COUNT,
};
use crate::weapon::WeaponSlotType;

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
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
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

/// Module wrapper for AIUpdateInterface to satisfy module creation parity.
#[derive(Debug)]
pub struct AIUpdateInterfaceModule {
    module_name_key: NameKeyType,
    data: Arc<AIUpdateModuleData>,
}

impl AIUpdateInterfaceModule {
    pub fn new(module_name_key: NameKeyType, data: Arc<AIUpdateModuleData>) -> Self {
        Self {
            module_name_key,
            data,
        }
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
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}
