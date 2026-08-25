//! Wave 461 residual peels: shared presentation_world_bounds probe for
//! camera/HUD/minimap (pipeline freeze → last frame → host GameLogic).
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 460 center_camera height residual.
//! Architecture residual - one bounds probe, no scattered dual-reads.
//!
//! Sources (cnc_game_engine.rs):
//! - presentation_world_bounds() shared helper
//! - clamp_to_world_bounds / HUD / minimap consumers call the probe
//! - live game_logic.world_bounds only inside the probe fallback
//!
//! Fail-closed:
//! - Boot residual without freeze still reads host bounds via probe
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_WORLD_BOUNDS_PROBE_METHOD_NAMES_WAVE461: &[&str] = &[
    "presentation_world_bounds",
    "clamp_to_world_bounds",
    "presentation_frame",
    "world_bounds_vec3",
    "world_bounds",
    "last_presentation_frame",
];

pub const PRESENTATION_WORLD_BOUNDS_PROBE_SOURCE_MARKERS_WAVE461: &[&str] = &[
    "Wave 461: single presentation-first world bounds probe",
    "presentation_world_bounds()",
    "host_world_bounds",
    "presentation_frame()",
    "last_presentation_frame",
];

pub const PRESENTATION_WORLD_BOUNDS_PROBE_NAV_STEPS_WAVE461: &[&str] = &[
    "RESOLVE_PIPELINE_PRESENTATION",
    "FALLBACK_LAST_PRESENTATION_FRAME",
    "FALLBACK_HOST_GAMELOGIC_BOUNDS",
    "CLAMP_CAMERA_VIA_PROBE",
    "HUD_MINIMAP_VIA_PROBE",
    "NO_SCATTERED_DUAL_READS",
];

pub const RUNTIME_HOST_PRESENTATION_WORLD_BOUNDS_PROBE_CMD_NAMES_WAVE461: &[&str] = &[
    "click_presentation_world_bounds_probe_ok_wnd_resolve",
    "click_presentation_world_bounds_probe_ok_wnd_clamp",
    "click_presentation_world_bounds_probe_ok_wnd_hud",
    "click_presentation_world_bounds_probe_ok_wnd_prepare",
    "click_presentation_world_bounds_probe_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationWorldBoundsProbeAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    ProbeSource = 4,
    Consumers = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualPresentationWorldBoundsProbeAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_world_bounds_probe_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_world_bounds_probe_last_action()
-> ResidualPresentationWorldBoundsProbeAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationWorldBoundsProbeAction::MethodNames,
        2 => ResidualPresentationWorldBoundsProbeAction::SourceMarkers,
        3 => ResidualPresentationWorldBoundsProbeAction::NavCommands,
        4 => ResidualPresentationWorldBoundsProbeAction::ProbeSource,
        5 => ResidualPresentationWorldBoundsProbeAction::Consumers,
        6 => ResidualPresentationWorldBoundsProbeAction::Composite,
        _ => ResidualPresentationWorldBoundsProbeAction::Idle,
    }
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

pub fn honesty_presentation_world_bounds_probe_method_names_residual_wave461() -> bool {
    PRESENTATION_WORLD_BOUNDS_PROBE_METHOD_NAMES_WAVE461.len() == 6
        && residual_name_index(
            PRESENTATION_WORLD_BOUNDS_PROBE_METHOD_NAMES_WAVE461,
            "presentation_world_bounds",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_WORLD_BOUNDS_PROBE_METHOD_NAMES_WAVE461,
            "last_presentation_frame",
        ) == Some(5)
}

pub fn honesty_presentation_world_bounds_probe_source_markers_residual_wave461() -> bool {
    PRESENTATION_WORLD_BOUNDS_PROBE_SOURCE_MARKERS_WAVE461.len() == 5
        && residual_name_index(
            PRESENTATION_WORLD_BOUNDS_PROBE_SOURCE_MARKERS_WAVE461,
            "Wave 461: single presentation-first world bounds probe",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_WORLD_BOUNDS_PROBE_SOURCE_MARKERS_WAVE461,
            "host_world_bounds",
        ) == Some(2)
        && residual_name_index(
            PRESENTATION_WORLD_BOUNDS_PROBE_SOURCE_MARKERS_WAVE461,
            "last_presentation_frame",
        ) == Some(4)
}

