//! C++ GameEngine.cpp301 + GameLogic.cpp3669/3799: in-flight command tail
//! survives subsystem reset; the source frame increment observes NEW_GAME.
use super::*;
use game_engine::common::message_stream::{GameMessage, GameMessageArgumentType};
use game_engine::common::recorder::RecorderOwnerInputs;

fn fixture() -> (tempfile::TempDir, GameLogic) {
    let directory = tempfile::tempdir().unwrap();
    let mut world = GameLogic::new();
    world
        .replay_pending
        .reset_recorder_owned(RecorderOwnerInputs {
            map: "Maps/SourceCommandPhase".into(),
            seed: 37,
            replay_directory: directory.path().join("Replays"),
            exe_crc: 0,
            ini_crc: 0,
            save_stats: false,
            stats_directory: directory.path().join("Stats"),
        });
    world.frame = 37;
    world.sim_time_seconds = 37. * LOGIC_FRAME_TIMESTEP;
    (directory, world)
}
fn boundary(progress: NativeTickProgress) -> NativeTickContinuation {
    match progress {
        NativeTickProgress::CommandBoundary(pending) => pending,
        NativeTickProgress::Complete(_) => panic!("source command must release world loan"),
    }
}
fn begin(world: &mut GameLogic, messages: Vec<GameMessage>) -> NativeTickContinuation {
    world
        .replay_pending
        .receive_client_messages(messages)
        .unwrap();
    boundary(world.begin_native_logic_tick(
        LOGIC_FRAME_TIMESTEP,
        None,
        Some(1),
        Default::default(),
        |_| {},
        |_| panic!("outgoing object/observer callbacks ran before command completion"),
    ))
}

#[test]
fn actual_source_clear_suspends_before_callbacks_and_advances_reset_frame_once() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "actual_source_clear_suspends_before_callbacks_and_advances_reset_frame_once",
        || {
            let (_directory, mut world) = fixture();
            let mut pending = begin(
                &mut world,
                vec![GameMessage::new(GameMessageType::ClearGameData)],
            );
            assert_eq!(world.frame, 37, "score must observe outgoing frame37");
            assert!(
                !world.host_logic_step_active,
                "Engine reset has no active world loan"
            );
            assert!(matches!(
                pending.take_action(),
                NativeCommandAction::ClearGameData
            ));
            world.reset();
            let mut callbacks = 0;
            let result = world.resume_native_logic_tick(
                pending,
                NativeCommandCompletion::Cleared {
                    reset_performed: true,
                },
                Default::default(),
                |_| {},
                |receiving| {
                    callbacks += 1;
                    assert_eq!(receiving.frame, 1, "source tail runs against cleared world");
                    assert!(receiving.objects.is_empty());
                },
            );
            assert!(matches!(result, NativeTickProgress::Complete(_)));
            assert_eq!(callbacks, 1);
            assert_eq!(world.frame, 1, "never increment stale outgoing frame37");
            assert_eq!(world.accumulated_time, 0.);
            assert_eq!(world.sim_time_seconds, LOGIC_FRAME_TIMESTEP);
        },
    );
}

#[test]
fn actual_source_tail_retains_newgame_payload_and_prevents_reset_frame_increment() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "actual_source_tail_retains_newgame_payload_and_prevents_reset_frame_increment",
        || {
            let (_directory, mut world) = fixture();
            let mut newgame = GameMessage::new(GameMessageType::NewGame);
            for value in [3, 2, 71, 60] {
                newgame.append_integer_argument(value);
            }
            let mut pending = begin(
                &mut world,
                vec![GameMessage::new(GameMessageType::ClearGameData), newgame],
            );
            assert!(matches!(
                pending.take_action(),
                NativeCommandAction::ClearGameData
            ));
            world.reset();
            let mut pending = boundary(world.resume_native_logic_tick(
                pending,
                NativeCommandCompletion::Cleared {
                    reset_performed: true,
                },
                Default::default(),
                |_| {},
                |_| panic!("NEW_GAME must still be dispatched before object callbacks"),
            ));
            assert_eq!(world.frame, 0);
            let NativeCommandAction::PrepareNewGame(message) = pending.take_action() else {
                panic!("ordered NewGame tail lost during reset")
            };
            for (index, expected) in [3, 2, 71, 60].into_iter().enumerate() {
                assert!(
                    matches!(message.get_argument(index), Some(GameMessageArgumentType::Integer(value)) if *value == expected)
                );
            }
            let mut callbacks = 0;
            assert!(matches!(
                world.resume_native_logic_tick(
                    pending,
                    NativeCommandCompletion::NewGamePrepared { prepared: true },
                    Default::default(),
                    |_| {},
                    |_| callbacks += 1
                ),
                NativeTickProgress::Complete(_)
            ));
            assert_eq!(callbacks, 1);
            assert_eq!(world.frame, 0, "C++ !m_startNewGame guards final increment");
            assert_eq!(world.sim_time_seconds, 0.);
            assert!(
                world
                    .replay_pending
                    .take_lifecycle_messages_owned()
                    .is_empty()
            );
        },
    );
}

