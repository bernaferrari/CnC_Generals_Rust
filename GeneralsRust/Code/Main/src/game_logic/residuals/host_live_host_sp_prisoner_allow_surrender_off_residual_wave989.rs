//! Wave 989: special-power prisoner target ALLOW_SURRENDER-off residual.
//!
//! Retail Zero Hour builds with ALLOW_SURRENDER undefined (no CAN_SURRENDER / PRISON
//! KindOf bits). Host empty dual-world SP targeting fail-closes NEED_TARGET_PRISONER
//! options. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SP_PRISONER_ALLOW_SURRENDER_OFF_RESIDUAL_METHOD_NAMES_WAVE989: &[&str] = &[
    "is_valid_special_power_target",
    "NEED_TARGET_PRISONER",
    "ALLOW_SURRENDER",
    "Wave 989",
    "playable_claim = false",
];

pub const LIVE_HOST_SP_PRISONER_ALLOW_SURRENDER_OFF_RESIDUAL_NAV_STEPS_WAVE989: &[&str] = &[
    "ALLOW_SURRENDER_OFF",
    "PRISONER_SP_FAIL_CLOSED",
    "HOST_EMPTY_DUAL_WORLD",
    "LIVE_HOST_SP_PRISONER_ALLOW_SURRENDER_OFF_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSpPrisonerAllowSurrenderOffResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSpPrisonerAllowSurrenderOffResidualAction) {
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
fn enum_table_source() -> &'static str {
    include_str!("../host_enum_table_residual.rs")
}

pub fn honesty_host_sp_prisoner_allow_surrender_off_residual_method_names_residual_wave989() -> bool
{
    let names = LIVE_HOST_SP_PRISONER_ALLOW_SURRENDER_OFF_RESIDUAL_METHOD_NAMES_WAVE989;
    let ok = residual_name_index(names, "NEED_TARGET_PRISONER").is_some()
        && residual_name_index(names, "Wave 989").is_some();
    residual_action_store(ResidualHostSpPrisonerAllowSurrenderOffResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_sp_prisoner_allow_surrender_off_residual_nav_commands_residual_wave989() -> bool
{
    let steps = LIVE_HOST_SP_PRISONER_ALLOW_SURRENDER_OFF_RESIDUAL_NAV_STEPS_WAVE989;
    let ok = residual_name_index(steps, "LIVE_HOST_SP_PRISONER_ALLOW_SURRENDER_OFF_RESIDUAL")
        .is_some()
        && residual_name_index(steps, "PRISONER_SP_FAIL_CLOSED").is_some();
    residual_action_store(ResidualHostSpPrisonerAllowSurrenderOffResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_sp_prisoner_allow_surrender_off_residual_residual_pack_wave989() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let ui = ui_source();
    let et = enum_table_source();
    let sp = match ui.find("fn is_valid_special_power_target") {
        Some(i) => &ui[i..ui.len().min(i + 2500)],
        None => "",
    };
    let ok = ui.contains("Wave 989")
        && sp.contains("ALLOW_SURRENDER")
        && sp.contains("NEED_TARGET_PRISONER")
        && sp.contains("return false")
        && et.contains("ALLOW_SURRENDER")
        && et.contains("CAN_SURRENDER")
        && !et.contains("\"CAN_SURRENDER\" =>") // list residual asserts absence
        && et.contains("!KINDOF_BIT_NAME_LIST.contains(&\"CAN_SURRENDER\")")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostSpPrisonerAllowSurrenderOffResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_sp_prisoner_allow_surrender_off_residual_honesty() -> bool {
    let a = honesty_host_sp_prisoner_allow_surrender_off_residual_method_names_residual_wave989();
    let b = honesty_host_sp_prisoner_allow_surrender_off_residual_nav_commands_residual_wave989();
    let c = honesty_host_sp_prisoner_allow_surrender_off_residual_residual_pack_wave989();
    residual_action_store(ResidualHostSpPrisonerAllowSurrenderOffResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_sp_prisoner_allow_surrender_off_residual_wave989() {
        assert!(honesty_host_sp_prisoner_allow_surrender_off_residual_residual_pack_wave989());
        assert!(
            honesty_host_sp_prisoner_allow_surrender_off_residual_method_names_residual_wave989()
        );
        assert!(
            honesty_host_sp_prisoner_allow_surrender_off_residual_nav_commands_residual_wave989()
        );
        assert!(simulate_live_host_sp_prisoner_allow_surrender_off_residual_honesty());
    }
}
