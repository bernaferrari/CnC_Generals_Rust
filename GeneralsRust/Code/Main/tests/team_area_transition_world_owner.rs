use game_engine::common::dict::Dict;
use game_engine::common::ini::ini_locomotor::LocomotorStore;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::DataChunkOutput;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::ScriptEngine;
use generals_main::game_logic::{GameLogic, GameMode, Player, Team};
use std::path::Path;

fn map_file(root: &Path, destinations: &[[f32; 3]]) -> std::path::PathBuf {
    let mut out = DataChunkOutput::new();
    out.open_data_chunk("HeightMapData", 3);
    for value in [41, 41, 1, 1681] {
        out.write_int(value);
    }
    for _ in 0..1681 {
        out.write_byte(0);
    }
    out.close_data_chunk();

    out.open_data_chunk("PolygonTriggers", 4);
    out.write_int(1);
    out.write_ascii_string("OwnerTransitionArea");
    out.write_ascii_string("OwnerLayer");
    out.write_int(1901);
    out.write_byte(1);
    out.write_byte(0);
    out.write_int(0);
    out.write_int(4);
    for (x, y) in [(10, 10), (50, 10), (50, 50), (10, 50)] {
        out.write_int(x);
        out.write_int(y);
        out.write_int(0);
    }
    out.close_data_chunk();

    out.open_data_chunk("ObjectsList", 3);
    for (index, destination) in destinations.iter().enumerate() {
        out.open_data_chunk("Object", 3);
        for value in [destination[0], destination[2], destination[1], 0.0] {
            out.write_real(value);
        }
        out.write_int(0);
        out.write_ascii_string("Waypoint");
        let mut properties = Dict::new();
        properties.set_int(
            NameKeyGenerator::name_to_key("waypointID"),
            701 + index as i32,
        );
        properties.set_ascii_string(
            NameKeyGenerator::name_to_key("waypointName"),
            if index == 0 { "EnterMove" } else { "ExitMove" },
        );
        out.write_dict(&properties);
        out.close_data_chunk();
    }
    out.close_data_chunk();

    let path = root.join("TeamAreaTransition.map");
    std::fs::write(&path, out.into_ckmp_bytes()).unwrap();
    path
}

fn world(map: &Path, position: [f32; 3]) -> (GameLogic, generals_main::game_logic::ObjectId) {
    let mut world = GameLogic::new();
    let mut locomotors = LocomotorStore::new_owned();
    locomotors
        .apply_source("Locomotor TeamEdgeLoco\n Surfaces = GROUND\n Speed = 60\n Acceleration = 60\n TurnRate = 120\n Appearance = TWO_LEGS\nEnd\n")
        .unwrap();
    world.admit_locomotor_definitions(locomotors).unwrap();
    world
        .admit_object_ini_sources([(
            "TeamAreaTransition.ini".into(),
            "Object OwnedTeamEdgeUnit\n KindOf = SELECTABLE INFANTRY\n Locomotor = SET_NORMAL TeamEdgeLoco\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface Brain\n End\nEnd\n".into(),
        )])
        .unwrap();
    world.start_new_game(GameMode::SinglePlayer);
    assert!(world.load_map(map.to_str().unwrap()));
    world.add_player(Player::new(1, Team::USA, "Owner", true).with_script_identity("Owner"));
    let unit = world
        .create_object_for_player(
            "OwnedTeamEdgeUnit",
            1,
            glam::Vec3::new(position[0], position[1], position[2]),
        )
        .unwrap();
    (world, unit)
}

fn transition_condition(kind: ConditionType, team: &str, area: &str) -> Condition {
    let mut condition = Condition::new(kind);
    condition
        .add_parameter(Parameter::with_string(ParameterType::Team, team.into()))
        .unwrap();
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::TriggerArea,
            area.into(),
        ))
        .unwrap();
    condition
        .add_parameter(Parameter::with_int(ParameterType::SurfacesAllowed, 1))
        .unwrap();
    condition
}

fn counter_action(counter: &str) -> ScriptAction {
    let mut action = ScriptAction::new(ScriptActionType::IncrementCounter);
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, 1))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Counter,
            counter.into(),
        ))
        .unwrap();
    action
}

fn engine(name: &str, cases: Vec<(Condition, String)>) -> ScriptEngine {
    let mut scripts = ScriptList::new();
    let mut counters = Vec::new();
    for (index, (condition, counter)) in cases.into_iter().enumerate() {
        let mut branch = OrCondition::new();
        branch.set_first_and_condition(Some(Box::new(condition)));
        let mut script = Script::new();
        script.script_name = format!("{name}_{index}").into();
        script.is_one_shot = true;
        script.delay_evaluation_seconds = 0;
        script.condition = Some(Box::new(branch));
        script.action = Some(Box::new(counter_action(&counter)));
        scripts.append_script(Box::new(script));
        counters.push(counter);
    }
    let engine = ScriptEngine::new().unwrap();
    for counter in counters {
        engine.set_counter(&counter, 0).unwrap();
    }
    engine
        .set_script_list_for_player(0, Some(Box::new(scripts)))
        .unwrap();
    engine
}

