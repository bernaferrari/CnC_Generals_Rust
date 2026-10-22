//! Actual receiving map foundation; CPP Team.cpp198 / PlayerList.cpp159.
use super::*;
use game_engine::common::well_known_keys::*;
use gamelogic::scripting::core::{
    Parameter, ParameterType, Script, ScriptAction, ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::ScriptEngine;

fn isolated(name: &str, run: impl FnOnce()) {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(module_path!(), name, run);
}

fn configured(map: &str, display: &str) -> GameLogic {
    crate::skirmish_config::lobby_definition_tests::admit_slot_definitions();
    let mut config = crate::skirmish_config::golden_skirmish_config(map);
    config.slots[0].player_name = display.into();
    config.slots[1].slot_index = 3;
    let mut world = GameLogic::new();
    // Admit the retail app rule explicitly before this real map load.
    world.admit_camera_application_movement(
        game_engine::common::ini::ini_game_data::CameraMovementRules::default()
            .overlay_ini_text("GameData\n PartitionCellSize = 40\nEnd\n")
            .unwrap(),
    );
    crate::skirmish_config::apply_skirmish_config(&mut world, &config).unwrap();
    world
}

#[test]
fn actual_retail_map_factory_uses_prepared_sparse_slots_and_receiving_controllers() {
    isolated(
        "actual_retail_map_factory_uses_prepared_sparse_slots_and_receiving_controllers",
        || {
            let map = ["Maps/Lone Eagle/Lone Eagle.map", "Lone Eagle"]
                .into_iter()
                .find_map(crate::game_logic::script_loader::find_map_file)
                .expect("retail Lone Eagle required for actual map foundation regression");
            let map = map.to_str().unwrap();
            let mut first = configured(map, "First Display");
            assert!(first.load_map(map));
            let first_before = {
                let mut factory = first.team_factory.lock().unwrap();
                let names = factory.prototype_names();
                let first_slot = factory
                    .find_team_prototype("teamplayer0")
                    .expect("prepared occupied slot0 default must reach actual receiving factory");
                let last_slot = factory
                    .find_team_prototype("teamplayer3")
                    .expect("prepared sparse slot3 default must reach actual receiving factory");
                assert!(first_slot.is_singleton() && last_slot.is_singleton());
                assert!(!names.iter().any(|name| name == "teamplayer1"));
                assert_eq!(
                    factory
                        .find_team("teamplayer0")
                        .unwrap()
                        .read()
                        .unwrap()
                        .get_controlling_player_id(),
                    Some(0)
                );
                assert_eq!(
                    factory
                        .find_team("teamplayer3")
                        .unwrap()
                        .read()
                        .unwrap()
                        .get_controlling_player_id(),
                    Some(3)
                );
                (names, first_slot.get_id(), last_slot.get_id())
            };
            let mut template = ThingTemplate::new("PreparedReceivingUnit");
            template.add_kind_of(KindOf::Infantry).set_health(100.0);
            first.templates.insert(template.name.clone(), template);
            let unit = first
                .create_object_for_player("PreparedReceivingUnit", 0, Vec3::ZERO)
                .unwrap();
            assert_eq!(
                first.host_object(unit).unwrap().team_instance_name,
                "teamplayer0"
            );
            assert_eq!(
                first.team_factory.lock().unwrap().prototype_names(),
                first_before.0,
                "object admission reuses prepared default instead of appending display-name duplicate"
            );
            let mut second = configured(map, "Second Display");
            assert!(second.load_map(map));
            {
                let mut factory = second.team_factory.lock().unwrap();
                assert_eq!(
                    factory.find_team_prototype("teamplayer0").unwrap().get_id(),
                    first_before.1
                );
                assert_eq!(
                    factory.find_team_prototype("teamplayer3").unwrap().get_id(),
                    first_before.2
                );
                assert_eq!(
                    factory
                        .find_team("teamplayer0")
                        .unwrap()
                        .read()
                        .unwrap()
                        .get_controlling_player_id(),
                    Some(0)
                );
            }
            assert_eq!(
                first.team_factory.lock().unwrap().prototype_names(),
                first_before.0,
                "peer map admission cannot reset first receiving factory"
            );
            assert_eq!(first.get_player(0).unwrap().name, "First Display");
            assert_eq!(second.get_player(0).unwrap().name, "Second Display");
        },
    );
}

fn callback_engine() -> ScriptEngine {
    let engine = ScriptEngine::new().unwrap();
    let mut list = ScriptList::new();
    for (name, kind, amount) in [
        ("First", ScriptActionType::SetCounter, 1),
        ("Second", ScriptActionType::IncrementCounter, 2),
    ] {
        let mut action = ScriptAction::new(kind);
        if matches!(kind, ScriptActionType::SetCounter) {
            action
                .add_parameter(Parameter::with_string(
                    ParameterType::Counter,
                    "PreparedOrder".into(),
                ))
                .unwrap();
            action
                .add_parameter(Parameter::with_int(ParameterType::Int, amount))
                .unwrap();
        } else {
            action
                .add_parameter(Parameter::with_int(ParameterType::Int, amount))
                .unwrap();
            action
                .add_parameter(Parameter::with_string(
                    ParameterType::Counter,
                    "PreparedOrder".into(),
                ))
                .unwrap();
        }
        let mut script = Script::new();
        script.script_name = name.into();
        script.is_subroutine = true;
        script.action = Some(Box::new(action));
        list.append_script(Box::new(script));
    }
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    engine
}

#[test]
fn prepared_authored_default_retains_configuration_and_callback_order() {
    isolated(
        "prepared_authored_default_retains_configuration_and_callback_order",
        || {
            let mut world = GameLogic::new();
            world.game_mode = GameMode::SinglePlayer;
            let mut player = Player::new(1, Team::USA, "Display Owner", true);
            player.map_side.map_player_name = "MapOwner".into();
            world.add_player(player);
            // A display label matching another side's authored owner cannot
            // override canonical map identity during controller resolution.
            let mut spoof = Player::new(2, Team::China, "MapOwner", false);
            spoof.map_side.map_player_name = "DifferentOwner".into();
            world.add_player(spoof);
            let mut neutral = Player::new(9, Team::Neutral, "Neutral", false);
            neutral.map_side.role = PlayerSideRole::Neutral;
            world.add_player(neutral);
            world.install_owned_script_engine(callback_engine());
            let mut side = Dict::new();
            side.set_ascii_string(key_player_name(), "MapOwner");
            side.set_ascii_string(key_player_faction(), "FactionAmerica");
            let mut definitions = Vec::new();
            for (name, callback) in [("teamMapOwner", "First"), ("AuthoredSecond", "Second")] {
                let mut team = Dict::new();
                team.set_ascii_string(key_team_name(), name);
                team.set_ascii_string(key_team_owner(), "MapOwner");
                team.set_bool(key_team_is_singleton(), name != "teamMapOwner");
                team.set_ascii_string(key_team_production_condition(), callback);
                team.set_bool(key_team_executes_actions_on_create(), true);
                definitions.push(team);
            }
            // validateSides repairs only the default singleton flag; it must not
            // replace this authored definition with a new inert default.
            let prepared = world.sync_legacy_sides_list_from_dicts(&[side], &definitions, &[], &[]);
            assert_eq!(
                prepared[0].get_ascii_string(key_team_name()),
                "teamMapOwner"
            );
            assert!(prepared[0].get_bool(key_team_is_singleton()));
            assert_eq!(
                prepared[0].get_ascii_string(key_team_production_condition()),
                "First"
            );
            assert!(world.sync_legacy_team_factory_from_team_dicts(&prepared));
            let mut factory = world.team_factory.lock().unwrap();
            let names = factory.prototype_names();
            assert_eq!(&names[..2], ["teamMapOwner", "AuthoredSecond"]);
            let prototype = factory.find_team_prototype("teamMapOwner").unwrap();
            assert!(prototype.is_singleton());
            assert_eq!(prototype.get_production_condition().as_str(), "First");
            assert_eq!(
                factory
                    .find_team("teamMapOwner")
                    .unwrap()
                    .read()
                    .unwrap()
                    .get_controlling_player_id(),
                Some(1)
            );
            drop(factory);
            assert_eq!(
                world
                    .owned_script_engine
                    .as_ref()
                    .unwrap()
                    .get_counter("PreparedOrder")
                    .unwrap()
                    .value,
                3
            );
        },
    );
}
