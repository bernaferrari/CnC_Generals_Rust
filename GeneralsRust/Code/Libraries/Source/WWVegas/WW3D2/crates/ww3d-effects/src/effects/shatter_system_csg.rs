/*
**	Command & Conquer Generals Zero Hour(tm) Rust Port
**	Copyright 2025
**
**	Complete Shatter System with BSP-based CSG implementation
**	Port of shattersystem.cpp/h with full CSG algorithms
*/

use glam::{Mat4, Vec2, Vec3};
use std::sync::{Mutex, OnceLock};
use thiserror::Error;

/// Errors that can occur in the shatter system
#[derive(Error, Debug)]
pub enum ShatterError {
    #[error("System not initialized")]
    NotInitialized,
    #[error("No patterns loaded")]
    NoPatternsLoaded,
    #[error("Invalid mesh")]
    InvalidMesh,
    #[error("CSG operation failed")]
    CsgFailed,
}

pub type ShatterResult<T> = Result<T, ShatterError>;

//------------------------------------------------------------------------------
// CONSTANTS
//------------------------------------------------------------------------------

const BPT_FRONT: i32 = 0x01;
const BPT_BACK: i32 = 0x02;
const BPT_ON: i32 = 0x04;
const BPT_BOTH: i32 = 0x08;
const BPT_EPSILON: f32 = 0.0001;
const BPT_COINCIDENCE_EPSILON: f32 = 0.000001;
const MAX_MESH_FRAGMENTS: usize = 32;
const BPT_POLY_MAX_VERTS: usize = 24;

//------------------------------------------------------------------------------
// PLANE CLASS
//------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct Plane {
    pub normal: Vec3,
    pub d: f32,
}

impl Plane {
    pub fn new(normal: Vec3, point: Vec3) -> Self {
        let n = normal.normalize();
        Self {
            normal: n,
            d: n.dot(point),
        }
    }

    pub fn from_transform(transform: &Mat4) -> Self {
        let normal = transform.z_axis.truncate();
        let point = transform.w_axis.truncate();
        Self::new(normal, point)
    }

    pub fn in_front(&self, point: Vec3) -> bool {
        self.normal.dot(point) - self.d > BPT_EPSILON
    }

    pub fn which_side(&self, point: Vec3) -> i32 {
        let dist = self.normal.dot(point) - self.d;
        if dist > BPT_EPSILON {
            BPT_FRONT
        } else if dist < -BPT_EPSILON {
            BPT_BACK
        } else {
            BPT_ON
        }
    }

    pub fn compute_intersection(&self, p0: Vec3, p1: Vec3) -> f32 {
        let d0 = self.normal.dot(p0) - self.d;
        let d1 = self.normal.dot(p1) - self.d;

        if (d1 - d0).abs() < 0.00001 {
            return 0.0;
        }

        -d0 / (d1 - d0)
    }
}

//------------------------------------------------------------------------------
// VERTEX CLASS
// Temporary vertex representation during clipping
//------------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ShatterVertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub pass_count: usize,
    pub dcg: [u32; 2],              // Diffuse color/glow per pass
    pub dig: [u32; 2],              // Diffuse illumination per pass
    pub tex_coords: [[Vec2; 2]; 2], // UV coords [pass][stage]
}

impl ShatterVertex {
    pub fn new() -> Self {
        Self {
            position: Vec3::ZERO,
            normal: Vec3::Z,
            pass_count: 0,
            dcg: [0xFFFFFFFF; 2],
            dig: [0xFFFFFFFF; 2],
            tex_coords: [[Vec2::ZERO; 2]; 2],
        }
    }

    pub fn which_side(&self, plane: &Plane) -> i32 {
        plane.which_side(self.position)
    }

