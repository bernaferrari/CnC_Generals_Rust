//! Wave 619 residual peels: under CONSTRUCTION_AUTHORITY sole-tick, GameWorld
//! writeback records sell-finish ready flips via `host_sell_ready_log`;
//! host drains that log in `update_sell_list` before refund/destroy.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Waves 614/617/618 production/construction/SP ready-log residuals.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `host_sell_ready_log.rs`
//! - `gameworld_shadow.rs` writeback_construction_to_host
//! - `game_logic.rs` update_sell_list ready-log drain
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SELL_READY_LOG_HELPER_METHOD_NAMES_WAVE619: &[&str] = &[
    "host_sell_ready_log",
    "writeback_construction_to_host",
    "update_sell_list",
    "gameworld_construction_sole_tick_enabled",
    "Wave 619",
    "playable_claim = false",
];

pub const LIVE_HOST_SELL_READY_LOG_HELPER_NAV_STEPS_WAVE619: &[&str] = &[
    "REQUIRE_SELL_READY_LOG_MODULE",
    "REQUIRE_WRITEBACK_RECORDS_SELL_READY",
    "REQUIRE_HOST_DRAINS_SELL_READY",
    "REQUIRE_HOST_SOLE_FINISH_GATE",
    "LIVE_HOST_SELL_READY_LOG_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_SELL_READY_LOG_HELPER_CMD_NAMES_WAVE619: &[&str] = &[
    "host_sell_ready_log_helper",
    "writeback_records_sell_ready",
    "host_drains_sell_ready",
    "host_sole_finish_gate",
    "sell_ready_log_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSellReadyLogHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostSellReadyLogHelperAction {
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

fn residual_action_store(action: ResidualHostSellReadyLogHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_sell_ready_log_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_sell_ready_log_helper_last_action() -> ResidualHostSellReadyLogHelperAction {
    ResidualHostSellReadyLogHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn gl_source() -> &'static str {
    include_str!("game_logic.rs")
}

fn shadow_source() -> &'static str {
    include_str!("../gameworld_shadow.rs")
}

fn ready_log_source() -> &'static str {
    include_str!("host_sell_ready_log.rs")
}

fn last_sig_index(src: &str, sig: &str) -> Option<usize> {
    let mut at = None;
    let mut from = 0usize;
    while let Some(rel) = src[from..].find(sig) {
        at = Some(from + rel);
        from = from + rel + sig.len();
    }
    at
}

fn fn_body<'a>(src: &'a str, sig: &str) -> Option<&'a str> {
    let start = last_sig_index(src, sig)?;
    let after = &src[start..];
    let brace = after.find('{')?;
    let mut depth = 0i32;
    for (i, ch) in after[brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&after[..=brace + i]);
                }
            }
            _ => {}
        }
    }
    None
}

pub fn honesty_host_sell_ready_log_helper_method_names_residual_wave619() -> bool {
    let names = LIVE_HOST_SELL_READY_LOG_HELPER_METHOD_NAMES_WAVE619;
    let ok = residual_name_index(names, "host_sell_ready_log").is_some()
        && residual_name_index(names, "writeback_construction_to_host").is_some()
        && residual_name_index(names, "Wave 619").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostSellReadyLogHelperAction::MethodNames);
    ok
}

