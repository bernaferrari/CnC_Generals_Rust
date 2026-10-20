//! Existing host tick tests; module/test identities are unchanged.
use super::*;
use game_engine::common::message_stream::GameMessageType;
static RANDOM_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// C++ GameLogic.cpp:3600 + 3614-3616 — scripts run BEFORE the freeze
/// return, but terrain/commands/modules never do: frame and sim time stay
/// put while the accumulated timestep is still consumed (the host ticked
/// a frozen frame).
#[test]
fn frozen_step_burns_time_advances_nothing_and_still_runs_scripts() {
    let mut logic = GameLogic::new();
    logic.scripts_loaded = true;
    logic.set_script_time_frozen_for_test(true);
    logic.mission_scripts.push_radar_forced(true);

    logic.step_simulation_with_budget(LOGIC_FRAME_TIMESTEP, None, None);

    assert_eq!(logic.frame, 0, "frozen step must not advance the frame");
    assert_eq!(
        logic.sim_time_seconds, 0.0,
        "frozen step must not advance sim time"
    );
    assert_eq!(
        logic.accumulated_time, 0.0,
        "frozen step must still consume the accumulated timestep"
    );
    let diag = logic.fixed_step_diagnostics();
    assert_eq!(diag.steps_run, 0);
    assert_eq!(diag.frozen_steps, 1, "frozen step must be counted");
    assert!(!diag.budget_hit);
    // Scripts evaluated before the freeze return (C++ line 3600): the
    // queued radar-forced action is applied even on the frozen frame.
    let ui = logic.update_ui_state(0);
    assert!(
        ui.radar_forced,
        "scripts must still evaluate on a frozen frame (GameLogic.cpp:3600)"
    );
}

fn host_ai_last_update_time(logic: &GameLogic) -> f32 {
    logic
        .ai_manager
        .ai_players
        .get(&1)
        .expect("host AI player")
        .last_update_time
}

fn add_test_host_ai(logic: &mut GameLogic) {
    logic.add_player(crate::game_logic::Player::new(
        1,
        crate::game_logic::Team::USA,
        "Test AI",
        false,
    ));
    logic.add_ai_opponent(
        1,
        crate::game_logic::Team::USA,
        crate::ai::AIDifficulty::Easy,
    );
}

#[test]
fn host_ai_runs_once_for_each_advanced_fixed_logic_frame() {
    let mut logic = GameLogic::new();
    add_test_host_ai(&mut logic);

    for expected_frame in 1..=12 {
        // C++ calls TheAI->UPDATE before advancing the frame's simulation
        // clock, so AI observes the time at the beginning of each tick.
        let tick_time = logic.sim_time_seconds;
        logic.step_simulation_with_budget(LOGIC_FRAME_TIMESTEP, None, None);
        assert_eq!(logic.frame, expected_frame);
        assert_eq!(
            host_ai_last_update_time(&logic).to_bits(),
            tick_time.to_bits(),
            "C++ GameLogic::update calls TheAI->UPDATE once on frame {expected_frame}"
        );
    }
}

#[test]
fn host_ai_does_not_run_on_frozen_steps_and_resumes_after_unfreeze() {
    let mut logic = GameLogic::new();
    add_test_host_ai(&mut logic);
    let initial_ai_time = host_ai_last_update_time(&logic);
    logic.set_script_time_frozen_for_test(true);
    logic.step_simulation_with_budget(LOGIC_FRAME_TIMESTEP, None, None);
    assert_eq!(logic.frame, 0);
    assert_eq!(host_ai_last_update_time(&logic), initial_ai_time);

    logic.set_script_time_frozen_for_test(false);
    let resumed_tick_time = logic.sim_time_seconds;
    logic.step_simulation_with_budget(LOGIC_FRAME_TIMESTEP, None, None);
    assert_eq!(logic.frame, 1);
    assert_eq!(
        host_ai_last_update_time(&logic).to_bits(),
        resumed_tick_time.to_bits()
    );
}

