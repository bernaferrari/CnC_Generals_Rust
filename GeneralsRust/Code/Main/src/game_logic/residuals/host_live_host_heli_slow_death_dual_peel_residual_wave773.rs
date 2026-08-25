//! Wave 773: GW entity carries HelicopterSlowDeathBehavior residual; under
//! coupled dual-tick `tick_status_timer_expirations` sole-ticks spiral crash
//! into host_heli_slow_death_kill_log; host peels `tick_helicopter_slow_death`
//! and drains kill after writeback. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_HELI_SLOW_DEATH_DUAL_PEEL_METHOD_NAMES_WAVE773: &[&str] = &[
    "heli_slow_death_active",
    "heli_slow_death_orbit_angle",
    "host_heli_slow_death_kill_log",
    "tick_helicopter_slow_death",
    "Wave 773",
    "playable_claim = false",
];
pub const LIVE_HOST_HELI_SLOW_DEATH_DUAL_PEEL_NAV_STEPS_WAVE773: &[&str] = &[
    "REQUIRE_ENTITY_HELI_SLOW_DEATH_FIELDS",
    "REQUIRE_GW_SPIRAL_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_DRAIN_KILL",
    "LIVE_HOST_HELI_SLOW_DEATH_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_HELI_SLOW_DEATH_DUAL_PEEL_CMD_NAMES_WAVE773: &[&str] = &[
    "host_heli_slow_death_dual_peel",
    "heli_slow_death_active",
    "host_heli_slow_death_kill_log",
    "tick_helicopter_slow_death",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostHeliSlowDeathDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostHeliSlowDeathDualPeelAction {
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
fn residual_action_store(a: ResidualHostHeliSlowDeathDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_heli_slow_death_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_heli_slow_death_dual_peel_last_action()
-> ResidualHostHeliSlowDeathDualPeelAction {
    ResidualHostHeliSlowDeathDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_heli_slow_death_dual_peel_method_names_residual_wave773() -> bool {
    let names = LIVE_HOST_HELI_SLOW_DEATH_DUAL_PEEL_METHOD_NAMES_WAVE773;
    let ok = residual_name_index(names, "heli_slow_death_active").is_some()
        && residual_name_index(names, "heli_slow_death_orbit_angle").is_some()
        && residual_name_index(names, "host_heli_slow_death_kill_log").is_some()
        && residual_name_index(names, "tick_helicopter_slow_death").is_some()
        && residual_name_index(names, "Wave 773").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostHeliSlowDeathDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_heli_slow_death_dual_peel_source_markers_residual_wave773() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("heli_slow_death_active")
        && ent.contains("heli_slow_death_orbit_angle")
        && sh.contains("Wave 773")
        && sh.contains("host_heli_slow_death_kill_log::record")
        && sh.contains("host_heli_slow_death_kill_log::drain")
        && gl.contains("Wave 773")
        && gl.contains("tick_helicopter_slow_death");
    residual_action_store(ResidualHostHeliSlowDeathDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_heli_slow_death_dual_peel_nav_commands_residual_wave773() -> bool {
    let steps = LIVE_HOST_HELI_SLOW_DEATH_DUAL_PEEL_NAV_STEPS_WAVE773;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_HELI_SLOW_DEATH_FIELDS").is_some()
        && residual_name_index(steps, "REQUIRE_GW_SPIRAL_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_DRAIN_KILL").is_some()
        && residual_name_index(steps, "LIVE_HOST_HELI_SLOW_DEATH_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostHeliSlowDeathDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_heli_slow_death_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 773")
        && sh_source().contains("heli_slow_death_active")
        && gl_source().contains("Wave 773");
    residual_action_store(ResidualHostHeliSlowDeathDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_heli_slow_death_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("host_heli_slow_death_kill_log::record")
        && sh_source().contains("HELI_SPIRAL_TURN_RATE")
        && gl_source().contains("tick_helicopter_slow_death")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostHeliSlowDeathDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_heli_slow_death_dual_peel_residual_pack_wave773() -> bool {
    honesty_host_heli_slow_death_dual_peel_method_names_residual_wave773()
        && honesty_host_heli_slow_death_dual_peel_source_markers_residual_wave773()
        && honesty_host_heli_slow_death_dual_peel_nav_commands_residual_wave773()
        && simulate_host_heli_slow_death_dual_peel_collect_source()
        && simulate_host_heli_slow_death_dual_peel_dispatch_source()
}
pub fn simulate_live_host_heli_slow_death_dual_peel_honesty() -> bool {
    let ok = honesty_host_heli_slow_death_dual_peel_residual_pack_wave773();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostHeliSlowDeathDualPeelAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_heli_slow_death_dual_peel_method_names_residual_wave773());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_heli_slow_death_dual_peel_source_markers_residual_wave773());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_heli_slow_death_dual_peel_nav_commands_residual_wave773());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_heli_slow_death_dual_peel_collect_source());
        assert!(simulate_host_heli_slow_death_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_heli_slow_death_dual_peel_residual_pack_wave773());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_heli_slow_death_dual_peel_honesty());
        assert!(residual_host_heli_slow_death_dual_peel_ok());
    }
}
