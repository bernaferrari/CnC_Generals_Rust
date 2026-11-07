//! Production request regression cases derived from AIUpdate.cpp:378–525.
use super::*;
use crate::game_logic::{KindOf, Object, Team, ThingTemplate, Weapon};

const ATTACKER: ObjectId = ObjectId(5101);
const VICTIM: ObjectId = ObjectId(5102);

fn world(loaded: bool, target: Vec3) -> GameLogic {
    let mut world = GameLogic::new();
    world.frame = 100;
    world.map_loaded = loaded;
    let mut template = ThingTemplate::new("AttackRequestUnit");
    template.add_kind_of(KindOf::Vehicle);
    world
        .templates
        .insert(template.name.clone(), template.clone());
    let mut unit = Object::new(template, ATTACKER, Team::USA);
    unit.set_position(Vec3::ZERO);
    unit.movement.max_speed = 30.0;
    world.objects.insert(ATTACKER, unit);
    let mut template = ThingTemplate::new("AttackRequestVictim");
    template.add_kind_of(KindOf::Infantry);
    world
        .templates
        .insert(template.name.clone(), template.clone());
    let mut victim = Object::new(template, VICTIM, Team::GLA);
    victim.set_position(target);
    world.objects.insert(VICTIM, victim);
    world
}

fn resolve(world: &mut GameLogic) {
    world.process_pathfind_queue();
}
#[test]
fn queued_attack_fallback_uses_receiver_ignore_after_snapshot() {
    use game_engine::common::{ini::ini_locomotor::LocomotorStore, system::DataChunkOutput};
    use crate::game_logic::{GameMode, Player};
    use crate::save_load::{Snapshot, SnapshotBuilder, WorldSnapshot, XferLoad, XferSave};
    // AIUpdate.cpp:435-437 adjusts before replacing the ignored obstacle with
    // the victim. A reservation belonging to another ally exposes that order.
    let directory = tempfile::tempdir().unwrap();
    let map = directory.path().join("AttackDestination.map");
    let mut data = DataChunkOutput::new();
    data.open_data_chunk("HeightMapData", 3);
    for value in [41, 41, 1, 1681] {
        data.write_int(value);
    }
    for _ in 0..1681 {
        data.write_byte(0);
    }
    data.close_data_chunk();
    data.open_data_chunk("ObjectsList", 3);
    data.close_data_chunk();
    std::fs::write(&map, data.into_ckmp_bytes()).unwrap();
    let make_world = |map: &std::path::Path| {
        let mut world = GameLogic::new();
        let mut locomotors = LocomotorStore::new_owned();
        locomotors.apply_source("Locomotor RetryWalk\n Surfaces = GROUND\n Speed = 30\n Acceleration = 60\n TurnRate = 360\n Appearance = TWO_LEGS\nEnd\n").unwrap();
        world.admit_locomotor_definitions(locomotors).unwrap();
        world.admit_object_ini_sources([
            ("RetryWalk.ini".into(), "Object RetryWalker\n KindOf = SELECTABLE INFANTRY\n Locomotor = SET_NORMAL RetryWalk\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = AIUpdateInterface Brain\n End\nEnd\n".into()),
        ]).unwrap();
        world.start_new_game(GameMode::SinglePlayer).unwrap();
        assert!(world.load_map(map.to_str().unwrap()));
        world.add_player(Player::new(1, Team::USA, "Owner", true));
        world
    };
    let goal = Vec3::new(155.0, 0.0, 105.0);
    let mut worlds = [make_world(&map), make_world(&map)];
    let mut ids = Vec::new();
    for receiver in &mut worlds {
        let attacker = receiver.create_object_for_player("RetryWalker", 1, Vec3::new(105.0, 0.0, 105.0)).unwrap();
        let victim = receiver.create_object_for_player("RetryWalker", 1, goal).unwrap();
        let reserving = receiver.create_object_for_player("RetryWalker", 1, Vec3::new(200.0, 0.0, 100.0)).unwrap();
        // requestAttackPath retains an existing path until doPathfind; keep
        // that path in flight across the object-update/queue boundary.
        assert!(receiver.assign_unit_path_exact(attacker, Vec3::new(105.0, 0.0, 205.0), &[]));
        // A moving victim rebuilds its separate saved goal on load. An idle
        // victim would correctly reserve its current position during
        // AIUpdate.cpp:5231-5245 and replace the reservation under test.
        receiver.host_object_mut(victim).unwrap().is_final_goal = true;
        assert!(receiver.assign_unit_path_exact(victim, Vec3::new(255.0, 0.0, 105.0), &[]));
        assert!(receiver.host_object(victim).unwrap().host_ai_is_moving());
        assert_ne!(receiver.host_object(victim).unwrap().pathfind_goal_cell, (15, 10));
        receiver.host_object_mut(reserving).unwrap().is_final_goal = true;
        assert!(receiver.assign_unit_path_exact(reserving, goal, &[]));
        ids.push((attacker, victim, reserving));
    }
    assert_eq!(ids[0], ids[1]);
    worlds[0].host_object_mut(ids[0].0).unwrap().ignored_obstacle_id = Some(ids[0].2);
    for receiver in &mut worlds {
        assert!(receiver.request_attack_path(ids[0].0, Some(ids[0].1), goal).unwrap());
    }
    let builder = SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&worlds[0]).unwrap();
    let mut bytes = std::io::Cursor::new(Vec::new());
    snapshot.xfer(&mut XferSave::new(&mut bytes)).unwrap();
    let mut snapshot = WorldSnapshot::default();
    snapshot.xfer(&mut XferLoad::new(std::io::Cursor::new(bytes.into_inner()))).unwrap();
    let mut restored = make_world(&map);
    builder.restore_from_snapshot(&snapshot, &mut restored).unwrap();
    let [owner, peer] = &mut worlds;
    for (label, receiver, ignored) in [("owner", owner, true), ("restored", &mut restored, true), ("peer", peer, false)] {
        assert!(receiver.host_object(ids[0].1).unwrap().host_ai_is_moving(), "{label}: victim must retain its separate moving goal");
        let before = receiver.host_object(ids[0].0).unwrap();
        assert!(before.waiting_for_path, "{label}: attack request was not restored/queued");
        assert_eq!(before.ignored_obstacle_id, ignored.then_some(ids[0].2), "{label}: receiver ignore before queue drain");
        assert_eq!(receiver.pathfinding_ground_goal(crate::game_logic::pathfinding::GridPos::new(15, 10)), Some(ids[0].2), "{label}: reservation before queue drain");
        let mut initial_destination = goal;
        assert!(receiver.adjust_to_possible_destination(ids[0].0, &mut initial_destination));
        assert_eq!(initial_destination == goal, ignored, "{label}: receiver context before queue drain, destination {initial_destination:?}");
        // Drive AI::update's real queue boundary directly; a full frame also
        // runs object updates before this boundary.
        receiver.process_pathfind_queue().unwrap();
        let attacker = receiver.host_object(ids[0].0).unwrap();
        assert!(!attacker.waiting_for_path);
        assert!(!attacker.is_attack_path, "the weaponless request must take the fallback");
        assert_eq!(attacker.ignored_obstacle_id, Some(ids[0].1));
        let actual = attacker.requested_destination.expect("fallback destination");
        assert_eq!(actual == goal, ignored, "{label}: queued fallback destination {actual:?}, victim position {:?}", receiver.host_object(ids[0].1).unwrap().get_position());
    }
}

