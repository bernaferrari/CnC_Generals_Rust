////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

//! FILE: ini_terrain.rs
//! Author: Colin Day, December 2001 (Converted to Rust)
//! Desc:   Terrain type INI loading

use crate::common::ascii_string::AsciiString;
use crate::debug_assert_crash;
use log::{debug, trace};
use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Result type for terrain parsing operations
pub type TerrainResult<T> = Result<T, TerrainError>;

/// Errors that can occur during terrain parsing
#[derive(Debug, Clone, PartialEq)]
pub enum TerrainError {
    InvalidName,
    InvalidType,
    AllocationError,
    ParseError(String),
    NotFound,
}

impl std::fmt::Display for TerrainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TerrainError::InvalidName => write!(f, "Invalid terrain name"),
            TerrainError::InvalidType => write!(f, "Invalid terrain type"),
            TerrainError::AllocationError => write!(f, "Failed to allocate terrain type"),
            TerrainError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            TerrainError::NotFound => write!(f, "Terrain not found"),
        }
    }
}

impl std::error::Error for TerrainError {}

/// Terrain surface types
#[derive(Debug, Clone, PartialEq)]
pub enum TerrainSurface {
    Grass,
    Dirt,
    Sand,
    Rock,
    Snow,
    Water,
    Pavement,
    Concrete,
    Metal,
    Wood,
    Custom(String),
}

impl TerrainSurface {
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "grass" => Self::Grass,
            "dirt" => Self::Dirt,
            "sand" => Self::Sand,
            "rock" => Self::Rock,
            "snow" => Self::Snow,
            "water" => Self::Water,
            "pavement" => Self::Pavement,
            "concrete" => Self::Concrete,
            "metal" => Self::Metal,
            "wood" => Self::Wood,
            _ => Self::Custom(s.to_string()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Grass => "Grass",
            Self::Dirt => "Dirt",
            Self::Sand => "Sand",
            Self::Rock => "Rock",
            Self::Snow => "Snow",
            Self::Water => "Water",
            Self::Pavement => "Pavement",
            Self::Concrete => "Concrete",
            Self::Metal => "Metal",
            Self::Wood => "Wood",
            Self::Custom(name) => name,
        }
    }

    pub fn from_class_name(class: &str) -> Option<Self> {
        let normalized_owned = class.trim().to_ascii_uppercase();
        let normalized = normalized_owned.trim_end_matches('_');

        match normalized {
            "ASPHALT" | "CONCRETE" | "RESIDENTIAL" => Some(Self::Pavement),
            "BEACH_PARK" | "BEACH_TROPICAL" | "SAND" | "SAND_ACCENT" | "DESERT_1" | "DESERT_2"
            | "DESERT_3" | "DESERT_DRY" | "DESERT_LIVE" => Some(Self::Sand),
            "DIRT" | "FIELD" | "GRASS" | "GRASS_ACCENT" | "GRASS_COBBLESTONE" => Some(Self::Grass),
            "CLIFF" | "MOUNTAIN_RUGGED" | "ROCK" | "ROCK_ACCENT" => Some(Self::Rock),
            "SNOW_1" | "SNOW_2" | "SNOW_3" | "SNOW_FLAT" | "SNOW_RUGGED" => Some(Self::Snow),
            "WATER" => Some(Self::Water),
            "WOOD" => Some(Self::Wood),
            _ => None,
        }
    }
}

/// Terrain movement modifiers for different unit types
#[derive(Debug, Clone)]
pub struct MovementModifiers {
    pub infantry_speed: f32,
    pub vehicle_speed: f32,
    pub tank_speed: f32,
    pub aircraft_speed: f32,
    pub naval_speed: f32,
    pub can_pass_infantry: bool,
    pub can_pass_vehicle: bool,
    pub can_pass_tank: bool,
    pub can_pass_aircraft: bool,
    pub can_pass_naval: bool,
}

impl Default for MovementModifiers {
    fn default() -> Self {
        Self {
            infantry_speed: 1.0,
            vehicle_speed: 1.0,
            tank_speed: 1.0,
            aircraft_speed: 1.0,
            naval_speed: 1.0,
            can_pass_infantry: true,
            can_pass_vehicle: true,
            can_pass_tank: true,
            can_pass_aircraft: true,
            can_pass_naval: false, // Most terrain doesn't allow naval units
        }
    }
}

