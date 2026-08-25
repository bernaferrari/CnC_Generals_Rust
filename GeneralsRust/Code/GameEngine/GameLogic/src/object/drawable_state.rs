//! Drawable visibility, terrain, shadows, stealth, and ambient-audio state.
//!
//! These methods preserve the C++ state transitions while isolating effects
//! that are orthogonal to construction and animation updates.

use super::*;

impl Drawable {
    pub fn is_selected(&self) -> bool {
        self.is_selected
    }

    pub fn set_terrain_decal(&mut self, decal_type: TerrainDecalType) {
        if self.terrain_decal == decal_type {
            return;
        }
        self.terrain_decal = decal_type;
        // C++ Drawable::setTerrainDecal: only the first draw module gets a decal.
        if let Some(first) = self
            .get_draw_modules_with_interface(ModuleInterfaceType::DRAW)
            .first()
            .cloned()
        {
            first.with_module(|module| set_decal_on_draw_module(module, decal_type));
        }
    }

    pub fn set_terrain_decal_size(&mut self, x: Real, y: Real) {
        for module_handle in self.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
            module_handle.with_module(|module| {
                with_draw_module_mut(module, |draw| draw.set_terrain_decal_size(x, y))
            });
        }
    }

    pub fn set_terrain_decal_fade_target(&mut self, target: Real, rate: Real) {
        if (self.decal_opacity_fade_target - target).abs() > f32::EPSILON {
            self.decal_opacity_fade_target = target;
            self.decal_opacity_fade_rate = rate;
        }
    }

    pub fn get_terrain_decal(&self) -> TerrainDecalType {
        self.terrain_decal
    }

    pub fn get_drawable_status_bits(&self) -> u32 {
        self.drawable_status_bits
    }

    pub fn test_drawable_status(&self, flag: u32) -> bool {
        (self.drawable_status_bits & flag) != 0
    }

    pub fn set_drawable_status(&mut self, flag: u32) {
        self.drawable_status_bits |= flag;
    }

    pub fn clear_drawable_status(&mut self, flag: u32) {
        self.drawable_status_bits &= !flag;
    }

    /// C++ parity: `Drawable::getShadowsEnabled()` — DRAWABLE_STATUS_SHADOWS bit.
    pub fn get_shadows_enabled(&self) -> bool {
        self.test_drawable_status(0x00000002)
    }

    /// C++ parity: `Drawable::setShadowsEnabled(Bool)`.
    ///
    /// Sets DRAWABLE_STATUS_SHADOWS and dispatches to draw modules. Fail-closed:
    /// modules record enable state only — not full shadow mesh GPU allocation.
    pub fn set_shadows_enabled(&mut self, enabled: bool) {
        if enabled {
            self.set_drawable_status(0x00000002); // DRAWABLE_STATUS_SHADOWS
        } else {
            self.clear_drawable_status(0x00000002);
        }
        for module_handle in self.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
            module_handle.with_module(|module| {
                with_draw_module_mut(module, |draw| draw.set_shadows_enabled(enabled))
            });
        }
    }

    /// C++ parity: `Drawable::releaseShadows()` — Options screen resource free.
    ///
    /// Fail-closed residual: notifies draw modules only; does not clear status bits
    /// and does not free GPU meshes that were never allocated.
    pub fn release_shadows(&mut self) {
        for module_handle in self.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
            module_handle
                .with_module(|module| with_draw_module_mut(module, |draw| draw.release_shadows()));
        }
    }

    /// C++ parity: `Drawable::allocateShadows()` — Options screen resource create.
    ///
    /// Fail-closed residual: notifies draw modules only; does not set status bits
    /// and does not allocate full shadow mesh GPU resources.
    pub fn allocate_shadows(&mut self) {
        for module_handle in self.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
            module_handle
                .with_module(|module| with_draw_module_mut(module, |draw| draw.allocate_shadows()));
        }
    }

    pub fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool) {
        if self.drawable_fully_obscured_by_shroud != fully_obscured {
            for module_handle in self.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
                module_handle.with_module(|module| {
                    with_draw_module_mut(module, |draw| {
                        draw.set_fully_obscured_by_shroud(fully_obscured)
                    })
                });
            }
            self.drawable_fully_obscured_by_shroud = fully_obscured;
        }
    }

    /// C++ `Drawable::getFullyObscuredByShroud`.
    pub fn fully_obscured_by_shroud(&self) -> bool {
        self.drawable_fully_obscured_by_shroud
    }

    /// Mirror C++ Drawable::changedTeam.
    pub fn changed_team(&mut self, object: &crate::object::Object) {
        let time_of_day = TheGlobalData::get()
            .map(|data| data.get_time_of_day())
            .unwrap_or(TimeOfDay::Day);
        let indicator = match time_of_day {
            TimeOfDay::Night => object.get_night_indicator_color(),
            _ => object.get_indicator_color(),
        };
        self.set_indicator_color(indicator);

        if object.is_kind_of(KindOf::FSFake) {
            let relationship = ThePlayerList()
                .read()
                .ok()
                .and_then(|list| list.get_local_player().cloned())
                .and_then(|player| {
                    let guard = player.read().ok()?;
                    let team = object.get_team()?;
                    let team_guard = team.read().ok()?;
                    Some(guard.get_relationship_with_team(&team_guard))
                })
                .unwrap_or(Relationship::Enemies);

            if matches!(relationship, Relationship::Allies | Relationship::Neutral) {
                self.set_terrain_decal(TerrainDecalType::ShadowTexture);
            } else {
                self.set_terrain_decal(TerrainDecalType::None);
            }
        }
    }

    pub fn enable_ambient_sound_from_script(&mut self, enabled: Bool) {
        self.ambient_sound_enabled_from_script = enabled;
        if !enabled {
            self.stop_ambient_sound();
        } else if self.ambient_sound_enabled {
            if let Some(object) = self.object_ref.as_ref().and_then(|weak| weak.upgrade()) {
                if let Ok(obj_guard) = object.read() {
                    let time_of_day = TheGlobalData::get()
                        .map(|data| data.get_time_of_day())
                        .unwrap_or(TimeOfDay::Day);
                    self.start_ambient_sound(&obj_guard, time_of_day);
                }
            }
        }
    }

    pub fn enable_ambient_sound(&mut self, enabled: Bool) {
        if self.ambient_sound_enabled == enabled {
            return;
        }
        self.ambient_sound_enabled = enabled;
        if enabled {
            if self.ambient_sound_enabled_from_script {
                if let Some(object) = self.object_ref.as_ref().and_then(|weak| weak.upgrade()) {
                    if let Ok(obj_guard) = object.read() {
                        let time_of_day = TheGlobalData::get()
                            .map(|data| data.get_time_of_day())
                            .unwrap_or(TimeOfDay::Day);
                        self.start_ambient_sound(&obj_guard, time_of_day);
                    }
                }
            }
        } else {
            self.stop_ambient_sound();
        }
    }

    pub fn is_ambient_sound_enabled(&self) -> Bool {
        self.ambient_sound_enabled
    }

    pub fn is_ambient_sound_enabled_from_script(&self) -> Bool {
        self.ambient_sound_enabled_from_script
    }

    pub fn is_ambient_sound_enabled_effective(&self) -> Bool {
        self.ambient_sound_enabled && self.ambient_sound_enabled_from_script
    }

    pub(super) fn mangle_custom_audio_name(&self, base_name: &str) -> String {
        // C++ parity: leading space avoids colliding with INI-defined names.
        format!(" CUSTOM {} {}", self.drawable_id, base_name)
    }

    pub(super) fn set_custom_sound_ambient_dynamic_info_internal(
        &mut self,
        custom_info: DynamicAudioEventInfo,
        restart_sound: bool,
    ) {
        self.clear_custom_sound_ambient(false);

        let info_name = custom_info.audio_event_info.audio_name.clone();
        let info_copy = custom_info.audio_event_info.clone();
        let registered_info = {
            let manager =
                get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
            manager.lock().ok().and_then(|mut guard| {
                guard.register_audio_event_info(info_copy.clone());
                guard.find_audio_event_info(&info_name)
            })
        };

        self.custom_sound_ambient_off = false;
        self.custom_sound_ambient_dynamic_info = Some(custom_info);
        self.custom_sound_ambient_info = registered_info.or_else(|| Some(Arc::new(info_copy)));

        if restart_sound && self.is_ambient_sound_enabled_effective() {
            if let Some(object) = self.object_ref.as_ref().and_then(|weak| weak.upgrade()) {
                if let Ok(obj_guard) = object.read() {
                    let time_of_day = TheGlobalData::get()
                        .map(|data| data.get_time_of_day())
                        .unwrap_or(TimeOfDay::Day);
                    self.start_ambient_sound(&obj_guard, time_of_day);
                }
            }
        }
    }

    pub fn set_custom_sound_ambient_dynamic_info(
        &mut self,
        mut custom_info: DynamicAudioEventInfo,
    ) {
        let custom_name = self.mangle_custom_audio_name(&custom_info.audio_event_info.audio_name);
        custom_info.override_audio_name(&custom_name);
        self.set_custom_sound_ambient_dynamic_info_internal(custom_info, true);
    }

    pub fn set_custom_sound_ambient_off(&mut self) {
        self.clear_custom_sound_ambient(false);
        self.custom_sound_ambient_off = true;
    }

    pub fn get_custom_sound_ambient_dynamic_info(&self) -> Option<&DynamicAudioEventInfo> {
        self.custom_sound_ambient_dynamic_info.as_ref()
    }

    pub fn is_custom_sound_ambient_off(&self) -> Bool {
        self.custom_sound_ambient_off
    }

    pub fn set_custom_sound_ambient_info(&mut self, info: Arc<AudioEventInfo>) {
        self.clear_custom_sound_ambient(false);
        self.custom_sound_ambient_off = false;
        self.custom_sound_ambient_info = Some(info);
        self.custom_sound_ambient_dynamic_info = None;
        if self.is_ambient_sound_enabled_effective() {
            if let Some(object) = self.object_ref.as_ref().and_then(|weak| weak.upgrade()) {
                if let Ok(obj_guard) = object.read() {
                    let time_of_day = TheGlobalData::get()
                        .map(|data| data.get_time_of_day())
                        .unwrap_or(TimeOfDay::Day);
                    self.start_ambient_sound(&obj_guard, time_of_day);
                }
            }
        }
    }

    fn is_permanent_ambient_sound(info: &AudioEventInfo) -> bool {
        // C++ AudioEventInfo::isPermanentSound: AC_LOOP && loopCount==0
        (info.control & AC_LOOP) != 0 && info.loop_count == 0
    }

    fn find_or_create_audio_event_info(event_name: &str) -> Option<Arc<AudioEventInfo>> {
        let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
        let manager = manager.lock().ok()?;
        // C++ addAudioEvent / getInfoForAudioEvent never invents a blank definition.
        manager.find_audio_event_info(event_name)
    }

    fn get_ambient_sound_for_damage(
        object: &crate::object::Object,
        damage_state: BodyDamageType,
    ) -> Option<AudioEventRts> {
        let template = object.get_template();
        match damage_state {
            BodyDamageType::Rubble => template.get_sound_ambient_rubble(),
            BodyDamageType::ReallyDamaged => template
                .get_sound_ambient_really_damaged()
                .or_else(|| template.get_sound_ambient()),
            BodyDamageType::Damaged => template
                .get_sound_ambient_damaged()
                .or_else(|| template.get_sound_ambient()),
            _ => template.get_sound_ambient(),
        }
    }

    pub(super) fn start_ambient_sound_internal(
        &mut self,
        object: &crate::object::Object,
        time_of_day: TimeOfDay,
        only_if_permanent: bool,
    ) {
        if !self.is_ambient_sound_enabled_effective() {
            self.stop_ambient_sound();
            return;
        }

        let damage_state = object
            .get_body_module()
            .and_then(|body| body.lock().ok().map(|guard| guard.get_damage_state()))
            .unwrap_or(BodyDamageType::Pristine);

        if self.custom_sound_ambient_off && damage_state != BodyDamageType::Rubble {
            self.stop_ambient_sound();
            return;
        }

        let (event_name, event_info) = if damage_state != BodyDamageType::Rubble {
            if let Some(custom_info) = &self.custom_sound_ambient_info {
                (
                    custom_info.audio_name.clone(),
                    Some(Arc::clone(custom_info)),
                )
            } else {
                let Some(event) = Self::get_ambient_sound_for_damage(object, damage_state) else {
                    self.stop_ambient_sound();
                    return;
                };
                let name = event.get_event_name().to_string();
                let info = if name.is_empty() {
                    None
                } else {
                    Self::find_or_create_audio_event_info(&name)
                };
                (name, info)
            }
        } else {
            let Some(event) = Self::get_ambient_sound_for_damage(object, damage_state) else {
                self.stop_ambient_sound();
                return;
            };
            let name = event.get_event_name().to_string();
            let info = if name.is_empty() {
                None
            } else {
                Self::find_or_create_audio_event_info(&name)
            };
            (name, info)
        };

        if event_name.is_empty() {
            self.stop_ambient_sound();
            return;
        }

        let Some(info) = event_info else {
            self.stop_ambient_sound();
            return;
        };

        if only_if_permanent && !Self::is_permanent_ambient_sound(&info) {
            self.stop_ambient_sound();
            return;
        }

        self.stop_ambient_sound();

        let mut audio_event = AudioEventRts::new(event_name);
        audio_event.set_drawable_id(self.drawable_id);
        audio_event.set_object_id(object.get_id());
        audio_event.set_time_of_day(time_of_day);

        if let Some(audio) = TheAudio::get() {
            self.ambient_sound_handle = audio.add_audio_event(&audio_event);
        }
    }

    pub fn clear_custom_sound_ambient(&mut self, restart_sound: bool) {
        self.custom_sound_ambient_info = None;
        self.custom_sound_ambient_dynamic_info = None;
        self.custom_sound_ambient_off = false;
        if restart_sound && self.is_ambient_sound_enabled_effective() {
            if let Some(object) = self.object_ref.as_ref().and_then(|weak| weak.upgrade()) {
                if let Ok(obj_guard) = object.read() {
                    let time_of_day = TheGlobalData::get()
                        .map(|data| data.get_time_of_day())
                        .unwrap_or(TimeOfDay::Day);
                    self.start_ambient_sound(&obj_guard, time_of_day);
                }
            }
        }
    }

    pub fn stop_ambient_sound(&mut self) {
        if self.ambient_sound_handle == 0 {
            return;
        }
        if let Some(audio) = TheAudio::get() {
            audio.remove_audio_event(self.ambient_sound_handle);
        }
        self.ambient_sound_handle = 0;
    }

    pub fn start_ambient_sound(&mut self, object: &crate::object::Object, time_of_day: TimeOfDay) {
        self.start_ambient_sound_internal(object, time_of_day, false);
    }

    /// Set whether the drawable is hidden
    pub fn set_drawable_hidden(&mut self, hidden: bool) -> Result<(), GameError> {
        self.hidden = hidden;
        self.is_visible = !hidden;
        self.update_hidden_status();
        Ok(())
    }

    /// Clear pending drawable dependency state before an explicit draw.
    /// Matches C++ Drawable::notifyDrawableDependencyCleared used by W3DOverlordTankDraw.
    pub fn notify_drawable_dependency_cleared(&mut self) {
        for module_handle in self.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
            module_handle.with_module(|module| {
                with_object_draw_interface_mut(module, |draw| {
                    draw.notify_draw_module_dependency_cleared();
                })
            });
        }
    }

    /// Check if the drawable is effectively hidden (by explicit hide or stealth)
    /// Matches C++ Drawable.h line 305: isDrawableEffectivelyHidden()
    /// Returns true if hidden via setDrawableHidden OR fully stealthed
    pub fn is_drawable_effectively_hidden(&self) -> bool {
        self.hidden || !self.is_visible || self.hidden_by_stealth
    }

    /// Update hidden state on draw modules and selection data.
    pub(super) fn update_hidden_status(&mut self) {
        let hidden = self.hidden || self.hidden_by_stealth;
        if hidden {
            self.set_selected(false);
        }
        for module_handle in self.get_draw_modules_with_interface(ModuleInterfaceType::DRAW) {
            module_handle.with_module(|module| {
                with_draw_module_mut(module, |draw| draw.set_hidden(hidden));
            });
        }
    }

    /// Set a specific model condition state flag
    /// Updates the model conditions by setting the specified flag
    pub fn set_model_condition_state(&mut self, state: ModelConditionFlags) {
        self.model_conditions |= state;
        self.update_conditional_model();
        self.propagate_model_condition_state_to_draw_modules();
    }

    /// Clear a specific model condition state flag
    /// Updates the model conditions by clearing the specified flag
    pub fn clear_model_condition_state(&mut self, state: ModelConditionFlags) {
        self.model_conditions &= !state;
        self.update_conditional_model();
        self.propagate_model_condition_state_to_draw_modules();
    }

    /// Clear one set of flags and set another atomically
    /// This is used to transition between states cleanly
    pub fn clear_and_set_model_condition_state(
        &mut self,
        clear: ModelConditionFlags,
        set: ModelConditionFlags,
    ) {
        self.model_conditions &= !clear;
        self.model_conditions |= set;
        self.update_conditional_model();
        self.propagate_model_condition_state_to_draw_modules();
    }

    /// Current model-condition bitset (C++ `getModelConditionFlags` / condition query path).
    pub fn get_model_conditions(&self) -> ModelConditionFlags {
        self.model_conditions
    }

    /// C++ parity: `Drawable::reactToBodyDamageStateChange` (Drawable.cpp:1077-1101).
    ///
    /// Maps body damage state onto model condition bits used by W3D draw modules
    /// (DAMAGED / REALLYDAMAGED / RUBBLE). Pristine clears all three.
    ///
    /// Fail-closed residual: this does **not** claim full animation/mesh swap parity —
    /// only the condition bit update (+ ambient restart when not loading a map).
    pub fn react_to_body_damage_state_change(&mut self, new_state: BodyDamageType) {
        // C++ TheDamageMap[BODYDAMAGETYPE_COUNT]: INVALID, DAMAGED, REALLY_DAMAGED, RUBBLE
        let clear = ModelConditionFlags::DAMAGED
            | ModelConditionFlags::REALLYDAMAGED
            | ModelConditionFlags::RUBBLE;
        let set = match new_state {
            BodyDamageType::Pristine => ModelConditionFlags::empty(),
            BodyDamageType::Damaged => ModelConditionFlags::DAMAGED,
            BodyDamageType::ReallyDamaged => ModelConditionFlags::REALLYDAMAGED,
            BodyDamageType::Rubble => ModelConditionFlags::RUBBLE,
        };
        self.clear_and_set_model_condition_state(clear, set);

        // C++: when loading map, ambient sound is deferred to onLevelStart so customizations apply.
        if !TheGameLogic::is_loading_map() {
            if let Some(object) = self.object_ref.as_ref().and_then(|weak| weak.upgrade()) {
                if let Ok(obj_guard) = object.read() {
                    let time_of_day = TheGlobalData::get()
                        .map(|data| data.get_time_of_day())
                        .unwrap_or(TimeOfDay::Day);
                    self.start_ambient_sound(&obj_guard, time_of_day);
                }
            }
        }
    }
}
