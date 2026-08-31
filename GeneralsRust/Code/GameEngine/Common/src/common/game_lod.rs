// game_lod.rs - Game Level of Detail system
// Loads GameLOD.ini and exposes dynamic LOD parameters used by gameplay systems.

use std::collections::HashMap;
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{OnceLock, RwLock};

use crate::common::ini::ini_game_data::{GlobalData, get_global_data};
use crate::common::ini::ini_game_lod::{
    ChipsetType, CpuType, DynamicGameLODLevel, StaticGameLODInfo, StaticGameLODLevel,
    get_game_lod_manager, get_game_lod_manager_mut, set_dynamic_lod_level,
};
use crate::common::ini::{INI, INILoadType};
use crate::common::user_preferences::UserPreferences;

static REBUILD_SHADOWS: OnceLock<fn()> = OnceLock::new();
static REBUILD_SHORELINE: OnceLock<fn()> = OnceLock::new();
static REBUILD_TANK_TRACKS: OnceLock<fn()> = OnceLock::new();
static ADJUST_CLIENT_LOD: OnceLock<fn(i32)> = OnceLock::new();
static GAME_LOD_INI_LOADED: OnceLock<bool> = OnceLock::new();

/// Terrain/Shadow slices register these; Common only invokes them.
pub fn register_rebuild_shadows(hook: fn()) {
    let _ = REBUILD_SHADOWS.set(hook);
}
pub fn register_rebuild_shoreline(hook: fn()) {
    let _ = REBUILD_SHORELINE.set(hook);
}
pub fn register_rebuild_tank_tracks(hook: fn()) {
    let _ = REBUILD_TANK_TRACKS.set(hook);
}
pub fn register_adjust_client_lod(hook: fn(i32)) {
    let _ = ADJUST_CLIENT_LOD.set(hook);
}

/// C++ `TheGlobalData->m_useShadowDecals` (Options "2D Shadows").
///
/// Options writes `writable.use_shadow_decals`; LOD / GameData.ini write
/// `GlobalData.use_shadow_decals`. Live addShadow / blob collect read this
/// helper so both stores stay one C++ field.
pub fn use_shadow_decals() -> bool {
    if let Ok(runtime) = crate::common::global_data::read_safe() {
        return runtime.writable.use_shadow_decals;
    }
    get_global_data()
        .map(|global| global.read().use_shadow_decals)
        .unwrap_or(true)
}

fn sync_writable_shadow_decals(use_volumes: bool, use_decals: bool) {
    if let Ok(mut runtime) = crate::common::global_data::write_safe() {
        runtime.writable.use_shadow_volumes = use_volumes;
        runtime.writable.use_shadow_decals = use_decals;
    }
}

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
static CURRENT_STATIC_LOD_NAME: OnceLock<RwLock<String>> = OnceLock::new();
static IDEAL_STATIC_LOD_NAME: OnceLock<RwLock<String>> = OnceLock::new();
static MEM_PASSED_OVERRIDE: OnceLock<RwLock<Option<bool>>> = OnceLock::new();
static CPU_FREQ_MHZ_OVERRIDE: OnceLock<RwLock<Option<i32>>> = OnceLock::new();
static CPU_TYPE_OVERRIDE: OnceLock<RwLock<Option<CpuType>>> = OnceLock::new();
static VIDEO_CHIP_OVERRIDE: OnceLock<RwLock<Option<ChipsetType>>> = OnceLock::new();
static RAM_MB_OVERRIDE: OnceLock<RwLock<Option<i32>>> = OnceLock::new();
static SKIP_OPTIONS_PERSIST: AtomicBool = AtomicBool::new(false);

fn dynamic_lod_name() -> &'static RwLock<String> {
    DYNAMIC_LOD_NAME.get_or_init(|| RwLock::new("High".to_string()))
}

