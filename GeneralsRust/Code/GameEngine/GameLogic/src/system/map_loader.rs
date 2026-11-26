//! MAP LOADING SEQUENCE
//!
//! Complete map loading and initialization based on:
//! - /GeneralsMD/Code/GameEngine/Source/GameClient/MapUtil.cpp
//! - /GeneralsMD/Code/GameEngine/Source/Common/System/SaveGame/GameStateMap.cpp
//! - /GeneralsMD/Code/GameEngine/Source/Common/GameEngine.cpp
//!
//! This module implements the complete map initialization flow from the C++ codebase.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use crate::common::{KindOf, ThingTemplate};
use crate::helpers::TheThingFactory;
use crate::polygon_trigger::{PolygonTrigger, PolygonTriggerList};
use crate::scripting::chunk_codec::ScriptTemplateLookup;
use crate::sides_list::{SidesList, get_sides_list};
use crate::system::engine_stores::WorldServices;
use game_engine::common::common_services::{CommonServices, common_services};
use game_engine::common::dict::{Dict, DictType};
use game_engine::common::name_key_generator::NameKeyNamespace;
use game_engine::common::system::DataChunkInfo;
use game_engine::common::system::DataChunkInput;
use game_engine::map_data::{MapObjectChunk, uncompressed_map_bytes};

pub use game_engine::common::system::geometry::Coord2D;
pub use game_engine::map_data::{
    BridgeData, BridgeEndpoints, BridgeMarkers, BridgeModels, HeightMap, MAX_AUTHORED_BRIDGES,
    MapData, MapWaypoint, read_mounted_bridge_model,
};
pub use game_engine::map_object::MAP_XY_FACTOR;

/// Waypoint identifier type
pub type WaypointID = u32;

/// Invalid waypoint constant
pub const INVALID_WAYPOINT_ID: WaypointID = 0xFFFFFFFF;

/// Maximum player slots (from C++ MAX_SLOTS)
pub const MAX_SLOTS: usize = 8;

/// Height map data version identifiers (from C++ MapUtil.cpp)
pub const K_HEIGHT_MAP_VERSION_1: u32 = 1;
pub const K_HEIGHT_MAP_VERSION_2: u32 = 2;
pub const K_HEIGHT_MAP_VERSION_3: u32 = 3;
pub const K_HEIGHT_MAP_VERSION_4: u32 = 4;

/// Objects chunk version (from C++ MapUtil.cpp)
pub const K_OBJECTS_VERSION_2: u32 = 2;

pub use crate::common::{Coord3D, ICoord2D, Region3D};

/// Waypoint map - maps waypoint names to positions
/// Matches C++ WaypointMap from MapUtil.cpp
pub type WaypointMap = HashMap<String, Coord3D>;

/// Map metadata structure
/// Matches C++ MapMetaData from MapUtil.cpp
#[derive(Debug, Clone)]
pub struct MapMetaData {
    pub file_name: String,
    pub filesize: u32,
    pub crc: u32,
    pub is_official: bool,
    pub is_multiplayer: bool,
    pub num_players: usize,
    pub extent: Region3D,
    pub display_name: String,
    pub name_lookup_tag: String,
    pub waypoints: WaypointMap,
    pub tech_positions: Vec<Coord3D>,
    pub supply_positions: Vec<Coord3D>,
    pub timestamp_high: u32,
    pub timestamp_low: u32,
}

impl Default for MapMetaData {
    fn default() -> Self {
        Self {
            file_name: String::new(),
            filesize: 0,
            crc: 0,
            is_official: false,
            is_multiplayer: false,
            num_players: 1,
            extent: Region3D::new(Coord3D::ZERO, Coord3D::ZERO),
            display_name: String::new(),
            name_lookup_tag: String::new(),
            waypoints: HashMap::new(),
            tech_positions: Vec::new(),
            supply_positions: Vec::new(),
            timestamp_high: 0,
            timestamp_low: 0,
        }
    }
}

