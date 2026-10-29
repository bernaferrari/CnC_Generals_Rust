use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList, THIS_PLAYER,
};
use gamelogic::scripting::engine::ScriptEngine;
use gamelogic::scripting::executor::{ScriptActionDispatcher, ScriptContext};
use generals_main::game_logic::{GameLogic, Player, Team};
use std::cell::RefCell;

fn action(kind: ScriptActionType, player: Option<&str>, value: Option<Parameter>) -> ScriptAction {
    let mut action = ScriptAction::new(kind);
    if let Some(player) = player {
        action
            .add_parameter(Parameter::with_string(ParameterType::Side, player.into()))
            .unwrap();
    }
    if let Some(value) = value {
        action.add_parameter(value).unwrap();
    }
    action
}

fn run(world: &mut GameLogic, side: usize, actions: Vec<ScriptAction>) {
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "RankOwner".into();
    script.condition = Some(Box::new(condition));
    script.is_one_shot = true;
    for action in actions {
        script.append_action(Box::new(action));
    }
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let engine = ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(side, Some(Box::new(list)))
        .unwrap();
    world
        .update_local_script_engine_with_owner(&engine)
        .unwrap();
}

fn world(name: &str) -> GameLogic {
    let mut world = GameLogic::new();
    world.add_player(Player::new(1, Team::USA, name, false).with_script_identity(name));
    world
}

fn integer(value: i32) -> Parameter {
    Parameter::with_int(ParameterType::Int, value)
}

#[test]
fn rank_and_experience_actions_use_the_receiving_world() {
    let mut first = world("First");
    let mut peer = world("Peer");
    run(
        &mut first,
        0,
        vec![
            action(
                ScriptActionType::PlayerSetRanklevellimit,
                None,
                Some(integer(2)),
            ),
            action(
                ScriptActionType::PlayerSetRanklevel,
                Some(THIS_PLAYER),
                Some(integer(5)),
            ),
        ],
    );
    run(
        &mut peer,
        0,
        vec![
            action(
                ScriptActionType::PlayerSetRanklevellimit,
                None,
                Some(integer(4)),
            ),
            action(
                ScriptActionType::PlayerSetRanklevel,
                Some(THIS_PLAYER),
                Some(integer(5)),
            ),
        ],
    );
    assert_eq!(first.get_player(1).unwrap().rank_level, 2);
    assert_eq!(peer.get_player(1).unwrap().rank_level, 4);

    run(
        &mut first,
        0,
        vec![
            action(
                ScriptActionType::PlayerAddRanklevel,
                Some(THIS_PLAYER),
                Some(integer(-1)),
            ),
            action(
                ScriptActionType::PlayerAffectReceivingExperience,
                Some(THIS_PLAYER),
                Some(Parameter::with_real(ParameterType::Real, 2.0)),
            ),
            action(
                ScriptActionType::PlayerAddSkillpoints,
                Some(THIS_PLAYER),
                Some(integer(25)),
            ),
        ],
    );
    assert_eq!(first.get_player(1).unwrap().rank_level, 1);
    assert_eq!(first.get_player(1).unwrap().skill_points, 50);
    assert_eq!(peer.get_player(1).unwrap().rank_level, 4);
    assert_eq!(peer.get_player(1).unwrap().skill_points_modifier, 1.0);

    run(
        &mut first,
        1,
        vec![action(
            ScriptActionType::PlayerSetRanklevel,
            Some(THIS_PLAYER),
            Some(integer(5)),
        )],
    );
    assert_eq!(first.get_player(1).unwrap().rank_level, 1);
    peer.reset();
    assert_eq!(first.rank_level_limit(), 2);
    assert_eq!(first.get_player(1).unwrap().skill_points, 50);
}

#[test]
fn standalone_rank_action_cannot_leak_into_a_later_world() {
    // Standalone Core dispatch used to leave a request in TLS. An unrelated
    // Main action then drained that request into whichever world ran next.
    let engine = ScriptEngine::new().unwrap();
    let context = RefCell::new(ScriptContext::at_frame(0));
    let old_core_limit = gamelogic::helpers::TheGameLogic::get_rank_level_limit();
    ScriptActionDispatcher::new(&engine, &context)
        .execute_action(&action(
            ScriptActionType::PlayerSetRanklevellimit,
            None,
            Some(integer(3)),
        ))
        .unwrap();
    gamelogic::helpers::TheGameLogic::set_rank_level_limit(old_core_limit);

    let mut recipient = world("Recipient");
    run(
        &mut recipient,
        0,
        vec![action(
            ScriptActionType::PlayerEnableUnitConstruction,
            Some(THIS_PLAYER),
            None,
        )],
    );
    assert_eq!(recipient.rank_level_limit(), 1000);
}
