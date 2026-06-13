////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

//! FILE: ini_terrain_bridge.rs
//! Author: Colin Day, December 2001 (Converted to Rust)
//! Desc:   Terrain bridge INI loading

use crate::common::ascii_string::AsciiString;
use crate::common::ini::{FieldParse, INIError, INI};
use crate::debug_assert_crash;
use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

pub const BODYDAMAGETYPE_COUNT: usize = 5;
pub const MAX_BRIDGE_BODY_FX: usize = 3;
const BODY_PRISTINE_INDEX: usize = 0;
const BODY_DAMAGED_INDEX: usize = 1;

fn parse_body_damage_type(value: &str) -> Result<usize, INIError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "pristine" => Ok(0),
        "light" => Ok(1),
        "medium" => Ok(2),
        "heavy" => Ok(3),
        "critical" => Ok(4),
        _ => value
            .parse::<usize>()
            .ok()
            .filter(|v| *v < BODYDAMAGETYPE_COUNT)
            .ok_or(INIError::InvalidData),
    }
}

struct BridgeParseContext {
    bridge: TerrainRoadType,
    properties: HashMap<String, String>,
}

impl BridgeParseContext {
    fn new(name: AsciiString) -> Self {
        Self {
            bridge: TerrainRoadType::new_bridge(name),
            properties: HashMap::new(),
        }
    }
}

fn store_property(
    ctx: &mut BridgeParseContext,
    key: &str,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .copied()
        .filter(|token| *token != "=")
        .collect::<Vec<_>>()
        .join(" ");

    if value.is_empty() {
        return Err(INIError::InvalidData);
    }

    ctx.properties.insert(key.to_string(), value);
    Ok(())
}

macro_rules! bridge_property_parser {
    ($fn_name:ident, $key:expr) => {
        fn $fn_name(
            _: &mut INI,
            ctx: &mut BridgeParseContext,
            tokens: &[&str],
        ) -> Result<(), INIError> {
            store_property(ctx, $key, tokens)
        }
    };
}

bridge_property_parser!(parse_material_field, "Material");
bridge_property_parser!(parse_texture_field, "Texture");
bridge_property_parser!(parse_model_field, "Model");
bridge_property_parser!(parse_width_field, "Width");
bridge_property_parser!(parse_height_field, "Height");
bridge_property_parser!(parse_length_field, "Length");
bridge_property_parser!(parse_max_span_field, "MaxSpan");
bridge_property_parser!(parse_health_field, "Health");
bridge_property_parser!(parse_armor_field, "Armor");
bridge_property_parser!(parse_can_be_destroyed_field, "CanBeDestroyed");
bridge_property_parser!(parse_can_be_repaired_field, "CanBeRepaired");
bridge_property_parser!(parse_construction_time_field, "ConstructionTime");
bridge_property_parser!(parse_construction_cost_field, "ConstructionCost");
bridge_property_parser!(parse_movement_speed_modifier_field, "MovementSpeedModifier");
bridge_property_parser!(parse_supports_infantry_field, "SupportsInfantry");
bridge_property_parser!(parse_supports_vehicles_field, "SupportsVehicles");
bridge_property_parser!(parse_supports_tanks_field, "SupportsTanks");
bridge_property_parser!(parse_supports_aircraft_field, "SupportsAircraft");
bridge_property_parser!(parse_sound_effect_walking_field, "SoundEffectWalking");
bridge_property_parser!(parse_sound_effect_driving_field, "SoundEffectDriving");
bridge_property_parser!(
    parse_sound_effect_destruction_field,
    "SoundEffectDestruction"
);
bridge_property_parser!(
    parse_particle_effect_destruction_field,
    "ParticleEffectDestruction"
);
bridge_property_parser!(parse_bridge_scale_field, "BridgeScale");
bridge_property_parser!(parse_radar_color_field, "RadarColor");
bridge_property_parser!(parse_bridge_model_name_field, "BridgeModelName");
bridge_property_parser!(
    parse_bridge_model_name_damaged_field,
    "BridgeModelNameDamaged"
);
bridge_property_parser!(
    parse_bridge_model_name_really_damaged_field,
    "BridgeModelNameReallyDamaged"
);
bridge_property_parser!(
    parse_bridge_model_name_broken_field,
    "BridgeModelNameBroken"
);
bridge_property_parser!(parse_texture_damaged_field, "TextureDamaged");
bridge_property_parser!(parse_texture_really_damaged_field, "TextureReallyDamaged");
bridge_property_parser!(parse_texture_broken_field, "TextureBroken");
bridge_property_parser!(parse_tower_from_left_field, "TowerObjectNameFromLeft");
bridge_property_parser!(parse_tower_from_right_field, "TowerObjectNameFromRight");
bridge_property_parser!(parse_tower_to_left_field, "TowerObjectNameToLeft");
bridge_property_parser!(parse_tower_to_right_field, "TowerObjectNameToRight");
bridge_property_parser!(parse_scaffold_object_field, "ScaffoldObjectName");
bridge_property_parser!(
    parse_scaffold_support_object_field,
    "ScaffoldSupportObjectName"
);
bridge_property_parser!(
    parse_transition_effects_height_field,
    "TransitionEffectsHeight"
);
bridge_property_parser!(parse_num_fx_per_type_field, "NumFXPerType");

fn parse_transition_to_ocl_field(
    _: &mut INI,
    ctx: &mut BridgeParseContext,
    tokens: &[&str],
) -> Result<(), INIError> {
    ctx.bridge.parse_bridge_effect_tokens(tokens, false)
}

fn parse_transition_to_fx_field(
    _: &mut INI,
    ctx: &mut BridgeParseContext,
    tokens: &[&str],
) -> Result<(), INIError> {
    ctx.bridge.parse_bridge_effect_tokens(tokens, true)
}

fn parse_damaged_to_sound_field(
    _: &mut INI,
    ctx: &mut BridgeParseContext,
    tokens: &[&str],
) -> Result<(), INIError> {
    ctx.bridge.parse_damage_to_sound_tokens(tokens)
}

fn parse_repaired_to_sound_field(
    _: &mut INI,
    ctx: &mut BridgeParseContext,
    tokens: &[&str],
) -> Result<(), INIError> {
    ctx.bridge.parse_repaired_to_sound_tokens(tokens)
}

fn parse_transition_to_ocl(
    _: &mut INI,
    road: &mut TerrainRoadType,
    tokens: &[&str],
) -> Result<(), INIError> {
    road.parse_bridge_effect_tokens(tokens, false)
}

fn parse_transition_to_fx(
    _: &mut INI,
    road: &mut TerrainRoadType,
    tokens: &[&str],
) -> Result<(), INIError> {
    road.parse_bridge_effect_tokens(tokens, true)
}

