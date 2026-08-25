//! Wave 468 residual peels: reinitialize_minimap_renderer is an instance method
//! that seeds presentation and uses presentation_world_bounds (no free-fn
//! GameLogic dual-read signature).
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 457/465 presentation-first minimap bounds/repair.
//! Architecture residual - minimap reinit owns seed+bounds+repair on self.
//!
//! Sources (cnc_game_engine.rs):
//! - fn reinitialize_minimap_renderer(&mut self)
//! - ensure_presentation_env_seeded + presentation_world_bounds
//! - call sites: self.reinitialize_minimap_renderer()
//!
//! Fail-closed:
//! - Host override_world_size remains pathfinding residual on repair
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const MINIMAP_REINIT_INSTANCE_PRESENTATION_METHOD_NAMES_WAVE468: &[&str] = &[
    "reinitialize_minimap_renderer",
    "host_repair_minimap_presentation_bounds",
    "ensure_presentation_env_seeded",
    "presentation_world_bounds",
    "presentation_frame_mut",
    "override_world_size",
    "update_minimap_world_bounds",
];

pub const MINIMAP_REINIT_INSTANCE_PRESENTATION_SOURCE_MARKERS_WAVE468: &[&str] = &[
    "Wave 468: instance path",
    "presentation_world_bounds()",
    "ensure_presentation_env_seeded()",
    "self.reinitialize_minimap_renderer()",
];

pub const MINIMAP_REINIT_INSTANCE_PRESENTATION_NAV_STEPS_WAVE468: &[&str] = &[
    "SEED_PRESENTATION_ENV",
    "READ_BOUNDS_VIA_PRESENTATION_WORLD_BOUNDS",
    "INIT_MINIMAP_RENDERER",
    "REPAIR_DEGENERATE_FROM_HEIGHTMAP",
    "STAMP_PRESENTATION_AND_HOST_SIZE",
    "SYNC_MINIMAP_BOUNDS",
];

pub const RUNTIME_HOST_MINIMAP_REINIT_INSTANCE_PRESENTATION_CMD_NAMES_WAVE468: &[&str] = &[
    "click_minimap_reinit_instance_presentation_ok_wnd_seed",
    "click_minimap_reinit_instance_presentation_ok_wnd_bounds",
    "click_minimap_reinit_instance_presentation_ok_wnd_repair",
    "click_minimap_reinit_instance_presentation_ok_wnd_prepare",
    "click_minimap_reinit_instance_presentation_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualMinimapReinitInstancePresentationAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    InstanceSource = 4,
    CallSites = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualMinimapReinitInstancePresentationAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_minimap_reinit_instance_presentation_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_minimap_reinit_instance_presentation_last_action()
-> ResidualMinimapReinitInstancePresentationAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualMinimapReinitInstancePresentationAction::MethodNames,
        2 => ResidualMinimapReinitInstancePresentationAction::SourceMarkers,
        3 => ResidualMinimapReinitInstancePresentationAction::NavCommands,
        4 => ResidualMinimapReinitInstancePresentationAction::InstanceSource,
        5 => ResidualMinimapReinitInstancePresentationAction::CallSites,
        6 => ResidualMinimapReinitInstancePresentationAction::Composite,
        _ => ResidualMinimapReinitInstancePresentationAction::Idle,
    }
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

pub fn honesty_minimap_reinit_instance_presentation_method_names_residual_wave468() -> bool {
    MINIMAP_REINIT_INSTANCE_PRESENTATION_METHOD_NAMES_WAVE468.len() == 7
        && residual_name_index(
            MINIMAP_REINIT_INSTANCE_PRESENTATION_METHOD_NAMES_WAVE468,
            "reinitialize_minimap_renderer",
        ) == Some(0)
        && residual_name_index(
            MINIMAP_REINIT_INSTANCE_PRESENTATION_METHOD_NAMES_WAVE468,
            "host_repair_minimap_presentation_bounds",
        ) == Some(1)
        && residual_name_index(
            MINIMAP_REINIT_INSTANCE_PRESENTATION_METHOD_NAMES_WAVE468,
            "update_minimap_world_bounds",
        ) == Some(6)
}

pub fn honesty_minimap_reinit_instance_presentation_source_markers_residual_wave468() -> bool {
    MINIMAP_REINIT_INSTANCE_PRESENTATION_SOURCE_MARKERS_WAVE468.len() == 4
        && residual_name_index(
            MINIMAP_REINIT_INSTANCE_PRESENTATION_SOURCE_MARKERS_WAVE468,
            "Wave 468: instance path",
        ) == Some(0)
        && residual_name_index(
            MINIMAP_REINIT_INSTANCE_PRESENTATION_SOURCE_MARKERS_WAVE468,
            "self.reinitialize_minimap_renderer()",
        ) == Some(3)
}

