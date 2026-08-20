// Drawable state, camera view bridge, and TheGameClient
//
// Split from `helpers.rs` for module-size parity.
// Observable behavior is unchanged.

#[derive(Clone, Debug)]
pub struct ProjectileStreamState {
    pub lines: Vec<Vec<Coord3D>>,
    pub texture_name: AsciiString,
    pub width: Real,
    pub tile_factor: Real,
    pub scroll_rate: Real,
}

/// Bone transform override for animated models (turret rotations, recoil shifts).
/// Consumed by the render bridge to produce `render_bridge::BoneOverride`.
#[derive(Clone, Debug)]
pub struct BoneOverrideState {
    pub bone_index: i32,
    pub transform: Matrix3D,
}

/// Per-frame mesh UV override for render sub-objects selected by mesh-name prefix.
#[derive(Clone, Debug)]
pub struct MeshUvOverrideState {
    pub mesh_name_prefix: String,
    pub u_offset: Real,
    pub v_offset: Real,
}

/// Per-frame sub-object visibility override.
///
/// Mirrors W3DModelDraw's C++ `HideShowSubObjInfo` list after normalization.
/// `hidden == true` corresponds to C++ `showSubObject(name, FALSE)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubObjectVisibilityState {
    pub sub_object_name: String,
    pub hidden: bool,
}

/// Stable identity for one actual DrawModule invocation in a logic drawable
/// frame.  This deliberately uses the runtime draw-module ordinal instead of
/// Rust's internal `ActiveModelState` index: C++ `Drawable::draw()` dispatches
/// its compacted `DrawModule**` list in that order, while a condition/transition
/// vector index is an implementation detail and is not a portable draw-state
/// identity.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModelDrawSourceIdentity {
    pub runtime_draw_ordinal: u32,
    pub module_name: String,
    pub module_tag: String,
    pub module_tag_name_key: NameKeyType,
}

/// The four authored W3D weapon-bone bases selected by this draw module's
/// current model-condition state.  They are transported as names, never as
/// renderer-local bone indices, so a later presentation layer can validate
/// them against a freshly loaded W3D hierarchy.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModelDrawWeaponBoneBindings {
    pub fire_fx: [String; WEAPONSLOT_COUNT],
    pub recoil: [String; WEAPONSLOT_COUNT],
    pub muzzle_flash: [String; WEAPONSLOT_COUNT],
    pub launch: [String; WEAPONSLOT_COUNT],
}

/// Per-frame model draw data written by W3DModelDraw::do_draw_module().
/// Read by the GameClient device layer to produce `render_bridge::DrawSubmission`.
#[derive(Clone, Debug)]
pub struct ModelDrawState {
    /// Filled only when the enclosing `Drawable::draw()` commits this active
    /// module result.  A module cannot mutate a result committed by a previous
    /// draw module in the same frame.
    pub source: ModelDrawSourceIdentity,
    /// The GameLogic drawable ID is diagnostic only.  It is intentionally not
    /// used as an object association key by the GameClient or Main renderer.
    pub logic_drawable_id: u32,
    pub model_name: String,
    pub world_transform: Matrix3D,
    /// Exact `RenderObjClass::Set_ObjectScale` input captured at the
    /// W3DModelDraw commit boundary.  `None` means the owning Drawable was not
    /// available; consumers must not substitute the usual scale of `1.0`.
    pub render_object_scale: Option<Real>,
    /// Exact `RenderObjClass::Set_ObjectColor` value captured at the
    /// W3DModelDraw commit boundary.  This is optional so source records made
    /// by non-W3D producers cannot accidentally acquire a guessed color.
    pub render_object_color: Option<u32>,
    /// Raw ModelConditionFlags bits (u128); client maps to RenderConditionFlags.
    pub condition_flags_bits: u128,
    pub bone_overrides: Vec<BoneOverrideState>,
    pub animation_name: Option<String>,
    /// 0.0–1.0 fraction through the current animation cycle.
    pub animation_time: f32,
    /// Matches AnimMode discriminant (0=Manual … 5=OnceBackwards).
    pub animation_mode: i32,
    pub mesh_uv_overrides: Vec<MeshUvOverrideState>,
    pub sub_object_visibility: Vec<SubObjectVisibilityState>,
    pub weapon_bone_bindings: ModelDrawWeaponBoneBindings,
}

#[derive(Clone, Debug)]
pub struct DrawableState {
    pub template_name: String,
    pub indicator_color: Color,
    pub position: Coord3D,
    pub orientation: Real,
    pub shroud_status_object_id: ObjectID,
    pub beam_start: Option<Coord3D>,
    pub beam_end: Option<Coord3D>,
    pub beam_width: Option<Real>,
    pub laser_growth_frames: Option<i32>,
    pub laser_growth_start_frame: Option<UnsignedInt>,
    pub projectile_stream: Option<ProjectileStreamState>,
    pub drawable: Option<Arc<RwLock<Drawable>>>,
    pub expiration_frame: Option<UnsignedInt>,
}

fn laser_width_scalar(
    growth_frames: Option<i32>,
    growth_start: Option<UnsignedInt>,
) -> Real {
    let Some(frames) = growth_frames else {
        return 1.0;
    };
    if frames == 0 {
        return 1.0;
    }
    let start = growth_start.unwrap_or(0);
    let now = TheGameLogic::get_frame();
    let elapsed = now.saturating_sub(start) as Real;
    let span = frames.unsigned_abs() as Real;
    if span <= 0.0 {
        return 1.0;
    }
    if frames > 0 {
        (elapsed / span).clamp(0.0, 1.0)
    } else {
        (1.0 - (elapsed / span)).clamp(0.0, 1.0)
    }
}

