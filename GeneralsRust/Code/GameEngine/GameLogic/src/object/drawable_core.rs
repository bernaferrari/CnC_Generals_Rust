//! Drawable construction, module registration, and core tint/module state.
//!
//! The implementation remains on the canonical `Drawable` type so callers
//! retain the original API while the behavior is organized by responsibility.

use super::*;

impl Drawable {
    /// Create a new Drawable
    pub fn new(
        drawable_id: DrawableID,
        object_id: ObjectID,
        model_name: String,
        drawable_type: DrawableType,
    ) -> Self {
        Drawable {
            drawable_id: normalize_drawable_id(drawable_id),
            object_id,
            object_ref: None,
            drawable_type,

            transform: Matrix3D::IDENTITY,
            instance_matrix: None,
            instance_scale: 1.0,
            world_position: Coord3D::new(0.0, 0.0, 0.0),
            world_rotation: Coord3D::new(0.0, 0.0, 0.0),
            world_scale: Coord3D::new(1.0, 1.0, 1.0),

            is_visible: true,
            hidden: false,
            hidden_by_stealth: false,
            always_visible: false,
            frustum_culled: false,
            occlusion_culled: false,
            distance_culled: false,
            current_lod: LevelOfDetail::High,
            lod_distances: [50.0, 100.0, 200.0, 400.0],

            model_name,
            submesh_names: Vec::new(),
            materials: Vec::new(),
            bounding_box: BoundingBox {
                min: Coord3D::new(-1.0, -1.0, -1.0),
                max: Coord3D::new(1.0, 1.0, 1.0),
            },
            bounding_sphere: BoundingSphere {
                center: Coord3D::new(0.0, 0.0, 0.0),
                radius: 1.0,
            },

            skeleton: Vec::new(),
            animation_clips: HashMap::new(),
            current_animation: None,
            animation_time: 0.0,
            animation_speed: 1.0,
            animation_state: AnimationState::Idle,
            blend_animations: Vec::new(),
            bone_transforms: Vec::new(),
            swaying_enabled: true,

            model_conditions: ModelConditionFlags::empty(),
            conditional_models: HashMap::new(),

            render_flags: RenderFlags::CAST_SHADOW | RenderFlags::RECEIVE_SHADOW,
            draw_priority: 0,
            alpha: 1.0,
            color_tint: Color::white(),
            indicator_color: Color::black(),
            selection_flash_envelope: None,
            color_tint_envelope: None,
            drawable_status_bits: 0x00000002, // DRAWABLE_STATUS_SHADOWS
            tint_status: TintStatus::NONE,
            prev_tint_status: TintStatus::NONE,
            fade_mode: 0,
            time_elapsed_fade: 0,
            time_to_fade: 0,
            loco_info: None,
            flash_count: 0,
            flash_color: Color::white(),
            shroud_status_object_id: object_id,
            expiration_date: 0,
            legacy_icons: Vec::new(),

            receives_lighting: true,
            casts_shadows: true,
            receives_shadows: true,
            self_illuminated: 0.0,

            particle_systems: Vec::new(),
            attachments: HashMap::new(),
            damage_states: Vec::new(),
            current_damage_state: 0,

            is_selected: false,
            selection_circle: None,
            health_bar: None,
            terrain_decal: TerrainDecalType::None,
            decal_opacity: 0.0,
            decal_opacity_fade_target: 0.0,
            decal_opacity_fade_rate: 0.0,
            drawable_fully_obscured_by_shroud: false,

            active_effects: Vec::new(),
            timed_effects: Vec::new(),

            modules: Vec::new(),

            last_update_frame: 0,
            update_frequency: 1,
            frozen: false,

            stealth_factor: 1.0,
            effective_stealth_opacity: 1.0,
            stealth_look: StealthLookType::None,
            second_material_pass_opacity: 0.0,
            cloak_texture: None,
            distortion_amount: 0.0,

            weather_affected: true,
            wetness_factor: 0.0,
            snow_accumulation: 0.0,

            attached_sounds: Vec::new(),
            ambient_sound_handle: 0,
            ambient_sound_enabled: true,
            ambient_sound_enabled_from_script: true,
            custom_sound_ambient_off: false,
            custom_sound_ambient_info: None,
            custom_sound_ambient_dynamic_info: None,

            terrain_following: false,
            ground_offset: 0.0,
            slope_adaptation: 0.0,

            screen_effects: Vec::new(),
        }
    }

