//! Authored Chinook definitions, defaults, INI parsing and runtime data conversion.

use super::AUTO_ACQUIRE_ENEMIES_NAMES;
use crate::common::{AsciiString, Bool, Int, LOGICFRAMES_PER_SECOND, Real, UnsignedInt};
use crate::object::draw::draw_module::RGBColor;
use crate::object::update::ai_update_interface::AIUpdateModuleData;
use crate::supply_system::SupplyTruckAIUpdateData;
use game_engine::common::global_data;
use game_engine::common::ini::{FieldParse, INI, INIError, INILoadType};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{ModuleData, NameKeyType};
use std::any::Any;

/// Module data for ChinookAIUpdate (INI-driven).
#[derive(Debug, Clone)]
pub struct ChinookAIUpdateModuleData {
    module_tag_name_key: NameKeyType,
    pub base: AIUpdateModuleData,
    pub max_boxes_data: Int,
    pub center_delay: UnsignedInt,
    pub warehouse_delay: UnsignedInt,
    pub warehouse_scan_distance: Real,
    pub supplies_depleted_voice: AsciiString,
    pub rope_name: AsciiString,
    pub rotor_wash_particle_system: AsciiString,
    pub rappel_speed: Real,
    pub rope_drop_speed: Real,
    pub rope_width: Real,
    pub rope_final_height: Real,
    pub rope_wobble_len: Real,
    pub rope_wobble_amp: Real,
    pub rope_wobble_rate: Real,
    pub rope_color: RGBColor,
    pub num_ropes: UnsignedInt,
    pub per_rope_delay_min: UnsignedInt,
    pub per_rope_delay_max: UnsignedInt,
    pub min_drop_height: Real,
    pub wait_for_ropes_to_drop: Bool,
    pub upgraded_supply_boost: Int,
}

impl Default for ChinookAIUpdateModuleData {
    fn default() -> Self {
        let gravity = global_data::read_safe()
            .map(|data| data.gravity.abs())
            .unwrap_or(9.81);
        let rappel_speed = gravity * LOGICFRAMES_PER_SECOND as f32 * 0.5;
        Self {
            module_tag_name_key: 0,
            base: AIUpdateModuleData::default(),
            max_boxes_data: 0,
            center_delay: 0,
            warehouse_delay: 0,
            warehouse_scan_distance: 100.0,
            supplies_depleted_voice: AsciiString::new(),
            rope_name: AsciiString::from("GenericRope"),
            rotor_wash_particle_system: AsciiString::new(),
            rappel_speed,
            rope_drop_speed: 1.0e10,
            rope_width: 0.5,
            rope_final_height: 0.0,
            rope_wobble_len: 10.0,
            rope_wobble_amp: 1.0,
            rope_wobble_rate: 0.1,
            rope_color: RGBColor::new(229, 204, 178),
            num_ropes: 4,
            per_rope_delay_min: 0x7fffffff,
            per_rope_delay_max: 0x7fffffff,
            min_drop_height: 30.0,
            wait_for_ropes_to_drop: true,
            upgraded_supply_boost: 0,
        }
    }
}

impl ChinookAIUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, CHINOOK_AI_UPDATE_FIELDS)
    }
}

impl ModuleData for ChinookAIUpdateModuleData {
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

impl Snapshotable for ChinookAIUpdateModuleData {
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
        xfer_io(xfer.xfer_ascii_string(self.rope_name.as_mut_string_buffer()))?;
        xfer_io(xfer.xfer_ascii_string(self.rotor_wash_particle_system.as_mut_string_buffer()))?;
        xfer_io(xfer.xfer_real(&mut self.rappel_speed))?;
        xfer_io(xfer.xfer_real(&mut self.rope_drop_speed))?;
        xfer_io(xfer.xfer_real(&mut self.rope_width))?;
        xfer_io(xfer.xfer_real(&mut self.rope_final_height))?;
        xfer_io(xfer.xfer_real(&mut self.rope_wobble_len))?;
        xfer_io(xfer.xfer_real(&mut self.rope_wobble_amp))?;
        xfer_io(xfer.xfer_real(&mut self.rope_wobble_rate))?;
        xfer_io(xfer.xfer_unsigned_byte(&mut self.rope_color.r))?;
        xfer_io(xfer.xfer_unsigned_byte(&mut self.rope_color.g))?;
        xfer_io(xfer.xfer_unsigned_byte(&mut self.rope_color.b))?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.num_ropes))?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.per_rope_delay_min))?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.per_rope_delay_max))?;
        xfer_io(xfer.xfer_real(&mut self.min_drop_height))?;
        xfer_io(xfer.xfer_bool(&mut self.wait_for_ropes_to_drop))?;
        xfer_io(xfer.xfer_int(&mut self.upgraded_supply_boost))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn parse_auto_acquire_field(
    _ini: &mut INI,
    data: &mut ChinookAIUpdateModuleData,
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

fn parse_duration_unsigned_field(
    setter: &mut dyn FnMut(UnsignedInt),
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_duration_unsigned_int(token)?);
    Ok(())
}