static DRAWABLE_STATE: Lazy<Mutex<HashMap<u32, DrawableState>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Per-object bridge state for one logic drawable frame.
///
/// This is intentionally separate from `DRAWABLE_STATE`, which is keyed by a
/// GameClient `DrawableID`.  C++ keeps DrawableID and ObjectID distinct and
/// binds them by pointer; collapsing them made normal W3D output disappear or
/// attach to a different object whenever the two counters diverged.
#[derive(Default)]
struct ObjectModelDrawFrameState {
    active_source: Option<ModelDrawSourceIdentity>,
    active: Option<ModelDrawState>,
    committed: Vec<ModelDrawState>,
}

static OBJECT_MODEL_DRAW_FRAMES: Lazy<Mutex<HashMap<ObjectID, ObjectModelDrawFrameState>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// C++ `TWheelInfo` subset used by W3DTruckDraw bone placement.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DrawWheelInfo {
    pub front_left_height_offset: Real,
    pub front_right_height_offset: Real,
    pub rear_left_height_offset: Real,
    pub rear_right_height_offset: Real,
    pub wheel_angle: Real,
}

static OBJECT_WHEEL_INFO: Lazy<Mutex<HashMap<ObjectID, DrawWheelInfo>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Persistent point light (C++ `W3DDynamicLight` used by police cars).
#[derive(Clone, Debug)]
pub struct ScenePointLight {
    pub id: u64,
    pub pos: [f32; 3],
    pub ambient: [f32; 3],
    pub diffuse: [f32; 3],
    pub far_start: f32,
    pub far_end: f32,
    pub enabled: bool,
    pub fade_remaining: u32,
}

static SCENE_POINT_LIGHTS: Lazy<Mutex<HashMap<u64, ScenePointLight>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static NEXT_SCENE_POINT_LIGHT: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(1);

pub fn create_scene_point_light() -> u64 {
    let id = NEXT_SCENE_POINT_LIGHT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if let Ok(mut lights) = SCENE_POINT_LIGHTS.lock() {
        lights.insert(
            id,
            ScenePointLight {
                id,
                pos: [0.0; 3],
                ambient: [0.0; 3],
                diffuse: [0.0; 3],
                far_start: 5.0,
                far_end: 15.0,
                enabled: true,
                fade_remaining: 0,
            },
        );
    }
    id
}

pub fn update_scene_point_light(
    id: u64,
    pos: [f32; 3],
    ambient: [f32; 3],
    diffuse: [f32; 3],
    far_start: f32,
    far_end: f32,
) {
    if let Ok(mut lights) = SCENE_POINT_LIGHTS.lock() {
        if let Some(light) = lights.get_mut(&id) {
            light.pos = pos;
            light.ambient = ambient;
            light.diffuse = diffuse;
            light.far_start = far_start;
            light.far_end = far_end;
            light.enabled = true;
        }
    }
}

pub fn fade_scene_point_light(id: u64, frames: u32) {
    if let Ok(mut lights) = SCENE_POINT_LIGHTS.lock() {
        if let Some(light) = lights.get_mut(&id) {
            light.fade_remaining = frames.max(1);
        }
    }
}

pub fn tick_scene_point_lights() {
    if let Ok(mut lights) = SCENE_POINT_LIGHTS.lock() {
        let mut dead = Vec::new();
        for light in lights.values_mut() {
            if light.fade_remaining == 0 {
                continue;
            }
            light.fade_remaining -= 1;
            let factor = light.fade_remaining as f32 / 5.0;
            light.ambient = [
                light.ambient[0] * factor,
                light.ambient[1] * factor,
                light.ambient[2] * factor,
            ];
            light.diffuse = [
                light.diffuse[0] * factor,
                light.diffuse[1] * factor,
                light.diffuse[2] * factor,
            ];
            light.far_end = (light.far_end * factor).max(light.far_start);
            if light.fade_remaining == 0 {
                light.enabled = false;
                dead.push(light.id);
            }
        }
        for id in dead {
            lights.remove(&id);
        }
    }
}

pub fn scene_point_lights() -> Vec<ScenePointLight> {
    SCENE_POINT_LIGHTS
        .lock()
        .map(|lights| lights.values().filter(|l| l.enabled).cloned().collect())
        .unwrap_or_default()
}

