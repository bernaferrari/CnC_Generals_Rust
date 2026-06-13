// game_lod.rs - Game Level of Detail system
// Loads GameLOD.ini and exposes dynamic LOD parameters used by gameplay systems.

use std::collections::HashMap;
use std::fs;
use std::sync::{OnceLock, RwLock};

use crate::common::ini::ini_game_data::{get_global_data, GlobalData};
use crate::common::ini::ini_game_lod::{
    get_game_lod_manager, get_game_lod_manager_mut, StaticGameLODInfo, StaticGameLODLevel,
};

const MINIMUM_MEMORY_BYTES: u64 = 256 * 1024 * 1024;
const PROFILE_ERROR_LIMIT: f32 = 0.94;

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
static MEM_PASSED_OVERRIDE: OnceLock<RwLock<Option<bool>>> = OnceLock::new();

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

fn mem_passed_override() -> &'static RwLock<Option<bool>> {
    MEM_PASSED_OVERRIDE.get_or_init(|| RwLock::new(None))
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

#[cfg(unix)]
fn detected_physical_memory_bytes() -> Option<u64> {
    let pages = unsafe { libc::sysconf(libc::_SC_PHYS_PAGES) };
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGE_SIZE) };
    if pages <= 0 || page_size <= 0 {
        return None;
    }
    Some((pages as u64).saturating_mul(page_size as u64))
}

