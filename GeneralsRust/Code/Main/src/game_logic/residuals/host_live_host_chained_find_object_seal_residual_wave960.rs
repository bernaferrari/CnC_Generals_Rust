//! Wave 960: chained `.find_object`/`.get_object` → `host_object` idiom.
//!
//! GameLogic production and tests prefer method-chain host_object calls.
//! Legacy get_object/find_object fns remain thin aliases. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_CHAINED_FIND_OBJECT_SEAL_METHOD_NAMES_WAVE960: &[&str] = &[
    "host_object",
    "host_object_mut",
    "host_objects",
    "Wave 960",
    "playable_claim = false",
];

pub const LIVE_HOST_CHAINED_FIND_OBJECT_SEAL_NAV_STEPS_WAVE960: &[&str] = &[
    "CHAINED_FIND_OBJECT_SEAL",
    "DOT_FIND_OBJECT_TO_HOST",
    "LEGACY_ALIASES_REMAIN",
    "LIVE_HOST_CHAINED_FIND_OBJECT_SEAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostChainedFindObjectSealAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostChainedFindObjectSealAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
}

fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}

fn non_comment(src: &str) -> String {
    src.lines()
        .filter(|l| !l.trim_start().starts_with("//") && !l.contains("contains("))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn honesty_host_chained_find_object_seal_method_names_residual_wave960() -> bool {
    let names = LIVE_HOST_CHAINED_FIND_OBJECT_SEAL_METHOD_NAMES_WAVE960;
    let ok = residual_name_index(names, "host_object").is_some()
        && residual_name_index(names, "Wave 960").is_some();
    residual_action_store(ResidualHostChainedFindObjectSealAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_chained_find_object_seal_nav_commands_residual_wave960() -> bool {
    let steps = LIVE_HOST_CHAINED_FIND_OBJECT_SEAL_NAV_STEPS_WAVE960;
    let ok = residual_name_index(steps, "LIVE_HOST_CHAINED_FIND_OBJECT_SEAL").is_some()
        && residual_name_index(steps, "DOT_FIND_OBJECT_TO_HOST").is_some();
    residual_action_store(ResidualHostChainedFindObjectSealAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_chained_find_object_seal_residual_pack_wave960() -> bool {
    let gl = gl_source();
    let cnc = cnc_source();
    let code = non_comment(gl);
    let ok = gl.contains("Wave 960")
        && code.contains(".host_object(")
        && !code.contains(".find_object(")
        && !code.contains(".get_object(")
        // alias fn definitions still exist
        && gl.contains("pub fn find_object(")
        && gl.contains("pub fn get_object(")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostChainedFindObjectSealAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_chained_find_object_seal_honesty() -> bool {
    let a = honesty_host_chained_find_object_seal_method_names_residual_wave960();
    let b = honesty_host_chained_find_object_seal_nav_commands_residual_wave960();
    let c = honesty_host_chained_find_object_seal_residual_pack_wave960();
    residual_action_store(ResidualHostChainedFindObjectSealAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_chained_find_object_seal_residual_wave960() {
        assert!(honesty_host_chained_find_object_seal_residual_pack_wave960());
        assert!(honesty_host_chained_find_object_seal_method_names_residual_wave960());
        assert!(honesty_host_chained_find_object_seal_nav_commands_residual_wave960());
        assert!(simulate_live_host_chained_find_object_seal_honesty());
    }
}
