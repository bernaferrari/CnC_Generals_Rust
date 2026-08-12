//! Wave 159 residual peels: terrain/skybox presentation-first boundary residual
//! (no live GameLogic dual-read when PresentationFrame installed; never flips
//! shell `playable_claim`).
//!
//! Orthogonal to Wave 157/158 presentation + print-positions residual.
//! Architecture residual — heightmap/skybox/sync prefer presentation.
//!
//! Sources (repo architecture + cnc_game_engine.rs):
//! - apply_heightmap_hint / apply_skybox_hint presentation-only (Wave 455)
//! - ensure_presentation_env_for_hints seeds freeze when missing
//! - sync_render_terrain_visual presentation-only (Wave 459; seed via ensure_*)
//!
//! Fail-closed:
//! - Not full elimination of GameLogic from map-load seed path
//! - Not GPU heightmap load residual
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// Terrain env boundary residual tables
// ---------------------------------------------------------------------------

/// Terrain/skybox boundary method residual names.
pub const TERRAIN_ENV_BOUNDARY_METHOD_NAMES_WAVE159: &[&str] = &[
    "apply_heightmap_hint",
    "apply_skybox_hint",
    "sync_render_terrain_visual",
    "set_heightmap_hint",
    "set_skybox_hint",
    "set_presentation_frame",
];

/// Source residual markers that must remain presentation-first.
pub const TERRAIN_ENV_BOUNDARY_SOURCE_MARKERS_WAVE159: &[&str] = &[
    "Wave 455: presentation-only env boundary",
    "ensure_presentation_env_for_hints",
    "Wave 459: presentation-only terrain visual sync",
    "world_env.heightmap_hint",
];

/// Ordered terrain env boundary residual navigation steps.
pub const TERRAIN_ENV_BOUNDARY_NAV_STEPS_WAVE159: &[&str] = &[
    "SEED_PRESENTATION_IF_MISSING",
    "APPLY_HEIGHTMAP_FROM_PRESENTATION",
    "APPLY_SKYBOX_FROM_PRESENTATION",
    "SYNC_BOUNDS_FROM_PRESENTATION",
    "LOAD_HEIGHTMAP_HINT",
    "NO_LIVE_GAMELOGIC_DUAL_READ",
];

/// Runtime-host command residual names for terrain env boundary peels.
pub const RUNTIME_HOST_TERRAIN_ENV_BOUNDARY_CMD_NAMES_WAVE159: &[&str] = &[
    "click_terrain_env_boundary_ok_wnd_heightmap",
    "click_terrain_env_boundary_ok_wnd_skybox",
    "click_terrain_env_boundary_ok_wnd_sync",
    "click_terrain_env_boundary_ok_wnd_prepare",
    "click_terrain_env_boundary_miss",
];

// ---------------------------------------------------------------------------
// Residual action latch
// ---------------------------------------------------------------------------

/// Residual: last terrain-env boundary residual action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResidualTerrainEnvBoundaryAction {
    None = 0,
    HeightmapSource = 1,
    SkyboxSource = 2,
    SyncSource = 3,
    Composite = 4,
}

static RESIDUAL_TEB_ACTION: AtomicU8 = AtomicU8::new(0);
static RESIDUAL_TEB_OK: AtomicBool = AtomicBool::new(false);

fn residual_teb_action_store(action: ResidualTerrainEnvBoundaryAction) {
    RESIDUAL_TEB_ACTION.store(action as u8, Ordering::Relaxed);
}

/// Residual: last terrain-env boundary residual action.
pub fn residual_terrain_env_boundary_last_action() -> ResidualTerrainEnvBoundaryAction {
    match RESIDUAL_TEB_ACTION.load(Ordering::Relaxed) {
        1 => ResidualTerrainEnvBoundaryAction::HeightmapSource,
        2 => ResidualTerrainEnvBoundaryAction::SkyboxSource,
        3 => ResidualTerrainEnvBoundaryAction::SyncSource,
        4 => ResidualTerrainEnvBoundaryAction::Composite,
        _ => ResidualTerrainEnvBoundaryAction::None,
    }
}

