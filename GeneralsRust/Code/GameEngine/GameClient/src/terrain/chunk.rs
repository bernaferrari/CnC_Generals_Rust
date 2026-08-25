//! Terrain Chunk System
//!
//! Manages terrain in spatial chunks for efficient rendering and level-of-detail.
//! This system divides the terrain into manageable pieces for culling, LOD,
//! and streaming optimizations.

use crate::fx_list::{do_the_dynamic_light, scene_dynamic_lights};
use crate::terrain::{
    HeightMap, TerrainConfig, TerrainError, TerrainLOD, TerrainModification, TerrainResult,
    TerrainVertex, calculate_terrain_lod,
};
use glam::{Mat4, Vec3, Vec4};
use std::collections::HashMap;
use std::time::Instant;
use wgpu::RenderPass;

fn bgra_u32_to_rgba_f32(packed: u32) -> [f32; 4] {
    let b = (packed & 0xFF) as f32 / 255.0;
    let g = ((packed >> 8) & 0xFF) as f32 / 255.0;
    let r = ((packed >> 16) & 0xFF) as f32 / 255.0;
    let a = ((packed >> 24) & 0xFF) as f32 / 255.0;
    [r, g, b, a]
}

fn rgba_f32_to_bgra_u32(color: [f32; 4]) -> u32 {
    let r = (color[0].clamp(0.0, 1.0) * 255.0) as u32;
    let g = (color[1].clamp(0.0, 1.0) * 255.0) as u32;
    let b = (color[2].clamp(0.0, 1.0) * 255.0) as u32;
    let a = (color[3].clamp(0.0, 1.0) * 255.0) as u32;
    b | (g << 8) | (r << 16) | (a << 24)
}

fn falloff_weight(distance: f32, radius: f32, falloff: f32) -> f32 {
    if radius <= 0.0 {
        return 0.0;
    }

    let normalized = 1.0 - (distance / radius).clamp(0.0, 1.0);
    if normalized <= 0.0 {
        return 0.0;
    }

    if falloff <= 0.0 {
        normalized
    } else {
        normalized.powf(falloff.max(1.0))
    }
}

/// C++ `TheGlobalData` sun used by `getStaticDiffuse` / `doTheLight`.
/// Light position is C++ Z-up; chunk normals are wgpu Y-up (`x,z,y`).
fn static_terrain_light_params() -> (Vec3, [f32; 3], [f32; 3]) {
    if let Some(global) = game_engine::common::ini::get_global_data() {
        let data = global.read();
        let pos = data.terrain_light_pos[0];
        return (
            Vec3::new(pos.x, pos.z, pos.y),
            [
                data.terrain_diffuse[0].r,
                data.terrain_diffuse[0].g,
                data.terrain_diffuse[0].b,
            ],
            [
                data.terrain_ambient[0].r,
                data.terrain_ambient[0].g,
                data.terrain_ambient[0].b,
            ],
        );
    }
    let data = game_engine::common::global_data::read();
    let pos = data.terrain_light_pos[0];
    (
        Vec3::new(pos[0], pos[2], pos[1]),
        data.terrain_diffuse[0],
        data.terrain_ambient[0],
    )
}

/// C++ `BaseHeightMapRenderObjClass::getStaticDiffuse` analog.
/// Same N·L as roads (`terrain_static_diffuse_from_normal`) plus a 0.35 floor.
fn static_diffuse_from_normal(normal: Vec3) -> [f32; 4] {
    const COLOR_FLOOR: f32 = 0.35;
    let (light_pos, sun_color, ambient_color) = static_terrain_light_params();
    let mut color = super::terrain_visual::TerrainVisualImpl::terrain_static_diffuse_from_normal(
        normal,
        light_pos,
        sun_color,
        ambient_color,
    );
    color[0] = color[0].max(COLOR_FLOOR);
    color[1] = color[1].max(COLOR_FLOOR);
    color[2] = color[2].max(COLOR_FLOOR);
    color
}

/// Unique identifier for terrain chunks
pub type ChunkId = u32;

/// Size of terrain chunks in world units
pub const CHUNK_SIZE: f32 = 128.0;

/// Maximum LOD level (0 = highest detail)
pub const MAX_LOD_LEVEL: u8 = 4;

/// Terrain chunk containing geometry and rendering data
#[derive(Debug, Clone)]
pub struct TerrainChunk {
    /// Unique identifier for this chunk
    pub id: ChunkId,

    /// World position of chunk center
    pub position: Vec3,

    /// Current level of detail (0 = highest)
    pub lod_level: u8,

    /// Chunk dimensions in world units
    pub size: f32,

    /// Heightmap data for this chunk
    pub heights: Vec<Vec<f32>>,

    /// Vertex data for rendering
    pub vertices: Vec<TerrainVertex>,

    /// Pre-dynamic VB diffuse (C++ `vbMirror`) used each `doTheDynamicLight` bake.
    pub base_colors: Vec<[f32; 4]>,

    /// Index data for triangle rendering
    pub indices: Vec<u32>,

    /// Texture blend weights for multi-texturing
    pub texture_weights: Vec<[f32; 4]>,

    /// Whether this chunk needs geometry regeneration
    pub dirty: bool,

    /// Whether this chunk is currently visible
    pub visible: bool,

    /// Distance from camera (for LOD calculations)
    pub camera_distance: f32,

    /// Bounding box for frustum culling
    pub bounds: ChunkBounds,

    /// Performance metrics
    pub stats: ChunkStats,

    /// Monotonic geometry revision that increments whenever vertices/indices change
    pub geometry_revision: u64,
}

/// Bounding box for terrain chunks
#[derive(Debug, Clone)]
pub struct ChunkBounds {
    pub min: Vec3,
    pub max: Vec3,
}

/// Performance statistics for terrain chunks
#[derive(Debug, Clone)]
pub struct ChunkStats {
    pub vertex_count: u32,
    pub triangle_count: u32,
    pub texture_memory: u64,
    pub last_render_time: std::time::Instant,
    pub render_count: u64,
}

impl Default for ChunkStats {
    fn default() -> Self {
        Self {
            vertex_count: 0,
            triangle_count: 0,
            texture_memory: 0,
            last_render_time: std::time::Instant::now(),
            render_count: 0,
        }
    }
}

/// Manages collection of terrain chunks
#[derive(Debug)]
pub struct ChunkManager {
    /// All terrain chunks indexed by ID
    chunks: HashMap<ChunkId, TerrainChunk>,

    /// Spatial grid for fast chunk lookup
    spatial_grid: HashMap<(i32, i32), ChunkId>,

    /// Configuration for chunk management
    config: TerrainConfig,

    /// Next available chunk ID
    next_chunk_id: ChunkId,

    /// Camera position for LOD calculations
    camera_position: Vec3,

    /// View frustum for culling
    view_frustum: ViewFrustum,

    /// Performance statistics
    stats: ChunkManagerStats,
}

/// View frustum for chunk culling
#[derive(Debug, Clone)]
pub struct ViewFrustum {
    pub planes: [Vec3; 6], // Left, Right, Top, Bottom, Near, Far
    pub view_matrix: Mat4,
    pub projection_matrix: Mat4,
}

/// Performance statistics for chunk manager
#[derive(Debug, Default)]
pub struct ChunkManagerStats {
    pub total_chunks: u32,
    pub visible_chunks: u32,
    pub rendered_chunks: u32,
    pub lod_transitions: u64,
    pub geometry_updates: u64,
    pub culling_time: std::time::Duration,
    pub update_time: std::time::Duration,
}

