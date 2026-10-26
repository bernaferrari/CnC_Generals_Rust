//! Lexical access to the actual installed Draw owner during a view build.
use super::*;
#[cfg(feature = "game_client")]
use crate::game_logic::NativeFactoryDrawableSource;
use crate::game_logic::{NativeSlaveSource, NativeViewEffects};
use crate::graphics::render_pipeline::native_pose_loan::NativeInstalledPoseLoan;
use crate::presentation_frame::camera::build::SlaveCameraSample;

#[cfg(feature = "game_client")]
pub(super) struct NativeRendererViewEffects<'a> {
    poses: NativeInstalledPoseLoan<'a>,
    graphics: &'a mut GraphicsSystem,
    drawables: game_client::core::game_client::NativeDrawableOwner<'a>,
    policy: game_client::core::game_client::ReceivingDrawablePolicy,
    retirement: &'a mut dyn game_client::core::game_client::NativeDrawableRetirement,
    host_epoch: u64,
    local_player_id: u32,
}
#[cfg(feature = "game_client")]
impl<'a> NativeRendererViewEffects<'a> {
    pub(super) fn new(
        poses: NativeInstalledPoseLoan<'a>,
        graphics: &'a mut GraphicsSystem,
        drawables: game_client::core::game_client::NativeDrawableOwner<'a>,
        host_epoch: u64,
        local_player_id: u32,
        policy: game_client::core::game_client::ReceivingDrawablePolicy,
        retirement: &'a mut dyn game_client::core::game_client::NativeDrawableRetirement,
    ) -> Self {
        Self {
            poses,
            graphics,
            drawables,
            policy,
            retirement,
            host_epoch,
            local_player_id,
        }
    }
    pub(super) fn admit_map_particle_definitions(
        &mut self,
        sources: &[String],
    ) -> Result<(), String> {
        self.drawables
            .particle_systems_mut()
            .admit_map_definition_overlays(sources)
    }
    pub(super) fn discard_map_attempt(&mut self) -> Result<(), String> {
        self.retirement
            .retire(&mut self.drawables, None)
            .map_err(|error| error.to_string())?;
        self.poses.clear_world();
        Ok(())
    }
    pub(super) fn admit_restored_object_drawables(
        &mut self,
        world: &mut crate::game_logic::GameLogic,
    ) -> Result<(), String> {
        if !world.native_drawable_birth_context().in_load_game {
            return Err(
                "native snapshot Drawable reconstruction requires the driving saved-load guard"
                    .into(),
            );
        }
        self.admit_lighting_policy(world);
        // Retain prepared terrain and pending map audio. Only the superseded
        // base-map native object world is retired, using its actual services.
        self.retirement
            .retire(&mut self.drawables, None)
            .map_err(|error| error.to_string())?;
        self.poses.clear_world();
        // Match this port's existing restore.rs object-construction order.
        // Original archive visitation order is a separate persistence obligation.
        let mut ids: Vec<_> = world.host_objects().registered_keys().copied().collect();
        ids.sort_unstable_by_key(|id| id.0);
        for id in ids {
            world
                .admit_factory_drawable_with_effects(
                    id,
                    &mut crate::game_logic::ScriptPresentationEffects {
                        view: Some(self),
                        ..Default::default()
                    },
                )
                .map_err(|error| error.to_string())?;
        }
        world
            .restore_jet_lockon_drawables_with_effects(
                &mut crate::game_logic::ScriptPresentationEffects {
                    view: Some(self),
                    ..Default::default()
                },
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    }
    fn retire_object_drawable(
        &mut self,
        identity: &crate::assets::LiveDrawPlaybackIdentity,
    ) -> Result<(), String> {
        let Some(key) = self
            .drawables
            .binding_key(self.host_epoch, identity.object_id())
        else {
            if self.drawables.has_object_drawable(identity.object_id())
                || self.poses.has_drawable_lifetime(identity.object_id())
            {
                return Err("native physical retirement has an existing receiver without its current Client binding".into());
            }
            // Genuine prebirth absence has neither an actual CPU association
            // nor a constructor lifetime. Never invent a key to delete it.
            return Ok(());
        };
        self.poses
            .validate_object_retirement(identity, key)
            .map_err(str::to_owned)?;
        let cpu = self
            .retirement
            .retire(&mut self.drawables, Some(key.drawable_id));
        let draw = if self
            .drawables
            .binding_key(self.host_epoch, identity.object_id())
            .is_none()
        {
            self.poses
                .retire_object_after_client(identity, key)
                .map_err(str::to_owned)
        } else {
            Ok(())
        };
        match (cpu, draw) {
            (Ok(()), Ok(())) => Ok(()),
            (cpu, draw) => Err(format!(
                "native physical retirement CPU: {}; Draw: {}",
                cpu.err()
                    .map(|error| error.to_string())
                    .unwrap_or_else(|| "completed".into()),
                draw.err().unwrap_or_else(|| "completed".into())
            )),
        }
    }
    /// Explicit receiving native boundary for the canonical early HP callback.
    /// Its caller supplies the session TOD/map gate and captured audio service.
    pub(super) fn react_owned_body_state(
        &mut self,
        identity: &crate::assets::LiveDrawPlaybackIdentity,
        state: gamelogic::common::types::BodyDamageType,
        loading_map: bool,
        time_of_day: Option<game_client::system::TimeOfDay>,
        audio: Option<&mut dyn game_client::drawable::drawable::NativeAmbientAudio>,
        locality: &game_engine::common::audio::AudioSubmissionLocality,
        policy: &game_engine::common::audio::script_policy::ScriptAudioPolicy,
    ) -> Result<bool, String> {
        let Some(key) = self
            .drawables
            .binding_key(self.host_epoch, identity.object_id())
        else {
            if self.drawables.has_object_drawable(identity.object_id())
                || self.poses.has_drawable_lifetime(identity.object_id())
            {
                return Err("native Body visual receiver lost its installed binding".into());
            }
            return Ok(false); // CPP Object::getDrawable == NULL before birth.
        };
        // Existing pure validation checks constructor identity and full binding;
        // it performs no retirement or object/registry mutation.
        self.poses
            .validate_object_retirement(identity, key)
            .map_err(str::to_owned)?;
        let time_of_day = time_of_day.ok_or_else(|| {
            "native Body visual receiver requires receiving time of day".to_owned()
        })?;
        self.drawables
            .body_state_changed_with_audio(
                key,
                state,
                loading_map,
                time_of_day,
                audio,
                locality,
                policy,
            )
            .map_err(|error| error.to_string())?;
        Ok(true)
    }

    pub(super) fn admit_lighting_policy(&mut self, world: &crate::game_logic::GameLogic) {
        self.policy.use_draw_module_lod = world.lighting_rules().use_draw_module_lod;
        self.policy.use_tree_sway = world.lighting_rules().use_tree_sway;
        self.policy.time_of_day = world.native_lighting_time_of_day();
    }
    pub(super) fn on_level_start(&mut self) -> Result<(), String> {
        let tod = self
            .policy
            .time_of_day
            .ok_or_else(|| "native level start requires receiving time of day".to_owned())?;
        self.drawables
            .on_level_start_at(tod)
            .map_err(|error| error.to_string())
    }
}
#[cfg(feature = "game_client")]
impl NativeViewEffects for NativeRendererViewEffects<'_> {
    fn visit_owned_ambient_audio_handles(&self, visit: &mut dyn FnMut(u32)) {
        self.drawables.visit_owned_ambient_audio_handles(visit);
    }

