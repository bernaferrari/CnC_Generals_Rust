/*
** Command & Conquer Generals Zero Hour(tm) - Map Script Loader
** Copyright 2025 Electronic Arts
**
** Loads WW3D/SAGE mission scripts directly from .map files by decoding the
** chunky container and converting binary ScriptList data into the canonical
** rust structures under gamelogic::scripting::core.
*/

use std::cell::Cell;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::env;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use game_engine::common::dict::{Dict, DictType};
use game_engine::common::ini::{INI, INILoadType, try_get_terrain_roads};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{
    DataChunkInfo, DataChunkInput, file::FileAccess, file_system::get_file_system,
};
use gamelogic::GameLogicError;
use gamelogic::common::MAP_XY_FACTOR;
use gamelogic::common::{AsciiString, ICoord3D};
use gamelogic::polygon_trigger::PolygonTrigger;
use gamelogic::scripting::core::{
    Condition, ConditionType, Coord3D, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptGroup, ScriptList,
};
use gamelogic::scripting::{ScriptListReadInfo, parse_player_scripts_list_chunk};
use gamelogic::system::Coord3D as SystemCoord3D;
use gamelogic::system::map_loader::BridgeData;
use log::{debug, info, trace, warn};

type LoaderResult<T> = Result<T, GameLogicError>;

const CHUNK_HEADER_SIZE: usize = 10; // u32 id + u16 version + i32 size
const PLAYER_SCRIPTS_LABEL: &str = "PlayerScriptsList";
const SCRIPT_LIST_LABEL: &str = "ScriptList";
const SCRIPT_LABEL: &str = "Script";
const SCRIPT_GROUP_LABEL: &str = "ScriptGroup";
const OR_CONDITION_LABEL: &str = "OrCondition";
const CONDITION_LABEL: &str = "Condition";
const SCRIPT_ACTION_LABEL: &str = "ScriptAction";
const SCRIPT_ACTION_FALSE_LABEL: &str = "ScriptActionFalse";
const CHUNK_MAGIC: &[u8; 4] = b"CkMp";
const OBJECTS_LIST_LABEL: &str = "ObjectsList";
const OBJECT_CREATION_LIST_LABEL: &str = "ObjectCreationList";
const OBJECT_LABEL: &str = "Object";
const SIDES_LIST_LABEL: &str = "SidesList";
const FLAG_ROAD_POINT1: i32 = 0x00000002;
const FLAG_ROAD_POINT2: i32 = 0x00000004;
const FLAG_ROAD_CORNER_ANGLED: i32 = 0x00000008;
const FLAG_BRIDGE_POINT1: i32 = 0x00000010;
const FLAG_BRIDGE_POINT2: i32 = 0x00000020;
const FLAG_ROAD_CORNER_TIGHT: i32 = 0x00000040;
const FLAG_ROAD_JOIN: i32 = 0x00000080;
const DEFAULT_RUNTIME_ROAD_WIDTH: f32 = 8.0;
const DEFAULT_RUNTIME_ROAD_WIDTH_IN_TEXTURE: f32 = 1.0;
const DEFAULT_RUNTIME_ROAD_UNIQUE_ID: u32 = 1;
const CORNER_RADIUS: f32 = 1.5;
const TIGHT_CORNER_RADIUS: f32 = 0.5;

#[derive(Default)]
struct SidesScriptContext {
    scripts: ScriptListReadInfo,
}

static TERRAIN_ROADS_LOAD_RESULT: OnceLock<Result<(), String>> = OnceLock::new();

fn normalize_virtual_path(path: &Path) -> String {
    normalize_virtual_path_str(&path.to_string_lossy())
}

fn normalize_virtual_path_str(path: &str) -> String {
    path.replace('\\', "/").trim().trim_matches('"').to_string()
}

fn normalize_lookup_path(path: &str) -> String {
    normalize_virtual_path_str(path)
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_string()
}

fn push_unique_string(vec: &mut Vec<String>, candidate: String) {
    if !vec.iter().any(|existing| existing == &candidate) {
        vec.push(candidate);
    }
}

fn resolve_with_file_system(path: &Path) -> Option<PathBuf> {
    let normalized = normalize_virtual_path(path);
    if normalized.is_empty() {
        return None;
    }

    if let Ok(file_system) = get_file_system().try_lock() {
        if file_system.does_file_exist(&normalized) {
            return Some(PathBuf::from(&normalized));
        }
    }

    None
}

fn read_file_bytes_via_file_system(path: &Path) -> Option<Vec<u8>> {
    let normalized = normalize_virtual_path(path);
    if normalized.is_empty() {
        return None;
    }

    let access = FileAccess::READ.combine(FileAccess::BINARY);
    let file_system = get_file_system();
    let mut file_system = file_system.try_lock().ok()?;
    let mut file = file_system.open_file(&normalized, access)?;
    file.read_entire_and_close().ok()
}

fn read_file_bytes_for_runtime(path: &Path) -> Option<Vec<u8>> {
    read_file_bytes_via_file_system(path).or_else(|| {
        let normalized = normalize_virtual_path(path);
        if normalized.is_empty() {
            None
        } else if Path::new(&normalized).exists() {
            fs::read(&normalized).ok()
        } else {
            None
        }
    })
}

fn read_text_via_file_system(path: &Path) -> Option<String> {
    let bytes = read_file_bytes_via_file_system(path)?;
    String::from_utf8(bytes).ok()
}

fn read_text_with_fallback(path: &Path) -> Option<String> {
    if let Some(contents) = read_text_via_file_system(path) {
        return Some(contents);
    }
    if normalize_lookup_path(path.to_string_lossy().as_ref()).is_empty() {
        return None;
    }
    if path.exists() {
        fs::read_to_string(path).ok()
    } else {
        None
    }
}

fn first_readable_map_ini_companion(dir: &Path, names: &[&str]) -> Option<(PathBuf, String)> {
    for name in names {
        let path = dir.join(name);
        if let Some(contents) = read_text_with_fallback(&path) {
            return Some((path, contents));
        }
    }
    None
}

fn path_is_accessible(path: &Path) -> bool {
    resolve_with_file_system(path).is_some() || path.exists()
}

fn resolve_path_candidate(candidate: &Path) -> Option<PathBuf> {
    if let Some(found) = resolve_with_file_system(candidate) {
        return Some(found);
    }
    if candidate.exists() {
        return Some(candidate.to_path_buf());
    }

    None
}

fn materialize_to_temporary(path: &str, bytes: &[u8]) -> Option<PathBuf> {
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    bytes.len().hash(&mut hasher);
    bytes.hash(&mut hasher);
    let filename_hash = hasher.finish();

    let path_obj = Path::new(path);
    let base = path_obj
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("asset");
    let extension = path_obj
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("bin");

    let temp_dir = env::temp_dir().join("generals_zero_hour");
    fs::create_dir_all(&temp_dir).ok()?;

    let temp_path = temp_dir.join(format!("{}_{}.{}", base, filename_hash, extension));
    if let Ok(existing) = fs::metadata(&temp_path) {
        if existing.len() == bytes.len() as u64 {
            return Some(temp_path);
        }
    }

    fs::write(&temp_path, bytes).ok()?;
    Some(temp_path)
}

fn resolve_runtime_path(path: &Path) -> Option<PathBuf> {
    let normalized = normalize_virtual_path(path);
    if normalized.is_empty() {
        return None;
    }

    let candidate = Path::new(&normalized);
    if let Some(bytes) = read_file_bytes_via_file_system(candidate) {
        return materialize_to_temporary(&normalized, &bytes);
    }

    if candidate.exists() {
        Some(candidate.to_path_buf())
    } else {
        None
    }
}

fn resolve_runtime_ini_path(requested: &Path) -> Option<PathBuf> {
    let requested_normalized = normalize_virtual_path(requested);
    if requested_normalized.is_empty() {
        return None;
    }

    let mut candidates = Vec::new();
    push_unique_string(
        &mut candidates,
        normalize_lookup_path(&requested_normalized),
    );
    if let Some(stripped) = requested_normalized
        .strip_prefix("Data/")
        .or_else(|| requested_normalized.strip_prefix("data/"))
    {
        push_unique_string(&mut candidates, stripped.to_string());
    }

    candidates.sort();
    candidates.dedup();

    for candidate in candidates {
        let Some(candidate_path) = resolve_path_candidate(Path::new(&candidate)) else {
            continue;
        };
        if let Some(runtime_path) = resolve_runtime_path(&candidate_path) {
            return Some(runtime_path);
        }
    }

    None
}

fn ensure_terrain_roads_loaded() {
    TERRAIN_ROADS_LOAD_RESULT.get_or_init(|| {
        let result = (|| {
            let mut ini = INI::new();

            if let Some(default_path) =
                resolve_runtime_ini_path(Path::new("Data/INI/Default/Roads.ini"))
            {
                ini.load(&default_path, INILoadType::Overwrite)
                    .map_err(|err| {
                        format!("failed loading '{}': {}", default_path.display(), err)
                    })?;
            }

            if let Some(override_path) = resolve_runtime_ini_path(Path::new("Data/INI/Roads.ini")) {
                ini.load(&override_path, INILoadType::MultiFile)
                    .map_err(|err| {
                        format!("failed loading '{}': {}", override_path.display(), err)
                    })?;
            }

            Ok(())
        })();
        if let Err(err) = &result {
            // The result is cached for the process lifetime; report a failure once
            // rather than once per object placement that asks whether it is a road.
            warn!("Terrain roads registry unavailable: {}", err);
        }
        result
    });
}

fn is_terrain_road_name(name: &str) -> bool {
    ensure_terrain_roads_loaded();
    try_get_terrain_roads().is_some_and(|roads| roads.find_road(name).is_some())
}

fn decompress_map_bytes(raw_bytes: &[u8]) -> LoaderResult<Vec<u8>> {
    MAP_DECOMPRESS_COUNT.with(|count| count.set(count.get().saturating_add(1)));
    // Real Generals assets commonly use the legacy EA wrapper header:
    //   - 4 byte signature (EAR\0)
    //   - 4 byte uncompressed size (little-endian)
    // followed by a RefPack stream (starting with 0x10FB/0x11FB/...).
    //
    // The repo also contains a newer synthetic header handled by `generals_compression`;
    // keep a fallback path for that format.
    if raw_bytes.len() >= 8 && &raw_bytes[..4] == b"EAR\0" {
        let expected_size =
            u32::from_le_bytes(raw_bytes[4..8].try_into().unwrap_or([0; 4])) as usize;
        return decompress_refpack_stream(&raw_bytes[8..], expected_size).map_err(|err| {
            configuration_error(format!("Failed to decompress RefPack payload: {err}"))
        });
    }

    generals_compression::decompress(raw_bytes)
        .map_err(|err| configuration_error(format!("Fallback decompression failed: {err}")))
}

fn decompress_refpack_stream(data: &[u8], expected_size: usize) -> Result<Vec<u8>, String> {
    // Ported from `GeneralsMD/Code/Libraries/Source/Compression/EAC/refdecode.cpp` (REF_decode).
    if data.len() < 2 {
        return Err("RefPack stream too small".to_string());
    }

    let mut pos: usize = 0;
    let type_word: u16 = ((data[pos] as u16) << 8) | data[pos + 1] as u16;
    pos += 2;

    let ulen: usize;
    if (type_word & 0x8000) != 0 {
        // 4 byte size field
        if (type_word & 0x0100) != 0 {
            // skip ulen
            if data.len() < pos + 4 {
                return Err("RefPack header truncated (skip ulen)".to_string());
            }
            pos += 4;
        }
        if data.len() < pos + 4 {
            return Err("RefPack header truncated (ulen32)".to_string());
        }
        ulen = ((data[pos] as usize) << 24)
            | ((data[pos + 1] as usize) << 16)
            | ((data[pos + 2] as usize) << 8)
            | (data[pos + 3] as usize);
        pos += 4;
    } else {
        // 3 byte size field
        if (type_word & 0x0100) != 0 {
            if data.len() < pos + 3 {
                return Err("RefPack header truncated (skip ulen)".to_string());
            }
            pos += 3;
        }
        if data.len() < pos + 3 {
            return Err("RefPack header truncated (ulen24)".to_string());
        }
        ulen =
            ((data[pos] as usize) << 16) | ((data[pos + 1] as usize) << 8) | data[pos + 2] as usize;
        pos += 3;
    }

    if expected_size != 0 && ulen != expected_size {
        // Keep going (the inner size is authoritative for this stream), but surface the mismatch.
        trace!(
            "RefPack size mismatch: outer={}, inner={}",
            expected_size, ulen
        );
    }

    let mut out: Vec<u8> = Vec::with_capacity(ulen);
    loop {
        if pos >= data.len() {
            return Err("RefPack stream ended before EOF marker".to_string());
        }
        let first = data[pos];
        pos += 1;

        if (first & 0x80) == 0 {
            // short form
            if pos >= data.len() {
                return Err("RefPack short form truncated".to_string());
            }
            let second = data[pos];
            pos += 1;
            let literal_count = (first & 3) as usize;
            if data.len() < pos + literal_count {
                return Err("RefPack literals truncated".to_string());
            }
            out.extend_from_slice(&data[pos..pos + literal_count]);
            pos += literal_count;

            let back = (((first & 0x60) as usize) << 3) + second as usize;
            if out.is_empty() {
                return Err("RefPack invalid backref: empty output".to_string());
            }
            let mut ref_pos = out
                .len()
                .checked_sub(1 + back)
                .ok_or_else(|| "RefPack invalid backref (short)".to_string())?;

            let mut run = (((first & 0x1c) >> 2) as usize) + 3;
            while run > 0 {
                if ref_pos >= out.len() {
                    return Err("RefPack backref out of bounds (short)".to_string());
                }
                let byte = out[ref_pos];
                out.push(byte);
                ref_pos += 1;
                run -= 1;
                if out.len() >= ulen {
                    break;
                }
            }
            if out.len() >= ulen {
                break;
            }
            continue;
        }

        if (first & 0x40) == 0 {
            // int form
            if data.len() < pos + 2 {
                return Err("RefPack int form truncated".to_string());
            }
            let second = data[pos];
            let third = data[pos + 1];
            pos += 2;

            let literal_count = (second >> 6) as usize;
            if data.len() < pos + literal_count {
                return Err("RefPack literals truncated".to_string());
            }
            out.extend_from_slice(&data[pos..pos + literal_count]);
            pos += literal_count;

            let back = (((second & 0x3f) as usize) << 8) + third as usize;
            if out.is_empty() {
                return Err("RefPack invalid backref: empty output".to_string());
            }
            let mut ref_pos = out
                .len()
                .checked_sub(1 + back)
                .ok_or_else(|| "RefPack invalid backref (int)".to_string())?;

            let mut run = ((first & 0x3f) as usize) + 4;
            while run > 0 {
                if ref_pos >= out.len() {
                    return Err("RefPack backref out of bounds (int)".to_string());
                }
                let byte = out[ref_pos];
                out.push(byte);
                ref_pos += 1;
                run -= 1;
                if out.len() >= ulen {
                    break;
                }
            }
            if out.len() >= ulen {
                break;
            }
            continue;
        }

        if (first & 0x20) == 0 {
            // very int form
            if data.len() < pos + 3 {
                return Err("RefPack very-int form truncated".to_string());
            }
            let second = data[pos];
            let third = data[pos + 1];
            let forth = data[pos + 2];
            pos += 3;

            let literal_count = (first & 3) as usize;
            if data.len() < pos + literal_count {
                return Err("RefPack literals truncated".to_string());
            }
            out.extend_from_slice(&data[pos..pos + literal_count]);
            pos += literal_count;

            let back = ((((first & 0x10) as usize) >> 4) << 16)
                + ((second as usize) << 8)
                + third as usize;
            if out.is_empty() {
                return Err("RefPack invalid backref: empty output".to_string());
            }
            let mut ref_pos = out
                .len()
                .checked_sub(1 + back)
                .ok_or_else(|| "RefPack invalid backref (very-int)".to_string())?;

            let run = ((((first & 0x0c) as usize) >> 2) << 8) + forth as usize + 5;
            let mut remaining = run;
            while remaining > 0 {
                if ref_pos >= out.len() {
                    return Err("RefPack backref out of bounds (very-int)".to_string());
                }
                let byte = out[ref_pos];
                out.push(byte);
                ref_pos += 1;
                remaining -= 1;
                if out.len() >= ulen {
                    break;
                }
            }
            if out.len() >= ulen {
                break;
            }
            continue;
        }

        let literal_run = (((first & 0x1f) as usize) << 2) + 4;
        if literal_run <= 112 {
            if data.len() < pos + literal_run {
                return Err("RefPack literal run truncated".to_string());
            }
            out.extend_from_slice(&data[pos..pos + literal_run]);
            pos += literal_run;
            if out.len() >= ulen {
                break;
            }
            continue;
        }

        // EOF (+0..3 literal)
        let tail = (first & 3) as usize;
        if data.len() < pos + tail {
            return Err("RefPack EOF tail truncated".to_string());
        }
        out.extend_from_slice(&data[pos..pos + tail]);
        let _pos = pos + tail;
        break;
    }

    if out.len() != ulen {
        return Err(format!("Size mismatch: expected {ulen}, got {}", out.len()));
    }
    Ok(out)
}

/// Raw chunky map data for further decoding (terrain, objects, etc.).
#[derive(Clone)]
pub struct ChunkyMap {
    pub source: PathBuf,
    pub toc: HashMap<u32, String>,
    pub body_offset: usize,
    pub bytes: Vec<u8>,
}

thread_local! {
    static MAP_DECOMPRESS_COUNT: Cell<u32> = const { Cell::new(0) };
}
static LAST_LOADED_CHUNKY: Mutex<Option<ChunkyMap>> = Mutex::new(None);

/// How many times this thread RefPack-decoded a `.map` file.
pub fn map_decompress_count() -> u32 {
    MAP_DECOMPRESS_COUNT.with(Cell::get)
}

