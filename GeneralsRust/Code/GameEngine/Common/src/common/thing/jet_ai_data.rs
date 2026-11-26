//! Authored jet movement, runway, and lock-on settings from JetAIUpdate.cpp.

use super::ai_update_data::{AI_UPDATE_FIELDS, AIUpdateModuleData, LocomotorSetType};
use super::module::{ModuleData, NameKeyType};
use crate::common::ascii_string::AsciiString;
use crate::common::ini::{FieldParse, INI, INIError};
use crate::common::system::{Snapshotable, Xfer};
use std::any::Any;

const LOCOMOTOR_SET_NAMES: &[&str] = &[
    "SET_NORMAL",
    "SET_NORMAL_UPGRADED",
    "SET_FREEFALL",
    "SET_WANDER",
    "SET_PANIC",
    "SET_TAXIING",
    "SET_SUPERSONIC",
    "SET_SLUGGISH",
];

/// Module data for JetAIUpdate (INI-driven).
#[derive(Debug, Clone)]
pub struct JetAIUpdateModuleData {
    module_tag_name_key: NameKeyType,
    pub base: AIUpdateModuleData,
    pub out_of_ammo_damage_per_second: f32,
    pub needs_runway: bool,
    pub keeps_parking_space_when_airborne: bool,
    pub takeoff_dist_for_max_lift: f32,
    pub takeoff_pause: u32,
    pub min_height: f32,
    pub parking_offset: f32,
    pub sneaky_offset_when_attacking: f32,
    pub attacking_loco: LocomotorSetType,
    pub attack_loco_persist_time: u32,
    pub attackers_miss_persist_time: u32,
    pub returning_loco: LocomotorSetType,
    pub lockon_time: u32,
    pub lockon_cursor: AsciiString,
    pub lockon_initial_dist: f32,
    pub lockon_freq: f32,
    pub lockon_angle_spin: f32,
    pub lockon_blinky: bool,
    pub return_to_base_idle_time: u32,
}

impl Default for JetAIUpdateModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            base: AIUpdateModuleData::default(),
            out_of_ammo_damage_per_second: 0.0,
            needs_runway: true,
            keeps_parking_space_when_airborne: true,
            takeoff_dist_for_max_lift: 0.0,
            takeoff_pause: 0,
            min_height: 0.0,
            parking_offset: 0.0,
            sneaky_offset_when_attacking: 0.0,
            attacking_loco: LocomotorSetType::Normal,
            attack_loco_persist_time: 0,
            attackers_miss_persist_time: 0,
            returning_loco: LocomotorSetType::Normal,
            lockon_time: 0,
            lockon_cursor: AsciiString::new(),
            lockon_initial_dist: 100.0,
            lockon_freq: 0.5,
            lockon_angle_spin: 720.0,
            lockon_blinky: false,
            return_to_base_idle_time: 0,
        }
    }
}

impl JetAIUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_inherited_fields(
            self,
            |data| &mut data.base,
            AI_UPDATE_FIELDS,
            JET_AI_UPDATE_FIELDS,
        )
    }
}

impl ModuleData for JetAIUpdateModuleData {
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

impl Snapshotable for JetAIUpdateModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let xfer_io = |r: std::io::Result<()>| r.map_err(|e| e.to_string());
        self.base.xfer(xfer)?;
        xfer_io(xfer.xfer_real(&mut self.out_of_ammo_damage_per_second))?;
        xfer_io(xfer.xfer_bool(&mut self.needs_runway))?;
        xfer_io(xfer.xfer_bool(&mut self.keeps_parking_space_when_airborne))?;
        xfer_io(xfer.xfer_real(&mut self.takeoff_dist_for_max_lift))?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.takeoff_pause))?;
        xfer_io(xfer.xfer_real(&mut self.min_height))?;
        xfer_io(xfer.xfer_real(&mut self.parking_offset))?;
        xfer_io(xfer.xfer_real(&mut self.sneaky_offset_when_attacking))?;
        let mut attacking_loco = self.attacking_loco as i32;
        xfer_io(xfer.xfer_int(&mut attacking_loco))?;
        self.attacking_loco = locomotor_set_from_xfer(attacking_loco)?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.attack_loco_persist_time))?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.attackers_miss_persist_time))?;
        let mut returning_loco = self.returning_loco as i32;
        xfer_io(xfer.xfer_int(&mut returning_loco))?;
        self.returning_loco = locomotor_set_from_xfer(returning_loco)?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.lockon_time))?;
        xfer_io(xfer.xfer_ascii_string(self.lockon_cursor.as_mut_string_buffer()))?;
        xfer_io(xfer.xfer_real(&mut self.lockon_initial_dist))?;
        xfer_io(xfer.xfer_real(&mut self.lockon_freq))?;
        xfer_io(xfer.xfer_real(&mut self.lockon_angle_spin))?;
        xfer_io(xfer.xfer_bool(&mut self.lockon_blinky))?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.return_to_base_idle_time))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn parse_bool_field(setter: &mut dyn FnMut(bool), tokens: &[&str]) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_bool(token)?);
    Ok(())
}

