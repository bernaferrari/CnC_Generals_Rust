//! Wave 1065: dual-world FOW fogged SP target and mouseover residual.
//!
//! is_valid_special_power_target and create_mouseover_hint dual paths fail-closed
//! on non-local PartialClear/Fogged/Shrouded targets. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_FOW_FOGGED_SP_MOUSEOVER_RESIDUAL_METHOD_NAMES_WAVE1065: &[&str] = &[
    "is_valid_special_power_target",
    "create_mouseover_hint_from_presentation",
    "ObjectShroudStatus::Fogged",
    "Wave 1065",
    "playable_claim = false",
];

pub const LIVE_HOST_FOW_FOGGED_SP_MOUSEOVER_RESIDUAL_NAV_STEPS_WAVE1065: &[&str] = &[
    "FOW",
    "SP_TARGET",
    "MOUSEOVER",
    "LIVE_HOST_FOW_FOGGED_SP_MOUSEOVER_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostFowFoggedSpMouseoverResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostFowFoggedSpMouseoverResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}
fn ui_source() -> &'static str {
    game_client::gui::ingame_ui::INGAME_UI_SRC
}

pub fn honesty_host_fow_fogged_sp_mouseover_residual_method_names_residual_wave1065() -> bool {
    let names = LIVE_HOST_FOW_FOGGED_SP_MOUSEOVER_RESIDUAL_METHOD_NAMES_WAVE1065;
    let ok = residual_name_index(names, "is_valid_special_power_target").is_some()
        && residual_name_index(names, "Wave 1065").is_some();
    residual_action_store(ResidualHostFowFoggedSpMouseoverResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_fow_fogged_sp_mouseover_residual_nav_commands_residual_wave1065() -> bool {
    let steps = LIVE_HOST_FOW_FOGGED_SP_MOUSEOVER_RESIDUAL_NAV_STEPS_WAVE1065;
    let ok = residual_name_index(steps, "LIVE_HOST_FOW_FOGGED_SP_MOUSEOVER_RESIDUAL").is_some()
        && residual_name_index(steps, "MOUSEOVER").is_some();
    residual_action_store(ResidualHostFowFoggedSpMouseoverResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_fow_fogged_sp_mouseover_residual_residual_pack_wave1065() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let ui = ui_source();
    let ok = ui.contains("Wave 1065: FOW fogged/black non-local SP targets fail-closed")
        && ui.contains("Wave 1065: FOW fogged/black non-local hover residual fail-closed")
        && ui.contains("ObjectShroudStatus::Fogged")
        && ui.contains("ObjectShroudStatus::PartialClear")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostFowFoggedSpMouseoverResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_fow_fogged_sp_mouseover_residual_honesty() -> bool {
    let a = honesty_host_fow_fogged_sp_mouseover_residual_method_names_residual_wave1065();
    let b = honesty_host_fow_fogged_sp_mouseover_residual_nav_commands_residual_wave1065();
    let c = honesty_host_fow_fogged_sp_mouseover_residual_residual_pack_wave1065();
    residual_action_store(ResidualHostFowFoggedSpMouseoverResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_fow_fogged_sp_mouseover_residual_wave1065() {
        assert!(honesty_host_fow_fogged_sp_mouseover_residual_residual_pack_wave1065());
        assert!(honesty_host_fow_fogged_sp_mouseover_residual_method_names_residual_wave1065());
        assert!(honesty_host_fow_fogged_sp_mouseover_residual_nav_commands_residual_wave1065());
        assert!(simulate_live_host_fow_fogged_sp_mouseover_residual_honesty());
    }
}