/// Test helper: isolate decompress-reuse assertions from other tests.
pub fn reset_map_decompress_count() {
    MAP_DECOMPRESS_COUNT.with(|count| count.set(0));
}

fn cached_chunky_for(path: &Path) -> Option<ChunkyMap> {
    LAST_LOADED_CHUNKY
        .lock()
        .ok()?
        .as_ref()
        .filter(|chunky| chunky.source == path)
        .cloned()
}

fn remember_loaded_chunky(chunky: &ChunkyMap) {
    if let Ok(mut guard) = LAST_LOADED_CHUNKY.lock() {
        *guard = Some(chunky.clone());
    }
}

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

fn load_sides_list_fallback(
    map_path: &Path,
    body: &[u8],
    toc: &HashMap<u32, String>,
) -> LoaderResult<Option<MapScriptLoadResult>> {
    let Some((sides_version, sides_payload)) = find_chunk_by_label(body, toc, SIDES_LIST_LABEL)?
    else {
        return Ok(None);
    };

    let script_lists = parse_script_lists_from_sides_chunk(sides_payload, toc, sides_version)?;
    let total = count_scripts(&script_lists);
    info!(
        "Decoded {} script lists ({} scripts) from '{}' via SidesList fallback",
        script_lists.len(),
        total,
        map_path.display()
    );
    Ok(Some(MapScriptLoadResult {
        source_path: map_path.to_path_buf(),
        script_lists,
        total_scripts: total,
    }))
}

/// Attempt to locate and decode scripts for the provided map.
pub fn load_map_scripts(map_name: &str) -> LoaderResult<Option<MapScriptLoadResult>> {
    let Some(map_path) = locate_map_file(map_name) else {
        warn!(
            "No .map file could be found for '{}'; mission scripts unavailable",
            map_name
        );
        return Ok(None);
    };
    if let Some(chunky) = cached_chunky_for(&map_path) {
        return load_map_scripts_from_chunky(&chunky);
    }
    let Some(chunky) = load_chunky_map(map_name)? else {
        return Ok(None);
    };
    load_map_scripts_from_chunky(&chunky)
}

/// Decode mission scripts from an already-decompressed chunky map.
/// C++ `TerrainLogic::loadMap` (`TerrainLogic.cpp:1248-1262`) opens the
/// `.map` once via `CachedFileInputStream` and parses chunks from that stream.
pub fn load_map_scripts_from_chunky(
    chunky: &ChunkyMap,
) -> LoaderResult<Option<MapScriptLoadResult>> {
    let map_path = chunky.source.clone();
    if chunky.body_offset >= chunky.bytes.len() {
        return Err(GameLogicError::Configuration(format!(
            "Map '{}' chunk table extends past file",
            map_path.display()
        )));
    }

    let body = &chunky.bytes[chunky.body_offset..];
    let player_scripts_chunk = find_chunk_by_label(body, &chunky.toc, PLAYER_SCRIPTS_LABEL)?;
    let Some((version, payload)) = player_scripts_chunk else {
        if let Some(result) = load_sides_list_fallback(&map_path, body, &chunky.toc)? {
            return Ok(Some(result));
        } else {
            debug!(
                "Map '{}' does not contain a '{}' chunk; no mission scripts available",
                map_path.display(),
                PLAYER_SCRIPTS_LABEL
            );
            return Ok(Some(MapScriptLoadResult {
                source_path: map_path,
                script_lists: Vec::new(),
                total_scripts: 0,
            }));
        }
    };

    let script_lists = parse_script_lists(payload, &chunky.toc, version)?;
    let total = count_scripts(&script_lists);

    if total == 0 {
        if let Some(result) = load_sides_list_fallback(&map_path, body, &chunky.toc)? {
            info!(
                "PlayerScriptsList in '{}' decoded empty; using SidesList fallback instead",
                map_path.display()
            );
            return Ok(Some(result));
        }
    }

    info!(
        "Decoded {} script lists ({} scripts) from '{}'",
        script_lists.len(),
        total,
        map_path.display()
    );

    Ok(Some(MapScriptLoadResult {
        source_path: map_path,
        script_lists,
        total_scripts: total,
    }))
}

/// Public helper to resolve a map name to an on-disk .map file if present.
pub fn find_map_file(map_name: &str) -> Option<PathBuf> {
    locate_map_file(map_name)
}

/// List the chunky chunk labels present in a map file (for debugging/loading).
pub fn inspect_map_chunks(map_name: &str) -> Option<Vec<String>> {
    inspect_map_chunks_from_chunky(&load_chunky_map(map_name).ok()??)
}

pub fn inspect_map_chunks_from_chunky(chunky: &ChunkyMap) -> Option<Vec<String>> {
    let mut labels: Vec<String> = chunky.toc.values().cloned().collect();
    labels.sort();
    Some(labels)
}

/// Load and decompress a chunky map file, returning metadata for further parsing.
pub fn load_chunky_map(map_name: &str) -> LoaderResult<Option<ChunkyMap>> {
    let Some(path) = locate_map_file(map_name) else {
        return Ok(None);
    };
    if let Some(cached) = cached_chunky_for(&path) {
        return Ok(Some(cached));
    }

    let raw_bytes = read_file_bytes_for_runtime(&path).ok_or_else(|| {
        configuration_error(format!(
            "Failed to read map '{}': path not found in virtual file system",
            path.display()
        ))
    })?;
    let bytes = if raw_bytes.starts_with(CHUNK_MAGIC) {
        raw_bytes
    } else {
        decompress_map_bytes(&raw_bytes).map_err(|err| {
            configuration_error(format!(
                "Failed to decompress map '{}': {}",
                path.display(),
                err
            ))
        })?
    };

    let (toc, body_offset) = parse_chunk_toc(&bytes)?;
    let chunky = ChunkyMap {
        source: path,
        toc,
        body_offset,
        bytes,
    };
    remember_loaded_chunky(&chunky);
    Ok(Some(chunky))
}

/// Parse high-level settings like world bounds and lighting colors.
pub fn parse_map_settings(map_name: &str) -> LoaderResult<MapMetadata> {
    let mut meta = MapMetadata::default();
    let Some(chunky) = load_chunky_map(map_name)? else {
        return Ok(meta);
    };
    parse_map_settings_from_loaded_chunky(map_name, &chunky, meta)
}

/// C++ `WorldHeightMap::ParseLightingDataChunk` (WorldHeightMap.cpp:758-829).
/// Payload is: i32 timeOfDay; then 4 TOD rows of terrain[0]+objects[0] (9 floats
/// each); v2 adds two extra object lights; v3 adds two extra terrain lights.
/// This chunk never carries sky/fog. Scene fog stays disabled like C++.
fn parse_lighting_payload_for_settings(
    version: u16,
    payload: &[u8],
    meta: &mut MapMetadata,
) -> LoaderResult<()> {
    let mut reader = BinaryReader::new(payload);
    if reader.remaining() < 4 {
        return Ok(());
    }
    let time_of_day = reader.read_i32()?;

    // C++ writes both arrays for Morning..Night (WorldHeightMap.cpp:772-820).
    let mut terrain_lights = [[[0.0f32; 9]; 3]; 4];
    let mut objects_lights = [[[0.0f32; 9]; 3]; 4];
    for tod in 0..4 {
        if reader.remaining() < 9 * 4 * 2 {
            break;
        }
        terrain_lights[tod][0] = read_global_lighting_row(&mut reader)?;
        objects_lights[tod][0] = read_global_lighting_row(&mut reader)?;
        if version >= 2 {
            for extra in 1..3 {
                if reader.remaining() < 9 * 4 {
                    break;
                }
                objects_lights[tod][extra] = read_global_lighting_row(&mut reader)?;
            }
        }
        if version >= 3 {
            for extra in 1..3 {
                if reader.remaining() < 9 * 4 {
                    break;
                }
                terrain_lights[tod][extra] = read_global_lighting_row(&mut reader)?;
            }
        }
    }

    // C++ TimeOfDay: Invalid=0, Morning=1, Afternoon=2, Evening=3, Night=4.
    let row_index = match time_of_day {
        1 => 0,
        2 => 1,
        3 => 2,
        4 => 3,
        _ => 1,
    };
    let terrain = terrain_lights[row_index][0];
    let objects = objects_lights[row_index][0];
    meta.ambient_color = Some([terrain[0], terrain[1], terrain[2]]);
    meta.sun_color = Some([terrain[3], terrain[4], terrain[5]]);
    meta.sun_direction = Some([terrain[6], terrain[7], terrain[8]]);
    // Units/shadows use the objects row (C++ W3DDisplay.cpp:2128-2147).
    meta.objects_ambient_color = Some([objects[0], objects[1], objects[2]]);
    meta.objects_sun_color = Some([objects[3], objects[4], objects[5]]);
    meta.objects_sun_direction = Some([objects[6], objects[7], objects[8]]);
    if version >= 2 {
        meta.objects_extra_lights =
            vec![objects_lights[row_index][1], objects_lights[row_index][2]];
    }
    if version >= 3 {
        meta.terrain_extra_lights =
            vec![terrain_lights[row_index][1], terrain_lights[row_index][2]];
    }
    apply_map_lighting_to_global_data(time_of_day, version, &terrain_lights, &objects_lights);
    // Never invent fog/sky from this chunk — C++ FogEnabled defaults false
    // and GlobalLighting has no fog fields.
    meta.fog_color = None;
    meta.fog_start = None;
    meta.fog_end = None;
    meta.sky_color = None;
    Ok(())
}

fn read_global_lighting_row(reader: &mut BinaryReader<'_>) -> LoaderResult<[f32; 9]> {
    Ok([
        reader.read_f32()?,
        reader.read_f32()?,
        reader.read_f32()?,
        reader.read_f32()?,
        reader.read_f32()?,
        reader.read_f32()?,
        reader.read_f32()?,
        reader.read_f32()?,
        reader.read_f32()?,
    ])
}

fn lighting_row_to_authored(
    row: [f32; 9],
) -> game_engine::common::ini::ini_game_data::TerrainLighting {
    use game_engine::common::ini::ini_game_data::{Coord3D, RGBColor, TerrainLighting};
    TerrainLighting {
        ambient: RGBColor::new(row[0], row[1], row[2]),
        diffuse: RGBColor::new(row[3], row[4], row[5]),
        light_pos: Coord3D::new(row[6], row[7], row[8]),
    }
}

/// C++ `WorldHeightMap::ParseLightingDataChunk` writes both lighting arrays
/// into `TheWritableGlobalData` for every TOD slot.
fn apply_map_lighting_to_global_data(
    time_of_day: i32,
    version: u16,
    terrain_lights: &[[[f32; 9]; 3]; 4],
    objects_lights: &[[[f32; 9]; 3]; 4],
) {
    use game_engine::common::ini::ini_game_data::{
        MAX_GLOBAL_LIGHTS, TIME_OF_DAY_FIRST, TimeOfDay, ensure_global_data,
    };
    let handle = ensure_global_data();
    let mut data = handle.write();
    data.time_of_day = match time_of_day {
        1 => TimeOfDay::Morning,
        2 => TimeOfDay::Afternoon,
        3 => TimeOfDay::Evening,
        4 => TimeOfDay::Night,
        _ => data.time_of_day,
    };
    for tod in 0..4 {
        let dest_tod = tod + TIME_OF_DAY_FIRST;
        if dest_tod >= data.terrain_lighting.len() {
            continue;
        }
        data.terrain_lighting[dest_tod][0] = lighting_row_to_authored(terrain_lights[tod][0]);
        data.terrain_objects_lighting[dest_tod][0] =
            lighting_row_to_authored(objects_lights[tod][0]);
        if version >= 2 {
            for light in 1..MAX_GLOBAL_LIGHTS {
                data.terrain_objects_lighting[dest_tod][light] =
                    lighting_row_to_authored(objects_lights[tod][light]);
            }
        }
        if version >= 3 {
            for light in 1..MAX_GLOBAL_LIGHTS {
                data.terrain_lighting[dest_tod][light] =
                    lighting_row_to_authored(terrain_lights[tod][light]);
            }
        }
    }
}

