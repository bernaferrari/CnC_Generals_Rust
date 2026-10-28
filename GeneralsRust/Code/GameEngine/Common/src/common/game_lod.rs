// game_lod.rs - Game Level of Detail system
// Loads GameLOD.ini and exposes dynamic LOD parameters used by gameplay systems.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

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

// Remaining classic probe/cache adapters; live level authority is on GameLODManager.
thread_local! {
    static DYNAMIC_LOD_SLOW_DEATH: RefCell<HashMap<String, f32>> = RefCell::new(HashMap::new());
    static MEM_PASSED_OVERRIDE: RefCell<Option<bool>> = RefCell::new(None);
    static CPU_FREQ_MHZ_OVERRIDE: RefCell<Option<i32>> = RefCell::new(None);
    static CPU_TYPE_OVERRIDE: RefCell<Option<CpuType>> = RefCell::new(None);
    static VIDEO_CHIP_OVERRIDE: RefCell<Option<ChipsetType>> = RefCell::new(None);
    static RAM_MB_OVERRIDE: RefCell<Option<i32>> = RefCell::new(None);
}
static SKIP_OPTIONS_PERSIST: AtomicBool = AtomicBool::new(false);

/// Result delivered to a receiving session after the application LOD request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStaticLodResult {
    /// Apply these preset-owned track controls to the receiving session.
    Apply {
        level: StaticGameLODLevel,
        rules: crate::common::ini::ini_game_data::TerrainTrackRules,
    },
    /// Static LOD is disabled. C++ changes current level to Custom without
    /// applying settings or rebuilding tracks, so update only the owner marker.
    MarkerOnly { level: StaticGameLODLevel },
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
    if let Some(level) = DynamicGameLODLevel::from_str(name) {
        set_dynamic_lod_level(level);
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
}

pub fn get_dynamic_lod() -> String {
    get_game_lod_manager()
        .get_dynamic_lod_level()
        .to_str()
        .to_owned()
}

pub fn set_static_lod_from_string(value: &str) {
    let _ = apply_static_lod_from_string(value, true);
}

/// Apply the application-selected static LOD and report its receiving-session
/// effect. Preset levels return only their three edge controls; Custom is a
/// reapply signal so Main preserves the receiving world's own rules.
///
/// The legacy terrain callback is intentionally not invoked here: Main owns
/// native terrain per render pipeline and must receive the effect explicitly.
pub fn set_static_lod_from_string_for_session(value: &str) -> Option<SessionStaticLodResult> {
    apply_static_lod_from_string(value, false)
}

fn apply_static_lod_from_string(
    value: &str,
    invoke_legacy_terrain_hook: bool,
) -> Option<SessionStaticLodResult> {
    let Some(mapped) = canonical_static_lod_name(value) else {
        return None;
    };
    let level = StaticGameLODLevel::from_str(mapped).expect("canonical StaticGameLOD level");
    let global_data = get_global_data()?;
    if !global_data.read().enable_static_lod {
        get_game_lod_manager_mut().set_current_static_lod_level(StaticGameLODLevel::Custom);
        return (!invoke_legacy_terrain_hook).then_some(SessionStaticLodResult::MarkerOnly {
            level: StaticGameLODLevel::Custom,
        });
    }
    if level == StaticGameLODLevel::Unknown
        || (level != StaticGameLODLevel::Custom
            && get_game_lod_manager().get_static_lod_level() == level)
    {
        // The application preset may already be active because another
        // receiving world selected it. Resolve the immutable preset anyway;
        // the receiving world performs its own same-level check.
        if !invoke_legacy_terrain_hook && level != StaticGameLODLevel::Unknown {
            return resolve_session_track_rules(level)
                .map(|rules| SessionStaticLodResult::Apply { level, rules });
        }
        return None;
    }
    // A synchronous callback may query this same manager. No manager loan
    // survives apply, and the original current level commits after callbacks.
    apply_static_lod_level(mapped, invoke_legacy_terrain_hook)?;
    get_game_lod_manager_mut().set_current_static_lod_level(level);
    if invoke_legacy_terrain_hook {
        None
    } else {
        resolve_session_track_rules(level)
            .map(|rules| SessionStaticLodResult::Apply { level, rules })
    }
}

