//! A disabled spawn-weapon site idles each receiving child before disabling it.
use generals_main::game_logic::{GameLogic, Player, Team};
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
            "
Object SpawnDisabledIdleSite
 KindOf = STRUCTURE SELECTABLE SPAWNS_ARE_THE_WEAPONS
 Body = ActiveBody SiteBody
  MaxHealth = 500
  InitialHealth = 500
 End
 Behavior = DefaultProductionExitUpdate Exit
 End
 Behavior = SpawnBehavior Spawn
  SpawnNumber = 1
  SpawnTemplateName = SpawnDisabledIdleChild
  OneShot = Yes
 End
End
Object SpawnDisabledIdleChild
 KindOf = VEHICLE DOZER SELECTABLE
 Locomotor = SET_NORMAL AmericaVehicleDozerLocomotor
 Body = ActiveBody ChildBody
  MaxHealth = 200
  InitialHealth = 200
 End
 Behavior = DozerAIUpdate ChildAI
 End
End
".into(),
        )])
        .unwrap();
    world.add_player(Player::new(1, Team::USA, "Owner", true));
    let site = world
        .create_object_for_player("SpawnDisabledIdleSite", 1, Vec3::ZERO)
        .unwrap();
    for _ in 0..32 {
        world.update();
    }
    let child = world
        .host_objects()
        .registered_values()
        .find(|object| object.producer_id == Some(site))
        .expect("authored SpawnBehavior child")
        .id;
    // Only the parent is inside the field. The child is reached by the callback.
    assert!(world.set_registered_object_position(child, Vec3::X * 2_000.0));
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
    assert!(owner.apply_emp_pulse_disable_field_at(1, Vec3::ZERO, None));

    let owner_child = owner.host_object(child).unwrap();
    assert!(owner_child.status.disabled_emp);
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
    assert!(!peer_child.status.disabled_emp);
    assert_eq!(peer_child.last_command_source, 71);
}
