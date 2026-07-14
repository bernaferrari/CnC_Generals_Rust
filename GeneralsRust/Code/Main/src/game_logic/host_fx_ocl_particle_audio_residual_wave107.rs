//! Wave 107 residual peels: ParticleSystem / FXList entry / OCL Create / Audio residual deepen.
//!
//! Orthogonal to Wave 88 (superweapon *name* tables) and Wave 93 (particle emit-rate /
//! priority residual). Host residual only — shell `playable_claim` stays false; network deferred.
//!
//! Sources (retail ZH C++ / INI):
//! - ParticleSys.h/.cpp ParticleShader/Type/EmissionVolume/Velocity/WindMotion name tables,
//!   ParticleSystemInfo ctor residual, MAX_KEYFRAMES, sample ParticleSystem.ini fields
//! - FXList.cpp TheFXListFieldParse + FXNugget field residual + ViewShake/Scorch lookups
//! - FXList.ini sample FX_Nuke entry residual (beyond Wave 88 name-only table)
//! - ObjectCreationList.cpp TheObjectCreationListFieldParse + DebrisDisposition +
//!   GenericObjectCreationNugget create residual + INVALID_ANGLE
//! - AudioEventInfo.h/.cpp AudioType / AudioPriority / SoundType / AudioControl residual
//!
//! Fail-closed:
//! - Not full ParticleSystemManager GPU spawn / LOD cull residual
//! - Not full FXListExecutor particle/sound/decal apply residual
//! - Not full OCL DeliverPayload flight matrix / ThingFactory create residual
//! - Not full Miles positional AudioEvent playback residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

/// Lookup residual name index (case-insensitive).
pub fn residual_name_index_ci(table: &[&str], name: &str) -> Option<usize> {
    table
        .iter()
        .position(|&n| n.eq_ignore_ascii_case(name))
}

/// Unique residual names check.
fn residual_names_unique(table: &[&str]) -> bool {
    let mut names: Vec<&str> = table.to_vec();
    names.sort_unstable();
    !names.windows(2).any(|w| w[0] == w[1])
}

// ---------------------------------------------------------------------------
// 1. ParticleSystem residual deepen (beyond Wave 93 emit-rate pack)
// ---------------------------------------------------------------------------

/// C++ `MAX_KEYFRAMES` residual (ParticleSys.h).
pub const PARTICLE_MAX_KEYFRAMES_RESIDUAL: usize = 8;

/// C++ ParticleShaderType residual names (DEFINE_PARTICLE_SYSTEM_NAMES).
/// Index 0 = INVALID_SHADER / "NONE".
pub const PARTICLE_SHADER_TYPE_NAMES_RESIDUAL: &[&str] =
    &["NONE", "ADDITIVE", "ALPHA", "ALPHA_TEST", "MULTIPLY"];

/// C++ ParticleType residual names.
pub const PARTICLE_TYPE_NAMES_RESIDUAL: &[&str] = &[
    "NONE",
    "PARTICLE",
    "DRAWABLE",
    "STREAK",
    "VOLUME_PARTICLE",
    "SMUDGE",
];

/// C++ EmissionVelocityType residual names.
pub const EMISSION_VELOCITY_TYPE_NAMES_RESIDUAL: &[&str] = &[
    "NONE",
    "ORTHO",
    "SPHERICAL",
    "HEMISPHERICAL",
    "CYLINDRICAL",
    "OUTWARD",
];

/// C++ EmissionVolumeType residual names.
pub const EMISSION_VOLUME_TYPE_NAMES_RESIDUAL: &[&str] =
    &["NONE", "POINT", "LINE", "BOX", "SPHERE", "CYLINDER"];

/// C++ WindMotion residual names (INI WindMotionNames).
/// Ordinals: INVALID=0, NOT_USED=1 ("Unused"), PING_PONG=2, CIRCULAR=3.
pub const WIND_MOTION_NAMES_RESIDUAL: &[&str] = &["NONE", "Unused", "PingPong", "Circular"];

/// C++ ParticleSystemInfo WindMotion enum residual.
pub const WIND_MOTION_INVALID: u8 = 0;
pub const WIND_MOTION_NOT_USED: u8 = 1;
pub const WIND_MOTION_PING_PONG: u8 = 2;
pub const WIND_MOTION_CIRCULAR: u8 = 3;

/// C++ ParticleSystemInfo ctor residual wind defaults.
pub const PARTICLE_WIND_ANGLE_CHANGE_CTOR: f32 = 0.15;
pub const PARTICLE_WIND_ANGLE_CHANGE_MIN_CTOR: f32 = 0.15;
pub const PARTICLE_WIND_ANGLE_CHANGE_MAX_CTOR: f32 = 0.45;
pub const PARTICLE_WIND_MOTION_START_ANGLE_MIN_CTOR: f32 = 0.0;
/// PI / 4 residual.
pub const PARTICLE_WIND_MOTION_START_ANGLE_MAX_CTOR: f32 = std::f32::consts::FRAC_PI_4;
/// TWO_PI - (PI / 4) residual.
pub const PARTICLE_WIND_MOTION_END_ANGLE_MIN_CTOR: f32 =
    std::f32::consts::TAU - std::f32::consts::FRAC_PI_4;
/// TWO_PI residual.
pub const PARTICLE_WIND_MOTION_END_ANGLE_MAX_CTOR: f32 = std::f32::consts::TAU;
pub const PARTICLE_WIND_MOTION_MOVING_TO_END_ANGLE_CTOR: bool = true;
pub const PARTICLE_GRAVITY_CTOR_RESIDUAL: f32 = 0.0;
pub const PARTICLE_IS_EMISSION_VOLUME_HOLLOW_CTOR: bool = false;
pub const PARTICLE_IS_GROUND_ALIGNED_CTOR: bool = false;
pub const PARTICLE_IS_EMIT_ABOVE_GROUND_ONLY_CTOR: bool = false;
pub const PARTICLE_IS_PARTICLE_UP_TOWARDS_EMITTER_CTOR: bool = false;

/// Sample field-parse key residual from ParticleSystemTemplate::m_fieldParseTable.
pub const PARTICLE_TEMPLATE_FIELD_PARSE_KEY_RESIDUAL: &[&str] = &[
    "Priority",
    "IsOneShot",
    "Shader",
    "Type",
    "ParticleName",
    "Gravity",
    "Lifetime",
    "SystemLifetime",
    "Size",
    "BurstDelay",
    "BurstCount",
    "InitialDelay",
    "VelocityType",
    "VolumeType",
    "IsHollow",
    "IsGroundAligned",
    "WindMotion",
    "WindAngleChangeMin",
    "WindAngleChangeMax",
];