pub static TERRAIN_TREE_STATE: Lazy<Mutex<HashMap<u32, TerrainTreeRegistration>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Bridge trait for camera view operations.
///
/// Implemented by GameClient to forward calls to the real `View` struct
/// (which lives in GameClient and cannot be imported by GameLogic).
///
/// All methods use `&self` because the implementation uses interior mutability
/// (the real View is accessed via `with_tactical_view` which uses a thread-local
/// `RefCell`).
///
/// Integer types are used for enums (`CameraLockType`, `FilterType`, `FilterMode`)
/// so that GameLogic does not need to import GameClient enum types. Values must
/// match the C++ enum ordering exactly.
pub trait CameraViewBridge: Send + Sync {
    fn set_camera_lock(&self, id: Option<u32>);
    fn set_snap_mode(&self, lock_type: i32, distance: f32);
    fn snap_to_camera_lock(&self);
    fn move_camera_to(
        &self,
        x: f32,
        y: f32,
        z: f32,
        ms: i32,
        shutter: i32,
        enabled: bool,
        ease_in: f32,
        ease_out: f32,
    );
    fn zoom_camera(&self, zoom: f32, ms: i32, ease_in: f32, ease_out: f32);
    fn pitch_camera(&self, pitch: f32, ms: i32, ease_in: f32, ease_out: f32);
    fn rotate_camera(&self, rotations: f32, ms: i32, ease_in: f32, ease_out: f32);
    fn camera_mod_look_toward(&self, x: f32, y: f32, z: f32);
    fn camera_mod_final_look_toward(&self, x: f32, y: f32, z: f32);
    fn camera_mod_final_pitch(&self, pitch: f32, ease_in: f32, ease_out: f32);
    fn camera_mod_final_zoom(&self, zoom: f32, ease_in: f32, ease_out: f32);
    fn camera_mod_freeze_time(&self);
    fn camera_mod_freeze_angle(&self);
    fn is_time_frozen(&self) -> bool {
        false
    }
    fn is_camera_movement_finished(&self) -> bool {
        true
    }
    fn set_default_view(&self, pitch: f32, angle: f32, max_height: f32);
    fn reset_camera(&self, x: f32, y: f32, z: f32, ms: i32, ease_in: f32, ease_out: f32);
    fn look_at(&self, x: f32, y: f32, z: f32);
    fn set_view_filter(&self, filter_type: i32) -> bool;
    fn set_view_filter_mode(&self, mode: i32) -> bool;
    fn set_view_filter_pos(&self, x: f32, y: f32, z: f32);
    fn rotate_camera_toward_object(
        &self,
        object_id: u32,
        milliseconds: i32,
        hold_milliseconds: i32,
        ease_in: f32,
        ease_out: f32,
    );
    fn rotate_camera_toward_position(
        &self,
        x: f32,
        y: f32,
        z: f32,
        milliseconds: i32,
        ease_in: f32,
        ease_out: f32,
        reverse: bool,
    );
}

static CAMERA_VIEW_BRIDGE: OnceLock<Arc<dyn CameraViewBridge>> = OnceLock::new();

pub fn register_camera_view_bridge(bridge: Arc<dyn CameraViewBridge>) -> bool {
    CAMERA_VIEW_BRIDGE.set(bridge).is_ok()
}

pub fn get_camera_view_bridge() -> Option<&'static Arc<dyn CameraViewBridge>> {
    CAMERA_VIEW_BRIDGE.get()
}

/// Game client bridge for drawables/scorch marks and visual effects
pub struct TheGameClient;

