//! Wave 128 residual peels: MessageBox WND residual
//! (Ok/Yes/No/Cancel + show kinds; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 123 QuitMenu confirm, Wave 125 ScoreScreen.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - MessageBox.cpp / MessageBox.wnd / QuitMessageBox.wnd
//! - ButtonOk, ButtonYes, ButtonNo, ButtonCancel
//!
//! Fail-closed:
//! - Not full layout create / callback invoke residual
//! - Not full extended/timeout message box residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// Message box residual tables
// ---------------------------------------------------------------------------

/// Retail MessageBox layout filename residual.
pub const MESSAGE_BOX_LAYOUT_FILENAME_WAVE128: &str = "Menus/MessageBox.wnd";

/// Retail QuitMessageBox layout filename residual (logo variant).
pub const QUIT_MESSAGE_BOX_LAYOUT_FILENAME_WAVE128: &str = "Menus/QuitMessageBox.wnd";

/// Retail MessageBox control names residual.
pub const MESSAGE_BOX_CONTROL_NAMES_WAVE128: &[&str] = &[
    "MessageBox.wnd:ButtonOk",
    "MessageBox.wnd:ButtonYes",
    "MessageBox.wnd:ButtonNo",
    "MessageBox.wnd:ButtonCancel",
    "MessageBox.wnd:StaticTextTitle",
    "MessageBox.wnd:StaticTextMessage",
];

/// Ordered MessageBox residual navigation steps.
pub const MESSAGE_BOX_NAV_STEPS_WAVE128: &[&str] = &[
    "CREATE_MESSAGE_BOX_LAYOUT",
    "SET_TITLE_BODY",
    "SHOW_BUTTON_FLAGS",
    "GBM_SELECTED_BUTTON_YES",
    "GBM_SELECTED_BUTTON_NO",
    "GBM_SELECTED_BUTTON_OK",
    "GBM_SELECTED_BUTTON_CANCEL",
    "INVOKE_CALLBACK",
    "DESTROY_MESSAGE_BOX",
];

/// Runtime-host command residual names for MessageBox peels.
pub const RUNTIME_HOST_MESSAGE_BOX_CMD_NAMES_WAVE128: &[&str] = &[
    "show_message_box_ok_wnd_yes_no",
    "show_message_box_ok_wnd_ok",
    "show_message_box_ok_wnd_ok_cancel",
    "show_message_box_miss",
    "click_message_box_ok_wnd_yes",
    "click_message_box_ok_wnd_no",
    "click_message_box_ok_wnd_ok",
    "click_message_box_ok_wnd_cancel",
    "click_message_box_ok_wnd_prepare_yes",
    "click_message_box_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: MessageBox control names residual pack.
pub fn honesty_message_box_control_names_residual_wave128() -> bool {
    MESSAGE_BOX_LAYOUT_FILENAME_WAVE128 == "Menus/MessageBox.wnd"
        && QUIT_MESSAGE_BOX_LAYOUT_FILENAME_WAVE128 == "Menus/QuitMessageBox.wnd"
        && MESSAGE_BOX_CONTROL_NAMES_WAVE128.len() == 6
        && residual_name_index(
            MESSAGE_BOX_CONTROL_NAMES_WAVE128,
            "MessageBox.wnd:ButtonYes",
        ) == Some(1)
        && residual_name_index(MESSAGE_BOX_CONTROL_NAMES_WAVE128, "MessageBox.wnd:ButtonOk")
            == Some(0)
        && residual_name_index(
            MESSAGE_BOX_CONTROL_NAMES_WAVE128,
            "MessageBox.wnd:ButtonCancel",
        ) == Some(3)
        && MESSAGE_BOX_CONTROL_NAMES_WAVE128
            .iter()
            .all(|n| n.starts_with("MessageBox.wnd:"))
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_message_box_nav_commands_residual_wave128() -> bool {
    MESSAGE_BOX_NAV_STEPS_WAVE128.len() == 9
        && residual_name_index(MESSAGE_BOX_NAV_STEPS_WAVE128, "SHOW_BUTTON_FLAGS") == Some(2)
        && residual_name_index(MESSAGE_BOX_NAV_STEPS_WAVE128, "GBM_SELECTED_BUTTON_YES") == Some(3)
        && residual_name_index(MESSAGE_BOX_NAV_STEPS_WAVE128, "DESTROY_MESSAGE_BOX") == Some(8)
        && RUNTIME_HOST_MESSAGE_BOX_CMD_NAMES_WAVE128.len() == 10
        && residual_name_index(
            RUNTIME_HOST_MESSAGE_BOX_CMD_NAMES_WAVE128,
            "show_message_box_ok_wnd_yes_no",
        ) == Some(0)
        && residual_name_index(
            RUNTIME_HOST_MESSAGE_BOX_CMD_NAMES_WAVE128,
            "click_message_box_ok_wnd_prepare_yes",
        ) == Some(8)
}

/// Wave 128 composite residual honesty pack.
pub fn honesty_message_box_residual_pack_wave128() -> bool {
    honesty_message_box_control_names_residual_wave128()
        && honesty_message_box_nav_commands_residual_wave128()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_names_residual() {
        assert!(honesty_message_box_control_names_residual_wave128());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_message_box_nav_commands_residual_wave128());
    }

    #[test]
    fn wave128_composite_pack() {
        assert!(honesty_message_box_residual_pack_wave128());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_message_box_prepare_yes_residual_live() {
        use game_client::gui::callbacks::{
            ResidualMessageBoxAction, residual_message_box_is_visible,
            residual_message_box_last_action, residual_message_box_type_ordinal,
            simulate_message_box_prepare_yes, simulate_message_box_show_yes_no,
        };
        assert!(simulate_message_box_show_yes_no("T", "B"));
        assert!(residual_message_box_is_visible());
        assert_eq!(residual_message_box_type_ordinal(), 2);
        assert_eq!(
            residual_message_box_last_action(),
            ResidualMessageBoxAction::ShowYesNo
        );
        assert!(
            simulate_message_box_prepare_yes("Quit", "Confirm"),
            "show+yes residual must latch"
        );
        assert!(!residual_message_box_is_visible());
        assert_eq!(
            residual_message_box_last_action(),
            ResidualMessageBoxAction::Yes
        );
    }
}