/// Retail ParticleSystem.ini sample residual: TsingMaTrailSmoke deepen fields.
pub const SAMPLE_PS_TSINGMA_NAME: &str = "TsingMaTrailSmoke";
pub const SAMPLE_PS_TSINGMA_SHADER: &str = "ALPHA";
pub const SAMPLE_PS_TSINGMA_TYPE: &str = "PARTICLE";
pub const SAMPLE_PS_TSINGMA_PARTICLE_NAME: &str = "EXSmokNew1.tga";
pub const SAMPLE_PS_TSINGMA_GRAVITY: f32 = 0.01;
pub const SAMPLE_PS_TSINGMA_LIFETIME: f32 = 60.0;
pub const SAMPLE_PS_TSINGMA_SIZE: f32 = 5.0;
pub const SAMPLE_PS_TSINGMA_SIZE_RATE: f32 = 3.0;
pub const SAMPLE_PS_TSINGMA_PRIORITY: &str = "WEAPON_EXPLOSION";

/// Retail ParticleSystem.ini sample residual: ParticleUplinkCannon_LaunchFlare deepen.
pub const SAMPLE_PS_PUC_FLARE_NAME: &str = "ParticleUplinkCannon_LaunchFlare";
pub const SAMPLE_PS_PUC_FLARE_PRIORITY: &str = "CRITICAL";
pub const SAMPLE_PS_PUC_FLARE_SHADER: &str = "ADDITIVE";
pub const SAMPLE_PS_PUC_FLARE_TYPE: &str = "PARTICLE";
pub const SAMPLE_PS_PUC_FLARE_SYSTEM_LIFETIME: u32 = 1;
pub const SAMPLE_PS_PUC_FLARE_LIFETIME: f32 = 80.0;
pub const SAMPLE_PS_PUC_FLARE_GRAVITY: f32 = 0.0;

/// Wave 107 honesty: ParticleSystem residual deepen pack.
///
/// Freezes shader/type/volume/velocity/wind residual tables, ctor wind defaults,
/// field-parse keys, and sample ParticleSystem.ini deepen fields.
/// Fail-closed: not full GPU particle spawn residual.
pub fn honesty_particle_system_residual_deepen_pack_wave107() -> bool {
    let shader_ok = PARTICLE_SHADER_TYPE_NAMES_RESIDUAL.len() == 5
        && residual_name_index(PARTICLE_SHADER_TYPE_NAMES_RESIDUAL, "NONE") == Some(0)
        && residual_name_index(PARTICLE_SHADER_TYPE_NAMES_RESIDUAL, "ADDITIVE") == Some(1)
        && residual_name_index(PARTICLE_SHADER_TYPE_NAMES_RESIDUAL, "ALPHA") == Some(2)
        && residual_name_index(PARTICLE_SHADER_TYPE_NAMES_RESIDUAL, "ALPHA_TEST") == Some(3)
        && residual_name_index(PARTICLE_SHADER_TYPE_NAMES_RESIDUAL, "MULTIPLY") == Some(4)
        && residual_names_unique(PARTICLE_SHADER_TYPE_NAMES_RESIDUAL);

    let type_ok = PARTICLE_TYPE_NAMES_RESIDUAL.len() == 6
        && residual_name_index(PARTICLE_TYPE_NAMES_RESIDUAL, "PARTICLE") == Some(1)
        && residual_name_index(PARTICLE_TYPE_NAMES_RESIDUAL, "DRAWABLE") == Some(2)
        && residual_name_index(PARTICLE_TYPE_NAMES_RESIDUAL, "STREAK") == Some(3)
        && residual_name_index(PARTICLE_TYPE_NAMES_RESIDUAL, "VOLUME_PARTICLE") == Some(4)
        && residual_name_index(PARTICLE_TYPE_NAMES_RESIDUAL, "SMUDGE") == Some(5)
        && residual_names_unique(PARTICLE_TYPE_NAMES_RESIDUAL);

    let vel_ok = EMISSION_VELOCITY_TYPE_NAMES_RESIDUAL.len() == 6
        && residual_name_index(EMISSION_VELOCITY_TYPE_NAMES_RESIDUAL, "ORTHO") == Some(1)
        && residual_name_index(EMISSION_VELOCITY_TYPE_NAMES_RESIDUAL, "OUTWARD") == Some(5)
        && residual_names_unique(EMISSION_VELOCITY_TYPE_NAMES_RESIDUAL);

    let vol_ok = EMISSION_VOLUME_TYPE_NAMES_RESIDUAL.len() == 6
        && residual_name_index(EMISSION_VOLUME_TYPE_NAMES_RESIDUAL, "POINT") == Some(1)
        && residual_name_index(EMISSION_VOLUME_TYPE_NAMES_RESIDUAL, "SPHERE") == Some(4)
        && residual_name_index(EMISSION_VOLUME_TYPE_NAMES_RESIDUAL, "CYLINDER") == Some(5)
        && residual_names_unique(EMISSION_VOLUME_TYPE_NAMES_RESIDUAL);

    let wind_ok = WIND_MOTION_NAMES_RESIDUAL.len() == 4
        && residual_name_index(WIND_MOTION_NAMES_RESIDUAL, "Unused") == Some(1)
        && residual_name_index(WIND_MOTION_NAMES_RESIDUAL, "PingPong") == Some(2)
        && residual_name_index(WIND_MOTION_NAMES_RESIDUAL, "Circular") == Some(3)
        && WIND_MOTION_NOT_USED == 1
        && WIND_MOTION_PING_PONG == 2
        && WIND_MOTION_CIRCULAR == 3
        && residual_names_unique(WIND_MOTION_NAMES_RESIDUAL);

    let ctor_ok = (PARTICLE_WIND_ANGLE_CHANGE_CTOR - 0.15).abs() < 1e-5
        && (PARTICLE_WIND_ANGLE_CHANGE_MIN_CTOR - 0.15).abs() < 1e-5
        && (PARTICLE_WIND_ANGLE_CHANGE_MAX_CTOR - 0.45).abs() < 1e-5
        && (PARTICLE_WIND_MOTION_START_ANGLE_MIN_CTOR - 0.0).abs() < 1e-5
        && (PARTICLE_WIND_MOTION_START_ANGLE_MAX_CTOR - std::f32::consts::FRAC_PI_4).abs() < 1e-5
        && (PARTICLE_WIND_MOTION_END_ANGLE_MIN_CTOR
            - (std::f32::consts::TAU - std::f32::consts::FRAC_PI_4))
            .abs()
            < 1e-5
        && (PARTICLE_WIND_MOTION_END_ANGLE_MAX_CTOR - std::f32::consts::TAU).abs() < 1e-5
        && PARTICLE_WIND_MOTION_MOVING_TO_END_ANGLE_CTOR
        && (PARTICLE_GRAVITY_CTOR_RESIDUAL - 0.0).abs() < 1e-5
        && !PARTICLE_IS_EMISSION_VOLUME_HOLLOW_CTOR
        && !PARTICLE_IS_GROUND_ALIGNED_CTOR
        && !PARTICLE_IS_EMIT_ABOVE_GROUND_ONLY_CTOR
        && !PARTICLE_IS_PARTICLE_UP_TOWARDS_EMITTER_CTOR
        && PARTICLE_MAX_KEYFRAMES_RESIDUAL == 8;

    let fields_ok = PARTICLE_TEMPLATE_FIELD_PARSE_KEY_RESIDUAL.len() >= 18
        && residual_name_index(PARTICLE_TEMPLATE_FIELD_PARSE_KEY_RESIDUAL, "Shader").is_some()
        && residual_name_index(PARTICLE_TEMPLATE_FIELD_PARSE_KEY_RESIDUAL, "VolumeType").is_some()
        && residual_name_index(PARTICLE_TEMPLATE_FIELD_PARSE_KEY_RESIDUAL, "WindMotion").is_some()
        && residual_names_unique(PARTICLE_TEMPLATE_FIELD_PARSE_KEY_RESIDUAL);

    let sample_tsingma_ok = SAMPLE_PS_TSINGMA_NAME == "TsingMaTrailSmoke"
        && SAMPLE_PS_TSINGMA_SHADER == "ALPHA"
        && residual_name_index(PARTICLE_SHADER_TYPE_NAMES_RESIDUAL, SAMPLE_PS_TSINGMA_SHADER)
            == Some(2)
        && SAMPLE_PS_TSINGMA_TYPE == "PARTICLE"
        && residual_name_index(PARTICLE_TYPE_NAMES_RESIDUAL, SAMPLE_PS_TSINGMA_TYPE) == Some(1)
        && SAMPLE_PS_TSINGMA_PARTICLE_NAME == "EXSmokNew1.tga"
        && (SAMPLE_PS_TSINGMA_GRAVITY - 0.01).abs() < 1e-5
        && (SAMPLE_PS_TSINGMA_LIFETIME - 60.0).abs() < 1e-5
        && (SAMPLE_PS_TSINGMA_SIZE - 5.0).abs() < 1e-5
        && (SAMPLE_PS_TSINGMA_SIZE_RATE - 3.0).abs() < 1e-5
        && SAMPLE_PS_TSINGMA_PRIORITY == "WEAPON_EXPLOSION";

    let sample_puc_ok = SAMPLE_PS_PUC_FLARE_NAME == "ParticleUplinkCannon_LaunchFlare"
        && SAMPLE_PS_PUC_FLARE_PRIORITY == "CRITICAL"
        && SAMPLE_PS_PUC_FLARE_SHADER == "ADDITIVE"
        && residual_name_index(PARTICLE_SHADER_TYPE_NAMES_RESIDUAL, SAMPLE_PS_PUC_FLARE_SHADER)
            == Some(1)
        && SAMPLE_PS_PUC_FLARE_TYPE == "PARTICLE"
        && SAMPLE_PS_PUC_FLARE_SYSTEM_LIFETIME == 1
        && (SAMPLE_PS_PUC_FLARE_LIFETIME - 80.0).abs() < 1e-5
        && (SAMPLE_PS_PUC_FLARE_GRAVITY - 0.0).abs() < 1e-5;

    shader_ok
        && type_ok
        && vel_ok
        && vol_ok
        && wind_ok
        && ctor_ok
        && fields_ok
        && sample_tsingma_ok
        && sample_puc_ok
}

