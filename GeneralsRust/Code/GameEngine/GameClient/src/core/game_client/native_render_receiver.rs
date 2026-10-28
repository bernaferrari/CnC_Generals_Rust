// Disjoint native render loan: mutable Drawable state and immutable render assets
// remain under the same receiving client without taking either owner out of it.
struct NativeRenderReceiver<'a, 'ambient> {
    drawables: &'a mut drawable_world::DrawableState,
    native_particle_capability_gaps: &'a mut Vec<NativeParticleCapabilityGap>,
    audio_device:
        Option<&'a std::sync::Arc<std::sync::Mutex<game_engine::common::audio::AudioManager>>>,
    ui: Option<&'a mut InGameUISubsystem>,
    frame: u32,
    local_player_id: i32,
    input_authority: ClientInputAuthority,
    ambient_update: Option<&'a mut crate::drawable::drawable::NativeAmbientUpdate<'ambient>>,
}
impl<'a, 'ambient> NativeRenderReceiver<'a, 'ambient> {
    fn new(
        drawables: &'a mut drawable_world::DrawableState,
        native_particle_capability_gaps: &'a mut Vec<NativeParticleCapabilityGap>,
        audio_device: Option<
            &'a std::sync::Arc<std::sync::Mutex<game_engine::common::audio::AudioManager>>,
        >,
        ui: Option<&'a mut InGameUISubsystem>,
        frame: u32,
        local_player_id: i32,
        input_authority: ClientInputAuthority,
    ) -> Self {
        Self {
            drawables,
            native_particle_capability_gaps,
            audio_device,
            ui,
            frame,
            local_player_id,
            input_authority,
            ambient_update: None,
        }
    }

    fn new_with_ambient(
        drawables: &'a mut drawable_world::DrawableState,
        native_particle_capability_gaps: &'a mut Vec<NativeParticleCapabilityGap>,
        audio_device: Option<
            &'a std::sync::Arc<std::sync::Mutex<game_engine::common::audio::AudioManager>>,
        >,
        ui: Option<&'a mut InGameUISubsystem>,
        frame: u32,
        local_player_id: i32,
        input_authority: ClientInputAuthority,
        ambient_update: &'a mut crate::drawable::drawable::NativeAmbientUpdate<'ambient>,
    ) -> Self {
        let mut receiver = Self::new(
            drawables,
            native_particle_capability_gaps,
            audio_device,
            ui,
            frame,
            local_player_id,
            input_authority,
        );
        receiver.ambient_update = Some(ambient_update);
        receiver
    }
    fn run_native_views_with_keys(
        &mut self,
        particles: &mut crate::effects::particle_manager::ParticleSystemManager,
        assets: Option<&ww3d_assets::AssetManager>,
        public_bones: &[String],
        frames: NativeParticleFrames,
        materials: &[NativeMaterialObservation],
        views: &[NativeDrawView],
        phase: NativeDrawPhase,
        lifecycle: Option<&mut crate::effects::particle_manager::NativeParticleObjectLifecycle<'_>>,
        mut keys: Option<&mut game_engine::common::name_key_generator::NameKeyNamespace>,
    ) -> GameClientResult<()> {
        if frames.client_frame != self.frame {
            return Err(GameClientError::InvalidOperation(
                "native view Client frame mismatch".into(),
            ));
        }
        if phase.draw_drawables {
            for view in views {
                view.validate()?;
            }
            let facts = self.drawables.resolve_material_draw_facts(materials)?;
            self.native_particle_capability_gaps.clear();
            for view in views {
                // Preserve view order and repeated eligibility across views.
                // Each receiver completes material -> installed Draw before
                // visiting the next receiver; this is not a union or GPU cull.
                for id in self.drawables.native_roster_for_view(view)? {
                    self.drawables.advance_material_drawable(id, &facts)?;
                    self.prepare_native_drawable_particles_with_assets_and_keys(
                        id,
                        particles,
                        assets,
                        public_bones,
                        frames.client_frame,
                        keys.as_deref_mut(),
                    )?;
                }
            }
        }
        // The receiving particle owner advances once, after ALL view work.
        self.run_native_draw_particle_phase_with_object_lifecycle(
            particles,
            assets,
            public_bones,
            frames,
            false,
            phase.update_particles,
            lifecycle,
        )
    }

