//! Wave 122 residual peels: ReplayMenu WND residual
//! (ListboxReplayFiles + Load/Delete/Copy/Back; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 118 ButtonReplay, Wave 121 SaveLoad.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - ReplayMenu.cpp / ReplayMenu.wnd
//! - ButtonLoadReplay, ButtonDeleteReplay, ButtonCopyReplay, ButtonBack
//! - ListboxReplayFiles
//!
//! Fail-closed:
//! - Not full replay file parse / playback engine residual
//! - Not full copy/delete filesystem residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// Replay menu residual tables
// ---------------------------------------------------------------------------

/// Retail ReplayMenu layout filename residual.
pub const REPLAY_MENU_LAYOUT_FILENAME_WAVE122: &str = "Menus/ReplayMenu.wnd";

/// Retail ReplayMenu control names residual.
pub const REPLAY_MENU_CONTROL_NAMES_WAVE122: &[&str] = &[
    "ReplayMenu.wnd:ParentReplayMenu",
    "ReplayMenu.wnd:GadgetParent",
    "ReplayMenu.wnd:ButtonLoadReplay",
    "ReplayMenu.wnd:ButtonBack",
    "ReplayMenu.wnd:ButtonDeleteReplay",
    "ReplayMenu.wnd:ButtonCopyReplay",
    "ReplayMenu.wnd:ListboxReplayFiles",
];

/// Ordered ReplayMenu residual navigation steps.
pub const REPLAY_MENU_NAV_STEPS_WAVE122: &[&str] = &[
    "PUSH_REPLAY_MENU_LAYOUT",
    "POPULATE_LISTBOX_REPLAY_FILES",
    "SELECT_LISTBOX_SLOT",
    "GBM_SELECTED_BUTTON_LOAD_REPLAY",
    "PLAYBACK_SELECTED_REPLAY",
    "GBM_SELECTED_BUTTON_DELETE_REPLAY",
    "CONFIRM_DELETE",
    "GBM_SELECTED_BUTTON_COPY_REPLAY",
    "CONFIRM_COPY",
    "GBM_SELECTED_BUTTON_BACK",
    "SHELL_POP",
];

/// Runtime-host command residual names for ReplayMenu peels.
pub const RUNTIME_HOST_REPLAY_MENU_CMD_NAMES_WAVE122: &[&str] = &[
    "open_load_replay_menu_ok_wnd",
    "open_load_replay_menu_ok",
    "click_replay_menu_ok_wnd_load",
    "click_replay_menu_ok_wnd_delete",
    "click_replay_menu_ok_wnd_copy",
    "click_replay_menu_ok_wnd_back",
    "click_replay_menu_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: ReplayMenu control names residual pack.
pub fn honesty_replay_menu_control_names_residual_wave122() -> bool {
    REPLAY_MENU_LAYOUT_FILENAME_WAVE122 == "Menus/ReplayMenu.wnd"
        && REPLAY_MENU_CONTROL_NAMES_WAVE122.len() == 7
        && residual_name_index(
            REPLAY_MENU_CONTROL_NAMES_WAVE122,
            "ReplayMenu.wnd:ButtonLoadReplay",
        ) == Some(2)
        && residual_name_index(
            REPLAY_MENU_CONTROL_NAMES_WAVE122,
            "ReplayMenu.wnd:ButtonDeleteReplay",
        ) == Some(4)
        && residual_name_index(
            REPLAY_MENU_CONTROL_NAMES_WAVE122,
            "ReplayMenu.wnd:ListboxReplayFiles",
        ) == Some(6)
        && REPLAY_MENU_CONTROL_NAMES_WAVE122
            .iter()
            .all(|n| n.starts_with("ReplayMenu.wnd:"))
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_replay_menu_nav_commands_residual_wave122() -> bool {
    REPLAY_MENU_NAV_STEPS_WAVE122.len() == 11
        && residual_name_index(REPLAY_MENU_NAV_STEPS_WAVE122, "SELECT_LISTBOX_SLOT") == Some(2)
        && residual_name_index(
            REPLAY_MENU_NAV_STEPS_WAVE122,
            "GBM_SELECTED_BUTTON_LOAD_REPLAY",
        ) == Some(3)
        && residual_name_index(REPLAY_MENU_NAV_STEPS_WAVE122, "SHELL_POP") == Some(10)
        && RUNTIME_HOST_REPLAY_MENU_CMD_NAMES_WAVE122.len() == 7
        && residual_name_index(
            RUNTIME_HOST_REPLAY_MENU_CMD_NAMES_WAVE122,
            "open_load_replay_menu_ok_wnd",
        ) == Some(0)
        && residual_name_index(
            RUNTIME_HOST_REPLAY_MENU_CMD_NAMES_WAVE122,
            "click_replay_menu_ok_wnd_load",
        ) == Some(2)
}

/// Wave 122 composite residual honesty pack.
pub fn honesty_replay_menu_residual_pack_wave122() -> bool {
    honesty_replay_menu_control_names_residual_wave122()
        && honesty_replay_menu_nav_commands_residual_wave122()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_names_residual() {
        assert!(honesty_replay_menu_control_names_residual_wave122());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_replay_menu_nav_commands_residual_wave122());
    }

    #[test]
    fn wave122_composite_pack() {
        assert!(honesty_replay_menu_residual_pack_wave122());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_replay_prepare_load_residual_live() {
        use game_client::gui::callbacks::{
            ResidualReplayMenuAction, residual_replay_menu_last_action,
            residual_replay_menu_selected_slot, simulate_replay_menu_back_button_gadget_selected,
            simulate_replay_menu_prepare_load,
        };
        assert!(
            simulate_replay_menu_prepare_load(0),
            "select+load residual must latch"
        );
        assert_eq!(residual_replay_menu_selected_slot(), Some(0));
        assert_eq!(
            residual_replay_menu_last_action(),
            ResidualReplayMenuAction::Load
        );
        assert!(simulate_replay_menu_back_button_gadget_selected());
        assert_eq!(
            residual_replay_menu_last_action(),
            ResidualReplayMenuAction::Back
        );
        assert!(residual_replay_menu_selected_slot().is_none());
    }
}