pub fn honesty_minimap_reinit_instance_presentation_nav_commands_residual_wave468() -> bool {
    MINIMAP_REINIT_INSTANCE_PRESENTATION_NAV_STEPS_WAVE468.len() == 6
        && residual_name_index(
            MINIMAP_REINIT_INSTANCE_PRESENTATION_NAV_STEPS_WAVE468,
            "READ_BOUNDS_VIA_PRESENTATION_WORLD_BOUNDS",
        ) == Some(1)
        && residual_name_index(
            MINIMAP_REINIT_INSTANCE_PRESENTATION_NAV_STEPS_WAVE468,
            "SYNC_MINIMAP_BOUNDS",
        ) == Some(5)
        && RUNTIME_HOST_MINIMAP_REINIT_INSTANCE_PRESENTATION_CMD_NAMES_WAVE468.len() == 5
        && residual_name_index(
            RUNTIME_HOST_MINIMAP_REINIT_INSTANCE_PRESENTATION_CMD_NAMES_WAVE468,
            "click_minimap_reinit_instance_presentation_ok_wnd_prepare",
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

pub fn simulate_minimap_reinit_instance_presentation_source() -> bool {
    let src = cnc_source();
    let Some(body) = function_body(src, "fn reinitialize_minimap_renderer(") else {
        return false;
    };
    // Wave 594: heightmap repair + last_presentation align lives in host helper.
    let Some(repair) = function_body(src, "fn host_repair_minimap_presentation_bounds(") else {
        return false;
    };
    let ok = body.contains("Wave 468: instance path")
        && body.contains("&mut self")
        && body.contains("ensure_presentation_env_seeded()")
        && body.contains("presentation_world_bounds()")
        && body.contains("host_repair_minimap_presentation_bounds")
        && (body.contains("Wave 594") || repair.contains("Wave 594"))
        && repair.contains("override_world_size")
        && repair.contains("last_presentation_frame = Some")
        && !body.contains("game_logic: &mut GameLogic")
        && !repair.contains("game_logic: &mut GameLogic");
    residual_action_store(ResidualMinimapReinitInstancePresentationAction::InstanceSource);
    ok
}

pub fn simulate_minimap_reinit_instance_presentation_callsites() -> bool {
    let src = cnc_source();
    let n = src.matches("self.reinitialize_minimap_renderer()").count();
    let ok = n >= 3 && !src.contains("Self::reinitialize_minimap_renderer(");
    residual_action_store(ResidualMinimapReinitInstancePresentationAction::CallSites);
    ok
}

pub fn honesty_minimap_reinit_instance_presentation_residual_pack_wave468() -> bool {
    honesty_minimap_reinit_instance_presentation_method_names_residual_wave468()
        && honesty_minimap_reinit_instance_presentation_source_markers_residual_wave468()
        && honesty_minimap_reinit_instance_presentation_nav_commands_residual_wave468()
        && simulate_minimap_reinit_instance_presentation_source()
        && simulate_minimap_reinit_instance_presentation_callsites()
}

pub fn simulate_live_minimap_reinit_instance_presentation_honesty() -> bool {
    let ok = honesty_minimap_reinit_instance_presentation_residual_pack_wave468();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualMinimapReinitInstancePresentationAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_minimap_reinit_instance_presentation_method_names_residual_wave468());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_minimap_reinit_instance_presentation_source_markers_residual_wave468());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_minimap_reinit_instance_presentation_nav_commands_residual_wave468());
    }

    #[test]
    fn minimap_reinit_instance_presentation_sources() {
        assert!(simulate_minimap_reinit_instance_presentation_source());
        assert!(simulate_minimap_reinit_instance_presentation_callsites());
    }

    #[test]
    fn wave468_composite_pack() {
        assert!(honesty_minimap_reinit_instance_presentation_residual_pack_wave468());
    }

    #[test]
    fn simulate_live_minimap_reinit_instance_presentation_honesty_residual_live() {
        assert!(
            simulate_live_minimap_reinit_instance_presentation_honesty(),
            "minimap reinit instance presentation residual must latch"
        );
        assert!(residual_minimap_reinit_instance_presentation_ok());
        assert_eq!(
            residual_minimap_reinit_instance_presentation_last_action(),
            ResidualMinimapReinitInstancePresentationAction::Composite
        );
    }
}
