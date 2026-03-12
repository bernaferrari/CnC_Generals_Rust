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

use crate::common::ini::{INIError, INIResult, INI};

// ============================================================================
// Constants
// ============================================================================

/// Maximum number of LOD presets per level
pub const MAX_LOD_PRESETS_PER_LEVEL: usize = 32;

/// Maximum number of benchmark profiles
pub const MAX_BENCH_PROFILES: usize = 16;

// ============================================================================
// Enumerations
// ============================================================================

/// Static game LOD levels
/// Must stay in sync with StaticGameLODNames
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum StaticGameLODLevel {
    Unknown = -1,
    #[default]
    Low = 0,
    Medium = 1,
    High = 2,
    Custom = 3,
    Count = 4,
}

impl StaticGameLODLevel {
    /// Convert from string name
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "low" => Some(StaticGameLODLevel::Low),
            "medium" => Some(StaticGameLODLevel::Medium),
            "high" => Some(StaticGameLODLevel::High),
            "custom" => Some(StaticGameLODLevel::Custom),
            "unknown" => Some(StaticGameLODLevel::Unknown),
            _ => None,
        }
    }

    /// Get string name
    pub fn to_str(self) -> &'static str {
        match self {
            StaticGameLODLevel::Unknown => "Unknown",
            StaticGameLODLevel::Low => "Low",
            StaticGameLODLevel::Medium => "Medium",
            StaticGameLODLevel::High => "High",
            StaticGameLODLevel::Custom => "Custom",
            StaticGameLODLevel::Count => "Count",
        }
    }

    /// Convert to index for array access (returns None for Unknown or Count)
    pub fn to_index(self) -> Option<usize> {
        match self {
            StaticGameLODLevel::Low => Some(0),
            StaticGameLODLevel::Medium => Some(1),
            StaticGameLODLevel::High => Some(2),
            StaticGameLODLevel::Custom => Some(3),
            _ => None,
        }
    }
}

/// Dynamic game LOD levels
/// Must stay in sync with DynamicGameLODNames
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum DynamicGameLODLevel {
    Unknown = -1,
    Low = 0,
    Medium = 1,
    #[default]
    High = 2,
    VeryHigh = 3,
    Count = 4,
}

impl DynamicGameLODLevel {
    /// Convert from string name
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "low" => Some(DynamicGameLODLevel::Low),
            "medium" => Some(DynamicGameLODLevel::Medium),
            "high" => Some(DynamicGameLODLevel::High),
            "veryhigh" | "very high" | "very_high" => Some(DynamicGameLODLevel::VeryHigh),
            "unknown" => Some(DynamicGameLODLevel::Unknown),
            _ => None,
        }
    }

    /// Get string name
    pub fn to_str(self) -> &'static str {
        match self {
            DynamicGameLODLevel::Unknown => "Unknown",
            DynamicGameLODLevel::Low => "Low",
            DynamicGameLODLevel::Medium => "Medium",
            DynamicGameLODLevel::High => "High",
            DynamicGameLODLevel::VeryHigh => "VeryHigh",
            DynamicGameLODLevel::Count => "Count",
        }
    }

    /// Convert to index for array access (returns None for Unknown or Count)
    pub fn to_index(self) -> Option<usize> {
        match self {
            DynamicGameLODLevel::Low => Some(0),
            DynamicGameLODLevel::Medium => Some(1),
            DynamicGameLODLevel::High => Some(2),
            DynamicGameLODLevel::VeryHigh => Some(3),
            _ => None,
        }
    }
}

/// CPU types
/// Must stay in sync with CPUNames in GameLOD.cpp
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum CpuType {
    #[default]
    Unknown = 0, // XX in C++
    P3 = 1,
    P4 = 2,
    K7 = 3,
}

