use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Script, ScriptAction, ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::ScriptEngine;
use generals_main::game_logic::{GameLogic, Player, Team};

fn world(color: [f32; 3]) -> GameLogic {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Owner", true));
    assert!(world.admit_radar_terrain_colors(vec![Some(color); 128 * 128], color));
    world
}

#[test]
fn refresh_radar_script_updates_only_its_receiving_world() {
    let mut owner = world([0.1, 0.2, 0.8]);
    let peer = world([0.8, 0.2, 0.1]);
    let owner_generation = owner.radar().terrain_generation();
    let peer_generation = peer.radar().terrain_generation();
    let peer_texture = peer.radar().get_terrain_texture().to_vec();

    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "OwnedRadarRefresh".into();
    script.is_one_shot = true;
    script.delay_evaluation_seconds = 0;
    script.condition = Some(Box::new(condition));
    script.action = Some(Box::new(ScriptAction::new(ScriptActionType::RefreshRadar)));
    let mut scripts = ScriptList::new();
    scripts.append_script(Box::new(script));
    let engine = ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(scripts)))
        .unwrap();
    owner
        .update_local_script_engine_with_owner(&engine)
        .unwrap();

    assert!(owner.radar().terrain_generation() > owner_generation);
    assert_eq!(peer.radar().terrain_generation(), peer_generation);
    assert_eq!(peer.radar().get_terrain_texture(), peer_texture);
}