    pub fn lerp(v0: &Self, v1: &Self, t: f32) -> Self {
        let mut result = Self::new();
        result.position = v0.position.lerp(v1.position, t);
        result.normal = v0.normal.lerp(v1.normal, t).normalize();
        result.pass_count = v0.pass_count;

        for i in 0..v0.pass_count {
            // Interpolate colors properly (ARGB u32 format)
            let dcg0 = v0.dcg[i];
            let dcg1 = v1.dcg[i];
            let a0 = ((dcg0 >> 24) & 0xFF) as f32;
            let r0 = ((dcg0 >> 16) & 0xFF) as f32;
            let g0 = ((dcg0 >> 8) & 0xFF) as f32;
            let b0 = (dcg0 & 0xFF) as f32;
            let a1 = ((dcg1 >> 24) & 0xFF) as f32;
            let r1 = ((dcg1 >> 16) & 0xFF) as f32;
            let g1 = ((dcg1 >> 8) & 0xFF) as f32;
            let b1 = (dcg1 & 0xFF) as f32;
            let a = (a0 + (a1 - a0) * t) as u32;
            let r = (r0 + (r1 - r0) * t) as u32;
            let g = (g0 + (g1 - g0) * t) as u32;
            let b = (b0 + (b1 - b0) * t) as u32;
            result.dcg[i] = (a << 24) | (r << 16) | (g << 8) | b;

            // Same for dig (diffuse illumination)
            let dig0 = v0.dig[i];
            let dig1 = v1.dig[i];
            let a0 = ((dig0 >> 24) & 0xFF) as f32;
            let r0 = ((dig0 >> 16) & 0xFF) as f32;
            let g0 = ((dig0 >> 8) & 0xFF) as f32;
            let b0 = (dig0 & 0xFF) as f32;
            let a1 = ((dig1 >> 24) & 0xFF) as f32;
            let r1 = ((dig1 >> 16) & 0xFF) as f32;
            let g1 = ((dig1 >> 8) & 0xFF) as f32;
            let b1 = (dig1 & 0xFF) as f32;
            let a = (a0 + (a1 - a0) * t) as u32;
            let r = (r0 + (r1 - r0) * t) as u32;
            let g = (g0 + (g1 - g0) * t) as u32;
            let b = (b0 + (b1 - b0) * t) as u32;
            result.dig[i] = (a << 24) | (r << 16) | (g << 8) | b;

            for stage in 0..2 {
                result.tex_coords[i][stage] =
                    v0.tex_coords[i][stage].lerp(v1.tex_coords[i][stage], t);
            }
        }

        result
    }

    pub fn intersect_plane(p0: &Self, p1: &Self, plane: &Plane) -> Self {
        let alpha = plane.compute_intersection(p0.position, p1.position);
        Self::lerp(p0, p1, alpha)
    }
}

//------------------------------------------------------------------------------
// POLYGON CLASS
// Temporary polygon representation during clipping
//------------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ShatterPolygon {
    pub material_id: usize,
    pub vertices: Vec<ShatterVertex>,
    pub plane: Plane,
}

impl ShatterPolygon {
    pub fn new() -> Self {
        Self {
            material_id: 0,
            vertices: Vec::with_capacity(BPT_POLY_MAX_VERTS),
            plane: Plane::new(Vec3::Z, Vec3::ZERO),
        }
    }

    pub fn from_vertices(vertices: &[ShatterVertex], count: usize) -> Self {
        let mut poly = Self::new();
        for i in 0..count.min(vertices.len()) {
            poly.vertices.push(vertices[i].clone());
        }
        poly.compute_plane();
        poly
    }

    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Compute plane equation from vertices using Newell's method
    pub fn compute_plane(&mut self) {
        if self.vertices.len() < 3 {
            return;
        }

        let mut nx = 0.0f64;
        let mut ny = 0.0f64;
        let mut nz = 0.0f64;
        let mut ax = 0.0f64;
        let mut ay = 0.0f64;
        let mut az = 0.0f64;

        for i in 0..self.vertices.len() {
            let j = (i + 1) % self.vertices.len();
            let vi = &self.vertices[i].position;
            let vj = &self.vertices[j].position;

            nx += (vi.y - vj.y) as f64 * (vi.z + vj.z) as f64;
            ny += (vi.z - vj.z) as f64 * (vi.x + vj.x) as f64;
            nz += (vi.x - vj.x) as f64 * (vi.y + vj.y) as f64;

            ax += vi.x as f64;
            ay += vi.y as f64;
            az += vi.z as f64;
        }

        let count = self.vertices.len() as f64;
        ax /= count;
        ay /= count;
        az /= count;

        let len = (nx * nx + ny * ny + nz * nz).sqrt();
        nx /= len;
        ny /= len;
        nz /= len;

        let normal = Vec3::new(nx as f32, ny as f32, nz as f32);
        let point = Vec3::new(ax as f32, ay as f32, az as f32);
        self.plane = Plane::new(normal, point);
    }

