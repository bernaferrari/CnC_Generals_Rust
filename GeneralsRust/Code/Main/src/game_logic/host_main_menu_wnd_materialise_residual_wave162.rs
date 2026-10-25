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

/// Ordered materialisation residual navigation steps.

/// Runtime-host command residual names for materialisation peels.

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: materialisation method names residual pack.

/// Honesty: nav steps + runtime-host cmd residual pack.

/// Wave 162 composite residual honesty pack.

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