impl TerrainChunk {
    /// Create a new terrain chunk
    pub fn new(id: ChunkId, position: Vec3, size: f32) -> Self {
        let bounds = ChunkBounds {
            min: Vec3::new(
                position.x - size / 2.0,
                0.0, // Will be updated based on heightmap
                position.z - size / 2.0,
            ),
            max: Vec3::new(
                position.x + size / 2.0,
                0.0, // Will be updated based on heightmap
                position.z + size / 2.0,
            ),
        };

        Self {
            id,
            position,
            lod_level: 0,
            size,
            heights: Vec::new(),
            vertices: Vec::new(),
            base_colors: Vec::new(),
            indices: Vec::new(),
            texture_weights: Vec::new(),
            dirty: true,
            visible: false,
            camera_distance: 0.0,
            bounds,
            stats: ChunkStats::default(),
            geometry_revision: 0,
        }
    }

    /// Generate geometry for this chunk based on heightmap
    pub fn generate_geometry(&mut self, resolution: u32) -> TerrainResult<()> {
        if self.heights.is_empty() {
            return Err(TerrainError::InvalidData(
                "No heightmap data available".to_string(),
            ));
        }

        self.vertices.clear();
        self.base_colors.clear();
        self.indices.clear();

        let step = self.size / (resolution as f32 - 1.0);
        let half_size = self.size / 2.0;

        // Generate vertices
        for z in 0..resolution {
            for x in 0..resolution {
                let tex_u = x as f32 / (resolution - 1) as f32;
                let tex_v = z as f32 / (resolution - 1) as f32;
                let world_x = self.position.x - half_size + (x as f32 * step);
                let world_z = self.position.z - half_size + (z as f32 * step);

                // Sample the chunk-local height field continuously. The populated height grid
                // matches the source terrain sample density, not the requested mesh resolution.
                let height = self.sample_height_bilinear(tex_u, tex_v);

                // Calculate normal using central differences so lighting matches the legacy renderer.
                let normal = self.compute_normal(tex_u, tex_v, step);

                // C++ HeightMap.cpp: vb->diffuse = getStaticDiffuse(x,y) then doTheDynamicLight.
                let base_color = static_diffuse_from_normal(normal);
                self.base_colors.push(base_color);
                self.vertices.push(TerrainVertex::from_components(
                    Vec3::new(world_x, height, world_z),
                    normal,
                    (tex_u, tex_v),
                    [0; 4],
                    [0.0; 4],
                    base_color,
                ));
            }
        }

        // C++ cell split p0→p2 (BaseHeightMap.cpp):
        //   p3 ----- p2
        //    |    /  |
        //    |  /    |
        //   p0 ----- p1
        for z in 0..(resolution - 1) {
            for x in 0..(resolution - 1) {
                let i = z * resolution + x;
                let p0 = i;
                let p1 = i + 1;
                let p3 = i + resolution;
                let p2 = i + resolution + 1;
                self.indices.push(p0);
                self.indices.push(p1);
                self.indices.push(p2);
                self.indices.push(p0);
                self.indices.push(p2);
                self.indices.push(p3);
            }
        }

        // Update bounding box
        self.update_bounds();

        // C++ HeightMap::doTheDynamicLight bakes pulse lights into VB diffuse.
        self.apply_dynamic_lights();

        // Update statistics
        self.stats.vertex_count = self.vertices.len() as u32;
        self.stats.triangle_count = self.indices.len() as u32 / 3;
        self.geometry_revision = self.geometry_revision.wrapping_add(1);

        self.dirty = false;
        Ok(())
    }

    /// C++ per-vertex dynamic light bake (`doTheDynamicLight`) into chunk VB colours.
    /// Starts from sun-lit `base_colors` (`getStaticDiffuse` / `vbMirror`) then adds pulses.
    /// Map coords are C++ Z-up: `(x, y, z) = (wgpu.x, wgpu.z, wgpu.y)`.
    pub fn apply_dynamic_lights(&mut self) {
        let lights = scene_dynamic_lights();
        for (i, vertex) in self.vertices.iter_mut().enumerate() {
            let base = self
                .base_colors
                .get(i)
                .copied()
                .unwrap_or([1.0, 1.0, 1.0, 1.0]);
            let base_packed = rgba_f32_to_bgra_u32(base);
            let xyz = [vertex.position[0], vertex.position[2], vertex.position[1]];
            let normal = [vertex.normal[0], vertex.normal[2], vertex.normal[1]];
            let packed = if lights.is_empty() {
                base_packed
            } else {
                do_the_dynamic_light(xyz, normal, base_packed, &lights)
            };
            vertex.color = bgra_u32_to_rgba_f32(packed);
        }
    }

    /// C++ HeightMap fragment: texture sample * vb.diffuse (already lit).
    #[must_use]
    pub fn modulate_terrain_sample_by_vb_color(
        sample_rgb: [f32; 3],
        vb_color: [f32; 4],
    ) -> [f32; 3] {
        [
            (sample_rgb[0] * vb_color[0]).clamp(0.0, 1.0),
            (sample_rgb[1] * vb_color[1]).clamp(0.0, 1.0),
            (sample_rgb[2] * vb_color[2]).clamp(0.0, 1.0),
        ]
    }

    fn sample_height_grid(&self, x: usize, z: usize) -> f32 {
        if self.heights.is_empty() {
            return 0.0;
        }

        let z_idx = z.min(self.heights.len() - 1);
        let x_idx = x.min(self.heights[z_idx].len() - 1);
        self.heights[z_idx][x_idx]
    }

    fn sample_height_bilinear(&self, u: f32, v: f32) -> f32 {
        if self.heights.is_empty() {
            return 0.0;
        }

        let max_z = self.heights.len().saturating_sub(1);
        let max_x = self.heights[0].len().saturating_sub(1);
        let sample_x = u.clamp(0.0, 1.0) * max_x as f32;
        let sample_z = v.clamp(0.0, 1.0) * max_z as f32;
        let x0 = sample_x.floor() as usize;
        let z0 = sample_z.floor() as usize;
        let x1 = (x0 + 1).min(max_x);
        let z1 = (z0 + 1).min(max_z);
        let tx = sample_x - x0 as f32;
        let tz = sample_z - z0 as f32;

        let h00 = self.sample_height_grid(x0, z0); // p0
        let h10 = self.sample_height_grid(x1, z0); // p1
        let h01 = self.sample_height_grid(x0, z1); // p3
        let h11 = self.sample_height_grid(x1, z1); // p2
        // Same plane split as HeightMap::get_height_at / C++ fy > fx upper triangle.
        if tz > tx {
            h01 + (1.0 - tz) * (h00 - h01) + tx * (h11 - h01)
        } else {
            h10 + tz * (h11 - h10) + (1.0 - tx) * (h00 - h10)
        }
    }

    fn compute_normal(&self, u: f32, v: f32, step: f32) -> Vec3 {
        let texel_x = if self.heights.first().map_or(0, Vec::len) > 1 {
            1.0 / (self.heights[0].len() as f32 - 1.0)
        } else {
            1.0
        };
        let texel_z = if self.heights.len() > 1 {
            1.0 / (self.heights.len() as f32 - 1.0)
        } else {
            1.0
        };

        let h_l = self.sample_height_bilinear(u - texel_x, v);
        let h_r = self.sample_height_bilinear(u + texel_x, v);
        let h_u = self.sample_height_bilinear(u, v - texel_z);
        let h_d = self.sample_height_bilinear(u, v + texel_z);

        // Approximate the partial derivatives using central differences.
        let step = step.max(f32::EPSILON);
        let dx = (h_r - h_l) / (2.0 * step);
        let dz = (h_d - h_u) / (2.0 * step);

        Vec3::new(-dx, 1.0, -dz).normalize()
    }

