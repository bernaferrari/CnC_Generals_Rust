// GameClient update loop, input/audio/drawable ticks, and post-draw UI.
// Split from `core/game_client.rs` dump. Included by `game_client/mod.rs`
// so this stays one logical `game_client` module (public API identical).

impl GameClient {
    /// Native Main supplies script output through its receiving driver. Only
    /// the standalone Core path consumes the original Core script latch.
    fn apply_core_script_display_state(&mut self) {
        if self.input_authority == ClientInputAuthority::StandaloneCore {
            if let Some(ui) = self.subsystem_manager.in_game_ui.as_mut() {
                apply_pending_script_display_state(ui, self.subsystem_manager.input_mouse.as_mut());
            }
        }
    }

    /// Updates the game client - main game loop entry point.
    ///
    /// C++ parity: frame sequence matches `GameClient::update()` (GameClient.cpp:489-752).
    pub fn update(
        &mut self,
        logic_frame: u32,
        game_mode: i32,
        view: &mut crate::display::view::View,
    ) -> GameClientResult<()> {
        self.update_with_window_owner(logic_frame, game_mode, view)
    }

    fn update_with_window_owner(
        &mut self,
        logic_frame: u32,
        game_mode: i32,
        view: &mut crate::display::view::View,
    ) -> GameClientResult<()> {
        if !self.initialized {
            return Err(GameClientError::InvalidOperation(
                "GameClient not initialized".to_string(),
            ));
        }

        let current_time = Instant::now();
        let elapsed = current_time
            .saturating_duration_since(self.last_update_time)
            .as_secs_f32();
        self.last_update_time = current_time;

        // GameLogic owns admission of the client frame (GameLogic.cpp:3593-3596).
        // Presenting or updating UI never advances simulation time.
        publish_live_game_client_frame(self);
        // Classic Core entry only. Native Main supplies both owning clocks to
        // update_presentation_shell_with_particle_frames without this lookup.
        let frames = NativeParticleFrames {
            logic_frame,
            client_frame: self.frame,
        };

        self.create_frame_tick_message()?;
        self.update_startup_movies()?;
        if self.startup_movies_active() {
            self.update_startup_movie_display()?;
            self.rendered_object_count = 0;
            self.finish_frame_timing(current_time);
            return Ok(());
        }
        self.ensure_shell_visible(None)?;
        let freeze_time = self.should_freeze_visual_time();
        let mut visual_delta = if freeze_time {
            0.0
        } else {
            elapsed.min(SECONDS_PER_LOGICFRAME_REAL)
        };
        let visual_speed = get_script_visual_speed_multiplier();
        visual_delta = if visual_speed <= 0 {
            0.0
        } else {
            visual_delta * visual_speed as f32
        };
        // C++ GameClient.cpp:560-565 — snow + Anim2D before input.
        self.update_cpp_snow_and_anim2d(visual_delta);
        // C++ GameClient.cpp:587-597 — camera follows first selected drawable.
        self.update_camera_tracking_drawable();

        // C++ lines 612-619: window manager and video player update BEFORE drawables
        self.update_pre_draw_ui()?;

        // Host/presentation residual: Main owns OS WindowEvent→commands and sole
        // RenderPipeline 3D present. When OBJECT_REGISTRY is empty, skip dual-world
        // shroud bind and dual Display DRAW present.
        let host_presentation_path = OBJECT_REGISTRY.is_empty();
        self.update_orphaned_w3d_ghosts_if_unfrozen(freeze_time);
        if host_presentation_path {
            // Shared THE_MOUSE/THE_KEYBOARD may still be polled for shell widgets;
            // Main remains command authority. Prefer local drawable modules only.
            self.update_drawables_local(visual_delta, frames)?;
            // Wave 1022: catalog shroud residual on presentation shell tick path.
            self.update_drawable_visibility(self.local_player_id)?;
            if self.should_skip_visual_updates_for_no_draw() {
                self.rendered_object_count = 0;
                self.finish_frame_timing(current_time);
                return Ok(());
            }
            self.update_effects(visual_delta)?;
            self.apply_core_script_display_state();
            self.update_display_only()?;
            self.update_native_draw_particle_phase(freeze_time, frames, true)?;
            // No draw_display — Main RenderPipeline is sole present path.
            self.draw_drawable_icon_ui();
            self.draw_presentation_selection_residual();
            // C++ TheInGameUI->update message/superweapon drains before postDraw.
            self.drain_logic_hud_messages(frames.logic_frame);
            self.apply_queued_weapon_recoils();
            for drawable in self.drawables.drawable_map.values_mut() {
                if let Some(basic) = drawable
                    .as_any_mut()
                    .downcast_mut::<crate::drawable::drawable::BasicDrawable>()
                {
                    basic.integrate_recoil_visual();
                }
            }
            self.sync_superweapon_strip_from_logic();
            let _ = self.draw_live_ingame_hud(frames.logic_frame);

            self.update_display_string_manager()?;
            self.update_post_draw_ui(None)?;
            self.process_beacon_notifications(frames.logic_frame)?;
            self.pump_message_stream(frames.logic_frame, game_mode, Some(view))?;
            self.rendered_object_count = 0;
            self.finish_frame_timing(current_time);
            return Ok(());
        }

        // Dual-world residual: full C++-ordered client tick with registry bind.
        // C++ lines 560-584: keyboard, mouse, Anim2D, Eva
        self.update_input()?;
        self.update_audio()?;

        // C++ lines 660-700: shroud check per-drawable then updateDrawable()
        self.update_drawables(visual_delta, frames)?;
        if self.should_skip_visual_updates_for_no_draw() {
            self.rendered_object_count = 0;
            self.finish_frame_timing(current_time);
            return Ok(());
        }

        // C++ line 721: terrain visual, C++ line 726: display UPDATE
        self.update_effects(visual_delta)?;
        self.apply_core_script_display_state();
        self.update_display_only()?;
        self.update_native_draw_particle_phase(freeze_time, frames, false)?;

        // W3DDisplay.cpp:1730-1835 — freeze/sync/updateViews/particles before DRAW GPU.
        crate::display::client_draw_schedule::run_dual_world_cpu_phases();

        // C++ line 735: TheDisplay->DRAW()
        self.draw_display()?;

        self.draw_drawable_icon_ui();
        self.draw_presentation_selection_residual();
        // C++ TheInGameUI->update message/superweapon drains before postDraw.
        self.drain_logic_hud_messages(frames.logic_frame);
        self.sync_superweapon_strip_from_logic();
        let _ = self.draw_live_ingame_hud(frames.logic_frame);

        // C++ line 740: DisplayStringManager update
        self.update_display_string_manager()?;

        // C++ lines 744-751: Shell and InGameUI AFTER draw
        self.update_post_draw_ui(None)?;

        self.process_beacon_notifications(frames.logic_frame)?;
        self.pump_message_stream(frames.logic_frame, game_mode, Some(view))?;

        self.rendered_object_count = 0;

        self.finish_frame_timing(current_time);
        Ok(())
    }

    fn finish_frame_timing(&self, frame_start: Instant) {
        let script_fps_limit = get_script_fps_limit();
        let target_frame_duration = if script_fps_limit > 0 {
            Duration::from_secs_f64(1.0 / script_fps_limit as f64)
        } else {
            self.target_frame_duration
        };
        let frame_elapsed = frame_start.elapsed();
        if frame_elapsed < target_frame_duration {
            thread::sleep(target_frame_duration - frame_elapsed);
        }
    }

    /// Deliver host input to the same instance-owned device updated below.
    pub fn inject_keyboard_key(&mut self, key: crate::input::KeyCode, pressed: bool) -> bool {
        self.subsystem_manager
            .input_keyboard
            .as_mut()
            .is_some_and(|keyboard| keyboard.handle_key_simple(key, pressed))
    }

    /// WinMain WM_SETFOCUS / WM_KILLFOCUS reset keys, not the input clock.
    pub fn reset_keyboard_keys(&mut self) {
        if let Some(keyboard) = self.subsystem_manager.input_keyboard.as_mut() {
            keyboard.reset_pressed_keys();
        }
    }

    pub fn update_input(&mut self) -> GameClientResult<()> {
        if let Some(ref mut keyboard) = self.subsystem_manager.input_keyboard {
            keyboard.update();
        }

        if let Some(ref mut mouse) = self.subsystem_manager.input_mouse {
            mouse.update();
        }

        Ok(())
    }

    /// C++ `TheSnowManager->UPDATE()` + `TheAnim2DCollection->UPDATE()`.
    pub fn update_cpp_snow_and_anim2d(&mut self, delta_seconds: f32) {
        self.snow.update(delta_seconds);
        crate::system::update_client_anim2d_collection();
    }

    /// C++ `TheTerrainVisual->UPDATE()` (GameClient.cpp:719-722). Fail-soft.
    pub fn update_terrain_visual(&self) {
        if let Some(tv) = &self.subsystem_manager.terrain_visual {
            let _ = tv.lock().unwrap_or_else(|e| e.into_inner()).update();
        }
    }

    /// C++ GameClient.cpp:587-597 — follow first selected drawable or clear flag.
    fn update_camera_tracking_drawable(&mut self) {
        if !self
            .subsystem_manager
            .in_game_ui
            .as_ref()
            .is_some_and(|ui| ui.is_camera_tracking_drawable())
        {
            return;
        }
        use crate::drawable::DrawableExt;
        // C++ `TheInGameUI->getFirstSelectedDrawable()` is the first entry of
        // the selection list. HashMap iteration is unordered, so pick the
        // lowest DrawableId among selected units for a stable follow target.
        let mut first: Option<(crate::drawable::DrawableId, crate::drawable::Vector3)> = None;
        for (id, drawable) in &self.drawables.drawable_map {
            let Some(basic) = drawable.downcast_ref::<crate::drawable::drawable::BasicDrawable>()
            else {
                continue;
            };
            if !basic.is_selected() {
                continue;
            }
            let pos = basic.get_position();
            match first {
                None => first = Some((*id, pos)),
                Some((prev, _)) if id.0 < prev.0 => first = Some((*id, pos)),
                _ => {}
            }
        }
        if let Some((_, pos)) = first {
            crate::display::view::with_tactical_view(|view| {
                view.look_at(&crate::display::view::Point3::new(pos.x, pos.y, pos.z));
            });
        } else {
            if let Some(ui) = self.subsystem_manager.in_game_ui.as_mut() {
                ui.set_camera_tracking_drawable(false);
            }
        }
    }

    fn update_audio(&mut self) -> GameClientResult<()> {
        if let Some(ref audio) = self.subsystem_manager.audio {
            audio.lock().unwrap_or_else(|e| e.into_inner()).update()?;
        }

        if let (Some(queue), Some(engine)) = (&mut self.audio_event_queue, &mut self.audio_engine) {
            for request in queue.drain() {
                match request {
                    crate::audio::AudioRequest::Play { event, .. } => {
                        let _ = engine.play_event(&event.event_name, event.position);
                    }
                    crate::audio::AudioRequest::Pause { handle } => {
                        // AudioEngine doesn't have pause yet; stop for now.
                        engine.stop_event(handle);
                    }
                    crate::audio::AudioRequest::Stop { handle } => {
                        engine.stop_event(handle);
                    }
                }
            }
        }

        if let (Some(music), Some(engine)) = (&mut self.music_system, &mut self.audio_engine) {
            music.update(engine);
        }

        if let (Some(speech), Some(engine)) = (&mut self.speech_system, &mut self.audio_engine) {
            speech.update(engine);
        }

        Ok(())
    }

    /// Tick drawable client modules without GameLogic OBJECT_REGISTRY binding.
    /// Used when Main presentation snapshot owns unit visuals (default host path).
    pub fn update_drawables_local(
        &mut self,
        delta_time: f32,
        frames: NativeParticleFrames,
    ) -> GameClientResult<()> {
        if frames.client_frame != self.frame {
            return Err(GameClientError::InvalidOperation(
                "Drawable Client frame does not match receiving owner".into(),
            ));
        }
        // C++ Drawable.cpp:1219 — `TheGameClient->getFrame() % DRAWABLE_FRAMES_PER_FLASH`.
        // BasicDrawable::update consumes `flash_count` from `self.current_frame`;
        // leaving it at 0 makes `0 % 15 == 0` fire every tick.
        self.tick_mapped_drawables(frames, delta_time)?;
        Ok(())
    }

