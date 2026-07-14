//! Wave 93 residual peels: Particle system emit-rate deepen / Drawable opacity+shroud /
//! Shadow deepen / Terrain texture / Road residual.
//!
//! Orthogonal to Waves 79 (drawable StealthLook ordinals), 81 (terrain height sample),
//! 84 (ShadowType enum table), 88 (superweapon particle *name* tables).
//! Host-testable packs for render/terrain residual honesty.
//!
//! Sources (retail ZH INI + C++):
//! - ParticleSys.h/.cpp ParticlePriorityType / BurstDelay / BurstCount / countCoeff
//! - ParticleSystem.ini sample emit-rate residual (TsingMaTrailSmoke, etc.)
//! - GameData.ini MaxParticleCount residual
//! - Drawable.h/.cpp explicit/stealth/effective opacity + shroud residual
//! - GlobalData StealthFriendlyOpacity residual (50% → 0.5)
//! - Shadow.h MAX_SHADOW_LIGHTS + W3DShadowManager m_shadowColor residual
//! - TerrainTex.h TILE_OFFSET / TileData.h TILE_PIXEL_EXTENT / TEXTURE_WIDTH
//! - CloudMapTerrainTextureClass cloud slide residual
//! - TerrainTypes.h TerrainClass + Terrain.ini Texture/Class residual
//! - TerrainRoads.cpp / Roads.ini / W3DRoadBuffer.h road residual
//! - GameData.ini MaxRoadSegments/Vertex/Index/Types residual
//!
//! Fail-closed:
//! - Not full ParticleSystemManager LOD cull / GPU particle draw residual
//! - Not full Drawable W3D material pass / heat-vision GPU residual
//! - Not full volumetric shadow stencil / projected shadow GPU residual
//! - Not full TerrainTextureClass atlas update / CloudMap GPU residual
//! - Not full W3DRoadBuffer mesh bake / DX8 VB residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// C++ `REAL_TO_INT` residual (truncate toward zero for positive burst counts).
pub fn real_to_int_residual(v: f32) -> i32 {
    v as i32
}

/// Clamp residual used by Drawable::setEffectiveOpacity (`MIN(1, MAX(0, x))`).
pub fn clamp01_residual(v: f32) -> f32 {
    if v < 0.0 {
        0.0
    } else if v > 1.0 {
        1.0
    } else {
        v
    }
}

// ---------------------------------------------------------------------------
// 1. Particle system residual deepen (emit rates / priority / caps)
// ---------------------------------------------------------------------------

/// C++ `ParticlePriorityType` residual ordinals (ParticleSys.h).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ParticlePriorityResidual {
    Invalid = 0,
    WeaponExplosion = 1, // PARTICLE_PRIORITY_LOWEST
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
    AlwaysRender = 13, // PARTICLE_PRIORITY_HIGHEST
}

/// C++ `NUM_PARTICLE_PRIORITIES` residual (keep-last sentinel value).
pub const NUM_PARTICLE_PRIORITIES_RESIDUAL: u8 = 14;
/// C++ `PARTICLE_PRIORITY_LOWEST` residual.
pub const PARTICLE_PRIORITY_LOWEST_RESIDUAL: u8 = ParticlePriorityResidual::WeaponExplosion as u8;
/// C++ `PARTICLE_PRIORITY_HIGHEST` residual.
pub const PARTICLE_PRIORITY_HIGHEST_RESIDUAL: u8 = ParticlePriorityResidual::AlwaysRender as u8;
/// C++ `INVALID_PARTICLE_SYSTEM_ID` residual.
pub const INVALID_PARTICLE_SYSTEM_ID_RESIDUAL: u32 = 0;
/// C++ `DEFAULT_VOLUME_PARTICLE_DEPTH` residual (0 = off).
pub const DEFAULT_VOLUME_PARTICLE_DEPTH_RESIDUAL: u32 = 0;

/// C++ `ParticlePriorityNames[]` residual (DEFINE_PARTICLE_SYSTEM_NAMES).
pub const PARTICLE_PRIORITY_NAMES_RESIDUAL: &[&str] = &[
    "NONE",
    "WEAPON_EXPLOSION",
    "SCORCHMARK",
    "DUST_TRAIL",
    "BUILDUP",
    "DEBRIS_TRAIL",
    "UNIT_DAMAGE_FX",
    "DEATH_EXPLOSION",
    "SEMI_CONSTANT",
    "CONSTANT",
    "WEAPON_TRAIL",
    "AREA_EFFECT",
    "CRITICAL",
    "ALWAYS_RENDER",
];

/// C++ ParticleSystemInfo ctor residual: priority = PARTICLE_PRIORITY_LOWEST.
pub const PARTICLE_TEMPLATE_PRIORITY_CTOR_RESIDUAL: u8 = PARTICLE_PRIORITY_LOWEST_RESIDUAL;
/// C++ ParticleSystemInfo ctor residual: m_systemLifetime = 0 (forever when left 0).
pub const PARTICLE_TEMPLATE_SYSTEM_LIFETIME_CTOR_RESIDUAL: u32 = 0;
/// C++ ParticleSystem ctor residual: m_countCoeff = 1.0.
pub const PARTICLE_BURST_COUNT_COEFF_DEFAULT_RESIDUAL: f32 = 1.0;
/// C++ ParticleSystem ctor residual: m_delayCoeff = 1.0.
pub const PARTICLE_BURST_DELAY_COEFF_DEFAULT_RESIDUAL: f32 = 1.0;
/// C++ ParticleSystemInfo ctor residual: m_isOneShot = FALSE.
pub const PARTICLE_TEMPLATE_IS_ONE_SHOT_CTOR_RESIDUAL: bool = false;
/// C++ ParticleSystemInfo wind residual: m_windAngleChange = 0.15.
pub const PARTICLE_WIND_ANGLE_CHANGE_DEFAULT_RESIDUAL: f32 = 0.15;
/// C++ ParticleSystemInfo wind residual: m_windAngleChangeMin = 0.15.
pub const PARTICLE_WIND_ANGLE_CHANGE_MIN_DEFAULT_RESIDUAL: f32 = 0.15;
/// C++ ParticleSystemInfo wind residual: m_windAngleChangeMax = 0.45.
pub const PARTICLE_WIND_ANGLE_CHANGE_MAX_DEFAULT_RESIDUAL: f32 = 0.45;

