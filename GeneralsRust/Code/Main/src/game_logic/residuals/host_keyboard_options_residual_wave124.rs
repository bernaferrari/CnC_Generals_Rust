//! Wave 124 residual peels: KeyboardOptionsMenu WND residual
//! (category/command/Assign/Reset/Back; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 123 QuitMenu Options button, Wave 116 Options prefs.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - KeyboardOptionsMenu.cpp / KeyboardOptionsMenu.wnd
//! - ComboBoxCategoryList, ListBoxCommandList
//! - ButtonAssign, ButtonResetAll, ButtonBack
//!
//! Fail-closed:
//! - Not full hotkey rebind persistence residual
//! - Not full command-map rewrite residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// Keyboard options residual tables
// ---------------------------------------------------------------------------

/// Retail KeyboardOptionsMenu layout filename residual.
pub const KEYBOARD_OPTIONS_LAYOUT_FILENAME_WAVE124: &str = "Menus/KeyboardOptionsMenu.wnd";

/// Retail KeyboardOptionsMenu control names residual.
pub const KEYBOARD_OPTIONS_CONTROL_NAMES_WAVE124: &[&str] = &[
    "KeyboardOptionsMenu.wnd:ParentKeyboardOptionsMenu",
    "KeyboardOptionsMenu.wnd:ButtonBack",
    "KeyboardOptionsMenu.wnd:ComboBoxCategoryList",
    "KeyboardOptionsMenu.wnd:ListBoxCommandList",
    "KeyboardOptionsMenu.wnd:StaticTextDescription",
    "KeyboardOptionsMenu.wnd:StaticTextCurrentHotkey",
    "KeyboardOptionsMenu.wnd:ButtonResetAll",
    "KeyboardOptionsMenu.wnd:TextEntryAssignHotkey",
    "KeyboardOptionsMenu.wnd:ButtonAssign",
];

/// Ordered KeyboardOptions residual navigation steps.
pub const KEYBOARD_OPTIONS_NAV_STEPS_WAVE124: &[&str] = &[
    "PUSH_KEYBOARD_OPTIONS_LAYOUT",
    "POPULATE_CATEGORY_COMBO",
    "GBM_VALUE_CHANGED_CATEGORY",
    "POPULATE_COMMAND_LIST",
    "GBM_VALUE_CHANGED_COMMAND",
    "TEXT_ENTRY_ASSIGN_HOTKEY",
    "GBM_SELECTED_BUTTON_ASSIGN",
    "GBM_SELECTED_BUTTON_RESET_ALL",
    "GBM_SELECTED_BUTTON_BACK",
    "SHELL_POP",
];

/// Runtime-host command residual names for KeyboardOptions peels.
pub const RUNTIME_HOST_KEYBOARD_OPTIONS_CMD_NAMES_WAVE124: &[&str] = &[
    "open_keyboard_options_ok_wnd",
    "open_keyboard_options_ok",
    "click_keyboard_options_ok_wnd_category",
    "click_keyboard_options_ok_wnd_command",
    "click_keyboard_options_ok_wnd_assign",
    "click_keyboard_options_ok_wnd_reset",
    "click_keyboard_options_ok_wnd_back",
    "click_keyboard_options_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: KeyboardOptions control names residual pack.
pub fn honesty_keyboard_options_control_names_residual_wave124() -> bool {
    KEYBOARD_OPTIONS_LAYOUT_FILENAME_WAVE124 == "Menus/KeyboardOptionsMenu.wnd"
        && KEYBOARD_OPTIONS_CONTROL_NAMES_WAVE124.len() == 9
        && residual_name_index(
            KEYBOARD_OPTIONS_CONTROL_NAMES_WAVE124,
            "KeyboardOptionsMenu.wnd:ButtonBack",
        ) == Some(1)
        && residual_name_index(
            KEYBOARD_OPTIONS_CONTROL_NAMES_WAVE124,
            "KeyboardOptionsMenu.wnd:ButtonAssign",
        ) == Some(8)
        && residual_name_index(
            KEYBOARD_OPTIONS_CONTROL_NAMES_WAVE124,
            "KeyboardOptionsMenu.wnd:ListBoxCommandList",
        ) == Some(3)
        && KEYBOARD_OPTIONS_CONTROL_NAMES_WAVE124
            .iter()
            .all(|n| n.starts_with("KeyboardOptionsMenu.wnd:"))
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_keyboard_options_nav_commands_residual_wave124() -> bool {
    KEYBOARD_OPTIONS_NAV_STEPS_WAVE124.len() == 10
        && residual_name_index(
            KEYBOARD_OPTIONS_NAV_STEPS_WAVE124,
            "GBM_VALUE_CHANGED_CATEGORY",
        ) == Some(2)
        && residual_name_index(
            KEYBOARD_OPTIONS_NAV_STEPS_WAVE124,
            "GBM_SELECTED_BUTTON_ASSIGN",
        ) == Some(6)
        && residual_name_index(KEYBOARD_OPTIONS_NAV_STEPS_WAVE124, "SHELL_POP") == Some(9)
        && RUNTIME_HOST_KEYBOARD_OPTIONS_CMD_NAMES_WAVE124.len() == 8
        && residual_name_index(
            RUNTIME_HOST_KEYBOARD_OPTIONS_CMD_NAMES_WAVE124,
            "open_keyboard_options_ok_wnd",
        ) == Some(0)
        && residual_name_index(
            RUNTIME_HOST_KEYBOARD_OPTIONS_CMD_NAMES_WAVE124,
            "click_keyboard_options_ok_wnd_assign",
        ) == Some(4)
}

/// Wave 124 composite residual honesty pack.
pub fn honesty_keyboard_options_residual_pack_wave124() -> bool {
    honesty_keyboard_options_control_names_residual_wave124()
        && honesty_keyboard_options_nav_commands_residual_wave124()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_names_residual() {
        assert!(honesty_keyboard_options_control_names_residual_wave124());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_keyboard_options_nav_commands_residual_wave124());
    }

    #[test]
    fn wave124_composite_pack() {
        assert!(honesty_keyboard_options_residual_pack_wave124());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_keyboard_prepare_assign_residual_live() {
        use game_client::gui::callbacks::{
            ResidualKeyboardOptionsAction, residual_keyboard_options_category_index,
            residual_keyboard_options_command_index, residual_keyboard_options_last_action,
            simulate_keyboard_options_back_button_gadget_selected,
            simulate_keyboard_options_prepare_assign,
        };
        assert!(
            simulate_keyboard_options_prepare_assign(1, 2),
            "category+command+assign residual must latch"
        );
        assert_eq!(residual_keyboard_options_category_index(), 1);
        assert_eq!(residual_keyboard_options_command_index(), Some(2));
        assert_eq!(
            residual_keyboard_options_last_action(),
            ResidualKeyboardOptionsAction::Assign
        );
        assert!(simulate_keyboard_options_back_button_gadget_selected());
        assert_eq!(
            residual_keyboard_options_last_action(),
            ResidualKeyboardOptionsAction::Back
        );
    }
}
