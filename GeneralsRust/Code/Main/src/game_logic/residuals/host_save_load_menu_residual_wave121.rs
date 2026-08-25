//! Wave 121 residual peels: PopupSaveLoad / SaveLoad menu residual
//! (ListboxGames + Load/Save/Delete/Back; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 106 SaveLoadLayoutType tables, Wave 118 ButtonLoadGame,
//! Wave 119 campaign. Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - PopupSaveLoad.cpp / SaveLoad.wnd / PopupSaveLoad.wnd
//! - SaveLoadLayoutType SaveAndLoad / LoadOnly / SaveOnly
//! - ButtonLoad/Save/Delete/Back + ListboxGames
//!
//! Fail-closed:
//! - Not full GameState::load_game / save file IO residual
//! - Not full confirm dialog widget residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// SaveLoad residual tables
// ---------------------------------------------------------------------------

/// Retail layout filename residual.
pub const SAVE_LOAD_LAYOUT_FILENAME_FULL_WAVE121: &str = "Menus/SaveLoad.wnd";
pub const SAVE_LOAD_LAYOUT_FILENAME_POPUP_WAVE121: &str = "Menus/PopupSaveLoad.wnd";

/// Retail layout prefix residual (NameKey gadget prefix).
pub const SAVE_LOAD_PREFIX_FULL_WAVE121: &str = "SaveLoad.wnd";
pub const SAVE_LOAD_PREFIX_POPUP_WAVE121: &str = "PopupSaveLoad.wnd";

/// Retail `SaveLoadLayoutType` residual names (skip Invalid sentinel).
pub const SAVE_LOAD_LAYOUT_TYPE_NAMES_WAVE121: &[&str] = &["SaveAndLoad", "LoadOnly", "SaveOnly"];

/// Retail SaveLoad control name stems residual.
pub const SAVE_LOAD_CONTROL_STEMS_WAVE121: &[&str] = &[
    "ButtonBack",
    "ButtonSave",
    "ButtonLoad",
    "ButtonDelete",
    "ListboxGames",
    "ButtonOverwriteCancel",
    "ButtonOverwriteConfirm",
    "ButtonLoadCancel",
    "ButtonLoadConfirm",
    "ButtonSaveDescCancel",
    "ButtonSaveDescConfirm",
    "ButtonDeleteConfirm",
    "ButtonDeleteCancel",
    "MenuButtonFrame",
    "OverwriteConfirmParent",
    "LoadConfirmParent",
    "SaveDescParent",
    "DeleteConfirmParent",
    "EntryDesc",
];

/// Ordered SaveLoad residual navigation steps.
pub const SAVE_LOAD_NAV_STEPS_WAVE121: &[&str] = &[
    "PUSH_SAVE_LOAD_LAYOUT",
    "BIND_GADGET_IDS",
    "POPULATE_LISTBOX_GAMES",
    "SELECT_LISTBOX_SLOT",
    "GBM_SELECTED_BUTTON_LOAD",
    "PROCESS_LOAD_OR_CONFIRM",
    "GBM_SELECTED_BUTTON_SAVE",
    "SAVE_DESC_OR_OVERWRITE",
    "GBM_SELECTED_BUTTON_DELETE",
    "DELETE_CONFIRM",
    "GBM_SELECTED_BUTTON_BACK",
];

/// Runtime-host command residual names for SaveLoad peels.
pub const RUNTIME_HOST_SAVE_LOAD_CMD_NAMES_WAVE121: &[&str] = &[
    "open_load_game_ok_wnd",
    "open_load_game_ok",
    "click_save_load_ok_wnd_load",
    "click_save_load_ok_wnd_save",
    "click_save_load_ok_wnd_delete",
    "click_save_load_ok_wnd_back",
    "click_save_load_miss",
];

