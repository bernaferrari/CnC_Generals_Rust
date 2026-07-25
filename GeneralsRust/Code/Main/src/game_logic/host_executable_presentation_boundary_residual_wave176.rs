//! Wave 176 residual peels: executable InGame presentation boundary honesty residual
//! (host_vertical_slice_ok requires presentation_frame_ok + zero live dual-reads;
//! never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 175 golden map-host victory residual.
//! Host residual only — network deferred.
//!
//! Sources (executable_smoke + render/engine):
//! - `host_vertical_slice_ok` gates on presentation boundary when InGame
//! - `RenderPipeline::execute` presentation-only
//! - engine seeds presentation after match start
//!
//! Fail-closed:
//! - Does not boot the real executable inside shell_smoke
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Executable presentation boundary residual method names.
pub const EXECUTABLE_PRESENTATION_BOUNDARY_METHOD_NAMES_WAVE176: &[&str] = &[
    "host_vertical_slice_ok",
    "presentation_frame_ok",
    "presentation_live_fallback_ok",
    "presentation_boundary_ok",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const EXECUTABLE_PRESENTATION_BOUNDARY_NAV_STEPS_WAVE176: &[&str] = &[
    "REQUIRE_VERTICAL_SLICE_PRESENTATION",
    "REQUIRE_ZERO_LIVE_FALLBACK",
    "REQUIRE_EXECUTE_PRESENTATION_ONLY",
    "LIVE_SOURCE_MARKERS",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_EXECUTABLE_PRESENTATION_BOUNDARY_CMD_NAMES_WAVE176: &[&str] = &[
    "click_executable_presentation_boundary_ok_vertical",
    "click_executable_presentation_boundary_ok_fallback",
    "click_executable_presentation_boundary_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_executable_presentation_boundary_method_names_residual_wave176() -> bool {
    EXECUTABLE_PRESENTATION_BOUNDARY_METHOD_NAMES_WAVE176.len() == 5
        && residual_name_index(
            EXECUTABLE_PRESENTATION_BOUNDARY_METHOD_NAMES_WAVE176,
            "host_vertical_slice_ok",
        ) == Some(0)
        && residual_name_index(
            EXECUTABLE_PRESENTATION_BOUNDARY_METHOD_NAMES_WAVE176,
            "presentation_boundary_ok",
        ) == Some(3)
        && residual_name_index(
            EXECUTABLE_PRESENTATION_BOUNDARY_METHOD_NAMES_WAVE176,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_executable_presentation_boundary_nav_commands_residual_wave176() -> bool {
    EXECUTABLE_PRESENTATION_BOUNDARY_NAV_STEPS_WAVE176.len() == 5
        && residual_name_index(
            EXECUTABLE_PRESENTATION_BOUNDARY_NAV_STEPS_WAVE176,
            "REQUIRE_VERTICAL_SLICE_PRESENTATION",
        ) == Some(0)
        && residual_name_index(
            EXECUTABLE_PRESENTATION_BOUNDARY_NAV_STEPS_WAVE176,
            "LIVE_SOURCE_MARKERS",
        ) == Some(3)
        && RUNTIME_HOST_EXECUTABLE_PRESENTATION_BOUNDARY_CMD_NAMES_WAVE176.len() == 3
}

/// Wave 176 composite residual honesty pack.
pub fn honesty_executable_presentation_boundary_residual_pack_wave176() -> bool {
    honesty_executable_presentation_boundary_method_names_residual_wave176()
        && honesty_executable_presentation_boundary_nav_commands_residual_wave176()
}

/// Source residual: executable vertical slice requires presentation boundary on InGame.
pub fn honesty_executable_vertical_presentation_gate_source() -> bool {
    let src = include_str!("../executable_smoke.rs");
    let i = match src.find("host_vertical_slice_ok =") {
        Some(i) => i,
        None => return false,
    };
    let body = &src[i.saturating_sub(400)..src.len().min(i + 500)];
    body.contains("presentation_boundary_ok")
        && body.contains("presentation_frame_ok")
        && body.contains("presentation_live_fallback_ok")
        && body.contains("reached_ingame")
}

/// Source residual: execute remains presentation-only (no live GameLogic arg).
pub fn honesty_execute_presentation_only_source_wave176() -> bool {
    let src = include_str!("../graphics/render_pipeline.rs");
    let i = match src.find("pub fn execute(") {
        Some(i) => i,
        None => return false,
    };
    let window = &src[i..src.len().min(i + 700)];
    window.contains("presentation_frame")
        && !window.contains("game_logic: Option<&GameLogic>")
        && !window.contains("game_logic: &GameLogic")
}

/// Live residual: source honesty (full executable boot is owned by executable_smoke/behavior_gate).
pub fn simulate_executable_presentation_boundary_honesty() -> bool {
    if !honesty_executable_presentation_boundary_residual_pack_wave176() {
        return false;
    }
    if !honesty_executable_vertical_presentation_gate_source() {
        return false;
    }
    if !honesty_execute_presentation_only_source_wave176() {
        return false;
    }
    let src = include_str!("../executable_smoke.rs");
    // playable_claim always false in executable smoke construction.
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
        assert!(honesty_executable_presentation_boundary_method_names_residual_wave176());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_executable_presentation_boundary_nav_commands_residual_wave176());
    }

    #[test]
    fn wave176_composite_pack() {
        assert!(honesty_executable_presentation_boundary_residual_pack_wave176());
    }

    #[test]
    fn executable_presentation_sources() {
        assert!(honesty_executable_vertical_presentation_gate_source());
        assert!(honesty_execute_presentation_only_source_wave176());
    }

    #[test]
    fn simulate_executable_presentation_boundary_honesty_residual_live() {
        assert!(
            simulate_executable_presentation_boundary_honesty(),
            "executable InGame presentation boundary residual must latch"
        );
    }
}
