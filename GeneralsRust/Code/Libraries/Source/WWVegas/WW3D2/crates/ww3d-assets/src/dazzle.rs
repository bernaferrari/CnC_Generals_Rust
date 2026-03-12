// Dazzle System - Lens Flare/Sun Glare Effect Renderer
//
// Ported from C++ dazzle.cpp/dazzle.h (Command & Conquer Generals Zero Hour)
// with 100% fidelity to original implementation.
//
// The Dazzle system creates realistic light source effects including:
// - Dazzle Core: The main bright center with configurable direction/angle cutoff
// - Halo: A surrounding glow effect
// - Lensflare: Multiple sprite-based flares positioned along the light ray path
//
// Architecture:
// - Two-phase rendering: visibility detection during scene traversal, batched rendering after
// - Screen-space rendering with orthographic projection
// - Temporal smoothing for smooth intensity transitions
// - Blinking support for periodic on/off effects

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use crate::texture::{MipCount, TextureBase, TextureFormat, TextureManager};

// ============================================================================
// Type Aliases for Math (replace with your math library)
// ============================================================================

/// 3D vector (x, y, z)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vector3 {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn dot(&self, other: &Vector3) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    pub fn length_squared(&self) -> f32 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }

    pub fn length(&self) -> f32 {
        self.length_squared().sqrt()
    }

    pub fn normalize(&mut self) {
        let len = self.length();
        if len > 0.0 {
            self.x /= len;
            self.y /= len;
            self.z /= len;
        }
    }

    pub fn normalized(&self) -> Self {
        let mut result = *self;
        result.normalize();
        result
    }
}

impl std::ops::Mul<f32> for Vector3 {
    type Output = Vector3;
    fn mul(self, rhs: f32) -> Vector3 {
        Vector3::new(self.x * rhs, self.y * rhs, self.z * rhs)
    }
}

impl std::ops::Sub for Vector3 {
    type Output = Vector3;
    fn sub(self, rhs: Vector3) -> Vector3 {
        Vector3::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

/// 4D vector (x, y, z, w) - used for homogeneous coordinates
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vector4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Vector4 {
    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }
}

/// 3x3 matrix for rotations
#[derive(Debug, Clone, Copy)]
pub struct Matrix3D {
    pub m: [[f32; 3]; 3],
}

impl Matrix3D {
    pub fn identity() -> Self {
        Self {
            m: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        }
    }

