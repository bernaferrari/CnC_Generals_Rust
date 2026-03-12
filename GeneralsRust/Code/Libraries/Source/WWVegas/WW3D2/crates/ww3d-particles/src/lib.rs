//! WW3D Particle System
//!
//! This crate implements the complete particle system from the original WW3D engine,
//! including particle emitters, buffers, streaks, point groups, and line renderers.
//! It provides a comprehensive solution for particle effects in 3D graphics.

pub mod buffer;
pub mod emitter;
pub mod integration;
pub mod line_renderer;
pub mod loader;
pub mod manager;
pub mod point_group;
pub mod properties;
pub mod sorting_renderer;
pub mod streak;

pub mod sortingrenderer;
pub use buffer::*;
pub use emitter::*;
pub use integration::*;
pub use line_renderer::{LineVertex, SegLineRendererClass, TextureMappingMode};
pub use loader::*;
pub use manager::*;
pub use point_group::*;
pub use properties::*;
pub use sorting_renderer::*;
pub use streak::{SegmentedLineRenderer, StreakLine};

/// Particle system runtime trait implemented by concrete managers.
pub trait ParticleSystem: std::fmt::Debug {
    fn update(&mut self, delta_time_ms: u32);
    fn emit(&mut self, transform: glam::Mat4);
    fn render<'pass>(&'pass mut self, render_pass: &mut wgpu::RenderPass<'pass>);
    fn active_particle_count(&self) -> usize;
    fn sorting_enabled(&self) -> bool;
    fn enable_sorting(&mut self, enabled: bool);
    fn sorting_stats(&self) -> (usize, usize, usize);
}
