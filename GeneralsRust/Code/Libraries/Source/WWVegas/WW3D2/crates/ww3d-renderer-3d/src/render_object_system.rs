//! Render Object System - Core rendering object management
//!
//! This module provides the fundamental interfaces and types for render objects
//! in the WW3D engine, equivalent to the C++ RenderObjClass hierarchy.

use crate::core::error::RendererResult;
use crate::rendering::mesh_system::MeshClass;
use bitflags::bitflags;
use glam::{Mat3, Mat4, Quat, Vec3, Vec4};
use std::any::Any;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
pub use ww3d_core::RenderObjClassId;
use ww3d_geometry::LineSegment;

const MAX_ADDITIONAL_MATERIAL_PASSES: usize = 32;
const MAX_OVERRIDE_FLAG_LEVEL: usize = 32;

/// Registration types used when a render object needs to hook into additional
/// scene processing paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SceneRegistrationType {
    /// Register for `on_frame_update` callbacks.
    OnFrameUpdate,
    /// Register as a light contributor so the scene gathers it for the light environment.
    Light,
    /// Register for deferred release processing.
    Release,
}

/// Interface exposed to render objects when they are added to or removed from a scene.
pub trait SceneBinding {
    /// Register the object for the requested processing stream.
    fn register(&mut self, object_id: usize, registration: SceneRegistrationType);
    /// Unregister the object from the requested processing stream.
    fn unregister(&mut self, object_id: usize, registration: SceneRegistrationType);
}

/// Base render object interface
pub trait RenderObjClass: std::fmt::Debug + Send + Sync + Any {
    /// Clone the render object
    fn clone_obj(&self) -> Box<dyn RenderObjClass>;

    /// Get the class ID
    fn class_id(&self) -> RenderObjClassId;

    /// Get the name of this object
    fn get_name(&self) -> &str;

    /// Set the name of this object
    fn set_name(&mut self, name: &str);

    /// Get the number of polygons in this object
    fn get_num_polys(&self) -> usize;

    /// Render the object
    fn render(&self, rinfo: &RenderInfoClass) -> RendererResult<()>;

    /// Special render (for custom rendering passes)
    fn special_render(&self, rinfo: &SpecialRenderInfoClass) -> RendererResult<()>;

    /// Called once per frame for objects registered via `SceneRegistrationType::OnFrameUpdate`.
    fn on_frame_update(&mut self, _delta_time: f32) -> RendererResult<()> {
        Ok(())
    }

    /// Called immediately before the object is rendered. Return `false` to skip drawing.
    fn pre_render(&self, _rinfo: &RenderInfoClass) -> RendererResult<bool> {
        Ok(true)
    }

    /// Called after the object finishes rendering.
    fn post_render(&self, _rinfo: &RenderInfoClass) -> RendererResult<()> {
        Ok(())
    }

    fn as_any(&self) -> &dyn Any;

    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// Cast ray for collision detection
    fn cast_ray(&self, raytest: &mut RayCollisionTestClass) -> bool;

    /// Cast AABB for collision detection
    fn cast_aabox(&self, boxtest: &mut AABoxCollisionTestClass) -> bool;

    /// Cast OBB for collision detection
    fn cast_obbox(&self, boxtest: &mut OBBoxCollisionTestClass) -> bool;

    /// Intersect with AABB
    fn intersect_aabox(&self, boxtest: &AABoxIntersectionTestClass) -> bool;

    /// Intersect with OBB
    fn intersect_obbox(&self, boxtest: &OBBoxIntersectionTestClass) -> bool;

    /// Get object-space bounding sphere
    fn get_obj_space_bounding_sphere(&self) -> SphereClass;

    /// Get object-space bounding box
    fn get_obj_space_bounding_box(&self) -> AABoxClass;

    /// Scale the object
    fn scale(&mut self, scale: f32);

    /// Scale the object with separate axes
    fn scale_xyz(&mut self, scalex: f32, scaley: f32, scalez: f32);

    /// Get material info
    fn get_material_info(&self) -> Option<&MaterialInfoClass>;

    /// Mark the object as hidden by animation state
    fn set_animation_hidden(&mut self, hidden: bool) {
        let _ = hidden;
    }

