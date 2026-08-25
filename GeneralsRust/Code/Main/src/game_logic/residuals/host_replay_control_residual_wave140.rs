//! Wave 140 residual peels: ReplayControls residual
//! (play/pause/stop/ff/seek; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 122 ReplayMenu, Wave 130 PopupReplay.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - ReplayControls.cpp
//! - Logical ButtonPlay/Pause/Stop/FastForward
//!
//! Fail-closed:
//! - Not full recorder frame stream residual
//! - Not full slider gadget residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// Replay control residual tables
// ---------------------------------------------------------------------------

/// Retail ReplayControls layout filename residual.
pub const REPLAY_CONTROL_LAYOUT_FILENAME_WAVE140: &str = "ReplayControls.wnd";

/// Logical ReplayControls button names residual.
pub const REPLAY_CONTROL_CONTROL_NAMES_WAVE140: &[&str] = &[
    "ReplayControls.wnd:ButtonPlay",
    "ReplayControls.wnd:ButtonPause",
    "ReplayControls.wnd:ButtonStop",
    "ReplayControls.wnd:ButtonFastForward",
];

/// Ordered ReplayControl residual navigation steps.
pub const REPLAY_CONTROL_NAV_STEPS_WAVE140: &[&str] = &[
    "SHOW_REPLAY_CONTROLS",
    "GBM_SELECTED_BUTTON_PLAY",
    "GBM_SELECTED_BUTTON_PAUSE",
    "GBM_SELECTED_BUTTON_FAST_FORWARD",
    "SLIDER_SEEK",
    "GBM_SELECTED_BUTTON_STOP",
];

/// Runtime-host command residual names for ReplayControl peels.
pub const RUNTIME_HOST_REPLAY_CONTROL_CMD_NAMES_WAVE140: &[&str] = &[
    "click_replay_control_ok_wnd_play",
    "click_replay_control_ok_wnd_pause",
    "click_replay_control_ok_wnd_stop",
    "click_replay_control_ok_wnd_ff",
    "click_replay_control_ok_wnd_seek",
    "click_replay_control_ok_wnd_prepare",
    "click_replay_control_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: ReplayControl control names residual pack.
pub fn honesty_replay_control_control_names_residual_wave140() -> bool {
    REPLAY_CONTROL_LAYOUT_FILENAME_WAVE140 == "ReplayControls.wnd"
        && REPLAY_CONTROL_CONTROL_NAMES_WAVE140.len() == 4
        && residual_name_index(
            REPLAY_CONTROL_CONTROL_NAMES_WAVE140,
            "ReplayControls.wnd:ButtonPlay",
        ) == Some(0)
        && residual_name_index(
            REPLAY_CONTROL_CONTROL_NAMES_WAVE140,
            "ReplayControls.wnd:ButtonStop",
        ) == Some(2)
        && REPLAY_CONTROL_CONTROL_NAMES_WAVE140
            .iter()
            .all(|n| n.starts_with("ReplayControls.wnd:"))
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_replay_control_nav_commands_residual_wave140() -> bool {
    REPLAY_CONTROL_NAV_STEPS_WAVE140.len() == 6
        && residual_name_index(REPLAY_CONTROL_NAV_STEPS_WAVE140, "GBM_SELECTED_BUTTON_PLAY")
            == Some(1)
        && residual_name_index(REPLAY_CONTROL_NAV_STEPS_WAVE140, "SLIDER_SEEK") == Some(4)
        && residual_name_index(REPLAY_CONTROL_NAV_STEPS_WAVE140, "GBM_SELECTED_BUTTON_STOP")
            == Some(5)
        && RUNTIME_HOST_REPLAY_CONTROL_CMD_NAMES_WAVE140.len() == 7
        && residual_name_index(
            RUNTIME_HOST_REPLAY_CONTROL_CMD_NAMES_WAVE140,
            "click_replay_control_ok_wnd_prepare",
        ) == Some(5)
}

/// Wave 140 composite residual honesty pack.
pub fn honesty_replay_control_residual_pack_wave140() -> bool {
    honesty_replay_control_control_names_residual_wave140()
        && honesty_replay_control_nav_commands_residual_wave140()
}

/// Live C++ `initControls` hide bit (ReplayControl.wnd:ParentReplayControl).
pub fn host_replay_control_window_hidden() -> bool {
    crate::save_load::host_replay_controls_hidden()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_names_residual() {
        assert!(honesty_replay_control_control_names_residual_wave140());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_replay_control_nav_commands_residual_wave140());
    }

    #[test]
    fn wave140_composite_pack() {
        assert!(honesty_replay_control_residual_pack_wave140());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_replay_control_prepare_play_residual_live() {
        use game_client::gui::callbacks::{
            ResidualReplayControlAction, residual_replay_control_last_action,
            residual_replay_control_position, simulate_replay_control_pause,
            simulate_replay_control_prepare_play_at, simulate_replay_control_stop,
        };
        assert!(
            simulate_replay_control_prepare_play_at(0.25),
            "seek+play residual must latch"
        );
        assert!((residual_replay_control_position() - 0.25).abs() < 0.0002);
        assert_eq!(
            residual_replay_control_last_action(),
            ResidualReplayControlAction::Play
        );
        assert!(simulate_replay_control_pause());
        assert_eq!(
            residual_replay_control_last_action(),
            ResidualReplayControlAction::Pause
        );
        assert!(simulate_replay_control_stop());
        assert_eq!(
            residual_replay_control_last_action(),
            ResidualReplayControlAction::Stop
        );
        assert_eq!(residual_replay_control_position(), 0.0);
    }
}