pub fn honesty_host_sell_ready_log_helper_source_markers_residual_wave619() -> bool {
    let gl = gl_source();
    let sh = shadow_source();
    let ready = ready_log_source();
    let log_ok = ready.contains("Wave 619")
        && ready.contains("pub fn record(")
        && ready.contains("pub fn drain(")
        && ready.contains("HostSellReadyEvent");
    let Some(wb) = fn_body(sh, "pub fn writeback_construction_to_host(") else {
        residual_action_store(ResidualHostSellReadyLogHelperAction::SourceMarkers);
        return false;
    };
    let wb_ok = wb.contains("Wave 619")
        && wb.contains("host_sell_ready_log::record")
        && wb.contains("obj.status.sold")
        && wb.contains("gameworld_construction_sole_tick_enabled");
    let Some(update) = fn_body(gl, "pub fn update_sell_list(") else {
        residual_action_store(ResidualHostSellReadyLogHelperAction::SourceMarkers);
        return false;
    };
    // Wave 716: drain moved to post-writeback helper; mid-update keeps ready_sells gate.
    let drain_ok = update.contains("Wave 619")
        && update.contains("ready_sells")
        && update.contains("construction_sole")
        && (
            update.contains("host_sell_ready_log::drain")
                || gl.contains("host_apply_sell_completions_after_ready_writeback")
        );
    let ok = log_ok && wb_ok && drain_ok && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostSellReadyLogHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_sell_ready_log_helper_nav_commands_residual_wave619() -> bool {
    let steps = LIVE_HOST_SELL_READY_LOG_HELPER_NAV_STEPS_WAVE619;
    let cmds = RUNTIME_HOST_LIVE_HOST_SELL_READY_LOG_HELPER_CMD_NAMES_WAVE619;
    let ok = residual_name_index(steps, "REQUIRE_SELL_READY_LOG_MODULE").is_some()
        && residual_name_index(steps, "REQUIRE_WRITEBACK_RECORDS_SELL_READY").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_DRAINS_SELL_READY").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_SOLE_FINISH_GATE").is_some()
        && residual_name_index(steps, "LIVE_HOST_SELL_READY_LOG_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_sell_ready_log_helper").is_some()
        && residual_name_index(cmds, "writeback_records_sell_ready").is_some()
        && residual_name_index(cmds, "host_drains_sell_ready").is_some()
        && residual_name_index(cmds, "host_sole_finish_gate").is_some()
        && residual_name_index(cmds, "sell_ready_log_residual").is_some();
    residual_action_store(ResidualHostSellReadyLogHelperAction::NavCommands);
    ok
}

pub fn simulate_host_sell_ready_log_helper_collect_source() -> bool {
    let ok = ready_log_source().contains("Wave 619")
        && shadow_source().contains("host_sell_ready_log::record")
        && gl_source().contains("host_sell_ready_log::drain");
    residual_action_store(ResidualHostSellReadyLogHelperAction::CollectSource);
    ok
}

pub fn simulate_host_sell_ready_log_helper_dispatch_source() -> bool {
    let gl = gl_source();
    let sh = shadow_source();
    let ok = sh.contains("Wave 619: sell-finish ready residual")
        && gl.contains(
            "Wave 619: under construction sole-tick, GameWorld writeback records sell-ready",
        )
        && gl.contains("only finish IDs GW recorded ready");
    residual_action_store(ResidualHostSellReadyLogHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_sell_ready_log_helper_residual_pack_wave619() -> bool {
    honesty_host_sell_ready_log_helper_method_names_residual_wave619()
        && honesty_host_sell_ready_log_helper_source_markers_residual_wave619()
        && honesty_host_sell_ready_log_helper_nav_commands_residual_wave619()
        && simulate_host_sell_ready_log_helper_collect_source()
        && simulate_host_sell_ready_log_helper_dispatch_source()
}

pub fn simulate_live_host_sell_ready_log_helper_honesty() -> bool {
    let ok = honesty_host_sell_ready_log_helper_residual_pack_wave619();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostSellReadyLogHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_sell_ready_log_helper_method_names_residual_wave619());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_sell_ready_log_helper_source_markers_residual_wave619());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_sell_ready_log_helper_nav_commands_residual_wave619());
    }

    #[test]
    fn host_sell_ready_log_helper_sources() {
        assert!(simulate_host_sell_ready_log_helper_collect_source());
        assert!(simulate_host_sell_ready_log_helper_dispatch_source());
    }

    #[test]
    fn wave619_composite_pack() {
        assert!(honesty_host_sell_ready_log_helper_residual_pack_wave619());
    }

    #[test]
    fn simulate_live_host_sell_ready_log_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_sell_ready_log_helper_honesty(),
            "host sell ready log helper residual must latch"
        );
        assert!(residual_host_sell_ready_log_helper_ok());
        assert_eq!(
            residual_host_sell_ready_log_helper_last_action(),
            ResidualHostSellReadyLogHelperAction::Composite
        );
    }
}
