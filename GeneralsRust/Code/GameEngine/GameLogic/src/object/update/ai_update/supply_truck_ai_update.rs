//! SupplyTruckAIUpdate module data + compatibility shim.
//!
//! Ported from GameLogic/Module/SupplyTruckAIUpdate.h.

pub use crate::ai::modules::SupplyTruckAIUpdate;

use std::any::Any;
use std::sync::Arc;

use crate::common::{AsciiString, Bool, Int, Real, UnsignedInt};
use crate::object::update::ai_update_interface::AIUpdateModuleData;
use game_engine::common::ini::{FieldParse, INIError, INILoadType, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

const AUTO_ACQUIRE_ENEMIES_NAMES: &[&str] = &[
    "YES",
    "STEALTHED",
    "NO",
    "NOTWHILEATTACKING",
    "ATTACK_BUILDINGS",
];

/// SupplyTruckAIUpdate module data (INI-driven).
#[derive(Debug, Clone)]
pub struct SupplyTruckAIUpdateModuleData {
    module_tag_name_key: NameKeyType,
    pub base: AIUpdateModuleData,
    pub max_boxes_data: Int,
    pub center_delay: UnsignedInt,
    pub warehouse_delay: UnsignedInt,
    pub warehouse_scan_distance: Real,
    pub supplies_depleted_voice: AsciiString,
}

impl Default for SupplyTruckAIUpdateModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            base: AIUpdateModuleData::default(),
            max_boxes_data: 0,
            center_delay: 0,
            warehouse_delay: 0,
            warehouse_scan_distance: 100.0,
            supplies_depleted_voice: AsciiString::new(),
        }
    }
}

impl SupplyTruckAIUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, SUPPLY_TRUCK_AI_UPDATE_FIELDS)
    }
}

impl ModuleData for SupplyTruckAIUpdateModuleData {
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

impl Snapshotable for SupplyTruckAIUpdateModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let xfer_io = |r: std::io::Result<()>| r.map_err(|e| e.to_string());
        self.base.xfer(xfer)?;
        xfer_io(xfer.xfer_int(&mut self.max_boxes_data))?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.center_delay))?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.warehouse_delay))?;
        xfer_io(xfer.xfer_real(&mut self.warehouse_scan_distance))?;
        xfer_io(xfer.xfer_ascii_string(self.supplies_depleted_voice.as_mut_string_buffer()))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn parse_auto_acquire_field(
    _ini: &mut INI,
    data: &mut SupplyTruckAIUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let values = value_tokens(tokens)?;
    let value = INI::parse_bit_string_32(&values, AUTO_ACQUIRE_ENEMIES_NAMES)?;
    data.base.set_auto_acquire_enemies_when_idle(value);
    Ok(())
}

fn required_value<'a>(tokens: &'a [&'a str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

fn value_tokens<'a>(tokens: &'a [&'a str]) -> Result<Vec<&'a str>, INIError> {
    let values: Vec<_> = tokens
        .iter()
        .copied()
        .filter(|token| *token != "=")
        .collect();
    if values.is_empty() {
        return Err(INIError::InvalidData);
    }
    Ok(values)
}

fn parse_duration_field(
    setter: &mut dyn FnMut(UnsignedInt),
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_duration_unsigned_int(token)?);
    Ok(())
}

fn parse_bool_field(setter: &mut dyn FnMut(Bool), tokens: &[&str]) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_bool(token)?);
    Ok(())
}

fn parse_real_field(setter: &mut dyn FnMut(Real), tokens: &[&str]) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_real(token)?);
    Ok(())
}

fn parse_int_field(setter: &mut dyn FnMut(Int), tokens: &[&str]) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_int(token)?);
    Ok(())
}

fn parse_locomotor_set_field(
    ini: &mut INI,
    data: &mut SupplyTruckAIUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let values = value_tokens(tokens)?;
    if values.len() < 2 {
        return Err(INIError::InvalidData);
    }

    let set = match values[0] {
        "SET_NORMAL" => crate::common::LocomotorSetType::Normal,
        "SET_NORMAL_UPGRADED" => crate::common::LocomotorSetType::NormalUpgraded,
        "SET_FREEFALL" => crate::common::LocomotorSetType::Freefall,
        "SET_WANDER" => crate::common::LocomotorSetType::Wander,
        "SET_PANIC" => crate::common::LocomotorSetType::Panic,
        "SET_TAXIING" => crate::common::LocomotorSetType::Taxiing,
        "SET_SUPERSONIC" => crate::common::LocomotorSetType::Supersonic,
        "SET_SLUGGISH" => crate::common::LocomotorSetType::Sluggish,
        _ => return Err(INIError::InvalidData),
    };

    if data.base.has_locomotor_set(set) && ini.get_load_type() != INILoadType::CreateOverrides {
        return Err(INIError::InvalidData);
    }

    let mut entries = Vec::new();
    for token in values.iter().skip(1) {
        if token.is_empty() || token.eq_ignore_ascii_case("None") {
            continue;
        }
        entries.push(AsciiString::from(*token));
    }
    if entries.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.base.set_locomotor_set_entries(set, entries);
    Ok(())
}

fn parse_audio_event(
    _ini: &mut INI,
    data: &mut SupplyTruckAIUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    data.supplies_depleted_voice = AsciiString::from(token);
    Ok(())
}

fn parse_turret_field(
    ini: &mut INI,
    data: &mut SupplyTruckAIUpdateModuleData,
    _tokens: &[&str],
) -> Result<(), INIError> {
    if data.base.turret_primary().is_some() {
        return Err(INIError::InvalidData);
    }
    let mut turret = crate::object::update::ai_update_interface::TurretAIData::default();
    turret.parse_from_ini(ini)?;
    data.base.set_turret_primary(turret);
    Ok(())
}

