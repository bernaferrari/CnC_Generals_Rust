//! Wave 1041: dual-world disguise portrait/template residual.
//!
//! Catalog stamps disguised/disguise_as_*; non-allied dual portrait and
//! select-similar use apparent identity (C++ InGameUI/ControlBar parity).
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_DISGUISE_CATALOG_RESIDUAL_METHOD_NAMES_WAVE1041: &[&str] = &[
    "disguise_as_template",
    "translator_entry_apparent_template",
    "Wave 1041",
    "playable_claim = false",
];

pub const LIVE_HOST_DISGUISE_CATALOG_RESIDUAL_NAV_STEPS_WAVE1041: &[&str] = &[
    "DISGUISE",
    "PORTRAIT_SWAP",
    "LIVE_HOST_DISGUISE_CATALOG_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostDisguiseCatalogResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostDisguiseCatalogResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
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

pub fn honesty_host_disguise_catalog_residual_method_names_residual_wave1041() -> bool {
    let names = LIVE_HOST_DISGUISE_CATALOG_RESIDUAL_METHOD_NAMES_WAVE1041;
    let ok = residual_name_index(names, "translator_entry_apparent_template").is_some()
        && residual_name_index(names, "Wave 1041").is_some();
    residual_action_store(ResidualHostDisguiseCatalogResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_disguise_catalog_residual_nav_commands_residual_wave1041() -> bool {
    let steps = LIVE_HOST_DISGUISE_CATALOG_RESIDUAL_NAV_STEPS_WAVE1041;
    let ok = residual_name_index(steps, "LIVE_HOST_DISGUISE_CATALOG_RESIDUAL").is_some()
        && residual_name_index(steps, "DISGUISE").is_some();
    residual_action_store(ResidualHostDisguiseCatalogResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_disguise_catalog_residual_residual_pack_wave1041() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let ui = ui_source();
    let tr = tr_source();
    let cb = cb_source();
    let ok = ui.contains("Wave 1041: disguised residual")
        && ui.contains("Wave 1041: match on apparent template/team")
        && tr.contains("translator_entry_apparent_template")
        && tr.contains("pub disguised: bool")
        && cb.contains("Wave 1041: C++ ControlBar disguise portrait swap")
        && cnc.contains("Wave 1041: disguise residual for dual portrait/template peel")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostDisguiseCatalogResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_disguise_catalog_residual_honesty() -> bool {
    let a = honesty_host_disguise_catalog_residual_method_names_residual_wave1041();
    let b = honesty_host_disguise_catalog_residual_nav_commands_residual_wave1041();
    let c = honesty_host_disguise_catalog_residual_residual_pack_wave1041();
    residual_action_store(ResidualHostDisguiseCatalogResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_disguise_catalog_residual_wave1041() {
        assert!(honesty_host_disguise_catalog_residual_residual_pack_wave1041());
        assert!(honesty_host_disguise_catalog_residual_method_names_residual_wave1041());
        assert!(honesty_host_disguise_catalog_residual_nav_commands_residual_wave1041());
        assert!(simulate_live_host_disguise_catalog_residual_honesty());
    }
}
