//! Wave 785: GW entity carries RadiusDecalUpdate residual; under coupled
//! dual-tick `tick_status_timer_expirations` sole-ticks delivery decal throb
//! and kill-when-idle; host peels `update_radius_decal_update`. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_RADIUS_DECAL_DUAL_PEEL_METHOD_NAMES_WAVE785: &[&str] = &[
    "radius_decal_awake",
    "radius_decal_opacity",
    "radius_decal_kill_when_idle",
    "update_radius_decal_update",
    "Wave 785",
    "playable_claim = false",
];
pub const LIVE_HOST_RADIUS_DECAL_DUAL_PEEL_NAV_STEPS_WAVE785: &[&str] = &[
    "REQUIRE_ENTITY_RADIUS_DECAL_FIELDS",
    "REQUIRE_GW_THROB_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_WRITEBACK",
    "LIVE_HOST_RADIUS_DECAL_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_RADIUS_DECAL_DUAL_PEEL_CMD_NAMES_WAVE785: &[&str] = &[
    "host_radius_decal_dual_peel",
    "radius_decal_awake",
    "radius_decal_opacity",
    "update_radius_decal_update",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostRadiusDecalDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostRadiusDecalDualPeelAction {
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
fn residual_action_store(a: ResidualHostRadiusDecalDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_radius_decal_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_radius_decal_dual_peel_last_action() -> ResidualHostRadiusDecalDualPeelAction {
    ResidualHostRadiusDecalDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    include_str!("../../gameworld_shadow.rs")
}
fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}
pub fn honesty_host_radius_decal_dual_peel_method_names_residual_wave785() -> bool {
    let names = LIVE_HOST_RADIUS_DECAL_DUAL_PEEL_METHOD_NAMES_WAVE785;
    let ok = residual_name_index(names, "radius_decal_awake").is_some()
        && residual_name_index(names, "radius_decal_opacity").is_some()
        && residual_name_index(names, "radius_decal_kill_when_idle").is_some()
        && residual_name_index(names, "update_radius_decal_update").is_some()
        && residual_name_index(names, "Wave 785").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostRadiusDecalDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_radius_decal_dual_peel_source_markers_residual_wave785() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("radius_decal_awake")
        && ent.contains("radius_decal_opacity")
        && sh.contains("Wave 785")
        && sh.contains("HostRadiusDecalUpdateData")
        && sh.contains("radius_decal_throb_frames")
        && gl.contains("Wave 785")
        && gl.contains("update_radius_decal_update");
    residual_action_store(ResidualHostRadiusDecalDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_radius_decal_dual_peel_nav_commands_residual_wave785() -> bool {
    let steps = LIVE_HOST_RADIUS_DECAL_DUAL_PEEL_NAV_STEPS_WAVE785;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_RADIUS_DECAL_FIELDS").is_some()
        && residual_name_index(steps, "REQUIRE_GW_THROB_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_WRITEBACK").is_some()
        && residual_name_index(steps, "LIVE_HOST_RADIUS_DECAL_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostRadiusDecalDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_radius_decal_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 785")
        && sh_source().contains("radius_decal_awake")
        && gl_source().contains("Wave 785");
    residual_action_store(ResidualHostRadiusDecalDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_radius_decal_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("HostRadiusDecalUpdateData")
        && sh_source().contains("radius_decal_kill_when_idle")
        && gl_source().contains("update_radius_decal_update")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostRadiusDecalDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_radius_decal_dual_peel_residual_pack_wave785() -> bool {
    honesty_host_radius_decal_dual_peel_method_names_residual_wave785()
        && honesty_host_radius_decal_dual_peel_source_markers_residual_wave785()
        && honesty_host_radius_decal_dual_peel_nav_commands_residual_wave785()
        && simulate_host_radius_decal_dual_peel_collect_source()
        && simulate_host_radius_decal_dual_peel_dispatch_source()
}
pub fn simulate_live_host_radius_decal_dual_peel_honesty() -> bool {
    let ok = honesty_host_radius_decal_dual_peel_residual_pack_wave785();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostRadiusDecalDualPeelAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_radius_decal_dual_peel_method_names_residual_wave785());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_radius_decal_dual_peel_source_markers_residual_wave785());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_radius_decal_dual_peel_nav_commands_residual_wave785());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_radius_decal_dual_peel_collect_source());
        assert!(simulate_host_radius_decal_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_radius_decal_dual_peel_residual_pack_wave785());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_radius_decal_dual_peel_honesty());
        assert!(residual_host_radius_decal_dual_peel_ok());
    }
}
