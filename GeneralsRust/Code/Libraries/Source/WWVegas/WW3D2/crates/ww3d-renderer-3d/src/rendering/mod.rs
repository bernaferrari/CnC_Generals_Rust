//! Rendering system modules

pub mod camera_system;
pub mod frame_graph;
pub mod frame_uniform_arena;
pub mod frustum;
pub mod light_system;
pub mod lighting_system;
pub mod mesh_system;
pub mod shader_core;
pub mod shader_system;
// DX8-era modules removed in favor of WGPU
// pub mod dx8caps;
// pub mod dx8fvf;
pub mod hlod_system;
pub mod lod_system;
pub mod render2d;
pub mod shadow_system;
pub mod swapchain_state;
pub mod texture_decode;
pub mod texture_metrics;
pub mod texture_quality;
pub mod texture_system;
pub mod wgpu_main_renderer;
pub mod wgpu_renderer;

// New modules for complete feature parity
pub mod box_render_obj;
pub mod dynamic_mesh;
pub mod line3d_render_obj;
pub mod mesh_builder;
pub mod ring_render_obj;
pub mod segment_line_renderer;
pub mod sphere_render_obj;

// Render state machine components
pub mod render_state;
pub mod sort_system;

// Batch rendering for draw call optimization
pub mod batching;

// Advanced rendering features (matching C++ visual fidelity)
pub mod debug_render_modes;
pub mod post_process;
pub mod reflection_system;
pub mod render_target;

// Core mesh rendering components (Package 1: Core Mesh Rendering System)
pub mod mesh_geometry;
pub mod mesh_mat_desc;
pub mod mesh_model;

// Re-export commonly used types
pub use crate::scene_system::{PolyRenderType, SceneClass, SceneId, SceneManagerClass};
pub use shader_core::ShaderManager;

// Re-export mesh model types for convenience
pub use mesh_geometry::{GeometryFlags, MeshGeometry, ShareBuffer, TriIndex};
pub use mesh_mat_desc::{
    ColorSourceType, MatBuffer, MeshMatDesc, TexBuffer, UVBuffer, MAX_COLOR_ARRAYS, MAX_PASSES,
    MAX_TEX_STAGES, MAX_UV_ARRAYS,
};
pub use mesh_model::{GapFiller, MaterialInfo, MeshModel};

// Re-export procedural render objects
pub use ring_render_obj::RingRenderObj;
pub use segment_line_renderer::{
    SegLineRenderer, SegLineVertex, TextureMapMode, TriIndex as SegLineTriIndex,
    MAX_SEGLINE_POINT_BUFFER_SIZE, MAX_SEGLINE_POLY_BUFFER_SIZE, MAX_SEGLINE_SUBDIV_LEVELS,
    SEGLINE_CHUNK_SIZE,
};
