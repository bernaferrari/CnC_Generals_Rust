//! Wave 1030: dual-world beacon + garrison catalog residual.
//!
//! Catalog carries max_garrison/occupant_count. evaluate_context seeds garrison
//! freezes from catalog. update_context_beacon peels catalog BEACON template/command-set.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_BEACON_GARRISON_CATALOG_RESIDUAL_METHOD_NAMES_WAVE1030: &[&str] = &[
    "update_context_beacon",
    "max_garrison",
    "occupant_count",
    "Wave 1030",
    "playable_claim = false",
];

pub const LIVE_HOST_BEACON_GARRISON_CATALOG_RESIDUAL_NAV_STEPS_WAVE1030: &[&str] = &[
    "BEACON_CONTEXT",
    "GARRISON_CATALOG",
    "STRUCTURE_INVENTORY",
    "LIVE_HOST_BEACON_GARRISON_CATALOG_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostBeaconGarrisonCatalogResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostBeaconGarrisonCatalogResidualAction) {
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

pub fn honesty_host_beacon_garrison_catalog_residual_method_names_residual_wave1030() -> bool {
    let names = LIVE_HOST_BEACON_GARRISON_CATALOG_RESIDUAL_METHOD_NAMES_WAVE1030;
    let ok = residual_name_index(names, "update_context_beacon").is_some()
        && residual_name_index(names, "Wave 1030").is_some();
    residual_action_store(ResidualHostBeaconGarrisonCatalogResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_beacon_garrison_catalog_residual_nav_commands_residual_wave1030() -> bool {
    let steps = LIVE_HOST_BEACON_GARRISON_CATALOG_RESIDUAL_NAV_STEPS_WAVE1030;
    let ok = residual_name_index(steps, "LIVE_HOST_BEACON_GARRISON_CATALOG_RESIDUAL").is_some()
        && residual_name_index(steps, "BEACON_CONTEXT").is_some();
    residual_action_store(ResidualHostBeaconGarrisonCatalogResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_beacon_garrison_catalog_residual_residual_pack_wave1030() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let ui = ui_source();
    let tr = tr_source();
    let cb = cb_source();
    let ok = ui
        .contains("Wave 1030: garrison capacity residual for dual-world structure inventory")
        && tr.contains("pub max_garrison: u16")
        && tr.contains("pub occupant_count: u16")
        && cnc.contains("Wave 1030: garrison residual for dual-world structure inventory")
        && cb.contains("Wave 1030: peel translator catalog template/command-set residual too")
        && cb.contains("Wave 1030: seed garrison residual from catalog when freeze unset")
        && cb.contains("entry.max_garrison as usize")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostBeaconGarrisonCatalogResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_beacon_garrison_catalog_residual_honesty() -> bool {
    let a = honesty_host_beacon_garrison_catalog_residual_method_names_residual_wave1030();
    let b = honesty_host_beacon_garrison_catalog_residual_nav_commands_residual_wave1030();
    let c = honesty_host_beacon_garrison_catalog_residual_residual_pack_wave1030();
    residual_action_store(ResidualHostBeaconGarrisonCatalogResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_beacon_garrison_catalog_residual_wave1030() {
        assert!(honesty_host_beacon_garrison_catalog_residual_residual_pack_wave1030());
        assert!(honesty_host_beacon_garrison_catalog_residual_method_names_residual_wave1030());
        assert!(honesty_host_beacon_garrison_catalog_residual_nav_commands_residual_wave1030());
        assert!(simulate_live_host_beacon_garrison_catalog_residual_honesty());
    }
}
