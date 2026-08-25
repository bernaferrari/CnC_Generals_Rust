//! Split-out inherent `update loop, model conditions, capture, events` methods for [`Object`].
//!
//! Child of `object` so private `Object` fields remain visible.

#![allow(unused_imports)]

use super::object_impl_imports::*;
use super::*;

impl Object {
    /// Apply a topple force if this object has a ToppleUpdate module.
    /// Mirrors C++ Object::topple() usage.
    pub fn topple(&mut self, topple_direction: &Coord3D, topple_speed: Real, options: u32) {
        let Some(object_arc) = crate::helpers::TheGameLogic::find_object_by_id(self.id) else {
            return;
        };
        for behavior in self.get_behavior_modules() {
            let Ok(mut behavior) = behavior.lock() else {
                continue;
            };
            let Some(topple) = behavior.get_topple_control_interface() else {
                continue;
            };
            if topple.is_able_to_be_toppled() {
                topple.apply_toppling_force_with_object(
                    self,
                    &object_arc,
                    topple_direction,
                    topple_speed,
                    options,
                );
            }
            break;
        }
    }

    //=========================================================================
    // HELPER METHODS FOR CRITICAL SYSTEMS
    //=========================================================================

    pub fn set_model_condition_state(&mut self, flag: ModelConditionFlags) {
        if let Some(drawable) = &self.drawable {
            if let Ok(mut drawable) = drawable.write() {
                drawable.set_model_condition_state(flag);
            }
        }
    }

    pub fn clear_model_condition_state(&mut self, flag: ModelConditionFlags) {
        if let Some(drawable) = &self.drawable {
            if let Ok(mut drawable) = drawable.write() {
                drawable.clear_model_condition_state(flag);
            }
        }
    }

    /// Set a special model condition state flag for a limited duration
    /// Matches C++ Object::setSpecialModelConditionState behavior for temporary flags.
    pub fn set_special_model_condition_state(
        &mut self,
        flag: ModelConditionFlags,
        duration_frames: UnsignedInt,
    ) {
        if self.smc_helper.is_none() {
            self.smc_helper = Some(Arc::new(Mutex::new(ObjectSMCHelper::new(
                ObjectSMCHelperModuleData::default(),
            ))));
        }

        self.clear_special_model_condition_states();

        if flag != ModelConditionFlags::empty() {
            self.set_model_condition_state(flag);
            self.special_model_condition_flag = flag;
            let current_frame = crate::helpers::TheGameLogic::get_frame();
            let mut frames = duration_frames;
            if frames == 0 {
                frames = 1;
            }
            self.smc_until = current_frame.saturating_add(frames);
            if let Some(helper) = &self.smc_helper {
                if let Ok(mut guard) = helper.lock() {
                    guard.sleep_until(self.smc_until);
                }
            }
        } else {
            self.special_model_condition_flag = ModelConditionFlags::empty();
            self.smc_until = NEVER;
        }
    }

    /// Clear special model condition states (matches C++ Object::clearSpecialModelConditionStates)
    pub fn clear_special_model_condition_states(&mut self) {
        if self.special_model_condition_flag != ModelConditionFlags::empty() {
            self.clear_model_condition_state(self.special_model_condition_flag);
        }
        self.special_model_condition_flag = ModelConditionFlags::empty();
        self.smc_until = NEVER;
    }

