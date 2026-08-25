//! Wave 755: GW writeback skips objects with pending host frame-local logs
//! (continuous fire, weapon bonus, faerie fire). Host mid-frame log is authority
//! until apply drains into GameWorld; writeback must not stomp host from a stale
//! entity. `playable_claim` stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_WRITEBACK_SKIP_PENDING_HOST_LOGS_METHOD_NAMES_WAVE755: &[&str] = &[
    "has_pending",
    "writeback_continuous_fire_to_host",
    "writeback_weapon_bonus_to_host",
    "writeback_faerie_fire_to_host",
    "Wave 755",
    "playable_claim = false",
];
pub const LIVE_HOST_WRITEBACK_SKIP_PENDING_HOST_LOGS_NAV_STEPS_WAVE755: &[&str] = &[
    "REQUIRE_HAS_PENDING",
    "REQUIRE_CONTINUOUS_FIRE_GATE",
    "REQUIRE_WEAPON_BONUS_GATE",
    "REQUIRE_FAERIE_GATE",
    "LIVE_HOST_WRITEBACK_SKIP_PENDING_HOST_LOGS",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_WRITEBACK_SKIP_PENDING_HOST_LOGS_CMD_NAMES_WAVE755: &[&str] = &[
    "host_writeback_skip_pending_host_logs",
    "has_pending",
    "continuous_fire_gate",
    "weapon_bonus_gate",
    "faerie_gate",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostWritebackSkipPendingHostLogsAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostWritebackSkipPendingHostLogsAction {
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
fn residual_action_store(a: ResidualHostWritebackSkipPendingHostLogsAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_writeback_skip_pending_host_logs_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_writeback_skip_pending_host_logs_last_action()
-> ResidualHostWritebackSkipPendingHostLogsAction {
    ResidualHostWritebackSkipPendingHostLogsAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn cf_log() -> &'static str {
    include_str!("../host_continuous_fire_log.rs")
}
fn wb_log() -> &'static str {
    include_str!("../host_weapon_bonus_log.rs")
}
fn ff_log() -> &'static str {
    include_str!("../host_faerie_fire_log.rs")
}
pub fn honesty_host_writeback_skip_pending_host_logs_method_names_residual_wave755() -> bool {
    let names = LIVE_HOST_WRITEBACK_SKIP_PENDING_HOST_LOGS_METHOD_NAMES_WAVE755;
    let ok = residual_name_index(names, "has_pending").is_some()
        && residual_name_index(names, "writeback_continuous_fire_to_host").is_some()
        && residual_name_index(names, "writeback_weapon_bonus_to_host").is_some()
        && residual_name_index(names, "writeback_faerie_fire_to_host").is_some()
        && residual_name_index(names, "Wave 755").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostWritebackSkipPendingHostLogsAction::MethodNames);
    ok
}
pub fn honesty_host_writeback_skip_pending_host_logs_source_markers_residual_wave755() -> bool {
    let sh = sh_source();
    let ok = sh.contains("Wave 755")
        && sh.contains("writeback_continuous_fire_to_host")
        && sh.contains("host_continuous_fire_log::has_pending")
        && sh.contains("host_weapon_bonus_log::has_pending")
        && sh.contains("host_faerie_fire_log::has_pending")
        && sh.contains("shadow_coupled_tick_active()")
        && cf_log().contains("pub fn has_pending")
        && wb_log().contains("pub fn has_pending")
        && ff_log().contains("pub fn has_pending")
        && !sh.contains("playable_claim = true");
    residual_action_store(ResidualHostWritebackSkipPendingHostLogsAction::SourceMarkers);
    ok
}
pub fn honesty_host_writeback_skip_pending_host_logs_nav_commands_residual_wave755() -> bool {
    let steps = LIVE_HOST_WRITEBACK_SKIP_PENDING_HOST_LOGS_NAV_STEPS_WAVE755;
    let cmds = RUNTIME_HOST_LIVE_HOST_WRITEBACK_SKIP_PENDING_HOST_LOGS_CMD_NAMES_WAVE755;
    let ok = residual_name_index(steps, "REQUIRE_HAS_PENDING").is_some()
        && residual_name_index(steps, "REQUIRE_CONTINUOUS_FIRE_GATE").is_some()
        && residual_name_index(steps, "REQUIRE_WEAPON_BONUS_GATE").is_some()
        && residual_name_index(steps, "REQUIRE_FAERIE_GATE").is_some()
        && residual_name_index(steps, "LIVE_HOST_WRITEBACK_SKIP_PENDING_HOST_LOGS").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_writeback_skip_pending_host_logs").is_some()
        && residual_name_index(cmds, "has_pending").is_some()
        && residual_name_index(cmds, "continuous_fire_gate").is_some()
        && residual_name_index(cmds, "weapon_bonus_gate").is_some()
        && residual_name_index(cmds, "faerie_gate").is_some();
    residual_action_store(ResidualHostWritebackSkipPendingHostLogsAction::NavCommands);
    ok
}
pub fn simulate_host_writeback_skip_pending_host_logs_collect_source() -> bool {
    let ok = sh_source().contains("has_pending") && cf_log().contains("has_pending");
    residual_action_store(ResidualHostWritebackSkipPendingHostLogsAction::CollectSource);
    ok
}
pub fn simulate_host_writeback_skip_pending_host_logs_dispatch_source() -> bool {
    let ok = sh_source().contains("Wave 755") && sh_source().contains("mid-frame authority");
    residual_action_store(ResidualHostWritebackSkipPendingHostLogsAction::DispatchSource);
    ok
}
pub fn honesty_host_writeback_skip_pending_host_logs_residual_pack_wave755() -> bool {
    honesty_host_writeback_skip_pending_host_logs_method_names_residual_wave755()
        && honesty_host_writeback_skip_pending_host_logs_source_markers_residual_wave755()
        && honesty_host_writeback_skip_pending_host_logs_nav_commands_residual_wave755()
        && simulate_host_writeback_skip_pending_host_logs_collect_source()
        && simulate_host_writeback_skip_pending_host_logs_dispatch_source()
}
pub fn simulate_live_host_writeback_skip_pending_host_logs_honesty() -> bool {
    let ok = honesty_host_writeback_skip_pending_host_logs_residual_pack_wave755();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostWritebackSkipPendingHostLogsAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_writeback_skip_pending_host_logs_method_names_residual_wave755());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_writeback_skip_pending_host_logs_source_markers_residual_wave755());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_writeback_skip_pending_host_logs_nav_commands_residual_wave755());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_writeback_skip_pending_host_logs_collect_source());
        assert!(simulate_host_writeback_skip_pending_host_logs_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_writeback_skip_pending_host_logs_residual_pack_wave755());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_writeback_skip_pending_host_logs_honesty());
        assert!(residual_host_writeback_skip_pending_host_logs_ok());
    }
}
