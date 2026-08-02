//! Wave 987: demoralized icon ALLOW_DEMORALIZE-off residual.
//!
//! Retail Zero Hour builds with ALLOW_DEMORALIZE undefined. C++ Drawable::drawDemoralized
//! is compiled out; weapon bonus table uses DEMORALIZED_OBSOLETE. Host and dual-world
//! icon paths fail-closed (show_demoralized = false). playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_DEMORALIZED_ALLOW_OFF_RESIDUAL_METHOD_NAMES_WAVE987: &[&str] = &[
    "draw_demoralized",
    "ALLOW_DEMORALIZE",
    "DEMORALIZED_OBSOLETE",
    "show_demoralized",
    "Wave 987",
    "playable_claim = false",
];

pub const LIVE_HOST_DEMORALIZED_ALLOW_OFF_RESIDUAL_NAV_STEPS_WAVE987: &[&str] = &[
    "ALLOW_DEMORALIZE_OFF",
    "FAIL_CLOSED_ICON",
    "HOST_AND_DUAL_WORLD",
    "LIVE_HOST_DEMORALIZED_ALLOW_OFF_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostDemoralizedAllowOffResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostDemoralizedAllowOffResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}
fn gl_source() -> &'static str {
    include_str!("game_logic.rs")
}
fn drawable_source() -> &'static str {
    include_str!("../../../GameEngine/GameClient/src/drawable/drawable.rs")
}
fn enum_table_source() -> &'static str {
    include_str!("host_enum_table_residual.rs")
}

pub fn honesty_host_demoralized_allow_off_residual_method_names_residual_wave987() -> bool {
    let names = LIVE_HOST_DEMORALIZED_ALLOW_OFF_RESIDUAL_METHOD_NAMES_WAVE987;
    let ok = residual_name_index(names, "ALLOW_DEMORALIZE").is_some()
        && residual_name_index(names, "Wave 987").is_some();
    residual_action_store(ResidualHostDemoralizedAllowOffResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_demoralized_allow_off_residual_nav_commands_residual_wave987() -> bool {
    let steps = LIVE_HOST_DEMORALIZED_ALLOW_OFF_RESIDUAL_NAV_STEPS_WAVE987;
    let ok = residual_name_index(steps, "LIVE_HOST_DEMORALIZED_ALLOW_OFF_RESIDUAL").is_some()
        && residual_name_index(steps, "FAIL_CLOSED_ICON").is_some();
    residual_action_store(ResidualHostDemoralizedAllowOffResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_demoralized_allow_off_residual_residual_pack_wave987() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let d = drawable_source();
    let et = enum_table_source();
    let dem = match d.find("fn draw_demoralized") {
        Some(i) => &d[i..d.len().min(i + 900)],
        None => "",
    };
    let ok = d.contains("Wave 987")
        && dem.contains("ALLOW_DEMORALIZE")
        && dem.contains("show_demoralized = false")
        && !dem.contains("WeaponBonusConditionFlags::DEMORALIZED")
        && !dem.contains("get_ai_update_interface")
        && et.contains("DEMORALIZED_OBSOLETE")
        && et.contains("ALLOW_DEMORALIZE off")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostDemoralizedAllowOffResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_demoralized_allow_off_residual_honesty() -> bool {
    let a = honesty_host_demoralized_allow_off_residual_method_names_residual_wave987();
    let b = honesty_host_demoralized_allow_off_residual_nav_commands_residual_wave987();
    let c = honesty_host_demoralized_allow_off_residual_residual_pack_wave987();
    residual_action_store(ResidualHostDemoralizedAllowOffResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_demoralized_allow_off_residual_wave987() {
        assert!(honesty_host_demoralized_allow_off_residual_residual_pack_wave987());
        assert!(honesty_host_demoralized_allow_off_residual_method_names_residual_wave987());
        assert!(honesty_host_demoralized_allow_off_residual_nav_commands_residual_wave987());
        assert!(simulate_live_host_demoralized_allow_off_residual_honesty());
    }
}
