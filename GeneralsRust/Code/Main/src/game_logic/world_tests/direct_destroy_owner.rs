//! C++ GameLogic::destroyObject is a synchronous deletion request, not body death.
//! Physical storage remains visible until processDestroyList finishes the frame.
use super::*;

fn world_with_owner() -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    let mut template = ThingTemplate::new("DirectDestroyOwnerVehicle");
    template.add_kind_of(KindOf::Vehicle).set_health(100.0);
    world.templates.insert(template.name.clone(), template);
    let id = world
        .create_object("DirectDestroyOwnerVehicle", Team::USA, Vec3::ZERO)
        .unwrap();
    let owner = world.host_object_mut(id).unwrap();
    owner.health.current = 40.0;
    owner.previous_health = 50.0;
    world.health_events.clear_damage();
    world.health_events.clear_heal();
    (world, id)
}

#[test]
fn direct_destroy_preserves_body_and_does_not_enter_die() {
    let (mut world, id) = world_with_owner();
    world.destroy_object(id);
    let object = world.host_object(id).unwrap();
    assert!(object.status.destroyed);
    assert!(
        !object.status.on_die_started,
        "destroyObject never implicitly calls onDie"
    );
    assert_eq!(
        (object.health.current, object.previous_health),
        (40.0, 50.0)
    );
    assert!(world.health_events.snapshot_damage().is_empty());
    world.destroy_object(id);
    assert_eq!(
        world
            .objects_to_destroy
            .iter()
            .filter(|e| e.id == id)
            .count(),
        1
    );
    world.process_destroy_list();
    assert!(world.host_object(id).is_none());
}

#[test]
fn direct_destroy_clears_goal_and_path_before_physical_removal() {
    let (mut world, id) = world_with_owner();
    let object = world.host_object_mut(id).unwrap();
    object.set_locomotor_goal_position_on_path();
    object.movement.path = vec![Vec3::new(10.0, 0.0, 0.0)];
    world.destroy_object(id);
    let object = world.host_object(id).unwrap();
    assert_eq!(object.locomotor_goal_type, LocoGoalType::None);
    assert!(object.movement.path.is_empty());
    assert_eq!(
        world.host_objects().len(),
        1,
        "lookup lifetime ends at the physical drain"
    );
}

#[test]
fn direct_destroy_final_drain_does_not_invent_death_particles() {
    let (mut world, id) = world_with_owner();
    let before = world.combat_particles.system_count();
    world.destroy_object(id);
    world.process_destroy_list();
    assert_eq!(
        world.combat_particles.system_count(),
        before,
        "direct cleanup has no death FX"
    );
}

#[test]
fn frenzy_deletion_update_preserves_marker_hp_and_emits_no_damage() {
    let mut world = GameLogic::new();
    let id = world
        .spawn_frenzy_invisible_marker(
            Team::China,
            Vec3::ZERO,
            crate::game_logic::host_frenzy::HostFrenzyLevel::One,
        )
        .unwrap();
    let hp = world.host_object(id).unwrap().health.current;
    assert!(hp > 0.0);
    world.health_events.clear_damage();
    world.update_frenzy_invisible_markers();
    assert!(!world.host_object(id).unwrap().status.destroyed);
    world.update_frenzy_invisible_markers();
    let object = world.host_object(id).unwrap();
    assert!(object.status.destroyed);
    assert_eq!(
        object.health.current, hp,
        "DeletionUpdate explicitly destroys, not kills"
    );
    assert!(!object.status.on_die_started);
    assert!(world.health_events.snapshot_damage().is_empty());
    world.process_destroy_list();
    assert!(world.host_object(id).is_none());
}

#[test]
fn direct_destroy_queue_and_reset_do_not_touch_same_id_in_another_world() {
    let (mut first, id) = world_with_owner();
    let (mut second, other) = world_with_owner();
    assert_eq!(id, other);
    second.host_object_mut(other).unwrap().health.current = 73.0;
    first.destroy_object(id);
    first.reset();
    first.process_destroy_list();
    let object = second.host_object(other).unwrap();
    assert_eq!(object.health.current, 73.0);
    assert!(!object.status.destroyed && !object.status.on_die_started);
    second.destroy_object(other);
    second.process_destroy_list();
    assert!(second.host_object(other).is_none());
}

