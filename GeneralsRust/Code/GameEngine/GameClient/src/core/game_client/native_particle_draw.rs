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
        // Preflight before erasing diagnostics or reaching any service.
        let required = self.drawables.native_particle_retirement_required(id)?;
        if !required {
            self.native_particle_capability_gaps
                .retain(|gap| id.is_some_and(|id| gap.drawable_id != id));
            return Ok(());
        }
        self.drawables
            .state
            .retire_native_particles_with_services(id, &mut self.drawables.particles)?;
        self.native_particle_capability_gaps
            .retain(|gap| id.is_some_and(|id| gap.drawable_id != id));
        Ok(())
    }

    fn retire_native_drawable_particles_with_services(
        &mut self,
        id: Option<DrawableId>,
        manager: &mut crate::effects::particle_manager::ParticleSystemManager,
    ) {
        // Existing internal test callers use already admitted owned modules.
        // Production fallible boundaries invoke the same owner kernel directly.
        self.drawables
            .retire_native_particles_with_services(id, manager)
            .expect("receiving native particle retirement must preserve module ownership");
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
        let assets = self.render_bridge.asset_manager();
        self.drawables
            .particles
            .admit_effective_definitions()
            .map_err(|error| GameClientError::SubsystemError(error.to_string()))?;
        let (state, particles) = (&mut self.drawables.state, &mut self.drawables.particles);
        NativeRenderReceiver::new(
            state,
            &mut self.native_particle_capability_gaps,
            &self.subsystem_manager,
            self.frame,
            self.local_player_id,
        )
        .run_native_draw_particle_phase_with_services(
            particles,
            Some(assets),
            &public_bones,
            frames,
            draw_models,
            advance_particles,
        )?;
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
        self.native_render_receiver()
            .run_native_draw_particle_phase_with_services(
                particles,
                assets,
                public_bones,
                frames,
                draw_models,
                advance_particles,
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
        self.native_render_receiver()
            .run_native_draw_particle_phase_with_object_lifecycle(
                particles,
                assets,
                public_bones,
                frames,
                draw_models,
                advance_particles,
                lifecycle,
            )
    }

    fn prepare_native_draw_particles_with_assets(
        &mut self,
        particles: &mut crate::effects::particle_manager::ParticleSystemManager,
        assets: Option<&ww3d_assets::AssetManager>,
        public_bones: &[String],
        frame: u32,
    ) -> GameClientResult<()> {
        self.native_render_receiver()
            .prepare_native_draw_particles_with_assets(particles, assets, public_bones, frame)
    }

    fn prepare_native_drawable_particles_with_assets(
        &mut self,
        id: DrawableId,
        particles: &mut crate::effects::particle_manager::ParticleSystemManager,
        assets: Option<&ww3d_assets::AssetManager>,
        public_bones: &[String],
        frame: u32,
    ) -> GameClientResult<()> {
        self.native_render_receiver()
            .prepare_native_drawable_particles_with_assets(
                id,
                particles,
                assets,
                public_bones,
                frame,
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
        self.native_render_receiver()
            .prepare_native_drawable_particles_with_assets_and_keys(
                id,
                particles,
                assets,
                public_bones,
                frame,
                keys,
            )
    }
}

impl drawable_world::DrawableState {
    /// Same preflight as physical Drawable deletion, over actual native roster.
    /// No flush/update, no ID unlink, and no manager discovery occurs here.
    fn native_particle_retirement_required(
        &mut self,
        id: Option<DrawableId>,
    ) -> GameClientResult<bool> {
        let mut required = false;
        for &known in &self.native_drawable_order {
            if id.is_some_and(|id| id != known) {
                continue;
            }
            let draw = self
                .drawable_map
                .get_mut(&known)
                .expect("native Drawable roster references a live receiver");
            if let Some(basic) = draw.as_any_mut().downcast_mut::<BasicDrawable>() {
                required |= basic.native_state_particle_count() != 0;
                required |= basic
                    .native_client_particle_retirement_required()
                    .map_err(GameClientError::InvalidOperation)?;
            }
        }
        Ok(required)
    }

    fn retire_native_particles_with_services(
        &mut self,
        id: Option<DrawableId>,
        manager: &mut crate::effects::particle_manager::ParticleSystemManager,
    ) -> GameClientResult<()> {
        // Preflight ALL selected receivers before the first external effect.
        self.native_particle_retirement_required(id)?;
        for &known in &self.native_drawable_order {
            if id.is_some_and(|id| id != known) {
                continue;
            }
            let draw = self
                .drawable_map
                .get_mut(&known)
                .expect("native Drawable roster references a live receiver");
            if let Some(basic) = draw.as_any_mut().downcast_mut::<BasicDrawable>() {
                basic.retire_native_state_particles(manager);
                basic
                    .retire_native_client_particles(manager)
                    .map_err(GameClientError::InvalidOperation)?;
            }
        }
        Ok(())
    }

    fn try_remove_for_map_attempt(
        &mut self,
        id: DrawableId,
        mut particles: Option<&mut crate::effects::particle_manager::ParticleSystemManager>,
    ) -> GameClientResult<()> {
        let required = self.native_particle_retirement_required(Some(id))?;
        if required && particles.is_none() {
            return Err(GameClientError::SubsystemError(
                "receiving particle manager absent during failed Drawable retirement".into(),
            ));
        }
        let Some(mut removed) = self.drawable_map.remove(&id) else {
            return Ok(());
        };
        self.native_drawable_order.retain(|known| *known != id);
        self.owned_laser_order.retain(|known| *known != id);
        if let Some(object) = removed.get_object_id() {
            if self.drawable_object_map.get(&object).copied() == Some(id) {
                self.drawable_object_map.remove(&object);
            }
        }
        self.presentation_direct_drawable_bindings.remove(&id);
        self.text_bearing_drawables.retain(|known| *known != id);
        let result = if let Some(basic) = removed.as_any_mut().downcast_mut::<BasicDrawable>() {
            let result = if let Some(manager) = particles.as_deref_mut() {
                basic.retire_native_state_particles(manager);
                basic
                    .retire_native_client_particles(manager)
                    .map_err(GameClientError::InvalidOperation)
            } else {
                Ok(())
            };
            // C++ Drawable destructor: module deletion before ambient stop.
            // Finish even if an unexpected post-effect retirement error occurs.
            basic.finish_native_retirement();
            result
        } else {
            Ok(())
        };
        self.ray_effects.delete(id);
        drop(removed);
        result
    }

    fn try_clear_for_map_attempt(
        &mut self,
        mut particles: Option<&mut crate::effects::particle_manager::ParticleSystemManager>,
    ) -> GameClientResult<()> {
        let required = self.native_particle_retirement_required(None)?;
        if required && particles.is_none() {
            return Err(GameClientError::SubsystemError(
                "receiving particle manager absent during map-attempt retirement".into(),
            ));
        }
        let order = self.native_drawable_order.clone();
        let mut first_error = None;
        for known in order {
            if let Err(error) = self.try_remove_for_map_attempt(known, particles.as_deref_mut()) {
                first_error.get_or_insert(error);
            }
        }
        debug_assert!(self.drawable_map.is_empty());
        self.ray_effects.reset();
        self.projected_decals.manager.clear();
        self.next_drawable_id = DrawableId(1);
        first_error.map_or(Ok(()), Err)
    }

    fn finish_native_retirement(&mut self) {
        for &known in &self.native_drawable_order {
            let draw = self
                .drawable_map
                .get_mut(&known)
                .expect("native Drawable roster references a live receiver");
            if let Some(basic) = draw.as_any_mut().downcast_mut::<BasicDrawable>() {
                basic.finish_native_retirement();
            }
        }
    }
}
impl NativeDrawableOwner<'_> {
    /// Original new-level callback over actual prepend Drawable roster.
    /// Saved-level admission must skip this operation at its driving boundary.
    pub fn on_level_start(&mut self) -> GameClientResult<()> {
        for &id in &self.drawables.native_drawable_order {
            let drawable = self
                .drawables
                .drawable_map
                .get_mut(&id)
                .expect("native Drawable roster references a live receiver");
            let basic = drawable
                .as_any_mut()
                .downcast_mut::<BasicDrawable>()
                .ok_or_else(|| {
                    GameClientError::InvalidOperation(
                        "native onLevelStart requires its actual BasicDrawable receiver".into(),
                    )
                })?;
            basic.on_level_start();
        }
        Ok(())
    }

    pub fn on_level_start_at(&mut self, tod: crate::system::TimeOfDay) -> GameClientResult<()> {
        for &id in &self.drawables.native_drawable_order {
            let drawable = self
                .drawables
                .drawable_map
                .get_mut(&id)
                .expect("native Drawable roster references a live receiver");
            let basic = drawable
                .as_any_mut()
                .downcast_mut::<BasicDrawable>()
                .ok_or_else(|| {
                    GameClientError::InvalidOperation(
                        "native onLevelStart requires its actual BasicDrawable receiver".into(),
                    )
                })?;
            basic.on_level_start_at(tod);
        }
        Ok(())
    }

    /// Failed constructor retirement selects only its actual Drawable identity.
    /// Required service validation precedes unlink; unrelated peers survive.
    pub fn try_remove_for_map_attempt(
        &mut self,
        id: DrawableId,
        particles: Option<&mut crate::effects::particle_manager::ParticleSystemManager>,
    ) -> GameClientResult<()> {
        self.drawables.try_remove_for_map_attempt(id, particles)
    }

    /// Retire the actual detached map world with an explicit receiving manager.
    /// Missing required services fail before any identity is erased.
    pub fn try_clear_for_map_attempt(
        &mut self,
        particles: Option<&mut crate::effects::particle_manager::ParticleSystemManager>,
    ) -> GameClientResult<()> {
        self.drawables.try_clear_for_map_attempt(particles)?;
        self.particles.reset_runtime();
        Ok(())
    }
}

