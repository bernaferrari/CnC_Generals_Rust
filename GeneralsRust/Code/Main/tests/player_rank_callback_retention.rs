//! The transitional Core player still emits rank science edges for scripts.
//! This regression prevents the Main ownership migration from dropping them.
use game_engine::common::name_key_generator::NameKeyGenerator;
use gamelogic::player::{Player as CorePlayer, PlayerList, player_list};
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::ScriptEngine;
use gamelogic::system::rank_info::{
    RankDefinition, RankDefinitionMode, RankInfoStore, init_global_rank_info_store,
    the_rank_info_store_mut,
};
use generals_main::game_logic::{GameLogic, Player, Team};
use std::sync::{Arc, RwLock};

#[test]
fn rank_action_retains_science_callback_on_its_receiving_interpreter() {
    // This test is its own integration executable: its Core registries cannot
    // interfere with other test executables in the same cargo invocation.
    *game_engine::common::ini::get_rank_info_store_mut() =
        game_engine::common::ini::RankInfoStore::new();
    init_global_rank_info_store();
    let science = NameKeyGenerator::name_to_key("SCIENCE_RankOwnerWitness") as i32;
    {
        let mut ranks = the_rank_info_store_mut().unwrap();
        *ranks = RankInfoStore::new();
        ranks
            .apply_rank_definition(RankDefinition::new(1, RankDefinitionMode::Create))
            .unwrap();
        let mut rank_two = RankDefinition::new(2, RankDefinitionMode::Create);
        rank_two.sciences_granted.push(science);
        ranks.apply_rank_definition(rank_two).unwrap();
    }
    gamelogic::helpers::TheGameLogic::set_rank_level_limit(1000);

    let mut core_player = CorePlayer::new(0);
    core_player.set_player_name_key(NameKeyGenerator::name_to_key("RankOwner"));
    {
        let handle = player_list();
        let mut players = handle.write().unwrap();
        *players = PlayerList::new();
        players.add_player(Arc::new(RwLock::new(core_player)));
    }

    let mut world = GameLogic::new();
    world.add_player(
        Player::new(0, Team::USA, "RankOwner", false).with_script_identity("RankOwner"),
    );
    let mut action = ScriptAction::new(ScriptActionType::PlayerSetRanklevel);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "RankOwner".into(),
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
    let engine = ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(scripts)))
        .unwrap();

    world
        .update_local_script_engine_with_owner(&engine)
        .unwrap();

    assert_eq!(world.get_player(0).unwrap().rank_level, 2);
    assert!(engine.is_science_acquired(0, science, true));
    assert!(!engine.is_science_acquired(0, science, true));
}
