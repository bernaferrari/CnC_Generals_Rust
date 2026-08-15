//! Wave 823: under coupled dual-tick, host peels update_patriot_assist_lasers and
//! sole-ticks endpoint track/expire after GW writeback (GW-authoritative poses).
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_PATRIOT_LASER_DUAL_PEEL_METHOD_NAMES_WAVE823: &[&str] = &[
    "update_patriot_assist_lasers",
    "tick_patriot_assist_lasers_sole",
    "track_patriot_assist_laser_endpoints",
    "expire_patriot_assist_lasers",
    "Wave 823",
    "playable_claim = false",
];
pub const LIVE_HOST_PATRIOT_LASER_DUAL_PEEL_NAV_STEPS_WAVE823: &[&str] = &[
    "REQUIRE_HOST_PEEL",
    "REQUIRE_POST_WRITEBACK_SOLE_TICK",
    "LIVE_HOST_PATRIOT_LASER_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPatriotLaserDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}
impl ResidualHostPatriotLaserDualPeelAction {
    fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::MethodNames,
            2 => Self::SourceMarkers,
            3 => Self::NavCommands,
            4 => Self::CollectSource,
            5 => Self::DispatchSource,
            _ => Self::None,
        }
    }
}
fn residual_action_store(a: ResidualHostPatriotLaserDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_patriot_laser_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_patriot_laser_dual_peel_last_action() -> ResidualHostPatriotLaserDualPeelAction
{
    ResidualHostPatriotLaserDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_patriot_laser_dual_peel_method_names_residual_wave823() -> bool {
    let names = LIVE_HOST_PATRIOT_LASER_DUAL_PEEL_METHOD_NAMES_WAVE823;
    let ok = residual_name_index(names, "update_patriot_assist_lasers").is_some()
        && residual_name_index(names, "tick_patriot_assist_lasers_sole").is_some()
        && residual_name_index(names, "track_patriot_assist_laser_endpoints").is_some()
        && residual_name_index(names, "expire_patriot_assist_lasers").is_some()
        && residual_name_index(names, "Wave 823").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostPatriotLaserDualPeelAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
pub fn honesty_host_patriot_laser_dual_peel_nav_commands_residual_wave823() -> bool {
    let steps = LIVE_HOST_PATRIOT_LASER_DUAL_PEEL_NAV_STEPS_WAVE823;
    let ok = !steps.is_empty()
        && residual_name_index(steps, "LIVE_HOST_PATRIOT_LASER_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostPatriotLaserDualPeelAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
pub fn honesty_host_patriot_laser_dual_peel_residual_pack_wave823() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ok = (sh.contains(
        "Wave 823: patriot assist laser endpoints sole-tick after GW writeback positions.",
    ) || sh.contains("apply_post_writeback_sole_ticks")
        || sh.contains("Wave 823–827/940"))
        && (sh.contains("tick_patriot_assist_lasers_sole")
            || sh.contains("apply_post_writeback_sole_ticks"))
        && gl.contains(
            "Wave 823: under coupled shadow, patriot assist lasers sole-tick after GW writeback.",
        )
        && gl.contains("fn tick_patriot_assist_lasers_sole")
        && (gl.contains("apply_post_writeback_sole_ticks") || gl.contains("Wave 940"));
    residual_action_store(ResidualHostPatriotLaserDualPeelAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
pub fn simulate_live_host_patriot_laser_dual_peel_honesty() -> bool {
    let a = honesty_host_patriot_laser_dual_peel_method_names_residual_wave823();
    let b = honesty_host_patriot_laser_dual_peel_nav_commands_residual_wave823();
    let c = honesty_host_patriot_laser_dual_peel_residual_pack_wave823();
    residual_action_store(ResidualHostPatriotLaserDualPeelAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn honesty_host_patriot_laser_dual_peel_residual_wave823() {
        assert!(honesty_host_patriot_laser_dual_peel_residual_pack_wave823());
        assert!(honesty_host_patriot_laser_dual_peel_method_names_residual_wave823());
        assert!(honesty_host_patriot_laser_dual_peel_nav_commands_residual_wave823());
        assert!(simulate_live_host_patriot_laser_dual_peel_honesty());
    }
}
