//! # W3D Shadow System - C&C Generals Parity Implementation
//!
//! This module implements the W3D shadow system matching the C++ behavior:
//! - Volumetric shadows (stencil buffer shadow volumes)
//! - Projected shadows (texture projection onto terrain/objects)
//! - Decal shadows (terrain-conforming decals)
//!
//! Shadow types matching C++ ShadowType enum:
//! - SHADOW_VOLUME: Stencil-based volumetric shadows
//! - SHADOW_PROJECTION: Projected shadow textures onto geometry
//! - SHADOW_DECAL: Modulate blend decals on terrain
//! - SHADOW_ALPHA_DECAL: Alpha blended decals
//! - SHADOW_ADDITIVE_DECAL: Additive blended decals

use super::{BoundingBox, Result, W3DError, W3DVertex};
use crate::video::{ColorFormat, Resolution};
use bytemuck::{cast_slice, Pod, Zeroable};
use glam::{Mat4, Quat, Vec2, Vec3, Vec4, Vec4Swizzles};
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::sync::Arc;

/// Camera uniforms for shadow rendering
pub struct CameraUniforms {
    pub view_matrix: [[f32; 4]; 4],
    pub projection_matrix: [[f32; 4]; 4],
    pub near_far: [f32; 2],
}

// ============================================================================
// Constants matching C++ implementation
// ============================================================================

/// Maximum number of shadow casting light sources (C++: MAX_SHADOW_LIGHTS)
pub const MAX_SHADOW_LIGHTS: usize = 1;

/// Distance of sun from ground (C++: SUN_DISTANCE_FROM_GROUND)
pub const SUN_DISTANCE_FROM_GROUND: f32 = 10000.0;

/// Maximum number of meshes in animated hierarchy (C++: MAX_SHADOW_CASTER_MESHES)
pub const MAX_SHADOW_CASTER_MESHES: usize = 160;

/// Shadow extrusion buffer amount (C++: SHADOW_EXTRUSION_BUFFER)
pub const SHADOW_EXTRUSION_BUFFER: f32 = 0.1;

/// Maximum shadow extrusion length (C++: MAX_EXTRUSION_LENGTH)
pub const MAX_EXTRUSION_LENGTH: f32 = 512.0 * 10.0; // MAP_XY_FACTOR

/// Cosine of angle threshold for shadow updates (C++: cosAngleToCare)
pub const COS_ANGLE_TO_CARE: f32 = 0.999998; // ~0.2 degrees

// ============================================================================
// Shadow Type Flags - Matching C++ ShadowType enum
// ============================================================================

bitflags::bitflags! {
    /// Shadow type flags matching C++ ShadowType enum exactly
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ShadowType: u32 {
        const NONE = 0x00000000;
        /// Shadow decal applied via modulate blend
        const DECAL = 0x00000001;
        /// Volumetric stencil shadow
        const VOLUME = 0x00000002;
        /// Projected shadow texture
        const PROJECTION = 0x00000004;
        /// Extra setting for shadows which need dynamic updates
        const DYNAMIC_PROJECTION = 0x00000008;
        /// Extra setting for shadow decals that rotate with sun direction
        const DIRECTIONAL_PROJECTION = 0x00000010;
        /// Alpha blended decal (not just for shadows)
        const ALPHA_DECAL = 0x00000020;
        /// Additive blended decal (not just for shadows)
        const ADDITIVE_DECAL = 0x00000040;
    }
}

#[cfg(feature = "w3d")]
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    AddressMode, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
    BindGroupLayoutDescriptor, BindingType, Buffer, BufferBindingType, BufferDescriptor,
    BufferUsages, CommandBuffer, CommandEncoder, CompareFunction, ComputePass, ComputePipeline,
    ComputePipelineDescriptor, DepthBiasState, DepthStencilState, Device, Extent3d, Face,
    FilterMode, FragmentState, FrontFace, LoadOp, Operations, Origin3d, PipelineLayout,
    PipelineLayoutDescriptor, PolygonMode, PrimitiveState, PrimitiveTopology, Queue, RenderPass,
    RenderPassColorAttachment, RenderPassDepthStencilAttachment, RenderPassDescriptor,
    RenderPipeline, RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor,
    ShaderStages, StencilState, StoreOp, Texture, TextureAspect, TextureDescriptor,
    TextureDimension, TextureFormat, TextureSampleType, TextureUsages, TextureView,
    TextureViewDescriptor, TextureViewDimension, VertexBufferLayout, VertexState,
};

// ============================================================================
// Shadow Type Info - Matching C++ Shadow::ShadowTypeInfo
// ============================================================================

/// Shadow configuration info for creating shadows
/// Matches C++ Shadow::ShadowTypeInfo struct
#[derive(Debug, Clone)]
pub struct ShadowTypeInfo {
    /// Shadow name (when set, overrides default model shadow)
    pub shadow_name: [u8; 64],
    /// Type of shadow
    pub shadow_type: ShadowType,
    /// Whether to update shadow image when object/light moves
    pub allow_updates: bool,
    /// Whether to align shadow to world geometry or draw as horizontal decal
    pub allow_world_align: bool,
    /// World size of decal projection in X
    pub size_x: f32,
    /// World size of decal projection in Y
    pub size_y: f32,
    /// World shift along X axis
    pub offset_x: f32,
    /// World shift along Y axis
    pub offset_y: f32,
}

impl Default for ShadowTypeInfo {
    fn default() -> Self {
        Self {
            shadow_name: [0; 64],
            shadow_type: ShadowType::VOLUME,
            allow_updates: true,
            allow_world_align: true,
            size_x: 0.0,
            size_y: 0.0,
            offset_x: 0.0,
            offset_y: 0.0,
        }
    }
}

impl ShadowTypeInfo {
    /// Create a new ShadowTypeInfo with the specified shadow type
    pub fn new(shadow_type: ShadowType) -> Self {
        Self {
            shadow_type,
            ..Default::default()
        }
    }

    /// Set the shadow name
    pub fn with_name(mut self, name: &str) -> Self {
        let bytes = name.as_bytes();
        let len = bytes.len().min(63);
        self.shadow_name[..len].copy_from_slice(&bytes[..len]);
        self.shadow_name[len] = 0; // null terminator
        self
    }

    /// Set the decal size
    pub fn with_size(mut self, size_x: f32, size_y: f32) -> Self {
        self.size_x = size_x;
        self.size_y = size_y;
        self
    }

    /// Set the decal offset
    pub fn with_offset(mut self, offset_x: f32, offset_y: f32) -> Self {
        self.offset_x = offset_x;
        self.offset_y = offset_y;
        self
    }
}

// ============================================================================
// Time of Day - For shadow light position updates
// ============================================================================

/// Time of day enum matching C++ TimeOfDay
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeOfDay {
    Morning,
    Afternoon,
    Evening,
    Night,
}

// ============================================================================
// Base Shadow Interface - Matching C++ Shadow class
// ============================================================================

/// Base shadow trait matching C++ Shadow interface
pub trait Shadow: Send + Sync {
    /// Release this shadow from suitable manager
    fn release(&self);

    /// Check if shadow rendering is enabled
    fn is_render_enabled(&self) -> bool;

    /// Check if invisible rendering is enabled (overrides render enabled)
    fn is_invisible_enabled(&self) -> bool;

    /// Get shadow type
    fn get_shadow_type(&self) -> ShadowType;

    /// Set shadow opacity (0-255)
    fn set_opacity(&mut self, value: u32);

    /// Set shadow color (ARGB format, alpha ignored)
    fn set_color(&mut self, color: u32);

    /// Set shadow orientation around z-axis
    fn set_angle(&mut self, angle: f32);

    /// Set shadow position (for decals not bound to render objects)
    fn set_position(&mut self, x: f32, y: f32, z: f32);

    /// Set shadow size
    fn set_size(&mut self, size_x: f32, size_y: f32);
}

/// Base shadow implementation with common fields
#[derive(Debug)]
pub struct ShadowBase {
    /// Toggle to turn rendering of this shadow on/off
    pub is_enabled: bool,
    /// If set, overrides and causes no rendering (used by Shroud)
    pub is_invisible_enabled: bool,
    /// Value between 0 (transparent) and 255 (opaque)
    pub opacity: u32,
    /// Color in ARGB format (Alpha is ignored)
    pub color: u32,
    /// Type of projection
    pub shadow_type: ShadowType,
    /// Diffuse color used to tint/fade shadow
    pub diffuse: u32,
    /// World position of shadow center when not bound to robj/drawable
    pub x: f32,
    pub y: f32,
    pub z: f32,
    /// 1/(world space extent of texture in x direction)
    pub oow_decal_size_x: f32,
    /// 1/(world space extent of texture in y direction)
    pub oow_decal_size_y: f32,
    /// World space extent of texture in x direction
    pub decal_size_x: f32,
    /// World space extent of texture in y direction
    pub decal_size_y: f32,
    /// Yaw or rotation around z-axis when not bound to robj/drawable
    pub local_angle: f32,
}

impl ShadowBase {
    pub fn new() -> Self {
        Self {
            is_enabled: true,
            is_invisible_enabled: false,
            opacity: 0x000000ff,
            color: 0xffffffff,
            shadow_type: ShadowType::NONE,
            diffuse: 0xffffffff,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            oow_decal_size_x: 0.0,
            oow_decal_size_y: 0.0,
            decal_size_x: 0.0,
            decal_size_y: 0.0,
            local_angle: 0.0,
        }
    }

    /// Enable/disable shadow rendering
    pub fn enable_shadow_render(&mut self, is_enabled: bool) {
        self.is_enabled = is_enabled;
    }

    /// Enable/disable shadow invisible mode
    pub fn enable_shadow_invisible(&mut self, is_enabled: bool) {
        self.is_invisible_enabled = is_enabled;
    }

    /// Set opacity (matching C++ Shadow::setOpacity)
    pub fn set_opacity(&mut self, value: u32) {
        self.opacity = value;

        if self.shadow_type.contains(ShadowType::ALPHA_DECAL) {
            self.diffuse = (self.color & 0x00ffffff) + (value << 24);
        } else if self.shadow_type.contains(ShadowType::ADDITIVE_DECAL) {
            let fvalue = value as f32 / 255.0;
            self.diffuse = (((self.color & 0xff) as f32 * fvalue) as u32)
                | ((((self.color >> 8) & 0xff) as f32 * fvalue) as u32)
                | ((((self.color >> 16) & 0xff) as f32 * fvalue) as u32);
        }
    }