fn parse_alt_turret_field(
    ini: &mut INI,
    data: &mut SupplyTruckAIUpdateModuleData,
    _tokens: &[&str],
) -> Result<(), INIError> {
    if data.base.turret_secondary().is_some() {
        return Err(INIError::InvalidData);
    }
    let mut turret = crate::object::update::ai_update_interface::TurretAIData::default();
    turret.parse_from_ini(ini)?;
    data.base.set_turret_secondary(turret);
    Ok(())
}

const SUPPLY_TRUCK_AI_UPDATE_FIELDS: &[FieldParse<SupplyTruckAIUpdateModuleData>] = &[
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
            parse_duration_field(
                &mut |value| data.base.set_mood_attack_check_rate(value),
                tokens,
            )
        },
    },
    FieldParse {
        token: "SurrenderDuration",
        parse: |_, data, tokens| {
            parse_duration_field(
                &mut |value| data.base.set_surrender_duration_frames(value),
                tokens,
            )
        },
    },
    FieldParse {
        token: "ForbidPlayerCommands",
        parse: |_, data, tokens| {
            parse_bool_field(
                &mut |value| data.base.set_forbid_player_commands(value),
                tokens,
            )
        },
    },
    FieldParse {
        token: "TurretsLinked",
        parse: |_, data, tokens| {
            parse_bool_field(&mut |value| data.base.set_turrets_linked(value), tokens)
        },
    },
    FieldParse {
        token: "MaxBoxes",
        parse: |_, data, tokens| parse_int_field(&mut |value| data.max_boxes_data = value, tokens),
    },
    FieldParse {
        token: "SupplyCenterActionDelay",
        parse: |_, data, tokens| {
            parse_duration_field(&mut |value| data.center_delay = value, tokens)
        },
    },
    FieldParse {
        token: "SupplyWarehouseActionDelay",
        parse: |_, data, tokens| {
            parse_duration_field(&mut |value| data.warehouse_delay = value, tokens)
        },
    },
    FieldParse {
        token: "SupplyWarehouseScanDistance",
        parse: |_, data, tokens| {
            parse_real_field(&mut |value| data.warehouse_scan_distance = value, tokens)
        },
    },
    FieldParse {
        token: "SuppliesDepletedVoice",
        parse: parse_audio_event,
    },
];

/// Module wrapper for SupplyTruckAIUpdate to align with module system expectations.
#[derive(Debug)]
pub struct SupplyTruckAIUpdateModule {
    module_name_key: NameKeyType,
    data: Arc<SupplyTruckAIUpdateModuleData>,
}

impl SupplyTruckAIUpdateModule {
    pub fn new(module_name_key: NameKeyType, data: Arc<SupplyTruckAIUpdateModuleData>) -> Self {
        Self {
            module_name_key,
            data,
        }
    }
}

impl Module for SupplyTruckAIUpdateModule {
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

impl Snapshotable for SupplyTruckAIUpdateModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.data.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Arc::make_mut(&mut self.data).xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::LocomotorSetType;

    fn parse_field(data: &mut SupplyTruckAIUpdateModuleData, token: &str, values: &[&str]) {
        let field = SUPPLY_TRUCK_AI_UPDATE_FIELDS
            .iter()
            .find(|field| field.token == token)
            .expect("field exists");
        let mut ini = INI::new();
        (field.parse)(&mut ini, data, values).expect("field parses");
    }

    #[test]
    fn supply_truck_fields_accept_ini_equals_token() {
        let mut data = SupplyTruckAIUpdateModuleData::default();

        parse_field(
            &mut data,
            "AutoAcquireEnemiesWhenIdle",
            &["=", "YES", "ATTACK_BUILDINGS"],
        );
        parse_field(
            &mut data,
            "Locomotor",
            &["=", "SET_NORMAL", "SupplyTruckLocomotor"],
        );
        parse_field(&mut data, "MoodAttackCheckRate", &["=", "2000"]);
        parse_field(&mut data, "SurrenderDuration", &["=", "3000"]);
        parse_field(&mut data, "ForbidPlayerCommands", &["=", "Yes"]);
        parse_field(&mut data, "TurretsLinked", &["=", "Yes"]);
        parse_field(&mut data, "MaxBoxes", &["=", "5"]);
        parse_field(&mut data, "SupplyCenterActionDelay", &["=", "1200"]);
        parse_field(&mut data, "SupplyWarehouseActionDelay", &["=", "900"]);
        parse_field(&mut data, "SupplyWarehouseScanDistance", &["=", "275.5"]);
        parse_field(
            &mut data,
            "SuppliesDepletedVoice",
            &["=", "SupplyTruckEmpty"],
        );

        assert_ne!(data.base.auto_acquire_enemies_when_idle(), 0);
        assert!(data.base.has_locomotor_set(LocomotorSetType::Normal));
        assert_eq!(data.base.mood_attack_check_rate(), 60);
        assert_eq!(data.base.surrender_duration_frames(), 90);
        assert!(data.base.forbid_player_commands());
        assert!(data.base.turrets_linked());
        assert_eq!(data.max_boxes_data, 5);
        assert_eq!(data.center_delay, 36);
        assert_eq!(data.warehouse_delay, 27);
        assert_eq!(data.warehouse_scan_distance, 275.5);
        assert_eq!(data.supplies_depleted_voice.as_str(), "SupplyTruckEmpty");
    }
}
