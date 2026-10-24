//! INI parsing for GameLOD definitions
//!
//! This module handles parsing StaticGameLOD, DynamicGameLOD, LODPreset, and BenchProfile
//! blocks from INI files (GameLOD.ini and GameLODPresets.ini).
//!
//! C++ Reference: GeneralsMD/Code/GameEngine/Source/Common/GameLOD.cpp
//! C++ Header: GeneralsMD/Code/GameEngine/Include/Common/GameLOD.h
//!
//! Rust port: 2025

use once_cell::sync::OnceCell;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::common::ini::{INI, INIError, INIResult};

mod definitions;
pub use definitions::{
    BenchProfile, ChipsetType, CpuType, DynamicGameLODInfo, DynamicGameLODLevel, LODPresetInfo,
    MAX_BENCH_PROFILES, MAX_LOD_PRESETS_PER_LEVEL, ParticlePriorityType, StaticGameLODInfo,
    StaticGameLODLevel,
};

impl ParticlePriorityType {
    fn parse_cpp_index_list_token(s: &str) -> Option<Self> {
        if let Ok(index) = s.parse::<usize>() {
            return ParticlePriorityType::from_index(index);
        }
        ParticlePriorityType::from_str(s)
    }
}

fn ini_field_key_value<'a>(tokens: &'a [&'a str]) -> (&'a str, &'a str) {
    let raw = tokens.first().copied().unwrap_or("");
    if let Some((key, value)) = raw.split_once('=') {
        let value = value.trim();
        if !value.is_empty() {
            return (key, value);
        }
        let value = tokens
            .iter()
            .skip(1)
            .copied()
            .find(|token| *token != "=")
            .unwrap_or("");
        return (key, value);
    }
    let value = tokens
        .iter()
        .skip(1)
        .copied()
        .find(|token| *token != "=")
        .unwrap_or("");
    (raw, value)
}

fn parse_cpp_yes_no_bool(token: &str) -> INIResult<bool> {
    match token.to_ascii_lowercase().as_str() {
        "yes" => Ok(true),
        "no" => Ok(false),
        _ => Err(INIError::InvalidData),
    }
}

fn parse_cpp_int_bits_as_u32(token: &str) -> INIResult<u32> {
    Ok(INI::parse_int(token)? as u32)
}

// ============================================================================
// Game LOD Manager
// ============================================================================

/// Game LOD Manager
/// Matches C++ GameLODManager from GameLOD.h
pub struct GameLODManager {
    /// Static LOD info for each level
    pub static_game_lod_info: [StaticGameLODInfo; 4],
    /// Dynamic LOD info for each level
    pub dynamic_game_lod_info: [DynamicGameLODInfo; 4],
    /// LOD presets for hardware detection (indexed by StaticGameLODLevel, excluding Custom)
    pub lod_presets: [[Option<LODPresetInfo>; MAX_LOD_PRESETS_PER_LEVEL]; 3],
    /// Benchmark profiles for hardware classification
    pub bench_profiles: [Option<BenchProfile>; MAX_BENCH_PROFILES],
    /// Number of presets per level
    num_level_presets: [usize; 3],
    /// Number of benchmark profiles
    num_bench_profiles: usize,
    /// Really low MHz threshold for disabling shell map
    pub really_low_mhz: i32,

    /// C++ GameLOD.cpp186–208: no static level has been applied at construction.
    current_static_lod: StaticGameLODLevel,
    ideal_static_lod: StaticGameLODLevel,

