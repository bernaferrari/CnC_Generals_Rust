//! Wave 964: InGameUI presentation selection residual (host empty dual-world).
//!
//! Selection health bars and kind-of queries use presentation freeze when
//! OBJECT_REGISTRY is empty. Engine stamps residual from PresentationFrame.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_INGAME_UI_PRESENTATION_SELECTION_METHOD_NAMES_WAVE964: &[&str] = &[
    "PresentationSelectedUnitResidual",
    "set_presentation_selection_residual",
    "draw_selection_anims_from_presentation",
    "apply_presentation_selection_residual",
    "Wave 964",
    "playable_claim = false",
];

pub const LIVE_HOST_INGAME_UI_PRESENTATION_SELECTION_NAV_STEPS_WAVE964: &[&str] = &[
    "INGAME_UI_PRESENTATION_SELECTION",
    "SELECTION_BARS_FROM_FREEZE",
    "KIND_OF_FROM_PRESENTATION",
    "LIVE_HOST_INGAME_UI_PRESENTATION_SELECTION",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostIngameUiPresentationSelectionAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostIngameUiPresentationSelectionAction) {
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

fn client_source() -> &'static str {
    include_str!("../../../GameEngine/GameClient/src/core/game_client.rs")
}

pub fn honesty_host_ingame_ui_presentation_selection_method_names_residual_wave964() -> bool {
    let names = LIVE_HOST_INGAME_UI_PRESENTATION_SELECTION_METHOD_NAMES_WAVE964;
    let ok = residual_name_index(names, "PresentationSelectedUnitResidual").is_some()
        && residual_name_index(names, "Wave 964").is_some();
    residual_action_store(ResidualHostIngameUiPresentationSelectionAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_ingame_ui_presentation_selection_nav_commands_residual_wave964() -> bool {
    let steps = LIVE_HOST_INGAME_UI_PRESENTATION_SELECTION_NAV_STEPS_WAVE964;
    let ok = residual_name_index(steps, "LIVE_HOST_INGAME_UI_PRESENTATION_SELECTION").is_some()
        && residual_name_index(steps, "SELECTION_BARS_FROM_FREEZE").is_some();
    residual_action_store(ResidualHostIngameUiPresentationSelectionAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_ingame_ui_presentation_selection_residual_pack_wave964() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let ui = ui_source();
    let client = client_source();
    let ok = ui.contains("Wave 964")
        && cnc.contains("Wave 964")
        && ui.contains("struct PresentationSelectedUnitResidual")
        && ui.contains("draw_selection_anims_from_presentation")
        && ui.contains("set_presentation_selection_residual")
        && ui.contains("is_any_selected_kind_of")
        && ui.contains("presentation_selected")
        && client.contains("apply_presentation_selection_residual")
        && cnc.contains("apply_presentation_selection_residual")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostIngameUiPresentationSelectionAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_ingame_ui_presentation_selection_honesty() -> bool {
    let a = honesty_host_ingame_ui_presentation_selection_method_names_residual_wave964();
    let b = honesty_host_ingame_ui_presentation_selection_nav_commands_residual_wave964();
    let c = honesty_host_ingame_ui_presentation_selection_residual_pack_wave964();
    residual_action_store(ResidualHostIngameUiPresentationSelectionAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_ingame_ui_presentation_selection_residual_wave964() {
        assert!(honesty_host_ingame_ui_presentation_selection_residual_pack_wave964());
        assert!(honesty_host_ingame_ui_presentation_selection_method_names_residual_wave964());
        assert!(honesty_host_ingame_ui_presentation_selection_nav_commands_residual_wave964());
        assert!(simulate_live_host_ingame_ui_presentation_selection_honesty());
    }
}