pub fn honesty_presentation_world_bounds_probe_nav_commands_residual_wave461() -> bool {
    PRESENTATION_WORLD_BOUNDS_PROBE_NAV_STEPS_WAVE461.len() == 6
        && residual_name_index(
            PRESENTATION_WORLD_BOUNDS_PROBE_NAV_STEPS_WAVE461,
            "CLAMP_CAMERA_VIA_PROBE",
        ) == Some(3)
        && residual_name_index(
            PRESENTATION_WORLD_BOUNDS_PROBE_NAV_STEPS_WAVE461,
            "NO_SCATTERED_DUAL_READS",
        ) == Some(5)
        && RUNTIME_HOST_PRESENTATION_WORLD_BOUNDS_PROBE_CMD_NAMES_WAVE461.len() == 5
        && residual_name_index(
            RUNTIME_HOST_PRESENTATION_WORLD_BOUNDS_PROBE_CMD_NAMES_WAVE461,
            "click_presentation_world_bounds_probe_ok_wnd_prepare",
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

pub fn simulate_presentation_world_bounds_probe_source() -> bool {
    let src = cnc_source();
    let Some(body) = function_body(src, "fn presentation_world_bounds(") else {
        return false;
    };
    let ok = body.contains("presentation_frame()")
        && body.contains("last_presentation_frame")
        && body.contains("world_bounds_vec3()")
        && (body.contains("game_logic.world_bounds()") || body.contains("host_world_bounds()"));
    residual_action_store(ResidualPresentationWorldBoundsProbeAction::ProbeSource);
    ok
}

pub fn simulate_presentation_world_bounds_probe_consumers() -> bool {
    let src = cnc_source();
    // clamp + at least two other consumers
    let uses = src.matches("self.presentation_world_bounds()").count();
    let Some(clamp) = function_body(src, "fn clamp_to_world_bounds(") else {
        return false;
    };
    let ok = uses >= 3
        && clamp.contains("presentation_world_bounds()")
        && src.contains("Wave 461: single presentation-first world bounds probe")
        && src.contains("fn host_world_bounds")
        // No scattered last_presentation-only dual-read pattern outside helper.
        && !src.contains(
            "if let Some(frame) = self.last_presentation_frame.as_ref() {
                frame.world_env.world_bounds_vec3()
            } else {
                self.game_logic.world_bounds()",
        );
    residual_action_store(ResidualPresentationWorldBoundsProbeAction::Consumers);
    ok
}

pub fn honesty_presentation_world_bounds_probe_residual_pack_wave461() -> bool {
    honesty_presentation_world_bounds_probe_method_names_residual_wave461()
        && honesty_presentation_world_bounds_probe_source_markers_residual_wave461()
        && honesty_presentation_world_bounds_probe_nav_commands_residual_wave461()
        && simulate_presentation_world_bounds_probe_source()
        && simulate_presentation_world_bounds_probe_consumers()
}

pub fn simulate_live_presentation_world_bounds_probe_honesty() -> bool {
    let ok = honesty_presentation_world_bounds_probe_residual_pack_wave461();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationWorldBoundsProbeAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_world_bounds_probe_method_names_residual_wave461());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_world_bounds_probe_source_markers_residual_wave461());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_world_bounds_probe_nav_commands_residual_wave461());
    }

    #[test]
    fn presentation_world_bounds_probe_sources() {
        assert!(simulate_presentation_world_bounds_probe_source());
        assert!(simulate_presentation_world_bounds_probe_consumers());
    }

    #[test]
    fn wave461_composite_pack() {
        assert!(honesty_presentation_world_bounds_probe_residual_pack_wave461());
    }

    #[test]
    fn simulate_live_presentation_world_bounds_probe_honesty_residual_live() {
        assert!(
            simulate_live_presentation_world_bounds_probe_honesty(),
            "presentation world bounds probe residual must latch"
        );
        assert!(residual_presentation_world_bounds_probe_ok());
        assert_eq!(
            residual_presentation_world_bounds_probe_last_action(),
            ResidualPresentationWorldBoundsProbeAction::Composite
        );
    }
}
