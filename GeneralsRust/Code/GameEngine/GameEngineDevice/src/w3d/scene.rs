//! CPU-side RTS W3D scene orchestration.
//!
//! This ports the non-GPU decision logic from
//! `GameEngineDevice/Source/W3DDevice/GameClient/W3DScene.cpp`: visibility
//! classification, deferred object queues, shroud/material pass selection,
//! translucent flushing, occluded-player stencil bookkeeping, light environment
//! preparation, and ray picking. Actual WGPU stencil/render-state submission
//! stays in the renderer/shadow modules.

use std::collections::BTreeMap;

use game_engine::common::game_common::MAX_PLAYER_COUNT;
use glam::{Mat4, Vec3};

pub const MAX_TRANSLUCENT_OBJECTS: usize = 500;
pub const MAX_OCCLUDER_OBJECTS: usize = 100;
pub const MAX_OCCLUDEE_OBJECTS: usize = 100;
pub const MAX_NON_OCCLUDER_OCCLUDEE_OBJECTS: usize = 500;
pub const MAX_VISIBLE_OCCLUDED_PLAYER_OBJECTS: usize = 512;
const NUMBER_PLAYER_COLOR_BITS: u32 = 4;

pub type RenderObjectId = u64;

pub const KINDOF_STRUCTURE: u32 = 1 << 0;
pub const KINDOF_SCORE: u32 = 1 << 1;
pub const KINDOF_SCORE_CREATE: u32 = 1 << 2;
pub const KINDOF_SCORE_DESTROY: u32 = 1 << 3;
pub const KINDOF_MP_COUNT_FOR_VICTORY: u32 = 1 << 4;
pub const KINDOF_INFANTRY: u32 = 1 << 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomScenePassMode {
    Default,
    AlphaMask,
}

