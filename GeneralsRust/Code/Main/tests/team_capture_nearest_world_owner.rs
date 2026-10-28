use game_engine::common::{
    dict::Dict,
    system::DataChunkOutput,
    well_known_keys::{
        key_player_display_name, key_player_faction, key_player_is_human, key_player_name,
        key_team_is_singleton, key_team_name, key_team_owner,
    },
};
use gamelogic::scripting::{
    core::{
        Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
        ScriptActionType, ScriptList,
    },
    engine::ScriptEngine,
};
use generals_main::game_logic::{GameLogic, GameMode, Team};
use glam::Vec3;

fn map_file(root: &std::path::Path) -> std::path::PathBuf {
    let mut out = DataChunkOutput::new();
    out.open_data_chunk("HeightMapData", 3);
    for value in [41, 41, 1, 1681] {
        out.write_int(value);
    }
    for _ in 0..1681 {
        out.write_byte(0);
    }
    out.close_data_chunk();
    out.open_data_chunk("SidesList", 2);
    out.write_int(2);
    let mut neutral = Dict::new();
    neutral.set_ascii_string(key_player_name(), "");
    neutral.set_ascii_string(key_player_faction(), "FactionCivilian");
    let mut human = Dict::new();
    human.set_ascii_string(key_player_name(), "Owner");
    human.set_ascii_string(key_player_faction(), "FactionAmerica");
    human.set_unicode_string(key_player_display_name(), "Owner");
    human.set_bool(key_player_is_human(), true);
    for side in [neutral, human] {
        out.write_dict(&side);
        out.write_int(0);
    }
    out.write_int(1);
    let mut team = Dict::new();
    team.set_ascii_string(key_team_name(), "CaptureRoster");
    team.set_ascii_string(key_team_owner(), "Owner");
    team.set_bool(key_team_is_singleton(), false);
    out.write_dict(&team);
    out.close_data_chunk();
    out.open_data_chunk("ObjectsList", 3);
    out.close_data_chunk();
    let path = root.join("CaptureNearest.map");
    std::fs::write(&path, out.into_ckmp_bytes()).unwrap();
    path
}

fn world(map: &std::path::Path) -> GameLogic {
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([(
            "CaptureNearest.ini".into(),
            "Object CaptureRanger\n KindOf = SELECTABLE INFANTRY\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface Brain\n End\nEnd\nObject CaptureHusk\n KindOf = VEHICLE SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 300\n InitialHealth = 300\n End\n Behavior = AIUpdateInterface Brain\n End\nEnd\n".into(),
        ), (
            "CaptureStatic.ini".into(),
            "Object CaptureStatic\n KindOf = STRUCTURE SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 300\n InitialHealth = 300\n End\nEnd\n".into(),
        )])
        .unwrap();
    world.start_new_game(GameMode::SinglePlayer);
    assert!(world.load_map(map.to_str().unwrap()));
    world
}

fn add_roster_member(world: &mut GameLogic) -> generals_main::game_logic::ObjectId {
    let id = world
        .create_object_for_player("CaptureRanger", 1, Vec3::new(10.0, 0.0, 50.0))
        .unwrap();
    world.host_object_mut(id).unwrap().team_instance_name = "CaptureRoster".into();
    world.activate_leftover_team_for_host_object(id);
    id
}

fn add_husk(world: &mut GameLogic, at: Vec3) -> generals_main::game_logic::ObjectId {
    let id = world
        .create_object("CaptureHusk", Team::Neutral, at)
        .unwrap();
    let husk = world.host_object_mut(id).unwrap();
    husk.status.disabled_unmanned = true;
    husk.owner_player_id = None;
    husk.team_instance_name.clear();
    id
}

fn script_engine() -> ScriptEngine {
    let mut action = ScriptAction::new(ScriptActionType::TeamCaptureNearestUnownedFactionUnit);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "CaptureRoster".into(),
        ))
        .unwrap();
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "CaptureNearest".into();
    script.is_one_shot = true;
    script.delay_evaluation_seconds = 0;
    script.condition = Some(Box::new(condition));
    script.action = Some(Box::new(action));
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let engine = ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    engine
}

