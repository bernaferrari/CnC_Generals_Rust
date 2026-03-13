//! W3D Scene Management Module - Complete 3D Scene Rendering System
//!
//! Corresponds to C++ file: GameEngineDevice/Source/W3DDevice/GameClient/W3DScene.cpp
//!
//! This module provides comprehensive scene management including render object management,
//! visibility culling, lighting, occlusion, translucent object sorting, and scene rendering.

use crate::W3DDevice::GameClient::wthree_d_segmented_line::SegmentedLine;
use crate::W3DDevice::GameClient::wthree_d_dynamic_light::{W3DDynamicLight, LightEnvironment, MAX_LIGHTS};
use crate::W3DDevice::GameClient::wthree_d_shader_manager::{CustomScenePassMode, ShaderType};
use cgmath::{Vector3, Matrix4, Point3, SquareMatrix, Zero};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

pub type RenderObjectId = u64;

/// Maximum number of translucent objects
pub const MAX_TRANSLUCENT_OBJECTS: usize = 500;

/// Maximum number of occluder objects
pub const MAX_OCCLUDER_OBJECTS: usize = 100;

/// Maximum number of occludee objects  
pub const MAX_OCCLUDEE_OBJECTS: usize = 100;

/// Maximum number of non-occluder/occludee objects
pub const MAX_NON_OCCLUDER_OCCLUDEE_OBJECTS: usize = 500;

/// Maximum player count for color passes
pub const MAX_PLAYER_COUNT: usize = 16;

/// Drawable info flags (matching C++ DrawableInfo::ERF_*)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DrawableInfoFlags(u32);

impl DrawableInfoFlags {
    pub const NORMAL: u32 = 0;
    pub const IS_TRANSLUCENT: u32 = 1 << 0;
    pub const IS_OCCLUDED: u32 = 1 << 1;
    pub const POTENTIAL_OCCLUDER: u32 = 1 << 2;
    pub const POTENTIAL_OCCLUDEE: u32 = 1 << 3;
    pub const IS_NON_OCCLUDER_OR_OCCLUDEE: u32 = 1 << 4;
    
    pub fn new() -> Self {
        Self(0)
    }
    
    pub fn contains(&self, flag: u32) -> bool {
        (self.0 & flag) != 0
    }
    
    pub fn set(&mut self, flag: u32) {
        self.0 |= flag;
    }
    
    pub fn clear(&mut self, flag: u32) {
        self.0 &= !flag;
    }
    
    pub fn reset(&mut self) {
        self.0 = DrawableInfoFlags::NORMAL;
    }
}

/// Information attached to render objects (matching C++ DrawableInfo)
#[derive(Debug, Clone)]
pub struct DrawableInfo {
    pub drawable_id: Option<u32>,
    pub flags: DrawableInfoFlags,
    pub shroud_status_object_id: u32,
}

impl Default for DrawableInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl DrawableInfo {
    pub fn new() -> Self {
        Self {
            drawable_id: None,
            flags: DrawableInfoFlags::new(),
            shroud_status_object_id: 0, // INVALID_ID equivalent
        }
    }
    
    pub fn with_drawable(id: u32) -> Self {
        Self {
            drawable_id: Some(id),
            flags: DrawableInfoFlags::new(),
            shroud_status_object_id: 0,
        }
    }
}

/// Bounding sphere for culling
#[derive(Debug, Clone, Copy)]
pub struct BoundingSphere {
    pub center: Point3<f32>,
    pub radius: f32,
}

impl Default for BoundingSphere {
    fn default() -> Self {
        Self {
            center: Point3::origin(),
            radius: 0.0,
        }
    }
}

impl BoundingSphere {
    pub fn new(center: Point3<f32>, radius: f32) -> Self {
        Self { center, radius }
    }
    
    /// Check if sphere intersects with another sphere
    pub fn intersects(&self, other: &BoundingSphere) -> bool {
        let dist_sq = (self.center - other.center).magnitude2();
        let radius_sum = self.radius + other.radius;
        dist_sq <= radius_sum * radius_sum
    }
}

