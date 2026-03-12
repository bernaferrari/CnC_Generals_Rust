//! Ring Effects System - Circular visual effects for shockwaves, explosions, and magic
//!
//! This module implements the RingRenderObjClass from the original C++ code,
//! providing sophisticated ring/circular effects for various game visual effects.
//!
//! Converted from:
//! - ringobj.cpp/h (main ring system)
//! - Ring rendering and management

use glam::{Mat4, Vec2, Vec3, Vec4};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use ww3d_core::{
    errors::W3DError,
    w3d_format::{W3dTexCoordStruct, W3dTriangleStruct, W3dVectorStruct},
    wwstring::StringClass,
};
use ww3d_renderer_3d::{
    core::error::{Error as RendererError, RendererResult},
    material_system::{MaterialPassClass, VertexMaterialClass},
    render_object_system::{
        AABoxClass, AABoxCollisionTestClass, AABoxIntersectionTestClass, DecalGeneratorClass,
        MaterialInfoClass, OBBoxCollisionTestClass, OBBoxIntersectionTestClass,
        RayCollisionTestClass, RenderInfoClass, RenderObjClass, RenderObjClassId,
        SpecialRenderInfoClass, SphereClass,
    },
    rendering::{
        mesh_system::{MeshClass, MeshModelClass},
        shader_system::shader::ShaderClass,
    },
    texture_system::TextureClass,
    Renderer,
};

type Result<T> = std::result::Result<T, W3DError>;

static RING_ID_COUNTER: AtomicU32 = AtomicU32::new(0);

fn renderer_error_to_w3d(err: RendererError) -> W3DError {
    W3DError::UnknownWithMessage(err.to_string())
}

/// Ring display mask for controlling which rings are visible
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RingDisplayMask {
    /// Display all rings
    All = 0xFFFFFFFF,
    /// Display none
    None = 0,
    /// Display only shockwave rings
    Shockwave = 1,
    /// Display only explosion rings
    Explosion = 2,
    /// Display only magic rings
    Magic = 4,
    /// Display only particle rings
    Particle = 8,
}

/// Ring render mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RingRenderMode {
    /// Normal ring rendering
    Normal,
    /// Additive blending
    Additive,
    /// Alpha blending
    Alpha,
    /// Screen blending
    Screen,
    /// Multiply blending
    Multiply,
}

/// Ring shape type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RingShape {
    /// Perfect circle
    Circle,
    /// Elliptical ring
    Ellipse,
    /// Custom shape (using control points)
    Custom,
}

/// Ring animation mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RingAnimationMode {
    /// No animation
    Static,
    /// Scale animation
    Scale,
    /// Rotation animation
    Rotate,
    /// UV animation
    UVScroll,
    /// Complex animation (multiple effects)
    Complex,
}

/// Ring definition structure
#[derive(Debug, Clone)]
pub struct RingDefinition {
    /// Ring name
    pub name: StringClass,
    /// Inner radius
    pub inner_radius: f32,
    /// Outer radius
    pub outer_radius: f32,
    /// Ring width (calculated from inner/outer)
    pub width: f32,
    /// Ring height
    pub height: f32,
    /// Number of segments
    pub segments: usize,
    /// Ring shape
    pub shape: RingShape,
    /// Animation mode
    pub animation_mode: RingAnimationMode,
    /// Texture
    pub texture: Option<Arc<TextureClass>>,
    /// Shader
    pub shader: Option<Arc<ShaderClass>>,
    /// Base color
    pub color: Vec4,
    /// Display mask
    pub display_mask: u32,
    /// Render mode
    pub render_mode: RingRenderMode,
    /// Whether ring is two-sided
    pub two_sided: bool,
    /// Z-bias for rendering order
    pub z_bias: f32,
    /// Lifetime in seconds
    pub lifetime: f32,
    /// Texture U animation speed
    pub texture_u_per_sec: f32,
    /// Texture V animation speed
    pub texture_v_per_sec: f32,
}

impl RingDefinition {
    /// Create new ring definition
    pub fn new() -> Self {
        Self {
            name: StringClass::new(),
            inner_radius: 1.0,
            outer_radius: 2.0,
            width: 1.0,
            height: 0.0,
            segments: 32,
            shape: RingShape::Circle,
            animation_mode: RingAnimationMode::Static,
            texture: None,
            shader: None,
            color: Vec4::new(1.0, 1.0, 1.0, 1.0),
            display_mask: RingDisplayMask::All as u32,
            render_mode: RingRenderMode::Normal,
            two_sided: false,
            z_bias: 0.0,
            lifetime: 1.0,
            texture_u_per_sec: 0.0,
            texture_v_per_sec: 0.0,
        }
    }