    /// Allocate a drawable ID with save/load counter parity when GameClient hooks exist.
    pub fn allocate_drawable_id() -> DrawableID {
        if let Some(counter) = get_runtime_drawable_id_counter() {
            let id = normalize_drawable_id(counter);
            let next = next_drawable_id_value(id);
            set_runtime_drawable_id_counter(next);
            LOCAL_NEXT_DRAWABLE_ID.store(next, Ordering::Relaxed);
            return id;
        }

        allocate_local_drawable_id()
    }

    /// Get the next drawable-id counter value.
    pub fn get_drawable_id_counter() -> DrawableID {
        if let Some(counter) = get_runtime_drawable_id_counter() {
            let normalized = normalize_drawable_id(counter);
            LOCAL_NEXT_DRAWABLE_ID.store(normalized, Ordering::Relaxed);
            return normalized;
        }
        normalize_drawable_id(LOCAL_NEXT_DRAWABLE_ID.load(Ordering::Relaxed))
    }

    /// Set the next drawable-id counter value.
    pub fn set_drawable_id_counter(next_drawable_id: DrawableID) {
        let normalized = normalize_drawable_id(next_drawable_id);
        LOCAL_NEXT_DRAWABLE_ID.store(normalized, Ordering::Relaxed);
        set_runtime_drawable_id_counter(normalized);
    }

    pub fn get_drawable_id(&self) -> DrawableID {
        self.drawable_id
    }

    pub fn set_drawable_id(&mut self, drawable_id: DrawableID) {
        self.drawable_id = normalize_drawable_id(drawable_id);
    }

    pub fn get_object_id(&self) -> ObjectID {
        self.object_id
    }

    /// Register a draw module instance with the drawable.
    pub fn add_module(
        &mut self,
        interface_mask: ModuleInterfaceType,
        name: AsciiString,
        tag: AsciiString,
        module_data: Arc<dyn ModuleData>,
        mut module: Box<dyn Module>,
    ) -> DrawableModuleHandle {
        // C++ constructs draw modules before `friend_bindToObject`.  Do not
        // bind them to INVALID_ID or notify them during construction; W3D
        // modules resolve their owning Object only after the real association
        // exists.  A module dynamically added to an already bound Drawable is
        // the one legitimate immediate-notification case.
        let is_bound = self
            .object_ref
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .is_some();
        if is_bound {
            let _ = with_draw_module_kind(module.as_mut(), |draw| {
                draw.bind_owner_id(self.object_id);
            });
            module.on_drawable_bound_to_object();
        }
        let entry = Arc::new(DrawModuleEntry::new(
            name,
            tag,
            interface_mask,
            module_data,
            module,
        ));
        self.modules.push(Arc::clone(&entry));
        DrawableModuleHandle::new(entry)
    }

    /// C++ `Drawable::getDrawModules()` — DRAW-interface modules (possibly empty).
    pub fn draw_modules(&self) -> Vec<DrawableModuleHandle> {
        self.get_draw_modules_with_interface(ModuleInterfaceType::DRAW)
    }

    /// Iterate every registered DRAW module, matching C++ `getDrawModules()` walks.
    pub fn for_each_draw_module_mut<F>(&self, mut func: F)
    where
        F: FnMut(&mut dyn DrawModule),
    {
        for module_handle in self.draw_modules() {
            module_handle.with_module(|module| {
                with_draw_module_mut(module, |draw| func(draw));
            });
        }
    }

    /// Iterate draw modules that expose `DebrisDrawInterface`.
    ///
    /// Mirrors C++:
    /// `for (DrawModule** dm = getDrawModules(); *dm; ++dm) if (DebrisDrawInterface* di = (*dm)->getDebrisDrawInterface())`
    pub fn for_each_debris_draw_interface<F>(&self, mut func: F)
    where
        F: FnMut(&mut dyn DebrisDrawInterface),
    {
        for module_handle in self.draw_modules() {
            let _ = module_handle.with_debris_draw_interface(|di| func(di));
        }
    }

