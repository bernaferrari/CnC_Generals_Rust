use game_engine::common::dict::Dict;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::DataChunkOutput;
use game_engine::common::well_known_keys::{
    key_player_faction, key_player_is_human, key_player_name, key_team_is_singleton, key_team_name,
    key_team_on_unit_destroyed_script, key_team_owner,
};
use gamelogic::scripting::chunk_codec::ScriptTemplateLookup;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList, THIS_TEAM,
};
use gamelogic::scripting::engine::ScriptEngine;
use gamelogic::sides_list::SidesList;
use generals_main::game_logic::{GameLogic, GameMode, ObjectId};
use glam::Vec3;

const UNIT: &str = "ResidentDeathOwnerUnit";
const TEAM: &str = "ResidentDeathOwnerTeam";
const SCRIPT: &str = "ResidentDeathOwnerSubroutine";
const WAYPOINT: &str = "ResidentDeathDestination";

fn map_file(root: &std::path::Path, destination: [f32; 3]) -> std::path::PathBuf {
    let mut sides = SidesList::new();
    let mut owner = Dict::new();
    owner.set_ascii_string(key_player_name(), "Owner");
    owner.set_ascii_string(key_player_faction(), "FactionAmerica");
    owner.set_bool(key_player_is_human(), true);
    sides.add_side(&owner);

    let mut team = Dict::new();
    team.set_ascii_string(key_team_name(), TEAM);
    team.set_ascii_string(key_team_owner(), "Owner");
    team.set_ascii_string(key_team_on_unit_destroyed_script(), SCRIPT);
    team.set_bool(key_team_is_singleton(), true);
    sides.add_team(&team);

    let mut action = ScriptAction::new(ScriptActionType::MoveTeamTo);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            THIS_TEAM.into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Waypoint,
            WAYPOINT.into(),
        ))
        .unwrap();
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut subroutine = Script::new();
    subroutine.script_name = SCRIPT.into();
    subroutine.is_subroutine = true;
    subroutine.condition = Some(Box::new(condition));
    subroutine.action = Some(Box::new(action));
    let mut script_list = ScriptList::new();
    script_list.append_script(Box::new(subroutine));
    sides
        .get_side_info_mut(0)
        .expect("Owner map side")
        .set_script_list(Some(Box::new(script_list)));

    let engine = ScriptEngine::new().expect("script templates");
    let mut out = DataChunkOutput::new();
    out.open_data_chunk("HeightMapData", 3);
    for value in [41, 41, 1, 1681] {
        out.write_int(value);
    }
    for _ in 0..1681 {
        out.write_byte(0);
    }
    out.close_data_chunk();
    out.open_data_chunk("ObjectsList", 3);
    for (name, position) in [
        ("ResidentDeathVictim", [100.0, 0.0, 100.0]),
        ("ResidentDeathSurvivor", [104.0, 0.0, 100.0]),
    ] {
        out.open_data_chunk("Object", 3);
        for value in [position[0], position[2], position[1], 0.0] {
            out.write_real(value);
        }
        out.write_int(0);
        out.write_ascii_string(UNIT);
        let mut properties = Dict::new();
        properties.set_ascii_string(NameKeyGenerator::name_to_key("objectName"), name);
        properties.set_ascii_string(NameKeyGenerator::name_to_key("originalOwner"), TEAM);
        out.write_dict(&properties);
        out.close_data_chunk();
    }
    out.open_data_chunk("Object", 3);
    for value in [destination[0], destination[2], destination[1], 0.0] {
        out.write_real(value);
    }
    out.write_int(0);
    out.write_ascii_string("Waypoint");
    let mut waypoint = Dict::new();
    waypoint.set_int(NameKeyGenerator::name_to_key("waypointID"), 701);
    waypoint.set_ascii_string(NameKeyGenerator::name_to_key("waypointName"), WAYPOINT);
    out.write_dict(&waypoint);
    out.close_data_chunk();
    out.close_data_chunk();
    sides.write_sides_data_chunk(&mut out, &ScriptTemplateLookup::from_engine(&engine));

    let path = root.join("ResidentDeath.map");
    std::fs::write(&path, out.into_ckmp_bytes()).expect("write receiving map");
    std::fs::write(root.join("Map.ini"), "").expect("write empty map overrides");
    path
}

fn world(map: &std::path::Path) -> (GameLogic, ObjectId, ObjectId) {
    let mut game = GameLogic::new();
    game.admit_object_ini_sources([(
        "ResidentDeathOwner.ini".into(),
        format!(
            "Object {UNIT}\n KindOf = INFANTRY SELECTABLE\n IsTrainable = Yes\n ExperienceRequired = 0 100 250 450\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface Brain\n End\nEnd\n"
        ),
    )])
    .expect("admit receiving unit template");
    game.start_new_game(GameMode::SinglePlayer);
    assert!(game.load_map(map.to_str().expect("UTF-8 map path")));
    // The authored map side at index 0 is admitted as Main player 1.
    assert!(game.get_player(1).is_some(), "map admits Owner participant");

    let find = |name: &str| {
        game.host_objects()
            .registered_values()
            .find(|object| object.name == name)
            .map(|object| object.id)
            .expect("map-admitted resident member")
    };
    let first = find("ResidentDeathVictim");
    let survivor = find("ResidentDeathSurvivor");
    let mut census = game.host_script_team_census_member_ids(TEAM);
    census.sort_unstable();
    let mut expected = vec![first.0, survivor.0];
    expected.sort_unstable();
    assert_eq!(
        census, expected,
        "both map-admitted members belong to the team"
    );
    assert_eq!(
        game.host_object(survivor).unwrap().requested_destination,
        None,
        "fixture action has not run before death"
    );
    (game, first, survivor)
}