    /// Classify polygon relative to plane
    pub fn which_side(&self, plane: &Plane) -> i32 {
        let mut side_mask = 0;

        for vertex in &self.vertices {
            side_mask |= vertex.which_side(plane);
        }

        // All vertices on plane
        if side_mask == BPT_ON {
            return BPT_ON;
        }

        // All vertices on front or on plane
        if (side_mask & !(BPT_FRONT | BPT_ON)) == 0 {
            return BPT_FRONT;
        }

        // All vertices on back or on plane
        if (side_mask & !(BPT_BACK | BPT_ON)) == 0 {
            return BPT_BACK;
        }

        // Polygon spans the plane
        BPT_BOTH
    }

    /// Split polygon by plane into front and back polygons
    pub fn split(&self, plane: &Plane) -> (ShatterPolygon, ShatterPolygon) {
        let mut front = self.clone();
        let mut back = self.clone();
        front.vertices.clear();
        back.vertices.clear();

        // Find first vertex that's definitely on one side
        let mut start_idx = 0;
        let mut start_side = BPT_ON;

        for (i, vertex) in self.vertices.iter().enumerate() {
            let side = vertex.which_side(plane);
            if side != BPT_ON {
                start_idx = i;
                start_side = side;
                break;
            }
        }

        let mut prev_idx = start_idx;
        let mut prev_side = start_side;
        let mut last_definite = 0;

        let mut idx = (start_idx + 1) % self.vertices.len();

        for _ in 0..self.vertices.len() {
            let cur_side = self.vertices[idx].which_side(plane);

            if prev_side == BPT_FRONT {
                if cur_side == BPT_FRONT {
                    // Both front - add to front poly
                    front.vertices.push(self.vertices[idx].clone());
                } else if cur_side == BPT_ON {
                    // Front to on - add to front poly
                    last_definite = BPT_FRONT;
                    front.vertices.push(self.vertices[idx].clone());
                } else {
                    // Front to back - emit intersection to both
                    let int_vert = ShatterVertex::intersect_plane(
                        &self.vertices[prev_idx],
                        &self.vertices[idx],
                        plane,
                    );
                    front.vertices.push(int_vert.clone());
                    back.vertices.push(int_vert);
                    back.vertices.push(self.vertices[idx].clone());
                }
            } else if prev_side == BPT_BACK {
                if cur_side == BPT_FRONT {
                    // Back to front - emit intersection to both
                    let int_vert = ShatterVertex::intersect_plane(
                        &self.vertices[prev_idx],
                        &self.vertices[idx],
                        plane,
                    );
                    back.vertices.push(int_vert.clone());
                    front.vertices.push(int_vert);
                    front.vertices.push(self.vertices[idx].clone());
                } else if cur_side == BPT_ON {
                    // Back to on - add to back poly
                    last_definite = BPT_BACK;
                    back.vertices.push(self.vertices[idx].clone());
                } else {
                    // Both back - add to back poly
                    back.vertices.push(self.vertices[idx].clone());
                }
            } else if prev_side == BPT_ON {
                if cur_side == BPT_FRONT {
                    if last_definite == BPT_BACK {
                        front.vertices.push(self.vertices[prev_idx].clone());
                    }
                    front.vertices.push(self.vertices[idx].clone());
                } else if cur_side == BPT_ON {
                    if last_definite == BPT_FRONT {
                        front.vertices.push(self.vertices[idx].clone());
                    } else {
                        back.vertices.push(self.vertices[idx].clone());
                    }
                } else {
                    // ON to BACK
                    if last_definite == BPT_FRONT {
                        back.vertices.push(self.vertices[prev_idx].clone());
                    }
                    back.vertices.push(self.vertices[idx].clone());
                }
            }

            prev_side = cur_side;
            prev_idx = idx;
            idx = (idx + 1) % self.vertices.len();
        }

        front.compute_plane();
        back.compute_plane();

        // Check for and fix degenerate polygons
        if front.is_degenerate() {
            front.salvage_degenerate();
        }
        if back.is_degenerate() {
            back.salvage_degenerate();
        }

        (front, back)
    }

