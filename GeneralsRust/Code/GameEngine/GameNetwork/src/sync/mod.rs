//! Deterministic multiplayer game state synchronization.
//!
//! This module provides the lockstep synchronization system that ensures all
//! clients in a multiplayer game execute exactly the same sequence of game
//! commands in exactly the same order. It mirrors the C++ approach of a
//! fixed-rate game loop (30 fps) with a configurable run-ahead buffer.
//!
//! # Architecture
//!
//! - [`GameSynchronizer`] owns the main lockstep loop. It collects commands
//!   from local input and remote peers, orders them by frame number, and
//!   dispatches them to GameLogic.
//! - [`FrameBuffer`] stores frame history for replay verification and
//!   anti-cheat auditing.
//!
//! # C++ Reference
//!
//! - `NetworkLogic.cpp` -- main network frame loop
//! - `Network.cpp` -- command dispatch
//! - `GameClient.cpp` -- input collection
//! - `NetworkDefs.h` / `NetworkUtil.cpp` -- timing constants

pub mod frame_buffer;
pub mod game_sync;

pub use frame_buffer::FrameBuffer;
pub use game_sync::{
    CommandBuffer, DesyncRecoveryAction, GameSynchronizer, NetCommand, SyncConfig, SyncMetrics,
    SyncState,
};
