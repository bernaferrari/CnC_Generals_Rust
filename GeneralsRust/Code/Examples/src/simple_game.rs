//! # Simple Game Example
//!
//! A minimal working example demonstrating the core capabilities of the
//! C&C Generals Zero Hour Rust engine with a simple playable game scenario.

use std::time::{Duration, Instant};
use anyhow::Result;
use tracing::{info, debug, error};
use tokio::time::interval;

use integration::{IntegrationSystem, IntegrationConfig};
use game_network::NetworkClock;
use ww3d_engine::FrameTiming;

/// Simple game state
#[derive(Debug)]
struct SimpleGame {
    integration: IntegrationSystem,
    game_state: GameState,
    frame_clock: game_engine::common::frame_clock::FrameClock,
    frame_count: u64,
    running: bool,
}

#[derive(Debug)]
struct GameState {
    units: Vec<Unit>,
    buildings: Vec<Building>,
    score: u32,
    time_elapsed: Duration,
}

#[derive(Debug, Clone)]
struct Unit {
    id: u32,
    position: glam::Vec3,
    health: f32,
    unit_type: UnitType,
}

#[derive(Debug, Clone)]
enum UnitType {
    Infantry,
    Tank,
    Aircraft,
}

#[derive(Debug, Clone)]
struct Building {
    id: u32,
    position: glam::Vec3,
    health: f32,
    building_type: BuildingType,
}

#[derive(Debug, Clone)]
enum BuildingType {
    CommandCenter,
    Barracks,
    Factory,
    PowerPlant,
}

impl SimpleGame {
    /// Create a new simple game
    async fn new() -> Result<Self> {
        info!("🎮 Creating Simple Game Example");
        
        // Initialize integration system with optimized config for demo
        let config = IntegrationConfig {
            performance: integration::PerformanceConfig {
                target_fps: 60.0,
                monitor_interval_ms: 1000,
                memory_warning_mb: 512,
                cpu_warning_percent: 80.0,
                auto_tuning: true,
            },
            ..Default::default()
        };
        
        let mut integration = IntegrationSystem::with_config(config).await?;
        integration.initialize().await?;
        
        let game_state = GameState {
            units: vec![
                Unit {
                    id: 1,
                    position: glam::Vec3::new(100.0, 0.0, 100.0),
                    health: 100.0,
                    unit_type: UnitType::Infantry,
                },
                Unit {
                    id: 2,
                    position: glam::Vec3::new(150.0, 0.0, 150.0),
                    health: 200.0,
                    unit_type: UnitType::Tank,
                },
            ],
            buildings: vec![
                Building {
                    id: 1,
                    position: glam::Vec3::new(50.0, 0.0, 50.0),
                    health: 500.0,
                    building_type: BuildingType::CommandCenter,
                },
                Building {
                    id: 2,
                    position: glam::Vec3::new(200.0, 0.0, 200.0),
                    health: 300.0,
                    building_type: BuildingType::Barracks,
                },
            ],
            score: 0,
            time_elapsed: Duration::ZERO,
        };
        
        Ok(Self {
            integration,
            game_state,
            frame_clock: FrameClock::new(),
            frame_count: 0,
            running: true,
        })
    }
    
    /// Run the simple game
    async fn run(&mut self) -> Result<()> {
        info!("🚀 Starting Simple Game Example");
        
        let target_frame_time = Duration::from_secs_f64(1.0 / 60.0); // 60 FPS
        let mut frame_timer = interval(target_frame_time);
        
        println!("🎯 Simple Game Controls:");
        println!("  - Game runs for 30 seconds automatically");
        println!("  - Watch the console for game updates");
        println!("  - Press Ctrl+C to exit early");
        println!();
        
        let game_start = Instant::now();
        let game_duration = Duration::from_secs(30);
        
        while self.running && game_start.elapsed() < game_duration {
            frame_timer.tick().await;
            
            let timing = self.frame_clock.next_frame();
            let delta_time = timing.delta_seconds();
            NetworkClock::override_with_duration(timing.total_time);
            
            // Update game
            self.update(delta_time).await?;
            
            // Update integration system
            self.integration.update(&timing).await?;
            
            self.frame_count = timing.frame_number;
            
            // Display status every 5 seconds
            if self.frame_count % (60 * 5) == 0 {
                self.display_status();
            }
        }
        
        NetworkClock::clear_override();
        
        info!("🏁 Simple Game Example completed");
        self.shutdown().await?;
        
        Ok(())
    }
    
