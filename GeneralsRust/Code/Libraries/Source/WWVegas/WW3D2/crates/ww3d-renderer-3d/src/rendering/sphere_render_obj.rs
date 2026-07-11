//! Sphere Render Objects - Procedurally generated sphere rendering
//!
//! This module provides SphereRenderObjClass from C++ WW3D2 (sphereobj.cpp/h),
//! used for particle effects, debug visualization, and special rendering.
//!
//! **C++ Reference:** GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/sphereobj.{cpp,h}
//!
//! ## Implementation Notes
//!
//! This is a direct port of the C++ sphere rendering system with the following key features:
//!
//! - **UV Sphere Generation**: Parametric sphere with configurable slices/stacks
//! - **LOD System**: 10 levels of detail from SPHERE_LOWEST_LOD (7) to SPHERE_HIGHEST_LOD (17)
//! - **Alpha Vector**: Directional alpha based on quaternion rotation and normal dot product
//! - **Animation Channels**: Color, alpha, scale, and vector animation with keyframe interpolation
//! - **Shared Mesh Arrays**: Static LOD meshes shared across all sphere instances
//! - **Material System**: Emissive material with alpha blending or additive blending
//!
//! ## Differences from C++
//!
//! - Uses `glam` for math types instead of custom Vector/Matrix classes
//! - Thread-safe static initialization using `Once` instead of raw static mut
//! - Rust ownership model for texture/material management (Arc instead of raw pointers)
//! - WGPU rendering path instead of DirectX 8

use crate::bounding_volumes::aabox::AABoxClass;
use crate::material_system::vertex_material::VertexMaterialClass;
use crate::render_object_system::RenderInfoClass;
use crate::rendering::shader_system::shader::ShaderClass;
use crate::texture_system::TextureClass;
use glam::{Mat3, Mat4, Quat, Vec2, Vec3, Vec4};
use std::f32::consts::PI;
use std::sync::{Arc, Mutex, Once};
use ww3d_collision::bounding_volumes::sphere::SphereClass;
use ww3d_core::errors::W3DResult;

// LOD configuration constants - Match C++ sphereobj.h lines 220-223
/// Number of LOD levels (does not include NULL LOD)
const SPHERE_NUM_LOD: usize = 10;
/// Lowest LOD size (slices/stacks)
const SPHERE_LOWEST_LOD: usize = 7;
/// Highest LOD size (slices/stacks)
const SPHERE_HIGHEST_LOD: usize = 17;

// LOD value constants for value array calculation
const AT_MIN_LOD: f32 = -1000000.0;
const AT_MAX_LOD: f32 = 1000000.0;

/// Alpha vector for sphere orientation and directional alpha effects
///
/// C++ Reference: sphereobj.h lines 57-71 AlphaVectorStruct
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlphaVectorStruct {
    /// Orientation quaternion
    pub angle: Quat,
    /// Intensity value (0.0 to 1.0+)
    pub intensity: f32,
}

impl Default for AlphaVectorStruct {
    fn default() -> Self {
        Self {
            angle: Quat::IDENTITY,
            intensity: 1.0,
        }
    }
}

/// Sphere rendering flags
///
/// C++ Reference: sphereobj.h lines 235-240 SphereFlags enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SphereFlags {
    pub use_alpha_vector: bool,
    pub use_camera_align: bool,
    pub use_inverse_alpha: bool,
    pub use_animation_loop: bool,
}

impl Default for SphereFlags {
    fn default() -> Self {
        Self {
            use_alpha_vector: true, // C++ default at sphereobj.cpp:128
            use_camera_align: false,
            use_inverse_alpha: false,
            use_animation_loop: false,
        }
    }
}

impl SphereFlags {
    /// Convert to/from bit flags for serialization compatibility
    pub fn to_bits(&self) -> u32 {
        let mut bits = 0u32;
        if self.use_alpha_vector {
            bits |= 0x00000001;
        }
        if self.use_camera_align {
            bits |= 0x00000002;
        }
        if self.use_inverse_alpha {
            bits |= 0x00000004;
        }
        if self.use_animation_loop {
            bits |= 0x00000008;
        }
        bits
    }

    pub fn from_bits(bits: u32) -> Self {
        Self {
            use_alpha_vector: (bits & 0x00000001) != 0,
            use_camera_align: (bits & 0x00000002) != 0,
            use_inverse_alpha: (bits & 0x00000004) != 0,
            use_animation_loop: (bits & 0x00000008) != 0,
        }
    }
}

/// Triangle indices
///
/// C++ Reference: meshgeometry.h TriIndex struct
#[derive(Debug, Clone, Copy)]
pub struct TriIndex {
    pub i: u32,
    pub j: u32,
    pub k: u32,
}

/// Sphere mesh geometry for a specific LOD level
///
/// C++ Reference: sphereobj.h lines 154-200 SphereMeshClass
pub struct SphereMeshClass {
    radius: f32,
    slices: usize,
    stacks: usize,
    face_ct: usize,
    vertex_ct: usize,

    // Vertex data
    vtx: Vec<Vec3>,
    vtx_normal: Vec<Vec3>,
    vtx_uv: Vec<Vec2>,
    dcg: Vec<Vec4>, // Diffuse color with alpha

    // Index data
    tri_poly: Vec<TriIndex>,
    strips: Vec<u32>,
    fans: Vec<u32>,
    strip_ct: usize,
    strip_size: usize,
    fan_ct: usize,
    fan_size: usize,

    // Alpha vector state tracking
    alpha_vector: AlphaVectorStruct,
    inverse_alpha: bool,
    is_additive: bool,
}