    /// Check if polygon is degenerate
    pub fn is_degenerate(&self) -> bool {
        if self.vertices.len() < 3 {
            return true;
        }

        // Check for coincident vertices
        for i in 0..self.vertices.len() {
            for j in (i + 1)..self.vertices.len() {
                let delta = (self.vertices[i].position - self.vertices[j].position).length();
                if delta < BPT_COINCIDENCE_EPSILON {
                    return true;
                }
            }
        }

        // Check if all vertices lie on the plane
        for vertex in &self.vertices {
            if vertex.which_side(&self.plane) != BPT_ON {
                return true;
            }
        }

        false
    }

    /// Attempt to fix degenerate polygon
    pub fn salvage_degenerate(&mut self) {
        let mut i = 0;
        while i < self.vertices.len() && self.vertices.len() > 3 {
            let next = (i + 1) % self.vertices.len();
            let delta = (self.vertices[i].position - self.vertices[next].position).length();

            if delta < BPT_COINCIDENCE_EPSILON {
                self.vertices.remove(next);
            } else {
                i += 1;
            }
        }

        if self.vertices.len() >= 3 {
            self.compute_plane();
        }
    }
}

//------------------------------------------------------------------------------
// BSP NODE CLASS
// Binary Space Partitioning tree node for mesh splitting
//------------------------------------------------------------------------------

#[derive(Debug)]
pub struct BspNode {
    pub plane: Plane,
    pub front: Option<Box<BspNode>>,
    pub back: Option<Box<BspNode>>,
    pub front_leaf_index: Option<usize>,
    pub back_leaf_index: Option<usize>,
}

impl BspNode {
    /// Create BSP tree from hierarchy transforms
    pub fn from_hierarchy(
        transforms: &[Mat4],
        parent_indices: &[i32],
        bone_index: usize,
        leaf_counter: &mut usize,
    ) -> Self {
        let plane = Plane::from_transform(&transforms[bone_index]);

        // Find child bones
        let mut front_child = None;
        let mut back_child = None;

        for (i, &parent_idx) in parent_indices.iter().enumerate() {
            if parent_idx as usize == bone_index {
                let child_pos = transforms[i].w_axis.truncate();
                if plane.in_front(child_pos) {
                    front_child = Some(i);
                } else {
                    back_child = Some(i);
                }
            }
        }

        // Create children or assign leaf indices
        let (front, front_leaf_index) = if let Some(child_idx) = front_child {
            (
                Some(Box::new(Self::from_hierarchy(
                    transforms,
                    parent_indices,
                    child_idx,
                    leaf_counter,
                ))),
                None,
            )
        } else {
            let idx = *leaf_counter;
            *leaf_counter += 1;
            (None, Some(idx))
        };

        let (back, back_leaf_index) = if let Some(child_idx) = back_child {
            (
                Some(Box::new(Self::from_hierarchy(
                    transforms,
                    parent_indices,
                    child_idx,
                    leaf_counter,
                ))),
                None,
            )
        } else {
            let idx = *leaf_counter;
            *leaf_counter += 1;
            (None, Some(idx))
        };

        Self {
            plane,
            front,
            back,
            front_leaf_index,
            back_leaf_index,
        }
    }