#[test]
fn resident_team_death_subroutine_runs_synchronously_in_its_receiving_world() {
    let owner_root = tempfile::tempdir().unwrap();
    let peer_root = tempfile::tempdir().unwrap();
    let owner_map = map_file(owner_root.path(), [180.0, 0.0, 210.0]);
    let peer_map = map_file(peer_root.path(), [300.0, 0.0, 320.0]);
    let (mut owner, owner_dead, owner_survivor) = world(&owner_map);
    let (mut peer, peer_dead, peer_survivor) = world(&peer_map);
    assert_eq!(
        (owner_dead, owner_survivor),
        (peer_dead, peer_survivor),
        "independent maps reuse ObjectIds"
    );

    // The public damage route enters Main's ordinary onDie callback boundary.
    // The map-authored team subroutine must finish before this call returns.
    assert_eq!(
        owner.apply_host_hive_damage_from(
            owner_dead,
            100.0,
            generals_main::game_logic::host_base_defense::HostHiveDamageClass::HitStructure,
            None,
            Some(owner_survivor),
        ),
        (true, false)
    );
    assert_eq!(
        owner
            .host_object(owner_survivor)
            .unwrap()
            .requested_destination,
        Some(Vec3::new(180.0, 0.0, 210.0)),
        "OnUnitDestroyed must synchronously run the receiving map's team script"
    );
    assert_eq!(
        peer.host_object(peer_survivor)
            .unwrap()
            .requested_destination,
        None,
        "same-ID peer script state remains isolated"
    );

    assert_eq!(
        peer.apply_host_hive_damage_from(
            peer_dead,
            100.0,
            generals_main::game_logic::host_base_defense::HostHiveDamageClass::HitStructure,
            None,
            Some(peer_survivor),
        ),
        (true, false)
    );
    assert_eq!(
        peer.host_object(peer_survivor)
            .unwrap()
            .requested_destination,
        Some(Vec3::new(300.0, 0.0, 320.0)),
        "peer callback must use its own map waypoint"
    );
    assert_eq!(
        owner
            .host_object(owner_survivor)
            .unwrap()
            .requested_destination,
        Some(Vec3::new(180.0, 0.0, 210.0)),
        "the second callback cannot overwrite the first world"
    );
}

#[test]
fn scripted_death_runs_the_callback_on_the_already_borrowed_interpreter() {
    let owner_root = tempfile::tempdir().unwrap();
    let peer_root = tempfile::tempdir().unwrap();
    let owner_map = map_file(owner_root.path(), [180.0, 0.0, 210.0]);
    let peer_map = map_file(peer_root.path(), [300.0, 0.0, 320.0]);
    let (mut owner, owner_dead, owner_survivor) = world(&owner_map);
    let (peer, peer_dead, peer_survivor) = world(&peer_map);
    assert_eq!((owner_dead, owner_survivor), (peer_dead, peer_survivor));

    let true_condition = || {
        let mut condition = OrCondition::new();
        condition
            .set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
        Some(Box::new(condition))
    };

    let mut move_team = ScriptAction::new(ScriptActionType::MoveTeamTo);
    move_team
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            THIS_TEAM.into(),
        ))
        .unwrap();
    move_team
        .add_parameter(Parameter::with_string(
            ParameterType::Waypoint,
            WAYPOINT.into(),
        ))
        .unwrap();
    let mut callback = Script::new();
    callback.script_name = SCRIPT.into();
    callback.is_subroutine = true;
    callback.condition = true_condition();
    callback.action = Some(Box::new(move_team));

    let mut kill = ScriptAction::new(ScriptActionType::NamedKill);
    kill.add_parameter(Parameter::with_string(
        ParameterType::Unit,
        "ResidentDeathVictim".into(),
    ))
    .unwrap();
    let mut trigger = Script::new();
    trigger.script_name = "KillResidentTeamMember".into();
    trigger.is_one_shot = true;
    trigger.delay_evaluation_seconds = 0;
    trigger.condition = true_condition();
    trigger.action = Some(Box::new(kill));

    let mut scripts = ScriptList::new();
    scripts.append_script(Box::new(callback));
    scripts.append_script(Box::new(trigger));
    let interpreter = ScriptEngine::new().unwrap();
    interpreter
        .set_script_list_for_player(0, Some(Box::new(scripts)))
        .unwrap();
    owner
        .update_local_script_engine_with_owner(&interpreter)
        .unwrap();

    assert!(!owner.host_object(owner_dead).unwrap().is_alive());
    assert_eq!(
        owner
            .host_object(owner_survivor)
            .unwrap()
            .requested_destination,
        Some(Vec3::new(180.0, 0.0, 210.0)),
        "the nested death callback uses the interpreter already executing the kill"
    );
    assert!(peer.host_object(peer_dead).unwrap().is_alive());
    assert_eq!(
        peer.host_object(peer_survivor)
            .unwrap()
            .requested_destination,
        None
    );
}
