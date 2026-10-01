//! TEMPORARY validation for the owned turret state machine refactor.
//! Mirrors the `#[cfg(test)]` unit tests in `src/ai/turret.rs`, which cannot be
//! built right now because unrelated object/** test files are in flight.
//! Delete this file after running.

use gamelogic::ai::turret::{TurretStateType, TurretAI, TurretTargetKind};
use gamelogic::common::Coord3D;
use gamelogic::helpers::TheGameLogic;
use gamelogic::state_machine::StateReturnType;
use std::sync::{Arc, Mutex, Weak};

fn test_turret() -> TurretAI {
    TurretAI::new(Weak::new())
}

#[test]
fn turret_defaults_match_cpp_runtime_fields() {
    let turret = test_turret();
    assert_eq!(turret.get_continuous_fire_expiration_frame(), u32::MAX);
    assert_eq!(turret.get_sleep_until(), 0);
    assert!(!turret.get_play_rot_sound());
    assert!(!turret.get_play_pitch_sound());
    assert!(!turret.get_did_fire());
}

#[test]
fn turret_rotation_and_pitch_set_sound_flags_when_moving() {
    let mut turret = test_turret();
    turret.set_turn_rate(0.1);
    turret.set_pitch_rate(0.1);
    turret.set_allows_pitch(true);

    assert!(!turret.rotate_towards_angle(1.0));
    assert!(turret.get_play_rot_sound());

    assert!(!turret.pitch_towards_angle(1.0));
    assert!(turret.get_play_pitch_sound());
}

#[test]
fn turret_wake_and_sleep_bookkeeping_matches_cpp_fields() {
    let mut turret = test_turret();
    let now = TheGameLogic::get_frame();

    turret.set_sleep_until(now.saturating_add(5));
    assert_eq!(turret.update_turret_ai(), StateReturnType::Sleep(5));

    turret.friend_notify_state_machine_changed();
    assert_eq!(turret.get_sleep_until(), now);

    turret.set_turret_enabled(false);
    turret.set_turret_enabled(true);
    assert_eq!(turret.get_sleep_until(), now);
}

#[test]
fn set_target_position_marks_position_kind() {
    let mut turret = test_turret();
    turret.set_target_position(Some(Coord3D::new(10.0, 0.0, 5.0)));
    assert_eq!(turret.get_target_kind(), TurretTargetKind::Position);
    assert!(turret.get_target_position().is_some());
    assert!(turret.get_current_target_id().is_none());
}

#[test]
fn disabled_turret_without_recenter_sleeps() {
    let mut turret = test_turret();
    turret.set_turret_enabled(false);
    match turret.update_turret_ai() {
        StateReturnType::Sleep(_) => {}
        other => panic!("expected sleep when disabled and not recentering, got {other:?}"),
    }
}

#[test]
fn owned_machine_builds_and_steps_through_shared_handle() {
    // Builds the machine the way UnitAIUpdate does (ai_core.rs build_turret_machine).
    let turret_ai = Arc::new(Mutex::new(TurretAI::new(Weak::new())));
    let machine = gamelogic::ai::turret::TurretStateMachine::new(
        Some(turret_ai.clone()),
        Weak::new(),
        "TurretAI",
    );
    let handle = machine.get_turret_ai().expect("handle");
    let idle_id = u32::from(TurretStateType::Idle);

    // Same chain object/unit/ai_interface_update.rs uses for still_idle.
    let still_idle = handle
        .lock()
        .ok()
        .and_then(|guard| guard.export_idle_goal().0)
        .and_then(|weak| weak.upgrade())
        .and_then(|machine| machine.lock().ok().and_then(|guard| guard.get_current_state_id()))
        == Some(idle_id);
    assert!(still_idle, "fresh turret must start in TURRETAI_IDLE");

    // Machine goal sync through the exported handle.
    let goal = Coord3D::new(30.0, 0.0, 0.0);
    handle.lock().unwrap().set_target_position(Some(goal));
    let (machine_handle, kind, target, pos) = handle.lock().unwrap().export_idle_goal();
    assert_eq!(kind, TurretTargetKind::Position);
    assert!(target.is_none());
    assert_eq!(pos, Some(goal));
    TurretAI::sync_machine_goal(machine_handle, kind, target, pos);
    let aim_id = u32::from(TurretStateType::Aim);
    let state = handle.lock().unwrap().get_current_state_id();
    assert_eq!(state, Some(aim_id), "position goal must force TURRETAI_AIM");

    // Recenter through the owned machine.
    handle.lock().unwrap().recenter_turret();
    let state = handle.lock().unwrap().get_current_state_id();
    assert_eq!(state, Some(u32::from(TurretStateType::Recenter)));

    // A full step stays inside the owned machine and reports Continue.
    handle.lock().unwrap().set_turret_enabled(true);
    let result = TurretAI::update_turret_ai_handle(&handle);
    assert!(matches!(result, StateReturnType::Continue | StateReturnType::Sleep(0)));
}

#[test]
fn fire_pitch_overrides_computed_aim_pitch() {
    let mut turret = test_turret();
    turret.set_allows_pitch(true);
    turret.set_fire_pitch(0.5);
    turret.set_ground_unit_pitch(0.25);
    let origin = Coord3D::new(0.0, 0.0, 0.0);
    let target = Coord3D::new(100.0, 0.0, 50.0);
    assert!(
        (turret.compute_desired_aim_pitch(&origin, &target, 20.0, 200.0) - 0.5).abs() < 1e-5
    );
}

#[test]
fn friend_turn_aligns_within_rel_thresh() {
    let mut turret = test_turret();
    turret.set_turn_rate(0.2);
    assert!(!turret.friend_turn_towards_angle(1.0, 1.0, 0.035));
    assert!(turret.get_play_rot_sound());
    for _ in 0..20 {
        if turret.friend_turn_towards_angle(1.0, 1.0, 0.035) {
            break;
        }
    }
    assert!(turret.friend_turn_towards_angle(1.0, 1.0, 0.035));
}
