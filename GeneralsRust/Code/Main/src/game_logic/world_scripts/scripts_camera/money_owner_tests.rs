//! CPP ScriptActions3978–4020: cash changes synchronously in the driving player.
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use gamelogic::scripting::core::{
    LOCAL_PLAYER, Parameter, ParameterType, ScriptAction, ScriptActionType,
};
use gamelogic::scripting::engine::ScriptEngine;

fn world(cash: u32) -> GameLogic {
    let mut world = GameLogic::new();
    let mut player = Player::new(1, Team::USA, "DisplayOwner", true);
    player.map_side.map_player_name = "AuthoredCashOwner".into();
    player.resources.supplies = cash;
    world.add_player(player);
    world
}
fn execute(world: &mut GameLogic, kind: ScriptActionType, player: &str, amount: i32) {
    let engine = ScriptEngine::new().unwrap();
    let mut action = ScriptAction::new(kind);
    action
        .add_parameter(Parameter::with_string(ParameterType::Side, player.into()))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, amount))
        .unwrap();
    engine.friend_execute_action_with_driver(
        &action,
        None,
        gamelogic::scripting::executor::ScriptContext::at_frame(world.frame),
        &mut HostScriptExecutionDriver::with_interpreter(world, &engine),
    );
}
fn cash(world: &GameLogic) -> u32 {
    world.get_player(1).unwrap().effective_supplies()
}

#[test]
fn money_actions_mutate_canonical_supplies_immediately_and_keep_peer_isolated() {
    let mut owner = world(100);
    let mut peer = world(700);
    execute(
        &mut owner,
        ScriptActionType::PlayerSetMoney,
        "AuthoredCashOwner",
        10_000,
    );
    assert_eq!(cash(&owner), 10_000);
    assert_eq!(cash(&peer), 700);
    execute(
        &mut owner,
        ScriptActionType::PlayerGiveMoney,
        LOCAL_PLAYER,
        -500,
    );
    assert_eq!(
        cash(&owner),
        9_500,
        "local token uses this world's real player"
    );
    execute(
        &mut owner,
        ScriptActionType::PlayerGiveMoney,
        "AbsentPlayer",
        900,
    );
    execute(
        &mut owner,
        ScriptActionType::PlayerSetMoney,
        "DisplayOwner",
        5,
    );
    assert_eq!(
        cash(&owner),
        9_500,
        "literal authored names do not match display aliases"
    );
    execute(
        &mut peer,
        ScriptActionType::PlayerSetMoney,
        LOCAL_PLAYER,
        400,
    );
    assert_eq!(cash(&peer), 400);
    assert_eq!(cash(&owner), 9_500);
    peer.reset();
    assert_eq!(cash(&owner), 9_500);
}

#[test]
fn money_withdraw_deposit_boundaries_and_actual_snapshot_keep_the_owner_balance() {
    let mut owner = world(100);
    execute(
        &mut owner,
        ScriptActionType::PlayerGiveMoney,
        LOCAL_PLAYER,
        i32::MIN,
    );
    assert_eq!(
        cash(&owner),
        0,
        "withdraw is capped at available cash without signed overflow"
    );
    execute(
        &mut owner,
        ScriptActionType::PlayerGiveMoney,
        LOCAL_PLAYER,
        250,
    );
    execute(
        &mut owner,
        ScriptActionType::PlayerGiveMoney,
        LOCAL_PLAYER,
        0,
    );
    assert_eq!(cash(&owner), 250);
    execute(
        &mut owner,
        ScriptActionType::PlayerSetMoney,
        LOCAL_PLAYER,
        -30,
    );
    assert_eq!(
        cash(&owner),
        0,
        "set withdraws first and negative deposit has no positive balance"
    );
    execute(
        &mut owner,
        ScriptActionType::PlayerSetMoney,
        LOCAL_PLAYER,
        1234,
    );
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&owner).unwrap();
    let mut restored = GameLogic::new();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert_eq!(cash(&restored), 1234);
    execute(
        &mut restored,
        ScriptActionType::PlayerGiveMoney,
        LOCAL_PLAYER,
        -34,
    );
    assert_eq!(cash(&restored), 1200);
    assert_eq!(cash(&owner), 1234);
}