// ---------------------------------------------------------------------------
// 2. FXList residual deepen (beyond Wave 88 name tables — entry / nugget residual)
// ---------------------------------------------------------------------------

/// C++ `TheFXListFieldParse` residual entry kinds (FXList.cpp).
pub const FXLIST_NUGGET_ENTRY_KIND_TABLE_RESIDUAL: &[&str] = &[
    "Sound",
    "RayEffect",
    "Tracer",
    "LightPulse",
    "ViewShake",
    "TerrainScorch",
    "ParticleSystem",
    "FXListAtBonePos",
];

/// C++ ParticleSystemFXNugget field residual keys.
pub const FX_PARTICLE_SYSTEM_NUGGET_FIELD_TABLE: &[&str] = &[
    "Name",
    "Count",
    "Offset",
    "Radius",
    "Height",
    "InitialDelay",
    "RotateX",
    "RotateY",
    "RotateZ",
    "OrientToObject",
    "Ricochet",
    "AttachToObject",
    "CreateAtGroundHeight",
    "UseCallersRadius",
];

/// C++ ParticleSystemFXNugget ctor residual: m_count = 1.
pub const FX_PARTICLE_SYSTEM_NUGGET_COUNT_CTOR: i32 = 1;
/// C++ ParticleSystemFXNugget ctor residual: m_delay range −1 (leave particle-system default).
pub const FX_PARTICLE_SYSTEM_NUGGET_DELAY_LEAVE_SENTINEL: f32 = -1.0;
/// C++ FXListAtBonePosFXNugget MAX_BONE_POINTS residual.
pub const FX_LIST_AT_BONE_POS_MAX_BONE_POINTS: usize = 40;
/// C++ FXListStore::findFXList residual: "None" returns null.
pub const FXLIST_FIND_NONE_TOKEN: &str = "None";

/// C++ View::CameraShakeType residual names (View.h + ViewShakeFXNugget lookup).
pub const VIEW_SHAKE_TYPE_NAMES_RESIDUAL: &[&str] = &[
    "SUBTLE",
    "NORMAL",
    "STRONG",
    "SEVERE",
    "CINE_EXTREME",
    "CINE_INSANE",
];
pub const VIEW_SHAKE_SUBTLE: u8 = 0;
pub const VIEW_SHAKE_NORMAL: u8 = 1;
pub const VIEW_SHAKE_STRONG: u8 = 2;
pub const VIEW_SHAKE_SEVERE: u8 = 3;
pub const VIEW_SHAKE_CINE_EXTREME: u8 = 4;
pub const VIEW_SHAKE_CINE_INSANE: u8 = 5;
pub const VIEW_SHAKE_COUNT: u8 = 6;

/// C++ TerrainScorchFXNugget parseScorchType residual names.
pub const TERRAIN_SCORCH_TYPE_NAMES_RESIDUAL: &[&str] = &[
    "SCORCH_1",
    "SCORCH_2",
    "SCORCH_3",
    "SCORCH_4",
    "SHADOW_SCORCH",
    "RANDOM",
];
pub const SCORCH_1_RESIDUAL: i32 = 0;
pub const SCORCH_2_RESIDUAL: i32 = 1;
pub const SCORCH_3_RESIDUAL: i32 = 2;
pub const SCORCH_4_RESIDUAL: i32 = 3;
pub const SHADOW_SCORCH_RESIDUAL: i32 = 4;
/// C++ RANDOM scorch residual maps to −1 (then randomize SCORCH_1..SCORCH_4 at apply).
pub const SCORCH_RANDOM_SENTINEL_RESIDUAL: i32 = -1;
/// C++ TerrainScorchFXNugget ctor residual: m_scorch = −1.
pub const TERRAIN_SCORCH_TYPE_CTOR_RESIDUAL: i32 = -1;