    /// Primary per-frame update hook (ports C++ Object::Update).
    ///
    /// Runs per-frame object maintenance and module-facing update hooks that are currently
    /// available in this port.
    pub fn update(&mut self, _delta_time: f32) -> Result<(), String> {
        let current_frame = crate::helpers::TheGameLogic::get_frame();
        self.check_disabled_status();
        self.update_firing_tracker();

        if let Some(contain) = &self.contain {
            if let Ok(mut contain_guard) = contain.lock() {
                if let Err(err) = contain_guard.update() {
                    log::warn!("Object {} contain update failed: {}", self.id, err);
                }
            }
        }

        // Clear repulsor status once the helper's wake frame is reached.
        let helper = self.repulsor_helper.clone();
        let mut should_clear_repulsor = false;
        if let Some(helper) = &helper {
            if let Ok(mut guard) = helper.lock() {
                should_clear_repulsor = guard.should_clear(current_frame);
                if should_clear_repulsor {
                    guard.mark_cleared();
                }
            }
        }
        if should_clear_repulsor {
            self.clear_status(ObjectStatusMaskType::from_status(
                crate::common::types::ObjectStatusTypes::Repulsor,
            ));
        }

        if self.special_model_condition_flag != ModelConditionFlags::empty()
            && self.smc_until != NEVER
            && current_frame >= self.smc_until
        {
            let flag = self.special_model_condition_flag;
            self.clear_model_condition_state(flag);
            self.special_model_condition_flag = ModelConditionFlags::empty();
            self.smc_until = NEVER;
        }

        if self.is_undetected_defector() {
            let helper = self.defection_helper.clone();
            if let Some(helper) = helper {
                if let Ok(mut guard) = helper.lock() {
                    let mut clear_defector = false;
                    let mut play_tick = false;
                    let mut play_ding = false;
                    let current_frame = crate::helpers::TheGameLogic::get_frame();

                    if guard.has_timer_expired(current_frame) {
                        clear_defector = true;
                        play_ding = guard.is_defector_fx_enabled();
                        if let Some(drawable) = self.get_drawable() {
                            if let Ok(mut draw_guard) = drawable.write() {
                                draw_guard.flash_as_selected();
                            }
                        }
                    } else if self.is_effectively_dead()
                        || self
                            .get_status_bits()
                            .test(ObjectStatusTypes::IsFiringWeapon)
                    {
                        clear_defector = true;
                    } else if guard.is_defector_fx_enabled() {
                        let (should_flash, _color) = guard.should_flash(current_frame);
                        if should_flash {
                            play_tick = true;
                            if let Some(drawable) = self.get_drawable() {
                                if let Ok(mut draw_guard) = drawable.write() {
                                    draw_guard.flash_as_selected();
                                }
                            }
                        }
                    }

                    if clear_defector {
                        drop(guard);
                        self.friend_set_undetected_defector(false);
                    }

                    if play_tick || play_ding {
                        if let Some(audio) = crate::helpers::TheAudio::get() {
                            if let Some(misc_audio) =
                                game_engine::common::ini::ini_misc_audio::get_misc_audio()
                            {
                                let misc_audio = misc_audio.read();
                                let sound_name = if play_ding {
                                    misc_audio
                                        .defector_timer_ding_sound
                                        .playable_event_name()
                                        .to_string()
                                } else {
                                    misc_audio
                                        .defector_timer_tick_sound
                                        .playable_event_name()
                                        .to_string()
                                };
                                let mut event =
                                    crate::object::special_power_template::AudioEventRts::new(
                                        sound_name,
                                    );
                                event.set_object_id(self.id);
                                audio.add_audio_event(&event);
                            }
                        }
                    }
                }
            }
        }

        if let Some(helper) = &self.subdual_damage_helper {
            if let Ok(mut guard) = helper.lock() {
                let _ = guard.update(current_frame);
            }
        }

        if let Some(helper) = &self.status_damage_helper {
            if let Ok(mut guard) = helper.lock() {
                if guard.has_active_status() && guard.get_frame_to_heal() <= current_frame {
                    let _ = guard.update(current_frame);
                }
            }
        }

        if let Some(helper) = &self.temp_weapon_bonus_helper {
            if let Ok(mut guard) = helper.lock() {
                if guard.has_active_bonus() && guard.get_frame_to_remove() <= current_frame {
                    let _ = guard.update(current_frame);
                }
            }
        }

        if self.get_last_shot_fired_frame() == current_frame {
            self.set_status(
                ObjectStatusMaskType::from_status(ObjectStatusTypes::IsFiringWeapon),
                true,
            );
        } else {
            self.clear_status(ObjectStatusMaskType::from_status(
                ObjectStatusTypes::IsFiringWeapon,
            ));
        }

        self.adjust_model_condition_for_weapon_status();

        // Update-module dispatch is handled by the GameLogic sleepy-update scheduler. This object
        // method is kept for parity with the legacy call graph and for systems that still expect a
        // per-object update hook.
        Ok(())
    }