    /// Update game logic
    async fn update(&mut self, delta_time: f32) -> Result<()> {
        self.game_state.time_elapsed += Duration::from_secs_f32(delta_time);
        
        // Update units
        self.update_units(delta_time);
        
        // Update buildings
        self.update_buildings(delta_time);
        
        // Update game score
        self.update_score(delta_time);
        
        Ok(())
    }
    
    /// Update units
    fn update_units(&mut self, delta_time: f32) {
        for unit in &mut self.game_state.units {
            match unit.unit_type {
                UnitType::Infantry => {
                    // Infantry moves slowly in circles
                    let time = self.game_state.time_elapsed.as_secs_f32();
                    let radius = 50.0;
                    unit.position.x = 100.0 + radius * (time * 0.5).cos();
                    unit.position.z = 100.0 + radius * (time * 0.5).sin();
                },
                UnitType::Tank => {
                    // Tank moves in a figure-8 pattern
                    let time = self.game_state.time_elapsed.as_secs_f32();
                    unit.position.x = 150.0 + 30.0 * (time * 0.8).sin();
                    unit.position.z = 150.0 + 15.0 * (time * 1.6).sin();
                },
                UnitType::Aircraft => {
                    // Aircraft flies high and fast
                    let time = self.game_state.time_elapsed.as_secs_f32();
                    unit.position.x = 200.0 + 100.0 * (time * 2.0).cos();
                    unit.position.z = 200.0 + 100.0 * (time * 2.0).sin();
                    unit.position.y = 50.0; // Flying height
                },
            }
        }
    }
    
    /// Update buildings
    fn update_buildings(&mut self, _delta_time: f32) {
        // Buildings generate resources and units
        for building in &mut self.game_state.buildings {
            match building.building_type {
                BuildingType::CommandCenter => {
                    // Command center provides overall control
                    if rand::random::<f32>() < 0.01 {
                        debug!("Command center coordinating operations");
                    }
                },
                BuildingType::Barracks => {
                    // Barracks occasionally spawn infantry
                    if rand::random::<f32>() < 0.005 {
                        self.spawn_unit(UnitType::Infantry, building.position);
                    }
                },
                BuildingType::Factory => {
                    // Factory occasionally spawns tanks
                    if rand::random::<f32>() < 0.003 {
                        self.spawn_unit(UnitType::Tank, building.position);
                    }
                },
                BuildingType::PowerPlant => {
                    // Power plant generates power (increases score)
                    self.game_state.score += 1;
                },
            }
        }
    }
    
    /// Spawn a new unit
    fn spawn_unit(&mut self, unit_type: UnitType, near_position: glam::Vec3) {
        let new_id = self.game_state.units.len() as u32 + 1;
        let spawn_offset = glam::Vec3::new(
            rand::random::<f32>() * 20.0 - 10.0,
            0.0,
            rand::random::<f32>() * 20.0 - 10.0,
        );
        
        let new_unit = Unit {
            id: new_id,
            position: near_position + spawn_offset,
            health: match unit_type {
                UnitType::Infantry => 100.0,
                UnitType::Tank => 200.0,
                UnitType::Aircraft => 150.0,
            },
            unit_type,
        };
        
        info!("🛡️ Spawned new {:?} unit (ID: {})", new_unit.unit_type, new_unit.id);
        self.game_state.units.push(new_unit);
        
        // Keep unit count reasonable
        if self.game_state.units.len() > 10 {
            self.game_state.units.remove(0);
            debug!("Removed oldest unit to maintain performance");
        }
    }
    
    /// Update game score
    fn update_score(&mut self, delta_time: f32) {
        // Score increases based on units and buildings
        let unit_score = self.game_state.units.len() as u32;
        let building_score = self.game_state.buildings.len() as u32 * 10;
        let time_bonus = (delta_time * 10.0) as u32;
        
        self.game_state.score += unit_score + building_score + time_bonus;
    }
    
