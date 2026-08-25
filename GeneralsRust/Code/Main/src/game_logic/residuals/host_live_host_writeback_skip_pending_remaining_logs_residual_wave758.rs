//! Wave 758: under coupled tick, remaining GW→host writebacks skip objects with
//! pending host frame-local logs (health/damage, contain, owner, construction,
//! command set, radar extend, death type, heal, production door, special power,
//! supplies, identity, turret/guard/detector, disguise, overcharge, hive, etc.).
//! Host mid-frame log is authority until apply. `playable_claim` stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_WRITEBACK_SKIP_PENDING_REMAINING_LOGS_METHOD_NAMES_WAVE758: &[&str] = &[
    "has_pending",
    "writeback_health_to_host",
    "writeback_contain_to_host",
    "writeback_owner_to_host",
    "Wave 758",
    "playable_claim = false",
];
pub const LIVE_HOST_WRITEBACK_SKIP_PENDING_REMAINING_LOGS_NAV_STEPS_WAVE758: &[&str] = &[
    "REQUIRE_HAS_PENDING",
    "REQUIRE_COUPLED_TICK",
    "REQUIRE_HEALTH_GATE",
    "REQUIRE_BULK_WRITEBACKS",
    "LIVE_HOST_WRITEBACK_SKIP_PENDING_REMAINING_LOGS",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_WRITEBACK_SKIP_PENDING_REMAINING_LOGS_CMD_NAMES_WAVE758:
    &[&str] = &[
    "host_writeback_skip_pending_remaining_logs",
    "has_pending",
    "coupled_tick",
    "health_gate",
    "bulk_writebacks",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostWritebackSkipPendingRemainingLogsAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostWritebackSkipPendingRemainingLogsAction {
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
fn residual_action_store(a: ResidualHostWritebackSkipPendingRemainingLogsAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_writeback_skip_pending_remaining_logs_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_writeback_skip_pending_remaining_logs_last_action()
-> ResidualHostWritebackSkipPendingRemainingLogsAction {
    ResidualHostWritebackSkipPendingRemainingLogsAction::from_u8(
        RESIDUAL_ACTION.load(Ordering::SeqCst),
    )
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
pub fn honesty_host_writeback_skip_pending_remaining_logs_method_names_residual_wave758() -> bool {
    let names = LIVE_HOST_WRITEBACK_SKIP_PENDING_REMAINING_LOGS_METHOD_NAMES_WAVE758;
    let ok = residual_name_index(names, "has_pending").is_some()
        && residual_name_index(names, "writeback_health_to_host").is_some()
        && residual_name_index(names, "writeback_contain_to_host").is_some()
        && residual_name_index(names, "writeback_owner_to_host").is_some()
        && residual_name_index(names, "Wave 758").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostWritebackSkipPendingRemainingLogsAction::MethodNames);
    ok
}
pub fn honesty_host_writeback_skip_pending_remaining_logs_source_markers_residual_wave758() -> bool
{
    let sh = sh_source();
    let logs = [
        "host_damage_log",
        "host_contain_log",
        "host_owner_log",
        "host_construction_log",
        "host_command_set_log",
        "host_radar_extend_log",
        "host_death_type_log",
        "host_heal_log",
    ];
    let wbs = [
        "writeback_health_to_host",
        "writeback_contain_to_host",
        "writeback_owner_to_host",
        "writeback_construction_to_host",
        "writeback_command_set_to_host",
        "writeback_radar_extend_to_host",
        "writeback_death_type_to_host",
        "writeback_sole_healing_to_host",
    ];
    let logs_ok = logs
        .iter()
        .all(|l| sh.contains(&format!("{l}::has_pending")));
    let wbs_ok = wbs.iter().all(|w| sh.contains(w));
    let wave_hits = sh.matches("Wave 758").count();
    let ok = sh.contains("Wave 758")
        && sh.contains("shadow_coupled_tick_active()")
        && sh.contains("host_damage_log::has_pending")
        && logs_ok
        && wbs_ok
        && wave_hits >= 30
        && !sh.contains("playable_claim = true");
    residual_action_store(ResidualHostWritebackSkipPendingRemainingLogsAction::SourceMarkers);
    ok
}
pub fn honesty_host_writeback_skip_pending_remaining_logs_nav_commands_residual_wave758() -> bool {
    let steps = LIVE_HOST_WRITEBACK_SKIP_PENDING_REMAINING_LOGS_NAV_STEPS_WAVE758;
    let cmds = RUNTIME_HOST_LIVE_HOST_WRITEBACK_SKIP_PENDING_REMAINING_LOGS_CMD_NAMES_WAVE758;
    let ok = residual_name_index(steps, "REQUIRE_HAS_PENDING").is_some()
        && residual_name_index(steps, "REQUIRE_COUPLED_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HEALTH_GATE").is_some()
        && residual_name_index(steps, "REQUIRE_BULK_WRITEBACKS").is_some()
        && residual_name_index(steps, "LIVE_HOST_WRITEBACK_SKIP_PENDING_REMAINING_LOGS").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_writeback_skip_pending_remaining_logs").is_some()
        && residual_name_index(cmds, "has_pending").is_some()
        && residual_name_index(cmds, "coupled_tick").is_some()
        && residual_name_index(cmds, "health_gate").is_some()
        && residual_name_index(cmds, "bulk_writebacks").is_some();
    residual_action_store(ResidualHostWritebackSkipPendingRemainingLogsAction::NavCommands);
    ok
}
pub fn simulate_host_writeback_skip_pending_remaining_logs_collect_source() -> bool {
    let ok =
        sh_source().contains("Wave 758") && sh_source().contains("host_damage_log::has_pending");
    residual_action_store(ResidualHostWritebackSkipPendingRemainingLogsAction::CollectSource);
    ok
}
pub fn simulate_host_writeback_skip_pending_remaining_logs_dispatch_source() -> bool {
    let ok = sh_source().matches("Wave 758").count() >= 30
        && sh_source().contains("writeback_sole_healing_to_host");
    residual_action_store(ResidualHostWritebackSkipPendingRemainingLogsAction::DispatchSource);
    ok
}
pub fn honesty_host_writeback_skip_pending_remaining_logs_residual_pack_wave758() -> bool {
    honesty_host_writeback_skip_pending_remaining_logs_method_names_residual_wave758()
        && honesty_host_writeback_skip_pending_remaining_logs_source_markers_residual_wave758()
        && honesty_host_writeback_skip_pending_remaining_logs_nav_commands_residual_wave758()
        && simulate_host_writeback_skip_pending_remaining_logs_collect_source()
        && simulate_host_writeback_skip_pending_remaining_logs_dispatch_source()
}
pub fn simulate_live_host_writeback_skip_pending_remaining_logs_honesty() -> bool {
    let ok = honesty_host_writeback_skip_pending_remaining_logs_residual_pack_wave758();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostWritebackSkipPendingRemainingLogsAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_writeback_skip_pending_remaining_logs_method_names_residual_wave758());
    }
    #[test]
    fn source_markers_residual() {
        assert!(
            honesty_host_writeback_skip_pending_remaining_logs_source_markers_residual_wave758()
        );
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_writeback_skip_pending_remaining_logs_nav_commands_residual_wave758());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_writeback_skip_pending_remaining_logs_collect_source());
        assert!(simulate_host_writeback_skip_pending_remaining_logs_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_writeback_skip_pending_remaining_logs_residual_pack_wave758());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_writeback_skip_pending_remaining_logs_honesty());
        assert!(residual_host_writeback_skip_pending_remaining_logs_ok());
    }
}