    /// Update LOD level based on camera distance
    pub fn update_lod(&mut self, camera_position: &Vec3, config: &TerrainConfig) {
        self.camera_distance = self.position.distance(*camera_position);

        // Keep chunk mesh density aligned with the terrain system's configured LOD bands
        // instead of using a second unrelated hardcoded policy.
        let new_lod = match calculate_terrain_lod(self.camera_distance, config) {
            TerrainLOD::High => 0,
            TerrainLOD::Medium => 1,
            TerrainLOD::Low => 2,
            TerrainLOD::None => {
                if self.camera_distance <= config.lod_far_distance * 2.0 {
                    3
                } else {
                    4
                }
            }
        }
        .min(2);

        if new_lod != self.lod_level {
            self.lod_level = new_lod;
            self.dirty = true; // Need to regenerate geometry
        }
    }

    /// Update bounding box based on current vertices
    fn update_bounds(&mut self) {
        if self.vertices.is_empty() {
            return;
        }

        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;

        for vertex in &self.vertices {
            let position = vertex.position();
            min_y = min_y.min(position.y);
            max_y = max_y.max(position.y);
        }

        self.bounds.min.y = min_y;
        self.bounds.max.y = max_y;
    }

    /// Check if chunk is visible in view frustum
    pub fn is_visible(&self, frustum: &ViewFrustum) -> bool {
        let view_proj = frustum.projection_matrix * frustum.view_matrix;
        let min = self.bounds.min;
        let max = self.bounds.max;
        // Degenerate Y=0 slabs (pre-geometry) sit on the ground plane and
        // must still pass; inflate so a flat AABB is not a zero-thickness test.
        let min_y = min.y.min(0.0);
        let max_y = max.y.max(min_y + 64.0);

        let corners = [
            Vec3::new(min.x, min_y, min.z),
            Vec3::new(max.x, min_y, min.z),
            Vec3::new(min.x, max_y, min.z),
            Vec3::new(max.x, max_y, min.z),
            Vec3::new(min.x, min_y, max.z),
            Vec3::new(max.x, min_y, max.z),
            Vec3::new(min.x, max_y, max.z),
            Vec3::new(max.x, max_y, max.z),
        ];

        let mut left_out = 0;
        let mut right_out = 0;
        let mut bottom_out = 0;
        let mut top_out = 0;
        let mut near_out = 0;
        let mut far_out = 0;

        for corner in corners.iter() {
            let clip = view_proj * Vec4::new(corner.x, corner.y, corner.z, 1.0);

            if clip.x < -clip.w {
                left_out += 1;
            }
            if clip.x > clip.w {
                right_out += 1;
            }
            if clip.y < -clip.w {
                bottom_out += 1;
            }
            if clip.y > clip.w {
                top_out += 1;
            }
            if clip.z < 0.0 {
                near_out += 1;
            }
            if clip.z > clip.w {
                far_out += 1;
            }
        }

        if left_out != 8
            && right_out != 8
            && bottom_out != 8
            && top_out != 8
            && near_out != 8
            && far_out != 8
        {
            return true;
        }

        // C++ HeightMap draws a window around the camera. If the clip test
        // rejects everything (identity/wrong clip space), keep chunks under
        // the look-at so Alpine is not a blue void.
        let camera = frustum.view_matrix.inverse().transform_point3(Vec3::ZERO);
        let pad = (max.x - min.x).abs().max((max.z - min.z).abs()).max(512.0);
        camera.x + pad >= min.x
            && camera.x - pad <= max.x
            && camera.z + pad >= min.z
            && camera.z - pad <= max.z
    }

    /// Apply terrain modification to this chunk
    pub fn apply_modification(&mut self, modification: &TerrainModification) -> TerrainResult<()> {
        match modification {
            TerrainModification::Raise {
                position,
                radius,
                strength,
                falloff,
            } => {
                self.apply_height_modification(position, *radius, *strength, *falloff)?;
            }
            TerrainModification::Lower {
                position,
                radius,
                strength,
                falloff,
            } => {
                self.apply_height_modification(position, *radius, -*strength, *falloff)?;
            }
            TerrainModification::Flatten {
                position,
                radius,
                target_height,
                falloff,
            } => {
                self.apply_flatten_modification(position, *radius, *target_height, *falloff)?;
            }
            TerrainModification::CreateCrater {
                position,
                radius,
                depth,
            } => {
                self.apply_crater_modification(position, *radius, *depth)?;
            }
            TerrainModification::Smooth {
                position,
                radius,
                strength,
            } => {
                self.apply_smooth_modification(position, *radius, *strength)?;
            }
        }

        self.dirty = true;
        Ok(())
    }

    /// Apply height modification (raise/lower)
    fn apply_height_modification(
        &mut self,
        position: &Vec3,
        radius: f32,
        strength: f32,
        falloff: f32,
    ) -> TerrainResult<()> {
        if radius <= 0.0 || self.heights.is_empty() || self.heights[0].is_empty() {
            return Ok(());
        }

        let radius_sq = radius * radius;
        let (rows, cols, step_x, step_z) = self.grid_metrics();

        let mut affected = false;
        for row in 0..rows {
            let world_z = self.bounds.min.z + step_z * row as f32;

            for col in 0..cols {
                let world_x = self.bounds.min.x + step_x * col as f32;

                let dx = world_x - position.x;
                let dz = world_z - position.z;
                let distance_sq = dx * dx + dz * dz;

                if distance_sq > radius_sq {
                    continue;
                }

                let distance = distance_sq.sqrt();
                let weight = falloff_weight(distance, radius, falloff);
                if weight <= 0.0 {
                    continue;
                }

                self.heights[row][col] += strength * weight;
                affected = true;
            }
        }

        if affected {
            self.recompute_vertical_bounds();
            self.dirty = true;
        }

        Ok(())
    }

    /// Apply flatten modification
    fn apply_flatten_modification(
        &mut self,
        position: &Vec3,
        radius: f32,
        target_height: f32,
        falloff: f32,
    ) -> TerrainResult<()> {
        if radius <= 0.0 || self.heights.is_empty() || self.heights[0].is_empty() {
            return Ok(());
        }

        let radius_sq = radius * radius;
        let (rows, cols, step_x, step_z) = self.grid_metrics();
        let mut affected = false;

        for row in 0..rows {
            let world_z = self.bounds.min.z + step_z * row as f32;

            for col in 0..cols {
                let world_x = self.bounds.min.x + step_x * col as f32;

                let dx = world_x - position.x;
                let dz = world_z - position.z;
                let distance_sq = dx * dx + dz * dz;

                if distance_sq > radius_sq {
                    continue;
                }

                let distance = distance_sq.sqrt();
                let weight = falloff_weight(distance, radius, falloff);
                if weight <= 0.0 {
                    continue;
                }

                let current = self.heights[row][col];
                self.heights[row][col] = current + (target_height - current) * weight;
                affected = true;
            }
        }

        if affected {
            self.recompute_vertical_bounds();
            self.dirty = true;
        }

        Ok(())
    }

    /// Apply crater modification
    fn apply_crater_modification(
        &mut self,
        position: &Vec3,
        radius: f32,
        depth: f32,
    ) -> TerrainResult<()> {
        let depth = depth.abs();
        self.apply_height_modification(position, radius, -depth, 2.0)
    }

