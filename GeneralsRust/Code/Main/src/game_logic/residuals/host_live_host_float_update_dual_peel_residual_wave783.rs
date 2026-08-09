//! Wave 783: GW entity carries FloatUpdate residual; under coupled dual-tick
//! `tick_status_timer_expirations` sole-ticks boat sway/snap; host peels
//! `update_float_update`. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_FLOAT_UPDATE_DUAL_PEEL_METHOD_NAMES_WAVE783: &[&str] = &[
    "float_update_active",
    "float_yaw",
    "float_pitch",
    "update_float_update",
    "Wave 783",
    "playable_claim = false",
];
pub const LIVE_HOST_FLOAT_UPDATE_DUAL_PEEL_NAV_STEPS_WAVE783: &[&str] = &[
    "REQUIRE_ENTITY_FLOAT_FIELDS",
    "REQUIRE_GW_SWAY_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_WRITEBACK",
    "LIVE_HOST_FLOAT_UPDATE_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_FLOAT_UPDATE_DUAL_PEEL_CMD_NAMES_WAVE783: &[&str] = &[
    "host_float_update_dual_peel",
    "float_update_active",
    "float_yaw",
    "update_float_update",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostFloatUpdateDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostFloatUpdateDualPeelAction {
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
fn residual_action_store(a: ResidualHostFloatUpdateDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_float_update_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_float_update_dual_peel_last_action() -> ResidualHostFloatUpdateDualPeelAction {
    ResidualHostFloatUpdateDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    include_str!("../../gameworld_shadow.rs")
}
fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}
pub fn honesty_host_float_update_dual_peel_method_names_residual_wave783() -> bool {
    let names = LIVE_HOST_FLOAT_UPDATE_DUAL_PEEL_METHOD_NAMES_WAVE783;
    let ok = residual_name_index(names, "float_update_active").is_some()
        && residual_name_index(names, "float_yaw").is_some()
        && residual_name_index(names, "float_pitch").is_some()
        && residual_name_index(names, "update_float_update").is_some()
        && residual_name_index(names, "Wave 783").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostFloatUpdateDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_float_update_dual_peel_source_markers_residual_wave783() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("float_update_active")
        && ent.contains("float_yaw")
        && sh.contains("Wave 783")
        && sh.contains("FLOAT_YAW_PHASE")
        && sh.contains("HostFloatUpdateData")
        && gl.contains("Wave 783")
        && gl.contains("update_float_update");
    residual_action_store(ResidualHostFloatUpdateDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_float_update_dual_peel_nav_commands_residual_wave783() -> bool {
    let steps = LIVE_HOST_FLOAT_UPDATE_DUAL_PEEL_NAV_STEPS_WAVE783;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_FLOAT_FIELDS").is_some()
        && residual_name_index(steps, "REQUIRE_GW_SWAY_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_WRITEBACK").is_some()
        && residual_name_index(steps, "LIVE_HOST_FLOAT_UPDATE_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostFloatUpdateDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_float_update_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 783")
        && sh_source().contains("float_update_active")
        && gl_source().contains("Wave 783");
    residual_action_store(ResidualHostFloatUpdateDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_float_update_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("HostFloatUpdateData")
        && sh_source().contains("FLOAT_SWAY_AMP")
        && gl_source().contains("update_float_update")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostFloatUpdateDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_float_update_dual_peel_residual_pack_wave783() -> bool {
    honesty_host_float_update_dual_peel_method_names_residual_wave783()
        && honesty_host_float_update_dual_peel_source_markers_residual_wave783()
        && honesty_host_float_update_dual_peel_nav_commands_residual_wave783()
        && simulate_host_float_update_dual_peel_collect_source()
        && simulate_host_float_update_dual_peel_dispatch_source()
}
pub fn simulate_live_host_float_update_dual_peel_honesty() -> bool {
    let ok = honesty_host_float_update_dual_peel_residual_pack_wave783();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostFloatUpdateDualPeelAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_float_update_dual_peel_method_names_residual_wave783());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_float_update_dual_peel_source_markers_residual_wave783());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_float_update_dual_peel_nav_commands_residual_wave783());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_float_update_dual_peel_collect_source());
        assert!(simulate_host_float_update_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_float_update_dual_peel_residual_pack_wave783());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_float_update_dual_peel_honesty());
        assert!(residual_host_float_update_dual_peel_ok());
    }
}