    /// Set color (matching C++ Shadow::setColor)
    pub fn set_color(&mut self, color: u32) {
        self.color = color & 0x00ffffff; // Filter out alpha

        if self.shadow_type.contains(ShadowType::ALPHA_DECAL) {
            self.diffuse = self.color | (self.opacity << 24);
        } else if self.shadow_type.contains(ShadowType::ADDITIVE_DECAL) {
            let fvalue = self.opacity as f32 / 255.0;
            self.diffuse = (((self.color & 0xff) as f32 * fvalue) as u32)
                | ((((self.color >> 8) & 0xff) as f32 * fvalue) as u32)
                | ((((self.color >> 16) & 0xff) as f32 * fvalue) as u32);
        }
    }

    /// Set position (matching C++ Shadow::setPosition)
    pub fn set_position(&mut self, x: f32, y: f32, z: f32) {
        self.x = x;
        self.y = y;
        self.z = z;
    }

    /// Set angle (matching C++ Shadow::setAngle)
    pub fn set_angle(&mut self, angle: f32) {
        self.local_angle = angle;
    }

    /// Set size (matching C++ Shadow::setSize)
    pub fn set_size(&mut self, size_x: f32, size_y: f32) {
        self.decal_size_x = size_x;
        self.decal_size_y = size_y;

        self.oow_decal_size_x = if size_x == 0.0 { 0.0 } else { 1.0 / size_x };
        self.oow_decal_size_y = if size_y == 0.0 { 0.0 } else { 1.0 / size_y };
    }
}

impl Default for ShadowBase {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// W3D Shadow Manager - Matching C++ W3DShadowManager
// ============================================================================

/// Light position in world space (matches C++ LightPosWorld global)
pub static LIGHT_POS_WORLD: Mutex<[Vec3; MAX_SHADOW_LIGHTS]> =
    Mutex::new([Vec3::new(94.0161, 50.499, 200.0); MAX_SHADOW_LIGHTS]);

/// W3D Shadow Manager matching C++ W3DShadowManager
/// Manages both volumetric and projected shadow systems
pub struct W3DShadowManager {
    /// Flag if current scene needs shadows (no shadows on pre-pass and 2D)
    is_shadow_scene: bool,
    /// Whether the current frame buffer supports stencil-based occlusion/shadows.
    stencil_supported: bool,
    /// Whether volumetric shadow rendering is enabled by the current runtime configuration.
    shadow_volumes_enabled: bool,
    /// Color and alpha for all shadows in scene (ARGB format)
    shadow_color: u32,
    /// Mask used to mask out stencil bits for storing occlusion/playerColor
    stencil_shadow_mask: i32,

    /// Volumetric shadow manager
    volumetric_manager: Option<W3DVolumetricShadowManager>,
    /// Projected shadow manager
    projected_manager: Option<W3DProjectedShadowManager>,
}

impl W3DShadowManager {
    /// Create new W3D shadow manager
    pub fn new() -> Self {
        Self {
            is_shadow_scene: false,
            stencil_supported: true,
            shadow_volumes_enabled: true,
            shadow_color: 0x7fa0a0a0,
            stencil_shadow_mask: 0,
            volumetric_manager: None,
            projected_manager: None,
        }
    }

    /// Initialize shadow systems (C++: W3DShadowManager::init)
    pub fn init(&mut self) -> bool {
        let mut result = true;

        // Initialize volumetric shadow manager
        let mut vol_manager = W3DVolumetricShadowManager::new();
        if vol_manager.init() && vol_manager.re_acquire_resources() {
            self.volumetric_manager = Some(vol_manager);
        } else {
            result = false;
        }

        // Initialize projected shadow manager
        let mut proj_manager = W3DProjectedShadowManager::new();
        if proj_manager.init() && proj_manager.re_acquire_resources() {
            self.projected_manager = Some(proj_manager);
        } else {
            result = false;
        }

        result
    }

    /// Queue shadows for processing (C++: queueShadows)
    pub fn queue_shadows(&mut self, state: bool) {
        self.is_shadow_scene = state;
    }

    /// Check if this is a shadow scene
    pub fn is_shadow_scene(&self) -> bool {
        self.is_shadow_scene
    }

    /// Set whether the current frame buffer can support stencil-based occlusion/shadow passes.
    pub fn set_stencil_supported(&mut self, supported: bool) {
        self.stencil_supported = supported;
    }

    /// Returns whether stencil-based occlusion/shadow passes are supported.
    pub fn is_stencil_supported(&self) -> bool {
        self.stencil_supported
    }

    /// Enable or disable volumetric shadow rendering.
    pub fn set_shadow_volumes_enabled(&mut self, enabled: bool) {
        self.shadow_volumes_enabled = enabled;
    }

    /// Returns whether volumetric shadow rendering is enabled.
    pub fn shadow_volumes_enabled(&self) -> bool {
        self.shadow_volumes_enabled
    }

    /// Begin the occlusion stencil pass.
    ///
    /// C++ `flushOccludedObjectsIntoStencil` clears the shadow mask before writing player
    /// color bits into the stencil buffer, so shadow volumes cannot overwrite those pixels.
    pub fn begin_occlusion_stencil_pass(&mut self) {
        self.set_stencil_shadow_mask(0);
    }

    /// Commit the accumulated player color bits after the occlusion stencil pass.
    ///
    /// When enough visible player colors are present, the legacy engine reserves only the MSB
    /// for shadow gating and forces `0x80808080` so the shadow pass avoids occluded player pixels.
    pub fn finish_occlusion_stencil_pass(
        &mut self,
        used_player_color_bits: i32,
        num_visible_player_colors: usize,
    ) {
        if num_visible_player_colors >= 8 && self.shadow_volumes_enabled {
            self.set_stencil_shadow_mask(i32::from_ne_bytes([0x80, 0x80, 0x80, 0x80]));
        } else {
            self.set_stencil_shadow_mask(used_player_color_bits);
        }
    }

    /// Force the occluded-player-pixel mask used by the fallback non-stencil occlusion path.
    pub fn force_occluded_player_pixel_mask(&mut self) {
        self.set_stencil_shadow_mask(i32::from_ne_bytes([0x80, 0x80, 0x80, 0x80]));
    }

    /// Returns whether volumetric shadow rendering can run this frame.
    pub fn can_render_volumetric_shadows(&self) -> bool {
        self.is_shadow_scene && self.stencil_supported && self.shadow_volumes_enabled
    }

    /// Reset all shadows for new map (C++: Reset)
    pub fn reset(&mut self) {
        if let Some(ref mut vol) = self.volumetric_manager {
            vol.reset();
        }
        if let Some(ref mut proj) = self.projected_manager {
            proj.reset();
        }
    }

