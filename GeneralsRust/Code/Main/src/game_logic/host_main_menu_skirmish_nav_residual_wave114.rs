//! Wave 114 residual peels: MainMenu → Skirmish WND navigation residual
//! (host-testable shell navigation path; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 106 MainMenu button/faction tables + WindowLayout,
//! Wave 110 MessageStream/InGameUI, Wave 112 Mouse/Keyboard/View,
//! Wave 113 Gadget/Video/Audio.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - MainMenu.cpp ButtonSkirmish GBM_SELECTED → PushShellScreen(
//!   Menus/SkirmishGameOptionsMenu.wnd) + ShellMainMenuSkirmishPushed
//! - MainMenu.wnd:ButtonSkirmish / MainMenuParent name keys
//! - SkirmishGameOptionsMenu.wnd:ButtonStart residual (cross-link Wave path)
//! - Gadget GBM_SELECTED = GGM_LEFT_DRAG + 8
//!
//! Fail-closed:
//! - Not full W3D TransitionHandler / animate-window retail residual
//! - Not full interactive mouse hit-test on live WND widgets
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// MainMenu → Skirmish navigation residual
// ---------------------------------------------------------------------------

/// Retail MainMenu layout filename residual.
pub const MAIN_MENU_LAYOUT_FILENAME_WAVE114: &str = "Menus/MainMenu.wnd";

/// Retail Skirmish options layout filename residual (ButtonSkirmish push target).
pub const SKIRMISH_OPTIONS_LAYOUT_FILENAME_WAVE114: &str = "Menus/SkirmishGameOptionsMenu.wnd";

/// Retail MainMenu parent window name residual.
pub const MAIN_MENU_PARENT_NAME_WAVE114: &str = "MainMenu.wnd:MainMenuParent";

/// Retail MainMenu ButtonSkirmish control name residual.
pub const MAIN_MENU_BUTTON_SKIRMISH_NAME_WAVE114: &str = "MainMenu.wnd:ButtonSkirmish";

/// Retail Skirmish ButtonStart control name residual.
pub const SKIRMISH_BUTTON_START_NAME_WAVE114: &str = "SkirmishGameOptionsMenu.wnd:ButtonStart";

/// Retail shell hook name residual (C++ THE_SHELL_HOOK_NAMES skirmish selected).
pub const SHELL_HOOK_MAIN_MENU_SKIRMISH_SELECTED_WAVE114: &str = "ShellMainMenuSkirmishPushed";

/// Retail transition group residual names on ButtonSkirmish path.
pub const MAIN_MENU_SKIRMISH_TRANSITION_GROUPS_WAVE114: &[&str] = &[
    "MainMenuFactionSkirmish",
    "MainMenuSinglePlayerMenuBackSkirmish",
];

/// Ordered MainMenu → Skirmish residual navigation steps.
pub const MAIN_MENU_SKIRMISH_NAV_STEPS_WAVE114: &[&str] = &[
    "ENSURE_MAIN_MENU_LAYOUT",
    "GBM_SELECTED_BUTTON_SKIRMISH",
    "LATCH_BUTTON_PUSHED",
    "LATCH_CAMPAIGN_SELECTED",
    "QUEUE_PUSH_SKIRMISH_OPTIONS",
    "SIGNAL_SHELL_HOOK_SKIRMISH",
    "OPEN_SKIRMISH_MENU_OK_WND",
    "GBM_SELECTED_BUTTON_START",
    "START_GAME_FROM_UI",
];

/// Retail `GGM_LEFT_DRAG` residual base (cross-link Wave 113).
pub const GGM_LEFT_DRAG_WAVE114: u32 = 16384;
/// Retail `GBM_SELECTED` residual (GGM_LEFT_DRAG + 8).
pub const GBM_SELECTED_WAVE114: u32 = GGM_LEFT_DRAG_WAVE114 + 8;

