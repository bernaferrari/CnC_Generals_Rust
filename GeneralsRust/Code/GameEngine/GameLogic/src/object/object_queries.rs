//! Split-out inherent `queries, containment, player/group, stealth helpers` methods for [`Object`].
//!
//! Child of `object` so private `Object` fields remain visible.

#![allow(unused_imports)]

use super::object_impl_imports::*;
use super::*;

impl Object {
    /// AI helper: idle if AI present.
    pub fn ai_idle(&mut self) {
        if let Some(ai) = &self.ai {
            if let Ok(mut guard) = ai.lock() {
                if let Err(err) = guard.ai_idle() {
                    log::debug!("Object::ai_idle failed: {err}");
                }
            }
        }
    }

    pub fn get_template_name(&self) -> &str {
        self.thing_template.get_name().as_str()
    }

    pub fn set_health_box_offset(&mut self, offset: Coord3D) {
        self.health_box_offset = offset;
    }

    pub fn enter_group(&mut self, group: &AIGroup) {
        self.group_id = Some(group.get_id());
    }

    pub fn leave_group(&mut self) {
        self.group_id = None;
    }

    pub fn get_group_id(&self) -> Option<u32> {
        self.group_id
    }

    pub fn get_controlling_player_id(&self) -> Option<UnsignedInt> {
        self.get_team()
            .as_ref()
            .and_then(|team| team.read().ok()?.get_controlling_player_id())
    }

    pub fn get_controlling_player(&self) -> Option<Arc<RwLock<Player>>> {
        let team = self.get_team()?;
        let player_index = team.read().ok()?.get_controlling_player_id()? as Int;
        let list = player_list().read().ok()?;
        list.get_player(player_index).cloned()
    }

    pub fn get_player_id(&self) -> Option<PlayerId> {
        self.get_controlling_player_id()
            .and_then(|raw| PlayerId::new(raw as u8))
    }

    pub fn is_neutral_controlled(&self) -> bool {
        if let Some(player) = self.get_controlling_player() {
            if let Ok(guard) = player.read() {
                return guard.get_player_type() == PlayerType::Neutral;
            }
        }
        false
    }

    pub fn relationship_to(&self, other: &Object) -> Relationship {
        if self.get_id() == other.get_id() {
            return Relationship::Allies;
        }

        if let (Some(my_team), Some(other_team)) = (self.get_team(), other.get_team()) {
            if let (Ok(my_guard), Ok(other_guard)) = (my_team.read(), other_team.read()) {
                if self.is_undetected_defector() {
                    return Relationship::Neutral;
                }
                if other.is_undetected_defector() {
                    return Relationship::Allies;
                }
                return my_guard.get_relationship(&other_guard);
            }
        }

        Relationship::Neutral
    }

    pub fn get_formation_id(&self) -> FormationID {
        self.formation_id
    }

    pub fn set_formation_id(&mut self, id: FormationID) {
        self.formation_id = id;
    }

    pub fn get_formation_offset(&self) -> Coord2D {
        self.formation_offset
    }

    pub fn set_formation_offset(&mut self, offset: Coord2D) {
        self.formation_offset = offset;
    }

