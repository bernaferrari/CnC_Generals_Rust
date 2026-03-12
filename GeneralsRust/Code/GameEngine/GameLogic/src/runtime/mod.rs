//! High level game-logic orchestration.

mod ai;
mod scheduler;

use crate::common::Coord3D;
use crate::logic::{GuardEvent, GuardParameters, GuardRegistry};
use crate::path::{LocomotorSet, PathEnvironment, PathfindServicesInterface};
use crate::world::{
    entities::{EntityId, TemplateRef, Transform},
    EntitySummary, PlayerId, PlayerInfo, World, WorldSnapshot,
};
use ai::AiRuntime;
pub use ai::{AiFrameTelemetry, AiPlayerTelemetry};
use game_engine::common::time::SimulationClock;
use scheduler::{phases, Scheduler, SchedulerRunContext};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Configuration for a [`GameLogic`] instance.
#[derive(Debug, Clone, PartialEq)]
pub struct GameLogicConfig {
    /// Target simulation frames per second.
    pub target_fps: u32,
    /// Maximum number of players supported by the simulation.
    pub max_players: usize,
}

impl Default for GameLogicConfig {
    fn default() -> Self {
        Self {
            target_fps: 30,
            max_players: 8,
        }
    }
}

/// Lightweight statistics captured while the simulation runs.
#[derive(Debug, Clone)]
pub struct SimulationStats {
    frame_count: u64,
    accumulated: Duration,
    last_step: Duration,
    started_at: Instant,
}

impl SimulationStats {
    fn new() -> Self {
        Self {
            frame_count: 0,
            accumulated: Duration::ZERO,
            last_step: Duration::ZERO,
            started_at: Instant::now(),
        }
    }

    fn record_step(&mut self, delta: Duration) {
        self.frame_count += 1;
        self.accumulated += delta;
        self.last_step = delta;
    }

    /// Total number of frames produced since the simulation started.
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Duration covered by the simulation.
    pub fn simulated_time(&self) -> Duration {
        self.accumulated
    }

    /// Wall-clock time spent since the simulation booted.
    pub fn wall_time(&self) -> Duration {
        self.started_at.elapsed()
    }

    /// Duration of the most recent frame.
    pub fn last_step(&self) -> Duration {
        self.last_step
    }
}

/// Result produced when advancing the simulation by one frame.
#[derive(Debug, Clone)]
pub struct FrameResult {
    /// Zero-based frame index.
    pub frame_index: u64,
    /// Snapshot of the deterministic simulation clock.
    pub clock: SimulationClock,
    /// Snapshot of current world state.
    pub world: WorldSnapshot,
    /// Events produced while processing the frame.
    pub events: Vec<SimulationEvent>,
}

/// Command issued to the simulation.
#[derive(Debug, Clone)]
pub enum SimulationCommand {
    /// Allocate a new player slot.
    SpawnPlayer {
        /// Optional display name.
        name: Option<String>,
        /// Optional team assignment.
        team: Option<u8>,
        /// Whether the player is human controlled.
        is_human: bool,
    },
    /// Remove a player from the simulation.
    RemovePlayer {
        /// Identifier to remove.
        id: PlayerId,
    },
    /// Spawn an entity.
    SpawnEntity {
        /// Template metadata.
        template: TemplateRef,
        /// Owning player.
        owner: Option<PlayerId>,
        /// Starting transform.
        transform: Transform,
        /// Starting hitpoints.
        health: f32,
    },
    /// Spawn an entity with guard behaviour.
    SpawnGuard {
        /// Template metadata.
        template: TemplateRef,
        /// Owning player.
        owner: Option<PlayerId>,
        /// Starting transform.
        transform: Transform,
        /// Starting hitpoints.
        health: f32,
        /// Guard configuration.
        guard: GuardParameters,
    },
    /// Remove an entity.
    RemoveEntity {
        /// Identifier to remove.
        id: EntityId,
    },
    /// Notify guard of hostile sighting.
    GuardSpotHostile {
        /// Guard entity.
        guard: EntityId,
        /// Hostile entity.
        target: EntityId,
    },
}

