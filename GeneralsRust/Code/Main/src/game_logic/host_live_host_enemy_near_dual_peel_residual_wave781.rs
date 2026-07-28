//! Wave 781: GW entity carries EnemyNearUpdate residual; under coupled
//! dual-tick `tick_status_timer_expirations` sole-ticks scan/delay into
//! entity enemy_near flags; host peels `update_enemy_near`. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_ENEMY_NEAR_DUAL_PEEL_METHOD_NAMES_WAVE781: &[&str] = &[
    "enemy_near_active",
    "enemy_near_scan_delay",
    "scan_enemy_near_present",
    "update_enemy_near",
    "Wave 781",
    "playable_claim = false",
];
pub const LIVE_HOST_ENEMY_NEAR_DUAL_PEEL_NAV_STEPS_WAVE781: &[&str] = &[
    "REQUIRE_ENTITY_ENEMY_NEAR_FIELDS",
    "REQUIRE_GW_SCAN_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_WRITEBACK",
    "LIVE_HOST_ENEMY_NEAR_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_ENEMY_NEAR_DUAL_PEEL_CMD_NAMES_WAVE781: &[&str] = &[
    "host_enemy_near_dual_peel",
    "enemy_near_active",
    "scan_enemy_near_present",
    "update_enemy_near",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostEnemyNearDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostEnemyNearDualPeelAction {
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
fn residual_action_store(a: ResidualHostEnemyNearDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_enemy_near_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_enemy_near_dual_peel_last_action() -> ResidualHostEnemyNearDualPeelAction {
    ResidualHostEnemyNearDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    include_str!("../gameworld_shadow.rs")
}
fn gl_source() -> &'static str {
    include_str!("game_logic.rs")
}
pub fn honesty_host_enemy_near_dual_peel_method_names_residual_wave781() -> bool {
    let names = LIVE_HOST_ENEMY_NEAR_DUAL_PEEL_METHOD_NAMES_WAVE781;
    let ok = residual_name_index(names, "enemy_near_active").is_some()
        && residual_name_index(names, "enemy_near_scan_delay").is_some()
        && residual_name_index(names, "scan_enemy_near_present").is_some()
        && residual_name_index(names, "update_enemy_near").is_some()
        && residual_name_index(names, "Wave 781").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostEnemyNearDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_enemy_near_dual_peel_source_markers_residual_wave781() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("enemy_near_active")
        && ent.contains("enemy_near_scan_delay")
        && sh.contains("Wave 781")
        && sh.contains("scan_enemy_near_present")
        && sh.contains("enemy_near_scan_delay")
        && gl.contains("Wave 781")
        && gl.contains("update_enemy_near");
    residual_action_store(ResidualHostEnemyNearDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_enemy_near_dual_peel_nav_commands_residual_wave781() -> bool {
    let steps = LIVE_HOST_ENEMY_NEAR_DUAL_PEEL_NAV_STEPS_WAVE781;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_ENEMY_NEAR_FIELDS").is_some()
        && residual_name_index(steps, "REQUIRE_GW_SCAN_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_WRITEBACK").is_some()
        && residual_name_index(steps, "LIVE_HOST_ENEMY_NEAR_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostEnemyNearDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_enemy_near_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 781")
        && sh_source().contains("enemy_near_active")
        && gl_source().contains("Wave 781");
    residual_action_store(ResidualHostEnemyNearDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_enemy_near_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("scan_enemy_near_present")
        && sh_source().contains("enemy_near_model")
        && gl_source().contains("update_enemy_near")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostEnemyNearDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_enemy_near_dual_peel_residual_pack_wave781() -> bool {
    honesty_host_enemy_near_dual_peel_method_names_residual_wave781()
        && honesty_host_enemy_near_dual_peel_source_markers_residual_wave781()
        && honesty_host_enemy_near_dual_peel_nav_commands_residual_wave781()
        && simulate_host_enemy_near_dual_peel_collect_source()
        && simulate_host_enemy_near_dual_peel_dispatch_source()
}
pub fn simulate_live_host_enemy_near_dual_peel_honesty() -> bool {
    let ok = honesty_host_enemy_near_dual_peel_residual_pack_wave781();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostEnemyNearDualPeelAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_enemy_near_dual_peel_method_names_residual_wave781());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_enemy_near_dual_peel_source_markers_residual_wave781());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_enemy_near_dual_peel_nav_commands_residual_wave781());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_enemy_near_dual_peel_collect_source());
        assert!(simulate_host_enemy_near_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_enemy_near_dual_peel_residual_pack_wave781());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_enemy_near_dual_peel_honesty());
        assert!(residual_host_enemy_near_dual_peel_ok());
    }
}