#[test]
fn host_ai_catch_up_updates_every_internal_fixed_step() {
    let mut logic = GameLogic::new();
    add_test_host_ai(&mut logic);
    let first_tick_time = logic.sim_time_seconds;
    logic.step_simulation_with_budget(4.0 * LOGIC_FRAME_TIMESTEP, None, Some(4));

    assert_eq!(logic.frame, 4);
    let final_ai_tick_time = first_tick_time + 3.0 * LOGIC_FRAME_TIMESTEP;
    assert_eq!(
        host_ai_last_update_time(&logic).to_bits(),
        final_ai_tick_time.to_bits(),
        "catch-up must call AI for its final advanced logic frame"
    );
    assert_eq!(logic.fixed_step_diagnostics().steps_run, 4);
}

/// C++ skips m_frame++ on frozen frames (early return at 3614-3616) and
/// resumes the sequence once freezeTime drops — no skipped or repeated
/// frame numbers.
#[test]
fn unfreeze_resumes_on_next_frame_without_skips() {
    let mut logic = GameLogic::new();
    logic.set_script_time_frozen_for_test(true);
    logic.step_simulation_with_budget(LOGIC_FRAME_TIMESTEP, None, None);
    logic.step_simulation_with_budget(LOGIC_FRAME_TIMESTEP, None, None);
    assert_eq!(logic.frame, 0, "frozen frames must not advance the frame");

    logic.set_script_time_frozen_for_test(false);
    logic.step_simulation_with_budget(LOGIC_FRAME_TIMESTEP, None, None);
    assert_eq!(logic.frame, 1, "unfreeze resumes on the very next frame");
    logic.step_simulation_with_budget(LOGIC_FRAME_TIMESTEP, None, None);
    assert_eq!(logic.frame, 2, "no repeated frames after unfreeze");
    assert!((logic.sim_time_seconds - 2.0 * LOGIC_FRAME_TIMESTEP).abs() < 1e-6);
}

/// GameLogic.cpp3603-3617 and ScriptEngine.cpp8455-8464: actual
/// receiving tick clears debug only and falls through once; script/camera
/// freeze survive, and the peer's interpreter is untouched.
#[test]
fn frozen_with_pending_clear_game_data_falls_through() {
    let mut logic = GameLogic::new();
    let mut peer = GameLogic::new();
    logic
        .owned_script_engine
        .as_mut()
        .unwrap()
        .set_time_frozen_debug(true);
    peer.owned_script_engine
        .as_mut()
        .unwrap()
        .set_time_frozen_debug(true);
    logic.set_script_time_frozen_for_test(true);
    logic.session_view.scripted.script_camera_freeze_time = true;
    logic.session_view.camera.camera_zoom_target = Some(2.0);
    logic.session_view.camera.camera_zoom_duration = 1.0;
    assert!(logic.is_script_camera_time_frozen());
    logic
        .replay_pending
        .receive_client_messages(vec![game_engine::common::message_stream::GameMessage::new(
            GameMessageType::ClearGameData,
        )])
        .unwrap();

    logic.step_simulation_with_budget(LOGIC_FRAME_TIMESTEP, None, Some(1));
    assert_eq!(
        logic.frame, 1,
        "clear escape falls through to the full update"
    );
    assert!(!logic.script_debug_frozen(None).unwrap());
    assert!(
        logic.is_script_time_frozen(),
        "debug continue must preserve script freeze"
    );
    assert!(logic.is_script_camera_time_frozen());
    assert!(peer.script_debug_frozen(None).unwrap());
    let diag = logic.fixed_step_diagnostics();
    assert_eq!(diag.steps_run, 1);
    assert_eq!(diag.frozen_steps, 0);
    let lifecycle = logic.replay_pending.take_lifecycle_messages_owned();
    assert!(
        matches!(lifecycle.as_slice(), [message] if matches!(message.get_type(), GameMessageType::ClearGameData)),
        "peek retains the command until its normal Recorder/command phase"
    );
    logic.step_simulation_with_budget(LOGIC_FRAME_TIMESTEP, None, Some(1));
    assert_eq!(
        logic.frame, 1,
        "script/camera freeze still apply on the next frame"
    );
}

