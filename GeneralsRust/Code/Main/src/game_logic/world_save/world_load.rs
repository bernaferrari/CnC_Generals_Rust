//! Map loading, validation helpers, and load side effects.
#![allow(unused_imports, non_snake_case)]
use super::*;

impl GameLogic {
    /// Replay commands belong to their recorded map; never substitute another match.
    pub(crate) fn load_recorded_map_with_progress<F>(
        &mut self,
        map: &str,
        progress: F,
    ) -> Option<String>
    where
        F: FnMut(f32, &str),
    {
        self.load_map_with_progress(map, progress)
            .then(|| map.to_owned())
    }

    pub(in super::super) fn ground_loaded_map_objects_to_terrain(
        &mut self,
        objects: &[super::script_loader::PlacedObject],
        spawned_object_ids: &[(ObjectId, usize)],
        effects: &mut ScriptPresentationEffects<'_>,
    ) {
        if self.terrain.is_none() || spawned_object_ids.is_empty() {
            return;
        }

        let mut grounded_positions = Vec::with_capacity(spawned_object_ids.len());
        for &(_, index) in spawned_object_ids {
            let object = &objects[index];
            let ground_height = self
                .terrain_ground_height_at(Vec3::new(object.position.x, 0.0, object.position.y))
                .unwrap_or(0.0);
            grounded_positions.push((
                index,
                object.position.x,
                object.position.z + ground_height,
                object.position.y,
            ));
        }

        let frame = self.frame;
        for ((object_id, _), (_, x, y, z)) in
            spawned_object_ids.iter().copied().zip(grounded_positions)
        {
            if let Some(object) = self.objects.get_mut(&object_id) {
                object.set_position_with_effects(
                    Vec3::new(x, y, z),
                    frame,
                    &mut effects.reborrow_for_object(&mut self.partition_manager),
                );
                // y is world height after grounding; residual height sample = y when terrain present.
                object.set_ground_height_residual(y, true);
                {}
            }
        }
        // Wave 475: host map grounding is host-ObjectStore only.
        // Dual-world OBJECT_REGISTRY pose writes retired — bridge/shadow materialize owns GW poses.
        let _ = objects;
    }

    /// Load a map with optional milestone progress reporting.
    pub fn load_map_with_progress<F>(&mut self, map_name: &str, report_progress: F) -> bool
    where
        F: FnMut(f32, &str),
    {
        let difficulty = self
            .owned_script_engine
            .as_ref()
            .map(|engine| engine.get_global_difficulty());
        self.load_map_with_script_difficulty_and_progress(map_name, difficulty, report_progress)
    }

    fn load_map_with_script_difficulty_and_progress<F>(
        &mut self,
        map_name: &str,
        difficulty: Option<gamelogic::player::GameDifficulty>,
        report_progress: F,
    ) -> bool
    where
        F: FnMut(f32, &str),
    {
        let services = Arc::clone(&self.world_services);
        gamelogic::system::engine_stores::with_world_services(&services, || {
            self.load_map_with_definition_source_and_progress(
                map_name,
                None,
                difficulty,
                None,
                None,
                crate::save_load::CampaignMapContext::default(),
                report_progress,
            )
        })
    }

    pub(crate) fn load_map_with_progress_and_native<F>(
        &mut self,
        map_name: &str,
        mut progress: F,
        native: Option<&mut dyn crate::game_logic::NativeMapAdmission>,
    ) -> bool
    where
        F: FnMut(f32, &str),
    {
        self.load_map_with_progress_and_native_checked(
            map_name,
            |_, _audio_policy, value, phase| {
                progress(value, phase);
                Ok(())
            },
            native,
        )
        .unwrap_or_else(|error| {
            log::error!("Map admission failed: {error}");
            false
        })
    }

    pub(crate) fn load_map_with_progress_and_native_checked<F>(
        &mut self,
        map_name: &str,
        progress: F,
        native: Option<&mut dyn crate::game_logic::NativeMapAdmission>,
    ) -> Result<bool, String>
    where
        F: FnMut(
            &mut game_engine::common::random_value::RandomState,
            &game_engine::common::audio::script_policy::ScriptAudioPolicy,
            f32,
            &str,
        ) -> Result<(), String>,
    {
        self.load_map_with_campaign_context_and_progress_and_native_checked(
            map_name,
            crate::save_load::CampaignMapContext::default(),
            progress,
            native,
        )
    }

    pub(crate) fn load_map_with_campaign_context_and_progress_and_native_checked<F>(
        &mut self,
        map_name: &str,
        campaign_context: crate::save_load::CampaignMapContext,
        progress: F,
        native: Option<&mut dyn crate::game_logic::NativeMapAdmission>,
    ) -> Result<bool, String>
    where
        F: FnMut(
            &mut game_engine::common::random_value::RandomState,
            &game_engine::common::audio::script_policy::ScriptAudioPolicy,
            f32,
            &str,
        ) -> Result<(), String>,
    {
        let difficulty = self
            .owned_script_engine
            .as_ref()
            .map(|engine| engine.get_global_difficulty());
        self.load_map_with_script_difficulty_and_native_checked(
            map_name,
            difficulty,
            campaign_context,
            progress,
            native,
        )
    }

    fn load_map_with_script_difficulty_and_native<F>(
        &mut self,
        map_name: &str,
        difficulty: Option<gamelogic::player::GameDifficulty>,
        mut progress: F,
        native: Option<&mut dyn crate::game_logic::NativeMapAdmission>,
    ) -> bool
    where
        F: FnMut(f32, &str),
    {
        self.load_map_with_script_difficulty_and_native_checked(
            map_name,
            difficulty,
            crate::save_load::CampaignMapContext::default(),
            |_, _audio_policy, value, phase| {
                progress(value, phase);
                Ok(())
            },
            native,
        )
        .unwrap_or_else(|error| {
            log::error!("Map admission failed: {error}");
            false
        })
    }

    fn load_map_with_script_difficulty_and_native_checked<F>(
        &mut self,
        map_name: &str,
        difficulty: Option<gamelogic::player::GameDifficulty>,
        campaign_context: crate::save_load::CampaignMapContext,
        progress: F,
        native: Option<&mut dyn crate::game_logic::NativeMapAdmission>,
    ) -> Result<bool, String>
    where
        F: FnMut(
            &mut game_engine::common::random_value::RandomState,
            &game_engine::common::audio::script_policy::ScriptAudioPolicy,
            f32,
            &str,
        ) -> Result<(), String>,
    {
        let services = Arc::clone(&self.world_services);
        gamelogic::system::engine_stores::with_world_services(&services, || {
            self.load_map_with_definition_source_and_progress_checked(
                map_name,
                None,
                difficulty,
                None,
                native,
                campaign_context,
                progress,
            )
        })
    }

    /// Saved geometry stays on the extracted map; only companion INIs use the
    /// original source, as in C++ GameLogic::loadMapINI saved-directory branch.
    pub(crate) fn load_map_with_definition_source(&mut self, map_name: &str, source: &str) -> bool {
        self.load_map_with_definition_source_and_water_delivery(map_name, source, None)
    }

    pub(crate) fn load_map_with_definition_source_and_water_delivery(
        &mut self,
        map_name: &str,
        source: &str,
        water_delivery: Option<&mut dyn FnMut(&gamelogic::terrain_water::WaterGridState)>,
    ) -> bool {
        self.load_map_with_definition_source_and_water_delivery_and_native(
            map_name,
            source,
            water_delivery,
            None,
        )
    }