fn parse_damage_to_sound(
    _: &mut INI,
    road: &mut TerrainRoadType,
    tokens: &[&str],
) -> Result<(), INIError> {
    road.parse_damage_to_sound_tokens(tokens)
}

fn parse_repaired_to_sound(
    _: &mut INI,
    road: &mut TerrainRoadType,
    tokens: &[&str],
) -> Result<(), INIError> {
    road.parse_repaired_to_sound_tokens(tokens)
}

const BRIDGE_PARSE_TABLE: &[FieldParse<BridgeParseContext>] = &[
    FieldParse {
        token: "Material",
        parse: parse_material_field,
    },
    FieldParse {
        token: "Texture",
        parse: parse_texture_field,
    },
    FieldParse {
        token: "Model",
        parse: parse_model_field,
    },
    FieldParse {
        token: "Width",
        parse: parse_width_field,
    },
    FieldParse {
        token: "Height",
        parse: parse_height_field,
    },
    FieldParse {
        token: "Length",
        parse: parse_length_field,
    },
    FieldParse {
        token: "MaxSpan",
        parse: parse_max_span_field,
    },
    FieldParse {
        token: "Health",
        parse: parse_health_field,
    },
    FieldParse {
        token: "Armor",
        parse: parse_armor_field,
    },
    FieldParse {
        token: "CanBeDestroyed",
        parse: parse_can_be_destroyed_field,
    },
    FieldParse {
        token: "CanBeRepaired",
        parse: parse_can_be_repaired_field,
    },
    FieldParse {
        token: "ConstructionTime",
        parse: parse_construction_time_field,
    },
    FieldParse {
        token: "ConstructionCost",
        parse: parse_construction_cost_field,
    },
    FieldParse {
        token: "MovementSpeedModifier",
        parse: parse_movement_speed_modifier_field,
    },
    FieldParse {
        token: "SupportsInfantry",
        parse: parse_supports_infantry_field,
    },
    FieldParse {
        token: "SupportsVehicles",
        parse: parse_supports_vehicles_field,
    },
    FieldParse {
        token: "SupportsTanks",
        parse: parse_supports_tanks_field,
    },
    FieldParse {
        token: "SupportsAircraft",
        parse: parse_supports_aircraft_field,
    },
    FieldParse {
        token: "SoundEffectWalking",
        parse: parse_sound_effect_walking_field,
    },
    FieldParse {
        token: "SoundEffectDriving",
        parse: parse_sound_effect_driving_field,
    },
    FieldParse {
        token: "SoundEffectDestruction",
        parse: parse_sound_effect_destruction_field,
    },
    FieldParse {
        token: "ParticleEffectDestruction",
        parse: parse_particle_effect_destruction_field,
    },
    FieldParse {
        token: "BridgeScale",
        parse: parse_bridge_scale_field,
    },
    FieldParse {
        token: "RadarColor",
        parse: parse_radar_color_field,
    },
    FieldParse {
        token: "BridgeModelName",
        parse: parse_bridge_model_name_field,
    },
    FieldParse {
        token: "BridgeModelNameDamaged",
        parse: parse_bridge_model_name_damaged_field,
    },
    FieldParse {
        token: "BridgeModelNameReallyDamaged",
        parse: parse_bridge_model_name_really_damaged_field,
    },
    FieldParse {
        token: "BridgeModelNameBroken",
        parse: parse_bridge_model_name_broken_field,
    },
    FieldParse {
        token: "TextureDamaged",
        parse: parse_texture_damaged_field,
    },
    FieldParse {
        token: "TextureReallyDamaged",
        parse: parse_texture_really_damaged_field,
    },
    FieldParse {
        token: "TextureBroken",
        parse: parse_texture_broken_field,
    },
    FieldParse {
        token: "TowerObjectNameFromLeft",
        parse: parse_tower_from_left_field,
    },
    FieldParse {
        token: "TowerObjectNameFromRight",
        parse: parse_tower_from_right_field,
    },
    FieldParse {
        token: "TowerObjectNameToLeft",
        parse: parse_tower_to_left_field,
    },
    FieldParse {
        token: "TowerObjectNameToRight",
        parse: parse_tower_to_right_field,
    },
    FieldParse {
        token: "ScaffoldObjectName",
        parse: parse_scaffold_object_field,
    },
    FieldParse {
        token: "ScaffoldSupportObjectName",
        parse: parse_scaffold_support_object_field,
    },
    FieldParse {
        token: "TransitionEffectsHeight",
        parse: parse_transition_effects_height_field,
    },
    FieldParse {
        token: "NumFXPerType",
        parse: parse_num_fx_per_type_field,
    },
    FieldParse {
        token: "DamagedToSound",
        parse: parse_damaged_to_sound_field,
    },
    FieldParse {
        token: "RepairedToSound",
        parse: parse_repaired_to_sound_field,
    },
    FieldParse {
        token: "TransitionToOCL",
        parse: parse_transition_to_ocl_field,
    },
    FieldParse {
        token: "TransitionToFX",
        parse: parse_transition_to_fx_field,
    },
];
/// Result type for terrain bridge parsing operations
pub type TerrainBridgeResult<T> = Result<T, TerrainBridgeError>;

/// Errors that can occur during terrain bridge parsing
#[derive(Debug, Clone, PartialEq)]
pub enum TerrainBridgeError {
    InvalidName,
    AllocationError,
    ParseError(String),
    InvalidData,
    NotFound,
    AlreadyExists,
    ConflictingType,
}

impl std::fmt::Display for TerrainBridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TerrainBridgeError::InvalidName => write!(f, "Invalid bridge name"),
            TerrainBridgeError::AllocationError => write!(f, "Failed to allocate bridge"),
            TerrainBridgeError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            TerrainBridgeError::InvalidData => write!(f, "Invalid bridge data"),
            TerrainBridgeError::NotFound => write!(f, "Bridge not found"),
            TerrainBridgeError::AlreadyExists => write!(f, "Bridge already exists"),
            TerrainBridgeError::ConflictingType => write!(f, "Conflicting bridge/road type"),
        }
    }
}

impl std::error::Error for TerrainBridgeError {}

/// Bridge construction materials
#[derive(Debug, Clone, PartialEq)]
pub enum BridgeMaterial {
    Wood,
    Stone,
    Steel,
    Concrete,
    Rope,
    Custom(String),
}