#[test]
fn later_source_clear_cancels_prepared_start_guard_without_dropping_tail() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "later_source_clear_cancels_prepared_start_guard_without_dropping_tail",
        || {
            let (_directory, mut world) = fixture();
            let mut pending = begin(
                &mut world,
                vec![
                    GameMessage::new(GameMessageType::NewGame),
                    GameMessage::new(GameMessageType::ClearGameData),
                ],
            );
            assert!(matches!(
                pending.take_action(),
                NativeCommandAction::PrepareNewGame(_)
            ));
            let mut pending = boundary(world.resume_native_logic_tick(
                pending,
                NativeCommandCompletion::NewGamePrepared { prepared: true },
                Default::default(),
                |_| {},
                |_| panic!("remaining clear not yet dispatched"),
            ));
            assert_eq!(world.frame, 37);
            assert!(matches!(
                pending.take_action(),
                NativeCommandAction::ClearGameData
            ));
            world.reset();
            assert!(matches!(
                world.resume_native_logic_tick(
                    pending,
                    NativeCommandCompletion::Cleared {
                        reset_performed: true
                    },
                    Default::default(),
                    |_| {},
                    |_| {}
                ),
                NativeTickProgress::Complete(_)
            ));
            assert_eq!(
                world.frame, 1,
                "later actual reset cancels earlier prepared-start guard"
            );
        },
    );
}

#[test]
fn source_prepare_updates_receiving_interpreter_and_rank_grants_only_humans() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "source_prepare_updates_receiving_interpreter_and_rank_grants_only_humans",
        || {
            let (_directory, mut receiving) = fixture();
            let mut foreign = GameLogic::new();
            foreign.prepare_native_command_difficulty(0);
            receiving.prepare_native_command_difficulty(2);
            receiving.prepare_native_command_mode(GameMode::SinglePlayer);
            assert_eq!(receiving.game_mode(), GameMode::SinglePlayer);
            assert_eq!(
                receiving
                    .owned_script_engine
                    .as_ref()
                    .unwrap()
                    .get_global_difficulty(),
                gamelogic::player::GameDifficulty::Hard
            );
            assert_eq!(
                foreign
                    .owned_script_engine
                    .as_ref()
                    .unwrap()
                    .get_global_difficulty(),
                gamelogic::player::GameDifficulty::Easy
            );
            receiving.players.clear();
            receiving.players.insert(
                0,
                crate::game_logic::Player::new(0, crate::game_logic::Team::USA, "Human", true),
            );
            receiving.players.insert(
                1,
                crate::game_logic::Player::new(
                    1,
                    crate::game_logic::Team::China,
                    "Computer",
                    false,
                ),
            );
            receiving.admit_native_initial_rank_points(37);
            assert_eq!(receiving.players[&0].skill_points, 37);
            assert_eq!(receiving.players[&1].skill_points, 0);
            receiving.prepare_native_command_mode(GameMode::Shell);
            receiving.admit_native_initial_rank_points(71);
            assert_eq!(
                receiving.players[&0].skill_points, 37,
                "Shell has no SP starting-rank grant"
            );
        },
    );
}

#[test]
fn source_cursor_rejects_a_foreign_receiving_world_before_mutating_it() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "source_cursor_rejects_a_foreign_receiving_world_before_mutating_it",
        || {
            let (_directory, mut source) = fixture();
            let mut foreign = GameLogic::new();
            foreign.frame = 19;
            let mut pending = begin(
                &mut source,
                vec![GameMessage::new(GameMessageType::ClearGameData)],
            );
            assert!(matches!(
                pending.take_action(),
                NativeCommandAction::ClearGameData
            ));
            let rejected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                foreign.resume_native_logic_tick(
                    pending,
                    NativeCommandCompletion::Cleared {
                        reset_performed: true,
                    },
                    Default::default(),
                    |_| {},
                    |_| panic!("foreign observer must never execute"),
                );
            }));
            assert!(rejected.is_err());
            assert_eq!(foreign.frame, 19);
            assert_eq!(source.frame, 37);
        },
    );
}
#[derive(Debug, PartialEq)]
struct BudgetReceipt {
    frame: u32,
    sim_time_bits: u32,
    carry_bits: u32,
    advanced: usize,
    frozen: usize,
    budget_hit: bool,
    dropped: bool,
    before: Vec<u32>,
    after: Vec<(u32, u32)>,
}

