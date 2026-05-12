////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

//! FILE: ini_road.rs
//! Author: Colin Day, December 2001 (Converted to Rust)
//! Desc: Terrain road and bridge parsing
//!
//! Matches C++ TerrainRoads.h and INITerrainRoad.cpp/INITerrainBridge.cpp
//! Road field parse: TerrainRoads.cpp lines 21-30
//! Bridge field parse: TerrainRoads.cpp lines 34-64

use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::common::ascii_string::AsciiString;
use crate::common::ini::ini::{INIError, INIResult, INI};

/// Result type for terrain road operations
pub type TerrainRoadResult<T> = Result<T, TerrainRoadError>;

/// Errors that can occur during terrain road parsing
#[derive(Debug, Clone, PartialEq)]
pub enum TerrainRoadError {
    InvalidName,
    InvalidType,
    ParseError(String),
    NotFound,
    AlreadyExists,
    InvalidBridgeTower,
    InvalidDamageType,
}

impl std::fmt::Display for TerrainRoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TerrainRoadError::InvalidName => write!(f, "Invalid terrain road name"),
            TerrainRoadError::InvalidType => write!(f, "Invalid terrain road type"),
            TerrainRoadError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            TerrainRoadError::NotFound => write!(f, "Terrain road not found"),
            TerrainRoadError::AlreadyExists => write!(f, "Terrain road already exists"),
            TerrainRoadError::InvalidBridgeTower => write!(f, "Invalid bridge tower type"),
            TerrainRoadError::InvalidDamageType => write!(f, "Invalid damage type"),
        }
    }
}

impl std::error::Error for TerrainRoadError {}

/// Bridge tower positions
/// Matches C++ enum BridgeTowerType from TerrainRoads.h lines 25-32
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BridgeTowerType {
    FromLeft = 0,
    FromRight = 1,
    ToLeft = 2,
    ToRight = 3,
}

impl BridgeTowerType {
    pub const MAX_TOWERS: usize = 4;

    pub fn from_index(idx: usize) -> Option<Self> {
        match idx {
            0 => Some(Self::FromLeft),
            1 => Some(Self::FromRight),
            2 => Some(Self::ToLeft),
            3 => Some(Self::ToRight),
            _ => None,
        }
    }

    pub fn as_index(&self) -> usize {
        *self as usize
    }
}

/// Body damage types
/// Matches C++ BodyDamageType
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BodyDamageType {
    Pristine = 0,
    Damaged = 1,
    ReallyDamaged = 2,
    Rubble = 3,
}

impl BodyDamageType {
    pub const COUNT: usize = 4;

    pub fn from_index(idx: usize) -> Option<Self> {
        match idx {
            0 => Some(Self::Pristine),
            1 => Some(Self::Damaged),
            2 => Some(Self::ReallyDamaged),
            3 => Some(Self::Rubble),
            _ => None,
        }
    }
}

/// RGB Color
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RGBColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl RGBColor {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub fn black() -> Self {
        Self { r: 0, g: 0, b: 0 }
    }
}

/// Maximum bridge body FX
/// Matches C++ MAX_BRIDGE_BODY_FX from TerrainRoads.h line 35
pub const MAX_BRIDGE_BODY_FX: usize = 3;

#[derive(Debug, Clone, Copy)]
struct TransitionSpec {
    damage_transition: bool,
    state: BodyDamageType,
    effect_index: usize,
}

fn parse_body_damage_type_name(token: &str) -> Option<BodyDamageType> {
    let normalized = token.trim().to_ascii_uppercase();
    match normalized.as_str() {
        "PRISTINE" | "BODY_PRISTINE" => Some(BodyDamageType::Pristine),
        "DAMAGED" | "BODY_DAMAGED" => Some(BodyDamageType::Damaged),
        "REALLYDAMAGED" | "REALLY_DAMAGED" | "BODY_REALLYDAMAGED" | "BODY_REALLY_DAMAGED" => {
            Some(BodyDamageType::ReallyDamaged)
        }
        "RUBBLE" | "BODY_RUBBLE" => Some(BodyDamageType::Rubble),
        _ => None,
    }
}