/// Terrain type definition
#[derive(Debug, Clone)]
pub struct TerrainType {
    pub name: AsciiString,
    pub surface_type: TerrainSurface,
    pub terrain_class: AsciiString,
    pub movement_modifiers: MovementModifiers,
    pub texture_name: AsciiString,
    pub blend_edge_texture: bool,
    pub restrict_construction: bool,
    pub normal_map: AsciiString,
    pub detail_texture: AsciiString,
    pub sound_effect: AsciiString,
    pub particle_effect: AsciiString,
    pub traction: f32,
    pub friction: f32,
    pub bounce: f32,
    pub hardness: f32,
    pub is_buildable: bool,
    pub is_harvestable: bool,
    pub resource_type: AsciiString,
    pub resource_amount: u32,
    pub minimap_color: (u8, u8, u8),
    pub properties: HashMap<String, String>,
}

impl TerrainType {
    pub fn new(name: AsciiString) -> Self {
        Self {
            name,
            surface_type: TerrainSurface::Grass,
            terrain_class: AsciiString::from("NONE"),
            movement_modifiers: MovementModifiers::default(),
            texture_name: AsciiString::from(""),
            blend_edge_texture: false,
            restrict_construction: false,
            normal_map: AsciiString::from(""),
            detail_texture: AsciiString::from(""),
            sound_effect: AsciiString::from(""),
            particle_effect: AsciiString::from(""),
            traction: 1.0,
            friction: 1.0,
            bounce: 0.0,
            hardness: 1.0,
            is_buildable: true,
            is_harvestable: false,
            resource_type: AsciiString::from(""),
            resource_amount: 0,
            minimap_color: (0, 255, 0), // Default green
            properties: HashMap::new(),
        }
    }

