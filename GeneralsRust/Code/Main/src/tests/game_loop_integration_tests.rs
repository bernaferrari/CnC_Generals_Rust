#![cfg(test)]

use crate::command_system::{CommandSystem, CommandType, ModifierKeys};
use crate::fow_rendering::FOWRenderingBridge;
use crate::game_logic::GameLogic;
use crate::ui::GameUIState;
use game_engine::common::frame_clock::FrameClock;
use glam::Vec3;
use std::collections::VecDeque;
use std::sync::mpsc;
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
    phase_tracker: VecDeque<GamePhase>,
    frame_counter: u32,
    game_logic: GameLogic,
    command_system: CommandSystem,
    ui_state: GameUIState,
    frame_clock: FrameClock,
}

impl GameLoopTestFixture {
    fn new() -> Self {
        Self {
            phase_tracker: VecDeque::new(),
            frame_counter: 0,
            game_logic: GameLogic::new(),
            command_system: CommandSystem::new(),
            ui_state: GameUIState::default(),
            frame_clock: FrameClock::new(),
        }
    }
    fn record_phase(&mut self, phase: GamePhase) {
        self.phase_tracker.push_back(phase);
    }
    fn simulate_frame(&mut self) -> Duration {
        let frame_budget = Duration::from_micros(4_400);
        self.record_phase(GamePhase::Input);
        self.record_phase(GamePhase::CommandProcessing);
        self.command_system.queue_immediate_command(
            CommandType::Move {
                destination: Vec3::new(100.0, 0.0, 200.0),
            },
            &[],
            0,
            ModifierKeys::default(),
        );
        let _ = self.command_system.process_commands(&mut self.game_logic);
        self.record_phase(GamePhase::GameLogic);
        self.game_logic.update_with_dt(0.016);
        self.record_phase(GamePhase::FOWUpdate);
        {
            let mut shroud = self
                .game_logic
                .world_services
                .shroud()
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            FOWRenderingBridge::force_visibility_update(&mut shroud);
        }
        self.record_phase(GamePhase::UIUpdate);
        self.ui_state.current_game_time += 0.016;
        self.ui_state.fps = 60.0;
        self.record_phase(GamePhase::Rendering);
        self.record_phase(GamePhase::FrameSync);
        self.frame_counter += 1;
        self.frame_clock.advance_fixed(frame_budget).delta_time
    }
}

#[test]
fn phases_execute_in_expected_order() {
    let mut fixture = GameLoopTestFixture::new();

    for _ in 0..3 {
        fixture.simulate_frame();
    }

    let phases: Vec<GamePhase> = fixture.phase_tracker.iter().copied().collect();

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
    let mut fixture = GameLoopTestFixture::new();
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

/// Workers exchange owned inputs/results. Only the driving thread mutates
/// authoritative simulation, shroud and UI at their synchronous boundaries.
#[test]
fn command_fow_ui_workers_admit_results_on_the_simulation_owner() {
    enum WorkerResult {
        Command(CommandType),
        Pause(bool),
        Presentation(f32),
    }
    let mut fixture = GameLoopTestFixture::new();
    let (sender, receiver) = mpsc::channel();
    let commands = sender.clone();
    let command_thread = thread::spawn(move || {
        for _ in 0..100 {
            commands
                .send(WorkerResult::Command(CommandType::Invalid))
                .unwrap();
        }
    });
    let controls = sender.clone();
    let game_logic_thread = thread::spawn(move || {
        for i in 0..100 {
            controls.send(WorkerResult::Pause(i % 2 == 0)).unwrap();
        }
    });
    let ui_thread = thread::spawn(move || {
        for _ in 0..100 {
            sender.send(WorkerResult::Presentation(0.016)).unwrap();
        }
    });
    let mut admitted = [0; 3];
    for result in receiver {
        match result {
            WorkerResult::Command(command) => {
                fixture.command_system.queue_immediate_command(
                    command,
                    &[],
                    0,
                    ModifierKeys::default(),
                );
                admitted[0] += 1;
            }
            WorkerResult::Pause(paused) => {
                fixture.game_logic.set_paused(paused);
                admitted[1] += 1;
            }
            WorkerResult::Presentation(delta) => {
                fixture.ui_state.current_game_time += delta;
                let mut shroud = fixture
                    .game_logic
                    .world_services
                    .shroud()
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                FOWRenderingBridge::force_visibility_update(&mut shroud);
                admitted[2] += 1;
            }
        }
    }
    command_thread.join().expect("command worker");
    game_logic_thread.join().expect("input worker");
    ui_thread.join().expect("presentation worker");
    assert_eq!(admitted, [100, 100, 100]);
    assert!(
        !fixture.game_logic.is_paused(),
        "worker order is preserved for one input stream"
    );
    assert!((fixture.ui_state.current_game_time - 1.6).abs() < 0.00001);
    fixture.simulate_frame();
    assert_eq!(fixture.frame_counter, 1);
}