impl BridgeMaterial {
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "wood" => Self::Wood,
            "stone" => Self::Stone,
            "steel" => Self::Steel,
            "concrete" => Self::Concrete,
            "rope" => Self::Rope,
            _ => Self::Custom(s.to_string()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Wood => "Wood",
            Self::Stone => "Stone",
            Self::Steel => "Steel",
            Self::Concrete => "Concrete",
            Self::Rope => "Rope",
            Self::Custom(name) => name,
        }
    }
}

fn parse_f32_field(field_name: &str, value: &str) -> Result<f32, String> {
    value
        .parse::<f32>()
        .map_err(|e| format!("{}: invalid real value '{}': {}", field_name, value, e))
}

fn parse_usize_field(field_name: &str, value: &str) -> Result<usize, String> {
    value
        .parse::<usize>()
        .map_err(|e| format!("{}: invalid integer value '{}': {}", field_name, value, e))
}

/// Bridge connection points
#[derive(Debug, Clone)]
pub struct BridgeConnectionPoint {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub orientation: f32, // Angle in radians
}

impl Default for BridgeConnectionPoint {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            orientation: 0.0,
        }
    }
}

/// Terrain road type definition (includes bridges)
#[derive(Debug, Clone)]
pub struct TerrainRoadType {
    pub name: AsciiString,
    pub is_bridge: bool,
    pub material: BridgeMaterial,
    pub texture_name: AsciiString,
    pub model_name: AsciiString,
    pub width: f32,
    pub road_width_in_texture: f32,
    pub height: f32,
    pub length: f32,
    pub max_span: f32,
    pub health: f32,
    pub armor: f32,
    pub can_be_destroyed: bool,
    pub can_be_repaired: bool,
    pub construction_time: f32,
    pub construction_cost: u32,
    pub sound_effect_walking: AsciiString,
    pub sound_effect_driving: AsciiString,
    pub sound_effect_destruction: AsciiString,
    pub particle_effect_destruction: AsciiString,
    pub connection_points: Vec<BridgeConnectionPoint>,
    pub movement_speed_modifier: f32,
    pub supports_infantry: bool,
    pub supports_vehicles: bool,
    pub supports_tanks: bool,
    pub supports_aircraft: bool,
    pub bridge_scale: f32,
    pub scaffold_object_name: AsciiString,
    pub scaffold_support_object_name: AsciiString,
    pub radar_color: AsciiString,
    pub transition_effects_height: f32,
    pub num_fx_per_type: usize,
    pub bridge_model_name_damaged: AsciiString,
    pub bridge_model_name_really_damaged: AsciiString,
    pub bridge_model_name_broken: AsciiString,
    pub texture_damaged: AsciiString,
    pub texture_really_damaged: AsciiString,
    pub texture_broken: AsciiString,
    pub tower_object_name_from_left: AsciiString,
    pub tower_object_name_from_right: AsciiString,
    pub tower_object_name_to_left: AsciiString,
    pub tower_object_name_to_right: AsciiString,
    pub properties: HashMap<String, String>,
    pub damage_to_fx_string: [[AsciiString; MAX_BRIDGE_BODY_FX]; BODYDAMAGETYPE_COUNT],
    pub damage_to_ocl_string: [[AsciiString; MAX_BRIDGE_BODY_FX]; BODYDAMAGETYPE_COUNT],
    pub repair_to_fx_string: [[AsciiString; MAX_BRIDGE_BODY_FX]; BODYDAMAGETYPE_COUNT],
    pub repair_to_ocl_string: [[AsciiString; MAX_BRIDGE_BODY_FX]; BODYDAMAGETYPE_COUNT],
    pub damage_to_sound_string: [AsciiString; BODYDAMAGETYPE_COUNT],
    pub repair_to_sound_string: [AsciiString; BODYDAMAGETYPE_COUNT],
}

impl TerrainRoadType {
    pub fn new_bridge(name: AsciiString) -> Self {
        Self {
            name,
            is_bridge: true,
            material: BridgeMaterial::Wood,
            texture_name: AsciiString::from(""),
            model_name: AsciiString::from(""),
            width: 10.0,
            road_width_in_texture: 0.0,
            height: 5.0,
            length: 50.0,
            max_span: 100.0,
            health: 1000.0,
            armor: 100.0,
            can_be_destroyed: true,
            can_be_repaired: true,
            construction_time: 30.0,
            construction_cost: 500,
            sound_effect_walking: AsciiString::from(""),
            sound_effect_driving: AsciiString::from(""),
            sound_effect_destruction: AsciiString::from(""),
            particle_effect_destruction: AsciiString::from(""),
            connection_points: Vec::new(),
            movement_speed_modifier: 1.0,
            supports_infantry: true,
            supports_vehicles: true,
            supports_tanks: true,
            supports_aircraft: false,
            bridge_scale: 1.0,
            scaffold_object_name: AsciiString::from(""),
            scaffold_support_object_name: AsciiString::from(""),
            radar_color: AsciiString::from(""),
            transition_effects_height: 0.0,
            num_fx_per_type: 0,
            bridge_model_name_damaged: AsciiString::from(""),
            bridge_model_name_really_damaged: AsciiString::from(""),
            bridge_model_name_broken: AsciiString::from(""),
            texture_damaged: AsciiString::from(""),
            texture_really_damaged: AsciiString::from(""),
            texture_broken: AsciiString::from(""),
            tower_object_name_from_left: AsciiString::from(""),
            tower_object_name_from_right: AsciiString::from(""),
            tower_object_name_to_left: AsciiString::from(""),
            tower_object_name_to_right: AsciiString::from(""),
            properties: HashMap::new(),
            damage_to_fx_string: Default::default(),
            damage_to_ocl_string: Default::default(),
            repair_to_fx_string: Default::default(),
            repair_to_ocl_string: Default::default(),
            damage_to_sound_string: empty_ascii_array(),
            repair_to_sound_string: empty_ascii_array(),
        }
    }