    fn run_native_draw_particle_phase_with_services(
        &mut self,
        particles: &mut crate::effects::particle_manager::ParticleSystemManager,
        assets: Option<&ww3d_assets::AssetManager>,
        public_bones: &[String],
        frames: NativeParticleFrames,
        draw_models: bool,
        advance_particles: bool,
    ) -> GameClientResult<()> {
        self.run_native_draw_particle_phase_with_object_lifecycle(
            particles,
            assets,
            public_bones,
            frames,
            draw_models,
            advance_particles,
            None,
        )
    }

    fn run_native_draw_particle_phase_with_object_lifecycle(
        &mut self,
        particles: &mut crate::effects::particle_manager::ParticleSystemManager,
        assets: Option<&ww3d_assets::AssetManager>,
        public_bones: &[String],
        frames: NativeParticleFrames,
        draw_models: bool,
        advance_particles: bool,
        lifecycle: Option<&mut crate::effects::particle_manager::NativeParticleObjectLifecycle<'_>>,
    ) -> GameClientResult<()> {
        if frames.client_frame != self.frame {
            return Err(GameClientError::InvalidOperation(
                "native particle phase Client clock does not match receiving owner".into(),
            ));
        }
        if draw_models {
            self.prepare_native_draw_particles_with_assets(
                particles,
                assets,
                public_bones,
                frames.client_frame,
            )?;
        }
        particles.set_local_player_index(self.local_player_id);
        // A retained Core Display walk, when present, performs its ordered
        // Draw→particle phase afterward; native callbacks never enter it.
        if advance_particles {
            let drawables = &self.drawables.drawable_map;
            let bindings = &self.drawables.presentation_direct_drawable_bindings;
            let resolve = |id: DrawableId| {
                let drawable = drawables.get(&id)?;
                let mut transform = drawable
                    .as_any()
                    .downcast_ref::<BasicDrawable>()
                    .map(BasicDrawable::native_particle_parent_transform)
                    .unwrap_or_else(|| drawable.get_transform().to_glam());
                if bindings.contains_key(&id)
                    || drawable
                        .as_any()
                        .downcast_ref::<BasicDrawable>()
                        .is_some_and(|basic| basic.native_auxiliary_generation().is_some())
                {
                    let swizzle = glam::Mat4::from_cols(
                        glam::Vec4::X,
                        glam::Vec4::Z,
                        glam::Vec4::Y,
                        glam::Vec4::W,
                    );
                    transform = swizzle * transform * swizzle;
                }
                Some(crate::effects::particle_system::DrawableParticlePose {
                    transform,
                    is_shrouded: drawable.fully_obscured_by_shroud()?,
                })
            };
            if let Some(lifecycle) = lifecycle {
                let associations = &self.drawables.drawable_object_map;
                let object_pose = |id| {
                    let mut pose = (lifecycle.object_pose)(id)?;
                    if let Some(drawable) = associations.get(&id).and_then(|id| resolve(*id)) {
                        pose.transform = drawable.transform;
                    }
                    Some(pose)
                };
                let mut receiving =
                    crate::effects::particle_manager::NativeParticleObjectLifecycle {
                        object_is_live: lifecycle.object_is_live,
                        object_pose: &object_pose,
                        ground_height: lifecycle.ground_height,
                        attached_object_lost: &mut *lifecycle.attached_object_lost,
                    };
                particles.update_with_receiving_drawables_and_objects(
                    self.local_player_id as i32,
                    frames.logic_frame,
                    frames.client_frame,
                    &resolve,
                    &mut receiving,
                );
            } else {
                particles.update_with_receiving_drawables(
                    self.local_player_id as i32,
                    frames.logic_frame,
                    frames.client_frame,
                    &resolve,
                );
            }
        }
        Ok(())
    }

