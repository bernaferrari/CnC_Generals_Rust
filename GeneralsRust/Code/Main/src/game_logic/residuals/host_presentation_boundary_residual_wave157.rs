//! Wave 157 residual peels: presentation boundary honesty residual
//! (execute/collect without live GameLogic; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 156 ControlBarScheme residual.
//! Architecture residual — immutable PresentationFrame → renderer boundary.
//!
//! Sources (repo architecture + render_pipeline.rs):
//! - RenderPipeline::execute presentation-only signature
//! - collect_render_items unit_render_inputs path
//! - debug_last_presentation_live_fallback_reads honesty counter
//!
//! Fail-closed:
//! - Not full removal of all GameLogic dual-reads outside execute
//! - Not GPU render smoke residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// Presentation boundary residual tables
// ---------------------------------------------------------------------------

/// Presentation boundary method residual names.
pub const PRESENTATION_BOUNDARY_METHOD_NAMES_WAVE157: &[&str] = &[
    "set_presentation_frame",
    "execute",
    "collect_render_items",
    "unit_render_inputs",
    "last_presentation_live_fallback_reads",
    "presentation_live_fallback_honesty_ok",
];

/// Presentation boundary source residual markers.
pub const PRESENTATION_BOUNDARY_SOURCE_MARKERS_WAVE157: &[&str] = &[
    "debug_last_presentation_live_fallback_reads",
    "Immutable presentation boundary",
    "unit_render_inputs",
    "presentation_frame",
];

/// Ordered presentation boundary residual navigation steps.
pub const PRESENTATION_BOUNDARY_NAV_STEPS_WAVE157: &[&str] = &[
    "BUILD_PRESENTATION_FRAME",
    "SET_PRESENTATION_FRAME",
    "COLLECT_FROM_UNIT_RENDER_INPUTS",
    "RESET_LIVE_FALLBACK_COUNTER",
    "EXECUTE_WITHOUT_GAME_LOGIC",
    "ASSERT_FALLBACK_READS_ZERO",
];

/// Runtime-host command residual names for presentation boundary peels.
pub const RUNTIME_HOST_PRESENTATION_BOUNDARY_CMD_NAMES_WAVE157: &[&str] = &[
    "click_presentation_boundary_ok_wnd_execute",
    "click_presentation_boundary_ok_wnd_collect",
    "click_presentation_boundary_ok_wnd_fallback",
    "click_presentation_boundary_ok_wnd_cnc",
    "click_presentation_boundary_ok_wnd_prepare",
    "click_presentation_boundary_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: presentation boundary method names residual pack.
pub fn honesty_presentation_boundary_method_names_residual_wave157() -> bool {
    PRESENTATION_BOUNDARY_METHOD_NAMES_WAVE157.len() == 6
        && residual_name_index(PRESENTATION_BOUNDARY_METHOD_NAMES_WAVE157, "execute") == Some(1)
        && residual_name_index(
            PRESENTATION_BOUNDARY_METHOD_NAMES_WAVE157,
            "unit_render_inputs",
        ) == Some(3)
        && residual_name_index(
            PRESENTATION_BOUNDARY_METHOD_NAMES_WAVE157,
            "presentation_live_fallback_honesty_ok",
        ) == Some(5)
}

/// Honesty: source markers residual pack.
pub fn honesty_presentation_boundary_source_markers_residual_wave157() -> bool {
    PRESENTATION_BOUNDARY_SOURCE_MARKERS_WAVE157.len() == 4
        && residual_name_index(
            PRESENTATION_BOUNDARY_SOURCE_MARKERS_WAVE157,
            "debug_last_presentation_live_fallback_reads",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_BOUNDARY_SOURCE_MARKERS_WAVE157,
            "unit_render_inputs",
        ) == Some(2)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_presentation_boundary_nav_commands_residual_wave157() -> bool {
    PRESENTATION_BOUNDARY_NAV_STEPS_WAVE157.len() == 6
        && residual_name_index(
            PRESENTATION_BOUNDARY_NAV_STEPS_WAVE157,
            "SET_PRESENTATION_FRAME",
        ) == Some(1)
        && residual_name_index(
            PRESENTATION_BOUNDARY_NAV_STEPS_WAVE157,
            "EXECUTE_WITHOUT_GAME_LOGIC",
        ) == Some(4)
        && residual_name_index(
            PRESENTATION_BOUNDARY_NAV_STEPS_WAVE157,
            "ASSERT_FALLBACK_READS_ZERO",
        ) == Some(5)
        && RUNTIME_HOST_PRESENTATION_BOUNDARY_CMD_NAMES_WAVE157.len() == 6
        && residual_name_index(
            RUNTIME_HOST_PRESENTATION_BOUNDARY_CMD_NAMES_WAVE157,
            "click_presentation_boundary_ok_wnd_prepare",
        ) == Some(4)
}

/// Wave 157 composite residual honesty pack.
pub fn honesty_presentation_boundary_residual_pack_wave157() -> bool {
    honesty_presentation_boundary_method_names_residual_wave157()
        && honesty_presentation_boundary_source_markers_residual_wave157()
        && honesty_presentation_boundary_nav_commands_residual_wave157()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_boundary_method_names_residual_wave157());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_boundary_source_markers_residual_wave157());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_boundary_nav_commands_residual_wave157());
    }

    #[test]
    fn wave157_composite_pack() {
        assert!(honesty_presentation_boundary_residual_pack_wave157());
    }

    #[test]
    fn simulate_presentation_boundary_prepare_honesty_residual_live() {
        use crate::graphics::{
            ResidualPresentationBoundaryAction, residual_presentation_boundary_last_action,
            residual_presentation_boundary_ok, simulate_presentation_boundary_prepare_honesty,
        };
        assert!(
            simulate_presentation_boundary_prepare_honesty(),
            "presentation boundary source residual must latch"
        );
        assert!(residual_presentation_boundary_ok());
        assert_eq!(
            residual_presentation_boundary_last_action(),
            ResidualPresentationBoundaryAction::Composite
        );
    }
}
