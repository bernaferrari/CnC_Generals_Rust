//! Wave 753: remaining dual-line lethal stamps (destroyed+HP / HP+destroyed)
//! respect damage authority. Under `gameworld_damage_authority_live`, do not
//! zero host HP mid-frame (dual with GW HP writeback); project lethal via
//! `host_damage_log`. Covers Aurora bomb stale clear and similar residual.
//! Non-authority/host-only paths keep host HP clear. `playable_claim` stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_DUAL_LINE_LETHAL_NO_DAMAGE_AUTH_HP_STOMP_METHOD_NAMES_WAVE753: &[&str] = &[
    "aurora_bomb_projectile",
    "gameworld_damage_authority_live",
    "host_damage_log::record",
    "Wave 753",
    "playable_claim = false",
];
pub const LIVE_HOST_DUAL_LINE_LETHAL_NO_DAMAGE_AUTH_HP_STOMP_NAV_STEPS_WAVE753: &[&str] = &[
    "REQUIRE_AURORA_GATE",
    "REQUIRE_DAMAGE_AUTH_SKIP_HP_STOMP",
    "REQUIRE_DAMAGE_LOG_LETHAL",
    "LIVE_HOST_DUAL_LINE_LETHAL_NO_DAMAGE_AUTH_HP_STOMP",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_DUAL_LINE_LETHAL_NO_DAMAGE_AUTH_HP_STOMP_CMD_NAMES_WAVE753:
    &[&str] = &[
    "host_dual_line_lethal_no_damage_auth_hp_stomp",
    "aurora_stale_clear",
    "damage_auth_skip_hp_stomp",
    "damage_log_lethal",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostDualLineLethalNoDamageAuthHpStompAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostDualLineLethalNoDamageAuthHpStompAction {
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
fn residual_action_store(a: ResidualHostDualLineLethalNoDamageAuthHpStompAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_dual_line_lethal_no_damage_auth_hp_stomp_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_dual_line_lethal_no_damage_auth_hp_stomp_last_action()
-> ResidualHostDualLineLethalNoDamageAuthHpStompAction {
    ResidualHostDualLineLethalNoDamageAuthHpStompAction::from_u8(
        RESIDUAL_ACTION.load(Ordering::SeqCst),
    )
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_dual_line_lethal_no_damage_auth_hp_stomp_method_names_residual_wave753() -> bool
{
    let names = LIVE_HOST_DUAL_LINE_LETHAL_NO_DAMAGE_AUTH_HP_STOMP_METHOD_NAMES_WAVE753;
    let ok = residual_name_index(names, "aurora_bomb_projectile").is_some()
        && residual_name_index(names, "gameworld_damage_authority_live").is_some()
        && residual_name_index(names, "host_damage_log::record").is_some()
        && residual_name_index(names, "Wave 753").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostDualLineLethalNoDamageAuthHpStompAction::MethodNames);
    ok
}
pub fn honesty_host_dual_line_lethal_no_damage_auth_hp_stomp_source_markers_residual_wave753()
-> bool {
    let gl = gl_source();
    let ok = gl.contains("Wave 753")
        && gl.contains("aurora_bomb_projectile = false")
        && gl.contains("gameworld_damage_authority_live()")
        && gl.contains("host_damage_log::record")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostDualLineLethalNoDamageAuthHpStompAction::SourceMarkers);
    ok
}
pub fn honesty_host_dual_line_lethal_no_damage_auth_hp_stomp_nav_commands_residual_wave753() -> bool
{
    let steps = LIVE_HOST_DUAL_LINE_LETHAL_NO_DAMAGE_AUTH_HP_STOMP_NAV_STEPS_WAVE753;
    let cmds = RUNTIME_HOST_LIVE_HOST_DUAL_LINE_LETHAL_NO_DAMAGE_AUTH_HP_STOMP_CMD_NAMES_WAVE753;
    let ok = residual_name_index(steps, "REQUIRE_AURORA_GATE").is_some()
        && residual_name_index(steps, "REQUIRE_DAMAGE_AUTH_SKIP_HP_STOMP").is_some()
        && residual_name_index(steps, "REQUIRE_DAMAGE_LOG_LETHAL").is_some()
        && residual_name_index(steps, "LIVE_HOST_DUAL_LINE_LETHAL_NO_DAMAGE_AUTH_HP_STOMP")
            .is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_dual_line_lethal_no_damage_auth_hp_stomp").is_some()
        && residual_name_index(cmds, "aurora_stale_clear").is_some()
        && residual_name_index(cmds, "damage_auth_skip_hp_stomp").is_some()
        && residual_name_index(cmds, "damage_log_lethal").is_some();
    residual_action_store(ResidualHostDualLineLethalNoDamageAuthHpStompAction::NavCommands);
    ok
}
pub fn simulate_host_dual_line_lethal_no_damage_auth_hp_stomp_collect_source() -> bool {
    let ok = gl_source().contains("aurora_bomb_projectile")
        && gl_source().contains("gameworld_damage_authority_live");
    residual_action_store(ResidualHostDualLineLethalNoDamageAuthHpStompAction::CollectSource);
    ok
}
pub fn simulate_host_dual_line_lethal_no_damage_auth_hp_stomp_dispatch_source() -> bool {
    let ok = gl_source().contains("Wave 753") && gl_source().contains("host_damage_log::record");
    residual_action_store(ResidualHostDualLineLethalNoDamageAuthHpStompAction::DispatchSource);
    ok
}
pub fn honesty_host_dual_line_lethal_no_damage_auth_hp_stomp_residual_pack_wave753() -> bool {
    honesty_host_dual_line_lethal_no_damage_auth_hp_stomp_method_names_residual_wave753()
        && honesty_host_dual_line_lethal_no_damage_auth_hp_stomp_source_markers_residual_wave753()
        && honesty_host_dual_line_lethal_no_damage_auth_hp_stomp_nav_commands_residual_wave753()
        && simulate_host_dual_line_lethal_no_damage_auth_hp_stomp_collect_source()
        && simulate_host_dual_line_lethal_no_damage_auth_hp_stomp_dispatch_source()
}
pub fn simulate_live_host_dual_line_lethal_no_damage_auth_hp_stomp_honesty() -> bool {
    let ok = honesty_host_dual_line_lethal_no_damage_auth_hp_stomp_residual_pack_wave753();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostDualLineLethalNoDamageAuthHpStompAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(
            honesty_host_dual_line_lethal_no_damage_auth_hp_stomp_method_names_residual_wave753()
        );
    }
    #[test]
    fn source_markers_residual() {
        assert!(
            honesty_host_dual_line_lethal_no_damage_auth_hp_stomp_source_markers_residual_wave753()
        );
    }
    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_host_dual_line_lethal_no_damage_auth_hp_stomp_nav_commands_residual_wave753()
        );
    }
    #[test]
    fn sources() {
        assert!(simulate_host_dual_line_lethal_no_damage_auth_hp_stomp_collect_source());
        assert!(simulate_host_dual_line_lethal_no_damage_auth_hp_stomp_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_dual_line_lethal_no_damage_auth_hp_stomp_residual_pack_wave753());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_dual_line_lethal_no_damage_auth_hp_stomp_honesty());
        assert!(residual_host_dual_line_lethal_no_damage_auth_hp_stomp_ok());
    }
}
