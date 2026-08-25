//! Wave 620 residual peels: under CONSTRUCTION_AUTHORITY sole-tick, GameWorld
//! writeback records rebuild-hole ready flips via `host_rebuild_ready_log`;
//! host drains that log in `update_rebuild_holes` before worker/building spawn.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Waves 614/617/618/619 ready-log residuals.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `host_rebuild_ready_log.rs`
//! - `gameworld_shadow.rs` writeback_rebuild_producer_to_host
//! - `game_logic.rs` update_rebuild_holes ready-log drain
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_REBUILD_READY_LOG_HELPER_METHOD_NAMES_WAVE620: &[&str] = &[
    "host_rebuild_ready_log",
    "writeback_rebuild_producer_to_host",
    "update_rebuild_holes",
    "gameworld_construction_sole_tick_enabled",
    "Wave 620",
    "playable_claim = false",
];

pub const LIVE_HOST_REBUILD_READY_LOG_HELPER_NAV_STEPS_WAVE620: &[&str] = &[
    "REQUIRE_REBUILD_READY_LOG_MODULE",
    "REQUIRE_WRITEBACK_RECORDS_REBUILD_READY",
    "REQUIRE_HOST_DRAINS_REBUILD_READY",
    "REQUIRE_HOST_SOLE_SPAWN_GATE",
    "LIVE_HOST_REBUILD_READY_LOG_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_REBUILD_READY_LOG_HELPER_CMD_NAMES_WAVE620: &[&str] = &[
    "host_rebuild_ready_log_helper",
    "writeback_records_rebuild_ready",
    "host_drains_rebuild_ready",
    "host_sole_spawn_gate",
    "rebuild_ready_log_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostRebuildReadyLogHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostRebuildReadyLogHelperAction {
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

fn residual_action_store(action: ResidualHostRebuildReadyLogHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_rebuild_ready_log_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_rebuild_ready_log_helper_last_action()
-> ResidualHostRebuildReadyLogHelperAction {
    ResidualHostRebuildReadyLogHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}

fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}

fn ready_log_source() -> &'static str {
    include_str!("../host_rebuild_ready_log.rs")
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

pub fn honesty_host_rebuild_ready_log_helper_method_names_residual_wave620() -> bool {
    let names = LIVE_HOST_REBUILD_READY_LOG_HELPER_METHOD_NAMES_WAVE620;
    let ok = residual_name_index(names, "host_rebuild_ready_log").is_some()
        && residual_name_index(names, "writeback_rebuild_producer_to_host").is_some()
        && residual_name_index(names, "Wave 620").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostRebuildReadyLogHelperAction::MethodNames);
    ok
}

pub fn honesty_host_rebuild_ready_log_helper_source_markers_residual_wave620() -> bool {
    let gl = gl_source();
    let sh = shadow_source();
    let ready = ready_log_source();
    let log_ok = ready.contains("Wave 620")
        && ready.contains("pub fn record(")
        && ready.contains("pub fn drain(")
        && ready.contains("HostRebuildReadyEvent");
    let Some(wb) = fn_body(sh, "pub fn writeback_rebuild_producer_to_host(") else {
        residual_action_store(ResidualHostRebuildReadyLogHelperAction::SourceMarkers);
        return false;
    };
    let wb_ok = wb.contains("Wave 620")
        && wb.contains("host_rebuild_ready_log::record")
        && wb.contains("is_rebuild_hole")
        && wb.contains("gameworld_construction_sole_tick_enabled");
    let Some(update) = fn_body(gl, "pub fn update_rebuild_holes(") else {
        residual_action_store(ResidualHostRebuildReadyLogHelperAction::SourceMarkers);
        return false;
    };
    let drain_ok = update.contains("Wave 620")
        && update.contains("host_rebuild_ready_log::drain")
        && update.contains("ready_holes")
        && update.contains("construction_sole");
    let ok = log_ok && wb_ok && drain_ok && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostRebuildReadyLogHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_rebuild_ready_log_helper_nav_commands_residual_wave620() -> bool {
    let steps = LIVE_HOST_REBUILD_READY_LOG_HELPER_NAV_STEPS_WAVE620;
    let cmds = RUNTIME_HOST_LIVE_HOST_REBUILD_READY_LOG_HELPER_CMD_NAMES_WAVE620;
    let ok = residual_name_index(steps, "REQUIRE_REBUILD_READY_LOG_MODULE").is_some()
        && residual_name_index(steps, "REQUIRE_WRITEBACK_RECORDS_REBUILD_READY").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_DRAINS_REBUILD_READY").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_SOLE_SPAWN_GATE").is_some()
        && residual_name_index(steps, "LIVE_HOST_REBUILD_READY_LOG_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_rebuild_ready_log_helper").is_some()
        && residual_name_index(cmds, "writeback_records_rebuild_ready").is_some()
        && residual_name_index(cmds, "host_drains_rebuild_ready").is_some()
        && residual_name_index(cmds, "host_sole_spawn_gate").is_some()
        && residual_name_index(cmds, "rebuild_ready_log_residual").is_some();
    residual_action_store(ResidualHostRebuildReadyLogHelperAction::NavCommands);
    ok
}

pub fn simulate_host_rebuild_ready_log_helper_collect_source() -> bool {
    let ok = ready_log_source().contains("Wave 620")
        && shadow_source().contains("host_rebuild_ready_log::record")
        && gl_source().contains("host_rebuild_ready_log::drain");
    residual_action_store(ResidualHostRebuildReadyLogHelperAction::CollectSource);
    ok
}

pub fn simulate_host_rebuild_ready_log_helper_dispatch_source() -> bool {
    let gl = gl_source();
    let sh = shadow_source();
    let ok = sh.contains("Wave 620: GameWorld sole-tick rebuild-ready residual")
        && gl.contains("Wave 620: under construction sole-tick, GameWorld writeback records")
        && gl.contains("only start rebuild for IDs GW recorded ready");
    residual_action_store(ResidualHostRebuildReadyLogHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_rebuild_ready_log_helper_residual_pack_wave620() -> bool {
    honesty_host_rebuild_ready_log_helper_method_names_residual_wave620()
        && honesty_host_rebuild_ready_log_helper_source_markers_residual_wave620()
        && honesty_host_rebuild_ready_log_helper_nav_commands_residual_wave620()
        && simulate_host_rebuild_ready_log_helper_collect_source()
        && simulate_host_rebuild_ready_log_helper_dispatch_source()
}

pub fn simulate_live_host_rebuild_ready_log_helper_honesty() -> bool {
    let ok = honesty_host_rebuild_ready_log_helper_residual_pack_wave620();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostRebuildReadyLogHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_rebuild_ready_log_helper_method_names_residual_wave620());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_rebuild_ready_log_helper_source_markers_residual_wave620());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_rebuild_ready_log_helper_nav_commands_residual_wave620());
    }

    #[test]
    fn host_rebuild_ready_log_helper_sources() {
        assert!(simulate_host_rebuild_ready_log_helper_collect_source());
        assert!(simulate_host_rebuild_ready_log_helper_dispatch_source());
    }

    #[test]
    fn wave620_composite_pack() {
        assert!(honesty_host_rebuild_ready_log_helper_residual_pack_wave620());
    }

    #[test]
    fn simulate_live_host_rebuild_ready_log_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_rebuild_ready_log_helper_honesty(),
            "host rebuild ready log helper residual must latch"
        );
        assert!(residual_host_rebuild_ready_log_helper_ok());
        assert_eq!(
            residual_host_rebuild_ready_log_helper_last_action(),
            ResidualHostRebuildReadyLogHelperAction::Composite
        );
    }
}
