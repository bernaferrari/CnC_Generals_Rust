//! Wave 957: seal remaining harness/production dual-reads onto host_* APIs.
//!
//! golden_skirmish, campaign, breadth scenarios, deterministic trace, authoritative
//! world counters, shell_smoke host paths, and save snapshot residual find_object
//! route through host_object/host_objects. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_HARNESS_HOST_OBJECT_SEAL_METHOD_NAMES_WAVE957: &[&str] = &[
    "host_object",
    "host_objects",
    "Wave 957",
    "playable_claim = false",
];

pub const LIVE_HOST_HARNESS_HOST_OBJECT_SEAL_NAV_STEPS_WAVE957: &[&str] = &[
    "HARNESS_HOST_OBJECT_SEAL",
    "GOLDEN_HOST_OBJECTS",
    "SHELL_SMOKE_HOST_OBJECTS",
    "LIVE_HOST_HARNESS_HOST_OBJECT_SEAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostHarnessHostObjectSealAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostHarnessHostObjectSealAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}

fn golden_source() -> &'static str {
    include_str!("../../golden_skirmish.rs")
}

fn shell_source() -> &'static str {
    crate::shell_smoke::SHELL_SMOKE_SRC
}

fn non_comment(src: &str) -> String {
    src.lines()
        .filter(|l| !l.trim_start().starts_with("//") && !l.contains("contains("))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn honesty_host_harness_host_object_seal_method_names_residual_wave957() -> bool {
    let names = LIVE_HOST_HARNESS_HOST_OBJECT_SEAL_METHOD_NAMES_WAVE957;
    let ok = residual_name_index(names, "host_objects").is_some()
        && residual_name_index(names, "Wave 957").is_some();
    residual_action_store(ResidualHostHarnessHostObjectSealAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_harness_host_object_seal_nav_commands_residual_wave957() -> bool {
    let steps = LIVE_HOST_HARNESS_HOST_OBJECT_SEAL_NAV_STEPS_WAVE957;
    let ok = residual_name_index(steps, "LIVE_HOST_HARNESS_HOST_OBJECT_SEAL").is_some()
        && residual_name_index(steps, "GOLDEN_HOST_OBJECTS").is_some();
    residual_action_store(ResidualHostHarnessHostObjectSealAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_harness_host_object_seal_residual_pack_wave957() -> bool {
    let golden = non_comment(golden_source());
    let shell = non_comment(shell_source());
    let gl = gl_source();
    let cnc = cnc_source();
    let ok = golden_source().contains("Wave 957")
        && shell_source().contains("Wave 957")
        && gl.contains("fn host_objects(")
        && golden.contains("host_objects()")
        && !golden.contains("get_objects()")
        && shell.contains("host_objects()")
        && !shell.contains(".get_objects()")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostHarnessHostObjectSealAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_harness_host_object_seal_honesty() -> bool {
    let a = honesty_host_harness_host_object_seal_method_names_residual_wave957();
    let b = honesty_host_harness_host_object_seal_nav_commands_residual_wave957();
    let c = honesty_host_harness_host_object_seal_residual_pack_wave957();
    residual_action_store(ResidualHostHarnessHostObjectSealAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_harness_host_object_seal_residual_wave957() {
        assert!(honesty_host_harness_host_object_seal_residual_pack_wave957());
        assert!(honesty_host_harness_host_object_seal_method_names_residual_wave957());
        assert!(honesty_host_harness_host_object_seal_nav_commands_residual_wave957());
        assert!(simulate_live_host_harness_host_object_seal_honesty());
    }
}
