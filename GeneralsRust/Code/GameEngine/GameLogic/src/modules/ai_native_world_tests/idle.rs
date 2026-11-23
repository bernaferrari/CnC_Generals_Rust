//! Receiving Idle dependencies execute against actual owners and partition data.
use super::*;
use crate::modules::ai_world::AiWorld;

fn admit_actual_search_partition(game: &mut NativeMatch) {
    // NativeMatch starts with only the source collision owner. Register the
    // target with its authored position/geometry; vision maintenance alone
    // updates influence data and does not create collision membership.
    let target = game.target.read().unwrap();
    let position = target.get_position();
    game.collision
        .register_object(
            target.get_id(),
            crate::object::collide::Coord3D::new(position.x, position.y, position.z),
            crate::object::collide::collision_geometry::GeometryInfo::new_sphere(
                target.get_geometry_info().get_bounding_circle_radius(),
                false,
            ),
            None,
        )
        .unwrap();
}

#[test]
fn idle_first_frame_restakes_then_uses_supplied_goal_pose_without_peer_mutation() {
    let mut game = super::containment_lifecycle::lifecycle_world(0.0, false);
    let peer = super::containment_lifecycle::lifecycle_world(0.0, false);
    game.operate(11, |world| {
        world.remove_contained(1, 2, false).unwrap();
    });
    let source = Arc::clone(&game.source);
    let peer_position = *peer.source.read().unwrap().get_position();
    let ai = source.read().unwrap().get_ai_update_interface().unwrap();
    let proxy = source
        .read()
        .unwrap()
        .named_update_registrations("AIUpdateInterface")
        .pop()
        .unwrap();
    game.operate_with_current(1, Some(proxy), |world| {
        world
            .set_object_position_handle(&source, &Coord3D::new(53.25, 54.75, 0.0))
            .unwrap();
        ai.lock()
            .unwrap()
            .update_locomotor_phase_with_world(world)
            .unwrap();
        // Re-enter Idle from an actual different state. The fixture's earlier
        // lifecycle tick has already consumed its first Idle initialization.
        ai.lock()
            .unwrap()
            .execute_command_with_world(
                &crate::ai::AiCommandParams::new(
                    crate::ai::AiCommandType::Busy,
                    crate::ai::CommandSourceType::FromAi,
                ),
                world,
            )
            .unwrap();
        ai.lock()
            .unwrap()
            .execute_command_with_world(
                &crate::ai::AiCommandParams::new(
                    crate::ai::AiCommandType::Idle,
                    crate::ai::CommandSourceType::FromAi,
                ),
                world,
            )
            .unwrap();
        ai.lock()
            .unwrap()
            .update_idle_phase_sleep_with_world(world)
            .unwrap();
        let goal_cell = ai
            .lock()
            .unwrap()
            .unit_ai_for_test()
            .unwrap()
            .native_waypoint_observation_for_test()
            .pathfind_goal_cell;
        let source = source.read().unwrap();
        let goal = world
            .idle_goal_position(&source, goal_cell)
            .unwrap()
            .expect("Idle must stake the actual owner before goal snapping");
        assert_eq!(*source.get_position(), goal);
    });
    assert_eq!(*peer.source.read().unwrap().get_position(), peer_position);
}

#[test]
fn repulsor_query_reads_actual_rule_and_partition_owner_without_peer_selection() {
    let mut game = NativeMatch::new(90.0);
    let mut peer = NativeMatch::new(90.0);
    admit_actual_search_partition(&mut game);
    admit_actual_search_partition(&mut peer);
    game.rules.enable_repulsors = true;
    peer.rules.enable_repulsors = false;
    game.target
        .write()
        .unwrap()
        .set_status(crate::common::ObjectStatusMaskType::REPULSOR, true);
    peer.target
        .write()
        .unwrap()
        .set_status(crate::common::ObjectStatusMaskType::REPULSOR, true);
    let source = Arc::clone(&game.source);
    let peer_source = Arc::clone(&peer.source);
    game.operate(31, |world| {
        assert_eq!(
            world
                .closest_repulsor(&source.read().unwrap(), 100.0)
                .unwrap(),
            Some(2)
        );
        assert_eq!(
            world
                .closest_repulsor(&source.read().unwrap(), 2.0)
                .unwrap(),
            None
        );
    });
    peer.operate(31, |world| {
        assert_eq!(
            world
                .closest_repulsor(&peer_source.read().unwrap(), 100.0)
                .unwrap(),
            None
        );
    });
}

