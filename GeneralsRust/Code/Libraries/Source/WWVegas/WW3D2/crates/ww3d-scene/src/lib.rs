// WW3D Scene Management Library
// Comprehensive scene management with full feature parity to C++ WW3D2
//
// This crate provides:
// - Hierarchical bone animation (HTree)
// - Level-of-detail management (LOD systems)
// - Advanced lighting with importance sorting
// - Multi-layer rendering
// - Visibility culling and bounding volumes
// - Scene graph management

use glam::{Mat4, Vec3};
use std::collections::HashMap;

// Module declarations
pub mod culling;
pub mod fog;
pub mod htree;
pub mod layer;
pub mod light;
pub mod lod;
pub mod multi_threading;
pub mod npatch;
pub mod npatch_pipeline;
pub mod physics_integration;
pub mod rendobj;
pub mod shader_validator;

// Package 7 - RenderObject Methods & Scene Integration
pub mod integration_tests;
pub mod mesh_model_impl;
pub mod render_object_ext;
pub mod scene_ext;

// Re-exports for convenience
pub use culling::{AABTree, AABTreeNode, AABox, Frustum, Plane, PlaneSide, Ray, Sphere};
pub use fog::FogSettings;
pub use htree::{Animation, AnimationCombo, HTree, HTreeManager, Pivot};
pub use layer::{Layer, LayerManager};
pub use light::{get_lighting_lod_cutoff, set_lighting_lod_cutoff};
pub use light::{Light, LightEnvironment, LightFlags, LightType, MAX_LIGHTS};
pub use lod::{
    DistLod, DistLodNode, HLod, LodLevel, ModelNode, Proxy, SnapPoint, NO_MAX_SCREEN_SIZE,
};
pub use npatch::{
    NPatchConfig, NPatchTessellator, NPatchVertex, SubdividedMesh, TessellationLevel,
};
pub use npatch_pipeline::{NPatchPipeline, NPatchShaderIntegration, PipelineStats, QualityLevel};
pub use shader_validator::{
    DetailColorFunc, DstBlendFunc, FogFunc, GpuCapabilities, GradientFunc, ShaderFallback,
    ShaderValidator, SrcBlendFunc, ValidationMessage, ValidationResult, ValidationSeverity,
};

// Package 7 re-exports
pub use mesh_model_impl::{BlendMode, MaterialPass, MeshGeometry, MeshModel, SkinData};
pub use render_object_ext::{
    AnimationMode, AnimationState, BoneAttachment, Material, PickRay, PickResult, RenderContext,
    RenderObjClassExt, TextureId,
};
pub use scene_ext::SceneExt;

/// Registration types for special processing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(non_camel_case_types)]
pub enum RegistrationType {
    ON_FRAME_UPDATE,
    RELEASE_ON_LEVEL_LOAD,
    LIGHT_PROCESSOR,
}

/// Scene iterator trait
pub trait SceneIterator {
    fn first(&mut self);
    fn next(&mut self);
    fn is_done(&self) -> bool;
    fn current_item(&self) -> Option<&dyn RenderObj>;
}

/// Render object trait - equivalent to C++ RenderObjClass
///
/// This is the base trait for all renderable objects in the scene.
/// It provides methods for updating, rendering, collision detection,
/// and hierarchical management.
pub trait RenderObj: std::fmt::Debug + Send + Sync {
    /// Update the object's state
    fn update(&mut self, dt: f32);

    /// Check if the object is visible from a camera position
    fn is_visible(&self, camera_pos: Vec3) -> bool;

    /// Get the object's name
    fn get_name(&self) -> &str;

    /// Set the object's transform
    fn set_transform(&mut self, transform: Mat4);

    /// Get the object's transform
    fn get_transform(&self) -> &Mat4;

    /// Render the object
    fn render(&self, render_info: &RenderInfoClass);

    /// Special render for different modes (shadows, reflections, etc.)
    fn special_render(&self, _render_info: &RenderInfoClass) {}

    /// Get the number of polygons in this render object
    fn get_num_polys(&self) -> usize {
        0
    }

    // === Collision Detection ===

    /// Cast ray for collision detection
    fn cast_ray(&self, _ray: &Ray) -> Option<f32> {
        None
    }