/// Error type for map loading
#[derive(Debug)]
pub enum LoadError {
    IoError(io::Error),
    ParseError(String),
    InvalidFormat(String),
}

impl From<io::Error> for LoadError {
    fn from(err: io::Error) -> Self {
        LoadError::IoError(err)
    }
}

/// Map loader responsible for loading and parsing .map files
/// Matches C++ loadMap() from MapUtil.cpp
pub struct MapLoader {
    heightmap: HeightMap,
    waypoints: WaypointMap,
    tech_positions: Vec<Coord3D>,
    supply_positions: Vec<Coord3D>,
    world_dict: HashMap<String, String>,
    bridges: Vec<BridgeData>,
    polygon_triggers: Vec<PolygonTrigger>,
    waypoint_defs: Vec<MapWaypoint>,
    waypoint_links: Vec<(u32, u32)>,
    sides: Option<SidesList>,
}

impl MapLoader {
    pub fn new() -> Self {
        Self {
            heightmap: HeightMap::new(),
            waypoints: HashMap::new(),
            tech_positions: Vec::new(),
            supply_positions: Vec::new(),
            world_dict: HashMap::new(),
            bridges: Vec::new(),
            polygon_triggers: Vec::new(),
            waypoint_defs: Vec::new(),
            waypoint_links: Vec::new(),
            sides: None,
        }
    }

    /// Load a .map file and extract terrain data
    /// Matches C++ loadMap() from MapUtil.cpp:214
    ///
    /// # Arguments
    /// * `path` - Path to the .map file
    ///
    /// # Returns
    /// Result containing MapData or LoadError
    pub fn load(path: &Path) -> Result<MapData, LoadError> {
        let mut loader = MapLoader::new();
        loader.load_map(path)?;

        // Convert loaded data to MapData structure
        let map_data = loader.to_map_data();
        Ok(map_data)
    }

    /// Convert loaded data to MapData structure
    /// Reference: C++ MapUtil.cpp lines 214-255
    pub fn to_map_data(&self) -> MapData {
        let (width, height) = self.heightmap.get_playable_dimensions();

        MapData {
            width: width as u32,
            height: height as u32,
            heightmap: self.heightmap.data.clone(),
            water_height: self.extract_water_height(),
            bridges: self.bridges.clone(),
            texture_tiles: Vec::new(), // Would be populated from additional map data
            boundaries: self.heightmap.boundaries.clone(),
            border_size: self.heightmap.border_size,
            polygon_triggers: self.polygon_triggers.clone(),
            waypoints: self.waypoint_defs.clone(),
            waypoint_links: self.waypoint_links.clone(),
        }
    }

    /// Extract water height from world dictionary
    /// Reference: C++ TerrainLogic.cpp water table initialization
    fn extract_water_height(&self) -> Option<f32> {
        // Check world dict for water height setting
        self.world_dict
            .get("waterHeight")
            .and_then(|s| s.parse::<f32>().ok())
    }

    /// Load a map file from the given path
    /// Matches C++ loadMap() from MapUtil.cpp:214
    pub fn load_map<P: AsRef<Path>>(&mut self, filename: P) -> io::Result<()> {
        self.load_map_from_bytes(&Self::read_map_file(filename.as_ref())?)
    }