#[test]
fn receiving_tick_debug_freeze_and_foreign_clear_are_instance_isolated() {
    let mut logic = GameLogic::new();
    let mut peer = GameLogic::new();
    logic
        .owned_script_engine
        .as_mut()
        .unwrap()
        .set_time_frozen_debug(true);
    peer.replay_pending
        .receive_client_messages(vec![game_engine::common::message_stream::GameMessage::new(
            GameMessageType::ClearGameData,
        )])
        .unwrap();
    assert!(peer.replay_pending.receiving_clear_command_pending());
    logic.step_simulation_with_budget(LOGIC_FRAME_TIMESTEP, None, Some(1));
    assert_eq!(logic.frame, 0, "debug-only freeze stops the actual tick");
    assert!(logic.script_debug_frozen(None).unwrap());
    assert!(peer.replay_pending.receiving_clear_command_pending());
    logic
        .owned_script_engine
        .as_mut()
        .unwrap()
        .set_time_frozen_debug(false);
    logic.step_simulation_with_budget(LOGIC_FRAME_TIMESTEP, None, Some(1));
    assert_eq!(logic.frame, 1);
}

#[test]
fn foreign_message_stream_clear_cannot_escape_receiving_tick_freeze() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "foreign_message_stream_clear_cannot_escape_receiving_tick_freeze",
        || {
            let stream = game_engine::common::message_stream::get_message_stream();
            {
                let mut stream = stream.write().unwrap_or_else(|e| e.into_inner());
                stream.clear_messages();
                stream.append_message(GameMessageType::ClearGameData);
            }
            let mut logic = GameLogic::new();
            logic.set_script_time_frozen_for_test(true);
            logic.step_simulation_with_budget(LOGIC_FRAME_TIMESTEP, None, Some(1));
            assert_eq!(
                logic.frame, 0,
                "foreign MessageStream is not receiving CommandList"
            );
            assert!(logic.is_script_time_frozen());
            assert!(
                stream
                    .read()
                    .unwrap_or_else(|e| e.into_inner())
                    .contains_message_of_type(&GameMessageType::ClearGameData)
            );
        },
    );
}

/// The fixed-step batch publishes the driving instance's logic RNG as the
/// Common scoped owner: any logic draw the step makes consumes the
/// instance, never the process fallback. C++ RandomValue.cpp:150-174
/// supplies the range/seed reference; the user-approved Rust sequence is
/// session-owned, while standalone boot/Core callers retain the fallback.
#[test]
fn fixed_step_batch_does_not_consume_the_global_logic_stream() {
    let _guard = RANDOM_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    game_engine::common::random_value::init_random_with_seed(0x5EED_0F1);
    let global_words = game_engine::common::random_value::get_game_logic_random_seed_state();

    let mut logic = GameLogic::new();
    logic.step_simulation_with_budget(LOGIC_FRAME_TIMESTEP, None, None);

    // Whatever the step drew (if anything) went to the driving instance:
    // the global fallback words stay bit-identical to the fresh seed.
    assert_eq!(
        game_engine::common::random_value::get_game_logic_random_seed_state(),
        global_words,
        "in-tick logic draws must not consume the global fallback"
    );
}

/// Match setup and restart change only the selected driving stream.
#[test]
fn foreign_setup_reset_and_process_reseed_do_not_change_driving_rng() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "foreign_setup_reset_and_process_reseed_do_not_change_driving_rng",
        || {
            use game_engine::common::random_value::{
                get_game_logic_random_value, init_game_logic_random,
            };
            const SEED: u32 = 0x5EED_0F2;
            let mut actual = GameLogic::new();
            let mut reference = GameLogic::new();
            actual.set_logic_random_seed(SEED);
            reference.set_logic_random_seed(SEED);
            for index in 0..8 {
                let mut expected = Vec::new();
                reference.tick_logic_frame_with_boundary(
                    LOGIC_FRAME_TIMESTEP,
                    None,
                    Some(1),
                    |_| {
                        expected.push(get_game_logic_random_value(0, 999));
                        expected.push(get_game_logic_random_value(0, 999));
                    },
                );
                let mut foreign = GameLogic::new();
                foreign.set_logic_random_seed(0xB000_0000 + index);
                foreign.start_new_game(GameMode::Skirmish);
                foreign.tick_logic_frame(LOGIC_FRAME_TIMESTEP, None, Some(1));
                foreign.reset();
                assert_eq!(foreign.logic_base_seed, 0xB000_0000 + index);
                drop(foreign);
                // Engine/menu fallback reseeding is independent too. This
                // is outside A's scoped operation; it must not be adopted.
                init_game_logic_random(0xF000_0000 + index);
                let mut got = Vec::new();
                actual.tick_logic_frame_with_boundary(LOGIC_FRAME_TIMESTEP, None, Some(1), |_| {
                    got.push(get_game_logic_random_value(0, 999));
                    let mut nested = GameLogic::new();
                    nested.set_logic_random_seed(0xC000_0000 + index);
                    nested.reset();
                    // Another Core initializer within A's published scope
                    // changes only its process fallback, never A's stream.
                    init_game_logic_random(0xD000_0000 + index);
                    drop(nested);
                    got.push(get_game_logic_random_value(0, 999));
                });
                assert_eq!(got, expected, "first divergence at frame {}", index + 1);
                assert_eq!(
                    actual.logic_random.seed_words(),
                    reference.logic_random.seed_words()
                );
                assert_eq!(actual.logic_base_seed, SEED);
            }
        },
    );
}

