//! INI parsing for MapData definitions
//!
//! This module handles parsing MapData entries from INI files.
//! MapData contains runtime map configuration and properties.
//!
//! Author: Colin Day, November 2001
//! Rust port: 2025

use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::common::ini::ini::INI;

/// 3D coordinate representation
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coord3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Coord3D {
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn zero() -> Self {
        Self::ZERO
    }
}

impl Default for Coord3D {
    fn default() -> Self {
        Self::zero()
    }
}

/// 2D coordinate representation
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coord2D {
    pub x: f32,
    pub y: f32,
}

impl Coord2D {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn zero() -> Self {
        Self::ZERO
    }
}

impl Default for Coord2D {
    fn default() -> Self {
        Self::zero()
    }
}

/// RGB Color representation
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RGBColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl RGBColor {
    pub fn new(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b }
    }

    pub fn black() -> Self {
        Self {
            r: 0.0,
            g: 0.0,
            b: 0.0,
        }
    }

    pub fn white() -> Self {
        Self {
            r: 1.0,
            g: 1.0,
            b: 1.0,
        }
    }
}

impl Default for RGBColor {
    fn default() -> Self {
        Self::black()
    }
}

/// Map lighting information
#[derive(Debug, Clone)]
pub struct MapLighting {
    pub ambient_color: RGBColor,
    pub diffuse_color: RGBColor,
    pub light_direction: Coord3D,
    pub shadow_color: RGBColor,
}

impl Default for MapLighting {
    fn default() -> Self {
        Self {
            ambient_color: RGBColor::new(0.3, 0.3, 0.3),
            diffuse_color: RGBColor::new(0.7, 0.7, 0.7),
            light_direction: Coord3D::new(0.0, 0.0, -1.0),
            shadow_color: RGBColor::new(0.2, 0.2, 0.2),
        }
    }
}

/// Map camera configuration
#[derive(Debug, Clone)]
pub struct MapCamera {
    pub position: Coord3D,
    pub target: Coord3D,
    pub zoom: f32,
    pub angle: f32,
    pub pitch: f32,
}

impl Default for MapCamera {
    fn default() -> Self {
        Self {
            position: Coord3D::new(0.0, 0.0, 100.0),
            target: Coord3D::ZERO,
            zoom: 1.0,
            angle: 0.0,
            pitch: 45.0,
        }
    }
}

/// Map environment settings
#[derive(Debug, Clone)]
pub struct MapEnvironment {
    pub wind_direction: Coord2D,
    pub wind_strength: f32,
    pub temperature: f32,
    pub humidity: f32,
    pub visibility: f32,
}

impl Default for MapEnvironment {
    fn default() -> Self {
        Self {
            wind_direction: Coord2D::new(1.0, 0.0),
            wind_strength: 1.0,
            temperature: 20.0,
            humidity: 0.5,
            visibility: 1.0,
        }
    }
}

/// Map bounds and dimensions
#[derive(Debug, Clone)]
pub struct MapBounds {
    pub min_x: f32,
    pub max_x: f32,
    pub min_y: f32,
    pub max_y: f32,
    pub min_z: f32,
    pub max_z: f32,
}

impl MapBounds {
    pub fn new(min_x: f32, max_x: f32, min_y: f32, max_y: f32, min_z: f32, max_z: f32) -> Self {
        Self {
            min_x,
            max_x,
            min_y,
            max_y,
            min_z,
            max_z,
        }
    }

    pub fn get_width(&self) -> f32 {
        self.max_x - self.min_x
    }

    pub fn get_height(&self) -> f32 {
        self.max_y - self.min_y
    }

    pub fn get_depth(&self) -> f32 {
        self.max_z - self.min_z
    }

    pub fn contains_point(&self, point: Coord3D) -> bool {
        point.x >= self.min_x
            && point.x <= self.max_x
            && point.y >= self.min_y
            && point.y <= self.max_y
            && point.z >= self.min_z
            && point.z <= self.max_z
    }

