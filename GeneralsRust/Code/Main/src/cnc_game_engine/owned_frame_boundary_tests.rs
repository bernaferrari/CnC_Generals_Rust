//! Completed frames read timers, health and presentation events from the receiving world.

use super::*;

fn advance_owned(logic: &mut GameLogic) {
    let snapshot = CnCGameEngine::host_update_logic_frame(
        logic,
        false,
        1.0 / 30.0,
        None,
        Some(1),
        |_| {},
        GameLogic::complete_presentation_observations,
    );
    assert_eq!(snapshot.steps_run, 1);
}

fn advance_owned_with_test_audio(
    logic: &mut GameLogic,
    manager: &mut crate::subsystem_manager::SubsystemManager,
) {
    advance_owned(logic);
    let requests = logic.process_audio_events(None);
    if let Some(audio) = manager.get_mut::<crate::subsystem_manager::AudioManagerSubsystem>() {
        for request in requests {
            audio.queue_admitted_event(request);
        }
    }
}

fn sabotage_command_browns_out_the_owner_until_after_its_deadline_case() {
    use crate::command_system::{CommandType, GameCommand, ModifierKeys};
    use crate::fixture_content::FixtureObject;
    use crate::game_logic::presentation_definitions::PresentationDefinitions;

    let fixture = |source_template, modules: &[&str]| {
        let mut source = FixtureObject::new(source_template)
            .kind_of("INFANTRY SELECTABLE")
            .health(120.)
            .walker()
            .instant_death();
        for (index, fields) in modules.iter().enumerate() {
            source = source.module(format!(
                "Behavior = SabotagePowerPlantCrateCollide SabotagePower{index}\n{fields}\nEnd"
            ));
        }
        let (mut world, _) = fixture_with(
            "SabotageWitness",
            100.0,
            &[
                source,
                FixtureObject::new("AmericaPowerPlant")
                    .kind_of("STRUCTURE FS_POWER SELECTABLE")
                    .energy(20)
                    .health(1000.)
                    .radar_provider(),
                FixtureObject::new("SabotageConsumer")
                    .kind_of("STRUCTURE POWERED")
                    .energy(-8)
                    .health(1000.),
            ],
        );
        let victim = world.get_player_by_team(Team::USA).unwrap().id;
        let infiltrator = world.get_player_by_team(Team::GLA).unwrap().id;
        let plant = world
            .create_object_for_player("AmericaPowerPlant", victim, Vec3::new(90., 0., 90.))
            .unwrap();
        let consumer = world
            .create_object_for_player("SabotageConsumer", victim, Vec3::new(120., 0., 90.))
            .unwrap();
        let saboteur = world
            .create_object_for_player(source_template, infiltrator, Vec3::new(92., 0., 90.))
            .unwrap();
        world.session_view.definitions =
            PresentationDefinitions::with_radar_cues("SabotageRadarUp", "SabotageRadarDown");
        advance_owned(&mut world);
        assert!(world.get_player(victim).unwrap().has_radar());
        (world, victim, infiltrator, plant, consumer, saboteur)
    };
    let saboteur_template = "GLAInfantrySaboteur";
    let (mut world, victim, infiltrator, plant, consumer, saboteur) = fixture(
        saboteur_template,
        &[
            " SabotagePowerDuration = 6000",
            " BuildingPickup = Yes\n RequiredKindOf = COMMANDCENTER\n SabotagePowerDuration = 4000",
            " BuildingPickup = Yes\n ForbiddenKindOf = FS_POWER\n SabotagePowerDuration = 3000",
            " BuildingPickup = Yes\n SabotagePowerDuration = 1500",
            " BuildingPickup = Yes\n SabotagePowerDuration = 1000",
        ],
    );
    let (mut peer, peer_victim, peer_infiltrator, peer_plant, peer_consumer, peer_saboteur) =
        fixture(saboteur_template, &[" BuildingPickup = Yes\n SabotagePowerDuration = 2000"]);
    assert_eq!(
        (victim, plant, consumer, saboteur),
        (peer_victim, peer_plant, peer_consumer, peer_saboteur)
    );
    let contact_frame = world.get_frame();
    let sabotage = |world: &mut GameLogic, player_id, source, target_id| {
        world.queue_command(GameCommand {
            command_type: CommandType::Sabotage { target_id },
            player_id,
            command_id: 1,
            timestamp: std::time::UNIX_EPOCH,
            selected_units: vec![source],
            modifier_keys: ModifierKeys::default(),
        });
    };
    sabotage(&mut world, infiltrator, saboteur, plant);
    advance_owned(&mut world);
    let victim_state = world.get_player(victim).unwrap();
    let deadline = victim_state.power_sabotaged_till_frame;
    assert_eq!(deadline, contact_frame + 30);
    assert!(!victim_state.has_radar());
    assert_eq!(victim_state.power_available, -8);
    assert!(
        world
            .host_object(consumer)
            .unwrap()
            .status
            .disabled_underpowered
    );
    assert!(
        world
            .host_object(saboteur)
            .is_none_or(|object| !object.is_alive())
    );
    assert_eq!(world.host_radar.offline_transitions, 1);

    for (processed_frame, powered) in [
        (deadline - 1, false),
        (deadline, false),
        (deadline + 1, true),
    ] {
        world.set_current_frame(u64::from(processed_frame));
        advance_owned(&mut world);
        let player = world.get_player(victim).unwrap();
        assert_eq!(world.get_frame(), processed_frame + 1);
        assert_eq!(
            player.power_sabotaged_till_frame,
            if powered { 0 } else { deadline }
        );
        assert_eq!(
            player.has_sufficient_energy_at(processed_frame),
            processed_frame >= deadline
        );
        assert_eq!(player.has_radar(), powered);
        assert_eq!(
            world
                .host_object(consumer)
                .unwrap()
                .status
                .disabled_underpowered,
            !powered
        );
        assert_eq!(world.host_radar.online_transitions, 1 + u32::from(powered));
    }
    let events = world.process_audio_events(None);
    assert_eq!(
        events
            .iter()
            .filter(|event| {
                event.event_type == crate::game_logic::host_saboteur::SABOTEUR_SHUTDOWN_AUDIO
            })
            .count(),
        2
    );
    let radar_cues: Vec<_> = events
        .into_iter()
        .filter(|event| {
            matches!(
                event.event_type.as_str(),
                "SabotageRadarDown" | "SabotageRadarUp"
            )
        })
        .map(|event| (event.event_type.clone(), event.player_index))
        .collect();
    assert_eq!(
        radar_cues,
        [
            ("SabotageRadarDown".to_owned(), Some(victim as i32)),
            ("SabotageRadarUp".to_owned(), Some(victim as i32)),
        ]
    );
    advance_owned(&mut peer);
    assert!(peer.get_player(peer_victim).unwrap().has_radar());
    assert_eq!(
        peer.get_player(peer_victim)
            .unwrap()
            .power_sabotaged_till_frame,
        0
    );
    assert!(
        !peer
            .host_object(peer_consumer)
            .unwrap()
            .status
            .disabled_underpowered
    );
    assert!(peer.host_object(peer_saboteur).unwrap().is_alive());
    assert_eq!(peer.host_radar.offline_transitions, 0);
    let peer_contact = peer.get_frame();
    sabotage(&mut peer, peer_infiltrator, peer_saboteur, peer_plant);
    advance_owned(&mut peer);
    let player = peer.get_player(peer_victim).unwrap();
    assert_eq!(player.power_sabotaged_till_frame, peer_contact + 60);
    assert!(!player.has_radar());
    assert!(world.get_player(victim).unwrap().has_radar());

    // An authored module with no duration starts an immediate outage with
    // a deadline equal to the contact frame. The name grants no capability.
    let (mut default, victim, infiltrator, plant, consumer, saboteur) =
        fixture("MissionInfiltrator", &[" BuildingPickup = Yes"]);
    let contact = default.get_frame();
    sabotage(&mut default, infiltrator, saboteur, plant);
    advance_owned(&mut default);
    assert_eq!(
        default
            .get_player(victim)
            .unwrap()
            .power_sabotaged_till_frame,
        contact
    );
    assert!(!default.get_player(victim).unwrap().has_radar());
    assert!(
        default
            .host_object(consumer)
            .unwrap()
            .status
            .disabled_underpowered
    );
    advance_owned(&mut default);
    assert!(default.get_player(victim).unwrap().has_radar());

    // A name-compatible template with the module removed leaves the grid alone.
    let (mut absent, victim, infiltrator, plant, consumer, saboteur) =
        fixture(saboteur_template, &[]);
    sabotage(&mut absent, infiltrator, saboteur, plant);
    advance_owned(&mut absent);
    assert_eq!(
        absent
            .get_player(victim)
            .unwrap()
            .power_sabotaged_till_frame,
        0
    );
    assert!(absent.get_player(victim).unwrap().has_radar());
    assert!(
        !absent
            .host_object(consumer)
            .unwrap()
            .status
            .disabled_underpowered
    );
    assert!(absent.host_object(saboteur).unwrap().is_alive());
}

