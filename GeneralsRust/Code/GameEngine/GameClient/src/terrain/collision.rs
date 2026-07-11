//! Terrain Collision System
//!
//! Manages collision detection and physics interactions with terrain
//! including height sampling, normal calculation, and spatial queries.

use crate::terrain::{TerrainError, TerrainResult};
use glam::Vec3;
use nalgebra::Point2;
use std::collections::HashMap;

/// Collision query result containing hit information
#[derive(Debug, Clone)]
pub struct CollisionResult {
    /// Whether collision occurred
    pub hit: bool,

    /// World position of collision point
    pub position: Vec3,

    /// Surface normal at collision point
    pub normal: Vec3,

    /// Distance from query origin to collision point
    pub distance: f32,

    /// Material type at collision point
    pub material: TerrainMaterial,

    /// Surface properties at collision point
    pub surface_properties: SurfaceProperties,
}

/// Terrain material types affecting physics and gameplay
#[derive(Debug, Clone, PartialEq)]
pub enum TerrainMaterial {
    /// Grass and soft ground
    Grass { density: f32, friction: f32 },

    /// Dirt and earth
    Dirt { compaction: f32, moisture: f32 },

    /// Rocky terrain
    Rock { hardness: f32, roughness: f32 },

    /// Sand and loose particles
    Sand { grain_size: f32, stability: f32 },

    /// Snow and ice
    Snow { depth: f32, temperature: f32 },

    /// Water and liquid surfaces
    Water { depth: f32, flow_speed: f32 },

    /// Artificial surfaces (roads, buildings)
    Concrete { condition: f32, friction: f32 },

    /// Muddy terrain
    Mud { viscosity: f32, depth: f32 },
}

/// Surface properties affecting movement and physics
#[derive(Debug, Clone)]
pub struct SurfaceProperties {
    /// Movement speed multiplier for units
    pub movement_modifier: f32,

    /// Surface friction coefficient
    pub friction: f32,

    /// Bounce/restitution coefficient
    pub restitution: f32,

    /// Whether surface can be traversed
    pub traversable: bool,

    /// Surface slope in radians
    pub slope: f32,

    /// Surface stability (for dynamic terrain)
    pub stability: f32,

    /// Sound dampening factor
    pub sound_dampening: f32,

    /// Visual/particle effects when interacting
    pub interaction_effects: Vec<InteractionEffect>,
}

/// Effects triggered by terrain interaction
#[derive(Debug, Clone, PartialEq)]
pub enum InteractionEffect {
    /// Dust particles
    DustCloud { intensity: f32 },

    /// Footprint/track marks
    Footprints { duration: f32 },

    /// Splash effects for water
    Splash { size: f32 },

    /// Sound effects
    Sound { sound_type: String, volume: f32 },

    /// Screen shake
    ScreenShake { intensity: f32 },

    /// Damage to units
    Damage { amount: f32, damage_type: String },
}

/// Ray for collision testing
#[derive(Debug, Clone)]
pub struct Ray {
    /// Ray origin point
    pub origin: Vec3,

    /// Ray direction (should be normalized)
    pub direction: Vec3,

    /// Maximum ray length
    pub max_distance: f32,
}

/// Sphere for collision testing
#[derive(Debug, Clone)]
pub struct CollisionSphere {
    /// Sphere center
    pub center: Vec3,

    /// Sphere radius
    pub radius: f32,
}

/// Axis-aligned bounding box for collision testing
#[derive(Debug, Clone)]
pub struct CollisionAABB {
    /// Minimum bounds
    pub min: Vec3,

    /// Maximum bounds
    pub max: Vec3,
}

/// Oriented bounding box for collision testing
#[derive(Debug, Clone)]
pub struct CollisionOBB {
    /// Box center
    pub center: Vec3,

    /// Box extents (half-sizes)
    pub extents: Vec3,

    /// Box orientation axes
    pub axes: [Vec3; 3],
}