fn parse_transition_spec(value: &str, effect_label: &str) -> TerrainRoadResult<TransitionSpec> {
    let mut transition_kind: Option<bool> = None;
    let mut state: Option<BodyDamageType> = None;
    let mut effect_num: Option<usize> = None;
    let mut effect_found = false;

    for token in value.split_whitespace() {
        let Some((key, raw_val)) = token.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let raw_val = raw_val.trim();

        if key.eq_ignore_ascii_case("Transition") {
            if raw_val.eq_ignore_ascii_case("Damage") {
                transition_kind = Some(true);
            } else if raw_val.eq_ignore_ascii_case("Repair") {
                transition_kind = Some(false);
            } else {
                return Err(TerrainRoadError::ParseError(format!(
                    "Invalid Transition value '{}'",
                    raw_val
                )));
            }
        } else if key.eq_ignore_ascii_case("ToState") {
            state = parse_body_damage_type_name(raw_val);
            if state.is_none() {
                return Err(TerrainRoadError::ParseError(format!(
                    "Invalid ToState value '{}'",
                    raw_val
                )));
            }
        } else if key.eq_ignore_ascii_case("EffectNum") {
            let parsed: i32 = raw_val.parse().map_err(|e| {
                TerrainRoadError::ParseError(format!("EffectNum parse failed '{}': {}", raw_val, e))
            })?;
            if parsed <= 0 || parsed as usize > MAX_BRIDGE_BODY_FX {
                return Err(TerrainRoadError::ParseError(format!(
                    "EffectNum '{}' out of range 1..={}",
                    parsed, MAX_BRIDGE_BODY_FX
                )));
            }
            effect_num = Some((parsed as usize) - 1);
        } else if key.eq_ignore_ascii_case(effect_label) {
            effect_found = !raw_val.is_empty();
        }
    }

    if !effect_found {
        return Err(TerrainRoadError::ParseError(format!(
            "Missing {} field in transition spec '{}'",
            effect_label, value
        )));
    }

    Ok(TransitionSpec {
        damage_transition: transition_kind.ok_or_else(|| {
            TerrainRoadError::ParseError(format!("Missing Transition in '{}'", value))
        })?,
        state: state.ok_or_else(|| {
            TerrainRoadError::ParseError(format!("Missing ToState in '{}'", value))
        })?,
        effect_index: effect_num.ok_or_else(|| {
            TerrainRoadError::ParseError(format!("Missing EffectNum in '{}'", value))
        })?,
    })
}

fn parse_rgb_color_value(value: &str) -> TerrainRoadResult<RGBColor> {
    // Legacy INI format commonly uses "R:100 G:114 B:245".
    let mut r: Option<u8> = None;
    let mut g: Option<u8> = None;
    let mut b: Option<u8> = None;

    for token in value.split_whitespace() {
        if let Some((component, raw_value)) = token.split_once(':') {
            let parsed = raw_value.trim().parse::<u8>().map_err(|err| {
                TerrainRoadError::ParseError(format!(
                    "RadarColor component '{}' parse failed: {}",
                    token, err
                ))
            })?;
            match component.trim().to_ascii_uppercase().as_str() {
                "R" => r = Some(parsed),
                "G" => g = Some(parsed),
                "B" => b = Some(parsed),
                _ => {}
            }
        }
    }

    if let (Some(r), Some(g), Some(b)) = (r, g, b) {
        return Ok(RGBColor::new(r, g, b));
    }

    // Backward compatibility for compact "R:G:B" numeric format.
    let parts: Vec<&str> = value.split(':').collect();
    if parts.len() == 3 {
        let r = parts[0].trim().parse::<u8>().map_err(|err| {
            TerrainRoadError::ParseError(format!(
                "RadarColor R parse failed '{}': {}",
                parts[0], err
            ))
        })?;
        let g = parts[1].trim().parse::<u8>().map_err(|err| {
            TerrainRoadError::ParseError(format!(
                "RadarColor G parse failed '{}': {}",
                parts[1], err
            ))
        })?;
        let b = parts[2].trim().parse::<u8>().map_err(|err| {
            TerrainRoadError::ParseError(format!(
                "RadarColor B parse failed '{}': {}",
                parts[2], err
            ))
        })?;
        return Ok(RGBColor::new(r, g, b));
    }

    Err(TerrainRoadError::ParseError(format!(
        "Invalid RadarColor format '{}'",
        value
    )))
}

fn parse_key_value_tokens(tokens: &[&str]) -> Option<(String, String)> {
    if let Some(eq_idx) = tokens.iter().position(|token| *token == "=") {
        if eq_idx == 0 {
            return None;
        }
        let key = tokens[0..eq_idx].join(" ").trim().to_string();
        let value = tokens[eq_idx + 1..].join(" ").trim().to_string();
        if key.is_empty() {
            return None;
        }
        return Some((key, value));
    }

    if tokens.len() >= 2 {
        let key = tokens[0].trim().to_string();
        let value = tokens[1..].join(" ").trim().to_string();
        if key.is_empty() {
            return None;
        }
        return Some((key, value));
    }

    None
}