    /// Get sort level for rendering order
    fn get_sort_level(&self) -> i32;

    /// Set sort level for rendering order
    fn set_sort_level(&mut self, level: i32);

    /// Create a decal on this object
    fn create_decal(&mut self, generator: &mut DecalGeneratorClass);

    /// Delete a decal from this object
    fn delete_decal(&mut self, decal_id: u32);

    /// Get the transform of this object
    fn transform(&self) -> &Mat4;

    /// Set the transform of this object
    fn set_transform(&mut self, transform: Mat4);

    /// Clone the render object (alternative to clone_obj)
    fn clone_render_obj(&self) -> Box<dyn RenderObjClass> {
        self.clone_obj()
    }

    /// Get the transform (alternative to transform)
    fn get_transform(&self) -> &Mat4 {
        self.transform()
    }

    /// Get bounding sphere (alternative to get_obj_space_bounding_sphere)
    fn get_bounding_sphere(&self) -> SphereClass {
        self.get_obj_space_bounding_sphere()
    }

    /// Get bounding box (alternative to get_obj_space_bounding_box)
    fn get_bounding_box(&self) -> AABoxClass {
        self.get_obj_space_bounding_box()
    }

    /// Engine reference counting (for resource management)
    fn add_engine_ref(&self) {
        // Default implementation - no-op
    }

    fn release_engine_ref(&self) {
        // Default implementation - no-op
    }

    fn engine_refs(&self) -> usize {
        0 // Default implementation
    }

    /// Notify the object that it has been inserted into a scene.
    fn notify_added(&mut self, _scene: &mut dyn SceneBinding, _object_id: usize) {}

    /// Notify the object that it has been detached from a scene.
    fn notify_removed(&mut self, _scene: &mut dyn SceneBinding, _object_id: usize) {}

    /// Query the object's visibility state for frustum tests and update passes.
    fn is_really_visible(&self) -> bool {
        true
    }

    /// Get the position of this object
    fn position(&self) -> Vec3 {
        let transform = self.transform();
        Vec3::new(transform.w_axis.x, transform.w_axis.y, transform.w_axis.z)
    }

    /// Set the position of this object
    fn set_position(&mut self, position: Vec3) {
        let mut transform = *self.transform();
        transform.w_axis = Vec4::new(position.x, position.y, position.z, 1.0);
        self.set_transform(transform);
    }

    /// Get the rotation of this object
    fn rotation(&self) -> Quat {
        let transform = self.transform();
        Quat::from_mat4(transform)
    }

    /// Set the rotation of this object
    fn set_rotation(&mut self, rotation: Quat) {
        let position = self.position();
        let transform = Mat4::from_rotation_translation(rotation, position);
        self.set_transform(transform);
    }
}

/// Wrapper used to register render objects with the static sort system.
#[derive(Debug, Clone)]
pub struct StaticSortRenderObject {
    inner: Arc<dyn RenderObjClass>,
}

impl StaticSortRenderObject {
    pub fn from_arc<T>(object: Arc<T>) -> Arc<Self>
    where
        T: RenderObjClass + 'static,
    {
        let inner: Arc<dyn RenderObjClass> = object;
        Arc::new(Self { inner })
    }

    pub fn from_dyn(object: Arc<dyn RenderObjClass>) -> Arc<Self> {
        Arc::new(Self { inner: object })
    }

    pub fn render_obj(&self) -> Arc<dyn RenderObjClass> {
        Arc::clone(&self.inner)
    }
}

