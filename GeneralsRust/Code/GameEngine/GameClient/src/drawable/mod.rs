//! Drawable System - Rendering and Scene Management
//!
//! This module provides the drawable system for the Command & Conquer Generals game client,
//! handling 3D objects, 2D sprites, UI elements, and their rendering properties.
//!
//! The system is organized around several key components:
//! - `Drawable` trait for all renderable objects
//! - `DrawableManager` for scene graph management and culling
//! - Various drawable types (models, sprites, particles, UI elements)
//! - Animation and effect systems
//! - Z-ordering and transparency management
//!
//! # Architecture
//!
//! The drawable system follows a modular design where:
//! - All renderable objects implement the `Drawable` trait
//! - The `DrawableManager` maintains the scene graph and handles:
//!   - Spatial organization and culling
//!   - Drawing order (Z-depth, transparency)
//!   - Animation and update systems
//!   - Memory management and lifecycle
//!
//! # Examples
//!
//! ```rust
//! use crate::drawable::{Drawable, DrawableManager, DrawableType};
//! use crate::core::math::{Vector3, Matrix4};
//!
//! // Create a drawable manager
//! let mut manager = DrawableManager::new();
//!
//! // Add a drawable object
//! let drawable_id = manager.create_drawable(DrawableType::Model {
//!     model_name: "tank.w3d".to_string(),
//!     position: Vector3::new(0.0, 0.0, 0.0),
//!     scale: 1.0,
//! });
//!
//! // Update and render
//! manager.update(delta_time);
//! manager.render(&view_matrix, &projection_matrix);
//! ```

pub mod drawable;
pub mod drawable_manager;
pub mod update;

pub use crate::drawable_info::{DrawableInfo, ExtraRenderFlags};

// Re-export commonly used types
pub use drawable::{
    BasicDrawable, Color, Drawable, DrawableDowncast, DrawableExt, DrawableId, DrawableOverlayData,
    DrawableStatus, DrawableType, EnvelopeState, ICoord2D, IRegion2D, Icon, IconInfo, IconType,
    LocoInfo, Matrix4, StealthLook, TerrainDecalType, TintEnvelope, TintStatus, Vector3, WheelInfo,
    INVALID_DRAWABLE_ID,
};

pub use drawable_manager::{DrawLayer, DrawableManager, Frustum, RenderPass, RenderStats, Vector4};
pub use update::{
    AnimatedParticleSysBoneClientUpdateModule, BeaconClientUpdateModule,
    BeaconClientUpdateModuleData, SwayClientUpdateModule,
};