    /// Native hosted shell tick with receiving TOD/locality supplied by Main.
    /// The already-bound client audio device remains the only playback owner.
    pub fn update_drawables_local_with_ambient(
        &mut self,
        delta_time: f32,
        frames: NativeParticleFrames,
        ambient: &mut crate::drawable::drawable::NativeAmbientUpdate<'_>,
    ) -> GameClientResult<()> {
        if frames.client_frame != self.frame {
            return Err(GameClientError::InvalidOperation(
                "Drawable Client frame does not match receiving owner".into(),
            ));
        }
        self.tick_mapped_drawables_with_ambient(frames, delta_time, Some(ambient))
    }

    /// C++ `GameClient` drawable walk: stamp the client frame, `updateDrawable`,
    /// then `destroyDrawable` when the expiration date is reached this frame.
    fn tick_mapped_drawables(
        &mut self,
        frames: NativeParticleFrames,
        delta_time: f32,
    ) -> GameClientResult<()> {
        self.tick_mapped_drawables_with_ambient(frames, delta_time, None)
    }

    fn tick_mapped_drawables_with_ambient(
        &mut self,
        frames: NativeParticleFrames,
        delta_time: f32,
        ambient: Option<&mut crate::drawable::drawable::NativeAmbientUpdate<'_>>,
    ) -> GameClientResult<()> {
        // This is the existing native manager boundary, borrowed once. Installed
        // CU/bone callbacks receive the receiver and never relock this parent.
        {
            let assets = self.render_bridge.asset_manager();
            self.drawables
                .particles
                .admit_effective_definitions()
                .map_err(|error| GameClientError::SubsystemError(error.to_string()))?;
            let (state, particles) = (&mut self.drawables.state, &mut self.drawables.particles);
            let ui = self.subsystem_manager.in_game_ui.as_mut();
            if let Some(ambient) = ambient {
                NativeRenderReceiver::new_with_ambient(
                    state,
                    &mut self.native_particle_capability_gaps,
                    self.native_audio_device.as_ref(),
                    ui,
                    self.frame,
                    self.local_player_id,
                    self.input_authority,
                    ambient,
                )
                .tick_mapped_drawables_with_particles(
                    frames,
                    delta_time,
                    Some(particles),
                    Some(assets),
                )?
            } else {
                NativeRenderReceiver::new(
                    state,
                    &mut self.native_particle_capability_gaps,
                    self.native_audio_device.as_ref(),
                    ui,
                    self.frame,
                    self.local_player_id,
                    self.input_authority,
                )
                .tick_mapped_drawables_with_particles(
                    frames,
                    delta_time,
                    Some(particles),
                    Some(assets),
                )?
            }
        };
        Ok(())
    }

    fn tick_mapped_drawables_with_particles(
        &mut self,
        frames: NativeParticleFrames,
        delta_time: f32,
        mut particles: Option<&mut crate::effects::particle_manager::ParticleSystemManager>,
        assets: Option<&ww3d_assets::AssetManager>,
    ) -> GameClientResult<Vec<DrawableId>> {
        self.native_render_receiver()
            .tick_mapped_drawables_with_particles(frames, delta_time, particles, assets)
    }

    /// Apply presentation-owned FOW shroud to bound drawables (no OBJECT_REGISTRY).
    ///
    /// C++ GameClient::update shroud residual (Fogged|Shrouded → fully obscured)
    /// driven by frozen `PresentationFrame` unit FOW instead of live object locks.
    pub fn apply_presentation_shroud_to_drawables<I>(&mut self, entries: I)
    where
        I: IntoIterator<Item = (u32, bool)>,
    {
        for (object_id, fully_obscured) in entries {
            let Some(drawable_id) = self.drawables.drawable_object_map.get(&object_id).copied()
            else {
                continue;
            };
            let Some(drawable) = self.drawables.drawable_map.get_mut(&drawable_id) else {
                continue;
            };
            drawable.set_fully_obscured_by_shroud(fully_obscured);
        }
    }

    /// Apply C++ `GameClient::update` direct-object shroud visibility from a
    /// frozen presentation frame.
    ///
    /// Each entry carries the full runtime binding key returned after the
    /// presentation sync.  It is accepted only if the host epoch, object id,
    /// Drawable id, binding generation, object map, and Drawable inverse map
    /// still agree.  This does not alter ordinary drawable visibility, hidden
    /// state, scene candidates, or clear-frame timestamps.  The latter remain
    /// owned by the W3D direct-scene dispatch that will be wired separately.
    ///
    /// Returns the number of current drawable associations whose direct shroud
    /// state accepted the update.
    pub fn apply_frozen_direct_shroud_statuses<I>(&mut self, logic_frame: u32, entries: I) -> usize
    where
        I: IntoIterator<Item = FrozenDirectShroudStatus>,
    {
        let mut applied = 0;

        for entry in entries {
            let Some(state) = self.presentation_direct_drawable_state(
                entry.binding_key.host_epoch,
                entry.binding_key.object_id,
            ) else {
                continue;
            };
            if state.binding_key != entry.binding_key {
                continue;
            }
            let drawable_id = state.binding_key.drawable_id;
            let Some(drawable) = self.drawables.drawable_map.get_mut(&drawable_id) else {
                continue;
            };
            if drawable.get_object_id() != Some(entry.binding_key.object_id) {
                continue;
            }
            if drawable
                .apply_frozen_direct_shroud_status(
                    logic_frame,
                    entry.raw_status,
                    entry.effectively_dead,
                )
                .is_some()
            {
                applied += 1;
            }
        }

        applied
    }

    /// Evaluate one frozen direct candidate at the C++ W3D scene boundary.
    ///
    /// Main invokes this only after its immutable presentation candidate has
    /// passed frustum culling and produced at least one real render item. The
    /// full runtime binding key prevents a stale frame from refreshing a
    /// replacement Drawable's volatile clear history. The concrete Drawable
    /// still owns its source-equivalent effective-hidden check.
    #[must_use]
    pub fn evaluate_frozen_direct_scene_candidate(
        &mut self,
        logic_frame: u32,
        binding_key: PresentationDirectDrawableBindingKey,
        raw_status: gamelogic::common::types::ObjectShroudStatus,
        effectively_dead: bool,
    ) -> Option<crate::drawable::SceneShroudDecision> {
        self.drawables.evaluate_frozen_direct_scene_candidate(
            logic_frame,
            binding_key,
            raw_status,
            effectively_dead,
        )
    }

    /// Consume one immutable Main scene-candidate ledger.
    ///
    /// The caller has already deduplicated candidates by full binding key and
    /// selected a single frozen view/pass. This client boundary validates the
    /// association again so a stale ledger can never refresh a replacement
    /// Drawable's clear-frame state.
    #[must_use]
    pub fn evaluate_frozen_direct_scene_shroud_candidates<I>(
        &mut self,
        logic_frame: u32,
        candidates: I,
    ) -> Vec<FrozenDirectSceneShroudDecision>
    where
        I: IntoIterator<Item = FrozenDirectSceneShroudCandidate>,
    {
        self.drawables
            .evaluate_frozen_direct_scene_shroud_candidates(logic_frame, candidates)
    }

    /// Read a direct presentation Drawable state guarded by frozen host-world
    /// and object identities.
    ///
    /// An unbound/objectless drawable, an unknown object, a host epoch from a
    /// previous world, a stale association, or a drawable without base direct
    /// shroud state returns `None` rather than being treated as clear.
    #[must_use]
    pub fn presentation_direct_drawable_state(
        &self,
        host_epoch: u64,
        object_id: ObjectID,
    ) -> Option<PresentationDirectDrawableState> {
        self.drawables
            .presentation_direct_drawable_state(host_epoch, object_id)
    }

    /// Compatibility reader for callers that do not yet retain a host epoch.
    ///
    /// New presentation code should use
    /// [`Self::presentation_direct_drawable_state`] and retain its full key.
    #[must_use]
    pub fn presentation_direct_fully_obscured(&self, object_id: ObjectID) -> Option<bool> {
        self.drawables.presentation_direct_fully_obscured(object_id)
    }

    /// Apply a frozen direct visual pose only when its full runtime binding
    /// key still resolves to the same Drawable.
    ///
    /// This is the keyed counterpart to the legacy
    /// [`Self::apply_presentation_pose_to_drawables`] helper.  Main's direct
    /// host path must use this method so a visual-template replacement cannot
    /// receive a pose captured for its predecessor.
    pub fn apply_frozen_direct_presentation_poses<I>(&mut self, entries: I) -> usize
    where
        I: IntoIterator<Item = FrozenDirectPresentationPose>,
    {
        let mut updated = 0usize;
        for entry in entries {
            let Some(state) = self.presentation_direct_drawable_state(
                entry.binding_key.host_epoch,
                entry.binding_key.object_id,
            ) else {
                continue;
            };
            if state.binding_key != entry.binding_key {
                continue;
            }
            let Some(drawable) = self
                .drawables
                .drawable_map
                .get_mut(&state.binding_key.drawable_id)
            else {
                continue;
            };
            if drawable.get_object_id() != Some(entry.binding_key.object_id) {
                continue;
            }
            let position = Vector3::new(entry.position[0], entry.position[1], entry.position[2]);
            drawable.set_position(position);
            let transform = Matrix4::rotation_y(entry.orientation);
            drawable.set_base_transform(transform);
            updated = updated.saturating_add(1);
        }
        updated
    }

    /// Apply presentation-owned world pose to bound drawables (no OBJECT_REGISTRY).
    ///
    /// C++ drawable instance transform residual driven by frozen `PresentationFrame`
    /// positions/orientation instead of live GameLogic object locks.
    pub fn apply_presentation_pose_to_drawables<I>(&mut self, entries: I) -> usize
    where
        I: IntoIterator<Item = (u32, [f32; 3], f32)>,
    {
        let mut updated = 0usize;
        for (object_id, pos, orientation) in entries {
            let Some(drawable_id) = self.drawables.drawable_object_map.get(&object_id).copied()
            else {
                continue;
            };
            let Some(drawable) = self.drawables.drawable_map.get_mut(&drawable_id) else {
                continue;
            };
            let position = Vector3::new(pos[0], pos[1], pos[2]);
            drawable.set_position(position);
            // C++ `Drawable::draw` starts with the owning Thing/Object world
            // transform, then post-multiplies only the local instance matrix.
            // `BasicDrawable::get_transform()` already supplies the world
            // translation from `set_position`, so carrying it in the instance
            // matrix as well would translate every host-synced drawable twice.
            let transform = Matrix4::rotation_y(orientation);
            drawable.set_base_transform(transform);
            updated += 1;
        }
        updated
    }

    /// Wave 962/963: host presentation drawable ensure (no OBJECT_REGISTRY dual-world).
    ///
    /// Creates missing drawables bound to presentation object ids so pose/shroud
    /// residuals apply without populating the dual-world registry.
    pub fn ensure_presentation_drawables<I>(&mut self, entries: I) -> GameClientResult<usize>
    where
        I: IntoIterator<Item = (u32, String, [f32; 3], f32)>,
    {
        let sync = entries
            .into_iter()
            .map(|(id, tmpl, pos, ori)| PresentationDrawableSync {
                receiving_science_player: None,
                module_admission:
                    crate::core::game_client::PresentationModuleAdmission::Compatibility,
                // The explicit tuple Compatibility adapter has no simulation
                // observations. Authored native admission supplies these facts.
                draw_logic_frame: 0,
                draw_indicator_color: [0; 3],
                draw_physics: Default::default(),
                draw_velocity_magnitude_squared: 0.,
                object_id: id,
                ambient_audio_locality: None,
                host_epoch: 0,
                resident: true,
                visual_template_name: tmpl.clone(),
                template_name: tmpl,
                position: pos,
                orientation: ori,
                float_yaw: 0.0,
                float_pitch: 0.0,
                destroyed: false,
                model_condition_bits: 0,
                body_damage_state: 0,
                kind_names: std::sync::Arc::new(Vec::new()),
                team_color: [1.0, 1.0, 1.0, 1.0],
                effectively_stealthed: false,
                scene_hidden_by_stealth: false,
                health_current: 0.0,
                health_max: 0.0,
                selected: false,
                veterancy_level: 0,
                under_construction: false,
                construction_percent: 0.0,
                sold: false,
                ammo_pip_total: 0,
                ammo_pip_full: 0,
                occupant_count: 0,
                max_garrison: 0,
                disabled: false,
                is_carbomb: false,
                bomb_type: 0,
                bomb_timer_seconds: 0,
                weapon_bonus_enthusiastic: false,
                show_healing: false,
                healing_icon_type: 0,
                garrisoned_ids: Vec::new(),
                emoticon_name: String::new(),
                emoticon_frames_left: 0,
                formation_id: 0,
                caption: String::new(),
                draw_module_names: std::sync::Arc::new(Vec::new()),
                health_box_width: 0.0,
                health_box_z_offset: 0.0,
            });
        Ok(self.sync_presentation_drawables(sync)?.0)
    }

