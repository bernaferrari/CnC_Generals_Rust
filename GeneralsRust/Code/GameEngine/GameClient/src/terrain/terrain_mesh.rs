//! # Terrain Mesh System
//!
//! Handles terrain mesh generation, LOD management, and tile-based rendering.
//! Creates optimized triangle meshes from heightmap data with multiple detail levels.

use std::collections::HashMap;

use nalgebra::{Matrix4, Point3, Vector3};
use wgpu::{Buffer, Device, Queue, RenderPass};

use super::{
    calculate_terrain_lod, ChunkId, HeightMap, TerrainConfig, TerrainError, TerrainLOD,
    TerrainResult,
};

/// Tile size for terrain mesh generation
pub const TERRAIN_TILE_SIZE: u32 = 64;

/// Maximum LOD levels for terrain rendering
pub const MAX_TERRAIN_LOD_LEVELS: u8 = 5;

/// Terrain tile containing mesh data
#[derive(Debug)]
pub struct TerrainTile {
    /// Unique identifier
    pub id: u32,

    /// World position of tile center
    pub center: Vec3,

    /// Tile bounds in heightmap coordinates
    pub heightmap_bounds: (u32, u32, u32, u32), // min_x, min_y, max_x, max_y

    /// Current LOD level (0 = highest detail)
    pub lod_level: u8,

    /// Vertex data for different LOD levels
    pub vertex_data: [Option<Vec<TerrainMeshVertex>>; MAX_TERRAIN_LOD_LEVELS as usize],

    /// Index data for different LOD levels
    pub index_data: [Option<Vec<u32>>; MAX_TERRAIN_LOD_LEVELS as usize],

    /// GPU buffers for different LOD levels
    pub vertex_buffers: [Option<Buffer>; MAX_TERRAIN_LOD_LEVELS as usize],
    pub index_buffers: [Option<Buffer>; MAX_TERRAIN_LOD_LEVELS as usize],

    /// Tile is dirty and needs regeneration
    pub dirty: bool,

    /// Tile visibility status
    pub visible: bool,

    /// Distance from camera for LOD calculation
    pub camera_distance: f32,

    /// Bounding box for frustum culling
    pub bounds: (Vec3, Vec3),
}

impl TerrainTile {
    pub fn new(id: u32, heightmap_bounds: (u32, u32, u32, u32), heightmap: &HeightMap) -> Self {
        let (min_x, min_y, max_x, max_y) = heightmap_bounds;

        // Calculate world center
        let center_x = ((min_x + max_x) as f32 / 2.0) * heightmap.scale;
        let center_y = ((min_y + max_y) as f32 / 2.0) * heightmap.scale;
        let center_height = heightmap.get_height_at(center_x, center_y);

        let center = Vec3::new(center_x, center_y, center_height);
        let bounds = heightmap.calculate_bounds(min_x, min_y, max_x, max_y);

        Self {
            id,
            center,
            heightmap_bounds,
            lod_level: 0,
            vertex_data: [None, None, None, None, None],
            index_data: [None, None, None, None, None],
            vertex_buffers: [None, None, None, None, None],
            index_buffers: [None, None, None, None, None],
            dirty: true,
            visible: true,
            camera_distance: 0.0,
            bounds,
        }
    }

    /// Generate mesh data for all LOD levels
    pub fn generate_mesh_data(&mut self, heightmap: &HeightMap) -> TerrainResult<()> {
        let (min_x, min_y, max_x, max_y) = self.heightmap_bounds;

        for lod in 0..MAX_TERRAIN_LOD_LEVELS {
            let (vertices, indices) =
                self.generate_lod_mesh(heightmap, min_x, min_y, max_x, max_y, lod)?;
            self.vertex_data[lod as usize] = Some(vertices);
            self.index_data[lod as usize] = Some(indices);
        }

        self.dirty = false;
        Ok(())
    }