#[cfg(not(unix))]
fn detected_physical_memory_bytes() -> Option<u64> {
    None
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

    if level == StaticGameLODLevel::Custom {
        refresh_custom_static_lod_level();
    }

    let (lod_info, requested_texture_reduction) = {
        let manager = get_game_lod_manager();
        let lod_info = manager.static_game_lod_info[index].clone();
        let texture_reduction = if level == StaticGameLODLevel::Custom {
            lod_info.texture_reduction
        } else {
            recommended_texture_reduction(&manager, level)
        };
        (lod_info, texture_reduction)
    };

    let requested_trees = if level == StaticGameLODLevel::Custom {
        lod_info.use_trees
    } else {
        did_mem_pass()
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
    global.texture_reduction_factor = requested_texture_reduction;
    global.use_tree_sway = lod_info.use_tree_sway;
    global.use_draw_module_lod = !lod_info.use_buildup_scaffolds;
    global.use_heat_effects = lod_info.use_heat_effects;
    global.enable_dynamic_lod = lod_info.enable_dynamic_lod;
    global.use_fps_limit = lod_info.use_fps_limit;
    global.use_trees = requested_trees;
    if !did_mem_pass() {
        global.shell_map_on = false;
    }
}

fn recommended_texture_reduction(
    manager: &crate::common::ini::ini_game_lod::GameLODManager,
    requested_level: StaticGameLODLevel,
) -> i32 {
    if !did_mem_pass() {
        return manager.static_game_lod_info[StaticGameLODLevel::Low.to_index().unwrap()]
            .texture_reduction;
    }

    let ideal_level = canonical_static_lod_name(&get_ideal_static_lod())
        .and_then(StaticGameLODLevel::from_str)
        .filter(|level| {
            matches!(
                level,
                StaticGameLODLevel::Low | StaticGameLODLevel::Medium | StaticGameLODLevel::High
            )
        })
        .unwrap_or(requested_level);

    manager.static_game_lod_info[ideal_level.to_index().unwrap()].texture_reduction
}

/// Mirrors C++ `GameLODManager::refreshCustomStaticLODLevel`.
///
/// The options menu writes custom display settings into GlobalData before selecting
/// `STATIC_GAME_LOD_CUSTOM`; the C++ manager snapshots those live values into the
/// Custom LOD slot before applying it.
fn refresh_custom_static_lod_level() {
    let Some(global_data) = get_global_data() else {
        return;
    };
    let global = global_data.read();
    let mut manager = get_game_lod_manager_mut();
    if let Some(index) = StaticGameLODLevel::Custom.to_index() {
        refresh_custom_static_lod_info_from_global(
            &mut manager.static_game_lod_info[index],
            &global,
        );
    }
}

fn refresh_custom_static_lod_info_from_global(
    lod_info: &mut StaticGameLODInfo,
    global: &GlobalData,
) {
    lod_info.max_particle_count = global.max_particle_count;
    lod_info.use_shadow_volumes = global.use_shadow_volumes;
    lod_info.use_shadow_decals = global.use_shadow_decals;
    lod_info.use_cloud_map = global.use_cloud_map;
    lod_info.use_light_map = global.use_light_map;
    lod_info.show_soft_water_edge = global.show_soft_water_edge;
    lod_info.max_tank_track_edges = global.max_tank_track_edges;
    lod_info.max_tank_track_opaque_edges = global.max_tank_track_opaque_edges;
    lod_info.max_tank_track_fade_delay = global.max_tank_track_fade_delay;
    lod_info.use_buildup_scaffolds = !global.use_draw_module_lod;
    lod_info.use_heat_effects = global.use_heat_effects;
    lod_info.use_tree_sway = lod_info.use_buildup_scaffolds;
    lod_info.texture_reduction = global.texture_reduction_factor;
    lod_info.use_fps_limit = global.use_fps_limit;
    lod_info.enable_dynamic_lod = global.enable_dynamic_lod;
    lod_info.use_trees = global.use_trees;
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

/// Matches C++ GameLODManager::didMemPass.
///
/// C++ sets this during GameLODManager::init when detected physical memory is
/// within PROFILE_ERROR_LIMIT of 256 MB. Rust falls back to passing when memory
/// detection is unavailable so low-level load-screen code does not trigger
/// graphics/display probing.
pub fn did_mem_pass() -> bool {
    if let Some(value) = mem_passed_override().read().ok().and_then(|guard| *guard) {
        return value;
    }

    detected_physical_memory_bytes()
        .map(|total_bytes| {
            (total_bytes as f32 / MINIMUM_MEMORY_BYTES as f32) >= PROFILE_ERROR_LIMIT
        })
        .unwrap_or(true)
}

#[doc(hidden)]
pub fn set_mem_passed_override_for_tests(value: Option<bool>) {
    if let Ok(mut guard) = mem_passed_override().write() {
        *guard = value;
    }
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

    #[test]
    fn custom_static_lod_snapshots_current_global_settings_before_apply() {
        crate::common::ini::ini_game_data::init_global_data();
        let global_data = get_global_data().expect("global data initialized");

        {
            let mut global = global_data.write();
            global.max_particle_count = 4321;
            global.use_shadow_volumes = false;
            global.use_shadow_decals = true;
            global.use_cloud_map = false;
            global.use_light_map = true;
            global.show_soft_water_edge = false;
            global.max_tank_track_edges = 17;
            global.max_tank_track_opaque_edges = 9;
            global.max_tank_track_fade_delay = 12345;
            global.use_draw_module_lod = true;
            global.use_heat_effects = false;
            global.texture_reduction_factor = 2;
            global.use_fps_limit = true;
            global.enable_dynamic_lod = false;
            global.use_trees = false;
        }

        {
            let mut manager = get_game_lod_manager_mut();
            let custom_index = StaticGameLODLevel::Custom.to_index().unwrap();
            let custom = &mut manager.static_game_lod_info[custom_index];
            custom.max_particle_count = 111;
            custom.use_shadow_volumes = true;
            custom.use_cloud_map = true;
            custom.texture_reduction = 0;
            custom.enable_dynamic_lod = true;
            custom.use_trees = true;
        }

        set_static_lod_from_string("Custom");

        {
            let global = global_data.read();
            assert_eq!(global.max_particle_count, 4321);
            assert!(!global.use_shadow_volumes);
            assert!(global.use_shadow_decals);
            assert!(!global.use_cloud_map);
            assert!(global.use_light_map);
            assert!(!global.show_soft_water_edge);
            assert_eq!(global.max_tank_track_edges, 17);
            assert_eq!(global.max_tank_track_opaque_edges, 9);
            assert_eq!(global.max_tank_track_fade_delay, 12345);
            assert!(global.use_draw_module_lod);
            assert!(!global.use_heat_effects);
            assert_eq!(global.texture_reduction_factor, 2);
            assert!(global.use_fps_limit);
            assert!(!global.enable_dynamic_lod);
            assert!(!global.use_trees);
        }

        let manager = get_game_lod_manager();
        let custom = &manager.static_game_lod_info[StaticGameLODLevel::Custom.to_index().unwrap()];
        assert_eq!(custom.max_particle_count, 4321);
        assert!(!custom.use_shadow_volumes);
        assert!(!custom.use_buildup_scaffolds);
        assert!(!custom.use_tree_sway);
        assert_eq!(custom.texture_reduction, 2);
        assert!(!custom.enable_dynamic_lod);
        assert!(!custom.use_trees);
    }

    #[test]
    fn non_custom_static_lod_uses_cpp_memory_recommended_texture_reduction() {
        crate::common::ini::ini_game_data::init_global_data();
        let global_data = get_global_data().expect("global data initialized");

        {
            let mut manager = get_game_lod_manager_mut();
            let low_index = StaticGameLODLevel::Low.to_index().unwrap();
            let high_index = StaticGameLODLevel::High.to_index().unwrap();
            manager.static_game_lod_info[low_index].texture_reduction = 3;
            manager.static_game_lod_info[high_index].texture_reduction = 0;
            manager.static_game_lod_info[high_index].use_trees = true;
        }

        {
            let mut global = global_data.write();
            global.texture_reduction_factor = 0;
            global.use_trees = true;
            global.shell_map_on = true;
        }

        set_ideal_static_lod_from_string("Unknown");
        set_mem_passed_override_for_tests(Some(false));
        set_static_lod_from_string("High");

        {
            let global = global_data.read();
            assert_eq!(global.texture_reduction_factor, 3);
            assert!(!global.use_trees);
            assert!(!global.shell_map_on);
        }

        set_mem_passed_override_for_tests(None);
    }

    #[test]
    fn did_mem_pass_uses_override_for_cpp_load_screen_gate() {
        set_mem_passed_override_for_tests(Some(false));
        assert!(!did_mem_pass());

        set_mem_passed_override_for_tests(Some(true));
        assert!(did_mem_pass());

        set_mem_passed_override_for_tests(None);
    }
}
