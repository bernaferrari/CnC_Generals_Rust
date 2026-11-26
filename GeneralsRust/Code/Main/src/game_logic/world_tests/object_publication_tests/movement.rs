//! Path, locomotor, and movement state.

use super::*;

#[test]
fn locomotor_advances_the_pose_that_is_published() {
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("MvAuth");
    crate::fixture_content::admit_fixture_objects(
        &mut logic,
        &[crate::fixture_content::FixtureObject::new("RangerMv")
            .kind_of("INFANTRY SELECTABLE")
            .health(100.0)
            .walker()],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let oid = logic
        .create_object("RangerMv", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("id");
    {
        let (o, mut movement_events) = logic.host_object_and_movement_events_mut(oid).expect("o");
        let binding = crate::game_logic::locomotor_bootstrap::fixture_locomotor_binding(
            "BasicHumanLocomotor",
        )
        .expect("authored locomotor");
        o.locomotor_set_names = vec!["BasicHumanLocomotor".into()];
        o.cur_locomotor_name = Some("BasicHumanLocomotor".into());
        crate::game_logic::locomotor_bootstrap::apply_host_locomotor_binding(o, &binding);
        o.movement.max_speed = 60.0;
        o.movement.velocity = glam::Vec3::ZERO;
        o.move_to(
            glam::Vec3::new(100.0, 0.0, 0.0),
            0,
            &mut movement_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }
    let initial = publish(&mut logic);
    let before = actor(&initial, oid).position.x;
    for _ in 0..10 {
        logic.update_movement_for_test(&[oid], 1.0 / 30.0);
    }
    let frame = publish(&mut logic);
    let after = actor(&frame, oid).position.x;
    assert!(after > before + 0.1, "before={before} after={after}");
    assert_eq!(after, logic.host_object(oid).unwrap().get_position().x);
    assert_eq!(actor(&initial, oid).position.x, before);
}

#[test]
fn fixed_updates_and_publication_follow_the_same_moving_object() {
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("MvSkip");
    crate::fixture_content::admit_fixture_objects(
        &mut logic,
        &[
            crate::fixture_content::FixtureObject::new("RangerSk")
                .kind_of("INFANTRY")
                .health(100.0)
                .walker(),
            crate::fixture_content::FixtureObject::new("VictoryKeepAlive")
                .kind_of("STRUCTURE MP_COUNT_FOR_VICTORY")
                .health(100.0),
        ],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let oid = logic
        .create_object(
            "RangerSk",
            crate::game_logic::Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("id");
    {
        let (o, mut movement_events) = logic.host_object_and_movement_events_mut(oid).expect("o");
        o.movement.max_speed = 60.0;
        // The original locomotor ramps movement by authored acceleration.
        o.movement.acceleration = 240.0;
        o.move_to(
            glam::Vec3::new(50.0, 0.0, 0.0),
            0,
            &mut movement_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }
    // Keep the player in the skirmish while the movement fixture runs.
    let _keep_alive = logic
        .create_object(
            "VictoryKeepAlive",
            Team::USA,
            glam::Vec3::new(30.0, 0.0, 0.0),
        )
        .expect("keep-alive structure");
    let before = logic
        .host_objects()
        .lookup(&oid)
        .expect("o")
        .get_position()
        .x;
    let initial = publish(&mut logic);
    for _ in 0..10 {
        logic.update();
        logic.complete_presentation_observations();
        let frame = publish(&mut logic);
        assert_eq!(
            actor(&frame, oid).position,
            logic.host_object(oid).unwrap().get_position()
        );
    }
    let after = logic.host_object(oid).unwrap().get_position().x;
    assert!(after > before + 1.0, "before={before} after={after}");
    assert_eq!(actor(&initial, oid).position.x, before);
}

#[test]
fn waypoint_emits_its_final_destination() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("PathLog");
    crate::fixture_content::admit_fixture_objects(
        &mut logic,
        &[crate::fixture_content::FixtureObject::new("PathU")
            .kind_of("SELECTABLE")
            .health(100.0)],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if let Some(t) = logic.templates.get_mut("PathU") {
        t.add_kind_of(KindOf::Infantry);
    }
    let id = logic
        .create_object("PathU", Team::USA, glam::Vec3::ZERO)
        .expect("id");
    {
        let o = logic.host_object_mut(id).unwrap();
        o.movement.max_speed = 20.0;
    }
    let dest = glam::Vec3::new(40.0, 0.0, 10.0);
    assert!(
        logic.append_unit_waypoint(id, dest),
        "append waypoint should succeed for mobile unit"
    );
    let events = logic.movement_events.drain_for_host_boundary();
    assert!(
        events.iter().any(|e| {
            e.unit == id
                && e.destination
                    .map(|d| (d[0] - 40.0).abs() < 0.5 && (d[2] - 10.0).abs() < 0.5)
                    .unwrap_or(false)
        }),
        "append_unit_waypoint must log final dest: {events:?}"
    );
}

#[test]
fn set_destination_emits_the_mobile_object_receipt() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("MoveLog");
    crate::fixture_content::admit_fixture_objects(
        &mut logic,
        &[crate::fixture_content::FixtureObject::new("MoveLogU")
            .kind_of("SELECTABLE")
            .health(100.0)],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if let Some(tmpl) = logic.templates.get_mut("MoveLogU") {
        tmpl.add_kind_of(KindOf::Infantry);
    }
    let a = logic
        .create_object("MoveLogU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("a");
    assert!(
        logic.host_objects().lookup(&a).unwrap().is_mobile(),
        "template Infantry should make object mobile"
    );
    let (object, mut events) = logic.host_object_and_movement_events_mut(a).unwrap();
    object.set_destination(
        glam::Vec3::new(10.0, 0.0, 0.0),
        0,
        &mut events,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    let ev = logic.movement_events.drain_for_host_boundary();
    assert_eq!(ev.len(), 1);
    assert_eq!(ev[0].unit, a);
    assert_eq!(ev[0].destination, Some([10.0, 0.0, 0.0]));
}

#[test]
fn fixed_step_advances_an_authored_locomotor_path() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("MoveBridge");
    crate::fixture_content::admit_fixture_objects(
        &mut logic,
        &[
            crate::fixture_content::FixtureObject::new("VictoryKeepAlive")
                .kind_of("STRUCTURE MP_COUNT_FOR_VICTORY")
                .health(100.0),
            // C++ moves only through m_curLocomotor (AIUpdate.cpp doLocomotor).
            crate::fixture_content::FixtureObject::new("MoveBrU")
                .kind_of("INFANTRY SELECTABLE")
                .health(100.0)
                .walker(),
        ],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    // update_with_dt evaluates the skirmish NO_BUILDINGS victory rule
    // (C++ VictoryConditions.cpp:87-95/168-196): a structure-less playable
    // player is defeated on frame 0-1 and kill_player_for_victory destroys
    // its army — the unit under test disappears from host_objects. Seed the
    // retail-style MpCountForVictory keep-alive (sell_heal.rs precedent).
    let _keep_alive = logic
        .create_object(
            "VictoryKeepAlive",
            Team::USA,
            glam::Vec3::new(300.0, 0.0, 300.0),
        )
        .expect("keep-alive structure");
    let id = logic
        .create_object("MoveBrU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("id");
    {
        let o = logic.host_object_mut(id).unwrap();
        o.movement.set_path_coords(vec![
            glam::Vec3::new(0.0, 0.0, 0.0),
            glam::Vec3::new(50.0, 0.0, 0.0),
        ]);
        o.movement.max_speed = 20.0;
        // C++ Locomotor::getMaxAcceleration: retail locomotors author
        // Acceleration (infantry ~220); the host integrate velocity-ramps by
        // acceleration*dt, so a 0-accel fixture never builds velocity.
        o.movement.acceleration = 240.0;
        o.movement.target_position = Some(glam::Vec3::new(50.0, 0.0, 0.0));
        o.status.moving = true;
        o.movement.max_speed = 20.0;
    }
    for _ in 0..10 {
        logic.update_with_dt(1.0 / 30.0);
    }
    let p = logic.host_objects().lookup(&id).unwrap().get_position();
    assert!(p.x > 0.05, "authored path must advance; pos={p:?}");
}

#[test]
fn path_assignment_leaves_a_published_moving_state() {
    use crate::game_logic::AIState;

    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("PathMv");
    crate::fixture_content::admit_fixture_objects(
        &mut logic,
        &[crate::fixture_content::FixtureObject::new("PmU")
            .kind_of("INFANTRY SELECTABLE")
            .health(100.0)
            .walker()],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let oid = logic
        .create_object("PmU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("id");
    if let Some(o) = logic.host_object_mut(oid) {
        // Ensure mobile residual (max_speed > 0).
        o.movement.max_speed = 20.0;
    }
    let ok = logic.assign_unit_path_for_test(oid, glam::Vec3::new(50.0, 0.0, 0.0), &[]);
    assert!(ok, "path assign should succeed");

    assert_eq!(
        logic.host_objects().lookup(&oid).unwrap().ai_state(),
        AIState::Moving,
        "host Moving immediate"
    );
    assert!(
        logic.host_objects().lookup(&oid).unwrap().status.moving
            || logic
                .host_objects()
                .lookup(&oid)
                .unwrap()
                .movement
                .target_position
                .is_some()
            || !logic
                .host_objects()
                .lookup(&oid)
                .unwrap()
                .movement
                .path
                .is_empty(),
        "movement residual still on host"
    );
    let frame = publish(&mut logic);
    let row = actor(&frame, oid);
    assert_eq!(row.ai_state_ordinal, AIState::Moving.to_ordinal());
}

#[test]
fn appending_a_waypoint_publishes_a_moving_state() {
    use crate::game_logic::AIState;

    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("WpAuth");
    crate::fixture_content::admit_fixture_objects(
        &mut logic,
        &[crate::fixture_content::FixtureObject::new("WpU")
            .kind_of("INFANTRY")
            .health(100.0)
            .walker()],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let oid = logic
        .create_object("WpU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("id");
    if let Some(o) = logic.host_object_mut(oid) {
        o.movement.max_speed = 20.0;
    }
    assert!(logic.append_unit_waypoint_for_test(oid, glam::Vec3::new(30.0, 0.0, 0.0)));

    assert_eq!(
        logic.host_objects().lookup(&oid).unwrap().ai_state(),
        AIState::Moving,
        "host Moving immediate"
    );
    let frame = publish(&mut logic);
    let row = actor(&frame, oid);
    assert_eq!(row.ai_state_ordinal, AIState::Moving.to_ordinal());
}

#[test]
fn gathering_is_published_after_the_ai_order() {
    use crate::game_logic::AIState;

    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("StateAw");
    logic
        .admit_object_ini_sources([("fixture-objects.ini".into(), "Object SaU\nEnd\n".into())])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("SaU");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("SaU".into(), t);
    }
    let oid = logic
        .create_object("SaU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("id");
    logic.set_ai_state_decision_aware_for_test(oid, AIState::Gathering);

    // C++ AIUpdate applies this state synchronously to the receiving owner.
    assert_eq!(
        logic.host_objects().lookup(&oid).unwrap().ai_state(),
        AIState::Gathering
    );
    let frame = publish(&mut logic);
    let row = actor(&frame, oid);
    assert_eq!(row.ai_state_ordinal, AIState::Gathering.to_ordinal());
}

#[test]
fn repairing_is_published_for_the_receiving_worker() {
    use crate::game_logic::AIState;

    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("DzAuth");
    logic
        .admit_object_ini_sources([("fixture-objects.ini".into(), "Object DzU\nEnd\n".into())])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("DzU");
        t.add_kind_of(KindOf::Vehicle);
        t.add_kind_of(KindOf::Worker);
        logic.templates.insert("DzU".into(), t);
    }
    let oid = logic
        .create_object("DzU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("id");
    logic.set_ai_state_decision_aware_for_test(oid, AIState::Repairing);

    assert_eq!(
        logic.host_objects().lookup(&oid).unwrap().ai_state(),
        AIState::Repairing,
        "host AI state applies immediately"
    );
    let frame = publish(&mut logic);
    let row = actor(&frame, oid);
    assert_eq!(row.ai_state_ordinal, AIState::Repairing.to_ordinal());
}

#[test]
fn front_crush_publishes_the_front_condition_without_back_crush() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("CrushFl");
    logic
        .admit_object_ini_sources([("fixture-objects.ini".into(), "Object CrushMe\nEnd\n".into())])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("CrushMe");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("CrushMe".into(), t);
    }
    let oid = logic
        .create_object("CrushMe", Team::USA, glam::Vec3::new(4.0, 0.0, 4.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.front_crushed = true;
        o.back_crushed = false;
        o.crusher_level = 1;
        o.crushable_level = 1;
    }
    let frame = publish(&mut logic);
    let row = actor(&frame, oid);

    assert!(row.front_crushed && !row.back_crushed);
    assert!(logic.host_object(oid).unwrap().front_crushed);
}

#[test]
fn publication_preserves_the_waiting_path_request() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("WaitPath");
    logic
        .admit_object_ini_sources([(
            "fixture-objects.ini".into(),
            "Object WaitUnit\nEnd\n".into(),
        )])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("WaitUnit");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("WaitUnit".into(), t);
    }
    let oid = logic
        .create_object("WaitUnit", Team::USA, glam::Vec3::new(6.0, 0.0, 6.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.waiting_for_path = true;
        o.movement.max_speed = 12.0;
    }
    let frame = publish(&mut logic);
    let row = actor(&frame, oid);
    assert_eq!(row.move_max_speed, 12.0);
    let o = logic.host_object(oid).unwrap();
    assert!(o.waiting_for_path);
}

#[test]
fn publication_preserves_locomotor_request_flags_and_frame_stamps() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("LocoPath");
    logic
        .admit_object_ini_sources([("fixture-objects.ini".into(), "Object LocoU\nEnd\n".into())])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("LocoU");
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("LocoU".into(), t);
    }
    let oid = logic
        .create_object("LocoU", Team::USA, glam::Vec3::new(9.0, 0.0, 9.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.locomotor_surfaces = 0b101; // ground|cliff
        o.is_attack_path = true;
        o.is_braking = true;
        o.is_blocked_and_stuck = false;
        o.is_safe_path = true;
        o.queue_for_path_frame = 3;
        o.path_timestamp = 42;
        o.waiting_for_path = true;
        o.movement.max_speed = 15.0;
    }
    let frame = publish(&mut logic);
    let row = actor(&frame, oid);
    assert_eq!(row.move_max_speed, 15.0);
    let o = logic.host_object(oid).unwrap();

    assert_eq!(o.locomotor_surfaces, 0b101);
    assert!(o.is_attack_path && o.is_braking && o.is_safe_path && o.waiting_for_path);
    assert_eq!((o.queue_for_path_frame, o.path_timestamp), (3, 42));
}

#[test]
fn publication_preserves_blocked_path_state_and_exact_peer_ids() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("BlockP");
    logic
        .admit_object_ini_sources([("fixture-objects.ini".into(), "Object BlockU\nEnd\n".into())])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("BlockU");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("BlockU".into(), t);
    }
    let oid = logic
        .create_object("BlockU", Team::USA, glam::Vec3::new(12.0, 0.0, 12.0))
        .expect("id");
    let other = logic
        .create_object("BlockU", Team::USA, glam::Vec3::new(14.0, 0.0, 12.0))
        .expect("other");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.cur_max_blocked_speed = 3.5;
        o.num_frames_blocked = 7;
        o.is_blocked = true;
        o.move_away_from = Some(other);
        o.requested_victim_id = Some(other);
        o.movement.max_speed = 10.0;
    }
    let frame = publish(&mut logic);
    let row = actor(&frame, oid);
    assert_eq!(row.move_max_speed, 10.0);
    let o = logic.host_object(oid).unwrap();

    assert_eq!((o.cur_max_blocked_speed, o.num_frames_blocked), (3.5, 7));
    assert!(o.is_blocked);
    assert_eq!(
        (o.move_away_from, o.requested_victim_id),
        (Some(other), Some(other))
    );
}

#[test]
fn published_path_retains_the_receiving_velocity_and_cursor() {
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("MvCh");
    logic
        .admit_object_ini_sources([("fixture-objects.ini".into(), "Object MvU\nEnd\n".into())])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("MvU");
        t.set_health(200.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("MvU".into(), t);
    }
    let oid = logic
        .create_object("MvU", Team::USA, glam::Vec3::new(26.0, 0.0, 26.0))
        .expect("id");

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.movement.velocity = Vec3::new(3.0, 0.0, 4.0);
        o.movement.max_speed = 12.5;
        o.movement
            .set_path_coords(vec![Vec3::new(1.0, 0.0, 1.0), Vec3::new(2.0, 0.0, 2.0)]);
        o.movement.path_cursor = o.movement.path.cursor_at_ordinal(1);
    }
    let frame = publish(&mut logic);
    let row = actor(&frame, oid);

    assert_eq!(row.velocity.x, 3.0);
    assert_eq!(row.move_max_speed, 12.5);
    assert_eq!(row.path_index, 1);
    assert_eq!(row.path_len, 2);
    assert_eq!(row.path_waypoints.len(), 2);
    let owner = logic.host_object(oid).unwrap();
    assert_eq!(
        owner
            .movement
            .path
            .cursor_ordinal(&owner.movement.path_cursor),
        1
    );
}

#[test]
fn attitude_is_isolated_across_publication_restore_and_peer_reset() {
    fn world() -> (GameLogic, crate::game_logic::ObjectId) {
        let mut logic = GameLogic::new();
        let mut template = ThingTemplate::new("AttitudeOwner");
        template.set_health(100.0);
        template.add_kind_of(KindOf::Selectable);
        logic.templates.insert("AttitudeOwner".into(), template);
        let id = logic
            .create_object("AttitudeOwner", Team::USA, glam::Vec3::new(15.0, 0.0, 15.0))
            .unwrap();
        (logic, id)
    }
    let (mut first, id) = world();
    first.host_object_mut(id).unwrap().set_ai_attitude_i8(2);
    let (mut second, peer_id) = world();
    assert_eq!(id, peer_id);
    assert_eq!(
        first
            .host_objects()
            .lookup(&id)
            .expect("admitted host Object")
            .ai_attitude,
        2,
        "construction must not publish another owner"
    );
    second
        .host_object_mut(peer_id)
        .unwrap()
        .set_ai_attitude_i8(-2);
    let original = publish(&mut first);
    let foreign = publish(&mut second);
    assert_eq!(actor(&original, id).ai_attitude, 2);
    assert_eq!(actor(&foreign, peer_id).ai_attitude, -2);
    first.host_object_mut(id).unwrap().set_ai_attitude_i8(1);
    assert_eq!(actor(&publish(&mut first), id).ai_attitude, 1);

    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&first).unwrap();
    let mut restored = GameLogic::new();
    restored.templates = first.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert_eq!(actor(&publish(&mut restored), id).ai_attitude, 1);
    restored.host_object_mut(id).unwrap().set_ai_attitude_i8(-2);
    second.reset();
    drop(second);
    assert_eq!(actor(&publish(&mut first), id).ai_attitude, 1);

    first.host_object_mut(id).unwrap().set_ai_attitude_i8(127);
    assert_eq!(first.host_object(id).unwrap().ai_attitude, 2);
    first
        .host_object_mut(id)
        .unwrap()
        .set_ai_attitude(crate::game_logic::host_strategy_center::HostAiAttitude::Passive);
    assert_eq!(first.host_object(id).unwrap().ai_attitude, -1);
    assert_eq!(actor(&original, id).ai_attitude, 2);
    assert_eq!(actor(&foreign, peer_id).ai_attitude, -2);
}

#[test]
fn publication_preserves_the_owners_mood_configuration() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("AiMood");
    logic
        .admit_object_ini_sources([("fixture-objects.ini".into(), "Object MoodU\nEnd\n".into())])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("MoodU");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("MoodU".into(), t);
    }
    let oid = logic
        .create_object("MoodU", Team::USA, glam::Vec3::new(40.0, 0.0, 40.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.idle_since_frame = 120;
        o.mood_attack_check_rate = 45;
        o.set_auto_acquire_when_idle(false);
        o.attack_priority_set = Some("Soldier".into());
    }
    let _frame = publish(&mut logic);
    let o = logic.host_object(oid).unwrap();

    assert_eq!(o.idle_since_frame, 120);
    assert_eq!(o.mood_attack_check_rate, 45);
    assert!(!o.auto_acquire_when_idle());
    assert_eq!(o.attack_priority_set.as_deref(), Some("Soldier"));
}

#[test]
fn publication_preserves_motive_force_and_collision_state() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("PhysMot");
    logic
        .admit_object_ini_sources([("fixture-objects.ini".into(), "Object PhysU\nEnd\n".into())])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("PhysU");
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("PhysU".into(), t);
    }
    let oid = logic
        .create_object("PhysU", Team::USA, glam::Vec3::new(70.0, 0.0, 70.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.motive_force_expires = 12;
        o.physics_mass = 2.5;
        o.physics_accel = glam::Vec3::new(1.0, 0.0, 0.5);
        o.forward_friction = 0.15;
        o.lateral_friction = 0.2;
        o.z_friction = 0.1;
        o.can_path_through_units = true;
        o.ignore_collisions_until_frame = 40;
        o.is_panicking = true;
        o.move_away_frames = 5;
        o.aerodynamic_friction = 0.05;
        o.extra_friction = 0.02;
        o.apply_friction_2d_when_airborne = true;
        o.center_of_mass_offset = -0.5;
        o.pitch_roll_yaw_factor = 1.2;
        o.immune_to_falling_damage = true;
    }
    let _frame = publish(&mut logic);
    let o = logic.host_object(oid).unwrap();

    assert_eq!(o.motive_force_expires, 12);
    assert!(o.is_motive(11));
    assert!(!o.is_motive(12));
    assert_eq!(o.physics_mass, 2.5);
    assert_eq!(o.physics_accel, Vec3::new(1.0, 0.0, 0.5));
    assert!(o.can_path_through_units && o.is_panicking);
    assert_eq!(o.ignore_collisions_until_frame, 40);
    assert_eq!(o.aerodynamic_friction, 0.05);
    assert!(o.immune_to_falling_damage);
}

#[test]
fn publication_preserves_landing_and_pending_bounce_audio() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("BounceL");
    logic
        .admit_object_ini_sources([("fixture-objects.ini".into(), "Object BounceU\nEnd\n".into())])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("BounceU");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("BounceU".into(), t);
    }
    let oid = logic
        .create_object("BounceU", Team::USA, glam::Vec3::new(80.0, 0.0, 80.0))
        .expect("id");
    let other = logic
        .create_object("BounceU", Team::USA, glam::Vec3::new(82.0, 0.0, 80.0))
        .expect("other");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.kill_when_resting_on_ground = true;
        o.bounce_land_events = 3;
        o.last_bounce_fall_dy = 12.0;
        o.bounce_sound_name = "Module:Bounce".into();
        o.last_bounce_volume = 0.75;
        o.bounce_audio_pending = 2;
        o.allow_collide_force = false;
        o.last_collidee = Some(other);
        o.ignore_collisions_with = Some(other);
    }
    let _frame = publish(&mut logic);
    let o = logic.host_object(oid).unwrap();

    assert!(o.kill_when_resting_on_ground);
    assert_eq!((o.bounce_land_events, o.bounce_audio_pending), (3, 2));
    assert_eq!(o.last_bounce_fall_dy, 12.0);
    assert_eq!(o.bounce_sound_name, "Module:Bounce");
    assert_eq!(o.last_bounce_volume, 0.75);
    assert!(!o.allow_collide_force);
    assert_eq!(o.last_collidee, Some(other));
}

