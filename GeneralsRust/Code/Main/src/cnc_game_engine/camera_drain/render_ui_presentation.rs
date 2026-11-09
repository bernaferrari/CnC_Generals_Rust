use super::*;

impl CnCGameEngine {
    /// Wave 590: seed PresentationFrame after match start (no logic advance).
    /// Syncs shadow, builds host+GW frame, applies HUD/ControlBar/UI state.
    #[inline]

    /// Wave 591: render-path UI presentation consumer residual.
    ///
    /// Prefers pipeline freeze, then `last_presentation_frame`. Builds
    /// `GameUIState` from presentation only (no live object walks). Boot/loading
    /// residual without a freeze still uses `host_update_ui_state`.

    /// Wave 592: render-path presentation overlays residual (script/clock/diag).
    ///
    /// Applies presentation-or-boot script messages, sim clock + fps/
    /// diagnostics/asset stats. Does not process UI events (caller owns that
    /// boundary); frozen radar events are delivered through the sole HUD apply.

    /// Wave 593: render-path UI finalize residual (minimap/radar/victory/last_ui).
    ///
    /// After overlays + `process_ui_events`, stamps minimap texture/coords/viewport,
    /// radar pings/messages from presentation world bounds, match-over/victory
    /// summary, and retains `last_ui_state`.
    pub(in crate::cnc_game_engine) fn host_finalize_render_ui_state(
        &mut self,
        ui_state: &mut crate::ui::GameUIState,
    ) {
        // Wave 593: render UI finalize residual.
        ui_state.minimap_texture_id = self.render_pipeline.get_minimap_texture_id();
        ui_state.minimap_coordinates = self.render_pipeline.get_minimap_coordinates().cloned();
        self.update_minimap_viewport(ui_state);
        // Prefer presentation world_env for radar/minimap when a frame is installed.
        let world_bounds = self.presentation_world_bounds();
        self.game_hud_mut().update_radar_pings(
            &ui_state.radar_pings,
            world_bounds.0,
            world_bounds.1,
        );
        // Frozen radar events are delivered by apply_presentation_to_huds.
        // The UI state's message/event lists describe those same events;
        // forwarding them here would enqueue each notification again.

        ui_state.match_over = self.match_over;
        if let Some(summary) = &self.victory_summary {
            ui_state.victory_summary = Some(summary.clone());
            ui_state.player_outcome = summary
                .player_results
                .iter()
                .find(|result| result.player_id == self.current_player_id)
                .map(|result| result.outcome);
        } else {
            ui_state.victory_summary = None;
            ui_state.player_outcome = None;
        }
        // Retain presentation-overlaid identity for consumers (was dropped each frame).
        self.last_ui_state = Some(ui_state.clone());
    }

    pub(in crate::cnc_game_engine) fn host_apply_render_ui_presentation_overlays(
        &mut self,
        ui_state: &mut crate::ui::GameUIState,
    ) {
        // Wave 570: presentation-or-boot script message residual via helper.
        let new_script_messages: Vec<String> = self.take_presentation_or_boot_new_script_messages();
        for msg in &new_script_messages {
            self.game_hud_mut().push_script_message(msg);
        }
        // Wave 462: prefer pipeline/last presentation sim clock residual.
        // Wave 553: via presentation_or_boot_total_play_time helper.
        ui_state.current_game_time = self.presentation_or_boot_total_play_time();
        ui_state.fps = self.fps;
        ui_state.frame_time_ms = if self.fps > 0.0 {
            1000.0 / self.fps
        } else {
            0.0
        };
        ui_state.performance_score = (ui_state.fps / 60.0).clamp(0.0, 1.5);
        if let Some(diag) = &self.diagnostics_overlay {
            ui_state.diagnostics = Some(diag.clone());
        } else {
            ui_state.diagnostics = Some(DiagnosticsOverlayStats::from_overall(
                ui_state.performance_score * 100.0,
            ));
        }
        ui_state.show_debug_overlay = self.show_debug_info;
        if let Ok(manager) = self.application_assets.lock() {
            let stats = manager.get_statistics();
            ui_state.assets_loaded = stats.archive_stats.total_files as u64;
            ui_state.asset_memory_mb = 0.0;
            ui_state.asset_cache_usage = 0.0;
        }
    }