impl CpuType {
    /// Convert from string name
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "xx" | "unknown" => Some(CpuType::Unknown),
            "p3" => Some(CpuType::P3),
            "p4" => Some(CpuType::P4),
            "k7" => Some(CpuType::K7),
            _ => None,
        }
    }

    /// Get string name
    pub fn to_str(self) -> &'static str {
        match self {
            CpuType::Unknown => "XX",
            CpuType::P3 => "P3",
            CpuType::P4 => "P4",
            CpuType::K7 => "K7",
        }
    }
}

/// Chipset/Video types
/// Must stay in sync with VideoNames in GameLOD.cpp
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum ChipsetType {
    #[default]
    Unknown = 0,
    Voodoo2 = 1,
    Voodoo3 = 2,
    Voodoo4 = 3,
    Voodoo5 = 4,
    TNT = 5,
    TNT2 = 6,
    GeForce2 = 7,
    Radeon = 8, // R100
    GenericPS11 = 9,
    GeForce3 = 10,
    GeForce4 = 11,
    GenericPS14 = 12,
    Radeon8500 = 13, // R200
    GenericPS20 = 14,
    Radeon9700 = 15, // R300
    Max = 16,
}

impl ChipsetType {
    /// Convert from string name
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "xx" | "unknown" => Some(ChipsetType::Unknown),
            "v2" => Some(ChipsetType::Voodoo2),
            "v3" => Some(ChipsetType::Voodoo3),
            "v4" => Some(ChipsetType::Voodoo4),
            "v5" => Some(ChipsetType::Voodoo5),
            "tnt" => Some(ChipsetType::TNT),
            "tnt2" => Some(ChipsetType::TNT2),
            "gf2" => Some(ChipsetType::GeForce2),
            "r100" => Some(ChipsetType::Radeon),
            "ps11" => Some(ChipsetType::GenericPS11),
            "gf3" => Some(ChipsetType::GeForce3),
            "gf4" => Some(ChipsetType::GeForce4),
            "ps14" => Some(ChipsetType::GenericPS14),
            "r200" => Some(ChipsetType::Radeon8500),
            "ps20" => Some(ChipsetType::GenericPS20),
            "r300" => Some(ChipsetType::Radeon9700),
            _ => None,
        }
    }

    /// Get string name
    pub fn to_str(self) -> &'static str {
        match self {
            ChipsetType::Unknown => "XX",
            ChipsetType::Voodoo2 => "V2",
            ChipsetType::Voodoo3 => "V3",
            ChipsetType::Voodoo4 => "V4",
            ChipsetType::Voodoo5 => "V5",
            ChipsetType::TNT => "TNT",
            ChipsetType::TNT2 => "TNT2",
            ChipsetType::GeForce2 => "GF2",
            ChipsetType::Radeon => "R100",
            ChipsetType::GenericPS11 => "PS11",
            ChipsetType::GeForce3 => "GF3",
            ChipsetType::GeForce4 => "GF4",
            ChipsetType::GenericPS14 => "PS14",
            ChipsetType::Radeon8500 => "R200",
            ChipsetType::GenericPS20 => "PS20",
            ChipsetType::Radeon9700 => "R300",
            ChipsetType::Max => "MAX",
        }
    }
}

/// Particle priority type
/// Must stay in sync with ParticlePriorityNames from ParticleSys.h
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum ParticlePriorityType {
    #[default]
    Lowest = 0,
    AreaEffect = 1,
    DustCloud = 2,
    Debris = 3,
    Scorch = 4,
    Smoke = 5,
    DustTrail = 6,
    WeaponExplosion = 7,
    Constant = 8,
    BuildingExplosion = 9,
    UnitWeaponTrail = 10,
    Critical = 11,
}

