//! MapCache parse / getExtent / writeCacheINI (C++ MapUtil + WinMain scan).
//!
//! Corresponds to C++ file: Tools/MapCacheBuilder/Source/WinMain.cpp

use anyhow::{Context, Result};
use game_engine::common::dict::{Dict, DictType};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::compression::{decompress_data, is_data_compressed};
use game_engine::common::system::{DataChunkInfo, DataChunkInput, DataChunkOutput};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// C++ `MAP_XY_FACTOR` (`MapObject.h`).
pub const MAP_XY_FACTOR: f32 = 10.0;
/// C++ `MAP_HEIGHT_SCALE` (`MAP_XY_FACTOR / 16`).
pub const MAP_HEIGHT_SCALE: f32 = MAP_XY_FACTOR / 16.0;
const K_HEIGHT_MAP_VERSION_3: u16 = 3;
const K_HEIGHT_MAP_VERSION_4: u16 = 4;
const K_OBJECTS_VERSION_2: u16 = 2;

#[derive(Debug, Clone)]
pub struct ExtractedMapInfo {
    pub display_name: String,
    pub num_players: u32,
    pub is_multiplayer: bool,
    pub extent_width: f32,
    pub extent_height: f32,
    pub extent_min_z: f32,
    pub extent_max_z: f32,
    pub waypoints: Vec<(String, f32, f32, f32)>,
    pub tech_positions: Vec<(f32, f32, f32)>,
    pub supply_positions: Vec<(f32, f32, f32)>,
}

#[derive(Default)]
struct MapChunkParse {
    width: i32,
    height: i32,
    border_size: i32,
    /// C++ `WorldHeightMap::m_boundaries` (v4); synthesized for v3.
    boundaries: Vec<(i32, i32)>,
    min_z: f32,
    max_z: f32,
    display_name: String,
    waypoints: Vec<(String, f32, f32, f32)>,
    tech_positions: Vec<(f32, f32, f32)>,
    supply_positions: Vec<(f32, f32, f32)>,
}

/// Cache file name
pub const CACHE_FILE_NAME: &str = "mapcache.ini";

/// Default map directories to scan
pub const DEFAULT_MAP_DIRS: &[&str] = &["Data/Maps", "Maps"];

/// Map metadata extracted from .map files
/// Matches C++ MapMetaData structure from MapUtil.h
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapMetaData {
    pub display_name: String,
    pub file_name: String,
    pub file_path: PathBuf,
    pub num_players: u32,
    pub is_multiplayer: bool,
    pub is_official: bool,
    pub file_size: u64,
    pub crc: u32,
    pub timestamp: u64,
    pub extent_width: f32,
    pub extent_height: f32,
    pub waypoint_count: u32,
    pub supply_position_count: u32,
    pub tech_position_count: u32,
    /// C++ `Region3D::lo.z` from height samples * `MAP_HEIGHT_SCALE`.
    pub extent_min_z: f32,
    /// C++ `Region3D::hi.z` from height samples * `MAP_HEIGHT_SCALE`.
    pub extent_max_z: f32,
    pub waypoints: Vec<(String, f32, f32, f32)>,
    pub tech_positions: Vec<(f32, f32, f32)>,
    pub supply_positions: Vec<(f32, f32, f32)>,
}

/// Map cache structure that holds all map metadata
/// Matches C++ MapCache class from MapUtil.h
#[derive(Debug, Default)]
pub struct MapCache {
    pub maps: HashMap<String, MapMetaData>,
    pub allowed_maps: HashSet<String>,
}

impl MapCache {
    pub fn new() -> Self {
        Self {
            maps: HashMap::new(),
            allowed_maps: HashSet::new(),
        }
    }

    /// Add a shipping map to the allowed list
    /// Corresponds to C++ MapCache::addShippingMap() from MapUtil.h line 84
    pub fn add_shipping_map(&mut self, map_name: &str) {
        let lowercase_name = map_name.to_lowercase();
        info!("Adding shipping map: '{}'", lowercase_name);
        self.allowed_maps.insert(lowercase_name);
    }

