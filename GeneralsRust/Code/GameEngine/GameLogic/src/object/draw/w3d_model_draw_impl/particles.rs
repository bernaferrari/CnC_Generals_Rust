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
        self.particle_systems
            .drain(..)
            .map(|tracker| tracker.id)
            .collect()
    }

    fn stop_client_particle_systems(&mut self) {
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
                .and_then(|state| state.find_pristine_bone_by_name(bone_name))
                .map(|(_, bone)| bone.bone_index)
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
        let Some(particle_sys_bones) = self.current_state_particle_bones() else {
            return;
        };
        let owner = if let Some(driver) = driver {
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
            return;
        };
        if no_state_particles {
            return;
        }

        let Some(ps_manager) = TheParticleSystemManager::get() else {
            return;
        };

        // C++ does not stop here. setModelState already stopped the old systems.
        // Recalc only creates and appends.

        let hidden = driver
            .map(|owner| owner.is_drawable_effectively_hidden())
            .unwrap_or(self.hidden)
            || self.fully_obscured_by_shroud;
        for info in particle_sys_bones.iter() {
            if info.particle_system.is_empty() {
                continue;
            }

            let Some(system_id) =
                ps_manager.create_particle_system(Some(info.particle_system.as_str()))
            else {
                continue;
            };

            let (bone_index, bone_transform) = self
                .live_particle_bone_model_space(info.bone_name.as_str(), driver)
                .or_else(|| {
                    self.current_state()
                        .and_then(|state| state.find_pristine_bone_by_name(info.bone_name.as_str()))
                        .map(|(_, bone)| (bone.bone_index, bone.transform))
                })
                .unwrap_or((0, Matrix3D::IDENTITY));

            if bone_index != 0 {
                let position = Self::matrix_translation(&bone_transform);
                let rotation = Self::matrix_z_rotation(&bone_transform);
                ps_manager.set_particle_system_position(system_id, &position);
                ps_manager.rotate_particle_system_local_transform_z(system_id, rotation);
            } else {
                ps_manager.set_particle_system_position(system_id, &Coord3D::origin());
            }

            ps_manager.attach_particle_system_to_drawable(system_id, drawable_id);
            ps_manager.set_particle_system_saveable(system_id, false);
            if hidden {
                ps_manager.stop_particle_system(system_id);
            }
            self.particle_systems.push(ParticleSysTracker {
                id: system_id,
                bone_index,
                bone_name: info.bone_name.clone(),
            });
        }
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
