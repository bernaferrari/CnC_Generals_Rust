//! DeliverPayloadData - data passed from ObjectCreationList to DeliverPayloadAIUpdate.
//!
//! Ported from GameLogic/Module/DeliverPayloadAIUpdate.h/.cpp.

use std::sync::Arc;

use crate::common::RadiusDecalTemplate;
use crate::common::{
    AsciiString, Bool, Coord3D, Int, Real, UnsignedInt, WeaponSlotType, SHADOW_NAMES,
};
use crate::effects::FXList;
use crate::helpers::TheFXListStore;
use crate::weapon::WeaponTemplate;
use game_engine::common::ini::{FieldParse, INIError, INI};

/// Delivery data passed to DeliverPayloadAIUpdate.
#[derive(Debug, Clone)]
pub struct DeliverPayloadData {
    pub visible_drop_bone_name: AsciiString,
    pub visible_sub_object_name: AsciiString,
    pub visible_payload_template_name: AsciiString,
    pub dist_to_target: Real,
    pub pre_open_distance: Real,
    pub max_attempts: Int,
    pub drop_offset: Coord3D,
    pub drop_variance: Coord3D,
    pub drop_delay: UnsignedInt,
    pub fire_weapon: Bool,
    pub self_destruct_object: Bool,
    pub visible_num_bones: Int,
    pub dive_start_distance: Real,
    pub dive_end_distance: Real,
    pub strafing_weapon_slot: Option<WeaponSlotType>,
    pub visible_items_dropped_per_interval: Int,
    pub inherit_transport_velocity: Bool,
    pub is_parachute_directly: Bool,
    pub exit_pitch_rate: Real,
    pub strafe_fx: Option<Arc<FXList>>,
    pub strafe_length: Real,
    pub visible_payload_weapon_template: Option<Arc<WeaponTemplate>>,
    pub delivery_decal_template: RadiusDecalTemplate,
    pub delivery_decal_radius: Real,
}

impl Default for DeliverPayloadData {
    fn default() -> Self {
        Self {
            visible_drop_bone_name: AsciiString::new(),
            visible_sub_object_name: AsciiString::new(),
            visible_payload_template_name: AsciiString::new(),
            dist_to_target: 0.0,
            pre_open_distance: 0.0,
            max_attempts: 1,
            drop_offset: Coord3D::ZERO,
            drop_variance: Coord3D::ZERO,
            drop_delay: 0,
            fire_weapon: false,
            self_destruct_object: false,
            visible_num_bones: 0,
            dive_start_distance: 0.0,
            dive_end_distance: 0.0,
            strafing_weapon_slot: None,
            visible_items_dropped_per_interval: 0,
            inherit_transport_velocity: false,
            is_parachute_directly: false,
            exit_pitch_rate: 0.0,
            strafe_fx: None,
            strafe_length: 0.0,
            visible_payload_weapon_template: None,
            delivery_decal_template: RadiusDecalTemplate::default(),
            delivery_decal_radius: 0.0,
        }
    }
}

fn parse_real(tokens: &[&str]) -> Result<Real, INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    INI::parse_real(token)
}

fn parse_int(tokens: &[&str]) -> Result<Int, INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    INI::parse_int(token)
}

fn parse_bool(tokens: &[&str]) -> Result<Bool, INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    INI::parse_bool(token)
}

fn parse_duration(tokens: &[&str]) -> Result<UnsignedInt, INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    INI::parse_duration_unsigned_int(token)
}

fn parse_ascii(tokens: &[&str]) -> Result<AsciiString, INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    Ok(AsciiString::from(*token))
}

fn parse_coord3d(tokens: &[&str]) -> Result<Coord3D, INIError> {
    if tokens.len() >= 3 {
        let x = INI::parse_real(tokens[0])?;
        let y = INI::parse_real(tokens[1])?;
        let z = INI::parse_real(tokens[2])?;
        return Ok(Coord3D::new(x, y, z));
    }

    let token = tokens.first().ok_or(INIError::InvalidData)?;
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
    Ok(Coord3D::new(x, y, z))
}