    /// Generate mesh for specific LOD level
    fn generate_lod_mesh(
        &self,
        heightmap: &HeightMap,
        min_x: u32,
        min_y: u32,
        max_x: u32,
        max_y: u32,
        lod_level: u8,
    ) -> TerrainResult<(Vec<TerrainMeshVertex>, Vec<u32>)> {
        let step = 1u32 << lod_level; // LOD step size: 1, 2, 4, 8, 16
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        // Calculate vertex grid dimensions
        let width_vertices = (max_x - min_x) / step + 1;
        let height_vertices = (max_y - min_y) / step + 1;

        // Generate vertices
        for grid_y in 0..height_vertices {
            for grid_x in 0..width_vertices {
                let hm_x = min_x + grid_x * step;
                let hm_y = min_y + grid_y * step;

                // Clamp to heightmap bounds
                let hm_x = hm_x.min(heightmap.width - 1);
                let hm_y = hm_y.min(heightmap.height - 1);

                let world_x = hm_x as f32 * heightmap.scale;
                let world_y = hm_y as f32 * heightmap.scale;
                let height = heightmap.get_height_at(world_x, world_y);
                let normal = heightmap.get_normal_at(world_x, world_y);

                // Calculate texture coordinates
                let tex_u = hm_x as f32 / heightmap.width as f32;
                let tex_v = hm_y as f32 / heightmap.height as f32;

                vertices.push(TerrainMeshVertex {
                    position: [world_x, world_y, height],
                    normal: [normal.x, normal.y, normal.z],
                    tex_coords: [tex_u, tex_v],
                    detail_coords: [0.0, 0.0],
                    blend_indices: [0; 4],
                    blend_weights: [0.0, 0.0, 0.0, 0.0],
                    color: [1.0, 1.0, 1.0, 1.0], // Default white
                });
            }
        }

        // Generate indices for triangles
        for grid_y in 0..height_vertices - 1 {
            for grid_x in 0..width_vertices - 1 {
                let base = grid_y * width_vertices + grid_x;

                // First triangle (top-left, bottom-left, top-right)
                indices.push(base);
                indices.push(base + width_vertices);
                indices.push(base + 1);

                // Second triangle (top-right, bottom-left, bottom-right)
                indices.push(base + 1);
                indices.push(base + width_vertices);
                indices.push(base + width_vertices + 1);
            }
        }

        Ok((vertices, indices))
    }

    /// Calculate texture blend weights based on terrain properties
    fn calculate_blend_weights(
        &self,
        _heightmap: &HeightMap,
        _hm_x: u32,
        _hm_y: u32,
        _height: f32,
    ) -> [f32; 4] {
        // Placeholder: Mesh builder defers to runtime blend rules.
        [0.0, 0.0, 0.0, 0.0]
    }

