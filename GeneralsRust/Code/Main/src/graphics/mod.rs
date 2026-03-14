/*
** Command & Conquer Generals Zero Hour(tm) - C++ SAGE Engine Equivalent Modules
** Copyright 2025 Electronic Arts Inc.
**
** Module declarations for C++ SAGE engine equivalent structures
*/

pub mod fow_uniform_integration;
pub mod graphics_system;
pub mod minimap_renderer;
pub mod render_item;
pub mod render_pipeline;
pub use graphics_system::{GlobalUniforms, GraphicsStatistics, GraphicsSystem};
pub use minimap_renderer::{MinimapCoordinates, MinimapTextureRenderer, UiTextureRegistrar};
pub use render_item::RenderItem;
pub use render_pipeline::{RenderPass, RenderPipeline};
