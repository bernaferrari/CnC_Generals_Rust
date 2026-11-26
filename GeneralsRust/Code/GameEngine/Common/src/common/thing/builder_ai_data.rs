//! Authored construction and supply settings for DozerAIUpdate and WorkerAIUpdate.

use super::ai_update_data::{AI_UPDATE_FIELDS, AIUpdateModuleData};
use super::module::{ModuleData, NameKeyType};
use crate::common::ascii_string::AsciiString;
use crate::common::ini::{FieldParse, INI, INIError};
use crate::common::system::{Snapshotable, Xfer};
use std::any::Any;

/// Module data for DozerAIUpdate (INI-driven).
#[derive(Debug, Clone)]
pub struct DozerAIUpdateModuleData {
    module_tag_name_key: NameKeyType,
    pub base: AIUpdateModuleData,
    pub repair_health_percent_per_second: f32,
    pub bored_time: f32,
    pub bored_range: f32,
}

impl Default for DozerAIUpdateModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            base: AIUpdateModuleData::default(),
            repair_health_percent_per_second: 0.0,
            bored_time: 0.0,
            bored_range: 0.0,
        }
    }
}

impl DozerAIUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_inherited_fields(
            self,
            |data| &mut data.base,
            AI_UPDATE_FIELDS,
            DOZER_AI_UPDATE_FIELDS,
        )
    }
}

impl ModuleData for DozerAIUpdateModuleData {
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
        Some(&self.base)
    }

    fn ai_update_data_mut(&mut self) -> Option<&mut AIUpdateModuleData> {
        Some(&mut self.base)
    }
}

impl Snapshotable for DozerAIUpdateModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let xfer_io = |r: std::io::Result<()>| r.map_err(|e| e.to_string());
        self.base.xfer(xfer)?;
        xfer_io(xfer.xfer_real(&mut self.repair_health_percent_per_second))?;
        xfer_io(xfer.xfer_real(&mut self.bored_time))?;
        xfer_io(xfer.xfer_real(&mut self.bored_range))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

const DOZER_AI_UPDATE_FIELDS: &[FieldParse<DozerAIUpdateModuleData>] = &[
    FieldParse {
        token: "RepairHealthPercentPerSecond",
        parse: |_, data, tokens| {
            parse_percent_to_real_field(
                &mut |value| data.repair_health_percent_per_second = value,
                tokens,
            )
        },
    },
    FieldParse {
        token: "BoredTime",
        parse: |_, data, tokens| {
            parse_duration_real_field(&mut |value| data.bored_time = value, tokens)
        },
    },
    FieldParse {
        token: "BoredRange",
        parse: |_, data, tokens| parse_real_field(&mut |value| data.bored_range = value, tokens),
    },
];

/// WorkerAIUpdate module data (INI-driven).
#[derive(Debug, Clone)]
pub struct WorkerAIUpdateModuleData {
    module_tag_name_key: NameKeyType,
    pub base: AIUpdateModuleData,
    pub max_boxes_data: i32,
    pub repair_health_percent_per_second: f32,
    pub bored_time: f32,
    pub bored_range: f32,
    pub center_delay: u32,
    pub warehouse_delay: u32,
    pub warehouse_scan_distance: f32,
    pub supplies_depleted_voice: AsciiString,
    pub upgraded_supply_boost: i32,
}

impl Default for WorkerAIUpdateModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            base: AIUpdateModuleData::default(),
            max_boxes_data: 0,
            repair_health_percent_per_second: 0.0,
            bored_time: 0.0,
            bored_range: 0.0,
            center_delay: 0,
            warehouse_delay: 0,
            warehouse_scan_distance: 100.0,
            supplies_depleted_voice: AsciiString::new(),
            upgraded_supply_boost: 0,
        }
    }
}

impl WorkerAIUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_inherited_fields(
            self,
            |data| &mut data.base,
            AI_UPDATE_FIELDS,
            WORKER_AI_UPDATE_FIELDS,
        )
    }
}

impl ModuleData for WorkerAIUpdateModuleData {
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
        Some(&self.base)
    }

    fn ai_update_data_mut(&mut self) -> Option<&mut AIUpdateModuleData> {
        Some(&mut self.base)
    }
}

impl Snapshotable for WorkerAIUpdateModuleData {
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
        xfer_io(xfer.xfer_real(&mut self.repair_health_percent_per_second))?;
        xfer_io(xfer.xfer_real(&mut self.bored_time))?;
        xfer_io(xfer.xfer_real(&mut self.bored_range))?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.center_delay))?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.warehouse_delay))?;
        xfer_io(xfer.xfer_real(&mut self.warehouse_scan_distance))?;
        xfer_io(xfer.xfer_ascii_string(self.supplies_depleted_voice.as_mut_string_buffer()))?;
        xfer_io(xfer.xfer_int(&mut self.upgraded_supply_boost))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