    /// Apply smooth modification
    fn apply_smooth_modification(
        &mut self,
        position: &Vec3,
        radius: f32,
        strength: f32,
    ) -> TerrainResult<()> {
        if radius <= 0.0 || self.heights.is_empty() || self.heights[0].is_empty() {
            return Ok(());
        }

        let radius_sq = radius * radius;
        let (rows, cols, step_x, step_z) = self.grid_metrics();
        let mut new_heights = self.heights.clone();
        let mut affected = false;

        let strength = strength.clamp(0.0, 1.0);

        for row in 0..rows {
            let world_z = self.bounds.min.z + step_z * row as f32;

            for col in 0..cols {
                let world_x = self.bounds.min.x + step_x * col as f32;

                let dx = world_x - position.x;
                let dz = world_z - position.z;
                let distance_sq = dx * dx + dz * dz;

                if distance_sq > radius_sq {
                    continue;
                }

                let distance = distance_sq.sqrt();
                let influence = falloff_weight(distance, radius, 1.0);
                if influence <= 0.0 {
                    continue;
                }

                let mut total = 0.0;
                let mut count = 0.0;
                for dz in -1..=1 {
                    for dx in -1..=1 {
                        let nz = (row as isize + dz).clamp(0, rows as isize - 1) as usize;
                        let nx = (col as isize + dx).clamp(0, cols as isize - 1) as usize;
                        total += self.heights[nz][nx];
                        count += 1.0;
                    }
                }

                if count > 0.0 {
                    let average = total / count;
                    let weight = (strength * influence).clamp(0.0, 1.0);
                    new_heights[row][col] =
                        self.heights[row][col] + (average - self.heights[row][col]) * weight;
                    affected = true;
                }
            }
        }

        if affected {
            self.heights = new_heights;
            self.recompute_vertical_bounds();
            self.dirty = true;
        }

        Ok(())
    }

    fn grid_metrics(&self) -> (usize, usize, f32, f32) {
        let rows = self.heights.len();
        let cols = self.heights[0].len();

        let extent_x = (self.bounds.max.x - self.bounds.min.x).max(0.0);
        let extent_z = (self.bounds.max.z - self.bounds.min.z).max(0.0);

        let step_x = if cols > 1 {
            extent_x / (cols as f32 - 1.0)
        } else {
            0.0
        };

        let step_z = if rows > 1 {
            extent_z / (rows as f32 - 1.0)
        } else {
            0.0
        };

        (rows, cols, step_x, step_z)
    }

    fn recompute_vertical_bounds(&mut self) {
        let mut min_height = f32::INFINITY;
        let mut max_height = f32::NEG_INFINITY;

        for row in &self.heights {
            for &sample in row {
                min_height = min_height.min(sample);
                max_height = max_height.max(sample);
            }
        }

        if !min_height.is_finite() || !max_height.is_finite() {
            min_height = 0.0;
            max_height = 0.0;
        }

        self.bounds.min.y = min_height;
        self.bounds.max.y = max_height;
    }
}

impl ChunkBounds {
    /// Check if point is inside bounds
    pub fn contains_point(&self, point: &Vec3) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
            && point.z >= self.min.z
            && point.z <= self.max.z
    }

    /// Check if bounds intersect with another bounds
    pub fn intersects(&self, other: &ChunkBounds) -> bool {
        !(self.max.x < other.min.x
            || self.min.x > other.max.x
            || self.max.y < other.min.y
            || self.min.y > other.max.y
            || self.max.z < other.min.z
            || self.min.z > other.max.z)
    }
}

impl ChunkManager {
    /// Create a new chunk manager with default configuration.
    pub fn new() -> Self {
        Self::with_config(TerrainConfig::default())
    }

    /// Create a new chunk manager using the provided configuration.
    pub fn with_config(config: TerrainConfig) -> Self {
        Self {
            chunks: HashMap::new(),
            spatial_grid: HashMap::new(),
            config,
            next_chunk_id: 1,
            camera_position: Vec3::new(0.0, 0.0, 0.0),
            view_frustum: ViewFrustum {
                planes: [Vec3::ZERO; 6],
                view_matrix: Mat4::IDENTITY,
                projection_matrix: Mat4::IDENTITY,
            },
            stats: ChunkManagerStats::default(),
        }
    }

    /// Update the terrain configuration used by the manager.
    pub fn set_config(&mut self, config: TerrainConfig) {
        self.config = config;
    }

    /// Access the current configuration.
    pub fn config(&self) -> &TerrainConfig {
        &self.config
    }

    /// Initialise the manager. Creates a default chunk grid if none exists.
    pub fn init(&mut self) -> TerrainResult<()> {
        self.stats = ChunkManagerStats::default();
        self.camera_position = Vec3::new(0.0, 0.0, 0.0);
        self.view_frustum = ViewFrustum::default();
        Ok(())
    }

    /// Reset the manager, discarding all chunk state.
    pub fn reset(&mut self) -> TerrainResult<()> {
        self.clear();
        Ok(())
    }

    /// Set the camera position used for LOD evaluation.
    pub fn set_camera(&mut self, camera_position: Vec3) {
        self.camera_position = camera_position;
    }

    /// Set the view frustum used for visibility checks.
    pub fn set_view_frustum(&mut self, view_frustum: ViewFrustum) {
        self.view_frustum = view_frustum;
    }

    /// Load the given heightmap into the chunk system, regenerating chunk data.
    pub fn load_heightmap(
        &mut self,
        heightmap: &HeightMap,
        config: &TerrainConfig,
    ) -> TerrainResult<()> {
        self.set_config(config.clone());
        self.clear();

        let chunk_size = self.config.chunk_size.max(1) as f32;
        let world_width = self.config.world_size.0.max(chunk_size);
        let world_depth = self.config.world_size.1.max(chunk_size);

        let min = Vec3::new(0.0, 0.0, 0.0);
        let max = Vec3::new(world_width, 0.0, world_depth);
        self.create_chunks_for_region(min, max, chunk_size)?;

        for chunk in self.chunks.values_mut() {
            Self::populate_chunk_from_heightmap(chunk, heightmap, &self.config);
        }

        self.stats.total_chunks = self.chunks.len() as u32;
        Ok(())
    }

    /// Mark all chunks overlapping the region as dirty so their geometry will be regenerated.
    pub fn mark_region_dirty(&mut self, min_x: f32, min_z: f32, max_x: f32, max_z: f32) {
        for chunk in self.chunks.values_mut() {
            let chunk_min_x = chunk.bounds.min.x;
            let chunk_max_x = chunk.bounds.max.x;
            let chunk_min_z = chunk.bounds.min.z;
            let chunk_max_z = chunk.bounds.max.z;

            let intersects_x = chunk_min_x <= max_x && chunk_max_x >= min_x;
            let intersects_z = chunk_min_z <= max_z && chunk_max_z >= min_z;

            if intersects_x && intersects_z {
                chunk.dirty = true;
            }
        }
    }

    /// Resample all chunks flagged as dirty from the authoritative heightmap data.
    pub fn refresh_dirty_chunks(&mut self, heightmap: &HeightMap) {
        for chunk in self.chunks.values_mut().filter(|chunk| chunk.dirty) {
            Self::populate_chunk_from_heightmap(chunk, heightmap, &self.config);
        }
    }

    /// Validate renderable chunk state for the current frame.
    ///
    /// GPU submission is handled by `TerrainVisualImpl`, but this method mirrors the C++ split
    /// between visibility/update traversal and the draw step.
    pub fn render(&self, _view_matrix: &Mat4, _projection_matrix: &Mat4) -> TerrainResult<()> {
        for chunk in self.chunks.values().filter(|chunk| chunk.visible) {
            if chunk.vertices.is_empty() || chunk.indices.is_empty() {
                // Dirty chunks are expected to be regenerated before they become renderable.
                if !chunk.dirty {
                    log::debug!(
                        "Chunk {} is visible but missing geometry (lod={}, verts={}, indices={})",
                        chunk.id,
                        chunk.lod_level,
                        chunk.vertices.len(),
                        chunk.indices.len()
                    );
                }
            }
        }
        Ok(())
    }

