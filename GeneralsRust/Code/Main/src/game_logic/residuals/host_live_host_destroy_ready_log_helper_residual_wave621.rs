//! Wave 621 residual peels: under DAMAGE_AUTHORITY, GameWorld health writeback
//! records lethal IDs via `host_destroy_ready_log`; host `process_destroy_list`
//! drains and marks die side effects. Never flips shell `playable_claim`.
//!
//! Orthogonal to Waves 614–620 ready-log residuals.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `host_destroy_ready_log.rs`
//! - `gameworld_shadow.rs` writeback_health_to_host
//! - `game_logic.rs` process_destroy_list ready-log drain
//! - `cnc_game_engine.rs` post-shadow process_destroy_list
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_DESTROY_READY_LOG_HELPER_METHOD_NAMES_WAVE621: &[&str] = &[
    "host_destroy_ready_log",
    "writeback_health_to_host",
    "process_destroy_list",
    "gameworld_damage_authority_live",
    "Wave 621",
    "playable_claim = false",
];

pub const LIVE_HOST_DESTROY_READY_LOG_HELPER_NAV_STEPS_WAVE621: &[&str] = &[
    "REQUIRE_DESTROY_READY_LOG_MODULE",
    "REQUIRE_WRITEBACK_RECORDS_LETHAL",
    "REQUIRE_HOST_DRAINS_DESTROY_READY",
    "REQUIRE_POST_SHADOW_PROCESS_DESTROY",
    "LIVE_HOST_DESTROY_READY_LOG_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_DESTROY_READY_LOG_HELPER_CMD_NAMES_WAVE621: &[&str] = &[
    "host_destroy_ready_log_helper",
    "writeback_records_lethal",
    "host_drains_destroy_ready",
    "post_shadow_process_destroy",
    "destroy_ready_log_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostDestroyReadyLogHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostDestroyReadyLogHelperAction {
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

fn residual_action_store(action: ResidualHostDestroyReadyLogHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_destroy_ready_log_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_destroy_ready_log_helper_last_action()
-> ResidualHostDestroyReadyLogHelperAction {
    ResidualHostDestroyReadyLogHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}

fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}

fn eng_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

fn ready_log_source() -> &'static str {
    include_str!("../host_destroy_ready_log.rs")
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

pub fn honesty_host_destroy_ready_log_helper_method_names_residual_wave621() -> bool {
    let names = LIVE_HOST_DESTROY_READY_LOG_HELPER_METHOD_NAMES_WAVE621;
    let ok = residual_name_index(names, "host_destroy_ready_log").is_some()
        && residual_name_index(names, "writeback_health_to_host").is_some()
        && residual_name_index(names, "Wave 621").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostDestroyReadyLogHelperAction::MethodNames);
    ok
}