fn apply_road_field(road: &mut TerrainRoadType, key: &str, value: &str) -> TerrainRoadResult<()> {
    match key {
        "Texture" => road.texture = AsciiString::from(value),
        "RoadWidth" => {
            road.road_width = value
                .parse()
                .map_err(|e| TerrainRoadError::ParseError(format!("RoadWidth: {}", e)))?;
        }
        "RoadWidthInTexture" => {
            road.road_width_in_texture = value
                .parse()
                .map_err(|e| TerrainRoadError::ParseError(format!("RoadWidthInTexture: {}", e)))?;
        }
        _ => {
            eprintln!("Warning: Unknown road field: {}", key);
        }
    }
    Ok(())
}

fn apply_bridge_field(
    bridge: &mut TerrainRoadType,
    key: &str,
    value: &str,
) -> TerrainRoadResult<()> {
    match key {
        "BridgeScale" => {
            bridge.bridge_scale = value
                .parse()
                .map_err(|e| TerrainRoadError::ParseError(format!("BridgeScale: {}", e)))?;
        }
        "ScaffoldObjectName" => bridge.scaffold_object_name = AsciiString::from(value),
        "ScaffoldSupportObjectName" => {
            bridge.scaffold_support_object_name = AsciiString::from(value)
        }
        "RadarColor" => {
            bridge.radar_color = parse_rgb_color_value(value)?;
        }
        "TransitionEffectsHeight" => {
            bridge.transition_effects_height = value.parse().map_err(|e| {
                TerrainRoadError::ParseError(format!("TransitionEffectsHeight: {}", e))
            })?;
        }
        "NumFXPerType" => {
            bridge.num_fx_per_type = value
                .parse()
                .map_err(|e| TerrainRoadError::ParseError(format!("NumFXPerType: {}", e)))?;
        }
        "BridgeModelName" => bridge.bridge_model_name = AsciiString::from(value),
        "Texture" => bridge.texture = AsciiString::from(value),
        "BridgeModelNameDamaged" => bridge.bridge_model_name_damaged = AsciiString::from(value),
        "TextureDamaged" => bridge.texture_damaged = AsciiString::from(value),
        "BridgeModelNameReallyDamaged" => {
            bridge.bridge_model_name_really_damaged = AsciiString::from(value)
        }
        "TextureReallyDamaged" => bridge.texture_really_damaged = AsciiString::from(value),
        "BridgeModelNameBroken" => bridge.bridge_model_name_broken = AsciiString::from(value),
        "TextureBroken" => bridge.texture_broken = AsciiString::from(value),
        "TowerObjectNameFromLeft" => {
            bridge.tower_object_name[BridgeTowerType::FromLeft.as_index()] =
                AsciiString::from(value)
        }
        "TowerObjectNameFromRight" => {
            bridge.tower_object_name[BridgeTowerType::FromRight.as_index()] =
                AsciiString::from(value)
        }
        "TowerObjectNameToLeft" => {
            bridge.tower_object_name[BridgeTowerType::ToLeft.as_index()] = AsciiString::from(value)
        }
        "TowerObjectNameToRight" => {
            bridge.tower_object_name[BridgeTowerType::ToRight.as_index()] = AsciiString::from(value)
        }
        "DamagedToSound" => {
            bridge.damage_to_sound_string[BodyDamageType::Damaged as usize] =
                AsciiString::from(value)
        }
        "RepairedToSound" => {
            bridge.repaired_to_sound_string[BodyDamageType::Damaged as usize] =
                AsciiString::from(value)
        }
        "TransitionToOCL" | "TransitionToFX" => {
            let effect_label = if key.eq_ignore_ascii_case("TransitionToOCL") {
                "OCL"
            } else {
                "FX"
            };
            let spec = parse_transition_spec(value, effect_label)?;
            let effect_value = value
                .split_whitespace()
                .find_map(|token| {
                    token
                        .split_once(':')
                        .filter(|(name, _)| name.eq_ignore_ascii_case(effect_label))
                        .map(|(_, val)| val.trim())
                })
                .unwrap_or("");
            let effect = AsciiString::from(effect_value);

            if key.eq_ignore_ascii_case("TransitionToOCL") {
                if spec.damage_transition {
                    bridge.damage_to_ocl_string[spec.state as usize][spec.effect_index] = effect;
                } else {
                    bridge.repaired_to_ocl_string[spec.state as usize][spec.effect_index] = effect;
                }
            } else if spec.damage_transition {
                bridge.damage_to_fx_string[spec.state as usize][spec.effect_index] = effect;
            } else {
                bridge.repaired_to_fx_string[spec.state as usize][spec.effect_index] = effect;
            }
        }
        _ => {
            eprintln!("Warning: Unknown bridge field: {}", key);
        }
    }

    Ok(())
}

