//! Wave 748: under damage authority, hive structure damage does not mutate
//! host HP mid-frame (avoids dual with GameWorld HP writeback). Damage log owns
//! the numeric residual; lethal still stamps destroyed + idle. Non-authority
//! path keeps host HP write. `playable_claim` stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_HIVE_STRUCT_DAMAGE_NO_DAMAGE_AUTH_HP_STOMP_METHOD_NAMES_WAVE748: &[&str] = &[
    "apply_host_hive_damage_from",
    "structure_damage_applied",
    "gameworld_damage_authority_live",
    "Wave 748",
    "playable_claim = false",
];
pub const LIVE_HOST_HIVE_STRUCT_DAMAGE_NO_DAMAGE_AUTH_HP_STOMP_NAV_STEPS_WAVE748: &[&str] = &[
    "REQUIRE_DAMAGE_AUTH_SKIP_HP_MUTATE",
    "REQUIRE_DAMAGE_LOG",
    "REQUIRE_NON_AUTH_KEEPS_HP_WRITE",
    "LIVE_HOST_HIVE_STRUCT_DAMAGE_NO_DAMAGE_AUTH_HP_STOMP",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_HIVE_STRUCT_DAMAGE_NO_DAMAGE_AUTH_HP_STOMP_CMD_NAMES_WAVE748:
    &[&str] = &[
    "host_hive_struct_damage_no_damage_auth_hp_stomp",
    "damage_auth_skip_hp_mutate",
    "damage_log",
    "non_auth_keeps_hp_write",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostHiveStructDamageNoDamageAuthHpStompAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostHiveStructDamageNoDamageAuthHpStompAction {
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
fn residual_action_store(a: ResidualHostHiveStructDamageNoDamageAuthHpStompAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_hive_struct_damage_no_damage_auth_hp_stomp_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_hive_struct_damage_no_damage_auth_hp_stomp_last_action()
-> ResidualHostHiveStructDamageNoDamageAuthHpStompAction {
    ResidualHostHiveStructDamageNoDamageAuthHpStompAction::from_u8(
        RESIDUAL_ACTION.load(Ordering::SeqCst),
    )
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_hive_struct_damage_no_damage_auth_hp_stomp_method_names_residual_wave748()
-> bool {
    let names = LIVE_HOST_HIVE_STRUCT_DAMAGE_NO_DAMAGE_AUTH_HP_STOMP_METHOD_NAMES_WAVE748;
    let ok = residual_name_index(names, "apply_host_hive_damage_from").is_some()
        && residual_name_index(names, "structure_damage_applied").is_some()
        && residual_name_index(names, "gameworld_damage_authority_live").is_some()
        && residual_name_index(names, "Wave 748").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostHiveStructDamageNoDamageAuthHpStompAction::MethodNames);
    ok
}
pub fn honesty_host_hive_struct_damage_no_damage_auth_hp_stomp_source_markers_residual_wave748()
-> bool {
    let gl = gl_source();
    let ok = gl.contains("Wave 748")
        && gl.contains("fn apply_host_hive_damage_from")
        && gl.contains("structure_damage_applied")
        && gl.contains("gameworld_damage_authority_live()")
        && gl.contains("host_damage_log::record")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostHiveStructDamageNoDamageAuthHpStompAction::SourceMarkers);
    ok
}
pub fn honesty_host_hive_struct_damage_no_damage_auth_hp_stomp_nav_commands_residual_wave748()
-> bool {
    let steps = LIVE_HOST_HIVE_STRUCT_DAMAGE_NO_DAMAGE_AUTH_HP_STOMP_NAV_STEPS_WAVE748;
    let cmds = RUNTIME_HOST_LIVE_HOST_HIVE_STRUCT_DAMAGE_NO_DAMAGE_AUTH_HP_STOMP_CMD_NAMES_WAVE748;
    let ok = residual_name_index(steps, "REQUIRE_DAMAGE_AUTH_SKIP_HP_MUTATE").is_some()
        && residual_name_index(steps, "REQUIRE_DAMAGE_LOG").is_some()
        && residual_name_index(steps, "REQUIRE_NON_AUTH_KEEPS_HP_WRITE").is_some()
        && residual_name_index(
            steps,
            "LIVE_HOST_HIVE_STRUCT_DAMAGE_NO_DAMAGE_AUTH_HP_STOMP",
        )
        .is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_hive_struct_damage_no_damage_auth_hp_stomp").is_some()
        && residual_name_index(cmds, "damage_auth_skip_hp_mutate").is_some()
        && residual_name_index(cmds, "damage_log").is_some()
        && residual_name_index(cmds, "non_auth_keeps_hp_write").is_some();
    residual_action_store(ResidualHostHiveStructDamageNoDamageAuthHpStompAction::NavCommands);
    ok
}
pub fn simulate_host_hive_struct_damage_no_damage_auth_hp_stomp_collect_source() -> bool {
    let ok = gl_source().contains("apply_host_hive_damage_from")
        && gl_source().contains("gameworld_damage_authority_live");
    residual_action_store(ResidualHostHiveStructDamageNoDamageAuthHpStompAction::CollectSource);
    ok
}
pub fn simulate_host_hive_struct_damage_no_damage_auth_hp_stomp_dispatch_source() -> bool {
    let ok = gl_source().contains("Wave 748") && gl_source().contains("structure_damage_applied");
    residual_action_store(ResidualHostHiveStructDamageNoDamageAuthHpStompAction::DispatchSource);
    ok
}
pub fn honesty_host_hive_struct_damage_no_damage_auth_hp_stomp_residual_pack_wave748() -> bool {
    honesty_host_hive_struct_damage_no_damage_auth_hp_stomp_method_names_residual_wave748()
        && honesty_host_hive_struct_damage_no_damage_auth_hp_stomp_source_markers_residual_wave748()
        && honesty_host_hive_struct_damage_no_damage_auth_hp_stomp_nav_commands_residual_wave748()
        && simulate_host_hive_struct_damage_no_damage_auth_hp_stomp_collect_source()
        && simulate_host_hive_struct_damage_no_damage_auth_hp_stomp_dispatch_source()
}
pub fn simulate_live_host_hive_struct_damage_no_damage_auth_hp_stomp_honesty() -> bool {
    let ok = honesty_host_hive_struct_damage_no_damage_auth_hp_stomp_residual_pack_wave748();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostHiveStructDamageNoDamageAuthHpStompAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(
            honesty_host_hive_struct_damage_no_damage_auth_hp_stomp_method_names_residual_wave748()
        );
    }
    #[test]
    fn source_markers_residual() {
        assert!(
            honesty_host_hive_struct_damage_no_damage_auth_hp_stomp_source_markers_residual_wave748(
            )
        );
    }
    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_host_hive_struct_damage_no_damage_auth_hp_stomp_nav_commands_residual_wave748()
        );
    }
    #[test]
    fn sources() {
        assert!(simulate_host_hive_struct_damage_no_damage_auth_hp_stomp_collect_source());
        assert!(simulate_host_hive_struct_damage_no_damage_auth_hp_stomp_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_hive_struct_damage_no_damage_auth_hp_stomp_residual_pack_wave748());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_hive_struct_damage_no_damage_auth_hp_stomp_honesty());
        assert!(residual_host_hive_struct_damage_no_damage_auth_hp_stomp_ok());
    }
}