    /// Reschedule all registered update-module proxies relative to the current frame.
    pub fn wake_update_modules_after(
        &mut self,
        current_frame: UnsignedInt,
        sleep: UpdateSleepTime,
    ) {
        if self.update_module_registrations.is_empty() {
            return;
        }

        let wake_frame = match sleep {
            UpdateSleepTime::None => 0,
            UpdateSleepTime::Forever => UpdateSleepTime::Forever.to_u32(),
            UpdateSleepTime::Frames(frames) => current_frame.saturating_add(frames.max(1)),
        };

        for module in &self.update_module_registrations {
            let _ = crate::helpers::TheGameLogic::register_update_module(
                self.id,
                module.clone(),
                wake_frame,
            );
        }
    }

    /// Live update-module proxies registered at object create (C++ behavior modules).
    pub fn update_module_registrations(&self) -> &[UpdateModulePtr] {
        &self.update_module_registrations
    }

    /// Test/restore helper: attach an already-built update proxy for loadPostProcess.
    pub fn attach_update_module_registration(&mut self, module: UpdateModulePtr) {
        self.update_module_registrations.push(module);
    }

    /// Check if object is moving
    pub fn is_moving(&self) -> bool {
        if let Some(ai) = &self.ai {
            if let Ok(ai_guard) = ai.lock() {
                return ai_guard.is_moving();
            }
        }
        false
    }

    /// Check if object is idle
    pub fn is_idle(&self) -> bool {
        if let Some(ai) = &self.ai {
            if let Ok(ai_guard) = ai.lock() {
                return ai_guard.is_idle();
            }
        }
        !self.is_moving()
    }

    /// Check if object is in combat
    pub fn is_in_combat(&self) -> bool {
        self.is_attacking() || self.status.test(ObjectStatusTypes::IsUsingAbility)
    }

    /// Get object type string
    pub fn get_type(&self) -> String {
        self.get_template_name().to_string()
    }