/// Runtime-host command residual names for this navigation peel.
pub const RUNTIME_HOST_SKIRMISH_NAV_CMD_NAMES_WAVE114: &[&str] = &[
    "open_skirmish_menu_ok",
    "open_skirmish_menu_ok_wnd",
    "click_skirmish_start_ok",
    "click_skirmish_start_ok_wnd",
    "click_skirmish_start_wnd_pending",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: MainMenu ButtonSkirmish name/layout residual pack.
pub fn honesty_main_menu_skirmish_names_residual_wave114() -> bool {
    MAIN_MENU_LAYOUT_FILENAME_WAVE114 == "Menus/MainMenu.wnd"
        && SKIRMISH_OPTIONS_LAYOUT_FILENAME_WAVE114 == "Menus/SkirmishGameOptionsMenu.wnd"
        && MAIN_MENU_PARENT_NAME_WAVE114 == "MainMenu.wnd:MainMenuParent"
        && MAIN_MENU_BUTTON_SKIRMISH_NAME_WAVE114 == "MainMenu.wnd:ButtonSkirmish"
        && SKIRMISH_BUTTON_START_NAME_WAVE114 == "SkirmishGameOptionsMenu.wnd:ButtonStart"
        && SHELL_HOOK_MAIN_MENU_SKIRMISH_SELECTED_WAVE114 == "ShellMainMenuSkirmishPushed"
        && MAIN_MENU_BUTTON_SKIRMISH_NAME_WAVE114.starts_with("MainMenu.wnd:")
        && SKIRMISH_BUTTON_START_NAME_WAVE114.starts_with("SkirmishGameOptionsMenu.wnd:")
}

/// Honesty: transition groups + nav step residual pack.
pub fn honesty_main_menu_skirmish_nav_steps_residual_wave114() -> bool {
    MAIN_MENU_SKIRMISH_TRANSITION_GROUPS_WAVE114.len() == 2
        && residual_name_index(
            MAIN_MENU_SKIRMISH_TRANSITION_GROUPS_WAVE114,
            "MainMenuFactionSkirmish",
        ) == Some(0)
        && residual_name_index(
            MAIN_MENU_SKIRMISH_TRANSITION_GROUPS_WAVE114,
            "MainMenuSinglePlayerMenuBackSkirmish",
        ) == Some(1)
        && MAIN_MENU_SKIRMISH_NAV_STEPS_WAVE114.len() == 9
        && residual_name_index(
            MAIN_MENU_SKIRMISH_NAV_STEPS_WAVE114,
            "GBM_SELECTED_BUTTON_SKIRMISH",
        ) == Some(1)
        && residual_name_index(
            MAIN_MENU_SKIRMISH_NAV_STEPS_WAVE114,
            "QUEUE_PUSH_SKIRMISH_OPTIONS",
        ) == Some(4)
        && residual_name_index(
            MAIN_MENU_SKIRMISH_NAV_STEPS_WAVE114,
            "OPEN_SKIRMISH_MENU_OK_WND",
        ) == Some(6)
        && residual_name_index(MAIN_MENU_SKIRMISH_NAV_STEPS_WAVE114, "START_GAME_FROM_UI")
            == Some(8)
}

/// Honesty: GBM_SELECTED + runtime-host command residual pack.
pub fn honesty_main_menu_skirmish_message_residual_wave114() -> bool {
    GGM_LEFT_DRAG_WAVE114 == 16384
        && GBM_SELECTED_WAVE114 == 16392
        && RUNTIME_HOST_SKIRMISH_NAV_CMD_NAMES_WAVE114.len() == 5
        && residual_name_index(
            RUNTIME_HOST_SKIRMISH_NAV_CMD_NAMES_WAVE114,
            "open_skirmish_menu_ok_wnd",
        ) == Some(1)
        && residual_name_index(
            RUNTIME_HOST_SKIRMISH_NAV_CMD_NAMES_WAVE114,
            "click_skirmish_start_ok_wnd",
        ) == Some(3)
        && residual_name_index(
            RUNTIME_HOST_SKIRMISH_NAV_CMD_NAMES_WAVE114,
            "open_skirmish_menu_ok",
        ) == Some(0)
}

/// Wave 114 composite residual honesty pack.
pub fn honesty_main_menu_skirmish_nav_residual_pack_wave114() -> bool {
    honesty_main_menu_skirmish_names_residual_wave114()
        && honesty_main_menu_skirmish_nav_steps_residual_wave114()
        && honesty_main_menu_skirmish_message_residual_wave114()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_menu_skirmish_names() {
        assert!(honesty_main_menu_skirmish_names_residual_wave114());
    }

    #[test]
    fn main_menu_skirmish_nav_steps() {
        assert!(honesty_main_menu_skirmish_nav_steps_residual_wave114());
    }

    #[test]
    fn main_menu_skirmish_message() {
        assert!(honesty_main_menu_skirmish_message_residual_wave114());
    }

    #[test]
    fn wave114_composite_pack() {
        assert!(honesty_main_menu_skirmish_nav_residual_pack_wave114());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_main_menu_skirmish_button_residual_live() {
        // Live GameClient residual: ButtonSkirmish GBM_SELECTED latch.
        assert!(
            game_client::gui::simulate_main_menu_skirmish_button_gadget_selected(),
            "MainMenu ButtonSkirmish residual must latch"
        );
    }
}
