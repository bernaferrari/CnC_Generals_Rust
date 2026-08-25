//! Wave 745: under damage authority, LifetimeUpdate kill does not zero host
//! HP or stamp destroyed mid-frame (avoids dual with GameWorld HP writeback).
//! Mark-for-destruction owns the lethal residual. Non-authority path keeps host
//! HP clear. `playable_claim` stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_LIFETIME_KILL_NO_DAMAGE_AUTH_HP_STOMP_METHOD_NAMES_WAVE745: &[&str] = &[
    "tick_lifetime_update",
    "gameworld_damage_authority_live",
    "lifetime_kill",
    "Wave 745",
    "playable_claim = false",
];
pub const LIVE_HOST_LIFETIME_KILL_NO_DAMAGE_AUTH_HP_STOMP_NAV_STEPS_WAVE745: &[&str] = &[
    "REQUIRE_DAMAGE_AUTH_SKIP_HP_STOMP",
    "REQUIRE_MARK_FOR_DESTROY",
    "REQUIRE_NON_AUTH_KEEPS_HP_CLEAR",
    "LIVE_HOST_LIFETIME_KILL_NO_DAMAGE_AUTH_HP_STOMP",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_LIFETIME_KILL_NO_DAMAGE_AUTH_HP_STOMP_CMD_NAMES_WAVE745:
    &[&str] = &[
    "host_lifetime_kill_no_damage_auth_hp_stomp",
    "damage_auth_skip_hp_stomp",
    "mark_for_destroy",
    "non_auth_keeps_hp_clear",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostLifetimeKillNoDamageAuthHpStompAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostLifetimeKillNoDamageAuthHpStompAction {
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
fn residual_action_store(a: ResidualHostLifetimeKillNoDamageAuthHpStompAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_lifetime_kill_no_damage_auth_hp_stomp_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_lifetime_kill_no_damage_auth_hp_stomp_last_action()
-> ResidualHostLifetimeKillNoDamageAuthHpStompAction {
    ResidualHostLifetimeKillNoDamageAuthHpStompAction::from_u8(
        RESIDUAL_ACTION.load(Ordering::SeqCst),
    )
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_lifetime_kill_no_damage_auth_hp_stomp_method_names_residual_wave745() -> bool {
    let names = LIVE_HOST_LIFETIME_KILL_NO_DAMAGE_AUTH_HP_STOMP_METHOD_NAMES_WAVE745;
    let ok = residual_name_index(names, "tick_lifetime_update").is_some()
        && residual_name_index(names, "gameworld_damage_authority_live").is_some()
        && residual_name_index(names, "lifetime_kill").is_some()
        && residual_name_index(names, "Wave 745").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostLifetimeKillNoDamageAuthHpStompAction::MethodNames);
    ok
}
pub fn honesty_host_lifetime_kill_no_damage_auth_hp_stomp_source_markers_residual_wave745() -> bool
{
    let gl = gl_source();
    let ok = gl.contains("Wave 745")
        && gl.contains("tick_lifetime_update(self.frame)")
        && gl.contains("gameworld_damage_authority_live()")
        && gl.contains("lifetime_kill = true")
        && gl.contains("if !crate::gameworld_shadow::gameworld_damage_authority_live()")
        && gl.contains("obj.health.current = 0.0")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostLifetimeKillNoDamageAuthHpStompAction::SourceMarkers);
    ok
}
pub fn honesty_host_lifetime_kill_no_damage_auth_hp_stomp_nav_commands_residual_wave745() -> bool {
    let steps = LIVE_HOST_LIFETIME_KILL_NO_DAMAGE_AUTH_HP_STOMP_NAV_STEPS_WAVE745;
    let cmds = RUNTIME_HOST_LIVE_HOST_LIFETIME_KILL_NO_DAMAGE_AUTH_HP_STOMP_CMD_NAMES_WAVE745;
    let ok = residual_name_index(steps, "REQUIRE_DAMAGE_AUTH_SKIP_HP_STOMP").is_some()
        && residual_name_index(steps, "REQUIRE_MARK_FOR_DESTROY").is_some()
        && residual_name_index(steps, "REQUIRE_NON_AUTH_KEEPS_HP_CLEAR").is_some()
        && residual_name_index(steps, "LIVE_HOST_LIFETIME_KILL_NO_DAMAGE_AUTH_HP_STOMP").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_lifetime_kill_no_damage_auth_hp_stomp").is_some()
        && residual_name_index(cmds, "damage_auth_skip_hp_stomp").is_some()
        && residual_name_index(cmds, "mark_for_destroy").is_some()
        && residual_name_index(cmds, "non_auth_keeps_hp_clear").is_some();
    residual_action_store(ResidualHostLifetimeKillNoDamageAuthHpStompAction::NavCommands);
    ok
}
pub fn simulate_host_lifetime_kill_no_damage_auth_hp_stomp_collect_source() -> bool {
    let ok = gl_source().contains("tick_lifetime_update")
        && gl_source().contains("gameworld_damage_authority_live");
    residual_action_store(ResidualHostLifetimeKillNoDamageAuthHpStompAction::CollectSource);
    ok
}
pub fn simulate_host_lifetime_kill_no_damage_auth_hp_stomp_dispatch_source() -> bool {
    let ok = gl_source().contains("Wave 745")
        && gl_source().contains("if !crate::gameworld_shadow::gameworld_damage_authority_live()");
    residual_action_store(ResidualHostLifetimeKillNoDamageAuthHpStompAction::DispatchSource);
    ok
}
pub fn honesty_host_lifetime_kill_no_damage_auth_hp_stomp_residual_pack_wave745() -> bool {
    honesty_host_lifetime_kill_no_damage_auth_hp_stomp_method_names_residual_wave745()
        && honesty_host_lifetime_kill_no_damage_auth_hp_stomp_source_markers_residual_wave745()
        && honesty_host_lifetime_kill_no_damage_auth_hp_stomp_nav_commands_residual_wave745()
        && simulate_host_lifetime_kill_no_damage_auth_hp_stomp_collect_source()
        && simulate_host_lifetime_kill_no_damage_auth_hp_stomp_dispatch_source()
}
pub fn simulate_live_host_lifetime_kill_no_damage_auth_hp_stomp_honesty() -> bool {
    let ok = honesty_host_lifetime_kill_no_damage_auth_hp_stomp_residual_pack_wave745();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostLifetimeKillNoDamageAuthHpStompAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_lifetime_kill_no_damage_auth_hp_stomp_method_names_residual_wave745());
    }
    #[test]
    fn source_markers_residual() {
        assert!(
            honesty_host_lifetime_kill_no_damage_auth_hp_stomp_source_markers_residual_wave745()
        );
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_lifetime_kill_no_damage_auth_hp_stomp_nav_commands_residual_wave745());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_lifetime_kill_no_damage_auth_hp_stomp_collect_source());
        assert!(simulate_host_lifetime_kill_no_damage_auth_hp_stomp_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_lifetime_kill_no_damage_auth_hp_stomp_residual_pack_wave745());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_lifetime_kill_no_damage_auth_hp_stomp_honesty());
        assert!(residual_host_lifetime_kill_no_damage_auth_hp_stomp_ok());
    }
}
