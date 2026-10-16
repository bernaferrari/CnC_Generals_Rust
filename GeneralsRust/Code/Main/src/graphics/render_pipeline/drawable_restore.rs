//! One SetAnimation/recoil-state restore kernel for Draw and pre-camera load.
//! No pause, completion, discharges, recoil evolution, or HTree evaluation.
use super::*;

pub(super) struct RestoredDrawable {
    pub(super) binding: Option<crate::assets::W3dAnimationBinding>,
    pub(super) frame: f32,
}

pub(super) fn restore_client_drawable_state(
    state: &mut ObjectVisualState,
    identity: &FrozenVisualDrawIdentity,
    model: &crate::assets::W3DModel,
    draw_model: &crate::assets::AuthoredDrawModel,
    saved: &ClientDrawableStateSnapshot,
    sync_ms: u32,
) -> Option<RestoredDrawable> {
    if !visual_identity_matches_snapshot(identity, saved) || !saved.has_finite_visual_values() {
        return None;
    }
    let topology =
        model.weapon_barrel_topology_for_authored_bindings(&draw_model.weapon_bone_bindings)?;
    let recoil_slots =
        restored_recoil_slots_for_topology(saved, &topology, &draw_model.recoil_kinematics)?;
    let (binding, animation) = match saved.animation.as_ref() {
        None if identity.animation.is_none() => (None, None),
        Some(saved_animation) => {
            let binding = RenderPipeline::cached_draw_animation_binding(
                model,
                &saved_animation.hierarchy_animation,
            )?;
            if !model.animation_binding_is_compatible(&binding) {
                return None;
            }
            let (frames, rate) = model.animation_binding_metadata(&binding)?;
            if frames == 0
                || rate == 0
                || saved_animation.frame < 0.
                || saved_animation.frame > frames.saturating_sub(1) as f32
            {
                return None;
            }
            let mode = authored_mode(saved_animation.mode);
            // CPP W3DModelDraw4213–4220 ignores saved mode and passes the
            // receiving RenderObj's current curMode to Set_Animation. A paused
            // Manual clock must stay Manual while the saved frame is restored.
            let playback_mode = state
                .animation
                .as_ref()
                .map_or_else(|| mode.clone(), |old| old.playback_mode.clone());
            let previous_frame = state
                .animation
                .as_ref()
                .map_or(saved_animation.frame, |old| old.current_frame);
            let animation = ObjectAnimationState {
                animation_binding_key: Some(binding.state_key()),
                animation_identity: saved_animation.hierarchy_animation.clone(),
                current_frame: saved_animation.frame,
                frame_rate: rate as f32,
                num_frames: frames,
                mode: mode.clone(),
                // Source Xfer invokes SetAnimation at current receiving Sync.
                last_sync_ms: sync_ms,
                previous_frame,
                frame_rate_multiplier: 1.,
                direction: ObjectAnimationState::initial_direction(&playback_mode),
                playback_mode,
            };
            (Some(binding), Some(animation))
        }
        None => return None,
    };
    let frame = animation
        .as_ref()
        .map_or(0., |animation| animation.current_frame);
    // All identity, asset and recoil checks completed before mutating owner.
    state.identity = Some(identity.clone());
    state.force_bind_pose = animation.is_none();
    state.animation = animation;
    state.last_seen_weapon_discharge_sequence = saved.last_seen_weapon_discharge_sequence;
    state.recoil_slots = recoil_slots;
    if let super::super::installed_draw_pose::InstalledDrawObject::Ready(pose) =
        &mut state.installed_draw_pose
    {
        pose.animation_binding = binding.clone();
        pose.hierarchy_valid = false;
    }
    Some(RestoredDrawable { binding, frame })
}

pub(super) fn reject_client_drawable_restore(state: &mut ObjectVisualState) {
    state.force_bind_pose = true;
    state.animation = None;
    state.recoil_slots = std::array::from_fn(|_| Vec::new());
    if let super::super::installed_draw_pose::InstalledDrawObject::Ready(pose) =
        &mut state.installed_draw_pose
    {
        pose.animation_binding = None;
        pose.hierarchy_valid = false;
    }
}

fn take_native_restore_candidates(
    pending: &mut Option<ClientDrawableWorldSnapshot>,
    imports: &mut HashMap<(u32, u32), ClientDrawableStateSnapshot>,
    object_id: u32,
) -> Vec<ClientDrawableStateSnapshot> {
    let mut candidates = Vec::new();
    if let Some(snapshot) = pending.as_mut() {
        let mut remaining = Vec::new();
        for saved in snapshot.drawables.drain(..) {
            if saved.object_id == object_id {
                candidates.push(saved);
            } else {
                remaining.push(saved);
            }
        }
        snapshot.drawables = remaining;
    }
    if pending
        .as_ref()
        .is_some_and(|snapshot| snapshot.drawables.is_empty())
    {
        *pending = None;
    }
    let mut imported_keys = imports
        .keys()
        .filter(|(object, _)| *object == object_id)
        .copied()
        .collect::<Vec<_>>();
    imported_keys.sort_unstable();
    for key in imported_keys {
        if let Some(saved) = imports.remove(&key) {
            candidates.push(saved);
        }
    }
    candidates
}

impl RenderPipeline {
    /// Consume this resident object's saved modules after constructor/dirty
    /// installation and before the first GetBoneTransform. Other objects stay
    /// queued; no visibility-filtered frame or Draw operation is required.
    pub(crate) fn admit_pending_native_drawable_restore(
        &mut self,
        id: crate::game_logic::ObjectId,
    ) {
        let candidates = take_native_restore_candidates(
            &mut self.pending_client_drawable_restore,
            &mut self.pending_client_drawable_imports,
            id.0,
        );
        let mut seen = HashSet::new();
        for saved in candidates {
            let key = (saved.object_id, saved.draw_module_index);
            if !saved.has_stable_source_identity()
                || !saved.has_finite_visual_values()
                || !seen.insert(key)
            {
                continue;
            }
            let Some(state) = self.drawable_visual_states.get_mut(&key) else {
                continue;
            };
            if !state.native_source_supported || !state.native_condition_supported {
                continue;
            }
            let Some(selected) = state.native_selected_condition.clone() else {
                continue;
            };
            let Some(identity) =
                frozen_visual_identity_for_draw_model(&state.native_source_template, &selected)
            else {
                continue;
            };
            // A foreign model/template/state with equal numeric ID is discarded
            // without changing the receiving constructor's state.
            if !visual_identity_matches_snapshot(&identity, &saved) {
                continue;
            }
            let super::super::installed_draw_pose::InstalledDrawObject::Ready(pose) =
                &state.installed_draw_pose
            else {
                continue;
            };
            let model = Arc::clone(&pose.model);
            if restore_client_drawable_state(
                state,
                &identity,
                &model,
                &selected,
                &saved,
                self.animation_sync_ms,
            )
            .is_none()
            {
                reject_client_drawable_restore(state);
            }
        }
    }
}

#[cfg(test)]
#[path = "drawable_restore_tests.rs"]
mod tests;