/// Spatial acceleration structure for fast collision queries
#[derive(Debug)]
pub struct CollisionGrid {
    /// Grid cell size
    cell_size: f32,

    /// Grid dimensions
    grid_size: (i32, i32),

    /// Grid origin (world position of grid[0][0])
    origin: Point2<f32>,

    /// Grid cells containing collision data
    cells: HashMap<(i32, i32), CollisionCell>,
}

/// Collision data for a single grid cell
#[derive(Debug, Clone)]
pub struct CollisionCell {
    /// Terrain triangles in this cell
    pub triangles: Vec<CollisionTriangle>,

    /// Heightmap samples for fast height queries
    pub height_samples: Vec<f32>,

    /// Material information
    pub materials: Vec<TerrainMaterial>,

    /// Surface properties
    pub properties: Vec<SurfaceProperties>,
}

impl CollisionCell {
    fn sample_height_bilinear(&self, local_x: f32, local_y: f32) -> Option<f32> {
        match self.height_samples.len() {
            0 => None,
            1 => self.height_samples.first().copied(),
            len => {
                let side = (len as f32).sqrt() as usize;
                if side < 2 || side * side != len {
                    return self.height_samples.first().copied();
                }

                let sample_x = local_x.clamp(0.0, 1.0) * (side - 1) as f32;
                let sample_y = local_y.clamp(0.0, 1.0) * (side - 1) as f32;
                let x0 = sample_x.floor() as usize;
                let y0 = sample_y.floor() as usize;
                let x1 = (x0 + 1).min(side - 1);
                let y1 = (y0 + 1).min(side - 1);
                let tx = sample_x - x0 as f32;
                let ty = sample_y - y0 as f32;

                let h00 = self.height_samples[y0 * side + x0];
                let h10 = self.height_samples[y0 * side + x1];
                let h01 = self.height_samples[y1 * side + x0];
                let h11 = self.height_samples[y1 * side + x1];
                let top = h00 + (h10 - h00) * tx;
                let bottom = h01 + (h11 - h01) * tx;
                Some(top + (bottom - top) * ty)
            }
        }
    }
}

/// Triangle for collision detection
#[derive(Debug, Clone)]
pub struct CollisionTriangle {
    /// Triangle vertices
    pub vertices: [Vec3; 3],

    /// Precomputed triangle normal
    pub normal: Vec3,

    /// Material at this triangle
    pub material: TerrainMaterial,

    /// Surface properties
    pub properties: SurfaceProperties,
}

/// Manages terrain collision detection and spatial queries
#[derive(Debug)]
pub struct TerrainCollision {
    /// Collision grid for spatial acceleration
    grid: CollisionGrid,

    /// Global collision configuration
    config: CollisionConfig,

    /// Performance statistics
    stats: CollisionStats,
}

/// Configuration for collision detection
#[derive(Debug, Clone)]
pub struct CollisionConfig {
    /// Grid cell size for spatial partitioning
    pub grid_cell_size: f32,

    /// Maximum query distance for optimization
    pub max_query_distance: f32,

    /// Height sampling resolution
    pub height_sample_resolution: u32,

    /// Whether to use triangle-level collision
    pub enable_triangle_collision: bool,

    /// Whether to cache collision results
    pub enable_result_caching: bool,

    /// Maximum slope for traversable terrain (radians)
    pub max_traversable_slope: f32,
}

/// Performance statistics for collision system
#[derive(Debug, Default)]
pub struct CollisionStats {
    pub ray_tests: u64,
    pub sphere_tests: u64,
    pub aabb_tests: u64,
    pub triangle_tests: u64,
    pub height_queries: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub avg_query_time: std::time::Duration,
    pub memory_usage: u64,
}

impl Default for TerrainMaterial {
    fn default() -> Self {
        TerrainMaterial::Grass {
            density: 0.8,
            friction: 0.6,
        }
    }
}

