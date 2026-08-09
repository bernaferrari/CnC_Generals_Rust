//! Wave 161 residual peels: MainMenu.wnd headless WindowManager load residual
//! (script → window tree; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 160 resolve/validate residual.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - Shell::push("Menus/MainMenu.wnd")
//! - WindowManager::create_windows_from_script / load_window
//!
//! Fail-closed:
//! - Not full W3DMainMenuInit / gadget draw residual
//! - Not full shell stack push residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// MainMenu.wnd load residual tables
// ---------------------------------------------------------------------------

/// Load path residual method names.
pub const MAIN_MENU_WND_LOAD_METHOD_NAMES_WAVE161: &[&str] = &[
    "main_menu_wnd_honesty_with_load",
    "try_load_main_menu_via_window_manager",
    "WindowManager::load_window",
    "create_windows_from_script",
    "window_count",
];

/// Retail script name residual candidates for load_window.
pub const MAIN_MENU_WND_LOAD_SCRIPT_NAMES_WAVE161: &[&str] = &[
    "Menus/MainMenu.wnd",
    "MainMenu.wnd",
    "Window/Menus/MainMenu.wnd",
];

/// Ordered MainMenu.wnd load residual navigation steps.
pub const MAIN_MENU_WND_LOAD_NAV_STEPS_WAVE161: &[&str] = &[
    "RESOLVE_MAIN_MENU_WND_PATH",
    "VALIDATE_MAIN_MENU_WND",
    "WINDOW_MANAGER_INIT",
    "LOAD_WINDOW_SCRIPT",
    "MATERIALISE_WINDOW_TREE",
    "RECORD_WINDOW_COUNT",
];

/// Runtime-host command residual names for MainMenu.wnd load peels.
pub const RUNTIME_HOST_MAIN_MENU_WND_LOAD_CMD_NAMES_WAVE161: &[&str] = &[
    "click_main_menu_wnd_ok_wnd_load",
    "click_main_menu_wnd_ok_wnd_prepare",
    "click_main_menu_wnd_ok_wnd_resolve",
    "click_main_menu_wnd_ok_wnd_validate",
    "click_main_menu_wnd_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: load method + script names residual pack.
pub fn honesty_main_menu_wnd_load_method_names_residual_wave161() -> bool {
    MAIN_MENU_WND_LOAD_METHOD_NAMES_WAVE161.len() == 5
        && residual_name_index(
            MAIN_MENU_WND_LOAD_METHOD_NAMES_WAVE161,
            "try_load_main_menu_via_window_manager",
        ) == Some(1)
        && residual_name_index(
            MAIN_MENU_WND_LOAD_METHOD_NAMES_WAVE161,
            "create_windows_from_script",
        ) == Some(3)
        && MAIN_MENU_WND_LOAD_SCRIPT_NAMES_WAVE161.len() == 3
        && residual_name_index(
            MAIN_MENU_WND_LOAD_SCRIPT_NAMES_WAVE161,
            "Menus/MainMenu.wnd",
        ) == Some(0)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_main_menu_wnd_load_nav_commands_residual_wave161() -> bool {
    MAIN_MENU_WND_LOAD_NAV_STEPS_WAVE161.len() == 6
        && residual_name_index(MAIN_MENU_WND_LOAD_NAV_STEPS_WAVE161, "WINDOW_MANAGER_INIT")
            == Some(2)
        && residual_name_index(
            MAIN_MENU_WND_LOAD_NAV_STEPS_WAVE161,
            "MATERIALISE_WINDOW_TREE",
        ) == Some(4)
        && residual_name_index(MAIN_MENU_WND_LOAD_NAV_STEPS_WAVE161, "RECORD_WINDOW_COUNT")
            == Some(5)
        && RUNTIME_HOST_MAIN_MENU_WND_LOAD_CMD_NAMES_WAVE161.len() == 5
        && residual_name_index(
            RUNTIME_HOST_MAIN_MENU_WND_LOAD_CMD_NAMES_WAVE161,
            "click_main_menu_wnd_ok_wnd_load",
        ) == Some(0)
}

/// Wave 161 composite residual honesty pack.
pub fn honesty_main_menu_wnd_load_residual_pack_wave161() -> bool {
    honesty_main_menu_wnd_load_method_names_residual_wave161()
        && honesty_main_menu_wnd_load_nav_commands_residual_wave161()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_main_menu_wnd_load_method_names_residual_wave161());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_main_menu_wnd_load_nav_commands_residual_wave161());
    }

    #[test]
    fn wave161_composite_pack() {
        assert!(honesty_main_menu_wnd_load_residual_pack_wave161());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_main_menu_wnd_prepare_load_honesty_residual_live() {
        use crate::gameplay_layout::{
            main_menu_wnd_honesty_with_load, simulate_main_menu_wnd_prepare_load_honesty,
        };
        assert!(
            simulate_main_menu_wnd_prepare_load_honesty(),
            "MainMenu.wnd load residual must stay green"
        );
        let h = main_menu_wnd_honesty_with_load(true);
        assert!(h.shell_residual_ok(), "{}", h.detail);
        if h.path_resolved && h.wnd_validated && h.window_loaded {
            assert!(h.window_count > 0, "materialised count: {}", h.detail);
        }
    }
}
