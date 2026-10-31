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
fn malformed_terrain_is_rejected_before_replacing_a_live_session() {
    use generals_main::game_logic::{Player, Team, ThingTemplate};
    use glam::Vec3;

    let builder = SnapshotBuilder::new();
    let mut source = GameLogic::new();
    source.add_player(Player::new(7, Team::USA, "Receiving player", true));
    for _ in 0..3 {
        source.update_with_dt(1.0 / 30.0);
    }
    for malformed in 0..3 {
        let mut receiving = GameLogic::new();
        receiving.add_player(Player::new(7, Team::USA, "Receiving player", true));
        receiving
            .templates
            .insert("RestoreProbe".into(), ThingTemplate::new("RestoreProbe"));
        let object = receiving
            .create_object_for_player("RestoreProbe", 7, Vec3::ZERO)
            .unwrap();
        receiving.update_with_dt(0.04);
        let frame = receiving.get_frame();
        let time = receiving.update_ui_state(0).current_game_time;
        let mut snapshot = builder.create_world_snapshot(&source).unwrap();
        match malformed {
            0 => {
                snapshot.terrain.logic_width = 2;
                snapshot.terrain.logic_height = 2;
                snapshot.terrain.logic_heights = vec![0; 3];
            }
            1 => {
                snapshot.terrain.width = 2;
                snapshot.terrain.height = 2;
                snapshot.terrain.height_map = vec![0.0; 3];
            }
            _ => {
                snapshot.terrain.width = 2;
                snapshot.terrain.height = 2;
                snapshot.terrain.passability_map = vec![true; 3];
            }
        }
        let error = builder
            .restore_from_snapshot(&snapshot, &mut receiving)
            .unwrap_err();
        let expected = match malformed {
            0 => "invalid complete terrain byte grid",
            1 => "invalid terrain height grid",
            _ => "invalid terrain passability grid",
        };
        assert!(error.to_string().contains(expected), "{error:?}");
        assert_eq!(receiving.get_frame(), frame, "malformed case {malformed}");
        assert_eq!(receiving.update_ui_state(0).current_game_time, time);
        assert!(receiving.get_player(7).is_some());
        assert!(receiving.host_object(object).is_some());
        assert_eq!(source.get_frame(), 3);
        assert!(source.get_player(7).is_some());
    }
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

#[test]
fn simulation_clock_follows_logic_steps_not_presentation_timestamps() {
    use generals_main::game_logic::{Player, Team};
    use std::time::{Duration, Instant};
    use ww3d_engine::FrameTiming;

    let timing = |total_seconds: u64| FrameTiming {
        frame_number: 123,
        delta_time: Duration::from_secs_f32(1.0 / 120.0),
        total_time: Duration::from_secs(total_seconds),
        fps: 120.0,
        frame_start: Instant::now(),
        sync_time: 0,
        previous_sync_time: 0,
    };
    let mut first = GameLogic::new();
    let mut second = GameLogic::new();
    for world in [&mut first, &mut second] {
        world.add_player(Player::new(0, Team::USA, "Human", true));
        world.add_player(Player::new(1, Team::GLA, "AI", false));
        world.setup_skirmish_ai(0);
        assert_eq!(world.host_ai_player_count(), 1);
    }
    let mut saw_ai_update = false;
    for _ in 0..12 {
        let a = first.update_with_timing(&timing(10));
        let b = second.update_with_timing(&timing(100));
        assert_eq!((a.frame, a.steps_run), (b.frame, b.steps_run));
        assert_eq!(
            first.update_ui_state(0).current_game_time,
            second.update_ui_state(0).current_game_time
        );
        assert!(
            (first.update_ui_state(0).current_game_time - a.frame as f32 / 30.0).abs() < 0.00001
        );
        let first_ai = first.capture_ai_player_queue_persist();
        let second_ai = second.capture_ai_player_queue_persist();
        assert_eq!(first_ai.len(), 1);
        assert_eq!(second_ai.len(), 1);
        assert_eq!(first_ai[0].last_update_time, second_ai[0].last_update_time);
        if a.frame > 1 {
            assert!((first_ai[0].last_update_time - (a.frame - 1) as f32 / 30.0).abs() < 0.00001);
            saw_ai_update = true;
        }
    }
    assert!(saw_ai_update, "the owned AI must actually update");

    let builder = SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&first).unwrap();
    let mut bytes = Cursor::new(Vec::new());
    snapshot.xfer(&mut XferSave::new(&mut bytes)).unwrap();
    let mut snapshot = WorldSnapshot::default();
    snapshot
        .xfer(&mut XferLoad::new(Cursor::new(bytes.into_inner())))
        .unwrap();
    let mut restored = GameLogic::new();
    restored.add_player(Player::new(0, Team::USA, "Human", true));
    restored.add_player(Player::new(1, Team::GLA, "AI", false));
    restored.setup_skirmish_ai(0);
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    for _ in 0..12 {
        let a = first.update_with_timing(&timing(10));
        let b = restored.update_with_timing(&timing(1000));
        assert_eq!((a.frame, a.steps_run), (b.frame, b.steps_run));
        let a_ai = first.capture_ai_player_queue_persist();
        let b_ai = restored.capture_ai_player_queue_persist();
        assert_eq!(a_ai.len(), 1);
        assert_eq!(b_ai.len(), 1);
        assert_eq!(a_ai[0].last_update_time, b_ai[0].last_update_time);
        assert_eq!(a_ai[0].next_team_time, b_ai[0].next_team_time);
        assert_eq!(a_ai[0].next_building_time, b_ai[0].next_building_time);
    }
}