impl ParticlePriorityType {
    /// Convert from string name
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "lowest" | "particle_priority_lowest" => Some(ParticlePriorityType::Lowest),
            "areaeffect" | "area_effect" | "particle_priority_area_effect" => {
                Some(ParticlePriorityType::AreaEffect)
            }
            "dustcloud" | "dust_cloud" | "particle_priority_dust_cloud" => {
                Some(ParticlePriorityType::DustCloud)
            }
            "debris" | "particle_priority_debris" => Some(ParticlePriorityType::Debris),
            "scorch" | "particle_priority_scorch" => Some(ParticlePriorityType::Scorch),
            "smoke" | "particle_priority_smoke" => Some(ParticlePriorityType::Smoke),
            "dusttrail" | "dust_trail" | "particle_priority_dust_trail" => {
                Some(ParticlePriorityType::DustTrail)
            }
            "weaponexplosion" | "weapon_explosion" | "particle_priority_weapon_explosion" => {
                Some(ParticlePriorityType::WeaponExplosion)
            }
            "constant" | "particle_priority_constant" => Some(ParticlePriorityType::Constant),
            "buildingexplosion" | "building_explosion" | "particle_priority_building_explosion" => {
                Some(ParticlePriorityType::BuildingExplosion)
            }
            "unitweapntrail" | "unit_weapon_trail" | "particle_priority_unit_weapon_trail" => {
                Some(ParticlePriorityType::UnitWeaponTrail)
            }
            "critical" | "particle_priority_critical" => Some(ParticlePriorityType::Critical),
            _ => None,
        }
    }

    /// Get string name
    pub fn to_str(self) -> &'static str {
        match self {
            ParticlePriorityType::Lowest => "PARTICLE_PRIORITY_LOWEST",
            ParticlePriorityType::AreaEffect => "PARTICLE_PRIORITY_AREA_EFFECT",
            ParticlePriorityType::DustCloud => "PARTICLE_PRIORITY_DUST_CLOUD",
            ParticlePriorityType::Debris => "PARTICLE_PRIORITY_DEBRIS",
            ParticlePriorityType::Scorch => "PARTICLE_PRIORITY_SCORCH",
            ParticlePriorityType::Smoke => "PARTICLE_PRIORITY_SMOKE",
            ParticlePriorityType::DustTrail => "PARTICLE_PRIORITY_DUST_TRAIL",
            ParticlePriorityType::WeaponExplosion => "PARTICLE_PRIORITY_WEAPON_EXPLOSION",
            ParticlePriorityType::Constant => "PARTICLE_PRIORITY_CONSTANT",
            ParticlePriorityType::BuildingExplosion => "PARTICLE_PRIORITY_BUILDING_EXPLOSION",
            ParticlePriorityType::UnitWeaponTrail => "PARTICLE_PRIORITY_UNIT_WEAPON_TRAIL",
            ParticlePriorityType::Critical => "PARTICLE_PRIORITY_CRITICAL",
        }
    }
}

// ============================================================================
// Data Structures
// ============================================================================

