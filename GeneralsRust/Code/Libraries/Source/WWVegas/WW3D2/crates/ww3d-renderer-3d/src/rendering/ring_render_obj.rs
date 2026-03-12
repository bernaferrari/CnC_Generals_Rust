//! Ring Render Objects - Procedurally generated ring primitives
//!
//! This module provides RingRenderObjClass from C++ WW3D2, used for procedural ring effects
//! like explosions, energy shields, magic circles, and area-of-effect visualizations.
//!
//! # C++ Reference
//! - Original: `/GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/ringobj.cpp`
//! - Header: `/GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/ringobj.h`
//!
//! # Architecture
//!
//! The ring system consists of:
//! - **RingRenderObjClass**: Main ring object with color, alpha, scale, animation
//! - **RingMeshClass**: Shared geometry at multiple LOD levels (10-50 segments)
//! - **Animation Channels**: LERP-based keyframe animation for color, alpha, scale
//!
//! # Ring Geometry
//!
//! Rings are defined by two concentric circles in the XY plane:
//! - **Inner circle**: radius = inner_extent * inner_scale
//! - **Outer circle**: radius = outer_extent * outer_scale
//! - **Segments**: N divisions around the circle (affects detail/LOD)
//! - **Vertices**: 2 per segment (inner + outer) forming a triangle strip
//!
//! Geometry layout:
//! ```text
//!     Outer circle (radius = outer_extent * outer_scale)
//!        ___
//!      /     \
//!     |  ___  |  <- Inner circle (radius = inner_extent * inner_scale)
//!     | /   \ |
//!     | \___/ |
//!      \_____/
//!
//! Vertices alternate: inner[0], outer[0], inner[1], outer[1], ...
//! Triangles: (i, i+1, i+2) for each segment
//! ```

use crate::bounding_volumes::aabox::AABoxClass;
use glam::{Mat4, Vec2, Vec3};
use std::f32::consts::PI;
use ww3d_collision::bounding_volumes::sphere::SphereClass;
use ww3d_geometry::primitive_animation::{AnimationChannel, LERPAnimationChannel};

/// Number of LOD levels (excluding NULL LOD)
/// C++ Reference: ringobj.h line 96
const RING_NUM_LOD: usize = 20;

/// Lowest LOD segment count
/// C++ Reference: ringobj.h line 97
const RING_LOWEST_LOD: usize = 10;

/// Highest LOD segment count
/// C++ Reference: ringobj.h line 98
const RING_HIGHEST_LOD: usize = 50;

/// Maximum name length (matches W3D_NAME_LEN * 2)
/// C++ Reference: ringobj.h line 70
const MAX_NAME_LEN: usize = 32;

/// Default texture tiling count
/// C++ Reference: ringobj.cpp line 181
const DEFAULT_TEXTURE_TILE_COUNT: i32 = 5;

/// Ring flags bitfield
/// C++ Reference: ringobj.h lines 110-113
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RingFlags(u32);

impl RingFlags {
    /// Use camera alignment (billboard towards camera)
    pub const USE_CAMERA_ALIGN: u32 = 0x00000001;
    /// Loop animation instead of playing once
    pub const USE_ANIMATION_LOOP: u32 = 0x00000002;

    pub fn new() -> Self {
        Self(0)
    }

    pub fn with_flag(mut self, flag: u32) -> Self {
        self.0 |= flag;
        self
    }

    pub fn has_flag(&self, flag: u32) -> bool {
        (self.0 & flag) != 0
    }

    pub fn set_flag(&mut self, flag: u32, enable: bool) {
        if enable {
            self.0 |= flag;
        } else {
            self.0 &= !flag;
        }
    }

    pub fn bits(&self) -> u32 {
        self.0
    }
}

impl Default for RingFlags {
    fn default() -> Self {
        Self::new()
    }
}

/// Triangle index for mesh geometry
/// C++ Reference: ringobj.cpp line 1595-1597
#[derive(Debug, Clone, Copy)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct TriIndex {
    i: u16,
    j: u16,
    k: u16,
}

/// Shared ring mesh geometry at a specific LOD level
///
/// All RingRenderObj instances share these pre-generated meshes.
/// C++ Reference: ringobj.cpp lines 106-143 (RingMeshClass)
#[derive(Clone)]
struct RingMesh {
    /// Radius of the unit ring
    #[cfg_attr(not(test), allow(dead_code))]
    radius: f32,
    /// Number of segments around the ring
    slices: usize,
    /// Number of vertices (slices * 2 + 2)
    vertex_count: usize,
    /// Number of triangles (slices * 2)
    face_count: usize,
    /// Current texture tiling count
    tile_count: i32,
    /// Current inner scale
    inner_scale: Vec2,
    /// Current outer scale
    outer_scale: Vec2,

    /// Vertex positions (world space, after scaling)
    vertices: Vec<Vec3>,
    /// Original unit-circle vertex positions (2D)
    orig_vertices: Vec<Vec2>,
    /// Vertex normals (point up from ring plane)
    normals: Vec<Vec3>,
    /// Texture coordinates
    uvs: Vec<Vec2>,
    /// Triangle indices
    triangles: Vec<TriIndex>,
}

impl RingMesh {
    /// Create a new ring mesh with specified radius and segment count
    /// C++ Reference: ringobj.cpp lines 1531-1601 (RingMeshClass::Generate)
    fn new(radius: f32, slices: usize) -> Self {
        let vertex_count = slices * 2 + 2;
        let face_count = slices * 2;

        let mut mesh = Self {
            radius,
            slices,
            vertex_count,
            face_count,
            tile_count: DEFAULT_TEXTURE_TILE_COUNT,
            inner_scale: Vec2::ONE,
            outer_scale: Vec2::ONE,
            vertices: vec![Vec3::ZERO; vertex_count],
            orig_vertices: vec![Vec2::ZERO; vertex_count],
            normals: vec![Vec3::Z; vertex_count],
            uvs: vec![Vec2::ZERO; vertex_count],
            triangles: vec![TriIndex { i: 0, j: 0, k: 0 }; face_count],
        };

        mesh.generate();
        mesh
    }

