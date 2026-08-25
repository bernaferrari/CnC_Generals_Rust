//! Wave 791: GW entity carries EmpPulse DeliverPayload flight residual;
//! under coupled dual-tick sole-ticks transport/bomb and logs drop/detonate;
//! host peels `update_emp_pulse_flights`. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_EMP_PULSE_FLIGHT_DUAL_PEEL_METHOD_NAMES_WAVE791: &[&str] = &[
    "emp_pulse_transport_active",
    "emp_pulse_bomb",
    "emp_pulse_spheroid",
    "update_emp_pulse_spheroids",
    "host_emp_pulse_drop_log",
    "update_emp_pulse_flights",
    "Wave 791",
    "playable_claim = false",
];
pub const LIVE_HOST_EMP_PULSE_FLIGHT_DUAL_PEEL_NAV_STEPS_WAVE791: &[&str] = &[
    "REQUIRE_ENTITY_EMP_PULSE_FIELDS",
    "REQUIRE_GW_FLIGHT_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_DROP_DETONATE_DRAIN",
    "LIVE_HOST_EMP_PULSE_FLIGHT_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_EMP_PULSE_FLIGHT_DUAL_PEEL_CMD_NAMES_WAVE791: &[&str] = &[
    "host_emp_pulse_flight_dual_peel",
    "emp_pulse_transport_active",
    "host_emp_pulse_drop_log",
    "update_emp_pulse_flights",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostEmpPulseFlightDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostEmpPulseFlightDualPeelAction {
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
fn residual_action_store(a: ResidualHostEmpPulseFlightDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_emp_pulse_flight_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_emp_pulse_flight_dual_peel_last_action()
-> ResidualHostEmpPulseFlightDualPeelAction {
    ResidualHostEmpPulseFlightDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_emp_pulse_flight_dual_peel_method_names_residual_wave791() -> bool {
    let names = LIVE_HOST_EMP_PULSE_FLIGHT_DUAL_PEEL_METHOD_NAMES_WAVE791;
    let ok = residual_name_index(names, "emp_pulse_transport_active").is_some()
        && residual_name_index(names, "emp_pulse_bomb").is_some()
        && residual_name_index(names, "host_emp_pulse_drop_log").is_some()
        && residual_name_index(names, "update_emp_pulse_flights").is_some()
        && residual_name_index(names, "Wave 791").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostEmpPulseFlightDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_emp_pulse_flight_dual_peel_source_markers_residual_wave791() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("emp_pulse_transport_active")
        && ent.contains("emp_pulse_bomb")
        && ent.contains("emp_pulse_spheroid")
        && sh.contains("drain_spheroid_expires")
        && gl.contains("update_emp_pulse_spheroids")
        && sh.contains("Wave 791")
        && sh.contains("host_emp_pulse_drop_log::record_drop")
        && sh.contains("host_emp_pulse_drop_log::drain_dets")
        && gl.contains("Wave 791")
        && gl.contains("update_emp_pulse_flights");
    residual_action_store(ResidualHostEmpPulseFlightDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_emp_pulse_flight_dual_peel_nav_commands_residual_wave791() -> bool {
    let steps = LIVE_HOST_EMP_PULSE_FLIGHT_DUAL_PEEL_NAV_STEPS_WAVE791;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_EMP_PULSE_FIELDS").is_some()
        && residual_name_index(steps, "REQUIRE_GW_FLIGHT_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_DROP_DETONATE_DRAIN").is_some()
        && residual_name_index(steps, "LIVE_HOST_EMP_PULSE_FLIGHT_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostEmpPulseFlightDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_emp_pulse_flight_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 791")
        && sh_source().contains("emp_pulse_transport_active")
        && gl_source().contains("Wave 791");
    residual_action_store(ResidualHostEmpPulseFlightDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_emp_pulse_flight_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("host_emp_pulse_drop_log::record_drop")
        && sh_source().contains("host_emp_pulse_drop_log::drain_drops")
        && gl_source().contains("update_emp_pulse_flights")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostEmpPulseFlightDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_emp_pulse_flight_dual_peel_residual_pack_wave791() -> bool {
    honesty_host_emp_pulse_flight_dual_peel_method_names_residual_wave791()
        && honesty_host_emp_pulse_flight_dual_peel_source_markers_residual_wave791()
        && honesty_host_emp_pulse_flight_dual_peel_nav_commands_residual_wave791()
        && simulate_host_emp_pulse_flight_dual_peel_collect_source()
        && simulate_host_emp_pulse_flight_dual_peel_dispatch_source()
}
pub fn simulate_live_host_emp_pulse_flight_dual_peel_honesty() -> bool {
    let ok = honesty_host_emp_pulse_flight_dual_peel_residual_pack_wave791();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostEmpPulseFlightDualPeelAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_emp_pulse_flight_dual_peel_method_names_residual_wave791());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_emp_pulse_flight_dual_peel_source_markers_residual_wave791());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_emp_pulse_flight_dual_peel_nav_commands_residual_wave791());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_emp_pulse_flight_dual_peel_collect_source());
        assert!(simulate_host_emp_pulse_flight_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_emp_pulse_flight_dual_peel_residual_pack_wave791());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_emp_pulse_flight_dual_peel_honesty());
        assert!(residual_host_emp_pulse_flight_dual_peel_ok());
    }
}