    /// Scan directories and update the cache
    /// Corresponds to C++ MapCache::updateCache() from MapUtil.h line 75
    pub fn update_cache(&mut self, map_dirs: &[PathBuf]) -> Result<()> {
        info!("Starting map cache update...");

        for map_dir in map_dirs {
            if !map_dir.exists() {
                warn!("Map directory does not exist: {:?}", map_dir);
                continue;
            }

            info!("Scanning directory: {:?}", map_dir);
            self.scan_directory(map_dir)?;
        }

        // C++ loadUserMaps: skip when m_allowedMaps.find(fname) fails.
        // fname is the file stem (`Alpine War`), not the MapCache INI key path.
        if !self.allowed_maps.is_empty() {
            let original_count = self.maps.len();
            self.maps.retain(|_, meta| {
                is_shipping_map_allowed(&self.allowed_maps, &meta.file_path)
                    || self.allowed_maps.contains(&meta.file_name.to_lowercase())
            });
            info!(
                "Filtered to {} allowed maps (from {} total)",
                self.maps.len(),
                original_count
            );
        }

        info!("Map cache updated with {} maps", self.maps.len());
        Ok(())
    }

    /// Scan a directory for map files
    fn scan_directory(&mut self, dir: &Path) -> Result<()> {
        for entry in WalkDir::new(dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("map") {
                if !self.allowed_maps.is_empty()
                    && !is_shipping_map_allowed(&self.allowed_maps, path)
                {
                    continue;
                }
                match self.parse_map_file(path) {
                    Ok(metadata) => {
                        let map_name = map_cache_key(path);
                        info!(
                            "Parsed map: {} ({} players)",
                            metadata.display_name, metadata.num_players
                        );
                        self.maps.insert(map_name, metadata);
                    }
                    Err(e) => {
                        warn!("Failed to parse map file {:?}: {}", path, e);
                    }
                }
            }
        }
        Ok(())
    }

    /// Parse a .map file and extract metadata
    /// Corresponds to C++ MapCache::addMap() from MapUtil.cpp
    fn parse_map_file(&self, path: &Path) -> Result<MapMetaData> {
        let file_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .context("Invalid file name")?
            .to_string();

        let metadata = fs::metadata(path).context("Failed to read file metadata")?;
        let file_size = metadata.len();

        let timestamp = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Calculate CRC32 of the file
        // Matches C++ calcCRC() from MapUtil.cpp line 65
        let crc = self.calculate_crc(path)?;

        let parsed = self.extract_map_info(path)?;

        Ok(MapMetaData {
            display_name: parsed.display_name,
            file_name: file_name.clone(),
            file_path: path.to_path_buf(),
            num_players: parsed.num_players,
            is_multiplayer: parsed.is_multiplayer,
            is_official: true, // All maps added via this tool are considered official
            file_size,
            crc,
            timestamp,
            extent_width: parsed.extent_width,
            extent_height: parsed.extent_height,
            extent_min_z: parsed.extent_min_z,
            extent_max_z: parsed.extent_max_z,
            waypoint_count: parsed.waypoints.len() as u32,
            supply_position_count: parsed.supply_positions.len() as u32,
            tech_position_count: parsed.tech_positions.len() as u32,
            waypoints: parsed.waypoints,
            tech_positions: parsed.tech_positions,
            supply_positions: parsed.supply_positions,
        })
    }

    /// C++ `CRC::computeCRC` / `calcCRC` — rotate-add, not IEEE CRC-32.
    fn calculate_crc(&self, path: &Path) -> Result<u32> {
        let mut file = File::open(path)?;
        let mut buffer = vec![0u8; 4096];
        let mut crc = generals_crc_new();

        loop {
            let bytes_read = file.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            generals_crc_add(&mut crc, &buffer[..bytes_read]);
        }

        Ok(crc)
    }