impl Default for SurfaceProperties {
    fn default() -> Self {
        Self {
            movement_modifier: 1.0,
            friction: 0.6,
            restitution: 0.1,
            traversable: true,
            slope: 0.0,
            stability: 1.0,
            sound_dampening: 0.1,
            interaction_effects: Vec::new(),
        }
    }
}

impl Default for CollisionConfig {
    fn default() -> Self {
        Self {
            grid_cell_size: 32.0,
            max_query_distance: 1000.0,
            height_sample_resolution: 16,
            enable_triangle_collision: true,
            enable_result_caching: true,
            max_traversable_slope: std::f32::consts::PI / 4.0, // 45 degrees
        }
    }
}

impl TerrainMaterial {
    /// Get movement speed modifier for this material
    pub fn get_movement_modifier(&self) -> f32 {
        match self {
            TerrainMaterial::Grass { .. } => 1.0,
            TerrainMaterial::Dirt { compaction, .. } => 0.8 + 0.2 * compaction,
            TerrainMaterial::Rock { .. } => 1.2,
            TerrainMaterial::Sand { stability, .. } => 0.6 + 0.3 * stability,
            TerrainMaterial::Snow { depth, .. } => (1.0 - depth * 0.1).max(0.3),
            TerrainMaterial::Water { depth, .. } => {
                if *depth > 1.0 {
                    0.0
                } else {
                    0.5
                }
            }
            TerrainMaterial::Concrete { condition, .. } => 1.3 * condition,
            TerrainMaterial::Mud { viscosity, .. } => (0.8 - viscosity * 0.5).max(0.2),
        }
    }

    /// Get friction coefficient for this material
    pub fn get_friction(&self) -> f32 {
        match self {
            TerrainMaterial::Grass { friction, .. } => *friction,
            TerrainMaterial::Dirt { .. } => 0.7,
            TerrainMaterial::Rock { roughness, .. } => 0.8 + roughness * 0.2,
            TerrainMaterial::Sand { .. } => 0.4,
            TerrainMaterial::Snow { temperature, .. } => {
                if *temperature < 0.0 {
                    0.1
                } else {
                    0.3
                }
            }
            TerrainMaterial::Water { .. } => 0.0,
            TerrainMaterial::Concrete { friction, .. } => *friction,
            TerrainMaterial::Mud { viscosity, .. } => 0.2 + viscosity * 0.3,
        }
    }

    /// Get sound dampening factor
    pub fn get_sound_dampening(&self) -> f32 {
        match self {
            TerrainMaterial::Grass { .. } => 0.3,
            TerrainMaterial::Dirt { .. } => 0.2,
            TerrainMaterial::Rock { .. } => 0.0,
            TerrainMaterial::Sand { .. } => 0.4,
            TerrainMaterial::Snow { depth, .. } => 0.5 + depth * 0.1,
            TerrainMaterial::Water { .. } => 0.1,
            TerrainMaterial::Concrete { .. } => 0.0,
            TerrainMaterial::Mud { .. } => 0.6,
        }
    }
}

impl SurfaceProperties {
    /// Create surface properties from terrain material
    pub fn from_material(material: &TerrainMaterial) -> Self {
        let mut properties = Self::default();

        properties.movement_modifier = material.get_movement_modifier();
        properties.friction = material.get_friction();
        properties.sound_dampening = material.get_sound_dampening();

        // Set material-specific properties
        match material {
            TerrainMaterial::Water { depth, .. } => {
                properties.traversable = *depth < 2.0;
                properties
                    .interaction_effects
                    .push(InteractionEffect::Splash { size: 1.0 });
            }
            TerrainMaterial::Snow { .. } => {
                properties
                    .interaction_effects
                    .push(InteractionEffect::Footprints { duration: 30.0 });
            }
            TerrainMaterial::Sand { .. } => {
                properties
                    .interaction_effects
                    .push(InteractionEffect::DustCloud { intensity: 0.5 });
                properties
                    .interaction_effects
                    .push(InteractionEffect::Footprints { duration: 10.0 });
            }
            TerrainMaterial::Mud { .. } => {
                properties
                    .interaction_effects
                    .push(InteractionEffect::Footprints { duration: 60.0 });
            }
            _ => {}
        }

        properties
    }

