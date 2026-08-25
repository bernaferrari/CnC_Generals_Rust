//! Wave 136 residual peels: InGameChat WND residual
//! (show/hide/type/clear/submit; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 133 ControlBar, Wave 129 Diplomacy mute.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - InGameChat.cpp / InGameChat.wnd
//! - ButtonClear, TextEntryChat, StaticTextChatType
//!
//! Fail-closed:
//! - Not full network chat dispatch residual
//! - Not full slash-command residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// In-game chat residual tables
// ---------------------------------------------------------------------------

/// Retail InGameChat layout filename residual.
pub const IN_GAME_CHAT_LAYOUT_FILENAME_WAVE136: &str = "InGameChat.wnd";

/// Retail InGameChat control names residual.
pub const IN_GAME_CHAT_CONTROL_NAMES_WAVE136: &[&str] = &[
    "InGameChat.wnd:ParentInGameChat",
    "InGameChat.wnd:ButtonClear",
    "InGameChat.wnd:TextEntryChat",
    "InGameChat.wnd:StaticTextChatType",
];

/// Ordered InGameChat residual navigation steps.
pub const IN_GAME_CHAT_NAV_STEPS_WAVE136: &[&str] = &[
    "TOGGLE_OR_SHOW_CHAT",
    "SET_CHAT_TYPE_EVERYONE_ALLIES_PLAYERS",
    "TYPE_MESSAGE",
    "ENTER_SUBMIT",
    "GBM_SELECTED_BUTTON_CLEAR",
    "HIDE_CHAT",
];

/// Runtime-host command residual names for InGameChat peels.
pub const RUNTIME_HOST_IN_GAME_CHAT_CMD_NAMES_WAVE136: &[&str] = &[
    "toggle_in_game_chat_ok_wnd",
    "toggle_in_game_chat_miss",
    "click_in_game_chat_ok_wnd_show",
    "click_in_game_chat_ok_wnd_submit",
    "click_in_game_chat_ok_wnd_clear",
    "click_in_game_chat_ok_wnd_prepare_submit",
    "click_in_game_chat_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: InGameChat control names residual pack.
pub fn honesty_in_game_chat_control_names_residual_wave136() -> bool {
    IN_GAME_CHAT_LAYOUT_FILENAME_WAVE136 == "InGameChat.wnd"
        && IN_GAME_CHAT_CONTROL_NAMES_WAVE136.len() == 4
        && residual_name_index(
            IN_GAME_CHAT_CONTROL_NAMES_WAVE136,
            "InGameChat.wnd:ButtonClear",
        ) == Some(1)
        && residual_name_index(
            IN_GAME_CHAT_CONTROL_NAMES_WAVE136,
            "InGameChat.wnd:TextEntryChat",
        ) == Some(2)
        && IN_GAME_CHAT_CONTROL_NAMES_WAVE136
            .iter()
            .all(|n| n.starts_with("InGameChat.wnd:"))
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_in_game_chat_nav_commands_residual_wave136() -> bool {
    IN_GAME_CHAT_NAV_STEPS_WAVE136.len() == 6
        && residual_name_index(IN_GAME_CHAT_NAV_STEPS_WAVE136, "ENTER_SUBMIT") == Some(3)
        && residual_name_index(IN_GAME_CHAT_NAV_STEPS_WAVE136, "GBM_SELECTED_BUTTON_CLEAR")
            == Some(4)
        && residual_name_index(IN_GAME_CHAT_NAV_STEPS_WAVE136, "HIDE_CHAT") == Some(5)
        && RUNTIME_HOST_IN_GAME_CHAT_CMD_NAMES_WAVE136.len() == 7
        && residual_name_index(
            RUNTIME_HOST_IN_GAME_CHAT_CMD_NAMES_WAVE136,
            "click_in_game_chat_ok_wnd_prepare_submit",
        ) == Some(5)
}

/// Wave 136 composite residual honesty pack.
pub fn honesty_in_game_chat_residual_pack_wave136() -> bool {
    honesty_in_game_chat_control_names_residual_wave136()
        && honesty_in_game_chat_nav_commands_residual_wave136()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_names_residual() {
        assert!(honesty_in_game_chat_control_names_residual_wave136());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_in_game_chat_nav_commands_residual_wave136());
    }

    #[test]
    fn wave136_composite_pack() {
        assert!(honesty_in_game_chat_residual_pack_wave136());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_chat_prepare_submit_residual_live() {
        use game_client::gui::callbacks::{
            ResidualInGameChatAction, residual_in_game_chat_is_active,
            residual_in_game_chat_last_action, residual_in_game_chat_text,
            residual_in_game_chat_type_ordinal, simulate_in_game_chat_clear_button_gadget_selected,
            simulate_in_game_chat_prepare_submit,
        };
        assert!(
            simulate_in_game_chat_prepare_submit("hello"),
            "show+type+submit residual must latch"
        );
        assert!(residual_in_game_chat_is_active());
        assert_eq!(residual_in_game_chat_type_ordinal(), 1);
        assert_eq!(residual_in_game_chat_text(), "hello");
        assert_eq!(
            residual_in_game_chat_last_action(),
            ResidualInGameChatAction::Submit
        );
        assert!(simulate_in_game_chat_clear_button_gadget_selected());
        assert!(residual_in_game_chat_text().is_empty());
    }
}