    /// Current dynamic LOD. C++ `m_currentDynamicLOD`, default HIGH.
    current_dynamic_lod: DynamicGameLODLevel,
    /// Particles generated since the last dynamic LOD apply. C++ `m_numParticleGenerations`.
    num_particle_generations: i32,
    /// Runtime particle skip mask copied from DynamicGameLODInfo. C++ `m_dynamicParticleSkipMask`.
    dynamic_particle_skip_mask: i32,
    /// Debris generated since the last dynamic LOD apply. C++ `m_numDebrisGenerations`.
    num_debris_generations: i32,
    /// Runtime debris skip mask copied from DynamicGameLODInfo. C++ `m_dynamicDebrisSkipMask`.
    dynamic_debris_skip_mask: i32,
    /// Values < 1.0 accelerate deaths. C++ `m_slowDeathScale`.
    slow_death_scale: f32,
    /// Minimum particle priority that may still render. C++ `m_minDynamicParticlePriority`.
    min_dynamic_particle_priority: ParticlePriorityType,
    /// Priorities at/above this never skip. C++ `m_minDynamicParticleSkipPriority`.
    min_dynamic_particle_skip_priority: ParticlePriorityType,
}

impl Default for GameLODManager {
    fn default() -> Self {
        Self::new()
    }
}

impl GameLODManager {
    pub fn new() -> Self {
        Self {
            static_game_lod_info: [
                StaticGameLODInfo::default(),
                StaticGameLODInfo::default(),
                StaticGameLODInfo::default(),
                StaticGameLODInfo::default(),
            ],
            dynamic_game_lod_info: [
                DynamicGameLODInfo::default(),
                DynamicGameLODInfo::default(),
                DynamicGameLODInfo::default(),
                DynamicGameLODInfo::default(),
            ],
            lod_presets: Default::default(),
            bench_profiles: Default::default(),
            num_level_presets: [0; 3],
            num_bench_profiles: 0,
            really_low_mhz: 400,
            current_static_lod: StaticGameLODLevel::Unknown,
            ideal_static_lod: StaticGameLODLevel::Unknown,
            current_dynamic_lod: DynamicGameLODLevel::High,
            num_particle_generations: 0,
            dynamic_particle_skip_mask: 0,
            num_debris_generations: 0,
            dynamic_debris_skip_mask: 0,
            slow_death_scale: 1.0,
            min_dynamic_particle_priority: ParticlePriorityType::WeaponExplosion,
            min_dynamic_particle_skip_priority: ParticlePriorityType::WeaponExplosion,
        }
    }

    /// Initialize the manager (clear all existing data)
    pub fn init(&mut self) {
        self.static_game_lod_info = [
            StaticGameLODInfo::default(),
            StaticGameLODInfo::default(),
            StaticGameLODInfo::default(),
            StaticGameLODInfo::default(),
        ];
        self.dynamic_game_lod_info = [
            DynamicGameLODInfo::default(),
            DynamicGameLODInfo::default(),
            DynamicGameLODInfo::default(),
            DynamicGameLODInfo::default(),
        ];
        self.lod_presets = Default::default();
        self.bench_profiles = Default::default();
        self.num_level_presets = [0; 3];
        self.num_bench_profiles = 0;
        self.really_low_mhz = 400;
        self.current_dynamic_lod = DynamicGameLODLevel::High;
        self.num_particle_generations = 0;
        self.dynamic_particle_skip_mask = 0;
        self.num_debris_generations = 0;
        self.dynamic_debris_skip_mask = 0;
        self.slow_death_scale = 1.0;
        self.min_dynamic_particle_priority = ParticlePriorityType::WeaponExplosion;
        self.min_dynamic_particle_skip_priority = ParticlePriorityType::WeaponExplosion;
    }

    pub(crate) fn get_static_lod_level(&self) -> StaticGameLODLevel {
        self.current_static_lod
    }
    pub(crate) fn set_current_static_lod_level(&mut self, level: StaticGameLODLevel) {
        self.current_static_lod = level;
    }
    pub(crate) fn get_ideal_static_lod_level(&self) -> StaticGameLODLevel {
        self.ideal_static_lod
    }
    pub(crate) fn set_ideal_static_lod_level(&mut self, level: StaticGameLODLevel) {
        self.ideal_static_lod = level;
    }

    /// Get static LOD index from name
    /// Matches C++ GameLODManager::getStaticGameLODIndex
    pub fn get_static_game_lod_index(&self, name: &str) -> Option<usize> {
        StaticGameLODLevel::from_str(name).and_then(|level| level.to_index())
    }