    /// Generate ring geometry
    /// C++ Reference: ringobj.cpp lines 1531-1601
    fn generate(&mut self) {
        let angle_inc = (2.0 * PI) / (self.slices as f32);
        let u_inc = (self.tile_count as f32) / (self.slices as f32);

        let mut angle = 0.0f32;
        let mut u_value = 0.0f32;

        // Generate vertices in pairs (inner, outer) around the ring
        for i in (0..self.vertex_count).step_by(2) {
            let x_pos = -angle.sin();
            let y_pos = angle.cos();

            // Inner vertex
            self.orig_vertices[i] = Vec2::new(x_pos, y_pos);
            self.vertices[i] = Vec3::new(x_pos, y_pos, 0.0);
            self.normals[i] = Vec3::Z;
            self.uvs[i] = Vec2::new(u_value, 0.0);

            // Outer vertex
            if i + 1 < self.vertex_count {
                self.orig_vertices[i + 1] = Vec2::new(x_pos, y_pos);
                self.vertices[i + 1] = Vec3::new(x_pos, y_pos, 0.0);
                self.normals[i + 1] = Vec3::Z;
                self.uvs[i + 1] = Vec2::new(u_value, 1.0);
            }

            angle += angle_inc;
            u_value += u_inc;
        }

        // Generate triangle indices
        // C++ Reference: ringobj.cpp lines 1594-1598
        for i in 0..self.face_count {
            self.triangles[i] = TriIndex {
                i: i as u16,
                j: (i + 1) as u16,
                k: (i + 2) as u16,
            };
        }
    }

    /// Scale the ring mesh to match inner and outer extents
    /// C++ Reference: ringobj.cpp lines 1482-1516 (RingMeshClass::Scale)
    fn scale(&mut self, inner_scale: Vec2, outer_scale: Vec2) {
        let do_inner = inner_scale != self.inner_scale;
        let do_outer = outer_scale != self.outer_scale;

        // Only update vertices that need scaling (optimization)
        if do_inner {
            for i in (0..self.vertex_count).step_by(2) {
                self.vertices[i] = Vec3::new(
                    self.orig_vertices[i].x * inner_scale.x,
                    self.orig_vertices[i].y * inner_scale.y,
                    0.0,
                );
            }
            self.inner_scale = inner_scale;
        }

        if do_outer {
            for i in (1..self.vertex_count).step_by(2) {
                self.vertices[i] = Vec3::new(
                    self.orig_vertices[i].x * outer_scale.x,
                    self.orig_vertices[i].y * outer_scale.y,
                    0.0,
                );
            }
            self.outer_scale = outer_scale;
        }
    }

    /// Update texture tiling
    /// C++ Reference: ringobj.cpp lines 1457-1479 (RingMeshClass::Set_Tiling)
    fn set_tiling(&mut self, count: i32) {
        if self.tile_count == count {
            return;
        }

        self.tile_count = count;
        let u_inc = (count as f32) / (self.slices as f32);
        let mut u_value = 0.0f32;

        // Update UV coordinates for new tiling
        for i in (0..self.vertex_count).step_by(2) {
            self.uvs[i] = Vec2::new(u_value, 0.0);
            if i + 1 < self.vertex_count {
                self.uvs[i + 1] = Vec2::new(u_value, 1.0);
            }
            u_value += u_inc;
        }
    }

    /// Get polygon count for this LOD
    fn get_num_polys(&self) -> usize {
        self.face_count
    }
}

/// Shared ring mesh array for all LOD levels
/// C++ Reference: ringobj.cpp line 146 (RingMeshArray)
struct RingMeshArray {
    meshes: Vec<RingMesh>,
    costs: Vec<f32>,
}

impl RingMeshArray {
    /// Generate all LOD levels
    /// C++ Reference: ringobj.cpp lines 367-390 (Generate_Shared_Mesh_Arrays)
    fn new() -> Self {
        let mut meshes = Vec::with_capacity(RING_NUM_LOD);
        let mut costs = Vec::with_capacity(RING_NUM_LOD + 1);

        // NULL LOD cost (very small to avoid division by zero)
        costs.push(0.000001f32);

        let step = (RING_HIGHEST_LOD - RING_LOWEST_LOD) as f32 / RING_NUM_LOD as f32;
        let mut size = RING_LOWEST_LOD as f32;

        for _ in 0..RING_NUM_LOD {
            let mesh = RingMesh::new(1.0, size as usize);
            costs.push(mesh.get_num_polys() as f32);
            meshes.push(mesh);
            size += step;
        }

        Self { meshes, costs }
    }

    fn get_mesh(&self, lod: usize) -> Option<&RingMesh> {
        if lod > 0 && lod <= RING_NUM_LOD {
            self.meshes.get(lod - 1)
        } else {
            None
        }
    }

    fn get_mesh_mut(&mut self, lod: usize) -> Option<&mut RingMesh> {
        if lod > 0 && lod <= RING_NUM_LOD {
            self.meshes.get_mut(lod - 1)
        } else {
            None
        }
    }

    fn get_cost(&self, lod: usize) -> f32 {
        self.costs.get(lod).copied().unwrap_or(0.0)
    }
}

// Thread-local storage for shared mesh arrays
// C++ used global static arrays, we use thread-local for safety
thread_local! {
    static RING_MESH_ARRAY: std::cell::RefCell<Option<RingMeshArray>> = std::cell::RefCell::new(None);
}

/// Initialize shared ring mesh arrays
fn ensure_mesh_array_initialized() {
    RING_MESH_ARRAY.with(|array| {
        let mut array = array.borrow_mut();
        if array.is_none() {
            *array = Some(RingMeshArray::new());
        }
    });
}

/// Main ring render object class
///
/// Provides procedurally generated ring primitives with animation support.
/// C++ Reference: ringobj.h lines 104-282 (RingRenderObjClass)
#[derive(Clone)]
pub struct RingRenderObj {
    /// Name of the ring
    name: String,

    /// Object-space center
    obj_space_center: Vec3,
    /// Object-space extent (bounding box half-sizes)
    obj_space_extent: Vec3,

    /// Inner extent (ring hole radius in X/Y)
    inner_extent: Vec2,
    /// Outer extent (ring outer radius in X/Y)
    outer_extent: Vec2,

    /// Current color (RGB)
    color: Vec3,
    /// Current alpha (transparency)
    alpha: f32,
    /// Current inner scale multiplier
    inner_scale: Vec2,
    /// Current outer scale multiplier
    outer_scale: Vec2,