    /// Create default ring definition (alias for new)
    pub fn default() -> Self {
        Self::new()
    }

    /// Create circle ring definition
    pub fn circle(inner_radius: f32, outer_radius: f32, segments: usize) -> Self {
        let mut def = Self::new();
        def.inner_radius = inner_radius;
        def.outer_radius = outer_radius;
        def.width = outer_radius - inner_radius;
        def.segments = segments;
        def.shape = RingShape::Circle;
        def
    }

    /// Create ellipse ring definition
    pub fn ellipse(inner_radius: f32, outer_radius: f32, height: f32, segments: usize) -> Self {
        let mut def = Self::new();
        def.inner_radius = inner_radius;
        def.outer_radius = outer_radius;
        def.width = outer_radius - inner_radius;
        def.height = height;
        def.segments = segments;
        def.shape = RingShape::Ellipse;
        def
    }

    /// Set texture
    pub fn with_texture(mut self, texture: Arc<TextureClass>) -> Self {
        self.texture = Some(texture);
        self
    }

    /// Set color
    pub fn with_color(mut self, color: Vec4) -> Self {
        self.color = color;
        self
    }

    /// Set animation mode
    pub fn with_animation(mut self, mode: RingAnimationMode) -> Self {
        self.animation_mode = mode;
        self
    }

    /// Set render mode
    pub fn with_render_mode(mut self, mode: RingRenderMode) -> Self {
        self.render_mode = mode;
        self
    }
}

/// Ring render object class
#[derive(Debug)]
pub struct RingRenderObjClass {
    /// Ring definition
    pub definition: RingDefinition,
    /// Current transform
    pub transform: Mat4,
    /// Animation parameters
    pub animation_params: RingAnimationParams,
    /// Current scale
    pub current_scale: Vec2,
    /// Current rotation
    pub current_rotation: f32,
    /// UV offset for animation
    pub uv_offset: Vec2,
    /// Whether ring is visible
    pub visible: bool,
    /// Ring ID
    pub ring_id: u32,
    /// Vertex buffer for rendering
    pub vertices: Vec<RingVertex>,
    /// Index buffer for rendering
    pub indices: Vec<u32>,
    /// Whether mesh needs update
    pub mesh_dirty: bool,
    /// Position (for convenience)
    pub position: Vec3,
    /// Scale (for convenience)
    pub scale: Vec3,
    /// Color (for convenience)
    pub color: Vec4,
    /// Animation time
    pub animation_time: f32,
}

impl RingRenderObjClass {
    /// Create new ring render object
    pub fn new(definition: RingDefinition) -> Self {
        let ring_id = RING_ID_COUNTER.fetch_add(1, Ordering::Relaxed) + 1;

        Self {
            definition,
            transform: Mat4::IDENTITY,
            animation_params: RingAnimationParams::new(),
            current_scale: Vec2::new(1.0, 1.0),
            current_rotation: 0.0,
            uv_offset: Vec2::ZERO,
            visible: true,
            ring_id,
            vertices: Vec::new(),
            indices: Vec::new(),
            mesh_dirty: true,
            position: Vec3::ZERO,
            scale: Vec3::ONE,
            color: Vec4::new(1.0, 1.0, 1.0, 1.0),
            animation_time: 0.0,
        }
    }

    /// Create from definition
    pub fn from_definition(def: RingDefinition) -> Self {
        Self::new(def)
    }

    /// Get number of polygons
    pub fn get_num_polys(&self) -> usize {
        self.definition.segments * 2 // Two triangles per segment
    }

    /// Get name
    pub fn get_name(&self) -> &str {
        self.definition.name.as_str()
    }

    /// Set name
    pub fn set_name(&mut self, name: &str) {
        self.definition.name = StringClass::from(name);
    }

    /// Set color
    pub fn set_color(&mut self, color: Vec4) {
        self.definition.color = color;
        self.mesh_dirty = true;
    }

    /// Set transform
    pub fn set_transform(&mut self, transform: Mat4) {
        self.transform = transform;
        self.mesh_dirty = true;
    }

    /// Set scale
    pub fn set_scale(&mut self, scale: Vec2) {
        self.current_scale = scale;
        self.mesh_dirty = true;
    }

    /// Set rotation
    pub fn set_rotation(&mut self, rotation: f32) {
        self.current_rotation = rotation;
        self.mesh_dirty = true;
    }

