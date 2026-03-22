//! # Game Client Rust
//!
//! This is a Rust conversion of the Command & Conquer Generals Zero Hour GameClient.
//! It provides all the client-side functionality including graphics, UI, input handling,
//! audio, and game presentation layers.
//!
//! The conversion maintains API compatibility with the original C++ implementation while
//! leveraging Rust's safety guarantees and modern ecosystem.

#![allow(missing_docs, unused_doc_comments)]
#![allow(dead_code)] // Allow during development phase
#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(private_interfaces)]
#![allow(unused_imports)]
#![allow(unused_assignments)]
#![cfg_attr(test, cfg(feature = "internal"))]

// Core modules
#[allow(non_snake_case)]
pub mod HotKey;
pub mod audio;
pub mod core;
pub mod display;
pub mod drawable;
pub mod drawable_info;
pub mod drawable_manager;
pub mod effects;
pub mod eva;
pub mod fx_list;
pub mod game_client;
pub mod game_client_dispatch;
pub mod graph_draw;
pub mod gui;
pub mod helpers;
pub mod in_game_ui;
pub mod input;
pub mod language_filter;
pub mod line2_d;
pub mod map_util;
pub mod parabolic_ease;
pub mod radius_decal;
pub mod selection_info;
pub mod snow;
pub mod statistics;
pub mod system;
pub mod terrain;
pub mod video_buffer;
pub mod video_player;
pub mod video_stream;
pub mod view;
pub mod water;

// Complete asset loading system
pub mod assets;
pub mod color;
pub mod credits;
pub mod display_string;
pub mod display_string_manager;
pub mod draw_group_info;
pub mod game_text;
#[cfg(feature = "network")]
pub mod gamespy_game;
#[cfg(feature = "network")]
pub mod gamespy_overlay;
pub mod global_language;

// Message processing system
pub mod message_stream;
pub mod network;

#[cfg(not(feature = "network"))]
#[path = "game_network.rs"]
mod game_network_compat;
#[cfg(not(feature = "network"))]
pub use game_network_compat::*;
#[cfg(not(feature = "network"))]
extern crate self as game_network;

// Platform abstraction layer
pub mod platform;

// Render bridge — connects GameLogic draw modules to WWVegas W3D renderer
pub mod render_bridge;

// Revolutionary W3D Engine
#[cfg(feature = "w3d_support")]
pub mod w3d;

// Consumers can import individual modules directly; we intentionally avoid
// glob re-exports to keep the public API explicit and reduce name clashes.

/// Common error type for the game client
pub type GameClientResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Version information for the game client
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Initialize the game client library
pub fn init() -> GameClientResult<()> {
    env_logger::init();
    log::info!("Initializing Game Client Rust v{}", VERSION);
    Ok(())
}
pub mod cd_check;
pub mod client_random_value;
pub mod font_desc;
pub mod gadget;
pub mod gadget_slider;
pub mod game_window_id;
pub mod gui_callbacks;
pub mod input_bridge;
pub mod key_defs;
pub mod selection_system;
pub mod shadow;
pub mod shell_hooks;