    /// Validate visible chunk state while a render pass is active.
    pub fn render_pass<'a>(&'a self, render_pass: &mut RenderPass<'a>) -> TerrainResult<()> {
        let _ = render_pass;
        self.render(
            &self.view_frustum.view_matrix,
            &self.view_frustum.projection_matrix,
        )
    }

    /// Validate visible chunk state while a depth-only render pass is active.
    pub fn render_depth<'a>(&'a self, render_pass: &mut RenderPass<'a>) -> TerrainResult<()> {
        self.render_pass(render_pass)
    }

    /// Submit GPU draw calls for all visible chunks.
    ///
    /// Caller must set the terrain pipeline and camera bind group (group 0) first.
    /// `bind_group_fn` returns per-chunk texture bind group (group 1). `mesh_fn`
    /// returns (vertex_slice, index_slice, index_count).
    pub fn render_pass_draw<'a, FBindGroup, FMesh>(
        &'a self,
        render_pass: &mut RenderPass<'a>,
        mut bind_group_fn: FBindGroup,
        mut mesh_fn: FMesh,
    ) -> TerrainResult<()>
    where
        FBindGroup: FnMut(ChunkId) -> Option<wgpu::BindGroup>,
        FMesh: FnMut(ChunkId) -> Option<(wgpu::BufferSlice<'a>, wgpu::BufferSlice<'a>, u32)>,
    {
        self.render(
            &self.view_frustum.view_matrix,
            &self.view_frustum.projection_matrix,
        )?;

        for chunk in self.chunks.values().filter(|chunk| {
            chunk.visible && !chunk.vertices.is_empty() && !chunk.indices.is_empty()
        }) {
            if let Some(bg) = bind_group_fn(chunk.id) {
                render_pass.set_bind_group(1, &bg, &[]);
            }

            let Some((vertex_slice, index_slice, index_count)) = mesh_fn(chunk.id) else {
                continue;
            };

            render_pass.set_vertex_buffer(0, vertex_slice);
            render_pass.set_index_buffer(index_slice, wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..index_count, 0, 0..1);
        }

        Ok(())
    }

    /// Number of visible chunks with generated geometry.
    pub fn renderable_chunk_count(&self) -> usize {
        self.chunks
            .values()
            .filter(|chunk| {
                chunk.visible && !chunk.vertices.is_empty() && !chunk.indices.is_empty()
            })
            .count()
    }

    /// Number of visible chunks still waiting for geometry.
    pub fn pending_visible_chunk_count(&self) -> usize {
        self.chunks
            .values()
            .filter(|chunk| {
                chunk.visible && (chunk.vertices.is_empty() || chunk.indices.is_empty())
            })
            .count()
    }

    /// Total triangle count for visible chunks.
    pub fn visible_triangle_count(&self) -> usize {
        self.chunks
            .values()
            .filter(|chunk| chunk.visible)
            .map(|chunk| chunk.indices.len() / 3)
            .sum()
    }

    /// Total vertex count for visible chunks.
    pub fn visible_vertex_count(&self) -> usize {
        self.chunks
            .values()
            .filter(|chunk| chunk.visible)
            .map(|chunk| chunk.vertices.len())
            .sum()
    }

    /// Returns true when every visible chunk has geometry ready for drawing.
    pub fn all_visible_chunks_renderable(&self) -> bool {
        self.pending_visible_chunk_count() == 0
    }

    /// Convenience used by renderer diagnostics.
    pub fn visible_chunk_ids(&self) -> Vec<ChunkId> {
        self.chunks
            .values()
            .filter(|chunk| chunk.visible)
            .map(|chunk| chunk.id)
            .collect()
    }

    /// Convenience used by renderer diagnostics.
    pub fn pending_visible_chunk_ids(&self) -> Vec<ChunkId> {
        self.chunks
            .values()
            .filter(|chunk| {
                chunk.visible && (chunk.vertices.is_empty() || chunk.indices.is_empty())
            })
            .map(|chunk| chunk.id)
            .collect()
    }

    /// Convenience used by renderer diagnostics.
    pub fn chunk_geometry_revision(&self, chunk_id: ChunkId) -> Option<u64> {
        self.chunks
            .get(&chunk_id)
            .map(|chunk| chunk.geometry_revision)
    }

    /// Sum of all chunk geometry revisions, used by renderer diagnostics/tests.
    pub fn total_geometry_revision(&self) -> u64 {
        self.chunks
            .values()
            .map(|chunk| chunk.geometry_revision)
            .sum()
    }

    /// Number of chunks waiting for terrain geometry regeneration.
    pub fn dirty_chunk_count(&self) -> usize {
        self.chunks.values().filter(|chunk| chunk.dirty).count()
    }

    /// Clear chunk dirty flags for diagnostics that need to observe a later invalidation.
    pub fn clear_dirty_flags_for_diagnostics(&mut self) {
        for chunk in self.chunks.values_mut() {
            chunk.dirty = false;
        }
    }

    /// Total number of chunks currently tracked.
    pub fn total_chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Expose immutable iterator support for downstream render code.
    pub fn iter_chunks(&self) -> impl Iterator<Item = &TerrainChunk> {
        self.chunks.values()
    }

    /// Expose immutable iterator over visible chunks for renderer code.
    pub fn iter_visible_chunks(&self) -> impl Iterator<Item = &TerrainChunk> {
        self.chunks.values().filter(|chunk| chunk.visible)
    }

    /// Expose immutable iterator over renderable visible chunks for renderer code.
    pub fn iter_renderable_visible_chunks(&self) -> impl Iterator<Item = &TerrainChunk> {
        self.chunks.values().filter(|chunk| {
            chunk.visible && !chunk.vertices.is_empty() && !chunk.indices.is_empty()
        })
    }

    /// Expose immutable iterator over pending visible chunks for renderer code.
    pub fn iter_pending_visible_chunks(&self) -> impl Iterator<Item = &TerrainChunk> {
        self.chunks.values().filter(|chunk| {
            chunk.visible && (chunk.vertices.is_empty() || chunk.indices.is_empty())
        })
    }

    /// Report whether any chunks are currently visible.
    pub fn has_visible_chunks(&self) -> bool {
        self.chunks.values().any(|chunk| chunk.visible)
    }

    /// Report whether any visible chunks are awaiting geometry.
    pub fn has_pending_visible_chunks(&self) -> bool {
        self.chunks
            .values()
            .any(|chunk| chunk.visible && (chunk.vertices.is_empty() || chunk.indices.is_empty()))
    }

    /// Report whether any visible chunk is renderable.
    pub fn has_renderable_visible_chunks(&self) -> bool {
        self.chunks
            .values()
            .any(|chunk| chunk.visible && !chunk.vertices.is_empty() && !chunk.indices.is_empty())
    }

    /// Count visible chunks by LOD level.
    pub fn visible_chunk_count_by_lod(&self) -> [usize; (MAX_LOD_LEVEL as usize) + 1] {
        let mut counts = [0usize; (MAX_LOD_LEVEL as usize) + 1];
        for chunk in self.chunks.values().filter(|chunk| chunk.visible) {
            let lod = chunk.lod_level.min(MAX_LOD_LEVEL) as usize;
            counts[lod] += 1;
        }
        counts
    }

    /// Count renderable visible chunks by LOD level.
    pub fn renderable_chunk_count_by_lod(&self) -> [usize; (MAX_LOD_LEVEL as usize) + 1] {
        let mut counts = [0usize; (MAX_LOD_LEVEL as usize) + 1];
        for chunk in self.chunks.values().filter(|chunk| {
            chunk.visible && !chunk.vertices.is_empty() && !chunk.indices.is_empty()
        }) {
            let lod = chunk.lod_level.min(MAX_LOD_LEVEL) as usize;
            counts[lod] += 1;
        }
        counts
    }

    /// Count pending visible chunks by LOD level.
    pub fn pending_chunk_count_by_lod(&self) -> [usize; (MAX_LOD_LEVEL as usize) + 1] {
        let mut counts = [0usize; (MAX_LOD_LEVEL as usize) + 1];
        for chunk in self.chunks.values().filter(|chunk| {
            chunk.visible && (chunk.vertices.is_empty() || chunk.indices.is_empty())
        }) {
            let lod = chunk.lod_level.min(MAX_LOD_LEVEL) as usize;
            counts[lod] += 1;
        }
        counts
    }

    /// Aggregate visible geometry memory estimate in bytes.
    pub fn visible_geometry_memory_bytes(&self) -> usize {
        self.chunks
            .values()
            .filter(|chunk| chunk.visible)
            .map(|chunk| {
                chunk.vertices.len() * std::mem::size_of::<TerrainVertex>()
                    + chunk.indices.len() * std::mem::size_of::<u32>()
            })
            .sum()
    }

    /// Aggregate renderable geometry memory estimate in bytes.
    pub fn renderable_geometry_memory_bytes(&self) -> usize {
        self.chunks
            .values()
            .filter(|chunk| {
                chunk.visible && !chunk.vertices.is_empty() && !chunk.indices.is_empty()
            })
            .map(|chunk| {
                chunk.vertices.len() * std::mem::size_of::<TerrainVertex>()
                    + chunk.indices.len() * std::mem::size_of::<u32>()
            })
            .sum()
    }

    /// Aggregate pending geometry memory estimate in bytes.
    pub fn pending_geometry_memory_bytes(&self) -> usize {
        self.chunks
            .values()
            .filter(|chunk| {
                chunk.visible && (chunk.vertices.is_empty() || chunk.indices.is_empty())
            })
            .map(|chunk| {
                chunk.vertices.len() * std::mem::size_of::<TerrainVertex>()
                    + chunk.indices.len() * std::mem::size_of::<u32>()
            })
            .sum()
    }

    /// Returns true when the visible chunk set is empty.
    pub fn visible_set_is_empty(&self) -> bool {
        !self.has_visible_chunks()
    }

    /// Returns true when the renderable visible chunk set is empty.
    pub fn renderable_visible_set_is_empty(&self) -> bool {
        !self.has_renderable_visible_chunks()
    }

    /// Returns true when the pending visible chunk set is empty.
    pub fn pending_visible_set_is_empty(&self) -> bool {
        !self.has_pending_visible_chunks()
    }

    /// Diagnostic summary string for renderer logging.
    pub fn render_diagnostic_summary(&self) -> String {
        let mut lod_counts = [0usize; (MAX_LOD_LEVEL as usize) + 1];
        for chunk in self.chunks.values().filter(|chunk| chunk.visible) {
            let lod = chunk.lod_level.min(MAX_LOD_LEVEL) as usize;
            lod_counts[lod] += 1;
        }

        format!(
            "chunks total={} visible={} renderable={} pending={} tris={} verts={} lods={:?}",
            self.total_chunk_count(),
            self.get_visible_chunks().len(),
            self.renderable_chunk_count(),
            self.pending_visible_chunk_count(),
            self.visible_triangle_count(),
            self.visible_vertex_count(),
            lod_counts,
        )
    }

    /// Generate a concise chunk-id list for diagnostics.
    pub fn renderable_visible_chunk_ids(&self) -> Vec<ChunkId> {
        self.chunks
            .values()
            .filter(|chunk| {
                chunk.visible && !chunk.vertices.is_empty() && !chunk.indices.is_empty()
            })
            .map(|chunk| chunk.id)
            .collect()
    }

    /// Returns true when chunk geometry revisions are monotonically non-zero for visible chunks.
    pub fn visible_chunk_revisions_valid(&self) -> bool {
        self.chunks
            .values()
            .filter(|chunk| {
                chunk.visible && !chunk.vertices.is_empty() && !chunk.indices.is_empty()
            })
            .all(|chunk| chunk.geometry_revision > 0)
    }

    /// Render-time sanity check used by diagnostics.
    pub fn validate_visible_chunk_geometry(&self) -> TerrainResult<()> {
        for chunk in self.chunks.values().filter(|chunk| chunk.visible) {
            if chunk.vertices.is_empty() || chunk.indices.is_empty() {
                continue;
            }
            if chunk.indices.len() % 3 != 0 {
                return Err(TerrainError::InvalidData(format!(
                    "Chunk {} has non-triangle index count {}",
                    chunk.id,
                    chunk.indices.len()
                )));
            }
        }
        Ok(())
    }

    fn sample_heightmap_world(
        heightmap: &HeightMap,
        world_x: f32,
        world_z: f32,
        world_width: f32,
        world_depth: f32,
    ) -> f32 {
        let clamped_x = world_x.clamp(0.0, world_width.max(0.0));
        let clamped_z = world_z.clamp(0.0, world_depth.max(0.0));
        heightmap.get_height_at(clamped_x, clamped_z)
    }

    fn populate_chunk_from_heightmap(
        chunk: &mut TerrainChunk,
        heightmap: &HeightMap,
        config: &TerrainConfig,
    ) {
        let half_size = chunk.size * 0.5;
        let map_width = config.world_size.0.max(chunk.size);
        let map_depth = config.world_size.1.max(chunk.size);

        let min_world_x = (chunk.position.x - half_size).clamp(0.0, map_width);
        let max_world_x = (chunk.position.x + half_size).clamp(0.0, map_width);
        let min_world_z = (chunk.position.z - half_size).clamp(0.0, map_depth);
        let max_world_z = (chunk.position.z + half_size).clamp(0.0, map_depth);

        let step_x = (map_width / heightmap.width.saturating_sub(1).max(1) as f32).max(1.0);
        let step_z = (map_depth / heightmap.height.saturating_sub(1).max(1) as f32).max(1.0);
        let span_x = (max_world_x - min_world_x).max(step_x);
        let span_z = (max_world_z - min_world_z).max(step_z);

        let samples_x = ((span_x / step_x).ceil() as usize).max(1) + 1;
        let samples_z = ((span_z / step_z).ceil() as usize).max(1) + 1;

        chunk.heights = vec![vec![0.0; samples_x]; samples_z];

        let mut min_height = f32::INFINITY;
        let mut max_height = f32::NEG_INFINITY;

        for (row_index, row) in chunk.heights.iter_mut().enumerate() {
            let t_z = if samples_z <= 1 {
                0.0
            } else {
                row_index as f32 / (samples_z as f32 - 1.0)
            };
            let world_z = min_world_z + t_z * (max_world_z - min_world_z);

            for (col_index, sample) in row.iter_mut().enumerate() {
                let t_x = if samples_x <= 1 {
                    0.0
                } else {
                    col_index as f32 / (samples_x as f32 - 1.0)
                };
                let world_x = min_world_x + t_x * (max_world_x - min_world_x);

                let height =
                    Self::sample_heightmap_world(heightmap, world_x, world_z, map_width, map_depth);
                *sample = height;
                min_height = min_height.min(height);
                max_height = max_height.max(height);
            }
        }

        if !min_height.is_finite() || !max_height.is_finite() {
            min_height = 0.0;
            max_height = 0.0;
        }

        chunk.bounds.min.x = min_world_x;
        chunk.bounds.max.x = max_world_x;
        chunk.bounds.min.z = min_world_z;
        chunk.bounds.max.z = max_world_z;
        chunk.bounds.min.y = min_height;
        chunk.bounds.max.y = max_height;
        chunk.dirty = true;
    }

    /// Create chunks for terrain region
    pub fn create_chunks_for_region(
        &mut self,
        min_pos: Vec3,
        max_pos: Vec3,
        chunk_size: f32,
    ) -> TerrainResult<()> {
        let extent_x = (max_pos.x - min_pos.x).max(chunk_size);
        let extent_z = (max_pos.z - min_pos.z).max(chunk_size);
        let chunks_x = (extent_x / chunk_size).ceil() as i32;
        let chunks_z = (extent_z / chunk_size).ceil() as i32;

        for z in 0..chunks_z {
            for x in 0..chunks_x {
                let chunk_pos = Vec3::new(
                    min_pos.x + (x as f32 + 0.5) * chunk_size,
                    0.0,
                    min_pos.z + (z as f32 + 0.5) * chunk_size,
                );

                let chunk = TerrainChunk::new(self.next_chunk_id, chunk_pos, chunk_size);

                self.spatial_grid.insert((x, z), self.next_chunk_id);
                self.chunks.insert(self.next_chunk_id, chunk);
                self.next_chunk_id += 1;
            }
        }

        self.stats.total_chunks = self.chunks.len() as u32;
        Ok(())
    }

    /// Get chunk by world position
    pub fn get_chunk_at_position(&self, position: &Vec3) -> Option<&TerrainChunk> {
        // Convert world position to grid coordinates
        let chunk_size = self.config.chunk_size.max(1) as f32;
        let grid_x = (position.x / chunk_size).floor() as i32;
        let grid_z = (position.z / chunk_size).floor() as i32;

        if let Some(&chunk_id) = self.spatial_grid.get(&(grid_x, grid_z)) {
            self.chunks.get(&chunk_id)
        } else {
            None
        }
    }

    /// Get mutable chunk by world position
    pub fn get_chunk_at_position_mut(&mut self, position: &Vec3) -> Option<&mut TerrainChunk> {
        let chunk_size = self.config.chunk_size.max(1) as f32;
        let grid_x = (position.x / chunk_size).floor() as i32;
        let grid_z = (position.z / chunk_size).floor() as i32;

        if let Some(&chunk_id) = self.spatial_grid.get(&(grid_x, grid_z)) {
            self.chunks.get_mut(&chunk_id)
        } else {
            None
        }
    }

    /// Update all chunks for the current frame using the stored camera state.
    pub fn update(&mut self) -> TerrainResult<()> {
        let start_time = Instant::now();

        let camera_position = self.camera_position;
        let view_frustum = self.view_frustum.clone();

        self.stats.visible_chunks = 0;
        self.stats.rendered_chunks = 0;

        // Update LOD and visibility for all chunks
        for chunk in self.chunks.values_mut() {
            // Update LOD based on distance
            let old_lod = chunk.lod_level;
            chunk.update_lod(&camera_position, &self.config);

            if old_lod != chunk.lod_level {
                self.stats.lod_transitions += 1;
            }

            // Check visibility
            chunk.visible = chunk.is_visible(&view_frustum);
            if chunk.visible {
                self.stats.visible_chunks += 1;
            }

            // Frustum can miss the look-at window. Still build dirty chunks
            // under the camera so Alpine is not a 3x3 cobble wall.
            let near_camera = {
                let cx = (chunk.bounds.min.x + chunk.bounds.max.x) * 0.5;
                let cz = (chunk.bounds.min.z + chunk.bounds.max.z) * 0.5;
                let dx = cx - camera_position.x;
                let dz = cz - camera_position.z;
                dx * dx + dz * dz <= 512.0 * 512.0
            };
            if chunk.dirty && (chunk.visible || near_camera) {
                let resolution = match chunk.lod_level {
                    0 => 65,
                    1 => 33,
                    2 => 17,
                    3 => 9,
                    _ => 5,
                };

                if let Err(e) = chunk.generate_geometry(resolution) {
                    log::warn!("Failed to generate geometry for chunk {}: {}", chunk.id, e);
                } else {
                    self.stats.geometry_updates += 1;
                }
            } else if (chunk.visible || near_camera) && !chunk.vertices.is_empty() {
                chunk.apply_dynamic_lights();
            }
        }

        self.stats.rendered_chunks = self.stats.visible_chunks;
        self.stats.update_time = start_time.elapsed();
        Ok(())
    }

    /// Get all visible chunks for rendering
    pub fn get_visible_chunks(&self) -> Vec<&TerrainChunk> {
        self.chunks.values().filter(|chunk| chunk.visible).collect()
    }

    /// Returns true if a chunk with the specified identifier exists.
    pub fn has_chunk(&self, id: ChunkId) -> bool {
        self.chunks.contains_key(&id)
    }

    /// Retrieve a chunk by identifier.
    pub fn get_chunk(&self, id: ChunkId) -> Option<&TerrainChunk> {
        self.chunks.get(&id)
    }

    /// Apply terrain modification to affected chunks
    pub fn apply_modification(&mut self, modification: &TerrainModification) -> TerrainResult<()> {
        let position = match modification {
            TerrainModification::Raise { position, .. }
            | TerrainModification::Lower { position, .. }
            | TerrainModification::Flatten { position, .. }
            | TerrainModification::CreateCrater { position, .. }
            | TerrainModification::Smooth { position, .. } => *position,
        };

        let radius = match modification {
            TerrainModification::Raise { radius, .. }
            | TerrainModification::Lower { radius, .. }
            | TerrainModification::Flatten { radius, .. }
            | TerrainModification::CreateCrater { radius, .. }
            | TerrainModification::Smooth { radius, .. } => *radius,
        };

        // Find all chunks that might be affected
        let affected_chunks: Vec<ChunkId> = self
            .chunks
            .values()
            .filter(|chunk| {
                let distance = chunk.position.distance(position);
                distance <= radius + chunk.size / 2.0
            })
            .map(|chunk| chunk.id)
            .collect();

        // Apply modification to each affected chunk
        for chunk_id in affected_chunks {
            if let Some(chunk) = self.chunks.get_mut(&chunk_id) {
                chunk.apply_modification(modification)?;
            }
        }

        Ok(())
    }

    /// Get performance statistics
    pub fn get_stats(&self) -> &ChunkManagerStats {
        &self.stats
    }

    /// Clear all chunks
    pub fn clear(&mut self) {
        self.chunks.clear();
        self.spatial_grid.clear();
        self.next_chunk_id = 1;
        self.stats = ChunkManagerStats::default();
    }
}

