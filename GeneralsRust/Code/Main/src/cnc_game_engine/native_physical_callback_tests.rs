//! Real WGPU/Client retirement with bounded service-failure injection.
use super::*;
use game_client::core::game_client::{NativeDrawableOwner, NativeDrawableRetirement};

struct RetirementService {
    fail: bool,
    calls: usize,
}
impl NativeDrawableRetirement for RetirementService {
    fn retire(
        &mut self,
        owner: &mut NativeDrawableOwner<'_>,
        selected: Option<game_client::core::game_client::DrawableId>,
    ) -> game_client::core::game_client::GameClientResult<()> {
        self.calls += 1;
        if self.fail {
            return Err(game_client::core::GameClientError::SubsystemError(
                "authored retirement service failure".into(),
            ));
        }
        game_client::core::game_client::NativeParticleRetirementService.retire(owner, selected)
    }
}
struct DestructionProbe<'a, 'b> {
    runtime: &'a mut NativeRendererViewEffects<'b>,
    expected_id: ObjectId,
    expected_team: u32,
    calls: usize,
}
impl NativeViewEffects for DestructionProbe<'_, '_> {
    fn projected_radius_allocator(
        &mut self,
    ) -> Option<&mut dyn crate::game_logic::NativeProjectedRadiusAllocator> {
        self.runtime.projected_radius_allocator()
    }

    #[cfg(feature = "game_client")]
    fn body_visual_changed(
        &mut self,
        change: &crate::game_logic::NativeBodyVisualChange,
        audio: Option<&mut dyn game_client::drawable::drawable::NativeAmbientAudio>,
    ) -> Result<bool, String> {
        self.runtime.body_visual_changed(change, audio)
    }

    fn local_player_id(&self) -> u32 {
        self.runtime.local_player_id()
    }
    fn host_epoch(&self) -> u64 {
        self.runtime.host_epoch()
    }
    fn sample_slave(&mut self, source: &NativeSlaveSource, bone: &str) -> SlaveCameraSample {
        self.runtime.sample_slave(source, bone)
    }
    fn admit_factory_drawable(
        &mut self,
        source: crate::game_logic::NativeFactoryDrawableSource,
        birth: game_client::core::game_client::NativeDrawableBirthContext,
        content: &mut game_engine::common::thing::module_factory::ModuleFactory,
    ) -> Result<(), String> {
        self.runtime.admit_factory_drawable(source, birth, content)
    }
    fn retire_physical_object_drawable(
        &mut self,
        source: &crate::game_logic::NativeObjectRetirement<'_>,
    ) -> Result<(), String> {
        self.calls += 1;
        assert_eq!(source.object.id, self.expected_id);
        assert_eq!(source.identity.object_id(), self.expected_id.0);
        assert!(!source.live_objects.contains_key(&self.expected_id));
        assert!(
            !source
                .live_objects
                .ordered_ids()
                .contains(&self.expected_id)
        );
        assert_eq!(source.object.team_instance_id, Some(self.expected_team));
        assert_eq!(source.object.team_instance_name, "RetiringSibling");
        assert_eq!(source.object.health.current, 40.);
        assert!(source.object.status.destroyed);
        self.runtime.retire_physical_object_drawable(source)
    }
}

