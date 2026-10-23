//! Original physical deletion stays in its checked outer frame phase.
use super::*;

#[test]
#[ignore = "requires native WGPU; explicit isolated recrew/observer phase acceptance"]
fn actual_recrew_and_host_observer_retain_queued_native_lifetimes_until_checked_endphase() {
    deletion_phase_scene(true);
}

#[test]
#[ignore = "requires native WGPU; explicit isolated observer-only phase acceptance"]
fn actual_observer_preserves_pending_native_deletion_without_a_physical_owner() {
    deletion_phase_scene(false);
}

fn deletion_phase_scene(recrew: bool) {
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
    let mut world = GameLogic::new();
    let text = [("PhaseInfantry", "INFANTRY"), ("PhaseVehicle", "VEHICLE")]
        .into_iter()
        .map(|(name, kind)| {
            format!("Object {name}\nKindOf = {kind}\nBody = ActiveBody BodyTag\nMaxHealth = 40\nInitialHealth = 40\nEnd\nBehavior = AIUpdateInterface PhaseAI\nEnd\nDraw = W3DModelDraw DrawTag\nDefaultConditionState\nModel = CameraModel\nEnd\nEnd\nEnd\n")
        })
        .collect::<String>();
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "recrew-phase.ini".to_owned(),
                text,
            )])
            .unwrap(),
        )
        .unwrap();
    world.require_native_drawable_owner();
    let vehicle = birth_physical_retirement_actor(
        &mut world,
        "PhaseVehicle",
        Vec3::ZERO,
        &mut client,
        &mut pipeline,
        &mut graphics,
        EPOCH,
    );
    let infantry = birth_physical_retirement_actor(
        &mut world,
        "PhaseInfantry",
        Vec3::new(1., 0., 0.),
        &mut client,
        &mut pipeline,
        &mut graphics,
        EPOCH,
    );
    let unrelated = birth_physical_retirement_actor(
        &mut world,
        "PhaseVehicle",
        Vec3::new(30., 0., 0.),
        &mut client,
        &mut pipeline,
        &mut graphics,
        EPOCH,
    );
    {
        let v = world.objects.get_mut(&vehicle).unwrap();
        v.apply_kill_pilot_unmanned(
            &mut world.health_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        v.name.clear();
        let i = world.objects.get_mut(&infantry).unwrap();
        i.name = "RecrewNamedInfantry".into();
        i.movement.max_speed = 30.;
        // Exercise the actual collision's ignored-obstacle recrew admission.
        i.ignored_obstacle_id = Some(vehicle);
    }
    assert!(world.can_execute_infantry_unmanned_recrew(infantry, vehicle));
    with_physical_retirement_view(
        &mut client,
        &mut pipeline,
        &mut graphics,
        EPOCH,
        |runtime| {
            let infantry_key = runtime.drawables.binding_key(EPOCH, infantry.0).unwrap();
            let unrelated_key = runtime.drawables.binding_key(EPOCH, unrelated.0).unwrap();
            let vehicle_key = runtime.drawables.binding_key(EPOCH, vehicle.0).unwrap();
            world.destroy_object_with_effects(
                unrelated,
                &mut crate::game_logic::ScriptPresentationEffects {
                    view: Some(&mut *runtime),
                    ..Default::default()
                },
            );
            assert!(world.objects[&unrelated].status.destroyed);
            if recrew {
                assert!(world.try_physics_collide_with_effects(
                    infantry,
                    vehicle,
                    1.,
                    &mut crate::game_logic::ScriptPresentationEffects {
                        view: Some(&mut *runtime),
                        ..Default::default()
                    }
                ));
                assert_eq!(world.unmanned_reclaim_count_for_test(), 1);
                assert!(!world.objects[&vehicle].status.disabled_unmanned);
                assert!(world.objects[&vehicle].is_private_captured());
                assert_eq!(world.objects[&vehicle].name, "RecrewNamedInfantry");
            } else {
                world.destroy_object_with_effects(
                    infantry,
                    &mut crate::game_logic::ScriptPresentationEffects {
                        view: Some(&mut *runtime),
                        ..Default::default()
                    },
                );
                assert_eq!(world.unmanned_reclaim_count_for_test(), 0);
            }
            assert!(
                world.objects.contains_key(&infantry),
                "recrew must retain queued infantry until physical endphase"
            );
            assert!(
                world.objects[&infantry].status.destroyed,
                "destroy/onDelete admission is immediate"
            );
            assert!(world.has_pending_object_removals());
            for id in [infantry, unrelated] {
                assert!(
                    world.objects.contains_key(&id),
                    "collision may not physically retire either queued object"
                );
                assert!(world.objects.ordered_ids().contains(&id));
            }
            assert_eq!(
                runtime.drawables.binding_key(EPOCH, infantry.0),
                Some(infantry_key)
            );
            assert_eq!(
                runtime.drawables.binding_key(EPOCH, unrelated.0),
                Some(unrelated_key)
            );
            // This is the actual native after-step observer helper. It receives no
            // native owner and must never perform a hidden physical flush.
            let mut shadow = crate::gameworld_shadow::GameWorldShadow::new(16);
            crate::gameworld_shadow::run_owned_host_boundary(Some(&mut shadow), &mut world);
            crate::gameworld_shadow::run_owned_host_boundary(None, &mut world);
            for id in [infantry, unrelated] {
                assert!(world.objects.contains_key(&id));
                assert!(world.objects.ordered_ids().contains(&id));
                assert!(shadow.entity_for_host(id).is_some());
            }
            assert_eq!(
                runtime.drawables.binding_key(EPOCH, infantry.0),
                Some(infantry_key)
            );
            assert_eq!(
                runtime.drawables.binding_key(EPOCH, unrelated.0),
                Some(unrelated_key)
            );
            world
                .process_destroy_list_with_effects_checked(
                    &mut crate::game_logic::ScriptPresentationEffects {
                        view: Some(&mut *runtime),
                        ..Default::default()
                    },
                )
                .unwrap();
            crate::gameworld_shadow::run_owned_host_boundary(Some(&mut shadow), &mut world);
            for id in [infantry, unrelated] {
                assert!(shadow.entity_for_host(id).is_none());
                assert!(!world.objects.contains_key(&id));
                assert!(!world.objects.ordered_ids().contains(&id));
                assert!(runtime.drawables.binding_key(EPOCH, id.0).is_none());
            }
            assert!(!world.has_pending_object_removals());
            assert!(world.objects.contains_key(&vehicle));
            assert_eq!(
                runtime.drawables.binding_key(EPOCH, vehicle.0),
                Some(vehicle_key)
            );
            world.destroy_object_with_effects(
                vehicle,
                &mut crate::game_logic::ScriptPresentationEffects {
                    view: Some(&mut *runtime),
                    ..Default::default()
                },
            );
            world
                .process_destroy_list_with_effects_checked(
                    &mut crate::game_logic::ScriptPresentationEffects {
                        view: Some(&mut *runtime),
                        ..Default::default()
                    },
                )
                .unwrap();
            assert!(world.objects.registered_is_empty());
            assert!(runtime.drawables.binding_key(EPOCH, vehicle.0).is_none());
        },
    );
}

