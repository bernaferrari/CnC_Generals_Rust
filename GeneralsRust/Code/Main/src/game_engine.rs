////////////////////////////////////////////////////////////////////////////////
//
//  (c) 2001-2003 Electronic Arts Inc.
//
////////////////////////////////////////////////////////////////////////////////

// FILE: game_engine.rs
//
// Main GameEngine implementation using factory pattern
// Cross-platform base implementation
//
// Author: Colin Day, April 2001 (Converted to Rust)
//
///////////////////////////////////////////////////////////////////////////////

use anyhow::{Context, Result};
use async_trait::async_trait;
use log::{error, info, warn};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::command_line::CommandLineArgs;
use crate::debug_system::{get_debug_system, initialize_debug_system, DebugConfig};
use crate::engine_factory::{DefaultSubsystemFactory, GameEngine};
use crate::single_instance::{initialize_single_instance_protection, SingleInstanceGuard};
use crate::subsystem_interfaces::SubsystemManager;
use crate::util::profiler::InitTimer;
use crate::version::{get_version_info, initialize_version_system};
use game_engine::common::frame_clock::{FrameClock, FrameTiming as ClockFrameTiming};
use ww3d_engine::FrameTiming;

/// Cross-platform game engine implementation
pub struct CrossPlatformGameEngine {
    /// Engine state
    initialized: bool,
    running: bool,

    /// Core systems
    command_line_args: Option<CommandLineArgs>,
    subsystem_manager: Option<SubsystemManager>,
    single_instance_guard: Option<SingleInstanceGuard>,

    /// Game timing
    target_fps: f64,
    target_frame_time: Duration,
    frame_clock: FrameClock,

    /// Game state
    game_paused: bool,
    exit_requested: bool,

    /// Performance tracking
    frame_count: u64,
    total_runtime: Duration,
}

impl CrossPlatformGameEngine {
    /// Create a new cross-platform game engine
    pub fn new() -> Self {
        let target_fps = 45.0; // DEFAULT_MAX_FPS from C++ GameEngine.h:13
        let target_frame_time = Duration::from_secs_f64(1.0 / target_fps);

        Self {
            initialized: false,
            running: false,
            command_line_args: None,
            subsystem_manager: None,
            single_instance_guard: None,
            target_fps,
            target_frame_time,
            frame_clock: FrameClock::new(),
            game_paused: false,
            exit_requested: false,
            frame_count: 0,
            total_runtime: Duration::ZERO,
        }
    }

    /// Initialize core systems (before subsystems)
    async fn init_core_systems(&mut self, args: &[String]) -> Result<()> {
        let core_timer = InitTimer::new("Core system initialization");
        info!("Initializing core game systems...");

        // Parse command line arguments
        self.command_line_args = Some(
            CommandLineArgs::parse_from_args(args.to_vec())
                .context("Failed to parse command line arguments")?,
        );

        let cmd_args = self
            .command_line_args
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Command line arguments missing after parse"))?;

        // Show help if requested
        if cmd_args.wants_help() {
            CommandLineArgs::print_help();
            return Err(anyhow::anyhow!("Help requested"));
        }

        // Initialize version system
        initialize_version_system();
        let version = get_version_info();
        info!("Game Version: {}", version);

        // Initialize debug system
        let debug_config = DebugConfig {
            log_level: cmd_args.get_log_level(),
            debug_ui_enabled: cmd_args.is_developer_mode(),
            performance_logging: cmd_args.is_developer_mode(),
            ..Default::default()
        };

        initialize_debug_system(Some(debug_config))?;

        // Initialize single instance protection
        self.single_instance_guard = Some(
            initialize_single_instance_protection()
                .context("Failed to acquire single instance lock")?,
        );

        info!("Core systems initialized successfully");
        core_timer.finish();
        Ok(())
    }

    /// Initialize all subsystems using factory pattern
    async fn init_subsystems(&mut self) -> Result<()> {
        let subsys_timer = InitTimer::new("Subsystem initialization");
        info!("Initializing game subsystems...");

        // Create subsystem factory
        let debug_config = DebugConfig::default();
        let factory = Arc::new(DefaultSubsystemFactory::new(debug_config));

        // Initialize subsystem manager
        let mut manager = SubsystemManager::new();
        manager
            .init_with_factory(factory)
            .await
            .context("Failed to initialize subsystems")?;

        self.subsystem_manager = Some(manager);

        info!("Game subsystems initialized successfully");
        subsys_timer.finish();
        Ok(())
    }

    /// Main game loop implementation
    async fn game_loop(&mut self) -> Result<()> {
        info!("Starting main game loop...");
        self.running = true;

        while self.running && !self.exit_requested {
            let frame_start = Instant::now();
            let clock_timing: ClockFrameTiming = self.frame_clock.next_frame();
            let sync_time = clock_timing.total_time.as_millis() as u32;
            let previous_sync_time =
                sync_time.saturating_sub(clock_timing.delta_time.as_millis() as u32);
            let timing = FrameTiming {
                frame_number: clock_timing.frame_number,
                delta_time: clock_timing.delta_time,
                total_time: clock_timing.total_time,
                fps: if clock_timing.delta_time.as_secs_f32() > 0.0 {
                    1.0 / clock_timing.delta_time.as_secs_f32()
                } else {
                    0.0
                },
                frame_start,
                sync_time,
                previous_sync_time,
            };
            let _delta_time = timing.delta_seconds();

            // Update subsystems
            if !self.game_paused {
                if let Some(ref mut manager) = self.subsystem_manager {
                    manager.update_all_with_timing(&timing).await?;
                }
            }

            // Track performance
            self.frame_count = timing.frame_number;
            self.total_runtime = timing.total_time;

            // Frame rate limiting
            let frame_duration = timing.frame_start.elapsed();
            if frame_duration < self.target_frame_time {
                tokio::time::sleep(self.target_frame_time - frame_duration).await;
            }

            // Check for exit conditions
            if self.should_exit() {
                self.exit_requested = true;
            }
        }

        info!("Main game loop ended");
        Ok(())
    }

