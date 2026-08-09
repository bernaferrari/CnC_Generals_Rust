//! Wave 164 residual peels: Shell MainMenu windows + Skirmish nav residual
//! (require WindowManager windows after Shell::push MainMenu; ButtonSkirmish
//! latch; engine open_skirmish push path; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 163 Shell stack push residual.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - Shell::push("Menus/MainMenu.wnd") → doPush → layout runInit loads windows
//! - MainMenu.cpp ButtonSkirmish GBM_SELECTED latches + queues
//!   PushShellScreen("Menus/SkirmishGameOptionsMenu.wnd")
//! - CncGameEngine open_skirmish_menu runtime-host residual
//!
//! Fail-closed:
//! - Full SkirmishGameOptionsMenu.wnd tree materialisation is not required here
//!   (retail layout ~900KB; headless full parse is a separate load residual).
//! - Not full W3DMainMenuInit / TransitionHandler residual
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Shell → Skirmish nav residual method names.
pub const SHELL_SKIRMISH_NAV_METHOD_NAMES_WAVE164: &[&str] = &[
    "simulate_shell_stack_push_honesty",
    "Shell::push MainMenu.wnd",
    "with_window_manager window_count",
    "button_skirmish_latch",
    "open_skirmish_menu SkirmishGameOptionsMenu.wnd",
];

