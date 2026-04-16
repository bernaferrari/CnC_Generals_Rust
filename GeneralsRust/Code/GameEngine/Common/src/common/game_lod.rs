// game_lod.rs - Game Level of Detail system
// Loads GameLOD.ini and exposes dynamic LOD parameters used by gameplay systems.

use std::collections::HashMap;
use std::fs;
use std::sync::{OnceLock, RwLock};

use crate::common::ini::ini_game_data::get_global_data;
use crate::common::ini::ini_game_lod::{get_game_lod_manager, StaticGameLODLevel};

/// LOD levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LodLevel {
    High = 0,
    Medium = 1,
    Low = 2,
}

/// LOD manager
pub struct GameLod {
    current_level: LodLevel,
}

impl Default for GameLod {
    fn default() -> Self {
        Self::new()
    }
}

impl GameLod {
    pub fn new() -> Self {
        Self {
            current_level: LodLevel::High,
        }
    }

    pub fn set_level(&mut self, level: LodLevel) {
        self.current_level = level;
    }

    pub fn get_level(&self) -> LodLevel {
        self.current_level
    }
}

static DYNAMIC_LOD_NAME: OnceLock<RwLock<String>> = OnceLock::new();
static DYNAMIC_LOD_SLOW_DEATH: OnceLock<RwLock<HashMap<String, f32>>> = OnceLock::new();
static STATIC_LOD_NAME: OnceLock<RwLock<String>> = OnceLock::new();
static IDEAL_STATIC_LOD_NAME: OnceLock<RwLock<String>> = OnceLock::new();

fn dynamic_lod_name() -> &'static RwLock<String> {
    DYNAMIC_LOD_NAME.get_or_init(|| RwLock::new("High".to_string()))
}

fn dynamic_lod_slow_death() -> &'static RwLock<HashMap<String, f32>> {
    DYNAMIC_LOD_SLOW_DEATH.get_or_init(|| RwLock::new(HashMap::new()))
}

fn static_lod_name() -> &'static RwLock<String> {
    STATIC_LOD_NAME.get_or_init(|| RwLock::new("Medium".to_string()))
}

fn ideal_static_lod_name() -> &'static RwLock<String> {
    IDEAL_STATIC_LOD_NAME.get_or_init(|| RwLock::new("Unknown".to_string()))
}

fn canonical_static_lod_name(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "low" => Some("Low"),
        "medium" => Some("Medium"),
        "high" => Some("High"),
        "custom" => Some("Custom"),
        "unknown" => Some("Unknown"),
        _ => None,
    }
}

pub fn set_dynamic_lod(name: &str) {
    if let Ok(mut guard) = dynamic_lod_name().write() {
        *guard = name.to_string();
    }
}

pub fn set_dynamic_lod_from_string(value: &str) {
    let normalized = value.trim().to_ascii_lowercase();
    let mapped = match normalized.as_str() {
        "veryhigh" | "very_high" | "very high" => "VeryHigh",
        "high" => "High",
        "medium" => "Medium",
        "low" => "Low",
        _ => value.trim(),
    };
    if !mapped.is_empty() {
        set_dynamic_lod(mapped);
    }
}

pub fn get_dynamic_lod() -> String {
    dynamic_lod_name()
        .read()
        .map(|guard| guard.clone())
        .unwrap_or_else(|_| "High".to_string())
}

pub fn set_static_lod_from_string(value: &str) {
    let Some(mapped) = canonical_static_lod_name(value) else {
        return;
    };
    if let Ok(mut guard) = static_lod_name().write() {
        *guard = mapped.to_string();
    }
    apply_static_lod_level(mapped);
}