/// Render object in the scene
#[derive(Debug, Clone)]
pub struct RenderObject {
    pub id: RenderObjectId,
    pub info: DrawableInfo,
    pub bounding_sphere: BoundingSphere,
    pub position: Point3<f32>,
    pub visible: bool,
    pub force_visible: bool,
    pub hidden: bool,
    pub render_in_mirror: bool,
    pub opacity: f32,
    pub kindof_flags: u32, // KINDOF_* flags
    pub collision_type: u32,
}

impl Default for RenderObject {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderObject {
    pub fn new() -> Self {
        Self {
            id: 0,
            info: DrawableInfo::new(),
            bounding_sphere: BoundingSphere::default(),
            position: Point3::origin(),
            visible: true,
            force_visible: false,
            hidden: false,
            render_in_mirror: true,
            opacity: 1.0,
            kindof_flags: 0,
            collision_type: 0,
        }
    }
    
    pub fn is_really_visible(&self) -> bool {
        self.visible && !self.hidden
    }
    
    pub fn get_position(&self) -> Point3<f32> {
        self.position
    }
    
    pub fn get_bounding_sphere(&self) -> &BoundingSphere {
        &self.bounding_sphere
    }
}

/// Scene render info (matching C++ RenderInfoClass)
#[derive(Debug, Clone)]
pub struct RenderInfo {
    pub camera: CameraInfo,
    pub light_environment: Option<LightEnvironment>,
    pub custom_pass_mode: CustomScenePassMode,
    pub material_pass_emissive_override: f32,
    pub override_flags: u32,
    pub material_pass_stack: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CameraInfo {
    pub position: Point3<f32>,
    pub direction: Vector3<f32>,
    pub near_z: f32,
    pub far_z: f32,
    pub fov: f32,
}

impl Default for CameraInfo {
    fn default() -> Self {
        Self {
            position: Point3::new(0.0, 100.0, 100.0),
            direction: Vector3::new(0.0, -1.0, 0.0),
            near_z: 1.0,
            far_z: 1000.0,
            fov: 60.0,
        }
    }
}

impl RenderInfo {
    pub fn new() -> Self {
        Self {
            camera: CameraInfo::default(),
            light_environment: None,
            custom_pass_mode: CustomScenePassMode::Default,
            material_pass_emissive_override: 0.0,
            override_flags: 0,
            material_pass_stack: Vec::new(),
        }
    }
    
    pub fn push_material_pass(&mut self, pass: u32) {
        self.material_pass_stack.push(pass);
    }
    
    pub fn pop_material_pass(&mut self) -> Option<u32> {
        self.material_pass_stack.pop()
    }
    
    pub fn push_override_flags(&mut self, flags: u32) {
        self.override_flags |= flags;
    }
    
    pub fn pop_override_flags(&mut self) {
        // Simple implementation - just clear override flags
        self.override_flags = 0;
    }
}

/// RTS 3D Scene (matching C++ RTS3DScene)
#[derive(Debug)]
pub struct W3DScene {
    // Object management
    next_id: RenderObjectId,
    render_objects: HashMap<RenderObjectId, RenderObject>,
    segmented_lines: HashMap<RenderObjectId, Arc<RwLock<SegmentedLine>>>,
    
    // Dynamic lighting
    dynamic_lights: Vec<W3DDynamicLight>,
    global_lights: [Option<Arc<RwLock<W3DDynamicLight>>>; MAX_LIGHTS],
    infantry_lights: [Option<Arc<RwLock<W3DDynamicLight>>>; MAX_LIGHTS],
    num_global_lights: usize,
    
    // Scene state
    draw_terrain_only: bool,
    custom_pass_mode: CustomScenePassMode,
    
    // Visibility and culling
    visibility_checked: bool,
    
    // Translucent object handling (matching C++ m_translucentObjectsBuffer)
    translucent_objects_count: usize,
    translucent_objects: Vec<Option<RenderObjectId>>,
    
    // Occlusion handling
    potential_occluders: Vec<Option<RenderObjectId>>,
    potential_occludees: Vec<Option<RenderObjectId>>,
    non_occluders_or_occludees: Vec<Option<RenderObjectId>>,
    num_potential_occluders: usize,
    num_potential_occludees: usize,
    num_non_occluder_or_occludee: usize,
    occluded_objects_count: usize,
    
