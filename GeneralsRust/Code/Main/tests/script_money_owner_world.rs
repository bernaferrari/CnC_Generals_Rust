use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::ScriptEngine;
use generals_main::game_logic::{GameLogic, GameMode, Player, Team};

const OWNER: &str = "SynchronousCashOwner";

fn world(initial: u32) -> GameLogic {
    let mut world = GameLogic::new();
    world.start_new_game(GameMode::SinglePlayer);
    let mut player = Player::new(3, Team::USA, OWNER, true).with_script_identity(OWNER);
    player.resources.supplies = initial;
    world.add_player(player);
    world
}

fn engine() -> ScriptEngine {
    let mut set = ScriptAction::new(ScriptActionType::PlayerSetMoney);
    set.add_parameter(Parameter::with_string(ParameterType::Side, OWNER.into()))
        .unwrap();
    set.add_parameter(Parameter::with_int(ParameterType::Int, 1000))
        .unwrap();

    let mut give = ScriptAction::new(ScriptActionType::PlayerGiveMoney);
    give.add_parameter(Parameter::with_string(ParameterType::Side, OWNER.into()))
        .unwrap();
    give.add_parameter(Parameter::with_int(ParameterType::Int, -250))
        .unwrap();
    set.next_action = Some(Box::new(give));

    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "SynchronousCashActions".into();
    script.is_one_shot = true;
    script.condition = Some(Box::new(branch));
    script.action = Some(Box::new(set));
    let mut scripts = ScriptList::new();
    scripts.append_script(Box::new(script));

    let engine = ScriptEngine::new().expect("script templates");
    engine
        .set_script_list_for_player(0, Some(Box::new(scripts)))
        .expect("admit script");
    engine
}

#[test]
fn authored_money_actions_are_synchronous_and_scoped_to_the_receiving_world() {
    let mut owner = world(300);
    let peer = world(700);
    assert_eq!(
        owner.get_player(3).unwrap().name,
        peer.get_player(3).unwrap().name
    );

    owner
        .update_local_script_engine_with_owner(&engine())
        .expect("run the authored money script through Main's receiving driver");

    assert_eq!(
        owner.get_player(3).unwrap().effective_supplies(),
        750,
        "Set then Give are visible in source order during one synchronous callback"
    );
    assert_eq!(
        peer.get_player(3).unwrap().effective_supplies(),
        700,
        "the same-ID peer receives neither immediate nor delayed money"
    );
}
