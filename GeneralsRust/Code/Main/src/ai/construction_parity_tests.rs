use super::{AIDifficulty, AIPlayer};
use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
use glam::Vec3;

#[test]
fn automatic_power_override_preserves_priority_and_scaffold_rules() {
    // AISkirmishPlayer.cpp:183-236: ordinary automatic pads cannot select
    // themselves, but an automatic FS_POWER plan overrides a priority plan
    // until another power plant is under construction.
    let mut logic = GameLogic::new();
    let mut player = Player::new(1, Team::USA, "AI", false);
    player.resources.supplies = 10_000;
    logic.add_player(player);
    for (name, power) in [("PriorityPlan", false), ("PowerPlan", true)] {
        let mut template = ThingTemplate::new(name);
        template.add_kind_of(KindOf::Structure).set_cost(100, 0);
        if power {
            template.add_kind_of(KindOf::FSPower);
        }
        logic.templates.insert(name.into(), template);
    }
    let mut ai = AIPlayer::new(1, Team::USA, AIDifficulty::Medium);
    ai.add_building("PriorityPlan", Vec3::new(100.0, 0.0, 100.0), 1);
    ai.add_layout_building("PowerPlan", Vec3::new(200.0, 0.0, 100.0), 1);
    assert_eq!(
        ai.select_priority_or_power_build(&logic, 0.0, false),
        Some(1)
    );

    ai.building_queue[1].automatic_build = false;
    assert_eq!(
        ai.select_priority_or_power_build(&logic, 0.0, false),
        Some(0)
    );
    assert_eq!(
        ai.select_priority_or_power_build(&logic, 0.0, true),
        Some(1)
    );

    let position = Vec3::new(300.0, 0.0, 100.0);
    let scaffold = logic
        .create_object_under_construction("PowerPlan", Team::USA, position)
        .expect("power scaffold");
    ai.add_layout_building("PowerPlan", position, 1);
    ai.building_queue[2].object_id = Some(scaffold);
    ai.building_queue[1].automatic_build = true;
    assert_eq!(
        ai.select_priority_or_power_build(&logic, 0.0, true),
        Some(0)
    );
}

#[test]
fn placement_retains_cpp_delayed_second_edge_result() {
    // AIPlayer.cpp:554-561 checks the bottom result only after the row loop.
    // Success at its first bottom sample is overwritten by the next top test;
    // success at its final bottom sample survives. The right edge is identical.
    for (query_to_accept, survives) in [(0, true), (1, false), (3, true), (5, false), (7, true)] {
        let queries = std::cell::Cell::new(0);
        let found = AIPlayer::wiggle_find_legal_build_position(Vec3::ZERO, |_| {
            let index = queries.get();
            queries.set(index + 1);
            index == query_to_accept
        });
        assert_eq!(found.is_some(), survives, "query {query_to_accept}");
        if !survives {
            assert_eq!(queries.get(), 3720, "search must exhaust all rings");
        }
    }
}