    /// Create GPU buffers for all LOD levels
    pub fn create_gpu_buffers(&mut self, device: &Device) -> TerrainResult<()> {
        for lod in 0..MAX_TERRAIN_LOD_LEVELS {
            let lod_idx = lod as usize;

            if let (Some(vertices), Some(indices)) =
                (&self.vertex_data[lod_idx], &self.index_data[lod_idx])
            {
                // Create vertex buffer
                let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("Terrain Tile {} LOD {} Vertices", self.id, lod)),
                    contents: bytemuck::cast_slice(vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });

                // Create index buffer
                let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("Terrain Tile {} LOD {} Indices", self.id, lod)),
                    contents: bytemuck::cast_slice(indices),
                    usage: wgpu::BufferUsages::INDEX,
                });

                self.vertex_buffers[lod_idx] = Some(vertex_buffer);
                self.index_buffers[lod_idx] = Some(index_buffer);
            }
        }

        Ok(())
    }

    /// Update LOD level based on camera distance
    pub fn update_lod(&mut self, camera_position: Vec3, config: &TerrainConfig) {
        // Calculate distance from camera to tile center
        self.camera_distance = (camera_position - self.center).norm();

        // Determine LOD level based on distance
        let terrain_lod = calculate_terrain_lod(self.camera_distance, config);

        self.lod_level = match terrain_lod {
            TerrainLOD::High => 0,
            TerrainLOD::Medium => 2,
            TerrainLOD::Low => 4,
            TerrainLOD::None => {
                self.visible = false;
                return;
            }
        };

        self.visible = true;
    }

    /// Render the tile at current LOD level
    pub fn render<'pass>(&self, render_pass: &mut RenderPass<'pass>) -> TerrainResult<()> {
        if !self.visible {
            return Ok(());
        }

        let lod_idx = self.lod_level as usize;

        if let (Some(vertex_buffer), Some(index_buffer)) =
            (&self.vertex_buffers[lod_idx], &self.index_buffers[lod_idx])
        {
            if let Some(indices) = &self.index_data[lod_idx] {
                render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..indices.len() as u32, 0, 0..1);
            }
        }

        Ok(())
    }

    /// Render depth-only version of the tile for pre-pass.
    pub fn render_depth<'pass>(&self, render_pass: &mut RenderPass<'pass>) -> TerrainResult<()> {
        if !self.visible {
            return Ok(());
        }

        let lod_idx = self.lod_level as usize;

        if let (Some(vertex_buffer), Some(index_buffer)) =
            (&self.vertex_buffers[lod_idx], &self.index_buffers[lod_idx])
        {
            if let Some(indices) = &self.index_data[lod_idx] {
                render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..indices.len() as u32, 0, 0..1);
            }
        }

        Ok(())
    }

    /// Check if tile is within view frustum
    pub fn is_visible(&self, view_matrix: &Mat4, projection_matrix: &Mat4) -> bool {
        // Simple bounding box frustum culling
        let mvp = projection_matrix * view_matrix;

        // Transform bounding box corners
        let corners = [
            Vec3::new(self.bounds.0.x, self.bounds.0.y, self.bounds.0.z), // min corner
            Vec3::new(self.bounds.1.x, self.bounds.1.y, self.bounds.1.z), // max corner
            Vec3::new(self.bounds.0.x, self.bounds.1.y, self.bounds.0.z),
            Vec3::new(self.bounds.1.x, self.bounds.0.y, self.bounds.0.z),
            Vec3::new(self.bounds.0.x, self.bounds.0.y, self.bounds.1.z),
            Vec3::new(self.bounds.1.x, self.bounds.1.y, self.bounds.0.z),
            Vec3::new(self.bounds.0.x, self.bounds.1.y, self.bounds.1.z),
            Vec3::new(self.bounds.1.x, self.bounds.0.y, self.bounds.1.z),
        ];

        let mut all_outside = true;

        for corner in corners.iter() {
            let clip_pos = mvp * corner.to_homogeneous();
            let w = clip_pos.w;

            // Check if point is inside NDC cube
            if clip_pos.x >= -w
                && clip_pos.x <= w
                && clip_pos.y >= -w
                && clip_pos.y <= w
                && clip_pos.z >= 0.0
                && clip_pos.z <= w
            {
                all_outside = false;
                break;
            }
        }

        !all_outside
    }
}

/// Terrain mesh manager
#[derive(Debug)]
pub struct TerrainMeshManager {
    /// All terrain tiles
    tiles: HashMap<u32, TerrainTile>,

    /// Next tile ID
    next_tile_id: u32,

    /// Tile generation parameters
    tile_size: u32,

    /// GPU device reference
    device: Option<Device>,
}

impl TerrainMeshManager {
    pub fn new() -> Self {
        Self {
            tiles: HashMap::new(),
            next_tile_id: 1,
            tile_size: TERRAIN_TILE_SIZE,
            device: None,
        }
    }

    /// Initialize with GPU device
    pub fn init_gpu(&mut self, device: &Device) {
        self.device = Some(device.clone());
    }

    /// Generate terrain tiles from heightmap
    pub fn generate_tiles(&mut self, heightmap: &HeightMap) -> TerrainResult<()> {
        self.tiles.clear();
        self.next_tile_id = 1;

        let tiles_x = (heightmap.width + self.tile_size - 1) / self.tile_size;
        let tiles_y = (heightmap.height + self.tile_size - 1) / self.tile_size;

        log::info!("Generating {}x{} terrain tiles", tiles_x, tiles_y);

        for tile_y in 0..tiles_y {
            for tile_x in 0..tiles_x {
                let min_x = tile_x * self.tile_size;
                let min_y = tile_y * self.tile_size;
                let max_x = ((tile_x + 1) * self.tile_size).min(heightmap.width - 1);
                let max_y = ((tile_y + 1) * self.tile_size).min(heightmap.height - 1);

                let tile_id = self.next_tile_id;
                self.next_tile_id += 1;

                let mut tile = TerrainTile::new(tile_id, (min_x, min_y, max_x, max_y), heightmap);

                // Generate mesh data for all LOD levels
                tile.generate_mesh_data(heightmap)?;

                // Create GPU buffers if device is available
                if let Some(ref device) = self.device {
                    tile.create_gpu_buffers(device)?;
                }

                self.tiles.insert(tile_id, tile);
            }
        }

        log::info!("Generated {} terrain tiles", self.tiles.len());
        Ok(())
    }

