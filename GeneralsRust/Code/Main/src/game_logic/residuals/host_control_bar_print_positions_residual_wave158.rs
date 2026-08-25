//! Wave 158 residual peels: ControlBar print-positions residual
//! (dump format/parent/script names; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 157 presentation boundary residual.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - ControlBarPrintPositions.cpp / ControlBarEasier.txt dump
//! - ControlBar.wnd:ControlBarParent + controlBarHidden.wnd
//!
//! Fail-closed:
//! - Not full WindowManager create_windows_from_script residual
//! - Not full disk write of ControlBarEasier.txt residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// ControlBar print-positions residual tables
// ---------------------------------------------------------------------------

/// Retail parent / script / output residual names.
pub const CONTROL_BAR_PRINT_PARENT_NAME_WAVE158: &str = "ControlBar.wnd:ControlBarParent";
pub const CONTROL_BAR_PRINT_HIDDEN_SCRIPT_WAVE158: &str = "controlBarHidden.wnd";
pub const CONTROL_BAR_PRINT_OUTPUT_FILE_WAVE158: &str = "ControlBarEasier.txt";

/// Dump block field residual names.
pub const CONTROL_BAR_PRINT_FIELD_NAMES_WAVE158: &[&str] =
    &["ControlBarResizer", "AltPosition", "AltSize", "END"];

/// Ordered print-positions residual navigation steps.
pub const CONTROL_BAR_PRINT_NAV_STEPS_WAVE158: &[&str] = &[
    "RESOLVE_CONTROL_BAR_PARENT",
    "LOAD_CONTROL_BAR_HIDDEN_WND",
    "WALK_WINDOW_TREE",
    "WRITE_ALT_POSITION_SIZE",
    "WRITE_CONTROL_BAR_EASIER_TXT",
    "DESTROY_TEMP_WINDOWS",
];

/// Runtime-host command residual names for print-positions peels.
pub const RUNTIME_HOST_CONTROL_BAR_PRINT_CMD_NAMES_WAVE158: &[&str] = &[
    "click_control_bar_print_positions_ok_wnd_parent",
    "click_control_bar_print_positions_ok_wnd_script",
    "click_control_bar_print_positions_ok_wnd_format",
    "click_control_bar_print_positions_ok_wnd_prepare",
    "click_control_bar_print_positions_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: parent/script/output + field names residual pack.
pub fn honesty_control_bar_print_names_residual_wave158() -> bool {
    CONTROL_BAR_PRINT_PARENT_NAME_WAVE158 == "ControlBar.wnd:ControlBarParent"
        && CONTROL_BAR_PRINT_HIDDEN_SCRIPT_WAVE158 == "controlBarHidden.wnd"
        && CONTROL_BAR_PRINT_OUTPUT_FILE_WAVE158 == "ControlBarEasier.txt"
        && CONTROL_BAR_PRINT_FIELD_NAMES_WAVE158.len() == 4
        && residual_name_index(CONTROL_BAR_PRINT_FIELD_NAMES_WAVE158, "ControlBarResizer")
            == Some(0)
        && residual_name_index(CONTROL_BAR_PRINT_FIELD_NAMES_WAVE158, "AltPosition") == Some(1)
        && residual_name_index(CONTROL_BAR_PRINT_FIELD_NAMES_WAVE158, "END") == Some(3)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_control_bar_print_nav_commands_residual_wave158() -> bool {
    CONTROL_BAR_PRINT_NAV_STEPS_WAVE158.len() == 6
        && residual_name_index(
            CONTROL_BAR_PRINT_NAV_STEPS_WAVE158,
            "RESOLVE_CONTROL_BAR_PARENT",
        ) == Some(0)
        && residual_name_index(
            CONTROL_BAR_PRINT_NAV_STEPS_WAVE158,
            "WRITE_CONTROL_BAR_EASIER_TXT",
        ) == Some(4)
        && residual_name_index(CONTROL_BAR_PRINT_NAV_STEPS_WAVE158, "DESTROY_TEMP_WINDOWS")
            == Some(5)
        && RUNTIME_HOST_CONTROL_BAR_PRINT_CMD_NAMES_WAVE158.len() == 5
        && residual_name_index(
            RUNTIME_HOST_CONTROL_BAR_PRINT_CMD_NAMES_WAVE158,
            "click_control_bar_print_positions_ok_wnd_prepare",
        ) == Some(3)
}

/// Wave 158 composite residual honesty pack.
pub fn honesty_control_bar_print_positions_residual_pack_wave158() -> bool {
    honesty_control_bar_print_names_residual_wave158()
        && honesty_control_bar_print_nav_commands_residual_wave158()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_residual() {
        assert!(honesty_control_bar_print_names_residual_wave158());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_control_bar_print_nav_commands_residual_wave158());
    }

    #[test]
    fn wave158_composite_pack() {
        assert!(honesty_control_bar_print_positions_residual_pack_wave158());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_control_bar_print_positions_prepare_sample_residual_live() {
        use game_client::gui::control_bar::{
            CONTROL_BAR_PRINT_PARENT_NAME, ResidualControlBarPrintPositionsAction,
            residual_control_bar_print_positions_last_action,
            residual_control_bar_print_positions_line_len,
            simulate_control_bar_print_positions_prepare_sample,
        };
        assert_eq!(
            CONTROL_BAR_PRINT_PARENT_NAME,
            CONTROL_BAR_PRINT_PARENT_NAME_WAVE158
        );
        assert!(
            simulate_control_bar_print_positions_prepare_sample(),
            "print-positions sample residual must latch"
        );
        assert!(residual_control_bar_print_positions_line_len() > 0);
        assert_eq!(
            residual_control_bar_print_positions_last_action(),
            ResidualControlBarPrintPositionsAction::Prepare
        );
    }
}