    pub fn new_road(name: AsciiString) -> Self {
        Self {
            name,
            is_bridge: false,
            material: BridgeMaterial::Stone,
            texture_name: AsciiString::from(""),
            model_name: AsciiString::from(""),
            width: 8.0,
            road_width_in_texture: 0.0,
            height: 0.5,
            length: 0.0, // Roads are continuous
            max_span: 0.0,
            health: 500.0,
            armor: 50.0,
            can_be_destroyed: false,
            can_be_repaired: false,
            construction_time: 10.0,
            construction_cost: 100,
            sound_effect_walking: AsciiString::from(""),
            sound_effect_driving: AsciiString::from(""),
            sound_effect_destruction: AsciiString::from(""),
            particle_effect_destruction: AsciiString::from(""),
            connection_points: Vec::new(),
            movement_speed_modifier: 1.2, // Roads are faster
            supports_infantry: true,
            supports_vehicles: true,
            supports_tanks: true,
            supports_aircraft: false,
            bridge_scale: 1.0,
            scaffold_object_name: AsciiString::from(""),
            scaffold_support_object_name: AsciiString::from(""),
            radar_color: AsciiString::from(""),
            transition_effects_height: 0.0,
            num_fx_per_type: 0,
            bridge_model_name_damaged: AsciiString::from(""),
            bridge_model_name_really_damaged: AsciiString::from(""),
            bridge_model_name_broken: AsciiString::from(""),
            texture_damaged: AsciiString::from(""),
            texture_really_damaged: AsciiString::from(""),
            texture_broken: AsciiString::from(""),
            tower_object_name_from_left: AsciiString::from(""),
            tower_object_name_from_right: AsciiString::from(""),
            tower_object_name_to_left: AsciiString::from(""),
            tower_object_name_to_right: AsciiString::from(""),
            properties: HashMap::new(),
            damage_to_fx_string: Default::default(),
            damage_to_ocl_string: Default::default(),
            repair_to_fx_string: Default::default(),
            repair_to_ocl_string: Default::default(),
            damage_to_sound_string: empty_ascii_array(),
            repair_to_sound_string: empty_ascii_array(),
        }
    }