/// Retail GameData.ini `MaxParticleCount` residual.
pub const MAX_PARTICLE_COUNT_RESIDUAL: i32 = 2500;
/// GlobalData ctor default for m_maxParticleCount residual (before INI).
pub const MAX_PARTICLE_COUNT_CTOR_RESIDUAL: i32 = 0;

/// Retail ParticleSystem.ini sample: TsingMaTrailSmoke emit residual.
pub const SAMPLE_PARTICLE_TSINGMA_NAME_RESIDUAL: &str = "TsingMaTrailSmoke";
pub const SAMPLE_PARTICLE_TSINGMA_PRIORITY_RESIDUAL: u8 =
    ParticlePriorityResidual::WeaponExplosion as u8;
pub const SAMPLE_PARTICLE_TSINGMA_BURST_DELAY_RESIDUAL: f32 = 40.0;
pub const SAMPLE_PARTICLE_TSINGMA_BURST_COUNT_MIN_RESIDUAL: f32 = 0.0;
pub const SAMPLE_PARTICLE_TSINGMA_BURST_COUNT_MAX_RESIDUAL: f32 = 2.0;
pub const SAMPLE_PARTICLE_TSINGMA_SYSTEM_LIFETIME_RESIDUAL: u32 = 0;
pub const SAMPLE_PARTICLE_TSINGMA_INITIAL_DELAY_RESIDUAL: f32 = 20.0;
pub const SAMPLE_PARTICLE_TSINGMA_IS_ONE_SHOT_RESIDUAL: bool = false;

/// Retail ParticleSystem.ini sample: finite SystemLifetime + burst residual.
pub const SAMPLE_PARTICLE_WATER_SPLASH_SYSTEM_LIFETIME_RESIDUAL: u32 = 30;
pub const SAMPLE_PARTICLE_WATER_SPLASH_BURST_DELAY_RESIDUAL: f32 = 1.0;
pub const SAMPLE_PARTICLE_WATER_SPLASH_BURST_COUNT_RESIDUAL: f32 = 10.0;

/// Resolve ParticlePriority name residual → ordinal (None for unknown).
pub fn particle_priority_name_index(name: &str) -> Option<u8> {
    PARTICLE_PRIORITY_NAMES_RESIDUAL
        .iter()
        .position(|&n| n == name)
        .map(|i| i as u8)
}

/// Emit residual: effective burst count after countCoeff
/// (`REAL_TO_INT(burstCount)` then `count *= m_countCoeff`).
pub fn particle_effective_burst_count_residual(burst_count: f32, count_coeff: f32) -> i32 {
    let base = real_to_int_residual(burst_count);
    real_to_int_residual((base as f32) * count_coeff)
}

/// Emit residual: effective burst delay frames after delayCoeff
/// (`(UnsignedInt)burstDelay.getValue() * delayCoeff`).
pub fn particle_effective_burst_delay_residual(burst_delay: f32, delay_coeff: f32) -> u32 {
    let base = real_to_int_residual(burst_delay).max(0) as u32;
    real_to_int_residual((base as f32) * delay_coeff).max(0) as u32
}

/// Emit residual: system is forever when SystemLifetime residual is 0.
pub fn particle_system_is_forever_residual(system_lifetime: u32) -> bool {
    system_lifetime == 0
}

/// Wave 93 honesty: particle system emit-rate residual deepen pack.
pub fn honesty_particle_system_emit_rate_residual_deepen_pack_wave93() -> bool {
    NUM_PARTICLE_PRIORITIES_RESIDUAL == 14
        && PARTICLE_PRIORITY_LOWEST_RESIDUAL == 1
        && PARTICLE_PRIORITY_HIGHEST_RESIDUAL == 13
        && PARTICLE_PRIORITY_NAMES_RESIDUAL.len() == 14
        && particle_priority_name_index("WEAPON_EXPLOSION") == Some(1)
        && particle_priority_name_index("CRITICAL") == Some(12)
        && particle_priority_name_index("ALWAYS_RENDER") == Some(13)
        && particle_priority_name_index("NONE") == Some(0)
        && particle_priority_name_index("NOT_A_PRIORITY").is_none()
        && PARTICLE_TEMPLATE_PRIORITY_CTOR_RESIDUAL == PARTICLE_PRIORITY_LOWEST_RESIDUAL
        && PARTICLE_TEMPLATE_SYSTEM_LIFETIME_CTOR_RESIDUAL == 0
        && (PARTICLE_BURST_COUNT_COEFF_DEFAULT_RESIDUAL - 1.0).abs() < 1e-5
        && (PARTICLE_BURST_DELAY_COEFF_DEFAULT_RESIDUAL - 1.0).abs() < 1e-5
        && !PARTICLE_TEMPLATE_IS_ONE_SHOT_CTOR_RESIDUAL
        && (PARTICLE_WIND_ANGLE_CHANGE_DEFAULT_RESIDUAL - 0.15).abs() < 1e-5
        && (PARTICLE_WIND_ANGLE_CHANGE_MIN_DEFAULT_RESIDUAL - 0.15).abs() < 1e-5
        && (PARTICLE_WIND_ANGLE_CHANGE_MAX_DEFAULT_RESIDUAL - 0.45).abs() < 1e-5
        && DEFAULT_VOLUME_PARTICLE_DEPTH_RESIDUAL == 0
        && INVALID_PARTICLE_SYSTEM_ID_RESIDUAL == 0
        && MAX_PARTICLE_COUNT_RESIDUAL == 2500
        && MAX_PARTICLE_COUNT_CTOR_RESIDUAL == 0
        && SAMPLE_PARTICLE_TSINGMA_NAME_RESIDUAL == "TsingMaTrailSmoke"
        && SAMPLE_PARTICLE_TSINGMA_PRIORITY_RESIDUAL == 1
        && (SAMPLE_PARTICLE_TSINGMA_BURST_DELAY_RESIDUAL - 40.0).abs() < 1e-5
        && (SAMPLE_PARTICLE_TSINGMA_BURST_COUNT_MIN_RESIDUAL - 0.0).abs() < 1e-5
        && (SAMPLE_PARTICLE_TSINGMA_BURST_COUNT_MAX_RESIDUAL - 2.0).abs() < 1e-5
        && SAMPLE_PARTICLE_TSINGMA_SYSTEM_LIFETIME_RESIDUAL == 0
        && (SAMPLE_PARTICLE_TSINGMA_INITIAL_DELAY_RESIDUAL - 20.0).abs() < 1e-5
        && !SAMPLE_PARTICLE_TSINGMA_IS_ONE_SHOT_RESIDUAL
        && particle_system_is_forever_residual(SAMPLE_PARTICLE_TSINGMA_SYSTEM_LIFETIME_RESIDUAL)
        && !particle_system_is_forever_residual(SAMPLE_PARTICLE_WATER_SPLASH_SYSTEM_LIFETIME_RESIDUAL)
        && SAMPLE_PARTICLE_WATER_SPLASH_SYSTEM_LIFETIME_RESIDUAL == 30
        && (SAMPLE_PARTICLE_WATER_SPLASH_BURST_DELAY_RESIDUAL - 1.0).abs() < 1e-5
        && (SAMPLE_PARTICLE_WATER_SPLASH_BURST_COUNT_RESIDUAL - 10.0).abs() < 1e-5
        && particle_effective_burst_count_residual(10.0, 1.0) == 10
        && particle_effective_burst_count_residual(10.0, 0.5) == 5
        && particle_effective_burst_count_residual(2.9, 1.0) == 2
        && particle_effective_burst_count_residual(0.0, 1.0) == 0
        && particle_effective_burst_delay_residual(40.0, 1.0) == 40
        && particle_effective_burst_delay_residual(40.0, 0.5) == 20
        && particle_effective_burst_delay_residual(3.0, 1.0) == 3
        && particle_effective_burst_delay_residual(
            SAMPLE_PARTICLE_TSINGMA_BURST_DELAY_RESIDUAL,
            PARTICLE_BURST_DELAY_COEFF_DEFAULT_RESIDUAL,
        ) == 40
}