    /// Decode the map, then resolve its authored objects in the receiving catalog.
    /// The caller installs the parsed sides when it admits the map.
    pub(crate) fn load_map_in(&mut self, path: &Path, world: &WorldServices) -> io::Result<()> {
        let bytes = Self::read_map_file(path)?;
        let templates = world
            .scripts()
            .engine()
            .read()
            .map_err(|error| io::Error::other(error.to_string()))?
            .as_ref()
            .map(ScriptTemplateLookup::from_engine)
            .unwrap_or_default();
        let common = world.common();
        let decoded = {
            let mut names = common
                .name_key_namespace
                .lock()
                .map_err(|error| io::Error::other(error.to_string()))?;
            Self::parse_map_data(&bytes, true, Some(&mut names), Some(templates), Some(common.clone()))?
        };
        let mut bridge_models = BridgeModels::default();
        self.accept_decoded(
            decoded,
            |name| TheThingFactory::find_template_in(common, name),
            |span| {
                let definition = common
                    .ini
                    .terrain_roads
                    .read()
                    .find_bridge(&span.template_name.as_str().into())
                    .map(|bridge| (bridge.model_name.clone(), bridge.bridge_scale));
                let Some((model, scale)) = definition else {
                    log::warn!(
                        "Map bridge '{}' has no Bridge definition",
                        span.template_name
                    );
                    return None;
                };
                bridge_models.deck(span, model.as_str(), scale, |path| {
                    read_mounted_bridge_model(&common.application.file_system, path)
                })
            },
        );
        Ok(())
    }

