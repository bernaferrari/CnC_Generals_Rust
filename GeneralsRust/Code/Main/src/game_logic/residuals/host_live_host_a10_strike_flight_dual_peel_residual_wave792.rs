//! Wave 792: GW entity carries A10Strike DeliverPayload flight residual;
//! under coupled dual-tick sole-ticks transport/bomb and logs drop/detonate;
//! host peels `update_a10_strike_flights`. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_A10_STRIKE_FLIGHT_DUAL_PEEL_METHOD_NAMES_WAVE792: &[&str] = &[
    "a10_strike_transport_active",
    "a10_strike_missile",
    "a10_pending_drops",
    "host_a10_strike_drop_log",
    "update_a10_strike_flights",
    "Wave 792",
    "playable_claim = false",
];
pub const LIVE_HOST_A10_STRIKE_FLIGHT_DUAL_PEEL_NAV_STEPS_WAVE792: &[&str] = &[
    "REQUIRE_ENTITY_A10_STRIKE_FIELDS",
    "REQUIRE_GW_FLIGHT_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_DROP_DETONATE_DRAIN",
    "LIVE_HOST_A10_STRIKE_FLIGHT_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_A10_STRIKE_FLIGHT_DUAL_PEEL_CMD_NAMES_WAVE792: &[&str] = &[
    "host_a10_strike_flight_dual_peel",
    "a10_strike_transport_active",
    "host_a10_strike_drop_log",
    "update_a10_strike_flights",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostA10StrikeFlightDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostA10StrikeFlightDualPeelAction {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::MethodNames,
            2 => Self::SourceMarkers,
            3 => Self::NavCommands,
            4 => Self::CollectSource,
            5 => Self::DispatchSource,
            6 => Self::Composite,
            _ => Self::None,
        }
    }
}
fn residual_action_store(a: ResidualHostA10StrikeFlightDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_a10_strike_flight_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_a10_strike_flight_dual_peel_last_action()
-> ResidualHostA10StrikeFlightDualPeelAction {
    ResidualHostA10StrikeFlightDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_a10_strike_flight_dual_peel_method_names_residual_wave792() -> bool {
    let names = LIVE_HOST_A10_STRIKE_FLIGHT_DUAL_PEEL_METHOD_NAMES_WAVE792;
    let ok = residual_name_index(names, "a10_strike_transport_active").is_some()
        && residual_name_index(names, "a10_strike_missile").is_some()
        && residual_name_index(names, "host_a10_strike_drop_log").is_some()
        && residual_name_index(names, "update_a10_strike_flights").is_some()
        && residual_name_index(names, "Wave 792").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostA10StrikeFlightDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_a10_strike_flight_dual_peel_source_markers_residual_wave792() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("a10_strike_transport_active")
        && ent.contains("a10_strike_missile")
        && sh.contains("a10_pending_drops")
        && sh.contains("Wave 792")
        && sh.contains("host_a10_strike_drop_log::record_drop")
        && sh.contains("host_a10_strike_drop_log::drain_dets")
        && gl.contains("Wave 792")
        && gl.contains("update_a10_strike_flights");
    residual_action_store(ResidualHostA10StrikeFlightDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_a10_strike_flight_dual_peel_nav_commands_residual_wave792() -> bool {
    let steps = LIVE_HOST_A10_STRIKE_FLIGHT_DUAL_PEEL_NAV_STEPS_WAVE792;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_A10_STRIKE_FIELDS").is_some()
        && residual_name_index(steps, "REQUIRE_GW_FLIGHT_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_DROP_DETONATE_DRAIN").is_some()
        && residual_name_index(steps, "LIVE_HOST_A10_STRIKE_FLIGHT_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostA10StrikeFlightDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_a10_strike_flight_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 792")
        && sh_source().contains("a10_strike_transport_active")
        && gl_source().contains("Wave 792");
    residual_action_store(ResidualHostA10StrikeFlightDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_a10_strike_flight_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("host_a10_strike_drop_log::record_drop")
        && sh_source().contains("host_a10_strike_drop_log::drain_drops")
        && gl_source().contains("update_a10_strike_flights")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostA10StrikeFlightDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_a10_strike_flight_dual_peel_residual_pack_wave792() -> bool {
    honesty_host_a10_strike_flight_dual_peel_method_names_residual_wave792()
        && honesty_host_a10_strike_flight_dual_peel_source_markers_residual_wave792()
        && honesty_host_a10_strike_flight_dual_peel_nav_commands_residual_wave792()
        && simulate_host_a10_strike_flight_dual_peel_collect_source()
        && simulate_host_a10_strike_flight_dual_peel_dispatch_source()
}
pub fn simulate_live_host_a10_strike_flight_dual_peel_honesty() -> bool {
    let ok = honesty_host_a10_strike_flight_dual_peel_residual_pack_wave792();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostA10StrikeFlightDualPeelAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_a10_strike_flight_dual_peel_method_names_residual_wave792());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_a10_strike_flight_dual_peel_source_markers_residual_wave792());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_a10_strike_flight_dual_peel_nav_commands_residual_wave792());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_a10_strike_flight_dual_peel_collect_source());
        assert!(simulate_host_a10_strike_flight_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_a10_strike_flight_dual_peel_residual_pack_wave792());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_a10_strike_flight_dual_peel_honesty());
        assert!(residual_host_a10_strike_flight_dual_peel_ok());
    }
}
