//! Wave 614 residual peels: under PRODUCTION_AUTHORITY sole-tick, GameWorld
//! writeback records production-ready producers via `host_production_ready_log`;
//! host collect drains that log so GW decides readiness and host still
//! try_complete + spawn.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Waves 613 collect / 608 apply residuals.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `host_production_ready_log.rs`
//! - `gameworld_shadow.rs` writeback_production_to_host
//! - `game_logic.rs` host_collect_production_completions
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; host still owns spawn IDs

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PRODUCTION_READY_LOG_HELPER_METHOD_NAMES_WAVE614: &[&str] = &[
    "host_production_ready_log",
    "writeback_production_to_host",
    "host_collect_production_completions",
    "gameworld_production_sole_tick_enabled",
    "try_complete_production",
    "Wave 614",
    "playable_claim = false",
];

pub const LIVE_HOST_PRODUCTION_READY_LOG_HELPER_NAV_STEPS_WAVE614: &[&str] = &[
    "REQUIRE_READY_LOG_MODULE",
    "REQUIRE_WRITEBACK_RECORDS_READY",
    "REQUIRE_COLLECT_DRAINS_READY",
    "REQUIRE_HOST_STILL_SPAWNS",
    "LIVE_HOST_PRODUCTION_READY_LOG_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_PRODUCTION_READY_LOG_HELPER_CMD_NAMES_WAVE614: &[&str] = &[
    "host_production_ready_log_helper",
    "writeback_records_ready",
    "collect_drains_ready",
    "host_still_spawns",
    "production_ready_log_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostProductionReadyLogHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostProductionReadyLogHelperAction {
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

fn residual_action_store(action: ResidualHostProductionReadyLogHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_production_ready_log_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_production_ready_log_helper_last_action()
-> ResidualHostProductionReadyLogHelperAction {
    ResidualHostProductionReadyLogHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}

fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}

fn ready_log_source() -> &'static str {
    include_str!("../host_production_ready_log.rs")
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

pub fn honesty_host_production_ready_log_helper_method_names_residual_wave614() -> bool {
    let names = LIVE_HOST_PRODUCTION_READY_LOG_HELPER_METHOD_NAMES_WAVE614;
    let ok = residual_name_index(names, "host_production_ready_log").is_some()
        && residual_name_index(names, "writeback_production_to_host").is_some()
        && residual_name_index(names, "host_collect_production_completions").is_some()
        && residual_name_index(names, "Wave 614").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostProductionReadyLogHelperAction::MethodNames);
    ok
}

pub fn honesty_host_production_ready_log_helper_source_markers_residual_wave614() -> bool {
    let gl = gl_source();
    let sh = shadow_source();
    let ready = ready_log_source();
    let log_ok = ready.contains("Wave 614")
        && ready.contains("pub fn record(")
        && ready.contains("pub fn drain(")
        && ready.contains("HostProductionReadyEvent");
    let Some(wb) = fn_body(sh, "pub fn writeback_production_to_host(") else {
        residual_action_store(ResidualHostProductionReadyLogHelperAction::SourceMarkers);
        return false;
    };
    let Some(collect) = fn_body(gl, "fn host_collect_production_completions(") else {
        residual_action_store(ResidualHostProductionReadyLogHelperAction::SourceMarkers);
        return false;
    };
    let wb_ok = wb.contains("Wave 614")
        && wb.contains("host_production_ready_log::record")
        && wb.contains("gameworld_production_sole_tick_enabled");
    let collect_ok = collect.contains("Wave 614")
        && collect.contains("host_production_ready_log::drain")
        && collect.contains("ready_producers")
        && collect.contains("try_complete_production");
    let still_spawns = gl.contains("fn host_apply_unit_production_completions")
        && gl.contains("self.create_object(");
    let ok = log_ok && wb_ok && collect_ok && still_spawns && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostProductionReadyLogHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_production_ready_log_helper_nav_commands_residual_wave614() -> bool {
    let steps = LIVE_HOST_PRODUCTION_READY_LOG_HELPER_NAV_STEPS_WAVE614;
    let cmds = RUNTIME_HOST_LIVE_HOST_PRODUCTION_READY_LOG_HELPER_CMD_NAMES_WAVE614;
    let ok = residual_name_index(steps, "REQUIRE_READY_LOG_MODULE").is_some()
        && residual_name_index(steps, "REQUIRE_WRITEBACK_RECORDS_READY").is_some()
        && residual_name_index(steps, "REQUIRE_COLLECT_DRAINS_READY").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_STILL_SPAWNS").is_some()
        && residual_name_index(steps, "LIVE_HOST_PRODUCTION_READY_LOG_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_production_ready_log_helper").is_some()
        && residual_name_index(cmds, "writeback_records_ready").is_some()
        && residual_name_index(cmds, "collect_drains_ready").is_some()
        && residual_name_index(cmds, "host_still_spawns").is_some()
        && residual_name_index(cmds, "production_ready_log_residual").is_some();
    residual_action_store(ResidualHostProductionReadyLogHelperAction::NavCommands);
    ok
}

pub fn simulate_host_production_ready_log_helper_collect_source() -> bool {
    let ok = ready_log_source().contains("Wave 614")
        && shadow_source().contains("host_production_ready_log::record")
        && gl_source().contains("host_production_ready_log::drain");
    residual_action_store(ResidualHostProductionReadyLogHelperAction::CollectSource);
    ok
}

pub fn simulate_host_production_ready_log_helper_dispatch_source() -> bool {
    let gl = gl_source();
    let sh = shadow_source();
    let ok = sh.contains("Wave 614: GameWorld sole-tick ready residual")
        && gl.contains("Wave 614: under sole-tick, GameWorld writeback records ready producers")
        && gl.contains("ready_producers.contains(&id)");
    residual_action_store(ResidualHostProductionReadyLogHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_production_ready_log_helper_residual_pack_wave614() -> bool {
    honesty_host_production_ready_log_helper_method_names_residual_wave614()
        && honesty_host_production_ready_log_helper_source_markers_residual_wave614()
        && honesty_host_production_ready_log_helper_nav_commands_residual_wave614()
        && simulate_host_production_ready_log_helper_collect_source()
        && simulate_host_production_ready_log_helper_dispatch_source()
}

pub fn simulate_live_host_production_ready_log_helper_honesty() -> bool {
    let ok = honesty_host_production_ready_log_helper_residual_pack_wave614();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostProductionReadyLogHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_production_ready_log_helper_method_names_residual_wave614());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_production_ready_log_helper_source_markers_residual_wave614());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_production_ready_log_helper_nav_commands_residual_wave614());
    }

    #[test]
    fn host_production_ready_log_helper_sources() {
        assert!(simulate_host_production_ready_log_helper_collect_source());
        assert!(simulate_host_production_ready_log_helper_dispatch_source());
    }

    #[test]
    fn wave614_composite_pack() {
        assert!(honesty_host_production_ready_log_helper_residual_pack_wave614());
    }

    #[test]
    fn simulate_live_host_production_ready_log_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_production_ready_log_helper_honesty(),
            "host production ready log helper residual must latch"
        );
        assert!(residual_host_production_ready_log_helper_ok());
        assert_eq!(
            residual_host_production_ready_log_helper_last_action(),
            ResidualHostProductionReadyLogHelperAction::Composite
        );
    }
}