    pub(crate) fn load_map_with_definition_source_and_water_delivery_and_native(
        &mut self,
        map_name: &str,
        source: &str,
        water_delivery: Option<&mut dyn FnMut(&gamelogic::terrain_water::WaterGridState)>,
        native: Option<&mut dyn crate::game_logic::NativeMapAdmission>,
    ) -> bool {
        self.load_map_with_definition_source_and_water_delivery_and_native_checked(
            map_name,
            source,
            water_delivery,
            native,
        )
        .unwrap_or_else(|error| {
            log::error!("Map admission failed: {error}");
            false
        })
    }

    pub(crate) fn load_map_with_definition_source_and_water_delivery_and_native_checked(
        &mut self,
        map_name: &str,
        source: &str,
        water_delivery: Option<&mut dyn FnMut(&gamelogic::terrain_water::WaterGridState)>,
        native: Option<&mut dyn crate::game_logic::NativeMapAdmission>,
    ) -> Result<bool, String> {
        self.load_map_with_campaign_context_and_definition_source_and_water_delivery_and_native_checked(
            map_name,
            source,
            crate::save_load::CampaignMapContext::default(),
            water_delivery,
            native,
        )
    }

    pub(crate) fn load_map_with_campaign_context_and_definition_source_and_water_delivery_and_native_checked(
        &mut self,
        map_name: &str,
        source: &str,
        campaign_context: crate::save_load::CampaignMapContext,
        water_delivery: Option<&mut dyn FnMut(&gamelogic::terrain_water::WaterGridState)>,
        native: Option<&mut dyn crate::game_logic::NativeMapAdmission>,
    ) -> Result<bool, String> {
        let difficulty = self
            .owned_script_engine
            .as_ref()
            .map(|engine| engine.get_global_difficulty());
        let services = Arc::clone(&self.world_services);
        gamelogic::system::engine_stores::with_world_services(&services, || {
            self.load_map_with_definition_source_and_progress_checked(
                map_name,
                Some(source),
                difficulty,
                water_delivery,
                native,
                campaign_context,
                |_, _, _, _| Ok(()),
            )
        })
    }