// ---------------------------------------------------------------------------
// 2. Drawable residual deepen (opacity + shroud)
// ---------------------------------------------------------------------------

/// C++ `StealthLookType` residual ordinals (Drawable.h) — beyond Wave 79 snapshot fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum StealthLookResidual {
    None = 0,
    VisibleFriendly = 1,
    DisguisedEnemy = 2,
    VisibleDetected = 3,
    VisibleFriendlyDetected = 4,
    Invisible = 5,
}

/// StealthLook residual count.
pub const STEALTH_LOOK_COUNT_RESIDUAL: u8 = 6;

/// C++ Drawable ctor residual: m_explicitOpacity = 1.0.
pub const DRAWABLE_EXPLICIT_OPACITY_DEFAULT_RESIDUAL: f32 = 1.0;
/// C++ GlobalData ctor residual: m_stealthFriendlyOpacity = 0.5f.
pub const STEALTH_FRIENDLY_OPACITY_CTOR_RESIDUAL: f32 = 0.5;
/// Retail GameData.ini StealthFriendlyOpacity 50.0% → parsePercentToReal 0.5.
pub const STEALTH_FRIENDLY_OPACITY_INI_RESIDUAL: f32 = 0.5;
/// C++ setStealthLook residual: default m_stealthOpacity = 1.0 before look switch.
pub const STEALTH_OPACITY_OPAQUE_RESIDUAL: f32 = 1.0;
/// C++ setStealthLook residual: m_secondMaterialPassOpacity heat-vision on = 1.0.
pub const SECOND_MATERIAL_PASS_OPACITY_ON_RESIDUAL: f32 = 1.0;
/// C++ setStealthLook residual: m_secondMaterialPassOpacity off = 0.0.
pub const SECOND_MATERIAL_PASS_OPACITY_OFF_RESIDUAL: f32 = 0.0;
/// C++ Drawable ctor residual: m_shroudClearFrame = 0.
pub const DRAWABLE_SHROUD_CLEAR_FRAME_DEFAULT_RESIDUAL: u32 = 0;
/// Sentinel for setEffectiveOpacity explicitOpacity default arg residual (−1.0 = leave).
pub const DRAWABLE_EXPLICIT_OPACITY_LEAVE_SENTINEL_RESIDUAL: f32 = -1.0;

/// C++ DrawableStatus residual bits (Drawable.h).
pub const DRAWABLE_STATUS_NONE_RESIDUAL: u32 = 0x0000_0000;
pub const DRAWABLE_STATUS_DRAWS_IN_MIRROR_RESIDUAL: u32 = 0x0000_0001;
pub const DRAWABLE_STATUS_SHADOWS_RESIDUAL: u32 = 0x0000_0002;
pub const DRAWABLE_STATUS_TINT_COLOR_LOCKED_RESIDUAL: u32 = 0x0000_0004;
pub const DRAWABLE_STATUS_NO_STATE_PARTICLES_RESIDUAL: u32 = 0x0000_0008;
pub const DRAWABLE_STATUS_NO_SAVE_RESIDUAL: u32 = 0x0000_0010;

/// C++ `getEffectiveOpacity` residual: `m_explicitOpacity * m_effectiveStealthOpacity`.
pub fn drawable_effective_opacity_residual(
    explicit_opacity: f32,
    effective_stealth_opacity: f32,
) -> f32 {
    explicit_opacity * effective_stealth_opacity
}

/// C++ `Drawable::setEffectiveOpacity` residual.
/// When `explicit_opacity == -1.0`, stealth floor is left unchanged.
pub fn drawable_set_effective_opacity_residual(
    stealth_opacity: f32,
    pulse_factor: f32,
    explicit_opacity: f32,
) -> (f32, f32) {
    let mut stealth = stealth_opacity;
    if (explicit_opacity - DRAWABLE_EXPLICIT_OPACITY_LEAVE_SENTINEL_RESIDUAL).abs() > 1e-6 {
        stealth = clamp01_residual(explicit_opacity);
    }
    let pf = clamp01_residual(pulse_factor);
    let pulse_margin = 1.0 - stealth;
    let pulse_amount = pulse_margin * pf;
    let effective = stealth + pulse_amount;
    (stealth, effective)
}

