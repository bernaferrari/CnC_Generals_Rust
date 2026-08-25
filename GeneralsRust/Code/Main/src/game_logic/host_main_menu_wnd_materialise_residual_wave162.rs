//! Wave 162 residual peels: MainMenu.wnd materialisation residual
//! (require WindowManager window_count when assets resolve; never flips
//! shell `playable_claim`).
//!
//! Orthogonal to Wave 161 soft-ok load residual.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - Shell::push("Menus/MainMenu.wnd") materialises layout tree
//! - WindowManager window_count after load_window
//!
//! Fail-closed:
//! - Not full shell stack showShell residual
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
// Materialisation residual tables
// ---------------------------------------------------------------------------

/// Retail MainMenu.wnd named-child residual floor for materialisation honesty.
pub const MAIN_MENU_WND_MATERIALISE_NAMED_FLOOR_WAVE162: usize = 63;

/// Materialisation residual method names.
pub const MAIN_MENU_WND_MATERIALISE_METHOD_NAMES_WAVE162: &[&str] = &[
    "main_menu_wnd_honesty_with_load",
    "try_load_main_menu_via_window_manager",
    "window_loaded",
    "window_count",
    "simulate_main_menu_wnd_prepare_load_honesty",
];

/// Ordered materialisation residual navigation steps.
pub const MAIN_MENU_WND_MATERIALISE_NAV_STEPS_WAVE162: &[&str] = &[
    "RESOLVE_ASSETS",
    "VALIDATE_MAIN_MENU_WND",
    "LOAD_WINDOW_SCRIPT",
    "REQUIRE_WINDOW_LOADED",
    "REQUIRE_WINDOW_COUNT_GT_ZERO",
    "COMPARE_NAMED_CHILD_FLOOR",
];

/// Runtime-host command residual names for materialisation peels.
pub const RUNTIME_HOST_MAIN_MENU_WND_MATERIALISE_CMD_NAMES_WAVE162: &[&str] = &[
    "click_main_menu_wnd_ok_wnd_load",
    "click_main_menu_wnd_ok_wnd_materialise",
    "click_main_menu_wnd_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: materialisation method names residual pack.
pub fn honesty_main_menu_wnd_materialise_method_names_residual_wave162() -> bool {
    MAIN_MENU_WND_MATERIALISE_NAMED_FLOOR_WAVE162 == 63
        && MAIN_MENU_WND_MATERIALISE_METHOD_NAMES_WAVE162.len() == 5
        && residual_name_index(
            MAIN_MENU_WND_MATERIALISE_METHOD_NAMES_WAVE162,
            "window_loaded",
        ) == Some(2)
        && residual_name_index(
            MAIN_MENU_WND_MATERIALISE_METHOD_NAMES_WAVE162,
            "window_count",
        ) == Some(3)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_main_menu_wnd_materialise_nav_commands_residual_wave162() -> bool {
    MAIN_MENU_WND_MATERIALISE_NAV_STEPS_WAVE162.len() == 6
        && residual_name_index(
            MAIN_MENU_WND_MATERIALISE_NAV_STEPS_WAVE162,
            "REQUIRE_WINDOW_LOADED",
        ) == Some(3)
        && residual_name_index(
            MAIN_MENU_WND_MATERIALISE_NAV_STEPS_WAVE162,
            "REQUIRE_WINDOW_COUNT_GT_ZERO",
        ) == Some(4)
        && RUNTIME_HOST_MAIN_MENU_WND_MATERIALISE_CMD_NAMES_WAVE162.len() == 3
        && residual_name_index(
            RUNTIME_HOST_MAIN_MENU_WND_MATERIALISE_CMD_NAMES_WAVE162,
            "click_main_menu_wnd_ok_wnd_materialise",
        ) == Some(1)
}

/// Wave 162 composite residual honesty pack.
pub fn honesty_main_menu_wnd_materialise_residual_pack_wave162() -> bool {
    honesty_main_menu_wnd_materialise_method_names_residual_wave162()
        && honesty_main_menu_wnd_materialise_nav_commands_residual_wave162()
}

/// Residual: materialisation peel — require load when assets resolve.
pub fn simulate_main_menu_wnd_materialise_honesty() -> bool {
    use crate::gameplay_layout::{
        MAIN_MENU_WND_NAMED_COUNT_RESIDUAL, main_menu_wnd_honesty_with_load,
        simulate_main_menu_wnd_prepare_load_honesty,
    };
    if !simulate_main_menu_wnd_prepare_load_honesty() {
        return false;
    }
    let h = main_menu_wnd_honesty_with_load(true);
    if !h.path_resolved {
        return true;
    }
    h.window_loaded
        && h.window_count > 0
        && h.window_count >= MAIN_MENU_WND_NAMED_COUNT_RESIDUAL / 2
        && MAIN_MENU_WND_NAMED_COUNT_RESIDUAL == MAIN_MENU_WND_MATERIALISE_NAMED_FLOOR_WAVE162
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_main_menu_wnd_materialise_method_names_residual_wave162());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_main_menu_wnd_materialise_nav_commands_residual_wave162());
    }

    #[test]
    fn wave162_composite_pack() {
        assert!(honesty_main_menu_wnd_materialise_residual_pack_wave162());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_main_menu_wnd_materialise_honesty_residual_live() {
        use crate::gameplay_layout::main_menu_wnd_honesty_with_load;
        assert!(
            simulate_main_menu_wnd_materialise_honesty(),
            "MainMenu.wnd materialisation residual must latch"
        );
        let h = main_menu_wnd_honesty_with_load(true);
        if h.path_resolved {
            assert!(h.window_loaded);
            assert!(h.window_count >= MAIN_MENU_WND_MATERIALISE_NAMED_FLOOR_WAVE162 / 2);
        }
    }
}