    /// Texture tiling count
    texture_tile_count: i32,

    /// Behavior flags
    flags: RingFlags,

    /// Transform matrix
    transform: Mat4,

    /// Cached world-space bounding box
    cached_box: AABoxClass,

    /// Current LOD level (0 = NULL, 1-20 = actual LOD)
    current_lod: usize,
    /// LOD bias multiplier
    lod_bias: f32,
    /// Pre-calculated LOD value array
    value: Vec<f32>,

    /// Animation state
    is_animating: bool,
    /// Animation time in normalized [0,1] range
    anim_time: f32,
    /// Animation duration in seconds
    anim_duration: f32,

    /// Animation channels
    color_channel: LERPAnimationChannel<Vec3>,
    alpha_channel: LERPAnimationChannel<f32>,
    inner_scale_channel: LERPAnimationChannel<Vec2>,
    outer_scale_channel: LERPAnimationChannel<Vec2>,
}

/// Prepared draw submission for a ring render call.
#[derive(Debug, Clone)]
pub struct RingRenderSubmission {
    pub world_transform: Mat4,
    pub vertex_count: usize,
    pub triangle_count: usize,
    pub tile_count: i32,
    pub color: Vec3,
    pub alpha: f32,
}

impl RingRenderObj {
    /// Create a new ring with default parameters
    /// C++ Reference: ringobj.cpp lines 168-197 (Constructor)
    pub fn new() -> Self {
        ensure_mesh_array_initialized();

        let mut ring = Self {
            name: String::new(),
            obj_space_center: Vec3::ZERO,
            obj_space_extent: Vec3::ONE,
            inner_extent: Vec2::new(0.5, 0.5),
            outer_extent: Vec2::ONE,
            color: Vec3::new(0.75, 0.75, 0.75),
            alpha: 1.0,
            inner_scale: Vec2::ONE,
            outer_scale: Vec2::ONE,
            texture_tile_count: DEFAULT_TEXTURE_TILE_COUNT,
            flags: RingFlags::new(),
            transform: Mat4::IDENTITY,
            cached_box: AABoxClass {
                center: Vec3::ZERO,
                extent: Vec3::ONE,
            },
            current_lod: RING_NUM_LOD,
            lod_bias: 1.0,
            value: vec![0.0; RING_NUM_LOD + 2],
            is_animating: false,
            anim_time: 0.0,
            anim_duration: 0.0,
            color_channel: LERPAnimationChannel::new(),
            alpha_channel: LERPAnimationChannel::new(),
            inner_scale_channel: LERPAnimationChannel::new(),
            outer_scale_channel: LERPAnimationChannel::new(),
        };

        // Initialize LOD value array
        ring.calculate_value_array(1.0);
        ring.update_cached_box();

        ring
    }

    /// Clone the ring object
    /// C++ Reference: ringobj.cpp lines 635-638 (Clone)
    pub fn clone_ring(&self) -> Self {
        self.clone()
    }

    /// Get the class ID for rings
    /// C++ Reference: ringobj.cpp lines 653-656 (Class_ID)
    pub fn class_id() -> i32 {
        // CLASSID_RING from RenderObjClass
        4 // Arbitrary ID matching C++ CLASSID_RING
    }

    // === Name Access ===

    /// Get the name of this ring
    /// C++ Reference: ringobj.cpp lines 484-487 (Get_Name)
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Set the name of this ring
    /// C++ Reference: ringobj.cpp lines 502-507 (Set_Name)
    pub fn set_name(&mut self, name: &str) {
        self.name = name.chars().take(MAX_NAME_LEN - 1).collect();
    }

    // === Color Access ===

    /// Set the current color
    /// C++ Reference: ringobj.h line 173 (Set_Color)
    pub fn set_color(&mut self, color: Vec3) {
        self.color = color;
    }

    /// Get the current color
    /// C++ Reference: ringobj.h line 178 (Get_Color)
    pub fn get_color(&self) -> Vec3 {
        self.color
    }

    /// Get the default color (first keyframe or current)
    /// C++ Reference: ringobj.cpp lines 752-763 (Get_Default_Color)
    pub fn get_default_color(&self) -> Vec3 {
        if self.color_channel.get_key_count() > 0 {
            if let Some(key) = self.color_channel.get_key(0) {
                return *key.get_value();
            }
        }
        self.color
    }

    // === Alpha Access ===

    /// Set the current alpha (transparency)
    /// C++ Reference: ringobj.h line 174 (Set_Alpha)
    pub fn set_alpha(&mut self, alpha: f32) {
        self.alpha = alpha.clamp(0.0, 1.0);
    }

    /// Get the current alpha
    /// C++ Reference: ringobj.h line 179 (Get_Alpha)
    pub fn get_alpha(&self) -> f32 {
        self.alpha
    }

    /// Get the default alpha (first keyframe or current)
    /// C++ Reference: ringobj.cpp lines 778-789 (Get_Default_Alpha)
    pub fn get_default_alpha(&self) -> f32 {
        if self.alpha_channel.get_key_count() > 0 {
            if let Some(key) = self.alpha_channel.get_key(0) {
                return *key.get_value();
            }
        }
        self.alpha
    }

    // === Scale Access ===

    /// Set the inner scale (ring hole size)
    /// C++ Reference: ringobj.h line 175 (Set_Inner_Scale)
    pub fn set_inner_scale(&mut self, scale: Vec2) {
        self.inner_scale = scale;
    }

    /// Get the inner scale
    /// C++ Reference: ringobj.h line 180 (Get_Inner_Scale)
    pub fn get_inner_scale(&self) -> Vec2 {
        self.inner_scale
    }

    /// Get the default inner scale (first keyframe or current)
    /// C++ Reference: ringobj.cpp lines 804-815 (Get_Default_Inner_Scale)
    pub fn get_default_inner_scale(&self) -> Vec2 {
        if self.inner_scale_channel.get_key_count() > 0 {
            if let Some(key) = self.inner_scale_channel.get_key(0) {
                return *key.get_value();
            }
        }
        self.inner_scale
    }

    /// Set the outer scale (ring outer radius)
    /// C++ Reference: ringobj.h line 176 (Set_Outer_Scale)
    pub fn set_outer_scale(&mut self, scale: Vec2) {
        self.outer_scale = scale;
    }