/// StealthLook residual: VISIBLE_FRIENDLY uses GlobalData stealthFriendlyOpacity.
pub fn drawable_stealth_look_opacity_residual(look: StealthLookResidual) -> f32 {
    match look {
        StealthLookResidual::None
        | StealthLookResidual::DisguisedEnemy
        | StealthLookResidual::VisibleDetected
        | StealthLookResidual::Invisible => STEALTH_OPACITY_OPAQUE_RESIDUAL,
        StealthLookResidual::VisibleFriendly | StealthLookResidual::VisibleFriendlyDetected => {
            STEALTH_FRIENDLY_OPACITY_INI_RESIDUAL
        }
    }
}

/// StealthLook residual: heat-vision second pass for VISIBLE_DETECTED / FRIENDLY_DETECTED
/// (non-mine residual; host freezes non-mine path).
pub fn drawable_stealth_look_second_pass_opacity_residual(look: StealthLookResidual) -> f32 {
    match look {
        StealthLookResidual::VisibleDetected | StealthLookResidual::VisibleFriendlyDetected => {
            SECOND_MATERIAL_PASS_OPACITY_ON_RESIDUAL
        }
        _ => SECOND_MATERIAL_PASS_OPACITY_OFF_RESIDUAL,
    }
}

/// Wave 93 honesty: Drawable opacity + shroud residual deepen pack.
pub fn honesty_drawable_opacity_shroud_residual_deepen_pack_wave93() -> bool {
    STEALTH_LOOK_COUNT_RESIDUAL == 6
        && StealthLookResidual::None as u8 == 0
        && StealthLookResidual::VisibleFriendly as u8 == 1
        && StealthLookResidual::DisguisedEnemy as u8 == 2
        && StealthLookResidual::VisibleDetected as u8 == 3
        && StealthLookResidual::VisibleFriendlyDetected as u8 == 4
        && StealthLookResidual::Invisible as u8 == 5
        && (DRAWABLE_EXPLICIT_OPACITY_DEFAULT_RESIDUAL - 1.0).abs() < 1e-5
        && (STEALTH_FRIENDLY_OPACITY_CTOR_RESIDUAL - 0.5).abs() < 1e-5
        && (STEALTH_FRIENDLY_OPACITY_INI_RESIDUAL - 0.5).abs() < 1e-5
        && (STEALTH_OPACITY_OPAQUE_RESIDUAL - 1.0).abs() < 1e-5
        && DRAWABLE_SHROUD_CLEAR_FRAME_DEFAULT_RESIDUAL == 0
        && (DRAWABLE_EXPLICIT_OPACITY_LEAVE_SENTINEL_RESIDUAL + 1.0).abs() < 1e-5
        && DRAWABLE_STATUS_NONE_RESIDUAL == 0
        && DRAWABLE_STATUS_DRAWS_IN_MIRROR_RESIDUAL == 0x01
        && DRAWABLE_STATUS_SHADOWS_RESIDUAL == 0x02
        && DRAWABLE_STATUS_TINT_COLOR_LOCKED_RESIDUAL == 0x04
        && DRAWABLE_STATUS_NO_STATE_PARTICLES_RESIDUAL == 0x08
        && DRAWABLE_STATUS_NO_SAVE_RESIDUAL == 0x10
        && (drawable_effective_opacity_residual(1.0, 0.5) - 0.5).abs() < 1e-5
        && (drawable_effective_opacity_residual(0.8, 0.5) - 0.4).abs() < 1e-5
        && (drawable_effective_opacity_residual(1.0, 1.0) - 1.0).abs() < 1e-5
        && {
            let (s, e) = drawable_set_effective_opacity_residual(0.5, 0.0, -1.0);
            (s - 0.5).abs() < 1e-5 && (e - 0.5).abs() < 1e-5
        }
        && {
            let (s, e) = drawable_set_effective_opacity_residual(0.5, 1.0, -1.0);
            (s - 0.5).abs() < 1e-5 && (e - 1.0).abs() < 1e-5
        }
        && {
            let (s, e) = drawable_set_effective_opacity_residual(0.5, 0.5, -1.0);
            (s - 0.5).abs() < 1e-5 && (e - 0.75).abs() < 1e-5
        }
        && {
            let (s, e) = drawable_set_effective_opacity_residual(1.0, 0.0, 0.25);
            (s - 0.25).abs() < 1e-5 && (e - 0.25).abs() < 1e-5
        }
        && {
            let (s, e) = drawable_set_effective_opacity_residual(0.5, 0.0, 2.0);
            (s - 1.0).abs() < 1e-5 && (e - 1.0).abs() < 1e-5
        }
        && (drawable_stealth_look_opacity_residual(StealthLookResidual::None) - 1.0).abs() < 1e-5
        && (drawable_stealth_look_opacity_residual(StealthLookResidual::VisibleFriendly) - 0.5)
            .abs()
            < 1e-5
        && (drawable_stealth_look_opacity_residual(StealthLookResidual::VisibleFriendlyDetected)
            - 0.5)
            .abs()
            < 1e-5
        && (drawable_stealth_look_second_pass_opacity_residual(StealthLookResidual::VisibleDetected)
            - 1.0)
            .abs()
            < 1e-5
        && (drawable_stealth_look_second_pass_opacity_residual(StealthLookResidual::None) - 0.0)
            .abs()
            < 1e-5
}

// ---------------------------------------------------------------------------
// 3. Shadow residual deepen (beyond Wave 84 ShadowType enum table)
// ---------------------------------------------------------------------------

