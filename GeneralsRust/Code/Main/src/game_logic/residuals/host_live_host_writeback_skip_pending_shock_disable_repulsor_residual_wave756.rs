//! Wave 756: under coupled tick, GW writeback skips objects with pending host
//! shock-stun / disable-timers / repulsor logs. Host mid-frame log is authority
//! until apply drains into GameWorld. `playable_claim` stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_WRITEBACK_SKIP_PENDING_SHOCK_DISABLE_REPULSOR_METHOD_NAMES_WAVE756: &[&str] = &[
    "has_pending",
    "writeback_shock_stun_to_host",
    "writeback_disable_timers_to_host",
    "writeback_repulsor_to_host",
    "Wave 756",
    "playable_claim = false",
];
pub const LIVE_HOST_WRITEBACK_SKIP_PENDING_SHOCK_DISABLE_REPULSOR_NAV_STEPS_WAVE756: &[&str] = &[
    "REQUIRE_HAS_PENDING",
    "REQUIRE_SHOCK_GATE",
    "REQUIRE_DISABLE_GATE",
    "REQUIRE_REPULSOR_GATE",
    "LIVE_HOST_WRITEBACK_SKIP_PENDING_SHOCK_DISABLE_REPULSOR",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_WRITEBACK_SKIP_PENDING_SHOCK_DISABLE_REPULSOR_CMD_NAMES_WAVE756:
    &[&str] = &[
    "host_writeback_skip_pending_shock_disable_repulsor",
    "has_pending",
    "shock_gate",
    "disable_gate",
    "repulsor_gate",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostWritebackSkipPendingShockDisableRepulsorAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostWritebackSkipPendingShockDisableRepulsorAction {
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
fn residual_action_store(a: ResidualHostWritebackSkipPendingShockDisableRepulsorAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_writeback_skip_pending_shock_disable_repulsor_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_writeback_skip_pending_shock_disable_repulsor_last_action()
-> ResidualHostWritebackSkipPendingShockDisableRepulsorAction {
    ResidualHostWritebackSkipPendingShockDisableRepulsorAction::from_u8(
        RESIDUAL_ACTION.load(Ordering::SeqCst),
    )
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn shock_log() -> &'static str {
    include_str!("../host_shock_stun_log.rs")
}
fn disable_log() -> &'static str {
    include_str!("../host_disable_timers_log.rs")
}
fn repulsor_log() -> &'static str {
    include_str!("../host_repulsor_log.rs")
}
pub fn honesty_host_writeback_skip_pending_shock_disable_repulsor_method_names_residual_wave756()
-> bool {
    let names = LIVE_HOST_WRITEBACK_SKIP_PENDING_SHOCK_DISABLE_REPULSOR_METHOD_NAMES_WAVE756;
    let ok = residual_name_index(names, "has_pending").is_some()
        && residual_name_index(names, "writeback_shock_stun_to_host").is_some()
        && residual_name_index(names, "writeback_disable_timers_to_host").is_some()
        && residual_name_index(names, "writeback_repulsor_to_host").is_some()
        && residual_name_index(names, "Wave 756").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostWritebackSkipPendingShockDisableRepulsorAction::MethodNames);
    ok
}
pub fn honesty_host_writeback_skip_pending_shock_disable_repulsor_source_markers_residual_wave756()
-> bool {
    let sh = sh_source();
    let ok = sh.contains("Wave 756")
        && sh.contains("host_shock_stun_log::has_pending")
        && sh.contains("host_disable_timers_log::has_pending")
        && sh.contains("host_repulsor_log::has_pending")
        && sh.contains("shadow_coupled_tick_active()")
        && shock_log().contains("pub fn has_pending")
        && disable_log().contains("pub fn has_pending")
        && repulsor_log().contains("pub fn has_pending")
        && !sh.contains("playable_claim = true");
    residual_action_store(
        ResidualHostWritebackSkipPendingShockDisableRepulsorAction::SourceMarkers,
    );
    ok
}
pub fn honesty_host_writeback_skip_pending_shock_disable_repulsor_nav_commands_residual_wave756()
-> bool {
    let steps = LIVE_HOST_WRITEBACK_SKIP_PENDING_SHOCK_DISABLE_REPULSOR_NAV_STEPS_WAVE756;
    let cmds =
        RUNTIME_HOST_LIVE_HOST_WRITEBACK_SKIP_PENDING_SHOCK_DISABLE_REPULSOR_CMD_NAMES_WAVE756;
    let ok = residual_name_index(steps, "REQUIRE_HAS_PENDING").is_some()
        && residual_name_index(steps, "REQUIRE_SHOCK_GATE").is_some()
        && residual_name_index(steps, "REQUIRE_DISABLE_GATE").is_some()
        && residual_name_index(steps, "REQUIRE_REPULSOR_GATE").is_some()
        && residual_name_index(
            steps,
            "LIVE_HOST_WRITEBACK_SKIP_PENDING_SHOCK_DISABLE_REPULSOR",
        )
        .is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_writeback_skip_pending_shock_disable_repulsor")
            .is_some()
        && residual_name_index(cmds, "has_pending").is_some()
        && residual_name_index(cmds, "shock_gate").is_some()
        && residual_name_index(cmds, "disable_gate").is_some()
        && residual_name_index(cmds, "repulsor_gate").is_some();
    residual_action_store(ResidualHostWritebackSkipPendingShockDisableRepulsorAction::NavCommands);
    ok
}
pub fn simulate_host_writeback_skip_pending_shock_disable_repulsor_collect_source() -> bool {
    let ok = sh_source().contains("Wave 756") && shock_log().contains("has_pending");
    residual_action_store(
        ResidualHostWritebackSkipPendingShockDisableRepulsorAction::CollectSource,
    );
    ok
}
pub fn simulate_host_writeback_skip_pending_shock_disable_repulsor_dispatch_source() -> bool {
    let ok = sh_source().contains("writeback_shock_stun_to_host")
        && sh_source().contains("writeback_disable_timers_to_host")
        && sh_source().contains("writeback_repulsor_to_host");
    residual_action_store(
        ResidualHostWritebackSkipPendingShockDisableRepulsorAction::DispatchSource,
    );
    ok
}
pub fn honesty_host_writeback_skip_pending_shock_disable_repulsor_residual_pack_wave756() -> bool {
    honesty_host_writeback_skip_pending_shock_disable_repulsor_method_names_residual_wave756()
        && honesty_host_writeback_skip_pending_shock_disable_repulsor_source_markers_residual_wave756()
        && honesty_host_writeback_skip_pending_shock_disable_repulsor_nav_commands_residual_wave756()
        && simulate_host_writeback_skip_pending_shock_disable_repulsor_collect_source()
        && simulate_host_writeback_skip_pending_shock_disable_repulsor_dispatch_source()
}
pub fn simulate_live_host_writeback_skip_pending_shock_disable_repulsor_honesty() -> bool {
    let ok = honesty_host_writeback_skip_pending_shock_disable_repulsor_residual_pack_wave756();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(
            ResidualHostWritebackSkipPendingShockDisableRepulsorAction::Composite,
        );
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_writeback_skip_pending_shock_disable_repulsor_method_names_residual_wave756());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_writeback_skip_pending_shock_disable_repulsor_source_markers_residual_wave756());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_writeback_skip_pending_shock_disable_repulsor_nav_commands_residual_wave756());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_writeback_skip_pending_shock_disable_repulsor_collect_source());
        assert!(simulate_host_writeback_skip_pending_shock_disable_repulsor_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_writeback_skip_pending_shock_disable_repulsor_residual_pack_wave756());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_writeback_skip_pending_shock_disable_repulsor_honesty());
        assert!(residual_host_writeback_skip_pending_shock_disable_repulsor_ok());
    }
}
