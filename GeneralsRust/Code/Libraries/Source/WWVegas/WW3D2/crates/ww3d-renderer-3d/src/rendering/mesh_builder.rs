//! Mesh Builder System - Runtime mesh construction
//!
//! This module provides the MeshBuilderClass functionality from C++ WW3D2,
//! allowing procedural mesh generation at runtime. Used for terrain, decals,
//! dynamic effects, and other runtime geometry.

use glam::{Vec2, Vec3};
use std::collections::HashSet;

const EPSILON: f32 = 0.0001;
const MAX_PASSES: usize = 4;
const MAX_STAGES: usize = 8;
const HASH_TABLE_SIZE: usize = 4096;

/// Builder state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuilderState {
    /// Accepting input triangles
    AcceptingInput,
    /// Mesh has been processed
    MeshProcessed,
}

/// World info interface for smoothing across meshes.
pub trait WorldInfo {
    fn get_shared_vertex_normal(&self, pos: Vec3, smgroup: i32) -> Vec3;
    fn are_meshes_smoothed(&self) -> bool {
        true
    }
}

/// Vertex data for mesh building
#[derive(Debug, Clone)]
pub struct VertClass {
    /// Position of the vertex
    pub position: Vec3,
    /// Vertex normal (can be calculated by mesh builder)
    pub normal: Vec3,
    /// Smoothing group of the face this vertex was submitted with
    pub sm_group: i32,
    /// ID of the vertex, must match for vert to be welded
    pub id: i32,
    /// Bone influence if the mesh is a skin
    pub bone_index: i32,
    /// Index into the Max mesh.vertCol array
    pub max_vert_col_index: i32,
    /// Texture coordinates for each pass and stage
    pub tex_coord: [[Vec2; MAX_STAGES]; MAX_PASSES],
    /// Diffuse color for each pass
    pub diffuse_color: [Vec3; MAX_PASSES],
    /// Specular color for each pass
    pub specular_color: [Vec3; MAX_PASSES],
    /// Pre-calculated diffuse illumination for each pass
    pub diffuse_illumination: [Vec3; MAX_PASSES],
    /// Alpha for each pass
    pub alpha: [f32; MAX_PASSES],
    /// Vertex material index for each pass
    pub vertex_material_index: [i32; MAX_PASSES],
    /// User-set attributes
    pub attribute0: i32,
    pub attribute1: i32,

    // Internal fields set by builder
    /// Smooth bits that were on in all faces that contributed to this final vertex
    pub shared_sm_group: i32,
    /// Internal unique index
    pub unique_index: i32,
    /// Internal shade index
    pub shade_index: i32,
}

impl Default for VertClass {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            normal: Vec3::ZERO,
            sm_group: 0,
            id: 0,
            bone_index: 0,
            max_vert_col_index: 0,
            tex_coord: [[Vec2::ZERO; MAX_STAGES]; MAX_PASSES],
            diffuse_color: [Vec3::ONE; MAX_PASSES],
            specular_color: [Vec3::ONE; MAX_PASSES],
            diffuse_illumination: [Vec3::ZERO; MAX_PASSES],
            alpha: [1.0; MAX_PASSES],
            vertex_material_index: [-1; MAX_PASSES],
            attribute0: 0,
            attribute1: 0,
            shared_sm_group: 0,
            unique_index: 0,
            shade_index: 0,
        }
    }
}

/// Face data for mesh building
#[derive(Debug, Clone)]
pub struct FaceClass {
    /// Array of 3 vertices
    pub verts: [VertClass; 3],
    /// Smoothing group
    pub sm_group: i32,
    /// User-set index of the face
    pub index: i32,
    /// User-set attributes
    pub attributes: i32,
    /// Texture to use for each pass and stage
    pub texture_index: [[i32; MAX_STAGES]; MAX_PASSES],
    /// Shader for each pass
    pub shader_index: [i32; MAX_PASSES],
    /// Surface type identifier
    pub surface_type: u32,

    // Set by builder
    /// Index of addition
    pub add_index: i32,
    /// "Optimized" vertex indices
    pub vert_idx: [i32; 3],
    /// Face normal
    pub normal: Vec3,
    /// Plane distance
    pub dist: f32,
}

impl Default for FaceClass {
    fn default() -> Self {
        Self {
            verts: [
                VertClass::default(),
                VertClass::default(),
                VertClass::default(),
            ],
            sm_group: 0,
            index: 0,
            attributes: 0,
            texture_index: [[-1; MAX_STAGES]; MAX_PASSES],
            shader_index: [-1; MAX_PASSES],
            surface_type: 0,
            add_index: 0,
            vert_idx: [0; 3],
            normal: Vec3::ZERO,
            dist: 0.0,
        }
    }
}

impl FaceClass {
    /// Reset this face to default state
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Compute the plane equation for this face
    pub fn compute_plane(&mut self) {
        let v0 = self.verts[0].position;
        let v1 = self.verts[1].position;
        let v2 = self.verts[2].position;

        let edge1 = v1 - v0;
        let edge2 = v2 - v0;

        self.normal = edge1.cross(edge2);
        let length = self.normal.length();

        if length > EPSILON {
            self.normal /= length;
            self.dist = self.normal.dot(v0);
        } else {
            self.normal = Vec3::Z;
            self.dist = 0.0;
        }
    }

