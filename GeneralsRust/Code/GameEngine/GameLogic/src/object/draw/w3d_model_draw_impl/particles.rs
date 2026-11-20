// Client particle lifecycle driven by this model and its borrowed Drawable.
// Pose lookup retains the existing skeleton/pristine source during migration.

impl W3DModelDraw {
    fn particle_hidden(&self) -> bool {
        // The model receives the combined explicit/stealth hiding flag.
        // Shroud is a separate particle emission gate; visibility is not hiding.
        self.hidden || self.fully_obscured_by_shroud
    }

    fn current_state_particle_bones(&self) -> Option<Vec<ParticleSysBoneInfo>> {
        let Some(state) = self.current_state() else {
            return None;
        };
        let particle_sys_bones = state.particle_sys_bones.clone();
        if particle_sys_bones.is_empty() {
            return None;
        }
        Some(particle_sys_bones)
    }

    /// Staged deletion uses the same installed model while callbacks run without
    /// its entry guard. Each phase is sampled after the previous callback.
    pub(crate) fn take_particle_systems_for_deletion(&mut self) -> Vec<UnsignedInt> {
        self.native_state_particle_failure = None;
        self.installed_state_particle_row = None;
        self.particle_systems
            .drain(..)
            .map(|tracker| tracker.id)
            .collect()
    }

    fn stop_client_particle_systems(&mut self) {
        self.native_state_particle_failure = None;
        self.installed_state_particle_row = None;
        let Some(ps_manager) = TheParticleSystemManager::get() else {
            self.particle_systems.clear();
            return;
        };
        for tracker in self.particle_systems.drain(..) {
            ps_manager.destroy_particle_system(tracker.id);
        }
    }

    /// Retain the existing skeleton/pristine source during the ownership migration.
    /// Current animated render-object pose is a separate renderer boundary.
    fn live_particle_bone_model_space(
        &self,
        bone_name: &str,
        driver: Option<&crate::object::drawable::DrawableRenderOwner<'_>>,
    ) -> Option<(i32, Matrix3D)> {
        if bone_name.is_empty() {
            return None;
        }
        let scaled_bone = |local: Matrix3D, scale: Real| {
            let scale = if scale.is_finite() && scale > 0.0 {
                scale
            } else {
                1.0
            };
            let index = self
                .current_state()
                .and_then(|state| {
                    state
                        .find_pristine_bone_by_name(bone_name)
                        .map(|(_, bone)| bone.bone_index)
                })
                .unwrap_or(0);
            (index, Matrix3D::from_scale(Coord3D::splat(scale)) * local)
        };
        if let Some(driver) = driver {
            Some(scaled_bone(
                driver.get_bone_local_transform(bone_name)?,
                driver.get_instance_scale(),
            ))
        } else {
            self.with_owner_drawable(|drawable| {
                drawable
                    .get_bone_local_transform(bone_name)
                    .map(|local| scaled_bone(local, drawable.get_instance_scale()))
            })
            .flatten()
        }
    }

    #[cfg(test)]
    fn recalc_bones_for_client_particle_systems(&mut self) {
        self.recalc_bones_for_client_particle_systems_for_owner(None);
    }

    fn recalc_bones_for_client_particle_systems_for_owner(
        &mut self,
        driver: Option<&crate::object::drawable::DrawableRenderOwner<'_>>,
    ) {
        if !self.need_recalc_bone_particle_systems {
            return;
        }

        self.recreate_state_particle_rows(driver);
        // CPP clears the request after all creation callbacks, including misses.
        self.need_recalc_bone_particle_systems = false;
    }

    fn recreate_state_particle_rows(
        &mut self,
        driver: Option<&crate::object::drawable::DrawableRenderOwner<'_>>,
    ) {
        self.recreate_state_particle_rows_with_receiver(driver, None, None)
            .expect("compatibility particle creation does not return errors");
    }

