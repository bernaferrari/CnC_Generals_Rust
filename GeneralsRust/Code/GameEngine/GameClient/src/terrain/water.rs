//! Terrain Water System
//!
//! Manages water bodies including rivers, lakes, oceans, and special water
//! effects like waterfalls and rapids. Handles water simulation, rendering,
//! and interaction with terrain and gameplay elements.

use crate::terrain::{TerrainError, TerrainResult};
use glam::{Mat4, Vec3};
use nalgebra::{Point2, Vector2};
use std::collections::HashMap;
use wgpu::RenderPass;

/// Unique identifier for water bodies
pub type WaterBodyId = u32;

/// Unique identifier for water segments
pub type WaterSegmentId = u32;

/// Types of water bodies with different properties
#[derive(Debug, Clone, PartialEq)]
pub enum WaterType {
    /// Still water bodies
    Lake {
        depth: f32,
        clarity: f32,
        temperature: f32,
    },

    /// Flowing rivers and streams
    River {
        flow_speed: f32,
        flow_direction: Vec3,
        depth: f32,
        turbulence: f32,
    },

    /// Ocean or sea water
    Ocean {
        wave_height: f32,
        wave_frequency: f32,
        tide_level: f32,
        current_direction: Vector2<f32>,
    },

    /// Marshland and swamps
    Marsh {
        vegetation_density: f32,
        mud_factor: f32,
        depth: f32,
    },

    /// Waterfalls and cascades
    Waterfall {
        drop_height: f32,
        flow_rate: f32,
        mist_generation: f32,
    },

    /// Artificial water features
    Fountain {
        spray_height: f32,
        spray_pattern: FountainPattern,
        flow_rate: f32,
    },
}

/// Fountain spray patterns
#[derive(Debug, Clone, PartialEq)]
pub enum FountainPattern {
    SingleSpray,
    MultipleJets,
    Cascade,
    Misting,
}

/// Water body containing connected water segments
#[derive(Debug, Clone)]
pub struct WaterBody {
    /// Unique identifier
    pub id: WaterBodyId,

    /// Display name
    pub name: String,

    /// Water type and properties
    pub water_type: WaterType,

    /// Connected water segments
    pub segments: Vec<WaterSegmentId>,

    /// Base water level (Y coordinate)
    pub water_level: f32,

    /// Water transparency (0.0 = opaque, 1.0 = transparent)
    pub transparency: f32,

    /// Water color tint
    pub color: [f32; 4],

    /// Whether water affects unit movement
    pub affects_movement: bool,

    /// Movement penalty for units in water
    pub movement_penalty: f32,

    /// Whether units can cross this water
    pub crossable: bool,

    /// Water surface animation properties
    pub animation: WaterAnimation,

    /// Whether water body is visible
    pub visible: bool,

    /// Environmental effects
    pub effects: WaterEffects,
}

/// Water animation properties
#[derive(Debug, Clone)]
pub struct WaterAnimation {
    /// Wave amplitude for surface animation
    pub wave_amplitude: f32,

    /// Wave frequency in Hz
    pub wave_frequency: f32,

    /// Animation speed multiplier
    pub speed_multiplier: f32,

    /// Texture scrolling speed for flowing water
    pub texture_scroll_speed: Vector2<f32>,

    /// Time offset for animation variation
    pub time_offset: f32,

    /// Whether animation is enabled
    pub enabled: bool,
}

/// Environmental effects for water bodies
#[derive(Debug, Clone)]
pub struct WaterEffects {
    /// Generate mist/fog effects
    pub generates_mist: bool,

    /// Mist intensity
    pub mist_intensity: f32,

    /// Splash effects when objects enter water
    pub splash_effects: bool,

    /// Ambient sound effects
    pub ambient_sounds: Vec<WaterSound>,

    /// Reflection quality (0 = none, 1 = full reflections)
    pub reflection_quality: f32,

    /// Refraction distortion strength
    pub refraction_strength: f32,
}

/// Water sound effects
#[derive(Debug, Clone, PartialEq)]
pub enum WaterSound {
    Lapping,  // Gentle water sounds
    Babbling, // Brook/stream sounds
    Rushing,  // Fast flowing water
    Crashing, // Waterfall or wave sounds
    Dripping, // Cave water drips
}

/// Individual water segment with geometry
#[derive(Debug, Clone)]
pub struct WaterSegment {
    /// Unique identifier
    pub id: WaterSegmentId,

    /// Parent water body ID
    pub water_body_id: WaterBodyId,

    /// Segment shape definition
    pub shape: WaterShape,

    /// Local water properties (can override body properties)
    pub local_properties: Option<WaterSegmentProperties>,

    /// Generated rendering geometry
    pub geometry: Option<WaterGeometry>,

    /// Flow simulation data
    pub flow_data: Option<WaterFlowData>,

    /// Whether geometry needs regeneration
    pub dirty: bool,

    /// Bounding box for culling
    pub bounds: WaterBounds,
}

/// Water segment shapes
#[derive(Debug, Clone)]
pub enum WaterShape {
    /// Rectangular area
    Rectangle {
        center: Vec3,
        width: f32,
        height: f32,
        rotation: f32,
    },

    /// Circular area
    Circle { center: Vec3, radius: f32 },

    /// Polygon defined by vertices
    Polygon { vertices: Vec<Vec3> },

    /// River/stream path
    Path {
        points: Vec<Vec3>,
        width: f32,
        width_variation: Vec<f32>, // Width multiplier at each point
    },

    /// Spline-based curved area
    Spline {
        control_points: Vec<Vec3>,
        width: f32,
        resolution: u32,
    },
}

/// Local properties for water segments
#[derive(Debug, Clone)]
pub struct WaterSegmentProperties {
    /// Local water level offset
    pub level_offset: f32,

    /// Local flow direction (for rivers)
    pub flow_direction: Option<Vec3>,

    /// Local depth variation
    pub depth_multiplier: f32,

    /// Local animation speed multiplier
    pub animation_speed: f32,

    /// Local color tint
    pub color_tint: [f32; 4],

    /// Custom texture UV scaling
    pub texture_scale: Vector2<f32>,
}

/// Rendering geometry for water segments
#[derive(Debug, Clone)]
pub struct WaterGeometry {
    /// Water surface vertices
    pub vertices: Vec<WaterVertex>,

    /// Triangle indices
    pub indices: Vec<u32>,

    /// Underwater geometry (for depth effects)
    pub underwater_vertices: Vec<WaterVertex>,

    /// Underwater triangle indices
    pub underwater_indices: Vec<u32>,

    /// Shore/edge geometry for blending with terrain
    pub shore_geometry: Option<ShoreGeometry>,
}

/// Vertex data for water rendering
#[derive(Debug, Clone)]
pub struct WaterVertex {
    /// World position
    pub position: [f32; 3],

