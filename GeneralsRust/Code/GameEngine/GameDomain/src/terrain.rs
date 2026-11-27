//! World scale of the authored heightmap (C++ MapObject.h).

/// World units per heightmap cell along either horizontal axis.
pub const MAP_XY_FACTOR: f32 = 10.0;
/// Multiplier from a raw height sample to world elevation.
pub const MAP_HEIGHT_SCALE: f32 = MAP_XY_FACTOR / 16.0;