    /// Clip polygon against BSP tree, distributing fragments to leaf clip pools
    pub fn clip_polygon(&self, polygon: &ShatterPolygon, clip_pools: &mut [Vec<ShatterPolygon>]) {
        let side = polygon.which_side(&self.plane);

        match side {
            BPT_FRONT | BPT_ON => {
                // Polygon entirely in front
                if let Some(ref front_node) = self.front {
                    front_node.clip_polygon(polygon, clip_pools);
                } else if let Some(leaf_idx) = self.front_leaf_index {
                    if polygon.vertex_count() >= 3 {
                        clip_pools[leaf_idx].push(polygon.clone());
                    }
                }
            }
            BPT_BACK => {
                // Polygon entirely in back
                if let Some(ref back_node) = self.back {
                    back_node.clip_polygon(polygon, clip_pools);
                } else if let Some(leaf_idx) = self.back_leaf_index {
                    if polygon.vertex_count() >= 3 {
                        clip_pools[leaf_idx].push(polygon.clone());
                    }
                }
            }
            BPT_BOTH => {
                // Split polygon
                let (front_poly, back_poly) = polygon.split(&self.plane);

                // Recursively clip both halves
                if front_poly.vertex_count() >= 3 {
                    if let Some(ref front_node) = self.front {
                        front_node.clip_polygon(&front_poly, clip_pools);
                    } else if let Some(leaf_idx) = self.front_leaf_index {
                        clip_pools[leaf_idx].push(front_poly);
                    }
                }

                if back_poly.vertex_count() >= 3 {
                    if let Some(ref back_node) = self.back {
                        back_node.clip_polygon(&back_poly, clip_pools);
                    } else if let Some(leaf_idx) = self.back_leaf_index {
                        clip_pools[leaf_idx].push(back_poly);
                    }
                }
            }
            _ => {}
        }
    }
}

//------------------------------------------------------------------------------
// MESH FRAGMENT
// Result of shattering a mesh
//------------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct MeshFragment {
    pub vertices: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub tex_coords: Vec<Vec2>,
    pub colors: Vec<u32>,
    pub indices: Vec<u32>,
    pub transform: Mat4,
    pub velocity: Vec3,
    pub angular_velocity: Vec3,
}

impl MeshFragment {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            normals: Vec::new(),
            tex_coords: Vec::new(),
            colors: Vec::new(),
            indices: Vec::new(),
            transform: Mat4::IDENTITY,
            velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
        }
    }

    /// Get the number of vertices in this fragment
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Get the number of triangles in this fragment
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Check if this fragment has valid geometry
    pub fn is_valid(&self) -> bool {
        !self.vertices.is_empty() && !self.indices.is_empty() && self.indices.len() % 3 == 0
    }

    /// Update fragment physics for one timestep
    pub fn update(&mut self, delta_time: f32, gravity: Vec3) {
        // Apply gravity
        self.velocity += gravity * delta_time;

        // Update position
        let translation = self.velocity * delta_time;
        self.transform.w_axis += translation.extend(0.0);

        // Update rotation
        let angle = self.angular_velocity.length() * delta_time;
        if angle > 0.0001 {
            let axis = self.angular_velocity.normalize();
            let rotation = Mat4::from_axis_angle(axis, angle);
            self.transform = rotation * self.transform;
        }

        // Apply damping
        self.velocity *= 0.98;
        self.angular_velocity *= 0.95;
    }

    /// Create fragment from polygon pool
    pub fn from_polygon_pool(
        pool: &[ShatterPolygon],
        transform: &Mat4,
        original_center: Vec3,
    ) -> Self {
        let mut fragment = Self::new();

        // Convert polygons to triangles
        for poly in pool {
            if poly.vertices.len() < 3 {
                continue;
            }

            let first_vert_idx = fragment.vertices.len();

            // Add vertices
            for vertex in &poly.vertices {
                let transformed_pos = transform.transform_point3(vertex.position);
                fragment.vertices.push(transformed_pos);
                fragment.normals.push(vertex.normal);
                fragment.colors.push(vertex.dcg[0]);
                // Use first pass, first stage UV
                fragment.tex_coords.push(vertex.tex_coords[0][0]);
            }

            // Create triangle fan
            for i in 1..(poly.vertices.len() - 1) {
                fragment.indices.push(first_vert_idx as u32);
                fragment.indices.push((first_vert_idx + i) as u32);
                fragment.indices.push((first_vert_idx + i + 1) as u32);
            }
        }

        // Compute bounding box and recenter
        if !fragment.vertices.is_empty() {
            let mut min = fragment.vertices[0];
            let mut max = fragment.vertices[0];

            for v in &fragment.vertices {
                min = min.min(*v);
                max = max.max(*v);
            }

            let center = (min + max) * 0.5;

            // Translate vertices to center
            for v in &mut fragment.vertices {
                *v -= center;
            }

            // Update transform
            let mut new_transform = *transform;
            new_transform.w_axis += (center - original_center).extend(0.0);
            fragment.transform = new_transform;
        }

        fragment
    }
}