/// C++ `MAX_SHADOW_LIGHTS` residual (Shadow.h) — only 1 light supported.
pub const MAX_SHADOW_LIGHTS_RESIDUAL: i32 = 1;
/// C++ W3DShadowManager ctor residual: m_shadowColor = 0x7fa0a0a0 (ARGB).
pub const SHADOW_COLOR_ARGB_RESIDUAL: u32 = 0x7f_a0_a0_a0;
/// Shadow color residual components.
pub const SHADOW_COLOR_A_RESIDUAL: u8 = 0x7f;
pub const SHADOW_COLOR_R_RESIDUAL: u8 = 0xa0;
pub const SHADOW_COLOR_G_RESIDUAL: u8 = 0xa0;
pub const SHADOW_COLOR_B_RESIDUAL: u8 = 0xa0;
/// C++ W3DShadowManager::addShadow default type residual = SHADOW_VOLUME.
pub const SHADOW_ADD_DEFAULT_TYPE_RESIDUAL: u32 = 0x0000_0002;
/// ShadowType residual bits (must match Wave 84 table).
pub const SHADOW_NONE_RESIDUAL: u32 = 0x0000_0000;
pub const SHADOW_DECAL_RESIDUAL: u32 = 0x0000_0001;
pub const SHADOW_VOLUME_RESIDUAL: u32 = 0x0000_0002;
pub const SHADOW_PROJECTION_RESIDUAL: u32 = 0x0000_0004;
pub const SHADOW_DYNAMIC_PROJECTION_RESIDUAL: u32 = 0x0000_0008;
pub const SHADOW_DIRECTIONAL_PROJECTION_RESIDUAL: u32 = 0x0000_0010;
pub const SHADOW_ALPHA_DECAL_RESIDUAL: u32 = 0x0000_0020;
pub const SHADOW_ADDITIVE_DECAL_RESIDUAL: u32 = 0x0000_0040;
/// DrawableStatus residual: shadows enabled bit (cross-links Drawable + Shadow).
pub const DRAWABLE_SHADOWS_ENABLED_BIT_RESIDUAL: u32 = DRAWABLE_STATUS_SHADOWS_RESIDUAL;

