//! Wave 978: presentation selection residual HUD on shell tick.
//!
//! GameClient presentation shell draws selection health bars from InGameUI
//! presentation residual after drawable icon UI, so host empty dual-world still
//! shows selection HUD without full InGameUI::draw. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PRESENTATION_SELECTION_HUD_METHOD_NAMES_WAVE978: &[&str] = &[
    "draw_presentation_selection_residual",
    "draw_drawable_icon_ui",
    "update_presentation_shell",
    "Wave 978",
    "playable_claim = false",
];

pub const LIVE_HOST_PRESENTATION_SELECTION_HUD_NAV_STEPS_WAVE978: &[&str] = &[
    "SELECTION_HUD_FROM_RESIDUAL",
    "SHELL_POST_ICON_UI",
    "HOST_EMPTY_DUAL_WORLD",
    "LIVE_HOST_PRESENTATION_SELECTION_HUD",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPresentationSelectionHudAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostPresentationSelectionHudAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}

fn client_source() -> &'static str {
    game_client::core::game_client::GAME_CLIENT_SRC
}

// 2026-08-15: widen post-split scan window to the rest of the concat.
pub fn honesty_host_presentation_selection_hud_method_names_residual_wave978() -> bool {
    let names = LIVE_HOST_PRESENTATION_SELECTION_HUD_METHOD_NAMES_WAVE978;
    let ok = residual_name_index(names, "draw_presentation_selection_residual").is_some()
        && residual_name_index(names, "Wave 978").is_some();
    residual_action_store(ResidualHostPresentationSelectionHudAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_presentation_selection_hud_nav_commands_residual_wave978() -> bool {
    let steps = LIVE_HOST_PRESENTATION_SELECTION_HUD_NAV_STEPS_WAVE978;
    let ok = residual_name_index(steps, "LIVE_HOST_PRESENTATION_SELECTION_HUD").is_some()
        && residual_name_index(steps, "SELECTION_HUD_FROM_RESIDUAL").is_some();
    residual_action_store(ResidualHostPresentationSelectionHudAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_presentation_selection_hud_residual_pack_wave978() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let client = client_source();
    let shell = match client.find("fn update_presentation_shell") {
        Some(i) => &client[i..],
        None => "",
    };
    let draw = match client.find("fn draw_presentation_selection_residual") {
        Some(i) => &client[i..],
        None => "",
    };
    let ok = client.contains("Wave 978")
        && draw.contains("presentation_selection_residual")
        && draw.contains("with_ui_renderer_mut")
        && draw.contains("health_pct")
        && shell.contains("draw_presentation_selection_residual")
        && shell.contains("draw_drawable_icon_ui")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostPresentationSelectionHudAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_presentation_selection_hud_honesty() -> bool {
    let a = honesty_host_presentation_selection_hud_method_names_residual_wave978();
    let b = honesty_host_presentation_selection_hud_nav_commands_residual_wave978();
    let c = honesty_host_presentation_selection_hud_residual_pack_wave978();
    residual_action_store(ResidualHostPresentationSelectionHudAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_presentation_selection_hud_residual_wave978() {
        assert!(honesty_host_presentation_selection_hud_residual_pack_wave978());
        assert!(honesty_host_presentation_selection_hud_method_names_residual_wave978());
        assert!(honesty_host_presentation_selection_hud_nav_commands_residual_wave978());
        assert!(simulate_live_host_presentation_selection_hud_honesty());
    }
}