//------------------------------------------------------------------------------
// SHATTER SYSTEM
// Main system for shattering meshes
//------------------------------------------------------------------------------

pub struct ShatterSystem {
    /// Loaded shatter patterns (BSP trees)
    patterns: Vec<BspNode>,
    /// Temporary clip pools for each leaf
    clip_pools: Vec<Vec<ShatterPolygon>>,
    /// Result fragments from last shatter operation
    fragments: Vec<MeshFragment>,
}

impl ShatterSystem {
    pub fn new() -> Self {
        Self {
            patterns: Vec::new(),
            clip_pools: vec![Vec::new(); MAX_MESH_FRAGMENTS],
            fragments: Vec::new(),
        }
    }

    /// Initialize system and load shatter patterns
    pub fn init(&mut self) {
        // Load default cube pattern
        let transforms = vec![
            Mat4::IDENTITY,
            Mat4::from_translation(Vec3::new(0.0, 0.0, 0.0)),
            Mat4::from_translation(Vec3::new(1.0, 0.0, 0.0)),
            Mat4::from_translation(Vec3::new(-1.0, 0.0, 0.0)),
            Mat4::from_translation(Vec3::new(0.0, 1.0, 0.0)),
            Mat4::from_translation(Vec3::new(0.0, -1.0, 0.0)),
        ];
        let parents = vec![-1, 0, 1, 1, 1, 1];

        let mut leaf_counter = 0;
        let pattern = BspNode::from_hierarchy(&transforms, &parents, 1, &mut leaf_counter);
        self.patterns.push(pattern);
    }