    /// Get the bridge field parse table
    pub fn get_bridge_field_parse(
        &self,
    ) -> Vec<(
        &'static str,
        fn(&str) -> Result<Box<dyn std::any::Any>, String>,
    )> {
        vec![
            ("Material", |value| {
                Ok(Box::new(BridgeMaterial::from_string(value)) as Box<dyn std::any::Any>)
            }),
            ("Texture", |value| {
                Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
            }),
            ("Model", |value| {
                Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
            }),
            ("Width", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse width: {}", e))
            }),
            ("Height", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse height: {}", e))
            }),
            ("Length", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse length: {}", e))
            }),
            ("MaxSpan", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse max span: {}", e))
            }),
            ("Health", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse health: {}", e))
            }),
            ("Armor", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse armor: {}", e))
            }),
            ("CanBeDestroyed", |value| {
                parse_bool(value)
                    .map(|b| Box::new(b) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse destroyable: {}", e))
            }),
            ("CanBeRepaired", |value| {
                parse_bool(value)
                    .map(|b| Box::new(b) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse repairable: {}", e))
            }),
            ("ConstructionTime", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse construction time: {}", e))
            }),
            ("ConstructionCost", |value| {
                value
                    .parse::<u32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse construction cost: {}", e))
            }),
            ("MovementSpeedModifier", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse movement speed modifier: {}", e))
            }),
            ("SupportsInfantry", |value| {
                parse_bool(value)
                    .map(|b| Box::new(b) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse supports infantry: {}", e))
            }),
            ("SupportsVehicles", |value| {
                parse_bool(value)
                    .map(|b| Box::new(b) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse supports vehicles: {}", e))
            }),
            ("SupportsTanks", |value| {
                parse_bool(value)
                    .map(|b| Box::new(b) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse supports tanks: {}", e))
            }),
            ("SoundEffectWalking", |value| {
                Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
            }),
            ("SoundEffectDriving", |value| {
                Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
            }),
            ("SoundEffectDestruction", |value| {
                Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
            }),
            ("ParticleEffectDestruction", |value| {
                Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
            }),
        ]
    }

    /// Get the road field parse table
    pub fn get_road_field_parse(
        &self,
    ) -> Vec<(
        &'static str,
        fn(&str) -> Result<Box<dyn std::any::Any>, String>,
    )> {
        // Roads share most properties with bridges but have some differences
        self.get_bridge_field_parse()
    }

    pub fn get_bridge_effect_field_parse(
        &self,
    ) -> Vec<(
        &'static str,
        fn(&mut INI, &mut TerrainRoadType, &[&str]) -> Result<(), INIError>,
    )> {
        vec![
            ("TransitionToOCL", parse_transition_to_ocl),
            ("TransitionToFX", parse_transition_to_fx),
            ("DamagedToSound", parse_damage_to_sound),
            ("RepairedToSound", parse_repaired_to_sound),
        ]
    }

    pub fn parse_bridge_effect_tokens(
        &mut self,
        tokens: &[&str],
        store_fx: bool,
    ) -> Result<(), INIError> {
        let mut is_damage: Option<bool> = None;
        let mut state: Option<usize> = None;
        let mut effect_num: Option<usize> = None;
        let mut value: Option<AsciiString> = None;

        let mut iter = tokens.iter().copied().filter(|token| *token != "=");
        while let Some(raw_token) = iter.next() {
            let token = raw_token.trim();
            if token.is_empty() {
                continue;
            }

            let (key, val) = if let Some((key, val)) = token.split_once(':') {
                (key, val)
            } else if let Some((key, val)) = token.split_once('=') {
                (key, val)
            } else {
                let next = iter.next().ok_or(INIError::InvalidData)?;
                (token, next)
            };

            let key_normalized = key.trim().to_ascii_lowercase();
            let value_str = val.trim();

            match key_normalized.as_str() {
                "transition" => {
                    is_damage = match value_str.to_ascii_lowercase().as_str() {
                        "damage" => Some(true),
                        "repair" => Some(false),
                        _ => return Err(INIError::InvalidData),
                    };
                }
                "tostate" => {
                    state = Some(parse_body_damage_type(value_str)?);
                }
                "effectnum" => {
                    let num: usize = value_str.parse().map_err(|_| INIError::InvalidData)?;
                    if num == 0 || num > MAX_BRIDGE_BODY_FX {
                        return Err(INIError::InvalidData);
                    }
                    effect_num = Some(num - 1);
                }
                "fx" if store_fx => {
                    value = Some(AsciiString::from(value_str));
                }
                "ocl" if !store_fx => {
                    value = Some(AsciiString::from(value_str));
                }
                // Accept labels we don't understand so we stay compatible with loose INI input.
                _ => {}
            }
        }

        let state = state.ok_or(INIError::InvalidData)?;
        if state >= BODYDAMAGETYPE_COUNT {
            return Err(INIError::InvalidData);
        }
        let effect = effect_num.ok_or(INIError::InvalidData)?;
        let is_damage = is_damage.ok_or(INIError::InvalidData)?;
        let value = value.ok_or(INIError::InvalidData)?;

        if store_fx {
            if is_damage {
                self.set_damage_to_fx_string(state, effect, value);
            } else {
                self.set_repaired_to_fx_string(state, effect, value);
            }
        } else if is_damage {
            self.set_damage_to_ocl_string(state, effect, value);
        } else {
            self.set_repaired_to_ocl_string(state, effect, value);
        }

        Ok(())
    }

    pub fn parse_damage_to_sound_tokens(&mut self, tokens: &[&str]) -> Result<(), INIError> {
        let name = tokens
            .iter()
            .copied()
            .filter(|token| *token != "=")
            .collect::<Vec<_>>()
            .join(" ");
        self.set_damage_to_sound_string(BODY_DAMAGED_INDEX, AsciiString::from(name.as_str()));
        Ok(())
    }

    pub fn parse_repaired_to_sound_tokens(&mut self, tokens: &[&str]) -> Result<(), INIError> {
        let name = tokens
            .iter()
            .copied()
            .filter(|token| *token != "=")
            .collect::<Vec<_>>()
            .join(" ");
        self.set_repaired_to_sound_string(BODY_DAMAGED_INDEX, AsciiString::from(name.as_str()));
        Ok(())
    }

    /// Update bridge from properties
    pub fn update_from_properties(
        &mut self,
        properties: &HashMap<String, String>,
    ) -> Result<(), String> {
        for (key, value) in properties {
            match key.as_str() {
                "Material" => {
                    self.material = BridgeMaterial::from_string(value);
                }
                "Texture" => {
                    self.texture_name = AsciiString::from(value);
                }
                "Model" => {
                    self.model_name = AsciiString::from(value);
                }
                "Width" => {
                    if let Ok(width) = value.parse::<f32>() {
                        self.width = width;
                    }
                }
                "Height" => {
                    if let Ok(height) = value.parse::<f32>() {
                        self.height = height;
                    }
                }
                "Length" => {
                    if let Ok(length) = value.parse::<f32>() {
                        self.length = length;
                    }
                }
                "MaxSpan" => {
                    if let Ok(span) = value.parse::<f32>() {
                        self.max_span = span;
                    }
                }
                "Health" => {
                    if let Ok(health) = value.parse::<f32>() {
                        self.health = health;
                    }
                }
                "Armor" => {
                    if let Ok(armor) = value.parse::<f32>() {
                        self.armor = armor;
                    }
                }
                "CanBeDestroyed" => {
                    if let Ok(destroyable) = parse_bool(value) {
                        self.can_be_destroyed = destroyable;
                    }
                }
                "CanBeRepaired" => {
                    if let Ok(repairable) = parse_bool(value) {
                        self.can_be_repaired = repairable;
                    }
                }
                "ConstructionTime" => {
                    if let Ok(time) = value.parse::<f32>() {
                        self.construction_time = time;
                    }
                }
                "ConstructionCost" => {
                    if let Ok(cost) = value.parse::<u32>() {
                        self.construction_cost = cost;
                    }
                }
                "MovementSpeedModifier" => {
                    if let Ok(modifier) = value.parse::<f32>() {
                        self.movement_speed_modifier = modifier;
                    }
                }
                "SupportsInfantry" => {
                    if let Ok(supports) = parse_bool(value) {
                        self.supports_infantry = supports;
                    }
                }
                "SupportsVehicles" => {
                    if let Ok(supports) = parse_bool(value) {
                        self.supports_vehicles = supports;
                    }
                }
                "SupportsTanks" => {
                    if let Ok(supports) = parse_bool(value) {
                        self.supports_tanks = supports;
                    }
                }
                "SupportsAircraft" => {
                    if let Ok(supports) = parse_bool(value) {
                        self.supports_aircraft = supports;
                    }
                }
                "SoundEffectWalking" => {
                    self.sound_effect_walking = AsciiString::from(value);
                }
                "SoundEffectDriving" => {
                    self.sound_effect_driving = AsciiString::from(value);
                }
                "SoundEffectDestruction" => {
                    self.sound_effect_destruction = AsciiString::from(value);
                }
                "ParticleEffectDestruction" => {
                    self.particle_effect_destruction = AsciiString::from(value);
                }
                "BridgeScale" => {
                    self.bridge_scale = parse_f32_field(key, value)?;
                }
                "RadarColor" => {
                    self.radar_color = AsciiString::from(value);
                }
                "TransitionEffectsHeight" => {
                    self.transition_effects_height = parse_f32_field(key, value)?;
                }
                "NumFXPerType" => {
                    self.num_fx_per_type = parse_usize_field(key, value)?;
                }
                "BridgeModelName" => {
                    self.model_name = AsciiString::from(value);
                }
                "BridgeModelNameDamaged" => {
                    self.bridge_model_name_damaged = AsciiString::from(value);
                }
                "BridgeModelNameReallyDamaged" => {
                    self.bridge_model_name_really_damaged = AsciiString::from(value);
                }
                "BridgeModelNameBroken" => {
                    self.bridge_model_name_broken = AsciiString::from(value);
                }
                "TextureDamaged" => {
                    self.texture_damaged = AsciiString::from(value);
                }
                "TextureReallyDamaged" => {
                    self.texture_really_damaged = AsciiString::from(value);
                }
                "TextureBroken" => {
                    self.texture_broken = AsciiString::from(value);
                }
                "TowerObjectNameFromLeft" => {
                    self.tower_object_name_from_left = AsciiString::from(value);
                }
                "TowerObjectNameFromRight" => {
                    self.tower_object_name_from_right = AsciiString::from(value);
                }
                "TowerObjectNameToLeft" => {
                    self.tower_object_name_to_left = AsciiString::from(value);
                }
                "TowerObjectNameToRight" => {
                    self.tower_object_name_to_right = AsciiString::from(value);
                }
                "ScaffoldObjectName" => {
                    self.scaffold_object_name = AsciiString::from(value);
                }
                "ScaffoldSupportObjectName" => {
                    self.scaffold_support_object_name = AsciiString::from(value);
                }
                _ => {
                    // Store unknown properties
                    self.properties.insert(key.clone(), value.clone());
                }
            }
        }

        Ok(())
    }

    pub fn get_name(&self) -> &AsciiString {
        &self.name
    }

    pub fn is_bridge(&self) -> bool {
        self.is_bridge
    }

    pub fn is_valid(&self) -> bool {
        !self.name.is_empty() && self.width > 0.0
    }

    pub fn can_support_unit(&self, unit_type: &str) -> bool {
        match unit_type.to_lowercase().as_str() {
            "infantry" => self.supports_infantry,
            "vehicle" => self.supports_vehicles,
            "tank" => self.supports_tanks,
            "aircraft" => self.supports_aircraft,
            _ => false,
        }
    }

    pub fn add_connection_point(&mut self, x: f32, y: f32, z: f32, orientation: f32) {
        self.connection_points.push(BridgeConnectionPoint {
            x,
            y,
            z,
            orientation,
        });
    }

    pub fn get_damage_to_fx_string(&self, state: usize, index: usize) -> Option<AsciiString> {
        if state >= BODYDAMAGETYPE_COUNT || index >= MAX_BRIDGE_BODY_FX {
            return None;
        }
        let value = &self.damage_to_fx_string[state][index];
        if value.is_empty() {
            None
        } else {
            Some(value.clone())
        }
    }

    pub fn set_damage_to_fx_string(&mut self, state: usize, index: usize, value: AsciiString) {
        if state < BODYDAMAGETYPE_COUNT && index < MAX_BRIDGE_BODY_FX {
            self.damage_to_fx_string[state][index] = value;
        }
    }

    pub fn get_damage_to_ocl_string(&self, state: usize, index: usize) -> Option<AsciiString> {
        if state >= BODYDAMAGETYPE_COUNT || index >= MAX_BRIDGE_BODY_FX {
            return None;
        }
        let value = &self.damage_to_ocl_string[state][index];
        if value.is_empty() {
            None
        } else {
            Some(value.clone())
        }
    }

    pub fn set_damage_to_ocl_string(&mut self, state: usize, index: usize, value: AsciiString) {
        if state < BODYDAMAGETYPE_COUNT && index < MAX_BRIDGE_BODY_FX {
            self.damage_to_ocl_string[state][index] = value;
        }
    }

    pub fn get_repaired_to_fx_string(&self, state: usize, index: usize) -> Option<AsciiString> {
        if state >= BODYDAMAGETYPE_COUNT || index >= MAX_BRIDGE_BODY_FX {
            return None;
        }
        let value = &self.repair_to_fx_string[state][index];
        if value.is_empty() {
            None
        } else {
            Some(value.clone())
        }
    }

    pub fn set_repaired_to_fx_string(&mut self, state: usize, index: usize, value: AsciiString) {
        if state < BODYDAMAGETYPE_COUNT && index < MAX_BRIDGE_BODY_FX {
            self.repair_to_fx_string[state][index] = value;
        }
    }

    pub fn get_repaired_to_ocl_string(&self, state: usize, index: usize) -> Option<AsciiString> {
        if state >= BODYDAMAGETYPE_COUNT || index >= MAX_BRIDGE_BODY_FX {
            return None;
        }
        let value = &self.repair_to_ocl_string[state][index];
        if value.is_empty() {
            None
        } else {
            Some(value.clone())
        }
    }

    pub fn set_repaired_to_ocl_string(&mut self, state: usize, index: usize, value: AsciiString) {
        if state < BODYDAMAGETYPE_COUNT && index < MAX_BRIDGE_BODY_FX {
            self.repair_to_ocl_string[state][index] = value;
        }
    }

    pub fn get_damage_to_sound_string(&self, state: usize) -> Option<AsciiString> {
        if state >= BODYDAMAGETYPE_COUNT {
            return None;
        }
        let value = &self.damage_to_sound_string[state];
        if value.is_empty() {
            None
        } else {
            Some(value.clone())
        }
    }

    pub fn set_damage_to_sound_string(&mut self, state: usize, value: AsciiString) {
        if state < BODYDAMAGETYPE_COUNT {
            self.damage_to_sound_string[state] = value;
        }
    }

    pub fn get_repaired_to_sound_string(&self, state: usize) -> Option<AsciiString> {
        if state >= BODYDAMAGETYPE_COUNT {
            return None;
        }
        let value = &self.repair_to_sound_string[state];
        if value.is_empty() {
            None
        } else {
            Some(value.clone())
        }
    }

    pub fn set_repaired_to_sound_string(&mut self, state: usize, value: AsciiString) {
        if state < BODYDAMAGETYPE_COUNT {
            self.repair_to_sound_string[state] = value;
        }
    }
}

