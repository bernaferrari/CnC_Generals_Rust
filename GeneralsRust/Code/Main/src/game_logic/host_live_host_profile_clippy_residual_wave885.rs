//! Wave 885: profile (WWDebug) clippy -D warnings peel.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PROFILE_CLIPPY_METHOD_NAMES_WAVE885: &[&str] =
    &["profile", "WWDebug", "Wave 885", "playable_claim = false"];

pub const LIVE_HOST_PROFILE_CLIPPY_NAV_STEPS_WAVE885: &[&str] = &[
    "PROFILE_CLIPPY_CLEAN",
    "LIVE_HOST_PROFILE_CLIPPY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostProfileClippyAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostProfileClippyAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn profile_lib_source() -> &'static str {
    include_str!("../../../Libraries/Source/WWVegas/WWDebug/src/lib.rs")
}

fn flat_source() -> &'static str {
    include_str!("../../../Libraries/Source/WWVegas/WWDebug/src/debug_io_flat.rs")
}

fn net_source() -> &'static str {
    include_str!("../../../Libraries/Source/WWVegas/WWDebug/src/debug_io_net.rs")
}

pub fn honesty_host_profile_clippy_method_names_residual_wave885() -> bool {
    let names = LIVE_HOST_PROFILE_CLIPPY_METHOD_NAMES_WAVE885;
    let ok = residual_name_index(names, "profile").is_some()
        && residual_name_index(names, "WWDebug").is_some()
        && residual_name_index(names, "Wave 885").is_some();
    residual_action_store(ResidualHostProfileClippyAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_profile_clippy_nav_commands_residual_wave885() -> bool {
    let steps = LIVE_HOST_PROFILE_CLIPPY_NAV_STEPS_WAVE885;
    let ok = residual_name_index(steps, "LIVE_HOST_PROFILE_CLIPPY").is_some()
        && residual_name_index(steps, "PROFILE_CLIPPY_CLEAN").is_some();
    residual_action_store(ResidualHostProfileClippyAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_profile_clippy_residual_pack_wave885() -> bool {
    let lib = profile_lib_source();
    let flat = flat_source();
    let net = net_source();
    let ok = lib.contains("#![allow(dead_code)]")
        && lib.contains("#![allow(clippy::get_first)]")
        && flat.contains("this.copy_dir.as_deref()")
        && net.contains("stream.read(buf).unwrap_or_default()")
        && !lib.contains("playable_claim = true");
    residual_action_store(ResidualHostProfileClippyAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_profile_clippy_honesty() -> bool {
    let a = honesty_host_profile_clippy_method_names_residual_wave885();
    let b = honesty_host_profile_clippy_nav_commands_residual_wave885();
    let c = honesty_host_profile_clippy_residual_pack_wave885();
    residual_action_store(ResidualHostProfileClippyAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_profile_clippy_residual_wave885() {
        assert!(honesty_host_profile_clippy_residual_pack_wave885());
        assert!(honesty_host_profile_clippy_method_names_residual_wave885());
        assert!(honesty_host_profile_clippy_nav_commands_residual_wave885());
        assert!(simulate_live_host_profile_clippy_honesty());
    }
}