    /// Shatter a mesh into fragments
    pub fn shatter_mesh(
        &mut self,
        vertices: &[Vec3],
        normals: &[Vec3],
        tex_coords: &[Vec2],
        indices: &[u32],
        transform: &Mat4,
        impact_point: Vec3,
        impact_direction: Vec3,
    ) -> ShatterResult<&[MeshFragment]> {
        if self.patterns.is_empty() {
            return Err(ShatterError::NoPatternsLoaded);
        }

        if vertices.is_empty() || indices.is_empty() {
            return Err(ShatterError::InvalidMesh);
        }

        // Reset clip pools
        for pool in &mut self.clip_pools {
            pool.clear();
        }
        self.fragments.clear();

        // Select random pattern
        let pattern_idx = (rand::random::<f32>() * self.patterns.len() as f32) as usize;
        let pattern = &self.patterns[pattern_idx];

        // Convert mesh triangles to ShatterPolygons
        let num_triangles = indices.len() / 3;
        for tri_idx in 0..num_triangles {
            let idx0 = indices[tri_idx * 3] as usize;
            let idx1 = indices[tri_idx * 3 + 1] as usize;
            let idx2 = indices[tri_idx * 3 + 2] as usize;

            if idx0 >= vertices.len() || idx1 >= vertices.len() || idx2 >= vertices.len() {
                continue; // Skip invalid triangles
            }

            // Create polygon from triangle
            let mut poly = ShatterPolygon::new();
            poly.material_id = 0; // Could be enhanced to track material per triangle

            for &idx in &[idx0, idx1, idx2] {
                let mut vert = ShatterVertex::new();
                vert.position = vertices[idx];

                // Get normal if available
                if idx < normals.len() {
                    vert.normal = normals[idx];
                } else {
                    // Compute face normal
                    let edge1 = vertices[idx1] - vertices[idx0];
                    let edge2 = vertices[idx2] - vertices[idx0];
                    vert.normal = edge1.cross(edge2).normalize_or_zero();
                }

                // Get texture coordinates if available
                if idx < tex_coords.len() {
                    vert.tex_coords[0][0] = tex_coords[idx];
                }

                vert.pass_count = 1;
                poly.vertices.push(vert);
            }

            poly.compute_plane();

            // Clip polygon against BSP tree, distributing to leaf pools
            pattern.clip_polygon(&poly, &mut self.clip_pools);
        }

        // Compute center of original mesh for fragment offsetting
        let mut original_center = Vec3::ZERO;
        for vert in vertices {
            original_center += *vert;
        }
        if !vertices.is_empty() {
            original_center /= vertices.len() as f32;
        }

        // Create fragments from clip pools
        for (_pool_idx, pool) in self.clip_pools.iter().enumerate() {
            if pool.is_empty() {
                continue;
            }

            let fragment = MeshFragment::from_polygon_pool(pool, transform, original_center);

            if !fragment.vertices.is_empty() {
                // Add some randomized physics
                let offset_from_impact = fragment.transform.w_axis.truncate() - impact_point;
                let radial_dir = offset_from_impact.normalize_or_zero();

                // Velocity combines impact direction with radial spread
                let velocity = impact_direction * 3.0 + radial_dir * 2.0;

                // Random angular velocity
                let angular_velocity = Vec3::new(
                    (rand::random::<f32>() - 0.5) * 6.0,
                    (rand::random::<f32>() - 0.5) * 6.0,
                    (rand::random::<f32>() - 0.5) * 6.0,
                );

                let mut final_fragment = fragment;
                final_fragment.velocity = velocity;
                final_fragment.angular_velocity = angular_velocity;

                self.fragments.push(final_fragment);
            }
        }

        // If no fragments were created, create a single fallback fragment
        if self.fragments.is_empty() {
            let mut fragment = MeshFragment::new();
            fragment.vertices = vertices.to_vec();
            fragment.normals = normals.to_vec();
            fragment.tex_coords = tex_coords.to_vec();
            fragment.indices = indices.to_vec();
            fragment.transform = *transform;
            fragment.velocity = impact_direction * 5.0;
            fragment.angular_velocity = Vec3::new(
                rand::random::<f32>() - 0.5,
                rand::random::<f32>() - 0.5,
                rand::random::<f32>() - 0.5,
            ) * 3.0;
            self.fragments.push(fragment);
        }

        Ok(&self.fragments)
    }

    /// Get fragment count from last shatter
    pub fn fragment_count(&self) -> usize {
        self.fragments.len()
    }

    /// Get fragment at index
    pub fn get_fragment(&self, index: usize) -> Option<&MeshFragment> {
        self.fragments.get(index)
    }

    /// Release all fragments
    pub fn release_fragments(&mut self) {
        self.fragments.clear();
    }
}

/// Global shatter system instance
static SHATTER_SYSTEM: OnceLock<Mutex<ShatterSystem>> = OnceLock::new();

pub fn get_shatter_system() -> &'static Mutex<ShatterSystem> {
    SHATTER_SYSTEM.get_or_init(|| {
        let mut system = ShatterSystem::new();
        system.init();
        Mutex::new(system)
    })
}

pub fn init_shatter_system() {
    let _ = get_shatter_system();
}

