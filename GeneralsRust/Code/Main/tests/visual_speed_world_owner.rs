use game_engine::common::system::DataChunkOutput;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::ScriptEngine;
use generals_main::game_logic::{GameLogic, GameMode, Player, Team};
use generals_main::save_load::SnapshotBuilder;

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
    let path = root.join("VisualSpeedOwner.map");
    std::fs::write(&path, output.into_ckmp_bytes()).unwrap();
    path
}

fn world(map: &std::path::Path) -> GameLogic {
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([("VisualSpeedOwner.ini".into(), String::new())])
        .unwrap();
    world.start_new_game(GameMode::SinglePlayer);
    assert!(world.load_map(map.to_str().unwrap()));
    world.add_player(Player::new(1, Team::USA, "Local", true).with_script_identity("Local"));
    world
}

fn speed_script(multiplier: i32) -> ScriptEngine {
    let mut action = ScriptAction::new(ScriptActionType::SetVisualSpeedMultiplier);
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, multiplier))
        .unwrap();
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "VisualSpeedOwner".into();
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

#[test]
fn authored_zero_negative_speed_updates_only_receiving_camera_visual_state() {
    let owner_root = tempfile::tempdir().unwrap();
    let peer_root = tempfile::tempdir().unwrap();
    let owner_map = minimal_map(owner_root.path());
    let peer_map = minimal_map(peer_root.path());
    let mut owner = world(&owner_map);
    let mut peer = world(&peer_map);
    assert_eq!(
        owner.get_player(1).unwrap().id,
        peer.get_player(1).unwrap().id
    );

    for multiplier in [4, 0, -2, 3] {
        let simulation_frame = owner.get_frame();
        owner
            .update_local_script_engine_with_owner(&speed_script(multiplier))
            .unwrap();
        assert_eq!(owner.visual_speed_multiplier(), multiplier as f32);
        assert_eq!(owner.get_frame(), simulation_frame);
        assert_eq!(peer.visual_speed_multiplier(), 1.0);
        let builder = SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&owner).unwrap();
        let mut restored = world(&owner_map);
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .unwrap();
        assert_eq!(restored.visual_speed_multiplier(), multiplier as f32);
        peer.reset();
        assert_eq!(owner.visual_speed_multiplier(), multiplier as f32);
    }
}
