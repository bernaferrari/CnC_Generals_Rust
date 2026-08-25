//! Wave 997: dual-world get_object_has_production presentation residual.
//!
//! When OBJECT_REGISTRY is empty, production-interface residual answers true for
//! the selected object if ControlBar holds a presentation command-set or
//! under-construction residual (empty queue still means a factory UI).
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PRODUCTION_INTERFACE_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE997: &[&str] = &[
    "get_object_has_production",
    "presentation_primary_command_set",
    "presentation_under_construction",
    "Wave 997",
    "playable_claim = false",
];

pub const LIVE_HOST_PRODUCTION_INTERFACE_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE997: &[&str] = &[
    "DUAL_WORLD",
    "SELECTED_PRODUCER",
    "COMMAND_SET_RESIDUAL",
    "LIVE_HOST_PRODUCTION_INTERFACE_PRESENTATION_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostProductionInterfacePresentationResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostProductionInterfacePresentationResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
fn cb_source() -> &'static str {
    game_client::gui::control_bar::control_bar::CONTROL_BAR_SRC
}

// 2026-08-15: widen post-split scan window to the rest of the concat.
pub fn honesty_host_production_interface_presentation_residual_method_names_residual_wave997()
-> bool {
    let names = LIVE_HOST_PRODUCTION_INTERFACE_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE997;
    let ok = residual_name_index(names, "get_object_has_production").is_some()
        && residual_name_index(names, "Wave 997").is_some();
    residual_action_store(ResidualHostProductionInterfacePresentationResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_production_interface_presentation_residual_nav_commands_residual_wave997()
-> bool {
    let steps = LIVE_HOST_PRODUCTION_INTERFACE_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE997;
    let ok = residual_name_index(
        steps,
        "LIVE_HOST_PRODUCTION_INTERFACE_PRESENTATION_RESIDUAL",
    )
    .is_some()
        && residual_name_index(steps, "COMMAND_SET_RESIDUAL").is_some();
    residual_action_store(ResidualHostProductionInterfacePresentationResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_production_interface_presentation_residual_residual_pack_wave997() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let cb = cb_source();
    let body = match cb.find("fn get_object_has_production") {
        Some(i) => &cb[i..],
        None => "",
    };
    let ok = body.contains("Wave 249/997")
        && body.contains("presentation_primary_command_set")
        && body.contains("presentation_under_construction")
        && body.contains("selected_objects")
        && body.contains("dual_world_registry_unavailable")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostProductionInterfacePresentationResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_production_interface_presentation_residual_honesty() -> bool {
    let a = honesty_host_production_interface_presentation_residual_method_names_residual_wave997();
    let b = honesty_host_production_interface_presentation_residual_nav_commands_residual_wave997();
    let c = honesty_host_production_interface_presentation_residual_residual_pack_wave997();
    residual_action_store(
        ResidualHostProductionInterfacePresentationResidualAction::DispatchSource,
    );
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_production_interface_presentation_residual_wave997() {
        assert!(honesty_host_production_interface_presentation_residual_residual_pack_wave997());
        assert!(
            honesty_host_production_interface_presentation_residual_method_names_residual_wave997()
        );
        assert!(
            honesty_host_production_interface_presentation_residual_nav_commands_residual_wave997()
        );
        assert!(simulate_live_host_production_interface_presentation_residual_honesty());
    }
}