/// Retail FXList.ini sample entry residual: FX_Nuke (beyond Wave 88 name-only).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FxListEntryNuggetResidual {
    pub kind: &'static str,
    pub key: &'static str,
    pub value: &'static str,
}

pub const SAMPLE_FX_NUKE_NAME: &str = "FX_Nuke";
/// Ordered residual nuggets for FX_Nuke head (ViewShake + Sound + first ParticleSystem).
pub const SAMPLE_FX_NUKE_ENTRY_NUGGETS: &[FxListEntryNuggetResidual] = &[
    FxListEntryNuggetResidual {
        kind: "ViewShake",
        key: "Type",
        value: "SEVERE",
    },
    FxListEntryNuggetResidual {
        kind: "Sound",
        key: "Name",
        value: "ExplosionNeutron",
    },
    FxListEntryNuggetResidual {
        kind: "ParticleSystem",
        key: "Name",
        value: "NukeFlare",
    },
    FxListEntryNuggetResidual {
        kind: "ParticleSystem",
        key: "Name",
        value: "NukeMushroomExplosion",
    },
    FxListEntryNuggetResidual {
        kind: "ParticleSystem",
        key: "Name",
        value: "NukeMushroomCloudRing",
    },
];
pub const SAMPLE_FX_NUKE_VIEW_SHAKE_TYPE: &str = "SEVERE";
pub const SAMPLE_FX_NUKE_SOUND_NAME: &str = "ExplosionNeutron";
pub const SAMPLE_FX_NUKE_FIRST_PS_NAME: &str = "NukeFlare";
pub const SAMPLE_FX_NUKE_FIRST_PS_OFFSET_Z: f32 = 90.0;

/// Wave 107 honesty: FXList entry residual deepen pack.
///
/// Freezes FXNugget entry kinds, ParticleSystemFXNugget fields/defaults, ViewShake/
/// Scorch lookups, find-None residual, and sample FX_Nuke entry residual.
/// Fail-closed: not full FXListExecutor residual.
pub fn honesty_fxlist_entry_residual_deepen_pack_wave107() -> bool {
    let kinds_ok = FXLIST_NUGGET_ENTRY_KIND_TABLE_RESIDUAL.len() == 8
        && residual_name_index(FXLIST_NUGGET_ENTRY_KIND_TABLE_RESIDUAL, "Sound") == Some(0)
        && residual_name_index(FXLIST_NUGGET_ENTRY_KIND_TABLE_RESIDUAL, "ParticleSystem")
            == Some(6)
        && residual_name_index(FXLIST_NUGGET_ENTRY_KIND_TABLE_RESIDUAL, "FXListAtBonePos")
            == Some(7)
        && residual_names_unique(FXLIST_NUGGET_ENTRY_KIND_TABLE_RESIDUAL);

    let ps_fields_ok = FX_PARTICLE_SYSTEM_NUGGET_FIELD_TABLE.len() >= 14
        && residual_name_index(FX_PARTICLE_SYSTEM_NUGGET_FIELD_TABLE, "Name") == Some(0)
        && residual_name_index(FX_PARTICLE_SYSTEM_NUGGET_FIELD_TABLE, "Count").is_some()
        && residual_name_index(FX_PARTICLE_SYSTEM_NUGGET_FIELD_TABLE, "UseCallersRadius")
            .is_some()
        && residual_names_unique(FX_PARTICLE_SYSTEM_NUGGET_FIELD_TABLE)
        && FX_PARTICLE_SYSTEM_NUGGET_COUNT_CTOR == 1
        && (FX_PARTICLE_SYSTEM_NUGGET_DELAY_LEAVE_SENTINEL + 1.0).abs() < 1e-5
        && FX_LIST_AT_BONE_POS_MAX_BONE_POINTS == 40
        && FXLIST_FIND_NONE_TOKEN == "None";

    let shake_ok = VIEW_SHAKE_TYPE_NAMES_RESIDUAL.len() == 6
        && VIEW_SHAKE_COUNT == 6
        && residual_name_index(VIEW_SHAKE_TYPE_NAMES_RESIDUAL, "SUBTLE") == Some(0)
        && residual_name_index(VIEW_SHAKE_TYPE_NAMES_RESIDUAL, "NORMAL") == Some(1)
        && residual_name_index(VIEW_SHAKE_TYPE_NAMES_RESIDUAL, "SEVERE") == Some(3)
        && residual_name_index(VIEW_SHAKE_TYPE_NAMES_RESIDUAL, "CINE_INSANE") == Some(5)
        && VIEW_SHAKE_SEVERE == 3
        && residual_names_unique(VIEW_SHAKE_TYPE_NAMES_RESIDUAL);

    let scorch_ok = TERRAIN_SCORCH_TYPE_NAMES_RESIDUAL.len() == 6
        && residual_name_index(TERRAIN_SCORCH_TYPE_NAMES_RESIDUAL, "SCORCH_1") == Some(0)
        && residual_name_index(TERRAIN_SCORCH_TYPE_NAMES_RESIDUAL, "SHADOW_SCORCH") == Some(4)
        && residual_name_index(TERRAIN_SCORCH_TYPE_NAMES_RESIDUAL, "RANDOM") == Some(5)
        && SCORCH_1_RESIDUAL == 0
        && SCORCH_4_RESIDUAL == 3
        && SHADOW_SCORCH_RESIDUAL == 4
        && SCORCH_RANDOM_SENTINEL_RESIDUAL == -1
        && TERRAIN_SCORCH_TYPE_CTOR_RESIDUAL == -1
        && residual_names_unique(TERRAIN_SCORCH_TYPE_NAMES_RESIDUAL);

    let nuke_ok = SAMPLE_FX_NUKE_NAME == "FX_Nuke"
        && SAMPLE_FX_NUKE_ENTRY_NUGGETS.len() >= 5
        && SAMPLE_FX_NUKE_ENTRY_NUGGETS[0].kind == "ViewShake"
        && SAMPLE_FX_NUKE_ENTRY_NUGGETS[0].value == "SEVERE"
        && residual_name_index(VIEW_SHAKE_TYPE_NAMES_RESIDUAL, SAMPLE_FX_NUKE_VIEW_SHAKE_TYPE)
            == Some(3)
        && SAMPLE_FX_NUKE_ENTRY_NUGGETS[1].kind == "Sound"
        && SAMPLE_FX_NUKE_ENTRY_NUGGETS[1].value == SAMPLE_FX_NUKE_SOUND_NAME
        && SAMPLE_FX_NUKE_ENTRY_NUGGETS[2].kind == "ParticleSystem"
        && SAMPLE_FX_NUKE_ENTRY_NUGGETS[2].value == SAMPLE_FX_NUKE_FIRST_PS_NAME
        && SAMPLE_FX_NUKE_FIRST_PS_NAME == "NukeFlare"
        && (SAMPLE_FX_NUKE_FIRST_PS_OFFSET_Z - 90.0).abs() < 1e-5
        // Every nugget kind in the sample entry is a known TheFXListFieldParse kind.
        && SAMPLE_FX_NUKE_ENTRY_NUGGETS.iter().all(|n| {
            residual_name_index(FXLIST_NUGGET_ENTRY_KIND_TABLE_RESIDUAL, n.kind).is_some()
        });

    kinds_ok && ps_fields_ok && shake_ok && scorch_ok && nuke_ok
}