    /// Check if face is degenerate
    pub fn is_degenerate(&self) -> bool {
        for v0 in 0..3 {
            for v1 in (v0 + 1)..3 {
                if self.vert_idx[v0] == self.vert_idx[v1] {
                    return true;
                }
                let p0 = self.verts[v0].position;
                let p1 = self.verts[v1].position;
                if p0 == p1 {
                    return true;
                }
            }
        }
        let v0 = self.verts[0].position;
        let v1 = self.verts[1].position;
        let v2 = self.verts[2].position;

        let edge1 = v1 - v0;
        let edge2 = v2 - v0;

        let cross = edge1.cross(edge2);
        cross.length_squared() < EPSILON * EPSILON
    }
}

/// Statistics about the mesh
#[derive(Debug, Clone)]
pub struct MeshStats {
    /// Has at least one texture in given pass/stage
    pub has_texture: [[bool; MAX_STAGES]; MAX_PASSES],
    /// Has at least one shader in given pass
    pub has_shader: [bool; MAX_PASSES],
    /// Has at least one vert material in given pass
    pub has_vertex_material: [bool; MAX_PASSES],
    /// Has 2+ textures in given pass/stage
    pub has_per_poly_texture: [[bool; MAX_STAGES]; MAX_PASSES],
    /// Has 2+ shaders in given pass
    pub has_per_poly_shader: [bool; MAX_PASSES],
    /// Has 2+ vertex materials in given pass
    pub has_per_vertex_material: [bool; MAX_PASSES],
    /// Has diffuse colors in given pass
    pub has_diffuse_color: [bool; MAX_PASSES],
    /// Has specular colors in given pass
    pub has_specular_color: [bool; MAX_PASSES],
    /// Has diffuse illumination in given pass
    pub has_diffuse_illumination: [bool; MAX_PASSES],
    /// Has texture coords in given pass
    pub has_tex_coords: [[bool; MAX_STAGES]; MAX_PASSES],
    /// How many vertices were split due solely to UV discontinuities
    pub uv_split_count: i32,
    /// Number of strips that were created
    pub strip_count: i32,
    /// Longest strip created
    pub max_strip_length: i32,
    /// Average strip length
    pub avg_strip_length: f32,
}

impl Default for MeshStats {
    fn default() -> Self {
        Self {
            has_texture: [[false; MAX_STAGES]; MAX_PASSES],
            has_shader: [false; MAX_PASSES],
            has_vertex_material: [false; MAX_PASSES],
            has_per_poly_texture: [[false; MAX_STAGES]; MAX_PASSES],
            has_per_poly_shader: [false; MAX_PASSES],
            has_per_vertex_material: [false; MAX_PASSES],
            has_diffuse_color: [false; MAX_PASSES],
            has_specular_color: [false; MAX_PASSES],
            has_diffuse_illumination: [false; MAX_PASSES],
            has_tex_coords: [[false; MAX_STAGES]; MAX_PASSES],
            uv_split_count: 0,
            strip_count: 0,
            max_strip_length: 0,
            avg_strip_length: 0.0,
        }
    }
}

/// Main mesh builder class for procedural mesh generation
pub struct MeshBuilderClass {
    /// Current state of the builder
    state: BuilderState,
    /// Number of render passes
    pass_count: usize,
    /// Array of faces
    faces: Vec<FaceClass>,
    /// Number of faces currently added
    face_count: usize,
    /// Array of vertices
    verts: Vec<VertClass>,
    /// Number of vertices
    vert_count: usize,
    /// Current face being added
    cur_face: usize,
    /// Statistics about the mesh
    stats: MeshStats,
    /// Order the polys using texture indices in this pass
    poly_order_pass: usize,
    /// Order the polys using texture indices in this stage
    poly_order_stage: usize,
    /// Allocated face count
    alloc_face_count: usize,
    /// Growth rate of face array
    alloc_face_growth: usize,
    /// Optional world info for smoothing
    world_info: Option<Box<dyn WorldInfo + Send + Sync>>,
    uv_split_count: i32,
}

struct VertexArray {
    verts: Vec<VertClass>,
    hash_table: Vec<Vec<usize>>,
    uv_splits: i32,
    match_normals: bool,
    center: Vec3,
    extent: Vec3,
}

impl VertexArray {
    fn new(maxsize: usize, match_normals: bool) -> Self {
        Self {
            verts: Vec::with_capacity(maxsize),
            hash_table: vec![Vec::new(); HASH_TABLE_SIZE],
            uv_splits: 0,
            match_normals,
            center: Vec3::ZERO,
            extent: Vec3::ONE,
        }
    }

    fn set_bounds(&mut self, minv: Vec3, maxv: Vec3) {
        self.center = (maxv + minv) * 0.5;
        self.extent = (maxv - minv) * 0.5;
        if self.extent.x.abs() < EPSILON {
            self.extent.x = 1.0;
        }
        if self.extent.y.abs() < EPSILON {
            self.extent.y = 1.0;
        }
        if self.extent.z.abs() < EPSILON {
            self.extent.z = 1.0;
        }
    }

    fn compute_hash(x: f32, y: f32) -> usize {
        let ix = (x as i32) & 0x3F;
        let iy = (y as i32) & 0x3F;
        ((iy << 6) | ix) as usize
    }

