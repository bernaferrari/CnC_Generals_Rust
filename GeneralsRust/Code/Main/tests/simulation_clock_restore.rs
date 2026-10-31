use generals_main::game_logic::GameLogic;
use generals_main::save_load::{Snapshot, SnapshotBuilder, WorldSnapshot, XferLoad, XferSave};
use std::io::Cursor;

#[test]
fn snapshot_restores_session_clock_and_partial_fixed_step() {
    let mut original = GameLogic::new();
    let initial = original.update_with_dt(0.05);
    assert_eq!(initial.steps_run, 1);
    assert!(initial.accumulated_time_seconds > 0.0);
    let saved_time = original.update_ui_state(0).current_game_time;
    let builder = SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&original).unwrap();
    let mut bytes = Cursor::new(Vec::new());
    snapshot.xfer(&mut XferSave::new(&mut bytes)).unwrap();
    let mut snapshot = WorldSnapshot::default();
    snapshot
        .xfer(&mut XferLoad::new(Cursor::new(bytes.into_inner())))
        .unwrap();

    let deltas = [0.01, 0.015, 0.025, 0.04, 0.005];
    let mut expected = Vec::new();
    for delta in deltas {
        let timing = original.update_with_dt(delta);
        expected.push((
            timing.frame,
            timing.steps_run,
            timing.accumulated_time_seconds,
            original.update_ui_state(0).current_game_time,
        ));
    }
    let mut fresh = GameLogic::new();
    builder
        .restore_from_snapshot(&snapshot, &mut fresh)
        .unwrap();
    builder
        .restore_from_snapshot(&snapshot, &mut original)
        .unwrap();
    assert_eq!(fresh.update_ui_state(0).current_game_time, saved_time);
    assert_eq!(original.update_ui_state(0).current_game_time, saved_time);
    for (delta, expected) in deltas.into_iter().zip(expected) {
        for world in [&mut fresh, &mut original] {
            let timing = world.update_with_dt(delta);
            assert_eq!(
                (
                    timing.frame,
                    timing.steps_run,
                    timing.accumulated_time_seconds,
                    world.update_ui_state(0).current_game_time
                ),
                expected
            );
        }
    }
}

#[test]
fn invalid_clock_is_rejected_before_mutating_a_session() {
    let mut world = GameLogic::new();
    world.update_with_dt(0.05);
    let builder = SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&world).unwrap();
    let frame = world.get_frame();
    let time = world.update_ui_state(0).current_game_time;
    snapshot
        .global_ai_state
        .global_timers
        .insert("session_clock_seconds".into(), f32::NAN);
    assert!(
        builder
            .restore_from_snapshot(&snapshot, &mut world)
            .is_err()
    );
    assert_eq!(world.get_frame(), frame);
    assert_eq!(world.update_ui_state(0).current_game_time, time);
    snapshot
        .global_ai_state
        .global_timers
        .insert("session_clock_seconds".into(), time);
    snapshot
        .global_ai_state
        .global_timers
        .insert("session_accumulated_seconds".into(), -0.1);
    assert!(
        builder
            .restore_from_snapshot(&snapshot, &mut world)
            .is_err()
    );
    assert_eq!(world.get_frame(), frame);
    assert_eq!(world.update_ui_state(0).current_game_time, time);
}

#[test]
fn older_snapshot_uses_frame_derived_time_and_no_partial_step() {
    let mut world = GameLogic::new();
    world.update_with_dt(0.05);
    let builder = SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&world).unwrap();
    snapshot
        .global_ai_state
        .global_timers
        .remove("session_clock_seconds");
    snapshot
        .global_ai_state
        .global_timers
        .remove("session_accumulated_seconds");
    let mut receiver = GameLogic::new();
    builder
        .restore_from_snapshot(&snapshot, &mut receiver)
        .unwrap();
    assert_eq!(
        receiver.update_ui_state(0).current_game_time,
        snapshot.frame_number as f32 / 30.0
    );
    assert_eq!(receiver.update_with_dt(0.01).steps_run, 0);
}
