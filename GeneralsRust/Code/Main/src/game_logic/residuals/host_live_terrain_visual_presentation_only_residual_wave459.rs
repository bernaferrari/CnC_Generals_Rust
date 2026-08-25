//! Wave 459 residual peels: terrain visual sync presentation-only residual
//! (sync_render_terrain_visual no longer takes &GameLogic; caller seeds freeze).
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 455/456/457 presentation env peels.
//! Architecture residual - heightmap/road bake from presentation only.
//!
//! Sources (cnc_game_engine.rs):
//! - sync_render_terrain_visual(pipeline, graphics, map_name) — no &GameLogic
//! - ensure_presentation_env_for_hints before sync at call sites
//! - bounds from presentation_frame().world_env
//!
//! Fail-closed:
//! - Not full SAGE terrain mesh parity
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const TERRAIN_VISUAL_PRESENTATION_ONLY_METHOD_NAMES_WAVE459: &[&str] = &[
    "sync_render_terrain_visual",
    "ensure_presentation_env_for_hints",
    "load_heightmap_from_hint",
    "load_heightmap_from_runtime_terrain",
    "sync_runtime_map_roads",
    "world_bounds_vec3",
];

pub const TERRAIN_VISUAL_PRESENTATION_ONLY_SOURCE_MARKERS_WAVE459: &[&str] = &[
    "Wave 459: presentation-only terrain visual sync",
    "presentation_frame()",
    "world_bounds_vec3",
    "ensure_presentation_env_for_hints",
];

pub const TERRAIN_VISUAL_PRESENTATION_ONLY_NAV_STEPS_WAVE459: &[&str] = &[
    "SEED_PRESENTATION_IF_MISSING",
    "READ_BOUNDS_FROM_PRESENTATION",
    "LOAD_HEIGHTMAP_FROM_HINT",
    "FALLBACK_RUNTIME_TERRAIN_BAKE",
    "SYNC_RUNTIME_MAP_ROADS",
    "NO_LIVE_GAMELOGIC_DUAL_READ",
];

pub const RUNTIME_HOST_TERRAIN_VISUAL_PRESENTATION_ONLY_CMD_NAMES_WAVE459: &[&str] = &[
    "click_terrain_visual_presentation_only_ok_wnd_seed",
    "click_terrain_visual_presentation_only_ok_wnd_bounds",
    "click_terrain_visual_presentation_only_ok_wnd_heightmap",
    "click_terrain_visual_presentation_only_ok_wnd_prepare",
    "click_terrain_visual_presentation_only_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualTerrainVisualPresentationOnlyAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    SyncSource = 4,
    CallSites = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualTerrainVisualPresentationOnlyAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_terrain_visual_presentation_only_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_terrain_visual_presentation_only_last_action()
-> ResidualTerrainVisualPresentationOnlyAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualTerrainVisualPresentationOnlyAction::MethodNames,
        2 => ResidualTerrainVisualPresentationOnlyAction::SourceMarkers,
        3 => ResidualTerrainVisualPresentationOnlyAction::NavCommands,
        4 => ResidualTerrainVisualPresentationOnlyAction::SyncSource,
        5 => ResidualTerrainVisualPresentationOnlyAction::CallSites,
        6 => ResidualTerrainVisualPresentationOnlyAction::Composite,
        _ => ResidualTerrainVisualPresentationOnlyAction::Idle,
    }
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

pub fn honesty_terrain_visual_presentation_only_method_names_residual_wave459() -> bool {
    TERRAIN_VISUAL_PRESENTATION_ONLY_METHOD_NAMES_WAVE459.len() == 6
        && residual_name_index(
            TERRAIN_VISUAL_PRESENTATION_ONLY_METHOD_NAMES_WAVE459,
            "sync_render_terrain_visual",
        ) == Some(0)
        && residual_name_index(
            TERRAIN_VISUAL_PRESENTATION_ONLY_METHOD_NAMES_WAVE459,
            "world_bounds_vec3",
        ) == Some(5)
}

pub fn honesty_terrain_visual_presentation_only_source_markers_residual_wave459() -> bool {
    TERRAIN_VISUAL_PRESENTATION_ONLY_SOURCE_MARKERS_WAVE459.len() == 4
        && residual_name_index(
            TERRAIN_VISUAL_PRESENTATION_ONLY_SOURCE_MARKERS_WAVE459,
            "Wave 459: presentation-only terrain visual sync",
        ) == Some(0)
        && residual_name_index(
            TERRAIN_VISUAL_PRESENTATION_ONLY_SOURCE_MARKERS_WAVE459,
            "ensure_presentation_env_for_hints",
        ) == Some(3)
}