fn parse_real_field(setter: &mut dyn FnMut(f32), tokens: &[&str]) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_real(token)?);
    Ok(())
}

fn parse_percent_field(setter: &mut dyn FnMut(f32), tokens: &[&str]) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_percent_to_real(token)?);
    Ok(())
}

fn parse_duration_field(setter: &mut dyn FnMut(u32), tokens: &[&str]) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_duration_unsigned_int(token)?);
    Ok(())
}

fn parse_locomotor_set(
    setter: &mut dyn FnMut(LocomotorSetType),
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    let index =
        INI::parse_index_list(token, LOCOMOTOR_SET_NAMES).map_err(|_| INIError::InvalidData)?;
    let set = match index {
        0 => LocomotorSetType::Normal,
        1 => LocomotorSetType::NormalUpgraded,
        2 => LocomotorSetType::Freefall,
        3 => LocomotorSetType::Wander,
        4 => LocomotorSetType::Panic,
        5 => LocomotorSetType::Taxiing,
        6 => LocomotorSetType::Supersonic,
        7 => LocomotorSetType::Sluggish,
        _ => return Err(INIError::InvalidData),
    };
    setter(set);
    Ok(())
}

fn locomotor_set_from_xfer(value: i32) -> Result<LocomotorSetType, String> {
    match value {
        0 => Ok(LocomotorSetType::Normal),
        1 => Ok(LocomotorSetType::NormalUpgraded),
        2 => Ok(LocomotorSetType::Freefall),
        3 => Ok(LocomotorSetType::Wander),
        4 => Ok(LocomotorSetType::Panic),
        5 => Ok(LocomotorSetType::Taxiing),
        6 => Ok(LocomotorSetType::Supersonic),
        7 => Ok(LocomotorSetType::Sluggish),
        _ => Err(format!("invalid JetAIUpdate locomotor set {value}")),
    }
}

fn parse_angle_field(setter: &mut dyn FnMut(f32), tokens: &[&str]) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_angle_real(token)?);
    Ok(())
}

