//! C++ AIGuard.cpp:602-735: Return completion selects Idle, not guard radius.
use super::guard_return_rate_control_tests::{authored_rates, unfinished_return};
use super::named_command_test_support::{execute, world};
use super::*;
use crate::game_logic::object::unit_ai_runtime::GuardPhase;
use gamelogic::scripting::core::ScriptActionType;

#[derive(Debug, PartialEq)]
struct Observation {
    phase: Option<GuardPhase>,
    wander_width: f32,
    deadline: Option<u32>,
    updated_frame: Option<u32>,
    anchor: Option<Vec3>,
    state: AIState,
    position: Vec3,
    path: Vec<Vec3>,
    index: usize,
    destination: Option<Vec3>,
    path_goal: Option<Vec3>,
    moving: bool,
    waiting: bool,
    target: Option<ObjectId>,
}

fn observe(world: &GameLogic, id: ObjectId) -> Observation {
    let unit = world.host_object(id).unwrap();
    Observation {
        phase: unit.unit_ai_runtime.guard_phase(),
        wander_width: unit.wander_width_factor,
        deadline: unit.unit_ai_runtime.guard_scan_deadline(),
        updated_frame: unit.unit_ai_runtime.guard_updated_frame(),
        anchor: unit.unit_ai_runtime.guard_anchor(),
        state: unit.ai_state.clone(),
        position: unit.get_position(),
        path: unit.movement.path.clone(),
        index: unit.movement.current_path_index,
        destination: unit.requested_destination,
        path_goal: unit.path_goal_position,
        moving: unit.status.moving,
        waiting: unit.waiting_for_path,
        target: unit.target,
    }
}

#[test]
fn unfinished_return_inside_inner_radius_keeps_return_scan_rate() {
    let (mut world, id) = unfinished_return();
    let before = world.host_object(id).unwrap().get_position();
    world.tick_host_guard_states(&[id]);
    let unit = world.host_object(id).unwrap();
    // OLD inferred Idle from distance and scheduled 107 here.
    assert_eq!(unit.unit_ai_runtime.guard_scan_deadline(), Some(160));
    assert!(matches!(
        unit.unit_ai_runtime.guard_phase(),
        Some(GuardPhase::Return { .. })
    ));
    assert_eq!(
        unit.get_position(),
        before,
        "state update must not integrate locomotion"
    );
    assert!(!GameLogic::host_internal_move_reached_goal(unit));
}

#[test]
fn actual_locomotor_arrival_enters_idle_once_then_reentry_returns() {
    let (mut world, id) = unfinished_return();
    let start = world.host_object(id).unwrap().get_position();
    world
        .host_object_mut(id)
        .unwrap()
        .unit_ai_runtime
        .set_guard_scan_deadline(Some(500));
    let mut entered = None;
    for frame in 100..250 {
        world.frame = frame;
        world.tick_host_guard_states(&[id]);
        if world.host_object(id).unwrap().unit_ai_runtime.guard_phase() == Some(GuardPhase::Idle) {
            entered = Some(frame);
            break;
        }
        world.update_movement_for_test(&[id], 1.0 / 30.0);
    }
    let entered = entered.expect("the real locomotor route must complete and enter Idle");
    let unit = world.host_object(id).unwrap();
    assert_ne!(
        unit.get_position(),
        start,
        "arrival must exercise actual movement"
    );
    assert!(!unit.status.moving);
    let deadline = unit.unit_ai_runtime.guard_scan_deadline().unwrap();
    assert!((entered..=entered + 7).contains(&deadline));
    world.tick_host_guard_states(&[id]);
    assert_eq!(
        world
            .host_object(id)
            .unwrap()
            .unit_ai_runtime
            .guard_scan_deadline(),
        Some(deadline)
    );

    // Observe a due Idle scan independently of its randomized entry delay.
    world
        .host_object_mut(id)
        .unwrap()
        .unit_ai_runtime
        .set_guard_scan_deadline(Some(entered + 1));
    world.frame = entered + 1;
    world.tick_host_guard_states(&[id]);
    assert_eq!(
        world
            .host_object(id)
            .unwrap()
            .unit_ai_runtime
            .guard_scan_deadline(),
        Some(entered + 8)
    );
    assert_eq!(
        execute(
            &mut world,
            &[(ScriptActionType::NamedGuard, "NamedUnit", "")]
        ),
        1
    );
    assert!(matches!(
        world.host_object(id).unwrap().unit_ai_runtime.guard_phase(),
        Some(GuardPhase::Return { .. })
    ));
}