    /// Central point for onCapture logic - called when object is captured by another player
    /// C++ Reference: Object.cpp lines 4509-4544
    ///
    /// This method handles all object-level capture processing:
    /// - Makes AI go idle (prevents continuing old player's orders)
    /// - Awards points to new owner
    /// - Notifies all behavior modules of capture
    /// - Handles partition cell maintenance (team change)
    /// - Clears unsellable script status
    /// - Updates UI
    /// - Special handling for AI capturing faction buildings (sells them)
    ///
    /// # Arguments
    /// * `old_owner` - The previous owner (can be None for neutral)
    /// * `new_owner` - The new owner (can be None for neutral)
    ///
    /// # Notes
    /// - This is called AFTER ownership has been changed
    /// - Ownership change itself should be done before calling this
    /// - Team and player must already be updated
    pub fn on_capture(
        &mut self,
        old_owner: Option<Arc<RwLock<Player>>>,
        new_owner: Option<Arc<RwLock<Player>>>,
    ) {
        // Everybody idles when captured so they don't keep doing something
        // the new player might not want them to be doing
        let owners_differ = match (&old_owner, &new_owner) {
            (Some(old), Some(new)) => !Arc::ptr_eq(old, new),
            (None, None) => false,
            _ => true,
        };

        if owners_differ {
            if let Some(ai) = &self.ai {
                log::debug!("Object {} AI going idle due to capture", self.id);
                ai.ai_idle(CommandSourceType::FromAi);
            }
        }

        self.on_capture_award_score(&new_owner);

        // Rip through the behavior modules and call the onCapture for any modules that care
        log::debug!("Object {} notifying behavior modules of capture", self.id);
        for entry in &self.modules {
            entry.with_module(|module| {
                if let Some(kind) = module_behavior_utility_kind(module) {
                    kind.notify_capture(old_owner.as_ref(), new_owner.as_ref());
                }
            });
        }

        let mut contain_notified = false;
        if let Some(contain) = &self.contain {
            if let Ok(mut contain_guard) = contain.lock() {
                if let Err(err) =
                    contain_guard.on_capture(self, old_owner.as_ref(), new_owner.as_ref())
                {
                    log::warn!("Object {} contain on_capture failed: {}", self.id, err);
                }
                contain_notified = true;
            }
        }

        for behavior in &self.behaviors {
            if let Ok(mut behavior_guard) = behavior.lock() {
                behavior_guard.on_capture(old_owner.as_ref(), new_owner.as_ref());
                if !contain_notified {
                    if let Some(contain) = behavior_guard.get_contain() {
                        if let Err(err) =
                            contain.on_capture(self, old_owner.as_ref(), new_owner.as_ref())
                        {
                            log::warn!(
                                "Object {} behavior-backed contain on_capture failed: {}",
                                self.id,
                                err
                            );
                        }
                        contain_notified = true;
                    }
                }
            }
        }

        if owners_differ {
            let upgrade_modules = self.modules.clone();
            for entry in &upgrade_modules {
                entry.with_module(|module| {
                    if let Some(upgrade) = super::module_upgrade_kind(module) {
                        upgrade.into_interface().on_capture(
                            self,
                            old_owner.as_ref(),
                            new_owner.as_ref(),
                        );
                    }
                });
            }
        }

        // We have to undo our look for the old team and redo it for the new.
        // onCapture is used now, so it better be called after ownership changes and not before.
        log::debug!(
            "Object {} handling partition cell maintenance after capture",
            self.id
        );
        self.handle_partition_cell_maintenance();

        // Design needs the player to be able to sell buildings he steals from the AI's build list,
        // and this is the easiest fix. The only snafu would be a key building build listed by the AI
        // that the player can capture and the AI tries to capture back but needs to not sell.
        // In that case, a Cinematic Unsellable version of the building needs to be made.
        // This fix has been okayed as the most non-lethal in November.
        self.clear_script_status(ObjectScriptStatusBit::Unsellable);

        // Mark the command bar to redraw
        log::debug!("Object {} marking UI dirty after capture", self.id);
        crate::control_bar::mark_ui_dirty();

        self.on_capture_sell_ai_faction_building(owners_differ, &new_owner);

        log::debug!("Object {} on_capture processing complete", self.id);
    }

    /// Set the captured status flag
    /// C++ Reference: Object.cpp lines 1971-1979
    pub fn set_captured(&mut self, is_captured: bool) {
        if is_captured {
            self.private_status |= ObjectPrivateStatusBits::Captured as u8;
            log::debug!("Object {} marked as captured", self.id);
        } else {
            // This should never happen according to C++ comments
            log::warn!(
                "Clearing Captured Status for object {}. This should never happen.",
                self.id
            );
            self.private_status &= !(ObjectPrivateStatusBits::Captured as u8);
        }
    }

    /// Check if object is captured
    pub fn is_captured(&self) -> bool {
        (self.private_status & ObjectPrivateStatusBits::Captured as u8) != 0
    }

    /// Apply movement force for physics
    pub async fn apply_movement_force(
        &mut self,
        force_x: f32,
        force_y: f32,
        force_z: f32,
    ) -> Result<(), String> {
        // Trigger force application event
        let force_data = (force_x, force_y, force_z);
        let serialized =
            bincode::serialize(&force_data).map_err(|e| format!("Serialization error: {}", e))?;
        self.trigger_event("apply_force", &serialized).await
    }

    /// Trigger an event on this object
    pub async fn trigger_event(&mut self, event: &str, _data: &[u8]) -> Result<(), String> {
        // Implementation would route to behavior system and update modules
        log::trace!("Object {} triggered event: {}", self.id, event);
        Ok(())
    }

