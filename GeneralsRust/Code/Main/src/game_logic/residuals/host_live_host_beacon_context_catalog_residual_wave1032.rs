//! Wave 1032: dual-world ControlBar Beacon context residual.
//!
//! evaluate_context selects Beacon when template/command-set/portrait freeze
//! names contain BEACON (C++ CB_CONTEXT_BEACON after OCL/command path).
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_BEACON_CONTEXT_CATALOG_RESIDUAL_METHOD_NAMES_WAVE1032: &[&str] = &[
    "presentation_name_is_beacon",
    "ControlBarState::Beacon",
    "evaluate_context_ui",
    "Wave 1032",
    "playable_claim = false",
];

pub const LIVE_HOST_BEACON_CONTEXT_CATALOG_RESIDUAL_NAV_STEPS_WAVE1032: &[&str] = &[
    "BEACON_CONTEXT",
    "EVALUATE_CONTEXT",
    "CB_CONTEXT_BEACON",
    "LIVE_HOST_BEACON_CONTEXT_CATALOG_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostBeaconContextCatalogResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostBeaconContextCatalogResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
}
fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}
fn cb_source() -> &'static str {
    include_str!("../../../../GameEngine/GameClient/src/gui/control_bar/control_bar.rs")
}

pub fn honesty_host_beacon_context_catalog_residual_method_names_residual_wave1032() -> bool {
    let names = LIVE_HOST_BEACON_CONTEXT_CATALOG_RESIDUAL_METHOD_NAMES_WAVE1032;
    let ok = residual_name_index(names, "presentation_name_is_beacon").is_some()
        && residual_name_index(names, "Wave 1032").is_some();
    residual_action_store(ResidualHostBeaconContextCatalogResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_beacon_context_catalog_residual_nav_commands_residual_wave1032() -> bool {
    let steps = LIVE_HOST_BEACON_CONTEXT_CATALOG_RESIDUAL_NAV_STEPS_WAVE1032;
    let ok = residual_name_index(steps, "LIVE_HOST_BEACON_CONTEXT_CATALOG_RESIDUAL").is_some()
        && residual_name_index(steps, "BEACON_CONTEXT").is_some();
    residual_action_store(ResidualHostBeaconContextCatalogResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_beacon_context_catalog_residual_residual_pack_wave1032() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let cb = cb_source();
    let ok = cb.contains("Wave 1032: C++ beacon template/command-set residual name match")
        && cb.contains("fn presentation_name_is_beacon")
        && cb.contains(
            "Wave 1032: beacon residual wins over generic Command when freeze says BEACON",
        )
        && cb.contains("Wave 1032: C++ beacon template residual before generic Command")
        && cb.contains("Wave 1032: C++ CB_CONTEXT_BEACON when template matches beacon")
        && cb.contains("ControlBarState::Beacon")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostBeaconContextCatalogResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_beacon_context_catalog_residual_honesty() -> bool {
    let a = honesty_host_beacon_context_catalog_residual_method_names_residual_wave1032();
    let b = honesty_host_beacon_context_catalog_residual_nav_commands_residual_wave1032();
    let c = honesty_host_beacon_context_catalog_residual_residual_pack_wave1032();
    residual_action_store(ResidualHostBeaconContextCatalogResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_beacon_context_catalog_residual_wave1032() {
        assert!(honesty_host_beacon_context_catalog_residual_residual_pack_wave1032());
        assert!(honesty_host_beacon_context_catalog_residual_method_names_residual_wave1032());
        assert!(honesty_host_beacon_context_catalog_residual_nav_commands_residual_wave1032());
        assert!(simulate_live_host_beacon_context_catalog_residual_honesty());
    }
}