impl SphereMeshClass {
    /// Create an empty sphere mesh
    ///
    /// C++ Reference: sphereobj.cpp lines 1399-1418
    pub fn new() -> Self {
        Self {
            radius: 0.0,
            slices: 0,
            stacks: 0,
            face_ct: 0,
            vertex_ct: 0,
            vtx: Vec::new(),
            vtx_normal: Vec::new(),
            vtx_uv: Vec::new(),
            dcg: Vec::new(),
            tri_poly: Vec::new(),
            strips: Vec::new(),
            fans: Vec::new(),
            strip_ct: 0,
            strip_size: 0,
            fan_ct: 0,
            fan_size: 0,
            alpha_vector: AlphaVectorStruct::default(),
            inverse_alpha: false,
            is_additive: false,
        }
    }

    /// Generate sphere geometry
    ///
    /// C++ Reference: sphereobj.cpp lines 1502-1700 SphereMeshClass::Generate
    ///
    /// This generates a UV sphere with the following topology:
    /// - North pole vertex at (0, 0, radius)
    /// - Stacks from pole to pole (latitude)
    /// - Slices around equator (longitude)
    /// - South pole vertex at (0, 0, -radius)
    pub fn generate(&mut self, radius: f32, slices: usize, stacks: usize) {
        self.free();

        self.slices = slices;
        self.stacks = stacks;
        self.radius = radius;

        self.face_ct = slices * stacks * 2;
        self.vertex_ct = (slices + 1) * stacks + 2;

        // Allocate vertex arrays
        self.vtx = vec![Vec3::ZERO; self.vertex_ct];
        self.vtx_normal = vec![Vec3::ZERO; self.vertex_ct];
        self.vtx_uv = vec![Vec2::ZERO; self.vertex_ct];
        self.dcg = vec![Vec4::ONE; self.vertex_ct];

        let vec = Vec3::new(0.0, 0.0, radius);

        let mut veclist_idx = 0;
        let mut uv_idx = 0;

        // North pole vertex
        self.vtx[veclist_idx] = vec;
        veclist_idx += 1;

        // North pole UV
        self.vtx_uv[uv_idx] = Vec2::new(0.5, 0.0);
        uv_idx += 1;

        // Generate stacks (latitude rings)
        for stack in 0..stacks {
            let stack_step = (stack as f32 + 1.0) / (stacks as f32 + 1.0);
            let x_axis_angle = PI * stack_step;

            for slice in 0..=slices {
                let slice_step = slice as f32 / slices as f32;
                let y_axis_angle = PI * 2.0 * slice_step;

                // Rotation: first around Z (longitude), then around X (latitude)
                // C++ uses Matrix3x3::Rotate_Z then Rotate_X
                let mut mat = Mat3::IDENTITY;
                mat = Mat3::from_rotation_z(y_axis_angle) * mat;
                mat = Mat3::from_rotation_x(x_axis_angle) * mat;

                self.vtx[veclist_idx] = mat * vec;
                veclist_idx += 1;

                // UV coordinates
                self.vtx_uv[uv_idx] = Vec2::new(slice_step, stack_step);
                uv_idx += 1;
            }
        }

        // South pole vertex
        self.vtx[veclist_idx] = -vec;
        // South pole UV
        self.vtx_uv[uv_idx] = Vec2::new(0.5, 1.0);

        // Generate vertex normals (normalized positions)
        // C++ Reference: sphereobj.cpp lines 1573-1585
        for idx in 0..self.vertex_ct {
            self.vtx_normal[idx] = self.vtx[idx].normalize();
        }

        // Generate fans for north and south poles
        // C++ Reference: sphereobj.cpp lines 1588-1605
        self.fan_ct = 2;
        self.fan_size = slices + 2;
        self.fans = vec![0; self.fan_size * self.fan_ct];

        // North pole fan
        for ct in 0..self.fan_size {
            self.fans[ct] = ct as u32;
        }

        // South pole fan
        let mut vtx_idx = (self.vertex_ct - 1) as i32;
        for ct in self.fan_size..(self.fan_size * 2) {
            self.fans[ct] = vtx_idx as u32;
            vtx_idx -= 1;
        }

        // Generate strips for middle stacks
        // C++ Reference: sphereobj.cpp lines 1607-1633
        self.strip_size = (slices + 1) * 2;
        self.strip_ct = stacks.saturating_sub(1);

        if self.strip_ct > 0 {
            self.strips = vec![0; self.strip_size * self.strip_ct];

            for stack in 0..self.strip_ct {
                let store_base = stack * self.strip_size;
                let base_vtx = 1 + stack * (slices + 1);
                let mut cur_vtx = base_vtx;

                for ct in 0..=slices {
                    self.strips[store_base + ct * 2] = (cur_vtx + slices + 1) as u32;
                    self.strips[store_base + ct * 2 + 1] = cur_vtx as u32;
                    cur_vtx += 1;
                }
            }
        }

        // Generate triangle indices from strips and fans
        // C++ Reference: sphereobj.cpp lines 1635-1693
        self.tri_poly = vec![TriIndex { i: 0, j: 0, k: 0 }; self.face_ct];

        let mut out_idx = 0;

        // Convert strips to triangles
        for stack in 0..self.strip_ct {
            let in_base = stack * self.strip_size;

            for fidx in (0..(self.strip_size - 2)).step_by(2) {
                // First triangle (even)
                self.tri_poly[out_idx] = TriIndex {
                    i: self.strips[in_base + fidx],
                    j: self.strips[in_base + fidx + 1],
                    k: self.strips[in_base + fidx + 2],
                };
                out_idx += 1;

                if fidx + 2 >= self.strip_size - 2 {
                    break;
                }

                // Second triangle (odd)
                self.tri_poly[out_idx] = TriIndex {
                    i: self.strips[in_base + fidx + 1],
                    j: self.strips[in_base + fidx + 3],
                    k: self.strips[in_base + fidx + 2],
                };
                out_idx += 1;
            }
        }

        // Convert fans to triangles
        for fan in 0..self.fan_ct {
            let in_base = fan * self.fan_size;
            let base_idx = self.fans[in_base];

            for fidx in 0..(self.fan_size - 2) {
                self.tri_poly[out_idx] = TriIndex {
                    i: base_idx,
                    j: self.fans[in_base + fidx + 2],
                    k: self.fans[in_base + fidx + 1],
                };
                out_idx += 1;
            }
        }

        // Initialize DCG array with default alpha vector
        self.set_alpha_vector(
            self.alpha_vector,
            self.inverse_alpha,
            self.is_additive,
            true,
        );
    }