    /// Attach a `W3DDebrisDraw` module so OCL/tests can set model/anim names.
    pub fn attach_w3d_debris_draw(&mut self) -> DrawableModuleHandle {
        let data = W3DDebrisDrawModuleData::new();
        self.add_module(
            ModuleInterfaceType::DRAW,
            AsciiString::from("W3DDebrisDraw"),
            AsciiString::from("W3DDebrisDraw"),
            Arc::new(W3DDebrisDrawModuleData::new()),
            Box::new(W3DDebrisDraw::new(data)),
        )
    }

    /// Apply OCL debris model/anim configuration (C++ `doStuffToObj` debris draw walk).
    pub fn apply_debris_draw(
        &mut self,
        model: &str,
        color: i32,
        shadow: u32,
        anims: Option<DebrisDrawAnims<'_>>,
    ) -> usize {
        let color = packed_color_from_i32(color);
        let shadow_type = shadow_type_from_bits(shadow);
        let mut applied = 0usize;
        self.for_each_debris_draw_interface(|di| {
            di.set_model_name(AsciiString::from(model), color, shadow_type);
            if let Some(anims) = anims {
                di.set_anim_names(
                    AsciiString::from(anims.initial),
                    AsciiString::from(anims.flying),
                    AsciiString::from(anims.final_anim),
                    anims.final_fx,
                );
            }
            applied += 1;
        });
        applied
    }

    /// Enable or disable sway effects (used by topple logic).
    pub fn set_swaying_enabled(&mut self, enabled: bool) {
        self.swaying_enabled = enabled;
    }

    /// Set tint status bit(s) on this drawable.
    pub fn set_tint_status(&mut self, status_bits: TintStatus) {
        self.tint_status.set(status_bits);
    }

    /// Replace the current tint status with an exact value.
    pub fn set_tint_status_exact(&mut self, status: TintStatus) {
        self.tint_status = status;
    }

    /// Get current tint status bitmask.
    pub fn get_tint_status(&self) -> TintStatus {
        self.tint_status
    }

    /// Get current color tint.
    pub fn get_tint_color(&self) -> Color {
        self.color_tint
    }

    /// Set color tint explicitly.
    pub fn set_color_tint(&mut self, color: Color) {
        self.color_tint = color;
    }

    /// Clear color tint back to default (white).
    pub fn clear_color_tint(&mut self) {
        self.color_tint = Color::white();
    }

    /// C++ `Drawable::setColorTintEnvelope` — copy the full envelope curve.
    pub fn copy_color_tint_envelope_from(&mut self, other: &Drawable) {
        self.color_tint_envelope = other.color_tint_envelope.clone();
        self.color_tint = other.color_tint;
        self.tint_status = other.tint_status;
    }

    /// C++ `Drawable::getShouldAnimate(considerPower)`.
    pub fn get_should_animate(&self, consider_power: bool) -> bool {
        let Some(object) = TheGameLogic::find_object_by_id(self.object_id) else {
            return true;
        };
        let Ok(obj) = object.read() else {
            return true;
        };
        object_should_animate(&obj, consider_power)
    }

    /// C++ Drawable::setInstanceScale — pulse drawables grow via this, not identity snap.
    pub fn set_instance_scale(&mut self, scale: Real) {
        self.instance_scale = scale;
    }

    pub fn get_instance_scale(&self) -> Real {
        self.instance_scale
    }

    /// C++ Drawable::colorTint — lock a saturated tint on the pulse drawable.
    pub fn color_tint(&mut self, color: Option<Color>) {
        const TINT_COLOR_LOCKED: u32 = 0x00000004;
        if let Some(color) = color {
            self.color_flash(Some(color), 0, 0, true);
            self.set_drawable_status(TINT_COLOR_LOCKED);
            self.color_tint = color;
        } else {
            if self.color_tint_envelope.is_none() {
                self.color_tint_envelope = Some(LegacyTintEnvelope::default());
            }
            if let Some(envelope) = self.color_tint_envelope.as_mut() {
                envelope.rest();
            }
            self.clear_drawable_status(TINT_COLOR_LOCKED);
            self.color_tint = Color::white();
        }
    }

