//! Wave 971: special-power targeting presentation catalog residual.
//!
//! Peels is_valid_special_power_target and overridable-destination checks onto
//! presentation unit catalog (team + special_power_ready) when OBJECT_REGISTRY
//! is empty. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SPECIAL_POWER_CATALOG_METHOD_NAMES_WAVE971: &[&str] = &[
    "is_valid_special_power_target",
    "source_has_overridable_special_power_destination",
    "special_power_ready",
    "Wave 971",
    "playable_claim = false",
];

pub const LIVE_HOST_SPECIAL_POWER_CATALOG_NAV_STEPS_WAVE971: &[&str] = &[
    "SPECIAL_POWER_FROM_CATALOG",
    "SP_READY_RESIDUAL",
    "HOST_EMPTY_DUAL_WORLD",
    "LIVE_HOST_SPECIAL_POWER_CATALOG",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSpecialPowerCatalogAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSpecialPowerCatalogAction) {
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

pub fn honesty_host_special_power_catalog_method_names_residual_wave971() -> bool {
    let names = LIVE_HOST_SPECIAL_POWER_CATALOG_METHOD_NAMES_WAVE971;
    let ok = residual_name_index(names, "is_valid_special_power_target").is_some()
        && residual_name_index(names, "Wave 971").is_some();
    residual_action_store(ResidualHostSpecialPowerCatalogAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_special_power_catalog_nav_commands_residual_wave971() -> bool {
    let steps = LIVE_HOST_SPECIAL_POWER_CATALOG_NAV_STEPS_WAVE971;
    let ok = residual_name_index(steps, "LIVE_HOST_SPECIAL_POWER_CATALOG").is_some()
        && residual_name_index(steps, "SPECIAL_POWER_FROM_CATALOG").is_some();
    residual_action_store(ResidualHostSpecialPowerCatalogAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_special_power_catalog_residual_pack_wave971() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let ui = ui_source();
    let valid = match ui.find("fn is_valid_special_power_target") {
        Some(i) => &ui[i..ui.len().min(i + 1800)],
        None => "",
    };
    let ov = match ui.find("fn source_has_overridable_special_power_destination") {
        Some(i) => &ui[i..ui.len().min(i + 700)],
        None => "",
    };
    let ok = ui.contains("Wave 971")
        && cnc.contains("Wave 971")
        && ui.contains("special_power_ready")
        && valid.contains("presentation_unit_catalog")
        && valid.contains("special_power_ready")
        && ov.contains("presentation_unit_catalog")
        && ov.contains("special_power_ready")
        && cnc.contains("special_power_ready:")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostSpecialPowerCatalogAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_special_power_catalog_honesty() -> bool {
    let a = honesty_host_special_power_catalog_method_names_residual_wave971();
    let b = honesty_host_special_power_catalog_nav_commands_residual_wave971();
    let c = honesty_host_special_power_catalog_residual_pack_wave971();
    residual_action_store(ResidualHostSpecialPowerCatalogAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_special_power_catalog_residual_wave971() {
        assert!(honesty_host_special_power_catalog_residual_pack_wave971());
        assert!(honesty_host_special_power_catalog_method_names_residual_wave971());
        assert!(honesty_host_special_power_catalog_nav_commands_residual_wave971());
        assert!(simulate_live_host_special_power_catalog_honesty());
    }
}