    /// Surface normal
    pub normal: [f32; 3],

    /// Texture coordinates
    pub tex_coords: [f32; 2],

    /// Water depth at this point
    pub depth: f32,

    /// Flow velocity for animation
    pub flow_velocity: [f32; 2],

    /// Water color/tint
    pub color: [f32; 4],
}

/// Shore geometry for water-terrain blending
#[derive(Debug, Clone)]
pub struct ShoreGeometry {
    pub vertices: Vec<WaterVertex>,
    pub indices: Vec<u32>,
    pub blend_distance: f32,
}

/// Flow simulation data for water segments
#[derive(Debug, Clone)]
pub struct WaterFlowData {
    /// Flow velocity field
    pub velocity_field: Vec<Vector2<f32>>,

    /// Flow field resolution
    pub field_resolution: (u32, u32),

    /// Flow obstacles (rocks, debris, etc.)
    pub obstacles: Vec<FlowObstacle>,

    /// Pressure field for realistic flow
    pub pressure_field: Vec<f32>,

    /// Time step for simulation
    pub time_step: f32,
}

/// Obstacles that affect water flow
#[derive(Debug, Clone)]
pub struct FlowObstacle {
    pub position: Point2<f32>,
    pub radius: f32,
    pub height: f32,
    pub flow_resistance: f32,
}

/// Bounding box for water segments
#[derive(Debug, Clone)]
pub struct WaterBounds {
    pub min: Vec3,
    pub max: Vec3,
}

/// Manages all water bodies and simulation
#[derive(Debug)]
pub struct WaterManager {
    /// All water bodies
    water_bodies: HashMap<WaterBodyId, WaterBody>,

    /// All water segments
    segments: HashMap<WaterSegmentId, WaterSegment>,

    /// Water simulation parameters
    simulation_config: WaterSimulationConfig,

    /// Rendering configuration
    render_config: WaterRenderConfig,

    /// Next available IDs
    next_body_id: WaterBodyId,
    next_segment_id: WaterSegmentId,

    /// Global simulation time
    simulation_time: f32,

    /// Performance statistics
    stats: WaterStats,

    /// Global enable flag
    enabled: bool,
}

/// Configuration for water simulation
#[derive(Debug, Clone)]
pub struct WaterSimulationConfig {
    /// Enable flow simulation
    pub enable_flow_simulation: bool,

    /// Flow simulation timestep
    pub flow_timestep: f32,

    /// Flow simulation iterations per frame
    pub flow_iterations: u32,

    /// Viscosity for flow simulation
    pub viscosity: f32,

    /// Gravity constant
    pub gravity: f32,

    /// Enable wave simulation
    pub enable_wave_simulation: bool,

    /// Wave propagation speed
    pub wave_speed: f32,

    /// Wave damping factor
    pub wave_damping: f32,
}

/// Configuration for water rendering
#[derive(Debug, Clone)]
pub struct WaterRenderConfig {
    /// Tessellation level for smooth surfaces
    pub tessellation_level: u32,

    /// Enable reflections
    pub enable_reflections: bool,

    /// Reflection texture resolution
    pub reflection_resolution: (u32, u32),

    /// Enable refractions
    pub enable_refractions: bool,

    /// Refraction texture resolution
    pub refraction_resolution: (u32, u32),

    /// Foam generation parameters
    pub foam_threshold: f32,
    pub foam_intensity: f32,

    /// Caustic light patterns
    pub enable_caustics: bool,
    pub caustic_intensity: f32,
}

/// Performance statistics for water system
#[derive(Debug, Default)]
pub struct WaterStats {
    pub total_bodies: u32,
    pub total_segments: u32,
    pub total_vertices: u32,
    pub total_triangles: u32,
    pub simulation_time: std::time::Duration,
    pub render_time: std::time::Duration,
    pub memory_usage: u64,
    pub active_simulations: u32,
}

impl Default for WaterAnimation {
    fn default() -> Self {
        Self {
            wave_amplitude: 0.05,
            wave_frequency: 0.5,
            speed_multiplier: 1.0,
            texture_scroll_speed: Vector2::new(0.1, 0.0),
            time_offset: 0.0,
            enabled: true,
        }
    }
}

impl Default for WaterEffects {
    fn default() -> Self {
        Self {
            generates_mist: false,
            mist_intensity: 0.0,
            splash_effects: true,
            ambient_sounds: vec![WaterSound::Lapping],
            reflection_quality: 0.5,
            refraction_strength: 0.1,
        }
    }
}

impl Default for WaterSegmentProperties {
    fn default() -> Self {
        Self {
            level_offset: 0.0,
            flow_direction: None,
            depth_multiplier: 1.0,
            animation_speed: 1.0,
            color_tint: [1.0, 1.0, 1.0, 1.0],
            texture_scale: Vector2::new(1.0, 1.0),
        }
    }
}

impl Default for WaterSimulationConfig {
    fn default() -> Self {
        Self {
            enable_flow_simulation: true,
            flow_timestep: 1.0 / 60.0,
            flow_iterations: 3,
            viscosity: 0.01,
            gravity: 9.81,
            enable_wave_simulation: true,
            wave_speed: 2.0,
            wave_damping: 0.95,
        }
    }
}

impl Default for WaterRenderConfig {
    fn default() -> Self {
        Self {
            tessellation_level: 4,
            enable_reflections: true,
            reflection_resolution: (512, 512),
            enable_refractions: true,
            refraction_resolution: (512, 512),
            foam_threshold: 0.5,
            foam_intensity: 0.8,
            enable_caustics: false,
            caustic_intensity: 0.3,
        }
    }
}

impl WaterBody {
    /// Create new water body
    pub fn new(id: WaterBodyId, name: String, water_type: WaterType) -> Self {
        let (color, transparency, affects_movement) = match &water_type {
            WaterType::Lake { .. } => ([0.2, 0.5, 0.8, 0.8], 0.8, true),
            WaterType::River { .. } => ([0.3, 0.6, 0.9, 0.7], 0.7, true),
            WaterType::Ocean { .. } => ([0.1, 0.3, 0.7, 0.9], 0.9, true),
            WaterType::Marsh { .. } => ([0.4, 0.5, 0.3, 0.6], 0.6, true),
            WaterType::Waterfall { .. } => ([0.9, 0.9, 1.0, 0.5], 0.3, false),
            WaterType::Fountain { .. } => ([0.8, 0.9, 1.0, 0.6], 0.6, false),
        };

        Self {
            id,
            name,
            water_type,
            segments: Vec::new(),
            water_level: 0.0,
            transparency,
            color,
            affects_movement,
            movement_penalty: 0.5,
            crossable: true,
            animation: WaterAnimation::default(),
            visible: true,
            effects: WaterEffects::default(),
        }
    }

