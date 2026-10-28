use game_engine::common::{
    dict::Dict,
    system::DataChunkOutput,
    well_known_keys::{
        key_player_display_name, key_player_faction, key_player_is_human, key_player_name,
        key_team_executes_actions_on_create, key_team_is_singleton, key_team_name, key_team_owner,
        key_team_production_condition,
    },
};
use gamelogic::scripting::{
    core::{
        Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
        ScriptActionType, ScriptList,
    },
    engine::ScriptEngine,
};
use generals_main::game_logic::{GameLogic, GameMode};

fn map_file(root: &std::path::Path, create_callback: bool) -> std::path::PathBuf {
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
    team.set_ascii_string(key_team_name(), "SameNamedCallbackTeam");
    team.set_ascii_string(key_team_owner(), "Owner");
    team.set_bool(key_team_is_singleton(), false);
    team.set_ascii_string(
        key_team_production_condition(),
        if create_callback {
            "NestedTeamCallback"
        } else {
            ""
        },
    );
    team.set_bool(key_team_executes_actions_on_create(), create_callback);
    out.write_dict(&team);
    out.close_data_chunk();
    out.open_data_chunk("ObjectsList", 3);
    out.close_data_chunk();
    let path = root.join("TeamHunt.map");
    std::fs::write(&path, out.into_ckmp_bytes()).unwrap();
    path
}

fn world(map: &std::path::Path) -> GameLogic {
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([(
            "TeamHunt.ini".into(),
            "Object HuntUnit\n KindOf = SELECTABLE INFANTRY\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface Brain\n End\nEnd\n".into(),
        )])
        .unwrap();
    world.start_new_game(GameMode::SinglePlayer);
    assert!(world.load_map(map.to_str().unwrap()));
    assert!(world.get_player(1).is_some());
    world
}

fn add_authored_team_member(world: &mut GameLogic) -> generals_main::game_logic::ObjectId {
    let id = world
        .create_object_for_player("HuntUnit", 1, glam::Vec3::new(50.0, 0.0, 50.0))
        .unwrap();
    let object = world.host_object_mut(id).unwrap();
    object.team_instance_name = "SameNamedCallbackTeam".into();
    world.activate_leftover_team_for_host_object(id);
    id
}

fn script_engine(team: &str) -> ScriptEngine {
    fn true_condition() -> Box<OrCondition> {
        let mut condition = OrCondition::new();
        condition
            .set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
        Box::new(condition)
    }

    let mut callback_action = ScriptAction::new(ScriptActionType::SetFlag);
    callback_action
        .add_parameter(Parameter::with_string(
            ParameterType::Flag,
            "ReceivingTeamCallbackRan".into(),
        ))
        .unwrap();
    callback_action
        .add_parameter(Parameter::with_int(ParameterType::Boolean, 1))
        .unwrap();
    let mut callback = Script::new();
    callback.script_name = "NestedTeamCallback".into();
    callback.is_subroutine = true;
    callback.is_one_shot = true;
    callback.condition = Some(true_condition());
    callback.action = Some(Box::new(callback_action));

    let mut hunt = ScriptAction::new(ScriptActionType::TeamHunt);
    hunt.add_parameter(Parameter::with_string(ParameterType::Team, team.into()))
        .unwrap();
    let mut trigger = Script::new();
    trigger.script_name = "TriggerTeamHunt".into();
    trigger.is_one_shot = true;
    trigger.delay_evaluation_seconds = 0;
    trigger.condition = Some(true_condition());
    trigger.action = Some(Box::new(hunt));

    let mut list = ScriptList::new();
    list.append_script(Box::new(callback));
    list.append_script(Box::new(trigger));
    let engine = ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    engine
}

#[test]
fn team_hunt_uses_existing_receiving_team_when_object_ids_collide() {
    let root = tempfile::tempdir().unwrap();
    let map = map_file(root.path(), false);
    let mut receiving = world(&map);
    let mut peer = world(&map);
    let receiving_id = add_authored_team_member(&mut receiving);
    let peer_id = add_authored_team_member(&mut peer);
    assert_eq!(receiving_id, peer_id);

    let receiving_engine = script_engine("SameNamedCallbackTeam");
    receiving
        .update_local_script_engine_with_owner(&receiving_engine)
        .unwrap();

    assert!(receiving.host_object(receiving_id).unwrap().is_hunting());
    assert_eq!(
        receiving
            .host_object(receiving_id)
            .unwrap()
            .last_command_source,
        1, // C++ CMD_FROM_SCRIPT
    );
    assert!(!peer.host_object(peer_id).unwrap().is_hunting());
}

#[test]
fn team_hunt_missing_authored_instance_does_not_create_team_or_run_callback() {
    let root = tempfile::tempdir().unwrap();
    let map = map_file(root.path(), true);
    let mut receiving = world(&map);
    let _peer = world(&map);
    let receiving_engine = script_engine("SameNamedCallbackTeam");
    let peer_engine = script_engine("SameNamedCallbackTeam");

    receiving
        .update_local_script_engine_with_owner(&receiving_engine)
        .unwrap();

    assert!(
        receiving_engine
            .get_flag("ReceivingTeamCallbackRan")
            .is_none_or(|flag| !flag.value)
    );
    assert!(
        peer_engine
            .get_flag("ReceivingTeamCallbackRan")
            .is_none_or(|flag| !flag.value)
    );
}
