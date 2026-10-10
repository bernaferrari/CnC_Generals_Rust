//! C++ AIStates.cpp:844-869 / AIUpdate.cpp:998,1097: temporary update and
//! base-state resumption precede locomotion; locomotor arrival is next-frame input.
use super::guard_return_rate_control_tests::{authored_rates, unfinished_return};
use super::named_command_test_support::{execute, world};
use super::*;
use crate::game_logic::object::unit_ai_runtime::GuardPhase;
use gamelogic::scripting::core::ScriptActionType;

#[derive(Debug, PartialEq)]
struct Observation {
    frame: u32,
    state: AIState,
    phase: Option<GuardPhase>,
    deadline: Option<u32>,
    updated: Option<u32>,
    quick: Option<u32>,
    anchor: Option<Vec3>,
    position: Vec3,
    velocity: Vec3,
    path: Vec<Vec3>,
    index: usize,
    destination: Option<Vec3>,
    path_goal: Option<Vec3>,
    moving: bool,
    waiting: bool,
    through_units: bool,
    adjust: bool,
}

fn observe(owner: &GameLogic, id: ObjectId) -> Observation {
    let unit = owner.host_object(id).unwrap();
    Observation {
        frame: owner.frame,
        state: unit.ai_state.clone(),
        phase: unit.unit_ai_runtime.guard_phase(),
        deadline: unit.unit_ai_runtime.guard_scan_deadline(),
        updated: unit.unit_ai_runtime.guard_updated_frame(),
        quick: unit.unit_ai_runtime.quick_exit_deadline(),
        anchor: unit.unit_ai_runtime.guard_anchor(),
        position: unit.get_position(),
        velocity: unit.movement.velocity,
        path: unit.movement.path.clone(),
        index: unit.movement.current_path_index,
        destination: unit.requested_destination,
        path_goal: unit.path_goal_position,
        moving: unit.status.moving,
        waiting: unit.waiting_for_path,
        through_units: unit.can_path_through_units,
        adjust: unit.adjust_destinations,
    }
}

fn step(owner: &mut GameLogic) -> u32 {
    let processed = owner.frame;
    owner.update_with_dt_budget(1.0 / 30.0, 1);
    assert_eq!(owner.frame, processed + 1, "one real fixed step must run");
    processed
}

fn idle_world() -> (GameLogic, ObjectId) {
    let (mut owner, id, _) = world();
    authored_rates(&mut owner, 60, 7);
    owner.host_object_mut(id).unwrap().vision_range = 100.0;
    step(&mut owner); // Wake Movement before admitting the order/overlay.
    assert_eq!(
        execute(
            &mut owner,
            &[(ScriptActionType::NamedGuard, "NamedUnit", "")]
        ),
        1
    );
    let end = *owner.host_object(id).unwrap().movement.path.last().unwrap();
    owner.host_object_mut(id).unwrap().set_position(end);
    owner.tick_host_guard_states(&[id]);
    assert_eq!(
        owner.host_object(id).unwrap().unit_ai_runtime.guard_phase(),
        Some(GuardPhase::Idle)
    );
    (owner, id)
}

fn overlay(owner: &mut GameLogic, id: ObjectId, until: u32, distance: f32) -> Vec3 {
    let unit = owner.host_object_mut(id).unwrap();
    let end = unit.get_position() + Vec3::new(distance, 0.0, 0.0);
    // Same explicit path/deadline admission as the real tunnel producer.
    unit.movement.path = vec![end, end];
    unit.movement.current_path_index = 0;
    unit.movement.target_position = Some(end);
    unit.movement.velocity = Vec3::ZERO;
    unit.requested_destination = Some(end);
    unit.path_goal_position = Some(end);
    unit.waiting_for_path = false;
    unit.close_enough_dist = Some(1.0);
    unit.can_path_through_units = true;
    unit.adjust_destinations = false;
    unit.unit_ai_runtime.set_quick_exit_deadline(Some(until));
    unit.set_ai_state(AIState::Moving);
    unit.set_status_moving(true);
    unit.refresh_follow_path_extra_distance();
    assert!(unit.unit_ai_runtime.guard_phase().is_some());
    assert!((unit.get_position() - end).length() > 1.0);
    end
}