    /// Add segment to water body
    pub fn add_segment(&mut self, segment_id: WaterSegmentId) {
        if !self.segments.contains(&segment_id) {
            self.segments.push(segment_id);
        }
    }

    /// Remove segment from water body
    pub fn remove_segment(&mut self, segment_id: WaterSegmentId) {
        self.segments.retain(|&id| id != segment_id);
    }

    /// Update water animation
    pub fn update_animation(&mut self, delta_time: f32) {
        if self.animation.enabled {
            self.animation.time_offset += delta_time * self.animation.speed_multiplier;
        }
    }

    /// Calculate wave height at position and time
    pub fn calculate_wave_height(&self, position: Point2<f32>, time: f32) -> f32 {
        if !self.animation.enabled {
            return 0.0;
        }

        let wave_time = time + self.animation.time_offset;
        let wave_x = (position.x * 0.1 + wave_time * self.animation.wave_frequency).sin();
        let wave_z = (position.y * 0.1 + wave_time * self.animation.wave_frequency * 0.7).sin();

        (wave_x + wave_z) * self.animation.wave_amplitude
    }
}

impl WaterSegment {
    fn compute_shore_blend_distance(render_config: &WaterRenderConfig) -> f32 {
        (render_config.tessellation_level.max(1) as f32 * 0.2).clamp(0.4, 3.0)
    }

    fn build_shore_geometry_from_ring(
        ring: &[WaterVertex],
        blend_distance: f32,
    ) -> Option<ShoreGeometry> {
        if ring.len() < 3 || blend_distance <= 0.0 {
            return None;
        }

        let mut center = Vector2::zeros();
        for v in ring {
            center.x += v.position[0];
            center.y += v.position[2];
        }
        center /= ring.len() as f32;

        let mut vertices = Vec::with_capacity(ring.len() * 2);
        let mut indices = Vec::with_capacity(ring.len() * 6);

        for vertex in ring {
            let mut inner = vertex.clone();
            let mut outer = vertex.clone();

            let outward =
                Vector2::new(vertex.position[0] - center.x, vertex.position[2] - center.y);
            let outward = if outward.norm_squared() > f32::EPSILON {
                outward.normalize()
            } else {
                Vector2::new(1.0, 0.0)
            };

            outer.position[0] += outward.x * blend_distance;
            outer.position[2] += outward.y * blend_distance;
            outer.color[3] = 0.0;
            inner.color[3] = inner.color[3].max(0.01);

            vertices.push(inner);
            vertices.push(outer);
        }

        let ring_count = ring.len() as u32;
        for i in 0..ring_count {
            let next = (i + 1) % ring_count;
            let inner_i = i * 2;
            let outer_i = inner_i + 1;
            let inner_next = next * 2;
            let outer_next = inner_next + 1;

            indices.push(inner_i);
            indices.push(inner_next);
            indices.push(outer_i);

            indices.push(outer_i);
            indices.push(inner_next);
            indices.push(outer_next);
        }

        Some(ShoreGeometry {
            vertices,
            indices,
            blend_distance,
        })
    }

    fn build_rectangle_shore_ring(vertices: &[WaterVertex], tessellation: u32) -> Vec<WaterVertex> {
        let row_stride = (tessellation + 1) as usize;
        let mut ring = Vec::new();

        // Top edge.
        for x in 0..=tessellation as usize {
            ring.push(vertices[x].clone());
        }
        // Right edge (without top corner).
        for z in 1..=tessellation as usize {
            ring.push(vertices[z * row_stride + tessellation as usize].clone());
        }
        // Bottom edge (without right corner).
        for x in (0..tessellation as usize).rev() {
            ring.push(vertices[tessellation as usize * row_stride + x].clone());
        }
        // Left edge (without top and bottom corners).
        for z in (1..tessellation as usize).rev() {
            ring.push(vertices[z * row_stride].clone());
        }

        ring
    }

    fn build_path_shore_ring(vertices: &[WaterVertex], point_count: usize) -> Vec<WaterVertex> {
        let mut ring = Vec::with_capacity(point_count * 2);
        for i in 0..point_count {
            ring.push(vertices[i * 2].clone());
        }
        for i in (0..point_count).rev() {
            ring.push(vertices[i * 2 + 1].clone());
        }
        ring
    }

    /// Create new water segment
    pub fn new(id: WaterSegmentId, water_body_id: WaterBodyId, shape: WaterShape) -> Self {
        let bounds = Self::calculate_bounds(&shape);

        Self {
            id,
            water_body_id,
            shape,
            local_properties: None,
            geometry: None,
            flow_data: None,
            dirty: true,
            bounds,
        }
    }

    /// Calculate bounding box for water shape
    fn calculate_bounds(shape: &WaterShape) -> WaterBounds {
        match shape {
            WaterShape::Rectangle {
                center,
                width,
                height,
                ..
            } => {
                let half_w = width / 2.0;
                let half_h = height / 2.0;
                WaterBounds {
                    min: Vec3::new(center.x - half_w, center.y - 1.0, center.z - half_h),
                    max: Vec3::new(center.x + half_w, center.y + 1.0, center.z + half_h),
                }
            }
            WaterShape::Circle { center, radius } => WaterBounds {
                min: Vec3::new(center.x - radius, center.y - 1.0, center.z - radius),
                max: Vec3::new(center.x + radius, center.y + 1.0, center.z + radius),
            },
            WaterShape::Polygon { vertices } => {
                if vertices.is_empty() {
                    return WaterBounds {
                        min: Vec3::ZERO,
                        max: Vec3::ZERO,
                    };
                }

                let mut min = vertices[0];
                let mut max = vertices[0];

                for vertex in vertices.iter().skip(1) {
                    min.x = min.x.min(vertex.x);
                    min.y = min.y.min(vertex.y);
                    min.z = min.z.min(vertex.z);
                    max.x = max.x.max(vertex.x);
                    max.y = max.y.max(vertex.y);
                    max.z = max.z.max(vertex.z);
                }

                WaterBounds { min, max }
            }
            WaterShape::Path { points, width, .. } => {
                if points.is_empty() {
                    return WaterBounds {
                        min: Vec3::ZERO,
                        max: Vec3::ZERO,
                    };
                }

                let mut min = points[0];
                let mut max = points[0];
                let half_width = width / 2.0;

                for point in points.iter().skip(1) {
                    min.x = (min.x).min(point.x - half_width);
                    min.y = (min.y).min(point.y);
                    min.z = (min.z).min(point.z - half_width);
                    max.x = (max.x).max(point.x + half_width);
                    max.y = (max.y).max(point.y);
                    max.z = (max.z).max(point.z + half_width);
                }

                WaterBounds { min, max }
            }
            WaterShape::Spline {
                control_points,
                width,
                ..
            } => {
                // Use control points as rough bounds approximation
                Self::calculate_bounds(&WaterShape::Path {
                    points: control_points.clone(),
                    width: *width,
                    width_variation: vec![1.0; control_points.len()],
                })
            }
        }
    }