    fn recreate_state_particle_rows_with_receiver(
        &mut self,
        driver: Option<&crate::object::drawable::DrawableRenderOwner<'_>>,
        context: Option<&NativeStateParticleContext>,
        mut receiver: Option<&mut dyn ClientParticleStateRuntime>,
    ) -> Result<(), String> {
        let Some(particle_sys_bones) = self.current_state_particle_bones() else {
            return Ok(());
        };
        let owner = if let Some(context) = context {
            Some((context.drawable_id, !context.state_particles))
        } else if let Some(driver) = driver {
            Some((
                driver.get_drawable_id(),
                driver.test_drawable_status(DRAWABLE_STATUS_NO_STATE_PARTICLES),
            ))
        } else {
            self.with_owner_drawable(|drawable| {
                (
                    drawable.get_drawable_id(),
                    drawable.test_drawable_status(DRAWABLE_STATUS_NO_STATE_PARTICLES),
                )
            })
        };
        let Some((drawable_id, no_state_particles)) = owner else {
            return Ok(());
        };
        if no_state_particles {
            return Ok(());
        }

        // The compatibility branch alone may resolve the historical manager.
        let ps_manager = if receiver.is_none() {
            TheParticleSystemManager::get()
        } else {
            None
        };
        if receiver.is_none() && ps_manager.is_none() {
            return Ok(());
        }

        // C++ does not stop here. setModelState already stopped the old systems.
        // Recalc only creates and appends.

        let hidden = context
            .map(|context| context.hidden)
            .or_else(|| driver.map(|owner| owner.is_drawable_effectively_hidden()))
            .unwrap_or(self.hidden)
            || self.fully_obscured_by_shroud;
        for info in particle_sys_bones.iter() {
            if info.particle_system.is_empty() {
                continue;
            }

            let system_id = if let Some(runtime) = receiver.as_mut() {
                runtime.create_particle_system(
                    info.particle_system.as_str(),
                    context.expect("receiving creation context").creation_frame,
                )?
            } else {
                ps_manager.and_then(|manager| {
                    manager.create_particle_system(Some(info.particle_system.as_str()))
                })
            };
            let Some(system_id) = system_id else {
                continue;
            };

            let model_bone = if let Some(runtime) = receiver.as_mut() {
                let model = self.current_state()
                    .map(|state| state.model_name.clone())
                    .unwrap_or_default();
                // C++ resolves the selected render object's authored name here,
                // independently of the public/pristine-bone cache.
                runtime.model_space_bone(model.as_str(), info.bone_name.as_str())
            } else {
                self.live_particle_bone_model_space(info.bone_name.as_str(), driver)
                    .or_else(|| {
                        self.current_state()
                            .and_then(|state| {
                                state
                                    .find_pristine_bone_by_name(info.bone_name.as_str())
                                    .map(|(_, bone)| (bone.bone_index, bone.transform))
                            })
                    })
            };
            let (bone_index, bone_transform) = model_bone.unwrap_or((0, Matrix3D::IDENTITY));

            if let Some(runtime) = receiver.as_mut() {
                runtime.configure_particle_system(
                    system_id,
                    drawable_id,
                    bone_index,
                    &bone_transform,
                    hidden,
                );
            } else {
                if bone_index != 0 {
                    let position = Self::matrix_translation(&bone_transform);
                    let rotation = Self::matrix_z_rotation(&bone_transform);
                    ps_manager
                        .expect("compatibility particle manager")
                        .set_particle_system_position(system_id, &position);
                    ps_manager
                        .expect("compatibility particle manager")
                        .rotate_particle_system_local_transform_z(system_id, rotation);
                } else {
                    ps_manager
                        .expect("compatibility particle manager")
                        .set_particle_system_position(system_id, &Coord3D::origin());
                }

                ps_manager
                    .expect("compatibility particle manager")
                    .attach_particle_system_to_drawable(system_id, drawable_id);
                ps_manager
                    .expect("compatibility particle manager")
                    .set_particle_system_saveable(system_id, false);
                if hidden {
                    ps_manager
                        .expect("compatibility particle manager")
                        .stop_particle_system(system_id);
                }
            }
            self.particle_systems.push(ParticleSysTracker {
                id: system_id,
                bone_index,
                bone_name: info.bone_name.clone(),
                installed_row: None,
            });
        }
        Ok(())
    }

