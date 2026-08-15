//! Wave 224 residual peels: force-complete construction + barracks
//! building_data stamping move from engine `get_object_mut` dual-mut into
//! GameLogic authority APIs (`force_complete_construction` /
//! `ensure_barracks_building_data`). Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 223 bootstrap-camera presentation-only residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `game_logic.rs` force_complete_construction / ensure_barracks_building_data
//! - `cnc_game_engine.rs` train producer force-complete path
//!
//! Fail-closed:
//! - Not full C++ Dozer construct completion parity
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Force-complete authority API residual method names.
pub const LIVE_FORCE_COMPLETE_AUTHORITY_API_METHOD_NAMES_WAVE224: &[&str] = &[
    "force_complete_construction",
    "ensure_barracks_building_data",
    "train_producer",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_FORCE_COMPLETE_AUTHORITY_API_NAV_STEPS_WAVE224: &[&str] = &[
    "REQUIRE_FORCE_COMPLETE_AUTHORITY_API",
    "REQUIRE_NO_ENGINE_GET_OBJECT_MUT",
    "LIVE_FORCE_COMPLETE_AUTHORITY_API",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_FORCE_COMPLETE_AUTHORITY_API_CMD_NAMES_WAVE224: &[&str] = &[
    "click_live_force_complete_authority_api_ok_prepare",
    "click_live_force_complete_authority_api_ok_live",
    "click_live_force_complete_authority_api_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_force_complete_authority_api_method_names_residual_wave224() -> bool {
    LIVE_FORCE_COMPLETE_AUTHORITY_API_METHOD_NAMES_WAVE224.len() == 4
        && residual_name_index(
            LIVE_FORCE_COMPLETE_AUTHORITY_API_METHOD_NAMES_WAVE224,
            "force_complete_construction",
        ) == Some(0)
        && residual_name_index(
            LIVE_FORCE_COMPLETE_AUTHORITY_API_METHOD_NAMES_WAVE224,
            "ensure_barracks_building_data",
        ) == Some(1)
        && residual_name_index(
            LIVE_FORCE_COMPLETE_AUTHORITY_API_METHOD_NAMES_WAVE224,
            "playable_claim = false",
        ) == Some(3)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_force_complete_authority_api_nav_commands_residual_wave224() -> bool {
    LIVE_FORCE_COMPLETE_AUTHORITY_API_NAV_STEPS_WAVE224.len() == 4
        && residual_name_index(
            LIVE_FORCE_COMPLETE_AUTHORITY_API_NAV_STEPS_WAVE224,
            "REQUIRE_FORCE_COMPLETE_AUTHORITY_API",
        ) == Some(0)
        && residual_name_index(
            LIVE_FORCE_COMPLETE_AUTHORITY_API_NAV_STEPS_WAVE224,
            "LIVE_FORCE_COMPLETE_AUTHORITY_API",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_FORCE_COMPLETE_AUTHORITY_API_CMD_NAMES_WAVE224.len() == 3
}

/// Wave 224 composite residual honesty pack.
pub fn honesty_live_force_complete_authority_api_residual_pack_wave224() -> bool {
    honesty_live_force_complete_authority_api_method_names_residual_wave224()
        && honesty_live_force_complete_authority_api_nav_commands_residual_wave224()
}

/// Source residual: GameLogic owns force-complete APIs; engine train path calls them.
pub fn honesty_force_complete_authority_api_source() -> bool {
    // 2026-08-15: scan host plus extra world_* splits.
    let gl = super::host_logic_scan_src();
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    gl.contains("pub fn force_complete_construction")
        && gl.contains("pub fn ensure_barracks_building_data")
        && gl.contains("Wave 224")
        && eng.contains("force_complete_construction(id)")
        && eng.contains("ensure_barracks_building_data(id)")
        && eng.contains("Wave 224: authority mutation via GameLogic API")
        // Train force-complete loop must not dual-mut via get_object_mut.
        && {
            let Some(i) = eng.find("for id in unfinished.into_iter().take(2)") else {
                return false;
            };
            let win = &eng[i..i + 450.min(eng.len() - i)];
            win.contains("force_complete_construction") && !win.contains("get_object_mut(id)")
        }
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_force_complete_authority_api_honesty() -> bool {
    honesty_live_force_complete_authority_api_residual_pack_wave224()
        && honesty_force_complete_authority_api_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_force_complete_authority_api_method_names_residual_wave224());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_force_complete_authority_api_nav_commands_residual_wave224());
    }

    #[test]
    fn wave224_composite_pack() {
        assert!(honesty_live_force_complete_authority_api_residual_pack_wave224());
    }

    #[test]
    fn force_complete_sources() {
        assert!(honesty_force_complete_authority_api_source());
    }

    #[test]
    fn simulate_live_force_complete_authority_api_honesty_residual_live() {
        assert!(
            simulate_live_force_complete_authority_api_honesty(),
            "force-complete authority API residual must latch"
        );
    }
}
