////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

//! FILE: ini_terrain_road.rs
//! Author: Colin Day, December 2001 (Converted to Rust)
//! Desc:   Terrain road INI loading

use crate::common::ascii_string::AsciiString;
use crate::common::ini::ini_terrain_bridge::{
    get_terrain_roads, initialize_terrain_roads, TerrainRoadType,
};
use crate::debug_assert_crash;
use std::collections::HashMap;

/// Result type for terrain road parsing operations
pub type TerrainRoadResult<T> = Result<T, TerrainRoadError>;

/// Errors that can occur during terrain road parsing
#[derive(Debug, Clone, PartialEq)]
pub enum TerrainRoadError {
    InvalidName,
    AllocationError,
    ParseError(String),
    InvalidData,
    NotFound,
    AlreadyExists,
    ConflictingType,
}

impl std::fmt::Display for TerrainRoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TerrainRoadError::InvalidName => write!(f, "Invalid road name"),
            TerrainRoadError::AllocationError => write!(f, "Failed to allocate road"),
            TerrainRoadError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            TerrainRoadError::InvalidData => write!(f, "Invalid road data"),
            TerrainRoadError::NotFound => write!(f, "Road not found"),
            TerrainRoadError::AlreadyExists => write!(f, "Road already exists"),
            TerrainRoadError::ConflictingType => write!(f, "Conflicting road/bridge type"),
        }
    }
}

impl std::error::Error for TerrainRoadError {}

/// Road surface materials
#[derive(Debug, Clone, PartialEq)]
pub enum RoadSurface {
    Dirt,
    Gravel,
    Asphalt,
    Cobblestone,
    Concrete,
    Brick,
    Wood,
    Custom(String),
}

impl RoadSurface {
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "dirt" => Self::Dirt,
            "gravel" => Self::Gravel,
            "asphalt" => Self::Asphalt,
            "cobblestone" => Self::Cobblestone,
            "concrete" => Self::Concrete,
            "brick" => Self::Brick,
            "wood" => Self::Wood,
            _ => Self::Custom(s.to_string()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Dirt => "Dirt",
            Self::Gravel => "Gravel",
            Self::Asphalt => "Asphalt",
            Self::Cobblestone => "Cobblestone",
            Self::Concrete => "Concrete",
            Self::Brick => "Brick",
            Self::Wood => "Wood",
            Self::Custom(name) => name,
        }
    }

    /// Get the typical speed modifier for this road surface
    pub fn get_speed_modifier(&self) -> f32 {
        match self {
            Self::Dirt => 1.1,
            Self::Gravel => 1.15,
            Self::Asphalt => 1.3,
            Self::Cobblestone => 1.2,
            Self::Concrete => 1.4,
            Self::Brick => 1.25,
            Self::Wood => 1.1,
            Self::Custom(_) => 1.2,
        }
    }

    /// Get the typical durability for this road surface
    pub fn get_durability(&self) -> f32 {
        match self {
            Self::Dirt => 0.3,
            Self::Gravel => 0.5,
            Self::Asphalt => 0.8,
            Self::Cobblestone => 0.9,
            Self::Concrete => 1.0,
            Self::Brick => 0.7,
            Self::Wood => 0.4,
            Self::Custom(_) => 0.6,
        }
    }
}

/// Road configuration and properties
#[derive(Debug, Clone)]
pub struct RoadConfiguration {
    pub surface: RoadSurface,
    pub width: f32,
    pub lane_count: u32,
    pub has_sidewalks: bool,
    pub has_streetlights: bool,
    pub construction_cost: u32,
    pub maintenance_cost: u32,
    pub wear_rate: f32,
    pub max_weight_limit: f32, // For vehicles
}

impl Default for RoadConfiguration {
    fn default() -> Self {
        Self {
            surface: RoadSurface::Dirt,
            width: 8.0,
            lane_count: 2,
            has_sidewalks: false,
            has_streetlights: false,
            construction_cost: 100,
            maintenance_cost: 5,
            wear_rate: 0.1,
            max_weight_limit: 1000.0,
        }
    }
}

/// Parse a boolean value from string
pub fn parse_bool(value: &str) -> Result<bool, String> {
    match value.trim().to_lowercase().as_str() {
        "true" | "yes" | "1" => Ok(true),
        "false" | "no" | "0" => Ok(false),
        _ => Err(format!("Invalid boolean value: {}", value)),
    }
}