#[test]
fn active_return_snapshot_continues_route_deadline_and_phase_without_new_order() {
    let (mut source, id) = unfinished_return();
    source.tick_host_guard_states(&[id]);
    let saved = observe(&source, id);
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let (mut restored, restored_id, _) = world();
    assert_eq!(id, restored_id);
    authored_rates(&mut restored, 60, 7);
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert_eq!(observe(&restored, id), saved);
    // Restoring the observed-frame stamp prevents another update in frame100.
    restored.tick_host_guard_states(&[id]);
    assert_eq!(observe(&restored, id), saved);

    let mut reached_idle = false;
    for frame in 101..250 {
        for owner in [&mut source, &mut restored] {
            owner.frame = frame;
            owner.tick_host_guard_states(&[id]);
            owner.update_movement_for_test(&[id], 1.0 / 30.0);
        }
        assert_eq!(
            observe(&restored, id),
            observe(&source, id),
            "continuation frame {frame}"
        );
        reached_idle |= source
            .host_object(id)
            .unwrap()
            .unit_ai_runtime
            .guard_phase()
            == Some(GuardPhase::Idle);
    }
    assert!(
        reached_idle,
        "saved Return must finish its real movement and reach Idle"
    );
    let enemy = source
        .host_objects()
        .values()
        .find(|u| u.name == "NamedTarget")
        .unwrap()
        .id;
    for owner in [&mut source, &mut restored] {
        owner
            .players
            .get_mut(&1)
            .unwrap()
            .set_map_relationship(2, gamelogic::common::Relationship::Enemies);
        let position = owner.host_object(id).unwrap().get_position() + Vec3::new(20.0, 0.0, 0.0);
        owner.host_object_mut(enemy).unwrap().set_position(position);
        owner.frame = 250;
        owner
            .host_object_mut(id)
            .unwrap()
            .unit_ai_runtime
            .set_guard_scan_deadline(Some(250));
        owner.tick_host_guard_states(&[id]);
        assert_eq!(owner.host_object(id).unwrap().target, Some(enemy));
        assert_eq!(owner.host_object(id).unwrap().ai_state, AIState::Attacking);
    }
    assert_eq!(observe(&restored, id), observe(&source, id));
}

#[test]
fn same_id_world_guard_phases_deadlines_and_reset_are_isolated() {
    let (mut first, id) = unfinished_return();
    let (mut second, second_id) = unfinished_return();
    assert_eq!(id, second_id);
    authored_rates(&mut second, 90, 11);
    second
        .host_object_mut(id)
        .unwrap()
        .set_position(Vec3::new(200.0, 0.0, 200.0));
    second.return_guard_to_post(id);
    second
        .host_object_mut(id)
        .unwrap()
        .unit_ai_runtime
        .set_guard_scan_deadline(Some(100));
    let pending = observe(&first, id);
    second.tick_host_guard_states(&[id]);
    assert_eq!(observe(&first, id), pending);
    assert_eq!(
        second
            .host_object(id)
            .unwrap()
            .unit_ai_runtime
            .guard_scan_deadline(),
        Some(190)
    );
    first.tick_host_guard_states(&[id]);
    assert_eq!(
        first
            .host_object(id)
            .unwrap()
            .unit_ai_runtime
            .guard_scan_deadline(),
        Some(160)
    );
    let continued = observe(&first, id);
    second.reset();
    assert_eq!(observe(&first, id), continued);
    first.frame = 160;
    first.tick_host_guard_states(&[id]);
    assert_eq!(
        first
            .host_object(id)
            .unwrap()
            .unit_ai_runtime
            .guard_scan_deadline(),
        Some(220)
    );
}

