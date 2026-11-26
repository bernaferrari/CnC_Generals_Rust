//! Authored delivery settings from DeliverPayloadAIUpdate.cpp.

use super::ai_update_data::{AI_UPDATE_FIELDS, AIUpdateModuleData};
use super::module::{ModuleData, NameKeyType};
use super::radius_decal_data::{RADIUS_DECAL_TEMPLATE_FIELDS, RadiusDecalTemplate};
use crate::common::ascii_string::AsciiString;
use crate::common::ini::{FieldParse, INI, INIError};
use crate::common::system::{Snapshotable, Xfer};
use glam::Vec3;
use std::any::Any;

/// DeliverPayloadAIUpdate module data (INI-driven).
#[derive(Debug, Clone)]
pub struct DeliverPayloadAIUpdateModuleData {
    module_tag_name_key: NameKeyType,
    pub base: AIUpdateModuleData,
    pub door_delay: u32,
    pub max_distance_to_target: f32,
    pub max_number_attempts: i32,
    pub drop_delay: u32,
    pub drop_offset: Vec3,
    pub drop_variance: Vec3,
    pub put_in_container_name: AsciiString,
    pub delivery_decal_template: RadiusDecalTemplate,
    pub delivery_decal_radius: f32,
}

impl Default for DeliverPayloadAIUpdateModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            base: AIUpdateModuleData::default(),
            door_delay: 0,
            max_distance_to_target: 0.0,
            max_number_attempts: 0,
            drop_delay: 0,
            drop_offset: Vec3::ZERO,
            drop_variance: Vec3::ZERO,
            put_in_container_name: AsciiString::new(),
            delivery_decal_template: RadiusDecalTemplate::default(),
            delivery_decal_radius: 0.0,
        }
    }
}

impl DeliverPayloadAIUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_inherited_fields(
            self,
            |data| &mut data.base,
            AI_UPDATE_FIELDS,
            DELIVER_PAYLOAD_AI_UPDATE_FIELDS,
        )
    }
}

impl ModuleData for DeliverPayloadAIUpdateModuleData {
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

impl Snapshotable for DeliverPayloadAIUpdateModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let xfer_io = |r: std::io::Result<()>| r.map_err(|e| e.to_string());
        self.base.xfer(xfer)?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.door_delay))?;
        xfer_io(xfer.xfer_real(&mut self.max_distance_to_target))?;
        xfer_io(xfer.xfer_int(&mut self.max_number_attempts))?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.drop_delay))?;
        for vector in [&mut self.drop_offset, &mut self.drop_variance] {
            let _ = xfer.xfer_real(&mut vector.x);
            let _ = xfer.xfer_real(&mut vector.y);
            let _ = xfer.xfer_real(&mut vector.z);
        }
        xfer_io(xfer.xfer_ascii_string(self.put_in_container_name.as_mut_string_buffer()))?;
        xfer_io(xfer.xfer_real(&mut self.delivery_decal_radius))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn required_value<'a>(tokens: &'a [&'a str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

fn value_tokens<'a>(tokens: &'a [&'a str]) -> Vec<&'a str> {
    tokens
        .iter()
        .copied()
        .filter(|token| *token != "=")
        .collect()
}

fn parse_duration_field(setter: &mut dyn FnMut(u32), tokens: &[&str]) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_duration_unsigned_int(token)?);
    Ok(())
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

fn parse_int_field(setter: &mut dyn FnMut(i32), tokens: &[&str]) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_int(token)?);
    Ok(())
}

fn parse_coord3d_field(setter: &mut dyn FnMut(Vec3), tokens: &[&str]) -> Result<(), INIError> {
    let coord = parse_delivery_coordinate(tokens)?;
    setter(coord);
    Ok(())
}

fn parse_ascii_field(setter: &mut dyn FnMut(AsciiString), tokens: &[&str]) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(AsciiString::from(token));
    Ok(())
}

fn parse_auto_acquire_field(
    _ini: &mut INI,
    data: &mut DeliverPayloadAIUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    const AUTO_ACQUIRE_ENEMIES_NAMES: &[&str] = &[
        "YES",
        "STEALTHED",
        "NO",
        "NOTWHILEATTACKING",
        "ATTACK_BUILDINGS",
    ];
    let values = value_tokens(tokens);
    let value = INI::parse_bit_string_32(&values, AUTO_ACQUIRE_ENEMIES_NAMES)?;
    data.base.set_auto_acquire_enemies_when_idle(value);
    Ok(())
}

fn parse_delivery_decal(
    ini: &mut INI,
    data: &mut DeliverPayloadAIUpdateModuleData,
    _tokens: &[&str],
) -> Result<(), INIError> {
    ini.init_from_ini_with_fields(
        &mut data.delivery_decal_template,
        RADIUS_DECAL_TEMPLATE_FIELDS,
    )
}

