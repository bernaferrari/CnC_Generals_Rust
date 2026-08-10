//! Host smoke residual assertions: core host/skirmish/frames/dual-tick/authority/HUD/claim.

use super::ShellSmokeResult;

pub(super) fn assert_core(r: &ShellSmokeResult) {
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
        r.damage_authority_env_ok,
        "damage authority should default on in shell gate: {}",
        r.detail
    );
    assert!(
        r.economy_authority_env_ok,
        "economy authority should default on in shell gate: {}",
        r.detail
    );
    assert!(
        r.dual_tick_policy_authority_only,
        "dual-tick must stay AuthorityOnly by default: {}",
        r.detail
    );
    assert!(
        r.engine_bridge_off,
        "engine OBJECT_REGISTRY bridge must stay off by default: {}",
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
        r.shell_host_playable_ok,
        "shell_host_playable_ok for successful headless host path: {}",
        r.detail
    );
    assert!(
        !r.playable_claim,
        "headless smoke must not claim retail playable"
    );
    assert_eq!(r.status, "success", "{}", r.detail);
    assert_eq!(
        r.shell_host_playable_ok,
        r.status == "success",
        "shell_host_playable_ok must track success without overclaiming playable_claim"
    );
}