#[test]
#[ignore = "requires native WGPU; explicit isolated physical callback acceptance"]
fn actual_native_destructor_observes_unregistered_object_and_terminal_service_failure() {
    ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 32,
        height: 32,
        ..Default::default()
    })
    .unwrap();
    struct Scope;
    impl Drop for Scope {
        fn drop(&mut self) {
            let _ = ww3d_engine::shutdown();
        }
    }
    let _scope = Scope;
    let mut graphics = GraphicsSystem::new(
        ww3d_engine::device().unwrap(),
        ww3d_engine::queue().unwrap(),
        ww3d_engine::color_format().unwrap(),
        ww3d_engine::depth_format().unwrap(),
    )
    .unwrap();
    graphics.cache_model(
        crate::assets::mesh_asset_resolve::canonical_model_key("CameraModel"),
        model("CameraModel", 0.),
    );
    for fail in [false, true] {
        let mut pipeline = RenderPipeline::initialize(&graphics).unwrap();
        let mut peer_pipeline = RenderPipeline::initialize(&graphics).unwrap();
        let mut client = GameClient::new().unwrap();
        let mut peer_client = GameClient::new().unwrap();
        let mut world = physical_retirement_world();
        let mut peer = physical_retirement_world();
        let id = birth_physical_retirement_actor(
            &mut world,
            "StrictFactoryOwner",
            Vec3::new(3., 5., 7.),
            &mut client,
            &mut pipeline,
            &mut graphics,
            EPOCH,
        );
        let peer_id = birth_physical_retirement_actor(
            &mut peer,
            "StrictFactoryOwner",
            Vec3::new(303., 5., 7.),
            &mut peer_client,
            &mut peer_pipeline,
            &mut graphics,
            EPOCH + 1,
        );
        assert_eq!(id, peer_id);
        // Deliberately same-name siblings both mention the numeric ObjectID;
        // only its exact resident team may be unlinked by the destructor.
        world.add_player(crate::game_logic::Player::new(
            1,
            Team::USA,
            "RetirementOwner",
            true,
        ));
        let (resident, sibling) = {
            let mut factory = world.team_factory.lock().unwrap();
            factory
                .init_team_owned(
                    "RetiringSibling".into(),
                    "RetirementOwner".into(),
                    false,
                    None,
                    Some(1),
                )
                .unwrap();
            (
                factory
                    .create_inactive_team_with_owned_effects("RetiringSibling")
                    .unwrap()
                    .0,
                factory
                    .create_inactive_team_with_owned_effects("RetiringSibling")
                    .unwrap()
                    .0,
            )
        };
        resident.write().unwrap().add_member(id.0);
        sibling.write().unwrap().add_member(id.0);
        let team = resident.read().unwrap().get_id();
        world.objects.get_mut(&id).unwrap().team_instance_name = "RetiringSibling".into();
        world.objects.get_mut(&id).unwrap().team_instance_id = Some(team);
        let old = world.current_native_slave_source(id, 0).unwrap();
        let peer_source = peer.current_native_slave_source(peer_id, 0).unwrap();
        with_physical_retirement_view(
            &mut client,
            &mut pipeline,
            &mut graphics,
            EPOCH,
            |runtime| {
                world.destroy_object_with_effects(
                    id,
                    &mut crate::game_logic::ScriptPresentationEffects {
                        view: Some(runtime),
                        ..Default::default()
                    },
                );
            },
        );
        assert!(world.objects.contains_key(&id));
        assert!(world.objects.ordered_ids().contains(&id));
        assert!(resident.read().unwrap().has_member(id.0));
        assert!(sibling.read().unwrap().has_member(id.0));
        let mut service = RetirementService { fail, calls: 0 };
        {
            let (_, _, owner) = client.split_frame_snow_and_drawable_owner();
            let mut runtime = NativeRendererViewEffects::new(
                pipeline.borrow_native_installed_poses(),
                &mut graphics,
                owner,
                EPOCH,
                0,
                receiving_native_drawable_policy(),
                &mut service,
            );
            let key = runtime.drawables.binding_key(EPOCH, id.0).unwrap();
            let mut probe = DestructionProbe {
                runtime: &mut runtime,
                expected_id: id,
                expected_team: team,
                calls: 0,
            };
            let result = world.process_destroy_list_with_effects_checked(
                &mut crate::game_logic::ScriptPresentationEffects {
                    view: Some(&mut probe),
                    ..Default::default()
                },
            );
            assert_eq!(probe.calls, 1);
            assert_eq!(result.is_err(), fail);
            assert!(!world.objects.contains_key(&id));
            assert!(!world.objects.ordered_ids().contains(&id));
            assert_eq!(resident.read().unwrap().has_member(id.0), fail);
            assert!(sibling.read().unwrap().has_member(id.0));
            if fail {
                assert_eq!(probe.runtime.drawables.binding_key(EPOCH, id.0), Some(key));
                assert_eq!(
                    pose(probe.runtime.sample_slave(&old, "CAMERA"))
                        .w_axis
                        .truncate(),
                    Vec3::new(3., 5., 7.)
                );
                let diagnostic = world.objects.failed_retirement_object().unwrap();
                assert_eq!(diagnostic.id, id);
                assert_eq!(diagnostic.team_instance_id, Some(team));
                assert!(
                    world
                        .objects
                        .failed_retirement_reason()
                        .unwrap()
                        .contains("authored retirement service failure")
                );
                assert!(
                    world
                        .process_destroy_list_with_effects_checked(
                            &mut crate::game_logic::ScriptPresentationEffects {
                                view: Some(&mut probe),
                                ..Default::default()
                            }
                        )
                        .is_err()
                );
                assert_eq!(
                    probe.calls, 1,
                    "terminal failure cannot replay the destructor"
                );
            } else {
                assert!(probe.runtime.drawables.binding_key(EPOCH, id.0).is_none());
                assert!(matches!(
                    probe.runtime.sample_slave(&old, "CAMERA"),
                    SlaveCameraSample::MissingDrawable
                ));
                assert!(world.objects.failed_retirement_object().is_none());
            }
        }
        assert_eq!(service.calls, 1);
        if fail {
            let frame = world.frame;
            assert!(
                crate::save_load::SnapshotBuilder::new()
                    .create_world_snapshot(&world)
                    .is_err()
            );
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    world.update_with_dt(1.);
                }))
                .is_err()
            );
            assert_eq!(world.frame, frame);
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| world.reset())).is_err()
            );
            assert_eq!(world.frame, frame);
            assert_eq!(world.objects.failed_retirement_object().unwrap().id, id);
            // Recovery is a separate native service discard, never world.reset
            // or another destructor retry. Release real CPU/Draw before drop.
            with_physical_retirement_view(
                &mut client,
                &mut pipeline,
                &mut graphics,
                EPOCH,
                |runtime| {
                    assert!(
                        world
                            .create_object_with_script_effects_checked(
                                "StrictFactoryOwner",
                                Team::USA,
                                Vec3::ZERO,
                                None,
                                &mut crate::game_logic::ScriptPresentationEffects {
                                    view: Some(&mut *runtime),
                                    ..Default::default()
                                }
                            )
                            .is_err(),
                        "terminal session cannot admit another factory operation"
                    );
                    assert_eq!(world.objects.failed_retirement_object().unwrap().id, id);
                    runtime.discard_map_attempt().unwrap();
                    assert!(runtime.drawables.binding_key(EPOCH, id.0).is_none());
                },
            );
        }
        with_physical_retirement_view(
            &mut peer_client,
            &mut peer_pipeline,
            &mut graphics,
            EPOCH + 1,
            |runtime| {
                assert_eq!(
                    pose(runtime.sample_slave(&peer_source, "CAMERA"))
                        .w_axis
                        .truncate(),
                    Vec3::new(303., 5., 7.)
                );
                peer.destroy_object_with_effects(
                    peer_id,
                    &mut crate::game_logic::ScriptPresentationEffects {
                        view: Some(&mut *runtime),
                        ..Default::default()
                    },
                );
                peer.process_destroy_list_with_effects_checked(
                    &mut crate::game_logic::ScriptPresentationEffects {
                        view: Some(&mut *runtime),
                        ..Default::default()
                    },
                )
                .unwrap();
            },
        );
        drop(world);
        let mut fresh = physical_retirement_world();
        let fresh_id = birth_physical_retirement_actor(
            &mut fresh,
            "StrictFactoryOwner",
            Vec3::new(83., 5., 7.),
            &mut client,
            &mut pipeline,
            &mut graphics,
            EPOCH + 2,
        );
        assert_eq!(fresh_id, id);
        with_physical_retirement_view(
            &mut client,
            &mut pipeline,
            &mut graphics,
            EPOCH + 2,
            |runtime| {
                fresh.destroy_object_with_effects(
                    fresh_id,
                    &mut crate::game_logic::ScriptPresentationEffects {
                        view: Some(&mut *runtime),
                        ..Default::default()
                    },
                );
                fresh
                    .process_destroy_list_with_effects_checked(
                        &mut crate::game_logic::ScriptPresentationEffects {
                            view: Some(&mut *runtime),
                            ..Default::default()
                        },
                    )
                    .unwrap();
            },
        );
    }
}