fn budget_receipt(
    mut message: Option<GameMessage>,
    late: bool,
    freeze_after_first: bool,
    budget: Option<usize>,
    delta: f32,
    absolute: Option<f32>,
) -> BudgetReceipt {
    let (_directory, mut world) = fixture();
    if !late {
        if let Some(message) = message.take() {
            world
                .replay_pending
                .receive_client_messages(vec![message])
                .unwrap();
        }
    }
    let timing = absolute.map(|total| FrameTiming {
        frame_number: 300,
        delta_time: std::time::Duration::from_secs_f32(delta),
        total_time: std::time::Duration::from_secs_f32(total),
        fps: 30.,
        frame_start: std::time::Instant::now(),
        sync_time: 10_000,
        previous_sync_time: 9_800,
    });
    let mut before_frames = Vec::new();
    let mut after_frames = Vec::new();
    {
        let mut before = |frame| before_frames.push(frame);
        let mut after = |receiving: &mut GameLogic| {
            after_frames.push((receiving.frame, receiving.sim_time_seconds.to_bits()));
            if freeze_after_first {
                receiving.set_script_time_frozen_for_test(true);
            }
            if let Some(message) = message.take() {
                receiving
                    .replay_pending
                    .receive_client_messages(vec![message])
                    .unwrap();
            }
        };
        let mut progress = world.begin_native_logic_tick(
            delta,
            timing.as_ref(),
            budget,
            Default::default(),
            &mut before,
            &mut after,
        );
        loop {
            let NativeTickProgress::CommandBoundary(mut cursor) = progress else {
                break;
            };
            let completion = match cursor.take_action() {
                NativeCommandAction::ClearGameData => NativeCommandCompletion::Cleared {
                    reset_performed: false,
                },
                NativeCommandAction::PrepareNewGame(_) => {
                    NativeCommandCompletion::NewGamePrepared { prepared: false }
                }
            };
            progress = world.resume_native_logic_tick(
                cursor,
                completion,
                Default::default(),
                &mut before,
                &mut after,
            );
        }
    }
    let diagnostics = world.fixed_step_diagnostics();
    BudgetReceipt {
        frame: world.frame,
        sim_time_bits: world.sim_time_seconds.to_bits(),
        carry_bits: world.accumulated_time.to_bits(),
        advanced: diagnostics.steps_run,
        frozen: diagnostics.frozen_steps,
        budget_hit: diagnostics.budget_hit,
        dropped: diagnostics.dropped_excess_time,
        before: before_frames,
        after: after_frames,
    }
}

#[test]
fn ignored_source_lifecycle_preserves_entire_original_fixed_step_budget() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "ignored_source_lifecycle_preserves_entire_original_fixed_step_budget",
        || {
            for (budget, delta, absolute, late, freeze) in [
                (Some(4), 4. * LOGIC_FRAME_TIMESTEP, None, false, false),
                (Some(4), 6. * LOGIC_FRAME_TIMESTEP, Some(10.), false, false),
                (None, 6. * LOGIC_FRAME_TIMESTEP, Some(10.), false, false),
                (Some(4), 6. * LOGIC_FRAME_TIMESTEP, Some(10.), true, false),
                (Some(4), 6. * LOGIC_FRAME_TIMESTEP, Some(10.), false, true),
            ] {
                let neutral = budget_receipt(None, late, freeze, budget, delta, absolute);
                for kind in [GameMessageType::ClearGameData, GameMessageType::NewGame] {
                    let receiving = budget_receipt(
                        Some(GameMessage::new(kind)),
                        late,
                        freeze,
                        budget,
                        delta,
                        absolute,
                    );
                    assert_eq!(
                        receiving, neutral,
                        "ignored source command changed original counts/carry/live clamp/absolute clock/client callbacks: budget={budget:?} late={late} freeze={freeze}"
                    );
                }
            }
        },
    );
}