/// Event emitted by the simulation when processing commands or advancing state.
#[derive(Debug, Clone, PartialEq)]
pub enum SimulationEvent {
    /// Player slot allocated.
    PlayerAdded {
        /// Player identifier.
        id: PlayerId,
        /// Associated metadata.
        info: PlayerInfo,
    },
    /// Player slot reclaimed.
    PlayerRemoved {
        /// Player identifier.
        id: PlayerId,
    },
    /// Entity spawned into the world.
    EntitySpawned {
        /// Identifier.
        id: EntityId,
        /// Snapshot produced at spawn time.
        info: EntitySummary,
    },
    /// Entity removed from the world.
    EntityRemoved {
        /// Identifier.
        id: EntityId,
    },
    /// Guard behaviour emitted an event.
    GuardState {
        /// Entity executing the guard behaviour.
        entity: EntityId,
        /// Detailed guard event.
        event: GuardEvent,
    },
    /// AI diagnostics captured during the execute phase.
    AiDiagnostics {
        /// Aggregated telemetry for the frame.
        telemetry: AiFrameTelemetry,
    },
    /// Frame advanced.
    Tick {
        /// Frame index after the tick.
        frame: u64,
    },
}

/// Central simulation façade.
pub struct GameLogic {
    config: GameLogicConfig,
    clock: SimulationClock,
    world: World,
    path_env: PathEnvironment,
    stats: SimulationStats,
    commands: VecDeque<SimulationCommand>,
    scheduler: Scheduler,
    ai_runtime: AiRuntime,
    guard_registry: GuardRegistry,
}

impl GameLogic {
    /// Create a new game logic instance with default configuration.
    pub fn new() -> Self {
        Self::with_config(GameLogicConfig::default())
    }

    /// Create a new instance using the provided configuration.
    pub fn with_config(config: GameLogicConfig) -> Self {
        let mut clock = SimulationClock::new(config.target_fps);
        // Reset explicitly to avoid inheriting state when reusing instances in tests.
        clock.reset();

        let mut scheduler = Scheduler::default();
        scheduler.register_system(
            phases::COMMAND_INTAKE,
            0,
            "Process Commands",
            process_commands_system,
        );
        scheduler.register_system(phases::AI_SENSE, 10, "AI Sense", ai_sense_system);
        scheduler.register_system(phases::AI_SENSE, 0, "Guard Sync", guard_sync_system);
        scheduler.register_system(phases::AI_DECIDE, 10, "AI Decide", ai_decide_system);
        scheduler.register_system(phases::AI_EXECUTE, 10, "AI Execute", ai_execute_system);
        scheduler.register_system(phases::AI_EXECUTE, 0, "Guard Tick", guard_tick_system);

        Self {
            world: World::new(config.max_players),
            path_env: PathEnvironment::new(),
            stats: SimulationStats::new(),
            clock,
            config,
            commands: VecDeque::new(),
            scheduler,
            ai_runtime: AiRuntime::new(),
            guard_registry: GuardRegistry::new(),
        }
    }

    /// Returns an immutable view of the simulation configuration.
    pub fn config(&self) -> &GameLogicConfig {
        &self.config
    }

    /// Mutable access to the configuration.
    ///
    /// See [`GameLogic::reconfigure`] to apply configuration changes that need
    /// to mutate internal state such as the simulation clock.
    pub fn config_mut(&mut self) -> &mut GameLogicConfig {
        &mut self.config
    }

    /// Apply configuration changes.
    ///
    /// This reconciles internal state (clock tick rate, world limits) with the
    /// updated configuration.
    pub fn reconfigure(&mut self) {
        self.clock.set_tick_rate(self.config.target_fps);
        self.world.resize(self.config.max_players);
        self.path_env.update_from_world(&self.world);
    }

