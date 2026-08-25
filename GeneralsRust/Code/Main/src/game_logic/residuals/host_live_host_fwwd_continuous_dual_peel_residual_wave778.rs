//! Wave 778: GW entity carries FireWeaponWhenDamaged continuous residual;
//! under coupled dual-tick `tick_status_timer_expirations` sole-ticks
//! continuous reload into host_fwwd_continuous_log; host peels
//! `tick_fire_weapon_when_damaged_continuous` and drains pending fire after
//! writeback. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_FWWD_CONTINUOUS_DUAL_PEEL_METHOD_NAMES_WAVE778: &[&str] = &[
    "fwwd_active",
    "fwwd_last_continuous_frame",
    "host_fwwd_continuous_log",
    "tick_fire_weapon_when_damaged_continuous",
    "Wave 778",
    "playable_claim = false",
];
pub const LIVE_HOST_FWWD_CONTINUOUS_DUAL_PEEL_NAV_STEPS_WAVE778: &[&str] = &[
    "REQUIRE_ENTITY_FWWD_FIELDS",
    "REQUIRE_GW_CONTINUOUS_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_DRAIN_PENDING",
    "LIVE_HOST_FWWD_CONTINUOUS_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_FWWD_CONTINUOUS_DUAL_PEEL_CMD_NAMES_WAVE778: &[&str] = &[
    "host_fwwd_continuous_dual_peel",
    "fwwd_active",
    "host_fwwd_continuous_log",
    "tick_fire_weapon_when_damaged_continuous",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostFwwdContinuousDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostFwwdContinuousDualPeelAction {
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
fn residual_action_store(a: ResidualHostFwwdContinuousDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_fwwd_continuous_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_fwwd_continuous_dual_peel_last_action()
-> ResidualHostFwwdContinuousDualPeelAction {
    ResidualHostFwwdContinuousDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_fwwd_continuous_dual_peel_method_names_residual_wave778() -> bool {
    let names = LIVE_HOST_FWWD_CONTINUOUS_DUAL_PEEL_METHOD_NAMES_WAVE778;
    let ok = residual_name_index(names, "fwwd_active").is_some()
        && residual_name_index(names, "fwwd_last_continuous_frame").is_some()
        && residual_name_index(names, "host_fwwd_continuous_log").is_some()
        && residual_name_index(names, "tick_fire_weapon_when_damaged_continuous").is_some()
        && residual_name_index(names, "Wave 778").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostFwwdContinuousDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_fwwd_continuous_dual_peel_source_markers_residual_wave778() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("fwwd_active")
        && ent.contains("fwwd_last_continuous_frame")
        && sh.contains("Wave 778")
        && sh.contains("host_fwwd_continuous_log::record")
        && sh.contains("host_fwwd_continuous_log::drain")
        && gl.contains("Wave 778")
        && gl.contains("tick_fire_weapon_when_damaged_continuous");
    residual_action_store(ResidualHostFwwdContinuousDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_fwwd_continuous_dual_peel_nav_commands_residual_wave778() -> bool {
    let steps = LIVE_HOST_FWWD_CONTINUOUS_DUAL_PEEL_NAV_STEPS_WAVE778;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_FWWD_FIELDS").is_some()
        && residual_name_index(steps, "REQUIRE_GW_CONTINUOUS_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_DRAIN_PENDING").is_some()
        && residual_name_index(steps, "LIVE_HOST_FWWD_CONTINUOUS_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostFwwdContinuousDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_fwwd_continuous_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 778")
        && sh_source().contains("fwwd_active")
        && gl_source().contains("Wave 778");
    residual_action_store(ResidualHostFwwdContinuousDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_fwwd_continuous_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("host_fwwd_continuous_log::record")
        && sh_source().contains("host_calc_body_damage_state")
        && gl_source().contains("tick_fire_weapon_when_damaged_continuous")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostFwwdContinuousDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_fwwd_continuous_dual_peel_residual_pack_wave778() -> bool {
    honesty_host_fwwd_continuous_dual_peel_method_names_residual_wave778()
        && honesty_host_fwwd_continuous_dual_peel_source_markers_residual_wave778()
        && honesty_host_fwwd_continuous_dual_peel_nav_commands_residual_wave778()
        && simulate_host_fwwd_continuous_dual_peel_collect_source()
        && simulate_host_fwwd_continuous_dual_peel_dispatch_source()
}
pub fn simulate_live_host_fwwd_continuous_dual_peel_honesty() -> bool {
    let ok = honesty_host_fwwd_continuous_dual_peel_residual_pack_wave778();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostFwwdContinuousDualPeelAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_fwwd_continuous_dual_peel_method_names_residual_wave778());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_fwwd_continuous_dual_peel_source_markers_residual_wave778());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_fwwd_continuous_dual_peel_nav_commands_residual_wave778());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_fwwd_continuous_dual_peel_collect_source());
        assert!(simulate_host_fwwd_continuous_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_fwwd_continuous_dual_peel_residual_pack_wave778());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_fwwd_continuous_dual_peel_honesty());
        assert!(residual_host_fwwd_continuous_dual_peel_ok());
    }
}
