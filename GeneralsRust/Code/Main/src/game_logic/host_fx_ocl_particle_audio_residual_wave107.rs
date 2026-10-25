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
    table.iter().position(|&n| n.eq_ignore_ascii_case(name))
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
pub const AUDIO_PRIORITY_NAMES_RESIDUAL: &[&str] = &["LOWEST", "LOW", "NORMAL", "HIGH", "CRITICAL"];

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
    "UI", "WORLD", "SHROUDED", "GLOBAL", "VOICE", "PLAYER", "ALLIES", "ENEMIES", "EVERYONE",
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

// ---------------------------------------------------------------------------
// 5. Combined Wave 107 residual pack
// ---------------------------------------------------------------------------

/// Combined Wave 107 residual honesty pack (particle/FXList/OCL/audio deepen).
///
/// Fail-closed aggregate — does not flip shell playable_claim.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn residual_pack_honesty_wave107_fail_closed_unknowns() {
        assert!(residual_name_index(PARTICLE_SHADER_TYPE_NAMES_RESIDUAL, "NOT_A_SHADER").is_none());
        assert!(
            residual_name_index(FXLIST_NUGGET_ENTRY_KIND_TABLE_RESIDUAL, "NotANugget").is_none()
        );
        assert!(residual_name_index(OCL_NUGGET_ENTRY_KIND_TABLE_RESIDUAL, "NotAnOcl").is_none());
        assert!(residual_name_index(AUDIO_PRIORITY_NAMES_RESIDUAL, "ULTRA").is_none());
        assert!(!audio_is_permanent_sound_residual(AC_INTERRUPT, 0));
    }
}