    /// Check if surface is traversable given slope
    pub fn is_traversable(&self, max_slope: f32) -> bool {
        self.traversable && self.slope <= max_slope
    }
}

impl Ray {
    /// Create new ray
    pub fn new(origin: Vec3, direction: Vec3, max_distance: f32) -> Self {
        Self {
            origin,
            direction: direction.normalize(),
            max_distance,
        }
    }

    /// Get point along ray at given distance
    pub fn point_at_distance(&self, distance: f32) -> Vec3 {
        self.origin + self.direction * distance
    }
}

impl CollisionTriangle {
    /// Create new collision triangle
    pub fn new(vertices: [Vec3; 3], material: TerrainMaterial) -> Self {
        // Calculate triangle normal
        let edge1 = vertices[1] - vertices[0];
        let edge2 = vertices[2] - vertices[0];
        let normal = edge1.cross(edge2).normalize();

        let properties = SurfaceProperties::from_material(&material);

        Self {
            vertices,
            normal,
            material: material.clone(),
            properties,
        }
    }

    /// Test ray intersection with triangle
    pub fn ray_intersect(&self, ray: &Ray) -> Option<CollisionResult> {
        // Möller-Trumbore ray-triangle intersection algorithm
        let edge1 = self.vertices[1] - self.vertices[0];
        let edge2 = self.vertices[2] - self.vertices[0];

        let h = ray.direction.cross(edge2);
        let a = edge1.dot(h);

        if a > -f32::EPSILON && a < f32::EPSILON {
            return None; // Ray is parallel to triangle
        }

        let f = 1.0 / a;
        let s = ray.origin - self.vertices[0];
        let u = f * s.dot(h);

        if !(0.0..=1.0).contains(&u) {
            return None;
        }

        let q = s.cross(edge1);
        let v = f * ray.direction.dot(q);

        if v < 0.0 || u + v > 1.0 {
            return None;
        }

        let t = f * edge2.dot(q);

        if t > f32::EPSILON && t <= ray.max_distance {
            let position = ray.point_at_distance(t);
            Some(CollisionResult {
                hit: true,
                position,
                normal: self.normal,
                distance: t,
                material: self.material.clone(),
                surface_properties: self.properties.clone(),
            })
        } else {
            None
        }
    }

    /// Get height at world position (assuming triangle is roughly horizontal)
    pub fn get_height_at_position(&self, position: Point2<f32>) -> Option<f32> {
        // Use barycentric coordinates to interpolate height
        let v0 = Point2::new(self.vertices[0].x, self.vertices[0].z);
        let v1 = Point2::new(self.vertices[1].x, self.vertices[1].z);
        let v2 = Point2::new(self.vertices[2].x, self.vertices[2].z);

        let denom = (v1.y - v2.y) * (v0.x - v2.x) + (v2.x - v1.x) * (v0.y - v2.y);
        if denom.abs() < f32::EPSILON {
            return None; // Degenerate triangle
        }

        let a = ((v1.y - v2.y) * (position.x - v2.x) + (v2.x - v1.x) * (position.y - v2.y)) / denom;
        let b = ((v2.y - v0.y) * (position.x - v2.x) + (v0.x - v2.x) * (position.y - v2.y)) / denom;
        let c = 1.0 - a - b;

        if a >= 0.0 && b >= 0.0 && c >= 0.0 {
            Some(a * self.vertices[0].y + b * self.vertices[1].y + c * self.vertices[2].y)
        } else {
            None
        }
    }
}

impl CollisionGrid {
    /// Create new collision grid
    pub fn new(cell_size: f32, origin: Point2<f32>, size: (i32, i32)) -> Self {
        Self {
            cell_size,
            grid_size: size,
            origin,
            cells: HashMap::new(),
        }
    }

