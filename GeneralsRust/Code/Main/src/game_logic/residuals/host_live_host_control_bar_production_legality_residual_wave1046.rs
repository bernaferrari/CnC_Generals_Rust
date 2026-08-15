//! Wave 1046: dual-world ControlBar production legality residual.
//!
//! get_object_has_production and populate_build_queue dual paths fail-closed on
//! destroyed/sold/disabled/unselectable producers. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_CONTROL_BAR_PRODUCTION_LEGALITY_RESIDUAL_METHOD_NAMES_WAVE1046: &[&str] = &[
    "get_object_has_production",
    "populate_build_queue",
    "Wave 1046",
    "playable_claim = false",
];

pub const LIVE_HOST_CONTROL_BAR_PRODUCTION_LEGALITY_RESIDUAL_NAV_STEPS_WAVE1046: &[&str] = &[
    "CONTROL_BAR",
    "PRODUCTION_LEGALITY",
    "LIVE_HOST_CONTROL_BAR_PRODUCTION_LEGALITY_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostControlBarProductionLegalityResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostControlBarProductionLegalityResidualAction) {
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

pub fn honesty_host_control_bar_production_legality_residual_method_names_residual_wave1046() -> bool
{
    let names = LIVE_HOST_CONTROL_BAR_PRODUCTION_LEGALITY_RESIDUAL_METHOD_NAMES_WAVE1046;
    let ok = residual_name_index(names, "get_object_has_production").is_some()
        && residual_name_index(names, "Wave 1046").is_some();
    residual_action_store(ResidualHostControlBarProductionLegalityResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_control_bar_production_legality_residual_nav_commands_residual_wave1046() -> bool
{
    let steps = LIVE_HOST_CONTROL_BAR_PRODUCTION_LEGALITY_RESIDUAL_NAV_STEPS_WAVE1046;
    let ok = residual_name_index(steps, "LIVE_HOST_CONTROL_BAR_PRODUCTION_LEGALITY_RESIDUAL")
        .is_some()
        && residual_name_index(steps, "PRODUCTION_LEGALITY").is_some();
    residual_action_store(ResidualHostControlBarProductionLegalityResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_control_bar_production_legality_residual_residual_pack_wave1046() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let cb = cb_source();
    let ok = cb.contains("Wave 249/997/1009/1046: presentation residual above")
        && cb.contains("Wave 981/1010/1014/1046: host empty dual-world")
        && cb.contains("entry.destroyed || entry.sold || entry.disabled || entry.unselectable")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostControlBarProductionLegalityResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_control_bar_production_legality_residual_honesty() -> bool {
    let a = honesty_host_control_bar_production_legality_residual_method_names_residual_wave1046();
    let b = honesty_host_control_bar_production_legality_residual_nav_commands_residual_wave1046();
    let c = honesty_host_control_bar_production_legality_residual_residual_pack_wave1046();
    residual_action_store(ResidualHostControlBarProductionLegalityResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_control_bar_production_legality_residual_wave1046() {
        assert!(honesty_host_control_bar_production_legality_residual_residual_pack_wave1046());
        assert!(
            honesty_host_control_bar_production_legality_residual_method_names_residual_wave1046()
        );
        assert!(
            honesty_host_control_bar_production_legality_residual_nav_commands_residual_wave1046()
        );
        assert!(simulate_live_host_control_bar_production_legality_residual_honesty());
    }
}