    /// Update this object instance with properties from a map object dict.
    /// Mirrors C++ Object::updateObjValuesFromMapProperties.
    pub fn update_obj_values_from_map_properties(&mut self, properties: &Dict) {
        let get_bool = |key| {
            if properties.get_type(key) == Some(DictType::Bool) {
                Some(properties.get_bool(key))
            } else {
                None
            }
        };
        let get_int = |key| {
            if properties.get_type(key) == Some(DictType::Int) {
                Some(properties.get_int(key))
            } else {
                None
            }
        };
        let get_real = |key| {
            if properties.get_type(key) == Some(DictType::Real) {
                Some(properties.get_real(key))
            } else {
                None
            }
        };
        let get_ascii = |key| {
            if properties.get_type(key) == Some(DictType::AsciiString) {
                Some(properties.get_ascii_string(key))
            } else {
                None
            }
        };

        if let Some(name) = get_ascii(crate::common::well_known_keys::key_object_name()) {
            if !name.is_empty() {
                self.set_name(AsciiString::from(name.as_str()));
            }
        }

        if let Some(max_hps) = get_int(crate::common::well_known_keys::key_object_max_hps()) {
            if max_hps >= 0 {
                if let Some(body) = self.get_body_module() {
                    if let Ok(mut guard) = body.lock() {
                        let _ = guard
                            .set_max_health(max_hps as f32, MaxHealthChangeType::PreserveRatio);
                    }
                }
            }
        }

        if let Some(initial_health) =
            get_int(crate::common::well_known_keys::key_object_initial_health())
        {
            if let Some(body) = self.get_body_module() {
                if let Ok(mut guard) = body.lock() {
                    let _ = guard.set_initial_health(initial_health);
                }
            }
        }

        if let Some(veterancy) = get_int(crate::common::well_known_keys::key_object_veterancy()) {
            if let Some(tracker) = self.get_experience_tracker() {
                if let Ok(mut guard) = tracker.lock() {
                    if guard.is_trainable() {
                        let level = match veterancy.clamp(0, 3) {
                            0 => VeterancyLevel::Regular,
                            1 => VeterancyLevel::Veteran,
                            2 => VeterancyLevel::Elite,
                            _ => VeterancyLevel::Heroic,
                        };
                        let _ = guard.set_veterancy_level(level);
                    }
                }
            }
        }

        if let Some(attitude_val) =
            get_int(crate::common::well_known_keys::key_object_aggressiveness())
        {
            if let Some(ai) = self.get_ai_update_interface() {
                if let Ok(mut guard) = ai.lock() {
                    let attitude = match attitude_val {
                        -2 => AIAttitudeType::Sleep,
                        -1 => AIAttitudeType::Passive,
                        1 => AIAttitudeType::Defensive,
                        2 => AIAttitudeType::Aggressive,
                        _ => AIAttitudeType::Normal,
                    };
                    let _ = guard.set_attitude(attitude);
                }
            }
        }

        if let Some(recruitable) =
            get_bool(crate::common::well_known_keys::key_object_recruitable_ai())
        {
            if let Some(ai) = self.get_ai_update_interface() {
                if let Ok(mut guard) = ai.lock() {
                    guard.set_is_recruitable(recruitable);
                }
            }
        }

        if let Some(selectable) = get_bool(crate::common::well_known_keys::key_object_selectable())
        {
            if selectable != self.is_selectable() {
                self.set_selectable(selectable);
            }
        }

        if let Some(stop_dist) =
            get_real(crate::common::well_known_keys::key_object_stopping_distance())
        {
            if stop_dist >= 0.5 {
                if let Some(ai) = self.get_ai_update_interface() {
                    if let Ok(ai_guard) = ai.lock() {
                        if let Some(loco) = ai_guard.get_cur_locomotor() {
                            if let Ok(mut loco_guard) = loco.lock() {
                                loco_guard.set_close_enough_dist(stop_dist);
                            }
                        }
                    }
                }
            }
        }

        if let Some(enabled) = get_bool(crate::common::well_known_keys::key_object_enabled()) {
            self.set_script_status(ObjectScriptStatusBit::ScriptDisabled, !enabled);
        }

        if let Some(powered) = get_bool(crate::common::well_known_keys::key_object_powered()) {
            self.set_script_status(ObjectScriptStatusBit::ScriptUnderpowered, !powered);
        }

        if let Some(indestructible) =
            get_bool(crate::common::well_known_keys::key_object_indestructible())
        {
            if let Some(body) = self.get_body_module() {
                if let Ok(mut guard) = body.lock() {
                    let _ = guard.set_indestructible(indestructible);
                }
            }
        }

        if let Some(unsellable) = get_bool(crate::common::well_known_keys::key_object_unsellable())
        {
            self.set_script_status(ObjectScriptStatusBit::Unsellable, unsellable);
        }

        if let Some(targetable) = get_bool(crate::common::well_known_keys::key_object_targetable())
        {
            self.set_script_status(ObjectScriptStatusBit::ScriptTargetable, targetable);
        }

        if let Some(visual_range) =
            get_int(crate::common::well_known_keys::key_object_visual_range())
        {
            let clamped = (visual_range as Real).max(0.0);
            self.set_vision_range(clamped);
        }

        if let Some(shroud_range) =
            get_int(crate::common::well_known_keys::key_object_shroud_clearing_distance())
        {
            let clamped = (shroud_range as Real).max(0.0);
            self.set_shroud_clearing_range(clamped);
        }

        let base_key_name = "objectGrantUpgrade";
        for upgrade_num in 0.. {
            let key_name = format!("{}{}", base_key_name, upgrade_num);
            let key = NameKeyGenerator::name_to_key(&key_name);
            let Some(upgrade_name) = get_ascii(key) else {
                break;
            };
            if upgrade_name.is_empty() {
                break;
            }

            let center = get_upgrade_center();
            let center_read = center.read();
            if let Ok(guard) = center_read {
                if let Some(template) = guard.find_upgrade(&upgrade_name) {
                    self.give_upgrade(&template);
                }
            }
        }

        if let Some(drawable) = self.get_drawable() {
            if let Ok(mut draw_guard) = drawable.write() {
                if let Some(time_val) = get_int(crate::common::well_known_keys::key_object_time()) {
                    match time_val {
                        1 => draw_guard.clear_model_condition_state(ModelConditionFlags::NIGHT),
                        2 => draw_guard.set_model_condition_state(ModelConditionFlags::NIGHT),
                        _ => {}
                    }
                }

                if let Some(weather_val) =
                    get_int(crate::common::well_known_keys::key_object_weather())
                {
                    match weather_val {
                        1 => draw_guard.clear_model_condition_state(ModelConditionFlags::SNOW),
                        2 => draw_guard.set_model_condition_state(ModelConditionFlags::SNOW),
                        _ => {}
                    }
                }

                let mut sound_enabled_exists = false;
                let mut sound_enabled = false;

                if let Some(enabled) =
                    get_bool(crate::common::well_known_keys::key_object_sound_ambient_enabled())
                {
                    sound_enabled_exists = true;
                    sound_enabled = enabled;
                }

                let mut audio_to_modify: Option<DynamicAudioEventInfo> = None;
                let mut info_modified = false;

                if let Some(sound_name) =
                    get_ascii(crate::common::well_known_keys::key_object_sound_ambient())
                {
                    if sound_name.is_empty() {
                        draw_guard.set_custom_sound_ambient_off();
                        sound_enabled_exists = true;
                        sound_enabled = false;
                    } else {
                        let manager = get_global_audio_manager()
                            .unwrap_or_else(initialize_global_audio_manager);
                        let manager_lock = manager.lock();
                        if let Ok(manager) = manager_lock {
                            if let Some(base_info) = manager.find_audio_event_info(&sound_name) {
                                audio_to_modify =
                                    Some(DynamicAudioEventInfo::from_base_info(&base_info));
                                info_modified = true;
                            }
                        }
                    }
                }

                if !draw_guard.is_custom_sound_ambient_off() {
                    if let Some(true) = get_bool(
                        crate::common::well_known_keys::key_object_sound_ambient_customized(),
                    ) {
                        if audio_to_modify.is_none() {
                            let template = self.get_template();
                            if let Some(base_event) = template.get_sound_ambient() {
                                let manager = get_global_audio_manager()
                                    .unwrap_or_else(initialize_global_audio_manager);
                                let manager_lock = manager.lock();
                                if let Ok(manager) = manager_lock {
                                    if let Some(base_info) =
                                        manager.find_audio_event_info(&base_event.event_name)
                                    {
                                        audio_to_modify =
                                            Some(DynamicAudioEventInfo::from_base_info(&base_info));
                                    }
                                }
                            }
                        }

                        if let Some(ref mut audio_info) = audio_to_modify {
                            if let Some(looping) = get_bool(
                                crate::common::well_known_keys::key_object_sound_ambient_looping(),
                            ) {
                                audio_info.override_loop_flag(looping);
                                info_modified = true;
                            }

                            if let Some(loop_count) = get_int(
                                crate::common::well_known_keys::key_object_sound_ambient_loop_count(
                                ),
                            ) {
                                if (audio_info.audio_event_info.control
                                    & game_engine::common::audio::AC_LOOP)
                                    != 0
                                {
                                    audio_info.override_loop_count(loop_count);
                                    info_modified = true;
                                }
                            }

                            if let Some(min_vol) = get_real(
                                crate::common::well_known_keys::key_object_sound_ambient_min_volume(
                                ),
                            ) {
                                audio_info.override_min_volume(min_vol);
                                info_modified = true;
                            }

                            if let Some(vol) = get_real(
                                crate::common::well_known_keys::key_object_sound_ambient_volume(),
                            ) {
                                audio_info.override_volume(vol);
                                info_modified = true;
                            }

                            if let Some(min_range) = get_real(
                                crate::common::well_known_keys::key_object_sound_ambient_min_range(
                                ),
                            ) {
                                audio_info.override_min_range(min_range);
                                info_modified = true;
                            }

                            if let Some(max_range) = get_real(
                                crate::common::well_known_keys::key_object_sound_ambient_max_range(
                                ),
                            ) {
                                audio_info.override_max_range(max_range);
                                info_modified = true;
                            }

                            if let Some(priority) = get_int(
                                crate::common::well_known_keys::key_object_sound_ambient_priority(),
                            ) {
                                let mapped = match priority {
                                    0 => AudioPriority::Lowest,
                                    1 => AudioPriority::Low,
                                    2 => AudioPriority::Normal,
                                    3 => AudioPriority::High,
                                    _ => AudioPriority::Critical,
                                };
                                audio_info.override_priority(mapped);
                                info_modified = true;
                            }
                        }
                    }
                }

                if !sound_enabled_exists {
                    // C++ Object.cpp: soundEnabled = audioToModify->isPermanentSound()
                    // (AC_LOOP && loopCount==0). AC_LOOP is 0x0001, not AC_ALL 0x0004.
                    if let Some(ref audio_info) = audio_to_modify {
                        sound_enabled = audio_info.audio_event_info.is_permanent_sound();
                        sound_enabled_exists = true;
                    } else {
                        let template = self.get_template();
                        if let Some(base_event) = template.get_sound_ambient() {
                            let manager = get_global_audio_manager()
                                .unwrap_or_else(initialize_global_audio_manager);
                            let manager_lock = manager.lock();
                            if let Ok(manager) = manager_lock {
                                if let Some(base_info) =
                                    manager.find_audio_event_info(&base_event.event_name)
                                {
                                    sound_enabled = base_info.is_permanent_sound();
                                    sound_enabled_exists = true;
                                }
                            }
                        }
                    }
                }

                if sound_enabled_exists && !sound_enabled {
                    draw_guard.enable_ambient_sound_from_script(false);
                }

                if info_modified {
                    if let Some(audio_info) = audio_to_modify.take() {
                        draw_guard.set_custom_sound_ambient_dynamic_info(audio_info);
                    }
                }
            }
        }
    }