    pub fn get_center(&self) -> Coord3D {
        Coord3D::new(
            (self.min_x + self.max_x) / 2.0,
            (self.min_y + self.max_y) / 2.0,
            (self.min_z + self.max_z) / 2.0,
        )
    }
}

impl Default for MapBounds {
    fn default() -> Self {
        Self {
            min_x: -1000.0,
            max_x: 1000.0,
            min_y: -1000.0,
            max_y: 1000.0,
            min_z: 0.0,
            max_z: 100.0,
        }
    }
}

/// Map script configuration
#[derive(Debug, Clone)]
pub struct MapScript {
    pub script_name: String,
    pub parameters: HashMap<String, String>,
    pub enabled: bool,
}

impl MapScript {
    pub fn new(name: String) -> Self {
        Self {
            script_name: name,
            parameters: HashMap::new(),
            enabled: true,
        }
    }

    pub fn set_parameter(&mut self, key: String, value: String) {
        self.parameters.insert(key, value);
    }

    pub fn get_parameter(&self, key: &str) -> Option<&String> {
        self.parameters.get(key)
    }
}

impl Default for MapScript {
    fn default() -> Self {
        Self {
            script_name: String::new(),
            parameters: HashMap::new(),
            enabled: false,
        }
    }
}

/// Map data configuration.
///
/// Contains runtime configuration and properties for a map.
#[derive(Debug, Clone)]
pub struct MapData {
    /// Map identification
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub author: String,
    pub version: String,

    /// Map dimensions and bounds
    pub bounds: MapBounds,
    pub recommended_players: i32,
    pub max_players: i32,

    /// Visual settings
    pub lighting: MapLighting,
    pub default_camera: MapCamera,
    pub environment: MapEnvironment,

    /// Gameplay settings
    pub starting_resources: i32,
    pub tech_level: i32,
    pub is_multiplayer: bool,
    pub is_official: bool,
    pub allow_cheats: bool,
    pub time_limit: i32, // seconds, 0 = no limit

    /// Scripting
    pub scripts: Vec<MapScript>,
    pub script_enabled: bool,

    /// Preview image
    pub preview_image: String,

    /// Custom properties
    pub custom_properties: HashMap<String, String>,
}

impl Default for MapData {
    fn default() -> Self {
        Self::new()
    }
}

impl MapData {
    /// Create a new MapData with default values
    pub fn new() -> Self {
        Self {
            name: String::new(),
            display_name: String::new(),
            description: String::new(),
            author: String::new(),
            version: "1.0".to_string(),

            bounds: MapBounds::default(),
            recommended_players: 2,
            max_players: 8,

            lighting: MapLighting::default(),
            default_camera: MapCamera::default(),
            environment: MapEnvironment::default(),

            starting_resources: 5000,
            tech_level: 1,
            is_multiplayer: false,
            is_official: false,
            allow_cheats: true,
            time_limit: 0,

            scripts: Vec::new(),
            script_enabled: false,

            preview_image: String::new(),

            custom_properties: HashMap::new(),
        }
    }

    /// Set map name and display name
    pub fn set_name(&mut self, name: String) {
        self.name = name.clone();
        if self.display_name.is_empty() {
            self.display_name = name;
        }
    }

    /// Add a script to the map
    pub fn add_script(&mut self, script: MapScript) {
        self.scripts.push(script);
    }

    /// Remove a script by name
    pub fn remove_script(&mut self, script_name: &str) {
        self.scripts
            .retain(|script| script.script_name != script_name);
    }

    /// Get a script by name
    pub fn get_script(&self, script_name: &str) -> Option<&MapScript> {
        self.scripts
            .iter()
            .find(|script| script.script_name == script_name)
    }

    /// Get a mutable script by name
    pub fn get_script_mut(&mut self, script_name: &str) -> Option<&mut MapScript> {
        self.scripts
            .iter_mut()
            .find(|script| script.script_name == script_name)
    }

    /// Set a custom property
    pub fn set_custom_property(&mut self, key: String, value: String) {
        self.custom_properties.insert(key, value);
    }

    /// Get a custom property
    pub fn get_custom_property(&self, key: &str) -> Option<&String> {
        self.custom_properties.get(key)
    }

