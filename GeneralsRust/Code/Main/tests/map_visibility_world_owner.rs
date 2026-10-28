use game_engine::common::system::DataChunkOutput;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::ScriptEngine;
use generals_main::game_logic::{GameLogic, GameMode, Player, Team};

fn minimal_map(root: &std::path::Path) -> std::path::PathBuf {
    let mut output = DataChunkOutput::new();
    output.open_data_chunk("HeightMapData", 3);
    for value in [41, 41, 1, 1681] {
        output.write_int(value);
    }
    for _ in 0..1681 {
        output.write_byte(0);
    }
    output.close_data_chunk();
    output.open_data_chunk("ObjectsList", 3);
    output.close_data_chunk();

    let path = root.join("MapVisibilityOwner.map");
    std::fs::write(&path, output.into_ckmp_bytes()).unwrap();
    path
}

fn world(map: &std::path::Path) -> GameLogic {
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([("MapVisibilityOwner.ini".into(), String::new())])
        .unwrap();
    world.start_new_game(GameMode::SinglePlayer);
    assert!(world.load_map(map.to_str().unwrap()));
    world.add_player(Player::new(1, Team::USA, "Local", true).with_script_identity("Local"));
    world
}

fn visibility_script(action_type: ScriptActionType, player: &str) -> ScriptEngine {
    let mut action = ScriptAction::new(action_type);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            player.to_owned(),
        ))
        .unwrap();

    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(
        ConditionType::ConditionTrue,
    ))));
    let mut script = Script::new();
    script.script_name = "MapVisibilityOwner".into();
    script.is_one_shot = true;
    script.delay_evaluation_seconds = 0;
    script.condition = Some(Box::new(condition));
    script.action = Some(Box::new(action));

    let mut scripts = ScriptList::new();
    scripts.append_script(Box::new(script));
    let engine = ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(scripts)))
        .unwrap();
    engine
}

fn run_visibility_action(
    world: &mut GameLogic,
    action_type: ScriptActionType,
    player: &str,
) {
    world
        .update_local_script_engine_with_owner(&visibility_script(action_type, player))
        .unwrap();
}

#[test]
fn map_wide_visibility_script_dispatch_targets_only_its_receiving_world() {
    let owner_root = tempfile::tempdir().unwrap();
    let peer_root = tempfile::tempdir().unwrap();
    let owner_map = minimal_map(owner_root.path());
    let peer_map = minimal_map(peer_root.path());
    let mut owner = world(&owner_map);
    let mut peer = world(&peer_map);
    assert_eq!(owner.get_player(1).unwrap().id, peer.get_player(1).unwrap().id);

    let location = glam::Vec3::ZERO;
    assert!(!owner.is_build_location_shroud_clear(1, location));
    assert!(!peer.is_build_location_shroud_clear(1, location));

    // Each action enters ScriptEngine::update_with_driver and production
    // ScriptActionDispatcher, rather than calling the owner callback directly.
    run_visibility_action(&mut peer, ScriptActionType::MapRevealAllPerm, "<Local Player>");
    assert!(peer.is_build_location_shroud_clear(1, location));
    assert!(!owner.is_build_location_shroud_clear(1, location));

    run_visibility_action(&mut owner, ScriptActionType::MapRevealAllPerm, "<Local Player>");
    assert!(owner.is_build_location_shroud_clear(1, location));
    assert!(peer.is_build_location_shroud_clear(1, location));

    run_visibility_action(
        &mut owner,
        ScriptActionType::MapRevealAllUndoPerm,
        "<Local Player>",
    );
    assert!(!owner.is_build_location_shroud_clear(1, location));
    assert!(peer.is_build_location_shroud_clear(1, location));

    // C++ PartitionCell::addShrouder leaves a cell with an active permanent
    // looker visible. Here the owner's looker was just undone, so this shroud
    // resets the receiving owner while preserving the already-visible peer.
    run_visibility_action(&mut owner, ScriptActionType::MapShroudAll, "<Local Player>");
    assert!(!owner.is_build_location_shroud_clear(1, location));
    assert!(peer.is_build_location_shroud_clear(1, location));
}
