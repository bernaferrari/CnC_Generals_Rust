// C++ ownership: MapObject/SidesList/WorldHeightMap record types shared by map loading.

/// Minimal object placement extracted from a chunky map.
#[derive(Debug, Clone)]
pub struct PlacedObject {
    pub template: String,
    pub name: Option<String>,
    pub position: Coord3D,
    pub rotation: Option<f32>,
    pub team_name: Option<String>,
    pub player_id: Option<u32>,
    pub upgrade: Option<String>,
    /// C++ Dict `objectUnsellable` / OBJECT_STATUS_SCRIPT_UNSELLABLE.
    pub unsellable: Option<bool>,
    /// C++ Dict `objectEnabled` / OBJECT_STATUS_SCRIPT_DISABLED when false.
    pub enabled: Option<bool>,
    /// C++ Dict `objectPowered` / OBJECT_STATUS_SCRIPT_UNPOWERED when false.
    pub powered: Option<bool>,
    /// C++ Dict `objectIndestructible` / ActiveBody::setIndestructible.
    pub indestructible: Option<bool>,

    /// C++ Dict `objectWeather` (`Object.cpp:3595-3605`): 0 follow map, 1 force
    /// `MODELCONDITION_SNOW` clear, 2 force set. Missing key is follow.
    pub object_weather: Option<i32>,
    /// Typed C++ MapObject Dict. Live spawn calls leftover
    /// `update_obj_values_from_map_properties` from this bag.
    pub properties: Dict,
}

/// C++ SidesList build-list entry residual (skirmish army / base placements).
#[derive(Debug, Clone)]
pub struct SideBuildEntry {
    pub building_name: String,
    pub template: String,
    pub position: Coord3D,
    pub angle: f32,
    pub initially_built: bool,
    pub num_rebuilds: i32,
    /// Side index in SidesList (0..N).
    pub side_index: u32,
    pub script_name: Option<String>,
    pub health: Option<i32>,
    pub whiner: Option<bool>,
    pub unsellable: Option<bool>,
    pub repairable: Option<bool>,
}

/// Top-level metadata parsed from a map file.
#[derive(Debug, Clone, Default)]
pub struct MapMetadata {
    pub objects: Vec<PlacedObject>,
    /// Wave 831: SidesList build-list entries (skirmish faction bases).
    pub side_builds: Vec<SideBuildEntry>,
    /// Wave 831: Player_N_Start / Player_N_Rally waypoints (name, position).
    pub start_waypoints: Vec<(String, Coord3D)>,
    pub world_min: Option<Coord3D>,
    pub world_max: Option<Coord3D>,
    pub initial_camera_position: Option<Coord3D>,
    /// Optional heightmap path located alongside the .map file (e.g. .hmp/.tga/.raw)
    pub heightmap_path: Option<PathBuf>,
    /// Optional skybox texture names (order: front, back, left, right, top)
    pub skybox_textures: Option<[String; 5]>,
    pub ambient_color: Option<[f32; 3]>,
    pub sun_color: Option<[f32; 3]>,
    pub sky_color: Option<[f32; 3]>,
    pub sun_direction: Option<[f32; 3]>,
    pub fog_color: Option<[f32; 3]>,
    pub fog_start: Option<f32>,
    pub fog_end: Option<f32>,
    /// C++ `m_terrainObjectsLighting[tod][0]` — unit/shadow scene light.
    pub objects_ambient_color: Option<[f32; 3]>,
    pub objects_sun_color: Option<[f32; 3]>,
    pub objects_sun_direction: Option<[f32; 3]>,
    /// Extra object lights 1..2 for the map TOD (chunk v2+).
    pub objects_extra_lights: Vec<[f32; 9]>,
    /// Extra terrain lights 1..2 for the map TOD (chunk v3+).
    pub terrain_extra_lights: Vec<[f32; 9]>,
}

#[derive(Debug, Clone)]
pub struct RuntimeWaypoint {
    pub id: u32,
    pub name: String,
    pub location: Coord3D,
    pub path_label1: String,
    pub path_label2: String,
    pub path_label3: String,
    pub bi_directional: bool,
}

#[derive(Debug, Clone)]
struct RuntimeBridgeEndpoint {
    template_name: String,
    location: Coord3D,
    is_point1: bool,
}

#[derive(Debug, Clone)]
struct RuntimeMapObjectStub {
    template_name: String,
    location: Coord3D,
    flags: i32,
}

#[derive(Debug, Clone)]
pub struct RuntimeRoadSegment {
    pub template_name: String,
    pub from: Coord3D,
    pub to: Coord3D,
    pub width: f32,
    pub width_in_texture: f32,
    pub road_type_id: u32,
    pub start_is_angled: bool,
    pub start_is_join: bool,
    pub end_is_angled: bool,
    pub end_is_join: bool,
    pub curve_radius: f32,
}

#[derive(Debug, Default)]
struct PendingRuntimeBridge {
    from: Option<Coord3D>,
    to: Option<Coord3D>,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeSidesData {
    pub side_dicts: Vec<Dict>,
    pub team_dicts: Vec<Dict>,
    /// Wave 831: build-list placements per side.
    pub side_builds: Vec<SideBuildEntry>,
}

/// Decoded heightmap data extracted from the `HeightMapData` chunk.
#[derive(Debug, Clone)]
pub struct HeightMapData {
    pub width: i32,
    pub height: i32,
    pub border_size: i32,
    pub boundaries: Vec<(i32, i32)>,
    /// Raw 8-bit height samples in row-major order (size = width * height).
    pub data: Vec<u8>,
}

/// Decoded `BlendTileData` fields needed by C++ terrain tile/color queries.
#[derive(Debug, Clone)]
pub struct BlendTileData {
    pub tile_ndxes: Vec<i16>,
    pub blend_tile_ndxes: Vec<i16>,
    /// C++ `m_extraBlendTileNdxes` (v6+). Parallel to `blend_tile_ndxes`.
    pub extra_blend_tile_ndxes: Vec<i16>,
    pub texture_classes: Vec<BlendTileTextureClass>,
    /// C++ `m_edgeTextureClasses` (BlendTileData v4+).
    pub edge_texture_classes: Vec<BlendTileTextureClass>,
    /// C++ `m_blendedTiles[1..]` — horiz/vert/diagonal + inverted + edge class.
    pub blended_tiles: Vec<BlendTileInfo>,
}

#[derive(Debug, Clone)]
pub struct BlendTileTextureClass {
    pub first_tile: i32,
    pub num_tiles: i32,
    pub width: i32,
    pub name: String,
}

/// C++ `TBlendTileInfo` (WorldHeightMap.h).
#[derive(Debug, Clone, Default)]
pub struct BlendTileInfo {
    pub blend_ndx: i32,
    pub horiz: u8,
    pub vert: u8,
    pub right_diagonal: u8,
    pub left_diagonal: u8,
    pub inverted: u8,
    pub long_diagonal: u8,
    pub custom_blend_edge_class: i32,
}

/// Result returned after decoding a map file.
pub struct MapScriptLoadResult {
    pub source_path: PathBuf,
    pub script_lists: Vec<ScriptList>,
    pub total_scripts: usize,
}
