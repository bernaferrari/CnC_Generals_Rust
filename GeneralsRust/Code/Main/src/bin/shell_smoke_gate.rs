use generals_main::shell_smoke::{format_shell_smoke_report, run_shell_smoke};

fn main() {
    let r = run_shell_smoke(10);
    println!("{}", format_shell_smoke_report(&r));
    // Fail-closed retail: headless smoke must never claim full W3D playability.
    // Limited host claim (shell_host_playable_ok) is required for success and is
    // independent of playable_claim — see shell_smoke module docs.
    let pass = r.status == "success"
        && !r.playable_claim
        && r.shell_host_playable_ok
        && r.control_bar_layout_ok
        && r.hud_selection_ok
        && r.selection_consumers_ok
        && r.screen_skirmish_ok
        && r.dual_tick_presentation_ok
        && r.minimap_fow_presentation_ok
        && r.laser_segment_upload_ok
        // Wave 75 residual honesty (host-testable; never flips playable_claim).
        && r.mesh_asset_residual_ok
        && r.rng_residual_pack_ok
        && r.special_power_wave72_residual_ok
        && r.special_power_wave73_residual_ok
        // Wave 76 residual honesty (host-testable; never flips playable_claim).
        && r.special_power_wave76_residual_ok
        && r.paradrop_wave76_residual_ok
        && r.control_bar_wave76_residual_ok
        && r.graphics_wave76_residual_ok
        && r.spectre_orbit_decal_presentation_ok
        // Wave 77 residual honesty (orthogonal to ControlBar/script; never flips playable_claim).
        && r.special_power_wave77_residual_ok
        && r.fow_residual_pack_ok
        && r.ground_height_presentation_ok
        && r.weapon_store_seed_residual_ok
        && r.ai_skirmish_residual_ok
        // Wave 78 residual honesty (reload table + science tiers; never flips playable_claim).
        && r.special_power_wave78_residual_ok
        && r.cluster_mines_wave78_residual_ok
        && r.gps_scrambler_wave78_residual_ok
        && r.cash_bounty_wave78_residual_ok
        // Wave 79 residual honesty (orthogonal to special powers; never flips playable_claim).
        && r.minimap_residual_pack_ok
        && r.selection_hud_residual_pack_ok
        && r.input_residual_pack_ok
        && r.drawable_residual_fields_ok
        && r.unit_training_wave79_residual_ok
        && r.upgrades_cost_time_application_ok;
    if pass {
        println!(
            "shell_smoke_gate: PASS (playable_claim={} shell_host_playable_ok={} control_bar={} cb_valid={} cb_loaded={} cb_windows={} dual_tick={} hud_sel={} sel_consumers={} minimap_fow={} laser_upload={} mesh={} sp72={} sp73={} sp76={} paradrop76={} cb76={} gfx76={} spectre_decal={} sp77={} fow77={} gh77={} weapon77={} ai77={} sp78={} cluster78={} gps78={} cash78={} minimap79={} sel79={} input79={} draw79={} train79={} upg79={} screen={} map_loaded={})",
            r.playable_claim,
            r.shell_host_playable_ok,
            r.control_bar_layout_ok,
            r.control_bar_wnd_validated,
            r.control_bar_window_loaded,
            r.control_bar_window_count,
            r.dual_tick_presentation_ok,
            r.hud_selection_ok,
            r.selection_consumers_ok,
            r.minimap_fow_presentation_ok,
            r.laser_segment_upload_ok,
            r.mesh_asset_residual_ok,
            r.special_power_wave72_residual_ok,
            r.special_power_wave73_residual_ok,
            r.special_power_wave76_residual_ok,
            r.paradrop_wave76_residual_ok,
            r.control_bar_wave76_residual_ok,
            r.graphics_wave76_residual_ok,
            r.spectre_orbit_decal_presentation_ok,
            r.special_power_wave77_residual_ok,
            r.fow_residual_pack_ok,
            r.ground_height_presentation_ok,
            r.weapon_store_seed_residual_ok,
            r.ai_skirmish_residual_ok,
            r.special_power_wave78_residual_ok,
            r.cluster_mines_wave78_residual_ok,
            r.gps_scrambler_wave78_residual_ok,
            r.cash_bounty_wave78_residual_ok,
            r.minimap_residual_pack_ok,
            r.selection_hud_residual_pack_ok,
            r.input_residual_pack_ok,
            r.drawable_residual_fields_ok,
            r.unit_training_wave79_residual_ok,
            r.upgrades_cost_time_application_ok,
            r.screen_skirmish_ok,
            r.map_loaded
        );
        std::process::exit(0);
    }
    eprintln!(
        "shell_smoke_gate: FAIL status={} playable_claim={} shell_host_playable_ok={} control_bar={} cb_loaded={} dual_tick={} sel_consumers={} laser={} {}",
        r.status,
        r.playable_claim,
        r.shell_host_playable_ok,
        r.control_bar_layout_ok,
        r.control_bar_window_loaded,
        r.dual_tick_presentation_ok,
        r.selection_consumers_ok,
        r.laser_segment_upload_ok,
        r.detail
    );
    std::process::exit(1);
}
