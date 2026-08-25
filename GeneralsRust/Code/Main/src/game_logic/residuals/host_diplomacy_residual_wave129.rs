//! Wave 129 residual peels: Diplomacy WND residual
//! (toggle/radio/mute/hide; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 128 MessageBox, in-game UI residual packs.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - Diplomacy.cpp / Diplomacy.wnd
//! - RadioButtonInGame, RadioButtonBuddies, ButtonMute/UnMute, ButtonHide
//!
//! Fail-closed:
//! - Not full layout animate / alliance request residual
//! - Not full buddy list / multiplayer-only residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// Diplomacy residual tables
// ---------------------------------------------------------------------------

/// Retail Diplomacy layout filename residual.
pub const DIPLOMACY_LAYOUT_FILENAME_WAVE129: &str = "Menus/Diplomacy.wnd";

/// Retail Diplomacy control names residual.
pub const DIPLOMACY_CONTROL_NAMES_WAVE129: &[&str] = &[
    "Diplomacy.wnd:RadioButtonInGame",
    "Diplomacy.wnd:RadioButtonBuddies",
    "Diplomacy.wnd:InGameParent",
    "Diplomacy.wnd:BuddiesParent",
    "Diplomacy.wnd:SoloParent",
    "Diplomacy.wnd:ButtonHide",
    "Diplomacy.wnd:ButtonMute0",
    "Diplomacy.wnd:ButtonUnMute0",
    "Diplomacy.wnd:StaticTextPlayer",
    "Diplomacy.wnd:StaticTextSide",
    "Diplomacy.wnd:StaticTextTeam",
    "Diplomacy.wnd:StaticTextStatus",
];

/// Ordered Diplomacy residual navigation steps.
pub const DIPLOMACY_NAV_STEPS_WAVE129: &[&str] = &[
    "TOGGLE_DIPLOMACY_SHOW",
    "LOAD_DIPLOMACY_LAYOUT",
    "GBM_SELECTED_RADIO_INGAME",
    "SHOW_INGAME_PARENT",
    "GBM_SELECTED_RADIO_BUDDIES",
    "SHOW_BUDDIES_PARENT",
    "GBM_SELECTED_BUTTON_MUTE",
    "GBM_SELECTED_BUTTON_UNMUTE",
    "GBM_SELECTED_BUTTON_HIDE",
    "TOGGLE_DIPLOMACY_HIDE",
];

/// Runtime-host command residual names for Diplomacy peels.
pub const RUNTIME_HOST_DIPLOMACY_CMD_NAMES_WAVE129: &[&str] = &[
    "toggle_diplomacy_ok_wnd",
    "toggle_diplomacy_miss",
    "click_diplomacy_ok_wnd_ingame",
    "click_diplomacy_ok_wnd_buddies",
    "click_diplomacy_ok_wnd_mute",
    "click_diplomacy_ok_wnd_unmute",
    "click_diplomacy_ok_wnd_hide",
    "click_diplomacy_ok_wnd_prepare_ingame",
    "click_diplomacy_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: Diplomacy control names residual pack.
pub fn honesty_diplomacy_control_names_residual_wave129() -> bool {
    DIPLOMACY_LAYOUT_FILENAME_WAVE129 == "Menus/Diplomacy.wnd"
        && DIPLOMACY_CONTROL_NAMES_WAVE129.len() == 12
        && residual_name_index(
            DIPLOMACY_CONTROL_NAMES_WAVE129,
            "Diplomacy.wnd:RadioButtonInGame",
        ) == Some(0)
        && residual_name_index(DIPLOMACY_CONTROL_NAMES_WAVE129, "Diplomacy.wnd:ButtonMute0")
            == Some(6)
        && residual_name_index(DIPLOMACY_CONTROL_NAMES_WAVE129, "Diplomacy.wnd:ButtonHide")
            == Some(5)
        && DIPLOMACY_CONTROL_NAMES_WAVE129
            .iter()
            .all(|n| n.starts_with("Diplomacy.wnd:"))
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_diplomacy_nav_commands_residual_wave129() -> bool {
    DIPLOMACY_NAV_STEPS_WAVE129.len() == 10
        && residual_name_index(DIPLOMACY_NAV_STEPS_WAVE129, "TOGGLE_DIPLOMACY_SHOW") == Some(0)
        && residual_name_index(DIPLOMACY_NAV_STEPS_WAVE129, "GBM_SELECTED_RADIO_INGAME") == Some(2)
        && residual_name_index(DIPLOMACY_NAV_STEPS_WAVE129, "TOGGLE_DIPLOMACY_HIDE") == Some(9)
        && RUNTIME_HOST_DIPLOMACY_CMD_NAMES_WAVE129.len() == 9
        && residual_name_index(
            RUNTIME_HOST_DIPLOMACY_CMD_NAMES_WAVE129,
            "toggle_diplomacy_ok_wnd",
        ) == Some(0)
        && residual_name_index(
            RUNTIME_HOST_DIPLOMACY_CMD_NAMES_WAVE129,
            "click_diplomacy_ok_wnd_prepare_ingame",
        ) == Some(7)
}

/// Wave 129 composite residual honesty pack.
pub fn honesty_diplomacy_residual_pack_wave129() -> bool {
    honesty_diplomacy_control_names_residual_wave129()
        && honesty_diplomacy_nav_commands_residual_wave129()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_names_residual() {
        assert!(honesty_diplomacy_control_names_residual_wave129());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_diplomacy_nav_commands_residual_wave129());
    }

    #[test]
    fn wave129_composite_pack() {
        assert!(honesty_diplomacy_residual_pack_wave129());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_diplomacy_prepare_ingame_residual_live() {
        use game_client::gui::callbacks::{
            ResidualDiplomacyAction, residual_diplomacy_is_active, residual_diplomacy_last_action,
            residual_diplomacy_mute_slot, simulate_diplomacy_hide, simulate_diplomacy_mute_slot,
            simulate_diplomacy_prepare_ingame,
        };
        assert!(
            simulate_diplomacy_prepare_ingame(),
            "show+ingame residual must latch"
        );
        assert!(residual_diplomacy_is_active());
        assert_eq!(
            residual_diplomacy_last_action(),
            ResidualDiplomacyAction::RadioInGame
        );
        assert!(simulate_diplomacy_mute_slot(1));
        assert_eq!(residual_diplomacy_mute_slot(), Some(1));
        assert_eq!(
            residual_diplomacy_last_action(),
            ResidualDiplomacyAction::Mute
        );
        assert!(simulate_diplomacy_hide());
        assert!(!residual_diplomacy_is_active());
    }
}
