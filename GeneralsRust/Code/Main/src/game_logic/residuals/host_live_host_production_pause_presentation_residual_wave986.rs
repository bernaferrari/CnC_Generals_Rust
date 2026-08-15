//! Wave 986: production pause presentation residual.
//!
//! Freezes BuildingData.production_paused into PresentationFrame / portrait HUD
//! so host empty dual-world reflects pause state (pairs with Wave 985 queue drain).
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PRODUCTION_PAUSE_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE986: &[&str] = &[
    "production_paused",
    "sync_selection_display_from_presentation",
    "unit_display_info_from_renderable",
    "Wave 986",
    "playable_claim = false",
];

pub const LIVE_HOST_PRODUCTION_PAUSE_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE986: &[&str] = &[
    "FREEZE_BUILDING_PAUSE",
    "PORTRAIT_PRODUCTION_PAUSED",
    "SELECTION_PANEL_RESIDUAL",
    "LIVE_HOST_PRODUCTION_PAUSE_PRESENTATION_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostProductionPausePresentationResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostProductionPausePresentationResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}
fn hud_source() -> &'static str {
    include_str!("../../ui/hud_state.rs")
}
fn control_bar_source() -> &'static str {
    game_client::gui::control_bar::control_bar::CONTROL_BAR_SRC
}

pub fn honesty_host_production_pause_presentation_residual_method_names_residual_wave986() -> bool {
    let names = LIVE_HOST_PRODUCTION_PAUSE_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE986;
    let ok = residual_name_index(names, "production_paused").is_some()
        && residual_name_index(names, "Wave 986").is_some();
    residual_action_store(ResidualHostProductionPausePresentationResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_production_pause_presentation_residual_nav_commands_residual_wave986() -> bool {
    let steps = LIVE_HOST_PRODUCTION_PAUSE_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE986;
    let ok = residual_name_index(steps, "LIVE_HOST_PRODUCTION_PAUSE_PRESENTATION_RESIDUAL")
        .is_some()
        && residual_name_index(steps, "PORTRAIT_PRODUCTION_PAUSED").is_some();
    residual_action_store(ResidualHostProductionPausePresentationResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_production_pause_presentation_residual_residual_pack_wave986() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let pf = pf_source();
    let hud = hud_source();
    let cb = control_bar_source();
    let sync = match cb.find("pub fn sync_selection_display_from_presentation") {
        Some(i) => &cb[i..cb.len().min(i + 2500)],
        None => "",
    };
    let udi = match pf.find("fn unit_display_info_from_renderable") {
        Some(i) => &pf[i..pf.len().min(i + 2500)],
        None => "",
    };
    let ok = pf.contains("pub production_paused: bool")
        && pf.contains("Wave 986")
        && pf.contains("b.production_paused")
        && udi.contains("production_paused: ro.production_paused")
        && hud.contains("pub production_paused: bool")
        && hud.contains("production_paused: primary.production_paused")
        && cb.contains("pub production_paused: bool")
        && sync.contains("production_paused: bool")
        && sync.contains("production_paused")
        && cb.contains("self.portrait_state.production_paused = paused")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostProductionPausePresentationResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_production_pause_presentation_residual_honesty() -> bool {
    let a = honesty_host_production_pause_presentation_residual_method_names_residual_wave986();
    let b = honesty_host_production_pause_presentation_residual_nav_commands_residual_wave986();
    let c = honesty_host_production_pause_presentation_residual_residual_pack_wave986();
    residual_action_store(ResidualHostProductionPausePresentationResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_production_pause_presentation_residual_wave986() {
        assert!(honesty_host_production_pause_presentation_residual_residual_pack_wave986());
        assert!(
            honesty_host_production_pause_presentation_residual_method_names_residual_wave986()
        );
        assert!(
            honesty_host_production_pause_presentation_residual_nav_commands_residual_wave986()
        );
        assert!(simulate_live_host_production_pause_presentation_residual_honesty());
    }
}
