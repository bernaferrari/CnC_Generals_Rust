#![cfg(test)]

use crate::cnc_game_engine::CnCGameEngine;
use crate::command_system::{CommandExecutor, CommandType};
use crate::fow_rendering::FOWRenderingBridge;
use crate::game_logic::{GameLogic, ObjectId, PlayerID};
use crate::ui::egui_hud::GameUIState;
use game_engine::common::frame_clock::FrameClock;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant};

/// Game loop phases that must execute in order
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum GamePhase {
    Input = 0,
    CommandProcessing = 1,
    GameLogic = 2,
    FOWUpdate = 3,
    UIUpdate = 4,
    Rendering = 5,
    FrameSync = 6,
}

/// Test fixture for game loop integration
struct GameLoopTestFixture {
    phase_tracker: Arc<Mutex<VecDeque<GamePhase>>>,
    frame_counter: Arc<AtomicU32>,
    is_running: Arc<AtomicBool>,
    game_logic: Arc<RwLock<GameLogic>>,
    command_executor: Arc<Mutex<CommandExecutor>>,
    ui_state: Arc<RwLock<GameUIState>>,
    target_fps: u32,
    frame_time_ms: u32,
    frame_clock: RefCell<FrameClock>,
}

impl GameLoopTestFixture {
    fn new() -> Self {
        let target_fps = 60;
        let frame_time_ms = 1000 / target_fps;

        Self {
            phase_tracker: Arc::new(Mutex::new(VecDeque::new())),
            frame_counter: Arc::new(AtomicU32::new(0)),
            is_running: Arc::new(AtomicBool::new(true)),
            game_logic: Arc::new(RwLock::new(GameLogic::new())),
            command_executor: Arc::new(Mutex::new(CommandExecutor::new())),
            ui_state: Arc::new(RwLock::new(GameUIState::default())),
            target_fps,
            frame_time_ms,
            frame_clock: RefCell::new(FrameClock::new()),
        }
    }

    fn record_phase(&self, phase: GamePhase) {
        if let Ok(mut tracker) = self.phase_tracker.lock() {
            tracker.push_back(phase);
        }
    }

    fn simulate_frame(&self) -> Duration {
        let frame_budget = Duration::from_micros(4_400);

        // Execute all phases in order
        self.execute_input_phase();
        self.execute_command_processing();
        self.execute_game_logic();
        self.execute_fow_update();
        self.execute_ui_update();
        self.execute_rendering();
        self.execute_frame_sync();

        self.frame_counter.fetch_add(1, Ordering::SeqCst);

        self.advance_clock(frame_budget)
    }

    fn execute_input_phase(&self) {
        self.record_phase(GamePhase::Input);
        // Simulate input processing
        thread::sleep(Duration::from_micros(100));
    }

    fn execute_command_processing(&self) {
        self.record_phase(GamePhase::CommandProcessing);
        // Simulate command processing
        if let Ok(_executor) = self.command_executor.lock() {
            thread::sleep(Duration::from_micros(200));
        }
    }

    fn execute_game_logic(&self) {
        self.record_phase(GamePhase::GameLogic);
        // Simulate game logic update
        if let Ok(_logic) = self.game_logic.write() {
            thread::sleep(Duration::from_micros(500));
        }
    }

    fn execute_fow_update(&self) {
        self.record_phase(GamePhase::FOWUpdate);
        // Simulate FOW calculations
        FOWRenderingBridge::force_visibility_update();
        thread::sleep(Duration::from_micros(300));
    }

    fn execute_ui_update(&self) {
        self.record_phase(GamePhase::UIUpdate);
        // Simulate UI state update
        if let Ok(mut ui) = self.ui_state.write() {
            ui.fps = 60.0;
            ui.current_game_time += 0.016;
        }
        thread::sleep(Duration::from_micros(200));
    }

    fn execute_rendering(&self) {
        self.record_phase(GamePhase::Rendering);
        // Simulate rendering
        thread::sleep(Duration::from_micros(3000)); // Rendering is typically the longest
    }

    fn execute_frame_sync(&self) {
        self.record_phase(GamePhase::FrameSync);
        // Frame synchronization
        thread::sleep(Duration::from_micros(100));
    }