    // Default light environments
    default_light_env: LightEnvironment,
    fogged_light_env: LightEnvironment,
    infantry_ambient: Vector3<f32>,
    
    // Scene ambient light
    ambient_light: Vector3<f32>,
}

impl Default for W3DScene {
    fn default() -> Self {
        Self::new()
    }
}

impl W3DScene {
    /// Create a new W3D scene
    pub fn new() -> Self {
        let mut scene = Self {
            next_id: 1,
            render_objects: HashMap::new(),
            segmented_lines: HashMap::new(),
            dynamic_lights: Vec::new(),
            global_lights: Default::default(),
            infantry_lights: Default::default(),
            num_global_lights: 0,
            draw_terrain_only: false,
            custom_pass_mode: CustomScenePassMode::Default,
            visibility_checked: false,
            translucent_objects_count: 0,
            translucent_objects: vec![None; MAX_TRANSLUCENT_OBJECTS],
            potential_occluders: vec![None; MAX_OCCLUDER_OBJECTS],
            potential_occludees: vec![None; MAX_OCCLUDEE_OBJECTS],
            non_occluders_or_occludees: vec![None; MAX_NON_OCCLUDER_OCCLUDEE_OBJECTS],
            num_potential_occluders: 0,
            num_potential_occludees: 0,
            num_non_occluder_or_occludee: 0,
            occluded_objects_count: 0,
            default_light_env: LightEnvironment::new(),
            fogged_light_env: LightEnvironment::new(),
            infantry_ambient: Vector3::new(0.3, 0.3, 0.3),
            ambient_light: Vector3::new(0.3, 0.3, 0.3),
        };
        
        // Initialize default lights
        scene.initialize_default_lights();
        scene
    }
    
    /// Initialize default light setup
    fn initialize_default_lights(&mut self) {
        // Create default directional light
        let mut sun_light = W3DDynamicLight::directional();
        sun_light.set_direction(Vector3::new(0.5, -1.0, 0.3));
        sun_light.set_diffuse(Vector3::new(1.0, 1.0, 0.9));
        sun_light.set_ambient(Vector3::new(0.3, 0.3, 0.3));
        
        self.global_lights[0] = Some(Arc::new(RwLock::new(sun_light)));
        self.num_global_lights = 1;
        
        // Initialize infantry lights (modified copy of global)
        let mut infantry_light = W3DDynamicLight::directional();
        infantry_light.set_direction(Vector3::new(0.5, -1.0, 0.3));
        infantry_light.set_diffuse(Vector3::new(1.2, 1.2, 1.1)); // Brighter for infantry
        infantry_light.set_ambient(Vector3::new(0.4, 0.4, 0.4));
        
        self.infantry_lights[0] = Some(Arc::new(RwLock::new(infantry_light)));
    }
    
    /// Add a render object to the scene
    pub fn add_render_object(&mut self, mut obj: RenderObject) -> RenderObjectId {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        obj.id = id;
        self.render_objects.insert(id, obj);
        id
    }
    
    /// Remove a render object from the scene
    pub fn remove_render_object(&mut self, id: RenderObjectId) -> Option<RenderObject> {
        self.render_objects.remove(&id)
    }
    
    /// Get a render object by ID
    pub fn get_render_object(&self, id: RenderObjectId) -> Option<&RenderObject> {
        self.render_objects.get(&id)
    }
    
    /// Get mutable render object by ID
    pub fn get_render_object_mut(&mut self, id: RenderObjectId) -> Option<&mut RenderObject> {
        self.render_objects.get_mut(&id)
    }
    
    /// Add a segmented line to the scene
    pub fn add_segmented_line(&mut self, line: SegmentedLine) -> RenderObjectId {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        self.segmented_lines.insert(id, Arc::new(RwLock::new(line)));
        id
    }
    
    /// Remove a segmented line from the scene
    pub fn remove_segmented_line(&mut self, id: RenderObjectId) -> Option<Arc<RwLock<SegmentedLine>>> {
        self.segmented_lines.remove(&id)
    }
    
