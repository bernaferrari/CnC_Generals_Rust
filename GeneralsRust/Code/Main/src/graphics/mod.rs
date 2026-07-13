/*
** Command & Conquer Generals Zero Hour(tm) - C++ SAGE Engine Equivalent Modules
** Copyright 2025 Electronic Arts Inc.
**
** Module declarations for C++ SAGE engine equivalent structures
*/

pub mod floating_text_layout;
pub mod fow_uniform_integration;
pub mod graphics_system;
pub mod laser_segment_upload;
pub mod minimap_renderer;
pub mod render_item;
pub mod render_pipeline;
pub mod selection_renderer;
pub mod ui_render_pass;
pub use floating_text_layout::{
    pack_floating_text_and_mark_ready, FloatingTextLayout, FloatingTextLayoutEntry,
    FloatingTextLayoutHonesty, FLOATING_TEXT_LAYOUT_BYTES, FLOATING_TEXT_LAYOUT_FLOATS,
};
pub use graphics_system::{GlobalUniforms, GraphicsStatistics, GraphicsSystem};
pub use laser_segment_upload::{
    pack_and_mark_upload_ready, LaserSegmentUpload, LaserSegmentUploadHonesty, LaserSegmentVertex,
    LASER_BYTES_PER_SEGMENT, LASER_VERTEX_FLOATS, LASER_VERTS_PER_SEGMENT,
};
pub use minimap_renderer::{MinimapCoordinates, MinimapTextureRenderer, UiTextureRegistrar};
pub use render_item::RenderItem;
pub use render_pipeline::{RenderPass, RenderPipeline};
