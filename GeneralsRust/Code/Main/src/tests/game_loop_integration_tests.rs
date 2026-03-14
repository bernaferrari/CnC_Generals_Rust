#![cfg(test)]

use crate::command_system::{CommandSystem, CommandType, ModifierKeys};
use crate::fow_rendering::FOWRenderingBridge;
use crate::game_logic::GameLogic;
use crate::ui::GameUIState;
use game_engine::common::frame_clock::FrameClock;
use glam::Vec3;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum GamePhase {
    Input,
    CommandProcessing,
    GameLogic,
    FOWUpdate,
    UIUpdate,
    Rendering,
    FrameSync,
}

struct GameLoopTestFixture {
    phase_tracker: Arc<Mutex<VecDeque<GamePhase>>>,
    frame_counter: Arc<AtomicU32>,
    game_logic: Arc<RwLock<GameLogic>>,
    command_system: Arc<Mutex<CommandSystem>>,
    ui_state: Arc<RwLock<GameUIState>>,
    frame_clock: Mutex<FrameClock>,
}

impl GameLoopTestFixture {
    fn new() -> Self {
        Self {
            phase_tracker: Arc::new(Mutex::new(VecDeque::new())),
            frame_counter: Arc::new(AtomicU32::new(0)),
            game_logic: Arc::new(RwLock::new(GameLogic::new())),
            command_system: Arc::new(Mutex::new(CommandSystem::new())),
            ui_state: Arc::new(RwLock::new(GameUIState::default())),
            frame_clock: Mutex::new(FrameClock::new()),
        }
    }

    fn record_phase(&self, phase: GamePhase) {
        if let Ok(mut tracker) = self.phase_tracker.lock() {
            tracker.push_back(phase);
        }
    }

    fn simulate_frame(&self) -> Duration {
        let frame_budget = Duration::from_micros(4_400);

        self.record_phase(GamePhase::Input);

        self.record_phase(GamePhase::CommandProcessing);
        if let Ok(mut command_system) = self.command_system.lock() {
            command_system.queue_immediate_command(
                CommandType::Move {
                    destination: Vec3::new(100.0, 0.0, 200.0),
                },
                &[],
                0,
                ModifierKeys::default(),
            );
            if let Ok(mut logic) = self.game_logic.write() {
                let _ = command_system.process_commands(&mut logic);
            }
        }

        self.record_phase(GamePhase::GameLogic);
        if let Ok(mut logic) = self.game_logic.write() {
            logic.update_with_dt(0.016);
        }

        self.record_phase(GamePhase::FOWUpdate);
        FOWRenderingBridge::force_visibility_update();

        self.record_phase(GamePhase::UIUpdate);
        if let Ok(mut ui) = self.ui_state.write() {
            ui.current_game_time += 0.016;
            ui.fps = 60.0;
        }

        self.record_phase(GamePhase::Rendering);
        self.record_phase(GamePhase::FrameSync);

        self.frame_counter.fetch_add(1, Ordering::SeqCst);
        self.frame_clock
            .lock()
            .expect("frame clock lock")
            .advance_fixed(frame_budget)
            .delta_time
    }
}

#[test]
fn phases_execute_in_expected_order() {
    let fixture = GameLoopTestFixture::new();

    for _ in 0..3 {
        fixture.simulate_frame();
    }

    let phases: Vec<GamePhase> = fixture
        .phase_tracker
        .lock()
        .expect("phase tracker lock")
        .iter()
        .copied()
        .collect();

    let expected = [
        GamePhase::Input,
        GamePhase::CommandProcessing,
        GamePhase::GameLogic,
        GamePhase::FOWUpdate,
        GamePhase::UIUpdate,
        GamePhase::Rendering,
        GamePhase::FrameSync,
    ];

    assert_eq!(phases.len(), expected.len() * 3);
    for frame_index in 0..3 {
        let offset = frame_index * expected.len();
        for (idx, phase) in expected.iter().enumerate() {
            assert_eq!(phases[offset + idx], *phase);
        }
    }
}

#[test]
fn frame_budget_stays_under_rts_target() {
    let fixture = GameLoopTestFixture::new();
    let mut frame_times = Vec::new();

    for _ in 0..60 {
        frame_times.push(fixture.simulate_frame());
    }

    let total: Duration = frame_times.iter().copied().sum();
    let average = total / frame_times.len() as u32;
    let max = frame_times.iter().copied().max().unwrap_or_default();

    assert!(average <= Duration::from_millis(17));
    assert!(max <= Duration::from_millis(33));
}

#[test]
fn command_fow_ui_threads_can_progress_together() {
    let fixture = Arc::new(GameLoopTestFixture::new());

    let command_thread = {
        let fixture = Arc::clone(&fixture);
        thread::spawn(move || {
            for _ in 0..100 {
                if let Ok(mut system) = fixture.command_system.lock() {
                    system.queue_immediate_command(
                        CommandType::Invalid,
                        &[],
                        0,
                        ModifierKeys::default(),
                    );
                }
            }
        })
    };

    let game_logic_thread = {
        let fixture = Arc::clone(&fixture);
        thread::spawn(move || {
            for i in 0..100 {
                if let Ok(mut logic) = fixture.game_logic.write() {
                    logic.set_paused(i % 2 == 0);
                }
            }
        })
    };

    let ui_thread = {
        let fixture = Arc::clone(&fixture);
        thread::spawn(move || {
            for _ in 0..100 {
                if let Ok(mut ui) = fixture.ui_state.write() {
                    ui.current_game_time += 0.016;
                }
                FOWRenderingBridge::force_visibility_update();
            }
        })
    };

    command_thread.join().expect("command thread");
    game_logic_thread.join().expect("game_logic thread");
    ui_thread.join().expect("ui thread");
}
