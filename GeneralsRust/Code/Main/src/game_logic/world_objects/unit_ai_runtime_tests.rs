use crate::game_logic::{GameLogic, Object, ObjectId, Team, ThingTemplate};
use glam::Vec3;

fn object(id: ObjectId) -> Object {
    Object::new(ThingTemplate::new("UnitAiLifetime"), id, Team::USA)
}

fn seed_runtime(world: &mut GameLogic, id: ObjectId) {
    world
        .unit_ai_runtime_mut(id)
        .expect("unit")
        .set_guard_scan_deadline(Some(90));
    world
        .unit_ai_runtime_mut(id)
        .expect("unit")
        .set_hunt_scan_deadline(Some(120));
    world
        .unit_ai_runtime_mut(id)
        .expect("unit")
        .observe_guard_anchor(Vec3::new(10.0, 0.0, 20.0));
    world
        .unit_ai_runtime_mut(id)
        .expect("unit")
        .set_quick_exit_deadline(Some(300));
}

fn runtime(
    world: &GameLogic,
    id: ObjectId,
) -> (Option<u32>, Option<u32>, Option<Vec3>, Option<u32>) {
    (
        world
            .unit_ai_runtime(id)
            .and_then(|runtime| runtime.guard_scan_deadline()),
        world
            .unit_ai_runtime(id)
            .and_then(|runtime| runtime.hunt_scan_deadline()),
        world
            .unit_ai_runtime(id)
            .and_then(|runtime| runtime.guard_anchor()),
        world
            .unit_ai_runtime(id)
            .and_then(|runtime| runtime.quick_exit_deadline()),
    )
}

#[test]
fn unit_ai_runtime_fresh_object_reusing_id_has_no_old_deadline_or_anchor() {
    let mut world = GameLogic::new();
    let id = ObjectId(934201);
    world.add_object(object(id));
    seed_runtime(&mut world, id);
    world.add_object(object(id));
    assert_eq!(runtime(&world, id), (None, None, None, None));
}

#[test]
fn unit_ai_runtime_world_reset_drops_old_guard_anchor_and_quick_exit() {
    let mut world = GameLogic::new();
    let id = ObjectId(934202);
    world.add_object(object(id));
    seed_runtime(&mut world, id);
    world.reset();
    world.add_object(object(id));
    assert_eq!(runtime(&world, id), (None, None, None, None));
}

#[test]
fn quick_exit_completion_restores_guard_before_same_tick_dispatch() {
    // AIUpdate::doQuickExit keeps the guard machine locked while the exit path
    // runs; support dispatch must see the restored guard state on completion.
    for (label, frame, endpoint, points, finished) in [
        ("deadline", 11, Vec3::new(10.0, 0.0, 0.0), 2, true),
        ("arrived", 9, Vec3::ZERO, 2, true),
        ("path gone", 9, Vec3::new(10.0, 0.0, 0.0), 1, true),
        ("still exiting", 9, Vec3::new(10.0, 0.0, 0.0), 2, false),
    ] {
        let mut world = GameLogic::new();
        world.frame = frame;
        let unit_id = ObjectId(934204);
        let guard_id = ObjectId(934205);
        let mut unit = object(unit_id);
        unit.guard_target = Some(guard_id);
        unit.set_ai_state(crate::game_logic::AIState::Moving);
        unit.movement.path = vec![endpoint; points];
        unit.movement.target_position = Some(endpoint);
        unit.can_path_through_units = true;
        unit.adjust_destinations = false;
        world.add_object(unit);
        world.add_object(object(guard_id));
        world
            .unit_ai_runtime_mut(unit_id)
            .unwrap()
            .set_quick_exit_deadline(Some(10));
        // The temporary exit suspends an existing nested Idle state.
        let runtime = world.unit_ai_runtime_mut(unit_id).unwrap();
        runtime.set_guard_phase(Some(
            crate::game_logic::object::unit_ai_runtime::GuardPhase::Idle,
        ));
        runtime.set_guard_scan_deadline(Some(frame));

        world.update_support_states(&[unit_id], 1.0 / 30.0);

        let unit = world.host_object(unit_id).unwrap();
        if finished {
            assert_eq!(
                unit.ai_state,
                crate::game_logic::AIState::GuardingObject,
                "{label}"
            );
            assert!(unit.movement.path.is_empty(), "{label}");
            assert_eq!(unit.movement.target_position, None, "{label}");
            assert!(!unit.can_path_through_units, "{label}");
            assert!(unit.adjust_destinations, "{label}");
            assert_eq!(
                world
                    .unit_ai_runtime(unit_id)
                    .unwrap()
                    .quick_exit_deadline(),
                None,
                "{label}"
            );
            assert_eq!(
                world.unit_ai_runtime(unit_id).unwrap().guard_anchor(),
                Some(Vec3::ZERO),
                "{label}: the refreshed guard state must dispatch in this tick"
            );
        } else {
            assert_eq!(unit.ai_state, crate::game_logic::AIState::Moving, "{label}");
            assert_eq!(unit.movement.path, vec![endpoint; points], "{label}");
            assert_eq!(
                world
                    .unit_ai_runtime(unit_id)
                    .unwrap()
                    .quick_exit_deadline(),
                Some(10),
                "{label}"
            );
            assert_eq!(
                world.unit_ai_runtime(unit_id).unwrap().guard_anchor(),
                None,
                "{label}"
            );
        }
    }
}

