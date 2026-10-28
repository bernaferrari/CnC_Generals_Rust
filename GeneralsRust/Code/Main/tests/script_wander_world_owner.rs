use game_engine::common::ini::ini_locomotor::{
    LocomotorStore, get_locomotor_store, get_locomotor_store_mut,
};
use game_engine::common::system::DataChunkOutput;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::ScriptEngine;
use generals_main::game_logic::{GameLogic, GameMode, Player, Team};
use generals_main::save_load::SnapshotBuilder;

fn world(map: &std::path::Path, radius: u32) -> (GameLogic, generals_main::game_logic::ObjectId) {
    let spy_speed = if radius == 0 { 4 } else { 8 };
    let mut locomotors = LocomotorStore::new_owned();
    locomotors
        .apply_source(&format!(
            "Locomotor OwnedNormal\n Surfaces = GROUND\n Speed = 20\n Acceleration = 60\n TurnRate = 120\n Appearance = TWO_LEGS\nEnd\n\
             Locomotor OwnedWander\n Surfaces = GROUND\n Speed = 20\n Acceleration = 60\n TurnRate = 120\n Appearance = TWO_LEGS\n WanderAboutPointRadius = {radius}\nEnd\n\
             Locomotor SpyDroneLocomotor\n Surfaces = AIR\n Speed = {spy_speed}\n Acceleration = 60\n TurnRate = 120\n Appearance = HOVER\nEnd\n"
        ))
        .unwrap();
    let mut world = GameLogic::new();
    world.admit_locomotor_definitions(locomotors).unwrap();
    world
        .admit_object_ini_sources([(
            "Wander.ini".into(),
            "Object OwnedWanderer\n KindOf = SELECTABLE INFANTRY\n Locomotor = SET_NORMAL OwnedNormal\n Locomotor = SET_WANDER OwnedWander\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface Brain\n End\nEnd\n\
             Object AmericaVehicleSpyDrone\n KindOf = SELECTABLE VEHICLE\n Locomotor = SET_NORMAL SpyDroneLocomotor\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface Brain\n End\nEnd\n".into(),
        )])
        .unwrap();
    world.start_new_game(GameMode::SinglePlayer);
    assert!(world.load_map(map.to_str().unwrap()));
    world.add_player(Player::new(1, Team::USA, "Display", true).with_script_identity("Owner"));
    let id = world
        .create_object_for_player("OwnedWanderer", 1, glam::Vec3::new(100.0, 0.0, 100.0))
        .unwrap();
    (world, id)
}

fn wander_script() -> ScriptEngine {
    wander_script_with_followup_actions(0)
}

fn wander_script_with_followup_actions(count: usize) -> ScriptEngine {
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut action = ScriptAction::new(ScriptActionType::TeamWanderInPlace);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "teamOwner".into(),
        ))
        .unwrap();
    let mut script = Script::new();
    script.script_name = "OwnedWander".into();
    script.is_one_shot = true;
    script.delay_evaluation_seconds = 0;
    script.condition = Some(Box::new(condition));
    script.action = Some(Box::new(action));
    for index in 0..count {
        let mut action = ScriptAction::new(ScriptActionType::SetFlag);
        action
            .add_parameter(Parameter::with_string(
                ParameterType::Flag,
                format!("UnrelatedFlag{index}"),
            ))
            .unwrap();
        action
            .add_parameter(Parameter::with_int(ParameterType::Boolean, 1))
            .unwrap();
        script.append_action(Box::new(action));
    }
    let mut scripts = ScriptList::new();
    scripts.append_script(Box::new(script));
    let engine = ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(scripts)))
        .unwrap();
    engine
}