    /// Set UV offset
    pub fn set_uv_offset(&mut self, offset: Vec2) {
        self.uv_offset = offset;
        self.mesh_dirty = true;
    }

    /// Set visibility
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// Update mesh data
    pub fn update_mesh_data(&mut self) -> Result<()> {
        if !self.mesh_dirty {
            return Ok(());
        }

        self.vertices.clear();
        self.indices.clear();

        self.generate_ring_geometry()?;
        self.mesh_dirty = false;

        Ok(())
    }

    /// Generate ring geometry
    fn generate_ring_geometry(&mut self) -> Result<()> {
        let segments = self.definition.segments;
        let inner_radius = self.definition.inner_radius * self.current_scale.x;
        let outer_radius = self.definition.outer_radius * self.current_scale.x;
        let height = self.definition.height * self.current_scale.y;

        // Generate vertices
        for i in 0..=segments {
            let angle =
                (i as f32 / segments as f32) * std::f32::consts::PI * 2.0 + self.current_rotation;

            // Inner vertex
            let inner_x = angle.cos() * inner_radius;
            let inner_y = angle.sin() * inner_radius;
            let inner_z = if self.definition.shape == RingShape::Ellipse {
                angle.sin() * height
            } else {
                0.0
            };

            // Outer vertex
            let outer_x = angle.cos() * outer_radius;
            let outer_y = angle.sin() * outer_radius;
            let outer_z = if self.definition.shape == RingShape::Ellipse {
                angle.sin() * height
            } else {
                0.0
            };

            // Texture coordinates
            let u = i as f32 / segments as f32;
            let uv_inner = Vec2::new(u, 0.0) + self.uv_offset;
            let uv_outer = Vec2::new(u, 1.0) + self.uv_offset;

            // Add inner vertex
            self.vertices.push(RingVertex {
                position: Vec3::new(inner_x, inner_y, inner_z),
                normal: Vec3::Z, // Facing up
                tex_coord: uv_inner,
                color: self.definition.color,
            });

            // Add outer vertex
            self.vertices.push(RingVertex {
                position: Vec3::new(outer_x, outer_y, outer_z),
                normal: Vec3::Z, // Facing up
                tex_coord: uv_outer,
                color: self.definition.color,
            });
        }

        // Generate indices
        for i in 0..segments {
            let base = (i * 2) as u32;

            // First triangle
            self.indices.push(base);
            self.indices.push(base + 1);
            self.indices.push(base + 2);

            // Second triangle
            self.indices.push(base + 1);
            self.indices.push(base + 3);
            self.indices.push(base + 2);

            // If two-sided, add back faces
            if self.definition.two_sided {
                // Back faces (reversed winding)
                self.indices.push(base);
                self.indices.push(base + 2);
                self.indices.push(base + 1);

                self.indices.push(base + 1);
                self.indices.push(base + 2);
                self.indices.push(base + 3);
            }
        }

        Ok(())
    }