    pub(in crate::cnc_game_engine) fn host_build_render_ui_state_from_presentation(
        &mut self,
    ) -> crate::ui::GameUIState {
        // Wave 591: render UI presentation consumer residual.
        // Wave 462: prefer pipeline freeze, then last_presentation_frame.
        // Boot residual: update_ui_state only when no frame is installed yet.
        // ControlBar selection panel health is presentation-owned.
        // GameUIState is built from PresentationFrame only (no live object walks).
        if let Some(pres) = self
            .render_pipeline
            .presentation_frame_handle()
            .or_else(|| self.last_presentation_frame.clone())
        {
            let mut ui = crate::ui::GameUIState::default();
            pres.apply_to_ui_state(&mut ui);
            self.apply_presentation_to_huds(&pres);
            // Presentation audio already dispatched; SFX dual-path retired.
            self.sync_eva_messages_from_presentation(&pres);
            #[cfg(feature = "game_client")]
            {
                self.update_owned_control_bar_animations();
                self.control_bar.set_receiving_upgrade_definitions(
                    &self.game_logic.upgrade_definitions,
                    self.game_logic
                        .receiving_module_content
                        .as_ref()
                        .map(|content| content.action_definitions_shared()),
                );
                self.control_bar
                    .bind_logic_owner(
                Some(self.game_logic.command_definitions()),
                self.game_logic.control_bar_bridge(),
            );
                {
                    let facts = self.receiving_menu_world_facts();
                    let bar = &mut self.control_bar;
                    self.game_client
                        .with_window_event_effects(Some(facts), |effects| {
                            effects.with_manager(|manager, effects| {
                                pres.apply_to_control_bar(bar, manager, effects)
                            })
                        })
                };
                let _ = self
                    .update_receiving_observer_control_bar(std::time::Duration::from_millis(33));
            }
            // Keep last_presentation aligned with pipeline freeze when it was the source.
            self.last_presentation_frame = Some(pres.into());
            ui
        } else {
            // Boot/loading residual only.
            self.host_update_ui_state(self.current_player_id)
        }
    }

    pub(in crate::cnc_game_engine) fn host_sync_shadow_and_build_presentation(
        &mut self,
        with_victory: bool,
    ) -> Result<crate::presentation_frame::PresentationFrame, String> {
        // Availability is checked before mutating the shadow observation mirror.
        self.game_logic.owned_script_engine.as_ref().ok_or(
            "Cannot freeze native camera fade: receiving interpreter is unavailable or lent",
        )?;
        self.game_logic.advance_world_animations_for_presentation();
        // Wave 926: single host→shadow sync + presentation build boundary.
        if let Some(ref mut shadow) = self.gameworld_shadow {
            shadow.sync_from_host(&self.game_logic);
        }
        let runtime_heightmap = self.presentation_runtime_heightmap_for_frame();
        let local_id = self.current_player_id;
        let mut frame = crate::presentation_frame::PresentationFrame::try_publish_for_engine_with_runtime_heightmap(
            &mut self.game_logic,
            local_id,
            self.gameworld_shadow.as_ref(),
            runtime_heightmap,
            with_victory,
        )?;
        #[cfg(feature = "game_client")]
        {
            frame.dynamic_lights = self.game_client.dynamic_lights().to_vec();
        }
        #[cfg(feature = "game_client")]
        frame.scene_lines.extend(
            self.game_client
                .owned_tracer_geometry(frame.frame.0)
                .into_iter()
                .filter_map(crate::presentation_frame::PresentationSceneLine::from_cpp_desc),
        );
        #[cfg(feature = "game_client")]
        frame.scene_lines.extend(
            self.game_client
                .render_bridge()
                .snapshot_visible_scene_lines()
                .into_iter()
                .map(crate::presentation_frame::PresentationSceneLine::from),
        );
        #[cfg(feature = "game_client")]
        {
            // The asset cache is a native service shared with loader workers;
            // only immutable authored texture dimensions cross this boundary.
            let texture_store = self.application_assets.as_ref();
            let texture_aspect = |name: &str| -> Result<f32, String> {
                let mut manager = texture_store
                    .lock()
                    .map_err(|_| "laser texture service poisoned".to_string())?;
                if manager.get_raw_texture(name).is_none() {
                    manager.prime_texture_raw_blocking(name);
                }
                let texture = manager
                    .get_raw_texture(name)
                    .filter(|texture| texture.width > 0 && texture.height > 0)
                    .ok_or_else(|| format!("authored laser texture not resident: {name}"))?;
                Ok(texture.width as f32 / texture.height as f32)
            };
            let logic = &self.game_logic;
            match crate::presentation_frame::freeze_owned_laser_scene_lines(
                &mut self.game_client,
                &texture_aspect,
                &mut |x, y| logic.terrain_height_at(glam::Vec3::new(x, 0.0, y)),
            ) {
                Ok(lines) => frame.scene_lines.extend(lines),
                Err(error) => log::warn!("receiving laser geometry unavailable: {error}"),
            }
        }
        Ok(frame)
    }