    /// Inspect the installed runtime rather than a mirrored count or query clone.
    pub fn native_state_particle_count(&self) -> usize {
        self.particle_systems.len()
    }

    /// Match the separate visibility gate even when Draw returns before state work.
    pub fn set_native_particle_visibility(
        &mut self,
        hidden: bool,
        fully_obscured: bool,
        receiver: &mut dyn ClientParticleStateRuntime,
    ) {
        let was_hidden = self.particle_hidden();
        self.hidden = hidden;
        self.fully_obscured_by_shroud = fully_obscured;
        let hidden = self.particle_hidden();
        if was_hidden != hidden {
            for tracker in &self.particle_systems {
                receiver.set_particle_system_running(tracker.id, !hidden);
            }
        }
    }

    /// Retire only this admitted module's systems at the receiving destruction phase.
    /// The receiving manager determines original deferred destruction semantics.
    pub fn retire_native_state_particles(&mut self, receiver: &mut dyn ClientParticleStateRuntime) {
        for id in self.take_particle_systems_for_deletion() {
            receiver.destroy_particle_system(id);
        }
    }

    /// Transition-bearing emitters use the receiving installed pose. TrackMarks
    /// and animation controls outside this path retain their source requirements.
    pub fn accepts_installed_state_particles(&self) -> bool {
        let data = self.model_data();
        data.track_file.is_empty()
            && !data.transition_states.is_empty()
            && data.transition_states.iter().any(|state| !state.animations.is_empty())
            && data.condition_states.iter().chain(&data.transition_states).any(|state| {
                !state.particle_sys_bones.is_empty()
            })
            && data.condition_states.iter().chain(&data.transition_states).all(|state| {
                state.animations.len() <= 1
                    && state.flags
                        & ((1 << ACBIT_RANDOMSTART)
                            | (1 << ACBIT_RESTART_ANIM_WHEN_COMPLETE)
                            | ALL_MAINTAIN_FRAME_FLAGS)
                        == 0
                    && state.anim_min_speed_factor.is_finite()
                    && state.anim_max_speed_factor.is_finite()
                    && state.anim_max_speed_factor <= state.anim_min_speed_factor
                    && state.weapon_recoil_bone.iter().all(|bone| bone.is_empty())
            })
    }

    /// C++ setModelState retires the old trackers; doDrawModule creates the
    /// current row when its installed hierarchy is ready.
    pub fn receive_installed_state_particles(
        &mut self,
        state: u32,
        rows: Option<&[InstalledStateParticleRow]>,
        context: &NativeStateParticleContext,
        receiver: &mut dyn ClientParticleStateRuntime,
    ) -> Result<bool, String> {
        if !self.accepts_installed_state_particles() {
            return Err("installed particle pose belongs to an unsupported Draw module".into());
        }
        let follows_animation = self.model_data().particles_attached_to_animated_bones;
        if self.installed_state_particle_row == Some(state) {
            self.set_native_particle_visibility(context.hidden, context.fully_obscured, receiver);
            if let Some(error) = &self.native_state_particle_failure {
                return Err(error.clone());
            }
            if follows_animation && !context.hidden && !context.fully_obscured
                && self.particle_systems.iter().any(|tracker| {
                    tracker.bone_index != 0 && receiver.particle_system_exists(tracker.id)
                })
            {
                let Some(rows) = rows else { return Ok(true) };
                self.follow_installed_particle_bones(rows, receiver);
            }
            return Ok(false);
        }
        self.retire_native_state_particles(receiver);
        self.set_native_particle_visibility(context.hidden, context.fully_obscured, receiver);
        if context.hidden || context.fully_obscured {
            return Ok(false);
        }
        if !context.state_particles {
            self.installed_state_particle_row = Some(state);
            return Ok(false);
        }
        let Some(rows) = rows else {
            return Ok(true);
        };
        self.installed_state_particle_row = Some(state);
        for (row_index, row) in rows.iter().enumerate() {
            let id = match receiver.create_particle_system(&row.template, context.creation_frame) {
                Ok(Some(id)) => id,
                Ok(None) => continue,
                Err(error) => {
                    self.native_state_particle_failure = Some(error.clone());
                    return Err(error);
                }
            };
            let (bone_index, local) = row.bone.unwrap_or((0, Matrix3D::IDENTITY));
            receiver.configure_particle_system(id, context.drawable_id, bone_index, &local, false);
            self.particle_systems.push(ParticleSysTracker {
                id,
                bone_index,
                bone_name: AsciiString::from(row.bone_name.as_str()),
                installed_row: Some(row_index),
            });
        }
        if follows_animation {
            self.follow_installed_particle_bones(rows, receiver);
        }
        Ok(false)
    }