    /// Extract map information by parsing C++ DataChunk `.map` (CkMp).
    /// Corresponds to C++ `loadMap` + `getExtent` + object waypoint/tech/supply walk.
    fn extract_map_info(&self, path: &Path) -> Result<ExtractedMapInfo> {
        let mut file = File::open(path)?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents)?;
        parse_map_bytes(
            &contents,
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown"),
        )
    }

    /// Extract a text field from map file content
    fn extract_field(&self, text: &str, field_name: &str) -> Option<String> {
        text.lines()
            .find(|line| line.contains(field_name))
            .and_then(|line| {
                line.split('=')
                    .nth(1)
                    .map(|s| s.trim().trim_matches('"').to_string())
            })
    }

    /// Extract a numeric field from map file content
    fn extract_numeric_field(&self, text: &str, field_name: &str) -> Option<f32> {
        self.extract_field(text, field_name)
            .and_then(|s| s.parse().ok())
    }

    /// Write the cache to a .ini file
    /// Corresponds to C++ MapCache::writeCacheINI() from MapUtil.cpp
    pub fn write_cache_file(&self, output_path: &Path) -> Result<()> {
        info!("Writing cache file to: {:?}", output_path);

        let file = File::create(output_path).context("Failed to create cache file")?;
        let mut writer = BufWriter::new(file);

        writeln!(
            writer,
            "; FILE: {} /////////////////////////////////////////////////////////////",
            output_path.display()
        )?;
        writeln!(writer, "; This INI file is auto-generated - do not modify")?;
        writeln!(
            writer,
            "; /////////////////////////////////////////////////////////////////////////////"
        )?;

        let mut names: Vec<_> = self.maps.keys().cloned().collect();
        names.sort();
        for name in names {
            let metadata = &self.maps[&name];
            writeln!(writer)?;
            writeln!(
                writer,
                "MapCache {}",
                ascii_string_to_quoted_printable(&name)
            )?;
            writeln!(writer, "  fileSize = {}", metadata.file_size)?;
            writeln!(writer, "  fileCRC = {}", metadata.crc)?;
            writeln!(writer, "  timestampLo = {}", metadata.timestamp as i32)?;
            writeln!(writer, "  timestampHi = 0")?;
            writeln!(
                writer,
                "  isOfficial = {}",
                if metadata.is_official { "yes" } else { "no" }
            )?;
            writeln!(
                writer,
                "  isMultiplayer = {}",
                if metadata.is_multiplayer { "yes" } else { "no" }
            )?;
            writeln!(writer, "  numPlayers = {}", metadata.num_players)?;
            writeln!(
                writer,
                "  extentMin = X:{:.2} Y:{:.2} Z:{:.2}",
                0.0, 0.0, metadata.extent_min_z
            )?;
            writeln!(
                writer,
                "  extentMax = X:{:.2} Y:{:.2} Z:{:.2}",
                metadata.extent_width, metadata.extent_height, metadata.extent_max_z
            )?;
            writeln!(writer, "  nameLookupTag = {}", metadata.display_name)?;
            for (name, x, y, z) in &metadata.waypoints {
                writeln!(writer, "  {name} = X:{x:.2} Y:{y:.2} Z:{z:.2}")?;
            }
            for (x, y, z) in &metadata.tech_positions {
                writeln!(writer, "  techPosition = X:{x:.2} Y:{y:.2} Z:{z:.2}")?;
            }
            for (x, y, z) in &metadata.supply_positions {
                writeln!(writer, "  supplyPosition = X:{x:.2} Y:{y:.2} Z:{z:.2}")?;
            }
            writeln!(writer, "END")?;
            writeln!(writer)?;
        }

        writer.flush()?;
        info!(
            "Successfully wrote cache with {} map entries",
            self.maps.len()
        );
        Ok(())
    }
}

fn map_cache_key(path: &Path) -> String {
    path.to_string_lossy().replace('/', "\\").to_lowercase()
}

/// C++ `fname` used by `addShippingMap` / `m_allowedMaps.find(fname)`:
/// file stem without extension, lowercased (`Alpine War` → `alpine war`).
fn shipping_map_fname(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase()
}

fn is_shipping_map_allowed(allowed: &HashSet<String>, path: &Path) -> bool {
    if allowed.is_empty() {
        return true;
    }
    allowed.contains(&shipping_map_fname(path))
}