/// INI parsing functions for terrain roads
pub struct IniTerrainRoad;

impl IniTerrainRoad {
    /// Parse terrain road definition - equivalent to INI::parseTerrainRoadDefinition
    pub fn parse_terrain_road_definition(name: AsciiString) -> TerrainRoadResult<()> {
        // Validate name
        if name.is_empty() {
            return Err(TerrainRoadError::InvalidName);
        }

        // Fetch terrain roads registry (lazily creates it on first use)
        let terrain_roads = initialize_terrain_roads();
        let mut terrain_roads = terrain_roads.write();

        // Find existing road - it should not be a bridge if it exists
        let existing_road = terrain_roads.find_road(&name);
        if let Some(existing) = existing_road {
            // Sanity check - if item is found it better not already be a bridge
            debug_assert_crash!(
                !existing.is_bridge(),
                "Redefining bridge '{}' as a road!",
                existing.get_name().as_str()
            );
            return Err(TerrainRoadError::ConflictingType);
        }

        // Create new road if it doesn't exist
        let road = if terrain_roads.find_road(&name).is_none() {
            terrain_roads.new_road(name.clone())
        } else {
            terrain_roads
                .find_road_mut(&name)
                .expect("Road should exist after lookup")
        };

        debug_assert_crash!(
            road.is_valid(),
            "Unable to allocate road '{}'",
            name.as_str()
        );

        // In the original C++, this would call:
        // ini->initFromINI(road, road->getRoadFieldParse());
        println!("Parsing terrain road definition for: {}", name.as_str());

        Ok(())
    }

    /// Parse a complete terrain road block from INI data
    pub fn parse_terrain_road_block(
        name: AsciiString,
        properties: HashMap<String, String>,
    ) -> TerrainRoadResult<TerrainRoadType> {
        // Validate name
        if name.is_empty() {
            return Err(TerrainRoadError::InvalidName);
        }

        // Create road
        let mut road = TerrainRoadType::new_road(name);

        // Update road from properties
        road.update_from_properties(&properties)
            .map_err(TerrainRoadError::ParseError)?;

        // Validate road
        if !road.is_valid() {
            return Err(TerrainRoadError::ParseError(
                "Invalid road configuration".to_string(),
            ));
        }

        Ok(road)
    }

    /// Parse road configuration from properties
    pub fn parse_road_configuration(
        properties: &HashMap<String, String>,
    ) -> TerrainRoadResult<RoadConfiguration> {
        let mut config = RoadConfiguration::default();

        for (key, value) in properties {
            match key.as_str() {
                "Surface" => {
                    config.surface = RoadSurface::from_string(value);
                }
                "Width" => {
                    config.width = value.parse::<f32>().map_err(|e| {
                        TerrainRoadError::ParseError(format!("Invalid width: {}", e))
                    })?;
                }
                "LaneCount" => {
                    config.lane_count = value.parse::<u32>().map_err(|e| {
                        TerrainRoadError::ParseError(format!("Invalid lane count: {}", e))
                    })?;
                }
                "HasSidewalks" => {
                    config.has_sidewalks = parse_bool(value).map_err(|e| {
                        TerrainRoadError::ParseError(format!("Invalid sidewalks flag: {}", e))
                    })?;
                }
                "HasStreetlights" => {
                    config.has_streetlights = parse_bool(value).map_err(|e| {
                        TerrainRoadError::ParseError(format!("Invalid streetlights flag: {}", e))
                    })?;
                }
                "ConstructionCost" => {
                    config.construction_cost = value.parse::<u32>().map_err(|e| {
                        TerrainRoadError::ParseError(format!("Invalid construction cost: {}", e))
                    })?;
                }
                "MaintenanceCost" => {
                    config.maintenance_cost = value.parse::<u32>().map_err(|e| {
                        TerrainRoadError::ParseError(format!("Invalid maintenance cost: {}", e))
                    })?;
                }
                "WearRate" => {
                    config.wear_rate = value.parse::<f32>().map_err(|e| {
                        TerrainRoadError::ParseError(format!("Invalid wear rate: {}", e))
                    })?;
                }
                "MaxWeightLimit" => {
                    config.max_weight_limit = value.parse::<f32>().map_err(|e| {
                        TerrainRoadError::ParseError(format!("Invalid weight limit: {}", e))
                    })?;
                }
                _ => {
                    // Ignore unknown properties for road configuration
                }
            }
        }

        Ok(config)
    }

