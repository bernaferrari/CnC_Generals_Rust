//! # Terrain Module
//!
//! Comprehensive terrain rendering and management system for Command & Conquer
//! Generals Zero Hour, including heightmaps, texturing, roads, and water.
//!
//! ## Features
//!
//! - High-resolution heightmap-based terrain
//! - Multi-layer texture blending with detail textures
//! - Road and path rendering system
//! - Water body rendering with reflection/refraction
//! - Terrain deformation for explosions and construction
//! - LOD system for performance optimization
//! - Collision detection and pathfinding integration
//!
//! ## Architecture
//!
//! The terrain system consists of several main components:
//! - [`TerrainManager`] - Central coordinator for all terrain operations
//! - [`TerrainChunk`] - Individual terrain tiles for efficient rendering
//! - [`TerrainTextures`] - Multi-layer texture management
//! - [`RoadSystem`] - Road and path rendering
//! - [`WaterSystem`] - Water body management
//!
//! ## Example Usage
//!
//! ```rust,no_run
//! use game_client_rust::terrain::{TerrainManager, TerrainConfig};
//!
//! let mut terrain = TerrainManager::new();
//! terrain.init().unwrap();
//!
//! // Load heightmap and texture layers
//! terrain.load_heightmap("Data/Maps/Example/ExampleHeight.raw").unwrap();
//! terrain
//!     .load_texture_layers(&[
//!         "Data/Terrain/Grass.dds",
//!         "Data/Terrain/Cliff.dds",
//!         "Data/Terrain/Snow.dds",
//!         "Data/Terrain/Sand.dds",
//!     ])
//!     .unwrap();
//!
//! // Feed the camera each frame before drawing
//! terrain.update(delta_time);
//! terrain.render(view_matrix, projection_matrix);
//! ```

pub mod chunk;
pub mod collision;
pub mod height_map;
pub mod manager;
pub mod roads;
pub mod terrain_background;
pub mod terrain_roads;
pub mod terrain_tracks;
pub mod terrain_visual;
pub mod textures;
pub mod tree_buffer;
pub mod vertex;
pub mod water;
pub mod water_tracks;

use glam::{Mat4, Vec3};
use std::collections::HashMap;
use thiserror::Error;

use crate::display::image::GameImageError;
use crate::system::SubsystemInterface;
use game_engine::common::ini::ini_terrain::TerrainError as IniTerrainError;

// Re-export main types for convenience
pub use chunk::{ChunkId, TerrainChunk};
pub use collision::TerrainCollision;
pub use height_map::HeightMap;
pub use manager::TerrainManager;
pub use roads::{Road, RoadSystem, RoadType};
pub use terrain_background::{
    IRegion2D, TerrainBackgroundCullStatus, TerrainBackgroundHeightMap, W3DTerrainBackground,
    TEX_1X, TEX_2X, TEX_4X,
};
pub use terrain_tracks::{
    compute_track_spacing, TerrainTrackHeightProvider, TerrainTrackLayer, TerrainTracksConfig,
    TerrainTracksRenderObjClassSystem, BRIDGE_OFFSET_FACTOR,
};
pub use textures::{BlendMode, TerrainTextures, TextureLayer};
pub use tree_buffer::{
    BreezeInfo, TreeCollisionUnit, TreeConstructionGeometry, TreeFxEvent, TreeFxKind,
    TreeGeometryType, TreeModuleData, TreeRandom, TreeRegion2D, TreeSaveRecord, TreeShroudStatus,
    TreeSphere, W3DToppleState, W3DTreeBuffer, ANGULAR_LIMIT, CONSTRUCTION_TREE_COLLISION_RADIUS,
    DELETED_TREE_TYPE, END_OF_PARTITION, MAX_TREES, MAX_TYPES, PARTITION_WIDTH_HEIGHT,
    TREE_RADIUS_APPROX, W3D_TOPPLE_OPTIONS_NONE, W3D_TOPPLE_OPTIONS_NO_BOUNCE,
    W3D_TOPPLE_OPTIONS_NO_FX,
};
pub use vertex::TerrainVertex;
pub use water::{WaterBody, WaterSettings, WaterSystem};
pub use water_tracks::{
    WaterTrackHeightProvider, WaterTrackSaveRecord, WaterTrackType, WaterTrackVertex,
    WaterTracksFlush, WaterTracksObj, WaterTracksRenderSystem, WATER_TRACK_WAVE_INFO,
};