fn observe(world: &mut GameLogic, name: &str, cases: Vec<(Condition, String)>) -> Vec<i32> {
    let counters: Vec<String> = cases.iter().map(|(_, counter)| counter.clone()).collect();
    let engine = engine(name, cases);
    world
        .update_local_script_engine_with_owner(&engine)
        .unwrap();
    counters
        .iter()
        .map(|counter| engine.get_counter(counter).unwrap().value)
        .collect()
}

#[test]
fn team_area_transition_conditions_use_receiving_team_flags_and_slots() {
    let owner_root = tempfile::tempdir().unwrap();
    let peer_root = tempfile::tempdir().unwrap();
    let owner_map = map_file(
        owner_root.path(),
        &[[180.0, 0.0, 210.0], [260.0, 0.0, 285.0]],
    );
    let peer_map = map_file(
        peer_root.path(),
        &[[360.0, 0.0, 390.0], [460.0, 0.0, 490.0]],
    );
    let (mut owner, owner_unit) = world(&owner_map, [20.0, 0.0, 20.0]);
    let (mut peer, peer_unit) = world(&peer_map, [80.0, 0.0, 80.0]);
    assert_eq!(owner_unit, peer_unit);

    // The first host projection compares the new object's position with the
    // origin and records this world team's real trigger transition.
    owner.inject_host_script_query_snapshot();
    peer.inject_host_script_query_snapshot();
    let entered_cases = || {
        vec![
            (
                transition_condition(
                    ConditionType::TeamEnteredAreaPartially,
                    "teamOwner",
                    "OwnerTransitionArea",
                ),
                "partial_enter".into(),
            ),
            (
                transition_condition(
                    ConditionType::TeamEnteredAreaEntirely,
                    "teamOwner",
                    "OwnerTransitionArea",
                ),
                "all_enter".into(),
            ),
        ]
    };
    assert_eq!(observe(&mut owner, "OwnerEntered", entered_cases()), [1, 1]);
    assert_eq!(observe(&mut peer, "PeerEntered", entered_cases()), [0, 0]);

    // Team events are reset at the end of the script update, just as C++
    // Team::updateState clears m_enteredOrExited.
    assert_eq!(
        observe(&mut owner, "ClearedEntered", entered_cases()),
        [0, 0]
    );

    let zero_root = tempfile::tempdir().unwrap();
    let zero_map = map_file(
        zero_root.path(),
        &[[180.0, 0.0, 210.0], [260.0, 0.0, 285.0]],
    );
    let (mut zero, zero_unit) = world(&zero_map, [20.0, 0.0, 20.0]);
    zero.host_object_mut(zero_unit).unwrap().locomotor_surfaces = 0;
    zero.inject_host_script_query_snapshot();
    assert_eq!(
        observe(
            &mut zero,
            "ZeroSurfaceAndMissing",
            vec![
                (
                    transition_condition(
                        ConditionType::TeamEnteredAreaPartially,
                        "teamOwner",
                        "OwnerTransitionArea",
                    ),
                    "zero_surface".into(),
                ),
                (
                    transition_condition(
                        ConditionType::TeamEnteredAreaPartially,
                        "MissingTeam",
                        "OwnerTransitionArea",
                    ),
                    "missing_team".into(),
                ),
                (
                    transition_condition(
                        ConditionType::TeamEnteredAreaPartially,
                        "teamOwner",
                        "MissingTransitionArea",
                    ),
                    "missing_area".into(),
                ),
            ],
        ),
        [0, 0, 0],
        "AI zero surfaces, missing teams and missing areas are false"
    );

    // A separate world tests exit ordering with a real owner movement API;
    // direct Object::set_position would bypass its registered partition owner.
    let exit_root = tempfile::tempdir().unwrap();
    let exit_map = map_file(
        exit_root.path(),
        &[[180.0, 0.0, 210.0], [260.0, 0.0, 285.0]],
    );
    let (mut exiting, exiting_unit) = world(&exit_map, [20.0, 0.0, 20.0]);
    exiting.inject_host_script_query_snapshot();
    let exit_cases = || {
        vec![
            (
                transition_condition(
                    ConditionType::TeamExitedAreaEntirely,
                    "teamOwner",
                    "OwnerTransitionArea",
                ),
                "all_exit".into(),
            ),
            (
                transition_condition(
                    ConditionType::TeamExitedAreaPartially,
                    "teamOwner",
                    "OwnerTransitionArea",
                ),
                "partial_exit".into(),
            ),
        ]
    };
    assert_eq!(observe(&mut exiting, "NoExitYet", exit_cases()), [0, 0]);
    assert!(exiting.move_object_to(exiting_unit, glam::Vec3::new(80.0, 0.0, 80.0)));

    let mut observed_exit = false;
    for frame in 0..240 {
        exiting.update();
        exiting.inject_host_script_query_snapshot();
        let result = observe(&mut exiting, &format!("ExitFrame{frame}"), exit_cases());
        if result == [1, 1] {
            observed_exit = true;
            break;
        }
    }
    assert!(
        observed_exit,
        "the receiving team's actual exit edge is observed"
    );
    assert_eq!(observe(&mut exiting, "ClearedExit", exit_cases()), [0, 0]);
}