/// Special render info for custom rendering passes
#[derive(Debug)]
pub struct SpecialRenderInfoClass {
    pub render_type: SpecialRenderType,
    pub context: Option<Box<dyn Any>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialRenderType {
    /// Shadow map rendering
    ShadowMap,
    /// Reflection rendering
    Reflection,
    /// Refraction rendering
    Refraction,
    /// Custom rendering pass
    Custom,
}

/// Render info for normal rendering passes
#[derive(Debug, Clone)]
pub struct RenderInfoClass {
    pub camera: Arc<crate::rendering::camera_system::CameraClass>,
    pub lighting: Option<crate::rendering::lighting_system::LightEnvironmentClass>,
    pub viewport: crate::rendering::camera_system::ViewportClass,
    pub time: f32,
    pub frame_count: u64,
    pub alpha_override: f32,
    pub additional_alpha_multiplier: f32,
    pub material_pass_alpha_override: f32,
    pub material_pass_emissive_override: f32,
    pub additional_material_passes: Vec<crate::material_system::MaterialPassClass>,
    pub override_flags: RenderInfoOverrideFlags,
    override_stack: Vec<RenderInfoOverrideFlags>,
    pub fog: Option<FogSettings>,
}

impl RenderInfoClass {
    pub fn new(camera: Arc<crate::rendering::camera_system::CameraClass>) -> Self {
        Self {
            camera,
            lighting: None,
            viewport: crate::rendering::camera_system::ViewportClass::new(),
            time: 0.0,
            frame_count: 0,
            alpha_override: 1.0,
            additional_alpha_multiplier: 1.0,
            material_pass_alpha_override: 1.0,
            material_pass_emissive_override: 1.0,
            additional_material_passes: Vec::new(),
            override_flags: RenderInfoOverrideFlags::empty(),
            override_stack: Vec::new(),
            fog: None,
        }
    }

    pub fn set_lighting_environment(
        &mut self,
        environment: crate::rendering::lighting_system::LightEnvironmentClass,
    ) {
        self.lighting = Some(environment);
    }

    pub fn set_fog(&mut self, fog: FogSettings) {
        self.fog = Some(fog);
    }

    pub fn clear_fog(&mut self) {
        self.fog = None;
    }

    pub fn fog_settings(&self) -> Option<&FogSettings> {
        self.fog.as_ref()
    }

    pub fn push_material_pass(&mut self, pass: crate::material_system::MaterialPassClass) {
        if self.additional_material_passes.len() < MAX_ADDITIONAL_MATERIAL_PASSES {
            self.additional_material_passes.push(pass);
        }
    }

    pub fn pop_material_pass(&mut self) -> Option<crate::material_system::MaterialPassClass> {
        self.additional_material_passes.pop()
    }

    pub fn additional_pass_count(&self) -> usize {
        self.additional_material_passes.len()
    }

    pub fn peek_additional_pass(
        &self,
        index: usize,
    ) -> Option<&crate::material_system::MaterialPassClass> {
        self.additional_material_passes.get(index)
    }

    pub fn push_override_flags(&mut self, flags: RenderInfoOverrideFlags) {
        if self.override_stack.len() < MAX_OVERRIDE_FLAG_LEVEL {
            self.override_stack.push(self.override_flags);
        }
        self.override_flags = flags;
    }

    pub fn pop_override_flags(&mut self) {
        if let Some(previous) = self.override_stack.pop() {
            self.override_flags = previous;
        } else {
            self.override_flags = RenderInfoOverrideFlags::empty();
        }
    }

    pub fn current_override_flags(&self) -> RenderInfoOverrideFlags {
        self.override_flags
    }
}

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct RenderInfoOverrideFlags: u32 {
        const FORCE_TWO_SIDED         = 0x0001;
        const FORCE_SORTING           = 0x0002;
        const ADDITIONAL_PASSES_ONLY  = 0x0004;
        const SHADOW_RENDERING        = 0x0008;
        const DECAL_RENDERING         = 0x0010;
    }
}

/// Collision test classes
pub use ww3d_collision::bounding_volumes::aabox::AABoxClass;
pub use ww3d_collision::bounding_volumes::obbox::OBBoxClass;
pub use ww3d_collision::bounding_volumes::sphere::SphereClass;
// Re-export RenderObjClassId (already imported at top of file)