const DELIVER_PAYLOAD_AI_UPDATE_FIELDS: &[FieldParse<DeliverPayloadAIUpdateModuleData>] = &[
    FieldParse {
        token: "AutoAcquireEnemiesWhenIdle",
        parse: parse_auto_acquire_field,
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
        token: "DoorDelay",
        parse: |_, data, tokens| parse_duration_field(&mut |v| data.door_delay = v, tokens),
    },
    FieldParse {
        token: "PutInContainer",
        parse: |_, data, tokens| parse_ascii_field(&mut |v| data.put_in_container_name = v, tokens),
    },
    FieldParse {
        token: "DeliveryDistance",
        parse: |_, data, tokens| parse_real_field(&mut |v| data.max_distance_to_target = v, tokens),
    },
    FieldParse {
        token: "MaxAttempts",
        parse: |_, data, tokens| parse_int_field(&mut |v| data.max_number_attempts = v, tokens),
    },
    FieldParse {
        token: "DropDelay",
        parse: |_, data, tokens| parse_duration_field(&mut |v| data.drop_delay = v, tokens),
    },
    FieldParse {
        token: "DropOffset",
        parse: |_, data, tokens| parse_coord3d_field(&mut |v| data.drop_offset = v, tokens),
    },
    FieldParse {
        token: "DropVariance",
        parse: |_, data, tokens| parse_coord3d_field(&mut |v| data.drop_variance = v, tokens),
    },
    FieldParse {
        token: "DeliveryDecal",
        parse: parse_delivery_decal,
    },
    FieldParse {
        token: "DeliveryDecalRadius",
        parse: |_, data, tokens| parse_real_field(&mut |v| data.delivery_decal_radius = v, tokens),
    },
];

pub fn parse_delivery_coordinate(tokens: &[&str]) -> Result<Vec3, INIError> {
    let values: Vec<&str> = tokens
        .iter()
        .copied()
        .filter(|token| *token != "=")
        .collect();
    // INI.cpp:1079-1087 and getNextSubToken:1648-1654 consume X, Y, Z
    // in order, case-insensitively, then scan each real. Keep the existing
    // unlabeled Rust fixture adapter separate from that original grammar.
    if values.iter().any(|token| token.contains([':', '=']))
        || values
            .first()
            .is_some_and(|token| token.eq_ignore_ascii_case("X"))
    {
        let (x, y, z) = INI::parse_coord_3d_ordered(&values)?;
        return Ok(Vec3::new(x, y, z));
    }
    if values.len() >= 3 {
        let x = INI::parse_real(values[0])?;
        let y = INI::parse_real(values[1])?;
        let z = INI::parse_real(values[2])?;
        return Ok(Vec3::new(x, y, z));
    }

    let token = required_value(tokens)?;
    let parts: Vec<&str> = token
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|part| !part.is_empty())
        .collect();
    if parts.len() != 3 {
        return Err(INIError::InvalidData);
    }

    let x = INI::parse_real(parts[0])?;
    let y = INI::parse_real(parts[1])?;
    let z = INI::parse_real(parts[2])?;
    Ok(Vec3::new(x, y, z))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deliver_payload_ai_fields_accept_ini_equals_token() {
        let mut data = DeliverPayloadAIUpdateModuleData::default();

        parse_duration_field(&mut |value| data.door_delay = value, &["=", "1000"]).unwrap();
        parse_ascii_field(
            &mut |value| data.put_in_container_name = value,
            &["=", "AmericaVehicleChinook"],
        )
        .unwrap();
        parse_real_field(
            &mut |value| data.max_distance_to_target = value,
            &["=", "250.5"],
        )
        .unwrap();
        parse_int_field(&mut |value| data.max_number_attempts = value, &["=", "3"]).unwrap();
        parse_duration_field(&mut |value| data.drop_delay = value, &["=", "3000"]).unwrap();
        parse_coord3d_field(
            &mut |value| data.drop_offset = value,
            &["=", "1.0", "2.0", "3.0"],
        )
        .unwrap();
        parse_coord3d_field(
            &mut |value| data.drop_variance = value,
            &["=", "4.0,5.0,6.0"],
        )
        .unwrap();
        parse_real_field(
            &mut |value| data.delivery_decal_radius = value,
            &["=", "80.0"],
        )
        .unwrap();

        assert_eq!(data.door_delay, 30);
        assert_eq!(data.put_in_container_name.as_str(), "AmericaVehicleChinook");
        assert!((data.max_distance_to_target - 250.5).abs() < f32::EPSILON);
        assert_eq!(data.max_number_attempts, 3);
        assert_eq!(data.drop_delay, 90);
        assert_eq!(data.drop_offset, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(data.drop_variance, Vec3::new(4.0, 5.0, 6.0));
        assert!((data.delivery_decal_radius - 80.0).abs() < f32::EPSILON);
    }

    #[test]
    fn deliver_payload_ai_base_fields_accept_ini_equals_token() {
        let mut ini = INI::new();
        let mut data = DeliverPayloadAIUpdateModuleData::default();

        parse_auto_acquire_field(&mut ini, &mut data, &["=", "YES", "ATTACK_BUILDINGS"]).unwrap();
        parse_duration_field(
            &mut |value| data.base.set_mood_attack_check_rate(value),
            &["=", "2000"],
        )
        .unwrap();
        parse_bool_field(
            &mut |value| data.base.set_forbid_player_commands(value),
            &["=", "Yes"],
        )
        .unwrap();
        parse_bool_field(
            &mut |value| data.base.set_turrets_linked(value),
            &["=", "Yes"],
        )
        .unwrap();

        assert_eq!(data.base.auto_acquire_enemies_when_idle(), 0b10001);
        assert_eq!(data.base.mood_attack_check_rate(), 60);
        assert!(data.base.forbid_player_commands());
        assert!(data.base.turrets_linked());
    }
}
