use generals_main::shell_smoke::{format_shell_smoke_report, run_shell_smoke};

fn main() -> gamelogic::GameLogicResult<()> {
    let _ = env_logger::try_init();
    let r = run_shell_smoke(10)?;
    println!("{}", format_shell_smoke_report(&r));
    let pass = r.status == "success"
        && r.session_ok
        && r.control_bar_layout_ok
        && r.hud_selection_ok
        && r.selection_consumers_ok
        && r.screen_skirmish_ok
        && r.dual_tick_presentation_ok
        && r.minimap_fow_presentation_ok
        && r.laser_segment_upload_ok
        && r.projectile_segment_upload_ok
        && r.move_line_upload_ok
        && r.attack_line_upload_ok
        && r.multi_beam_soft_edge_ok
        && r.floating_text_layout_ok
        && r.floating_text_vanish_ok
        && r.world_anim_layout_ok
        && r.world_anim_fade_ok
        && r.anim2d_frame_ok
        && r.game_text_caption_ok
        && r.display_string_measure_ok
        && r.game_text_csf_str_ok
        && r.rng_stream_residual_ok;
    if !pass {
        std::process::exit(1);
    }
    Ok(())
}
