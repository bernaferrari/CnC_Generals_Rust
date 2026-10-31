use game_engine::common::dict::Dict;
use game_engine::common::ini::INILoadType;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::rts::player_template::PlayerTemplateStore;
use game_engine::common::system::DataChunkOutput;
use game_engine::common::well_known_keys::{
    key_player_faction, key_player_is_human, key_player_name, key_team_enemy_sighted_script,
    key_team_is_singleton, key_team_name, key_team_owner,
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
    world_with_target(
        root,
        boundary,
        button_admitted,
        glam::Vec3::new(80.0, 0.0, 80.0),
    )
}

fn world_with_target(
    root: &std::path::Path,
    boundary: i32,
    button_admitted: bool,
    target_position: glam::Vec3,
) -> (GameLogic, ObjectId, ObjectId) {
    world_with_roster(root, boundary, button_admitted, target_position, false)
}

fn world_with_roster(
    root: &std::path::Path,
    boundary: i32,
    button_admitted: bool,
    target_position: glam::Vec3,
    add_non_ai_member: bool,
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
    team.set_ascii_string(key_team_enemy_sighted_script(), "OnOwnedEnemySighted");
    sides.add_team(&team);
    let mut action = ScriptAction::new(ScriptActionType::PlayerGiveMoney);
    action
        .add_parameter(Parameter::with_string(ParameterType::Side, "Owner".into()))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, 17))
        .unwrap();
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut hook = Script::new();
    hook.script_name = "OnOwnedEnemySighted".into();
    hook.is_active = true;
    hook.is_subroutine = true;
    hook.condition = Some(Box::new(condition));
    hook.action = Some(Box::new(action));
    let mut scripts = ScriptList::new();
    scripts.append_script(Box::new(hook));
    sides
        .get_side_info_mut(0)
        .unwrap()
        .set_script_list(Some(Box::new(scripts)));
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
    if add_non_ai_member {
        // Team admission prepends: this member is visited before the caster.
        map.open_data_chunk("Object", 3);
        for value in [95.0, 95.0, 0.0, 0.0] {
            map.write_real(value);
        }
        map.write_int(0);
        map.write_ascii_string("OwnedNoAiInfantry");
        let mut properties = Dict::new();
        properties.set_ascii_string(NameKeyGenerator::name_to_key("objectName"), "NoAiMember");
        properties.set_ascii_string(NameKeyGenerator::name_to_key("originalOwner"), "OwnedTeam");
        map.write_dict(&properties);
        map.close_data_chunk();
    }
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
    game.admit_object_ini_sources([("Objects.ini".into(), "Object AmericaInfantryMissileDefender\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface Brain\n End\n Behavior = SpecialAbility Laser\n SpecialPowerTemplate = SpecialAbilityMissileDefenderLaserGuidedMissiles\n End\nEnd\nObject OwnedNoAiInfantry\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\nObject OwnedVehicle\n KindOf = VEHICLE SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 500\n InitialHealth = 500\n End\nEnd\nObject OwnedNearestGarrison\n KindOf = STRUCTURE SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 1000\n InitialHealth = 1000\n End\n Behavior = GarrisonContain GarrisonTag\n ContainMax = 1\n End\nEnd\nObject OwnedZeroSlotGarrison\n KindOf = STRUCTURE SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 1000\n InitialHealth = 1000\n End\n Behavior = GarrisonContain GarrisonTag\n ContainMax = 0\n End\nEnd\nObject OwnedNoContainer\n KindOf = STRUCTURE SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 1000\n InitialHealth = 1000\n End\nEnd\n".into())]).unwrap();
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
        .create_object_for_player("OwnedVehicle", 2, target_position)
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