    /// Test AABB intersection
    fn intersect_aabb(&self, _aabb: &AABox) -> bool {
        false
    }

    // === Bounding Volumes ===

    /// Get the world-space bounding sphere
    fn get_bounding_sphere(&self) -> Sphere {
        Sphere::new(Vec3::ZERO, 1.0)
    }

    /// Get the world-space bounding box
    fn get_bounding_box(&self) -> AABox {
        AABox::new(Vec3::splat(-1.0), Vec3::splat(1.0))
    }

    /// Get the object-space bounding sphere
    fn get_obj_space_bounding_sphere(&self) -> Sphere {
        Sphere::new(Vec3::ZERO, 1.0)
    }

    /// Get the object-space bounding box
    fn get_obj_space_bounding_box(&self) -> AABox {
        AABox::new(Vec3::splat(-1.0), Vec3::splat(1.0))
    }

    // === Hierarchical Management ===

    /// Get the number of sub-objects
    fn get_num_sub_objects(&self) -> usize {
        0
    }

    /// Get a sub-object by index
    fn get_sub_object(&self, _index: usize) -> Option<&dyn RenderObj> {
        None
    }

    /// Notify that this object was added to a scene
    fn notify_added(&mut self, _scene: &mut SceneClass) {}

    /// Notify that this object was removed from a scene
    fn notify_removed(&mut self, _scene: &mut SceneClass) {}

    // === LOD Management ===

    /// Prepare LOD selection based on camera
    fn prepare_lod(&mut self, _camera: &CameraClass) {}

    /// Get the LOD level
    fn get_lod_level(&self) -> usize {
        0
    }

    /// Set the LOD level
    fn set_lod_level(&mut self, _lod: usize) {}

    // === Visibility and Culling ===

    /// Check if object is hidden
    fn is_hidden(&self) -> bool {
        false
    }

    /// Set hidden state
    fn set_hidden(&mut self, _hidden: bool) {}

    /// Test against view frustum
    fn is_in_frustum(&self, _frustum: &Frustum) -> bool {
        true
    }
}

/// Render info class - equivalent to C++ RenderInfoClass
///
/// Contains all information needed to render an object, including camera,
/// lighting, and material overrides.
#[derive(Debug, Clone)]
pub struct RenderInfoClass {
    pub camera: CameraClass,
    pub light_environment: Option<LightEnvironment>,
    pub alpha_override: f32,
    pub material_pass_alpha_override: f32,
    pub material_pass_emissive_override: f32,
    pub frame_time: f32,
}

impl RenderInfoClass {
    pub fn new(camera: CameraClass) -> Self {
        Self {
            camera,
            light_environment: None,
            alpha_override: 1.0,
            material_pass_alpha_override: 1.0,
            material_pass_emissive_override: 1.0,
            frame_time: 0.0,
        }
    }

    pub fn with_light_environment(mut self, light_env: LightEnvironment) -> Self {
        self.light_environment = Some(light_env);
        self
    }
}

/// Camera class - manages view and projection
#[derive(Debug, Clone)]
pub struct CameraClass {
    pub transform: Mat4,
    pub position: Vec3,
    pub view_matrix: Mat4,
    pub projection_matrix: Mat4,
    pub view_projection_matrix: Mat4,
    pub frustum: Option<Frustum>,
}

impl CameraClass {
    pub fn new() -> Self {
        Self {
            transform: Mat4::IDENTITY,
            position: Vec3::ZERO,
            view_matrix: Mat4::IDENTITY,
            projection_matrix: Mat4::IDENTITY,
            view_projection_matrix: Mat4::IDENTITY,
            frustum: None,
        }
    }

    pub fn get_position(&self) -> Vec3 {
        self.position
    }

    pub fn set_position(&mut self, position: Vec3) {
        self.position = position;
        self.update_transform();
    }

    pub fn set_view_matrix(&mut self, view: Mat4) {
        self.view_matrix = view;
        self.update_view_projection();
    }

    pub fn set_projection_matrix(&mut self, projection: Mat4) {
        self.projection_matrix = projection;
        self.update_view_projection();
    }

    /// Get view matrix (convenience accessor)
    pub fn view_matrix(&self) -> Mat4 {
        self.view_matrix
    }