#[test]
fn attack_request_mapless_fallback_keeps_victim_and_skips_move_initializer() {
    let goal = Vec3::new(80.0, 0.0, 0.0);
    let mut world = world(false, goal);
    world.objects.get_mut(&ATTACKER).unwrap().is_final_goal = true;
    assert!(
        world
            .request_attack_path(ATTACKER, Some(VICTIM), goal)
            .expect("valid attack request fixture")
    );
    let unit = &world.objects[&ATTACKER];
    assert_eq!(unit.requested_victim_id, Some(VICTIM));
    assert!(!unit.is_attack_path);
    assert!(!unit.waiting_for_path);
    assert_eq!(unit.queue_for_path_frame, 0);
    assert!(!unit.movement.path.is_empty());
    assert!(unit.is_final_goal);
}

#[test]
fn attack_request_loaded_admission_preserves_old_path_until_resolution() {
    let goal = Vec3::new(80.0, 0.0, 0.0);
    let mut world = world(true, goal);
    let old = vec![Vec3::ZERO, Vec3::new(0.0, 0.0, 20.0)];
    world.objects.get_mut(&ATTACKER).unwrap().movement.path = old.clone();
    assert!(
        world
            .request_attack_path(ATTACKER, Some(VICTIM), goal)
            .expect("valid attack request fixture")
    );
    let unit = &world.objects[&ATTACKER];
    assert_eq!(unit.movement.path, old);
    assert!(unit.waiting_for_path);
    assert!(unit.is_attack_path);
    assert_eq!(unit.requested_victim_id, Some(VICTIM));
    assert_eq!(world.pathfinding_system.pending_path_count(), 1);
    // A second accepted request replaces its queued entry, rather than searching.
    assert!(
        world
            .request_attack_path(ATTACKER, Some(VICTIM), goal + Vec3::Z)
            .expect("valid attack request fixture")
    );
    assert_eq!(world.pathfinding_system.pending_path_count(), 1);
    assert_eq!(
        world.snapshot_pending_host_paths().into_iter()
            .next()
            .unwrap()
            .destination,
        goal + Vec3::Z
    );
}