/// Build residual control name for full-screen SaveLoad.wnd.
pub fn save_load_full_control_name_wave121(stem: &str) -> Option<String> {
    if !SAVE_LOAD_CONTROL_STEMS_WAVE121.contains(&stem) {
        return None;
    }
    Some(format!("{SAVE_LOAD_PREFIX_FULL_WAVE121}:{stem}"))
}

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: layout filenames + layout type residual pack.
pub fn honesty_save_load_layout_residual_wave121() -> bool {
    SAVE_LOAD_LAYOUT_FILENAME_FULL_WAVE121 == "Menus/SaveLoad.wnd"
        && SAVE_LOAD_LAYOUT_FILENAME_POPUP_WAVE121 == "Menus/PopupSaveLoad.wnd"
        && SAVE_LOAD_PREFIX_FULL_WAVE121 == "SaveLoad.wnd"
        && SAVE_LOAD_PREFIX_POPUP_WAVE121 == "PopupSaveLoad.wnd"
        && SAVE_LOAD_LAYOUT_TYPE_NAMES_WAVE121.len() == 3
        && residual_name_index(SAVE_LOAD_LAYOUT_TYPE_NAMES_WAVE121, "SaveAndLoad") == Some(0)
        && residual_name_index(SAVE_LOAD_LAYOUT_TYPE_NAMES_WAVE121, "LoadOnly") == Some(1)
        && residual_name_index(SAVE_LOAD_LAYOUT_TYPE_NAMES_WAVE121, "SaveOnly") == Some(2)
}

/// Honesty: control stems residual pack.
pub fn honesty_save_load_control_stems_residual_wave121() -> bool {
    SAVE_LOAD_CONTROL_STEMS_WAVE121.len() == 19
        && residual_name_index(SAVE_LOAD_CONTROL_STEMS_WAVE121, "ButtonLoad") == Some(2)
        && residual_name_index(SAVE_LOAD_CONTROL_STEMS_WAVE121, "ListboxGames") == Some(4)
        && residual_name_index(SAVE_LOAD_CONTROL_STEMS_WAVE121, "ButtonDeleteConfirm") == Some(11)
        && save_load_full_control_name_wave121("ButtonSave")
            == Some("SaveLoad.wnd:ButtonSave".into())
        && save_load_full_control_name_wave121("NoSuch").is_none()
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_save_load_nav_commands_residual_wave121() -> bool {
    SAVE_LOAD_NAV_STEPS_WAVE121.len() == 11
        && residual_name_index(SAVE_LOAD_NAV_STEPS_WAVE121, "SELECT_LISTBOX_SLOT") == Some(3)
        && residual_name_index(SAVE_LOAD_NAV_STEPS_WAVE121, "GBM_SELECTED_BUTTON_LOAD") == Some(4)
        && residual_name_index(SAVE_LOAD_NAV_STEPS_WAVE121, "GBM_SELECTED_BUTTON_BACK") == Some(10)
        && RUNTIME_HOST_SAVE_LOAD_CMD_NAMES_WAVE121.len() == 7
        && residual_name_index(
            RUNTIME_HOST_SAVE_LOAD_CMD_NAMES_WAVE121,
            "open_load_game_ok_wnd",
        ) == Some(0)
        && residual_name_index(
            RUNTIME_HOST_SAVE_LOAD_CMD_NAMES_WAVE121,
            "click_save_load_ok_wnd_load",
        ) == Some(2)
}

/// Wave 121 composite residual honesty pack.
pub fn honesty_save_load_menu_residual_pack_wave121() -> bool {
    honesty_save_load_layout_residual_wave121()
        && honesty_save_load_control_stems_residual_wave121()
        && honesty_save_load_nav_commands_residual_wave121()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_residual() {
        assert!(honesty_save_load_layout_residual_wave121());
    }

    #[test]
    fn control_stems_residual() {
        assert!(honesty_save_load_control_stems_residual_wave121());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_save_load_nav_commands_residual_wave121());
    }

    #[test]
    fn wave121_composite_pack() {
        assert!(honesty_save_load_menu_residual_pack_wave121());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_save_load_prepare_load_residual_live() {
        use game_client::gui::callbacks::{
            ResidualSaveLoadAction, residual_save_load_last_action,
            residual_save_load_selected_slot, simulate_save_load_menu_back_button_gadget_selected,
            simulate_save_load_menu_prepare_load,
        };
        assert!(
            simulate_save_load_menu_prepare_load(0),
            "LoadOnly select+load residual must latch"
        );
        assert_eq!(residual_save_load_selected_slot(), Some(0));
        assert_eq!(
            residual_save_load_last_action(),
            ResidualSaveLoadAction::Load
        );
        assert!(simulate_save_load_menu_back_button_gadget_selected());
        assert_eq!(
            residual_save_load_last_action(),
            ResidualSaveLoadAction::Back
        );
        assert!(residual_save_load_selected_slot().is_none());
    }
}