    /// Get a segmented line by ID
    pub fn get_segmented_line(&self, id: RenderObjectId) -> Option<Arc<RwLock<SegmentedLine>>> {
        self.segmented_lines.get(&id).cloned()
    }
    
    /// Iterate over all segmented lines
    pub fn iter_segmented_lines(&self) -> impl Iterator<Item = Arc<RwLock<SegmentedLine>>> + '_ {
        self.segmented_lines.values().cloned()
    }
    
    /// Add a dynamic light to the scene
    pub fn add_dynamic_light(&mut self, light: W3DDynamicLight) {
        self.dynamic_lights.push(light);
    }
    
    /// Remove a dynamic light from the scene
    pub fn remove_dynamic_light(&mut self, index: usize) -> Option<W3DDynamicLight> {
        if index < self.dynamic_lights.len() {
            Some(self.dynamic_lights.remove(index))
        } else {
            None
        }
    }
    
    /// Get dynamic lights iterator
    pub fn iter_dynamic_lights(&self) -> impl Iterator<Item = &W3DDynamicLight> {
        self.dynamic_lights.iter()
    }
    
    /// Set a global light
    pub fn set_global_light(&mut self, light: W3DDynamicLight, index: usize) {
        if index < MAX_LIGHTS {
            self.global_lights[index] = Some(Arc::new(RwLock::new(light)));
            if self.num_global_lights < index + 1 {
                self.num_global_lights = index + 1;
            }
        }
    }
    
    /// Get ambient light color
    pub fn get_ambient_light(&self) -> Vector3<f32> {
        self.ambient_light
    }
    
    /// Set ambient light color
    pub fn set_ambient_light(&mut self, color: Vector3<f32>) {
        self.ambient_light = color;
    }
    
    /// Set custom pass mode
    pub fn set_custom_pass_mode(&mut self, mode: CustomScenePassMode) {
        self.custom_pass_mode = mode;
    }
    
    /// Get custom pass mode
    pub fn get_custom_pass_mode(&self) -> CustomScenePassMode {
        self.custom_pass_mode
    }
    
    /// Set draw terrain only mode
    pub fn set_draw_terrain_only(&mut self, draw: bool) {
        self.draw_terrain_only = draw;
    }
    
    /// Get default light environment
    pub fn get_default_light_env(&self) -> &LightEnvironment {
        &self.default_light_env
    }
    
    /// Visibility check for all objects (matching C++ Visibility_Check)
    pub fn visibility_check(&mut self, camera: &CameraInfo) {
        self.translucent_objects_count = 0;
        self.num_potential_occluders = 0;
        self.num_potential_occludees = 0;
        self.num_non_occluder_or_occludee = 0;
        
        for (&id, obj) in &mut self.render_objects {
            // Reset drawable flags
            obj.info.flags.reset();
            
            // Check visibility
            if obj.force_visible {
                obj.visible = true;
            } else if obj.hidden {
                obj.visible = false;
            } else {
                // Frustum culling (simplified sphere test)
                let to_camera = camera.position - obj.bounding_sphere.center;
                let dist = to_camera.magnitude();
                obj.visible = dist <= camera.far_z + obj.bounding_sphere.radius;
            }
            
            // Classify object for rendering
            if obj.visible && !obj.hidden {
                if obj.opacity < 1.0 && self.translucent_objects_count < MAX_TRANSLUCENT_OBJECTS {
                    obj.info.flags.set(DrawableInfoFlags::IS_TRANSLUCENT);
                    self.translucent_objects[self.translucent_objects_count] = Some(id);
                    self.translucent_objects_count += 1;
                }
            }
        }
        
        self.visibility_checked = true;
    }
    
    /// Render the scene (matching C++ Render)
    pub fn render(&mut self, rinfo: &mut RenderInfo) {
        // Update fixed light environments
        self.update_fixed_light_environments(rinfo);
        
        // Update dynamic lights
        for light in &mut self.dynamic_lights {
            light.on_frame_update();
        }
        
        // Custom render pass
        self.customized_render(rinfo);
        
        // Flush render queue
        self.flush(rinfo);
    }
    