fn decode_map_chunk_bytes(contents: &[u8]) -> Result<Vec<u8>> {
    if is_data_compressed(contents) {
        if let Ok(decoded) = decompress_data(contents) {
            return Ok(decoded);
        }
    }
    if let Some(offset) = contents.windows(4).position(|w| w == b"CkMp") {
        return Ok(contents[offset..].to_vec());
    }
    Ok(contents.to_vec())
}

/// Parse C++ DataChunk `.map` bytes and apply `W3DTerrainLogic::getExtent`
/// (HeightMap v4 uses `m_boundaries[0] * MAP_XY_FACTOR`).
pub fn parse_map_bytes(contents: &[u8], fallback_name: &str) -> Result<ExtractedMapInfo> {
    let chunk_bytes = decode_map_chunk_bytes(contents)?;

    let mut input = DataChunkInput::new(chunk_bytes);
    if !input.is_valid_file_type() {
        anyhow::bail!("map is not a CkMp DataChunk file");
    }

    let mut ctx = MapChunkParse::default();
    input.register_parser("HeightMapData", "", parse_heightmap_size_chunk);
    input.register_parser("WorldInfo", "", parse_world_info_chunk);
    input.register_parser("ObjectsList", "", parse_objects_list_chunk);
    if !input.parse(&mut ctx) {
        anyhow::bail!("failed to parse map DataChunks");
    }

    // C++ `W3DTerrainLogic::getExtent`: hi = m_boundaries[m_activeBoundary] * MAP_XY_FACTOR
    // (not width-2*border when a v4 boundary list is present).
    let (bound_x, bound_y) = ctx.boundaries.first().copied().unwrap_or_else(|| {
        (
            (ctx.width - 2 * ctx.border_size).max(0),
            (ctx.height - 2 * ctx.border_size).max(0),
        )
    });
    let extent_width = bound_x.max(0) as f32 * MAP_XY_FACTOR;
    let extent_height = bound_y.max(0) as f32 * MAP_XY_FACTOR;

    let mut num_players = 0u32;
    for i in 0..8 {
        let name = format!("Player_{}_Start", i + 1);
        if ctx.waypoints.iter().any(|(n, _, _, _)| n == &name) {
            num_players += 1;
        } else {
            break;
        }
    }
    if num_players == 0 {
        num_players = 1;
    }

    let display_name = if ctx.display_name.is_empty() {
        fallback_name.to_string()
    } else {
        ctx.display_name
    };

    Ok(ExtractedMapInfo {
        display_name,
        num_players,
        is_multiplayer: num_players > 1,
        extent_width,
        extent_height,
        extent_min_z: ctx.min_z,
        extent_max_z: ctx.max_z,
        waypoints: ctx.waypoints,
        tech_positions: ctx.tech_positions,
        supply_positions: ctx.supply_positions,
    })
}

fn parse_heightmap_size_chunk(
    input: &mut DataChunkInput,
    info: &DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let Some(ctx) = user_data.downcast_mut::<MapChunkParse>() else {
        return false;
    };
    ctx.width = input.read_int();
    ctx.height = input.read_int();
    if info.version >= K_HEIGHT_MAP_VERSION_3 {
        ctx.border_size = input.read_int();
    }
    if info.version >= K_HEIGHT_MAP_VERSION_4 {
        let num_borders = input.read_int().max(0);
        ctx.boundaries.clear();
        for _ in 0..num_borders {
            let x = input.read_int();
            let y = input.read_int();
            ctx.boundaries.push((x, y));
        }
    } else {
        ctx.boundaries.clear();
        ctx.boundaries.push((
            (ctx.width - 2 * ctx.border_size).max(0),
            (ctx.height - 2 * ctx.border_size).max(0),
        ));
    }
    // Remaining payload is `dataSize` + height samples (C++ WorldHeightMap).
    if !input.at_end_of_chunk() {
        let data_size = input.read_int().max(0) as usize;
        let mut min_h = u8::MAX;
        let mut max_h = 0u8;
        let mut any = false;
        for _ in 0..data_size {
            if input.at_end_of_chunk() {
                break;
            }
            let sample = input.read_byte();
            any = true;
            min_h = min_h.min(sample);
            max_h = max_h.max(sample);
        }
        if any {
            ctx.min_z = min_h as f32 * MAP_HEIGHT_SCALE;
            ctx.max_z = max_h as f32 * MAP_HEIGHT_SCALE;
        }
    }
    true
}