// Existing fixture()/begin()/boundary() exercise the actual receiving Recorder
// and private non-Clone cursor. This does not construct a native Engine.
fn repeated_new_game_message(difficulty: i32, rank: i32, fps: i32) -> GameMessage {
    let mut message = GameMessage::new(GameMessageType::NewGame);
    for value in [0, difficulty, rank, fps] {
        message.append_integer_argument(value);
    }
    message
}
fn prepare_retained_new_game(world: &mut GameLogic, message: GameMessage) -> [i32; 4] {
    let mut values = [0; 4];
    for (index, value) in values.iter_mut().enumerate() {
        let Some(GameMessageArgumentType::Integer(actual)) = message.get_argument(index) else {
            panic!("real admitted NewGame lost integer payload {index}")
        };
        *value = *actual;
    }
    assert_eq!(values[0], 0); // Original GAME_SINGLE_PLAYER, not a fabricated None mode.
    world.prepare_native_command_difficulty(values[1]);
    world.prepare_native_command_mode(GameMode::SinglePlayer);
    values
}
fn resume_repeated_cursor(
    world: &mut GameLogic,
    cursor: NativeTickContinuation,
    completion: NativeCommandCompletion,
    after: impl FnMut(&mut GameLogic),
) -> NativeTickProgress {
    world.resume_native_logic_tick(cursor, completion, Default::default(), |_| {}, after)
}

#[test]
fn repeated_source_newgame_replaces_preparation_and_retains_first_stage_frame_guard() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "repeated_source_newgame_replaces_preparation_and_retains_first_stage_frame_guard",
        || {
            let (_directory, mut world) = fixture();
            let source_frame = world.frame;
            let mut first = begin(
                &mut world,
                vec![
                    repeated_new_game_message(0, 11, 30),
                    repeated_new_game_message(2, 29, 60),
                ],
            );
            let NativeCommandAction::PrepareNewGame(message) = first.take_action() else {
                panic!("first start")
            };
            assert_eq!(
                prepare_retained_new_game(&mut world, message),
                [0, 0, 11, 30]
            );
            let mut second = boundary(resume_repeated_cursor(
                &mut world,
                first,
                NativeCommandCompletion::NewGamePrepared { prepared: true },
                |_| panic!("second NewGame must dispatch before completed-step observer"),
            ));
            let NativeCommandAction::PrepareNewGame(message) = second.take_action() else {
                panic!("second start")
            };
            assert_eq!(
                prepare_retained_new_game(&mut world, message),
                [0, 2, 29, 60]
            );
            let mut callbacks = 0;
            assert!(matches!(
                resume_repeated_cursor(
                    &mut world,
                    second,
                    NativeCommandCompletion::NewGamePrepared { prepared: true },
                    |receiving| {
                        callbacks += 1;
                        assert_eq!(receiving.frame, source_frame);
                    }
                ),
                NativeTickProgress::Complete(_)
            ));
            assert_eq!(callbacks, 1);
            assert_eq!(
                world.frame, source_frame,
                "prepareNewGame resets start flag before EACH startNewGame first stage; do not synchronously load on second message"
            );
            assert_eq!(
                world
                    .owned_script_engine
                    .as_ref()
                    .unwrap()
                    .get_global_difficulty(),
                gamelogic::player::GameDifficulty::Hard
            );
        },
    );
}

#[test]
fn repeated_source_newgame_then_clear_cancels_guard_only_after_actual_reset() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "repeated_source_newgame_then_clear_cancels_guard_only_after_actual_reset",
        || {
            let (_directory, mut world) = fixture();
            let mut cursor = begin(
                &mut world,
                vec![
                    repeated_new_game_message(0, 11, 30),
                    repeated_new_game_message(2, 29, 60),
                    GameMessage::new(GameMessageType::ClearGameData),
                ],
            );
            for expected in [[0, 0, 11, 30], [0, 2, 29, 60]] {
                let NativeCommandAction::PrepareNewGame(message) = cursor.take_action() else {
                    panic!("ordered start")
                };
                assert_eq!(prepare_retained_new_game(&mut world, message), expected);
                cursor = boundary(resume_repeated_cursor(
                    &mut world,
                    cursor,
                    NativeCommandCompletion::NewGamePrepared { prepared: true },
                    |_| panic!("Clear must still dispatch before completed observer"),
                ));
            }
            assert!(matches!(
                cursor.take_action(),
                NativeCommandAction::ClearGameData
            ));
            assert_eq!(
                world.game_mode(),
                GameMode::SinglePlayer,
                "native Clear sees prepared non-None mode even before a map loads"
            );
            world.reset();
            world.finish_clear_game_data_after_reset();
            assert!(matches!(
                resume_repeated_cursor(
                    &mut world,
                    cursor,
                    NativeCommandCompletion::Cleared {
                        reset_performed: true
                    },
                    |_| {}
                ),
                NativeTickProgress::Complete(_)
            ));
            assert_eq!(
                world.frame, 1,
                "actual later reset cancels both prepared guards"
            );
            assert_eq!(world.game_mode(), GameMode::None);
        },
    );
}