#[test]
fn attack_request_queued_fallback_observes_current_victim_position() {
    let goal = Vec3::new(80.0, 0.0, 0.0);
    let mut world = world(true, goal);
    assert!(
        world
            .request_attack_path(ATTACKER, Some(VICTIM), goal)
            .expect("valid attack request fixture")
    );
    let current = Vec3::new(120.0, 0.0, 0.0);
    world.set_fixture_object_position(VICTIM, current);
    let mut adjusted = current;
    world.adjust_to_possible_destination(ATTACKER, &mut adjusted);
    resolve(&mut world);
    let unit = &world.objects[&ATTACKER];
    assert_eq!(unit.requested_victim_id, Some(VICTIM));
    assert_eq!(unit.requested_destination, Some(adjusted));
    assert_eq!(unit.ignored_obstacle_id, Some(VICTIM));
    assert!(!unit.is_attack_path);
    assert!(!unit.waiting_for_path);
    assert!(!unit.movement.path.is_empty());
    assert_eq!(unit.queue_for_path_frame, 0);
}

#[test]
fn attack_request_missing_victim_keeps_destination_without_new_ignore() {
    let goal = Vec3::new(80.0, 0.0, 0.0);
    let mut world = world(true, goal);
    assert!(
        world
            .request_attack_path(ATTACKER, Some(VICTIM), goal)
            .expect("valid attack request fixture")
    );
    world.objects.remove(&VICTIM);
    resolve(&mut world);
    let unit = &world.objects[&ATTACKER];
    assert_eq!(unit.requested_victim_id, Some(VICTIM));
    assert_eq!(unit.requested_destination, Some(goal));
    assert_eq!(unit.ignored_obstacle_id, None);
    assert!(!unit.is_attack_path);
    assert!(!unit.waiting_for_path);
}

#[test]
fn attack_request_ranged_search_restores_attack_flag() {
    let goal = Vec3::new(80.0, 0.0, 0.0);
    let mut world = world(true, goal);
    world.objects.get_mut(&ATTACKER).unwrap().weapon = Some(Weapon {
        range: 30.0,
        damage: 10.0,
        can_target_ground: true,
        ..Default::default()
    });
    assert!(
        world
            .request_attack_path(ATTACKER, Some(VICTIM), goal)
            .expect("valid attack request fixture")
    );
    resolve(&mut world);
    let unit = &world.objects[&ATTACKER];
    assert!(unit.is_attack_path);
    assert!(!unit.waiting_for_path);
    assert_eq!(unit.requested_victim_id, Some(VICTIM));
    assert!(!unit.movement.path.is_empty());
    assert_ne!(unit.movement.path.last(), Some(&goal));
}

#[test]
fn attack_request_cancelled_waiting_does_not_install_stale_entry() {
    let goal = Vec3::new(80.0, 0.0, 0.0);
    let mut world = world(true, goal);
    assert!(
        world
            .request_attack_path(ATTACKER, Some(VICTIM), goal)
            .expect("valid attack request fixture")
    );
    world.objects.get_mut(&ATTACKER).unwrap().waiting_for_path = false;
    resolve(&mut world);
    assert!(world.objects[&ATTACKER].movement.path.is_empty());
    assert_eq!(world.pathfinding_system.pending_path_count(), 0);
}

#[test]
fn attack_request_fresh_move_replaces_attack_metadata() {
    let goal = Vec3::new(80.0, 0.0, 0.0);
    let mut world = world(true, goal);
    assert!(
        world
            .request_attack_path(ATTACKER, Some(VICTIM), goal)
            .expect("valid attack request fixture")
    );
    let newest = Vec3::new(0.0, 0.0, 100.0);
    assert!(world.assign_unit_path(ATTACKER, newest, &[]));
    assert_eq!(world.pathfinding_system.pending_path_count(), 1);
    resolve(&mut world);
    let unit = &world.objects[&ATTACKER];
    assert_eq!(unit.requested_victim_id, None);
    assert_eq!(unit.requested_destination, Some(newest));
    assert!(!unit.is_attack_path);
    assert!(!unit.waiting_for_path);
}

#[test]
fn attack_request_repath_two_and_three_frame_boundary() {
    let goal = Vec3::new(80.0, 0.0, 0.0);
    for (frame, accepted) in [(101, false), (102, false), (103, true)] {
        let mut world = world(true, goal);
        world.frame = frame;
        world.objects.get_mut(&ATTACKER).unwrap().path_timestamp = 100;
        assert_eq!(
            world
                .request_attack_path(ATTACKER, Some(VICTIM), goal)
                .expect("valid attack request fixture"),
            accepted
        );
        let unit = &world.objects[&ATTACKER];
        assert_eq!(unit.requested_victim_id, Some(VICTIM));
        assert!(unit.is_attack_path);
        assert!(unit.waiting_for_path);
        assert_eq!(
            world.pathfinding_system.pending_path_count(),
            usize::from(accepted)
        );
        assert_eq!(unit.queue_for_path_frame, if accepted { 0 } else { frame + 60 });
        if !accepted {
            assert!(unit.movement.target_position.is_none());
        }
    }
}

