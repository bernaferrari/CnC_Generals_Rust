#![allow(clippy::pedantic)]
#![allow(clippy::nursery)]
#![allow(clippy::cargo)]
#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(unused_assignments)]
#![allow(deprecated)]
#![allow(nonstandard_style)]
#![allow(unconditional_recursion)]
#![allow(mismatched_lifetime_syntaxes)]
#![allow(unexpected_cfgs)]
#![allow(private_interfaces)]
#![allow(missing_docs)]
#![allow(unused_parens)]
#![allow(unused_must_use)]
#![allow(unreachable_patterns)]
#![allow(noop_method_call)]
#![allow(rust_2018_idioms)]

//! Game Engine Device Library
//!
//! This library provides platform-specific device implementations for
//! the Command & Conquer Generals game engine.

#[cfg(feature = "legacy-full")]
pub mod miles_audio_device;
#[cfg(feature = "legacy-full")]
#[path = "VideoDevice/mod.rs"]
pub mod video_device;
#[cfg(all(windows, feature = "legacy-full"))]
pub mod win32_device;
#[cfg(feature = "legacy-full")]
#[path = "W3DDevice/mod.rs"]
pub mod w3d_device;
pub mod w3d_device_compat;

#[cfg(feature = "legacy-full")]
pub use miles_audio_device::*;
#[cfg(all(windows, feature = "legacy-full"))]
pub use win32_device::*;
#[cfg(feature = "legacy-full")]
pub use w3d_device::*;
pub use w3d_device_compat::*;
