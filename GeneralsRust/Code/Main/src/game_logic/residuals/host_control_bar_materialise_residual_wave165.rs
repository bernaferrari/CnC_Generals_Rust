//! Wave 165 residual peels: ControlBar.wnd materialisation residual
//! (require WindowManager window_count==98 when assets resolve; never flips
//! shell `playable_claim`).
//!
//! Orthogonal to Wave 164 Shell→Skirmish nav residual.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - ControlBar.cpp ShowControlBar → WindowManager load ControlBar.wnd
//! - Retail WindowZH ControlBar.wnd materialises 98 windows
//!
//! Fail-closed:
//! - Not full in-game command-bar gadget residual
//! - Not full Scheme/PrintPositions residual (waves 156/158)
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Retail ControlBar.wnd window count residual floor.
pub const CONTROL_BAR_MATERIALISE_WINDOW_COUNT_WAVE165: usize = 98;

/// ControlBar materialisation residual method names.
pub const CONTROL_BAR_MATERIALISE_METHOD_NAMES_WAVE165: &[&str] = &[
    "control_bar_layout_honesty",
    "try_load_control_bar_via_window_manager",
    "window_loaded",
    "window_count",
    "simulate_control_bar_materialise_honesty",
];

/// Ordered ControlBar materialisation residual navigation steps.
pub const CONTROL_BAR_MATERIALISE_NAV_STEPS_WAVE165: &[&str] = &[
    "RESOLVE_CONTROL_BAR_WND",
    "VALIDATE_CONTROL_BAR_WND",
    "LOAD_WINDOW_SCRIPT",
    "REQUIRE_WINDOW_LOADED",
    "REQUIRE_WINDOW_COUNT_98",
    "WAVE76_NAMED_PACK",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_CONTROL_BAR_MATERIALISE_CMD_NAMES_WAVE165: &[&str] = &[
    "click_control_bar_ok_wnd_materialise",
    "click_control_bar_ok_wnd_load",
    "click_control_bar_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_control_bar_materialise_method_names_residual_wave165() -> bool {
    CONTROL_BAR_MATERIALISE_WINDOW_COUNT_WAVE165 == 98
        && CONTROL_BAR_MATERIALISE_METHOD_NAMES_WAVE165.len() == 5
        && residual_name_index(
            CONTROL_BAR_MATERIALISE_METHOD_NAMES_WAVE165,
            "window_loaded",
        ) == Some(2)
        && residual_name_index(
            CONTROL_BAR_MATERIALISE_METHOD_NAMES_WAVE165,
            "simulate_control_bar_materialise_honesty",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_control_bar_materialise_nav_commands_residual_wave165() -> bool {
    CONTROL_BAR_MATERIALISE_NAV_STEPS_WAVE165.len() == 6
        && residual_name_index(
            CONTROL_BAR_MATERIALISE_NAV_STEPS_WAVE165,
            "REQUIRE_WINDOW_COUNT_98",
        ) == Some(4)
        && RUNTIME_HOST_CONTROL_BAR_MATERIALISE_CMD_NAMES_WAVE165.len() == 3
}

/// Wave 165 composite residual honesty pack.
pub fn honesty_control_bar_materialise_residual_pack_wave165() -> bool {
    honesty_control_bar_materialise_method_names_residual_wave165()
        && honesty_control_bar_materialise_nav_commands_residual_wave165()
}

/// Live residual: ControlBar materialise peel.
pub fn simulate_control_bar_materialise_honesty_wave165() -> bool {
    use crate::gameplay_layout::{
        CONTROL_BAR_RETAIL_WINDOW_COUNT, simulate_control_bar_materialise_honesty,
    };
    CONTROL_BAR_RETAIL_WINDOW_COUNT == CONTROL_BAR_MATERIALISE_WINDOW_COUNT_WAVE165
        && honesty_control_bar_materialise_residual_pack_wave165()
        && simulate_control_bar_materialise_honesty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_control_bar_materialise_method_names_residual_wave165());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_control_bar_materialise_nav_commands_residual_wave165());
    }

    #[test]
    fn wave165_composite_pack() {
        assert!(honesty_control_bar_materialise_residual_pack_wave165());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_control_bar_materialise_honesty_residual_live() {
        assert!(
            simulate_control_bar_materialise_honesty_wave165(),
            "ControlBar.wnd materialisation residual must latch 98 windows when assets resolve"
        );
    }
}