/// Extract only preset-owned track edge controls. The receiving session keeps
/// its own MaxTerrainTracks/MakeTrackMarks values. Custom returns a signal
/// payload; Main deliberately ignores those placeholder fields and reapplies
/// its current receiving rules.
fn resolve_session_track_rules(
    level: StaticGameLODLevel,
) -> Option<crate::common::ini::ini_game_data::TerrainTrackRules> {
    use crate::common::ini::ini_game_data::TerrainTrackRules;
    if level == StaticGameLODLevel::Custom {
        return Some(TerrainTrackRules::default());
    }
    let index = level.to_index()?;
    let preset = get_game_lod_manager().static_game_lod_info[index].clone();
    let mut rules = TerrainTrackRules::default();
    rules.max_tank_track_edges = preset.max_tank_track_edges;
    rules.max_tank_track_opaque_edges = preset.max_tank_track_opaque_edges;
    rules.max_tank_track_fade_delay = preset.max_tank_track_fade_delay;
    Some(rules)
}

/// Apply StaticGameLOD settings to GlobalData.
/// Matches C++ GameLODManager::applyStaticLODLevel().
fn apply_static_lod_level(
    level_name: &str,
    invoke_legacy_terrain_hook: bool,
) -> Option<crate::common::ini::ini_game_data::TerrainTrackRules> {
    let level = match StaticGameLODLevel::from_str(level_name) {
        Some(l) => l,
        None => return None,
    };
    let index = match level.to_index() {
        Some(i) => i,
        None => return None,
    };

    if level == StaticGameLODLevel::Custom {
        refresh_custom_static_lod_level();
    }

    let lod_info = get_game_lod_manager().static_game_lod_info[index].clone();
    // Lazy recommendation can now write this manager's ideal field. End the
    // preset read before invoking it; callbacks likewise retain no guard.
    let requested_texture_reduction = if level == StaticGameLODLevel::Custom {
        lod_info.texture_reduction
    } else {
        recommended_texture_reduction(level)
    };

    let requested_trees = if level == StaticGameLODLevel::Custom {
        lod_info.use_trees
    } else {
        did_mem_pass()
    };

    let Some(global_data) = get_global_data() else {
        return None;
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
    let track_rules =
        crate::common::ini::ini_game_data::TerrainTrackRules::from_global_data(&global);
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
    if invoke_legacy_terrain_hook {
        if let Some(hook) = REBUILD_TANK_TRACKS.get() {
            hook();
        }
    }
    Some(track_rules)
}

fn recommended_texture_reduction(requested_level: StaticGameLODLevel) -> i32 {
    if get_ideal_static_lod().eq_ignore_ascii_case("Unknown") {
        let _ = find_static_lod_level();
    }
    if !did_mem_pass() {
        return get_game_lod_manager().static_game_lod_info
            [StaticGameLODLevel::Low.to_index().unwrap()]
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

    get_game_lod_manager().static_game_lod_info[ideal_level.to_index().unwrap()].texture_reduction
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
    get_game_lod_manager()
        .get_static_lod_level()
        .to_str()
        .to_owned()
}

pub fn set_ideal_static_lod_from_string(value: &str) {
    let Some(mapped) = canonical_static_lod_name(value) else {
        return;
    };
    get_game_lod_manager_mut().set_ideal_static_lod_level(
        StaticGameLODLevel::from_str(mapped).expect("canonical ideal StaticGameLOD level"),
    );
}

pub fn get_ideal_static_lod() -> String {
    get_game_lod_manager()
        .get_ideal_static_lod_level()
        .to_str()
        .to_owned()
}

/// Matches C++ GameLODManager::didMemPass.
///
/// C++ sets this during GameLODManager::init when detected physical memory is
/// within PROFILE_ERROR_LIMIT of 256 MB. Rust falls back to passing when memory
/// detection is unavailable so low-level load-screen code does not trigger
/// graphics/display probing.
pub fn did_mem_pass() -> bool {
    if let Some(value) = MEM_PASSED_OVERRIDE.with(|cell| *cell.borrow()) {
        return value;
    }

    detected_physical_memory_bytes()
        .map(|total_bytes| {
            (total_bytes as f32 / MINIMUM_MEMORY_BYTES as f32) >= PROFILE_ERROR_LIMIT
        })
        .unwrap_or(true)
}

pub fn is_really_low_mhz() -> bool {
    let cpu_freq_mhz = CPU_FREQ_MHZ_OVERRIDE
        .with(|cell| *cell.borrow())
        .or_else(detected_cpu_frequency_mhz);
    let Some(cpu_freq_mhz) = cpu_freq_mhz else {
        return false;
    };

    cpu_freq_mhz < get_game_lod_manager().really_low_mhz
}

#[doc(hidden)]
pub fn set_mem_passed_override_for_tests(value: Option<bool>) {
    MEM_PASSED_OVERRIDE.with(|cell| *cell.borrow_mut() = value);
}

#[doc(hidden)]
pub fn set_cpu_freq_mhz_override_for_tests(value: Option<i32>) {
    CPU_FREQ_MHZ_OVERRIDE.with(|cell| *cell.borrow_mut() = value);
}

#[doc(hidden)]
pub fn reset_static_lod_state_for_tests() {
    SKIP_OPTIONS_PERSIST.store(true, Ordering::Relaxed);
    {
        let mut manager = get_game_lod_manager_mut();
        manager.set_current_static_lod_level(StaticGameLODLevel::Unknown);
        manager.set_ideal_static_lod_level(StaticGameLODLevel::Unknown);
    }
    set_mem_passed_override_for_tests(None);
    set_hardware_overrides_for_tests(None, None, None, None);
}

fn probe_cpu_type() -> CpuType {
    if let Some(value) = CPU_TYPE_OVERRIDE.with(|cell| *cell.borrow()) {
        return value;
    }
    // Presets only name P3/P4/K7. Modern hardware is treated as P4 so the
    // HIGH walk can match; unknown XX never equals a preset (C++ remaps via bench).
    CpuType::P4
}

fn probe_video_chip() -> ChipsetType {
    if let Some(value) = VIDEO_CHIP_OVERRIDE.with(|cell| *cell.borrow()) {
        return value;
    }
    // C++ unknown video becomes TNT2. Modern wgpu/Metal is at least R300.
    ChipsetType::Radeon9700
}

fn probe_ram_mb() -> i32 {
    if let Some(value) = RAM_MB_OVERRIDE.with(|cell| *cell.borrow()) {
        return value;
    }
    detected_physical_memory_bytes()
        .map(|bytes| (bytes / (1024 * 1024)) as i32)
        .unwrap_or(256)
}

fn probe_cpu_mhz() -> i32 {
    CPU_FREQ_MHZ_OVERRIDE
        .with(|cell| *cell.borrow())
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

    let current_unknown =
        get_game_lod_manager().get_static_lod_level() == StaticGameLODLevel::Unknown;
    persist_recommended_static_lod(name, current_unknown);
    name.to_string()
}

/// C++ W3DDisplay::init / Options first-open: if static LOD is still
/// UNKNOWN, apply `findStaticLODLevel()`.
pub fn ensure_static_lod_applied() {
    let unknown = get_game_lod_manager().get_static_lod_level() == StaticGameLODLevel::Unknown;
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
    CPU_TYPE_OVERRIDE.with(|cell| *cell.borrow_mut() = cpu);
    set_cpu_freq_mhz_override_for_tests(mhz);
    VIDEO_CHIP_OVERRIDE.with(|cell| *cell.borrow_mut() = video);
    RAM_MB_OVERRIDE.with(|cell| *cell.borrow_mut() = ram_mb);
}

pub fn prefers_low_res_movies() -> bool {
    // C++ ScoreScreen: !didMemPass() || findStaticLODLevel()==LOW || getStatic==LOW
    !did_mem_pass()
        || matches!(find_static_lod_level().as_str(), "Low")
        || matches!(get_static_lod().as_str(), "Low")
}

fn ensure_game_lod_loaded() {
    load_game_lod_ini_presets_and_options();

    DYNAMIC_LOD_SLOW_DEATH.with(|cell| {
        let mut map_guard = cell.borrow_mut();
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
    });
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
#[path = "game_lod/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "game_lod/level_authority_tests.rs"]
mod lod_level_acceptance_controls;

#[cfg(test)]
#[path = "game_lod/static_request_tests.rs"]
mod static_request_controls;