fn parse_weapon_slot(tokens: &[&str]) -> Result<Option<WeaponSlotType>, INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    let value = token.trim();
    if value.eq_ignore_ascii_case("NONE") || value == "-1" {
        return Ok(None);
    }

    let slot = match value.to_ascii_uppercase().as_str() {
        "PRIMARY" | "PRIMARY_WEAPON" => WeaponSlotType::Primary,
        "SECONDARY" | "SECONDARY_WEAPON" => WeaponSlotType::Secondary,
        "TERTIARY" | "TERTIARY_WEAPON" => WeaponSlotType::Tertiary,
        _ => return Err(INIError::InvalidData),
    };
    Ok(Some(slot))
}

fn parse_fx_list(tokens: &[&str]) -> Result<Option<Arc<FXList>>, INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    if token.eq_ignore_ascii_case("NONE") {
        return Ok(None);
    }
    Ok(TheFXListStore::find_fx_list(token))
}

fn parse_weapon_template(tokens: &[&str]) -> Result<Option<Arc<WeaponTemplate>>, INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    if token.eq_ignore_ascii_case("NONE") {
        return Ok(None);
    }
    let template = crate::weapon::with_weapon_store(|s| s.find_weapon_template(token).cloned())
        .map_err(|_| INIError::InvalidData)?;
    Ok(template)
}

fn parse_radius_decal_texture(
    _ini: &mut INI,
    data: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.texture_name = AsciiString::from(*token);
    Ok(())
}

fn parse_radius_decal_style(
    _ini: &mut INI,
    data: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.shadow_type = INI::parse_bit_string_32(tokens, &SHADOW_NAMES)?;
    Ok(())
}

fn parse_radius_decal_opacity_min(
    _ini: &mut INI,
    data: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.min_opacity = INI::parse_percent_to_real(token)?;
    data.opacity = data.min_opacity;
    Ok(())
}

fn parse_radius_decal_opacity_max(
    _ini: &mut INI,
    data: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.max_opacity = INI::parse_percent_to_real(token)?;
    data.opacity = data.max_opacity;
    Ok(())
}

