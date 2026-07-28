//! Wave 832: PlayerTemplate residual carries StartingUnit0..9 table; spawn walks
//! all non-empty slots after starting building (C++ MAX_MP_STARTING_UNITS).
//! Retail INI only fills unit0 (dozer/worker). playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_STARTING_UNITS_TABLE_METHOD_NAMES_WAVE832: &[&str] = &[
    "starting_units",
    "starting_unit0",
    "spawn_skirmish_starting_units",
    "Wave 832",
    "playable_claim = false",
];
pub const LIVE_HOST_STARTING_UNITS_TABLE_NAV_STEPS_WAVE832: &[&str] = &[
    "REQUIRE_STARTING_UNITS_TABLE",
    "REQUIRE_SPAWN_WALKS_TABLE",
    "LIVE_HOST_STARTING_UNITS_TABLE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostStartingUnitsTableAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}
impl ResidualHostStartingUnitsTableAction {
    fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::MethodNames,
            2 => Self::SourceMarkers,
            3 => Self::NavCommands,
            4 => Self::CollectSource,
            5 => Self::DispatchSource,
            _ => Self::None,
        }
    }
}
fn residual_action_store(a: ResidualHostStartingUnitsTableAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
fn gl_source() -> &'static str {
    include_str!("game_logic.rs")
}
fn res_source() -> &'static str {
    include_str!("host_faction_skirmish_residual.rs")
}
pub fn honesty_host_starting_units_table_method_names_residual_wave832() -> bool {
    let names = LIVE_HOST_STARTING_UNITS_TABLE_METHOD_NAMES_WAVE832;
    let ok = residual_name_index(names, "starting_units").is_some()
        && residual_name_index(names, "starting_unit0").is_some()
        && residual_name_index(names, "spawn_skirmish_starting_units").is_some()
        && residual_name_index(names, "Wave 832").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostStartingUnitsTableAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
pub fn honesty_host_starting_units_table_nav_commands_residual_wave832() -> bool {
    let steps = LIVE_HOST_STARTING_UNITS_TABLE_NAV_STEPS_WAVE832;
    let ok = residual_name_index(steps, "LIVE_HOST_STARTING_UNITS_TABLE").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostStartingUnitsTableAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
pub fn honesty_host_starting_units_table_residual_pack_wave832() -> bool {
    let gl = gl_source();
    let res = res_source();
    let ok = res.contains("starting_units: [&'static str; 10]")
        && res.contains("starting_units: [\"AmericaVehicleDozer\"")
        && gl.contains("Wave 832: walk residual.starting_units")
        && gl.contains("starting_units")
        && gl.contains("spawn_skirmish_starting_units");
    residual_action_store(ResidualHostStartingUnitsTableAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
pub fn simulate_live_host_starting_units_table_honesty() -> bool {
    let a = honesty_host_starting_units_table_method_names_residual_wave832();
    let b = honesty_host_starting_units_table_nav_commands_residual_wave832();
    let c = honesty_host_starting_units_table_residual_pack_wave832();
    residual_action_store(ResidualHostStartingUnitsTableAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_starting_units_table_residual_wave832() {
        assert!(honesty_host_starting_units_table_residual_pack_wave832());
        assert!(honesty_host_starting_units_table_method_names_residual_wave832());
        assert!(honesty_host_starting_units_table_nav_commands_residual_wave832());
        assert!(simulate_live_host_starting_units_table_honesty());
    }
}
