//! Game State Integration Trait
//!
//! This module defines the interface between the network layer and game logic.
//! It provides an abstraction that allows the network system to remain independent
//! of specific game logic implementations.

use crate::commands::GameCommandData;
use crate::error::NetworkError;
use std::collections::BTreeMap;

/// Unique identifier for a game entity (unit, building, etc.)
pub type EntityId = u32;

/// Unique identifier for a player
pub type PlayerId = u8;

/// Frame number in the game simulation
pub type FrameNumber = u32;

/// CRC checksum value
pub type CRCValue = u32;

/// Snapshot of a game entity for CRC calculation
#[derive(Debug, Clone, PartialEq)]
pub struct EntitySnapshot {
    pub id: EntityId,
    pub position: (f32, f32, f32),
    pub health: i32,
    pub owner: PlayerId,
    pub entity_type: u16,
    pub state: u8,
}

impl EntitySnapshot {
    /// Convert entity snapshot to deterministic byte representation for CRC
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        // Entity ID (4 bytes)
        bytes.extend_from_slice(&self.id.to_le_bytes());

        // Position (12 bytes) - using to_bits() for deterministic float representation
        bytes.extend_from_slice(&self.position.0.to_bits().to_le_bytes());
        bytes.extend_from_slice(&self.position.1.to_bits().to_le_bytes());
        bytes.extend_from_slice(&self.position.2.to_bits().to_le_bytes());

        // Health (4 bytes)
        bytes.extend_from_slice(&self.health.to_le_bytes());

        // Owner (1 byte)
        bytes.push(self.owner);

        // Entity type (2 bytes)
        bytes.extend_from_slice(&self.entity_type.to_le_bytes());

        // State (1 byte)
        bytes.push(self.state);

        bytes
    }
}

/// Player resource state
#[derive(Debug, Clone, PartialEq)]
pub struct ResourceState {
    pub money: i32,
    pub power: i32,
    pub power_consumed: i32,
}

impl ResourceState {
    /// Convert resource state to deterministic byte representation for CRC
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.money.to_le_bytes());
        bytes.extend_from_slice(&self.power.to_le_bytes());
        bytes.extend_from_slice(&self.power_consumed.to_le_bytes());
        bytes
    }
}

/// Complete game state CRC snapshot
#[derive(Debug, Clone)]
pub struct GameStateCRC {
    pub frame: FrameNumber,
    pub entities: Vec<EntitySnapshot>,
    pub resources: BTreeMap<PlayerId, ResourceState>,
    pub random_seed: u32,
}

/// Result type for game state operations
pub type GameStateResult<T> = Result<T, GameStateError>;

/// Errors that can occur during game state operations
#[derive(Debug, Clone, thiserror::Error)]
pub enum GameStateError {
    #[error("Entity {0} not found")]
    EntityNotFound(EntityId),

    #[error("Invalid command type: {0}")]
    InvalidCommandType(u32),

    #[error("Command execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Player {0} not found")]
    PlayerNotFound(PlayerId),

    #[error("CRC computation failed: {0}")]
    CRCComputationFailed(String),
}

impl From<GameStateError> for NetworkError {
    fn from(err: GameStateError) -> Self {
        NetworkError::generic(format!("Game state error: {}", err))
    }
}

/// Main trait that game logic must implement to integrate with networking
///
/// This trait provides the interface between the deterministic network layer
/// and the game simulation. All methods must be deterministic and produce
/// identical results given the same input across all clients.
pub trait GameState: Send + Sync {
    /// Get complete game state for CRC calculation
    ///
    /// This must include ALL mutable game state in a deterministic order:
    /// - All entities sorted by ID
    /// - All player resources sorted by player ID
    /// - Random number generator state
    /// - Any other state that can affect simulation outcome
    fn get_state_for_crc(&self) -> GameStateCRC;

    /// Execute a network command
    ///
    /// This method is called by the network layer when a command is ready to execute.
    /// Commands are provided in deterministic order (by player ID, then sequence number).
    ///
    /// # Determinism Requirements
    /// - Must produce identical results on all clients
    /// - Must not depend on timing or external state
    /// - Must execute in fixed order
    fn execute_command(&mut self, command: &GameCommandData) -> GameStateResult<()>;

