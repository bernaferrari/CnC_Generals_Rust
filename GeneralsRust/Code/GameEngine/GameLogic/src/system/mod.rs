//! Compatibility shims for legacy `system::` namespaces from the C++ codebase.
//! These modules provide the minimal surface expected by partially ported
//! subsystems while we continue to rebuild the real implementations in Rust.

pub mod beacon_manager;
pub mod build_assistant_bridge;
pub mod cave_system;
pub mod crate_system;
pub mod damage;
pub mod detection_events;
pub mod detection_manager;
pub mod detection_modifiers;
pub mod detection_performance;
pub mod disguise_manager;
pub mod explored_territory;
pub mod game_logic;
pub mod game_logic_dispatch;
pub mod minimap_fow;
#[cfg(feature = "network")]
pub mod network_bridge;
#[cfg(not(feature = "network"))]
pub mod network_bridge_stub;
pub mod radar_notifier;
pub mod rank_info;
pub mod shroud_manager;
pub mod stealth_conditions;
pub mod stealth_errors;
pub mod stealth_features_missing;
pub mod stealth_integration;
pub mod stealth_integration_layer;
pub mod stealth_manager;
pub mod stealth_special_power;
pub mod stealth_upgrade;
pub mod stealth_validation;
pub mod thing_factory_bridge;

// Game initialization system modules
pub mod game_initialization;
pub mod game_start;
pub mod map_loader;
pub mod player_init;
pub mod victory_conditions;

#[cfg(feature = "network")]
pub use network_bridge::{BridgeStatistics, NetworkCommandBridge};
#[cfg(not(feature = "network"))]
pub use network_bridge_stub::{BridgeStatistics, NetworkCommandBridge};

// Re-export commonly used initialization types
pub use map_loader::{
    BridgeData, Coord2D, Coord3D, HeightMap, ICoord2D, LoadError, MapCache, MapData, MapLoader,
    MapMetaData, Region3D, WaypointID, WaypointMap, INVALID_WAYPOINT_ID, MAP_XY_FACTOR, MAX_SLOTS,
};

pub use game_engine::common::rts::player_template::PlayerTemplate;
pub use player_init::{
    make_observer_template, make_player_template, Difficulty, Player, PlayerColor, PlayerIndex,
    PlayerInitializer, PlayerList, PlayerRelationship, DEFAULT_PLAYER_COLORS,
    DEFAULT_STARTING_MONEY, MAX_PLAYER_COUNT,
};

pub use game_start::{
    AIPlayerState, CameraPosition, FogOfWar, GameStartSequence, MinimapGenerator, ScriptResult,
};

pub use victory_conditions::{
    EliminationDetector, GameResult, PlayerScore, ScoreCategory, ScoreKeeper, VictoryConditions,
    VictoryType,
};

pub use game_initialization::{
    GameDifficulty, GameInitParams, GameInitializer, GameMode, GameState, MapCacheManager,
};

#[cfg(test)]
pub mod stealth_detection_integration_tests;

#[cfg(test)]
pub mod stealth_detection_comprehensive_tests;

#[cfg(test)]
pub mod stealth_stress_tests;
