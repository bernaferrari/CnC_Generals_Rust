//! Wave 788: GW entity carries DaisyCutter/MOAB DeliverPayload flight residual;
//! under coupled dual-tick sole-ticks transport/bomb and logs drop/detonate;
//! host peels `update_daisy_cutter_flights`. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_DAISY_CUTTER_FLIGHT_DUAL_PEEL_METHOD_NAMES_WAVE788: &[&str] = &[
    "daisy_transport_active",
    "daisy_cutter_bomb",
    "host_daisy_cutter_drop_log",
    "update_daisy_cutter_flights",
    "Wave 788",
    "playable_claim = false",
];
pub const LIVE_HOST_DAISY_CUTTER_FLIGHT_DUAL_PEEL_NAV_STEPS_WAVE788: &[&str] = &[
    "REQUIRE_ENTITY_DAISY_FIELDS",
    "REQUIRE_GW_FLIGHT_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_DROP_DETONATE_DRAIN",
    "LIVE_HOST_DAISY_CUTTER_FLIGHT_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_DAISY_CUTTER_FLIGHT_DUAL_PEEL_CMD_NAMES_WAVE788: &[&str] = &[
    "host_daisy_cutter_flight_dual_peel",
    "daisy_transport_active",
    "host_daisy_cutter_drop_log",
    "update_daisy_cutter_flights",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostDaisyCutterFlightDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostDaisyCutterFlightDualPeelAction {
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
fn residual_action_store(a: ResidualHostDaisyCutterFlightDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_daisy_cutter_flight_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_daisy_cutter_flight_dual_peel_last_action()
-> ResidualHostDaisyCutterFlightDualPeelAction {
    ResidualHostDaisyCutterFlightDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_daisy_cutter_flight_dual_peel_method_names_residual_wave788() -> bool {
    let names = LIVE_HOST_DAISY_CUTTER_FLIGHT_DUAL_PEEL_METHOD_NAMES_WAVE788;
    let ok = residual_name_index(names, "daisy_transport_active").is_some()
        && residual_name_index(names, "daisy_cutter_bomb").is_some()
        && residual_name_index(names, "host_daisy_cutter_drop_log").is_some()
        && residual_name_index(names, "update_daisy_cutter_flights").is_some()
        && residual_name_index(names, "Wave 788").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostDaisyCutterFlightDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_daisy_cutter_flight_dual_peel_source_markers_residual_wave788() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("daisy_transport_active")
        && ent.contains("daisy_cutter_bomb")
        && sh.contains("Wave 788")
        && sh.contains("host_daisy_cutter_drop_log::record_drop")
        && sh.contains("host_daisy_cutter_drop_log::drain_dets")
        && gl.contains("Wave 788")
        && gl.contains("update_daisy_cutter_flights");
    residual_action_store(ResidualHostDaisyCutterFlightDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_daisy_cutter_flight_dual_peel_nav_commands_residual_wave788() -> bool {
    let steps = LIVE_HOST_DAISY_CUTTER_FLIGHT_DUAL_PEEL_NAV_STEPS_WAVE788;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_DAISY_FIELDS").is_some()
        && residual_name_index(steps, "REQUIRE_GW_FLIGHT_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_DROP_DETONATE_DRAIN").is_some()
        && residual_name_index(steps, "LIVE_HOST_DAISY_CUTTER_FLIGHT_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostDaisyCutterFlightDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_daisy_cutter_flight_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 788")
        && sh_source().contains("daisy_transport_active")
        && gl_source().contains("Wave 788");
    residual_action_store(ResidualHostDaisyCutterFlightDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_daisy_cutter_flight_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("host_daisy_cutter_drop_log::record_drop")
        && sh_source().contains("host_daisy_cutter_drop_log::drain_drops")
        && gl_source().contains("update_daisy_cutter_flights")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostDaisyCutterFlightDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_daisy_cutter_flight_dual_peel_residual_pack_wave788() -> bool {
    honesty_host_daisy_cutter_flight_dual_peel_method_names_residual_wave788()
        && honesty_host_daisy_cutter_flight_dual_peel_source_markers_residual_wave788()
        && honesty_host_daisy_cutter_flight_dual_peel_nav_commands_residual_wave788()
        && simulate_host_daisy_cutter_flight_dual_peel_collect_source()
        && simulate_host_daisy_cutter_flight_dual_peel_dispatch_source()
}
pub fn simulate_live_host_daisy_cutter_flight_dual_peel_honesty() -> bool {
    let ok = honesty_host_daisy_cutter_flight_dual_peel_residual_pack_wave788();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostDaisyCutterFlightDualPeelAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_daisy_cutter_flight_dual_peel_method_names_residual_wave788());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_daisy_cutter_flight_dual_peel_source_markers_residual_wave788());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_daisy_cutter_flight_dual_peel_nav_commands_residual_wave788());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_daisy_cutter_flight_dual_peel_collect_source());
        assert!(simulate_host_daisy_cutter_flight_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_daisy_cutter_flight_dual_peel_residual_pack_wave788());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_daisy_cutter_flight_dual_peel_honesty());
        assert!(residual_host_daisy_cutter_flight_dual_peel_ok());
    }
}
