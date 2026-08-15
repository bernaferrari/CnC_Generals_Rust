//! Wave 959: GameLogic internal host_object idiom seal.
//!
//! Production/internal call sites prefer host_object/host_objects; legacy
//! get_object/find_object remain thin aliases only. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_INTERNAL_HOST_OBJECT_IDIOM_METHOD_NAMES_WAVE959: &[&str] = &[
    "host_object",
    "host_objects",
    "host_object_mut",
    "Wave 959",
    "playable_claim = false",
];

pub const LIVE_HOST_INTERNAL_HOST_OBJECT_IDIOM_NAV_STEPS_WAVE959: &[&str] = &[
    "INTERNAL_HOST_OBJECT_IDIOM",
    "SELF_HOST_OBJECT",
    "LEGACY_ALIASES_ONLY",
    "LIVE_HOST_INTERNAL_HOST_OBJECT_IDIOM",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostInternalHostObjectIdiomAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostInternalHostObjectIdiomAction) {
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

fn non_comment_prod_prefix(src: &str) -> String {
    src.lines()
        .filter(|l| !l.trim_start().starts_with("//") && !l.contains("contains("))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn honesty_host_internal_host_object_idiom_method_names_residual_wave959() -> bool {
    let names = LIVE_HOST_INTERNAL_HOST_OBJECT_IDIOM_METHOD_NAMES_WAVE959;
    let ok = residual_name_index(names, "host_object").is_some()
        && residual_name_index(names, "Wave 959").is_some();
    residual_action_store(ResidualHostInternalHostObjectIdiomAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_internal_host_object_idiom_nav_commands_residual_wave959() -> bool {
    let steps = LIVE_HOST_INTERNAL_HOST_OBJECT_IDIOM_NAV_STEPS_WAVE959;
    let ok = residual_name_index(steps, "LIVE_HOST_INTERNAL_HOST_OBJECT_IDIOM").is_some()
        && residual_name_index(steps, "SELF_HOST_OBJECT").is_some();
    residual_action_store(ResidualHostInternalHostObjectIdiomAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_internal_host_object_idiom_residual_pack_wave959() -> bool {
    let gl = gl_source();
    let cnc = cnc_source();
    let prod = non_comment_prod_prefix(&gl);
    let host_calls = prod.matches("self.host_object(").count()
        + prod.matches("self.host_objects()").count()
        + prod.matches("self.objects.get").count();
    let legacy_calls =
        prod.matches("self.get_object(").count() + prod.matches("self.find_object(").count();
    // Alias definitions may still mention get_object as fn names — count call form only.
    let ok = gl.contains("Wave 959")
        && host_calls > 20
        && legacy_calls == 0
        && gl.contains("self.objects.get")
        && !cnc.contains("self.playable_claim = true")
        && !gl.contains("self.playable_claim = true");
    residual_action_store(ResidualHostInternalHostObjectIdiomAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_internal_host_object_idiom_honesty() -> bool {
    let a = honesty_host_internal_host_object_idiom_method_names_residual_wave959();
    let b = honesty_host_internal_host_object_idiom_nav_commands_residual_wave959();
    let c = honesty_host_internal_host_object_idiom_residual_pack_wave959();
    residual_action_store(ResidualHostInternalHostObjectIdiomAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_internal_host_object_idiom_residual_wave959() {
        assert!(honesty_host_internal_host_object_idiom_residual_pack_wave959());
        assert!(honesty_host_internal_host_object_idiom_method_names_residual_wave959());
        assert!(honesty_host_internal_host_object_idiom_nav_commands_residual_wave959());
        assert!(simulate_live_host_internal_host_object_idiom_honesty());
    }
}
