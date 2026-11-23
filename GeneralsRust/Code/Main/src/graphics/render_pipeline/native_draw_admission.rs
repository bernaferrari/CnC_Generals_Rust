//! Synchronous native Drawable mutations admitted to the existing RenderObj owner.
use super::installed_draw_pose::{InstalledDrawObject, InstalledDrawPose};
use super::native_draw_source::NativeDrawSource;
use super::*;
use game_client::core::game_client::PresentationDrawableMutation;

enum NativeDrawMutation {
    Creating {
        initial_transform: game_client::drawable::Matrix4,
        sync_ms: u32,
    },
    TransformInstalled {
        transform: game_client::drawable::Matrix4,
    },
    ConditionsMarked {
        bits: u128,
    },
    Destroyed,
}

impl RenderPipeline {
    pub(crate) fn current_animation_sync_ms(&self) -> u32 {
        self.animation_sync_ms
    }

    #[cfg(test)]
    pub(crate) fn receive_native_drawable_mutation(
        &mut self,
        mutation: PresentationDrawableMutation<'_>,
        source: Option<&crate::presentation_frame::UnitRenderInput>,
        graphics: &mut GraphicsSystem,
    ) {
        self.receive_native_display_mutation(mutation, source.map(NativeDrawSource::display), graphics);
    }

    pub(crate) fn receive_native_display_mutation(
        &mut self,
        mutation: PresentationDrawableMutation<'_>,
        source: Option<NativeDrawSource<'_>>,
        graphics: &mut GraphicsSystem,
    ) {
        if let PresentationDrawableMutation::TerrainPropAdded { source, request } = mutation {
            self.receive_native_prop(source.host_epoch, request);
            return;
        }
        if let PresentationDrawableMutation::Destroyed { terrain_tracks, .. } = &mutation
            && let Some(terrain) = self.native_terrain.as_mut()
        {
            for binding in *terrain_tracks {
                terrain.retire_terrain_track(binding);
            }
        }
        self.borrow_native_installed_poses()
            .receive_native_drawable_mutation(mutation, source, graphics);
        self.flush_native_props();
    }

    /// Source getDrawModules on the same installed owner, through a bounded loan.
    pub(crate) fn consume_native_camera_conditions(
        &mut self,
        id: crate::game_logic::ObjectId,
        graphics: &mut GraphicsSystem,
    ) {
        self.borrow_native_installed_poses()
            .consume_native_camera_conditions(id, graphics);
    }
    fn install_native_condition(
        &mut self,
        id: crate::game_logic::ObjectId,
        index: u32,
        world: Mat4,
        sync_ms: u32,
        graphics: &mut GraphicsSystem,
    ) {
        self.borrow_native_installed_poses()
            .install_native_condition(id, index, world, sync_ms, graphics);
    }
}

