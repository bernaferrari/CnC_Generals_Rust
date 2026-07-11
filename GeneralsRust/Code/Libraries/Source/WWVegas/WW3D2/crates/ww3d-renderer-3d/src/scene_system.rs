//! Scene Management System
//!
//! This module provides comprehensive scene management functionality
//! for organizing and rendering 3D objects efficiently.

use crate::core::error::RendererResult;
use crate::render_object_system::{
    FogSettings, RenderInfoClass, RenderObjClass, SceneBinding, SceneRegistrationType,
    SpecialRenderInfoClass,
};
use crate::rendering::lighting_system::LightEnvironmentClass;
use glam::{Vec3, Vec4};
use std::collections::HashMap;

/// Scene ID enumeration - equivalent to C++ SceneClass RTTI
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SceneId {
    Unknown = 0xFFFFFFFF,
    #[default]
    Scene = 0,
    Simple = 1,
}

/// Polygon render type - equivalent to C++ PolyRenderType
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolyRenderType {
    Point,
    Line,
    Fill,
}

/// Scene manager class - equivalent to C++ SceneClass
#[derive(Debug)]
pub struct SceneManagerClass {
    /// All render objects in the scene
    render_objects: HashMap<usize, Box<dyn RenderObjClass>>,
    /// Deterministic render order mirroring the legacy linked list
    render_order: Vec<usize>,
    /// Next available object ID
    next_object_id: usize,
    /// Ambient light color
    ambient_light: Vec3,
    /// Fog settings
    fog_enabled: bool,
    fog_color: Vec4,
    fog_start: f32,
    fog_end: f32,
    /// Lighting environment shared across the scene
    light_environment: LightEnvironmentClass,
    /// Depth cue settings
    depth_cue_enabled: bool,
    depth_cue_start: f32,
    depth_cue_end: f32,
    /// Polygon render mode
    polygon_mode: PolyRenderType,
    /// Scene ID
    scene_id: SceneId,
    /// Objects requesting per-frame updates
    update_list: Vec<usize>,
    /// Objects contributing lights to the scene
    light_list: Vec<usize>,
    /// Objects queued for release processing
    release_list: Vec<usize>,
}

impl SceneManagerClass {
    /// Create a new scene manager
    pub fn new() -> Self {
        Self {
            render_objects: HashMap::new(),
            render_order: Vec::new(),
            next_object_id: 0,
            ambient_light: Vec3::new(0.5, 0.5, 0.5),
            fog_enabled: false,
            fog_color: Vec4::new(0.5, 0.5, 0.5, 1.0),
            fog_start: 100.0,
            fog_end: 1000.0,
            depth_cue_enabled: false,
            depth_cue_start: 50.0,
            depth_cue_end: 500.0,
            polygon_mode: PolyRenderType::Fill,
            scene_id: SceneId::default(),
            light_environment: LightEnvironmentClass::new(),
            update_list: Vec::new(),
            light_list: Vec::new(),
            release_list: Vec::new(),
        }
    }

    /// Add a render object to the scene
    pub fn add_render_object(&mut self, mut object: Box<dyn RenderObjClass>) -> usize {
        let id = self.next_object_id;
        self.next_object_id += 1;
        {
            let mut binding = SceneRegistrationContext::new(self, id);
            object.notify_added(&mut binding, id);
        }
        self.render_order.push(id);
        self.render_objects.insert(id, object);
        id
    }

    /// Remove a render object from the scene
    pub fn remove_render_object(&mut self, id: usize) -> bool {
        if let Some(mut object) = self.render_objects.remove(&id) {
            {
                let mut binding = SceneRegistrationContext::new(self, id);
                object.notify_removed(&mut binding, id);
            }
            self.unregister_all(id);
            self.render_order.retain(|&entry| entry != id);
            true
        } else {
            false
        }
    }

    /// Get a render object by ID
    pub fn get_render_object(&self, id: usize) -> Option<&Box<dyn RenderObjClass>> {
        self.render_objects.get(&id)
    }

    /// Get a mutable render object by ID
    pub fn get_render_object_mut(&mut self, id: usize) -> Option<&mut Box<dyn RenderObjClass>> {
        self.render_objects.get_mut(&id)
    }

    /// Render all objects in the scene
    pub fn render(&self, rinfo: &RenderInfoClass) -> RendererResult<()> {
        let mode = self.polygon_mode;
        for object_id in &self.render_order {
            let object = match self.render_objects.get(object_id) {
                Some(obj) => obj,
                None => continue,
            };

            if !object.is_really_visible() {
                continue;
            }

            if !object.pre_render(rinfo)? {
                continue;
            }

            match mode {
                PolyRenderType::Fill | PolyRenderType::Line | PolyRenderType::Point => {
                    object.render(rinfo)?;
                }
            }

            object.post_render(rinfo)?;
        }
        Ok(())
    }

    /// Special render all objects
    pub fn special_render(&self, rinfo: &SpecialRenderInfoClass) -> RendererResult<()> {
        for object_id in &self.render_order {
            if let Some(object) = self.render_objects.get(object_id) {
                object.special_render(rinfo)?;
            }
        }
        Ok(())
    }

