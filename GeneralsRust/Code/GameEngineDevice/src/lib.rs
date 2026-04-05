//! Game Engine Device Library
//!
//! This library provides platform-specific device implementations for
//! the Command & Conquer Generals game engine.

pub mod miles_audio_device;
pub mod video_device;
pub mod win32_device;
#[path = "W3DDevice/mod.rs"]
pub mod w3d_device;
pub mod w3d_device_compat;

pub use miles_audio_device::*;
pub use win32_device::*;
pub use w3d_device::*;
pub use w3d_device_compat::*;