    /// Wave 963: presentation drawable sync residual (ensure + pose/model + prune).
    ///
    /// Direct visual residency is controlled solely by
    /// [`PresentationDrawableSync::resident`].  In particular, `destroyed`
    /// is diagnostic/render data and must not remove an active slow-death or
    /// rubble visual.  Returns `(created, updated, pruned)`. No
    /// OBJECT_REGISTRY dual-world populate.
    pub fn sync_presentation_drawables<I>(
        &mut self,
        entries: I,
    ) -> GameClientResult<(usize, usize, usize)>
    where
        I: IntoIterator<Item = PresentationDrawableSync>,
    {
        self.sync_presentation_drawables_observed(entries, 0, |_| {})
    }

    /// Same actual admission, with a synchronous receiving native-render owner.
    pub fn sync_presentation_drawables_observed<I>(
        &mut self,
        entries: I,
        sync_ms: u32,
        mut observe: impl FnMut(PresentationDrawableMutation<'_>),
    ) -> GameClientResult<(usize, usize, usize)>
    where
        I: IntoIterator<Item = PresentationDrawableSync>,
    {
        self.sync_presentation_drawables_with_content_inner(entries, sync_ms, None, None, observe)
    }

    /// Exact authored admission. The same mutable content owner crosses loading-worker handoff.
    pub fn sync_presentation_drawables_observed_with_content<I>(
        &mut self,
        entries: I,
        sync_ms: u32,
        content: &mut game_engine::common::thing::module_factory::ModuleFactory,
        policy: ReceivingDrawablePolicy,
        observe: impl FnMut(PresentationDrawableMutation<'_>),
    ) -> GameClientResult<(usize, usize, usize)>
    where
        I: IntoIterator<Item = PresentationDrawableSync>,
    {
        self.sync_presentation_drawables_with_content_inner(
            entries,
            sync_ms,
            Some(content),
            Some(policy),
            observe,
        )
    }

    fn sync_presentation_drawables_with_content_inner<I>(
        &mut self,
        entries: I,
        sync_ms: u32,
        mut content: Option<&mut game_engine::common::thing::module_factory::ModuleFactory>,
        policy: Option<ReceivingDrawablePolicy>,
        mut observe: impl FnMut(PresentationDrawableMutation<'_>),
    ) -> GameClientResult<(usize, usize, usize)>
    where
        I: IntoIterator<Item = PresentationDrawableSync>,
    {
        use crate::drawable::DrawableExt;

        let mut created = 0usize;
        let mut updated = 0usize;
        let mut live_bindings = std::collections::HashSet::new();

        for e in entries {
            let receiving_tod = if e.module_admission.authored_template().is_some() {
                Some(
                    policy
                        .and_then(|policy| policy.time_of_day)
                        .ok_or_else(|| {
                            GameClientError::InvalidOperation(
                                "authored Drawable requires receiving time of day".into(),
                            )
                        })?,
                )
            } else {
                None
            };

            let semantic_lifetime = self
                .drawables
                .drawable_object_map
                .get(&e.object_id)
                .and_then(|id| self.drawables.presentation_direct_drawable_bindings.get(id))
                .is_some_and(|binding| {
                    binding.semantic_lifetime && binding.binding_key.host_epoch == e.host_epoch
                });
            if !e.resident && !semantic_lifetime {
                continue;
            }
            let visual_template_name = Self::presentation_visual_template_name(&e).to_string();
            let existing_drawable_id = self
                .drawables
                .drawable_object_map
                .get(&e.object_id)
                .copied();
            let retains_binding = existing_drawable_id.is_some_and(|drawable_id| {
                self
                    .drawables
                    .presentation_direct_drawable_bindings
                    .get(&drawable_id)
                    .map(|binding| {
                        binding.binding_key.host_epoch == e.host_epoch
                            && binding.binding_key.object_id == e.object_id
                            && binding.binding_key.drawable_id == drawable_id
                            && binding.visual_template_name == visual_template_name
                            && self.drawables.drawable_map.get(&drawable_id).is_some_and(
                                |drawable| drawable.get_object_id() == Some(e.object_id)
                                    && drawable.as_any().downcast_ref::<BasicDrawable>().is_some_and(|basic| {
                                        basic.receiving_template_matches(match &e.module_admission {
                                            PresentationModuleAdmission::Authored(plan) => Some(plan),
                                            PresentationModuleAdmission::Compatibility => None,
                                        })
                                    }),
                            )
                    })
                    .unwrap_or(false)
            });
            if e.module_admission.authored_template().is_some()
                && e.receiving_science_player.is_none()
            {
                let requires_player = if retains_binding {
                    // LOD is a constructor choice. A retained High-LOD module
                    // still needs facts after the receiving policy becomes Low.
                    self.drawables
                        .drawable_map
                        .get_mut(&existing_drawable_id.expect("retained binding has an id"))
                        .and_then(|drawable| drawable.as_any_mut().downcast_mut::<BasicDrawable>())
                        .is_some_and(|drawable| {
                            drawable.get_draw_modules_mut().iter_mut().any(|slot| {
                                slot.logic_module_mut().is_some_and(|module| {
                                    module
                                        .get_module_data()
                                        .as_any()
                                        .downcast_ref::<W3DScienceModelDrawModuleData>()
                                        .is_some_and(|data| {
                                            data.required_science
                                                != game_engine::common::rts::SCIENCE_INVALID
                                        })
                                })
                            })
                        })
                } else {
                    Self::fresh_drawable_requires_science(&e, policy)
                };
                if requires_player {
                    return Err(GameClientError::InvalidOperation(
                        "authored science Drawable requires its receiving local-player observation"
                            .into(),
                    ));
                }
            }
            live_bindings.insert((e.host_epoch, e.object_id));

            let mut inherited_material_opacity = None;
            if let Some(drawable_id) = existing_drawable_id {
                if retains_binding {
                    let Some(drawable) = self.drawables.drawable_map.get_mut(&drawable_id) else {
                        continue;
                    };
                    let position = Vector3::new(e.position[0], e.position[1], e.position[2]);
                    drawable.set_position(position);
                    if let Some(basic) = drawable.downcast_ref::<BasicDrawable>() {
                        observe(PresentationDrawableMutation::TransformInstalled {
                            source: &e,
                            transform: basic.native_base_transform(),
                        });
                    }
                    let transform = Matrix4::rotation_y(e.orientation);
                    drawable.set_base_transform(transform);
                    if let Some(basic) = drawable.downcast_ref::<BasicDrawable>() {
                        observe(PresentationDrawableMutation::TransformInstalled {
                            source: &e,
                            transform: basic.native_base_transform(),
                        });
                    }
                    if let Some(basic) = drawable.downcast_mut::<BasicDrawable>() {
                        basic.admit_receiving_time_of_day(receiving_tod);
                        if !visual_template_name.is_empty() {
                            basic.set_template_name_with_receiving(
                                Some(visual_template_name.clone()),
                                match &e.module_admission {
                                    PresentationModuleAdmission::Authored(template) => {
                                        Some(template.clone())
                                    }
                                    PresentationModuleAdmission::Compatibility => None,
                                },
                            );
                        }
                        Self::stamp_presentation_object_residual(basic, &e, true);
                        let flags = basic.get_model_condition_flags();
                        let bits = (0..128.min(flags.get_bit_names().len()))
                            .fold(0u128, |bits, i| bits | ((flags.test(i) as u128) << i));
                        observe(PresentationDrawableMutation::ConditionsMarked {
                            source: &e,
                            bits,
                        });
                    }
                    updated = updated.saturating_add(1);
                    continue;
                }

                // C++ Drawable::imitateStealthLook copies the second pass
                // across an ordinary visual replacement of the same Object.
                // A different host epoch is a new world and inherits nothing.
                let same_owner = self
                    .drawables
                    .presentation_direct_drawable_bindings
                    .get(&drawable_id)
                    .is_some_and(|binding| {
                        binding.binding_key.host_epoch == e.host_epoch
                            && binding.binding_key.object_id == e.object_id
                            && binding.binding_key.drawable_id == drawable_id
                    });
                if same_owner {
                    inherited_material_opacity = self
                        .drawables
                        .drawable_map
                        .get(&drawable_id)
                        .filter(|drawable| drawable.get_object_id() == Some(e.object_id))
                        .and_then(|drawable| drawable.as_any().downcast_ref::<BasicDrawable>())
                        .map(BasicDrawable::material_pass_opacity);
                }
                // Destruction must succeed before displacing any identity or
                // creating its replacement; failure keeps the installed owner.
                self.destroy_drawable(drawable_id)?;
                observe(PresentationDrawableMutation::Destroyed {
                    host_epoch: e.host_epoch,
                    object_id: e.object_id,
                });
                if self
                    .drawables
                    .drawable_object_map
                    .get(&e.object_id)
                    .copied()
                    == Some(drawable_id)
                {
                    self.drawables.drawable_object_map.remove(&e.object_id);
                }
                self.drawables
                    .presentation_direct_drawable_bindings
                    .remove(&drawable_id);
            }

            self.borrow_native_drawable_owner().admit_fresh_drawable(
                &e,
                sync_ms,
                content.as_deref_mut(),
                policy,
                semantic_lifetime,
                NativeConstructorAmbientPolicy::PresentationStartup,
                inherited_material_opacity,
                &mut NativeParticleRetirementService,
                &mut |event| {
                    observe(event);
                    Ok(())
                },
            )?;
            created = created.saturating_add(1);
        }

        let stale: Vec<(DrawableId, PresentationDirectDrawableBindingKey)> = self
            .drawables
            .presentation_direct_drawable_bindings
            .iter()
            .filter_map(|(&drawable_id, binding)| {
                let binding_key = binding.binding_key;
                if binding.semantic_lifetime
                    || live_bindings.contains(&(binding_key.host_epoch, binding_key.object_id))
                {
                    None
                } else {
                    Some((drawable_id, binding_key))
                }
            })
            .collect();
        let mut pruned = 0usize;
        for (drawable_id, binding_key) in stale {
            self.destroy_drawable(drawable_id)?;
            observe(PresentationDrawableMutation::Destroyed {
                host_epoch: binding_key.host_epoch,
                object_id: binding_key.object_id,
            });
            if self
                .drawables
                .drawable_object_map
                .get(&binding_key.object_id)
                .copied()
                == Some(drawable_id)
            {
                self.drawables
                    .drawable_object_map
                    .remove(&binding_key.object_id);
            }
            self.drawables
                .presentation_direct_drawable_bindings
                .remove(&drawable_id);
            pruned = pruned.saturating_add(1);
        }

        Ok((created, updated, pruned))
    }

    fn presentation_visual_template_name(e: &PresentationDrawableSync) -> &str {
        if e.visual_template_name.is_empty() {
            &e.template_name
        } else {
            &e.visual_template_name
        }
    }

    fn alloc_presentation_direct_binding_generation(&mut self) -> u64 {
        self.drawables.alloc_binding_generation()
    }

    fn stamp_presentation_object_residual(
        drawable: &mut BasicDrawable,
        e: &PresentationDrawableSync,
        advance_specialized: bool,
    ) {
        use game_engine::common::bit_flags::{ModelConditionFlags, create_model_condition_flags};
        use gamelogic::common::types::BodyDamageType;

        let bit_names = ModelConditionFlags::BIT_NAMES;
        let mut set = create_model_condition_flags();
        let mut clear_all = create_model_condition_flags();
        let n = bit_names.len().min(128);
        // Main's authored receiving entry carries its session TOD. Core-only
        // compatibility Drawables retain the original ambient client NIGHT.
        // SNOW remains client-owned until its separate producer migration.
        let receiving = e.module_admission.authored_template().is_some();
        for i in 0..n {
            if i == ModelConditionFlags::SNOW || (i == ModelConditionFlags::NIGHT && !receiving) {
                continue;
            }
            clear_all.set(i, true);
            if (e.model_condition_bits >> i) & 1 == 1 {
                set.set(i, true);
            }
        }
        drawable.clear_and_set_model_condition_flags(&clear_all, &set);

        let body = match e.body_damage_state {
            1 => BodyDamageType::Damaged,
            2 => BodyDamageType::ReallyDamaged,
            3 => BodyDamageType::Rubble,
            _ => BodyDamageType::Pristine,
        };
        drawable.react_to_body_damage_state_change(body);

        // Wave 965: host residual cache for kind/stealth/color/health without dual-world.
        let r = (e.team_color[0].clamp(0.0, 1.0) * 255.0) as u8;
        let g = (e.team_color[1].clamp(0.0, 1.0) * 255.0) as u8;
        let b = (e.team_color[2].clamp(0.0, 1.0) * 255.0) as u8;
        let health_pct = if e.health_max > 0.0 {
            (e.health_current / e.health_max).clamp(0.0, 1.0)
        } else {
            0.0
        };
        drawable.set_presentation_host_residual(
            std::sync::Arc::clone(&e.kind_names),
            Some((r, g, b)),
            e.effectively_stealthed,
            e.scene_hidden_by_stealth,
            health_pct,
            e.selected,
            e.veterancy_level,
            e.under_construction,
            e.construction_percent,
            e.ammo_pip_total,
            e.ammo_pip_full,
            e.occupant_count,
            e.max_garrison,
            e.disabled,
            e.is_carbomb,
            e.bomb_type,
            e.bomb_timer_seconds,
            e.weapon_bonus_enthusiastic,
            e.orientation,
            e.show_healing,
            e.healing_icon_type,
            e.garrisoned_ids.clone(),
            e.emoticon_name.clone(),
            e.emoticon_frames_left,
            e.formation_id,
            e.caption.clone(),
        );
        drawable.set_presentation_health_box(e.health_box_width, e.health_box_z_offset);
        // Wave 1115: sold residual after host overlay stamp (C++ OBJECT_STATUS_SOLD).
        drawable.set_presentation_sold(e.sold);
        // Factory birth binds modules; ClientUpdate belongs to the later
        // presentation update, not sendObjectCreated/friend_bindToObject.
        if advance_specialized {
            Self::tick_presentation_specialized_draw_modules(drawable, e);
        }
        let visual = if e.visual_template_name.is_empty() {
            e.template_name.as_str()
        } else {
            e.visual_template_name.as_str()
        };
        if e.module_admission.authored_template().is_none() {
            if let Some(data) = leftover_science_model_data(visual) {
                if tick_live_host_science_model_hide(visual, Some(&data)) {
                    drawable.set_drawable_hidden(true);
                }
            }
        }
        // Authored science gates its actual typed module at doDrawModule, not
        // this whole Drawable or an unbound temporary compatibility module.
    }

    /// Apply presentation cinematic letterbox residual to GraphicsDisplay.
    ///
    /// C++ script camera letterbox residual without dual-owning Main RenderPipeline 3D draw.
    pub fn apply_presentation_cinematic_letterbox(&mut self, enabled: bool) {
        self.script_display.fx_mut().set_letterbox_enabled(enabled);
    }

    /// C++ `W3DDisplay::renderLetterBox` fade (LETTER_BOX_FADE_TIME = 1000 ms).
    pub fn letterbox_overlay_fade(&self) -> f32 {
        const LETTER_BOX_FADE_TIME_MS: f32 = 1000.0;
        let Some(start) = self.script_display.fx().letterbox_fade_start_time else {
            return if self.script_display.fx().letterbox_enabled() {
                1.0
            } else {
                0.0
            };
        };
        let t = (start.elapsed().as_millis() as f32 / LETTER_BOX_FADE_TIME_MS).clamp(0.0, 1.0);
        if self.script_display.fx().letterbox_enabled() {
            t
        } else {
            1.0 - t
        }
    }

    /// Whether letterbox bars should be drawn (enabled or still fading out).
    pub fn letterbox_overlay_visible(&self) -> bool {
        self.letterbox_overlay_fade() > 0.0
    }

    pub fn letterbox_overlay_enabled(&self) -> bool {
        self.script_display.fx().letterbox_enabled()
    }

    /// C++ W3DDisplay::draw movie blit + copyright + letterbox on the live present path.
    pub fn queue_host_display_overlays(&mut self, width: f32, height: f32) {
        let fade = self.letterbox_overlay_fade();
        let enabled = self.letterbox_overlay_enabled();
        let (display, renderer) = self.split_script_display_and_ui_renderer();
        if let Some(renderer) = renderer {
            display.fx_mut().present_host_overlays(
                renderer,
                width.max(1.0) as u32,
                height.max(1.0) as u32,
                fade,
                enabled,
            );
        }
    }

    /// Apply presentation military caption residual to InGameUI subsystem.
    ///
    /// C++ military subtitle residual; duration from presentation freeze.
    pub fn apply_presentation_military_caption(
        &mut self,
        caption: Option<&str>,
        remaining_ms: Option<i32>,
        frame: u32,
    ) {
        let text = caption.filter(|t| !t.is_empty());
        let incoming_ms = remaining_ms.unwrap_or(-1);
        // Presentation freezes each frame. C++ militarySubtitle always
        // remove+recreate. Re-arm when leftover remaining_ms jumps up
        // for the same string (scripts_camera re-fire).
        if self.last_applied_military_caption.as_deref() == text {
            if incoming_ms
                > self
                    .last_applied_military_caption_remaining_ms
                    .unwrap_or(-1)
            {
                if let Some(text) = text {
                    self.push_presentation_military_caption(text, remaining_ms, frame);
                }
            }
            self.last_applied_military_caption_remaining_ms = remaining_ms;
            return;
        }
        self.last_applied_military_caption = text.map(|t| t.to_string());
        self.last_applied_military_caption_remaining_ms = remaining_ms;
        let Some(text) = text else {
            // C++ removeMilitarySubtitle deletes the subtitle — nothing remains.
            if let Some(ui) = self.subsystem_manager.in_game_ui.as_mut() {
                {
                    let ui = ui;
                    ui.clear_military_subtitles();
                }
            }
            return;
        };
        self.push_presentation_military_caption(text, remaining_ms, frame);
    }

    fn push_presentation_military_caption(
        &mut self,
        text: &str,
        remaining_ms: Option<i32>,
        frame: u32,
    ) {
        let Some(ui) = self.subsystem_manager.in_game_ui.as_mut() else {
            return;
        };
        let ui = ui;
        let ms = remaining_ms.unwrap_or(10_000).max(0);
        self.window_manager_owner.with_manager(|manager| {
            crate::gui::callbacks::diplomacy::update_diplomacy_briefing_text(
                text,
                false,
                manager,
                self.menu_manager.diplomacy_mut(),
            );
            ui.push_military_subtitle(text, ms, frame, manager);
        });
    }

    /// Wave 1060: presentation floating cash/text residual → InGameUI subsystem residual.
    pub fn apply_presentation_floating_texts(
        &mut self,
        entries: &[(String, [f32; 3], (u8, u8, u8, u8), u32, u32)],
    ) {
        let Some(ui) = self.subsystem_manager.in_game_ui.as_mut() else {
            return;
        };
        let ui = ui;
        ui.replace_floating_texts_from_presentation(entries);
    }

    /// Presentation MoneyPickUp / world Anim2D residual → InGameUI postDraw.
    pub fn apply_presentation_world_anims(
        &mut self,
        host_epoch: u64,
        logic_frame: u32,
        game_paused: bool,
        entries: &[game_engine::common::world_animation::WorldAnimationPresentation],
    ) {
        self.owned_world_animations
            .observe_owner(host_epoch, logic_frame, game_paused, entries);
        let Some(ui) = self.subsystem_manager.in_game_ui.as_mut() else {
            return;
        };
        let ui = ui;
        ui.replace_world_anims_from_presentation(entries);
    }

    /// Presentation PublicTimer residual → InGameUI postDraw countdown strip.
    pub fn apply_presentation_superweapon_timers(&mut self, timers: &[(String, String, bool)]) {
        let Some(ui) = self.subsystem_manager.in_game_ui.as_mut() else {
            return;
        };
        let ui = ui;
        let packed: Vec<crate::core::subsystems::PresentationSuperweaponTimerResidual> = timers
            .iter()
            .map(|(name, countdown_text, ready)| {
                crate::core::subsystems::PresentationSuperweaponTimerResidual {
                    name: name.clone(),
                    countdown_text: countdown_text.clone(),
                    ready: *ready,
                }
            })
            .collect();
        ui.replace_superweapon_timers_from_presentation(&packed);
    }

    /// Host injection point for the InGameUI subsystem (creation order is
    /// host-owned; tests install a default subsystem to observe the HUD).
    pub fn set_in_game_ui_subsystem(&mut self, ui: crate::core::subsystems::InGameUISubsystem) {
        let mut ui = ui;
        if self.input_authority == ClientInputAuthority::NativeHosted {
            ui.admit_native_control_bar_transport();
        }
        self.subsystem_manager.in_game_ui = Some(ui);
    }

    /// Borrow the already installed receiving mouse service; never selects the singleton.
    pub fn input_mouse(&self) -> Option<&crate::input::Mouse> {
        self.subsystem_manager.input_mouse.as_ref()
    }
    pub fn input_mouse_mut(&mut self) -> Option<&mut crate::input::Mouse> {
        self.subsystem_manager.input_mouse.as_mut()
    }

    /// Borrow this client's installed UI; no published or cloned mutable owner.
    pub fn in_game_ui_subsystem(&self) -> Option<&crate::core::subsystems::InGameUISubsystem> {
        self.subsystem_manager.in_game_ui.as_ref()
    }
    pub fn in_game_ui_subsystem_mut(
        &mut self,
    ) -> Option<&mut crate::core::subsystems::InGameUISubsystem> {
        self.subsystem_manager.in_game_ui.as_mut()
    }

    /// C++ InGameUI::addSuperweapon bridge (InGameUI.cpp:548-580): drain
    /// GameLogic's TheInGameUI::add_superweapon store into the live client
    /// superweapon strip, tick the countdown, and bridge the visible draw
    /// entries into the presentation residual impl_draw renders each HUD
    /// build. Runs from the client frame tick (dual-world and presentation
    /// shell paths); C++ is same-process so this store did not exist there.
    pub fn sync_superweapon_strip_from_logic(&mut self) {
        if self.input_authority == ClientInputAuthority::NativeHosted {
            return;
        }
        let entries = gamelogic::helpers::TheInGameUI::superweapon_entries();
        let Some(ui) = self.subsystem_manager.in_game_ui.as_mut() else {
            return;
        };
        let guard = ui;
        crate::gui::ingame_ui::live_superweapon_strip_sync(
            &mut guard.superweapon_strip,
            &entries,
            self.frame,
        );
        let packed: Vec<crate::core::subsystems::PresentationSuperweaponTimerResidual> =
            crate::gui::ingame_ui::live_superweapon_draw_entries(&mut guard.superweapon_strip)
                .into_iter()
                .map(|(name, countdown_text, ready)| {
                    crate::core::subsystems::PresentationSuperweaponTimerResidual {
                        name,
                        countdown_text,
                        ready,
                    }
                })
                .collect();
        guard.replace_superweapon_timers_from_presentation(&packed);
    }

    /// Display a message through this client's HUD with its logic frame.
    pub fn push_hud_message_at(&mut self, message: String, frame: u32) -> bool {
        let Some(ui) = self.subsystem_manager.in_game_ui.as_mut() else {
            return false;
        };
        let ui = ui;
        ui.push_hud_message_at(message, frame);
        true
    }

    /// C++ `TheInGameUI->message` HUD bridge: drain GameLogic's displayed
    /// message store into the drawn InGameUI-subsystem HUD messages with
    /// GameText localization (VictoryConditions.cpp:174-176 pushes
    /// `GUI:PlayerHasBeenDefeated` + player name; radar messages push
    /// `RADAR:*` labels). The first local defeat also forces the radar on
    /// and switches in-game chat to Everyone (VictoryConditions.cpp:200-215).
    pub fn drain_logic_hud_messages(&mut self, logic_frame: u32) {
        if self.input_authority == ClientInputAuthority::NativeHosted {
            return;
        }
        let messages = gamelogic::helpers::TheInGameUI::drain_displayed_messages();
        if messages.is_empty() {
            return;
        }
        let Some(ui) = self.subsystem_manager.in_game_ui.as_mut() else {
            return;
        };
        let guard = ui;
        for message in messages {
            let (display, defeat) = Self::localize_logic_hud_message(&message);
            guard.push_hud_message_at(display, logic_frame);
            if defeat && gamelogic::helpers::TheVictoryConditions::is_local_defeat() {
                // C++ VictoryConditions.cpp:212-213 (one-shot latch on the
                // local player's elimination).
                if let Ok(mut radar) =
                    game_engine::common::system::radar::get_radar_system().write()
                {
                    radar.force_on(true);
                }
                let label = self.menu_manager.ingame_ui_mut().assign_chat_type(
                    crate::gui::callbacks::ingame_callbacks::InGameChatType::Everyone,
                );
                if let Some(label) = label {
                    let _ = label.borrow_mut().set_text(
                        &crate::gui::callbacks::ingame_callbacks::chat_type_label(
                            &crate::gui::callbacks::ingame_callbacks::InGameChatType::Everyone,
                            false,
                        ),
                    );
                }
            }
        }
    }

    /// Localize one GameLogic HUD message the way C++ `InGameUI::message`
    /// does: the label is a GameText key; `GUI:PlayerHasBeenDefeated {name}`
    /// substitutes the player name into the localized template. Returns the
    /// display text and whether this is a defeat announcement.
    fn localize_logic_hud_message(message: &str) -> (String, bool) {
        const DEFEAT_LABEL: &str = "GUI:PlayerHasBeenDefeated";
        if message == DEFEAT_LABEL {
            return Self::localize_defeat_message("");
        }
        if let Some(rest) = message.strip_prefix(DEFEAT_LABEL) {
            if rest.starts_with(' ') {
                return Self::localize_defeat_message(rest.trim_start());
            }
        }
        // Plain label-style strings ("RADAR:UnderAttack") localize through
        // the same GameText seam the rest of the HUD uses; anything the
        // string table does not know is displayed verbatim.
        if message.contains(':') {
            let (text, exists) = crate::game_text::GameText::fetch_with_exists(message);
            if exists {
                return (text, false);
            }
        }
        (message.to_string(), false)
    }

    fn localize_defeat_message(name: &str) -> (String, bool) {
        let template = crate::game_text::GameText::fetch("GUI:PlayerHasBeenDefeated");
        if template.starts_with("MISSING:") {
            return (format!("{name} has been defeated"), true);
        }
        (template.replace("%s", name), true)
    }

    /// Apply presentation cinematic text as a W3DDisplay caption residual.
    ///
    /// C++ `doDisplayCinematicText` → `setCinematicText` / `setCinematicFont` /
    /// `setCinematicTextFrames(LOGICFRAMES_PER_SECOND * time)`. Drawn centered
    /// at 90% screen height over the letterbox; not an InGameUI HUD message.
    pub fn apply_presentation_cinematic_text(
        &mut self,
        text: Option<&str>,
        remaining_ms: Option<i32>,
        font: Option<&str>,
    ) {
        let text = text.filter(|t| !t.is_empty());
        let incoming_ms = remaining_ms.unwrap_or(-1);
        if self.last_applied_cinematic_text.as_deref() == text {
            // C++ resets frames on every fire. Leftover remaining_ms jumps up
            // only when DISPLAY_CINEMATIC_TEXT re-arms the same string.
            if incoming_ms > self.last_applied_cinematic_remaining_ms.unwrap_or(-1) {
                self.arm_cinematic_overlay(remaining_ms, font);
            } else if let Some(font) = font.filter(|f| !f.is_empty()) {
                self.cinematic_overlay_font = Some(font.to_string());
            }
            self.last_applied_cinematic_remaining_ms = remaining_ms;
            return;
        }
        self.last_applied_cinematic_text = text.map(|t| t.to_string());
        self.last_applied_cinematic_remaining_ms = remaining_ms;
        let Some(_) = text else {
            self.cinematic_overlay_font = None;
            self.cinematic_overlay_frames = 0;
            return;
        };
        self.arm_cinematic_overlay(remaining_ms, font);
    }

    fn arm_cinematic_overlay(&mut self, remaining_ms: Option<i32>, font: Option<&str>) {
        self.cinematic_overlay_font = font.filter(|f| !f.is_empty()).map(|f| f.to_string());
        // C++ frames = LOGICFRAMES_PER_SECOND * time; remaining_ms is the
        // live residual of that countdown.
        const LOGICFRAMES_PER_SECOND: u32 = 30;
        self.cinematic_overlay_frames = remaining_ms
            .map(|ms| ((ms.max(0) as u32) * LOGICFRAMES_PER_SECOND + 999) / 1000)
            .filter(|frames| *frames > 0)
            .unwrap_or(LOGICFRAMES_PER_SECOND * 10);
    }

    /// C++ `ScriptActions::doDisplayCinematicText` fontType parse.
    /// WorldBuilder: `"Name - Size:N"` / `"Name - Size:N [Bold]"`.
    /// GUIEdit: `"Name - Size: N [Bold]"`.
    pub fn parse_cinematic_font_type(font_type: &str) -> (String, i32, bool) {
        let bold = font_type.ends_with("[Bold]");
        let size = font_type
            .rsplit_once("Size:")
            .and_then(|(_, rest)| {
                let digits: String = rest
                    .trim_start()
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                digits.parse().ok()
            })
            .unwrap_or(0);
        let name = font_type
            .split_once(" - ")
            .map(|(name, _)| name.trim().to_string())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| {
                font_type
                    .split(|c: char| c == ' ' || c == '-')
                    .next()
                    .unwrap_or(font_type)
                    .to_string()
            });
        (name, size, bold)
    }

    /// C++ `adjustFontSize` of the parsed FONT_NAME point size.
    pub fn cinematic_caption_point_size(font_type: Option<&str>) -> i32 {
        let parsed = font_type
            .map(Self::parse_cinematic_font_type)
            .map(|(_, size, _)| size)
            .filter(|&size| size > 0)
            .unwrap_or(12);
        crate::global_language::get_global_language_data()
            .read()
            .map(|data| data.adjust_font_size(parsed))
            .unwrap_or(parsed)
    }

    /// Live cinematic caption (text, optional font, remaining rendered frames).
    pub fn cinematic_overlay(&self) -> Option<(&str, Option<&str>, u32)> {
        let text = self.last_applied_cinematic_text.as_deref()?;
        if self.cinematic_overlay_frames == 0 {
            return None;
        }
        Some((
            text,
            self.cinematic_overlay_font.as_deref(),
            self.cinematic_overlay_frames,
        ))
    }

    /// C++ `m_cinematicTextFrames--` once per rendered frame.
    pub fn decrement_cinematic_overlay_frame(&mut self) {
        if self.cinematic_overlay_frames > 0 {
            self.cinematic_overlay_frames -= 1;
            if self.cinematic_overlay_frames == 0 {
                self.last_applied_cinematic_text = None;
                self.last_applied_cinematic_remaining_ms = None;
                self.cinematic_overlay_font = None;
            }
        }
    }

    /// Wave 964: presentation selection residual → InGameUI (host empty dual-world).
    pub fn apply_presentation_selection_residual(
        &mut self,
        units: Vec<crate::gui::ingame_ui::PresentationSelectedUnitResidual>,
    ) {
        if let Some(ui) = self.subsystem_manager.in_game_ui.as_mut() {
            {
                let guard = ui;
                guard.set_presentation_selection_residual(units);
            }
        }
    }

    /// Wave 966: stamp presentation unit catalog residual for host select-similar.
    pub fn apply_presentation_unit_catalog(
        &mut self,
        units: Vec<crate::gui::ingame_ui::PresentationUnitCatalogEntry>,
    ) {
        // Wave 974: catalog position for context pick.
        // Wave 973: stamp translator residual before moving into InGameUI store.
        let translator_catalog: Vec<
            crate::presentation_translator_residual::TranslatorCatalogEntry,
        > = units
            .iter()
            .map(
                |u| crate::presentation_translator_residual::TranslatorCatalogEntry {
                    object_id: u.object_id,
                    template_name: u.template_name.clone(),
                    team_name: u.team_name.clone(),
                    selectable: u.selectable,
                    kind_names: u.kind_names.clone(),
                    special_power_ready: u.special_power_ready,
                    position: u.position,
                    orientation: u.orientation,
                    disabled: u.disabled,
                    under_construction: u.under_construction,
                    construction_percent: u.construction_percent,
                    max_garrison: u.max_garrison,
                    occupant_count: u.occupant_count,
                    ocl_timer_seconds: u.ocl_timer_seconds,
                    sold: u.sold,
                    script_unsellable: u.script_unsellable,
                    unselectable: u.unselectable,
                    destroyed: u.destroyed,
                    masked: u.masked,
                    effectively_stealthed: u.effectively_stealthed,
                    // Wave 1041: disguise residual.
                    disguised: u.disguised,
                    disguise_as_template: u.disguise_as_template.clone(),
                    disguise_as_team: u.disguise_as_team.clone(),
                    airborne_target: u.airborne_target,
                    shroud_status: u.shroud_status as u8,
                    slaver_object_id: u.slaver_object_id,
                    health_current: u.health_current,
                    health_maximum: u.health_maximum,
                    veterancy_overlay: u.veterancy_overlay.clone(),
                    production_progress: u.production_progress,
                    production_template: u.production_template.clone(),
                    production_paused: u.production_paused,
                    command_set_name: u.command_set_name.clone(),
                    // Wave 1055: hotkey group residual.
                    hotkey_group: u.hotkey_group,
                    caption: u.caption.clone(),
                },
            )
            .collect();
        let local_team = if let Some(ui) = self.subsystem_manager.in_game_ui.as_mut() {
            ui.presentation_local_team_name().to_string()
        } else {
            String::new()
        };
        crate::presentation_translator_residual::set_translator_presentation_residual(
            local_team,
            translator_catalog,
        );

        // Wave 1055: stamp control-group residual onto drawable presentation shell.
        for u in &units {
            if let Some(did) = self
                .drawables
                .drawable_object_map
                .get(&u.object_id)
                .copied()
            {
                if let Some(drawable) = self.drawables.drawable_map.get_mut(&did) {
                    if let Some(basic) = drawable
                        .as_any_mut()
                        .downcast_mut::<crate::drawable::drawable::BasicDrawable>()
                    {
                        basic.set_presentation_hotkey_group(u.hotkey_group);
                        basic.set_presentation_sold(u.sold);
                    }
                }
            }
        }

        if let Some(ui) = self.subsystem_manager.in_game_ui.as_mut() {
            {
                let guard = ui;
                guard.set_presentation_unit_catalog(units);
            }
        }
    }

    /// Wave 968: stamp local player team residual for host ownership queries.
    pub fn apply_presentation_local_team_name(&mut self, team_name: impl Into<String>) {
        let team_name = team_name.into();
        if let Some(ui) = self.subsystem_manager.in_game_ui.as_mut() {
            {
                let guard = ui;
                guard.set_presentation_local_team_name(team_name.clone());
                // Wave 973: refresh translator residual local team, keep catalog.
                let catalog: Vec<_> = guard
                    .presentation_unit_catalog()
                    .iter()
                    .map(
                        |u| crate::presentation_translator_residual::TranslatorCatalogEntry {
                            object_id: u.object_id,
                            template_name: u.template_name.clone(),
                            team_name: u.team_name.clone(),
                            selectable: u.selectable,
                            kind_names: u.kind_names.clone(),
                            special_power_ready: u.special_power_ready,
                            position: u.position,
                            orientation: u.orientation,
                            disabled: u.disabled,
                            under_construction: u.under_construction,
                            construction_percent: u.construction_percent,
                            max_garrison: u.max_garrison,
                            occupant_count: u.occupant_count,
                            ocl_timer_seconds: u.ocl_timer_seconds,
                            sold: u.sold,
                            script_unsellable: u.script_unsellable,
                            unselectable: u.unselectable,
                            destroyed: u.destroyed,
                            masked: u.masked,
                            effectively_stealthed: u.effectively_stealthed,
                            // Wave 1041: disguise residual.
                            disguised: u.disguised,
                            disguise_as_template: u.disguise_as_template.clone(),
                            disguise_as_team: u.disguise_as_team.clone(),
                            airborne_target: u.airborne_target,
                            shroud_status: u.shroud_status as u8,
                            slaver_object_id: u.slaver_object_id,
                            health_current: u.health_current,
                            health_maximum: u.health_maximum,
                            veterancy_overlay: u.veterancy_overlay.clone(),
                            production_progress: u.production_progress,
                            production_template: u.production_template.clone(),
                            production_paused: u.production_paused,
                            command_set_name: u.command_set_name.clone(),
                            // Wave 1055: hotkey group residual.
                            hotkey_group: u.hotkey_group,
                            caption: u.caption.clone(),
                        },
                    )
                    .collect();
                crate::presentation_translator_residual::set_translator_presentation_residual(
                    team_name, catalog,
                );
            }
        } else {
            crate::presentation_translator_residual::set_translator_presentation_residual(
                team_name,
                Vec::new(),
            );
        }
    }

    /// C++ HintSpy.cpp:26-35 — live host has no MessageStream mouseover
    /// messages, so Main calls this from SelectionXlat-equivalent hover.
    pub fn create_mouseover_hint(
        &mut self,
        drawable_id: Option<u32>,
        is_location_hint: bool,
        logic_frame: u32,
        facts: &crate::gui::ingame_ui::ReceivingMouseoverFacts,
    ) {
        if let (Some(ui), Some(mouse)) = (
            self.subsystem_manager.in_game_ui.as_mut(),
            self.subsystem_manager.input_mouse.as_mut(),
        ) {
            ui.create_mouseover_hint(
                drawable_id,
                is_location_hint,
                logic_frame,
                self.frame,
                mouse,
                facts,
                &mut self.drawable_manager,
            );
        }
    }

    /// C++ InGameUI.cpp:2462 — TheRecorder playback + LookAtXlat
    /// `hasMouseMovedRecently`. Live host owns both signals and must stamp
    /// leftover InGameUI before `create_mouseover_hint`.
    pub fn feed_look_at_replay_hover_gate(&mut self, playback: bool, mouse_moved_recently: bool) {
        if let Some(ui) = self.subsystem_manager.in_game_ui.as_mut() {
            {
                let guard = ui;
                guard.set_recorder_playback_active(playback);
                guard.set_look_at_mouse_moved_recently(mouse_moved_recently);
            }
        }
    }

    /// Shell/presentation client tick without dual-world OBJECT_REGISTRY drawable bind.
    ///
    /// Mirrors the safe subset of C++ `GameClient::update` ordering that Main does not
    /// already own as dual presenters. Includes frame tick, client input device poll,
    /// client audio subsystem drain, local drawable modules, FX/weather residual,
    /// post-draw UI, beacon notifications, and message pump.
    /// Main still owns OS WindowEvent→commands, GameLogic audio event requests, and
    /// sole RenderPipeline 3D draw (`draw_display` stays off). Shroud still comes from
    /// PresentationFrame at render time — not live OBJECT_REGISTRY.
    pub fn update_presentation_shell(
        &mut self,
        delta_time: f32,
        logic_frame: u32,
        game_mode: i32,
    ) -> GameClientResult<()> {
        // Compatibility entry for the classic Core driver. Native Main passes
        // both actual owners explicitly through the operation below.
        let frames = NativeParticleFrames {
            logic_frame,
            client_frame: self.frame,
        };
        self.update_presentation_shell_with_particle_policy(
            delta_time,
            frames,
            OBJECT_REGISTRY.is_empty(),
            &[],
            game_mode,
        )
    }

    pub fn update_presentation_shell_with_particle_frames(
        &mut self,
        delta_time: f32,
        particle_frames: NativeParticleFrames,
        game_mode: i32,
    ) -> GameClientResult<()> {
        self.update_presentation_shell_with_particle_policy(
            delta_time,
            particle_frames,
            true,
            &[],
            game_mode,
        )
    }

    /// Native Main passes operation-scoped facts from its driving world.
    pub fn update_presentation_shell_with_material_frames(
        &mut self,
        delta_time: f32,
        particle_frames: NativeParticleFrames,
        materials: &[NativeMaterialObservation],
        game_mode: i32,
    ) -> GameClientResult<()> {
        self.validate_material_observations(materials)?;
        self.update_presentation_shell_with_particle_policy(
            delta_time,
            particle_frames,
            true,
            materials,
            game_mode,
        )
    }

    fn update_presentation_shell_with_particle_policy(
        &mut self,
        delta_time: f32,
        particle_frames: NativeParticleFrames,
        advance_particles: bool,
        materials: &[NativeMaterialObservation],
        game_mode: i32,
    ) -> GameClientResult<()> {
        let prepared = self.begin_presentation_shell(delta_time, particle_frames, game_mode)?;
        if !prepared.skip_draw {
            self.update_native_draw_particle_phase_with_materials(
                prepared.frozen,
                prepared.frames,
                advance_particles,
                materials,
            )?;
        }
        self.finish_presentation_shell(prepared)
    }

    pub fn begin_presentation_shell_with_material_frames(
        &mut self,
        delta_time: f32,
        particle_frames: NativeParticleFrames,
        materials: &[NativeMaterialObservation],
        game_mode: i32,
    ) -> GameClientResult<PreparedPresentationShell> {
        self.validate_material_observations(materials)?;
        self.begin_presentation_shell(delta_time, particle_frames, game_mode)
    }

    pub fn begin_presentation_shell_with_owned_visual_time(
        &mut self,
        delta_time: f32,
        particle_frames: NativeParticleFrames,
        materials: &[NativeMaterialObservation],
        observe: impl FnOnce() -> NativeVisualTime,
        game_mode: i32,
    ) -> GameClientResult<PreparedPresentationShell> {
        self.try_begin_presentation_shell_with_owned_visual_time(
            delta_time,
            particle_frames,
            materials,
            || Ok(observe()),
            game_mode,
        )
    }
    /// Same late source observation point, with explicit owner failure.
    pub fn try_begin_presentation_shell_with_owned_visual_time(
        &mut self,
        delta_time: f32,
        particle_frames: NativeParticleFrames,
        materials: &[NativeMaterialObservation],
        observe: impl FnOnce() -> GameClientResult<NativeVisualTime>,
        game_mode: i32,
    ) -> GameClientResult<PreparedPresentationShell> {
        self.validate_material_observations(materials)?;
        self.begin_presentation_shell_observing(
            delta_time,
            particle_frames,
            observe,
            game_mode,
            None,
        )
    }

    /// Native Main supplies the receiving world's TOD and per-object audio
    /// locality for the final ambient phase of each Drawable update.
    pub fn try_begin_presentation_shell_with_owned_visual_time_and_ambient(
        &mut self,
        delta_time: f32,
        particle_frames: NativeParticleFrames,
        materials: &[NativeMaterialObservation],
        observe: impl FnOnce() -> GameClientResult<NativeVisualTime>,
        game_mode: i32,
        ambient: &mut crate::drawable::drawable::NativeAmbientUpdate<'_>,
    ) -> GameClientResult<PreparedPresentationShell> {
        self.validate_material_observations(materials)?;
        self.begin_presentation_shell_observing(
            delta_time,
            particle_frames,
            observe,
            game_mode,
            Some(ambient),
        )
    }

    fn begin_presentation_shell(
        &mut self,
        delta_time: f32,
        particle_frames: NativeParticleFrames,
        game_mode: i32,
    ) -> GameClientResult<PreparedPresentationShell> {
        self.begin_presentation_shell_observing(
            delta_time,
            particle_frames,
            || Ok(legacy_visual_time_observation()),
            game_mode,
            None,
        )
    }

    fn begin_presentation_shell_observing(
        &mut self,
        delta_time: f32,
        particle_frames: NativeParticleFrames,
        observe: impl FnOnce() -> GameClientResult<NativeVisualTime>,
        game_mode: i32,
        mut ambient: Option<&mut crate::drawable::drawable::NativeAmbientUpdate<'_>>,
    ) -> GameClientResult<PreparedPresentationShell> {
        if particle_frames.client_frame != self.frame {
            return Err(GameClientError::InvalidOperation(
                "native particle Client frame does not match receiving owner".into(),
            ));
        }
        if !self.initialized {
            return Err(GameClientError::InvalidOperation(
                "GameClient not initialized".to_string(),
            ));
        }

        self.presentation_shell_generation = self
            .presentation_shell_generation
            .checked_add(1)
            .ok_or_else(|| {
                GameClientError::InvalidOperation("Client presentation operation exhausted".into())
            })?;
        let current_time = Instant::now();
        self.last_update_time = current_time;
        // GameLogic owns admission of the client frame (GameLogic.cpp:3593-3596).
        // Presenting or updating UI never advances simulation time.
        publish_live_game_client_frame(self);

        self.create_frame_tick_message()?;
        // Meta TOD changes are queued on this receiving client's UI owner.
        let pending_tod = self
            .subsystem_manager
            .in_game_ui
            .as_mut()
            .and_then(|ui| ui.pending_drawable_time_of_day.take());
        if let Some(ini_tod) = pending_tod {
            let client_tod = match ini_tod {
                game_engine::common::ini::TimeOfDay::Morning => TimeOfDay::Morning,
                game_engine::common::ini::TimeOfDay::Afternoon => TimeOfDay::Afternoon,
                game_engine::common::ini::TimeOfDay::Evening => TimeOfDay::Evening,
                game_engine::common::ini::TimeOfDay::Night => TimeOfDay::Night,
                game_engine::common::ini::TimeOfDay::Invalid => TimeOfDay::Afternoon,
            };
            let _ = self.set_time_of_day(client_tod);
        }

        // Weather/time model flags are drained from this receiving client's owner.
        let pending_model_conditions = self
            .subsystem_manager
            .in_game_ui
            .as_mut()
            .and_then(|ui| ui.pending_model_condition_weather.take());
        if let Some((is_night, is_snow)) = pending_model_conditions {
            use game_engine::common::bit_flags::{
                ModelConditionFlags, create_model_condition_flags,
            };
            let mut clear = create_model_condition_flags();
            let mut set = create_model_condition_flags();
            if let Some(is_night) = is_night {
                clear.set(ModelConditionFlags::NIGHT, true);
                set.set(ModelConditionFlags::NIGHT, is_night);
            }
            if let Some(is_snow) = is_snow {
                clear.set(ModelConditionFlags::SNOW, true);
                set.set(ModelConditionFlags::SNOW, is_snow);
            }
            for drawable in self.drawables.drawable_map.values_mut() {
                use crate::drawable::drawable::DrawableExt;
                if let Some(basic) =
                    drawable.downcast_mut::<crate::drawable::drawable::BasicDrawable>()
                {
                    basic.clear_and_set_model_condition_flags(&clear, &set);
                }
            }
        }

        // Wave 984: drain contained-flash residual onto presentation drawables.
        for object_id in take_host_contained_flash_object_ids() {
            let Some(drawable_id) = self.get_drawable_for_object(object_id) else {
                continue;
            };
            if let Some(drawable) = self.drawables.drawable_map.get_mut(&drawable_id) {
                use crate::drawable::drawable::DrawableExt;
                // White selection flash residual (C++ Drawable::flashAsSelected).
                if let Some(basic) =
                    drawable.downcast_mut::<crate::drawable::drawable::BasicDrawable>()
                {
                    basic.color_flash(Vector3::new(1.0, 1.0, 1.0), 4);
                } else {
                    let _ = drawable;
                }
            }
        }

        // Native Main owns Menu admission and startup movie completion.
        // A hidden shell during a match must stay hidden: showing its map
        // here would enqueue GAME_SHELL and replace the receiving game.
        // Snow/Anim2D: C++ runs every GameClient::update. Live host ticks
        // them from cnc_game_engine residuals in every state (including when
        // this shell is skipped). Dual-world `update()` still ticks above.
        // Main owns selected-object tracking through its receiving SessionView.
        // Standalone update retains the Core drawable tracking adapter.
        self.update_pre_draw_ui()?;

        // C++ visual freeze + script visual-speed residual (same as full update),
        // without Main-owned input/audio or Display DRAW dual-ownership.
        // Device state is shared THE_MOUSE/THE_KEYBOARD (Main inject; no second OS poll).
        // C++ GameClient.cpp621-630 observes after pre-draw UI, including
        // synchronous camera/script changes made by those callbacks.
        let (freeze_time, visual_speed) = self.admit_visual_time(observe()?)?;
        let visual_delta = if freeze_time || visual_speed <= 0. {
            0.
        } else {
            delta_time * visual_speed
        };

        // Main owns OS event intake and shared THE_MOUSE/THE_KEYBOARD (no shell
        // update_input dual-tick). Audio/SFX dispatched by Main before shell tick (no update_audio dual-drain).
        // Presentation gameplay SFX is dispatched by Main via
        // PresentationFrame::dispatch_audio_events_direct before this shell tick.

        // C++ GameClient.cpp:632-657 — GhostObjectManager::updateOrphanedObjects.
        self.update_orphaned_w3d_ghosts_if_unfrozen(freeze_time);

        // Local drawable client modules only (no OBJECT_REGISTRY shroud bind).
        // Eva residual runs via update_post_draw_ui (no dual OS input ownership).
        if let Some(ambient) = ambient.as_deref_mut() {
            self.update_drawables_local_with_ambient(visual_delta, particle_frames, ambient)?;
        } else {
            self.update_drawables_local(visual_delta, particle_frames)?;
        }
        if self.should_skip_visual_updates_for_no_draw() {
            self.rendered_object_count = 0;
            // Wave 876: Main owns frame pacing — no shell dual-sleep.
            return Ok(PreparedPresentationShell {
                owner: std::sync::Arc::clone(&self.presentation_shell_owner),
                generation: self.presentation_shell_generation,
                frames: particle_frames,
                game_mode,
                frozen: freeze_time,
                skip_draw: true,
            });
        }

        self.update_native_effects(visual_delta)?;
        self.apply_core_script_display_state();
        // C++ GameClient.cpp:719-722 — TheTerrainVisual->UPDATE().
        self.update_terrain_visual();
        // C++ line 726: display UPDATE (not DRAW). Main RenderPipeline remains sole
        // 3D present path; skip draw_display to avoid dual surface present.
        self.update_display_only()?;
        Ok(PreparedPresentationShell {
            owner: std::sync::Arc::clone(&self.presentation_shell_owner),
            generation: self.presentation_shell_generation,
            frames: particle_frames,
            game_mode,
            frozen: freeze_time,
            skip_draw: false,
        })
    }

    pub fn finish_presentation_shell_with_native_views(
        &mut self,
        prepared: PreparedPresentationShell,
        materials: &[NativeMaterialObservation],
        views: &[NativeDrawView],
        phase: NativeDrawPhase,
    ) -> GameClientResult<()> {
        self.finish_presentation_shell_with_native_views_routed(
            prepared, materials, views, phase, None, None, None,
        )
    }

    pub fn finish_presentation_shell_with_receiving_commands(
        &mut self,
        prepared: PreparedPresentationShell,
        materials: &[NativeMaterialObservation],
        views: &[NativeDrawView],
        phase: NativeDrawPhase,
        recorder: &game_engine::common::recorder::Recorder,
        receive: &mut dyn FnMut(Vec<GameMessage>) -> GameClientResult<()>,
    ) -> GameClientResult<()> {
        self.finish_presentation_shell_with_native_views_routed(
            prepared,
            materials,
            views,
            phase,
            Some((recorder, receive)),
            None,
            None,
        )
    }

    pub fn finish_presentation_shell_with_receiving_commands_and_object_lifecycle(
        &mut self,
        prepared: PreparedPresentationShell,
        materials: &[NativeMaterialObservation],
        views: &[NativeDrawView],
        phase: NativeDrawPhase,
        recorder: &game_engine::common::recorder::Recorder,
        receive: &mut dyn FnMut(Vec<GameMessage>) -> GameClientResult<()>,
        lifecycle: &mut crate::effects::particle_manager::NativeParticleObjectLifecycle<'_>,
    ) -> GameClientResult<()> {
        self.finish_presentation_shell_with_native_views_routed(
            prepared,
            materials,
            views,
            phase,
            Some((recorder, receive)),
            Some(lifecycle),
            None,
        )
    }

    pub fn finish_presentation_shell_with_receiving_commands_and_object_lifecycle_and_content(
        &mut self,
        prepared: PreparedPresentationShell,
        materials: &[NativeMaterialObservation],
        views: &[NativeDrawView],
        phase: NativeDrawPhase,
        recorder: &game_engine::common::recorder::Recorder,
        receive: &mut dyn FnMut(Vec<GameMessage>) -> GameClientResult<()>,
        lifecycle: &mut crate::effects::particle_manager::NativeParticleObjectLifecycle<'_>,
        content: &mut game_engine::common::thing::module_factory::ModuleFactory,
    ) -> GameClientResult<()> {
        self.finish_presentation_shell_with_native_views_routed(
            prepared,
            materials,
            views,
            phase,
            Some((recorder, receive)),
            Some(lifecycle),
            Some(content),
        )
    }

    fn finish_presentation_shell_with_native_views_routed(
        &mut self,
        prepared: PreparedPresentationShell,
        materials: &[NativeMaterialObservation],
        views: &[NativeDrawView],
        phase: NativeDrawPhase,
        receive: Option<(
            &game_engine::common::recorder::Recorder,
            &mut dyn FnMut(Vec<GameMessage>) -> GameClientResult<()>,
        )>,
        lifecycle: Option<&mut crate::effects::particle_manager::NativeParticleObjectLifecycle<'_>>,
        content: Option<&mut game_engine::common::thing::module_factory::ModuleFactory>,
    ) -> GameClientResult<()> {
        if !std::sync::Arc::ptr_eq(&prepared.owner, &self.presentation_shell_owner)
            || prepared.generation != self.presentation_shell_generation
            || prepared.frames.client_frame != self.frame
        {
            return Err(GameClientError::InvalidOperation(
                "prepared shell Client frame changed".into(),
            ));
        }
        if !prepared.skip_draw {
            if let Some(content) = content {
                self.draw_native_views_with_object_lifecycle_and_content(
                    prepared.frames,
                    materials,
                    views,
                    phase,
                    lifecycle,
                    content,
                )?;
            } else {
                self.draw_native_views_with_object_lifecycle(
                    prepared.frames,
                    materials,
                    views,
                    phase,
                    lifecycle,
                )?;
            }
        }
        self.finish_presentation_shell_routed(prepared, receive)
    }

    fn finish_presentation_shell(
        &mut self,
        prepared: PreparedPresentationShell,
    ) -> GameClientResult<()> {
        self.finish_presentation_shell_routed(prepared, None)
    }

    fn finish_presentation_shell_routed(
        &mut self,
        prepared: PreparedPresentationShell,
        receive: Option<(
            &game_engine::common::recorder::Recorder,
            &mut dyn FnMut(Vec<GameMessage>) -> GameClientResult<()>,
        )>,
    ) -> GameClientResult<()> {
        if prepared.skip_draw {
            return Ok(());
        }
        // C++ W3DView::drawablePostDraw icon UI residual (health/status icons).
        self.draw_drawable_icon_ui();
        // Wave 980: weapon/UI residual peels companion.
        // Wave 978/980: presentation selection residual HUD (host empty dual-world InGameUI).
        self.draw_presentation_selection_residual();
        // C++ TheInGameUI->update message/superweapon drains before postDraw.
        self.drain_logic_hud_messages(prepared.frames.logic_frame);
        self.sync_superweapon_strip_from_logic();
        // C++ InGameUI::preDraw/postDraw + Drawable::drawIconUI submit.
        let _ = self.draw_live_ingame_hud(prepared.frames.logic_frame);

        // C++ DisplayStringManager after drawable/effects residual.
        self.update_display_string_manager()?;

        self.update_post_draw_ui(None)?;
        self.process_beacon_notifications(prepared.frames.logic_frame)?;
        if let Some((recorder, receive)) = receive {
            self.pump_message_stream_to_receiving_owner(
                recorder,
                receive,
                prepared.frames.logic_frame,
                prepared.game_mode,
            )?;
        } else {
            self.pump_message_stream(prepared.frames.logic_frame, prepared.game_mode, None)?;
        }

        self.rendered_object_count = 0;
        // Wave 876: Main owns frame pacing — no shell dual-sleep.
        Ok(())
    }

    fn update_orphaned_w3d_ghosts_if_unfrozen(&self, freeze_time: bool) {
        if freeze_time {
            return;
        }
        if let Ok(mut logic) = gamelogic::system::game_logic::get_game_logic().lock() {
            logic.update_orphaned_w3d_ghosts();
            return;
        }
        if let Ok(mut manager) = gamelogic::object::THE_W3D_GHOST_OBJECT_MANAGER.write() {
            manager.update_orphaned_objects(&[]);
        }
    }

    pub fn update_drawables(
        &mut self,
        delta_time: f32,
        frames: NativeParticleFrames,
    ) -> GameClientResult<()> {
        if frames.client_frame != self.frame {
            return Err(GameClientError::InvalidOperation(
                "Drawable Client frame does not match receiving owner".into(),
            ));
        }
        let frame = frames.client_frame;
        let local_player_index = self.local_player_id;

        self.tick_mapped_drawables(frames, delta_time)?;

        // Host/presentation path: Wave 1020/1021 peels catalog shroud onto drawable_map
        // when dual-world registry is empty (PresentationFrame apply_* still primary).
        if OBJECT_REGISTRY.is_empty() {
            self.update_drawable_visibility(local_player_index)?;
            let _ = frame;
            return Ok(());
        }
        let mut expired_logic_drawables = Vec::new();
        self.iterate_objects_with_drawables(|obj_ref| {
            let Ok(mut obj) = obj_ref.write() else {
                return;
            };
            let object_id = obj.get_id();
            let shroud = obj.get_shrouded_status(local_player_index);
            let is_effectively_dead = obj.is_effectively_dead();
            let fully_obscured = matches!(
                shroud,
                gamelogic::common::types::ObjectShroudStatus::Fogged
                    | gamelogic::common::types::ObjectShroudStatus::Shrouded
                    | gamelogic::common::types::ObjectShroudStatus::InvalidButPreviousValid
            );

            if let Some(drawable_arc) = obj.get_drawable() {
                if let Ok(mut drawable_guard) = drawable_arc.write() {
                    drawable_guard.set_fully_obscured_by_shroud(fully_obscured);
                    let _ = drawable_guard.update(delta_time, frame, frames.logic_frame);
                    let expiration = drawable_guard.expiration_date();
                    if expiration != 0 && frames.logic_frame >= expiration {
                        expired_logic_drawables.push(drawable_guard.get_drawable_id());
                    }
                }
            }

            let _ = (object_id, is_effectively_dead);
        })?;
        if let Some(client) = TheGameClient::get() {
            for id in expired_logic_drawables {
                client.destroy_drawable(id);
            }
        }
        Ok(())
    }

    fn update_effects(&mut self, delta_time: f32) -> GameClientResult<()> {
        self.update_scene_lights(delta_time);
        if delta_time > 0.0 {
            crate::display::view::with_tactical_view(|view| view.tick_impulse_shake());
        }
        self.update_effect_decals(delta_time)
    }

    // Main advances its receiving SessionView impulse at the admitted view phase.
    // Native scene effects must not advance an unrelated standalone Core View.
    fn update_native_effects(&mut self, delta_time: f32) -> GameClientResult<()> {
        self.update_scene_lights(delta_time);
        self.update_effect_decals(delta_time)
    }

    fn update_scene_lights(&mut self, delta_time: f32) {
        if delta_time > 0.0 {
            self.drawables.light_scene.update();
        }
    }

    fn update_effect_decals(&mut self, delta_time: f32) -> GameClientResult<()> {
        if let Some(ref decals) = self.subsystem_manager.decal_manager {
            if let Ok(mut guard) = decals.lock() {
                let config = EffectsConfig::default();
                guard.update(delta_time, &config);
            }
        }
        Ok(())
    }

    fn should_freeze_visual_time(&mut self) -> bool {
        // Full standalone update samples speed later at its original point.
        self.consume_visual_frame(legacy_visual_time_frozen())
    }

    fn consume_visual_frame(&mut self, frozen: bool) -> bool {
        let frozen = frozen || self.last_visual_time_frame == self.frame;
        self.last_visual_time_frame = self.frame;
        frozen
    }

    fn admit_visual_time(
        &mut self,
        observation: NativeVisualTime,
    ) -> GameClientResult<(bool, f32)> {
        if !observation.visual_speed.is_finite() {
            return Err(GameClientError::InvalidOperation(
                "nonfinite native visual speed".into(),
            ));
        }
        Ok((
            self.consume_visual_frame(observation.frozen),
            observation.visual_speed,
        ))
    }

    #[inline]
    fn should_skip_visual_updates_for_no_draw(&self) -> bool {
        #[cfg(any(debug_assertions, feature = "internal"))]
        {
            let logic_frame = TheGameLogic::get_frame();
            if logic_frame == 0 {
                return false;
            }
            get_global_data()
                .map(|global| global.read().no_draw > logic_frame)
                .unwrap_or(false)
        }

        #[cfg(not(any(debug_assertions, feature = "internal")))]
        {
            false
        }
    }

    fn preload_template_assets_from_factory(
        &mut self,
        time_of_day: TimeOfDay,
    ) -> GameClientResult<()> {
        let preload_everything = get_global_data()
            .map(|global| global.read().preload_everything)
            .unwrap_or(false);

        let Ok(thing_factory_guard) = get_thing_factory() else {
            return Ok(());
        };
        let Some(thing_factory) = thing_factory_guard.as_ref() else {
            return Ok(());
        };

        let mut templates_to_preload: Vec<Arc<ThingTemplate>> = Vec::new();

        let mut current = thing_factory.first_template().cloned();
        while let Some(template) = current {
            if Self::should_preload_template(template.as_ref(), preload_everything) {
                templates_to_preload.push(template.clone());
            }

            let mut override_template = template.get_next_override();
            while let Some(override_entry) = override_template {
                if Self::should_preload_template(override_entry.as_ref(), preload_everything) {
                    templates_to_preload.push(override_entry.clone());
                }
                override_template = override_entry.get_next_override();
            }

            current = template.get_next_template().clone();
        }

        drop(thing_factory_guard);

        for template in templates_to_preload {
            self.preload_template_assets(template.as_ref(), time_of_day);
        }

        Ok(())
    }

    fn should_preload_template(template: &ThingTemplate, preload_everything: bool) -> bool {
        // C++ parity: GameClient.cpp::preloadAssets checks KINDOF_PRELOAD unless preloadEverything is forced.
        const KINDOF_PRELOAD: u64 = 26;
        preload_everything || template.is_kind_of(KINDOF_PRELOAD)
    }

    fn preload_template_assets(&mut self, template: &ThingTemplate, time_of_day: TimeOfDay) {
        // C++ parity: create temp drawable from template, preload, then destroy.
        let temp_id = match self.create_drawable_from_template(template) {
            Ok(id) => id,
            Err(err) => {
                log::warn!(
                    "Failed to create temporary preload drawable for template '{}': {}",
                    template.get_name(),
                    err
                );
                return;
            }
        };

        if let Some(drawable) = self.find_drawable_by_id(temp_id) {
            if let Err(err) = drawable.preload_assets(time_of_day) {
                log::warn!(
                    "Failed to preload assets for template '{}': {}",
                    template.get_name(),
                    err
                );
            }
        }

        if let Err(err) = self.destroy_drawable(temp_id) {
            log::warn!(
                "Failed to destroy temporary preload drawable for template '{}': {}",
                template.get_name(),
                err
            );
        }
    }

    fn update_display_only(&mut self) -> GameClientResult<()> {
        if let Some(ref display) = self.subsystem_manager.display {
            display
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .update_receiving(self.script_display.fx_mut())?;
        }
        Ok(())
    }

    fn update_startup_movie_display(&mut self) -> GameClientResult<()> {
        if let Some(ref display) = self.subsystem_manager.display {
            let mut display = display.lock().unwrap_or_else(|e| e.into_inner());
            display.draw_owned(
                self.script_display.fx_mut(),
                self.ui_renderer.as_mut(),
                Some(&self.window_manager_owner),
                self.subsystem_manager.in_game_ui.as_ref(),
                self.frame,
                &mut self.drawable_manager,
                &mut self.render_bridge,
            )?;
            display.update_receiving(self.script_display.fx_mut())?;
        }
        Ok(())
    }

    fn startup_movies_active(&self) -> bool {
        get_global_data()
            .map(|data| {
                let data = data.read();
                data.play_intro || data.after_intro
            })
            .unwrap_or(false)
    }

    fn should_activate_shell_after_startup(&self) -> bool {
        self.menu_host.shell_map_settings().initial_file.is_empty()
    }

    fn activate_shell_after_startup(&mut self) -> GameClientResult<()> {
        if !self.should_activate_shell_after_startup() {
            return Ok(());
        }
        log::info!("Activating receiving Shell after startup movie flow");
        self.with_receiving_shell(None, |shell, effects| {
            shell.show_shell_map(true, effects);
            shell.show_shell(true, effects)
        })
        .ok_or_else(|| GameClientError::SubsystemError("startup Shell is already borrowed".into()))?
        .map_err(|error| {
            GameClientError::SubsystemError(format!("Failed to activate startup Shell: {error}"))
        })
    }

    fn show_low_memory_legal_page(
        &mut self,
        display: &mut GraphicsDisplay,
    ) -> GameClientResult<()> {
        let Some((layout, _info)) = self.with_window_event_effects(None, |effects| {
            effects.with_manager(|manager, effects| {
                manager
                    .create_layout_with_windows("Menus/LegalPage.wnd", effects)
                    .ok()
            })
        }) else {
            return Ok(());
        };

        self.window_manager_owner.with_manager(|manager| {
            let mut layout_mut = layout.borrow_mut();
            layout_mut.hide(manager, false);
            layout_mut.bring_forward(manager);
        });

        let begin = Instant::now();
        while begin.elapsed() < Duration::from_millis(4000) {
            self.with_window_event_effects(None, |effects| {
                effects.with_manager(|manager, effects| manager.update(effects))
            });
            display.draw_owned(
                self.script_display.fx_mut(),
                self.ui_renderer.as_mut(),
                Some(&self.window_manager_owner),
                self.subsystem_manager.in_game_ui.as_ref(),
                self.frame,
                &mut self.drawable_manager,
                &mut self.render_bridge,
            )?;
            thread::sleep(Duration::from_millis(100));
        }

        self.with_window_event_effects(None, |effects| {
            effects.with_manager(|manager, effects| manager.destroy_layout(&layout, effects))
        });
        Ok(())
    }

    fn finish_unavailable_startup_movies(&mut self) -> GameClientResult<()> {
        let Some(global_data) = get_global_data() else {
            return Ok(());
        };

        {
            let mut global = global_data.write();
            global.break_the_movie = true;
            global.allow_exit_out_of_movies = true;
            global.after_intro = false;
            global.play_intro = false;
            self.startup_sizzle_pending = false;
        }

        self.activate_shell_after_startup()
    }

    fn update_startup_movies(&mut self) -> GameClientResult<()> {
        let Some(global_data) = get_global_data() else {
            return Ok(());
        };
        let Some(display_arc) = self.subsystem_manager.display.as_ref().cloned() else {
            return self.finish_unavailable_startup_movies();
        };

        let mut display = display_arc
            .lock()
            .map_err(|_| GameClientError::SubsystemError("Display lock poisoned".to_string()))?;
        if display.is_movie_playing() {
            return Ok(());
        }

        let mut global = global_data.write();
        let low_res_movies = prefers_low_res_movies();
        let Some(action) = startup_movie_action(
            global.play_intro,
            global.after_intro,
            global.play_sizzle,
            self.startup_sizzle_pending,
            low_res_movies,
        ) else {
            return Ok(());
        };

        match action {
            StartupMovieAction::PlayLogo(movie_name) => {
                display.play_logo_movie(
                    self.script_display.fx_mut(),
                    movie_name.to_string(),
                    5000,
                    3000,
                );
                global.play_intro = false;
                global.after_intro = true;
                self.startup_sizzle_pending = true;
            }
            StartupMovieAction::PlaySizzle(movie_name) => {
                global.allow_exit_out_of_movies = true;
                if display.play_movie(self.script_display.fx_mut(), movie_name.to_string()) {
                    self.startup_sizzle_pending = false;
                    return Ok(());
                }
                self.startup_sizzle_pending = false;
                global.break_the_movie = true;
                global.after_intro = false;
                drop(global);
                drop(display);
                self.activate_shell_after_startup()?;
            }
            StartupMovieAction::FinalizeStartup => {
                global.break_the_movie = true;
                global.allow_exit_out_of_movies = true;
                global.after_intro = false;
                drop(global);
                if low_res_movies {
                    self.show_low_memory_legal_page(&mut display)?;
                }
                drop(display);
                self.activate_shell_after_startup()?;
            }
        }
        Ok(())
    }

    pub fn ensure_shell_visible(
        &mut self,
        menu_world: Option<crate::gui::campaign_launch_host_bridge::MenuWorldFacts>,
    ) -> GameClientResult<()> {
        if !self.should_activate_shell_after_startup() {
            return Ok(());
        }
        self.with_receiving_shell(menu_world, |shell, effects| {
            let needs_screen = shell.get_screen_count() == 0 && !shell.is_shell_map_on();
            if needs_screen || !shell.is_shell_active() {
                shell.show_shell_map(true, effects);
                shell.show_shell(true, effects)?;
            }
            Ok::<(), crate::gui::shell::base::ShellError>(())
        })
        .ok_or_else(|| {
            GameClientError::SubsystemError("receiving Shell is already borrowed".into())
        })?
        .map_err(|error| {
            GameClientError::SubsystemError(format!(
                "Failed to ensure receiving Shell visibility: {error}"
            ))
        })
    }

    pub fn update_pre_draw_ui(&mut self) -> GameClientResult<()> {
        if self.subsystem_manager.window_manager_initialized {
            // C++ WindowManager::update retains its existing pre-video phase,
            // with the same client's actual callback recipients.
            self.with_window_event_effects(None, |effects| {
                effects.with_manager(|manager, effects| manager.update(effects))
            });
        }

        if let Some(ref video_player) = self.subsystem_manager.video_player {
            video_player
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .update()?;
        }

        Ok(())
    }

    pub fn update_post_draw_ui(
        &mut self,
        menu_world: Option<crate::gui::campaign_launch_host_bridge::MenuWorldFacts>,
    ) -> GameClientResult<()> {
        {
            let mut shell = self.window_manager_owner.shell_handle();
            let mut messages = Vec::new();
            let mut effects = crate::gui::game_window::WindowEventEffects {
                cursor_mouse: self.subsystem_manager.input_mouse.as_mut(),
                control_bar_observation: None,
                menu_audio: None,
                input_provenance:
                    crate::gui::control_bar::HostControlBarInputProvenance::InjectedOrUnknown,
                standalone_view: None,
                client_frame: Some(self.frame),
                logic_frame: menu_world.map(|facts| facts.logic_frame),
                ui: self.subsystem_manager.in_game_ui.as_mut(),
                payloads: Default::default(),
                pending_window_callbacks: Default::default(),
                diplomacy_world: None,
                menus: Some(self.menu_manager.receiving_menus(self.input_authority)),
                ime: Some(&mut self.ime),
                manager: None,
                script_display: None,
                window_videos: Some(&mut self.window_videos.challenge),
                window_manager_owner: Some(&self.window_manager_owner),
                selected_window: None,
                emitted_messages: &mut messages,
                pending_control_bar_clicks: None,
                campaign_manager: Some(&mut self.campaign_manager),
                main_menu: Some(&mut self.main_menu),
                menu_world,
                menu_host: Some(&mut self.menu_host),
                pending_main_menu_actions: Vec::new(),
            };
            shell.update(&mut effects).map_err(|err| {
                GameClientError::SubsystemError(format!("Shell update failed: {err}"))
            })?;
            for message in messages {
                self.message_stream.append_message_value(message);
            }
        }

        if let Some(ui) = self.subsystem_manager.in_game_ui.as_mut() {
            self.window_videos.in_game.update_in_game_movies();
            self.window_manager_owner.with_manager(|manager| {
                ui.update_receiving(manager, self.menu_manager.diplomacy_mut())
            })?;
        }

        if self.input_authority == ClientInputAuthority::StandaloneCore {
            crate::eva::update_eva_system();
        }

        Ok(())
    }

    pub fn update_display_string_manager(&self) -> GameClientResult<()> {
        crate::display_string_manager::update_display_string_manager()
            .map_err(|err| GameClientError::SubsystemError(format!("{err}")))
    }

    fn process_beacon_notifications(&mut self, logic_frame: u32) -> GameClientResult<()> {
        // Native Main projects its own world beacon records in camera_drain.
        // Its UI must never drain another standalone Core session's queue.
        if self.input_authority == ClientInputAuthority::NativeHosted {
            return Ok(());
        }
        let notifications = beacon_display::drain_notifications();
        if notifications.is_empty() {
            return Ok(());
        }

        for notification in notifications {
            if let Some(ui) = self.subsystem_manager.in_game_ui.as_mut() {
                let ui_guard = ui;
                ui_guard
                    .handle_beacon_notification(&notification, logic_frame)
                    .map_err(|err| {
                        GameClientError::SubsystemError(format!(
                            "Failed to handle beacon notification: {err}"
                        ))
                    })?;
            } else {
                log::info!("Beacon event: {:?}", notification);
            }
        }

        Ok(())
    }

    fn set_frame_rate(&mut self, duration_per_frame: Duration) -> GameClientResult<()> {
        if duration_per_frame.is_zero() {
            return Err(GameClientError::InvalidOperation(
                "frame duration must be greater than zero".to_string(),
            ));
        }

        self.target_frame_duration = duration_per_frame;
        log::info!(
            "Target frame duration set to {:?} (~{:.2} FPS)",
            duration_per_frame,
            1.0 / duration_per_frame.as_secs_f64()
        );
        Ok(())
    }
}

/// Explicit compatibility observation for standalone Core entrypoints only.
fn legacy_visual_time_frozen() -> bool {
    let camera_frozen =
        with_tactical_view_ref(|view| view.is_time_frozen() && !view.is_camera_movement_finished());
    camera_frozen
        || TheScriptEngine::is_time_frozen_debug()
        || TheScriptEngine::is_time_frozen_script()
        || TheGameLogic::is_game_paused()
}

fn legacy_visual_time_observation() -> NativeVisualTime {
    NativeVisualTime {
        frozen: legacy_visual_time_frozen(),
        visual_speed: get_script_visual_speed_multiplier() as f32,
    }
}

#[cfg(test)]
#[path = "native_visual_time_tests.rs"]
mod native_visual_time_tests;