    fn follow_installed_particle_bones(
        &self,
        rows: &[InstalledStateParticleRow],
        receiver: &mut dyn ClientParticleStateRuntime,
    ) {
        // W3DModelDraw.cpp:2080–2086 creates all trackers before following
        // them, then applies recoil. Missing bones keep the birth offset.
        for tracker in &self.particle_systems {
            if tracker.bone_index == 0 || !receiver.particle_system_exists(tracker.id) {
                continue;
            }
            let Some(row) = tracker.installed_row.and_then(|index| rows.get(index)) else {
                continue;
            };
            if let Some(world) = row.world_bone {
                receiver.follow_particle_bone(
                    tracker.id,
                    &Self::matrix_translation(&world),
                    Self::matrix_z_rotation(&world),
                    &world,
                );
            }
        }
    }

    /// Selected static-model continuation at the real receiving Draw phase.
    /// Unsupported animation/terrain routes are explicit obligations; never
    /// invoke their ambient services through a seemingly owned particle route.
    pub fn prepare_state_particles_with_receiver(
        &mut self,
        context: &NativeStateParticleContext,
        receiver: &mut dyn ClientParticleStateRuntime,
    ) -> Result<NativeStateParticlePreparation, String> {
        {
            let data = self.model_data();
            let terrain_tracks = !data.track_file.is_empty();
            let animation_pose = data
                .condition_states
                .iter()
                .any(|state| !state.animations.is_empty())
                || data
                    .transition_states
                    .iter()
                    .any(|state| !state.animations.is_empty());
            if animation_pose || terrain_tracks {
                // Unrelated unsupported animation/terrain models must retain their
                // existing WGPU route. Static models still select state because
                // their ObjectDraw may answer a later ordered bone query.
                if self.particle_systems.is_empty()
                    && data
                        .condition_states
                        .iter()
                        .all(|state| state.particle_sys_bones.is_empty())
                    && data
                        .transition_states
                        .iter()
                        .all(|state| state.particle_sys_bones.is_empty())
                {
                    return Ok(NativeStateParticlePreparation::Prepared(false));
                }
                return Ok(NativeStateParticlePreparation::Unsupported {
                    animation_pose,
                    terrain_tracks,
                });
            }
        }
        // Drawable's original early return permits visibility synchronization
        // but no state selection, cache lookup, RNG, or emitter birth.
        if context.hidden || context.fully_obscured {
            self.set_native_particle_visibility(context.hidden, context.fully_obscured, receiver);
            if let Some(error) = &self.native_state_particle_failure {
                return Err(error.clone());
            }
            return Ok(NativeStateParticlePreparation::Prepared(false));
        }
        let conditions = self.apply_pending_carrying(context.conditions);
        self.last_model_conditions = conditions;
        self.hide_headlights = !conditions.contains(ModelConditionFlags::NIGHT);
        let Some(state_index) = self.find_best_state_index(&conditions) else {
            return Ok(NativeStateParticlePreparation::Prepared(false));
        };
        if let Some(error) = &self.native_state_particle_failure {
            let unchanged = (self.cur_state == Some(ActiveModelState::Condition(state_index))
                && self.next_state.is_none())
                || self.next_state == Some(state_index);
            if unchanged {
                return Err(error.clone());
            }
        }
        let facts = ModelDrawContext {
            instance_scale: context.instance_scale,
            state_particles: context.state_particles,
        };
        self.set_model_state_with_receiver(state_index, Some(&facts), Some(&mut *receiver));
        self.hide_all_headlights_with(&mut |model| receiver.sub_object_names(model));
        self.set_native_particle_visibility(context.hidden, context.fully_obscured, receiver);
        if self.need_recalc_bone_particle_systems {
            if let Err(error) =
                self.recreate_state_particle_rows_with_receiver(None, Some(context), Some(receiver))
            {
                self.native_state_particle_failure = Some(error.clone());
                return Err(error);
            }
            self.need_recalc_bone_particle_systems = false;
        }
        Ok(NativeStateParticlePreparation::Prepared(true))
    }

