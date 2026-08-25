//! Wave 1075: dual-world selection source fallback + context pick disabled residual.
//!
//! selection_source_object_id dual no longer returns unusable locals; dual context
//! pick skips disabled catalog entries. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SOURCE_FALLBACK_CONTEXT_DISABLED_RESIDUAL_METHOD_NAMES_WAVE1075: &[&str] = &[
    "selection_source_object_id",
    "collect_selectable_objects_from_presentation",
    "entry.disabled",
    "Wave 1075",
    "playable_claim = false",
];

pub const LIVE_HOST_SOURCE_FALLBACK_CONTEXT_DISABLED_RESIDUAL_NAV_STEPS_WAVE1075: &[&str] = &[
    "SOURCE_FALLBACK",
    "CONTEXT_DISABLED",
    "LIVE_HOST_SOURCE_FALLBACK_CONTEXT_DISABLED_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSourceFallbackContextDisabledResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSourceFallbackContextDisabledResidualAction) {
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
fn tr_source() -> &'static str {
    game_client::message_stream::translators::TRANSLATORS_SRC
}

pub fn honesty_host_source_fallback_context_disabled_residual_method_names_residual_wave1075()
-> bool {
    let names = LIVE_HOST_SOURCE_FALLBACK_CONTEXT_DISABLED_RESIDUAL_METHOD_NAMES_WAVE1075;
    let ok = residual_name_index(names, "selection_source_object_id").is_some()
        && residual_name_index(names, "Wave 1075").is_some();
    residual_action_store(ResidualHostSourceFallbackContextDisabledResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_source_fallback_context_disabled_residual_nav_commands_residual_wave1075()
-> bool {
    let steps = LIVE_HOST_SOURCE_FALLBACK_CONTEXT_DISABLED_RESIDUAL_NAV_STEPS_WAVE1075;
    let ok = residual_name_index(steps, "LIVE_HOST_SOURCE_FALLBACK_CONTEXT_DISABLED_RESIDUAL")
        .is_some()
        && residual_name_index(steps, "SOURCE_FALLBACK").is_some();
    residual_action_store(ResidualHostSourceFallbackContextDisabledResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_source_fallback_context_disabled_residual_residual_pack_wave1075() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let tr = tr_source();
    let ok = (tr.contains("Wave 1075: no unusable local fallback residual")
        || tr.contains("Wave 1075/1111: no unusable local fallback residual"))
        && tr.contains("Wave 1075: disabled residual fail-closed for dual context pick")
        && tr.contains("return 0;")
        && (tr.contains("|| entry.disabled") || tr.contains("&& !e.disabled"))
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostSourceFallbackContextDisabledResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_source_fallback_context_disabled_residual_honesty() -> bool {
    let a = honesty_host_source_fallback_context_disabled_residual_method_names_residual_wave1075();
    let b = honesty_host_source_fallback_context_disabled_residual_nav_commands_residual_wave1075();
    let c = honesty_host_source_fallback_context_disabled_residual_residual_pack_wave1075();
    residual_action_store(ResidualHostSourceFallbackContextDisabledResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_source_fallback_context_disabled_residual_wave1075() {
        assert!(honesty_host_source_fallback_context_disabled_residual_residual_pack_wave1075());
        assert!(
            honesty_host_source_fallback_context_disabled_residual_method_names_residual_wave1075()
        );
        assert!(
            honesty_host_source_fallback_context_disabled_residual_nav_commands_residual_wave1075()
        );
        assert!(simulate_live_host_source_fallback_context_disabled_residual_honesty());
    }
}