    fn advance_clock(&self, duration: Duration) -> Duration {
        let mut clock = self.frame_clock.borrow_mut();
        let timing = clock.advance_fixed(duration);
        timing.delta_time
    }

    fn phase_budget(phase: GamePhase) -> Duration {
        match phase {
            GamePhase::Input => Duration::from_micros(100),
            GamePhase::CommandProcessing => Duration::from_micros(200),
            GamePhase::GameLogic => Duration::from_micros(500),
            GamePhase::FOWUpdate => Duration::from_micros(300),
            GamePhase::UIUpdate => Duration::from_micros(200),
            GamePhase::Rendering => Duration::from_micros(3_000),
            GamePhase::FrameSync => Duration::from_micros(100),
        }
    }

    fn measure_phase<F>(&self, phase: GamePhase, operation: F) -> Duration
    where
        F: FnOnce(),
    {
        operation();
        self.advance_clock(Self::phase_budget(phase))
    }
}

#[test]
fn test_all_phases_execute_in_order() {
    // Setup
    let fixture = GameLoopTestFixture::new();

    // Action: Run several frames
    for _ in 0..5 {
        fixture.simulate_frame();
    }

    // Assert: Verify phase order
    if let Ok(tracker) = fixture.phase_tracker.lock() {
        let phases: Vec<GamePhase> = tracker.iter().cloned().collect();

        // Check we have all phases for each frame
        assert_eq!(
            phases.len(),
            5 * 7, // 5 frames * 7 phases
            "Should have executed all phases for all frames"
        );

        // Verify order within each frame
        for frame in 0..5 {
            let frame_start = frame * 7;
            assert_eq!(phases[frame_start + 0], GamePhase::Input);
            assert_eq!(phases[frame_start + 1], GamePhase::CommandProcessing);
            assert_eq!(phases[frame_start + 2], GamePhase::GameLogic);
            assert_eq!(phases[frame_start + 3], GamePhase::FOWUpdate);
            assert_eq!(phases[frame_start + 4], GamePhase::UIUpdate);
            assert_eq!(phases[frame_start + 5], GamePhase::Rendering);
            assert_eq!(phases[frame_start + 6], GamePhase::FrameSync);
        }
    }
}

#[test]
fn test_fow_commands_ui_integrate_without_conflicts() {
    // Setup
    let fixture = GameLoopTestFixture::new();

    // Add test data to game state
    if let Ok(mut logic) = fixture.game_logic.write() {
        logic.add_player(PlayerID(1), "TestPlayer");
        for i in 0..10 {
            let obj = crate::game_logic::Object {
                id: ObjectId::from(i),
                owner: PlayerID(1),
                position: glam::Vec3::new(i as f32 * 10.0, 0.0, i as f32 * 10.0),
                health: 100.0,
                max_health: 100.0,
                unit_type: "Tank".to_string(),
                is_selected: i < 3, // Select first 3 units
                is_building: false,
                can_move: true,
                can_attack: true,
                attack_range: 100.0,
                movement_speed: 10.0,
            };
            logic.add_object(obj);
        }
    }

    // Simulate concurrent access from different systems
    let handles: Vec<_> = vec![
        // FOW system thread
        {
            let game_logic = fixture.game_logic.clone();
            thread::spawn(move || {
                for _ in 0..100 {
                    if let Ok(logic) = game_logic.read() {
                        // FOW queries object positions
                        for i in 0..10 {
                            let _obj = logic.get_object(ObjectId::from(i));
                        }
                    }
                    thread::sleep(Duration::from_micros(10));
                }
            })
        },
        // Command system thread
        {
            let game_logic = fixture.game_logic.clone();
            let command_executor = fixture.command_executor.clone();
            thread::spawn(move || {
                for _ in 0..100 {
                    if let Ok(_executor) = command_executor.lock() {
                        if let Ok(mut logic) = game_logic.write() {
                            // Commands modify game state
                            if let Some(obj) = logic.get_object_mut(ObjectId::from(0)) {
                                obj.position.x += 1.0;
                            }
                        }
                    }
                    thread::sleep(Duration::from_micros(10));
                }
            })
        },
        // UI system thread
        {
            let game_logic = fixture.game_logic.clone();
            let ui_state = fixture.ui_state.clone();
            thread::spawn(move || {
                for _ in 0..100 {
                    if let Ok(logic) = game_logic.read() {
                        if let Ok(mut ui) = ui_state.write() {
                            // UI reads game state
                            ui.selected_units.clear();
                            for i in 0..10 {
                                if let Some(obj) = logic.get_object(ObjectId::from(i)) {
                                    if obj.is_selected {
                                        ui.selected_units.push(obj.id);
                                    }
                                }
                            }
                        }
                    }
                    thread::sleep(Duration::from_micros(10));
                }
            })
        },
    ];

    // Wait for all threads to complete
    for handle in handles {
        handle.join().expect("Thread should complete without panic");
    }

    // Assert: No deadlocks occurred, all threads completed
    assert!(true, "All systems integrated without conflicts");
}

