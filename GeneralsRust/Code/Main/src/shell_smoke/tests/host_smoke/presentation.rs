//! Host smoke residual assertions: laser/projectile/minimap/floating text/anim2d/rng.

use super::ShellSmokeResult;

pub(super) fn assert_presentation(r: &ShellSmokeResult) {
    assert!(
        r.minimap_fow_presentation_ok,
        "minimap FOW presentation residual: {}",
        r.detail
    );
    assert!(
        r.laser_segment_upload_ok,
        "laser segment CPU upload residual: {}",
        r.detail
    );
    assert!(
        r.projectile_segment_upload_ok,
        "projectile segment CPU upload residual: {}",
        r.detail
    );
    assert!(
        r.move_line_upload_ok,
        "move line CPU upload residual: {}",
        r.detail
    );
    assert!(
        r.attack_line_upload_ok,
        "attack line CPU upload residual: {}",
        r.detail
    );
    assert!(
        r.multi_beam_soft_edge_ok,
        "multi-beam soft-edge residual: {}",
        r.detail
    );
    assert!(
        r.laser_presentation_residual_ok,
        "laser presentation residual: {}",
        r.detail
    );
    assert!(
        r.floating_text_layout_ok,
        "floating text CPU layout residual: {}",
        r.detail
    );
    assert!(
        r.floating_text_vanish_ok,
        "floating text vanish-rate residual: {}",
        r.detail
    );
    assert!(
        r.game_text_caption_ok,
        "GUI:AddCash caption residual: {}",
        r.detail
    );
    assert!(
        r.game_text_csf_str_ok,
        "CSF/STR GameText residual: {}",
        r.detail
    );
    assert!(
        r.display_string_measure_ok,
        "DisplayString measure residual: {}",
        r.detail
    );
    assert!(
        r.translate_copy_residual_ok,
        "translate_copy residual: {}",
        r.detail
    );
    assert!(
        r.world_anim_layout_ok,
        "world anim CPU layout residual: {}",
        r.detail
    );
    assert!(
        r.world_anim_fade_ok,
        "world anim fade residual: {}",
        r.detail
    );
    assert!(
        r.anim2d_frame_ok,
        "Anim2D frame advance residual: {}",
        r.detail
    );
    assert!(
        r.anim2d_collection_residual_ok,
        "Anim2DCollection residual: {}",
        r.detail
    );
    assert!(
        r.rng_stream_residual_ok,
        "RNG stream residual: {}",
        r.detail
    );
    assert!(
        r.mesh_asset_residual_ok,
        "mesh asset residual: {}",
        r.detail
    );
    assert!(
        r.rng_residual_pack_ok,
        "RNG residual pack wave72: {}",
        r.detail
    );

    assert!(
        r.world_anim_presentation_ok,
        "world anim presentation residual: {}",
        r.detail
    );
}