    /// Generate geometry for this water segment
    pub fn generate_geometry(
        &mut self,
        water_body: &WaterBody,
        render_config: &WaterRenderConfig,
    ) -> TerrainResult<()> {
        let geometry = match &self.shape {
            WaterShape::Rectangle {
                center,
                width,
                height,
                rotation,
            } => self.generate_rectangle_geometry(
                *center,
                *width,
                *height,
                *rotation,
                water_body,
                render_config,
            )?,
            WaterShape::Circle { center, radius } => {
                self.generate_circle_geometry(*center, *radius, water_body, render_config)?
            }
            WaterShape::Polygon { vertices } => {
                self.generate_polygon_geometry(vertices, water_body, render_config)?
            }
            WaterShape::Path {
                points,
                width,
                width_variation,
            } => self.generate_path_geometry(
                points,
                *width,
                width_variation,
                water_body,
                render_config,
            )?,
            WaterShape::Spline {
                control_points,
                width,
                resolution,
            } => self.generate_spline_geometry(
                control_points,
                *width,
                *resolution,
                water_body,
                render_config,
            )?,
        };

        self.geometry = Some(geometry);
        self.dirty = false;
        Ok(())
    }

    /// Generate geometry for rectangular water area
    fn generate_rectangle_geometry(
        &self,
        center: Vec3,
        width: f32,
        height: f32,
        _rotation: f32,
        water_body: &WaterBody,
        render_config: &WaterRenderConfig,
    ) -> TerrainResult<WaterGeometry> {
        let tessellation = render_config.tessellation_level;
        let half_w = width / 2.0;
        let half_h = height / 2.0;

        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut depth_samples = Vec::new();

        // Generate tessellated grid
        for z in 0..=tessellation {
            for x in 0..=tessellation {
                let u = x as f32 / tessellation as f32;
                let v = z as f32 / tessellation as f32;

                let pos_x = center.x + (u - 0.5) * width;
                let pos_z = center.z + (v - 0.5) * height;
                let pos_y = center.y + water_body.water_level;

                let wave_height = water_body.calculate_wave_height(
                    Point2::new(pos_x, pos_z),
                    0.0, // Will be updated during rendering
                );

                let depth = ((center.y - (pos_y + wave_height)).abs()).max(0.0);
                depth_samples.push(depth);

                vertices.push(WaterVertex {
                    position: [pos_x, pos_y + wave_height, pos_z],
                    normal: [0.0, 1.0, 0.0], // Will be recalculated based on waves
                    tex_coords: [u, v],
                    depth,
                    flow_velocity: [0.0, 0.0],
                    color: water_body.color,
                });
            }
        }

        // Generate indices
        for z in 0..tessellation {
            for x in 0..tessellation {
                let i = z * (tessellation + 1) + x;
                let vertices_per_row = tessellation + 1;

                // First triangle
                indices.push(i);
                indices.push(i + vertices_per_row);
                indices.push(i + 1);

                // Second triangle
                indices.push(i + 1);
                indices.push(i + vertices_per_row);
                indices.push(i + vertices_per_row + 1);
            }
        }

        let (underwater_vertices, underwater_indices) =
            Self::build_underwater_geometry(&vertices, &indices, &depth_samples, water_body);
        let shore_ring = Self::build_rectangle_shore_ring(&vertices, tessellation);
        let shore_geometry = Self::build_shore_geometry_from_ring(
            &shore_ring,
            Self::compute_shore_blend_distance(render_config),
        );

        Ok(WaterGeometry {
            vertices,
            indices,
            underwater_vertices,
            underwater_indices,
            shore_geometry,
        })
    }

    /// Generate geometry for circular water area
    fn generate_circle_geometry(
        &self,
        center: Vec3,
        radius: f32,
        water_body: &WaterBody,
        render_config: &WaterRenderConfig,
    ) -> TerrainResult<WaterGeometry> {
        let segments = render_config.tessellation_level * 4; // More segments for circles
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut depth_samples = Vec::new();

        // Center vertex
        let center_wave = water_body.calculate_wave_height(Point2::new(center.x, center.z), 0.0);
        let center_depth =
            ((center.y - (center.y + water_body.water_level + center_wave)).abs()).max(0.0);
        depth_samples.push(center_depth);
        vertices.push(WaterVertex {
            position: [
                center.x,
                center.y + water_body.water_level + center_wave,
                center.z,
            ],
            normal: [0.0, 1.0, 0.0],
            tex_coords: [0.5, 0.5],
            depth: center_depth,
            flow_velocity: [0.0, 0.0],
            color: water_body.color,
        });

        // Ring vertices
        for i in 0..segments {
            let angle = (i as f32) * 2.0 * std::f32::consts::PI / (segments as f32);
            let x = center.x + radius * angle.cos();
            let z = center.z + radius * angle.sin();

            let u = 0.5 + 0.5 * angle.cos();
            let v = 0.5 + 0.5 * angle.sin();

            let wave_height = water_body.calculate_wave_height(Point2::new(x, z), 0.0);
            let depth =
                ((center.y - (center.y + water_body.water_level + wave_height)).abs()).max(0.0);
            depth_samples.push(depth);
            vertices.push(WaterVertex {
                position: [x, center.y + water_body.water_level + wave_height, z],
                normal: [0.0, 1.0, 0.0],
                tex_coords: [u, v],
                depth,
                flow_velocity: [0.0, 0.0],
                color: water_body.color,
            });
        }

        // Generate triangles from center to ring
        for i in 0..segments {
            let next_i = (i + 1) % segments;
            indices.push(0); // Center vertex
            indices.push((i + 1) as u32);
            indices.push((next_i + 1) as u32);
        }

        let (underwater_vertices, underwater_indices) =
            Self::build_underwater_geometry(&vertices, &indices, &depth_samples, water_body);
        let ring = vertices.iter().skip(1).cloned().collect::<Vec<_>>();
        let shore_geometry = Self::build_shore_geometry_from_ring(
            &ring,
            Self::compute_shore_blend_distance(render_config),
        );

        Ok(WaterGeometry {
            vertices,
            indices,
            underwater_vertices,
            underwater_indices,
            shore_geometry,
        })
    }

