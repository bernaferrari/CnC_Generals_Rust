// Fill ShellSmokeResult fields from fields_presentation.rs (no include! in struct-literal position).
#[rustfmt::skip]
fn fill_presentation(
    out: &mut ShellSmokeResult,
    presentation: &super::presentation::PresentationResiduals,
) {
    out.minimap_fow_presentation_ok = presentation.minimap_fow_presentation_ok;
    out.laser_segment_upload_ok = presentation.laser_segment_upload_ok;
    out.projectile_segment_upload_ok = presentation.projectile_segment_upload_ok;
    out.move_line_upload_ok = presentation.move_line_upload_ok;
    out.attack_line_upload_ok = presentation.attack_line_upload_ok;
    out.multi_beam_soft_edge_ok = presentation.multi_beam_soft_edge_ok;
    out.laser_presentation_residual_ok = presentation.laser_presentation_residual_ok;
    out.floating_text_layout_ok = presentation.floating_text_layout_ok;
    out.floating_text_vanish_ok = presentation.floating_text_vanish_ok;
    out.world_anim_presentation_ok = presentation.world_anim_presentation_ok;
    out.world_anim_layout_ok = presentation.world_anim_layout_ok;
    out.world_anim_fade_ok = presentation.world_anim_fade_ok;
    out.anim2d_frame_ok = presentation.anim2d_frame_ok;
    out.anim2d_collection_residual_ok = presentation.anim2d_collection_residual_ok;
    out.translate_copy_residual_ok = presentation.translate_copy_residual_ok;
    out.game_text_caption_ok = presentation.game_text_caption_ok;
    out.game_text_csf_str_ok = presentation.game_text_csf_str_ok;
    out.display_string_measure_ok = presentation.display_string_measure_ok;
    out.rng_stream_residual_ok = presentation.rng_stream_residual_ok;
}
