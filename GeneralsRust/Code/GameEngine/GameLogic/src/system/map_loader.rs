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

use crate::common::KindOf;
use crate::helpers::TheThingFactory;
use crate::polygon_trigger::{PolygonTrigger, PolygonTriggerList};
use crate::sides_list::get_sides_list;
use game_engine::common::dict::{Dict, DictType};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::DataChunkInfo;
use game_engine::common::system::DataChunkInput;

/// Waypoint identifier type
pub type WaypointID = u32;

/// Invalid waypoint constant
pub const INVALID_WAYPOINT_ID: WaypointID = 0xFFFFFFFF;

/// Map XY scale factor (from C++ MAP_XY_FACTOR constant)
pub const MAP_XY_FACTOR: f32 = 10.0;

/// Maximum player slots (from C++ MAX_SLOTS)
pub const MAX_SLOTS: usize = 8;

/// Height map data version identifiers (from C++ MapUtil.cpp)
pub const K_HEIGHT_MAP_VERSION_1: u32 = 1;
pub const K_HEIGHT_MAP_VERSION_2: u32 = 2;
pub const K_HEIGHT_MAP_VERSION_3: u32 = 3;
pub const K_HEIGHT_MAP_VERSION_4: u32 = 4;

/// Objects chunk version (from C++ MapUtil.cpp)
pub const K_OBJECTS_VERSION_2: u32 = 2;

const FLAG_BRIDGE_POINT1: i32 = 0x00000010;
const FLAG_BRIDGE_POINT2: i32 = 0x00000020;

/// 3D coordinate structure
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coord3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Coord3D {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn zero() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }

    pub fn origin() -> Self {
        Self::zero()
    }
}

// Re-export ICoord2D from common types
pub use crate::common::ICoord2D;

/// 3D region bounds
#[derive(Debug, Clone, Copy)]
pub struct Region3D {
    pub lo: Coord3D,
    pub hi: Coord3D,
}

impl Region3D {
    pub fn new(lo: Coord3D, hi: Coord3D) -> Self {
        Self { lo, hi }
    }

    pub fn width(&self) -> f32 {
        self.hi.x - self.lo.x
    }

    pub fn height(&self) -> f32 {
        self.hi.y - self.lo.y
    }
}

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
            extent: Region3D::new(Coord3D::origin(), Coord3D::origin()),
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

/// Height map data structure
/// Matches C++ heightmap from MapUtil.cpp
#[derive(Debug)]
pub struct HeightMap {
    pub width: i32,
    pub height: i32,
    pub border_size: i32,
    pub boundaries: Vec<ICoord2D>,
    pub data: Vec<u8>,
}

impl HeightMap {
    pub fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            border_size: 0,
            boundaries: Vec::new(),
            data: Vec::new(),
        }
    }

    /// Get playable map dimensions (excluding border)
    pub fn get_playable_dimensions(&self) -> (i32, i32) {
        let dx = self.width - 2 * self.border_size;
        let dy = self.height - 2 * self.border_size;
        (dx, dy)
    }

    /// Calculate map extent in world coordinates
    /// Matches C++ getExtent() from MapUtil.cpp
    pub fn get_extent(&self) -> Region3D {
        let (dx, dy) = self.get_playable_dimensions();

        Region3D::new(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(dx as f32 * MAP_XY_FACTOR, dy as f32 * MAP_XY_FACTOR, 0.0),
        )
    }
}

/// Bridge data extracted from map files
/// Corresponds to BridgeInfo from C++ TerrainLogic.h
#[derive(Debug, Clone)]
pub struct BridgeData {
    /// The 4 corners of the rectangle that the bridge covers
    pub polygon: Vec<Coord2D>,
    /// Bridge deck height (interpolated between from.z and to.z)
    pub height: f32,
    /// Bridge endpoints
    pub from: Coord3D,
    pub to: Coord3D,
    /// Bridge width
    pub width: f32,
    /// Bridge template name
    pub template_name: String,
}

/// 2D coordinate structure
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coord2D {
    pub x: f32,
    pub y: f32,
}

