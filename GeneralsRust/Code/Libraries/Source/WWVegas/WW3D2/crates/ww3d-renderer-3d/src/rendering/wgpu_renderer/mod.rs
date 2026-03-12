//! # WGPU Renderer Module
//!
//! This module provides a WGPU-based replacement for the DirectX8 wrapper functionality.
//! It maintains the same API as the original DX8Wrapper but uses modern WGPU for cross-platform rendering.
//!
//! ## Features
//!
//! - Cross-platform rendering with WGPU
//! - DirectX8 API compatibility layer
//! - Modern shader pipeline support
//! - Efficient resource management
//! - Statistics and profiling support
//! - DirectX8 FVF to WGPU vertex format conversion
//! - Comprehensive texture management
//! - Buffer management with reference counting

// Core WGPU wrapper (equivalent to DX8Wrapper)
pub mod wgpu_wrapper;

// Buffer management (equivalent to DX8 vertex/index buffers)
pub mod wgpu_buffer;

// Texture management (equivalent to DX8 texture system)
pub mod wgpu_texture;
pub mod wgpu_texture_manager;

// Shader management (equivalent to DX8 shader system)
pub mod wgpu_shader;

// Render state management
pub mod wgpu_render_state;

// Runtime helpers for bootstrapping Devices/Queues/Surfaces
pub mod runtime;

// Bind helpers and pipeline manager for WGPU
pub mod wgpu_material_binds;
pub mod wgpu_pipeline_manager;

// Device and surface management
pub mod wgpu_adapter;
pub mod wgpu_device;
pub mod wgpu_surface;

// Vertex format conversion (equivalent to DX8 FVF)
pub mod wgpu_vertex_format;

// Re-export a curated set of entry points for the higher level renderer
pub use runtime::{RuntimeBuilder, RuntimeParts};
pub use wgpu_render_state::RenderStateStruct as WgpuRenderState;
pub use wgpu_wrapper::WgpuWrapper;
