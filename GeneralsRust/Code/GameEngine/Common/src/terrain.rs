// FILE: terrain.rs
// Ported from C++ Common/Terrain.h
//
// Common terrain/map information shared by GameLogic and GameClient.
// Author: Colin Day, April 2001
//
// The original C++ Terrain.h contains only this constant.
// Actual terrain type definitions live in TerrainTypes.h (ported to terrain_types.rs).

/// Maximum size of map filename (with extension).
pub const MAX_TERRAIN_NAME_LEN: usize = 64;
