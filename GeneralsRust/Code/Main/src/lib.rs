#![allow(ambiguous_glob_reexports)]
#![allow(dead_code)]
#![allow(missing_docs)]
#![allow(unused_imports)]
#![allow(unused_doc_comments)]

//! Command & Conquer Generals Main Application Module
//!
//! This library provides the main entry point and core initialization
//! for the Command & Conquer Generals Zero Hour game.
//!
//! Complete integration of wgpu graphics, rodio audio, and game systems.

extern crate ww3d_renderer_3d as ww3d_renderer_3d;

pub mod assets;
pub mod cnc_game_engine;
pub mod fow_rendering;
pub mod game_engine;
pub mod game_logic;
pub mod game_results_queue;
pub mod graphics;
#[cfg(feature = "integration-diagnostics")]
pub mod integration_bridge;
pub mod localization;
pub mod resource;
pub mod ui;
pub mod util;
pub mod win_main;
// pub mod ui_demo; // Temporarily disabled due to import issues
pub mod ai;
pub mod ai_decisions;
pub mod ai_skirmish;
pub mod command_executor;
pub mod command_integration;
pub mod command_system;
pub mod config;
pub mod effects;
pub mod input_integration;
pub mod input_system;
pub mod input_system_simple;
pub mod input_test;
#[cfg(feature = "network")]
pub mod network;
#[cfg(not(feature = "network"))]
pub mod network {
    use std::sync::Arc;
    use tokio::sync::RwLock;

    #[derive(Debug, Clone)]
    pub struct NetworkInterface;

    #[derive(Debug, Clone)]
    pub struct NetworkConfig;

    impl Default for NetworkConfig {
        fn default() -> Self {
            Self
        }
    }

    #[derive(Debug, Clone)]
    pub enum NetworkError {
        Disabled,
    }

    impl std::fmt::Display for NetworkError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Disabled => write!(f, "network feature is disabled"),
            }
        }
    }

    impl std::error::Error for NetworkError {}

    pub type NetworkResult<T> = Result<T, NetworkError>;

    impl NetworkInterface {
        pub fn new(_config: NetworkConfig) -> NetworkResult<Self> {
            Err(NetworkError::Disabled)
        }
    }

    pub fn init_network() -> NetworkResult<Arc<RwLock<NetworkInterface>>> {
        Err(NetworkError::Disabled)
    }

    pub fn create_network_interface(
        _config: NetworkConfig,
    ) -> NetworkResult<Arc<RwLock<NetworkInterface>>> {
        Err(NetworkError::Disabled)
    }

    pub fn has_active_network_interface() -> bool {
        false
    }

    pub fn active_session_frame_data_ready() -> Option<bool> {
        None
    }

    pub fn clear_active_network_interface() {}
}
pub mod save_load;
pub mod selection_renderer;
pub mod unit_control;
pub mod unit_input_handler;
pub mod win32_game_engine;

// Playability integration
pub mod ai_skirmish_activity;
pub mod authoritative_world;
pub mod presentation_frame;
pub mod breadth_scenarios;
pub mod golden_skirmish;
pub mod map_frame_scenario;
pub mod playability_integration;
pub mod release_candidate;
pub mod skirmish_config;

// New factory pattern modules
pub mod command_line;
pub mod copy_protection;
pub mod debug_system;
pub mod deterministic_trace;
pub mod engine_factory;
pub mod platform;
pub mod single_instance;
pub mod subsystem_interfaces;
pub mod subsystem_manager;
pub mod version;

// Re-export main functionality
pub use ::game_engine::common::frame_clock::FrameClock;
pub use fow_rendering::*;
pub use game_engine::*;
#[cfg(feature = "integration-diagnostics")]
pub use integration_bridge::*;
pub use localization::*;
pub use resource::*;
pub use ui::*;
pub use win_main::*;
// pub use ui_demo::*; // Temporarily disabled
pub use ai::*;
pub use ai_decisions::*;
pub use ai_skirmish::*;
pub use config::*;
pub use input_integration::*;
pub use input_system::*;
pub use input_system_simple::*;
pub use input_test::*;
pub use network::*;
pub use save_load::*;
pub use selection_renderer::*;
pub use unit_control::*;
pub use unit_input_handler::*;
pub use win32_game_engine::*;

// Re-export new modules
pub use command_line::*;
pub use copy_protection::*;
pub use debug_system::*;
pub use deterministic_trace::*;
pub use engine_factory::*;
pub use platform::*;
pub use single_instance::*;
pub use subsystem_interfaces::*;
pub use subsystem_manager::*;
pub use version::*;

pub mod runtime;
// Test modules (opt-in so `cargo test` can run lightweight unit tests without heavy UI harness)
#[cfg(all(test, feature = "integration-tests"))]
pub mod tests {
    pub mod command_integration_tests;
    pub mod fow_integration_tests;
    pub mod game_loop_integration_tests;
}