/// Result type for terrain operations
pub type TerrainResult<T> = Result<T, TerrainError>;

/// Terrain system errors
#[derive(Error, Debug)]
pub enum TerrainError {
    #[error("Terrain initialization failed: {0}")]
    InitializationError(String),

    #[error("Heightmap loading error: {0}")]
    HeightmapError(String),

    #[error("Texture loading error: {0}")]
    TextureError(#[from] GameImageError),

    #[error("Mesh generation error: {0}")]
    MeshError(String),

    #[error("GPU resource error: {0}")]
    GPUError(String),

    #[error("Invalid terrain coordinates: ({x}, {y})")]
    InvalidCoordinates { x: f32, y: f32 },

    #[error("Terrain chunk not found: {0}")]
    ChunkNotFound(ChunkId),

    #[error("Road system error: {0}")]
    RoadError(String),

    #[error("Water system error: {0}")]
    WaterError(String),

    #[error("Invalid terrain data: {0}")]
    InvalidData(String),
}

impl From<IniTerrainError> for TerrainError {
    fn from(value: IniTerrainError) -> Self {
        TerrainError::InvalidData(value.to_string())
    }
}

/// Terrain configuration settings
#[derive(Debug, Clone)]
pub struct TerrainConfig {
    /// Terrain world dimensions
    pub world_size: (f32, f32),

    /// Heightmap resolution
    pub heightmap_resolution: (u32, u32),

    /// Maximum terrain height
    pub max_height: f32,

    /// Height scale factor
    pub height_scale: f32,

    /// Chunk size for LOD and culling
    pub chunk_size: u32,

    /// Number of texture layers
    pub max_texture_layers: usize,

    /// Enable/disable terrain features
    pub roads_enabled: bool,
    pub water_enabled: bool,
    pub deformation_enabled: bool,

    /// LOD settings
    pub lod_near_distance: f32,
    pub lod_medium_distance: f32,
    pub lod_far_distance: f32,

    /// Performance settings
    pub max_chunks_per_frame: usize,
    pub enable_frustum_culling: bool,
}

impl Default for TerrainConfig {
    fn default() -> Self {
        Self {
            world_size: (1024.0, 1024.0),
            heightmap_resolution: (512, 512),
            max_height: 100.0,
            height_scale: 1.0,
            chunk_size: 64,
            max_texture_layers: 4,
            roads_enabled: true,
            water_enabled: true,
            deformation_enabled: true,
            lod_near_distance: 100.0,
            lod_medium_distance: 300.0,
            lod_far_distance: 600.0,
            max_chunks_per_frame: 20,
            enable_frustum_culling: true,
        }
    }
}

/// Level of detail for terrain rendering
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainLOD {
    /// Full detail with all features
    High,
    /// Reduced triangle count and detail
    Medium,
    /// Low triangle count, basic texturing
    Low,
    /// No rendering (too far)
    None,
}

impl TerrainLOD {
    /// Get triangle count multiplier for this LOD level
    pub fn triangle_multiplier(self) -> f32 {
        match self {
            TerrainLOD::High => 1.0,
            TerrainLOD::Medium => 0.5,
            TerrainLOD::Low => 0.25,
            TerrainLOD::None => 0.0,
        }
    }

