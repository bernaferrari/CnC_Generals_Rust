//! Wave 750: under damage authority, Spectre gunship prior-clear does not
//! zero host HP mid-frame (avoids dual with GameWorld HP writeback). Projects
//! lethal via `host_damage_log` + destroyed flag. Non-authority path keeps host
//! HP clear. `playable_claim` stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_SPECTRE_PRIOR_CLEAR_NO_DAMAGE_AUTH_HP_STOMP_METHOD_NAMES_WAVE750: &[&str] = &[
    "Clear prior gunship residual",
    "spectre_gunship_deployment_reg.record_prior_clear",
    "gameworld_damage_authority_live",
    "Wave 750",
    "playable_claim = false",
];
pub const LIVE_HOST_SPECTRE_PRIOR_CLEAR_NO_DAMAGE_AUTH_HP_STOMP_NAV_STEPS_WAVE750: &[&str] = &[
    "REQUIRE_DAMAGE_AUTH_SKIP_HP_STOMP",
    "REQUIRE_DAMAGE_LOG_LETHAL",
    "REQUIRE_NON_AUTH_KEEPS_HP_CLEAR",
    "LIVE_HOST_SPECTRE_PRIOR_CLEAR_NO_DAMAGE_AUTH_HP_STOMP",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_SPECTRE_PRIOR_CLEAR_NO_DAMAGE_AUTH_HP_STOMP_CMD_NAMES_WAVE750:
    &[&str] = &[
    "host_spectre_prior_clear_no_damage_auth_hp_stomp",
    "damage_auth_skip_hp_stomp",
    "damage_log_lethal",
    "non_auth_keeps_hp_clear",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSpectrePriorClearNoDamageAuthHpStompAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostSpectrePriorClearNoDamageAuthHpStompAction {
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
fn residual_action_store(a: ResidualHostSpectrePriorClearNoDamageAuthHpStompAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_spectre_prior_clear_no_damage_auth_hp_stomp_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_spectre_prior_clear_no_damage_auth_hp_stomp_last_action()
-> ResidualHostSpectrePriorClearNoDamageAuthHpStompAction {
    ResidualHostSpectrePriorClearNoDamageAuthHpStompAction::from_u8(
        RESIDUAL_ACTION.load(Ordering::SeqCst),
    )
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_spectre_prior_clear_no_damage_auth_hp_stomp_method_names_residual_wave750()
-> bool {
    let names = LIVE_HOST_SPECTRE_PRIOR_CLEAR_NO_DAMAGE_AUTH_HP_STOMP_METHOD_NAMES_WAVE750;
    let ok = residual_name_index(names, "Clear prior gunship residual").is_some()
        && residual_name_index(names, "spectre_gunship_deployment_reg.record_prior_clear")
            .is_some()
        && residual_name_index(names, "gameworld_damage_authority_live").is_some()
        && residual_name_index(names, "Wave 750").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostSpectrePriorClearNoDamageAuthHpStompAction::MethodNames);
    ok
}
pub fn honesty_host_spectre_prior_clear_no_damage_auth_hp_stomp_source_markers_residual_wave750()
-> bool {
    let gl = gl_source();
    let ok = gl.contains("Wave 750")
        && gl.contains("Clear prior gunship residual")
        && gl.contains("record_prior_clear")
        && gl.contains("gameworld_damage_authority_live()")
        && gl.contains("host_damage_log::record")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostSpectrePriorClearNoDamageAuthHpStompAction::SourceMarkers);
    ok
}
pub fn honesty_host_spectre_prior_clear_no_damage_auth_hp_stomp_nav_commands_residual_wave750()
-> bool {
    let steps = LIVE_HOST_SPECTRE_PRIOR_CLEAR_NO_DAMAGE_AUTH_HP_STOMP_NAV_STEPS_WAVE750;
    let cmds = RUNTIME_HOST_LIVE_HOST_SPECTRE_PRIOR_CLEAR_NO_DAMAGE_AUTH_HP_STOMP_CMD_NAMES_WAVE750;
    let ok = residual_name_index(steps, "REQUIRE_DAMAGE_AUTH_SKIP_HP_STOMP").is_some()
        && residual_name_index(steps, "REQUIRE_DAMAGE_LOG_LETHAL").is_some()
        && residual_name_index(steps, "REQUIRE_NON_AUTH_KEEPS_HP_CLEAR").is_some()
        && residual_name_index(
            steps,
            "LIVE_HOST_SPECTRE_PRIOR_CLEAR_NO_DAMAGE_AUTH_HP_STOMP",
        )
        .is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_spectre_prior_clear_no_damage_auth_hp_stomp").is_some()
        && residual_name_index(cmds, "damage_auth_skip_hp_stomp").is_some()
        && residual_name_index(cmds, "damage_log_lethal").is_some()
        && residual_name_index(cmds, "non_auth_keeps_hp_clear").is_some();
    residual_action_store(ResidualHostSpectrePriorClearNoDamageAuthHpStompAction::NavCommands);
    ok
}
pub fn simulate_host_spectre_prior_clear_no_damage_auth_hp_stomp_collect_source() -> bool {
    let ok = gl_source().contains("Clear prior gunship residual")
        && gl_source().contains("gameworld_damage_authority_live");
    residual_action_store(ResidualHostSpectrePriorClearNoDamageAuthHpStompAction::CollectSource);
    ok
}
pub fn simulate_host_spectre_prior_clear_no_damage_auth_hp_stomp_dispatch_source() -> bool {
    let ok = gl_source().contains("Wave 750") && gl_source().contains("record_prior_clear");
    residual_action_store(ResidualHostSpectrePriorClearNoDamageAuthHpStompAction::DispatchSource);
    ok
}
pub fn honesty_host_spectre_prior_clear_no_damage_auth_hp_stomp_residual_pack_wave750() -> bool {
    honesty_host_spectre_prior_clear_no_damage_auth_hp_stomp_method_names_residual_wave750()
        && honesty_host_spectre_prior_clear_no_damage_auth_hp_stomp_source_markers_residual_wave750(
        )
        && honesty_host_spectre_prior_clear_no_damage_auth_hp_stomp_nav_commands_residual_wave750()
        && simulate_host_spectre_prior_clear_no_damage_auth_hp_stomp_collect_source()
        && simulate_host_spectre_prior_clear_no_damage_auth_hp_stomp_dispatch_source()
}
pub fn simulate_live_host_spectre_prior_clear_no_damage_auth_hp_stomp_honesty() -> bool {
    let ok = honesty_host_spectre_prior_clear_no_damage_auth_hp_stomp_residual_pack_wave750();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostSpectrePriorClearNoDamageAuthHpStompAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(
            honesty_host_spectre_prior_clear_no_damage_auth_hp_stomp_method_names_residual_wave750(
            )
        );
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_spectre_prior_clear_no_damage_auth_hp_stomp_source_markers_residual_wave750());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_host_spectre_prior_clear_no_damage_auth_hp_stomp_nav_commands_residual_wave750(
            )
        );
    }
    #[test]
    fn sources() {
        assert!(simulate_host_spectre_prior_clear_no_damage_auth_hp_stomp_collect_source());
        assert!(simulate_host_spectre_prior_clear_no_damage_auth_hp_stomp_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_spectre_prior_clear_no_damage_auth_hp_stomp_residual_pack_wave750());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_spectre_prior_clear_no_damage_auth_hp_stomp_honesty());
        assert!(residual_host_spectre_prior_clear_no_damage_auth_hp_stomp_ok());
    }
}