    /// Get the owning player (the player who originally built/owns this object).
    /// C++ Reference: Object.h line 229 (getOwningPlayer)
    /// In C++, this is the team the object belongs to. Returns controlling player as fallback.
    pub fn get_owning_player(&self) -> Option<Arc<RwLock<Player>>> {
        self.get_controlling_player()
    }

    /// Calculate the natural rally point for this object (where produced units should gather).
    /// C++ Reference: Object.cpp line 2819 (Object::calcNaturalRallyPoint)
    /// The C++ version transforms a model-space point through the object's transform matrix.
    /// This simplified version uses the object's current position as the rally point.
    pub fn calc_natural_rally_point(&self) -> Coord2D {
        let pos = self.get_position();
        Coord2D { x: pos.x, y: pos.y }
    }

    /// Get the experience points this object has accumulated.
    /// C++ Reference: Object.h line 325 (getExperiencePoints)
    pub fn get_experience_points(&self) -> Real {
        self.experience_points
    }

    pub fn is_locally_controlled(&self) -> bool {
        if let Some(player) = self.get_controlling_player() {
            if let Ok(guard) = player.read() {
                return guard.is_local_player();
            }
        }
        false
    }

    /// Check if object is detected (for stealth mechanics)
    pub fn is_detected(&self) -> bool {
        if !self.is_stealthed() {
            return true;
        }

        // Stealthed units are considered detected only when the DETECTED status is set.
        self.test_status(ObjectStatusTypes::Detected)
    }

