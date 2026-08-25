/*
** Command & Conquer Generals Zero Hour(tm) - C++ SAGE Engine Equivalent Modules
** Copyright 2025 Electronic Arts Inc.
**
** Module declarations for C++ SAGE engine equivalent structures
*/

pub mod attack_line_upload;
pub mod floating_text_layout;
pub mod fow_uniform_integration;
pub mod game_text_residual;
pub mod granny_honesty;
pub mod graphics_system;
pub mod laser_draw;
pub mod laser_segment_upload;
pub mod move_line_upload;
pub use attack_line_upload::AttackLineUpload;
pub mod particle_system_upload;
pub mod projected_shroud_upload;
pub mod projectile_segment_upload;
pub use move_line_upload::MoveLineUpload;
pub use particle_system_upload::{
    ParticleSystemUpload, ParticleSystemUploadHonesty,
    pack_from_presentation as pack_particle_systems_from_presentation,
};
pub use projected_shroud_upload::{
    PROJECTED_SHROUD_SAMPLER_POLICY, PROJECTED_SHROUD_TEXTURE_FORMAT, ProjectedShroudGpuUploader,
    ProjectedShroudSamplerPolicy, ProjectedShroudTextureState, ProjectedShroudUploadAction,
    ProjectedShroudUploadPlan,
};
pub use projectile_segment_upload::ProjectileSegmentUpload;
pub mod minimap_renderer;
pub mod occlusion_bridge;

pub mod render_item;
pub mod render_pipeline;
pub use render_pipeline::{
    ResidualPresentationBoundaryAction, residual_presentation_boundary_last_action,
    residual_presentation_boundary_ok, simulate_presentation_boundary_cnc_execute_source,
    simulate_presentation_boundary_collect_source, simulate_presentation_boundary_execute_source,
    simulate_presentation_boundary_fallback_counter_source,
    simulate_presentation_boundary_prepare_honesty,
};
pub mod selection_renderer;
pub mod ui_render_pass;
pub mod world_anim_layout;
pub use floating_text_layout::{
    FLOATING_TEXT_FONT_NAME, FLOATING_TEXT_FONT_POINT_SIZE, FLOATING_TEXT_LAYOUT_BYTES,
    FLOATING_TEXT_LAYOUT_FLOATS, FloatingTextLayout, FloatingTextLayoutEntry,
    FloatingTextLayoutHonesty, GUI_ADD_CASH_KEY, INGAME_UI_FONT_RESIDUAL_TABLE,
    honesty_display_string_vanish_color_alpha_residual_ok,
    honesty_graphics_residual_pack_wave76_ok, honesty_ingame_ui_font_table_residual_ok,
    pack_floating_text_and_mark_ready, resolve_add_cash_caption,
};
pub use game_text_residual::{
    GUI_ADD_CASH_RETAIL_TEMPLATE, GameTextResidualExercise, GameTextResidualHonesty,
    exercise_host_game_text_residual, format_printf_d, measure_display_string_residual,
};
pub use graphics_system::{GlobalUniforms, GraphicsStatistics, GraphicsSystem};
pub use laser_segment_upload::{
    LASER_BYTES_PER_SEGMENT, LASER_VERTEX_FLOATS, LASER_VERTS_PER_SEGMENT, LaserSegmentUpload,
    LaserSegmentUploadHonesty, LaserSegmentVertex, pack_and_mark_upload_ready,
};
pub use minimap_renderer::{MinimapCoordinates, MinimapTextureRenderer, UiTextureRegistrar};
pub use render_item::RenderItem;
pub use render_pipeline::{RenderPass, RenderPipeline};
pub use world_anim_layout::{
    WORLD_ANIM_LAYOUT_BYTES, WORLD_ANIM_LAYOUT_FLOATS, WorldAnimLayout, WorldAnimLayoutEntry,
    WorldAnimLayoutHonesty, pack_world_anim_and_mark_ready,
};