    fn verts_shading_match(a: &VertClass, b: &VertClass) -> bool {
        let dv = (a.position - b.position).length();
        let smgroup_match = (a.sm_group & b.sm_group) != 0 || a.sm_group == b.sm_group;
        dv < EPSILON && smgroup_match && a.id == b.id
    }

    fn verts_match(&mut self, a: &VertClass, b: &VertClass) -> bool {
        if a.id != b.id {
            return false;
        }
        if (a.position - b.position).length() > EPSILON {
            return false;
        }

        if !self.match_normals {
            let smgroup_match = (a.sm_group & b.sm_group) != 0 || a.sm_group == b.sm_group;
            if !smgroup_match {
                return false;
            }
        } else if (a.normal - b.normal).length() > EPSILON {
            return false;
        }

        for pass in 0..MAX_PASSES {
            if a.diffuse_color[pass] != b.diffuse_color[pass] {
                return false;
            }
            if a.specular_color[pass] != b.specular_color[pass] {
                return false;
            }
            if a.diffuse_illumination[pass] != b.diffuse_illumination[pass] {
                return false;
            }
            if (a.alpha[pass] - b.alpha[pass]).abs() > EPSILON {
                return false;
            }
            if a.vertex_material_index[pass] != b.vertex_material_index[pass] {
                return false;
            }
        }

        for pass in 0..MAX_PASSES {
            for stage in 0..MAX_STAGES {
                if a.tex_coord[pass][stage] != b.tex_coord[pass][stage] {
                    self.uv_splits += 1;
                    return false;
                }
            }
        }
        true
    }

    fn submit_vertex(&mut self, vert: &VertClass) -> usize {
        let mut shadeindex: Option<usize> = None;
        let mut last_hash = usize::MAX;

        let xstart = if self.extent.x.abs() > EPSILON {
            (vert.position.x - self.center.x) / self.extent.x
        } else {
            self.center.x
        };
        let ystart = if self.extent.y.abs() > EPSILON {
            (vert.position.y - self.center.y) / self.extent.y
        } else {
            self.center.y
        };

        let mut x = xstart - EPSILON;
        while x <= xstart + EPSILON + 0.0000001 {
            let mut y = ystart - EPSILON;
            while y <= ystart + EPSILON + 0.000001 {
                let hash = Self::compute_hash(x, y);
                if hash != last_hash {
                    let bucket = self.hash_table[hash].clone();
                    for idx in bucket {
                        let test_vert = self.verts[idx].clone();
                        if Self::verts_shading_match(vert, &test_vert) && shadeindex.is_none() {
                            let shared = self.verts[idx].shared_sm_group & vert.sm_group;
                            self.verts[idx].shared_sm_group = shared;
                            shadeindex = Some(test_vert.unique_index as usize);
                        }
                        if self.verts_match(vert, &test_vert) {
                            return test_vert.unique_index as usize;
                        }
                    }
                }
                last_hash = hash;
                y += EPSILON;
            }
            x += EPSILON;
        }

        let new_index = self.verts.len();
        let mut new_vert = vert.clone();
        new_vert.unique_index = new_index as i32;
        if let Some(shade) = shadeindex {
            new_vert.shade_index = shade as i32;
        } else {
            new_vert.shade_index = new_index as i32;
            new_vert.shared_sm_group = new_vert.sm_group;
        }
        self.verts.push(new_vert);

        let hx = (vert.position.x - self.center.x) / self.extent.x;
        let hy = (vert.position.y - self.center.y) / self.extent.y;
        let hash = Self::compute_hash(hx, hy);
        self.hash_table[hash].push(new_index);
        new_index
    }

    fn propagate_shared_smooth_groups(&mut self) {
        for i in 0..self.verts.len() {
            let shade_index = self.verts[i].shade_index as usize;
            if shade_index < self.verts.len() {
                self.verts[i].shared_sm_group = self.verts[shade_index].shared_sm_group;
            }
        }
    }
}

impl MeshBuilderClass {
    /// Create a new mesh builder
    pub fn new(pass_count: usize, face_count_guess: usize, face_count_growth_rate: usize) -> Self {
        let mut builder = Self {
            state: BuilderState::AcceptingInput,
            pass_count: pass_count.min(MAX_PASSES),
            faces: Vec::with_capacity(face_count_guess),
            face_count: 0,
            verts: Vec::new(),
            vert_count: 0,
            cur_face: 0,
            stats: MeshStats::default(),
            poly_order_pass: 0,
            poly_order_stage: 0,
            alloc_face_count: face_count_guess,
            alloc_face_growth: face_count_growth_rate,
            world_info: None,
            uv_split_count: 0,
        };

        // Pre-allocate faces
        builder.faces.resize(face_count_guess, FaceClass::default());

        builder
    }

    /// Reset the builder to accept new input
    pub fn reset(
        &mut self,
        pass_count: usize,
        face_count_guess: usize,
        face_count_growth_rate: usize,
    ) {
        self.state = BuilderState::AcceptingInput;
        self.pass_count = pass_count.min(MAX_PASSES);
        self.face_count = 0;
        self.vert_count = 0;
        self.cur_face = 0;
        self.stats = MeshStats::default();
        self.poly_order_pass = 0;
        self.poly_order_stage = 0;
        self.alloc_face_count = face_count_guess;
        self.alloc_face_growth = face_count_growth_rate;
        self.world_info = None;
        self.uv_split_count = 0;

        self.faces.clear();
        self.faces.resize(face_count_guess, FaceClass::default());
        self.verts.clear();
    }