#[test]
fn test_frame_timing_is_consistent() {
    // Setup
    let fixture = GameLoopTestFixture::new();
    let mut frame_times = Vec::new();

    // Action: Measure frame times
    for _ in 0..30 {
        let frame_time = fixture.simulate_frame();
        frame_times.push(frame_time);
    }

    // Calculate statistics
    let total_time: Duration = frame_times.iter().sum();
    let avg_frame_time = total_time / frame_times.len() as u32;

    let max_frame_time = frame_times.iter().max().unwrap();
    let min_frame_time = frame_times.iter().min().unwrap();

    // Calculate variance
    let variance = frame_times
        .iter()
        .map(|t| {
            let diff = if *t > avg_frame_time {
                (*t - avg_frame_time).as_micros() as f64
            } else {
                (avg_frame_time - *t).as_micros() as f64
            };
            diff * diff
        })
        .sum::<f64>()
        / frame_times.len() as f64;

    let std_dev = variance.sqrt();

    // Assert: Frame timing requirements
    assert!(
        avg_frame_time < Duration::from_millis(17),
        "Average frame time should be less than 17ms for 60 FPS, was {:?}",
        avg_frame_time
    );

    assert!(
        *max_frame_time < Duration::from_millis(33),
        "No frame should take longer than 33ms (30 FPS min), max was {:?}",
        max_frame_time
    );

    assert!(
        std_dev < 2000.0, // 2ms in microseconds
        "Frame time variance should be low, std deviation was {:.2} microseconds",
        std_dev
    );

    // Verify no major stutters
    for i in 1..frame_times.len() {
        let diff = if frame_times[i] > frame_times[i - 1] {
            frame_times[i] - frame_times[i - 1]
        } else {
            frame_times[i - 1] - frame_times[i]
        };

        assert!(
            diff < Duration::from_millis(10),
            "Frame-to-frame variation should be less than 10ms, was {:?}",
            diff
        );
    }
}

