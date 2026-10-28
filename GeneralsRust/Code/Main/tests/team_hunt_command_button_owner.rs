use game_engine::common::{
    dict::Dict,
    ini::{ini_command_button as buttons, ini_command_set as command_sets},
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
use generals_main::game_logic::{GameLogic, GameMode};

const BUTTON: &str = "Command_SwitchWeapon";
const COMMAND_SET: &str = "GlobalHuntButtonSet";
const TEAM: &str = "CommandButtonOwnerTeam";

fn install_global_button() {
    buttons::initialize_control_bar();
    command_sets::initialize_command_set_manager();
    let mut bar = buttons::get_control_bar_mut().unwrap();
    bar.new_command_button(BUTTON.into()).command = "SWITCH_WEAPON".into();
    drop(bar);
    command_sets::get_command_set_manager_mut()
        .unwrap()
        .new_command_set(COMMAND_SET.into())
        .set_button_at_position(0, BUTTON.into())
        .unwrap();
    gamelogic::control_bar::refresh_control_bar_bridge_from_common().unwrap();
    assert!(gamelogic::control_bar::get_control_bar_bridge()
        .unwrap()
        .find_command_button_by_name(BUTTON)
        .is_some());
}

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
    team.set_ascii_string(key_team_name(), TEAM);
    team.set_ascii_string(key_team_owner(), "Owner");
    team.set_bool(key_team_is_singleton(), false);
    out.write_dict(&team);
    out.close_data_chunk();
    out.open_data_chunk("ObjectsList", 3);
    out.close_data_chunk();
    let path = root.join("CommandButtonOwner.map");
    std::fs::write(&path, out.into_ckmp_bytes()).unwrap();
    path
}

fn world(map: &std::path::Path) -> GameLogic {
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([(
            "CommandButtonOwner.ini".into(),
            format!(
                "Object CommandButtonOwnerRanger\n KindOf = SELECTABLE INFANTRY\n CommandSet = {COMMAND_SET}\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface Brain\n End\n Behavior = CommandButtonHuntUpdate Hunt\n End\nEnd\n"
            ),
        )])
        .unwrap();
    world.start_new_game(GameMode::SinglePlayer);
    assert!(world.load_map(map.to_str().unwrap()));
    world
}

fn add_team_member(world: &mut GameLogic) -> generals_main::game_logic::ObjectId {
    let id = world
        .create_object_for_player(
            "CommandButtonOwnerRanger",
            1,
            glam::Vec3::new(50.0, 0.0, 50.0),
        )
        .unwrap();
    world.host_object_mut(id).unwrap().team_instance_name = TEAM.into();
    world.activate_leftover_team_for_host_object(id);
    id
}

fn script_engine() -> ScriptEngine {
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut action = ScriptAction::new(ScriptActionType::TeamHuntWithCommandButton);
    action
        .add_parameter(Parameter::with_string(ParameterType::Team, TEAM.into()))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::CommandButton,
            BUTTON.into(),
        ))
        .unwrap();
    let mut script = Script::new();
    script.script_name = "HuntWithGlobalOnlyButton".into();
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
fn authored_hunt_button_action_ignores_global_button_when_receiving_catalog_is_empty() {
    install_global_button();
    let root = tempfile::tempdir().unwrap();
    let map = map_file(root.path());
    let mut receiving = world(&map);
    let mut peer = world(&map);
    let receiving_id = add_team_member(&mut receiving);
    let peer_id = add_team_member(&mut peer);
    assert_eq!(receiving_id, peer_id);

    receiving
        .update_local_script_engine_with_owner(&script_engine())
        .unwrap();

    assert!(
        !receiving.host_object(receiving_id).unwrap().is_hunting(),
        "an admitted empty action catalog is authoritative even when Common has a matching button"
    );
    assert!(!peer.host_object(peer_id).unwrap().is_hunting());
}