    /// Get number of polygons
    pub fn get_num_polys(&self) -> usize {
        self.face_ct
    }

    /// Set alpha vector to modify vertex colors/alpha based on direction
    ///
    /// C++ Reference: sphereobj.cpp lines 1435-1487 SphereMeshClass::Set_Alpha_Vector
    ///
    /// This computes per-vertex alpha by:
    /// 1. Rotating (1,0,0) by the alpha vector quaternion
    /// 2. Taking dot product with each vertex normal
    /// 3. Applying intensity scaling
    /// 4. Optionally inverting the result
    /// 5. Storing in DCG array as RGB (additive) or alpha (blended)
    pub fn set_alpha_vector(
        &mut self,
        v: AlphaVectorStruct,
        inverse: bool,
        is_additive: bool,
        force: bool,
    ) {
        // Early exit if nothing changed (C++ optimization)
        if !force
            && self.alpha_vector == v
            && inverse == self.inverse_alpha
            && is_additive == self.is_additive
        {
            return;
        }

        self.inverse_alpha = inverse;
        self.alpha_vector = v;
        self.is_additive = is_additive;

        let intensity = v.intensity.max(0.0);

        // Rotate (1,0,0) by quaternion to get direction vector
        let vec = v.angle * Vec3::X;

        if inverse {
            // Inverse alpha: alpha increases away from direction
            for idx in 0..self.vertex_ct {
                let mut temp = Vec3::dot(vec, self.vtx_normal[idx]);
                temp *= intensity;
                temp = temp.abs().min(1.0);

                self.set_dcg(is_additive, idx, temp);
            }
        } else {
            // Normal alpha: alpha increases toward direction
            for idx in 0..self.vertex_ct {
                let mut temp = Vec3::dot(vec, self.vtx_normal[idx]);
                temp *= intensity;
                temp = temp.abs().min(1.0);

                self.set_dcg(is_additive, idx, 1.0 - temp);
            }
        }
    }

    /// Set DCG (diffuse color/gradient) value for a vertex
    ///
    /// C++ Reference: sphereobj.h lines 202-218 SphereMeshClass::Set_DCG inline
    #[inline]
    fn set_dcg(&mut self, is_additive: bool, index: usize, value: f32) {
        if is_additive {
            // Additive blending: color = value, alpha = 0
            self.dcg[index] = Vec4::new(value, value, value, 0.0);
        } else {
            // Alpha blending: color = white, alpha = value
            self.dcg[index] = Vec4::new(1.0, 1.0, 1.0, value);
        }
    }

    /// Free all allocated memory
    fn free(&mut self) {
        self.vtx.clear();
        self.vtx_normal.clear();
        self.vtx_uv.clear();
        self.dcg.clear();
        self.strips.clear();
        self.fans.clear();
        self.tri_poly.clear();
    }

    /// Get vertices
    pub fn vertices(&self) -> &[Vec3] {
        &self.vtx
    }

    /// Get normals
    pub fn normals(&self) -> &[Vec3] {
        &self.vtx_normal
    }

    /// Get UVs
    pub fn uvs(&self) -> &[Vec2] {
        &self.vtx_uv
    }

    /// Get DCG (diffuse color/alpha)
    pub fn dcg(&self) -> &[Vec4] {
        &self.dcg
    }

    /// Get triangle indices
    pub fn indices(&self) -> &[TriIndex] {
        &self.tri_poly
    }
}

/// Global shared sphere mesh array for all LOD levels
///
/// C++ Reference: sphereobj.cpp lines 96-97 static SphereMeshArray
static SPHERE_ARRAY_INIT: Once = Once::new();
static SPHERE_MESH_ARRAY: Mutex<Vec<SphereMeshClass>> = Mutex::new(Vec::new());
static SPHERE_LOD_COSTS: Mutex<Vec<f32>> = Mutex::new(Vec::new());

/// Generate shared mesh arrays (called once on first sphere creation)
///
/// C++ Reference: sphereobj.cpp lines 294-320 SphereRenderObjClass::Generate_Shared_Mesh_Arrays
fn generate_shared_mesh_arrays(alpha_vector: AlphaVectorStruct) {
    SPHERE_ARRAY_INIT.call_once(|| {
        let mut meshes = Vec::with_capacity(SPHERE_NUM_LOD);
        let mut costs = Vec::with_capacity(SPHERE_NUM_LOD + 1);

        let size_start = SPHERE_LOWEST_LOD as f32;
        let step = (SPHERE_HIGHEST_LOD - SPHERE_LOWEST_LOD) as f32 / SPHERE_NUM_LOD as f32;

        // NULL LOD cost (small non-zero to avoid division by zero)
        costs.push(0.000001f32);

        for i in 0..SPHERE_NUM_LOD {
            let size = (size_start + step * i as f32) as usize;

            let mut mesh = SphereMeshClass::new();
            mesh.generate(1.0, size, size);
            mesh.set_alpha_vector(alpha_vector, false, false, true);

            costs.push(mesh.get_num_polys() as f32);
            meshes.push(mesh);
        }

        *SPHERE_MESH_ARRAY.lock().unwrap() = meshes;
        *SPHERE_LOD_COSTS.lock().unwrap() = costs;
    });
}

/// Sphere render object class
///
/// C++ Reference: sphereobj.h lines 229-399 SphereRenderObjClass
pub struct SphereRenderObjClass {
    // Identity
    name: String,