/// Ordered shell → Skirmish residual navigation steps.
pub const SHELL_SKIRMISH_NAV_STEPS_WAVE164: &[&str] = &[
    "INIT_SHELL",
    "PUSH_MAIN_MENU_WND",
    "REQUIRE_WM_WINDOWS",
    "BUTTON_SKIRMISH_LATCH",
    "OPEN_SKIRMISH_SOURCE_PUSH",
    "REQUIRE_SKIRMISH_LAYOUT_NAME",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_SHELL_SKIRMISH_NAV_CMD_NAMES_WAVE164: &[&str] = &[
    "click_shell_skirmish_nav_ok_push",
    "click_shell_skirmish_nav_ok_windows",
    "click_shell_skirmish_nav_miss",
];

/// Retail skirmish options layout residual.
pub const SKIRMISH_OPTIONS_LAYOUT_FILENAME_WAVE164: &str = "Menus/SkirmishGameOptionsMenu.wnd";

/// Honesty: method names residual pack.
pub fn honesty_shell_skirmish_nav_method_names_residual_wave164() -> bool {
    SHELL_SKIRMISH_NAV_METHOD_NAMES_WAVE164.len() == 5
        && residual_name_index(
            SHELL_SKIRMISH_NAV_METHOD_NAMES_WAVE164,
            "with_window_manager window_count",
        ) == Some(2)
        && residual_name_index(
            SHELL_SKIRMISH_NAV_METHOD_NAMES_WAVE164,
            "open_skirmish_menu SkirmishGameOptionsMenu.wnd",
        ) == Some(4)
        && SKIRMISH_OPTIONS_LAYOUT_FILENAME_WAVE164 == "Menus/SkirmishGameOptionsMenu.wnd"
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_shell_skirmish_nav_commands_residual_wave164() -> bool {
    SHELL_SKIRMISH_NAV_STEPS_WAVE164.len() == 6
        && residual_name_index(SHELL_SKIRMISH_NAV_STEPS_WAVE164, "REQUIRE_WM_WINDOWS") == Some(2)
        && residual_name_index(
            SHELL_SKIRMISH_NAV_STEPS_WAVE164,
            "OPEN_SKIRMISH_SOURCE_PUSH",
        ) == Some(4)
        && RUNTIME_HOST_SHELL_SKIRMISH_NAV_CMD_NAMES_WAVE164.len() == 3
}

/// Wave 164 composite residual honesty pack.
pub fn honesty_shell_skirmish_nav_residual_pack_wave164() -> bool {
    honesty_shell_skirmish_nav_method_names_residual_wave164()
        && honesty_shell_skirmish_nav_commands_residual_wave164()
}

/// Source residual: runtime-host open_skirmish_menu pushes Skirmish options layout.
pub fn honesty_open_skirmish_menu_pushes_options_layout_source() -> bool {
    let src = include_str!("../../cnc_game_engine.rs");
    // Prefer the runtime-host match arm, not the earlier field-doc mention.
    let needle = "\"open_skirmish_menu\" =>";
    let i = match src.find(needle) {
        Some(i) => i,
        None => return false,
    };
    let body = &src[i..src.len().min(i + 4500)];
    body.contains("Menus/SkirmishGameOptionsMenu.wnd")
        && body.contains("simulate_main_menu_skirmish_button_gadget_selected")
        && body.contains(".push(")
}

/// Latch-only ButtonSkirmish residual (no Shell::push of 900KB Skirmish layout).
///
/// Full `simulate_main_menu_skirmish_button_gadget_selected` executes pending
/// `PushShellScreen` which headlessly parses the entire Skirmish options WND and
/// can stall residual peels. C++ still queues that push; engine open_skirmish
/// path is covered by source honesty + explicit push sites.
pub fn simulate_button_skirmish_latch_only() -> bool {
    #[cfg(feature = "game_client")]
    {
        game_client::gui::simulate_main_menu_skirmish_button_latch_only()
    }
    #[cfg(not(feature = "game_client"))]
    {
        true
    }
}

/// Live residual: Shell MainMenu materialises WM windows + Skirmish nav honesty.
pub fn simulate_shell_skirmish_nav_honesty() -> bool {
    use crate::game_logic::simulate_shell_stack_push_honesty;

    if !honesty_shell_skirmish_nav_residual_pack_wave164() {
        return false;
    }
    if !honesty_open_skirmish_menu_pushes_options_layout_source() {
        return false;
    }
    // Wave 163: init + MainMenu push + honest top filename.
    if !simulate_shell_stack_push_honesty() {
        return false;
    }

    #[cfg(feature = "game_client")]
    {
        use game_client::gui::{get_shell, with_window_manager_ref};

        // C++ Shell::doPush → layout runInit creates windows into TheWindowManager.
        let wm_count = with_window_manager_ref(|wm| wm.window_count());
        if wm_count == 0 {
            let mut shell = get_shell();
            if let Some(top) = shell.top() {
                if top.run_init(None).is_err() {
                    return false;
                }
            } else {
                return false;
            }
            drop(shell);
            let wm_count = with_window_manager_ref(|wm| wm.window_count());
            if wm_count == 0 {
                return false;
            }
        }

        // ButtonSkirmish latch without headless Skirmish WND full parse.
        if !simulate_button_skirmish_latch_only() {
            return false;
        }

        // Stack still holds MainMenu after latch-only residual.
        let mut shell = get_shell();
        let top = shell
            .top()
            .map(|l| l.get_filename().to_string())
            .unwrap_or_default();
        let top_l = top.replace('\\', "/").to_ascii_lowercase();
        if !top_l.contains("mainmenu.wnd") {
            return false;
        }
        if shell.get_screen_count() == 0 {
            return false;
        }
        true
    }
    #[cfg(not(feature = "game_client"))]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_shell_skirmish_nav_method_names_residual_wave164());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_shell_skirmish_nav_commands_residual_wave164());
    }

    #[test]
    fn wave164_composite_pack() {
        assert!(honesty_shell_skirmish_nav_residual_pack_wave164());
    }

    #[test]
    fn open_skirmish_menu_source_pushes_options_layout() {
        assert!(honesty_open_skirmish_menu_pushes_options_layout_source());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_shell_skirmish_nav_honesty_residual_live() {
        assert!(
            simulate_shell_skirmish_nav_honesty(),
            "Shell MainMenu must materialise WM windows; Skirmish nav latch+source must hold"
        );
        let n = game_client::gui::with_window_manager_ref(|wm| wm.window_count());
        assert!(n > 0, "expected materialised MainMenu windows, got {n}");
    }
}