    fn prepare_native_draw_particles_with_assets(
        &mut self,
        particles: &mut crate::effects::particle_manager::ParticleSystemManager,
        assets: Option<&ww3d_assets::AssetManager>,
        public_bones: &[String],
        frame: u32,
    ) -> GameClientResult<()> {
        self.native_particle_capability_gaps.clear();
        for id in self.drawables.native_drawable_order.clone() {
            self.prepare_native_drawable_particles_with_assets(
                id,
                particles,
                assets,
                public_bones,
                frame,
            )?;
        }
        Ok(())
    }

    fn prepare_native_drawable_particles_with_assets(
        &mut self,
        id: DrawableId,
        particles: &mut crate::effects::particle_manager::ParticleSystemManager,
        assets: Option<&ww3d_assets::AssetManager>,
        public_bones: &[String],
        frame: u32,
    ) -> GameClientResult<()> {
        self.prepare_native_drawable_particles_with_assets_and_keys(
            id,
            particles,
            assets,
            public_bones,
            frame,
            None,
        )
    }

    fn prepare_native_drawable_particles_with_assets_and_keys(
        &mut self,
        id: DrawableId,
        particles: &mut crate::effects::particle_manager::ParticleSystemManager,
        assets: Option<&ww3d_assets::AssetManager>,
        public_bones: &[String],
        frame: u32,
        keys: Option<&mut game_engine::common::name_key_generator::NameKeyNamespace>,
    ) -> GameClientResult<()> {
        let drawable = self
            .drawables
            .drawable_map
            .get_mut(&id)
            .expect("native Drawable roster references a live receiver");
        let visible = drawable.is_visible();
        if let Some(drawable) = drawable.as_any_mut().downcast_mut::<BasicDrawable>() {
            drawable.synchronize_native_particle_visibility(particles);
            if !visible {
                return Ok(());
            }
            let outcomes = drawable
                .prepare_native_state_particles_with_keys(
                    particles,
                    assets,
                    public_bones,
                    frame,
                    keys,
                )
                .map_err(GameClientError::InvalidOperation)?;
            for (module_index, outcome) in outcomes {
                use gamelogic::object::draw::draw_module::NativeStateParticlePreparation;
                let (module_preparation, animation_pose, terrain_tracks) = match outcome {
                    NativeStateParticlePreparation::Prepared(_) => continue,
                    NativeStateParticlePreparation::Unsupported {
                        animation_pose,
                        terrain_tracks,
                    } => (false, animation_pose, terrain_tracks),
                    NativeStateParticlePreparation::UnsupportedModule => (true, false, false),
                };
                self.native_particle_capability_gaps
                    .push(NativeParticleCapabilityGap {
                        drawable_id: id,
                        module_index,
                        module_preparation,
                        animation_pose,
                        terrain_tracks,
                    });
            }
        }
        Ok(())
    }

    fn tick_mapped_drawables_with_particles(
        &mut self,
        frames: NativeParticleFrames,
        delta_time: f32,
        mut particles: Option<&mut crate::effects::particle_manager::ParticleSystemManager>,
        assets: Option<&ww3d_assets::AssetManager>,
    ) -> GameClientResult<Vec<DrawableId>> {
        let mut expired = Vec::new();
        // C++ walks the constructor-prepended Drawable list. Lookup storage
        // must not choose the order of ClientUpdate or expiration callbacks.
        // Callbacks borrow one installed node and cannot edit this owner's roster.
        // Removing that node shifts its successor into the current cursor slot,
        // preserving the C++ cached-next walk without a second snapshot roster.
        let mut cursor = 0;
        while let Some(&id) = self.drawables.native_drawable_order.get(cursor) {
            let host_projection = self
                .drawables
                .presentation_direct_drawable_bindings
                .contains_key(&id)
                || self
                    .drawables
                    .drawable_map
                    .get(&id)
                    .and_then(|draw| draw.as_any().downcast_ref::<BasicDrawable>())
                    .is_some_and(|basic| basic.native_auxiliary_generation().is_some());
            let drawable = self
                .drawables
                .drawable_map
                .get_mut(&id)
                .expect("native Drawable roster references a live update receiver");
            drawable.set_current_frame(frames.client_frame);
            if let Some(basic) = drawable.as_any_mut().downcast_mut::<BasicDrawable>() {
                basic
                    .update_native_client_modules(particles.as_deref_mut(), host_projection, assets)
                    .map_err(GameClientError::InvalidOperation)?;
                if let Some(ambient) = self.ambient_update.as_deref_mut() {
                    basic
                        .update_after_client_modules_with_ambient(
                            frames.logic_frame,
                            ambient,
                            self.audio_device,
                        )
                        .map_err(GameClientError::InvalidOperation)?;
                } else {
                    basic.update_after_client_modules(frames.logic_frame);
                }
            } else {
                drawable.update(delta_time, frames.logic_frame);
            }
            if drawable.is_expired(frames.logic_frame) {
                if !drawable.as_any().is::<BasicDrawable>() {
                    return Err(GameClientError::InvalidOperation(
                        "native expiration requires an installed BasicDrawable teardown".into(),
                    ));
                }
                self.destroy_drawable_with_particles(id, particles.as_deref_mut())?;
                expired.push(id);
                // Current was unlinked; its cached successor now occupies cursor.
            } else {
                cursor += 1;
            }
        }
        Ok(expired)
    }