#[test]
#[ignore = "requires native WGPU; explicit isolated AI startup deletion phase acceptance"]
fn actual_ai_startup_build_callbacks_keep_map_cc_and_unrelated_drawables_until_checked_endphase() {
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
    fn make_world() -> GameLogic {
        let definitions = [
            ("AmericaCommandCenter", "STRUCTURE COMMANDCENTER"),
            ("AmericaSupplyCenter", "STRUCTURE FS_SUPPLY_CENTER"),
            ("AmericaVehicleChinook", "VEHICLE AIRCRAFT HARVESTER"),
            ("AIPhaseUnrelated", "VEHICLE"),
        ];
        let text = definitions.into_iter().map(|(name, kind)| {
            format!("Object {name}\nKindOf = {kind}\nBody = ActiveBody BodyTag\nMaxHealth = 40\nInitialHealth = 40\nEnd\nDraw = W3DModelDraw DrawTag\nDefaultConditionState\nModel = CameraModel\nEnd\nEnd\nEnd\n")
        }).collect::<String>();
        let mut world = GameLogic::new();
        world
            .admit_receiving_module_content(
                crate::assets::ReceivingObjectContent::from_sources([(
                    "ai-phase.ini".to_owned(),
                    text,
                )])
                .unwrap(),
            )
            .unwrap();
        let mut player = crate::game_logic::Player::new(1, Team::USA, "AIPhaseOwner", false);
        player.resources.supplies = 10_000;
        world.add_player(player);
        world.require_native_drawable_owner();
        world
    }
    let mut world = make_world();
    let mut peer = make_world();
    let old_cc = birth_physical_retirement_actor(
        &mut world,
        "AmericaCommandCenter",
        Vec3::new(20., 0., 30.),
        &mut client,
        &mut pipeline,
        &mut graphics,
        EPOCH,
    );
    let peer_cc = birth_physical_retirement_actor(
        &mut peer,
        "AmericaCommandCenter",
        Vec3::new(120., 0., 130.),
        &mut peer_client,
        &mut peer_pipeline,
        &mut graphics,
        EPOCH + 1,
    );
    assert_eq!(old_cc, peer_cc);
    let unrelated = birth_physical_retirement_actor(
        &mut world,
        "AIPhaseUnrelated",
        Vec3::new(60., 0., 0.),
        &mut client,
        &mut pipeline,
        &mut graphics,
        EPOCH,
    );
    world.add_ai_opponent(1, Team::USA, crate::ai::AIDifficulty::Medium);
    {
        let ai = world.ai_manager.ai_players.get_mut(&1).unwrap();
        ai.building_queue.clear();
        ai.next_team_queue_time = 999.;
    }
    let mut data = game_engine::common::ini::AIData::default();
    data.rotate_skirmish_bases = false;
    data.side_build_lists.clear();
    let mut list = game_engine::common::ini::AiSideBuildList::new("America".into());
    for (name, template, location) in [
        ("CC", "AmericaCommandCenter", (0., 0.)),
        ("Supply", "AmericaSupplyCenter", (50., 0.)),
    ] {
        list.entries.push(game_engine::common::ini::BuildListEntry {
            building_name: name.into(),
            template_name: template.into(),
            location,
            rebuilds: 0,
            angle_radians: 0.,
            initially_built: true,
            rally_point_offset: (0., 0.),
            automatically_build: true,
        });
    }
    data.side_build_lists.push(list);
    world.set_ai_definition_base(data);
    struct StartupProbe<'a, 'b> {
        runtime: &'a mut NativeRendererViewEffects<'b>,
        old: u32,
        unrelated: u32,
        births: Vec<String>,
        allow_retirement: bool,
    }
    impl NativeViewEffects for StartupProbe<'_, '_> {
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
        fn retire_physical_object_drawable(
            &mut self,
            source: &crate::game_logic::NativeObjectRetirement<'_>,
        ) -> Result<(), String> {
            assert!(
                self.allow_retirement,
                "AI startup must not flush any queued physical deletion"
            );
            self.runtime.retire_physical_object_drawable(source)
        }
        fn admit_factory_drawable(
            &mut self,
            source: crate::game_logic::NativeFactoryDrawableSource,
            birth: game_client::core::game_client::NativeDrawableBirthContext,
            content: &mut game_engine::common::thing::module_factory::ModuleFactory,
        ) -> Result<(), String> {
            for id in [self.old, self.unrelated] {
                assert!(
                    self.runtime.drawables.binding_key(EPOCH, id).is_some(),
                    "real building/collector constructor observes original queued Client lifetimes"
                );
            }
            let name = source.drawable.template_name.clone();
            self.runtime
                .admit_factory_drawable(source, birth, content)?;
            self.births.push(name);
            Ok(())
        }
    }
    with_physical_retirement_view(
        &mut client,
        &mut pipeline,
        &mut graphics,
        EPOCH,
        |runtime| {
            let old_key = runtime.drawables.binding_key(EPOCH, old_cc.0).unwrap();
            let unrelated_key = runtime.drawables.binding_key(EPOCH, unrelated.0).unwrap();
            let mut probe = StartupProbe {
                runtime,
                old: old_cc.0,
                unrelated: unrelated.0,
                births: Vec::new(),
                allow_retirement: false,
            };
            world.destroy_object_with_effects(
                unrelated,
                &mut crate::game_logic::ScriptPresentationEffects {
                    view: Some(&mut probe),
                    ..Default::default()
                },
            );
            world.initialize_owned_skirmish_ai_from_definitions_for_test(
                1,
                &mut crate::game_logic::ScriptPresentationEffects {
                    view: Some(&mut probe),
                    ..Default::default()
                },
            );
            for name in [
                "AmericaCommandCenter",
                "AmericaSupplyCenter",
                "AmericaVehicleChinook",
            ] {
                assert!(
                    probe.births.iter().any(|created| created == name),
                    "actual receiving factory callback missing: {name}"
                );
            }
            assert_eq!(
                world.ai_manager.ai_players[&1].next_team_queue_time, 0.,
                "actual collector callback reaches canonical AI owner"
            );
            for id in [old_cc, unrelated] {
                assert!(world.objects.contains_key(&id));
                assert!(world.objects.ordered_ids().contains(&id));
                assert!(world.objects[&id].status.destroyed);
                assert_eq!(world.objects[&id].health.current, 40.);
            }
            assert_eq!(
                probe.runtime.drawables.binding_key(EPOCH, old_cc.0),
                Some(old_key)
            );
            assert_eq!(
                probe.runtime.drawables.binding_key(EPOCH, unrelated.0),
                Some(unrelated_key)
            );
            assert!(peer.objects.contains_key(&peer_cc));
            assert!(!peer.objects[&peer_cc].status.destroyed);
            probe.allow_retirement = true;
            world
                .process_destroy_list_with_effects_checked(
                    &mut crate::game_logic::ScriptPresentationEffects {
                        view: Some(&mut probe),
                        ..Default::default()
                    },
                )
                .unwrap();
            for id in [old_cc, unrelated] {
                assert!(!world.objects.contains_key(&id));
                assert!(probe.runtime.drawables.binding_key(EPOCH, id.0).is_none());
            }
            assert!(!world.has_pending_object_removals());
            let survivors = world.objects.ordered_ids().to_vec();
            for id in survivors {
                world.destroy_object_with_effects(
                    id,
                    &mut crate::game_logic::ScriptPresentationEffects {
                        view: Some(&mut probe),
                        ..Default::default()
                    },
                );
            }
            world
                .process_destroy_list_with_effects_checked(
                    &mut crate::game_logic::ScriptPresentationEffects {
                        view: Some(&mut probe),
                        ..Default::default()
                    },
                )
                .unwrap();
        },
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
                    .binding_key(EPOCH + 1, peer_cc.0)
                    .is_some()
            );
            peer.destroy_object_with_effects(
                peer_cc,
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
}