    /// Get the outer scale
    /// C++ Reference: ringobj.h line 181 (Get_Outer_Scale)
    pub fn get_outer_scale(&self) -> Vec2 {
        self.outer_scale
    }

    /// Get the default outer scale (first keyframe or current)
    /// C++ Reference: ringobj.cpp lines 830-841 (Get_Default_Outer_Scale)
    pub fn get_default_outer_scale(&self) -> Vec2 {
        if self.outer_scale_channel.get_key_count() > 0 {
            if let Some(key) = self.outer_scale_channel.get_key(0) {
                return *key.get_value();
            }
        }
        self.outer_scale
    }

    // === Extent Access ===

    /// Get the inner extent (ring hole radius)
    /// C++ Reference: ringobj.h line 189 (Get_Inner_Extent)
    pub fn get_inner_extent(&self) -> Vec2 {
        self.inner_extent
    }

    /// Set the inner extent
    /// C++ Reference: ringobj.h lines 284-287 (Set_Inner_Extent)
    pub fn set_inner_extent(&mut self, extent: Vec2) {
        self.inner_extent = extent;
    }

    /// Get the outer extent (ring outer radius)
    /// C++ Reference: ringobj.h line 190 (Get_Outer_Extent)
    pub fn get_outer_extent(&self) -> Vec2 {
        self.outer_extent
    }

    /// Set the outer extent
    /// C++ Reference: ringobj.h lines 289-296 (Set_Outer_Extent)
    pub fn set_outer_extent(&mut self, extent: Vec2) {
        self.outer_extent = extent;
        self.obj_space_extent.x = extent.x;
        self.obj_space_extent.y = extent.y;
        self.obj_space_extent.z = 0.0;
        self.update_cached_box();
    }

    /// Set local center and extent
    /// C++ Reference: ringobj.h lines 298-303 (Set_Local_Center_Extent)
    pub fn set_local_center_extent(&mut self, center: Vec3, extent: Vec3) {
        self.obj_space_center = center;
        self.obj_space_extent = extent;
        self.update_cached_box();
    }

    /// Set local min/max bounds
    /// C++ Reference: ringobj.h lines 305-310 (Set_Local_Min_Max)
    pub fn set_local_min_max(&mut self, min: Vec3, max: Vec3) {
        self.obj_space_center = (max + min) / 2.0;
        self.obj_space_extent = (max - min) / 2.0;
        self.update_cached_box();
    }

    // === Texture Tiling ===

    /// Get texture tiling count
    /// C++ Reference: ringobj.h line 169 (Get_Texture_Tiling)
    pub fn get_texture_tiling(&self) -> i32 {
        self.texture_tile_count
    }

    /// Set texture tiling count
    /// C++ Reference: ringobj.h line 170 (Set_Texture_Tiling)
    pub fn set_texture_tiling(&mut self, count: i32) {
        self.texture_tile_count = count;
    }

    // === Flags ===

    /// Get behavior flags
    /// C++ Reference: ringobj.h line 159 (Get_Flags)
    pub fn get_flags(&self) -> u32 {
        self.flags.bits()
    }

    /// Set behavior flags
    /// C++ Reference: ringobj.h line 160 (Set_Flags)
    pub fn set_flags(&mut self, flags: u32) {
        self.flags = RingFlags(flags);
    }

    /// Set a specific flag
    /// C++ Reference: ringobj.h line 161 (Set_Flag)
    pub fn set_flag(&mut self, flag: u32, enable: bool) {
        self.flags.set_flag(flag, enable);
    }

    // === Animation Control ===

    /// Check if animating
    /// C++ Reference: ringobj.h line 164 (Is_Animating)
    pub fn is_animating(&self) -> bool {
        self.is_animating
    }

    /// Start animation playback
    /// C++ Reference: ringobj.h line 165 (Start_Animating)
    pub fn start_animating(&mut self) {
        self.is_animating = true;
        self.anim_time = 0.0;
    }

    /// Stop animation playback
    /// C++ Reference: ringobj.h line 166 (Stop_Animating)
    pub fn stop_animating(&mut self) {
        self.is_animating = false;
        self.anim_time = 0.0;
    }

    /// Get animation duration in seconds
    /// C++ Reference: ringobj.h line 203 (Get_Animation_Duration)
    pub fn get_animation_duration(&self) -> f32 {
        self.anim_duration
    }

    /// Set animation duration in seconds
    /// C++ Reference: ringobj.h line 204 (Set_Animation_Duration)
    pub fn set_animation_duration(&mut self, duration: f32) {
        self.anim_duration = duration;
        self.anim_time = 0.0;
    }

    /// Restart animation from beginning
    /// C++ Reference: ringobj.h line 205 (Restart_Animation)
    pub fn restart_animation(&mut self) {
        self.anim_time = 0.0;
    }

    // === Animation Channels ===

    /// Get mutable reference to color channel
    /// C++ Reference: ringobj.h line 208 (Get_Color_Channel)
    pub fn get_color_channel_mut(&mut self) -> &mut LERPAnimationChannel<Vec3> {
        &mut self.color_channel
    }

    /// Get reference to color channel
    /// C++ Reference: ringobj.h line 209 (Peek_Color_Channel)
    pub fn get_color_channel(&self) -> &LERPAnimationChannel<Vec3> {
        &self.color_channel
    }

    /// Set color channel
    /// C++ Reference: ringobj.h line 220 (Set_Color_Channel)
    pub fn set_color_channel(&mut self, channel: LERPAnimationChannel<Vec3>) {
        self.color_channel = channel;
    }

    /// Get mutable reference to alpha channel
    /// C++ Reference: ringobj.h line 211 (Get_Alpha_Channel)
    pub fn get_alpha_channel_mut(&mut self) -> &mut LERPAnimationChannel<f32> {
        &mut self.alpha_channel
    }

    /// Get reference to alpha channel
    /// C++ Reference: ringobj.h line 212 (Peek_Alpha_Channel)
    pub fn get_alpha_channel(&self) -> &LERPAnimationChannel<f32> {
        &self.alpha_channel
    }

    /// Set alpha channel
    /// C++ Reference: ringobj.h line 221 (Set_Alpha_Channel)
    pub fn set_alpha_channel(&mut self, channel: LERPAnimationChannel<f32>) {
        self.alpha_channel = channel;
    }