#[test]
fn scheduled_repulsor_escape_queues_and_installs_a_receiving_path() {
    const ESCAPE_LOOP: &str = "NativeRepulsorEscapeLoop";
    fn install_escape_audio(audio: &mut AudioManager) {
        audio.init();
        audio.register_audio_event_info(game_engine::common::audio::AudioEventInfo {
            audio_name: ESCAPE_LOOP.into(),
            volume: 1.0,
            sounds: vec!["escape.wav".into()],
            filename: "escape.wav".into(),
            type_field: game_engine::common::audio::ST_GLOBAL,
            control: game_engine::common::audio::AC_LOOP,
            pitch_shift_min: 1.0,
            pitch_shift_max: 1.0,
            ..Default::default()
        });
    }

    let services = crate::system::engine_stores::new_world_services();
    crate::system::engine_stores::with_world_services(&services, || {
        let mut base = DefaultThingTemplate::new("NativeRepulsibleInfantry".into());
        base.add_kind_of(KindOf::Infantry);
        base.add_kind_of(KindOf::CanBeRepulsed);
        base.parse_object_fields_from_ini(&[("SoundMoveLoop".into(), ESCAPE_LOOP.into())].into())
            .unwrap();
        let mut body = crate::object::body::active_body::ActiveBodyModuleData::default();
        body.initial_health = 100.0;
        body.max_health = 100.0;
        let locomotor_name = "NativeRepulsorInfantryGround";
        let mut locomotor = crate::locomotor::LocomotorTemplate::new_infantry(locomotor_name.into());
        locomotor.surfaces = crate::locomotor::SURFACE_GROUND;
        locomotor.max_speed = 3.0;
        locomotor.acceleration = 0.1;
        locomotor.max_turn_rate = 0.1;
        crate::system::engine_stores::active_services()
            .locomotor_store()
            .register_template(locomotor.clone());
        let mut ai_data = crate::object::update::AIUpdateModuleData::default();
        ai_data.set_locomotor_set_entries(
            crate::common::LocomotorSetType::Normal,
            vec![locomotor_name.into()],
        );
        let modules = vec![
            TemplateModuleInfo {
                name: "AIUpdateInterface".into(),
                module_tag: "NativeRepulsorAI".into(),
                data: game_engine::common::thing::module::ModuleDataHandle::new(ai_data),
                interface_mask: ModuleInterfaceType::UPDATE,
            },
            TemplateModuleInfo {
                name: "ActiveBody".into(),
                module_tag: "NativeRepulsorBody".into(),
                data: game_engine::common::thing::module::ModuleDataHandle::new(body),
                interface_mask: ModuleInterfaceType::BODY,
            },
            TemplateModuleInfo {
                name: "PhysicsBehavior".into(),
                module_tag: "NativeRepulsorPhysics".into(),
                data: game_engine::common::thing::module::ModuleDataHandle::new(
                    crate::object::behavior::physics_update::PhysicsBehaviorModuleData::default(),
                ),
                interface_mask: ModuleInterfaceType::UPDATE,
            },
        ];
        let infantry: Arc<dyn ThingTemplate> = Arc::new(EntryTemplate { base, modules });
        let structure = container_template();
        let catalog = super::object_restore_tests::Catalog::from_templates(
            Arc::clone(&infantry),
            Arc::clone(&structure),
            locomotor,
        );
        let mut receiver = NativeMatch::with_source_template(
            60.0,
            structure,
            false,
            infantry.clone(),
        );
        let mut peer = NativeMatch::with_source_template(95.0, container_template(), false, infantry);
        install_escape_audio(&mut receiver.audio);
        install_escape_audio(&mut peer.audio);
        admit_actual_search_partition(&mut receiver);
        admit_actual_search_partition(&mut peer);
        receiver.rules.enable_repulsors = true;
        receiver.rules.repulsed_distance = 25.0;
        peer.rules.enable_repulsors = false;
        peer.rules.repulsed_distance = 3.0;
        for world in [&mut receiver, &mut peer] {
            world.source.write().unwrap().set_vision_range(20.0);
            world
                .target
                .write()
                .unwrap()
                .set_status(crate::common::ObjectStatusMaskType::REPULSOR, true);
        }
        let tick = |game: &mut NativeMatch, frame| {
            let ai = game.source.read().unwrap().get_ai_update_interface().unwrap();
            let proxy = game
                .source
                .read()
                .unwrap()
                .named_update_registrations("AIUpdateInterface")
                .pop()
                .unwrap();
            game.operate_with_current(frame, Some(proxy), |world| {
                {
                    let mut ai = ai.lock().unwrap();
                    if ai.get_current_state_id().is_none() {
                        ai.execute_command_with_world(
                            &AiCommandParams::new(AiCommandType::Idle, CommandSourceType::FromAI),
                            world,
                        )
                        .unwrap();
                    }
                    ai.update_scheduled_tick_with_world(world).unwrap();
                }
                world.process_enter_path_queue(8).unwrap();
            });
        };
        let away = Some(crate::modules::AIStateType::MoveAwayFromRepulsors as u32);
        let repulsor = *receiver.target.read().unwrap().get_position();
        tick(&mut peer, 7);
        tick(&mut receiver, 131);
        let (state, has_path, end, _, layer) = installed_path_observation(&receiver);
        assert_eq!(state, away, "native Idle must enter the receiving panic state");
        assert!(has_path, "the receiving pathfinder must install the queued safe path");
        assert_eq!(receiver.audio.pending_play_request_count_for(ESCAPE_LOOP), 1);
        assert_eq!(peer.audio.pending_play_request_count_for(ESCAPE_LOOP), 0);
        let end = end.unwrap();
        assert!((end.x - repulsor.x).hypot(end.y - repulsor.y) > 45.0);
        assert_eq!(layer, Some(crate::common::PathfindLayerEnum::Ground as u8));
        let peer_path = installed_path_observation(&peer);
        assert_ne!(peer_path.0, away);
        assert!(!peer_path.1);

        // Commit the ordinary Core Object blocks while escape is in flight.
        // The new receiving world must resume the same scheduled move.
        let source_name = receiver.source.read().unwrap().get_template().get_name().to_string();
        let target_name = receiver.target.read().unwrap().get_template().get_name().to_string();
        let restore_with_sound = |saved: &[_], frame, state| {
            let mut restored = super::object_restore_tests::prepare_with_templates(
                &catalog,
                Some(saved),
                &source_name,
                &target_name,
            )
            .unwrap();
            install_escape_audio(&mut restored.audio);
            super::object_restore_tests::transfer(&mut restored, &catalog, frame, |services| {
                services.load_objects(saved).unwrap();
            });
            assert_eq!(installed_path_observation(&restored).0, state);
            assert_eq!(restored.audio.pending_play_request_count_for(ESCAPE_LOOP), 0);
            super::object_restore_tests::finish_restored_objects(&mut restored, saved, frame)
                .unwrap();
            let sounds: Vec<_> = restored
                .audio
                .pending_play_events()
                .filter(|event| event.get_event_name() == ESCAPE_LOOP)
                .map(|event| event.get_object_id())
                .collect();
            assert_eq!(
                sounds,
                [restored.source.read().unwrap().get_id()],
                "movement postload submits one loop for the receiving object",
            );
            restored
        };
        let saved = super::object_restore_tests::capture_at(&mut receiver, &catalog, 131);
        let mut restored = restore_with_sound(&saved, 131, away);
        assert_eq!(peer.audio.pending_play_request_count_for(ESCAPE_LOOP), 0);
        restored.rules = receiver.rules.clone();
        tick(&mut restored, 132);
        let restored_path = installed_path_observation(&restored);
        assert_eq!(restored_path.0, away);
        assert!(restored_path.1);
        assert_eq!(restored_path.2, Some(end));
        assert_eq!(restored_path.4, layer);

        // The next scheduled update must consume the path without replacing it
        // with a normal move request to the old goal.
        tick(&mut receiver, 132);
        let next_path = installed_path_observation(&receiver);
        assert_eq!(next_path.0, away);
        assert!(next_path.1);
        assert_eq!(next_path.2, Some(end));

        // Run the receiving Physics phase as the real object follows the
        // escape route. C++ enters WanderInPlace when the movement completes.
        let actor = Arc::clone(&receiver.source);
        let physics_proxy = actor
            .read()
            .unwrap()
            .named_update_registrations("PhysicsBehavior")
            .pop()
            .unwrap();
        let restored_actor = Arc::clone(&restored.source);
        let restored_physics_proxy = restored_actor
            .read()
            .unwrap()
            .named_update_registrations("PhysicsBehavior")
            .pop()
            .unwrap();
        let wander = Some(crate::modules::AIStateType::WanderInPlace as u32);
        let mut arrival_frame = None;
        for frame in 133..=500 {
            tick(&mut receiver, frame);
            tick(&mut restored, frame);
            let state = installed_path_observation(&receiver).0;
            assert_eq!(installed_path_observation(&restored).0, state);
            if state == wander {
                arrival_frame = Some(frame);
                break;
            }
            assert_eq!(state, away);
            receiver.operate_with_current(frame, Some(physics_proxy.clone()), |world| {
                world.update_physics_phase(&actor).unwrap();
            });
            restored.operate_with_current(frame, Some(restored_physics_proxy.clone()), |world| {
                world.update_physics_phase(&restored_actor).unwrap();
            });
            assert_eq!(
                actor.read().unwrap().get_position(),
                restored_actor.read().unwrap().get_position(),
                "restored escape pose at frame {frame}",
            );
        }
        let position = *actor.read().unwrap().get_position();
        let arrival_frame = arrival_frame.unwrap_or_else(|| {
            panic!("unit did not follow its safe path to {end:?}; final position {position:?}")
        });
        assert!((position.x - repulsor.x).hypot(position.y - repulsor.y) > 20.0);
        tick(&mut receiver, arrival_frame + 1);
        assert_eq!(installed_path_observation(&receiver).0, wander);
        let saved = super::object_restore_tests::capture_at(
            &mut receiver,
            &catalog,
            arrival_frame + 1,
        );
        let resumed_wander = restore_with_sound(&saved, arrival_frame + 1, wander);
        let (_, restored_has_path, restored_end, _, restored_layer) =
            installed_path_observation(&resumed_wander);
        let (_, has_path, end, _, layer) = installed_path_observation(&receiver);
        assert_eq!((restored_has_path, restored_end, restored_layer), (has_path, end, layer));
        assert_eq!(
            resumed_wander.source.read().unwrap().get_position(),
            receiver.source.read().unwrap().get_position(),
        );
        assert_eq!(installed_path_observation(&peer), peer_path);
        assert_eq!(peer.audio.pending_play_request_count_for(ESCAPE_LOOP), 0);
    });
}