    /// Get dynamic LOD index from name
    /// Matches C++ GameLODManager::getDynamicGameLODIndex
    pub fn get_dynamic_game_lod_index(&self, name: &str) -> Option<usize> {
        DynamicGameLODLevel::from_str(name).and_then(|level| level.to_index())
    }

    /// Walk `m_lodPresets[HIGH..LOW]` against CPU/MHz/video/RAM.
    /// Matches C++ `GameLODManager::findStaticLODLevel` hardware compare.
    pub fn match_static_lod_presets(
        &self,
        cpu_type: CpuType,
        cpu_mhz: i32,
        video: ChipsetType,
        ram_mb: i32,
    ) -> StaticGameLODLevel {
        const PROFILE_ERROR_LIMIT: f32 = 0.94;
        for level_index in (0..=2).rev() {
            let count = self.num_level_presets[level_index];
            for j in 0..count {
                let Some(preset) = self.lod_presets[level_index][j].as_ref() else {
                    continue;
                };
                let mhz_ok =
                    preset.mhz > 0 && (cpu_mhz as f32) / (preset.mhz as f32) >= PROFILE_ERROR_LIMIT;
                let ram_ok = preset.memory > 0
                    && (ram_mb as f32) / (preset.memory as f32) >= PROFILE_ERROR_LIMIT;
                let video_ok = (video as i32) >= (preset.video_type as i32);
                if cpu_type == preset.cpu_type && mhz_ok && video_ok && ram_ok {
                    return match level_index {
                        2 => StaticGameLODLevel::High,
                        1 => StaticGameLODLevel::Medium,
                        _ => StaticGameLODLevel::Low,
                    };
                }
            }
        }
        StaticGameLODLevel::Low
    }

    /// Add a new LOD preset
    /// Matches C++ GameLODManager::newLODPreset
    pub fn new_lod_preset(&mut self, level_index: usize) -> Option<&mut LODPresetInfo> {
        if level_index >= 3 {
            return None;
        }
        let count = self.num_level_presets[level_index];
        if count >= MAX_LOD_PRESETS_PER_LEVEL {
            return None;
        }
        self.num_level_presets[level_index] += 1;
        self.lod_presets[level_index][count] = Some(LODPresetInfo::default());
        self.lod_presets[level_index][count].as_mut()
    }

    /// Add a new benchmark profile
    /// Matches C++ GameLODManager::newBenchProfile
    pub fn new_bench_profile(&mut self) -> Option<&mut BenchProfile> {
        if self.num_bench_profiles >= MAX_BENCH_PROFILES {
            return None;
        }
        let count = self.num_bench_profiles;
        self.num_bench_profiles += 1;
        self.bench_profiles[count] = Some(BenchProfile::default());
        self.bench_profiles[count].as_mut()
    }

    /// Set the really low MHz threshold
    pub fn set_really_low_mhz(&mut self, mhz: i32) {
        self.really_low_mhz = mhz;
    }

    /// Current dynamic LOD level.
    /// Matches C++ `GameLODManager::getDynamicLODLevel`.
    pub fn get_dynamic_lod_level(&self) -> DynamicGameLODLevel {
        self.current_dynamic_lod
    }

    /// Set the current dynamic LOD level and apply skip masks / death scale.
    /// Matches C++ `GameLODManager::setDynamicLODLevel`.
    pub fn set_dynamic_lod_level(&mut self, level: DynamicGameLODLevel) -> bool {
        if level == DynamicGameLODLevel::Unknown || self.current_dynamic_lod == level {
            return false;
        }
        self.current_dynamic_lod = level;
        self.apply_dynamic_lod_level(level);
        true
    }

    /// Copy skip masks and related runtime fields from the selected DynamicGameLODInfo.
    /// Matches C++ `GameLODManager::applyDynamicLODLevel`.
    fn apply_dynamic_lod_level(&mut self, level: DynamicGameLODLevel) {
        let Some(index) = level.to_index() else {
            return;
        };
        let info = self.dynamic_game_lod_info[index].clone();

        self.num_particle_generations = 0;
        self.dynamic_particle_skip_mask = info.dynamic_particle_skip_mask as i32;

        self.num_debris_generations = 0;
        self.dynamic_debris_skip_mask = info.dynamic_debris_skip_mask as i32;

        self.slow_death_scale = info.slow_death_scale;
        self.min_dynamic_particle_priority = info.min_dynamic_particle_priority;
        self.min_dynamic_particle_skip_priority = info.min_dynamic_particle_skip_priority;
    }

