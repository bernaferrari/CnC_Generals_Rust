//! Wave 141 residual peels: ShellMap residual
//! (show/hide/toggle; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 118 MainMenu, shell layout residual packs.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - Shell::showShellMap / GameData ShellMapOn
//! - 3D shell background map enable/disable
//!
//! Fail-closed:
//! - Not full 3D shell map load / ClearGameData residual
//! - Not full initial_file skip branch residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// Shell map residual tables
// ---------------------------------------------------------------------------

/// Retail GameData shell map preference key residual.
pub const SHELL_MAP_GAMEDATA_KEY_WAVE141: &str = "ShellMapOn";

/// Retail default shell map name residual (GameData.ShellMapName).
pub const SHELL_MAP_DEFAULT_NAME_WAVE141: &str = "Maps\\ShellMap1\\ShellMap1.map";

/// Ordered ShellMap residual navigation steps.
pub const SHELL_MAP_NAV_STEPS_WAVE141: &[&str] = &[
    "READ_SHELL_MAP_ON_PREF",
    "SHOW_SHELL_MAP",
    "LOAD_SHELL_MAP_IF_ENABLED",
    "MENU_TRANSITION_KEEP_MAP",
    "HIDE_SHELL_MAP",
    "CLEAR_SHELL_MAP_IF_NEEDED",
];

/// Runtime-host command residual names for ShellMap peels.
pub const RUNTIME_HOST_SHELL_MAP_CMD_NAMES_WAVE141: &[&str] = &[
    "toggle_shell_map_ok_wnd",
    "toggle_shell_map_miss",
    "click_shell_map_ok_wnd_show",
    "click_shell_map_ok_wnd_hide",
    "click_shell_map_ok_wnd_toggle",
    "click_shell_map_ok_wnd_prepare_cycle",
    "click_shell_map_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: ShellMap preference/name residual pack.
pub fn honesty_shell_map_names_residual_wave141() -> bool {
    SHELL_MAP_GAMEDATA_KEY_WAVE141 == "ShellMapOn"
        && SHELL_MAP_DEFAULT_NAME_WAVE141 == "Maps\\ShellMap1\\ShellMap1.map"
        && SHELL_MAP_DEFAULT_NAME_WAVE141.starts_with("Maps\\")
        && SHELL_MAP_DEFAULT_NAME_WAVE141.ends_with(".map")
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_shell_map_nav_commands_residual_wave141() -> bool {
    SHELL_MAP_NAV_STEPS_WAVE141.len() == 6
        && residual_name_index(SHELL_MAP_NAV_STEPS_WAVE141, "SHOW_SHELL_MAP") == Some(1)
        && residual_name_index(SHELL_MAP_NAV_STEPS_WAVE141, "HIDE_SHELL_MAP") == Some(4)
        && residual_name_index(SHELL_MAP_NAV_STEPS_WAVE141, "CLEAR_SHELL_MAP_IF_NEEDED") == Some(5)
        && RUNTIME_HOST_SHELL_MAP_CMD_NAMES_WAVE141.len() == 7
        && residual_name_index(
            RUNTIME_HOST_SHELL_MAP_CMD_NAMES_WAVE141,
            "click_shell_map_ok_wnd_prepare_cycle",
        ) == Some(5)
}

/// Wave 141 composite residual honesty pack.
pub fn honesty_shell_map_residual_pack_wave141() -> bool {
    honesty_shell_map_names_residual_wave141() && honesty_shell_map_nav_commands_residual_wave141()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_residual() {
        assert!(honesty_shell_map_names_residual_wave141());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_shell_map_nav_commands_residual_wave141());
    }

    #[test]
    fn wave141_composite_pack() {
        assert!(honesty_shell_map_residual_pack_wave141());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_shell_map_prepare_cycle_residual_live() {
        use game_client::gui::{
            ResidualShellMapAction, residual_shell_map_is_on, residual_shell_map_last_action,
            simulate_shell_map_prepare_cycle, simulate_shell_map_show,
        };
        assert!(simulate_shell_map_show());
        assert!(residual_shell_map_is_on());
        assert_eq!(
            residual_shell_map_last_action(),
            ResidualShellMapAction::Show
        );
        assert!(
            simulate_shell_map_prepare_cycle(),
            "show+hide residual must latch"
        );
        assert!(!residual_shell_map_is_on());
        assert_eq!(
            residual_shell_map_last_action(),
            ResidualShellMapAction::Hide
        );
    }
}
