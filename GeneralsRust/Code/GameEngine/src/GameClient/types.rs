// FILE: types.rs
// Author: Ported from C++ GameType.h and BaseType.h
// Desc: Basic geometric types and game identifiers for the camera system

use std::f32::consts::PI;

/// 3D coordinate with Real (f32) components
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coord3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Coord3D {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn zero() -> Self {
        Self { x: 0.0, y: 0.0, z: 0.0 }
    }
}

impl Default for Coord3D {
    fn default() -> Self {
        Self::zero()
    }
}

/// 2D coordinate with Real (f32) components
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coord2D {
    pub x: f32,
    pub y: f32,
}

impl Coord2D {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }

    pub fn normalize(&mut self) {
        let len = (self.x * self.x + self.y * self.y).sqrt();
        if len > 0.0 {
            self.x /= len;
            self.y /= len;
        }
    }
}

impl Default for Coord2D {
    fn default() -> Self {
        Self::zero()
    }
}

/// 2D coordinate with integer components (screen coordinates)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ICoord2D {
    pub x: i32,
    pub y: i32,
}

impl ICoord2D {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn zero() -> Self {
        Self { x: 0, y: 0 }
    }
}

impl Default for ICoord2D {
    fn default() -> Self {
        Self::zero()
    }
}

/// 2D region with integer components
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IRegion2D {
    pub lo: ICoord2D,
    pub hi: ICoord2D,
}

impl IRegion2D {
    pub fn new(lo: ICoord2D, hi: ICoord2D) -> Self {
        Self { lo, hi }
    }
}

/// Unique identifier for game objects
/// Matches C++ ObjectID enum from GameType.h
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectID(pub u32);

impl ObjectID {
    pub const INVALID: ObjectID = ObjectID(0);

    pub fn is_valid(self) -> bool {
        self.0 != 0
    }
}

impl Default for ObjectID {
    fn default() -> Self {
        Self::INVALID
    }
}

/// Unique identifier for drawable objects
/// Matches C++ DrawableID enum from GameType.h
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DrawableID(pub u32);

impl DrawableID {
    pub const INVALID: DrawableID = DrawableID(0);

    pub fn is_valid(self) -> bool {
        self.0 != 0
    }
}

impl Default for DrawableID {
    fn default() -> Self {
        Self::INVALID
    }
}

/// Helper constant: PI as f32
pub const PI_F32: f32 = PI;