#[test]
fn unrelated_script_actions_do_not_advance_wander_before_the_simulation_step() {
    let mut map = DataChunkOutput::new();
    map.open_data_chunk("HeightMapData", 3);
    for value in [41, 41, 1, 1681] {
        map.write_int(value);
    }
    for _ in 0..1681 {
        map.write_byte(8);
    }
    map.close_data_chunk();
    map.open_data_chunk("ObjectsList", 3);
    map.close_data_chunk();
    let root = tempfile::tempdir().unwrap();
    let map_path = root.path().join("Wander.map");
    std::fs::write(&map_path, map.into_ckmp_bytes()).unwrap();

    let (mut control, control_id) = world(&map_path, 0);
    let (mut with_extra_actions, extra_id) = world(&map_path, 0);
    assert_eq!(control_id, extra_id);
    let snapshot = SnapshotBuilder::new();
    assert_eq!(
        snapshot
            .create_world_snapshot(&control)
            .unwrap()
            .logic_rng_seed_words,
        snapshot
            .create_world_snapshot(&with_extra_actions)
            .unwrap()
            .logic_rng_seed_words
    );
    control
        .update_local_script_engine_with_owner(&wander_script())
        .unwrap();
    with_extra_actions
        .update_local_script_engine_with_owner(&wander_script_with_followup_actions(5))
        .unwrap();
    let after_entry = snapshot
        .create_world_snapshot(&control)
        .unwrap()
        .logic_rng_seed_words;
    assert_eq!(
        snapshot
            .create_world_snapshot(&with_extra_actions)
            .unwrap()
            .logic_rng_seed_words,
        after_entry,
        "unrelated script actions must not tick an already-entered AI state"
    );

    // The live loop deliberately advances at most one logic frame per call.
    control.update_with_dt(1.0 / 30.0);
    control.update_with_dt(1.0 / 30.0);
    assert_ne!(
        snapshot
            .create_world_snapshot(&control)
            .unwrap()
            .logic_rng_seed_words,
        after_entry,
        "the receiving simulation must still advance the wander state"
    );
    assert_eq!(
        snapshot
            .create_world_snapshot(&with_extra_actions)
            .unwrap()
            .logic_rng_seed_words,
        after_entry,
        "advancing one world must not tick the other world's AI"
    );
    with_extra_actions.set_script_time_frozen_for_test(true);
    with_extra_actions.update_with_dt(1.0 / 30.0);
    with_extra_actions.update_with_dt(1.0 / 30.0);
    assert_eq!(
        snapshot
            .create_world_snapshot(&with_extra_actions)
            .unwrap()
            .logic_rng_seed_words,
        after_entry,
        "a frozen match must not run the periodic wander update"
    );
    with_extra_actions.set_script_time_frozen_for_test(false);
    with_extra_actions.update_with_dt(1.0 / 30.0);
    with_extra_actions.update_with_dt(1.0 / 30.0);
    assert_ne!(
        snapshot
            .create_world_snapshot(&with_extra_actions)
            .unwrap()
            .logic_rng_seed_words,
        after_entry,
        "wander must resume when the receiving match unfreezes"
    );
}

#[test]
fn scripts_wander_only_the_receiving_team_using_its_current_locomotor() {
    // A foreign definition has the same name but a very different radius.
    get_locomotor_store_mut()
        .apply_source("Locomotor OwnedWander\n Surfaces = GROUND\n Speed = 20\n WanderAboutPointRadius = 1000\nEnd\n")
        .unwrap();
    let global_names = get_locomotor_store()
        .get_template_names()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    let mut map = DataChunkOutput::new();
    map.open_data_chunk("HeightMapData", 3);
    for value in [41, 41, 1, 1681] {
        map.write_int(value);
    }
    for _ in 0..1681 {
        map.write_byte(8);
    }
    map.close_data_chunk();
    map.open_data_chunk("ObjectsList", 3);
    map.close_data_chunk();
    let root = tempfile::tempdir().unwrap();
    let map_path = root.path().join("Wander.map");
    std::fs::write(&map_path, map.into_ckmp_bytes()).unwrap();

    let (mut first, first_id) = world(&map_path, 0);
    let (mut peer, peer_id) = world(&map_path, 60);
    assert_eq!(first_id, peer_id);
    first.ensure_ai_faction_templates(Team::USA);
    assert_eq!(
        get_locomotor_store()
            .get_template_names()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        global_names,
        "starting owned sessions and preparing AI templates must not seed the process catalog"
    );
    let first_script = wander_script();
    let peer_script = wander_script();

    first
        .update_local_script_engine_with_owner(&first_script)
        .unwrap();
    let object = first.host_object(first_id).unwrap();
    assert_eq!(object.cur_locomotor_name.as_deref(), Some("OwnedWander"));
    assert!(
        object.requested_destination.is_none(),
        "zero radius stays at the origin"
    );
    assert_eq!(
        peer.host_object(peer_id)
            .unwrap()
            .cur_locomotor_name
            .as_deref(),
        Some("OwnedNormal"),
        "the first interpreter cannot command the other session"
    );

    peer.update_local_script_engine_with_owner(&peer_script)
        .unwrap();
    let peer_object = peer.host_object(peer_id).unwrap();
    assert_eq!(
        peer_object.cur_locomotor_name.as_deref(),
        Some("OwnedWander")
    );
    let destination = peer_object
        .requested_destination
        .expect("peer received a wander hop");
    for offset in [destination.x - 100.0, destination.z - 100.0] {
        assert!(offset.abs() <= 60.0, "hop must use the peer's own radius");
    }
    assert!(
        first
            .host_object(first_id)
            .unwrap()
            .requested_destination
            .is_none()
    );

    let snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&first)
        .unwrap();
    let (mut restored, _) = world(&map_path, 0);
    SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    restored
        .update_local_script_engine_with_owner(&wander_script())
        .unwrap();
    let object = restored.host_object(first_id).unwrap();
    assert_eq!(object.cur_locomotor_name.as_deref(), Some("OwnedWander"));
    assert!(object.requested_destination.is_none());

    for (world, expected_speed) in [(&mut first, 4.0), (&mut peer, 8.0)] {
        assert!(world.activate_spy_drone(1, Team::USA, glam::Vec3::new(150.0, 0.0, 150.0), None));
        let drone = world.spy_drones().last().unwrap().spawned_id.unwrap();
        assert!(
            (world.host_object(drone).unwrap().movement.max_speed - expected_speed).abs() < 0.001
        );
    }
    assert_eq!(
        get_locomotor_store()
            .get_template_names()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        global_names,
        "spy-drone creation must use only the receiving session's catalog"
    );
}