    /// Get texture detail level
    pub fn texture_detail(self) -> u32 {
        match self {
            TerrainLOD::High => 0,   // Full resolution
            TerrainLOD::Medium => 1, // Half resolution
            TerrainLOD::Low => 2,    // Quarter resolution
            TerrainLOD::None => 3,   // Minimal resolution
        }
    }
}

/// Calculate terrain LOD based on distance
pub fn calculate_terrain_lod(distance: f32, config: &TerrainConfig) -> TerrainLOD {
    if distance <= config.lod_near_distance {
        TerrainLOD::High
    } else if distance <= config.lod_medium_distance {
        TerrainLOD::Medium
    } else if distance <= config.lod_far_distance {
        TerrainLOD::Low
    } else {
        TerrainLOD::None
    }
}

/// Terrain modification operations for dynamic terrain
#[derive(Debug, Clone)]
pub enum TerrainModification {
    /// Raise terrain at position
    Raise {
        position: Vec3,
        radius: f32,
        strength: f32,
        falloff: f32,
    },
    /// Lower terrain at position
    Lower {
        position: Vec3,
        radius: f32,
        strength: f32,
        falloff: f32,
    },
    /// Flatten terrain to specific height
    Flatten {
        position: Vec3,
        radius: f32,
        target_height: f32,
        falloff: f32,
    },
    /// Create crater from explosion
    CreateCrater {
        position: Vec3,
        radius: f32,
        depth: f32,
    },
    /// Smooth terrain in area
    Smooth {
        position: Vec3,
        radius: f32,
        strength: f32,
    },
}

impl TerrainModification {
    /// Create an explosion crater
    pub fn explosion_crater(position: Vec3, explosion_radius: f32) -> Self {
        Self::CreateCrater {
            position,
            radius: explosion_radius * 0.8,
            depth: explosion_radius * 0.3,
        }
    }

    /// Create construction site flattening
    pub fn construction_site(position: Vec3, size: f32, height: f32) -> Self {
        Self::Flatten {
            position,
            radius: size,
            target_height: height,
            falloff: 0.8,
        }
    }
}

/// Terrain statistics for debugging and optimization
#[derive(Debug, Default)]
pub struct TerrainStats {
    /// Number of chunks loaded
    pub loaded_chunks: usize,

    /// Number of chunks rendered this frame
    pub rendered_chunks: usize,

    /// Number of chunks culled this frame
    pub culled_chunks: usize,

    /// Total triangles rendered
    pub triangles_rendered: usize,

    /// GPU memory usage in bytes
    pub gpu_memory_used: usize,

    /// CPU update time in milliseconds
    pub update_time_ms: f64,

    /// GPU render time in milliseconds
    pub render_time_ms: f64,

    /// Number of terrain modifications this frame
    pub modifications_applied: usize,
}

impl TerrainStats {
    /// Reset all statistics
    pub fn reset(&mut self) {
        self.rendered_chunks = 0;
        self.culled_chunks = 0;
        self.triangles_rendered = 0;
        self.modifications_applied = 0;
        // Keep loaded_chunks and gpu_memory_used as they persist
    }
}

/// Legacy terrain visual interface for compatibility
pub trait TerrainVisual: SubsystemInterface {
    /// Render terrain with given view parameters
    fn render(&mut self, view_matrix: &Mat4, projection_matrix: &Mat4) -> Result<(), TerrainError>;

    /// Get terrain height at world position
    fn get_height_at(&self, x: f32, y: f32) -> Result<f32, TerrainError>;

    /// Get terrain normal at world position
    fn get_normal_at(&self, x: f32, y: f32) -> Result<Vec3, TerrainError>;

    /// Check if position is valid terrain coordinate
    fn is_valid_position(&self, x: f32, y: f32) -> bool;

    /// Expose chunk manager for render passes.
    fn chunk_manager(&self) -> &crate::terrain::chunk::ChunkManager;

    /// Visible chunk count for stats.
    fn chunk_draw_count(&self) -> usize;

    /// Oversize the terrain mesh/visibility for script-driven map reveal effects.
    fn oversize_terrain(&mut self, _amount: i32) {}

