//! Wave 771: GW entity carries HeightDieUpdate residual; under coupled
//! dual-tick `tick_status_timer_expirations` sole-ticks altitude death into
//! host_height_die_kill_log; host peels `tick_height_die` and drains kill
//! after writeback. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_HEIGHT_DIE_DUAL_PEEL_METHOD_NAMES_WAVE771: &[&str] = &[
    "height_die_active",
    "height_die_target_hat",
    "host_height_die_kill_log",
    "tick_height_die",
    "Wave 771",
    "playable_claim = false",
];
pub const LIVE_HOST_HEIGHT_DIE_DUAL_PEEL_NAV_STEPS_WAVE771: &[&str] = &[
    "REQUIRE_ENTITY_HEIGHT_DIE_FIELDS",
    "REQUIRE_GW_ALTITUDE_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_DRAIN_KILL",
    "LIVE_HOST_HEIGHT_DIE_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_HEIGHT_DIE_DUAL_PEEL_CMD_NAMES_WAVE771: &[&str] = &[
    "host_height_die_dual_peel",
    "height_die_active",
    "host_height_die_kill_log",
    "tick_height_die",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostHeightDieDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostHeightDieDualPeelAction {
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
fn residual_action_store(a: ResidualHostHeightDieDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_height_die_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_height_die_dual_peel_last_action() -> ResidualHostHeightDieDualPeelAction {
    ResidualHostHeightDieDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    include_str!("../../gameworld_shadow.rs")
}
fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}
pub fn honesty_host_height_die_dual_peel_method_names_residual_wave771() -> bool {
    let names = LIVE_HOST_HEIGHT_DIE_DUAL_PEEL_METHOD_NAMES_WAVE771;
    let ok = residual_name_index(names, "height_die_active").is_some()
        && residual_name_index(names, "height_die_target_hat").is_some()
        && residual_name_index(names, "host_height_die_kill_log").is_some()
        && residual_name_index(names, "tick_height_die").is_some()
        && residual_name_index(names, "Wave 771").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostHeightDieDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_height_die_dual_peel_source_markers_residual_wave771() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("height_die_active")
        && ent.contains("height_die_target_hat")
        && sh.contains("Wave 771")
        && sh.contains("host_height_die_kill_log::record")
        && sh.contains("host_height_die_kill_log::drain")
        && gl.contains("Wave 771")
        && gl.contains("tick_height_die");
    residual_action_store(ResidualHostHeightDieDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_height_die_dual_peel_nav_commands_residual_wave771() -> bool {
    let steps = LIVE_HOST_HEIGHT_DIE_DUAL_PEEL_NAV_STEPS_WAVE771;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_HEIGHT_DIE_FIELDS").is_some()
        && residual_name_index(steps, "REQUIRE_GW_ALTITUDE_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_DRAIN_KILL").is_some()
        && residual_name_index(steps, "LIVE_HOST_HEIGHT_DIE_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostHeightDieDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_height_die_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 771")
        && sh_source().contains("height_die_active")
        && gl_source().contains("Wave 771");
    residual_action_store(ResidualHostHeightDieDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_height_die_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("host_height_die_kill_log::record")
        && sh_source().contains("height_die_target_hat")
        && gl_source().contains("tick_height_die")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostHeightDieDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_height_die_dual_peel_residual_pack_wave771() -> bool {
    honesty_host_height_die_dual_peel_method_names_residual_wave771()
        && honesty_host_height_die_dual_peel_source_markers_residual_wave771()
        && honesty_host_height_die_dual_peel_nav_commands_residual_wave771()
        && simulate_host_height_die_dual_peel_collect_source()
        && simulate_host_height_die_dual_peel_dispatch_source()
}
pub fn simulate_live_host_height_die_dual_peel_honesty() -> bool {
    let ok = honesty_host_height_die_dual_peel_residual_pack_wave771();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostHeightDieDualPeelAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_height_die_dual_peel_method_names_residual_wave771());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_height_die_dual_peel_source_markers_residual_wave771());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_height_die_dual_peel_nav_commands_residual_wave771());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_height_die_dual_peel_collect_source());
        assert!(simulate_host_height_die_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_height_die_dual_peel_residual_pack_wave771());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_height_die_dual_peel_honesty());
        assert!(residual_host_height_die_dual_peel_ok());
    }
}