// Helper to create default 2D arrays of AsciiStrings
fn default_ascii_string_2d() -> [[AsciiString; MAX_BRIDGE_BODY_FX]; BodyDamageType::COUNT] {
    [
        [
            AsciiString::from(""),
            AsciiString::from(""),
            AsciiString::from(""),
        ],
        [
            AsciiString::from(""),
            AsciiString::from(""),
            AsciiString::from(""),
        ],
        [
            AsciiString::from(""),
            AsciiString::from(""),
            AsciiString::from(""),
        ],
        [
            AsciiString::from(""),
            AsciiString::from(""),
            AsciiString::from(""),
        ],
    ]
}

/// Terrain road/bridge type definition
/// Matches C++ TerrainRoadType from TerrainRoads.h lines 40-169
#[derive(Debug, Clone)]
pub struct TerrainRoadType {
    /// Entry name
    pub name: AsciiString,

    /// True if entry is for a bridge
    pub is_bridge: bool,

    /// Unique ID
    pub id: u32,

    // Road-specific fields (from m_terrainRoadFieldParseTable lines 21-30)
    /// Texture filename
    pub texture: AsciiString,

    /// Width of road
    pub road_width: f32,

    /// Width of road in the texture
    pub road_width_in_texture: f32,

    // Bridge-specific fields (from m_terrainBridgeFieldParseTable lines 34-64)
    /// Scale for bridge
    pub bridge_scale: f32,

    /// Scaffold object name
    pub scaffold_object_name: AsciiString,

    /// Scaffold support object name
    pub scaffold_support_object_name: AsciiString,

    /// Color for this bridge on the radar
    pub radar_color: RGBColor,

    /// Model name for bridge
    pub bridge_model_name: AsciiString,

    /// Model name for damaged bridge
    pub bridge_model_name_damaged: AsciiString,

    /// Model name for really damaged bridge
    pub bridge_model_name_really_damaged: AsciiString,

    /// Model name for broken bridge
    pub bridge_model_name_broken: AsciiString,

    /// Texture for damaged bridge
    pub texture_damaged: AsciiString,

    /// Texture for really damaged bridge
    pub texture_really_damaged: AsciiString,

    /// Texture for broken bridge
    pub texture_broken: AsciiString,

    /// Object names for the targetable towers on the bridge [4]
    pub tower_object_name: [AsciiString; BridgeTowerType::MAX_TOWERS],

    // Transition effects for damage/repair
    /// Sounds to play on damage transition [4]
    pub damage_to_sound_string: [AsciiString; BodyDamageType::COUNT],

    /// OCL to play on damage transition [4][3]
    pub damage_to_ocl_string: [[AsciiString; MAX_BRIDGE_BODY_FX]; BodyDamageType::COUNT],

    /// FX to play on damage transition [4][3]
    pub damage_to_fx_string: [[AsciiString; MAX_BRIDGE_BODY_FX]; BodyDamageType::COUNT],

    /// Sounds to play on repair transition [4]
    pub repaired_to_sound_string: [AsciiString; BodyDamageType::COUNT],

    /// OCL to play on repair transition [4][3]
    pub repaired_to_ocl_string: [[AsciiString; MAX_BRIDGE_BODY_FX]; BodyDamageType::COUNT],

    /// FX to play on repair transition [4][3]
    pub repaired_to_fx_string: [[AsciiString; MAX_BRIDGE_BODY_FX]; BodyDamageType::COUNT],

    /// Height at which to play transition effects
    pub transition_effects_height: f32,

    /// For each fx/ocl we will make this many of them on the bridge area
    pub num_fx_per_type: i32,
}