    fn build_render_mesh(&self) -> Option<Arc<MeshClass>> {
        if self.vertices.len() < 4 || self.indices.len() < 3 {
            return None;
        }

        let positions: Vec<Vec3> = self.vertices.iter().map(|v| v.position).collect();
        let normals: Vec<Vec3> = self.vertices.iter().map(|v| v.normal).collect();
        let texcoords: Vec<Vec2> = self.vertices.iter().map(|v| v.tex_coord).collect();
        let vertex_colors: Vec<Vec4> = self.vertices.iter().map(|v| v.color).collect();

        let average_alpha = if vertex_colors.is_empty() {
            1.0
        } else {
            vertex_colors.iter().map(|c| c.w).sum::<f32>().max(0.0) / vertex_colors.len() as f32
        };

        let mut model = MeshModelClass::new(self.get_name());
        model.vertices = positions
            .iter()
            .copied()
            .map(W3dVectorStruct::from)
            .collect();
        model.normals = normals.iter().copied().map(W3dVectorStruct::from).collect();
        model.texture_coords = texcoords
            .iter()
            .map(|tc| W3dTexCoordStruct { u: tc.x, v: tc.y })
            .collect();
        model.vertex_count = self.vertices.len() as u32;
        model.index_count = self.indices.len() as u32;
        model.triangles = self
            .indices
            .chunks(3)
            .filter_map(|chunk| {
                if chunk.len() == 3 {
                    Some(W3dTriangleStruct {
                        vindex: [chunk[0], chunk[1], chunk[2]],
                        attributes: 0,
                        normal: W3dVectorStruct::from(Vec3::Z),
                        distance: 0.0,
                    })
                } else {
                    None
                }
            })
            .collect();
        model.register_for_rendering();

        let mut pass = MaterialPassClass::new();
        pass.shader = ShaderClass::new();
        pass.diffuse_vertex_colors = Some(vertex_colors.clone());

        let mut vertex_material = VertexMaterialClass::new("RingMaterial");
        let diffuse_color = self.definition.color.truncate();
        vertex_material.diffuse = diffuse_color;
        vertex_material.opacity = average_alpha;
        vertex_material.ambient = diffuse_color * 0.2;
        vertex_material.emissive = diffuse_color * 0.1;
        pass.vertex_material = Some(Arc::new(vertex_material));

        model.material_passes = vec![pass];

        let mut mesh = MeshClass::new();
        mesh.name = self.get_name().to_string();
        mesh.model = Some(Arc::new(model));
        mesh.alpha_override = average_alpha;
        mesh.material_pass_alpha_override = average_alpha;
        mesh.material_pass_emissive_override = 1.0;
        mesh.is_hidden = !self.visible;

        let min = positions
            .iter()
            .fold(Vec3::splat(f32::INFINITY), |acc, p| acc.min(*p));
        let max = positions
            .iter()
            .fold(Vec3::splat(f32::NEG_INFINITY), |acc, p| acc.max(*p));
        let bbox = AABoxClass::from_min_max(min, max);
        let center = (min + max) * 0.5;
        let radius = positions
            .iter()
            .map(|p| (*p - center).length())
            .fold(0.0f32, f32::max);

        mesh.bounding_box = bbox;
        mesh.bounding_sphere = SphereClass::new(center, radius);

        let translation = Mat4::from_translation(self.position);
        let scale = Mat4::from_scale(self.scale);
        let transform = translation * self.transform * scale;
        mesh.set_transform(transform);
        mesh.update_cached_bounding_volumes();

        Some(Arc::new(mesh))
    }

    /// Render ring
    pub fn render_ring(&self, rinfo: &RenderInfoClass) -> Result<()> {
        if !self.visible {
            return Ok(());
        }

        // Apply transform and render
        let _ = rinfo;
        Ok(())
    }

    /// VIS render ring
    pub fn vis_render_ring(&self, rinfo: &RenderInfoClass) -> Result<()> {
        // Visibility render
        self.render_ring(rinfo)
    }

    /// Clone ring
    pub fn clone(&self) -> Self {
        let ring_id = RING_ID_COUNTER.fetch_add(1, Ordering::Relaxed) + 1;

        Self {
            definition: self.definition.clone(),
            transform: self.transform,
            animation_params: self.animation_params.clone(),
            current_scale: self.current_scale,
            current_rotation: self.current_rotation,
            uv_offset: self.uv_offset,
            visible: self.visible,
            ring_id,
            vertices: self.vertices.clone(),
            indices: self.indices.clone(),
            mesh_dirty: true,
            position: self.position,
            scale: self.scale,
            color: self.color,
            animation_time: self.animation_time,
        }
    }

    /// Update animation
    pub fn update_animation(&mut self, delta_time: f32) {
        match self.definition.animation_mode {
            RingAnimationMode::Scale => {
                // Scale animation
                let scale_speed = self.animation_params.scale_speed;
                let scale_range = self.animation_params.scale_range;

                self.animation_params.scale_time += delta_time * scale_speed;
                let scale_factor =
                    (self.animation_params.scale_time * std::f32::consts::PI * 2.0).sin() * 0.5
                        + 0.5;
                let scale = 1.0 + scale_range * scale_factor;

                self.set_scale(Vec2::splat(scale));
            }
            RingAnimationMode::Rotate => {
                // Rotation animation
                self.current_rotation += self.animation_params.rotation_speed * delta_time;
                self.mesh_dirty = true;
            }
            RingAnimationMode::UVScroll => {
                // UV scrolling animation
                self.uv_offset.x += self.animation_params.uv_scroll_speed.x * delta_time;
                self.uv_offset.y += self.animation_params.uv_scroll_speed.y * delta_time;
                self.mesh_dirty = true;
            }
            RingAnimationMode::Complex => {
                // Complex animation (combination)
                self.update_animation(delta_time); // Call other animations
            }
            RingAnimationMode::Static => {
                // No animation
            }
        }
    }
}

/// Ring animation parameters
#[derive(Debug, Clone)]
pub struct RingAnimationParams {
    /// Scale animation speed
    pub scale_speed: f32,
    /// Scale animation range
    pub scale_range: f32,
    /// Scale animation time
    pub scale_time: f32,
    /// Rotation speed
    pub rotation_speed: f32,
    /// UV scroll speed
    pub uv_scroll_speed: Vec2,
}