    fn destroy_drawable_with_particles(
        &mut self,
        id: DrawableId,
        mut particles: Option<&mut crate::effects::particle_manager::ParticleSystemManager>,
    ) -> GameClientResult<()> {
        let mut laser_particles = false;
        let mut state_particles = 0;
        if let Some(basic) = self
            .drawables
            .drawable_map
            .get_mut(&id)
            .and_then(|draw| draw.as_any_mut().downcast_mut::<BasicDrawable>())
        {
            basic
                .preflight_native_retirement_with_device(self.audio_device)
                .map_err(GameClientError::InvalidOperation)?;
            state_particles = basic.native_state_particle_count();
            laser_particles = basic
                .native_client_particle_retirement_required()
                .map_err(GameClientError::InvalidOperation)?;
        }
        if particles.is_none() && (laser_particles || state_particles != 0) {
            return Err(GameClientError::SubsystemError(
                "receiving particle manager absent during Drawable retirement".into(),
            ));
        }
        if let Some(drawable) = self.drawables.drawable_map.get(&id) {
            // Native Main retires its actual world selection with the object.
            // Only the original standalone Core route owns this Core registry.
            if self.input_authority == ClientInputAuthority::StandaloneCore {
                let object_id = drawable.get_object_id().unwrap_or(id.0);
                crate::gui::ingame_ui::InGameUI::disregard_live_drawable(object_id);
            }
            if let Some(ui) = self.ui.as_deref_mut() {
                ui.disregard_drawable(drawable.as_ref())?;
            }
        }
        self.native_particle_capability_gaps
            .retain(|gap| gap.drawable_id != id);
        self.drawables
            .owned_laser_order
            .retain(|owner| *owner != id);
        self.drawables
            .native_drawable_order
            .retain(|known| *known != id);
        let mut removed = self.drawables.drawable_map.remove(&id);
        if let Some(drawable) = removed.as_ref() {
            if let Some(object_id) = drawable.get_object_id() {
                if self.drawables.drawable_object_map.get(&object_id).copied() == Some(id) {
                    self.drawables.drawable_object_map.remove(&object_id);
                }
                prune_live_host_animated_particle_sys_bones(object_id);
            }
        }
        self.drawables
            .presentation_direct_drawable_bindings
            .remove(&id);
        self.drawables
            .text_bearing_drawables
            .retain(|&stored_id| stored_id != id);
        // Identity is no longer discoverable, as in C++ deleteInstance. Retire
        // actual installed fields before Drop without rediscovering this manager.
        let mut retirement = Ok(());
        if let Some(basic) = removed
            .as_mut()
            .and_then(|draw| draw.as_any_mut().downcast_mut::<BasicDrawable>())
        {
            if let Some(manager) = particles.as_deref_mut() {
                basic.retire_native_state_particles(manager);
                retirement = basic
                    .retire_native_client_particles(manager)
                    .map_err(GameClientError::InvalidOperation);
            }
        }
        if let Some(basic) = removed
            .as_mut()
            .and_then(|draw| draw.as_any_mut().downcast_mut::<BasicDrawable>())
        {
            basic
                .finish_native_retirement_with_device(self.audio_device)
                .map_err(GameClientError::InvalidOperation)?;
        }
        self.drawables.ray_effects.delete(id);
        drop(removed);
        retirement
    }
}