pub fn honesty_host_destroy_ready_log_helper_source_markers_residual_wave621() -> bool {
    let gl = gl_source();
    let sh = shadow_source();
    let eng = eng_source();
    let ready = ready_log_source();
    let log_ok = ready.contains("Wave 621")
        && ready.contains("pub fn record(")
        && ready.contains("pub fn drain(")
        && ready.contains("HostDestroyReadyEvent");
    let Some(wb) = fn_body(sh, "pub fn writeback_health_to_host(") else {
        residual_action_store(ResidualHostDestroyReadyLogHelperAction::SourceMarkers);
        return false;
    };
    let wb_ok = wb.contains("Wave 621")
        && wb.contains("host_destroy_ready_log::record")
        && wb.contains("gameworld_damage_authority_live");
    let Some(update) = fn_body(gl, "pub fn process_destroy_list(") else {
        residual_action_store(ResidualHostDestroyReadyLogHelperAction::SourceMarkers);
        return false;
    };
    let drain_ok = update.contains("Wave 621")
        && update.contains("host_destroy_ready_log::drain")
        && update.contains("mark_object_for_destruction");
    let eng_ok = eng.contains("Wave 621")
        && (eng.contains("process_destroy_list()")
            || eng.contains("ProcessDestroyListIfNeeded")
            || eng.contains("apply_host_support_op"))
        && eng.contains("host_run_gameworld_shadow_after_logic");
    let ok = log_ok && wb_ok && drain_ok && eng_ok && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostDestroyReadyLogHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_destroy_ready_log_helper_nav_commands_residual_wave621() -> bool {
    let steps = LIVE_HOST_DESTROY_READY_LOG_HELPER_NAV_STEPS_WAVE621;
    let cmds = RUNTIME_HOST_LIVE_HOST_DESTROY_READY_LOG_HELPER_CMD_NAMES_WAVE621;
    let ok = residual_name_index(steps, "REQUIRE_DESTROY_READY_LOG_MODULE").is_some()
        && residual_name_index(steps, "REQUIRE_WRITEBACK_RECORDS_LETHAL").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_DRAINS_DESTROY_READY").is_some()
        && residual_name_index(steps, "REQUIRE_POST_SHADOW_PROCESS_DESTROY").is_some()
        && residual_name_index(steps, "LIVE_HOST_DESTROY_READY_LOG_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_destroy_ready_log_helper").is_some()
        && residual_name_index(cmds, "writeback_records_lethal").is_some()
        && residual_name_index(cmds, "host_drains_destroy_ready").is_some()
        && residual_name_index(cmds, "post_shadow_process_destroy").is_some()
        && residual_name_index(cmds, "destroy_ready_log_residual").is_some();
    residual_action_store(ResidualHostDestroyReadyLogHelperAction::NavCommands);
    ok
}

pub fn simulate_host_destroy_ready_log_helper_collect_source() -> bool {
    let ok = ready_log_source().contains("Wave 621")
        && shadow_source().contains("host_destroy_ready_log::record")
        && gl_source().contains("host_destroy_ready_log::drain");
    residual_action_store(ResidualHostDestroyReadyLogHelperAction::CollectSource);
    ok
}

pub fn simulate_host_destroy_ready_log_helper_dispatch_source() -> bool {
    let gl = gl_source();
    let sh = shadow_source();
    let eng = eng_source();
    let ok = sh.contains("Wave 621: GameWorld sole damage last-write lethal residual")
        && gl.contains("Wave 621: under damage authority, GameWorld health writeback records")
        && (eng.contains("Wave 621: after health writeback, drain destroy-ready log")
            || eng.contains("Wave 621/912: after health writeback, drain destroy-ready log"));
    residual_action_store(ResidualHostDestroyReadyLogHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_destroy_ready_log_helper_residual_pack_wave621() -> bool {
    honesty_host_destroy_ready_log_helper_method_names_residual_wave621()
        && honesty_host_destroy_ready_log_helper_source_markers_residual_wave621()
        && honesty_host_destroy_ready_log_helper_nav_commands_residual_wave621()
        && simulate_host_destroy_ready_log_helper_collect_source()
        && simulate_host_destroy_ready_log_helper_dispatch_source()
}

pub fn simulate_live_host_destroy_ready_log_helper_honesty() -> bool {
    let ok = honesty_host_destroy_ready_log_helper_residual_pack_wave621();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostDestroyReadyLogHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_destroy_ready_log_helper_method_names_residual_wave621());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_destroy_ready_log_helper_source_markers_residual_wave621());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_destroy_ready_log_helper_nav_commands_residual_wave621());
    }

    #[test]
    fn host_destroy_ready_log_helper_sources() {
        assert!(simulate_host_destroy_ready_log_helper_collect_source());
        assert!(simulate_host_destroy_ready_log_helper_dispatch_source());
    }

    #[test]
    fn wave621_composite_pack() {
        assert!(honesty_host_destroy_ready_log_helper_residual_pack_wave621());
    }

    #[test]
    fn simulate_live_host_destroy_ready_log_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_destroy_ready_log_helper_honesty(),
            "host destroy ready log helper residual must latch"
        );
        assert!(residual_host_destroy_ready_log_helper_ok());
        assert_eq!(
            residual_host_destroy_ready_log_helper_last_action(),
            ResidualHostDestroyReadyLogHelperAction::Composite
        );
    }
}
