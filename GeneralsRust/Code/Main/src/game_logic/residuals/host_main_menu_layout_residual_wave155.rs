//! Wave 155 residual peels: MainMenu UI layout residual
//! (create/hit-test 22 buttons; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 154 WindowVideo residual.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - MainMenu.wnd button/panel names
//! - UILayoutManager::create_main_menu_layout (800x600 authoring)
//!
//! Fail-closed:
//! - Not full WindowManager script load of MainMenu.wnd
//! - Not full WGPU UI draw residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// MainMenu layout residual tables
// ---------------------------------------------------------------------------

/// Retail MainMenu primary button count residual.
pub const MAIN_MENU_LAYOUT_BUTTON_COUNT_WAVE155: usize = 22;

/// Authoring resolution residual.
pub const MAIN_MENU_LAYOUT_BASE_RESOLUTION_WAVE155: (u32, u32) = (800, 600);

/// MainMenu layout button names residual (create_main_menu_layout).
pub const MAIN_MENU_LAYOUT_BUTTON_NAMES_WAVE155: &[&str] = &[
    "ButtonSinglePlayer",
    "ButtonMultiplayer",
    "ButtonLoadReplay",
    "ButtonOptions",
    "ButtonCredits",
    "ButtonExit",
    "ButtonUSA",
    "ButtonGLA",
    "ButtonChina",
    "ButtonChallenge",
    "ButtonSkirmish",
    "ButtonSingleBack",
    "ButtonOnline",
    "ButtonNetwork",
    "ButtonMultiBack",
    "ButtonLoadGame",
    "ButtonReplay",
    "ButtonLoadReplayBack",
    "ButtonEasy",
    "ButtonMedium",
    "ButtonHard",
    "ButtonDiffBack",
];

/// MainMenu chrome element names residual.
pub const MAIN_MENU_LAYOUT_CHROME_NAMES_WAVE155: &[&str] = &[
    "MainMenuBackground",
    "MainMenuRuler",
    "MainMenuTitle",
    "MapBorder2",
    "MapBorder",
    "MapBorder1",
    "MapBorder3",
    "MapBorder4",
    "StaticTextSelectDifficulty",
];

/// Ordered MainMenu layout residual navigation steps.
pub const MAIN_MENU_LAYOUT_NAV_STEPS_WAVE155: &[&str] = &[
    "CREATE_MAIN_MENU_LAYOUT_800x600",
    "SCALE_RECTS_FROM_AUTHORING",
    "INSERT_CHROME_PANELS",
    "ADD_PRIMARY_BUTTONS",
    "HIT_TEST_SINGLE_PLAYER",
    "ENTER_SHELL_OWNED_SCREEN",
];

/// Runtime-host command residual names for MainMenu layout peels.
pub const RUNTIME_HOST_MAIN_MENU_LAYOUT_CMD_NAMES_WAVE155: &[&str] = &[
    "click_main_menu_layout_ok_wnd_create",
    "click_main_menu_layout_ok_wnd_clear",
    "click_main_menu_layout_ok_wnd_hit",
    "click_main_menu_layout_ok_wnd_prepare",
    "click_main_menu_layout_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: MainMenu layout button/chrome names residual pack.
pub fn honesty_main_menu_layout_names_residual_wave155() -> bool {
    MAIN_MENU_LAYOUT_BUTTON_COUNT_WAVE155 == 22
        && MAIN_MENU_LAYOUT_BASE_RESOLUTION_WAVE155 == (800, 600)
        && MAIN_MENU_LAYOUT_BUTTON_NAMES_WAVE155.len() == 22
        && residual_name_index(MAIN_MENU_LAYOUT_BUTTON_NAMES_WAVE155, "ButtonSinglePlayer")
            == Some(0)
        && residual_name_index(MAIN_MENU_LAYOUT_BUTTON_NAMES_WAVE155, "ButtonExit") == Some(5)
        && residual_name_index(MAIN_MENU_LAYOUT_BUTTON_NAMES_WAVE155, "ButtonDiffBack") == Some(21)
        && MAIN_MENU_LAYOUT_CHROME_NAMES_WAVE155.len() == 9
        && residual_name_index(MAIN_MENU_LAYOUT_CHROME_NAMES_WAVE155, "MainMenuBackground")
            == Some(0)
        && residual_name_index(
            MAIN_MENU_LAYOUT_CHROME_NAMES_WAVE155,
            "StaticTextSelectDifficulty",
        ) == Some(8)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_main_menu_layout_nav_commands_residual_wave155() -> bool {
    MAIN_MENU_LAYOUT_NAV_STEPS_WAVE155.len() == 6
        && residual_name_index(
            MAIN_MENU_LAYOUT_NAV_STEPS_WAVE155,
            "CREATE_MAIN_MENU_LAYOUT_800x600",
        ) == Some(0)
        && residual_name_index(MAIN_MENU_LAYOUT_NAV_STEPS_WAVE155, "HIT_TEST_SINGLE_PLAYER")
            == Some(4)
        && residual_name_index(
            MAIN_MENU_LAYOUT_NAV_STEPS_WAVE155,
            "ENTER_SHELL_OWNED_SCREEN",
        ) == Some(5)
        && RUNTIME_HOST_MAIN_MENU_LAYOUT_CMD_NAMES_WAVE155.len() == 5
        && residual_name_index(
            RUNTIME_HOST_MAIN_MENU_LAYOUT_CMD_NAMES_WAVE155,
            "click_main_menu_layout_ok_wnd_prepare",
        ) == Some(3)
}

/// Wave 155 composite residual honesty pack.
pub fn honesty_main_menu_layout_residual_pack_wave155() -> bool {
    honesty_main_menu_layout_names_residual_wave155()
        && honesty_main_menu_layout_nav_commands_residual_wave155()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_residual() {
        assert!(honesty_main_menu_layout_names_residual_wave155());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_main_menu_layout_nav_commands_residual_wave155());
    }

    #[test]
    fn wave155_composite_pack() {
        assert!(honesty_main_menu_layout_residual_pack_wave155());
    }

    #[test]
    fn simulate_main_menu_layout_prepare_default_residual_live() {
        use crate::ui::{
            MAIN_MENU_LAYOUT_BUTTON_NAMES, ResidualMainMenuLayoutAction,
            residual_main_menu_layout_button_count, residual_main_menu_layout_last_action,
            simulate_main_menu_layout_prepare_default,
        };
        assert_eq!(
            MAIN_MENU_LAYOUT_BUTTON_NAMES,
            MAIN_MENU_LAYOUT_BUTTON_NAMES_WAVE155
        );
        assert!(
            simulate_main_menu_layout_prepare_default(),
            "create+hit-test residual must latch"
        );
        assert_eq!(
            residual_main_menu_layout_button_count(),
            MAIN_MENU_LAYOUT_BUTTON_COUNT_WAVE155
        );
        assert_eq!(
            residual_main_menu_layout_last_action(),
            ResidualMainMenuLayoutAction::HitTest
        );
    }
}
