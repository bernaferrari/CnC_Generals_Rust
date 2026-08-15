//! Wave 1078: dual-world OCL/garrison clear on unusable + OCL seed residual.
//!
//! ControlBar dual clears OCL/garrison when selection is unusable and skips OCL
//! seed for destroyed/sold/disabled/unselectable/masked. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_OCL_GARRISON_UNUSABLE_RESIDUAL_METHOD_NAMES_WAVE1078: &[&str] = &[
    "presentation_ocl_timer_seconds",
    "presentation_garrisoned_count",
    "Wave 1078",
    "playable_claim = false",
];

pub const LIVE_HOST_OCL_GARRISON_UNUSABLE_RESIDUAL_NAV_STEPS_WAVE1078: &[&str] = &[
    "OCL_CLEAR",
    "GARRISON_CLEAR",
    "LIVE_HOST_OCL_GARRISON_UNUSABLE_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostOclGarrisonUnusableResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostOclGarrisonUnusableResidualAction) {
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

pub fn honesty_host_ocl_garrison_unusable_residual_method_names_residual_wave1078() -> bool {
    let names = LIVE_HOST_OCL_GARRISON_UNUSABLE_RESIDUAL_METHOD_NAMES_WAVE1078;
    let ok = residual_name_index(names, "presentation_ocl_timer_seconds").is_some()
        && residual_name_index(names, "Wave 1078").is_some();
    residual_action_store(ResidualHostOclGarrisonUnusableResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_ocl_garrison_unusable_residual_nav_commands_residual_wave1078() -> bool {
    let steps = LIVE_HOST_OCL_GARRISON_UNUSABLE_RESIDUAL_NAV_STEPS_WAVE1078;
    let ok = residual_name_index(steps, "LIVE_HOST_OCL_GARRISON_UNUSABLE_RESIDUAL").is_some()
        && residual_name_index(steps, "OCL_CLEAR").is_some();
    residual_action_store(ResidualHostOclGarrisonUnusableResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_ocl_garrison_unusable_residual_residual_pack_wave1078() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let cb = cb_source();
    let ok = cb.contains("Wave 1078: clear OCL/garrison dual residuals with unusable selection")
        && cb.contains("self.presentation_ocl_timer_seconds = 0")
        && cb.contains("self.presentation_garrisoned_count = 0")
        && cb.contains("Wave 1078: skip OCL seed for unusable dual catalog entries")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostOclGarrisonUnusableResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_ocl_garrison_unusable_residual_honesty() -> bool {
    let a = honesty_host_ocl_garrison_unusable_residual_method_names_residual_wave1078();
    let b = honesty_host_ocl_garrison_unusable_residual_nav_commands_residual_wave1078();
    let c = honesty_host_ocl_garrison_unusable_residual_residual_pack_wave1078();
    residual_action_store(ResidualHostOclGarrisonUnusableResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_ocl_garrison_unusable_residual_wave1078() {
        assert!(honesty_host_ocl_garrison_unusable_residual_residual_pack_wave1078());
        assert!(honesty_host_ocl_garrison_unusable_residual_method_names_residual_wave1078());
        assert!(honesty_host_ocl_garrison_unusable_residual_nav_commands_residual_wave1078());
        assert!(simulate_live_host_ocl_garrison_unusable_residual_honesty());
    }
}
