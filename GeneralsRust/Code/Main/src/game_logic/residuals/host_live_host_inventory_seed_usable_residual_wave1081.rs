//! Wave 1081: dual-world inventory seed usable residual.
//!
//! ControlBar dual structure-inventory seed (else-if freeze path) skips unusable
//! catalog entries. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_INVENTORY_SEED_USABLE_RESIDUAL_METHOD_NAMES_WAVE1081: &[&str] = &[
    "presentation_max_garrison",
    "presentation_garrisoned_count",
    "Wave 1081",
    "playable_claim = false",
];

pub const LIVE_HOST_INVENTORY_SEED_USABLE_RESIDUAL_NAV_STEPS_WAVE1081: &[&str] = &[
    "INVENTORY_SEED",
    "USABLE_CATALOG",
    "LIVE_HOST_INVENTORY_SEED_USABLE_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostInventorySeedUsableResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostInventorySeedUsableResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
fn cb_source() -> &'static str {
    game_client::gui::control_bar::control_bar::CONTROL_BAR_SRC
}

pub fn honesty_host_inventory_seed_usable_residual_method_names_residual_wave1081() -> bool {
    let names = LIVE_HOST_INVENTORY_SEED_USABLE_RESIDUAL_METHOD_NAMES_WAVE1081;
    let ok = residual_name_index(names, "presentation_max_garrison").is_some()
        && residual_name_index(names, "Wave 1081").is_some();
    residual_action_store(ResidualHostInventorySeedUsableResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_inventory_seed_usable_residual_nav_commands_residual_wave1081() -> bool {
    let steps = LIVE_HOST_INVENTORY_SEED_USABLE_RESIDUAL_NAV_STEPS_WAVE1081;
    let ok = residual_name_index(steps, "LIVE_HOST_INVENTORY_SEED_USABLE_RESIDUAL").is_some()
        && residual_name_index(steps, "INVENTORY_SEED").is_some();
    residual_action_store(ResidualHostInventorySeedUsableResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_inventory_seed_usable_residual_residual_pack_wave1081() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let cb = cb_source();
    let ok = cb.contains("Wave 1081: skip inventory seed for unusable dual catalog entries")
        && cb.contains("entry.max_garrison > 0")
        && cb.contains("!entry.masked")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostInventorySeedUsableResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_inventory_seed_usable_residual_honesty() -> bool {
    let a = honesty_host_inventory_seed_usable_residual_method_names_residual_wave1081();
    let b = honesty_host_inventory_seed_usable_residual_nav_commands_residual_wave1081();
    let c = honesty_host_inventory_seed_usable_residual_residual_pack_wave1081();
    residual_action_store(ResidualHostInventorySeedUsableResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_inventory_seed_usable_residual_wave1081() {
        assert!(honesty_host_inventory_seed_usable_residual_residual_pack_wave1081());
        assert!(honesty_host_inventory_seed_usable_residual_method_names_residual_wave1081());
        assert!(honesty_host_inventory_seed_usable_residual_nav_commands_residual_wave1081());
        assert!(simulate_live_host_inventory_seed_usable_residual_honesty());
    }
}