impl Default for CustomScenePassMode {
    fn default() -> Self {
        Self::Default
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtraPassPolygonMode {
    Disable,
    Line,
    ClearLine,
    DepthOnly,
}

impl Default for ExtraPassPolygonMode {
    fn default() -> Self {
        Self::Disable
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderObjectClass {
    Model,
    TileMap,
    Image3D,
    Other,
}

impl Default for RenderObjectClass {
    fn default() -> Self {
        Self::Model
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectShroudStatus {
    Invalid,
    Clear,
    PartialClear,
    Fogged,
    Shrouded,
}

impl ObjectShroudStatus {
    fn needs_shroud_pass(self) -> bool {
        !matches!(self, Self::Clear | Self::PartialClear)
    }

    fn is_fogged_or_worse(self) -> bool {
        matches!(self, Self::Fogged | Self::Shrouded)
    }
}

impl Default for ObjectShroudStatus {
    fn default() -> Self {
        Self::Invalid
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DrawableRenderFlags(u32);

impl DrawableRenderFlags {
    pub const IS_NORMAL: u32 = 0;
    pub const IS_TRANSLUCENT: u32 = 1 << 0;
    pub const IS_OCCLUDED: u32 = 1 << 1;
    pub const POTENTIAL_OCCLUDER: u32 = 1 << 2;
    pub const POTENTIAL_OCCLUDEE: u32 = 1 << 3;
    pub const IS_NON_OCCLUDER_OR_OCCLUDEE: u32 = 1 << 4;
    pub const DELAYED_RENDER: u32 = 1 << 5;

    pub fn bits(self) -> u32 {
        self.0
    }

    pub fn contains(self, flag: u32) -> bool {
        (self.0 & flag) != 0
    }

    pub fn insert(&mut self, flag: u32) {
        self.0 |= flag;
    }

    pub fn remove(&mut self, flag: u32) {
        self.0 &= !flag;
    }

    pub fn reset(&mut self) {
        self.0 = Self::IS_NORMAL;
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundingSphere {
    pub center: Vec3,
    pub radius: f32,
}

impl BoundingSphere {
    pub fn new(center: Vec3, radius: f32) -> Self {
        Self { center, radius }
    }
}

impl Default for BoundingSphere {
    fn default() -> Self {
        Self {
            center: Vec3::ZERO,
            radius: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraInfo {
    pub transform: Mat4,
    pub position: Vec3,
    pub far_z: f32,
}

impl Default for CameraInfo {
    fn default() -> Self {
        Self {
            transform: Mat4::IDENTITY,
            position: Vec3::new(0.0, -100.0, 100.0),
            far_z: 10_000.0,
        }
    }
}

impl CameraInfo {
    pub fn sphere_visible(self, sphere: &BoundingSphere) -> bool {
        self.position.distance(sphere.center) <= self.far_z + sphere.radius
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct W3DLight {
    pub direction: Vec3,
    pub diffuse: Vec3,
    pub ambient: Vec3,
    pub enabled: bool,
    pub point_radius: Option<f32>,
    pub position: Vec3,
}

impl W3DLight {
    pub fn directional(direction: Vec3, diffuse: Vec3, ambient: Vec3) -> Self {
        Self {
            direction,
            diffuse,
            ambient,
            enabled: true,
            point_radius: None,
            position: Vec3::ZERO,
        }
    }

    pub fn point(position: Vec3, radius: f32, diffuse: Vec3, ambient: Vec3) -> Self {
        Self {
            direction: Vec3::ZERO,
            diffuse,
            ambient,
            enabled: true,
            point_radius: Some(radius),
            position,
        }
    }

    fn scaled_for_fog(self, fogged_light_frac: f32) -> Self {
        Self {
            diffuse: self.diffuse * fogged_light_frac,
            ambient: self.ambient * fogged_light_frac,
            ..self
        }
    }

    fn scaled_for_infantry(self, infantry_light_scale: f32) -> Self {
        Self {
            diffuse: (self.diffuse * infantry_light_scale).min(Vec3::ONE),
            ambient: (self.ambient * infantry_light_scale).min(Vec3::ONE),
            ..self
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LightEnvironment {
    pub center: Vec3,
    pub ambient: Vec3,
    pub lights: Vec<W3DLight>,
    pub last_camera_transform: Mat4,
}

impl Default for LightEnvironment {
    fn default() -> Self {
        Self {
            center: Vec3::ZERO,
            ambient: Vec3::ZERO,
            lights: Vec::new(),
            last_camera_transform: Mat4::IDENTITY,
        }
    }
}

impl LightEnvironment {
    pub fn reset(&mut self, center: Vec3, ambient: Vec3) {
        self.center = center;
        self.ambient = ambient;
        self.lights.clear();
    }

    pub fn add_light(&mut self, light: W3DLight) {
        if light.enabled {
            self.lights.push(light);
        }
    }

    pub fn pre_render_update(&mut self, camera_transform: Mat4) {
        self.last_camera_transform = camera_transform;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DrawableState {
    pub drawable_id: u32,
    pub object_id: Option<u32>,
    pub kindof_flags: u32,
    pub controlling_player_index: usize,
    pub player_color: u32,
    pub draws_in_mirror: bool,
    pub effectively_hidden: bool,
    pub fully_obscured_by_shroud: bool,
    pub effective_opacity: f32,
    pub safe_occlusion_frame: u32,
    pub shroud_status: ObjectShroudStatus,
    pub shroud_clear_frame: u32,
    pub effectively_dead: bool,
    pub receives_dynamic_lights: bool,
    pub tint_color: Option<Vec3>,
    pub selection_color: Option<Vec3>,
    pub second_material_pass_opacity: f32,
    pub stealth_visible_detected: bool,
}

impl Default for DrawableState {
    fn default() -> Self {
        Self {
            drawable_id: 0,
            object_id: None,
            kindof_flags: 0,
            controlling_player_index: 0,
            player_color: 0xffff_ffff,
            draws_in_mirror: true,
            effectively_hidden: false,
            fully_obscured_by_shroud: false,
            effective_opacity: 1.0,
            safe_occlusion_frame: 0,
            shroud_status: ObjectShroudStatus::Clear,
            shroud_clear_frame: 0,
            effectively_dead: false,
            receives_dynamic_lights: true,
            tint_color: None,
            selection_color: None,
            second_material_pass_opacity: 0.0,
            stealth_visible_detected: false,
        }
    }
}

impl DrawableState {
    pub fn is_kind_of(&self, flag: u32) -> bool {
        (self.kindof_flags & flag) != 0
    }

    fn score_occludee_kind(&self) -> bool {
        self.is_kind_of(KINDOF_SCORE)
            || self.is_kind_of(KINDOF_SCORE_CREATE)
            || self.is_kind_of(KINDOF_SCORE_DESTROY)
            || self.is_kind_of(KINDOF_MP_COUNT_FOR_VICTORY)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DrawableInfo {
    pub drawable: Option<DrawableState>,
    pub flags: DrawableRenderFlags,
    pub shroud_status_object_id: Option<u32>,
}

impl DrawableInfo {
    pub fn new(drawable: Option<DrawableState>) -> Self {
        Self {
            drawable,
            flags: DrawableRenderFlags::default(),
            shroud_status_object_id: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderObject {
    pub id: RenderObjectId,
    pub class_id: RenderObjectClass,
    pub name: String,
    pub bounding_sphere: BoundingSphere,
    pub visible: bool,
    pub force_visible: bool,
    pub hidden: bool,
    pub collision_type: u32,
    pub drawable_info: Option<DrawableInfo>,
    pub frame_update_count: u64,
}

impl Default for RenderObject {
    fn default() -> Self {
        Self {
            id: 0,
            class_id: RenderObjectClass::Model,
            name: String::new(),
            bounding_sphere: BoundingSphere::default(),
            visible: true,
            force_visible: false,
            hidden: false,
            collision_type: u32::MAX,
            drawable_info: None,
            frame_update_count: 0,
        }
    }
}

impl RenderObject {
    pub fn with_drawable(mut self, drawable: DrawableState) -> Self {
        self.drawable_info = Some(DrawableInfo::new(Some(drawable)));
        self
    }

    pub fn is_really_visible(&self) -> bool {
        self.visible && !self.hidden
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
    pub max_distance: f32,
}

impl Ray {
    pub fn new(origin: Vec3, direction: Vec3, max_distance: f32) -> Self {
        Self {
            origin,
            direction: direction.normalize_or_zero(),
            max_distance,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayHit {
    pub object_id: RenderObjectId,
    pub distance: f32,
    pub clipped_end: Vec3,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RenderEvent {
    FrameUpdate(RenderObjectId),
    Terrain(RenderObjectId, RenderPassKind),
    Object(RenderObjectId, RenderPassKind, LightEnvKind),
    Image3D(RenderObjectId),
    MeshFlush,
    ShaderFlush,
    Trees,
    NonStencilShadows,
    StencilShadows,
    StaticSortLists,
    Particles,
    SortingRendererFlush,
    ClearPendingDeletes,
    OccludedPlayerColor {
        player_index: usize,
        color_index: usize,
        stencil_ref: u32,
        color: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderPassKind {
    Normal,
    Shroud,
    Mask,
    HeatVision,
    HeatVisionOnly,
    Fogged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightEnvKind {
    Default,
    Infantry,
    Fogged,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneConfig {
    pub max_visible_translucent_objects: usize,
    pub max_visible_occluder_objects: usize,
    pub max_visible_occludee_objects: usize,
    pub max_visible_non_occluder_or_occludee_objects: usize,
    pub default_occlusion_delay: u32,
    pub enable_behind_building_markers: bool,
    pub show_behind_building_markers: bool,
    pub fog_alpha: f32,
    pub clear_alpha: f32,
    pub infantry_light_scale: f32,
    pub occluded_luminance_scale: f32,
    pub use_shadow_volumes: bool,
    pub shroud_on: bool,
}

impl Default for SceneConfig {
    fn default() -> Self {
        Self {
            max_visible_translucent_objects: MAX_TRANSLUCENT_OBJECTS,
            max_visible_occluder_objects: MAX_OCCLUDER_OBJECTS,
            max_visible_occludee_objects: MAX_OCCLUDEE_OBJECTS,
            max_visible_non_occluder_or_occludee_objects: MAX_NON_OCCLUDER_OCCLUDEE_OBJECTS,
            default_occlusion_delay: 0,
            enable_behind_building_markers: true,
            show_behind_building_markers: true,
            fog_alpha: 0.5,
            clear_alpha: 1.0,
            infantry_light_scale: 1.0,
            occluded_luminance_scale: 0.5,
            use_shadow_volumes: true,
            shroud_on: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderInfo {
    pub camera: CameraInfo,
    pub custom_pass_mode: CustomScenePassMode,
    pub material_pass_emissive_override: f32,
    pub alpha_override: f32,
    pub override_flags: u32,
}

impl Default for RenderInfo {
    fn default() -> Self {
        Self {
            camera: CameraInfo::default(),
            custom_pass_mode: CustomScenePassMode::Default,
            material_pass_emissive_override: 0.0,
            alpha_override: 1.0,
            override_flags: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct W3DScene {
    name: String,
    next_id: RenderObjectId,
    render_objects: Vec<RenderObject>,
    dynamic_lights: Vec<W3DLight>,
    global_lights: Vec<W3DLight>,
    infantry_lights: Vec<W3DLight>,
    draw_terrain_only: bool,
    custom_pass_mode: CustomScenePassMode,
    extra_pass_polygon_mode: ExtraPassPolygonMode,
    backface_culling_inverted: bool,
    visibility_checked: bool,
    terrain_object_present: bool,
    current_frame: u32,
    ambient_light: Vec3,
    infantry_ambient: Vec3,
    default_light_env: LightEnvironment,
    fogged_light_env: LightEnvironment,
    translucent_objects: Vec<RenderObjectId>,
    potential_occluders: Vec<RenderObjectId>,
    potential_occludees: Vec<RenderObjectId>,
    non_occluders_or_occludees: Vec<RenderObjectId>,
    occluded_objects_count: usize,
    stencil_shadow_mask: i32,
    render_events: Vec<RenderEvent>,
    config: SceneConfig,
}

impl Default for W3DScene {
    fn default() -> Self {
        Self::new(SceneConfig::default())
    }
}

impl W3DScene {
    pub fn new(config: SceneConfig) -> Self {
        Self {
            name: "RTS3DScene".to_string(),
            next_id: 1,
            render_objects: Vec::new(),
            dynamic_lights: Vec::new(),
            global_lights: Vec::new(),
            infantry_lights: Vec::new(),
            draw_terrain_only: false,
            custom_pass_mode: CustomScenePassMode::Default,
            extra_pass_polygon_mode: ExtraPassPolygonMode::Disable,
            backface_culling_inverted: false,
            visibility_checked: false,
            terrain_object_present: false,
            current_frame: 0,
            ambient_light: Vec3::splat(0.3),
            infantry_ambient: Vec3::splat(0.3),
            default_light_env: LightEnvironment::default(),
            fogged_light_env: LightEnvironment::default(),
            translucent_objects: Vec::with_capacity(config.max_visible_translucent_objects),
            potential_occluders: Vec::with_capacity(config.max_visible_occluder_objects),
            potential_occludees: Vec::with_capacity(config.max_visible_occludee_objects),
            non_occluders_or_occludees: Vec::with_capacity(
                config.max_visible_non_occluder_or_occludee_objects,
            ),
            occluded_objects_count: 0,
            stencil_shadow_mask: 0,
            render_events: Vec::new(),
            config,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn add_render_object(&mut self, mut object: RenderObject) -> RenderObjectId {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        object.id = id;
        self.render_objects.push(object);
        id
    }

    pub fn remove_render_object(&mut self, id: RenderObjectId) -> Option<RenderObject> {
        let index = self.render_objects.iter().position(|obj| obj.id == id)?;
        Some(self.render_objects.remove(index))
    }

    pub fn get_render_object(&self, id: RenderObjectId) -> Option<&RenderObject> {
        self.render_objects.iter().find(|obj| obj.id == id)
    }

    pub fn get_render_object_mut(&mut self, id: RenderObjectId) -> Option<&mut RenderObject> {
        self.render_objects.iter_mut().find(|obj| obj.id == id)
    }

    pub fn render_object_count(&self) -> usize {
        self.render_objects.len()
    }

    pub fn iter_render_objects(&self) -> impl Iterator<Item = &RenderObject> {
        self.render_objects.iter()
    }

    pub fn add_dynamic_light(&mut self, light: W3DLight) {
        self.dynamic_lights.push(light);
    }

    pub fn remove_dynamic_light(&mut self, index: usize) -> Option<W3DLight> {
        (index < self.dynamic_lights.len()).then(|| self.dynamic_lights.remove(index))
    }

    pub fn get_a_dynamic_light(&mut self) -> &mut W3DLight {
        let index = self
            .dynamic_lights
            .iter()
            .position(|light| !light.enabled)
            .unwrap_or_else(|| {
                self.dynamic_lights.push(W3DLight::directional(
                    Vec3::new(0.0, 0.0, -1.0),
                    Vec3::ONE,
                    Vec3::ZERO,
                ));
                self.dynamic_lights.len() - 1
            });
        self.dynamic_lights[index].enabled = true;
        &mut self.dynamic_lights[index]
    }

    pub fn dynamic_light_count(&self) -> usize {
        self.dynamic_lights.len()
    }

    pub fn set_global_light(&mut self, light: W3DLight, index: usize) {
        if self.global_lights.len() <= index {
            self.global_lights.resize(index + 1, light);
        }
        self.global_lights[index] = light;
    }

    pub fn set_ambient_light(&mut self, ambient: Vec3) {
        self.ambient_light = ambient;
    }

    pub fn set_custom_pass_mode(&mut self, mode: CustomScenePassMode) {
        self.custom_pass_mode = mode;
    }

    pub fn set_extra_pass_polygon_mode(&mut self, mode: ExtraPassPolygonMode) {
        self.extra_pass_polygon_mode = mode;
    }

    pub fn set_backface_culling_inverted(&mut self, inverted: bool) {
        self.backface_culling_inverted = inverted;
    }

    pub fn draw_terrain_only(&mut self, draw: bool) {
        self.draw_terrain_only = draw;
    }

    pub fn set_current_frame(&mut self, frame: u32) {
        self.current_frame = frame;
    }

    pub fn default_light_env(&self) -> &LightEnvironment {
        &self.default_light_env
    }

    pub fn fogged_light_env(&self) -> &LightEnvironment {
        &self.fogged_light_env
    }

    pub fn stencil_shadow_mask(&self) -> i32 {
        self.stencil_shadow_mask
    }

    pub fn render_events(&self) -> &[RenderEvent] {
        &self.render_events
    }

    pub fn clear_render_events(&mut self) {
        self.render_events.clear();
    }

    pub fn queue_counts(&self) -> (usize, usize, usize, usize) {
        (
            self.translucent_objects.len(),
            self.potential_occluders.len(),
            self.potential_occludees.len(),
            self.non_occluders_or_occludees.len(),
        )
    }

    pub fn visibility_check(&mut self, camera: &CameraInfo) {
        self.translucent_objects.clear();
        self.potential_occluders.clear();
        self.potential_occludees.clear();
        self.non_occluders_or_occludees.clear();
        self.occluded_objects_count = 0;
        self.terrain_object_present = false;
        self.stencil_shadow_mask = 0;

        let mut current_frame = self.current_frame;
        if current_frame <= self.config.default_occlusion_delay {
            current_frame = self.config.default_occlusion_delay + 1;
        }

        for object in &mut self.render_objects {
            if self.backface_culling_inverted {
                let draws_in_mirror = object
                    .drawable_info
                    .as_ref()
                    .and_then(|info| info.drawable.as_ref())
                    .map_or(true, |drawable| drawable.draws_in_mirror);
                object.visible = object.force_visible
                    || (draws_in_mirror && camera.sphere_visible(&object.bounding_sphere));
                continue;
            }

            object.visible = if object.force_visible {
                true
            } else if object.hidden {
                false
            } else {
                camera.sphere_visible(&object.bounding_sphere)
            };

            if !object.visible {
                continue;
            }

            if object.class_id == RenderObjectClass::TileMap {
                self.terrain_object_present = true;
            }

            let Some(info) = &mut object.drawable_info else {
                continue;
            };
            let Some(drawable) = &mut info.drawable else {
                continue;
            };

            if drawable.effectively_hidden || drawable.fully_obscured_by_shroud {
                object.visible = false;
                continue;
            }

            info.flags.reset();

            if drawable.effective_opacity != 1.0
                && self.translucent_objects.len() < self.config.max_visible_translucent_objects
            {
                info.flags.insert(DrawableRenderFlags::IS_TRANSLUCENT);
                self.translucent_objects.push(object.id);
            }

            if self.config.enable_behind_building_markers
                && self.config.show_behind_building_markers
            {
                if drawable.is_kind_of(KINDOF_STRUCTURE) {
                    if !info.flags.contains(DrawableRenderFlags::IS_TRANSLUCENT)
                        && self.potential_occluders.len() < self.config.max_visible_occluder_objects
                    {
                        self.potential_occluders.push(object.id);
                    }
                    info.flags.insert(DrawableRenderFlags::POTENTIAL_OCCLUDER);
                } else if drawable.object_id.is_some()
                    && drawable.score_occludee_kind()
                    && drawable.safe_occlusion_frame <= current_frame
                    && self.potential_occludees.len() < self.config.max_visible_occludee_objects
                {
                    self.potential_occludees.push(object.id);
                    info.flags.insert(DrawableRenderFlags::POTENTIAL_OCCLUDEE);
                } else if info.flags.bits() == DrawableRenderFlags::IS_NORMAL
                    && self.non_occluders_or_occludees.len()
                        < self.config.max_visible_non_occluder_or_occludee_objects
                {
                    self.non_occluders_or_occludees.push(object.id);
                    info.flags
                        .insert(DrawableRenderFlags::IS_NON_OCCLUDER_OR_OCCLUDEE);
                }
            }
        }

        self.visibility_checked = true;
    }

    pub fn render_specific_drawables(&mut self, rinfo: &mut RenderInfo, drawable_ids: &[u32]) {
        let object_ids: Vec<_> = self
            .render_objects
            .iter()
            .filter(|object| {
                object
                    .drawable_info
                    .as_ref()
                    .and_then(|info| info.drawable.as_ref())
                    .is_some_and(|drawable| drawable_ids.contains(&drawable.drawable_id))
            })
            .map(|object| object.id)
            .collect();

        for id in object_ids {
            self.render_one_object_by_id(rinfo, id);
        }
    }

    pub fn render(&mut self, rinfo: &mut RenderInfo) {
        rinfo.custom_pass_mode = self.custom_pass_mode;
        if self.extra_pass_polygon_mode == ExtraPassPolygonMode::Disable {
            self.update_player_color_passes();
            self.update_fixed_light_environments(rinfo);
            self.customized_render(rinfo);
            self.flush(rinfo);
        } else {
            let old_mode = self.custom_pass_mode;
            if self.extra_pass_polygon_mode == ExtraPassPolygonMode::ClearLine {
                self.custom_pass_mode = CustomScenePassMode::AlphaMask;
                self.customized_render(rinfo);
                self.flush(rinfo);
            }
            self.custom_pass_mode = old_mode;
            self.customized_render(rinfo);
            self.flush(rinfo);
        }
    }

    pub fn customized_render(&mut self, rinfo: &mut RenderInfo) {
        self.translucent_objects.clear();
        self.occluded_objects_count = 0;

        if !self.visibility_checked {
            self.visibility_check(&rinfo.camera);
        }
        self.visibility_checked = false;

        let object_ids: Vec<_> = self.render_objects.iter().map(|obj| obj.id).collect();
        let mut terrain_id = None;

        let backface_culling_inverted = self.backface_culling_inverted;
        for id in &object_ids {
            let mut pushed_frame_update = false;
            {
                let Some(object) = self.get_render_object_mut(*id) else {
                    continue;
                };
                if object.class_id == RenderObjectClass::TileMap {
                    terrain_id = Some(*id);
                }
                if !backface_culling_inverted {
                    object.frame_update_count = object.frame_update_count.saturating_add(1);
                    pushed_frame_update = true;
                }
            }
            if pushed_frame_update {
                self.render_events.push(RenderEvent::FrameUpdate(*id));
            }
        }

        if let Some(id) = terrain_id {
            self.render_terrain_object(id);
        }

        if self.draw_terrain_only {
            return;
        }

        for id in object_ids {
            let Some(object) = self.get_render_object(id) else {
                continue;
            };
            if object.class_id == RenderObjectClass::TileMap || !object.is_really_visible() {
                continue;
            }
            if object
                .drawable_info
                .as_ref()
                .is_some_and(|info| Self::is_delayed_in_stencil_scene(info.flags))
            {
                continue;
            }
            self.render_one_object_by_id(rinfo, id);
        }

        if self.custom_pass_mode == CustomScenePassMode::Default
            && self.terrain_object_present
            && !self.backface_culling_inverted
            && self.extra_pass_polygon_mode == ExtraPassPolygonMode::Disable
        {
            self.render_events.push(RenderEvent::StencilShadows);
        }
    }

    pub fn flush(&mut self, rinfo: &mut RenderInfo) {
        if self.custom_pass_mode == CustomScenePassMode::Default
            && self.extra_pass_polygon_mode == ExtraPassPolygonMode::Disable
        {
            self.render_events.push(RenderEvent::NonStencilShadows);
        }
        self.render_events.push(RenderEvent::MeshFlush);

        self.flush_occluded_objects_into_stencil(rinfo);
        self.render_events.push(RenderEvent::ShaderFlush);
        self.render_events.push(RenderEvent::Trees);

        if self.custom_pass_mode == CustomScenePassMode::Default
            && self.extra_pass_polygon_mode == ExtraPassPolygonMode::Disable
        {
            self.render_events.push(RenderEvent::StencilShadows);
        }

        self.render_events.push(RenderEvent::StaticSortLists);
        if self.custom_pass_mode == CustomScenePassMode::Default
            && self.extra_pass_polygon_mode == ExtraPassPolygonMode::Disable
        {
            self.flush_translucent_objects(rinfo);
            self.render_events.push(RenderEvent::Particles);
        }
        self.render_events.push(RenderEvent::SortingRendererFlush);
        self.render_events.push(RenderEvent::ClearPendingDeletes);
    }

    pub fn cast_ray(&self, ray: Ray, test_all: bool, collision_type: u32) -> Option<RayHit> {
        let mut best: Option<RayHit> = None;
        let mut best_distance = ray.max_distance;

        for object in &self.render_objects {
            if !test_all && !object.is_really_visible() {
                continue;
            }
            if object.collision_type & collision_type == 0 {
                continue;
            }
            let Some(distance) = ray_sphere_distance(ray, object.bounding_sphere) else {
                continue;
            };
            if distance < best_distance {
                best_distance = distance;
                best = Some(RayHit {
                    object_id: object.id,
                    distance,
                    clipped_end: ray.origin + ray.direction * distance,
                });
            }
        }

        best
    }

    pub fn clear_render_objects(&mut self) {
        self.render_objects.clear();
        self.translucent_objects.clear();
        self.potential_occluders.clear();
        self.potential_occludees.clear();
        self.non_occluders_or_occludees.clear();
        self.occluded_objects_count = 0;
        self.stencil_shadow_mask = 0;
        self.terrain_object_present = false;
        self.visibility_checked = false;
    }

    fn render_terrain_object(&mut self, id: RenderObjectId) {
        let pass = match self.custom_pass_mode {
            CustomScenePassMode::Default if self.config.shroud_on => RenderPassKind::Shroud,
            CustomScenePassMode::AlphaMask => RenderPassKind::Mask,
            CustomScenePassMode::Default => RenderPassKind::Normal,
        };
        self.render_events.push(RenderEvent::Terrain(id, pass));
    }

    fn render_one_object_by_id(&mut self, rinfo: &mut RenderInfo, id: RenderObjectId) {
        let Some(index) = self.render_objects.iter().position(|obj| obj.id == id) else {
            return;
        };

        let (class_id, visible, hidden, sphere, draw_info) = {
            let object = &self.render_objects[index];
            (
                object.class_id,
                object.visible,
                object.hidden,
                object.bounding_sphere,
                object.drawable_info.clone(),
            )
        };

        if class_id == RenderObjectClass::Image3D {
            self.render_events.push(RenderEvent::Image3D(id));
            return;
        }
        if !visible || hidden {
            return;
        }

        let mut light_kind = LightEnvKind::Default;
        let mut pass = RenderPassKind::Normal;

        match draw_info {
            Some(info) => {
                let Some(mut drawable) = info.drawable else {
                    pass = RenderPassKind::Fogged;
                    light_kind = LightEnvKind::Fogged;
                    self.render_events
                        .push(RenderEvent::Object(id, pass, light_kind));
                    return;
                };

                if drawable.effectively_hidden {
                    return;
                }

                let shroud_status = self.effective_shroud_status(&mut drawable);
                if drawable.is_kind_of(KINDOF_INFANTRY) {
                    light_kind = LightEnvKind::Infantry;
                }

                if drawable.second_material_pass_opacity != 0.0 {
                    rinfo.material_pass_emissive_override = drawable.second_material_pass_opacity;
                    pass = if drawable.stealth_visible_detected {
                        RenderPassKind::HeatVisionOnly
                    } else {
                        RenderPassKind::HeatVision
                    };
                } else if self.custom_pass_mode == CustomScenePassMode::AlphaMask {
                    pass = RenderPassKind::Mask;
                } else if self.custom_pass_mode == CustomScenePassMode::Default
                    && shroud_status.needs_shroud_pass()
                {
                    pass = RenderPassKind::Shroud;
                }

                self.add_object_lights(sphere, drawable.receives_dynamic_lights);
            }
            None => {
                self.add_object_lights(sphere, true);
            }
        }

        self.render_events
            .push(RenderEvent::Object(id, pass, light_kind));
    }

    fn effective_shroud_status(&self, drawable: &mut DrawableState) -> ObjectShroudStatus {
        let mut status = drawable.shroud_status;
        if status == ObjectShroudStatus::Clear {
            drawable.shroud_clear_frame = self.current_frame;
        } else if status.is_fogged_or_worse() && drawable.shroud_clear_frame != 0 {
            let mut limit = 2 * 30;
            if drawable.effectively_dead {
                limit += 3 * 30;
            }
            if self.current_frame < drawable.shroud_clear_frame + limit {
                status = ObjectShroudStatus::PartialClear;
            }
        }
        status
    }

    fn update_fixed_light_environments(&mut self, rinfo: &RenderInfo) {
        let fogged_light_frac = if self.config.clear_alpha == 0.0 {
            0.0
        } else {
            self.config.fog_alpha / self.config.clear_alpha
        };

        self.default_light_env.reset(Vec3::ZERO, self.ambient_light);
        self.fogged_light_env
            .reset(Vec3::ZERO, self.ambient_light * fogged_light_frac);
        self.infantry_lights.clear();

        for light in &self.global_lights {
            self.default_light_env.add_light(*light);
            self.infantry_lights
                .push(light.scaled_for_infantry(self.config.infantry_light_scale));
            self.fogged_light_env
                .add_light(light.scaled_for_fog(fogged_light_frac));
        }

        self.default_light_env
            .pre_render_update(rinfo.camera.transform);
        self.fogged_light_env
            .pre_render_update(rinfo.camera.transform);
        self.infantry_ambient = self.ambient_light;
    }

    fn update_player_color_passes(&mut self) {
        if !(self.config.enable_behind_building_markers && self.config.show_behind_building_markers)
        {
            return;
        }
    }

    fn add_object_lights(&mut self, sphere: BoundingSphere, receives_dynamic_lights: bool) {
        if !receives_dynamic_lights {
            return;
        }
        for light in &self.dynamic_lights {
            if !light.enabled {
                continue;
            }
            if let Some(radius) = light.point_radius {
                let light_sphere = BoundingSphere::new(light.position, radius);
                if !spheres_intersect(sphere, light_sphere) {
                    continue;
                }
            }
        }
    }

    fn flush_translucent_objects(&mut self, rinfo: &mut RenderInfo) {
        let translucent = self.translucent_objects.clone();
        for id in translucent {
            let Some(opacity) = self
                .get_render_object(id)
                .and_then(|object| object.drawable_info.as_ref())
                .and_then(|info| info.drawable.as_ref())
                .map(|drawable| drawable.effective_opacity)
            else {
                continue;
            };
            rinfo.alpha_override = opacity;
            self.render_one_object_by_id(rinfo, id);
        }
        rinfo.alpha_override = 1.0;
        self.translucent_objects.clear();
    }

    fn flush_occluded_objects_into_stencil(&mut self, rinfo: &mut RenderInfo) {
        self.stencil_shadow_mask = 0;
        if self.potential_occludees.is_empty() && self.potential_occluders.is_empty() {
            self.flush_deferred_without_stencil(rinfo);
            return;
        }

        let mut buckets: BTreeMap<usize, Vec<RenderObjectId>> = BTreeMap::new();
        for id in &self.potential_occludees {
            if let Some(drawable) = self
                .get_render_object(*id)
                .and_then(|object| object.drawable_info.as_ref())
                .and_then(|info| info.drawable.as_ref())
            {
                let player_index = drawable
                    .controlling_player_index
                    .min(MAX_PLAYER_COUNT.saturating_sub(1));
                let bucket = buckets.entry(player_index).or_default();
                if bucket.len() < MAX_VISIBLE_OCCLUDED_PLAYER_OBJECTS {
                    bucket.push(*id);
                }
            }
        }

        if buckets.is_empty() {
            self.flush_deferred_without_stencil(rinfo);
            return;
        }

        let mut used_player_color_bits = 0u32;
        let mut visible_player_colors = 0usize;
        let buckets: Vec<_> = buckets.into_iter().collect();
        for (player_index, ids) in buckets {
            let color_index = player_index_to_color_index(visible_player_colors + 1);
            let stencil_ref = ((color_index as u32) << 3) | 0x80;
            let color = self
                .get_render_object(ids[0])
                .and_then(|object| object.drawable_info.as_ref())
                .and_then(|info| info.drawable.as_ref())
                .map_or(0x80ff_ffff, |drawable| {
                    scale_argb_luminance(
                        drawable.player_color,
                        self.config.occluded_luminance_scale,
                    )
                });

            used_player_color_bits |= stencil_ref;
            visible_player_colors += 1;
            self.render_events.push(RenderEvent::OccludedPlayerColor {
                player_index,
                color_index,
                stencil_ref,
                color,
            });

            for id in ids {
                if let Some(object) = self.get_render_object_mut(id) {
                    if let Some(info) = &mut object.drawable_info {
                        info.flags.insert(DrawableRenderFlags::IS_OCCLUDED);
                    }
                }
                self.render_one_object_by_id(rinfo, id);
            }
        }

        let non_occluders = self.non_occluders_or_occludees.clone();
        for id in non_occluders {
            self.render_one_object_by_id(rinfo, id);
        }

        let occluders = self.potential_occluders.clone();
        for id in occluders {
            self.render_one_object_by_id(rinfo, id);
        }

        self.occluded_objects_count = self
            .render_objects
            .iter()
            .filter(|object| {
                object
                    .drawable_info
                    .as_ref()
                    .is_some_and(|info| info.flags.contains(DrawableRenderFlags::IS_OCCLUDED))
            })
            .count();

        self.stencil_shadow_mask = if visible_player_colors >= 8 && self.config.use_shadow_volumes {
            i32::from_ne_bytes([0x80, 0x80, 0x80, 0x80])
        } else {
            used_player_color_bits as i32
        };
    }

    fn flush_deferred_without_stencil(&mut self, rinfo: &mut RenderInfo) {
        let occludees = self.potential_occludees.clone();
        for id in occludees {
            self.render_one_object_by_id(rinfo, id);
        }

        let occluders = self.potential_occluders.clone();
        for id in occluders {
            self.render_one_object_by_id(rinfo, id);
        }

        let non_occluders = self.non_occluders_or_occludees.clone();
        for id in non_occluders {
            self.render_one_object_by_id(rinfo, id);
        }
    }

    fn is_delayed_in_stencil_scene(flags: DrawableRenderFlags) -> bool {
        flags.contains(DrawableRenderFlags::DELAYED_RENDER)
            || flags.contains(DrawableRenderFlags::POTENTIAL_OCCLUDER)
            || flags.contains(DrawableRenderFlags::IS_NON_OCCLUDER_OR_OCCLUDEE)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct W3D2DScene {
    name: String,
    object_ids: Vec<RenderObjectId>,
}

impl Default for W3D2DScene {
    fn default() -> Self {
        Self::new()
    }
}

impl W3D2DScene {
    pub fn new() -> Self {
        Self {
            name: "RTS2DScene".to_string(),
            object_ids: Vec::new(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn add_object(&mut self, id: RenderObjectId) {
        self.object_ids.push(id);
    }

    pub fn remove_object(&mut self, id: RenderObjectId) {
        self.object_ids.retain(|object_id| *object_id != id);
    }

    pub fn iter_objects(&self) -> impl Iterator<Item = RenderObjectId> + '_ {
        self.object_ids.iter().copied()
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct W3DInterfaceScene {
    object_ids: Vec<RenderObjectId>,
}

impl W3DInterfaceScene {
    pub fn add_object(&mut self, id: RenderObjectId) {
        self.object_ids.push(id);
    }

    pub fn remove_object(&mut self, id: RenderObjectId) {
        self.object_ids.retain(|object_id| *object_id != id);
    }

    pub fn iter_objects(&self) -> impl Iterator<Item = RenderObjectId> + '_ {
        self.object_ids.iter().copied()
    }
}

pub fn player_index_to_color_index(player_index: usize) -> usize {
    let mut result = 0usize;
    for bit in 0..NUMBER_PLAYER_COLOR_BITS {
        let flipped = NUMBER_PLAYER_COLOR_BITS - 1 - bit;
        if (player_index & (1usize << bit)) != 0 {
            result |= 1usize << flipped;
        }
    }
    result
}

fn ray_sphere_distance(ray: Ray, sphere: BoundingSphere) -> Option<f32> {
    if ray.direction == Vec3::ZERO {
        return None;
    }

    let sphere_vector = sphere.center - ray.origin;
    let alpha = sphere_vector.dot(ray.direction);
    let beta = sphere.radius * sphere.radius - (sphere_vector.dot(sphere_vector) - alpha * alpha);
    if beta < 0.0 {
        return None;
    }

    let distance = alpha - beta.sqrt();
    (distance >= 0.0 && distance <= ray.max_distance).then_some(distance)
}

fn spheres_intersect(a: BoundingSphere, b: BoundingSphere) -> bool {
    a.center.distance_squared(b.center) <= (a.radius + b.radius).powi(2)
}

fn scale_argb_luminance(color: u32, scale: f32) -> u32 {
    let alpha = color & 0xff00_0000;
    let r = (((color >> 16) & 0xff) as f32 * scale).clamp(0.0, 255.0) as u32;
    let g = (((color >> 8) & 0xff) as f32 * scale).clamp(0.0, 255.0) as u32;
    let b = ((color & 0xff) as f32 * scale).clamp(0.0, 255.0) as u32;
    alpha | (r << 16) | (g << 8) | b
}
