use super::*;

fn world() -> GameLogic {
    let mut world = GameLogic::new();
    world.add_player(
        crate::game_logic::Player::new(1, crate::game_logic::Team::USA, "Human", true)
            .with_script_identity("AuthoredPlayer"),
    );
    world
}

fn queue_permanent_reveal(world: &GameLogic, player: u32) {
    world
        .world_services
        .shroud()
        .lock()
        .unwrap()
        .reveal_map_for_player_permanently(player)
        .unwrap();
}

#[test]
fn named_map_shroud_mutates_only_the_receiving_world() {
    let mut owner = world();
    let peer = world();
    queue_permanent_reveal(&owner, 1);
    queue_permanent_reveal(&peer, 1);

    {
        let mut driver = HostScriptExecutionDriver::new(&mut owner);
        assert!(
            driver
                .map_visibility_all(
                    generals_sim::scripting::world::ScriptMapVisibilityOperation::Shroud,
                    "AuthoredPlayer",
                    None,
                )
                .is_some()
        );
    }

    assert!(
        owner
            .world_services
            .shroud()
            .lock()
            .unwrap()
            .snapshot_state()
            .pending_permanent_reveal_players
            .is_empty()
    );
    assert_eq!(
        peer.world_services
            .shroud()
            .lock()
            .unwrap()
            .snapshot_state()
            .pending_permanent_reveal_players,
        vec![1]
    );
}

#[test]
fn unnamed_map_shroud_targets_human_players_only() {
    let mut owner = world();
    owner.add_player(crate::game_logic::Player::new(
        2,
        crate::game_logic::Team::China,
        "AI",
        false,
    ));
    queue_permanent_reveal(&owner, 1);
    queue_permanent_reveal(&owner, 2);

    {
        let mut driver = HostScriptExecutionDriver::new(&mut owner);
        assert!(
            driver
                .map_visibility_all(
                    generals_sim::scripting::world::ScriptMapVisibilityOperation::Shroud,
                    "",
                    None,
                )
                .is_some()
        );
    }

    let pending = owner
        .world_services
        .shroud()
        .lock()
        .unwrap()
        .snapshot_state()
        .pending_permanent_reveal_players;
    assert_eq!(pending, vec![2]);
}

#[test]
fn map_reveal_operations_mutate_only_the_receiving_world() {
    use generals_sim::scripting::world::ScriptMapVisibilityOperation as Op;

    for operation in [Op::Reveal, Op::RevealPermanently, Op::UndoPermanentReveal] {
        let mut owner = world();
        let peer = world();
        if matches!(operation, Op::UndoPermanentReveal) {
            queue_permanent_reveal(&owner, 1);
            queue_permanent_reveal(&peer, 1);
        }

        assert!(
            HostScriptExecutionDriver::new(&mut owner)
                .map_visibility_all(operation, "AuthoredPlayer", None)
                .is_some()
        );

        let owner_state = owner
            .world_services
            .shroud()
            .lock()
            .unwrap()
            .snapshot_state();
        let peer_state = peer
            .world_services
            .shroud()
            .lock()
            .unwrap()
            .snapshot_state();
        match operation {
            Op::Reveal => {
                assert_eq!(owner_state.pending_full_reveal_players, vec![1]);
                assert!(peer_state.pending_full_reveal_players.is_empty());
            }
            Op::RevealPermanently => {
                assert_eq!(owner_state.pending_permanent_reveal_players, vec![1]);
                assert!(peer_state.pending_permanent_reveal_players.is_empty());
            }
            Op::UndoPermanentReveal => {
                assert!(owner_state.pending_permanent_reveal_players.is_empty());
                assert_eq!(peer_state.pending_permanent_reveal_players, vec![1]);
            }
            Op::Shroud => unreachable!(),
        }
    }
}

#[test]
fn map_shroud_resolves_script_player_aliases_in_the_receiving_world() {
    use generals_sim::scripting::program::{
        LOCAL_PLAYER, THE_PLAYER, THIS_PLAYER, THIS_PLAYER_ENEMY,
    };

    let mut owner = world();
    owner.add_player(
        crate::game_logic::Player::new(2, crate::game_logic::Team::China, "Current AI", false)
            .with_script_identity("CurrentAI"),
    );
    owner.add_player(
        crate::game_logic::Player::new(3, crate::game_logic::Team::GLA, "Enemy", true)
            .with_script_identity("Enemy"),
    );
    owner.add_player(
        crate::game_logic::Player::new(4, crate::game_logic::Team::USA, "ThePlayer", true)
            .with_script_identity(THE_PLAYER),
    );
    owner.ai_manager.add_ai_player(
        2,
        crate::game_logic::Team::China,
        crate::ai::AIDifficulty::Medium,
    );
    owner
        .ai_manager
        .ai_players
        .get_mut(&2)
        .unwrap()
        .enemy_player_id = Some(3);
    owner.admit_challenge_session(true);

    for id in [1, 2, 3, 4] {
        queue_permanent_reveal(&owner, id);
    }
    assert!(
        HostScriptExecutionDriver::new(&mut owner)
            .map_visibility_all(
                generals_sim::scripting::world::ScriptMapVisibilityOperation::Shroud,
                LOCAL_PLAYER,
                None,
            )
            .is_some()
    );
    assert_eq!(pending_reveals(&owner), vec![2, 3, 4]);
    assert!(
        HostScriptExecutionDriver::new(&mut owner)
            .map_visibility_all(
                generals_sim::scripting::world::ScriptMapVisibilityOperation::Shroud,
                THIS_PLAYER,
                Some("CurrentAI"),
            )
            .is_some()
    );
    assert_eq!(pending_reveals(&owner), vec![3, 4]);
    assert!(
        HostScriptExecutionDriver::new(&mut owner)
            .map_visibility_all(
                generals_sim::scripting::world::ScriptMapVisibilityOperation::Shroud,
                THIS_PLAYER_ENEMY,
                Some("CurrentAI"),
            )
            .is_some()
    );
    assert_eq!(pending_reveals(&owner), vec![4]);
    queue_permanent_reveal(&owner, 1);
    assert!(
        HostScriptExecutionDriver::new(&mut owner)
            .map_visibility_all(
                generals_sim::scripting::world::ScriptMapVisibilityOperation::Shroud,
                THE_PLAYER,
                Some("CurrentAI"),
            )
            .is_some()
    );

    assert_eq!(pending_reveals(&owner), vec![4]);
}

fn pending_reveals(world: &GameLogic) -> Vec<u32> {
    world
        .world_services
        .shroud()
        .lock()
        .unwrap()
        .snapshot_state()
        .pending_permanent_reveal_players
}