    pub(in crate::cnc_game_engine) fn host_seed_presentation_after_match_start(
        &mut self,
    ) -> Result<(), String> {
        self.game_logic.owned_script_engine.as_ref().ok_or(
            "Cannot freeze native camera fade: receiving interpreter is unavailable or lent",
        )?;
        // Wave 590: match-start presentation seed residual.
        self.match_damage_applied = 0.0;
        self.match_kills = 0;
        self.game_logic.health_events.reset_cumulative();

        // Wave 172/590: match-start seed always syncs GameWorldShadow from host
        // then build_for_engine (GW overlay/rebuild lives inside that freeze).
        if let Some(ref mut shadow) = self.gameworld_shadow {
            shadow.sync_from_host(&self.game_logic);
        }
        let runtime_heightmap = self.presentation_runtime_heightmap_for_frame();
        let mut pres =
            crate::presentation_frame::PresentationFrame::try_publish_for_engine_with_runtime_heightmap(
                &mut self.game_logic,
                self.current_player_id,
                self.gameworld_shadow.as_ref(),
                runtime_heightmap,
                false,
            )?;
        #[cfg(feature = "game_client")]
        pres.scene_lines.extend(
            self.game_client
                .render_bridge()
                .snapshot_visible_scene_lines()
                .into_iter()
                .map(crate::presentation_frame::PresentationSceneLine::from),
        );
        // Seed the same instance HUD used by input and ordinary frame updates.
        self.apply_presentation_to_huds(&pres);
        #[cfg(feature = "game_client")]
        {
            self.update_owned_control_bar_animations();
            self.control_bar.set_receiving_upgrade_definitions(
                &self.game_logic.upgrade_definitions,
                self.game_logic
                    .receiving_module_content
                    .as_ref()
                    .map(|content| content.action_definitions_shared()),
            );
            self.control_bar
                .bind_logic_owner(
                Some(self.game_logic.command_definitions()),
                self.game_logic.control_bar_bridge(),
            );
            {
                let facts = self.receiving_menu_world_facts();
                let bar = &mut self.control_bar;
                self.game_client
                    .with_window_event_effects(Some(facts), |effects| {
                        effects.with_manager(|manager, effects| {
                            pres.apply_to_control_bar(bar, manager, effects)
                        })
                    })
            };
            let _ =
                self.update_receiving_observer_control_bar(std::time::Duration::from_millis(33));
        }
        let mut ui = GameUIState::default();
        pres.apply_to_ui_state(&mut ui);
        self.last_ui_state = Some(ui);
        self.last_presentation_frame = Some(pres.into());
        // C++ does not evaluate victory on the load frame. A just-seeded
        // alpine/empty world would otherwise stamp match_over and jump to Defeat.
        if let Some(pres) = self
            .last_presentation_frame
            .as_mut()
            .map(std::rc::Rc::make_mut)
        {
            pres.match_over = false;
        }
        self.host_match_over = Some(false);
        self.host_match_boot_victory_condition = Some(None);
        self.match_over = false;
        #[cfg(feature = "game_client")]
        {
            // A map/load seed can transition directly to InGame before the
            // next logic tick reaches `host_tick_game_client_presentation_shell`.
            // Establish the exact direct bindings now so the first W3D
            // candidate ledger has a current key/status/pose rather than
            // silently skipping its C++ clear-frame boundary for one render.
            let presentation_time_frozen =
                self.presentation_or_boot_time_frozen() || self.game_paused;
            match self.host_sync_presentation_direct_drawables(presentation_time_frozen) {
                Ok(()) => {
                    if let Err(error) = self.host_prepare_presentation_materials() {
                        log::trace!("Initial Drawable material admission failed: {error}");
                    }
                }
                Err(error) => log::trace!("Initial Drawable association failed: {error}"),
            }
        }
        // Wave 844: stamp host sim residuals after match-start seed.
        self.host_refresh_match_sim_residuals_from_logic();
        Ok(())
    }