    /// Add a face to the mesh
    pub fn add_face(&mut self, face: FaceClass) -> Result<i32, String> {
        if self.state != BuilderState::AcceptingInput {
            return Err("Mesh builder is not accepting input".to_string());
        }

        // Grow face array if needed
        if self.cur_face >= self.faces.len() {
            self.grow_face_array();
        }

        let mut new_face = face;
        new_face.add_index = self.cur_face as i32;
        new_face.compute_plane();
        for i in 0..3 {
            new_face.verts[i].sm_group = new_face.sm_group;
        }

        self.faces[self.cur_face] = new_face;
        self.cur_face += 1;

        Ok((self.cur_face - 1) as i32)
    }

    /// Grow the face array
    fn grow_face_array(&mut self) {
        let new_size = self.faces.len() + self.alloc_face_growth;
        self.faces.resize(new_size, FaceClass::default());
    }

    /// Build the mesh from submitted faces
    pub fn build_mesh(&mut self, compute_normals: bool) -> Result<(), String> {
        if self.state != BuilderState::AcceptingInput {
            return Err("Mesh builder is not in accepting input state".to_string());
        }

        self.state = BuilderState::MeshProcessed;
        self.face_count = self.cur_face;

        // Optimize the mesh
        self.optimize_mesh(compute_normals)?;

        // Compute statistics and strip optimization in optimize_mesh
        Ok(())
    }

    /// Optimize the mesh
    fn optimize_mesh(&mut self, compute_normals: bool) -> Result<(), String> {
        // Build unique vertex array
        self.build_unique_vertices(compute_normals)?;

        // Remove degenerate/duplicate faces
        self.remove_degenerate_faces();

        // Compute face normals
        self.compute_face_normals();

        // Compute vertex normals if needed
        if compute_normals {
            self.compute_vertex_normals();
        }

        // Compute mesh stats
        self.compute_mesh_stats();
        self.stats.uv_split_count = self.uv_split_count;

        // Sort faces by material
        self.sort_faces_by_material();

        // Sort vertices by bone/material
        self.sort_vertices();

        // Strip optimize
        self.strip_optimize_mesh()?;

        Ok(())
    }

    /// Remove degenerate faces
    fn remove_degenerate_faces(&mut self) {
        let mut unique = HashSet::new();
        let mut new_faces = Vec::with_capacity(self.face_count);

        for face in self.faces.iter().take(self.face_count) {
            if face.is_degenerate() {
                continue;
            }
            let key = (face.vert_idx[0], face.vert_idx[1], face.vert_idx[2]);
            if unique.insert(key) {
                new_faces.push(face.clone());
            }
        }

        self.face_count = new_faces.len();
        self.faces.clear();
        self.faces.extend_from_slice(&new_faces);
        self.cur_face = self.face_count;
    }

    /// Compute face normals
    fn compute_face_normals(&mut self) {
        for i in 0..self.face_count {
            self.faces[i].compute_plane();
        }
    }

    /// Build unique vertices from face vertices
    fn build_unique_vertices(&mut self, compute_normals: bool) -> Result<(), String> {
        // Estimate vertex count (usually less than face_count * 3)
        let estimated_verts = (self.face_count * 3).min(self.face_count * 2);
        let match_normals = !compute_normals;
        let mut unique = VertexArray::new(estimated_verts, match_normals);

        if self.face_count == 0 {
            self.verts.clear();
            self.vert_count = 0;
            return Ok(());
        }

        let mut minv = self.faces[0].verts[0].position;
        let mut maxv = self.faces[0].verts[0].position;
        for face in self.faces.iter().take(self.face_count) {
            for vert in &face.verts {
                minv = minv.min(vert.position);
                maxv = maxv.max(vert.position);
            }
        }
        unique.set_bounds(minv, maxv);

        for face_idx in 0..self.face_count {
            for vert_idx in 0..3 {
                let vert = &self.faces[face_idx].verts[vert_idx];
                let idx = unique.submit_vertex(vert);
                self.faces[face_idx].vert_idx[vert_idx] = idx as i32;
            }
        }

        unique.propagate_shared_smooth_groups();
        self.uv_split_count = unique.uv_splits;

        self.verts = unique.verts;
        self.vert_count = self.verts.len();
        Ok(())
    }

    /// Create a hash key for vertex uniqueness testing
    /// Compute vertex normals
    fn compute_vertex_normals(&mut self) {
        // Reset all normals
        for vert in &mut self.verts {
            vert.normal = Vec3::ZERO;
        }

        // Accumulate face normals
        for face in &self.faces[0..self.face_count] {
            for &vert_idx in &face.vert_idx {
                if (vert_idx as usize) < self.verts.len() {
                    let shade_index = self.verts[vert_idx as usize].shade_index as usize;
                    if shade_index < self.verts.len() {
                        self.verts[shade_index].normal += face.normal;
                    }
                }
            }
        }

        // Smooth with world info
        if let Some(world) = &self.world_info {
            if world.are_meshes_smoothed() {
                for vert in &mut self.verts {
                    if vert.shade_index as usize == vert.unique_index as usize {
                        vert.normal +=
                            world.get_shared_vertex_normal(vert.position, vert.shared_sm_group);
                    }
                }
            }
        }

        // Normalize
        for vert in &mut self.verts {
            let length = vert.normal.length();
            if length > EPSILON {
                vert.normal /= length;
            }
        }

        // Propagate normals to all verts with same shade index
        for idx in 0..self.verts.len() {
            let shade_index = self.verts[idx].shade_index as usize;
            if shade_index < self.verts.len() {
                self.verts[idx].normal = self.verts[shade_index].normal.normalize_or_zero();
            }
        }
    }