    fn read_map_file(path: &Path) -> io::Result<Vec<u8>> {
        if !path.extension().is_some_and(|ext| ext == "map") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "File must have .map extension",
            ));
        }
        let mut file = File::open(path)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        Ok(bytes)
    }

    /// Load map data from raw bytes (used by archive-backed file systems).
    pub fn load_map_from_bytes(&mut self, data: &[u8]) -> io::Result<()> {
        self.load_with_legacy_services(data, true)
    }

    /// Load only the chunks needed to rebuild runtime terrain/sides/waypoint state.
    ///
    /// This skips `ObjectsList` because these callers already have placement data.
    pub fn load_runtime_support_from_bytes(&mut self, data: &[u8]) -> io::Result<()> {
        self.load_with_legacy_services(data, false)
    }

    fn load_with_legacy_services(&mut self, bytes: &[u8], include_objects: bool) -> io::Result<()> {
        let decoded = Self::parse_map_data(bytes, include_objects, None, None, None)?;
        self.accept_decoded(
            decoded,
            |name| TheThingFactory::find_template(name),
            |span| {
                let width = TheThingFactory::find_template(&span.template_name)
                    .map(|template| template.get_template_geometry_info().get_minor_radius() * 2.0)
                    .filter(|&width| width > 0.0)
                    .unwrap_or(MAP_XY_FACTOR * 2.0);
                Some(BridgeData::new(
                    span.from,
                    span.to,
                    width,
                    span.template_name,
                ))
            },
        );
        if self.sides.is_some() {
            self.install_sides_in(&get_sides_list())?;
        }
        Ok(())
    }

    /// A SidesList chunk replaces the receiving authored sides (MapUtil.cpp).
    pub(crate) fn install_sides_in(&mut self, owner: &RwLock<SidesList>) -> io::Result<()> {
        if let Some(sides) = self.sides.take() {
            *owner
                .write()
                .map_err(|error| io::Error::other(error.to_string()))? = sides;
        }
        Ok(())
    }

    /// Decode DataChunk content with the supplied namespace and script definitions.
    fn parse_map_data(
        data: &[u8],
        include_objects: bool,
        names: Option<&mut NameKeyNamespace>,
        script_templates: Option<ScriptTemplateLookup>,
        common: Option<Arc<CommonServices>>,
    ) -> io::Result<MapParseContext> {
        let chunk_bytes = uncompressed_map_bytes(data)?.into_owned();
        let mut input = match names {
            Some(names) => DataChunkInput::with_namespace(chunk_bytes, names),
            None => DataChunkInput::new(chunk_bytes),
        };
        if !input.is_valid_file_type() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Map file is not a valid DataChunk map",
            ));
        }

        let mut context = MapParseContext::new(script_templates, common);
        input.register_parser("HeightMapData", "", parse_heightmap_size_chunk);
        input.register_parser("WorldInfo", "", parse_world_info_chunk);
        if include_objects {
            input.register_parser("ObjectsList", "", parse_objects_list_chunk);
        }
        input.register_parser("PolygonTriggers", "", parse_polygon_triggers_chunk);
        input.register_parser("WaypointsList", "", parse_waypoints_list_chunk);
        input.register_parser("SidesList", "", parse_sides_list_chunk);

        if !input.parse(&mut context) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Map file could not be parsed",
            ));
        }

        if context.heightmap.width <= 0 || context.heightmap.height <= 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Map file missing heightmap dimensions",
            ));
        }

        if context.heightmap.data.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Map file missing heightmap data",
            ));
        }

        Ok(context)
    }

    fn accept_decoded(
        &mut self,
        context: MapParseContext,
        template: impl Fn(&str) -> Option<Arc<dyn ThingTemplate>>,
        bridge: impl FnMut(BridgeEndpoints) -> Option<BridgeData>,
    ) {
        self.heightmap = context.heightmap;
        self.world_dict = context.world_dict;
        self.waypoints = context.waypoints;
        self.polygon_triggers = context.triggers.get_triggers().to_vec();
        self.waypoint_defs = context.waypoint_defs;
        self.waypoint_links = context.waypoint_links;
        self.sides = context.sides;

        self.tech_positions.clear();
        self.supply_positions.clear();
        for (name, location) in context.placed_objects {
            if let Some(template) = template(&name) {
                if template.is_kind_of(KindOf::TechBuilding) {
                    self.tech_positions.push(location);
                } else if template.is_kind_of(KindOf::SupplySourceOnPreview) {
                    self.supply_positions.push(location);
                }
            }
        }
        self.bridges = context
            .bridges
            .into_iter()
            .filter_map(bridge)
            .take(MAX_AUTHORED_BRIDGES)
            .collect();
    }

    /// Get the loaded heightmap
    pub fn get_heightmap(&self) -> &HeightMap {
        &self.heightmap
    }

    /// Get waypoints
    pub fn get_waypoints(&self) -> &WaypointMap {
        &self.waypoints
    }

    /// Get tech building positions
    pub fn get_tech_positions(&self) -> &[Coord3D] {
        &self.tech_positions
    }

    /// Get supply source positions
    pub fn get_supply_positions(&self) -> &[Coord3D] {
        &self.supply_positions
    }

    /// Get world dictionary
    pub fn get_world_dict(&self) -> &HashMap<String, String> {
        &self.world_dict
    }

    /// Count start spots from waypoints
    /// Matches C++ WaypointMap::update() from MapUtil.cpp:291
    pub fn count_start_spots(&self) -> usize {
        let mut num_start_spots = 0;

        // Count Player_N_Start waypoints (1-based in C++)
        for i in 0..MAX_SLOTS {
            let waypoint_name = format!("Player_{}_Start", i + 1);
            if self.waypoints.contains_key(&waypoint_name) {
                num_start_spots += 1;
            } else {
                break;
            }
        }

        num_start_spots.max(1) // At least 1 player
    }

    /// Get initial camera position waypoint
    /// Matches C++ InitialCameraPosition from MapUtil.cpp:301
    pub fn get_initial_camera_position(&self) -> Option<Coord3D> {
        self.waypoints.get("InitialCameraPosition").copied()
    }

    /// Get bridges
    pub fn get_bridges(&self) -> &[BridgeData] {
        &self.bridges
    }

    /// Reset loader state
    pub fn reset(&mut self) {
        self.heightmap = HeightMap::new();
        self.waypoints.clear();
        self.tech_positions.clear();
        self.supply_positions.clear();
        self.world_dict.clear();
        self.bridges.clear();
        self.polygon_triggers.clear();
        self.waypoint_defs.clear();
        self.waypoint_links.clear();
        self.sides = None;
    }
}

struct MapParseContext {
    heightmap: HeightMap,
    world_dict: HashMap<String, String>,
    waypoints: WaypointMap,
    placed_objects: Vec<(String, Coord3D)>,
    triggers: PolygonTriggerList,
    waypoint_defs: Vec<MapWaypoint>,
    waypoint_links: Vec<(u32, u32)>,
    bridges: Vec<BridgeEndpoints>,
    sides: Option<SidesList>,
    script_templates: Option<ScriptTemplateLookup>,
    bridge_markers: BridgeMarkers,
    common: Option<Arc<CommonServices>>,
}