    /// Access the world state.
    pub fn world(&self) -> &World {
        &self.world
    }

    /// Mutable world access for subsystem integration.
    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    /// Simulation statistics captured so far.
    pub fn stats(&self) -> &SimulationStats {
        &self.stats
    }

    /// Run a single frame of simulation.
    pub fn tick(&mut self) -> FrameResult {
        let delta = self.clock.delta();
        self.clock.advance();
        self.world.advance(self.clock.frame());
        self.path_env.update_from_world(&self.world);
        self.stats.record_step(delta);

        let mut events = Vec::new();
        let frame_index = self.stats.frame_count();
        self.ai_runtime.begin_frame(frame_index, delta);
        {
            let ctx = SchedulerRunContext {
                frame_index,
                delta,
                world: &mut self.world,
                ai_runtime: &mut self.ai_runtime,
                path_env: &mut self.path_env,
                guard_registry: &mut self.guard_registry,
                commands: &mut self.commands,
                events: &mut events,
            };
            self.scheduler.run(ctx);
        }
        events.push(SimulationEvent::Tick {
            frame: self.clock.frame() as u64,
        });

        FrameResult {
            frame_index: self.stats.frame_count(),
            clock: self.clock.clone(),
            world: self.world.snapshot(),
            events,
        }
    }

    /// Run the simulation for the given number of frames.
    pub fn simulate_for(&mut self, frames: u32) -> FrameResult {
        if frames == 0 {
            return FrameResult {
                frame_index: self.stats.frame_count(),
                clock: self.clock.clone(),
                world: self.world.snapshot(),
                events: Vec::new(),
            };
        }

        let mut accumulated_events = Vec::new();
        let mut result = FrameResult {
            frame_index: self.stats.frame_count(),
            clock: self.clock.clone(),
            world: self.world.snapshot(),
            events: Vec::new(),
        };

        for _ in 0..frames {
            let frame = self.tick();
            accumulated_events.extend(frame.events.iter().cloned());
            result = frame;
        }

        result.events = accumulated_events;
        result
    }

    /// Convenience helper for handing out player identifiers.
    pub fn allocate_player(&mut self) -> Option<PlayerId> {
        self.world.allocate_player()
    }

    /// Enqueue a simulation command for processing on the next tick.
    pub fn enqueue_command(&mut self, command: SimulationCommand) {
        self.commands.push_back(command);
    }

    /// Convenience helper to spawn a guarded entity.
    pub fn enqueue_guard_spawn(
        &mut self,
        template: TemplateRef,
        owner: Option<PlayerId>,
        transform: Transform,
        health: f32,
        guard: GuardParameters,
    ) {
        self.enqueue_command(SimulationCommand::SpawnGuard {
            template,
            owner,
            transform,
            health,
            guard,
        });
    }

    /// Convenience helper to notify a guard about a hostile entity.
    pub fn enqueue_guard_hostile(&mut self, guard: EntityId, target: EntityId) {
        self.enqueue_command(SimulationCommand::GuardSpotHostile { guard, target });
    }

    /// Mutable access to the path environment.
    pub fn path_environment_mut(&mut self) -> &mut PathEnvironment {
        &mut self.path_env
    }

    /// Immutable access to the path environment.
    pub fn path_environment(&self) -> &PathEnvironment {
        &self.path_env
    }