    /// Get mutable reference to inner scale channel
    /// C++ Reference: ringobj.h line 214 (Get_Inner_Scale_Channel)
    pub fn get_inner_scale_channel_mut(&mut self) -> &mut LERPAnimationChannel<Vec2> {
        &mut self.inner_scale_channel
    }

    /// Get reference to inner scale channel
    /// C++ Reference: ringobj.h line 215 (Peek_Inner_Scale_Channel)
    pub fn get_inner_scale_channel(&self) -> &LERPAnimationChannel<Vec2> {
        &self.inner_scale_channel
    }

    /// Set inner scale channel
    /// C++ Reference: ringobj.h line 222 (Set_Inner_Scale_Channel)
    pub fn set_inner_scale_channel(&mut self, channel: LERPAnimationChannel<Vec2>) {
        self.inner_scale_channel = channel;
    }

    /// Get mutable reference to outer scale channel
    /// C++ Reference: ringobj.h line 217 (Get_Outer_Scale_Channel)
    pub fn get_outer_scale_channel_mut(&mut self) -> &mut LERPAnimationChannel<Vec2> {
        &mut self.outer_scale_channel
    }

    /// Get reference to outer scale channel
    /// C++ Reference: ringobj.h line 218 (Peek_Outer_Scale_Channel)
    pub fn get_outer_scale_channel(&self) -> &LERPAnimationChannel<Vec2> {
        &self.outer_scale_channel
    }

    /// Set outer scale channel
    /// C++ Reference: ringobj.h line 223 (Set_Outer_Scale_Channel)
    pub fn set_outer_scale_channel(&mut self, channel: LERPAnimationChannel<Vec2>) {
        self.outer_scale_channel = channel;
    }

    // === Transform ===

    /// Set the transform matrix
    /// C++ Reference: ringobj.cpp lines 881-885 (Set_Transform)
    pub fn set_transform(&mut self, transform: Mat4) {
        self.transform = transform;
        self.update_cached_box();
    }

    /// Get the transform matrix
    pub fn get_transform(&self) -> Mat4 {
        self.transform
    }

    /// Set position
    /// C++ Reference: ringobj.cpp lines 900-904 (Set_Position)
    pub fn set_position(&mut self, position: Vec3) {
        self.transform.w_axis = position.extend(1.0);
        self.update_cached_box();
    }

    /// Get position
    pub fn get_position(&self) -> Vec3 {
        self.transform.w_axis.truncate()
    }

    // === Bounding Volumes ===

    /// Get the world-space bounding box
    /// C++ Reference: ringobj.h lines 312-317 (Get_Box)
    pub fn get_box(&self) -> &AABoxClass {
        &self.cached_box
    }

    /// Get object-space bounding sphere
    /// C++ Reference: ringobj.cpp lines 938-942 (Get_Obj_Space_Bounding_Sphere)
    pub fn get_obj_space_bounding_sphere(&self) -> SphereClass {
        SphereClass::new(self.obj_space_center, self.obj_space_extent.length())
    }

    /// Get object-space bounding box
    /// C++ Reference: ringobj.cpp lines 957-961 (Get_Obj_Space_Bounding_Box)
    pub fn get_obj_space_bounding_box(&self) -> AABoxClass {
        AABoxClass {
            center: self.obj_space_center,
            extent: self.obj_space_extent,
        }
    }

    /// Update cached world-space bounding box
    /// C++ Reference: ringobj.cpp lines 919-923 (update_cached_box)
    fn update_cached_box(&mut self) {
        let position = self.transform.w_axis.truncate();
        self.cached_box = AABoxClass {
            center: position + self.obj_space_center,
            extent: self.obj_space_extent,
        };
    }

    // === LOD System ===

    /// Get current LOD level
    /// C++ Reference: ringobj.cpp lines 1005-1008 (Get_LOD_Level)
    pub fn get_lod_level(&self) -> usize {
        self.current_lod
    }

    /// Set LOD level (0 = NULL, 1-20 = actual LODs)
    /// C++ Reference: ringobj.cpp lines 1000-1003 (Set_LOD_Level)
    pub fn set_lod_level(&mut self, lod: usize) {
        self.current_lod = lod.min(RING_NUM_LOD);
    }

    /// Get number of LOD levels (including NULL)
    /// C++ Reference: ringobj.cpp lines 1010-1013 (Get_LOD_Count)
    pub fn get_lod_count(&self) -> usize {
        RING_NUM_LOD + 1
    }

    /// Set LOD bias multiplier
    /// C++ Reference: ringobj.h line 142 (Set_LOD_Bias)
    pub fn set_lod_bias(&mut self, bias: f32) {
        self.lod_bias = bias.max(0.0);
    }

    /// Increment LOD (lower detail)
    /// C++ Reference: ringobj.cpp lines 975-978 (Increment_LOD)
    pub fn increment_lod(&mut self) {
        if self.current_lod < RING_NUM_LOD {
            self.current_lod += 1;
        }
    }

    /// Decrement LOD (higher detail)
    /// C++ Reference: ringobj.cpp lines 980-983 (Decrement_LOD)
    pub fn decrement_lod(&mut self) {
        if self.current_lod > 0 {
            self.current_lod -= 1;
        }
    }

    /// Get rendering cost (polygon count)
    /// C++ Reference: ringobj.cpp lines 985-988 (Get_Cost)
    pub fn get_cost(&self) -> f32 {
        self.get_num_polys() as f32
    }

    /// Get LOD value at current level
    /// C++ Reference: ringobj.cpp lines 990-993 (Get_Value)
    pub fn get_value(&self) -> f32 {
        self.value.get(self.current_lod).copied().unwrap_or(0.0)
    }

    /// Get LOD value after incrementing
    /// C++ Reference: ringobj.cpp lines 995-998 (Get_Post_Increment_Value)
    pub fn get_post_increment_value(&self) -> f32 {
        self.value.get(self.current_lod + 1).copied().unwrap_or(0.0)
    }