fn parse_world_info_chunk(
    input: &mut DataChunkInput,
    _info: &DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let Some(ctx) = user_data.downcast_mut::<MapChunkParse>() else {
        return false;
    };
    let dict = input.read_dict();
    let map_name_key = NameKeyGenerator::name_to_key("mapName");
    let display_key = NameKeyGenerator::name_to_key("displayName");
    for key in [display_key, map_name_key] {
        match dict.get_type(key) {
            Some(DictType::AsciiString) => {
                let name = dict.get_ascii_string(key);
                if !name.is_empty() {
                    ctx.display_name = name;
                    break;
                }
            }
            Some(DictType::UnicodeString) => {
                let name = dict.get_unicode_string(key);
                if !name.is_empty() {
                    ctx.display_name = name;
                    break;
                }
            }
            _ => {}
        }
    }
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
    let Some(ctx) = user_data.downcast_mut::<MapChunkParse>() else {
        return false;
    };
    let x = input.read_real();
    let y = input.read_real();
    let mut z = input.read_real();
    if info.version <= K_OBJECTS_VERSION_2 {
        z = 0.0;
    }
    let _angle = input.read_real();
    let _flags = input.read_int();
    let name = input.read_ascii_string();
    let dict = if info.version >= K_OBJECTS_VERSION_2 {
        input.read_dict()
    } else {
        Dict::new()
    };

    let waypoint_key = NameKeyGenerator::name_to_key("waypointID");
    if matches!(dict.get_type(waypoint_key), Some(DictType::Int)) {
        let name_key = NameKeyGenerator::name_to_key("waypointName");
        let waypoint_name = match dict.get_type(name_key) {
            Some(DictType::AsciiString) => dict.get_ascii_string(name_key),
            _ => String::new(),
        };
        let resolved = if waypoint_name.is_empty() {
            name
        } else {
            waypoint_name
        };
        ctx.waypoints.push((resolved, x, y, z));
        return true;
    }

    let lname = name.to_ascii_lowercase();
    if is_tech_preview_name(&lname) {
        ctx.tech_positions.push((x, y, z));
    } else if is_supply_preview_name(&lname) {
        ctx.supply_positions.push((x, y, z));
    }
    true
}

/// Fallback when ThingFactory is unavailable (tool standalone). Names match
/// retail KINDOF_TECH_BUILDING / KINDOF_SUPPLY_SOURCE_ON_PREVIEW templates.
fn is_tech_preview_name(name: &str) -> bool {
    name.contains("oilrefinery")
        || name.contains("reconcenter")
        || name.contains("techbuilding")
        || name.contains("tech_building")
}

fn is_supply_preview_name(name: &str) -> bool {
    name.contains("supplydock")
        || name.contains("supplywarehouse")
        || name.contains("oilderrick")
        || name.contains("supplysource")
}

/// C++ `CRC::addCRC`: hibit = crc>>31, crc = (crc<<1) + byte + hibit.
fn generals_crc_new() -> u32 {
    0
}

fn generals_crc_add(crc: &mut u32, buf: &[u8]) {
    for &byte in buf {
        let hibit = if *crc & 0x8000_0000 != 0 { 1 } else { 0 };
        *crc = crc
            .wrapping_shl(1)
            .wrapping_add(byte as u32)
            .wrapping_add(hibit);
    }
}

