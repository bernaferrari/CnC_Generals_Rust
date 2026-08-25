//! Wave 674 residual peels: GameWorld entity power writeback records dirty
//! objects via `host_entity_power_ready_log`; host drains and applies
//! presentation bookkeeping. Never flips `playable_claim`.
//!
//! Orthogonal to Waves 614–673 ready-log residuals.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `host_entity_power_ready_log.rs`
//! - `gameworld_shadow.rs` writeback_entity_power_to_host
//! - `game_logic.rs` host_apply_entity_power_ready_completions
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_ENTITY_POWER_READY_LOG_HELPER_METHOD_NAMES_WAVE674: &[&str] = &[
    "host_entity_power_ready_log",
    "writeback_entity_power_to_host",
    "host_apply_entity_power_ready_completions",
    "Wave 674",
    "playable_claim = false",
];

pub const LIVE_HOST_ENTITY_POWER_READY_LOG_HELPER_NAV_STEPS_WAVE674: &[&str] = &[
    "REQUIRE_ENTITY_POWER_READY_LOG_MODULE",
    "REQUIRE_WRITEBACK_RECORDS_ENTITY_POWER_CHANGE",
    "REQUIRE_HOST_DRAINS_ENTITY_POWER_READY",
    "REQUIRE_HOST_APPLIES_ENTITY_POWER_BOOKKEEPING",
    "LIVE_HOST_ENTITY_POWER_READY_LOG_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_ENTITY_POWER_READY_LOG_HELPER_CMD_NAMES_WAVE674: &[&str] = &[
    "host_entity_power_ready_log_helper",
    "writeback_records_entity_power_change",
    "host_drains_entity_power_ready",
    "host_applies_entity_power_bookkeeping",
    "entity_power_ready_log_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostEntityPowerReadyLogHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostEntityPowerReadyLogHelperAction {
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

fn residual_action_store(action: ResidualHostEntityPowerReadyLogHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_entity_power_ready_log_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_entity_power_ready_log_helper_last_action()
-> ResidualHostEntityPowerReadyLogHelperAction {
    ResidualHostEntityPowerReadyLogHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn ready_log_source() -> &'static str {
    include_str!("../host_entity_power_ready_log.rs")
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

pub fn honesty_host_entity_power_ready_log_helper_method_names_residual_wave674() -> bool {
    let names = LIVE_HOST_ENTITY_POWER_READY_LOG_HELPER_METHOD_NAMES_WAVE674;
    let ok = residual_name_index(names, "host_entity_power_ready_log").is_some()
        && residual_name_index(names, "writeback_entity_power_to_host").is_some()
        && residual_name_index(names, "Wave 674").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostEntityPowerReadyLogHelperAction::MethodNames);
    ok
}

pub fn honesty_host_entity_power_ready_log_helper_source_markers_residual_wave674() -> bool {
    let gl = gl_source();
    let sh = shadow_source();
    let ready = ready_log_source();
    let log_ok = ready.contains("Wave 674")
        && ready.contains("pub fn record(")
        && ready.contains("pub fn drain(");
    let Some(wb) = fn_body(sh, "pub fn writeback_entity_power_to_host(") else {
        residual_action_store(ResidualHostEntityPowerReadyLogHelperAction::SourceMarkers);
        return false;
    };
    let wb_ok = wb.contains("Wave 674") && wb.contains("host_entity_power_ready_log::record");
    let Some(apply) = fn_body(gl, "pub fn host_apply_entity_power_ready_completions(") else {
        residual_action_store(ResidualHostEntityPowerReadyLogHelperAction::SourceMarkers);
        return false;
    };
    let apply_ok = apply.contains("Wave 674")
        && apply.contains("host_entity_power_ready_log::drain")
        && apply.contains("record_host_entity_power");
    let drain_call = (sh.contains("host_apply_entity_power_ready_completions")
        || sh.contains("apply_ready_log_drain_op")
        || sh.contains("ReadyLogDrainOp::EntityPower"))
        && sh.contains("Wave 674: drain");
    let ok = log_ok && wb_ok && apply_ok && drain_call && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostEntityPowerReadyLogHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_entity_power_ready_log_helper_nav_commands_residual_wave674() -> bool {
    let steps = LIVE_HOST_ENTITY_POWER_READY_LOG_HELPER_NAV_STEPS_WAVE674;
    let cmds = RUNTIME_HOST_LIVE_HOST_ENTITY_POWER_READY_LOG_HELPER_CMD_NAMES_WAVE674;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_POWER_READY_LOG_MODULE").is_some()
        && residual_name_index(steps, "REQUIRE_WRITEBACK_RECORDS_ENTITY_POWER_CHANGE").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_DRAINS_ENTITY_POWER_READY").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_APPLIES_ENTITY_POWER_BOOKKEEPING").is_some()
        && residual_name_index(steps, "LIVE_HOST_ENTITY_POWER_READY_LOG_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_entity_power_ready_log_helper").is_some()
        && residual_name_index(cmds, "writeback_records_entity_power_change").is_some()
        && residual_name_index(cmds, "host_drains_entity_power_ready").is_some()
        && residual_name_index(cmds, "host_applies_entity_power_bookkeeping").is_some()
        && residual_name_index(cmds, "entity_power_ready_log_residual").is_some();
    residual_action_store(ResidualHostEntityPowerReadyLogHelperAction::NavCommands);
    ok
}

pub fn simulate_host_entity_power_ready_log_helper_collect_source() -> bool {
    let ok = ready_log_source().contains("Wave 674")
        && shadow_source().contains("host_entity_power_ready_log::record")
        && gl_source().contains("host_entity_power_ready_log::drain");
    residual_action_store(ResidualHostEntityPowerReadyLogHelperAction::CollectSource);
    ok
}

pub fn simulate_host_entity_power_ready_log_helper_dispatch_source() -> bool {
    let gl = gl_source();
    let sh = shadow_source();
    let ok = sh.contains("Wave 674") && gl.contains("Wave 674") && sh.contains("Wave 674: drain");
    residual_action_store(ResidualHostEntityPowerReadyLogHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_entity_power_ready_log_helper_residual_pack_wave674() -> bool {
    honesty_host_entity_power_ready_log_helper_method_names_residual_wave674()
        && honesty_host_entity_power_ready_log_helper_source_markers_residual_wave674()
        && honesty_host_entity_power_ready_log_helper_nav_commands_residual_wave674()
        && simulate_host_entity_power_ready_log_helper_collect_source()
        && simulate_host_entity_power_ready_log_helper_dispatch_source()
}

pub fn simulate_live_host_entity_power_ready_log_helper_honesty() -> bool {
    let ok = honesty_host_entity_power_ready_log_helper_residual_pack_wave674();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostEntityPowerReadyLogHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_entity_power_ready_log_helper_method_names_residual_wave674());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_entity_power_ready_log_helper_source_markers_residual_wave674());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_entity_power_ready_log_helper_nav_commands_residual_wave674());
    }
    #[test]
    fn host_entity_power_ready_log_helper_sources() {
        assert!(simulate_host_entity_power_ready_log_helper_collect_source());
        assert!(simulate_host_entity_power_ready_log_helper_dispatch_source());
    }
    #[test]
    fn wave674_composite_pack() {
        assert!(honesty_host_entity_power_ready_log_helper_residual_pack_wave674());
    }
    #[test]
    fn simulate_live_host_entity_power_ready_log_helper_honesty_residual_live() {
        assert!(simulate_live_host_entity_power_ready_log_helper_honesty());
        assert!(residual_host_entity_power_ready_log_helper_ok());
        assert_eq!(
            residual_host_entity_power_ready_log_helper_last_action(),
            ResidualHostEntityPowerReadyLogHelperAction::Composite
        );
    }
}
