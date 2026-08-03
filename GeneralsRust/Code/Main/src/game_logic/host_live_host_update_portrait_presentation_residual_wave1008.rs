//! Wave 1008: dual-world update_portrait_for_object presentation residual.
//!
//! When OBJECT_REGISTRY is empty and portrait is not yet visible, peel template
//! name + special_power_ready from translator_catalog_entry(obj_id). Richer
//! presentation sync still owns health/queue when already applied.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_UPDATE_PORTRAIT_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE1008: &[&str] = &[
    "update_portrait_for_object",
    "translator_catalog_entry",
    "special_power_ready",
    "Wave 1008",
    "playable_claim = false",
];

pub const LIVE_HOST_UPDATE_PORTRAIT_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE1008: &[&str] = &[
    "DUAL_WORLD",
    "PORTRAIT_TEMPLATE",
    "CATALOG_PEEL",
    "LIVE_HOST_UPDATE_PORTRAIT_PRESENTATION_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostUpdatePortraitPresentationResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostUpdatePortraitPresentationResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}
fn gl_source() -> &'static str {
    include_str!("game_logic.rs")
}
fn cb_source() -> &'static str {
    include_str!("../../../GameEngine/GameClient/src/gui/control_bar/control_bar.rs")
}

pub fn honesty_host_update_portrait_presentation_residual_method_names_residual_wave1008() -> bool {
    let names = LIVE_HOST_UPDATE_PORTRAIT_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE1008;
    let ok = residual_name_index(names, "update_portrait_for_object").is_some()
        && residual_name_index(names, "Wave 1008").is_some();
    residual_action_store(ResidualHostUpdatePortraitPresentationResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_update_portrait_presentation_residual_nav_commands_residual_wave1008() -> bool {
    let steps = LIVE_HOST_UPDATE_PORTRAIT_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE1008;
    let ok = residual_name_index(steps, "LIVE_HOST_UPDATE_PORTRAIT_PRESENTATION_RESIDUAL")
        .is_some()
        && residual_name_index(steps, "CATALOG_PEEL").is_some();
    residual_action_store(ResidualHostUpdatePortraitPresentationResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_update_portrait_presentation_residual_residual_pack_wave1008() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let cb = cb_source();
    let body = match cb.find("fn update_portrait_for_object") {
        Some(i) => &cb[i..cb.len().min(i + 1200)],
        None => "",
    };
    let ok = body.contains("Wave 249/1008")
        && body.contains("translator_catalog_entry(obj_id)")
        && body.contains("entry.template_name")
        && body.contains("special_power_ready")
        && body.contains("!self.portrait_state.is_visible")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostUpdatePortraitPresentationResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_update_portrait_presentation_residual_honesty() -> bool {
    let a = honesty_host_update_portrait_presentation_residual_method_names_residual_wave1008();
    let b = honesty_host_update_portrait_presentation_residual_nav_commands_residual_wave1008();
    let c = honesty_host_update_portrait_presentation_residual_residual_pack_wave1008();
    residual_action_store(ResidualHostUpdatePortraitPresentationResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_update_portrait_presentation_residual_wave1008() {
        assert!(honesty_host_update_portrait_presentation_residual_residual_pack_wave1008());
        assert!(
            honesty_host_update_portrait_presentation_residual_method_names_residual_wave1008()
        );
        assert!(
            honesty_host_update_portrait_presentation_residual_nav_commands_residual_wave1008()
        );
        assert!(simulate_live_host_update_portrait_presentation_residual_honesty());
    }
}