impl MapParseContext {
    fn new(
        script_templates: Option<ScriptTemplateLookup>,
        common: Option<Arc<CommonServices>>,
    ) -> Self {
        Self {
            heightmap: HeightMap::new(),
            world_dict: HashMap::new(),
            waypoints: HashMap::new(),
            placed_objects: Vec::new(),
            triggers: PolygonTriggerList::new(),
            waypoint_defs: Vec::new(),
            waypoint_links: Vec::new(),
            bridges: Vec::new(),
            sides: None,
            script_templates,
            bridge_markers: BridgeMarkers::default(),
            common,
        }
    }
}

fn map_parse_context(user_data: &mut dyn std::any::Any) -> Option<&mut MapParseContext> {
    user_data.downcast_mut::<MapParseContext>()
}

fn parse_heightmap_size_chunk(
    input: &mut DataChunkInput,
    info: &DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let Some(ctx) = map_parse_context(user_data) else {
        return false;
    };

    ctx.heightmap.width = input.read_int();
    ctx.heightmap.height = input.read_int();
    if info.version >= K_HEIGHT_MAP_VERSION_3 as u16 {
        ctx.heightmap.border_size = input.read_int();
    } else {
        ctx.heightmap.border_size = 0;
    }

    if info.version >= K_HEIGHT_MAP_VERSION_4 as u16 {
        let num_borders = input.read_int().max(0);
        ctx.heightmap.boundaries.clear();
        for _ in 0..num_borders {
            let x = input.read_int();
            let y = input.read_int();
            ctx.heightmap.boundaries.push(ICoord2D::new(x, y));
        }
    }

    let data_size = input.read_int();
    if data_size <= 0 {
        return false;
    }

    let expected = ctx.heightmap.width * ctx.heightmap.height;
    if expected <= 0 || data_size != expected {
        return false;
    }

    let mut data = Vec::with_capacity(data_size as usize);
    for _ in 0..data_size {
        data.push(input.read_byte());
    }
    ctx.heightmap.data = data;

    if info.version == K_HEIGHT_MAP_VERSION_1 as u16 {
        ctx.heightmap.downsample_version_one();
    }

    true
}

fn parse_world_info_chunk(
    input: &mut DataChunkInput,
    _info: &DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let Some(ctx) = map_parse_context(user_data) else {
        return false;
    };

    let dict = input.read_dict();
    ctx.world_dict = dict_to_string_map(&dict, input);
    true
}

fn parse_objects_list_chunk(
    input: &mut DataChunkInput,
    info: &DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let Some(ctx) = map_parse_context(user_data) else {
        return false;
    };
    ctx.bridge_markers = BridgeMarkers::default();
    input.register_parser("Object", &info.label, parse_object_chunk);
    input.parse(user_data)
}

fn parse_object_chunk(
    input: &mut DataChunkInput,
    info: &DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let Some(ctx) = map_parse_context(user_data) else {
        return false;
    };

    let row = MapObjectChunk::read(input, info.version);
    let loc = row.position;
    let dict = &row.properties;
    if let Some(span) = ctx.bridge_markers.observe(row.flags, &row.name, loc, dict) {
        ctx.bridges.push(span);
    }

    if let Some((id, resolved)) = row.waypoint(input) {
        let label1_key = input.intern_name("waypointPathLabel1");
        let label2_key = input.intern_name("waypointPathLabel2");
        let label3_key = input.intern_name("waypointPathLabel3");
        let bidir_key = input.intern_name("waypointPathBiDirectional");

        let waypoint = MapWaypoint {
            id: id as u32,
            name: resolved.clone(),
            location: loc,
            path_label1: dict_get_string(dict, label1_key),
            path_label2: dict_get_string(dict, label2_key),
            path_label3: dict_get_string(dict, label3_key),
            bi_directional: dict_get_bool(dict, bidir_key),
        };

        ctx.waypoints.insert(resolved, loc);
        ctx.waypoint_defs.push(waypoint);
        return true;
    }

    ctx.placed_objects.push((row.name, loc));

    true
}