    /// Generate geometry for polygon water area
    fn generate_polygon_geometry(
        &self,
        vertices_points: &[Vec3],
        water_body: &WaterBody,
        render_config: &WaterRenderConfig,
    ) -> TerrainResult<WaterGeometry> {
        if vertices_points.len() < 3 {
            return Err(TerrainError::InvalidData(
                "Polygon must have at least 3 vertices".to_string(),
            ));
        }

        // Simple triangulation (ear clipping would be better for complex polygons)
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut depth_samples = Vec::new();

        // Add all polygon vertices
        for (i, point) in vertices_points.iter().enumerate() {
            let u = i as f32 / (vertices_points.len() - 1) as f32;
            let wave_height = water_body.calculate_wave_height(Point2::new(point.x, point.z), 0.0);
            let depth =
                ((point.y - (point.y + water_body.water_level + wave_height)).abs()).max(0.0);
            depth_samples.push(depth);
            vertices.push(WaterVertex {
                position: [
                    point.x,
                    point.y + water_body.water_level + wave_height,
                    point.z,
                ],
                normal: [0.0, 1.0, 0.0],
                tex_coords: [u, 0.5],
                depth,
                flow_velocity: [0.0, 0.0],
                color: water_body.color,
            });
        }

        // Simple fan triangulation from first vertex
        for i in 1..(vertices_points.len() - 1) {
            indices.push(0);
            indices.push(i as u32);
            indices.push((i + 1) as u32);
        }

        let (underwater_vertices, underwater_indices) =
            Self::build_underwater_geometry(&vertices, &indices, &depth_samples, water_body);
        let shore_geometry = Self::build_shore_geometry_from_ring(
            &vertices,
            Self::compute_shore_blend_distance(render_config),
        );

        Ok(WaterGeometry {
            vertices,
            indices,
            underwater_vertices,
            underwater_indices,
            shore_geometry,
        })
    }

    /// Generate geometry for path-based water (rivers, streams)
    fn generate_path_geometry(
        &self,
        points: &[Vec3],
        width: f32,
        width_variation: &[f32],
        water_body: &WaterBody,
        render_config: &WaterRenderConfig,
    ) -> TerrainResult<WaterGeometry> {
        if points.len() < 2 {
            return Err(TerrainError::InvalidData(
                "Path must have at least 2 points".to_string(),
            ));
        }

        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut depth_samples = Vec::new();
        let half_width = width / 2.0;

        // Generate vertices along the path
        for (i, point) in points.iter().enumerate() {
            let width_mult = width_variation.get(i).copied().unwrap_or(1.0);
            let local_half_width = half_width * width_mult;

            // Calculate direction for perpendicular
            let direction = if i < points.len() - 1 {
                (points[i + 1] - points[i]).normalize()
            } else {
                (points[i] - points[i - 1]).normalize()
            };

            let up = Vec3::new(0.0, 1.0, 0.0);
            let right = direction.cross(up).normalize();

            // Left and right vertices
            let left_pos = *point + right * local_half_width;
            let right_pos = *point - right * local_half_width;

            let v = i as f32 / (points.len() - 1) as f32;

            // Left vertex
            let left_wave =
                water_body.calculate_wave_height(Point2::new(left_pos.x, left_pos.z), 0.0);
            let left_depth =
                ((left_pos.y - (left_pos.y + water_body.water_level + left_wave)).abs()).max(0.0);
            depth_samples.push(left_depth);
            vertices.push(WaterVertex {
                position: [
                    left_pos.x,
                    left_pos.y + water_body.water_level + left_wave,
                    left_pos.z,
                ],
                normal: [0.0, 1.0, 0.0],
                tex_coords: [0.0, v],
                depth: left_depth,
                flow_velocity: [direction.x, direction.z],
                color: water_body.color,
            });

            // Right vertex
            let right_wave =
                water_body.calculate_wave_height(Point2::new(right_pos.x, right_pos.z), 0.0);
            let right_depth = ((right_pos.y - (right_pos.y + water_body.water_level + right_wave))
                .abs())
            .max(0.0);
            depth_samples.push(right_depth);
            vertices.push(WaterVertex {
                position: [
                    right_pos.x,
                    right_pos.y + water_body.water_level + right_wave,
                    right_pos.z,
                ],
                normal: [0.0, 1.0, 0.0],
                tex_coords: [1.0, v],
                depth: right_depth,
                flow_velocity: [direction.x, direction.z],
                color: water_body.color,
            });
        }

        // Generate indices for triangle strip
        for i in 0..(points.len() - 1) {
            let base = (i * 2) as u32;

            // First triangle
            indices.push(base);
            indices.push(base + 2);
            indices.push(base + 1);

            // Second triangle
            indices.push(base + 1);
            indices.push(base + 2);
            indices.push(base + 3);
        }

        let (underwater_vertices, underwater_indices) =
            Self::build_underwater_geometry(&vertices, &indices, &depth_samples, water_body);
        let shore_ring = Self::build_path_shore_ring(&vertices, points.len());
        let shore_geometry = Self::build_shore_geometry_from_ring(
            &shore_ring,
            Self::compute_shore_blend_distance(render_config),
        );

        Ok(WaterGeometry {
            vertices,
            indices,
            underwater_vertices,
            underwater_indices,
            shore_geometry,
        })
    }

    /// Generate geometry for spline-based water
    fn generate_spline_geometry(
        &self,
        control_points: &[Vec3],
        width: f32,
        resolution: u32,
        water_body: &WaterBody,
        render_config: &WaterRenderConfig,
    ) -> TerrainResult<WaterGeometry> {
        if control_points.len() < 2 {
            return Err(TerrainError::InvalidData(
                "Spline requires at least 2 control points".to_string(),
            ));
        }

        let steps = resolution.max(2) as usize;
        let mut path_points = Vec::with_capacity(steps);

        for i in 0..steps {
            let t = if steps == 1 {
                0.0
            } else {
                i as f32 / (steps - 1) as f32
            };
            path_points.push(Self::sample_catmull_rom(control_points, t));
        }

        self.generate_path_geometry(
            &path_points,
            width,
            &vec![1.0; path_points.len()],
            water_body,
            render_config,
        )
    }

    fn build_underwater_geometry(
        surface_vertices: &[WaterVertex],
        surface_indices: &[u32],
        depths: &[f32],
        water_body: &WaterBody,
    ) -> (Vec<WaterVertex>, Vec<u32>) {
        if surface_vertices.is_empty() {
            return (Vec::new(), Vec::new());
        }

        let mut underwater_vertices = Vec::with_capacity(surface_vertices.len());
        for (idx, v) in surface_vertices.iter().enumerate() {
            let depth = depths.get(idx).copied().unwrap_or(0.0).max(0.0);
            underwater_vertices.push(WaterVertex {
                position: [v.position[0], v.position[1] - depth.max(0.2), v.position[2]],
                normal: [0.0, -1.0, 0.0],
                tex_coords: v.tex_coords,
                depth,
                flow_velocity: v.flow_velocity,
                color: water_body.color,
            });
        }

        (underwater_vertices, surface_indices.to_vec())
    }

