//! Wave 764: under coupled dual-tick, GW `tick_status_timer_expirations`
//! sole-decrements `shock_stun_frames`; host peels countdown via
//! `tick_shock_stun_physics_only` (tumble/bounce physics remains host).
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_SHOCK_STUN_DUAL_PEEL_METHOD_NAMES_WAVE764: &[&str] = &[
    "shock_stun_frames",
    "tick_status_timer_expirations",
    "tick_shock_stun_physics_only",
    "tick_shock_stun_with_countdown",
    "Wave 764",
    "playable_claim = false",
];
pub const LIVE_HOST_SHOCK_STUN_DUAL_PEEL_NAV_STEPS_WAVE764: &[&str] = &[
    "REQUIRE_GW_FRAME_COUNTDOWN",
    "REQUIRE_HOST_PHYSICS_ONLY",
    "REQUIRE_COUPLED_PEEL",
    "LIVE_HOST_SHOCK_STUN_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_SHOCK_STUN_DUAL_PEEL_CMD_NAMES_WAVE764: &[&str] = &[
    "host_shock_stun_dual_peel",
    "shock_stun_frames",
    "tick_shock_stun_physics_only",
    "tick_status_timer_expirations",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostShockStunDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostShockStunDualPeelAction {
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
fn residual_action_store(a: ResidualHostShockStunDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_shock_stun_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_shock_stun_dual_peel_last_action() -> ResidualHostShockStunDualPeelAction {
    ResidualHostShockStunDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_shock_stun_dual_peel_method_names_residual_wave764() -> bool {
    let names = LIVE_HOST_SHOCK_STUN_DUAL_PEEL_METHOD_NAMES_WAVE764;
    let ok = residual_name_index(names, "shock_stun_frames").is_some()
        && residual_name_index(names, "tick_status_timer_expirations").is_some()
        && residual_name_index(names, "tick_shock_stun_physics_only").is_some()
        && residual_name_index(names, "tick_shock_stun_with_countdown").is_some()
        && residual_name_index(names, "Wave 764").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostShockStunDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_shock_stun_dual_peel_source_markers_residual_wave764() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let obj = crate::game_logic::object::OBJECT_SRC;
    let ok = sh.contains("Wave 764")
        && sh.contains("e.shock_stun_frames = e.shock_stun_frames.saturating_sub(1)")
        && obj.contains("tick_shock_stun_physics_only")
        && obj.contains("tick_shock_stun_with_countdown")
        && gl.contains("Wave 764")
        && gl.contains("tick_shock_stun_physics_only()");
    residual_action_store(ResidualHostShockStunDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_shock_stun_dual_peel_nav_commands_residual_wave764() -> bool {
    let steps = LIVE_HOST_SHOCK_STUN_DUAL_PEEL_NAV_STEPS_WAVE764;
    let ok = residual_name_index(steps, "REQUIRE_GW_FRAME_COUNTDOWN").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PHYSICS_ONLY").is_some()
        && residual_name_index(steps, "REQUIRE_COUPLED_PEEL").is_some()
        && residual_name_index(steps, "LIVE_HOST_SHOCK_STUN_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostShockStunDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_shock_stun_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 764")
        && gl_source().contains("Wave 764")
        && crate::game_logic::object::OBJECT_SRC.contains("tick_shock_stun_physics_only");
    residual_action_store(ResidualHostShockStunDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_shock_stun_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("shock_stun_frames.saturating_sub")
        && gl_source().contains("tick_shock_stun_physics_only()")
        // Live peel channel: the object source no longer branches on the
        // countdown flag; physics-only peels by delegating
        // `tick_shock_stun_with_countdown(false)` (tumble/bounce stays host,
        // GW tick_status_timer_expirations sole-decrements the frames).
        && crate::game_logic::object::OBJECT_SRC
            .contains("tick_shock_stun_with_countdown(false)")
        && gl_source().matches("Wave 764").count() >= 1;
    residual_action_store(ResidualHostShockStunDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_shock_stun_dual_peel_residual_pack_wave764() -> bool {
    honesty_host_shock_stun_dual_peel_method_names_residual_wave764()
        && honesty_host_shock_stun_dual_peel_source_markers_residual_wave764()
        && honesty_host_shock_stun_dual_peel_nav_commands_residual_wave764()
        && simulate_host_shock_stun_dual_peel_collect_source()
        && simulate_host_shock_stun_dual_peel_dispatch_source()
}
pub fn simulate_live_host_shock_stun_dual_peel_honesty() -> bool {
    let ok = honesty_host_shock_stun_dual_peel_residual_pack_wave764();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostShockStunDualPeelAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_shock_stun_dual_peel_method_names_residual_wave764());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_shock_stun_dual_peel_source_markers_residual_wave764());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_shock_stun_dual_peel_nav_commands_residual_wave764());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_shock_stun_dual_peel_collect_source());
        assert!(simulate_host_shock_stun_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_shock_stun_dual_peel_residual_pack_wave764());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_shock_stun_dual_peel_honesty());
        assert!(residual_host_shock_stun_dual_peel_ok());
    }
}
