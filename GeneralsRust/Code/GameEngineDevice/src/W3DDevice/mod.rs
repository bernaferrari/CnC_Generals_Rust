//! W3D Device Infrastructure
//!
//! This module provides the W3D rendering infrastructure for C&C Generals Zero Hour,
//! including model loading, asset management, animation, skeletal systems, and materials.
//!
//! # Architecture
//!
//! - `w3d_loader`: Binary W3D file parsing with nom combinators
//! - `asset_manager`: Thread-safe asset caching and lifecycle management
//! - `animation`: Frame-based animation playback and blending
//! - `skeleton`: Hierarchical bone system with transform computation
//! - `material`: Material parameters and texture management
//!
//! # Thread Safety
//!
//! All major components use Arc<RwLock<>> for thread-safe access and support
//! concurrent loading and rendering operations.

pub mod w3d_loader;
pub mod asset_manager;
pub mod animation;
pub mod skeleton;
pub mod material;
#[path = "Common/mod.rs"]
pub mod common;
#[path = "GameClient/mod.rs"]
pub mod game_client;
#[path = "GameLogic/mod.rs"]
pub mod game_logic;

// Re-export main types for convenience
pub use w3d_loader::{W3DLoader, W3DFile, W3DChunk, W3DError};
pub use asset_manager::{W3DAssetManager, AssetHandle, AssetCache};
pub use animation::{Animation, AnimationState, AnimationBlender};
pub use skeleton::{Skeleton, Bone, BoneTransform};
pub use material::{Material, Texture, TextureManager};