    /// Set animation state
    pub fn set_animation(&mut self, animation: &str, progress: f32) {
        // Implementation would update drawable/model state
        log::trace!(
            "Object {} animation: {} at {}%",
            self.id,
            animation,
            progress * 100.0
        );
    }

    /// Set animation to loop in N frames
    ///
    /// This call says, "I want the current animation (if any) to take n frames to complete a single cycle".
    /// If it's a looping anim, each loop will take n frames.
    /// Note that you must call this AFTER setting the condition codes.
    ///
    /// Reference: C++ Drawable.h:469 - setAnimationLoopDuration
    pub fn set_animation_loop_duration(&mut self, num_frames: u32) {
        if let Some(ref drawable) = self.drawable {
            if let Ok(mut guard) = drawable.write() {
                guard.set_animation_loop_duration(num_frames);
            }
        }
    }

    /// Set animation completion time
    ///
    /// Similar to setAnimationLoopDuration, but assumes that the current state is a "ONCE",
    /// and is smart about transition states... if there is a transition state "inbetween",
    /// it is included in the completion time.
    ///
    /// Reference: C++ Drawable.h:475 - setAnimationCompletionTime
    pub fn set_animation_completion_time(&mut self, num_frames: u32) {
        if let Some(ref drawable) = self.drawable {
            if let Ok(mut guard) = drawable.write() {
                guard.set_animation_completion_time(num_frames);
            }
        }
    }

    /// Set animation frame manually
    ///
    /// Manually set a drawable's current animation to a specific frame.
    ///
    /// Reference: C++ Drawable.h:478 - setAnimationFrame
    pub fn set_animation_frame(&mut self, frame: i32) {
        if let Some(ref drawable) = self.drawable {
            if let Ok(mut guard) = drawable.write() {
                guard.set_animation_frame(frame);
            }
        }
    }

    /// Fire an object event to the scripting system
    pub(super) fn fire_object_event(&self, event: GameEvent) {
        let event_manager = get_event_manager();
        if let Err(e) = futures::executor::block_on(event_manager.fire_event(event)) {
            log::warn!("Failed to fire object event for object {}: {}", self.id, e);
        }
    }

    /// Fire object created event
    pub fn fire_created_event(&self, template_name: &str) {
        let event = GameEvent::new(
            GameEventType::UnitCreated,
            format!("Object {} ({}) created", self.id, template_name),
        )
        .with_source_object(self.id)
        .with_parameter(
            "template_name".to_string(),
            ScriptValue::String(template_name.to_string()),
        )
        .with_parameter(
            "position".to_string(),
            ScriptValue::Coord3D([
                self.geometry_info.position.x,
                self.geometry_info.position.y,
                self.geometry_info.position.z,
            ]),
        );

        self.fire_object_event(event);
    }

    /// Fire object destroyed event
    pub fn fire_destroyed_event(&self, killer_id: Option<ObjectID>) {
        let template_name = self.get_template_name().to_string();
        let controlling_player_id = self.get_controlling_player_id().map(|id| id as u32);

        let mut event = GameEvent::new(
            GameEventType::UnitDestroyed,
            format!("Object {} destroyed", self.id),
        )
        .with_source_object(self.id)
        .with_priority(ScriptPriority::High)
        .with_parameter(
            "template_name".to_string(),
            ScriptValue::String(template_name),
        )
        .with_parameter(
            "position".to_string(),
            ScriptValue::Coord3D([
                self.geometry_info.position.x,
                self.geometry_info.position.y,
                self.geometry_info.position.z,
            ]),
        );

        if let Some(player_id) = controlling_player_id {
            event = event.with_player(player_id).with_parameter(
                "owner_player".to_string(),
                ScriptValue::Int(player_id as i64),
            );
        }

        if let Some(killer) = killer_id {
            event = event
                .with_target_object(killer)
                .with_parameter("killer_id".to_string(), ScriptValue::ObjectId(killer));
        }

        self.fire_object_event(event);
    }