    fn sample_catmull_rom(points: &[Vec3], t: f32) -> Vec3 {
        if points.len() == 2 {
            return points[0].lerp(points[1], t);
        }

        let segment_count = points.len() - 1;
        let scaled = (t * segment_count as f32).clamp(0.0, segment_count as f32 - 1e-6);
        let seg = scaled.floor() as isize;
        let local_t = scaled - seg as f32;

        let i0 = (seg - 1).max(0) as usize;
        let i1 = seg as usize;
        let i2 = (seg + 1).min(segment_count as isize) as usize;
        let i3 = (seg + 2).min(segment_count as isize) as usize;

        let p0 = points[i0];
        let p1 = points[i1];
        let p2 = points[i2];
        let p3 = points[i3];

        let t2 = local_t * local_t;
        let t3 = t2 * local_t;

        0.5 * ((p1 * 2.0)
            + (p2 - p0) * local_t
            + (p0 * 2.0 - p1 * 5.0 + p2 * 4.0 - p3) * t2
            + (p3 - p0 + (p1 * 3.0 - p2 * 3.0)) * t3)
    }
}

impl WaterManager {
    /// Create new water manager
    pub fn new() -> Self {
        Self {
            water_bodies: HashMap::new(),
            segments: HashMap::new(),
            simulation_config: WaterSimulationConfig::default(),
            render_config: WaterRenderConfig::default(),
            next_body_id: 1,
            next_segment_id: 1,
            simulation_time: 0.0,
            stats: WaterStats::default(),
            enabled: true,
        }
    }

    /// Initialize water resources and rebuild water geometry.
    pub fn init(&mut self) -> TerrainResult<()> {
        if self.enabled {
            self.update_geometry()?;
        }
        Ok(())
    }

    /// Reset water data to defaults
    pub fn reset(&mut self) -> TerrainResult<()> {
        self.water_bodies.clear();
        self.segments.clear();
        self.next_body_id = 1;
        self.next_segment_id = 1;
        self.simulation_time = 0.0;
        self.stats = WaterStats::default();
        self.enabled = true;
        Ok(())
    }

    /// Update water simulation with a default timestep
    pub fn update(&mut self) -> TerrainResult<()> {
        self.update_with_delta(1.0 / 60.0)
    }

    /// Validate water geometry and collect per-segment draw parameters.
    ///
    /// Returns a list of (vertex_count, index_count) tuples for each segment with valid geometry,
    /// ready for GPU submission via `render_pass_draw`.  Invalid segments produce errors.
    pub fn render(&self, _view: &Mat4, _projection: &Mat4) -> TerrainResult<()> {
        if !self.enabled {
            return Ok(());
        }

        for segment in self.segments.values() {
            let Some(geometry) = segment.geometry.as_ref() else {
                continue;
            };

            if geometry.indices.len() % 3 != 0 {
                return Err(TerrainError::InvalidData(format!(
                    "Water segment {} has non-triangle index count {}",
                    segment.id,
                    geometry.indices.len()
                )));
            }
        }
        Ok(())
    }