    /// Check if map supports the given number of players
    pub fn supports_player_count(&self, player_count: i32) -> bool {
        player_count >= 1 && player_count <= self.max_players
    }

    /// Get map area in square units
    pub fn get_area(&self) -> f32 {
        self.bounds.get_width() * self.bounds.get_height()
    }

    /// Check if this is a valid map configuration
    pub fn is_valid(&self) -> bool {
        !self.name.is_empty()
            && self.max_players > 0
            && self.bounds.get_width() > 0.0
            && self.bounds.get_height() > 0.0
    }

    /// Parse from INI file.
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), String> {
        loop {
            ini.read_line().map_err(|error| error.to_string())?;
            if ini.is_eof() {
                return Err("Unexpected EOF while parsing MapData block".to_string());
            }

            let tokens = ini.get_line_tokens();
            let Some(key) = tokens.first().copied() else {
                continue;
            };
            if key.eq_ignore_ascii_case("End") {
                break;
            }

            let values: Vec<&str> = tokens
                .iter()
                .skip(1)
                .copied()
                .filter(|token| *token != "=")
                .collect();
            if values.is_empty() {
                continue;
            }

            let key_lc = key.to_ascii_lowercase();
            match key_lc.as_str() {
                "name" => self.set_name(values.join(" ")),
                "displayname" => self.display_name = values.join(" "),
                "description" => self.description = values.join(" "),
                "author" => self.author = values.join(" "),
                "version" => self.version = values.join(" "),
                "recommendedplayers" => {
                    if let Ok(v) = INI::parse_int(values[0]) {
                        self.recommended_players = v.max(1);
                    }
                }
                "maxplayers" => {
                    if let Ok(v) = INI::parse_int(values[0]) {
                        self.max_players = v.max(1);
                    }
                }
                "startingresources" => {
                    if let Ok(v) = INI::parse_int(values[0]) {
                        self.starting_resources = v.max(0);
                    }
                }
                "techlevel" => {
                    if let Ok(v) = INI::parse_int(values[0]) {
                        self.tech_level = v.max(0);
                    }
                }
                "ismultiplayer" => {
                    if let Ok(v) = INI::parse_bool(values[0]) {
                        self.is_multiplayer = v;
                    }
                }
                "isofficial" => {
                    if let Ok(v) = INI::parse_bool(values[0]) {
                        self.is_official = v;
                    }
                }
                "allowcheats" => {
                    if let Ok(v) = INI::parse_bool(values[0]) {
                        self.allow_cheats = v;
                    }
                }
                "timelimit" => {
                    if let Ok(v) = INI::parse_int(values[0]) {
                        self.time_limit = v.max(0);
                    }
                }
                "scriptenabled" => {
                    if let Ok(v) = INI::parse_bool(values[0]) {
                        self.script_enabled = v;
                    }
                }
                "previewimage" => self.preview_image = values.join(" "),
                "extentmin" => {
                    if let Ok((x, y, z)) = INI::parse_coord_3d(&values) {
                        self.bounds.min_x = x;
                        self.bounds.min_y = y;
                        self.bounds.min_z = z;
                    }
                }
                "extentmax" => {
                    if let Ok((x, y, z)) = INI::parse_coord_3d(&values) {
                        self.bounds.max_x = x;
                        self.bounds.max_y = y;
                        self.bounds.max_z = z;
                    }
                }
                "cameraposition" => {
                    if let Ok((x, y, z)) = INI::parse_coord_3d(&values) {
                        self.default_camera.position = Coord3D::new(x, y, z);
                    }
                }
                "cameratarget" => {
                    if let Ok((x, y, z)) = INI::parse_coord_3d(&values) {
                        self.default_camera.target = Coord3D::new(x, y, z);
                    }
                }
                "camerazoom" => {
                    if let Ok(v) = INI::parse_real(values[0]) {
                        self.default_camera.zoom = v.max(0.0);
                    }
                }
                "cameraangle" => {
                    if let Ok(v) = INI::parse_real(values[0]) {
                        self.default_camera.angle = v;
                    }
                }
                "camerapitch" => {
                    if let Ok(v) = INI::parse_real(values[0]) {
                        self.default_camera.pitch = v;
                    }
                }
                "winddirection" => {
                    if let Ok((x, y)) = INI::parse_coord_2d(&values) {
                        self.environment.wind_direction = Coord2D::new(x, y);
                    }
                }
                "windstrength" => {
                    if let Ok(v) = INI::parse_real(values[0]) {
                        self.environment.wind_strength = v.max(0.0);
                    }
                }
                "temperature" => {
                    if let Ok(v) = INI::parse_real(values[0]) {
                        self.environment.temperature = v;
                    }
                }
                "humidity" => {
                    if let Ok(v) = INI::parse_real(values[0]) {
                        self.environment.humidity = v.clamp(0.0, 1.0);
                    }
                }
                "visibility" => {
                    if let Ok(v) = INI::parse_real(values[0]) {
                        self.environment.visibility = v.max(0.0);
                    }
                }
                "script" => self.add_script(MapScript::new(values.join(" "))),
                "property" => {
                    if values.len() >= 2 {
                        self.set_custom_property(values[0].to_string(), values[1..].join(" "));
                    }
                }
                _ => {
                    // Keep unknown entries forward-compatible.
                }
            }
        }

        Ok(())
    }
}

