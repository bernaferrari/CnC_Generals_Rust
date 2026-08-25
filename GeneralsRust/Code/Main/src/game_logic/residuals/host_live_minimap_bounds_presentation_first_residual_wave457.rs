//! Wave 457 residual peels: minimap world-bounds presentation-first residual
//! (reinitialize_minimap_renderer prefers PresentationFrame.world_env bounds;
//! host override only repairs degenerate bounds + stamps freeze).
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 456 map lighting presentation-only.
//! Architecture residual - minimap bounds prefer presentation freeze.
//!
//! Sources (cnc_game_engine.rs / render_pipeline.rs):
//! - reinitialize_minimap_renderer presentation-first bounds
//! - ensure_presentation_env_for_hints before minimap reinit
//! - presentation_frame_mut stamps repaired bounds
//!
//! Fail-closed:
//! - Not full radar/minimap FOW retail parity
//! - Host still mutates GameLogic world size on degenerate bounds
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const MINIMAP_BOUNDS_PRESENTATION_FIRST_METHOD_NAMES_WAVE457: &[&str] = &[
    "reinitialize_minimap_renderer",
    "ensure_presentation_env_for_hints",
    "presentation_frame",
    "presentation_frame_mut",
    "world_bounds_vec3",
    "update_minimap_world_bounds",
];

pub const MINIMAP_BOUNDS_PRESENTATION_FIRST_SOURCE_MARKERS_WAVE457: &[&str] = &[
    "Wave 457: prefer presentation freeze for minimap world bounds",
    "presentation_frame()",
    "world_bounds_vec3",
    "presentation_frame_mut",
];

pub const MINIMAP_BOUNDS_PRESENTATION_FIRST_NAV_STEPS_WAVE457: &[&str] = &[
    "SEED_PRESENTATION_IF_MISSING",
    "READ_BOUNDS_FROM_PRESENTATION",
    "INIT_MINIMAP_RENDERER",
    "REPAIR_DEGENERATE_VIA_HEIGHTMAP",
    "STAMP_BOUNDS_INTO_PRESENTATION",
    "SYNC_HEIGHTMAP_AND_MINIMAP_BOUNDS",
];

pub const RUNTIME_HOST_MINIMAP_BOUNDS_PRESENTATION_FIRST_CMD_NAMES_WAVE457: &[&str] = &[
    "click_minimap_bounds_presentation_first_ok_wnd_seed",
    "click_minimap_bounds_presentation_first_ok_wnd_bounds",
    "click_minimap_bounds_presentation_first_ok_wnd_init",
    "click_minimap_bounds_presentation_first_ok_wnd_prepare",
    "click_minimap_bounds_presentation_first_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualMinimapBoundsPresentationFirstAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    MinimapSource = 4,
    CallSites = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualMinimapBoundsPresentationFirstAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_minimap_bounds_presentation_first_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_minimap_bounds_presentation_first_last_action()
-> ResidualMinimapBoundsPresentationFirstAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualMinimapBoundsPresentationFirstAction::MethodNames,
        2 => ResidualMinimapBoundsPresentationFirstAction::SourceMarkers,
        3 => ResidualMinimapBoundsPresentationFirstAction::NavCommands,
        4 => ResidualMinimapBoundsPresentationFirstAction::MinimapSource,
        5 => ResidualMinimapBoundsPresentationFirstAction::CallSites,
        6 => ResidualMinimapBoundsPresentationFirstAction::Composite,
        _ => ResidualMinimapBoundsPresentationFirstAction::Idle,
    }
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

fn pipeline_source() -> &'static str {
    crate::graphics::render_pipeline::RENDER_PIPELINE_SRC
}

pub fn honesty_minimap_bounds_presentation_first_method_names_residual_wave457() -> bool {
    MINIMAP_BOUNDS_PRESENTATION_FIRST_METHOD_NAMES_WAVE457.len() == 6
        && residual_name_index(
            MINIMAP_BOUNDS_PRESENTATION_FIRST_METHOD_NAMES_WAVE457,
            "reinitialize_minimap_renderer",
        ) == Some(0)
        && residual_name_index(
            MINIMAP_BOUNDS_PRESENTATION_FIRST_METHOD_NAMES_WAVE457,
            "update_minimap_world_bounds",
        ) == Some(5)
}

pub fn honesty_minimap_bounds_presentation_first_source_markers_residual_wave457() -> bool {
    MINIMAP_BOUNDS_PRESENTATION_FIRST_SOURCE_MARKERS_WAVE457.len() == 4
        && residual_name_index(
            MINIMAP_BOUNDS_PRESENTATION_FIRST_SOURCE_MARKERS_WAVE457,
            "Wave 457: prefer presentation freeze for minimap world bounds",
        ) == Some(0)
        && residual_name_index(
            MINIMAP_BOUNDS_PRESENTATION_FIRST_SOURCE_MARKERS_WAVE457,
            "presentation_frame_mut",
        ) == Some(3)
}

