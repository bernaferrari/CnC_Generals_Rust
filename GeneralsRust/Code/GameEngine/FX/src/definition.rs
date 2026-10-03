pub type FXListResult<T> = Result<T, FXListError>;

#[derive(Debug, Clone, PartialEq)]
pub enum FXListError {
    InvalidName,
    ParseError(String),
    NotFound,
}

impl std::fmt::Display for FXListError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FXListError::InvalidName => write!(f, "Invalid FXList name"),
            FXListError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            FXListError::NotFound => write!(f, "FXList not found"),
        }
    }
}

impl std::error::Error for FXListError {}

/// View shake types (C++ View::CameraShakeType, FXList.cpp:397)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraShakeType {
    Subtle,
    Normal,
    Strong,
    Severe,
    CineExtreme,
    CineInsane,
}

/// Terrain scorch types (C++ Scorches enum)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ScorchType {
    Scorch1,
    Scorch2,
    Scorch3,
    Scorch4,
    ShadowScorch,
    Random = -1,
}

/// FX Nugget types - audio/visual effect components
/// Matches C++ TheFXListFieldParse[] (FXList.cpp:746)
#[derive(Debug, Clone)]
pub enum FXNugget<Name = String> {
    Sound {
        name: Name,
    },
    Tracer {
        name: Name,
        bone_name: Name,
        speed: f32,
        decay_at: f32,
        length: f32,
        width: f32,
        color: (f32, f32, f32),
        probability: f32,
    },
    RayEffect {
        name: Name,
        primary_offset: (f32, f32, f32),
        secondary_offset: (f32, f32, f32),
    },
    LightPulse {
        color: (f32, f32, f32),
        radius: f32,
        radius_as_percent_of_object_size: f32,
        increase_frames: u32,
        decrease_frames: u32,
    },
    ViewShake {
        shake_type: CameraShakeType,
    },
    TerrainScorch {
        scorch_type: ScorchType,
        radius: f32,
    },
    ParticleSystem {
        name: Name,
        count: i32,
        offset: (f32, f32, f32),
        radius: FxRandomVariable,
        height: FxRandomVariable,
        initial_delay: FxRandomVariable,
        rotate_x: f32,
        rotate_y: f32,
        rotate_z: f32,
        orient_to_object: bool,
        ricochet: bool,
        attach_to_object: bool,
        create_at_ground_height: bool,
        use_callers_radius: bool,
    },
    FXListAtBonePos {
        fx_name: Name,
        bone_name: Name,
        orient_to_bone: bool,
    },
}

/// Authored ranges are definitions; sampling belongs to the executing client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Distribution {
    Constant,
    Uniform,
    Gaussian,
    Triangular,
    LowBias,
    HighBias,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FxRandomVariable {
    pub minimum: f32,
    pub maximum: f32,
    pub distribution: Distribution,
}
impl From<f32> for FxRandomVariable {
    fn from(value: f32) -> Self {
        Self {
            minimum: value,
            maximum: value,
            distribution: Distribution::Constant,
        }
    }
}

/// Nuggets actually visited by Common `FXList::doFXObj` (tests + leftover drain).
#[derive(Debug, Clone, PartialEq)]
pub enum DispatchedFxNugget {
    Sound(String),
    Tracer(String),
    RayEffect(String),
    LightPulse,
    ViewShake(CameraShakeType),
    TerrainScorch(ScorchType),
    ParticleSystem(String),
    FXListAtBonePos(String),
}

impl<Name: std::borrow::Borrow<str>> FXNugget<Name> {
    pub fn dispatched_kind(&self) -> DispatchedFxNugget {
        match self {
            FXNugget::Sound { name } => DispatchedFxNugget::Sound(name.borrow().to_string()),
            FXNugget::Tracer { name, .. } => DispatchedFxNugget::Tracer(name.borrow().to_string()),
            FXNugget::RayEffect { name, .. } => {
                DispatchedFxNugget::RayEffect(name.borrow().to_string())
            }
            FXNugget::LightPulse { .. } => DispatchedFxNugget::LightPulse,
            FXNugget::ViewShake { shake_type } => DispatchedFxNugget::ViewShake(*shake_type),
            FXNugget::TerrainScorch { scorch_type, .. } => {
                DispatchedFxNugget::TerrainScorch(*scorch_type)
            }
            FXNugget::ParticleSystem { name, .. } => {
                DispatchedFxNugget::ParticleSystem(name.borrow().to_string())
            }
            FXNugget::FXListAtBonePos { fx_name, .. } => {
                DispatchedFxNugget::FXListAtBonePos(fx_name.borrow().to_string())
            }
        }
    }
}