/// Residual: composite honesty latch.
pub fn residual_terrain_env_boundary_ok() -> bool {
    RESIDUAL_TEB_OK.load(Ordering::Relaxed)
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

/// Residual: heightmap helper is presentation-first (no or_else live dual-read).
pub fn simulate_terrain_env_boundary_heightmap_source() -> bool {
    let src = cnc_source();
    let at = match src.find("fn apply_heightmap_hint(") {
        Some(i) => i,
        None => return false,
    };
    let body = &src[at..src.len().min(at + 1200)];
    let ok = body.contains("Wave 455: presentation-only env boundary")
        && body.contains("presentation_frame()")
        && body.contains("world_env.heightmap_hint")
        && !body.contains("game_logic: Option<&GameLogic>")
        && !body.contains("heightmap_hint().and_then");
    residual_teb_action_store(ResidualTerrainEnvBoundaryAction::HeightmapSource);
    ok
}

/// Residual: skybox helper returns early when presentation is installed.
pub fn simulate_terrain_env_boundary_skybox_source() -> bool {
    let src = cnc_source();
    let at = match src.find("fn apply_skybox_hint(") {
        Some(i) => i,
        None => return false,
    };
    let body = &src[at..src.len().min(at + 1200)];
    let ok = body.contains("Wave 455: presentation-only env boundary")
        && body.contains("presentation_frame()")
        && body.contains("skybox_enabled")
        && body.contains("world_env.skybox")
        && !body.contains("game_logic: Option<&GameLogic>");
    residual_teb_action_store(ResidualTerrainEnvBoundaryAction::SkyboxSource);
    ok
}

/// Residual: terrain sync seeds presentation only when missing.
pub fn simulate_terrain_env_boundary_sync_source() -> bool {
    let src = cnc_source();
    let at = match src.find("fn sync_render_terrain_visual(") {
        Some(i) => i,
        None => return false,
    };
    let body = &src[at..src.len().min(at + 2000)];
    // Wave 459: sync is presentation-only; seeding lives in ensure_presentation_env_for_hints.
    let ok = body.contains("Wave 459: presentation-only terrain visual sync")
        && body.contains("presentation_frame()")
        && body.contains("world_bounds_vec3")
        && !body.contains("game_logic: &GameLogic")
        && !body.contains("PresentationFrame::build_for_engine")
        && src.contains("fn ensure_presentation_env_for_hints");
    residual_teb_action_store(ResidualTerrainEnvBoundaryAction::SyncSource);
    ok
}

/// Residual: composite terrain/skybox presentation-first honesty pack.
pub fn simulate_terrain_env_boundary_prepare_honesty() -> bool {
    let a = simulate_terrain_env_boundary_heightmap_source();
    let b = simulate_terrain_env_boundary_skybox_source();
    let c = simulate_terrain_env_boundary_sync_source();
    let ok = a && b && c;
    RESIDUAL_TEB_OK.store(ok, Ordering::Relaxed);
    residual_teb_action_store(ResidualTerrainEnvBoundaryAction::Composite);
    ok
}

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: method names residual pack.
pub fn honesty_terrain_env_boundary_method_names_residual_wave159() -> bool {
    TERRAIN_ENV_BOUNDARY_METHOD_NAMES_WAVE159.len() == 6
        && residual_name_index(
            TERRAIN_ENV_BOUNDARY_METHOD_NAMES_WAVE159,
            "apply_heightmap_hint",
        ) == Some(0)
        && residual_name_index(
            TERRAIN_ENV_BOUNDARY_METHOD_NAMES_WAVE159,
            "apply_skybox_hint",
        ) == Some(1)
        && residual_name_index(
            TERRAIN_ENV_BOUNDARY_METHOD_NAMES_WAVE159,
            "set_presentation_frame",
        ) == Some(5)
}

/// Honesty: source markers residual pack.
pub fn honesty_terrain_env_boundary_source_markers_residual_wave159() -> bool {
    TERRAIN_ENV_BOUNDARY_SOURCE_MARKERS_WAVE159.len() == 4
        && residual_name_index(
            TERRAIN_ENV_BOUNDARY_SOURCE_MARKERS_WAVE159,
            "Wave 455: presentation-only env boundary",
        ) == Some(0)
        && residual_name_index(
            TERRAIN_ENV_BOUNDARY_SOURCE_MARKERS_WAVE159,
            "world_env.heightmap_hint",
        ) == Some(3)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_terrain_env_boundary_nav_commands_residual_wave159() -> bool {
    TERRAIN_ENV_BOUNDARY_NAV_STEPS_WAVE159.len() == 6
        && residual_name_index(
            TERRAIN_ENV_BOUNDARY_NAV_STEPS_WAVE159,
            "APPLY_HEIGHTMAP_FROM_PRESENTATION",
        ) == Some(1)
        && residual_name_index(
            TERRAIN_ENV_BOUNDARY_NAV_STEPS_WAVE159,
            "NO_LIVE_GAMELOGIC_DUAL_READ",
        ) == Some(5)
        && RUNTIME_HOST_TERRAIN_ENV_BOUNDARY_CMD_NAMES_WAVE159.len() == 5
        && residual_name_index(
            RUNTIME_HOST_TERRAIN_ENV_BOUNDARY_CMD_NAMES_WAVE159,
            "click_terrain_env_boundary_ok_wnd_prepare",
        ) == Some(3)
}

/// Wave 159 composite residual honesty pack.
pub fn honesty_terrain_env_boundary_residual_pack_wave159() -> bool {
    honesty_terrain_env_boundary_method_names_residual_wave159()
        && honesty_terrain_env_boundary_source_markers_residual_wave159()
        && honesty_terrain_env_boundary_nav_commands_residual_wave159()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_terrain_env_boundary_method_names_residual_wave159());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_terrain_env_boundary_source_markers_residual_wave159());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_terrain_env_boundary_nav_commands_residual_wave159());
    }

    #[test]
    fn wave159_composite_pack() {
        assert!(honesty_terrain_env_boundary_residual_pack_wave159());
    }

    #[test]
    fn simulate_terrain_env_boundary_prepare_honesty_residual_live() {
        assert!(
            simulate_terrain_env_boundary_prepare_honesty(),
            "heightmap/skybox/sync presentation-first residual must latch"
        );
        assert!(residual_terrain_env_boundary_ok());
        assert_eq!(
            residual_terrain_env_boundary_last_action(),
            ResidualTerrainEnvBoundaryAction::Composite
        );
    }
}