impl TheGameClient {
    pub fn get() -> Option<&'static Self> {
        static CLIENT: OnceLock<TheGameClient> = OnceLock::new();
        Some(CLIENT.get_or_init(|| TheGameClient))
    }

    pub fn register_camera_view_bridge(bridge: Arc<dyn CameraViewBridge>) -> bool {
        register_camera_view_bridge(bridge)
    }

    pub fn get_camera_view_bridge() -> Option<&'static Arc<dyn CameraViewBridge>> {
        get_camera_view_bridge()
    }

    /// Synchronize the client frame counter with the logic frame.
    ///
    /// ## C++ Reference: GameLogic.cpp line 3596
    /// C++: TheGameClient->setFrame(now);
    pub fn set_frame(&self, _frame: UnsignedInt) {
        // In the full implementation, this would sync the drawable/camera
        // frame counter so client-side animations and effects advance
        // in lock-step with the simulation. The current Rust client-side
        // does not maintain a separate frame counter.
        let _ = _frame; // suppress unused warning until full implementation
    }

    pub fn notify_terrain_object_moved(&self, object_id: ObjectID) {
        // C++ W3DGameClient.cpp:202 TheTerrainRenderObject->unitMoved(obj)
        let Some(info) = OBJECT_REGISTRY.with_object(object_id, |obj| {
            let pos = *obj.get_position();
            let (dir_x, dir_y) = obj.get_unit_direction_vector_2d();
            let geom = obj.get_geometry_info();
            TerrainUnitMovedInfo {
                object_id: object_id as u32,
                x: pos.x,
                y: pos.y,
                z: pos.z,
                dir_x,
                dir_y,
                major_radius: geom.get_major_radius(),
                minor_radius: geom.get_minor_radius(),
                is_box: false,
                crusher_level: obj.get_crusher_level() as i32,
                immobile: obj.is_kind_of(crate::common::KindOf::Immobile),
                frame: TheGameLogic::get_frame(),
            }
        }) else {
            return;
        };
        if let Some(hook) = get_terrain_unit_moved_hook() {
            hook(info);
        }
    }

    pub fn create_drawable(&self, template: &dyn crate::common::ThingTemplate) -> u32 {
        let id = Drawable::allocate_drawable_id();
        let beam_width = template
            .as_any()
            .downcast_ref::<EngineThingTemplateAdapter>()
            .and_then(|adapter| {
                adapter.draw_modules.iter().find_map(|entry| {
                    if entry.name.as_str().eq_ignore_ascii_case("W3DLaserDraw") {
                        entry
                            .data
                            .as_any()
                            .downcast_ref::<W3DLaserDrawModuleData>()
                            .map(|data| data.outer_beam_width * 0.5)
                    } else {
                        None
                    }
                })
            });

        let model_name = template.get_model_name().to_string();
        let drawable_type = if template.is_kind_of(KindOf::Structure) {
            DrawableType::Static
        } else {
            DrawableType::Animated
        };
        let drawable = Arc::new(RwLock::new(Drawable::new(
            id,
            INVALID_ID,
            model_name,
            drawable_type,
        )));

        let module_thing: Arc<dyn ModuleThing> = Arc::new(DrawableThingHandle::new(&drawable));
        let mut drawable_modules: Vec<(
            ModuleInterfaceType,
            AsciiString,
            AsciiString,
            Arc<dyn ModuleData>,
            Box<dyn Module>,
        )> = Vec::new();

        if let Ok(factory_guard) = get_module_factory() {
            if let Some(factory) = factory_guard.as_ref() {
                for entry in template.get_draw_module_info().iter() {
                    let module_name = entry.name.clone();
                    let module_data = Arc::clone(&entry.data);
                    let module_data_for_entry = Arc::clone(&module_data);
                    let interface_mask = entry.interface_flags();

                    if factory.find_module_interface_mask(&module_name, ModuleType::Draw)
                        == ModuleInterfaceType::NONE
                    {
                        continue;
                    }

                    if let Ok(module) = factory.new_module(
                        module_thing.clone(),
                        &module_name,
                        module_data,
                        ModuleType::Draw,
                    ) {
                        drawable_modules.push((
                            interface_mask,
                            module_name.clone(),
                            entry.module_tag.clone(),
                            module_data_for_entry,
                            module,
                        ));
                    }
                }

                for entry in template.get_client_update_module_info().iter() {
                    let module_name = entry.name.clone();
                    let module_data = Arc::clone(&entry.data);
                    let module_data_for_entry = Arc::clone(&module_data);
                    let interface_mask = entry.interface_flags();

                    if factory.find_module_interface_mask(&module_name, ModuleType::ClientUpdate)
                        == ModuleInterfaceType::NONE
                    {
                        continue;
                    }

                    if let Ok(module) = factory.new_module(
                        module_thing.clone(),
                        &module_name,
                        module_data,
                        ModuleType::ClientUpdate,
                    ) {
                        drawable_modules.push((
                            interface_mask,
                            module_name.clone(),
                            entry.module_tag.clone(),
                            module_data_for_entry,
                            module,
                        ));
                    }
                }
            }
        }

        if !drawable_modules.is_empty() {
            if let Ok(mut guard) = drawable.write() {
                for (interface_mask, name, tag, module_data, module) in drawable_modules {
                    let _ = guard.add_module(interface_mask, name, tag, module_data, module);
                }
            }
        }
        let mut map = DRAWABLE_STATE.lock().unwrap();
        map.insert(
            id,
            DrawableState {
                template_name: template.get_name().to_string(),
                indicator_color: Color::default(),
                position: Coord3D::ZERO,
                orientation: 0.0,
                shroud_status_object_id: INVALID_ID,
                beam_start: None,
                beam_end: None,
                beam_width,
                laser_growth_frames: None,
                laser_growth_start_frame: None,
                projectile_stream: None,
                drawable: Some(Arc::clone(&drawable)),
                expiration_frame: None,
            },
        );
        id
    }

    pub fn destroy_drawable(&self, id: u32) {
        let mut map = DRAWABLE_STATE.lock().unwrap();
        let removed_drawable = map
            .remove(&id)
            .and_then(|state| state.drawable)
            .and_then(|drawable| drawable.read().ok().map(|guard| guard.get_object_id()));
        drop(map);

        if let Some(object_id) = removed_drawable {
            self.clear_object_model_draws(object_id);
        }

        let mut tree_map = TERRAIN_TREE_STATE.lock().unwrap();
        let removed = tree_map.remove(&id).is_some();
        drop(tree_map);

        if removed {
            if let Some(hook) = get_terrain_tree_hook() {
                hook(TerrainTreeEvent::Remove(id));
            }
        }
    }

    pub fn set_drawable_indicator_color(&self, id: u32, color: Color) {
        let mut map = DRAWABLE_STATE.lock().unwrap();
        if let Some(state) = map.get_mut(&id) {
            state.indicator_color = color;
            if let Some(drawable) = state.drawable.as_ref() {
                if let Ok(mut guard) = drawable.write() {
                    guard.set_indicator_color(color);
                }
            }
        }
    }

    pub fn set_drawable_position(&self, id: u32, position: &Coord3D) {
        let mut map = DRAWABLE_STATE.lock().unwrap();
        if let Some(state) = map.get_mut(&id) {
            state.position = *position;
            if let Some(drawable) = state.drawable.as_ref() {
                if let Ok(mut guard) = drawable.write() {
                    guard.set_transform(Matrix3D::from_translation(*position));
                }
            }
        }
    }

    pub fn set_drawable_orientation(&self, id: u32, orientation: Real) {
        let mut map = DRAWABLE_STATE.lock().unwrap();
        if let Some(state) = map.get_mut(&id) {
            state.orientation = orientation;
            if let Some(drawable) = state.drawable.as_ref() {
                if let Ok(mut guard) = drawable.write() {
                    let translation = guard.get_position();
                    let rotation = glam::Quat::from_rotation_z(orientation);
                    let transform = Matrix3D::from_scale_rotation_translation(
                        glam::Vec3::ONE,
                        rotation,
                        glam::Vec3::new(translation.x, translation.y, translation.z),
                    );
                    guard.set_transform(transform);
                }
            }
        }
    }

    pub fn set_drawable_hidden(&self, id: u32, hidden: bool) {
        let mut map = DRAWABLE_STATE.lock().unwrap();
        if let Some(state) = map.get_mut(&id) {
            if let Some(drawable) = state.drawable.as_ref() {
                if let Ok(mut guard) = drawable.write() {
                    let _ = guard.set_drawable_hidden(hidden);
                }
            }
        }
    }

    pub fn set_drawable_shroud_status_object_id(&self, id: u32, object_id: ObjectID) {
        let mut map = DRAWABLE_STATE.lock().unwrap();
        if let Some(state) = map.get_mut(&id) {
            state.shroud_status_object_id = object_id;
        }
    }

    pub fn set_drawable_beam(&self, id: u32, start: &Coord3D, end: &Coord3D) {
        let mut map = DRAWABLE_STATE.lock().unwrap();
        if let Some(state) = map.get_mut(&id) {
            state.beam_start = Some(*start);
            state.beam_end = Some(*end);
        }
    }

    /// C++ `LaserUpdate::initLaser` on the drawable ClientUpdate module.
    pub fn init_drawable_laser(
        &self,
        id: u32,
        start: &Coord3D,
        end: &Coord3D,
        growth_frames: i32,
    ) {
        {
            let mut map = DRAWABLE_STATE.lock().unwrap();
            if let Some(state) = map.get_mut(&id) {
                state.beam_start = Some(*start);
                state.beam_end = Some(*end);
                if growth_frames != 0 {
                    state.laser_growth_frames = Some(growth_frames);
                    state.laser_growth_start_frame = Some(TheGameLogic::get_frame());
                }
            }
        }

        let Some(drawable) = self.get_drawable_arc(id) else {
            return;
        };
        let Ok(guard) = drawable.read() else {
            return;
        };
        for module in guard.modules() {
            module.with_module(|module| {
                if let Some(laser) = module.get_laser_update_interface() {
                    laser.init_laser(
                        None,
                        None,
                        Some(start.to_array()),
                        Some(end.to_array()),
                        String::new(),
                        growth_frames,
                    );
                }
            });
        }
    }

    /// C++ `LaserUpdate::getCurrentLaserRadius`.
    pub fn get_current_laser_radius(&self, id: u32) -> Option<Real> {
        let (template_width, growth_frames, growth_start) = {
            let map = DRAWABLE_STATE.lock().ok()?;
            let state = map.get(&id)?;
            (
                state.beam_width.unwrap_or(1.0),
                state.laser_growth_frames,
                state.laser_growth_start_frame,
            )
        };

        if let Some(drawable) = self.get_drawable_arc(id) {
            if let Ok(guard) = drawable.read() {
                for module in guard.modules() {
                    let mut scale = None;
                    module.with_module(|module| {
                        if let Some(client_update) = module.get_client_update_interface() {
                            client_update.client_update();
                        }
                        if let Some(laser) = module.get_laser_update_interface() {
                            scale = Some(laser.get_width_scale());
                        }
                    });
                    if let Some(scale) = scale {
                        return Some(template_width * scale);
                    }
                }
            }
        }

        Some(template_width * laser_width_scalar(growth_frames, growth_start))
    }

    pub fn set_drawable_projectile_stream(
        &self,
        id: u32,
        lines: Vec<Vec<Coord3D>>,
        texture_name: AsciiString,
        width: Real,
        tile_factor: Real,
        scroll_rate: Real,
    ) {
        let mut map = DRAWABLE_STATE.lock().unwrap();
        if let Some(state) = map.get_mut(&id) {
            state.projectile_stream = Some(ProjectileStreamState {
                lines,
                texture_name,
                width,
                tile_factor,
                scroll_rate,
            });
        }
    }

    pub fn get_drawable_projectile_stream(&self, id: u32) -> Option<ProjectileStreamState> {
        let map = DRAWABLE_STATE.lock().ok()?;
        map.get(&id)
            .and_then(|state| state.projectile_stream.clone())
    }

    /// Begin a new C++ `Drawable::draw()` bridge frame for one bound object.
    ///
    /// This clears the previous committed records before the hidden early-out
    /// in `Drawable::draw`, so stale model output cannot survive a hidden,
    /// stealth, or shroud-suppressed frame.
    pub fn begin_object_model_draw_frame(&self, object_id: ObjectID) {
        if object_id == INVALID_ID {
            return;
        }

        let mut frames = OBJECT_MODEL_DRAW_FRAMES.lock().unwrap();
        let frame = frames.entry(object_id).or_default();
        frame.active_source = None;
        frame.active = None;
        frame.committed.clear();
    }

    /// Start one actual DRAW-interface module invocation.  The outer drawable
    /// commits its active record only after that module has returned, allowing
    /// Truck/Police subclasses to refine their own base W3D result without
    /// overwriting an earlier module in the same frame.
    pub fn begin_active_object_model_draw(
        &self,
        object_id: ObjectID,
        source: ModelDrawSourceIdentity,
    ) {
        if object_id == INVALID_ID {
            return;
        }

        let mut frames = OBJECT_MODEL_DRAW_FRAMES.lock().unwrap();
        let frame = frames.entry(object_id).or_default();
        frame.active_source = Some(source);
        frame.active = None;
    }

    /// Replace the active result of the current draw-module invocation.
    /// Results without an enclosing `begin_active_object_model_draw` are
    /// discarded rather than creating an unbound, guessed association.
    pub fn set_active_object_model_draw(&self, object_id: ObjectID, model_draw: ModelDrawState) {
        if object_id == INVALID_ID {
            return;
        }

        let mut frames = OBJECT_MODEL_DRAW_FRAMES.lock().unwrap();
        let Some(frame) = frames.get_mut(&object_id) else {
            return;
        };
        if frame.active_source.is_some() {
            frame.active = Some(model_draw);
        }
    }

    /// Mutate only the active result of the current draw-module invocation.
    /// In particular, this never exposes previously committed module output to
    /// a wrapper whose base module did not publish a result.
    pub fn with_active_object_model_draw<R>(
        &self,
        object_id: ObjectID,
        func: impl FnOnce(&mut ModelDrawState) -> R,
    ) -> Option<R> {
        if object_id == INVALID_ID {
            return None;
        }

        let mut frames = OBJECT_MODEL_DRAW_FRAMES.lock().ok()?;
        let frame = frames.get_mut(&object_id)?;
        frame.active.as_mut().map(func)
    }

    pub fn set_object_wheel_info(&self, object_id: ObjectID, info: DrawWheelInfo) {
        if object_id == INVALID_ID {
            return;
        }
        if let Ok(mut map) = OBJECT_WHEEL_INFO.lock() {
            map.insert(object_id, info);
        }
    }

    pub fn get_object_wheel_info(&self, object_id: ObjectID) -> Option<DrawWheelInfo> {
        if object_id == INVALID_ID {
            return None;
        }
        OBJECT_WHEEL_INFO
            .lock()
            .ok()
            .and_then(|map| map.get(&object_id).copied())
    }

    /// Commit the active output after its enclosing draw module completes.
    pub fn commit_active_object_model_draw(&self, object_id: ObjectID, logic_drawable_id: u32) {
        if object_id == INVALID_ID {
            return;
        }

        let mut frames = OBJECT_MODEL_DRAW_FRAMES.lock().unwrap();
        let Some(frame) = frames.get_mut(&object_id) else {
            return;
        };
        let source = frame.active_source.take();
        let active = frame.active.take();
        if let (Some(source), Some(mut active)) = (source, active) {
            active.source = source;
            active.logic_drawable_id = logic_drawable_id;
            frame.committed.push(active);
        }
    }

    /// Read the complete ordered W3D model output for a gameplay object.
    /// The client `DrawableID` map is intentionally not consulted here.
    pub fn object_model_draws(&self, object_id: ObjectID) -> Vec<ModelDrawState> {
        if object_id == INVALID_ID {
            return Vec::new();
        }

        OBJECT_MODEL_DRAW_FRAMES
            .lock()
            .ok()
            .and_then(|frames| frames.get(&object_id).map(|frame| frame.committed.clone()))
            .unwrap_or_default()
    }

    /// Build the exact logic-side input record available to a W3D ghost
    /// snapshot provider for one object.
    ///
    /// This is intentionally a source bridge, not a final
    /// `W3DGhostSnapshotCapture`: the logic side has the current Drawable
    /// transform/geometry and committed W3DModelDraw records, but it does not
    /// own the cloned W3D render object (class, object color/scale, and
    /// per-sub-object transforms). A device adapter must materialize those
    /// fields before registering the final capture hook.
    pub fn object_w3d_ghost_snapshot_source(
        &self,
        object_id: ObjectID,
    ) -> Option<crate::object::w3d_ghost_object::W3DGhostSnapshotCaptureSource> {
        if object_id == INVALID_ID {
            return None;
        }

        let object = TheGameLogic::find_object_by_id(object_id)?;
        let (drawable, geometry, position, angle) = {
            let object_guard = object.read().ok()?;
            (
                object_guard.get_drawable()?,
                object_guard.get_geometry_info().clone(),
                *object_guard.get_position(),
                object_guard.get_orientation(),
            )
        };

        let drawable_guard = drawable.read().ok()?;
        let drawable_id = drawable_guard.get_drawable_id();
        let world_scale = drawable_guard.get_world_scale();
        let source = crate::object::w3d_ghost_object::W3DGhostSnapshotCaptureSource {
            object_id,
            drawable_id,
            drawable_effectively_hidden: drawable_guard.is_drawable_effectively_hidden(),
            drawable_transform: crate::object::w3d_ghost_object::Matrix3x4::from_logic_matrix(
                drawable_guard.get_transform_matrix(),
            ),
            drawable_scale: [world_scale.x, world_scale.y, world_scale.z],
            model_draws: self.object_model_draws(object_id),
            geometry: crate::object::w3d_ghost_object::ParentGeometrySnapshot {
                geometry_type: crate::common::types::geometry_type_to_u32(
                    geometry.get_geometry_type(),
                ),
                is_small: geometry.get_is_small(),
                major_radius: geometry.get_major_radius(),
                minor_radius: geometry.get_minor_radius(),
                position: [position.x, position.y, position.z],
                angle,
            },
        };
        Some(source)
    }

    /// Remove all retained bridge state for an object that is being destroyed
    /// or rebound.  This is separate from `destroy_drawable`, because a
    /// DrawableID is not an ObjectID.
    pub fn clear_object_model_draws(&self, object_id: ObjectID) {
        if object_id != INVALID_ID {
            OBJECT_MODEL_DRAW_FRAMES.lock().unwrap().remove(&object_id);
        }
    }

    pub fn find_drawable_by_id(&self, id: u32) -> Option<DrawableState> {
        let map = DRAWABLE_STATE.lock().ok()?;
        map.get(&id).cloned()
    }

    /// Wave 1006: dual-world residual — count of host drawable state entries.
    pub fn drawable_count(&self) -> usize {
        DRAWABLE_STATE.lock().ok().map(|m| m.len()).unwrap_or(0)
    }

    pub fn get_drawable_beam_width(&self, id: u32) -> Option<Real> {
        let map = DRAWABLE_STATE.lock().ok()?;
        map.get(&id).and_then(|state| state.beam_width)
    }

    pub fn get_drawable_arc(&self, id: u32) -> Option<Arc<RwLock<Drawable>>> {
        let map = DRAWABLE_STATE.lock().ok()?;
        map.get(&id)
            .and_then(|state| state.drawable.as_ref().cloned())
    }

    #[cfg(test)]
    pub(crate) fn register_drawable_arc_for_test(&self, id: u32, drawable: Arc<RwLock<Drawable>>) {
        let position = drawable
            .read()
            .ok()
            .map(|guard| guard.get_position())
            .unwrap_or(Coord3D::ZERO);
        let mut map = DRAWABLE_STATE.lock().unwrap();
        map.insert(
            id,
            DrawableState {
                template_name: "TestDrawable".to_string(),
                indicator_color: Color::default(),
                position,
                orientation: 0.0,
                shroud_status_object_id: INVALID_ID,
                beam_start: None,
                beam_end: None,
                beam_width: None,
                laser_growth_frames: None,
                laser_growth_start_frame: None,
                projectile_stream: None,
                drawable: Some(drawable),
                expiration_frame: None,
            },
        );
    }

    /// C++ `W3DTreeDraw::reactToTransformChange` reads `getDrawable()` current
    /// pose (`W3DTreeDraw.cpp:115-127`). Prefer a non-blocking drawable read so
    /// a caller already holding the drawable write lock (set_transform) can
    /// still fall back to the last bridged DRAWABLE_STATE pose.
    pub fn drawable_pose_for_tree(&self, id: u32) -> Option<(Coord3D, Real, Real)> {
        if id == INVALID_ID {
            return None;
        }
        let map = DRAWABLE_STATE.lock().ok()?;
        let state = map.get(&id)?;
        if let Some(drawable) = state.drawable.as_ref() {
            if let Ok(guard) = drawable.try_read() {
                let pos = guard.get_position();
                let transform = guard.get_transform_matrix();
                let (scale, rotation, _) = transform.to_scale_rotation_translation();
                let (_, _, angle) = rotation.to_euler(glam::EulerRot::XYZ);
                let scale = if (guard.get_instance_scale() - 1.0).abs() > f32::EPSILON {
                    guard.get_instance_scale()
                } else {
                    scale.x
                };
                return Some((pos, angle, scale));
            }
        }
        Some((state.position, state.orientation, 1.0))
    }

    #[cfg(test)]
    pub(crate) fn seed_drawable_pose_for_test(
        &self,
        id: u32,
        position: Coord3D,
        orientation: Real,
    ) {
        let mut map = DRAWABLE_STATE.lock().unwrap();
        if let Some(state) = map.get_mut(&id) {
            state.position = position;
            state.orientation = orientation;
            return;
        }
        map.insert(
            id,
            DrawableState {
                template_name: "TestTree".to_string(),
                indicator_color: Color::default(),
                position,
                orientation,
                shroud_status_object_id: INVALID_ID,
                beam_start: None,
                beam_end: None,
                beam_width: None,
                laser_growth_frames: None,
                laser_growth_start_frame: None,
                projectile_stream: None,
                drawable: None,
                expiration_frame: None,
            },
        );
    }

    pub fn set_drawable_expiration_date(&self, id: u32, frame: UnsignedInt) {
        let mut map = DRAWABLE_STATE.lock().unwrap();
        if let Some(state) = map.get_mut(&id) {
            state.expiration_frame = Some(frame);
        }
    }

    pub fn update_drawables(&self, frame: UnsignedInt) {
        let mut map = DRAWABLE_STATE.lock().unwrap();
        let expired: Vec<u32> = map
            .iter()
            .filter_map(|(id, state)| {
                if let Some(expiration) = state.expiration_frame {
                    if frame >= expiration {
                        return Some(*id);
                    }
                }
                None
            })
            .collect();

        for id in expired {
            map.remove(&id);
            let removed_tree = {
                let mut tree_map = TERRAIN_TREE_STATE.lock().unwrap();
                tree_map.remove(&id).is_some()
            };
            if removed_tree {
                if let Some(hook) = get_terrain_tree_hook() {
                    hook(TerrainTreeEvent::Remove(id));
                }
            }
        }
    }

    pub fn add_scorch(&self, _pos: &Coord3D, _size: Real, _scorch_type: i32) {
        if let Some(hook) = get_scorch_hook() {
            hook(_pos, _size, _scorch_type);
        }
    }

    pub fn get_animation_duration_ms(&self, animation_name: &str) -> Option<Real> {
        let hook = get_animation_metadata_hook()?;
        if animation_name.trim().is_empty() {
            return None;
        }
        hook(animation_name)
    }

    pub fn add_tree(
        &self,
        drawable_id: u32,
        location: &Coord3D,
        scale: Real,
        angle: Real,
        random_scale_amount: Real,
        module_data: &W3DTreeDrawModuleData,
    ) {
        if drawable_id == INVALID_ID {
            return;
        }

        let registration = TerrainTreeRegistration {
            drawable_id,
            location: *location,
            scale,
            angle,
            random_scale_amount,
            module_data: module_data.clone(),
        };

        let mut tree_map = TERRAIN_TREE_STATE.lock().unwrap();
        tree_map.insert(drawable_id, registration.clone());
        drop(tree_map);

        if let Some(hook) = get_terrain_tree_hook() {
            hook(TerrainTreeEvent::Add(registration));
        }
    }

    pub fn remove_tree(&self, drawable_id: u32) {
        if drawable_id == INVALID_ID {
            return;
        }

        if let Ok(mut tree_map) = TERRAIN_TREE_STATE.lock() {
            tree_map.remove(&drawable_id);
        }

        if let Some(hook) = get_terrain_tree_hook() {
            hook(TerrainTreeEvent::Remove(drawable_id));
        }
    }

    pub fn get_registered_tree(&self, drawable_id: u32) -> Option<TerrainTreeRegistration> {
        let tree_map = TERRAIN_TREE_STATE.lock().ok()?;
        tree_map.get(&drawable_id).cloned()
    }
}