impl drawable_world::DrawableState {
    fn validate_material_observations(
        &self,
        facts: &[NativeMaterialObservation],
    ) -> GameClientResult<()> {
        let mut seen = std::collections::HashSet::new();
        for fact in facts {
            let key = fact.binding_key;
            if !seen.insert(key)
                || self
                    .presentation_direct_drawable_state(key.host_epoch, key.object_id)
                    .is_none_or(|state| state.binding_key != key)
                || !self
                    .drawable_map
                    .get(&key.drawable_id)
                    .is_some_and(|drawable| drawable.as_any().is::<BasicDrawable>())
            {
                return Err(GameClientError::InvalidOperation(
                    "stale or duplicate native material observation".into(),
                ));
            }
        }
        Ok(())
    }

    fn resolve_material_draw_facts(
        &self,
        facts: &[NativeMaterialObservation],
    ) -> GameClientResult<HashMap<PresentationDirectDrawableBindingKey, NativeMaterialObservation>>
    {
        // CU may retire an owner before Draw; live reused IDs still undergo
        // the complete key check. No registry or guessed-alive fallback.
        let live: Vec<_> = facts
            .iter()
            .copied()
            .filter(|fact| {
                self.drawable_map
                    .contains_key(&fact.binding_key.drawable_id)
            })
            .collect();
        self.validate_material_observations(&live)?;
        let by_key: HashMap<_, _> = live
            .into_iter()
            .map(|fact| (fact.binding_key, fact))
            .collect();
        for id in &self.native_drawable_order {
            if let Some(binding) = self.presentation_direct_drawable_bindings.get(id) {
                if !by_key.contains_key(&binding.binding_key) {
                    return Err(GameClientError::InvalidOperation(
                        "missing native material observation".into(),
                    ));
                }
            }
        }
        Ok(by_key)
    }

    fn advance_material_drawable(
        &mut self,
        id: DrawableId,
        by_key: &HashMap<PresentationDirectDrawableBindingKey, NativeMaterialObservation>,
    ) -> GameClientResult<()> {
        let receiver = self.drawable_map.get_mut(&id).ok_or_else(|| {
            GameClientError::InvalidOperation(
                "native Draw receiver retired during traversal".into(),
            )
        })?;
        let Some(receiver) = receiver.as_any_mut().downcast_mut::<BasicDrawable>() else {
            return Ok(());
        };
        if let Some(binding) = self.presentation_direct_drawable_bindings.get(&id) {
            let fact = by_key.get(&binding.binding_key).ok_or_else(|| {
                GameClientError::InvalidOperation(
                    "native Draw binding changed during traversal".into(),
                )
            })?;
            receiver.advance_material_draw(fact.effectively_dead, fact.frenzy);
        } else {
            receiver.advance_core_material_draw();
        }
        Ok(())
    }

    pub(crate) fn native_roster_for_view(
        &self,
        view: &NativeDrawView,
    ) -> GameClientResult<Vec<DrawableId>> {
        view.validate()?;
        let mut selected = Vec::new();
        for &id in &self.native_drawable_order {
            let drawable = self.drawable_map.get(&id).ok_or_else(|| {
                GameClientError::InvalidOperation("native view roster receiver absent".into())
            })?;
            let position = drawable.get_position();
            let point = if let Some(binding) = self.presentation_direct_drawable_bindings.get(&id) {
                let key = binding.binding_key;
                if key.drawable_id != id
                    || self
                        .presentation_direct_drawable_state(key.host_epoch, key.object_id)
                        .is_none_or(|state| state.binding_key != key)
                    || !drawable.as_any().is::<BasicDrawable>()
                {
                    return Err(GameClientError::InvalidOperation(
                        "stale native view binding".into(),
                    ));
                }
                // Main stores raw host positions Y-up. Instance matrices,
                // float pose, scale and recoil are not getPosition().
                [position.x, position.z, position.y]
            } else if drawable
                .as_any()
                .downcast_ref::<BasicDrawable>()
                .is_some_and(|basic| basic.native_auxiliary_generation().is_some())
            {
                [position.x, position.z, position.y]
            } else {
                [position.x, position.y, position.z]
            };
            if view.region.contains(point) {
                selected.push(id);
            }
        }
        Ok(selected)
    }
}