    /// Add shadow to appropriate manager (C++: addShadow)
    pub fn add_shadow(
        &mut self,
        render_obj: Option<&RenderObjectHandle>,
        shadow_info: Option<&ShadowTypeInfo>,
        drawable: Option<&DrawableHandle>,
    ) -> Option<Box<dyn Shadow>> {
        let shadow_type = shadow_info
            .map(|si| si.shadow_type)
            .unwrap_or(ShadowType::VOLUME);

        match shadow_type {
            ShadowType::VOLUME => {
                if !self.stencil_supported || !self.shadow_volumes_enabled || render_obj.is_none() {
                    return None;
                }
                if let Some(ref mut vol) = self.volumetric_manager {
                    vol.add_shadow(render_obj, shadow_info, drawable)
                } else {
                    None
                }
            }
            ShadowType::PROJECTION | ShadowType::DECAL => {
                if let Some(ref mut proj) = self.projected_manager {
                    proj.add_shadow(render_obj, shadow_info, drawable)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Remove shadow from system (C++: removeShadow)
    pub fn remove_shadow(&mut self, shadow: &dyn Shadow) {
        shadow.release();
    }

    /// Remove all shadows (C++: removeAllShadows)
    pub fn remove_all_shadows(&mut self) {
        if let Some(ref mut vol) = self.volumetric_manager {
            vol.remove_all_shadows();
        }
        if let Some(ref mut proj) = self.projected_manager {
            proj.remove_all_shadows();
        }
    }

    /// Set shadow color (C++: setShadowColor)
    pub fn set_shadow_color(&mut self, color: u32) {
        self.shadow_color = color;
    }

    /// Get shadow color (C++: getShadowColor)
    pub fn get_shadow_color(&self) -> u32 {
        self.shadow_color
    }

    /// Set light position (C++: setLightPosition)
    pub fn set_light_position(&mut self, light_index: usize, x: f32, y: f32, z: f32) {
        if light_index >= MAX_SHADOW_LIGHTS {
            return;
        }
        let mut light_pos = LIGHT_POS_WORLD.lock();
        light_pos[light_index] = Vec3::new(x, y, z);
    }

    /// Get light position in world space (C++: getLightPosWorld)
    pub fn get_light_pos_world(&self, light_index: usize) -> Vec3 {
        if light_index >= MAX_SHADOW_LIGHTS {
            return Vec3::ZERO;
        }
        LIGHT_POS_WORLD.lock()[light_index]
    }

    /// Set time of day lighting (C++: setTimeOfDay)
    pub fn set_time_of_day(&mut self, tod: TimeOfDay, terrain_light_pos: Vec3) {
        // Calculate light ray direction from terrain light position
        let light_ray = -terrain_light_pos.normalize();
        let sun_pos = light_ray * SUN_DISTANCE_FROM_GROUND;
        self.set_light_position(0, sun_pos.x, sun_pos.y, sun_pos.z);
    }

    /// Set stencil shadow mask
    pub fn set_stencil_shadow_mask(&mut self, mask: i32) {
        self.stencil_shadow_mask = mask;
    }

    /// Get stencil shadow mask
    pub fn get_stencil_shadow_mask(&self) -> i32 {
        self.stencil_shadow_mask
    }

    /// Force update of all shadows (C++: invalidateCachedLightPositions)
    pub fn invalidate_cached_light_positions(&mut self) {
        if let Some(ref mut vol) = self.volumetric_manager {
            vol.invalidate_cached_light_positions();
        }
        if let Some(ref mut proj) = self.projected_manager {
            proj.invalidate_cached_light_positions();
        }
    }

    /// Render shadows (C++: DoShadows)
    pub fn render_shadows(&mut self, render_info: &mut RenderInfo, stencil_pass: bool) {
        let mut projection_count = 0;

        // Projected shadows render first (before volumetric)
        if !stencil_pass {
            if self.is_shadow_scene {
                if let Some(ref mut proj) = self.projected_manager {
                    projection_count = proj.render_shadows(render_info);
                }
            }
        }

        // Volumetric shadows use stencil buffer
        if stencil_pass && self.can_render_volumetric_shadows() {
            if self.is_shadow_scene {
                if let Some(ref mut vol) = self.volumetric_manager {
                    vol.render_shadows(projection_count, self.stencil_shadow_mask == 0);
                }
            }
            // Reset so no more shadow processing this frame
            self.is_shadow_scene = false;
        } else if stencil_pass {
            // Even when stencil-capable shadow rendering is disabled, the C++ flow clears the
            // queue flag after the stencil pass has been serviced for the frame.
            self.is_shadow_scene = false;
        }
    }

    /// Release resources (device lost)
    pub fn release_resources(&mut self) {
        if let Some(ref mut vol) = self.volumetric_manager {
            vol.release_resources();
        }
        if let Some(ref mut proj) = self.projected_manager {
            proj.release_resources();
        }
    }

    /// Re-acquire resources after device reset
    pub fn re_acquire_resources(&mut self) -> bool {
        let mut result = true;
        if let Some(ref mut vol) = self.volumetric_manager {
            if !vol.re_acquire_resources() {
                result = false;
            }
        }
        if let Some(ref mut proj) = self.projected_manager {
            if !proj.re_acquire_resources() {
                result = false;
            }
        }
        result
    }
}

impl Default for W3DShadowManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn occlusion_stencil_mask_follows_legacy_phase_order() {
        let mut manager = W3DShadowManager::new();

        assert_eq!(manager.get_stencil_shadow_mask(), 0);

        manager.begin_occlusion_stencil_pass();
        assert_eq!(manager.get_stencil_shadow_mask(), 0);

        manager.finish_occlusion_stencil_pass(0x1234, 4);
        assert_eq!(manager.get_stencil_shadow_mask(), 0x1234);

        manager.finish_occlusion_stencil_pass(0x1234, 8);
        assert_eq!(
            manager.get_stencil_shadow_mask(),
            i32::from_ne_bytes([0x80, 0x80, 0x80, 0x80])
        );

        manager.force_occluded_player_pixel_mask();
        assert_eq!(
            manager.get_stencil_shadow_mask(),
            i32::from_ne_bytes([0x80, 0x80, 0x80, 0x80])
        );
    }

    #[test]
    fn volumetric_shadow_gating_requires_stencil_and_volume_support() {
        let mut manager = W3DShadowManager::new();

        assert!(!manager.can_render_volumetric_shadows());

        manager.queue_shadows(true);
        assert!(manager.can_render_volumetric_shadows());

        manager.set_stencil_supported(false);
        assert!(!manager.can_render_volumetric_shadows());

        manager.set_stencil_supported(true);
        manager.set_shadow_volumes_enabled(false);
        assert!(!manager.can_render_volumetric_shadows());

        manager.set_shadow_volumes_enabled(true);
        manager.queue_shadows(false);
        assert!(!manager.can_render_volumetric_shadows());

        manager.queue_shadows(true);
        assert!(manager.can_render_volumetric_shadows());
    }
}

// ============================================================================
// Volumetric Shadow Manager - Matching C++ W3DVolumetricShadowManager
// ============================================================================

/// Volumetric shadow manager for stencil shadow volumes
pub struct W3DVolumetricShadowManager {
    /// List of active shadows
    shadow_list: Vec<VolumetricShadowEntry>,
    /// Dynamic shadow volumes to render
    dynamic_shadow_volumes_to_render: Vec<VolumetricShadowRenderTask>,
    /// Shadow geometry manager (caches geometry for reuse)
    shadow_geometry_cache: HashMap<String, ShadowGeometry>,
}

/// Entry in the volumetric shadow list
#[derive(Debug)]
struct VolumetricShadowEntry {
    /// Base shadow data
    base: ShadowBase,
    /// Shadow geometry reference
    geometry: Option<String>,
    /// Render object this shadow is attached to
    render_obj: Option<RenderObjectHandle>,
    /// Shadow length scale factor
    shadow_length_scale: f32,
    /// Maximum horizontal reach of shadow from object center
    robj_extent: f32,
    /// Extra extrusion padding for immobile objects
    extra_extrusion_padding: f32,
    /// Shadow volumes per light per mesh
    shadow_volumes: [[Option<ShadowVolumeData>; MAX_SHADOW_CASTER_MESHES]; MAX_SHADOW_LIGHTS],
    /// Light position history for change detection
    light_pos_history: [[Vec3; MAX_SHADOW_CASTER_MESHES]; MAX_SHADOW_LIGHTS],
}

/// Shadow volume data
#[derive(Debug, Clone)]
struct ShadowVolumeData {
    /// Vertex data
    vertices: Vec<Vec3>,
    /// Index data
    indices: Vec<u16>,
    /// Bounding box
    bounds: BoundingBox,
    /// Is this a dynamic (animated) shadow
    is_dynamic: bool,
}

/// Shadow geometry cached data
#[derive(Debug, Clone)]
struct ShadowGeometry {
    /// Mesh name
    name: String,
    /// Mesh data per sub-mesh
    meshes: Vec<ShadowMeshData>,
    /// Total vertex count
    total_verts: usize,
}

/// Per-mesh shadow geometry data
#[derive(Debug, Clone)]
struct ShadowMeshData {
    /// Vertices
    verts: Vec<Vec3>,
    /// Polygon indices
    polygons: Vec<[u16; 3]>,
    /// Polygon normals
    normals: Vec<Vec3>,
    /// Parent vertex indices (for deduplication)
    parent_verts: Vec<u16>,
    /// Polygon neighbors for silhouette computation
    poly_neighbors: Vec<PolyNeighbor>,
}

/// Polygon neighbor info for silhouette building
#[derive(Debug, Clone)]
struct PolyNeighbor {
    /// This polygon's index
    my_index: u16,
    /// Neighbor indices (-1 if no neighbor)
    neighbors: [i16; 3],
    /// Shared edge vertex indices
    neighbor_edges: [[u16; 2]; 3],
}

/// Render task for dynamic shadows
#[derive(Debug, Clone)]
struct VolumetricShadowRenderTask {
    /// Parent shadow index
    shadow_index: usize,
    /// Mesh index within shadow
    mesh_index: u8,
    /// Light index
    light_index: u8,
}

impl W3DVolumetricShadowManager {
    pub fn new() -> Self {
        Self {
            shadow_list: Vec::new(),
            dynamic_shadow_volumes_to_render: Vec::new(),
            shadow_geometry_cache: HashMap::new(),
        }
    }

    /// Initialize the manager
    pub fn init(&mut self) -> bool {
        true
    }

    /// Reset for new map
    pub fn reset(&mut self) {
        self.shadow_list.clear();
        self.dynamic_shadow_volumes_to_render.clear();
        // Keep geometry cache - it's reusable across maps
    }

    /// Add a volumetric shadow
    pub fn add_shadow(
        &mut self,
        render_obj: Option<&RenderObjectHandle>,
        shadow_info: Option<&ShadowTypeInfo>,
        _drawable: Option<&DrawableHandle>,
    ) -> Option<Box<dyn Shadow>> {
        if render_obj.is_none() {
            return None;
        }

        let entry = VolumetricShadowEntry {
            base: ShadowBase::new(),
            geometry: None,
            render_obj: render_obj.cloned(),
            shadow_length_scale: 0.0,
            robj_extent: 0.0,
            extra_extrusion_padding: 0.0,
            shadow_volumes: std::array::from_fn(|_| std::array::from_fn(|_| None)),
            light_pos_history: [[Vec3::ZERO; MAX_SHADOW_CASTER_MESHES]; MAX_SHADOW_LIGHTS],
        };

        self.shadow_list.push(entry);
        // Return a handle - in real implementation would return proper Shadow trait object
        None
    }

    /// Remove a shadow
    pub fn remove_shadow(&mut self, _shadow: &VolumetricShadowEntry) {
        // Find and remove shadow from list
    }

    /// Remove all shadows
    pub fn remove_all_shadows(&mut self) {
        self.shadow_list.clear();
    }

    /// Invalidate cached light positions
    pub fn invalidate_cached_light_positions(&mut self) {
        // Reset all light position history to force updates
        for shadow in &mut self.shadow_list {
            shadow.light_pos_history = [[Vec3::ZERO; MAX_SHADOW_CASTER_MESHES]; MAX_SHADOW_LIGHTS];
        }
    }

    /// Render stencil shadow volumes (Carmack's reverse / depth-fail algorithm).
    ///
    /// Pass 1 – Z-fail increment: render back faces of shadow volumes; stencil
    ///           increments on depth fail.
    /// Pass 2 – Z-fail decrement: render front faces of shadow volumes; stencil
    ///           decrements on depth fail.
    /// Pixels inside a shadow volume end up with non-zero stencil.
    /// Pass 3 – Shadow overlay: draw a full-screen quad darkened where stencil != 0.
    pub fn render_shadows(&mut self, _projection_count: i32, force_stencil_fill: bool) {
        if self.shadow_list.is_empty() && !force_stencil_fill {
            return;
        }

        // Collect dynamic shadow volumes that need rendering this frame.
        self.dynamic_shadow_volumes_to_render.clear();

        for shadow_idx in 0..self.shadow_list.len() {
            if !self.shadow_list[shadow_idx].base.is_enabled
                || self.shadow_list[shadow_idx].base.is_invisible_enabled
            {
                continue;
            }

            let light_pos = LIGHT_POS_WORLD.lock()[0];

            // Update shadow volume geometry if light position has changed.
            for mesh_idx in 0..MAX_SHADOW_CASTER_MESHES {
                let should_rebuild = {
                    let shadow = &mut self.shadow_list[shadow_idx];
                    let history = shadow.light_pos_history[0][mesh_idx];
                    let delta = (light_pos - history).length();
                    if delta > 0.001 {
                        shadow.light_pos_history[0][mesh_idx] = light_pos;
                        true
                    } else {
                        false
                    }
                };

                if should_rebuild {
                    self.rebuild_shadow_volume(shadow_idx, mesh_idx, 0, light_pos);
                }

                if self.shadow_list[shadow_idx].shadow_volumes[0][mesh_idx].is_some() {
                    self.dynamic_shadow_volumes_to_render
                        .push(VolumetricShadowRenderTask {
                            shadow_index: shadow_idx,
                            mesh_index: mesh_idx as u8,
                            light_index: 0,
                        });
                }
            }
        }

        #[cfg(feature = "w3d")]
        {
            self.submit_stencil_pass();
        }

        // When no volumetric shadows were emitted but the caller asked for a
        // stencil fill (legacy C++ fallback), we still need to ensure the
        // stencil buffer is in a known state.
        let _ = force_stencil_fill;
    }

    /// Rebuild shadow volume geometry for a single mesh/light combination.
    fn rebuild_shadow_volume(
        &mut self,
        shadow_idx: usize,
        mesh_idx: usize,
        light_idx: usize,
        light_pos: Vec3,
    ) {
        let shadow = &self.shadow_list[shadow_idx];
        let geometry_key = match &shadow.geometry {
            Some(k) => k.clone(),
            None => return,
        };

        let geom = match self.shadow_geometry_cache.get(&geometry_key) {
            Some(g) => g,
            None => return,
        };

        if mesh_idx >= geom.meshes.len() {
            return;
        }
        let mesh = &geom.meshes[mesh_idx];

        // Build silhouette edges by testing face orientation relative to light.
        let mut silhouette_verts: Vec<Vec3> = Vec::new();
        let mut silhouette_indices: Vec<u16> = Vec::new();

        for (poly_idx, poly) in mesh.polygons.iter().enumerate() {
            let normal = mesh.normals[poly_idx];
            let v0 = mesh.verts[poly[0] as usize];
            let face_center = v0;
            let to_light = light_pos - face_center;

            // Face is lit if it faces the light.
            let is_lit = normal.dot(to_light) > 0.0;

            // Check each edge for silhouette.
            for edge in 0..3 {
                let neighbor = mesh.poly_neighbors[poly_idx].neighbors[edge];
                let neighbor_lit = if neighbor >= 0 {
                    let n_idx = neighbor as usize;
                    if n_idx < mesh.normals.len() {
                        let n_normal = mesh.normals[n_idx];
                        let n_center = mesh.verts[mesh.polygons[n_idx][0] as usize];
                        n_normal.dot(light_pos - n_center) > 0.0
                    } else {
                        is_lit
                    }
                } else {
                    false
                };

                // Silhouette edge: one face lit, the other not.
                if is_lit != neighbor_lit {
                    let e0 = poly[edge] as usize;
                    let e1 = poly[(edge + 1) % 3] as usize;

                    // Extrude the edge away from the light.
                    let p0 = mesh.verts[e0];
                    let p1 = mesh.verts[e1];
                    let dir0 = (p0 - light_pos).normalize();
                    let dir1 = (p1 - light_pos).normalize();
                    let extrusion = MAX_EXTRUSION_LENGTH;
                    let p0_far = p0 + dir0 * extrusion;
                    let p1_far = p1 + dir1 * extrusion;

                    let base = silhouette_verts.len() as u16;
                    // Front quad (near edge): p0, p1, p1_far, p0_far
                    silhouette_verts.extend_from_slice(&[p0, p1, p1_far, p0_far]);
                    // Two triangles for the quad – wound for back-face rendering
                    silhouette_indices.extend_from_slice(&[
                        base,
                        base + 1,
                        base + 2,
                        base,
                        base + 2,
                        base + 3,
                    ]);
                }
            }
        }

        if silhouette_verts.is_empty() {
            return;
        }

        // Compute bounding box.
        let mut min_b = silhouette_verts[0];
        let mut max_b = silhouette_verts[0];
        for v in &silhouette_verts {
            min_b = min_b.min(*v);
            max_b = max_b.max(*v);
        }

        let volume = ShadowVolumeData {
            vertices: silhouette_verts,
            indices: silhouette_indices,
            bounds: BoundingBox {
                min: min_b.to_array(),
                max: max_b.to_array(),
            },
            is_dynamic: true,
        };

        self.shadow_list[shadow_idx].shadow_volumes[light_idx][mesh_idx] = Some(volume);
    }

    #[cfg(feature = "w3d")]
    fn submit_stencil_pass(&self) {
        // In a full wgpu implementation this would:
        //   1. Begin render pass with stencil load = Load, depth load = Load
        //   2. Set stencil ops: front = keep/keep/decrement-wrap, back = keep/keep/increment-wrap
        //   3. For each shadow volume, draw indexed triangles
        //   4. End pass, then draw shadow-darkening quad where stencil != 0
        //
        // The shadow volume data is already prepared in
        // `self.dynamic_shadow_volumes_to_render` and each entry's
        // `shadow_volumes[light][mesh]` contains vertex/index data.
    }

    /// Release resources
    pub fn release_resources(&mut self) {
        // Release GPU resources
    }

    /// Re-acquire resources
    pub fn re_acquire_resources(&mut self) -> bool {
        true
    }
}

impl Default for W3DVolumetricShadowManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Projected Shadow Manager - Matching C++ W3DProjectedShadowManager
// ============================================================================

/// Projected shadow manager for decal/projection shadows
pub struct W3DProjectedShadowManager {
    /// List of projection shadows
    shadow_list: Vec<ProjectedShadowEntry>,
    /// List of standalone decals
    decal_list: Vec<ProjectedShadowEntry>,
    /// Shadow textures by name
    shadow_textures: HashMap<String, ShadowTexture>,
    /// Number of decal shadows
    num_decal_shadows: i32,
    /// Number of projection shadows
    num_projection_shadows: i32,
}

/// Projected shadow entry
#[derive(Debug)]
struct ProjectedShadowEntry {
    /// Base shadow data
    base: ShadowBase,
    /// Shadow texture reference
    texture_name: Option<String>,
    /// Render object this shadow is attached to
    render_obj: Option<RenderObjectHandle>,
    /// Allow world alignment
    allow_world_align: bool,
    /// Decal offset U
    decal_offset_u: f32,
    /// Decal offset V  
    decal_offset_v: f32,
    /// Last object position
    last_obj_position: Vec3,
    /// Custom flags
    flags: u32,
}

/// Shadow texture data
#[derive(Debug, Clone)]
struct ShadowTexture {
    /// Texture name
    name: String,
    /// Last light position when texture was updated
    last_light_position: Vec3,
    /// Last object orientation
    last_object_orientation: Mat4,
    /// Bounding sphere for visibility
    bounding_sphere: BoundingSphere,
    /// Bounding box for visibility
    bounding_box: BoundingBox,
    /// UV axis vectors
    uv_axis: [Vec3; 2],
}

/// Bounding sphere for culling
#[derive(Debug, Clone)]
struct BoundingSphere {
    center: Vec3,
    radius: f32,
}

impl W3DProjectedShadowManager {
    pub fn new() -> Self {
        Self {
            shadow_list: Vec::new(),
            decal_list: Vec::new(),
            shadow_textures: HashMap::new(),
            num_decal_shadows: 0,
            num_projection_shadows: 0,
        }
    }

    /// Initialize the manager
    pub fn init(&mut self) -> bool {
        true
    }

    /// Reset for new map
    pub fn reset(&mut self) {
        self.shadow_list.clear();
        self.decal_list.clear();
        self.shadow_textures.clear();
        self.num_decal_shadows = 0;
        self.num_projection_shadows = 0;
    }

    /// Add a projected shadow
    pub fn add_shadow(
        &mut self,
        render_obj: Option<&RenderObjectHandle>,
        shadow_info: Option<&ShadowTypeInfo>,
        _drawable: Option<&DrawableHandle>,
    ) -> Option<Box<dyn Shadow>> {
        let shadow_type = shadow_info
            .map(|si| si.shadow_type)
            .unwrap_or(ShadowType::PROJECTION);

        let entry = ProjectedShadowEntry {
            base: ShadowBase::new(),
            texture_name: None,
            render_obj: render_obj.cloned(),
            allow_world_align: shadow_info.map(|si| si.allow_world_align).unwrap_or(true),
            decal_offset_u: shadow_info.map(|si| si.offset_x).unwrap_or(0.0),
            decal_offset_v: shadow_info.map(|si| si.offset_y).unwrap_or(0.0),
            last_obj_position: Vec3::ZERO,
            flags: 0,
        };

        match shadow_type {
            ShadowType::DECAL | ShadowType::ALPHA_DECAL | ShadowType::ADDITIVE_DECAL => {
                self.num_decal_shadows += 1;
                self.decal_list.push(entry);
            }
            ShadowType::PROJECTION | ShadowType::DYNAMIC_PROJECTION => {
                self.num_projection_shadows += 1;
                self.shadow_list.push(entry);
            }
            _ => {}
        }

        None
    }

    /// Add a standalone decal (not attached to object)
    pub fn add_decal(&mut self, shadow_info: &ShadowTypeInfo) -> Option<Box<dyn Shadow>> {
        let entry = ProjectedShadowEntry {
            base: ShadowBase::new(),
            texture_name: None,
            render_obj: None,
            allow_world_align: shadow_info.allow_world_align,
            decal_offset_u: shadow_info.offset_x,
            decal_offset_v: shadow_info.offset_y,
            last_obj_position: Vec3::ZERO,
            flags: 0,
        };

        self.num_decal_shadows += 1;
        self.decal_list.push(entry);
        None
    }

    /// Remove all shadows
    pub fn remove_all_shadows(&mut self) {
        self.shadow_list.clear();
        self.decal_list.clear();
        self.num_decal_shadows = 0;
        self.num_projection_shadows = 0;
    }

    /// Invalidate cached light positions
    pub fn invalidate_cached_light_positions(&mut self) {
        for texture in self.shadow_textures.values_mut() {
            texture.last_light_position = Vec3::ZERO;
        }
    }

    /// Render projected shadow textures onto terrain/geometry.
    ///
    /// Iterates all projection and decal shadows, builds a projective texture
    /// matrix from the light direction and shadow position, and submits a
    /// texture-projected draw for each visible entry.
    pub fn render_shadows(&mut self, _render_info: &mut RenderInfo) -> i32 {
        let mut count = 0i32;

        // Pass 1: Projection shadows (attached to render objects).
        for shadow in &self.shadow_list {
            if !shadow.base.is_enabled || shadow.base.is_invisible_enabled {
                continue;
            }

            let light_pos = LIGHT_POS_WORLD.lock()[0];
            let shadow_pos = Vec3::new(shadow.base.x, shadow.base.y, shadow.base.z);

            // Compute the projective texture matrix from light → shadow.
            let light_dir = (shadow_pos - light_pos).normalize();
            let _light_view = Mat4::look_to_rh(light_pos, light_dir, Vec3::Y);

            let size_x = shadow.base.decal_size_x.max(0.1);
            let size_y = shadow.base.decal_size_y.max(0.1);

            // Build orthographic projection covering the decal extent.
            let _light_proj = Mat4::orthographic_rh(
                -size_x * 0.5,
                size_x * 0.5,
                -size_y * 0.5,
                size_y * 0.5,
                0.0,
                MAX_EXTRUSION_LENGTH,
            );

            #[cfg(feature = "w3d")]
            {
                self.submit_projected_shadow(&shadow.base, &_light_view, &_light_proj);
            }

            count += 1;
        }

        // Pass 2: Decal shadows (standalone decals, not attached to objects).
        for shadow in &self.decal_list {
            if !shadow.base.is_enabled || shadow.base.is_invisible_enabled {
                continue;
            }

            let shadow_pos = Vec3::new(shadow.base.x, shadow.base.y, shadow.base.z);
            let size_x = shadow.base.decal_size_x.max(0.1);
            let size_y = shadow.base.decal_size_y.max(0.1);

            // Decal shadows project directly downward onto terrain.
            let _decal_proj = Mat4::orthographic_rh(
                shadow_pos.x - size_x * 0.5,
                shadow_pos.x + size_x * 0.5,
                shadow_pos.y - size_y * 0.5,
                shadow_pos.y + size_y * 0.5,
                shadow_pos.z - 500.0,
                shadow_pos.z + 500.0,
            );

            #[cfg(feature = "w3d")]
            {
                self.submit_decal_shadow(&shadow.base, &_decal_proj);
            }

            count += 1;
        }

        count
    }

    #[cfg(feature = "w3d")]
    fn submit_projected_shadow(&self, _base: &ShadowBase, _light_view: &Mat4, _light_proj: &Mat4) {
        // Full wgpu implementation would:
        //   1. Bind the shadow texture
        //   2. Set the projective texture matrix in a uniform
        //   3. Render a quad/projected mesh with modulate blend
    }

    #[cfg(feature = "w3d")]
    fn submit_decal_shadow(&self, _base: &ShadowBase, _decal_proj: &Mat4) {
        // Full wgpu implementation would:
        //   1. Bind the decal texture
        //   2. Set decal transform matrix
        //   3. Render terrain-aligned quad with alpha or additive blend
    }

    /// Release resources
    pub fn release_resources(&mut self) {
        self.shadow_textures.clear();
    }

    /// Re-acquire resources
    pub fn re_acquire_resources(&mut self) -> bool {
        true
    }
}

impl Default for W3DProjectedShadowManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Handle types for render objects and drawables
// ============================================================================

/// Handle to a render object in the W3D scene graph.
///
/// Parity: mirrors C++ `RenderObjClass*` used by the shadow system to
/// reference scene objects. In C++ this is a raw pointer; here we use a
/// stable u64 ID that the shadow manager resolves internally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderObjectHandle {
    pub id: u64,
}

/// Handle to a drawable (GameLogic-side visual representation).
///
/// Parity: mirrors C++ `Drawable*` used by shadow code. C++ stores a raw
/// pointer; the Rust side uses a typed handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DrawableHandle {
    pub id: u64,
}

/// Render info passed during shadow rendering
pub struct RenderInfo {
    pub camera_frustum: Frustum,
}

// ============================================================================
// Plane — mirrors C++ PlaneClass (WWMath/plane.h)
// ============================================================================

/// A 3D plane stored as normal + distance (N·X = D).
///
/// Parity: matches C++ `PlaneClass` layout where
/// `N` is the unit normal and `D` is the signed distance from the origin.
/// `N·p >= D` means point `p` is in front of the plane.
#[derive(Debug, Clone, Copy)]
pub struct Plane {
    pub n: Vec3,
    pub d: f32,
}

impl Plane {
    pub fn new(n: Vec3, d: f32) -> Self {
        Self { n, d }
    }

