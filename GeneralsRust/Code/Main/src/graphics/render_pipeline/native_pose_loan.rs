//! Bounded access to the actual installed Draw state during synchronous callbacks.
//! This is a field loan, not another renderer, animation clock or pose cache.
use super::*;
use installed_draw_pose::{NativeCameraPoseUnavailable, ObjectVisualState};

pub(crate) struct NativeInstalledPoseLoan<'a> {
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
    /// Stamp the actual post-sync Client lifetime on the existing Draw owner.
    /// A changed key requires a real Destroy/Creating producer, never relabeling.
    #[cfg(feature = "game_client")]
    pub(super) fn admit_binding(
        &mut self,
        identity: &crate::assets::LiveDrawPlaybackIdentity,
        key: game_client::core::PresentationDirectDrawableBindingKey,
    ) -> Result<(), &'static str> {
        if key.object_id != identity.object_id() {
            return Err("Client binding object mismatch");
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
        for ((id, _), state) in &mut *self.drawable_visual_states {
            if *id == identity.object_id() {
                state.native_client_binding = Some(key);
            }
        }
        Ok(())
    }
    #[cfg(feature = "game_client")]
    pub(super) fn validate_binding(
        &self,
        identity: &crate::assets::LiveDrawPlaybackIdentity,
        key: game_client::core::PresentationDirectDrawableBindingKey,
        module_index: u32,
    ) -> Result<(), NativeCameraPoseUnavailable> {
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
        let models = source
            .receiving_draw_models
            .object_draws(source.condition_bits)
            .map_err(|_| NativeCameraPoseUnavailable::UnsupportedSourceControls)?;
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
        self.sample_camera_pose(identity, key, first.module_index, bone_name, graphics)
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
