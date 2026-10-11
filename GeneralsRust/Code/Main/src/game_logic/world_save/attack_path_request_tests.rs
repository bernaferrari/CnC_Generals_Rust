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
fn attack_request_mapless_fallback_keeps_victim_and_skips_move_initializer() {
    let goal = Vec3::new(80.0, 0.0, 0.0);
    let mut world = world(false, goal);
    world.objects.get_mut(&ATTACKER).unwrap().is_final_goal = true;
    assert!(world.request_attack_path(ATTACKER, Some(VICTIM), goal));
    let unit = &world.objects[&ATTACKER];
    assert_eq!(unit.requested_victim_id, Some(VICTIM));
    assert!(!unit.is_attack_path);
    assert!(!unit.waiting_for_path);
    assert_eq!(unit.queue_for_path_frames, 0);
    assert!(!unit.movement.path.is_empty());
    assert!(unit.is_final_goal);
}

#[test]
fn attack_request_loaded_admission_preserves_old_path_until_resolution() {
    let goal = Vec3::new(80.0, 0.0, 0.0);
    let mut world = world(true, goal);
    let old = vec![Vec3::ZERO, Vec3::new(0.0, 0.0, 20.0)];
    world.objects.get_mut(&ATTACKER).unwrap().movement.path = old.clone();
    assert!(world.request_attack_path(ATTACKER, Some(VICTIM), goal));
    let unit = &world.objects[&ATTACKER];
    assert_eq!(unit.movement.path, old);
    assert!(unit.waiting_for_path);
    assert!(unit.is_attack_path);
    assert_eq!(unit.requested_victim_id, Some(VICTIM));
    assert_eq!(world.pathfinding_system.pending_path_count(), 1);
    // A second accepted request replaces its queued entry, rather than searching.
    assert!(world.request_attack_path(ATTACKER, Some(VICTIM), goal + Vec3::Z));
    assert_eq!(world.pathfinding_system.pending_path_count(), 1);
    assert_eq!(
        world
            .pathfinding_system
            .pending_paths()
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
    assert!(world.request_attack_path(ATTACKER, Some(VICTIM), goal));
    let current = Vec3::new(120.0, 0.0, 0.0);
    world
        .objects
        .get_mut(&VICTIM)
        .unwrap()
        .set_position(current);
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
    assert_eq!(unit.queue_for_path_frames, 0);
}

#[test]
fn attack_request_missing_victim_keeps_destination_without_new_ignore() {
    let goal = Vec3::new(80.0, 0.0, 0.0);
    let mut world = world(true, goal);
    assert!(world.request_attack_path(ATTACKER, Some(VICTIM), goal));
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
    assert!(world.request_attack_path(ATTACKER, Some(VICTIM), goal));
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
    assert!(world.request_attack_path(ATTACKER, Some(VICTIM), goal));
    world.objects.get_mut(&ATTACKER).unwrap().waiting_for_path = false;
    resolve(&mut world);
    assert!(world.objects[&ATTACKER].movement.path.is_empty());
    assert_eq!(world.pathfinding_system.pending_path_count(), 0);
}

#[test]
fn attack_request_fresh_move_replaces_attack_metadata() {
    let goal = Vec3::new(80.0, 0.0, 0.0);
    let mut world = world(true, goal);
    assert!(world.request_attack_path(ATTACKER, Some(VICTIM), goal));
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
            world.request_attack_path(ATTACKER, Some(VICTIM), goal),
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
        assert_eq!(unit.queue_for_path_frames, if accepted { 0 } else { 60 });
        if !accepted {
            assert!(unit.movement.target_position.is_none());
        }
    }
}

#[test]
fn attack_request_same_ids_resolve_in_their_own_world() {
    let mut a = world(true, Vec3::new(80.0, 0.0, 0.0));
    let mut b = world(true, Vec3::new(0.0, 0.0, 120.0));
    assert!(a.request_attack_path(ATTACKER, Some(VICTIM), Vec3::new(80.0, 0.0, 0.0)));
    assert!(b.request_attack_path(ATTACKER, Some(VICTIM), Vec3::new(0.0, 0.0, 120.0)));
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
            source.request_attack_path(ATTACKER, Some(VICTIM), goal),
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
        assert_eq!(a.queue_for_path_frames, b.queue_for_path_frames);
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
                    .queue_for_path_frames = 0;
                world.requeue_expired_path_requests(&[ATTACKER]);
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
        assert!(world.attack_approach_compute_path(ATTACKER, Some(VICTIM), None));
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
