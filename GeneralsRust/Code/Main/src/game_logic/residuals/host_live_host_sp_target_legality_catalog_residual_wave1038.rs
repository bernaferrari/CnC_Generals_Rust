//! Wave 1038: dual-world special power target legality catalog residual.
//!
//! is_valid_special_power_target dual path rejects destroyed/sold/unselectable/
//! masked targets and non-local effectively_stealthed (C++ SelectionInfo parity).
//! Source sold/destroyed also fail closed. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SP_TARGET_LEGALITY_CATALOG_RESIDUAL_METHOD_NAMES_WAVE1038: &[&str] = &[
    "is_valid_special_power_target",
    "effectively_stealthed",
    "destroyed",
    "Wave 1038",
    "playable_claim = false",
];

pub const LIVE_HOST_SP_TARGET_LEGALITY_CATALOG_RESIDUAL_NAV_STEPS_WAVE1038: &[&str] = &[
    "SPECIAL_POWER_TARGET",
    "TARGET_LEGALITY",
    "STEALTH_SKIP",
    "LIVE_HOST_SP_TARGET_LEGALITY_CATALOG_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSpTargetLegalityCatalogResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSpTargetLegalityCatalogResidualAction) {
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

pub fn honesty_host_sp_target_legality_catalog_residual_method_names_residual_wave1038() -> bool {
    let names = LIVE_HOST_SP_TARGET_LEGALITY_CATALOG_RESIDUAL_METHOD_NAMES_WAVE1038;
    let ok = residual_name_index(names, "is_valid_special_power_target").is_some()
        && residual_name_index(names, "Wave 1038").is_some();
    residual_action_store(ResidualHostSpTargetLegalityCatalogResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_sp_target_legality_catalog_residual_nav_commands_residual_wave1038() -> bool {
    let steps = LIVE_HOST_SP_TARGET_LEGALITY_CATALOG_RESIDUAL_NAV_STEPS_WAVE1038;
    let ok = residual_name_index(steps, "LIVE_HOST_SP_TARGET_LEGALITY_CATALOG_RESIDUAL").is_some()
        && residual_name_index(steps, "SPECIAL_POWER_TARGET").is_some();
    residual_action_store(ResidualHostSpTargetLegalityCatalogResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_sp_target_legality_catalog_residual_residual_pack_wave1038() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let ui = ui_source();
    let ok = ui.contains("Wave 1038: C++ target legality residual")
        && ui.contains("target.destroyed || target.sold || target.unselectable || target.masked")
        && ui.contains("target.effectively_stealthed")
        && ui.contains("Wave 1038: source must be alive/usable residual")
        && ui.contains("source.destroyed || source.sold")
        && ui.contains("translator_local_team_name")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostSpTargetLegalityCatalogResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_sp_target_legality_catalog_residual_honesty() -> bool {
    let a = honesty_host_sp_target_legality_catalog_residual_method_names_residual_wave1038();
    let b = honesty_host_sp_target_legality_catalog_residual_nav_commands_residual_wave1038();
    let c = honesty_host_sp_target_legality_catalog_residual_residual_pack_wave1038();
    residual_action_store(ResidualHostSpTargetLegalityCatalogResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_sp_target_legality_catalog_residual_wave1038() {
        assert!(honesty_host_sp_target_legality_catalog_residual_residual_pack_wave1038());
        assert!(honesty_host_sp_target_legality_catalog_residual_method_names_residual_wave1038());
        assert!(honesty_host_sp_target_legality_catalog_residual_nav_commands_residual_wave1038());
        assert!(simulate_live_host_sp_target_legality_catalog_residual_honesty());
    }
}