/// Terrain roads manager - manages both roads and bridges
#[derive(Debug)]
pub struct TerrainRoads {
    pub road_types: HashMap<String, TerrainRoadType>,
}

impl TerrainRoads {
    pub fn new() -> Self {
        Self {
            road_types: HashMap::new(),
        }
    }

    /// Find a bridge by name
    pub fn find_bridge(&self, name: &AsciiString) -> Option<&TerrainRoadType> {
        self.road_types
            .get(name.as_str())
            .filter(|road| road.is_bridge)
    }

    /// Find a mutable bridge by name
    pub fn find_bridge_mut(&mut self, name: &AsciiString) -> Option<&mut TerrainRoadType> {
        self.road_types
            .get_mut(name.as_str())
            .filter(|road| road.is_bridge)
    }

    /// Find a road by name
    pub fn find_road(&self, name: &AsciiString) -> Option<&TerrainRoadType> {
        self.road_types
            .get(name.as_str())
            .filter(|road| !road.is_bridge)
    }

    /// Find a mutable road by name
    pub fn find_road_mut(&mut self, name: &AsciiString) -> Option<&mut TerrainRoadType> {
        self.road_types
            .get_mut(name.as_str())
            .filter(|road| !road.is_bridge)
    }

    /// Create a new bridge
    pub fn new_bridge(&mut self, name: AsciiString) -> &mut TerrainRoadType {
        let default_template = {
            let default_name = AsciiString::from("DefaultBridge");
            self.find_bridge(&default_name).cloned()
        };

        let mut bridge = TerrainRoadType::new_bridge(name.clone());

        if let Some(default_bridge) = default_template {
            bridge.texture_name = default_bridge.texture_name.clone();
            bridge.bridge_scale = default_bridge.bridge_scale;
            bridge.model_name = default_bridge.model_name.clone();
            bridge.bridge_model_name_damaged = default_bridge.bridge_model_name_damaged.clone();
            bridge.bridge_model_name_really_damaged =
                default_bridge.bridge_model_name_really_damaged.clone();
            bridge.bridge_model_name_broken = default_bridge.bridge_model_name_broken.clone();
            bridge.texture_damaged = default_bridge.texture_damaged.clone();
            bridge.texture_really_damaged = default_bridge.texture_really_damaged.clone();
            bridge.texture_broken = default_bridge.texture_broken.clone();
            bridge.transition_effects_height = default_bridge.transition_effects_height;
            bridge.num_fx_per_type = default_bridge.num_fx_per_type;
            bridge.damage_to_sound_string = default_bridge.damage_to_sound_string.clone();
            bridge.repair_to_sound_string = default_bridge.repair_to_sound_string.clone();
            bridge.damage_to_ocl_string = default_bridge.damage_to_ocl_string.clone();
            bridge.damage_to_fx_string = default_bridge.damage_to_fx_string.clone();
            bridge.repair_to_ocl_string = default_bridge.repair_to_ocl_string.clone();
            bridge.repair_to_fx_string = default_bridge.repair_to_fx_string.clone();
        }

        self.road_types.insert(name.as_str().to_string(), bridge);
        self.road_types.get_mut(name.as_str()).unwrap()
    }

