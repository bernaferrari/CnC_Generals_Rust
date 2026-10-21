// Native installed Draw state-emitter work at the original Display CPU phase.

/// Clocks sampled from the driving logic and receiving Client owners. They are
/// separate C++ fields and may not be inferred from one another.
#[derive(Debug, Clone, Copy)]
pub struct NativeParticleFrames {
    pub logic_frame: u32,
    pub client_frame: u32,
}
/// Missing receiving services for a valid authored Draw module. This is a
/// migration diagnostic, never a factory failure or an installed substitute pose.
#[derive(Debug, Clone, PartialEq, Eq)]
struct NativeParticleCapabilityGap {
    drawable_id: DrawableId,
    module_index: usize,
    module_preparation: bool,
    animation_pose: bool,
    terrain_tracks: bool,
}
impl GameClient {
    fn retire_native_drawable_particles(&mut self, id: Option<DrawableId>) -> GameClientResult<()> {
        self.native_particle_capability_gaps
            .retain(|gap| id.is_some_and(|id| gap.drawable_id != id));
        let count: usize = self
            .drawables
            .drawable_map
            .iter_mut()
            .filter(|(known, _)| id.is_none_or(|id| id == **known))
            .filter_map(|(_, drawable)| drawable.as_any_mut().downcast_mut::<BasicDrawable>())
            .map(|drawable| drawable.native_state_particle_count())
            .sum();
        if count == 0 {
            return Ok(());
        }
        let mut manager = crate::effects::particle_manager::get_particle_system_manager_mut()
            .map_err(|error| {
                GameClientError::SubsystemError(format!(
                    "Particle manager unavailable during Drawable retirement: {error}"
                ))
            })?;
        let manager = manager.as_mut().ok_or_else(|| {
            GameClientError::SubsystemError(
                "receiving particle manager absent during native emitter retirement".into(),
            )
        })?;
        self.retire_native_drawable_particles_with_services(id, manager);
        Ok(())
    }

    fn retire_native_drawable_particles_with_services(
        &mut self,
        id: Option<DrawableId>,
        manager: &mut crate::effects::particle_manager::ParticleSystemManager,
    ) {
        for &known in &self.drawables.native_drawable_order {
            let drawable = self
                .drawables
                .drawable_map
                .get_mut(&known)
                .expect("native Drawable roster references a live receiver");
            if id.is_some_and(|id| id != known) {
                continue;
            }
            if let Some(drawable) = drawable.as_any_mut().downcast_mut::<BasicDrawable>() {
                drawable.retire_native_state_particles(manager);
            }
        }
    }

    fn update_native_draw_particle_phase(
        &mut self,
        frozen: bool,
        frames: NativeParticleFrames,
        advance_particles: bool,
    ) -> GameClientResult<()> {
        self.update_native_draw_particle_phase_with_materials(
            frozen,
            frames,
            advance_particles,
            &[],
        )
    }

    fn update_native_draw_particle_phase_with_materials(
        &mut self,
        frozen: bool,
        frames: NativeParticleFrames,
        advance_particles: bool,
        materials: &[NativeMaterialObservation],
    ) -> GameClientResult<()> {
        if frames.client_frame != self.frame {
            return Err(GameClientError::InvalidOperation(
                "native material Client clock mismatch".into(),
            ));
        }
        let draw_models = self.admit_native_draw_frame(frozen);
        // Native Main owns this CPU Draw plus the particle update. The
        // retained Core driver instead performs its actual Drawable::render
        // during draw_display below; preparation must not advance it twice.
        if draw_models && advance_particles {
            self.advance_native_material_draw(materials)?;
        }
        let public_bones: Vec<String> = game_engine::common::global_data::read()
            .standard_public_bones
            .iter()
            .map(|bone| bone.as_str().to_owned())
            .collect();
        let assets = crate::render_bridge::THE_RENDER_BRIDGE
            .lock()
            .map_err(|error| {
                GameClientError::SubsystemError(format!("Render assets unavailable: {error}"))
            })?;
        let mut particles = crate::effects::particle_manager::get_particle_system_manager_mut()
            .map_err(|error| {
                GameClientError::SubsystemError(format!("Particle manager unavailable: {error}"))
            })?;
        if let Some(particles) = particles.as_mut() {
            self.run_native_draw_particle_phase_with_services(
                particles,
                assets.as_ref().map(|bridge| bridge.asset_manager()),
                &public_bones,
                frames,
                draw_models,
                advance_particles,
            )?;
        }
        drop(particles);
        drop(assets);
        crate::effects::update_tracer_fx(frames.logic_frame);
        crate::effects::update_ray_effects(frames.logic_frame);
        Ok(())
    }

    fn admit_native_draw_frame(&mut self, frozen: bool) -> bool {
        let repeated = self.last_native_draw_frame == self.frame;
        // C++ records lastFrame before freeze/fast/Draw work, including failures.
        // Reset/load does not reset the Display static observation.
        self.last_native_draw_frame = self.frame;
        !frozen && !repeated
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
                if bindings.contains_key(&id) {
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
                particles.update_with_receiving_drawables_and_objects(
                    self.local_player_id as i32,
                    frames.logic_frame,
                    frames.client_frame,
                    &resolve,
                    lifecycle,
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
}
