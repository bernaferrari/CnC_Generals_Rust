//! AISkirmishPlayer.cpp:473-515 reads live peer targets in numeric slot order.
use super::*;

fn world_with_enemy_objects(rows: &[(u32, Team, Vec3, bool)]) -> GameLogic {
    let mut world = GameLogic::new();
    for (name, kinds) in [
        ("PeerBarracks", vec![KindOf::Structure, KindOf::FSBarracks]),
        ("PeerInfantry", vec![KindOf::Infantry]),
    ] {
        let mut template = ThingTemplate::new(name);
        for kind in kinds {
            template.add_kind_of(kind);
        }
        world.templates.insert(name.into(), template);
    }
    for &(id, team, center, units) in rows {
        world.add_player(Player::new(id, team, "Peer slot", false));
        world
            .create_object_for_player("PeerBarracks", id, center)
            .unwrap();
        if units {
            world
                .create_object_for_player("PeerInfantry", id, center)
                .unwrap();
        }
    }
    world
}

fn add_assessing_ai(world: &mut GameLogic, id: u32, center: Vec3) {
    let team = world.get_player(id).unwrap().team;
    let mut ai =
        AIPlayer::new_with_team_factory(id, team, AIDifficulty::Medium, world.team_factory.clone());
    ai.base_center = center;
    ai.skirmish_new_map_applied = true;
    ai.enemy_check_time = -5.0;
    ai.next_building_time = f32::MAX;
    ai.next_team_time = f32::MAX;
    ai.next_team_queue_time = f32::MAX;
    world.ai_manager.ai_players.insert(id, ai);
}

fn live_peer_world() -> GameLogic {
    let centers = [Vec3::new(-100.0, 0.0, 50.0), Vec3::new(100.0, 0.0, 50.0)];
    let mut world = world_with_enemy_objects(&[
        (1, Team::USA, centers[0], true),
        (2, Team::China, centers[1], true),
        (3, Team::GLA, Vec3::new(0.0, 0.0, 50.0), true),
    ]);
    // Registration order differs from the C++ player-slot execution order.
    add_assessing_ai(&mut world, 2, centers[1]);
    add_assessing_ai(&mut world, 1, centers[0]);
    world
}

#[test]
fn fixed_tick_later_ai_sees_earlier_slots_new_enemy_in_both_worlds() {
    let mut first = live_peer_world();
    let mut second = live_peer_world();
    second.update_with_dt_budget(1.0 / 30.0, 1);
    first.update_with_dt_budget(1.0 / 30.0, 1);
    for world in [&first, &second] {
        assert_eq!(world.frame, 1);
        assert_eq!(world.ai_manager.ai_players[&1].enemy_player_id, Some(3));
        // Slot1 has just selected slot3: its 500^2 penalty makes slot1 preferable.
        assert_eq!(world.ai_manager.ai_players[&2].enemy_player_id, Some(1));
        assert_eq!(world.ai_manager.ai_players[&2].enemy_check_time, 0.0);
    }
}

#[test]
fn fixed_tick_existing_self_target_participates_when_both_enemies_are_crippled() {
    let mut world = world_with_enemy_objects(&[
        (1, Team::USA, Vec3::ZERO, true),
        (2, Team::China, Vec3::new(-100.0, 0.0, 50.0), false),
        (3, Team::GLA, Vec3::new(100.0, 0.0, 50.0), false),
    ]);
    add_assessing_ai(&mut world, 1, Vec3::ZERO);
    world
        .ai_manager
        .ai_players
        .get_mut(&1)
        .unwrap()
        .enemy_player_id = Some(2);
    world.update_with_dt_budget(1.0 / 30.0, 1);
    assert_eq!(world.frame, 1);
    // C++ skips peer k==candidate i, not the assessing AI itself. Both enemies
    // get HUGE_DIST^2/2, then our existing slot2 target receives the penalty.
    assert_eq!(world.ai_manager.ai_players[&1].enemy_player_id, Some(3));
}

#[test]
fn fixed_tick_peer_subtraction_clamps_before_later_slot_penalty() {
    let center = Vec3::new(-200.0, 0.0, -200.0);
    let mut world = world_with_enemy_objects(&[
        (1, Team::China, center, true),
        (2, Team::China, center, true),
        (3, Team::China, center, true),
        (4, Team::USA, center, true),
        (5, Team::GLA, Vec3::new(200.0, 0.0, 101.0), true),
    ]);
    for id in [3, 2, 1] {
        add_assessing_ai(&mut world, id, center);
    }
    // Retained peer targets are legal inputs even after relationships change.
    // Inactive peers keep them during this tick, isolating the ordered scoring.
    let low = world.ai_manager.ai_players.get_mut(&1).unwrap();
    low.enemy_player_id = Some(2);
    low.is_active = false;
    let high = world.ai_manager.ai_players.get_mut(&3).unwrap();
    high.enemy_player_id = Some(4);
    high.is_active = false;
    world.update_with_dt_budget(1.0 / 30.0, 1);
    assert_eq!(world.frame, 1);
    // Candidate4: max(0 - 625, 0) + 250000 = 250000.
    // Candidate5: 400^2 + 301^2 - 625 = 249976.
    // Reversing the peer slots would score candidate4 at249375 and select it.
    assert_eq!(world.ai_manager.ai_players[&2].enemy_player_id, Some(5));
    assert_eq!(world.ai_manager.ai_players[&1].enemy_player_id, Some(2));
    assert_eq!(world.ai_manager.ai_players[&3].enemy_player_id, Some(4));
}