    /// Run per-frame update callbacks for objects that registered interest.
    pub fn update(&mut self, delta_time: f32) -> RendererResult<()> {
        let pending = self.update_list.clone();
        for object_id in pending {
            if let Some(object) = self.render_objects.get_mut(&object_id) {
                object.on_frame_update(delta_time)?;
            }
        }
        Ok(())
    }

    /// Get the number of render objects
    pub fn get_num_render_objects(&self) -> usize {
        self.render_objects.len()
    }

    /// Clear all render objects
    pub fn clear(&mut self) {
        self.render_objects.clear();
        self.render_order.clear();
        self.next_object_id = 0;
        self.update_list.clear();
        self.light_list.clear();
        self.release_list.clear();
    }

    /// Set ambient light color
    pub fn set_ambient_light(&mut self, color: Vec3) {
        self.ambient_light = color;
        self.light_environment.set_ambient(color);
    }

    /// Get ambient light color
    pub fn get_ambient_light(&self) -> Vec3 {
        self.ambient_light
    }

    /// Replace the scene lighting environment wholesale
    pub fn set_light_environment(&mut self, environment: LightEnvironmentClass) {
        self.ambient_light = environment.ambient;
        self.light_environment = environment;
    }

    /// Borrow the lighting environment for read-only inspection
    pub fn light_environment(&self) -> &LightEnvironmentClass {
        &self.light_environment
    }

    /// Borrow the lighting environment for editing lights in-place
    pub fn light_environment_mut(&mut self) -> &mut LightEnvironmentClass {
        &mut self.light_environment
    }

    /// Enable/disable fog
    pub fn set_fog_enabled(&mut self, enabled: bool) {
        self.fog_enabled = enabled;
    }

    /// Check if fog is enabled
    pub fn is_fog_enabled(&self) -> bool {
        self.fog_enabled
    }

    /// Set fog color
    pub fn set_fog_color(&mut self, color: Vec4) {
        self.fog_color = color;
    }

    /// Get fog color
    pub fn get_fog_color(&self) -> Vec4 {
        self.fog_color
    }

    /// Set fog range
    pub fn set_fog_range(&mut self, start: f32, end: f32) {
        self.fog_start = start;
        self.fog_end = end;
    }

    /// Get fog range
    pub fn get_fog_range(&self) -> (f32, f32) {
        (self.fog_start, self.fog_end)
    }

    /// Apply the scene's lighting and fog state to a render info payload
    pub fn apply_environment_to_render_info(&self, render_info: &mut RenderInfoClass) {
        render_info.set_lighting_environment(self.light_environment.clone());

        if self.fog_enabled {
            render_info.set_fog(FogSettings {
                enabled: true,
                color: Vec3::new(self.fog_color.x, self.fog_color.y, self.fog_color.z),
                start: self.fog_start,
                end: self.fog_end,
            });
        } else {
            render_info.clear_fog();
        }
    }

    /// Enable/disable depth cue
    pub fn set_depth_cue_enabled(&mut self, enabled: bool) {
        self.depth_cue_enabled = enabled;
    }

    /// Check if depth cue is enabled
    pub fn is_depth_cue_enabled(&self) -> bool {
        self.depth_cue_enabled
    }

    /// Set depth cue range
    pub fn set_depth_cue_range(&mut self, start: f32, end: f32) {
        self.depth_cue_start = start;
        self.depth_cue_end = end;
    }

    /// Get depth cue range
    pub fn get_depth_cue_range(&self) -> (f32, f32) {
        (self.depth_cue_start, self.depth_cue_end)
    }

    /// Get all render objects (for iteration)
    pub fn render_objects(&self) -> &HashMap<usize, Box<dyn RenderObjClass>> {
        &self.render_objects
    }

    /// Get mutable access to render objects
    pub fn render_objects_mut(&mut self) -> &mut HashMap<usize, Box<dyn RenderObjClass>> {
        &mut self.render_objects
    }

    /// Set the polygon rendering mode for subsequent draws
    pub fn set_polygon_mode(&mut self, mode: PolyRenderType) {
        self.polygon_mode = mode;
    }

    /// Current polygon rendering mode
    pub fn polygon_mode(&self) -> PolyRenderType {
        self.polygon_mode
    }

    /// Assign a scene identifier so higher-level systems can distinguish scene types
    pub fn set_scene_id(&mut self, scene_id: SceneId) {
        self.scene_id = scene_id;
    }

    /// Retrieve the scene identifier associated with this manager
    pub fn scene_id(&self) -> SceneId {
        self.scene_id
    }
}

impl SceneManagerClass {
    fn register_internal(&mut self, object_id: usize, registration: SceneRegistrationType) {
        let list = match registration {
            SceneRegistrationType::OnFrameUpdate => &mut self.update_list,
            SceneRegistrationType::Light => &mut self.light_list,
            SceneRegistrationType::Release => &mut self.release_list,
        };

        if !list.contains(&object_id) {
            list.push(object_id);
        }
    }