#[test]
fn deadline_equality_control_integrates_unfinished_temporary_route() {
    let (mut owner, id) = idle_world();
    let end = overlay(&mut owner, id, 10, 200.0);
    owner.frame = 9;
    step(&mut owner);
    let before = observe(&owner, id);
    assert_eq!(step(&mut owner), 10);
    let after = observe(&owner, id);
    assert_ne!(after.position, before.position);
    assert!((after.position - end).length() > 1.0);
    assert_eq!(after.quick, Some(10));
    assert_eq!(after.state, AIState::Moving);
}

#[test]
fn deadline_equality_moves_but_expired_idle_overlay_stops_before_locomotion() {
    let (mut owner, id) = idle_world();
    owner
        .host_object_mut(id)
        .unwrap()
        .unit_ai_runtime
        .set_guard_scan_deadline(Some(11));
    let end = overlay(&mut owner, id, 10, 200.0);
    owner.frame = 9;
    step(&mut owner);
    let before = observe(&owner, id);
    assert_eq!(step(&mut owner), 10);
    let at_deadline = observe(&owner, id);
    assert_ne!(
        at_deadline.position, before.position,
        "inclusive end frame still integrates the real exit route"
    );
    assert!(
        (at_deadline.position - end).length() > 1.0,
        "expiry witness must remain unfinished"
    );
    assert_eq!(at_deadline.quick, Some(10));
    assert_eq!(at_deadline.state, AIState::Moving);
    assert_eq!(at_deadline.deadline, Some(11));
    assert_eq!(step(&mut owner), 11);
    let expired = observe(&owner, id);
    assert_eq!(
        expired.position, at_deadline.position,
        "expired route must not move during frame11"
    );
    assert_eq!(expired.quick, None);
    assert_eq!(expired.state, AIState::GuardingArea);
    assert_eq!(expired.phase, Some(GuardPhase::Idle));
    assert_eq!(
        expired.deadline,
        Some(18),
        "due Idle body resumes once at11 with authored7-frame rate"
    );
    assert_eq!(expired.updated, Some(11));
    assert!(!expired.through_units && expired.adjust);
    assert!(expired.path.is_empty());
    assert_eq!(step(&mut owner), 12);
    let sleeping = observe(&owner, id);
    assert_eq!(sleeping.deadline, Some(18));
    assert_eq!(sleeping.updated, Some(12));
    assert_eq!(sleeping.position, expired.position);
}

#[test]
fn real_locomotor_arrival_finishes_temporary_state_on_next_frame_only() {
    let (mut owner, id) = idle_world();
    owner
        .host_object_mut(id)
        .unwrap()
        .unit_ai_runtime
        .set_guard_scan_deadline(Some(500));
    let end = overlay(&mut owner, id, 500, 8.0);
    let initial = owner.host_object(id).unwrap().get_position();
    let mut arrival = None;
    for _ in 0..150 {
        let before = owner.host_object(id).unwrap().get_position();
        assert!(
            (before - end).length() >= 1.0,
            "state boundary starts before arrival"
        );
        let processed = step(&mut owner);
        let observed = observe(&owner, id);
        assert_eq!(
            observed.quick,
            Some(500),
            "locomotor-completing frame {processed} cannot exit in late support dispatch"
        );
        assert_eq!(observed.phase, Some(GuardPhase::Idle));
        if (observed.position - end).length() < 1.0 {
            arrival = Some(observed);
            break;
        }
    }
    let arrived = arrival.expect("actual locomotor must reach the admitted endpoint");
    assert_ne!(arrived.position, initial, "no teleported arrival fixture");
    assert_eq!(arrived.state, AIState::Moving);
    assert!(
        arrived.path.len() >= 2,
        "completion evidence survives until next state update"
    );
    let processed = step(&mut owner);
    let resumed = observe(&owner, id);
    assert_eq!(resumed.quick, None);
    assert_eq!(resumed.phase, Some(GuardPhase::Idle));
    assert_eq!(resumed.state, AIState::GuardingArea);
    assert_eq!(resumed.deadline, Some(500));
    assert_eq!(resumed.updated, Some(processed));
    assert_eq!(resumed.position, arrived.position);
}

