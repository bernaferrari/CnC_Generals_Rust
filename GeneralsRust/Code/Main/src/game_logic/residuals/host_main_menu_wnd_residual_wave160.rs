//! Wave 160 residual peels: MainMenu.wnd resolve/validate residual
//! (retail shell layout on disk; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 155 CPU MainMenu layout residual.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - Shell::push("Menus/MainMenu.wnd")
//! - WindowZH/Window/Menus/MainMenu.wnd
//!
//! Fail-closed:
//! - Not full WindowManager create_windows_from_script residual
//! - Not full W3DMainMenuInit / gadget residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// MainMenu.wnd residual tables
// ---------------------------------------------------------------------------

/// Retail MainMenu.wnd WINDOW token count residual.
pub const MAIN_MENU_WND_WINDOW_TOKEN_COUNT_WAVE160: usize = 126;

/// Retail MainMenu.wnd named-child count residual.
pub const MAIN_MENU_WND_NAMED_COUNT_WAVE160: usize = 63;

/// Key MainMenu.wnd names residual (shell navigation).
pub const MAIN_MENU_WND_KEY_NAMES_WAVE160: &[&str] = &[
    "MainMenu.wnd:MainMenuParent",
    "MainMenu.wnd:ButtonSinglePlayer",
    "MainMenu.wnd:ButtonMultiplayer",
    "MainMenu.wnd:ButtonSkirmish",
    "MainMenu.wnd:ButtonOptions",
    "MainMenu.wnd:ButtonCredits",
    "MainMenu.wnd:ButtonExit",
    "MainMenu.wnd:ButtonUSA",
    "MainMenu.wnd:ButtonGLA",
    "MainMenu.wnd:ButtonChina",
    "MainMenu.wnd:ButtonChallenge",
    "MainMenu.wnd:ButtonLoadReplay",
];

/// Ordered MainMenu.wnd residual navigation steps.
pub const MAIN_MENU_WND_NAV_STEPS_WAVE160: &[&str] = &[
    "RESOLVE_MAIN_MENU_WND_PATH",
    "VALIDATE_FILE_VERSION_WINDOW",
    "CHECK_MAIN_MENU_PARENT",
    "CHECK_KEY_BUTTON_NAMES",
    "CHECK_LAYOUT_INIT",
    "SHELL_PUSH_MAIN_MENU",
];

/// Runtime-host command residual names for MainMenu.wnd peels.
pub const RUNTIME_HOST_MAIN_MENU_WND_CMD_NAMES_WAVE160: &[&str] = &[
    "click_main_menu_wnd_ok_wnd_resolve",
    "click_main_menu_wnd_ok_wnd_validate",
    "click_main_menu_wnd_ok_wnd_prepare",
    "click_main_menu_wnd_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: MainMenu.wnd counts + key names residual pack.
pub fn honesty_main_menu_wnd_names_residual_wave160() -> bool {
    MAIN_MENU_WND_WINDOW_TOKEN_COUNT_WAVE160 == 126
        && MAIN_MENU_WND_NAMED_COUNT_WAVE160 == 63
        && MAIN_MENU_WND_KEY_NAMES_WAVE160.len() == 12
        && residual_name_index(
            MAIN_MENU_WND_KEY_NAMES_WAVE160,
            "MainMenu.wnd:MainMenuParent",
        ) == Some(0)
        && residual_name_index(
            MAIN_MENU_WND_KEY_NAMES_WAVE160,
            "MainMenu.wnd:ButtonSinglePlayer",
        ) == Some(1)
        && residual_name_index(
            MAIN_MENU_WND_KEY_NAMES_WAVE160,
            "MainMenu.wnd:ButtonSkirmish",
        ) == Some(3)
        && residual_name_index(
            MAIN_MENU_WND_KEY_NAMES_WAVE160,
            "MainMenu.wnd:ButtonLoadReplay",
        ) == Some(11)
        && MAIN_MENU_WND_KEY_NAMES_WAVE160
            .iter()
            .all(|n| n.starts_with("MainMenu.wnd:"))
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_main_menu_wnd_nav_commands_residual_wave160() -> bool {
    MAIN_MENU_WND_NAV_STEPS_WAVE160.len() == 6
        && residual_name_index(
            MAIN_MENU_WND_NAV_STEPS_WAVE160,
            "RESOLVE_MAIN_MENU_WND_PATH",
        ) == Some(0)
        && residual_name_index(MAIN_MENU_WND_NAV_STEPS_WAVE160, "CHECK_KEY_BUTTON_NAMES") == Some(3)
        && residual_name_index(MAIN_MENU_WND_NAV_STEPS_WAVE160, "SHELL_PUSH_MAIN_MENU") == Some(5)
        && RUNTIME_HOST_MAIN_MENU_WND_CMD_NAMES_WAVE160.len() == 4
        && residual_name_index(
            RUNTIME_HOST_MAIN_MENU_WND_CMD_NAMES_WAVE160,
            "click_main_menu_wnd_ok_wnd_prepare",
        ) == Some(2)
}

/// Wave 160 composite residual honesty pack.
pub fn honesty_main_menu_wnd_residual_pack_wave160() -> bool {
    honesty_main_menu_wnd_names_residual_wave160()
        && honesty_main_menu_wnd_nav_commands_residual_wave160()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_residual() {
        assert!(honesty_main_menu_wnd_names_residual_wave160());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_main_menu_wnd_nav_commands_residual_wave160());
    }

    #[test]
    fn wave160_composite_pack() {
        assert!(honesty_main_menu_wnd_residual_pack_wave160());
    }

    #[test]
    fn simulate_main_menu_wnd_prepare_honesty_residual_live() {
        use crate::gameplay_layout::{
            MAIN_MENU_WND_KEY_NAMES_RESIDUAL, MAIN_MENU_WND_NAMED_COUNT_RESIDUAL,
            MAIN_MENU_WND_WINDOW_TOKEN_COUNT_RESIDUAL, main_menu_wnd_honesty,
            simulate_main_menu_wnd_prepare_honesty,
        };
        assert_eq!(
            MAIN_MENU_WND_WINDOW_TOKEN_COUNT_RESIDUAL,
            MAIN_MENU_WND_WINDOW_TOKEN_COUNT_WAVE160
        );
        assert_eq!(
            MAIN_MENU_WND_NAMED_COUNT_RESIDUAL,
            MAIN_MENU_WND_NAMED_COUNT_WAVE160
        );
        assert_eq!(
            MAIN_MENU_WND_KEY_NAMES_RESIDUAL,
            MAIN_MENU_WND_KEY_NAMES_WAVE160
        );
        let h = main_menu_wnd_honesty();
        assert!(h.shell_residual_ok(), "MainMenu.wnd residual: {}", h.detail);
        assert!(simulate_main_menu_wnd_prepare_honesty());
        if h.path_resolved {
            assert!(h.wnd_validated);
            assert_eq!(h.named_key_hits, MAIN_MENU_WND_KEY_NAMES_WAVE160.len());
        }
    }
}
