//! Radius decal definitions and INI fields from RadiusDecal.cpp.

use crate::common::ascii_string::AsciiString;
use crate::common::game_common::LOGICFRAMES_PER_SECOND;
use crate::common::ini::{FieldParse, INI, INIError};

// Shadow type bit flags (matches GameClient/Shadow.h TheShadowNames order)
pub const SHADOW_DECAL: u32 = 0x0000_0001;
pub const SHADOW_VOLUME: u32 = 0x0000_0002;
pub const SHADOW_PROJECTION: u32 = 0x0000_0004;
pub const SHADOW_DYNAMIC_PROJECTION: u32 = 0x0000_0008;
pub const SHADOW_DIRECTIONAL_PROJECTION: u32 = 0x0000_0010;
pub const SHADOW_ALPHA_DECAL: u32 = 0x0000_0020;
pub const SHADOW_ADDITIVE_DECAL: u32 = 0x0000_0040;

pub const SHADOW_NAMES: [&str; 7] = [
    "SHADOW_DECAL",
    "SHADOW_VOLUME",
    "SHADOW_PROJECTION",
    "SHADOW_DYNAMIC_PROJECTION",
    "SHADOW_DIRECTIONAL_PROJECTION",
    "SHADOW_ALPHA_DECAL",
    "SHADOW_ADDITIVE_DECAL",
];

/// Template for radius decals
#[derive(Debug, Clone)]
pub struct RadiusDecalTemplate {
    /// Default radius
    pub radius: f32,
    /// Default opacity
    pub opacity: f32,
    /// Default color
    pub color: u32,
    /// Texture name (if applicable)
    pub texture_name: AsciiString,
    /// Shadow/decal style flags (bitset, matches ShadowType)
    pub shadow_type: u32,
    /// Minimum opacity for throb effects
    pub min_opacity: f32,
    /// Maximum opacity for throb effects
    pub max_opacity: f32,
    /// Opacity throb time (frames)
    pub opacity_throb_time: u32,
    /// Visibility restricted to owning player
    pub only_visible_to_owning_player: bool,
}

impl Default for RadiusDecalTemplate {
    fn default() -> Self {
        Self {
            radius: 0.0,
            opacity: 1.0,
            color: 0,
            texture_name: AsciiString::new(),
            shadow_type: SHADOW_ALPHA_DECAL,
            min_opacity: 1.0,
            max_opacity: 1.0,
            opacity_throb_time: LOGICFRAMES_PER_SECOND,
            only_visible_to_owning_player: true,
        }
    }
}

fn required_value<'a>(tokens: &'a [&'a str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

fn parse_radius_decal_texture(
    _ini: &mut INI,
    data: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.texture_name = AsciiString::from(required_value(tokens)?);
    Ok(())
}

fn parse_radius_decal_style(
    _ini: &mut INI,
    data: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    let values: Vec<&str> = tokens
        .iter()
        .copied()
        .filter(|token| *token != "=")
        .collect();
    data.shadow_type = INI::parse_bit_string_32(&values, &SHADOW_NAMES)?;
    Ok(())
}

fn parse_radius_decal_opacity_min(
    _ini: &mut INI,
    data: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    data.min_opacity = INI::parse_percent_to_real(token)?;
    data.opacity = data.min_opacity;
    Ok(())
}

fn parse_radius_decal_opacity_max(
    _ini: &mut INI,
    data: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    data.max_opacity = INI::parse_percent_to_real(token)?;
    data.opacity = data.max_opacity;
    Ok(())
}

fn parse_radius_decal_opacity_throb_time(
    _ini: &mut INI,
    data: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    data.opacity_throb_time = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

fn parse_radius_decal_color(
    _ini: &mut INI,
    data: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    // Retain the existing single packed-u32 Rust fixture adapter separately
    // from C++ RadiusDecal.cpp:94-109's ordered INI::parseColorInt grammar.
    let mut values = tokens.iter().copied().filter(|value| *value != "=");
    if let Some(value) = values.next() {
        if values.next().is_none() {
            if let Ok(packed) = value.parse::<u32>() {
                data.color = packed;
                return Ok(());
            }
        }
    }
    data.color = INI::parse_color_int_ordered(tokens)?;
    Ok(())
}

fn parse_radius_decal_only_visible(
    _ini: &mut INI,
    data: &mut RadiusDecalTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delivery_decal_fields_accept_ini_equals_token() {
        let mut ini = INI::new();
        let mut decal = RadiusDecalTemplate::default();

        parse_radius_decal_texture(&mut ini, &mut decal, &["=", "SCCArtilleryBarrage_China"])
            .unwrap();
        parse_radius_decal_opacity_min(&mut ini, &mut decal, &["=", "25%"]).unwrap();
        parse_radius_decal_opacity_max(&mut ini, &mut decal, &["=", "50%"]).unwrap();
        parse_radius_decal_opacity_throb_time(&mut ini, &mut decal, &["=", "500"]).unwrap();
        parse_radius_decal_color(
            &mut ini,
            &mut decal,
            &["=", "R:255", "G:156", "B:0", "A:255"],
        )
        .unwrap();
        parse_radius_decal_only_visible(&mut ini, &mut decal, &["=", "Yes"]).unwrap();

        assert_eq!(decal.texture_name.as_str(), "SCCArtilleryBarrage_China");
        assert!((decal.min_opacity - 0.25).abs() < f32::EPSILON);
        assert!((decal.max_opacity - 0.5).abs() < f32::EPSILON);
        assert_eq!(decal.opacity_throb_time, 15);
        // C++ Color.h:37-40 GameMakeColor(255, 156, 0, 255).
        assert_eq!(decal.color, 0xffff9c00);
        assert!(decal.only_visible_to_owning_player);
    }
}
