//! MapCacheBuilder tool chrome model (no egui/gpui).
//!
//! Scan/Build call the shipped HeightMap v4 parse/`getExtent` path in [`crate::cache`].

use crate::cache::{CACHE_FILE_NAME, MAP_HEIGHT_SCALE, MAP_XY_FACTOR, MapCache, parse_map_bytes};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const MAX_LOGS: usize = 200;

/// Last `W3DTerrainLogic::getExtent` sample shown in the status strip.
#[derive(Debug, Clone, PartialEq)]
pub struct MapExtentStatus {
    pub map_name: String,
    pub width: f32,
    pub height: f32,
    pub min_z: f32,
    pub max_z: f32,
}

/// Window chrome state: input folder, output ini, logs, map count, last extent.
#[derive(Debug, Clone)]
pub struct MapCacheChrome {
    pub input_maps_folder: PathBuf,
    pub output_ini_path: PathBuf,
    pub logs: Vec<String>,
    pub map_count: usize,
    pub last_extent: Option<MapExtentStatus>,
    pub last_error: Option<String>,
    pub status: String,
}

impl Default for MapCacheChrome {
    fn default() -> Self {
        Self::new()
    }
}

impl MapCacheChrome {
    pub fn new() -> Self {
        Self {
            input_maps_folder: PathBuf::from("Maps"),
            output_ini_path: PathBuf::from(CACHE_FILE_NAME),
            logs: vec!["MapCacheBuilder chrome initialized".to_string()],
            map_count: 0,
            last_extent: None,
            last_error: None,
            status: "Select a maps folder and click Scan or Build.".to_string(),
        }
    }

    fn push_log(&mut self, message: impl Into<String>) {
        self.logs.push(message.into());
        if self.logs.len() > MAX_LOGS {
            let overflow = self.logs.len() - MAX_LOGS;
            self.logs.drain(0..overflow);
        }
    }

    fn set_error(&mut self, err: impl Into<String>) {
        let message = err.into();
        self.last_error = Some(message.clone());
        self.status = format!("Error: {message}");
        self.push_log(self.status.clone());
    }

    pub fn set_input_maps_folder(&mut self, path: PathBuf) {
        self.input_maps_folder = path;
        self.push_log(format!(
            "Input maps folder: {}",
            self.input_maps_folder.display()
        ));
    }

    pub fn set_output_ini_path(&mut self, path: PathBuf) {
        self.output_ini_path = path;
        self.push_log(format!("Output INI: {}", self.output_ini_path.display()));
    }