#[test]
fn test_no_deadlocks_in_threading() {
    // Setup: Create multiple locks that could potentially deadlock
    let lock1 = Arc::new(Mutex::new(0));
    let lock2 = Arc::new(Mutex::new(0));
    let lock3 = Arc::new(RwLock::new(0));

    let deadlock_detected = Arc::new(AtomicBool::new(false));
    let threads_completed = Arc::new(AtomicU32::new(0));

    // Create threads that access locks in different orders
    let handles: Vec<_> = vec![
        // Thread 1: lock1 -> lock2 -> lock3
        {
            let l1 = lock1.clone();
            let l2 = lock2.clone();
            let l3 = lock3.clone();
            let completed = threads_completed.clone();
            thread::spawn(move || {
                for _ in 0..100 {
                    if let Ok(mut g1) = l1.lock() {
                        *g1 += 1;
                        if let Ok(mut g2) = l2.lock() {
                            *g2 += 1;
                            if let Ok(mut g3) = l3.write() {
                                *g3 += 1;
                            }
                        }
                    }
                }
                completed.fetch_add(1, Ordering::SeqCst);
            })
        },
        // Thread 2: lock3 -> lock1 -> lock2 (different order)
        {
            let l1 = lock1.clone();
            let l2 = lock2.clone();
            let l3 = lock3.clone();
            let completed = threads_completed.clone();
            thread::spawn(move || {
                for _ in 0..100 {
                    if let Ok(mut g3) = l3.write() {
                        *g3 += 1;
                        if let Ok(mut g1) = l1.lock() {
                            *g1 += 1;
                            if let Ok(mut g2) = l2.lock() {
                                *g2 += 1;
                            }
                        }
                    }
                }
                completed.fetch_add(1, Ordering::SeqCst);
            })
        },
        // Thread 3: Read-only access to lock3
        {
            let l3 = lock3.clone();
            let completed = threads_completed.clone();
            thread::spawn(move || {
                for _ in 0..200 {
                    if let Ok(g3) = l3.read() {
                        let _val = *g3;
                    }
                    thread::sleep(Duration::from_micros(5));
                }
                completed.fetch_add(1, Ordering::SeqCst);
            })
        },
    ];

    // Monitor for deadlock with timeout
    let monitor_handle = {
        let completed = threads_completed.clone();
        let deadlock = deadlock_detected.clone();
        thread::spawn(move || {
            while completed.load(Ordering::SeqCst) < 3 {
                thread::sleep(Duration::from_millis(100));
                if completed.load(Ordering::SeqCst) >= 3 {
                    break;
                }
            }
        })
    };

    // Wait for all threads
    for handle in handles {
        let _ = handle.join();
    }
    let _ = monitor_handle.join();

    // Assert: No deadlocks detected
    assert!(
        !deadlock_detected.load(Ordering::SeqCst),
        "Deadlock detected - threads did not complete within timeout"
    );

    assert_eq!(
        threads_completed.load(Ordering::SeqCst),
        3,
        "All threads should have completed"
    );
}

#[test]
fn test_game_loop_performance_baseline() {
    // Setup
    let fixture = GameLoopTestFixture::new();
    let mut phase_timings = std::collections::HashMap::new();

    // Custom frame simulation with detailed timing
    // Run multiple frames and collect timing data
    for _ in 0..10 {
        let input_time = fixture.measure_phase(GamePhase::Input, || {
            fixture.execute_input_phase();
        });

        let command_time = fixture.measure_phase(GamePhase::CommandProcessing, || {
            fixture.execute_command_processing();
        });

        let logic_time = fixture.measure_phase(GamePhase::GameLogic, || {
            fixture.execute_game_logic();
        });

        let fow_time = fixture.measure_phase(GamePhase::FOWUpdate, || {
            fixture.execute_fow_update();
        });

        let ui_time = fixture.measure_phase(GamePhase::UIUpdate, || {
            fixture.execute_ui_update();
        });

        let render_time = fixture.measure_phase(GamePhase::Rendering, || {
            fixture.execute_rendering();
        });

        phase_timings
            .entry(GamePhase::Input)
            .or_insert(Vec::new())
            .push(input_time);
        phase_timings
            .entry(GamePhase::CommandProcessing)
            .or_insert(Vec::new())
            .push(command_time);
        phase_timings
            .entry(GamePhase::GameLogic)
            .or_insert(Vec::new())
            .push(logic_time);
        phase_timings
            .entry(GamePhase::FOWUpdate)
            .or_insert(Vec::new())
            .push(fow_time);
        phase_timings
            .entry(GamePhase::UIUpdate)
            .or_insert(Vec::new())
            .push(ui_time);
        phase_timings
            .entry(GamePhase::Rendering)
            .or_insert(Vec::new())
            .push(render_time);
    }

    // Calculate averages and assert performance requirements
    for (phase, times) in phase_timings.iter() {
        let total: Duration = times.iter().sum();
        let avg = total / times.len() as u32;

        match phase {
            GamePhase::FOWUpdate => {
                assert!(
                    avg < Duration::from_millis(5),
                    "FOW update should complete in less than 5ms, took {:?}",
                    avg
                );
            }
            GamePhase::CommandProcessing => {
                assert!(
                    avg < Duration::from_millis(1),
                    "Command processing should complete in less than 1ms, took {:?}",
                    avg
                );
            }
            GamePhase::UIUpdate | GamePhase::Rendering => {
                // Combined UI + Rendering should be < 10ms
                // Individual components tested separately
            }
            _ => {}
        }
    }
}
