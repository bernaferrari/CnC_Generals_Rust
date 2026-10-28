/// Receiving native visual facts observed once after pre-draw UI callbacks.
/// This is ephemeral input; the client retains only its existing frame marker.
#[derive(Debug, Clone, Copy)]
pub struct NativeVisualTime {
    pub frozen: bool,
    pub visual_speed: f32,
}

// The owned continuation between client CU and the driving display's Draw.
// It contains no mutable world publication or borrowed self-reference.
#[must_use]
pub struct PreparedPresentationShell {
    owner: std::sync::Arc<()>,
    generation: u64,
    frames: NativeParticleFrames,
    game_mode: i32,
    frozen: bool,
    skip_draw: bool,
}

impl PreparedPresentationShell {
    /// C++ noDraw returns before invoking Display::draw and its virtual clock.
    pub fn display_requested(&self) -> bool {
        !self.skip_draw
    }
}

/// Admission supplied by the driving display's single virtual clock. GPU
/// eligibility is deliberately absent: TiVO can skip GPU while CPU Draw runs.
#[derive(Debug, Clone, Copy)]
pub struct NativeDrawPhase {
    pub draw_drawables: bool,
    pub update_particles: bool,
}

impl GameClient {
    fn invalidate_presentation_shell(&mut self) {
        self.presentation_shell_owner = std::sync::Arc::new(());
        self.presentation_shell_generation = 0;
    }

    pub fn draw_native_views_with_frames(
        &mut self,
        frames: NativeParticleFrames,
        materials: &[NativeMaterialObservation],
        views: &[NativeDrawView],
        phase: NativeDrawPhase,
    ) -> GameClientResult<()> {
        self.draw_native_views_with_object_lifecycle(frames, materials, views, phase, None)
    }
    pub fn draw_native_views_with_object_lifecycle(
        &mut self,
        frames: NativeParticleFrames,
        materials: &[NativeMaterialObservation],
        views: &[NativeDrawView],
        phase: NativeDrawPhase,
        lifecycle: Option<&mut crate::effects::particle_manager::NativeParticleObjectLifecycle<'_>>,
    ) -> GameClientResult<()> {
        self.draw_native_views_with_name_keys(frames, materials, views, phase, lifecycle, None)
    }

    pub fn draw_native_views_with_object_lifecycle_and_content(
        &mut self,
        frames: NativeParticleFrames,
        materials: &[NativeMaterialObservation],
        views: &[NativeDrawView],
        phase: NativeDrawPhase,
        lifecycle: Option<&mut crate::effects::particle_manager::NativeParticleObjectLifecycle<'_>>,
        content: &mut game_engine::common::thing::module_factory::ModuleFactory,
    ) -> GameClientResult<()> {
        let keys = content.receiving_name_keys_mut().ok_or_else(|| {
            GameClientError::InvalidOperation(
                "native Draw requires its receiving name-key owner".to_owned(),
            )
        })?;
        self.draw_native_views_with_name_keys(
            frames,
            materials,
            views,
            phase,
            lifecycle,
            Some(keys),
        )
    }

    fn draw_native_views_with_name_keys(
        &mut self,
        frames: NativeParticleFrames,
        materials: &[NativeMaterialObservation],
        views: &[NativeDrawView],
        phase: NativeDrawPhase,
        lifecycle: Option<&mut crate::effects::particle_manager::NativeParticleObjectLifecycle<'_>>,
        keys: Option<&mut game_engine::common::name_key_generator::NameKeyNamespace>,
    ) -> GameClientResult<()> {
        if frames.client_frame != self.frame {
            return Err(GameClientError::InvalidOperation(
                "native view Client frame mismatch".into(),
            ));
        }
        // Fast/throttled/unavailable-device paths perform no native work.
        if !phase.draw_drawables && !phase.update_particles {
            return Ok(());
        }
        let public_bones: Vec<_> = game_engine::common::global_data::read()
            .standard_public_bones
            .iter()
            .map(|bone| bone.as_str().to_owned())
            .collect();
        let assets = self.render_bridge.asset_manager();
        self.drawables
            .particles
            .admit_effective_definitions()
            .map_err(|error| GameClientError::SubsystemError(error.to_string()))?;
        let (state, manager) = (&mut self.drawables.state, &mut self.drawables.particles);
        NativeRenderReceiver::new(
            state,
            &mut self.native_particle_capability_gaps,
            self.native_audio_device.as_ref(),
            self.subsystem_manager.in_game_ui.as_mut(),
            self.frame,
            self.local_player_id,
            self.input_authority,
        )
        .run_native_views_with_keys(
            manager,
            Some(assets),
            &public_bones,
            frames,
            materials,
            views,
            phase,
            lifecycle,
            keys,
        )?;
        // FX retirement can reenter drawable teardown and its particle owner.
        // Preserve the prior boundary: no service guard crosses those callbacks.
        Ok(())
    }

    fn run_native_views_with_services(
        &mut self,
        particles: &mut crate::effects::particle_manager::ParticleSystemManager,
        assets: Option<&ww3d_assets::AssetManager>,
        public_bones: &[String],
        frames: NativeParticleFrames,
        materials: &[NativeMaterialObservation],
        views: &[NativeDrawView],
        phase: NativeDrawPhase,
    ) -> GameClientResult<()> {
        self.run_native_views_with_object_lifecycle(
            particles,
            assets,
            public_bones,
            frames,
            materials,
            views,
            phase,
            None,
        )
    }
    fn run_native_views_with_object_lifecycle(
        &mut self,
        particles: &mut crate::effects::particle_manager::ParticleSystemManager,
        assets: Option<&ww3d_assets::AssetManager>,
        public_bones: &[String],
        frames: NativeParticleFrames,
        materials: &[NativeMaterialObservation],
        views: &[NativeDrawView],
        phase: NativeDrawPhase,
        lifecycle: Option<&mut crate::effects::particle_manager::NativeParticleObjectLifecycle<'_>>,
    ) -> GameClientResult<()> {
        self.run_native_views_with_keys(
            particles,
            assets,
            public_bones,
            frames,
            materials,
            views,
            phase,
            lifecycle,
            None,
        )
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
        self.native_render_receiver().run_native_views_with_keys(
            particles,
            assets,
            public_bones,
            frames,
            materials,
            views,
            phase,
            lifecycle,
            keys,
        )
    }
}

#[cfg(test)]
#[path = "native_particle_owner_lifecycle_tests.rs"]
mod particle_owner_lifecycle_tests;
