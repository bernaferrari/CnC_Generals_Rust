//! C++ compatibility shim for RendObj.h
//!
//! Re-exports the render object interfaces from the modern Rust module
//! to preserve the C++ public API naming expectations.

pub use crate::{CameraClass, RenderInfoClass, RenderObj, SceneClass};