impl super::native_pose_loan::NativeInstalledPoseLoan<'_> {
    /// Retire this receiver's render state and playback at one destroy boundary.
    /// Numeric ObjectIds in other pipeline owners are independent.
    pub(super) fn retire_native_drawable(&mut self, id: crate::game_logic::ObjectId) {
        self.retire_native_owner(NativeDrawOwnerId::Actor(id.0));
    }

    /// GameClient reset has already retired every CPU Drawable before the
    /// subsequent GameLogic reset begins. Retire the corresponding renderer
    /// constructor receipts at that same engine boundary, so later Object
    /// callbacks observe the source-ordered absence instead of a stale pose
    /// lifetime with no Client binding.
    pub(crate) fn retire_after_game_client_reset(
        &mut self,
        client: &game_client::core::game_client::NativeDrawableOwner<'_>,
    ) -> Result<(), String> {
        if client.has_live_drawable_world() {
            return Err("GameClient reset left native Drawables installed".into());
        }
        let owners: Vec<_> = self.native_drawable_lifetimes.keys().copied().collect();
        for owner in owners {
            self.retire_native_owner(owner);
        }
        *self.pending_client_drawable_restore = None;
        self.pending_client_drawable_imports.clear();
        Ok(())
    }
    /// Drop one owner's module states (its contiguous key range only).
    fn forget_owner_module_states(&mut self, owner: NativeDrawOwnerId) {
        let keys: Vec<_> = self
            .drawable_visual_states
            .range(native_draw_world::owner_module_keys(owner))
            .map(|(key, _)| *key)
            .collect();
        for key in keys {
            self.drawable_visual_states.remove(&key);
        }
    }

    fn retire_native_owner(&mut self, owner: NativeDrawOwnerId) {
        self.native_drawable_lifetimes.remove(&owner);
        self.forget_owner_module_states(owner);
        self.live_draw_playback
            .retain(|key, _| key.owner_id() != owner);
    }

    pub(crate) fn receive_native_drawable_mutation(
        &mut self,
        mutation: PresentationDrawableMutation<'_>,
        source: Option<NativeDrawSource<'_>>,
        graphics: &mut GraphicsSystem,
    ) {
        let owner = NativeDrawOwnerId::Actor(mutation.object_id());
        let (mirror, finishes_mirror) = match &mutation {
            PresentationDrawableMutation::TransformInstalled { source: sync, .. } => {
                (source.and_then(|source| source.mirrored_pose(sync)), false)
            }
            PresentationDrawableMutation::ConditionsMarked { source: sync, .. } => {
                (source.and_then(|source| source.mirrored_pose(sync)), true)
            }
            _ => (None, false),
        };
        // ConditionsMarked follows both mirror setters. Skip the pair only
        // after this exact pose has already been observed on this birth.
        // Actual constructor and Object setter callbacks still invalidate.
        if let Some((identity, pose)) = mirror
            && !finishes_mirror
            && self
                .native_drawable_lifetimes
                .get(&owner)
                .is_some_and(|lifetime| {
                    lifetime.identity == *identity && lifetime.last_mirrored_pose == Some(pose)
                })
        {
            return;
        }
        self.receive_native_mutation_from_source(mutation, source, graphics);
        if let Some((identity, pose)) = mirror
            && finishes_mirror
            && let Some(lifetime) = self.native_drawable_lifetimes.get_mut(&owner)
            && lifetime.identity == *identity
        {
            lifetime.last_mirrored_pose = Some(pose);
        }
    }

    /// A factory callback supplies the current receiving object directly. A
    /// missing source or invalid module selection cannot look like admission.
    pub(crate) fn receive_native_factory_mutation(
        &mut self,
        mutation: PresentationDrawableMutation<'_>,
        source: &crate::game_logic::NativeSlaveSource,
        graphics: &mut GraphicsSystem,
    ) -> Result<(), String> {
        let id = mutation.object_id();
        if id != source.identity.object_id() {
            return Err("factory Draw source belongs to another object".into());
        }
        let expected = match &mutation {
            PresentationDrawableMutation::Creating { .. } => Some(
                source
                    .receiving_draw_models
                    .native_render_draws(0)
                    .map_err(|_| "invalid receiving default Draw selection")?,
            ),
            PresentationDrawableMutation::ConditionsMarked { bits, .. } => {
                source
                    .receiving_draw_models
                    .native_render_draws(*bits)
                    .map_err(|_| "invalid receiving current Draw selection")?;
                None
            }
            _ => None,
        };
        self.receive_native_mutation_from_source(
            mutation,
            Some(NativeDrawSource::current(source)),
            graphics,
        );
        if let Some(expected) = expected {
            let actual = self
                .drawable_visual_states
                .range(native_draw_world::owner_module_keys(NativeDrawOwnerId::Actor(id)))
                .filter(|((object, _), _)| *object == NativeDrawOwnerId::Actor(id))
                .collect::<Vec<_>>();
            if actual.len() != expected.len()
                || expected.iter().any(|module| {
                    !actual.iter().any(|((_, index), state)| {
                        *index == module.module_index
                            && state.native_host_identity.as_ref() == Some(&source.identity)
                    })
                })
            {
                return Err(
                    "factory Draw constructor did not admit its complete receiving roster".into(),
                );
            }
        }
        Ok(())
    }

    pub(crate) fn receive_native_auxiliary_mutation(
        &mut self,
        mutation: game_client::core::game_client::NativeAuxiliaryMutation<'_>,
        identity: &crate::assets::LiveDrawPlaybackIdentity,
        receiving: &Arc<crate::game_logic::thing::ReceivingDrawModels>,
        template: &str,
        mesh_scale: f32,
        graphics: &mut GraphicsSystem,
    ) -> Result<(), String> {
        use game_client::core::game_client::NativeAuxiliaryMutation as Mutation;
        let receipt = match &mutation {
            Mutation::Creating { receipt, .. }
            | Mutation::TransformInstalled { receipt, .. }
            | Mutation::ConditionsMarked { receipt, .. }
            | Mutation::Destroyed { receipt } => *receipt,
        };
        let owner = NativeDrawOwnerId::Aux(receipt.drawable_id());
        if !identity.matches_auxiliary_receipt(receipt) {
            return Err("auxiliary RenderObj source belongs to another Drawable lifetime".into());
        }
        if !matches!(mutation, Mutation::Creating { .. })
            && self
                .native_drawable_lifetimes
                .get(&owner)
                .is_none_or(|lifetime| lifetime.identity != *identity)
        {
            return Err("auxiliary RenderObj continuation has no matching constructor".into());
        }
        let normalized = match mutation {
            Mutation::Creating {
                initial_transform,
                sync_ms,
                ..
            } => {
                receiving
                    .native_render_draws(0)
                    .map_err(|_| "invalid receiving auxiliary Draw roster")?;
                NativeDrawMutation::Creating {
                    initial_transform,
                    sync_ms,
                }
            }
            Mutation::TransformInstalled { transform, .. } => {
                NativeDrawMutation::TransformInstalled { transform }
            }
            Mutation::ConditionsMarked { bits, .. } => {
                receiving
                    .native_render_draws(bits)
                    .map_err(|_| "invalid receiving auxiliary Draw condition")?;
                NativeDrawMutation::ConditionsMarked { bits }
            }
            Mutation::Destroyed { .. } => NativeDrawMutation::Destroyed,
        };
        self.receive_native_mutation_for_owner(
            owner,
            normalized,
            Some(NativeDrawSource::auxiliary(
                identity, receiving, template, mesh_scale,
            )),
            graphics,
        );
        if let Some(lifetime) = self.native_drawable_lifetimes.get_mut(&owner) {
            lifetime.auxiliary = Some(Arc::clone(receiving));
        }
        Ok(())
    }

    pub(crate) fn retire_native_auxiliary(
        &mut self,
        receipt: &game_client::core::game_client::NativeAuxiliaryReceipt,
        identity: &crate::assets::LiveDrawPlaybackIdentity,
    ) -> Result<(), String> {
        let owner = NativeDrawOwnerId::Aux(receipt.drawable_id());
        if !identity.matches_auxiliary_receipt(receipt)
            || self
                .native_drawable_lifetimes
                .get(&owner)
                .is_none_or(|lifetime| lifetime.identity != *identity)
        {
            return Err("auxiliary retirement has no matching RenderObj birth".into());
        }
        self.retire_native_owner(owner);
        Ok(())
    }

    pub(crate) fn clear_world(&mut self) {
        self.native_drawable_lifetimes.clear();
        self.drawable_visual_states.clear();
        self.live_draw_playback.clear();
        *self.pending_client_drawable_restore = None;
        self.pending_client_drawable_imports.clear();
    }

    fn receive_native_mutation_from_source(
        &mut self,
        mutation: PresentationDrawableMutation<'_>,
        source: Option<NativeDrawSource<'_>>,
        graphics: &mut GraphicsSystem,
    ) {
        if let PresentationDrawableMutation::TerrainPropAdded { source, request } = mutation {
            if !self.pending_native_props.push(source.host_epoch, request) {
                warn!("rejected terrain prop from a different draw-world epoch");
            }
            return;
        }
        if let PresentationDrawableMutation::TerrainTreeAdded { source: sync, mut request } = mutation {
            if let Some(source) = source {
                request.scale = source.mesh_scale;
            }
            if !self.pending_native_trees.push(sync.host_epoch, request) {
                warn!("rejected terrain tree from a different draw-world epoch");
            }
            return;
        }
        let owner = NativeDrawOwnerId::Actor(mutation.object_id());
        let mutation = match mutation {
            PresentationDrawableMutation::Creating {
                initial_transform,
                sync_ms,
                ..
            } => NativeDrawMutation::Creating {
                initial_transform,
                sync_ms,
            },
            PresentationDrawableMutation::TransformInstalled { transform, .. } => {
                NativeDrawMutation::TransformInstalled { transform }
            }
            PresentationDrawableMutation::ConditionsMarked { bits, .. } => {
                NativeDrawMutation::ConditionsMarked { bits }
            }
            PresentationDrawableMutation::Destroyed { .. } => NativeDrawMutation::Destroyed,
            PresentationDrawableMutation::TerrainPropAdded { .. }
            | PresentationDrawableMutation::TerrainTreeAdded { .. } => unreachable!(),
        };
        self.receive_native_mutation_for_owner(owner, mutation, source, graphics);
    }

    fn receive_native_mutation_for_owner(
        &mut self,
        owner: NativeDrawOwnerId,
        mutation: NativeDrawMutation,
        source: Option<NativeDrawSource<'_>>,
        graphics: &mut GraphicsSystem,
    ) {
        // Frozen admission leases distinguish restore/reset/id reuse even when
        // the receiving Client mirror can retain its numeric drawable binding.
        if let NativeDrawMutation::TransformInstalled { .. } = &mutation {
            if let Some(identity) = source.and_then(|s| s.identity) {
                let replacement = self
                    .native_drawable_lifetimes
                    .get(&owner)
                    .is_some_and(|lifetime| lifetime.identity != *identity)
                    || self
                        .drawable_visual_states
                        .range(native_draw_world::owner_module_keys(owner))
                        .any(|((object, _), state)| {
                            *object == owner
                                && state.native_host_identity.as_ref() != Some(identity)
                        });
                if replacement {
                    self.receive_native_mutation_for_owner(
                        owner,
                        NativeDrawMutation::Creating {
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
            NativeDrawMutation::Destroyed => {
                self.retire_native_owner(owner);
            }
            NativeDrawMutation::Creating {
                initial_transform,
                sync_ms,
                ..
            } => {
                self.native_drawable_lifetimes.remove(&owner);
                if let Some(identity) = source.and_then(|source| source.identity) {
                    self.native_drawable_lifetimes.insert(
                        owner,
                        native_draw_world::NativeDrawableLifetime {
                            auxiliary: None,
                            identity: identity.clone(),
                            last_mirrored_pose: None,
                            #[cfg(feature = "game_client")]
                            binding: None,
                        },
                    );
                }
                self.forget_owner_module_states(owner);
                let Some(source) = source else { return };
                let Some(receiving) = source.receiving else {
                    return;
                };
                let Ok(models) = receiving.native_render_draws(0) else {
                    return;
                };
                let world = Mat4::from_cols_array_2d(&initial_transform.elements).transpose();
                for model in models.iter() {
                    let index = model.module_index;
                    let state = self
                        .drawable_visual_states
                        .entry((owner, index))
                        .or_default();
                    state.native_host_identity = source.identity.cloned();
                    state.native_source_template = source.template.to_owned();
                    state.native_instance_scale =
                        source.mesh_scale.is_finite().then_some(source.mesh_scale);
                    state.native_condition_bits = Some(0);
                    state.native_drawable_transform = world;
                    state.native_pending_condition = Some(model.clone());
                    self.install_native_condition_for_owner(owner, index, world, sync_ms, graphics);
                }
            }
            NativeDrawMutation::TransformInstalled { transform, .. } => {
                let Some(source) = source else { return };
                if let Some(lifetime) = self.native_drawable_lifetimes.get_mut(&owner) {
                    lifetime.last_mirrored_pose = None;
                }
                for ((object, _), state) in self
                    .drawable_visual_states
                    .range_mut(native_draw_world::owner_module_keys(owner))
                {
                    if *object == owner {
                        source.install_observed_transform(state, transform);
                    }
                }
            }
            NativeDrawMutation::ConditionsMarked { bits, .. } => {
                let Some(source) = source else { return };
                let Some(receiving) = source.receiving else {
                    return;
                };
                let Ok(models) = receiving.native_render_draws(bits) else {
                    // A malformed sibling cannot leave an earlier module's
                    // previous condition looking like a successful admission.
                    for ((object, _), state) in self
                    .drawable_visual_states
                    .range_mut(native_draw_world::owner_module_keys(owner))
                {
                        if *object == owner {
                            state.native_condition_supported = false;
                            state.native_pending_condition = None;
                            state.installed_draw_pose = InstalledDrawObject::AwaitingAdmission;
                        }
                    }
                    return;
                };
                for model in models.iter() {
                    let state = self
                        .drawable_visual_states
                        .entry((owner, model.module_index))
                        .or_default();
                    state.mark_native_condition(bits, model);
                }
            }
        }
    }

    /// Mirror the directly borrowed Drawable flags into this object's native
    /// Draw modules for the next explicit dirty-condition consumer.
    #[cfg(feature = "game_client")]
    pub(crate) fn mark_receiving_drawable_conditions(
        &mut self,
        source: &crate::game_logic::NativeSlaveSource,
        bits: u128,
        graphics: &mut GraphicsSystem,
    ) {
        self.receive_native_mutation_for_owner(
            NativeDrawOwnerId::Actor(source.identity.object_id()),
            NativeDrawMutation::ConditionsMarked { bits },
            Some(NativeDrawSource::current(source)),
            graphics,
        );
    }

    /// Source getDrawModules: consume dirty selections only for the actual
    /// named camera target. This does not run Draw pause/completion/recoil.
    pub(crate) fn consume_native_camera_conditions(
        &mut self,
        id: crate::game_logic::ObjectId,
        graphics: &mut GraphicsSystem,
    ) {
        let owner = NativeDrawOwnerId::Actor(id.0);
        // The owner range is already in module declaration order.
        let modules: Vec<_> = self
            .drawable_visual_states
            .range(native_draw_world::owner_module_keys(owner))
            .filter_map(|((_, index), state)| state.native_pending_condition.is_some().then_some(*index))
            .collect();
        for index in modules {
            let world = match self
                .drawable_visual_states
                .get(&(owner, index))
                .map(|s| &s.installed_draw_pose)
            {
                Some(InstalledDrawObject::Ready(pose)) => pose.world,
                // Model=None has no retained render transform. Its actual
                // Drawable transform must remain available for recreation.
                _ => self
                    .drawable_visual_states
                    .get(&(owner, index))
                    .map(|s| s.native_drawable_transform)
                    .unwrap_or(Mat4::IDENTITY),
            };
            self.install_native_condition(id, index, world, self.animation_sync_ms, graphics);
        }
        self.admit_pending_native_drawable_restore(id);
    }

    pub(super) fn install_native_condition(
        &mut self,
        id: crate::game_logic::ObjectId,
        index: u32,
        world: Mat4,
        sync_ms: u32,
        graphics: &mut GraphicsSystem,
    ) {
        self.install_native_condition_for_owner(
            NativeDrawOwnerId::Actor(id.0),
            index,
            world,
            sync_ms,
            graphics,
        );
    }
    pub(super) fn install_native_condition_for_owner(
        &mut self,
        owner: NativeDrawOwnerId,
        index: u32,
        world: Mat4,
        sync_ms: u32,
        graphics: &mut GraphicsSystem,
    ) {
        let Some(state) = self.drawable_visual_states.get_mut(&(owner, index)) else {
            return;
        };
        let Some(selected) = state.take_native_condition_to_install() else {
            return;
        };
        // C++ setModelState can wait or select a transition only when the
        // current state has a transition key. With no current key, installing
        // the requested real state is direct even if it carries its own keys.
        // Keep potentially pending transitions unavailable until their full
        // sequencing is admitted.
        let needs_transition = match state.native_selected_condition.as_ref() {
            None => false,
            Some(_)
                if matches!(
                    &state.installed_draw_pose,
                    InstalledDrawObject::AwaitingAdmission
                ) =>
            {
                !selected.transition_key.is_empty() || !selected.allow_to_finish_key.is_empty()
            }
            Some(current) => {
                !current.transition_key.is_empty()
                    && (selected.allow_to_finish_key == current.transition_key
                        || !selected.transition_key.is_empty())
            }
        };
        if needs_transition
            || selected.is_transition
            || selected.animations.len() > 1
            || selected.flags & ((1 << 0) | (1 << 5) | (1 << 7) | (1 << 8) | (1 << 9)) != 0
        {
            state.installed_draw_pose = InstalledDrawObject::AwaitingAdmission;
            state.native_selected_condition = Some(selected);
            return;
        }
        let Some(speed) =
            super::pipeline_drawable_state::deterministic_authored_animation_rate(&selected)
        else {
            state.installed_draw_pose = InstalledDrawObject::AwaitingAdmission;
            state.native_selected_condition = Some(selected);
            return;
        };
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
        let model = match RenderPipeline::ensure_render_model_loaded(
            self.application_assets.as_deref(),
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
                    .get_mut(&(owner, index))
                    .unwrap()
                    .installed_draw_pose = InstalledDrawObject::AwaitingAdmission;
                return;
            }
        };
        let Some(identity) = super::pipeline_drawable_state::frozen_visual_identity_for_draw_model(
            &template, &selected,
        ) else {
            self.drawable_visual_states
                .get_mut(&(owner, index))
                .unwrap()
                .installed_draw_pose = InstalledDrawObject::AwaitingAdmission;
            return;
        };
        let binding = identity.animation.as_ref().and_then(|animation| {
            RenderPipeline::cached_draw_animation_binding(self.application_assets.as_deref(), &model, &animation.hierarchy_animation)
        });
        let state = self
            .drawable_visual_states
            .get_mut(&(owner, index))
            .unwrap();
        let retain = match &state.installed_draw_pose {
            InstalledDrawObject::Ready(pose) => {
                Arc::ptr_eq(&pose.model, &model)
                    && pose.turret_names.matches(&selected.primary_turret)
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
                let start =
                    super::pipeline_drawable_state::authored_animation_start_frame(&selected, frames);
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
            state.installed_draw_pose = InstalledDrawObject::Ready(
                InstalledDrawPose::created(model, world, &selected, binding)
                    .with_asset_scale(state.native_instance_scale),
            );
            // nukeCurrentRender resets the module pause latch on recreation.
            state.animation_paused = false;
        }
        state.native_condition_supported = true;
    }
}

// These are the same dirty/consume phases used by the actual native receiver;
// ownership stays with ObjectVisualState, independent of graphics allocation.
impl ObjectVisualState {
    /// Shared actual transform/support producer for display observers and an
    /// immediate current-object camera query. It never constructs a Drawable.
    pub(super) fn receive_native_source_transform(
        &mut self,
        source: &crate::presentation_frame::UnitRenderInput,
    ) {
        let Some(identity) = source.draw_playback_identity.as_ref() else {
            return;
        };
        self.receive_native_transform_observations(
            identity,
            source.mesh_scale,
            source.float_yaw,
            source.float_pitch,
            source.under_construction,
            source.construction_percent,
            source.max_height_above_position,
        );
    }
    pub(super) fn receive_native_transform_observations(
        &mut self,
        identity: &crate::assets::LiveDrawPlaybackIdentity,
        mesh_scale: f32,
        float_yaw: f32,
        float_pitch: f32,
        under_construction: bool,
        construction_percent: f32,
        max_height_above_position: f32,
    ) {
        let Some((revision, world)) = identity.native_pose() else {
            return;
        };
        self.native_instance_scale = mesh_scale.is_finite().then_some(mesh_scale);
        if self.native_transform_revision == Some(revision) {
            return;
        }
        self.install_native_transform(
            world,
            Some(revision),
            under_construction,
            construction_percent,
            max_height_above_position,
        );
    }

    /// An actual Set_Transform visit is authoritative even when the source
    /// observation revision is unchanged. Never replace it with native_pose().
    pub(super) fn receive_native_installed_transform(
        &mut self,
        identity: &crate::assets::LiveDrawPlaybackIdentity,
        world: Mat4,
        mesh_scale: f32,
        float_yaw: f32,
        float_pitch: f32,
        under_construction: bool,
        construction_percent: f32,
        max_height_above_position: f32,
    ) {
        if self.native_host_identity.as_ref() != Some(identity) {
            return;
        }
        let revision = identity.native_pose().map(|(revision, _)| revision);
        self.native_instance_scale = mesh_scale.is_finite().then_some(mesh_scale);
        self.install_native_transform(
            world,
            revision,
            under_construction,
            construction_percent,
            max_height_above_position,
        );
    }

    fn install_native_transform(
        &mut self,
        world: Mat4,
        revision: Option<u64>,
        under_construction: bool,
        construction_percent: f32,
        max_height_above_position: f32,
    ) {
        self.native_transform_revision = revision;
        self.native_drawable_transform = world;
        if let InstalledDrawObject::Ready(pose) = &mut self.installed_draw_pose {
            let mut adjusted = world;
            if let Some(selected) = self.native_selected_condition.as_ref() {
                if selected.flags
                    & (1 << crate::assets::ACBIT_ADJUST_HEIGHT_BY_CONSTRUCTION_PERCENT)
                    != 0
                {
                    let percent = if under_construction {
                        construction_percent.clamp(0., 1.) * 100.
                    } else {
                        -1.
                    };
                    if let Some(delta) = crate::assets::construction_percent_height_delta(
                        percent,
                        max_height_above_position,
                    ) {
                        adjusted *= Mat4::from_translation(Vec3::Y * delta);
                    }
                }
            }
            pose.install_transform(adjusted);
        }
    }
    pub(super) fn mark_native_condition(
        &mut self,
        bits: u128,
        selected: &crate::assets::AuthoredDrawModel,
    ) {
        if self.native_condition_bits != Some(bits) || !self.native_condition_supported {
            self.native_condition_bits = Some(bits);
            self.native_pending_condition = Some(selected.clone());
        }
    }

    fn take_native_condition_to_install(&mut self) -> Option<crate::assets::AuthoredDrawModel> {
        let selected = self.native_pending_condition.take()?;
        if self.native_instance_scale.is_none() {
            // The previous condition is no longer admitted. When supported
            // source controls return, identical bits must be eligible again.
            self.native_condition_supported = false;
            self.installed_draw_pose = InstalledDrawObject::AwaitingAdmission;
            return None;
        }
        if self.native_condition_supported
            && self.native_selected_condition.as_ref() == Some(&selected)
        {
            return None;
        }
        self.native_condition_supported = false;
        Some(selected)
    }
}

#[cfg(test)]
mod condition_recovery_tests {
    use super::*;

    #[test]
    fn unsupported_source_then_same_bits_requeues_actual_native_condition() {
        let previous = crate::assets::AuthoredDrawModel {
            model_key: "ModelA".into(),
            ..Default::default()
        };
        let next = crate::assets::AuthoredDrawModel {
            model_key: "ModelB".into(),
            ..Default::default()
        };
        let mut state = ObjectVisualState {
            native_instance_scale: Some(1.0),
            native_condition_supported: true,
            native_condition_bits: Some(0),
            native_selected_condition: Some(previous),
            installed_draw_pose: InstalledDrawObject::NoRenderObject,
            ..Default::default()
        };
        let peer = state.clone();
        state.native_instance_scale = None;
        state.mark_native_condition(1, &next);
        assert!(state.take_native_condition_to_install().is_none());
        assert!(!state.native_condition_supported);
        assert!(matches!(
            state.installed_draw_pose,
            InstalledDrawObject::AwaitingAdmission
        ));
        assert!(state.native_pending_condition.is_none());
        state.native_instance_scale = Some(1.0);
        // Main's actual GameClient observer emits ConditionsMarked every sync.
        // No bit change, recreation, or alternate camera is needed to recover.
        state.mark_native_condition(1, &next);
        assert_eq!(state.take_native_condition_to_install(), Some(next));
        assert!(
            !state.native_condition_supported,
            "asset install still must complete"
        );
        assert!(state.native_pending_condition.is_none());
        assert!(peer.native_condition_supported);
        assert!(matches!(
            peer.installed_draw_pose,
            InstalledDrawObject::NoRenderObject
        ));
    }
}