// ---------------------------------------------------------------------------
// 3. ObjectCreationList Create residual deepen (beyond Wave 88 OCL name tables)
// ---------------------------------------------------------------------------

/// C++ `TheObjectCreationListFieldParse` residual entry kinds.
pub const OCL_NUGGET_ENTRY_KIND_TABLE_RESIDUAL: &[&str] = &[
    "CreateObject",
    "CreateDebris",
    "ApplyRandomForce",
    "DeliverPayload",
    "FireWeapon",
    "Attack",
];

/// C++ DebrisDisposition residual bit flags.
pub const DEBRIS_LIKE_EXISTING: u32 = 0x0000_0001;
pub const DEBRIS_ON_GROUND_ALIGNED: u32 = 0x0000_0002;
pub const DEBRIS_SEND_IT_FLYING: u32 = 0x0000_0004;
pub const DEBRIS_SEND_IT_UP: u32 = 0x0000_0008;
pub const DEBRIS_SEND_IT_OUT: u32 = 0x0000_0010;
pub const DEBRIS_RANDOM_FORCE: u32 = 0x0000_0020;
pub const DEBRIS_FLOATING: u32 = 0x0000_0040;
pub const DEBRIS_INHERIT_VELOCITY: u32 = 0x0000_0080;
pub const DEBRIS_WHIRLING: u32 = 0x0000_0100;

/// Ordered DebrisDispositionNames residual.
pub const DEBRIS_DISPOSITION_NAMES_RESIDUAL: &[&str] = &[
    "LIKE_EXISTING",
    "ON_GROUND_ALIGNED",
    "SEND_IT_FLYING",
    "SEND_IT_UP",
    "SEND_IT_OUT",
    "RANDOM_FORCE",
    "FLOATING",
    "INHERIT_VELOCITY",
    "WHIRLING",
];

/// C++ GenericObjectCreationNugget ctor residual defaults.
pub const OCL_GENERIC_DEBRIS_TO_GENERATE_CTOR: i32 = 1;
pub const OCL_GENERIC_DISPOSITION_CTOR: u32 = DEBRIS_ON_GROUND_ALIGNED;
pub const OCL_GENERIC_SPIN_RATE_CTOR: f32 = -1.0;
pub const OCL_GENERIC_YAW_RATE_CTOR: f32 = -1.0;
pub const OCL_GENERIC_ROLL_RATE_CTOR: f32 = -1.0;
pub const OCL_GENERIC_PITCH_RATE_CTOR: f32 = -1.0;
pub const OCL_GENERIC_MIN_HEALTH_CTOR: f32 = 1.0;
pub const OCL_GENERIC_MAX_HEALTH_CTOR: f32 = 1.0;
pub const OCL_GENERIC_PRESERVE_LAYER_CTOR: bool = true;
pub const OCL_GENERIC_NAME_ARE_OBJECTS_CTOR: bool = true;
pub const OCL_GENERIC_REQUIRES_LIVE_PLAYER_CTOR: bool = false;

/// C++ `INVALID_ANGLE` residual (GameType.h) — OCL create defaults angle to 0 when hit.
pub const OCL_INVALID_ANGLE_RESIDUAL: f32 = -100.0;
/// C++ GenericObjectCreationNugget create residual: INVALID_ANGLE → angle = 0.0.
pub const OCL_CREATE_ANGLE_DEFAULT_WHEN_INVALID: f32 = 0.0;
/// C++ lifetimeFrames default residual on create overloads.
pub const OCL_CREATE_LIFETIME_FRAMES_DEFAULT: u32 = 0;
/// C++ AttackNugget ctor residual: m_numberOfShots = 1.
pub const OCL_ATTACK_NUMBER_OF_SHOTS_CTOR: i32 = 1;

/// Sample CreateObject residual fields (OCL_A10DeathFinalBlowUp).
pub const SAMPLE_OCL_A10_DEATH_FINAL_NAME: &str = "OCL_A10DeathFinalBlowUp";
pub const SAMPLE_OCL_A10_CREATE_KIND: &str = "CreateObject";
pub const SAMPLE_OCL_A10_OBJECT_NAME: &str = "AmericaJetA10Hulk";
pub const SAMPLE_OCL_A10_COUNT: i32 = 1;
pub const SAMPLE_OCL_A10_DISPOSITION_INTENSITY: f32 = 0.4;
/// Combined SEND_IT_FLYING | INHERIT_VELOCITY | RANDOM_FORCE residual bits.
pub const SAMPLE_OCL_A10_DISPOSITION_BITS: u32 =
    DEBRIS_SEND_IT_FLYING | DEBRIS_INHERIT_VELOCITY | DEBRIS_RANDOM_FORCE;

/// Sample CreateDebris residual (OCL_A10DeathHitGround first debris wing).
pub const SAMPLE_OCL_A10_HIT_GROUND_NAME: &str = "OCL_A10DeathHitGround";
pub const SAMPLE_OCL_A10_DEBRIS_KIND: &str = "CreateDebris";
pub const SAMPLE_OCL_A10_DEBRIS_MODEL: &str = "AVWarthog_D2";
pub const SAMPLE_OCL_A10_DEBRIS_MASS: f32 = 3.0;
pub const SAMPLE_OCL_A10_DEBRIS_COUNT: i32 = 1;
pub const SAMPLE_OCL_A10_DEBRIS_MIN_LIFETIME_MS: u32 = 4000;
pub const SAMPLE_OCL_A10_DEBRIS_MAX_LIFETIME_MS: u32 = 6000;

