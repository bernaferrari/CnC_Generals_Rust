//! Game Engine Device Library
//!
//! This library provides platform-specific device implementations for
//! the Command & Conquer Generals game engine.

#[cfg(feature = "legacy-full")]
pub mod miles_audio_device;
#[cfg(feature = "legacy-full")]
#[path = "VideoDevice/mod.rs"]
pub mod video_device;
#[cfg(feature = "legacy-full")]
pub mod win32_device;
#[cfg(feature = "legacy-full")]
#[path = "W3DDevice/mod.rs"]
pub mod w3d_device;
pub mod w3d_device_compat;

#[cfg(feature = "legacy-full")]
pub use miles_audio_device::*;
#[cfg(feature = "legacy-full")]
pub use win32_device::*;
#[cfg(feature = "legacy-full")]
pub use w3d_device::*;
pub use w3d_device_compat::*;
