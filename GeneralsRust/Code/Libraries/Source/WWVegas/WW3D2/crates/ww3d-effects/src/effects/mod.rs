//! Effects system modules
//!
//! This module contains all visual effects functionality:
//! - Particle systems and emitters
//! - Light effects (dazzle, lens flares)
//! - Destruction effects (shatter system)
//! - Decal projection and rendering
//! - Trail effects (streak system)
//! - Ring effects for various visual phenomena

pub mod dazzle_renderer;
pub mod dazzle_system;
pub mod decal_system;
pub mod mesh_render_obj;
pub mod particle_emitter_core;
pub mod particle_system;
pub mod rendering_integration;
pub mod ring_system;
pub mod segline_render_obj;
pub mod shatter_system;
pub mod shatter_system_csg;
pub mod sphere_render_obj;
pub mod streak_advanced;
pub mod streak_system;

// Re-export commonly used effect types
pub use dazzle_renderer::{DazzleGpuRenderer, DazzleRenderInstance, LensFlare, LensFlareConfig};
pub use dazzle_system::{DazzleManager, DazzleRenderObj};
pub use decal_system::{
    DecalGenerator, DecalSystem, MultiFixedPoolDecalSystem, RigidDecalMesh, SkinDecalMesh,
};
pub use mesh_render_obj::MeshRenderObj;
pub use particle_system::ParticleSystem;
pub use rendering_integration::{
    get_decal_geometry, get_fragment_geometry, should_cull_fragment, update_fragments,
};
pub use ring_system::RingManager;
pub use segline_render_obj::SegLineRenderObj;
pub use shatter_system::ShatterSystem;
pub use shatter_system_csg::{BspNode, MeshFragment, ShatterSystem as ShatterSystemCSG};
pub use sphere_render_obj::SphereRenderObj;
pub use streak_advanced::{AdvancedStreak, LightningStreak, StreakLodConfig, SubdivisionConfig};
pub use streak_system::StreakRenderer;