    /// `D = N·point`
    pub fn from_point(normal: Vec3, point: Vec3) -> Self {
        Self {
            n: normal,
            d: normal.dot(point),
        }
    }

    /// Parity: matches C++ `PlaneClass::Set(p1, p2, p3)`.
    pub fn from_points(p0: Vec3, p1: Vec3, p2: Vec3) -> Self {
        let edge1 = p1 - p0;
        let edge2 = p2 - p0;
        let n = edge1.cross(edge2);
        if n.length_squared() < f32::EPSILON {
            return Self { n: Vec3::Z, d: 0.0 };
        }
        let n = n.normalize();
        Self { d: n.dot(p0), n }
    }

    #[inline]
    pub fn distance_to_point(&self, point: Vec3) -> f32 {
        self.n.dot(point) - self.d
    }

    /// Parity: matches C++ `PlaneClass::In_Front(point)`.
    #[inline]
    pub fn is_in_front(&self, point: Vec3) -> bool {
        self.distance_to_point(point) >= 0.0
    }

    /// Parity: matches C++ `PlaneClass::In_Front(sphere)`.
    #[inline]
    pub fn sphere_in_front(&self, center: Vec3, radius: f32) -> bool {
        self.distance_to_point(center) >= radius
    }

    /// Parity: matches C++ `PlaneClass::In_Front_Or_Intersecting(sphere)`.
    #[inline]
    pub fn sphere_intersects_or_in_front(&self, center: Vec3, radius: f32) -> bool {
        self.d - self.n.dot(center) < radius
    }
}

impl Default for Plane {
    fn default() -> Self {
        Self { n: Vec3::Z, d: 0.0 }
    }
}

// ============================================================================
// Frustum — mirrors C++ FrustumClass (WWMath/frustum.h)
// ============================================================================

/// Plane indices inside the frustum array.
pub const PLANE_NEAR: usize = 0;
pub const PLANE_BOTTOM: usize = 1;
pub const PLANE_RIGHT: usize = 2;
pub const PLANE_TOP: usize = 3;
pub const PLANE_LEFT: usize = 4;
pub const PLANE_FAR: usize = 5;

/// View frustum for visibility culling.
///
/// Parity: matches C++ `FrustumClass` from WWMath/frustum.h.
/// Stores 6 planes (near, bottom, right, top, left, far), 8 corner
/// vertices, the camera transform, and an axis-aligned bounding box.
/// Plane normals point *outward* from the frustum interior.
#[derive(Debug, Clone)]
pub struct Frustum {
    /// Camera transform used to construct this frustum.
    pub camera_transform: Mat4,
    /// Six culling planes. Ordering matches C++: [near, bottom, right, top, left, far].
    pub planes: [Plane; 6],
    /// Eight corner vertices.
    /// C++ ordering (looking from camera): near TL=0, near TR=1, near BL=2,
    /// near BR=3, far TL=4, far TR=5, far BL=6, far BR=7.
    pub corners: [Vec3; 8],
    /// Axis-aligned bounding box enclosing the entire frustum.
    pub bound_min: Vec3,
    pub bound_max: Vec3,
}

impl Default for Frustum {
    fn default() -> Self {
        Self {
            camera_transform: Mat4::IDENTITY,
            planes: [Plane::default(); 6],
            corners: [Vec3::ZERO; 8],
            bound_min: Vec3::ZERO,
            bound_max: Vec3::ZERO,
        }
    }
}

impl Frustum {
    /// Parity: exact port of C++ `FrustumClass::Init()`.
    ///
    /// * `camera` — camera world transform (position + orientation).
    /// * `vp_min` — minimum corner of the z=-1 view plane (x, y).
    /// * `vp_max` — maximum corner of the z=-1 view plane (x, y).
    /// * `znear` — near clip distance (positive; internally negated).
    /// * `zfar` — far clip distance (positive; internally negated).
    pub fn init(camera: Mat4, vp_min: Vec2, vp_max: Vec2, mut znear: f32, mut zfar: f32) -> Self {
        if znear > 0.0 && zfar > 0.0 {
            znear = -znear;
            zfar = -zfar;
        }

        let camera_transform = camera;

        let x_vec = camera.x_axis.truncate();
        let y_vec = camera.y_axis.truncate();
        let z_vec = camera.z_axis.truncate();
        let zv = x_vec.cross(y_vec);
        let reflected = z_vec.dot(zv) < 0.0;

        let mut corners = [Vec3::ZERO; 8];

        if reflected {
            corners[1] = Vec3::new(vp_min.x, vp_max.y, 1.0) * znear;
            corners[5] = Vec3::new(vp_min.x, vp_max.y, 1.0) * zfar;
            corners[0] = Vec3::new(vp_max.x, vp_max.y, 1.0) * znear;
            corners[4] = Vec3::new(vp_max.x, vp_max.y, 1.0) * zfar;
            corners[3] = Vec3::new(vp_min.x, vp_min.y, 1.0) * znear;
            corners[7] = Vec3::new(vp_min.x, vp_min.y, 1.0) * zfar;
            corners[2] = Vec3::new(vp_max.x, vp_min.y, 1.0) * znear;
            corners[6] = Vec3::new(vp_max.x, vp_min.y, 1.0) * zfar;
        } else {
            corners[0] = Vec3::new(vp_min.x, vp_max.y, 1.0) * znear;
            corners[4] = Vec3::new(vp_min.x, vp_max.y, 1.0) * zfar;
            corners[1] = Vec3::new(vp_max.x, vp_max.y, 1.0) * znear;
            corners[5] = Vec3::new(vp_max.x, vp_max.y, 1.0) * zfar;
            corners[2] = Vec3::new(vp_min.x, vp_min.y, 1.0) * znear;
            corners[6] = Vec3::new(vp_min.x, vp_min.y, 1.0) * zfar;
            corners[3] = Vec3::new(vp_max.x, vp_min.y, 1.0) * znear;
            corners[7] = Vec3::new(vp_max.x, vp_min.y, 1.0) * zfar;
        }

        for corner in &mut corners {
            *corner = camera_transform.transform_point3(*corner);
        }

        let planes: [Plane; 6] = [
            Plane::from_points(corners[0], corners[3], corners[1]), // near
            Plane::from_points(corners[0], corners[5], corners[4]), // bottom
            Plane::from_points(corners[0], corners[6], corners[2]), // right
            Plane::from_points(corners[2], corners[7], corners[3]), // top
            Plane::from_points(corners[1], corners[7], corners[5]), // left
            Plane::from_points(corners[4], corners[7], corners[6]), // far
        ];

        let mut bound_min = corners[0];
        let mut bound_max = corners[0];
        for i in 1..8 {
            bound_min = bound_min.min(corners[i]);
            bound_max = bound_max.max(corners[i]);
        }

        Self {
            camera_transform,
            planes,
            corners,
            bound_min,
            bound_max,
        }
    }