fn parse_waypoints_list_chunk(
    input: &mut DataChunkInput,
    _info: &DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let Some(ctx) = map_parse_context(user_data) else {
        return false;
    };

    let count = input.read_int().max(0);
    for _ in 0..count {
        let id1 = input.read_int() as u32;
        let id2 = input.read_int() as u32;
        ctx.waypoint_links.push((id1, id2));
    }

    if !input.at_end_of_chunk() {
        log::debug!("WaypointsList chunk has trailing data; ignoring remainder");
    }

    true
}

fn parse_sides_list_chunk(
    input: &mut DataChunkInput,
    info: &DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let Some(ctx) = map_parse_context(user_data) else {
        return false;
    };
    let sides = ctx.sides.get_or_insert_with(SidesList::new);
    match ctx.script_templates.as_ref() {
        Some(templates) => sides.parse_sides_data_chunk_in(input, info, templates.clone()),
        None => sides.parse_sides_data_chunk(input, info),
    }
}

fn parse_polygon_triggers_chunk(
    input: &mut DataChunkInput,
    info: &DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let Some(ctx) = map_parse_context(user_data) else {
        return false;
    };
    ctx.triggers.read_polygon_triggers_data_chunk(input, info.version, || {
        let common = ctx.common.clone().unwrap_or_else(common_services);
        let data = common
            .system
            .global_data
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        data.map_or([0.0; 2], |data| {
            let data = data.read();
            [data.water_extent_x, data.water_extent_y]
        })
    });
    true
}

fn dict_to_string_map(dict: &Dict, input: &DataChunkInput) -> HashMap<String, String> {
    let mut result = HashMap::new();
    let count = dict.get_pair_count();
    for idx in 0..count {
        let Some(key) = dict.get_nth_key(idx) else {
            continue;
        };
        let name = input.name_for_key(key).unwrap_or_default();
        let value = match dict.get_nth_type(idx) {
            Some(DictType::Bool) => dict.get_nth_bool(idx).to_string(),
            Some(DictType::Int) => dict.get_nth_int(idx).to_string(),
            Some(DictType::Real) => dict.get_nth_real(idx).to_string(),
            Some(DictType::AsciiString) => dict.get_nth_ascii_string(idx),
            Some(DictType::UnicodeString) => dict.get_nth_unicode_string(idx),
            None => String::new(),
        };
        if !name.is_empty() {
            result.insert(name, value);
        }
    }
    result
}

fn dict_get_bool(dict: &Dict, key: u32) -> bool {
    match dict.get_type(key) {
        Some(DictType::Bool) => dict.get_bool(key),
        Some(DictType::Int) => dict.get_int(key) != 0,
        _ => false,
    }
}

fn dict_get_string(dict: &Dict, key: u32) -> String {
    match dict.get_type(key) {
        Some(DictType::AsciiString) => dict.get_ascii_string(key),
        Some(DictType::UnicodeString) => dict.get_unicode_string(key),
        _ => String::new(),
    }
}

impl Default for MapLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Map cache for storing metadata about available maps
/// Matches C++ MapCache from MapUtil.cpp
pub struct MapCache {
    maps: HashMap<String, MapMetaData>,
    map_dir: PathBuf,
    user_map_dir: PathBuf,
}

impl MapCache {
    pub fn new() -> Self {
        Self {
            maps: HashMap::new(),
            map_dir: PathBuf::from("Maps"),
            user_map_dir: PathBuf::from("UserData/Maps"),
        }
    }

    /// Get map directory
    pub fn get_map_dir(&self) -> &Path {
        &self.map_dir
    }

    /// Get user map directory
    pub fn get_user_map_dir(&self) -> &Path {
        &self.user_map_dir
    }

