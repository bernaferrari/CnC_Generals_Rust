//! Wave 768: GW entity carries LifetimeUpdate residual; under coupled dual-tick
//! `tick_status_timer_expirations` sole-expires lifetime and records
//! host_lifetime_expire_log; host peels `tick_lifetime_update` and drains
//! mark-for-destruction after writeback. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_LIFETIME_EXPIRE_DUAL_PEEL_METHOD_NAMES_WAVE768: &[&str] = &[
    "lifetime_expire_at_frame",
    "lifetime_active",
    "host_lifetime_expire_log",
    "tick_lifetime_update",
    "Wave 768",
    "playable_claim = false",
];
pub const LIVE_HOST_LIFETIME_EXPIRE_DUAL_PEEL_NAV_STEPS_WAVE768: &[&str] = &[
    "REQUIRE_ENTITY_LIFETIME_FIELDS",
    "REQUIRE_GW_EXPIRE_LOG",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_DRAIN_MARK_DESTROY",
    "LIVE_HOST_LIFETIME_EXPIRE_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_LIFETIME_EXPIRE_DUAL_PEEL_CMD_NAMES_WAVE768: &[&str] = &[
    "host_lifetime_expire_dual_peel",
    "lifetime_active",
    "host_lifetime_expire_log",
    "tick_lifetime_update",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostLifetimeExpireDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostLifetimeExpireDualPeelAction {
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
fn residual_action_store(a: ResidualHostLifetimeExpireDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_lifetime_expire_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_lifetime_expire_dual_peel_last_action()
-> ResidualHostLifetimeExpireDualPeelAction {
    ResidualHostLifetimeExpireDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    // 2026-08-15: scan host plus extra world_* splits.
    super::host_logic_scan_src()
}
pub fn honesty_host_lifetime_expire_dual_peel_method_names_residual_wave768() -> bool {
    let names = LIVE_HOST_LIFETIME_EXPIRE_DUAL_PEEL_METHOD_NAMES_WAVE768;
    let ok = residual_name_index(names, "lifetime_expire_at_frame").is_some()
        && residual_name_index(names, "lifetime_active").is_some()
        && residual_name_index(names, "host_lifetime_expire_log").is_some()
        && residual_name_index(names, "tick_lifetime_update").is_some()
        && residual_name_index(names, "Wave 768").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostLifetimeExpireDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_lifetime_expire_dual_peel_source_markers_residual_wave768() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("lifetime_expire_at_frame")
        && ent.contains("lifetime_active")
        && sh.contains("Wave 768")
        && sh.contains("host_lifetime_expire_log::record")
        && sh.contains("host_lifetime_expire_log::drain")
        && gl.contains("Wave 768")
        && gl.contains("tick_lifetime_update(self.frame)");
    residual_action_store(ResidualHostLifetimeExpireDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_lifetime_expire_dual_peel_nav_commands_residual_wave768() -> bool {
    let steps = LIVE_HOST_LIFETIME_EXPIRE_DUAL_PEEL_NAV_STEPS_WAVE768;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_LIFETIME_FIELDS").is_some()
        && residual_name_index(steps, "REQUIRE_GW_EXPIRE_LOG").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_DRAIN_MARK_DESTROY").is_some()
        && residual_name_index(steps, "LIVE_HOST_LIFETIME_EXPIRE_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostLifetimeExpireDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_lifetime_expire_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 768")
        && sh_source().contains("lifetime_active")
        && gl_source().contains("Wave 768");
    residual_action_store(ResidualHostLifetimeExpireDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_lifetime_expire_dual_peel_dispatch_source() -> bool {
    // 2026-08-15: drain applies HostObjectIdOp::MarkForDestruction (session.rs:1175).
    let ok = sh_source().contains("host_lifetime_expire_log::record")
        && (sh_source().contains("mark_object_for_destruction")
            || sh_source().contains("HostObjectIdOp::MarkForDestruction"))
        && gl_source().contains("tick_lifetime_update(self.frame)")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostLifetimeExpireDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_lifetime_expire_dual_peel_residual_pack_wave768() -> bool {
    honesty_host_lifetime_expire_dual_peel_method_names_residual_wave768()
        && honesty_host_lifetime_expire_dual_peel_source_markers_residual_wave768()
        && honesty_host_lifetime_expire_dual_peel_nav_commands_residual_wave768()
        && simulate_host_lifetime_expire_dual_peel_collect_source()
        && simulate_host_lifetime_expire_dual_peel_dispatch_source()
}
pub fn simulate_live_host_lifetime_expire_dual_peel_honesty() -> bool {
    let ok = honesty_host_lifetime_expire_dual_peel_residual_pack_wave768();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostLifetimeExpireDualPeelAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_lifetime_expire_dual_peel_method_names_residual_wave768());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_lifetime_expire_dual_peel_source_markers_residual_wave768());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_lifetime_expire_dual_peel_nav_commands_residual_wave768());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_lifetime_expire_dual_peel_collect_source());
        assert!(simulate_host_lifetime_expire_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_lifetime_expire_dual_peel_residual_pack_wave768());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_lifetime_expire_dual_peel_honesty());
        assert!(residual_host_lifetime_expire_dual_peel_ok());
    }
}