    pub(super) fn load_map_contents_with_definition_source_and_progress<F>(
        &mut self,
        map_name: &str,
        definition_source: Option<&str>,
        map_script_difficulty: Option<gamelogic::player::GameDifficulty>,
        water_delivery: Option<&mut dyn FnMut(&gamelogic::terrain_water::WaterGridState)>,
        mut native: Option<&mut dyn crate::game_logic::NativeMapAdmission>,
        campaign_context: crate::save_load::CampaignMapContext,
        mut report_progress: F,
    ) -> Result<bool, String>
    where
        F: FnMut(
            &mut game_engine::common::random_value::RandomState,
            &game_engine::common::audio::script_policy::ScriptAudioPolicy,
            f32,
            &str,
        ) -> Result<(), String>,
    {
        if !self.objects.registered_is_empty() || self.has_pending_object_removals() {
            log::error!("Map contents require completed outgoing Object retirement");
            return Ok(false);
        }
        if self.receiving_module_content.is_some() && self.receiving_template_baseline.is_none() {
            log::error!("Receiving map admission has no accepted host template baseline");
            return Ok(false);
        }
        // The interpreter belongs to the session even when map decoding fails.
        // A loan must be returned before destructive map admission can begin.
        let Some(engine) = self.owned_script_engine.as_mut() else {
            log::error!("Map admission requires the receiving ScriptEngine to be returned");
            return Ok(false);
        };
        // ScriptEngine.cpp5252–5405 resets map runtime/lists, not the owning
        // interpreter or its difficulty/debug controls. Core reset currently
        // clears those controls too, so preserve their actual receiving values.
        let retained_difficulty = engine.get_global_difficulty();
        let retained_debug = engine.is_time_frozen_debug();
        engine.reset();
        engine.set_global_difficulty(retained_difficulty);
        engine.set_time_frozen_debug(retained_debug);
        report_progress(
            &mut self.session_view.audio_random,
            self.queued_audio_events.policy(),
            0.26,
            "Preparing map data",
        )?;
        log::info!("Loading map: {}", map_name);
        let load_started = Instant::now();
        // A failed decode must never leave the previous map marked playable.
        // `load_map_with_progress` mutates the world incrementally, so the
        // successful tail below is the only place allowed to set this back to
        // true. This matters especially for a selected-map → default-map
        // fallback: if both attempts fail, callers must see that no map loaded.
        self.map_loaded = false;
        self.finish_all_building_sounds();
        self.objects
            .retire_owned_scalar_stamps(&mut self.partition_manager);
        self.objects.clear();
        self.ai_groups.clear_groups();
        self.partition_manager =
            crate::game_logic::partition_manager::PartitionManager::with_scalar_owner(
                self.world_services.shroud(),
            );
        self.partition_manager
            .set_map_extent(self.world_min, self.world_max);
        self.world_services
            .shroud()
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .reset_for_new_game();
        self.session_view.camera.movement_policy.clear_map_rules();
        self.beacon_client.clear();
        if self.receiving_module_content.is_some() {
            if let Some(baseline) = &self.receiving_template_baseline {
                self.templates = baseline.clone();
            }
        }
        // Direct map admission also enters without the broader engine reset.
        // Restore original base versions before ordered Map/Solo declarations.
        if let Some(content) = self.receiving_module_content.as_mut() {
            content.reset_map_overrides();
        }
        if let Some(definitions) = self.receiving_object_definitions.as_mut() {
            definitions.reset_map_overrides();
        }
        self.refresh_receiving_client_templates();
        self.physics_definitions.effective = self.physics_definitions.base;
        self.locomotor_definitions = self.locomotor_definition_base.clone();
        if let Err(error) = self.restore_projectile_definition_base() {
            log::error!("Cannot restore receiving projectile base: {error}");
            return Ok(false);
        }
        self.body_damage_definitions.effective = self.body_damage_definitions.base;
        self.combat_particles.reset_body_auto_particle_map_rules();
        self.restore_combat_bonus_baseline();
        self.restore_tunnel_capacity_baseline();
        self.lighting_definitions.effective = self.lighting_definitions.base.clone();
        self.terrain_road_definitions.effective = self.terrain_road_definitions.base.clone();
        self.reset_ocl_map_definitions();
        self.named_script_cache.clear();
        self.construction_events.reset();
        self.map_name = map_name.to_string();
        self.map_definition_source = Some(definition_source.unwrap_or(map_name).to_string());
        self.pathfinding_height_samples = None;
        // A new map must not inherit the preceding map's raw flight surface.
        self.terrain = None;
        self.runtime_terrain_texture_classes.clear();
        self.configure_victory_rules_for_map(map_name, &campaign_context);
        self.scripts_loaded = false;
        self.script_event_pump_in_flight
            .store(false, Ordering::Release);
        self.script_event_pump_busy_frames = 0;
        self.loaded_script_lists.clear();
        // The admission retains actual interpreter difficulty even across a
        // failed selected-map attempt. C++ reset/newMap do not reset it.
        // Keep the reset receiving interpreter installed through decode errors.
        // Successful script-definition admission replaces it at its existing phase.
        self.objects_receive_difficulty_bonus = true;
        self.set_script_input_enabled(true);
        self.script_source_path = None;
        self.script_broadcasts.clear();
        self.new_script_messages.clear();
        self.pending_popup_messages.clear();
        self.pending_view_guardband = None;
        self.native_view_guardband = [0.; 2];
        self.pending_camera_bw_mode = None;
        self.pending_camera_motion_blur.clear();
        self.script_skybox_enabled = true;
        self.script_object_count_changed_frame = 0;
        self.script_cameo_flash_count.clear();
        self.script_named_timers.clear();
        self.script_named_timer_display_shown = true;
        self.script_superweapon_display_enabled = true;
        self.script_superweapon_hidden_objects.clear();
        self.eva_superweapon_science_hidden.clear();
        self.mission_objectives = self.load_campaign_objectives(map_name, &campaign_context);
        self.rebuild_objective_lookup();

        // Try to locate the real map file so scripts and future terrain loaders have a source.
        report_progress(
            &mut self.session_view.audio_random,
            self.queued_audio_events.policy(),
            0.30,
            "Resolving map resources",
        )?;
        let resolved_map = super::script_loader::find_map_file(map_name);
        let mut decoded_map = None;
        if let Some(path) = &resolved_map {
            log::info!("Resolved map '{}' to '{}'", map_name, path.display());
            if let Ok(Some(chunky)) = super::script_loader::load_chunky_map(map_name) {
                // An ordinary bare/virtual name must retain the directory
                // actually resolved by this decode for future saved companions.
                // Saved pristine identities remain explicit, independent from
                // the extracted ChunkyMap geometry source.
                let definition_source = definition_source.map_or_else(
                    || chunky.source.to_string_lossy().into_owned(),
                    str::to_string,
                );
                self.map_definition_source = Some(definition_source.clone());
                let companion_sources = if self.receiving_module_content.is_some() {
                    match super::script_loader::read_receiving_definition_companions(
                        &definition_source,
                    ) {
                        Ok(sources) => sources,
                        Err(error) => {
                            log::error!("Cannot read receiving map definitions: {error}");
                            return Ok(false);
                        }
                    }
                } else {
                    super::script_loader::read_weapon_override_companions(&definition_source)
                };
                let weapon_overrides: Vec<_> = companion_sources
                    .iter()
                    .map(|(_, text)| text.clone())
                    .collect();
                let mut damage_fx = self.damage_fx_base.clone();
                for source in &weapon_overrides {
                    if let Err(error) = damage_fx.apply_source(source) {
                        log::error!("Cannot admit receiving map DamageFX definitions: {error}");
                        return Ok(false);
                    }
                }
                self.damage_fx_definitions = damage_fx;
                if let Err(error) = self.admit_locomotor_companion_sources(&weapon_overrides) {
                    log::error!("Cannot admit receiving map locomotors: {error}");
                    return Ok(false);
                }
                if let Some(base) = self.player_template_definitions.as_ref() {
                    let mut candidate = base.clone();
                    for source in &weapon_overrides {
                        if let Err(error) = candidate.apply_companion_source(source) {
                            log::error!("Cannot admit receiving map PlayerTemplate rules: {error}");
                            return Ok(false);
                        }
                    }
                    self.player_template_definitions = Some(candidate);
                }
                if let Err(error) = self.admit_body_damage_companion_sources(&weapon_overrides) {
                    log::error!("Cannot admit receiving map Body rules: {error}");
                    return Ok(false);
                }
                if let Err(error) = self.admit_tunnel_capacity_companion_sources(&weapon_overrides)
                {
                    log::error!("Cannot admit receiving map tunnel capacity: {error}");
                    return Ok(false);
                }
                if let Err(error) = self.admit_combat_bonus_companion_sources(&weapon_overrides) {
                    log::error!("Cannot admit receiving map combat bonuses: {error}");
                    return Ok(false);
                }
                if let Err(error) = self.admit_lighting_companion_sources(&weapon_overrides) {
                    log::error!("Cannot admit receiving map lighting rules: {error}");
                    return Ok(false);
                }
                if let Err(error) = self.apply_receiving_module_overrides(&weapon_overrides) {
                    log::error!("Cannot admit receiving map Object modules: {error}");
                    return Ok(false);
                }
                if let Err(error) = self.apply_camera_input_overrides(&weapon_overrides) {
                    log::error!("Cannot admit receiving map camera input overrides: {error}");
                    return Ok(false);
                }
                if let Err(error) = self.apply_camera_movement_overrides(&weapon_overrides) {
                    log::error!("Cannot admit receiving map camera movement rule: {error}");
                    return Ok(false);
                }
                if let Err(error) = self.apply_receiving_ocl_overrides(&weapon_overrides) {
                    log::error!("Cannot admit receiving map OCL overrides: {error}");
                    return Ok(false);
                }
                if let Err(error) = self.apply_receiving_weapon_overrides(&weapon_overrides) {
                    log::error!("Cannot admit map weapon overrides: {error}");
                    return Ok(false);
                }
                #[cfg(feature = "game_client")]
                if let Err(error) = self.admit_fx_companion_sources(&weapon_overrides) {
                    log::error!("Cannot admit receiving map FX definitions: {error}");
                    return Ok(false);
                }
                if let Some(chunks) = super::script_loader::inspect_map_chunks_from_chunky(&chunky)
                {
                    log::debug!(
                        "Map '{}' contains chunky sections: {}",
                        map_name,
                        chunks.join(", ")
                    );
                }
                report_progress(
                    &mut self.session_view.audio_random,
                    self.queued_audio_events.policy(),
                    0.34,
                    "Parsing map chunks",
                )?;
                log::info!(
                    "Map '{}' parsed: {} TOC entries, body offset {} bytes",
                    map_name,
                    chunky.toc.len(),
                    chunky.body_offset
                );
                // Replace the test map with parsed object placements for basic fidelity.
                let settings_started = Instant::now();
                report_progress(
                    &mut self.session_view.audio_random,
                    self.queued_audio_events.policy(),
                    0.36,
                    "Reading map settings",
                )?;
                let mut ai_draft = self.ai_definitions.map_override_draft();
                let parsed =
                    super::script_loader::parse_map_settings_from_chunky_with_owned_definition_catalogs(
                        &chunky,
                        &mut ai_draft,
                        &definition_source,
                        Some(*self.production_energy_rules()),
                        Some(self.power_bar_baseline()),
                        Some(self.combat_particles.body_auto_particle_baseline().clone()),
                        Some(self.physics_definitions.effective),
                        Some(self.session_view.camera.settings.current()),
                        Some(self.session_view.camera.clip_policy.current()),
                        Some(self.session_view.camera.shake_policy.current()),
                        Arc::clone(self.effective_terrain_definitions()),
                        self.terrain_road_baseline().clone(),
                        self.current_armor_definitions().cloned(),
                        self.receiving_module_content.as_ref().map(|_| companion_sources.as_slice()),
                        self.upgrade_definition_base.clone(),
                        self.receiving_module_content
                            .as_ref()
                            .map(|content| content.science_facts())
                            .unwrap_or_default(),
                        self.player_template_definitions.is_some(),
                    );
                let parsed_settings = match parsed {
                    Ok(meta) => match self.admit_ai_map_overrides(ai_draft) {
                        Ok(()) => {
                            if let Some(lighting) = &meta.lighting {
                                self.apply_map_lighting(lighting);
                            }
                            if let Some(definitions) = &meta.upgrade_definitions {
                                self.upgrade_definitions = definitions.clone();
                            }
                            if let Some(definitions) = &meta.armor_definitions {
                                self.admit_armor_definitions(Arc::clone(definitions));
                            }
                            if let Some(definitions) = &meta.terrain_definitions {
                                self.terrain_definitions = Arc::clone(definitions);
                            }
                            if let Some(definitions) = &meta.road_definitions {
                                self.terrain_road_definitions.effective = definitions.clone();
                            }
                            self.weather_definition_updates =
                                meta.weather_definition_updates.clone();
                            if let Some(rules) = meta.camera_shake_rules {
                                if let Err(error) = self.admit_camera_map_shake(rules) {
                                    log::warn!("Map camera shake admission failed: {error}");
                                    return Ok(false);
                                }
                            }
                            if let Some(rules) = meta.camera_clip_rules {
                                self.admit_camera_map_clip(rules);
                            }
                            if let Some(rules) = meta.camera_rules {
                                if let Err(error) = self.admit_camera_map_rules(rules) {
                                    log::warn!("Map camera rules admission failed: {error}");
                                    return Ok(false);
                                }
                            }
                            if let Some(rules) = meta.physics_rules {
                                if let Err(error) = self.admit_physics_map_rules(rules) {
                                    log::warn!("Map Physics admission failed: {error}");
                                    return Ok(false);
                                }
                            }
                            if let Some(rules) = meta.production_energy {
                                self.production_energy.effective = rules;
                            }
                            if let Some(rules) = meta.power_bar_rules {
                                self.power_bar_definitions.effective = rules;
                            }
                            if let Some(rules) = &meta.body_auto_particle_rules {
                                self.combat_particles
                                    .apply_body_auto_particle_map_rules(rules.clone());
                            }
                            Some(meta)
                        }
                        Err(err) => {
                            log::warn!("Map AI definition admission failed: {err}");
                            return Ok(false);
                        }
                    },
                    Err(err) => {
                        log::warn!("Map settings parse failed: {err}");
                        return Ok(false);
                    }
                };
                log::info!(
                    "Map '{}' settings parse finished in {:.2}s (present={})",
                    map_name,
                    settings_started.elapsed().as_secs_f32(),
                    parsed_settings.is_some()
                );
                if let Some(meta) = parsed_settings.as_ref() {
                    self.admit_map_starting_cash(meta.default_starting_cash);
                }
                if self.game_mode != GameMode::Shell {
                    report_progress(
                        &mut self.session_view.audio_random,
                        self.queued_audio_events.policy(),
                        0.40,
                        "Syncing runtime objects",
                    )?;
                } else {
                    report_progress(
                        &mut self.session_view.audio_random,
                        self.queued_audio_events.policy(),
                        0.40,
                        "Syncing shell runtime",
                    )?;
                }
                let sync_started = Instant::now();
                if !self.sync_legacy_runtime_from_fast_chunky_with_water_delivery(
                    path,
                    &chunky,
                    map_script_difficulty,
                    water_delivery,
                    native.as_mut().map(|native| {
                        &mut **native as &mut dyn crate::game_logic::NativeMapAdmission
                    }),
                    parsed_settings.as_ref(),
                    &mut report_progress,
                )? {
                    return Ok(false);
                }
                log::info!(
                    "Map '{}' legacy runtime sync finished in {:.2}s (fast path)",
                    map_name,
                    sync_started.elapsed().as_secs_f32()
                );

                let heightmap_started = Instant::now();
                report_progress(
                    &mut self.session_view.audio_random,
                    self.queued_audio_events.policy(),
                    0.46,
                    "Parsing terrain heightmap",
                )?;
                let heightmap_data =
                    super::script_loader::parse_heightmap_data_from_chunky(&chunky)
                        .ok()
                        .flatten();
                let blend_tile_data = heightmap_data.as_ref().and_then(|hm| {
                    match super::script_loader::parse_blend_tile_data_from_chunky(&chunky, hm) {
                        Ok(value) => value,
                        Err(err) => {
                            log::warn!("Map '{}' BlendTileData parse failed: {}", map_name, err);
                            None
                        }
                    }
                });
                self.runtime_terrain_texture_classes = blend_tile_data
                    .as_ref()
                    .map(|blend| blend.texture_classes.clone())
                    .unwrap_or_default();
                log::info!(
                    "Map '{}' heightmap parse finished in {:.2}s (heightmap_present={}, blend_tiles_present={})",
                    map_name,
                    heightmap_started.elapsed().as_secs_f32(),
                    heightmap_data.is_some(),
                    blend_tile_data.is_some()
                );

                let bounds_override = super::script_loader::resolved_map_height_bounds(
                    parsed_settings.as_ref(),
                    heightmap_data
                        .as_ref()
                        .map(|map| (map.width, map.height, map.border_size)),
                );
                if let Some((min, max)) = bounds_override {
                    self.world_min = min;
                    self.world_max = max;
                    self.world_width = (max.x - min.x).max(1.0);
                    self.world_height = (max.z - min.z).max(1.0);
                } else {
                    self.world_min =
                        Vec3::new(-self.world_width * 0.5, 0.0, -self.world_height * 0.5);
                    self.world_max =
                        Vec3::new(self.world_width * 0.5, 0.0, self.world_height * 0.5);
                }
                if let Err(error) = self.initialize_receiving_partition_grid() {
                    log::error!("Cannot initialize receiving partition grid: {error}");
                    return Ok(false);
                }
                if let Some(meta) = parsed_settings.as_ref() {
                    self.last_map_settings = Some(meta.clone());
                    log::info!(
                        "Map '{}' metadata: objects={}, heightmap_hint={:?}, world_min={:?}, world_max={:?}",
                        map_name,
                        meta.objects.len(),
                        meta.heightmap_path,
                        meta.world_min,
                        meta.world_max
                    );
                    let objects = &meta.objects;
                    if !objects.is_empty() {
                        let named_count = objects.iter().filter(|obj| obj.name.is_some()).count();
                        if named_count > 0 {
                            log::info!(
                                "Map '{}' contains {} named object placements",
                                map_name,
                                named_count
                            );
                        }
                        let object_spawn_started = Instant::now();
                        report_progress(
                            &mut self.session_view.audio_random,
                            self.queued_audio_events.policy(),
                            0.58,
                            "Spawning world objects",
                        )?;
                        // Singleton onCreate actions may already have created
                        // objects during PlayerList admission. Preserve them.

                        // Build a mapping from map-defined player IDs to teams.
                        let mut map_player_to_team: HashMap<u32, Team> = HashMap::new();
                        for obj in objects {
                            if let Some(pid) = obj.player_id {
                                if let Some(team) =
                                    obj.team_name.as_deref().and_then(Self::team_from_string)
                                {
                                    map_player_to_team.entry(pid).or_insert(team);
                                }
                            }
                        }
                        // Wave 830: seed player→team from skirmish slots when map
                        // team_name strings are missing / unparseable (Lone Eagle).
                        for (pid, player) in &self.players {
                            if *pid == 0 || player.team != Team::Neutral {
                                map_player_to_team.entry(*pid).or_insert(player.team);
                            }
                        }
                        // Common skirmish residual: player 0 human USA, 1 AI GLA.
                        if self.uses_lobby_side_foundation() {
                            map_player_to_team.entry(0).or_insert(Team::USA);
                            map_player_to_team.entry(1).or_insert(Team::GLA);
                        }
                        // Seed players from map ownership only when no skirmish/host
                        // players were already configured. Wiping would destroy
                        // apply_skirmish_config slots/AI on Lone Eagle-style loads.
                        if !map_player_to_team.is_empty() {
                            let preserve_host_players = (self.uses_lobby_side_foundation()
                                || self.game_mode == GameMode::SinglePlayer
                                || self.prepared_side_identity_is_complete())
                                && !self.players.is_empty();
                            if preserve_host_players {
                                log::info!(
                                    "Preserving {} host player(s) across map load (skirmish/SP config)",
                                    self.players.len()
                                );
                                // C++ prepares occupied Skirmish slots from GameInfo,
                                // not the authored map's Civilian/player dictionaries.
                                if self.game_mode != GameMode::Skirmish
                                    && (self.uses_lobby_side_foundation()
                                        || !self.prepared_side_identity_is_complete())
                                {
                                    self.apply_host_players_from_sides_list(false);
                                }
                            } else {
                                self.players.clear();
                                self.select_local_player(None);
                                for (&pid, &team) in &map_player_to_team {
                                    let is_local = pid == 0;
                                    let name = format!("Player{}", pid + 1);
                                    self.players
                                        .insert(pid, Player::new(pid, team, &name, is_local));
                                }
                                let selected = self.players.contains_key(&0).then_some(0);
                                self.select_local_player(selected);
                                self.apply_host_players_from_sides_list(true);
                            }
                            self.stash_side_builds_on_host(&meta.side_builds);
                        }

                        let mut spawned_object_ids: Vec<(ObjectId, usize)> = Vec::new();
                        let total_objects = objects.len().max(1) as f32;
                        for (index, obj) in objects.iter().enumerate() {
                            if index % 4 == 0 {
                                let t = (index as f32 / total_objects).clamp(0.0, 1.0);
                                report_progress(
                                    &mut self.session_view.audio_random,
                                    self.queued_audio_events.policy(),
                                    0.58 + t * 0.20,
                                    "Spawning world objects",
                                )?;
                            }
                            if Self::apply_map_object_scorch(obj) {
                                continue;
                            }

                            let mut spawn_position =
                                Vec3::new(obj.position.x, obj.position.z, obj.position.y);
                            if let Some(ground_height) = self.terrain_ground_height_at(Vec3::new(
                                spawn_position.x,
                                0.0,
                                spawn_position.z,
                            )) {
                                // Match C++ map-object placement: map z-offset sits on top of terrain.
                                spawn_position.y += ground_height;
                            }
                            let mut effects = native
                                .as_mut()
                                .map(|native| native.effects())
                                .unwrap_or_default();
                            let created = match self.create_placed_map_object_with_effects_checked(
                                obj,
                                spawn_position,
                                &mut effects,
                            ) {
                                Ok(created) => created,
                                Err(error) => {
                                    log::error!("Map object construction rejected: {error}");
                                    return Ok(false);
                                }
                            };
                            if let Some(birth) = created {
                                let id = birth.id;
                                spawned_object_ids.push((id, index));
                                self.apply_update_obj_values_from_map_properties_with_effects(
                                    id,
                                    &obj.properties,
                                    &mut native
                                        .as_mut()
                                        .map(|native| native.effects())
                                        .unwrap_or_default(),
                                );
                                if obj.unsellable == Some(true) {
                                    if let Some(created) = self.objects.get_mut(&id) {
                                        created.set_script_unsellable(true);
                                    }
                                }
                                if obj.enabled == Some(false) {
                                    if let Some(created) = self.objects.get_mut(&id) {
                                        created.set_script_disabled_with_partition_owner(
                                            true,
                                            &mut self.partition_manager,
                                        );
                                    }
                                }
                                if obj.powered == Some(false) {
                                    if let Some(created) = self.objects.get_mut(&id) {
                                        created.set_script_underpowered_with_partition_owner(
                                            true,
                                            &mut self.partition_manager,
                                        );
                                    }
                                }
                                if let Some(indestructible) = obj.indestructible {
                                    self.set_object_indestructible(id, indestructible);
                                }

                                self.apply_spawned_object_weather(
                                    id,
                                    obj.object_weather.unwrap_or(0),
                                );

                                if let Some(name) =
                                    obj.name.as_deref().map(str::trim).filter(|n| !n.is_empty())
                                {
                                    self.set_host_object_name(id, name);
                                }

                                if let Some(rot) = obj.rotation {
                                    if let Some(created) = self.objects.get_mut(&id) {
                                        created.set_orientation_with_partition_owner(
                                            rot,
                                            &mut self.partition_manager,
                                        );
                                    }
                                }
                                if let Some(upgrade) = obj.upgrade.as_deref() {
                                    // ObjectCreationList encodes upgrade/facing hints in a freeform string.
                                    // Apply all upgrades separated by commas/semicolons and treat a numeric-only
                                    // token as a facing override if the chunk omitted rotation.
                                    let mut applied_facing = false;
                                    for token in upgrade.split(&[',', ';'][..]) {
                                        let trimmed = token.trim();
                                        if trimmed.is_empty() {
                                            continue;
                                        }
                                        if !applied_facing && obj.rotation.is_none() {
                                            if let Ok(angle) = trimmed.parse::<f32>() {
                                                if let Some(created) = self.objects.get_mut(&id) {
                                                    created.set_orientation_with_partition_owner(
                                                        angle,
                                                        &mut self.partition_manager,
                                                    );
                                                }
                                                applied_facing = true;
                                                continue;
                                            }
                                        }
                                        self.apply_upgrade_to_object_with_effects(
                                            id,
                                            trimmed,
                                            &mut native
                                                .as_mut()
                                                .map(|native| native.effects())
                                                .unwrap_or_default(),
                                        );
                                    }
                                }
                                if let Err(error) = birth.activate(self) {
                                    log::error!("Map Team activation rejected: {error}");
                                    return Ok(false);
                                }
                            }
                        }
                        report_progress(
                            &mut self.session_view.audio_random,
                            self.queued_audio_events.policy(),
                            0.80,
                            "World objects spawned",
                        )?;
                        let spawned_ids = spawned_object_ids;
                        self.spawned_map_object_ids = spawned_ids.clone();
                        if let Err(error) = self
                            .register_spawned_landmark_bridges_with_effects_checked(
                                objects,
                                &spawned_ids,
                                &mut native
                                    .as_mut()
                                    .map(|native| native.effects())
                                    .unwrap_or_default(),
                            )
                        {
                            log::error!("Map native child construction rejected: {error}");
                            return Ok(false);
                        }
                        // Wave 831: SidesList build-list faction bases (skirmish armies).
                        let side_spawned =
                            self.spawn_side_build_list(&meta.side_builds, &map_player_to_team);
                        if side_spawned > 0 {
                            log::info!(
                                "Spawned {} SidesList build-list objects for '{}'",
                                side_spawned,
                                map_name
                            );
                        }
                        // Wave 831: starting yard+dozer at Player_N_Start waypoints.
                        if let Err(error) = self.spawn_skirmish_starting_units_with_effects_checked(
                            &mut native
                                .as_mut()
                                .map(|native| native.effects())
                                .unwrap_or_default(),
                        ) {
                            log::error!("Map starting object rejected: {error}");
                            return Ok(false);
                        }
                        report_progress(
                            &mut self.session_view.audio_random,
                            self.queued_audio_events.policy(),
                            0.82,
                            "Finalizing world objects",
                        )?;
                        self.ensure_non_shell_player_presence_with_effects(
                            parsed_settings.as_ref(),
                            &mut native
                                .as_mut()
                                .map(|native| native.effects())
                                .unwrap_or_default(),
                        );
                        log::info!(
                            "Spawned {} objects from map placement data for '{}' in {:.2}s",
                            self.objects.registered_len(),
                            map_name,
                            object_spawn_started.elapsed().as_secs_f32()
                        );
                    }
                }
                let bounds_started = Instant::now();
                report_progress(
                    &mut self.session_view.audio_random,
                    self.queued_audio_events.policy(),
                    0.84,
                    "Building world bounds",
                )?;
                if let Some((min, max)) = bounds_override {
                    self.world_min = min;
                    self.world_max = max;
                    self.world_width = (self.world_max.x - self.world_min.x).max(1.0);
                    self.world_height = (self.world_max.z - self.world_min.z).max(1.0);
                    let mut replacement_pathfinding = PathfindingSystem::new_with_origin(
                        self.world_min,
                        self.world_width,
                        self.world_height,
                    );
                    replacement_pathfinding
                        .grid
                        .inherit_bridge_admission(&mut self.pathfinding_system.grid);
                    self.pathfinding_system = replacement_pathfinding;
                    self.refresh_pathfinding_ai_definitions();
                    log::info!(
                        "Map '{}' bounds set to min({:.1},{:.1},{:.1}) max({:.1},{:.1},{:.1})",
                        map_name,
                        self.world_min.x,
                        self.world_min.y,
                        self.world_min.z,
                        self.world_max.x,
                        self.world_max.y,
                        self.world_max.z
                    );

                    #[cfg(not(feature = "game_client"))]
                    if let Some(hm) = heightmap_data.as_ref() {
                        self.terrain = super::terrain::TerrainData::from_raw_heightmap(
                            hm.width.max(1) as u32,
                            hm.height.max(1) as u32,
                            &hm.data,
                            self.world_min,
                            self.world_max,
                            hm.border_size.max(0) as u32,
                        );
                    }

                    #[cfg(feature = "game_client")]
                    if let Some(hm) = heightmap_data.as_ref() {
                        use gamelogic::common::{MAP_HEIGHT_SCALE, MAP_XY_FACTOR};
                        let width = hm.width.max(1) as u32;
                        let height = hm.height.max(1) as u32;
                        if hm.data.len() == (width * height) as usize {
                            let max_height = 255.0 * MAP_HEIGHT_SCALE;
                            // C++ MapObject.h MAP_XY_FACTOR=10; WorldHeightMap
                            // stores map-authored border (ZH Alpine = 70).
                            // HeightMap::new defaults scale=1 / border=0; the
                            // visual freeze copies these fields verbatim.
                            let mut heightmap = game_client::terrain::height_map::HeightMap::new(
                                width,
                                height,
                                max_height,
                                MAP_XY_FACTOR,
                            );
                            apply_cpp_heightmap_xy_and_border(&mut heightmap, hm.border_size);
                            heightmap.heights = hm.data.iter().map(|h| *h as f32 / 255.0).collect();
                            if let Some(blend) = blend_tile_data.as_ref() {
                                if blend.tile_ndxes.len() == heightmap.tile_ndxes.len() {
                                    heightmap.tile_ndxes = blend.tile_ndxes.clone();
                                }
                                if blend.blend_tile_ndxes.len() == heightmap.blend_tile_ndxes.len()
                                {
                                    heightmap.blend_tile_ndxes = blend.blend_tile_ndxes.clone();
                                }
                                if blend.extra_blend_tile_ndxes.len()
                                    == heightmap.extra_blend_tile_ndxes.len()
                                {
                                    heightmap.extra_blend_tile_ndxes =
                                        blend.extra_blend_tile_ndxes.clone();
                                }
                                if !blend.blended_tiles.is_empty() {
                                    let mut tiles =
                                        vec![game_client::terrain::textures::BlendTileInfo::new()];
                                    tiles.extend(blend.blended_tiles.iter().map(|t| {
                                        game_client::terrain::textures::BlendTileInfo {
                                            blend_ndx: t.blend_ndx,
                                            horiz: t.horiz,
                                            vert: t.vert,
                                            right_diagonal: t.right_diagonal,
                                            left_diagonal: t.left_diagonal,
                                            inverted: t.inverted,
                                            long_diagonal: t.long_diagonal,
                                            custom_blend_edge_class: t.custom_blend_edge_class,
                                        }
                                    }));
                                    heightmap.assign_blended_tiles(tiles);
                                }
                            }

                            let border = heightmap.border_size.max(0) as u32;
                            self.terrain = Some(super::terrain::TerrainData::from_heightmap(
                                heightmap,
                                self.world_min,
                                self.world_max,
                                border,
                            ));
                            self.copy_crate_water_into_host_terrain();
                            if let Some(meta) = self.last_map_settings.clone() {
                                let spawned_map_object_ids = self.spawned_map_object_ids.clone();
                                self.ground_loaded_map_objects_to_terrain(
                                    &meta.objects,
                                    &spawned_map_object_ids,
                                    &mut native
                                        .as_mut()
                                        .map(|native| native.effects())
                                        .unwrap_or_default(),
                                );
                            }
                            if let Err(error) = self
                                .seed_pathfinding_from_terrain_with_effects_checked(
                                    &mut native
                                        .as_mut()
                                        .map(|native| native.effects())
                                        .unwrap_or_default(),
                                )
                            {
                                log::error!("Map native child construction rejected: {error}");
                                return Ok(false);
                            }
                            let wall_height = self.pathfind_ai_rules().wall_height;
                            ({
                                let ground_owner = self.world_services.terrain();
                                let ground_loan = ground_owner
                                    .read()
                                    .unwrap_or_else(|error| error.into_inner());
                                let ground_source = if self.map_loaded
                                    || ground_loan.has_height_map()
                                {
                                    crate::game_logic::pathfinding::PathfindingGround::receiving(
                                        &ground_loan,
                                    )
                                } else {
                                    crate::game_logic::pathfinding::PathfindingGround::mapless(
                                        self.terrain.as_ref(),
                                        self.pathfinding_height_samples.as_ref(),
                                        &self.pathfinding_system.grid,
                                    )
                                };
                                self.pathfinding_system.apply_structure_static_blocks(
                                    self.objects.read(),
                                    wall_height,
                                    &ground_source,
                                )
                            });
                        }
                    }
                } else {
                    // Default symmetrical bounds based on current width/height.
                    self.world_min =
                        Vec3::new(-self.world_width * 0.5, 0.0, -self.world_height * 0.5);
                    self.world_max =
                        Vec3::new(self.world_width * 0.5, 0.0, self.world_height * 0.5);
                    let mut replacement_pathfinding = PathfindingSystem::new_with_origin(
                        self.world_min,
                        self.world_width,
                        self.world_height,
                    );
                    replacement_pathfinding
                        .grid
                        .inherit_bridge_admission(&mut self.pathfinding_system.grid);
                    self.pathfinding_system = replacement_pathfinding;
                    self.refresh_pathfinding_ai_definitions();
                }

                self.refresh_receiving_object_map_status();
                report_progress(
                    &mut self.session_view.audio_random,
                    self.queued_audio_events.policy(),
                    0.88,
                    "Initializing shroud and pathfinding",
                )?;
                log::info!(
                    "Map '{}' bounds/terrain/shroud hookup finished in {:.2}s",
                    map_name,
                    bounds_started.elapsed().as_secs_f32()
                );
                decoded_map = Some(chunky);
            } else {
                log::error!(
                    "Map '{}' was found at '{}' but could not be decoded as a chunky map",
                    map_name,
                    path.display()
                );
                return Ok(false);
            }
        } else {
            // Development-only fallback maps: keep the legacy test layout for demos.
            if matches!(map_name, "TestMap" | "demo_map") {
                log::warn!(
                    "Map '{}' not found on disk; using built-in test layout",
                    map_name
                );
                self.create_test_map();
            } else {
                log::warn!("Map '{}' not found on disk", map_name);
                return Ok(false);
            }
        }

        // Terrain hookup: if a heightmap path was discovered next to the map, load it for height
        // queries and derive a first-pass impassability mask for the pathfinding grid.
        #[cfg(feature = "game_client")]
        {
            if self.terrain.is_none() {
                if let Some(heightmap_path) = self.heightmap_hint() {
                    if let Some(path_str) = heightmap_path.to_str() {
                        let loaded = if path_str.ends_with(".hmp") {
                            game_client::terrain::height_map::HeightMap::load_hmp(path_str).ok()
                        } else if path_str.ends_with(".tga") {
                            game_client::terrain::height_map::HeightMap::load_tga(path_str).ok()
                        } else if path_str.ends_with(".raw") {
                            game_client::terrain::height_map::HeightMap::load_raw(path_str).ok()
                        } else {
                            None
                        };

                        if let Some(mut heightmap) = loaded {
                            let border_size = heightmap.border_size;
                            apply_cpp_heightmap_xy_and_border(&mut heightmap, border_size);
                            let border = heightmap.border_size.max(0) as u32;
                            let terrain = super::terrain::TerrainData::from_heightmap(
                                heightmap,
                                self.world_min,
                                self.world_max,
                                border,
                            );
                            self.terrain = Some(terrain);
                            self.copy_crate_water_into_host_terrain();
                            if let Some(meta) = self.last_map_settings.clone() {
                                let spawned_map_object_ids = self.spawned_map_object_ids.clone();
                                self.ground_loaded_map_objects_to_terrain(
                                    &meta.objects,
                                    &spawned_map_object_ids,
                                    &mut native
                                        .as_mut()
                                        .map(|native| native.effects())
                                        .unwrap_or_default(),
                                );
                            }
                            if let Err(error) = self
                                .seed_pathfinding_from_terrain_with_effects_checked(
                                    &mut native
                                        .as_mut()
                                        .map(|native| native.effects())
                                        .unwrap_or_default(),
                                )
                            {
                                log::error!("Map native child construction rejected: {error}");
                                return Ok(false);
                            }
                            let wall_height = self.pathfind_ai_rules().wall_height;
                            ({
                                let ground_owner = self.world_services.terrain();
                                let ground_loan = ground_owner
                                    .read()
                                    .unwrap_or_else(|error| error.into_inner());
                                let ground_source = if self.map_loaded
                                    || ground_loan.has_height_map()
                                {
                                    crate::game_logic::pathfinding::PathfindingGround::receiving(
                                        &ground_loan,
                                    )
                                } else {
                                    crate::game_logic::pathfinding::PathfindingGround::mapless(
                                        self.terrain.as_ref(),
                                        self.pathfinding_height_samples.as_ref(),
                                        &self.pathfinding_system.grid,
                                    )
                                };
                                self.pathfinding_system.apply_structure_static_blocks(
                                    self.objects.read(),
                                    wall_height,
                                    &ground_source,
                                )
                            });
                        } else {
                            log::warn!("Failed to load heightmap '{}'", path_str);
                        }
                    }
                }
            }
        }

        let scripts_started = Instant::now();
        report_progress(
            &mut self.session_view.audio_random,
            self.queued_audio_events.policy(),
            0.92,
            "Initializing mission scripts",
        )?;
        self.initialize_scripts_with_map_data_and_difficulty(
            map_name,
            decoded_map.as_ref(),
            map_script_difficulty,
        );
        if matches!(self.game_mode, GameMode::Skirmish | GameMode::Replay) {
            // C++ startNewGame adds ReplayObserver after sides, then installs
            // MultiplayerScripts.scb (numTeams>1) and permanently reveals the map.
            self.install_replay_observer_side();
            self.install_multiplayer_scripts_if_needed();
            self.reveal_replay_observer_map();
        }
        if matches!(self.game_mode, GameMode::Replay) {
            self.apply_replay_observer_as_local_player();
        }
        log::info!(
            "Map '{}' script init finished in {:.2}s",
            map_name,
            scripts_started.elapsed().as_secs_f32()
        );

        // Skirmish/SP: map spawn clears world objects. Rebind host AI (stale
        // object/factory refs, rebuild budget) and re-ensure GLA_*/faction templates
        // without wiping players, cash, difficulty, or is_active.
        if self.uses_lobby_side_foundation() || self.is_single_player_scenario() {
            self.rebind_host_ai_after_map_load();
            // C++ GameLogic.cpp placeObjectAtPosition loop for PlayerTemplate StartingUnitN.
            // Without this, Lone Eagle-style maps keep buildings but no dozers/workers.
            if let Err(error) = self.spawn_skirmish_starting_units_with_effects_checked(
                &mut native
                    .as_mut()
                    .map(|native| native.effects())
                    .unwrap_or_default(),
            ) {
                log::error!("Map starting object rejected: {error}");
                return Ok(false);
            }
        }

        // C++ TerrainLogic.cpp:2589 TheRadar->newMap after bounds, then
        // re-add live objects. Stay off sides_list / polygon.
        self.host_radar_on_map_loaded_with_effects(
            &mut native
                .as_mut()
                .map(|native| native.effects())
                .unwrap_or_default(),
        );

        // GameLogic.cpp2060: initial PartitionManager::update before the
        // first script frame. Constructors only attached dirty modules.
        self.update_owned_partition_with_effects(
            &mut native
                .as_mut()
                .map(|native| native.effects())
                .unwrap_or_default(),
        )
        .map_err(|error| error.to_string())?;

        self.map_loaded = true;
        if let Some(draw_sky_box) = self
            .last_map_settings
            .as_ref()
            .and_then(|settings| settings.draw_sky_box)
        {
            self.script_skybox_enabled = draw_sky_box;
        }
        // C++ start-of-match residual: reveal FOW around loaded units/structures
        // immediately so build placement / presentation FOW are not stuck LBC_SHROUD
        // until the first logic tick. Same XZ→shroud mapping as update path.
        // Wave 827: under coupled shadow, host system residuals sole-tick after GW writeback.
        {
            self.update_main_crate_vision();
        }
        report_progress(
            &mut self.session_view.audio_random,
            self.queued_audio_events.policy(),
            0.96,
            "Map load complete",
        )?;
        log::info!(
            "Map loaded successfully in {:.2}s",
            load_started.elapsed().as_secs_f32()
        );
        Ok(true)
    }