#[test]
fn snapshot_rejects_a_pending_physical_removal_instead_of_losing_its_queue() {
    let (mut world, id) = world_with_owner();
    world.destroy_object(id);
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    assert!(
        builder.create_world_snapshot(&world).is_err(),
        "the removal queue is not serialized"
    );
    world.process_destroy_list();
    let snapshot = builder.create_world_snapshot(&world).unwrap();
    assert!(!snapshot.objects.contains_key(&id));
}

#[test]
fn victory_kill_commits_body_damage_before_owner_death() {
    let (mut world, id) = world_with_owner();
    world.add_player(Player::new(0, Team::USA, "Owner", true));
    world.kill_player_for_victory(0);
    let object = world.host_object(id).unwrap();
    assert_eq!(
        object.health.current, 0.0,
        "Team::killTeam uses Object::kill, not destroyObject"
    );
    assert_eq!(object.previous_health, 40.0);
    assert!(object.status.on_die_started);
    assert!(
        world
            .health_events
            .snapshot_damage()
            .iter()
            .any(|e| e.target == id)
    );
}

#[test]
fn victory_kill_respects_indestructible_body_instead_of_directly_deleting_it() {
    let (mut world, id) = world_with_owner();
    world.add_player(Player::new(0, Team::USA, "Owner", true));
    world.host_object_mut(id).unwrap().indestructible = true;
    world.kill_player_for_victory(0);
    let object = world.host_object(id).unwrap();
    assert_eq!(object.health.current, 40.0);
    assert!(!object.status.destroyed && !object.status.on_die_started);
    world.process_destroy_list();
    assert!(world.host_object(id).is_some());
}

#[test]
fn local_open_contain_direct_delete_cascades_in_list_order_before_removal() {
    let (mut world, child) = world_with_owner();
    let mut template = ThingTemplate::new("DirectDeleteTransport");
    template.add_kind_of(KindOf::Vehicle).set_health(100.0);
    template.contain_module.kind = ContainModuleKind::Transport;
    template.contain_module.slots = Some(3);
    world.templates.insert(template.name.clone(), template);
    let parent = world
        .create_object("DirectDeleteTransport", Team::USA, Vec3::ZERO)
        .unwrap();
    let second = world
        .create_object("DirectDestroyOwnerVehicle", Team::USA, Vec3::ZERO)
        .unwrap();
    for id in [second, child] {
        assert!(world.host_object_mut(parent).unwrap().add_occupant(id));
        world
            .host_object_mut(id)
            .unwrap()
            .set_contained_by(Some(parent));
    }
    world.destroy_object(parent);
    assert_eq!(
        world
            .objects_to_destroy
            .iter()
            .map(|e| e.id)
            .collect::<Vec<_>>(),
        vec![parent, second, child]
    );
    assert!(
        world
            .host_object(parent)
            .unwrap()
            .contained_units()
            .is_empty()
    );
    for id in [second, child] {
        let object = world.host_object(id).unwrap();
        assert!(object.status.destroyed && !object.status.on_die_started);
        assert!(object.health.current > 0.0);
        assert_eq!(object.contained_by, None);
    }
    world.process_destroy_list();
    assert!(world.host_objects().is_empty());
}

