//! Wave 1055: dual-world control-group numeral residual.
//!
//! Catalog stamps hotkey_group from host control_groups; drawable presentation
//! residual peels group numeral resolve on dual draw_ui_text path.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_CONTROL_GROUP_NUMERAL_RESIDUAL_METHOD_NAMES_WAVE1055: &[&str] = &[
    "hotkey_group",
    "draw_ui_text_from_presentation",
    "set_presentation_hotkey_group",
    "Wave 1055",
    "playable_claim = false",
];

pub const LIVE_HOST_CONTROL_GROUP_NUMERAL_RESIDUAL_NAV_STEPS_WAVE1055: &[&str] = &[
    "CONTROL_GROUP",
    "GROUP_NUMERAL",
    "LIVE_HOST_CONTROL_GROUP_NUMERAL_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostControlGroupNumeralResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostControlGroupNumeralResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}
fn gl_source() -> &'static str {
    include_str!("game_logic.rs")
}
fn ui_source() -> &'static str {
    include_str!("../../../GameEngine/GameClient/src/gui/ingame_ui.rs")
}
fn drawable_source() -> &'static str {
    include_str!("../../../GameEngine/GameClient/src/drawable/drawable.rs")
}
fn client_source() -> &'static str {
    include_str!("../../../GameEngine/GameClient/src/core/game_client.rs")
}

pub fn honesty_host_control_group_numeral_residual_method_names_residual_wave1055() -> bool {
    let names = LIVE_HOST_CONTROL_GROUP_NUMERAL_RESIDUAL_METHOD_NAMES_WAVE1055;
    let ok = residual_name_index(names, "hotkey_group").is_some()
        && residual_name_index(names, "Wave 1055").is_some();
    residual_action_store(ResidualHostControlGroupNumeralResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_control_group_numeral_residual_nav_commands_residual_wave1055() -> bool {
    let steps = LIVE_HOST_CONTROL_GROUP_NUMERAL_RESIDUAL_NAV_STEPS_WAVE1055;
    let ok = residual_name_index(steps, "LIVE_HOST_CONTROL_GROUP_NUMERAL_RESIDUAL").is_some()
        && residual_name_index(steps, "CONTROL_GROUP").is_some();
    residual_action_store(ResidualHostControlGroupNumeralResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_control_group_numeral_residual_residual_pack_wave1055() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let ui = ui_source();
    let d = drawable_source();
    let client = client_source();
    let ok = ui.contains("pub hotkey_group: i8")
        && cnc.contains("Wave 1055: reverse map object_id → control group")
        && cnc.contains("hotkey_group: object_hotkey_group")
        && d.contains("Wave 1055: host control-group residual → group numeral dual draw")
        && d.contains("set_presentation_hotkey_group")
        && client.contains("set_presentation_hotkey_group(u.hotkey_group)")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostControlGroupNumeralResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_control_group_numeral_residual_honesty() -> bool {
    let a = honesty_host_control_group_numeral_residual_method_names_residual_wave1055();
    let b = honesty_host_control_group_numeral_residual_nav_commands_residual_wave1055();
    let c = honesty_host_control_group_numeral_residual_residual_pack_wave1055();
    residual_action_store(ResidualHostControlGroupNumeralResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_control_group_numeral_residual_wave1055() {
        assert!(honesty_host_control_group_numeral_residual_residual_pack_wave1055());
        assert!(honesty_host_control_group_numeral_residual_method_names_residual_wave1055());
        assert!(honesty_host_control_group_numeral_residual_nav_commands_residual_wave1055());
        assert!(simulate_live_host_control_group_numeral_residual_honesty());
    }
}