    /// Get the field parse table for this terrain type
    pub fn get_field_parse(
        &self,
    ) -> Vec<(
        &'static str,
        fn(&str) -> Result<Box<dyn std::any::Any>, String>,
    )> {
        vec![
            ("Texture", |value| {
                Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
            }),
            ("BlendEdges", |value| {
                parse_bool(value)
                    .map(|b| Box::new(b) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse BlendEdges: {}", e))
            }),
            ("Class", |value| {
                parse_terrain_class(value)
                    .map(|class| Box::new(class) as Box<dyn std::any::Any>)
                    .map_err(|e| e.to_string())
            }),
            ("RestrictConstruction", |value| {
                parse_bool(value)
                    .map(|b| Box::new(b) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse RestrictConstruction: {}", e))
            }),
        ]
    }

    /// Update terrain type from properties
    pub fn update_from_properties(
        &mut self,
        properties: &HashMap<String, String>,
    ) -> TerrainResult<()> {
        self.properties.extend(properties.clone());

        for (key, value) in properties {
            match key.as_str() {
                "Class" => {
                    self.terrain_class = parse_terrain_class(value)?;
                    if let Some(surface) = TerrainSurface::from_class_name(value) {
                        self.surface_type = surface;
                    }
                }
                "Texture" => {
                    self.texture_name = AsciiString::from(value);
                }
                "BlendEdges" => {
                    self.blend_edge_texture =
                        parse_bool(value).map_err(TerrainError::ParseError)?;
                }
                "RestrictConstruction" => {
                    self.restrict_construction =
                        parse_bool(value).map_err(TerrainError::ParseError)?;
                }
                _ => {
                    return Err(TerrainError::ParseError(format!(
                        "Unknown terrain field: {}",
                        key
                    )));
                }
            }
        }
        Ok(())
    }

    pub fn get_name(&self) -> &AsciiString {
        &self.name
    }

    pub fn is_valid(&self) -> bool {
        !self.name.is_empty()
    }

    pub fn can_unit_pass(&self, unit_type: &str) -> bool {
        match unit_type.to_lowercase().as_str() {
            "infantry" => self.movement_modifiers.can_pass_infantry,
            "vehicle" => self.movement_modifiers.can_pass_vehicle,
            "tank" => self.movement_modifiers.can_pass_tank,
            "aircraft" => self.movement_modifiers.can_pass_aircraft,
            "naval" => self.movement_modifiers.can_pass_naval,
            _ => false,
        }
    }

    pub fn get_speed_modifier(&self, unit_type: &str) -> f32 {
        match unit_type.to_lowercase().as_str() {
            "infantry" => self.movement_modifiers.infantry_speed,
            "vehicle" => self.movement_modifiers.vehicle_speed,
            "tank" => self.movement_modifiers.tank_speed,
            "aircraft" => self.movement_modifiers.aircraft_speed,
            "naval" => self.movement_modifiers.naval_speed,
            _ => 1.0,
        }
    }
}

/// Terrain types manager - manages all terrain type definitions
#[derive(Debug, Clone)]
pub struct TerrainTypes {
    terrain_types: HashMap<String, TerrainType>,
    terrain_order: Vec<String>,
}

impl TerrainTypes {
    pub fn new() -> Self {
        Self {
            terrain_types: HashMap::new(),
            terrain_order: Vec::new(),
        }
    }

    /// Find a terrain type by name
    pub fn find_terrain(&self, name: &AsciiString) -> Option<&TerrainType> {
        self.terrain_types.get(name.as_str())
    }

    /// Find a mutable terrain type by name
    pub fn find_terrain_mut(&mut self, name: &AsciiString) -> Option<&mut TerrainType> {
        self.terrain_types.get_mut(name.as_str())
    }

    /// Create a new terrain type
    pub fn new_terrain(&mut self, name: AsciiString) -> &mut TerrainType {
        let key = name.as_str().to_string();
        let terrain_type = if let Some(default_terrain) = self.terrain_types.get("DefaultTerrain") {
            let mut terrain_type = default_terrain.clone();
            terrain_type.name = name;
            terrain_type
        } else {
            TerrainType::new(name)
        };

        if !self.terrain_types.contains_key(&key) {
            self.terrain_order.insert(0, key.clone());
        }
        self.terrain_types.insert(key.clone(), terrain_type);
        self.terrain_types.get_mut(&key).unwrap()
    }

    /// Get or create a terrain type
    pub fn get_or_create_terrain(&mut self, name: &AsciiString) -> &mut TerrainType {
        if !self.terrain_types.contains_key(name.as_str()) {
            self.new_terrain(name.clone());
        }
        self.terrain_types.get_mut(name.as_str()).unwrap()
    }

    /// Register a terrain type
    pub fn register_terrain(&mut self, terrain_type: TerrainType) {
        let name = terrain_type.name.as_str().to_string();
        if !self.terrain_types.contains_key(&name) {
            self.terrain_order.insert(0, name.clone());
        }
        self.terrain_types.insert(name, terrain_type);
    }

    /// Register parsed terrain properties with C++ default inheritance.
    pub fn register_terrain_properties(
        &mut self,
        name: AsciiString,
        properties: &HashMap<String, String>,
    ) -> TerrainResult<()> {
        if name.is_empty() {
            return Err(TerrainError::InvalidName);
        }

        let terrain_type = self.get_or_create_terrain(&name);
        terrain_type.update_from_properties(properties)?;
        Ok(())
    }

    /// Get all terrain type names
    pub fn get_terrain_names(&self) -> Vec<&String> {
        self.terrain_order.iter().collect()
    }

    /// Get terrain types by surface
    pub fn get_terrains_by_surface(&self, surface: &TerrainSurface) -> Vec<&TerrainType> {
        self.terrain_order
            .iter()
            .filter_map(|name| self.terrain_types.get(name))
            .filter(|t| &t.surface_type == surface)
            .collect()
    }

    /// Remove a terrain type
    pub fn remove_terrain(&mut self, name: &AsciiString) -> bool {
        let removed = self.terrain_types.remove(name.as_str()).is_some();
        if removed {
            self.terrain_order
                .retain(|terrain_name| terrain_name != name.as_str());
        }
        removed
    }

    /// Clear all terrain types
    pub fn clear(&mut self) {
        self.terrain_types.clear();
        self.terrain_order.clear();
    }

    /// Get terrain type count
    pub fn get_terrain_count(&self) -> usize {
        self.terrain_types.len()
    }
}

impl Default for TerrainTypes {
    fn default() -> Self {
        Self::new()
    }
}

/// Global terrain types registry (thread-safe)
static TERRAIN_TYPES: OnceCell<Arc<RwLock<TerrainTypes>>> = OnceCell::new();

/// Ensure the terrain registry exists and return a handle to it
pub fn initialize_terrain_types() -> Arc<RwLock<TerrainTypes>> {
    TERRAIN_TYPES
        .get_or_init(|| Arc::new(RwLock::new(TerrainTypes::new())))
        .clone()
}

/// Get the terrain registry if it has already been initialized
pub fn get_terrain_types() -> Option<Arc<RwLock<TerrainTypes>>> {
    TERRAIN_TYPES.get().cloned()
}

/// Parse a boolean value from string
pub fn parse_bool(value: &str) -> Result<bool, String> {
    match value.trim().to_lowercase().as_str() {
        "yes" => Ok(true),
        "no" => Ok(false),
        _ => Err(format!("Invalid boolean value: {}", value)),
    }
}

const TERRAIN_TYPE_NAMES: &[&str] = &[
    "NONE",
    "DESERT_1",
    "DESERT_2",
    "DESERT_3",
    "EASTERN_EUROPE_1",
    "EASTERN_EUROPE_2",
    "EASTERN_EUROPE_3",
    "SWISS_1",
    "SWISS_2",
    "SWISS_3",
    "SNOW_1",
    "SNOW_2",
    "SNOW_3",
    "DIRT",
    "GRASS",
    "TRANSITION",
    "ROCK",
    "SAND",
    "CLIFF",
    "WOOD",
    "BLEND_EDGE",
    "DESERT_LIVE",
    "DESERT_DRY",
    "SAND_ACCENT",
    "BEACH_TROPICAL",
    "BEACH_PARK",
    "MOUNTAIN_RUGGED",
    "GRASS_COBBLESTONE",
    "GRASS_ACCENT",
    "RESIDENTIAL",
    "SNOW_RUGGED",
    "SNOW_FLAT",
    "FIELD",
    "ASPHALT",
    "CONCRETE",
    "CHINA",
    "ROCK_ACCENT",
    "URBAN",
];

fn parse_terrain_class(value: &str) -> TerrainResult<AsciiString> {
    let token = value.trim();
    TERRAIN_TYPE_NAMES
        .iter()
        .find(|name| name.eq_ignore_ascii_case(token))
        .map(|name| AsciiString::from(*name))
        .ok_or_else(|| TerrainError::ParseError(format!("Invalid terrain class: {}", value)))
}

/// Parse RGB color from string (format: R G B or R,G,B)
pub fn parse_color_rgb(value: &str) -> Result<(u8, u8, u8), String> {
    let parts: Vec<&str> = if value.contains(',') {
        value.split(',').collect()
    } else {
        value.split_whitespace().collect()
    };

    if parts.len() != 3 {
        return Err(format!("Invalid color format: {}", value));
    }

    let r = parts[0]
        .trim()
        .parse::<u8>()
        .map_err(|_| format!("Invalid red component: {}", parts[0]))?;
    let g = parts[1]
        .trim()
        .parse::<u8>()
        .map_err(|_| format!("Invalid green component: {}", parts[1]))?;
    let b = parts[2]
        .trim()
        .parse::<u8>()
        .map_err(|_| format!("Invalid blue component: {}", parts[2]))?;

    Ok((r, g, b))
}

/// INI parsing functions for terrain
pub struct IniTerrain;

impl IniTerrain {
    /// Parse terrain definition - equivalent to INI::parseTerrainDefinition
    pub fn parse_terrain_definition(name: AsciiString) -> TerrainResult<()> {
        // Validate name
        if name.is_empty() {
            return Err(TerrainError::InvalidName);
        }

        // Fetch terrain registry (lazily creates it on first use)
        let terrain_types = initialize_terrain_types();
        let mut terrain_types = terrain_types.write();

        // Find existing terrain type or create new one
        let terrain_type = if terrain_types.find_terrain(&name).is_some() {
            terrain_types
                .find_terrain_mut(&name)
                .expect("Terrain should exist after lookup")
        } else {
            terrain_types.new_terrain(name.clone())
        };

        // Sanity check
        debug_assert_crash!(
            terrain_type.is_valid(),
            "Unable to allocate terrain type '{}'",
            name.as_str()
        );

        // In the original C++, this would call:
        // ini->initFromINI(terrainType, terrainType->getFieldParse());
        println!("Parsing terrain definition for: {}", name.as_str());

        Ok(())
    }

    /// Parse a complete terrain block from INI data
    pub fn parse_terrain_block(
        name: AsciiString,
        properties: HashMap<String, String>,
    ) -> TerrainResult<TerrainType> {
        // Validate name
        if name.is_empty() {
            return Err(TerrainError::InvalidName);
        }

        // Create terrain type
        let mut terrain_type = TerrainType::new(name);

        // Update terrain type from properties
        terrain_type.update_from_properties(&properties)?;

        // Validate terrain type
        if !terrain_type.is_valid() {
            return Err(TerrainError::ParseError(
                "Invalid terrain type configuration".to_string(),
            ));
        }

        Ok(terrain_type)
    }

    /// Register a terrain type
    pub fn register_terrain_type(terrain_type: TerrainType) -> TerrainResult<()> {
        let terrain_types = initialize_terrain_types();
        if terrain_type.properties.is_empty() {
            terrain_types.write().register_terrain(terrain_type);
        } else {
            let name = terrain_type.name.clone();
            terrain_types
                .write()
                .register_terrain_properties(name, &terrain_type.properties)?;
        }
        Ok(())
    }

    /// Find a terrain type by name
    pub fn find_terrain_type_by_name(name: &AsciiString) -> Option<TerrainType> {
        get_terrain_types()
            .and_then(|terrain_types| terrain_types.read().find_terrain(name).cloned())
    }

    /// Validate terrain name format
    pub fn validate_name(name: &AsciiString) -> bool {
        !name.is_empty() && name.len() < 128 // Reasonable length limit
    }
}

/// Token parser for extracting terrain data from INI tokens
pub struct TerrainTokenParser;

impl TerrainTokenParser {
    /// Extract the next terrain name token
    pub fn get_next_name(token: &str) -> TerrainResult<AsciiString> {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            return Err(TerrainError::InvalidName);
        }

        Ok(AsciiString::from(trimmed))
    }

    /// Parse a property line (key = value)
    pub fn parse_property_line(line: &str) -> TerrainResult<(String, String)> {
        if let Some(eq_pos) = line.find('=') {
            let key = line[..eq_pos].trim().to_string();
            let value = line[eq_pos + 1..].trim().to_string();
            Ok((key, value))
        } else {
            Err(TerrainError::ParseError(format!(
                "Invalid property line format: {}",
                line
            )))
        }
    }
}

fn parse_terrain_file_into(target: &mut TerrainTypes, path: &Path) -> TerrainResult<()> {
    if !path.exists() {
        trace!("Terrain INI '{}' not found; skipping", path.display());
        return Ok(());
    }

    let file = File::open(path).map_err(|err| {
        TerrainError::ParseError(format!(
            "Failed to open terrain INI '{}': {}",
            path.display(),
            err
        ))
    })?;
    let reader = BufReader::new(file);

    let before_count = target.get_terrain_count();
    let mut current_name: Option<AsciiString> = None;
    let mut properties: HashMap<String, String> = HashMap::new();

    for (line_index, line_result) in reader.lines().enumerate() {
        let mut line = line_result.map_err(|err| {
            TerrainError::ParseError(format!(
                "Failed to read terrain INI '{}': {}",
                path.display(),
                err
            ))
        })?;

        if let Some(comment_pos) = line.find(';') {
            line.truncate(comment_pos);
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.eq_ignore_ascii_case("END") {
            if let Some(name) = current_name.take() {
                let props = std::mem::take(&mut properties);
                target.register_terrain_properties(name, &props)?;
            } else {
                trace!(
                    "Encountered 'End' without active terrain block in '{}' (line {})",
                    path.display(),
                    line_index + 1
                );
            }
            continue;
        }

        if trimmed.starts_with('[') {
            if let Some(name) = current_name.take() {
                return Err(TerrainError::ParseError(format!(
                    "Terrain block '{}' missing End before section '{}' (line {})",
                    name.as_str(),
                    trimmed,
                    line_index + 1
                )));
            }
            properties.clear();
            continue;
        }

        if trimmed.len() >= 7
            && trimmed[..7].eq_ignore_ascii_case("terrain")
            && trimmed
                .chars()
                .nth(7)
                .map(|ch| ch.is_whitespace())
                .unwrap_or(false)
        {
            if let Some(name) = current_name.take() {
                return Err(TerrainError::ParseError(format!(
                    "Terrain block '{}' missing End before '{}' (line {})",
                    name.as_str(),
                    trimmed,
                    line_index + 1
                )));
            }

            let raw_name = trimmed[7..].trim();
            if raw_name.is_empty() {
                return Err(TerrainError::ParseError(format!(
                    "Terrain block missing name in '{}' (line {})",
                    path.display(),
                    line_index + 1
                )));
            }

            current_name = Some(TerrainTokenParser::get_next_name(raw_name)?);
            properties.clear();
            continue;
        }

        if current_name.is_none() {
            continue;
        }

        match TerrainTokenParser::parse_property_line(trimmed) {
            Ok((key, value)) => {
                properties.insert(key, value);
            }
            Err(err) => {
                return Err(TerrainError::ParseError(format!(
                    "Invalid terrain property in '{}' (line {}): {}",
                    path.display(),
                    line_index + 1,
                    err
                )));
            }
        }
    }

    if let Some(name) = current_name.take() {
        return Err(TerrainError::ParseError(format!(
            "Terrain block '{}' missing End in '{}'",
            name.as_str(),
            path.display()
        )));
    }

    let after_count = target.get_terrain_count();
    debug!(
        "Loaded {} terrain definitions from '{}'",
        after_count.saturating_sub(before_count),
        path.display()
    );

    Ok(())
}

pub fn load_terrain_definitions(paths: &[PathBuf]) -> TerrainResult<usize> {
    if paths.is_empty() {
        return Ok(get_terrain_types()
            .map(|handle| handle.read().get_terrain_count())
            .unwrap_or(0));
    }

    let mut combined = TerrainTypes::new();
    for path in paths {
        parse_terrain_file_into(&mut combined, path)?;
    }

    let registry = initialize_terrain_types();
    let mut guard = registry.write();
    *guard = combined;

    let count = guard.get_terrain_count();
    debug!("Terrain registry initialised with {} entries", count);

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terrain_surface_parsing() {
        assert_eq!(TerrainSurface::from_string("grass"), TerrainSurface::Grass);
        assert_eq!(TerrainSurface::from_string("WATER"), TerrainSurface::Water);
        assert_eq!(
            TerrainSurface::from_string("CustomSurface"),
            TerrainSurface::Custom("CustomSurface".to_string())
        );
    }

    #[test]
    fn test_terrain_type_creation() {
        let name = AsciiString::from("TestTerrain");
        let terrain_type = TerrainType::new(name.clone());

        assert_eq!(terrain_type.name, name);
        assert!(terrain_type.is_buildable);
        assert!(!terrain_type.is_harvestable);
        assert!(terrain_type.is_valid());
    }

    #[test]
    fn test_terrain_types_manager() {
        let mut manager = TerrainTypes::new();
        let name = AsciiString::from("TestTerrain");

        // Create new terrain type
        let terrain_type = manager.new_terrain(name.clone());
        terrain_type.surface_type = TerrainSurface::Sand;
        terrain_type.traction = 0.8;

        // Find terrain type
        let found = manager.find_terrain(&name);
        assert!(found.is_some());
        assert_eq!(found.unwrap().traction, 0.8);
        assert!(matches!(found.unwrap().surface_type, TerrainSurface::Sand));

        // Count terrain types
        assert_eq!(manager.get_terrain_count(), 1);
    }

    #[test]
    fn terrain_types_new_terrain_copies_default_and_lists_newest_first() {
        let mut manager = TerrainTypes::new();

        {
            let default = manager.new_terrain(AsciiString::from("DefaultTerrain"));
            default.surface_type = TerrainSurface::Rock;
            default.texture_name = AsciiString::from("default_rock.tga");
            default.restrict_construction = true;
            default.traction = 0.75;
        }

        let first = manager.new_terrain(AsciiString::from("FirstTerrain"));
        assert_eq!(first.name.as_str(), "FirstTerrain");
        assert!(matches!(first.surface_type, TerrainSurface::Rock));
        assert_eq!(first.texture_name.as_str(), "default_rock.tga");
        assert!(first.restrict_construction);
        assert_eq!(first.traction, 0.75);

        manager.new_terrain(AsciiString::from("SecondTerrain"));
        let names: Vec<&str> = manager
            .get_terrain_names()
            .into_iter()
            .map(String::as_str)
            .collect();
        assert_eq!(
            names,
            vec!["SecondTerrain", "FirstTerrain", "DefaultTerrain"]
        );
    }

    #[test]
    fn parsed_terrain_properties_preserve_inherited_defaults() {
        let mut manager = TerrainTypes::new();
        {
            let default = manager.new_terrain(AsciiString::from("DefaultTerrain"));
            default.texture_name = AsciiString::from("default_texture.tga");
            default.restrict_construction = true;
            default.traction = 0.5;
        }

        let mut properties = HashMap::new();
        properties.insert("Texture".to_string(), "custom_texture.tga".to_string());

        manager
            .register_terrain_properties(AsciiString::from("CustomTerrain"), &properties)
            .unwrap();

        let terrain = manager
            .find_terrain(&AsciiString::from("CustomTerrain"))
            .unwrap();
        assert_eq!(terrain.texture_name.as_str(), "custom_texture.tga");
        assert!(terrain.restrict_construction);
        assert_eq!(terrain.traction, 0.5);
        assert_eq!(
            terrain.properties.get("Texture").map(String::as_str),
            Some("custom_texture.tga")
        );
    }

    #[test]
    fn test_movement_modifiers() {
        let mut terrain_type = TerrainType::new(AsciiString::from("TestTerrain"));
        terrain_type.movement_modifiers.infantry_speed = 0.5;
        terrain_type.movement_modifiers.tank_speed = 0.8;
        terrain_type.movement_modifiers.can_pass_naval = false;

        assert_eq!(terrain_type.get_speed_modifier("infantry"), 0.5);
        assert_eq!(terrain_type.get_speed_modifier("tank"), 0.8);
        assert!(!terrain_type.can_unit_pass("naval"));
        assert!(terrain_type.can_unit_pass("infantry"));
    }

    #[test]
    fn test_terrain_properties_update() {
        let mut terrain_type = TerrainType::new(AsciiString::from("Test"));
        let mut properties = HashMap::new();
        properties.insert("Class".to_string(), "ROCK".to_string());
        properties.insert("Texture".to_string(), "rock.tga".to_string());
        properties.insert("BlendEdges".to_string(), "Yes".to_string());
        properties.insert("RestrictConstruction".to_string(), "No".to_string());

        terrain_type.update_from_properties(&properties).unwrap();

        assert!(matches!(terrain_type.surface_type, TerrainSurface::Rock));
        assert_eq!(terrain_type.terrain_class.as_str(), "ROCK");
        assert_eq!(terrain_type.texture_name.as_str(), "rock.tga");
        assert!(terrain_type.blend_edge_texture);
        assert!(!terrain_type.restrict_construction);
    }

    #[test]
    fn test_parse_bool() {
        assert_eq!(parse_bool("yes"), Ok(true));
        assert_eq!(parse_bool("Yes"), Ok(true));

        assert_eq!(parse_bool("no"), Ok(false));
        assert_eq!(parse_bool("No"), Ok(false));

        assert!(parse_bool("true").is_err());
        assert!(parse_bool("1").is_err());
        assert!(parse_bool("false").is_err());
        assert!(parse_bool("0").is_err());
        assert!(parse_bool("invalid").is_err());
    }

    #[test]
    fn terrain_properties_reject_non_cpp_fields_and_values() {
        let mut terrain_type = TerrainType::new(AsciiString::from("Test"));

        let mut unknown = HashMap::new();
        unknown.insert("Surface".to_string(), "Rock".to_string());
        assert!(terrain_type.update_from_properties(&unknown).is_err());

        let mut bad_bool = HashMap::new();
        bad_bool.insert("BlendEdges".to_string(), "true".to_string());
        assert!(terrain_type.update_from_properties(&bad_bool).is_err());

        let mut bad_class = HashMap::new();
        bad_class.insert("Class".to_string(), "NOT_A_TERRAIN_CLASS".to_string());
        assert!(terrain_type.update_from_properties(&bad_class).is_err());
    }

    #[test]
    fn test_parse_color_rgb() {
        assert_eq!(parse_color_rgb("255 128 0"), Ok((255, 128, 0)));
        assert_eq!(parse_color_rgb("255,128,0"), Ok((255, 128, 0)));
        assert_eq!(parse_color_rgb("  100  50  200  "), Ok((100, 50, 200)));

        assert!(parse_color_rgb("255").is_err());
        assert!(parse_color_rgb("255 128").is_err());
        assert!(parse_color_rgb("256 128 0").is_err());
        assert!(parse_color_rgb("invalid").is_err());
    }

    #[test]
    fn test_terrain_registry_singleton() {
        let handle_a = initialize_terrain_types();
        let handle_b = initialize_terrain_types();

        assert!(Arc::ptr_eq(&handle_a, &handle_b));
        assert!(get_terrain_types().is_some());
    }

    #[test]
    fn test_validate_name() {
        assert!(IniTerrain::validate_name(&AsciiString::from("ValidName")));
        assert!(!IniTerrain::validate_name(&AsciiString::from("")));
    }

    #[test]
    fn test_token_parser() {
        assert!(TerrainTokenParser::get_next_name("TestTerrain").is_ok());
        assert!(TerrainTokenParser::get_next_name("  SpacedName  ").is_ok());
        assert!(TerrainTokenParser::get_next_name("").is_err());

        let result = TerrainTokenParser::parse_property_line("Texture = terrain.tga");
        assert!(result.is_ok());
        let (key, value) = result.unwrap();
        assert_eq!(key, "Texture");
        assert_eq!(value, "terrain.tga");
    }

    #[test]
    fn test_parse_terrain_class() {
        assert_eq!(parse_terrain_class("rock").unwrap().as_str(), "ROCK");
        assert_eq!(
            parse_terrain_class("Eastern_Europe_1").unwrap().as_str(),
            "EASTERN_EUROPE_1"
        );
        assert!(parse_terrain_class("NOT_A_TERRAIN_CLASS").is_err());
    }

    #[test]
    fn test_surface_from_class() {
        assert_eq!(
            TerrainSurface::from_class_name("ASPHALT"),
            Some(TerrainSurface::Pavement)
        );
        assert_eq!(
            TerrainSurface::from_class_name("DESERT_DRY"),
            Some(TerrainSurface::Sand)
        );
        assert_eq!(TerrainSurface::from_class_name("unknown"), None);
    }

    #[test]
    fn test_parse_terrain_file_into() {
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("terrain_test_{}.ini", unique));
        let contents = r#"
            Terrain TestSample
              Texture = SampleTexture.tga
              Class = ROCK
              BlendEdges = Yes
              RestrictConstruction = No
            End
        "#;
        fs::write(&path, contents).expect("failed to write temp terrain ini");

        let mut manager = TerrainTypes::new();
        super::parse_terrain_file_into(&mut manager, &path).expect("failed to parse terrain INI");

        let terrain = manager
            .find_terrain(&AsciiString::from("TestSample"))
            .expect("Terrain definition missing");
        assert_eq!(terrain.texture_name.as_str(), "SampleTexture.tga");
        assert_eq!(terrain.surface_type, TerrainSurface::Rock);
        assert_eq!(terrain.terrain_class.as_str(), "ROCK");
        assert!(terrain.blend_edge_texture);
        assert!(!terrain.restrict_construction);

        let count = load_terrain_definitions(&[path.clone()]).expect("failed to load terrain INI");
        assert_eq!(count, 1);
        let registry = get_terrain_types().expect("registry not initialised");
        let guard = registry.read();
        assert!(guard
            .find_terrain(&AsciiString::from("TestSample"))
            .is_some());

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn terrain_file_rejects_missing_end_and_unknown_fields() {
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let missing_end_path =
            std::env::temp_dir().join(format!("terrain_missing_end_{}.ini", unique));
        let unknown_field_path =
            std::env::temp_dir().join(format!("terrain_unknown_field_{}.ini", unique));

        fs::write(
            &missing_end_path,
            "Terrain BadTerrain\nTexture = SampleTexture.tga\nClass = ROCK\n",
        )
        .expect("failed to write missing-End terrain ini");
        fs::write(
            &unknown_field_path,
            "Terrain BadTerrain\nTexture = SampleTexture.tga\nSurface = Rock\nEnd\n",
        )
        .expect("failed to write unknown-field terrain ini");

        let mut manager = TerrainTypes::new();
        assert!(super::parse_terrain_file_into(&mut manager, &missing_end_path).is_err());
        assert!(super::parse_terrain_file_into(&mut manager, &unknown_field_path).is_err());

        let _ = fs::remove_file(&missing_end_path);
        let _ = fs::remove_file(&unknown_field_path);
    }
}
