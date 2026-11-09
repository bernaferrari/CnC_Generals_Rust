//! The transitional Core player still emits rank science edges for scripts.
//! This regression prevents the Main ownership migration from dropping them.
use game_engine::common::name_key_generator::NameKeyGenerator;
use gamelogic::player::{Player as CorePlayer, PlayerList};
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList, LOCAL_PLAYER,
};
use gamelogic::scripting::engine::ScriptEngine;
use gamelogic::system::rank_info::{RankDefinition, RankDefinitionMode, RankInfoStore};
use generals_main::game_logic::{GameLogic, Player, Team};
use std::sync::{Arc, RwLock};

#[test]
fn rank_action_retains_science_callback_on_its_receiving_interpreter() {
    let mut world = GameLogic::new();
    let science = NameKeyGenerator::name_to_key("SCIENCE_RankOwnerWitness") as i32;
    {
        let mut ranks = world.core_rank_info_store().write().unwrap();
        *ranks = RankInfoStore::new();
        ranks
            .apply_rank_definition(RankDefinition::new(1, RankDefinitionMode::Create))
            .unwrap();
        let mut rank_two = RankDefinition::new(2, RankDefinitionMode::Create);
        rank_two.sciences_granted.push(science);
        ranks.apply_rank_definition(rank_two).unwrap();
    }

    let mut core_player = CorePlayer::new(0);
    core_player.set_player_name_key(NameKeyGenerator::name_to_key("RankOwner"));
    {
        let handle = world.core_player_roster();
        let mut players = handle.write().unwrap();
        *players = PlayerList::new();
        players.add_player(Arc::new(RwLock::new(core_player)));
    }

    world.add_player(
        Player::new(0, Team::USA, "RankOwner", false).with_script_identity("RankOwner"),
    );
    assert!(world.select_local_player(Some(0)));
    let mut action = ScriptAction::new(ScriptActionType::PlayerSetRanklevel);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            LOCAL_PLAYER.into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, 2))
        .unwrap();
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "RankScienceCallback".into();
    script.is_one_shot = true;
    script.condition = Some(Box::new(condition));
    script.action = Some(Box::new(action));
    let mut scripts = ScriptList::new();
    scripts.append_script(Box::new(script));
    let foreign = gamelogic::system::engine_stores::new_world_services();
    let mut foreign_player = CorePlayer::new(0);
    foreign_player.set_player_name_key(NameKeyGenerator::name_to_key("RankOwner"));
    let foreign_player = Arc::new(RwLock::new(foreign_player));
    foreign.players().write().unwrap().add_player(Arc::clone(&foreign_player));
    let engine = ScriptEngine::new_with_services(&foreign).unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(scripts)))
        .unwrap();

    world
        .update_local_script_engine_with_owner(&engine)
        .unwrap();

    assert_eq!(world.get_player(0).unwrap().rank_level, 2);
    assert!(engine.is_science_acquired(0, science, true));
    assert_eq!(foreign_player.read().unwrap().get_rank_level(), 1);
    assert!(!engine.is_science_acquired(0, science, true));
}
