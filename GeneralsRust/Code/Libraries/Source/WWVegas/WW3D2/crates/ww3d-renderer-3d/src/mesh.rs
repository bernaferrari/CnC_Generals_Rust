//! Mesh system compatibility layer.
//!
//! The original stub exposed only placeholder structures. This module now re-exports the full
//! mesh implementation from `rendering::mesh_system` so higher-level systems (effects, shatter,
//! etc.) can depend on the real renderer-backed types while keeping legacy names.
//! Any additional helper aliases can be added here to mirror the C++ header layout.

pub use crate::render_object_system::MaterialInfoClass as MaterialInfo;
pub use crate::rendering::mesh_system::{MeshClass, MeshModelClass, MeshRenderManager};

/// Legacy namespace mirroring the original stub module layout.
pub mod mesh_core {
    pub use crate::rendering::mesh_system::{MeshClass, MeshModelClass, MeshRenderManager};
}
