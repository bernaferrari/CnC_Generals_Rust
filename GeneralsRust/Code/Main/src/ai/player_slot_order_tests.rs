//! PlayerList.cpp:221-229 and AISkirmishPlayer.cpp:473-515 visit numeric slots.
use super::*;

fn roster_with_first_enemy(first: u32, insertion: [u32; 3]) -> GameLogic {
    // Realize both legal HashMap orders explicitly, so OLD fails reliably rather
    // than only when the process happens to randomize the buckets unfavorably.
    for _ in 0..256 {
        let mut world = GameLogic::new();
        for id in insertion {
            let team = match id {
                1 => Team::USA,
                2 => Team::China,
                3 => Team::GLA,
                _ => unreachable!(),
            };
            // Distinct lobby teams: every slot is an enemy of the others.
            let mut player = Player::new(id, team, "Numeric slot", false);
            player.alliance_team = id as i32;
            world.add_player(player);
        }
        if world.get_players().keys().copied().find(|id| *id != 1) == Some(first) {
            return world;
        }
    }
    panic!("could not realize fixture HashMap order");
}

fn enemy_tie_world(first: u32, insertion: [u32; 3]) -> GameLogic {
    let mut world = roster_with_first_enemy(first, insertion);
    for (name, kinds) in [
        ("SlotBarracks", vec![KindOf::Structure, KindOf::FSBarracks]),
        ("SlotInfantry", vec![KindOf::Infantry]),
    ] {
        let mut template = ThingTemplate::new(name);
        for kind in kinds {
            template.add_kind_of(kind);
        }
        world.templates.insert(name.into(), template);
    }
    for (id, x) in [(2, -100.0), (3, 100.0)] {
        world
            .create_object_for_player("SlotBarracks", id, Vec3::new(x, 0.0, 50.0))
            .unwrap();
        world
            .create_object_for_player("SlotInfantry", id, Vec3::new(x, 0.0, 80.0))
            .unwrap();
    }
    let mut ai = AIPlayer::new_with_team_factory(
        1,
        Team::USA,
        AIDifficulty::Medium,
        world.team_factory.clone(),
    );
    ai.skirmish_new_map_applied = true;
    ai.enemy_check_time = -5.0;
    ai.next_building_time = f32::MAX;
    ai.next_team_time = f32::MAX;
    ai.next_team_queue_time = f32::MAX;
    world.ai_manager.ai_players.insert(1, ai);
    world
}

#[test]
fn fixed_tick_enemy_ties_choose_lower_slot_in_independent_worlds() {
    let mut first = enemy_tie_world(3, [1, 2, 3]);
    let mut second = enemy_tie_world(2, [3, 2, 1]);
    second.update_with_dt_budget(1.0 / 30.0, 1);
    first.update_with_dt_budget(1.0 / 30.0, 1);
    for world in [&first, &second] {
        assert_eq!(world.frame, 1);
        assert_eq!(world.ai_manager.ai_players[&1].enemy_player_id, Some(2));
        assert_eq!(world.ai_manager.ai_players[&1].enemy_check_time, 0.0);
    }
}

#[test]
fn fixed_tick_scaffold_admission_follows_numeric_ai_slots() {
    let mut world = GameLogic::new();
    let mut dozer = ThingTemplate::new("SlotDozer");
    dozer
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Worker)
        .add_kind_of(KindOf::Dozer);
    world.templates.insert("SlotDozer".into(), dozer);
    let mut plan = ThingTemplate::new("SlotPlan");
    plan.add_kind_of(KindOf::Structure).set_cost(100, 0);
    plan.build_time = 10.0;
    world.templates.insert("SlotPlan".into(), plan);
    // Opposite registration order: actual scaffold ObjectIds must still reflect 1,3.
    for (id, team, x) in [(3, Team::China, 180.0), (1, Team::USA, -180.0)] {
        let mut player = Player::new(id, team, "Builder slot", false);
        player.resources.supplies = 1_000;
        world.add_player(player);
        world
            .create_object_for_player("SlotDozer", id, Vec3::new(x, 0.0, x))
            .unwrap();
        let mut ai = AIPlayer::new_with_team_factory(
            id,
            team,
            AIDifficulty::Medium,
            world.team_factory.clone(),
        );
        ai.skirmish_new_map_applied = true;
        ai.next_team_time = f32::MAX;
        ai.next_team_queue_time = f32::MAX;
        ai.add_building("SlotPlan", Vec3::new(x + 16.0, 0.0, x), 1);
        world.ai_manager.ai_players.insert(id, ai);
    }
    // The real construction admission must never ask a foreign Core runtime
    // for wall/layer data while Main is stepping its own service context.
    let foreign = gamelogic::system::engine_stores::new_for_world();
    let held = foreign.ai().write().unwrap();
    gamelogic::system::engine_stores::with_active_stores(&foreign, || {
        world.update_with_dt_budget(1.0 / 30.0, 1);
    });
    drop(held);
    assert_eq!(world.frame, 1);
    let low = world.ai_manager.ai_players[&1].building_queue[0]
        .object_id
        .expect("slot1 scaffold");
    let high = world.ai_manager.ai_players[&3].building_queue[0]
        .object_id
        .expect("slot3 scaffold");
    assert!(
        low.0 < high.0,
        "numeric AI order must drive real admissions"
    );
    assert_eq!(world.host_object(low).unwrap().team, Team::USA);
    assert_eq!(world.host_object(high).unwrap().team, Team::China);
    assert_eq!(world.get_player(1).unwrap().effective_supplies(), 900);
    assert_eq!(world.get_player(3).unwrap().effective_supplies(), 900);
}
