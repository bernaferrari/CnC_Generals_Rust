//! One driving native render owner, reborrowed at the original phase boundaries.
use super::*;
use crate::graphics::render_pipeline::{
    FilterCameraMatrices, FrozenDirectDrawableSceneCandidate, FrozenDirectDrawableSceneDecision,
    NativeFrameRenderReceiver, native_pose_loan::NativeInstalledPoseLoan,
};
pub(super) struct NativeFrameReceiver<'a> {
    pub audio: Option<super::native_audio_effects::NativeAudioService<'a>>,
    pub world: &'a mut crate::game_logic::GameLogic,
    pub drawables: game_client::core::game_client::NativeDrawableOwner<'a>,
    pub look_at_modes: &'a mut super::mouse::LookAtHostModes,
    pub world_bounds: (Vec3, Vec3),
    pub slave_source: Option<crate::game_logic::NativeSlaveSource>,
    pub slave_binding_key: Option<game_client::core::PresentationDirectDrawableBindingKey>,
    pub host_epoch: u64,
    pub local_player_id: u32,
    pub assets: &'a dyn crate::assets::pristine_bones::PristineBoneAssets,
    pub logic_frame: u32,
}
impl NativeFrameRenderReceiver for NativeFrameReceiver<'_> {
    fn actor_draw_candidate_is_current(
        &self,
        key: game_client::core::PresentationDirectDrawableBindingKey,
        input: &crate::presentation_frame::UnitRenderInput,
    ) -> std::result::Result<bool, String> {
        if self.drawables.binding_key(key.host_epoch, key.object_id) != Some(key) {
            return Ok(false);
        }
        let Some(object) = self.world.host_object(input.id) else {
            return Ok(false);
        };
        let identity = input
            .draw_playback_identity
            .as_ref()
            .ok_or("native actor candidate has no frozen birth")?;
        Ok(self.world.live_draw_playback_identity(object) == *identity)
    }

    fn native_draw_candidates(
        &mut self,
    ) -> std::result::Result<Vec<game_client::core::game_client::NativeDrawCandidate>, String> {
        self.drawables
            .native_draw_candidates()
            .map_err(|error| error.to_string())
    }
    fn observe_auxiliary_drawable(
        &mut self,
        receipt: &game_client::core::game_client::NativeAuxiliaryReceipt,
    ) -> std::result::Result<
        Option<game_client::core::game_client::NativeAuxiliaryDrawObservation>,
        String,
    > {
        self.drawables
            .observe_auxiliary_drawable(receipt)
            .map_err(|error| error.to_string())
    }

    fn complete_physics_visual(
        &mut self,
        input: &mut crate::presentation_frame::UnitRenderInput,
        frame: &crate::presentation_frame::PresentationFrame,
    ) -> std::result::Result<Option<Mat4>, String> {
        let instance = if input.drawable_shroud.lifetime.is_direct_host_object() {
            let identity = input
                .draw_playback_identity
                .as_ref()
                .ok_or_else(|| "native draw instance has no frozen source birth".to_owned())?;
            let object = self
                .world
                .host_object(input.id)
                .ok_or_else(|| "native draw instance source is absent".to_owned())?;
            if &self.world.live_draw_playback_identity(object) != identity {
                return Err(
                    "native draw instance source belongs to another Object birth/world".into(),
                );
            }
            let key = self
                .drawables
                .binding_key(self.host_epoch, input.id.0)
                .ok_or_else(|| {
                    "native draw instance has no receiving Drawable binding".to_owned()
                })?;
            Some(
                self.drawables
                    .instance_transform(key)
                    .map_err(|error| error.to_string())?
                    .into(),
            )
        } else {
            None
        };
        self.world
            .host_physics_visuals
            .borrow_mut()
            .complete_input(input, frame);
        Ok(instance)
    }
    fn direct_scene_decisions(
        &mut self,
        candidates: &[FrozenDirectDrawableSceneCandidate],
    ) -> Vec<FrozenDirectDrawableSceneDecision> {
        let game_client_candidates = candidates.iter().copied().map(|candidate| {
            game_client::core::FrozenDirectSceneShroudCandidate {
                binding_key: game_client::core::PresentationDirectDrawableBindingKey {
                    host_epoch: candidate.host_epoch,
                    object_id: candidate.object_id.0,
                    drawable_id: game_client::core::DrawableId(candidate.drawable_id),
                    binding_generation: candidate.binding_generation,
                },
                raw_status: candidate.raw_status,
                effectively_dead: candidate.effectively_dead,
            }
        });
        // Preserve the exact scene-local outcome before WGPU. The
        // pipeline verifies the full key again, drops a hidden direct
        // Drawable before forward rendering, and retains Clear versus
        // PartialClear/Fogged pass eligibility for hq-1a1's projected
        // material route. No scalar FOW result is used here.
        self.drawables
                    .evaluate_frozen_direct_scene_shroud_candidates(
                        self.logic_frame,
                        game_client_candidates,
                    )
                    .into_iter()
                    .filter_map(|decision| {
                        let outcome = match decision.decision {
                            game_client::drawable::SceneShroudDecision::HiddenDirectDrawable => {
                                crate::graphics::render_pipeline::FrozenDirectDrawableSceneOutcome::HiddenDirectDrawable
                            }
                            game_client::drawable::SceneShroudDecision::RenderDrawable {
                                final_status,
                                pushes_projected_shroud_pass,
                            } => crate::graphics::render_pipeline::FrozenDirectDrawableSceneOutcome::RenderDrawable {
                                final_status,
                                pushes_projected_shroud_pass,
                            },
                            game_client::drawable::SceneShroudDecision::RenderGhostWithFogLight
                            | game_client::drawable::SceneShroudDecision::RenderWithoutDrawable => {
                                return None;
                            }
                        };
                        Some(
                            crate::graphics::render_pipeline::FrozenDirectDrawableSceneDecision {
                                host_epoch: decision.binding_key.host_epoch,
                                object_id: crate::game_logic::ObjectId(
                                    decision.binding_key.object_id,
                                ),
                                drawable_id: decision.binding_key.drawable_id.0,
                                binding_generation: decision.binding_key.binding_generation,
                                outcome,
                            },
                        )
                    })
                    .collect()
    }
    fn view_filter(&self) -> &game_client::display::view_filter::ViewFilterState {
        &self.world.session_view.filter
    }
    fn view_filter_mut(&mut self) -> &mut game_client::display::view_filter::ViewFilterState {
        &mut self.world.session_view.filter
    }
    fn complete_filter_camera(
        &mut self,
        point: Option<Vec3>,
        poses: &mut NativeInstalledPoseLoan<'_>,
        graphics: &mut GraphicsSystem,
    ) -> std::result::Result<FilterCameraMatrices, String> {
        let terrain_owner = Arc::clone(self.world.world_services.terrain());
        let terrain = terrain_owner
            .read()
            .map_err(|_| "receiving filter LookAt terrain poisoned".to_owned())?;
        let (_, target, impulse, mut cancellation, _, native) =
            self.world.split_filter_camera_owner();
        let mut camera = super::camera_filter::NativeFilterCameraContext {
            look_at_modes: self.look_at_modes,
            target,
            impulse,
            native,
            world_bounds: self.world_bounds,
            terrain: &terrain,
        };
        let source = &self.slave_source;
        let binding = self.slave_binding_key;
        camera.finish(point, &mut || cancellation.cancel(), &mut |mode| {
            source.as_ref().map_or(
                crate::presentation_frame::camera::build::SlaveCameraSample::MissingObject,
                |source| {
                    super::native_view_effects::sample_installed_slave(
                        poses,
                        graphics,
                        source,
                        &mode.bone_name,
                        binding,
                    )
                },
            )
        })
    }
    fn complete_tree_cpu_draw(
        &mut self,
        terrain: &mut game_client::terrain::terrain_visual::TerrainVisualImpl,
        mut poses: NativeInstalledPoseLoan<'_>,
        graphics: &mut GraphicsSystem,
        motion: game_client::terrain::terrain_visual::TerrainVisualMotion,
    ) -> std::result::Result<Option<game_client::terrain::TreeCpuDrawReceipt>, String> {
        let Some(mut cursor) = terrain.begin_owned_tree_cpu_draw(
            motion,
            &mut ReceivingTreeRandom(&mut self.world.session_view.client_random),
        ) else {
            return Ok(None);
        };
        loop {
            // A real shroud owner is borrowed only until the next FX suspension.
            // No guard or buffer loan spans authored FX.
            let event = terrain
                .tree_buffer_mut()
                .try_advance_cpu_draw(&mut cursor, |tree| {
                    let shroud = self
                        .world
                        .world_services
                        .shroud()
                        .lock()
                        .map_err(|_| "receiving tree shroud poisoned".to_owned())?;
                    let position = gamelogic::common::Coord3D {
                        x: tree.location.x,
                        y: tree.location.y,
                        z: tree.location.z,
                    };
                    let player = self.world.local_player_id().unwrap_or(0);
                    let status = shroud
                        .prop_shroud_status(player, &position)
                        .ok_or_else(|| {
                            "native tree draw requires its admitted shroud grid".to_owned()
                        })?;
                    Ok::<_, String>(match status {
                        gamelogic::common::ObjectShroudStatus::Fogged => {
                            game_client::terrain::TreeShroudStatus::Fogged
                        }
                        gamelogic::common::ObjectShroudStatus::Shrouded => {
                            game_client::terrain::TreeShroudStatus::Shrouded
                        }
                        _ => game_client::terrain::TreeShroudStatus::Clear,
                    })
                })?;
            let Some(event) = event else {
                break;
            };
            let policy =
                super::native_view_effects::receiving_native_drawable_policy_for(self.world);
            let mut retirement = game_client::core::game_client::NativeParticleRetirementService;
            let mut view = super::native_view_effects::NativeRendererViewEffects::new(
                poses.reborrow(),
                graphics,
                self.drawables.reborrow(),
                self.host_epoch,
                self.local_player_id,
                policy,
                &mut retirement,
            );
            let audio = &mut self.audio;
            self.world
                .dispatch_fx_list_at_host_position_oriented_with_effects(
                    &event.fx_name,
                    Vec3::new(event.position.x, event.position.z, event.position.y),
                    None,
                    0.0,
                    0.0,
                    None,
                    &mut crate::game_logic::ScriptPresentationEffects {
                        pristine_bones: Some(self.assets),
                        terrain: Some(terrain),
                        view: Some(&mut view),
                        audio: audio.as_mut().map(|audio| audio.receiver()),
                        ..Default::default()
                    },
                );
        }
        terrain
            .tree_buffer_mut()
            .finish_cpu_draw(cursor)
            .map(Some)
            .map_err(str::to_owned)
    }
}

/// Borrowed adapter to the same receiving client stream used by authored FX.
struct ReceivingTreeRandom<'a>(&'a mut game_engine::common::random_value::RandomState);
impl game_client::terrain::TreeRandom for ReceivingTreeRandom<'_> {
    fn int_range(&mut self, min: i32, max: i32) -> i32 {
        let width = max.wrapping_sub(min).wrapping_add(1) as u32;
        if width == 0 {
            return max;
        }
        ((self.0.next_u32() % width) as i32).wrapping_add(min)
    }
    fn real_range(&mut self, min: f32, max: f32) -> f32 {
        self.0.next_real(min, max)
    }
}