    /// Load a map without external progress reporting.
    pub fn load_map(&mut self, map_name: &str) -> bool {
        self.load_map_with_progress(map_name, |_progress, _phase| {})
    }

    /// Admit a map using campaign metadata selected by the receiving session.
    /// This keeps mission objectives and victory rules tied to that session
    /// without consulting ambient campaign state.
    pub fn load_map_with_campaign_context(
        &mut self,
        map_name: &str,
        campaign_context: crate::save_load::CampaignMapContext,
    ) -> bool {
        self.load_map_with_campaign_context_and_progress_and_native_checked(
            map_name,
            campaign_context,
            |_, _, _, _| Ok(()),
            None,
        )
        .unwrap_or_else(|error| {
            log::error!("Map admission failed: {error}");
            false
        })
    }

    /// Load a requested map, then an explicit fallback, returning the identity
    /// that actually loaded. `None` means neither attempt produced a playable
    /// map; it is deliberately not an alias for the fallback name.
    #[inline]
    pub fn load_map_or_fallback(&mut self, map_name: &str, fallback: &str) -> Option<String> {
        self.load_map_or_fallback_with_progress(map_name, fallback, |_progress, _phase| {})
    }

    /// Load a requested map, then an explicit fallback, while exposing the
    /// real map-load milestones from each attempt. The fallback order and
    /// successful-tail requirement intentionally match `load_map_or_fallback`.
    #[inline]
    pub fn load_map_or_fallback_with_progress<F>(
        &mut self,
        map_name: &str,
        fallback: &str,
        mut report_progress: F,
    ) -> Option<String>
    where
        F: FnMut(f32, &str),
    {
        self.load_map_or_fallback_with_progress_and_native(
            map_name,
            fallback,
            report_progress,
            None,
        )
    }