#[test]
fn quick_exit_deadline_preserves_path_at_end_frame_and_expires_next_frame() {
    // AIStates.cpp:844-868 updates the temporary state, then expires only when
    // m_temporaryStateFramEnd < frame. A continuing path survives equality.
    let mut world = GameLogic::new();
    let unit_id = ObjectId(934206);
    let guard_id = ObjectId(934207);
    let endpoint = Vec3::new(10.0, 0.0, 0.0);
    let mut unit = object(unit_id);
    unit.guard_target = Some(guard_id);
    unit.set_ai_state(crate::game_logic::AIState::Moving);
    unit.movement.path = vec![endpoint, endpoint];
    unit.movement.target_position = Some(endpoint);
    unit.can_path_through_units = true;
    unit.adjust_destinations = false;
    world.add_object(unit);
    world.add_object(object(guard_id));
    world
        .unit_ai_runtime_mut(unit_id)
        .unwrap()
        .set_quick_exit_deadline(Some(10));
    let runtime = world.unit_ai_runtime_mut(unit_id).unwrap();
    runtime.set_guard_phase(Some(
        crate::game_logic::object::unit_ai_runtime::GuardPhase::Idle,
    ));
    runtime.set_guard_scan_deadline(Some(10));

    world.frame = 10;
    world.update_support_states(&[unit_id], 1.0 / 30.0);
    let unit = world.host_object(unit_id).unwrap();
    assert_eq!(
        unit.ai_state,
        crate::game_logic::AIState::Moving,
        "C++ continuing temporary exit state is still active at its end frame"
    );
    assert_eq!(unit.movement.path, vec![endpoint, endpoint]);
    assert!(unit.can_path_through_units);
    assert!(!unit.adjust_destinations);
    assert_eq!(
        world
            .unit_ai_runtime(unit_id)
            .unwrap()
            .quick_exit_deadline(),
        Some(10)
    );

    world.frame = 11;
    world.update_support_states(&[unit_id], 1.0 / 30.0);
    let unit = world.host_object(unit_id).unwrap();
    assert_eq!(unit.ai_state, crate::game_logic::AIState::GuardingObject);
    assert!(unit.movement.path.is_empty());
    assert!(!unit.can_path_through_units);
    assert!(unit.adjust_destinations);
    assert_eq!(
        world
            .unit_ai_runtime(unit_id)
            .unwrap()
            .quick_exit_deadline(),
        None
    );
    assert_eq!(
        world.unit_ai_runtime(unit_id).unwrap().guard_anchor(),
        Some(Vec3::ZERO),
        "underlying guard dispatch resumes in the same frame as expiry"
    );
}
#[test]
fn unit_ai_runtime_cloned_object_has_fresh_state_on_cross_world_admission() {
    let mut source = GameLogic::new();
    let source_id = ObjectId(934208);
    source.add_object(object(source_id));
    seed_runtime(&mut source, source_id);
    let expected_source = runtime(&source, source_id);
    for destination_id in [source_id, ObjectId(934209)] {
        let mut destination = GameLogic::new();
        let mut cloned = source.host_object(source_id).unwrap().clone();
        cloned.id = destination_id;
        destination.add_object(cloned);
        assert_eq!(
            runtime(&destination, destination_id),
            (None, None, None, None),
            "new admission with {destination_id:?} must not inherit another world's AI runtime"
        );
        assert_eq!(runtime(&source, source_id), expected_source);
    }
}

#[test]
fn unit_ai_runtime_query_clone_and_live_crush_reinsert_preserve_existing_state() {
    let mut world = GameLogic::new();
    let id = ObjectId(934210);
    let crusher_id = ObjectId(934211);
    world.add_object(object(id));
    world.add_object(object(crusher_id));
    seed_runtime(&mut world, id);
    let expected = runtime(&world, id);
    let generation = world.host_object(id).unwrap().visual_object_generation;
    let query_snapshot = world.host_object(id).unwrap().clone();
    assert_eq!(
        (
            query_snapshot.unit_ai_runtime.guard_scan_deadline(),
            query_snapshot.unit_ai_runtime.hunt_scan_deadline(),
            query_snapshot.unit_ai_runtime.guard_anchor(),
            query_snapshot.unit_ai_runtime.quick_exit_deadline(),
        ),
        expected,
        "read-only Object clones must preserve the owner's snapshot"
    );

    // This real physics API extracts and reinserts the same crushee to split
    // mutable borrows; it is not a new admission or a new AI state lifetime.
    assert!(!world.apply_overlap_crush_check(crusher_id, id, false));
    assert_eq!(runtime(&world, id), expected);
    assert_eq!(
        world.host_object(id).unwrap().visual_object_generation,
        generation
    );
}