#[test]
fn same_selected_seed_restart_repeats_owned_rng_without_broadcast() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "same_selected_seed_restart_repeats_owned_rng_without_broadcast",
        || {
            use game_engine::common::random_value::{
                get_game_logic_random_value, with_session_random,
            };
            let mut logic = GameLogic::new();
            logic.set_logic_random_seed(0x5EED_0F3);
            let fallback = game_engine::common::random_value::get_game_logic_random_seed_state();
            let first = with_session_random(&logic.logic_random, || {
                (0..4)
                    .map(|_| get_game_logic_random_value(0, 999))
                    .collect::<Vec<_>>()
            });
            let first_end = logic.logic_random.seed_words();
            logic.reset();
            assert_eq!(logic.logic_base_seed, 0x5EED_0F3);
            let second = with_session_random(&logic.logic_random, || {
                (0..4)
                    .map(|_| get_game_logic_random_value(0, 999))
                    .collect::<Vec<_>>()
            });
            assert_eq!(first, second);
            assert_eq!(logic.logic_random.seed_words(), first_end);
            assert_eq!(
                game_engine::common::random_value::get_game_logic_random_seed_state(),
                fallback
            );
        },
    );
}

/// C++ parity regression: the shroud looker pass (Phase 17) must run on
/// every logic frame even while the GameWorld shadow coupled dual-tick is
/// active (production default ON). The GW tick has no shroud/looker
/// channel of its own, so gating the pass off left the shroud grid and
/// player_visible_objects unfed on the live path. After 2-3 driven InGame
/// frames the local looker must appear in the shroud membership, reveal
/// real grid cells, and activate the presentation FOW snapshot.
#[test]
fn vision_feed_runs_on_canonical_owner_frames() {
    use crate::fow_rendering::FOWRenderingBridge;
    use gamelogic::system::shroud_manager::ShroudState;

    let mut logic = GameLogic::new();
    logic.add_player(crate::game_logic::Player::new(
        1,
        crate::game_logic::Team::USA,
        "USA",
        true,
    ));
    let mut tpl = crate::game_logic::ThingTemplate::new("VisionProbeC17");
    tpl.sight_range = 100.0;
    tpl.shroud_clearing_range = 240.0;
    logic.templates.insert("VisionProbeC17".into(), tpl);
    let _looker = logic
        .create_object_for_player("VisionProbeC17", 1, glam::Vec3::new(10.0, 0.0, 20.0))
        .expect("spawn looker");
    {
        let shroud = logic.world_services.shroud();
        let mut mgr = shroud.lock().expect("shroud");
        mgr.clear_all();
        mgr.init_shroud_grid(512.0, 512.0);
    }

    // Drive 3 InGame logic frames through the standard step loop while
    // the coupled tick is active — Phase 17 must still feed the shroud.
    for _ in 0..3 {
        logic.step_simulation_with_budget(LOGIC_FRAME_TIMESTEP, None, None);
    }

    {
        let shroud = logic.world_services.shroud();
        let mut mgr = shroud.lock().expect("shroud");
        assert!(
            !mgr.get_visible_objects(1).is_empty(),
            "own looker must appear in visible membership after coupled frames"
        );
        let cells = mgr
            .snapshot_grid_for_player(1)
            .expect("shroud grid initialized");
        assert!(
            cells.iter().any(|&c| c == ShroudState::Visible as u8),
            "looker circle must reveal cells on the shroud grid"
        );
    }
    // Freeze the same world after releasing its mutable vision borrow.
    let grid = FOWRenderingBridge::snapshot_terrain_grid(
        logic.world_services.shroud().lock().ok().as_deref(),
        1,
        false,
    );
    assert!(grid.active, "membership present: snapshot must be active");
    assert!(
        grid.to_r8_texture().iter().any(|&v| v != 255),
        "revealed looker circle must darken part of the R8 payload"
    );
    // Reset only this fixture's world.
    {
        let shroud = logic.world_services.shroud();
        let mut mgr = shroud.lock().expect("shroud");
        mgr.clear_all();
    }
}

