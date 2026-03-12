//! MapCacheBuilder Main Entry Point
//!
//! Corresponds to C++ file: Tools/MapCacheBuilder/Source/WinMain.cpp
//!
//! This tool pre-computes map metadata and caches it for faster loading in the game.
//! It scans map files (.map), extracts metadata (dimensions, player count, waypoints, etc.),
//! and writes the data to a cache file (mapcache.ini) to avoid re-parsing at runtime.

use anyhow::{Context, Result};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Cache file name
const CACHE_FILE_NAME: &str = "mapcache.ini";

/// Default map directories to scan
const DEFAULT_MAP_DIRS: &[&str] = &["Data/Maps", "Maps"];

/// Map metadata extracted from .map files
/// Matches C++ MapMetaData structure from MapUtil.h
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MapMetaData {
    display_name: String,
    file_name: String,
    file_path: PathBuf,
    num_players: u32,
    is_multiplayer: bool,
    is_official: bool,
    file_size: u64,
    crc: u32,
    timestamp: u64,
    extent_width: f32,
    extent_height: f32,
    waypoint_count: u32,
    supply_position_count: u32,
    tech_position_count: u32,
}

/// Map cache structure that holds all map metadata
/// Matches C++ MapCache class from MapUtil.h
#[derive(Debug, Default)]
struct MapCache {
    maps: HashMap<String, MapMetaData>,
    allowed_maps: HashSet<String>,
}

impl MapCache {
    fn new() -> Self {
        Self {
            maps: HashMap::new(),
            allowed_maps: HashSet::new(),
        }
    }

    /// Add a shipping map to the allowed list
    /// Corresponds to C++ MapCache::addShippingMap() from MapUtil.h line 84
    fn add_shipping_map(&mut self, map_name: &str) {
        let lowercase_name = map_name.to_lowercase();
        info!("Adding shipping map: '{}'", lowercase_name);
        self.allowed_maps.insert(lowercase_name);
    }

