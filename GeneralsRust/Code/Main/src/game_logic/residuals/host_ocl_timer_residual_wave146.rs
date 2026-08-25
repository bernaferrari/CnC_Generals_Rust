//! Wave 146 residual peels: ControlBar OCL timer residual
//! (format/frames/should-update/prepare; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 145 Smudge residual.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - ControlBarOCLTimer.cpp format / frames-to-display / populate
//! - LOGICFRAMES_PER_SECOND = 30
//!
//! Fail-closed:
//! - Not full selected-object OCLUpdate residual
//! - Not full control-bar WND progress gadget residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// OCL timer residual tables
// ---------------------------------------------------------------------------

/// Logic frames per second residual (GameCommon.h).
pub const OCL_LOGICFRAMES_PER_SECOND_WAVE146: u32 = 30;

/// OCL timer helper method residual names.
pub const OCL_TIMER_METHOD_NAMES_WAVE146: &[&str] = &[
    "formatOCLTimerDisplay",
    "oclFramesToDisplay",
    "shouldUpdateTimerText",
    "populateOCLTimer",
    "updateContextOCLTimer",
];

/// Ordered OCL timer residual navigation steps.
pub const OCL_TIMER_NAV_STEPS_WAVE146: &[&str] = &[
    "SELECT_OCL_OBJECT",
    "READ_REMAINING_FRAMES",
    "FRAMES_TO_DISPLAY",
    "FORMAT_M_SS",
    "UPDATE_PROGRESS_BAR",
    "SHOULD_UPDATE_TEXT",
    "POPULATE_OCL_TIMER",
];

/// Runtime-host command residual names for OCL timer peels.
pub const RUNTIME_HOST_OCL_TIMER_CMD_NAMES_WAVE146: &[&str] = &[
    "click_ocl_timer_ok_wnd_format",
    "click_ocl_timer_ok_wnd_frames",
    "click_ocl_timer_ok_wnd_should_update",
    "click_ocl_timer_ok_wnd_prepare",
    "click_ocl_timer_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: OCL timer method names + FPS residual pack.
pub fn honesty_ocl_timer_method_names_residual_wave146() -> bool {
    OCL_LOGICFRAMES_PER_SECOND_WAVE146 == 30
        && OCL_TIMER_METHOD_NAMES_WAVE146.len() == 5
        && residual_name_index(OCL_TIMER_METHOD_NAMES_WAVE146, "formatOCLTimerDisplay") == Some(0)
        && residual_name_index(OCL_TIMER_METHOD_NAMES_WAVE146, "oclFramesToDisplay") == Some(1)
        && residual_name_index(OCL_TIMER_METHOD_NAMES_WAVE146, "populateOCLTimer") == Some(3)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_ocl_timer_nav_commands_residual_wave146() -> bool {
    OCL_TIMER_NAV_STEPS_WAVE146.len() == 7
        && residual_name_index(OCL_TIMER_NAV_STEPS_WAVE146, "FRAMES_TO_DISPLAY") == Some(2)
        && residual_name_index(OCL_TIMER_NAV_STEPS_WAVE146, "FORMAT_M_SS") == Some(3)
        && residual_name_index(OCL_TIMER_NAV_STEPS_WAVE146, "POPULATE_OCL_TIMER") == Some(6)
        && RUNTIME_HOST_OCL_TIMER_CMD_NAMES_WAVE146.len() == 5
        && residual_name_index(
            RUNTIME_HOST_OCL_TIMER_CMD_NAMES_WAVE146,
            "click_ocl_timer_ok_wnd_prepare",
        ) == Some(3)
}

/// Wave 146 composite residual honesty pack.
pub fn honesty_ocl_timer_residual_pack_wave146() -> bool {
    honesty_ocl_timer_method_names_residual_wave146()
        && honesty_ocl_timer_nav_commands_residual_wave146()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_ocl_timer_method_names_residual_wave146());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_ocl_timer_nav_commands_residual_wave146());
    }

    #[test]
    fn wave146_composite_pack() {
        assert!(honesty_ocl_timer_residual_pack_wave146());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_ocl_timer_prepare_display_residual_live() {
        use game_client::gui::control_bar::{
            ResidualOclTimerAction, residual_ocl_timer_last_action, residual_ocl_timer_seconds,
            simulate_ocl_timer_prepare_display,
        };
        // 870 remaining / 30 fps = 29 seconds (matches existing unit test).
        let Some((text, _progress, secs)) = simulate_ocl_timer_prepare_display(870, 900) else {
            panic!("prepare display residual must latch");
        };
        assert_eq!(text, "0:29");
        assert_eq!(secs, 29);
        assert_eq!(residual_ocl_timer_seconds(), 29);
        assert_eq!(
            residual_ocl_timer_last_action(),
            ResidualOclTimerAction::Prepare
        );
    }
}
