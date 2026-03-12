use super::SimulationEvent;
use crate::logic::guard_registry::GuardMetrics;
use crate::world::{PlayerId, World};
use std::collections::HashMap;
use std::time::Duration;

/// Per-player snapshot emitted as part of the AI telemetry stream.
#[derive(Debug, Clone, PartialEq)]
pub struct AiPlayerTelemetry {
    /// Player identifier associated with the metrics.
    pub player: PlayerId,
    /// Indicates whether the controller is human.
    pub is_human: bool,
    /// Optional team slot for the player.
    pub team: Option<u8>,
    /// Number of entities owned by the player.
    pub entity_count: usize,
    /// Average health of the player's entities.
    pub average_health: f32,
}

/// Aggregated AI telemetry captured for a single frame.
#[derive(Debug, Clone, PartialEq)]
pub struct AiFrameTelemetry {
    /// Frame index associated with the snapshot.
    pub frame: u64,
    /// Simulation delta used to advance the frame.
    pub delta: Duration,
    /// Total entities tracked in the world.
    pub world_entities: usize,
    /// Neutral (un-owned) entity count.
    pub neutral_entities: usize,
    /// Number of commands processed during the intake phase.
    pub commands_processed: usize,
    /// Command backlog present at the start of the intake phase.
    pub command_backlog: usize,
    /// Guard controller metrics for the frame.
    pub guard: GuardMetrics,
    /// Per-player statistics.
    pub players: Vec<AiPlayerTelemetry>,
    /// Normalised aggression heuristic derived from the telemetry.
    pub aggression: f32,
}

/// Lightweight AI runtime that gathers per-frame telemetry and emits diagnostics.
#[derive(Debug, Default)]
pub struct AiRuntime {
    frame_state: Option<AiFrameState>,
}

#[derive(Debug)]
struct AiFrameState {
    frame: u64,
    delta: Duration,
    world_entities: usize,
    neutral_entities: usize,
    commands_processed: usize,
    command_backlog: usize,
    guard_metrics: GuardMetrics,
    players: Vec<AiPlayerTelemetry>,
    aggression: f32,
}

impl AiRuntime {
    /// Create a new, idle AI runtime.
    pub fn new() -> Self {
        Self { frame_state: None }
    }

    /// Begin collecting telemetry for a new simulation frame.
    pub fn begin_frame(&mut self, frame: u64, delta: Duration) {
        self.frame_state = Some(AiFrameState {
            frame,
            delta,
            world_entities: 0,
            neutral_entities: 0,
            commands_processed: 0,
            command_backlog: 0,
            guard_metrics: GuardMetrics::default(),
            players: Vec::new(),
            aggression: 0.0,
        });
    }

    /// Record command intake statistics for the current frame.
    pub fn record_command_batch(&mut self, processed: usize, backlog: usize) {
        if let Some(state) = self.frame_state.as_mut() {
            state.commands_processed = processed;
            state.command_backlog = backlog;
        }
    }

    /// Sense the current world state and capture baseline telemetry.
    pub fn sense(&mut self, world: &World, guard_metrics: GuardMetrics) {
        if let Some(state) = self.frame_state.as_mut() {
            state.guard_metrics = guard_metrics;

            let mut per_player: HashMap<PlayerId, (usize, f32)> = HashMap::new();
            let mut total_entities = 0usize;
            let mut neutral_entities = 0usize;

            for entity in world.entities() {
                total_entities += 1;
                if let Some(owner) = entity.owner {
                    let entry = per_player.entry(owner).or_insert((0, 0.0));
                    entry.0 += 1;
                    entry.1 += entity.health;
                } else {
                    neutral_entities += 1;
                }
            }

            state.world_entities = total_entities;
            state.neutral_entities = neutral_entities;

            state.players.clear();
            state
                .players
                .extend(world.active_players().map(|(player_id, data)| {
                    let (count, total_health) = per_player.remove(&player_id).unwrap_or((0, 0.0));
                    let average_health = if count > 0 {
                        total_health / count as f32
                    } else {
                        0.0
                    };

                    AiPlayerTelemetry {
                        player: player_id,
                        is_human: data.is_human,
                        team: data.team,
                        entity_count: count,
                        average_health,
                    }
                }));

            if !state.players.is_empty() {
                state.players.sort_by_key(|player| player.player.get());
            }
        }
    }

    /// Evolve decisions based on the sensed telemetry.
    pub fn decide(&mut self) {
        if let Some(state) = self.frame_state.as_mut() {
            let guard_pressure = if state.guard_metrics.registered > 0 {
                state.guard_metrics.engaged as f32 / state.guard_metrics.registered as f32
            } else {
                0.0
            };

            let command_pressure = if state.commands_processed > 0 {
                0.3
            } else {
                0.0
            } + if state.command_backlog > 0 { 0.2 } else { 0.0 };

            let owned_entities = state.world_entities.saturating_sub(state.neutral_entities);
            let entity_pressure = if state.world_entities > 0 {
                owned_entities as f32 / state.world_entities as f32 * 0.3
            } else {
                0.0
            };

            let delta_factor = (state.delta.as_secs_f32() * 60.0).clamp(0.0, 2.0) * 0.1;

            state.aggression = (guard_pressure + command_pressure + entity_pressure + delta_factor)
                .clamp(0.0, 1.0);
        }
    }

    /// Execute AI actions by emitting deterministic diagnostics into the event queue.
    pub fn execute(&mut self, events: &mut Vec<SimulationEvent>) {
        if let Some(state) = self.frame_state.take() {
            let telemetry = AiFrameTelemetry {
                frame: state.frame,
                delta: state.delta,
                world_entities: state.world_entities,
                neutral_entities: state.neutral_entities,
                commands_processed: state.commands_processed,
                command_backlog: state.command_backlog,
                guard: state.guard_metrics,
                players: state.players,
                aggression: state.aggression,
            };
            events.push(SimulationEvent::AiDiagnostics { telemetry });
        }
    }
}