    pub fn update_bones_for_client_particle_systems(&mut self) -> bool {
        let Some((_, _, drawable)) = self.owner_drawable_handles() else {
            return true;
        };
        if self.current_state().is_none()
            || self
                .current_state()
                .is_some_and(|state| state.model_name.as_str().is_empty())
        {
            return true;
        }

        let Ok(drawable_guard) = drawable.read() else {
            return true;
        };
        self.update_particle_bone_transforms(|name| {
            drawable_guard.get_current_worldspace_client_bone_positions(name)
        })
    }

    fn update_particle_bones_for_owner(
        &self,
        owner: &crate::object::drawable::DrawableRenderOwner<'_>,
    ) -> bool {
        if self.current_state().is_none()
            || self
                .current_state()
                .is_some_and(|state| state.model_name.as_str().is_empty())
        {
            return true;
        }

        self.update_particle_bone_transforms(|name| {
            owner.get_current_worldspace_client_bone_positions(self, name)
        })
    }

    fn update_particle_bone_transforms(
        &self,
        mut query: impl FnMut(&str) -> Option<Matrix3D>,
    ) -> bool {
        let Some(ps_manager) = TheParticleSystemManager::get() else {
            return true;
        };

        for tracker in &self.particle_systems {
            if tracker.bone_index == 0 || tracker.bone_name.is_empty() {
                continue;
            }

            if ps_manager.find_particle_system(tracker.id).is_none() {
                continue;
            }

            if let Some(transform) = query(tracker.bone_name.as_str()) {
                let position = Self::matrix_translation(&transform);
                let orientation = Self::matrix_z_rotation(&transform);
                ps_manager.set_particle_system_position(tracker.id, &position);
                ps_manager.rotate_particle_system_local_transform_z(tracker.id, orientation);
                ps_manager.set_particle_system_transform(tracker.id, &transform);
                ps_manager.set_particle_system_skip_parent_xfrm(tracker.id, true);
            }
        }

        true
    }

    /// C++ W3DModelDraw.cpp2653: update only this model's admitted trackers.
    /// A model without a render state still consumes the first-success slot.
    pub fn update_particle_bones_with_receiver(
        &self,
        receiver: &mut dyn ClientParticleBoneRuntime,
    ) -> bool {
        let Some(model) = self.current_state().map(|state| state.model_name.clone()) else {
            return true;
        };
        if model.as_str().is_empty() {
            return true;
        }
        for tracker in &self.particle_systems {
            // C++ checks the particle before querying its bone. Missing systems
            // cannot consume a query or mutate another receiving manager.
            if !receiver.particle_system_exists(tracker.id) {
                continue;
            }
            if tracker.bone_index == 0 {
                continue;
            }
            if let Some(transform) = receiver.bone_transform(
                model.as_str(),
                tracker.bone_index,
                tracker.bone_name.as_str(),
            ) {
                receiver.set_particle_bone_transform(
                    tracker.id,
                    &Self::matrix_translation(&transform),
                    Self::matrix_z_rotation(&transform),
                    &transform,
                );
            }
        }
        true
    }

    fn do_start_or_stop_particle_sys(&self) {
        let hidden = self.particle_hidden();
        let Some(ps_manager) = TheParticleSystemManager::get() else {
            return;
        };
        for tracker in &self.particle_systems {
            if hidden {
                ps_manager.stop_particle_system(tracker.id);
            } else {
                ps_manager.start_particle_system(tracker.id);
            }
        }
    }
}
