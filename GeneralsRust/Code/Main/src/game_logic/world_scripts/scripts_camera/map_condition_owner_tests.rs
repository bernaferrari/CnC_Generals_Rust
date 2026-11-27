use super::*;
use game_engine::common::rts::player_template::{PlayerTemplate, PlayerTemplateStore};
use game_engine::polygon_trigger::PolygonTrigger;
use gamelogic::scripting::engine::ScriptEngine;
use generals_sim::scripting::program::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, THIS_PLAYER,
};

fn world(side: &str, start: i32, areas: &[&str]) -> GameLogic {
    let mut world = GameLogic::new();
    let mut player =
        Player::new(1, Team::USA, "Display Name", true).with_script_identity("MapSide");
    player.start_position = start;
    world.add_player(player);
    // Both worlds use the same template identity but admit different records.
    let mut template = PlayerTemplate::new("TestGeneral".to_string());
    template.side = side.into();
    template.base_side = "USA".into();
    let mut definitions = PlayerTemplateStore::new_owned();
    definitions.add_template(template);
    world
        .admit_player_template_definitions(definitions)
        .unwrap();
    let identity = world
        .player_template_identity_from_name("TestGeneral")
        .unwrap();
    assert!(world.bind_player_template_identity(1, identity));

    let triggers: Vec<_> = areas
        .iter()
        .enumerate()
        .map(|(index, name)| {
            PolygonTrigger::new(
                index as i32 + 1,
                (*name).into(),
                vec![glam::IVec3::ZERO, glam::IVec3::new(20, 20, 0)],
            )
        })
        .collect();
    world
        .host_trigger_world
        .lock()
        .unwrap()
        .set_trigger_areas(&triggers);
    world
}

fn script(kind: ConditionType, side: &str, operand: Parameter) -> Script {
    let mut condition = Condition::new(kind);
    condition
        .add_parameter(Parameter::with_string(ParameterType::Side, side.into()))
        .unwrap();
    condition.add_parameter(operand).unwrap();
    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(condition)));
    Script {
        condition: Some(Box::new(branch)),
        ..Script::new()
    }
}

#[test]
fn script_faction_and_start_use_the_receiving_player_slot() {
    std::thread::spawn(|| {
        let mut first = world("AmericaAirForceGeneral", 2, &[]);
        let mut peer = world("AmericaLaserGeneral", 4, &[]);
        let engine = ScriptEngine::new_in_published_world().unwrap();
        let faction = script(
            ConditionType::SkirmishPlayerFaction,
            THIS_PLAYER,
            Parameter::with_string(ParameterType::FactionName, "AmericaAirForceGeneral".into()),
        );
        let start = script(
            ConditionType::StartPositionIs,
            THIS_PLAYER,
            Parameter::with_int(ParameterType::Int, 3),
        );

        for condition in [faction, start] {
            assert!(first.evaluate_owned_script_conditions(
                &engine,
                &mut condition.clone(),
                Some("MapSide")
            ));
            assert!(!peer.evaluate_owned_script_conditions(
                &engine,
                &mut condition.clone(),
                Some("MapSide")
            ));
            assert!(!first.evaluate_owned_script_conditions(
                &engine,
                &mut condition.clone(),
                Some("Display Name")
            ));
        }
    })
    .join()
    .unwrap();
}

#[test]
fn script_start_uses_the_authored_map_index_and_its_zero_default() {
    use game_engine::common::dict::Dict;
    use game_engine::common::well_known_keys::{key_multiplayer_start_index, key_player_name};

    std::thread::spawn(|| {
        let mut session = GameLogic::new();
        let mut player = Player::new(1, Team::USA, "Display Name", true);
        let mut side = Dict::new();
        side.set_ascii_string(key_player_name(), "AuthoredSide");
        player.apply_map_side_dict(&side, false, &mut Default::default());
        session.add_player(player);
        let engine = ScriptEngine::new_in_published_world().unwrap();
        let start = |index| {
            script(
                ConditionType::StartPositionIs,
                "AuthoredSide",
                Parameter::with_int(ParameterType::Int, index),
            )
        };
        assert!(session.evaluate_owned_script_conditions(&engine, &mut start(1), None));
        assert!(!session.evaluate_owned_script_conditions(&engine, &mut start(0), None));

        side.set_int(key_multiplayer_start_index(), 5);
        session.players.get_mut(&1).unwrap().apply_map_side_dict(
            &side,
            false,
            &mut Default::default(),
        );
        assert!(session.evaluate_owned_script_conditions(&engine, &mut start(6), None));
        assert!(!session.evaluate_owned_script_conditions(&engine, &mut start(1), None));
    })
    .join()
    .unwrap();
}

#[test]
fn script_named_area_ignores_side_and_qualifies_perimeter_from_current_player() {
    std::thread::spawn(|| {
        let mut first = world("America", 2, &["InnerPerimeter3", "Literal"]);
        let mut peer = world("America", 4, &["InnerPerimeter3"]);
        let engine = ScriptEngine::new_in_published_world().unwrap();
        let named_area = |name: &str| {
            script(
                ConditionType::SkirmishNamedAreaExist,
                "MissingAndUnused",
                Parameter::with_string(ParameterType::TriggerArea, name.into()),
            )
        };
        let perimeter = named_area("[Skirmish]MyInnerPerimeter");
        assert!(first.evaluate_owned_script_conditions(
            &engine,
            &mut perimeter.clone(),
            Some("MapSide")
        ));
        assert!(!peer.evaluate_owned_script_conditions(
            &engine,
            &mut perimeter.clone(),
            Some("MapSide")
        ));
        assert!(!first.evaluate_owned_script_conditions(&engine, &mut perimeter.clone(), None));
        assert!(first.evaluate_owned_script_conditions(&engine, &mut named_area("Literal"), None));
        assert!(!peer.evaluate_owned_script_conditions(&engine, &mut named_area("Literal"), None));
    })
    .join()
    .unwrap();
}
