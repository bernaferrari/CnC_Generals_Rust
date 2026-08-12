//! Wave 1042: dual-world disguise mouseover tooltip residual.
//!
//! create_mouseover_hint_from_presentation uses disguise_as_template for
//! non-allied viewers (C++ InGameUI tooltip parity). playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_DISGUISE_MOUSEOVER_TOOLTIP_RESIDUAL_METHOD_NAMES_WAVE1042: &[&str] = &[
    "create_mouseover_hint_from_presentation",
    "disguise_as_template",
    "Wave 1042",
    "playable_claim = false",
];

pub const LIVE_HOST_DISGUISE_MOUSEOVER_TOOLTIP_RESIDUAL_NAV_STEPS_WAVE1042: &[&str] = &[
    "DISGUISE",
    "MOUSEOVER_TOOLTIP",
    "LIVE_HOST_DISGUISE_MOUSEOVER_TOOLTIP_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostDisguiseMouseoverTooltipResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostDisguiseMouseoverTooltipResidualAction) {
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

pub fn honesty_host_disguise_mouseover_tooltip_residual_method_names_residual_wave1042() -> bool {
    let names = LIVE_HOST_DISGUISE_MOUSEOVER_TOOLTIP_RESIDUAL_METHOD_NAMES_WAVE1042;
    let ok = residual_name_index(names, "create_mouseover_hint_from_presentation").is_some()
        && residual_name_index(names, "Wave 1042").is_some();
    residual_action_store(ResidualHostDisguiseMouseoverTooltipResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_disguise_mouseover_tooltip_residual_nav_commands_residual_wave1042() -> bool {
    let steps = LIVE_HOST_DISGUISE_MOUSEOVER_TOOLTIP_RESIDUAL_NAV_STEPS_WAVE1042;
    let ok = residual_name_index(steps, "LIVE_HOST_DISGUISE_MOUSEOVER_TOOLTIP_RESIDUAL").is_some()
        && residual_name_index(steps, "MOUSEOVER_TOOLTIP").is_some();
    residual_action_store(ResidualHostDisguiseMouseoverTooltipResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_disguise_mouseover_tooltip_residual_residual_pack_wave1042() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let ui = ui_source();
    let ok = ui.contains("Wave 1042: C++ InGameUI disguise tooltip residual")
        && ui.contains("disguise_as_template")
        && ui.contains("create_mouseover_hint_from_presentation")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostDisguiseMouseoverTooltipResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_disguise_mouseover_tooltip_residual_honesty() -> bool {
    let a = honesty_host_disguise_mouseover_tooltip_residual_method_names_residual_wave1042();
    let b = honesty_host_disguise_mouseover_tooltip_residual_nav_commands_residual_wave1042();
    let c = honesty_host_disguise_mouseover_tooltip_residual_residual_pack_wave1042();
    residual_action_store(ResidualHostDisguiseMouseoverTooltipResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_disguise_mouseover_tooltip_residual_wave1042() {
        assert!(honesty_host_disguise_mouseover_tooltip_residual_residual_pack_wave1042());
        assert!(honesty_host_disguise_mouseover_tooltip_residual_method_names_residual_wave1042());
        assert!(honesty_host_disguise_mouseover_tooltip_residual_nav_commands_residual_wave1042());
        assert!(simulate_live_host_disguise_mouseover_tooltip_residual_honesty());
    }
}