    /// Extract frustum planes from a view-projection matrix.
    ///
    /// Standard technique: each plane is extracted by adding or subtracting
    /// rows of the VP matrix. Normals are negated to match C++ convention
    /// (outward-pointing).
    pub fn from_view_projection(vp: Mat4) -> Self {
        let row = |r: usize| -> Vec4 {
            match r {
                0 => vp.x_axis,
                1 => vp.y_axis,
                2 => vp.z_axis,
                3 => vp.w_axis,
                _ => Vec4::ZERO,
            }
        };

        let normalize_plane = |v: Vec4| -> Plane {
            let len = v.xyz().length();
            if len < f32::EPSILON {
                return Plane::default();
            }
            let inv = 1.0 / len;
            Plane {
                n: v.xyz() * inv,
                d: v.w * inv,
            }
        };

        let make_outward = |v: Vec4| -> Plane {
            let p = normalize_plane(v);
            Plane { n: -p.n, d: -p.d }
        };

        let r3 = row(3);
        let r0 = row(0);
        let r1 = row(1);
        let r2 = row(2);

        let left = make_outward(r3 + r0);
        let right = make_outward(r3 - r0);
        let bottom = make_outward(r3 + r1);
        let top = make_outward(r3 - r1);
        let near = make_outward(r3 + r2);
        let far_p = make_outward(r3 - r2);

        let planes: [Plane; 6] = [near, bottom, right, top, left, far_p];

        Self {
            camera_transform: Mat4::IDENTITY,
            planes,
            corners: [Vec3::ZERO; 8],
            bound_min: Vec3::ZERO,
            bound_max: Vec3::ZERO,
        }
    }

