// GameClient update loop, input/audio/drawable ticks, and post-draw UI.
// Split from `core/game_client.rs` dump. Included by `game_client/mod.rs`
// so this stays one logical `game_client` module (public API identical).

impl GameClient {
    /// Updates the game client - main game loop entry point.
    ///
    /// C++ parity: frame sequence matches `GameClient::update()` (GameClient.cpp:489-752).
    pub fn update(&mut self) -> GameClientResult<()> {
        if !self.initialized {
            return Err(GameClientError::InvalidOperation(
                "GameClient not initialized".to_string(),
            ));
        }

        let current_time = Instant::now();
        self.last_update_time = current_time;

        self.frame = self.frame.wrapping_add(1);
        publish_live_game_client_frame(self);

        self.create_frame_tick_message()?;
        self.update_startup_movies()?;
        if self.startup_movies_active() {
            self.update_startup_movie_display()?;
            self.rendered_object_count = 0;
            self.finish_frame_timing(current_time);
            return Ok(());
        }
        self.ensure_shell_visible()?;
        // C++ GameClient.cpp:560-565 — snow + Anim2D before input.
        self.update_cpp_snow_and_anim2d(SECONDS_PER_LOGICFRAME_REAL);
        // C++ GameClient.cpp:587-597 — camera follows first selected drawable.
        self.update_camera_tracking_drawable();

        // C++ lines 612-619: window manager and video player update BEFORE drawables
        self.update_pre_draw_ui()?;

        let freeze_time = self.should_freeze_visual_time();
        let mut visual_delta = if freeze_time {
            0.0
        } else {
            SECONDS_PER_LOGICFRAME_REAL
        };
        let visual_speed = get_script_visual_speed_multiplier();
        visual_delta = if visual_speed <= 0 {
            0.0
        } else {
            visual_delta * visual_speed as f32
        };

        // Host/presentation residual: Main owns OS WindowEvent→commands and sole
        // RenderPipeline 3D present. When OBJECT_REGISTRY is empty, skip dual-world
        // shroud bind and dual Display DRAW present.
        let host_presentation_path = OBJECT_REGISTRY.is_empty();
        self.update_orphaned_w3d_ghosts_if_unfrozen(freeze_time);
        if host_presentation_path {
            // Shared THE_MOUSE/THE_KEYBOARD may still be polled for shell widgets;
            // Main remains command authority. Prefer local drawable modules only.
            self.update_drawables_local(visual_delta)?;
            // Wave 1022: catalog shroud residual on presentation shell tick path.
            self.update_drawable_visibility(self.local_player_id)?;
            if self.should_skip_visual_updates_for_no_draw() {
                self.rendered_object_count = 0;
                self.finish_frame_timing(current_time);
                return Ok(());
            }
            self.update_particle_system_local_player()?;
            self.update_effects(visual_delta)?;
            apply_pending_script_display_state();
            self.update_display_only()?;
            // No draw_display — Main RenderPipeline is sole present path.
            self.draw_drawable_icon_ui();
            self.draw_presentation_selection_residual();
            let _ = self.draw_live_ingame_hud();

            self.update_display_string_manager()?;
            self.update_post_draw_ui()?;
            self.process_beacon_notifications()?;
            self.pump_message_stream()?;
            self.rendered_object_count = 0;
            self.finish_frame_timing(current_time);
            return Ok(());
        }

        // Dual-world residual: full C++-ordered client tick with registry bind.
        // C++ lines 560-584: keyboard, mouse, Anim2D, Eva
        self.update_input()?;
        self.update_audio()?;

        // C++ lines 660-700: shroud check per-drawable then updateDrawable()
        self.update_drawables(visual_delta)?;
        if self.should_skip_visual_updates_for_no_draw() {
            self.rendered_object_count = 0;
            self.finish_frame_timing(current_time);
            return Ok(());
        }

        self.set_particle_system_local_player()?;

        // C++ line 721: terrain visual, C++ line 726: display UPDATE
        self.update_effects(visual_delta)?;
        apply_pending_script_display_state();
        self.update_display_only()?;

        // W3DDisplay.cpp:1730-1835 — freeze/sync/updateViews/particles before DRAW GPU.
        crate::display::client_draw_schedule::run_dual_world_cpu_phases();

        // C++ line 735: TheDisplay->DRAW()
        self.draw_display()?;

        // C++ W3DView::drawablePostDraw(): draw per-drawable icon UI after the
        // 3D drawable pass and before post-draw shell/InGameUI updates.
        self.draw_drawable_icon_ui();
        self.draw_presentation_selection_residual();
        let _ = self.draw_live_ingame_hud();


        // C++ line 740: DisplayStringManager update
        self.update_display_string_manager()?;

        // C++ lines 744-751: Shell and InGameUI AFTER draw
        self.update_post_draw_ui()?;

        self.process_beacon_notifications()?;
        self.pump_message_stream()?;

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

    pub fn update_input(&mut self) -> GameClientResult<()> {
        if let Some(ref keyboard) = self.subsystem_manager.input_keyboard {
            keyboard.lock().unwrap_or_else(|e| e.into_inner()).update();
        }

        if let Some(ref mouse) = self.subsystem_manager.input_mouse {
            mouse.lock().unwrap_or_else(|e| e.into_inner()).update();
        }

        Ok(())
    }

    /// C++ `TheSnowManager->UPDATE()` + `TheAnim2DCollection->UPDATE()`.
    pub fn update_cpp_snow_and_anim2d(&self, delta_seconds: f32) {
        if let Some(snow) = crate::snow::get_snow_manager() {
            if let Ok(mut guard) = snow.lock() {
                guard.update(delta_seconds);
            }
        }
        crate::system::update_client_anim2d_collection();
    }

    /// C++ `TheTerrainVisual->UPDATE()` (GameClient.cpp:719-722). Fail-soft.
    pub fn update_terrain_visual(&self) {
        if let Some(tv) = &self.subsystem_manager.terrain_visual {
            let _ = tv.lock().unwrap_or_else(|e| e.into_inner()).update();
        }
    }

    /// C++ GameClient.cpp:587-597 — follow first selected drawable or clear flag.
    fn update_camera_tracking_drawable(&self) {
        if !crate::helpers::TheInGameUI::is_camera_tracking_drawable() {
            return;
        }
        use crate::drawable::DrawableExt;
        // C++ `TheInGameUI->getFirstSelectedDrawable()` is the first entry of
        // the selection list. HashMap iteration is unordered, so pick the
        // lowest DrawableId among selected units for a stable follow target.
        let mut first: Option<(crate::drawable::DrawableId, crate::drawable::Coord3D)> = None;
        for (id, drawable) in &self.drawable_map {
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
            crate::helpers::TheInGameUI::set_camera_tracking_drawable(false);
        }
    }

    fn update_audio(&mut self) -> GameClientResult<()> {
        if let Some(ref audio) = self.subsystem_manager.audio {
            audio.lock().unwrap_or_else(|e| e.into_inner()).update()?;
        }

        if let (Some(queue), Some(engine)) =
            (&mut self.audio_event_queue, &mut self.audio_engine)
        {
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

        if let (Some(music), Some(engine)) =
            (&mut self.music_system, &mut self.audio_engine)
        {
            music.update(engine);
        }

        if let (Some(speech), Some(engine)) =
            (&mut self.speech_system, &mut self.audio_engine)
        {
            speech.update(engine);
        }

        Ok(())
    }

    /// Tick drawable client modules without GameLogic OBJECT_REGISTRY binding.
    /// Used when Main presentation snapshot owns unit visuals (default host path).
    pub fn update_drawables_local(&mut self, delta_time: f32) -> GameClientResult<()> {
        for drawable in self.drawable_map.values_mut() {
            drawable.update(delta_time);
        }
        Ok(())
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
            let Some(drawable_id) = self.drawable_object_map.get(&object_id).copied() else {
                continue;
            };
            let Some(drawable) = self.drawable_map.get_mut(&drawable_id) else {
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
            let Some(drawable) = self.drawable_map.get_mut(&drawable_id) else {
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
        let state =
            self.presentation_direct_drawable_state(binding_key.host_epoch, binding_key.object_id)?;
        if state.binding_key != binding_key {
            return None;
        }
        let drawable = self.drawable_map.get_mut(&binding_key.drawable_id)?;
        if drawable.get_object_id() != Some(binding_key.object_id) {
            return None;
        }
        drawable.evaluate_frozen_direct_scene_candidate(logic_frame, raw_status, effectively_dead)
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
        candidates
            .into_iter()
            .filter_map(|candidate| {
                let decision = self.evaluate_frozen_direct_scene_candidate(
                    logic_frame,
                    candidate.binding_key,
                    candidate.raw_status,
                    candidate.effectively_dead,
                )?;
                Some(FrozenDirectSceneShroudDecision {
                    binding_key: candidate.binding_key,
                    decision,
                })
            })
            .collect()
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
        // Epoch zero belongs only to the legacy convenience ensure helper.
        // Main's direct host pipeline starts at one and is the only route that
        // may expose a renderable direct binding key.
        if host_epoch == 0 {
            return None;
        }
        let drawable_id = self.drawable_object_map.get(&object_id)?;
        let binding = self
            .presentation_direct_drawable_bindings
            .get(drawable_id)?;
        let binding_key = binding.binding_key;
        if binding_key.host_epoch != host_epoch
            || binding_key.object_id != object_id
            || binding_key.drawable_id != *drawable_id
        {
            return None;
        }
        let drawable = self.drawable_map.get(drawable_id)?;
        if drawable.get_object_id() != Some(object_id) {
            return None;
        }
        let scene_effectively_hidden = drawable.scene_effectively_hidden()?;
        let fully_obscured = drawable.fully_obscured_by_shroud()?;
        Some(PresentationDirectDrawableState {
            binding_key,
            scene_effectively_hidden,
            fully_obscured,
        })
    }

    /// Compatibility reader for callers that do not yet retain a host epoch.
    ///
    /// New presentation code should use
    /// [`Self::presentation_direct_drawable_state`] and retain its full key.
    #[must_use]
    pub fn presentation_direct_fully_obscured(&self, object_id: ObjectID) -> Option<bool> {
        let drawable_id = self.drawable_object_map.get(&object_id)?;
        let host_epoch = self
            .presentation_direct_drawable_bindings
            .get(drawable_id)?
            .binding_key
            .host_epoch;
        self.presentation_direct_drawable_state(host_epoch, object_id)
            .map(|state| state.fully_obscured)
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
            let Some(drawable) = self.drawable_map.get_mut(&state.binding_key.drawable_id) else {
                continue;
            };
            if drawable.get_object_id() != Some(entry.binding_key.object_id) {
                continue;
            }
            let position = Vector3::new(entry.position[0], entry.position[1], entry.position[2]);
            drawable.set_position(position);
            drawable.set_instance_transform(Matrix4::rotation_y(entry.orientation));
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
            let Some(drawable_id) = self.drawable_object_map.get(&object_id).copied() else {
                continue;
            };
            let Some(drawable) = self.drawable_map.get_mut(&drawable_id) else {
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
            drawable.set_instance_transform(transform);
            updated += 1;
        }
        updated
    }

    /// Wave 962/963: host presentation drawable ensure (no OBJECT_REGISTRY dual-world).
    ///
    /// Creates missing drawables bound to presentation object ids so pose/shroud
    /// residuals apply without populating the dual-world registry.
    pub fn ensure_presentation_drawables<I>(&mut self, entries: I) -> usize
    where
        I: IntoIterator<Item = (u32, String, [f32; 3], f32)>,
    {
        let sync = entries
            .into_iter()
            .map(|(id, tmpl, pos, ori)| PresentationDrawableSync {
                object_id: id,
                host_epoch: 0,
                resident: true,
                visual_template_name: tmpl.clone(),
                template_name: tmpl,
                position: pos,
                orientation: ori,
                destroyed: false,
                model_condition_bits: 0,
                body_damage_state: 0,
                kind_names: Vec::new(),
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
                weapon_bonus_enthusiastic: false,
                show_healing: false,
                healing_icon_type: 0,
                garrisoned_ids: Vec::new(),
                emoticon_name: String::new(),
                emoticon_frames_left: 0,
                formation_id: 0,
                caption: String::new(),
            });
        self.sync_presentation_drawables(sync).0
    }

    /// Wave 963: presentation drawable sync residual (ensure + pose/model + prune).
    ///
    /// Direct visual residency is controlled solely by
    /// [`PresentationDrawableSync::resident`].  In particular, `destroyed`
    /// is diagnostic/render data and must not remove an active slow-death or
    /// rubble visual.  Returns `(created, updated, pruned)`. No
    /// OBJECT_REGISTRY dual-world populate.
    pub fn sync_presentation_drawables<I>(&mut self, entries: I) -> (usize, usize, usize)
    where
        I: IntoIterator<Item = PresentationDrawableSync>,
    {
        use crate::drawable::DrawableExt;

        let mut created = 0usize;
        let mut updated = 0usize;
        let mut live_bindings = std::collections::HashSet::new();

        for e in entries {
            if !e.resident {
                continue;
            }
            let visual_template_name = Self::presentation_visual_template_name(&e).to_string();
            live_bindings.insert((e.host_epoch, e.object_id));

            if let Some(&drawable_id) = self.drawable_object_map.get(&e.object_id) {
                let retains_binding = self
                    .presentation_direct_drawable_bindings
                    .get(&drawable_id)
                    .map(|binding| {
                        binding.binding_key.host_epoch == e.host_epoch
                            && binding.binding_key.object_id == e.object_id
                            && binding.binding_key.drawable_id == drawable_id
                            && binding.visual_template_name == visual_template_name
                            && self.drawable_map.get(&drawable_id).is_some_and(|drawable| {
                                drawable.get_object_id() == Some(e.object_id)
                            })
                    })
                    .unwrap_or(false);

                if retains_binding {
                    let Some(drawable) = self.drawable_map.get_mut(&drawable_id) else {
                        continue;
                    };
                    let position = Vector3::new(e.position[0], e.position[1], e.position[2]);
                    drawable.set_position(position);
                    let transform = Matrix4::rotation_y(e.orientation);
                    drawable.set_instance_transform(transform);
                    if let Some(basic) = drawable.downcast_mut::<BasicDrawable>() {
                        if !visual_template_name.is_empty() {
                            basic.set_template_name(Some(visual_template_name.clone()));
                        }
                        Self::stamp_presentation_object_residual(basic, &e);
                    }
                    updated = updated.saturating_add(1);
                    continue;
                }

                // A mapped Drawable with a different host epoch, visual
                // template, or missing direct record cannot retain volatile
                // direct state.  Recreate it instead of silently rebinding.
                let _ = self.destroy_drawable(drawable_id);
                if self.drawable_object_map.get(&e.object_id).copied() == Some(drawable_id) {
                    self.drawable_object_map.remove(&e.object_id);
                }
                self.presentation_direct_drawable_bindings
                    .remove(&drawable_id);
            }

            let mut drawable = BasicDrawable::new(DrawableId::INVALID);
            if !visual_template_name.is_empty() {
                drawable.set_template_name(Some(visual_template_name.clone()));
            }
            drawable.set_object_id(Some(e.object_id));
            let position = Vector3::new(e.position[0], e.position[1], e.position[2]);
            drawable.set_position(position);
            let transform = Matrix4::rotation_y(e.orientation);
            drawable.set_instance_transform(transform);
            Self::stamp_presentation_object_residual(&mut drawable, &e);
            let id = self.alloc_drawable_id();
            drawable.set_id(id);
            self.drawable_map.insert(id, Box::new(drawable));
            self.drawable_object_map.insert(e.object_id, id);
            let binding_generation = self.alloc_presentation_direct_binding_generation();
            let binding_key = PresentationDirectDrawableBindingKey {
                host_epoch: e.host_epoch,
                object_id: e.object_id,
                drawable_id: id,
                binding_generation,
            };
            self.presentation_direct_drawable_bindings.insert(
                id,
                PresentationDirectDrawableBinding {
                    binding_key,
                    visual_template_name,
                },
            );
            created = created.saturating_add(1);
        }

        let stale: Vec<(DrawableId, PresentationDirectDrawableBindingKey)> = self
            .presentation_direct_drawable_bindings
            .iter()
            .filter_map(|(&drawable_id, binding)| {
                let binding_key = binding.binding_key;
                if live_bindings.contains(&(binding_key.host_epoch, binding_key.object_id)) {
                    None
                } else {
                    Some((drawable_id, binding_key))
                }
            })
            .collect();
        let mut pruned = 0usize;
        for (drawable_id, binding_key) in stale {
            let _ = self.destroy_drawable(drawable_id);
            if self
                .drawable_object_map
                .get(&binding_key.object_id)
                .copied()
                == Some(drawable_id)
            {
                self.drawable_object_map.remove(&binding_key.object_id);
            }
            self.presentation_direct_drawable_bindings
                .remove(&drawable_id);
            pruned = pruned.saturating_add(1);
        }

        (created, updated, pruned)
    }

    fn presentation_visual_template_name(e: &PresentationDrawableSync) -> &str {
        if e.visual_template_name.is_empty() {
            &e.template_name
        } else {
            &e.visual_template_name
        }
    }

    fn alloc_presentation_direct_binding_generation(&mut self) -> u64 {
        let generation = self.next_presentation_direct_binding_generation.max(1);
        self.next_presentation_direct_binding_generation = generation.wrapping_add(1).max(1);
        generation
    }

    fn stamp_presentation_object_residual(
        drawable: &mut BasicDrawable,
        e: &PresentationDrawableSync,
    ) {
        use game_engine::common::bit_flags::{create_model_condition_flags, ModelConditionFlags};
        use gamelogic::common::types::BodyDamageType;

        let bit_names = ModelConditionFlags::BIT_NAMES;
        let mut set = create_model_condition_flags();
        let mut clear_all = create_model_condition_flags();
        let n = bit_names.len().min(128);
        for i in 0..n {
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
            e.kind_names.clone(),
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
        // Wave 1115: sold residual after host overlay stamp (C++ OBJECT_STATUS_SOLD).
        drawable.set_presentation_sold(e.sold);
    }

    /// Apply presentation cinematic letterbox residual to GraphicsDisplay.
    ///
    /// C++ script camera letterbox residual without dual-owning Main RenderPipeline 3D draw.
    pub fn apply_presentation_cinematic_letterbox(&mut self, enabled: bool) {
        if self.letterbox_overlay_enabled != enabled {
            self.letterbox_overlay_enabled = enabled;
            self.letterbox_overlay_fade_start = Some(Instant::now());
        }
        if let Some(ref display) = self.subsystem_manager.display {
            let mut display = display.lock().unwrap_or_else(|e| e.into_inner());
            if display.is_letter_box_enabled() != enabled {
                display.enable_letter_box(enabled);
            }
        }
    }

    /// C++ `W3DDisplay::renderLetterBox` fade (LETTER_BOX_FADE_TIME = 1000 ms).
    pub fn letterbox_overlay_fade(&self) -> f32 {
        const LETTER_BOX_FADE_TIME_MS: f32 = 1000.0;
        let Some(start) = self.letterbox_overlay_fade_start else {
            return if self.letterbox_overlay_enabled {
                1.0
            } else {
                0.0
            };
        };
        let t = (start.elapsed().as_millis() as f32 / LETTER_BOX_FADE_TIME_MS).clamp(0.0, 1.0);
        if self.letterbox_overlay_enabled {
            t
        } else {
            1.0 - t
        }
    }

    /// Whether letterbox bars should be drawn (enabled or still fading out).
    pub fn letterbox_overlay_visible(&self) -> bool {
        self.letterbox_overlay_fade() > 0.0
    }

    /// Apply presentation military caption residual to InGameUI subsystem.
    ///
    /// C++ military subtitle residual; duration from presentation freeze.
    pub fn apply_presentation_military_caption(
        &mut self,
        caption: Option<&str>,
        remaining_ms: Option<i32>,
    ) {
        let text = caption.filter(|t| !t.is_empty());
        // Only push when caption text changes (presentation freezes each frame).
        if self.last_applied_military_caption.as_deref() == text {
            return;
        }
        self.last_applied_military_caption = text.map(|t| t.to_string());
        let Some(text) = text else {
            return;
        };
        let Some(ref ui) = self.subsystem_manager.in_game_ui else {
            return;
        };
        let Ok(mut ui) = ui.lock() else {
            return;
        };
        let ms = remaining_ms.unwrap_or(10_000).max(0);
        ui.push_military_subtitle(text, ms);
    }

    /// Wave 1060: presentation floating cash/text residual → InGameUI subsystem residual.
    pub fn apply_presentation_floating_texts(
        &mut self,
        entries: &[(String, [f32; 3], (u8, u8, u8), u32, u32)],
    ) {
        let Some(ref ui) = self.subsystem_manager.in_game_ui else {
            return;
        };
        let Ok(mut ui) = ui.lock() else {
            return;
        };
        ui.replace_floating_texts_from_presentation(entries);
    }

    /// Presentation PublicTimer residual → InGameUI postDraw countdown strip.
    pub fn apply_presentation_superweapon_timers(
        &mut self,
        timers: &[(String, String, bool)],
    ) {
        let Some(ui) = &self.subsystem_manager.in_game_ui else {
            return;
        };
        let Ok(mut ui) = ui.lock() else {
            return;
        };
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
        if self.last_applied_cinematic_text.as_deref() == text {
            return;
        }
        self.last_applied_cinematic_text = text.map(|t| t.to_string());
        self.cinematic_overlay_font = font
            .filter(|f| !f.is_empty())
            .map(|f| f.to_string());
        let Some(_) = text else {
            self.cinematic_overlay_frames = 0;
            return;
        };
        // C++ frames = LOGICFRAMES_PER_SECOND * time; remaining_ms is the
        // live residual of that countdown.
        const LOGICFRAMES_PER_SECOND: u32 = 30;
        self.cinematic_overlay_frames = remaining_ms
            .map(|ms| ((ms.max(0) as u32) * LOGICFRAMES_PER_SECOND + 999) / 1000)
            .filter(|frames| *frames > 0)
            .unwrap_or(LOGICFRAMES_PER_SECOND * 10);
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
                self.cinematic_overlay_font = None;
            }
        }
    }

    /// Wave 964: presentation selection residual → InGameUI (host empty dual-world).
    pub fn apply_presentation_selection_residual(
        &mut self,
        units: Vec<crate::gui::ingame_ui::PresentationSelectedUnitResidual>,
    ) {
        if let Some(ref ui) = self.subsystem_manager.in_game_ui {
            if let Ok(mut guard) = ui.lock() {
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
                },
            )
            .collect();
        let local_team = if let Some(ref ui) = self.subsystem_manager.in_game_ui {
            ui.lock()
                .ok()
                .map(|g| g.presentation_local_team_name().to_string())
                .unwrap_or_default()
        } else {
            String::new()
        };
        crate::presentation_translator_residual::set_translator_presentation_residual(
            local_team,
            translator_catalog,
        );

        // Wave 1055: stamp control-group residual onto drawable presentation shell.
        for u in &units {
            if let Some(did) = self.drawable_object_map.get(&u.object_id).copied() {
                if let Some(drawable) = self.drawable_map.get_mut(&did) {
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

        if let Some(ref ui) = self.subsystem_manager.in_game_ui {
            if let Ok(mut guard) = ui.lock() {
                guard.set_presentation_unit_catalog(units);
            }
        }
    }

    /// Wave 968: stamp local player team residual for host ownership queries.
    pub fn apply_presentation_local_team_name(&mut self, team_name: impl Into<String>) {
        let team_name = team_name.into();
        if let Some(ref ui) = self.subsystem_manager.in_game_ui {
            if let Ok(mut guard) = ui.lock() {
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

    /// Shell/presentation client tick without dual-world OBJECT_REGISTRY drawable bind.
    ///
    /// Mirrors the safe subset of C++ `GameClient::update` ordering that Main does not
    /// already own as dual presenters. Includes frame tick, client input device poll,
    /// client audio subsystem drain, local drawable modules, FX/weather residual,
    /// post-draw UI, beacon notifications, and message pump.
    /// Main still owns OS WindowEvent→commands, GameLogic audio event requests, and
    /// sole RenderPipeline 3D draw (`draw_display` stays off). Shroud still comes from
    /// PresentationFrame at render time — not live OBJECT_REGISTRY.
    pub fn update_presentation_shell(&mut self, delta_time: f32) -> GameClientResult<()> {
        if !self.initialized {
            return Err(GameClientError::InvalidOperation(
                "GameClient not initialized".to_string(),
            ));
        }

        let current_time = Instant::now();
        self.last_update_time = current_time;
        self.frame = self.frame.wrapping_add(1);
        publish_live_game_client_frame(self);

        self.create_frame_tick_message()?;
        // Wave 981: drain meta TOD residual onto host presentation drawables.
        if let Some(ini_tod) = crate::message_stream::meta_event::take_host_drawable_tod_residual()
        {
            let client_tod = match ini_tod {
                game_engine::common::ini::TimeOfDay::Morning => TimeOfDay::Morning,
                game_engine::common::ini::TimeOfDay::Afternoon => TimeOfDay::Afternoon,
                game_engine::common::ini::TimeOfDay::Evening => TimeOfDay::Evening,
                game_engine::common::ini::TimeOfDay::Night => TimeOfDay::Night,
                game_engine::common::ini::TimeOfDay::Invalid => TimeOfDay::Afternoon,
            };
            let _ = self.set_time_of_day(client_tod);
        }

        // Wave 988: drain NIGHT/SNOW model-condition residual onto presentation drawables.
        if let Some((is_night, is_snow)) =
            crate::message_stream::meta_event::take_host_model_condition_weather_residual()
        {
            use game_engine::common::bit_flags::{
                create_model_condition_flags, ModelConditionFlags,
            };
            let mut clear = create_model_condition_flags();
            let mut set = create_model_condition_flags();
            clear.set(ModelConditionFlags::NIGHT, true);
            clear.set(ModelConditionFlags::SNOW, true);
            if is_night {
                set.set(ModelConditionFlags::NIGHT, true);
            }
            if is_snow {
                set.set(ModelConditionFlags::SNOW, true);
            }
            for drawable in self.drawable_map.values_mut() {
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
            if let Some(drawable) = self.drawable_map.get_mut(&drawable_id) {
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

        // Startup movies remain Main/runtime-host owned; skip movie branch here.
        self.ensure_shell_visible()?;
        // Snow/Anim2D: C++ runs every GameClient::update. Live host ticks
        // them from cnc_game_engine residuals in every state (including when
        // this shell is skipped). Dual-world `update()` still ticks above.
        // C++ GameClient.cpp:587-597 — camera follows first selected drawable.
        self.update_camera_tracking_drawable();
        self.update_pre_draw_ui()?;

        // C++ visual freeze + script visual-speed residual (same as full update),
        // without Main-owned input/audio or Display DRAW dual-ownership.
        // Device state is shared THE_MOUSE/THE_KEYBOARD (Main inject; no second OS poll).
        let freeze_time = self.should_freeze_visual_time();
        let mut visual_delta = if freeze_time { 0.0 } else { delta_time };
        let visual_speed = get_script_visual_speed_multiplier();
        visual_delta = if visual_speed <= 0 {
            0.0
        } else {
            visual_delta * visual_speed as f32
        };

        // Main owns OS event intake and shared THE_MOUSE/THE_KEYBOARD (no shell
        // update_input dual-tick). Audio/SFX dispatched by Main before shell tick (no update_audio dual-drain).
        // Presentation gameplay SFX is dispatched by Main via
        // PresentationFrame::dispatch_audio_events_direct before this shell tick.

        // C++ GameClient.cpp:632-657 — GhostObjectManager::updateOrphanedObjects.
        self.update_orphaned_w3d_ghosts_if_unfrozen(freeze_time);

        // Local drawable client modules only (no OBJECT_REGISTRY shroud bind).
        // Eva residual runs via update_post_draw_ui (no dual OS input ownership).
        self.update_drawables_local(visual_delta)?;
        if self.should_skip_visual_updates_for_no_draw() {
            self.rendered_object_count = 0;
            // Wave 876: Main owns frame pacing — no shell dual-sleep.
            return Ok(());
        }

        self.update_particle_system_local_player()?;
        self.update_effects(visual_delta)?;
        apply_pending_script_display_state();
        // C++ GameClient.cpp:719-722 — TheTerrainVisual->UPDATE().
        self.update_terrain_visual();
        // C++ line 726: display UPDATE (not DRAW). Main RenderPipeline remains sole
        // 3D present path; skip draw_display to avoid dual surface present.
        self.update_display_only()?;
        // C++ W3DView::drawablePostDraw icon UI residual (health/status icons).
        self.draw_drawable_icon_ui();
        // Wave 980: weapon/UI residual peels companion.
        // Wave 978/980: presentation selection residual HUD (host empty dual-world InGameUI).
        self.draw_presentation_selection_residual();
        // C++ InGameUI::preDraw/postDraw + Drawable::drawIconUI submit.
        let _ = self.draw_live_ingame_hud();

        // C++ DisplayStringManager after drawable/effects residual.
        self.update_display_string_manager()?;

        self.update_post_draw_ui()?;
        self.process_beacon_notifications()?;
        self.pump_message_stream()?;

        self.rendered_object_count = 0;
        // Wave 876: Main owns frame pacing — no shell dual-sleep.
        let _ = current_time;
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

    pub fn update_drawables(&mut self, delta_time: f32) -> GameClientResult<()> {
        let frame = self.frame;
        let local_player_index = self.local_player_id;

        for drawable in self.drawable_map.values_mut() {
            drawable.update(delta_time);
        }

        // Host/presentation path: Wave 1020/1021 peels catalog shroud onto drawable_map
        // when dual-world registry is empty (PresentationFrame apply_* still primary).
        if OBJECT_REGISTRY.is_empty() {
            self.update_drawable_visibility(local_player_index)?;
            let _ = frame;
            return Ok(());
        }

        // C++ parity: GameClient.cpp lines 660-700 iterates drawables with shroud check.
        // For each drawable bound to an object, check shroud status and set visibility
        // before calling updateDrawable().
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
                    let _ = drawable_guard.update(delta_time, frame);
                }
            }

            let _ = (object_id, is_effectively_dead);
        })?;
        Ok(())
    }

    fn update_effects(&mut self, delta_time: f32) -> GameClientResult<()> {
        if delta_time > 0.0 {
            crate::fx_list::tick_scene_dynamic_lights();
            crate::display::view::with_tactical_view(|view| view.tick_impulse_shake());
        }
        if let Some(ref decals) = self.subsystem_manager.decal_manager {
            if let Ok(mut guard) = decals.lock() {
                let config = EffectsConfig::default();
                guard.update(delta_time, &config);
            }
        }
        if let Ok(mut weather_guard) = get_weather_system_mut() {
            if let Some(weather) = weather_guard.as_mut() {
                let camera_pos = with_tactical_view_ref(|view| view.get_3d_camera_position());
                weather.update(
                    delta_time,
                    Point3::new(camera_pos.x, camera_pos.y, camera_pos.z),
                );
            }
        }
        Ok(())
    }

    fn should_freeze_visual_time(&mut self) -> bool {
        let camera_frozen = with_tactical_view_ref(|view| {
            view.is_time_frozen() && !view.is_camera_movement_finished()
        });
        let mut freeze_time = camera_frozen
            || TheScriptEngine::is_time_frozen_debug()
            || TheScriptEngine::is_time_frozen_script()
            || TheGameLogic::is_game_paused();
        // C++ compares against GameClient::m_frame, set from live GameLogic.
        // Host calls set_frame(host_logic_frame) before the presentation shell.
        let logic_frame = self.frame;
        freeze_time = freeze_time || (self.last_visual_time_frame == logic_frame);
        self.last_visual_time_frame = logic_frame;
        freeze_time
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
            display.lock().unwrap_or_else(|e| e.into_inner()).update()?;
        }
        Ok(())
    }

    fn update_startup_movie_display(&mut self) -> GameClientResult<()> {
        if let Some(ref display) = self.subsystem_manager.display {
            let mut display = display.lock().unwrap_or_else(|e| e.into_inner());
            display.draw()?;
            display.update()?;
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
        let Some(global_data) = get_global_data() else {
            return true;
        };
        let global = global_data.read();
        global.initial_file.is_empty()
    }

    fn activate_shell_after_startup(&self) -> GameClientResult<()> {
        if !self.should_activate_shell_after_startup() {
            return Ok(());
        }

        log::info!("Activating shell after startup movie flow");
        let mut shell = get_shell();
        shell.show_shell_map(true);
        shell.show_shell(true).map_err(|err| {
            GameClientError::SubsystemError(format!(
                "Failed to activate shell after startup movies: {}",
                err
            ))
        })?;
        Ok(())
    }

    fn show_low_memory_legal_page(&self, display: &mut GraphicsDisplay) -> GameClientResult<()> {
        let Some((layout, _info)) = with_window_manager(|manager| {
            manager
                .create_layout_with_windows("Menus/LegalPage.wnd")
                .ok()
        }) else {
            return Ok(());
        };

        {
            let mut layout_mut = layout.borrow_mut();
            layout_mut.hide(false);
            layout_mut.bring_forward();
        }

        let begin = Instant::now();
        while begin.elapsed() < Duration::from_millis(4000) {
            with_window_manager(|manager| manager.update());
            display.draw()?;
            thread::sleep(Duration::from_millis(100));
        }

        with_window_manager(|manager| manager.destroy_layout(&layout));
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
                display.play_logo_movie(movie_name.to_string(), 5000, 3000);
                global.play_intro = false;
                global.after_intro = true;
                self.startup_sizzle_pending = true;
            }
            StartupMovieAction::PlaySizzle(movie_name) => {
                global.allow_exit_out_of_movies = true;
                if display.play_movie(movie_name.to_string()) {
                    self.startup_sizzle_pending = false;
                    return Ok(());
                }
                self.startup_sizzle_pending = false;
                global.break_the_movie = true;
                global.after_intro = false;
                drop(global);
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
                self.activate_shell_after_startup()?;
            }
        }
        Ok(())
    }

    pub fn ensure_shell_visible(&self) -> GameClientResult<()> {
        if !self.should_activate_shell_after_startup() {
            return Ok(());
        }

        let mut shell = get_shell();
        let needs_screen = shell.get_screen_count() == 0 && !shell.is_shell_map_on();
        if needs_screen || !shell.is_shell_active() {
            log::info!(
                "Activating shell: screen_count={}, shell_active={}, shell_map_on={}",
                shell.get_screen_count(),
                shell.is_shell_active(),
                shell.is_shell_map_on()
            );
            shell.show_shell_map(true);
            shell.show_shell(true).map_err(|err| {
                GameClientError::SubsystemError(format!(
                    "Failed to ensure shell visibility: {}",
                    err
                ))
            })?;
        }
        Ok(())
    }

    pub fn update_pre_draw_ui(&mut self) -> GameClientResult<()> {
        if let Some(ref window_manager) = self.subsystem_manager.window_manager {
            window_manager
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .update()?;
        }

        if let Some(ref video_player) = self.subsystem_manager.video_player {
            video_player
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .update()?;
        }

        Ok(())
    }

    pub fn update_post_draw_ui(&mut self) -> GameClientResult<()> {
        {
            let mut shell = get_shell();
            shell.update().map_err(|err| {
                GameClientError::SubsystemError(format!("Shell update failed: {err}"))
            })?;
        }

        if let Some(ref ui) = self.subsystem_manager.in_game_ui {
            ui.lock().unwrap_or_else(|e| e.into_inner()).update()?;
        }

        crate::eva::update_eva_system();

        Ok(())
    }

    pub fn update_display_string_manager(&self) -> GameClientResult<()> {
        crate::display_string_manager::update_display_string_manager()
            .map_err(|err| GameClientError::SubsystemError(format!("{err}")))
    }

    fn update_particle_system_local_player(&self) -> GameClientResult<()> {
        self.set_particle_system_local_player()?;
        if let Ok(mut manager_guard) =
            crate::effects::particle_manager::get_particle_system_manager_mut()
        {
            if let Some(manager) = manager_guard.as_mut() {
                manager.update(self.local_player_id as i32, self.frame);
            }
        }
        crate::effects::update_tracer_fx(self.frame);
        Ok(())
    }

    fn set_particle_system_local_player(&self) -> GameClientResult<()> {
        if let Ok(mut manager_guard) =
            crate::effects::particle_manager::get_particle_system_manager_mut()
        {
            if let Some(manager) = manager_guard.as_mut() {
                manager.set_local_player_index(self.local_player_id);
            }
        }
        Ok(())
    }

    fn update_ui(&mut self) -> GameClientResult<()> {
        {
            let mut shell = get_shell();
            shell.update().map_err(|err| {
                GameClientError::SubsystemError(format!("Shell update failed: {err}"))
            })?;
        }

        if let Some(ref ui) = self.subsystem_manager.in_game_ui {
            ui.lock().unwrap_or_else(|e| e.into_inner()).update()?;
        }

        if let Some(ref window_manager) = self.subsystem_manager.window_manager {
            window_manager
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .update()?;
        }

        if let Some(ref video_player) = self.subsystem_manager.video_player {
            video_player
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .update()?;
        }

        crate::eva::update_eva_system();

        Ok(())
    }

    fn process_beacon_notifications(&self) -> GameClientResult<()> {
        let notifications = beacon_display::drain_notifications();
        if notifications.is_empty() {
            return Ok(());
        }

        for notification in notifications {
            if let Some(ref ui) = self.subsystem_manager.in_game_ui {
                let mut ui_guard = ui.lock().map_err(|_| {
                    GameClientError::SubsystemError("In-game UI lock poisoned".to_string())
                })?;
                ui_guard
                    .handle_beacon_notification(&notification)
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