#[cfg(test)]
mod model_draw_bridge_tests {
    use super::*;

    fn source(ordinal: u32, name: &str) -> ModelDrawSourceIdentity {
        ModelDrawSourceIdentity {
            runtime_draw_ordinal: ordinal,
            module_name: name.to_string(),
            module_tag: format!("{name}Tag"),
            module_tag_name_key: ordinal + 100,
        }
    }

    fn state(model_name: &str) -> ModelDrawState {
        ModelDrawState {
            source: Default::default(),
            logic_drawable_id: 0,
            model_name: model_name.to_string(),
            world_transform: Matrix3D::IDENTITY,
            render_object_scale: Some(1.0),
            render_object_color: Some(0),
            condition_flags_bits: 0,
            bone_overrides: Vec::new(),
            animation_name: None,
            animation_time: 0.0,
            animation_mode: 0,
            mesh_uv_overrides: Vec::new(),
            sub_object_visibility: Vec::new(),
            weapon_bone_bindings: Default::default(),
        }
    }

    #[test]
    fn object_keyed_model_draw_state_never_conflates_drawable_id() {
        let client = TheGameClient::get().expect("game-client bridge");
        let object_id = 9_010_001;
        let unrelated_client_drawable_id = 42;
        client.clear_object_model_draws(object_id);

        client.begin_object_model_draw_frame(object_id);
        client.begin_active_object_model_draw(object_id, source(0, "W3DModelDraw"));
        client.set_active_object_model_draw(object_id, state("BridgeTank"));
        client.commit_active_object_model_draw(object_id, 777);

        let committed = client.object_model_draws(object_id);
        assert_eq!(committed.len(), 1);
        assert_eq!(committed[0].model_name, "BridgeTank");
        assert_eq!(committed[0].logic_drawable_id, 777);
        assert_eq!(committed[0].source.runtime_draw_ordinal, 0);
        assert!(
            DRAWABLE_STATE
                .lock()
                .expect("drawable state")
                .get(&unrelated_client_drawable_id)
                .is_none(),
            "object-keyed W3D output must not manufacture a client-DrawableID entry"
        );

        client.clear_object_model_draws(object_id);
    }