    pub(super) fn presentation_kind_names(
        cache: &std::cell::RefCell<HashMap<u128, std::sync::Arc<Vec<String>>>>,
        kinds: &[crate::game_logic::KindOf],
    ) -> std::sync::Arc<Vec<String>> {
        let mut key = 0u128;
        for kind in kinds {
            let bit = *kind as u32;
            if bit < 128 {
                key |= 1u128 << bit;
            }
        }
        if let Ok(guard) = cache.try_borrow() {
            if let Some(hit) = guard.get(&key) {
                return std::sync::Arc::clone(hit);
            }
        }
        let names = std::sync::Arc::new(kinds.iter().map(|kind| kind.name().to_string()).collect());
        if let Ok(mut guard) = cache.try_borrow_mut() {
            guard.insert(key, std::sync::Arc::clone(&names));
        }
        names
    }

    #[cfg(feature = "game_client")]
    pub(super) fn presentation_draw_module_names_from_template(
        cache: &std::cell::RefCell<HashMap<String, std::sync::Arc<Vec<String>>>>,
        template: &str,
        receiving: Option<&crate::game_logic::thing::ReceivingDrawModels>,
        application_assets: Option<&std::sync::Mutex<crate::assets::manager::AssetManager>>,
    ) -> std::sync::Arc<Vec<String>> {
        if let Some(receiving) = receiving {
            // An admitted empty Draw list is also authoritative. Neither an
            // application cache hit nor a same-name peer may replace it.
            return receiving.module_class_names();
        }
        if let Ok(guard) = cache.try_borrow() {
            if let Some(hit) = guard.get(template) {
                return std::sync::Arc::clone(hit);
            }
        }
        let Some(manager) = application_assets else {
            return std::sync::Arc::new(Vec::new());
        };
        let Ok(manager) = manager.lock() else {
            return std::sync::Arc::new(Vec::new());
        };
        let Some(definition) = manager.get_object_definition(template) else {
            return std::sync::Arc::new(Vec::new());
        };
        let names = std::sync::Arc::new(
            definition
                .draw_modules
                .iter()
                .filter_map(|module| {
                    module
                        .declaration
                        .split_whitespace()
                        .next()
                        .map(str::to_string)
                })
                .collect::<Vec<_>>(),
        );
        drop(manager);
        if let Ok(mut guard) = cache.try_borrow_mut() {
            guard.insert(template.to_string(), std::sync::Arc::clone(&names));
        }
        names
    }