#[test]
fn publication_preserves_locomotor_height_turning_and_path_state() {
    use crate::game_logic::{LocomotorAppearance, LocomotorBehaviorZ};

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("Loco");
    logic
        .admit_object_ini_sources([("fixture-objects.ini".into(), "Object LocoU\nEnd\n".into())])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("LocoU");
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("LocoU".into(), t);
    }
    let oid = logic
        .create_object("LocoU", Team::USA, glam::Vec3::new(150.0, 0.0, 150.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.is_approach_path = true;
        o.on_invalid_movement_terrain = true;
        o.was_airborne_last_frame = true;
        o.can_move_backward = true;
        o.moving_backwards = true;
        o.no_slow_down_as_approaching_dest = true;
        o.turn_pivot_offset = -0.5;
        o.wander_width_factor = 0.2;
        o.loco_apply_2d_friction_airborne = true;
        o.loco_extra_2d_friction = 0.03;
        o.loco_preferred_height = 40.0;
        o.loco_preferred_height_damping = 0.7;
        o.loco_appearance = LocomotorAppearance::Wings;
        o.loco_behavior_z = LocomotorBehaviorZ::AbsoluteHeight;
        o.min_turn_speed = 5.5;
        o.physics_turning = crate::game_logic::PhysicsTurningType::TurnPositive;
    }
    let frame = publish(&mut logic);
    let row = actor(&frame, oid);
    assert!(row.moving_backwards);
    let o = logic.host_object(oid).unwrap();

    assert!(o.is_approach_path && o.was_airborne_last_frame && o.moving_backwards);
    assert_eq!((o.turn_pivot_offset, o.loco_preferred_height), (-0.5, 40.0));
    assert_eq!(o.loco_appearance, LocomotorAppearance::Wings);
    assert_eq!(o.loco_behavior_z, LocomotorBehaviorZ::AbsoluteHeight);
    assert_eq!(o.min_turn_speed, 5.5);
    assert_eq!(
        o.physics_turning,
        crate::game_logic::PhysicsTurningType::TurnPositive
    );
}