const WORKER_AI_UPDATE_FIELDS: &[FieldParse<WorkerAIUpdateModuleData>] = &[
    FieldParse {
        token: "MaxBoxes",
        parse: |_, data, tokens| parse_int_field(&mut |value| data.max_boxes_data = value, tokens),
    },
    FieldParse {
        token: "RepairHealthPercentPerSecond",
        parse: |_, data, tokens| {
            parse_percent_to_real_field(
                &mut |value| data.repair_health_percent_per_second = value,
                tokens,
            )
        },
    },
    FieldParse {
        token: "BoredTime",
        parse: |_, data, tokens| {
            parse_duration_real_field(&mut |value| data.bored_time = value, tokens)
        },
    },
    FieldParse {
        token: "BoredRange",
        parse: |_, data, tokens| parse_real_field(&mut |value| data.bored_range = value, tokens),
    },
    FieldParse {
        token: "SupplyCenterActionDelay",
        parse: |_, data, tokens| {
            parse_duration_unsigned_field(&mut |value| data.center_delay = value, tokens)
        },
    },
    FieldParse {
        token: "SupplyWarehouseActionDelay",
        parse: |_, data, tokens| {
            parse_duration_unsigned_field(&mut |value| data.warehouse_delay = value, tokens)
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
    FieldParse {
        token: "UpgradedSupplyBoost",
        parse: |_, data, tokens| {
            parse_int_field(&mut |value| data.upgraded_supply_boost = value, tokens)
        },
    },
];

fn required_value<'a>(tokens: &'a [&'a str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

fn parse_duration_unsigned_field(
    setter: &mut dyn FnMut(u32),
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_duration_unsigned_int(token)?);
    Ok(())
}

fn parse_duration_real_field(setter: &mut dyn FnMut(f32), tokens: &[&str]) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_duration_real(token)?);
    Ok(())
}

fn parse_real_field(setter: &mut dyn FnMut(f32), tokens: &[&str]) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_real(token)?);
    Ok(())
}

fn parse_int_field(setter: &mut dyn FnMut(i32), tokens: &[&str]) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_int(token)?);
    Ok(())
}

fn parse_percent_to_real_field(
    setter: &mut dyn FnMut(f32),
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_percent_to_real(token)?);
    Ok(())
}

fn parse_audio_event(
    _ini: &mut INI,
    data: &mut WorkerAIUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    data.supplies_depleted_voice = AsciiString::from(token);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::ai_update_data::LocomotorSetType;
    use super::*;

    #[test]
    fn dozer_fields_accept_ini_equals_token() {
        let mut data = DozerAIUpdateModuleData::default();

        INI::new()
            .with_inline_source(
                "AutoAcquireEnemiesWhenIdle = YES ATTACK_BUILDINGS
Locomotor = SET_NORMAL DozerLocomotor
MoodAttackCheckRate = 2000
SurrenderDuration = 3000
ForbidPlayerCommands = Yes
TurretsLinked = Yes
RepairHealthPercentPerSecond = 25%
BoredTime = 1500
BoredRange = 125.5
End",
                |ini| data.parse_from_ini(ini),
            )
            .expect("builder fields parse");

        assert_ne!(data.base.auto_acquire_enemies_when_idle(), 0);
        assert!(data.base.has_locomotor_set(LocomotorSetType::Normal));
        assert_eq!(data.base.mood_attack_check_rate(), 60);
        assert_eq!(data.base.surrender_duration_frames(), 90);
        assert!(data.base.forbid_player_commands());
        assert!(data.base.turrets_linked());
        assert_eq!(data.repair_health_percent_per_second, 0.25);
        assert_eq!(data.bored_time, 45.0);
        assert_eq!(data.bored_range, 125.5);
    }

    #[test]
    fn worker_fields_accept_ini_equals_token() {
        let mut data = WorkerAIUpdateModuleData::default();

        INI::new()
            .with_inline_source(
                "AutoAcquireEnemiesWhenIdle = YES ATTACK_BUILDINGS
Locomotor = SET_NORMAL WorkerLocomotor
MoodAttackCheckRate = 2000
SurrenderDuration = 3000
ForbidPlayerCommands = Yes
TurretsLinked = Yes
MaxBoxes = 5
RepairHealthPercentPerSecond = 25%
BoredTime = 1500
BoredRange = 125.5
SupplyCenterActionDelay = 1200
SupplyWarehouseActionDelay = 900
SupplyWarehouseScanDistance = 275.5
SuppliesDepletedVoice = WorkerSupplyEmpty
UpgradedSupplyBoost = 3
End",
                |ini| data.parse_from_ini(ini),
            )
            .expect("builder fields parse");

        assert_ne!(data.base.auto_acquire_enemies_when_idle(), 0);
        assert!(data.base.has_locomotor_set(LocomotorSetType::Normal));
        assert_eq!(data.base.mood_attack_check_rate(), 60);
        assert_eq!(data.base.surrender_duration_frames(), 90);
        assert!(data.base.forbid_player_commands());
        assert!(data.base.turrets_linked());
        assert_eq!(data.max_boxes_data, 5);
        assert_eq!(data.repair_health_percent_per_second, 0.25);
        assert_eq!(data.bored_time, 45.0);
        assert_eq!(data.bored_range, 125.5);
        assert_eq!(data.center_delay, 36);
        assert_eq!(data.warehouse_delay, 27);
        assert_eq!(data.warehouse_scan_distance, 275.5);
        assert_eq!(data.supplies_depleted_voice.as_str(), "WorkerSupplyEmpty");
        assert_eq!(data.upgraded_supply_boost, 3);
    }

    #[test]
    fn parse_duration_real_field_accepts_duration_suffixes() {
        let mut parsed = 0.0;
        parse_duration_real_field(&mut |value| parsed = value, &["1500ms"]).expect("duration");
        assert!((parsed - 45.0).abs() < f32::EPSILON);

        parse_duration_real_field(&mut |value| parsed = value, &["1.5s"]).expect("duration");
        assert!((parsed - 45.0).abs() < f32::EPSILON);
    }
}