#[test]
fn due_return_enemy_acquisition_precedes_simultaneous_arrival() {
    let (mut owner, id) = unfinished_return();
    let enemy = owner
        .host_objects()
        .values()
        .find(|u| u.name == "NamedTarget")
        .unwrap()
        .id;
    owner
        .players
        .get_mut(&1)
        .unwrap()
        .set_map_relationship(2, gamelogic::common::Relationship::Enemies);
    let last = *owner.host_object(id).unwrap().movement.path.last().unwrap();
    owner.host_object_mut(id).unwrap().set_position(last);
    assert!(GameLogic::host_internal_move_reached_goal(
        owner.host_object(id).unwrap()
    ));
    owner.tick_host_guard_states(&[id]);
    let unit = owner.host_object(id).unwrap();
    assert_eq!(unit.target, Some(enemy));
    assert_eq!(unit.ai_state, AIState::Attacking);
    assert_eq!(
        unit.unit_ai_runtime.guard_phase(),
        None,
        "acquisition exits Return before Idle can enter"
    );
}

#[test]
fn guardee_drift_is_observed_only_in_due_idle_body() {
    let (mut owner, id, target) = world();
    authored_rates(&mut owner, 60, 7);
    assert!(owner.unit_command_guard_object(id, target));
    let last = *owner.host_object(id).unwrap().movement.path.last().unwrap();
    owner.host_object_mut(id).unwrap().set_position(last);
    owner.frame = 1;
    owner.tick_host_guard_states(&[id]);
    assert_eq!(
        owner.host_object(id).unwrap().unit_ai_runtime.guard_phase(),
        Some(GuardPhase::Idle)
    );
    let original_anchor = owner
        .host_object(id)
        .unwrap()
        .unit_ai_runtime
        .guard_anchor()
        .unwrap();
    owner
        .host_object_mut(id)
        .unwrap()
        .unit_ai_runtime
        .set_guard_scan_deadline(Some(10));
    let displaced = original_anchor + Vec3::new(50.0, 0.0, 0.0);
    owner
        .host_object_mut(target)
        .unwrap()
        .set_position(displaced);
    owner.frame = 9;
    owner.tick_host_guard_states(&[id]);
    let unit = owner.host_object(id).unwrap();
    assert_eq!(unit.unit_ai_runtime.guard_phase(), Some(GuardPhase::Idle));
    assert_eq!(unit.unit_ai_runtime.guard_anchor(), Some(original_anchor));
    owner.frame = 10;
    owner.tick_host_guard_states(&[id]);
    let unit = owner.host_object(id).unwrap();
    assert!(
        matches!(unit.unit_ai_runtime.guard_phase(), Some(GuardPhase::Return { goal }) if (goal-displaced).length() < 15.0)
    );
    assert!(unit.status.moving);
    assert!(!unit.movement.path.is_empty());
}

#[test]
fn cancelled_return_path_does_not_count_as_arrival() {
    let (mut owner, id) = unfinished_return();
    owner.host_object_mut(id).unwrap().stop_moving();
    assert!(!GameLogic::host_internal_move_reached_goal(
        owner.host_object(id).unwrap()
    ));
    owner.tick_host_guard_states(&[id]);
    assert!(matches!(
        owner.host_object(id).unwrap().unit_ai_runtime.guard_phase(),
        Some(GuardPhase::Return { .. })
    ));
}