    /// Create a new road
    pub fn new_road(&mut self, name: AsciiString) -> &mut TerrainRoadType {
        let default_template = {
            let default_name = AsciiString::from("DefaultRoad");
            self.find_road(&default_name).cloned()
        };

        let mut road = TerrainRoadType::new_road(name.clone());

        if let Some(default_road) = default_template {
            road.texture_name = default_road.texture_name.clone();
            road.width = default_road.width;
            road.road_width_in_texture = default_road.road_width_in_texture;
        }

        self.road_types.insert(name.as_str().to_string(), road);
        self.road_types.get_mut(name.as_str()).unwrap()
    }

    /// Register a fully constructed road/bridge type
    pub fn register_road_type(&mut self, road_type: TerrainRoadType) {
        let name = road_type.name.as_str().to_string();
        self.road_types.insert(name, road_type);
    }

    /// Get all road/bridge names
    pub fn get_road_names(&self) -> Vec<&String> {
        self.road_types.keys().collect()
    }

    /// Get all bridges
    pub fn get_bridges(&self) -> Vec<&TerrainRoadType> {
        self.road_types
            .values()
            .filter(|road| road.is_bridge)
            .collect()
    }

    /// Get all roads (non-bridges)
    pub fn get_roads(&self) -> Vec<&TerrainRoadType> {
        self.road_types
            .values()
            .filter(|road| !road.is_bridge)
            .collect()
    }

    /// Remove a road/bridge
    pub fn remove_road_type(&mut self, name: &AsciiString) -> bool {
        self.road_types.remove(name.as_str()).is_some()
    }

    /// Clear all road types
    pub fn clear(&mut self) {
        self.road_types.clear();
    }

    /// Get road type count
    pub fn get_road_count(&self) -> usize {
        self.road_types.len()
    }
}

impl Default for TerrainRoads {
    fn default() -> Self {
        Self::new()
    }
}

/// Global terrain roads registry (thread-safe)
static TERRAIN_ROADS: OnceCell<Arc<RwLock<TerrainRoads>>> = OnceCell::new();

/// Ensure the terrain roads registry exists and return a handle to it
pub fn initialize_terrain_roads() -> Arc<RwLock<TerrainRoads>> {
    TERRAIN_ROADS
        .get_or_init(|| Arc::new(RwLock::new(TerrainRoads::new())))
        .clone()
}

/// Get the terrain roads registry if it has already been initialized
pub fn get_terrain_roads() -> Option<Arc<RwLock<TerrainRoads>>> {
    TERRAIN_ROADS.get().cloned()
}

/// Parse a boolean value from string
pub fn parse_bool(value: &str) -> Result<bool, String> {
    match value.trim().to_lowercase().as_str() {
        "true" | "yes" | "1" => Ok(true),
        "false" | "no" | "0" => Ok(false),
        _ => Err(format!("Invalid boolean value: {}", value)),
    }
}

fn empty_ascii_array<const N: usize>() -> [AsciiString; N] {
    std::array::from_fn(|_| AsciiString::default())
}

/// INI parsing functions for terrain bridges
pub struct IniTerrainBridge;

impl IniTerrainBridge {
    /// Parse terrain bridge definition - equivalent to INI::parseTerrainBridgeDefinition
    pub fn parse_terrain_bridge_definition(
        ini: &mut INI,
        name: AsciiString,
    ) -> TerrainBridgeResult<()> {
        if name.is_empty() {
            return Err(TerrainBridgeError::InvalidName);
        }

        let terrain_roads_handle = initialize_terrain_roads();
        let mut terrain_roads = terrain_roads_handle.write();

        if let Some(existing_road) = terrain_roads.find_road(&name) {
            if !existing_road.is_bridge {
                return Err(TerrainBridgeError::ConflictingType);
            }
        }

        let mut context = BridgeParseContext::new(name.clone());

        ini.init_from_ini_with_fields(&mut context, BRIDGE_PARSE_TABLE)
            .map_err(|err| match err {
                INIError::InvalidData => TerrainBridgeError::InvalidData,
                INIError::UnknownToken => TerrainBridgeError::ParseError(format!(
                    "Unknown token while parsing bridge '{}'",
                    name.as_str()
                )),
                other => TerrainBridgeError::ParseError(other.to_string()),
            })?;

        context
            .bridge
            .update_from_properties(&context.properties)
            .map_err(TerrainBridgeError::ParseError)?;

        if !context.bridge.is_valid() {
            return Err(TerrainBridgeError::ParseError(
                "Invalid bridge configuration".to_string(),
            ));
        }

        terrain_roads.register_road_type(context.bridge);
        Ok(())
    }

    /// Parse a complete terrain bridge block from pre-collected properties
    pub fn parse_terrain_bridge_block(
        name: AsciiString,
        properties: HashMap<String, String>,
    ) -> TerrainBridgeResult<TerrainRoadType> {
        if name.is_empty() {
            return Err(TerrainBridgeError::InvalidName);
        }

        let mut bridge = TerrainRoadType::new_bridge(name);
        bridge
            .update_from_properties(&properties)
            .map_err(TerrainBridgeError::ParseError)?;

        if !bridge.is_valid() {
            return Err(TerrainBridgeError::ParseError(
                "Invalid bridge configuration".to_string(),
            ));
        }

        Ok(bridge)
    }

    /// Register a terrain bridge
    pub fn register_terrain_bridge(bridge: TerrainRoadType) -> TerrainBridgeResult<()> {
        if !bridge.is_bridge {
            return Err(TerrainBridgeError::ConflictingType);
        }

        let terrain_roads = initialize_terrain_roads();
        terrain_roads.write().register_road_type(bridge);
        Ok(())
    }

    /// Find a terrain bridge by name
    pub fn find_terrain_bridge_by_name(name: &AsciiString) -> Option<TerrainRoadType> {
        get_terrain_roads()
            .and_then(|terrain_roads| terrain_roads.read().find_bridge(name).cloned())
    }

    /// Validate bridge name format
    pub fn validate_name(name: &AsciiString) -> bool {
        !name.is_empty() && name.len() < 128 // Reasonable length limit
    }