#[derive(Debug, Clone)]
pub struct RayCollisionResult {
    pub start_bad: bool,
    pub fraction: f32,
    pub normal: Vec3,
    pub surface_type: u32,
    pub contact_point: Vec3,
    pub compute_contact_point: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct FogSettings {
    pub enabled: bool,
    pub color: glam::Vec3,
    pub start: f32,
    pub end: f32,
}

impl Default for RayCollisionResult {
    fn default() -> Self {
        Self {
            start_bad: false,
            fraction: 1.0,
            normal: Vec3::ZERO,
            surface_type: 0,
            contact_point: Vec3::ZERO,
            compute_contact_point: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RayCollisionTestClass {
    pub line: LineSegment,
    pub collision_type: u32,
    pub check_translucent: bool,
    pub check_hidden: bool,
    pub collided_render_obj: Option<usize>,
    pub result: RayCollisionResult,
}

impl RayCollisionTestClass {
    pub fn new(line: LineSegment, collision_type: u32) -> Self {
        Self {
            line,
            collision_type,
            check_translucent: false,
            check_hidden: false,
            collided_render_obj: None,
            result: RayCollisionResult::default(),
        }
    }

    pub fn with_flags(
        line: LineSegment,
        collision_type: u32,
        check_translucent: bool,
        check_hidden: bool,
    ) -> Self {
        Self {
            line,
            collision_type,
            check_translucent,
            check_hidden,
            collided_render_obj: None,
            result: RayCollisionResult::default(),
        }
    }

    pub fn transformed_by_matrix(&self, matrix: Mat4) -> Self {
        let start = matrix.transform_point3(self.line.start);
        let end = matrix.transform_point3(self.line.end);
        let mut transformed = self.clone();
        transformed.line = LineSegment::new(start, end);
        transformed
    }

    pub fn origin(&self) -> Vec3 {
        self.line.start
    }

    pub fn direction(&self) -> Vec3 {
        self.line.direction().normalize_or_zero()
    }

    pub fn length(&self) -> f32 {
        self.line.length()
    }
}

#[derive(Debug, Clone, Default)]
pub struct AABoxCollisionResult {
    pub intersection: bool,
    pub contact_points: Vec<Vec3>,
}

#[derive(Debug, Clone)]
pub struct AABoxCollisionTestClass {
    pub box_obj: AABoxClass,
    pub move_vector: Vec3,
    pub collision_type: u32,
    pub check_translucent: bool,
    pub collided_render_obj: Option<usize>,
    pub sweep_min: Vec3,
    pub sweep_max: Vec3,
    pub result: Option<AABoxCollisionResult>,
}

impl AABoxCollisionTestClass {
    pub fn new(box_obj: AABoxClass, move_vector: Vec3, collision_type: u32) -> Self {
        let mut instance = Self {
            box_obj,
            move_vector,
            collision_type,
            check_translucent: false,
            collided_render_obj: None,
            sweep_min: Vec3::ZERO,
            sweep_max: Vec3::ZERO,
            result: None,
        };
        instance.update_sweep_bounds();
        instance
    }

    pub fn with_flags(
        box_obj: AABoxClass,
        move_vector: Vec3,
        collision_type: u32,
        check_translucent: bool,
    ) -> Self {
        let mut instance = Self {
            box_obj,
            move_vector,
            collision_type,
            check_translucent,
            collided_render_obj: None,
            sweep_min: Vec3::ZERO,
            sweep_max: Vec3::ZERO,
            result: None,
        };
        instance.update_sweep_bounds();
        instance
    }

    pub fn transformed_by_matrix(&self, matrix: Mat4) -> Self {
        let corners = aabox_corners(&self.box_obj);
        let mut new_min = Vec3::splat(f32::INFINITY);
        let mut new_max = Vec3::splat(f32::NEG_INFINITY);
        for corner in &corners {
            let transformed_corner = matrix.transform_point3(*corner);
            new_min = new_min.min(transformed_corner);
            new_max = new_max.max(transformed_corner);
        }

        let transformed_box = AABoxClass::from_min_max(new_min, new_max);
        let transformed_move = matrix.transform_vector3(self.move_vector);

        let mut result = Self {
            box_obj: transformed_box,
            move_vector: transformed_move,
            collision_type: self.collision_type,
            check_translucent: self.check_translucent,
            collided_render_obj: self.collided_render_obj,
            sweep_min: Vec3::ZERO,
            sweep_max: Vec3::ZERO,
            result: self.result.clone(),
        };
        result.update_sweep_bounds();
        result
    }

    pub fn update_sweep_bounds(&mut self) {
        let start_min = self.box_obj.min();
        let start_max = self.box_obj.max();

        let end_center = self.box_obj.center + self.move_vector;
        let end_box = AABoxClass::from_center_and_extent(end_center, self.box_obj.extent);
        let end_min = end_box.min();
        let end_max = end_box.max();

        self.sweep_min = start_min.min(end_min);
        self.sweep_max = start_max.max(end_max);
    }
}

#[derive(Debug, Clone, Default)]
pub struct OBBoxCollisionResult {
    pub intersection: bool,
    pub contact_points: Vec<Vec3>,
}

#[derive(Debug, Clone)]
pub struct OBBoxCollisionTestClass {
    pub box_obj: OBBoxClass,
    pub move_vector: Vec3,
    pub collision_type: u32,
    pub collided_render_obj: Option<usize>,
    pub result: Option<OBBoxCollisionResult>,
}

impl OBBoxCollisionTestClass {
    pub fn new(box_obj: OBBoxClass, move_vector: Vec3, collision_type: u32) -> Self {
        Self {
            box_obj,
            move_vector,
            collision_type,
            collided_render_obj: None,
            result: None,
        }
    }

    pub fn transformed_by_matrix(&self, matrix: Mat4) -> Self {
        let center = matrix.transform_point3(self.box_obj.center);
        let rotation = Mat3::from_mat4(matrix);
        let mut basis = self.box_obj.basis;
        basis[0] = rotation.mul_vec3(basis[0]);
        basis[1] = rotation.mul_vec3(basis[1]);
        basis[2] = rotation.mul_vec3(basis[2]);

        Self {
            box_obj: OBBoxClass::new(center, self.box_obj.extent, basis),
            move_vector: matrix.transform_vector3(self.move_vector),
            collision_type: self.collision_type,
            collided_render_obj: self.collided_render_obj,
            result: self.result.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AABoxIntersectionTestClass {
    pub box_obj: AABoxClass,
    pub collision_type: u32,
}

impl AABoxIntersectionTestClass {
    pub fn new(box_obj: AABoxClass, collision_type: u32) -> Self {
        Self {
            box_obj,
            collision_type,
        }
    }

    pub fn transformed_by_matrix(&self, matrix: Mat4) -> Self {
        let corners = aabox_corners(&self.box_obj);
        let mut new_min = Vec3::splat(f32::INFINITY);
        let mut new_max = Vec3::splat(f32::NEG_INFINITY);
        for corner in &corners {
            let transformed_corner = matrix.transform_point3(*corner);
            new_min = new_min.min(transformed_corner);
            new_max = new_max.max(transformed_corner);
        }

        Self {
            box_obj: AABoxClass::from_min_max(new_min, new_max),
            collision_type: self.collision_type,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OBBoxIntersectionTestClass {
    pub box_obj: OBBoxClass,
    pub collision_type: u32,
}

impl OBBoxIntersectionTestClass {
    pub fn new(box_obj: OBBoxClass, collision_type: u32) -> Self {
        Self {
            box_obj,
            collision_type,
        }
    }

    pub fn transformed_by_matrix(&self, matrix: Mat4) -> Self {
        let center = matrix.transform_point3(self.box_obj.center);
        let rotation = Mat3::from_mat4(matrix);
        let mut basis = self.box_obj.basis;
        basis[0] = rotation.mul_vec3(basis[0]);
        basis[1] = rotation.mul_vec3(basis[1]);
        basis[2] = rotation.mul_vec3(basis[2]);

        Self {
            box_obj: OBBoxClass::new(center, self.box_obj.extent, basis),
            collision_type: self.collision_type,
        }
    }
}

fn aabox_corners(box_obj: &AABoxClass) -> [Vec3; 8] {
    let min = box_obj.center - box_obj.extent;
    let max = box_obj.center + box_obj.extent;

    [
        Vec3::new(min.x, min.y, min.z),
        Vec3::new(max.x, min.y, min.z),
        Vec3::new(min.x, max.y, min.z),
        Vec3::new(max.x, max.y, min.z),
        Vec3::new(min.x, min.y, max.z),
        Vec3::new(max.x, min.y, max.z),
        Vec3::new(min.x, max.y, max.z),
        Vec3::new(max.x, max.y, max.z),
    ]
}

#[allow(dead_code)] // C++ parity
fn obbox_corners(box_obj: &OBBoxClass) -> [Vec3; 8] {
    box_obj.get_corners()
}

/// Material info class
#[derive(Debug, Clone)]
pub struct MaterialInfoClass {
    pub vertex_materials: Vec<crate::material_system::VertexMaterialClass>,
    pub textures: Vec<Arc<crate::texture_system::TextureClass>>,
    pub passes: Vec<MaterialPassClass>,
}

impl Default for MaterialInfoClass {
    fn default() -> Self {
        Self {
            vertex_materials: Vec::new(),
            textures: Vec::new(),
            passes: Vec::new(),
        }
    }
}

/// Material pass class
// Use the unified MaterialPassClass from material_system
pub type MaterialPassClass = crate::material_system::MaterialPassClass;

/// Decal generator class
static NEXT_LOGICAL_DECAL_ID: AtomicU32 = AtomicU32::new(1);

/// Decal generator that mirrors the responsibilities of the original C++ `DecalGeneratorClass`.
///
/// The generator encapsulates the projector transform, projection volume and material state
/// required to bake decal geometry against meshes. It exposes safe Rust builders while keeping
/// parity with the mutable configuration style of the legacy engine.
#[derive(Debug)]
pub struct DecalGeneratorClass {
    material_pass: Arc<MaterialPassClass>,
    lifetime: f32,
    apply_to_translucent_meshes: bool,
    backface_threshold: f32,
    surface_bias: f32,
    projector_world: Mat4,
    world_to_projector: Mat4,
    mesh_transform: Mat4,
    mesh_to_projector: Mat4,
    half_extents: Vec3,
    logical_id: u32,
    mesh_handles: Mutex<Vec<usize>>,
}

impl DecalGeneratorClass {
    /// Create a generator with an orthographic projector whose extents are defined by `size`.
    /// `size` represents the full width/height/depth of the volume in world units.
    pub fn new(material_pass: MaterialPassClass, size: Vec3, lifetime: f32) -> Self {
        Self::with_material(Arc::new(material_pass), size, lifetime)
    }

    /// Builder that accepts a shared material pass reference up-front. This mirrors the
    /// ref-counted material ownership of the C++ implementation and enables multiple decals to
    /// reuse identical pipeline state without cloning heavy texture metadata.
    pub fn with_material(material_pass: Arc<MaterialPassClass>, size: Vec3, lifetime: f32) -> Self {
        let half_extents = Vec3::new(
            (size.x * 0.5).max(0.001),
            (size.y * 0.5).max(0.001),
            (size.z * 0.5).max(0.001),
        );

        let projector_world = Mat4::IDENTITY;
        let world_to_projector = projector_world.inverse();
        let logical_id = NEXT_LOGICAL_DECAL_ID.fetch_add(1, Ordering::Relaxed);

        Self {
            material_pass,
            lifetime,
            apply_to_translucent_meshes: false,
            backface_threshold: 0.0,
            surface_bias: 0.002,
            projector_world,
            world_to_projector,
            mesh_transform: Mat4::IDENTITY,
            mesh_to_projector: world_to_projector,
            half_extents,
            logical_id,
            mesh_handles: Mutex::new(Vec::new()),
        }
    }

    /// Override the projector transform (world space). Accepts any affine Mat4; non-uniform
    /// scaling is supported and reflected in the returned bounding volume and UV mapping.
    pub fn with_transform(mut self, transform: Mat4) -> Self {
        self.set_transform(transform);
        self
    }

    /// Enable or disable translucent meshes for this generator, matching the legacy
    /// `Apply_To_Translucent_Meshes` behaviour.
    pub fn with_translucent_support(mut self, allow: bool) -> Self {
        self.apply_to_translucent_meshes = allow;
        self
    }

    /// Adjust the backface threshold used to cull polygons whose normals diverge from the
    /// projector direction. Values range [-1, 1] with 0 matching the C++ default.
    pub fn with_backface_threshold(mut self, threshold: f32) -> Self {
        self.backface_threshold = threshold.clamp(-1.0, 1.0);
        self
    }

    /// Configure how far decal vertices should be nudged along the projector direction to avoid
    /// z-fighting. The value is specified in world units and defaults to 2mm.
    pub fn with_surface_bias(mut self, bias: f32) -> Self {
        self.surface_bias = bias.max(0.0);
        self
    }

    /// Manually set the projector transform after construction.
    pub fn set_transform(&mut self, transform: Mat4) {
        self.projector_world = transform;
        self.world_to_projector = transform.inverse();
        self.mesh_to_projector = self.world_to_projector * self.mesh_transform;
    }

    /// Read the current projector transform.
    pub fn transform(&self) -> Mat4 {
        self.projector_world
    }

    pub fn set_mesh_transform(&mut self, mesh_transform: Mat4) {
        self.mesh_transform = mesh_transform;
        self.mesh_to_projector = self.world_to_projector * self.mesh_transform;
    }

    pub fn compute_mesh_texture_coordinate(&self, mesh_position: Vec3) -> Vec3 {
        let local = self.mesh_to_projector.transform_point3(mesh_position);
        let u = 0.5 + (local.x / (self.half_extents.x * 2.0));
        let v = 0.5 - (local.y / (self.half_extents.y * 2.0));
        let q = 0.5 + (local.z / (self.half_extents.z * 2.0));
        Vec3::new(u, v, q)
    }

    /// Obtain the logical decal id associated with this generator, mirroring
    /// `DecalGeneratorClass::Get_Decal_ID`.
    pub fn get_decal_id(&self) -> u32 {
        self.logical_id
    }

    pub fn add_mesh_handle(&self, mesh_ptr: *const MeshClass) {
        if mesh_ptr.is_null() {
            return;
        }
        if let Ok(mut handles) = self.mesh_handles.lock() {
            let address = mesh_ptr as usize;
            if !handles.iter().any(|&existing| existing == address) {
                handles.push(address);
            }
        }
    }

    pub fn registered_mesh_handles(&self) -> Vec<usize> {
        self.mesh_handles
            .lock()
            .map(|handles| handles.clone())
            .unwrap_or_default()
    }

    pub fn get_lifetime(&self) -> f32 {
        self.lifetime
    }

    pub fn material_pass(&self) -> Arc<MaterialPassClass> {
        Arc::clone(&self.material_pass)
    }

    pub fn allow_translucent_meshes(&self) -> bool {
        self.apply_to_translucent_meshes
    }

    pub fn surface_bias(&self) -> f32 {
        self.surface_bias
    }

    pub fn backface_threshold(&self) -> f32 {
        self.backface_threshold
    }

    /// Direction the projector is firing in world space (its +Z axis).
    pub fn projector_direction(&self) -> Vec3 {
        let dir = self.projector_world.z_axis.truncate();
        if dir.length_squared() > 0.0 {
            dir.normalize()
        } else {
            Vec3::Z
        }
    }

    /// World-space oriented bounding box used for coarse triangle rejection.
    pub fn get_bounding_volume(&self) -> OBBoxClass {
        let local_box = OBBoxClass::from_center_extent(Vec3::ZERO, self.half_extents);
        local_box.transformed(self.projector_world)
    }

    /// Compute UVW coordinates for a world-space position using the current projector settings.
    /// Returns (u, v, q) where q is a depth factor in [0, 1].
    pub fn compute_texture_coordinate(&self, world_position: Vec3) -> Vec3 {
        let local = self.world_to_projector.transform_point3(world_position);

        let u = 0.5 + (local.x / (self.half_extents.x * 2.0));
        let v = 0.5 - (local.y / (self.half_extents.y * 2.0));
        let q = 0.5 + (local.z / (self.half_extents.z * 2.0));

        Vec3::new(u, v, q)
    }
}

impl Drop for DecalGeneratorClass {
    fn drop(&mut self) {
        if let Ok(handles) = self.mesh_handles.lock() {
            for &address in handles.iter() {
                // Validate pointer before dereferencing
                // Address 0 is reserved as null, small addresses are likely invalid
                if address == 0 || address < std::mem::size_of::<MeshClass>() {
                    continue;
                }

                let mesh_ptr = address as *mut MeshClass;

                // Additional safety: only dereference if pointer is properly aligned
                if mesh_ptr as usize % std::mem::align_of::<MeshClass>() != 0 {
                    continue;
                }

                unsafe {
                    // This is unavoidable for C++ interop, but validated above
                    if let Some(mesh) = mesh_ptr.as_mut() {
                        mesh.delete_decal(self.logical_id);
                    }
                }
            }
        }
    }
}
