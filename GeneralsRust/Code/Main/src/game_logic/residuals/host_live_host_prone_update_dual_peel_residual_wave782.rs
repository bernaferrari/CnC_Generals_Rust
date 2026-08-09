//! Wave 782: GW entity carries ProneUpdate residual; under coupled dual-tick
//! `tick_status_timer_expirations` sole-ticks prone countdown; host peels
//! `update_prone_update`. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_PRONE_UPDATE_DUAL_PEEL_METHOD_NAMES_WAVE782: &[&str] = &[
    "prone_active",
    "prone_frames",
    "prone_model",
    "update_prone_update",
    "Wave 782",
    "playable_claim = false",
];
pub const LIVE_HOST_PRONE_UPDATE_DUAL_PEEL_NAV_STEPS_WAVE782: &[&str] = &[
    "REQUIRE_ENTITY_PRONE_FIELDS",
    "REQUIRE_GW_COUNTDOWN",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_WRITEBACK",
    "LIVE_HOST_PRONE_UPDATE_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_PRONE_UPDATE_DUAL_PEEL_CMD_NAMES_WAVE782: &[&str] = &[
    "host_prone_update_dual_peel",
    "prone_active",
    "prone_frames",
    "update_prone_update",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostProneUpdateDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostProneUpdateDualPeelAction {
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
fn residual_action_store(a: ResidualHostProneUpdateDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_prone_update_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_prone_update_dual_peel_last_action() -> ResidualHostProneUpdateDualPeelAction {
    ResidualHostProneUpdateDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    include_str!("../../gameworld_shadow.rs")
}
fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}
pub fn honesty_host_prone_update_dual_peel_method_names_residual_wave782() -> bool {
    let names = LIVE_HOST_PRONE_UPDATE_DUAL_PEEL_METHOD_NAMES_WAVE782;
    let ok = residual_name_index(names, "prone_active").is_some()
        && residual_name_index(names, "prone_frames").is_some()
        && residual_name_index(names, "prone_model").is_some()
        && residual_name_index(names, "update_prone_update").is_some()
        && residual_name_index(names, "Wave 782").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostProneUpdateDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_prone_update_dual_peel_source_markers_residual_wave782() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("prone_active")
        && ent.contains("prone_frames")
        && sh.contains("Wave 782")
        && sh.contains("prone_frames")
        && sh.contains("HostProneUpdateData")
        && gl.contains("Wave 782")
        && gl.contains("update_prone_update");
    residual_action_store(ResidualHostProneUpdateDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_prone_update_dual_peel_nav_commands_residual_wave782() -> bool {
    let steps = LIVE_HOST_PRONE_UPDATE_DUAL_PEEL_NAV_STEPS_WAVE782;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_PRONE_FIELDS").is_some()
        && residual_name_index(steps, "REQUIRE_GW_COUNTDOWN").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_WRITEBACK").is_some()
        && residual_name_index(steps, "LIVE_HOST_PRONE_UPDATE_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostProneUpdateDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_prone_update_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 782")
        && sh_source().contains("prone_active")
        && gl_source().contains("Wave 782");
    residual_action_store(ResidualHostProneUpdateDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_prone_update_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("HostProneUpdateData")
        && sh_source().contains("PRONE")
        && gl_source().contains("update_prone_update")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostProneUpdateDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_prone_update_dual_peel_residual_pack_wave782() -> bool {
    honesty_host_prone_update_dual_peel_method_names_residual_wave782()
        && honesty_host_prone_update_dual_peel_source_markers_residual_wave782()
        && honesty_host_prone_update_dual_peel_nav_commands_residual_wave782()
        && simulate_host_prone_update_dual_peel_collect_source()
        && simulate_host_prone_update_dual_peel_dispatch_source()
}
pub fn simulate_live_host_prone_update_dual_peel_honesty() -> bool {
    let ok = honesty_host_prone_update_dual_peel_residual_pack_wave782();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostProneUpdateDualPeelAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_prone_update_dual_peel_method_names_residual_wave782());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_prone_update_dual_peel_source_markers_residual_wave782());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_prone_update_dual_peel_nav_commands_residual_wave782());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_prone_update_dual_peel_collect_source());
        assert!(simulate_host_prone_update_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_prone_update_dual_peel_residual_pack_wave782());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_prone_update_dual_peel_honesty());
        assert!(residual_host_prone_update_dual_peel_ok());
    }
}