    pub fn load_map_or_fallback_with_progress_and_native<F>(
        &mut self,
        map_name: &str,
        fallback: &str,
        mut report_progress: F,
        mut native: Option<&mut dyn crate::game_logic::NativeMapAdmission>,
    ) -> Option<String>
    where
        F: FnMut(f32, &str),
    {
        self.load_map_or_fallback_with_progress_and_native_checked(
            map_name,
            fallback,
            |_, _audio_policy, value, phase| {
                report_progress(value, phase);
                Ok(())
            },
            native,
        )
        .unwrap_or_else(|error| {
            log::error!("Map fallback admission failed: {error}");
            None
        })
    }

    pub(crate) fn load_map_or_fallback_with_progress_and_native_checked<F>(
        &mut self,
        map_name: &str,
        fallback: &str,
        report_progress: F,
        native: Option<&mut dyn crate::game_logic::NativeMapAdmission>,
    ) -> Result<Option<String>, String>
    where
        F: FnMut(
            &mut game_engine::common::random_value::RandomState,
            &game_engine::common::audio::script_policy::ScriptAudioPolicy,
            f32,
            &str,
        ) -> Result<(), String>,
    {
        self.load_map_or_fallback_with_campaign_contexts_and_progress_and_native_checked(
            map_name,
            fallback,
            crate::save_load::CampaignMapContext::default(),
            crate::save_load::CampaignMapContext::default(),
            report_progress,
            native,
        )
    }