/// Static game LOD configuration
/// Matches C++ StaticGameLODInfo from GameLOD.h lines 66-87
///
/// # C++ Definition
/// ```cpp
/// struct StaticGameLODInfo
/// {
///     Int m_minFPS;
///     Int m_minProcessorFPS;
///     Int m_sampleCount2D;
///     Int m_sampleCount3D;
///     Int m_streamCount;
///     Int m_maxParticleCount;
///     Bool m_useShadowVolumes;
///     Bool m_useShadowDecals;
///     Bool m_useCloudMap;
///     Bool m_useLightMap;
///     Bool m_showSoftWaterEdge;
///     Int m_maxTankTrackEdges;
///     Int m_maxTankTrackOpaqueEdges;
///     Int m_maxTankTrackFadeDelay;
///     Bool m_useBuildupScaffolds;
///     Bool m_useTreeSway;
///     Bool m_useEmissiveNightMaterials;
///     Bool m_useHeatEffects;
///     Int m_textureReduction;
///     Bool m_useFpsLimit;
///     Bool m_enableDynamicLOD;
///     Bool m_useTrees;
/// };
/// ```
#[derive(Debug, Clone)]
pub struct StaticGameLODInfo {
    /// Minimum fps in order to recommend this LOD
    pub min_fps: i32,
    /// Minimum CPU time (in ms) to recommend this LOD
    pub min_processor_fps: i32,
    /// How many 2-D (UI) samples to allow simultaneously
    pub sample_count_2d: i32,
    /// How many 3-D (World) samples to allow simultaneously
    pub sample_count_3d: i32,
    /// How many streaming audio things to allow simultaneously
    pub stream_count: i32,
    /// Maximum number of particles that can exist
    pub max_particle_count: i32,
    /// Use volumetric shadows if available
    pub use_shadow_volumes: bool,
    /// Use 2D Decal shadows
    pub use_shadow_decals: bool,
    /// Use cloud shadows scrolling over terrain
    pub use_cloud_map: bool,
    /// Use noise pattern over terrain to break up tiling
    pub use_light_map: bool,
    /// Feather water edge if supported by hardware
    pub show_soft_water_edge: bool,
    /// Maximum length of tank track
    pub max_tank_track_edges: i32,
    /// Maximum length of tank track before it starts fading
    pub max_tank_track_opaque_edges: i32,
    /// Maximum amount of time a tank track segment remains visible (in ms)
    pub max_tank_track_fade_delay: i32,
    /// Draw scaffold during structure building
    pub use_buildup_scaffolds: bool,
    /// Sway trees to simulate wind
    pub use_tree_sway: bool,
    /// Perform second lighting pass on night buildings
    pub use_emissive_night_materials: bool,
    /// Draw heat distortion effects
    pub use_heat_effects: bool,
    /// Reduce texture resolution by dividing in half n times
    pub texture_reduction: i32,
    /// Don't lock fps to 30hz
    pub use_fps_limit: bool,
    /// Don't do dynamic lod based on current fps
    pub enable_dynamic_lod: bool,
    /// Include trees on map
    pub use_trees: bool,
}

impl Default for StaticGameLODInfo {
    /// Default values match C++ StaticGameLODInfo constructor
    fn default() -> Self {
        Self {
            min_fps: 0,
            min_processor_fps: 0,
            sample_count_2d: 6,
            sample_count_3d: 24,
            stream_count: 2,
            max_particle_count: 2500,
            use_shadow_volumes: true,
            use_shadow_decals: true,
            use_cloud_map: true,
            use_light_map: true,
            show_soft_water_edge: true,
            max_tank_track_edges: 100,
            max_tank_track_opaque_edges: 25,
            max_tank_track_fade_delay: 300000,
            use_buildup_scaffolds: true,
            use_tree_sway: true,
            use_emissive_night_materials: true,
            use_heat_effects: true,
            texture_reduction: 0,
            use_fps_limit: true,
            enable_dynamic_lod: true,
            use_trees: true,
        }
    }
}

/// Dynamic game LOD configuration
/// Matches C++ DynamicGameLODInfo from GameLOD.h lines 89-97
///
/// # C++ Definition
/// ```cpp
/// struct DynamicGameLODInfo
/// {
///     Int m_minFPS;
///     UnsignedInt m_dynamicParticleSkipMask;
///     UnsignedInt m_dynamicDebrisSkipMask;
///     Real m_slowDeathScale;
///     ParticlePriorityType m_minDynamicParticlePriority;
///     ParticlePriorityType m_minDynamicParticleSkipPriority;
/// };
/// ```
#[derive(Debug, Clone)]
pub struct DynamicGameLODInfo {
    /// Minimum fps in order to recommend this LOD
    pub min_fps: i32,
    /// Mask used to enable rendering of every Nth particle
    pub dynamic_particle_skip_mask: u32,
    /// Mask used to enable rendering of every Nth debris
    pub dynamic_debris_skip_mask: u32,
    /// Values < 1.0f are used to accelerate deaths
    pub slow_death_scale: f32,
    /// Only priorities above/including this value are allowed to render
    pub min_dynamic_particle_priority: ParticlePriorityType,
    /// Priorities above/including this value never skip particles
    pub min_dynamic_particle_skip_priority: ParticlePriorityType,
}

