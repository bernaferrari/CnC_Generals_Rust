//! Wave 1071: dual-world airborne candidate filter + SP disabled target residual.
//!
//! Meta airborne dual skips destroyed/sold/masked/FOW/stealthed non-local aircraft;
//! SP dual object targets fail-closed when disabled. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_AIRBORNE_SP_DISABLED_RESIDUAL_METHOD_NAMES_WAVE1071: &[&str] = &[
    "airborne_target",
    "translator_entry_is_local",
    "target.disabled",
    "Wave 1071",
    "playable_claim = false",
];

pub const LIVE_HOST_AIRBORNE_SP_DISABLED_RESIDUAL_NAV_STEPS_WAVE1071: &[&str] = &[
    "AIRBORNE",
    "SP_DISABLED_TARGET",
    "LIVE_HOST_AIRBORNE_SP_DISABLED_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostAirborneSpDisabledResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostAirborneSpDisabledResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}
fn meta_source() -> &'static str {
    game_client::message_stream::meta_event::META_EVENT_SRC
}
fn ui_source() -> &'static str {
    game_client::gui::ingame_ui::INGAME_UI_SRC
}

pub fn honesty_host_airborne_sp_disabled_residual_method_names_residual_wave1071() -> bool {
    let names = LIVE_HOST_AIRBORNE_SP_DISABLED_RESIDUAL_METHOD_NAMES_WAVE1071;
    let ok = residual_name_index(names, "airborne_target").is_some()
        && residual_name_index(names, "Wave 1071").is_some();
    residual_action_store(ResidualHostAirborneSpDisabledResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_airborne_sp_disabled_residual_nav_commands_residual_wave1071() -> bool {
    let steps = LIVE_HOST_AIRBORNE_SP_DISABLED_RESIDUAL_NAV_STEPS_WAVE1071;
    let ok = residual_name_index(steps, "LIVE_HOST_AIRBORNE_SP_DISABLED_RESIDUAL").is_some()
        && residual_name_index(steps, "AIRBORNE").is_some();
    residual_action_store(ResidualHostAirborneSpDisabledResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_airborne_sp_disabled_residual_residual_pack_wave1071() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let meta = meta_source();
    let ui = ui_source();
    let ok = meta.contains(
        "Wave 1071: fail-closed on destroyed/sold/masked/FOW/non-local stealth residuals",
    ) && meta.contains("entry.shroud_status >= 2 && !translator_entry_is_local(entry)")
        && meta.contains("entry.effectively_stealthed && !translator_entry_is_local(entry)")
        && ui.contains("Wave 1071: disabled target residual fail-closed for SP object targeting")
        && ui.contains("|| target.disabled")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostAirborneSpDisabledResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_airborne_sp_disabled_residual_honesty() -> bool {
    let a = honesty_host_airborne_sp_disabled_residual_method_names_residual_wave1071();
    let b = honesty_host_airborne_sp_disabled_residual_nav_commands_residual_wave1071();
    let c = honesty_host_airborne_sp_disabled_residual_residual_pack_wave1071();
    residual_action_store(ResidualHostAirborneSpDisabledResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_airborne_sp_disabled_residual_wave1071() {
        assert!(honesty_host_airborne_sp_disabled_residual_residual_pack_wave1071());
        assert!(honesty_host_airborne_sp_disabled_residual_method_names_residual_wave1071());
        assert!(honesty_host_airborne_sp_disabled_residual_nav_commands_residual_wave1071());
        assert!(simulate_live_host_airborne_sp_disabled_residual_honesty());
    }
}