    /// Freeze-to-GameClient direct Drawable association boundary shared by the
    /// ordinary presentation shell tick and the initial match/load seed.
    ///
    /// This is deliberately narrower than `host_tick_game_client_presentation_shell`:
    /// it does not consume input, update UI/effects, or advance client visual
    /// time. It only establishes the C++ Object-backed Drawable association,
    /// status, and pose from one immutable presentation frame, so the first
    /// InGame W3D candidate can validate and write its clear-frame state.
    #[cfg(feature = "game_client")]
    pub(in crate::cnc_game_engine) fn host_sync_presentation_direct_drawables(
        &mut self,
        presentation_time_frozen: bool,
    ) -> Result<(), String> {
        let Some(pres) = self.last_presentation_frame.as_ref() else {
            return Ok(());
        };
        crate::game_logic::refresh_host_fx_object_poses_from_presentation_on(
            &self.game_logic.world_services,
            pres,
        );

        let logic_frame = pres.frame.0;
        let host_epoch = self.host_direct_visual_world_epoch;
        let draw_module_cache = &self.draw_module_name_cache;
        // Materialize every borrowed presentation fact before mutating
        // GameClient, preserving the immutable host→client boundary.
        for direct in &pres.direct_host_drawables {
            if direct.resident
                && direct.authored_client_template_required
                && direct.receiving_client_template.is_none()
            {
                return Err(format!(
                    "Authored visual template '{}' has no admitted receiving module data",
                    direct.visual_template_name
                ));
            }
        }
        let sync_entries = pres
            .direct_host_drawables
            .iter()
            .map(|direct| {
                let o = &direct.object;
                game_client::core::game_client::PresentationDrawableSync {
                    receiving_science_player: pres.receiving_science_player.clone(),
                    object_id: o.id.0,
                    ambient_audio_locality: Some(self.game_logic.receiving_audio_locality_for_player(
                        o.owner_player_id.map(|owner| owner as i32).unwrap_or(-1),
                    )),
                    host_epoch,
                    // The direct-host roster was frozen from actual host
                    // ownership. It deliberately remains resident through
                    // gameplay death/slow death/rubble until that host entry
                    // is removed, matching C++ Drawable lifetime.
                    resident: direct.resident,
                    visual_template_name: direct.visual_template_name.clone(),
                    template_name: o.template_name.clone(),
                    position: [o.position.x, o.position.y, o.position.z],
                    orientation: o.orientation,
                    float_yaw: o.float_yaw,
                    float_pitch: o.float_pitch,
                    destroyed: o.destroyed,
                    model_condition_bits: o.model_condition_bits,
                    body_damage_state: o.body_damage_state,
                    // Wave 970: overlay residual (vet/construct) on Wave 965 kind/stealth/color/health.
                    kind_names: Self::presentation_kind_names(&self.kind_name_cache, &o.kind_of),
                    team_color: o.team_color,
                    effectively_stealthed: o.effectively_stealthed,
                    // C++ StealthUpdate resolves the look for this viewer:
                    // undetected enemy stealth is invisible, while allied
                    // stealth remains a translucent visible look. Dead
                    // drawables are source-visible even if old status bits
                    // still say stealthed.
                    scene_hidden_by_stealth: pres.local_viewer_hides_stealthed(o),
                    health_current: o.health_current,
                    health_max: o.health_max,
                    selected: o.selected,
                    veterancy_level: match o.veterancy {
                        crate::presentation_frame::PresentationVeterancy::Rookie => 0,
                        crate::presentation_frame::PresentationVeterancy::Veteran => 1,
                        crate::presentation_frame::PresentationVeterancy::Elite => 2,
                        crate::presentation_frame::PresentationVeterancy::Heroic => 3,
                    },
                    under_construction: o.under_construction,
                    construction_percent: o.construction_percent.clamp(0.0, 1.0),
                    // Wave 1115: sold residual for construct-percent fail-closed.
                    sold: o.sold,
                    // Wave 972: icon-pip residual.
                    ammo_pip_total: o.ammo_pip_total.min(255) as u8,
                    ammo_pip_full: o.ammo_pip_full.min(255) as u8,
                    occupant_count: (o.occupant_count as u32).min(255) as u8,
                    max_garrison: (o.max_garrison as u32).min(255) as u8,
                    disabled: o.disabled,
                    // C++ ICON_CARBOMB only when WEAPONSET_CARBOMB && local owner.
                    is_carbomb: o.weapon_set_carbomb
                        && o.owner_player_id == Some(pres.local_player_id),
                    bomb_type: o.bomb_type,
                    bomb_timer_seconds: o.bomb_timer_seconds,
                    weapon_bonus_enthusiastic: o.weapon_bonus_enthusiastic,
                    // Wave 983: healing icon residual.
                    show_healing: o.show_healing,
                    healing_icon_type: o.healing_icon_type,
                    // Wave 984: garrisoned unit ids for contained-flash residual.
                    garrisoned_ids: o.garrisoned_units.iter().map(|id| id.0).collect(),
                    // Wave 1057: emoticon residual for dual icon UI.
                    emoticon_name: o.emoticon_name.clone(),
                    emoticon_frames_left: o.emoticon_frames_left,
                    // Wave 1058: formation residual for dual formation letter.
                    formation_id: o.formation_id,
                    // C++ Drawable captions are explicit beacon text; authored
                    // Object names are not captions.
                    caption: crate::command_executor::host_beacon_caption(
                        &self.game_logic,
                        crate::game_logic::ObjectId(o.id.0),
                    )
                    .unwrap_or_default(),
                    module_admission: direct
                        .receiving_client_template
                        .as_ref()
                        .map(crate::game_logic::thing::ReceivingClientTemplate::client_admission)
                        .unwrap_or(game_client::core::game_client::PresentationModuleAdmission::Compatibility),
                    draw_logic_frame: logic_frame,
                    draw_indicator_color: direct.draw_indicator_color,
                    draw_physics: direct.draw_physics,
                    draw_velocity_magnitude_squared: direct.draw_velocity_magnitude_squared,
                    draw_module_names: Self::presentation_draw_module_names_from_template(
                        draw_module_cache,
                        if direct.visual_template_name.trim().is_empty() {
                            o.template_name.as_str()
                        } else {
                            direct.visual_template_name.as_str()
                        },
                        direct.receiving_visual_draw_models.as_deref(),
                        Some(self.application_assets.as_ref()),
                    ),
                    health_box_width: o.health_box_width,
                    health_box_z_offset: o.health_box_z_offset,
                }
            })
            .collect::<Vec<_>>();
        let direct_sources = pres
            .direct_host_drawables
            .iter()
            .filter(|direct| direct.resident)
            .filter_map(|direct| {
                let object = &direct.object;
                let (raw_status, effectively_dead) =
                    object.drawable_shroud.direct_game_client_status()?;
                Some((
                    object.id.0,
                    raw_status,
                    effectively_dead,
                    [object.position.x, object.position.y, object.position.z],
                    object.orientation,
                    object.float_yaw,
                    object.float_pitch,
                ))
            })
            .collect::<Vec<_>>();

        let native_source_inputs: HashMap<_, _> = pres
            .native_resident_drawable_inputs()
            .into_iter()
            .map(|unit| (unit.id.0, unit))
            .collect();
        let sync_ms = self.render_pipeline.current_animation_sync_ms();
        let draw_policy =
            super::native_view_effects::receiving_native_drawable_policy_for(&self.game_logic);
        let native_owner = &mut self.render_pipeline;
        native_owner.bind_native_prop_epoch(host_epoch);
        let graphics = &mut self.graphics_system;
        let (created, updated, pruned) = if let Some(content) =
            &mut self.game_logic.receiving_module_content
        {
            self.game_client
                .sync_presentation_drawables_observed_with_content(
                    sync_entries,
                    sync_ms,
                    content.module_factory_mut(),
                    draw_policy,
                    |mutation| {
                        let source = native_source_inputs.get(&mutation.object_id());
                        native_owner.receive_native_drawable_mutation(mutation, source, graphics);
                    },
                )
                .map_err(|error| error.to_string())?
        } else {
            self.game_client
                .sync_presentation_drawables_observed(sync_entries, sync_ms, |mutation| {
                    let source = native_source_inputs.get(&mutation.object_id());
                    native_owner.receive_native_drawable_mutation(mutation, source, graphics);
                })
                .map_err(|error| error.to_string())?
        };
        if created + updated + pruned > 0 {
            log::trace!(
                "presentation drawable sync created={created} updated={updated} pruned={pruned}"
            );
        }
        // Creating precedes allocation of the real Client association. Only
        // now bind that exact lifetime to the already constructed Draw owner.
        // A changed key without Destroy/Creating is rejected, never relabeled.
        for (object_id, source) in &native_source_inputs {
            if let (Some(identity), Some(binding)) = (
                source.draw_playback_identity.as_ref(),
                self.game_client
                    .presentation_direct_drawable_state(host_epoch, *object_id),
            ) {
                self.render_pipeline
                    .admit_native_drawable_binding(identity, binding.binding_key)
                    .map_err(str::to_owned)?;
            }
        }
        // Resolve the complete current binding *after* sync. The same runtime
        // key then guards raw C++ shroud status and frozen pose, so an ordinary
        // update preserves it while a visual replacement rejects predecessor
        // frame data.
        let direct_bindings = direct_sources
            .into_iter()
            .filter_map(
                |(
                    object_id,
                    raw_status,
                    effectively_dead,
                    position,
                    orientation,
                    float_yaw,
                    float_pitch,
                )| {
                    let binding_key = self
                        .game_client
                        .presentation_direct_drawable_state(host_epoch, object_id)?
                        .binding_key;
                    Some((
                        binding_key,
                        raw_status,
                        effectively_dead,
                        position,
                        orientation,
                        float_yaw,
                        float_pitch,
                    ))
                },
            )
            .collect::<Vec<_>>();
        // C++ `GameClient::update` does not recompute bound Drawable shroud
        // state while visual time is frozen. The view/scene phase still runs
        // later against this retained client state, where a real eligible
        // Clear candidate may refresh its clear frame.
        if !presentation_time_frozen {
            let shroud_entries = direct_bindings.iter().map(
                |(binding_key, raw_status, effectively_dead, _, _, _, _)| {
                    game_client::core::game_client::FrozenDirectShroudStatus {
                        binding_key: *binding_key,
                        raw_status: *raw_status,
                        effectively_dead: *effectively_dead,
                    }
                },
            );
            let _ = self
                .game_client
                .apply_frozen_direct_shroud_statuses(logic_frame, shroud_entries);
        }
        // Pose belongs to the identical direct binding rather than a raw
        // ObjectID. Do not filter gameplay-destroyed records here: the direct
        // source roster controls C++ Drawable residency.
        let pose_entries = direct_bindings.into_iter().map(
            |(binding_key, _, _, position, orientation, float_yaw, float_pitch)| {
                game_client::core::game_client::FrozenDirectPresentationPose {
                    binding_key,
                    position,
                    orientation,
                    float_yaw,
                    float_pitch,
                }
            },
        );
        let n = self
            .game_client
            .apply_frozen_direct_presentation_poses(pose_entries);
        if n > 0 {
            log::trace!("presentation pose applied to {n} drawables");
        }
        // Freeze specialized state after sync and pose application. Sharing
        // this small completed bundle never mutates/copies the shared frame.
        if let Some(frame) = self.last_presentation_frame.as_ref() {
            self.render_pipeline.capture_specialized_draw_inputs(
                frame,
                &self.game_client,
                self.host_direct_visual_world_epoch,
            );
        }
        Ok(())
    }
}
