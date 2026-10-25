//! Ordinary production frames borrow their Main owner and publish observations.
//! These witnesses use the engine's fixed-step adapter, without shadow coupling.

use super::*;

fn advance_owned(logic: &mut GameLogic, shadow: Option<&mut GameWorldShadow>) {
    let mut shadow = shadow;
    let snapshot = CnCGameEngine::host_update_logic_frame(
        logic,
        false,
        1.0 / 30.0,
        None,
        Some(1),
        |_| {},
        |owner| {
            gameworld_shadow::run_owned_host_boundary(shadow.as_deref_mut(), owner);
        },
    );
    assert_eq!(snapshot.steps_run, 1);
}

fn advance_owned_with_test_audio(
    logic: &mut GameLogic,
    shadow: Option<&mut GameWorldShadow>,
    manager: &mut crate::subsystem_manager::SubsystemManager,
) {
    advance_owned(logic, shadow);
    let requests = logic.process_audio_events();
    if let Some(audio) = manager.get_mut::<crate::subsystem_manager::AudioManagerSubsystem>() {
        for request in requests {
            audio.queue_event(request);
        }
    }
}

fn author_timers(logic: &mut GameLogic, id: ObjectId) -> u32 {
    let (object, health_events) = logic.host_object_and_health_events_mut(id).unwrap();
    object.flash_as_selected_with_color([0.1, 0.2, 0.3]);
    object.apply_disabled_emp(
        1,
        health_events,
        &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
    );
    object.apply_disabled_hacked(
        2,
        health_events,
        &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
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

fn ordinary_shadow_frames_preserve_owned_timer_deadlines_and_save_continuation_case() {
    let (mut source, id, mut shadow) = fixture("OwnedFrameTimers", 100.0);
    let initial_flash = author_timers(&mut source, id);
    shadow.sync_from_host(&source);
    advance_owned(&mut source, Some(&mut shadow));
    assert_timers(&source, id, initial_flash);

    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    // Native restore requires the receiving map's actual team definitions.
    // Re-admit the same content rather than sharing the source factory.
    let (mut restored, restored_id, _) = fixture("OwnedFrameTimers", 100.0);
    assert_eq!(restored_id, id);
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    let mut restored_shadow = GameWorldShadow::new(64);
    restored_shadow.sync_from_host(&restored);

    for _ in 0..2 {
        // An observation may be stale or corrupt; it cannot reverse-write the
        // owner's health, selection envelope, or authored expiry phase.
        let entity = shadow.entity_for_host(id).unwrap();
        let observer = shadow.world_mut().world_mut().entity_mut(entity).unwrap();
        observer.health = 999.0;
        observer.selection_flash_remaining = 999;
        advance_owned(&mut source, Some(&mut shadow));
        advance_owned(&mut restored, Some(&mut restored_shadow));
        assert_timers(&source, id, initial_flash);
        assert_timers(&restored, id, initial_flash);
        assert_eq!(source.host_authoritative_health(id), Some(100.0));
        assert_eq!(shadow.world().entity(entity).unwrap().health, 100.0);
        assert_eq!(
            shadow
                .world()
                .entity(entity)
                .unwrap()
                .selection_flash_remaining,
            initial_flash - source.get_frame()
        );
    }
}

fn ordinary_frames_with_and_without_observers_keep_same_id_worlds_independent_case() {
    let (mut a, id_a, mut shadow_a) = fixture("OwnedObserved", 100.0);
    let (mut b, id_b, _) = fixture("OwnedUnobserved", 240.0);
    assert_eq!(id_a, id_b);
    let flash_a = author_timers(&mut a, id_a);
    let flash_b = author_timers(&mut b, id_b);
    for _ in 0..3 {
        advance_owned(&mut a, Some(&mut shadow_a));
        advance_owned(&mut b, None);
        assert_timers(&a, id_a, flash_a);
        assert_timers(&b, id_b, flash_b);
        assert_eq!(a.host_authoritative_health(id_a), Some(100.0));
        assert_eq!(b.host_authoritative_health(id_b), Some(240.0));
    }
}

fn ordinary_boundary_admits_owned_damage_and_removes_dead_objects_before_next_frame_case() {
    let (mut logic, id, mut shadow) = fixture("OwnedFrameDamage", 100.0);
    let entity = shadow.entity_for_host(id).unwrap();
    let (object, events) = logic.host_object_and_health_events_mut(id).unwrap();
    assert!(!object.take_damage_from(40.0, None, events));
    assert_eq!(logic.host_authoritative_health(id), Some(60.0));
    shadow
        .world_mut()
        .world_mut()
        .entity_mut(entity)
        .unwrap()
        .health = 999.0;
    advance_owned(&mut logic, Some(&mut shadow));
    assert_eq!(logic.host_authoritative_health(id), Some(60.0));
    assert_eq!(shadow.world().entity(entity).unwrap().health, 60.0);

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
    advance_owned(&mut logic, Some(&mut shadow));
    assert!(
        logic.host_object(id).is_none(),
        "owner destroy queue must complete"
    );
    assert!(
        shadow.entity_for_host(id).is_none(),
        "observer copies actual removal"
    );
    advance_owned(&mut logic, Some(&mut shadow));
    assert!(
        logic.host_object(id).is_none(),
        "a stale mirror cannot revive the ID"
    );
}

fn ordinary_boundary_preserves_actual_move_commands_for_presentation_case() {
    use crate::presentation_frame::{PresentationEvent, PresentationFrame};
    let (mut logic, id, mut shadow) = fixture("OwnedFrameMove", 100.0);
    let (mut other, other_id, mut other_shadow) = fixture("OtherFrameMove", 100.0);
    assert_eq!(id, other_id, "numeric object IDs deliberately collide");
    let session = logic.movement_events.session().clone();
    let other_session = other.movement_events.session().clone();
    assert_ne!(session, other_session);

    let first = Vec3::new(23.0, 0.0, 24.0);
    assert!(logic.append_unit_waypoint(id, first));
    advance_owned(&mut logic, Some(&mut shadow));
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
    advance_owned(&mut other, Some(&mut other_shadow));
    assert_eq!(logic.movement_events.snapshot_pending(), pending);
    advance_owned(&mut logic, Some(&mut shadow));
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

    let foreign_frame = PresentationFrame::build_from_logic(&mut other, 0);
    assert!(foreign_frame.events.iter().any(|event| matches!(event,
        PresentationEvent::MoveOrdered { session, destination, .. }
            if *session == other_session && *destination == foreign.to_array()
    )));
    assert_eq!(logic.movement_events.snapshot_completed(), completed);
    let frame = PresentationFrame::build_from_logic(&mut logic, 0);
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
    let next = PresentationFrame::build_from_logic(&mut logic, 0);
    assert!(
        !next
            .events
            .iter()
            .any(|event| matches!(event, PresentationEvent::MoveOrdered { .. })),
        "completed movement is consumed exactly once"
    );
}

fn ordinary_boundary_keeps_actual_construction_completion_pending_for_presentation_case() {
    let (mut logic, _, mut shadow) = fixture("OwnedFrameBuilder", 100.0);
    // C++ Energy returns zero production for an unpowered player even when
    // consumption is zero. Supply a real plant for this full-frame witness.
    let mut power = ThingTemplate::new("OwnedFramePower");
    power.add_kind_of(KindOf::Structure);
    power.energy_production = Some(10);
    logic.templates.insert(power.name.clone(), power);
    logic
        .create_object("OwnedFramePower", Team::USA, Vec3::new(40.0, 0.0, 40.0))
        .expect("actual power admission");
    let mut template = ThingTemplate::new("OwnedFrameBuilding");
    template.add_kind_of(KindOf::Structure).set_health(500.0);
    template.build_time = 1.0;
    logic
        .templates
        .insert("OwnedFrameBuilding".into(), template);
    let id = logic
        .create_object("OwnedFrameBuilding", Team::USA, Vec3::new(80.0, 0.0, 80.0))
        .unwrap();
    // Admit a real exclusive builder at the ACTION dock. C++ DozerAIUpdate
    // advances construction only from this task; no ghost builder is permitted.
    let mut dozer_template = ThingTemplate::new("OwnedFrameDozer");
    dozer_template
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Dozer)
        .set_health(200.0);
    logic
        .templates
        .insert("OwnedFrameDozer".into(), dozer_template);
    let dozer = logic
        .create_object("OwnedFrameDozer", Team::USA, Vec3::new(80.0, 0.0, 80.0))
        .expect("actual docked builder");
    let object = logic.host_object_mut(id).unwrap();
    object.set_status_under_construction(true);
    object.construction_percent = 0.99;
    object.builder_id = Some(dozer);
    let (builder, attack_events) = logic.host_object_and_attack_events_mut(dozer).unwrap();
    builder.set_target(
        Some(id),
        attack_events,
        &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
    );
    builder.set_ai_state(
        AIState::Constructing,
        &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
    );
    builder.dozer_task_build_target = Some(id);
    builder.status.moving = false;
    builder.dozer_dock_action = Some(builder.get_position());
    shadow.sync_from_host(&logic);
    // Sleepy UpdateModules cannot wake before frame 1. Frame 0 preserves the
    // scaffold; the next actual frame performs completion and its observation.
    advance_owned(&mut logic, Some(&mut shadow));
    assert!(logic.host_object(id).unwrap().status.under_construction);
    assert!(!logic.construction_events.has_pending(id));
    advance_owned(&mut logic, Some(&mut shadow));
    assert!(!logic.host_object(id).unwrap().status.under_construction);
    assert!(logic.construction_events.has_pending(id));

    let frame = crate::presentation_frame::PresentationFrame::build_from_logic(&mut logic, 0);
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
    let (mut logic, _, mut shadow) = fixture("OwnedDisableClock", 100.0);
    let mut factory = ThingTemplate::new("OwnedDisableBarracks");
    factory
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBarracks)
        .set_health(1000.0);
    factory.energy_production = Some(0);
    logic
        .templates
        .insert("OwnedDisableBarracks".into(), factory);
    let mut unit = ThingTemplate::new("OwnedDisableInfantry");
    unit.add_kind_of(KindOf::Infantry)
        .set_health(100.0)
        .set_cost(0, 0);
    unit.build_time = 10.0;
    logic.templates.insert("OwnedDisableInfantry".into(), unit);
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
        &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
    );
    object.apply_disabled_hacked(
        1,
        health_events,
        &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
    );
    object.apply_disabled_paralyzed(
        1,
        &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
    );
    shadow.sync_from_host(&logic);

    for next_frame in 1..=3 {
        advance_owned(&mut logic, Some(&mut shadow));
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
    use crate::subsystem_manager::AudioManagerSubsystem;
    let mut audio_manager = crate::subsystem_manager::SubsystemManager::new();
    AudioManagerSubsystem::install_test_queue(&mut audio_manager);
    for disable_generator in [false, true] {
        let (mut logic, _, mut shadow) = fixture("OwnedExpiryEffects", 100.0);
        let owner = logic.get_player_by_team(Team::USA).unwrap().id;
        let other = logic
            .get_player(1)
            .expect("actual foreign skirmish slot")
            .id;
        let mut cc = ThingTemplate::new("AmericaCommandCenter");
        cc.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::CommandCenter)
            .set_health(1000.0);
        cc.energy_production = Some(-3);
        logic.templates.insert("AmericaCommandCenter".into(), cc);
        let cc = logic
            .create_object_for_player("AmericaCommandCenter", owner, Vec3::new(90.0, 0.0, 90.0))
            .unwrap();
        let mut plant = ThingTemplate::new("OwnedExpiryPowerPlant");
        plant.add_kind_of(KindOf::Structure).set_health(1000.0);
        plant.energy_production = Some(10);
        logic
            .templates
            .insert("OwnedExpiryPowerPlant".into(), plant);
        let plant = logic
            .create_object_for_player("OwnedExpiryPowerPlant", owner, Vec3::new(110.0, 0.0, 90.0))
            .unwrap();
        logic
            .create_object_for_player("OwnedExpiryPowerPlant", other, Vec3::new(190.0, 0.0, 190.0))
            .unwrap();
        let mut consumer = ThingTemplate::new("OwnedExpiryPoweredConsumer");
        consumer
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Powered)
            .set_health(1000.0);
        consumer.energy_production = Some(-5);
        logic
            .templates
            .insert("OwnedExpiryPoweredConsumer".into(), consumer);
        let consumer = logic
            .create_object_for_player(
                "OwnedExpiryPoweredConsumer",
                owner,
                Vec3::new(130.0, 0.0, 90.0),
            )
            .unwrap();
        shadow.sync_from_host(&logic);
        advance_owned_with_test_audio(&mut logic, Some(&mut shadow), &mut audio_manager);
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
                &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
            );
        };
        advance_owned_with_test_audio(&mut logic, Some(&mut shadow), &mut audio_manager);
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
        for sound in [
            crate::game_logic::host_radar::RADAR_OFFLINE_AUDIO,
            crate::game_logic::host_radar::RADAR_ONLINE_AUDIO,
        ] {
            assert_eq!(
                delivered_audio
                    .iter()
                    .filter(|event| event.event_type == sound)
                    .count(),
                1,
                "exactly one expiry-frame radar sound {sound}"
            );
        }
        advance_owned_with_test_audio(&mut logic, Some(&mut shadow), &mut audio_manager);
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
                    event.event_type != crate::game_logic::host_radar::RADAR_OFFLINE_AUDIO
                        && event.event_type != crate::game_logic::host_radar::RADAR_ONLINE_AUDIO
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
        let (mut logic, _, _) = fixture("OwnedRadarCallbackCanary", 100.0);
        let mut cc = ThingTemplate::new("AmericaCommandCenter");
        cc.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::CommandCenter)
            .set_health(1000.0);
        cc.energy_production = Some(-3);
        logic.templates.insert("AmericaCommandCenter".into(), cc);
        let id = logic
            .create_object(
                "AmericaCommandCenter",
                Team::USA,
                Vec3::new(90.0, 0.0, 90.0),
            )
            .unwrap();
        let mut plant = ThingTemplate::new("OwnedRadarCallbackPower");
        plant.add_kind_of(KindOf::Structure).set_health(1000.0);
        plant.energy_production = Some(10);
        logic
            .templates
            .insert("OwnedRadarCallbackPower".into(), plant);
        logic
            .create_object(
                "OwnedRadarCallbackPower",
                Team::USA,
                Vec3::new(110.0, 0.0, 90.0),
            )
            .unwrap();
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        advance_owned(&mut logic, Some(&mut shadow));
        (logic, id, shadow)
    };
    let (mut a, id_a, mut observer_a) = make();
    let (mut b, id_b, mut observer_b) = make();
    assert_eq!(id_a, id_b);
    let owner = a.get_player_by_team(Team::USA).unwrap().id;
    {
        let (object, health_events) = a.host_object_and_health_events_mut(id_a).unwrap();
        object.apply_disabled_emp(
            1,
            health_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
    };
    {
        let (object, health_events) = b.host_object_and_health_events_mut(id_b).unwrap();
        object.apply_disabled_emp(
            10,
            health_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
    };
    assert_eq!(
        b.host_object(id_b)
            .unwrap()
            .pending_radar_disabled_edges
            .len(),
        1
    );
    advance_owned(&mut a, Some(&mut observer_a));
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
    advance_owned(&mut b, Some(&mut observer_b));
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
fn ordinary_shadow_frames_preserve_owned_timer_deadlines_and_save_continuation() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "ordinary_shadow_frames_preserve_owned_timer_deadlines_and_save_continuation",
        ordinary_shadow_frames_preserve_owned_timer_deadlines_and_save_continuation_case,
    );
}

#[test]
fn ordinary_frames_with_and_without_observers_keep_same_id_worlds_independent() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "ordinary_frames_with_and_without_observers_keep_same_id_worlds_independent",
        ordinary_frames_with_and_without_observers_keep_same_id_worlds_independent_case,
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