    /// Get the current goal object (target) for this object's AI
    /// Returns None if no AI module exists or no goal is set
    /// C++ Reference: Object.cpp - getGoalObject()
    pub fn get_goal_object_id(&self) -> Option<ObjectID> {
        let ai = self.ai.as_ref()?;
        let guard = ai.lock().ok()?;
        let id = guard.get_goal_object_id();
        if id != INVALID_ID { Some(id) } else { None }
    }

    pub fn get_goal_object(&self) -> Option<Arc<RwLock<Object>>> {
        // Wave 264: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return None;
        }

        let goal_id = self.get_goal_object_id()?;
        crate::helpers::TheGameLogic::find_object_by_id(goal_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(goal_id))
    }

    /// Get the thing template for this object
    /// Returns reference to the template that defines this object's type
    pub fn get_template(&self) -> &Arc<dyn ThingTemplate> {
        &self.thing_template
    }

    /// Returns the unmodified build cost from the object's template.
    pub fn get_build_cost(&self) -> crate::common::Int {
        self.thing_template.get_build_cost()
    }

    /// Get the frame when this object was contained by another object
    /// Used for healing timers and containment tracking
    pub fn get_contained_by_frame(&self) -> UnsignedInt {
        self.contained_by_frame
    }

    /// Set the frame when this object was contained by another object
    /// Used for healing timers and containment tracking
    pub fn set_contained_by_frame(&mut self, frame: UnsignedInt) {
        self.contained_by_frame = frame;
    }

    /// Get the object ID of the container this object is inside, if any
    /// Returns None if the object is not contained
    /// C++ Reference: Object.cpp - getContainedBy()
    pub fn get_contained_by(&self) -> Option<ObjectID> {
        // Matches C++ Object::getContainedBy() behavior (returns container id).
        if self.contained_by_id == INVALID_ID {
            None
        } else {
            Some(self.contained_by_id)
        }
    }

    /// Check if this object is inside a container
    ///
    /// Matches C++ Object::isContained() from Object.h line 421
    pub fn is_contained(&self) -> bool {
        self.contained_by_id != INVALID_ID
    }

    /// Get locomotor for this object, if any.
    /// C++ Reference: Object.cpp - getLocomotor()
    pub fn get_locomotor(&self) -> Option<Arc<Mutex<crate::locomotor::Locomotor>>> {
        let ai = self.ai.as_ref()?;
        let guard = ai.lock().ok()?;
        guard.get_cur_locomotor()
    }

    /// C++ ControlBarCommand.cpp:1140 `dozerAI->isTaskPending(DOZER_TASK_BUILD)`.
    pub fn is_dozer_task_pending(&self) -> bool {
        let Some(ai) = self.get_ai_update_interface() else {
            return false;
        };
        let Ok(mut guard) = ai.lock() else {
            return false;
        };
        guard
            .get_dozer_ai_update_interface_mut()
            .is_some_and(|dozer| {
                dozer.is_task_pending(
                    crate::object::update::ai_update::dozer_ai_update::DozerTask::Build,
                )
            })
    }

    /// C++ parity: Object::hasContainedObjects()
    pub fn has_contained_objects(&self) -> bool {
        self.get_contain()
            .map(|c| {
                c.lock()
                    .map(|guard| guard.get_contain_count() > 0)
                    .unwrap_or(false)
            })
            .unwrap_or(false)
    }

    /// Distance to another object
    pub fn distance_to(&self, other: &Object) -> f32 {
        let pos1 = self.get_position();
        let pos2 = other.get_position();
        ((pos1.x - pos2.x).powi(2) + (pos1.y - pos2.y).powi(2) + (pos1.z - pos2.z).powi(2)).sqrt()
    }

    /// Find live enemy objects within 2D radius using the global partition registry.
    pub fn find_enemy_ids_in_radius(&self, radius: f32) -> Result<Vec<ObjectID>, String> {
        // Wave 264: empty dual-world → Ok(empty).
        if dual_world_registry_unavailable() {
            return Ok(Vec::new());
        }

        let Some(partition) = ThePartitionManager::get() else {
            return Ok(Vec::new());
        };

        let mut enemies = Vec::new();
        for object_id in partition.get_objects_in_range(self.get_position(), radius.max(0.0)) {
            if object_id == self.get_id() {
                continue;
            }

            let is_enemy = registry::OBJECT_REGISTRY.with_object(object_id, |candidate| {
                if candidate.is_effectively_dead() {
                    return false;
                }
                self.relationship_to(candidate) == Relationship::Enemies
            });
            if is_enemy.unwrap_or(false) {
                enemies.push(object_id);
            }
        }

        Ok(enemies)
    }

    /// Compatibility wrapper: resolves enemy IDs to handles at the call boundary.
    pub fn find_enemies_in_radius(&self, radius: f32) -> Result<Vec<Arc<RwLock<Object>>>, String> {
        // Wave 264: empty dual-world → Ok(empty).
        if dual_world_registry_unavailable() {
            return Ok(Vec::new());
        }

        let mut enemies = Vec::new();
        for object_id in self.find_enemy_ids_in_radius(radius)? {
            if let Some(object) = registry::OBJECT_REGISTRY.get_object(object_id) {
                enemies.push(object);
            }
        }
        Ok(enemies)
    }

    /// Check if the controlling player's power grid can cover an additional demand.
    ///
    /// C++ callers ultimately query `Player::getEnergy()->hasSufficientPower()`.
    /// The optional amount is a Rust-side helper extension for callers that want
    /// to test a prospective drain before applying it.
    pub fn has_sufficient_power(&self, amount: f32) -> bool {
        let Some(player) = self.get_controlling_player() else {
            return false;
        };
        let Ok(player_guard) = player.read() else {
            return false;
        };
        let energy = player_guard.get_energy();
        if energy.is_power_sabotaged() {
            return false;
        }

        let requested = amount.max(0.0).ceil() as Int;
        energy.get_power() >= requested
    }

    /// Drain power
    pub fn drain_power(&mut self, amount: i32) -> bool {
        if amount <= 0 {
            return true;
        }
        if !self.has_sufficient_power(amount as f32) {
            return false;
        }

        let Some(player) = self.get_controlling_player() else {
            return false;
        };
        let Ok(mut player_guard) = player.write() else {
            return false;
        };
        player_guard.adjust_power(-amount, true);
        true
    }

    /// Enable/disable stealth capability.
    pub fn enable_stealth_capability(&mut self, enabled: bool) {
        if let Some(stealth) = &self.stealth {
            if let Ok(mut guard) = stealth.lock() {
                let _ = guard.receive_grant(enabled, 0, TheGameLogic::get_frame());
                return;
            }
        }

        self.set_status(ObjectStatusMaskType::CAN_STEALTH, enabled);
        if !enabled {
            self.set_status(ObjectStatusMaskType::STEALTHED, false);
            self.set_status(ObjectStatusMaskType::DETECTED, false);
        }
    }

    /// Set stealth visibility level
    pub async fn set_stealth_visibility(&mut self, visibility: f32) -> Result<(), String> {
        let visibility = visibility.clamp(0.0, 1.0);

        if let Some(drawable) = self.get_drawable() {
            if let Ok(mut drawable_guard) = drawable.write() {
                drawable_guard.set_effective_opacity(visibility, Some(visibility));
            }
        }

        if visibility <= 0.001 {
            self.set_status(ObjectStatusMaskType::STEALTHED, true);
            self.set_status(ObjectStatusMaskType::DETECTED, false);
        } else if visibility >= 0.999 {
            self.set_status(ObjectStatusMaskType::STEALTHED, false);
            self.set_status(ObjectStatusMaskType::DETECTED, false);
        }

        Ok(())
    }

    /// Set radar visibility
    pub async fn set_radar_visibility(&mut self, visible: bool) -> Result<(), String> {
        if let Some(player) = self.get_controlling_player() {
            let mut guard = player
                .write()
                .map_err(|_| "Failed to lock controlling player".to_string())?;
            if visible {
                guard.add_radar(false);
            } else {
                guard.remove_radar(false);
            }
        }
        Ok(())
    }

    /// Play visual effect
    pub async fn play_fx(&self, fx_name: &str) -> Result<(), String> {
        // Implementation would create particle effects
        log::trace!("Object {} playing FX: {}", self.id, fx_name);
        Ok(())
    }

    /// Play sound effect
    pub async fn play_sound(&self, sound_name: &str) -> Result<(), String> {
        // Implementation would play audio
        log::trace!("Object {} playing sound: {}", self.id, sound_name);
        Ok(())
    }

    /// Check if wants to stealth (for stealth behavior)
    pub fn wants_to_stealth(&self) -> bool {
        self.status.test(ObjectStatusTypes::CanStealth)
            && !self.status.test(ObjectStatusTypes::Detected)
            && !self.is_disabled()
    }

    /// Get terrain type at object position
    pub fn get_terrain_type(&self) -> String {
        "Ground".to_string() // Simplified
    }

    /// Check if can detect stealth
    pub fn can_detect_stealth(&self) -> bool {
        self.find_update_module("StealthDetectorUpdate").is_some()
    }

    /// Get stealth detection range
    pub fn get_stealth_detection_range(&self) -> f32 {
        if self.can_detect_stealth() {
            self.get_vision_range().max(1.0)
        } else {
            0.0
        }
    }

    /// Get drawable reference
    /// C++ Reference: Object.h line 163 - getDrawable()
    pub fn get_drawable(&self) -> Option<Arc<RwLock<Drawable>>> {
        // Return the drawable associated with this object
        // Matches C++ Object::getDrawable() which returns m_drawable
        self.drawable.clone()
    }

    /// Get command set string for this object
    /// C++ Reference: Object.cpp - Command set string accessor
    pub fn get_command_set_string(&self) -> &str {
        // Check for override first (set by special behaviors or scripts)
        // Matches C++ Object::getCommandSetString() behavior
        if !self.command_set_string_override.is_empty() {
            return &self.command_set_string_override;
        }

        self.thing_template.get_command_set_string().as_str()
    }

    pub fn set_command_set_string_override(&mut self, command_set: &AsciiString) {
        self.command_set_string_override = command_set.clone();
        crate::control_bar::mark_ui_dirty();
    }

    //=========================================================================
    // CONTAINER AND PARTITION METHODS
    // Container and spatial partition management
    //=========================================================================

    /// Set the container that contains this object
    /// C++ Reference: Object.cpp - Container management
    ///
    /// # Arguments
    /// * `container` - Optional reference to the containing object
    ///
    /// # Returns
    /// * `Ok(())` - Container reference set successfully
    /// * `Err(ObjectError)` - Failed to set container
    pub fn set_contained_by(&mut self, container_id: Option<ObjectID>) -> Result<(), ObjectError> {
        self.set_contained_by_id(container_id.unwrap_or(INVALID_ID))
    }

    /// ID-first container association.
    pub fn set_contained_by_id(&mut self, container_id: ObjectID) -> Result<(), ObjectError> {
        self.contained_by_id = container_id;
        if container_id != INVALID_ID {
            self.contained_by_frame = crate::helpers::TheGameLogic::get_frame();
        } else {
            self.contained_by_frame = 0;
        }
        Ok(())
    }

    /// Called when this object is added to a container
    /// C++ Reference: Object.cpp lines 671-683
    ///
    /// This method handles all object-level containment processing:
    /// - Sets UNSELECTABLE status bit (contained objects can't be selected)
    /// - Sets MASKED status if container is enclosing (hides object from players/AI)
    /// - Updates contained_by reference
    /// - Updates contained_by_frame for tracking
    /// - Handles partition cell maintenance (removes from spatial queries)
    ///
    /// # Arguments
    /// * `container` - Reference to the container object
    ///
    /// # Returns
    /// * `Ok(())` - Containment handled successfully
    /// * `Err(ObjectError)` - Failed to handle containment
    pub fn on_contained_by(&mut self, container_id: ObjectID) -> Result<(), ObjectError> {
        // Wave 264: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        use crate::common::types::ObjectStatusMaskType;
        use crate::modules::ContainModuleInterfaceExt;

        // Set UNSELECTABLE status (C++ line 673)
        self.set_status(ObjectStatusMaskType::UNSELECTABLE, true);

        // Check if container is enclosing - if so, set MASKED status (C++ lines 674-677)
        let is_enclosing = if container_id != INVALID_ID {
            if let Some(container) = crate::helpers::TheGameLogic::find_object_by_id(container_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(container_id))
            {
                if let Ok(guard) = container.read() {
                    guard
                        .get_contain()
                        .map(|contain| contain.is_enclosing_container_for(self))
                        .unwrap_or(true)
                } else {
                    true
                }
            } else {
                true
            }
        } else {
            true
        };
        if is_enclosing {
            self.set_status(ObjectStatusMaskType::MASKED, true);
        } else {
            self.clear_status(ObjectStatusMaskType::MASKED);
        }

        // Update contained_by reference (C++ line 678)
        self.contained_by_id = container_id;

        // Update contained_by_frame (C++ line 679)
        self.contained_by_frame = crate::helpers::TheGameLogic::get_frame();

        // Handle partition cell maintenance (C++ line 681)
        // This removes the object from spatial queries now that it's contained
        self.handle_partition_cell_maintenance();

        Ok(())
    }

    /// Called when this object is removed from a container
    /// C++ Reference: Object.cpp lines 688-696
    ///
    /// This method handles all object-level container removal processing:
    /// - Clears MASKED and UNSELECTABLE status bits
    /// - Clears contained_by reference
    /// - Clears contained_by_frame
    /// - Handles partition cell maintenance (adds back to spatial queries)
    ///
    /// # Arguments
    /// * `container` - Reference to the container object this was removed from
    ///
    /// # Returns
    /// * `Ok(())` - Removal handled successfully
    /// * `Err(ObjectError)` - Failed to handle removal
    pub fn on_removed_from(&mut self, _container_id: ObjectID) -> Result<(), ObjectError> {
        use crate::common::types::ObjectStatusMaskType;

        // Clear MASKED and UNSELECTABLE status (C++ line 690)
        self.clear_status(ObjectStatusMaskType::MASKED | ObjectStatusMaskType::UNSELECTABLE);

        // Clear contained_by reference (C++ line 691)
        self.contained_by_id = INVALID_ID;

        // Clear contained_by_frame (C++ line 692)
        self.contained_by_frame = 0;

        // Handle partition cell maintenance (C++ line 694)
        // Get a clean look, now that we're outdoors again
        self.handle_partition_cell_maintenance();

        Ok(())
    }

    /// Get the number of transport slots this object has
    /// C++ Reference: Object::getTransportSlotCount (Object.cpp:700-717)
    ///
    /// Returns the template's raw TransportSlotCount, except for special
    /// zero-slot containers (parachutes), which report the sum of their
    /// riders' transport slot counts instead.
    pub fn get_transport_slot_count(&self) -> usize {
        let mut count = self.thing_template.get_raw_transport_slot_count() as usize;

        let zero_slot_riders: Option<Vec<ObjectID>> = self.contain.as_ref().and_then(|contain| {
            let guard = contain.lock().ok()?;
            if !guard.is_special_zero_slot_container() {
                return None;
            }
            Some(guard.get_contained_objects().to_vec())
        });

        if let Some(rider_ids) = zero_slot_riders {
            count = 0;
            for rider_id in rider_ids {
                if let Some(rider) = crate::helpers::TheGameLogic::find_object_by_id(rider_id)
                    .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(rider_id))
                {
                    if let Ok(rider_guard) = rider.read() {
                        count += rider_guard.get_transport_slot_count();
                    }
                }
            }
        }

        count
    }

    /// Get the container this object is inside
    /// C++ Reference: Object.cpp - Containment system
    ///
    /// # Returns
    /// An optional Arc to the container object
    pub fn get_container_id(&self) -> Option<ObjectID> {
        if self.contained_by_id == INVALID_ID {
            None
        } else {
            Some(self.contained_by_id)
        }
    }

    pub fn get_container(&self) -> Option<Arc<RwLock<Object>>> {
        // Wave 264: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return None;
        }

        let container_id = self.get_container_id()?;
        crate::helpers::TheGameLogic::find_object_by_id(container_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(container_id))
    }

    pub fn get_indicator_color(&self) -> Color {
        if self.indicator_color != Color::default() {
            return self.indicator_color;
        }

        self.get_controlling_player()
            .and_then(|player| player.read().ok().map(|guard| guard.get_player_color()))
            .unwrap_or(Color::black())
    }

    pub fn get_night_indicator_color(&self) -> Color {
        if self.indicator_color != Color::default() {
            return self.indicator_color;
        }

        self.get_controlling_player()
            .and_then(|player| {
                player
                    .read()
                    .ok()
                    .map(|guard| guard.get_player_night_color())
            })
            .unwrap_or(Color::black())
    }

    pub fn set_custom_indicator_color(&mut self, color: Color) {
        if self.indicator_color != color {
            self.indicator_color = color;
            self.update_drawable_team_visuals();
        }
    }

    pub fn clear_custom_indicator_color(&mut self) {
        if self.indicator_color != Color::default() {
            self.indicator_color = Color::default();
            self.update_drawable_team_visuals();
        }
    }

    pub(super) fn update_drawable_team_visuals(&self) {
        let Some(drawable) = &self.drawable else {
            return;
        };
        if let Ok(mut guard) = drawable.write() {
            guard.changed_team(self);
        }
    }

    /// Handle collision with another object or terrain
    /// C++ Reference: Object.cpp line 253 - onCollide
    ///
    /// # Arguments
    /// * `other` - Optional other object involved in collision
    /// * `loc` - Location of collision
    /// * `normal` - Normal vector at collision point
    pub fn on_collide(&mut self, other: Option<&Object>, loc: &Coord3D, normal: &Coord3D) {
        if self.test_status(ObjectStatusTypes::NoCollisions) {
            return;
        }
        let other_handle =
            other.and_then(|obj| crate::helpers::TheGameLogic::find_object_by_id(obj.get_id()));
        let other_game_object = other_handle
            .as_ref()
            .map(|handle| handle as &dyn crate::object::collide::GameObject);
        let collide_loc = crate::object::collide::Coord3D::new(loc.x, loc.y, loc.z);
        let collide_normal = crate::object::collide::Coord3D::new(normal.x, normal.y, normal.z);

        if let Err(err) = crate::object::collide::COLLISION_MANAGER.handle_collision(
            self.id,
            other_game_object,
            &collide_loc,
            &collide_normal,
        ) {
            log::warn!(
                "Object {} collision handling failed at ({}, {}, {}): {}",
                self.id,
                loc.x,
                loc.y,
                loc.z,
                err
            );
        }
    }

    pub fn get_group(&self) -> Option<Arc<RwLock<crate::ai::AiGroup>>> {
        let group_id = self.group_id?;
        crate::ai::the_ai()
            .read()
            .ok()
            .and_then(|ai_guard| ai_guard.find_group(group_id))
    }

    // ========================================================================
    // HEALTH BOX VISUAL (2 methods)
    // C++ Reference: Object.cpp getHealthBoxPosition, getHealthBoxDimensions
    // ========================================================================

    pub fn get_health_box_position(&self) -> Coord3D {
        let pos = *self.get_position();
        let mut result = Coord3D::new(
            pos.x + self.health_box_offset.x,
            pos.y + self.health_box_offset.y,
            pos.z
                + self.geometry_info.get_max_height_above_position()
                + 10.0
                + self.health_box_offset.z,
        );

        if self.is_kind_of(KindOf::MobNexus) {
            result.z += 20.0;
        }

        result
    }

    pub fn get_health_box_dimensions(&self) -> (f32, f32) {
        // C++ Object.cpp:3402-3413 (`CALC_HEALTHBAR_FROM_HITPOINTS` is undefined).
        // The ifdef HP path used `min(3, max(5, hp/50))`, which is always 3.0.
        if self.is_kind_of(KindOf::IgnoredInGui) {
            (0.0, 0.0)
        } else {
            let size = (self.geometry_info.get_major_radius()
                + self.geometry_info.get_minor_radius())
            .min(150.0)
            .max(20.0);
            (3.0, (size * 2.0).max(20.0))
        }
    }

    /// Try to get a read reference to this object (for compatibility with Arc<RwLock<Object>>).
    pub fn try_read(&self) -> Result<&Self, String> {
        Ok(self)
    }
}