    /// Convert world position to grid coordinates
    pub fn world_to_grid(&self, position: Point2<f32>) -> (i32, i32) {
        let local_pos = position - self.origin;
        let grid_x = (local_pos.x / self.cell_size).floor() as i32;
        let grid_y = (local_pos.y / self.cell_size).floor() as i32;
        (grid_x, grid_y)
    }

    /// Get cell at grid coordinates
    pub fn get_cell(&self, grid_pos: (i32, i32)) -> Option<&CollisionCell> {
        self.cells.get(&grid_pos)
    }

    /// Get mutable cell at grid coordinates
    pub fn get_cell_mut(&mut self, grid_pos: (i32, i32)) -> Option<&mut CollisionCell> {
        self.cells.get_mut(&grid_pos)
    }

    /// Add collision triangle to appropriate grid cells
    pub fn add_triangle(&mut self, triangle: CollisionTriangle) {
        // Find all grid cells that intersect with triangle bounds
        let min_x = triangle
            .vertices
            .iter()
            .map(|v| v.x)
            .fold(f32::INFINITY, f32::min);
        let max_x = triangle
            .vertices
            .iter()
            .map(|v| v.x)
            .fold(f32::NEG_INFINITY, f32::max);
        let min_z = triangle
            .vertices
            .iter()
            .map(|v| v.z)
            .fold(f32::INFINITY, f32::min);
        let max_z = triangle
            .vertices
            .iter()
            .map(|v| v.z)
            .fold(f32::NEG_INFINITY, f32::max);

        let min_grid = self.world_to_grid(Point2::new(min_x, min_z));
        let max_grid = self.world_to_grid(Point2::new(max_x, max_z));

        for grid_y in min_grid.1..=max_grid.1 {
            for grid_x in min_grid.0..=max_grid.0 {
                let cell = self
                    .cells
                    .entry((grid_x, grid_y))
                    .or_insert_with(|| CollisionCell {
                        triangles: Vec::new(),
                        height_samples: Vec::new(),
                        materials: Vec::new(),
                        properties: Vec::new(),
                    });

                cell.triangles.push(triangle.clone());
            }
        }
    }

    /// Query height at world position
    pub fn query_height(&self, position: Point2<f32>) -> Option<f32> {
        let grid_pos = self.world_to_grid(position);

        if let Some(cell) = self.get_cell(grid_pos) {
            // Check triangles for precise height
            for triangle in &cell.triangles {
                if let Some(height) = triangle.get_height_at_position(position) {
                    return Some(height);
                }
            }

            // Fall back to height samples if available
            if !cell.height_samples.is_empty() {
                let local_x = (position.x - (self.origin.x + grid_pos.0 as f32 * self.cell_size))
                    / self.cell_size;
                let local_y = (position.y - (self.origin.y + grid_pos.1 as f32 * self.cell_size))
                    / self.cell_size;
                return cell.sample_height_bilinear(local_x, local_y);
            }
        }

        None
    }

    /// Query material at world position
    pub fn query_material(&self, position: Point2<f32>) -> Option<TerrainMaterial> {
        let grid_pos = self.world_to_grid(position);

        if let Some(cell) = self.get_cell(grid_pos) {
            if !cell.triangles.is_empty() {
                return Some(cell.triangles[0].material.clone());
            }
        }

        None
    }
}

impl TerrainCollision {
    /// Create new terrain collision system
    pub fn new(config: CollisionConfig) -> Self {
        let grid = CollisionGrid::new(
            config.grid_cell_size,
            Point2::new(0.0, 0.0),
            (100, 100), // Default grid size
        );

        Self {
            grid,
            config,
            stats: CollisionStats::default(),
        }
    }

    /// Add terrain triangle to collision system
    pub fn add_triangle(&mut self, triangle: CollisionTriangle) {
        self.grid.add_triangle(triangle);
    }