struct PanicAfterActualFactoryBirth<'a, 'b> {
    runtime: &'a mut NativeRendererViewEffects<'b>,
    born: Option<ObjectId>,
    calls: usize,
}
impl NativeViewEffects for PanicAfterActualFactoryBirth<'_, '_> {
    fn projected_radius_allocator(
        &mut self,
    ) -> Option<&mut dyn crate::game_logic::NativeProjectedRadiusAllocator> {
        self.runtime.projected_radius_allocator()
    }

    #[cfg(feature = "game_client")]
    fn body_visual_changed(
        &mut self,
        change: &crate::game_logic::NativeBodyVisualChange,
        audio: Option<&mut dyn game_client::drawable::drawable::NativeAmbientAudio>,
    ) -> Result<bool, String> {
        self.runtime.body_visual_changed(change, audio)
    }

    fn local_player_id(&self) -> u32 {
        self.runtime.local_player_id()
    }
    fn host_epoch(&self) -> u64 {
        self.runtime.host_epoch()
    }
    fn sample_slave(&mut self, source: &NativeSlaveSource, bone: &str) -> SlaveCameraSample {
        self.runtime.sample_slave(source, bone)
    }
    fn admit_factory_drawable(
        &mut self,
        source: crate::game_logic::NativeFactoryDrawableSource,
        birth: game_client::core::game_client::NativeDrawableBirthContext,
        content: &mut game_engine::common::thing::module_factory::ModuleFactory,
    ) -> Result<(), String> {
        let id = ObjectId(source.drawable.object_id);
        self.runtime
            .admit_factory_drawable(source, birth, content)?;
        self.born = Some(id);
        self.calls += 1;
        panic!("actual receiving factory service panic after native birth")
    }
    fn retire_physical_object_drawable(
        &mut self,
        source: &crate::game_logic::NativeObjectRetirement<'_>,
    ) -> Result<(), String> {
        self.runtime.retire_physical_object_drawable(source)
    }
}
#[test]
#[ignore = "requires native WGPU; actual receiving factory panic continuation"]
fn actual_team_creation_panic_retains_exact_team_partial_native_life_and_peer() {
    use game_engine::common::{dict::Dict, well_known_keys::*};
    use gamelogic::scripting::core::{
        Parameter, ParameterType, Script, ScriptAction, ScriptActionType, ScriptList,
    };
    ww3d_engine::init_headless_blocking(ww3d_engine::EngineConfig {
        width: 32,
        height: 32,
        ..Default::default()
    })
    .unwrap();
    struct Scope;
    impl Drop for Scope {
        fn drop(&mut self) {
            let _ = ww3d_engine::shutdown();
        }
    }
    let _scope = Scope;
    let mut graphics = GraphicsSystem::new(
        ww3d_engine::device().unwrap(),
        ww3d_engine::queue().unwrap(),
        ww3d_engine::color_format().unwrap(),
        ww3d_engine::depth_format().unwrap(),
    )
    .unwrap();
    graphics.cache_model(
        crate::assets::mesh_asset_resolve::canonical_model_key("CameraModel"),
        model("CameraModel", 0.),
    );
    let mut pipeline = RenderPipeline::initialize(&graphics).unwrap();
    let mut client = GameClient::new().unwrap();
    let mut peer_pipeline = RenderPipeline::initialize(&graphics).unwrap();
    let mut peer_client = GameClient::new().unwrap();
    let mut world = physical_retirement_world();
    let mut peer = physical_retirement_world();
    let peer_id = birth_physical_retirement_actor(
        &mut peer,
        "StrictFactoryOwner",
        Vec3::new(303., 0., 0.),
        &mut peer_client,
        &mut peer_pipeline,
        &mut graphics,
        EPOCH + 1,
    );
    let mut player = crate::game_logic::Player::new(1, Team::USA, "PanicOwner", true);
    player.map_side.map_player_name = "PanicOwner".into();
    world.add_player(player);
    let mut dict = Dict::new();
    dict.set_bool(key_team_executes_actions_on_create(), true);
    dict.set_ascii_string(key_team_production_condition(), "NativePanicCreate");
    world
        .team_factory
        .lock()
        .unwrap()
        .init_team_owned(
            "NativePanicTeam".into(),
            "PanicOwner".into(),
            false,
            Some(&dict),
            Some(1),
        )
        .unwrap();
    let mut action = ScriptAction::new(ScriptActionType::CreateObject);
    for parameter in [
        Parameter::with_string(ParameterType::ObjectType, "StrictFactoryOwner".into()),
        Parameter::with_string(ParameterType::Team, "NativePanicTeam".into()),
        Parameter::with_coord(
            ParameterType::Coord3D,
            gamelogic::common::Coord3D::new(3., 5., 7.),
        ),
        Parameter::with_real(ParameterType::Angle, 0.),
    ] {
        action.add_parameter(parameter).unwrap();
    }
    let mut script = Script::new();
    script.script_name = "NativePanicCreate".into();
    script.is_subroutine = true;
    script.action = Some(Box::new(action));
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let mut interpreter = gamelogic::scripting::engine::ScriptEngine::new().unwrap();
    interpreter
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    world.install_owned_script_engine(interpreter);
    let make_incoming = || {
        let mut actor = Object::new(
            ThingTemplate::new("ExternalPanicPrimary"),
            ObjectId(990),
            Team::USA,
        );
        actor.owner_player_id = Some(1);
        actor.team_instance_name = "NativePanicTeam".into();
        actor
    };
    let born = with_physical_retirement_view(
        &mut client,
        &mut pipeline,
        &mut graphics,
        EPOCH,
        |runtime| {
            let mut service = PanicAfterActualFactoryBirth {
                runtime,
                born: None,
                calls: 0,
            };
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                world.try_add_object_with_effects(
                    make_incoming(),
                    &mut crate::game_logic::ScriptPresentationEffects {
                        view: Some(&mut service),
                        ..Default::default()
                    },
                )
            }));
            assert!(result.is_err());
            assert_eq!(service.calls, 1);
            service.born.unwrap()
        },
    );
    assert_eq!(
        born, peer_id,
        "same numeric identity is independent of receiving Client"
    );
    let exact = world
        .team_factory
        .lock()
        .unwrap()
        .get_all_teams()
        .into_iter()
        .find(|team| team.read().unwrap().get_name().as_str() == "NativePanicTeam")
        .unwrap();
    assert!(
        exact
            .read()
            .unwrap()
            .creation_failure()
            .unwrap()
            .contains("panicked")
    );
    assert!(!exact.read().unwrap().creation_is_running());
    assert!(world.host_object(born).is_some());
    assert!(world.host_object(ObjectId(990)).is_none());
    assert!(world.owned_script_engine.is_some());
    with_physical_retirement_view(
        &mut client,
        &mut pipeline,
        &mut graphics,
        EPOCH,
        |runtime| {
            assert!(runtime.drawables.binding_key(EPOCH, born.0).is_some());
        },
    );
    assert!(
        crate::save_load::SnapshotBuilder::new()
            .create_world_snapshot_with_client(&world, &client)
            .is_err()
    );
    let error = world.try_add_object(make_incoming()).unwrap_err();
    assert!(error.to_string().contains("panicked"));
    let same = world
        .team_factory
        .lock()
        .unwrap()
        .find_team_by_id(exact.read().unwrap().get_id())
        .unwrap();
    assert!(std::sync::Arc::ptr_eq(&same, &exact));
    assert_eq!(
        peer.host_object(peer_id).unwrap().get_position(),
        Vec3::new(303., 0., 0.)
    );
    with_physical_retirement_view(
        &mut peer_client,
        &mut peer_pipeline,
        &mut graphics,
        EPOCH + 1,
        |runtime| {
            assert!(
                runtime
                    .drawables
                    .binding_key(EPOCH + 1, peer_id.0)
                    .is_some()
            );
        },
    );
    assert!(peer.team_creation_error().is_none());
}