    /// Register a terrain road
    pub fn register_terrain_road(road: TerrainRoadType) -> TerrainRoadResult<()> {
        if road.is_bridge {
            return Err(TerrainRoadError::ConflictingType);
        }

        let terrain_roads = initialize_terrain_roads();
        terrain_roads.write().register_road_type(road);
        Ok(())
    }

    /// Find a terrain road by name
    pub fn find_terrain_road_by_name(name: &AsciiString) -> Option<TerrainRoadType> {
        get_terrain_roads().and_then(|terrain_roads| terrain_roads.read().find_road(name).cloned())
    }

    /// Validate road name format
    pub fn validate_name(name: &AsciiString) -> bool {
        !name.is_empty() && name.len() < 128 // Reasonable length limit
    }

    /// Check if a name conflicts with existing bridges
    pub fn check_name_conflict(name: &AsciiString) -> bool {
        get_terrain_roads()
            .map(|terrain_roads| terrain_roads.read().find_bridge(name).is_some())
            .unwrap_or(false)
    }

    /// Create a road with specific configuration
    pub fn create_road_with_config(
        name: AsciiString,
        config: RoadConfiguration,
    ) -> TerrainRoadResult<TerrainRoadType> {
        let mut road = TerrainRoadType::new_road(name);

        // Apply configuration
        road.width = config.width;
        road.construction_cost = config.construction_cost;
        road.movement_speed_modifier = config.surface.get_speed_modifier();

        // Set additional properties based on configuration
        if config.has_sidewalks {
            road.supports_infantry = true;
        }

        // Adjust durability based on surface
        let base_health = road.health;
        road.health = base_health * config.surface.get_durability();

        Ok(road)
    }

    /// Get recommended configuration for a road surface
    pub fn get_recommended_config_for_surface(surface: RoadSurface) -> RoadConfiguration {
        let mut config = RoadConfiguration::default();
        config.surface = surface.clone();

        match surface {
            RoadSurface::Dirt => {
                config.width = 6.0;
                config.lane_count = 1;
                config.construction_cost = 50;
                config.maintenance_cost = 2;
                config.has_sidewalks = false;
                config.has_streetlights = false;
            }
            RoadSurface::Gravel => {
                config.width = 8.0;
                config.lane_count = 2;
                config.construction_cost = 100;
                config.maintenance_cost = 5;
            }
            RoadSurface::Asphalt => {
                config.width = 12.0;
                config.lane_count = 2;
                config.construction_cost = 200;
                config.maintenance_cost = 10;
                config.has_sidewalks = true;
                config.has_streetlights = true;
            }
            RoadSurface::Concrete => {
                config.width = 15.0;
                config.lane_count = 4;
                config.construction_cost = 300;
                config.maintenance_cost = 8;
                config.has_sidewalks = true;
                config.has_streetlights = true;
                config.max_weight_limit = 2000.0;
            }
            _ => {
                // Use defaults for other surfaces
            }
        }

        config
    }
}

/// Token parser for extracting road data from INI tokens
pub struct RoadTokenParser;

impl RoadTokenParser {
    /// Extract the next road name token
    pub fn get_next_name(token: &str) -> TerrainRoadResult<AsciiString> {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            return Err(TerrainRoadError::InvalidName);
        }