    /// Perform ray-terrain intersection test
    pub fn ray_cast(&mut self, ray: Ray) -> CollisionResult {
        self.stats.ray_tests += 1;
        let start_time = std::time::Instant::now();

        let mut closest_result = CollisionResult {
            hit: false,
            position: Vec3::ZERO,
            normal: Vec3::new(0.0, 1.0, 0.0),
            distance: f32::INFINITY,
            material: TerrainMaterial::default(),
            surface_properties: SurfaceProperties::default(),
        };

        // Sample points along ray and test grid cells
        let step_size = self.config.grid_cell_size / 2.0;
        let num_steps = (ray.max_distance / step_size).ceil() as u32;

        for i in 0..num_steps {
            let distance = i as f32 * step_size;
            if distance > ray.max_distance {
                break;
            }

            let sample_point = ray.point_at_distance(distance);
            let grid_pos = self
                .grid
                .world_to_grid(Point2::new(sample_point.x, sample_point.z));

            if let Some(cell) = self.grid.get_cell(grid_pos) {
                for triangle in &cell.triangles {
                    if let Some(result) = triangle.ray_intersect(&ray) {
                        if result.distance < closest_result.distance {
                            closest_result = result;
                            closest_result.hit = true;
                        }
                    }
                    self.stats.triangle_tests += 1;
                }
            }
        }

        self.stats.avg_query_time = start_time.elapsed();
        closest_result
    }

    /// Get terrain height at world position
    pub fn get_height_at_position(&mut self, position: Point2<f32>) -> f32 {
        self.stats.height_queries += 1;

        self.grid.query_height(position).unwrap_or(0.0)
    }

    /// Get terrain material at world position
    pub fn get_material_at_position(&self, position: Point2<f32>) -> TerrainMaterial {
        self.grid
            .query_material(position)
            .unwrap_or_default()
    }

    /// Get surface properties at world position
    pub fn get_surface_properties(&self, position: Point2<f32>) -> SurfaceProperties {
        let material = self.get_material_at_position(position);
        let mut properties = SurfaceProperties::from_material(&material);

        // Calculate slope from surrounding heights
        let offset = 0.5;
        let h_center = self.grid.query_height(position).unwrap_or(0.0);
        let h_right = self
            .grid
            .query_height(Point2::new(position.x + offset, position.y))
            .unwrap_or(h_center);
        let h_up = self
            .grid
            .query_height(Point2::new(position.x, position.y + offset))
            .unwrap_or(h_center);

        let dx = h_right - h_center;
        let dy = h_up - h_center;
        properties.slope = (dx * dx + dy * dy).sqrt().atan();

        properties
    }

    /// Test sphere collision with terrain
    pub fn sphere_test(&mut self, sphere: CollisionSphere) -> Vec<CollisionResult> {
        self.stats.sphere_tests += 1;

        let mut results = Vec::new();

        // Sample grid cells around sphere
        let grid_radius = (sphere.radius / self.config.grid_cell_size).ceil() as i32;
        let center_grid = self
            .grid
            .world_to_grid(Point2::new(sphere.center.x, sphere.center.z));

        for grid_y in (center_grid.1 - grid_radius)..=(center_grid.1 + grid_radius) {
            for grid_x in (center_grid.0 - grid_radius)..=(center_grid.0 + grid_radius) {
                if let Some(cell) = self.grid.get_cell((grid_x, grid_y)) {
                    for triangle in &cell.triangles {
                        if let Some(result) = self.sphere_triangle_test(&sphere, triangle) {
                            results.push(result);
                        }
                    }
                }
            }
        }

        results
    }

    /// Test sphere-triangle intersection
    fn sphere_triangle_test(
        &self,
        sphere: &CollisionSphere,
        triangle: &CollisionTriangle,
    ) -> Option<CollisionResult> {
        // Find closest point on triangle to sphere center
        let closest_point = self.closest_point_on_triangle(sphere.center, triangle);
        let distance_to_closest = (closest_point - sphere.center).length();

        if distance_to_closest <= sphere.radius {
            let normal = if distance_to_closest > 0.0 {
                (sphere.center - closest_point).normalize()
            } else {
                triangle.normal
            };

            Some(CollisionResult {
                hit: true,
                position: closest_point,
                normal,
                distance: distance_to_closest,
                material: triangle.material.clone(),
                surface_properties: triangle.properties.clone(),
            })
        } else {
            None
        }
    }