impl Default for DynamicGameLODInfo {
    /// Default values match C++ DynamicGameLODInfo constructor
    fn default() -> Self {
        Self {
            min_fps: 0,
            dynamic_particle_skip_mask: 0,
            dynamic_debris_skip_mask: 0,
            slow_death_scale: 1.0,
            min_dynamic_particle_priority: ParticlePriorityType::Lowest,
            min_dynamic_particle_skip_priority: ParticlePriorityType::Lowest,
        }
    }
}

/// LOD preset configuration for hardware detection
/// Matches C++ LODPresetInfo from GameLOD.h lines 99-106
///
/// # C++ Definition
/// ```cpp
/// struct LODPresetInfo
/// {
///     CpuType  m_cpuType;
///     Int      m_mhz;
///     Real     m_cpuPerfIndex;
///     ChipsetType m_videoType;
///     Int      m_memory;
/// };
/// ```
#[derive(Debug, Clone)]
pub struct LODPresetInfo {
    /// CPU type (P3, P4, K7, or Unknown)
    pub cpu_type: CpuType,
    /// CPU frequency in MHz
    pub mhz: i32,
    /// Performance index for selecting preset for unidentified CPUs
    pub cpu_perf_index: f32,
    /// Video chipset type
    pub video_type: ChipsetType,
    /// Amount of video memory in MB
    pub memory: i32,
}

impl Default for LODPresetInfo {
    fn default() -> Self {
        Self {
            cpu_type: CpuType::Unknown,
            mhz: 1,
            cpu_perf_index: 1.0,
            video_type: ChipsetType::Unknown,
            memory: 1,
        }
    }
}

/// Benchmark profile for hardware classification
/// Matches C++ BenchProfile from GameLOD.h lines 108-116
///
/// # C++ Definition
/// ```cpp
/// struct BenchProfile
/// {
///     CpuType  m_cpuType;
///     Int      m_mhz;
///     Real     m_intBenchIndex;
///     Real     m_floatBenchIndex;
///     Real     m_memBenchIndex;
/// };
/// ```
#[derive(Debug, Clone)]
pub struct BenchProfile {
    /// CPU type (P3, P4, K7, or Unknown)
    pub cpu_type: CpuType,
    /// CPU frequency in MHz
    pub mhz: i32,
    /// Integer benchmark performance index
    pub int_bench_index: f32,
    /// Floating-point benchmark performance index
    pub float_bench_index: f32,
    /// Memory benchmark performance index
    pub mem_bench_index: f32,
}

impl Default for BenchProfile {
    fn default() -> Self {
        Self {
            cpu_type: CpuType::Unknown,
            mhz: 1,
            int_bench_index: 1.0,
            float_bench_index: 1.0,
            mem_bench_index: 1.0,
        }
    }
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
            if ini.is_eof() {
                return Err(INIError::MissingEndToken);
            }

            let tokens = ini.get_line_tokens();
            if tokens.is_empty() {
                continue;
            }

            let key = tokens[0];
            if key.eq_ignore_ascii_case("End") {
                break;
            }

