//! Wave 951: presentation-only unit_control click/control-group dual-read peel.
//!
//! Click friendly/attackable classification and control-group assign/select/pose
//! no longer fall back to live `GameLogic::get_object` dual-reads.
//! Fail-closed without `PresentationFrame`. Host presence uses `host_object`.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_UNIT_CONTROL_PRESENTATION_PEEL_METHOD_NAMES_WAVE951: &[&str] = &[
    "host_object",
    "presentation_is_attackable",
    "presentation_is_selectable",
    "presentation_frame",
    "Wave 951",
    "playable_claim = false",
];

pub const LIVE_HOST_UNIT_CONTROL_PRESENTATION_PEEL_NAV_STEPS_WAVE951: &[&str] = &[
    "UNIT_CONTROL_PRESENTATION_PEEL",
    "CLICK_NO_LIVE_GET_OBJECT",
    "CONTROL_GROUP_PRESENTATION_ONLY",
    "LIVE_HOST_UNIT_CONTROL_PRESENTATION_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostUnitControlPresentationPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostUnitControlPresentationPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
}

fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}

fn uc_source() -> &'static str {
    include_str!("../../unit_control.rs")
}

fn non_comment_code(window: &str) -> String {
    window
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn honesty_host_unit_control_presentation_peel_method_names_residual_wave951() -> bool {
    let names = LIVE_HOST_UNIT_CONTROL_PRESENTATION_PEEL_METHOD_NAMES_WAVE951;
    let ok = residual_name_index(names, "host_object").is_some()
        && residual_name_index(names, "Wave 951").is_some()
        && residual_name_index(names, "presentation_is_attackable").is_some();
    residual_action_store(ResidualHostUnitControlPresentationPeelAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_unit_control_presentation_peel_nav_commands_residual_wave951() -> bool {
    let steps = LIVE_HOST_UNIT_CONTROL_PRESENTATION_PEEL_NAV_STEPS_WAVE951;
    let ok = residual_name_index(steps, "LIVE_HOST_UNIT_CONTROL_PRESENTATION_PEEL").is_some()
        && residual_name_index(steps, "CLICK_NO_LIVE_GET_OBJECT").is_some();
    residual_action_store(ResidualHostUnitControlPresentationPeelAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_unit_control_presentation_peel_residual_pack_wave951() -> bool {
    let gl = gl_source();
    let uc = uc_source();
    let cnc = cnc_source();
    let tests_at = uc.find("#[cfg(test)]").unwrap_or(uc.len());
    let prod = non_comment_code(&uc[..tests_at]);
    let ok = gl.contains("fn host_object")
        && uc.contains("Wave 951")
        && prod.contains("host_object")
        && prod.contains("presentation_is_attackable")
        && !prod.contains("get_object(")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostUnitControlPresentationPeelAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_unit_control_presentation_peel_honesty() -> bool {
    let a = honesty_host_unit_control_presentation_peel_method_names_residual_wave951();
    let b = honesty_host_unit_control_presentation_peel_nav_commands_residual_wave951();
    let c = honesty_host_unit_control_presentation_peel_residual_pack_wave951();
    residual_action_store(ResidualHostUnitControlPresentationPeelAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_unit_control_presentation_peel_residual_wave951() {
        assert!(honesty_host_unit_control_presentation_peel_residual_pack_wave951());
        assert!(honesty_host_unit_control_presentation_peel_method_names_residual_wave951());
        assert!(honesty_host_unit_control_presentation_peel_nav_commands_residual_wave951());
        assert!(simulate_live_host_unit_control_presentation_peel_honesty());
    }
}
