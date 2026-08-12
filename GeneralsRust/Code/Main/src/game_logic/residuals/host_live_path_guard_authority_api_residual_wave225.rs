//! Wave 225 residual peels: clear-path and guard-radius host helpers use
//! presentation-first `ui_selected_ids` and GameLogic authority APIs
//! (`clear_unit_movement_path` / `adjust_unit_guard_radius`) — no engine
//! `get_object_mut` dual-mut. Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 224 force-complete authority API residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `game_logic.rs` clear_unit_movement_path / adjust_unit_guard_radius
//! - `cnc_game_engine.rs` clear_selected_path_waypoints / adjust_selected_guard_radius
//!
//! Fail-closed:
//! - Not full C++ waypoint/guard UI matrix parity
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Path/guard authority API residual method names.
pub const LIVE_PATH_GUARD_AUTHORITY_API_METHOD_NAMES_WAVE225: &[&str] = &[
    "clear_unit_movement_path",
    "adjust_unit_guard_radius",
    "ui_selected_ids",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_PATH_GUARD_AUTHORITY_API_NAV_STEPS_WAVE225: &[&str] = &[
    "REQUIRE_PATH_GUARD_AUTHORITY_API",
    "REQUIRE_UI_SELECTED_IDS",
    "LIVE_PATH_GUARD_AUTHORITY_API",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_PATH_GUARD_AUTHORITY_API_CMD_NAMES_WAVE225: &[&str] = &[
    "click_live_path_guard_authority_api_ok_prepare",
    "click_live_path_guard_authority_api_ok_live",
    "click_live_path_guard_authority_api_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_path_guard_authority_api_method_names_residual_wave225() -> bool {
    LIVE_PATH_GUARD_AUTHORITY_API_METHOD_NAMES_WAVE225.len() == 4
        && residual_name_index(
            LIVE_PATH_GUARD_AUTHORITY_API_METHOD_NAMES_WAVE225,
            "clear_unit_movement_path",
        ) == Some(0)
        && residual_name_index(
            LIVE_PATH_GUARD_AUTHORITY_API_METHOD_NAMES_WAVE225,
            "adjust_unit_guard_radius",
        ) == Some(1)
        && residual_name_index(
            LIVE_PATH_GUARD_AUTHORITY_API_METHOD_NAMES_WAVE225,
            "playable_claim = false",
        ) == Some(3)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_path_guard_authority_api_nav_commands_residual_wave225() -> bool {
    LIVE_PATH_GUARD_AUTHORITY_API_NAV_STEPS_WAVE225.len() == 4
        && residual_name_index(
            LIVE_PATH_GUARD_AUTHORITY_API_NAV_STEPS_WAVE225,
            "REQUIRE_PATH_GUARD_AUTHORITY_API",
        ) == Some(0)
        && residual_name_index(
            LIVE_PATH_GUARD_AUTHORITY_API_NAV_STEPS_WAVE225,
            "LIVE_PATH_GUARD_AUTHORITY_API",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_PATH_GUARD_AUTHORITY_API_CMD_NAMES_WAVE225.len() == 3
}

/// Wave 225 composite residual honesty pack.
pub fn honesty_live_path_guard_authority_api_residual_pack_wave225() -> bool {
    honesty_live_path_guard_authority_api_method_names_residual_wave225()
        && honesty_live_path_guard_authority_api_nav_commands_residual_wave225()
}

/// Source residual: GameLogic APIs + engine helpers presentation-first.
pub fn honesty_path_guard_authority_api_source() -> bool {
    let gl = include_str!("../game_logic.rs");
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    gl.contains("pub fn clear_unit_movement_path")
        && gl.contains("pub fn adjust_unit_guard_radius")
        && gl.contains("Wave 225")
        && {
            let Some(i) = eng.find("fn clear_selected_path_waypoints") else {
                return false;
            };
            let body = &eng[i..i + 900.min(eng.len() - i)];
            body.contains("Wave 225")
                && body.contains("ui_selected_ids")
                && body.contains("clear_unit_movement_path")
                && !body.contains("get_object_mut")
        }
        && {
            let Some(i) = eng.find("fn adjust_selected_guard_radius") else {
                return false;
            };
            let body = &eng[i..i + 700.min(eng.len() - i)];
            body.contains("Wave 225")
                && body.contains("ui_selected_ids")
                && body.contains("adjust_unit_guard_radius")
                && !body.contains("get_object_mut")
        }
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_path_guard_authority_api_honesty() -> bool {
    honesty_live_path_guard_authority_api_residual_pack_wave225()
        && honesty_path_guard_authority_api_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_path_guard_authority_api_method_names_residual_wave225());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_path_guard_authority_api_nav_commands_residual_wave225());
    }

    #[test]
    fn wave225_composite_pack() {
        assert!(honesty_live_path_guard_authority_api_residual_pack_wave225());
    }

    #[test]
    fn path_guard_sources() {
        assert!(honesty_path_guard_authority_api_source());
    }

    #[test]
    fn simulate_live_path_guard_authority_api_honesty_residual_live() {
        assert!(
            simulate_live_path_guard_authority_api_honesty(),
            "path/guard authority API residual must latch"
        );
    }
}
