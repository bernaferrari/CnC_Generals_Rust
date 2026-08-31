//! Wave 188 residual peels: executable InGame GameWorld presentation residual
//! (status `gameworld_presentation_entities`; vertical slice requires GW observe
//! path when render alives > 0; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 187 PresentationFrame overlay residual.
//! Host residual only — network deferred.
//!
//! Sources (architecture migration):
//! - engine status exports `gameworld_presentation_entities`
//! - executable_smoke tracks max GW presentation entities
//! - host_vertical_slice_ok gates GW presentation when InGame has alives
//!
//! Fail-closed:
//! - Does not boot the real executable inside shell_smoke
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Executable GameWorld presentation residual method names.
pub const EXECUTABLE_GAMEWORLD_PRESENTATION_METHOD_NAMES_WAVE188: &[&str] = &[
    "gameworld_presentation_entities",
    "gameworld_presentation_entities_ok",
    "max_gameworld_presentation_entities",
    "gameworld_presentation_boundary_ok",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const EXECUTABLE_GAMEWORLD_PRESENTATION_NAV_STEPS_WAVE188: &[&str] = &[
    "REQUIRE_STATUS_EXPORTS_GW_ENTS",
    "REQUIRE_EXECUTABLE_TRACKS_GW_ENTS",
    "REQUIRE_VERTICAL_GATES_GW_VIEW",
    "LIVE_SOURCE_MARKERS",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_EXECUTABLE_GAMEWORLD_PRESENTATION_CMD_NAMES_WAVE188: &[&str] = &[
    "click_executable_gameworld_presentation_ok_status",
    "click_executable_gameworld_presentation_ok_vertical",
    "click_executable_gameworld_presentation_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_executable_gameworld_presentation_method_names_residual_wave188() -> bool {
    EXECUTABLE_GAMEWORLD_PRESENTATION_METHOD_NAMES_WAVE188.len() == 5
        && residual_name_index(
            EXECUTABLE_GAMEWORLD_PRESENTATION_METHOD_NAMES_WAVE188,
            "gameworld_presentation_entities",
        ) == Some(0)
        && residual_name_index(
            EXECUTABLE_GAMEWORLD_PRESENTATION_METHOD_NAMES_WAVE188,
            "gameworld_presentation_boundary_ok",
        ) == Some(3)
        && residual_name_index(
            EXECUTABLE_GAMEWORLD_PRESENTATION_METHOD_NAMES_WAVE188,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_executable_gameworld_presentation_nav_commands_residual_wave188() -> bool {
    EXECUTABLE_GAMEWORLD_PRESENTATION_NAV_STEPS_WAVE188.len() == 5
        && residual_name_index(
            EXECUTABLE_GAMEWORLD_PRESENTATION_NAV_STEPS_WAVE188,
            "REQUIRE_STATUS_EXPORTS_GW_ENTS",
        ) == Some(0)
        && residual_name_index(
            EXECUTABLE_GAMEWORLD_PRESENTATION_NAV_STEPS_WAVE188,
            "LIVE_SOURCE_MARKERS",
        ) == Some(3)
        && RUNTIME_HOST_EXECUTABLE_GAMEWORLD_PRESENTATION_CMD_NAMES_WAVE188.len() == 3
}

/// Wave 188 composite residual honesty pack.
pub fn honesty_executable_gameworld_presentation_residual_pack_wave188() -> bool {
    honesty_executable_gameworld_presentation_method_names_residual_wave188()
        && honesty_executable_gameworld_presentation_nav_commands_residual_wave188()
}

/// Source residual: engine status exports gameworld_presentation_entities.
pub fn honesty_engine_status_exports_gw_presentation_entities_source() -> bool {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    eng.contains("gameworld_presentation_entities")
        && eng.contains("last_gameworld_presentation_entity_count")
        && eng.contains("presentation_view_from_shadow")
}

/// Source residual: executable smoke tracks and gates GW presentation entities.
pub fn honesty_executable_tracks_gw_presentation_entities_source() -> bool {
    let src = crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC;
    src.contains("gameworld_presentation_entities_ok")
        && src.contains("max_gameworld_presentation_entities")
        && src.contains("gameworld_presentation_boundary_ok")
        && src.contains("\"gameworld_presentation_entities\"")
        && src.contains("host_vertical_slice_ok")
}

/// Live residual: source honesty only (full executable owned by executable_smoke/behavior_gate).
pub fn simulate_executable_gameworld_presentation_honesty() -> bool {
    if !honesty_executable_gameworld_presentation_residual_pack_wave188() {
        return false;
    }
    if !honesty_engine_status_exports_gw_presentation_entities_source() {
        return false;
    }
    if !honesty_executable_tracks_gw_presentation_entities_source() {
        return false;
    }
    let src = crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC;
    if !(src.contains("playable_claim: false") || src.contains("playable_claim = false")) {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_executable_gameworld_presentation_method_names_residual_wave188());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_executable_gameworld_presentation_nav_commands_residual_wave188());
    }

    #[test]
    fn wave188_composite_pack() {
        assert!(honesty_executable_gameworld_presentation_residual_pack_wave188());
    }

    #[test]
    fn executable_gw_presentation_sources() {
        assert!(honesty_engine_status_exports_gw_presentation_entities_source());
        assert!(honesty_executable_tracks_gw_presentation_entities_source());
    }

    #[test]
    fn simulate_executable_gameworld_presentation_honesty_residual_live() {
        assert!(
            simulate_executable_gameworld_presentation_honesty(),
            "executable GameWorld presentation entities residual must latch"
        );
    }
}