    /// Sort faces by material
    fn sort_faces_by_material(&mut self) {
        let pass = self.poly_order_pass;
        let stage = self.poly_order_stage;
        self.faces[0..self.face_count].sort_by(|a, b| {
            let ta = a.texture_index[pass][stage];
            let tb = b.texture_index[pass][stage];
            if ta != tb {
                return ta.cmp(&tb);
            }
            let va = a.verts[0].vertex_material_index[pass];
            let vb = b.verts[0].vertex_material_index[pass];
            va.cmp(&vb)
        });
    }

    fn sort_vertices(&mut self) {
        self.verts.sort_by(|a, b| {
            if a.bone_index != b.bone_index {
                return a.bone_index.cmp(&b.bone_index);
            }
            a.vertex_material_index[0].cmp(&b.vertex_material_index[0])
        });

        let mut remap = vec![0usize; self.verts.len()];
        for (new_idx, vert) in self.verts.iter().enumerate() {
            remap[vert.unique_index as usize] = new_idx;
        }
        for face in self.faces.iter_mut().take(self.face_count) {
            for idx in &mut face.vert_idx {
                let old = *idx as usize;
                if old < remap.len() {
                    *idx = remap[old] as i32;
                }
            }
        }
    }

    /// Strip optimize the mesh
    fn strip_optimize_mesh(&mut self) -> Result<(), String> {
        if self.face_count == 0 {
            self.stats.strip_count = 0;
            self.stats.max_strip_length = 0;
            self.stats.avg_strip_length = 0.0;
            return Ok(());
        }

        #[derive(Clone, Copy)]
        struct WingedEdge {
            material_idx: i32,
            vertex: [i32; 2],
            poly: [i32; 2],
        }

        #[derive(Clone, Copy)]
        struct WingedEdgePoly {
            edge: [usize; 3],
        }

        let mut edge_hash: Vec<Vec<usize>> = vec![Vec::new(); 512];
        let mut edges: Vec<WingedEdge> = Vec::with_capacity(self.face_count * 3);
        let mut pedges: Vec<WingedEdgePoly> =
            vec![WingedEdgePoly { edge: [0; 3] }; self.face_count];

        let mut new_faces: Vec<FaceClass> = vec![FaceClass::default(); self.face_count];
        let mut newmat: Vec<i32> = vec![0; self.face_count];
        let mut premap: Vec<i32> = vec![-1; self.face_count];
        let mut vtimestamp: Vec<i32> = vec![-1; self.vert_count];

        let mut vcount: i32 = 0;
        let mut polysinserted: usize = 0;
        let mut lastmat: i32 = 0;

        // Build edge table
        for i in 0..self.face_count {
            let mat = self.faces[i].texture_index[self.poly_order_pass][self.poly_order_stage];
            for j in 0..3 {
                let v0 = self.faces[i].vert_idx[j];
                let v1 = self.faces[i].vert_idx[(j + 1) % 3];
                let (a, b) = if v0 > v1 { (v1, v0) } else { (v0, v1) };
                let hash = ((a + b * 119) & 511) as usize;

                let mut found: Option<usize> = None;
                for &edge_idx in &edge_hash[hash] {
                    let edge = &edges[edge_idx];
                    if edge.vertex[0] == a && edge.vertex[1] == b && edge.material_idx == mat {
                        found = Some(edge_idx);
                        break;
                    }
                }

                let edge_idx = if let Some(idx) = found {
                    edges[idx].poly[1] = i as i32;
                    idx
                } else {
                    let edge = WingedEdge {
                        material_idx: mat,
                        vertex: [a, b],
                        poly: [i as i32, -1],
                    };
                    edges.push(edge);
                    let idx = edges.len() - 1;
                    edge_hash[hash].push(idx);
                    idx
                };

                pedges[i].edge[j] = edge_idx;
            }
        }

        self.stats.strip_count = 0;
        self.stats.max_strip_length = 0;
        self.stats.avg_strip_length = 0.0;

        while polysinserted < self.face_count {
            let mut startpoly: i32 = -1;
            let mut bestc: i32 = 1 << 29;
            let startpass = 0;

            for findpass in startpass..2 {
                for i in 0..self.face_count {
                    if premap[i] != -1 {
                        continue;
                    }
                    if findpass == 0
                        && self.faces[i].texture_index[self.poly_order_pass][self.poly_order_stage]
                            != lastmat
                    {
                        continue;
                    }

                    let mut c: i32 = 0;
                    for j in 0..3 {
                        if edges[pedges[i].edge[j]].poly[1] >= 0 {
                            c += vcount + 1;
                        }
                    }
                    for j in 0..3 {
                        let vidx = self.faces[i].vert_idx[j] as usize;
                        c += vcount - vtimestamp[vidx];
                    }
                    if c < bestc {
                        bestc = c;
                        startpoly = i as i32;
                    }
                }
                if startpoly != -1 {
                    break;
                }
            }

            if startpoly == -1 {
                break;
            }

            self.stats.strip_count += 1;
            let startpoly_usize = startpoly as usize;
            lastmat = self.faces[startpoly_usize].texture_index[self.poly_order_pass]
                [self.poly_order_stage];
            newmat[polysinserted] = lastmat;

            let mut found_shared_edge = false;
            new_faces[polysinserted] = self.faces[startpoly_usize].clone();
            {
                let newpoly = &mut new_faces[polysinserted];
                for edge_index in 0..3 {
                    if found_shared_edge {
                        break;
                    }
                    for side_index in 0..2 {
                        let poly = edges[pedges[startpoly_usize].edge[edge_index]].poly[side_index];
                        if poly != -1
                            && poly as usize != startpoly_usize
                            && premap[poly as usize] == -1
                        {
                            let edge = &edges[pedges[startpoly_usize].edge[edge_index]];
                            let mut first_vert = -1;
                            for vidx in 0..3 {
                                let v = newpoly.vert_idx[vidx];
                                if v != edge.vertex[0] && v != edge.vertex[1] {
                                    first_vert = v;
                                    break;
                                }
                            }
                            if first_vert != -1 {
                                while newpoly.vert_idx[0] != first_vert {
                                    let tmp = newpoly.vert_idx[0];
                                    newpoly.vert_idx[0] = newpoly.vert_idx[1];
                                    newpoly.vert_idx[1] = newpoly.vert_idx[2];
                                    newpoly.vert_idx[2] = tmp;
                                }
                                found_shared_edge = true;
                                break;
                            }
                        }
                    }
                }
            }

            if !found_shared_edge {
                new_faces[polysinserted] = self.faces[startpoly_usize].clone();
            }

            premap[startpoly_usize] = polysinserted as i32;
            polysinserted += 1;

            for i in 0..3 {
                let v = self.faces[startpoly_usize].vert_idx[i] as usize;
                if vtimestamp[v] == -1 {
                    vtimestamp[v] = vcount;
                    vcount += 1;
                }
            }

            if edges[pedges[startpoly_usize].edge[0]].poly[1] == -1
                && edges[pedges[startpoly_usize].edge[1]].poly[1] == -1
                && edges[pedges[startpoly_usize].edge[2]].poly[1] == -1
            {
                continue;
            }

            let mut v_fifo = [
                new_faces[polysinserted - 1].vert_idx[1],
                new_faces[polysinserted - 1].vert_idx[2],
            ];
            let mut scnt = 0;
            let mut nextpoly = startpoly;

            while nextpoly != -1 {
                let start = nextpoly as usize;
                nextpoly = -1;

                for i in 0..3 {
                    let edge = &edges[pedges[start].edge[i]];
                    let match_edge = (edge.vertex[0] == v_fifo[0] && edge.vertex[1] == v_fifo[1])
                        || (edge.vertex[1] == v_fifo[0] && edge.vertex[0] == v_fifo[1]);
                    if !match_edge {
                        continue;
                    }
                    for j in 0..2 {
                        let poly = edge.poly[j];
                        if poly > -1 && premap[poly as usize] == -1 {
                            nextpoly = poly;
                            break;
                        }
                    }
                    if nextpoly != -1 {
                        break;
                    }
                }

                if nextpoly == -1 {
                    break;
                }

                let np = nextpoly as usize;
                let mut nw = -1;
                for i in 0..3 {
                    let vidx = self.faces[np].vert_idx[i];
                    if vidx != v_fifo[0] && vidx != v_fifo[1] {
                        nw = i as i32;
                        break;
                    }
                }
                if nw == -1 {
                    break;
                }

                let new_vindex = self.faces[np].vert_idx[nw as usize];
                newmat[polysinserted] =
                    self.faces[np].texture_index[self.poly_order_pass][self.poly_order_stage];

                new_faces[polysinserted].vert_idx[0] = v_fifo[0];
                new_faces[polysinserted].vert_idx[1] = v_fifo[1];
                new_faces[polysinserted].vert_idx[2] = new_vindex;

                if scnt & 1 == 0 {
                    new_faces[polysinserted].vert_idx.swap(0, 1);
                }

                v_fifo[0] = v_fifo[1];
                v_fifo[1] = new_vindex;

                let new_vidx = new_vindex as usize;
                if vtimestamp[new_vidx] == -1 {
                    vtimestamp[new_vidx] = vcount;
                    vcount += 1;
                }

                premap[np] = polysinserted as i32;
                polysinserted += 1;
                scnt += 1;
            }

            self.stats.avg_strip_length += (scnt + 1) as f32;
            if scnt + 1 > self.stats.max_strip_length {
                self.stats.max_strip_length = scnt + 1;
            }
        }

        for i in 0..self.face_count {
            let old_idx = i;
            let new_idx = premap[i] as usize;
            for pass in 0..MAX_PASSES {
                for stage in 0..MAX_STAGES {
                    new_faces[new_idx].texture_index[pass][stage] =
                        self.faces[old_idx].texture_index[pass][stage];
                }
                new_faces[new_idx].shader_index[pass] = self.faces[old_idx].shader_index[pass];
            }

            new_faces[new_idx].sm_group = self.faces[old_idx].sm_group;
            new_faces[new_idx].index = self.faces[old_idx].index;
            new_faces[new_idx].attributes = self.faces[old_idx].attributes;
            new_faces[new_idx].add_index = self.faces[old_idx].add_index;
            new_faces[new_idx].normal = self.faces[old_idx].normal;
            new_faces[new_idx].dist = self.faces[old_idx].dist;
            new_faces[new_idx].surface_type = self.faces[old_idx].surface_type;

            debug_assert!(
                newmat[new_idx]
                    == self.faces[old_idx].texture_index[self.poly_order_pass]
                        [self.poly_order_stage]
            );
        }

        self.faces = new_faces;
        self.alloc_face_count = self.face_count;

        if self.stats.strip_count > 0 {
            self.stats.avg_strip_length /= self.stats.strip_count as f32;
        }

        Ok(())
    }

