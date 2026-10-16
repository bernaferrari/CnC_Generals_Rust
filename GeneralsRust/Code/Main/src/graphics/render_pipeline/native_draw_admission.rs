//! Synchronous native Drawable mutations admitted to the existing RenderObj owner.
use super::installed_draw_pose::{InstalledDrawObject, InstalledDrawPose};
use super::*;
use game_client::core::game_client::PresentationDrawableMutation;

impl RenderPipeline {
    pub(crate) fn current_animation_sync_ms(&self) -> u32 {
        self.animation_sync_ms
    }

    pub(crate) fn receive_native_drawable_mutation(
        &mut self,
        mutation: PresentationDrawableMutation<'_>,
        source: Option<&crate::presentation_frame::UnitRenderInput>,
        graphics: &mut GraphicsSystem,
    ) {
        let id = crate::game_logic::ObjectId(mutation.object_id());
        // Frozen admission leases distinguish restore/reset/id reuse even when
        // the receiving Client mirror can retain its numeric drawable binding.
        if let PresentationDrawableMutation::TransformInstalled { source: event, .. } = &mutation {
            if let Some(identity) = source.and_then(|s| s.draw_playback_identity.as_ref()) {
                let replacement = self
                    .drawable_visual_states
                    .iter()
                    .any(|((object, _), state)| {
                        *object == id.0 && state.native_host_identity.as_ref() != Some(identity)
                    });
                if replacement {
                    self.receive_native_drawable_mutation(
                        PresentationDrawableMutation::Creating {
                            source: event,
                            initial_transform: game_client::drawable::Matrix4::identity(),
                            sync_ms: self.animation_sync_ms,
                        },
                        source,
                        graphics,
                    );
                }
            }
        }
        match mutation {
            PresentationDrawableMutation::Destroyed { .. } => {
                self.drawable_visual_states
                    .retain(|(object, _), _| *object != id.0);
                self.live_draw_playback
                    .retain(|key, _| key.object_id() != id.0);
            }
            PresentationDrawableMutation::Creating {
                initial_transform,
                sync_ms,
                ..
            } => {
                self.drawable_visual_states
                    .retain(|(object, _), _| *object != id.0);
                let Some(source) = source else { return };
                let Some(receiving) = source.receiving_draw_models.as_deref() else {
                    return;
                };
                let Ok(models) = receiving.object_draws(0) else {
                    return;
                };
                let world = Mat4::from_cols_array_2d(&initial_transform.elements).transpose();
                for model in models {
                    let index = model.module_index;
                    let state = self
                        .drawable_visual_states
                        .entry((id.0, index))
                        .or_default();
                    state.native_host_identity = source.draw_playback_identity.clone();
                    state.native_source_template = source.template_name.clone();
                    // Native cached W3D is unscaled. C++ applies asset scale to
                    // HTree data, not camera basis. Until baked scale admission
                    // exists, do not accept a scaled object's bone camera.
                    state.native_source_supported = source.mesh_scale == 1.
                        && source.float_yaw == 0.
                        && source.float_pitch == 0.;
                    state.native_condition_bits = Some(0);
                    state.native_drawable_transform = world;
                    state.native_pending_condition = Some(model);
                    self.install_native_condition(id, index, world, sync_ms, graphics);
                }
            }
            PresentationDrawableMutation::TransformInstalled { .. } => {
                let Some((revision, world)) = source
                    .and_then(|s| s.draw_playback_identity.as_ref())
                    .and_then(|identity| identity.native_pose())
                else {
                    return;
                };
                for ((object, _), state) in &mut self.drawable_visual_states {
                    if *object == id.0 {
                        if let Some(source) = source {
                            state.native_source_supported = source.mesh_scale == 1.
                                && source.float_yaw == 0.
                                && source.float_pitch == 0.;
                        }
                    }
                    if *object == id.0 && state.native_transform_revision != Some(revision) {
                        state.native_transform_revision = Some(revision);
                        state.native_drawable_transform = world;
                        if let InstalledDrawObject::Ready(pose) = &mut state.installed_draw_pose {
                            let mut adjusted = world;
                            if let (Some(source), Some(selected)) =
                                (source, state.native_selected_condition.as_ref())
                            {
                                if selected.flags&(1<<crate::assets::ACBIT_ADJUST_HEIGHT_BY_CONSTRUCTION_PERCENT)!=0 {
                                    let percent=if source.under_construction {source.construction_percent.clamp(0.,1.)*100.}else{-1.};
                                    if let Some(delta)=crate::assets::construction_percent_height_delta(percent,source.max_height_above_position) {
                                        adjusted*=Mat4::from_translation(Vec3::Y*delta);
                                    }
                                }
                            }
                            pose.install_transform(adjusted);
                        }
                    }
                }
            }
            PresentationDrawableMutation::ConditionsMarked { bits, .. } => {
                let Some(source) = source else { return };
                let Some(receiving) = source.receiving_draw_models.as_deref() else {
                    return;
                };
                let Ok(models) = receiving.object_draws(bits) else {
                    return;
                };
                for model in models {
                    let state = self
                        .drawable_visual_states
                        .entry((id.0, model.module_index))
                        .or_default();
                    if state.native_condition_bits != Some(bits) {
                        state.native_condition_bits = Some(bits);
                        state.native_pending_condition = Some(model);
                    }
                }
            }
        }
    }