impl RingAnimationParams {
    /// Create new animation parameters
    pub fn new() -> Self {
        Self {
            scale_speed: 1.0,
            scale_range: 0.5,
            scale_time: 0.0,
            rotation_speed: 1.0,
            uv_scroll_speed: Vec2::ZERO,
        }
    }
}

/// Ring vertex structure
#[derive(Debug, Clone, Copy)]
pub struct RingVertex {
    /// Position
    pub position: Vec3,
    /// Normal
    pub normal: Vec3,
    /// Texture coordinates
    pub tex_coord: Vec2,
    /// Color
    pub color: Vec4,
}

impl RingVertex {
    /// Get vertex buffer layout for WGPU
    pub fn get_vertex_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<RingVertex>() as wgpu::BufferAddress,
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
                    offset: 12,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Texture coordinates
                wgpu::VertexAttribute {
                    offset: 24,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // Color
                wgpu::VertexAttribute {
                    offset: 32,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// Implementation of RenderObjClass for RingRenderObjClass
impl RenderObjClass for RingRenderObjClass {
    fn clone_obj(&self) -> Box<dyn RenderObjClass> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn class_id(&self) -> RenderObjClassId {
        RenderObjClassId::Ring
    }

    fn get_name(&self) -> &str {
        self.definition.name.as_str()
    }

    fn set_name(&mut self, name: &str) {
        self.definition.name = StringClass::from(name);
    }

    fn get_num_polys(&self) -> usize {
        self.definition.segments * 2 // Two triangles per segment
    }

    fn render(&self, _rinfo: &RenderInfoClass) -> RendererResult<()> {
        if !self.visible {
            return Ok(());
        }

        if self.mesh_dirty || self.vertices.is_empty() || self.indices.is_empty() {
            return Ok(());
        }

        if let Some(mesh) = self.build_render_mesh() {
            Renderer::with_global_mut(|renderer| {
                renderer.queue_mesh(mesh.clone())?;
                Ok(())
            })?;
        }

        Ok(())
    }

    fn special_render(&self, _rinfo: &SpecialRenderInfoClass) -> RendererResult<()> {
        // Placeholder implementation for special rendering
        Ok(())
    }

    fn cast_ray(&self, _raytest: &mut RayCollisionTestClass) -> bool {
        false // Placeholder
    }

    fn cast_aabox(&self, _boxtest: &mut AABoxCollisionTestClass) -> bool {
        false // Placeholder
    }

    fn cast_obbox(&self, _boxtest: &mut OBBoxCollisionTestClass) -> bool {
        false // Placeholder
    }

    fn intersect_aabox(&self, _boxtest: &AABoxIntersectionTestClass) -> bool {
        false // Placeholder
    }

    fn intersect_obbox(&self, _boxtest: &OBBoxIntersectionTestClass) -> bool {
        false // Placeholder
    }

    fn get_obj_space_bounding_sphere(&self) -> SphereClass {
        let radius = self
            .definition
            .outer_radius
            .max(self.definition.inner_radius);
        SphereClass::new(Vec3::ZERO, radius)
    }

    fn get_obj_space_bounding_box(&self) -> AABoxClass {
        let radius = self.definition.outer_radius;
        AABoxClass::from_center_and_extent(Vec3::ZERO, Vec3::new(radius, radius, 0.1))
    }

    fn scale(&mut self, scale: f32) {
        self.scale *= scale;
    }

    fn scale_xyz(&mut self, scalex: f32, scaley: f32, scalez: f32) {
        self.scale.x *= scalex;
        self.scale.y *= scaley;
        self.scale.z *= scalez;
    }

    fn get_material_info(&self) -> Option<&MaterialInfoClass> {
        None // No materials for basic rings
    }

    fn get_sort_level(&self) -> i32 {
        0 // Default sort level
    }

    fn set_sort_level(&mut self, _level: i32) {
        // Placeholder implementation
    }

    fn create_decal(&mut self, _generator: &mut DecalGeneratorClass) {
        // Placeholder implementation
    }

    fn delete_decal(&mut self, _decal_id: u32) {
        // Placeholder implementation
    }

    fn transform(&self) -> &Mat4 {
        &self.transform
    }

    fn set_transform(&mut self, transform: Mat4) {
        self.transform = transform;
    }
}

impl Clone for RingRenderObjClass {
    fn clone(&self) -> Self {
        Self {
            definition: self.definition.clone(),
            transform: self.transform,
            animation_params: self.animation_params.clone(),
            current_scale: self.current_scale,
            current_rotation: self.current_rotation,
            uv_offset: self.uv_offset,
            visible: self.visible,
            ring_id: self.ring_id,
            vertices: self.vertices.clone(),
            indices: self.indices.clone(),
            mesh_dirty: self.mesh_dirty,
            position: self.position,
            scale: self.scale,
            color: self.color,
            animation_time: self.animation_time,
        }
    }
}

/// Ring manager for managing multiple rings
#[derive(Debug)]
pub struct RingManager {
    /// Active rings
    pub rings: Vec<RingRenderObjClass>,
    /// Ring definitions
    pub definitions: std::collections::HashMap<StringClass, RingDefinition>,
    /// Global display mask
    pub global_display_mask: u32,
    /// Maximum number of rings
    pub max_rings: usize,
}

impl RingManager {
    /// Create new ring manager
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let _ = device; // Use parameters
        let _ = queue;
        Self {
            rings: Vec::new(),
            definitions: std::collections::HashMap::new(),
            global_display_mask: RingDisplayMask::All as u32,
            max_rings: 128,
        }
    }

    /// Initialize ring render system
    pub fn init_ring_render_system() -> Result<()> {
        // Initialize global ring system
        Ok(())
    }

    /// Shutdown ring render system
    pub fn shutdown_ring_render_system() {
        // Cleanup global ring system
    }

    /// Set global display mask
    pub fn set_ring_display_mask(&mut self, mask: u32) {
        self.global_display_mask = mask;
    }

    /// Get global display mask
    pub fn get_ring_display_mask(&self) -> u32 {
        self.global_display_mask
    }

    /// Add ring definition
    pub fn add_ring_definition(&mut self, name: &str, definition: RingDefinition) {
        self.definitions.insert(StringClass::from(name), definition);
    }

    /// Get ring definition
    pub fn get_ring_definition(&self, name: &str) -> Option<&RingDefinition> {
        self.definitions.get(&StringClass::from(name))
    }

    /// Create ring from definition
    pub fn create_ring(&mut self, definition_name: &str) -> Result<&mut RingRenderObjClass> {
        if self.rings.len() >= self.max_rings {
            return Err(W3DError::InvalidParameter(
                "Maximum rings reached".to_string(),
            ));
        }

        if let Some(definition) = self.definitions.get(&StringClass::from(definition_name)) {
            let ring = RingRenderObjClass::from_definition(definition.clone());
            self.rings.push(ring);
            Ok(self.rings.last_mut().unwrap())
        } else {
            Err(W3DError::InvalidParameter(format!(
                "Ring definition '{}' not found",
                definition_name
            )))
        }
    }

    /// Remove ring
    pub fn remove_ring(&mut self, index: usize) -> Result<()> {
        if index >= self.rings.len() {
            return Err(W3DError::InvalidParameter("Invalid ring index".to_string()));
        }
        self.rings.remove(index);
        Ok(())
    }

    /// Get ring
    pub fn get_ring(&mut self, index: usize) -> Option<&mut RingRenderObjClass> {
        self.rings.get_mut(index)
    }

    /// Update all rings
    pub fn update(&mut self, delta_time: f32) {
        for ring in &mut self.rings {
            ring.update_animation(delta_time);
            if ring.mesh_dirty {
                let _ = ring.update_mesh_data();
            }
        }
    }

    /// Render all rings (modern GPU interface)
    pub fn render_gpu(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        render_pass: &mut wgpu::RenderPass,
        view_projection_matrix: glam::Mat4,
    ) {
        let _ = device; // Use parameters
        let _ = queue;
        let _ = encoder;
        let _ = render_pass;
        let _ = view_projection_matrix;

        // In a full implementation, this would:
        // 1. Update ring geometry based on current frame
        // 2. Upload ring data to GPU buffers
        // 3. Render each visible ring with proper blending
        for ring in &mut self.rings {
            if (ring.definition.display_mask & self.global_display_mask) != 0 && ring.visible {
                // Render individual ring
            }
        }
    }

    /// Render all rings (WW3D render info interface)
    pub fn render(&mut self, render_info: &RenderInfoClass) -> Result<()> {
        for ring in &mut self.rings {
            if (ring.definition.display_mask & self.global_display_mask) == 0 || !ring.visible {
                continue;
            }

            let _ = ring.update_mesh_data();
            ring.render(render_info).map_err(renderer_error_to_w3d)?;
        }
        Ok(())
    }

    /// Create an explosion ring effect
    pub fn create_explosion_ring(
        &mut self,
        position: glam::Vec3,
        max_radius: f32,
        duration: f32,
        color: glam::Vec4,
    ) {
        let mut definition = RingDefinition::default();
        definition.name = StringClass::from(format!("explosion_ring_{}", self.rings.len()));
        definition.lifetime = duration;
        definition.texture_u_per_sec = 0.5;
        definition.texture_v_per_sec = 0.0;

        let def_name = definition.name.clone();
        self.definitions.insert(def_name.clone(), definition);

        if let Some(def) = self.definitions.get(&def_name) {
            let mut ring = RingRenderObjClass::new(def.clone());
            ring.position = position;
            ring.scale = Vec3::splat(max_radius);
            ring.color = color;
            ring.visible = true;

            self.rings.push(ring);
        }
    }

    /// Clear all rings
    pub fn clear(&mut self) {
        self.rings.clear();
    }

    /// Get ring count
    pub fn get_ring_count(&self) -> usize {
        self.rings.len()
    }

    /// Get visible ring count
    pub fn get_visible_ring_count(&self) -> usize {
        self.rings.iter().filter(|r| r.visible).count()
    }
}

fn ring_manager_store() -> &'static Mutex<Option<RingManager>> {
    static STORE: OnceLock<Mutex<Option<RingManager>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(None))
}