/// C++ GenericObjectCreationNugget common field residual keys (subset).
pub const OCL_GENERIC_COMMON_FIELD_PARSE_KEYS: &[&str] = &[
    "PutInContainer",
    "ParticleSystem",
    "Count",
    "IgnorePrimaryObstacle",
    "OrientInForceDirection",
    "ExtraBounciness",
    "ExtraFriction",
    "Offset",
    "Disposition",
    "DispositionIntensity",
    "SpinRate",
    "YawRate",
    "RollRate",
    "PitchRate",
    "MinForceMagnitude",
    "MaxForceMagnitude",
    "MinForcePitch",
    "MaxForcePitch",
    "MinLifetime",
    "MaxLifetime",
    "SpreadFormation",
    "FadeIn",
    "FadeOut",
];

/// Resolve OCL create angle residual (INVALID_ANGLE → 0).
pub fn ocl_create_resolve_angle_residual(angle: f32) -> f32 {
    if (angle - OCL_INVALID_ANGLE_RESIDUAL).abs() < 1e-5 {
        OCL_CREATE_ANGLE_DEFAULT_WHEN_INVALID
    } else {
        angle
    }
}

/// Wave 107 honesty: OCL Create residual deepen pack.
///
/// Freezes OCL nugget entry kinds, DebrisDisposition, GenericObjectCreationNugget
/// ctor defaults, INVALID_ANGLE create residual, and sample CreateObject/CreateDebris.
/// Fail-closed: not full DeliverPayload flight / ThingFactory create residual.
pub fn honesty_ocl_create_residual_deepen_pack_wave107() -> bool {
    let kinds_ok = OCL_NUGGET_ENTRY_KIND_TABLE_RESIDUAL.len() == 6
        && residual_name_index(OCL_NUGGET_ENTRY_KIND_TABLE_RESIDUAL, "CreateObject") == Some(0)
        && residual_name_index(OCL_NUGGET_ENTRY_KIND_TABLE_RESIDUAL, "CreateDebris") == Some(1)
        && residual_name_index(OCL_NUGGET_ENTRY_KIND_TABLE_RESIDUAL, "DeliverPayload") == Some(3)
        && residual_name_index(OCL_NUGGET_ENTRY_KIND_TABLE_RESIDUAL, "FireWeapon") == Some(4)
        && residual_name_index(OCL_NUGGET_ENTRY_KIND_TABLE_RESIDUAL, "Attack") == Some(5)
        && residual_names_unique(OCL_NUGGET_ENTRY_KIND_TABLE_RESIDUAL);

    let disposition_ok = DEBRIS_DISPOSITION_NAMES_RESIDUAL.len() == 9
        && residual_name_index(DEBRIS_DISPOSITION_NAMES_RESIDUAL, "LIKE_EXISTING") == Some(0)
        && residual_name_index(DEBRIS_DISPOSITION_NAMES_RESIDUAL, "ON_GROUND_ALIGNED") == Some(1)
        && residual_name_index(DEBRIS_DISPOSITION_NAMES_RESIDUAL, "SEND_IT_FLYING") == Some(2)
        && residual_name_index(DEBRIS_DISPOSITION_NAMES_RESIDUAL, "WHIRLING") == Some(8)
        && DEBRIS_LIKE_EXISTING == 0x1
        && DEBRIS_ON_GROUND_ALIGNED == 0x2
        && DEBRIS_SEND_IT_FLYING == 0x4
        && DEBRIS_INHERIT_VELOCITY == 0x80
        && DEBRIS_WHIRLING == 0x100
        && residual_names_unique(DEBRIS_DISPOSITION_NAMES_RESIDUAL);

    let ctor_ok = OCL_GENERIC_DEBRIS_TO_GENERATE_CTOR == 1
        && OCL_GENERIC_DISPOSITION_CTOR == DEBRIS_ON_GROUND_ALIGNED
        && (OCL_GENERIC_SPIN_RATE_CTOR + 1.0).abs() < 1e-5
        && (OCL_GENERIC_YAW_RATE_CTOR + 1.0).abs() < 1e-5
        && (OCL_GENERIC_ROLL_RATE_CTOR + 1.0).abs() < 1e-5
        && (OCL_GENERIC_PITCH_RATE_CTOR + 1.0).abs() < 1e-5
        && (OCL_GENERIC_MIN_HEALTH_CTOR - 1.0).abs() < 1e-5
        && (OCL_GENERIC_MAX_HEALTH_CTOR - 1.0).abs() < 1e-5
        && OCL_GENERIC_PRESERVE_LAYER_CTOR
        && OCL_GENERIC_NAME_ARE_OBJECTS_CTOR
        && !OCL_GENERIC_REQUIRES_LIVE_PLAYER_CTOR
        && OCL_ATTACK_NUMBER_OF_SHOTS_CTOR == 1
        && OCL_CREATE_LIFETIME_FRAMES_DEFAULT == 0;

    let angle_ok = (OCL_INVALID_ANGLE_RESIDUAL + 100.0).abs() < 1e-5
        && (ocl_create_resolve_angle_residual(OCL_INVALID_ANGLE_RESIDUAL) - 0.0).abs() < 1e-5
        && (ocl_create_resolve_angle_residual(1.5) - 1.5).abs() < 1e-5
        && (ocl_create_resolve_angle_residual(0.0) - 0.0).abs() < 1e-5;

    let fields_ok = OCL_GENERIC_COMMON_FIELD_PARSE_KEYS.len() >= 20
        && residual_name_index(OCL_GENERIC_COMMON_FIELD_PARSE_KEYS, "Disposition").is_some()
        && residual_name_index(OCL_GENERIC_COMMON_FIELD_PARSE_KEYS, "Count").is_some()
        && residual_name_index(OCL_GENERIC_COMMON_FIELD_PARSE_KEYS, "MinLifetime").is_some()
        && residual_names_unique(OCL_GENERIC_COMMON_FIELD_PARSE_KEYS);

    let sample_create_ok = SAMPLE_OCL_A10_DEATH_FINAL_NAME == "OCL_A10DeathFinalBlowUp"
        && SAMPLE_OCL_A10_CREATE_KIND == "CreateObject"
        && residual_name_index(OCL_NUGGET_ENTRY_KIND_TABLE_RESIDUAL, SAMPLE_OCL_A10_CREATE_KIND)
            == Some(0)
        && SAMPLE_OCL_A10_OBJECT_NAME == "AmericaJetA10Hulk"
        && SAMPLE_OCL_A10_COUNT == 1
        && (SAMPLE_OCL_A10_DISPOSITION_INTENSITY - 0.4).abs() < 1e-5
        && SAMPLE_OCL_A10_DISPOSITION_BITS
            == (DEBRIS_SEND_IT_FLYING | DEBRIS_INHERIT_VELOCITY | DEBRIS_RANDOM_FORCE)
        && (SAMPLE_OCL_A10_DISPOSITION_BITS & DEBRIS_SEND_IT_FLYING) != 0
        && (SAMPLE_OCL_A10_DISPOSITION_BITS & DEBRIS_INHERIT_VELOCITY) != 0
        && (SAMPLE_OCL_A10_DISPOSITION_BITS & DEBRIS_RANDOM_FORCE) != 0
        && (SAMPLE_OCL_A10_DISPOSITION_BITS & DEBRIS_ON_GROUND_ALIGNED) == 0;

    let sample_debris_ok = SAMPLE_OCL_A10_HIT_GROUND_NAME == "OCL_A10DeathHitGround"
        && SAMPLE_OCL_A10_DEBRIS_KIND == "CreateDebris"
        && residual_name_index(OCL_NUGGET_ENTRY_KIND_TABLE_RESIDUAL, SAMPLE_OCL_A10_DEBRIS_KIND)
            == Some(1)
        && SAMPLE_OCL_A10_DEBRIS_MODEL == "AVWarthog_D2"
        && (SAMPLE_OCL_A10_DEBRIS_MASS - 3.0).abs() < 1e-5
        && SAMPLE_OCL_A10_DEBRIS_COUNT == 1
        && SAMPLE_OCL_A10_DEBRIS_MIN_LIFETIME_MS == 4000
        && SAMPLE_OCL_A10_DEBRIS_MAX_LIFETIME_MS == 6000
        && SAMPLE_OCL_A10_DEBRIS_MAX_LIFETIME_MS > SAMPLE_OCL_A10_DEBRIS_MIN_LIFETIME_MS;

    kinds_ok
        && disposition_ok
        && ctor_ok
        && angle_ok
        && fields_ok
        && sample_create_ok
        && sample_debris_ok
}

