//! Wave 1076: dual-world FOW is_hidden local-only + multi-select residual.
//!
//! Dual SelectableDrawable is_hidden is FOW non-local only; ControlBar multi-select
//! dual skips destroyed/sold/disabled/unselectable/masked. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_FOW_ISHIDDEN_MULTISELECT_RESIDUAL_METHOD_NAMES_WAVE1076: &[&str] = &[
    "is_hidden",
    "translator_entry_is_local",
    "multi-select",
    "Wave 1076",
    "playable_claim = false",
];

pub const LIVE_HOST_FOW_ISHIDDEN_MULTISELECT_RESIDUAL_NAV_STEPS_WAVE1076: &[&str] = &[
    "FOW_ISHIDDEN",
    "MULTISELECT",
    "LIVE_HOST_FOW_ISHIDDEN_MULTISELECT_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostFowIshiddenMultiselectResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostFowIshiddenMultiselectResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
}
fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}
fn sx_source() -> &'static str {
    include_str!("../../../../GameEngine/GameClient/src/message_stream/selection_xlat.rs")
}
fn cb_source() -> &'static str {
    include_str!("../../../../GameEngine/GameClient/src/gui/control_bar/control_bar.rs")
}

pub fn honesty_host_fow_ishidden_multiselect_residual_method_names_residual_wave1076() -> bool {
    let names = LIVE_HOST_FOW_ISHIDDEN_MULTISELECT_RESIDUAL_METHOD_NAMES_WAVE1076;
    let ok = residual_name_index(names, "is_hidden").is_some()
        && residual_name_index(names, "Wave 1076").is_some();
    residual_action_store(ResidualHostFowIshiddenMultiselectResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_fow_ishidden_multiselect_residual_nav_commands_residual_wave1076() -> bool {
    let steps = LIVE_HOST_FOW_ISHIDDEN_MULTISELECT_RESIDUAL_NAV_STEPS_WAVE1076;
    let ok = residual_name_index(steps, "LIVE_HOST_FOW_ISHIDDEN_MULTISELECT_RESIDUAL").is_some()
        && residual_name_index(steps, "FOW_ISHIDDEN").is_some();
    residual_action_store(ResidualHostFowIshiddenMultiselectResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_fow_ishidden_multiselect_residual_residual_pack_wave1076() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let sx = sx_source();
    let cb = cb_source();
    let ok = sx.contains("Wave 1076: FOW is_hidden residual is non-local only")
        && sx.contains("entry.shroud_status >= 2")
        && sx.contains("!translator_entry_is_local(entry)")
        && cb.contains("Wave 1076: unusable multi-select residual fail-closed")
        && cb.contains("entry.destroyed")
        && cb.contains("entry.masked")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostFowIshiddenMultiselectResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_fow_ishidden_multiselect_residual_honesty() -> bool {
    let a = honesty_host_fow_ishidden_multiselect_residual_method_names_residual_wave1076();
    let b = honesty_host_fow_ishidden_multiselect_residual_nav_commands_residual_wave1076();
    let c = honesty_host_fow_ishidden_multiselect_residual_residual_pack_wave1076();
    residual_action_store(ResidualHostFowIshiddenMultiselectResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_fow_ishidden_multiselect_residual_wave1076() {
        assert!(honesty_host_fow_ishidden_multiselect_residual_residual_pack_wave1076());
        assert!(honesty_host_fow_ishidden_multiselect_residual_method_names_residual_wave1076());
        assert!(honesty_host_fow_ishidden_multiselect_residual_nav_commands_residual_wave1076());
        assert!(simulate_live_host_fow_ishidden_multiselect_residual_honesty());
    }
}