#[test]
fn team_sighting_hook_uses_receiving_map_status() {
    let small_root = tempfile::tempdir().unwrap();
    let large_root = tempfile::tempdir().unwrap();
    let off_root = tempfile::tempdir().unwrap();
    let (mut small, source, _) = world(small_root.path(), 10, true);
    let (mut large, peer, _) = world(large_root.path(), 20, true);
    let (mut both_off, third, _) = world(off_root.path(), 8, true);
    assert_eq!((source, source), (peer, third));
    for (world, id) in [
        (&mut small, source),
        (&mut large, peer),
        (&mut both_off, third),
    ] {
        world.host_object_mut(id).unwrap().vision_range = 100.0;
    }
    let initial_small = small.get_player(1).unwrap().resources.supplies;
    let initial_large = large.get_player(1).unwrap().resources.supplies;
    let initial_off = both_off.get_player(1).unwrap().resources.supplies;
    large.update();
    large.update();
    assert_eq!(
        large.get_player(1).unwrap().resources.supplies,
        initial_large + 17
    );
    assert_eq!(
        small.get_player(1).unwrap().resources.supplies,
        initial_small
    );
    small.update();
    small.update();
    assert_eq!(
        small.get_player(1).unwrap().resources.supplies,
        initial_small
    );
    both_off.update();
    both_off.update();
    assert_eq!(
        both_off.get_player(1).unwrap().resources.supplies,
        initial_off + 17
    );
}

#[test]
fn team_sighting_hook_requires_strictly_inside_vision_range() {
    let edge_root = tempfile::tempdir().unwrap();
    let inside_root = tempfile::tempdir().unwrap();
    let target_position = glam::Vec3::new(80.0, 0.0, 100.0);
    let (mut edge, source, _) = world_with_target(edge_root.path(), 20, true, target_position);
    let (mut inside, peer, _) = world_with_target(inside_root.path(), 20, true, target_position);
    assert_eq!(source, peer);
    edge.host_object_mut(source).unwrap().vision_range = 20.0;
    inside.host_object_mut(peer).unwrap().vision_range = 20.01;
    let edge_before = edge.get_player(1).unwrap().resources.supplies;
    let inside_before = inside.get_player(1).unwrap().resources.supplies;
    inside.update();
    inside.update();
    assert_eq!(
        inside.get_player(1).unwrap().resources.supplies,
        inside_before + 17
    );
    edge.update();
    edge.update();
    assert_eq!(edge.get_player(1).unwrap().resources.supplies, edge_before);
}

fn execute_garrison(game: &mut GameLogic, by_team: bool) {
    execute_garrison_for(game, by_team, "OwnedCaster");
}

fn execute_garrison_for(game: &mut GameLogic, by_team: bool, unit: &str) {
    let mut action = ScriptAction::new(if by_team {
        ScriptActionType::TeamGarrisonNearestBuilding
    } else {
        ScriptActionType::NamedGarrisonNearestBuilding
    });
    action
        .add_parameter(Parameter::with_string(
            if by_team {
                ParameterType::Team
            } else {
                ParameterType::Unit
            },
            if by_team { "OwnedTeam" } else { unit }.into(),
        ))
        .unwrap();
    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "OwnedGarrisonCommand".into();
    script.is_one_shot = true;
    script.delay_evaluation_seconds = 0;
    script.condition = Some(Box::new(branch));
    script.action = Some(Box::new(action));
    let mut scripts = ScriptList::new();
    scripts.append_script(Box::new(script));
    let engine = ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(scripts)))
        .unwrap();
    game.update_local_script_engine_with_owner(&engine).unwrap();
}

#[test]
fn team_garrison_skips_infantry_without_ai_before_consuming_slot() {
    let root = tempfile::tempdir().unwrap();
    let (mut game, source, _) = world_with_roster(
        root.path(),
        20,
        true,
        glam::Vec3::new(80.0, 0.0, 80.0),
        true,
    );
    let non_ai = game
        .host_objects()
        .registered_values()
        .find(|object| object.name == "NoAiMember")
        .unwrap()
        .id;
    assert!(!game.host_object(non_ai).unwrap().has_ai_update_interface());
    assert!(game.host_object(source).unwrap().has_ai_update_interface());
    let building = game
        .create_object_for_player("OwnedNearestGarrison", 1, glam::Vec3::new(80.0, 0.0, 80.0))
        .unwrap();
    execute_garrison(&mut game, true);
    assert_eq!(game.host_object(non_ai).unwrap().target, None);
    assert_eq!(game.host_object(source).unwrap().target, Some(building));
}

