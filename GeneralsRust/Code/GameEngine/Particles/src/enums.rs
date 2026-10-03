/// Particle priority levels (matches C++ ParticleSys.h exactly)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ParticlePriorityType {
    /// C++ `INVALID_PRIORITY` / `ParticlePriorityNames[0] = "NONE"`.
    None = 0,
    WeaponExplosion = 1,
    ScorchMark,
    DustTrail,
    Buildup,
    DebrisTrail,
    UnitDamageFx,
    DeathExplosion,
    SemiConstant,
    Constant,
    WeaponTrail,
    AreaEffect,
    Critical,
    AlwaysRender,
}

impl ParticlePriorityType {
    pub fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(ParticlePriorityType::None),
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
}

/// Particle shader types (matches C++ exactly)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticleShaderType {
    /// C++ `INVALID_SHADER` / retail INI `Shader = NONE`.
    /// It is intentionally preserved rather than coerced into a visible blend
    /// mode; callers that do not implement the associated particle subtype
    /// fail closed.
    Invalid = 0,
    Additive = 1,
    Alpha,
    AlphaTest,
    Multiply,
}

/// Particle types (matches C++ exactly)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticleType {
    /// C++ `INVALID_TYPE` / retail INI `Type = NONE`.
    Invalid = 0,
    Particle = 1,
    Drawable,
    Streak,
    VolumeParticle,
    Smudge,
}

/// Emission velocity types (matches C++ exactly)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmissionVelocityType {
    /// C++ `INVALID_VELOCITY` / retail INI `VelocityType = NONE`.
    Invalid = 0,
    Ortho = 1,
    Spherical,
    Hemispherical,
    Cylindrical,
    Outward,
}

/// Emission volume types (matches C++ exactly)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmissionVolumeType {
    /// C++ `INVALID_VOLUME` / retail INI `VolumeType = NONE`.
    Invalid = 0,
    Point = 1,
    Line,
    Box,
    Sphere,
    Cylinder,
}

/// Wind motion types (matches C++ exactly)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindMotion {
    /// C++ `NONE`; the shipped data normally uses `Unused` instead.
    Invalid = 0,
    NotUsed = 1,
    PingPong,
    Circular,
}
