//! W3D Scene Management Module - Complete 3D Scene Rendering System
//!
//! Corresponds to C++ file: GameEngineDevice/Source/W3DDevice/GameClient/W3DScene.cpp
//!
//! This module provides comprehensive scene management including render object management,
//! visibility culling, lighting, occlusion, translucent object sorting, and scene rendering.

use crate::W3DDevice::GameClient::wthree_d_asset_manager::{AssetMeshPayload, WthreeDAssetManager};
use crate::W3DDevice::GameClient::wthree_d_dynamic_light::{
    LightEnvironment, W3DDynamicLight, MAX_LIGHTS,
};
use crate::W3DDevice::GameClient::wthree_d_segmented_line::SegmentedLine;
use crate::W3DDevice::GameClient::wthree_d_shader_manager::CustomScenePassMode;
use crate::W3DDevice::GameClient::Shadow::wthree_d_shadow::{
    do_shadows, the_w3d_shadow_manager, Frustum as ShadowFrustum, RenderInfo as ShadowRenderInfo,
};
use cgmath::{Matrix4, Point3, SquareMatrix, Vector3, Zero};
use parking_lot::RwLock;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use wgpu::{IndexFormat, RenderPass};

pub type RenderObjectId = u64;

pub trait SceneRenderHook: std::fmt::Debug + Send + Sync {
    fn render(&self, rinfo: &RenderInfo);
}

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
    pub prototype_handle: Option<u64>,
    pub bounding_sphere: BoundingSphere,
    pub world_transform: Matrix4<f32>,
    pub position: Point3<f32>,
    pub object_scale: f32,
    pub visible: bool,
    pub force_visible: bool,
    pub hidden: bool,
    pub render_in_mirror: bool,
    pub opacity: f32,
    pub kindof_flags: u32, // KINDOF_* flags
    pub collision_type: u32,
    pub controlling_player_index: Option<usize>,
    pub is_terrain: bool,
    pub render_hook: Option<Arc<dyn SceneRenderHook>>,
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
            prototype_handle: None,
            bounding_sphere: BoundingSphere::default(),
            world_transform: Matrix4::identity(),
            position: Point3::origin(),
            object_scale: 1.0,
            visible: true,
            force_visible: false,
            hidden: false,
            render_in_mirror: true,
            opacity: 1.0,
            kindof_flags: 0,
            collision_type: 0,
            controlling_player_index: None,
            is_terrain: false,
            render_hook: None,
        }
    }

    pub fn with_render_hook(mut self, render_hook: Arc<dyn SceneRenderHook>) -> Self {
        self.render_hook = Some(render_hook);
        self
    }

    pub fn with_prototype_handle(mut self, prototype_handle: u64) -> Self {
        self.prototype_handle = Some(prototype_handle);
        self
    }

    pub fn set_render_hook(&mut self, render_hook: Option<Arc<dyn SceneRenderHook>>) {
        self.render_hook = render_hook;
    }

    pub fn set_prototype_handle(&mut self, prototype_handle: Option<u64>) {
        self.prototype_handle = prototype_handle;
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

    pub fn set_controlling_player_index(&mut self, player_index: Option<usize>) {
        self.controlling_player_index = player_index;
    }

    pub fn prototype_meshes<'a>(
        &'a self,
        asset_manager: &'a WthreeDAssetManager,
    ) -> Option<&'a [AssetMeshPayload]> {
        let prototype_handle = self.prototype_handle?;
        let prototype = asset_manager.find_prototype_by_handle(prototype_handle)?;
        Some(prototype.meshes.as_slice())
    }

    pub fn render<'a>(
        &'a self,
        rinfo: &RenderInfo,
        render_pass: Option<&mut RenderPass<'a>>,
        asset_manager: Option<&'a WthreeDAssetManager>,
    ) {
        if let Some(render_hook) = &self.render_hook {
            render_hook.render(rinfo);
            return;
        }

        let (Some(render_pass), Some(asset_manager)) = (render_pass, asset_manager) else {
            return;
        };

        let Some(meshes) = self.prototype_meshes(asset_manager) else {
            return;
        };

        for mesh in meshes {
            let (Some(vertex_buffer), Some(index_buffer)) =
                (&mesh.vertex_buffer, &mesh.index_buffer)
            else {
                continue;
            };

            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.set_index_buffer(index_buffer.slice(..), IndexFormat::Uint32);
            render_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
        }
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
    terrain_object_present: bool,
    frame_number: u64,

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
    last_stencil_shadow_mask: i32,

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
            terrain_object_present: false,
            frame_number: 0,
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
            last_stencil_shadow_mask: 0,
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
    pub fn remove_segmented_line(
        &mut self,
        id: RenderObjectId,
    ) -> Option<Arc<RwLock<SegmentedLine>>> {
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

    /// Set whether a terrain object is currently present in the scene.
    pub fn set_terrain_object_present(&mut self, present: bool) {
        self.terrain_object_present = present;
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
        self.occluded_objects_count = 0;
        self.terrain_object_present = false;
        self.last_stencil_shadow_mask = 0;
        the_w3d_shadow_manager().write().set_stencil_shadow_mask(0);

        for (&id, obj) in &mut self.render_objects {
            // Preserve explicit classification bits if upstream code set them.
            let classification_bits = obj.info.flags.0
                & (DrawableInfoFlags::POTENTIAL_OCCLUDER
                    | DrawableInfoFlags::POTENTIAL_OCCLUDEE
                    | DrawableInfoFlags::IS_NON_OCCLUDER_OR_OCCLUDEE);

            // Reset transient drawable flags.
            obj.info.flags.reset();
            obj.info.flags.set(classification_bits);

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
                if obj.is_terrain {
                    self.terrain_object_present = true;
                }

                if obj.opacity < 1.0 && self.translucent_objects_count < MAX_TRANSLUCENT_OBJECTS {
                    obj.info.flags.set(DrawableInfoFlags::IS_TRANSLUCENT);
                    self.translucent_objects[self.translucent_objects_count] = Some(id);
                    self.translucent_objects_count += 1;
                    continue;
                }

                if obj
                    .info
                    .flags
                    .contains(DrawableInfoFlags::POTENTIAL_OCCLUDER)
                    && self.num_potential_occluders < MAX_OCCLUDER_OBJECTS
                {
                    self.potential_occluders[self.num_potential_occluders] = Some(id);
                    self.num_potential_occluders += 1;
                    continue;
                }

                if obj
                    .info
                    .flags
                    .contains(DrawableInfoFlags::IS_NON_OCCLUDER_OR_OCCLUDEE)
                    && self.num_non_occluder_or_occludee < MAX_NON_OCCLUDER_OCCLUDEE_OBJECTS
                {
                    self.non_occluders_or_occludees[self.num_non_occluder_or_occludee] = Some(id);
                    self.num_non_occluder_or_occludee += 1;
                    continue;
                }

                // Default opaque path: treat as potential occludee when no explicit class exists.
                if self.num_potential_occludees < MAX_OCCLUDEE_OBJECTS {
                    obj.info.flags.set(DrawableInfoFlags::POTENTIAL_OCCLUDEE);
                    self.potential_occludees[self.num_potential_occludees] = Some(id);
                    self.num_potential_occludees += 1;
                }
            }
        }

        self.visibility_checked = true;
    }

    /// Render the scene (matching C++ Render)
    pub fn render(&mut self, rinfo: &mut RenderInfo) {
        let frame_number = self.frame_number;
        self.frame_number = self.frame_number.wrapping_add(1);

        // Update fixed light environments
        self.update_fixed_light_environments(rinfo);

        // Update dynamic lights
        for light in &mut self.dynamic_lights {
            light.on_frame_update();
        }

        // Custom render pass
        self.customized_render(rinfo);

        // Flush render queue
        self.flush(rinfo, frame_number);
    }

    /// Custom render pass (matching C++ Customized_Render)
    pub fn customized_render(&mut self, rinfo: &RenderInfo) {
        if !self.visibility_checked {
            self.visibility_check(&rinfo.camera);
        }

        // C++ clears the visibility flag after the per-frame render traversal so the next
        // frame will rebuild the visible/occlusion lists.
        self.visibility_checked = false;

        let should_queue_shadows =
            self.custom_pass_mode == CustomScenePassMode::Default && self.terrain_object_present;

        // C++ queues shadows only after the terrain pass is known to exist.
        // This module only emulates the queueing signal, because the actual shadow render
        // path lives in the separate shadow module with a different RenderInfo type.
        the_w3d_shadow_manager()
            .write()
            .queue_shadows(should_queue_shadows);

        // Render all visible objects
        for obj in self.render_objects.values() {
            if obj.visible && !obj.hidden {
                obj.render(rinfo, None, None);
            }
        }
    }

    /// Flush render queue — matches C++ RTS3DScene::Flush() (W3DScene.cpp:809-848)
    pub fn flush(&mut self, rinfo: &RenderInfo, frame_number: u64) {
        if self.custom_pass_mode != CustomScenePassMode::Default {
            self.flush_occluded_objects_into_stencil(rinfo);
            self.flush_translucent_objects(rinfo);
            return;
        }

        self.flush_non_stencil_shadow_sequence_hook(rinfo, frame_number);
        self.flush_occluded_objects_into_stencil(rinfo);
        self.flush_stencil_shadow_sequence_hook(rinfo, frame_number);
        self.flush_translucent_objects(rinfo);
    }

    fn flush_non_stencil_shadow_sequence_hook(&self, rinfo: &RenderInfo, frame_number: u64) {
        if self.custom_pass_mode != CustomScenePassMode::Default {
            return;
        }

        let mut shadow_rinfo = self.build_shadow_render_info(rinfo, frame_number);
        do_shadows(&mut shadow_rinfo, false);
    }

    /// Flush translucent objects
    fn flush_translucent_objects(&self, rinfo: &RenderInfo) {
        // C++ flushTranslucentObjects (W3DScene.cpp:1581): iterates translucent buffer,
        // sets rinfo.alphaOverride per drawable, calls renderOneObject, then flushes
        // the mesh renderer and resets alphaOverride.
        for i in 0..self.translucent_objects_count {
            if let Some(Some(id)) = self.translucent_objects.get(i) {
                if let Some(obj) = self.render_objects.get(id) {
                    obj.render(rinfo, None, None);
                }
            }
        }
        self.translucent_objects_count = 0;
    }

    fn flush_stencil_shadow_sequence_hook(&self, rinfo: &RenderInfo, frame_number: u64) {
        if self.custom_pass_mode != CustomScenePassMode::Default {
            return;
        }

        let mut shadow_rinfo = self.build_shadow_render_info(rinfo, frame_number);
        do_shadows(&mut shadow_rinfo, true);
    }

    fn build_shadow_render_info(&self, rinfo: &RenderInfo, frame_number: u64) -> ShadowRenderInfo {
        let camera_frustum = Some(ShadowFrustum::from_camera(
            glam::Vec3::new(
                rinfo.camera.position.x,
                rinfo.camera.position.y,
                rinfo.camera.position.z,
            ),
            glam::Vec3::new(
                rinfo.camera.direction.x,
                rinfo.camera.direction.y,
                rinfo.camera.direction.z,
            ),
            rinfo.camera.near_z,
            rinfo.camera.far_z,
            rinfo.camera.fov,
        ));

        ShadowRenderInfo {
            camera_frustum,
            frame_number,
        }
    }

    fn player_index_to_color_index(player_index: usize) -> usize {
        const NUMBER_PLAYER_COLOR_BITS: usize = 4;
        let nibble = (player_index & ((1 << NUMBER_PLAYER_COLOR_BITS) - 1)) as u8;
        (nibble.reverse_bits() >> (8 - NUMBER_PLAYER_COLOR_BITS)) as usize
    }

    fn flush_occluded_objects_into_stencil(&mut self, rinfo: &RenderInfo) {
        self.last_stencil_shadow_mask = 0;
        the_w3d_shadow_manager().write().set_stencil_shadow_mask(0);

        let has_deferred_lists = self.num_potential_occludees > 0
            || self.num_potential_occluders > 0
            || self.num_non_occluder_or_occludee > 0;

        if !has_deferred_lists {
            return;
        }

        let occluder_ids =
            self.collect_render_object_ids(&self.potential_occluders, self.num_potential_occluders);
        let occludee_ids =
            self.collect_render_object_ids(&self.potential_occludees, self.num_potential_occludees);
        let non_occluder_ids = self.collect_render_object_ids(
            &self.non_occluders_or_occludees,
            self.num_non_occluder_or_occludee,
        );

        let mut visible_occludees = Vec::new();
        let mut buckets: BTreeMap<usize, Vec<RenderObjectId>> = BTreeMap::new();

        for id in occludee_ids {
            let Some(occludee) = self.render_objects.get(&id) else {
                continue;
            };

            if self.is_occluded_by_any_occluder(&rinfo.camera, occludee, &occluder_ids) {
                let player_index = occludee
                    .controlling_player_index
                    .unwrap_or(0)
                    .min(MAX_PLAYER_COUNT.saturating_sub(1));
                buckets.entry(player_index).or_default().push(id);
            } else {
                visible_occludees.push(id);
            }
        }

        self.occluded_objects_count = buckets.values().map(|ids| ids.len()).sum();
        if self.occluded_objects_count == 0 {
            self.flush_deferred_lists_without_stencil(
                &visible_occludees,
                &occluder_ids,
                &non_occluder_ids,
                rinfo,
            );
            return;
        }

        let mut used_stencil_refs: u32 = 0;
        let mut visible_player_count = 0usize;

        // C++ (W3DScene.cpp:1366-1415): iterates each player bucket, sets stencil state per
        // player color index, then calls renderOneObject on every occluded object in that
        // bucket. We invoke per-object render here; actual stencil buffer state manipulation
        // requires WGPU pipeline integration (PARITY_NOTE: stencil enable, Z-enable, stencil
        // func/pass/zfail/fail ops, and per-player color index ref are all missing).
        for (_player_index, object_ids) in &buckets {
            let color_index = Self::player_index_to_color_index(visible_player_count + 1);
            visible_player_count += 1;
            let stencil_ref = ((color_index as u32) << 3) | 0x80;
            used_stencil_refs |= stencil_ref;

            for id in object_ids {
                if let Some(obj) = self.render_objects.get_mut(id) {
                    obj.info.flags.set(DrawableInfoFlags::IS_OCCLUDED);
                }
                if let Some(obj) = self.render_objects.get(id) {
                    obj.render(rinfo, None, None);
                }
            }
        }

        if visible_player_count >= 8 {
            // C++ falls back to an MSB-only stencil mask once too many visible players are in
            // play because there are not enough low bits left for shadow volumes.
            used_stencil_refs = 0x80808080;
        }

        self.last_stencil_shadow_mask = used_stencil_refs as i32;
        the_w3d_shadow_manager()
            .write()
            .set_stencil_shadow_mask(self.last_stencil_shadow_mask);

        // Non-occluder/occludee and occluder lists are still present in the deferred queues.
        // We keep the flush ordering explicit even though the real mesh renderer is not wired in.
        self.flush_deferred_lists_without_stencil(
            &visible_occludees,
            &occluder_ids,
            &non_occluder_ids,
            rinfo,
        );
    }

    fn flush_deferred_lists_without_stencil(
        &mut self,
        visible_occludees: &[RenderObjectId],
        occluders: &[RenderObjectId],
        non_occluders_or_occludees: &[RenderObjectId],
        rinfo: &RenderInfo,
    ) {
        if visible_occludees.is_empty()
            && occluders.is_empty()
            && non_occluders_or_occludees.is_empty()
        {
            return;
        }

        // Fallback path mirroring the C++ "no occluded objects" branch: render deferred
        // occludees, then occluders, then non-occluder/non-occludee objects.
        for id in visible_occludees {
            if let Some(obj) = self.render_objects.get(id) {
                obj.render(rinfo, None, None);
            }
        }

        for id in occluders {
            if let Some(obj) = self.render_objects.get(id) {
                obj.render(rinfo, None, None);
            }
        }

        for id in non_occluders_or_occludees {
            if let Some(obj) = self.render_objects.get(id) {
                obj.render(rinfo, None, None);
            }
        }
    }

    fn collect_render_object_ids(
        &self,
        list: &[Option<RenderObjectId>],
        count: usize,
    ) -> Vec<RenderObjectId> {
        list.iter().take(count).filter_map(|id| *id).collect()
    }

    fn is_occluded_by_any_occluder(
        &self,
        camera: &CameraInfo,
        occludee: &RenderObject,
        occluder_ids: &[RenderObjectId],
    ) -> bool {
        let ray_origin = camera.position;
        let ray_target = occludee.bounding_sphere.center;
        let ray = ray_target - ray_origin;
        let ray_length = ray.magnitude();

        if ray_length <= f32::EPSILON {
            return false;
        }

        let ray_dir = ray / ray_length;
        for occluder_id in occluder_ids {
            let Some(occluder) = self.render_objects.get(occluder_id) else {
                continue;
            };

            if occluder.id == occludee.id {
                continue;
            }

            if Self::ray_hits_sphere(ray_origin, ray_dir, ray_length, &occluder.bounding_sphere) {
                return true;
            }
        }

        false
    }

    fn ray_hits_sphere(
        ray_origin: Point3<f32>,
        ray_dir: Vector3<f32>,
        max_distance: f32,
        sphere: &BoundingSphere,
    ) -> bool {
        let to_sphere = sphere.center - ray_origin;
        let alpha = to_sphere.dot(ray_dir);
        let beta = sphere.radius * sphere.radius - (to_sphere.dot(to_sphere) - alpha * alpha);

        if beta < 0.0 {
            return false;
        }

        let hit_distance = alpha - beta.sqrt();
        hit_distance >= 0.0 && hit_distance <= max_distance
    }

    fn synthetic_player_color(player_index: usize, color_index: usize) -> u32 {
        let seed =
            ((player_index as u32 + 1) * 0x45d9f3b) ^ ((color_index as u32 + 1) * 0x27d4eb2d);
        0xff000000 | (seed & 0x00ff_ffff)
    }

    /// Update fixed light environments (matching C++ updateFixedLightEnvironments)
    fn update_fixed_light_environments(&mut self, _rinfo: &RenderInfo) {
        // Reset default light environment
        self.default_light_env
            .reset(Vector3::zero(), self.ambient_light);

        // Add global lights
        for i in 0..self.num_global_lights {
            if let Some(ref light_arc) = self.global_lights[i] {
                let light = light_arc.read();
                self.default_light_env.add_light(&light);
            }
        }

        // Setup fogged light environment
        let fogged_light_frac = 0.5; // From global data
        self.fogged_light_env
            .reset(Vector3::zero(), self.ambient_light * fogged_light_frac);

        // Update infantry ambient
        self.infantry_ambient = self.ambient_light;
    }

    /// Cast ray against scene objects (matching C++ castRay)
    pub fn cast_ray(
        &self,
        ray_origin: Point3<f32>,
        ray_dir: Vector3<f32>,
        test_all: bool,
        collision_type: u32,
    ) -> Option<(RenderObjectId, f32)> {
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
        self.last_stencil_shadow_mask = 0;
        self.terrain_object_present = false;
        self.visibility_checked = false;
    }

    /// Iterate all render objects (used by the GPU render pass to draw visible objects).
    pub fn iter_render_objects(&self) -> impl Iterator<Item = &RenderObject> {
        self.render_objects.values()
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

    /// Render 2D overlay objects.
    ///
    /// # C++ Reference
    ///
    /// Matches `RTS2DScene::Customized_Render` (W3DScene.cpp lines 1744-1750)
    /// which calls `SimpleSceneClass::Customized_Render(rinfo)` to iterate all
    /// render objects in the scene. Each render object's `Render()` is called,
    /// which queues draw primitives. After all objects are processed, the 2D
    /// pipeline is flushed.
    ///
    /// In C++, `RTS2DScene::draw()` triggers `WW3D::Render(this, m_camera)`
    /// which walks the render object list and calls each object's render method.
    /// The Rust equivalent iterates object IDs, looks them up in the parent
    /// scene's render object map, and calls `RenderObject::render()`.
    pub fn render<'a>(
        &'a self,
        rinfo: &RenderInfo,
        render_pass: Option<&mut RenderPass<'a>>,
        scene: &'a W3DScene,
    ) {
        for &obj_id in &self.objects {
            if let Some(obj) = scene.render_objects.get(&obj_id) {
                if obj.visible && !obj.hidden {
                    obj.render(rinfo, render_pass.as_deref_mut(), None);
                }
            }
        }
    }

    pub fn iter_objects(&self) -> impl Iterator<Item = &RenderObjectId> {
        self.objects.iter()
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

    pub fn remove_object(&mut self, id: RenderObjectId) {
        self.objects.retain(|&obj_id| obj_id != id);
    }

    /// # C++ Reference
    ///
    /// Matches `RTS3DInterfaceScene::Customized_Render` (W3DScene.cpp lines
    /// 1812-1817) which delegates to `SimpleSceneClass::Customized_Render(rinfo)`.
    /// That walks the render list calling each object's `Render()` method.
    ///
    /// Interface scene objects are rendered on top of everything else (3D world,
    /// 2D overlay) to provide in-world UI elements like health bars, selection
    /// indicators, rally-point markers, and similar decorations.
    pub fn render<'a>(
        &'a self,
        rinfo: &RenderInfo,
        render_pass: Option<&mut RenderPass<'a>>,
        scene: &'a W3DScene,
    ) {
        for &obj_id in &self.objects {
            if let Some(obj) = scene.render_objects.get(&obj_id) {
                if obj.visible && !obj.hidden {
                    obj.render(rinfo, render_pass.as_deref_mut(), None);
                }
            }
        }
    }

    pub fn iter_objects(&self) -> impl Iterator<Item = &RenderObjectId> {
        self.objects.iter()
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