impl Default for ViewFrustum {
    fn default() -> Self {
        Self {
            planes: [Vec3::ZERO; 6],
            view_matrix: Mat4::IDENTITY,
            projection_matrix: Mat4::IDENTITY,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_creation() {
        let chunk = TerrainChunk::new(1, Vec3::new(0.0, 0.0, 0.0), 64.0);
        assert_eq!(chunk.id, 1);
        assert_eq!(chunk.size, 64.0);
        assert!(chunk.dirty);
        assert!(!chunk.visible);
    }

    #[test]
    fn test_chunk_bounds() {
        let bounds = ChunkBounds {
            min: Vec3::new(-10.0, -5.0, -10.0),
            max: Vec3::new(10.0, 5.0, 10.0),
        };

        assert!(bounds.contains_point(&Vec3::new(0.0, 0.0, 0.0)));
        assert!(!bounds.contains_point(&Vec3::new(15.0, 0.0, 0.0)));
    }

    #[test]
    fn test_chunk_manager_creation() {
        let config = TerrainConfig::default();
        let manager = ChunkManager::with_config(config);
        assert_eq!(manager.chunks.len(), 0);
        assert_eq!(manager.next_chunk_id, 1);
    }

    #[test]
    fn test_chunk_region_creation() {
        let config = TerrainConfig::default();
        let mut manager = ChunkManager::with_config(config);

        let result = manager.create_chunks_for_region(
            Vec3::new(-64.0, 0.0, -64.0),
            Vec3::new(64.0, 0.0, 64.0),
            64.0,
        );

        assert!(result.is_ok());
        assert_eq!(manager.chunks.len(), 4); // 2x2 grid
    }

    #[test]
    fn test_lod_calculation() {
        let mut chunk = TerrainChunk::new(1, Vec3::new(0.0, 0.0, 0.0), 64.0);
        let config = TerrainConfig::default();

        // Close camera - should be LOD 0
        chunk.update_lod(&Vec3::new(25.0, 0.0, 0.0), &config);
        assert_eq!(chunk.lod_level, 0);

        chunk.update_lod(&Vec3::new(250.0, 0.0, 0.0), &config);
        assert_eq!(chunk.lod_level, 1);

        chunk.update_lod(&Vec3::new(500.0, 0.0, 0.0), &config);
        assert_eq!(chunk.lod_level, 2);

        // Very distant camera - should be coarsest LOD
        chunk.update_lod(&Vec3::new(1500.0, 0.0, 0.0), &config);
        assert_eq!(chunk.lod_level, 4);
    }

    #[test]
    fn generate_geometry_uses_p0_p2_split_like_cpp_heightmap() {
        crate::fx_list::clear_scene_dynamic_lights();
        let mut chunk = TerrainChunk::new(1, Vec3::new(32.0, 0.0, 32.0), 64.0);
        chunk.heights = vec![vec![0.0, 10.0], vec![20.0, 30.0]];

        chunk.generate_geometry(4).unwrap();

        let center = &chunk.vertices[5];
        let center_height = center.position().y;
        assert!(
            center_height > 10.0 && center_height < 20.0,
            "center vertex should interpolate chunk heights, got {center_height}"
        );
        assert_eq!(chunk.indices.len(), 3 * 3 * 2 * 3);
        assert_eq!(chunk.indices[0], 0);
        assert_eq!(chunk.indices[1], 1);
        assert_eq!(chunk.indices[2], 5);
        assert_eq!(chunk.indices[3], 0);
        assert_eq!(chunk.indices[4], 5);
        assert_eq!(chunk.indices[5], 4);
        let upper = chunk.sample_height_bilinear(0.25, 0.75);
        assert!(
            (upper - 17.5).abs() < 0.01,
            "upper triangle p0-p2 plane, got {upper}"
        );

        let last_col_top = chunk.vertices[3].position().y;
        assert!(
            (last_col_top - 10.0).abs() < 0.001,
            "top-right corner should preserve corner height, got {last_col_top}"
        );
    }

    #[test]
    fn generate_geometry_bakes_do_the_dynamic_light_into_vertex_color() {
        use crate::fx_list::{
            DisplayLightPulse, clear_scene_dynamic_lights, create_display_light_pulse,
            do_the_dynamic_light, drain_display_light_pulses, scene_dynamic_lights,
        };

        let _ = drain_display_light_pulses();
        clear_scene_dynamic_lights();
        assert!(create_display_light_pulse(DisplayLightPulse {
            pos: [0.0, 0.0, 0.0],
            color: [1.0, 0.0, 0.0],
            inner_radius: 10.0,
            outer_radius: 80.0,
            increase_frames: 0,
            decay_frames: 0,
        }));

        let mut chunk = TerrainChunk::new(1, Vec3::new(0.0, 0.0, 0.0), 32.0);
        chunk.heights = vec![vec![0.0, 0.0], vec![0.0, 0.0]];
        chunk.generate_geometry(3).unwrap();
        assert!(!chunk.vertices.is_empty());
        // White global fill clamps added light; C++ vbMirror is sun-lit gray.
        for base in &mut chunk.base_colors {
            *base = [0.2, 0.2, 0.2, 1.0];
        }
        chunk.apply_dynamic_lights();

        let lights = scene_dynamic_lights();
        let v = &chunk.vertices[0];
        let expected = do_the_dynamic_light(
            [v.position[0], v.position[2], v.position[1]],
            [v.normal[0], v.normal[2], v.normal[1]],
            super::rgba_f32_to_bgra_u32([0.2, 0.2, 0.2, 1.0]),
            &lights,
        );
        let expected_color = super::bgra_u32_to_rgba_f32(expected);
        for i in 0..4 {
            assert!(
                (v.color[i] - expected_color[i]).abs() < 1e-5,
                "vertex color[{i}]={} expected={}",
                v.color[i],
                expected_color[i]
            );
        }
        assert!(
            v.color[0] > v.color[1] + 0.05,
            "red pulse must raise R vs G from gray base, got {:?}",
            v.color
        );

        // Shipped wgpu FS must multiply sample by this baked color (no fake N·L).
        let shaded = TerrainChunk::modulate_terrain_sample_by_vb_color([1.0, 1.0, 1.0], v.color);
        assert!((shaded[0] - v.color[0]).abs() < 1e-5);
        assert!((shaded[1] - v.color[1]).abs() < 1e-5);
        assert!(shaded[0] > shaded[1] + 0.05);

        let shader = include_str!("../shaders/terrain.wgsl");
        assert!(
            shader.contains("out.color = vertex.color"),
            "terrain VS must pass baked doTheDynamicLight color"
        );
        assert!(
            shader.contains("final_color.rgb * in.color.rgb"),
            "terrain FS must modulate by vb.diffuse"
        );
        assert!(
            !shader.contains("light_dir = normalize(vec3<f32>(0.3, 0.7, 0.2))"),
            "terrain FS must not fake-N·L after HeightMap bake"
        );
        assert!(
            shader.contains("mode & 1") && shader.contains("mode & 2"),
            "terrain FS must gate cloud/noise on UseCloudMap/UseLightMap flags"
        );
        assert!(
            !shader.contains("final_color.rgb * cloud * noise"),
            "terrain FS must not multiply cloud+noise unconditionally"
        );
        clear_scene_dynamic_lights();
    }

    #[test]
    fn load_heightmap_preserves_exact_edge_samples_for_render_chunks() {
        let mut heightmap = HeightMap::new(4, 4, 100.0, 1.0);
        heightmap.set_height_at_index(3, 0, 0.25);
        heightmap.set_height_at_index(0, 3, 0.5);
        heightmap.set_height_at_index(3, 3, 0.75);

        let config = TerrainConfig {
            world_size: (3.0, 3.0),
            chunk_size: 3,
            ..Default::default()
        };
        let mut manager = ChunkManager::with_config(config.clone());
        manager.load_heightmap(&heightmap, &config).unwrap();

        let chunk = manager.get_chunk(1).unwrap();
        let top_right = *chunk.heights.first().and_then(|row| row.last()).unwrap();
        let bottom_left = chunk
            .heights
            .last()
            .and_then(|row| row.first())
            .copied()
            .unwrap();
        let bottom_right = chunk
            .heights
            .last()
            .and_then(|row| row.last())
            .copied()
            .unwrap();

        assert!((top_right - 25.0).abs() < 0.001);
        assert!((bottom_left - 50.0).abs() < 0.001);
        assert!((bottom_right - 75.0).abs() < 0.001);
    }
}