    /// Skip every Nth particle based on the current dynamic LOD mask.
    /// Matches C++ `GameLODManager::isParticleSkipped`:
    /// `return (++m_numParticleGenerations & m_dynamicParticleSkipMask) != m_dynamicParticleSkipMask;`
    ///
    /// Mask 0 never skips (`(++n & 0) != 0` is always false).
    pub fn is_particle_skipped(&mut self) -> bool {
        self.num_particle_generations = self.num_particle_generations.wrapping_add(1);
        (self.num_particle_generations & self.dynamic_particle_skip_mask)
            != self.dynamic_particle_skip_mask
    }

    /// Skip every Nth debris based on the current dynamic LOD mask.
    /// Matches C++ `GameLODManager::isDebrisSkipped`:
    /// `return (++m_numDebrisGenerations & m_dynamicDebrisSkipMask) != m_dynamicDebrisSkipMask;`
    ///
    /// Mask 0 never skips (`(++n & 0) != 0` is always false).
    pub fn is_debris_skipped(&mut self) -> bool {
        self.num_debris_generations = self.num_debris_generations.wrapping_add(1);
        (self.num_debris_generations & self.dynamic_debris_skip_mask)
            != self.dynamic_debris_skip_mask
    }

    /// Runtime debris skip mask (C++ `m_dynamicDebrisSkipMask`).
    pub fn dynamic_debris_skip_mask(&self) -> i32 {
        self.dynamic_debris_skip_mask
    }

    /// Set the runtime debris skip mask and reset the generation counter.
    /// Tests / OCL skip paths use this to force `isDebrisSkipped` without a full LOD apply.
    pub fn set_dynamic_debris_skip_mask(&mut self, mask: i32) {
        self.dynamic_debris_skip_mask = mask;
        self.num_debris_generations = 0;
    }

    /// Matches C++ `GameLODManager::getSlowDeathScale`.
    pub fn get_slow_death_scale(&self) -> f32 {
        self.slow_death_scale
    }

    /// Matches C++ `GameLODManager::getMinDynamicParticlePriority`.
    pub fn get_min_dynamic_particle_priority(&self) -> ParticlePriorityType {
        self.min_dynamic_particle_priority
    }

    /// Matches C++ `GameLODManager::getMinDynamicParticleSkipPriority`.
    pub fn get_min_dynamic_particle_skip_priority(&self) -> ParticlePriorityType {
        self.min_dynamic_particle_skip_priority
    }