    /// Fire object damaged event
    pub fn fire_damaged_event(&self, damage: Real, attacker_id: Option<ObjectID>) {
        let health_percentage = (self.get_health() / self.get_max_health()) * 100.0;

        let mut event = GameEvent::new(
            GameEventType::UnitDamaged,
            format!(
                "Object {} damaged ({}% health remaining)",
                self.id, health_percentage as i32
            ),
        )
        .with_source_object(self.id)
        .with_parameter("damage".to_string(), ScriptValue::Float(damage as f64))
        .with_parameter(
            "health".to_string(),
            ScriptValue::Float(self.get_health() as f64),
        )
        .with_parameter(
            "health_percentage".to_string(),
            ScriptValue::Float(health_percentage as f64),
        );

        if let Some(attacker) = attacker_id {
            event = event
                .with_target_object(attacker)
                .with_parameter("attacker_id".to_string(), ScriptValue::ObjectId(attacker));
        }

        self.fire_object_event(event);
    }

    /// Fire veterancy gained event
    pub fn fire_veterancy_event(&self, old_level: VeterancyLevel, new_level: VeterancyLevel) {
        let event = GameEvent::new(
            GameEventType::UnitPromoted,
            format!("Object {} promoted to level {:?}", self.id, new_level),
        )
        .with_source_object(self.id)
        .with_priority(ScriptPriority::High)
        .with_parameter("old_level".to_string(), ScriptValue::Int(old_level as i64))
        .with_parameter("new_level".to_string(), ScriptValue::Int(new_level as i64));

        self.fire_object_event(event);
    }

    /// Fire weapon fired event
    pub fn fire_weapon_fired_event(&self, weapon_name: &str, target_id: Option<ObjectID>) {
        let mut event = GameEvent::new(
            GameEventType::WeaponFired,
            format!("Object {} fired weapon {}", self.id, weapon_name),
        )
        .with_source_object(self.id)
        .with_parameter(
            "weapon_name".to_string(),
            ScriptValue::String(weapon_name.to_string()),
        );

        if let Some(target) = target_id {
            event = event
                .with_target_object(target)
                .with_parameter("target_id".to_string(), ScriptValue::ObjectId(target));
        }

        self.fire_object_event(event);
    }

    /// Clear and set model condition flags atomically
    /// C++ Reference: Object.cpp line 1320 - clearAndSetModelConditionFlags
    pub fn clear_and_set_model_condition_flags(
        &mut self,
        clear: ModelConditionFlags,
        set: ModelConditionFlags,
    ) -> Result<(), String> {
        // Update drawable model condition flags if drawable exists
        // Matches C++ Object::clearAndSetModelConditionFlags behavior
        if let Some(drawable) = &self.drawable {
            if let Ok(mut drawable_guard) = drawable.write() {
                // Clear the flags first, then set new ones
                drawable_guard.clear_model_condition_state(clear);
                drawable_guard.set_model_condition_state(set);
            }
        }
        Ok(())
    }

    /// Clear model condition flags
    /// C++ Reference: Object.cpp - clearModelConditionFlags
    pub fn clear_model_condition_flags(
        &mut self,
        clear: ModelConditionFlags,
    ) -> Result<(), String> {
        // Update drawable model condition flags if drawable exists
        // Matches C++ behavior by delegating to drawable
        if let Some(drawable) = &self.drawable {
            if let Ok(mut drawable_guard) = drawable.write() {
                drawable_guard.clear_model_condition_state(clear);
            }
        }
        Ok(())
    }

    /// Set model condition flags
    /// C++ Reference: Object.cpp line 1311 - setModelConditionFlags
    pub fn set_model_condition_flags(&mut self, set: ModelConditionFlags) -> Result<(), String> {
        // Update drawable model condition flags if drawable exists
        // Matches C++ Object::setModelConditionFlags behavior
        if let Some(drawable) = &self.drawable {
            if let Ok(mut drawable_guard) = drawable.write() {
                drawable_guard.set_model_condition_state(set);
            }
        }
        Ok(())
    }

