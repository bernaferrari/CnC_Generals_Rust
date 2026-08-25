//! Wave 130 residual peels: PopupReplay WND residual
//! (ListboxGames + name/Save/Back; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 125 ScoreScreen SaveReplay, Wave 122 ReplayMenu.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - PopupReplay.cpp / PopupReplay.wnd
//! - ButtonSave, ButtonBack, ListboxGames, TextEntryReplayName
//!
//! Fail-closed:
//! - Not full filesystem replay write residual
//! - Not full listbox populate residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// Popup replay residual tables
// ---------------------------------------------------------------------------

/// Retail PopupReplay layout filename residual.
pub const POPUP_REPLAY_LAYOUT_FILENAME_WAVE130: &str = "Menus/PopupReplay.wnd";

/// Retail PopupReplay control names residual.
pub const POPUP_REPLAY_CONTROL_NAMES_WAVE130: &[&str] = &[
    "PopupReplay.wnd:PopupReplayMenu",
    "PopupReplay.wnd:ButtonBack",
    "PopupReplay.wnd:ButtonSave",
    "PopupReplay.wnd:ListboxGames",
    "PopupReplay.wnd:TextEntryReplayName",
    "PopupReplay.wnd:PopupReplaySaved",
    "PopupReplay.wnd:MenuButtonFrame",
];

/// Ordered PopupReplay residual navigation steps.
pub const POPUP_REPLAY_NAV_STEPS_WAVE130: &[&str] = &[
    "OPEN_FROM_SCORE_SCREEN_SAVE_REPLAY",
    "POPULATE_LISTBOX_GAMES",
    "GLM_SELECTED_LISTBOX_SLOT",
    "COPY_NAME_TO_TEXT_ENTRY",
    "TEXT_ENTRY_REPLAY_NAME",
    "GBM_SELECTED_BUTTON_SAVE",
    "WRITE_REPLAY_FILE",
    "SHOW_REPLAY_SAVED_POPUP",
    "GBM_SELECTED_BUTTON_BACK",
    "ENABLE_SCORE_SCREEN_CONTROLS",
];

/// Runtime-host command residual names for PopupReplay peels.
pub const RUNTIME_HOST_POPUP_REPLAY_CMD_NAMES_WAVE130: &[&str] = &[
    "open_popup_replay_ok_wnd",
    "open_popup_replay_ok",
    "click_popup_replay_ok_wnd_save",
    "click_popup_replay_ok_wnd_select",
    "click_popup_replay_ok_wnd_name",
    "click_popup_replay_ok_wnd_back",
    "click_popup_replay_ok_wnd_prepare_save",
    "click_popup_replay_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: PopupReplay control names residual pack.
pub fn honesty_popup_replay_control_names_residual_wave130() -> bool {
    POPUP_REPLAY_LAYOUT_FILENAME_WAVE130 == "Menus/PopupReplay.wnd"
        && POPUP_REPLAY_CONTROL_NAMES_WAVE130.len() == 7
        && residual_name_index(
            POPUP_REPLAY_CONTROL_NAMES_WAVE130,
            "PopupReplay.wnd:ButtonSave",
        ) == Some(2)
        && residual_name_index(
            POPUP_REPLAY_CONTROL_NAMES_WAVE130,
            "PopupReplay.wnd:ListboxGames",
        ) == Some(3)
        && residual_name_index(
            POPUP_REPLAY_CONTROL_NAMES_WAVE130,
            "PopupReplay.wnd:TextEntryReplayName",
        ) == Some(4)
        && POPUP_REPLAY_CONTROL_NAMES_WAVE130
            .iter()
            .all(|n| n.starts_with("PopupReplay.wnd:"))
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_popup_replay_nav_commands_residual_wave130() -> bool {
    POPUP_REPLAY_NAV_STEPS_WAVE130.len() == 10
        && residual_name_index(
            POPUP_REPLAY_NAV_STEPS_WAVE130,
            "OPEN_FROM_SCORE_SCREEN_SAVE_REPLAY",
        ) == Some(0)
        && residual_name_index(POPUP_REPLAY_NAV_STEPS_WAVE130, "GBM_SELECTED_BUTTON_SAVE")
            == Some(5)
        && residual_name_index(
            POPUP_REPLAY_NAV_STEPS_WAVE130,
            "ENABLE_SCORE_SCREEN_CONTROLS",
        ) == Some(9)
        && RUNTIME_HOST_POPUP_REPLAY_CMD_NAMES_WAVE130.len() == 8
        && residual_name_index(
            RUNTIME_HOST_POPUP_REPLAY_CMD_NAMES_WAVE130,
            "open_popup_replay_ok_wnd",
        ) == Some(0)
        && residual_name_index(
            RUNTIME_HOST_POPUP_REPLAY_CMD_NAMES_WAVE130,
            "click_popup_replay_ok_wnd_prepare_save",
        ) == Some(6)
}

/// Wave 130 composite residual honesty pack.
pub fn honesty_popup_replay_residual_pack_wave130() -> bool {
    honesty_popup_replay_control_names_residual_wave130()
        && honesty_popup_replay_nav_commands_residual_wave130()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_names_residual() {
        assert!(honesty_popup_replay_control_names_residual_wave130());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_popup_replay_nav_commands_residual_wave130());
    }

    #[test]
    fn wave130_composite_pack() {
        assert!(honesty_popup_replay_residual_pack_wave130());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_popup_replay_prepare_save_residual_live() {
        use game_client::gui::callbacks::{
            ResidualPopupReplayAction, residual_popup_replay_last_action,
            residual_popup_replay_name, residual_popup_replay_selected_slot,
            simulate_popup_replay_back_button_gadget_selected,
            simulate_popup_replay_prepare_save_from_slot,
        };
        assert!(
            simulate_popup_replay_prepare_save_from_slot(0, "TestReplay"),
            "slot+name+save residual must latch"
        );
        assert_eq!(residual_popup_replay_selected_slot(), Some(0));
        assert_eq!(residual_popup_replay_name(), "TestReplay");
        assert_eq!(
            residual_popup_replay_last_action(),
            ResidualPopupReplayAction::Save
        );
        assert!(simulate_popup_replay_back_button_gadget_selected());
        assert_eq!(
            residual_popup_replay_last_action(),
            ResidualPopupReplayAction::Back
        );
        assert!(residual_popup_replay_selected_slot().is_none());
        assert!(residual_popup_replay_name().is_empty());
    }
}