            // Get value tokens (skip key and '=' signs)
            let mut value_tokens: Vec<&str> = tokens.iter().skip(1).copied().collect();
            value_tokens.retain(|t| *t != "=");
            let value_str = value_tokens.first().copied().unwrap_or("");

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
                    lod_info.use_shadow_volumes = INI::parse_bool(value_str)?;
                }
                "useshadowdecals" => {
                    lod_info.use_shadow_decals = INI::parse_bool(value_str)?;
                }
                "usecloudmap" => {
                    lod_info.use_cloud_map = INI::parse_bool(value_str)?;
                }
                "uselightmap" => {
                    lod_info.use_light_map = INI::parse_bool(value_str)?;
                }
                "showsoftwateredge" => {
                    lod_info.show_soft_water_edge = INI::parse_bool(value_str)?;
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
                    lod_info.use_buildup_scaffolds = INI::parse_bool(value_str)?;
                }
                "usetreesway" => {
                    lod_info.use_tree_sway = INI::parse_bool(value_str)?;
                }
                "useemissivenightmaterials" => {
                    lod_info.use_emissive_night_materials = INI::parse_bool(value_str)?;
                }
                "useheateffects" => {
                    lod_info.use_heat_effects = INI::parse_bool(value_str)?;
                }
                "texturereductionfactor" => {
                    lod_info.texture_reduction = INI::parse_int(value_str)?;
                }
                "usefpslimit" => {
                    lod_info.use_fps_limit = INI::parse_bool(value_str)?;
                }
                "enabledynamiclod" => {
                    lod_info.enable_dynamic_lod = INI::parse_bool(value_str)?;
                }
                "usetrees" => {
                    lod_info.use_trees = INI::parse_bool(value_str)?;
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

        // Parse fields until End
        loop {
            ini.read_line()?;
            if ini.is_eof() {
                return Err(INIError::MissingEndToken);
            }

            let tokens = ini.get_line_tokens();
            if tokens.is_empty() {
                continue;
            }

            let key = tokens[0];
            if key.eq_ignore_ascii_case("End") {
                break;
            }

            // Get value tokens (skip key and '=' signs)
            let mut value_tokens: Vec<&str> = tokens.iter().skip(1).copied().collect();
            value_tokens.retain(|t| *t != "=");
            let value_str = value_tokens.first().copied().unwrap_or("");

            // Parse fields based on C++ TheDynamicGameLODFieldParseTable
            match key.to_ascii_lowercase().as_str() {
                "minimumfps" => {
                    lod_info.min_fps = INI::parse_int(value_str)?;
                }
                "particleskipmask" => {
                    lod_info.dynamic_particle_skip_mask = INI::parse_unsigned_int(value_str)?;
                }
                "debrisskipmask" => {
                    lod_info.dynamic_debris_skip_mask = INI::parse_unsigned_int(value_str)?;
                }
                "slowdeathscale" => {
                    lod_info.slow_death_scale = INI::parse_real(value_str)?;
                }
                "minparticlepriority" => {
                    lod_info.min_dynamic_particle_priority =
                        ParticlePriorityType::from_str(value_str).ok_or(INIError::InvalidData)?;
                }
                "minparticleskippriority" => {
                    lod_info.min_dynamic_particle_skip_priority =
                        ParticlePriorityType::from_str(value_str).ok_or(INIError::InvalidData)?;
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
mod tests {
    use super::*;

    #[test]
    fn test_static_lod_level_conversion() {
        assert_eq!(
            StaticGameLODLevel::from_str("Low"),
            Some(StaticGameLODLevel::Low)
        );
        assert_eq!(
            StaticGameLODLevel::from_str("Medium"),
            Some(StaticGameLODLevel::Medium)
        );
        assert_eq!(
            StaticGameLODLevel::from_str("High"),
            Some(StaticGameLODLevel::High)
        );
        assert_eq!(
            StaticGameLODLevel::from_str("Custom"),
            Some(StaticGameLODLevel::Custom)
        );
        assert_eq!(StaticGameLODLevel::from_str("Invalid"), None);

        assert_eq!(StaticGameLODLevel::Low.to_str(), "Low");
        assert_eq!(StaticGameLODLevel::Medium.to_str(), "Medium");
        assert_eq!(StaticGameLODLevel::High.to_str(), "High");
        assert_eq!(StaticGameLODLevel::Custom.to_str(), "Custom");
    }

    #[test]
    fn test_dynamic_lod_level_conversion() {
        assert_eq!(
            DynamicGameLODLevel::from_str("Low"),
            Some(DynamicGameLODLevel::Low)
        );
        assert_eq!(
            DynamicGameLODLevel::from_str("VeryHigh"),
            Some(DynamicGameLODLevel::VeryHigh)
        );
        assert_eq!(
            DynamicGameLODLevel::from_str("very high"),
            Some(DynamicGameLODLevel::VeryHigh)
        );
        assert_eq!(DynamicGameLODLevel::from_str("Invalid"), None);

        assert_eq!(DynamicGameLODLevel::VeryHigh.to_str(), "VeryHigh");
    }

    #[test]
    fn test_cpu_type_conversion() {
        assert_eq!(CpuType::from_str("P3"), Some(CpuType::P3));
        assert_eq!(CpuType::from_str("P4"), Some(CpuType::P4));
        assert_eq!(CpuType::from_str("K7"), Some(CpuType::K7));
        assert_eq!(CpuType::from_str("Invalid"), None);

        assert_eq!(CpuType::P3.to_str(), "P3");
        assert_eq!(CpuType::P4.to_str(), "P4");
        assert_eq!(CpuType::K7.to_str(), "K7");
    }

    #[test]
    fn test_chipset_type_conversion() {
        assert_eq!(ChipsetType::from_str("GF3"), Some(ChipsetType::GeForce3));
        assert_eq!(ChipsetType::from_str("GF4"), Some(ChipsetType::GeForce4));
        assert_eq!(ChipsetType::from_str("R300"), Some(ChipsetType::Radeon9700));
        assert_eq!(ChipsetType::from_str("Invalid"), None);

        assert_eq!(ChipsetType::GeForce3.to_str(), "GF3");
        assert_eq!(ChipsetType::Radeon9700.to_str(), "R300");
    }

    #[test]
    fn test_static_lod_info_defaults() {
        let info = StaticGameLODInfo::default();
        assert_eq!(info.min_fps, 0);
        assert_eq!(info.sample_count_2d, 6);
        assert_eq!(info.sample_count_3d, 24);
        assert_eq!(info.max_particle_count, 2500);
        assert!(info.use_shadow_volumes);
        assert!(info.use_shadow_decals);
        assert_eq!(info.texture_reduction, 0);
    }

    #[test]
    fn test_dynamic_lod_info_defaults() {
        let info = DynamicGameLODInfo::default();
        assert_eq!(info.min_fps, 0);
        assert_eq!(info.dynamic_particle_skip_mask, 0);
        assert_eq!(info.slow_death_scale, 1.0);
        assert_eq!(
            info.min_dynamic_particle_priority,
            ParticlePriorityType::Lowest
        );
    }

    #[test]
    fn test_game_lod_manager() {
        init_game_lod_manager();
        let manager = get_game_lod_manager();

        assert!(manager.get_static_game_lod_index("Low").is_some());
        assert!(manager.get_static_game_lod_index("Invalid").is_none());
        assert!(manager.get_dynamic_game_lod_index("VeryHigh").is_some());
    }

    #[test]
    fn test_add_lod_preset() {
        let mut manager = GameLODManager::new();

        // Add preset to Low level (index 0)
        let preset = manager.new_lod_preset(0);
        assert!(preset.is_some());

        let preset = preset.unwrap();
        preset.cpu_type = CpuType::P4;
        preset.mhz = 1500;
        preset.video_type = ChipsetType::GeForce3;
        preset.memory = 128;

        assert_eq!(manager.num_level_presets[0], 1);
    }

    #[test]
    fn test_add_bench_profile() {
        let mut manager = GameLODManager::new();

        let profile = manager.new_bench_profile();
        assert!(profile.is_some());

        let profile = profile.unwrap();
        profile.cpu_type = CpuType::P4;
        profile.mhz = 2189;
        profile.int_bench_index = 6.108187;
        profile.float_bench_index = 15.113676;
        profile.mem_bench_index = 9.402223;

        assert_eq!(manager.num_bench_profiles, 1);
    }
}