    /// Set map directory
    pub fn set_map_dir(&mut self, dir: PathBuf) {
        self.map_dir = dir;
    }

    /// Set user map directory
    pub fn set_user_map_dir(&mut self, dir: PathBuf) {
        self.user_map_dir = dir;
    }

    /// Find map metadata by name
    pub fn find_map(&self, name: &str) -> Option<&MapMetaData> {
        let lowercase_name = name.to_lowercase();
        self.maps.get(&lowercase_name)
    }

    /// Add map to cache
    pub fn add_map(&mut self, metadata: MapMetaData) {
        let lowercase_name = metadata.file_name.to_lowercase();
        self.maps.insert(lowercase_name, metadata);
    }

    /// Update cache with maps from directory
    /// Matches C++ MapCache::updateCache() from MapUtil.cpp:435
    pub fn update_cache(&mut self) -> io::Result<()> {
        // Scan map directories and update metadata
        // In full implementation, would scan both standard and user map dirs
        Ok(())
    }

    /// Clear cache
    pub fn clear(&mut self) {
        self.maps.clear();
    }

    /// Get number of cached maps
    pub fn len(&self) -> usize {
        self.maps.len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.maps.is_empty()
    }
}

impl Default for MapCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_chunk_bytes(label: &str, version: u16, payload: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"CkMp");
        bytes.extend_from_slice(&1i32.to_le_bytes());
        bytes.push(label.len() as u8);
        bytes.extend_from_slice(label.as_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&version.to_le_bytes());
        bytes.extend_from_slice(&(payload.len() as i32).to_le_bytes());
        bytes.extend_from_slice(payload);
        bytes
    }

    fn make_heightmap_chunk(width: i32, height: i32, border_size: i32, samples: &[u8]) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&width.to_le_bytes());
        payload.extend_from_slice(&height.to_le_bytes());
        payload.extend_from_slice(&border_size.to_le_bytes());
        payload.extend_from_slice(&0i32.to_le_bytes());
        payload.extend_from_slice(&(samples.len() as i32).to_le_bytes());
        payload.extend_from_slice(samples);
        make_chunk_bytes("HeightMapData", K_HEIGHT_MAP_VERSION_4 as u16, &payload)
    }

    #[test]
    fn test_coord3d() {
        let coord = Coord3D::new(100.0, 200.0, 50.0);
        assert_eq!(coord.x, 100.0);
        assert_eq!(coord.y, 200.0);
        assert_eq!(coord.z, 50.0);
    }

    #[test]
    fn test_heightmap_extent() {
        let mut heightmap = HeightMap::new();
        heightmap.width = 100;
        heightmap.height = 100;
        heightmap.border_size = 10;

        let extent = heightmap.get_extent();
        let expected_dim = (100 - 2 * 10) as f32 * MAP_XY_FACTOR;

        assert_eq!(extent.lo.x, 0.0);
        assert_eq!(extent.lo.y, 0.0);
        assert_eq!(extent.hi.x, expected_dim);
        assert_eq!(extent.hi.y, expected_dim);
    }

    #[test]
    fn test_map_loader_start_spots() {
        let mut loader = MapLoader::new();

        // Add player start waypoints
        loader
            .waypoints
            .insert("Player_1_Start".to_string(), Coord3D::ZERO);
        loader
            .waypoints
            .insert("Player_2_Start".to_string(), Coord3D::ZERO);
        loader
            .waypoints
            .insert("Player_3_Start".to_string(), Coord3D::ZERO);

        assert_eq!(loader.count_start_spots(), 3);
    }

    #[test]
    fn test_map_loader_no_start_spots() {
        let loader = MapLoader::new();
        // Should return minimum of 1
        assert_eq!(loader.count_start_spots(), 1);
    }

    #[test]
    fn version_one_polygon_water_uses_the_receiving_map_loader_owner() {
        use game_engine::common::common_services::with_common_services;
        use game_engine::common::ini::ini_game_data::ensure_global_data;
        use game_engine::common::system::DataChunkOutput;

        fn with_water_extent(x: f32, y: f32) -> Arc<CommonServices> {
            let common = Arc::new(CommonServices::new());
            with_common_services(&common, || {
                let data = ensure_global_data();
                let mut data = data.write();
                data.water_extent_x = x;
                data.water_extent_y = y;
            });
            common
        }

        let mut out = DataChunkOutput::new();
        out.open_data_chunk("HeightMapData", 3);
        for value in [2, 2, 0, 4] {
            out.write_int(value);
        }
        for _ in 0..4 {
            out.write_byte(0);
        }
        out.close_data_chunk();
        out.open_data_chunk("PolygonTriggers", 1);
        out.write_int(0);
        out.close_data_chunk();
        let bytes = out.into_ckmp_bytes();

        let a = with_water_extent(88.0, 112.0);
        let b = with_water_extent(224.0, 330.0);
        let point = |active: &Arc<CommonServices>, receiving: &Arc<CommonServices>| {
            with_common_services(active, || {
                let decoded =
                    MapLoader::parse_map_data(&bytes, false, None, None, Some(receiving.clone()))
                        .unwrap();
                let water = decoded.triggers.get_by_id(0).unwrap();
                assert!(water.is_water_area());
                *water.get_point(2).unwrap()
            })
        };
        assert_eq!(point(&b, &a), glam::IVec3::new(388, 412, 7));
        assert_eq!(point(&a, &b), glam::IVec3::new(524, 630, 7));
        assert_eq!(point(&b, &a), glam::IVec3::new(388, 412, 7));
    }

    #[test]
    fn test_waypoints_list_parser_tolerates_trailing_bytes() {
        let mut payload = Vec::new();
        payload.extend_from_slice(&1i32.to_le_bytes());
        payload.extend_from_slice(&7i32.to_le_bytes());
        payload.extend_from_slice(&9i32.to_le_bytes());
        payload.push(0xAA);

        let mut input = DataChunkInput::new(make_chunk_bytes("WaypointsList", 0, &payload));
        assert!(input.is_valid_file_type());

        let mut ctx = MapParseContext::new(None, None);
        input.register_parser("WaypointsList", "", parse_waypoints_list_chunk);
        assert!(input.parse(&mut ctx));
        assert_eq!(ctx.waypoint_links, vec![(7, 9)]);
    }

    #[test]
    fn test_map_loader_accepts_compressed_map_chunks() {
        use game_engine::common::system::compression::{
            CompressionLevel, CompressionType, compress_data,
        };

        let raw = make_heightmap_chunk(2, 2, 0, &[1, 2, 3, 4]);
        let compressed = compress_data(&raw, CompressionType::RefPack, CompressionLevel::Default)
            .expect("compress map bytes");

        let mut loader = MapLoader::new();
        loader
            .load_map_from_bytes(&compressed)
            .expect("load compressed chunky map");

        assert_eq!(loader.heightmap.width, 2);
        assert_eq!(loader.heightmap.height, 2);
        assert_eq!(loader.heightmap.data, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_map_loader_loads_retail_lone_eagle_map() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            "../../../../windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
        );
        let bytes = std::fs::read(&path).unwrap_or_else(|err| {
            panic!("read {}: {err}", path.display());
        });
        assert!(bytes.starts_with(b"EAR\0"), "retail map must carry the EAR header");
        let mut loader = MapLoader::new();
        loader
            .load_map_from_bytes(&bytes)
            .expect("load retail Lone Eagle.map");
        assert!(loader.heightmap.width > 0);
        assert!(loader.heightmap.height > 0);
        assert!(!loader.heightmap.data.is_empty());
    }

    #[test]
    fn test_map_cache() {
        let mut cache = MapCache::new();

        let mut metadata = MapMetaData::default();
        metadata.file_name = "TestMap.map".to_string();
        metadata.num_players = 2;

        cache.add_map(metadata);

        assert_eq!(cache.len(), 1);
        assert!(cache.find_map("testmap.map").is_some());
    }
}