/// Pack ARGB residual from components.
pub fn shadow_color_argb_residual(a: u8, r: u8, g: u8, b: u8) -> u32 {
    ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

/// Wave 93 honesty: shadow residual deepen pack.
pub fn honesty_shadow_residual_deepen_pack_wave93() -> bool {
    MAX_SHADOW_LIGHTS_RESIDUAL == 1
        && SHADOW_COLOR_ARGB_RESIDUAL == 0x7f_a0_a0_a0
        && SHADOW_COLOR_A_RESIDUAL == 0x7f
        && SHADOW_COLOR_R_RESIDUAL == 0xa0
        && SHADOW_COLOR_G_RESIDUAL == 0xa0
        && SHADOW_COLOR_B_RESIDUAL == 0xa0
        && shadow_color_argb_residual(
            SHADOW_COLOR_A_RESIDUAL,
            SHADOW_COLOR_R_RESIDUAL,
            SHADOW_COLOR_G_RESIDUAL,
            SHADOW_COLOR_B_RESIDUAL,
        ) == SHADOW_COLOR_ARGB_RESIDUAL
        && SHADOW_ADD_DEFAULT_TYPE_RESIDUAL == SHADOW_VOLUME_RESIDUAL
        && SHADOW_NONE_RESIDUAL == 0
        && SHADOW_DECAL_RESIDUAL == 0x01
        && SHADOW_VOLUME_RESIDUAL == 0x02
        && SHADOW_PROJECTION_RESIDUAL == 0x04
        && SHADOW_DYNAMIC_PROJECTION_RESIDUAL == 0x08
        && SHADOW_DIRECTIONAL_PROJECTION_RESIDUAL == 0x10
        && SHADOW_ALPHA_DECAL_RESIDUAL == 0x20
        && SHADOW_ADDITIVE_DECAL_RESIDUAL == 0x40
        && DRAWABLE_SHADOWS_ENABLED_BIT_RESIDUAL == 0x02
        && (SHADOW_VOLUME_RESIDUAL | SHADOW_DECAL_RESIDUAL) == 0x03
        && (SHADOW_ALPHA_DECAL_RESIDUAL & SHADOW_ADDITIVE_DECAL_RESIDUAL) == 0
}

// ---------------------------------------------------------------------------
// 4. Terrain texture residual peels
// ---------------------------------------------------------------------------

/// C++ TerrainTex.h `TILE_OFFSET` residual.
pub const TERRAIN_TILE_OFFSET_RESIDUAL: i32 = 8;
/// C++ TileData.h `TILE_PIXEL_EXTENT` residual.
pub const TERRAIN_TILE_PIXEL_EXTENT_RESIDUAL: i32 = 64;
/// C++ TileData.h `TEXTURE_WIDTH` residual (was 1024; ZH = 2048).
pub const TERRAIN_TEXTURE_WIDTH_RESIDUAL: i32 = 2048;
/// C++ TileData.h mip residual extents.
pub const TERRAIN_TILE_PIXEL_EXTENT_MIP1_RESIDUAL: i32 = 32;
pub const TERRAIN_TILE_PIXEL_EXTENT_MIP2_RESIDUAL: i32 = 16;
pub const TERRAIN_TILE_PIXEL_EXTENT_MIP3_RESIDUAL: i32 = 8;

/// C++ CloudMapTerrainTextureClass residual: m_xSlidePerSecond = −0.02.
pub const CLOUD_X_SLIDE_PER_SECOND_RESIDUAL: f32 = -0.02;
/// C++ CloudMap residual: m_ySlidePerSecond = 1.50 * m_xSlidePerSecond.
pub const CLOUD_Y_SLIDE_PER_SECOND_RESIDUAL: f32 = 1.50 * CLOUD_X_SLIDE_PER_SECOND_RESIDUAL;

/// Terrain class residual name anchors (TerrainTypes.h `terrainTypeNames[]`).
/// Note: some string labels differ from C enum identifiers (e.g. BLEND_EDGE,
/// DESERT_LIVE, SAND_ACCENT).
pub const TERRAIN_CLASS_NAMES_RESIDUAL: &[&str] = &[
    "NONE",
    "DESERT_1",
    "DESERT_2",
    "DESERT_3",
    "EASTERN_EUROPE_1",
    "EASTERN_EUROPE_2",
    "EASTERN_EUROPE_3",
    "SWISS_1",
    "SWISS_2",
    "SWISS_3",
    "SNOW_1",
    "SNOW_2",
    "SNOW_3",
    "DIRT",
    "GRASS",
    "TRANSITION",
    "ROCK",
    "SAND",
    "CLIFF",
    "WOOD",
    "BLEND_EDGE",
    "DESERT_LIVE",
    "DESERT_DRY",
    "SAND_ACCENT",
    "BEACH_TROPICAL",
    "BEACH_PARK",
    "MOUNTAIN_RUGGED",
    "GRASS_COBBLESTONE",
    "GRASS_ACCENT",
    "RESIDENTIAL",
    "SNOW_RUGGED",
    "SNOW_FLAT",
    "FIELD",
    "ASPHALT",
    "CONCRETE",
    "CHINA",
    "ROCK_ACCENT",
    "URBAN",
];

/// TerrainType FieldParse residual keys.
pub const TERRAIN_TYPE_FIELD_TEXTURE_RESIDUAL: &str = "Texture";
pub const TERRAIN_TYPE_FIELD_BLEND_EDGES_RESIDUAL: &str = "BlendEdges";
pub const TERRAIN_TYPE_FIELD_CLASS_RESIDUAL: &str = "Class";
pub const TERRAIN_TYPE_FIELD_RESTRICT_CONSTRUCTION_RESIDUAL: &str = "RestrictConstruction";

/// Retail Terrain.ini sample residual rows.
pub const SAMPLE_TERRAIN_ASPHALT_TYPE1_NAME_RESIDUAL: &str = "AsphaltType1";
pub const SAMPLE_TERRAIN_ASPHALT_TYPE1_TEXTURE_RESIDUAL: &str = "TXAsph01a.tga";
pub const SAMPLE_TERRAIN_ASPHALT_TYPE1_CLASS_RESIDUAL: &str = "ASPHALT";
pub const SAMPLE_TERRAIN_GRASS_ROCK_TRANSITION_NAME_RESIDUAL: &str = "GrassRockTransitionType1";
pub const SAMPLE_TERRAIN_GRASS_ROCK_TRANSITION_TEXTURE_RESIDUAL: &str = "TTGrasRock01a.tga";
pub const SAMPLE_TERRAIN_GRASS_ROCK_TRANSITION_CLASS_RESIDUAL: &str = "TRANSITION";

/// TerrainType ctor residual: m_blendEdgeTexture = FALSE, m_class = TERRAIN_NONE,
/// m_restrictConstruction = FALSE.
pub const TERRAIN_TYPE_BLEND_EDGE_DEFAULT_RESIDUAL: bool = false;
pub const TERRAIN_TYPE_CLASS_DEFAULT_RESIDUAL: &str = "NONE";
pub const TERRAIN_TYPE_RESTRICT_CONSTRUCTION_DEFAULT_RESIDUAL: bool = false;

/// Resolve terrain class residual name → index.
pub fn terrain_class_name_index(name: &str) -> Option<usize> {
    TERRAIN_CLASS_NAMES_RESIDUAL.iter().position(|&n| n == name)
}

/// Cloud offset residual step: `offset += slide * delta_ms / 1000`.
pub fn cloud_slide_offset_step_residual(slide_per_second: f32, delta_ms: f32) -> f32 {
    slide_per_second * delta_ms / 1000.0
}

/// Wave 93 honesty: terrain texture residual pack.
pub fn honesty_terrain_texture_residual_pack_wave93() -> bool {
    TERRAIN_TILE_OFFSET_RESIDUAL == 8
        && TERRAIN_TILE_PIXEL_EXTENT_RESIDUAL == 64
        && TERRAIN_TEXTURE_WIDTH_RESIDUAL == 2048
        && TERRAIN_TILE_PIXEL_EXTENT_MIP1_RESIDUAL == 32
        && TERRAIN_TILE_PIXEL_EXTENT_MIP2_RESIDUAL == 16
        && TERRAIN_TILE_PIXEL_EXTENT_MIP3_RESIDUAL == 8
        && TERRAIN_TILE_PIXEL_EXTENT_MIP1_RESIDUAL * 2 == TERRAIN_TILE_PIXEL_EXTENT_RESIDUAL
        && (CLOUD_X_SLIDE_PER_SECOND_RESIDUAL - (-0.02)).abs() < 1e-5
        && (CLOUD_Y_SLIDE_PER_SECOND_RESIDUAL - (1.50 * -0.02)).abs() < 1e-5
        && (cloud_slide_offset_step_residual(CLOUD_X_SLIDE_PER_SECOND_RESIDUAL, 1000.0)
            - CLOUD_X_SLIDE_PER_SECOND_RESIDUAL)
            .abs()
            < 1e-5
        && (cloud_slide_offset_step_residual(CLOUD_X_SLIDE_PER_SECOND_RESIDUAL, 500.0)
            - (CLOUD_X_SLIDE_PER_SECOND_RESIDUAL * 0.5))
            .abs()
            < 1e-5
        && TERRAIN_CLASS_NAMES_RESIDUAL.len() == 38
        && terrain_class_name_index("NONE") == Some(0)
        && terrain_class_name_index("TRANSITION") == Some(15)
        && terrain_class_name_index("ASPHALT") == Some(33)
        && terrain_class_name_index("URBAN") == Some(37)
        && terrain_class_name_index("NOT_A_CLASS").is_none()
        && TERRAIN_TYPE_FIELD_TEXTURE_RESIDUAL == "Texture"
        && TERRAIN_TYPE_FIELD_BLEND_EDGES_RESIDUAL == "BlendEdges"
        && TERRAIN_TYPE_FIELD_CLASS_RESIDUAL == "Class"
        && TERRAIN_TYPE_FIELD_RESTRICT_CONSTRUCTION_RESIDUAL == "RestrictConstruction"
        && !TERRAIN_TYPE_BLEND_EDGE_DEFAULT_RESIDUAL
        && TERRAIN_TYPE_CLASS_DEFAULT_RESIDUAL == "NONE"
        && !TERRAIN_TYPE_RESTRICT_CONSTRUCTION_DEFAULT_RESIDUAL
        && SAMPLE_TERRAIN_ASPHALT_TYPE1_NAME_RESIDUAL == "AsphaltType1"
        && SAMPLE_TERRAIN_ASPHALT_TYPE1_TEXTURE_RESIDUAL == "TXAsph01a.tga"
        && SAMPLE_TERRAIN_ASPHALT_TYPE1_CLASS_RESIDUAL == "ASPHALT"
        && SAMPLE_TERRAIN_GRASS_ROCK_TRANSITION_NAME_RESIDUAL == "GrassRockTransitionType1"
        && SAMPLE_TERRAIN_GRASS_ROCK_TRANSITION_TEXTURE_RESIDUAL == "TTGrasRock01a.tga"
        && SAMPLE_TERRAIN_GRASS_ROCK_TRANSITION_CLASS_RESIDUAL == "TRANSITION"
}

// ---------------------------------------------------------------------------
// 5. Road residual peels
// ---------------------------------------------------------------------------

/// C++ W3DRoadBuffer.h `DEFAULT_ROAD_SCALE` residual.
pub const DEFAULT_ROAD_SCALE_RESIDUAL: f32 = 8.0;
/// C++ W3DRoadBuffer.h `MAX_SEG_VERTEX` residual.
pub const ROAD_MAX_SEG_VERTEX_RESIDUAL: i32 = 500;
/// C++ W3DRoadBuffer.h `MAX_SEG_INDEX` residual.
pub const ROAD_MAX_SEG_INDEX_RESIDUAL: i32 = 2000;
/// C++ W3DRoadBuffer.h `NUM_CORNERS` residual.
pub const ROAD_NUM_CORNERS_RESIDUAL: i32 = 4;
/// C++ W3DRoadBuffer.h `NUM_JOINS` residual (TCorner keep-last sentinel = 8).
pub const ROAD_NUM_JOINS_RESIDUAL: i32 = 8;

/// C++ TCorner residual ordinals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RoadCornerTypeResidual {
    Segment = 0,
    Curve = 1,
    Tee = 2,
    FourWay = 3,
    ThreeWayY = 4,
    ThreeWayH = 5,
    ThreeWayHFlip = 6,
    AlphaJoin = 7,
}