#[test]
fn ordered_enemy_query_obeys_actual_relationship_and_range() {
    let mut hostile = NativeMatch::with_target_owner(90.0, container_template(), true);
    let mut allied = NativeMatch::with_target_owner(90.0, container_template(), false);
    admit_actual_search_partition(&mut hostile);
    admit_actual_search_partition(&mut allied);
    let source = Arc::clone(&hostile.source);
    let peer_source = Arc::clone(&allied.source);
    hostile.operate(41, |world| {
        let source = source.read().unwrap();
        assert_eq!(
            world
                .closest_mood_enemy(
                    &source,
                    100.0,
                    crate::ai::search_qualifiers::ATTACK_BUILDINGS
                )
                .unwrap(),
            Some(2)
        );
        assert_eq!(
            world
                .closest_mood_enemy(&source, 2.0, crate::ai::search_qualifiers::ATTACK_BUILDINGS)
                .unwrap(),
            None
        );
    });
    allied.operate(41, |world| {
        assert_eq!(
            world
                .closest_mood_enemy(
                    &peer_source.read().unwrap(),
                    100.0,
                    crate::ai::search_qualifiers::ATTACK_BUILDINGS
                )
                .unwrap(),
            None
        );
    });
}

#[test]
fn mood_priority_reads_receiving_script_definitions_without_peer_override() {
    let mut game = NativeMatch::with_target_owner(90.0, container_template(), true);
    let mut peer = NativeMatch::with_target_owner(90.0, container_template(), true);
    admit_actual_search_partition(&mut game);
    admit_actual_search_partition(&mut peer);
    let name = game
        .target
        .read()
        .unwrap()
        .get_template()
        .get_name()
        .to_string();
    let scripts = &game.disposal.scripts;
    assert!(
        scripts
            .set_priority_thing_with_lookup("OwnedIdlePriority", &name, 0, |query| (query == name)
                .then(|| name.clone()))
    );
    scripts.set_object_attack_priority_set(1, "OwnedIdlePriority");
    let source = Arc::clone(&game.source);
    let peer_source = Arc::clone(&peer.source);
    game.operate(43, |world| {
        assert_eq!(
            world
                .closest_mood_enemy(
                    &source.read().unwrap(),
                    100.0,
                    crate::ai::search_qualifiers::ATTACK_BUILDINGS
                )
                .unwrap(),
            None
        );
    });
    peer.operate(43, |world| {
        assert_eq!(
            world
                .closest_mood_enemy(
                    &peer_source.read().unwrap(),
                    100.0,
                    crate::ai::search_qualifiers::ATTACK_BUILDINGS
                )
                .unwrap(),
            Some(2)
        );
    });
    assert!(game.disposal.scripts.set_priority_thing_with_lookup(
        "OwnedIdlePriority",
        &name,
        7,
        |query| (query == name).then(|| name.clone())
    ));
    game.operate(44, |world| {
        assert_eq!(
            world
                .closest_mood_enemy(
                    &source.read().unwrap(),
                    100.0,
                    crate::ai::search_qualifiers::ATTACK_BUILDINGS
                )
                .unwrap(),
            Some(2)
        );
    });
}