#[test]
fn publication_preserves_target_requests_and_pending_actor_state() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("AiReq");
    logic
        .admit_object_ini_sources([("fixture-objects.ini".into(), "Object AiU\nEnd\n".into())])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("AiU");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("AiU".into(), t);
    }
    let oid = logic
        .create_object("AiU", Team::USA, glam::Vec3::new(170.0, 0.0, 170.0))
        .expect("id");
    let victim = logic
        .create_object("AiU", Team::China, glam::Vec3::new(200.0, 0.0, 170.0))
        .expect("v");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.requested_victim_id = Some(victim);
        o.requested_destination = Some(glam::Vec3::new(9.0, 0.0, 8.0));
        o.set_prev_victim_pos(Some(glam::Vec3::new(1.0, 2.0, 3.0)));
        o.crate_created = Some(ObjectId(99));
        o.set_guard_retaliate_victim(Some(victim));
        o.set_guard_retaliate_anchor(Some(glam::Vec3::new(4.0, 0.0, 5.0)));
        o.path_timestamp = 77;
        o.disguise_pending_template = Some("FakeTank".into());
        o.disguise_pending_team = Some(Team::GLA);
        o.set_weapon_condition_at_frame(5, true, 0);
        o.armor_crate_upgrade = 1;
        o.selection_flash_remaining = 15;
    }
    let frame = publish(&mut logic);
    let row = actor(&frame, oid);
    assert_eq!(row.selection_flash_remaining, 15);
    let o = logic.host_object(oid).unwrap();

    assert_eq!(o.requested_victim_id, Some(victim));
    assert_eq!(o.requested_destination, Some(Vec3::new(9.0, 0.0, 8.0)));
    assert_eq!(o.prev_victim_pos(), Some(Vec3::new(1.0, 2.0, 3.0)));
    assert_eq!(o.crate_created, Some(ObjectId(99)));
    assert_eq!(o.guard_retaliate_victim(), Some(victim));
    assert_eq!(o.path_timestamp, 77);
    assert_eq!(o.disguise_pending_template.as_deref(), Some("FakeTank"));
    assert_eq!(o.disguise_pending_team, Some(Team::GLA));
    assert_eq!(o.weapon_crate_upgrade(), 2);
}