    /// Source getDrawModules: consume dirty selections only for the actual
    /// named camera target. This does not run Draw pause/completion/recoil.
    pub(crate) fn consume_native_camera_conditions(
        &mut self,
        id: crate::game_logic::ObjectId,
        graphics: &mut GraphicsSystem,
    ) {
        let modules: Vec<_> = self
            .drawable_visual_states
            .iter()
            .filter_map(|((object, index), state)| {
                (*object == id.0 && state.native_pending_condition.is_some()).then_some(*index)
            })
            .collect();
        let mut modules = modules;
        modules.sort_unstable();
        for index in modules {
            let world = match self
                .drawable_visual_states
                .get(&(id.0, index))
                .map(|s| &s.installed_draw_pose)
            {
                Some(InstalledDrawObject::Ready(pose)) => pose.world,
                // Model=None has no retained render transform. Its actual
                // Drawable transform must remain available for recreation.
                _ => self
                    .drawable_visual_states
                    .get(&(id.0, index))
                    .map(|s| s.native_drawable_transform)
                    .unwrap_or(Mat4::IDENTITY),
            };
            self.install_native_condition(id, index, world, self.animation_sync_ms, graphics);
        }
    }

    fn install_native_condition(
        &mut self,
        id: crate::game_logic::ObjectId,
        index: u32,
        world: Mat4,
        sync_ms: u32,
        graphics: &mut GraphicsSystem,
    ) {
        let Some(state) = self.drawable_visual_states.get_mut(&(id.0, index)) else {
            return;
        };
        let Some(selected) = state.native_pending_condition.take() else {
            return;
        };
        if !state.native_source_supported {
            state.installed_draw_pose = InstalledDrawObject::AwaitingAdmission;
            return;
        }
        if state.native_selected_condition.as_ref() == Some(&selected) {
            return;
        }
        state.native_condition_supported = false;
        // Transition/wait state admission still needs the source setModelState
        // contract; never replace it with an immediately selected destination.
        if !selected.transition_key.is_empty()
            || !selected.allow_to_finish_key.is_empty()
            || selected.is_transition
            || selected.animations.len() > 1
            || selected.flags & ((1 << 0) | (1 << 5) | (1 << 7) | (1 << 8) | (1 << 9)) != 0
        {
            state.installed_draw_pose = InstalledDrawObject::AwaitingAdmission;
            state.native_selected_condition = Some(selected);
            return;
        }
        let range = selected
            .animation_speed_range_bits
            .unwrap_or([1f32.to_bits(); 2]);
        let low = f32::from_bits(range[0]);
        let speed = f32::from_bits(range[1]);
        // Source GetGameClientRandomValueReal returns hi without drawing
        // when delta <= 0, including reversed authored bounds.
        if !low.is_finite() || !speed.is_finite() || speed - low > 0. {
            state.installed_draw_pose = InstalledDrawObject::AwaitingAdmission;
            state.native_selected_condition = Some(selected);
            return;
        }
        let template = state.native_source_template.clone();
        state.native_selected_condition = Some(selected.clone());
        if selected.model_key.is_empty() {
            state.installed_draw_pose = InstalledDrawObject::NoRenderObject;
            state.animation = None;
            state.identity = None;
            state.native_condition_supported = true;
            return;
        }
        let mut budget = usize::MAX;
        let model = match Self::ensure_render_model_loaded(
            &mut self.failed_model_resolutions,
            graphics,
            &template,
            &selected.model_key,
            true,
            &mut budget,
        ) {
            RenderModelLoadResult::Ready(model) => model,
            _ => {
                self.drawable_visual_states
                    .get_mut(&(id.0, index))
                    .unwrap()
                    .installed_draw_pose = InstalledDrawObject::AwaitingAdmission;
                return;
            }
        };
        let Some(identity) = super::pipeline_drawable_state::frozen_visual_identity_for_draw_model(
            &template, &selected,
        ) else {
            self.drawable_visual_states
                .get_mut(&(id.0, index))
                .unwrap()
                .installed_draw_pose = InstalledDrawObject::AwaitingAdmission;
            return;
        };
        let binding = identity.animation.as_ref().and_then(|animation| {
            Self::cached_draw_animation_binding(&model, &animation.hierarchy_animation)
        });
        let state = self.drawable_visual_states.get_mut(&(id.0, index)).unwrap();
        let retain = match &state.installed_draw_pose {
            InstalledDrawObject::Ready(pose) => {
                Arc::ptr_eq(&pose.model, &model)
                    && pose.turret_names.0 == selected.primary_turret.yaw_bone
                    && pose.turret_names.1 == selected.primary_turret.pitch_bone
            }
            _ => false,
        };
        if identity.animation.is_some() && binding.is_none() {
            state.installed_draw_pose = InstalledDrawObject::AwaitingAdmission;
            return;
        }
        state.identity = Some(identity.clone());
        state.native_drawable_transform = world;
        if !retain {
            state.animation = None;
        }
        if let (Some(binding), Some(animation)) = (binding.as_ref(), identity.animation.as_ref()) {
            if let Some((frames, rate)) = model.animation_binding_metadata(binding) {
                if frames == 0 || rate == 0 {
                    state.installed_draw_pose = InstalledDrawObject::AwaitingAdmission;
                    return;
                }
                let backwards = matches!(
                    selected.animation_mode,
                    crate::assets::AuthoredDrawAnimationMode::OnceBackwards
                        | crate::assets::AuthoredDrawAnimationMode::LoopBackwards
                );
                let mut start = if backwards {
                    frames.saturating_sub(1) as f32
                } else {
                    0.
                };
                if selected.flags & (1 << 1) != 0 {
                    start = 0.;
                } else if selected.flags & (1 << 2) != 0 {
                    start = frames.saturating_sub(1) as f32;
                }
                state.animation = Some(ObjectAnimationState {
                    animation_binding_key: Some(binding.state_key()),
                    animation_identity: animation.hierarchy_animation.clone(),
                    current_frame: start,
                    frame_rate: rate as f32,
                    num_frames: frames,
                    mode: selected.animation_mode.clone(),
                    last_sync_ms: sync_ms,
                    previous_frame: start,
                    frame_rate_multiplier: speed,
                    direction: ObjectAnimationState::initial_direction(&selected.animation_mode),
                    playback_mode: selected.animation_mode.clone(),
                });
            }
        }
        if retain {
            let InstalledDrawObject::Ready(pose) = &mut state.installed_draw_pose else {
                unreachable!()
            };
            // Source adjustAnimation with no entries leaves installed HAnim.
            if !selected.animations.is_empty() {
                pose.animation_binding = binding;
                pose.hierarchy_valid = false;
            }
        } else {
            state.installed_draw_pose = InstalledDrawObject::Ready(InstalledDrawPose::created(
                model, world, &selected, binding,
            ));
            // nukeCurrentRender resets the module pause latch on recreation.
            state.animation_paused = false;
        }
        state.native_condition_supported = true;
    }
}