#[test]
fn mood_range_uses_receiving_player_and_attitude_rules() {
    let mut game = NativeMatch::new(90.0);
    let mut peer = NativeMatch::new(90.0);
    game.rules.guard_outer_modifier_ai = 2.0;
    game.rules.guard_outer_modifier_human = 3.0;
    game.rules.aggressive_range_modifier = 4.0;
    peer.rules.guard_outer_modifier_ai = 5.0;
    peer.rules.guard_outer_modifier_human = 6.0;
    peer.rules.aggressive_range_modifier = 7.0;
    game.source.write().unwrap().set_vision_range(10.0);
    peer.source.write().unwrap().set_vision_range(10.0);
    let source = Arc::clone(&game.source);
    let peer_source = Arc::clone(&peer.source);
    game.operate(45, |world| {
        let source = source.read().unwrap();
        let human = world.controller_is_human(&source) == Some(true);
        assert_eq!(
            world
                .mood_vision_range(&source, crate::modules::AIAttitudeType::Aggressive)
                .unwrap(),
            if human { 30.0 } else { 80.0 }
        );
    });
    peer.operate(45, |world| {
        let source = peer_source.read().unwrap();
        let human = world.controller_is_human(&source) == Some(true);
        assert_eq!(
            world
                .mood_vision_range(&source, crate::modules::AIAttitudeType::Aggressive)
                .unwrap(),
            if human { 60.0 } else { 350.0 }
        );
    });
}