#[test]
fn suspended_return_recomputes_captured_goal_without_reentry_or_deadline_reset() {
    let (mut owner, id) = unfinished_return();
    step(&mut owner);
    let retained = owner.host_object(id).unwrap().unit_ai_runtime.guard_phase();
    let Some(GuardPhase::Return { goal }) = retained else {
        panic!("unfinished real Return required")
    };
    owner
        .host_object_mut(id)
        .unwrap()
        .unit_ai_runtime
        .set_guard_scan_deadline(Some(500));
    let temporary_goal = overlay(&mut owner, id, 101, 200.0);
    assert_eq!(step(&mut owner), 101);
    assert_eq!(
        owner.host_object(id).unwrap().unit_ai_runtime.guard_phase(),
        retained
    );
    let at_deadline = observe(&owner, id);
    assert_eq!(step(&mut owner), 102);
    let expired = observe(&owner, id);
    assert_eq!(expired.quick, None);
    assert_eq!(expired.phase, retained);
    assert_eq!(expired.deadline, Some(500));
    assert_eq!(
        expired.position, at_deadline.position,
        "unfinished temporary expiry cannot integrate either route in frame102"
    );
    assert!(
        expired.path.is_empty(),
        "AIUpdate consumes movement completion only after the resumed state update"
    );
    assert_eq!(step(&mut owner), 103);
    let resumed = observe(&owner, id);
    assert_eq!(resumed.quick, None);
    assert_eq!(
        resumed.phase, retained,
        "temporary exit cannot reenter Return with a newly captured goal"
    );
    assert_eq!(resumed.deadline, Some(500));
    assert_eq!(resumed.state, AIState::GuardingArea);
    assert_eq!(resumed.destination, Some(goal));
    assert_eq!(resumed.path_goal, Some(goal));
    assert_ne!(resumed.path.last().copied(), Some(temporary_goal));
    assert!(
        resumed.waiting || !resumed.path.is_empty(),
        "InternalMove must recompute its cleared route toward the retained goal"
    );
    let distance = (resumed.position - goal).length();
    for _ in 0..15 {
        step(&mut owner);
    }
    assert!(
        (owner.host_object(id).unwrap().get_position() - goal).length() < distance,
        "resumed route must make actual progress toward the captured goal"
    );
}

#[test]
fn authored_close_enough_completes_before_the_hardcoded_one_unit_band() {
    let (mut owner, id) = idle_world();
    owner
        .host_object_mut(id)
        .unwrap()
        .unit_ai_runtime
        .set_guard_scan_deadline(Some(500));
    let end = overlay(&mut owner, id, 500, 20.0);
    owner.host_object_mut(id).unwrap().close_enough_dist = Some(6.0);
    let initial = owner.host_object(id).unwrap().get_position();
    let mut near = None;
    for _ in 0..150 {
        step(&mut owner);
        let state = observe(&owner, id);
        assert_eq!(
            state.quick,
            Some(500),
            "the completion frame still belongs to the overlay"
        );
        let distance = (state.position - end).length();
        if distance < 6.0 {
            assert!(
                distance >= 1.0,
                "calibration must distinguish authored6wu from the old fixed1wu predicate"
            );
            near = Some(state);
            break;
        }
    }
    let near = near.expect("real locomotion must reach the authored close-enough band");
    assert_ne!(near.position, initial);
    step(&mut owner);
    let resumed = observe(&owner, id);
    assert_eq!(
        resumed.quick, None,
        "AIInternalMove observes the authored close-enough threshold"
    );
    assert_eq!(resumed.position, near.position);
    assert_eq!(resumed.phase, Some(GuardPhase::Idle));
}