    pub(crate) fn load_map_or_fallback_with_campaign_contexts_and_progress_and_native_checked<F>(
        &mut self,
        map_name: &str,
        fallback: &str,
        requested_campaign_context: crate::save_load::CampaignMapContext,
        fallback_campaign_context: crate::save_load::CampaignMapContext,
        mut report_progress: F,
        mut native: Option<&mut dyn crate::game_logic::NativeMapAdmission>,
    ) -> Result<Option<String>, String>
    where
        F: FnMut(
            &mut game_engine::common::random_value::RandomState,
            &game_engine::common::audio::script_policy::ScriptAudioPolicy,
            f32,
            &str,
        ) -> Result<(), String>,
    {
        // One admission owns both attempts. Capture the receiving difficulty
        // before either attempt resets map runtime.
        let difficulty = self
            .owned_script_engine
            .as_ref()
            .map(|engine| engine.get_global_difficulty());
        if self.load_map_with_script_difficulty_and_native_checked(
            map_name,
            difficulty,
            requested_campaign_context,
            |random, audio_policy, progress, phase| {
                report_progress(random, audio_policy, progress, phase)
            },
            native
                .as_mut()
                .map(|native| &mut **native as &mut dyn crate::game_logic::NativeMapAdmission),
        )? && self.map_loaded
        {
            return Ok(Some(self.map_name.clone()));
        }

        // Do not pretend a second attempt occurred when the requested identity
        // already was the fallback. More importantly, never report a fallback
        // name unless that attempt really reached the successful load tail.
        if map_name != fallback
            && self.load_map_with_script_difficulty_and_native_checked(
                fallback,
                difficulty,
                fallback_campaign_context,
                |random, audio_policy, progress, phase| {
                    report_progress(random, audio_policy, progress, phase)
                },
                native
                    .as_mut()
                    .map(|native| &mut **native as &mut dyn crate::game_logic::NativeMapAdmission),
            )?
            && self.map_loaded
        {
            return Ok(Some(self.map_name.clone()));
        }

        // No world is available. Clear the transient identity set at the start
        // of either failed attempt so UI/save/render callers cannot describe an
        // unloaded map as the active match.
        self.map_name.clear();
        self.map_loaded = false;
        Ok(None)
    }
}

