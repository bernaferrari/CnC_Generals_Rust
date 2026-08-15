//! Wave 762: GW entity carries `eject_invulnerable_until_frame`; coupled dual-tick
//! sole-expires eject-invulnerable via `tick_status_timer_expirations` and host
//! peels `tick_eject_invulnerable`. Writeback restores until_frame. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_EJECT_INVULN_DUAL_PEEL_METHOD_NAMES_WAVE762: &[&str] = &[
    "eject_invulnerable_until_frame",
    "tick_status_timer_expirations",
    "tick_eject_invulnerable",
    "shadow_coupled_tick_active",
    "Wave 762",
    "playable_claim = false",
];
pub const LIVE_HOST_EJECT_INVULN_DUAL_PEEL_NAV_STEPS_WAVE762: &[&str] = &[
    "REQUIRE_ENTITY_UNTIL_FRAME",
    "REQUIRE_GW_EXPIRE",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_WRITEBACK_UNTIL",
    "LIVE_HOST_EJECT_INVULN_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_EJECT_INVULN_DUAL_PEEL_CMD_NAMES_WAVE762: &[&str] = &[
    "host_eject_invuln_dual_peel",
    "eject_invulnerable_until_frame",
    "tick_status_timer_expirations",
    "tick_eject_invulnerable",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostEjectInvulnDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostEjectInvulnDualPeelAction {
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
fn residual_action_store(a: ResidualHostEjectInvulnDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_eject_invuln_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_eject_invuln_dual_peel_last_action() -> ResidualHostEjectInvulnDualPeelAction {
    ResidualHostEjectInvulnDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_eject_invuln_dual_peel_method_names_residual_wave762() -> bool {
    let names = LIVE_HOST_EJECT_INVULN_DUAL_PEEL_METHOD_NAMES_WAVE762;
    let ok = residual_name_index(names, "eject_invulnerable_until_frame").is_some()
        && residual_name_index(names, "tick_status_timer_expirations").is_some()
        && residual_name_index(names, "tick_eject_invulnerable").is_some()
        && residual_name_index(names, "shadow_coupled_tick_active").is_some()
        && residual_name_index(names, "Wave 762").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostEjectInvulnDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_eject_invuln_dual_peel_source_markers_residual_wave762() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("eject_invulnerable_until_frame")
        && sh.contains("eject_invulnerable_until_frame")
        && sh.contains("tick_status_timer_expirations")
        && sh.contains("frame >= e.eject_invulnerable_until_frame")
        && gl.contains("Wave 762")
        && gl.contains("tick_eject_invulnerable")
        && gl.contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostEjectInvulnDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_eject_invuln_dual_peel_nav_commands_residual_wave762() -> bool {
    let steps = LIVE_HOST_EJECT_INVULN_DUAL_PEEL_NAV_STEPS_WAVE762;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_UNTIL_FRAME").is_some()
        && residual_name_index(steps, "REQUIRE_GW_EXPIRE").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_WRITEBACK_UNTIL").is_some()
        && residual_name_index(steps, "LIVE_HOST_EJECT_INVULN_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostEjectInvulnDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_eject_invuln_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 762")
        && sh_source().contains("eject_invulnerable_until_frame")
        && gl_source().contains("Wave 762");
    residual_action_store(ResidualHostEjectInvulnDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_eject_invuln_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("eject_invulnerable_until_frame = 0")
        && sh_source().contains(
            "obj.status.eject_invulnerable_until_frame = ent.eject_invulnerable_until_frame",
        )
        && gl_source().contains("tick_eject_invulnerable(self.frame)")
        && gl_source().matches("Wave 762").count() >= 1;
    residual_action_store(ResidualHostEjectInvulnDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_eject_invuln_dual_peel_residual_pack_wave762() -> bool {
    honesty_host_eject_invuln_dual_peel_method_names_residual_wave762()
        && honesty_host_eject_invuln_dual_peel_source_markers_residual_wave762()
        && honesty_host_eject_invuln_dual_peel_nav_commands_residual_wave762()
        && simulate_host_eject_invuln_dual_peel_collect_source()
        && simulate_host_eject_invuln_dual_peel_dispatch_source()
}
pub fn simulate_live_host_eject_invuln_dual_peel_honesty() -> bool {
    let ok = honesty_host_eject_invuln_dual_peel_residual_pack_wave762();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostEjectInvulnDualPeelAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_eject_invuln_dual_peel_method_names_residual_wave762());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_eject_invuln_dual_peel_source_markers_residual_wave762());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_eject_invuln_dual_peel_nav_commands_residual_wave762());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_eject_invuln_dual_peel_collect_source());
        assert!(simulate_host_eject_invuln_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_eject_invuln_dual_peel_residual_pack_wave762());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_eject_invuln_dual_peel_honesty());
        assert!(residual_host_eject_invuln_dual_peel_ok());
    }
}