    fn create_jet_lockon(
        &mut self,
        source: &crate::game_logic::NativeAuxiliarySource,
        content: &mut game_engine::common::thing::module_factory::ModuleFactory,
    ) -> Result<game_client::core::game_client::NativeAuxiliaryReceipt, String> {
        if source.epoch != self.host_epoch {
            return Err("lock-on source epoch mismatch".into());
        }
        let poses = &mut self.poses;
        let graphics = &mut *self.graphics;
        let sync = poses.current_animation_sync_ms();
        self.drawables
            .create_auxiliary_drawable(
                std::sync::Arc::clone(&source.template),
                content,
                source.scale,
                self.policy,
                source.birth,
                source.frame,
                sync,
                |mutation| admit_auxiliary_mutation(poses, graphics, source, mutation),
            )
            .map_err(|error| error.to_string())
    }
    fn position_jet_lockon(
        &mut self,
        source: &crate::game_logic::NativeAuxiliarySource,
        receipt: &game_client::core::game_client::NativeAuxiliaryReceipt,
        position: [f32; 3],
        orientation: Option<f32>,
    ) -> Result<(), String> {
        if source.epoch != self.host_epoch {
            return Err("lock-on source epoch mismatch".into());
        }
        let poses = &mut self.poses;
        let graphics = &mut *self.graphics;
        self.drawables
            .position_auxiliary_drawable(receipt, position, orientation, |mutation| {
                admit_auxiliary_mutation(poses, graphics, source, mutation)
            })
            .map_err(|error| error.to_string())
    }
    fn set_jet_lockon_scene_flags(
        &mut self,
        source: &crate::game_logic::NativeAuxiliarySource,
        receipt: &game_client::core::game_client::NativeAuxiliaryReceipt,
        hidden: bool,
        shroud_object: Option<u32>,
    ) -> Result<(), String> {
        if source.epoch != self.host_epoch {
            return Err("lock-on visibility source epoch mismatch".into());
        }
        self.drawables
            .set_auxiliary_scene_flags(receipt, hidden, shroud_object)
            .map_err(|error| error.to_string())
    }
    fn retire_jet_lockon(
        &mut self,
        source: &crate::game_logic::NativeAuxiliarySource,
        receipt: &game_client::core::game_client::NativeAuxiliaryReceipt,
    ) -> Result<(), String> {
        if source.epoch != self.host_epoch {
            return Err("lock-on source epoch mismatch".into());
        }
        let poses = &mut self.poses;
        let graphics = &mut *self.graphics;
        self.drawables
            .retire_auxiliary_drawable(receipt, |mutation| {
                admit_auxiliary_mutation(poses, graphics, source, mutation)
            })
            .map_err(|error| error.to_string())
    }