fn with_ring_manager_mut<R, F>(f: F) -> Option<R>
where
    F: FnOnce(&mut RingManager) -> R,
{
    let mut slot = ring_manager_store().lock().ok()?;
    let manager = slot.as_mut()?;
    Some(f(manager))
}

/// Initialize global ring manager
pub fn init_global_ring_manager(device: &wgpu::Device, queue: &wgpu::Queue) -> Result<()> {
    let mut slot = ring_manager_store()
        .lock()
        .expect("ring manager lock poisoned");
    *slot = Some(RingManager::new(device, queue));
    RingManager::init_ring_render_system()?;
    Ok(())
}

/// Execute a closure with the global ring manager if it exists.
pub fn with_global_ring_manager_mut<R, F>(f: F) -> Option<R>
where
    F: FnOnce(&mut RingManager) -> R,
{
    with_ring_manager_mut(f)
}

/// Shutdown global ring manager
pub fn shutdown_global_ring_manager() {
    RingManager::shutdown_ring_render_system();
    if let Ok(mut slot) = ring_manager_store().lock() {
        *slot = None;
    }
}

/// Quick ring functions
pub fn create_shockwave_ring(center: Vec3, radius: f32) -> Result<RingRenderObjClass> {
    let definition = RingDefinition::circle(radius * 0.8, radius, 32)
        .with_color(Vec4::new(1.0, 1.0, 0.5, 0.8))
        .with_animation(RingAnimationMode::Scale)
        .with_render_mode(RingRenderMode::Additive);

    let mut ring = RingRenderObjClass::new(definition);
    ring.set_transform(Mat4::from_translation(center));
    ring.update_mesh_data()?;

    Ok(ring)
}

