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
            "BEACH_PARK" | "BEACH_TROPICAL" | "SAND" | "SAND_ACCENT" | "DESERT" | "DESERT_DRY" => {
                Some(Self::Sand)
            }
            "FIELD" | "GRASS_ACCENT" | "GRASS_COBBLESTONE" => Some(Self::Grass),
            "MOUNTAIN_RUGGED" | "ROCK_ACCENT" => Some(Self::Rock),
            "SNOW_FLAT" | "SNOW_RUGGED" => Some(Self::Snow),
            "WATER" => Some(Self::Water),
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
    pub movement_modifiers: MovementModifiers,
    pub texture_name: AsciiString,
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
            movement_modifiers: MovementModifiers::default(),
            texture_name: AsciiString::from(""),
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
            ("Surface", |value| {
                Ok(Box::new(TerrainSurface::from_string(value)) as Box<dyn std::any::Any>)
            }),
            ("Texture", |value| {
                Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
            }),
            ("NormalMap", |value| {
                Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
            }),
            ("DetailTexture", |value| {
                Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
            }),
            ("SoundEffect", |value| {
                Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
            }),
            ("ParticleEffect", |value| {
                Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
            }),
            ("InfantrySpeedModifier", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse infantry speed: {}", e))
            }),
            ("VehicleSpeedModifier", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse vehicle speed: {}", e))
            }),
            ("TankSpeedModifier", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse tank speed: {}", e))
            }),
            ("Traction", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse traction: {}", e))
            }),
            ("Friction", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse friction: {}", e))
            }),
            ("Bounce", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse bounce: {}", e))
            }),
            ("Hardness", |value| {
                value
                    .parse::<f32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse hardness: {}", e))
            }),
            ("IsBuildable", |value| {
                parse_bool(value)
                    .map(|b| Box::new(b) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse buildable: {}", e))
            }),
            ("IsHarvestable", |value| {
                parse_bool(value)
                    .map(|b| Box::new(b) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse harvestable: {}", e))
            }),
            ("ResourceType", |value| {
                Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
            }),
            ("ResourceAmount", |value| {
                value
                    .parse::<u32>()
                    .map(|v| Box::new(v) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse resource amount: {}", e))
            }),
            ("MinimapColor", |value| {
                parse_color_rgb(value)
                    .map(|c| Box::new(c) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse minimap color: {}", e))
            }),
        ]
    }

    /// Update terrain type from properties
    pub fn update_from_properties(&mut self, properties: &HashMap<String, String>) {
        for (key, value) in properties {
            match key.as_str() {
                "Surface" => {
                    self.surface_type = TerrainSurface::from_string(value);
                }
                "Class" => {
                    if let Some(surface) = TerrainSurface::from_class_name(value) {
                        self.surface_type = surface;
                    }
                    self.properties.insert(key.clone(), value.clone());
                }
                "Texture" => {
                    self.texture_name = AsciiString::from(value);
                }
                "NormalMap" => {
                    self.normal_map = AsciiString::from(value);
                }
                "DetailTexture" => {
                    self.detail_texture = AsciiString::from(value);
                }
                "SoundEffect" => {
                    self.sound_effect = AsciiString::from(value);
                }
                "ParticleEffect" => {
                    self.particle_effect = AsciiString::from(value);
                }
                "InfantrySpeedModifier" => {
                    if let Ok(speed) = value.parse::<f32>() {
                        self.movement_modifiers.infantry_speed = speed;
                    }
                }
                "VehicleSpeedModifier" => {
                    if let Ok(speed) = value.parse::<f32>() {
                        self.movement_modifiers.vehicle_speed = speed;
                    }
                }
                "TankSpeedModifier" => {
                    if let Ok(speed) = value.parse::<f32>() {
                        self.movement_modifiers.tank_speed = speed;
                    }
                }
                "Traction" => {
                    if let Ok(traction) = value.parse::<f32>() {
                        self.traction = traction;
                    }
                }
                "Friction" => {
                    if let Ok(friction) = value.parse::<f32>() {
                        self.friction = friction;
                    }
                }
                "Bounce" => {
                    if let Ok(bounce) = value.parse::<f32>() {
                        self.bounce = bounce;
                    }
                }
                "Hardness" => {
                    if let Ok(hardness) = value.parse::<f32>() {
                        self.hardness = hardness;
                    }
                }
                "IsBuildable" => {
                    if let Ok(buildable) = parse_bool(value) {
                        self.is_buildable = buildable;
                    }
                }
                "IsHarvestable" => {
                    if let Ok(harvestable) = parse_bool(value) {
                        self.is_harvestable = harvestable;
                    }
                }
                "ResourceType" => {
                    self.resource_type = AsciiString::from(value);
                }
                "ResourceAmount" => {
                    if let Ok(amount) = value.parse::<u32>() {
                        self.resource_amount = amount;
                    }
                }
                "MinimapColor" => {
                    if let Ok(color) = parse_color_rgb(value) {
                        self.minimap_color = color;
                    }
                }
                _ => {
                    // Store unknown properties
                    self.properties.insert(key.clone(), value.clone());
                }
            }
        }
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
}

impl TerrainTypes {
    pub fn new() -> Self {
        Self {
            terrain_types: HashMap::new(),
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
        let terrain_type = TerrainType::new(name.clone());
        self.terrain_types
            .insert(name.as_str().to_string(), terrain_type);
        self.terrain_types.get_mut(name.as_str()).unwrap()
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
        self.terrain_types.insert(name, terrain_type);
    }

    /// Get all terrain type names
    pub fn get_terrain_names(&self) -> Vec<&String> {
        self.terrain_types.keys().collect()
    }

    /// Get terrain types by surface
    pub fn get_terrains_by_surface(&self, surface: &TerrainSurface) -> Vec<&TerrainType> {
        self.terrain_types
            .values()
            .filter(|t| &t.surface_type == surface)
            .collect()
    }

    /// Remove a terrain type
    pub fn remove_terrain(&mut self, name: &AsciiString) -> bool {
        self.terrain_types.remove(name.as_str()).is_some()
    }

    /// Clear all terrain types
    pub fn clear(&mut self) {
        self.terrain_types.clear();
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
        "true" | "yes" | "1" => Ok(true),
        "false" | "no" | "0" => Ok(false),
        _ => Err(format!("Invalid boolean value: {}", value)),
    }
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
        terrain_type.update_from_properties(&properties);

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
        terrain_types.write().register_terrain(terrain_type);
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
                let terrain = IniTerrain::parse_terrain_block(name, props)?;
                target.register_terrain(terrain);
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
                let props = std::mem::take(&mut properties);
                let terrain = IniTerrain::parse_terrain_block(name, props)?;
                target.register_terrain(terrain);
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
                let props = std::mem::take(&mut properties);
                let terrain = IniTerrain::parse_terrain_block(name, props)?;
                target.register_terrain(terrain);
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
        let props = std::mem::take(&mut properties);
        let terrain = IniTerrain::parse_terrain_block(name, props)?;
        target.register_terrain(terrain);
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
        properties.insert("Surface".to_string(), "Rock".to_string());
        properties.insert("Traction".to_string(), "1.2".to_string());
        properties.insert("IsBuildable".to_string(), "false".to_string());
        properties.insert("ResourceAmount".to_string(), "500".to_string());

        terrain_type.update_from_properties(&properties);

        assert!(matches!(terrain_type.surface_type, TerrainSurface::Rock));
        assert_eq!(terrain_type.traction, 1.2);
        assert!(!terrain_type.is_buildable);
        assert_eq!(terrain_type.resource_amount, 500);
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

        let result = TerrainTokenParser::parse_property_line("Traction = 1.5");
        assert!(result.is_ok());
        let (key, value) = result.unwrap();
        assert_eq!(key, "Traction");
        assert_eq!(value, "1.5");
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
              Surface = Rock
              HeightMin = 10
              HeightMax = 25
              SlopeMinDegrees = 30
              Priority = 6
            End
        "#;
        fs::write(&path, contents).expect("failed to write temp terrain ini");

        let mut manager = TerrainTypes::new();
        super::parse_terrain_file_into(&mut manager, &path).expect("failed to parse terrain INI");

        let terrain = manager
            .find_terrain(&AsciiString::from("TestSample"))
            .expect("Terrain definition missing");
        assert_eq!(terrain.texture_name.as_str(), "SampleTexture.tga");
        assert_eq!(
            terrain.properties.get("HeightMin").map(String::as_str),
            Some("10")
        );
        assert_eq!(terrain.surface_type, TerrainSurface::Rock);

        let count = load_terrain_definitions(&[path.clone()]).expect("failed to load terrain INI");
        assert_eq!(count, 1);
        let registry = get_terrain_types().expect("registry not initialised");
        let guard = registry.read();
        assert!(guard
            .find_terrain(&AsciiString::from("TestSample"))
            .is_some());

        let _ = fs::remove_file(&path);
    }
}