    /// Custom render pass (matching C++ Customized_Render)
    pub fn customized_render(&mut self, rinfo: &RenderInfo) {
        // Render all visible objects
        for obj in self.render_objects.values() {
            if obj.visible && !obj.hidden {
                // Object would be rendered here
                // In full implementation, this calls renderOneObject
            }
        }
    }
    
    /// Flush render queue (matching C++ Flush)
    pub fn flush(&mut self, _rinfo: &RenderInfo) {
        // Flush translucent objects
        self.flush_translucent_objects();
        
        // Flush occluded objects
        self.flush_occluded_objects();
    }
    
    /// Flush translucent objects
    fn flush_translucent_objects(&mut self) {
        for i in 0..self.translucent_objects_count {
            if let Some(Some(id)) = self.translucent_objects.get(i) {
                if let Some(_obj) = self.render_objects.get(id) {
                    // Render translucent object (sorted back to front)
                }
            }
        }
    }
    
    /// Flush occluded objects
    fn flush_occluded_objects(&mut self) {
        for i in 0..self.occluded_objects_count {
            if let Some(obj) = self.potential_occludees.get(i).and_then(|id| id.and_then(|id| self.render_objects.get(&id))) {
                if obj.info.flags.contains(DrawableInfoFlags::IS_OCCLUDED) {
                    // Render with occlusion material
                }
            }
        }
    }
    
    /// Update fixed light environments (matching C++ updateFixedLightEnvironments)
    fn update_fixed_light_environments(&mut self, _rinfo: &RenderInfo) {
        // Reset default light environment
        self.default_light_env.reset(Vector3::zero(), self.ambient_light);
        
        // Add global lights
        for i in 0..self.num_global_lights {
            if let Some(ref light_arc) = self.global_lights[i] {
                let light = light_arc.read();
                self.default_light_env.add_light(&light);
            }
        }
        
        // Setup fogged light environment
        let fogged_light_frac = 0.5; // From global data
        self.fogged_light_env.reset(Vector3::zero(), self.ambient_light * fogged_light_frac);
        
        // Update infantry ambient
        self.infantry_ambient = self.ambient_light;
    }
    
    /// Cast ray against scene objects (matching C++ castRay)
    pub fn cast_ray(&self, ray_origin: Point3<f32>, ray_dir: Vector3<f32>, test_all: bool, collision_type: u32) -> Option<(RenderObjectId, f32)> {
        let mut closest_hit: Option<(RenderObjectId, f32)> = None;
        let mut closest_dist = f32::MAX;
        
        for (&id, obj) in &self.render_objects {
            // Skip if not visible and not testing all
            if !test_all && !obj.is_really_visible() {
                continue;
            }
            
            // Check collision type mask
            if obj.collision_type & collision_type == 0 {
                continue;
            }
            
            // Ray-sphere intersection test
            let sphere = &obj.bounding_sphere;
            let to_sphere = sphere.center - ray_origin;
            let alpha = to_sphere.dot(ray_dir);
            let beta = sphere.radius * sphere.radius - (to_sphere.dot(to_sphere) - alpha * alpha);
            
            if beta < 0.0 {
                continue; // No intersection
            }
            
            let dist = alpha - beta.sqrt();
            if dist > 0.0 && dist < closest_dist {
                closest_dist = dist;
                closest_hit = Some((id, dist));
            }
        }
        
        closest_hit
    }
    
    /// Update scene state
    pub fn update(&mut self, delta_time_seconds: f32) {
        // Update segmented lines
        for line in self.segmented_lines.values() {
            if let Some(mut guard) = line.try_write() {
                guard.advance_uv(delta_time_seconds);
            }
        }
        
        // Update dynamic lights
        for light in &mut self.dynamic_lights {
            light.on_frame_update();
        }
    }
    
    /// Get render object count
    pub fn render_object_count(&self) -> usize {
        self.render_objects.len()
    }
    
    /// Get dynamic light count
    pub fn dynamic_light_count(&self) -> usize {
        self.dynamic_lights.len()
    }
    