    /// Find closest point on triangle to given point
    fn closest_point_on_triangle(&self, point: Vec3, triangle: &CollisionTriangle) -> Vec3 {
        let v0 = triangle.vertices[0];
        let v1 = triangle.vertices[1];
        let v2 = triangle.vertices[2];

        // Check if projection lies inside triangle using barycentric coordinates
        let edge0 = v1 - v0;
        let edge1 = v2 - v0;
        let to_point = point - v0;

        let a = edge0.dot(edge0);
        let b = edge0.dot(edge1);
        let c = edge1.dot(edge1);
        let d = edge0.dot(to_point);
        let e = edge1.dot(to_point);

        let det = a * c - b * b;
        let s = b * e - c * d;
        let t = b * d - a * e;

        if s + t <= det {
            if s < 0.0 {
                if t < 0.0 {
                    // Region 4
                    if d < 0.0 {
                        v0 + (-d / a).clamp(0.0, 1.0) * edge0
                    } else {
                        v0 + (-e / c).clamp(0.0, 1.0) * edge1
                    }
                } else {
                    // Region 3
                    v0 + (-e / c).clamp(0.0, 1.0) * edge1
                }
            } else if t < 0.0 {
                // Region 5
                v0 + (-d / a).clamp(0.0, 1.0) * edge0
            } else {
                // Region 0 - inside triangle
                let inv_det = 1.0 / det;
                let u = s * inv_det;
                let v = t * inv_det;
                v0 + u * edge0 + v * edge1
            }
        } else {
            // Region 2, 1, or 6
            if s < 0.0 {
                // Region 2
                let tmp0 = b + d;
                let tmp1 = c + e;
                if tmp1 > tmp0 {
                    let numer = tmp1 - tmp0;
                    let denom = a - 2.0 * b + c;
                    v0 + (numer / denom).clamp(0.0, 1.0) * (edge0 - edge1) + edge1
                } else {
                    v0 + (-e / c).clamp(0.0, 1.0) * edge1
                }
            } else if t < 0.0 {
                // Region 6
                let tmp0 = b + e;
                let tmp1 = a + d;
                if tmp1 > tmp0 {
                    let numer = tmp1 - tmp0;
                    let denom = a - 2.0 * b + c;
                    v0 + (numer / denom).clamp(0.0, 1.0) * (edge1 - edge0) + edge0
                } else {
                    v0 + (-d / a).clamp(0.0, 1.0) * edge0
                }
            } else {
                // Region 1
                let numer = (c + e) - (b + d);
                let denom = a - 2.0 * b + c;
                v0 + (numer / denom).clamp(0.0, 1.0) * (edge0 - edge1) + edge1
            }
        }
    }

    /// Check if position is traversable
    pub fn is_traversable(&self, position: Point2<f32>) -> bool {
        let properties = self.get_surface_properties(position);
        properties.is_traversable(self.config.max_traversable_slope)
    }

    /// Get performance statistics
    pub fn get_stats(&self) -> &CollisionStats {
        &self.stats
    }

    /// Clear all collision data
    pub fn clear(&mut self) {
        self.grid.cells.clear();
        self.stats = CollisionStats::default();
    }
}