    /// Request a path using the authoritative path environment.
    pub fn request_path(
        &mut self,
        locomotor: &LocomotorSet,
        from: &Coord3D,
        to: &Coord3D,
    ) -> Option<Vec<Coord3D>> {
        let handle = {
            let pathfinder = self.path_env.pathfinder_mut();
            pathfinder.find_path(0, locomotor, from, to)?
        };

        let mut positions = {
            let pathfinder = self.path_env.pathfinder();
            let path = pathfinder.path(handle)?;
            path.positions()
        };

        self.path_env.pathfinder_mut().release_path(handle);

        if positions.first().is_none() {
            positions.push(*from);
        }

        let needs_start = positions.first().is_some_and(|first| {
            (first.x - from.x).abs() >= 1.0 || (first.y - from.y).abs() >= 1.0
        });
        if needs_start {
            positions.insert(0, *from);
        }

        let needs_goal = positions.last().is_some_and(|last| {
            (last.x - to.x).abs() >= crate::path::PATHFIND_CELL_SIZE_F
                || (last.y - to.y).abs() >= crate::path::PATHFIND_CELL_SIZE_F
        });
        if needs_goal {
            positions.push(*to);
        }

        Some(positions)
    }
}

fn ai_sense_system(ctx: &mut SchedulerRunContext<'_>) {
    let guard_metrics = ctx.guard_registry.metrics();
    ctx.ai_runtime.sense(&*ctx.world, guard_metrics);
}

fn ai_decide_system(ctx: &mut SchedulerRunContext<'_>) {
    ctx.ai_runtime.decide();
}

fn ai_execute_system(ctx: &mut SchedulerRunContext<'_>) {
    ctx.ai_runtime.execute(ctx.events);
}

fn process_commands_system(ctx: &mut SchedulerRunContext<'_>) {
    let backlog = ctx.commands.len();
    let mut processed = 0usize;
    while let Some(command) = ctx.commands.pop_front() {
        processed += 1;
        match command {
            SimulationCommand::SpawnPlayer {
                name,
                team,
                is_human,
            } => {
                if let Some(id) = ctx.world.allocate_player_with_name(name, team, is_human) {
                    if let Some(data) = ctx.world.player(id) {
                        ctx.events.push(SimulationEvent::PlayerAdded {
                            id,
                            info: data.to_info(id),
                        });
                    }
                }
            }
            SimulationCommand::RemovePlayer { id } => {
                if ctx.world.remove_player(id).is_some() {
                    ctx.events.push(SimulationEvent::PlayerRemoved { id });
                }
            }
            SimulationCommand::SpawnEntity {
                template,
                owner,
                transform,
                health,
            } => {
                let id = ctx.world.spawn_entity(template, owner, transform, health);
                if let Some(info) = ctx.world.entity_summary_by_id(id) {
                    ctx.events.push(SimulationEvent::EntitySpawned { id, info });
                }
            }
            SimulationCommand::SpawnGuard {
                template,
                owner,
                transform,
                health,
                guard,
            } => {
                let id = ctx.world.spawn_entity(template, owner, transform, health);
                ctx.guard_registry.register_guard(id, transform, guard);
                if let Some(info) = ctx.world.entity_summary_by_id(id) {
                    ctx.events.push(SimulationEvent::EntitySpawned { id, info });
                }
            }
            SimulationCommand::RemoveEntity { id } => {
                if ctx.world.remove_entity(id) {
                    ctx.guard_registry.remove_guard(id);
                    ctx.events.push(SimulationEvent::EntityRemoved { id });
                }
            }
            SimulationCommand::GuardSpotHostile { guard, target } => {
                ctx.guard_registry.spot_hostile(guard, target);
            }
        }
    }
    ctx.ai_runtime.record_command_batch(processed, backlog);
}

fn guard_sync_system(ctx: &mut SchedulerRunContext<'_>) {
    ctx.guard_registry.sync_from_world(ctx.world);
}

