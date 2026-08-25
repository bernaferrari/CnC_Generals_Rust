//! Wave 127 residual peels: CreditsMenu WND residual
//! (bind/skip/finished/shutdown; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 118 ButtonCredits, Wave 126 OptionsMenu.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - CreditsMenu.cpp / CreditsMenu.wnd
//! - ParentCreditsWindow, ESC skip, auto-pop on finished
//!
//! Fail-closed:
//! - Not full Credits.ini scroll/render residual
//! - Not full credits music audio residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// Credits menu residual tables
// ---------------------------------------------------------------------------

/// Retail CreditsMenu layout filename residual.
pub const CREDITS_MENU_LAYOUT_FILENAME_WAVE127: &str = "Menus/CreditsMenu.wnd";

/// Retail CreditsMenu control names residual.
pub const CREDITS_MENU_CONTROL_NAMES_WAVE127: &[&str] = &["CreditsMenu.wnd:ParentCreditsWindow"];

/// Ordered CreditsMenu residual navigation steps.
pub const CREDITS_MENU_NAV_STEPS_WAVE127: &[&str] = &[
    "PUSH_CREDITS_MENU_LAYOUT",
    "LOAD_CREDITS_INI",
    "START_CREDITS_MUSIC",
    "SCROLL_UPDATE",
    "ESC_SKIP_OR_FINISHED",
    "SHELL_POP",
    "SHUTDOWN_RESET_AUDIO",
];

/// Runtime-host command residual names for CreditsMenu peels.
pub const RUNTIME_HOST_CREDITS_MENU_CMD_NAMES_WAVE127: &[&str] = &[
    "open_credits_ok_wnd",
    "open_credits_ok",
    "click_credits_menu_ok_wnd_skip",
    "click_credits_menu_ok_wnd_finished",
    "click_credits_menu_ok_wnd_shutdown",
    "click_credits_menu_ok_wnd_prepare_skip",
    "click_credits_menu_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: CreditsMenu control names residual pack.
pub fn honesty_credits_menu_control_names_residual_wave127() -> bool {
    CREDITS_MENU_LAYOUT_FILENAME_WAVE127 == "Menus/CreditsMenu.wnd"
        && CREDITS_MENU_CONTROL_NAMES_WAVE127.len() == 1
        && residual_name_index(
            CREDITS_MENU_CONTROL_NAMES_WAVE127,
            "CreditsMenu.wnd:ParentCreditsWindow",
        ) == Some(0)
        && CREDITS_MENU_CONTROL_NAMES_WAVE127
            .iter()
            .all(|n| n.starts_with("CreditsMenu.wnd:"))
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_credits_menu_nav_commands_residual_wave127() -> bool {
    CREDITS_MENU_NAV_STEPS_WAVE127.len() == 7
        && residual_name_index(CREDITS_MENU_NAV_STEPS_WAVE127, "LOAD_CREDITS_INI") == Some(1)
        && residual_name_index(CREDITS_MENU_NAV_STEPS_WAVE127, "ESC_SKIP_OR_FINISHED") == Some(4)
        && residual_name_index(CREDITS_MENU_NAV_STEPS_WAVE127, "SHELL_POP") == Some(5)
        && RUNTIME_HOST_CREDITS_MENU_CMD_NAMES_WAVE127.len() == 7
        && residual_name_index(
            RUNTIME_HOST_CREDITS_MENU_CMD_NAMES_WAVE127,
            "open_credits_ok_wnd",
        ) == Some(0)
        && residual_name_index(
            RUNTIME_HOST_CREDITS_MENU_CMD_NAMES_WAVE127,
            "click_credits_menu_ok_wnd_skip",
        ) == Some(2)
}

/// Wave 127 composite residual honesty pack.
pub fn honesty_credits_menu_residual_pack_wave127() -> bool {
    honesty_credits_menu_control_names_residual_wave127()
        && honesty_credits_menu_nav_commands_residual_wave127()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_names_residual() {
        assert!(honesty_credits_menu_control_names_residual_wave127());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_credits_menu_nav_commands_residual_wave127());
    }

    #[test]
    fn wave127_composite_pack() {
        assert!(honesty_credits_menu_residual_pack_wave127());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_credits_prepare_skip_residual_live() {
        use game_client::gui::callbacks::{
            ResidualCreditsMenuAction, residual_credits_menu_is_active,
            residual_credits_menu_last_action, simulate_credits_menu_bind_controls,
            simulate_credits_menu_prepare_skip,
        };
        assert!(simulate_credits_menu_bind_controls());
        assert!(residual_credits_menu_is_active());
        assert_eq!(
            residual_credits_menu_last_action(),
            ResidualCreditsMenuAction::Bind
        );
        assert!(
            simulate_credits_menu_prepare_skip(),
            "bind+skip residual must latch"
        );
        assert!(!residual_credits_menu_is_active());
        assert_eq!(
            residual_credits_menu_last_action(),
            ResidualCreditsMenuAction::Skip
        );
    }
}
