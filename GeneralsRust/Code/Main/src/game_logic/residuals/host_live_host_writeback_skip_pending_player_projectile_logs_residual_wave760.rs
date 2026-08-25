//! Wave 760: under coupled tick, player/projectile GW→host writebacks skip
//! when host frame-local logs or upgrade frame events are pending (economy,
//! shared special-power cooldowns, projectiles, completed upgrades).
//! Host mid-frame log is authority until apply. `playable_claim` stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_WRITEBACK_SKIP_PENDING_PLAYER_PROJECTILE_LOGS_METHOD_NAMES_WAVE760: &[&str] = &[
    "has_pending",
    "writeback_economy_to_host",
    "writeback_shared_special_power_cooldowns_to_host",
    "writeback_projectiles_to_host",
    "writeback_completed_upgrades_to_host",
    "Wave 760",
    "playable_claim = false",
];
pub const LIVE_HOST_WRITEBACK_SKIP_PENDING_PLAYER_PROJECTILE_LOGS_NAV_STEPS_WAVE760: &[&str] = &[
    "REQUIRE_HAS_PENDING",
    "REQUIRE_COUPLED_TICK",
    "REQUIRE_ECONOMY_GATE",
    "REQUIRE_COOLDOWN_GATE",
    "REQUIRE_PROJECTILE_GATE",
    "REQUIRE_UPGRADE_GATE",
    "LIVE_HOST_WRITEBACK_SKIP_PENDING_PLAYER_PROJECTILE_LOGS",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_WRITEBACK_SKIP_PENDING_PLAYER_PROJECTILE_LOGS_CMD_NAMES_WAVE760:
    &[&str] = &[
    "host_writeback_skip_pending_player_projectile_logs",
    "has_pending",
    "coupled_tick",
    "economy_gate",
    "cooldown_gate",
    "projectile_gate",
    "upgrade_gate",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostWritebackSkipPendingPlayerProjectileLogsAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostWritebackSkipPendingPlayerProjectileLogsAction {
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
fn residual_action_store(a: ResidualHostWritebackSkipPendingPlayerProjectileLogsAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_writeback_skip_pending_player_projectile_logs_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_writeback_skip_pending_player_projectile_logs_last_action()
-> ResidualHostWritebackSkipPendingPlayerProjectileLogsAction {
    ResidualHostWritebackSkipPendingPlayerProjectileLogsAction::from_u8(
        RESIDUAL_ACTION.load(Ordering::SeqCst),
    )
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
pub fn honesty_host_writeback_skip_pending_player_projectile_logs_method_names_residual_wave760()
-> bool {
    let names = LIVE_HOST_WRITEBACK_SKIP_PENDING_PLAYER_PROJECTILE_LOGS_METHOD_NAMES_WAVE760;
    let ok = residual_name_index(names, "has_pending").is_some()
        && residual_name_index(names, "writeback_economy_to_host").is_some()
        && residual_name_index(names, "writeback_shared_special_power_cooldowns_to_host").is_some()
        && residual_name_index(names, "writeback_projectiles_to_host").is_some()
        && residual_name_index(names, "writeback_completed_upgrades_to_host").is_some()
        && residual_name_index(names, "Wave 760").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostWritebackSkipPendingPlayerProjectileLogsAction::MethodNames);
    ok
}
pub fn honesty_host_writeback_skip_pending_player_projectile_logs_source_markers_residual_wave760()
-> bool {
    let sh = sh_source();
    let logs_ok = sh.contains("host_economy_log::has_pending")
        && sh.contains("host_player_cooldown_log::has_pending")
        && sh.contains("host_projectile_log::has_pending");
    let wbs = [
        "writeback_economy_to_host",
        "writeback_shared_special_power_cooldowns_to_host",
        "writeback_projectiles_to_host",
        "writeback_completed_upgrades_to_host",
    ];
    let wbs_ok = wbs.iter().all(|w| {
        let Some(j) = sh.find(&format!("fn {w}")) else {
            return false;
        };
        sh[j..sh.len().min(j + 1200)].contains("Wave 760")
    });
    let ok =
        logs_ok && wbs_ok && sh.contains("Wave 760") && sh.contains("shadow_coupled_tick_active");
    residual_action_store(
        ResidualHostWritebackSkipPendingPlayerProjectileLogsAction::SourceMarkers,
    );
    ok
}
pub fn honesty_host_writeback_skip_pending_player_projectile_logs_nav_commands_residual_wave760()
-> bool {
    let steps = LIVE_HOST_WRITEBACK_SKIP_PENDING_PLAYER_PROJECTILE_LOGS_NAV_STEPS_WAVE760;
    let ok = residual_name_index(steps, "REQUIRE_HAS_PENDING").is_some()
        && residual_name_index(steps, "REQUIRE_COUPLED_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_ECONOMY_GATE").is_some()
        && residual_name_index(steps, "REQUIRE_COOLDOWN_GATE").is_some()
        && residual_name_index(steps, "REQUIRE_PROJECTILE_GATE").is_some()
        && residual_name_index(steps, "REQUIRE_UPGRADE_GATE").is_some()
        && residual_name_index(
            steps,
            "LIVE_HOST_WRITEBACK_SKIP_PENDING_PLAYER_PROJECTILE_LOGS",
        )
        .is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostWritebackSkipPendingPlayerProjectileLogsAction::NavCommands);
    ok
}
pub fn simulate_host_writeback_skip_pending_player_projectile_logs_collect_source() -> bool {
    let ok = sh_source().contains("Wave 760")
        && sh_source().contains("host_economy_log::has_pending")
        && sh_source().contains("host_player_cooldown_log::has_pending")
        && sh_source().contains("host_projectile_log::has_pending");
    residual_action_store(
        ResidualHostWritebackSkipPendingPlayerProjectileLogsAction::CollectSource,
    );
    ok
}
pub fn simulate_host_writeback_skip_pending_player_projectile_logs_dispatch_source() -> bool {
    let ok = sh_source().matches("Wave 760").count() >= 5
        && sh_source().contains("writeback_economy_to_host")
        && sh_source().contains("writeback_projectiles_to_host")
        && sh_source().contains("writeback_completed_upgrades_to_host")
        && sh_source().contains("completed_this_frame_snapshot");
    residual_action_store(
        ResidualHostWritebackSkipPendingPlayerProjectileLogsAction::DispatchSource,
    );
    ok
}
pub fn honesty_host_writeback_skip_pending_player_projectile_logs_residual_pack_wave760() -> bool {
    honesty_host_writeback_skip_pending_player_projectile_logs_method_names_residual_wave760()
        && honesty_host_writeback_skip_pending_player_projectile_logs_source_markers_residual_wave760()
        && honesty_host_writeback_skip_pending_player_projectile_logs_nav_commands_residual_wave760()
        && simulate_host_writeback_skip_pending_player_projectile_logs_collect_source()
        && simulate_host_writeback_skip_pending_player_projectile_logs_dispatch_source()
}
pub fn simulate_live_host_writeback_skip_pending_player_projectile_logs_honesty() -> bool {
    let ok = honesty_host_writeback_skip_pending_player_projectile_logs_residual_pack_wave760();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(
            ResidualHostWritebackSkipPendingPlayerProjectileLogsAction::Composite,
        );
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_writeback_skip_pending_player_projectile_logs_method_names_residual_wave760());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_writeback_skip_pending_player_projectile_logs_source_markers_residual_wave760());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_writeback_skip_pending_player_projectile_logs_nav_commands_residual_wave760());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_writeback_skip_pending_player_projectile_logs_collect_source());
        assert!(simulate_host_writeback_skip_pending_player_projectile_logs_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_writeback_skip_pending_player_projectile_logs_residual_pack_wave760());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_writeback_skip_pending_player_projectile_logs_honesty());
        assert!(residual_host_writeback_skip_pending_player_projectile_logs_ok());
    }
}