    /// Forward C++ `setTerrainTracksDetail` into the owned terrain-track system.
    fn set_terrain_tracks_detail(&mut self) {}
}

/// Terrain utilities
pub mod utils {
    use super::*;

    /// Convert world coordinates to heightmap coordinates
    pub fn world_to_heightmap(
        world_pos: (f32, f32),
        world_size: (f32, f32),
        heightmap_resolution: (u32, u32),
    ) -> (u32, u32) {
        let u = (world_pos.0 / world_size.0) * heightmap_resolution.0 as f32;
        let v = (world_pos.1 / world_size.1) * heightmap_resolution.1 as f32;

        (
            u.clamp(0.0, heightmap_resolution.0 as f32 - 1.0) as u32,
            v.clamp(0.0, heightmap_resolution.1 as f32 - 1.0) as u32,
        )
    }

    /// Convert heightmap coordinates to world coordinates
    pub fn heightmap_to_world(
        heightmap_pos: (u32, u32),
        world_size: (f32, f32),
        heightmap_resolution: (u32, u32),
    ) -> (f32, f32) {
        let world_x = (heightmap_pos.0 as f32 / heightmap_resolution.0 as f32) * world_size.0;
        let world_y = (heightmap_pos.1 as f32 / heightmap_resolution.1 as f32) * world_size.1;

        (world_x, world_y)
    }

    /// Bilinear interpolation for height sampling
    pub fn bilinear_interpolate(
        heights: &[f32], // 2x2 array of heights
        u: f32,          // 0.0 to 1.0
        v: f32,          // 0.0 to 1.0
    ) -> f32 {
        let h00 = heights[0]; // Top-left
        let h10 = heights[1]; // Top-right
        let h01 = heights[2]; // Bottom-left
        let h11 = heights[3]; // Bottom-right

        let h0 = h00 * (1.0 - u) + h10 * u; // Top edge
        let h1 = h01 * (1.0 - u) + h11 * u; // Bottom edge

        h0 * (1.0 - v) + h1 * v // Final interpolation
    }

    /// Calculate terrain normal from heights
    pub fn calculate_normal(
        center_height: f32,
        left_height: f32,
        right_height: f32,
        up_height: f32,
        down_height: f32,
        scale: f32,
    ) -> Vec3 {
        let dx = (right_height - left_height) * scale;
        let dy = (up_height - down_height) * scale;

        Vec3::new(-dx, -dy, 2.0).normalize()
    }

    /// Apply falloff curve to terrain modification
    pub fn apply_falloff(distance: f32, radius: f32, falloff: f32) -> f32 {
        if distance >= radius {
            return 0.0;
        }

        let normalized_distance = distance / radius;
        let factor = 1.0 - normalized_distance;

        // Apply falloff curve (higher falloff = sharper edges)
        factor.powf(falloff)
    }
}

#[cfg(test)]
mod tests {
    use super::utils::*;
    use super::*;

    #[test]
    fn test_terrain_config_defaults() {
        let config = TerrainConfig::default();

        assert_eq!(config.world_size, (1024.0, 1024.0));
        assert_eq!(config.heightmap_resolution, (512, 512));
        assert_eq!(config.max_height, 100.0);
        assert_eq!(config.chunk_size, 64);
        assert!(config.roads_enabled);
        assert!(config.water_enabled);
    }

    #[test]
    fn test_terrain_lod_calculation() {
        let config = TerrainConfig::default();

        assert_eq!(calculate_terrain_lod(50.0, &config), TerrainLOD::High);
        assert_eq!(calculate_terrain_lod(200.0, &config), TerrainLOD::Medium);
        assert_eq!(calculate_terrain_lod(500.0, &config), TerrainLOD::Low);
        assert_eq!(calculate_terrain_lod(1000.0, &config), TerrainLOD::None);
    }

