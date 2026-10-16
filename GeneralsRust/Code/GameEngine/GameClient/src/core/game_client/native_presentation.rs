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
        let assets = crate::render_bridge::THE_RENDER_BRIDGE
            .lock()
            .map_err(|error| {
                GameClientError::SubsystemError(format!("native view assets unavailable: {error}"))
            })?;
        let mut manager_guard = crate::effects::particle_manager::get_particle_system_manager_mut()
            .map_err(|error| {
                GameClientError::SubsystemError(format!(
                    "native view particle owner unavailable: {error}"
                ))
            })?;
        let manager = manager_guard.as_mut().ok_or_else(|| {
            GameClientError::SubsystemError("native view particle owner absent".into())
        })?;
        self.run_native_views_with_services(
            manager,
            assets.as_ref().map(|bridge| bridge.asset_manager()),
            &public_bones,
            frames,
            materials,
            views,
            phase,
        )?;
        // FX retirement can reenter drawable teardown and its particle owner.
        // Preserve the prior boundary: no service guard crosses those callbacks.
        drop(manager_guard);
        drop(assets);
        crate::effects::update_tracer_fx(frames.logic_frame);
        crate::effects::update_ray_effects(frames.logic_frame);
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
        if frames.client_frame != self.frame {
            return Err(GameClientError::InvalidOperation(
                "native view Client frame mismatch".into(),
            ));
        }
        if phase.draw_drawables {
            for view in views {
                view.validate()?;
            }
            let facts = self.resolve_material_draw_facts(materials)?;
            self.native_particle_capability_gaps.clear();
            for view in views {
                // Preserve view order and repeated eligibility across views.
                // Each receiver completes material -> installed Draw before
                // visiting the next receiver; this is not a union or GPU cull.
                for id in self.native_roster_for_view(view)? {
                    self.advance_material_drawable(id, &facts)?;
                    self.prepare_native_drawable_particles_with_assets(
                        id,
                        particles,
                        assets,
                        public_bones,
                        frames.client_frame,
                    )?;
                }
            }
        }
        // The receiving particle owner advances once, after ALL view work.
        self.run_native_draw_particle_phase_with_services(
            particles,
            assets,
            public_bones,
            frames,
            false,
            phase.update_particles,
        )
    }
}