fn guard_tick_system(ctx: &mut SchedulerRunContext<'_>) {
    ctx.guard_registry.tick(ctx.world, ctx.events);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::{LocomotorSet, PATHFIND_CELL_SIZE_F, SURFACE_GROUND};
    use crate::world::entities::{TemplateRef, Transform};

    #[test]
    fn request_path_produces_positions() {
        let mut logic = GameLogic::new();
        let locomotor = LocomotorSet::new(SURFACE_GROUND, false, 5.0);
        let start = Coord3D::new(0.0, 0.0, 0.0);
        let goal = Coord3D::new(30.0, 0.0, 0.0);

        let path = logic
            .request_path(&locomotor, &start, &goal)
            .expect("path exists");

        assert!(
            path.len() >= 2,
            "expected at least start and goal positions"
        );
        let first = path.first().copied().unwrap();
        let last = path.last().copied().unwrap();
        assert!((first.x - start.x).abs() < 1.0);
        assert!((first.y - start.y).abs() < 1.0);
        assert!((last.x - goal.x).abs() < PATHFIND_CELL_SIZE_F);
        assert!((last.y - goal.y).abs() < PATHFIND_CELL_SIZE_F);
    }

    #[test]
    fn simulation_advances_clock() {
        let mut game = GameLogic::new();
        let before = game.stats().frame_count();
        let frame = game.tick();

        assert_eq!(before + 1, frame.frame_index);
        assert_eq!(frame.frame_index, game.stats().frame_count());
        assert_eq!(frame.clock.frame() as u64, game.stats().frame_count());
        match frame.events.as_slice() {
            [SimulationEvent::AiDiagnostics { telemetry }, SimulationEvent::Tick { frame: 1 }] => {
                assert_eq!(telemetry.frame, 1);
                assert_eq!(telemetry.world_entities, 0);
                assert_eq!(telemetry.command_backlog, 0);
            }
            other => panic!("unexpected events {:?}", other),
        }
    }

    #[test]
    fn simulate_for_runs_requested_frames() {
        let mut game = GameLogic::with_config(GameLogicConfig {
            target_fps: 15,
            max_players: 2,
        });

        game.enqueue_command(SimulationCommand::SpawnPlayer {
            name: Some("Alice".into()),
            team: Some(1),
            is_human: true,
        });
        let frame = game.simulate_for(10);

        assert_eq!(frame.frame_index, 10);
        assert_eq!(game.stats().frame_count(), 10);
        assert!(game.stats().simulated_time() > Duration::ZERO);
        assert_eq!(frame.world.players.len(), 1);
        assert!(frame
            .events
            .iter()
            .any(|event| matches!(event, SimulationEvent::PlayerAdded { .. })));
    }

    #[test]
    fn remove_player_command_emits_event() {
        let mut game = GameLogic::new();
        game.enqueue_command(SimulationCommand::SpawnPlayer {
            name: None,
            team: None,
            is_human: true,
        });
        let frame = game.tick();
        let player_id = match frame.events.iter().find_map(|event| match event {
            SimulationEvent::PlayerAdded { id, .. } => Some(*id),
            _ => None,
        }) {
            Some(id) => id,
            None => panic!("expected player spawn event"),
        };

        game.enqueue_command(SimulationCommand::RemovePlayer { id: player_id });
        let frame = game.tick();
        assert!(frame.events.iter().any(
            |event| matches!(event, SimulationEvent::PlayerRemoved { id } if *id == player_id)
        ));
    }

    #[test]
    fn entity_spawn_and_removal_commands_emit_events() {
        let mut game = GameLogic::new();
        game.enqueue_command(SimulationCommand::SpawnEntity {
            template: TemplateRef::new("USAInfantryRanger"),
            owner: Some(PlayerId::FIRST),
            transform: Transform::new([5.0, 0.0, 0.0], 0.0),
            health: 200.0,
        });

        let frame = game.tick();
        let entity_id = match frame.events.iter().find_map(|event| match event {
            SimulationEvent::EntitySpawned { id, .. } => Some(*id),
            _ => None,
        }) {
            Some(id) => id,
            None => panic!("expected entity spawn event"),
        };

        game.enqueue_command(SimulationCommand::RemoveEntity { id: entity_id });
        let frame = game.tick();
        assert!(frame.events.iter().any(
            |event| matches!(event, SimulationEvent::EntityRemoved { id } if *id == entity_id)
        ));
    }
}