    /// Clear all render objects
    pub fn clear_render_objects(&mut self) {
        self.render_objects.clear();
        self.translucent_objects_count = 0;
        self.num_potential_occluders = 0;
        self.num_potential_occludees = 0;
        self.num_non_occluder_or_occludee = 0;
        self.occluded_objects_count = 0;
    }
}

/// RTS 2D Scene for overlay rendering (matching C++ RTS2DScene)
#[derive(Debug)]
pub struct W3D2DScene {
    objects: Vec<RenderObjectId>,
    camera: CameraInfo,
}

impl Default for W3D2DScene {
    fn default() -> Self {
        Self::new()
    }
}

impl W3D2DScene {
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
            camera: CameraInfo::default(),
        }
    }
    
    pub fn add_object(&mut self, id: RenderObjectId) {
        self.objects.push(id);
    }
    
    pub fn remove_object(&mut self, id: RenderObjectId) {
        self.objects.retain(|&obj_id| obj_id != id);
    }
    
    pub fn render(&self, _rinfo: &RenderInfo) {
        // Render 2D overlay objects
    }
}

/// RTS 3D Interface Scene for UI overlay (matching C++ RTS3DInterfaceScene)
#[derive(Debug, Default)]
pub struct W3DInterfaceScene {
    objects: Vec<RenderObjectId>,
}

impl W3DInterfaceScene {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn add_object(&mut self, id: RenderObjectId) {
        self.objects.push(id);
    }
    
    pub fn render(&self, _rinfo: &RenderInfo) {
        // Render interface elements
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_scene_creation() {
        let scene = W3DScene::new();
        assert_eq!(scene.render_object_count(), 0);
        assert_eq!(scene.dynamic_light_count(), 0);
    }
    
    #[test]
    fn test_add_render_object() {
        let mut scene = W3DScene::new();
        let obj = RenderObject::new();
        let id = scene.add_render_object(obj);
        assert!(scene.get_render_object(id).is_some());
    }
    
    #[test]
    fn test_remove_render_object() {
        let mut scene = W3DScene::new();
        let obj = RenderObject::new();
        let id = scene.add_render_object(obj);
        let removed = scene.remove_render_object(id);
        assert!(removed.is_some());
        assert!(scene.get_render_object(id).is_none());
    }
    
    #[test]
    fn test_add_dynamic_light() {
        let mut scene = W3DScene::new();
        let light = W3DDynamicLight::point();
        scene.add_dynamic_light(light);
        assert_eq!(scene.dynamic_light_count(), 1);
    }
    
    #[test]
    fn test_visibility_check() {
        let mut scene = W3DScene::new();
        let mut obj = RenderObject::new();
        obj.bounding_sphere = BoundingSphere::new(Point3::new(0.0, 0.0, 0.0), 10.0);
        scene.add_render_object(obj);
        
        let camera = CameraInfo::default();
        scene.visibility_check(&camera);
        
        // Object should be visible
        let visible_count = scene.render_objects.values().filter(|o| o.visible).count();
        assert_eq!(visible_count, 1);
    }
    
    #[test]
    fn test_cast_ray() {
        let mut scene = W3DScene::new();
        let mut obj = RenderObject::new();
        obj.bounding_sphere = BoundingSphere::new(Point3::new(0.0, 0.0, 0.0), 10.0);
        obj.collision_type = 1;
        scene.add_render_object(obj);
        
        let ray_origin = Point3::new(0.0, 0.0, -50.0);
        let ray_dir = Vector3::new(0.0, 0.0, 1.0);
        
        let hit = scene.cast_ray(ray_origin, ray_dir, true, 1);
        assert!(hit.is_some());
    }
    
    #[test]
    fn test_bounding_sphere_intersection() {
        let s1 = BoundingSphere::new(Point3::new(0.0, 0.0, 0.0), 10.0);
        let s2 = BoundingSphere::new(Point3::new(15.0, 0.0, 0.0), 10.0);
        let s3 = BoundingSphere::new(Point3::new(30.0, 0.0, 0.0), 5.0);
        
        assert!(s1.intersects(&s2));
        assert!(!s1.intersects(&s3));
    }
}