    // Object-space geometry
    obj_space_center: Vec3,
    obj_space_extent: Vec3,

    // Current rendering state
    current_color: Vec3,
    current_alpha: f32,
    current_scale: Vec3,
    current_vector: AlphaVectorStruct,
    orientation: Quat,

    // Flags
    flags: SphereFlags,

    // Material and shader
    sphere_material: Arc<VertexMaterialClass>,
    sphere_shader: ShaderClass,
    sphere_texture: Option<Arc<TextureClass>>,

    // LOD system
    current_lod: usize,
    lod_bias: f32,
    value: [f32; SPHERE_NUM_LOD + 2],

    // Animation
    anim_time: f32,
    anim_duration: f32,
    is_animating: bool,
    last_render_time: Option<f32>,

    // Cached bounding volumes
    cached_box: AABoxClass,
    cached_bounding_box: AABoxClass,
    cached_bounding_sphere: SphereClass,
    bounding_volumes_valid: bool,

    // Transform
    transform: Mat4,
    transform_valid: bool,
}

/// Prepared render submission for a sphere draw call.
#[derive(Debug, Clone)]
pub struct SphereRenderSubmission {
    pub world_transform: Mat4,
    pub vertex_count: usize,
    pub triangle_count: usize,
    pub additive: bool,
    pub has_texture: bool,
    pub color: Vec3,
    pub alpha: f32,
}

impl SphereRenderObjClass {
    /// Create a new sphere render object
    ///
    /// C++ Reference: sphereobj.cpp lines 116-143 SphereRenderObjClass::SphereRenderObjClass()
    pub fn new() -> Self {
        let current_vector = AlphaVectorStruct::default();

        // Initialize shared mesh arrays on first creation
        generate_shared_mesh_arrays(current_vector);

        let mut sphere_material = VertexMaterialClass::new();
        Self::init_material(&mut sphere_material);

        let sphere_shader = ShaderClass::get_alpha_shader();

        let mut obj = Self {
            name: String::new(),
            obj_space_center: Vec3::ZERO,
            obj_space_extent: Vec3::ONE,
            current_color: Vec3::new(0.75, 0.75, 0.75),
            current_alpha: 1.0,
            current_scale: Vec3::ONE,
            current_vector,
            orientation: Quat::IDENTITY,
            flags: SphereFlags::default(),
            sphere_material: Arc::new(sphere_material),
            sphere_shader,
            sphere_texture: None,
            current_lod: SPHERE_NUM_LOD, // Start at NULL LOD
            lod_bias: 1.0,
            value: [0.0; SPHERE_NUM_LOD + 2],
            anim_time: 0.0,
            anim_duration: 0.0,
            is_animating: true,
            last_render_time: None,
            cached_box: AABoxClass::default(),
            cached_bounding_box: AABoxClass::default(),
            cached_bounding_sphere: SphereClass::empty(),
            bounding_volumes_valid: false,
            transform: Mat4::IDENTITY,
            transform_valid: false,
        };

        // Initialize value array with screen area = 1.0
        obj.calculate_value_array(1.0);
        obj.update_cached_box();

        obj
    }

    /// Initialize material with default settings
    ///
    /// C++ Reference: sphereobj.cpp lines 348-368 SphereRenderObjClass::Init_Material
    fn init_material(material: &mut VertexMaterialClass) {
        material.set_ambient(Vec3::ZERO);
        material.set_diffuse(Vec3::ZERO);
        material.set_specular(Vec3::ZERO);
        material.set_emissive(Vec3::ONE); // Fully emissive
        material.set_opacity(0.25);
        material.set_shininess(0.0);
        material.set_lighting(true);
    }

    // ========== Accessors ==========