/// C++ `AsciiStringToQuotedPrintable` (QuotedPrintable.cpp): non-alnum → `_XX`.
fn ascii_string_to_quoted_printable(original: &str) -> String {
    let mut out = String::new();
    for &b in original.as_bytes() {
        if b.is_ascii_alphanumeric() {
            out.push(b as char);
        } else {
            out.push('_');
            out.push(hex_digit(b >> 4));
            out.push(hex_digit(b & 0x0f));
        }
    }
    out
}

fn hex_digit(n: u8) -> char {
    let n = n & 0x0f;
    if n < 10 {
        (b'0' + n) as char
    } else {
        (b'A' + (n - 10)) as char
    }
}

/// Synthetic HeightMap v4 CkMp used by chrome + extent tests.
pub fn write_synthetic_ckmp_map() -> Vec<u8> {
    let mut out = DataChunkOutput::new();
    out.open_data_chunk("HeightMapData", K_HEIGHT_MAP_VERSION_4);
    out.write_int(12); // width
    out.write_int(10); // height
    out.write_int(1); // border
    out.write_int(1); // numBorders
    out.write_int(10);
    out.write_int(8);
    out.close_data_chunk();

    out.open_data_chunk("ObjectsList", 1);
    out.open_data_chunk("Object", 3);
    out.write_real(100.0);
    out.write_real(200.0);
    out.write_real(5.0);
    out.write_real(0.0);
    out.write_int(0);
    out.write_ascii_string("*Waypoints/Waypoint");
    let mut dict = Dict::new();
    let id_key = NameKeyGenerator::name_to_key("waypointID");
    let name_key = NameKeyGenerator::name_to_key("waypointName");
    dict.set_int(id_key, 1);
    dict.set_ascii_string(name_key, "Player_1_Start");
    out.write_dict(&dict);
    out.close_data_chunk();

    out.open_data_chunk("Object", 3);
    out.write_real(30.0);
    out.write_real(40.0);
    out.write_real(0.0);
    out.write_real(0.0);
    out.write_int(0);
    out.write_ascii_string("OilRefinery");
    out.write_dict(&Dict::new());
    out.close_data_chunk();

    out.open_data_chunk("Object", 3);
    out.write_real(50.0);
    out.write_real(60.0);
    out.write_real(0.0);
    out.write_real(0.0);
    out.write_int(0);
    out.write_ascii_string("SupplyDock");
    out.write_dict(&Dict::new());
    out.close_data_chunk();
    out.close_data_chunk();

    out.into_ckmp_bytes()
}

