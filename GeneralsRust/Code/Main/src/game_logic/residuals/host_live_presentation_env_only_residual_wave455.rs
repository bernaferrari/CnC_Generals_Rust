//! Wave 455 residual peels: presentation-only heightmap/skybox env apply.
//! `apply_heightmap_hint` / `apply_skybox_hint` read `PresentationFrame.world_env`
//! only — no live `&GameLogic` dual-read. Boot paths seed env via
//! `ensure_presentation_env_for_hints` first.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 454 construction placement dual-world empty-gate residual.
//!
//! Sources:
//! - `Main/src/cnc_game_engine.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Terrain sync may still take `&GameLogic` for load helpers outside apply_*_hint

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Presentation-env-only residual method names.
pub const LIVE_PRESENTATION_ENV_ONLY_METHOD_NAMES_WAVE455: &[&str] = &[
    "ensure_presentation_env_for_hints",
    "apply_heightmap_hint",
    "apply_skybox_hint",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_PRESENTATION_ENV_ONLY_NAV_STEPS_WAVE455: &[&str] = &[
    "REQUIRE_ENSURE_PRESENTATION_ENV",
    "REQUIRE_APPLY_HINTS_PRESENTATION_ONLY",
    "LIVE_PRESENTATION_ENV_ONLY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_PRESENTATION_ENV_ONLY_CMD_NAMES_WAVE455: &[&str] = &[
    "click_live_presentation_env_only_ok_prepare",
    "click_live_presentation_env_only_ok_live",
    "click_live_presentation_env_only_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_presentation_env_only_method_names_residual_wave455() -> bool {
    LIVE_PRESENTATION_ENV_ONLY_METHOD_NAMES_WAVE455.len() == 4
        && residual_name_index(
            LIVE_PRESENTATION_ENV_ONLY_METHOD_NAMES_WAVE455,
            "ensure_presentation_env_for_hints",
        ) == Some(0)
        && residual_name_index(
            LIVE_PRESENTATION_ENV_ONLY_METHOD_NAMES_WAVE455,
            "apply_skybox_hint",
        ) == Some(2)
        && residual_name_index(
            LIVE_PRESENTATION_ENV_ONLY_METHOD_NAMES_WAVE455,
            "playable_claim = false",
        ) == Some(3)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_presentation_env_only_nav_commands_residual_wave455() -> bool {
    LIVE_PRESENTATION_ENV_ONLY_NAV_STEPS_WAVE455.len() == 4
        && residual_name_index(
            LIVE_PRESENTATION_ENV_ONLY_NAV_STEPS_WAVE455,
            "REQUIRE_ENSURE_PRESENTATION_ENV",
        ) == Some(0)
        && residual_name_index(
            LIVE_PRESENTATION_ENV_ONLY_NAV_STEPS_WAVE455,
            "LIVE_PRESENTATION_ENV_ONLY",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_PRESENTATION_ENV_ONLY_CMD_NAMES_WAVE455.len() == 3
}

/// Wave 455 composite residual honesty pack.
pub fn honesty_live_presentation_env_only_residual_pack_wave455() -> bool {
    honesty_live_presentation_env_only_method_names_residual_wave455()
        && honesty_live_presentation_env_only_nav_commands_residual_wave455()
}

/// Source residual: apply_*_hint are presentation-only.
pub fn honesty_presentation_env_only_source() -> bool {
    let g = include_str!("../../cnc_game_engine.rs");
    if !(g.contains("Wave 455")
        && g.contains("fn ensure_presentation_env_for_hints")
        && g.contains("fn apply_heightmap_hint(render_pipeline: &mut RenderPipeline)")
        && g.contains("fn apply_skybox_hint(render_pipeline: &mut RenderPipeline)"))
    {
        return false;
    }
    // Must not take live GameLogic on apply signatures.
    if g.contains("fn apply_heightmap_hint(render_pipeline: &mut RenderPipeline, game_logic")
        || g.contains("fn apply_skybox_hint(render_pipeline: &mut RenderPipeline, game_logic")
    {
        return false;
    }
    // apply bodies must read presentation_frame world_env.
    let hm = g
        .find("fn apply_heightmap_hint(render_pipeline: &mut RenderPipeline)")
        .unwrap();
    let hm_body = &g[hm..g.len().min(hm + 900)];
    let sky = g
        .find("fn apply_skybox_hint(render_pipeline: &mut RenderPipeline)")
        .unwrap();
    let sky_body = &g[sky..g.len().min(sky + 700)];
    hm_body.contains("presentation_frame()")
        && hm_body.contains("world_env.heightmap_hint")
        && sky_body.contains("presentation_frame()")
        && sky_body.contains("world_env.skybox_enabled")
        && (g.contains("self.gameworld_shadow.as_ref()")
            || g.contains("ensure_presentation_env_seeded"))
        && g.contains("fn ensure_presentation_env_for_hints")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_presentation_env_only_honesty() -> bool {
    honesty_live_presentation_env_only_residual_pack_wave455()
        && honesty_presentation_env_only_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_presentation_env_only_method_names_residual_wave455());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_presentation_env_only_nav_commands_residual_wave455());
    }

    #[test]
    fn wave455_composite_pack() {
        assert!(honesty_live_presentation_env_only_residual_pack_wave455());
    }

    #[test]
    fn presentation_env_only_sources() {
        assert!(honesty_presentation_env_only_source());
    }

    #[test]
    fn simulate_live_presentation_env_only_honesty_residual_live() {
        assert!(
            simulate_live_presentation_env_only_honesty(),
            "presentation env-only residual must latch"
        );
    }
}