fn dynamic_lod_slow_death() -> &'static RwLock<HashMap<String, f32>> {
    DYNAMIC_LOD_SLOW_DEATH.get_or_init(|| RwLock::new(HashMap::new()))
}

fn static_lod_name() -> &'static RwLock<String> {
    STATIC_LOD_NAME.get_or_init(|| RwLock::new("Medium".to_string()))
}

fn current_static_lod_name() -> &'static RwLock<String> {
    CURRENT_STATIC_LOD_NAME.get_or_init(|| RwLock::new("Unknown".to_string()))
}

fn ideal_static_lod_name() -> &'static RwLock<String> {
    IDEAL_STATIC_LOD_NAME.get_or_init(|| RwLock::new("Unknown".to_string()))
}

fn mem_passed_override() -> &'static RwLock<Option<bool>> {
    MEM_PASSED_OVERRIDE.get_or_init(|| RwLock::new(None))
}

fn cpu_freq_mhz_override() -> &'static RwLock<Option<i32>> {
    CPU_FREQ_MHZ_OVERRIDE.get_or_init(|| RwLock::new(None))
}

fn cpu_type_override() -> &'static RwLock<Option<CpuType>> {
    CPU_TYPE_OVERRIDE.get_or_init(|| RwLock::new(None))
}

fn video_chip_override() -> &'static RwLock<Option<ChipsetType>> {
    VIDEO_CHIP_OVERRIDE.get_or_init(|| RwLock::new(None))
}

fn ram_mb_override() -> &'static RwLock<Option<i32>> {
    RAM_MB_OVERRIDE.get_or_init(|| RwLock::new(None))
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
    // SAFETY: sysconf(_SC_PHYS_PAGES) is a thread-safe libc query taking
    // no pointers.
    let pages = unsafe { libc::sysconf(libc::_SC_PHYS_PAGES) };
    // SAFETY: sysconf(_SC_PAGE_SIZE) is a thread-safe libc query taking
    // no pointers.
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

#[cfg(target_os = "linux")]
fn detected_cpu_frequency_mhz() -> Option<i32> {
    let contents = fs::read_to_string("/proc/cpuinfo").ok()?;
    contents.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        if !key.trim().eq_ignore_ascii_case("cpu mhz") {
            return None;
        }
        value.trim().parse::<f32>().ok().map(|mhz| mhz as i32)
    })
}