    /// Update all tiles (LOD, visibility, etc.)
    pub fn update(
        &mut self,
        camera_position: Vec3,
        view_matrix: &Mat4,
        projection_matrix: &Mat4,
        config: &TerrainConfig,
    ) {
        let mut visible_count = 0;

        for tile in self.tiles.values_mut() {
            // Update LOD based on camera distance
            tile.update_lod(camera_position, config);

            // Update visibility based on frustum culling
            if tile.visible && config.enable_frustum_culling {
                tile.visible = tile.is_visible(view_matrix, projection_matrix);
            }

            if tile.visible {
                visible_count += 1;
            }
        }

        log::debug!(
            "Terrain tiles: {} visible out of {}",
            visible_count,
            self.tiles.len()
        );
    }

    /// Render all visible tiles
    pub fn render<'pass>(&self, render_pass: &mut RenderPass<'pass>) -> TerrainResult<()> {
        let mut rendered_tiles = 0;

        // Sort tiles by LOD and distance for optimal rendering
        let mut visible_tiles: Vec<&TerrainTile> =
            self.tiles.values().filter(|tile| tile.visible).collect();

        visible_tiles.sort_by(|a, b| {
            // First by LOD level (render high detail first)
            match a.lod_level.cmp(&b.lod_level) {
                std::cmp::Ordering::Equal => {
                    // Then by distance (closer first)
                    a.camera_distance
                        .partial_cmp(&b.camera_distance)
                        .unwrap_or(std::cmp::Ordering::Equal)
                }
                other => other,
            }
        });

        // Render tiles
        for tile in visible_tiles {
            tile.render(render_pass)?;
            rendered_tiles += 1;
        }

        log::trace!("Rendered {} terrain tiles", rendered_tiles);
        Ok(())
    }

    /// Render visible tiles depth-only.
    pub fn render_depth_only<'pass>(
        &self,
        render_pass: &mut RenderPass<'pass>,
    ) -> TerrainResult<()> {
        let mut rendered_tiles = 0;
        for tile in self.tiles.values().filter(|tile| tile.visible) {
            tile.render_depth(render_pass)?;
            rendered_tiles += 1;
        }
        log::trace!("Rendered {} terrain tiles (depth)", rendered_tiles);
        Ok(())
    }

    /// Mark region as dirty (needs regeneration)
    pub fn mark_region_dirty(
        &mut self,
        min_world_x: f32,
        min_world_y: f32,
        max_world_x: f32,
        max_world_y: f32,
    ) {
        for tile in self.tiles.values_mut() {
            // Check if tile overlaps with dirty region
            if tile.bounds.0.x <= max_world_x
                && tile.bounds.1.x >= min_world_x
                && tile.bounds.0.y <= max_world_y
                && tile.bounds.1.y >= min_world_y
            {
                tile.dirty = true;
            }
        }
    }

    /// Regenerate dirty tiles
    pub fn regenerate_dirty_tiles(&mut self, heightmap: &HeightMap) -> TerrainResult<()> {
        let mut regenerated_count = 0;

        for tile in self.tiles.values_mut() {
            if tile.dirty {
                // Regenerate mesh data
                tile.generate_mesh_data(heightmap)?;

                // Recreate GPU buffers
                if let Some(ref device) = self.device {
                    tile.create_gpu_buffers(device)?;
                }

                tile.dirty = false;
                regenerated_count += 1;
            }
        }

        if regenerated_count > 0 {
            log::info!("Regenerated {} terrain tiles", regenerated_count);
        }

        Ok(())
    }

    /// Get terrain statistics
    pub fn get_statistics(&self) -> TerrainMeshStats {
        let mut total_vertices = 0;
        let mut total_triangles = 0;
        let mut visible_tiles = 0;

        for tile in self.tiles.values() {
            if tile.visible {
                visible_tiles += 1;

                let lod_idx = tile.lod_level as usize;
                if let (Some(vertices), Some(indices)) =
                    (&tile.vertex_data[lod_idx], &tile.index_data[lod_idx])
                {
                    total_vertices += vertices.len();
                    total_triangles += indices.len() / 3;
                }
            }
        }

        TerrainMeshStats {
            total_tiles: self.tiles.len(),
            visible_tiles,
            total_vertices,
            total_triangles,
            tile_size: self.tile_size,
        }
    }
}