#[test]
fn named_garrison_does_not_order_infantry_without_ai() {
    let root = tempfile::tempdir().unwrap();
    let (mut game, _, _) = world_with_roster(
        root.path(),
        20,
        true,
        glam::Vec3::new(80.0, 0.0, 80.0),
        true,
    );
    let non_ai = game
        .host_objects()
        .registered_values()
        .find(|object| object.name == "NoAiMember")
        .unwrap()
        .id;
    assert!(!game.host_object(non_ai).unwrap().has_ai_update_interface());
    game.create_object_for_player("OwnedNearestGarrison", 1, glam::Vec3::new(80.0, 0.0, 80.0))
        .unwrap();
    execute_garrison_for(&mut game, false, "NoAiMember");
    assert_eq!(game.host_object(non_ai).unwrap().target, None);
}

#[test]
fn named_and_team_garrison_use_receiving_map_status() {
    for by_team in [true, false] {
        let small_root = tempfile::tempdir().unwrap();
        let large_root = tempfile::tempdir().unwrap();
        let off_root = tempfile::tempdir().unwrap();
        let (mut small, source, _) = world(small_root.path(), 10, true);
        let (mut large, peer, _) = world(large_root.path(), 20, true);
        let (mut both_off, third, _) = world(off_root.path(), 8, true);
        assert_eq!((source, source), (peer, third));
        let small_building = small
            .create_object_for_player("OwnedNearestGarrison", 1, glam::Vec3::new(80.0, 0.0, 80.0))
            .unwrap();
        let large_building = large
            .create_object_for_player("OwnedNearestGarrison", 1, glam::Vec3::new(80.0, 0.0, 80.0))
            .unwrap();
        let off_building = both_off
            .create_object_for_player("OwnedNearestGarrison", 1, glam::Vec3::new(80.0, 0.0, 80.0))
            .unwrap();
        assert_eq!(
            (small_building, small_building),
            (large_building, off_building)
        );
        execute_garrison(&mut large, by_team);
        assert_eq!(
            large.host_object(peer).unwrap().target,
            Some(large_building),
            "positive on-map case, team={by_team}"
        );
        execute_garrison(&mut small, by_team);
        assert_eq!(
            small.host_object(source).unwrap().target,
            None,
            "mixed map status, team={by_team}"
        );
        execute_garrison(&mut both_off, by_team);
        assert_eq!(
            both_off.host_object(third).unwrap().target,
            Some(off_building),
            "positive off-map case, team={by_team}"
        );
    }
}

#[test]
fn named_and_team_garrison_use_different_capacity_rules() {
    let named_root = tempfile::tempdir().unwrap();
    let team_root = tempfile::tempdir().unwrap();
    let (mut named, named_unit, _) = world(named_root.path(), 20, true);
    let (mut team, team_unit, _) = world(team_root.path(), 20, true);
    assert_eq!(named_unit, team_unit);
    let mut ids = Vec::new();
    for game in [&mut named, &mut team] {
        let no_container = game
            .create_object_for_player("OwnedNoContainer", 1, glam::Vec3::new(90.0, 0.0, 90.0))
            .unwrap();
        let zero_capacity = game
            .create_object_for_player("OwnedZeroSlotGarrison", 1, glam::Vec3::new(80.0, 0.0, 80.0))
            .unwrap();
        let available = game
            .create_object_for_player("OwnedNearestGarrison", 1, glam::Vec3::new(50.0, 0.0, 50.0))
            .unwrap();
        ids.push((no_container, zero_capacity, available));
    }
    assert_eq!(ids[0], ids[1]);
    execute_garrison(&mut team, true);
    assert_eq!(team.host_object(team_unit).unwrap().target, Some(ids[1].2));
    execute_garrison(&mut named, false);
    assert_eq!(
        named.host_object(named_unit).unwrap().target,
        Some(ids[0].1)
    );
}