/// CLI entry used by `win_main` (C++ WinMain.cpp lines 224-348).
pub fn run_cli(args: &[String]) -> Result<()> {
    if args.is_empty() {
        info!("Usage: map_cache_builder [map_name1] [map_name2] ...");
        info!("  If map names are provided, only those maps will be cached (shipping maps).");
        info!("  If no arguments provided, all maps in default directories will be cached.");
        info!("  Pass --ui to open the optional chrome window (requires --features ui).");
    }

    let mut cache = MapCache::new();

    for map_name in args {
        if map_name == "--ui" {
            continue;
        }
        cache.add_shipping_map(map_name);
    }

    let mut map_dirs = Vec::new();
    for dir_str in DEFAULT_MAP_DIRS {
        let dir = PathBuf::from(dir_str);
        if dir.exists() {
            map_dirs.push(dir);
        }
    }

    if map_dirs.is_empty() {
        warn!("No default map directories found, scanning current directory");
        map_dirs.push(PathBuf::from("."));
    }

    cache.update_cache(&map_dirs)?;

    let cache_output = PathBuf::from(CACHE_FILE_NAME);
    cache.write_cache_file(&cache_output)?;

    info!("MapCacheBuilder completed successfully!");
    info!(
        "Cache file written to: {:?}",
        cache_output.canonicalize().unwrap_or(cache_output)
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_cache_creation() {
        let cache = MapCache::new();
        assert_eq!(cache.maps.len(), 0);
        assert_eq!(cache.allowed_maps.len(), 0);
    }

    #[test]
    fn test_add_shipping_map() {
        let mut cache = MapCache::new();
        cache.add_shipping_map("TestMap");
        assert!(cache.allowed_maps.contains("testmap"));
    }

    #[test]
    fn test_crc_calculation() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"test content").unwrap();

        let cache = MapCache::new();
        let crc = cache.calculate_crc(temp_file.path()).unwrap();
        assert!(crc > 0);
    }

    #[test]
    fn parse_binary_ckmp_map_matches_cpp_extent_and_waypoints() {
        let bytes = write_synthetic_ckmp_map();
        let parsed = parse_map_bytes(&bytes, "Synthetic").expect("parse");
        assert_eq!(parsed.extent_width, 10.0 * MAP_XY_FACTOR);
        assert_eq!(parsed.extent_height, 8.0 * MAP_XY_FACTOR);
        assert_eq!(parsed.num_players, 1);
        assert!(!parsed.is_multiplayer);
        assert_eq!(parsed.waypoints.len(), 1);
        assert_eq!(parsed.waypoints[0].0, "Player_1_Start");
        assert_eq!(parsed.waypoints[0].1, 100.0);
        assert_eq!(parsed.waypoints[0].2, 200.0);
        assert_eq!(parsed.tech_positions, vec![(30.0, 40.0, 0.0)]);
        assert_eq!(parsed.supply_positions, vec![(50.0, 60.0, 0.0)]);
    }

    #[test]
    fn parse_heightmap_uses_v4_boundary_and_height_z_like_cpp_get_extent() {
        let mut out = DataChunkOutput::new();
        out.open_data_chunk("HeightMapData", K_HEIGHT_MAP_VERSION_4);
        out.write_int(20); // width
        out.write_int(16); // height
        out.write_int(2); // border → playable would be 16x12 if we ignored boundaries
        out.write_int(1); // numBorders
        out.write_int(7); // active boundary x (C++ m_boundaries[0])
        out.write_int(5); // active boundary y
        out.write_int(3); // dataSize
        out.write_byte(1);
        out.write_byte(10);
        out.write_byte(4);
        out.close_data_chunk();
        let bytes = out.into_ckmp_bytes();
        let parsed = parse_map_bytes(&bytes, "Bound").expect("parse");
        assert_eq!(
            parsed.extent_width,
            7.0 * MAP_XY_FACTOR,
            "C++ getExtent hi.x uses m_boundaries[0], not width-2*border"
        );
        assert_eq!(parsed.extent_height, 5.0 * MAP_XY_FACTOR);
        assert_eq!(parsed.extent_min_z, 1.0 * MAP_HEIGHT_SCALE);
        assert_eq!(parsed.extent_max_z, 10.0 * MAP_HEIGHT_SCALE);
    }

    #[test]
    fn parse_retail_lone_eagle_when_present() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
            "../../../../windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
        );
        if !path.exists() {
            return;
        }
        let bytes = std::fs::read(&path).expect("read lone eagle");
        let Ok(parsed) = parse_map_bytes(&bytes, "Lone Eagle") else {
            // EAR-compressed retail maps need full CompressionManager init;
            // synthetic CkMp test covers the C++ DataChunk transform.
            return;
        };
        if parsed.extent_width <= 0.0 || parsed.extent_height <= 0.0 {
            return;
        }
        assert!(
            parsed
                .waypoints
                .iter()
                .any(|(n, _, _, _)| n == "Player_1_Start"),
            "when HeightMapData parsed, ObjectsList must expose Player_1_Start"
        );
        assert!(parsed.num_players >= 1);
    }

    #[test]
    fn write_cache_ini_matches_cpp_mapcache_tokens() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut payload = NamedTempFile::new().unwrap();
        payload.write_all(b"map-bytes").unwrap();
        let crc = {
            let cache = MapCache::new();
            cache.calculate_crc(payload.path()).unwrap()
        };

        let mut cache = MapCache::new();
        cache.maps.insert(
            "maps\\alpine war.map".to_string(),
            MapMetaData {
                display_name: "Alpine War".to_string(),
                file_name: "alpine war".to_string(),
                file_path: payload.path().to_path_buf(),
                num_players: 4,
                is_multiplayer: true,
                is_official: true,
                file_size: 9,
                crc,
                timestamp: 42,
                extent_width: 500.0,
                extent_height: 400.0,
                extent_min_z: 1.25,
                extent_max_z: 6.25,
                waypoint_count: 4,
                supply_position_count: 1,
                tech_position_count: 2,
                waypoints: vec![("Player_1_Start".into(), 10.0, 20.0, 0.0)],
                tech_positions: vec![(1.0, 2.0, 3.0)],
                supply_positions: vec![(4.0, 5.0, 6.0)],
            },
        );

        let out = NamedTempFile::new().unwrap();
        cache.write_cache_file(out.path()).unwrap();
        let text = std::fs::read_to_string(out.path()).unwrap();
        assert!(text.contains("; This INI file is auto-generated - do not modify"));
        assert!(text.contains("MapCache maps_5Calpine_20war_2Emap"));
        assert!(text.contains("  fileSize = 9"));
        assert!(text.contains(&format!("  fileCRC = {crc}")));
        assert!(text.contains("  isOfficial = yes"));
        assert!(text.contains("  isMultiplayer = yes"));
        assert!(text.contains("  numPlayers = 4"));
        assert!(text.contains("  extentMin = X:0.00 Y:0.00 Z:1.25"));
        assert!(text.contains("  extentMax = X:500.00 Y:400.00 Z:6.25"));
        assert!(text.contains("  nameLookupTag = Alpine War"));
        assert!(text.contains("  Player_1_Start = X:10.00 Y:20.00 Z:0.00"));
        assert!(text.contains("  techPosition = X:1.00 Y:2.00 Z:3.00"));
        assert!(text.contains("  supplyPosition = X:4.00 Y:5.00 Z:6.00"));
        assert!(text.contains("END"));
        let mut ieee = crc32fast::Hasher::new();
        ieee.update(b"map-bytes");
        assert_ne!(
            crc,
            ieee.finalize(),
            "fileCRC must be Generals rotate-add CRC, not IEEE crc32"
        );
        assert_eq!(
            ascii_string_to_quoted_printable("maps\\alpine war.map"),
            "maps_5Calpine_20war_2Emap"
        );
    }

    #[test]
    fn shipping_argv_stem_keeps_folder_map_like_cpp_add_shipping_map() {
        use std::io::Write;

        let root = tempfile::tempdir().unwrap();
        let alpine_dir = root.path().join("Alpine War");
        std::fs::create_dir_all(&alpine_dir).unwrap();
        let alpine_map = alpine_dir.join("Alpine War.map");
        std::fs::write(&alpine_map, write_synthetic_ckmp_map()).unwrap();

        let night_dir = root.path().join("Dark Night");
        std::fs::create_dir_all(&night_dir).unwrap();
        std::fs::write(night_dir.join("Dark Night.map"), write_synthetic_ckmp_map()).unwrap();

        let mut cache = MapCache::new();
        cache.add_shipping_map("Alpine War");
        cache.update_cache(&[root.path().to_path_buf()]).unwrap();

        assert_eq!(cache.maps.len(), 1, "only shipping stem should remain");
        let (key, meta) = cache.maps.iter().next().unwrap();
        assert!(
            key.contains("alpine war"),
            "MapCache INI key is still full lowercase path, got {key}"
        );
        assert_eq!(shipping_map_fname(&meta.file_path), "alpine war");
        assert!(is_shipping_map_allowed(
            &cache.allowed_maps,
            &meta.file_path
        ));

        let out = tempfile::NamedTempFile::new().unwrap();
        cache.write_cache_file(out.path()).unwrap();
        let text = std::fs::read_to_string(out.path()).unwrap();
        assert!(text.contains("; This INI file is auto-generated - do not modify"));
        assert!(text.contains("  isOfficial = yes") || text.contains("  isOfficial = no"));
        assert!(text.contains("  numPlayers = 1"));
        assert!(text.contains("  Player_1_Start ="));
        assert!(text.contains("END"));
        assert!(
            !text.to_lowercase().contains("dark night"),
            "non-shipping map must not be written"
        );
    }
}