    /// Display game status
    fn display_status(&self) {
        let metrics = self.integration.get_performance_metrics();
        let diagnostics = self.integration.get_diagnostics();
        
        println!("\n📊 === Game Status ===");
        println!("Time: {:.1}s | Frame: {} | Score: {}", 
                 self.game_state.time_elapsed.as_secs_f64(),
                 self.frame_count,
                 self.game_state.score);
        
        println!("Units: {} | Buildings: {}", 
                 self.game_state.units.len(),
                 self.game_state.buildings.len());
        
        println!("Performance: {:.1} FPS | CPU: {:.1}% | Memory: {:.1} MB",
                 metrics.graphics.fps,
                 metrics.cpu.usage_percent,
                 metrics.memory.used_mb as f64 / 1024.0 / 1024.0);
        
        println!("Health Score: {:.1}/100 | Stability: {:.1}%",
                 diagnostics.health_score,
                 metrics.overall.stability);
        
        // Display unit positions
        println!("\n🎯 Unit Positions:");
        for unit in &self.game_state.units {
            println!("  {:?} #{}: ({:.1}, {:.1}, {:.1}) HP: {:.1}",
                     unit.unit_type, unit.id,
                     unit.position.x, unit.position.y, unit.position.z,
                     unit.health);
        }
        println!();
    }
    
    /// Shutdown the game
    async fn shutdown(&mut self) -> Result<()> {
        info!("🔌 Shutting down Simple Game Example");
        
        // Display final statistics
        println!("\n🏆 === Final Game Statistics ===");
        println!("Total Time: {:.1}s", self.game_state.time_elapsed.as_secs_f64());
        println!("Total Frames: {}", self.frame_count);
        println!("Final Score: {}", self.game_state.score);
        println!("Average FPS: {:.1}", self.frame_count as f64 / self.game_state.time_elapsed.as_secs_f64());
        println!("Final Unit Count: {}", self.game_state.units.len());
        println!("Building Count: {}", self.game_state.buildings.len());
        
        // Show performance metrics
        let metrics = self.integration.get_performance_metrics();
        println!("\n📈 Performance Summary:");
        println!("Peak Memory Usage: {:.1} MB", metrics.memory.used_mb as f64 / 1024.0 / 1024.0);
        println!("Average CPU Usage: {:.1}%", metrics.cpu.usage_percent);
        println!("Graphics Performance: {:.1} FPS", metrics.graphics.fps);
        
        // Shutdown integration system
        self.integration.shutdown().await?;
        
        println!("\n✅ Game shutdown completed successfully!");
        Ok(())
    }
}

/// Minimal WW3D-style frame clock for driving demos outside the full engine loop.
#[derive(Debug, Clone)]
struct FrameClock {
    last_frame_start: Instant,
    total_time: Duration,
    frame_number: u64,
}

impl FrameClock {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            last_frame_start: now,
            total_time: Duration::ZERO,
            frame_number: 0,
        }
    }

    fn next_frame(&mut self) -> FrameTiming {
        let now = Instant::now();
        let delta = now.saturating_duration_since(self.last_frame_start);
        self.last_frame_start = now;
        self.accumulate(delta, now)
    }

    fn accumulate(&mut self, delta: Duration, frame_start: Instant) -> FrameTiming {
        self.frame_number = self.frame_number.wrapping_add(1);
        self.total_time += delta;
        let fps = if delta.as_secs_f32() > 0.0 {
            1.0 / delta.as_secs_f32()
        } else {
            0.0
        };

        FrameTiming {
            frame_number: self.frame_number,
            delta_time: delta,
            total_time: self.total_time,
            fps,
            frame_start,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info,integration=debug,simple_game=debug")
        .init();
    
    println!("🎮 C&C Generals Zero Hour - Simple Game Example");
    println!("================================================");
    println!();
    
    // Create and run the simple game
    let mut game = SimpleGame::new().await?;
    game.run().await?;
    
    Ok(())
}