/// Presentation identity for C++ `W3DBridge::load` (`W3DBridgeBuffer.cpp:182-191`).
/// Unit-separated so map template names stay unambiguous.
pub(crate) fn encode_authored_bridge_visual(
    template_name: &str,
    model_name: &str,
    scale: f32,
    towers: [&str; 4],
) -> String {
    format!(
        "AUTHBR\u{1f}{template_name}\u{1f}{model_name}\u{1f}{scale}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
        towers[0], towers[1], towers[2], towers[3]
    )
}

/// C++ PlayerList.cpp:167-199.
pub(in super::super) fn apply_logic_player_list_relationships(
    logic_list: &mut LogicPlayerList,
    side_dicts: &[Dict],
) {
    let mut name_to_index: HashMap<String, i32> = HashMap::new();
    for dict in side_dicts {
        let name = dict.get_ascii_string(key_player_name());
        if name.is_empty() {
            continue;
        }
        if let Some(player) = logic_list.find_player_by_name(&name) {
            if let Ok(guard) = player.read() {
                name_to_index.insert(name, guard.get_player_index());
            }
        }
    }
    for dict in side_dicts {
        let name = dict.get_ascii_string(key_player_name());
        let Some(&index) = name_to_index.get(&name) else {
            continue;
        };
        let Some(player_arc) = logic_list.get_player(index) else {
            continue;
        };
        let Ok(mut player) = player_arc.write() else {
            continue;
        };
        player.set_player_relationship_by_index(index, gamelogic::common::Relationship::Allies);
        if index != 0 {
            player.set_player_relationship_by_index(0, gamelogic::common::Relationship::Neutral);
        }
        for token in dict
            .get_ascii_string(key_player_enemies())
            .split_whitespace()
        {
            if let Some(&enemy) = name_to_index.get(token) {
                player.set_player_relationship_by_index(
                    enemy,
                    gamelogic::common::Relationship::Enemies,
                );
            }
        }
        for token in dict
            .get_ascii_string(key_player_allies())
            .split_whitespace()
        {
            if let Some(&ally) = name_to_index.get(token) {
                player.set_player_relationship_by_index(
                    ally,
                    gamelogic::common::Relationship::Allies,
                );
            }
        }
    }
}