impl TerrainRoadType {
    /// Create a new terrain road type
    pub fn new(name: AsciiString, is_bridge: bool) -> Self {
        Self {
            name,
            is_bridge,
            id: 0,
            texture: AsciiString::from(""),
            road_width: 0.0,
            road_width_in_texture: 0.0,
            bridge_scale: 1.0,
            scaffold_object_name: AsciiString::from(""),
            scaffold_support_object_name: AsciiString::from(""),
            radar_color: RGBColor::black(),
            bridge_model_name: AsciiString::from(""),
            bridge_model_name_damaged: AsciiString::from(""),
            bridge_model_name_really_damaged: AsciiString::from(""),
            bridge_model_name_broken: AsciiString::from(""),
            texture_damaged: AsciiString::from(""),
            texture_really_damaged: AsciiString::from(""),
            texture_broken: AsciiString::from(""),
            tower_object_name: [
                AsciiString::from(""),
                AsciiString::from(""),
                AsciiString::from(""),
                AsciiString::from(""),
            ],
            damage_to_sound_string: [
                AsciiString::from(""),
                AsciiString::from(""),
                AsciiString::from(""),
                AsciiString::from(""),
            ],
            damage_to_ocl_string: default_ascii_string_2d(),
            damage_to_fx_string: default_ascii_string_2d(),
            repaired_to_sound_string: [
                AsciiString::from(""),
                AsciiString::from(""),
                AsciiString::from(""),
                AsciiString::from(""),
            ],
            repaired_to_ocl_string: default_ascii_string_2d(),
            repaired_to_fx_string: default_ascii_string_2d(),
            transition_effects_height: 0.0,
            num_fx_per_type: 0,
        }
    }
}

/// Terrain road collection
/// Matches C++ TerrainRoadCollection from TerrainRoads.h lines 174-203
pub struct TerrainRoadCollection {
    roads: HashMap<AsciiString, TerrainRoadType>,
    bridges: HashMap<AsciiString, TerrainRoadType>,
    road_order: Vec<AsciiString>,
    bridge_order: Vec<AsciiString>,
    id_counter: u32,
}

impl TerrainRoadCollection {
    pub fn new() -> Self {
        Self {
            roads: HashMap::new(),
            bridges: HashMap::new(),
            road_order: Vec::new(),
            bridge_order: Vec::new(),
            id_counter: 0,
        }
    }

    fn next_id(&mut self) -> u32 {
        let id = self.id_counter;
        self.id_counter += 1;
        id
    }

    pub fn find_road(&self, name: &str) -> Option<&TerrainRoadType> {
        self.roads.get(&AsciiString::from(name))
    }

    pub fn find_road_mut(&mut self, name: &str) -> Option<&mut TerrainRoadType> {
        self.roads.get_mut(&AsciiString::from(name))
    }

    pub fn new_road(&mut self, name: AsciiString) -> &mut TerrainRoadType {
        let id = self.next_id();
        let mut road = TerrainRoadType::new(name.clone(), false);
        road.id = id;
        if let Some(default_road) = self.find_road("DefaultRoad") {
            road.texture = default_road.texture.clone();
            road.road_width = default_road.road_width;
            road.road_width_in_texture = default_road.road_width_in_texture;
        }
        self.road_order.retain(|existing| existing != &name);
        self.road_order.insert(0, name.clone());
        self.roads.insert(name.clone(), road);
        self.roads.get_mut(&name).unwrap()
    }

    pub fn find_bridge(&self, name: &str) -> Option<&TerrainRoadType> {
        self.bridges.get(&AsciiString::from(name))
    }

    pub fn find_bridge_mut(&mut self, name: &str) -> Option<&mut TerrainRoadType> {
        self.bridges.get_mut(&AsciiString::from(name))
    }

    pub fn new_bridge(&mut self, name: AsciiString) -> &mut TerrainRoadType {
        let id = self.next_id();
        let mut bridge = TerrainRoadType::new(name.clone(), true);
        bridge.id = id;
        if let Some(default_bridge) = self.find_bridge("DefaultBridge") {
            bridge.texture = default_bridge.texture.clone();
            bridge.bridge_scale = default_bridge.bridge_scale;
            bridge.bridge_model_name = default_bridge.bridge_model_name.clone();
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
            bridge.damage_to_ocl_string = default_bridge.damage_to_ocl_string.clone();
            bridge.damage_to_fx_string = default_bridge.damage_to_fx_string.clone();
            bridge.repaired_to_sound_string = default_bridge.repaired_to_sound_string.clone();
            bridge.repaired_to_ocl_string = default_bridge.repaired_to_ocl_string.clone();
            bridge.repaired_to_fx_string = default_bridge.repaired_to_fx_string.clone();
        }
        self.bridge_order.retain(|existing| existing != &name);
        self.bridge_order.insert(0, name.clone());
        self.bridges.insert(name.clone(), bridge);
        self.bridges.get_mut(&name).unwrap()
    }