    /// Calculate LOD value array for a given screen area
    /// C++ Reference: ringobj.cpp lines 394-403 (calculate_value_array)
    fn calculate_value_array(&mut self, screen_area: f32) {
        const AT_MIN_LOD: f32 = -1e30f32;
        const AT_MAX_LOD: f32 = 1e30f32;

        self.value[0] = AT_MIN_LOD;

        RING_MESH_ARRAY.with(|array| {
            if let Some(ref array) = *array.borrow() {
                for lod in 1..=RING_NUM_LOD {
                    let polycount = array.get_cost(lod);
                    let benefit_factor = 1.0 - (0.5 / (polycount * polycount));
                    self.value[lod] = (benefit_factor * screen_area * self.lod_bias) / polycount;
                }
            }
        });

        self.value[RING_NUM_LOD + 1] = AT_MAX_LOD;
    }

    /// Get polygon count at current LOD
    /// C++ Reference: ringobj.cpp lines 449-452 (Get_Num_Polys)
    pub fn get_num_polys(&self) -> usize {
        RING_MESH_ARRAY.with(|array| {
            if let Some(ref array) = *array.borrow() {
                array.get_cost(self.current_lod) as usize
            } else {
                0
            }
        })
    }

    // === Scaling ===

    /// Scale uniformly
    /// C++ Reference: ringobj.cpp lines 1039-1059 (Scale uniform)
    pub fn scale(&mut self, scale: f32) {
        if scale == 1.0 {
            return;
        }

        // Scale default values
        self.inner_scale *= scale;
        self.outer_scale *= scale;

        // Scale all keyframes in animation channels
        let inner_count = self.inner_scale_channel.get_key_count();
        for i in 0..inner_count {
            if let Some(key) = self.inner_scale_channel.get_key(i) {
                let scaled_value = *key.get_value() * scale;
                self.inner_scale_channel.set_key_value(i, scaled_value);
            }
        }

        let outer_count = self.outer_scale_channel.get_key_count();
        for i in 0..outer_count {
            if let Some(key) = self.outer_scale_channel.get_key(i) {
                let scaled_value = *key.get_value() * scale;
                self.outer_scale_channel.set_key_value(i, scaled_value);
            }
        }
    }

    /// Scale non-uniformly
    /// C++ Reference: ringobj.cpp lines 1074-1100 (Scale non-uniform)
    pub fn scale_xyz(&mut self, scale_x: f32, scale_y: f32, _scale_z: f32) {
        // Scale default values
        self.inner_scale.x *= scale_x;
        self.inner_scale.y *= scale_y;
        self.outer_scale.x *= scale_x;
        self.outer_scale.y *= scale_y;

        // Scale all keyframes in animation channels
        let inner_count = self.inner_scale_channel.get_key_count();
        for i in 0..inner_count {
            if let Some(key) = self.inner_scale_channel.get_key(i) {
                let mut scaled_value = *key.get_value();
                scaled_value.x *= scale_x;
                scaled_value.y *= scale_y;
                self.inner_scale_channel.set_key_value(i, scaled_value);
            }
        }

        let outer_count = self.outer_scale_channel.get_key_count();
        for i in 0..outer_count {
            if let Some(key) = self.outer_scale_channel.get_key(i) {
                let mut scaled_value = *key.get_value();
                scaled_value.x *= scale_x;
                scaled_value.y *= scale_y;
                self.outer_scale_channel.set_key_value(i, scaled_value);
            }
        }
    }

    // === Animation Update ===

    /// Update animation state (call per frame)
    /// C++ Reference: ringobj.cpp lines 1140-1188 (animate)
    pub fn animate(&mut self, frame_time_ms: f32) {
        if !self.is_animating {
            return;
        }

        let has_animation = self.color_channel.get_key_count() > 0
            || self.alpha_channel.get_key_count() > 0
            || self.inner_scale_channel.get_key_count() > 0
            || self.outer_scale_channel.get_key_count() > 0;

        if !has_animation {
            return;
        }

        // Convert milliseconds to normalized time [0, 1]
        if self.anim_duration > 0.0 {
            let frame_time_sec = frame_time_ms * 0.001;
            let normalized_time = frame_time_sec / self.anim_duration;
            self.anim_time += normalized_time;
        } else {
            self.anim_time = 1.0;
        }

        // Handle looping
        if self.flags.has_flag(RingFlags::USE_ANIMATION_LOOP) && self.anim_time > 1.0 {
            self.anim_time -= 1.0;
        }

        // Evaluate channels at current time
        if self.color_channel.get_key_count() > 0 {
            self.color = self.color_channel.evaluate(self.anim_time);
        }

        if self.alpha_channel.get_key_count() > 0 {
            self.alpha = self.alpha_channel.evaluate(self.anim_time);
        }

        if self.inner_scale_channel.get_key_count() > 0 {
            self.inner_scale = self.inner_scale_channel.evaluate(self.anim_time);
        }

        if self.outer_scale_channel.get_key_count() > 0 {
            self.outer_scale = self.outer_scale_channel.evaluate(self.anim_time);
            // Update bounding volume when outer scale changes
            self.obj_space_extent.x = self.outer_scale.x * self.outer_extent.x;
            self.obj_space_extent.y = self.outer_scale.y * self.outer_extent.y;
            self.obj_space_extent.z = 0.0;
            self.update_cached_box();
        }
    }

    // === Rendering ===

    /// Prepare immutable draw submission data for backend renderers.
    pub fn prepare_render_submission(&self) -> Option<RingRenderSubmission> {
        if self.current_lod == 0 {
            return None;
        }

        RING_MESH_ARRAY.with(|array| {
            let array = array.borrow();
            let mesh = array.as_ref()?.get_mesh(self.current_lod)?;
            Some(RingRenderSubmission {
                world_transform: self.transform,
                vertex_count: mesh.vertices.len(),
                triangle_count: mesh.triangles.len(),
                tile_count: self.texture_tile_count,
                color: self.color,
                alpha: self.alpha,
            })
        })
    }

    /// Render the ring
    /// C++ Reference: ringobj.cpp lines 671-737 (Render)
    pub fn render(&mut self, frame_time_ms: f32) {
        // Skip NULL LOD
        if self.current_lod == 0 {
            return;
        }

        // Update animation
        self.animate(frame_time_ms);

        // Scale the mesh to match current inner/outer scales
        let inner_scale = Vec2::new(
            self.inner_extent.x * self.inner_scale.x,
            self.inner_extent.y * self.inner_scale.y,
        );
        let outer_scale = Vec2::new(
            self.outer_extent.x * self.outer_scale.x,
            self.outer_extent.y * self.outer_scale.y,
        );

        RING_MESH_ARRAY.with(|array| {
            if let Some(ref mut array) = *array.borrow_mut() {
                if let Some(mesh) = array.get_mesh_mut(self.current_lod) {
                    mesh.scale(inner_scale, outer_scale);
                    mesh.set_tiling(self.texture_tile_count);
                }
            }
        });

        if let Some(submission) = self.prepare_render_submission() {
            let _ = submission;
        }
    }

