//! A disabled spawn-weapon site idles each receiving child before disabling it.
use generals_main::game_logic::{GameLogic, GameMode, Player, Team};
use glam::Vec3;

fn world() -> (
    GameLogic,
    generals_main::game_logic::ObjectId,
    generals_main::game_logic::ObjectId,
) {
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([(
            "SpawnDisabledIdle.ini".into(),
            "Object SpawnDisabledIdleSite\n KindOf = STRUCTURE SELECTABLE SPAWNS_ARE_THE_WEAPONS\n Body = ActiveBody SiteBody\n MaxHealth = 500\n InitialHealth = 500\n End\nEnd\nObject SpawnDisabledIdleChild\n KindOf = VEHICLE DOZER SELECTABLE\n Locomotor = SET_NORMAL AmericaVehicleDozerLocomotor\n Body = ActiveBody ChildBody\n MaxHealth = 200\n InitialHealth = 200\n End\n Behavior = DozerAIUpdate ChildAI\n End\nEnd\n".into(),
        )])
        .unwrap();
    world.start_new_game(GameMode::SinglePlayer);
    world.add_player(Player::new(1, Team::USA, "Owner", true));
    let site = world
        .create_object_for_player("SpawnDisabledIdleSite", 1, Vec3::ZERO)
        .unwrap();
    let child = world
        .create_object_for_player("SpawnDisabledIdleChild", 1, Vec3::new(5.0, 0.0, 0.0))
        .unwrap();
    // This fixture supplies the producer link directly. The authored KindOf
    // is real; original SpawnBehavior roster admission and timing remain a
    // separate ownership obligation.
    assert!(world.host_object(site).unwrap().is_spawns_are_the_weapons());
    world.host_object_mut(child).unwrap().producer_id = Some(site);
    (world, site, child)
}

#[test]
fn disabled_spawn_site_runs_child_ai_idle_in_its_receiving_world_first() {
    let (mut owner, site, child) = world();
    let (mut peer, peer_site, peer_child) = world();
    assert_eq!((site, child), (peer_site, peer_child));

    owner.host_object_mut(child).unwrap().last_command_source = 71;
    peer.host_object_mut(peer_child)
        .unwrap()
        .last_command_source = 71;
    owner
        .host_object_mut(child)
        .unwrap()
        .set_actively_constructing(true);
    peer.host_object_mut(peer_child)
        .unwrap()
        .set_actively_constructing(true);
    {
        let site = owner.host_object_mut(site).unwrap();
        site.status.disabled_hacked = true;
        site.status.disabled_hacked_until_frame = 90;
    }

    // Main's ordinary fixed-step driver reaches propagate_hacked_to_spawn_slaves.
    owner.update_with_dt(1.0 / 30.0);

    let owner_child = owner.host_object(child).unwrap();
    assert!(owner_child.status.disabled_hacked);
    assert_eq!(
        owner_child.last_command_source,
        2, // C++ CMD_FROM_AI, set by the owned derived Idle before disable.
    );
    assert_ne!(
        owner_child.model_condition_bits,
        peer.host_object(peer_child).unwrap().model_condition_bits,
        "DozerAIUpdate::aiDoCommand clears its active-construction model condition"
    );
    let peer_child = peer.host_object(peer_child).unwrap();
    assert!(!peer_child.status.disabled_hacked);
    assert_eq!(peer_child.last_command_source, 71);
}
