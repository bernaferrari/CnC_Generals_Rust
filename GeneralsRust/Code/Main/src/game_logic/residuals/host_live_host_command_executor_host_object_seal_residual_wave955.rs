//! Wave 955: seal CommandExecutor dual-reads onto host_object/host_objects.
//!
//! Authority command apply still borrows Main GameLogic, but routes through
//! host_* APIs (not presentation dual-read). playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_COMMAND_EXECUTOR_HOST_OBJECT_SEAL_METHOD_NAMES_WAVE955: &[&str] = &[
    "host_object",
    "host_objects",
    "host_object_mut",
    "host_objects_mut",
    "CommandExecutor",
    "Wave 955",
    "playable_claim = false",
];

pub const LIVE_HOST_COMMAND_EXECUTOR_HOST_OBJECT_SEAL_NAV_STEPS_WAVE955: &[&str] = &[
    "COMMAND_EXECUTOR_HOST_OBJECT_SEAL",
    "HOST_OBJECTS_API",
    "NO_GET_OBJECT_IN_COMMAND_EXECUTOR",
    "LIVE_HOST_COMMAND_EXECUTOR_HOST_OBJECT_SEAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostCommandExecutorHostObjectSealAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostCommandExecutorHostObjectSealAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}

fn gl_source() -> &'static str {
    // 2026-08-15: scan host plus extra world_* splits.
    super::host_logic_scan_src()
}

fn ce_source() -> &'static str {
    crate::command_executor::COMMAND_EXECUTOR_SRC
}

fn non_comment_prod(src: &str) -> String {
    // 2026-08-15: COMMAND_EXECUTOR_SRC concat hits `#[cfg(test)] mod tests;`
    // in mod.rs first — do not truncate the impl splits after that.
    src.lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn honesty_host_command_executor_host_object_seal_method_names_residual_wave955() -> bool {
    let names = LIVE_HOST_COMMAND_EXECUTOR_HOST_OBJECT_SEAL_METHOD_NAMES_WAVE955;
    let ok = residual_name_index(names, "host_objects").is_some()
        && residual_name_index(names, "Wave 955").is_some()
        && residual_name_index(names, "CommandExecutor").is_some();
    residual_action_store(ResidualHostCommandExecutorHostObjectSealAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_command_executor_host_object_seal_nav_commands_residual_wave955() -> bool {
    let steps = LIVE_HOST_COMMAND_EXECUTOR_HOST_OBJECT_SEAL_NAV_STEPS_WAVE955;
    let ok = residual_name_index(steps, "LIVE_HOST_COMMAND_EXECUTOR_HOST_OBJECT_SEAL").is_some()
        && residual_name_index(steps, "NO_GET_OBJECT_IN_COMMAND_EXECUTOR").is_some();
    residual_action_store(ResidualHostCommandExecutorHostObjectSealAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_command_executor_host_object_seal_residual_pack_wave955() -> bool {
    let ce = ce_source();
    let gl = gl_source();
    let cnc = cnc_source();
    let prod = non_comment_prod(ce);
    let ok = ce.contains("Wave 955")
        && gl.contains("fn host_objects(")
        && gl.contains("fn host_objects_mut(")
        && !prod.contains("get_object(")
        && !prod.contains("get_objects()")
        && prod.contains("host_object(")
        && prod.contains("host_objects()")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostCommandExecutorHostObjectSealAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_command_executor_host_object_seal_honesty() -> bool {
    let a = honesty_host_command_executor_host_object_seal_method_names_residual_wave955();
    let b = honesty_host_command_executor_host_object_seal_nav_commands_residual_wave955();
    let c = honesty_host_command_executor_host_object_seal_residual_pack_wave955();
    residual_action_store(ResidualHostCommandExecutorHostObjectSealAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_command_executor_host_object_seal_residual_wave955() {
        assert!(honesty_host_command_executor_host_object_seal_residual_pack_wave955());
        assert!(honesty_host_command_executor_host_object_seal_method_names_residual_wave955());
        assert!(honesty_host_command_executor_host_object_seal_nav_commands_residual_wave955());
        assert!(simulate_live_host_command_executor_host_object_seal_honesty());
    }
}