    /// C++ Drawable::colorFlash(color, decayFrames, attackFrames, sustainAtPeak).
    pub fn color_flash(
        &mut self,
        color: Option<Color>,
        decay_frames: u32,
        attack_frames: u32,
        sustain_at_peak: bool,
    ) {
        const TINT_COLOR_LOCKED: u32 = 0x00000004;
        if self.color_tint_envelope.is_none() {
            self.color_tint_envelope = Some(LegacyTintEnvelope::default());
        }
        let rgb = color.unwrap_or(Color::white());
        let peak = Coord3D::new(
            rgb.r as f32 / 255.0,
            rgb.g as f32 / 255.0,
            rgb.b as f32 / 255.0,
        );
        if let Some(envelope) = self.color_tint_envelope.as_mut() {
            envelope.play(peak, attack_frames, decay_frames, sustain_at_peak);
        }
        self.flash_color = rgb;
        self.flash_count = if decay_frames == 0 {
            0
        } else {
            decay_frames as i32
        };
        self.clear_drawable_status(TINT_COLOR_LOCKED);
    }

    /// C++ `Drawable::setFlashColor`.
    pub fn set_flash_color(&mut self, color: Color) {
        self.flash_color = color;
    }

    /// C++ `Drawable::setFlashCount`.
    pub fn set_flash_count(&mut self, count: i32) {
        self.flash_count = count.max(0);
    }

    pub fn get_flash_color(&self) -> Color {
        self.flash_color
    }

    pub fn get_flash_count(&self) -> i32 {
        self.flash_count
    }

    pub fn set_time_of_day(&mut self, time_of_day: TimeOfDay) {
        match time_of_day {
            TimeOfDay::Night => self.set_model_condition_state(ModelConditionFlags::NIGHT),
            _ => self.clear_model_condition_state(ModelConditionFlags::NIGHT),
        }

        if let Some(object) = self.object_ref.as_ref().and_then(|weak| weak.upgrade()) {
            if let Ok(obj_guard) = object.read() {
                self.start_ambient_sound(&obj_guard, time_of_day);
            }
        }
    }

    pub fn set_indicator_color(&mut self, color: Color) {
        self.indicator_color = color;
        let packed = ((color.r as i32) << 16) | ((color.g as i32) << 8) | (color.b as i32);
        self.for_each_draw_module_mut(|draw| {
            if let Some(interface) = draw.get_object_draw_interface_mut() {
                interface.replace_indicator_color(packed);
            }
        });
    }

    pub fn get_indicator_color(&self) -> Color {
        self.indicator_color
    }

    /// Clear tint status bit(s) on this drawable.
    pub fn clear_tint_status(&mut self, status_bits: TintStatus) {
        self.tint_status.clear(status_bits);
    }

    /// Test tint status bit(s).
    pub fn test_tint_status(&self, status_bits: TintStatus) -> bool {
        self.tint_status.is_set(status_bits)
    }

    /// Return draw modules that advertise the requested interface.
    pub fn modules_with_interface(
        &self,
        interface: ModuleInterfaceType,
    ) -> Vec<DrawableModuleHandle> {
        self.modules
            .iter()
            .filter(|entry| (entry.mask().0 & interface.0) != 0)
            .map(|entry| DrawableModuleHandle::new(Arc::clone(entry)))
            .collect()
    }

    pub(super) fn get_draw_modules_with_interface(
        &self,
        interface: ModuleInterfaceType,
    ) -> Vec<DrawableModuleHandle> {
        self.modules_with_interface(interface)
    }