impl Default for TerrainMeshManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Enhanced terrain mesh vertex
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TerrainMeshVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tex_coords: [f32; 2], // Main texture coordinates (0-1 across heightmap)
    pub detail_coords: [f32; 2], // Detail texture coordinates (tiled)
    pub blend_indices: [u16; 4], // Texture array indices
    pub blend_weights: [f32; 4], // Texture blending weights
    pub color: [f32; 4],      // Vertex color for additional shading
}

impl TerrainMeshVertex {
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<TerrainMeshVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // Position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Normal
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Texture coordinates
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 6]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // Detail coordinates
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // Blend indices
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 10]>() as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Uint16x4,
                },
                // Blend weights
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 12]>() as wgpu::BufferAddress,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // Color
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 16]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

unsafe impl bytemuck::Pod for TerrainMeshVertex {}
unsafe impl bytemuck::Zeroable for TerrainMeshVertex {}

/// Terrain mesh statistics
#[derive(Debug, Clone)]
pub struct TerrainMeshStats {
    pub total_tiles: usize,
    pub visible_tiles: usize,
    pub total_vertices: usize,
    pub total_triangles: usize,
    pub tile_size: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::HeightMap;

    #[test]
    fn test_terrain_tile_creation() {
        let heightmap = HeightMap::new(128, 128, 100.0, 1.0);
        let tile = TerrainTile::new(1, (0, 0, 63, 63), &heightmap);

        assert_eq!(tile.id, 1);
        assert_eq!(tile.heightmap_bounds, (0, 0, 63, 63));
        assert!(tile.dirty);
        assert!(tile.visible);
    }

    #[test]
    fn test_mesh_generation() {
        let heightmap = HeightMap::new(64, 64, 100.0, 1.0);
        let mut tile = TerrainTile::new(1, (0, 0, 31, 31), &heightmap);

        tile.generate_mesh_data(&heightmap).unwrap();

        // Check that all LOD levels were generated
        for lod in 0..MAX_TERRAIN_LOD_LEVELS {
            assert!(tile.vertex_data[lod as usize].is_some());
            assert!(tile.index_data[lod as usize].is_some());
        }

        // LOD 0 should have more vertices than LOD 4
        let lod0_vertices = tile.vertex_data[0].as_ref().unwrap().len();
        let lod4_vertices = tile.vertex_data[4].as_ref().unwrap().len();
        assert!(lod0_vertices > lod4_vertices);
    }

    #[test]
    fn test_blend_weight_calculation() {
        let heightmap = HeightMap::new(32, 32, 100.0, 1.0);
        let tile = TerrainTile::new(1, (0, 0, 15, 15), &heightmap);

        let weights = tile.calculate_blend_weights(&heightmap, 8, 8, 50.0);

        assert!(weights.iter().all(|w| *w == 0.0));
    }

    #[test]
    fn test_terrain_mesh_manager() {
        let heightmap = HeightMap::new(128, 128, 100.0, 1.0);
        let mut manager = TerrainMeshManager::new();

        manager.generate_tiles(&heightmap).unwrap();

        // Should generate 2x2 = 4 tiles for 128x128 heightmap with 64x64 tile size
        let expected_tiles = ((128 + 63) / 64) * ((128 + 63) / 64);
        assert_eq!(manager.tiles.len(), expected_tiles);
    }
}