/// Live catch-up clamp: with no explicit budget (the live drive_frame
/// path), a stall-fed backlog runs one fixed step per call and the
/// excess accumulated time is DROPPED instead of carried into later
/// frames (C++ GameEngine::update calls GameLogic::UPDATE once).
/// Headless callers passing Some(budget) keep the carry-over semantics.
#[test]
fn live_catchup_runs_one_step_and_drops_excess_backlog() {
    let mut logic = GameLogic::new();
    // 20 steps' worth of stall backlog on the live path (budget = None).
    logic.step_simulation_with_budget(20.0 * LOGIC_FRAME_TIMESTEP, None, None);

    assert_eq!(logic.frame, 1, "C++ live update advances one logic frame");
    let diag = logic.fixed_step_diagnostics();
    assert_eq!(diag.steps_run, 1);
    assert!(diag.budget_hit, "the clamp ceiling reports as budget_hit");
    assert!(
        diag.dropped_excess_time,
        "live clamp must drop the excess accumulated backlog"
    );
    assert_eq!(
        logic.accumulated_time, 0.0,
        "dropped backlog must not carry into the next drive_frame"
    );

    // Headless path unchanged: an explicit budget keeps the carry-over.
    let mut headless = GameLogic::new();
    headless.step_simulation_with_budget(20.0 * LOGIC_FRAME_TIMESTEP, None, Some(3));
    assert_eq!(headless.frame, 3);
    assert!(
        headless.accumulated_time >= LOGIC_FRAME_TIMESTEP,
        "headless budget path carries the remaining backlog"
    );
    assert!(!headless.fixed_step_diagnostics().dropped_excess_time);
}

/// Perf-gate seam: on a fresh world every residual registry is empty, so
/// each gated residual pass must be SKIPPED (counter bumps once per step)
/// while the step itself still advances normally — the gates are pure
/// performance cuts with no observable behavior when registries are empty.
#[test]
fn residual_gates_skip_empty_registries() {
    residual_gate_seam::reset_all();

    let mut logic = GameLogic::new();
    logic.step_simulation_with_budget(LOGIC_FRAME_TIMESTEP, None, None);

    // Every registry-backed residual pass must have skipped on the empty
    // world (each gate counter bumped once by this step).
    residual_gate_seam::assert_each_skipped_at_least_once();
    assert_eq!(
        logic.frame, 1,
        "the gated step must still advance exactly one frame"
    );
}
fn seed_boundary_test_owner(logic: &mut GameLogic, seed: u32) {
    logic.set_logic_random_seed(seed);
}

#[test]
fn step_boundary_callback_draws_from_the_driving_instance_in_sequence() {
    use game_engine::common::random_value::{get_game_logic_random_value, with_session_random};

    const SEED: u32 = 0xB0A7_DA7A;
    const DT: f32 = LOGIC_FRAME_TIMESTEP;
    let mut actual = GameLogic::new();
    let mut reference = GameLogic::new();
    seed_boundary_test_owner(&mut actual, SEED);
    seed_boundary_test_owner(&mut reference, SEED);
    let global_words = game_engine::common::random_value::get_game_logic_random_seed_state();
    let mut draws = Vec::new();
    let snapshot = actual.tick_logic_frame_with_boundary(2.0 * DT, None, Some(2), |_| {
        draws.push(get_game_logic_random_value(0, 999))
    });

    let _ = reference.update_with_dt_budget(DT, 1);
    let expected_first = with_session_random(&reference.logic_random, || {
        get_game_logic_random_value(0, 999)
    });
    let _ = reference.update_with_dt_budget(DT, 1);
    let expected_second = with_session_random(&reference.logic_random, || {
        get_game_logic_random_value(0, 999)
    });
    let actual_next =
        with_session_random(&actual.logic_random, || get_game_logic_random_value(0, 999));
    let expected_next = with_session_random(&reference.logic_random, || {
        get_game_logic_random_value(0, 999)
    });

    assert_eq!(snapshot.steps_run, 2);
    assert_eq!(draws, vec![expected_first, expected_second]);
    assert_eq!(actual_next, expected_next);
    assert_eq!(
        game_engine::common::random_value::get_game_logic_random_seed_state(),
        global_words,
        "step and callback draws leave the process fallback untouched"
    );
}

