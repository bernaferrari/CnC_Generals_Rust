//! # Display Module
//!
//! Modern graphics display system providing cross-platform rendering capabilities.
//!
//! This module contains the main display management system and image/texture handling
//! for the Command & Conquer Generals Zero Hour game client. It provides:
//!
//! - Cross-platform rendering using wgpu
//! - Image loading and management with various format support
//! - Display mode management (resolution, fullscreen/windowed)
//! - Drawing primitives (lines, rectangles, images)
//! - GPU resource management
//! - Error handling for graphics operations
//!
//! ## Key Components
//!
//! - [`Display`] - Main display management and rendering system
//! - [`Image`] - High-level image representation and texture management
//! - [`ImageCollection`] - Asset management for collections of images
//!
//! ## Example Usage
//!
//! ```rust,no_run
//! use game_client_rust::display::{Display, DisplaySettings};
//! use winit::event_loop::EventLoop;
//!
//! async fn setup_display() -> Result<(), Box<dyn std::error::Error>> {
//!     let event_loop = EventLoop::new();
//!     let display = Display::new(&event_loop, 1920, 1080, false).await?;
//!     
//!     // Set up display mode
//!     display.set_display_mode(1920, 1080, 32, false)?;
//!     
//!     Ok(())
//! }
//! ```

use crate::system::SubsystemInterface;
use std::error::Error;

// Public modules
pub mod cinematic_camera;
pub mod display;
pub mod image;
pub mod movie_player;
pub mod texture_system;
pub mod video_texture;
pub mod view;

/// Legacy Display interface for compatibility
pub trait DisplayInterface: SubsystemInterface {
    fn draw(&self) -> Result<(), Box<dyn Error>>;
    fn preload_common_textures(&self) -> Result<(), Box<dyn Error>>;
}

/// Font library interface  
pub trait FontLibrary: SubsystemInterface {
    // Font operations
}

/// Window manager interface
pub trait GameWindowManager: SubsystemInterface {
    // Window management operations
}