#[test]
fn completed_temporary_return_route_is_observed_before_post_frame_cleanup() {
    let (mut owner, id) = unfinished_return();
    step(&mut owner);
    let Some(GuardPhase::Return { goal }) =
        owner.host_object(id).unwrap().unit_ai_runtime.guard_phase()
    else {
        panic!("real Return required")
    };
    owner
        .host_object_mut(id)
        .unwrap()
        .unit_ai_runtime
        .set_guard_scan_deadline(Some(500));
    let end = overlay(&mut owner, id, 500, 8.0);
    assert!(
        (end - goal).length() > 10.0,
        "exit endpoint deliberately differs from the captured Return post"
    );
    let initial = owner.host_object(id).unwrap().get_position();
    let mut reached = None;
    for _ in 0..150 {
        step(&mut owner);
        let state = observe(&owner, id);
        assert_eq!(state.quick, Some(500));
        assert_eq!(state.phase, Some(GuardPhase::Return { goal }));
        if (state.position - end).length() < 1.0 {
            reached = Some(state);
            break;
        }
    }
    let reached = reached.expect("real temporary movement must complete");
    assert_ne!(reached.position, initial);
    let processed = step(&mut owner);
    let idle = observe(&owner, id);
    assert_eq!(idle.quick, None);
    assert_eq!(
        idle.phase,
        Some(GuardPhase::Idle),
        "resumed Return observes the completed shared route before AIUpdate destroys it"
    );
    assert_eq!(idle.state, AIState::GuardingArea);
    assert!((processed..=processed + 7).contains(&idle.deadline.unwrap()));
    assert_eq!(
        idle.destination, None,
        "no forced replacement Return order may be issued before observing completion"
    );
    assert!(idle.path.is_empty());
}

#[test]
fn suspended_overlay_snapshot_preserves_future_full_step_continuation() {
    let (mut source, id) = idle_world();
    source
        .host_object_mut(id)
        .unwrap()
        .unit_ai_runtime
        .set_guard_scan_deadline(Some(11));
    overlay(&mut source, id, 10, 200.0);
    source.frame = 9;
    step(&mut source);
    let builder = crate::save_load::SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&source).unwrap();
    let encoded = bincode_legacy::serialize(&saved).unwrap();
    let decoded: crate::save_load::snapshot::WorldSnapshot =
        bincode_legacy::deserialize(&encoded).unwrap();
    let (mut restored, restored_id, _) = world();
    assert_eq!(id, restored_id);
    authored_rates(&mut restored, 60, 7);
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert_eq!(
        observe(&restored, id),
        observe(&source, id),
        "restoration is inert, including suspended phase/path/deadline"
    );
    for _ in 0..12 {
        step(&mut source);
        step(&mut restored);
        assert_eq!(
            observe(&restored, id),
            observe(&source, id),
            "future continuation including expiry and resumed Idle scan"
        );
    }
    assert_eq!(observe(&source, id).quick, None);
    assert_eq!(observe(&source, id).phase, Some(GuardPhase::Idle));
}

#[test]
fn same_id_world_temporary_and_base_state_updates_are_independent() {
    let (mut first, id) = idle_world();
    let (mut second, second_id) = idle_world();
    assert_eq!(id, second_id);
    authored_rates(&mut second, 90, 11);
    first
        .host_object_mut(id)
        .unwrap()
        .unit_ai_runtime
        .set_guard_scan_deadline(Some(11));
    second
        .host_object_mut(id)
        .unwrap()
        .unit_ai_runtime
        .set_guard_scan_deadline(Some(41));
    overlay(&mut first, id, 10, 200.0);
    overlay(&mut second, id, 40, 300.0);
    first.frame = 11;
    second.frame = 41;
    let initial_second = observe(&second, id);
    step(&mut first);
    assert_eq!(observe(&second, id), initial_second);
    let first_after = observe(&first, id);
    assert_eq!(first_after.deadline, Some(18));
    step(&mut second);
    assert_eq!(observe(&first, id), first_after);
    assert_eq!(observe(&second, id).deadline, Some(52));
    second.reset();
    drop(second);
    assert_eq!(observe(&first, id), first_after);
    step(&mut first);
    assert_eq!(observe(&first, id).deadline, Some(18));
}