    pub(super) fn xfer_drawable_modules(&mut self, xfer: &mut dyn Xfer) {
        let current_version: u8 = 1;
        let mut version = current_version;
        let _ = xfer.xfer_version(&mut version, current_version);

        let xfer_mode = xfer.get_xfer_mode();
        let is_saving = matches!(
            xfer_mode,
            game_engine::system::XferMode::Save | game_engine::system::XferMode::Crc
        );

        let mut module_types = 2u16;
        let _ = xfer.xfer_unsigned_short(&mut module_types);

        for module_type in 0..module_types {
            let interface = match module_type {
                0 => ModuleInterfaceType::DRAW,
                1 => ModuleInterfaceType::CLIENT_UPDATE,
                _ => ModuleInterfaceType::NONE,
            };

            if interface == ModuleInterfaceType::NONE {
                warn!(
                    "Drawable::xfer_drawable_modules encountered unsupported module type bucket {} on drawable {}",
                    module_type, self.drawable_id
                );
                let mut unknown_count = 0u16;
                let _ = xfer.xfer_unsigned_short(&mut unknown_count);
                for _ in 0..unknown_count {
                    let mut ignored = String::new();
                    let _ = xfer.xfer_ascii_string(&mut ignored);
                    let block_size = xfer.begin_block().unwrap_or(0);
                    if block_size > 0 {
                        let _ = xfer.skip(block_size);
                    }
                    let _ = xfer.end_block();
                }
                continue;
            }

            if is_saving {
                let modules_for_type: Vec<&Arc<DrawModuleEntry>> = self
                    .modules
                    .iter()
                    .filter(|entry| (entry.mask().0 & interface.0) != 0)
                    .collect();

                let mut module_count = modules_for_type.len().min(u16::MAX as usize) as u16;
                let _ = xfer.xfer_unsigned_short(&mut module_count);

                for entry in modules_for_type.into_iter().take(module_count as usize) {
                    let mut module_identifier = entry
                        .with_module(|module| {
                            NameKeyGenerator::key_to_name(module.get_module_tag_name_key())
                        })
                        .unwrap_or_default();
                    if module_identifier.is_empty() {
                        log::warn!(
                            "Drawable::xfer_drawable_modules unresolved module identifier for tag '{}' on drawable {}",
                            entry.tag(),
                            self.drawable_id
                        );
                    }
                    let _ = xfer.xfer_ascii_string(&mut module_identifier);

                    let _ = xfer.begin_block();
                    entry.with_module(|module| {
                        if let Err(err) = module.xfer(xfer) {
                            panic!(
                                "Drawable::xfer_drawable_modules failed for '{}' on drawable {}: {}",
                                module_identifier, self.drawable_id, err
                            );
                        };
                    });
                    let _ = xfer.end_block();
                }
            } else {
                let mut module_count = 0u16;
                let _ = xfer.xfer_unsigned_short(&mut module_count);

                for _ in 0..module_count {
                    let mut module_identifier = String::new();
                    let _ = xfer.xfer_ascii_string(&mut module_identifier);
                    let module_identifier_key = NameKeyGenerator::name_to_key(&module_identifier);

                    let module_index = self.modules.iter().position(|entry| {
                        (entry.mask().0 & interface.0) != 0
                            && entry.with_module(|module| {
                                module.get_module_tag_name_key() == module_identifier_key
                            })
                    });

                    let data_size = xfer.begin_block().unwrap_or(0);
                    if let Some(index) = module_index {
                        let entry = &self.modules[index];
                        entry.with_module(|module| {
                            if let Err(err) = module.xfer(xfer) {
                                panic!(
                                    "Drawable::xfer_drawable_modules load failed for '{}' on drawable {}: {}",
                                    module_identifier, self.drawable_id, err
                                );
                            }
                        });
                    } else if data_size > 0 {
                        // C++ Drawable.cpp:4854-4867 — missing module is a DEBUG_CRASH then skip.
                        log::warn!(
                            "Drawable::xfer_drawable_modules - Module '{}' was indicated in file, but not found on Drawable {}",
                            module_identifier,
                            self.drawable_id
                        );
                        let _ = xfer.skip(data_size);
                    }
                    let _ = xfer.end_block();
                }
            }
        }
    }

    /// Retrieve all registered drawable modules.
    pub fn modules(&self) -> Vec<DrawableModuleHandle> {
        self.modules
            .iter()
            .cloned()
            .map(DrawableModuleHandle::new)
            .collect()
    }

    /// Retrieve a draw module by its logical name.
    pub fn module_by_name(&self, name: &AsciiString) -> Option<DrawableModuleHandle> {
        self.modules
            .iter()
            .find(|entry| entry.name() == name)
            .cloned()
            .map(DrawableModuleHandle::new)
    }

    /// Retrieve a draw module by its tag identifier.
    pub fn module_by_tag(&self, tag: &AsciiString) -> Option<DrawableModuleHandle> {
        self.modules
            .iter()
            .find(|entry| entry.tag() == tag)
            .cloned()
            .map(DrawableModuleHandle::new)
    }

    /// Remove all registered modules, invoking their delete hooks.
    pub fn clear_modules(&mut self) {
        for entry in &self.modules {
            entry.with_module(|module| module.on_delete());
        }
        self.modules.clear();
    }
}