/// Global MapData instance (placeholder for actual global)
static MAP_DATA: OnceCell<RwLock<MapData>> = OnceCell::new();

/// Initialize the global map data
pub fn init_global_map_data() {
    if MAP_DATA.get().is_none() {
        let _ = MAP_DATA.set(RwLock::new(MapData::new()));
    } else if let Some(data) = MAP_DATA.get() {
        if let Ok(mut guard) = data.write() {
            *guard = MapData::new();
        }
    }
}

/// Get reference to global map data
pub fn get_map_data() -> Option<RwLockReadGuard<'static, MapData>> {
    MAP_DATA
        .get()
        .map(|data| data.read().expect("MapData poisoned"))
}

/// Get mutable reference to global map data
pub fn get_map_data_mut() -> Option<RwLockWriteGuard<'static, MapData>> {
    MAP_DATA
        .get()
        .map(|data| data.write().expect("MapData poisoned"))
}

/// INI parsing function for MapData definition (matches C++ interface)
///
/// This is the main entry point for parsing MapData definitions from INI files.
/// In the original C++, this function was empty, but we provide a framework
/// for future implementation.
pub fn parse_map_data_definition(ini: &mut INI) -> Result<(), String> {
    let header_tokens = ini.get_line_tokens();
    let map_name = header_tokens
        .iter()
        .skip(1)
        .find(|token| **token != "=")
        .map(|token| token.to_string());

    // Get or create global map data
    if MAP_DATA.get().is_none() {
        init_global_map_data();
    }

    if let Some(mut map_data) = get_map_data_mut() {
        if let Some(name) = map_name {
            map_data.set_name(name);
        }
        map_data.parse_from_ini(ini)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_data_creation() {
        let map_data = MapData::new();
        assert_eq!(map_data.version, "1.0");
        assert_eq!(map_data.recommended_players, 2);
        assert_eq!(map_data.max_players, 8);
        assert!(!map_data.is_multiplayer);
        assert!(!map_data.is_official);
        assert!(map_data.allow_cheats);
        assert_eq!(map_data.time_limit, 0);
    }

    #[test]
    fn test_map_bounds() {
        let bounds = MapBounds::new(-500.0, 500.0, -300.0, 300.0, 0.0, 50.0);

        assert_eq!(bounds.get_width(), 1000.0);
        assert_eq!(bounds.get_height(), 600.0);
        assert_eq!(bounds.get_depth(), 50.0);

        let center = bounds.get_center();
        assert_eq!(center.x, 0.0);
        assert_eq!(center.y, 0.0);
        assert_eq!(center.z, 25.0);

        assert!(bounds.contains_point(Coord3D::new(0.0, 0.0, 25.0)));
        assert!(!bounds.contains_point(Coord3D::new(600.0, 0.0, 25.0)));
    }

    #[test]
    fn test_map_script() {
        let mut script = MapScript::new("TestScript".to_string());
        assert_eq!(script.script_name, "TestScript");
        assert!(script.enabled);
        assert!(script.parameters.is_empty());

        script.set_parameter("param1".to_string(), "value1".to_string());
        assert_eq!(script.get_parameter("param1"), Some(&"value1".to_string()));
        assert_eq!(script.get_parameter("nonexistent"), None);
    }

    #[test]
    fn test_map_lighting() {
        let lighting = MapLighting::default();
        assert_eq!(lighting.ambient_color.r, 0.3);
        assert_eq!(lighting.diffuse_color.r, 0.7);
        assert_eq!(lighting.light_direction.z, -1.0);
    }

    #[test]
    fn test_map_camera() {
        let camera = MapCamera::default();
        assert_eq!(camera.position.z, 100.0);
        assert_eq!(camera.zoom, 1.0);
        assert_eq!(camera.pitch, 45.0);
    }

    #[test]
    fn test_map_environment() {
        let environment = MapEnvironment::default();
        assert_eq!(environment.wind_direction.x, 1.0);
        assert_eq!(environment.wind_strength, 1.0);
        assert_eq!(environment.temperature, 20.0);
        assert_eq!(environment.humidity, 0.5);
        assert_eq!(environment.visibility, 1.0);
    }

    #[test]
    fn test_map_data_operations() {
        let mut map_data = MapData::new();

        map_data.set_name("TestMap".to_string());
        assert_eq!(map_data.name, "TestMap");
        assert_eq!(map_data.display_name, "TestMap");

        // Test scripts
        let script = MapScript::new("InitScript".to_string());
        map_data.add_script(script);
        assert_eq!(map_data.scripts.len(), 1);

        assert!(map_data.get_script("InitScript").is_some());
        assert!(map_data.get_script("NonExistent").is_none());

        map_data.remove_script("InitScript");
        assert_eq!(map_data.scripts.len(), 0);

        // Test custom properties
        map_data.set_custom_property("difficulty".to_string(), "hard".to_string());
        assert_eq!(
            map_data.get_custom_property("difficulty"),
            Some(&"hard".to_string())
        );

        // Test player support
        assert!(map_data.supports_player_count(4));
        assert!(!map_data.supports_player_count(10));

        // Test area calculation
        let area = map_data.get_area();
        assert!(area > 0.0);

        // Test validity
        map_data.set_name("ValidMap".to_string());
        assert!(map_data.is_valid());

        let mut invalid_map = MapData::new();
        invalid_map.name = "".to_string(); // Invalid: empty name
        assert!(!invalid_map.is_valid());
    }

    #[test]
    fn test_coordinates() {
        let coord3d = Coord3D::new(1.0, 2.0, 3.0);
        assert_eq!(coord3d.x, 1.0);
        assert_eq!(coord3d.y, 2.0);
        assert_eq!(coord3d.z, 3.0);

        let zero3d = Coord3D::ZERO;
        assert_eq!(zero3d.x, 0.0);

        let coord2d = Coord2D::new(5.0, 6.0);
        assert_eq!(coord2d.x, 5.0);
        assert_eq!(coord2d.y, 6.0);
    }

    #[test]
    fn test_rgb_color() {
        let red = RGBColor::new(1.0, 0.0, 0.0);
        assert_eq!(red.r, 1.0);
        assert_eq!(red.g, 0.0);
        assert_eq!(red.b, 0.0);

        let white = RGBColor::white();
        assert_eq!(white.r, 1.0);
        assert_eq!(white.g, 1.0);
        assert_eq!(white.b, 1.0);

        let black = RGBColor::black();
        assert_eq!(black.r, 0.0);
    }

    #[test]
    fn test_global_map_data() {
        init_global_map_data();

        assert!(get_map_data().is_some());

        if let Some(mut map_data) = get_map_data_mut() {
            map_data.set_name("GlobalTest".to_string());
        }

        if let Some(map_data) = get_map_data() {
            assert_eq!(map_data.name, "GlobalTest");
        }
    }
}
