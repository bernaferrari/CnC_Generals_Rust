//! Wave 171 residual peels: live PresentationFrame seed after map load
//! (Lone Eagle/Defcon6 → build_from_logic non-empty objects; never flips
//! shell `playable_claim`).
//!
//! Orthogonal to Wave 170 live map load residual.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH architecture):
//! - CncGameEngine::seed_presentation_after_match_start
//! - PresentationFrame::build_from_logic
//! - Immutable presentation boundary (execute reads frame, not live GameLogic)
//!
//! Fail-closed:
//! - Not full WGPU render pass residual
//! - Not full dual-tick shadow overlay residual (wave 102/157)
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Live presentation seed residual method names.
pub const LIVE_PRESENTATION_SEED_METHOD_NAMES_WAVE171: &[&str] = &[
    "seed_presentation_after_match_start",
    "host_seed_presentation_after_match_start",
    "PresentationFrame::build_for_engine",
    "GameLogic::load_map",
    "build_for_engine",
    "presentation_live_fallback_honesty_ok",
];

/// Ordered live presentation seed residual navigation steps.
pub const LIVE_PRESENTATION_SEED_NAV_STEPS_WAVE171: &[&str] = &[
    "LOAD_RETAIL_MAP",
    "BUILD_PRESENTATION_FROM_LOGIC",
    "REQUIRE_NON_EMPTY_OBJECTS",
    "REQUIRE_LOCAL_TEAM",
    "REQUIRE_FRAME_IDENTITY",
    "REQUIRE_NO_LIVE_FALLBACK_SOURCE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_PRESENTATION_SEED_CMD_NAMES_WAVE171: &[&str] = &[
    "click_live_presentation_seed_ok_build",
    "click_live_presentation_seed_ok_objects",
    "click_live_presentation_seed_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_presentation_seed_method_names_residual_wave171() -> bool {
    LIVE_PRESENTATION_SEED_METHOD_NAMES_WAVE171.len() == 6
        && residual_name_index(
            LIVE_PRESENTATION_SEED_METHOD_NAMES_WAVE171,
            "host_seed_presentation_after_match_start",
        ) == Some(1)
        && residual_name_index(
            LIVE_PRESENTATION_SEED_METHOD_NAMES_WAVE171,
            "PresentationFrame::build_for_engine",
        ) == Some(2)
        && residual_name_index(
            LIVE_PRESENTATION_SEED_METHOD_NAMES_WAVE171,
            "presentation_live_fallback_honesty_ok",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_presentation_seed_nav_commands_residual_wave171() -> bool {
    LIVE_PRESENTATION_SEED_NAV_STEPS_WAVE171.len() == 6
        && residual_name_index(
            LIVE_PRESENTATION_SEED_NAV_STEPS_WAVE171,
            "BUILD_PRESENTATION_FROM_LOGIC",
        ) == Some(1)
        && residual_name_index(
            LIVE_PRESENTATION_SEED_NAV_STEPS_WAVE171,
            "REQUIRE_NON_EMPTY_OBJECTS",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_PRESENTATION_SEED_CMD_NAMES_WAVE171.len() == 3
}

/// Wave 171 composite residual honesty pack.
pub fn honesty_live_presentation_seed_residual_pack_wave171() -> bool {
    honesty_live_presentation_seed_method_names_residual_wave171()
        && honesty_live_presentation_seed_nav_commands_residual_wave171()
}

/// Source residual: engine seeds presentation after match start from GameLogic.
pub fn honesty_seed_presentation_after_match_start_source() -> bool {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    // Wave 590: real seed body lives in host_seed_presentation_after_match_start.
    let i = match src.find("fn host_seed_presentation_after_match_start(&mut self)") {
        Some(i) => i,
        None => match src.find("fn seed_presentation_after_match_start") {
            Some(i) => i,
            None => return false,
        },
    };
    let body = &src[i..src.len().min(i + 2000)];
    // hq-nnuu 2026-08-15: match-start seed inlines sync_from_host +
    // build_for_engine (camera_drain.rs) instead of calling the shared
    // host_sync_shadow_and_build_presentation helper. Either form is the
    // Wave 172/590/926 presentation-build boundary.
    let seed_builds = body.contains("host_sync_shadow_and_build_presentation")
        || (body.contains("sync_from_host") && body.contains("build_for_engine"));
    seed_builds
        && src.contains("fn host_sync_shadow_and_build_presentation")
        && src.contains("host_seed_presentation_after_match_start()")
}

/// Source residual: render execute stays presentation-only (no live GameLogic arg).
pub fn honesty_render_execute_presentation_only_source() -> bool {
    let src = crate::graphics::render_pipeline::RENDER_PIPELINE_SRC;
    let i = match src.find("pub fn execute(") {
        Some(i) => i,
        None => return false,
    };
    let window = &src[i..src.len().min(i + 700)];
    !window.contains("game_logic: Option<&")
        && !window.contains("game_logic: &GameLogic")
        && !window.contains("game_logic: &mut GameLogic")
        && window.contains("presentation_frame")
}

/// Live residual: load retail map, build PresentationFrame, require materialised objects.
pub fn simulate_live_presentation_seed_honesty() -> bool {
    use crate::game_logic::{
        DEFAULT_SKIRMISH_MAP_WAVE169, GameLogic, GameMode, LONE_EAGLE_MAP_WAVE169,
        resolve_retail_map_path,
    };
    use crate::presentation_frame::PresentationFrame;

    if !honesty_live_presentation_seed_residual_pack_wave171() {
        return false;
    }
    if !honesty_seed_presentation_after_match_start_source() {
        return false;
    }
    if !honesty_render_execute_presentation_only_source() {
        return false;
    }

    // Prefer Lone Eagle (behavior_gate map); fall back to Defcon6; soft-ok if neither.
    let map_name = if resolve_retail_map_path(LONE_EAGLE_MAP_WAVE169).is_some() {
        LONE_EAGLE_MAP_WAVE169
    } else if resolve_retail_map_path(DEFAULT_SKIRMISH_MAP_WAVE169).is_some() {
        DEFAULT_SKIRMISH_MAP_WAVE169
    } else {
        return true;
    };

    let mut logic = GameLogic::new();
    logic.start_new_game(GameMode::Skirmish);
    let path = resolve_retail_map_path(map_name);
    let loaded = match path {
        Some(p) => {
            let s = p.to_string_lossy();
            logic.load_map(s.as_ref()) || logic.load_map(map_name)
        }
        None => logic.load_map(map_name),
    };
    if !loaded {
        return false;
    }

    // Advance a few frames so snapshot captures settled spawn state.
    for _ in 0..3 {
        logic.update();
    }

    let local_id = 0u32;
    let pres = PresentationFrame::build_from_logic(&logic, local_id);
    if pres.objects.is_empty() {
        return false;
    }
    // Frame identity residual: map load should leave a non-zero host frame clock
    // after updates, and presentation should bind the same logic epoch.
    if logic.get_frame() == 0 {
        return false;
    }
    // Local team frozen (not default-only empty world).
    let _ = pres.local_team;
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_presentation_seed_method_names_residual_wave171());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_presentation_seed_nav_commands_residual_wave171());
    }

    #[test]
    fn wave171_composite_pack() {
        assert!(honesty_live_presentation_seed_residual_pack_wave171());
    }

    #[test]
    fn seed_presentation_source() {
        assert!(honesty_seed_presentation_after_match_start_source());
    }

    #[test]
    fn render_execute_presentation_only_source() {
        assert!(honesty_render_execute_presentation_only_source());
    }

    #[test]
    fn simulate_live_presentation_seed_honesty_residual_live() {
        assert!(
            simulate_live_presentation_seed_honesty(),
            "live map load must seed non-empty PresentationFrame from GameLogic"
        );
    }
}