impl Default for TerrainCollision {
    fn default() -> Self {
        Self::new(CollisionConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_material_properties() {
        let grass = TerrainMaterial::Grass {
            density: 0.8,
            friction: 0.6,
        };
        assert_eq!(grass.get_movement_modifier(), 1.0);
        assert_eq!(grass.get_friction(), 0.6);

        let water = TerrainMaterial::Water {
            depth: 3.0,
            flow_speed: 1.0,
        };
        assert_eq!(water.get_movement_modifier(), 0.0); // Too deep to traverse
    }

    #[test]
    fn test_surface_properties_from_material() {
        let sand = TerrainMaterial::Sand {
            grain_size: 0.5,
            stability: 0.7,
        };
        let properties = SurfaceProperties::from_material(&sand);

        assert_eq!(properties.movement_modifier, sand.get_movement_modifier());
        assert_eq!(properties.friction, sand.get_friction());
        assert!(properties.interaction_effects.len() > 0);
    }

    #[test]
    fn test_ray_creation() {
        let origin = Vec3::new(0.0, 10.0, 0.0);
        let direction = Vec3::new(0.0, -1.0, 0.0);
        let ray = Ray::new(origin, direction, 20.0);

        assert_eq!(ray.origin, origin);
        assert_eq!(ray.direction, direction);
        assert_eq!(ray.max_distance, 20.0);

        let point_at_5 = ray.point_at_distance(5.0);
        assert_eq!(point_at_5, Vec3::new(0.0, 5.0, 0.0));
    }

    #[test]
    fn test_collision_triangle() {
        let vertices = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(5.0, 0.0, 10.0),
        ];
        let material = TerrainMaterial::Grass {
            density: 0.8,
            friction: 0.6,
        };
        let triangle = CollisionTriangle::new(vertices, material);

        // Test height query inside triangle
        let height = triangle.get_height_at_position(Point2::new(5.0, 3.0));
        assert!(height.is_some());

        // Test height query outside triangle
        let height_outside = triangle.get_height_at_position(Point2::new(15.0, 3.0));
        assert!(height_outside.is_none());
    }

    #[test]
    fn test_collision_grid() {
        let mut grid = CollisionGrid::new(10.0, Point2::new(0.0, 0.0), (10, 10));

        let triangle = CollisionTriangle::new(
            [
                Vec3::new(5.0, 0.0, 5.0),
                Vec3::new(15.0, 0.0, 5.0),
                Vec3::new(10.0, 0.0, 15.0),
            ],
            TerrainMaterial::default(),
        );

        grid.add_triangle(triangle);

        // Test height query
        let height = grid.query_height(Point2::new(10.0, 8.0));
        assert!(height.is_some());

        // Test material query
        let material = grid.query_material(Point2::new(10.0, 8.0));
        assert!(material.is_some());
    }

    #[test]
    fn test_collision_grid_height_sample_bilinear_fallback() {
        let mut grid = CollisionGrid::new(10.0, Point2::new(0.0, 0.0), (10, 10));
        grid.cells.insert(
            (0, 0),
            CollisionCell {
                triangles: Vec::new(),
                height_samples: vec![0.0, 10.0, 20.0, 30.0],
                materials: Vec::new(),
                properties: Vec::new(),
            },
        );

        let height = grid.query_height(Point2::new(5.0, 5.0));
        assert_eq!(height, Some(15.0));
    }

    #[test]
    fn test_terrain_collision_system() {
        let config = CollisionConfig::default();
        let mut collision_system = TerrainCollision::new(config);

        let triangle = CollisionTriangle::new(
            [
                Vec3::new(-5.0, 0.0, -5.0),
                Vec3::new(5.0, 0.0, -5.0),
                Vec3::new(0.0, 0.0, 5.0),
            ],
            TerrainMaterial::Rock {
                hardness: 0.8,
                roughness: 0.3,
            },
        );

        collision_system.add_triangle(triangle);

        // Test height query
        let height = collision_system.get_height_at_position(Point2::new(0.0, 0.0));
        assert_eq!(height, 0.0);

        // Test traversability
        let traversable = collision_system.is_traversable(Point2::new(0.0, 0.0));
        assert!(traversable);

        // Test ray cast
        let ray = Ray::new(Vec3::new(0.0, 10.0, 0.0), Vec3::new(0.0, -1.0, 0.0), 20.0);
        let result = collision_system.ray_cast(ray);
        assert!(result.hit);
        assert_eq!(collision_system.get_stats().ray_tests, 1);
    }
}