#[cfg(target_os = "macos")]
fn detected_cpu_frequency_mhz() -> Option<i32> {
    let mut freq_hz: u64 = 0;
    let mut len = std::mem::size_of::<u64>();
    let name = b"hw.cpufrequency\0";
    // SAFETY: sysctlbyname with a static NUL-terminated name, an output
    // buffer sized by `len`, and null oldp/newp arguments.
    let result = unsafe {
        libc::sysctlbyname(
            name.as_ptr() as *const libc::c_char,
            (&mut freq_hz as *mut u64).cast(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if result != 0 || freq_hz == 0 {
        return None;
    }
    Some((freq_hz / 1_000_000) as i32)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn detected_cpu_frequency_mhz() -> Option<i32> {
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
    if mapped.is_empty() {
        return;
    }
    set_dynamic_lod(mapped);
    if let Some(level) = DynamicGameLODLevel::from_str(mapped) {
        set_dynamic_lod_level(level);
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
    if mapped != "Custom"
        && current_static_lod_name()
            .read()
            .map(|guard| guard.as_str() == mapped)
            .unwrap_or(false)
    {
        return;
    }
    apply_static_lod_level(mapped);
    if mapped != "Unknown" {
        if let Ok(mut guard) = current_static_lod_name().write() {
            *guard = mapped.to_string();
        }
    }
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

    let prev_shadow_volumes = global.use_shadow_volumes;
    let prev_shadow_decals = global.use_shadow_decals;
    let prev_soft_water = global.show_soft_water_edge;

    let prev_texture = global.texture_reduction_factor;

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
    if !did_mem_pass() || is_really_low_mhz() {
        global.shell_map_on = false;
    }
    drop(global);
    sync_writable_shadow_decals(lod_info.use_shadow_volumes, lod_info.use_shadow_decals);

    // C++ GameLODManager::applyStaticLODLevel client/terrain side effects.
    if requested_texture_reduction != prev_texture {
        if let Some(hook) = ADJUST_CLIENT_LOD.get() {
            hook(0);
        }
    }
    if lod_info.use_shadow_volumes != prev_shadow_volumes
        || lod_info.use_shadow_decals != prev_shadow_decals
    {
        if let Some(hook) = REBUILD_SHADOWS.get() {
            hook();
        }
    }
    if lod_info.show_soft_water_edge != prev_soft_water {
        if let Some(hook) = REBUILD_SHORELINE.get() {
            hook();
        }
    }
    if let Some(hook) = REBUILD_TANK_TRACKS.get() {
        hook();
    }
}

fn recommended_texture_reduction(
    manager: &crate::common::ini::ini_game_lod::GameLODManager,
    requested_level: StaticGameLODLevel,
) -> i32 {
    if get_ideal_static_lod().eq_ignore_ascii_case("Unknown") {
        let _ = find_static_lod_level();
    }
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

pub fn is_really_low_mhz() -> bool {
    let Some(cpu_freq_mhz) = cpu_freq_mhz_override()
        .read()
        .ok()
        .and_then(|guard| *guard)
        .or_else(detected_cpu_frequency_mhz)
    else {
        return false;
    };

    cpu_freq_mhz < get_game_lod_manager().really_low_mhz
}

#[doc(hidden)]
pub fn set_mem_passed_override_for_tests(value: Option<bool>) {
    if let Ok(mut guard) = mem_passed_override().write() {
        *guard = value;
    }
}

#[doc(hidden)]
pub fn set_cpu_freq_mhz_override_for_tests(value: Option<i32>) {
    if let Ok(mut guard) = cpu_freq_mhz_override().write() {
        *guard = value;
    }
}

#[doc(hidden)]
pub fn reset_static_lod_state_for_tests() {
    SKIP_OPTIONS_PERSIST.store(true, Ordering::Relaxed);
    if let Ok(mut guard) = static_lod_name().write() {
        *guard = "Medium".to_string();
    }
    if let Ok(mut guard) = current_static_lod_name().write() {
        *guard = "Unknown".to_string();
    }
    if let Ok(mut guard) = ideal_static_lod_name().write() {
        *guard = "Unknown".to_string();
    }
    set_mem_passed_override_for_tests(None);
    set_hardware_overrides_for_tests(None, None, None, None);
}

fn probe_cpu_type() -> CpuType {
    if let Some(value) = cpu_type_override().read().ok().and_then(|guard| *guard) {
        return value;
    }
    // Presets only name P3/P4/K7. Modern hardware is treated as P4 so the
    // HIGH walk can match; unknown XX never equals a preset (C++ remaps via bench).
    CpuType::P4
}

fn probe_video_chip() -> ChipsetType {
    if let Some(value) = video_chip_override().read().ok().and_then(|guard| *guard) {
        return value;
    }
    // C++ unknown video becomes TNT2. Modern wgpu/Metal is at least R300.
    ChipsetType::Radeon9700
}

fn probe_ram_mb() -> i32 {
    if let Some(value) = ram_mb_override().read().ok().and_then(|guard| *guard) {
        return value;
    }
    detected_physical_memory_bytes()
        .map(|bytes| (bytes / (1024 * 1024)) as i32)
        .unwrap_or(256)
}

fn probe_cpu_mhz() -> i32 {
    cpu_freq_mhz_override()
        .read()
        .ok()
        .and_then(|guard| *guard)
        .or_else(detected_cpu_frequency_mhz)
        .unwrap_or(2000)
}

fn persist_recommended_static_lod(ideal: &str, also_static: bool) {
    if SKIP_OPTIONS_PERSIST.load(Ordering::Relaxed) {
        return;
    }
    let mut prefs = UserPreferences::new();
    let _ = prefs.load("Options.ini");
    prefs.set_string("IdealStaticGameLOD", ideal.to_string());
    if also_static {
        prefs.set_string("StaticGameLOD", ideal.to_string());
    }
    let _ = prefs.write();
}

/// C++ `GameLODManager::findStaticLODLevel`.
///
/// First run (ideal still Unknown) walks GameLODPresets.ini against
/// CPU/MHz/video/RAM (`PROFILE_ERROR_LIMIT` 0.94), writes
/// `IdealStaticGameLOD`, and writes `StaticGameLOD` when current is Unknown.
pub fn find_static_lod_level() -> String {
    let current_ideal = get_ideal_static_lod();
    if !current_ideal.eq_ignore_ascii_case("Unknown") {
        return current_ideal;
    }

    let matched = get_game_lod_manager().match_static_lod_presets(
        probe_cpu_type(),
        probe_cpu_mhz(),
        probe_video_chip(),
        probe_ram_mb(),
    );
    let name = matched.to_str();
    set_ideal_static_lod_from_string(name);

    let current_unknown = current_static_lod_name()
        .read()
        .map(|guard| guard.eq_ignore_ascii_case("Unknown"))
        .unwrap_or(true);
    persist_recommended_static_lod(name, current_unknown);
    if current_unknown {
        if let Ok(mut guard) = static_lod_name().write() {
            *guard = name.to_string();
        }
    }
    name.to_string()
}

/// C++ W3DDisplay::init / Options first-open: if static LOD is still
/// UNKNOWN, apply `findStaticLODLevel()`.
pub fn ensure_static_lod_applied() {
    let unknown = current_static_lod_name()
        .read()
        .map(|guard| guard.eq_ignore_ascii_case("Unknown"))
        .unwrap_or(true);
    if unknown {
        let level = find_static_lod_level();
        set_static_lod_from_string(&level);
    }
}

#[doc(hidden)]
pub fn set_hardware_overrides_for_tests(
    cpu: Option<CpuType>,
    mhz: Option<i32>,
    video: Option<ChipsetType>,
    ram_mb: Option<i32>,
) {
    if let Ok(mut guard) = cpu_type_override().write() {
        *guard = cpu;
    }
    set_cpu_freq_mhz_override_for_tests(mhz);
    if let Ok(mut guard) = video_chip_override().write() {
        *guard = video;
    }
    if let Ok(mut guard) = ram_mb_override().write() {
        *guard = ram_mb;
    }
}

pub fn prefers_low_res_movies() -> bool {
    // C++ ScoreScreen: !didMemPass() || findStaticLODLevel()==LOW || getStatic==LOW
    !did_mem_pass()
        || matches!(find_static_lod_level().as_str(), "Low")
        || matches!(get_static_lod().as_str(), "Low")
}

fn ensure_game_lod_loaded() {
    load_game_lod_ini_presets_and_options();

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

/// C++ `GameLODManager::init`: load GameLOD.ini, GameLODPresets.ini, snapshot
/// Custom from GlobalData, apply OptionPreferences, then setStaticLODLevel.
pub fn load_game_lod_ini_presets_and_options() {
    if GAME_LOD_INI_LOADED.get().is_some() {
        return;
    }
    let _ = GAME_LOD_INI_LOADED.set(true);

    crate::common::ini::ini_game_lod::init_game_lod_manager();

    let mut ini = INI::new();
    for virtual_path in ["Data/INI/GameLOD.ini", "Data/INI/GameLODPresets.ini"] {
        if let Some(path) =
            crate::common::system::install_layout::resolve_data_ini_file(virtual_path)
        {
            if let Err(err) = ini.load(&path, INILoadType::Overwrite) {
                eprintln!("Failed to load GameLOD INI '{}': {err}", path.display());
            }
        }
    }

    refresh_custom_static_lod_level();

    let mut prefs = UserPreferences::new();
    let _ = prefs.load("Options.ini");
    if let Some(ideal) = prefs.get_string("IdealStaticGameLOD").cloned() {
        set_ideal_static_lod_from_string(&ideal);
    }
    let user_detail = prefs
        .get_string("StaticGameLOD")
        .cloned()
        .unwrap_or_else(|| "Unknown".to_string());

    if user_detail.eq_ignore_ascii_case("Custom") {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            if let Some(v) = prefs.get_int("TextureReduction") {
                if v >= 0 {
                    global.texture_reduction_factor = v;
                }
            }
            if let Some(v) = prefs.get_bool("UseShadowVolumes") {
                global.use_shadow_volumes = v;
            }
            if let Some(v) = prefs.get_bool("UseShadowDecals") {
                global.use_shadow_decals = v;
            }
            if let Some(v) = prefs.get_int("MaxParticleCount") {
                global.max_particle_count = v;
            }
            if let Some(v) = prefs.get_bool("UseLightMap") {
                global.use_light_map = v;
            }
            if let Some(v) = prefs.get_bool("UseCloudMap") {
                global.use_cloud_map = v;
            }
            if let Some(v) = prefs.get_bool("ShowSoftWaterEdge") {
                global.show_soft_water_edge = v;
            }
            // C++ OptionPreferences keys (OptionsMenu.cpp), not GameLOD.ini names.
            if let Some(v) = prefs.get_bool("HeatEffects") {
                global.use_heat_effects = v;
            }
            if let Some(v) = prefs.get_bool("ShowTrees") {
                global.use_trees = v;
            }
            if let Some(v) = prefs.get_bool("DynamicLOD") {
                global.enable_dynamic_lod = v;
            }
            if let Some(v) = prefs.get_bool("FPSLimit") {
                global.use_fps_limit = v;
            }
            if let Some(v) = prefs.get_bool("BuildingOcclusion") {
                global.enable_behind_building_markers = v;
            }
            if let Some(v) = prefs.get_bool("ExtraAnimations") {
                // ExtraAnimations=yes → extra enabled → useDrawModuleLOD false
                global.use_draw_module_lod = !v;
                global.use_tree_sway = !global.use_draw_module_lod;
            }
        }
        if let Some(global_data) = get_global_data() {
            let global = global_data.read();
            sync_writable_shadow_decals(global.use_shadow_volumes, global.use_shadow_decals);
        }
    }

    if user_detail.eq_ignore_ascii_case("Unknown") {
        ensure_static_lod_applied();
    } else {
        set_static_lod_from_string(&user_detail);
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
    // C++ TheGameLODManager->getSlowDeathScale() reads the live field
    // copied by setDynamicLODLevel / applyDynamicLODLevel.
    get_game_lod_manager().get_slow_death_scale()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_lod_parser_tracks_current_and_ideal_low_detail() {
        reset_static_lod_state_for_tests();
        set_static_lod_from_string("Medium");
        set_ideal_static_lod_from_string("Medium");
        assert!(!prefers_low_res_movies());

        set_static_lod_from_string("Low");
        assert!(prefers_low_res_movies());

        set_static_lod_from_string("High");
        set_ideal_static_lod_from_string("Low");
        assert!(prefers_low_res_movies());
    }

    #[test]
    fn custom_static_lod_snapshots_current_global_settings_before_apply() {
        reset_static_lod_state_for_tests();
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
        reset_static_lod_state_for_tests();
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
    fn repeated_non_custom_static_lod_does_not_reapply_cpp_current_level() {
        reset_static_lod_state_for_tests();
        crate::common::ini::ini_game_data::init_global_data();
        let global_data = get_global_data().expect("global data initialized");

        {
            let mut manager = get_game_lod_manager_mut();
            let high_index = StaticGameLODLevel::High.to_index().unwrap();
            manager.static_game_lod_info[high_index].max_particle_count = 222;
        }

        {
            let mut global = global_data.write();
            global.max_particle_count = 111;
        }

        set_mem_passed_override_for_tests(Some(true));
        set_cpu_freq_mhz_override_for_tests(Some(1000));
        set_static_lod_from_string("High");
        assert_eq!(global_data.read().max_particle_count, 222);

        global_data.write().max_particle_count = 333;
        set_static_lod_from_string("High");
        assert_eq!(global_data.read().max_particle_count, 333);

        global_data.write().max_particle_count = 444;
        set_static_lod_from_string("Custom");
        assert_eq!(global_data.read().max_particle_count, 444);

        set_mem_passed_override_for_tests(None);
        set_cpu_freq_mhz_override_for_tests(None);
    }

    #[test]
    fn really_low_mhz_disables_shell_map_even_when_memory_passes() {
        reset_static_lod_state_for_tests();
        crate::common::ini::ini_game_data::init_global_data();
        let global_data = get_global_data().expect("global data initialized");

        {
            let mut manager = get_game_lod_manager_mut();
            manager.set_really_low_mhz(400);
        }

        {
            let mut global = global_data.write();
            global.shell_map_on = true;
        }

        set_mem_passed_override_for_tests(Some(true));
        set_cpu_freq_mhz_override_for_tests(Some(399));
        set_static_lod_from_string("High");

        assert!(!global_data.read().shell_map_on);

        set_mem_passed_override_for_tests(None);
        set_cpu_freq_mhz_override_for_tests(None);
    }

    #[test]
    fn did_mem_pass_uses_override_for_cpp_load_screen_gate() {
        set_mem_passed_override_for_tests(Some(false));
        assert!(!did_mem_pass());

        set_mem_passed_override_for_tests(Some(true));
        assert!(did_mem_pass());

        set_mem_passed_override_for_tests(None);
    }

    #[test]
    fn find_static_lod_level_walks_presets_high_to_low() {
        reset_static_lod_state_for_tests();
        {
            let mut manager = get_game_lod_manager_mut();
            manager.init();
            let high = manager.new_lod_preset(2).expect("high preset");
            high.cpu_type = CpuType::P4;
            high.mhz = 1500;
            high.video_type = ChipsetType::GeForce4;
            high.memory = 256;
            let low = manager.new_lod_preset(0).expect("low preset");
            low.cpu_type = CpuType::P3;
            low.mhz = 800;
            low.video_type = ChipsetType::TNT2;
            low.memory = 128;
        }

        set_hardware_overrides_for_tests(
            Some(CpuType::P4),
            Some(2000),
            Some(ChipsetType::Radeon9700),
            Some(1024),
        );
        assert_eq!(find_static_lod_level(), "High");

        reset_static_lod_state_for_tests();
        set_hardware_overrides_for_tests(
            Some(CpuType::P3),
            Some(850),
            Some(ChipsetType::TNT2),
            Some(128),
        );
        assert_eq!(find_static_lod_level(), "Low");
        set_hardware_overrides_for_tests(None, None, None, None);
        get_game_lod_manager_mut().init();
    }

    #[test]
    fn get_slow_death_scale_reads_live_dynamic_lod() {
        reset_static_lod_state_for_tests();
        set_dynamic_lod_from_string("High");
        {
            let mut manager = get_game_lod_manager_mut();
            let low = DynamicGameLODLevel::Low.to_index().unwrap();
            manager.dynamic_game_lod_info[low].slow_death_scale = 0.25;
        }
        set_dynamic_lod_from_string("Low");
        assert!((get_slow_death_scale() - 0.25).abs() < f32::EPSILON);
        set_dynamic_lod_from_string("High");
    }
}
