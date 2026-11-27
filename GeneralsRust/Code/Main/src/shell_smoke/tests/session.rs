use super::*;

fn assert_core(r: &ShellSmokeResult) {
    assert!(r.host_constructed, "host only after apply: {}", r.detail);
    assert!(r.skirmish_config_ok, "{}", r.detail);
    assert!(r.menu_config_ok, "{}", r.detail);
    assert!(r.frames_ok, "{}", r.detail);
    assert!(r.hud_selection_ok, "HUD selection residual: {}", r.detail);
    assert!(r.presentation_ok, "published simulation frame: {}", r.detail);

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

#[test]
fn skirmish_session_advances_and_updates_presentation_consumers() {
    let result = run_shell_smoke(8).unwrap();
    assert_eq!(result.frames_advanced, 8, "{}", result.detail);
    assert_core(&result);
}