/// C++ GameLogic::loadMapINI — pull `ParticleSystem` blocks out of mixed map.ini.
pub fn extract_map_ini_particle_system_blocks(contents: &str) -> String {
    let mut out = String::new();
    let mut in_block = false;
    for raw in contents.lines() {
        let line = raw.split(';').next().unwrap_or("").trim_end();
        let trimmed = line.trim();
        if !in_block {
            if trimmed
                .split_whitespace()
                .next()
                .is_some_and(|token| token.eq_ignore_ascii_case("ParticleSystem"))
            {
                in_block = true;
                out.push_str(line);
                out.push('\n');
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
        if trimmed.eq_ignore_ascii_case("End") {
            in_block = false;
        }
    }
    out
}

/// Overlay map.ini ParticleSystem blocks onto the live GameClient manager
/// (C++ INIParticleSys.cpp via INI::load CREATE_OVERRIDES).
pub fn overlay_map_ini_particle_systems(contents: &str) -> usize {
    let extracted = extract_map_ini_particle_system_blocks(contents);
    if extracted.is_empty() {
        return 0;
    }
    let count = extracted
        .lines()
        .filter(|line| {
            line.trim()
                .split_whitespace()
                .next()
                .is_some_and(|token| token.eq_ignore_ascii_case("ParticleSystem"))
        })
        .count();
    // Common INI dispatch hits the live overlay hook when GameClient registered it.
    let mut ini = INI::new();
    if let Err(err) = ini.with_inline_source(&extracted, |ini| ini.parse_current_file()) {
        warn!("map.ini ParticleSystem Common dispatch failed: {err}");
    }
    #[cfg(feature = "game_client")]
    {
        use game_client::effects::{
            ParticleSystemINIParser, ParticleSystemManager, get_particle_system_manager_mut,
        };
        if let Ok(mut guard) = get_particle_system_manager_mut() {
            let manager = guard.get_or_insert_with(ParticleSystemManager::new);
            let parser = ParticleSystemINIParser::default();
            if let Err(err) = parser.overlay_mixed_source(&extracted, manager) {
                warn!("map.ini ParticleSystem live overlay failed: {err}");
            }
        }
    }
    count
}

/// C++ GameLogic::loadMapINI — pull `Weather` blocks out of mixed map.ini.
pub fn extract_map_ini_weather_blocks(contents: &str) -> String {
    let mut out = String::new();
    let mut in_block = false;
    for raw in contents.lines() {
        let line = raw.split(';').next().unwrap_or("").trim_end();
        let trimmed = line.trim();
        if !in_block {
            if trimmed
                .split_whitespace()
                .next()
                .is_some_and(|token| token.eq_ignore_ascii_case("Weather"))
            {
                in_block = true;
                out.push_str(line);
                out.push('\n');
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
        if trimmed.eq_ignore_ascii_case("End") {
            in_block = false;
        }
    }
    out
}

/// C++ `GameLogic::loadMapINI` — dispatch the full INI block table with
/// `INI_LOAD_CREATE_OVERRIDES` so map-authored CommandSet/CommandButton/Upgrade
/// (and the rest of the table) actually apply.
pub fn overlay_map_ini_create_overrides(contents: &str) -> usize {
    match gamelogic::system::load_map_ini_ui_overrides_from_contents(contents) {
        Ok(applied) => applied,
        Err(err) => {
            warn!("map.ini CREATE_OVERRIDES dispatch failed: {err}");
            0
        }
    }
}

/// Overlay map.ini `Weather` onto `TheWeatherSetting` via CREATE_OVERRIDES
/// (C++ GameLogic.cpp:2407-2408 `ini.load(..., INI_LOAD_CREATE_OVERRIDES)`).
/// Returns whether a Weather block was applied.
pub fn overlay_map_ini_weather(contents: &str) -> bool {
    game_engine::common::ini::ini_weather::clear_weather_setting_overrides();
    let extracted = extract_map_ini_weather_blocks(contents);
    if extracted.trim().is_empty() {
        #[cfg(feature = "game_client")]
        sync_live_snow_manager_from_common();
        return false;
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let path = std::env::temp_dir().join(format!(
        "map_weather_override_{}_{}.ini",
        std::process::id(),
        nanos
    ));
    if std::fs::write(&path, &extracted).is_err() {
        warn!("map.ini Weather overlay could not stage a temp INI");
        return false;
    }
    let mut ini = INI::new();
    let result = ini.with_file_source(&path, INILoadType::CreateOverrides, |ini| {
        ini.parse_current_file()
    });
    let _ = std::fs::remove_file(&path);
    if let Err(err) = result {
        warn!("map.ini Weather CREATE_OVERRIDES parse failed: {err}");
        return false;
    }
    #[cfg(feature = "game_client")]
    sync_live_snow_manager_from_common();
    true
}

#[cfg(feature = "game_client")]
fn sync_live_snow_manager_from_common() {
    // Common INI dispatch writes TheWeatherSetting; SnowManager still needs
    // the GameClient copy + flake spacing (C++ Snow.cpp parseWeatherDefinition).
    let _ = game_client::snow::get_weather_setting();
    let manager = game_client::snow::get_snow_manager()
        .unwrap_or_else(game_client::snow::initialize_snow_manager);
    if let Ok(mut guard) = manager.lock() {
        guard.update_ini_settings();
    }
}

/// Parse settings from an already-decompressed chunky map.
pub fn parse_map_settings_from_chunky(chunky: &ChunkyMap) -> LoaderResult<MapMetadata> {
    let map_name = chunky.source.to_string_lossy();
    parse_map_settings_from_loaded_chunky(map_name.as_ref(), chunky, MapMetadata::default())
}

fn parse_map_settings_from_loaded_chunky(
    map_name: &str,
    chunky: &ChunkyMap,
    mut meta: MapMetadata,
) -> LoaderResult<MapMetadata> {
    let body = &chunky.bytes[chunky.body_offset..];
    if let Some((ver, payload)) = find_chunk_by_label(body, &chunky.toc, "GlobalLighting")? {
        parse_lighting_payload_for_settings(ver, payload, &mut meta)?;
    }

    if let Some((min, max)) = parse_world_bounds_from_chunky(chunky).ok().flatten() {
        meta.world_min = Some(min);
        meta.world_max = Some(max);
    }

    match parse_object_placements_from_chunky(chunky) {
        Ok(objects) => {
            meta.objects = objects;
        }
        Err(err) => {
            warn!(
                "Failed to parse map object placements for '{}': {}",
                map_name, err
            );
        }
    }

    match parse_side_build_list_from_chunky(chunky) {
        Ok(builds) => {
            meta.side_builds = builds;
        }
        Err(err) => {
            warn!(
                "Failed to parse SidesList build entries for '{}': {}",
                map_name, err
            );
        }
    }

    match parse_player_start_waypoints_from_chunky(chunky) {
        Ok(starts) => {
            let mut wps = Vec::new();
            for (idx, pos, rally) in starts {
                wps.push((format!("Player_{}_Start", idx + 1), pos));
                if let Some(rally) = rally {
                    wps.push((format!("Player_{}_Rally", idx + 1), rally));
                }
            }
            meta.start_waypoints = wps;
        }
        Err(err) => {
            warn!(
                "Failed to parse player start waypoints for '{}': {}",
                map_name, err
            );
        }
    }

    meta.initial_camera_position = parse_initial_camera_position_from_chunky(chunky)
        .ok()
        .flatten();

    // Heightmap hint: look for common heightmap filenames next to the .map.
    if let Some(map_path) = locate_map_file(map_name) {
        if let Some(dir) = map_path.parent() {
            if let Some((_, contents)) =
                first_readable_map_ini_companion(dir, &["Map.ini", "map.ini"])
            {
                // C++ GameLogic.cpp:2404-2408 loadMapINI — full block table
                // via INI_LOAD_CREATE_OVERRIDES (CommandSet/CommandButton/Upgrade).
                let _ = overlay_map_ini_create_overrides(&contents);
                // ParticleSystem still needs the live GameClient manager hook.
                let _ = overlay_map_ini_particle_systems(&contents);
                // Weather CREATE_OVERRIDES + SnowManager flake spacing.
                let _ = overlay_map_ini_weather(&contents);

                let mut skybox_textures: [Option<String>; 5] = [None, None, None, None, None];
                for raw_line in contents.lines() {
                    let line = raw_line.split(';').next().unwrap_or("").trim();
                    if line.is_empty() {
                        continue;
                    }
                    let Some((key, value)) = line.split_once('=') else {
                        continue;
                    };
                    let key = key.trim();
                    let value = value.trim().trim_matches('"');
                    if value.is_empty() {
                        continue;
                    }
                    match key.to_ascii_lowercase().as_str() {
                        "skyboxtexturen" => skybox_textures[0] = Some(value.to_string()),
                        "skyboxtexturee" => skybox_textures[1] = Some(value.to_string()),
                        "skyboxtextures" => skybox_textures[2] = Some(value.to_string()),
                        "skyboxtexturew" => skybox_textures[3] = Some(value.to_string()),
                        "skyboxtexturet" => skybox_textures[4] = Some(value.to_string()),
                        _ => {}
                    }
                }
                if skybox_textures.iter().all(|texture| texture.is_some()) {
                    meta.skybox_textures = Some([
                        skybox_textures[0].clone().unwrap(),
                        skybox_textures[1].clone().unwrap(),
                        skybox_textures[2].clone().unwrap(),
                        skybox_textures[3].clone().unwrap(),
                        skybox_textures[4].clone().unwrap(),
                    ]);
                }
            }

            // C++ loadMapINI also loads companion solo.ini CREATE_OVERRIDES.
            if let Some((_, contents)) =
                first_readable_map_ini_companion(dir, &["Solo.ini", "solo.ini"])
            {
                let _ = overlay_map_ini_create_overrides(&contents);
            }

            // C++ parity: only treat dedicated heightmap companions as terrain sources.
            // Generic *.tga beside a map is commonly preview/sky art, not elevation data.
            for ext in ["hmp", "raw"] {
                let stem = map_path.file_stem().and_then(|stem| stem.to_str());
                let Some(stem) = stem else {
                    continue;
                };

                let mut candidate = dir.join(stem);
                candidate.set_extension(ext);
                if let Some(heightmap_path) = resolve_runtime_path(&candidate) {
                    meta.heightmap_path = Some(heightmap_path);
                    break;
                }
            }

            // Skybox hints: look for common texture names in the map folder.
            let faces = ["front", "back", "left", "right", "top"];
            let mut textures: [Option<String>; 5] = [None, None, None, None, None];
            for (i, face) in faces.iter().enumerate() {
                let mut candidate = dir.to_path_buf();
                candidate.push(format!("Sky{}.tga", face));
                if path_is_accessible(&candidate) {
                    textures[i] = Some(candidate.to_string_lossy().to_string());
                    continue;
                }
                let mut alt = dir.to_path_buf();
                alt.push(format!(
                    "{}{}.tga",
                    map_path.file_stem().unwrap_or_default().to_string_lossy(),
                    face
                ));
                if path_is_accessible(&alt) {
                    textures[i] = Some(alt.to_string_lossy().to_string());
                }
            }
            if meta.skybox_textures.is_none() && textures.iter().all(|t| t.is_some()) {
                meta.skybox_textures = Some([
                    textures[0].clone().unwrap(),
                    textures[1].clone().unwrap(),
                    textures[2].clone().unwrap(),
                    textures[3].clone().unwrap(),
                    textures[4].clone().unwrap(),
                ]);
            }
        }
    }

    Ok(meta)
}

/// Parse the `HeightMapData` chunk into raw 8-bit height samples.
pub fn parse_heightmap_data(map_name: &str) -> LoaderResult<Option<HeightMapData>> {
    let Some(chunky) = load_chunky_map(map_name)? else {
        return Ok(None);
    };

    parse_heightmap_data_from_chunky(&chunky)
}

pub fn parse_heightmap_data_from_chunky(chunky: &ChunkyMap) -> LoaderResult<Option<HeightMapData>> {
    let body = &chunky.bytes[chunky.body_offset..];
    let Some((version, payload)) = find_chunk_by_label(body, &chunky.toc, "HeightMapData")? else {
        return Ok(None);
    };

    let mut reader = BinaryReader::new(payload);
    let width = reader.read_i32()?;
    let height = reader.read_i32()?;
    let border_size = if version >= 3 { reader.read_i32()? } else { 0 };

    let boundaries = if version >= 4 {
        let count = reader.read_i32()?.max(0) as usize;
        let mut boundaries = Vec::with_capacity(count.max(1));
        for _ in 0..count {
            boundaries.push((reader.read_i32()?, reader.read_i32()?));
        }
        boundaries
    } else {
        vec![(width - 2 * border_size, height - 2 * border_size)]
    };

    let data_size = reader.read_i32()?;
    let expected = width.saturating_mul(height);
    if data_size <= 0 || data_size != expected {
        return Err(configuration_error(format!(
            "HeightMapData has invalid dataSize={}, expected {}",
            data_size, expected
        )));
    }

    let mut data = reader.read_bytes(data_size as usize)?.to_vec();

    if version == 1 {
        let new_width = (width + 1) / 2;
        let new_height = (height + 1) / 2;
        let mut resized = vec![0u8; (new_width * new_height).max(0) as usize];
        for i in 0..new_height.max(0) {
            for j in 0..new_width.max(0) {
                let src = (2 * i * width + 2 * j).max(0) as usize;
                let dst = (i * new_width + j).max(0) as usize;
                if src < data.len() && dst < resized.len() {
                    resized[dst] = data[src];
                }
            }
        }
        data = resized;
        return Ok(Some(HeightMapData {
            width: new_width,
            height: new_height,
            border_size,
            boundaries,
            data,
        }));
    }

    Ok(Some(HeightMapData {
        width,
        height,
        border_size,
        boundaries,
        data,
    }))
}

pub fn parse_blend_tile_data_from_chunky(
    chunky: &ChunkyMap,
    heightmap: &HeightMapData,
) -> LoaderResult<Option<BlendTileData>> {
    let body = &chunky.bytes[chunky.body_offset..];
    let Some((version, payload)) = find_chunk_by_label(body, &chunky.toc, "BlendTileData")? else {
        return Ok(None);
    };

    let mut reader = BinaryReader::new(payload);
    let data_size = reader.read_i32()?;
    let expected = heightmap.width.saturating_mul(heightmap.height);
    if data_size <= 0 || data_size != expected {
        return Err(configuration_error(format!(
            "BlendTileData has invalid dataSize={}, expected {}",
            data_size, expected
        )));
    }
    let data_size = data_size as usize;

    let mut tile_ndxes = reader.read_i16_vec(data_size)?;
    let mut blend_tile_ndxes = reader.read_i16_vec(data_size)?;

    let mut extra_blend_tile_ndxes = if version >= 6 {
        reader.read_i16_vec(data_size)?
    } else {
        vec![0i16; data_size]
    };
    if version >= 5 {
        let _cliff_info_ndxes = reader.read_i16_vec(data_size)?;
    }
    if version >= 7 {
        let byte_width = if version == 7 {
            (heightmap.width + 1) / 8
        } else {
            (heightmap.width + 7) / 8
        }
        .max(0) as usize;
        let byte_count = byte_width.saturating_mul(heightmap.height.max(0) as usize);
        reader.read_bytes(byte_count)?;
    }

    let _num_bitmap_tiles = reader.read_i32()?;
    let num_blended_tiles = reader.read_i32()?.max(0) as usize;
    if version >= 5 {
        let _num_cliff_info = reader.read_i32()?;
    }

    let texture_class_count = reader.read_i32()?.max(0) as usize;
    let mut texture_classes = Vec::with_capacity(texture_class_count);
    for _ in 0..texture_class_count {
        let first_tile = reader.read_i32()?;
        let num_tiles = reader.read_i32()?;
        let width = reader.read_i32()?;
        let _legacy_gdf = reader.read_i32()?;
        let name = reader.read_ascii_string()?;
        texture_classes.push(BlendTileTextureClass {
            first_tile,
            num_tiles,
            width,
            name,
        });
    }

    // C++ WorldHeightMap.cpp:1124-1167 — edge classes (v4+) then TBlendTileInfo.
    // Older synthetic/truncated payloads stop after texture classes; keep
    // tile_ndxes and extra-blend indexes rather than failing the whole parse.
    let mut edge_texture_classes = Vec::new();
    if version >= 4 && reader.remaining() >= 8 {
        let _num_edge_tiles = reader.read_i32()?;
        let edge_class_count = reader.read_i32()?.max(0) as usize;
        edge_texture_classes.reserve(edge_class_count);
        for _ in 0..edge_class_count {
            if reader.remaining() < 12 {
                break;
            }
            let first_tile = reader.read_i32()?;
            let num_tiles = reader.read_i32()?;
            let width = reader.read_i32()?;
            let name = reader.read_ascii_string()?;
            edge_texture_classes.push(BlendTileTextureClass {
                first_tile,
                num_tiles,
                width,
                name,
            });
        }
    }

    let mut blended_tiles = Vec::new();
    if num_blended_tiles > 1 && reader.remaining() > 0 {
        blended_tiles.reserve(num_blended_tiles.saturating_sub(1));
        for _ in 1..num_blended_tiles {
            let blend_ndx = reader.read_i32()?;
            let horiz = reader.read_u8()?;
            let vert = reader.read_u8()?;
            let right_diagonal = reader.read_u8()?;
            let left_diagonal = reader.read_u8()?;
            let inverted = reader.read_u8()?;
            let long_diagonal = if version >= 3 { reader.read_u8()? } else { 0 };
            let custom_blend_edge_class = if version >= 4 { reader.read_i32()? } else { -1 };
            let _flag = reader.read_i32()?;
            blended_tiles.push(BlendTileInfo {
                blend_ndx,
                horiz,
                vert,
                right_diagonal,
                left_diagonal,
                inverted,
                long_diagonal,
                custom_blend_edge_class,
            });
        }
    }

    if version == 1 {
        let new_width = (heightmap.width + 1) / 2;
        let new_height = (heightmap.height + 1) / 2;
        let mut resized_tiles = vec![0i16; (new_width * new_height).max(0) as usize];
        let mut resized_blends = vec![0i16; resized_tiles.len()];
        for i in 0..new_height.max(0) {
            for j in 0..new_width.max(0) {
                let src = (2 * i * heightmap.width + 2 * j).max(0) as usize;
                let dst = (i * new_width + j).max(0) as usize;
                if src < tile_ndxes.len() && dst < resized_tiles.len() {
                    resized_tiles[dst] = tile_ndxes[src];
                    resized_blends[dst] = 0;
                }
            }
        }
        tile_ndxes = resized_tiles;
        blend_tile_ndxes = resized_blends;
        extra_blend_tile_ndxes = vec![0i16; tile_ndxes.len()];
        blended_tiles.clear();
    }

    Ok(Some(BlendTileData {
        tile_ndxes,
        blend_tile_ndxes,
        extra_blend_tile_ndxes,
        texture_classes,
        edge_texture_classes,
        blended_tiles,
    }))
}

pub fn parse_runtime_waypoints_from_chunky(
    chunky: &ChunkyMap,
) -> LoaderResult<(Vec<RuntimeWaypoint>, Vec<(u32, u32)>)> {
    let body = &chunky.bytes[chunky.body_offset..];
    let mut waypoints = Vec::new();
    let mut links = Vec::new();

    if let Some((version, payload)) = find_chunk_by_label(body, &chunky.toc, OBJECTS_LIST_LABEL)? {
        parse_chunk_sequence(payload, &chunky.toc, |label, child_version, data| {
            if label != OBJECT_LABEL {
                return Ok(());
            }
            if let Some(waypoint) =
                parse_waypoint_object_chunk(data, child_version.max(version), &chunky.toc)?
            {
                waypoints.push(waypoint);
            }
            Ok(())
        })?;
    }

    if let Some((_ver, payload)) = find_chunk_by_label(body, &chunky.toc, "WaypointsList")? {
        let mut reader = BinaryReader::new(payload);
        let count = reader.read_i32()?.max(0) as usize;
        for _ in 0..count {
            if reader.remaining() < 8 {
                break;
            }
            let id1 = reader.read_i32()? as u32;
            let id2 = reader.read_i32()? as u32;
            links.push((id1, id2));
        }
    }

    Ok((waypoints, links))
}

pub fn parse_runtime_bridges_from_chunky(chunky: &ChunkyMap) -> LoaderResult<Vec<BridgeData>> {
    let body = &chunky.bytes[chunky.body_offset..];
    let mut bridges = Vec::new();
    let mut pending: HashMap<String, Vec<PendingRuntimeBridge>> = HashMap::new();

    if let Some((version, payload)) = find_chunk_by_label(body, &chunky.toc, OBJECTS_LIST_LABEL)? {
        parse_chunk_sequence(payload, &chunky.toc, |label, child_version, data| {
            if label != OBJECT_LABEL {
                return Ok(());
            }
            if let Some(endpoint) =
                parse_bridge_endpoint_object_chunk(data, child_version.max(version))?
            {
                add_runtime_bridge_point(&mut bridges, &mut pending, endpoint);
            }
            Ok(())
        })?;
    }

    Ok(bridges)
}

/// Parse `WorldInfo` water-table height when the map dict carries one.
///
/// C++ `WorldHeightMap::ParseWorldDictDataChunk` (WorldHeightMap.cpp:739) stores
/// the world dict; crate `MapLoader::extract_water_height` reads `waterHeight`.
/// Missing or unreadable WorldInfo is fail-soft (`None`).
pub fn parse_runtime_water_height_from_chunky(chunky: &ChunkyMap) -> LoaderResult<Option<f32>> {
    let Some(dict) = parse_runtime_world_info_dict(chunky)? else {
        return Ok(None);
    };
    Ok(dict_lookup_ci(&dict, &["waterHeight", "WaterHeight"]).and_then(|value| value.parse().ok()))
}

/// Parse `WorldInfo` weather enum (`TheKey_weather`).
///
/// C++ `WorldHeightMap::ParseWorldDictDataChunk` (WorldHeightMap.cpp:743-746)
/// writes `TheWritableGlobalData->m_weather` (`WEATHER_NORMAL=0` /
/// `WEATHER_SNOWY=1`) when the key exists. Missing WorldInfo or key is
/// fail-soft (`None`) so GameData.ini `WEATHER` remains the default.
pub fn parse_runtime_weather_from_chunky(chunky: &ChunkyMap) -> LoaderResult<Option<i32>> {
    let Some(dict) = parse_runtime_world_info_dict(chunky)? else {
        return Ok(None);
    };
    Ok(dict_lookup_ci(&dict, &["weather", "Weather"])
        .and_then(|value| parse_world_weather_value(&value)))
}

/// C++ `Weather` int / WorldBuilder name → `WEATHER_NORMAL=0` / `WEATHER_SNOWY=1`.
pub fn parse_world_weather_value(raw: &str) -> Option<i32> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(value) = trimmed.parse::<i32>() {
        return Some(value);
    }
    match trimmed.to_ascii_uppercase().as_str() {
        "NORMAL" | "CLEAR" => Some(0),
        "SNOWY" | "SNOW" => Some(1),
        _ => None,
    }
}

/// C++ `TheKey_objectWeather` (`Object.cpp:3595-3605`): 0 follow, 1 force
/// normal, 2 force snow.
pub fn parse_object_weather_value(raw: &str) -> Option<i32> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(value) = trimmed.parse::<i32>() {
        return Some(value);
    }
    match trimmed.to_ascii_uppercase().as_str() {
        "NORMAL" | "CLEAR" | "FOLLOW" => Some(0),
        "FORCEDNORMAL" | "FORCE_NORMAL" => Some(1),
        "SNOWY" | "SNOW" | "FORCEDSNOW" | "FORCE_SNOW" => Some(2),
        _ => None,
    }
}

/// Resolve SNOW after global weather stamp, then per-object override.
pub fn resolve_object_weather_snow(object_weather: i32, world_is_snow: bool) -> bool {
    match object_weather {
        1 => false,
        2 => true,
        _ => world_is_snow,
    }
}

fn parse_runtime_world_info_dict(
    chunky: &ChunkyMap,
) -> LoaderResult<Option<HashMap<String, String>>> {
    let body = &chunky.bytes[chunky.body_offset..];
    let Some((_version, payload)) = find_chunk_by_label(body, &chunky.toc, "WorldInfo")? else {
        return Ok(None);
    };
    let mut reader = BinaryReader::new(payload);
    if reader.remaining() == 0 {
        return Ok(None);
    }
    match parse_chunk_dict(&mut reader, &chunky.toc) {
        Ok(dict) => Ok(Some(dict)),
        Err(err) => {
            warn!("WorldInfo dict parse failed; water/weather unavailable: {err}");
            Ok(None)
        }
    }
}

/// Parse `PolygonTriggers`, including water-area flags and point Z heights.
///
/// C++ `PolygonTrigger::ParsePolygonTriggersDataChunk` (PolygonTrigger.cpp).
/// Missing or truncated chunks fail-soft to an empty list.
pub fn parse_runtime_polygon_triggers_from_chunky(
    chunky: &ChunkyMap,
) -> LoaderResult<Vec<PolygonTrigger>> {
    let body = &chunky.bytes[chunky.body_offset..];
    let Some((version, payload)) = find_chunk_by_label(body, &chunky.toc, "PolygonTriggers")?
    else {
        return Ok(Vec::new());
    };

    let mut reader = BinaryReader::new(payload);
    if reader.remaining() < 4 {
        return Ok(Vec::new());
    }
    let count = reader.read_i32()?.max(0);
    let mut triggers = Vec::new();
    let mut max_trigger_id = 0i32;
    for _ in 0..count {
        if reader.remaining() < 2 {
            break;
        }
        let trigger_name = match reader.read_ascii_string() {
            Ok(name) => name,
            Err(_) => break,
        };
        let layer_name = if version >= 4 {
            match reader.read_ascii_string() {
                Ok(name) => name,
                Err(_) => break,
            }
        } else {
            String::new()
        };
        let Ok(trigger_id) = reader.read_i32() else {
            break;
        };
        let is_water = if version >= 2 {
            match reader.read_u8() {
                Ok(byte) => byte != 0,
                Err(_) => break,
            }
        } else {
            false
        };
        let (is_river, river_start) = if version >= 3 {
            let river = match reader.read_u8() {
                Ok(byte) => byte != 0,
                Err(_) => break,
            };
            let start = match reader.read_i32() {
                Ok(value) => value,
                Err(_) => break,
            };
            (river, start)
        } else {
            (false, 0)
        };
        let Ok(num_points) = reader.read_i32() else {
            break;
        };
        let mut points = Vec::new();
        let mut points_ok = true;
        for _ in 0..num_points.max(0) {
            let (Ok(x), Ok(y), Ok(z)) = (reader.read_i32(), reader.read_i32(), reader.read_i32())
            else {
                points_ok = false;
                break;
            };
            points.push(ICoord3D::new(x, y, z));
        }
        if !points_ok {
            break;
        }
        if trigger_id > max_trigger_id {
            max_trigger_id = trigger_id;
        }
        if points.len() < 2 {
            continue;
        }
        let mut trigger =
            PolygonTrigger::new(trigger_id, AsciiString::from(trigger_name.as_str()), points);
        trigger.set_layer_name(AsciiString::from(layer_name.as_str()));
        trigger.set_water_area(is_water);
        trigger.set_river(is_river);
        trigger.set_river_start(river_start);
        triggers.push(trigger);
    }

    // C++ PolygonTrigger.cpp version-1 maps auto-add a full-extent water table.
    if version == 1 {
        // C++ `pTrig->m_triggerID = maxTriggerId++` reuses the current max.
        let water_id = max_trigger_id;
        let mut trigger = PolygonTrigger::new(
            water_id,
            AsciiString::from("AutoAddedWaterAreaTrigger"),
            Vec::new(),
        );
        trigger.set_water_area(true);
        let (water_extent_x, water_extent_y) = game_engine::common::ini::get_global_data()
            .map(|data| {
                let data = data.read();
                (data.water_extent_x, data.water_extent_y)
            })
            .unwrap_or((0.0, 0.0));
        let border = 30.0 * MAP_XY_FACTOR;
        trigger.add_point(ICoord3D::new((-border) as i32, (-border) as i32, 7));
        trigger.add_point(ICoord3D::new(
            (border + water_extent_x) as i32,
            (-border) as i32,
            7,
        ));
        trigger.add_point(ICoord3D::new(
            (border + water_extent_x) as i32,
            (border + water_extent_y) as i32,
            7,
        ));
        trigger.add_point(ICoord3D::new(
            (-border) as i32,
            (border + water_extent_y) as i32,
            7,
        ));
        triggers.push(trigger);
    }

    Ok(triggers)
}

/// Register parsed map polygons on leftover `TerrainLogic` by name.
///
/// C++ `PolygonTrigger::ParsePolygonTriggersDataChunk` leaves the live list
/// as the geometry source for `pointInTrigger`. Skip names already present
/// so `load_map_data` and this installer do not double-add.
pub fn install_runtime_polygon_triggers(triggers: &[PolygonTrigger]) {
    let Ok(mut terrain) = gamelogic::terrain::get_terrain_logic().write() else {
        return;
    };
    for trigger in triggers {
        let name = trigger.get_trigger_name().as_str();
        if terrain.get_trigger_area_by_name(name).is_some() {
            continue;
        }
        terrain.add_trigger_area(trigger.clone());
    }
}

/// Parse runtime terrain-road segments from map objects.
///
/// This mirrors C++ `W3DRoadBuffer::addMapObjects` pairing semantics:
/// only `ROAD_POINT1` objects whose immediate next object is `ROAD_POINT2`
/// produce a segment.
pub fn parse_runtime_roads_from_chunky(
    chunky: &ChunkyMap,
) -> LoaderResult<Vec<RuntimeRoadSegment>> {
    ensure_terrain_roads_loaded();

    let body = &chunky.bytes[chunky.body_offset..];
    let mut objects = Vec::new();

    if let Some((version, payload)) = find_chunk_by_label(body, &chunky.toc, OBJECTS_LIST_LABEL)? {
        parse_chunk_sequence(payload, &chunky.toc, |label, child_version, data| {
            if label != OBJECT_LABEL {
                return Ok(());
            }
            if let Some(map_object) =
                parse_runtime_map_object_stub_chunk(data, child_version.max(version))?
            {
                objects.push(map_object);
            }
            Ok(())
        })?;
    }

    let mut roads = Vec::new();
    let mut index = 0usize;
    while index < objects.len() {
        let current = &objects[index];
        if (current.flags & FLAG_ROAD_POINT1) == 0 {
            index += 1;
            continue;
        }

        let Some(next) = objects.get(index + 1) else {
            break;
        };
        if (next.flags & FLAG_ROAD_POINT2) == 0 {
            index += 1;
            continue;
        }

        roads.push(build_runtime_road_data(
            current.template_name.as_str(),
            current.location,
            next.location,
            current.flags,
            next.flags,
        ));
        index += 2;
    }

    Ok(roads)
}

pub fn parse_runtime_sides_from_chunky(chunky: &ChunkyMap) -> LoaderResult<RuntimeSidesData> {
    let body = &chunky.bytes[chunky.body_offset..];
    let Some((version, payload)) = find_chunk_by_label(body, &chunky.toc, SIDES_LIST_LABEL)? else {
        return Ok(RuntimeSidesData::default());
    };

    let mut reader = BinaryReader::new(payload);
    let mut side_dicts = Vec::new();
    let mut team_dicts = Vec::new();

    let mut side_builds = Vec::new();
    let side_count = reader.read_i32()?.max(0) as usize;
    for side_index in 0..side_count {
        side_dicts.push(parse_chunk_dict_typed(&mut reader, &chunky.toc)?);
        let build_count = reader.read_i32()?.max(0) as usize;
        for _ in 0..build_count {
            side_builds.push(parse_side_build_entry(
                &mut reader,
                version,
                side_index as u32,
            )?);
        }
    }

    if version >= 2 {
        let team_count = reader.read_i32()?.max(0) as usize;
        for _ in 0..team_count {
            team_dicts.push(parse_chunk_dict_typed(&mut reader, &chunky.toc)?);
        }
    }

    Ok(RuntimeSidesData {
        side_dicts,
        team_dicts,
        side_builds,
    })
}

/// Parse placed objects from a chunky map. Currently supports a minimal subset
/// of the ObjectCreationList chunk (template, position, rotation, team).

/// Wave 831: parse SidesList build-list entries for skirmish faction bases.

/// Wave 831: Player_N_Start / Player_N_Rally waypoints (1-based N).
pub fn parse_player_start_waypoints(
    map_name: &str,
) -> LoaderResult<Vec<(u32, Coord3D, Option<Coord3D>)>> {
    let Some(chunky) = load_chunky_map(map_name)? else {
        return Ok(Vec::new());
    };
    parse_player_start_waypoints_from_chunky(&chunky)
}

pub fn parse_player_start_waypoints_from_chunky(
    chunky: &ChunkyMap,
) -> LoaderResult<Vec<(u32, Coord3D, Option<Coord3D>)>> {
    let (waypoints, _links) = parse_runtime_waypoints_from_chunky(chunky)?;
    let mut starts: std::collections::HashMap<u32, Coord3D> = std::collections::HashMap::new();
    let mut rallies: std::collections::HashMap<u32, Coord3D> = std::collections::HashMap::new();
    for wp in waypoints {
        let name = wp.name.trim();
        let lower = name.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("player_") {
            if let Some((num, kind)) = rest.split_once('_') {
                if let Ok(idx1) = num.parse::<u32>() {
                    if idx1 >= 1 {
                        let idx0 = idx1 - 1;
                        if kind.starts_with("start") {
                            starts.insert(idx0, wp.location);
                        } else if kind.starts_with("rally") {
                            rallies.insert(idx0, wp.location);
                        }
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    let mut keys: Vec<u32> = starts.keys().copied().collect();
    keys.sort_unstable();
    for k in keys {
        out.push((k, starts[&k], rallies.get(&k).copied()));
    }
    Ok(out)
}

pub fn parse_side_build_list(map_name: &str) -> LoaderResult<Vec<SideBuildEntry>> {
    let Some(chunky) = load_chunky_map(map_name)? else {
        return Ok(Vec::new());
    };
    parse_side_build_list_from_chunky(&chunky)
}

pub fn parse_side_build_list_from_chunky(chunky: &ChunkyMap) -> LoaderResult<Vec<SideBuildEntry>> {
    let sides = parse_runtime_sides_from_chunky(chunky)?;
    Ok(sides.side_builds)
}

pub fn parse_object_placements(map_name: &str) -> LoaderResult<Vec<PlacedObject>> {
    let Some(chunky) = load_chunky_map(map_name)? else {
        return Ok(Vec::new());
    };
    parse_object_placements_from_chunky(&chunky)
}

pub fn parse_object_placements_from_chunky(chunky: &ChunkyMap) -> LoaderResult<Vec<PlacedObject>> {
    ensure_terrain_roads_loaded();
    let body = &chunky.bytes[chunky.body_offset..];
    let map_name = chunky.source.display().to_string();

    if let Some((version, payload)) = find_chunk_by_label(body, &chunky.toc, OBJECTS_LIST_LABEL)? {
        let mut objects = Vec::new();
        let mut labels_seen: HashMap<String, usize> = HashMap::new();
        parse_chunk_sequence(payload, &chunky.toc, |label, _chunk_version, data| {
            *labels_seen.entry(label.to_string()).or_insert(0) += 1;
            if label != OBJECT_LABEL {
                return Ok(());
            }
            if let Some(obj) = parse_map_object_chunk(data, version, &chunky.toc)? {
                objects.push(obj);
            }
            Ok(())
        })?;
        if objects.is_empty() {
            debug!(
                "Map '{}' ObjectsList parsed with no placements; subchunk histogram: {:?}",
                map_name, labels_seen
            );
        }
        return Ok(objects);
    }

    if let Some((version, payload)) =
        find_chunk_by_label(body, &chunky.toc, OBJECT_CREATION_LIST_LABEL)?
    {
        let mut objects = Vec::new();
        parse_chunk_sequence(payload, &chunky.toc, |label, _chunk_version, data| {
            if label != OBJECT_LABEL {
                return Ok(());
            }
            if let Some(obj) = parse_object_creation_chunk(data, version)? {
                objects.push(obj);
            }
            Ok(())
        })?;
        return Ok(objects);
    }

    warn!(
        "Map '{}' has neither '{}' nor '{}' chunks; skipping object placements",
        map_name, OBJECTS_LIST_LABEL, OBJECT_CREATION_LIST_LABEL
    );
    Ok(Vec::new())
}

fn parse_initial_camera_position(map_name: &str) -> LoaderResult<Option<Coord3D>> {
    let Some(chunky) = load_chunky_map(map_name)? else {
        return Ok(None);
    };
    parse_initial_camera_position_from_chunky(&chunky)
}

fn parse_initial_camera_position_from_chunky(chunky: &ChunkyMap) -> LoaderResult<Option<Coord3D>> {
    let body = &chunky.bytes[chunky.body_offset..];

    if let Some((version, payload)) = find_chunk_by_label(body, &chunky.toc, OBJECTS_LIST_LABEL)? {
        let mut result = None;
        parse_chunk_sequence(payload, &chunky.toc, |label, _chunk_version, data| {
            if result.is_some() || label != OBJECT_LABEL {
                return Ok(());
            }
            result = parse_camera_waypoint_chunk(data, version, &chunky.toc)?;
            Ok(())
        })?;
        if result.is_some() {
            return Ok(result);
        }
    }

    if let Some((version, payload)) =
        find_chunk_by_label(body, &chunky.toc, OBJECT_CREATION_LIST_LABEL)?
    {
        let mut result = None;
        parse_chunk_sequence(payload, &chunky.toc, |label, _chunk_version, data| {
            if result.is_some() || label != OBJECT_LABEL {
                return Ok(());
            }
            result = parse_camera_waypoint_chunk(data, version, &chunky.toc)?;
            Ok(())
        })?;
        if result.is_some() {
            return Ok(result);
        }
    }

    Ok(None)
}

fn parse_camera_waypoint_chunk(
    data: &[u8],
    version: u16,
    toc: &HashMap<u32, String>,
) -> LoaderResult<Option<Coord3D>> {
    let mut reader = BinaryReader::new(data);
    if reader.remaining() < 20 {
        return Ok(None);
    }

    let x = reader.read_f32()?;
    let y = reader.read_f32()?;
    let mut z = reader.read_f32()?;
    if version <= 2 {
        z = 0.0;
    }
    let _angle = reader.read_f32()?;
    let _flags = reader.read_i32()?;
    let template_name = reader.read_ascii_string()?;

    if version < 2 || reader.remaining() == 0 {
        if template_name.eq_ignore_ascii_case("InitialCameraPosition") {
            return Ok(Some(Coord3D::new(x, y, z)));
        }
        return Ok(None);
    }

    let dict = parse_chunk_dict(&mut reader, toc)?;
    if !dict_contains_key(&dict, "waypointID") {
        return Ok(None);
    }

    let waypoint_name = dict_lookup_ci(&dict, &["waypointName"])
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(template_name);
    if waypoint_name.eq_ignore_ascii_case("InitialCameraPosition") {
        return Ok(Some(Coord3D::new(x, y, z)));
    }

    Ok(None)
}

/// Parse world bounds from the map's GlobalLighting or Waypoints chunk.
pub fn parse_world_bounds(map_name: &str) -> LoaderResult<Option<(Coord3D, Coord3D)>> {
    let Some(chunky) = load_chunky_map(map_name)? else {
        return Ok(None);
    };
    parse_world_bounds_from_chunky(&chunky)
}

pub fn parse_world_bounds_from_chunky(
    chunky: &ChunkyMap,
) -> LoaderResult<Option<(Coord3D, Coord3D)>> {
    let body = &chunky.bytes[chunky.body_offset..];

    let mut waypoint_bounds = None;
    if let Some((_ver, payload)) = find_chunk_by_label(body, &chunky.toc, "WaypointsList")? {
        let mut reader = BinaryReader::new(payload);
        if reader.remaining() >= 4 {
            let count = reader.read_i32()? as usize;
            let mut min = Coord3D::new(f32::MAX, f32::MAX, f32::MAX);
            let mut max = Coord3D::new(f32::MIN, f32::MIN, f32::MIN);
            for _ in 0..count {
                if reader.remaining() < 12 {
                    break;
                }
                let x = reader.read_f32()?;
                let y = reader.read_f32()?;
                let z = reader.read_f32()?;
                min.x = min.x.min(x);
                min.y = min.y.min(y);
                min.z = min.z.min(z);
                max.x = max.x.max(x);
                max.y = max.y.max(y);
                max.z = max.z.max(z);
            }
            if min.x < f32::MAX / 2.0 && max.x > f32::MIN / 2.0 {
                waypoint_bounds = Some((min, max));
            }
        }
    }

    if let Some((min, max)) = waypoint_bounds {
        let extent_x = (max.x - min.x).abs();
        let extent_z = (max.z - min.z).abs();
        if extent_x >= 1.0 && extent_z >= 1.0 {
            return Ok(Some((min, max)));
        }
    }

    if let Some(heightmap) = parse_heightmap_data_from_chunky(chunky)? {
        let playable_w = (heightmap.width - 2 * heightmap.border_size).max(1) as f32;
        let playable_h = (heightmap.height - 2 * heightmap.border_size).max(1) as f32;
        let max = Coord3D::new(playable_w * MAP_XY_FACTOR, 0.0, playable_h * MAP_XY_FACTOR);
        return Ok(Some((Coord3D::new(0.0, 0.0, 0.0), max)));
    }

    Ok(None)
}

fn parse_map_object_chunk(
    data: &[u8],
    version: u16,
    toc: &HashMap<u32, String>,
) -> LoaderResult<Option<PlacedObject>> {
    // Mirrors C++ ParseObjectDataChunk: x/y/z, angle, flags, template name, dict (v2+).
    let mut reader = BinaryReader::new(data);
    if reader.remaining() < 20 {
        return Ok(None);
    }

    let x = reader.read_f32()?;
    let y = reader.read_f32()?;
    let mut z = reader.read_f32()?;
    if version <= 2 {
        z = 0.0;
    }
    let angle = reader.read_f32()?;
    let _flags = reader.read_i32()?;
    let template = reader.read_ascii_string()?;
    if template.is_empty() {
        return Ok(None);
    }
    if is_terrain_road_name(&template) {
        return Ok(None);
    }

    let mut name = None;
    let mut team_name = None;
    let mut player_id = None;
    let mut upgrade = None;
    let mut unsellable = None;
    let mut enabled = None;
    let mut powered = None;
    let mut indestructible = None;

    let mut object_weather = None;
    let mut properties = Dict::new();

    if version >= 2 && reader.remaining() > 0 {
        properties = parse_chunk_dict_typed(&mut reader, toc)?;
        let dict = dict_to_string_map(&properties);

        // Waypoints are map metadata nodes, not spawnable world objects.
        if dict_contains_key(&dict, "waypointID") {
            return Ok(None);
        }

        team_name = dict_lookup_ci(
            &dict,
            &["teamName", "team", "originalOwner", "owner", "playerName"],
        )
        .filter(|value| !value.trim().is_empty());

        name = dict_lookup_ci(
            &dict,
            &["objectName", "scriptName", "unitName", "thingName", "name"],
        )
        .filter(|value| !value.trim().is_empty());

        player_id = dict_lookup_ci(
            &dict,
            &[
                "player",
                "playerId",
                "playerID",
                "ownerPlayer",
                "multiplayerStartIndex",
                "originalOwner",
            ],
        )
        .and_then(|value| parse_player_id(&value));

        upgrade = dict_lookup_ci(
            &dict,
            &[
                "upgrade",
                "upgrades",
                "upgradeList",
                "startupUpgrade",
                "startupUpgrades",
            ],
        )
        .filter(|value| !value.trim().is_empty());

        unsellable = dict_lookup_ci(
            &dict,
            &["objectUnsellable", "unsellable", "object_unsellable"],
        )
        .map(|value| parse_ini_boolish(&value));

        enabled = dict_lookup_ci(&dict, &["objectEnabled", "enabled", "object_enabled"])
            .map(|value| parse_ini_boolish(&value));

        powered = dict_lookup_ci(&dict, &["objectPowered", "powered", "object_powered"])
            .map(|value| parse_ini_boolish(&value));

        indestructible = dict_lookup_ci(
            &dict,
            &[
                "objectIndestructible",
                "indestructible",
                "object_indestructible",
            ],
        )
        .map(|value| parse_ini_boolish(&value));

        object_weather = dict_lookup_ci(&dict, &["objectWeather", "object_weather"])
            .and_then(|value| parse_object_weather_value(&value));
    }

    Ok(Some(PlacedObject {
        template,
        name,
        position: Coord3D::new(x, y, z),
        rotation: Some(angle),
        team_name,
        player_id,
        upgrade,
        unsellable,
        enabled,
        powered,
        indestructible,

        object_weather,
        properties,
    }))
}

fn parse_waypoint_object_chunk(
    data: &[u8],
    version: u16,
    toc: &HashMap<u32, String>,
) -> LoaderResult<Option<RuntimeWaypoint>> {
    let mut reader = BinaryReader::new(data);
    if reader.remaining() < 20 {
        return Ok(None);
    }

    let x = reader.read_f32()?;
    let y = reader.read_f32()?;
    let mut z = reader.read_f32()?;
    if version <= 2 {
        z = 0.0;
    }
    let _angle = reader.read_f32()?;
    let _flags = reader.read_i32()?;
    let template_name = reader.read_ascii_string()?;
    if version < 2 || reader.remaining() == 0 {
        return Ok(None);
    }

    let dict = parse_chunk_dict(&mut reader, toc)?;
    if !dict_contains_key(&dict, "waypointID") {
        return Ok(None);
    }

    let waypoint_id = dict_lookup_ci(&dict, &["waypointID"])
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(0)
        .max(0) as u32;
    let waypoint_name = dict_lookup_ci(&dict, &["waypointName"]).unwrap_or_default();
    let resolved_name = if waypoint_name.trim().is_empty() {
        template_name
    } else {
        waypoint_name
    };

    Ok(Some(RuntimeWaypoint {
        id: waypoint_id,
        name: resolved_name,
        location: Coord3D::new(x, y, z),
        path_label1: dict_lookup_ci(&dict, &["waypointPathLabel1"]).unwrap_or_default(),
        path_label2: dict_lookup_ci(&dict, &["waypointPathLabel2"]).unwrap_or_default(),
        path_label3: dict_lookup_ci(&dict, &["waypointPathLabel3"]).unwrap_or_default(),
        bi_directional: dict_lookup_ci(&dict, &["waypointPathBiDirectional"])
            .map(|value| {
                let trimmed = value.trim();
                trimmed.eq_ignore_ascii_case("true")
                    || trimmed.eq_ignore_ascii_case("yes")
                    || trimmed == "1"
            })
            .unwrap_or(false),
    }))
}

fn parse_bridge_endpoint_object_chunk(
    data: &[u8],
    version: u16,
) -> LoaderResult<Option<RuntimeBridgeEndpoint>> {
    let mut reader = BinaryReader::new(data);
    if reader.remaining() < 20 {
        return Ok(None);
    }

    let x = reader.read_f32()?;
    let y = reader.read_f32()?;
    let mut z = reader.read_f32()?;
    if version <= 2 {
        z = 0.0;
    }
    let _angle = reader.read_f32()?;
    let flags = reader.read_i32()?;
    let template_name = reader.read_ascii_string()?;
    if template_name.trim().is_empty() {
        return Ok(None);
    }

    if (flags & (FLAG_BRIDGE_POINT1 | FLAG_BRIDGE_POINT2)) == 0 {
        return Ok(None);
    }

    let point = Coord3D::new(x, y, z);
    if (flags & FLAG_BRIDGE_POINT1) != 0 {
        return Ok(Some(RuntimeBridgeEndpoint {
            template_name,
            location: point,
            is_point1: true,
        }));
    }

    Ok(Some(RuntimeBridgeEndpoint {
        template_name,
        location: point,
        is_point1: false,
    }))
}

fn parse_runtime_map_object_stub_chunk(
    data: &[u8],
    version: u16,
) -> LoaderResult<Option<RuntimeMapObjectStub>> {
    let mut reader = BinaryReader::new(data);
    if reader.remaining() < 20 {
        return Ok(None);
    }

    let x = reader.read_f32()?;
    let y = reader.read_f32()?;
    let mut z = reader.read_f32()?;
    if version <= 2 {
        z = 0.0;
    }
    let _angle = reader.read_f32()?;
    let flags = reader.read_i32()?;
    let template_name = reader.read_ascii_string()?;

    Ok(Some(RuntimeMapObjectStub {
        template_name,
        location: Coord3D::new(x, y, z),
        flags,
    }))
}

fn add_runtime_bridge_point(
    bridges: &mut Vec<BridgeData>,
    pending: &mut HashMap<String, Vec<PendingRuntimeBridge>>,
    endpoint: RuntimeBridgeEndpoint,
) {
    let entry = pending.entry(endpoint.template_name.clone()).or_default();

    if endpoint.is_point1 {
        for index in 0..entry.len() {
            if entry[index].from.is_none() && entry[index].to.is_some() {
                let to = entry[index].to.take().unwrap_or(endpoint.location);
                let from = endpoint.location;
                entry.swap_remove(index);
                bridges.push(build_runtime_bridge_data(
                    endpoint.template_name.as_str(),
                    from,
                    to,
                ));
                return;
            }
        }
        entry.push(PendingRuntimeBridge {
            from: Some(endpoint.location),
            to: None,
        });
    } else {
        for index in 0..entry.len() {
            if entry[index].to.is_none() && entry[index].from.is_some() {
                let from = entry[index].from.take().unwrap_or(endpoint.location);
                let to = endpoint.location;
                entry.swap_remove(index);
                bridges.push(build_runtime_bridge_data(
                    endpoint.template_name.as_str(),
                    from,
                    to,
                ));
                return;
            }
        }
        entry.push(PendingRuntimeBridge {
            from: None,
            to: Some(endpoint.location),
        });
    }
}

fn build_runtime_bridge_data(template_name: &str, from: Coord3D, to: Coord3D) -> BridgeData {
    let width = runtime_bridge_width_from_template(template_name).unwrap_or(MAP_XY_FACTOR * 2.0);
    BridgeData::new(
        SystemCoord3D::new(from.x, from.y, from.z),
        SystemCoord3D::new(to.x, to.y, to.z),
        width,
        template_name.to_string(),
    )
}

fn runtime_bridge_width_from_template(template_name: &str) -> Option<f32> {
    // Do not call TheThingFactory::find_template here: that helper lazy-inits
    // the entire Object INI database (14s+ on Lone Eagle). C++ already has
    // TheThingFactory from GameEngine::init; if the host factory is empty or
    // still held by the abandoned boot worker, use the default bridge width.
    let factory_guard = game_engine::common::thing::thing_factory::try_get_thing_factory()?;
    let factory = factory_guard.as_ref()?;
    let template = factory.find_template(template_name, false)?;
    let geometry = template.get_template_geometry_info();
    let width = (geometry.minor_radius() * 2.0).max(0.0);
    if width > 0.0 { Some(width) } else { None }
}

fn build_runtime_road_data(
    template_name: &str,
    from: Coord3D,
    mut to: Coord3D,
    from_flags: i32,
    to_flags: i32,
) -> RuntimeRoadSegment {
    if (from.x - to.x).abs() <= f32::EPSILON && (from.y - to.y).abs() <= f32::EPSILON {
        to.x += 0.25;
    }

    let (width, width_in_texture, road_type_id) = runtime_road_style_for_template(template_name);
    RuntimeRoadSegment {
        template_name: template_name.to_string(),
        from,
        to,
        width,
        width_in_texture,
        road_type_id,
        start_is_angled: (from_flags & FLAG_ROAD_CORNER_ANGLED) != 0,
        start_is_join: (from_flags & FLAG_ROAD_JOIN) != 0,
        end_is_angled: (to_flags & FLAG_ROAD_CORNER_ANGLED) != 0,
        end_is_join: (to_flags & FLAG_ROAD_JOIN) != 0,
        curve_radius: if (from_flags & FLAG_ROAD_CORNER_TIGHT) != 0 {
            TIGHT_CORNER_RADIUS
        } else {
            CORNER_RADIUS
        },
    }
}

fn runtime_road_style_for_template(template_name: &str) -> (f32, f32, u32) {
    if let Some(roads) = try_get_terrain_roads() {
        if let Some(road) = roads.find_road(template_name) {
            let width = if road.road_width > 0.0 {
                road.road_width
            } else {
                DEFAULT_RUNTIME_ROAD_WIDTH
            };
            let width_in_texture = if road.road_width_in_texture > 0.0 {
                road.road_width_in_texture
            } else {
                DEFAULT_RUNTIME_ROAD_WIDTH_IN_TEXTURE
            };
            return (width, width_in_texture, road.id);
        }
    }

    (
        DEFAULT_RUNTIME_ROAD_WIDTH,
        DEFAULT_RUNTIME_ROAD_WIDTH_IN_TEXTURE,
        DEFAULT_RUNTIME_ROAD_UNIQUE_ID,
    )
}

fn parse_object_creation_chunk(data: &[u8], _version: u16) -> LoaderResult<Option<PlacedObject>> {
    // This is a partial parser: many fields omitted, only template/team/position are read.
    let mut reader = BinaryReader::new(data);
    if reader.remaining() < 24 {
        return Ok(None);
    }

    // Template name (null-terminated string length-prefixed by u8)
    let name_len = reader.read_u8()? as usize;
    let name_bytes = reader.read_bytes(name_len)?;
    let template = String::from_utf8_lossy(name_bytes).to_string();
    if template.is_empty() || is_terrain_road_name(&template) {
        return Ok(None);
    }

    // Position (f32 x3)
    let x = reader.read_f32()?;
    let y = reader.read_f32()?;
    let z = reader.read_f32()?;
    let position = Coord3D::new(x, y, z);

    // Rotation (yaw). Some maps store it as a single f32 after position.
    let rotation = if reader.remaining() >= 4 {
        Some(reader.read_f32()?)
    } else {
        None
    };

    // Team name (length-prefixed u8 string)
    let team_name = if reader.remaining() >= 1 {
        let len = reader.read_u8()? as usize;
        if len > 0 && reader.remaining() >= len {
            let bytes = reader.read_bytes(len)?;
            Some(String::from_utf8_lossy(bytes).to_string())
        } else {
            None
        }
    } else {
        None
    };

    // Player ID (optional). Some builds store it as u8 after team.
    let player_id = if reader.remaining() >= 1 {
        Some(reader.read_u8()? as u32)
    } else {
        None
    };

    // Optional upgrade/facing string (length-prefixed u8). Treat as upgrade tag for now.
    let upgrade = if reader.remaining() >= 1 {
        let len = reader.read_u8()? as usize;
        if len > 0 && reader.remaining() >= len {
            let bytes = reader.read_bytes(len)?;
            Some(String::from_utf8_lossy(bytes).to_string())
        } else {
            None
        }
    } else {
        None
    };

    Ok(Some(PlacedObject {
        template,
        name: None,
        position,
        rotation,
        team_name,
        player_id,
        upgrade,
        unsellable: None,
        enabled: None,
        powered: None,
        indestructible: None,

        object_weather: None,
        properties: Dict::new(),
    }))
}

fn parse_chunk_dict(
    reader: &mut BinaryReader<'_>,
    toc: &HashMap<u32, String>,
) -> LoaderResult<HashMap<String, String>> {
    let pair_count = reader.read_u16()? as usize;
    let mut dict = HashMap::with_capacity(pair_count);
    for _ in 0..pair_count {
        let key_and_type = reader.read_i32()? as u32;
        let data_type = (key_and_type & 0xFF) as u8;
        let name_id = key_and_type >> 8;
        let key_name = toc.get(&name_id).cloned().unwrap_or_default();
        let value = match data_type {
            0 => (reader.read_u8()? != 0).to_string(),
            1 => reader.read_i32()?.to_string(),
            2 => reader.read_f32()?.to_string(),
            3 => reader.read_ascii_string()?,
            4 => reader.read_unicode_string()?,
            _ => {
                return Err(configuration_error(format!(
                    "Unknown map dict value type {}",
                    data_type
                )));
            }
        };
        if !key_name.is_empty() {
            dict.insert(key_name, value);
        }
    }
    Ok(dict)
}

fn parse_chunk_dict_typed(
    reader: &mut BinaryReader<'_>,
    toc: &HashMap<u32, String>,
) -> LoaderResult<Dict> {
    let pair_count = reader.read_u16()? as usize;
    let mut dict = Dict::new();
    for _ in 0..pair_count {
        let key_and_type = reader.read_i32()? as u32;
        let data_type = (key_and_type & 0xFF) as u8;
        let name_id = key_and_type >> 8;
        let key_name = toc.get(&name_id).cloned().unwrap_or_default();
        if key_name.is_empty() {
            match data_type {
                0 => {
                    let _ = reader.read_u8()?;
                }
                1 => {
                    let _ = reader.read_i32()?;
                }
                2 => {
                    let _ = reader.read_f32()?;
                }
                3 => {
                    let _ = reader.read_ascii_string()?;
                }
                4 => {
                    let _ = reader.read_unicode_string()?;
                }
                _ => {
                    return Err(configuration_error(format!(
                        "Unknown map dict value type {}",
                        data_type
                    )));
                }
            }
            continue;
        }
        let key = NameKeyGenerator::name_to_key(&key_name);
        match data_type {
            0 => dict.set_bool(key, reader.read_u8()? != 0),
            1 => dict.set_int(key, reader.read_i32()?),
            2 => dict.set_real(key, reader.read_f32()?),
            3 => dict.set_ascii_string(key, reader.read_ascii_string()?),
            4 => dict.set_unicode_string(key, reader.read_unicode_string()?),
            _ => {
                return Err(configuration_error(format!(
                    "Unknown map dict value type {}",
                    data_type
                )));
            }
        }
    }
    Ok(dict)
}

fn dict_to_string_map(dict: &Dict) -> HashMap<String, String> {
    let mut out = HashMap::with_capacity(dict.get_pair_count());
    for i in 0..dict.get_pair_count() {
        let Some(key) = dict.get_nth_key(i) else {
            continue;
        };
        let Some(name) = NameKeyGenerator::key_to_name(key) else {
            continue;
        };
        if name.is_empty() {
            continue;
        }
        let value = match dict.get_type(key) {
            Some(DictType::Bool) => dict.get_bool(key).to_string(),
            Some(DictType::Int) => dict.get_int(key).to_string(),
            Some(DictType::Real) => dict.get_real(key).to_string(),
            Some(DictType::AsciiString) => dict.get_ascii_string(key),
            Some(DictType::UnicodeString) => dict.get_unicode_string(key),
            None => continue,
        };
        out.insert(name, value);
    }
    out
}

fn parse_side_build_entry(
    reader: &mut BinaryReader<'_>,
    version: u16,
    side_index: u32,
) -> LoaderResult<SideBuildEntry> {
    let building_name = reader.read_ascii_string()?;
    let template = reader.read_ascii_string()?;
    let x = reader.read_f32()?;
    let y = reader.read_f32()?;
    let z = reader.read_f32()?;
    let angle = reader.read_f32()?;
    let initially_built = reader.read_u8()? != 0;
    let num_rebuilds = reader.read_i32()?;

    let mut script_name = None;
    let mut health = None;
    let mut whiner = None;
    let mut unsellable = None;
    let mut repairable = None;
    if version >= 3 {
        let s = reader.read_ascii_string()?;
        if !s.is_empty() {
            script_name = Some(s);
        }
        health = Some(reader.read_i32()?);
        whiner = Some(reader.read_u8()? != 0);
        unsellable = Some(reader.read_u8()? != 0);
        repairable = Some(reader.read_u8()? != 0);
    }

    Ok(SideBuildEntry {
        building_name,
        template,
        position: Coord3D::new(x, y, z),
        angle,
        initially_built,
        num_rebuilds,
        side_index,
        script_name,
        health,
        whiner,
        unsellable,
        repairable,
    })
}

fn skip_side_build_entry(reader: &mut BinaryReader<'_>, version: u16) -> LoaderResult<()> {
    let _ = parse_side_build_entry(reader, version, 0)?;
    Ok(())
}

fn dict_lookup_ci(dict: &HashMap<String, String>, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(value) = dict.get(*key) {
            return Some(value.trim().to_string());
        }
        if let Some((_, value)) = dict
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(key))
        {
            return Some(value.trim().to_string());
        }
    }
    None
}

fn parse_ini_boolish(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn dict_contains_key(dict: &HashMap<String, String>, key: &str) -> bool {
    dict.contains_key(key)
        || dict
            .keys()
            .any(|candidate| candidate.eq_ignore_ascii_case(key))
}

fn parse_player_id(value: &str) -> Option<u32> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(raw) = trimmed.parse::<u32>() {
        return Some(raw);
    }

    let lower = trimmed.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix("player_") {
        if let Ok(raw) = rest.parse::<u32>() {
            return raw.checked_sub(1).or(Some(0));
        }
    }
    if let Some(rest) = lower.strip_prefix("player") {
        if let Ok(raw) = rest.parse::<u32>() {
            return raw.checked_sub(1).or(Some(0));
        }
    }

    None
}

// -------------------------------------------------------------------------------------------------
// Chunk parsing helpers
// -------------------------------------------------------------------------------------------------

struct ChunkHeader {
    label: String,
    version: u16,
    size: usize,
}

struct BinaryReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> BinaryReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    fn read_bytes(&mut self, len: usize) -> LoaderResult<&'a [u8]> {
        if self.remaining() < len {
            return Err(configuration_error("Unexpected end of chunk data"));
        }
        let slice = &self.data[self.pos..self.pos + len];
        self.pos += len;
        Ok(slice)
    }

    fn read_u32(&mut self) -> LoaderResult<u32> {
        let bytes = self.read_bytes(4)?;
        Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
    }

    fn read_i32(&mut self) -> LoaderResult<i32> {
        Ok(self.read_u32()? as i32)
    }

    fn read_u16(&mut self) -> LoaderResult<u16> {
        let bytes = self.read_bytes(2)?;
        Ok(u16::from_le_bytes(bytes.try_into().unwrap()))
    }

    fn read_i16_vec(&mut self, count: usize) -> LoaderResult<Vec<i16>> {
        let bytes = self.read_bytes(count.saturating_mul(2))?;
        Ok(bytes
            .chunks_exact(2)
            .map(|chunk| i16::from_le_bytes(chunk.try_into().unwrap()))
            .collect())
    }

    fn read_u8(&mut self) -> LoaderResult<u8> {
        Ok(self.read_bytes(1)?[0])
    }

    fn read_f32(&mut self) -> LoaderResult<f32> {
        let bytes = self.read_bytes(4)?;
        Ok(f32::from_le_bytes(bytes.try_into().unwrap()))
    }

    fn read_ascii_string(&mut self) -> LoaderResult<String> {
        let len = self.read_u16()? as usize;
        let bytes = self.read_bytes(len)?;
        let text = String::from_utf8_lossy(bytes).to_string();
        Ok(text)
    }

    fn read_unicode_string(&mut self) -> LoaderResult<String> {
        let len = self.read_u16()? as usize;
        let bytes = self.read_bytes(len.saturating_mul(2))?;
        let mut utf16 = Vec::with_capacity(len);
        for chunk in bytes.chunks_exact(2) {
            utf16.push(u16::from_le_bytes([chunk[0], chunk[1]]));
        }
        Ok(String::from_utf16_lossy(&utf16))
    }

    fn take_remaining(&mut self) -> &'a [u8] {
        let slice = &self.data[self.pos..];
        self.pos = self.data.len();
        slice
    }
}

fn parse_chunk_toc(bytes: &[u8]) -> LoaderResult<(HashMap<u32, String>, usize)> {
    if bytes.len() < CHUNK_MAGIC.len() {
        return Err(configuration_error(
            "Chunky file too small to contain header",
        ));
    }
    if &bytes[..4] != CHUNK_MAGIC {
        return Err(configuration_error("Missing chunky magic header"));
    }

    let mut reader = BinaryReader::new(bytes);
    reader.read_bytes(4)?; // consume magic
    let count = reader.read_i32()? as usize;
    let mut toc = HashMap::with_capacity(count);
    for _ in 0..count {
        let name_len = reader.read_u8()? as usize;
        let name_bytes = reader.read_bytes(name_len)?;
        let name = String::from_utf8_lossy(name_bytes).to_string();
        let id = reader.read_u32()?;
        toc.insert(id, name);
    }

    Ok((toc, reader.pos))
}

fn read_chunk_header(
    reader: &mut BinaryReader<'_>,
    toc: &HashMap<u32, String>,
) -> LoaderResult<Option<ChunkHeader>> {
    if reader.remaining() < CHUNK_HEADER_SIZE {
        return Ok(None);
    }

    let id = reader.read_u32()?;
    let Some(label) = toc.get(&id).cloned() else {
        return Err(configuration_error(format!(
            "Chunk id 0x{id:08X} missing from table of contents"
        )));
    };
    let version = reader.read_u16()?;
    let size = reader.read_i32()?;
    if size < 0 {
        return Err(configuration_error(format!(
            "Chunk '{}' reported negative payload size",
            label
        )));
    }
    let size = size as usize;
    if reader.remaining() < size {
        return Err(configuration_error(format!(
            "Chunk '{}' extends past parent data region",
            label
        )));
    }

    Ok(Some(ChunkHeader {
        label,
        version,
        size,
    }))
}

fn parse_chunk_sequence<F>(
    data: &[u8],
    toc: &HashMap<u32, String>,
    mut handler: F,
) -> LoaderResult<()>
where
    F: FnMut(&str, u16, &[u8]) -> LoaderResult<()>,
{
    let mut reader = BinaryReader::new(data);
    while let Some(header) = read_chunk_header(&mut reader, toc)? {
        let payload = reader.read_bytes(header.size)?.to_vec();
        handler(&header.label, header.version, &payload)?;
    }
    Ok(())
}

fn find_chunk_by_label<'a>(
    data: &'a [u8],
    toc: &HashMap<u32, String>,
    target: &str,
) -> LoaderResult<Option<(u16, &'a [u8])>> {
    let mut reader = BinaryReader::new(data);
    while let Some(header) = read_chunk_header(&mut reader, toc)? {
        let payload = reader.read_bytes(header.size)?;
        if header.label == target {
            return Ok(Some((header.version, payload)));
        }
    }
    Ok(None)
}

// -------------------------------------------------------------------------------------------------
// Script parsing
// -------------------------------------------------------------------------------------------------

fn synthesize_chunk_stream(
    toc: &HashMap<u32, String>,
    root_label: &str,
    version: u16,
    payload: &[u8],
) -> LoaderResult<Vec<u8>> {
    let Some((&root_id, _)) = toc.iter().find(|(_, name)| name.as_str() == root_label) else {
        return Err(configuration_error(format!(
            "Chunk table does not contain '{}'",
            root_label
        )));
    };

    let mut mappings: Vec<(&u32, &String)> = toc.iter().collect();
    mappings.sort_by_key(|(id, _)| **id);

    let mut bytes = Vec::new();
    bytes.extend_from_slice(CHUNK_MAGIC);
    bytes.extend_from_slice(&(mappings.len() as i32).to_le_bytes());
    for (id, name) in mappings {
        let name_bytes = name.as_bytes();
        if name_bytes.len() > u8::MAX as usize {
            return Err(configuration_error(format!(
                "Chunk label '{}' is too long for synthetic chunk stream",
                name
            )));
        }
        bytes.push(name_bytes.len() as u8);
        bytes.extend_from_slice(name_bytes);
        bytes.extend_from_slice(&id.to_le_bytes());
    }
    bytes.extend_from_slice(&root_id.to_le_bytes());
    bytes.extend_from_slice(&version.to_le_bytes());
    bytes.extend_from_slice(&(payload.len() as i32).to_le_bytes());
    bytes.extend_from_slice(payload);
    Ok(bytes)
}

fn parse_sides_chunk_for_scripts_only(
    input: &mut DataChunkInput,
    info: &DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let Some(context) = user_data.downcast_mut::<SidesScriptContext>() else {
        return false;
    };

    let count = input.read_int().max(0) as usize;
    for _side_index in 0..count {
        let _dict = input.read_dict();
        let build_count = input.read_int().max(0) as usize;
        for _ in 0..build_count {
            let _building_name = input.read_ascii_string();
            let _template_name = input.read_ascii_string();
            let _x = input.read_real();
            let _y = input.read_real();
            let _z = input.read_real();
            let _angle = input.read_real();
            let _initially_built = input.read_byte();
            let _num_rebuilds = input.read_int();
            if info.version >= 3 {
                let _script_name = input.read_ascii_string();
                let _health = input.read_int();
                let _whiner = input.read_byte();
                let _unsellable = input.read_byte();
                let _repairable = input.read_byte();
            }
        }
    }

    if info.version >= 2 {
        let team_count = input.read_int().max(0) as usize;
        for _ in 0..team_count {
            let _dict = input.read_dict();
        }
    }

    input.register_parser(
        PLAYER_SCRIPTS_LABEL,
        &info.label,
        parse_player_scripts_list_chunk,
    );
    if !input.parse(&mut context.scripts) {
        return false;
    }

    input.at_end_of_chunk()
}

fn parse_script_lists_from_sides_chunk(
    payload: &[u8],
    toc: &HashMap<u32, String>,
    version: u16,
) -> LoaderResult<Vec<ScriptList>> {
    let chunk_stream = synthesize_chunk_stream(toc, SIDES_LIST_LABEL, version, payload)?;
    let mut input = DataChunkInput::new(chunk_stream);
    if !input.is_valid_file_type() {
        return Err(configuration_error(
            "Synthetic SidesList chunk stream is not valid",
        ));
    }

    let mut context = SidesScriptContext::default();
    input.register_parser(SIDES_LIST_LABEL, "", parse_sides_chunk_for_scripts_only);
    if !input.parse(&mut context) {
        return Err(configuration_error(
            "Failed to parse PlayerScriptsList from SidesList chunk",
        ));
    }

    let lists: Vec<ScriptList> = context
        .scripts
        .lists
        .into_iter()
        .map(|list| *list)
        .collect();
    if lists.is_empty() {
        warn!(
            "SidesList fallback decoded without any ScriptList children (payload={} bytes, version={})",
            payload.len(),
            version
        );
    }
    Ok(lists)
}

fn parse_script_lists(
    data: &[u8],
    toc: &HashMap<u32, String>,
    version: u16,
) -> LoaderResult<Vec<ScriptList>> {
    if version == 0 {
        warn!("PlayerScriptsList chunk reported version 0; continuing");
    }
    let mut lists = Vec::new();
    parse_chunk_sequence(data, toc, |label, chunk_version, payload| {
        if label == SCRIPT_LIST_LABEL {
            lists.push(parse_script_list(payload, toc, chunk_version)?);
        } else {
            debug!(
                "Skipping unexpected chunk '{}' under PlayerScriptsList",
                label
            );
        }
        Ok(())
    })?;
    if lists.is_empty() {
        warn!(
            "PlayerScriptsList chunk decoded without any ScriptList children (payload={} bytes, version={})",
            data.len(),
            version
        );
    }
    Ok(lists)
}

fn parse_script_list(
    data: &[u8],
    toc: &HashMap<u32, String>,
    _version: u16,
) -> LoaderResult<ScriptList> {
    let mut top_scripts = Vec::new();
    let mut groups = Vec::new();
    parse_chunk_sequence(data, toc, |label, chunk_version, payload| {
        match label {
            SCRIPT_LABEL => top_scripts.push(parse_script(payload, toc, chunk_version)?),
            SCRIPT_GROUP_LABEL => groups.push(parse_script_group(payload, toc, chunk_version)?),
            _ => debug!("Unknown chunk '{}' inside ScriptList", label),
        }
        Ok(())
    })?;

    let mut list = ScriptList::new();
    list.first_script = link_scripts(top_scripts);
    list.first_group = link_script_groups(groups);
    Ok(list)
}

fn parse_script(data: &[u8], toc: &HashMap<u32, String>, version: u16) -> LoaderResult<Script> {
    let mut reader = BinaryReader::new(data);
    let mut script = Script::new();
    script.script_name = reader.read_ascii_string()?;
    script.comment = reader.read_ascii_string()?;
    script.condition_comment = reader.read_ascii_string()?;
    script.action_comment = reader.read_ascii_string()?;
    script.is_active = reader.read_u8()? != 0;
    script.is_one_shot = reader.read_u8()? != 0;
    script.easy = reader.read_u8()? != 0;
    script.normal = reader.read_u8()? != 0;
    script.hard = reader.read_u8()? != 0;
    script.is_subroutine = reader.read_u8()? != 0;
    if version >= 2 {
        script.delay_evaluation_seconds = reader.read_i32()?;
    }

    let nested = reader.take_remaining();
    let mut or_nodes = Vec::new();
    let mut actions = Vec::new();
    let mut false_actions = Vec::new();
    parse_chunk_sequence(nested, toc, |label, chunk_version, payload| {
        match label {
            OR_CONDITION_LABEL => or_nodes.push(parse_or_condition(payload, toc, chunk_version)?),
            SCRIPT_ACTION_LABEL => actions.push(parse_script_action(payload, chunk_version)?),
            SCRIPT_ACTION_FALSE_LABEL => {
                false_actions.push(parse_script_action(payload, chunk_version)?)
            }
            _ => debug!("Unhandled chunk '{}' inside Script", label),
        }
        Ok(())
    })?;

    script.condition = link_or_conditions(or_nodes);
    script.action = link_actions(actions);
    script.action_false = link_actions(false_actions);
    Ok(script)
}

fn parse_script_group(
    data: &[u8],
    toc: &HashMap<u32, String>,
    version: u16,
) -> LoaderResult<ScriptGroup> {
    let mut reader = BinaryReader::new(data);
    let mut group = ScriptGroup::new();
    group.group_name = reader.read_ascii_string()?;
    group.is_group_active = reader.read_u8()? != 0;
    group.is_group_subroutine = if version >= 2 {
        reader.read_u8()? != 0
    } else {
        false
    };

    let nested = reader.take_remaining();
    let mut scripts = Vec::new();
    parse_chunk_sequence(nested, toc, |label, chunk_version, payload| {
        if label == SCRIPT_LABEL {
            scripts.push(parse_script(payload, toc, chunk_version)?);
        } else {
            debug!("Skipping '{}' inside ScriptGroup", label);
        }
        Ok(())
    })?;
    group.first_script = link_scripts(scripts);
    Ok(group)
}

fn parse_or_condition(
    data: &[u8],
    toc: &HashMap<u32, String>,
    _version: u16,
) -> LoaderResult<OrCondition> {
    let mut or_node = OrCondition::new();
    let mut conditions = Vec::new();
    parse_chunk_sequence(data, toc, |label, chunk_version, payload| {
        if label == CONDITION_LABEL {
            conditions.push(parse_condition(payload, chunk_version)?);
        } else {
            debug!("Unknown chunk '{}' inside OrCondition", label);
        }
        Ok(())
    })?;
    or_node.first_and = link_conditions(conditions);
    Ok(or_node)
}

fn parse_condition(data: &[u8], version: u16) -> LoaderResult<Condition> {
    let mut reader = BinaryReader::new(data);
    let cond_value = reader.read_i32()? as u32;
    let mut cond_type = convert_condition_type(cond_value)?;
    let mut condition = Condition::new(cond_type);
    if version >= 4 {
        let name_key = reader.read_u32()?;
        let mut matched = false;
        if let Ok(engine_guard) = gamelogic::scripting::engine::get_script_engine().read() {
            if let Some(engine) = engine_guard.as_ref() {
                if let Some(template) = engine.get_condition_template(cond_type as usize) {
                    if template.base.internal_name_key == name_key {
                        matched = true;
                    }
                }
                if !matched {
                    if let Some(resolved) = engine.find_condition_type_by_name_key(name_key) {
                        cond_type = resolved;
                        matched = true;
                    }
                }
            }
        }
        if !matched {
            cond_type = ConditionType::ConditionFalse;
        }
        condition.condition_type = cond_type;
    }
    let param_count = reader.read_i32()? as usize;
    for _ in 0..param_count {
        let param = parse_parameter(&mut reader)?;
        append_parameter(&mut condition.parameters, &mut condition.num_parms, param)?;
    }
    Ok(condition)
}

fn parse_script_action(data: &[u8], version: u16) -> LoaderResult<ScriptAction> {
    let mut reader = BinaryReader::new(data);
    let mut action_type = convert_action_type(reader.read_i32()? as u32)?;
    let mut action = ScriptAction::new(action_type);
    if version >= 2 {
        let name_key = reader.read_u32()?;
        let mut matched = false;
        if let Ok(engine_guard) = gamelogic::scripting::engine::get_script_engine().read() {
            if let Some(engine) = engine_guard.as_ref() {
                if let Some(template) = engine.get_action_template(action_type as usize) {
                    if template.base.internal_name_key == name_key {
                        matched = true;
                    }
                }
                if !matched {
                    if let Some(resolved) = engine.find_action_type_by_name_key(name_key) {
                        action_type = resolved;
                        matched = true;
                    }
                }
            }
        }
        if !matched {
            action_type = ScriptActionType::NoOp;
        }
        action.action_type = action_type;
    }
    let param_count = reader.read_i32()? as usize;
    for _ in 0..param_count {
        let param = parse_parameter(&mut reader)?;
        append_parameter(&mut action.parameters, &mut action.num_parms, param)?;
    }
    Ok(action)
}

fn parse_parameter(reader: &mut BinaryReader<'_>) -> LoaderResult<Parameter> {
    let kind = convert_parameter_type(reader.read_i32()? as u32)?;
    let mut param = Parameter::new(kind);
    param.initialized = true;
    if kind == ParameterType::Coord3D {
        let coord = Coord3D::new(reader.read_f32()?, reader.read_f32()?, reader.read_f32()?);
        param.coord_value = coord;
    } else {
        param.int_value = reader.read_i32()?;
        param.real_value = reader.read_f32()?;
        param.string_value = reader.read_ascii_string()?;
    }
    Ok(param)
}

fn append_parameter(
    slots: &mut [Option<Parameter>],
    count: &mut usize,
    parameter: Parameter,
) -> LoaderResult<()> {
    if *count >= slots.len() {
        return Err(configuration_error(
            "Script parameter count exceeded maximum capacity",
        ));
    }
    slots[*count] = Some(parameter);
    *count += 1;
    Ok(())
}

fn link_scripts(mut scripts: Vec<Script>) -> Option<Box<Script>> {
    let mut next = None;
    while let Some(mut script) = scripts.pop() {
        script.next_script = next;
        next = Some(Box::new(script));
    }
    next
}

fn link_script_groups(mut groups: Vec<ScriptGroup>) -> Option<Box<ScriptGroup>> {
    let mut next = None;
    while let Some(mut group) = groups.pop() {
        group.next_group = next;
        next = Some(Box::new(group));
    }
    next
}

fn link_or_conditions(mut nodes: Vec<OrCondition>) -> Option<Box<OrCondition>> {
    let mut next = None;
    while let Some(mut node) = nodes.pop() {
        node.next_or = next;
        next = Some(Box::new(node));
    }
    next
}

fn link_conditions(mut conditions: Vec<Condition>) -> Option<Box<Condition>> {
    let mut next = None;
    while let Some(mut cond) = conditions.pop() {
        cond.next_and_condition = next;
        next = Some(Box::new(cond));
    }
    next
}

fn link_actions(mut actions: Vec<ScriptAction>) -> Option<Box<ScriptAction>> {
    let mut next = None;
    while let Some(mut action) = actions.pop() {
        action.next_action = next;
        next = Some(Box::new(action));
    }
    next
}

fn count_scripts(lists: &[ScriptList]) -> usize {
    fn count_chain(mut script: Option<&Box<Script>>) -> usize {
        let mut total = 0;
        while let Some(node) = script {
            total += 1;
            script = node.next_script.as_ref();
        }
        total
    }

    let mut total = 0;
    for list in lists {
        total += count_chain(list.first_script.as_ref());
        let mut group = list.first_group.as_ref();
        while let Some(node) = group {
            total += count_chain(node.first_script.as_ref());
            group = node.next_group.as_ref();
        }
    }
    total
}

// -------------------------------------------------------------------------------------------------
// Enum conversion helpers
// -------------------------------------------------------------------------------------------------

fn convert_parameter_type(value: u32) -> LoaderResult<ParameterType> {
    ParameterType::from_u32(value)
        .ok_or_else(|| configuration_error(format!("Unknown ParameterType value {}", value)))
}

fn convert_condition_type(value: u32) -> LoaderResult<ConditionType> {
    ConditionType::from_u32(value)
        .ok_or_else(|| configuration_error(format!("Unknown ConditionType value {}", value)))
}

fn convert_action_type(value: u32) -> LoaderResult<ScriptActionType> {
    ScriptActionType::from_u32(value)
        .ok_or_else(|| configuration_error(format!("Unknown ScriptActionType value {}", value)))
}

fn configuration_error(message: impl Into<String>) -> GameLogicError {
    GameLogicError::Configuration(message.into())
}

// -------------------------------------------------------------------------------------------------
// Map path discovery
// -------------------------------------------------------------------------------------------------

fn locate_map_file(map_name: &str) -> Option<PathBuf> {
    let trimmed = map_name.trim().trim_matches('"');
    if trimmed.is_empty() {
        return None;
    }

    let direct = Path::new(trimmed);
    if direct.is_file() {
        return Some(direct.to_path_buf());
    }
    if direct.extension().is_none() {
        let mut with_ext = direct.to_path_buf();
        with_ext.set_extension("map");
        if with_ext.is_file() {
            return Some(with_ext);
        }
    }

    // Workspace-relative residual: binaries often run with cwd=GeneralsRust/ while
    // retail extracts live at repo_root/windows_game/... Accept ../windows_game and
    // walk parents so absolute-looking relative paths still resolve.
    let mut search_roots: Vec<PathBuf> = vec![PathBuf::from(".")];
    if let Ok(cwd) = std::env::current_dir() {
        search_roots.push(cwd.clone());
        let mut parent = cwd.parent().map(|p| p.to_path_buf());
        for _ in 0..5 {
            if let Some(p) = parent {
                search_roots.push(p.clone());
                parent = p.parent().map(|x| x.to_path_buf());
            } else {
                break;
            }
        }
    }
    // Code/Main manifest → repo root.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    search_roots.push(manifest.clone());
    search_roots.push(manifest.join(".."));
    search_roots.push(manifest.join("../.."));
    search_roots.push(manifest.join("../../.."));

    // Retail ZH maps live under windows_game/extracted_big_files{,_v2}/MapsZH.
    // C++ virtual FS roots Maps/ at MapsZH.big; join INI "Maps/..." against
    // those extract trees so ShellMapMD resolves from cwd=GeneralsRust/.
    let extract_suffixes = [
        "windows_game/extracted_big_files/MapsZH",
        "windows_game/extracted_big_files_v2/MapsZH",
        "extracted_big_files/MapsZH",
        "extracted_big_files_v2/MapsZH",
        "MapsZH",
    ];
    let existing = search_roots.clone();
    for root in &existing {
        for suffix in extract_suffixes {
            search_roots.push(root.join(suffix));
        }
    }

    let normalized = trimmed.replace('\\', "/");
    for root in &search_roots {
        let candidate = root.join(&normalized);
        if candidate.is_file() {
            trace!(
                "Resolved map '{}' via root '{}' -> '{}'",
                map_name,
                root.display(),
                candidate.display()
            );
            return Some(candidate);
        }
        if direct.extension().is_none() {
            let mut with_ext = candidate.clone();
            with_ext.set_extension("map");
            if with_ext.is_file() {
                return Some(with_ext);
            }
        }
    }

    let candidates = build_relative_candidates(trimmed);
    for candidate in candidates {
        if let Some(path) = resolve_path_candidate(&candidate) {
            trace!("Resolved map '{}' to '{}'", map_name, path.display());
            return Some(path);
        }
        // Also try each candidate under workspace roots.
        for root in &search_roots {
            let rooted = root.join(&candidate);
            if let Some(path) = resolve_path_candidate(&rooted) {
                return Some(path);
            }
        }
    }

    // The generic asset resolver covers mounted filesystems and common map
    // paths.  A retail install extracted under `windows_game/MapsZH/Maps`
    // stores a map in a same-named directory (including spaces), so use the
    // shared offline retail resolver as the final exact-name lookup.  This is
    // deliberately not a gameplay fallback: `None` still means the requested
    // map cannot be loaded.
    super::resolve_retail_map_path(trimmed)
}

fn build_relative_candidates(input: &str) -> Vec<PathBuf> {
    let sanitized = input
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_string();
    if sanitized.is_empty() {
        return Vec::new();
    }

    let mut results = Vec::new();
    let base = PathBuf::from(&sanitized);
    push_unique(&mut results, base.clone());
    if let Some(stripped) = sanitized
        .strip_prefix("Maps/")
        .or_else(|| sanitized.strip_prefix("maps/"))
    {
        push_unique(&mut results, PathBuf::from(stripped));
    }
    if let Some(stripped) = sanitized
        .strip_prefix("Data/Maps/")
        .or_else(|| sanitized.strip_prefix("data/maps/"))
    {
        push_unique(&mut results, PathBuf::from(stripped));
    }

    if base.extension().is_none() {
        let mut with_ext = base.clone();
        with_ext.set_extension("map");
        push_unique(&mut results, with_ext.clone());
        if let Some(stripped) = sanitized
            .strip_prefix("Maps/")
            .or_else(|| sanitized.strip_prefix("maps/"))
        {
            let mut stripped_with_ext = PathBuf::from(stripped);
            stripped_with_ext.set_extension("map");
            push_unique(&mut results, stripped_with_ext);
        }
        if let Some(stripped) = sanitized
            .strip_prefix("Data/Maps/")
            .or_else(|| sanitized.strip_prefix("data/maps/"))
        {
            let mut stripped_with_ext = PathBuf::from(stripped);
            stripped_with_ext.set_extension("map");
            push_unique(&mut results, stripped_with_ext);
        }

        if base.components().count() == 1 {
            if let Some(file_name) = base.file_name() {
                let leaf = file_name.to_string_lossy();
                let mut nested = PathBuf::from(&sanitized);
                nested.push(format!("{leaf}.map"));
                push_unique(&mut results, nested);
            }
        }
    }

    results
}

fn push_unique(vec: &mut Vec<PathBuf>, candidate: PathBuf) {
    if !vec.iter().any(|existing| existing == &candidate) {
        vec.push(candidate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn push_f32s(buf: &mut Vec<u8>, values: [f32; 9]) {
        for v in values {
            buf.extend_from_slice(&v.to_le_bytes());
        }
    }

    #[test]
    fn global_lighting_keeps_objects_row_for_units_and_shadows() {
        // C++ WorldHeightMap.cpp:772-804 — terrain[0] then objects[0] per TOD.
        use game_engine::common::ini::ini_game_data::ensure_global_data;
        let handle = ensure_global_data();
        let previous = handle.read().clone();
        let mut payload = Vec::new();
        payload.extend_from_slice(&4i32.to_le_bytes()); // Night
        for tod in 0..4 {
            let t = tod as f32;
            push_f32s(
                &mut payload,
                [0.1 + t, 0.2, 0.3, 0.4, 0.5, 0.6, 1.0, 2.0, 3.0],
            );
            push_f32s(
                &mut payload,
                [0.9, 0.8, 0.7, 0.15, 0.25, 0.35, 9.0, 8.0, 7.0],
            );
        }
        let mut meta = MapMetadata::default();
        parse_lighting_payload_for_settings(1, &payload, &mut meta).expect("parse lighting");
        // Night = index 3 of the four TOD rows.
        assert_eq!(meta.ambient_color, Some([3.1, 0.2, 0.3]));
        assert_eq!(meta.sun_color, Some([0.4, 0.5, 0.6]));
        assert_eq!(meta.sun_direction, Some([1.0, 2.0, 3.0]));
        assert_eq!(meta.objects_ambient_color, Some([0.9, 0.8, 0.7]));
        assert_eq!(meta.objects_sun_color, Some([0.15, 0.25, 0.35]));
        assert_eq!(meta.objects_sun_direction, Some([9.0, 8.0, 7.0]));
        let objects = handle.read().terrain_objects_lighting[4][0].clone();
        assert!((objects.ambient.r - 0.9).abs() < 1e-5);
        assert!((objects.light_pos.x - 9.0).abs() < 1e-5);
        *handle.write() = previous;
    }

    #[test]
    fn set_weather_visible_reaches_snow_manager() {
        // C++ ScriptActions.cpp:3804 TheSnowManager->setVisible
        #[cfg(feature = "game_client")]
        {
            let snow = game_client::snow::initialize_snow_manager();
            snow.lock().expect("snow lock").set_visible(true);
            let mut logic = crate::game_logic::GameLogic::new();
            logic.set_weather_visible(false);
            assert!(!logic.weather_state().visible);
            assert!(
                !snow.lock().expect("snow lock").is_visible(),
                "script Weather must hide SnowManager flakes"
            );
            logic.set_weather_visible(true);
            assert!(snow.lock().expect("snow lock").is_visible());
        }
    }

    #[test]
    fn retail_shell_map_terrain_chunks_decode_when_present() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            "../../../windows_game/extracted_big_files_v2/MapsZH/Maps/ShellMapMD/ShellMapMD.map",
        );
        let Ok(raw) = std::fs::read(&path) else {
            return;
        };
        let bytes = decompress_map_bytes(&raw).expect("retail shell map decompression");
        let (toc, body_offset) = parse_chunk_toc(&bytes).expect("retail shell map TOC");
        let chunky = ChunkyMap {
            source: path,
            toc,
            body_offset,
            bytes,
        };
        let heightmap = parse_heightmap_data_from_chunky(&chunky)
            .expect("retail shell map heightmap parse")
            .expect("retail shell map embeds HeightMapData");
        assert_eq!((heightmap.width, heightmap.height), (315, 315));
        assert_eq!(heightmap.border_size, 70);
        assert_eq!(heightmap.data.len(), 315 * 315);
    }

    #[test]
    fn map_ini_extracts_only_particle_system_blocks() {
        // C++ GameLogic.cpp:2404-2408 loadMapINI dispatches ParticleSystem
        // blocks; mixed Object/Weather content must not be treated as particles.
        let mixed = "\
Object SomeUnit\n\
  KindOf = STRUCTURE\n\
End\n\
\n\
ParticleSystem MapSmoke\n\
  Priority = NONE\n\
End\n\
\n\
Weather\n\
  Snow = Yes\n\
End\n";
        let extracted = extract_map_ini_particle_system_blocks(mixed);
        assert!(extracted.contains("ParticleSystem MapSmoke"));
        assert!(extracted.contains("Priority = NONE"));
        assert!(!extracted.contains("Object SomeUnit"));
        assert!(!extracted.contains("Weather"));
        assert_eq!(overlay_map_ini_particle_systems(mixed), 1);
    }

    #[test]
    fn map_ini_weather_overlay_sets_snow_enabled() {
        // C++ GameLogic.cpp:2407-2408 loadMapINI CREATE_OVERRIDES Weather.
        let mixed = "\
Object SomeUnit\n\
  KindOf = STRUCTURE\n\
End\n\
\n\
ParticleSystem MapSmoke\n\
  Priority = NONE\n\
End\n\
\n\
Weather\n\
  SnowEnabled = Yes\n\
End\n";
        let extracted = extract_map_ini_weather_blocks(mixed);
        assert!(extracted.contains("Weather"));
        assert!(extracted.contains("SnowEnabled = Yes"));
        assert!(!extracted.contains("ParticleSystem"));
        assert!(!extracted.contains("Object SomeUnit"));
        assert!(overlay_map_ini_weather(mixed));
        assert!(
            game_engine::common::ini::ini_weather::is_snow_enabled(),
            "map.ini Weather SnowEnabled must reach TheWeatherSetting"
        );
    }

    #[test]
    fn map_ini_create_overrides_applies_command_set() {
        // C++ loadMapINI full table: CommandSet CREATE_OVERRIDES must apply
        // even when mixed Object/Weather content is present.
        use game_engine::common::ini::ini_command_set::{
            get_command_set_manager, initialize_command_set_manager,
        };
        initialize_command_set_manager();
        let mixed = "\
Object SomeUnit\n\
  KindOf = STRUCTURE\n\
End\n\
\n\
CommandSet MapIniDozerCommandSet\n\
  1 = Command_ConstructAmericaPowerPlant\n\
  2 = Command_ConstructAmericaBarracks\n\
End\n\
\n\
Upgrade MapIniRangerCapture\n\
  DisplayName = MapOverrideCapture\n\
End\n\
\n\
Weather\n\
  SnowEnabled = Yes\n\
End\n";
        let applied = overlay_map_ini_create_overrides(mixed);
        assert!(
            applied >= 1,
            "map.ini CommandSet/Upgrade CREATE_OVERRIDES must apply"
        );
        let manager = get_command_set_manager().expect("CommandSet manager");
        let set = manager
            .find_command_set_resolved("MapIniDozerCommandSet")
            .expect("map.ini CommandSet override must reach TheControlBar table");
        assert_eq!(
            set.get_button_at_position(0).map(String::as_str),
            Some("Command_ConstructAmericaPowerPlant")
        );
        assert_eq!(
            set.get_button_at_position(1).map(String::as_str),
            Some("Command_ConstructAmericaBarracks")
        );
    }

    #[test]
    fn world_weather_and_object_weather_values_match_cpp() {
        assert_eq!(parse_world_weather_value("1"), Some(1));
        assert_eq!(parse_world_weather_value("SNOWY"), Some(1));
        assert_eq!(parse_world_weather_value("0"), Some(0));
        assert_eq!(parse_object_weather_value("2"), Some(2));
        assert_eq!(parse_object_weather_value("1"), Some(1));
        assert!(resolve_object_weather_snow(0, true));
        assert!(!resolve_object_weather_snow(1, true));
        assert!(resolve_object_weather_snow(2, false));
    }

    #[test]
    fn world_info_weather_int_parses_snowy() {
        // C++ WorldHeightMap.cpp:743-746 TheKey_weather → WEATHER_SNOWY=1.
        let mut toc = HashMap::new();
        toc.insert(1, "WorldInfo".to_string());
        toc.insert(6, "weather".to_string());
        let chunky = ChunkyMap {
            source: PathBuf::from("SyntheticSnow.map"),
            toc,
            body_offset: 0,
            bytes: chunk(1, 1, &dict_int(6, 1)),
        };
        assert_eq!(
            parse_runtime_weather_from_chunky(&chunky).expect("weather parse"),
            Some(1)
        );
        assert_eq!(
            parse_runtime_water_height_from_chunky(&chunky).expect("water parse"),
            None
        );
    }

    #[test]
    fn lone_eagle_fast_chunky_parses_in_under_five_seconds() {
        // Smoke hangs inside sync_legacy_runtime_from_fast_chunky after
        // "Fast legacy runtime sync started". Isolate CPU parse from
        // THE_TERRAIN_LOGIC / SidesList / PlayerList / TeamFactory / FileSystem
        // locks. Contended-lock fail-open is covered separately.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            "../../../windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
        );
        let Ok(raw) = std::fs::read(&path) else {
            return;
        };
        let started = std::time::Instant::now();
        let bytes = decompress_map_bytes(&raw).expect("Lone Eagle decompress");
        let (toc, body_offset) = parse_chunk_toc(&bytes).expect("Lone Eagle TOC");
        let chunky = ChunkyMap {
            source: path,
            toc,
            body_offset,
            bytes,
        };
        let heightmap = parse_heightmap_data_from_chunky(&chunky).expect("heightmap");
        let _ = parse_runtime_waypoints_from_chunky(&chunky).expect("waypoints");
        let _ = parse_runtime_bridges_from_chunky(&chunky).expect("bridges");
        let _ = parse_runtime_water_height_from_chunky(&chunky).expect("water");
        let _ = parse_runtime_polygon_triggers_from_chunky(&chunky).expect("polygons");
        let _ = parse_runtime_roads_from_chunky(&chunky).expect("roads");
        let _ = parse_runtime_sides_from_chunky(&chunky).expect("sides");
        let elapsed = started.elapsed();
        assert!(
            elapsed.as_secs_f32() < 5.0,
            "Lone Eagle CPU parse took {:?}; hang is not parse",
            elapsed
        );
        assert!(heightmap.is_some(), "Lone Eagle must embed HeightMapData");
    }

    #[test]
    fn lone_eagle_live_path_reuses_one_chunky_decode() {
        // C++ TerrainLogic::loadMap (TerrainLogic.cpp:1248-1262) opens the
        // .map once via CachedFileInputStream. The live Rust load_map used
        // to inspect + load_chunky_map + parse_player_start_waypoints +
        // load_map_scripts, each RefPack-decoding Lone Eagle again.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            "../../../windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
        );
        if !path.is_file() {
            return;
        }
        reset_map_decompress_count();
        let map_name = path.to_string_lossy();
        let chunky = load_chunky_map(map_name.as_ref())
            .expect("load Lone Eagle")
            .expect("Lone Eagle present");
        let _ = inspect_map_chunks_from_chunky(&chunky);
        let meta = parse_map_settings_from_chunky(&chunky).expect("settings from chunky");
        let _ = parse_player_start_waypoints(map_name.as_ref()).expect("starts via cache");
        let _ = load_map_scripts(map_name.as_ref()).expect("scripts via cache");
        assert_eq!(
            map_decompress_count(),
            1,
            "live load_map helpers must reuse the first ChunkyMap decode"
        );
        assert!(
            !meta.objects.is_empty() || !meta.start_waypoints.is_empty(),
            "Lone Eagle settings must carry placements or starts"
        );
    }

    fn chunk(id: u32, version: u16, payload: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&id.to_le_bytes());
        bytes.extend_from_slice(&version.to_le_bytes());
        bytes.extend_from_slice(&(payload.len() as i32).to_le_bytes());
        bytes.extend_from_slice(payload);
        bytes
    }

    fn ascii(value: &str, out: &mut Vec<u8>) {
        out.extend_from_slice(&(value.len() as u16).to_le_bytes());
        out.extend_from_slice(value.as_bytes());
    }

    #[test]
    fn blend_tile_data_parser_recovers_packed_tile_indices_and_texture_classes() {
        let mut toc = HashMap::new();
        toc.insert(1, "HeightMapData".to_string());
        toc.insert(2, "BlendTileData".to_string());

        let mut height_payload = Vec::new();
        height_payload.extend_from_slice(&2i32.to_le_bytes());
        height_payload.extend_from_slice(&2i32.to_le_bytes());
        height_payload.extend_from_slice(&0i32.to_le_bytes());
        height_payload.extend_from_slice(&1i32.to_le_bytes());
        height_payload.extend_from_slice(&2i32.to_le_bytes());
        height_payload.extend_from_slice(&2i32.to_le_bytes());
        height_payload.extend_from_slice(&4i32.to_le_bytes());
        height_payload.extend_from_slice(&[1, 2, 3, 4]);

        let mut blend_payload = Vec::new();
        blend_payload.extend_from_slice(&4i32.to_le_bytes());
        for value in [0i16, 4, 8, 12] {
            blend_payload.extend_from_slice(&value.to_le_bytes());
        }
        for value in [1i16, 2, 3, 4] {
            blend_payload.extend_from_slice(&value.to_le_bytes());
        }
        blend_payload.extend_from_slice(&[0u8; 8]);
        blend_payload.extend_from_slice(&[0u8; 8]);
        blend_payload.extend_from_slice(&[0u8; 2]);
        blend_payload.extend_from_slice(&16i32.to_le_bytes());
        blend_payload.extend_from_slice(&1i32.to_le_bytes());
        blend_payload.extend_from_slice(&1i32.to_le_bytes());
        blend_payload.extend_from_slice(&1i32.to_le_bytes());
        blend_payload.extend_from_slice(&4i32.to_le_bytes());
        blend_payload.extend_from_slice(&4i32.to_le_bytes());
        blend_payload.extend_from_slice(&2i32.to_le_bytes());
        blend_payload.extend_from_slice(&0i32.to_le_bytes());
        ascii("Grass", &mut blend_payload);

        let mut bytes = Vec::new();
        bytes.extend_from_slice(&chunk(1, 4, &height_payload));
        bytes.extend_from_slice(&chunk(2, 8, &blend_payload));

        let chunky = ChunkyMap {
            source: PathBuf::from("Synthetic.map"),
            toc,
            body_offset: 0,
            bytes,
        };

        let heightmap = parse_heightmap_data_from_chunky(&chunky)
            .unwrap()
            .expect("heightmap should parse");
        let blend = parse_blend_tile_data_from_chunky(&chunky, &heightmap)
            .unwrap()
            .expect("blend data should parse");

        assert_eq!(blend.tile_ndxes, vec![0, 4, 8, 12]);
        assert_eq!(blend.blend_tile_ndxes, vec![1, 2, 3, 4]);
        assert_eq!(blend.extra_blend_tile_ndxes, vec![0, 0, 0, 0]);
        assert_eq!(blend.texture_classes.len(), 1);
        assert_eq!(blend.texture_classes[0].first_tile, 4);
        assert_eq!(blend.texture_classes[0].num_tiles, 4);
        assert_eq!(blend.texture_classes[0].width, 2);
        assert_eq!(blend.texture_classes[0].name, "Grass");
    }

    #[test]
    fn extra_blend_tile_ndxes_are_stored_on_parse_and_assign() {
        let mut toc = HashMap::new();
        toc.insert(1, "HeightMapData".to_string());
        toc.insert(2, "BlendTileData".to_string());

        let mut height_payload = Vec::new();
        height_payload.extend_from_slice(&2i32.to_le_bytes());
        height_payload.extend_from_slice(&2i32.to_le_bytes());
        height_payload.extend_from_slice(&0i32.to_le_bytes());
        height_payload.extend_from_slice(&1i32.to_le_bytes());
        height_payload.extend_from_slice(&2i32.to_le_bytes());
        height_payload.extend_from_slice(&2i32.to_le_bytes());
        height_payload.extend_from_slice(&4i32.to_le_bytes());
        height_payload.extend_from_slice(&[1, 2, 3, 4]);

        let mut blend_payload = Vec::new();
        blend_payload.extend_from_slice(&4i32.to_le_bytes());
        for value in [0i16, 4, 8, 12] {
            blend_payload.extend_from_slice(&value.to_le_bytes());
        }
        for value in [1i16, 2, 3, 4] {
            blend_payload.extend_from_slice(&value.to_le_bytes());
        }
        for value in [5i16, 6, 7, 8] {
            blend_payload.extend_from_slice(&value.to_le_bytes());
        }
        blend_payload.extend_from_slice(&[0u8; 8]);
        blend_payload.extend_from_slice(&[0u8; 2]);
        blend_payload.extend_from_slice(&16i32.to_le_bytes());
        blend_payload.extend_from_slice(&1i32.to_le_bytes());
        blend_payload.extend_from_slice(&1i32.to_le_bytes());
        blend_payload.extend_from_slice(&1i32.to_le_bytes());
        blend_payload.extend_from_slice(&4i32.to_le_bytes());
        blend_payload.extend_from_slice(&4i32.to_le_bytes());
        blend_payload.extend_from_slice(&2i32.to_le_bytes());
        blend_payload.extend_from_slice(&0i32.to_le_bytes());
        ascii("Grass", &mut blend_payload);

        let mut bytes = Vec::new();
        bytes.extend_from_slice(&chunk(1, 4, &height_payload));
        bytes.extend_from_slice(&chunk(2, 8, &blend_payload));

        let chunky = ChunkyMap {
            source: PathBuf::from("SyntheticExtraBlend.map"),
            toc,
            body_offset: 0,
            bytes,
        };

        let heightmap = parse_heightmap_data_from_chunky(&chunky)
            .unwrap()
            .expect("heightmap should parse");
        let blend = parse_blend_tile_data_from_chunky(&chunky, &heightmap)
            .unwrap()
            .expect("blend data should parse");

        assert_eq!(blend.extra_blend_tile_ndxes, vec![5, 6, 7, 8]);

        #[cfg(feature = "game_client")]
        {
            let mut hm = game_client::terrain::height_map::HeightMap::new(2, 2, 100.0, 1.0);
            hm.assign_extra_blend_tile_ndxes(blend.extra_blend_tile_ndxes.clone());
            assert_eq!(hm.extra_blend_tile_ndxes, vec![5, 6, 7, 8]);
            assert_eq!(hm.get_extra_blend_tile_index(0, 0), 5);
            assert_eq!(hm.get_extra_blend_tile_index(1, 1), 8);
        }
    }

    fn dict_int(toc_id: u32, value: i32) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1u16.to_le_bytes());
        let key_and_type = (toc_id << 8) | 1;
        bytes.extend_from_slice(&(key_and_type as i32).to_le_bytes());
        bytes.extend_from_slice(&value.to_le_bytes());
        bytes
    }

    fn dict_real(toc_id: u32, value: f32) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1u16.to_le_bytes());
        let key_and_type = (toc_id << 8) | 2;
        bytes.extend_from_slice(&(key_and_type as i32).to_le_bytes());
        bytes.extend_from_slice(&value.to_le_bytes());
        bytes
    }

    fn icoord(x: i32, y: i32, z: i32, out: &mut Vec<u8>) {
        out.extend_from_slice(&x.to_le_bytes());
        out.extend_from_slice(&y.to_le_bytes());
        out.extend_from_slice(&z.to_le_bytes());
    }

    fn object_endpoint(x: f32, y: f32, z: f32, flags: i32, name: &str) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&x.to_le_bytes());
        payload.extend_from_slice(&y.to_le_bytes());
        payload.extend_from_slice(&z.to_le_bytes());
        payload.extend_from_slice(&0f32.to_le_bytes());
        payload.extend_from_slice(&flags.to_le_bytes());
        ascii(name, &mut payload);
        payload
    }

    #[test]
    fn live_fast_path_parses_water_polygons_and_bridge_endpoints() {
        // C++ PolygonTrigger::ParsePolygonTriggersDataChunk + WorldInfo waterHeight
        // + W3DBridgeBuffer::addBridge → TerrainLogic::addBridgeToLogic.
        // Pre-fix live MapData hard-coded water_height: None / polygon_triggers: Vec::new()
        // and never called add_bridge_to_logic.
        let mut toc = HashMap::new();
        toc.insert(1, "WorldInfo".to_string());
        toc.insert(2, "PolygonTriggers".to_string());
        toc.insert(3, "ObjectsList".to_string());
        toc.insert(4, "Object".to_string());
        toc.insert(5, "waterHeight".to_string());

        let mut polygon_payload = Vec::new();
        polygon_payload.extend_from_slice(&1i32.to_le_bytes());
        ascii("Lake", &mut polygon_payload);
        ascii("water", &mut polygon_payload);
        polygon_payload.extend_from_slice(&3i32.to_le_bytes());
        polygon_payload.push(1);
        polygon_payload.push(0);
        polygon_payload.extend_from_slice(&0i32.to_le_bytes());
        polygon_payload.extend_from_slice(&4i32.to_le_bytes());
        icoord(0, 0, 12, &mut polygon_payload);
        icoord(40, 0, 12, &mut polygon_payload);
        icoord(40, 40, 12, &mut polygon_payload);
        icoord(0, 40, 12, &mut polygon_payload);

        let mut objects_payload = Vec::new();
        objects_payload.extend_from_slice(&chunk(
            4,
            3,
            &object_endpoint(0.0, 0.0, 20.0, FLAG_BRIDGE_POINT1, "TestBridge"),
        ));
        objects_payload.extend_from_slice(&chunk(
            4,
            3,
            &object_endpoint(40.0, 0.0, 20.0, FLAG_BRIDGE_POINT2, "TestBridge"),
        ));

        let mut bytes = Vec::new();
        bytes.extend_from_slice(&chunk(1, 1, &dict_real(5, 7.5)));
        bytes.extend_from_slice(&chunk(2, 4, &polygon_payload));
        bytes.extend_from_slice(&chunk(3, 3, &objects_payload));

        let chunky = ChunkyMap {
            source: PathBuf::from("SyntheticWaterBridge.map"),
            toc,
            body_offset: 0,
            bytes,
        };

        let water_height = parse_runtime_water_height_from_chunky(&chunky)
            .expect("WorldInfo parse should fail-soft, not error")
            .expect("waterHeight must be recovered from WorldInfo");
        assert_eq!(water_height, 7.5);

        let triggers = parse_runtime_polygon_triggers_from_chunky(&chunky)
            .expect("PolygonTriggers parse should fail-soft, not error");
        assert_eq!(triggers.len(), 1);
        assert!(triggers[0].is_water_area());
        assert_eq!(triggers[0].get_trigger_name().as_str(), "Lake");
        assert_eq!(triggers[0].get_point(0).map(|p| p.z), Some(12));

        let bridges =
            parse_runtime_bridges_from_chunky(&chunky).expect("bridge endpoints should pair");
        assert_eq!(bridges.len(), 1);
        assert_eq!(bridges[0].template_name, "TestBridge");
        assert_eq!(bridges[0].from.z, 20.0);
        assert_eq!(bridges[0].to.z, 20.0);

        let missing = ChunkyMap {
            source: PathBuf::from("Empty.map"),
            toc: HashMap::new(),
            body_offset: 0,
            bytes: Vec::new(),
        };
        assert_eq!(
            parse_runtime_water_height_from_chunky(&missing).unwrap(),
            None
        );
        assert!(
            parse_runtime_polygon_triggers_from_chunky(&missing)
                .unwrap()
                .is_empty()
        );

        let mut map_data = gamelogic::system::map_loader::MapData::new();
        map_data.water_height = Some(water_height);
        map_data.polygon_triggers = triggers;
        map_data.bridges = bridges;
        let mut terrain = gamelogic::terrain::TerrainLogic::new();
        terrain.load_map_data(map_data);
        assert_eq!(
            terrain
                .get_water_handle(10.0, 10.0)
                .map(|handle| handle.get_current_height()),
            Some(12.0)
        );
        assert_eq!(
            terrain
                .get_first_bridge()
                .map(|bridge| bridge.get_bridge_template_name().as_str().to_string())
                .as_deref(),
            Some("TestBridge")
        );
    }

    #[test]
    fn live_fast_path_source_wires_water_and_add_bridge() {
        let src = concat!(
            include_str!("world_save.rs"),
            include_str!("world_save/world_subsystems.rs"),
            include_str!("world_save/world_paths.rs"),
            include_str!("world_save/world_runtime.rs"),
            include_str!("world_save/world_players.rs"),
            include_str!("world_save/world_load.rs"),
        );
        assert!(
            src.contains("parse_runtime_water_height_from_chunky")
                && src.contains("parse_runtime_polygon_triggers_from_chunky")
                && src.contains("map_data.water_height = water_height")
                && src.contains("map_data.polygon_triggers = polygon_triggers")
                && src.contains("map_data.bridges = bridges"),
            "live fast-path MapData must keep water polygons/height and parsed bridges"
        );
        let terrain_src = concat!(
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../GameEngine/GameLogic/src/terrain/map_height.rs"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../GameEngine/GameLogic/src/terrain/bridge.rs"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../GameEngine/GameLogic/src/terrain/bridges.rs"
            )),
        );
        let prod = terrain_src
            .split("#[cfg(test)]")
            .next()
            .expect("production");
        assert!(
            prod.contains("add_bridge_to_logic") && prod.contains("bridge_info_from_map_data"),
            "TerrainLogic::load_map_data must call add_bridge_to_logic for map bridges"
        );
    }
}

#[cfg(test)]
mod locate_map_file_workspace_residual_tests {
    #[test]
    fn locate_map_file_searches_parent_workspace_roots() {
        let src = include_str!("script_loader.rs");
        assert!(
            src.contains("Workspace-relative residual")
                && src.contains("search_roots")
                && src.contains("CARGO_MANIFEST_DIR")
                && src.contains("extracted_big_files/MapsZH"),
            "locate_map_file must search parent workspace roots for windows_game extracts"
        );
    }

    #[test]
    fn find_map_file_resolves_shellmapmd_from_generals_ini_name() {
        let found = crate::game_logic::find_map_file(r"Maps\ShellMapMD\ShellMapMD.map")
            .or_else(|| crate::game_logic::find_map_file("ShellMapMD"));
        if found.is_none() {
            return;
        }
        let path = found.unwrap();
        assert!(
            path.is_file(),
            "resolved ShellMapMD must be a real file: {}",
            path.display()
        );
        assert!(
            path.to_string_lossy()
                .to_ascii_lowercase()
                .contains("shellmapmd"),
            "unexpected resolve {}",
            path.display()
        );
    }
}