#[test]
fn idle_snapshot_preserves_anchor_and_due_drift_continuation() {
    let (mut source, id, target) = world();
    authored_rates(&mut source, 60, 7);
    assert!(source.unit_command_guard_object(id, target));
    let last = *source
        .host_object(id)
        .unwrap()
        .movement
        .path
        .last()
        .unwrap();
    source.host_object_mut(id).unwrap().set_position(last);
    source.frame = 1;
    source.tick_host_guard_states(&[id]);
    assert_eq!(
        source
            .host_object(id)
            .unwrap()
            .unit_ai_runtime
            .guard_phase(),
        Some(GuardPhase::Idle)
    );
    source
        .host_object_mut(id)
        .unwrap()
        .unit_ai_runtime
        .set_guard_scan_deadline(Some(20));
    let snapshot = crate::save_load::SnapshotBuilder::new()
        .create_world_snapshot(&source)
        .unwrap();
    let (mut restored, _, _) = world();
    authored_rates(&mut restored, 60, 7);
    crate::save_load::SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert_eq!(observe(&source, id), observe(&restored, id));
    for owner in [&mut source, &mut restored] {
        let p = owner.host_object(target).unwrap().get_position() + Vec3::new(50.0, 0.0, 0.0);
        owner.host_object_mut(target).unwrap().set_position(p);
        owner.frame = 20;
        owner.tick_host_guard_states(&[id]);
    }
    assert_eq!(observe(&source, id), observe(&restored, id));
    assert!(matches!(
        source
            .host_object(id)
            .unwrap()
            .unit_ai_runtime
            .guard_phase(),
        Some(GuardPhase::Return { .. })
    ));
}

#[test]
fn temporary_quick_exit_preserves_idle_and_resumes_once_on_completion_frame() {
    let (mut owner, id, target) = world();
    authored_rates(&mut owner, 60, 7);
    assert!(owner.unit_command_guard_object(id, target));
    let last = *owner.host_object(id).unwrap().movement.path.last().unwrap();
    owner.host_object_mut(id).unwrap().set_position(last);
    owner.frame = 1;
    owner.tick_host_guard_states(&[id]);
    let unit = owner.host_object_mut(id).unwrap();
    assert_eq!(unit.unit_ai_runtime.guard_phase(), Some(GuardPhase::Idle));
    let anchor = unit.unit_ai_runtime.guard_anchor();
    unit.unit_ai_runtime.set_guard_scan_deadline(Some(11));
    // Same explicit overlay admission used by the tunnel producer.
    let end = unit.get_position() + Vec3::new(100.0, 0.0, 0.0);
    unit.movement.path = vec![end, end];
    unit.movement.target_position = Some(end);
    unit.can_path_through_units = true;
    unit.adjust_destinations = false;
    unit.unit_ai_runtime.set_quick_exit_deadline(Some(10));
    unit.set_ai_state(AIState::Moving);
    assert_eq!(unit.unit_ai_runtime.guard_phase(), Some(GuardPhase::Idle));
    assert_eq!(unit.unit_ai_runtime.guard_scan_deadline(), Some(11));
    owner.frame = 10;
    owner.update_support_states(&[id], 1.0 / 30.0);
    let unit = owner.host_object(id).unwrap();
    assert_eq!(unit.ai_state, AIState::Moving);
    assert_eq!(unit.unit_ai_runtime.guard_scan_deadline(), Some(11));
    assert_eq!(unit.unit_ai_runtime.guard_anchor(), anchor);
    owner.frame = 11;
    owner.update_support_states(&[id], 1.0 / 30.0);
    let unit = owner.host_object(id).unwrap();
    assert_eq!(unit.ai_state, AIState::GuardingObject);
    assert_eq!(unit.unit_ai_runtime.guard_phase(), Some(GuardPhase::Idle));
    assert_eq!(unit.unit_ai_runtime.guard_scan_deadline(), Some(18));
    assert_eq!(unit.unit_ai_runtime.quick_exit_deadline(), None);
    owner.update_support_states(&[id], 1.0 / 30.0);
    assert_eq!(
        owner
            .host_object(id)
            .unwrap()
            .unit_ai_runtime
            .guard_scan_deadline(),
        Some(18)
    );
    let unit = owner.host_object_mut(id).unwrap();
    unit.set_ai_state(AIState::Moving);
    assert_eq!(unit.unit_ai_runtime.guard_phase(), None);
    assert_eq!(unit.unit_ai_runtime.guard_scan_deadline(), None);
    assert_eq!(unit.unit_ai_runtime.guard_anchor(), None);
}
