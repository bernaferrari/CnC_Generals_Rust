//! Full Map Loading Pipeline
//!
//! Orchestrates loading a .map file through the complete initialization sequence:
//!   1. Parse the binary .map file (DataChunk format) via system::map_loader
//!   2. Parse embedded INI sections (lighting, weather, scripts)
//!   3. Build terrain tiles from the heightmap and texture data
//!   4. Place objects (buildings, units, waypoints)
//!   5. Register polygon triggers for scripts and water areas
//!   6. Set up player starting positions and teams
//!
//! This module connects the INI parser to the runtime map state so that a
//! single `load_and_apply()` call takes a file path to a fully playable map.

use super::object_placer::ObjectPlacer;
use super::terrain_loader::TerrainLoader;
use crate::common::*;
use crate::polygon_trigger::PolygonTriggerList;
use crate::scripting::ini_parser::parse_script_from_ini;
use crate::scripting::{MapMetadata, MapScriptLoader};
use crate::sides_list::get_sides_list;
use crate::system::map_loader::{
    BridgeData, Coord3D as MapCoord3D, ICoord2D, LoadError, MapData, MapLoader as BinaryMapLoader,
    MapWaypoint,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors that can occur during the full map-loading pipeline.
#[derive(Debug)]
pub enum MapLoadError {
    /// The requested file does not exist.
    FileNotFound(String),
    /// A low-level parse error inside the binary .map file.
    ParseError(String, usize),
    /// Heightmap dimensions or content are invalid.
    InvalidTerrainData(String),
    /// A required INI section is missing from the map.
    MissingRequiredSection(String),
    /// An object in the map refers to an unknown thing-template.
    InvalidObjectDefinition(String),
    /// The terrain logic singleton is unavailable (not initialised).
    TerrainLogicUnavailable,
    /// Wrapped I/O error.
    Io(std::io::Error),
}

impl std::fmt::Display for MapLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FileNotFound(p) => write!(f, "map file not found: {}", p),
            Self::ParseError(file, line) => {
                write!(f, "parse error in {} at line {}", file, line)
            }
            Self::InvalidTerrainData(msg) => write!(f, "invalid terrain data: {}", msg),
            Self::MissingRequiredSection(name) => {
                write!(f, "required map section missing: {}", name)
            }
            Self::InvalidObjectDefinition(msg) => {
                write!(f, "invalid object definition: {}", msg)
            }
            Self::TerrainLogicUnavailable => write!(f, "terrain logic singleton unavailable"),
            Self::Io(e) => write!(f, "I/O error: {}", e),
        }
    }
}

impl std::error::Error for MapLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for MapLoadError {
    fn from(e: std::io::Error) -> Self {
        MapLoadError::Io(e)
    }
}

impl From<LoadError> for MapLoadError {
    fn from(e: LoadError) -> Self {
        match e {
            LoadError::IoError(io) => MapLoadError::Io(io),
            LoadError::ParseError(msg) => MapLoadError::ParseError(msg, 0),
            LoadError::InvalidFormat(msg) => MapLoadError::InvalidTerrainData(msg),
        }
    }
}

// ---------------------------------------------------------------------------
// Lighting & weather
// ---------------------------------------------------------------------------

/// Time-of-day presets matching C++ `TimeOfDayNames`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeOfDay {
    Invalid,
    Morning,
    Afternoon,
    Evening,
    Night,
}

impl TimeOfDay {
    /// Attempt to parse from the C++ name strings.
    pub fn from_str_loose(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "morning" => Self::Morning,
            "afternoon" | "after_noon" => Self::Afternoon,
            "evening" => Self::Evening,
            "night" => Self::Night,
            _ => Self::Invalid,
        }
    }
}

/// Lighting and weather settings embedded in the map's `WorldInfo` dict.
#[derive(Debug, Clone)]
pub struct LightingSettings {
    /// Time-of-day preset used for water shader selection.
    pub time_of_day: TimeOfDay,
    /// Ambient light colour (RGB 0-1).
    pub ambient_color: [f32; 3],
    /// Global fog density (0 = none).
    pub fog_density: f32,
    /// Wind direction angle in degrees.
    pub wind_angle: f32,
    /// Wind strength multiplier.
    pub wind_strength: f32,
}