pub(in super::super) fn load_multiplayer_scripts_scb() -> Option<ScriptList> {
    use game_engine::common::system::DataChunkInput;
    use game_engine::common::system::file::FileAccess;
    use game_engine::common::system::file_system::get_file_system;
    use gamelogic::scripting::core::{ScriptListReadInfo, parse_player_scripts_list_chunk};

    const VIRTUAL: &str = "Data\\Scripts\\MultiplayerScripts.scb";
    let data = {
        let fs = get_file_system();
        let mut guard = fs.lock().ok()?;
        let mut file = guard.open_file(VIRTUAL, FileAccess::READ.combine(FileAccess::BINARY))?;
        file.read_entire_and_close().ok()
    }
    .or_else(|| {
        const CANDIDATES: &[&str] = &[
            "windows_game/Command & Conquer Generals Zero Hour/Data/Scripts/MultiplayerScripts.scb",
            "Data/Scripts/MultiplayerScripts.scb",
        ];
        CANDIDATES.iter().find_map(|path| std::fs::read(path).ok())
    })?;
    let mut input = DataChunkInput::new(data);
    let mut read_info = ScriptListReadInfo::default();
    input.register_parser("PlayerScriptsList", "", parse_player_scripts_list_chunk);
    if !input.parse(&mut read_info) {
        return None;
    }
    read_info.lists.into_iter().next().map(|boxed| *boxed)
}

pub(in super::super) fn clear_live_campaign_victorious_for_new_game(world: &GameLogic) {
    if let Some(engine) = &world.owned_script_engine {
        engine.set_campaign_victorious(false);
    }
}

#[cfg(feature = "game_client")]
pub(in super::super) fn apply_cpp_heightmap_xy_and_border(
    heightmap: &mut game_client::terrain::height_map::HeightMap,
    map_border: i32,
) {
    use gamelogic::common::MAP_XY_FACTOR;
    // WorldHeightMap.cpp866-873 preserves the authored border; pre-v3 is zero.
    heightmap.scale = MAP_XY_FACTOR;
    heightmap.border_size = map_border;
}

#[cfg(test)]
#[path = "map_interpreter_lifetime_tests.rs"]
mod map_interpreter_lifetime_tests;