pub fn honesty_minimap_bounds_presentation_first_nav_commands_residual_wave457() -> bool {
    MINIMAP_BOUNDS_PRESENTATION_FIRST_NAV_STEPS_WAVE457.len() == 6
        && residual_name_index(
            MINIMAP_BOUNDS_PRESENTATION_FIRST_NAV_STEPS_WAVE457,
            "READ_BOUNDS_FROM_PRESENTATION",
        ) == Some(1)
        && residual_name_index(
            MINIMAP_BOUNDS_PRESENTATION_FIRST_NAV_STEPS_WAVE457,
            "SYNC_HEIGHTMAP_AND_MINIMAP_BOUNDS",
        ) == Some(5)
        && RUNTIME_HOST_MINIMAP_BOUNDS_PRESENTATION_FIRST_CMD_NAMES_WAVE457.len() == 5
        && residual_name_index(
            RUNTIME_HOST_MINIMAP_BOUNDS_PRESENTATION_FIRST_CMD_NAMES_WAVE457,
            "click_minimap_bounds_presentation_first_ok_wnd_prepare",
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

/// Residual: minimap reinit prefers presentation bounds.
pub fn simulate_minimap_bounds_presentation_first_source() -> bool {
    let src = cnc_source();
    let Some(body) = function_body(src, "fn reinitialize_minimap_renderer(") else {
        return false;
    };
    // Wave 594: heightmap stamp + last_presentation align lives in host helper.
    let Some(repair) = function_body(src, "fn host_repair_minimap_presentation_bounds(") else {
        return false;
    };
    let pipe = pipeline_source();
    // Wave 468/594: instance method uses presentation_world_bounds + host repair helper.
    let ok = body.contains("&mut self")
        && (body.contains("presentation_world_bounds()") || body.contains("world_bounds_vec3()"))
        && body.contains("host_repair_minimap_presentation_bounds")
        && repair.contains("presentation_frame_mut()")
        && repair.contains("override_world_size")
        && (body.contains("Wave 468: instance path")
            || body.contains("Wave 465: presentation-first minimap bounds")
            || body.contains("Wave 457: prefer presentation freeze")
            || body.contains("Wave 594")
            || repair.contains("Wave 594"))
        && pipe.contains("fn presentation_frame_mut");
    residual_action_store(ResidualMinimapBoundsPresentationFirstAction::MinimapSource);
    ok
}

pub fn simulate_minimap_bounds_presentation_first_callsites() -> bool {
    let src = cnc_source();
    // Wave 468: instance method seeds presentation inside reinitialize_minimap_renderer.
    let instance = src.contains("fn reinitialize_minimap_renderer(&mut self)")
        && src.contains("ensure_presentation_env_seeded()")
        && src.contains("presentation_world_bounds()");
    let call_n = src.matches("self.reinitialize_minimap_renderer()").count();
    let ok = instance && call_n >= 3;
    residual_action_store(ResidualMinimapBoundsPresentationFirstAction::CallSites);
    ok
}

pub fn honesty_minimap_bounds_presentation_first_residual_pack_wave457() -> bool {
    honesty_minimap_bounds_presentation_first_method_names_residual_wave457()
        && honesty_minimap_bounds_presentation_first_source_markers_residual_wave457()
        && honesty_minimap_bounds_presentation_first_nav_commands_residual_wave457()
        && simulate_minimap_bounds_presentation_first_source()
        && simulate_minimap_bounds_presentation_first_callsites()
}

pub fn simulate_live_minimap_bounds_presentation_first_honesty() -> bool {
    let ok = honesty_minimap_bounds_presentation_first_residual_pack_wave457();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualMinimapBoundsPresentationFirstAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_minimap_bounds_presentation_first_method_names_residual_wave457());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_minimap_bounds_presentation_first_source_markers_residual_wave457());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_minimap_bounds_presentation_first_nav_commands_residual_wave457());
    }

    #[test]
    fn minimap_bounds_presentation_first_sources() {
        assert!(simulate_minimap_bounds_presentation_first_source());
        assert!(simulate_minimap_bounds_presentation_first_callsites());
    }

    #[test]
    fn wave457_composite_pack() {
        assert!(honesty_minimap_bounds_presentation_first_residual_pack_wave457());
    }

    #[test]
    fn simulate_live_minimap_bounds_presentation_first_honesty_residual_live() {
        assert!(
            simulate_live_minimap_bounds_presentation_first_honesty(),
            "minimap bounds presentation-first residual must latch"
        );
        assert!(residual_minimap_bounds_presentation_first_ok());
        assert_eq!(
            residual_minimap_bounds_presentation_first_last_action(),
            ResidualMinimapBoundsPresentationFirstAction::Composite
        );
    }
}
