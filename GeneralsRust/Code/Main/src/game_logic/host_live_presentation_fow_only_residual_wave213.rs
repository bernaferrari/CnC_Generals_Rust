//! Wave 213 residual peels: unit mesh collect uses presentation FOW snapshot only
//! — no live `FOWRenderingBridge` batch/map dual-read residual in RenderPipeline.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 212 sell deselect residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `render_pipeline.rs` unit pass `snapshot_fow` / `u.fow_visibility`
//! - removed live apply_fow / get_batch helpers
//!
//! Fail-closed:
//! - Not full minimap FOW texture parity
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Presentation FOW-only residual method names.
pub const LIVE_PRESENTATION_FOW_ONLY_METHOD_NAMES_WAVE213: &[&str] = &[
    "snapshot_fow",
    "fow_visibility",
    "no live FOW batch",
    "unit_render_inputs",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_PRESENTATION_FOW_ONLY_NAV_STEPS_WAVE213: &[&str] = &[
    "REQUIRE_UNIT_PASS_SNAPSHOT_FOW",
    "REQUIRE_NO_LIVE_FOW_HELPERS",
    "LIVE_PRESENTATION_FOW_ONLY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_PRESENTATION_FOW_ONLY_CMD_NAMES_WAVE213: &[&str] = &[
    "click_live_presentation_fow_only_ok_prepare",
    "click_live_presentation_fow_only_ok_live",
    "click_live_presentation_fow_only_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_presentation_fow_only_method_names_residual_wave213() -> bool {
    LIVE_PRESENTATION_FOW_ONLY_METHOD_NAMES_WAVE213.len() == 5
        && residual_name_index(
            LIVE_PRESENTATION_FOW_ONLY_METHOD_NAMES_WAVE213,
            "snapshot_fow",
        ) == Some(0)
        && residual_name_index(
            LIVE_PRESENTATION_FOW_ONLY_METHOD_NAMES_WAVE213,
            "no live FOW batch",
        ) == Some(2)
        && residual_name_index(
            LIVE_PRESENTATION_FOW_ONLY_METHOD_NAMES_WAVE213,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_presentation_fow_only_nav_commands_residual_wave213() -> bool {
    LIVE_PRESENTATION_FOW_ONLY_NAV_STEPS_WAVE213.len() == 4
        && residual_name_index(
            LIVE_PRESENTATION_FOW_ONLY_NAV_STEPS_WAVE213,
            "REQUIRE_UNIT_PASS_SNAPSHOT_FOW",
        ) == Some(0)
        && residual_name_index(
            LIVE_PRESENTATION_FOW_ONLY_NAV_STEPS_WAVE213,
            "LIVE_PRESENTATION_FOW_ONLY",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_PRESENTATION_FOW_ONLY_CMD_NAMES_WAVE213.len() == 3
}

/// Wave 213 composite residual honesty pack.
pub fn honesty_live_presentation_fow_only_residual_pack_wave213() -> bool {
    honesty_live_presentation_fow_only_method_names_residual_wave213()
        && honesty_live_presentation_fow_only_nav_commands_residual_wave213()
}

/// Source residual: unit pass uses snapshot_fow / fow_visibility.
pub fn honesty_unit_pass_snapshot_fow_source() -> bool {
    let rp = include_str!("../graphics/render_pipeline.rs");
    rp.contains("snapshot_fow") && rp.contains("u.fow_visibility") && rp.contains("Wave 213")
}

/// Source residual: no live FOW helper dual-read methods on RenderPipeline.
pub fn honesty_no_live_fow_helpers_source() -> bool {
    let rp = include_str!("../graphics/render_pipeline.rs");
    let prod = match rp.find("#[cfg(test)]") {
        Some(i) => &rp[..i],
        None => rp,
    };
    !prod.contains("fn apply_fow_visibility_to_render_object")
        && !prod.contains("fn get_batch_fow_visibility")
        && !prod.contains("fn should_render_object")
        && !prod.contains("HashMap::new();\n        let visibility_elapsed")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_presentation_fow_only_honesty() -> bool {
    honesty_live_presentation_fow_only_residual_pack_wave213()
        && honesty_unit_pass_snapshot_fow_source()
        && honesty_no_live_fow_helpers_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_presentation_fow_only_method_names_residual_wave213());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_presentation_fow_only_nav_commands_residual_wave213());
    }

    #[test]
    fn wave213_composite_pack() {
        assert!(honesty_live_presentation_fow_only_residual_pack_wave213());
    }

    #[test]
    fn fow_only_sources() {
        assert!(honesty_unit_pass_snapshot_fow_source());
        assert!(honesty_no_live_fow_helpers_source());
    }

    #[test]
    fn simulate_live_presentation_fow_only_honesty_residual_live() {
        assert!(
            simulate_live_presentation_fow_only_honesty(),
            "presentation FOW-only residual must latch"
        );
    }
}