    /// Compute mesh statistics
    fn compute_mesh_stats(&mut self) {
        self.stats = MeshStats::default();
        let mut tex_index = [[-1; MAX_STAGES]; MAX_PASSES];
        let mut shader_index = [-1; MAX_PASSES];
        let mut vmat_index = [-1; MAX_PASSES];

        if self.face_count > 0 {
            for pass in 0..MAX_PASSES {
                for stage in 0..MAX_STAGES {
                    tex_index[pass][stage] = self.faces[0].texture_index[pass][stage];
                }
                shader_index[pass] = self.faces[0].shader_index[pass];
                vmat_index[pass] = self
                    .verts
                    .first()
                    .map(|v| v.vertex_material_index[pass])
                    .unwrap_or(-1);
            }
        }

        for pass in 0..MAX_PASSES {
            for stage in 0..MAX_STAGES {
                for face in &self.faces[0..self.face_count] {
                    if face.texture_index[pass][stage] != tex_index[pass][stage] {
                        self.stats.has_per_poly_texture[pass][stage] = true;
                        break;
                    }
                }
                for vert in &self.verts {
                    if vert.tex_coord[pass][stage] != Vec2::ZERO {
                        self.stats.has_tex_coords[pass][stage] = true;
                        break;
                    }
                }
            }

            for face in &self.faces[0..self.face_count] {
                if face.shader_index[pass] != shader_index[pass] {
                    self.stats.has_per_poly_shader[pass] = true;
                    break;
                }
            }

            for vert in &self.verts {
                if vert.vertex_material_index[pass] != vmat_index[pass] {
                    self.stats.has_per_vertex_material[pass] = true;
                    break;
                }
            }

            for vert in &self.verts {
                if vert.diffuse_color[pass] != Vec3::ONE || (vert.alpha[pass] - 1.0).abs() > EPSILON
                {
                    self.stats.has_diffuse_color[pass] = true;
                    break;
                }
            }

            for vert in &self.verts {
                if vert.specular_color[pass] != Vec3::ONE {
                    self.stats.has_specular_color[pass] = true;
                    break;
                }
            }

            for vert in &self.verts {
                if vert.diffuse_illumination[pass] != Vec3::ZERO {
                    self.stats.has_diffuse_illumination[pass] = true;
                    break;
                }
            }

            for stage in 0..MAX_STAGES {
                for face in &self.faces[0..self.face_count] {
                    if face.texture_index[pass][stage] != -1 {
                        self.stats.has_texture[pass][stage] = true;
                        break;
                    }
                }
            }

            for face in &self.faces[0..self.face_count] {
                if face.shader_index[pass] != -1 {
                    self.stats.has_shader[pass] = true;
                    break;
                }
            }

            for vert in &self.verts {
                if vert.vertex_material_index[pass] != -1 {
                    self.stats.has_vertex_material[pass] = true;
                    break;
                }
            }
        }
    }

