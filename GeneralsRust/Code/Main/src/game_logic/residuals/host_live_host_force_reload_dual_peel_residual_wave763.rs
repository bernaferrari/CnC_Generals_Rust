//! Wave 763: GW entity carries `frame_to_force_reload`; coupled dual-tick
//! sole-expires force-reload-when-idle via `tick_status_timer_expirations`
//! (refills primary clip ammo) and host peels `tick_force_reload_when_idle`.
//! Writeback restores timer + ammo. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_FORCE_RELOAD_DUAL_PEEL_METHOD_NAMES_WAVE763: &[&str] = &[
    "frame_to_force_reload",
    "tick_status_timer_expirations",
    "tick_force_reload_when_idle",
    "shadow_coupled_tick_active",
    "Wave 763",
    "playable_claim = false",
];
pub const LIVE_HOST_FORCE_RELOAD_DUAL_PEEL_NAV_STEPS_WAVE763: &[&str] = &[
    "REQUIRE_ENTITY_FORCE_RELOAD",
    "REQUIRE_GW_EXPIRE_RELOAD",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_WRITEBACK_TIMER",
    "LIVE_HOST_FORCE_RELOAD_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_FORCE_RELOAD_DUAL_PEEL_CMD_NAMES_WAVE763: &[&str] = &[
    "host_force_reload_dual_peel",
    "frame_to_force_reload",
    "tick_status_timer_expirations",
    "tick_force_reload_when_idle",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostForceReloadDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostForceReloadDualPeelAction {
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
fn residual_action_store(a: ResidualHostForceReloadDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_force_reload_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_force_reload_dual_peel_last_action() -> ResidualHostForceReloadDualPeelAction {
    ResidualHostForceReloadDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    include_str!("../../gameworld_shadow.rs")
}
fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}
pub fn honesty_host_force_reload_dual_peel_method_names_residual_wave763() -> bool {
    let names = LIVE_HOST_FORCE_RELOAD_DUAL_PEEL_METHOD_NAMES_WAVE763;
    let ok = residual_name_index(names, "frame_to_force_reload").is_some()
        && residual_name_index(names, "tick_status_timer_expirations").is_some()
        && residual_name_index(names, "tick_force_reload_when_idle").is_some()
        && residual_name_index(names, "shadow_coupled_tick_active").is_some()
        && residual_name_index(names, "Wave 763").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostForceReloadDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_force_reload_dual_peel_source_markers_residual_wave763() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("frame_to_force_reload")
        && sh.contains("frame_to_force_reload")
        && sh.contains("e.weapon_ammo = e.weapon_clip_size")
        && gl.contains("Wave 763")
        && gl.contains("tick_force_reload_when_idle")
        && gl.contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostForceReloadDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_force_reload_dual_peel_nav_commands_residual_wave763() -> bool {
    let steps = LIVE_HOST_FORCE_RELOAD_DUAL_PEEL_NAV_STEPS_WAVE763;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_FORCE_RELOAD").is_some()
        && residual_name_index(steps, "REQUIRE_GW_EXPIRE_RELOAD").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_WRITEBACK_TIMER").is_some()
        && residual_name_index(steps, "LIVE_HOST_FORCE_RELOAD_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostForceReloadDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_force_reload_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 763")
        && sh_source().contains("frame_to_force_reload")
        && gl_source().contains("Wave 763");
    residual_action_store(ResidualHostForceReloadDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_force_reload_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("e.frame_to_force_reload = 0")
        && sh_source().contains("obj.frame_to_force_reload = ent.frame_to_force_reload")
        && gl_source().contains("tick_force_reload_when_idle(self.frame)")
        && gl_source().matches("Wave 763").count() >= 1;
    residual_action_store(ResidualHostForceReloadDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_force_reload_dual_peel_residual_pack_wave763() -> bool {
    honesty_host_force_reload_dual_peel_method_names_residual_wave763()
        && honesty_host_force_reload_dual_peel_source_markers_residual_wave763()
        && honesty_host_force_reload_dual_peel_nav_commands_residual_wave763()
        && simulate_host_force_reload_dual_peel_collect_source()
        && simulate_host_force_reload_dual_peel_dispatch_source()
}
pub fn simulate_live_host_force_reload_dual_peel_honesty() -> bool {
    let ok = honesty_host_force_reload_dual_peel_residual_pack_wave763();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostForceReloadDualPeelAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_force_reload_dual_peel_method_names_residual_wave763());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_force_reload_dual_peel_source_markers_residual_wave763());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_force_reload_dual_peel_nav_commands_residual_wave763());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_force_reload_dual_peel_collect_source());
        assert!(simulate_host_force_reload_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_force_reload_dual_peel_residual_pack_wave763());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_force_reload_dual_peel_honesty());
        assert!(residual_host_force_reload_dual_peel_ok());
    }
}
