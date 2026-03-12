//! Game Logic Integration Layer
//!
//! This module provides the integration between the network layer and game logic.
//! It defines the interfaces and utilities needed for:
//! - Command execution from network to game
//! - CRC computation and validation for desync detection
//! - Desynchronization handling and recovery
//! - Game state snapshots for debugging
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────┐
//! │  Network Layer  │
//! │  (GameNetwork)  │
//! └────────┬────────┘
//!          │
//!          │ GameCommandData
//!          │
//!          ▼
//! ┌─────────────────┐
//! │   Integration   │
//! │     Layer       │
//! │                 │
//! │  ┌──────────┐   │
//! │  │GameState │   │
//! │  │  Trait   │   │
//! │  └──────────┘   │
//! │       ▲         │
//! │       │         │
//! │  ┌────┴─────┐   │
//! │  │Command   │   │
//! │  │Executor  │   │
//! │  └──────────┘   │
//! │       ▲         │
//! │       │         │
//! │  ┌────┴─────┐   │
//! │  │   CRC    │   │
//! │  │Validator │   │
//! │  └──────────┘   │
//! └────────┬────────┘
//!          │
//!          │ execute_command()
//!          │ get_state_for_crc()
//!          │
//!          ▼
//! ┌─────────────────┐
//! │   Game Logic    │
//! │  (GameLogic)    │
//! └─────────────────┘
//! ```
//!
//! # Usage
//!
//! ## 1. Implement GameState Trait
//!
//! ```rust,ignore
//! use game_network::integration::{GameState, GameStateCRC};
//!
//! struct MyGameState {
//!     // ... game state fields
//! }
//!
//! impl GameState for MyGameState {
//!     fn execute_command(&mut self, command: &GameCommandData) -> GameStateResult<()> {
//!         // Execute command on game state
//!     }
//!
//!     fn get_state_for_crc(&self) -> GameStateCRC {
//!         // Return complete game state for CRC
//!     }
//!
//!     // ... implement other methods
//! }
//! ```
//!
//! ## 2. Create Command Executor
//!
//! ```rust,ignore
//! use game_network::integration::CommandExecutor;
//!
//! let game_state = Arc::new(Mutex::new(MyGameState::new()));
//! let mut executor = CommandExecutor::new(game_state.clone());
//!
//! // Execute a command
//! executor.execute_command(&command, player_id)?;
//! ```
//!
//! ## 3. Set Up CRC Validation
//!
//! ```rust,ignore
//! use game_network::integration::GameStateCRCValidator;
//!
//! let validator = GameStateCRCValidator::new(game_state.clone());
//!
//! // Compute and validate CRC
//! let local_crc = validator.compute_crc(frame)?;
//! let is_synced = validator.validate(frame, remote_crc)?;
//! ```
//!
//! ## 4. Handle Desyncs
//!
//! ```rust,ignore
//! use game_network::integration::{DesyncHandler, DesyncStrategy};
//!
//! let mut desync_handler = DesyncHandler::with_settings(
//!     game_state.clone(),
//!     DesyncStrategy::DisconnectVote,
//!     Some(PathBuf::from("./dumps")),
//! );
//!
//! // Check for desync
//! let status = desync_handler.detect_desync(frame, local_crc, remote_crcs);
//!
//! match status {
//!     DesyncStatus::Desynchronized { frame, desynced_players } => {
//!         // Handle desync...
//!     }
//!     DesyncStatus::Synchronized => {
//!         // All good!
//!     }
//! }
//! ```

pub mod command_executor;
pub mod crc_validator;
pub mod desync_handler;
pub mod game_loop;
pub mod game_state;

// Re-export main types
pub use command_executor::{BatchCommandExecutor, CommandExecutor, ExecutionStats};
pub use crc_validator::{
    compute_entities_crc, compute_resources_crc, CRCComputer, GameStateCRCValidator,
};
pub use desync_handler::{
    DesyncHandler, DesyncRecord, DesyncStatus, DesyncStrategy, MultiPlayerCRCValidator,
};
pub use game_loop::{GameLoopConfig, GameLoopExecutor, GameLoopStats};
pub use game_state::{
    CRCValue, CommandConverter, EntityId, EntitySnapshot, FrameNumber, GameState, GameStateCRC,
    GameStateError, GameStateResult, PlayerId, ResourceState,
};
