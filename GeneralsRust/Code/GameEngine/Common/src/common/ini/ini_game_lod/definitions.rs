//! GameLOD.h value definitions and original defaults.

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(i32)]
pub enum ParticlePriorityType {
    Invalid = 0,
    WeaponExplosion = 1,
    ScorchMark = 2,
    DustTrail = 3,
    Buildup = 4,
    DebrisTrail = 5,
    UnitDamageFx = 6,
    DeathExplosion = 7,
    SemiConstant = 8,
    Constant = 9,
    WeaponTrail = 10,
    AreaEffect = 11,
    Critical = 12,
    AlwaysRender = 13,
}

impl Default for ParticlePriorityType {
    fn default() -> Self {
        ParticlePriorityType::WeaponExplosion
    }
}

impl ParticlePriorityType {
    /// Convert from string name
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "none" | "invalid_priority" => Some(ParticlePriorityType::Invalid),
            "lowest" | "particle_priority_lowest" | "weapon_explosion" => {
                Some(ParticlePriorityType::WeaponExplosion)
            }
            "scorchmark" | "scorch_mark" => Some(ParticlePriorityType::ScorchMark),
            "dust_trail" | "dusttrail" => Some(ParticlePriorityType::DustTrail),
            "buildup" => Some(ParticlePriorityType::Buildup),
            "debris_trail" | "debristrail" => Some(ParticlePriorityType::DebrisTrail),
            "unit_damage_fx" | "unitdamagefx" => Some(ParticlePriorityType::UnitDamageFx),
            "death_explosion" | "deathexplosion" => Some(ParticlePriorityType::DeathExplosion),
            "semi_constant" | "semiconstant" => Some(ParticlePriorityType::SemiConstant),
            "constant" => Some(ParticlePriorityType::Constant),
            "weapon_trail" | "weapontrail" => Some(ParticlePriorityType::WeaponTrail),
            "area_effect" | "areaeffect" => Some(ParticlePriorityType::AreaEffect),
            "critical" => Some(ParticlePriorityType::Critical),
            "always_render" | "alwaysrender" => Some(ParticlePriorityType::AlwaysRender),
            _ => None,
        }
    }

    pub fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(ParticlePriorityType::Invalid),
            1 => Some(ParticlePriorityType::WeaponExplosion),
            2 => Some(ParticlePriorityType::ScorchMark),
            3 => Some(ParticlePriorityType::DustTrail),
            4 => Some(ParticlePriorityType::Buildup),
            5 => Some(ParticlePriorityType::DebrisTrail),
            6 => Some(ParticlePriorityType::UnitDamageFx),
            7 => Some(ParticlePriorityType::DeathExplosion),
            8 => Some(ParticlePriorityType::SemiConstant),
            9 => Some(ParticlePriorityType::Constant),
            10 => Some(ParticlePriorityType::WeaponTrail),
            11 => Some(ParticlePriorityType::AreaEffect),
            12 => Some(ParticlePriorityType::Critical),
            13 => Some(ParticlePriorityType::AlwaysRender),
            _ => None,
        }
    }

    fn to_cpp_name(self) -> &'static str {
        match self {
            ParticlePriorityType::Invalid => "NONE",
            ParticlePriorityType::WeaponExplosion => "WEAPON_EXPLOSION",
            ParticlePriorityType::ScorchMark => "SCORCHMARK",
            ParticlePriorityType::DustTrail => "DUST_TRAIL",
            ParticlePriorityType::Buildup => "BUILDUP",
            ParticlePriorityType::DebrisTrail => "DEBRIS_TRAIL",
            ParticlePriorityType::UnitDamageFx => "UNIT_DAMAGE_FX",
            ParticlePriorityType::DeathExplosion => "DEATH_EXPLOSION",
            ParticlePriorityType::SemiConstant => "SEMI_CONSTANT",
            ParticlePriorityType::Constant => "CONSTANT",
            ParticlePriorityType::WeaponTrail => "WEAPON_TRAIL",
            ParticlePriorityType::AreaEffect => "AREA_EFFECT",
            ParticlePriorityType::Critical => "CRITICAL",
            ParticlePriorityType::AlwaysRender => "ALWAYS_RENDER",
        }
    }

    /// Get string name
    pub fn to_str(self) -> &'static str {
        self.to_cpp_name()
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
            min_dynamic_particle_priority: ParticlePriorityType::WeaponExplosion,
            min_dynamic_particle_skip_priority: ParticlePriorityType::WeaponExplosion,
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