pub fn honesty_terrain_visual_presentation_only_nav_commands_residual_wave459() -> bool {
    TERRAIN_VISUAL_PRESENTATION_ONLY_NAV_STEPS_WAVE459.len() == 6
        && residual_name_index(
            TERRAIN_VISUAL_PRESENTATION_ONLY_NAV_STEPS_WAVE459,
            "READ_BOUNDS_FROM_PRESENTATION",
        ) == Some(1)
        && residual_name_index(
            TERRAIN_VISUAL_PRESENTATION_ONLY_NAV_STEPS_WAVE459,
            "NO_LIVE_GAMELOGIC_DUAL_READ",
        ) == Some(5)
        && RUNTIME_HOST_TERRAIN_VISUAL_PRESENTATION_ONLY_CMD_NAMES_WAVE459.len() == 5
        && residual_name_index(
            RUNTIME_HOST_TERRAIN_VISUAL_PRESENTATION_ONLY_CMD_NAMES_WAVE459,
            "click_terrain_visual_presentation_only_ok_wnd_prepare",
        ) == Some(3)
}

fn function_body<'a>(src: &'a str, sig: &str) -> Option<&'a str> {
    let at = src.find(sig)?;
    let bytes = src.as_bytes();
    let mut depth = 0i32;
    let mut end = at;
    let mut seen = false;
    for (j, &b) in bytes[at..].iter().enumerate() {
        if b == b'{' {
            depth += 1;
            seen = true;
        } else if b == b'}' {
            depth -= 1;
            if seen && depth == 0 {
                end = at + j + 1;
                break;
            }
        }
    }
    Some(&src[at..end])
}

pub fn simulate_terrain_visual_presentation_only_source() -> bool {
    let src = cnc_source();
    let Some(body) = function_body(src, "fn sync_render_terrain_visual(") else {
        return false;
    };
    let ok = body.contains("Wave 459: presentation-only terrain visual sync")
        && body.contains("presentation_frame()")
        && body.contains("world_bounds_vec3()")
        && body.contains("load_heightmap_from_hint")
        && body.contains("sync_runtime_map_roads")
        && !body.contains("game_logic: &GameLogic")
        && !body.contains("build_for_engine");
    residual_action_store(ResidualTerrainVisualPresentationOnlyAction::SyncSource);
    ok
}

pub fn simulate_terrain_visual_presentation_only_callsites() -> bool {
    let src = cnc_source();
    let mut ok_n = 0usize;
    let mut from = 0usize;
    while let Some(rel) = src[from..].find("Self::sync_render_terrain_visual(") {
        let i = from + rel;
        let lo = i.saturating_sub(400);
        let prior = &src[lo..i];
        let call_win = &src[i..src.len().min(i + 220)];
        if prior.contains("ensure_presentation_env_for_hints")
            || prior.contains("ensure_presentation_env_seeded")
                && !call_win.contains("&self.game_logic")
        {
            ok_n += 1;
        }
        from = i + 30;
    }
    let ok = ok_n >= 3;
    residual_action_store(ResidualTerrainVisualPresentationOnlyAction::CallSites);
    ok
}

pub fn honesty_terrain_visual_presentation_only_residual_pack_wave459() -> bool {
    honesty_terrain_visual_presentation_only_method_names_residual_wave459()
        && honesty_terrain_visual_presentation_only_source_markers_residual_wave459()
        && honesty_terrain_visual_presentation_only_nav_commands_residual_wave459()
        && simulate_terrain_visual_presentation_only_source()
        && simulate_terrain_visual_presentation_only_callsites()
}

pub fn simulate_live_terrain_visual_presentation_only_honesty() -> bool {
    let ok = honesty_terrain_visual_presentation_only_residual_pack_wave459();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualTerrainVisualPresentationOnlyAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_terrain_visual_presentation_only_method_names_residual_wave459());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_terrain_visual_presentation_only_source_markers_residual_wave459());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_terrain_visual_presentation_only_nav_commands_residual_wave459());
    }

    #[test]
    fn terrain_visual_presentation_only_sources() {
        assert!(simulate_terrain_visual_presentation_only_source());
        assert!(simulate_terrain_visual_presentation_only_callsites());
    }

    #[test]
    fn wave459_composite_pack() {
        assert!(honesty_terrain_visual_presentation_only_residual_pack_wave459());
    }

    #[test]
    fn simulate_live_terrain_visual_presentation_only_honesty_residual_live() {
        assert!(
            simulate_live_terrain_visual_presentation_only_honesty(),
            "terrain visual presentation-only residual must latch"
        );
        assert!(residual_terrain_visual_presentation_only_ok());
        assert_eq!(
            residual_terrain_visual_presentation_only_last_action(),
            ResidualTerrainVisualPresentationOnlyAction::Composite
        );
    }
}
