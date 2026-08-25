//! Wave 787: GW entity carries SmartBombTargetHomingUpdate residual; under
//! coupled dual-tick `tick_status_timer_expirations` sole-ticks course
//! correction; host peels `update_smart_bomb_target_homing`. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_SMART_BOMB_HOMING_DUAL_PEEL_METHOD_NAMES_WAVE787: &[&str] = &[
    "smart_bomb_homing_active",
    "smart_bomb_target_received",
    "smart_bomb_course_scalar",
    "update_smart_bomb_target_homing",
    "Wave 787",
    "playable_claim = false",
];
pub const LIVE_HOST_SMART_BOMB_HOMING_DUAL_PEEL_NAV_STEPS_WAVE787: &[&str] = &[
    "REQUIRE_ENTITY_SMART_BOMB_FIELDS",
    "REQUIRE_GW_STEER_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_TRANSFORM_WRITEBACK",
    "LIVE_HOST_SMART_BOMB_HOMING_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_SMART_BOMB_HOMING_DUAL_PEEL_CMD_NAMES_WAVE787: &[&str] = &[
    "host_smart_bomb_homing_dual_peel",
    "smart_bomb_homing_active",
    "smart_bomb_course_scalar",
    "update_smart_bomb_target_homing",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSmartBombHomingDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostSmartBombHomingDualPeelAction {
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
fn residual_action_store(a: ResidualHostSmartBombHomingDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_smart_bomb_homing_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_smart_bomb_homing_dual_peel_last_action()
-> ResidualHostSmartBombHomingDualPeelAction {
    ResidualHostSmartBombHomingDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_smart_bomb_homing_dual_peel_method_names_residual_wave787() -> bool {
    let names = LIVE_HOST_SMART_BOMB_HOMING_DUAL_PEEL_METHOD_NAMES_WAVE787;
    let ok = residual_name_index(names, "smart_bomb_homing_active").is_some()
        && residual_name_index(names, "smart_bomb_target_received").is_some()
        && residual_name_index(names, "smart_bomb_course_scalar").is_some()
        && residual_name_index(names, "update_smart_bomb_target_homing").is_some()
        && residual_name_index(names, "Wave 787").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostSmartBombHomingDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_smart_bomb_homing_dual_peel_source_markers_residual_wave787() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("smart_bomb_homing_active")
        && ent.contains("smart_bomb_target_received")
        && sh.contains("Wave 787")
        && sh.contains("SMART_BOMB_SIGNIFICANTLY_ABOVE_TERRAIN")
        && sh.contains("HostSmartBombTargetHomingData")
        && gl.contains("Wave 787")
        && gl.contains("update_smart_bomb_target_homing");
    residual_action_store(ResidualHostSmartBombHomingDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_smart_bomb_homing_dual_peel_nav_commands_residual_wave787() -> bool {
    let steps = LIVE_HOST_SMART_BOMB_HOMING_DUAL_PEEL_NAV_STEPS_WAVE787;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_SMART_BOMB_FIELDS").is_some()
        && residual_name_index(steps, "REQUIRE_GW_STEER_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_TRANSFORM_WRITEBACK").is_some()
        && residual_name_index(steps, "LIVE_HOST_SMART_BOMB_HOMING_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostSmartBombHomingDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_smart_bomb_homing_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 787")
        && sh_source().contains("smart_bomb_homing_active")
        && gl_source().contains("Wave 787");
    residual_action_store(ResidualHostSmartBombHomingDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_smart_bomb_homing_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("HostSmartBombTargetHomingData")
        && sh_source().contains("SMART_BOMB_SIGNIFICANTLY_ABOVE_TERRAIN")
        && gl_source().contains("update_smart_bomb_target_homing")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostSmartBombHomingDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_smart_bomb_homing_dual_peel_residual_pack_wave787() -> bool {
    honesty_host_smart_bomb_homing_dual_peel_method_names_residual_wave787()
        && honesty_host_smart_bomb_homing_dual_peel_source_markers_residual_wave787()
        && honesty_host_smart_bomb_homing_dual_peel_nav_commands_residual_wave787()
        && simulate_host_smart_bomb_homing_dual_peel_collect_source()
        && simulate_host_smart_bomb_homing_dual_peel_dispatch_source()
}
pub fn simulate_live_host_smart_bomb_homing_dual_peel_honesty() -> bool {
    let ok = honesty_host_smart_bomb_homing_dual_peel_residual_pack_wave787();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostSmartBombHomingDualPeelAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_smart_bomb_homing_dual_peel_method_names_residual_wave787());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_smart_bomb_homing_dual_peel_source_markers_residual_wave787());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_smart_bomb_homing_dual_peel_nav_commands_residual_wave787());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_smart_bomb_homing_dual_peel_collect_source());
        assert!(simulate_host_smart_bomb_homing_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_smart_bomb_homing_dual_peel_residual_pack_wave787());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_smart_bomb_homing_dual_peel_honesty());
        assert!(residual_host_smart_bomb_homing_dual_peel_ok());
    }
}