#[test]
fn sabotage_command_browns_out_the_owner_until_after_its_deadline() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "sabotage_command_browns_out_the_owner_until_after_its_deadline",
        sabotage_command_browns_out_the_owner_until_after_its_deadline_case,
    );
}

fn author_timers(logic: &mut GameLogic, id: ObjectId) -> u32 {
    let (object, health_events) = logic.host_object_and_health_events_mut(id).unwrap();
    object.flash_as_selected_with_color([0.1, 0.2, 0.3]);
    object.apply_disabled_emp(
        1,
        health_events,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    object.apply_disabled_hacked(
        2,
        health_events,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    object.begin_undetected_defection(0, 1, false);
    object.selection_flash_remaining
}

fn assert_timers(logic: &GameLogic, id: ObjectId, initial_flash: u32) {
    let frame = logic.get_frame();
    let object = logic.host_object(id).expect("live authored unit");
    assert_eq!(object.selection_flash_remaining, initial_flash - frame);
    // GameLogic expires disabled flags at >= the deadline after gameplay. This adapter
    // reports the next frame after completing the previous frame's phases.
    assert_eq!(object.status.disabled_emp, frame <= 1);
    assert_eq!(object.status.disabled_hacked, frame <= 2);
    assert_eq!(object.is_undetected_defector(), frame <= 1);
}

fn ordinary_frames_preserve_owned_timer_deadlines_and_save_continuation_case() {
    let (mut source, id) = fixture("OwnedFrameTimers", 100.0);
    let initial_flash = author_timers(&mut source, id);
    advance_owned(&mut source);
    assert_timers(&source, id, initial_flash);

    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    // Native restore requires the receiving map's actual team definitions.
    // Re-admit the same content rather than sharing the source factory.
    let (mut restored, restored_id) = fixture("OwnedFrameTimers", 100.0);
    assert_eq!(restored_id, id);
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    let mut stale = PresentationFrame::try_build_from_logic(&mut source, 0).unwrap();
    let stale_row = stale.objects.iter_mut().find(|row| row.id == id).unwrap();
    stale_row.health_current = 999.0;
    stale_row.selection_flash_remaining = 999;

    for _ in 0..2 {
        for world in [&mut source, &mut restored] {
            advance_owned(world);
            assert_timers(world, id, initial_flash);
            assert_eq!(world.host_authoritative_health(id), Some(100.0));
            let frame = PresentationFrame::try_build_from_logic(world, 0).unwrap();
            let row = frame_object(&frame, id).unwrap();
            assert_eq!(row.health_current, 100.0);
            assert_eq!(row.selection_flash_remaining, initial_flash - frame.frame.0);
        }
        assert_eq!(source.get_frame(), restored.get_frame());
        assert_eq!(frame_object(&stale, id).unwrap().health_current, 999.0);
    }
}

fn publishing_one_world_keeps_same_id_worlds_independent_case() {
    let (mut a, id_a) = fixture("OwnedObserved", 100.0);
    let (mut b, id_b) = fixture("OwnedUnobserved", 240.0);
    assert_eq!(id_a, id_b);
    let flash_a = author_timers(&mut a, id_a);
    let flash_b = author_timers(&mut b, id_b);
    for _ in 0..3 {
        advance_owned(&mut a);
        let frame = PresentationFrame::try_build_from_logic(&mut a, 0).unwrap();
        let row = frame_object(&frame, id_a).unwrap();
        assert_eq!(row.health_current, 100.0);
        assert_eq!(row.selection_flash_remaining, flash_a - frame.frame.0);
        advance_owned(&mut b);
        assert_timers(&a, id_a, flash_a);
        assert_timers(&b, id_b, flash_b);
        assert_eq!(a.host_authoritative_health(id_a), Some(100.0));
        assert_eq!(b.host_authoritative_health(id_b), Some(240.0));
    }
}

fn ordinary_boundary_admits_owned_damage_and_removes_dead_objects_before_next_frame_case() {
    let (mut logic, id) = fixture("OwnedFrameDamage", 100.0);
    let before = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let (object, events) = logic.host_object_and_health_events_mut(id).unwrap();
    assert!(!object.take_damage_from(40.0, None, 0, events));
    assert_eq!(logic.host_authoritative_health(id), Some(60.0));
    advance_owned(&mut logic);
    assert_eq!(logic.host_authoritative_health(id), Some(60.0));
    let damaged = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    assert_eq!(frame_object(&damaged, id).unwrap().health_current, 60.0);
    assert_eq!(frame_object(&before, id).unwrap().health_current, 100.0);

    logic
        .apply_owned_damage(
            id,
            1000.0,
            None,
            crate::game_logic::combat::DamageType::Unresistable,
            crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            None,
            &crate::game_logic::object::DamageHitContext::new(
                None,
                None,
                crate::game_logic::combat::DamageType::Unresistable,
            ),
        )
        .expect("receiving body and onDie operation");
    for _ in 0..2 {
        advance_owned(&mut logic);
        assert!(
            logic.host_object(id).is_none(),
            "owner destroy queue must complete"
        );
        let frame = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
        assert!(
            frame_object(&frame, id).is_none(),
            "retired ID cannot reappear"
        );
    }
    assert_eq!(frame_object(&damaged, id).unwrap().health_current, 60.0);
}

fn ordinary_boundary_preserves_actual_move_commands_for_presentation_case() {
    use crate::presentation_frame::{PresentationEvent, PresentationFrame};
    let (mut logic, id) = fixture("OwnedFrameMove", 100.0);
    let (mut other, other_id) = fixture("OtherFrameMove", 100.0);
    assert_eq!(id, other_id, "numeric object IDs deliberately collide");
    let session = logic.movement_events.session().clone();
    let other_session = other.movement_events.session().clone();
    assert_ne!(session, other_session);

    let first = Vec3::new(23.0, 0.0, 24.0);
    assert!(logic.append_unit_waypoint(id, first));
    advance_owned(&mut logic);
    let first_completed = logic.movement_events.snapshot_completed().to_vec();
    assert!(
        first_completed
            .iter()
            .any(|event| { event.unit == id && event.destination == Some(first.to_array()) }),
        "the real path command must reach completed frame output"
    );

    let second = Vec3::new(43.0, 0.0, 44.0);
    let foreign = Vec3::new(83.0, 0.0, 84.0);
    assert!(logic.append_unit_waypoint(id, second));
    assert!(other.append_unit_waypoint(other_id, foreign));
    let pending = logic.movement_events.snapshot_pending().to_vec();
    // Constructing, resetting and dropping another world cannot clear this one.
    let mut reset_world = GameLogic::new();
    reset_world.reset();
    drop(reset_world);
    advance_owned(&mut other);
    assert_eq!(logic.movement_events.snapshot_pending(), pending);
    advance_owned(&mut logic);
    let completed = logic.movement_events.snapshot_completed().to_vec();
    assert!(
        completed.starts_with(&first_completed),
        "the next nonempty fixed step must retain all prior observations"
    );
    assert!(
        completed
            .iter()
            .any(|event| { event.unit == id && event.destination == Some(second.to_array()) })
    );
    assert!(completed.iter().all(|event| event.session == session));
    assert!(
        completed
            .windows(2)
            .all(|pair| pair[0].sequence < pair[1].sequence)
    );
    assert!(completed.iter().any(|event| event.logic_frame == 0));
    assert!(completed.iter().any(|event| event.logic_frame == 1));

    let foreign_frame = PresentationFrame::try_build_from_logic(&mut other, 0).unwrap();
    assert!(foreign_frame.events.iter().any(|event| matches!(event,
        PresentationEvent::MoveOrdered { session, destination, .. }
            if *session == other_session && *destination == foreign.to_array()
    )));
    assert_eq!(logic.movement_events.snapshot_completed(), completed);
    let frame = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let frozen: Vec<_> = frame
        .events
        .iter()
        .filter_map(|event| match event {
            PresentationEvent::MoveOrdered {
                unit,
                destination,
                session,
                logic_frame,
                sequence,
            } => Some((
                *unit,
                *destination,
                session.clone(),
                *logic_frame,
                *sequence,
            )),
            _ => None,
        })
        .collect();
    let expected: Vec<_> = completed
        .iter()
        .filter_map(|event| {
            event.destination.map(|destination| {
                (
                    event.unit,
                    destination,
                    event.session.clone(),
                    event.logic_frame,
                    event.sequence,
                )
            })
        })
        .collect();
    assert_eq!(
        frozen, expected,
        "publication must preserve producing frame and FIFO receipts"
    );
    let next = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    assert!(
        !next
            .events
            .iter()
            .any(|event| matches!(event, PresentationEvent::MoveOrdered { .. })),
        "completed movement is consumed exactly once"
    );
}

fn ordinary_boundary_keeps_actual_construction_completion_pending_for_presentation_case() {
    use crate::fixture_content::FixtureObject;
    use generals_sim::dozer::DozerBuildSubTask;
    let (mut logic, _) = fixture_with(
        "OwnedFrameBuilder",
        100.0,
        &[
            FixtureObject::new("OwnedFramePower").kind_of("STRUCTURE").energy(10).health(1000.0),
            FixtureObject::new("OwnedFrameBuilding").kind_of("STRUCTURE").build_time(1.0).health(500.0),
            FixtureObject::new("OwnedFrameDozer").kind_of("VEHICLE DOZER").health(200.0).walker().dozer(),
        ],
    );
    crate::fixture_content::admit_retail_production_energy_rules(&mut logic);
    // C++ Energy returns zero production for an unpowered player even when
    // consumption is zero. Supply a real plant for this full-frame witness.
    let plant = logic
        .create_object("OwnedFramePower", Team::USA, Vec3::new(40.0, 0.0, 40.0))
        .expect("actual power admission");
    // A plant under construction supplies no energy (Energy.cpp only counts
    // completed buildings), so the builder's grid would stay empty; complete it.
    {
        let plant = logic.host_object_mut(plant).expect("plant");
        plant.set_status_under_construction(false);
        plant.construction_percent = 1.0;
    }
    // The real BUILD request (DozerAIUpdate.cpp:1948-2008 newTask): the
    // builder places an authored scaffold and its own DozerAIUpdate enters
    // the construction phase; no target/AI state is written by hand.
    let dozer = logic
        .create_object("OwnedFrameDozer", Team::USA, Vec3::new(80.0, 0.0, 80.0))
        .expect("actual builder");
    // DozerAIUpdate.cpp:2244-2267 selects only orders after frame zero.
    advance_owned(&mut logic);
    let owner = logic.host_object(dozer).unwrap().owner_player_id.expect("builder owner");
    let id = logic
        .create_dozer_scaffold(
            "OwnedFrameBuilding",
            owner,
            dozer,
            Vec3::new(80.0, 0.0, 80.0),
            0.0,
            crate::game_logic::Resources::default(),
        )
        .expect("scaffold factory")
        .expect("authored scaffold");
    assert!(logic.dozer_new_task_build(dozer, id).unwrap());
    // Advance owned frames until the builder's first BUILD contribution.
    for _ in 0..600 {
        advance_owned(&mut logic);
        if logic.host_object(id).unwrap().construction_percent > 0.0 {
            break;
        }
    }
    let actor = logic.host_object(dozer).unwrap();
    assert!(
        logic.host_object(id).unwrap().construction_percent > 0.0,
        "dozer never contributed BUILD progress: frame={} request={:?} ai={:?} build={:?} dozer_pos={:?} dock={:?}",
        logic.frame,
        actor.dozer_tasks.requests[0],
        actor.ai_state(),
        actor.dozer_tasks.build_sub_task,
        actor.get_position(),
        actor.dozer_dock_action(),
    );
    assert_eq!(actor.ai_state(), crate::game_logic::AIState::Idle);
    assert_eq!(
        actor.dozer_tasks.build_sub_task,
        DozerBuildSubTask::DoBuildAtDock
    );
    assert!(logic.host_object(id).unwrap().status.under_construction);
    assert!(!logic.construction_events.has_pending(id));
    // One more BUILD frame (1 / 30 of a one-second BuildTime) completes it.
    logic.host_object_mut(id).unwrap().construction_percent = 0.99;
    advance_owned(&mut logic);
    assert!(!logic.host_object(id).unwrap().status.under_construction);
    assert!(logic.construction_events.has_pending(id));

    let frame = crate::presentation_frame::PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    assert!(frame.events.iter().any(|event| matches!(
        event,
        crate::presentation_frame::PresentationEvent::ConstructionComplete {
            id: completed,
            template,
        } if *completed == id && template == "OwnedFrameBuilding"
    )));
    assert!(!logic.construction_events.has_pending(id));
}

fn ordinary_disabled_producer_resumes_only_after_the_expiry_frame_case() {
    use crate::fixture_content::FixtureObject;
    let (mut logic, _) = fixture_with(
        "OwnedDisableClock",
        100.0,
        &[
            FixtureObject::new("OwnedDisableBarracks")
                .kind_of("STRUCTURE FS_BARRACKS")
                .energy(0)
                .health(1000.0)
                .module("Behavior = ProductionUpdate FixtureProduction\nEnd"),
            FixtureObject::new("OwnedDisableInfantry").kind_of("INFANTRY").build_cost(0).build_time(10.0).health(100.0),
        ],
    );
    let producer = logic
        .create_object(
            "OwnedDisableBarracks",
            Team::USA,
            Vec3::new(90.0, 0.0, 90.0),
        )
        .expect("actual producer");
    assert!(logic.enqueue_production(producer, "OwnedDisableInfantry".into()));
    let (object, health_events) = logic.host_object_and_health_events_mut(producer).unwrap();
    object.apply_disabled_emp(
        1,
        health_events,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    object.apply_disabled_hacked(
        1,
        health_events,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    object.apply_disabled_paralyzed(
        1,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );

    for next_frame in 1..=3 {
        advance_owned(&mut logic);
        assert_eq!(logic.get_frame(), next_frame);
        let object = logic.host_object(producer).expect("live producer");
        assert_eq!(object.status.disabled_emp, next_frame == 1);
        assert_eq!(object.status.disabled_hacked, next_frame == 1);
        assert_eq!(object.status.disabled_paralyzed, next_frame == 1);
        let head = &object.building_data.as_ref().unwrap().production_queue[0];
        // C++ GameLogic.cpp:3677/3783-3801 runs ProductionUpdate while still
        // disabled on processed frame 1, then expires the flags and reports 2.
        assert_eq!(head.construction_frames, u32::from(next_frame == 3));
        if next_frame <= 2 {
            assert_eq!(
                head.progress, 0.0,
                "expiry cannot advance production mid-frame"
            );
        } else {
            assert!(
                head.progress > 0.0,
                "the next real production phase resumes"
            );
        }
    }
}

#[test]
fn ordinary_disabled_producer_resumes_only_after_the_expiry_frame() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "ordinary_disabled_producer_resumes_only_after_the_expiry_frame",
        ordinary_disabled_producer_resumes_only_after_the_expiry_frame_case,
    );
}

fn ordinary_expiry_restores_radar_and_generation_in_the_same_completed_frame_case() {
    use crate::game_logic::presentation_definitions::PresentationDefinitions;
    use crate::subsystem_manager::AudioManagerSubsystem;
    let mut audio_manager = crate::subsystem_manager::SubsystemManager::new();
    AudioManagerSubsystem::install_test_queue(&mut audio_manager);
    for disable_generator in [false, true] {
        use crate::fixture_content::FixtureObject;
        let (mut logic, _) = fixture_with(
            "OwnedExpiryEffects",
            100.0,
            &[
                FixtureObject::new("AmericaCommandCenter").kind_of("STRUCTURE COMMANDCENTER").energy(-3).health(1000.0).radar_provider(),
                FixtureObject::new("OwnedExpiryPowerPlant").kind_of("STRUCTURE").energy(10).health(1000.0),
                FixtureObject::new("OwnedExpiryPoweredConsumer").kind_of("STRUCTURE POWERED").energy(-5).health(1000.0),
            ],
        );
        let radar_on = "ExpiryRadarOn";
        let radar_off = "ExpiryRadarOff";
        logic.session_view.definitions = PresentationDefinitions::with_radar_cues(radar_on, radar_off);
        let owner = logic.get_player_by_team(Team::USA).unwrap().id;
        let other = logic
            .get_player(1)
            .expect("actual foreign skirmish slot")
            .id;
        let cc = logic
            .create_object_for_player("AmericaCommandCenter", owner, Vec3::new(90.0, 0.0, 90.0))
            .unwrap();
        let plant = logic
            .create_object_for_player("OwnedExpiryPowerPlant", owner, Vec3::new(110.0, 0.0, 90.0))
            .unwrap();
        logic
            .create_object_for_player("OwnedExpiryPowerPlant", other, Vec3::new(190.0, 0.0, 190.0))
            .unwrap();
        let consumer = logic
            .create_object_for_player(
                "OwnedExpiryPoweredConsumer",
                owner,
                Vec3::new(130.0, 0.0, 90.0),
            )
            .unwrap();
        advance_owned_with_test_audio(&mut logic, &mut audio_manager);
        assert_eq!(logic.get_frame(), 1);
        assert!(logic.get_player(owner).unwrap().has_radar());
        assert_eq!(logic.host_radar.online_transitions, 1);
        assert_eq!(logic.host_radar.offline_transitions, 0);
        let foreign = logic.get_player(other).unwrap();
        let foreign_power = (
            foreign.power_produced,
            foreign.power_consumed,
            foreign.power_available,
        );
        AudioManagerSubsystem::take_test_queue(&mut audio_manager);
        {
            let (object, health_events) = logic
                .host_object_and_health_events_mut(if disable_generator { plant } else { cc })
                .unwrap();
            object.apply_disabled_emp(
                1,
                health_events,
                &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
            );
        };
        advance_owned_with_test_audio(&mut logic, &mut audio_manager);
        assert_eq!(logic.get_frame(), 2, "processed expiry frame 1 completed");
        assert!(!logic.host_object(cc).unwrap().status.disabled_emp);
        assert!(!logic.host_object(plant).unwrap().status.disabled_emp);
        assert!(
            !logic
                .host_object(consumer)
                .unwrap()
                .status
                .disabled_underpowered
        );
        let player = logic.get_player(owner).unwrap();
        assert_eq!(
            player.radar_count, 1,
            "onDisabledEdge adds the real owner's provider immediately"
        );
        assert!(player.has_radar(), "no extra frame of stale disabled radar");
        assert_eq!(
            (
                player.power_produced,
                player.power_consumed,
                player.power_available
            ),
            (10, 8, 2)
        );
        let foreign = logic.get_player(other).unwrap();
        assert_eq!(
            (
                foreign.power_produced,
                foreign.power_consumed,
                foreign.power_available
            ),
            foreign_power
        );
        assert_eq!(logic.host_radar.online_transitions, 2);
        assert_eq!(logic.host_radar.offline_transitions, 1);
        let delivered_audio = AudioManagerSubsystem::take_test_queue(&mut audio_manager);
        let radar_requests: Vec<_> = delivered_audio
            .iter()
            .filter(|event| [radar_off, radar_on].contains(&event.event_type.as_str()))
            .map(|event| (event.event_type.as_str(), event.player_index))
            .collect();
        assert_eq!(
            radar_requests,
            [(radar_off, Some(owner as i32)), (radar_on, Some(owner as i32))]
        );
        advance_owned_with_test_audio(&mut logic, &mut audio_manager);
        assert_eq!(logic.get_player(owner).unwrap().radar_count, 1);
        assert_eq!(
            logic.host_radar.online_transitions, 2,
            "no deferred edge replay"
        );
        assert_eq!(logic.host_radar.offline_transitions, 1);
        assert!(
            AudioManagerSubsystem::take_test_queue(&mut audio_manager)
                .iter()
                .all(|event| {
                    ![radar_off, radar_on].contains(&event.event_type.as_str())
                }),
            "no deferred radar audio replay"
        );
    }
}

#[test]
fn ordinary_expiry_restores_radar_and_generation_in_the_same_completed_frame() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "ordinary_expiry_restores_radar_and_generation_in_the_same_completed_frame",
        ordinary_expiry_restores_radar_and_generation_in_the_same_completed_frame_case,
    );
}

