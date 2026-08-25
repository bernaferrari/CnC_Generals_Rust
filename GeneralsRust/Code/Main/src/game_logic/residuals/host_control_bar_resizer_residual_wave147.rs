//! Wave 147 residual peels: ControlBarResizer residual
//! (add/clear/base/resize; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 146 OCL timer residual.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - ControlBarResizer.cpp ResizerWindow / base 800x600
//!
//! Fail-closed:
//! - Not full GameWindow layout apply residual
//! - Not full controlBarHidden.wnd dump residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// ControlBarResizer residual tables
// ---------------------------------------------------------------------------

/// Retail default base resolution residual.
pub const CONTROL_BAR_RESIZER_DEFAULT_BASE_WAVE147: (u32, u32) = (800, 600);

/// ControlBarResizer method residual names.
pub const CONTROL_BAR_RESIZER_METHOD_NAMES_WAVE147: &[&str] = &[
    "addWindow",
    "clear",
    "setBaseResolution",
    "resize",
    "getOptimalSize",
];

/// Ordered ControlBarResizer residual navigation steps.
pub const CONTROL_BAR_RESIZER_NAV_STEPS_WAVE147: &[&str] = &[
    "CLEAR_RESIZER_WINDOWS",
    "SET_BASE_800x600",
    "ADD_CONTROL_BAR_PARENT",
    "RESIZE_TO_DISPLAY",
    "GET_OPTIMAL_SIZE",
    "APPLY_SCALE_TO_WINDOWS",
];

/// Runtime-host command residual names for ControlBarResizer peels.
pub const RUNTIME_HOST_CONTROL_BAR_RESIZER_CMD_NAMES_WAVE147: &[&str] = &[
    "click_control_bar_resizer_ok_wnd_add",
    "click_control_bar_resizer_ok_wnd_clear",
    "click_control_bar_resizer_ok_wnd_base",
    "click_control_bar_resizer_ok_wnd_resize",
    "click_control_bar_resizer_ok_wnd_optimal",
    "click_control_bar_resizer_ok_wnd_prepare",
    "click_control_bar_resizer_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: ControlBarResizer method names + default base residual pack.
pub fn honesty_control_bar_resizer_method_names_residual_wave147() -> bool {
    CONTROL_BAR_RESIZER_DEFAULT_BASE_WAVE147 == (800, 600)
        && CONTROL_BAR_RESIZER_METHOD_NAMES_WAVE147.len() == 5
        && residual_name_index(CONTROL_BAR_RESIZER_METHOD_NAMES_WAVE147, "addWindow") == Some(0)
        && residual_name_index(
            CONTROL_BAR_RESIZER_METHOD_NAMES_WAVE147,
            "setBaseResolution",
        ) == Some(2)
        && residual_name_index(CONTROL_BAR_RESIZER_METHOD_NAMES_WAVE147, "getOptimalSize")
            == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_control_bar_resizer_nav_commands_residual_wave147() -> bool {
    CONTROL_BAR_RESIZER_NAV_STEPS_WAVE147.len() == 6
        && residual_name_index(CONTROL_BAR_RESIZER_NAV_STEPS_WAVE147, "SET_BASE_800x600") == Some(1)
        && residual_name_index(
            CONTROL_BAR_RESIZER_NAV_STEPS_WAVE147,
            "ADD_CONTROL_BAR_PARENT",
        ) == Some(2)
        && residual_name_index(
            CONTROL_BAR_RESIZER_NAV_STEPS_WAVE147,
            "APPLY_SCALE_TO_WINDOWS",
        ) == Some(5)
        && RUNTIME_HOST_CONTROL_BAR_RESIZER_CMD_NAMES_WAVE147.len() == 7
        && residual_name_index(
            RUNTIME_HOST_CONTROL_BAR_RESIZER_CMD_NAMES_WAVE147,
            "click_control_bar_resizer_ok_wnd_prepare",
        ) == Some(5)
}

/// Wave 147 composite residual honesty pack.
pub fn honesty_control_bar_resizer_residual_pack_wave147() -> bool {
    honesty_control_bar_resizer_method_names_residual_wave147()
        && honesty_control_bar_resizer_nav_commands_residual_wave147()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_control_bar_resizer_method_names_residual_wave147());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_control_bar_resizer_nav_commands_residual_wave147());
    }

    #[test]
    fn wave147_composite_pack() {
        assert!(honesty_control_bar_resizer_residual_pack_wave147());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_control_bar_resizer_prepare_default_residual_live() {
        use game_client::gui::control_bar::{
            ResidualControlBarResizerAction, residual_control_bar_resizer_base_resolution,
            residual_control_bar_resizer_last_action, residual_control_bar_resizer_window_count,
            simulate_control_bar_resizer_prepare_default,
        };
        assert!(
            simulate_control_bar_resizer_prepare_default(),
            "clear+base+add+resize residual must latch"
        );
        assert_eq!(residual_control_bar_resizer_window_count(), 1);
        assert_eq!(residual_control_bar_resizer_base_resolution(), (800, 600));
        assert_eq!(
            residual_control_bar_resizer_last_action(),
            ResidualControlBarResizerAction::Prepare
        );
    }
}