    #[test]
    fn draw_modules_commit_in_order_and_wrappers_only_mutate_active_state() {
        let client = TheGameClient::get().expect("game-client bridge");
        let object_id = 9_010_002;
        client.clear_object_model_draws(object_id);
        client.begin_object_model_draw_frame(object_id);

        client.begin_active_object_model_draw(object_id, source(0, "First"));
        client.set_active_object_model_draw(object_id, state("FirstModel"));
        client.commit_active_object_model_draw(object_id, 501);

        client.begin_active_object_model_draw(object_id, source(1, "Second"));
        client.set_active_object_model_draw(object_id, state("SecondModel"));
        let mutated = client.with_active_object_model_draw(object_id, |active| {
            active.animation_time = 0.75;
        });
        assert_eq!(mutated, Some(()));
        client.commit_active_object_model_draw(object_id, 501);

        let committed = client.object_model_draws(object_id);
        assert_eq!(committed.len(), 2);
        assert_eq!(committed[0].model_name, "FirstModel");
        assert_eq!(committed[0].animation_time, 0.0);
        assert_eq!(committed[1].model_name, "SecondModel");
        assert_eq!(committed[1].animation_time, 0.75);
        assert_eq!(committed[1].source.runtime_draw_ordinal, 1);

        client.clear_object_model_draws(object_id);
    }

