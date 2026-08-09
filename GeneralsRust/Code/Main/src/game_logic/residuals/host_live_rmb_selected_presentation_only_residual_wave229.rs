//! Wave 229 residual peels: RMB classification selected-unit capability probes
//! prefer `PresentationSelectedUnitHint` on `MouseCommandContext`; cursor hover
//! selection uses `ui_selected_ids`. Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 228 RMB target presentation-only residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `command_system.rs` PresentationSelectedUnitHint / classify
//! - `cnc_game_engine.rs` presentation_selected_unit_hints / cursor selection
//!
//! Fail-closed:
//! - Not full C++ unit capability matrix
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// RMB selected presentation-only residual method names.
pub const LIVE_RMB_SELECTED_PRESENTATION_ONLY_METHOD_NAMES_WAVE229: &[&str] = &[
    "PresentationSelectedUnitHint",
    "selected_presentation",
    "presentation_selected_unit_hints",
    "ui_selected_ids",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_RMB_SELECTED_PRESENTATION_ONLY_NAV_STEPS_WAVE229: &[&str] = &[
    "REQUIRE_RMB_SELECTED_PRESENTATION_ONLY",
    "REQUIRE_SELECTED_UNIT_HINTS",
    "LIVE_RMB_SELECTED_PRESENTATION_ONLY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_RMB_SELECTED_PRESENTATION_ONLY_CMD_NAMES_WAVE229: &[&str] = &[
    "click_live_rmb_selected_presentation_only_ok_prepare",
    "click_live_rmb_selected_presentation_only_ok_live",
    "click_live_rmb_selected_presentation_only_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_rmb_selected_presentation_only_method_names_residual_wave229() -> bool {
    LIVE_RMB_SELECTED_PRESENTATION_ONLY_METHOD_NAMES_WAVE229.len() == 5
        && residual_name_index(
            LIVE_RMB_SELECTED_PRESENTATION_ONLY_METHOD_NAMES_WAVE229,
            "PresentationSelectedUnitHint",
        ) == Some(0)
        && residual_name_index(
            LIVE_RMB_SELECTED_PRESENTATION_ONLY_METHOD_NAMES_WAVE229,
            "presentation_selected_unit_hints",
        ) == Some(2)
        && residual_name_index(
            LIVE_RMB_SELECTED_PRESENTATION_ONLY_METHOD_NAMES_WAVE229,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_rmb_selected_presentation_only_nav_commands_residual_wave229() -> bool {
    LIVE_RMB_SELECTED_PRESENTATION_ONLY_NAV_STEPS_WAVE229.len() == 4
        && residual_name_index(
            LIVE_RMB_SELECTED_PRESENTATION_ONLY_NAV_STEPS_WAVE229,
            "REQUIRE_RMB_SELECTED_PRESENTATION_ONLY",
        ) == Some(0)
        && residual_name_index(
            LIVE_RMB_SELECTED_PRESENTATION_ONLY_NAV_STEPS_WAVE229,
            "LIVE_RMB_SELECTED_PRESENTATION_ONLY",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_RMB_SELECTED_PRESENTATION_ONLY_CMD_NAMES_WAVE229.len() == 3
}

/// Wave 229 composite residual honesty pack.
pub fn honesty_live_rmb_selected_presentation_only_residual_pack_wave229() -> bool {
    honesty_live_rmb_selected_presentation_only_method_names_residual_wave229()
        && honesty_live_rmb_selected_presentation_only_nav_commands_residual_wave229()
}

/// Source residual: selected unit hints + cursor ui_selected_ids.
pub fn honesty_rmb_selected_presentation_only_source() -> bool {
    let cs = include_str!("../../command_system.rs");
    let eng = include_str!("../../cnc_game_engine.rs");
    cs.contains("struct PresentationSelectedUnitHint")
        && cs.contains("selected_presentation: Vec<PresentationSelectedUnitHint>")
        && cs.contains("selected_presentation: &[PresentationSelectedUnitHint]")
        && cs.contains("Wave 228/229")
        && eng.contains("fn presentation_selected_unit_hints")
        && eng.contains("presentation_selected_unit_hints(&selected)")
        && eng.contains("Wave 229: selection via presentation-first ui_selected_ids")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_rmb_selected_presentation_only_honesty() -> bool {
    honesty_live_rmb_selected_presentation_only_residual_pack_wave229()
        && honesty_rmb_selected_presentation_only_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_rmb_selected_presentation_only_method_names_residual_wave229());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_rmb_selected_presentation_only_nav_commands_residual_wave229());
    }

    #[test]
    fn wave229_composite_pack() {
        assert!(honesty_live_rmb_selected_presentation_only_residual_pack_wave229());
    }

    #[test]
    fn rmb_selected_sources() {
        assert!(honesty_rmb_selected_presentation_only_source());
    }

    #[test]
    fn simulate_live_rmb_selected_presentation_only_honesty_residual_live() {
        assert!(
            simulate_live_rmb_selected_presentation_only_honesty(),
            "rmb selected presentation-only residual must latch"
        );
    }
}