    /// Make this object defect to another team
    /// C++ Reference: Object.cpp - Defection system
    ///
    /// # Arguments
    /// * `new_team` - The team to defect to
    /// * `defection_type` - Type of defection (0 = normal)
    pub fn defect(&mut self, new_team: Option<Arc<RwLock<Team>>>, defection_type: u32) {
        // C++ Object::defect does not early-out on an empty dual-world registry.

        // C++ parity: contained units do not defect.
        if self.get_container_id().is_some() {
            return;
        }

        let Some(player) = self.get_controlling_player() else {
            return;
        };
        let my_default_team = player
            .read()
            .ok()
            .and_then(|guard| guard.get_default_team());

        let Some(target_team) = new_team.clone() else {
            return;
        };
        let my_default_team_id = my_default_team
            .as_ref()
            .and_then(|team_ref| team_ref.read().ok())
            .map(|team_guard| team_guard.get_id());
        let new_team_id = target_team
            .read()
            .ok()
            .map(|team_guard| team_guard.get_id());
        if my_default_team_id.is_some() && my_default_team_id == new_team_id {
            return;
        }

        // things that are under construction, or sold, cannot defect.
        if self.test_status(ObjectStatusTypes::UnderConstruction)
            || self.test_status(ObjectStatusTypes::Sold)
        {
            return;
        }

        // C++ parity: cancel and refund active production before ownership switch.
        self.cancel_and_refund_all_production_for_capture_or_defection();

        // C++ parity: radar infiltration ping before team switch when both sides are playable.
        let team_controller_is_playable = |team: &Arc<RwLock<Team>>| -> bool {
            team.read()
                .ok()
                .and_then(|team_guard| team_guard.get_controlling_player_id())
                .and_then(|id| {
                    player_list()
                        .read()
                        .ok()
                        .and_then(|list| list.get_player(id as i32).cloned())
                })
                .and_then(|player_arc| {
                    player_arc
                        .read()
                        .ok()
                        .map(|player_guard| player_guard.is_playable_side())
                })
                .unwrap_or(false)
        };
        if self.radar_data.is_some()
            && team_controller_is_playable(&target_team)
            && my_default_team
                .as_ref()
                .map(team_controller_is_playable)
                .unwrap_or(false)
        {
            let _ = crate::helpers::TheRadar::try_infiltration_event_for_object(self);
        }

        self.friend_set_undetected_defector(defection_type > 0);
        if self.defection_helper.is_none() {
            self.defection_helper = Some(Arc::new(Mutex::new(ObjectDefectionHelper::new(
                ObjectDefectionHelperModuleData::new(),
            ))));
        }
        if let Some(helper) = &self.defection_helper {
            if let Ok(mut helper_guard) = helper.lock() {
                let current_frame = crate::helpers::TheGameLogic::get_frame();
                helper_guard.start_defection_timer(
                    defection_type as UnsignedInt,
                    true,
                    current_frame,
                    self.is_undetected_defector(),
                );
            }
        }

        if let Err(err) = self.set_team(Some(target_team.clone())) {
            log::warn!(
                "Object::defect failed to set team for object {}: {}",
                self.id,
                err
            );
            return;
        }

        self.handle_partition_cell_maintenance();
        if let Some(ai) = self.get_ai_update_interface() {
            ai.ai_idle(CommandSourceType::FromAi);
        }

        if let Some(drawable) = &self.drawable {
            if let Ok(mut draw_guard) = drawable.write() {
                draw_guard.flash_as_selected();
            }
        }
        self.defect_play_voice_and_timer();

        if let Some(contain) = self.get_contain() {
            if let Ok(mut contain_guard) = contain.lock() {
                if contain_guard.is_kick_out_on_capture() {
                    let _ = contain_guard.remove_all_contained(true);
                }
            }
        }

        let detection_time = defection_type as UnsignedInt;
        let _ = self.with_parking_place_behavior(|parking| {
            parking.defect_all_parked_units(target_team.clone(), detection_time);
        });

        self.defect_owned_mines(&target_team);
    }
}