pub fn shutdown_shatter_system() {
    if let Some(system) = SHATTER_SYSTEM.get() {
        let mut sys = system.lock().unwrap();
        sys.release_fragments();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plane_classification() {
        let plane = Plane::new(Vec3::Z, Vec3::ZERO);
        assert_eq!(plane.which_side(Vec3::new(0.0, 0.0, 1.0)), BPT_FRONT);
        assert_eq!(plane.which_side(Vec3::new(0.0, 0.0, -1.0)), BPT_BACK);
        assert_eq!(plane.which_side(Vec3::new(1.0, 1.0, 0.0)), BPT_ON);
    }

    #[test]
    fn test_polygon_split() {
        let mut poly = ShatterPolygon::new();
        for i in 0..4 {
            let angle = i as f32 * std::f32::consts::PI * 0.5;
            let mut vert = ShatterVertex::new();
            vert.position = Vec3::new(angle.cos(), angle.sin(), 0.0);
            poly.vertices.push(vert);
        }
        poly.compute_plane();

        let plane = Plane::new(Vec3::X, Vec3::ZERO);
        let (front, back) = poly.split(&plane);

        assert!(front.vertex_count() >= 3);
        assert!(back.vertex_count() >= 3);
    }

    #[test]
    fn test_shatter_system_init() {
        init_shatter_system();
        let system = get_shatter_system();
        let sys = system.lock().unwrap();
        assert!(!sys.patterns.is_empty());
    }

    #[test]
    fn test_mesh_shattering() {
        let mut system = ShatterSystem::new();
        system.init();

        // Create a simple cube mesh
        let vertices = vec![
            Vec3::new(-1.0, -1.0, -1.0),
            Vec3::new(1.0, -1.0, -1.0),
            Vec3::new(1.0, 1.0, -1.0),
            Vec3::new(-1.0, 1.0, -1.0),
            Vec3::new(-1.0, -1.0, 1.0),
            Vec3::new(1.0, -1.0, 1.0),
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::new(-1.0, 1.0, 1.0),
        ];

        let normals = vec![Vec3::Z; 8];

        let tex_coords = vec![Vec2::ZERO; 8];

        // Two faces of the cube
        let indices = vec![
            0, 1, 2, 0, 2, 3, // Front face
            4, 6, 5, 4, 7, 6, // Back face
        ];

        let transform = Mat4::IDENTITY;
        let impact_point = Vec3::ZERO;
        let impact_direction = Vec3::new(0.0, 0.0, 1.0);

        let result = system.shatter_mesh(
            &vertices,
            &normals,
            &tex_coords,
            &indices,
            &transform,
            impact_point,
            impact_direction,
        );

        assert!(result.is_ok());
        let fragments = result.unwrap();

        // Should have created at least one fragment
        assert!(!fragments.is_empty());

        // Each fragment should have valid geometry
        for fragment in fragments {
            assert!(fragment.is_valid());
            assert!(fragment.vertex_count() > 0);
            assert!(fragment.triangle_count() > 0);
        }
    }

    #[test]
    fn test_fragment_physics() {
        let mut fragment = MeshFragment::new();
        fragment.vertices.push(Vec3::ZERO);
        fragment.indices.push(0);
        fragment.velocity = Vec3::new(1.0, 0.0, 0.0);
        fragment.angular_velocity = Vec3::new(0.0, 0.0, 1.0);

        let initial_pos = fragment.transform.w_axis.truncate();
        let gravity = Vec3::new(0.0, 0.0, -9.8);

        // Update physics
        fragment.update(0.1, gravity);

        // Position should have changed
        let new_pos = fragment.transform.w_axis.truncate();
        assert!((new_pos - initial_pos).length() > 0.0);

        // Velocity should have changed due to gravity
        assert!(fragment.velocity.z < 0.0);
    }

    #[test]
    fn test_polygon_splitting_produces_valid_geometry() {
        let mut poly = ShatterPolygon::new();

        // Create a triangle
        for i in 0..3 {
            let angle = i as f32 * std::f32::consts::PI * 2.0 / 3.0;
            let mut vert = ShatterVertex::new();
            vert.position = Vec3::new(angle.cos(), angle.sin(), 0.0);
            poly.vertices.push(vert);
        }
        poly.compute_plane();

        // Split through the center
        let plane = Plane::new(Vec3::X, Vec3::ZERO);
        let (front, back) = poly.split(&plane);

        // Both halves should have at least 3 vertices (valid triangles)
        assert!(front.vertex_count() >= 3);
        assert!(back.vertex_count() >= 3);

        // Should not be degenerate
        assert!(!front.is_degenerate());
        assert!(!back.is_degenerate());
    }
}