    #[test]
    fn beginning_hidden_or_empty_frame_clears_previous_model_output() {
        let client = TheGameClient::get().expect("game-client bridge");
        let object_id = 9_010_003;
        client.clear_object_model_draws(object_id);

        client.begin_object_model_draw_frame(object_id);
        client.begin_active_object_model_draw(object_id, source(0, "Visible"));
        client.set_active_object_model_draw(object_id, state("VisibleModel"));
        client.commit_active_object_model_draw(object_id, 502);
        assert_eq!(client.object_model_draws(object_id).len(), 1);

        // Drawable::draw invokes this before its hidden/stealth/shroud return.
        client.begin_object_model_draw_frame(object_id);
        assert!(client.object_model_draws(object_id).is_empty());

        client.clear_object_model_draws(object_id);
    }

    #[test]
    fn drawable_pose_for_tree_prefers_seeded_bridge_state() {
        // C++ W3DTreeDraw::reactToTransformChange reads getDrawable() pose
        // (W3DTreeDraw.cpp:115-127). The live Rust module looks up this bridge.
        let client = TheGameClient::get().expect("game-client bridge");
        let drawable_id = 0x00EE_7101;
        let location = Coord3D::new(8.0, 9.0, 1.0);
        client.seed_drawable_pose_for_test(drawable_id, location, 1.25);

        let pose = client
            .drawable_pose_for_tree(drawable_id)
            .expect("seeded tree pose");
        assert_eq!(pose.0, location);
        assert_eq!(pose.1, 1.25);
        assert_eq!(pose.2, 1.0);
        assert!(client.drawable_pose_for_tree(INVALID_ID).is_none());
    }
}
