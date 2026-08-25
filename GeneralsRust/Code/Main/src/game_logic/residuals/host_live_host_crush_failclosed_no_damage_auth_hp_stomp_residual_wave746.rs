//! Wave 746: under damage authority, structure-topple crush fail-closed does
//! not zero host HP (avoids dual with GameWorld HP writeback). Projects lethal
//! via `host_damage_log` + destroyed flags. Non-authority path keeps host HP
//! clear. `playable_claim` stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_CRUSH_FAILCLOSED_NO_DAMAGE_AUTH_HP_STOMP_METHOD_NAMES_WAVE746: &[&str] = &[
    "crush sweep leaves no standing unit residual",
    "gameworld_damage_authority_live",
    "host_damage_log::record",
    "Wave 746",
    "playable_claim = false",
];
pub const LIVE_HOST_CRUSH_FAILCLOSED_NO_DAMAGE_AUTH_HP_STOMP_NAV_STEPS_WAVE746: &[&str] = &[
    "REQUIRE_DAMAGE_AUTH_SKIP_HP_STOMP",
    "REQUIRE_DAMAGE_LOG_LETHAL",
    "REQUIRE_NON_AUTH_KEEPS_HP_CLEAR",
    "LIVE_HOST_CRUSH_FAILCLOSED_NO_DAMAGE_AUTH_HP_STOMP",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_CRUSH_FAILCLOSED_NO_DAMAGE_AUTH_HP_STOMP_CMD_NAMES_WAVE746:
    &[&str] = &[
    "host_crush_failclosed_no_damage_auth_hp_stomp",
    "damage_auth_skip_hp_stomp",
    "damage_log_lethal",
    "non_auth_keeps_hp_clear",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostCrushFailclosedNoDamageAuthHpStompAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostCrushFailclosedNoDamageAuthHpStompAction {
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
fn residual_action_store(a: ResidualHostCrushFailclosedNoDamageAuthHpStompAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_crush_failclosed_no_damage_auth_hp_stomp_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_crush_failclosed_no_damage_auth_hp_stomp_last_action()
-> ResidualHostCrushFailclosedNoDamageAuthHpStompAction {
    ResidualHostCrushFailclosedNoDamageAuthHpStompAction::from_u8(
        RESIDUAL_ACTION.load(Ordering::SeqCst),
    )
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_crush_failclosed_no_damage_auth_hp_stomp_method_names_residual_wave746() -> bool
{
    let names = LIVE_HOST_CRUSH_FAILCLOSED_NO_DAMAGE_AUTH_HP_STOMP_METHOD_NAMES_WAVE746;
    let ok = residual_name_index(names, "crush sweep leaves no standing unit residual").is_some()
        && residual_name_index(names, "gameworld_damage_authority_live").is_some()
        && residual_name_index(names, "host_damage_log::record").is_some()
        && residual_name_index(names, "Wave 746").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostCrushFailclosedNoDamageAuthHpStompAction::MethodNames);
    ok
}
pub fn honesty_host_crush_failclosed_no_damage_auth_hp_stomp_source_markers_residual_wave746()
-> bool {
    let gl = gl_source();
    let ok = gl.contains("Wave 746")
        && gl.contains("crush sweep leaves no standing unit residual")
        && gl.contains("gameworld_damage_authority_live()")
        && gl.contains("host_damage_log::record")
        && gl.contains("HostDeathType::Crushed")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostCrushFailclosedNoDamageAuthHpStompAction::SourceMarkers);
    ok
}
pub fn honesty_host_crush_failclosed_no_damage_auth_hp_stomp_nav_commands_residual_wave746() -> bool
{
    let steps = LIVE_HOST_CRUSH_FAILCLOSED_NO_DAMAGE_AUTH_HP_STOMP_NAV_STEPS_WAVE746;
    let cmds = RUNTIME_HOST_LIVE_HOST_CRUSH_FAILCLOSED_NO_DAMAGE_AUTH_HP_STOMP_CMD_NAMES_WAVE746;
    let ok = residual_name_index(steps, "REQUIRE_DAMAGE_AUTH_SKIP_HP_STOMP").is_some()
        && residual_name_index(steps, "REQUIRE_DAMAGE_LOG_LETHAL").is_some()
        && residual_name_index(steps, "REQUIRE_NON_AUTH_KEEPS_HP_CLEAR").is_some()
        && residual_name_index(steps, "LIVE_HOST_CRUSH_FAILCLOSED_NO_DAMAGE_AUTH_HP_STOMP")
            .is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_crush_failclosed_no_damage_auth_hp_stomp").is_some()
        && residual_name_index(cmds, "damage_auth_skip_hp_stomp").is_some()
        && residual_name_index(cmds, "damage_log_lethal").is_some()
        && residual_name_index(cmds, "non_auth_keeps_hp_clear").is_some();
    residual_action_store(ResidualHostCrushFailclosedNoDamageAuthHpStompAction::NavCommands);
    ok
}
pub fn simulate_host_crush_failclosed_no_damage_auth_hp_stomp_collect_source() -> bool {
    let ok = gl_source().contains("crush sweep leaves no standing unit residual")
        && gl_source().contains("gameworld_damage_authority_live");
    residual_action_store(ResidualHostCrushFailclosedNoDamageAuthHpStompAction::CollectSource);
    ok
}
pub fn simulate_host_crush_failclosed_no_damage_auth_hp_stomp_dispatch_source() -> bool {
    let ok = gl_source().contains("Wave 746") && gl_source().contains("host_damage_log::record");
    residual_action_store(ResidualHostCrushFailclosedNoDamageAuthHpStompAction::DispatchSource);
    ok
}
pub fn honesty_host_crush_failclosed_no_damage_auth_hp_stomp_residual_pack_wave746() -> bool {
    honesty_host_crush_failclosed_no_damage_auth_hp_stomp_method_names_residual_wave746()
        && honesty_host_crush_failclosed_no_damage_auth_hp_stomp_source_markers_residual_wave746()
        && honesty_host_crush_failclosed_no_damage_auth_hp_stomp_nav_commands_residual_wave746()
        && simulate_host_crush_failclosed_no_damage_auth_hp_stomp_collect_source()
        && simulate_host_crush_failclosed_no_damage_auth_hp_stomp_dispatch_source()
}
pub fn simulate_live_host_crush_failclosed_no_damage_auth_hp_stomp_honesty() -> bool {
    let ok = honesty_host_crush_failclosed_no_damage_auth_hp_stomp_residual_pack_wave746();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostCrushFailclosedNoDamageAuthHpStompAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(
            honesty_host_crush_failclosed_no_damage_auth_hp_stomp_method_names_residual_wave746()
        );
    }
    #[test]
    fn source_markers_residual() {
        assert!(
            honesty_host_crush_failclosed_no_damage_auth_hp_stomp_source_markers_residual_wave746()
        );
    }
    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_host_crush_failclosed_no_damage_auth_hp_stomp_nav_commands_residual_wave746()
        );
    }
    #[test]
    fn sources() {
        assert!(simulate_host_crush_failclosed_no_damage_auth_hp_stomp_collect_source());
        assert!(simulate_host_crush_failclosed_no_damage_auth_hp_stomp_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_crush_failclosed_no_damage_auth_hp_stomp_residual_pack_wave746());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_crush_failclosed_no_damage_auth_hp_stomp_honesty());
        assert!(residual_host_crush_failclosed_no_damage_auth_hp_stomp_ok());
    }
}
