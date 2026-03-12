//! W3D Asset Loader Module
//!
//! This module provides the main asset loading interface, re-exporting
//! streaming and other loading functionality.

pub mod w3d_streaming_loader;

// Re-export key types for easy access
pub use w3d_streaming_loader::{
    AssetLoadError, AssetLoadRequest, AssetLoadResult, AssetType, LoadingPriority,
};
