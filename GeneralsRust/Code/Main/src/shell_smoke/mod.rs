//! Runs a real headless skirmish session and checks its presentation/UI consumers.
mod helpers;
mod host;
mod hud;
mod imports;

mod maps;
mod presentation;
mod report;
mod result;

mod shell;
pub use report::format_shell_smoke_report;
pub use result::ShellSmokeResult;

pub fn run_shell_smoke(frames: u32) -> gamelogic::GameLogicResult<ShellSmokeResult> {
    let host = host::run_host_session(frames)?;
    let presentation =
        presentation::evaluate_presentation_residuals(&host.pres, host.presentation_ok);
    let (hud_selection_ok, selection_consumers_ok) = hud::evaluate_selection(&host);
    let shell_ui = shell::evaluate_shell_ui();
    let host_path_ok = host.host_constructed
        && host.skirmish_config_ok
        && host.menu_config_ok
        && host.frames_ok
        && host.presentation_ok
        && hud_selection_ok
        && selection_consumers_ok
        && host.dual_tick_presentation_ok
        && shell_ui.screen_skirmish_ok
        && shell_ui.control_bar_layout_ok
        && (!host.map_resolved || host.map_loaded);
    let mut out = ShellSmokeResult {
        hud_selection_ok,
        selection_consumers_ok,
        session_ok: host_path_ok,
        status: if host_path_ok { "success" } else { "partial" },
        detail: format!(
            "frames={} map_loaded={} selection={} presentation={} layout={}",
            host.frames_advanced,
            host.map_loaded,
            hud_selection_ok,
            host.presentation_ok,
            shell_ui.layout_report
        ),
        ..ShellSmokeResult::default()
    };
    out.host_constructed = host.host_constructed;
    out.skirmish_config_ok = host.skirmish_config_ok;
    out.menu_config_ok = host.menu_config_ok;
    out.map_resolved = host.map_resolved;
    out.map_loaded = host.map_loaded;
    out.frames_advanced = host.frames_advanced;
    out.frames_ok = host.frames_ok;
    out.presentation_ok = host.presentation_ok;
    out.dual_tick_presentation_ok = host.dual_tick_presentation_ok;
    out.dual_tick_counters_ok = host.dual_tick_counters_ok;
    out.gameworld_shadow_ok = host.gameworld_shadow_ok;
    out.dual_tick_policy_authority_only = host.dual_tick_policy_authority_only;
    out.minimap_fow_presentation_ok = presentation.minimap_fow_presentation_ok;
    out.laser_segment_upload_ok = presentation.laser_segment_upload_ok;
    out.projectile_segment_upload_ok = presentation.projectile_segment_upload_ok;
    out.move_line_upload_ok = presentation.move_line_upload_ok;
    out.attack_line_upload_ok = presentation.attack_line_upload_ok;
    out.multi_beam_soft_edge_ok = presentation.multi_beam_soft_edge_ok;
    out.laser_presentation_residual_ok = presentation.laser_presentation_residual_ok;
    out.floating_text_layout_ok = presentation.floating_text_layout_ok;
    out.floating_text_vanish_ok = presentation.floating_text_vanish_ok;
    out.game_text_caption_ok = presentation.game_text_caption_ok;
    out.display_string_measure_ok = presentation.display_string_measure_ok;
    out.game_text_csf_str_ok = presentation.game_text_csf_str_ok;
    out.translate_copy_residual_ok = presentation.translate_copy_residual_ok;
    out.world_anim_presentation_ok = presentation.world_anim_presentation_ok;
    out.world_anim_layout_ok = presentation.world_anim_layout_ok;
    out.world_anim_fade_ok = presentation.world_anim_fade_ok;
    out.anim2d_frame_ok = presentation.anim2d_frame_ok;
    out.anim2d_collection_residual_ok = presentation.anim2d_collection_residual_ok;
    out.rng_stream_residual_ok = presentation.rng_stream_residual_ok;
    out.screen_skirmish_ok = shell_ui.screen_skirmish_ok;
    out.control_bar_layout_ok = shell_ui.control_bar_layout_ok;
    out.control_bar_path_resolved = shell_ui.control_bar_path_resolved;
    out.control_bar_wnd_validated = shell_ui.control_bar_wnd_validated;
    out.control_bar_window_loaded = shell_ui.control_bar_window_loaded;
    out.control_bar_window_count = shell_ui.control_bar_window_count;
    Ok(out)
}

#[cfg(test)]
mod tests;