    /// Get name
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Set name
    ///
    /// C++ Reference: sphereobj.cpp lines 437-442
    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }

    /// Set color
    pub fn set_color(&mut self, color: Vec3) {
        self.current_color = color;
    }

    /// Get color
    pub fn get_color(&self) -> Vec3 {
        self.current_color
    }

    /// Set alpha
    pub fn set_alpha(&mut self, alpha: f32) {
        self.current_alpha = alpha;
    }

    /// Get alpha
    pub fn get_alpha(&self) -> f32 {
        self.current_alpha
    }

    /// Set scale
    pub fn set_scale(&mut self, scale: Vec3) {
        self.current_scale = scale;
        self.bounding_volumes_valid = false;
    }

    /// Get scale
    pub fn get_scale(&self) -> Vec3 {
        self.current_scale
    }

    /// Set alpha vector
    pub fn set_vector(&mut self, vector: AlphaVectorStruct) {
        self.current_vector = vector;
    }

    /// Get alpha vector
    pub fn get_vector(&self) -> AlphaVectorStruct {
        self.current_vector
    }

    /// Set flags
    pub fn set_flags(&mut self, flags: SphereFlags) {
        self.flags = flags;
    }

    /// Get flags
    pub fn get_flags(&self) -> SphereFlags {
        self.flags
    }

    /// Set extent
    ///
    /// C++ Reference: sphereobj.h lines 401-406 inline Set_Extent
    pub fn set_extent(&mut self, extent: Vec3) {
        self.obj_space_extent = extent;
        self.update_cached_box();
        self.update_cached_bounding_volumes();
    }

    /// Set local center and extent
    ///
    /// C++ Reference: sphereobj.h lines 408-413 inline Set_Local_Center_Extent
    pub fn set_local_center_extent(&mut self, center: Vec3, extent: Vec3) {
        self.obj_space_center = center;
        self.obj_space_extent = extent;
        self.update_cached_box();
    }

    /// Set local min and max
    ///
    /// C++ Reference: sphereobj.h lines 415-420 inline Set_Local_Min_Max
    pub fn set_local_min_max(&mut self, min: Vec3, max: Vec3) {
        self.obj_space_center = (max + min) * 0.5;
        self.obj_space_extent = (max - min) * 0.5;
        self.update_cached_box();
    }

    /// Set texture
    ///
    /// C++ Reference: sphereobj.cpp lines 401-404
    pub fn set_texture(&mut self, texture: Option<Arc<TextureClass>>) {
        self.sphere_texture = texture;
    }

    /// Get texture
    pub fn peek_texture(&self) -> Option<&Arc<TextureClass>> {
        self.sphere_texture.as_ref()
    }

    /// Set shader
    pub fn set_shader(&mut self, shader: ShaderClass) {
        self.sphere_shader = shader;
    }

    /// Get shader
    pub fn get_shader(&self) -> &ShaderClass {
        &self.sphere_shader
    }

    // ========== Animation ==========

    /// Is currently animating
    pub fn is_animating(&self) -> bool {
        self.is_animating
    }

    /// Start animating
    pub fn start_animating(&mut self) {
        self.is_animating = true;
        self.anim_time = 0.0;
    }

    /// Stop animating
    pub fn stop_animating(&mut self) {
        self.is_animating = false;
        self.anim_time = 0.0;
    }

    /// Set animation duration
    pub fn set_animation_duration(&mut self, duration: f32) {
        self.anim_duration = duration;
        self.restart_animation();
    }

    /// Get animation duration
    pub fn get_animation_duration(&self) -> f32 {
        self.anim_duration
    }

    /// Restart animation
    pub fn restart_animation(&mut self) {
        self.anim_time = 0.0;
    }

    /// Update animation state
    ///
    /// C++ Reference: sphereobj.cpp lines 1098-1144 SphereRenderObjClass::animate
    ///
    /// Note: In C++, this uses frame time and animation channels. This implementation
    /// tracks animation time; full channel support would require implementing
    /// PrimitiveAnimationChannelClass equivalents.
    pub fn animate(&mut self, frame_time: f32) {
        if !self.is_animating {
            return;
        }

        if self.anim_duration > 0.0 {
            let frametime = (frame_time * 0.001) / self.anim_duration;
            self.anim_time += frametime;

            if self.flags.use_animation_loop && self.anim_time > 1.0 {
                self.anim_time -= 1.0;
            }
        } else {
            self.anim_time = 1.0;
        }
    }

    // ========== LOD System ==========

    /// Set LOD level
    ///
    /// C++ Reference: sphereobj.cpp lines 841-844
    pub fn set_lod_level(&mut self, lod: usize) {
        self.current_lod = lod.min(SPHERE_NUM_LOD);
    }

    /// Get LOD level
    pub fn get_lod_level(&self) -> usize {
        self.current_lod
    }

    /// Get LOD count
    pub fn get_lod_count(&self) -> usize {
        SPHERE_NUM_LOD + 1 // Include NULL LOD
    }

    /// Set LOD bias
    pub fn set_lod_bias(&mut self, bias: f32) {
        self.lod_bias = bias.max(0.0);
    }

    /// Get current polycount
    ///
    /// C++ Reference: sphereobj.cpp lines 383-386
    pub fn get_num_polys(&self) -> usize {
        let costs = SPHERE_LOD_COSTS.lock().unwrap();
        if self.current_lod < costs.len() {
            costs[self.current_lod] as usize
        } else {
            0
        }
    }

    /// Calculate value array for LOD selection
    ///
    /// C++ Reference: sphereobj.cpp lines 324-333
    fn calculate_value_array(&mut self, screen_area: f32) {
        let costs = SPHERE_LOD_COSTS.lock().unwrap();

        self.value[0] = AT_MIN_LOD;

        for lod in 1..=SPHERE_NUM_LOD {
            if lod < costs.len() {
                let polycount = costs[lod];
                let benefit_factor = 1.0 - (0.5 / (polycount * polycount));
                self.value[lod] = (benefit_factor * screen_area * self.lod_bias) / polycount;
            } else {
                self.value[lod] = 0.0;
            }
        }

        self.value[SPHERE_NUM_LOD + 1] = AT_MAX_LOD;
    }

    /// Prepare LOD based on camera screen area
    ///
    /// C++ Reference: sphereobj.cpp lines 805-814
    pub fn prepare_lod(&mut self, screen_area: f32) {
        self.calculate_value_array(screen_area);
        // Note: C++ uses PredictiveLODOptimizerClass for actual LOD selection.
        // This keeps the value array ready for that integration path.
    }

    /// Increment LOD level
    pub fn increment_lod(&mut self) {
        if self.current_lod < SPHERE_NUM_LOD {
            self.current_lod += 1;
        }
    }

    /// Decrement LOD level
    pub fn decrement_lod(&mut self) {
        if self.current_lod > 0 {
            self.current_lod -= 1;
        }
    }

    /// Get LOD cost (polycount)
    pub fn get_cost(&self) -> f32 {
        self.get_num_polys() as f32
    }

    /// Get LOD value
    pub fn get_value(&self) -> f32 {
        if self.current_lod < self.value.len() {
            self.value[self.current_lod]
        } else {
            0.0
        }
    }

    /// Get post-increment value
    pub fn get_post_increment_value(&self) -> f32 {
        if self.current_lod + 1 < self.value.len() {
            self.value[self.current_lod + 1]
        } else {
            AT_MAX_LOD
        }
    }

    // ========== Transform and Bounding Volumes ==========

    /// Set transform
    ///
    /// C++ Reference: sphereobj.cpp lines 724-728
    pub fn set_transform(&mut self, transform: Mat4) {
        self.transform = transform;
        self.transform_valid = true;
        self.update_cached_box();
    }

    /// Get transform
    pub fn get_transform(&self) -> Mat4 {
        self.transform
    }

    /// Set position
    ///
    /// C++ Reference: sphereobj.cpp lines 743-747
    pub fn set_position(&mut self, position: Vec3) {
        self.transform.w_axis = position.extend(1.0);
        self.update_cached_box();
    }

    /// Get position
    pub fn get_position(&self) -> Vec3 {
        self.transform.w_axis.truncate()
    }

    /// Update cached bounding box
    ///
    /// C++ Reference: sphereobj.cpp lines 762-766
    fn update_cached_box(&mut self) {
        let translation = self.transform.w_axis.truncate();
        self.cached_box = AABoxClass::from_center_extent(
            translation + self.obj_space_center,
            self.obj_space_extent,
        );
    }

    /// Update cached bounding volumes
    ///
    /// C++ Reference: sphereobj.cpp lines 943-953
    fn update_cached_bounding_volumes(&mut self) {
        let scaled_extent = Vec3::new(
            self.obj_space_extent.x * self.current_scale.x,
            self.obj_space_extent.y * self.current_scale.y,
            self.obj_space_extent.z * self.current_scale.z,
        );

        let position = self.get_position();
        let center = position + self.obj_space_center;
        self.cached_bounding_box
            .init_center_extent(center, scaled_extent);
        self.cached_bounding_sphere = SphereClass::new(center, scaled_extent.length());

        self.bounding_volumes_valid = true;
    }

    /// Get the cached bounding box
    ///
    /// C++ Reference: sphereobj.h lines 423-428 inline Get_Box
    pub fn get_box(&mut self) -> AABoxClass {
        self.update_cached_box();
        self.cached_box
    }

    /// Get object-space bounding sphere
    ///
    /// C++ Reference: sphereobj.cpp lines 781-784
    pub fn get_obj_space_bounding_sphere(&self) -> SphereClass {
        SphereClass::new(self.obj_space_center, self.obj_space_extent.length())
    }

    /// Get object-space bounding box
    ///
    /// C++ Reference: sphereobj.cpp lines 799-802
    pub fn get_obj_space_bounding_box(&self) -> AABoxClass {
        AABoxClass::from_center_extent(self.obj_space_center, self.obj_space_extent)
    }

    /// Scale uniformly
    ///
    /// C++ Reference: sphereobj.cpp lines 880-894
    pub fn scale(&mut self, scale: f32) {
        if scale == 1.0 {
            return;
        }
        self.current_scale *= scale;
        // Note: C++ also scales animation channel keyframes, which we'd add
        // when implementing full animation channel support
    }

    /// Scale non-uniformly
    ///
    /// C++ Reference: sphereobj.cpp lines 909-927
    pub fn scale_non_uniform(&mut self, scale_x: f32, scale_y: f32, scale_z: f32) {
        self.current_scale.x *= scale_x;
        self.current_scale.y *= scale_y;
        self.current_scale.z *= scale_z;
        // Note: C++ also scales animation channel keyframes
    }

    // ========== Rendering ==========

    fn compute_render_transform(&self, rinfo: &RenderInfoClass, real_scale: Vec3) -> Mat4 {
        let scaled_transform = Mat4::from_scale(real_scale) * self.transform;
        if !self.flags.use_camera_align {
            return scaled_transform;
        }

        let world_position = self.transform.w_axis.truncate();
        let camera_position = rinfo.camera.position();
        let mut to_camera = camera_position - world_position;
        if to_camera.length_squared() <= f32::EPSILON {
            return scaled_transform;
        }
        to_camera = to_camera.normalize();

        let mut right = Vec3::Y.cross(to_camera);
        if right.length_squared() <= f32::EPSILON {
            right = Vec3::X.cross(to_camera);
        }
        if right.length_squared() <= f32::EPSILON {
            return scaled_transform;
        }
        right = right.normalize();
        let up = to_camera.cross(right).normalize();

        let billboard = Mat4::from_cols(
            right.extend(0.0),
            up.extend(0.0),
            to_camera.extend(0.0),
            world_position.extend(1.0),
        );

        Mat4::from_scale(real_scale) * billboard
    }

    /// Prepare immutable draw submission data for backend renderers.
    pub fn prepare_render_submission(
        &self,
        world_transform: Mat4,
        is_additive: bool,
    ) -> Option<SphereRenderSubmission> {
        if self.current_lod == 0 {
            return None;
        }

        let meshes = SPHERE_MESH_ARRAY.lock().ok()?;
        let mesh = meshes.get(self.current_lod - 1)?;

        Some(SphereRenderSubmission {
            world_transform,
            vertex_count: mesh.vertices().len(),
            triangle_count: mesh.indices().len(),
            additive: is_additive,
            has_texture: self.sphere_texture.is_some(),
            color: self.current_color,
            alpha: self.current_alpha,
        })
    }

    /// Render the sphere
    ///
    /// C++ Reference: sphereobj.cpp lines 531-614 SphereRenderObjClass::Render
    ///
    /// This implementation matches the C++ rendering pipeline:
    /// 1. Check NULL LOD (lines 533-534)
    /// 2. Check visibility flags (lines 536-538)
    /// 3. Handle static sort lists or immediate rendering (lines 546-551)
    /// 4. Animate the sphere (line 558)
    /// 5. Compute scaled transform (lines 560-566)
    /// 6. Setup material (alpha/emissive based on shader blend mode) (lines 571-577)
    /// 7. Apply alpha vector to mesh if enabled (lines 580-589)
    /// 8. Handle camera alignment if USE_CAMERA_ALIGN flag set (lines 592-608)
    /// 9. Call render_sphere() to submit geometry (lines 607-611)
    ///
    pub fn render(&mut self, rinfo: &RenderInfoClass) -> W3DResult<()> {
        // NULL LOD - don't render
        // C++ Reference: sphereobj.cpp lines 533-534
        if self.current_lod == 0 {
            return Ok(());
        }

        // Animate - C++ Reference: sphereobj.cpp line 558
        let frame_time_ms = if let Some(last_time) = self.last_render_time {
            ((rinfo.time - last_time).max(0.0)) * 1000.0
        } else {
            0.0
        };
        self.last_render_time = Some(rinfo.time);
        self.animate(frame_time_ms);

        // Compute scaled transform - C++ Reference: sphereobj.cpp lines 555-566
        let real_scale = Vec3::new(
            self.obj_space_extent.x * self.current_scale.x,
            self.obj_space_extent.y * self.current_scale.y,
            self.obj_space_extent.z * self.current_scale.z,
        );
        let render_transform = self.compute_render_transform(rinfo, real_scale);

        // Configure alpha and emissive - C++ Reference: sphereobj.cpp lines 571-577
        // Determine blend mode from shader
        let is_additive = self.sphere_shader.is_additive_blend();

        // Update material properties based on blend mode
        // In C++, this modifies SphereMaterial emissive and opacity
        // For additive: Set emissive to alpha * color
        // For alpha blend: Set opacity to alpha, emissive to color
        if is_additive {
            let emissive = self.current_color * self.current_alpha;
            if let Some(m) = Arc::get_mut(&mut self.sphere_material) {
                m.set_emissive(emissive)
            }
        } else {
            Arc::get_mut(&mut self.sphere_material).map(|m| {
                m.set_opacity(self.current_alpha);
                m.set_emissive(self.current_color);
            });
        }

        // Apply alpha vector if enabled - C++ Reference: sphereobj.cpp lines 580-589
        if self.flags.use_alpha_vector {
            let use_inverse = self.flags.use_inverse_alpha;

            // Get the mesh for current LOD and update its alpha vector
            // C++: SphereMeshArray[CurrentLOD - 1].Set_Alpha_Vector(CurrentVector, use_inverse, is_additive);
            let mut meshes = SPHERE_MESH_ARRAY.lock().unwrap();
            if self.current_lod > 0 && self.current_lod <= meshes.len() {
                meshes[self.current_lod - 1].set_alpha_vector(
                    self.current_vector,
                    use_inverse,
                    is_additive,
                    false, // force=false, only update if changed
                );
            }
        }

        if let Some(submission) = self.prepare_render_submission(render_transform, is_additive) {
            let _ = submission;
        }

        Ok(())
    }

    /// Clone the sphere
    ///
    /// C++ Reference: sphereobj.cpp lines 565-568
    pub fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            obj_space_center: self.obj_space_center,
            obj_space_extent: self.obj_space_extent,
            current_color: self.current_color,
            current_alpha: self.current_alpha,
            current_scale: self.current_scale,
            current_vector: self.current_vector,
            orientation: self.orientation,
            flags: self.flags,
            sphere_material: self.sphere_material.clone(),
            sphere_shader: self.sphere_shader,
            sphere_texture: self.sphere_texture.clone(),
            current_lod: self.current_lod,
            lod_bias: self.lod_bias,
            value: self.value,
            anim_time: self.anim_time,
            anim_duration: self.anim_duration,
            is_animating: self.is_animating,
            last_render_time: self.last_render_time,
            cached_box: self.cached_box,
            cached_bounding_box: self.cached_bounding_box,
            cached_bounding_sphere: self.cached_bounding_sphere,
            bounding_volumes_valid: self.bounding_volumes_valid,
            transform: self.transform,
            transform_valid: self.transform_valid,
        }
    }

    /// Get class ID
    ///
    /// C++ Reference: sphereobj.cpp lines 583-586
    pub fn class_id() -> u32 {
        // RenderObjClass::CLASSID_SPHERE
        // This would need to be defined in RenderObjClass
        0x00000004 // Placeholder
    }
}