    pub fn find_road_or_bridge(&self, name: &str) -> Option<&TerrainRoadType> {
        self.find_road(name).or_else(|| self.find_bridge(name))
    }

    pub fn iter_roads(&self) -> impl Iterator<Item = &TerrainRoadType> {
        self.road_order
            .iter()
            .filter_map(|name| self.roads.get(name))
    }

    pub fn iter_bridges(&self) -> impl Iterator<Item = &TerrainRoadType> {
        self.bridge_order
            .iter()
            .filter_map(|name| self.bridges.get(name))
    }
}

impl Default for TerrainRoadCollection {
    fn default() -> Self {
        Self::new()
    }
}

/// Global terrain road collection
static TERRAIN_ROAD_COLLECTION: OnceCell<RwLock<TerrainRoadCollection>> = OnceCell::new();

/// Get the global terrain road collection
pub fn get_terrain_roads() -> RwLockReadGuard<'static, TerrainRoadCollection> {
    TERRAIN_ROAD_COLLECTION
        .get_or_init(|| RwLock::new(TerrainRoadCollection::new()))
        .read()
        .unwrap()
}

/// Get mutable access to the global terrain road collection
pub fn get_terrain_roads_mut() -> RwLockWriteGuard<'static, TerrainRoadCollection> {
    TERRAIN_ROAD_COLLECTION
        .get_or_init(|| RwLock::new(TerrainRoadCollection::new()))
        .write()
        .unwrap()
}

/// Parse a terrain road definition from INI
/// Matches C++ INI::parseTerrainRoadDefinition from INITerrainRoad.cpp lines 15-46
/// Field parse table from TerrainRoads.cpp lines 21-30
pub fn parse_terrain_road_definition(
    name: &str,
    properties: &HashMap<String, String>,
) -> TerrainRoadResult<TerrainRoadType> {
    let mut road = TerrainRoadType::new(AsciiString::from(name), false);

    for (key, value) in properties {
        apply_road_field(&mut road, key.as_str(), value.as_str())?;
    }

    Ok(road)
}

/// Parse a terrain bridge definition from INI
/// Matches C++ INI::parseTerrainBridgeDefinition from INITerrainBridge.cpp lines 15-46
/// Field parse table from TerrainRoads.cpp lines 34-64
pub fn parse_terrain_bridge_definition(
    name: &str,
    properties: &HashMap<String, String>,
) -> TerrainRoadResult<TerrainRoadType> {
    let mut bridge = TerrainRoadType::new(AsciiString::from(name), true);

    for (key, value) in properties {
        apply_bridge_field(&mut bridge, key.as_str(), value.as_str())?;
    }

    Ok(bridge)
}

/// Parse a `Road` block directly from the active INI stream.
pub fn parse_terrain_road_definition_from_ini(ini: &mut INI) -> INIResult<()> {
    let tokens = ini.get_line_tokens();
    let name = tokens
        .iter()
        .skip(1)
        .find(|token| **token != "=")
        .ok_or(INIError::InvalidData)?
        .to_string();

    let mut roads = get_terrain_roads_mut();
    if roads.find_road(&name).is_some() || roads.find_bridge(&name).is_some() {
        return Err(INIError::InvalidData);
    }

    let road = roads.new_road(AsciiString::from(name.as_str()));
    loop {
        ini.read_line()?;
        if ini.is_eof() {
            return Err(INIError::MissingEndToken);
        }

        let tokens = ini.get_line_tokens();
        let Some(first) = tokens.first().copied() else {
            continue;
        };
        if first.eq_ignore_ascii_case("End") {
            break;
        }

        if let Some((key, value)) = parse_key_value_tokens(&tokens) {
            apply_road_field(road, key.as_str(), value.as_str())
                .map_err(|_| INIError::InvalidData)?;
        }
    }

    Ok(())
}

