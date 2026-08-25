//! Wave 156 residual peels: ControlBarScheme residual
//! (load/get/clear 8x6 schemes; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 155 MainMenu layout residual.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - ControlBarScheme.ini 8x6 scheme names
//! - ControlBarSchemeManager load/get/set
//!
//! Fail-closed:
//! - Not full INI image/animation parse residual
//! - Not full control-bar WND re-skin residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// ControlBarScheme residual tables
// ---------------------------------------------------------------------------

/// Retail ControlBarScheme.ini 8x6 scheme count residual.
pub const CONTROL_BAR_SCHEME_8X6_COUNT_WAVE156: usize = 14;

/// Retail ControlBarScheme.ini 8x6 names residual.
pub const CONTROL_BAR_SCHEME_NAMES_8X6_WAVE156: &[&str] = &[
    "America8x6",
    "GLA8x6",
    "China8x6",
    "Observer8x6",
    "AmericaSuperWeaponGeneral8x6",
    "AmericaLaserGeneral8x6",
    "AmericaAirForceGeneral8x6",
    "ChinaTankGeneral8x6",
    "ChinaInfantryGeneral8x6",
    "ChinaNukeGeneral8x6",
    "GLAToxinGeneral8x6",
    "GLADemolitionGeneral8x6",
    "GLAStealthGeneral8x6",
    "ChinaBossGeneral8x6",
];

/// ControlBarSchemeManager method residual names.
pub const CONTROL_BAR_SCHEME_METHOD_NAMES_WAVE156: &[&str] =
    &["loadScheme", "getScheme", "setScheme"];

/// Ordered ControlBarScheme residual navigation steps.
pub const CONTROL_BAR_SCHEME_NAV_STEPS_WAVE156: &[&str] = &[
    "CLEAR_LOADED_SCHEMES",
    "LOAD_AMERICA_8X6",
    "LOAD_OBSERVER_8X6",
    "GET_CURRENT_SCHEME",
    "APPLY_SCHEME_IMAGES",
    "RESIZE_CONTROL_BAR",
];

/// Runtime-host command residual names for ControlBarScheme peels.
pub const RUNTIME_HOST_CONTROL_BAR_SCHEME_CMD_NAMES_WAVE156: &[&str] = &[
    "click_control_bar_scheme_ok_wnd_load",
    "click_control_bar_scheme_ok_wnd_get",
    "click_control_bar_scheme_ok_wnd_clear",
    "click_control_bar_scheme_ok_wnd_prepare",
    "click_control_bar_scheme_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: ControlBarScheme 8x6 names residual pack.
pub fn honesty_control_bar_scheme_names_residual_wave156() -> bool {
    CONTROL_BAR_SCHEME_8X6_COUNT_WAVE156 == 14
        && CONTROL_BAR_SCHEME_NAMES_8X6_WAVE156.len() == 14
        && residual_name_index(CONTROL_BAR_SCHEME_NAMES_8X6_WAVE156, "America8x6") == Some(0)
        && residual_name_index(CONTROL_BAR_SCHEME_NAMES_8X6_WAVE156, "Observer8x6") == Some(3)
        && residual_name_index(CONTROL_BAR_SCHEME_NAMES_8X6_WAVE156, "ChinaBossGeneral8x6")
            == Some(13)
        && CONTROL_BAR_SCHEME_NAMES_8X6_WAVE156
            .iter()
            .all(|n| n.ends_with("8x6"))
}

/// Honesty: method names residual pack.
pub fn honesty_control_bar_scheme_method_names_residual_wave156() -> bool {
    CONTROL_BAR_SCHEME_METHOD_NAMES_WAVE156.len() == 3
        && residual_name_index(CONTROL_BAR_SCHEME_METHOD_NAMES_WAVE156, "loadScheme") == Some(0)
        && residual_name_index(CONTROL_BAR_SCHEME_METHOD_NAMES_WAVE156, "setScheme") == Some(2)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_control_bar_scheme_nav_commands_residual_wave156() -> bool {
    CONTROL_BAR_SCHEME_NAV_STEPS_WAVE156.len() == 6
        && residual_name_index(CONTROL_BAR_SCHEME_NAV_STEPS_WAVE156, "LOAD_AMERICA_8X6") == Some(1)
        && residual_name_index(CONTROL_BAR_SCHEME_NAV_STEPS_WAVE156, "GET_CURRENT_SCHEME")
            == Some(3)
        && residual_name_index(CONTROL_BAR_SCHEME_NAV_STEPS_WAVE156, "RESIZE_CONTROL_BAR")
            == Some(5)
        && RUNTIME_HOST_CONTROL_BAR_SCHEME_CMD_NAMES_WAVE156.len() == 5
        && residual_name_index(
            RUNTIME_HOST_CONTROL_BAR_SCHEME_CMD_NAMES_WAVE156,
            "click_control_bar_scheme_ok_wnd_prepare",
        ) == Some(3)
}

/// Wave 156 composite residual honesty pack.
pub fn honesty_control_bar_scheme_residual_pack_wave156() -> bool {
    honesty_control_bar_scheme_names_residual_wave156()
        && honesty_control_bar_scheme_method_names_residual_wave156()
        && honesty_control_bar_scheme_nav_commands_residual_wave156()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_residual() {
        assert!(honesty_control_bar_scheme_names_residual_wave156());
    }

    #[test]
    fn method_names_residual() {
        assert!(honesty_control_bar_scheme_method_names_residual_wave156());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_control_bar_scheme_nav_commands_residual_wave156());
    }

    #[test]
    fn wave156_composite_pack() {
        assert!(honesty_control_bar_scheme_residual_pack_wave156());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_control_bar_scheme_prepare_default_residual_live() {
        use game_client::gui::control_bar::{
            CONTROL_BAR_SCHEME_NAMES_8X6, ResidualControlBarSchemeAction,
            residual_control_bar_scheme_has_current, residual_control_bar_scheme_last_action,
            residual_control_bar_scheme_loaded_count, simulate_control_bar_scheme_prepare_default,
        };
        assert_eq!(
            CONTROL_BAR_SCHEME_NAMES_8X6,
            CONTROL_BAR_SCHEME_NAMES_8X6_WAVE156
        );
        assert!(
            simulate_control_bar_scheme_prepare_default(),
            "load America+Observer residual must latch"
        );
        assert!(residual_control_bar_scheme_has_current());
        assert!(residual_control_bar_scheme_loaded_count() >= 2);
        assert_eq!(
            residual_control_bar_scheme_last_action(),
            ResidualControlBarSchemeAction::Get
        );
    }
}