fn ordinary_same_id_worlds_keep_pending_disabled_radar_callbacks_owned_case() {
    let make = || {
        use crate::fixture_content::FixtureObject;
        let (mut logic, _) = fixture_with(
            "OwnedRadarCallbackCanary",
            100.0,
            &[
                FixtureObject::new("AmericaCommandCenter").kind_of("STRUCTURE COMMANDCENTER").energy(-3).health(1000.0).radar_provider(),
                FixtureObject::new("OwnedRadarCallbackPower").kind_of("STRUCTURE").energy(10).health(1000.0),
            ],
        );
        let id = logic
            .create_object(
                "AmericaCommandCenter",
                Team::USA,
                Vec3::new(90.0, 0.0, 90.0),
            )
            .unwrap();
        logic
            .create_object(
                "OwnedRadarCallbackPower",
                Team::USA,
                Vec3::new(110.0, 0.0, 90.0),
            )
            .unwrap();
        advance_owned(&mut logic);
        (logic, id)
    };
    let (mut a, id_a) = make();
    let (mut b, id_b) = make();
    assert_eq!(id_a, id_b);
    let owner = a.get_player_by_team(Team::USA).unwrap().id;
    {
        let (object, health_events) = a.host_object_and_health_events_mut(id_a).unwrap();
        object.apply_disabled_emp(
            1,
            health_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    };
    {
        let (object, health_events) = b.host_object_and_health_events_mut(id_b).unwrap();
        object.apply_disabled_emp(
            10,
            health_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    };
    assert_eq!(
        b.host_object(id_b)
            .unwrap()
            .pending_radar_disabled_edges
            .len(),
        1
    );
    advance_owned(&mut a);
    assert_eq!(a.get_player(owner).unwrap().radar_count, 1);
    assert!(a.get_player(owner).unwrap().has_radar());
    assert!(b.host_object(id_b).unwrap().status.disabled_emp);
    assert_eq!(b.get_frame(), 1);
    assert_eq!(b.get_player(owner).unwrap().radar_count, 1);
    assert_eq!(
        b.host_object(id_b)
            .unwrap()
            .pending_radar_disabled_edges
            .len(),
        1,
        "A cannot consume a same-ID foreign Object's pending owner callback"
    );
    advance_owned(&mut b);
    assert_eq!(b.get_player(owner).unwrap().radar_count, 0);
    assert!(!b.get_player(owner).unwrap().has_radar());
    assert!(
        b.host_object(id_b)
            .unwrap()
            .pending_radar_disabled_edges
            .is_empty()
    );
    assert_eq!(a.get_player(owner).unwrap().radar_count, 1);
    assert!(a.get_player(owner).unwrap().has_radar());
}

#[test]
fn ordinary_same_id_worlds_keep_pending_disabled_radar_callbacks_owned() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "ordinary_same_id_worlds_keep_pending_disabled_radar_callbacks_owned",
        ordinary_same_id_worlds_keep_pending_disabled_radar_callbacks_owned_case,
    );
}