fn required_value<'a>(tokens: &'a [&'a str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

const JET_AI_UPDATE_FIELDS: &[FieldParse<JetAIUpdateModuleData>] = &[
    FieldParse {
        token: "OutOfAmmoDamagePerSecond",
        parse: |_, data, tokens| {
            parse_percent_field(
                &mut |value| data.out_of_ammo_damage_per_second = value,
                tokens,
            )
        },
    },
    FieldParse {
        token: "NeedsRunway",
        parse: |_, data, tokens| parse_bool_field(&mut |value| data.needs_runway = value, tokens),
    },
    FieldParse {
        token: "KeepsParkingSpaceWhenAirborne",
        parse: |_, data, tokens| {
            parse_bool_field(
                &mut |value| data.keeps_parking_space_when_airborne = value,
                tokens,
            )
        },
    },
    FieldParse {
        token: "TakeoffDistForMaxLift",
        parse: |_, data, tokens| {
            parse_percent_field(&mut |value| data.takeoff_dist_for_max_lift = value, tokens)
        },
    },
    FieldParse {
        token: "TakeoffPause",
        parse: |_, data, tokens| {
            parse_duration_field(&mut |value| data.takeoff_pause = value, tokens)
        },
    },
    FieldParse {
        token: "MinHeight",
        parse: |_, data, tokens| parse_real_field(&mut |value| data.min_height = value, tokens),
    },
    FieldParse {
        token: "ParkingOffset",
        parse: |_, data, tokens| parse_real_field(&mut |value| data.parking_offset = value, tokens),
    },
    FieldParse {
        token: "SneakyOffsetWhenAttacking",
        parse: |_, data, tokens| {
            parse_real_field(
                &mut |value| data.sneaky_offset_when_attacking = value,
                tokens,
            )
        },
    },
    FieldParse {
        token: "AttackLocomotorType",
        parse: |_, data, tokens| {
            parse_locomotor_set(&mut |value| data.attacking_loco = value, tokens)
        },
    },
    FieldParse {
        token: "AttackLocomotorPersistTime",
        parse: |_, data, tokens| {
            parse_duration_field(&mut |value| data.attack_loco_persist_time = value, tokens)
        },
    },
    FieldParse {
        token: "AttackersMissPersistTime",
        parse: |_, data, tokens| {
            parse_duration_field(
                &mut |value| data.attackers_miss_persist_time = value,
                tokens,
            )
        },
    },
    FieldParse {
        token: "ReturnForAmmoLocomotorType",
        parse: |_, data, tokens| {
            parse_locomotor_set(&mut |value| data.returning_loco = value, tokens)
        },
    },
    FieldParse {
        token: "LockonTime",
        parse: |_, data, tokens| {
            parse_duration_field(&mut |value| data.lockon_time = value, tokens)
        },
    },
    FieldParse {
        token: "LockonCursor",
        parse: |_, data, tokens| {
            let token = required_value(tokens)?;
            data.lockon_cursor = AsciiString::from(token);
            Ok(())
        },
    },
    FieldParse {
        token: "LockonInitialDist",
        parse: |_, data, tokens| {
            parse_real_field(&mut |value| data.lockon_initial_dist = value, tokens)
        },
    },
    FieldParse {
        token: "LockonFreq",
        parse: |_, data, tokens| parse_real_field(&mut |value| data.lockon_freq = value, tokens),
    },
    FieldParse {
        token: "LockonAngleSpin",
        parse: |_, data, tokens| {
            parse_angle_field(&mut |value| data.lockon_angle_spin = value, tokens)
        },
    },
    FieldParse {
        token: "LockonBlinky",
        parse: |_, data, tokens| parse_bool_field(&mut |value| data.lockon_blinky = value, tokens),
    },
    FieldParse {
        token: "ReturnToBaseIdleTime",
        parse: |_, data, tokens| {
            parse_duration_field(&mut |value| data.return_to_base_idle_time = value, tokens)
        },
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
    use std::io::Cursor;

    fn parse_field(data: &mut JetAIUpdateModuleData, token: &str, values: &[&str]) {
        let field = JET_AI_UPDATE_FIELDS
            .iter()
            .find(|field| field.token == token)
            .expect("field exists");
        let mut ini = INI::new();
        (field.parse)(&mut ini, data, values).expect("field parses");
    }

    #[test]
    fn jet_fields_accept_ini_equals_token() {
        let mut data = JetAIUpdateModuleData::default();

        parse_field(&mut data, "OutOfAmmoDamagePerSecond", &["=", "25%"]);
        parse_field(&mut data, "NeedsRunway", &["=", "No"]);
        parse_field(&mut data, "KeepsParkingSpaceWhenAirborne", &["=", "No"]);
        parse_field(&mut data, "TakeoffDistForMaxLift", &["=", "75%"]);
        parse_field(&mut data, "TakeoffPause", &["=", "1500"]);
        parse_field(&mut data, "MinHeight", &["=", "80.5"]);
        parse_field(&mut data, "ParkingOffset", &["=", "12.25"]);
        parse_field(&mut data, "SneakyOffsetWhenAttacking", &["=", "33.75"]);
        parse_field(&mut data, "AttackLocomotorType", &["=", "SET_SUPERSONIC"]);
        parse_field(&mut data, "AttackLocomotorPersistTime", &["=", "2400"]);
        parse_field(&mut data, "AttackersMissPersistTime", &["=", "900"]);
        parse_field(
            &mut data,
            "ReturnForAmmoLocomotorType",
            &["=", "SET_TAXIING"],
        );
        parse_field(&mut data, "LockonTime", &["=", "1200"]);
        parse_field(&mut data, "LockonCursor", &["=", "LaserGuidedMissile"]);
        parse_field(&mut data, "LockonInitialDist", &["=", "180.0"]);
        parse_field(&mut data, "LockonFreq", &["=", "0.25"]);
        parse_field(&mut data, "LockonAngleSpin", &["=", "360"]);
        parse_field(&mut data, "LockonBlinky", &["=", "Yes"]);
        parse_field(&mut data, "ReturnToBaseIdleTime", &["=", "3000"]);

        assert_eq!(data.out_of_ammo_damage_per_second, 0.25);
        assert!(!data.needs_runway);
        assert!(!data.keeps_parking_space_when_airborne);
        assert_eq!(data.takeoff_dist_for_max_lift, 0.75);
        assert_eq!(data.takeoff_pause, 45);
        assert_eq!(data.min_height, 80.5);
        assert_eq!(data.parking_offset, 12.25);
        assert_eq!(data.sneaky_offset_when_attacking, 33.75);
        assert_eq!(data.attacking_loco, LocomotorSetType::Supersonic);
        assert_eq!(data.attack_loco_persist_time, 72);
        assert_eq!(data.attackers_miss_persist_time, 27);
        assert_eq!(data.returning_loco, LocomotorSetType::Taxiing);
        assert_eq!(data.lockon_time, 36);
        assert_eq!(data.lockon_cursor.as_str(), "LaserGuidedMissile");
        assert_eq!(data.lockon_initial_dist, 180.0);
        assert_eq!(data.lockon_freq, 0.25);
        assert_eq!(
            data.lockon_angle_spin,
            INI::parse_angle_real("360").unwrap()
        );
        assert!(data.lockon_blinky);
        assert_eq!(data.return_to_base_idle_time, 90);
    }

    #[test]
    fn jet_xfer_restores_locomotor_sets() {
        let mut original = JetAIUpdateModuleData {
            attacking_loco: LocomotorSetType::Supersonic,
            returning_loco: LocomotorSetType::Taxiing,
            ..JetAIUpdateModuleData::default()
        };

        let mut bytes = Vec::new();
        {
            let cursor = Cursor::new(&mut bytes);
            let mut save = XferSave::new(cursor, 1);
            original.xfer(&mut save).unwrap();
        }

        let mut restored = JetAIUpdateModuleData::default();
        {
            let cursor = Cursor::new(bytes);
            let mut load = XferLoad::new(cursor, 1);
            restored.xfer(&mut load).unwrap();
        }

        assert_eq!(restored.attacking_loco, LocomotorSetType::Supersonic);
        assert_eq!(restored.returning_loco, LocomotorSetType::Taxiing);
    }
}