#[test]
fn forced_penalty_kill_overrides_scalar_and_keeps_cpp_extra4_ordinal() {
    let (mut world, id) = world_with_owner();
    let object = world.host_object_mut(id).unwrap();
    object.health.current = 80.0;
    object.weapon_bonus_battle_plan_hold_the_line = true;
    let _ = world.apply_owned_kill(
        id,
        crate::game_logic::combat::DamageType::Penalty,
        crate::game_logic::host_usa_pilot::HostDeathType::Extra4,
    );
    let object = world.host_object(id).unwrap();
    assert_eq!((object.health.current, object.previous_health), (0.0, 80.0));
    assert!(object.status.on_die_started);
    assert_eq!(object.status.death_type.ordinal(), 15);
    assert_eq!(
        crate::game_logic::host_usa_pilot::HostDeathType::from_ordinal(15),
        object.status.death_type
    );
    assert_eq!(
        crate::game_logic::host_usa_pilot::HostDeathType::from_store(
            gamelogic::damage::DeathType::Extra4
        ),
        object.status.death_type
    );
    let events = world.health_events.snapshot_damage();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].amount, 80.0);
    assert_eq!(events[0].source, None);
}

#[test]
fn direct_tunnel_delete_remaps_then_directly_deletes_last_shared_pool() {
    let mut world = GameLogic::new();
    world.add_player(Player::new(2, Team::GLA, "Owner", true));
    let first = create_test_tunnel_network(&mut world, Vec3::ZERO);
    let second = create_test_tunnel_network(&mut world, Vec3::new(20.0, 0.0, 0.0));
    let key = world.host_object(first).unwrap().tunnel_system_key();
    world.tunnel_network.on_tunnel_created(key, first);
    world.tunnel_network.on_tunnel_created(key, second);
    ensure_test_infantry_template(&mut world);
    let child = world
        .create_object("TestInfantry", Team::GLA, Vec3::ZERO)
        .unwrap();
    assert!(world.tunnel_network.record_enter(key, child, first));
    world
        .host_object_mut(child)
        .unwrap()
        .set_contained_by(Some(first));
    let hp = world.host_object(child).unwrap().health.current;
    // Retail OneShot SpawnBehavior admits defenders independently; RequiresSpawner
    // is false. Direct tunnel deletion must leave those unrelated units intact.
    let mut unrelated: Vec<_> = world
        .host_objects()
        .keys()
        .copied()
        .filter(|id| ![first, second, child].contains(id))
        .collect();
    unrelated.sort_by_key(|id| id.0);
    world.destroy_object(first);
    assert_eq!(world.host_object(child).unwrap().contained_by, Some(second));
    assert!(!world.host_object(child).unwrap().status.destroyed);
    assert_eq!(world.tunnel_network.contain_count(key), 1);
    world.destroy_object(second);
    let object = world.host_object(child).unwrap();
    assert!(object.status.destroyed && !object.status.on_die_started);
    assert_eq!(object.health.current, hp);
    assert_eq!(object.contained_by, None);
    assert_eq!(world.tunnel_network.contain_count(key), 0);
    assert_eq!(
        world
            .objects_to_destroy
            .iter()
            .map(|e| e.id)
            .collect::<Vec<_>>(),
        vec![first, second, child]
    );
    world.process_destroy_list();
    for id in [first, second, child] {
        assert!(world.host_object(id).is_none());
    }
    let mut remaining: Vec<_> = world.host_objects().keys().copied().collect();
    remaining.sort_by_key(|id| id.0);
    assert_eq!(remaining, unrelated);
}

#[test]
fn unregistered_tunnel_delete_does_not_consume_a_live_shared_pool() {
    let mut world = GameLogic::new();
    world.add_player(Player::new(2, Team::GLA, "Owner", true));
    let tunnel = create_test_tunnel_network(&mut world, Vec3::ZERO);
    let key = world.host_object(tunnel).unwrap().tunnel_system_key();
    ensure_test_infantry_template(&mut world);
    let child = world
        .create_object("TestInfantry", Team::GLA, Vec3::ZERO)
        .unwrap();
    assert!(world.tunnel_network.record_enter(key, child, tunnel));
    world.tunnel_network.network_mut(key).tunnel_ids.clear();
    let before = world.tunnel_network.tunnels_destroyed;
    world.destroy_object(tunnel);
    assert_eq!(world.tunnel_network.tunnels_destroyed, before);
    assert!(world.tunnel_network.is_in_network(key, child));
    assert!(!world.host_object(child).unwrap().status.destroyed);
}
