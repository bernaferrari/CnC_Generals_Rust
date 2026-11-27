use gamelogic::player::GameDifficulty;
use gamelogic::scripting::engine::ScriptEngine;
use generals_main::game_logic::{GameLogic, Player, Team};
use generals_sim::scripting::program::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};

fn difficulty_scripts() -> Box<ScriptList> {
    let mut list = ScriptList::new();
    for (name, easy, normal, hard) in [
        ("EasyBranch", true, false, false),
        ("NormalBranch", false, true, false),
        ("HardBranch", false, false, true),
    ] {
        let mut action = ScriptAction::new(ScriptActionType::SetFlag);
        action
            .add_parameter(Parameter::with_string(ParameterType::Flag, name.into()))
            .unwrap();
        action
            .add_parameter(Parameter::with_int(ParameterType::Boolean, 1))
            .unwrap();
        let mut condition = OrCondition::new();
        condition
            .set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));

        let mut script = Script::new();
        script.script_name = name.into();
        script.condition = Some(Box::new(condition));
        script.action = Some(Box::new(action));
        script.easy = easy;
        script.normal = normal;
        script.hard = hard;
        list.append_script(Box::new(script));
    }
    Box::new(list)
}

fn run(world: &mut GameLogic, engine: &ScriptEngine, frame: u32) -> [bool; 3] {
    world.set_current_frame(u64::from(frame));
    for flag in ["EasyBranch", "NormalBranch", "HardBranch"] {
        engine.set_flag(flag, false).unwrap();
    }
    world.update_local_script_engine_with_owner(engine).unwrap();
    ["EasyBranch", "NormalBranch", "HardBranch"].map(|flag| engine.get_flag(flag).unwrap().value)
}

#[test]
fn ordinary_scripts_observe_ai_and_match_difficulty_on_each_receiving_world() {
    let mut source = GameLogic::new();
    let mut peer = GameLogic::new();
    source
        .add_player(Player::new(1, Team::USA, "Commander", false).with_script_identity("SameSide"));
    peer.add_player(Player::new(1, Team::USA, "Commander", true).with_script_identity("SameSide"));
    source.add_ai_opponent(1, Team::USA, generals_game_domain::AIDifficulty::Hard);

    let mut source_engine = ScriptEngine::new().unwrap();
    let mut peer_engine = ScriptEngine::new().unwrap();
    source_engine.set_global_difficulty(GameDifficulty::Easy);
    peer_engine.set_global_difficulty(GameDifficulty::Easy);
    source_engine
        .set_script_list_for_player(1, Some(difficulty_scripts()))
        .unwrap();
    peer_engine
        .set_script_list_for_player(1, Some(difficulty_scripts()))
        .unwrap();

    assert_eq!(run(&mut source, &source_engine, 1), [false, false, true]);
    assert_eq!(run(&mut peer, &peer_engine, 1), [true, false, false]);
    source.set_ai_difficulty(1, generals_game_domain::AIDifficulty::Easy);
    peer_engine.set_global_difficulty(GameDifficulty::Hard);
    assert_eq!(run(&mut source, &source_engine, 2), [true, false, false]);
    assert_eq!(run(&mut peer, &peer_engine, 2), [false, false, true]);

    // A side list can run with no installed player, such as an on-map
    // script in a world that has not admitted its roster yet.
    let mut world = GameLogic::new();
    let mut engine = ScriptEngine::new().unwrap();
    engine.set_global_difficulty(GameDifficulty::Hard);
    engine
        .set_script_list_for_player(0, Some(difficulty_scripts()))
        .unwrap();
    assert_eq!(run(&mut world, &engine, 1), [false, false, true]);
}