fn parse_radius_decal_opacity_throb_time(
    _ini: &mut INI,
    data: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.opacity_throb_time = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

fn parse_radius_decal_color(
    _ini: &mut INI,
    data: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    if tokens.len() == 1 {
        if let Ok(value) = tokens[0].parse::<u32>() {
            data.color = value;
            return Ok(());
        }
    }

    let mut r: u8 = 0;
    let mut g: u8 = 0;
    let mut b: u8 = 0;
    let mut a: u8 = 255;

    for token in tokens {
        let (key, value) = match token.split_once(':') {
            Some((k, v)) => (k.trim(), v.trim()),
            None => ("", token.trim()),
        };
        let parsed = value.parse::<u8>().map_err(|_| INIError::InvalidData)?;
        match key.to_ascii_uppercase().as_str() {
            "R" => r = parsed,
            "G" => g = parsed,
            "B" => b = parsed,
            "A" => a = parsed,
            _ => {}
        }
    }

    data.color = ((a as u32) << 24) | ((b as u32) << 16) | ((g as u32) << 8) | (r as u32);
    Ok(())
}

fn parse_radius_decal_only_visible(
    _ini: &mut INI,
    data: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.only_visible_to_owning_player = INI::parse_bool(token)?;
    Ok(())
}

pub const RADIUS_DECAL_TEMPLATE_FIELDS: &[FieldParse<RadiusDecalTemplate>] = &[
    FieldParse {
        token: "Texture",
        parse: parse_radius_decal_texture,
    },
    FieldParse {
        token: "Style",
        parse: parse_radius_decal_style,
    },
    FieldParse {
        token: "OpacityMin",
        parse: parse_radius_decal_opacity_min,
    },
    FieldParse {
        token: "OpacityMax",
        parse: parse_radius_decal_opacity_max,
    },
    FieldParse {
        token: "OpacityThrobTime",
        parse: parse_radius_decal_opacity_throb_time,
    },
    FieldParse {
        token: "Color",
        parse: parse_radius_decal_color,
    },
    FieldParse {
        token: "OnlyVisibleToOwningPlayer",
        parse: parse_radius_decal_only_visible,
    },
];

fn parse_delivery_decal(
    ini: &mut INI,
    data: &mut DeliverPayloadData,
    _tokens: &[&str],
) -> Result<(), INIError> {
    ini.init_from_ini_with_fields(
        &mut data.delivery_decal_template,
        RADIUS_DECAL_TEMPLATE_FIELDS,
    )
}

pub const DELIVER_PAYLOAD_DATA_FIELDS: &[FieldParse<DeliverPayloadData>] = &[
    FieldParse {
        token: "DeliveryDistance",
        parse: |_, data, tokens| {
            data.dist_to_target = parse_real(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "PreOpenDistance",
        parse: |_, data, tokens| {
            data.pre_open_distance = parse_real(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "MaxAttempts",
        parse: |_, data, tokens| {
            data.max_attempts = parse_int(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "DropDelay",
        parse: |_, data, tokens| {
            data.drop_delay = parse_duration(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "DropOffset",
        parse: |_, data, tokens| {
            data.drop_offset = parse_coord3d(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "DropVariance",
        parse: |_, data, tokens| {
            data.drop_variance = parse_coord3d(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "InheritTransportVelocity",
        parse: |_, data, tokens| {
            data.inherit_transport_velocity = parse_bool(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "ExitPitchRate",
        parse: |_, data, tokens| {
            data.exit_pitch_rate = parse_real(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "ParachuteDirectly",
        parse: |_, data, tokens| {
            data.is_parachute_directly = parse_bool(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "VisibleItemsDroppedPerInterval",
        parse: |_, data, tokens| {
            data.visible_items_dropped_per_interval = parse_int(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "VisibleDropBoneBaseName",
        parse: |_, data, tokens| {
            data.visible_drop_bone_name = parse_ascii(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "VisibleSubObjectBaseName",
        parse: |_, data, tokens| {
            data.visible_sub_object_name = parse_ascii(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "VisibleNumBones",
        parse: |_, data, tokens| {
            data.visible_num_bones = parse_int(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "VisiblePayloadTemplateName",
        parse: |_, data, tokens| {
            data.visible_payload_template_name = parse_ascii(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "VisiblePayloadWeaponTemplate",
        parse: |_, data, tokens| {
            data.visible_payload_weapon_template = parse_weapon_template(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "SelfDestructObject",
        parse: |_, data, tokens| {
            data.self_destruct_object = parse_bool(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "FireWeapon",
        parse: |_, data, tokens| {
            data.fire_weapon = parse_bool(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "DiveStartDistance",
        parse: |_, data, tokens| {
            data.dive_start_distance = parse_real(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "DiveEndDistance",
        parse: |_, data, tokens| {
            data.dive_end_distance = parse_real(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "StrafingWeaponSlot",
        parse: |_, data, tokens| {
            data.strafing_weapon_slot = parse_weapon_slot(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "StrafeWeaponFX",
        parse: |_, data, tokens| {
            data.strafe_fx = parse_fx_list(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "StrafeLength",
        parse: |_, data, tokens| {
            data.strafe_length = parse_real(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "DeliveryDecal",
        parse: parse_delivery_decal,
    },
    FieldParse {
        token: "DeliveryDecalRadius",
        parse: |_, data, tokens| {
            data.delivery_decal_radius = parse_real(tokens)?;
            Ok(())
        },
    },
];