impl GameClient {
    fn native_render_receiver(&mut self) -> NativeRenderReceiver<'_, '_> {
        NativeRenderReceiver::new(
            &mut self.drawables.state,
            &mut self.native_particle_capability_gaps,
            self.native_audio_device.as_ref(),
            self.subsystem_manager.in_game_ui.as_mut(),
            self.frame,
            self.local_player_id,
            self.input_authority,
        )
    }
}

impl GameClient {
    pub fn render_bridge(&self) -> &crate::render_bridge::RenderBridge {
        &self.render_bridge
    }
    pub fn render_bridge_mut(&mut self) -> &mut crate::render_bridge::RenderBridge {
        &mut self.render_bridge
    }
}

impl GameClient {
    pub fn split_render_bridge_and_drawables(
        &mut self,
    ) -> (
        &mut crate::render_bridge::RenderBridge,
        NativeDrawableOwner<'_>,
    ) {
        let audio_device = self.native_audio_device.as_ref();
        let mut drawables = self.drawables.borrow_native();
        drawables.audio_device = audio_device;
        (&mut self.render_bridge, drawables)
    }
}

impl drawable_world::DrawableState {
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
}

impl drawable_world::DrawableState {
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
}

impl NativeDrawableOwner<'_> {
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
}

impl GameClient {
    pub fn split_render_bridge_drawables_and_ui_renderer(
        &mut self,
    ) -> (
        &mut crate::render_bridge::RenderBridge,
        NativeDrawableOwner<'_>,
        Option<&mut crate::gui::UIRenderer>,
        &crate::gui::window_manager::WindowManagerOwner,
        &crate::gui::ime_manager::ImeManager,
        Option<&mut crate::core::subsystems::InGameUISubsystem>,
        Option<&mut crate::input::Mouse>,
    ) {
        (
            &mut self.render_bridge,
            NativeDrawableOwner {
                auxiliary_identity: &self.drawables.auxiliary_identity,
                drawables: &mut self.drawables.state,
                particles: &mut self.drawables.particles,
                audio_device: self.native_audio_device.as_ref(),
            },
            self.ui_renderer.as_mut(),
            &self.window_manager_owner,
            &self.ime,
            self.subsystem_manager.in_game_ui.as_mut(),
            self.subsystem_manager.input_mouse.as_mut(),
        )
    }
    pub fn draw_receiving_ui(
        &mut self,
        facts: crate::display::display_fx::DisplayDrawFacts,
        power: Option<crate::gui::window_draw_context::ReceivingPowerObservation>,
    ) {
        let mut context = crate::gui::window_draw_context::WindowDrawContext::new(
            self.ui_renderer.as_mut(),
            facts,
        );
        context.ime = Some(&self.ime);
        context.receiving_power = Some(power);
        self.window_manager_owner
            .with_manager(|manager| manager.draw_all(&mut context));
    }
    pub fn draw_receiving_ui_with_radar(
        &mut self,
        radar: &mut crate::gui::w3d_gadget_draw::RadarDrawContext<'_>,
        facts: crate::display::display_fx::DisplayDrawFacts,
        power: Option<crate::gui::window_draw_context::ReceivingPowerObservation>,
    ) {
        let mut context = crate::gui::window_draw_context::WindowDrawContext::new(
            self.ui_renderer.as_mut(),
            facts,
        );
        context.ime = Some(&self.ime);
        context.receiving_power = Some(power);
        self.window_manager_owner
            .with_manager(|manager| manager.draw_all_with_radar(radar, &mut context));
    }
}