impl Coord2D {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Complete map data loaded from .map files
/// Matches data needed by TerrainLogic
#[derive(Debug, Clone)]
pub struct MapData {
    /// Map width in cells
    pub width: u32,
    /// Map height in cells
    pub height: u32,
    /// Raw height values (8-bit heightmap)
    pub heightmap: Vec<u8>,
    /// Global water table height (if any)
    pub water_height: Option<f32>,
    /// Bridge structures extracted from map objects
    pub bridges: Vec<BridgeData>,
    /// Surface type per tile (for terrain texture mapping)
    pub texture_tiles: Vec<u8>,
    /// Map boundaries
    pub boundaries: Vec<ICoord2D>,
    /// Border size
    pub border_size: i32,
    /// Polygon trigger areas for scripts and water
    pub polygon_triggers: Vec<PolygonTrigger>,
    /// Waypoints extracted from map objects
    pub waypoints: Vec<MapWaypoint>,
    /// Waypoint link pairs (id1, id2)
    pub waypoint_links: Vec<(u32, u32)>,
}

impl MapData {
    pub fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            heightmap: Vec::new(),
            water_height: None,
            bridges: Vec::new(),
            texture_tiles: Vec::new(),
            boundaries: Vec::new(),
            border_size: 0,
            polygon_triggers: Vec::new(),
            waypoints: Vec::new(),
            waypoint_links: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MapWaypoint {
    pub id: u32,
    pub name: String,
    pub location: Coord3D,
    pub path_label1: String,
    pub path_label2: String,
    pub path_label3: String,
    pub bi_directional: bool,
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
        let path = filename.as_ref();

        // Validate file extension
        if !path.extension().map_or(false, |ext| ext == "map") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "File must have .map extension",
            ));
        }

        // Open file
        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        // Parse map data (simplified - would use DataChunk system in full implementation)
        self.parse_map_data(&buffer, true)?;

        Ok(())
    }

    /// Load map data from raw bytes (used by archive-backed file systems).
    pub fn load_map_from_bytes(&mut self, data: &[u8]) -> io::Result<()> {
        self.parse_map_data(data, true)
    }

    /// Load only the chunks needed to rebuild runtime terrain/sides/waypoint state.
    ///
    /// This intentionally skips full `ObjectsList` parsing because higher-level callers may
    /// already have object placement data from a separate map pipeline and only need legacy
    /// runtime support structures such as sides, waypoints, polygon triggers, and heightmap.
    pub fn load_runtime_support_from_bytes(&mut self, data: &[u8]) -> io::Result<()> {
        self.parse_map_data(data, false)
    }

    /// Parse map binary data
    /// Reference: C++ MapUtil.cpp:214-255 loadMap()
    /// Reference: C++ MapUtil.cpp:167-207 ParseSizeOnly() for heightmap
    /// Reference: C++ MapUtil.cpp:105-152 ParseObjectDataChunk() for objects
    ///
    /// Generals .map files use a chunked binary format:
    /// - Each chunk has: type string, label string, version, size, data
    /// - Main chunks: HeightMapData, WorldInfo, ObjectsList
    fn parse_map_data(&mut self, data: &[u8], include_objects: bool) -> io::Result<()> {
        self.polygon_triggers.clear();

        let mut input = DataChunkInput::new(data.to_vec());
        if !input.is_valid_file_type() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Map file is not a valid DataChunk map",
            ));
        }

        let mut context = MapParseContext::new();
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

        self.heightmap = context.heightmap;
        self.world_dict = context.world_dict;
        self.waypoints = context.waypoints;
        self.tech_positions = context.tech_positions;
        self.supply_positions = context.supply_positions;
        self.polygon_triggers = context.triggers.get_triggers().to_vec();
        self.waypoint_defs = context.waypoint_defs;
        self.waypoint_links = context.waypoint_links;
        self.bridges = context.bridges;

        Ok(())
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
    }
}

struct MapParseContext {
    heightmap: HeightMap,
    world_dict: HashMap<String, String>,
    waypoints: WaypointMap,
    tech_positions: Vec<Coord3D>,
    supply_positions: Vec<Coord3D>,
    triggers: PolygonTriggerList,
    waypoint_defs: Vec<MapWaypoint>,
    waypoint_links: Vec<(u32, u32)>,
    bridges: Vec<BridgeData>,
    pending_bridges: HashMap<String, Vec<PendingBridge>>,
}