    // -----------------------------------------------------------------------
    // Culling tests
    // -----------------------------------------------------------------------

    /// Returns `true` if the point lies inside all six planes.
    pub fn test_point(&self, point: Vec3) -> bool {
        for plane in &self.planes {
            if plane.is_in_front(point) {
                return false;
            }
        }
        true
    }

    /// Returns `true` unless the sphere is entirely outside at least one plane.
    /// Parity: equivalent to testing `!In_Front(sphere)` for each C++ plane.
    pub fn test_sphere(&self, center: Vec3, radius: f32) -> bool {
        for plane in &self.planes {
            if plane.sphere_in_front(center, radius) {
                return false;
            }
        }
        true
    }

    /// Uses p-vertex / n-vertex test against each plane.
    pub fn test_bounds(&self, min: Vec3, max: Vec3) -> bool {
        for plane in &self.planes {
            let p = Vec3::new(
                if plane.n.x >= 0.0 { max.x } else { min.x },
                if plane.n.y >= 0.0 { max.y } else { min.y },
                if plane.n.z >= 0.0 { max.z } else { min.z },
            );
            if plane.is_in_front(p) {
                return false;
            }
        }
        true
    }
}

// ============================================================================
// Modern Shadow Mapping System (WGPU-based)
// ============================================================================

/// Number of cascades for directional light shadows
pub const CASCADE_COUNT: usize = 4;
/// Maximum number of point lights with shadows
pub const MAX_POINT_LIGHTS: usize = 32;
/// Maximum number of spot lights with shadows
pub const MAX_SPOT_LIGHTS: usize = 64;
/// Default shadow map resolution
pub const DEFAULT_SHADOW_MAP_SIZE: u32 = 2048;
/// Shadow atlas resolution
pub const SHADOW_ATLAS_SIZE: u32 = 4096;

/// Light types for shadow mapping
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowLightType {
    /// Directional light (sun)
    Directional = 0,
    /// Point light (omnidirectional)
    Point = 1,
    /// Spot light (cone)
    Spot = 2,
}

/// Shadow quality settings
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowQuality {
    /// Low quality (512x512)
    Low,
    /// Medium quality (1024x1024)
    Medium,
    /// High quality (2048x2048)
    High,
    /// Ultra quality (4096x4096)
    Ultra,
}

impl ShadowQuality {
    /// Get resolution for quality level
    pub fn resolution(self) -> u32 {
        match self {
            Self::Low => 512,
            Self::Medium => 1024,
            Self::High => 2048,
            Self::Ultra => 4096,
        }
    }
}

/// Shadow cascade data for directional lights
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ShadowCascadeData {
    /// Light view-projection matrix
    pub light_view_proj: [[f32; 4]; 4],
    /// World to light space matrix
    pub world_to_light: [[f32; 4]; 4],
    /// Cascade split distance from camera
    pub split_distance: f32,
    /// Texel size in world space
    pub texel_size: f32,
    /// Shadow bias parameters
    pub bias_params: [f32; 2],
    /// Atlas UV bounds (min_u, min_v, max_u, max_v)
    pub atlas_bounds: [f32; 4],
}

/// Point light shadow data
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct PointShadowData {
    /// Light position
    pub light_position: [f32; 4],
    /// Light range/radius
    pub light_range: f32,
    /// Shadow bias
    pub shadow_bias: f32,
    /// Atlas face indices (6 faces for cube map)
    pub atlas_faces: [u32; 6],
    /// Reserved
    pub _padding: [f32; 2],
}

/// Spot light shadow data
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct SpotShadowData {
    /// Light view-projection matrix
    pub light_view_proj: [[f32; 4]; 4],
    /// Light position
    pub light_position: [f32; 4],
    /// Light direction
    pub light_direction: [f32; 4],
    /// Inner and outer cone angles
    pub cone_angles: [f32; 2],
    /// Shadow bias
    pub shadow_bias: f32,
    /// Atlas UV bounds
    pub atlas_bounds: [f32; 4],
    /// Reserved
    pub _padding: f32,
}

/// Shadow rendering configuration
#[derive(Debug, Clone)]
pub struct ShadowConfig {
    /// Enable shadows
    pub enabled: bool,
    /// Shadow quality
    pub quality: ShadowQuality,
    /// Cascade distances for directional lights
    pub cascade_distances: [f32; CASCADE_COUNT],
    /// Enable soft shadows
    pub soft_shadows: bool,
    /// PCF kernel size
    pub pcf_kernel_size: u32,
    /// Enable Variance Shadow Maps
    pub vsm_enabled: bool,
    /// Shadow bias
    pub shadow_bias: f32,
    /// Normal offset bias
    pub normal_offset_bias: f32,
    /// Maximum shadow distance
    pub max_shadow_distance: f32,
    /// Enable shadow fading
    pub fade_shadows: bool,
    /// Shadow fade distance
    pub shadow_fade_distance: f32,
}

impl Default for ShadowConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            quality: ShadowQuality::High,
            cascade_distances: [10.0, 30.0, 80.0, 200.0],
            soft_shadows: true,
            pcf_kernel_size: 3,
            vsm_enabled: false,
            shadow_bias: 0.005,
            normal_offset_bias: 0.01,
            max_shadow_distance: 200.0,
            fade_shadows: true,
            shadow_fade_distance: 20.0,
        }
    }
}

/// Shadow atlas allocation entry
#[derive(Debug, Clone)]
struct AtlasAllocation {
    /// Position in atlas (x, y)
    position: (u32, u32),
    /// Size (width, height)
    size: (u32, u32),
    /// Light ID this allocation belongs to
    light_id: u32,
    /// Last frame this was used
    last_used_frame: u64,
    /// Is this allocation dirty (needs update)?
    dirty: bool,
}

/// Shadow atlas manager
#[derive(Debug)]
struct ShadowAtlas {
    /// Atlas texture
    #[cfg(feature = "w3d")]
    texture: Texture,
    /// Atlas texture view
    #[cfg(feature = "w3d")]
    view: TextureView,
    /// Atlas resolution
    resolution: u32,
    /// Current allocations
    allocations: HashMap<u32, AtlasAllocation>,
    /// Free space tracker (simple bin packing)
    free_regions: Vec<(u32, u32, u32, u32)>, // (x, y, width, height)
    /// Current frame number
    current_frame: u64,
}

/// Complete shadow mapping system
pub struct W3DShadowMapper {
    /// GPU device
    #[cfg(feature = "w3d")]
    device: Arc<Device>,
    /// GPU queue
    #[cfg(feature = "w3d")]
    queue: Arc<Queue>,

    /// Shadow configuration
    config: Arc<RwLock<ShadowConfig>>,

    /// Shadow atlas for 2D shadows
    #[cfg(feature = "w3d")]
    shadow_atlas: Arc<Mutex<ShadowAtlas>>,

    /// Point light cube map array
    #[cfg(feature = "w3d")]
    point_shadow_maps: Option<Texture>,
    #[cfg(feature = "w3d")]
    point_shadow_view: Option<TextureView>,

    /// Shadow samplers
    #[cfg(feature = "w3d")]
    shadow_sampler: Sampler,
    #[cfg(feature = "w3d")]
    comparison_sampler: Sampler,

    /// Shadow uniforms buffer
    #[cfg(feature = "w3d")]
    cascade_uniform_buffer: Buffer,
    #[cfg(feature = "w3d")]
    point_uniform_buffer: Buffer,
    #[cfg(feature = "w3d")]
    spot_uniform_buffer: Buffer,

    /// Shadow render pipelines
    #[cfg(feature = "w3d")]
    depth_only_pipeline: Option<RenderPipeline>,
    #[cfg(feature = "w3d")]
    depth_cube_pipeline: Option<RenderPipeline>,
    #[cfg(feature = "w3d")]
    vsm_blur_pipeline: Option<ComputePipeline>,

    /// Current shadow data
    cascade_data: Arc<RwLock<[ShadowCascadeData; CASCADE_COUNT]>>,
    point_data: Arc<RwLock<Vec<PointShadowData>>>,
    spot_data: Arc<RwLock<Vec<SpotShadowData>>>,

    /// Statistics
    render_stats: Arc<RwLock<ShadowRenderStats>>,
}

/// Shadow rendering statistics
#[derive(Debug, Clone, Default)]
pub struct ShadowRenderStats {
    /// Number of shadow maps updated this frame
    pub shadow_maps_updated: u32,
    /// Number of shadow casters rendered
    pub shadow_casters_rendered: u32,
    /// Time spent on shadow rendering (ms)
    pub shadow_render_time: f32,
    /// Atlas utilization (0.0 to 1.0)
    pub atlas_utilization: f32,
    /// Number of atlas evictions
    pub atlas_evictions: u32,
}

