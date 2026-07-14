/*
** Command & Conquer Generals Zero Hour(tm) - C++ SAGE Engine Equivalent Modules
** Copyright 2025 Electronic Arts Inc.
**
** Module declarations for C++ SAGE engine equivalent structures
*/

pub mod floating_text_layout;
pub mod fow_uniform_integration;
pub mod game_text_residual;
pub mod graphics_system;
pub mod laser_segment_upload;
pub mod minimap_renderer;
pub mod render_item;
pub mod render_pipeline;
pub mod selection_renderer;
pub mod ui_render_pass;
pub mod world_anim_layout;
pub use floating_text_layout::{
    honesty_display_string_vanish_color_alpha_residual_ok,
    honesty_graphics_residual_pack_wave76_ok, honesty_ingame_ui_font_table_residual_ok,
    pack_floating_text_and_mark_ready, resolve_add_cash_caption, FloatingTextLayout,
    FloatingTextLayoutEntry, FloatingTextLayoutHonesty, FLOATING_TEXT_FONT_NAME,
    FLOATING_TEXT_FONT_POINT_SIZE, FLOATING_TEXT_LAYOUT_BYTES, FLOATING_TEXT_LAYOUT_FLOATS,
    GUI_ADD_CASH_KEY, INGAME_UI_FONT_RESIDUAL_TABLE,
};
pub use game_text_residual::{
    exercise_host_game_text_residual, format_printf_d, measure_display_string_residual,
    GameTextResidualExercise, GameTextResidualHonesty, GUI_ADD_CASH_RETAIL_TEMPLATE,
};
pub use graphics_system::{GlobalUniforms, GraphicsStatistics, GraphicsSystem};
pub use laser_segment_upload::{
    pack_and_mark_upload_ready, LaserSegmentUpload, LaserSegmentUploadHonesty, LaserSegmentVertex,
    LASER_BYTES_PER_SEGMENT, LASER_VERTEX_FLOATS, LASER_VERTS_PER_SEGMENT,
};
pub use minimap_renderer::{MinimapCoordinates, MinimapTextureRenderer, UiTextureRegistrar};
pub use render_item::RenderItem;
pub use render_pipeline::{RenderPass, RenderPipeline};
pub use world_anim_layout::{
    pack_world_anim_and_mark_ready, WorldAnimLayout, WorldAnimLayoutEntry, WorldAnimLayoutHonesty,
    WORLD_ANIM_LAYOUT_BYTES, WORLD_ANIM_LAYOUT_FLOATS,
};
