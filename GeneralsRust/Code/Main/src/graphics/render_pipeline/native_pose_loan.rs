//! Bounded access to the actual installed Draw state during synchronous callbacks.
//! This is a field loan, not another renderer, animation clock or pose cache.
use super::*;
use installed_draw_pose::{NativeCameraPoseUnavailable, ObjectVisualState};

pub struct NativeInstalledPoseLoan<'a> {
    #[cfg(feature = "game_client")]
    pub(super) pending_native_props: &'a mut game_client::terrain::terrain_visual::TerrainPropQueue,
    #[cfg(feature = "game_client")]
    pub(super) pending_native_trees: &'a mut game_client::terrain::terrain_visual::TerrainTreeQueue,
    pub(super) native_drawable_lifetimes:
        &'a mut game_engine::common::fast_hash::FxHashMap<NativeDrawOwnerId, native_draw_world::NativeDrawableLifetime>,
    pub(super) live_draw_playback:
        &'a mut HashMap<crate::assets::LiveDrawPlaybackKey, crate::assets::LiveDrawPlayback>,
    pub(super) drawable_visual_states: &'a mut native_draw_world::DrawableVisualStates,
    pub(super) failed_model_resolutions: &'a mut pipeline_collect::FailedModelResolutions,
    pub(super) application_assets: Option<Arc<Mutex<crate::assets::AssetManager>>>,
    pub(super) pending_client_drawable_restore:
        &'a mut Option<crate::save_load::snapshot::ClientDrawableWorldSnapshot>,
    pub(super) pending_client_drawable_imports:
        &'a mut HashMap<(u32, u32), crate::save_load::snapshot::ClientDrawableStateSnapshot>,
    pub(super) animation_sync_ms: u32,
}
impl RenderPipeline {
    #[cfg(feature = "game_client")]
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn admit_native_drawable_binding(
        &mut self,
        identity: &crate::assets::LiveDrawPlaybackIdentity,
        key: game_client::core::PresentationDirectDrawableBindingKey,
    ) -> Result<(), &'static str> {
        self.borrow_native_installed_poses()
            .admit_binding(identity, key)
    }

    /// Admit every post-sync Client binding of one presentation update.
    /// Same per-binding validation as `admit_native_drawable_binding`, through
    /// one loan; each binding touches only its owner's module range.
    #[cfg(feature = "game_client")]
    pub(crate) fn admit_native_drawable_bindings<'i>(
        &mut self,
        bindings: impl IntoIterator<
            Item = (
                &'i crate::assets::LiveDrawPlaybackIdentity,
                game_client::core::PresentationDirectDrawableBindingKey,
            ),
        >,
    ) -> Result<(), &'static str> {
        let mut loan = self.borrow_native_installed_poses();
        for (identity, key) in bindings {
            loan.admit_binding(identity, key)?;
        }
        Ok(())
    }

    pub(crate) fn borrow_native_installed_poses(&mut self) -> NativeInstalledPoseLoan<'_> {
        self.native_draw_world
            .borrow_installed_poses(&mut self.failed_model_resolutions, self.animation_sync_ms, self.forward_pass.application_assets.clone())
    }
}
impl NativeInstalledPoseLoan<'_> {
    #[cfg(feature = "game_client")]
    pub(crate) fn bind_native_prop_epoch(&mut self, epoch: u64) {
        self.pending_native_props.bind_epoch(epoch);
        self.pending_native_trees.bind_epoch(epoch);
    }

    #[cfg(feature = "game_client")]
    pub(crate) fn admit_pending_native_trees(
        &mut self,
        terrain: &mut game_client::terrain::terrain_visual::TerrainVisualImpl,
        rng: &mut impl game_client::terrain::TreeRandom,
    ) {
        let Some(assets) = self.application_assets.as_ref() else {
            return;
        };
        for request in self.pending_native_trees.drain() {
            Self::admit_tree_with_assets(terrain, request, assets, rng);
        }
        let trees = terrain.tree_buffer_mut();
        if trees.need_to_update_texture() {
            let mut assets = assets.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            trees.hydrate_tree_texture_sources(|path| {
                assets.archive_file_system_mut().open_file_sync(path).ok()
            });
        }
    }

    /// The authored tree callback borrows assets independently of the
    /// renderer pose callbacks delivered during the same transient birth.
    #[cfg(feature = "game_client")]
    pub(crate) fn tree_assets(
        &self,
    ) -> Option<std::sync::Arc<std::sync::Mutex<crate::assets::AssetManager>>> {
        self.application_assets.clone()
    }

    #[cfg(feature = "game_client")]
    pub(crate) fn admit_tree_with_assets(
        terrain: &mut game_client::terrain::terrain_visual::TerrainVisualImpl,
        request: gamelogic::object::draw::w3d_tree_draw::TerrainTreeAddRequest,
        assets: &std::sync::Mutex<crate::assets::AssetManager>,
        rng: &mut impl game_client::terrain::TreeRandom,
    ) {
        terrain.admit_authored_tree(
            request,
            |name| {
                let mut assets = assets.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                super::native_tree_mesh::load_native_tree_mesh(&mut assets, name)
            },
            rng,
        );
    }

    pub(crate) fn reborrow(&mut self) -> NativeInstalledPoseLoan<'_> {
        NativeInstalledPoseLoan {
            #[cfg(feature = "game_client")]
            pending_native_props: &mut *self.pending_native_props,
            #[cfg(feature = "game_client")]
            pending_native_trees: &mut *self.pending_native_trees,
            native_drawable_lifetimes: &mut *self.native_drawable_lifetimes,
            live_draw_playback: &mut *self.live_draw_playback,
            drawable_visual_states: &mut *self.drawable_visual_states,
            failed_model_resolutions: &mut *self.failed_model_resolutions,
            application_assets: self.application_assets.clone(),
            pending_client_drawable_restore: &mut *self.pending_client_drawable_restore,
            pending_client_drawable_imports: &mut *self.pending_client_drawable_imports,
            animation_sync_ms: self.animation_sync_ms,
        }
    }
    pub(crate) fn has_drawable_lifetime(&self, object: u32) -> bool {
        self.native_drawable_lifetimes
            .contains_key(&NativeDrawOwnerId::Actor(object))
    }

    /// The Drawable itself can exist even when a Draw module has no mesh.
    #[cfg(feature = "game_client")]
    pub(crate) fn matches_object_drawable_binding(
        &self,
        identity: &crate::assets::LiveDrawPlaybackIdentity,
        key: game_client::core::PresentationDirectDrawableBindingKey,
    ) -> bool {
        key.object_id == identity.object_id()
            && self.native_drawable_lifetimes
                .get(&NativeDrawOwnerId::Actor(identity.object_id()))
                .is_some_and(|lifetime| lifetime.identity == *identity && lifetime.binding == Some(key))
    }

    pub(crate) fn current_animation_sync_ms(&self) -> u32 {
        self.animation_sync_ms
    }

    #[cfg(feature = "game_client")]
    pub(crate) fn installed_track_width(
        &mut self,
        identity: &crate::assets::LiveDrawPlaybackIdentity,
        key: game_client::core::PresentationDirectDrawableBindingKey,
        module_index: u32,
    ) -> Result<f32, NativeCameraPoseUnavailable> {
        use game_client::terrain::terrain_tracks::compute_track_spacing;
        use installed_draw_pose::InstalledDrawObject;
        use NativeCameraPoseUnavailable::*;

        self.validate_binding(identity, key, module_index)?;
        let state = self.drawable_visual_states
            .get_mut(&(identity.owner_id(), module_index))
            .ok_or(UnadmittedDrawableLifecycle)?;
        if state.native_instance_scale != Some(1.0) || !state.native_condition_supported {
            return Err(UnadmittedDrawableLifecycle);
        }
        let pose = match &state.installed_draw_pose {
            InstalledDrawObject::Ready(pose) => pose,
            InstalledDrawObject::NoRenderObject => return Err(NoRenderObject),
            InstalledDrawObject::AwaitingAdmission => return Err(UnadmittedDrawableLifecycle),
        };

        let has_pivot = |name| {
            pose.model.source_named_bone_index(name)
                .map(|index| index.is_some_and(|index| index != 0))
                .map_err(Bone)
        };
        // C++ only evaluates bone transforms after both indices resolve.
        if !has_pivot("TREADFX01")? || !has_pivot("TREADFX02")? {
            return Ok(compute_track_spacing(None, None));
        }
        let left = state.installed_client_bone_transform(self.animation_sync_ms, "TREADFX01")?
            .ok_or(UnadmittedDrawableLifecycle)?;
        let right = state.installed_client_bone_transform(self.animation_sync_ms, "TREADFX02")?
            .ok_or(UnadmittedDrawableLifecycle)?;
        Ok(compute_track_spacing(
            Some(left.w_axis.truncate()),
            Some(right.w_axis.truncate()),
        ))
    }
    /// Stamp the actual post-sync Client lifetime on the existing Draw owner.
    /// A changed key requires a real Destroy/Creating producer, never relabeling.
    #[cfg(feature = "game_client")]
    pub(crate) fn admit_binding(
        &mut self,
        identity: &crate::assets::LiveDrawPlaybackIdentity,
        key: game_client::core::PresentationDirectDrawableBindingKey,
    ) -> Result<(), &'static str> {
        let owner = NativeDrawOwnerId::Actor(identity.object_id());
        let modules: Vec<u32> = self
            .drawable_visual_states
            .range(native_draw_world::owner_module_keys(owner))
            .map(|((_, module), _)| *module)
            .collect();
        self.admit_binding_for_modules(identity, key, &modules)
    }

    /// `admit_binding` with the owner's Draw-module state keys supplied.
    #[cfg(feature = "game_client")]
    fn admit_binding_for_modules(
        &mut self,
        identity: &crate::assets::LiveDrawPlaybackIdentity,
        key: game_client::core::PresentationDirectDrawableBindingKey,
        modules: &[u32],
    ) -> Result<(), &'static str> {
        if key.object_id != identity.object_id() {
            return Err("Client binding object mismatch");
        }
        let owner = NativeDrawOwnerId::Actor(identity.object_id());
        let lifetime = self
            .native_drawable_lifetimes
            .get(&NativeDrawOwnerId::Actor(identity.object_id()))
            .ok_or("Client binding has no actual native Drawable constructor")?;
        if lifetime.identity != *identity
            || lifetime.binding.is_some_and(|installed| installed != key)
        {
            return Err("Client binding belongs to another native Drawable constructor");
        }
        if modules.iter().any(|module| {
            self.drawable_visual_states
                .get(&(owner, *module))
                .is_some_and(|state| {
                    state.native_host_identity.as_ref() != Some(identity)
                        || state
                            .native_client_binding
                            .is_some_and(|installed| installed != key)
                })
        }) {
            return Err("Client binding did not reach actual native constructor owner");
        }
        self.native_drawable_lifetimes
            .get_mut(&NativeDrawOwnerId::Actor(identity.object_id()))
            .unwrap()
            .binding = Some(key);
        for module in modules {
            if let Some(state) = self.drawable_visual_states.get_mut(&(owner, *module)) {
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
            .get(&NativeDrawOwnerId::Actor(identity.object_id()))
            .ok_or("native physical retirement has no actual constructor")?;
        if key.object_id != identity.object_id()
            || lifetime.identity != *identity
            || lifetime.binding != Some(key)
        {
            return Err("native physical retirement identity/binding mismatch");
        }
        if self
            .drawable_visual_states
            .range(native_draw_world::owner_module_keys(NativeDrawOwnerId::Actor(
                identity.object_id(),
            )))
            .any(|((object, _), state)| {
                *object == NativeDrawOwnerId::Actor(identity.object_id())
                    && (state.native_host_identity.as_ref() != Some(identity)
                        || state.native_client_binding != Some(key))
            })
        {
            // Diagnose the first anonymous/stale module without mutating the owner.
            let mut invalid = self
                .drawable_visual_states
                .iter()
                .filter(|((object, _), state)| {
                    *object == NativeDrawOwnerId::Actor(identity.object_id())
                        && (state.native_host_identity.as_ref() != Some(identity)
                            || state.native_client_binding != Some(key))
                })
                .collect::<Vec<_>>();
            invalid.sort_by_key(|((_, module), _)| *module);
            for ((object, module), state) in invalid.into_iter().take(8) {
                log::error!(
                    "native retirement module mismatch object={:?} module={} template={} host_identity_present={} host_identity_matches={} binding={:?} expected_binding={:?} pending_model={} selected_model={}",
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
        // Retirement validates the constructor and each installed module's
        // identity and binding. A pose query additionally needs this module.
        if !self.drawable_visual_states.contains_key(&(
            NativeDrawOwnerId::Actor(identity.object_id()),
            module_index,
        )) {
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
            .get(&NativeDrawOwnerId::Actor(id))
            .is_none_or(|lifetime| lifetime.identity != *identity)
        {
            return Err(NativeCameraPoseUnavailable::UnadmittedDrawableLifecycle);
        }
        let state = self
            .drawable_visual_states
            .get(&(NativeDrawOwnerId::Actor(id), module_index))
            .ok_or(NativeCameraPoseUnavailable::UnadmittedDrawableLifecycle)?;
        if state.native_host_identity.as_ref() != Some(identity)
            || self
                .drawable_visual_states
                .range(native_draw_world::owner_module_keys(NativeDrawOwnerId::Actor(id)))
                .any(|((object, _), state)| {
                    *object == NativeDrawOwnerId::Actor(id)
                        && state.native_host_identity.as_ref() != Some(identity)
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
        let owner = NativeDrawOwnerId::Actor(identity.object_id());
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
            !self.drawable_visual_states.contains_key(&(
                NativeDrawOwnerId::Actor(identity.object_id()),
                model.module_index,
            ))
        }) || self
            .drawable_visual_states
            .range(native_draw_world::owner_module_keys(NativeDrawOwnerId::Actor(
                identity.object_id(),
            )))
            .any(|((object, _), state)| {
                *object == NativeDrawOwnerId::Actor(identity.object_id())
                    && state.native_source_template != source.template_name
            })
        {
            return Err(NativeCameraPoseUnavailable::UnadmittedDrawableLifecycle);
        }
        for (_, state) in self
            .drawable_visual_states
            .range_mut(native_draw_world::owner_module_keys(owner))
        {
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
        for model in models {
            let state = self
                .drawable_visual_states
                .get_mut(&(
                    NativeDrawOwnerId::Actor(identity.object_id()),
                    model.module_index,
                ))
                .ok_or(NativeCameraPoseUnavailable::UnadmittedDrawableLifecycle)?;
            state.mark_native_condition(source.condition_bits, &model);
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
                .get_mut(&(NativeDrawOwnerId::Actor(identity.object_id()), module))
                .ok_or(NativeCameraPoseUnavailable::UnadmittedDrawableLifecycle)?;
            if state.native_instance_scale.is_none() || !state.native_condition_supported {
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
                // C++ applies the Drawable scale to the local query transform
                // after sampling the RenderObj's already scaled HTree.
                transforms.push(Mat4::from_scale(Vec3::splat(source.mesh_scale)) * local);
                count += 1;
                if count >= limit {
                    break;
                }
            }
        }
        Ok(transforms)
    }

    /// W3DModelDraw.cpp:3672–3740. The temporary detonation weapon fires
    /// PRIMARY barrel zero. Visit each Draw module until one handles the list.
    #[cfg(feature = "game_client")]
    pub(crate) fn begin_detonation_draw(
        &mut self,
        source: &crate::game_logic::NativeSlaveSource,
        drawable: game_client::core::game_client::PresentationDirectDrawableState,
        pre_physics_pose: Option<Mat4>,
        has_fx: bool,
        graphics: &mut GraphicsSystem,
    ) -> Result<Option<crate::game_logic::NativeDetonationFx>, NativeCameraPoseUnavailable> {
        use installed_draw_pose::InstalledDrawObject;
        use NativeCameraPoseUnavailable::*;

        let key = drawable.binding_key;
        let identity = &source.identity;
        let (_, actor_pose) = identity.native_pose().ok_or(UnsupportedSourceControls)?;
        let modules = self.receive_current_bone_source(source, key, graphics)?;
        let slot = gamelogic::common::WeaponSlotType::Primary as u8;
        for module_index in modules {
            let state = self.drawable_visual_states.get_mut(&(identity.owner_id(), module_index))
                .ok_or(UnadmittedDrawableLifecycle)?;
            if state.native_instance_scale.is_none() || !state.native_condition_supported {
                return Err(UnsupportedSourceControls);
            }
            let (Some(selected), InstalledDrawObject::Ready(pose)) = (
                &state.native_selected_condition, &state.installed_draw_pose,
            ) else { continue };
            let Some(topology) = pose.model
                .weapon_barrel_topology_for_authored_bindings(&selected.weapon_bone_bindings)
            else { continue };
            let Some(barrel) = topology.slot(slot).and_then(|barrels| barrels.first())
            else { continue };
            if let Some(pivot) = barrel.fire_fx_pivot_index.filter(|_| has_fx) {
                let fx_pose = if drawable.scene_effectively_hidden {
                    pre_physics_pose.unwrap_or(actor_pose)
                } else {
                    let mut world = pose.world;
                    if let Some(historical_pose) = pre_physics_pose {
                        // Keep the Draw adjustments at the captured impact pose.
                        world = historical_pose * actor_pose.inverse() * world;
                        if !world.is_finite() {
                            return Err(UnsupportedSourceControls);
                        }
                    }
                    let local = state.installed_client_bone_at_index(self.animation_sync_ms, pivot as usize)?;
                    world * crate::game_logic::host_fx_list_dispatch::host_to_leftover_mat4(local)
                };
                return Ok(Some(crate::game_logic::NativeDetonationFx {
                    pose: fx_pose,
                    module_index,
                    condition_bits: state.native_condition_bits,
                }));
            }
            let kinematics = selected.recoil_kinematics.clone();
            pipeline_drawable_state::start_weapon_barrel_recoil(
                state, &topology, &kinematics, slot, 0,
            );
        }
        Ok(None)
    }

    #[cfg(feature = "game_client")]
    pub(crate) fn finish_detonation_draw(
        &mut self,
        identity: &crate::assets::LiveDrawPlaybackIdentity,
        key: game_client::core::PresentationDirectDrawableBindingKey,
        fx: crate::game_logic::NativeDetonationFx,
    ) {
        use installed_draw_pose::InstalledDrawObject;

        if self.validate_binding(identity, key, fx.module_index).is_err() {
            return;
        }
        let Some(state) = self.drawable_visual_states.get_mut(&(identity.owner_id(), fx.module_index))
        else { return };
        if state.native_condition_bits != fx.condition_bits {
            return;
        }
        let (Some(selected), InstalledDrawObject::Ready(pose)) = (
            &state.native_selected_condition, &state.installed_draw_pose,
        ) else { return };
        let Some(topology) = pose.model
            .weapon_barrel_topology_for_authored_bindings(&selected.weapon_bone_bindings)
        else { return };
        let kinematics = selected.recoil_kinematics.clone();
        pipeline_drawable_state::start_weapon_barrel_recoil(
            state, &topology, &kinematics, gamelogic::common::WeaponSlotType::Primary as u8, 0,
        );
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
                self.application_assets.as_deref(),
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
                    RenderPipeline::cached_draw_animation_binding(self.application_assets.as_deref(), &model, &animation.name)
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
            .get_mut(&(NativeDrawOwnerId::Actor(id.0), module_index))
            .ok_or(NativeCameraPoseUnavailable::UnadmittedDrawableLifecycle)?;
        if state.native_instance_scale != Some(1.0) || !state.native_condition_supported {
            return Err(NativeCameraPoseUnavailable::UnadmittedDrawableLifecycle);
        }
        state.installed_camera_pose(self.animation_sync_ms, bone_name)
    }
}

#[cfg(all(test, feature = "game_client"))]
#[path = "native_pose_loan_tests.rs"]
mod tests;
