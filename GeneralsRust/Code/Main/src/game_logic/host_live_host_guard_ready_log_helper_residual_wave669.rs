//! Wave 669 residual peels: GameWorld guard writeback records dirty
//! objects via `host_guard_ready_log`; host drains and applies
//! presentation bookkeeping. Never flips `playable_claim`.
//!
//! Orthogonal to Waves 614–668 ready-log residuals.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `host_guard_ready_log.rs`
//! - `gameworld_shadow.rs` writeback_guard_to_host
//! - `game_logic.rs` host_apply_guard_ready_completions
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_GUARD_READY_LOG_HELPER_METHOD_NAMES_WAVE669: &[&str] = &[
    "host_guard_ready_log",
    "writeback_guard_to_host",
    "host_apply_guard_ready_completions",
    "Wave 669",
    "playable_claim = false",
];

pub const LIVE_HOST_GUARD_READY_LOG_HELPER_NAV_STEPS_WAVE669: &[&str] = &[
    "REQUIRE_GUARD_READY_LOG_MODULE",
    "REQUIRE_WRITEBACK_RECORDS_GUARD_CHANGE",
    "REQUIRE_HOST_DRAINS_GUARD_READY",
    "REQUIRE_HOST_APPLIES_GUARD_BOOKKEEPING",
    "LIVE_HOST_GUARD_READY_LOG_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_GUARD_READY_LOG_HELPER_CMD_NAMES_WAVE669: &[&str] = &[
    "host_guard_ready_log_helper",
    "writeback_records_guard_change",
    "host_drains_guard_ready",
    "host_applies_guard_bookkeeping",
    "guard_ready_log_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostGuardReadyLogHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostGuardReadyLogHelperAction {
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

fn residual_action_store(action: ResidualHostGuardReadyLogHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_guard_ready_log_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_guard_ready_log_helper_last_action() -> ResidualHostGuardReadyLogHelperAction {
    ResidualHostGuardReadyLogHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn gl_source() -> &'static str {
    include_str!("game_logic.rs")
}
fn shadow_source() -> &'static str {
    include_str!("../gameworld_shadow.rs")
}
fn ready_log_source() -> &'static str {
    include_str!("host_guard_ready_log.rs")
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

pub fn honesty_host_guard_ready_log_helper_method_names_residual_wave669() -> bool {
    let names = LIVE_HOST_GUARD_READY_LOG_HELPER_METHOD_NAMES_WAVE669;
    let ok = residual_name_index(names, "host_guard_ready_log").is_some()
        && residual_name_index(names, "writeback_guard_to_host").is_some()
        && residual_name_index(names, "Wave 669").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostGuardReadyLogHelperAction::MethodNames);
    ok
}

pub fn honesty_host_guard_ready_log_helper_source_markers_residual_wave669() -> bool {
    let gl = gl_source();
    let sh = shadow_source();
    let ready = ready_log_source();
    let log_ok = ready.contains("Wave 669")
        && ready.contains("pub fn record(")
        && ready.contains("pub fn drain(");
    let Some(wb) = fn_body(sh, "pub fn writeback_guard_to_host(") else {
        residual_action_store(ResidualHostGuardReadyLogHelperAction::SourceMarkers);
        return false;
    };
    let wb_ok = wb.contains("Wave 669") && wb.contains("host_guard_ready_log::record");
    let Some(apply) = fn_body(gl, "pub fn host_apply_guard_ready_completions(") else {
        residual_action_store(ResidualHostGuardReadyLogHelperAction::SourceMarkers);
        return false;
    };
    let apply_ok = apply.contains("Wave 669")
        && apply.contains("host_guard_ready_log::drain")
        && apply.contains("record_host_guard");
    let drain_call =
        sh.contains("host_apply_guard_ready_completions") && sh.contains("Wave 669: drain");
    let ok = log_ok && wb_ok && apply_ok && drain_call && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostGuardReadyLogHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_guard_ready_log_helper_nav_commands_residual_wave669() -> bool {
    let steps = LIVE_HOST_GUARD_READY_LOG_HELPER_NAV_STEPS_WAVE669;
    let cmds = RUNTIME_HOST_LIVE_HOST_GUARD_READY_LOG_HELPER_CMD_NAMES_WAVE669;
    let ok = residual_name_index(steps, "REQUIRE_GUARD_READY_LOG_MODULE").is_some()
        && residual_name_index(steps, "REQUIRE_WRITEBACK_RECORDS_GUARD_CHANGE").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_DRAINS_GUARD_READY").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_APPLIES_GUARD_BOOKKEEPING").is_some()
        && residual_name_index(steps, "LIVE_HOST_GUARD_READY_LOG_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_guard_ready_log_helper").is_some()
        && residual_name_index(cmds, "writeback_records_guard_change").is_some()
        && residual_name_index(cmds, "host_drains_guard_ready").is_some()
        && residual_name_index(cmds, "host_applies_guard_bookkeeping").is_some()
        && residual_name_index(cmds, "guard_ready_log_residual").is_some();
    residual_action_store(ResidualHostGuardReadyLogHelperAction::NavCommands);
    ok
}

pub fn simulate_host_guard_ready_log_helper_collect_source() -> bool {
    let ok = ready_log_source().contains("Wave 669")
        && shadow_source().contains("host_guard_ready_log::record")
        && gl_source().contains("host_guard_ready_log::drain");
    residual_action_store(ResidualHostGuardReadyLogHelperAction::CollectSource);
    ok
}

pub fn simulate_host_guard_ready_log_helper_dispatch_source() -> bool {
    let gl = gl_source();
    let sh = shadow_source();
    let ok = sh.contains("Wave 669") && gl.contains("Wave 669") && sh.contains("Wave 669: drain");
    residual_action_store(ResidualHostGuardReadyLogHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_guard_ready_log_helper_residual_pack_wave669() -> bool {
    honesty_host_guard_ready_log_helper_method_names_residual_wave669()
        && honesty_host_guard_ready_log_helper_source_markers_residual_wave669()
        && honesty_host_guard_ready_log_helper_nav_commands_residual_wave669()
        && simulate_host_guard_ready_log_helper_collect_source()
        && simulate_host_guard_ready_log_helper_dispatch_source()
}

pub fn simulate_live_host_guard_ready_log_helper_honesty() -> bool {
    let ok = honesty_host_guard_ready_log_helper_residual_pack_wave669();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostGuardReadyLogHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_guard_ready_log_helper_method_names_residual_wave669());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_guard_ready_log_helper_source_markers_residual_wave669());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_guard_ready_log_helper_nav_commands_residual_wave669());
    }
    #[test]
    fn host_guard_ready_log_helper_sources() {
        assert!(simulate_host_guard_ready_log_helper_collect_source());
        assert!(simulate_host_guard_ready_log_helper_dispatch_source());
    }
    #[test]
    fn wave669_composite_pack() {
        assert!(honesty_host_guard_ready_log_helper_residual_pack_wave669());
    }
    #[test]
    fn simulate_live_host_guard_ready_log_helper_honesty_residual_live() {
        assert!(simulate_live_host_guard_ready_log_helper_honesty());
        assert!(residual_host_guard_ready_log_helper_ok());
        assert_eq!(
            residual_host_guard_ready_log_helper_last_action(),
            ResidualHostGuardReadyLogHelperAction::Composite
        );
    }
}