/// C++ TerrainRoadType FieldParse residual keys (roads, not bridges).
pub const ROAD_FIELD_TEXTURE_RESIDUAL: &str = "Texture";
pub const ROAD_FIELD_ROAD_WIDTH_RESIDUAL: &str = "RoadWidth";
pub const ROAD_FIELD_ROAD_WIDTH_IN_TEXTURE_RESIDUAL: &str = "RoadWidthInTexture";

/// C++ TerrainRoadType ctor residual defaults.
pub const ROAD_WIDTH_CTOR_DEFAULT_RESIDUAL: f32 = 0.0;
pub const ROAD_WIDTH_IN_TEXTURE_CTOR_DEFAULT_RESIDUAL: f32 = 0.0;
/// C++ TerrainRoadCollection id counter residual: MUST start at 1.
pub const ROAD_ID_COUNTER_START_RESIDUAL: u32 = 1;

/// GlobalData ctor residual MaxRoad* defaults (before INI).
pub const MAX_ROAD_SEGMENTS_CTOR_RESIDUAL: i32 = 0;
pub const MAX_ROAD_VERTEX_CTOR_RESIDUAL: i32 = 0;
pub const MAX_ROAD_INDEX_CTOR_RESIDUAL: i32 = 0;
pub const MAX_ROAD_TYPES_CTOR_RESIDUAL: i32 = 0;

/// Retail GameData.ini MaxRoad* residual.
pub const MAX_ROAD_SEGMENTS_RESIDUAL: i32 = 4000;
pub const MAX_ROAD_VERTEX_RESIDUAL: i32 = 3000;
pub const MAX_ROAD_INDEX_RESIDUAL: i32 = 5000;
pub const MAX_ROAD_TYPES_RESIDUAL: i32 = 100;

/// Retail Roads.ini sample residual rows.
pub const SAMPLE_ROAD_TWO_LANE_NAME_RESIDUAL: &str = "TwoLane";
pub const SAMPLE_ROAD_TWO_LANE_TEXTURE_RESIDUAL: &str = "TRTwoLane.tga";
pub const SAMPLE_ROAD_TWO_LANE_WIDTH_RESIDUAL: f32 = 35.0;
pub const SAMPLE_ROAD_TWO_LANE_WIDTH_IN_TEXTURE_RESIDUAL: f32 = 0.9;

pub const SAMPLE_ROAD_FOUR_LANE_NAME_RESIDUAL: &str = "FourLane";
pub const SAMPLE_ROAD_FOUR_LANE_TEXTURE_RESIDUAL: &str = "TRFourLane.tga";
pub const SAMPLE_ROAD_FOUR_LANE_WIDTH_RESIDUAL: f32 = 60.0;
pub const SAMPLE_ROAD_FOUR_LANE_WIDTH_IN_TEXTURE_RESIDUAL: f32 = 0.9;

pub const SAMPLE_ROAD_COBBLESTONE_NAME_RESIDUAL: &str = "Cobblestone";
pub const SAMPLE_ROAD_COBBLESTONE_TEXTURE_RESIDUAL: &str = "TRCobbRoad.tga";
pub const SAMPLE_ROAD_COBBLESTONE_WIDTH_RESIDUAL: f32 = 30.0;

pub const SAMPLE_ROAD_GRASS_STRIP_NAME_RESIDUAL: &str = "GrassStrip";
pub const SAMPLE_ROAD_GRASS_STRIP_TEXTURE_RESIDUAL: &str = "TRGrassStrip.tga";
pub const SAMPLE_ROAD_GRASS_STRIP_WIDTH_RESIDUAL: f32 = 8.0;

/// Roads.ini path residual.
pub const ROADS_INI_PATH_RESIDUAL: &str = "Data\\INI\\Roads.ini";

/// Host residual: next road id after start (first assigned = 1, counter becomes 2).
pub fn road_next_id_residual(counter: u32) -> (u32, u32) {
    (counter, counter.saturating_add(1))
}