impl Default for LightingSettings {
    fn default() -> Self {
        Self {
            time_of_day: TimeOfDay::Afternoon,
            ambient_color: [0.6, 0.6, 0.6],
            fog_density: 0.0,
            wind_angle: 0.0,
            wind_strength: 1.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Player starting data
// ---------------------------------------------------------------------------

/// Per-player data extracted from the map (sides list + waypoints).
#[derive(Debug, Clone)]
pub struct PlayerStartInfo {
    /// Player slot index (0-based).
    pub slot: usize,
    /// Side / faction display name (e.g. "America", "China", "GLA").
    pub faction: String,
    /// Owner key used by the object system (e.g. "PlyrCivilian", "Plyr1").
    pub owner: String,
    /// Starting position in world coordinates.
    pub start_position: Option<MapCoord3D>,
    /// Starting money, if overridden by the map.
    pub start_money: Option<u32>,
    /// Whether this slot is human-controlled (from map default; may be overridden).
    pub is_human: bool,
    /// Ally player indices.
    pub allies: Vec<usize>,
    /// Enemy player indices.
    pub enemies: Vec<usize>,
}

// ---------------------------------------------------------------------------
// Camera path entry
// ---------------------------------------------------------------------------

/// A single node in an intro / cinematic camera path.
#[derive(Debug, Clone)]
pub struct CameraPathNode {
    pub position: MapCoord3D,
    pub look_at: MapCoord3D,
    pub transition_time_ms: u32,
}

/// Camera path data for the map's intro sequence.
#[derive(Debug, Clone, Default)]
pub struct CameraPath {
    pub nodes: Vec<CameraPathNode>,
    pub letterbox: bool,
}

// ---------------------------------------------------------------------------
// Full map data (high-level)
// ---------------------------------------------------------------------------

/// Complete high-level representation of a loaded map, bridging the raw binary
/// parse result with the subsystems that consume it.
#[derive(Debug, Clone)]
pub struct FullMapData {
    // --- terrain -----------------------------------------------------------
    /// Raw heightmap bytes.
    pub heightmap: Vec<u8>,
    /// Playable width in cells (excluding border).
    pub width: u32,
    /// Playable height in cells (excluding border).
    pub height: u32,
    /// Border width in cells.
    pub border_size: i32,
    /// Map boundary points.
    pub boundaries: Vec<ICoord2D>,
    /// Flat texture tile indices (splat-map layer 0).
    pub texture_tiles: Vec<u8>,
    /// Global water table height.
    pub water_height: Option<f32>,

    // --- objects & bridges -------------------------------------------------
    /// Bridges extracted from map objects.
    pub bridges: Vec<BridgeData>,
    /// Named waypoints for AI navigation.
    pub waypoints: Vec<MapWaypoint>,
    /// Directed waypoint links (id_from, id_to).
    pub waypoint_links: Vec<(u32, u32)>,

    // --- polygon triggers --------------------------------------------------
    /// Trigger areas, water polygons, rivers.
    pub polygon_triggers: Vec<crate::polygon_trigger::PolygonTrigger>,

    // --- lighting & weather ------------------------------------------------
    pub lighting: LightingSettings,

    // --- players -----------------------------------------------------------
    pub players: Vec<PlayerStartInfo>,

    // --- camera ------------------------------------------------------------
    pub camera_path: CameraPath,

    // --- scripts -----------------------------------------------------------
    /// Parsed script list (may be empty for maps without embedded scripts).
    pub scripts: Option<Box<crate::scripting::core::ScriptList>>,

    // --- metadata ----------------------------------------------------------
    pub metadata: MapMetadata,

    // --- raw world dict for subsystems that need keys not explicitly modelled
    pub world_dict: HashMap<String, String>,
}

impl Default for FullMapData {
    fn default() -> Self {
        Self {
            heightmap: Vec::new(),
            width: 0,
            height: 0,
            border_size: 0,
            boundaries: Vec::new(),
            texture_tiles: Vec::new(),
            water_height: None,
            bridges: Vec::new(),
            waypoints: Vec::new(),
            waypoint_links: Vec::new(),
            polygon_triggers: Vec::new(),
            lighting: LightingSettings::default(),
            players: Vec::new(),
            camera_path: CameraPath::default(),
            scripts: None,
            metadata: MapMetadata::default(),
            world_dict: HashMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// MapLoader (pipeline orchestrator)
// ---------------------------------------------------------------------------

/// High-level map loader that runs the full load sequence.
///
/// Usage:
/// ```ignore
/// let map = MapLoader::load_and_apply("maps/my_map.map")?;
/// ```
pub struct MapLoader;

impl MapLoader {
    /// Load a map from *path* and return the fully populated `FullMapData`.
    ///
    /// This does **not** mutate any engine singletons -- it is a pure parse step.
    /// Call `apply_map` afterwards to push the data into the running game.
    pub fn load(path: &str) -> Result<FullMapData, MapLoadError> {
        let map_path = Path::new(path);
        if !map_path.exists() {
            return Err(MapLoadError::FileNotFound(path.to_string()));
        }

        // 1. Binary parse (DataChunk format)
        let binary_data = BinaryMapLoader::load(map_path)?;

        // 2. INI sections (lighting, scripts, etc.) -- parsed from WorldInfo dict
        //    and any embedded text chunks inside the binary map.
        let lighting = extract_lighting(&binary_data);
        let players = extract_player_starts(&binary_data);
        let camera = extract_camera_path(&binary_data);

        // 3. Attempt to load embedded scripts (many maps ship them in binary form
        //    and are handled by the sides-list chunk; some INI-only maps carry a
        //    text [Scripts] section that we try to parse here).
        let scripts = load_embedded_scripts(path);

        // 4. Build FullMapData
        let metadata = MapMetadata {
            name: Path::file_stem(map_path)
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string(),
            description: String::new(),
            author: String::new(),
            player_count: players.len(),
            width: binary_data.width,
            height: binary_data.height,
        };

        let full = FullMapData {
            heightmap: binary_data.heightmap,
            width: binary_data.width,
            height: binary_data.height,
            border_size: binary_data.border_size,
            boundaries: binary_data.boundaries,
            texture_tiles: binary_data.texture_tiles,
            water_height: binary_data.water_height,
            bridges: binary_data.bridges,
            waypoints: binary_data.waypoints,
            waypoint_links: binary_data.waypoint_links,
            polygon_triggers: binary_data.polygon_triggers,
            lighting,
            players,
            camera_path: camera,
            scripts,
            metadata,
            world_dict: HashMap::new(),
        };

        Ok(full)
    }

    /// Load a map and immediately apply it to all engine singletons.
    ///
    /// This is the one-call entry point used by the game-start sequence.
    pub fn load_and_apply(path: &str) -> Result<FullMapData, MapLoadError> {
        let full = Self::load(path)?;
        Self::apply_map(&full)?;
        Ok(full)
    }

    /// Push `FullMapData` into the running engine singletons:
    ///   - TerrainLogic (heightmap, bridges, polygon triggers, waypoints)
    ///   - MapSystem (object list)
    ///   - SidesList (player / team definitions)
    ///   - ScriptEngine (trigger scripts)
    pub fn apply_map(data: &FullMapData) -> Result<(), MapLoadError> {
        // --- terrain -------------------------------------------------------
        // Build a system::MapData from our high-level representation and feed
        // it to TerrainLogic::load_map_data.
        let terrain_data = crate::system::map_loader::MapData {
            width: data.width,
            height: data.height,
            heightmap: data.heightmap.clone(),
            water_height: data.water_height,
            bridges: data.bridges.clone(),
            texture_tiles: data.texture_tiles.clone(),
            boundaries: data.boundaries.clone(),
            border_size: data.border_size,
            polygon_triggers: data.polygon_triggers.clone(),
            waypoints: data.waypoints.clone(),
            waypoint_links: data.waypoint_links.clone(),
        };

        {
            let mut terrain_logic = crate::terrain::get_terrain_logic()
                .write()
                .map_err(|_| MapLoadError::TerrainLogicUnavailable)?;
            terrain_logic.load_map_data(terrain_data);
        }

        // --- sides list (player / team) ------------------------------------
        // The sides list is populated by the binary map parser's SidesList
        // chunk handler.  That runs inside BinaryMapLoader::load() already,
        // so by the time we get here the sides list should be populated.
        // We still register player-start positions if the map provides them.
        register_player_starts(data);

        // --- scripts -------------------------------------------------------
        // Script registration is handled separately by the scripting subsystem.
        // The parsed scripts are available via data.scripts for callers that
        // need them (e.g. the game-start sequence).
        if data.scripts.is_some() {
            log::debug!("MapLoader::apply_map: script lists available for registration");
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn extract_lighting(_data: &MapData) -> LightingSettings {
    // The binary map does not carry lighting info directly on MapData.
    // The lighting settings are defaulted here.  When the full WorldInfo dict
    // is available through the binary loader, the caller can override.
    LightingSettings::default()
}

fn extract_player_starts(data: &MapData) -> Vec<PlayerStartInfo> {
    let mut players = Vec::new();

    for waypoint in &data.waypoints {
        if let Some(slot) = parse_player_start_slot(&waypoint.name) {
            players.push(PlayerStartInfo {
                slot,
                faction: String::new(),
                owner: format!("Plyr{}", slot + 1),
                start_position: Some(waypoint.location),
                start_money: None,
                is_human: slot == 0,
                allies: Vec::new(),
                enemies: Vec::new(),
            });
        }
    }

    players.sort_by_key(|player| player.slot);

    // Ensure at least one player entry
    if players.is_empty() {
        players.push(PlayerStartInfo {
            slot: 0,
            faction: String::new(),
            owner: "PlyrCivilian".to_string(),
            start_position: None,
            start_money: None,
            is_human: true,
            allies: Vec::new(),
            enemies: Vec::new(),
        });
    }

    players
}

fn parse_player_start_slot(name: &str) -> Option<usize> {
    let lower = name.to_ascii_lowercase();
    let number = lower
        .strip_prefix("player_")
        .and_then(|rest| rest.strip_suffix("_start"))?;

    number
        .parse::<usize>()
        .ok()
        .and_then(|slot| slot.checked_sub(1))
}

fn extract_camera_path(data: &MapData) -> CameraPath {
    // The camera path is stored in WorldInfo dict keys like
    //   "cameraPathNode0X" / "cameraPathNode0Y" / "cameraPathNode0Z"
    // Since MapData does not expose world_dict, we return a default path.
    // When the full pipeline has access to the dict, this can be populated.
    let mut cam = CameraPath::default();

    // Look for InitialCameraPosition waypoint
    for wp in &data.waypoints {
        if wp.name.eq_ignore_ascii_case("InitialCameraPosition") {
            cam.nodes.push(CameraPathNode {
                position: wp.location,
                look_at: MapCoord3D::new(0.0, 0.0, 0.0),
                transition_time_ms: 0,
            });
            break;
        }
    }

    cam
}

/// Try to load embedded map scripts.  Returns `None` if no scripts are found.
fn load_embedded_scripts(path: &str) -> Option<Box<crate::scripting::core::ScriptList>> {
    let map_path = Path::new(path);
    if !map_path.exists() {
        return None;
    }

    let mut loader = MapScriptLoader::new();
    match loader.load_from_map(map_path) {
        Ok(scripts) => Some(scripts),
        Err(_) => None,
    }
}

fn register_player_starts(data: &FullMapData) {
    for player in &data.players {
        if let Some(pos) = player.start_position {
            // Store the starting position in the map system's world dict
            let key = format!("Player_{}_Start", player.slot + 1);
            if let Ok(mut system) = crate::map::get_map_system().write() {
                system
                    .get_world_dict_mut()
                    .insert(key, format!("{},{},{}", pos.x, pos.y, pos.z));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_of_day_parsing() {
        assert_eq!(TimeOfDay::from_str_loose("Morning"), TimeOfDay::Morning);
        assert_eq!(TimeOfDay::from_str_loose("AFTERNOON"), TimeOfDay::Afternoon);
        assert_eq!(TimeOfDay::from_str_loose("Night"), TimeOfDay::Night);
        assert_eq!(TimeOfDay::from_str_loose("unknown"), TimeOfDay::Invalid);
    }

    #[test]
    fn test_lighting_settings_default() {
        let lit = LightingSettings::default();
        assert_eq!(lit.time_of_day, TimeOfDay::Afternoon);
        assert_eq!(lit.ambient_color, [0.6, 0.6, 0.6]);
    }

    #[test]
    fn test_full_map_data_default() {
        let data = FullMapData::default();
        assert!(data.heightmap.is_empty());
        assert!(data.players.is_empty());
        assert!(data.waypoints.is_empty());
    }

    #[test]
    fn test_map_load_error_display() {
        let err = MapLoadError::FileNotFound("test.map".to_string());
        assert_eq!(format!("{}", err), "map file not found: test.map");

        let err = MapLoadError::MissingRequiredSection("HeightMapData".to_string());
        assert!(format!("{}", err).contains("HeightMapData"));
    }

    #[test]
    fn test_load_nonexistent_map() {
        let result = MapLoader::load("/nonexistent/path/test.map");
        assert!(result.is_err());
        match result.unwrap_err() {
            MapLoadError::FileNotFound(_) => {}
            other => panic!("expected FileNotFound, got {:?}", other),
        }
    }

    #[test]
    fn test_extract_player_starts_from_waypoints() {
        let data = MapData {
            width: 100,
            height: 100,
            heightmap: vec![0u8; 100 * 100],
            water_height: None,
            bridges: Vec::new(),
            texture_tiles: Vec::new(),
            boundaries: Vec::new(),
            border_size: 10,
            polygon_triggers: Vec::new(),
            waypoints: vec![
                MapWaypoint {
                    id: 1,
                    name: "Player_1_Start".to_string(),
                    location: MapCoord3D::new(100.0, 200.0, 0.0),
                    path_label1: String::new(),
                    path_label2: String::new(),
                    path_label3: String::new(),
                    bi_directional: false,
                },
                MapWaypoint {
                    id: 2,
                    name: "Player_2_Start".to_string(),
                    location: MapCoord3D::new(500.0, 600.0, 0.0),
                    path_label1: String::new(),
                    path_label2: String::new(),
                    path_label3: String::new(),
                    bi_directional: false,
                },
            ],
            waypoint_links: Vec::new(),
        };

        let players = extract_player_starts(&data);
        assert_eq!(players.len(), 2);
        assert_eq!(players[0].owner, "Plyr1");
        assert_eq!(players[1].owner, "Plyr2");
    }

    #[test]
    fn test_extract_player_starts_uses_start_name_slot_not_waypoint_order() {
        let data = MapData {
            width: 100,
            height: 100,
            heightmap: vec![0u8; 100 * 100],
            water_height: None,
            bridges: Vec::new(),
            texture_tiles: Vec::new(),
            boundaries: Vec::new(),
            border_size: 10,
            polygon_triggers: Vec::new(),
            waypoints: vec![
                MapWaypoint {
                    id: 99,
                    name: "CameraStart".to_string(),
                    location: MapCoord3D::new(50.0, 60.0, 0.0),
                    path_label1: String::new(),
                    path_label2: String::new(),
                    path_label3: String::new(),
                    bi_directional: false,
                },
                MapWaypoint {
                    id: 2,
                    name: "Player_2_Start".to_string(),
                    location: MapCoord3D::new(500.0, 600.0, 0.0),
                    path_label1: String::new(),
                    path_label2: String::new(),
                    path_label3: String::new(),
                    bi_directional: false,
                },
                MapWaypoint {
                    id: 1,
                    name: "Player_1_Start".to_string(),
                    location: MapCoord3D::new(100.0, 200.0, 0.0),
                    path_label1: String::new(),
                    path_label2: String::new(),
                    path_label3: String::new(),
                    bi_directional: false,
                },
            ],
            waypoint_links: Vec::new(),
        };

        let players = extract_player_starts(&data);
        assert_eq!(players.len(), 2);
        assert_eq!(players[0].slot, 0);
        assert_eq!(players[0].owner, "Plyr1");
        assert_eq!(
            players[0].start_position,
            Some(MapCoord3D::new(100.0, 200.0, 0.0))
        );
        assert_eq!(players[1].slot, 1);
        assert_eq!(players[1].owner, "Plyr2");
    }

    #[test]
    fn test_extract_player_starts_requires_cpp_waypoint_name() {
        let data = MapData {
            width: 100,
            height: 100,
            heightmap: vec![0u8; 100 * 100],
            water_height: None,
            bridges: Vec::new(),
            texture_tiles: Vec::new(),
            boundaries: Vec::new(),
            border_size: 10,
            polygon_triggers: Vec::new(),
            waypoints: vec![
                MapWaypoint {
                    id: 1,
                    name: "PlayerStart".to_string(),
                    location: MapCoord3D::new(100.0, 200.0, 0.0),
                    path_label1: String::new(),
                    path_label2: String::new(),
                    path_label3: String::new(),
                    bi_directional: false,
                },
                MapWaypoint {
                    id: 2,
                    name: "Plyr1Start".to_string(),
                    location: MapCoord3D::new(300.0, 400.0, 0.0),
                    path_label1: String::new(),
                    path_label2: String::new(),
                    path_label3: String::new(),
                    bi_directional: false,
                },
            ],
            waypoint_links: Vec::new(),
        };

        let players = extract_player_starts(&data);
        assert_eq!(players.len(), 1);
        assert_eq!(players[0].owner, "PlyrCivilian");
        assert_eq!(players[0].start_position, None);
    }

    #[test]
    fn test_extract_player_starts_empty() {
        let data = MapData {
            width: 100,
            height: 100,
            heightmap: vec![0u8; 100 * 100],
            water_height: None,
            bridges: Vec::new(),
            texture_tiles: Vec::new(),
            boundaries: Vec::new(),
            border_size: 10,
            polygon_triggers: Vec::new(),
            waypoints: Vec::new(),
            waypoint_links: Vec::new(),
        };

        let players = extract_player_starts(&data);
        assert_eq!(players.len(), 1); // default entry
        assert_eq!(players[0].owner, "PlyrCivilian");
    }
}