    /// Check if a name conflicts with existing roads
    pub fn check_name_conflict(name: &AsciiString) -> bool {
        get_terrain_roads()
            .map(|terrain_roads| terrain_roads.read().find_road(name).is_some())
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_material_parsing() {
        assert_eq!(BridgeMaterial::from_string("wood"), BridgeMaterial::Wood);
        assert_eq!(BridgeMaterial::from_string("STEEL"), BridgeMaterial::Steel);
        assert_eq!(
            BridgeMaterial::from_string("CustomMaterial"),
            BridgeMaterial::Custom("CustomMaterial".to_string())
        );
    }

    #[test]
    fn test_terrain_road_type_creation() {
        let name = AsciiString::from("TestBridge");
        let bridge = TerrainRoadType::new_bridge(name.clone());

        assert_eq!(bridge.name, name);
        assert!(bridge.is_bridge());
        assert!(bridge.can_be_destroyed);
        assert!(bridge.supports_tanks);
        assert!(bridge.is_valid());

        let road_name = AsciiString::from("TestRoad");
        let road = TerrainRoadType::new_road(road_name.clone());

        assert_eq!(road.name, road_name);
        assert!(!road.is_bridge());
        assert!(!road.can_be_destroyed);
        assert_eq!(road.movement_speed_modifier, 1.2);
    }

    #[test]
    fn test_terrain_roads_manager() {
        let mut manager = TerrainRoads::new();
        let bridge_name = AsciiString::from("TestBridge");
        let road_name = AsciiString::from("TestRoad");

        // Create new bridge
        let bridge = manager.new_bridge(bridge_name.clone());
        bridge.material = BridgeMaterial::Steel;
        bridge.max_span = 200.0;

        // Create new road
        let road = manager.new_road(road_name.clone());
        road.movement_speed_modifier = 1.5;

        // Find bridge and road
        let found_bridge = manager.find_bridge(&bridge_name);
        assert!(found_bridge.is_some());
        assert!(matches!(
            found_bridge.unwrap().material,
            BridgeMaterial::Steel
        ));
        assert_eq!(found_bridge.unwrap().max_span, 200.0);

        let found_road = manager.find_road(&road_name);
        assert!(found_road.is_some());
        assert_eq!(found_road.unwrap().movement_speed_modifier, 1.5);

        // Count items
        assert_eq!(manager.get_road_count(), 2);
        assert_eq!(manager.get_bridges().len(), 1);
        assert_eq!(manager.get_roads().len(), 1);
    }

    #[test]
    fn new_road_inherits_default_road_fields_like_cpp() {
        let mut manager = TerrainRoads::new();
        let default = manager.new_road(AsciiString::from("DefaultRoad"));
        default.texture_name = AsciiString::from("DefaultRoadTexture.tga");
        default.width = 22.0;
        default.road_width_in_texture = 96.0;

        let road = manager.new_road(AsciiString::from("DerivedRoad"));

        assert_eq!(road.texture_name.as_str(), "DefaultRoadTexture.tga");
        assert_eq!(road.width, 22.0);
        assert_eq!(road.road_width_in_texture, 96.0);
    }

    #[test]
    fn test_bridge_properties_update() {
        let mut bridge = TerrainRoadType::new_bridge(AsciiString::from("Test"));
        let mut properties = HashMap::new();
        properties.insert("Material".to_string(), "Concrete".to_string());
        properties.insert("Width".to_string(), "15.0".to_string());
        properties.insert("Health".to_string(), "2000.0".to_string());
        properties.insert("CanBeDestroyed".to_string(), "false".to_string());

        bridge.update_from_properties(&properties).unwrap();

        assert!(matches!(bridge.material, BridgeMaterial::Concrete));
        assert_eq!(bridge.width, 15.0);
        assert_eq!(bridge.health, 2000.0);
        assert!(!bridge.can_be_destroyed);
    }

    #[test]
    fn bridge_cpp_numeric_fields_reject_invalid_values() {
        let mut properties = HashMap::new();
        properties.insert("BridgeScale".to_string(), "wide".to_string());
        assert!(IniTerrainBridge::parse_terrain_bridge_block(
            AsciiString::from("BadBridgeScale"),
            properties
        )
        .is_err());

        let mut properties = HashMap::new();
        properties.insert("TransitionEffectsHeight".to_string(), "high".to_string());
        assert!(IniTerrainBridge::parse_terrain_bridge_block(
            AsciiString::from("BadTransitionHeight"),
            properties,
        )
        .is_err());

        let mut properties = HashMap::new();
        properties.insert("NumFXPerType".to_string(), "many".to_string());
        assert!(IniTerrainBridge::parse_terrain_bridge_block(
            AsciiString::from("BadNumFX"),
            properties
        )
        .is_err());
    }

    #[test]
    fn test_terrain_roads_registry_singleton() {
        let handle_a = initialize_terrain_roads();
        let handle_b = initialize_terrain_roads();

        assert!(Arc::ptr_eq(&handle_a, &handle_b));
        assert!(get_terrain_roads().is_some());
    }

    #[test]
    fn test_connection_points() {
        let mut bridge = TerrainRoadType::new_bridge(AsciiString::from("TestBridge"));

        bridge.add_connection_point(0.0, 0.0, 0.0, 0.0);
        bridge.add_connection_point(50.0, 0.0, 0.0, std::f32::consts::PI);

        assert_eq!(bridge.connection_points.len(), 2);
        assert_eq!(bridge.connection_points[0].x, 0.0);
        assert_eq!(bridge.connection_points[1].x, 50.0);
        assert_eq!(
            bridge.connection_points[1].orientation,
            std::f32::consts::PI
        );
    }

    #[test]
    fn test_unit_support() {
        let mut bridge = TerrainRoadType::new_bridge(AsciiString::from("TestBridge"));
        bridge.supports_tanks = false;

        assert!(bridge.can_support_unit("infantry"));
        assert!(bridge.can_support_unit("vehicle"));
        assert!(!bridge.can_support_unit("tank"));
        assert!(!bridge.can_support_unit("aircraft"));
    }

    #[test]
    fn test_parse_bool() {
        assert_eq!(parse_bool("true"), Ok(true));
        assert_eq!(parse_bool("TRUE"), Ok(true));
        assert_eq!(parse_bool("yes"), Ok(true));
        assert_eq!(parse_bool("1"), Ok(true));

        assert_eq!(parse_bool("false"), Ok(false));
        assert_eq!(parse_bool("FALSE"), Ok(false));
        assert_eq!(parse_bool("no"), Ok(false));
        assert_eq!(parse_bool("0"), Ok(false));

        assert!(parse_bool("invalid").is_err());
    }

    #[test]
    fn test_validate_name() {
        assert!(IniTerrainBridge::validate_name(&AsciiString::from(
            "ValidName"
        )));
        assert!(!IniTerrainBridge::validate_name(&AsciiString::from("")));
    }
}