/// Wave 93 honesty: road residual pack.
pub fn honesty_road_residual_pack_wave93() -> bool {
    (DEFAULT_ROAD_SCALE_RESIDUAL - 8.0).abs() < 1e-5
        && ROAD_MAX_SEG_VERTEX_RESIDUAL == 500
        && ROAD_MAX_SEG_INDEX_RESIDUAL == 2000
        && ROAD_NUM_CORNERS_RESIDUAL == 4
        && ROAD_NUM_JOINS_RESIDUAL == 8
        && RoadCornerTypeResidual::Segment as u8 == 0
        && RoadCornerTypeResidual::Curve as u8 == 1
        && RoadCornerTypeResidual::Tee as u8 == 2
        && RoadCornerTypeResidual::FourWay as u8 == 3
        && RoadCornerTypeResidual::AlphaJoin as u8 == 7
        && (RoadCornerTypeResidual::AlphaJoin as i32) + 1 == ROAD_NUM_JOINS_RESIDUAL
        && ROAD_FIELD_TEXTURE_RESIDUAL == "Texture"
        && ROAD_FIELD_ROAD_WIDTH_RESIDUAL == "RoadWidth"
        && ROAD_FIELD_ROAD_WIDTH_IN_TEXTURE_RESIDUAL == "RoadWidthInTexture"
        && (ROAD_WIDTH_CTOR_DEFAULT_RESIDUAL - 0.0).abs() < 1e-5
        && (ROAD_WIDTH_IN_TEXTURE_CTOR_DEFAULT_RESIDUAL - 0.0).abs() < 1e-5
        && ROAD_ID_COUNTER_START_RESIDUAL == 1
        && {
            let (id, next) = road_next_id_residual(ROAD_ID_COUNTER_START_RESIDUAL);
            id == 1 && next == 2
        }
        && MAX_ROAD_SEGMENTS_CTOR_RESIDUAL == 0
        && MAX_ROAD_VERTEX_CTOR_RESIDUAL == 0
        && MAX_ROAD_INDEX_CTOR_RESIDUAL == 0
        && MAX_ROAD_TYPES_CTOR_RESIDUAL == 0
        && MAX_ROAD_SEGMENTS_RESIDUAL == 4000
        && MAX_ROAD_VERTEX_RESIDUAL == 3000
        && MAX_ROAD_INDEX_RESIDUAL == 5000
        && MAX_ROAD_TYPES_RESIDUAL == 100
        && MAX_ROAD_SEGMENTS_RESIDUAL > MAX_ROAD_SEGMENTS_CTOR_RESIDUAL
        && SAMPLE_ROAD_TWO_LANE_NAME_RESIDUAL == "TwoLane"
        && SAMPLE_ROAD_TWO_LANE_TEXTURE_RESIDUAL == "TRTwoLane.tga"
        && (SAMPLE_ROAD_TWO_LANE_WIDTH_RESIDUAL - 35.0).abs() < 1e-5
        && (SAMPLE_ROAD_TWO_LANE_WIDTH_IN_TEXTURE_RESIDUAL - 0.9).abs() < 1e-5
        && SAMPLE_ROAD_FOUR_LANE_NAME_RESIDUAL == "FourLane"
        && SAMPLE_ROAD_FOUR_LANE_TEXTURE_RESIDUAL == "TRFourLane.tga"
        && (SAMPLE_ROAD_FOUR_LANE_WIDTH_RESIDUAL - 60.0).abs() < 1e-5
        && (SAMPLE_ROAD_FOUR_LANE_WIDTH_IN_TEXTURE_RESIDUAL - 0.9).abs() < 1e-5
        && SAMPLE_ROAD_COBBLESTONE_NAME_RESIDUAL == "Cobblestone"
        && SAMPLE_ROAD_COBBLESTONE_TEXTURE_RESIDUAL == "TRCobbRoad.tga"
        && (SAMPLE_ROAD_COBBLESTONE_WIDTH_RESIDUAL - 30.0).abs() < 1e-5
        && SAMPLE_ROAD_GRASS_STRIP_NAME_RESIDUAL == "GrassStrip"
        && SAMPLE_ROAD_GRASS_STRIP_TEXTURE_RESIDUAL == "TRGrassStrip.tga"
        && (SAMPLE_ROAD_GRASS_STRIP_WIDTH_RESIDUAL - 8.0).abs() < 1e-5
        && ROADS_INI_PATH_RESIDUAL == "Data\\INI\\Roads.ini"
        // GrassStrip width matches DEFAULT_ROAD_SCALE residual (8).
        && (SAMPLE_ROAD_GRASS_STRIP_WIDTH_RESIDUAL - DEFAULT_ROAD_SCALE_RESIDUAL).abs() < 1e-5
}

// ---------------------------------------------------------------------------
// Combined Wave 93 pack
// ---------------------------------------------------------------------------

/// Combined Wave 93 honesty: all render/terrain residual packs.
pub fn honesty_render_terrain_residual_pack_wave93() -> bool {
    honesty_particle_system_emit_rate_residual_deepen_pack_wave93()
        && honesty_drawable_opacity_shroud_residual_deepen_pack_wave93()
        && honesty_shadow_residual_deepen_pack_wave93()
        && honesty_terrain_texture_residual_pack_wave93()
        && honesty_road_residual_pack_wave93()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn particle_emit_rate_wave93_honesty() {
        assert!(honesty_particle_system_emit_rate_residual_deepen_pack_wave93());
        assert_eq!(
            particle_effective_burst_count_residual(
                SAMPLE_PARTICLE_WATER_SPLASH_BURST_COUNT_RESIDUAL,
                1.0
            ),
            10
        );
    }

    #[test]
    fn drawable_opacity_shroud_wave93_honesty() {
        assert!(honesty_drawable_opacity_shroud_residual_deepen_pack_wave93());
        let (s, e) = drawable_set_effective_opacity_residual(0.5, 0.5, -1.0);
        assert!((s - 0.5).abs() < 1e-5);
        assert!((e - 0.75).abs() < 1e-5);
    }

    #[test]
    fn shadow_deepen_wave93_honesty() {
        assert!(honesty_shadow_residual_deepen_pack_wave93());
        assert_eq!(shadow_color_argb_residual(0x7f, 0xa0, 0xa0, 0xa0), 0x7fa0a0a0);
    }

    #[test]
    fn terrain_texture_wave93_honesty() {
        assert!(honesty_terrain_texture_residual_pack_wave93());
        assert_eq!(terrain_class_name_index("ASPHALT"), Some(33));
    }

    #[test]
    fn road_wave93_honesty() {
        assert!(honesty_road_residual_pack_wave93());
        assert_eq!(road_next_id_residual(1), (1, 2));
    }

    #[test]
    fn render_terrain_combined_wave93_honesty() {
        assert!(honesty_render_terrain_residual_pack_wave93());
    }
}
