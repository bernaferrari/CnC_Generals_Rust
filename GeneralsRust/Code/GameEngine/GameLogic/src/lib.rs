//! Game Logic System - Rust Implementation
//!
//! This crate provides the core game logic systems for Command & Conquer Generals Zero Hour,
//! converted from the original C++ implementation to idiomatic Rust.
//!
//! The main systems include:
//! - AI system with pathfinding and group management
//! - Damage and weapon systems
//! - Behavior and update systems
//! - Scripting engine integration
//! - Object management and lifecycle

#![allow(missing_docs)]
#![allow(dead_code)]
#![allow(unused_imports)]
#![warn(rust_2018_idioms)]

// Public modules
pub mod action_manager;
pub mod ai;
pub mod error;
pub mod helpers;
pub mod system;

pub mod contain_module_overrides;
pub mod game_logic;
pub mod modules;
pub mod object;
pub mod script_engine;
pub mod scripting;
pub mod weapon;

pub mod attack;
pub mod build_list_info;
pub mod command_button;
pub mod commands;
pub mod compat;
pub mod contain_module;
pub mod control_bar;
pub mod damage;
pub mod drawable;
pub mod economy;
pub mod effects;
pub mod experience;
pub mod formation;
pub mod locomotor;
pub mod locomotor_impl;
pub mod map;
pub mod messages;
pub mod path;
pub mod physics;
pub mod player;
pub mod polygon_trigger;
pub mod resource;
pub mod resource_world;
pub mod special_power;
pub mod squad;
pub mod stealth;
pub mod stealth_update;
pub mod supply_system;
pub mod waypoint;
pub mod world;
pub use player::PlayerArcExt;
pub mod alliance;
pub mod object_creation_list;
pub mod object_manager;
pub mod pow_truck_ai_update;
pub mod sides_list;
pub mod special_power_module;
pub mod state_machine;
pub mod team;
pub mod template;
pub mod terrain;
pub mod thing_template;
pub mod tunnel_tracker;
pub mod upgrade;
pub mod upgrade_legacy;

// Network transport layer
pub mod transport;

// Internal/common modules
pub mod common;
pub mod logic;
pub mod prelude;
pub mod runtime;

// Integration tests (disabled - needs API updates)
// #[cfg(test)]
// mod integration_tests;

// Performance benchmarks (run with --ignored flag)
#[cfg(test)]
mod benchmarks;

// Stress tests (run with --ignored flag)
#[cfg(test)]
mod stress_tests;

// Stability tests (run with --ignored flag)
#[cfg(test)]
mod stability_tests;

#[cfg(test)]
pub mod test_sync {
    use once_cell::sync::Lazy;
    use std::sync::{Mutex, MutexGuard};

    static GLOBAL_TEST_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    pub fn lock() -> MutexGuard<'static, ()> {
        GLOBAL_TEST_LOCK
            .lock()
            .expect("global test synchronization lock poisoned")
    }
}

// Re-export commonly used types from the AI module
pub use ai::{
    AiCommandInterface, AiCommandParams, AiCommandType, AiData, AiError, AiGroup, AttitudeType,
    CommandSourceType, AI, THE_AI,
};

pub use common::GeometryInfo;
pub use object::{Object, ObjectId, ObjectTemplate};

pub use modules::{ModuleInterface, UpdateModule, UpdateModulePtr};

pub use weapon::{
    initialize_weapon_store, with_weapon_store, with_weapon_store_mut, Coord2D, Coord3D,
    DamageType, DeathType, VeterancyLevel, Weapon, WeaponAffectsMask, WeaponAntiMask, WeaponBonus,
    WeaponBonusConditionFlags, WeaponBonusSet, WeaponCollideMask, WeaponPrefireType,
    WeaponReloadType, WeaponSlotType, WeaponStatus, WeaponStore, WeaponTemplate,
};

pub use system::game_logic::{
    get_game_logic, init_game_logic, reset_game_logic, update_game_logic, BuildableStatus, CrcMode,
    GameLogic, GameMode, Snapshot, SubsystemInterface, MAX_SLOTS,
};

pub use common::INVALID_ID;
pub use player::{player_list, GameDifficulty};
#[cfg(feature = "network")]
pub use system::network_bridge::{BridgeStatistics, NetworkCommandBridge};
#[cfg(not(feature = "network"))]
pub use system::network_bridge_stub::{BridgeStatistics, NetworkCommandBridge};

pub use scripting::{
    get_script_engine, ActionRegistry, ConditionRegistry, EventFilter, EventManager, GameEvent,
    GameEventType, GameStateContext, Script, ScriptContext, ScriptPriority, ScriptResult,
    ScriptTrigger, ScriptValue, ScriptingEngine, VictoryCondition, VictoryManager,
};

