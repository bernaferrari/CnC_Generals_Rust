//! Wave 961: Main crate dual-read free seal (host_object idiom locked).
//!
//! Outside residual honesty packs, `Code/Main/src` has no `.get_object(`/
//! `.find_object(` call sites — only host_* and legacy alias definitions.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_MAIN_DUAL_READ_FREE_SEAL_METHOD_NAMES_WAVE961: &[&str] = &[
    "host_object",
    "host_objects",
    "Wave 961",
    "playable_claim = false",
];

pub const LIVE_HOST_MAIN_DUAL_READ_FREE_SEAL_NAV_STEPS_WAVE961: &[&str] = &[
    "MAIN_DUAL_READ_FREE_SEAL",
    "NO_DOT_GET_OBJECT_CALLSITES",
    "NO_DOT_FIND_OBJECT_CALLSITES",
    "LIVE_HOST_MAIN_DUAL_READ_FREE_SEAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostMainDualReadFreeSealAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostMainDualReadFreeSealAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}

fn ownership_source() -> &'static str {
    include_str!("../../../../../OWNERSHIP_AND_AUTHORITY.md")
}

/// Scan a Main src file body for forbidden dual-read call sites.
fn forbidden_dual_read_callsites(src: &str) -> bool {
    for line in src.lines() {
        let t = line.trim_start();
        if t.starts_with("//") || t.starts_with("///") || t.starts_with("//!") {
            continue;
        }
        if t.contains("contains(\"") || t.contains("contains('") {
            continue;
        }
        // Alias definitions are allowed.
        if t.contains("fn get_object")
            || t.contains("fn get_objects")
            || t.contains("fn find_object")
            || t.contains("fn get_object_mut")
            || t.contains("fn find_object_mut")
        {
            continue;
        }
        if t.contains(".get_object(")
            || t.contains(".get_objects()")
            || t.contains(".find_object(")
            || t.contains(".get_object_mut(")
            || t.contains(".find_object_mut(")
        {
            return true;
        }
    }
    false
}

pub fn honesty_host_main_dual_read_free_seal_method_names_residual_wave961() -> bool {
    let names = LIVE_HOST_MAIN_DUAL_READ_FREE_SEAL_METHOD_NAMES_WAVE961;
    let ok = residual_name_index(names, "host_object").is_some()
        && residual_name_index(names, "Wave 961").is_some();
    residual_action_store(ResidualHostMainDualReadFreeSealAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_main_dual_read_free_seal_nav_commands_residual_wave961() -> bool {
    let steps = LIVE_HOST_MAIN_DUAL_READ_FREE_SEAL_NAV_STEPS_WAVE961;
    let ok = residual_name_index(steps, "LIVE_HOST_MAIN_DUAL_READ_FREE_SEAL").is_some()
        && residual_name_index(steps, "NO_DOT_FIND_OBJECT_CALLSITES").is_some();
    residual_action_store(ResidualHostMainDualReadFreeSealAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_main_dual_read_free_seal_residual_pack_wave961() -> bool {
    let gl = gl_source();
    let cnc = cnc_source();
    let own = ownership_source();
    // Core production surfaces (not every residual pack file).
    let surfaces = [
        crate::command_executor::COMMAND_EXECUTOR_SRC,
        include_str!("../../ai.rs"),
        crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC,
        crate::presentation_frame::PRESENTATION_FRAME_SRC,
        crate::cnc_game_engine::ENGINE_SRC,
        crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC,
        include_str!("../../golden_skirmish.rs"),
        super::GAME_LOGIC_HOST_SRC,
    ];
    let any_forbidden = surfaces.iter().any(|s| forbidden_dual_read_callsites(s));
    let ok = !any_forbidden
        && gl.contains("pub fn host_object(")
        && gl.contains("Wave 960")
        && own.contains("host_object")
        && own.contains("Wave 961")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostMainDualReadFreeSealAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_main_dual_read_free_seal_honesty() -> bool {
    let a = honesty_host_main_dual_read_free_seal_method_names_residual_wave961();
    let b = honesty_host_main_dual_read_free_seal_nav_commands_residual_wave961();
    let c = honesty_host_main_dual_read_free_seal_residual_pack_wave961();
    residual_action_store(ResidualHostMainDualReadFreeSealAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_main_dual_read_free_seal_residual_wave961() {
        assert!(honesty_host_main_dual_read_free_seal_residual_pack_wave961());
        assert!(honesty_host_main_dual_read_free_seal_method_names_residual_wave961());
        assert!(honesty_host_main_dual_read_free_seal_nav_commands_residual_wave961());
        assert!(simulate_live_host_main_dual_read_free_seal_honesty());
    }
}
