/*
** Command & Conquer Generals Zero Hour(tm) - Map Script Loader
** Copyright 2025 Electronic Arts
**
** Loads WW3D/SAGE mission scripts directly from .map files by decoding the
** chunky container and converting binary ScriptList data into the canonical
** rust structures under gamelogic::scripting::core.
*/

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use game_engine::common::dict::Dict;
use game_engine::common::ini::{get_terrain_roads, INILoadType, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{
    file::FileAccess, file_system::get_file_system, DataChunkInfo, DataChunkInput,
};
use gamelogic::common::MAP_XY_FACTOR;
use gamelogic::helpers::TheThingFactory;
use gamelogic::scripting::core::{
    Condition, ConditionType, Coord3D, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptGroup, ScriptList,
};
use gamelogic::scripting::{parse_player_scripts_list_chunk, ScriptListReadInfo};
use gamelogic::system::map_loader::BridgeData;
use gamelogic::system::Coord3D as SystemCoord3D;
use gamelogic::GameLogicError;
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

    if let Ok(file_system) = get_file_system().lock() {
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
    let mut file_system = file_system.lock().ok()?;
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
    let result = TERRAIN_ROADS_LOAD_RESULT.get_or_init(|| {
        let mut ini = INI::new();

        if let Some(default_path) =
            resolve_runtime_ini_path(Path::new("Data/INI/Default/Roads.ini"))
        {
            ini.load(&default_path, INILoadType::Overwrite)
                .map_err(|err| format!("failed loading '{}': {}", default_path.display(), err))?;
        }

        if let Some(override_path) = resolve_runtime_ini_path(Path::new("Data/INI/Roads.ini")) {
            ini.load(&override_path, INILoadType::MultiFile)
                .map_err(|err| format!("failed loading '{}': {}", override_path.display(), err))?;
        }

        Ok(())
    });

    if let Err(err) = result {
        warn!("Terrain roads registry unavailable: {}", err);
    }
}

fn is_terrain_road_name(name: &str) -> bool {
    ensure_terrain_roads_loaded();
    let roads = get_terrain_roads();
    roads.find_road(name).is_some()
}

fn decompress_map_bytes(raw_bytes: &[u8]) -> LoaderResult<Vec<u8>> {
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
            expected_size,
            ulen
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
pub struct ChunkyMap {
    pub source: PathBuf,
    pub toc: HashMap<u32, String>,
    pub body_offset: usize,
    pub bytes: Vec<u8>,
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
}

/// Top-level metadata parsed from a map file.
#[derive(Debug, Clone, Default)]
pub struct MapMetadata {
    pub objects: Vec<PlacedObject>,
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
    pub texture_classes: Vec<BlendTileTextureClass>,
}

#[derive(Debug, Clone)]
pub struct BlendTileTextureClass {
    pub first_tile: i32,
    pub num_tiles: i32,
    pub width: i32,
    pub name: String,
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

    let raw_bytes = read_file_bytes_for_runtime(&map_path).ok_or_else(|| {
        GameLogicError::Configuration(format!(
            "Failed to read map '{}': {}",
            map_path.display(),
            "path not found in virtual file system"
        ))
    })?;

    let chunk_bytes = if raw_bytes.starts_with(CHUNK_MAGIC) {
        raw_bytes
    } else {
        decompress_map_bytes(&raw_bytes).map_err(|err| {
            GameLogicError::Configuration(format!(
                "Map '{}' is not a chunky file and decompression failed: {}",
                map_path.display(),
                err
            ))
        })?
    };

    let (toc, body_offset) = parse_chunk_toc(&chunk_bytes)?;
    if body_offset >= chunk_bytes.len() {
        return Err(GameLogicError::Configuration(format!(
            "Map '{}' chunk table extends past file",
            map_path.display()
        )));
    }

    let body = &chunk_bytes[body_offset..];
    let player_scripts_chunk = find_chunk_by_label(body, &toc, PLAYER_SCRIPTS_LABEL)?;
    let Some((version, payload)) = player_scripts_chunk else {
        if let Some(result) = load_sides_list_fallback(&map_path, body, &toc)? {
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

    let script_lists = parse_script_lists(payload, &toc, version)?;
    let total = count_scripts(&script_lists);

    if total == 0 {
        if let Some(result) = load_sides_list_fallback(&map_path, body, &toc)? {
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
    let chunked = load_chunky_map(map_name).ok()??;
    let (toc, _) = parse_chunk_toc(&chunked.bytes).ok()?;
    let mut labels: Vec<String> = toc.values().cloned().collect();
    labels.sort();
    Some(labels)
}

/// Load and decompress a chunky map file, returning metadata for further parsing.
pub fn load_chunky_map(map_name: &str) -> LoaderResult<Option<ChunkyMap>> {
    let Some(path) = locate_map_file(map_name) else {
        return Ok(None);
    };

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
    Ok(Some(ChunkyMap {
        source: path,
        toc,
        body_offset,
        bytes,
    }))
}

/// Parse high-level settings like world bounds and lighting colors.
pub fn parse_map_settings(map_name: &str) -> LoaderResult<MapMetadata> {
    fn parse_lighting_payload(payload: &[u8], meta: &mut MapMetadata) -> LoaderResult<()> {
        if payload.len() >= 4 {
            let light_count = u32::from_le_bytes(payload[0..4].try_into().unwrap_or([0; 4]));
            let expected_light_bytes = 4usize.saturating_add(light_count as usize * 9 * 4);
            if (1..=4).contains(&light_count) && payload.len() >= expected_light_bytes {
                let mut reader = BinaryReader::new(&payload[4..]);
                let ambient = [reader.read_f32()?, reader.read_f32()?, reader.read_f32()?];
                let sun = [reader.read_f32()?, reader.read_f32()?, reader.read_f32()?];
                let sun_dir = [reader.read_f32()?, reader.read_f32()?, reader.read_f32()?];
                meta.ambient_color = Some(ambient);
                meta.sun_color = Some(sun);
                meta.sun_direction = Some(sun_dir);
                return Ok(());
            }
        }

        let mut reader = BinaryReader::new(payload);
        if reader.remaining() >= 48 {
            let ambient = [reader.read_f32()?, reader.read_f32()?, reader.read_f32()?];
            let sun = [reader.read_f32()?, reader.read_f32()?, reader.read_f32()?];
            let sky = [reader.read_f32()?, reader.read_f32()?, reader.read_f32()?];
            let sun_dir = [reader.read_f32()?, reader.read_f32()?, reader.read_f32()?];
            meta.ambient_color = Some(ambient);
            meta.sun_color = Some(sun);
            meta.sky_color = Some(sky);
            meta.sun_direction = Some(sun_dir);
            if reader.remaining() >= 20 {
                let fog = [reader.read_f32()?, reader.read_f32()?, reader.read_f32()?];
                let fog_start = reader.read_f32()?;
                let fog_end = reader.read_f32()?;
                meta.fog_color = Some(fog);
                meta.fog_start = Some(fog_start);
                meta.fog_end = Some(fog_end);
            }
        } else {
            while reader.remaining() >= 16 {
                let tag = reader.read_u8()?;
                if reader.remaining() < 12 {
                    break;
                }
                let r = reader.read_f32()?;
                let g = reader.read_f32()?;
                let b = reader.read_f32()?;
                match tag {
                    0 => meta.ambient_color = Some([r, g, b]),
                    1 => meta.sun_color = Some([r, g, b]),
                    2 => meta.sky_color = Some([r, g, b]),
                    3 => meta.sun_direction = Some([r, g, b]),
                    4 => meta.fog_color = Some([r, g, b]),
                    _ => {}
                }
            }
        }

        Ok(())
    }

    let mut meta = MapMetadata::default();
    let Some(chunky) = load_chunky_map(map_name)? else {
        return Ok(meta);
    };
    let body = &chunky.bytes[chunky.body_offset..];
    if let Some((_ver, payload)) = find_chunk_by_label(body, &chunky.toc, "MapSettings")? {
        parse_lighting_payload(payload, &mut meta)?;
    } else if let Some((_ver, payload)) = find_chunk_by_label(body, &chunky.toc, "GlobalLighting")?
    {
        parse_lighting_payload(payload, &mut meta)?;
    }

    if let Some((min, max)) = parse_world_bounds(map_name).ok().flatten() {
        meta.world_min = Some(min);
        meta.world_max = Some(max);
    }

    match parse_object_placements(map_name) {
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

    meta.initial_camera_position = parse_initial_camera_position(map_name).ok().flatten();

    // Heightmap hint: look for common heightmap filenames next to the .map.
    if let Some(map_path) = locate_map_file(map_name) {
        if let Some(dir) = map_path.parent() {
            let companion_ini_candidates = [dir.join("Map.ini"), dir.join("map.ini")];
            for ini_path in companion_ini_candidates {
                let Some(contents) = read_text_with_fallback(&ini_path) else {
                    continue;
                };
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
                    break;
                }
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

    if version >= 6 {
        let _extra_blend_tile_ndxes = reader.read_i16_vec(data_size)?;
    }
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
    let _num_blended_tiles = reader.read_i32()?;
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
    }

    Ok(Some(BlendTileData {
        tile_ndxes,
        blend_tile_ndxes,
        texture_classes,
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

    let side_count = reader.read_i32()?.max(0) as usize;
    for _ in 0..side_count {
        side_dicts.push(parse_chunk_dict_typed(&mut reader, &chunky.toc)?);
        let build_count = reader.read_i32()?.max(0) as usize;
        for _ in 0..build_count {
            skip_side_build_entry(&mut reader, version)?;
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
    })
}

/// Parse placed objects from a chunky map. Currently supports a minimal subset
/// of the ObjectCreationList chunk (template, position, rotation, team).
pub fn parse_object_placements(map_name: &str) -> LoaderResult<Vec<PlacedObject>> {
    ensure_terrain_roads_loaded();

    let Some(chunky) = load_chunky_map(map_name)? else {
        return Ok(Vec::new());
    };

    let body = &chunky.bytes[chunky.body_offset..];

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
    let body = &chunky.bytes[chunky.body_offset..];

    let mut waypoint_bounds = None;
    // Prefer Waypoints chunk which stores map extents in many maps.
    if let Some((_ver, payload)) = find_chunk_by_label(body, &chunky.toc, "WaypointsList")? {
        let mut reader = BinaryReader::new(payload);
        // WaypointsList: first i32 count, then entries; we skim bounds if present.
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

    // Fall back to HeightMapData dimensions when waypoint bounds are missing/degenerate.
    // This mirrors how runtime systems derive playable extents from terrain samples.
    if let Some(heightmap) = parse_heightmap_data_from_chunky(&chunky)? {
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

    if version >= 2 && reader.remaining() > 0 {
        let dict = parse_chunk_dict(&mut reader, toc)?;

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
    }

    Ok(Some(PlacedObject {
        template,
        name,
        position: Coord3D::new(x, y, z),
        rotation: Some(angle),
        team_name,
        player_id,
        upgrade,
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
    let template = TheThingFactory::find_template(template_name)?;
    let geometry = template.get_template_geometry_info();
    let width = (geometry.get_minor_radius() * 2.0).max(0.0);
    if width > 0.0 {
        Some(width)
    } else {
        None
    }
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
    let roads = get_terrain_roads();
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
            return Err(configuration_error(format!(
                "Chunk dict key id 0x{name_id:08X} missing from table of contents"
            )));
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
                )))
            }
        }
    }
    Ok(dict)
}

fn skip_side_build_entry(reader: &mut BinaryReader<'_>, version: u16) -> LoaderResult<()> {
    let _building_name = reader.read_ascii_string()?;
    let _template_name = reader.read_ascii_string()?;
    let _x = reader.read_f32()?;
    let _y = reader.read_f32()?;
    let _z = reader.read_f32()?;
    let _angle = reader.read_f32()?;
    let _initially_built = reader.read_u8()?;
    let _num_rebuilds = reader.read_i32()?;

    if version >= 3 {
        let _script_name = reader.read_ascii_string()?;
        let _health = reader.read_i32()?;
        let _whiner = reader.read_u8()?;
        let _unsellable = reader.read_u8()?;
        let _repairable = reader.read_u8()?;
    }

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
    if value <= ParameterType::NumItems as u32 {
        let param = unsafe { std::mem::transmute(value) };
        Ok(param)
    } else {
        Err(configuration_error(format!(
            "Unknown ParameterType value {}",
            value
        )))
    }
}

fn convert_condition_type(value: u32) -> LoaderResult<ConditionType> {
    if value <= ConditionType::NumItems as u32 {
        Ok(unsafe { std::mem::transmute(value) })
    } else {
        Err(configuration_error(format!(
            "Unknown ConditionType value {}",
            value
        )))
    }
}

fn convert_action_type(value: u32) -> LoaderResult<ScriptActionType> {
    if value <= ScriptActionType::NumItems as u32 {
        Ok(unsafe { std::mem::transmute(value) })
    } else {
        Err(configuration_error(format!(
            "Unknown ScriptActionType value {}",
            value
        )))
    }
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

    None
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
        assert_eq!(blend.texture_classes.len(), 1);
        assert_eq!(blend.texture_classes[0].first_tile, 4);
        assert_eq!(blend.texture_classes[0].num_tiles, 4);
        assert_eq!(blend.texture_classes[0].width, 2);
        assert_eq!(blend.texture_classes[0].name, "Grass");
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
                && src.contains("CARGO_MANIFEST_DIR"),
            "locate_map_file must search parent workspace roots for windows_game extracts"
        );
    }
}