    fn unregister_internal(&mut self, object_id: usize, registration: SceneRegistrationType) {
        let list = match registration {
            SceneRegistrationType::OnFrameUpdate => &mut self.update_list,
            SceneRegistrationType::Light => &mut self.light_list,
            SceneRegistrationType::Release => &mut self.release_list,
        };

        if let Some(index) = list.iter().position(|&id| id == object_id) {
            list.swap_remove(index);
        }
    }

    fn unregister_all(&mut self, object_id: usize) {
        self.update_list.retain(|&id| id != object_id);
        self.light_list.retain(|&id| id != object_id);
        self.release_list.retain(|&id| id != object_id);
    }

    /// Register an object identifier in the scene light stream.
    pub fn register_light_object(&mut self, object_id: usize) {
        self.register_internal(object_id, SceneRegistrationType::Light);
    }

    /// Remove an object identifier from the scene light stream.
    pub fn unregister_light_object(&mut self, object_id: usize) {
        self.unregister_internal(object_id, SceneRegistrationType::Light);
    }

    /// Check whether an object identifier is registered as a light.
    pub fn is_light_registered(&self, object_id: usize) -> bool {
        self.light_list.contains(&object_id)
    }

    /// Number of objects currently registered as lights.
    pub fn registered_light_count(&self) -> usize {
        self.light_list.len()
    }
}

struct SceneRegistrationContext<'a> {
    scene: &'a mut SceneManagerClass,
    object_id: usize,
}

impl<'a> SceneRegistrationContext<'a> {
    fn new(scene: &'a mut SceneManagerClass, object_id: usize) -> Self {
        Self { scene, object_id }
    }
}

impl<'a> SceneBinding for SceneRegistrationContext<'a> {
    fn register(&mut self, object_id: usize, registration: SceneRegistrationType) {
        debug_assert_eq!(object_id, self.object_id);
        self.scene.register_internal(object_id, registration);
    }

    fn unregister(&mut self, object_id: usize, registration: SceneRegistrationType) {
        debug_assert_eq!(object_id, self.object_id);
        self.scene.unregister_internal(object_id, registration);
    }
}

impl Default for SceneManagerClass {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple scene class - basic scene implementation
#[derive(Debug)]
pub struct SimpleSceneClass {
    base_scene: SceneManagerClass,
}

impl SimpleSceneClass {
    /// Create a new simple scene
    pub fn new() -> Self {
        Self {
            base_scene: SceneManagerClass::new(),
        }
    }

    /// Add a render object to the scene
    pub fn add_render_object(&mut self, object: Box<dyn RenderObjClass>) -> usize {
        self.base_scene.add_render_object(object)
    }

    /// Remove a render object from the scene
    pub fn remove_render_object(&mut self, id: usize) -> bool {
        self.base_scene.remove_render_object(id)
    }

    /// Render the scene
    pub fn render(&self, rinfo: &RenderInfoClass) -> RendererResult<()> {
        self.base_scene.render(rinfo)
    }

    /// Run per-frame update callbacks for registered objects.
    pub fn update(&mut self, delta_time: f32) -> RendererResult<()> {
        self.base_scene.update(delta_time)
    }

    /// Get the underlying scene manager
    pub fn scene_manager(&self) -> &SceneManagerClass {
        &self.base_scene
    }

    /// Get mutable access to the scene manager
    pub fn scene_manager_mut(&mut self) -> &mut SceneManagerClass {
        &mut self.base_scene
    }

    /// Register a light object with the underlying scene.
    pub fn register_light_object(&mut self, object_id: usize) {
        self.base_scene.register_light_object(object_id);
    }

    /// Unregister a light object from the underlying scene.
    pub fn unregister_light_object(&mut self, object_id: usize) {
        self.base_scene.unregister_light_object(object_id);
    }

    /// Check whether a light object is registered with the scene.
    pub fn is_light_registered(&self, object_id: usize) -> bool {
        self.base_scene.is_light_registered(object_id)
    }

    /// Number of light objects registered with the scene.
    pub fn registered_light_count(&self) -> usize {
        self.base_scene.registered_light_count()
    }
}

impl Default for SimpleSceneClass {
    fn default() -> Self {
        Self::new()
    }
}

/// Scene iterator for efficient scene traversal
pub struct SceneIterator<'a> {
    scene: &'a SceneManagerClass,
    index: usize,
}

impl<'a> SceneIterator<'a> {
    pub fn new(scene: &'a SceneManagerClass) -> Self {
        Self { scene, index: 0 }
    }
}

impl<'a> Iterator for SceneIterator<'a> {
    type Item = (usize, &'a Box<dyn RenderObjClass>);

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < self.scene.render_order.len() {
            let object_id = self.scene.render_order[self.index];
            self.index += 1;
            if let Some(object) = self.scene.render_objects.get(&object_id) {
                return Some((object_id, object));
            }
        }
        None
    }
}

impl SceneManagerClass {
    /// Create an iterator over all render objects
    pub fn iter(&self) -> SceneIterator<'_> {
        SceneIterator::new(self)
    }
}

/// Compatibility alias for existing code
pub type SceneClass = SimpleSceneClass;

/// Scene submodule for compatibility with effects
pub mod scene {
    pub use super::SceneClass;
}