#[test]
fn nested_foreign_fixed_step_restores_the_outer_instance_stream() {
    use game_engine::common::random_value::{get_game_logic_random_value, with_session_random};

    const SEED: u32 = 0xA101_0001;
    const FOREIGN_SEED: u32 = 0xB202_0002;
    const DT: f32 = LOGIC_FRAME_TIMESTEP;
    let mut a = GameLogic::new();
    let mut a_ref = GameLogic::new();
    let mut b = GameLogic::new();
    let mut b_ref = GameLogic::new();
    seed_boundary_test_owner(&mut a, SEED);
    seed_boundary_test_owner(&mut a_ref, SEED);
    b.set_logic_random_seed(FOREIGN_SEED);
    b_ref.set_logic_random_seed(FOREIGN_SEED);

    let mut got_a = Vec::new();
    let mut got_b = Vec::new();
    let _ = a.tick_logic_frame_with_boundary(DT, None, Some(1), |_| {
        got_a.push(get_game_logic_random_value(0, 999));
        let _ = b.tick_logic_frame_with_boundary(DT, None, Some(1), |_| {
            got_b.push(get_game_logic_random_value(0, 999));
        });
        got_a.push(get_game_logic_random_value(0, 999));
    });

    let _ = a_ref.update_with_dt_budget(DT, 1);
    let expected_a_first =
        with_session_random(&a_ref.logic_random, || get_game_logic_random_value(0, 999));
    let _ = b_ref.update_with_dt_budget(DT, 1);
    let expected_b =
        with_session_random(&b_ref.logic_random, || get_game_logic_random_value(0, 999));
    let expected_a_second =
        with_session_random(&a_ref.logic_random, || get_game_logic_random_value(0, 999));

    assert_eq!(got_a, vec![expected_a_first, expected_a_second]);
    assert_eq!(got_b, vec![expected_b]);
    assert_eq!(a.get_frame(), 1);
    assert_eq!(b.get_frame(), 1);
}

