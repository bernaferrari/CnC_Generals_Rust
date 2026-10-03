use glam::Vec3;

/// Keyframe for scalar values
#[derive(Debug, Clone, Copy)]
pub struct Keyframe {
    pub value: f32,
    pub frame: u32,
}

impl Default for Keyframe {
    fn default() -> Self {
        Self {
            value: 0.0,
            frame: 0,
        }
    }
}

/// RGB color keyframe
#[derive(Debug, Clone, Copy)]
pub struct RGBColorKeyframe {
    pub color: [f32; 3], // RGB
    pub frame: u32,
}

impl Default for RGBColorKeyframe {
    fn default() -> Self {
        Self {
            color: [0.0, 0.0, 0.0],
            frame: 0,
        }
    }
}

/// Random keyframe with range
#[derive(Debug, Clone, Copy)]
pub struct RandomKeyframe {
    pub min_value: f32,
    pub max_value: f32,
    pub distribution_type: u32,
    pub frame: u32,
}

impl Default for RandomKeyframe {
    fn default() -> Self {
        Self {
            min_value: 0.0,
            max_value: 0.0,
            distribution_type: 0,
            frame: 0,
        }
    }
}

/// Authored client random range used by the current particle adapter.
#[derive(Debug, Clone, Copy)]
pub struct GameClientRandomVariable {
    pub min: f32,
    pub max: f32,
    /// Existing adapter encoding: 0 means its supported uniform range. This
    /// is not the C++ CONSTANT/UNIFORM enum representation; retain it until
    /// the snapshot migration has its own compatibility evidence.
    pub distribution_type: u32,
}

impl Default for GameClientRandomVariable {
    fn default() -> Self {
        Self {
            min: 0.0,
            max: 0.0,
            distribution_type: 0,
        }
    }
}

impl GameClientRandomVariable {
    pub fn new(min: f32, max: f32) -> Self {
        Self {
            min,
            max,
            distribution_type: 0,
        }
    }

    /// Sample through the caller's stream. The existing adapter's numeric
    /// encoding uses 0 for supported uniform ranges; other encodings retain
    /// the C++ release fallback of zero. Constant/empty ranges never draw.
    pub fn sample_with(&self, mut draw: impl FnMut(f32, f32) -> f32) -> f32 {
        match self.distribution_type {
            0 if self.max - self.min <= 0.0 => self.max,
            0 => draw(self.min, self.max),
            _ => 0.0,
        }
    }
}

/// Emission velocity configuration
#[derive(Debug, Clone, Copy)]
pub enum EmissionVelocity {
    Ortho {
        x: GameClientRandomVariable,
        y: GameClientRandomVariable,
        z: GameClientRandomVariable,
    },
    Spherical {
        speed: GameClientRandomVariable,
    },
    Hemispherical {
        speed: GameClientRandomVariable,
    },
    Cylindrical {
        radial: GameClientRandomVariable,
        normal: GameClientRandomVariable,
    },
    Outward {
        speed: GameClientRandomVariable,
        other_speed: GameClientRandomVariable,
    },
}

impl Default for EmissionVelocity {
    fn default() -> Self {
        EmissionVelocity::Ortho {
            x: GameClientRandomVariable::default(),
            y: GameClientRandomVariable::default(),
            z: GameClientRandomVariable::default(),
        }
    }
}

/// Emission volume configuration
#[derive(Debug, Clone, Copy, Default)]
pub enum EmissionVolume {
    #[default]
    Point,
    Line {
        start: Vec3,
        end: Vec3,
    },
    Box {
        half_size: Vec3,
    },
    Sphere {
        radius: f32,
    },
    Cylinder {
        radius: f32,
        length: f32,
    },
}
