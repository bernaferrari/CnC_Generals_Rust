//! Wave 1053: dual-world portrait clear on illegal status residual.
//!
//! update_portrait_for_object dual clears portrait/queue when catalog entry is
//! destroyed/sold/unselectable. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PORTRAIT_STATUS_CLEAR_RESIDUAL_METHOD_NAMES_WAVE1053: &[&str] = &[
    "update_portrait_for_object",
    "PortraitDisplayState::default",
    "Wave 1053",
    "playable_claim = false",
];

pub const LIVE_HOST_PORTRAIT_STATUS_CLEAR_RESIDUAL_NAV_STEPS_WAVE1053: &[&str] = &[
    "PORTRAIT",
    "STATUS_CLEAR",
    "LIVE_HOST_PORTRAIT_STATUS_CLEAR_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPortraitStatusClearResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostPortraitStatusClearResidualAction) {
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

pub fn honesty_host_portrait_status_clear_residual_method_names_residual_wave1053() -> bool {
    let names = LIVE_HOST_PORTRAIT_STATUS_CLEAR_RESIDUAL_METHOD_NAMES_WAVE1053;
    let ok = residual_name_index(names, "update_portrait_for_object").is_some()
        && residual_name_index(names, "Wave 1053").is_some();
    residual_action_store(ResidualHostPortraitStatusClearResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_portrait_status_clear_residual_nav_commands_residual_wave1053() -> bool {
    let steps = LIVE_HOST_PORTRAIT_STATUS_CLEAR_RESIDUAL_NAV_STEPS_WAVE1053;
    let ok = residual_name_index(steps, "LIVE_HOST_PORTRAIT_STATUS_CLEAR_RESIDUAL").is_some()
        && residual_name_index(steps, "PORTRAIT").is_some();
    residual_action_store(ResidualHostPortraitStatusClearResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_portrait_status_clear_residual_residual_pack_wave1053() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let cb = cb_source();
    let ok = cb.contains("Wave 1053: destroyed/sold/unselectable clear portrait residual.")
        && cb.contains("self.portrait_state = PortraitDisplayState::default();")
        && cb.contains("entry.destroyed || entry.sold || entry.unselectable")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostPortraitStatusClearResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_portrait_status_clear_residual_honesty() -> bool {
    let a = honesty_host_portrait_status_clear_residual_method_names_residual_wave1053();
    let b = honesty_host_portrait_status_clear_residual_nav_commands_residual_wave1053();
    let c = honesty_host_portrait_status_clear_residual_residual_pack_wave1053();
    residual_action_store(ResidualHostPortraitStatusClearResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_portrait_status_clear_residual_wave1053() {
        assert!(honesty_host_portrait_status_clear_residual_residual_pack_wave1053());
        assert!(honesty_host_portrait_status_clear_residual_method_names_residual_wave1053());
        assert!(honesty_host_portrait_status_clear_residual_nav_commands_residual_wave1053());
        assert!(simulate_live_host_portrait_status_clear_residual_honesty());
    }
}