    /// Submit GPU draw calls for water surfaces.
    ///
    /// Caller must set the water pipeline and camera bind group (group 0) first.
    /// `mesh_fn` is called once per enabled segment with geometry to obtain the
    /// (vertex_slice, index_slice, index_count) for that segment's mesh, then
    /// issues `draw_indexed` per surface.  Mirrors C++ W3DTerrainVisual per-surface draw loop.
    pub fn render_pass_draw<'a, FMesh>(
        &self,
        render_pass: &mut RenderPass<'a>,
        mut mesh_fn: FMesh,
    ) -> TerrainResult<()>
    where
        FMesh: FnMut() -> Option<(wgpu::BufferSlice<'a>, wgpu::BufferSlice<'a>, u32)>,
    {
        if !self.enabled {
            return Ok(());
        }

        // Iterate all enabled segments with geometry, submitting one draw call each.
        // Matches C++ W3DTerrainVisual::doRender which loops water render objects.
        for _segment in self.segments.values() {
            let Some((vertex_slice, index_slice, index_count)) = mesh_fn() else {
                continue;
            };
            if index_count == 0 {
                continue;
            }
            render_pass.set_vertex_buffer(0, vertex_slice);
            render_pass.set_index_buffer(index_slice, wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..index_count, 0, 0..1);
        }

        Ok(())
    }

    /// Create new water body
    pub fn create_water_body(&mut self, name: String, water_type: WaterType) -> WaterBodyId {
        let body = WaterBody::new(self.next_body_id, name, water_type);
        let id = body.id;

        self.water_bodies.insert(id, body);
        self.next_body_id += 1;
        self.stats.total_bodies += 1;

        id
    }

    /// Create new water segment
    pub fn create_segment(
        &mut self,
        water_body_id: WaterBodyId,
        shape: WaterShape,
    ) -> TerrainResult<WaterSegmentId> {
        let segment = WaterSegment::new(self.next_segment_id, water_body_id, shape);
        let id = segment.id;

        // Add segment to water body
        if let Some(body) = self.water_bodies.get_mut(&water_body_id) {
            body.add_segment(id);
        } else {
            return Err(TerrainError::InvalidData(format!(
                "Water body {} not found",
                water_body_id
            )));
        }

        self.segments.insert(id, segment);
        self.next_segment_id += 1;
        self.stats.total_segments += 1;

        Ok(id)
    }

    /// Update water simulation and animation
    pub fn update_with_delta(&mut self, delta_time: f32) -> TerrainResult<()> {
        if !self.enabled {
            return Ok(());
        }

        let start_time = std::time::Instant::now();

        self.simulation_time += delta_time;

        // Update water body animations
        for body in self.water_bodies.values_mut() {
            body.update_animation(delta_time);
        }

        // Update flow simulation if enabled
        if self.simulation_config.enable_flow_simulation {
            self.update_flow_simulation(delta_time)?;
        }

        // Regenerate geometry for dirty segments
        self.update_geometry()?;

        self.stats.simulation_time = start_time.elapsed();
        Ok(())
    }

    /// Enable or disable water simulation and rendering.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.stats.active_simulations = 0;
        }
    }

    /// Update flow simulation for all segments
    fn update_flow_simulation(&mut self, delta_time: f32) -> TerrainResult<()> {
        fn index_for(x: u32, y: u32, width: u32) -> usize {
            (y * width + x) as usize
        }

        fn sample_velocity(
            flow: &WaterFlowData,
            bounds: &WaterBounds,
            world_pos: Vec3,
        ) -> Vector2<f32> {
            let (w, h) = flow.field_resolution;
            if w == 0 || h == 0 {
                return Vector2::zeros();
            }

            let size = bounds.max - bounds.min;
            let nx = if size.x.abs() < f32::EPSILON {
                0.0
            } else {
                ((world_pos.x - bounds.min.x) / size.x).clamp(0.0, 1.0)
            };
            let ny = if size.z.abs() < f32::EPSILON {
                0.0
            } else {
                ((world_pos.z - bounds.min.z) / size.z).clamp(0.0, 1.0)
            };

            let fx = nx * (w.saturating_sub(1) as f32);
            let fy = ny * (h.saturating_sub(1) as f32);
            let x0 = fx.floor() as u32;
            let y0 = fy.floor() as u32;
            let x1 = (x0 + 1).min(w.saturating_sub(1));
            let y1 = (y0 + 1).min(h.saturating_sub(1));
            let tx = fx - x0 as f32;
            let ty = fy - y0 as f32;

            let v00 = flow.velocity_field[index_for(x0, y0, w)];
            let v10 = flow.velocity_field[index_for(x1, y0, w)];
            let v01 = flow.velocity_field[index_for(x0, y1, w)];
            let v11 = flow.velocity_field[index_for(x1, y1, w)];

            let v0 = v00 * (1.0 - tx) + v10 * tx;
            let v1 = v01 * (1.0 - tx) + v11 * tx;
            v0 * (1.0 - ty) + v1 * ty
        }

        self.stats.active_simulations = self
            .segments
            .values()
            .filter(|s| s.flow_data.is_some())
            .count() as u32;

        for segment in self.segments.values_mut() {
            let Some(flow) = segment.flow_data.as_mut() else {
                continue;
            };

            let (w, h) = flow.field_resolution;
            if w == 0 || h == 0 {
                continue;
            }

            let expected = (w * h) as usize;
            if flow.velocity_field.len() != expected {
                flow.velocity_field = vec![Vector2::zeros(); expected];
            }
            if flow.pressure_field.len() != expected {
                flow.pressure_field = vec![0.0; expected];
            }

            let dt = if flow.time_step > 0.0 {
                flow.time_step.min(delta_time.max(0.0))
            } else {
                delta_time.max(0.0)
            };

            let bounds = segment.bounds.clone();
            let size = bounds.max - bounds.min;
            let cell_w = if w > 0 { size.x / w as f32 } else { 0.0 };
            let cell_h = if h > 0 { size.z / h as f32 } else { 0.0 };

            let mut new_field = flow.velocity_field.clone();

            for y in 0..h {
                for x in 0..w {
                    let idx = index_for(x, y, w);
                    let mut velocity = flow.velocity_field[idx];

                    // Smooth with neighbors to mimic shallow water diffusion.
                    let mut neighbor_sum = Vector2::zeros();
                    let mut neighbor_count = 0.0;
                    for (nx, ny) in [
                        (x.saturating_sub(1), y),
                        ((x + 1).min(w - 1), y),
                        (x, y.saturating_sub(1)),
                        (x, (y + 1).min(h - 1)),
                    ] {
                        neighbor_sum += flow.velocity_field[index_for(nx, ny, w)];
                        neighbor_count += 1.0;
                    }
                    if neighbor_count > 0.0 {
                        let avg = neighbor_sum / neighbor_count;
                        velocity = velocity * 0.7 + avg * 0.3;
                    }

                    // Apply obstacle resistance and deflection.
                    let world_x = bounds.min.x + (x as f32 + 0.5) * cell_w;
                    let world_z = bounds.min.z + (y as f32 + 0.5) * cell_h;
                    let cell_pos = Point2::new(world_x, world_z);
                    for obstacle in &flow.obstacles {
                        let offset = cell_pos - obstacle.position;
                        let dist = offset.norm().max(0.001);
                        if dist < obstacle.radius {
                            let resistance = obstacle.flow_resistance.clamp(0.0, 1.0);
                            velocity *= 1.0 - resistance;
                            let push = offset.normalize() * (obstacle.radius - dist);
                            velocity += Vector2::new(push.x, push.y) * resistance;
                        }
                    }

                    // Damping to stabilize the flow field.
                    velocity *= (1.0 - 0.4 * dt).clamp(0.0, 1.0);
                    new_field[idx] = velocity;
                    flow.pressure_field[idx] = velocity.norm();
                }
            }

            flow.velocity_field = new_field;

            if let Some(geometry) = segment.geometry.as_mut() {
                for vertex in &mut geometry.vertices {
                    let world =
                        Vec3::new(vertex.position[0], vertex.position[1], vertex.position[2]);
                    let v = sample_velocity(flow, &bounds, world);
                    vertex.flow_velocity = [v.x, v.y];
                }
                for vertex in &mut geometry.underwater_vertices {
                    let world =
                        Vec3::new(vertex.position[0], vertex.position[1], vertex.position[2]);
                    let v = sample_velocity(flow, &bounds, world);
                    vertex.flow_velocity = [v.x, v.y];
                }
                if let Some(shore) = geometry.shore_geometry.as_mut() {
                    for vertex in &mut shore.vertices {
                        let world =
                            Vec3::new(vertex.position[0], vertex.position[1], vertex.position[2]);
                        let v = sample_velocity(flow, &bounds, world);
                        vertex.flow_velocity = [v.x, v.y];
                    }
                }
            }

            // Flow simulation updates per-vertex velocity fields directly; do not force a full
            // geometry regeneration every frame.
        }

        Ok(())
    }

    /// Update geometry for dirty segments
    fn update_geometry(&mut self) -> TerrainResult<()> {
        let dirty_segments: Vec<WaterSegmentId> = self
            .segments
            .values()
            .filter(|segment| segment.dirty)
            .map(|segment| segment.id)
            .collect();

        for segment_id in dirty_segments {
            if let Some(segment) = self.segments.get_mut(&segment_id) {
                if let Some(water_body) = self.water_bodies.get(&segment.water_body_id) {
                    segment.generate_geometry(water_body, &self.render_config)?;
                }
            }
        }

        self.update_statistics();
        Ok(())
    }

    /// Update statistics
    fn update_statistics(&mut self) {
        let mut total_vertices = 0;
        let mut total_triangles = 0;
        let mut memory_usage = 0;

        for segment in self.segments.values() {
            if let Some(geometry) = &segment.geometry {
                total_vertices += geometry.vertices.len() as u32;
                total_triangles += geometry.indices.len() as u32 / 3;
                memory_usage += std::mem::size_of_val(&*geometry.vertices) as u64;
                memory_usage += std::mem::size_of_val(&*geometry.indices) as u64;
            }
        }

        self.stats.total_vertices = total_vertices;
        self.stats.total_triangles = total_triangles;
        self.stats.memory_usage = memory_usage;
    }

    /// Get water body by ID
    pub fn get_water_body(&self, id: WaterBodyId) -> Option<&WaterBody> {
        self.water_bodies.get(&id)
    }

    /// Get mutable water body by ID
    pub fn get_water_body_mut(&mut self, id: WaterBodyId) -> Option<&mut WaterBody> {
        self.water_bodies.get_mut(&id)
    }

    /// Get water segment by ID
    pub fn get_segment(&self, id: WaterSegmentId) -> Option<&WaterSegment> {
        self.segments.get(&id)
    }

    /// Get mutable water segment by ID
    pub fn get_segment_mut(&mut self, id: WaterSegmentId) -> Option<&mut WaterSegment> {
        self.segments.get_mut(&id)
    }

    /// Find water bodies near a position
    pub fn find_water_near(&self, position: Vec3, radius: f32) -> Vec<WaterBodyId> {
        self.water_bodies
            .values()
            .filter(|body| {
                body.segments
                    .iter()
                    .filter_map(|&id| self.segments.get(&id))
                    .any(|segment| {
                        let center_x = (segment.bounds.min.x + segment.bounds.max.x) / 2.0;
                        let center_z = (segment.bounds.min.z + segment.bounds.max.z) / 2.0;
                        let center = Vec3::new(center_x, position.y, center_z);
                        (center - position).length() <= radius
                    })
            })
            .map(|body| body.id)
            .collect()
    }

    /// Get performance statistics
    pub fn get_stats(&self) -> &WaterStats {
        &self.stats
    }

    /// Clear all water bodies and segments
    pub fn clear(&mut self) {
        self.water_bodies.clear();
        self.segments.clear();
        self.next_body_id = 1;
        self.next_segment_id = 1;
        self.simulation_time = 0.0;
        self.stats = WaterStats::default();
        self.enabled = true;
    }
}

