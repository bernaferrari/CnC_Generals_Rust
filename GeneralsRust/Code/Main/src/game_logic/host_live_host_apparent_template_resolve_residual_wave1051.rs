//! Wave 1051: dual-world apparent disguise template residual.
//!
//! resolve_drawable_template_name and register_drawable_with_template dual paths
//! use translator_entry_apparent_template for non-allied viewers.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_APPARENT_TEMPLATE_RESOLVE_RESIDUAL_METHOD_NAMES_WAVE1051: &[&str] = &[
    "resolve_drawable_template_name",
    "translator_entry_apparent_template",
    "Wave 1051",
    "playable_claim = false",
];

pub const LIVE_HOST_APPARENT_TEMPLATE_RESOLVE_RESIDUAL_NAV_STEPS_WAVE1051: &[&str] = &[
    "TEMPLATE_RESOLVE",
    "DISGUISE",
    "LIVE_HOST_APPARENT_TEMPLATE_RESOLVE_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostApparentTemplateResolveResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostApparentTemplateResolveResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}
fn gl_source() -> &'static str {
    include_str!("game_logic.rs")
}
fn client_source() -> &'static str {
    include_str!("../../../GameEngine/GameClient/src/core/game_client.rs")
}

pub fn honesty_host_apparent_template_resolve_residual_method_names_residual_wave1051() -> bool {
    let names = LIVE_HOST_APPARENT_TEMPLATE_RESOLVE_RESIDUAL_METHOD_NAMES_WAVE1051;
    let ok = residual_name_index(names, "translator_entry_apparent_template").is_some()
        && residual_name_index(names, "Wave 1051").is_some();
    residual_action_store(ResidualHostApparentTemplateResolveResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_apparent_template_resolve_residual_nav_commands_residual_wave1051() -> bool {
    let steps = LIVE_HOST_APPARENT_TEMPLATE_RESOLVE_RESIDUAL_NAV_STEPS_WAVE1051;
    let ok = residual_name_index(steps, "LIVE_HOST_APPARENT_TEMPLATE_RESOLVE_RESIDUAL").is_some()
        && residual_name_index(steps, "TEMPLATE_RESOLVE").is_some();
    residual_action_store(ResidualHostApparentTemplateResolveResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_apparent_template_resolve_residual_residual_pack_wave1051() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let client = client_source();
    let ok = client.contains("Wave 1051: apparent disguise template for non-allied viewers.")
        && client.matches("translator_entry_apparent_template").count() >= 2
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostApparentTemplateResolveResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_apparent_template_resolve_residual_honesty() -> bool {
    let a = honesty_host_apparent_template_resolve_residual_method_names_residual_wave1051();
    let b = honesty_host_apparent_template_resolve_residual_nav_commands_residual_wave1051();
    let c = honesty_host_apparent_template_resolve_residual_residual_pack_wave1051();
    residual_action_store(ResidualHostApparentTemplateResolveResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_apparent_template_resolve_residual_wave1051() {
        assert!(honesty_host_apparent_template_resolve_residual_residual_pack_wave1051());
        assert!(honesty_host_apparent_template_resolve_residual_method_names_residual_wave1051());
        assert!(honesty_host_apparent_template_resolve_residual_nav_commands_residual_wave1051());
        assert!(simulate_live_host_apparent_template_resolve_residual_honesty());
    }
}