    /// Parse static game LOD definition
    /// Matches C++ INI::parseStaticGameLODDefinition
    pub fn parse_static_game_lod_definition(&mut self, ini: &mut INI) -> INIResult<()> {
        // Read the level name
        let name = ini.get_next_value_token().ok_or(INIError::InvalidData)?;

        let index = self
            .get_static_game_lod_index(&name)
            .ok_or(INIError::InvalidData)?;
        let lod_info = &mut self.static_game_lod_info[index];

        // Parse fields until End
        loop {
            ini.read_line()?;
            let tokens = ini.get_line_tokens();
            if tokens.iter().any(|token| token.eq_ignore_ascii_case("End")) {
                break;
            }
            if ini.is_eof() {
                return Err(INIError::MissingEndToken);
            }
            if tokens.is_empty() {
                continue;
            }
            let (key, value_str) = ini_field_key_value(&tokens);

            // Parse fields based on C++ TheStaticGameLODFieldParseTable
            match key.to_ascii_lowercase().as_str() {
                "minimumfps" => {
                    lod_info.min_fps = INI::parse_int(value_str)?;
                }
                "minimumprocessorfps" => {
                    lod_info.min_processor_fps = INI::parse_int(value_str)?;
                }
                "samplecount2d" => {
                    lod_info.sample_count_2d = INI::parse_int(value_str)?;
                }
                "samplecount3d" => {
                    lod_info.sample_count_3d = INI::parse_int(value_str)?;
                }
                "streamcount" => {
                    lod_info.stream_count = INI::parse_int(value_str)?;
                }
                "maxparticlecount" => {
                    lod_info.max_particle_count = INI::parse_int(value_str)?;
                }
                "useshadowvolumes" => {
                    lod_info.use_shadow_volumes = parse_cpp_yes_no_bool(value_str)?;
                }
                "useshadowdecals" => {
                    lod_info.use_shadow_decals = parse_cpp_yes_no_bool(value_str)?;
                }
                "usecloudmap" => {
                    lod_info.use_cloud_map = parse_cpp_yes_no_bool(value_str)?;
                }
                "uselightmap" => {
                    lod_info.use_light_map = parse_cpp_yes_no_bool(value_str)?;
                }
                "showsoftwateredge" => {
                    lod_info.show_soft_water_edge = parse_cpp_yes_no_bool(value_str)?;
                }
                "maxtanktrackedges" => {
                    lod_info.max_tank_track_edges = INI::parse_int(value_str)?;
                }
                "maxtanktrackopaqueedges" => {
                    lod_info.max_tank_track_opaque_edges = INI::parse_int(value_str)?;
                }
                "maxtanktrackfadedelay" => {
                    lod_info.max_tank_track_fade_delay = INI::parse_int(value_str)?;
                }
                "usebuildupscaffolds" => {
                    lod_info.use_buildup_scaffolds = parse_cpp_yes_no_bool(value_str)?;
                }
                "usetreesway" => {
                    lod_info.use_tree_sway = parse_cpp_yes_no_bool(value_str)?;
                }
                "useemissivenightmaterials" => {
                    lod_info.use_emissive_night_materials = parse_cpp_yes_no_bool(value_str)?;
                }
                "useheateffects" => {
                    lod_info.use_heat_effects = parse_cpp_yes_no_bool(value_str)?;
                }
                "texturereductionfactor" => {
                    lod_info.texture_reduction = INI::parse_int(value_str)?;
                }
                _ => {
                    // Unknown field - silently ignore like C++
                }
            }
        }

        Ok(())
    }

    /// Parse dynamic game LOD definition
    /// Matches C++ INI::parseDynamicGameLODDefinition
    pub fn parse_dynamic_game_lod_definition(&mut self, ini: &mut INI) -> INIResult<()> {
        // Read the level name
        let name = ini.get_next_value_token().ok_or(INIError::InvalidData)?;

        let index = self
            .get_dynamic_game_lod_index(&name)
            .ok_or(INIError::InvalidData)?;
        let lod_info = &mut self.dynamic_game_lod_info[index];

        // Parse fields until End. The retail file's last End has no newline,
        // so EOF is set on that same read; recognize End before treating EOF
        // as a missing terminator. C++ also splits `Key=Value`.
        loop {
            ini.read_line()?;
            let tokens = ini.get_line_tokens();
            if tokens.iter().any(|token| token.eq_ignore_ascii_case("End")) {
                break;
            }
            if ini.is_eof() {
                return Err(INIError::MissingEndToken);
            }
            if tokens.is_empty() {
                continue;
            }
            let (key, value_str) = ini_field_key_value(&tokens);

            // Parse fields based on C++ TheDynamicGameLODFieldParseTable
            match key.to_ascii_lowercase().as_str() {
                "minimumfps" => {
                    lod_info.min_fps = INI::parse_int(value_str)?;
                }
                "particleskipmask" => {
                    lod_info.dynamic_particle_skip_mask = parse_cpp_int_bits_as_u32(value_str)?;
                }
                "debrisskipmask" => {
                    lod_info.dynamic_debris_skip_mask = parse_cpp_int_bits_as_u32(value_str)?;
                }
                "slowdeathscale" => {
                    lod_info.slow_death_scale = INI::parse_real(value_str)?;
                }
                "minparticlepriority" => {
                    lod_info.min_dynamic_particle_priority =
                        ParticlePriorityType::parse_cpp_index_list_token(value_str)
                            .ok_or(INIError::InvalidData)?;
                }
                "minparticleskippriority" => {
                    lod_info.min_dynamic_particle_skip_priority =
                        ParticlePriorityType::parse_cpp_index_list_token(value_str)
                            .ok_or(INIError::InvalidData)?;
                }
                _ => {
                    // Unknown field - silently ignore like C++
                }
            }
        }

        Ok(())
    }