    /// Get projection matrix (convenience accessor)
    pub fn projection_matrix(&self) -> Mat4 {
        self.projection_matrix
    }

    /// Get view-projection matrix (convenience accessor)
    pub fn view_projection_matrix(&self) -> Mat4 {
        self.view_projection_matrix
    }

    /// Get camera position (convenience alias)
    pub fn position(&self) -> Vec3 {
        self.position
    }

    fn update_transform(&mut self) {
        self.transform = Mat4::from_translation(self.position);
    }

    fn update_view_projection(&mut self) {
        self.view_projection_matrix = self.projection_matrix * self.view_matrix;
        self.frustum = Some(Frustum::from_matrix(&self.view_projection_matrix));
    }

    /// Cull a sphere against the frustum
    pub fn cull_sphere(&self, sphere: &Sphere) -> bool {
        if let Some(frustum) = &self.frustum {
            !frustum.test_sphere(sphere)
        } else {
            false
        }
    }

    /// Cull a box against the frustum
    pub fn cull_box(&self, box_bounds: &AABox) -> bool {
        if let Some(frustum) = &self.frustum {
            !frustum.test_box(box_bounds)
        } else {
            false
        }
    }

    /// Create a perspective camera
    pub fn perspective(_name: String, aspect: f32, fov: f32, near: f32, far: f32) -> Self {
        let mut camera = Self::new();
        let fov_rad = fov.to_radians();
        camera.projection_matrix = Mat4::perspective_rh(fov_rad, aspect, near, far);
        camera.update_view_projection();
        camera
    }
}

impl Default for CameraClass {
    fn default() -> Self {
        Self::new()
    }
}

/// Scene class - equivalent to C++ SceneClass
///
/// Manages a collection of render objects with support for:
/// - Object lifecycle management
/// - Registration for special processing
/// - Visibility culling
/// - Lighting environment
pub struct SceneClass {
    pub objects: Vec<Box<dyn RenderObj>>,
    pub registered_objects: HashMap<RegistrationType, Vec<Box<dyn RenderObj>>>,
    pub lights: Vec<Light>,
    pub light_environment: Option<LightEnvironment>,
    pub ambient_light: Vec3,
    pub fog_enabled: bool,
    pub fog_color: Vec3,
    pub fog_start: f32,
    pub fog_end: f32,
    pub visibility_checked: bool,
}