    #[test]
    fn test_terrain_lod_multipliers() {
        assert_eq!(TerrainLOD::High.triangle_multiplier(), 1.0);
        assert_eq!(TerrainLOD::Medium.triangle_multiplier(), 0.5);
        assert_eq!(TerrainLOD::Low.triangle_multiplier(), 0.25);
        assert_eq!(TerrainLOD::None.triangle_multiplier(), 0.0);

        assert_eq!(TerrainLOD::High.texture_detail(), 0);
        assert_eq!(TerrainLOD::Medium.texture_detail(), 1);
        assert_eq!(TerrainLOD::Low.texture_detail(), 2);
    }

    #[test]
    fn test_coordinate_conversion() {
        let world_size = (1024.0, 1024.0);
        let heightmap_resolution = (512, 512);

        // Test world to heightmap conversion
        let (hm_x, hm_y) = world_to_heightmap((512.0, 256.0), world_size, heightmap_resolution);
        assert_eq!(hm_x, 256);
        assert_eq!(hm_y, 128);

        // Test heightmap to world conversion
        let (world_x, world_y) = heightmap_to_world((256, 128), world_size, heightmap_resolution);
        assert_eq!(world_x, 512.0);
        assert_eq!(world_y, 256.0);
    }

    #[test]
    fn test_bilinear_interpolation() {
        let heights = [0.0, 10.0, 5.0, 15.0]; // 2x2 grid

        // Test corners
        assert_eq!(bilinear_interpolate(&heights, 0.0, 0.0), 0.0);
        assert_eq!(bilinear_interpolate(&heights, 1.0, 0.0), 10.0);
        assert_eq!(bilinear_interpolate(&heights, 0.0, 1.0), 5.0);
        assert_eq!(bilinear_interpolate(&heights, 1.0, 1.0), 15.0);

        // Test center (should be average)
        let center = bilinear_interpolate(&heights, 0.5, 0.5);
        assert!((center - 7.5).abs() < 0.001);
    }

    #[test]
    fn test_normal_calculation() {
        let normal = calculate_normal(5.0, 3.0, 7.0, 6.0, 4.0, 1.0);

        // Normal should point generally upward
        assert!(normal.z > 0.0);

        // Normal should be normalized
        assert!((normal.length() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_falloff_function() {
        // Test center (distance = 0)
        assert_eq!(apply_falloff(0.0, 10.0, 1.0), 1.0);

        // Test edge (distance = radius)
        assert_eq!(apply_falloff(10.0, 10.0, 1.0), 0.0);

        // Test beyond edge
        assert_eq!(apply_falloff(15.0, 10.0, 1.0), 0.0);

        // Test middle with different falloff values
        let mid1 = apply_falloff(5.0, 10.0, 1.0);
        let mid2 = apply_falloff(5.0, 10.0, 2.0);

        assert!(mid1 > 0.0 && mid1 < 1.0);
        assert!(mid2 < mid1); // Higher falloff = sharper curve
    }

    #[test]
    fn test_terrain_modifications() {
        let pos = Vec3::new(100.0, 200.0, 10.0);

        let crater = TerrainModification::explosion_crater(pos, 20.0);
        match crater {
            TerrainModification::CreateCrater { radius, depth, .. } => {
                assert_eq!(radius, 16.0); // 20.0 * 0.8
                assert_eq!(depth, 6.0); // 20.0 * 0.3
            }
            _ => panic!("Expected crater modification"),
        }

        let construction = TerrainModification::construction_site(pos, 30.0, 15.0);
        match construction {
            TerrainModification::Flatten {
                radius,
                target_height,
                ..
            } => {
                assert_eq!(radius, 30.0);
                assert_eq!(target_height, 15.0);
            }
            _ => panic!("Expected flatten modification"),
        }
    }

    #[test]
    fn test_terrain_stats() {
        let mut stats = TerrainStats::default();

        stats.rendered_chunks = 10;
        stats.triangles_rendered = 5000;

        stats.reset();

        assert_eq!(stats.rendered_chunks, 0);
        assert_eq!(stats.triangles_rendered, 0);
        // loaded_chunks should persist
    }
}