#[test]
fn callback_unwind_restores_an_enclosing_logic_rng_owner() {
    use game_engine::common::random_value::{
        RandomState, get_game_logic_random_value, with_logic_rng_owner,
    };
    use std::panic::{AssertUnwindSafe, catch_unwind};

    let mut driver = GameLogic::new();
    seed_boundary_test_owner(&mut driver, 0xCA11_BACC);
    let mut outer = RandomState::default();
    outer.seed_random(0x0A77_EA01);
    let mut replay = RandomState::default();
    replay.seed_random(0x0A77_EA01);
    let expected_first = with_logic_rng_owner(&mut replay, || get_game_logic_random_value(0, 999));
    let expected_after = with_logic_rng_owner(&mut replay, || get_game_logic_random_value(0, 999));

    let (first, after) = with_logic_rng_owner(&mut outer, || {
        let first = get_game_logic_random_value(0, 999);
        let unwind = catch_unwind(AssertUnwindSafe(|| {
            let _ =
                driver.tick_logic_frame_with_boundary(LOGIC_FRAME_TIMESTEP, None, Some(1), |_| {
                    let _ = get_game_logic_random_value(0, 999);
                    panic!("exercise callback unwind after an actual fixed update");
                });
        }));
        assert!(unwind.is_err());
        // The tick's owner guard must have restored this enclosing owner.
        let after = get_game_logic_random_value(0, 999);
        (first, after)
    });

    assert_eq!(first, expected_first);
    assert_eq!(after, expected_after);
}
#[test]
fn immutable_frame_timing_keeps_the_existing_absolute_clock_rule() {
    let timing = FrameTiming {
        frame_number: 300,
        delta_time: Duration::from_secs_f32(1.0 / 30.0),
        total_time: Duration::from_secs(10),
        fps: 30.0,
        frame_start: Instant::now(),
        sync_time: 10_000,
        previous_sync_time: 9967,
    };
    let mut logic = GameLogic::new();
    let mut callbacks = 0;
    // timing.delta_seconds takes precedence over the dt argument as before.
    let snap =
        logic.tick_logic_frame_with_boundary(0.0, Some(&timing), Some(1), |_| callbacks += 1);
    assert_eq!(callbacks, 1);
    assert_eq!(snap.frame, 1);
    assert_eq!(logic.sim_time_seconds, 10.0);
    let mut frozen = GameLogic::new();
    frozen.set_script_time_frozen_for_test(true);
    let snap = frozen.tick_logic_frame_with_boundary(0.0, Some(&timing), Some(1), |_| {
        panic!("frozen step cannot deliver")
    });
    assert_eq!(snap.steps_run, 0);
    assert_eq!(frozen.sim_time_seconds, 0.0);
}
#[test]
fn host_logic_step_active_phase_restores_after_real_frozen_and_advanced_steps() {
    let mut logic = GameLogic::new();
    logic.scripts_loaded = true;
    logic.set_script_time_frozen_for_test(true);
    logic.step_simulation_with_budget(LOGIC_FRAME_TIMESTEP, None, None);
    assert_eq!(logic.frame, 0, "frozen update does not advance frame");
    assert_eq!(logic.fixed_step_diagnostics().frozen_steps, 1);
    assert!(
        logic.with_host_logic_step(|_| ()).is_some(),
        "real frozen return must drop the owner phase"
    );

    logic.set_script_time_frozen_for_test(false);
    logic.step_simulation_with_budget(LOGIC_FRAME_TIMESTEP, None, None);
    assert_eq!(logic.frame, 1, "advanced update advances one frame");
    assert_eq!(logic.fixed_step_diagnostics().steps_run, 1);
    assert!(
        logic.with_host_logic_step(|_| ()).is_some(),
        "real advanced return must drop the owner phase"
    );
}

#[test]
fn another_world_step_does_not_replace_the_driving_object_phase() {
    let mut first = GameLogic::new();
    first
        .with_host_logic_step(|first| {
            // Construction is inert even while a different instance is driving.
            let mut second = GameLogic::new();
            assert!(!second.host_logic_step_active);
            assert!(first.host_logic_step_active);
            second.step_simulation_with_budget(LOGIC_FRAME_TIMESTEP, None, Some(1));
            assert_eq!(second.frame, 1);
            assert!(!second.host_logic_step_active);
            assert!(first.host_logic_step_active);
            assert!(first.with_host_logic_step(|_| ()).is_none());
        })
        .expect("first owner scope");
    assert!(!first.host_logic_step_active);
}
#[test]
fn empty_scheduler_clock_boundary_admits_maximum_then_wraps_logic_once() {
    let mut logic = GameLogic::new();
    // Isolate UnsignedInt frame arithmetic. C++ sleepy priorities have a
    // 30-bit wake domain; recurring updates cannot complete at u32::MAX.
    // This is not a claim that a full match supports that frame value.
    for kind in super::super::HostSleepyKind::ALL {
        assert!(logic.host_sleepy.erase(kind));
    }
    assert_eq!(logic.host_sleepy.len(), 0);
    logic.frame = u32::MAX;
    let mut admitted = Vec::new();
    let mut completed = Vec::new();
    let snapshot = logic.tick_logic_frame_with_client_frame(
        LOGIC_FRAME_TIMESTEP,
        None,
        Some(1),
        |frame| admitted.push(frame),
        |owner| completed.push(owner.frame),
    );
    assert_eq!(admitted, vec![u32::MAX]);
    assert_eq!(completed, vec![0]);
    assert_eq!(snapshot.steps_run, 1);
    assert_eq!(snapshot.frame, 0);
    assert_eq!(logic.frame, 0);
}