    /// Parse LOD preset definition
    /// Matches C++ INI::parseLODPreset
    pub fn parse_lod_preset(&mut self, ini: &mut INI) -> INIResult<()> {
        // Format: LODPreset = LEVEL CPU_TYPE MHZ VIDEO_TYPE MEMORY
        // Example: LODPreset = MEDIUM P4 1500 GF4 256

        // Read the level name
        let level_name = ini.get_next_value_token().ok_or(INIError::InvalidData)?;
        let level_index = self
            .get_static_game_lod_index(&level_name)
            .ok_or(INIError::InvalidData)?;

        // Get or create the preset
        let preset = self
            .new_lod_preset(level_index)
            .ok_or(INIError::InvalidData)?;

        // Read the remaining values
        let cpu_type_str = ini.get_next_value_token().ok_or(INIError::InvalidData)?;
        let mhz_str = ini.get_next_value_token().ok_or(INIError::InvalidData)?;
        let video_type_str = ini.get_next_value_token().ok_or(INIError::InvalidData)?;
        let memory_str = ini.get_next_value_token().ok_or(INIError::InvalidData)?;

        preset.cpu_type = CpuType::from_str(&cpu_type_str).ok_or(INIError::InvalidData)?;
        preset.mhz = INI::parse_int(&mhz_str)?;
        preset.video_type = ChipsetType::from_str(&video_type_str).ok_or(INIError::InvalidData)?;
        preset.memory = INI::parse_int(&memory_str)?;

        Ok(())
    }

    /// Parse benchmark profile definition
    /// Matches C++ INI::parseBenchProfile
    pub fn parse_bench_profile(&mut self, ini: &mut INI) -> INIResult<()> {
        // Format: BenchProfile = CPU_TYPE MHZ INT_BENCH FLOAT_BENCH MEM_BENCH
        // Example: BenchProfile = P4 2189 6.108187 15.113676 9.402223

        // Get or create the profile
        let profile = self.new_bench_profile().ok_or(INIError::InvalidData)?;

        // Read the values
        let cpu_type_str = ini.get_next_value_token().ok_or(INIError::InvalidData)?;
        let mhz_str = ini.get_next_value_token().ok_or(INIError::InvalidData)?;
        let int_bench_str = ini.get_next_value_token().ok_or(INIError::InvalidData)?;
        let float_bench_str = ini.get_next_value_token().ok_or(INIError::InvalidData)?;
        let mem_bench_str = ini.get_next_value_token().ok_or(INIError::InvalidData)?;

        profile.cpu_type = CpuType::from_str(&cpu_type_str).ok_or(INIError::InvalidData)?;
        profile.mhz = INI::parse_int(&mhz_str)?;
        profile.int_bench_index = INI::parse_real(&int_bench_str)?;
        profile.float_bench_index = INI::parse_real(&float_bench_str)?;
        profile.mem_bench_index = INI::parse_real(&mem_bench_str)?;

        Ok(())
    }
}

// ============================================================================
// Global Instance Management
// ============================================================================

/// Global Game LOD Manager instance
static GAME_LOD_MANAGER: OnceCell<RwLock<GameLODManager>> = OnceCell::new();

/// Get the global Game LOD Manager (read access)
pub fn get_game_lod_manager() -> RwLockReadGuard<'static, GameLODManager> {
    GAME_LOD_MANAGER
        .get_or_init(|| RwLock::new(GameLODManager::new()))
        .read()
        .unwrap()
}