// ---------------------------------------------------------------------------
// 4. Audio residual deepen (beyond Wave 88 superweapon AudioEvent name tables)
// ---------------------------------------------------------------------------

/// C++ AudioType residual (AudioEventInfo.h).
pub const AT_MUSIC: u8 = 0;
pub const AT_STREAMING: u8 = 1;
pub const AT_SOUND_EFFECT: u8 = 2;
pub const AUDIO_TYPE_NAMES_RESIDUAL: &[&str] = &["Music", "Streaming", "SoundEffect"];

/// C++ AudioPriority residual + theAudioPriorityNames.
pub const AP_LOWEST: u8 = 0;
pub const AP_LOW: u8 = 1;
pub const AP_NORMAL: u8 = 2;
pub const AP_HIGH: u8 = 3;
pub const AP_CRITICAL: u8 = 4;
pub const AUDIO_PRIORITY_NAMES_RESIDUAL: &[&str] =
    &["LOWEST", "LOW", "NORMAL", "HIGH", "CRITICAL"];

/// C++ SoundType residual bit flags + theSoundTypeNames order.
pub const ST_UI: u32 = 0x0001;
pub const ST_WORLD: u32 = 0x0002;
pub const ST_SHROUDED: u32 = 0x0004;
pub const ST_GLOBAL: u32 = 0x0008;
pub const ST_VOICE: u32 = 0x0010;
pub const ST_PLAYER: u32 = 0x0020;
pub const ST_ALLIES: u32 = 0x0040;
pub const ST_ENEMIES: u32 = 0x0080;
pub const ST_EVERYONE: u32 = 0x0100;
pub const SOUND_TYPE_NAMES_RESIDUAL: &[&str] = &[
    "UI",
    "WORLD",
    "SHROUDED",
    "GLOBAL",
    "VOICE",
    "PLAYER",
    "ALLIES",
    "ENEMIES",
    "EVERYONE",
];

/// C++ AudioControl residual bit flags + theAudioControlNames.
pub const AC_LOOP: u32 = 0x0001;
pub const AC_RANDOM: u32 = 0x0002;
pub const AC_ALL: u32 = 0x0004;
pub const AC_POSTDELAY: u32 = 0x0008;
pub const AC_INTERRUPT: u32 = 0x0010;
pub const AUDIO_CONTROL_NAMES_RESIDUAL: &[&str] =
    &["LOOP", "RANDOM", "ALL", "POSTDELAY", "INTERRUPT"];

/// C++ AudioEventInfo field-parse residual keys (INIAudioEventInfo.cpp).
pub const AUDIO_EVENT_INFO_FIELD_PARSE_KEYS: &[&str] = &[
    "Filename",
    "Volume",
    "VolumeShift",
    "MinVolume",
    "PitchShift",
    "Delay",
    "Limit",
    "LoopCount",
    "Priority",
    "Type",
    "Control",
    "Sounds",
    "SoundsNight",
    "SoundsEvening",
    "SoundsMorning",
    "Attack",
    "Decay",
    "MinRange",
    "MaxRange",
    "LowPassCutoff",
];

/// C++ OwnerType residual (AudioEventRTS.h).
pub const OT_POSITIONAL: u8 = 0;
pub const OT_DRAWABLE: u8 = 1;
pub const OT_OBJECT: u8 = 2;
pub const OT_DEAD: u8 = 3;
pub const OT_INVALID: u8 = 4;
pub const OWNER_TYPE_NAMES_RESIDUAL: &[&str] =
    &["Positional", "Drawable", "Object", "Dead", "Invalid"];

/// C++ PortionToPlay residual (AudioEventRTS.h).
pub const PP_ATTACK: u8 = 0;
pub const PP_SOUND: u8 = 1;
pub const PP_DECAY: u8 = 2;
pub const PP_DONE: u8 = 3;
pub const PORTION_TO_PLAY_NAMES_RESIDUAL: &[&str] = &["Attack", "Sound", "Decay", "Done"];

/// C++ AudioEventInfo::isPermanentSound residual: LOOP && loopCount == 0.
pub fn audio_is_permanent_sound_residual(control: u32, loop_count: i32) -> bool {
    (control & AC_LOOP) != 0 && loop_count == 0
}

/// C++ parsePitchShift residual: pitch = 1.0 + pct/100.
pub fn audio_pitch_shift_from_percent_residual(pct: f32) -> f32 {
    1.0 + pct / 100.0
}