#[test]
fn actual_tunnel_sally_installs_and_suspends_owned_guard_overlay() {
    let (mut owner, id, enemy) = world();
    authored_rates(&mut owner, 60, 7);
    step(&mut owner);
    let mut tunnel = ThingTemplate::new("OwnedQuickExitTunnel");
    tunnel.add_kind_of(KindOf::Structure).set_health(1000.0);
    tunnel.contain_module.kind = crate::game_logic::ContainModuleKind::Tunnel;
    tunnel.contain_module.slots = Some(crate::game_logic::host_tunnel_network::MAX_TUNNEL_CAPACITY);
    owner.templates.insert(tunnel.name.clone(), tunnel);
    let tunnel = owner
        .create_object_for_player("OwnedQuickExitTunnel", 1, Vec3::new(200.0, 0.0, 200.0))
        .unwrap();
    owner
        .host_object_mut(tunnel)
        .unwrap()
        .install_tunnel_network_residual();
    owner.host_object_mut(id).unwrap().close_enough_dist = Some(1.0);
    assert!(owner.unit_command_guard_object(id, tunnel));
    let unit = owner.host_object_mut(id).unwrap();
    unit.stop_moving();
    unit.unit_ai_runtime.set_guard_phase(Some(GuardPhase::Idle));
    unit.unit_ai_runtime.set_guard_scan_deadline(Some(500));
    unit.unit_ai_runtime
        .set_guard_anchor(Some(Vec3::new(200.0, 0.0, 200.0)));
    unit.set_contained_by(Some(tunnel));
    let key = owner.host_object(tunnel).unwrap().tunnel_system_key();
    owner.tunnel_network.on_tunnel_created(key, tunnel);
    assert!(owner.tunnel_network.record_enter(key, id, tunnel));
    owner
        .players
        .get_mut(&1)
        .unwrap()
        .set_map_relationship(2, gamelogic::common::Relationship::Enemies);
    owner.host_object_mut(tunnel).unwrap().target = Some(enemy);
    // Admission uses actual TunnelContain/AITNGuard producer; acceptance below
    // drives the whole fixed-step boundary, not another support-only update.
    owner.update_support_states(&[tunnel, id, enemy], 1.0 / 30.0);
    let admitted = observe(&owner, id);
    assert_eq!(owner.host_object(id).unwrap().contained_by, None);
    assert_eq!(admitted.state, AIState::Moving);
    assert_eq!(admitted.phase, Some(GuardPhase::Idle));
    assert_eq!(admitted.deadline, Some(500));
    assert_eq!(admitted.quick, Some(owner.frame + 300));
    assert!(admitted.through_units && !admitted.adjust);
    assert_eq!(admitted.path.len(), 2);
    assert!((admitted.position - *admitted.path.last().unwrap()).length() > 1.0);
    let deadline = owner.frame + 300;
    // Isolate the original end-frame inclusion on this admitted route without
    // manufacturing any exit flags or replacing its actual door path.
    owner.frame = deadline - 1;
    step(&mut owner);
    let before_deadline = observe(&owner, id);
    assert_eq!(
        before_deadline.quick,
        Some(deadline),
        "real exit must remain unfinished after warmup: {before_deadline:?}, admitted: {admitted:?}"
    );
    owner
        .host_object_mut(id)
        .unwrap()
        .unit_ai_runtime
        .set_guard_scan_deadline(Some(deadline + 20));
    step(&mut owner);
    let active = observe(&owner, id);
    assert_eq!(
        active.quick,
        Some(deadline),
        "inclusive deadline observation: {active:?}, before: {before_deadline:?}"
    );
    assert_eq!(active.phase, Some(GuardPhase::Idle));
    assert_eq!(active.deadline, Some(deadline + 20));
    assert_ne!(
        active.position, before_deadline.position,
        "producer exit route must move on its inclusive end frame"
    );
}