impl SceneClass {
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
            registered_objects: HashMap::new(),
            lights: Vec::new(),
            light_environment: None,
            ambient_light: Vec3::new(0.2, 0.2, 0.2),
            fog_enabled: false,
            fog_color: Vec3::new(0.5, 0.5, 0.5),
            fog_start: 0.0,
            fog_end: 1000.0,
            visibility_checked: false,
        }
    }

    /// Add a render object to the scene
    pub fn add_render_object(&mut self, mut obj: Box<dyn RenderObj>) {
        obj.notify_added(self);
        self.objects.push(obj);
    }

    /// Remove a render object from the scene
    pub fn remove_render_object(&mut self, name: &str) -> bool {
        let initial_len = self.objects.len();
        self.objects.retain(|obj| obj.get_name() != name);
        self.objects.len() < initial_len
    }

    /// Register an object for special processing
    pub fn register(&mut self, obj: Box<dyn RenderObj>, reg_type: RegistrationType) {
        self.registered_objects
            .entry(reg_type)
            .or_insert_with(Vec::new)
            .push(obj);
    }

    /// Unregister an object
    pub fn unregister(&mut self, name: &str, reg_type: RegistrationType) -> bool {
        if let Some(objects) = self.registered_objects.get_mut(&reg_type) {
            let initial_len = objects.len();
            objects.retain(|obj| obj.get_name() != name);
            return objects.len() < initial_len;
        }
        false
    }

    /// Add a light to the scene
    pub fn add_light(&mut self, light: Light) {
        self.lights.push(light);
    }

    /// Remove a light by name
    pub fn remove_light(&mut self, name: &str) -> bool {
        let initial_len = self.lights.len();
        self.lights.retain(|l| l.name != name);
        self.lights.len() < initial_len
    }

    /// Perform visibility checking against camera frustum
    pub fn visibility_check(&mut self, camera: &CameraClass) {
        if let Some(frustum) = &camera.frustum {
            for obj in &self.objects {
                let sphere = obj.get_bounding_sphere();
                // Mark as visible/invisible based on frustum test
                // Note: This would require extending RenderObj with a visibility flag
                // For now, the is_in_frustum method handles this
                let _ = frustum.test_sphere(&sphere);
            }
            self.visibility_checked = true;
        }
    }

    /// Update all objects in the scene
    pub fn update(&mut self, dt: f32) {
        // Update regular objects
        for obj in &mut self.objects {
            obj.update(dt);
        }

        // Update registered objects for frame updates
        if let Some(frame_objects) = self
            .registered_objects
            .get_mut(&RegistrationType::ON_FRAME_UPDATE)
        {
            for obj in frame_objects {
                obj.update(dt);
            }
        }
    }

    /// Render the scene with multi-pass support
    /// C++ Reference: scene.cpp lines 213-249 (SceneClass::Render with EXTRA_PASS support)
    pub fn render(&self, camera: &CameraClass) {
        // Create light environment for this render
        let mut light_env = LightEnvironment::new();
        light_env.reset(Vec3::ZERO, self.ambient_light);

        // Add scene lights
        for light in &self.lights {
            light_env.add_light(light);
        }

        // Pre-render update (transform to camera space)
        light_env.pre_render_update(&camera.view_matrix);

        // Create render info
        let render_info = RenderInfoClass::new(camera.clone()).with_light_environment(light_env);

        // Render all visible objects
        for obj in &self.objects {
            // Check visibility
            if !obj.is_visible(camera.get_position()) {
                continue;
            }

            // Frustum culling
            if let Some(frustum) = &camera.frustum {
                let sphere = obj.get_bounding_sphere();
                if !frustum.test_sphere(&sphere) {
                    continue; // Culled
                }
            }

            // Render the object (single-pass by default)
            obj.render(&render_info);
        }
    }

    /// Render the scene with explicit multi-pass filtering
    /// This method implements the C++ multi-pass rendering loop
    /// C++ Reference: dx8renderer.cpp lines 1225-1240 (for loop over passes)
    pub fn render_multi_pass(&self, camera: &CameraClass, max_passes: usize) {
        // Create light environment for this render
        let mut light_env = LightEnvironment::new();
        light_env.reset(Vec3::ZERO, self.ambient_light);

        // Add scene lights
        for light in &self.lights {
            light_env.add_light(light);
        }

        // Pre-render update (transform to camera space)
        light_env.pre_render_update(&camera.view_matrix);

        // Create render info
        let render_info = RenderInfoClass::new(camera.clone()).with_light_environment(light_env);

        // Iterate through all rendering passes
        // C++ Reference: dx8renderer.cpp line 1225 (for (unsigned pass=0;pass<split_table.Get_Pass_Count();++pass))
        for _pass_index in 0..max_passes {
            // Render all visible objects for this pass
            for obj in &self.objects {
                // Check visibility
                if !obj.is_visible(camera.get_position()) {
                    continue;
                }

                // Frustum culling
                if let Some(frustum) = &camera.frustum {
                    let sphere = obj.get_bounding_sphere();
                    if !frustum.test_sphere(&sphere) {
                        continue; // Culled
                    }
                }

                // Render the object for this specific pass
                // The object's render implementation should filter polygons by pass_index
                obj.special_render(&render_info);
            }
        }
    }

    /// Create an iterator for the scene
    pub fn create_iterator(&self) -> Box<dyn SceneIterator + '_> {
        Box::new(SceneIteratorImpl {
            objects: &self.objects,
            current_index: 0,
        })
    }

    /// Get the number of render objects
    pub fn get_num_render_objects(&self) -> usize {
        self.objects.len()
    }

    /// Remove all render objects
    pub fn remove_all_render_objects(&mut self) {
        self.objects.clear();
        self.registered_objects.clear();
    }

    /// Set ambient light
    pub fn set_ambient_light(&mut self, ambient: Vec3) {
        self.ambient_light = ambient;
    }

    /// Get ambient light
    pub fn get_ambient_light(&self) -> Vec3 {
        self.ambient_light
    }

    /// Set fog parameters
    pub fn set_fog(&mut self, enabled: bool, color: Vec3, start: f32, end: f32) {
        self.fog_enabled = enabled;
        self.fog_color = color;
        self.fog_start = start;
        self.fog_end = end;
    }

    /// Get fog parameters
    pub fn get_fog(&self) -> (bool, Vec3, f32, f32) {
        (
            self.fog_enabled,
            self.fog_color,
            self.fog_start,
            self.fog_end,
        )
    }
}