#[test]
fn ordinary_frames_preserve_owned_timer_deadlines_and_save_continuation() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "ordinary_frames_preserve_owned_timer_deadlines_and_save_continuation",
        ordinary_frames_preserve_owned_timer_deadlines_and_save_continuation_case,
    );
}

#[test]
fn publishing_one_world_keeps_same_id_worlds_independent() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "publishing_one_world_keeps_same_id_worlds_independent",
        publishing_one_world_keeps_same_id_worlds_independent_case,
    );
}

#[test]
fn ordinary_boundary_admits_owned_damage_and_removes_dead_objects_before_next_frame() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "ordinary_boundary_admits_owned_damage_and_removes_dead_objects_before_next_frame",
        ordinary_boundary_admits_owned_damage_and_removes_dead_objects_before_next_frame_case,
    );
}

#[test]
fn ordinary_boundary_preserves_actual_move_commands_for_presentation() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "ordinary_boundary_preserves_actual_move_commands_for_presentation",
        ordinary_boundary_preserves_actual_move_commands_for_presentation_case,
    );
}

#[test]
fn ordinary_boundary_keeps_actual_construction_completion_pending_for_presentation() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "ordinary_boundary_keeps_actual_construction_completion_pending_for_presentation",
        ordinary_boundary_keeps_actual_construction_completion_pending_for_presentation_case,
    );
}