    fn collect_map_paths(&self) -> Vec<PathBuf> {
        let mut maps = Vec::new();
        if !self.input_maps_folder.exists() {
            return maps;
        }
        for entry in WalkDir::new(&self.input_maps_folder)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("map") {
                maps.push(path.to_path_buf());
            }
        }
        maps.sort();
        maps
    }

    /// Scan maps folder; updates map count without writing INI.
    pub fn scan(&mut self) -> Result<usize> {
        self.last_error = None;
        if !self.input_maps_folder.exists() {
            let msg = format!(
                "maps folder does not exist: {}",
                self.input_maps_folder.display()
            );
            self.set_error(&msg);
            anyhow::bail!("{msg}");
        }
        let maps = self.collect_map_paths();
        self.map_count = maps.len();
        self.push_log(format!(
            "Scan found {} map(s) in {}",
            self.map_count,
            self.input_maps_folder.display()
        ));
        for path in maps.iter().take(40) {
            self.push_log(format!("  {}", path.display()));
        }
        if maps.len() > 40 {
            self.push_log(format!("  … {} more", maps.len() - 40));
        }
        self.status = format!("Scanned {} map(s)", self.map_count);
        Ok(self.map_count)
    }

    /// Build: scan + parse via [`parse_map_bytes`] / HeightMap v4 getExtent, write mapcache.ini.
    pub fn build(&mut self) -> Result<()> {
        self.last_error = None;
        self.last_extent = None;
        if !self.input_maps_folder.exists() {
            let msg = format!(
                "maps folder does not exist: {}",
                self.input_maps_folder.display()
            );
            self.set_error(&msg);
            anyhow::bail!("{msg}");
        }

        let mut cache = MapCache::new();
        cache
            .update_cache(&[self.input_maps_folder.clone()])
            .with_context(|| {
                format!(
                    "failed updating map cache from {}",
                    self.input_maps_folder.display()
                )
            })?;

        self.map_count = cache.maps.len();
        self.record_last_extent_from_cache(&cache);

        if let Some(parent) = self.output_ini_path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed creating '{}'", parent.display()))?;
            }
        }
        cache
            .write_cache_file(&self.output_ini_path)
            .with_context(|| {
                format!(
                    "failed writing mapcache.ini '{}'",
                    self.output_ini_path.display()
                )
            })?;

        let extent_note = self
            .last_extent
            .as_ref()
            .map(|e| {
                format!(
                    " last extent {}x{} z=[{:.2},{:.2}] ({})",
                    e.width, e.height, e.min_z, e.max_z, e.map_name
                )
            })
            .unwrap_or_default();
        self.status = format!(
            "Built {} map(s) → {}{extent_note}",
            self.map_count,
            self.output_ini_path.display()
        );
        self.push_log(self.status.clone());
        Ok(())
    }

    fn record_last_extent_from_cache(&mut self, cache: &MapCache) {
        let mut names: Vec<_> = cache.maps.keys().cloned().collect();
        names.sort();
        let Some(name) = names.last() else {
            return;
        };
        let meta = &cache.maps[name];
        // Re-run shipped parse/getExtent so chrome last_extent is not a stub copy.
        if let Ok(bytes) = fs::read(&meta.file_path) {
            if let Ok(parsed) = parse_map_bytes(&bytes, &meta.file_name) {
                self.last_extent = Some(MapExtentStatus {
                    map_name: meta.file_name.clone(),
                    width: parsed.extent_width,
                    height: parsed.extent_height,
                    min_z: parsed.extent_min_z,
                    max_z: parsed.extent_max_z,
                });
                self.push_log(format!(
                    "getExtent {}: {:.1}x{:.1} z=[{:.2},{:.2}] (MAP_XY_FACTOR={})",
                    meta.file_name,
                    parsed.extent_width,
                    parsed.extent_height,
                    parsed.extent_min_z,
                    parsed.extent_max_z,
                    MAP_XY_FACTOR
                ));
                return;
            }
        }
        self.last_extent = Some(MapExtentStatus {
            map_name: meta.file_name.clone(),
            width: meta.extent_width,
            height: meta.extent_height,
            min_z: meta.extent_min_z,
            max_z: meta.extent_max_z,
        });
    }

    /// Parse a single `.map` through the shipped HeightMap v4 getExtent path.
    pub fn parse_map_extent(path: &Path) -> Result<MapExtentStatus> {
        let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
        let fallback = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown");
        let parsed = parse_map_bytes(&bytes, fallback)?;
        Ok(MapExtentStatus {
            map_name: fallback.to_string(),
            width: parsed.extent_width,
            height: parsed.extent_height,
            min_z: parsed.extent_min_z,
            max_z: parsed.extent_max_z,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::write_synthetic_ckmp_map;

    #[test]
    fn chrome_build_synthetic_v4_updates_map_count_and_last_extent() {
        let root = tempfile::tempdir().unwrap();
        let map_dir = root.path().join("Alpine War");
        fs::create_dir_all(&map_dir).unwrap();
        let map_path = map_dir.join("Alpine War.map");
        fs::write(&map_path, write_synthetic_ckmp_map()).unwrap();

        let mut chrome = MapCacheChrome::new();
        chrome.set_input_maps_folder(root.path().to_path_buf());
        chrome.set_output_ini_path(root.path().join("mapcache.ini"));

        let scanned = chrome.scan().unwrap();
        assert_eq!(scanned, 1);
        assert_eq!(chrome.map_count, 1);
        assert!(chrome.last_error.is_none());

        chrome.build().expect("build");
        assert_eq!(chrome.map_count, 1);
        assert!(chrome.last_error.is_none());
        let extent = chrome.last_extent.expect("last extent");
        assert_eq!(extent.width, 10.0 * MAP_XY_FACTOR);
        assert_eq!(extent.height, 8.0 * MAP_XY_FACTOR);
        assert_eq!(extent.min_z, 0.0);
        assert_eq!(extent.max_z, 0.0);
        // Height samples omitted in synthetic writer → z stays 0; width/height from v4 boundary.
        assert_eq!(extent.map_name.to_lowercase(), "alpine war");
        assert!(chrome.output_ini_path.exists());
        let text = fs::read_to_string(&chrome.output_ini_path).unwrap();
        assert!(text.contains("; This INI file is auto-generated - do not modify"));
        assert!(text.contains("  extentMax = X:100.00 Y:80.00 Z:0.00"));
        assert!(text.contains("  Player_1_Start ="));
        assert!(text.contains("END"));
        assert!(chrome.status.contains("Built 1 map"));
        let _ = MAP_HEIGHT_SCALE;
    }

    #[test]
    fn chrome_scan_missing_folder_sets_last_error() {
        let mut chrome = MapCacheChrome::new();
        chrome.set_input_maps_folder(PathBuf::from("/this/path/does/not/exist-mapcache-chrome"));
        let err = chrome.scan().unwrap_err();
        assert!(err.to_string().contains("does not exist"));
        assert!(chrome.last_error.is_some());
        assert_eq!(chrome.map_count, 0);
    }

    #[test]
    fn chrome_parse_map_extent_uses_heightmap_v4_boundary_not_width_minus_border() {
        let bytes = {
            use game_engine::common::system::DataChunkOutput;
            let mut out = DataChunkOutput::new();
            out.open_data_chunk("HeightMapData", 4);
            out.write_int(20);
            out.write_int(16);
            out.write_int(2);
            out.write_int(1);
            out.write_int(7);
            out.write_int(5);
            out.write_int(3);
            out.write_byte(1);
            out.write_byte(10);
            out.write_byte(4);
            out.close_data_chunk();
            out.into_ckmp_bytes()
        };
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Bound.map");
        fs::write(&path, bytes).unwrap();
        let extent = MapCacheChrome::parse_map_extent(&path).unwrap();
        assert_eq!(extent.width, 7.0 * MAP_XY_FACTOR);
        assert_eq!(extent.height, 5.0 * MAP_XY_FACTOR);
        assert_eq!(extent.min_z, 1.0 * MAP_HEIGHT_SCALE);
        assert_eq!(extent.max_z, 10.0 * MAP_HEIGHT_SCALE);
    }
}
