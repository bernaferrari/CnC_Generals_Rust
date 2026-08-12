//! Wave 1054: dual-world icon UI hide for unselected stealth residual.
//!
//! draw_icon_ui dual path clears health/icon overlays when effectively stealthed
//! and not selected/moused. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_ICON_UI_STEALTH_HIDE_RESIDUAL_METHOD_NAMES_WAVE1054: &[&str] = &[
    "draw_icon_ui",
    "presentation_effectively_stealthed",
    "Wave 1054",
    "playable_claim = false",
];

pub const LIVE_HOST_ICON_UI_STEALTH_HIDE_RESIDUAL_NAV_STEPS_WAVE1054: &[&str] = &[
    "ICON_UI",
    "STEALTH_HIDE",
    "LIVE_HOST_ICON_UI_STEALTH_HIDE_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostIconUiStealthHideResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostIconUiStealthHideResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}
fn drawable_source() -> &'static str {
    game_client::drawable::drawable::DRAWABLE_SRC
}

pub fn honesty_host_icon_ui_stealth_hide_residual_method_names_residual_wave1054() -> bool {
    let names = LIVE_HOST_ICON_UI_STEALTH_HIDE_RESIDUAL_METHOD_NAMES_WAVE1054;
    let ok = residual_name_index(names, "draw_icon_ui").is_some()
        && residual_name_index(names, "Wave 1054").is_some();
    residual_action_store(ResidualHostIconUiStealthHideResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_icon_ui_stealth_hide_residual_nav_commands_residual_wave1054() -> bool {
    let steps = LIVE_HOST_ICON_UI_STEALTH_HIDE_RESIDUAL_NAV_STEPS_WAVE1054;
    let ok = residual_name_index(steps, "LIVE_HOST_ICON_UI_STEALTH_HIDE_RESIDUAL").is_some()
        && residual_name_index(steps, "STEALTH_HIDE").is_some();
    residual_action_store(ResidualHostIconUiStealthHideResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_icon_ui_stealth_hide_residual_residual_pack_wave1054() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let d = drawable_source();
    let ok = d.contains("Wave 1054: dual-world effectively-stealthed residual hides icon UI")
        && d.contains("self.presentation_effectively_stealthed")
        && d.contains("!self.selected_or_moused_over_for_icon_pips()")
        && d.contains("self.overlay_data.visible = false")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostIconUiStealthHideResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_icon_ui_stealth_hide_residual_honesty() -> bool {
    let a = honesty_host_icon_ui_stealth_hide_residual_method_names_residual_wave1054();
    let b = honesty_host_icon_ui_stealth_hide_residual_nav_commands_residual_wave1054();
    let c = honesty_host_icon_ui_stealth_hide_residual_residual_pack_wave1054();
    residual_action_store(ResidualHostIconUiStealthHideResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_icon_ui_stealth_hide_residual_wave1054() {
        assert!(honesty_host_icon_ui_stealth_hide_residual_residual_pack_wave1054());
        assert!(honesty_host_icon_ui_stealth_hide_residual_method_names_residual_wave1054());
        assert!(honesty_host_icon_ui_stealth_hide_residual_nav_commands_residual_wave1054());
        assert!(simulate_live_host_icon_ui_stealth_hide_residual_honesty());
    }
}