/// Container system for composite objects
/// C++ Reference: container.h/container.cpp ContainerClass hierarchy
///
/// Containers allow grouping multiple render objects together as a single unit.
/// This supports complex game objects made up of multiple sub-meshes and effects.
#[derive(Debug, Clone)]
pub struct ContainerInfo {
    /// Container name (from W3DMeshHeader container_name)
    pub name: String,
    /// Child object names/IDs
    pub children: Vec<String>,
    /// Container-level transform
    pub transform: Mat4,
    /// Whether this container is hidden
    pub hidden: bool,
}

impl ContainerInfo {
    /// Create a new container info
    /// C++ Reference: container.cpp ContainerClass::ContainerClass()
    pub fn new(name: String) -> Self {
        Self {
            name,
            children: Vec::new(),
            transform: Mat4::IDENTITY,
            hidden: false,
        }
    }

    /// Add a child object to this container
    pub fn add_child(&mut self, child_name: String) {
        if !self.children.contains(&child_name) {
            self.children.push(child_name);
        }
    }

    /// Remove a child object from this container
    pub fn remove_child(&mut self, child_name: &str) {
        self.children.retain(|n| n != child_name);
    }

    /// Get number of child objects
    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    /// Check if this container contains a specific child
    pub fn contains_child(&self, child_name: &str) -> bool {
        self.children.iter().any(|n| n == child_name)
    }
}

/// Container manager for maintaining hierarchy of composite objects
/// C++ Reference: container.h ContainerClass::Registry
#[derive(Debug, Default)]
pub struct ContainerManager {
    /// Map of container names to container info
    containers: HashMap<String, ContainerInfo>,
    /// Parent container for each child object (reverse lookup)
    child_to_parent: HashMap<String, String>,
}

impl ContainerManager {
    /// Create a new container manager
    pub fn new() -> Self {
        Self {
            containers: HashMap::new(),
            child_to_parent: HashMap::new(),
        }
    }

    /// Register a container
    pub fn register_container(&mut self, container: ContainerInfo) {
        self.containers.insert(container.name.clone(), container);
    }

    /// Register a child relationship
    pub fn register_child(&mut self, parent: String, child: String) {
        if let Some(container) = self.containers.get_mut(&parent) {
            container.add_child(child.clone());
        }
        self.child_to_parent.insert(child, parent);
    }

    /// Get a container by name
    pub fn get_container(&self, name: &str) -> Option<&ContainerInfo> {
        self.containers.get(name)
    }

    /// Get a mutable container by name
    pub fn get_container_mut(&mut self, name: &str) -> Option<&mut ContainerInfo> {
        self.containers.get_mut(name)
    }

    /// Get the parent container of a child object
    pub fn get_parent(&self, child: &str) -> Option<&str> {
        self.child_to_parent.get(child).map(|s| s.as_str())
    }

    /// Get all containers
    pub fn containers(&self) -> &HashMap<String, ContainerInfo> {
        &self.containers
    }

    /// Get number of registered containers
    pub fn container_count(&self) -> usize {
        self.containers.len()
    }

    /// Register a mesh object as a container child
    /// This is called during asset loading when a mesh has a container_name
    pub fn register_mesh_container(&mut self, mesh_name: String, container_name: String) {
        // If container doesn't exist, create it
        if !self.containers.contains_key(&container_name) {
            self.containers.insert(
                container_name.clone(),
                ContainerInfo::new(container_name.clone()),
            );
        }

        // Register the mesh as a child of the container
        self.register_child(container_name, mesh_name);
    }

    /// Get all children of a container
    pub fn get_container_children(&self, container: &str) -> Option<&Vec<String>> {
        self.containers.get(container).map(|c| &c.children)
    }