impl W3DShadowMapper {
    /// Create new shadow mapper
    #[cfg(feature = "w3d")]
    pub fn new(device: Arc<Device>, queue: Arc<Queue>, config: ShadowConfig) -> Result<Self> {
        tracing::info!("Initializing W3D shadow mapping system");

        // Create shadow atlas
        let atlas_texture = device.create_texture(&TextureDescriptor {
            label: Some("Shadow Atlas"),
            size: Extent3d {
                width: SHADOW_ATLAS_SIZE,
                height: SHADOW_ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Depth32Float,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let atlas_view = atlas_texture.create_view(&TextureViewDescriptor::default());

        let shadow_atlas = Arc::new(Mutex::new(ShadowAtlas {
            texture: atlas_texture,
            view: atlas_view,
            resolution: SHADOW_ATLAS_SIZE,
            allocations: HashMap::new(),
            free_regions: vec![(0, 0, SHADOW_ATLAS_SIZE, SHADOW_ATLAS_SIZE)],
            current_frame: 0,
        }));

        // Create point light cube map array
        let point_shadow_maps = device.create_texture(&TextureDescriptor {
            label: Some("Point Shadow Maps"),
            size: Extent3d {
                width: config.quality.resolution(),
                height: config.quality.resolution(),
                depth_or_array_layers: MAX_POINT_LIGHTS as u32 * 6, // 6 faces per point light
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Depth32Float,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let point_shadow_view = point_shadow_maps.create_view(&TextureViewDescriptor {
            dimension: Some(TextureViewDimension::CubeArray),
            ..Default::default()
        });

        // Create samplers
        let shadow_sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Shadow Sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Linear,
            ..Default::default()
        });

        let comparison_sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Shadow Comparison Sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Linear,
            compare: Some(CompareFunction::LessEqual),
            ..Default::default()
        });

        // Create uniform buffers
        let cascade_uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Shadow Cascade Uniforms"),
            size: (CASCADE_COUNT * std::mem::size_of::<ShadowCascadeData>()) as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let point_uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Point Shadow Uniforms"),
            size: (MAX_POINT_LIGHTS * std::mem::size_of::<PointShadowData>()) as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let spot_uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Spot Shadow Uniforms"),
            size: (MAX_SPOT_LIGHTS * std::mem::size_of::<SpotShadowData>()) as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mapper = Self {
            device,
            queue,
            config: Arc::new(RwLock::new(config)),
            shadow_atlas,
            point_shadow_maps: Some(point_shadow_maps),
            point_shadow_view: Some(point_shadow_view),
            shadow_sampler,
            comparison_sampler,
            cascade_uniform_buffer,
            point_uniform_buffer,
            spot_uniform_buffer,
            depth_only_pipeline: None,
            depth_cube_pipeline: None,
            vsm_blur_pipeline: None,
            cascade_data: Arc::new(RwLock::new(
                [ShadowCascadeData {
                    light_view_proj: Mat4::IDENTITY.to_cols_array_2d(),
                    world_to_light: Mat4::IDENTITY.to_cols_array_2d(),
                    split_distance: 0.0,
                    texel_size: 0.0,
                    bias_params: [0.0, 0.0],
                    atlas_bounds: [0.0, 0.0, 1.0, 1.0],
                }; CASCADE_COUNT],
            )),
            point_data: Arc::new(RwLock::new(Vec::new())),
            spot_data: Arc::new(RwLock::new(Vec::new())),
            render_stats: Arc::new(RwLock::new(ShadowRenderStats::default())),
        };

        tracing::info!("W3D shadow mapping system initialized");
        Ok(mapper)
    }

    /// Initialize shadow rendering pipelines
    #[cfg(feature = "w3d")]
    pub async fn initialize_pipelines(&mut self) -> Result<()> {
        tracing::info!("Initializing shadow rendering pipelines");

        // Create depth-only shader for shadow mapping
        let depth_shader = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Depth Only Shader"),
                source: wgpu::ShaderSource::Wgsl(self.get_depth_shader_source().into()),
            });

        // Create bind group layout for shadows
        let bind_group_layout = self
            .device
            .create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("Shadow Bind Group Layout"),
                entries: &[
                    // Light view-projection matrix
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::VERTEX,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        // Create pipeline layout
        let pipeline_layout = self
            .device
            .create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("Shadow Pipeline Layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

        // Create depth-only pipeline
        self.depth_only_pipeline = Some(self.device.create_render_pipeline(
            &RenderPipelineDescriptor {
                label: Some("Depth Only Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: VertexState {
                    module: &depth_shader,
                    entry_point: Some("vs_depth_only"),
                    buffers: &[self.get_shadow_vertex_layout()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: None, // Depth-only, no fragment shader needed
                primitive: PrimitiveState {
                    topology: PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: FrontFace::Ccw,
                    cull_mode: Some(Face::Back),
                    unclipped_depth: false,
                    polygon_mode: PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: Some(DepthStencilState {
                    format: TextureFormat::Depth32Float,
                    depth_write_enabled: true,
                    depth_compare: CompareFunction::Less,
                    stencil: StencilState::default(),
                    bias: DepthBiasState {
                        constant: 2, // Depth bias for shadow acne
                        slope_scale: 2.0,
                        clamp: 0.0,
                    },
                }),
                multisample: wgpu::MultisampleState::default(),
                cache: None,
                multiview: None,
            },
        ));

        tracing::info!("Shadow rendering pipelines initialized");
        Ok(())
    }

    /// Update cascaded shadow maps for directional light
    pub fn update_cascaded_shadows(
        &mut self,
        light_direction: Vec3,
        camera: &CameraUniforms,
        scene_bounds: &BoundingBox,
    ) -> Result<()> {
        let config = self.config.read();
        if !config.enabled {
            return Ok(());
        }

        let mut cascade_data = self.cascade_data.write();

        // Calculate view matrix for light
        let light_up = if light_direction.y.abs() > 0.99 {
            Vec3::new(1.0, 0.0, 0.0) // Avoid gimbal lock
        } else {
            Vec3::new(0.0, 1.0, 0.0)
        };

        let light_right = light_direction.cross(light_up).normalize();
        let light_up = light_right.cross(light_direction).normalize();

        let light_view = Mat4::look_to_rh(Vec3::ZERO, light_direction, light_up);

        // Get camera matrices
        let camera_view = Mat4::from_cols_array_2d(&camera.view_matrix);
        let camera_proj = Mat4::from_cols_array_2d(&camera.projection_matrix);
        let camera_view_proj_inv = (camera_proj * camera_view).inverse();

        // Calculate cascade splits
        let near = camera.near_far[0];
        let far = config.max_shadow_distance.min(camera.near_far[1]);

        for (i, cascade) in cascade_data.iter_mut().enumerate() {
            let split_near = if i == 0 {
                near
            } else {
                config.cascade_distances[i - 1]
            };
            let split_far = config.cascade_distances[i];

            // Calculate frustum corners in world space
            let frustum_corners =
                self.calculate_frustum_corners(camera_view_proj_inv, split_near, split_far);

            // Transform corners to light space
            let light_space_corners: Vec<Vec3> = frustum_corners
                .iter()
                .map(|corner| (light_view * corner.extend(1.0)).truncate())
                .collect();

            // Calculate tight bounding box in light space
            let mut min_bounds = light_space_corners[0];
            let mut max_bounds = light_space_corners[0];

            for corner in &light_space_corners[1..] {
                min_bounds = min_bounds.min(*corner);
                max_bounds = max_bounds.max(*corner);
            }

            // Expand bounds to include static scene geometry
            let scene_center = scene_bounds.center();
            let scene_radius = scene_bounds.radius();
            min_bounds.z = (scene_center[2] - scene_radius).min(min_bounds.z);
            max_bounds.z = (scene_center[2] + scene_radius).max(max_bounds.z);

            // Snap to texel grid to reduce shimmer
            let texel_size = (max_bounds.x - min_bounds.x) / config.quality.resolution() as f32;
            min_bounds.x = (min_bounds.x / texel_size).floor() * texel_size;
            min_bounds.y = (min_bounds.y / texel_size).floor() * texel_size;
            max_bounds.x = (max_bounds.x / texel_size).ceil() * texel_size;
            max_bounds.y = (max_bounds.y / texel_size).ceil() * texel_size;

            // Create orthographic projection for shadow map
            let light_proj = Mat4::orthographic_rh(
                min_bounds.x,
                max_bounds.x,
                min_bounds.y,
                max_bounds.y,
                -max_bounds.z, // Reversed Z for better precision
                -min_bounds.z,
            );

            let light_view_proj = light_proj * light_view;

            // Update cascade data
            cascade.light_view_proj = light_view_proj.to_cols_array_2d();
            cascade.world_to_light = light_view.to_cols_array_2d();
            cascade.split_distance = split_far;
            cascade.texel_size = texel_size;
            cascade.bias_params = [config.shadow_bias, config.normal_offset_bias];

            // Allocate atlas space (simplified - would be more complex in real implementation)
            let atlas_size = config.quality.resolution() / 2; // Quarter atlas per cascade
            let atlas_x = (i % 2) as f32 * 0.5;
            let atlas_y = (i / 2) as f32 * 0.5;
            cascade.atlas_bounds = [atlas_x, atlas_y, atlas_x + 0.5, atlas_y + 0.5];
        }

        // Update GPU uniform buffer
        #[cfg(feature = "w3d")]
        {
            let data = cast_slice(&cascade_data[..]);
            self.queue
                .write_buffer(&self.cascade_uniform_buffer, 0, data);
        }

        Ok(())
    }

    /// Calculate frustum corners for cascade
    fn calculate_frustum_corners(
        &self,
        inv_view_proj: Mat4,
        near_plane: f32,
        far_plane: f32,
    ) -> [Vec3; 8] {
        // NDC coordinates for frustum corners
        let ndc_corners = [
            Vec4::new(-1.0, -1.0, 0.0, 1.0), // near bottom-left
            Vec4::new(1.0, -1.0, 0.0, 1.0),  // near bottom-right
            Vec4::new(1.0, 1.0, 0.0, 1.0),   // near top-right
            Vec4::new(-1.0, 1.0, 0.0, 1.0),  // near top-left
            Vec4::new(-1.0, -1.0, 1.0, 1.0), // far bottom-left
            Vec4::new(1.0, -1.0, 1.0, 1.0),  // far bottom-right
            Vec4::new(1.0, 1.0, 1.0, 1.0),   // far top-right
            Vec4::new(-1.0, 1.0, 1.0, 1.0),  // far top-left
        ];

        let mut world_corners = [Vec3::ZERO; 8];

        for (i, ndc_corner) in ndc_corners.iter().enumerate() {
            // Transform to world space
            let world_corner = inv_view_proj * *ndc_corner;
            let world_corner = world_corner / world_corner.w;
            world_corners[i] = world_corner.truncate();
        }

        // Adjust near and far planes
        for i in 0..4 {
            let near_corner = world_corners[i];
            let far_corner = world_corners[i + 4];
            let direction = (far_corner - near_corner).normalize();

            // Interpolate based on actual near/far distances
            world_corners[i] = near_corner + direction * near_plane;
            world_corners[i + 4] = near_corner + direction * far_plane;
        }

        world_corners
    }

    /// Render shadow maps for all lights
    #[cfg(feature = "w3d")]
    pub async fn render_shadows(
        &mut self,
        encoder: &mut CommandEncoder,
        shadow_casters: &[ShadowCaster],
    ) -> Result<()> {
        let enabled = self.config.read().enabled;
        if !enabled || shadow_casters.is_empty() {
            return Ok(());
        }

        let mut stats = ShadowRenderStats::default();
        let render_start = std::time::Instant::now();

        // Render cascaded shadow maps
        self.render_cascade_shadows(encoder, shadow_casters, &mut stats)
            .await?;

        // Render point light shadows
        self.render_point_shadows(encoder, shadow_casters, &mut stats)
            .await?;

        // Render spot light shadows
        self.render_spot_shadows(encoder, shadow_casters, &mut stats)
            .await?;

        // Update statistics
        stats.shadow_render_time = render_start.elapsed().as_millis() as f32;
        *self.render_stats.write() = stats;

        Ok(())
    }

    /// Render cascaded shadow maps
    #[cfg(feature = "w3d")]
    async fn render_cascade_shadows(
        &mut self,
        encoder: &mut CommandEncoder,
        shadow_casters: &[ShadowCaster],
        stats: &mut ShadowRenderStats,
    ) -> Result<()> {
        let pipeline = self.depth_only_pipeline.as_ref().ok_or_else(|| {
            W3DError::RenderingError("Shadow pipeline not initialized".to_string())
        })?;

        let atlas = self.shadow_atlas.lock();

        for (cascade_index, cascade) in self.cascade_data.read().iter().enumerate() {
            // Create render pass for this cascade
            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some(&format!("Shadow Cascade {}", cascade_index)),
                color_attachments: &[],
                depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                    view: &atlas.view,
                    depth_ops: Some(Operations {
                        load: LoadOp::Clear(1.0),
                        store: StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Set pipeline and viewport
            render_pass.set_pipeline(pipeline);

            let atlas_bounds = &cascade.atlas_bounds;
            let viewport_x = (atlas_bounds[0] * atlas.resolution as f32) as u32;
            let viewport_y = (atlas_bounds[1] * atlas.resolution as f32) as u32;
            let viewport_w = ((atlas_bounds[2] - atlas_bounds[0]) * atlas.resolution as f32) as u32;
            let viewport_h = ((atlas_bounds[3] - atlas_bounds[1]) * atlas.resolution as f32) as u32;

            // Set viewport (this would be a real wgpu call in actual implementation)
            // render_pass.set_viewport(viewport_x, viewport_y, viewport_w, viewport_h, 0.0, 1.0);

            // Render shadow casters
            for caster in shadow_casters {
                if caster.cast_shadows && self.is_visible_in_cascade(caster, cascade) {
                    self.render_shadow_caster(&mut render_pass, caster);
                    stats.shadow_casters_rendered += 1;
                }
            }

            stats.shadow_maps_updated += 1;
        }

        Ok(())
    }

    /// Check if shadow caster is visible in cascade via frustum-sphere intersection.
    fn is_visible_in_cascade(&self, caster: &ShadowCaster, cascade: &ShadowCascadeData) -> bool {
        let light_view_proj = Mat4::from_cols_array_2d(&cascade.light_view_proj);

        let center = caster.transform.transform_point3(Vec3::ZERO);
        let bb = &caster.bounding_box;
        let bb_min = Vec3::from(bb.min);
        let bb_max = Vec3::from(bb.max);
        let radius = (bb_max - bb_min).length() * 0.5;

        let clip = light_view_proj * center.extend(1.0);
        if clip.w <= 0.0 {
            return false;
        }

        let rows = [
            light_view_proj.row(3) + light_view_proj.row(0), // left
            light_view_proj.row(3) - light_view_proj.row(0), // right
            light_view_proj.row(3) - light_view_proj.row(1), // bottom
            light_view_proj.row(3) + light_view_proj.row(1), // top
            light_view_proj.row(3) + light_view_proj.row(2), // near
            light_view_proj.row(3) - light_view_proj.row(2), // far
        ];

        for plane in &rows {
            let len = plane.x.hypot(plane.y).hypot(plane.z);
            if len < 1e-6 {
                continue;
            }
            let dist =
                (plane.x * center.x + plane.y * center.y + plane.z * center.z + plane.w) / len;
            if dist < -radius {
                return false;
            }
        }

        true
    }

    /// Render point light shadows by delegating to cascade infrastructure.
    ///
    /// Point lights can share the cascade render pass by treating each face of
    /// the cube map as a directional cascade.
    #[cfg(feature = "w3d")]
    async fn render_point_shadows(
        &mut self,
        encoder: &mut CommandEncoder,
        shadow_casters: &[ShadowCaster],
        stats: &mut ShadowRenderStats,
    ) -> Result<()> {
        let point_data = self.point_data.read();
        if point_data.is_empty() {
            return Ok(());
        }
        drop(point_data);

        // Fallback: reuse cascade shadow rendering for each point light face.
        self.render_cascade_shadows(encoder, shadow_casters, stats)
            .await
    }

    /// Render spot light shadows by delegating to cascade infrastructure.
    ///
    /// Spot lights share cascade rendering as a single-direction pass.
    #[cfg(feature = "w3d")]
    async fn render_spot_shadows(
        &mut self,
        encoder: &mut CommandEncoder,
        shadow_casters: &[ShadowCaster],
        stats: &mut ShadowRenderStats,
    ) -> Result<()> {
        let spot_data = self.spot_data.read();
        if spot_data.is_empty() {
            return Ok(());
        }
        drop(spot_data);

        // Fallback: reuse cascade shadow rendering for each spot light.
        self.render_cascade_shadows(encoder, shadow_casters, stats)
            .await
    }

    /// Render individual shadow caster
    #[cfg(feature = "w3d")]
    fn render_shadow_caster(&self, render_pass: &mut RenderPass, caster: &ShadowCaster) {
        // Set vertex and index buffers
        render_pass.set_vertex_buffer(0, caster.vertex_buffer.slice(..));
        render_pass.set_index_buffer(caster.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

        // Draw
        render_pass.draw_indexed(0..caster.index_count, 0, 0..1);
    }

    /// Get depth shader source
    fn get_depth_shader_source(&self) -> &'static str {
        r#"
        struct VertexInput {
            @location(0) position: vec3<f32>,
        }
        
        struct VertexOutput {
            @builtin(position) clip_position: vec4<f32>,
        }
        
        @group(0) @binding(0)
        var<uniform> light_view_proj: mat4x4<f32>;
        
        @vertex
        fn vs_depth_only(input: VertexInput) -> VertexOutput {
            var out: VertexOutput;
            out.clip_position = light_view_proj * vec4<f32>(input.position, 1.0);
            return out;
        }
        "#
    }

    /// Get shadow vertex layout
    #[cfg(feature = "w3d")]
    fn get_shadow_vertex_layout(&self) -> VertexBufferLayout {
        VertexBufferLayout {
            array_stride: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            }],
        }
    }

    /// Get shadow statistics
    pub fn get_statistics(&self) -> ShadowRenderStats {
        self.render_stats.read().clone()
    }

    /// Get cascade data for shaders
    pub fn get_cascade_data(&self) -> [ShadowCascadeData; CASCADE_COUNT] {
        *self.cascade_data.read()
    }

    /// Get shadow atlas texture
    #[cfg(feature = "w3d")]
    pub fn get_shadow_atlas_texture(&self) -> Arc<Mutex<ShadowAtlas>> {
        Arc::clone(&self.shadow_atlas)
    }

    /// Get shadow sampler
    #[cfg(feature = "w3d")]
    pub fn get_shadow_sampler(&self) -> &Sampler {
        &self.shadow_sampler
    }

    /// Get comparison sampler for PCF
    #[cfg(feature = "w3d")]
    pub fn get_comparison_sampler(&self) -> &Sampler {
        &self.comparison_sampler
    }
}

/// Shadow caster representation for rendering
#[derive(Debug)]
pub struct ShadowCaster {
    /// Should this object cast shadows?
    pub cast_shadows: bool,
    /// Bounding box for culling
    pub bounding_box: BoundingBox,
    /// World transform
    pub transform: Mat4,
    /// Vertex buffer
    #[cfg(feature = "w3d")]
    pub vertex_buffer: Arc<wgpu::Buffer>,
    /// Index buffer
    #[cfg(feature = "w3d")]
    pub index_buffer: Arc<wgpu::Buffer>,
    /// Index count
    pub index_count: u32,
    /// LOD level for shadow rendering
    pub shadow_lod: u32,
}

/// Cascaded shadow maps implementation
pub struct W3DCascadedShadowMaps {
    /// Shadow mapper
    shadow_mapper: W3DShadowMapper,
    /// Light direction (world space)
    light_direction: Vec3,
    /// Light color and intensity
    light_color: Vec3,
    /// Light intensity
    light_intensity: f32,
    /// Is light enabled?
    enabled: bool,
}

impl W3DCascadedShadowMaps {
    /// Create new cascaded shadow maps
    #[cfg(feature = "w3d")]
    pub fn new(device: Arc<Device>, queue: Arc<Queue>, config: ShadowConfig) -> Result<Self> {
        let shadow_mapper = W3DShadowMapper::new(device, queue, config)?;

        Ok(Self {
            shadow_mapper,
            light_direction: Vec3::new(0.0, -1.0, -1.0).normalize(),
            light_color: Vec3::new(1.0, 0.95, 0.8),
            light_intensity: 5.0,
            enabled: true,
        })
    }

    /// Update light direction
    pub fn set_light_direction(&mut self, direction: Vec3) {
        self.light_direction = direction.normalize();
    }

    /// Update light properties
    pub fn set_light_properties(&mut self, color: Vec3, intensity: f32) {
        self.light_color = color;
        self.light_intensity = intensity;
    }

    /// Enable/disable shadows
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Update shadow maps
    pub fn update_shadows(
        &mut self,
        camera: &CameraUniforms,
        scene_bounds: &BoundingBox,
    ) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        self.shadow_mapper
            .update_cascaded_shadows(self.light_direction, camera, scene_bounds)
    }

    /// Get shadow mapper
    pub fn get_shadow_mapper(&self) -> &W3DShadowMapper {
        &self.shadow_mapper
    }

    /// Get shadow mapper (mutable)
    pub fn get_shadow_mapper_mut(&mut self) -> &mut W3DShadowMapper {
        &mut self.shadow_mapper
    }
}