// C++ AIGuard.cpp:690-735: due Idle observes >2-cell guardee drift and
// enters Return. AIStates.cpp:1584/1612 calls friend_startingMove; the
// AIUpdate.cpp:1018 completion cleanup must therefore not erase that request.
fn expired_object_overlay_fresh_return_cancellation(delayed: bool) {
    let (mut owner, id, enemy) = world();
    authored_rates(&mut owner, 60, 7);
    owner
        .host_object_mut(enemy)
        .unwrap()
        .set_position(Vec3::new(950.0, 0.0, 950.0));
    let guardee = owner
        .create_object_for_player("OwnedNamedInfantry", 1, Vec3::new(80.0, 0.0, 80.0))
        .unwrap();
    step(&mut owner); // Wake the real Movement module before order admission.
    assert!(owner.unit_command_guard_object(id, guardee));
    let end = *owner.host_object(id).unwrap().movement.path.last().unwrap();
    owner.host_object_mut(id).unwrap().set_position(end);
    owner.tick_host_guard_states(&[id]); // Initial Idle admission only.
    let idle = observe(&owner, id);
    assert_eq!(idle.phase, Some(GuardPhase::Idle));
    assert_eq!(idle.state, AIState::GuardingObject);
    let anchor = idle
        .anchor
        .expect("Object Idle captures its guardee anchor");
    owner
        .host_object_mut(id)
        .unwrap()
        .unit_ai_runtime
        .set_guard_scan_deadline(Some(11));
    let temporary_goal = overlay(&mut owner, id, 10, 200.0);
    owner.frame = 10;
    step(&mut owner);
    let suspended = observe(&owner, id);
    assert_eq!(suspended.quick, Some(10));
    assert_eq!(suspended.phase, Some(GuardPhase::Idle));
    assert!((suspended.position - temporary_goal).length() > 20.0);
    // More than two cells on both ground axes; no RNG-dependent acquisition.
    let moved_anchor = anchor + Vec3::new(60.0, 0.0, 40.0);
    owner
        .host_object_mut(guardee)
        .unwrap()
        .set_position(moved_anchor);
    if delayed {
        // Real requestPath's recent-path throttle, not a fake waiting flag.
        // A fresh frame11 request must remain pending instead of being erased.
        owner.host_object_mut(id).unwrap().path_timestamp = 10;
    } else {
        owner.host_object_mut(id).unwrap().path_timestamp = 0;
    }
    let mut adjusted_goal = moved_anchor;
    owner.adjust_guard_goal(id, &mut adjusted_goal);
    assert_eq!(step(&mut owner), 11);
    let resumed = observe(&owner, id);
    assert_eq!(resumed.quick, None);
    assert_eq!(resumed.state, AIState::GuardingObject);
    let Some(GuardPhase::Return { goal }) = resumed.phase else {
        panic!("due resumed Idle must admit a fresh Return after guardee drift")
    };
    assert_eq!(
        goal, adjusted_goal,
        "Return captures the adjusted moved guardee goal"
    );
    assert_eq!(resumed.anchor, Some(moved_anchor));
    assert_eq!(resumed.updated, Some(11));
    assert_eq!(resumed.destination, Some(goal));
    assert_ne!(resumed.destination, Some(temporary_goal));
    assert!(
        resumed.moving,
        "friend_startingMove cancels pending movement completion"
    );
    assert!(
        !resumed.path.is_empty(),
        "post-base completion cannot destroy the live route"
    );
    assert!(resumed.adjust && !resumed.through_units);
    if delayed {
        assert!(
            resumed.waiting,
            "recent-path admission must preserve the actual pending request"
        );
        assert!(owner.host_object(id).unwrap().queue_for_path_frames > 0);
        assert_eq!(
            resumed.path.last().copied(),
            Some(temporary_goal),
            "pending request retains the shared predecessor route until actual replacement"
        );
    } else {
        assert!(!resumed.waiting);
        assert_eq!(resumed.path_goal, Some(goal));
        assert_ne!(resumed.path.last().copied(), Some(temporary_goal));
        let initial_distance = (resumed.position - goal).length();
        for _ in 0..15 {
            step(&mut owner);
        }
        assert!(
            (owner.host_object(id).unwrap().get_position() - goal).length() < initial_distance,
            "the surviving replacement request must drive real locomotion"
        );
    }
}

#[test]
fn expired_object_overlay_due_idle_fresh_return_survives_completion_cleanup() {
    expired_object_overlay_fresh_return_cancellation(false);
}

#[test]
fn expired_object_overlay_due_idle_delayed_return_survives_completion_cleanup() {
    expired_object_overlay_fresh_return_cancellation(true);
}