#[cfg(test)]
mod missing_binding_retirement_tests {
    use super::*;
    #[test]
    fn actual_client_load_postprocess_keeps_cpu_actor_when_binding_disappears() {
        #[cfg(not(target_arch = "wasm32"))]
        if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
            module_path!(),
            "actual_client_load_postprocess_keeps_cpu_actor_when_binding_disappears",
        ) {
            return;
        }
        let mut client = GameClient::new().unwrap();
        let mut peer = GameClient::new().unwrap();
        assert!(!client.drawables.borrow_native().has_object_drawable(42));
        let entry = super::tests::presentation_drawable_sync_for_test(
            42,
            91,
            "LoadedUnboundOwner",
            true,
            false,
            [0.; 3],
            0.,
        );
        client.sync_presentation_drawables([entry.clone()]).unwrap();
        peer.sync_presentation_drawables([entry]).unwrap();
        let key = client
            .drawables
            .borrow_native()
            .binding_key(91, 42)
            .unwrap();
        let peer_key = peer.drawables.borrow_native().binding_key(91, 42).unwrap();
        client.load_post_process().unwrap();
        assert!(
            client
                .drawables
                .borrow_native()
                .binding_key(91, 42)
                .is_none()
        );
        assert!(
            client.drawables.borrow_native().has_object_drawable(42),
            "decoded-unbound actor is not genuine prebirth absence"
        );
        assert!(client.find_drawable_by_id(key.drawable_id).is_some());
        assert_eq!(
            peer.drawables.borrow_native().binding_key(91, 42),
            Some(peer_key)
        );
        assert!(peer.drawables.borrow_native().has_object_drawable(42));
        client.drawables.drawable_object_map.remove(&42);
        assert!(
            client.drawables.borrow_native().has_object_drawable(42),
            "inconsistent inverse map cannot hide an actual bound CPU node"
        );
        client.invalidate_presentation_drawable_world();
        assert!(!client.drawables.borrow_native().has_object_drawable(42));
        peer.invalidate_presentation_drawable_world();
    }
}