    /// Set polygon ordering channel
    pub fn set_polygon_ordering_channel(&mut self, pass: usize, stage: usize) {
        self.poly_order_pass = pass.min(MAX_PASSES - 1);
        self.poly_order_stage = stage.min(MAX_STAGES - 1);
    }

    /// Get pass count
    pub fn get_pass_count(&self) -> usize {
        self.pass_count
    }

    /// Get vertex count
    pub fn get_vertex_count(&self) -> Result<usize, String> {
        if self.state != BuilderState::MeshProcessed {
            return Err("Mesh not yet processed".to_string());
        }
        Ok(self.vert_count)
    }

    /// Get face count
    pub fn get_face_count(&self) -> Result<usize, String> {
        if self.state != BuilderState::MeshProcessed {
            return Err("Mesh not yet processed".to_string());
        }
        Ok(self.face_count)
    }

    /// Get vertex at index
    pub fn get_vertex(&self, index: usize) -> Result<&VertClass, String> {
        if self.state != BuilderState::MeshProcessed {
            return Err("Mesh not yet processed".to_string());
        }
        self.verts
            .get(index)
            .ok_or_else(|| format!("Vertex index {} out of bounds", index))
    }

    /// Get face at index
    pub fn get_face(&self, index: usize) -> Result<&FaceClass, String> {
        if self.state != BuilderState::MeshProcessed {
            return Err("Mesh not yet processed".to_string());
        }
        if index >= self.face_count {
            return Err(format!("Face index {} out of bounds", index));
        }
        Ok(&self.faces[index])
    }

