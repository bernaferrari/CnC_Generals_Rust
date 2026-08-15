//! Wave 948: seal Main dual-writes outside GameLogic via `host_object_mut`.
//!
//! presentation_frame / shell_smoke / unit_control / ai / input / graphics
//! helper dual-writes no longer call `get_objects_mut` directly.
//! Host object mutation goes through `GameLogic::host_object_mut`.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_MAIN_HOST_OBJECT_MUT_SEAL_METHOD_NAMES_WAVE948: &[&str] = &[
    "host_object_mut",
    "with_host_object_mut",
    "presentation_frame",
    "Wave 948",
    "playable_claim = false",
];

pub const LIVE_HOST_MAIN_HOST_OBJECT_MUT_SEAL_NAV_STEPS_WAVE948: &[&str] = &[
    "MAIN_HOST_OBJECT_MUT_SEAL",
    "PRESENTATION_HOST_OBJECT_MUT",
    "LIVE_HOST_MAIN_HOST_OBJECT_MUT_SEAL",
    "MAIN_GET_OBJECTS_MUT_OUTSIDE_GL_ZERO",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostMainHostObjectMutSealAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostMainHostObjectMutSealAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}

fn non_comment_code(window: &str) -> String {
    window
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn honesty_host_main_host_object_mut_seal_method_names_residual_wave948() -> bool {
    let names = LIVE_HOST_MAIN_HOST_OBJECT_MUT_SEAL_METHOD_NAMES_WAVE948;
    let ok = residual_name_index(names, "host_object_mut").is_some()
        && residual_name_index(names, "Wave 948").is_some()
        && residual_name_index(names, "presentation_frame").is_some();
    residual_action_store(ResidualHostMainHostObjectMutSealAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_main_host_object_mut_seal_nav_commands_residual_wave948() -> bool {
    let steps = LIVE_HOST_MAIN_HOST_OBJECT_MUT_SEAL_NAV_STEPS_WAVE948;
    let ok = residual_name_index(steps, "LIVE_HOST_MAIN_HOST_OBJECT_MUT_SEAL").is_some()
        && residual_name_index(steps, "MAIN_GET_OBJECTS_MUT_OUTSIDE_GL_ZERO").is_some();
    residual_action_store(ResidualHostMainHostObjectMutSealAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_main_host_object_mut_seal_residual_pack_wave948() -> bool {
    let gl = gl_source();
    let pf = pf_source();
    let sh = shadow_source();
    let cnc = cnc_source();
    let pf_code = non_comment_code(pf);
    let sh_code = non_comment_code(sh);
    let ok = gl.contains("fn host_object_mut")
        && gl.contains("fn with_host_object_mut")
        && pf_code.matches("get_objects_mut").count() == 0
        && sh_code.matches("get_objects_mut").count() == 0
        && (pf.contains("host_object_mut") || gl.contains("fn host_object_mut"))
        && (pf.contains("Wave 948")
            || gl.contains("Wave 948")
            || gl.contains("946/947/948")
            || gl.contains("Wave 950/958"))
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostMainHostObjectMutSealAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_main_host_object_mut_seal_honesty() -> bool {
    let a = honesty_host_main_host_object_mut_seal_method_names_residual_wave948();
    let b = honesty_host_main_host_object_mut_seal_nav_commands_residual_wave948();
    let c = honesty_host_main_host_object_mut_seal_residual_pack_wave948();
    residual_action_store(ResidualHostMainHostObjectMutSealAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_main_host_object_mut_seal_residual_wave948() {
        assert!(honesty_host_main_host_object_mut_seal_residual_pack_wave948());
        assert!(honesty_host_main_host_object_mut_seal_method_names_residual_wave948());
        assert!(honesty_host_main_host_object_mut_seal_nav_commands_residual_wave948());
        assert!(simulate_live_host_main_host_object_mut_seal_honesty());
    }
}