/// Parse a `Bridge` block directly from the active INI stream.
pub fn parse_terrain_bridge_definition_from_ini(ini: &mut INI) -> INIResult<()> {
    let tokens = ini.get_line_tokens();
    let name = tokens
        .iter()
        .skip(1)
        .find(|token| **token != "=")
        .ok_or(INIError::InvalidData)?
        .to_string();

    let mut roads = get_terrain_roads_mut();
    if roads.find_bridge(&name).is_some() || roads.find_road(&name).is_some() {
        return Err(INIError::InvalidData);
    }

    let bridge = roads.new_bridge(AsciiString::from(name.as_str()));
    loop {
        ini.read_line()?;
        if ini.is_eof() {
            return Err(INIError::MissingEndToken);
        }

        let tokens = ini.get_line_tokens();
        let Some(first) = tokens.first().copied() else {
            continue;
        };
        if first.eq_ignore_ascii_case("End") {
            break;
        }

        if let Some((key, value)) = parse_key_value_tokens(&tokens) {
            apply_bridge_field(bridge, key.as_str(), value.as_str())
                .map_err(|_| INIError::InvalidData)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_tower_type() {
        assert_eq!(
            BridgeTowerType::from_index(0),
            Some(BridgeTowerType::FromLeft)
        );
        assert_eq!(
            BridgeTowerType::from_index(3),
            Some(BridgeTowerType::ToRight)
        );
        assert_eq!(BridgeTowerType::from_index(4), None);
    }

    #[test]
    fn test_terrain_road_creation() {
        let road = TerrainRoadType::new(AsciiString::from("TestRoad"), false);
        assert_eq!(road.name.to_str(), "TestRoad");
        assert!(!road.is_bridge);
        assert_eq!(road.road_width, 0.0);
    }

    #[test]
    fn test_terrain_bridge_creation() {
        let bridge = TerrainRoadType::new(AsciiString::from("TestBridge"), true);
        assert_eq!(bridge.name.to_str(), "TestBridge");
        assert!(bridge.is_bridge);
        assert_eq!(bridge.bridge_scale, 1.0);
    }

    #[test]
    fn test_terrain_road_collection() {
        let mut collection = TerrainRoadCollection::new();

        collection.new_road(AsciiString::from("Road1"));
        collection.new_bridge(AsciiString::from("Bridge1"));

        assert!(collection.find_road("Road1").is_some());
        assert!(collection.find_bridge("Bridge1").is_some());
        assert!(collection.find_road("Bridge1").is_none());
        assert!(collection.find_bridge("Road1").is_none());
    }

    #[test]
    fn test_terrain_road_collection_uses_cpp_head_list_order() {
        let mut collection = TerrainRoadCollection::new();

        collection.new_road(AsciiString::from("Road1"));
        collection.new_road(AsciiString::from("Road2"));
        collection.new_bridge(AsciiString::from("Bridge1"));
        collection.new_bridge(AsciiString::from("Bridge2"));

        let road_names: Vec<&str> = collection
            .iter_roads()
            .map(|road| road.name.to_str())
            .collect();
        let bridge_names: Vec<&str> = collection
            .iter_bridges()
            .map(|bridge| bridge.name.to_str())
            .collect();

        assert_eq!(road_names, vec!["Road2", "Road1"]);
        assert_eq!(bridge_names, vec!["Bridge2", "Bridge1"]);
    }

    #[test]
    fn test_terrain_road_collection_copies_cpp_default_road_fields() {
        let mut collection = TerrainRoadCollection::new();

        let default = collection.new_road(AsciiString::from("DefaultRoad"));
        default.texture = AsciiString::from("default_road.tga");
        default.road_width = 23.0;
        default.road_width_in_texture = 17.0;

        let road = collection.new_road(AsciiString::from("CityRoad"));

        assert_eq!(road.texture.to_str(), "default_road.tga");
        assert_eq!(road.road_width, 23.0);
        assert_eq!(road.road_width_in_texture, 17.0);
        assert_eq!(road.name.to_str(), "CityRoad");
    }

    #[test]
    fn test_terrain_road_collection_copies_cpp_default_bridge_fields() {
        let mut collection = TerrainRoadCollection::new();

        let default = collection.new_bridge(AsciiString::from("DefaultBridge"));
        default.texture = AsciiString::from("default_bridge.tga");
        default.bridge_scale = 2.5;
        default.bridge_model_name = AsciiString::from("default_bridge.w3d");
        default.texture_broken = AsciiString::from("default_bridge_broken.tga");
        default.transition_effects_height = 11.0;
        default.num_fx_per_type = 3;
        default.damage_to_fx_string[BodyDamageType::Damaged as usize][0] =
            AsciiString::from("DefaultBridgeDamageFX");

        let bridge = collection.new_bridge(AsciiString::from("RiverBridge"));

        assert_eq!(bridge.texture.to_str(), "default_bridge.tga");
        assert_eq!(bridge.bridge_scale, 2.5);
        assert_eq!(bridge.bridge_model_name.to_str(), "default_bridge.w3d");
        assert_eq!(bridge.texture_broken.to_str(), "default_bridge_broken.tga");
        assert_eq!(bridge.transition_effects_height, 11.0);
        assert_eq!(bridge.num_fx_per_type, 3);
        assert_eq!(
            bridge.damage_to_fx_string[BodyDamageType::Damaged as usize][0].to_str(),
            "DefaultBridgeDamageFX"
        );
        assert_eq!(bridge.name.to_str(), "RiverBridge");
    }

    #[test]
    fn test_parse_terrain_road() {
        let mut props = HashMap::new();
        props.insert("Texture".to_string(), "road.tga".to_string());
        props.insert("RoadWidth".to_string(), "10.0".to_string());
        props.insert("RoadWidthInTexture".to_string(), "8.0".to_string());

        let result = parse_terrain_road_definition("TestRoad", &props);
        assert!(result.is_ok());

        let road = result.unwrap();
        assert_eq!(road.texture.to_str(), "road.tga");
        assert_eq!(road.road_width, 10.0);
        assert_eq!(road.road_width_in_texture, 8.0);
    }

    #[test]
    fn test_parse_terrain_bridge() {
        let mut props = HashMap::new();
        props.insert("BridgeModelName".to_string(), "bridge.w3d".to_string());
        props.insert("BridgeScale".to_string(), "1.5".to_string());
        props.insert("RadarColor".to_string(), "255:128:64".to_string());

        let result = parse_terrain_bridge_definition("TestBridge", &props);
        assert!(result.is_ok());

        let bridge = result.unwrap();
        assert_eq!(bridge.bridge_model_name.to_str(), "bridge.w3d");
        assert_eq!(bridge.bridge_scale, 1.5);
        assert_eq!(bridge.radar_color.r, 255);
        assert_eq!(bridge.radar_color.g, 128);
        assert_eq!(bridge.radar_color.b, 64);
    }

    #[test]
    fn test_parse_bridge_radar_color_component_format() {
        let mut props = HashMap::new();
        props.insert("RadarColor".to_string(), "R:100 G:114 B:245".to_string());

        let bridge = parse_terrain_bridge_definition("TestBridge", &props).expect("bridge parse");
        assert_eq!(bridge.radar_color, RGBColor::new(100, 114, 245));
    }

    #[test]
    fn test_parse_bridge_transition_effects() {
        let mut props = HashMap::new();
        props.insert(
            "TransitionToFX".to_string(),
            "Transition:Damage ToState:DAMAGED EffectNum:2 FX:BridgeHitFX".to_string(),
        );
        props.insert(
            "TransitionToOCL".to_string(),
            "Transition:Repair ToState:REALLYDAMAGED EffectNum:1 OCL:BridgeRepairOCL".to_string(),
        );

        let bridge = parse_terrain_bridge_definition("TestBridge", &props).expect("bridge parse");

        assert_eq!(
            bridge.damage_to_fx_string[BodyDamageType::Damaged as usize][1].to_str(),
            "BridgeHitFX"
        );
        assert_eq!(
            bridge.repaired_to_ocl_string[BodyDamageType::ReallyDamaged as usize][0].to_str(),
            "BridgeRepairOCL"
        );
    }

    #[test]
    fn test_repeated_transition_fields_do_not_overwrite_previous_effect_slots() {
        let mut bridge = TerrainRoadType::new(AsciiString::from("BridgeX"), true);
        apply_bridge_field(
            &mut bridge,
            "TransitionToFX",
            "Transition:Damage ToState:DAMAGED EffectNum:1 FX:FX_A",
        )
        .expect("first transition");
        apply_bridge_field(
            &mut bridge,
            "TransitionToFX",
            "Transition:Damage ToState:DAMAGED EffectNum:2 FX:FX_B",
        )
        .expect("second transition");

        assert_eq!(
            bridge.damage_to_fx_string[BodyDamageType::Damaged as usize][0].to_str(),
            "FX_A"
        );
        assert_eq!(
            bridge.damage_to_fx_string[BodyDamageType::Damaged as usize][1].to_str(),
            "FX_B"
        );
    }
}