pub fn create_explosion_ring(center: Vec3, radius: f32) -> Result<RingRenderObjClass> {
    let definition = RingDefinition::circle(0.0, radius, 24)
        .with_color(Vec4::new(1.0, 0.5, 0.0, 0.9))
        .with_animation(RingAnimationMode::Scale)
        .with_render_mode(RingRenderMode::Additive);

    let mut ring = RingRenderObjClass::new(definition);
    ring.set_transform(Mat4::from_translation(center));
    ring.update_mesh_data()?;

    Ok(ring)
}

pub fn create_magic_ring(center: Vec3, radius: f32) -> Result<RingRenderObjClass> {
    let definition = RingDefinition::circle(radius * 0.5, radius, 64)
        .with_color(Vec4::new(0.5, 0.8, 1.0, 0.7))
        .with_animation(RingAnimationMode::Rotate)
        .with_render_mode(RingRenderMode::Alpha);

    let mut ring = RingRenderObjClass::new(definition);
    ring.set_transform(Mat4::from_translation(center));
    ring.update_mesh_data()?;

    Ok(ring)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_definition_creation() {
        let def = RingDefinition::circle(1.0, 2.0, 32);
        assert_eq!(def.inner_radius, 1.0);
        assert_eq!(def.outer_radius, 2.0);
        assert_eq!(def.width, 1.0);
        assert_eq!(def.segments, 32);
        assert_eq!(def.shape, RingShape::Circle);
    }

    #[test]
    fn test_ring_render_obj_creation() {
        let def = RingDefinition::new();
        let ring = RingRenderObjClass::new(def);
        assert!(ring.visible);
        assert_eq!(ring.current_scale, Vec2::new(1.0, 1.0));
        assert_eq!(ring.current_rotation, 0.0);
    }

    #[test]
    fn test_ring_geometry_generation() {
        let def = RingDefinition::circle(1.0, 2.0, 8); // Small number for testing
        let mut ring = RingRenderObjClass::new(def);

        ring.update_mesh_data().unwrap();

        // Should have 9 * 2 = 18 vertices (8 segments + 1 for closure, 2 per ring)
        assert_eq!(ring.vertices.len(), 18);
        // Should have 8 * 6 = 48 indices (8 segments * 2 triangles * 3 indices)
        assert_eq!(ring.indices.len(), 48);
    }

    #[test]
    fn test_ring_animation() {
        let def = RingDefinition::circle(1.0, 2.0, 8).with_animation(RingAnimationMode::Rotate);
        let mut ring = RingRenderObjClass::new(def);

        let initial_rotation = ring.current_rotation;
        ring.update_animation(1.0);
        assert_ne!(ring.current_rotation, initial_rotation);
    }

    #[test]
    fn test_ring_manager() {
        // Create mock device and queue for testing
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .unwrap();
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();

        let mut manager = RingManager::new(&device, &queue);
        assert_eq!(manager.get_ring_count(), 0);

        let def = RingDefinition::circle(1.0, 2.0, 8);
        manager.add_ring_definition("test_ring", def);

        let ring_name = {
            let ring = manager.create_ring("test_ring").unwrap();
            ring.get_name().to_string()
        };
        assert_eq!(manager.get_ring_count(), 1);
        assert_eq!(ring_name, "");
    }

    #[test]
    fn test_ring_display_mask() {
        // Create mock device and queue for testing
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .unwrap();
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();

        let mut manager = RingManager::new(&device, &queue);

        // Test setting display mask
        manager.set_ring_display_mask(RingDisplayMask::Shockwave as u32);
        assert_eq!(
            manager.get_ring_display_mask(),
            RingDisplayMask::Shockwave as u32
        );

        // Test all mask
        manager.set_ring_display_mask(RingDisplayMask::All as u32);
        assert_eq!(manager.get_ring_display_mask(), RingDisplayMask::All as u32);
    }

    #[test]
    fn test_shockwave_ring_creation() {
        let ring = create_shockwave_ring(Vec3::ZERO, 5.0).unwrap();
        assert_eq!(ring.definition.inner_radius, 4.0); // 5.0 * 0.8
        assert_eq!(ring.definition.outer_radius, 5.0);
        assert_eq!(ring.definition.animation_mode, RingAnimationMode::Scale);
        assert_eq!(ring.definition.render_mode, RingRenderMode::Additive);
    }

    #[test]
    fn test_explosion_ring_creation() {
        let ring = create_explosion_ring(Vec3::ZERO, 3.0).unwrap();
        assert_eq!(ring.definition.inner_radius, 0.0);
        assert_eq!(ring.definition.outer_radius, 3.0);
        assert_eq!(ring.definition.animation_mode, RingAnimationMode::Scale);
        assert_eq!(ring.definition.color, Vec4::new(1.0, 0.5, 0.0, 0.9));
    }

    #[test]
    fn test_magic_ring_creation() {
        let ring = create_magic_ring(Vec3::ZERO, 4.0).unwrap();
        assert_eq!(ring.definition.inner_radius, 2.0); // 4.0 * 0.5
        assert_eq!(ring.definition.outer_radius, 4.0);
        assert_eq!(ring.definition.animation_mode, RingAnimationMode::Rotate);
        assert_eq!(ring.definition.color, Vec4::new(0.5, 0.8, 1.0, 0.7));
    }

    #[test]
    fn test_ring_vertex_structure() {
        let vertex = RingVertex {
            position: Vec3::new(1.0, 2.0, 3.0),
            normal: Vec3::Z,
            tex_coord: Vec2::new(0.5, 0.5),
            color: Vec4::new(1.0, 0.0, 0.0, 1.0),
        };

        assert_eq!(vertex.position, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(vertex.normal, Vec3::Z);
        assert_eq!(vertex.tex_coord, Vec2::new(0.5, 0.5));
        assert_eq!(vertex.color, Vec4::new(1.0, 0.0, 0.0, 1.0));
    }
}