#[test]
fn attack_request_same_ids_resolve_in_their_own_world() {
    let mut a = world(true, Vec3::new(80.0, 0.0, 0.0));
    let mut b = world(true, Vec3::new(0.0, 0.0, 120.0));
    assert!(
        a.request_attack_path(ATTACKER, Some(VICTIM), Vec3::new(80.0, 0.0, 0.0))
            .expect("valid attack request fixture")
    );
    assert!(
        b.request_attack_path(ATTACKER, Some(VICTIM), Vec3::new(0.0, 0.0, 120.0))
            .expect("valid attack request fixture")
    );
    resolve(&mut b);
    resolve(&mut a);
    assert_ne!(
        a.objects[&ATTACKER].requested_destination,
        b.objects[&ATTACKER].requested_destination
    );
    assert_eq!(a.objects[&ATTACKER].requested_victim_id, Some(VICTIM));
    assert_eq!(b.objects[&ATTACKER].requested_victim_id, Some(VICTIM));
    assert_ne!(
        a.objects[&ATTACKER].movement.path,
        b.objects[&ATTACKER].movement.path
    );
}

#[test]
fn attack_request_snapshot_retains_queued_and_deferred_continuation() {
    use crate::save_load::SnapshotBuilder;
    let goal = Vec3::new(80.0, 0.0, 0.0);
    for deferred in [false, true] {
        let mut source = world(true, goal);
        if deferred {
            source.objects.get_mut(&ATTACKER).unwrap().path_timestamp = 99;
        }
        assert_eq!(
            source
                .request_attack_path(ATTACKER, Some(VICTIM), goal)
                .expect("valid attack request fixture"),
            !deferred
        );
        let builder = SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).unwrap();
        let mut restored = GameLogic::new();
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .unwrap();
        let a = &source.objects[&ATTACKER];
        let b = &restored.objects[&ATTACKER];
        assert_eq!(a.requested_victim_id, b.requested_victim_id);
        assert_eq!(a.requested_destination, b.requested_destination);
        assert_eq!(a.is_attack_path, b.is_attack_path);
        assert_eq!(a.waiting_for_path, b.waiting_for_path);
        assert_eq!(a.queue_for_path_frame, b.queue_for_path_frame);
        assert_eq!(
            source.pathfinding_system.pending_path_count(),
            restored.pathfinding_system.pending_path_count()
        );
        if deferred {
            // Same expiry phase used by the fixed-step path request requeue.
            for world in [&mut source, &mut restored] {
                world.frame += 60;
                world
                    .objects
                    .get_mut(&ATTACKER)
                    .unwrap()
                    .queue_for_path_frame = 0;
                world.requeue_expired_path_request(ATTACKER);
            }
        }
        resolve(&mut source);
        resolve(&mut restored);
        let a = &source.objects[&ATTACKER];
        let b = &restored.objects[&ATTACKER];
        assert_eq!(a.movement.path, b.movement.path);
        assert_eq!(a.requested_victim_id, b.requested_victim_id);
        assert_eq!(a.is_attack_path, b.is_attack_path);
        assert_eq!(a.waiting_for_path, b.waiting_for_path);
    }
}

#[test]
fn attack_request_loaded_contact_approach_waits_for_owned_search_phase() {
    let goal = Vec3::new(80.0, 0.0, 0.0);
    for old in [Vec::new(), vec![Vec3::ZERO, Vec3::new(0.0, 0.0, 20.0)]] {
        let mut world = world(true, goal);
        let unit = world.objects.get_mut(&ATTACKER).unwrap();
        unit.weapon = Some(Weapon {
            range: 5.0,
            damage: 10.0,
            can_target_ground: true,
            ..Default::default()
        });
        unit.movement.path = old.clone();
        assert!(
            world
                .attack_approach_compute_path(ATTACKER, Some(VICTIM), None)
                .expect("valid attack request fixture")
        );
        let unit = &world.objects[&ATTACKER];
        assert!(unit.waiting_for_path);
        assert!(unit.is_attack_path);
        assert_eq!(unit.requested_victim_id, Some(VICTIM));
        assert_eq!(unit.movement.path, old);
        assert_eq!(world.pathfinding_system.pending_path_count(), 1);
        resolve(&mut world);
        let unit = &world.objects[&ATTACKER];
        assert!(!unit.waiting_for_path);
        assert_eq!(unit.requested_victim_id, Some(VICTIM));
        assert!(!unit.movement.path.is_empty());
    }
}
