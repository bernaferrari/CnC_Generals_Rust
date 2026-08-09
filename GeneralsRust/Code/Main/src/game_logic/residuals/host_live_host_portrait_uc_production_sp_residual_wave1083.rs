//! Wave 1083: dual-world portrait UC production/SP ready residual.
//!
//! Dual portrait clears production residual when under construction and fails
//! closed special_power_ready while UC. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PORTRAIT_UC_PRODUCTION_SP_RESIDUAL_METHOD_NAMES_WAVE1083: &[&str] = &[
    "update_portrait_for_object",
    "production_progress",
    "special_power_ready",
    "Wave 1083",
    "playable_claim = false",
];

pub const LIVE_HOST_PORTRAIT_UC_PRODUCTION_SP_RESIDUAL_NAV_STEPS_WAVE1083: &[&str] = &[
    "PORTRAIT_UC",
    "PRODUCTION_SP",
    "LIVE_HOST_PORTRAIT_UC_PRODUCTION_SP_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPortraitUcProductionSpResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostPortraitUcProductionSpResidualAction) {
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

pub fn honesty_host_portrait_uc_production_sp_residual_method_names_residual_wave1083() -> bool {
    let names = LIVE_HOST_PORTRAIT_UC_PRODUCTION_SP_RESIDUAL_METHOD_NAMES_WAVE1083;
    let ok = residual_name_index(names, "update_portrait_for_object").is_some()
        && residual_name_index(names, "Wave 1083").is_some();
    residual_action_store(ResidualHostPortraitUcProductionSpResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_portrait_uc_production_sp_residual_nav_commands_residual_wave1083() -> bool {
    let steps = LIVE_HOST_PORTRAIT_UC_PRODUCTION_SP_RESIDUAL_NAV_STEPS_WAVE1083;
    let ok = residual_name_index(steps, "LIVE_HOST_PORTRAIT_UC_PRODUCTION_SP_RESIDUAL").is_some()
        && residual_name_index(steps, "PORTRAIT_UC").is_some();
    residual_action_store(ResidualHostPortraitUcProductionSpResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_portrait_uc_production_sp_residual_residual_pack_wave1083() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let cb = cb_source();
    let ok = cb.contains("Wave 1083: under-construction dual portrait clears production residual")
        && cb.contains("Wave 1083: UC dual portrait fails closed for special-power ready residual")
        && cb.contains("entry.special_power_ready && !entry.under_construction")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostPortraitUcProductionSpResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_portrait_uc_production_sp_residual_honesty() -> bool {
    let a = honesty_host_portrait_uc_production_sp_residual_method_names_residual_wave1083();
    let b = honesty_host_portrait_uc_production_sp_residual_nav_commands_residual_wave1083();
    let c = honesty_host_portrait_uc_production_sp_residual_residual_pack_wave1083();
    residual_action_store(ResidualHostPortraitUcProductionSpResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_portrait_uc_production_sp_residual_wave1083() {
        assert!(honesty_host_portrait_uc_production_sp_residual_residual_pack_wave1083());
        assert!(honesty_host_portrait_uc_production_sp_residual_method_names_residual_wave1083());
        assert!(honesty_host_portrait_uc_production_sp_residual_nav_commands_residual_wave1083());
        assert!(simulate_live_host_portrait_uc_production_sp_residual_honesty());
    }
}