    /// Check if the game should exit
    fn should_exit(&self) -> bool {
        // This would check for various exit conditions
        // For now, we'll just return false to keep running
        false
    }

    /// Get performance statistics
    pub fn get_performance_stats(&self) -> (u64, Duration, f64) {
        let avg_fps = if self.total_runtime.as_secs_f64() > 0.0 {
            self.frame_count as f64 / self.total_runtime.as_secs_f64()
        } else {
            0.0
        };

        (self.frame_count, self.total_runtime, avg_fps)
    }

    /// Request game exit
    pub fn request_exit(&mut self) {
        info!("Exit requested");
        self.exit_requested = true;
    }

    /// Pause or unpause the game
    pub fn set_paused(&mut self, paused: bool) {
        if self.game_paused != paused {
            self.game_paused = paused;
            info!("Game {}", if paused { "paused" } else { "resumed" });
        }
    }

    /// Check if the game is paused
    pub fn is_paused(&self) -> bool {
        self.game_paused
    }
}

#[async_trait]
impl GameEngine for CrossPlatformGameEngine {
    async fn init(&mut self, args: &[String]) -> Result<()> {
        if self.initialized {
            warn!("Game engine already initialized");
            return Ok(());
        }

        info!("Initializing cross-platform game engine...");

        // Initialize core systems first
        self.init_core_systems(args)
            .await
            .context("Failed to initialize core systems")?;

        // Initialize subsystems
        self.init_subsystems()
            .await
            .context("Failed to initialize subsystems")?;

        self.initialized = true;
        info!("Game engine initialization complete");

        Ok(())
    }

    async fn execute(&mut self) -> Result<()> {
        if !self.initialized {
            return Err(anyhow::anyhow!("Game engine not initialized"));
        }

        info!("Executing game engine...");

        // Run the main game loop
        self.game_loop().await.context("Game loop failed")?;

        info!("Game execution completed");
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        if !self.initialized {
            return Ok(()); // Already shut down
        }

        info!("Shutting down game engine...");

        // Stop the main loop
        self.running = false;

        // Shutdown subsystems
        if let Some(ref mut manager) = self.subsystem_manager {
            manager
                .shutdown_all()
                .await
                .context("Failed to shutdown subsystems")?;
        }

        // Flush debug logs
        if let Some(debug_system) = get_debug_system() {
            debug_system
                .flush()
                .unwrap_or_else(|e| error!("Failed to flush debug logs: {}", e));
        }

        // Print performance statistics
        let (frames, runtime, avg_fps) = self.get_performance_stats();
        info!(
            "Performance Stats: {} frames in {:.2}s (avg {:.1} FPS)",
            frames,
            runtime.as_secs_f64(),
            avg_fps
        );

        self.initialized = false;
        info!("Game engine shutdown complete");

        Ok(())
    }

    fn get_name(&self) -> &str {
        "CrossPlatformGameEngine"
    }

    fn is_initialized(&self) -> bool {
        self.initialized
    }

    fn get_subsystem_manager(&self) -> Option<&SubsystemManager> {
        self.subsystem_manager.as_ref()
    }

    fn get_subsystem_manager_mut(&mut self) -> Option<&mut SubsystemManager> {
        self.subsystem_manager.as_mut()
    }
}

impl Drop for CrossPlatformGameEngine {
    fn drop(&mut self) {
        if self.initialized {
            warn!("Game engine dropped without proper shutdown - forcing shutdown");
            // We can't call async methods in Drop, so just log the issue
            // Subsystems should clean up themselves via their own Drop implementations
        }
    }
}

/// Simple game engine implementation for testing
pub struct SimpleGameEngine {
    initialized: bool,
    name: String,
}

impl SimpleGameEngine {
    pub fn new() -> Self {
        Self {
            initialized: false,
            name: "SimpleGameEngine".to_string(),
        }
    }
}

#[async_trait]
impl GameEngine for SimpleGameEngine {
    async fn init(&mut self, _args: &[String]) -> Result<()> {
        info!("Initializing simple game engine");
        self.initialized = true;
        Ok(())
    }

    async fn execute(&mut self) -> Result<()> {
        info!("Executing simple game engine");
        // Simple implementation that just waits a bit then exits
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down simple game engine");
        self.initialized = false;
        Ok(())
    }

    fn get_name(&self) -> &str {
        &self.name
    }

    fn is_initialized(&self) -> bool {
        self.initialized
    }

    fn get_subsystem_manager(&self) -> Option<&SubsystemManager> {
        None
    }

    fn get_subsystem_manager_mut(&mut self) -> Option<&mut SubsystemManager> {
        None
    }
}