/// Apply StaticGameLOD settings to GlobalData.
/// Matches C++ GameLODManager::applyStaticLODLevel().
fn apply_static_lod_level(level_name: &str) {
    let level = match StaticGameLODLevel::from_str(level_name) {
        Some(l) => l,
        None => return,
    };
    let index = match level.to_index() {
        Some(i) => i,
        None => return,
    };

    let lod_info = {
        let manager = get_game_lod_manager();
        manager.static_game_lod_info[index].clone()
    };

    let Some(global_data) = get_global_data() else {
        return;
    };
    let mut global = global_data.write();

    global.max_particle_count = lod_info.max_particle_count;
    global.use_shadow_volumes = lod_info.use_shadow_volumes;
    global.use_shadow_decals = lod_info.use_shadow_decals;
    global.use_cloud_map = lod_info.use_cloud_map;
    global.use_light_map = lod_info.use_light_map;
    global.show_soft_water_edge = lod_info.show_soft_water_edge;
    global.max_tank_track_edges = lod_info.max_tank_track_edges;
    global.max_tank_track_opaque_edges = lod_info.max_tank_track_opaque_edges;
    global.max_tank_track_fade_delay = lod_info.max_tank_track_fade_delay;
    global.use_tree_sway = lod_info.use_tree_sway;
    global.use_draw_module_lod = !lod_info.use_buildup_scaffolds;
    global.use_heat_effects = lod_info.use_heat_effects;
    global.enable_dynamic_lod = lod_info.enable_dynamic_lod;
    global.use_fps_limit = lod_info.use_fps_limit;
    global.use_trees = lod_info.use_trees;
}

pub fn get_static_lod() -> String {
    static_lod_name()
        .read()
        .map(|guard| guard.clone())
        .unwrap_or_else(|_| "Medium".to_string())
}

pub fn set_ideal_static_lod_from_string(value: &str) {
    let Some(mapped) = canonical_static_lod_name(value) else {
        return;
    };
    if let Ok(mut guard) = ideal_static_lod_name().write() {
        *guard = mapped.to_string();
    }
}

pub fn get_ideal_static_lod() -> String {
    ideal_static_lod_name()
        .read()
        .map(|guard| guard.clone())
        .unwrap_or_else(|_| "Unknown".to_string())
}

pub fn prefers_low_res_movies() -> bool {
    matches!(get_static_lod().as_str(), "Low") || matches!(get_ideal_static_lod().as_str(), "Low")
}

fn ensure_game_lod_loaded() {
    let mut map_guard = match dynamic_lod_slow_death().write() {
        Ok(guard) => guard,
        Err(_) => return,
    };
    if !map_guard.is_empty() {
        return;
    }

    let mut files = Vec::new();
    let default_path = "Data/INI/Default/GameLOD.ini";
    let override_path = "Data/INI/GameLOD.ini";
    if std::path::Path::new(default_path).exists() {
        files.push(default_path.to_string());
    }
    if std::path::Path::new(override_path).exists() {
        files.push(override_path.to_string());
    }

    for path in files {
        if let Ok(contents) = fs::read_to_string(&path) {
            parse_game_lod_ini(&contents, &mut map_guard);
        }
    }
}

fn parse_game_lod_ini(contents: &str, map: &mut HashMap<String, f32>) {
    let mut current_dynamic: Option<String> = None;

    for raw_line in contents.lines() {
        let line = raw_line.split(';').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with("DynamicGameLOD") {
            let parts: Vec<_> = line.split('=').collect();
            if parts.len() >= 2 {
                current_dynamic = Some(parts[1].trim().to_string());
            }
            continue;
        }

        if line.eq_ignore_ascii_case("End") {
            current_dynamic = None;
            continue;
        }

        if let Some(name) = current_dynamic.as_ref() {
            if let Some((key, value)) = line.split_once('=') {
                if key.trim().eq_ignore_ascii_case("SlowDeathScale") {
                    if let Ok(scale) = value.trim().parse::<f32>() {
                        map.insert(name.clone(), scale);
                    }
                }
            }
        }
    }
}

pub fn get_slow_death_scale() -> f32 {
    ensure_game_lod_loaded();
    let name = get_dynamic_lod();
    dynamic_lod_slow_death()
        .read()
        .ok()
        .and_then(|guard| guard.get(&name).copied())
        .unwrap_or(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_lod_parser_tracks_current_and_ideal_low_detail() {
        set_static_lod_from_string("Medium");
        set_ideal_static_lod_from_string("Unknown");
        assert!(!prefers_low_res_movies());

        set_static_lod_from_string("Low");
        assert!(prefers_low_res_movies());

        set_static_lod_from_string("High");
        set_ideal_static_lod_from_string("Low");
        assert!(prefers_low_res_movies());
    }
}