pub use team::get_team_factory;

// Re-export singleton stubs from helpers for gameplay systems
pub use helpers::{TheInGameUI, ThePartitionManager, TheRadar};

// Re-export ModuleFactory from game_engine for convenient access
pub use game_engine::common::thing::module_factory::get_module_factory;

pub type GameLogicResult<T> = Result<T, GameLogicError>;

/// Main error type for game logic operations
#[derive(Debug, thiserror::Error)]
pub enum GameLogicError {
    /// AI system errors
    #[error("AI error: {0}")]
    Ai(#[from] ai::AiError),
    /// Weapon system errors
    #[error("Weapon error: {0}")]
    Weapon(#[from] weapon::WeaponError),

    /// Invalid object reference
    #[error("Invalid object ID: {0}")]
    InvalidObject(u32),

    /// System not initialized
    #[error("System not initialized: {0}")]
    SystemNotInitialized(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// Threading error
    #[error("Threading error: {0}")]
    Threading(String),

    /// Module failure
    #[error("Module error: {0}")]
    ModuleError(String),

    /// I/O operation failure
    #[error("I/O error: {0}")]
    IO(String),
}

/// Initialize the game logic systems
///
/// This function should be called once at the start of the program to initialize
/// all game logic subsystems.
pub fn initialize() -> GameLogicResult<()> {
    // Initialize core GameLogic system
    init_game_logic().map_err(|e| {
        GameLogicError::SystemNotInitialized(format!("GameLogic init failed: {}", e))
    })?;

    // Initialize AI system
    #[cfg(feature = "legacy_port")]
    {
        THE_AI
            .write()
            .map_err(|e| GameLogicError::Threading(format!("Failed to acquire AI lock: {}", e)))?
            .init();
    }

    // Initialize weapon system
    initialize_weapon_store()?;

    // Register terrain height provider for Common layer's Thing trait
    game_engine::common::thing::register_terrain_height_provider(|x, y| {
        let terrain = crate::terrain::get_terrain_logic();
        terrain
            .read()
            .ok()
            .map(|guard| guard.get_ground_height(x, y, None))
            .unwrap_or(0.0)
    });
    game_engine::common::thing::register_underwater_provider(|x, y| {
        let terrain = crate::terrain::get_terrain_logic();
        terrain
            .read()
            .ok()
            .map(|guard| {
                let mut water_z = 0.0f32;
                let underwater = guard.is_underwater(x, y, Some(&mut water_z), None);
                (underwater, water_z)
            })
            .unwrap_or((false, 0.0))
    });

    log::info!("Game Logic systems initialized successfully");
    Ok(())
}

/// Reset all game logic systems
///
/// This function should be called when loading a new map or restarting the game
/// to reset all subsystems to their initial state.
pub fn reset() -> GameLogicResult<()> {
    // Reset core GameLogic system
    reset_game_logic().map_err(|e| {
        GameLogicError::SystemNotInitialized(format!("GameLogic reset failed: {}", e))
    })?;

    // Reset AI system
    THE_AI
        .write()
        .map_err(|e| GameLogicError::Threading(format!("Failed to acquire AI lock: {}", e)))?
        .reset();

    // Reset weapon system
    with_weapon_store_mut(|store| store.reset())??;

    log::info!("Game Logic systems reset successfully");
    Ok(())
}

/// Update all game logic systems for one frame
///
/// This function should be called once per game frame to update all subsystems.
pub fn update() -> GameLogicResult<()> {
    // Update core GameLogic system (this handles the main game loop)
    update_game_logic().map_err(|e| {
        GameLogicError::SystemNotInitialized(format!("GameLogic update failed: {}", e))
    })?;

    // Get current frame for AI update
    let frame = {
        let mutex = get_game_logic();
        let logic = mutex.lock().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire GameLogic lock: {}", e))
        })?;
        logic.get_frame()
    };

    // Update AI system
    THE_AI
        .write()
        .map_err(|e| GameLogicError::Threading(format!("Failed to acquire AI lock: {}", e)))?
        .update(frame)
        .map_err(GameLogicError::Ai)?;

    // Update weapon system
    with_weapon_store_mut(|store| store.update())??;

    Ok(())
}

/// Get the current version of the game logic system
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialization() {
        assert!(initialize().is_ok());
    }

    #[test]
    fn test_reset() {
        initialize().unwrap();
        assert!(reset().is_ok());
    }

    #[test]
    fn test_version() {
        let version = version();
        assert!(!version.is_empty());
    }

    #[test]
    fn test_ai_singleton() {
        // Test that THE_AI is accessible
        let ai = &*THE_AI;
        assert!(ai.read().is_ok());
    }
}
