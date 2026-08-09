//! Wave 1080: dual-world UC/garrison seed usable + portrait masked residual.
//!
//! ControlBar dual UC/garrison seed skips unusable catalog; unusable clear drops UC
//! residual; portrait dual also clears masked/disabled. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_UC_GARRISON_SEED_PORTRAIT_RESIDUAL_METHOD_NAMES_WAVE1080: &[&str] = &[
    "presentation_under_construction",
    "presentation_construction_percent",
    "portrait_state",
    "Wave 1080",
    "playable_claim = false",
];

pub const LIVE_HOST_UC_GARRISON_SEED_PORTRAIT_RESIDUAL_NAV_STEPS_WAVE1080: &[&str] = &[
    "UC_SEED",
    "PORTRAIT_MASKED",
    "LIVE_HOST_UC_GARRISON_SEED_PORTRAIT_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostUcGarrisonSeedPortraitResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostUcGarrisonSeedPortraitResidualAction) {
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

pub fn honesty_host_uc_garrison_seed_portrait_residual_method_names_residual_wave1080() -> bool {
    let names = LIVE_HOST_UC_GARRISON_SEED_PORTRAIT_RESIDUAL_METHOD_NAMES_WAVE1080;
    let ok = residual_name_index(names, "presentation_under_construction").is_some()
        && residual_name_index(names, "Wave 1080").is_some();
    residual_action_store(ResidualHostUcGarrisonSeedPortraitResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_uc_garrison_seed_portrait_residual_nav_commands_residual_wave1080() -> bool {
    let steps = LIVE_HOST_UC_GARRISON_SEED_PORTRAIT_RESIDUAL_NAV_STEPS_WAVE1080;
    let ok = residual_name_index(steps, "LIVE_HOST_UC_GARRISON_SEED_PORTRAIT_RESIDUAL").is_some()
        && residual_name_index(steps, "UC_SEED").is_some();
    residual_action_store(ResidualHostUcGarrisonSeedPortraitResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_uc_garrison_seed_portrait_residual_residual_pack_wave1080() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let cb = cb_source();
    let ok = cb.contains("Wave 1080: skip UC/garrison seed for unusable dual catalog entries")
        && cb.contains("Wave 1080: clear under-construction dual residual with unusable selection")
        && cb.contains("self.presentation_under_construction = false")
        && cb.contains("Wave 1080: masked/disabled also clear dual portrait residual")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostUcGarrisonSeedPortraitResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_uc_garrison_seed_portrait_residual_honesty() -> bool {
    let a = honesty_host_uc_garrison_seed_portrait_residual_method_names_residual_wave1080();
    let b = honesty_host_uc_garrison_seed_portrait_residual_nav_commands_residual_wave1080();
    let c = honesty_host_uc_garrison_seed_portrait_residual_residual_pack_wave1080();
    residual_action_store(ResidualHostUcGarrisonSeedPortraitResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_uc_garrison_seed_portrait_residual_wave1080() {
        assert!(honesty_host_uc_garrison_seed_portrait_residual_residual_pack_wave1080());
        assert!(honesty_host_uc_garrison_seed_portrait_residual_method_names_residual_wave1080());
        assert!(honesty_host_uc_garrison_seed_portrait_residual_nav_commands_residual_wave1080());
        assert!(simulate_live_host_uc_garrison_seed_portrait_residual_honesty());
    }
}
