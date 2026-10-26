//! Bounded access to the actual installed Draw state during synchronous callbacks.
//! This is a field loan, not another renderer, animation clock or pose cache.
use super::*;
use installed_draw_pose::{NativeCameraPoseUnavailable, ObjectVisualState};

pub(crate) struct NativeInstalledPoseLoan<'a> {
    pub(super) native_drawable_lifetimes:
        &'a mut HashMap<u32, native_draw_world::NativeDrawableLifetime>,
    pub(super) live_draw_playback:
        &'a mut HashMap<crate::assets::LiveDrawPlaybackKey, crate::assets::LiveDrawPlayback>,
    pub(super) drawable_visual_states: &'a mut HashMap<(u32, u32), ObjectVisualState>,
    pub(super) failed_model_resolutions: &'a mut pipeline_collect::FailedModelResolutions,
    pub(super) pending_client_drawable_restore:
        &'a mut Option<crate::save_load::snapshot::ClientDrawableWorldSnapshot>,
    pub(super) pending_client_drawable_imports:
        &'a mut HashMap<(u32, u32), crate::save_load::snapshot::ClientDrawableStateSnapshot>,
    pub(super) animation_sync_ms: u32,
}
impl RenderPipeline {
    #[cfg(feature = "game_client")]
    pub(crate) fn admit_native_drawable_binding(
        &mut self,
        identity: &crate::assets::LiveDrawPlaybackIdentity,
        key: game_client::core::PresentationDirectDrawableBindingKey,
    ) -> Result<(), &'static str> {
        self.borrow_native_installed_poses()
            .admit_binding(identity, key)
    }

    pub(crate) fn borrow_native_installed_poses(&mut self) -> NativeInstalledPoseLoan<'_> {
        self.native_draw_world
            .borrow_installed_poses(&mut self.failed_model_resolutions, self.animation_sync_ms)
    }
}
impl NativeInstalledPoseLoan<'_> {
    pub(crate) fn reborrow(&mut self) -> NativeInstalledPoseLoan<'_> {
        NativeInstalledPoseLoan {
            native_drawable_lifetimes: &mut *self.native_drawable_lifetimes,
            live_draw_playback: &mut *self.live_draw_playback,
            drawable_visual_states: &mut *self.drawable_visual_states,
            failed_model_resolutions: &mut *self.failed_model_resolutions,
            pending_client_drawable_restore: &mut *self.pending_client_drawable_restore,
            pending_client_drawable_imports: &mut *self.pending_client_drawable_imports,
            animation_sync_ms: self.animation_sync_ms,
        }
    }
    pub(crate) fn has_drawable_lifetime(&self, object: u32) -> bool {
        self.native_drawable_lifetimes.contains_key(&object)
    }

    pub(crate) fn current_animation_sync_ms(&self) -> u32 {
        self.animation_sync_ms
    }
    /// Stamp the actual post-sync Client lifetime on the existing Draw owner.
    /// A changed key requires a real Destroy/Creating producer, never relabeling.
    #[cfg(feature = "game_client")]
    pub(crate) fn admit_binding(
        &mut self,
        identity: &crate::assets::LiveDrawPlaybackIdentity,
        key: game_client::core::PresentationDirectDrawableBindingKey,
    ) -> Result<(), &'static str> {
        if key.object_id != identity.object_id() {
            return Err("Client binding object mismatch");
        }
        let lifetime = self
            .native_drawable_lifetimes
            .get(&identity.object_id())
            .ok_or("Client binding has no actual native Drawable constructor")?;
        if lifetime.identity != *identity
            || lifetime.binding.is_some_and(|installed| installed != key)
        {
            return Err("Client binding belongs to another native Drawable constructor");
        }
        if self.drawable_visual_states.iter().any(|((id, _), state)| {
            *id == identity.object_id()
                && (state.native_host_identity.as_ref() != Some(identity)
                    || state
                        .native_client_binding
                        .is_some_and(|installed| installed != key))
        }) {
            return Err("Client binding did not reach actual native constructor owner");
        }
        self.native_drawable_lifetimes
            .get_mut(&identity.object_id())
            .unwrap()
            .binding = Some(key);
        for ((id, _), state) in &mut *self.drawable_visual_states {
            if *id == identity.object_id() {
                state.native_client_binding = Some(key);
            }
        }
        Ok(())
    }
    /// Validate the actual object constructor even when it has no Draw modules.
    #[cfg(feature = "game_client")]
    pub(crate) fn validate_object_retirement(
        &self,
        identity: &crate::assets::LiveDrawPlaybackIdentity,
        key: game_client::core::PresentationDirectDrawableBindingKey,
    ) -> Result<(), &'static str> {
        let lifetime = self
            .native_drawable_lifetimes
            .get(&identity.object_id())
            .ok_or("native physical retirement has no actual constructor")?;
        if key.object_id != identity.object_id()
            || lifetime.identity != *identity
            || lifetime.binding != Some(key)
        {
            return Err("native physical retirement identity/binding mismatch");
        }
        if self
            .drawable_visual_states
            .iter()
            .any(|((object, _), state)| {
                *object == identity.object_id()
                    && (state.native_host_identity.as_ref() != Some(identity)
                        || state.native_client_binding != Some(key))
            })
        {
            // Diagnose the first anonymous/stale module without mutating the owner.
            let mut invalid = self
                .drawable_visual_states
                .iter()
                .filter(|((object, _), state)| {
                    *object == identity.object_id()
                        && (state.native_host_identity.as_ref() != Some(identity)
                            || state.native_client_binding != Some(key))
                })
                .collect::<Vec<_>>();
            invalid.sort_by_key(|((_, module), _)| *module);
            for ((object, module), state) in invalid.into_iter().take(8) {
                log::error!(
                    "native retirement module mismatch object={} module={} template={} host_identity_present={} host_identity_matches={} binding={:?} expected_binding={:?} pending_model={} selected_model={}",
                    object,
                    module,
                    state.native_source_template,
                    state.native_host_identity.is_some(),
                    state.native_host_identity.as_ref() == Some(identity),
                    state.native_client_binding,
                    key,
                    state.native_pending_condition.is_some(),
                    state.native_selected_condition.is_some()
                );
            }
            return Err("native physical retirement module identity/binding mismatch");
        }
        Ok(())
    }
    #[cfg(feature = "game_client")]
    pub(crate) fn retire_object_after_client(
        &mut self,
        identity: &crate::assets::LiveDrawPlaybackIdentity,
        key: game_client::core::PresentationDirectDrawableBindingKey,
    ) -> Result<(), &'static str> {
        self.validate_object_retirement(identity, key)?;
        self.retire_native_drawable(crate::game_logic::ObjectId(identity.object_id()));
        Ok(())
    }
    #[cfg(feature = "game_client")]
    pub(super) fn validate_binding(
        &self,
        identity: &crate::assets::LiveDrawPlaybackIdentity,
        key: game_client::core::PresentationDirectDrawableBindingKey,
        module_index: u32,
    ) -> Result<(), NativeCameraPoseUnavailable> {
        self.validate_object_retirement(identity, key)
            .map_err(|_| NativeCameraPoseUnavailable::UnadmittedDrawableLifecycle)?;
        self.validate_identity(identity, module_index)?;
        if key.object_id != identity.object_id()
            || self.drawable_visual_states.iter().any(|((id, _), state)| {
                *id == identity.object_id() && state.native_client_binding != Some(key)
            })
        {
            return Err(NativeCameraPoseUnavailable::UnadmittedDrawableLifecycle);
        }
        Ok(())
    }
    pub(super) fn validate_identity(
        &self,
        identity: &crate::assets::LiveDrawPlaybackIdentity,
        module_index: u32,
    ) -> Result<(), NativeCameraPoseUnavailable> {
        let id = identity.object_id();
        if self
            .native_drawable_lifetimes
            .get(&id)
            .is_none_or(|lifetime| lifetime.identity != *identity)
        {
            return Err(NativeCameraPoseUnavailable::UnadmittedDrawableLifecycle);
        }
        let state = self
            .drawable_visual_states
            .get(&(id, module_index))
            .ok_or(NativeCameraPoseUnavailable::UnadmittedDrawableLifecycle)?;
        if state.native_host_identity.as_ref() != Some(identity)
            || self
                .drawable_visual_states
                .iter()
                .any(|((object, _), state)| {
                    *object == id && state.native_host_identity.as_ref() != Some(identity)
                })
        {
            return Err(NativeCameraPoseUnavailable::UnadmittedDrawableLifecycle);
        }
        Ok(())
    }
    /// Current host values reach the same transform and dirty-selection
    /// producers as display sync, without synthesizing a missing lifecycle.
    #[cfg(feature = "game_client")]
    pub(crate) fn sample_current_source(
        &mut self,
        source: &crate::game_logic::NativeSlaveSource,
        key: game_client::core::PresentationDirectDrawableBindingKey,
        bone_name: &str,
        graphics: &mut GraphicsSystem,
    ) -> Result<Mat4, NativeCameraPoseUnavailable> {
        let identity = &source.identity;
        let first = source
            .receiving_draw_models
            .first_object_draw(source.condition_bits)
            .map_err(|_| NativeCameraPoseUnavailable::UnsupportedSourceControls)?
            .ok_or(NativeCameraPoseUnavailable::UnsupportedSourceControls)?;
        self.validate_binding(identity, key, first.module_index)?;
        self.receive_current_bone_source(source, key, graphics)?;
        self.installed_camera_pose_for(identity, key, first.module_index, bone_name)
    }
    #[cfg(feature = "game_client")]
    fn receive_current_bone_source(
        &mut self,
        source: &crate::game_logic::NativeSlaveSource,
        key: game_client::core::PresentationDirectDrawableBindingKey,
        graphics: &mut GraphicsSystem,
    ) -> Result<Vec<u32>, NativeCameraPoseUnavailable> {
        let identity = &source.identity;
        let models = source
            .receiving_draw_models
            .object_draws(source.condition_bits)
            .map_err(|_| NativeCameraPoseUnavailable::UnsupportedSourceControls)?;
        if let Some(first) = models.first() {
            self.validate_binding(identity, key, first.module_index)?;
        } else {
            self.validate_object_retirement(identity, key)
                .map_err(|_| NativeCameraPoseUnavailable::UnadmittedDrawableLifecycle)?;
        }
        let module_indices = models.iter().map(|model| model.module_index).collect();
        // Querying can consume current dirty state, but cannot invent the
        // constructor/receiving association that C++ already created.
        if models.iter().any(|model| {
            !self
                .drawable_visual_states
                .contains_key(&(identity.object_id(), model.module_index))
        }) || self
            .drawable_visual_states
            .iter()
            .any(|((object, _), state)| {
                *object == identity.object_id()
                    && state.native_source_template != source.template_name
            })
        {
            return Err(NativeCameraPoseUnavailable::UnadmittedDrawableLifecycle);
        }
        for ((object, _), state) in &mut *self.drawable_visual_states {
            if *object == identity.object_id() {
                state.receive_native_transform_observations(
                    identity,
                    source.mesh_scale,
                    source.float_yaw,
                    source.float_pitch,
                    source.under_construction,
                    source.construction_percent,
                    source.max_height_above_position,
                );
            }
        }
        for model in models {
            let state = self
                .drawable_visual_states
                .get_mut(&(identity.object_id(), model.module_index))
                .ok_or(NativeCameraPoseUnavailable::UnadmittedDrawableLifecycle)?;
            state.mark_native_condition(source.condition_bits, model);
        }
        self.consume_native_camera_conditions(
            crate::game_logic::ObjectId(identity.object_id()),
            graphics,
        );
        Ok(module_indices)
    }

    /// C++ Drawable::getCurrentClientBonePositions (Drawable.cpp776-801) and
    /// W3DModelDraw::getCurrentBonePositions (W3DModelDraw.cpp3545-3622).
    /// Returns scaled model-local C++ Z-up transforms from the actual installed
    /// HTree. A real missing bone/model contributes zero; unsupported/stale
    /// native state is an error and never becomes an inline test-bone fallback.
    #[cfg(feature = "game_client")]
    pub(crate) fn sample_current_client_bone_transforms(
        &mut self,
        source: &crate::game_logic::NativeSlaveSource,
        key: game_client::core::PresentationDirectDrawableBindingKey,
        bone_prefix: &str,
        start_index: i32,
        max_bones: usize,
        graphics: &mut GraphicsSystem,
    ) -> Result<Vec<Mat4>, NativeCameraPoseUnavailable> {
        if max_bones == 0 || bone_prefix.is_empty() {
            return Ok(Vec::new());
        }
        if !source.mesh_scale.is_finite() {
            return Err(NativeCameraPoseUnavailable::UnsupportedSourceControls);
        }
        let identity = &source.identity;
        let modules = self.receive_current_bone_source(source, key, graphics)?;
        let mut transforms = Vec::new();
        for module in modules {
            if transforms.len() >= max_bones {
                break;
            }
            self.validate_binding(identity, key, module)?;
            let state = self
                .drawable_visual_states
                .get_mut(&(identity.object_id(), module))
                .ok_or(NativeCameraPoseUnavailable::UnadmittedDrawableLifecycle)?;
            if !state.native_source_supported || !state.native_condition_supported {
                return Err(NativeCameraPoseUnavailable::UnsupportedSourceControls);
            }
            // The 64-bone clamp belongs to each ObjectDrawInterface, while
            // Drawable passes its remaining output capacity to the next module.
            let limit = (max_bones - transforms.len()).min(64);
            let end_index = if start_index == 0 { 0 } else { 99 };
            let mut count = 0;
            for index in start_index..=end_index {
                let name = if index == 0 {
                    bone_prefix.to_owned()
                } else {
                    format!("{bone_prefix}{index:02}")
                };
                let local =
                    match state.installed_client_bone_transform(self.animation_sync_ms, &name) {
                        Ok(Some(local)) => local,
                        Ok(None) | Err(NativeCameraPoseUnavailable::NoRenderObject) => break,
                        Err(error) => return Err(error),
                    };
                // C++ removes the RenderObj world transform then applies the
                // Drawable instance scale. The source hierarchy is already Z-up.
                transforms.push(Mat4::from_scale(Vec3::splat(source.mesh_scale)) * local);
                count += 1;
                if count >= limit {
                    break;
                }
            }
        }
        Ok(transforms)
    }

    /// C++ pristine queries select the logical condition afresh, independently
    /// of the installed transition/current animation. Model definitions are
    /// immutable; this never replaces or advances the live RenderObj pose.
    #[cfg(feature = "game_client")]
    pub(crate) fn sample_pristine_client_bone_transforms(
        &mut self,
        source: &crate::game_logic::NativeSlaveSource,
        key: game_client::core::PresentationDirectDrawableBindingKey,
        bone_prefix: &str,
        start_index: i32,
        max_bones: usize,
        graphics: &mut GraphicsSystem,
    ) -> Result<Vec<Mat4>, NativeCameraPoseUnavailable> {
        use NativeCameraPoseUnavailable::*;
        if max_bones == 0 || bone_prefix.is_empty() {
            return Ok(Vec::new());
        }
        if !source.mesh_scale.is_finite() || start_index < 0 {
            return Err(UnsupportedSourceControls);
        }
        let models = source
            .receiving_draw_models
            .object_draws(source.condition_bits)
            .map_err(|_| UnsupportedSourceControls)?;
        self.validate_object_retirement(&source.identity, key)
            .map_err(|_| UnadmittedDrawableLifecycle)?;
        let mut transforms = Vec::new();
        for selected in models {
            if transforms.len() >= max_bones {
                break;
            }
            self.validate_binding(&source.identity, key, selected.module_index)?;
            if selected.model_key.is_empty() {
                continue;
            }
            let mut budget = usize::MAX;
            let model = match RenderPipeline::ensure_render_model_loaded(
                &mut self.failed_model_resolutions,
                graphics,
                &source.template_name,
                &selected.model_key,
                true,
                &mut budget,
            ) {
                RenderModelLoadResult::Ready(model) => model,
                _ => return Err(UnadmittedDrawableLifecycle),
            };
            // validateStuff always chooses the first authored animation and
            // frame zero (or the explicit PRISTINE_BONE_POS_IN_FINAL_FRAME).
            let binding = match selected.animations.first() {
                Some(animation) => Some(
                    RenderPipeline::cached_draw_animation_binding(&model, &animation.name)
                        .ok_or(UnsupportedSourceControls)?,
                ),
                None => None,
            };
            transforms.extend(
                crate::assets::pristine_bones::model_pristine_bone_transforms(
                    &model,
                    &selected,
                    binding.as_ref(),
                    source.mesh_scale,
                    bone_prefix,
                    start_index,
                    max_bones - transforms.len(),
                )
                .map_err(|_| UnsupportedSourceControls)?,
            );
        }
        Ok(transforms)
    }

    /// Validate the exact receiving generation BEFORE consuming dirty state.
    /// Dirty selection/restore and the lazy HTree query execute at this call.
    #[cfg(feature = "game_client")]
    pub(crate) fn sample_camera_pose(
        &mut self,
        identity: &crate::assets::LiveDrawPlaybackIdentity,
        key: game_client::core::PresentationDirectDrawableBindingKey,
        module_index: u32,
        bone_name: &str,
        graphics: &mut GraphicsSystem,
    ) -> Result<Mat4, NativeCameraPoseUnavailable> {
        self.validate_binding(identity, key, module_index)?;
        self.consume_native_camera_conditions(
            crate::game_logic::ObjectId(identity.object_id()),
            graphics,
        );
        self.installed_camera_pose_for(identity, key, module_index, bone_name)
    }
    #[cfg(feature = "game_client")]
    pub(super) fn installed_camera_pose_for(
        &mut self,
        identity: &crate::assets::LiveDrawPlaybackIdentity,
        key: game_client::core::PresentationDirectDrawableBindingKey,
        module_index: u32,
        bone_name: &str,
    ) -> Result<Mat4, NativeCameraPoseUnavailable> {
        self.validate_binding(identity, key, module_index)?;
        self.installed_camera_pose(
            crate::game_logic::ObjectId(identity.object_id()),
            module_index,
            bone_name,
        )
    }
    pub(super) fn installed_camera_pose(
        &mut self,
        id: crate::game_logic::ObjectId,
        module_index: u32,
        bone_name: &str,
    ) -> Result<Mat4, NativeCameraPoseUnavailable> {
        let state = self
            .drawable_visual_states
            .get_mut(&(id.0, module_index))
            .ok_or(NativeCameraPoseUnavailable::UnadmittedDrawableLifecycle)?;
        if !state.native_source_supported || !state.native_condition_supported {
            return Err(NativeCameraPoseUnavailable::UnadmittedDrawableLifecycle);
        }
        state.installed_camera_pose(self.animation_sync_ms, bone_name)
    }
}

#[cfg(all(test, feature = "game_client"))]
#[path = "native_pose_loan_tests.rs"]
mod tests;