#[test]
fn owned_dispatch_sleeping_computer_ignores_commands() {
    // C++ AIUpdate.cpp:2572-2590 isAllowedToRespondToAiCommands gates aiDoCommand.
    let services = crate::system::engine_stores::new_world_services();
    crate::system::engine_stores::with_world_services(&services, || {
        let mut game = NativeMatch::new(50.0);
        game.players
            .get_player(0)
            .unwrap()
            .write()
            .unwrap()
            .set_player_type(crate::player::PlayerType::Computer, true);
        let source = Arc::clone(&game.source);
        let ai = source.read().unwrap().get_ai_update_interface().unwrap();
        game.operate(0, |world| {
            ai.lock()
                .unwrap()
                .execute_command_with_world(
                    &crate::ai::AiCommandParams::new(
                        crate::ai::AiCommandType::Idle,
                        crate::ai::CommandSourceType::FromAi,
                    ),
                    world,
                )
                .unwrap();
        });
        ai.lock()
            .unwrap()
            .set_attitude(crate::modules::AIAttitudeType::Sleep)
            .unwrap();
        let busy = Some(crate::modules::AIStateType::Busy as u32);
        assert_ne!(ai.lock().unwrap().get_current_state_id(), busy);
        game.operate(1, |world| {
            ai.lock()
                .unwrap()
                .execute_command_with_world(
                    &crate::ai::AiCommandParams::new(
                        crate::ai::AiCommandType::Busy,
                        crate::ai::CommandSourceType::FromScript,
                    ),
                    world,
                )
                .unwrap();
        });
        assert_ne!(
            ai.lock().unwrap().get_current_state_id(),
            busy,
            "sleeping computer AI must ignore owned Busy"
        );
        ai.lock()
            .unwrap()
            .set_attitude(crate::modules::AIAttitudeType::Normal)
            .unwrap();
        game.operate(2, |world| {
            ai.lock()
                .unwrap()
                .execute_command_with_world(
                    &crate::ai::AiCommandParams::new(
                        crate::ai::AiCommandType::Busy,
                        crate::ai::CommandSourceType::FromScript,
                    ),
                    world,
                )
                .unwrap();
        });
        assert_eq!(ai.lock().unwrap().get_current_state_id(), busy);
    });
}