    /// Get mutable vertex at index
    pub fn get_vertex_mut(&mut self, index: usize) -> Result<&mut VertClass, String> {
        if self.state != BuilderState::MeshProcessed {
            return Err("Mesh not yet processed".to_string());
        }
        self.verts
            .get_mut(index)
            .ok_or_else(|| format!("Vertex index {} out of bounds", index))
    }

    /// Get mutable face at index
    pub fn get_face_mut(&mut self, index: usize) -> Result<&mut FaceClass, String> {
        if self.state != BuilderState::MeshProcessed {
            return Err("Mesh not yet processed".to_string());
        }
        if index >= self.face_count {
            return Err(format!("Face index {} out of bounds", index));
        }
        Ok(&mut self.faces[index])
    }

    /// Compute bounding box
    pub fn compute_bounding_box(&self) -> Result<(Vec3, Vec3), String> {
        if self.state != BuilderState::MeshProcessed {
            return Err("Mesh not yet processed".to_string());
        }

        if self.verts.is_empty() {
            return Ok((Vec3::ZERO, Vec3::ZERO));
        }

        let mut min = self.verts[0].position;
        let mut max = self.verts[0].position;

        for vert in &self.verts[1..] {
            min = min.min(vert.position);
            max = max.max(vert.position);
        }

        Ok((min, max))
    }

    /// Compute bounding sphere
    pub fn compute_bounding_sphere(&self) -> Result<(Vec3, f32), String> {
        if self.state != BuilderState::MeshProcessed {
            return Err("Mesh not yet processed".to_string());
        }

        if self.verts.is_empty() {
            return Ok((Vec3::ZERO, 0.0));
        }

        let mut xmin = self.verts[0].position;
        let mut xmax = self.verts[0].position;
        let mut ymin = self.verts[0].position;
        let mut ymax = self.verts[0].position;
        let mut zmin = self.verts[0].position;
        let mut zmax = self.verts[0].position;

        for vert in &self.verts[1..] {
            let p = vert.position;
            if p.x < xmin.x {
                xmin = p;
            }
            if p.x > xmax.x {
                xmax = p;
            }
            if p.y < ymin.y {
                ymin = p;
            }
            if p.y > ymax.y {
                ymax = p;
            }
            if p.z < zmin.z {
                zmin = p;
            }
            if p.z > zmax.z {
                zmax = p;
            }
        }

        let xspan = (xmax - xmin).length_squared();
        let yspan = (ymax - ymin).length_squared();
        let zspan = (zmax - zmin).length_squared();

        let mut dia1 = xmin;
        let mut dia2 = xmax;
        let mut maxspan = xspan;
        if yspan > maxspan {
            maxspan = yspan;
            dia1 = ymin;
            dia2 = ymax;
        }
        if zspan > maxspan {
            dia1 = zmin;
            dia2 = zmax;
        }

        let mut center = (dia1 + dia2) * 0.5;
        let mut radius = (dia2 - center).length();
        let mut radsqr = radius * radius;

        for vert in &self.verts {
            let diff = vert.position - center;
            let testrad2 = diff.length_squared();
            if testrad2 > radsqr {
                let testrad = testrad2.sqrt();
                radius = (radius + testrad) * 0.5;
                radsqr = radius * radius;
                let old_to_new = testrad - radius;
                if testrad > 0.0 {
                    center = (center * radius + vert.position * old_to_new) / testrad;
                }
            }
        }

        Ok((center, radius))
    }

    /// Get mesh statistics
    pub fn get_mesh_stats(&self) -> Result<&MeshStats, String> {
        if self.state != BuilderState::MeshProcessed {
            return Err("Mesh not yet processed".to_string());
        }
        Ok(&self.stats)
    }

    pub fn set_world_info(&mut self, world_info: Option<Box<dyn WorldInfo + Send + Sync>>) {
        self.world_info = world_info;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mesh_builder_basic() {
        let mut builder = MeshBuilderClass::new(1, 10, 10);

        // Create a simple triangle
        let mut face = FaceClass::default();
        face.verts[0].position = Vec3::new(0.0, 0.0, 0.0);
        face.verts[1].position = Vec3::new(1.0, 0.0, 0.0);
        face.verts[2].position = Vec3::new(0.0, 1.0, 0.0);

        let result = builder.add_face(face);
        assert!(result.is_ok());

        let result = builder.build_mesh(true);
        assert!(result.is_ok());

        assert_eq!(builder.get_face_count().unwrap(), 1);
        assert!(builder.get_vertex_count().unwrap() <= 3);
    }

    #[test]
    fn test_degenerate_face_removal() {
        let mut builder = MeshBuilderClass::new(1, 10, 10);

        // Create a degenerate triangle (all points collinear)
        let mut face = FaceClass::default();
        face.verts[0].position = Vec3::new(0.0, 0.0, 0.0);
        face.verts[1].position = Vec3::new(1.0, 0.0, 0.0);
        face.verts[2].position = Vec3::new(2.0, 0.0, 0.0);

        builder.add_face(face).unwrap();
        builder.build_mesh(false).unwrap();

        // Degenerate face should be removed
        assert_eq!(builder.get_face_count().unwrap(), 0);
    }
}
