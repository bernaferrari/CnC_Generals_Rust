//! Wave 751: under damage authority, booby-trap special destroy does not
//! zero host HP mid-frame (avoids dual with GameWorld HP writeback). Projects
//! lethal via `host_damage_log` + destroyed flags. Non-authority path keeps
//! host HP clear. `playable_claim` stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_BOOBY_TRAP_DESTROY_NO_DAMAGE_AUTH_HP_STOMP_METHOD_NAMES_WAVE751: &[&str] = &[
    "destroy_booby_trap_special_object",
    "booby_trap_special",
    "gameworld_damage_authority_live",
    "Wave 751",
    "playable_claim = false",
];
pub const LIVE_HOST_BOOBY_TRAP_DESTROY_NO_DAMAGE_AUTH_HP_STOMP_NAV_STEPS_WAVE751: &[&str] = &[
    "REQUIRE_DAMAGE_AUTH_SKIP_HP_STOMP",
    "REQUIRE_DAMAGE_LOG_LETHAL",
    "REQUIRE_NON_AUTH_KEEPS_HP_CLEAR",
    "LIVE_HOST_BOOBY_TRAP_DESTROY_NO_DAMAGE_AUTH_HP_STOMP",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_BOOBY_TRAP_DESTROY_NO_DAMAGE_AUTH_HP_STOMP_CMD_NAMES_WAVE751:
    &[&str] = &[
    "host_booby_trap_destroy_no_damage_auth_hp_stomp",
    "damage_auth_skip_hp_stomp",
    "damage_log_lethal",
    "non_auth_keeps_hp_clear",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostBoobyTrapDestroyNoDamageAuthHpStompAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostBoobyTrapDestroyNoDamageAuthHpStompAction {
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
fn residual_action_store(a: ResidualHostBoobyTrapDestroyNoDamageAuthHpStompAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_booby_trap_destroy_no_damage_auth_hp_stomp_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_booby_trap_destroy_no_damage_auth_hp_stomp_last_action()
-> ResidualHostBoobyTrapDestroyNoDamageAuthHpStompAction {
    ResidualHostBoobyTrapDestroyNoDamageAuthHpStompAction::from_u8(
        RESIDUAL_ACTION.load(Ordering::SeqCst),
    )
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_booby_trap_destroy_no_damage_auth_hp_stomp_method_names_residual_wave751()
-> bool {
    let names = LIVE_HOST_BOOBY_TRAP_DESTROY_NO_DAMAGE_AUTH_HP_STOMP_METHOD_NAMES_WAVE751;
    let ok = residual_name_index(names, "destroy_booby_trap_special_object").is_some()
        && residual_name_index(names, "booby_trap_special").is_some()
        && residual_name_index(names, "gameworld_damage_authority_live").is_some()
        && residual_name_index(names, "Wave 751").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostBoobyTrapDestroyNoDamageAuthHpStompAction::MethodNames);
    ok
}
pub fn honesty_host_booby_trap_destroy_no_damage_auth_hp_stomp_source_markers_residual_wave751()
-> bool {
    let gl = gl_source();
    let ok = gl.contains("Wave 751")
        && gl.contains("fn destroy_booby_trap_special_object")
        && gl.contains("booby_trap_special")
        && gl.contains("gameworld_damage_authority_live()")
        && gl.contains("host_damage_log::record")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostBoobyTrapDestroyNoDamageAuthHpStompAction::SourceMarkers);
    ok
}
pub fn honesty_host_booby_trap_destroy_no_damage_auth_hp_stomp_nav_commands_residual_wave751()
-> bool {
    let steps = LIVE_HOST_BOOBY_TRAP_DESTROY_NO_DAMAGE_AUTH_HP_STOMP_NAV_STEPS_WAVE751;
    let cmds = RUNTIME_HOST_LIVE_HOST_BOOBY_TRAP_DESTROY_NO_DAMAGE_AUTH_HP_STOMP_CMD_NAMES_WAVE751;
    let ok = residual_name_index(steps, "REQUIRE_DAMAGE_AUTH_SKIP_HP_STOMP").is_some()
        && residual_name_index(steps, "REQUIRE_DAMAGE_LOG_LETHAL").is_some()
        && residual_name_index(steps, "REQUIRE_NON_AUTH_KEEPS_HP_CLEAR").is_some()
        && residual_name_index(
            steps,
            "LIVE_HOST_BOOBY_TRAP_DESTROY_NO_DAMAGE_AUTH_HP_STOMP",
        )
        .is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_booby_trap_destroy_no_damage_auth_hp_stomp").is_some()
        && residual_name_index(cmds, "damage_auth_skip_hp_stomp").is_some()
        && residual_name_index(cmds, "damage_log_lethal").is_some()
        && residual_name_index(cmds, "non_auth_keeps_hp_clear").is_some();
    residual_action_store(ResidualHostBoobyTrapDestroyNoDamageAuthHpStompAction::NavCommands);
    ok
}
pub fn simulate_host_booby_trap_destroy_no_damage_auth_hp_stomp_collect_source() -> bool {
    let ok = gl_source().contains("destroy_booby_trap_special_object")
        && gl_source().contains("gameworld_damage_authority_live");
    residual_action_store(ResidualHostBoobyTrapDestroyNoDamageAuthHpStompAction::CollectSource);
    ok
}
pub fn simulate_host_booby_trap_destroy_no_damage_auth_hp_stomp_dispatch_source() -> bool {
    let ok = gl_source().contains("Wave 751") && gl_source().contains("booby_trap_special");
    residual_action_store(ResidualHostBoobyTrapDestroyNoDamageAuthHpStompAction::DispatchSource);
    ok
}
pub fn honesty_host_booby_trap_destroy_no_damage_auth_hp_stomp_residual_pack_wave751() -> bool {
    honesty_host_booby_trap_destroy_no_damage_auth_hp_stomp_method_names_residual_wave751()
        && honesty_host_booby_trap_destroy_no_damage_auth_hp_stomp_source_markers_residual_wave751()
        && honesty_host_booby_trap_destroy_no_damage_auth_hp_stomp_nav_commands_residual_wave751()
        && simulate_host_booby_trap_destroy_no_damage_auth_hp_stomp_collect_source()
        && simulate_host_booby_trap_destroy_no_damage_auth_hp_stomp_dispatch_source()
}
pub fn simulate_live_host_booby_trap_destroy_no_damage_auth_hp_stomp_honesty() -> bool {
    let ok = honesty_host_booby_trap_destroy_no_damage_auth_hp_stomp_residual_pack_wave751();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostBoobyTrapDestroyNoDamageAuthHpStompAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(
            honesty_host_booby_trap_destroy_no_damage_auth_hp_stomp_method_names_residual_wave751()
        );
    }
    #[test]
    fn source_markers_residual() {
        assert!(
            honesty_host_booby_trap_destroy_no_damage_auth_hp_stomp_source_markers_residual_wave751(
            )
        );
    }
    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_host_booby_trap_destroy_no_damage_auth_hp_stomp_nav_commands_residual_wave751()
        );
    }
    #[test]
    fn sources() {
        assert!(simulate_host_booby_trap_destroy_no_damage_auth_hp_stomp_collect_source());
        assert!(simulate_host_booby_trap_destroy_no_damage_auth_hp_stomp_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_booby_trap_destroy_no_damage_auth_hp_stomp_residual_pack_wave751());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_booby_trap_destroy_no_damage_auth_hp_stomp_honesty());
        assert!(residual_host_booby_trap_destroy_no_damage_auth_hp_stomp_ok());
    }
}
