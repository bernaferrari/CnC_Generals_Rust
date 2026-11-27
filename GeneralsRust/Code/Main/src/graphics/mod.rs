/*
** Command & Conquer Generals Zero Hour(tm) - C++ SAGE Engine Equivalent Modules
** Copyright 2025 Electronic Arts Inc.
**
** Module declarations for C++ SAGE engine equivalent structures
*/

pub mod fow_uniform_integration;
pub mod graphics_system;
pub mod laser_draw;
pub mod laser_segment_upload;
pub mod projected_shroud_upload;
pub use projected_shroud_upload::{
    PROJECTED_SHROUD_SAMPLER_POLICY, PROJECTED_SHROUD_TEXTURE_FORMAT, ProjectedShroudGpuUploader,
    ProjectedShroudSamplerPolicy, ProjectedShroudTextureState, ProjectedShroudUploadAction,
    ProjectedShroudUploadPlan,
};
pub mod minimap_renderer;
pub mod occlusion_bridge;

pub mod render_item;
pub mod render_pipeline;

pub(crate) mod radar_view;
mod order_lines;
pub mod selection_renderer;
pub(crate) mod ui_radar;
#[cfg(feature = "integration-tests")]
pub use ui_radar::NativeRadarFrame;
pub mod ui_render_pass;
pub use graphics_system::{GlobalUniforms, GraphicsStatistics, GraphicsSystem};
pub use laser_segment_upload::{
    LASER_BYTES_PER_SEGMENT, LASER_VERTEX_FLOATS, LASER_VERTS_PER_SEGMENT, LaserSegmentUpload,
};
pub use minimap_renderer::{MinimapCoordinates, MinimapTextureRenderer, UiTextureRegistrar};
pub use render_item::RenderItem;
pub use render_pipeline::{RenderPass, RenderPipeline};
