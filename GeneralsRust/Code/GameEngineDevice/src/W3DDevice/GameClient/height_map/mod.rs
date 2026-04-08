//! Height Map System Module
//!
//! Port of C++ terrain height query infrastructure:
//! - HeightMap.h/cpp (2429 lines)
//! - WorldHeightMap.h/cpp (2554 lines)
//! - BaseHeightMap.h/cpp (2990 lines)
//! - FlatHeightMap.h/cpp (615 lines)
//!
//! PARITY_NOTE: Total 8,588 lines ported. This module provides the core terrain
//! height data storage and query functions used by gameplay, AI, rendering, and physics.

pub mod base_height_map;
pub mod flat_height_map;
pub mod height_map;
pub mod world_height_map;

pub use base_height_map::{
    BaseHeightMap, HeightSampleType, ScorchMark, ShoreLineTileInfo, ShoreLineTileSortInfo, TBounds,
    DEFAULT_IMPASSABLE_SLOPE, LOS_FUDGE, MAP_HEIGHT_SCALE, MAP_XY_FACTOR,
    MAX_ENABLED_DYNAMIC_LIGHTS, MAX_SCORCH_INDEX, MAX_SCORCH_MARKS, MAX_SCORCH_VERTEX,
    PATHFIND_CLIFF_SLOPE_LIMIT_F, SCORCH_MARKS_IN_TEXTURE, SCORCH_PER_ROW,
};
pub use flat_height_map::{FlatHeightMap, FlatUpdateState};
pub use height_map::{
    HeightMap, DEFAULT_MAX_BATCH_SHORELINE_TILES, DEFAULT_MAX_FRAME_EXTRA_BLEND_TILES,
    DEFAULT_MAX_MAP_EXTRA_BLEND_TILES, DEFAULT_MAX_MAP_SHORELINE_TILES, FLIP_TRIANGLES,
    VERTEX_BUFFER_TILE_LENGTH,
};
pub use world_height_map::{
    ICoord2D, TBlendTileInfo, TCliffInfo, TXTextureClass, WorldHeightMap, FLAG_VAL, K_MAX_HEIGHT,
    K_MIN_HEIGHT, NORMAL_DRAW_HEIGHT, NORMAL_DRAW_WIDTH, NUM_ALPHA_TILES, NUM_BLEND_TILES,
    NUM_CLIFF_INFO, NUM_SOURCE_TILES, NUM_TEXTURE_CLASSES, STRETCH_DRAW_HEIGHT, STRETCH_DRAW_WIDTH,
    TEXTURE_WIDTH, TILE_PIXEL_EXTENT,
};

// Re-export CellsPerTile as constant
pub const CELLS_PER_TILE: i32 = flat_height_map::CELLS_PER_TILE;
