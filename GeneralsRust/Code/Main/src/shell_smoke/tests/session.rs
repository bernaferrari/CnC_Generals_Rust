use super::*;

fn assert_core(r: &ShellSmokeResult) {
    assert!(r.host_constructed, "host only after apply: {}", r.detail);
    assert!(r.skirmish_config_ok, "{}", r.detail);
    assert!(r.menu_config_ok, "{}", r.detail);
    assert!(r.frames_advanced > 0, "{}", r.detail);
    assert!(r.hud_selection_ok, "HUD selection residual: {}", r.detail);
    assert!(
        r.dual_tick_presentation_ok,
        "dual-tick presentation residual: {}",
        r.detail
    );
    assert!(
        r.dual_tick_counters_ok,
        "dual-tick residual counters: {}",
        r.detail
    );
    assert!(
        r.gameworld_shadow_ok,
        "gameworld shadow count parity: {}",
        r.detail
    );

    assert!(
        r.dual_tick_policy_authority_only,
        "dual-tick must stay AuthorityOnly by default: {}",
        r.detail
    );

    assert!(
        r.control_bar_layout_ok,
        "ControlBar.wnd ensure residual: {}",
        r.detail
    );
    assert!(
        r.selection_consumers_ok,
        "multi-consumer selection panel residual: {}",
        r.detail
    );
    // When WindowZH is present, path+validate honesty must be true; prefer
    // headless WindowManager load (not required for CI without assets).
    if r.control_bar_path_resolved {
        assert!(
            r.control_bar_wnd_validated,
            "ControlBar structural validate residual: {}",
            r.detail
        );
        #[cfg(feature = "game_client")]
        if r.control_bar_window_loaded {
            assert!(
                r.control_bar_window_count > 0,
                "WindowManager load must materialise windows: {}",
                r.detail
            );
        }
    } else {
        assert!(
            !r.control_bar_window_loaded && r.control_bar_window_count == 0,
            "missing assets must not claim window load: {}",
            r.detail
        );
    }
    assert!(
        r.screen_skirmish_ok,
        "shell→InGame screen residual: {}",
        r.detail
    );
    // Limited host claim when path is fully operational; never retail W3D claim.
    assert!(
        r.session_ok,
        "session_ok for successful headless host path: {}",
        r.detail
    );
    assert_eq!(r.status, "success", "{}", r.detail);
    assert_eq!(
        r.session_ok,
        r.status == "success",
        "session_ok must track success without overclaiming playable_claim"
    );
}

fn assert_presentation(r: &ShellSmokeResult) {
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
        r.world_anim_presentation_ok,
        "world anim presentation residual: {}",
        r.detail
    );
}

#[test]
fn skirmish_session_advances_and_updates_presentation_consumers() {
    let result = run_shell_smoke(8).unwrap();
    assert_core(&result);
    assert_presentation(&result);
}