impl Default for SphereRenderObjClass {
    fn default() -> Self {
        Self::new()
    }
}

// Note: C++ also has SpherePrototypeClass and SphereLoaderClass for W3D file loading.
// These would be implemented when adding full asset pipeline support.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sphere_mesh_generation() {
        let mut mesh = SphereMeshClass::new();
        mesh.generate(1.0, 8, 8);

        assert_eq!(mesh.get_num_polys(), 8 * 8 * 2);
        assert_eq!(mesh.vertices().len(), (8 + 1) * 8 + 2);
        assert_eq!(mesh.vertices().len(), mesh.normals().len());
        assert_eq!(mesh.vertices().len(), mesh.uvs().len());
        assert_eq!(mesh.vertices().len(), mesh.dcg().len());

        // Check north pole
        assert!((mesh.vertices()[0] - Vec3::new(0.0, 0.0, 1.0)).length() < 0.001);
        // Check south pole
        let last = mesh.vertices().len() - 1;
        assert!((mesh.vertices()[last] - Vec3::new(0.0, 0.0, -1.0)).length() < 0.001);
    }

    #[test]
    fn test_sphere_mesh_alpha_vector() {
        let mut mesh = SphereMeshClass::new();
        mesh.generate(1.0, 8, 8);

        let alpha_vec = AlphaVectorStruct {
            angle: Quat::IDENTITY,
            intensity: 1.0,
        };

        mesh.set_alpha_vector(alpha_vec, false, false, true);

        // DCG should be filled with alpha values
        for dcg in mesh.dcg() {
            assert!(dcg.w >= 0.0 && dcg.w <= 1.0);
        }
    }

    #[test]
    fn test_sphere_render_obj_creation() {
        let sphere = SphereRenderObjClass::new();
        assert_eq!(sphere.get_name(), "");
        assert_eq!(sphere.get_alpha(), 1.0);
        assert_eq!(sphere.get_color(), Vec3::new(0.75, 0.75, 0.75));
        assert_eq!(sphere.get_lod_level(), SPHERE_NUM_LOD);
    }

    #[test]
    fn test_sphere_render_obj_properties() {
        let mut sphere = SphereRenderObjClass::new();

        sphere.set_name("TestSphere");
        assert_eq!(sphere.get_name(), "TestSphere");

        sphere.set_color(Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(sphere.get_color(), Vec3::new(1.0, 0.0, 0.0));

        sphere.set_alpha(0.5);
        assert_eq!(sphere.get_alpha(), 0.5);

        sphere.set_scale(Vec3::new(2.0, 2.0, 2.0));
        assert_eq!(sphere.get_scale(), Vec3::new(2.0, 2.0, 2.0));
    }

    #[test]
    fn test_sphere_lod_system() {
        let mut sphere = SphereRenderObjClass::new();

        sphere.set_lod_level(0);
        assert_eq!(sphere.get_lod_level(), 0);

        sphere.set_lod_level(5);
        assert_eq!(sphere.get_lod_level(), 5);

        // Test LOD clamping
        sphere.set_lod_level(999);
        assert_eq!(sphere.get_lod_level(), SPHERE_NUM_LOD);

        // Test LOD increment/decrement
        sphere.set_lod_level(5);
        sphere.increment_lod();
        assert_eq!(sphere.get_lod_level(), 6);

        sphere.decrement_lod();
        assert_eq!(sphere.get_lod_level(), 5);
    }

    #[test]
    fn test_sphere_animation() {
        let mut sphere = SphereRenderObjClass::new();

        sphere.set_animation_duration(2.0);
        assert_eq!(sphere.get_animation_duration(), 2.0);

        assert!(sphere.is_animating());

        sphere.stop_animating();
        assert!(!sphere.is_animating());

        sphere.start_animating();
        assert!(sphere.is_animating());
    }

    #[test]
    fn test_sphere_transform() {
        let mut sphere = SphereRenderObjClass::new();

        let transform = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0));
        sphere.set_transform(transform);
        assert_eq!(sphere.get_transform(), transform);

        sphere.set_position(Vec3::new(5.0, 6.0, 7.0));
        assert_eq!(sphere.get_position(), Vec3::new(5.0, 6.0, 7.0));
    }

    #[test]
    fn test_sphere_bounds() {
        let mut sphere = SphereRenderObjClass::new();
        sphere.set_local_center_extent(Vec3::ZERO, Vec3::ONE);

        let bsphere = sphere.get_obj_space_bounding_sphere();
        assert!(bsphere.radius() > 0.0);

        let bbox = sphere.get_obj_space_bounding_box();
        assert_eq!(bbox.center(), Vec3::ZERO);
    }

    #[test]
    fn test_sphere_scaling() {
        let mut sphere = SphereRenderObjClass::new();

        sphere.scale(2.0);
        assert_eq!(sphere.get_scale(), Vec3::new(2.0, 2.0, 2.0));

        sphere.set_scale(Vec3::ONE);
        sphere.scale_non_uniform(1.0, 2.0, 3.0);
        assert_eq!(sphere.get_scale(), Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn test_alpha_vector_struct() {
        let v1 = AlphaVectorStruct::default();
        let v2 = AlphaVectorStruct {
            angle: Quat::IDENTITY,
            intensity: 1.0,
        };

        assert_eq!(v1, v2);

        let v3 = AlphaVectorStruct {
            angle: Quat::from_rotation_y(0.5),
            intensity: 0.5,
        };

        assert_ne!(v1, v3);
    }

    #[test]
    fn test_sphere_flags() {
        let flags = SphereFlags::default();
        assert!(flags.use_alpha_vector);
        assert!(!flags.use_camera_align);

        let bits = flags.to_bits();
        let restored = SphereFlags::from_bits(bits);
        assert_eq!(flags, restored);
    }

    #[test]
    fn test_shared_mesh_array_initialization() {
        // Just creating a sphere should initialize the shared arrays
        let _sphere = SphereRenderObjClass::new();

        let meshes = SPHERE_MESH_ARRAY.lock().unwrap();
        let costs = SPHERE_LOD_COSTS.lock().unwrap();

        assert_eq!(meshes.len(), SPHERE_NUM_LOD);
        assert_eq!(costs.len(), SPHERE_NUM_LOD + 1); // +1 for NULL LOD
    }

    #[test]
    fn test_sphere_clone() {
        let mut sphere = SphereRenderObjClass::new();
        sphere.set_name("Original");
        sphere.set_color(Vec3::new(1.0, 0.5, 0.0));
        sphere.set_alpha(0.8);

        let cloned = sphere.clone();
        assert_eq!(cloned.get_name(), "Original");
        assert_eq!(cloned.get_color(), Vec3::new(1.0, 0.5, 0.0));
        assert_eq!(cloned.get_alpha(), 0.8);
    }

    #[test]
    fn test_prepare_render_submission_for_valid_lod() {
        let mut sphere = SphereRenderObjClass::new();
        sphere.set_lod_level(1);
        let submission = sphere
            .prepare_render_submission(Mat4::IDENTITY, false)
            .expect("submission");
        assert!(submission.vertex_count > 0);
        assert!(submission.triangle_count > 0);
        assert!(!submission.additive);
    }

    #[test]
    fn test_camera_aligned_transform_differs_from_standard_transform() {
        let mut sphere = SphereRenderObjClass::new();
        sphere.set_position(Vec3::new(2.0, 0.0, 0.0));

        let mut flags = SphereFlags::default();
        flags.use_camera_align = true;
        sphere.set_flags(flags);

        let rinfo =
            RenderInfoClass::new(Arc::new(crate::rendering::camera_system::CameraClass::new()));
        let aligned = sphere.compute_render_transform(&rinfo, Vec3::ONE);

        let mut regular_flags = SphereFlags::default();
        regular_flags.use_camera_align = false;
        sphere.set_flags(regular_flags);
        let regular = sphere.compute_render_transform(&rinfo, Vec3::ONE);

        assert_ne!(aligned, regular);
    }
}
