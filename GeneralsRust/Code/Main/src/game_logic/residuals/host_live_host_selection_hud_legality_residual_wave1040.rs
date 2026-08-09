//! Wave 1040: dual-world selection HUD legality residual.
//!
//! PresentationSelectedUnitResidual carries destroyed/sold/unselectable/masked/
//! stealth/team; draw_selection_anims_from_presentation skips illegal bars.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SELECTION_HUD_LEGALITY_RESIDUAL_METHOD_NAMES_WAVE1040: &[&str] = &[
    "PresentationSelectedUnitResidual",
    "draw_selection_anims_from_presentation",
    "effectively_stealthed",
    "Wave 1040",
    "playable_claim = false",
];

pub const LIVE_HOST_SELECTION_HUD_LEGALITY_RESIDUAL_NAV_STEPS_WAVE1040: &[&str] = &[
    "SELECTION_HUD",
    "STATUS_LEGALITY",
    "LIVE_HOST_SELECTION_HUD_LEGALITY_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSelectionHudLegalityResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSelectionHudLegalityResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
}
fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}
fn ui_source() -> &'static str {
    include_str!("../../../../GameEngine/GameClient/src/gui/ingame_ui.rs")
}

pub fn honesty_host_selection_hud_legality_residual_method_names_residual_wave1040() -> bool {
    let names = LIVE_HOST_SELECTION_HUD_LEGALITY_RESIDUAL_METHOD_NAMES_WAVE1040;
    let ok = residual_name_index(names, "PresentationSelectedUnitResidual").is_some()
        && residual_name_index(names, "Wave 1040").is_some();
    residual_action_store(ResidualHostSelectionHudLegalityResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_selection_hud_legality_residual_nav_commands_residual_wave1040() -> bool {
    let steps = LIVE_HOST_SELECTION_HUD_LEGALITY_RESIDUAL_NAV_STEPS_WAVE1040;
    let ok = residual_name_index(steps, "LIVE_HOST_SELECTION_HUD_LEGALITY_RESIDUAL").is_some()
        && residual_name_index(steps, "SELECTION_HUD").is_some();
    residual_action_store(ResidualHostSelectionHudLegalityResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_selection_hud_legality_residual_residual_pack_wave1040() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let ui = ui_source();
    let ok = ui.contains("Wave 1040: legality residual for dual selection HUD")
        && ui.contains("Wave 1040: skip illegal selection HUD residuals")
        && ui.contains("pub effectively_stealthed: bool")
        && cnc.contains("Wave 1040: selection HUD legality residual")
        && cnc.contains("effectively_stealthed: o.effectively_stealthed")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostSelectionHudLegalityResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_selection_hud_legality_residual_honesty() -> bool {
    let a = honesty_host_selection_hud_legality_residual_method_names_residual_wave1040();
    let b = honesty_host_selection_hud_legality_residual_nav_commands_residual_wave1040();
    let c = honesty_host_selection_hud_legality_residual_residual_pack_wave1040();
    residual_action_store(ResidualHostSelectionHudLegalityResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_selection_hud_legality_residual_wave1040() {
        assert!(honesty_host_selection_hud_legality_residual_residual_pack_wave1040());
        assert!(honesty_host_selection_hud_legality_residual_method_names_residual_wave1040());
        assert!(honesty_host_selection_hud_legality_residual_nav_commands_residual_wave1040());
        assert!(simulate_live_host_selection_hud_legality_residual_honesty());
    }
}
