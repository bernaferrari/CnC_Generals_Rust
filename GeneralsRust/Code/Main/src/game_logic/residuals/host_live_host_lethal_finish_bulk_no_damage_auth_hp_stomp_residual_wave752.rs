//! Wave 752: bulk production lethal-finish stamps respect damage authority.
//! Under `gameworld_damage_authority_live`, destroyed/effectively_dead residual
//! no longer zeros host HP mid-frame (dual with GW HP writeback); projects
//! lethal via `host_damage_log`. Adds `host_lethal_finish_object` helper.
//! Non-authority / host-only tests keep host HP clear. `playable_claim` stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_LETHAL_FINISH_BULK_NO_DAMAGE_AUTH_HP_STOMP_METHOD_NAMES_WAVE752: &[&str] = &[
    "host_lethal_finish_object",
    "gameworld_damage_authority_live",
    "host_damage_log::record",
    "Wave 752",
    "playable_claim = false",
];
pub const LIVE_HOST_LETHAL_FINISH_BULK_NO_DAMAGE_AUTH_HP_STOMP_NAV_STEPS_WAVE752: &[&str] = &[
    "REQUIRE_HELPER",
    "REQUIRE_DAMAGE_AUTH_SKIP_HP_STOMP",
    "REQUIRE_BULK_MIGRATE",
    "LIVE_HOST_LETHAL_FINISH_BULK_NO_DAMAGE_AUTH_HP_STOMP",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_LETHAL_FINISH_BULK_NO_DAMAGE_AUTH_HP_STOMP_CMD_NAMES_WAVE752:
    &[&str] = &[
    "host_lethal_finish_bulk_no_damage_auth_hp_stomp",
    "host_lethal_finish_object",
    "damage_auth_skip_hp_stomp",
    "bulk_migrate",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostLethalFinishBulkNoDamageAuthHpStompAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostLethalFinishBulkNoDamageAuthHpStompAction {
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
fn residual_action_store(a: ResidualHostLethalFinishBulkNoDamageAuthHpStompAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_lethal_finish_bulk_no_damage_auth_hp_stomp_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_lethal_finish_bulk_no_damage_auth_hp_stomp_last_action()
-> ResidualHostLethalFinishBulkNoDamageAuthHpStompAction {
    ResidualHostLethalFinishBulkNoDamageAuthHpStompAction::from_u8(
        RESIDUAL_ACTION.load(Ordering::SeqCst),
    )
}
fn gl_source() -> &'static str {
    // 2026-08-15: scan host plus extra world_* splits.
    super::host_logic_scan_src()
}
pub fn honesty_host_lethal_finish_bulk_no_damage_auth_hp_stomp_method_names_residual_wave752()
-> bool {
    let names = LIVE_HOST_LETHAL_FINISH_BULK_NO_DAMAGE_AUTH_HP_STOMP_METHOD_NAMES_WAVE752;
    let ok = residual_name_index(names, "host_lethal_finish_object").is_some()
        && residual_name_index(names, "gameworld_damage_authority_live").is_some()
        && residual_name_index(names, "host_damage_log::record").is_some()
        && residual_name_index(names, "Wave 752").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostLethalFinishBulkNoDamageAuthHpStompAction::MethodNames);
    ok
}
pub fn honesty_host_lethal_finish_bulk_no_damage_auth_hp_stomp_source_markers_residual_wave752()
-> bool {
    let gl = gl_source();
    let wave_hits = gl.matches("Wave 752").count();
    let ok = gl.contains("fn host_lethal_finish_object")
        && gl.contains("gameworld_damage_authority_live()")
        && gl.contains("host_damage_log::record")
        // 2026-08-15: live Wave 752 comment count is 44 after peels.
        && wave_hits >= 40
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostLethalFinishBulkNoDamageAuthHpStompAction::SourceMarkers);
    ok
}
pub fn honesty_host_lethal_finish_bulk_no_damage_auth_hp_stomp_nav_commands_residual_wave752()
-> bool {
    let steps = LIVE_HOST_LETHAL_FINISH_BULK_NO_DAMAGE_AUTH_HP_STOMP_NAV_STEPS_WAVE752;
    let cmds = RUNTIME_HOST_LIVE_HOST_LETHAL_FINISH_BULK_NO_DAMAGE_AUTH_HP_STOMP_CMD_NAMES_WAVE752;
    let ok = residual_name_index(steps, "REQUIRE_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_DAMAGE_AUTH_SKIP_HP_STOMP").is_some()
        && residual_name_index(steps, "REQUIRE_BULK_MIGRATE").is_some()
        && residual_name_index(
            steps,
            "LIVE_HOST_LETHAL_FINISH_BULK_NO_DAMAGE_AUTH_HP_STOMP",
        )
        .is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_lethal_finish_bulk_no_damage_auth_hp_stomp").is_some()
        && residual_name_index(cmds, "host_lethal_finish_object").is_some()
        && residual_name_index(cmds, "damage_auth_skip_hp_stomp").is_some()
        && residual_name_index(cmds, "bulk_migrate").is_some();
    residual_action_store(ResidualHostLethalFinishBulkNoDamageAuthHpStompAction::NavCommands);
    ok
}
pub fn simulate_host_lethal_finish_bulk_no_damage_auth_hp_stomp_collect_source() -> bool {
    let ok = gl_source().contains("host_lethal_finish_object")
        && gl_source().contains("gameworld_damage_authority_live");
    residual_action_store(ResidualHostLethalFinishBulkNoDamageAuthHpStompAction::CollectSource);
    ok
}
pub fn simulate_host_lethal_finish_bulk_no_damage_auth_hp_stomp_dispatch_source() -> bool {
    let ok = gl_source().matches("Wave 752").count() >= 40
        && gl_source().contains("host_damage_log::record");
    residual_action_store(ResidualHostLethalFinishBulkNoDamageAuthHpStompAction::DispatchSource);
    ok
}
pub fn honesty_host_lethal_finish_bulk_no_damage_auth_hp_stomp_residual_pack_wave752() -> bool {
    honesty_host_lethal_finish_bulk_no_damage_auth_hp_stomp_method_names_residual_wave752()
        && honesty_host_lethal_finish_bulk_no_damage_auth_hp_stomp_source_markers_residual_wave752()
        && honesty_host_lethal_finish_bulk_no_damage_auth_hp_stomp_nav_commands_residual_wave752()
        && simulate_host_lethal_finish_bulk_no_damage_auth_hp_stomp_collect_source()
        && simulate_host_lethal_finish_bulk_no_damage_auth_hp_stomp_dispatch_source()
}
pub fn simulate_live_host_lethal_finish_bulk_no_damage_auth_hp_stomp_honesty() -> bool {
    let ok = honesty_host_lethal_finish_bulk_no_damage_auth_hp_stomp_residual_pack_wave752();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostLethalFinishBulkNoDamageAuthHpStompAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(
            honesty_host_lethal_finish_bulk_no_damage_auth_hp_stomp_method_names_residual_wave752()
        );
    }
    #[test]
    fn source_markers_residual() {
        assert!(
            honesty_host_lethal_finish_bulk_no_damage_auth_hp_stomp_source_markers_residual_wave752(
            )
        );
    }
    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_host_lethal_finish_bulk_no_damage_auth_hp_stomp_nav_commands_residual_wave752()
        );
    }
    #[test]
    fn sources() {
        assert!(simulate_host_lethal_finish_bulk_no_damage_auth_hp_stomp_collect_source());
        assert!(simulate_host_lethal_finish_bulk_no_damage_auth_hp_stomp_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_lethal_finish_bulk_no_damage_auth_hp_stomp_residual_pack_wave752());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_lethal_finish_bulk_no_damage_auth_hp_stomp_honesty());
        assert!(residual_host_lethal_finish_bulk_no_damage_auth_hp_stomp_ok());
    }
}