        Ok(AsciiString::from(trimmed))
    }

    /// Parse a property line (key = value)
    pub fn parse_property_line(line: &str) -> TerrainRoadResult<(String, String)> {
        if let Some(eq_pos) = line.find('=') {
            let key = line[..eq_pos].trim().to_string();
            let value = line[eq_pos + 1..].trim().to_string();
            Ok((key, value))
        } else {
            Err(TerrainRoadError::ParseError(format!(
                "Invalid property line format: {}",
                line
            )))
        }
    }

    /// Parse road surface from token
    pub fn parse_road_surface(token: &str) -> TerrainRoadResult<RoadSurface> {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            return Err(TerrainRoadError::InvalidName);
        }

        Ok(RoadSurface::from_string(trimmed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_road_surface_parsing() {
        assert_eq!(RoadSurface::from_string("dirt"), RoadSurface::Dirt);
        assert_eq!(RoadSurface::from_string("ASPHALT"), RoadSurface::Asphalt);
        assert_eq!(
            RoadSurface::from_string("CustomSurface"),
            RoadSurface::Custom("CustomSurface".to_string())
        );
    }

    #[test]
    fn test_road_surface_properties() {
        assert_eq!(RoadSurface::Dirt.get_speed_modifier(), 1.1);
        assert_eq!(RoadSurface::Concrete.get_speed_modifier(), 1.4);
        assert_eq!(RoadSurface::Dirt.get_durability(), 0.3);
        assert_eq!(RoadSurface::Concrete.get_durability(), 1.0);
    }

    #[test]
    fn test_road_configuration() {
        let config = RoadConfiguration::default();

        assert!(matches!(config.surface, RoadSurface::Dirt));
        assert_eq!(config.width, 8.0);
        assert_eq!(config.lane_count, 2);
        assert!(!config.has_sidewalks);
        assert!(!config.has_streetlights);
    }

    #[test]
    fn test_parse_road_configuration() {
        let mut properties = HashMap::new();
        properties.insert("Surface".to_string(), "Asphalt".to_string());
        properties.insert("Width".to_string(), "12.0".to_string());
        properties.insert("LaneCount".to_string(), "4".to_string());
        properties.insert("HasSidewalks".to_string(), "true".to_string());
        properties.insert("ConstructionCost".to_string(), "250".to_string());

        let result = IniTerrainRoad::parse_road_configuration(&properties);
        assert!(result.is_ok());

        let config = result.unwrap();
        assert!(matches!(config.surface, RoadSurface::Asphalt));
        assert_eq!(config.width, 12.0);
        assert_eq!(config.lane_count, 4);
        assert!(config.has_sidewalks);
        assert_eq!(config.construction_cost, 250);
    }

    #[test]
    fn test_recommended_config() {
        let dirt_config = IniTerrainRoad::get_recommended_config_for_surface(RoadSurface::Dirt);
        assert_eq!(dirt_config.width, 6.0);
        assert_eq!(dirt_config.lane_count, 1);
        assert!(!dirt_config.has_sidewalks);

        let concrete_config =
            IniTerrainRoad::get_recommended_config_for_surface(RoadSurface::Concrete);
        assert_eq!(concrete_config.width, 15.0);
        assert_eq!(concrete_config.lane_count, 4);
        assert!(concrete_config.has_sidewalks);
        assert!(concrete_config.has_streetlights);
    }

    #[test]
    fn test_create_road_with_config() {
        let name = AsciiString::from("TestRoad");
        let mut config = RoadConfiguration::default();
        config.surface = RoadSurface::Asphalt;
        config.width = 10.0;
        config.has_sidewalks = true;

        let result = IniTerrainRoad::create_road_with_config(name.clone(), config);
        assert!(result.is_ok());

        let road = result.unwrap();
        assert_eq!(road.name, name);
        assert_eq!(road.width, 10.0);
        assert_eq!(
            road.movement_speed_modifier,
            RoadSurface::Asphalt.get_speed_modifier()
        );
        assert!(road.supports_infantry);
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
        assert!(IniTerrainRoad::validate_name(&AsciiString::from(
            "ValidName"
        )));
        assert!(!IniTerrainRoad::validate_name(&AsciiString::from("")));
    }

    #[test]
    fn test_token_parser() {
        assert!(RoadTokenParser::get_next_name("TestRoad").is_ok());
        assert!(RoadTokenParser::get_next_name("  SpacedName  ").is_ok());
        assert!(RoadTokenParser::get_next_name("").is_err());

        let result = RoadTokenParser::parse_property_line("Width = 10.0");
        assert!(result.is_ok());
        let (key, value) = result.unwrap();
        assert_eq!(key, "Width");
        assert_eq!(value, "10.0");

        assert!(RoadTokenParser::parse_road_surface("Asphalt").is_ok());
        assert!(RoadTokenParser::parse_road_surface("").is_err());
    }
}
