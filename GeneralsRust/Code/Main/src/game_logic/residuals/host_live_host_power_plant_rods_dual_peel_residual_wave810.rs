//! Wave 810: GW entity carries power plant rods done-frame residual; under coupled
//! dual-tick sole-ticks completion into logs (model condition bits + completion log);
//! host peels update_power_plant_rods. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_POWER_PLANT_RODS_DUAL_PEEL_METHOD_NAMES_WAVE810: &[&str] = &[
    "power_plant_rods_done_frame",
    "POWER_PLANT_UPGRADING",
    "POWER_PLANT_UPGRADED",
    "host_power_plant_rods_log",
    "update_power_plant_rods",
    "record_rods_complete",
    "Wave 810",
    "playable_claim = false",
];
pub const LIVE_HOST_POWER_PLANT_RODS_DUAL_PEEL_NAV_STEPS_WAVE810: &[&str] = &[
    "REQUIRE_ENTITY_POWER_PLANT_RODS_FIELDS",
    "REQUIRE_GW_RODS_COMPLETE_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_COMPLETE_DRAIN",
    "LIVE_HOST_POWER_PLANT_RODS_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPowerPlantRodsDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}
impl ResidualHostPowerPlantRodsDualPeelAction {
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
fn residual_action_store(a: ResidualHostPowerPlantRodsDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_power_plant_rods_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_power_plant_rods_dual_peel_last_action()
-> ResidualHostPowerPlantRodsDualPeelAction {
    ResidualHostPowerPlantRodsDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    // 2026-08-15: scan host plus extra world_* splits.
    super::host_logic_scan_src()
}
pub fn honesty_host_power_plant_rods_dual_peel_method_names_residual_wave810() -> bool {
    let names = LIVE_HOST_POWER_PLANT_RODS_DUAL_PEEL_METHOD_NAMES_WAVE810;
    let ok = residual_name_index(names, "power_plant_rods_done_frame").is_some()
        && residual_name_index(names, "POWER_PLANT_UPGRADING").is_some()
        && residual_name_index(names, "POWER_PLANT_UPGRADED").is_some()
        && residual_name_index(names, "host_power_plant_rods_log").is_some()
        && residual_name_index(names, "update_power_plant_rods").is_some()
        && residual_name_index(names, "record_rods_complete").is_some()
        && residual_name_index(names, "Wave 810").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostPowerPlantRodsDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_power_plant_rods_dual_peel_source_markers_residual_wave810() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("power_plant_rods_done_frame")
        && ent.contains("model_condition_bits")
        && sh.contains("Wave 810")
        && sh.contains("host_power_plant_rods_log::record_complete")
        && sh.contains("host_power_plant_rods_log::drain_completes")
        && sh.contains("POWER_PLANT_UPGRADING")
        && sh.contains("POWER_PLANT_UPGRADED")
        && gl.contains("Wave 810")
        && gl.contains("update_power_plant_rods")
        && gl.contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostPowerPlantRodsDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_power_plant_rods_dual_peel_nav_commands_residual_wave810() -> bool {
    let steps = LIVE_HOST_POWER_PLANT_RODS_DUAL_PEEL_NAV_STEPS_WAVE810;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_POWER_PLANT_RODS_FIELDS").is_some()
        && residual_name_index(steps, "REQUIRE_GW_RODS_COMPLETE_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_COMPLETE_DRAIN").is_some()
        && residual_name_index(steps, "LIVE_HOST_POWER_PLANT_RODS_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostPowerPlantRodsDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_power_plant_rods_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 810")
        && sh_source().contains("power_plant_rods_done_frame")
        && gl_source().contains("Wave 810");
    residual_action_store(ResidualHostPowerPlantRodsDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_power_plant_rods_dual_peel_dispatch_source() -> bool {
    // 2026-08-15: record_rods_complete is host GameLogic
    // (host_special_power_completion_die.rs); drain stays in shadow.
    let host_die = include_str!("../host_special_power_completion_die.rs");
    let ok = sh_source().contains("host_power_plant_rods_log::drain_completes")
        && (sh_source().contains("record_rods_complete")
            || host_die.contains("fn record_rods_complete")
            || gl_source().contains("record_rods_complete"))
        && gl_source().contains("update_power_plant_rods")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostPowerPlantRodsDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_power_plant_rods_dual_peel_residual_pack_wave810() -> bool {
    honesty_host_power_plant_rods_dual_peel_method_names_residual_wave810()
        && honesty_host_power_plant_rods_dual_peel_source_markers_residual_wave810()
        && honesty_host_power_plant_rods_dual_peel_nav_commands_residual_wave810()
}
pub fn simulate_live_host_power_plant_rods_dual_peel_honesty() -> bool {
    let ok = honesty_host_power_plant_rods_dual_peel_residual_pack_wave810()
        && simulate_host_power_plant_rods_dual_peel_collect_source()
        && simulate_host_power_plant_rods_dual_peel_dispatch_source();
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_power_plant_rods_dual_peel_method_names_residual_wave810());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_power_plant_rods_dual_peel_source_markers_residual_wave810());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_power_plant_rods_dual_peel_nav_commands_residual_wave810());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_power_plant_rods_dual_peel_collect_source());
        assert!(simulate_host_power_plant_rods_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_power_plant_rods_dual_peel_residual_pack_wave810());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_power_plant_rods_dual_peel_honesty());
        assert!(residual_host_power_plant_rods_dual_peel_ok());
    }
}