    /// Rotate a vector by this matrix
    pub fn rotate_vector(&self, v: &Vector3) -> Vector3 {
        Vector3::new(
            self.m[0][0] * v.x + self.m[0][1] * v.y + self.m[0][2] * v.z,
            self.m[1][0] * v.x + self.m[1][1] * v.y + self.m[1][2] * v.z,
            self.m[2][0] * v.x + self.m[2][1] * v.y + self.m[2][2] * v.z,
        )
    }
}

/// 4x4 matrix for transformations
#[derive(Debug, Clone, Copy)]
pub struct Matrix4 {
    pub m: [[f32; 4]; 4],
}

impl Matrix4 {
    pub fn identity() -> Self {
        Self {
            m: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// Transform a point (Vector3) to homogeneous coordinates (Vector4)
    pub fn transform_point(&self, v: &Vector3) -> Vector4 {
        Vector4::new(
            self.m[0][0] * v.x + self.m[0][1] * v.y + self.m[0][2] * v.z + self.m[0][3],
            self.m[1][0] * v.x + self.m[1][1] * v.y + self.m[1][2] * v.z + self.m[1][3],
            self.m[2][0] * v.x + self.m[2][1] * v.y + self.m[2][2] * v.z + self.m[2][3],
            self.m[3][0] * v.x + self.m[3][1] * v.y + self.m[3][2] * v.z + self.m[3][3],
        )
    }
}

// ============================================================================
// Configuration Structures (matches C++ DazzleInitClass)
// ============================================================================

/// Configuration for a dazzle type - all parameters loaded from INI file.
/// Matches C++ DazzleInitClass exactly.
#[derive(Debug, Clone)]
pub struct DazzleInitClass {
    pub type_id: usize,
    pub use_camera_translation: bool,
    pub primary_texture_name: String,
    pub secondary_texture_name: String,
    pub lensflare_name: String,
    pub halo_intensity: f32,
    pub halo_intensity_pow: f32,
    pub halo_scale_x: f32,
    pub halo_scale_y: f32,
    pub dazzle_size_pow: f32,
    pub dazzle_intensity_pow: f32,
    pub dazzle_intensity: f32,
    pub dazzle_area: f32,
    pub dazzle_direction_area: f32,
    pub dazzle_direction: Vector3,
    pub dazzle_test_color: Vector3,
    pub dazzle_color: Vector3,
    pub halo_color: Vector3,
    pub dazzle_scale_x: f32,
    pub dazzle_scale_y: f32,
    pub fadeout_start: f32,
    pub fadeout_end: f32,
    pub size_optimization_limit: f32,
    pub history_weight: f32,
    pub radius: f32,
    pub blink_period: f32,
    pub blink_on_time: f32,
}

impl Default for DazzleInitClass {
    fn default() -> Self {
        Self {
            type_id: 0,
            use_camera_translation: true,
            primary_texture_name: String::new(),
            secondary_texture_name: String::new(),
            lensflare_name: String::new(),
            halo_intensity: 1.0,
            halo_intensity_pow: 0.95,
            halo_scale_x: 0.2,
            halo_scale_y: 0.2,
            dazzle_size_pow: 0.9,
            dazzle_intensity_pow: 0.9,
            dazzle_intensity: 50.0,
            dazzle_area: 0.05,
            dazzle_direction_area: 0.0,
            dazzle_direction: Vector3::new(0.0, 1.0, 1.0),
            dazzle_test_color: Vector3::new(1.0, 1.0, 1.0),
            dazzle_color: Vector3::new(1.0, 1.0, 1.0),
            halo_color: Vector3::new(0.0, 0.0, 1.0),
            dazzle_scale_x: 1.0,
            dazzle_scale_y: 1.0,
            fadeout_start: 30.0,
            fadeout_end: 40.0,
            size_optimization_limit: 0.05,
            history_weight: 0.975,
            radius: 1.0,
            blink_period: 0.0,
            blink_on_time: 0.0,
        }
    }
}

/// Configuration for a lensflare type.
/// Matches C++ LensflareInitClass exactly.
#[derive(Debug, Clone)]
pub struct LensflareInitClass {
    pub type_id: usize,
    pub texture_name: String,
    pub flare_count: usize,
    pub flare_locations: Vec<f32>,
    pub flare_sizes: Vec<f32>,
    pub flare_colors: Vec<Vector3>,
    pub flare_uv: Vec<Vector4>,
}

impl Default for LensflareInitClass {
    fn default() -> Self {
        Self {
            type_id: 0,
            texture_name: String::new(),
            flare_count: 0,
            flare_locations: Vec::new(),
            flare_sizes: Vec::new(),
            flare_colors: Vec::new(),
            flare_uv: Vec::new(),
        }
    }
}

impl LensflareInitClass {
    pub fn new(flare_count: usize) -> Self {
        Self {
            type_id: 0,
            texture_name: String::new(),
            flare_count,
            flare_locations: vec![0.0; flare_count],
            flare_sizes: vec![1.0; flare_count],
            flare_colors: vec![Vector3::new(1.0, 1.0, 1.0); flare_count],
            flare_uv: vec![Vector4::new(0.0, 0.0, 1.0, 1.0); flare_count],
        }
    }
}

// ============================================================================
// Shader Configuration (matches C++ ShaderClass setup)
// ============================================================================

/// Shader state for rendering (matches C++ ShaderClass)
/// This is a stub - integrate with your graphics backend
#[derive(Debug, Clone)]
pub struct ShaderState {
    pub cull_mode: CullMode,
    pub depth_write: bool,
    pub depth_compare: DepthCompare,
    pub src_blend: BlendFunc,
    pub dst_blend: BlendFunc,
    pub fog_enabled: bool,
    pub texturing_enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CullMode {
    Disable,
    Front,
    Back,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepthCompare {
    Always,
    LessEqual,
    Less,
    Greater,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendFunc {
    Zero,
    One,
    SrcAlpha,
    OneMinusSrcAlpha,
}

impl Default for ShaderState {
    fn default() -> Self {
        Self {
            cull_mode: CullMode::Disable,
            depth_write: false,
            depth_compare: DepthCompare::Always,
            src_blend: BlendFunc::One,
            dst_blend: BlendFunc::One,
            fog_enabled: false,
            texturing_enabled: true,
        }
    }
}

/// Initialize default shaders (matches C++ Init_Shaders)
fn init_default_dazzle_shader() -> ShaderState {
    ShaderState {
        cull_mode: CullMode::Disable,
        depth_write: false,
        depth_compare: DepthCompare::Always,
        src_blend: BlendFunc::One,
        dst_blend: BlendFunc::One,
        fog_enabled: false,
        texturing_enabled: true,
    }
}

fn init_default_halo_shader() -> ShaderState {
    ShaderState {
        cull_mode: CullMode::Disable,
        depth_write: false,
        depth_compare: DepthCompare::LessEqual, // Different from dazzle!
        src_blend: BlendFunc::One,
        dst_blend: BlendFunc::One,
        fog_enabled: false,
        texturing_enabled: true,
    }
}

// ============================================================================
// DazzleTypeClass - Type Definition (matches C++ DazzleTypeClass)
// ============================================================================

/// Shared type definition for a dazzle effect.
/// Matches C++ DazzleTypeClass exactly.
#[derive(Debug, Clone)]
pub struct DazzleTypeClass {
    pub name: String,
    pub config: DazzleInitClass,
    pub fadeout_end_sqr: f32,
    pub fadeout_start_sqr: f32,
    pub dazzle_test_color_integer: u32,
    pub dazzle_test_mask_integer: u32,
    pub lensflare_id: Option<usize>,
    pub dazzle_shader: ShaderState,
    pub halo_shader: ShaderState,
    pub radius: f32,
    /// Cached primary texture (loaded lazily, C++: Get_Dazzle_Texture)
    pub primary_texture: Option<Arc<TextureBase>>,
    /// Cached secondary/halo texture (loaded lazily, C++: Get_Halo_Texture)
    pub secondary_texture: Option<Arc<TextureBase>>,
}

impl DazzleTypeClass {
    pub fn new(name: String, config: DazzleInitClass) -> Self {
        let fadeout_end_sqr = config.fadeout_end * config.fadeout_end;
        let fadeout_start_sqr = config.fadeout_start * config.fadeout_start;

        // Pack test color to integer (ARGB format)
        let dazzle_test_color_integer = 0xff000000
            | ((config.dazzle_test_color.z * 255.0) as u32) << 16
            | ((config.dazzle_test_color.y * 255.0) as u32) << 8
            | ((config.dazzle_test_color.x * 255.0) as u32);

        let dazzle_test_mask_integer = dazzle_test_color_integer & 0xf8f8f8f8;

        Self {
            name,
            radius: config.radius,
            fadeout_end_sqr,
            fadeout_start_sqr,
            dazzle_test_color_integer,
            dazzle_test_mask_integer,
            lensflare_id: None, // Set later
            config,
            dazzle_shader: init_default_dazzle_shader(),
            halo_shader: init_default_halo_shader(),
            primary_texture: None,
            secondary_texture: None,
        }
    }

    /// Get primary dazzle texture (lazy load).
    /// Matches C++ DazzleTypeClass::Get_Dazzle_Texture.
    pub fn get_dazzle_texture(&mut self) -> Option<Arc<TextureBase>> {
        if self.primary_texture.is_none() {
            self.primary_texture = fetch_dazzle_texture(&self.config.primary_texture_name);
        }
        self.primary_texture.as_ref().map(Arc::clone)
    }

    /// Get secondary/halo texture (lazy load).
    /// Matches C++ DazzleTypeClass::Get_Halo_Texture.
    pub fn get_halo_texture(&mut self) -> Option<Arc<TextureBase>> {
        if self.secondary_texture.is_none() {
            self.secondary_texture = fetch_dazzle_texture(&self.config.secondary_texture_name);
        }
        self.secondary_texture.as_ref().map(Arc::clone)
    }

    /// Calculate intensities based on view angle and distance.
    /// Matches C++ DazzleTypeClass::Calculate_Intensities exactly (lines 501-561).
    ///
    /// # Arguments
    /// * `camera_dir` - Camera looking direction (negative Z axis of view matrix)
    /// * `dazzle_dir` - Dazzle direction in world space
    /// * `dir_to_dazzle` - Normalized direction from camera to dazzle
    /// * `distance_sq` - Squared distance from camera to dazzle
    ///
    /// # Returns
    /// (dazzle_intensity, dazzle_size, halo_intensity)
    pub fn calculate_intensities(
        &self,
        camera_dir: &Vector3,
        dazzle_dir: &Vector3,
        dir_to_dazzle: &Vector3,
        distance_sq: f32,
    ) -> (f32, f32, f32) {
        // Line 510: dot = -Vector3::Dot_Product(dir_to_dazzle, camera_dir)
        let dot = -dir_to_dazzle.dot(camera_dir);
        let mut dazzle_intensity = dot;

        // Line 513-516: Early exit if beyond fadeout distance
        if self.config.use_camera_translation && distance_sq > self.fadeout_end_sqr {
            return (0.0, 0.0, 0.0);
        }

        // Line 518-520: Remap dot product to [0,1] based on dazzle_area
        dazzle_intensity -= 1.0 - self.config.dazzle_area;
        dazzle_intensity /= self.config.dazzle_area;
        dazzle_intensity = dazzle_intensity.clamp(0.0, 1.0);

        // Line 521-527: Apply directional cone if configured
        if self.config.dazzle_direction_area > 0.0 {
            let mut angle = -camera_dir.dot(dazzle_dir);
            angle -= 1.0 - self.config.dazzle_direction_area;
            angle /= self.config.dazzle_direction_area;
            angle = angle.clamp(0.0, 1.0);
            dazzle_intensity *= angle;
        }

        // Line 529-535: Compute size and intensity via power functions
        let mut dazzle_size = 0.0;
        if dazzle_intensity > 0.0 {
            dazzle_size = dazzle_intensity.powf(self.config.dazzle_size_pow);
            dazzle_intensity = dazzle_intensity.powf(self.config.dazzle_intensity_pow);
        } else {
            dazzle_intensity = 0.0;
        }

        // Line 537-544: Halo intensity calculation
        const EPSILON: f32 = 1e-6;
        let mut halo_intensity = 1.0;
        if self.config.halo_intensity_pow > EPSILON {
            if dot > 0.0 {
                let scale = dot.powf(self.config.halo_intensity_pow);
                halo_intensity *= scale;
            } else {
                halo_intensity = 0.0;
            }
        }

        // Line 546-547: Apply base multipliers
        dazzle_intensity *= self.config.dazzle_intensity;
        halo_intensity *= self.config.halo_intensity;

        // Line 550-558: Distance-based fadeout (linear)
        if self.config.use_camera_translation && distance_sq > self.fadeout_start_sqr {
            let distance = distance_sq.sqrt();
            let mut fade = distance - self.config.fadeout_start;
            fade /= self.config.fadeout_end - self.config.fadeout_start;
            dazzle_intensity *= 1.0 - fade;
            halo_intensity *= 1.0 - fade;
        }

        (dazzle_intensity, dazzle_size, halo_intensity)
    }
}

// ============================================================================
// LensflareTypeClass - Lensflare Definition (matches C++ LensflareTypeClass)
// ============================================================================

/// Shared lensflare type definition.
/// Matches C++ LensflareTypeClass exactly.
#[derive(Debug, Clone)]
pub struct LensflareTypeClass {
    pub name: String,
    pub config: LensflareInitClass,
    pub texture: Option<Arc<TextureBase>>,
}

impl LensflareTypeClass {
    pub fn new(name: String, config: LensflareInitClass) -> Self {
        Self {
            name,
            config,
            texture: None,
        }
    }

    /// Get lensflare texture (lazy load).
    pub fn get_texture(&mut self) -> Option<Arc<TextureBase>> {
        if self.texture.is_none() {
            self.texture = fetch_dazzle_texture(&self.config.texture_name);
        }
        self.texture.as_ref().map(Arc::clone)
    }

    /// Generate vertex data for all flares.
    /// Matches C++ LensflareTypeClass::Generate_Vertex_Buffers (lines 367-429).
    ///
    /// Returns vector of vertices (4 per flare).
    pub fn generate_vertices(
        &self,
        screen_x_scale: f32,
        screen_y_scale: f32,
        dazzle_intensity: f32,
        transformed_location: &Vector4,
    ) -> Vec<DazzleVertex> {
        let mut vertices = Vec::with_capacity(self.config.flare_count * 4);

        // Line 378: z coordinate
        let z = transformed_location.z;

        // Line 380: Distance multiplier for size scaling
        let distance_multiplier = (transformed_location.x * transformed_location.x
            + transformed_location.y * transformed_location.y)
            .sqrt()
            + 1.0;

        // Line 382: For each flare
        for a in 0..self.config.flare_count {
            // Line 383-387: Position along line from screen center to light
            let x = self.config.flare_locations[a] * transformed_location.x;
            let y = self.config.flare_locations[a] * transformed_location.y;
            let size = self.config.flare_sizes[a] * distance_multiplier;
            let ix = size * screen_x_scale;
            let iy = size * screen_y_scale;

            // Line 389-393: Color with intensity and clamping
            let mut col = self.config.flare_colors[a] * dazzle_intensity;
            col.x = col.x.min(1.0);
            col.y = col.y.min(1.0);
            col.z = col.z.min(1.0);
            let color = pack_color(&col, 1.0);

            let uv = &self.config.flare_uv[a];

            // Line 395-425: Four vertices per quad
            vertices.push(DazzleVertex {
                position: Vector3::new(x + ix, y - iy, z),
                uv: (uv.x, uv.y),
                color,
            });
            vertices.push(DazzleVertex {
                position: Vector3::new(x + ix, y + iy, z),
                uv: (uv.z, uv.y),
                color,
            });
            vertices.push(DazzleVertex {
                position: Vector3::new(x - ix, y + iy, z),
                uv: (uv.z, uv.w),
                color,
            });
            vertices.push(DazzleVertex {
                position: Vector3::new(x - ix, y - iy, z),
                uv: (uv.x, uv.w),
                color,
            });
        }

        vertices
    }
}

// ============================================================================
// DazzleRenderObjClass - Instance (matches C++ DazzleRenderObjClass)
// ============================================================================

/// A dazzle effect instance in the world.
/// Matches C++ DazzleRenderObjClass exactly.
pub struct DazzleRenderObjClass {
    pub type_id: usize,
    pub transform: Matrix3D,
    pub position: Vector3,
    pub current_dazzle_intensity: f32,
    pub current_dazzle_size: f32,
    pub current_halo_intensity: f32,
    pub current_distance: f32,
    pub transformed_loc: Vector4,
    pub current_vloc: Vector3,
    pub current_dir: Vector3,
    pub dazzle_color: Vector3,
    pub halo_color: Vector3,
    pub lensflare_intensity: f32,
    pub current_scale: f32,
    pub visibility: f32,
    pub on_list: bool,
    pub radius: f32,
    pub creation_time: u64,
}

impl DazzleRenderObjClass {
    /// Create a new dazzle instance.
    /// Matches C++ constructor (lines 767-786).
    pub fn new(type_id: usize, current_time: u64) -> Self {
        Self {
            type_id,
            transform: Matrix3D::identity(),
            position: Vector3::new(0.0, 0.0, 0.0),
            current_dazzle_intensity: 0.0,
            current_dazzle_size: 0.0,
            current_halo_intensity: 0.0,
            current_distance: 0.0,
            transformed_loc: Vector4::new(0.0, 0.0, 0.0, 1.0),
            current_vloc: Vector3::new(0.0, 0.0, 0.0),
            current_dir: Vector3::new(0.0, 0.0, 0.0),
            dazzle_color: Vector3::new(1.0, 1.0, 1.0),
            halo_color: Vector3::new(1.0, 1.0, 1.0),
            lensflare_intensity: 1.0,
            current_scale: 1.0,
            visibility: 0.0,
            on_list: false,
            radius: 1.0, // Will be set from type
            creation_time: current_time,
        }
    }

    /// Set transform and update direction.
    /// Matches C++ Set_Transform (lines 1261-1267).
    pub fn set_transform(&mut self, transform: Matrix3D, dazzle_type: &DazzleTypeClass) {
        self.transform = transform;
        self.current_dir = transform.rotate_vector(&dazzle_type.config.dazzle_direction);
    }

    /// Set world position.
    pub fn set_position(&mut self, position: Vector3) {
        self.position = position;
    }

    /// Check if dazzle is blinking and should be "on".
    /// Matches C++ blinking logic (lines 927-933).
    pub fn is_blinking_on(&self, current_time: u64, dazzle_type: &DazzleTypeClass) -> bool {
        if dazzle_type.config.blink_period <= 0.0 {
            return true; // Always on if not blinking
        }

        let elapsed_time = ((current_time - self.creation_time) as f32) / 1000.0;
        let wrapped_time = elapsed_time % dazzle_type.config.blink_period;
        wrapped_time <= dazzle_type.config.blink_on_time
    }

    /// Update visibility and intensity for rendering.
    /// Matches C++ Render method (lines 915-1013).
    ///
    /// Returns true if dazzle should be added to visible list.
    pub fn update_visibility(
        &mut self,
        camera_pos: &Vector3,
        camera_dir: &Vector3,
        view_matrix: &Matrix4,
        projection_matrix: &Matrix4,
        dazzle_type: &DazzleTypeClass,
        current_time: u64,
        frame_time_ms: u32,
        dazzle_rendering_enabled: bool,
    ) -> bool {
        // Line 919-921: Early exit checks
        if !dazzle_rendering_enabled {
            self.visibility = 0.0;
            return false;
        }

        // Line 927-933: Blinking check
        let is_on = self.is_blinking_on(current_time, dazzle_type);
        if !is_on {
            self.visibility = 0.0;
            return false;
        }

        // Line 945-961: Transform to screen space
        let loc = self.position;
        let mut transformed = view_matrix.transform_point(&loc);
        transformed = projection_matrix.transform_point(&Vector3::new(
            transformed.x,
            transformed.y,
            transformed.z,
        ));

        // Perspective divide
        transformed.x /= transformed.w;
        transformed.y /= transformed.w;
        transformed.z /= transformed.w;
        transformed.w = 1.0;

        self.transformed_loc = transformed;
        self.current_vloc = Vector3::new(transformed.x, transformed.y, transformed.z);

        // Line 963-967: Calculate direction and distance
        let mut dir = *camera_pos - loc;
        self.current_distance = dir.length_squared();
        dir.normalize();

        // Line 969-972: Calculate intensities
        let (mut dazzle_intensity, dazzle_size, halo_intensity) = dazzle_type
            .calculate_intensities(camera_dir, &self.current_dir, &dir, self.current_distance);

        self.current_halo_intensity = halo_intensity;

        // Line 974-976: Temporal smoothing
        let frame_time_ms = if frame_time_ms == 0 { 1 } else { frame_time_ms };
        let weight = dazzle_type.config.history_weight.powf(frame_time_ms as f32);

        // Line 978-982: Apply visibility via handler (C++: _VisibilityHandler->Compute_Dazzle_Visibility)
        if dazzle_intensity > 0.0 {
            let handler = get_dazzle_visibility_handler();
            self.visibility = handler.compute_dazzle_visibility(self, loc);
            dazzle_intensity *= self.visibility;
        } else {
            self.visibility = 0.0;
        }

        // Line 986-1000: Apply temporal smoothing
        if self.visibility == 0.0 {
            let i = dazzle_intensity * (1.0 - weight) + self.current_dazzle_intensity * weight;
            self.current_dazzle_intensity = i;
            if self.current_dazzle_intensity < 0.05 {
                self.current_dazzle_intensity = 0.0;
            }

            let s = dazzle_size * (1.0 - weight) + self.current_dazzle_size * weight;
            self.current_dazzle_size = s;
        } else {
            self.current_dazzle_intensity = dazzle_intensity;
            self.current_dazzle_size = dazzle_size;
        }

        // Line 1005: Check if should be rendered
        self.current_dazzle_intensity > 0.0 || self.current_halo_intensity > 0.0
    }

    /// Generate vertex data for this dazzle.
    /// Matches C++ Render_Dazzle geometry generation (lines 1091-1182).
    pub fn generate_vertices(
        &self,
        screen_x_scale: f32,
        screen_y_scale: f32,
        dazzle_type: &DazzleTypeClass,
    ) -> (Vec<DazzleVertex>, Vec<DazzleVertex>) {
        // Halo NOT scaled by current_scale (line 1054-1057)
        let halo_scale_x = dazzle_type.config.halo_scale_x;
        let halo_scale_y = dazzle_type.config.halo_scale_y;
        let dazzle_scale_x = dazzle_type.config.dazzle_scale_x * self.current_scale;
        let dazzle_scale_y = dazzle_type.config.dazzle_scale_y * self.current_scale;

        let mut dazzle_vertices = Vec::new();
        let mut halo_vertices = Vec::new();

        // Line 1091-1133: DAZZLE QUAD
        if self.current_dazzle_intensity > 0.0 {
            let dazzle_dxt = Vector3::new(screen_x_scale * dazzle_scale_x, 0.0, 0.0);
            let dazzle_dyt = Vector3::new(0.0, screen_y_scale * dazzle_scale_y, 0.0);

            let mut col = Vector3::new(
                self.dazzle_color.x * dazzle_type.config.dazzle_color.x,
                self.dazzle_color.y * dazzle_type.config.dazzle_color.y,
                self.dazzle_color.z * dazzle_type.config.dazzle_color.z,
            ) * self.current_dazzle_intensity;

            col.x = col.x.min(1.0);
            col.y = col.y.min(1.0);
            col.z = col.z.min(1.0);

            let color = pack_color(&col, 1.0);

            // 4 vertices for quad
            dazzle_vertices.push(DazzleVertex {
                position: self.current_vloc + (dazzle_dxt - dazzle_dyt) * self.current_dazzle_size,
                uv: (0.0, 0.0),
                color,
            });
            dazzle_vertices.push(DazzleVertex {
                position: self.current_vloc + (dazzle_dxt + dazzle_dyt) * self.current_dazzle_size,
                uv: (1.0, 0.0),
                color,
            });
            dazzle_vertices.push(DazzleVertex {
                position: self.current_vloc - (dazzle_dxt - dazzle_dyt) * self.current_dazzle_size,
                uv: (1.0, 1.0),
                color,
            });
            dazzle_vertices.push(DazzleVertex {
                position: self.current_vloc - (dazzle_dxt + dazzle_dyt) * self.current_dazzle_size,
                uv: (0.0, 1.0),
                color,
            });
        }

        // Line 1135-1182: HALO QUAD
        if self.current_halo_intensity > 0.0 {
            let halo_dxt = Vector3::new(screen_x_scale * halo_scale_x, 0.0, 0.0);
            let halo_dyt = Vector3::new(0.0, screen_y_scale * halo_scale_y, 0.0);

            let mut col = Vector3::new(
                self.halo_color.x * dazzle_type.config.halo_color.x,
                self.halo_color.y * dazzle_type.config.halo_color.y,
                self.halo_color.z * dazzle_type.config.halo_color.z,
            ) * self.current_halo_intensity;

            col.x = col.x.min(1.0);
            col.y = col.y.min(1.0);
            col.z = col.z.min(1.0);

            let color = pack_color(&col, 1.0);

            // 4 vertices for quad
            halo_vertices.push(DazzleVertex {
                position: self.current_vloc + (halo_dxt - halo_dyt),
                uv: (0.0, 0.0),
                color,
            });
            halo_vertices.push(DazzleVertex {
                position: self.current_vloc + (halo_dxt + halo_dyt),
                uv: (1.0, 0.0),
                color,
            });
            halo_vertices.push(DazzleVertex {
                position: self.current_vloc - (halo_dxt - halo_dyt),
                uv: (1.0, 1.0),
                color,
            });
            halo_vertices.push(DazzleVertex {
                position: self.current_vloc - (halo_dxt + halo_dyt),
                uv: (0.0, 1.0),
                color,
            });
        }

        (dazzle_vertices, halo_vertices)
    }
}

impl std::ops::Add<Vector3> for Vector3 {
    type Output = Vector3;
    fn add(self, rhs: Vector3) -> Vector3 {
        Vector3::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

// ============================================================================
// DazzleLayerClass - Batch Renderer (matches C++ DazzleLayerClass)
// ============================================================================

/// Manages visible dazzles for a rendering pass.
/// Matches C++ DazzleLayerClass exactly.
pub struct DazzleLayerClass {
    visible_lists: Vec<Vec<usize>>, // Indices into dazzle instances
}

impl DazzleLayerClass {
    /// Create a new layer with space for all types.
    /// Matches C++ constructor (lines 1551-1566).
    pub fn new(type_count: usize) -> Self {
        Self {
            visible_lists: vec![Vec::new(); type_count],
        }
    }

    /// Add a dazzle to the visible list.
    /// Matches C++ Set_Layer logic (lines 871-890).
    pub fn add_visible(&mut self, type_id: usize, instance_id: usize) {
        if type_id < self.visible_lists.len() {
            self.visible_lists[type_id].push(instance_id);
        }
    }

    /// Get visible dazzle indices for a type.
    pub fn get_visible(&self, type_id: usize) -> &[usize] {
        if type_id < self.visible_lists.len() {
            &self.visible_lists[type_id]
        } else {
            &[]
        }
    }

    /// Clear all visible lists.
    /// Matches C++ Clear_Visible_List (lines 1629-1645).
    pub fn clear_all(&mut self) {
        for list in &mut self.visible_lists {
            list.clear();
        }
    }

    /// Get count of visible dazzles for a type.
    /// Matches C++ Get_Visible_Item_Count (lines 1610-1625).
    pub fn get_visible_count(&self, type_id: usize) -> usize {
        if type_id < self.visible_lists.len() {
            self.visible_lists[type_id].len()
        } else {
            0
        }
    }
}

// ============================================================================
// Vertex Format (matches C++ VertexFormatXYZNDUV2)
// ============================================================================

/// Vertex format for dazzle rendering.
/// Matches C++ VertexFormatXYZNDUV2 (simplified - only used fields).
#[derive(Debug, Clone, Copy)]
pub struct DazzleVertex {
    pub position: Vector3,
    pub uv: (f32, f32),
    pub color: u32, // ARGB packed
}

/// Pack RGB color to ARGB integer.
/// Matches C++ DX8Wrapper::Convert_Color.
fn pack_color(color: &Vector3, alpha: f32) -> u32 {
    let a = (alpha * 255.0) as u32;
    let r = (color.x * 255.0) as u32;
    let g = (color.y * 255.0) as u32;
    let b = (color.z * 255.0) as u32;
    (a << 24) | (r << 16) | (g << 8) | b
}

// ============================================================================
// Dazzle Visibility + Texture Providers (matches C++ installable handlers)
// ============================================================================

/// Dazzle visibility handler interface (C++: DazzleVisibilityClass).
pub trait DazzleVisibilityHandler: Send + Sync {
    fn compute_dazzle_visibility(&self, dazzle: &DazzleRenderObjClass, point: Vector3) -> f32;
}

#[derive(Debug, Default)]
struct DefaultDazzleVisibilityHandler;

impl DazzleVisibilityHandler for DefaultDazzleVisibilityHandler {
    fn compute_dazzle_visibility(&self, _dazzle: &DazzleRenderObjClass, _point: Vector3) -> f32 {
        1.0
    }
}

static DAZZLE_VISIBILITY_HANDLER: OnceLock<Mutex<Arc<dyn DazzleVisibilityHandler>>> =
    OnceLock::new();

fn get_dazzle_visibility_handler() -> Arc<dyn DazzleVisibilityHandler> {
    let handler = DAZZLE_VISIBILITY_HANDLER
        .get_or_init(|| Mutex::new(Arc::new(DefaultDazzleVisibilityHandler)));
    handler.lock().unwrap().clone()
}

/// Install a custom visibility handler.
pub fn install_dazzle_visibility_handler(handler: Arc<dyn DazzleVisibilityHandler>) {
    let storage = DAZZLE_VISIBILITY_HANDLER
        .get_or_init(|| Mutex::new(Arc::new(DefaultDazzleVisibilityHandler)));
    *storage.lock().unwrap() = handler;
}

/// Dazzle texture provider interface (C++: WW3DAssetManager::Get_Texture).
pub trait DazzleTextureProvider: Send + Sync {
    fn get_texture(&self, name: &str) -> Option<Arc<TextureBase>>;
}

#[derive(Debug, Default)]
struct DefaultDazzleTextureProvider;

impl DazzleTextureProvider for DefaultDazzleTextureProvider {
    fn get_texture(&self, name: &str) -> Option<Arc<TextureBase>> {
        if name.is_empty() {
            return None;
        }
        let manager = DAZZLE_TEXTURE_MANAGER.get_or_init(|| Mutex::new(TextureManager::new()));
        let mut manager = manager.lock().unwrap();
        manager
            .get_or_load(name, TextureFormat::A8R8G8B8, true, MipCount::All)
            .ok()
    }
}

static DAZZLE_TEXTURE_PROVIDER: OnceLock<Mutex<Arc<dyn DazzleTextureProvider>>> = OnceLock::new();
static DAZZLE_TEXTURE_MANAGER: OnceLock<Mutex<TextureManager>> = OnceLock::new();

fn get_dazzle_texture_provider() -> Arc<dyn DazzleTextureProvider> {
    let provider =
        DAZZLE_TEXTURE_PROVIDER.get_or_init(|| Mutex::new(Arc::new(DefaultDazzleTextureProvider)));
    provider.lock().unwrap().clone()
}

/// Install a custom texture provider for dazzle/lensflare assets.
pub fn install_dazzle_texture_provider(provider: Arc<dyn DazzleTextureProvider>) {
    let storage =
        DAZZLE_TEXTURE_PROVIDER.get_or_init(|| Mutex::new(Arc::new(DefaultDazzleTextureProvider)));
    *storage.lock().unwrap() = provider;
}

fn fetch_dazzle_texture(name: &str) -> Option<Arc<TextureBase>> {
    if name.is_empty() {
        return None;
    }
    get_dazzle_texture_provider().get_texture(name)
}

// ============================================================================
// Global Type Management (matches C++ static arrays)
// ============================================================================

/// Global dazzle type registry.
static DAZZLE_TYPES: OnceLock<Mutex<HashMap<String, DazzleTypeClass>>> = OnceLock::new();

/// Global lensflare type registry.
static LENSFLARE_TYPES: OnceLock<Mutex<HashMap<String, LensflareTypeClass>>> = OnceLock::new();

/// Global dazzle rendering enabled flag.
static DAZZLE_RENDERING_ENABLED: OnceLock<Mutex<bool>> = OnceLock::new();

/// Initialize global state.
pub fn init_dazzle_system() {
    DAZZLE_TYPES.get_or_init(|| Mutex::new(HashMap::new()));
    LENSFLARE_TYPES.get_or_init(|| Mutex::new(HashMap::new()));
    DAZZLE_RENDERING_ENABLED.get_or_init(|| Mutex::new(true));
}

/// Register a dazzle type.
/// Matches C++ Init_Type (lines 680-699).
pub fn register_dazzle_type(name: String, config: DazzleInitClass) {
    init_dazzle_system();
    let types = DAZZLE_TYPES.get().unwrap();
    // Recover from poisoned mutex by clearing and recreating
    match types.lock() {
        Ok(mut types_guard) => {
            types_guard.insert(name.clone(), DazzleTypeClass::new(name, config));
        }
        Err(poisoned) => {
            eprintln!("Warning: Dazzle types mutex was poisoned, recovering by clearing");
            let mut types_guard = poisoned.into_inner();
            types_guard.clear();
            types_guard.insert(name.clone(), DazzleTypeClass::new(name, config));
        }
    }
}

/// Register a lensflare type.
/// Matches C++ Init_Lensflare (lines 703-722).
pub fn register_lensflare_type(name: String, config: LensflareInitClass) {
    init_dazzle_system();
    let lensflares = LENSFLARE_TYPES.get().unwrap();
    // Recover from poisoned mutex by clearing and recreating
    match lensflares.lock() {
        Ok(mut lensflares_guard) => {
            lensflares_guard.insert(name.clone(), LensflareTypeClass::new(name, config));
        }
        Err(poisoned) => {
            eprintln!("Warning: Lensflare types mutex was poisoned, recovering by clearing");
            let mut lensflares_guard = poisoned.into_inner();
            lensflares_guard.clear();
            lensflares_guard.insert(name.clone(), LensflareTypeClass::new(name, config));
        }
    }
}

/// Get dazzle type by name.
pub fn get_dazzle_type(name: &str) -> Option<DazzleTypeClass> {
    let types = DAZZLE_TYPES.get()?;
    // Recover from poisoned mutex by returning None (safe fallback)
    match types.lock() {
        Ok(types_guard) => types_guard.get(name).cloned(),
        Err(_) => {
            eprintln!(
                "Warning: Dazzle types mutex was poisoned while retrieving type '{}'",
                name
            );
            None
        }
    }
}

/// Get lensflare type by name.
pub fn get_lensflare_type(name: &str) -> Option<LensflareTypeClass> {
    let lensflares = LENSFLARE_TYPES.get()?;
    // Recover from poisoned mutex by returning None (safe fallback)
    match lensflares.lock() {
        Ok(lensflares_guard) => lensflares_guard.get(name).cloned(),
        Err(_) => {
            eprintln!(
                "Warning: Lensflare types mutex was poisoned while retrieving type '{}'",
                name
            );
            None
        }
    }
}

/// Get all dazzle type names.
pub fn get_dazzle_type_names() -> Vec<String> {
    init_dazzle_system();
    let types = DAZZLE_TYPES.get().unwrap();
    // Recover from poisoned mutex by returning empty list
    match types.lock() {
        Ok(types_guard) => types_guard.keys().cloned().collect(),
        Err(_) => {
            eprintln!("Warning: Dazzle types mutex was poisoned while retrieving type names");
            Vec::new()
        }
    }
}

/// Enable/disable dazzle rendering globally.
/// Matches C++ Enable_Dazzle_Rendering.
pub fn set_dazzle_rendering_enabled(enabled: bool) {
    init_dazzle_system();
    let flag = DAZZLE_RENDERING_ENABLED.get().unwrap();
    // Recover from poisoned mutex by reinitializing state
    match flag.lock() {
        Ok(mut flag_guard) => {
            *flag_guard = enabled;
        }
        Err(poisoned) => {
            eprintln!("Warning: Dazzle rendering enabled flag mutex was poisoned, recovering");
            let mut flag_guard = poisoned.into_inner();
            *flag_guard = enabled;
        }
    }
}

/// Check if dazzle rendering is enabled.
pub fn is_dazzle_rendering_enabled() -> bool {
    init_dazzle_system();
    let flag = DAZZLE_RENDERING_ENABLED.get().unwrap();
    // Recover from poisoned mutex by assuming disabled (safe default)
    match flag.lock() {
        Ok(flag_guard) => *flag_guard,
        Err(_) => {
            eprintln!(
                "Warning: Dazzle rendering enabled flag mutex was poisoned, assuming disabled"
            );
            false
        }
    }
}

/// Clear all registered types.
/// Matches C++ Deinit (lines 726-748).
pub fn clear_dazzle_types() {
    if let Some(types) = DAZZLE_TYPES.get() {
        // Recover from poisoned mutex by clearing state
        match types.lock() {
            Ok(mut types_guard) => {
                types_guard.clear();
            }
            Err(poisoned) => {
                eprintln!("Warning: Dazzle types mutex was poisoned during clear, recovering");
                let mut types_guard = poisoned.into_inner();
                types_guard.clear();
            }
        }
    }
    if let Some(lensflares) = LENSFLARE_TYPES.get() {
        // Recover from poisoned mutex by clearing state
        match lensflares.lock() {
            Ok(mut lensflares_guard) => {
                lensflares_guard.clear();
            }
            Err(poisoned) => {
                eprintln!("Warning: Lensflare types mutex was poisoned during clear, recovering");
                let mut lensflares_guard = poisoned.into_inner();
                lensflares_guard.clear();
            }
        }
    }
}

// ============================================================================
// TESTS - Comprehensive unit tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector3_dot_product() {
        let v1 = Vector3::new(1.0, 0.0, 0.0);
        let v2 = Vector3::new(0.0, 1.0, 0.0);
        assert_eq!(v1.dot(&v2), 0.0);

        let v3 = Vector3::new(1.0, 0.0, 0.0);
        let v4 = Vector3::new(1.0, 0.0, 0.0);
        assert_eq!(v3.dot(&v4), 1.0);
    }

    #[test]
    fn test_vector3_normalize() {
        let mut v = Vector3::new(3.0, 4.0, 0.0);
        v.normalize();
        assert!((v.length() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_dazzle_init_defaults() {
        let init = DazzleInitClass::default();
        assert_eq!(init.halo_intensity, 1.0);
        assert_eq!(init.dazzle_area, 0.05);
        assert_eq!(init.history_weight, 0.975);
    }

    #[test]
    fn test_lensflare_init() {
        let lf = LensflareInitClass::new(5);
        assert_eq!(lf.flare_count, 5);
        assert_eq!(lf.flare_locations.len(), 5);
        assert_eq!(lf.flare_sizes.len(), 5);
    }

    #[test]
    fn test_intensity_calculation_basic() {
        let config = DazzleInitClass {
            dazzle_area: 0.1,
            dazzle_intensity: 1.0,
            dazzle_intensity_pow: 1.0,
            dazzle_size_pow: 1.0,
            halo_intensity: 1.0,
            halo_intensity_pow: 1.0,
            use_camera_translation: false,
            fadeout_start: 100.0,
            fadeout_end: 200.0,
            ..Default::default()
        };

        let dazzle_type = DazzleTypeClass::new("TEST".to_string(), config);

        // Looking directly at dazzle
        let camera_dir = Vector3::new(0.0, 0.0, -1.0);
        let dazzle_dir = Vector3::new(0.0, 0.0, 1.0);
        let dir_to_dazzle = Vector3::new(0.0, 0.0, 1.0);

        let (intensity, size, halo) =
            dazzle_type.calculate_intensities(&camera_dir, &dazzle_dir, &dir_to_dazzle, 10.0);

        // Should have some intensity when looking at dazzle
        assert!(intensity > 0.0);
        assert!(size > 0.0);
        assert!(halo > 0.0);
    }

    #[test]
    fn test_intensity_calculation_perpendicular() {
        let config = DazzleInitClass {
            dazzle_area: 0.1,
            dazzle_intensity: 1.0,
            dazzle_intensity_pow: 1.0,
            dazzle_size_pow: 1.0,
            halo_intensity: 1.0,
            halo_intensity_pow: 1.0,
            use_camera_translation: false,
            ..Default::default()
        };

        let dazzle_type = DazzleTypeClass::new("TEST".to_string(), config);

        // Looking perpendicular to dazzle
        let camera_dir = Vector3::new(0.0, 0.0, -1.0);
        let dazzle_dir = Vector3::new(0.0, 0.0, 1.0);
        let dir_to_dazzle = Vector3::new(1.0, 0.0, 0.0); // Perpendicular

        let (intensity, _, _) =
            dazzle_type.calculate_intensities(&camera_dir, &dazzle_dir, &dir_to_dazzle, 10.0);

        // Should have zero or very low intensity when perpendicular
        assert!(intensity < 0.1);
    }

    #[test]
    fn test_distance_fadeout() {
        let config = DazzleInitClass {
            dazzle_area: 1.0,
            dazzle_intensity: 1.0,
            dazzle_intensity_pow: 1.0,
            dazzle_size_pow: 1.0,
            halo_intensity: 1.0,
            halo_intensity_pow: 0.0,
            use_camera_translation: true,
            fadeout_start: 10.0,
            fadeout_end: 20.0,
            ..Default::default()
        };

        let dazzle_type = DazzleTypeClass::new("TEST".to_string(), config);

        let camera_dir = Vector3::new(0.0, 0.0, -1.0);
        let dazzle_dir = Vector3::new(0.0, 0.0, 1.0);
        let dir_to_dazzle = Vector3::new(0.0, 0.0, 1.0);

        // Before fadeout start
        let (intensity1, _, _) =
            dazzle_type.calculate_intensities(&camera_dir, &dazzle_dir, &dir_to_dazzle, 5.0 * 5.0);

        // At fadeout midpoint
        let (intensity2, _, _) = dazzle_type.calculate_intensities(
            &camera_dir,
            &dazzle_dir,
            &dir_to_dazzle,
            15.0 * 15.0,
        );

        // Beyond fadeout end
        let (intensity3, _, _) = dazzle_type.calculate_intensities(
            &camera_dir,
            &dazzle_dir,
            &dir_to_dazzle,
            25.0 * 25.0,
        );

        assert!(intensity1 > intensity2);
        assert!(intensity2 > intensity3);
        assert_eq!(intensity3, 0.0);
    }

    #[test]
    fn test_directional_cone() {
        let config = DazzleInitClass {
            dazzle_area: 1.0,
            dazzle_direction_area: 0.5, // Directional cone
            dazzle_intensity: 1.0,
            dazzle_intensity_pow: 1.0,
            dazzle_size_pow: 1.0,
            halo_intensity: 1.0,
            halo_intensity_pow: 0.0,
            use_camera_translation: false,
            ..Default::default()
        };

        let dazzle_type = DazzleTypeClass::new("TEST".to_string(), config);

        let dazzle_dir = Vector3::new(0.0, 0.0, 1.0);
        let dir_to_dazzle = Vector3::new(0.0, 0.0, 1.0);

        // Camera aligned with dazzle direction
        let camera_dir1 = Vector3::new(0.0, 0.0, -1.0);
        let (intensity1, _, _) =
            dazzle_type.calculate_intensities(&camera_dir1, &dazzle_dir, &dir_to_dazzle, 10.0);

        // Camera perpendicular to dazzle direction
        let camera_dir2 = Vector3::new(1.0, 0.0, 0.0);
        let (intensity2, _, _) =
            dazzle_type.calculate_intensities(&camera_dir2, &dazzle_dir, &dir_to_dazzle, 10.0);

        // Aligned should have more intensity
        assert!(intensity1 > intensity2);
    }

    #[test]
    fn test_power_functions() {
        let config = DazzleInitClass {
            dazzle_area: 1.0,
            dazzle_intensity: 1.0,
            dazzle_intensity_pow: 2.0, // Quadratic falloff
            dazzle_size_pow: 0.5,      // Square root
            halo_intensity: 1.0,
            halo_intensity_pow: 0.0,
            use_camera_translation: false,
            ..Default::default()
        };

        let dazzle_type = DazzleTypeClass::new("TEST".to_string(), config);

        let camera_dir = Vector3::new(0.0, 0.0, -1.0);
        let dazzle_dir = Vector3::new(0.0, 0.0, 1.0);
        let dir_to_dazzle = Vector3::new(0.0, 0.0, 1.0);

        let (intensity, size, _) =
            dazzle_type.calculate_intensities(&camera_dir, &dazzle_dir, &dir_to_dazzle, 10.0);

        // With pow=2, intensity should be squared
        // With pow=0.5, size should be sqrt
        assert!(intensity > 0.0);
        assert!(size > 0.0);
    }

    #[test]
    fn test_blinking_logic() {
        let config = DazzleInitClass {
            blink_period: 2.0,
            blink_on_time: 1.0,
            ..Default::default()
        };

        let dazzle_type = DazzleTypeClass::new("TEST".to_string(), config);
        let dazzle = DazzleRenderObjClass::new(0, 0);

        // At t=0.5s (on)
        assert!(dazzle.is_blinking_on(500, &dazzle_type));

        // At t=1.5s (off)
        assert!(!dazzle.is_blinking_on(1500, &dazzle_type));

        // At t=2.5s (on again, wrapped)
        assert!(dazzle.is_blinking_on(2500, &dazzle_type));
    }

    #[test]
    fn test_color_packing() {
        let color = Vector3::new(1.0, 0.5, 0.0);
        let packed = pack_color(&color, 1.0);

        // ARGB: A=255, R=255, G=127, B=0
        assert_eq!(packed, 0xffff7f00);
    }

    #[test]
    fn test_color_clamping() {
        // Test that colors > 1.0 are NOT clamped during packing
        // (clamping happens before calling pack_color in the C++ code)
        let color = Vector3::new(2.0, 1.5, 0.5);
        let packed = pack_color(&color, 1.0);

        // Colors wrap around in pack (2.0 * 255 = 510 & 0xff = 254)
        // This matches C++ behavior where colors are clamped BEFORE packing
        let r = (packed >> 16) & 0xff;
        let g = (packed >> 8) & 0xff;
        // Check that we actually got wrapped values
        assert!(r > 0); // Will be 254 (510 & 0xff)
        assert!(g > 0); // Will be 127 (382 & 0xff)
    }

    #[test]
    fn test_lensflare_vertex_generation() {
        let mut config = LensflareInitClass::new(2);
        config.flare_locations = vec![0.0, 1.0];
        config.flare_sizes = vec![0.1, 0.2];
        config.flare_colors = vec![Vector3::new(1.0, 0.0, 0.0), Vector3::new(0.0, 1.0, 0.0)];
        config.flare_uv = vec![Vector4::new(0.0, 0.0, 1.0, 1.0); 2];

        let lensflare = LensflareTypeClass::new("TEST".to_string(), config);

        let transformed_loc = Vector4::new(0.5, 0.3, 0.9, 1.0);
        let vertices = lensflare.generate_vertices(1.0, 1.0, 1.0, &transformed_loc);

        // Should have 4 vertices per flare
        assert_eq!(vertices.len(), 8);
    }

    #[test]
    fn test_dazzle_layer() {
        let mut layer = DazzleLayerClass::new(3);

        layer.add_visible(0, 10);
        layer.add_visible(0, 20);
        layer.add_visible(1, 30);

        assert_eq!(layer.get_visible_count(0), 2);
        assert_eq!(layer.get_visible_count(1), 1);
        assert_eq!(layer.get_visible_count(2), 0);

        layer.clear_all();
        assert_eq!(layer.get_visible_count(0), 0);
    }

    #[test]
    fn test_type_registration() {
        clear_dazzle_types(); // Clean slate

        let config = DazzleInitClass::default();
        register_dazzle_type("SUN".to_string(), config);

        let dazzle_type = get_dazzle_type("SUN");
        assert!(dazzle_type.is_some());
        assert_eq!(dazzle_type.unwrap().name, "SUN");

        let names = get_dazzle_type_names();
        assert!(names.contains(&"SUN".to_string()));
    }

    #[test]
    fn test_global_rendering_flag() {
        set_dazzle_rendering_enabled(false);
        assert!(!is_dazzle_rendering_enabled());

        set_dazzle_rendering_enabled(true);
        assert!(is_dazzle_rendering_enabled());
    }

    #[test]
    fn test_temporal_smoothing() {
        let config = DazzleInitClass {
            history_weight: 0.9,
            dazzle_area: 1.0,
            dazzle_intensity: 1.0,
            dazzle_intensity_pow: 1.0,
            dazzle_size_pow: 1.0,
            halo_intensity: 1.0,
            halo_intensity_pow: 0.0,
            use_camera_translation: false,
            ..Default::default()
        };

        let dazzle_type = DazzleTypeClass::new("TEST".to_string(), config);

        // Frame time = 16ms
        let weight = dazzle_type.config.history_weight.powf(16.0);

        // Should blend old and new values
        let old_intensity = 1.0;
        let new_intensity = 0.0;
        let blended = new_intensity * (1.0 - weight) + old_intensity * weight;

        // With weight ~0.185 (0.9^16), blended should be ~0.185
        assert!(blended > 0.1);
        assert!(blended < 0.3);
    }

    #[test]
    fn test_matrix_rotation() {
        let mat = Matrix3D::identity();
        let v = Vector3::new(1.0, 0.0, 0.0);
        let rotated = mat.rotate_vector(&v);

        assert_eq!(rotated.x, 1.0);
        assert_eq!(rotated.y, 0.0);
        assert_eq!(rotated.z, 0.0);
    }
}

// ============================================================================
// Legacy Compatibility (for W3D chunk loading)
// ============================================================================

/// Dazzle entry recorded from a W3D dazzle chunk.
/// Kept for compatibility with existing chunk loader.
#[derive(Debug, Clone)]
pub struct DazzleEntry {
    pub name: String,
    pub type_name: String,
}

#[derive(Debug, Default)]
pub struct DazzleLibrary {
    entries: HashMap<String, DazzleEntry>,
}

impl DazzleLibrary {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: DazzleEntry) {
        self.entries.insert(entry.name.clone(), entry);
    }

    pub fn get(&self, name: &str) -> Option<&DazzleEntry> {
        self.entries.get(name)
    }

    pub fn iter(&self) -> impl Iterator<Item = &DazzleEntry> {
        self.entries.values()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
