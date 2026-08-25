//! Wave 154 residual peels: WindowVideoManager residual
//! (init/pause/resume/stop/update; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 153 GameWorld authority residual.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - WindowVideoManager.cpp pauseAll/stopAll/resumeAll
//! - WindowVideoPlayType / WindowVideoState
//!
//! Fail-closed:
//! - Not full movie stream open residual
//! - Not full GameWindow attach residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// WindowVideo residual tables
// ---------------------------------------------------------------------------

/// WindowVideoPlayType residual names.
pub const WINDOW_VIDEO_PLAY_TYPE_NAMES_WAVE154: &[&str] = &["Once", "Loop", "ShowLastFrame"];

/// WindowVideoState residual names.
pub const WINDOW_VIDEO_STATE_NAMES_WAVE154: &[&str] = &["Start", "Stop", "Pause", "Play", "Hidden"];

/// WindowVideoManager method residual names.
pub const WINDOW_VIDEO_METHOD_NAMES_WAVE154: &[&str] = &[
    "init",
    "reset",
    "playMovie",
    "pauseMovie",
    "resumeMovie",
    "stopMovie",
    "pauseAllMovies",
    "resumeAllMovies",
    "stopAllMovies",
    "update",
];

/// Ordered WindowVideo residual navigation steps.
pub const WINDOW_VIDEO_NAV_STEPS_WAVE154: &[&str] = &[
    "WINDOW_VIDEO_INIT",
    "PAUSE_ALL_MOVIES",
    "RESUME_ALL_MOVIES",
    "STOP_ALL_MOVIES",
    "WINDOW_VIDEO_UPDATE",
    "CLEAR_PLAYING_MAP",
];

/// Runtime-host command residual names for WindowVideo peels.
pub const RUNTIME_HOST_WINDOW_VIDEO_CMD_NAMES_WAVE154: &[&str] = &[
    "click_window_video_ok_wnd_init",
    "click_window_video_ok_wnd_reset",
    "click_window_video_ok_wnd_pause",
    "click_window_video_ok_wnd_resume",
    "click_window_video_ok_wnd_stop",
    "click_window_video_ok_wnd_update",
    "click_window_video_ok_wnd_prepare",
    "click_window_video_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: WindowVideo play-type/state names residual pack.
pub fn honesty_window_video_type_state_names_residual_wave154() -> bool {
    WINDOW_VIDEO_PLAY_TYPE_NAMES_WAVE154.len() == 3
        && residual_name_index(WINDOW_VIDEO_PLAY_TYPE_NAMES_WAVE154, "Once") == Some(0)
        && residual_name_index(WINDOW_VIDEO_PLAY_TYPE_NAMES_WAVE154, "Loop") == Some(1)
        && residual_name_index(WINDOW_VIDEO_PLAY_TYPE_NAMES_WAVE154, "ShowLastFrame") == Some(2)
        && WINDOW_VIDEO_STATE_NAMES_WAVE154.len() == 5
        && residual_name_index(WINDOW_VIDEO_STATE_NAMES_WAVE154, "Play") == Some(3)
        && residual_name_index(WINDOW_VIDEO_STATE_NAMES_WAVE154, "Hidden") == Some(4)
}

/// Honesty: method names residual pack.
pub fn honesty_window_video_method_names_residual_wave154() -> bool {
    WINDOW_VIDEO_METHOD_NAMES_WAVE154.len() == 10
        && residual_name_index(WINDOW_VIDEO_METHOD_NAMES_WAVE154, "playMovie") == Some(2)
        && residual_name_index(WINDOW_VIDEO_METHOD_NAMES_WAVE154, "pauseAllMovies") == Some(6)
        && residual_name_index(WINDOW_VIDEO_METHOD_NAMES_WAVE154, "stopAllMovies") == Some(8)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_window_video_nav_commands_residual_wave154() -> bool {
    WINDOW_VIDEO_NAV_STEPS_WAVE154.len() == 6
        && residual_name_index(WINDOW_VIDEO_NAV_STEPS_WAVE154, "PAUSE_ALL_MOVIES") == Some(1)
        && residual_name_index(WINDOW_VIDEO_NAV_STEPS_WAVE154, "STOP_ALL_MOVIES") == Some(3)
        && residual_name_index(WINDOW_VIDEO_NAV_STEPS_WAVE154, "CLEAR_PLAYING_MAP") == Some(5)
        && RUNTIME_HOST_WINDOW_VIDEO_CMD_NAMES_WAVE154.len() == 8
        && residual_name_index(
            RUNTIME_HOST_WINDOW_VIDEO_CMD_NAMES_WAVE154,
            "click_window_video_ok_wnd_prepare",
        ) == Some(6)
}

/// Wave 154 composite residual honesty pack.
pub fn honesty_window_video_residual_pack_wave154() -> bool {
    honesty_window_video_type_state_names_residual_wave154()
        && honesty_window_video_method_names_residual_wave154()
        && honesty_window_video_nav_commands_residual_wave154()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_state_names_residual() {
        assert!(honesty_window_video_type_state_names_residual_wave154());
    }

    #[test]
    fn method_names_residual() {
        assert!(honesty_window_video_method_names_residual_wave154());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_window_video_nav_commands_residual_wave154());
    }

    #[test]
    fn wave154_composite_pack() {
        assert!(honesty_window_video_residual_pack_wave154());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_window_video_prepare_control_cycle_residual_live() {
        use game_client::gui::{
            ResidualWindowVideoAction, WINDOW_VIDEO_PLAY_TYPE_NAMES, WINDOW_VIDEO_STATE_NAMES,
            residual_window_video_last_action, residual_window_video_playing_count,
            residual_window_video_stop_all, simulate_window_video_prepare_control_cycle,
        };
        assert_eq!(
            WINDOW_VIDEO_PLAY_TYPE_NAMES,
            WINDOW_VIDEO_PLAY_TYPE_NAMES_WAVE154
        );
        assert_eq!(WINDOW_VIDEO_STATE_NAMES, WINDOW_VIDEO_STATE_NAMES_WAVE154);
        assert!(
            simulate_window_video_prepare_control_cycle(),
            "init+pause+resume+stop+update residual must latch"
        );
        assert!(residual_window_video_stop_all());
        assert_eq!(residual_window_video_playing_count(), 0);
        assert_eq!(
            residual_window_video_last_action(),
            ResidualWindowVideoAction::Update
        );
    }
}