fn parse_unsigned_field(
    setter: &mut dyn FnMut(UnsignedInt),
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_unsigned_int(token)?);
    Ok(())
}

#[allow(dead_code)]
fn parse_duration_real_field(
    setter: &mut dyn FnMut(Real),
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_duration_real(token)?);
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

fn parse_velocity_real_field(
    setter: &mut dyn FnMut(Real),
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_real(token)?);
    Ok(())
}

fn parse_angular_velocity_real_field(
    setter: &mut dyn FnMut(Real),
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_angular_velocity_real(token)?);
    Ok(())
}

fn parse_int_field(setter: &mut dyn FnMut(Int), tokens: &[&str]) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_int(token)?);
    Ok(())
}

fn parse_ascii_string_field(
    setter: &mut dyn FnMut(AsciiString),
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(AsciiString::from(token));
    Ok(())
}

fn parse_rgb_color_field(
    setter: &mut dyn FnMut(RGBColor),
    tokens: &[&str],
) -> Result<(), INIError> {
    let values = value_tokens(tokens)?;
    let (r, g, b) = INI::parse_rgb_color(&values)?;
    let to_u8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    setter(RGBColor::new(to_u8(r), to_u8(g), to_u8(b)));
    Ok(())
}

fn parse_locomotor_set_field(
    ini: &mut INI,
    data: &mut ChinookAIUpdateModuleData,
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

fn parse_turret_field(
    ini: &mut INI,
    data: &mut ChinookAIUpdateModuleData,
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
    data: &mut ChinookAIUpdateModuleData,
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

pub(super) const CHINOOK_AI_UPDATE_FIELDS: &[FieldParse<ChinookAIUpdateModuleData>] = &[
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
            parse_duration_unsigned_field(
                &mut |value| data.base.set_mood_attack_check_rate(value),
                tokens,
            )
        },
    },
    FieldParse {
        token: "SurrenderDuration",
        parse: |_, data, tokens| {
            parse_duration_unsigned_field(
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
        parse: |_, data, tokens| {
            parse_ascii_string_field(&mut |value| data.supplies_depleted_voice = value, tokens)
        },
    },
    FieldParse {
        token: "RappelSpeed",
        parse: |_, data, tokens| {
            parse_velocity_real_field(&mut |value| data.rappel_speed = value, tokens)
        },
    },
    FieldParse {
        token: "RopeDropSpeed",
        parse: |_, data, tokens| {
            parse_velocity_real_field(&mut |value| data.rope_drop_speed = value, tokens)
        },
    },
    FieldParse {
        token: "RopeName",
        parse: |_, data, tokens| {
            parse_ascii_string_field(&mut |value| data.rope_name = value, tokens)
        },
    },
    FieldParse {
        token: "RopeFinalHeight",
        parse: |_, data, tokens| {
            parse_real_field(&mut |value| data.rope_final_height = value, tokens)
        },
    },
    FieldParse {
        token: "RopeWidth",
        parse: |_, data, tokens| parse_real_field(&mut |value| data.rope_width = value, tokens),
    },
    FieldParse {
        token: "RopeWobbleLen",
        parse: |_, data, tokens| {
            parse_real_field(&mut |value| data.rope_wobble_len = value, tokens)
        },
    },
    FieldParse {
        token: "RopeWobbleAmplitude",
        parse: |_, data, tokens| {
            parse_real_field(&mut |value| data.rope_wobble_amp = value, tokens)
        },
    },
    FieldParse {
        token: "RopeWobbleRate",
        parse: |_, data, tokens| {
            parse_angular_velocity_real_field(&mut |value| data.rope_wobble_rate = value, tokens)
        },
    },
    FieldParse {
        token: "RopeColor",
        parse: |_, data, tokens| {
            parse_rgb_color_field(&mut |value| data.rope_color = value, tokens)
        },
    },
    FieldParse {
        token: "NumRopes",
        parse: |_, data, tokens| parse_unsigned_field(&mut |value| data.num_ropes = value, tokens),
    },
    FieldParse {
        token: "PerRopeDelayMin",
        parse: |_, data, tokens| {
            parse_duration_unsigned_field(&mut |value| data.per_rope_delay_min = value, tokens)
        },
    },
    FieldParse {
        token: "PerRopeDelayMax",
        parse: |_, data, tokens| {
            parse_duration_unsigned_field(&mut |value| data.per_rope_delay_max = value, tokens)
        },
    },
    FieldParse {
        token: "MinDropHeight",
        parse: |_, data, tokens| {
            parse_real_field(&mut |value| data.min_drop_height = value, tokens)
        },
    },
    FieldParse {
        token: "WaitForRopesToDrop",
        parse: |_, data, tokens| {
            parse_bool_field(&mut |value| data.wait_for_ropes_to_drop = value, tokens)
        },
    },
    FieldParse {
        token: "RotorWashParticleSystem",
        parse: |_, data, tokens| {
            parse_ascii_string_field(&mut |value| data.rotor_wash_particle_system = value, tokens)
        },
    },
    FieldParse {
        token: "UpgradedSupplyBoost",
        parse: |_, data, tokens| {
            parse_int_field(&mut |value| data.upgraded_supply_boost = value, tokens)
        },
    },
];

/// Runtime data for Chinook AI.
#[derive(Debug, Clone)]
pub struct ChinookAIUpdateData {
    pub supply: SupplyTruckAIUpdateData,
    pub rope_name: AsciiString,
    pub rotor_wash_particle_system: AsciiString,
    pub rappel_speed: Real,
    pub rope_drop_speed: Real,
    pub rope_width: Real,
    pub rope_final_height: Real,
    pub rope_wobble_len: Real,
    pub rope_wobble_amp: Real,
    pub rope_wobble_rate: Real,
    pub rope_color: RGBColor,
    pub num_ropes: UnsignedInt,
    pub per_rope_delay_min: UnsignedInt,
    pub per_rope_delay_max: UnsignedInt,
    pub min_drop_height: Real,
    pub wait_for_ropes_to_drop: Bool,
    pub upgraded_supply_boost: Int,
}

impl Default for ChinookAIUpdateData {
    fn default() -> Self {
        let module = ChinookAIUpdateModuleData::default();
        Self::from_module(&module)
    }
}

impl ChinookAIUpdateData {
    pub fn from_module(data: &ChinookAIUpdateModuleData) -> Self {
        Self {
            supply: SupplyTruckAIUpdateData {
                max_boxes: data.max_boxes_data,
                warehouse_scan_distance: data.warehouse_scan_distance,
                warehouse_delay: data.warehouse_delay,
                center_delay: data.center_delay,
                supplies_depleted_voice: data.supplies_depleted_voice.to_string(),
            },
            rope_name: data.rope_name.clone(),
            rotor_wash_particle_system: data.rotor_wash_particle_system.clone(),
            rappel_speed: data.rappel_speed,
            rope_drop_speed: data.rope_drop_speed,
            rope_width: data.rope_width,
            rope_final_height: data.rope_final_height,
            rope_wobble_len: data.rope_wobble_len,
            rope_wobble_amp: data.rope_wobble_amp,
            rope_wobble_rate: data.rope_wobble_rate,
            rope_color: data.rope_color,
            num_ropes: data.num_ropes,
            per_rope_delay_min: data.per_rope_delay_min,
            per_rope_delay_max: data.per_rope_delay_max,
            min_drop_height: data.min_drop_height,
            wait_for_ropes_to_drop: data.wait_for_ropes_to_drop,
            upgraded_supply_boost: data.upgraded_supply_boost,
        }
    }
}