    /// Get vertex data for current LOD (for custom rendering)
    pub fn get_vertex_data(&self) -> Option<(Vec<Vec3>, Vec<Vec3>, Vec<Vec2>, Vec<TriIndex>)> {
        RING_MESH_ARRAY.with(|array| {
            if let Some(ref array) = *array.borrow() {
                if let Some(mesh) = array.get_mesh(self.current_lod) {
                    return Some((
                        mesh.vertices.clone(),
                        mesh.normals.clone(),
                        mesh.uvs.clone(),
                        mesh.triangles.clone(),
                    ));
                }
            }
            None
        })
    }
}

impl Default for RingRenderObj {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_creation() {
        let ring = RingRenderObj::new();
        assert_eq!(ring.get_name(), "");
        assert_eq!(ring.get_color(), Vec3::new(0.75, 0.75, 0.75));
        assert_eq!(ring.get_alpha(), 1.0);
        assert_eq!(ring.get_inner_scale(), Vec2::ONE);
        assert_eq!(ring.get_outer_scale(), Vec2::ONE);
        assert_eq!(ring.get_lod_level(), RING_NUM_LOD);
        assert!(!ring.is_animating());
    }

    #[test]
    fn test_ring_properties() {
        let mut ring = RingRenderObj::new();

        ring.set_name("TestRing");
        assert_eq!(ring.get_name(), "TestRing");

        ring.set_color(Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(ring.get_color(), Vec3::new(1.0, 0.0, 0.0));

        ring.set_alpha(0.5);
        assert_eq!(ring.get_alpha(), 0.5);

        ring.set_inner_scale(Vec2::new(0.5, 0.5));
        assert_eq!(ring.get_inner_scale(), Vec2::new(0.5, 0.5));

        ring.set_outer_scale(Vec2::new(2.0, 2.0));
        assert_eq!(ring.get_outer_scale(), Vec2::new(2.0, 2.0));
    }

    #[test]
    fn test_ring_extents() {
        let mut ring = RingRenderObj::new();

        ring.set_inner_extent(Vec2::new(0.25, 0.25));
        assert_eq!(ring.get_inner_extent(), Vec2::new(0.25, 0.25));

        ring.set_outer_extent(Vec2::new(1.5, 1.5));
        assert_eq!(ring.get_outer_extent(), Vec2::new(1.5, 1.5));
    }

    #[test]
    fn test_ring_flags() {
        let mut ring = RingRenderObj::new();

        assert_eq!(ring.get_flags(), 0);

        ring.set_flag(RingFlags::USE_CAMERA_ALIGN, true);
        assert_eq!(ring.get_flags(), RingFlags::USE_CAMERA_ALIGN);

        ring.set_flag(RingFlags::USE_ANIMATION_LOOP, true);
        assert_eq!(
            ring.get_flags(),
            RingFlags::USE_CAMERA_ALIGN | RingFlags::USE_ANIMATION_LOOP
        );

        ring.set_flag(RingFlags::USE_CAMERA_ALIGN, false);
        assert_eq!(ring.get_flags(), RingFlags::USE_ANIMATION_LOOP);
    }

    #[test]
    fn test_ring_animation_control() {
        let mut ring = RingRenderObj::new();

        assert!(!ring.is_animating());

        ring.start_animating();
        assert!(ring.is_animating());

        ring.stop_animating();
        assert!(!ring.is_animating());

        ring.set_animation_duration(5.0);
        assert_eq!(ring.get_animation_duration(), 5.0);
    }

    #[test]
    fn test_ring_lod_system() {
        let mut ring = RingRenderObj::new();

        assert_eq!(ring.get_lod_level(), RING_NUM_LOD);
        assert_eq!(ring.get_lod_count(), RING_NUM_LOD + 1);

        ring.set_lod_level(10);
        assert_eq!(ring.get_lod_level(), 10);

        ring.increment_lod();
        assert_eq!(ring.get_lod_level(), 11);

        ring.decrement_lod();
        assert_eq!(ring.get_lod_level(), 10);

        // Test bounds
        ring.set_lod_level(0);
        ring.decrement_lod();
        assert_eq!(ring.get_lod_level(), 0);

        ring.set_lod_level(RING_NUM_LOD);
        ring.increment_lod();
        assert_eq!(ring.get_lod_level(), RING_NUM_LOD);
    }

    #[test]
    fn test_ring_transform() {
        let mut ring = RingRenderObj::new();

        let pos = Vec3::new(10.0, 20.0, 30.0);
        ring.set_position(pos);
        assert_eq!(ring.get_position(), pos);

        let transform = Mat4::from_translation(Vec3::new(5.0, 10.0, 15.0));
        ring.set_transform(transform);
        assert_eq!(ring.get_transform(), transform);
    }

    #[test]
    fn test_ring_bounding_volumes() {
        let ring = RingRenderObj::new();

        let sphere = ring.get_obj_space_bounding_sphere();
        assert_eq!(sphere.center, Vec3::ZERO);
        assert!(sphere.radius > 0.0);

        let aabb = ring.get_obj_space_bounding_box();
        assert_eq!(aabb.center, Vec3::ZERO);
        assert_eq!(aabb.extent, Vec3::ONE);
    }

    #[test]
    fn test_ring_scaling() {
        let mut ring = RingRenderObj::new();

        ring.set_inner_scale(Vec2::ONE);
        ring.set_outer_scale(Vec2::ONE);

        ring.scale(2.0);
        assert_eq!(ring.get_inner_scale(), Vec2::new(2.0, 2.0));
        assert_eq!(ring.get_outer_scale(), Vec2::new(2.0, 2.0));

        ring.set_inner_scale(Vec2::ONE);
        ring.set_outer_scale(Vec2::ONE);

        ring.scale_xyz(2.0, 3.0, 1.0);
        assert_eq!(ring.get_inner_scale(), Vec2::new(2.0, 3.0));
        assert_eq!(ring.get_outer_scale(), Vec2::new(2.0, 3.0));
    }

    #[test]
    fn test_ring_animation_channels() {
        let mut ring = RingRenderObj::new();

        // Add color keyframes
        ring.get_color_channel_mut()
            .add_key(Vec3::new(1.0, 0.0, 0.0), 0.0);
        ring.get_color_channel_mut()
            .add_key(Vec3::new(0.0, 1.0, 0.0), 1.0);

        assert_eq!(ring.get_color_channel().get_key_count(), 2);
        assert_eq!(ring.get_default_color(), Vec3::new(1.0, 0.0, 0.0));

        // Add alpha keyframes
        ring.get_alpha_channel_mut().add_key(0.0, 0.0);
        ring.get_alpha_channel_mut().add_key(1.0, 1.0);

        assert_eq!(ring.get_alpha_channel().get_key_count(), 2);
        assert_eq!(ring.get_default_alpha(), 0.0);
    }

    #[test]
    fn test_ring_mesh_generation() {
        let mesh = RingMesh::new(1.0, 10);
        assert_eq!(mesh.slices, 10);
        assert_eq!(mesh.vertex_count, 22); // 10 * 2 + 2
        assert_eq!(mesh.face_count, 20); // 10 * 2
        assert_eq!(mesh.vertices.len(), 22);
        assert_eq!(mesh.triangles.len(), 20);
    }

    #[test]
    fn test_ring_mesh_scaling() {
        let mut mesh = RingMesh::new(1.0, 10);

        let inner_scale = Vec2::new(0.5, 0.5);
        let outer_scale = Vec2::new(2.0, 2.0);

        mesh.scale(inner_scale, outer_scale);

        // Check that inner vertices (even indices) are scaled correctly
        assert!((mesh.vertices[0].length() - 0.5).abs() < 0.01);

        // Check that outer vertices (odd indices) are scaled correctly
        assert!((mesh.vertices[1].length() - 2.0).abs() < 0.01);
    }

    #[test]
    fn test_ring_mesh_tiling() {
        let mut mesh = RingMesh::new(1.0, 10);

        mesh.set_tiling(10);
        assert_eq!(mesh.tile_count, 10);

        // Check UV coordinates are updated
        assert_eq!(mesh.uvs[0].x, 0.0);
        assert_eq!(mesh.uvs[0].y, 0.0);
        assert_eq!(mesh.uvs[1].x, 0.0);
        assert_eq!(mesh.uvs[1].y, 1.0);
    }

    #[test]
    fn test_ring_mesh_array_generation() {
        let array = RingMeshArray::new();
        assert_eq!(array.meshes.len(), RING_NUM_LOD);
        assert_eq!(array.costs.len(), RING_NUM_LOD + 1);

        // Check that costs increase with LOD
        for i in 1..RING_NUM_LOD {
            assert!(array.costs[i] < array.costs[i + 1]);
        }
    }

    #[test]
    fn test_ring_clone() {
        let mut ring1 = RingRenderObj::new();
        ring1.set_name("Original");
        ring1.set_color(Vec3::new(1.0, 0.0, 0.0));
        ring1.set_alpha(0.5);

        let ring2 = ring1.clone_ring();
        assert_eq!(ring2.get_name(), "Original");
        assert_eq!(ring2.get_color(), Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(ring2.get_alpha(), 0.5);
    }

    #[test]
    fn test_ring_texture_tiling() {
        let mut ring = RingRenderObj::new();

        ring.set_texture_tiling(10);
        assert_eq!(ring.get_texture_tiling(), 10);

        ring.set_texture_tiling(20);
        assert_eq!(ring.get_texture_tiling(), 20);
    }

    #[test]
    fn test_ring_animation_update() {
        let mut ring = RingRenderObj::new();

        // Set up color animation
        ring.get_color_channel_mut()
            .add_key(Vec3::new(1.0, 0.0, 0.0), 0.0);
        ring.get_color_channel_mut()
            .add_key(Vec3::new(0.0, 1.0, 0.0), 1.0);

        ring.set_animation_duration(1.0);
        ring.start_animating();

        // Animate half way
        ring.animate(500.0); // 500ms = 0.5 seconds
        let color = ring.get_color();

        // Should be somewhere between red and green
        assert!(color.x > 0.0 && color.x < 1.0);
        assert!(color.y > 0.0 && color.y < 1.0);
        assert_eq!(color.z, 0.0);
    }

    #[test]
    fn test_ring_lod_costs() {
        let ring = RingRenderObj::new();

        // Test that higher LODs have higher costs
        let mut prev_cost = 0.0;
        for lod in 1..=RING_NUM_LOD {
            RING_MESH_ARRAY.with(|array| {
                if let Some(ref array) = *array.borrow() {
                    let cost = array.get_cost(lod);
                    assert!(cost > prev_cost);
                    prev_cost = cost;
                }
            });
        }
    }

    #[test]
    fn test_ring_geometry_validation() {
        let ring = RingRenderObj::new();

        // Set a specific LOD and get vertex data
        let mut test_ring = ring.clone();
        test_ring.set_lod_level(10);

        if let Some((vertices, normals, uvs, triangles)) = test_ring.get_vertex_data() {
            // Verify vertex count
            assert!(vertices.len() > 0);
            assert_eq!(vertices.len(), normals.len());
            assert_eq!(vertices.len(), uvs.len());

            // Verify all vertices are in XY plane (Z=0)
            for vertex in &vertices {
                assert_eq!(vertex.z, 0.0);
            }

            // Verify all normals point up
            for normal in &normals {
                assert_eq!(*normal, Vec3::Z);
            }

            // Verify UV coordinates are in valid range
            for uv in &uvs {
                assert!(uv.x >= 0.0);
                assert!(uv.y >= 0.0 && uv.y <= 1.0);
            }

            // Verify triangles don't have out-of-bounds indices
            let max_index = vertices.len() as u16;
            for tri in &triangles {
                assert!(tri.i < max_index);
                assert!(tri.j < max_index);
                assert!(tri.k < max_index);
            }
        }
    }

    #[test]
    fn test_prepare_render_submission_returns_geometry_for_valid_lod() {
        let mut ring = RingRenderObj::new();
        ring.set_lod_level(1);
        let submission = ring.prepare_render_submission().expect("submission");
        assert!(submission.vertex_count > 0);
        assert!(submission.triangle_count > 0);
    }
}