/// Get the global Game LOD Manager (write access)
pub fn get_game_lod_manager_mut() -> RwLockWriteGuard<'static, GameLODManager> {
    GAME_LOD_MANAGER
        .get_or_init(|| RwLock::new(GameLODManager::new()))
        .write()
        .unwrap()
}

/// Initialize the global Game LOD Manager
pub fn init_game_lod_manager() {
    if GAME_LOD_MANAGER.get().is_none() {
        let _ = GAME_LOD_MANAGER.set(RwLock::new(GameLODManager::new()));
    } else if let Some(manager) = GAME_LOD_MANAGER.get() {
        if let Ok(mut guard) = manager.write() {
            guard.init();
        }
    }
}

/// C++ `TheGameLODManager->isParticleSkipped()`.
pub fn is_particle_skipped() -> bool {
    get_game_lod_manager_mut().is_particle_skipped()
}

/// C++ `TheGameLODManager->isDebrisSkipped()`.
/// OCL generic-debris spawn should skip when this returns true.
pub fn is_debris_skipped() -> bool {
    get_game_lod_manager_mut().is_debris_skipped()
}

/// Set the runtime debris skip mask (and reset the generation counter).
pub fn set_dynamic_debris_skip_mask(mask: i32) {
    get_game_lod_manager_mut().set_dynamic_debris_skip_mask(mask);
}

/// C++ `TheGameLODManager->setDynamicLODLevel(level)`.
pub fn set_dynamic_lod_level(level: DynamicGameLODLevel) -> bool {
    get_game_lod_manager_mut().set_dynamic_lod_level(level)
}

/// C++ `TheGameLODManager->getDynamicLODLevel()`.
pub fn get_dynamic_lod_level() -> DynamicGameLODLevel {
    get_game_lod_manager().get_dynamic_lod_level()
}

// ============================================================================
// INI Parser Functions
// ============================================================================

/// Parse StaticGameLOD block from INI
/// This is the main entry point for the INI parser
/// Matches C++ INI::parseStaticGameLODDefinition
pub fn parse_static_game_lod_definition(ini: &mut INI) -> Result<(), String> {
    let mut manager = get_game_lod_manager_mut();
    manager
        .parse_static_game_lod_definition(ini)
        .map_err(|e| format!("StaticGameLOD parse error: {:?}", e))
}

/// Parse DynamicGameLOD block from INI
/// This is the main entry point for the INI parser
/// Matches C++ INI::parseDynamicGameLODDefinition
pub fn parse_dynamic_game_lod_definition(ini: &mut INI) -> Result<(), String> {
    let mut manager = get_game_lod_manager_mut();
    manager
        .parse_dynamic_game_lod_definition(ini)
        .map_err(|e| format!("DynamicGameLOD parse error: {:?}", e))
}

/// Parse LODPreset line from INI
/// This is the main entry point for the INI parser
/// Matches C++ INI::parseLODPreset
pub fn parse_lod_preset(ini: &mut INI) -> Result<(), String> {
    let mut manager = get_game_lod_manager_mut();
    manager
        .parse_lod_preset(ini)
        .map_err(|e| format!("LODPreset parse error: {:?}", e))
}

/// Parse BenchProfile line from INI
/// This is the main entry point for the INI parser
/// Matches C++ INI::parseBenchProfile
pub fn parse_bench_profile(ini: &mut INI) -> Result<(), String> {
    let mut manager = get_game_lod_manager_mut();
    manager
        .parse_bench_profile(ini)
        .map_err(|e| format!("BenchProfile parse error: {:?}", e))
}

/// Parse ReallyLowMHz line from INI
/// Matches C++ parseReallyLowMHz
pub fn parse_really_low_mhz(ini: &mut INI) -> Result<(), String> {
    let mhz_str = ini.get_next_value_token().ok_or("Missing MHz value")?;
    let mhz: i32 = INI::parse_int(&mhz_str).map_err(|e| format!("Invalid MHz: {:?}", e))?;

    let mut manager = get_game_lod_manager_mut();
    manager.set_really_low_mhz(mhz);

    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[path = "ini_game_lod/tests.rs"]
mod tests;