impl MapParseContext {
    fn new() -> Self {
        Self {
            heightmap: HeightMap::new(),
            world_dict: HashMap::new(),
            waypoints: HashMap::new(),
            tech_positions: Vec::new(),
            supply_positions: Vec::new(),
            triggers: PolygonTriggerList::new(),
            waypoint_defs: Vec::new(),
            waypoint_links: Vec::new(),
            bridges: Vec::new(),
            pending_bridges: HashMap::new(),
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
        let new_width = (ctx.heightmap.width + 1) / 2;
        let new_height = (ctx.heightmap.height + 1) / 2;
        let mut new_data = vec![0u8; (new_width * new_height).max(0) as usize];
        for i in 0..new_height {
            for j in 0..new_width {
                let src_idx = (2 * i * ctx.heightmap.width + 2 * j) as usize;
                let dst_idx = (i * new_width + j) as usize;
                if let Some(sample) = ctx.heightmap.data.get(src_idx) {
                    if let Some(slot) = new_data.get_mut(dst_idx) {
                        *slot = *sample;
                    }
                }
            }
        }
        ctx.heightmap.width = new_width;
        ctx.heightmap.height = new_height;
        ctx.heightmap.data = new_data;
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
    ctx.world_dict = dict_to_string_map(&dict);
    true
}

fn parse_objects_list_chunk(
    input: &mut DataChunkInput,
    info: &DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
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

    let mut loc = Coord3D::new(input.read_real(), input.read_real(), input.read_real());
    if info.version <= K_OBJECTS_VERSION_2 as u16 {
        loc.z = 0.0;
    }

    let _angle = input.read_real();
    let flags = input.read_int();
    let name = input.read_ascii_string();
    let dict = if info.version >= K_OBJECTS_VERSION_2 as u16 {
        input.read_dict()
    } else {
        Dict::new()
    };

    let waypoint_key = NameKeyGenerator::name_to_key("waypointID");
    if let Some(id) = dict_get_int(&dict, waypoint_key) {
        let name_key = NameKeyGenerator::name_to_key("waypointName");
        let waypoint_name = dict_get_string(&dict, name_key);
        let resolved = if waypoint_name.is_empty() {
            name.clone()
        } else {
            waypoint_name
        };
        let label1_key = NameKeyGenerator::name_to_key("waypointPathLabel1");
        let label2_key = NameKeyGenerator::name_to_key("waypointPathLabel2");
        let label3_key = NameKeyGenerator::name_to_key("waypointPathLabel3");
        let bidir_key = NameKeyGenerator::name_to_key("waypointPathBiDirectional");

        let waypoint = MapWaypoint {
            id: id as u32,
            name: resolved.clone(),
            location: loc,
            path_label1: dict_get_string(&dict, label1_key),
            path_label2: dict_get_string(&dict, label2_key),
            path_label3: dict_get_string(&dict, label3_key),
            bi_directional: dict_get_bool(&dict, bidir_key),
        };

        ctx.waypoints.insert(resolved, loc);
        ctx.waypoint_defs.push(waypoint);
        return true;
    }

    if let Some(template) = TheThingFactory::find_template(&name) {
        if template.is_kind_of(KindOf::TechBuilding) {
            ctx.tech_positions.push(loc);
        } else if template.is_kind_of(KindOf::SupplySourceOnPreview) {
            ctx.supply_positions.push(loc);
        }
    }

    if (flags & (FLAG_BRIDGE_POINT1 | FLAG_BRIDGE_POINT2)) != 0 {
        let is_point1 = (flags & FLAG_BRIDGE_POINT1) != 0;
        add_bridge_point(ctx, name, loc, is_point1);
    }

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
    _user_data: &mut dyn std::any::Any,
) -> bool {
    let sides_list = get_sides_list();
    let Ok(mut sides) = sides_list.write() else {
        return false;
    };
    sides.parse_sides_data_chunk_without_scripts(input, info)
}

fn parse_polygon_triggers_chunk(
    input: &mut DataChunkInput,
    info: &DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let Some(ctx) = map_parse_context(user_data) else {
        return false;
    };
    PolygonTriggerList::parse_polygon_triggers_data_chunk(
        input,
        info,
        &mut ctx.triggers as &mut dyn std::any::Any,
    )
}

fn dict_to_string_map(dict: &Dict) -> HashMap<String, String> {
    let mut result = HashMap::new();
    let count = dict.get_pair_count();
    for idx in 0..count {
        let Some(key) = dict.get_nth_key(idx) else {
            continue;
        };
        let name = NameKeyGenerator::key_to_name(key).unwrap_or_default();
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

fn dict_get_int(dict: &Dict, key: u32) -> Option<i32> {
    match dict.get_type(key) {
        Some(DictType::Int) => Some(dict.get_int(key)),
        _ => None,
    }
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

#[derive(Debug)]
struct PendingBridge {
    from: Option<Coord3D>,
    to: Option<Coord3D>,
}

fn add_bridge_point(ctx: &mut MapParseContext, name: String, loc: Coord3D, is_point1: bool) {
    let entry = ctx.pending_bridges.entry(name.clone()).or_default();

    if is_point1 {
        for index in 0..entry.len() {
            if entry[index].from.is_none() && entry[index].to.is_some() {
                let to = entry[index].to.take().unwrap_or(loc);
                let from = loc;
                entry.swap_remove(index);
                ctx.bridges.push(build_bridge_data(&name, from, to));
                return;
            }
        }
        entry.push(PendingBridge {
            from: Some(loc),
            to: None,
        });
    } else {
        for index in 0..entry.len() {
            if entry[index].to.is_none() && entry[index].from.is_some() {
                let from = entry[index].from.take().unwrap_or(loc);
                let to = loc;
                entry.swap_remove(index);
                ctx.bridges.push(build_bridge_data(&name, from, to));
                return;
            }
        }
        entry.push(PendingBridge {
            from: None,
            to: Some(loc),
        });
    }
}

fn build_bridge_data(name: &str, from: Coord3D, to: Coord3D) -> BridgeData {
    let width = bridge_width_from_template(name).unwrap_or(MAP_XY_FACTOR * 2.0);
    BridgeData::new(from, to, width, name.to_string())
}

fn bridge_width_from_template(name: &str) -> Option<f32> {
    let template = TheThingFactory::find_template(name)?;
    let geom = template.get_template_geometry_info();
    let width = (geom.get_minor_radius() * 2.0).max(0.0);
    if width > 0.0 {
        Some(width)
    } else {
        None
    }
}

impl BridgeData {
    /// Create bridge data from endpoints and width
    /// Reference: C++ TerrainLogic.cpp:196-304 Bridge::Bridge() constructor
    ///
    /// # Arguments
    /// * `from` - Starting point of bridge
    /// * `to` - Ending point of bridge
    /// * `width` - Width of the bridge
    /// * `template_name` - Bridge template name
    ///
    /// # Returns
    /// BridgeData structure with calculated polygon
    pub fn new(from: Coord3D, to: Coord3D, width: f32, template_name: String) -> Self {
        // Calculate bridge polygon (4 corners)
        // Reference: C++ TerrainLogic.cpp:203-218

        // Vector along bridge
        let dx = to.x - from.x;
        let dy = to.y - from.y;
        let length = (dx * dx + dy * dy).sqrt();

        // Normalized perpendicular vector
        let perp_x = -dy / length;
        let perp_y = dx / length;

        // Half width offset
        let half_width = width / 2.0;

        // Calculate 4 corners
        let from_left = Coord2D::new(from.x + perp_x * half_width, from.y + perp_y * half_width);
        let from_right = Coord2D::new(from.x - perp_x * half_width, from.y - perp_y * half_width);
        let to_left = Coord2D::new(to.x + perp_x * half_width, to.y + perp_y * half_width);
        let to_right = Coord2D::new(to.x - perp_x * half_width, to.y - perp_y * half_width);

        // Average height of bridge deck
        let avg_height = (from.z + to.z) / 2.0;

        BridgeData {
            polygon: vec![from_left, from_right, to_right, to_left],
            height: avg_height,
            from,
            to,
            width,
            template_name,
        }
    }

    /// Get interpolated height at position along bridge
    /// Reference: C++ TerrainLogic.cpp Bridge::getBridgeHeight()
    pub fn get_height_at(&self, x: f32, y: f32) -> f32 {
        // Calculate position along bridge (0.0 to 1.0)
        let bridge_dx = self.to.x - self.from.x;
        let bridge_dy = self.to.y - self.from.y;
        let bridge_length_sq = bridge_dx * bridge_dx + bridge_dy * bridge_dy;

        if bridge_length_sq == 0.0 {
            return self.from.z;
        }

        let pos_dx = x - self.from.x;
        let pos_dy = y - self.from.y;
        let dot = pos_dx * bridge_dx + pos_dy * bridge_dy;
        let t = (dot / bridge_length_sq).clamp(0.0, 1.0);

        // Linear interpolation between from.z and to.z
        self.from.z + (self.to.z - self.from.z) * t
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
            .insert("Player_1_Start".to_string(), Coord3D::origin());
        loader
            .waypoints
            .insert("Player_2_Start".to_string(), Coord3D::origin());
        loader
            .waypoints
            .insert("Player_3_Start".to_string(), Coord3D::origin());

        assert_eq!(loader.count_start_spots(), 3);
    }

    #[test]
    fn test_map_loader_no_start_spots() {
        let loader = MapLoader::new();
        // Should return minimum of 1
        assert_eq!(loader.count_start_spots(), 1);
    }

    #[test]
    fn test_waypoints_list_parser_tolerates_trailing_bytes() {
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

        let mut payload = Vec::new();
        payload.extend_from_slice(&1i32.to_le_bytes());
        payload.extend_from_slice(&7i32.to_le_bytes());
        payload.extend_from_slice(&9i32.to_le_bytes());
        payload.push(0xAA);

        let mut input = DataChunkInput::new(make_chunk_bytes("WaypointsList", 0, &payload));
        assert!(input.is_valid_file_type());

        let mut ctx = MapParseContext::new();
        input.register_parser("WaypointsList", "", parse_waypoints_list_chunk);
        assert!(input.parse(&mut ctx));
        assert_eq!(ctx.waypoint_links, vec![(7, 9)]);
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