    /// Get current game frame number
    ///
    /// Frame numbers must be synchronized across all clients.
    fn current_frame(&self) -> FrameNumber;

    /// Advance to the next frame
    ///
    /// Called after all commands for the current frame have been executed.
    /// Should increment frame counter and perform any per-frame updates.
    fn advance_frame(&mut self);

    /// Get list of all entities for CRC calculation
    ///
    /// Must return entities in deterministic order (sorted by ID).
    fn get_entities(&self) -> Vec<EntitySnapshot>;

    /// Get resource state for all players
    ///
    /// Must return resources in deterministic order (sorted by player ID).
    fn get_resources(&self) -> BTreeMap<PlayerId, ResourceState>;

    /// Get current random seed state
    ///
    /// The random number generator must be deterministic and synchronized.
    fn get_random_seed(&self) -> u32;

    /// Set random seed (for desync recovery)
    ///
    /// Should reset the random number generator to the specified state.
    fn set_random_seed(&mut self, seed: u32);

    /// Check if entity exists
    fn entity_exists(&self, entity_id: EntityId) -> bool;

    /// Get entity owner
    fn get_entity_owner(&self, entity_id: EntityId) -> Option<PlayerId>;

    /// Validate command before execution
    ///
    /// Optional validation step called before execute_command.
    /// Can be used to reject invalid commands early.
    fn validate_command(&self, _command: &GameCommandData) -> GameStateResult<()> {
        // Default implementation: accept all commands
        Ok(())
    }

    /// Handle desynchronization
    ///
    /// Called when CRC mismatch is detected. Game should save state dump
    /// for debugging and prepare for disconnect.
    fn handle_desync(&mut self, frame: FrameNumber, local_crc: CRCValue, remote_crc: CRCValue);

    /// Get game state dump for debugging
    ///
    /// Returns a detailed string representation of game state for desync debugging.
    fn get_state_dump(&self) -> String {
        format!(
            "Frame {}: {} entities, {} players",
            self.current_frame(),
            self.get_entities().len(),
            self.get_resources().len()
        )
    }
}

/// Helper trait for command conversion
///
/// Provides methods to convert between network command data and game-specific command types.
pub trait CommandConverter {
    /// Convert GameCommandData to game-specific command type
    fn from_network_command(data: &GameCommandData) -> GameStateResult<Self>
    where
        Self: Sized;

    /// Convert game-specific command to GameCommandData
    fn to_network_command(&self) -> GameCommandData;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_snapshot_to_bytes() {
        let snapshot = EntitySnapshot {
            id: 42,
            position: (100.5, 200.5, 0.0),
            health: 100,
            owner: 0,
            entity_type: 5,
            state: 1,
        };

        let bytes = snapshot.to_bytes();

        // Verify size: 4 + 12 + 4 + 1 + 2 + 1 = 24 bytes
        assert_eq!(bytes.len(), 24);

        // Verify entity ID
        assert_eq!(
            u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            42
        );
    }

    #[test]
    fn test_entity_snapshot_deterministic() {
        let snapshot1 = EntitySnapshot {
            id: 42,
            position: (100.5, 200.5, 0.0),
            health: 100,
            owner: 0,
            entity_type: 5,
            state: 1,
        };

        let snapshot2 = EntitySnapshot {
            id: 42,
            position: (100.5, 200.5, 0.0),
            health: 100,
            owner: 0,
            entity_type: 5,
            state: 1,
        };

        // Same entity should produce identical bytes
        assert_eq!(snapshot1.to_bytes(), snapshot2.to_bytes());
    }

    #[test]
    fn test_resource_state_to_bytes() {
        let resources = ResourceState {
            money: 1000,
            power: 50,
            power_consumed: 30,
        };

        let bytes = resources.to_bytes();

        // Verify size: 4 + 4 + 4 = 12 bytes
        assert_eq!(bytes.len(), 12);
    }
}
