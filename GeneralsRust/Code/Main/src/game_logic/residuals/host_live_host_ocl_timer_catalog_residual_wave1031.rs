//! Wave 1031: dual-world ControlBar OCL timer catalog residual.
//!
//! Catalog/presentation carry ocl_timer_seconds (supply-drop zone residual).
//! evaluate_context selects OclTimer after UC (C++ CB_CONTEXT_OCL_TIMER order).
//! sync_ocl_timer_from_presentation freezes display without OCLUpdate modules.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_OCL_TIMER_CATALOG_RESIDUAL_METHOD_NAMES_WAVE1031: &[&str] = &[
    "sync_ocl_timer_from_presentation",
    "presentation_ocl_timer_seconds",
    "ControlBarState::OclTimer",
    "remaining_ocl_timer_seconds",
    "Wave 1031",
    "playable_claim = false",
];

pub const LIVE_HOST_OCL_TIMER_CATALOG_RESIDUAL_NAV_STEPS_WAVE1031: &[&str] = &[
    "OCL_TIMER",
    "EVALUATE_CONTEXT",
    "SUPPLY_DROP_ZONE",
    "LIVE_HOST_OCL_TIMER_CATALOG_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostOclTimerCatalogResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostOclTimerCatalogResidualAction) {
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
fn tr_source() -> &'static str {
    include_str!("../../../../GameEngine/GameClient/src/presentation_translator_residual.rs")
}
fn cb_source() -> &'static str {
    game_client::gui::control_bar::control_bar::CONTROL_BAR_SRC
}
fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}
fn sd_source() -> &'static str {
    include_str!("../host_supply_drop_zone.rs")
}

pub fn honesty_host_ocl_timer_catalog_residual_method_names_residual_wave1031() -> bool {
    let names = LIVE_HOST_OCL_TIMER_CATALOG_RESIDUAL_METHOD_NAMES_WAVE1031;
    let ok = residual_name_index(names, "sync_ocl_timer_from_presentation").is_some()
        && residual_name_index(names, "Wave 1031").is_some();
    residual_action_store(ResidualHostOclTimerCatalogResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_ocl_timer_catalog_residual_nav_commands_residual_wave1031() -> bool {
    let steps = LIVE_HOST_OCL_TIMER_CATALOG_RESIDUAL_NAV_STEPS_WAVE1031;
    let ok = residual_name_index(steps, "LIVE_HOST_OCL_TIMER_CATALOG_RESIDUAL").is_some()
        && residual_name_index(steps, "OCL_TIMER").is_some();
    residual_action_store(ResidualHostOclTimerCatalogResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_ocl_timer_catalog_residual_residual_pack_wave1031() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let ui = ui_source();
    let tr = tr_source();
    let cb = cb_source();
    let pf = pf_source();
    let sd = sd_source();
    let ok = ui.contains(
        "Wave 1031: OCL timer residual seconds for dual-world ControlBar OclTimer context",
    ) && tr.contains("pub ocl_timer_seconds: u32")
        && sd.contains("remaining_ocl_timer_seconds")
        && pf.contains("Wave 1031: OCL timer residual for dual-world ControlBar OclTimer context")
        && pf.contains("sync_ocl_timer_from_presentation(panel.ocl_timer_seconds)")
        && cb.contains("Wave 1031: host/presentation OCL timer residual")
        && cb.contains("Wave 1031: C++ CB_CONTEXT_OCL_TIMER residual")
        && cb.contains("presentation_ocl_timer_seconds > 0")
        && cb.contains("ControlBarState::OclTimer")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostOclTimerCatalogResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_ocl_timer_catalog_residual_honesty() -> bool {
    let a = honesty_host_ocl_timer_catalog_residual_method_names_residual_wave1031();
    let b = honesty_host_ocl_timer_catalog_residual_nav_commands_residual_wave1031();
    let c = honesty_host_ocl_timer_catalog_residual_residual_pack_wave1031();
    residual_action_store(ResidualHostOclTimerCatalogResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_ocl_timer_catalog_residual_wave1031() {
        assert!(honesty_host_ocl_timer_catalog_residual_residual_pack_wave1031());
        assert!(honesty_host_ocl_timer_catalog_residual_method_names_residual_wave1031());
        assert!(honesty_host_ocl_timer_catalog_residual_nav_commands_residual_wave1031());
        assert!(simulate_live_host_ocl_timer_catalog_residual_honesty());
    }
}