    fn spawn_tracer(
        &mut self,
        template: &game_engine::common::thing::ThingTemplate,
        request: &game_client::fx_list::FxTracerRequest<'_>,
    ) -> Result<(), String> {
        self.drawables
            .create_owned_tracer(template, request)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
    fn create_ray(
        &mut self,
        template: &game_engine::common::thing::ThingTemplate,
        request: &game_client::fx_list::FxRayRequest<'_>,
    ) -> Result<(), String> {
        self.drawables
            .create_owned_ray(template, request.primary, request.secondary)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    fn create_light_pulse(
        &mut self,
        pulse: game_client::fx_list::DisplayLightPulse,
    ) -> Result<bool, String> {
        Ok(self.drawables.create_light_pulse(pulse))
    }

    fn particle_systems_mut(
        &mut self,
    ) -> Option<&mut game_client::effects::particle_manager::ParticleSystemManager> {
        Some(self.drawables.particle_systems_mut())
    }

    fn current_client_bone_transforms(
        &mut self,
        source: &NativeSlaveSource,
        bone_name: &str,
        start: i32,
        max_bones: usize,
    ) -> Result<Vec<glam::Mat4>, String> {
        let key = self
            .drawables
            .binding_key(self.host_epoch, source.identity.object_id())
            .ok_or_else(|| {
                "native FX bone query has no matching receiving Drawable binding".to_owned()
            })?;
        self.poses
            .sample_current_client_bone_transforms(
                source,
                key,
                bone_name,
                start,
                max_bones,
                self.graphics,
            )
            .map_err(|error| format!("native FX bone query unavailable: {error:?}"))
    }

    fn drawable_model_condition_bits(
        &mut self,
        source: &NativeSlaveSource,
    ) -> Result<Option<u128>, String> {
        let key = self
            .drawables
            .binding_key(self.host_epoch, source.identity.object_id())
            .ok_or_else(|| "native condition query has no receiving Drawable binding".to_owned())?;
        self.poses
            .validate_object_retirement(&source.identity, key)
            .map_err(str::to_owned)?;
        self.drawables
            .model_condition_bits(key)
            .map(Some)
            .map_err(|error| error.to_string())
    }
    fn drawable_instance_transform(
        &mut self,
        source: &NativeSlaveSource,
    ) -> Result<Option<glam::Mat4>, String> {
        let key = self
            .drawables
            .binding_key(self.host_epoch, source.identity.object_id())
            .ok_or_else(|| "native instance query has no receiving Drawable".to_owned())?;
        self.poses
            .validate_object_retirement(&source.identity, key)
            .map_err(str::to_owned)?;
        self.drawables
            .instance_transform(key)
            .map(|matrix| Some(matrix.into()))
            .map_err(|error| error.to_string())
    }
    fn set_drawable_instance_transform(
        &mut self,
        source: &NativeSlaveSource,
        matrix: Option<glam::Mat4>,
    ) -> Result<bool, String> {
        let key = self
            .drawables
            .binding_key(self.host_epoch, source.identity.object_id())
            .ok_or_else(|| "native instance mutation has no receiving Drawable".to_owned())?;
        self.poses
            .validate_object_retirement(&source.identity, key)
            .map_err(str::to_owned)?;
        self.drawables
            .set_instance_transform(key, matrix.map(game_client::drawable::Matrix4::from))
            .map_err(|error| error.to_string())?;
        Ok(true)
    }

    fn pristine_client_bone_transforms(
        &mut self,
        source: &NativeSlaveSource,
        bone_name: &str,
        start: i32,
        max_bones: usize,
    ) -> Result<Vec<glam::Mat4>, String> {
        let key = self
            .drawables
            .binding_key(self.host_epoch, source.identity.object_id())
            .ok_or_else(|| {
                "native pristine bone query has no matching receiving Drawable binding".to_owned()
            })?;
        self.poses
            .sample_pristine_client_bone_transforms(
                source,
                key,
                bone_name,
                start,
                max_bones,
                self.graphics,
            )
            .map_err(|error| format!("native pristine bone query unavailable: {error:?}"))
    }

    fn projected_radius_allocator(
        &mut self,
    ) -> Option<&mut dyn crate::game_logic::NativeProjectedRadiusAllocator> {
        Some(&mut self.drawables)
    }

    fn body_visual_changed(
        &mut self,
        change: &crate::game_logic::NativeBodyVisualChange,
        audio: Option<&mut dyn game_client::drawable::drawable::NativeAmbientAudio>,
    ) -> Result<bool, String> {
        self.react_owned_body_state(
            &change.identity,
            change.state,
            change.loading_map,
            change.time_of_day,
            audio,
            &change.locality,
            &change.audio_policy,
        )
    }
    fn local_player_id(&self) -> u32 {
        self.local_player_id
    }
    fn retire_physical_object_drawable(
        &mut self,
        source: &crate::game_logic::NativeObjectRetirement<'_>,
    ) -> Result<(), String> {
        let id = source.object.id;
        if source.identity.object_id() != id.0
            || source.live_objects.contains_key(&id)
            || source.live_objects.ordered_ids().contains(&id)
        {
            return Err(
                "physical Drawable retirement requires an unregistered owned Object".into(),
            );
        }
        self.retire_object_drawable(&source.identity)
    }
    fn host_epoch(&self) -> u64 {
        self.host_epoch
    }
    fn admit_factory_drawable(
        &mut self,
        source: NativeFactoryDrawableSource,
        birth: game_client::core::game_client::NativeDrawableBirthContext,
        content: &mut game_engine::common::thing::module_factory::ModuleFactory,
    ) -> Result<(), String> {
        use game_client::core::{GameClientError, game_client::PresentationDrawableMutation};
        let NativeFactoryDrawableSource { drawable, render } = source;
        if drawable.host_epoch != self.host_epoch {
            return Err("factory Drawable source belongs to another native epoch".into());
        }
        let sync_ms = self.poses.current_animation_sync_ms();
        let poses = &mut self.poses;
        let graphics = &mut *self.graphics;
        let key = self
            .drawables
            .admit_semantic_drawable_with_content(
                drawable,
                birth,
                sync_ms,
                content,
                self.policy,
                self.retirement,
                |event| {
                    poses
                        .receive_native_factory_mutation(event, &render, graphics)
                        .map_err(GameClientError::SubsystemError)
                },
            )
            .map_err(|error| error.to_string())?;
        if let Err(error) = self.poses.admit_binding(&render.identity, key) {
            let cleanup = self
                .retirement
                .retire(&mut self.drawables, Some(key.drawable_id));
            let renderer_cleanup = if self
                .drawables
                .binding_key(self.host_epoch, key.object_id)
                .is_none()
            {
                self.poses.receive_native_factory_mutation(
                    PresentationDrawableMutation::Destroyed {
                        host_epoch: self.host_epoch,
                        object_id: key.object_id,
                    },
                    &render,
                    self.graphics,
                )
            } else {
                Ok(())
            };
            return Err(format!(
                "factory Drawable binding failed: {error}; receiver retirement: {}; renderer retirement: {}",
                cleanup
                    .err()
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| "completed".into()),
                renderer_cleanup.err().unwrap_or_else(|| "completed".into())
            ));
        }
        Ok(())
    }
    fn sample_slave(&mut self, source: &NativeSlaveSource, bone: &str) -> SlaveCameraSample {
        sample_installed_slave(
            &mut self.poses,
            self.graphics,
            source,
            bone,
            self.drawables
                .binding_key(self.host_epoch, source.identity.object_id()),
        )
    }
}
/// Same actual source/dirty/restore/query body for script, display and filter.
/// The PostRender caller supplies an immutable association fact only after
/// proving no binding producer can execute during that render operation.
#[cfg(feature = "game_client")]
pub(super) fn sample_installed_slave(
    poses: &mut NativeInstalledPoseLoan<'_>,
    graphics: &mut GraphicsSystem,
    source: &NativeSlaveSource,
    bone: &str,
    binding_key: Option<game_client::core::PresentationDirectDrawableBindingKey>,
) -> SlaveCameraSample {
    let Some(binding_key) = binding_key else {
        return SlaveCameraSample::MissingDrawable;
    };
    match source
        .receiving_draw_models
        .first_object_draw(source.condition_bits)
    {
        Ok(None) => return SlaveCameraSample::NoInterface,
        Err(()) => return SlaveCameraSample::Unavailable("unsupported current Draw condition"),
        Ok(Some(_)) => {}
    }
    match poses.sample_current_source(source, binding_key, bone, graphics) {
        Ok(pose) => SlaveCameraSample::Pose(pose),
        Err(reason) => {
            log::trace!("Receiving native camera pose unavailable: {reason:?}");
            SlaveCameraSample::Unavailable("unavailable installed receiving Draw pose")
        }
    }
}
impl CnCGameEngine {
    /// A real source setCameraTransform, including every immediate slave query.
    /// Pure inspection/redraw helpers deliberately do not call this operation.
    pub(super) fn set_native_camera_transform(&mut self) -> Result<(), String> {
        self.with_native_camera_effects(|world, effects| {
            world.commit_native_camera_with_effects(effects)
        })
    }
    pub(super) fn with_native_camera_effects(
        &mut self,
        body: impl FnOnce(
            &mut crate::game_logic::GameLogic,
            &mut crate::game_logic::ScriptPresentationEffects<'_>,
        ) -> Result<(), String>,
    ) -> Result<(), String> {
        self.with_native_camera_input_effects(|world, effects, _input| body(world, effects))
    }
    pub(super) fn with_native_camera_input_effects<R>(
        &mut self,
        body: impl FnOnce(
            &mut crate::game_logic::GameLogic,
            &mut crate::game_logic::ScriptPresentationEffects<'_>,
            super::mouse::CameraInputLoan<'_>,
        ) -> R,
    ) -> R {
        let mut audio =
            super::native_audio_effects::NativeAudioService::receiving(&mut self.subsystem_manager);
        #[cfg(feature = "game_client")]
        {
            let policy = receiving_native_drawable_policy_for(&self.game_logic);
            let mut retirement = game_client::core::game_client::NativeParticleRetirementService;
            let (frame, _, drawables) = self.game_client.split_frame_snow_and_drawable_owner();
            let input = super::mouse::CameraInputLoan::new(
                &mut self.look_at_modes,
                &mut self.is_mmb_rotating,
                &mut self.mmb_anchor,
                frame,
            );
            let (poses, terrain) = self
                .render_pipeline
                .borrow_native_installed_poses_and_terrain();
            let mut view = NativeRendererViewEffects::new(
                poses,
                &mut self.graphics_system,
                drawables,
                self.host_direct_visual_world_epoch,
                self.current_player_id,
                policy,
                &mut retirement,
            );
            body(
                &mut self.game_logic,
                &mut crate::game_logic::ScriptPresentationEffects {
                    pristine_bones: Some(self.application_assets.as_ref()),
                    script_events: None,
                    partition: None,
                    audio: audio.as_mut().map(|service| service.receiver()),
                    terrain: terrain
                        .map(|terrain| terrain as &mut dyn crate::game_logic::NativeTerrainEffects),
                    ui: None,
                    view: Some(&mut view),
                },
                input,
            )
        }
        #[cfg(not(feature = "game_client"))]
        body(
            &mut self.game_logic,
            &mut crate::game_logic::ScriptPresentationEffects {
                pristine_bones: Some(self.application_assets.as_ref()),
                audio: audio.as_mut().map(|service| service.receiver()),
                ..Default::default()
            },
            super::mouse::CameraInputLoan::new(
                &mut self.look_at_modes,
                &mut self.is_mmb_rotating,
                &mut self.mmb_anchor,
                self.frame_counter,
            ),
        )
    }
}

#[cfg(all(test, feature = "game_client"))]
#[path = "native_view_effects_tests.rs"]
mod tests;

/// Snapshot immutable application constructor policy before lending field owners.
#[cfg(feature = "game_client")]
pub(super) fn receiving_native_drawable_policy()
-> game_client::core::game_client::ReceivingDrawablePolicy {
    use game_engine::common::thing::module::StaticGameLodLevel;
    let current = game_engine::common::game_lod::get_static_lod();
    let data = game_engine::common::global_data::read();
    game_client::core::game_client::ReceivingDrawablePolicy {
        time_of_day: native_time_of_day(data.time_of_day),
        use_draw_module_lod: data.use_draw_module_lod,
        use_tree_sway: data.use_tree_sway,
        static_game_lod: match current.to_ascii_uppercase().as_str() {
            "LOW" => StaticGameLodLevel::Low,
            "HIGH" => StaticGameLodLevel::High,
            _ => StaticGameLodLevel::Medium,
        },
    }
}

#[cfg(feature = "game_client")]
pub(super) fn receiving_native_drawable_policy_for(
    world: &crate::game_logic::GameLogic,
) -> game_client::core::game_client::ReceivingDrawablePolicy {
    use game_engine::common::thing::module::StaticGameLodLevel;
    let current = game_engine::common::game_lod::get_static_lod();
    let rules = world.lighting_rules();
    game_client::core::game_client::ReceivingDrawablePolicy {
        time_of_day: world.native_lighting_time_of_day(),
        use_draw_module_lod: rules.use_draw_module_lod,
        use_tree_sway: rules.use_tree_sway,
        static_game_lod: match current.to_ascii_uppercase().as_str() {
            "LOW" => StaticGameLodLevel::Low,
            "HIGH" => StaticGameLodLevel::High,
            _ => StaticGameLodLevel::Medium,
        },
    }
}

#[cfg(feature = "game_client")]
pub(super) fn native_time_of_day(
    value: game_engine::common::global_data::TimeOfDay,
) -> Option<game_client::system::TimeOfDay> {
    use game_client::system::TimeOfDay as Native;
    use game_engine::common::global_data::TimeOfDay as Source;
    Some(match value {
        Source::Morning => Native::Morning,
        Source::Afternoon => Native::Afternoon,
        Source::Evening => Native::Evening,
        Source::Night => Native::Night,
        Source::Invalid => return None,
    })
}

#[cfg(feature = "game_client")]
impl crate::game_logic::NativeProjectedRadiusAllocator
    for game_client::core::game_client::NativeDrawableOwner<'_>
{
    fn allocate_radius(
        &mut self,
        texture: &str,
        radius: f32,
        position: [f32; 3],
        color: u32,
        opacity: f32,
    ) -> Option<game_client::radius_decal::ShadowHandle> {
        self.allocate_projected_radius(texture, radius, position, color, opacity)
    }
}

#[cfg(feature = "game_client")]
fn admit_auxiliary_mutation(
    poses: &mut NativeInstalledPoseLoan<'_>,
    graphics: &mut GraphicsSystem,
    source: &crate::game_logic::NativeAuxiliarySource,
    mutation: game_client::core::game_client::NativeAuxiliaryMutation<'_>,
) -> game_client::core::GameClientResult<()> {
    use game_client::core::game_client::NativeAuxiliaryMutation as Mutation;
    let receipt = match &mutation {
        Mutation::Creating { receipt, .. }
        | Mutation::TransformInstalled { receipt, .. }
        | Mutation::ConditionsMarked { receipt, .. }
        | Mutation::Destroyed { receipt } => *receipt,
    };
    let identity = crate::assets::LiveDrawPlaybackIdentity::new_auxiliary(
        std::sync::Arc::clone(&source.world),
        source.epoch,
        receipt,
    )
    .map_err(game_client::core::GameClientError::SubsystemError)?;
    if matches!(mutation, Mutation::Destroyed { .. }) {
        return poses
            .retire_native_auxiliary(receipt, &identity)
            .map_err(game_client::core::GameClientError::SubsystemError);
    }
    poses
        .receive_native_auxiliary_mutation(
            mutation,
            &identity,
            &source.draws,
            source.template.name(),
            source.scale,
            graphics,
        )
        .map_err(game_client::core::GameClientError::SubsystemError)
}
