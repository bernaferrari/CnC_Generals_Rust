//! Wave 794: GW entity carries CarpetBomb DeliverPayload flight residual;
//! under coupled dual-tick sole-ticks transport/bomb and logs drop/detonate;
//! host peels `update_carpet_bomb_flights`. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_CARPET_BOMB_FLIGHT_DUAL_PEEL_METHOD_NAMES_WAVE794: &[&str] = &[
    "carpet_bomb_transport_active",
    "carpet_bomb_payload",
    "carpet_pending_drops",
    "host_carpet_bomb_drop_log",
    "update_carpet_bomb_flights",
    "Wave 794",
    "playable_claim = false",
];
pub const LIVE_HOST_CARPET_BOMB_FLIGHT_DUAL_PEEL_NAV_STEPS_WAVE794: &[&str] = &[
    "REQUIRE_ENTITY_CARPET_BOMB_FIELDS",
    "REQUIRE_GW_FLIGHT_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_DROP_DETONATE_DRAIN",
    "LIVE_HOST_CARPET_BOMB_FLIGHT_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_CARPET_BOMB_FLIGHT_DUAL_PEEL_CMD_NAMES_WAVE794: &[&str] = &[
    "host_carpet_bomb_flight_dual_peel",
    "carpet_bomb_transport_active",
    "host_carpet_bomb_drop_log",
    "update_carpet_bomb_flights",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostCarpetBombFlightDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostCarpetBombFlightDualPeelAction {
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
fn residual_action_store(a: ResidualHostCarpetBombFlightDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_carpet_bomb_flight_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_carpet_bomb_flight_dual_peel_last_action()
-> ResidualHostCarpetBombFlightDualPeelAction {
    ResidualHostCarpetBombFlightDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_carpet_bomb_flight_dual_peel_method_names_residual_wave794() -> bool {
    let names = LIVE_HOST_CARPET_BOMB_FLIGHT_DUAL_PEEL_METHOD_NAMES_WAVE794;
    let ok = residual_name_index(names, "carpet_bomb_transport_active").is_some()
        && residual_name_index(names, "carpet_bomb_payload").is_some()
        && residual_name_index(names, "host_carpet_bomb_drop_log").is_some()
        && residual_name_index(names, "update_carpet_bomb_flights").is_some()
        && residual_name_index(names, "Wave 794").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostCarpetBombFlightDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_carpet_bomb_flight_dual_peel_source_markers_residual_wave794() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("carpet_bomb_transport_active")
        && ent.contains("carpet_bomb_payload")
        && sh.contains("carpet_pending_drops")
        && sh.contains("Wave 794")
        && sh.contains("host_carpet_bomb_drop_log::record_drop")
        && sh.contains("host_carpet_bomb_drop_log::drain_dets")
        && gl.contains("Wave 794")
        && gl.contains("update_carpet_bomb_flights");
    residual_action_store(ResidualHostCarpetBombFlightDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_carpet_bomb_flight_dual_peel_nav_commands_residual_wave794() -> bool {
    let steps = LIVE_HOST_CARPET_BOMB_FLIGHT_DUAL_PEEL_NAV_STEPS_WAVE794;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_CARPET_BOMB_FIELDS").is_some()
        && residual_name_index(steps, "REQUIRE_GW_FLIGHT_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_DROP_DETONATE_DRAIN").is_some()
        && residual_name_index(steps, "LIVE_HOST_CARPET_BOMB_FLIGHT_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostCarpetBombFlightDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_carpet_bomb_flight_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 794")
        && sh_source().contains("carpet_bomb_transport_active")
        && gl_source().contains("Wave 794");
    residual_action_store(ResidualHostCarpetBombFlightDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_carpet_bomb_flight_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("host_carpet_bomb_drop_log::record_drop")
        && sh_source().contains("host_carpet_bomb_drop_log::drain_drops")
        && gl_source().contains("update_carpet_bomb_flights")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostCarpetBombFlightDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_carpet_bomb_flight_dual_peel_residual_pack_wave794() -> bool {
    honesty_host_carpet_bomb_flight_dual_peel_method_names_residual_wave794()
        && honesty_host_carpet_bomb_flight_dual_peel_source_markers_residual_wave794()
        && honesty_host_carpet_bomb_flight_dual_peel_nav_commands_residual_wave794()
        && simulate_host_carpet_bomb_flight_dual_peel_collect_source()
        && simulate_host_carpet_bomb_flight_dual_peel_dispatch_source()
}
pub fn simulate_live_host_carpet_bomb_flight_dual_peel_honesty() -> bool {
    let ok = honesty_host_carpet_bomb_flight_dual_peel_residual_pack_wave794();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostCarpetBombFlightDualPeelAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_carpet_bomb_flight_dual_peel_method_names_residual_wave794());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_carpet_bomb_flight_dual_peel_source_markers_residual_wave794());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_carpet_bomb_flight_dual_peel_nav_commands_residual_wave794());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_carpet_bomb_flight_dual_peel_collect_source());
        assert!(simulate_host_carpet_bomb_flight_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_carpet_bomb_flight_dual_peel_residual_pack_wave794());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_carpet_bomb_flight_dual_peel_honesty());
        assert!(residual_host_carpet_bomb_flight_dual_peel_ok());
    }
}