/// Wave 107 honesty: Audio residual deepen pack.
///
/// Freezes AudioType/Priority/SoundType/Control residual tables, field-parse keys,
/// OwnerType/PortionToPlay, isPermanentSound + pitch-shift residual helpers.
/// Fail-closed: not full Miles positional playback residual.
pub fn honesty_audio_residual_deepen_pack_wave107() -> bool {
    let type_ok = AUDIO_TYPE_NAMES_RESIDUAL.len() == 3
        && AT_MUSIC == 0
        && AT_STREAMING == 1
        && AT_SOUND_EFFECT == 2
        && residual_name_index(AUDIO_TYPE_NAMES_RESIDUAL, "Music") == Some(0)
        && residual_name_index(AUDIO_TYPE_NAMES_RESIDUAL, "SoundEffect") == Some(2)
        && residual_names_unique(AUDIO_TYPE_NAMES_RESIDUAL);

    let priority_ok = AUDIO_PRIORITY_NAMES_RESIDUAL.len() == 5
        && AP_LOWEST == 0
        && AP_CRITICAL == 4
        && residual_name_index(AUDIO_PRIORITY_NAMES_RESIDUAL, "LOWEST") == Some(0)
        && residual_name_index(AUDIO_PRIORITY_NAMES_RESIDUAL, "NORMAL") == Some(2)
        && residual_name_index(AUDIO_PRIORITY_NAMES_RESIDUAL, "CRITICAL") == Some(4)
        && residual_names_unique(AUDIO_PRIORITY_NAMES_RESIDUAL);

    let sound_type_ok = SOUND_TYPE_NAMES_RESIDUAL.len() == 9
        && ST_UI == 0x1
        && ST_WORLD == 0x2
        && ST_EVERYONE == 0x100
        && residual_name_index(SOUND_TYPE_NAMES_RESIDUAL, "UI") == Some(0)
        && residual_name_index(SOUND_TYPE_NAMES_RESIDUAL, "WORLD") == Some(1)
        && residual_name_index(SOUND_TYPE_NAMES_RESIDUAL, "EVERYONE") == Some(8)
        && residual_names_unique(SOUND_TYPE_NAMES_RESIDUAL)
        // Bit order matches name index: bit = 1 << index.
        && (0..SOUND_TYPE_NAMES_RESIDUAL.len()).all(|i| {
            let expected = 1u32 << i;
            match i {
                0 => ST_UI == expected,
                1 => ST_WORLD == expected,
                2 => ST_SHROUDED == expected,
                3 => ST_GLOBAL == expected,
                4 => ST_VOICE == expected,
                5 => ST_PLAYER == expected,
                6 => ST_ALLIES == expected,
                7 => ST_ENEMIES == expected,
                8 => ST_EVERYONE == expected,
                _ => false,
            }
        });

    let control_ok = AUDIO_CONTROL_NAMES_RESIDUAL.len() == 5
        && AC_LOOP == 0x1
        && AC_INTERRUPT == 0x10
        && residual_name_index(AUDIO_CONTROL_NAMES_RESIDUAL, "LOOP") == Some(0)
        && residual_name_index(AUDIO_CONTROL_NAMES_RESIDUAL, "INTERRUPT") == Some(4)
        && residual_names_unique(AUDIO_CONTROL_NAMES_RESIDUAL);

    let fields_ok = AUDIO_EVENT_INFO_FIELD_PARSE_KEYS.len() >= 20
        && residual_name_index(AUDIO_EVENT_INFO_FIELD_PARSE_KEYS, "Priority").is_some()
        && residual_name_index(AUDIO_EVENT_INFO_FIELD_PARSE_KEYS, "Sounds").is_some()
        && residual_name_index(AUDIO_EVENT_INFO_FIELD_PARSE_KEYS, "MinRange").is_some()
        && residual_names_unique(AUDIO_EVENT_INFO_FIELD_PARSE_KEYS);

    let owner_ok = OWNER_TYPE_NAMES_RESIDUAL.len() == 5
        && OT_POSITIONAL == 0
        && OT_OBJECT == 2
        && OT_INVALID == 4
        && residual_names_unique(OWNER_TYPE_NAMES_RESIDUAL);

    let portion_ok = PORTION_TO_PLAY_NAMES_RESIDUAL.len() == 4
        && PP_ATTACK == 0
        && PP_SOUND == 1
        && PP_DECAY == 2
        && PP_DONE == 3
        && residual_names_unique(PORTION_TO_PLAY_NAMES_RESIDUAL);

    let helpers_ok = audio_is_permanent_sound_residual(AC_LOOP, 0)
        && !audio_is_permanent_sound_residual(AC_LOOP, 3)
        && !audio_is_permanent_sound_residual(0, 0)
        && !audio_is_permanent_sound_residual(AC_RANDOM, 0)
        && (audio_pitch_shift_from_percent_residual(0.0) - 1.0).abs() < 1e-5
        && (audio_pitch_shift_from_percent_residual(10.0) - 1.1).abs() < 1e-5
        && (audio_pitch_shift_from_percent_residual(-50.0) - 0.5).abs() < 1e-5;

    type_ok
        && priority_ok
        && sound_type_ok
        && control_ok
        && fields_ok
        && owner_ok
        && portion_ok
        && helpers_ok
}

// ---------------------------------------------------------------------------
// 5. Combined Wave 107 residual pack
// ---------------------------------------------------------------------------

/// Combined Wave 107 residual honesty pack (particle/FXList/OCL/audio deepen).
///
/// Fail-closed aggregate — does not flip shell playable_claim.
pub fn honesty_fx_ocl_particle_audio_residual_pack_wave107() -> bool {
    honesty_particle_system_residual_deepen_pack_wave107()
        && honesty_fxlist_entry_residual_deepen_pack_wave107()
        && honesty_ocl_create_residual_deepen_pack_wave107()
        && honesty_audio_residual_deepen_pack_wave107()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn residual_pack_honesty_wave107_particle() {
        assert!(honesty_particle_system_residual_deepen_pack_wave107());
    }

    #[test]
    fn residual_pack_honesty_wave107_fxlist() {
        assert!(honesty_fxlist_entry_residual_deepen_pack_wave107());
    }

    #[test]
    fn residual_pack_honesty_wave107_ocl() {
        assert!(honesty_ocl_create_residual_deepen_pack_wave107());
    }

    #[test]
    fn residual_pack_honesty_wave107_audio() {
        assert!(honesty_audio_residual_deepen_pack_wave107());
    }

    #[test]
    fn residual_pack_honesty_wave107_combined() {
        assert!(honesty_fx_ocl_particle_audio_residual_pack_wave107());
    }

    #[test]
    fn residual_pack_honesty_wave107_fail_closed_unknowns() {
        assert!(residual_name_index(PARTICLE_SHADER_TYPE_NAMES_RESIDUAL, "NOT_A_SHADER").is_none());
        assert!(residual_name_index(FXLIST_NUGGET_ENTRY_KIND_TABLE_RESIDUAL, "NotANugget").is_none());
        assert!(residual_name_index(OCL_NUGGET_ENTRY_KIND_TABLE_RESIDUAL, "NotAnOcl").is_none());
        assert!(residual_name_index(AUDIO_PRIORITY_NAMES_RESIDUAL, "ULTRA").is_none());
        assert!(!audio_is_permanent_sound_residual(AC_INTERRUPT, 0));
    }
}
