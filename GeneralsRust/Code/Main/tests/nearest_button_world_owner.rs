use game_engine::common::dict::Dict;
use game_engine::common::ini::INILoadType;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::rts::player_template::PlayerTemplateStore;
use game_engine::common::system::DataChunkOutput;
use game_engine::common::well_known_keys::{
    key_player_faction, key_player_is_human, key_player_name, key_team_is_singleton, key_team_name,
    key_team_owner,
};
use gamelogic::scripting::chunk_codec::ScriptTemplateLookup;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::ScriptEngine;
use gamelogic::sides_list::SidesList;
use generals_main::game_logic::{AIState, GameLogic, GameMode, ObjectId, Player, Team};

fn world(
    root: &std::path::Path,
    boundary: i32,
    button_admitted: bool,
) -> (GameLogic, ObjectId, ObjectId) {
    let mut sides = SidesList::new();
    let mut player = Dict::new();
    player.set_ascii_string(key_player_name(), "Owner");
    player.set_ascii_string(key_player_faction(), "FactionAmerica");
    player.set_bool(key_player_is_human(), false);
    sides.add_side(&player);
    let mut team = Dict::new();
    team.set_ascii_string(key_team_name(), "OwnedTeam");
    team.set_ascii_string(key_team_owner(), "Owner");
    team.set_bool(key_team_is_singleton(), true);
    sides.add_team(&team);
    let engine = ScriptEngine::new().unwrap();
    let mut map = DataChunkOutput::new();
    map.open_data_chunk("HeightMapData", 4);
    for value in [41, 41, 1, 1, boundary, boundary, 1681] {
        map.write_int(value);
    }
    for _ in 0..1681 {
        map.write_byte(0);
    }
    map.close_data_chunk();
    map.open_data_chunk("ObjectsList", 3);
    map.open_data_chunk("Object", 3);
    for value in [100.0, 100.0, 0.0, 0.0] {
        map.write_real(value);
    }
    map.write_int(0);
    map.write_ascii_string("AmericaInfantryMissileDefender");
    let mut properties = Dict::new();
    properties.set_ascii_string(NameKeyGenerator::name_to_key("objectName"), "OwnedCaster");
    properties.set_ascii_string(NameKeyGenerator::name_to_key("originalOwner"), "OwnedTeam");
    map.write_dict(&properties);
    map.close_data_chunk();
    map.close_data_chunk();
    sides.write_sides_data_chunk(&mut map, &ScriptTemplateLookup::from_engine(&engine));
    let path = root.join("NearestButton.map");
    std::fs::write(&path, map.into_ckmp_bytes()).unwrap();
    let power = "SpecialPower SpecialAbilityMissileDefenderLaserGuidedMissiles\n Enum = SPECIAL_MISSILE_DEFENDER_LASER_GUIDED_MISSILES\n ReloadTime = 0\nEnd\n";
    let button = "CommandButton OwnedLaser\n Command = SPECIAL_POWER\n SpecialPower = SpecialAbilityMissileDefenderLaserGuidedMissiles\n Options = NEED_TARGET_ENEMY_OBJECT\nEnd\n";
    std::fs::write(
        root.join("Map.ini"),
        format!("{power}{}", if button_admitted { button } else { "" }),
    )
    .unwrap();
    let mut game = GameLogic::new();
    game.admit_object_ini_sources([("Objects.ini".into(), "Object AmericaInfantryMissileDefender\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface Brain\n End\n Behavior = SpecialAbility Laser\n SpecialPowerTemplate = SpecialAbilityMissileDefenderLaserGuidedMissiles\n End\nEnd\nObject OwnedVehicle\n KindOf = VEHICLE SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 500\n InitialHealth = 500\n End\nEnd\n".into())]).unwrap();
    let mut definitions = PlayerTemplateStore::new_owned();
    definitions.apply_source("PlayerTemplate FactionCivilian\n Side = Civilian\nEnd\nPlayerTemplate FactionAmerica\n Side = America\nEnd\n", INILoadType::Overwrite).unwrap();
    game.admit_player_template_definitions(definitions).unwrap();
    game.start_new_game(GameMode::SinglePlayer);
    assert!(game.load_map(path.to_str().unwrap()));
    game.add_player(Player::new(2, Team::China, "Opponent", false));
    game.get_player_mut(1)
        .unwrap()
        .set_map_relationship(2, gamelogic::common::Relationship::Enemies);
    let source = game
        .host_objects()
        .registered_values()
        .find(|object| object.name == "OwnedCaster")
        .unwrap()
        .id;
    assert!(
        !game
            .host_object(source)
            .unwrap()
            .get_template()
            .special_power_modules
            .is_empty(),
        "authored caster must have its special-power module"
    );
    game.host_object_mut(source).unwrap().secondary_weapon = Some(
        generals_main::game_logic::host_missile_defender::missile_defender_laser_guided_weapon(),
    );
    game.host_object_mut(source)
        .unwrap()
        .set_special_power_ready_seconds(
            &generals_main::command_system::SpecialPowerType::MissileDefenderLaserGuided,
            0.0,
        );
    let target = game
        .create_object_for_player("OwnedVehicle", 2, glam::Vec3::new(80.0, 0.0, 80.0))
        .unwrap();
    (game, source, target)
}

fn execute(game: &mut GameLogic) {
    let mut action = ScriptAction::new(ScriptActionType::TeamAllUseCommandbuttonOnNearestEnemyUnit);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "OwnedTeam".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::CommandButton,
            "OwnedLaser".into(),
        ))
        .unwrap();
    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "OwnerNearestLaser".into();
    script.is_one_shot = true;
    script.delay_evaluation_seconds = 0;
    script.condition = Some(Box::new(branch));
    script.action = Some(Box::new(action));
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let engine = ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    game.update_local_script_engine_with_owner(&engine).unwrap();
}

#[test]
fn nearest_button_uses_receiving_definitions_and_map_status() {
    let small_root = tempfile::tempdir().unwrap();
    let large_root = tempfile::tempdir().unwrap();
    let off_root = tempfile::tempdir().unwrap();
    let missing_root = tempfile::tempdir().unwrap();
    let (mut small, source, _) = world(small_root.path(), 10, true);
    let (mut large, peer, _) = world(large_root.path(), 20, true);
    let (mut both_off, third, _) = world(off_root.path(), 8, true);
    let (mut missing, fourth, _) = world(missing_root.path(), 20, false);
    assert_eq!((source, source), (peer, third));
    execute(&mut large);
    assert_eq!(
        large.host_object(peer).unwrap().ai_state(),
        AIState::SpecialAbility
    );
    execute(&mut small);
    assert_eq!(small.host_object(source).unwrap().ai_state(), AIState::Idle);
    execute(&mut both_off);
    assert_eq!(
        both_off.host_object(third).unwrap().ai_state(),
        AIState::SpecialAbility
    );
    assert_eq!(source, fourth);
    execute(&mut missing);
    assert_eq!(
        missing.host_object(fourth).unwrap().ai_state(),
        AIState::Idle
    );
}