#[test]
fn authored_capture_action_is_synchronous_nearest_and_world_owned() {
    let root = tempfile::tempdir().unwrap();
    let map = map_file(root.path());
    let mut receiving = world(&map);
    let mut peer = world(&map);
    let receiving_unit = add_roster_member(&mut receiving);
    let peer_unit = add_roster_member(&mut peer);
    assert_eq!(receiving_unit, peer_unit);

    let receiving_near = add_husk(&mut receiving, Vec3::new(80.0, 0.0, 50.0));
    let peer_near = add_husk(&mut peer, Vec3::new(80.0, 0.0, 50.0));
    let receiving_far = add_husk(&mut receiving, Vec3::new(250.0, 0.0, 250.0));
    let peer_far = add_husk(&mut peer, Vec3::new(250.0, 0.0, 250.0));
    // This off-map husk is closer than the valid one; it must be excluded.
    let receiving_off_map = add_husk(&mut receiving, Vec3::new(-1.0, 0.0, 50.0));
    let peer_off_map = add_husk(&mut peer, Vec3::new(-1.0, 0.0, 50.0));
    assert_eq!(receiving_near, peer_near);
    assert_eq!(receiving_far, peer_far);
    assert_eq!(receiving_off_map, peer_off_map);

    receiving
        .update_local_script_engine_with_owner(&script_engine())
        .unwrap();
    assert_eq!(
        receiving.host_object(receiving_unit).unwrap().target,
        Some(receiving_near),
        "the authored action chooses the nearest on-map unmanned unit in the receiving world"
    );
    assert_eq!(
        receiving.host_object(receiving_unit).unwrap().ai_state(),
        generals_main::game_logic::AIState::Entering
    );
    assert_eq!(
        peer.host_object(peer_unit).unwrap().target,
        None,
        "the same-ID peer remains untouched"
    );

    for id in [peer_near, peer_far, peer_off_map] {
        peer.host_object_mut(id).unwrap().status.disabled_unmanned = false;
    }
    peer.update_local_script_engine_with_owner(&script_engine())
        .unwrap();
    assert_eq!(
        peer.host_object(peer_unit).unwrap().target,
        None,
        "an action with no eligible target does not issue groupEnter"
    );
}

#[test]
fn capture_uses_ai_member_center_and_rejects_allied_unmanned_units() {
    let root = tempfile::tempdir().unwrap();
    let map = map_file(root.path());
    let mut receiving = world(&map);
    let mut peer = world(&map);
    let ranger = add_roster_member(&mut receiving);
    let peer_ranger = add_roster_member(&mut peer);
    assert_eq!(ranger, peer_ranger);

    for world in [&mut receiving, &mut peer] {
        let structure = world
            .create_object_for_player("CaptureStatic", 1, Vec3::new(300.0, 0.0, 50.0))
            .unwrap();
        world.host_object_mut(structure).unwrap().team_instance_name = "CaptureRoster".into();
        world.activate_leftover_team_for_host_object(structure);
        world.add_player(
            generals_main::game_logic::Player::new(3, Team::USA, "Ally", false)
                .with_script_identity("Ally"),
        );
        world
            .get_player_mut(1)
            .unwrap()
            .set_map_relationship(3, gamelogic::common::Relationship::Allies);
        world
            .get_player_mut(1)
            .unwrap()
            .set_team_instance_player_override(
                "CaptureRoster",
                3,
                gamelogic::common::Relationship::Enemies,
            );
        let allied = world
            .create_object_for_player("CaptureHusk", 3, Vec3::new(11.0, 0.0, 50.0))
            .unwrap();
        world
            .host_object_mut(allied)
            .unwrap()
            .status
            .disabled_unmanned = true;
    }
    let receiving_near = add_husk(&mut receiving, Vec3::new(80.0, 0.0, 50.0));
    let peer_near = add_husk(&mut peer, Vec3::new(80.0, 0.0, 50.0));
    add_husk(&mut receiving, Vec3::new(160.0, 0.0, 50.0));
    add_husk(&mut peer, Vec3::new(160.0, 0.0, 50.0));
    assert_eq!(receiving_near, peer_near);

    receiving
        .update_local_script_engine_with_owner(&script_engine())
        .unwrap();
    assert_eq!(
        receiving.host_object(ranger).unwrap().target,
        Some(receiving_near)
    );
    assert_eq!(peer.host_object(peer_ranger).unwrap().target, None);
}
