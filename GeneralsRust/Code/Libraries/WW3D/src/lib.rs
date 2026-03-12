// W3D (Westwood 3D) Model Rendering System
// Faithful port from C++ to Rust with wgpu backend

pub mod w3d_file;
pub mod math;
pub mod render_object;
pub mod mesh;
pub mod mesh_geometry;
pub mod mesh_model;
pub mod hierarchy;
pub mod animation;
pub mod material;
pub mod texture;
pub mod shader;
pub mod renderer;
pub mod culling;
pub mod collision;
pub mod prototype;

pub use w3d_file::*;
pub use math::*;
pub use render_object::*;
pub use mesh::*;
pub use mesh_geometry::*;
pub use mesh_model::*;
pub use hierarchy::*;
pub use animation::*;
pub use material::*;
pub use texture::*;
pub use shader::*;
pub use renderer::*;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum W3DError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid chunk type: {0}")]
    InvalidChunkType(u32),

    #[error("Invalid mesh data")]
    InvalidMeshData,

    #[error("Animation error: {0}")]
    AnimationError(String),

    #[error("Render error: {0}")]
    RenderError(String),

    #[error("Resource not found: {0}")]
    ResourceNotFound(String),
}

pub type Result<T> = std::result::Result<T, W3DError>;