impl Default for WaterManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Legacy-friendly aliases aligning with C++ types
pub type WaterSystem = WaterManager;
pub type WaterSettings = WaterRenderConfig;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_water_body_creation() {
        let water_type = WaterType::Lake {
            depth: 5.0,
            clarity: 0.8,
            temperature: 15.0,
        };

        let body = WaterBody::new(1, "Test Lake".to_string(), water_type);
        assert_eq!(body.id, 1);
        assert_eq!(body.name, "Test Lake");
        assert!(body.segments.is_empty());
    }

    #[test]
    fn test_water_segment_creation() {
        let shape = WaterShape::Rectangle {
            center: Vec3::new(0.0, 0.0, 0.0),
            width: 10.0,
            height: 10.0,
            rotation: 0.0,
        };

        let segment = WaterSegment::new(1, 1, shape);
        assert_eq!(segment.id, 1);
        assert_eq!(segment.water_body_id, 1);
        assert!(segment.dirty);
    }

    #[test]
    fn test_water_manager() {
        let mut manager = WaterManager::new();

        let body_id = manager.create_water_body(
            "Test River".to_string(),
            WaterType::River {
                flow_speed: 2.0,
                flow_direction: Vec3::new(1.0, 0.0, 0.0),
                depth: 2.0,
                turbulence: 0.1,
            },
        );

        assert_eq!(body_id, 1);
        assert!(manager.get_water_body(body_id).is_some());

        let shape = WaterShape::Path {
            points: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(10.0, 0.0, 0.0),
                Vec3::new(20.0, 0.0, 5.0),
            ],
            width: 5.0,
            width_variation: vec![1.0, 1.2, 0.8],
        };

        let segment_result = manager.create_segment(body_id, shape);
        assert!(segment_result.is_ok());

        let segment_id = segment_result.unwrap();
        assert_eq!(segment_id, 1);

        // Check that body now contains the segment
        let body = manager.get_water_body(body_id).unwrap();
        assert_eq!(body.segments.len(), 1);
        assert_eq!(body.segments[0], segment_id);
    }

    #[test]
    fn test_wave_calculation() {
        let water_type = WaterType::Ocean {
            wave_height: 1.0,
            wave_frequency: 0.5,
            tide_level: 0.0,
            current_direction: Vector2::new(1.0, 0.0),
        };

        let mut body = WaterBody::new(1, "Test Ocean".to_string(), water_type);
        body.animation.wave_amplitude = 0.5;

        let wave_height1 = body.calculate_wave_height(Point2::new(0.0, 0.0), 0.0);
        let wave_height2 = body.calculate_wave_height(Point2::new(10.0, 0.0), 0.0);

        // Wave heights should be different at different positions
        assert_ne!(wave_height1, wave_height2);
    }

    #[test]
    fn test_bounds_calculation() {
        let rectangle_shape = WaterShape::Rectangle {
            center: Vec3::new(5.0, 0.0, 5.0),
            width: 10.0,
            height: 8.0,
            rotation: 0.0,
        };

        let segment = WaterSegment::new(1, 1, rectangle_shape);
        assert_eq!(segment.bounds.min.x, 0.0);
        assert_eq!(segment.bounds.max.x, 10.0);
        assert_eq!(segment.bounds.min.z, 1.0);
        assert_eq!(segment.bounds.max.z, 9.0);
    }

    #[test]
    fn test_rectangle_water_generates_shore_geometry() {
        let mut segment = WaterSegment::new(
            1,
            1,
            WaterShape::Rectangle {
                center: Vec3::new(0.0, 0.0, 0.0),
                width: 16.0,
                height: 12.0,
                rotation: 0.0,
            },
        );
        let body = WaterBody::new(
            1,
            "ShoreRect".to_string(),
            WaterType::Lake {
                depth: 2.0,
                clarity: 0.7,
                temperature: 20.0,
            },
        );
        segment
            .generate_geometry(&body, &WaterRenderConfig::default())
            .unwrap();

        let geometry = segment.geometry.as_ref().unwrap();
        let shore = geometry.shore_geometry.as_ref().unwrap();

        assert!(!shore.vertices.is_empty());
        assert_eq!(shore.indices.len() % 3, 0);
        assert!(shore.vertices.iter().any(|v| v.color[3] == 0.0));
        assert!(shore.blend_distance > 0.0);
    }

    #[test]
    fn test_path_water_generates_shore_geometry() {
        let mut segment = WaterSegment::new(
            7,
            2,
            WaterShape::Path {
                points: vec![
                    Vec3::new(0.0, 0.0, 0.0),
                    Vec3::new(8.0, 0.0, 1.0),
                    Vec3::new(16.0, 0.0, 4.0),
                ],
                width: 4.0,
                width_variation: vec![1.0, 1.2, 0.9],
            },
        );
        let body = WaterBody::new(
            2,
            "ShorePath".to_string(),
            WaterType::River {
                flow_speed: 1.0,
                flow_direction: Vec3::new(1.0, 0.0, 0.0),
                depth: 1.5,
                turbulence: 0.2,
            },
        );
        segment
            .generate_geometry(&body, &WaterRenderConfig::default())
            .unwrap();

        let geometry = segment.geometry.as_ref().unwrap();
        let shore = geometry.shore_geometry.as_ref().unwrap();

        assert!(!shore.vertices.is_empty());
        assert_eq!(shore.indices.len() % 3, 0);
    }
}