    /// Scan directories and update the cache
    /// Corresponds to C++ MapCache::updateCache() from MapUtil.h line 75
    fn update_cache(&mut self, map_dirs: &[PathBuf]) -> Result<()> {
        info!("Starting map cache update...");

        for map_dir in map_dirs {
            if !map_dir.exists() {
                warn!("Map directory does not exist: {:?}", map_dir);
                continue;
            }

            info!("Scanning directory: {:?}", map_dir);
            self.scan_directory(map_dir)?;
        }

        // Filter to only allowed maps if list is specified
        if !self.allowed_maps.is_empty() {
            let original_count = self.maps.len();
            self.maps
                .retain(|name, _| self.allowed_maps.contains(&name.to_lowercase()));
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
                match self.parse_map_file(path) {
                    Ok(metadata) => {
                        let map_name = metadata.file_name.to_lowercase();
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

        // Parse map file for detailed metadata
        let (
            display_name,
            num_players,
            is_multiplayer,
            extent_width,
            extent_height,
            waypoint_count,
            supply_count,
            tech_count,
        ) = self.extract_map_info(path)?;

        Ok(MapMetaData {
            display_name,
            file_name: file_name.clone(),
            file_path: path.to_path_buf(),
            num_players,
            is_multiplayer,
            is_official: true, // All maps added via this tool are considered official
            file_size,
            crc,
            timestamp,
            extent_width,
            extent_height,
            waypoint_count,
            supply_position_count: supply_count,
            tech_position_count: tech_count,
        })
    }

    /// Calculate CRC32 checksum of a file
    /// Matches C++ calcCRC() from MapUtil.cpp lines 65-103
    fn calculate_crc(&self, path: &Path) -> Result<u32> {
        let mut file = File::open(path)?;
        let mut buffer = vec![0u8; 4096];
        let mut hasher = crc32fast::Hasher::new();

        loop {
            let bytes_read = file.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        Ok(hasher.finalize())
    }

    /// Extract map information by parsing the .map file
    /// Simplified parser that extracts key metadata
    fn extract_map_info(
        &self,
        path: &Path,
    ) -> Result<(String, u32, bool, f32, f32, u32, u32, u32)> {
        let mut file = File::open(path)?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents)?;

        // Parse as text to find key fields
        let text = String::from_utf8_lossy(&contents);

        // Extract display name (look for displayName or use filename)
        let display_name = self.extract_field(&text, "displayName").unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown")
                .to_string()
        });

        // Count waypoints (player start positions)
        let waypoint_count = text.matches("waypointID").count() as u32;

        // Infer player count from start positions
        let player_waypoints = text.matches("Player_").count() as u32;
        let num_players = if player_waypoints > 0 {
            player_waypoints
        } else {
            waypoint_count.min(8) // Default guess
        };

        // Check if multiplayer (more than 1 player)
        let is_multiplayer = num_players > 1;

        // Extract map extent (dimensions)
        let extent_width = self.extract_numeric_field(&text, "width").unwrap_or(512.0);
        let extent_height = self.extract_numeric_field(&text, "height").unwrap_or(512.0);

        // Count supply and tech buildings
        let supply_count = text.matches("KINDOF_SUPPLY_SOURCE").count() as u32;
        let tech_count = text.matches("KINDOF_TECH_BUILDING").count() as u32;

        Ok((
            display_name,
            num_players,
            is_multiplayer,
            extent_width,
            extent_height,
            waypoint_count,
            supply_count,
            tech_count,
        ))
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
    fn write_cache_file(&self, output_path: &Path) -> Result<()> {
        info!("Writing cache file to: {:?}", output_path);

        let file = File::create(output_path).context("Failed to create cache file")?;
        let mut writer = BufWriter::new(file);

        writeln!(writer, "; Map Cache File")?;
        writeln!(writer, "; Generated by MapCacheBuilder")?;
        writeln!(writer, "; Total Maps: {}", self.maps.len())?;
        writeln!(writer)?;

        for (map_name, metadata) in &self.maps {
            writeln!(writer, "[{}]", map_name)?;
            writeln!(writer, "DisplayName = \"{}\"", metadata.display_name)?;
            writeln!(writer, "FileName = \"{}\"", metadata.file_name)?;
            writeln!(writer, "NumPlayers = {}", metadata.num_players)?;
            writeln!(
                writer,
                "IsMultiplayer = {}",
                if metadata.is_multiplayer {
                    "true"
                } else {
                    "false"
                }
            )?;
            writeln!(
                writer,
                "IsOfficial = {}",
                if metadata.is_official {
                    "true"
                } else {
                    "false"
                }
            )?;
            writeln!(writer, "FileSize = {}", metadata.file_size)?;
            writeln!(writer, "CRC = 0x{:08X}", metadata.crc)?;
            writeln!(writer, "Timestamp = {}", metadata.timestamp)?;
            writeln!(writer, "ExtentWidth = {}", metadata.extent_width)?;
            writeln!(writer, "ExtentHeight = {}", metadata.extent_height)?;
            writeln!(writer, "WaypointCount = {}", metadata.waypoint_count)?;
            writeln!(
                writer,
                "SupplyPositions = {}",
                metadata.supply_position_count
            )?;
            writeln!(writer, "TechPositions = {}", metadata.tech_position_count)?;
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

/// Main entry point for the MapCacheBuilder tool
/// Corresponds to C++ WinMain() from WinMain.cpp lines 224-348
fn main() -> Result<()> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("MapCacheBuilder starting...");
    info!("Command & Conquer Generals Zero Hour - Map Cache Builder");

    // Parse command line arguments
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() {
        info!("Usage: map_cache_builder [map_name1] [map_name2] ...");
        info!("  If map names are provided, only those maps will be cached (shipping maps).");
        info!("  If no arguments provided, all maps in default directories will be cached.");
    }

    // Create map cache
    let mut cache = MapCache::new();

    // Add shipping maps from command line arguments
    // Matches C++ WinMain.cpp lines 308-312
    for map_name in &args {
        cache.add_shipping_map(map_name);
    }

    // Determine map directories to scan
    let mut map_dirs = Vec::new();
    for dir_str in DEFAULT_MAP_DIRS {
        let dir = PathBuf::from(dir_str);
        if dir.exists() {
            map_dirs.push(dir);
        }
    }

    // Add current directory if no standard dirs found
    if map_dirs.is_empty() {
        warn!("No default map directories found, scanning current directory");
        map_dirs.push(PathBuf::from("."));
    }

    // Update the cache (scan and parse map files)
    // Matches C++ WinMain.cpp line 314
    cache.update_cache(&map_dirs)?;

    // Write the cache file
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
}