    /// Update container transform (affects all children during rendering)
    pub fn set_container_transform(&mut self, container: &str, transform: Mat4) {
        if let Some(c) = self.containers.get_mut(container) {
            c.transform = transform;
        }
    }

    /// Get container transform
    pub fn get_container_transform(&self, container: &str) -> Option<Mat4> {
        self.containers.get(container).map(|c| c.transform)
    }

    /// Hide/show a container (affects visibility of all children)
    pub fn set_container_hidden(&mut self, container: &str, hidden: bool) {
        if let Some(c) = self.containers.get_mut(container) {
            c.hidden = hidden;
        }
    }

    /// Check if container is hidden
    pub fn is_container_hidden(&self, container: &str) -> bool {
        self.containers
            .get(container)
            .map(|c| c.hidden)
            .unwrap_or(false)
    }
}

impl Default for SceneClass {
    fn default() -> Self {
        Self::new()
    }
}

/// Scene iterator implementation
struct SceneIteratorImpl<'a> {
    objects: &'a Vec<Box<dyn RenderObj>>,
    current_index: usize,
}

impl<'a> SceneIterator for SceneIteratorImpl<'a> {
    fn first(&mut self) {
        self.current_index = 0;
    }

    fn next(&mut self) {
        if self.current_index < self.objects.len() {
            self.current_index += 1;
        }
    }

    fn is_done(&self) -> bool {
        self.current_index >= self.objects.len()
    }

    fn current_item(&self) -> Option<&dyn RenderObj> {
        if self.current_index < self.objects.len() {
            Some(&*self.objects[self.current_index])
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct MockObj {
        name: String,
        transform: Mat4,
    }

    impl MockObj {
        fn new() -> Self {
            Self {
                name: "MockObj".to_string(),
                transform: Mat4::IDENTITY,
            }
        }
    }

    impl RenderObj for MockObj {
        fn update(&mut self, _dt: f32) {}
        fn is_visible(&self, _camera_pos: Vec3) -> bool {
            true
        }
        fn get_name(&self) -> &str {
            &self.name
        }
        fn set_transform(&mut self, transform: Mat4) {
            self.transform = transform;
        }
        fn get_transform(&self) -> &Mat4 {
            &self.transform
        }
        fn render(&self, _render_info: &RenderInfoClass) {
            // Mock render implementation
        }
    }

    #[test]
    fn test_scene_add_update() {
        let mut scene = SceneClass::new();
        scene.add_render_object(Box::new(MockObj::new()));
        scene.update(0.016);
        assert_eq!(scene.objects.len(), 1);
    }

    #[test]
    fn test_scene_lighting() {
        let mut scene = SceneClass::new();
        scene.set_ambient_light(Vec3::new(0.3, 0.3, 0.3));

        let light = Light::point(
            "TestLight".to_string(),
            Vec3::new(0.0, 10.0, 0.0),
            Vec3::ONE,
            50.0,
        );
        scene.add_light(light);

        assert_eq!(scene.lights.len(), 1);
        assert_eq!(scene.get_ambient_light(), Vec3::new(0.3, 0.3, 0.3));
    }

    #[test]
    fn test_camera_frustum() {
        let mut camera = CameraClass::new();
        camera.set_position(Vec3::new(0.0, 0.0, 10.0));

        let projection = Mat4::perspective_rh(60.0_f32.to_radians(), 16.0 / 9.0, 0.1, 1000.0);
        camera.set_projection_matrix(projection);

        assert!(camera.frustum.is_some());
    }

    #[test]
    fn test_scene_visibility_check() {
        let mut scene = SceneClass::new();
        scene.add_render_object(Box::new(MockObj::new()));

        let mut camera = CameraClass::new();
        // Set up a proper camera with frustum
        let projection = Mat4::perspective_rh(60.0_f32.to_radians(), 16.0 / 9.0, 0.1, 1000.0);
        let view = Mat4::look_at_rh(Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO, Vec3::Y);
        camera.set_projection_matrix(projection);
        camera.set_view_matrix(view);

        scene.visibility_check(&camera);

        assert!(scene.visibility_checked);
    }
}
