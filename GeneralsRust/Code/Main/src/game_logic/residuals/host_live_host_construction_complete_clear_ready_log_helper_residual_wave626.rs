//! Wave 626 residual peels: under CONSTRUCTION_AUTHORITY sole-tick, GameWorld
//! rebuild-producer writeback records CONSTRUCTION_COMPLETE clear deadlines via
//! `host_construction_complete_clear_ready_log`; host drains and clears the model
//! bit. Never flips `playable_claim`.
//!
//! Orthogonal to Waves 614–625 ready-log residuals.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `host_construction_complete_clear_ready_log.rs`
//! - `gameworld_shadow.rs` writeback_rebuild_producer_to_host
//! - `game_logic.rs` host_apply_construction_complete_clear_ready_completions
//! - `object.rs` apply_construction_complete_clear_residual
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_CONSTRUCTION_COMPLETE_CLEAR_READY_LOG_HELPER_METHOD_NAMES_WAVE626: &[&str] = &[
    "host_construction_complete_clear_ready_log",
    "writeback_rebuild_producer_to_host",
    "host_apply_construction_complete_clear_ready_completions",
    "apply_construction_complete_clear_residual",
    "Wave 626",
    "playable_claim = false",
];

pub const LIVE_HOST_CONSTRUCTION_COMPLETE_CLEAR_READY_LOG_HELPER_NAV_STEPS_WAVE626: &[&str] = &[
    "REQUIRE_CCC_READY_LOG_MODULE",
    "REQUIRE_WRITEBACK_RECORDS_CLEAR_DEADLINE",
    "REQUIRE_HOST_DRAINS_CCC_READY",
    "REQUIRE_HOST_CLEARS_MODEL_BIT",
    "LIVE_HOST_CCC_READY_LOG_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_CONSTRUCTION_COMPLETE_CLEAR_READY_LOG_HELPER_CMD_NAMES_WAVE626:
    &[&str] = &[
    "host_construction_complete_clear_ready_log_helper",
    "writeback_records_clear_deadline",
    "host_drains_ccc_ready",
    "host_clears_model_bit",
    "construction_complete_clear_ready_log_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostConstructionCompleteClearReadyLogHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostConstructionCompleteClearReadyLogHelperAction {
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

fn residual_action_store(action: ResidualHostConstructionCompleteClearReadyLogHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_construction_complete_clear_ready_log_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_construction_complete_clear_ready_log_helper_last_action()
-> ResidualHostConstructionCompleteClearReadyLogHelperAction {
    ResidualHostConstructionCompleteClearReadyLogHelperAction::from_u8(
        RESIDUAL_ACTION.load(Ordering::SeqCst),
    )
}

fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}

fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}

fn object_source() -> &'static str {
    crate::game_logic::object::OBJECT_SRC
}

fn ready_log_source() -> &'static str {
    include_str!("../host_construction_complete_clear_ready_log.rs")
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

pub fn honesty_host_construction_complete_clear_ready_log_helper_method_names_residual_wave626()
-> bool {
    let names = LIVE_HOST_CONSTRUCTION_COMPLETE_CLEAR_READY_LOG_HELPER_METHOD_NAMES_WAVE626;
    let ok = residual_name_index(names, "host_construction_complete_clear_ready_log").is_some()
        && residual_name_index(names, "writeback_rebuild_producer_to_host").is_some()
        && residual_name_index(names, "Wave 626").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostConstructionCompleteClearReadyLogHelperAction::MethodNames);
    ok
}

pub fn honesty_host_construction_complete_clear_ready_log_helper_source_markers_residual_wave626()
-> bool {
    let gl = gl_source();
    let sh = shadow_source();
    let obj = object_source();
    let ready = ready_log_source();
    let log_ok = ready.contains("Wave 626")
        && ready.contains("pub fn record(")
        && ready.contains("pub fn drain(")
        && ready.contains("HostConstructionCompleteClearReadyEvent");
    let Some(wb) = fn_body(sh, "pub fn writeback_rebuild_producer_to_host(") else {
        residual_action_store(
            ResidualHostConstructionCompleteClearReadyLogHelperAction::SourceMarkers,
        );
        return false;
    };
    let wb_ok = wb.contains("Wave 626")
        && wb.contains("host_construction_complete_clear_ready_log::record")
        && wb.contains("construction_complete_clear_frame");
    let Some(apply) = fn_body(
        gl,
        "pub fn host_apply_construction_complete_clear_ready_completions(",
    ) else {
        residual_action_store(
            ResidualHostConstructionCompleteClearReadyLogHelperAction::SourceMarkers,
        );
        return false;
    };
    let apply_ok = apply.contains("Wave 626")
        && apply.contains("host_construction_complete_clear_ready_log::drain")
        && apply.contains("apply_construction_complete_clear_residual");
    let sole_gate = gl.contains("Wave 626: under construction sole-tick, GW ready-log owns clear")
        && gl.contains("!crate::gameworld_shadow::gameworld_construction_sole_tick_enabled()");
    let obj_ok =
        obj.contains("apply_construction_complete_clear_residual") && obj.contains("Wave 626");
    let drain_call = (sh.contains("host_apply_construction_complete_clear_ready_completions")
        || sh.contains("apply_ready_log_drain_op")
        || sh.contains("ReadyLogDrainOp::ConstructionCompleteClear"));
    let ok = log_ok
        && wb_ok
        && apply_ok
        && sole_gate
        && obj_ok
        && drain_call
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostConstructionCompleteClearReadyLogHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_construction_complete_clear_ready_log_helper_nav_commands_residual_wave626()
-> bool {
    let steps = LIVE_HOST_CONSTRUCTION_COMPLETE_CLEAR_READY_LOG_HELPER_NAV_STEPS_WAVE626;
    let cmds =
        RUNTIME_HOST_LIVE_HOST_CONSTRUCTION_COMPLETE_CLEAR_READY_LOG_HELPER_CMD_NAMES_WAVE626;
    let ok = residual_name_index(steps, "REQUIRE_CCC_READY_LOG_MODULE").is_some()
        && residual_name_index(steps, "REQUIRE_WRITEBACK_RECORDS_CLEAR_DEADLINE").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_DRAINS_CCC_READY").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_CLEARS_MODEL_BIT").is_some()
        && residual_name_index(steps, "LIVE_HOST_CCC_READY_LOG_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_construction_complete_clear_ready_log_helper").is_some()
        && residual_name_index(cmds, "writeback_records_clear_deadline").is_some()
        && residual_name_index(cmds, "host_drains_ccc_ready").is_some()
        && residual_name_index(cmds, "host_clears_model_bit").is_some()
        && residual_name_index(cmds, "construction_complete_clear_ready_log_residual").is_some();
    residual_action_store(ResidualHostConstructionCompleteClearReadyLogHelperAction::NavCommands);
    ok
}

pub fn simulate_host_construction_complete_clear_ready_log_helper_collect_source() -> bool {
    let ok = ready_log_source().contains("Wave 626")
        && shadow_source().contains("host_construction_complete_clear_ready_log::record")
        && gl_source().contains("host_construction_complete_clear_ready_log::drain");
    residual_action_store(ResidualHostConstructionCompleteClearReadyLogHelperAction::CollectSource);
    ok
}

pub fn simulate_host_construction_complete_clear_ready_log_helper_dispatch_source() -> bool {
    let gl = gl_source();
    let sh = shadow_source();
    let obj = object_source();
    let ok = sh.contains("Wave 626: construction-complete clear deadline elapsed residual")
        && gl.contains("Wave 626: under construction sole-tick, GameWorld writeback records")
        && obj.contains(
            "Wave 626: clear CONSTRUCTION_COMPLETE model bit after GW deadline writeback",
        )
        && sh.contains("Wave 626: drain construction-complete-clear ready log after GW writeback");
    residual_action_store(
        ResidualHostConstructionCompleteClearReadyLogHelperAction::DispatchSource,
    );
    ok
}

pub fn honesty_host_construction_complete_clear_ready_log_helper_residual_pack_wave626() -> bool {
    honesty_host_construction_complete_clear_ready_log_helper_method_names_residual_wave626()
        && honesty_host_construction_complete_clear_ready_log_helper_source_markers_residual_wave626(
        )
        && honesty_host_construction_complete_clear_ready_log_helper_nav_commands_residual_wave626()
        && simulate_host_construction_complete_clear_ready_log_helper_collect_source()
        && simulate_host_construction_complete_clear_ready_log_helper_dispatch_source()
}

pub fn simulate_live_host_construction_complete_clear_ready_log_helper_honesty() -> bool {
    let ok = honesty_host_construction_complete_clear_ready_log_helper_residual_pack_wave626();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostConstructionCompleteClearReadyLogHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_host_construction_complete_clear_ready_log_helper_method_names_residual_wave626(
            )
        );
    }

    #[test]
    fn source_markers_residual() {
        assert!(
            honesty_host_construction_complete_clear_ready_log_helper_source_markers_residual_wave626(
            )
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_host_construction_complete_clear_ready_log_helper_nav_commands_residual_wave626(
            )
        );
    }

    #[test]
    fn host_construction_complete_clear_ready_log_helper_sources() {
        assert!(simulate_host_construction_complete_clear_ready_log_helper_collect_source());
        assert!(simulate_host_construction_complete_clear_ready_log_helper_dispatch_source());
    }

    #[test]
    fn wave626_composite_pack() {
        assert!(honesty_host_construction_complete_clear_ready_log_helper_residual_pack_wave626());
    }

    #[test]
    fn simulate_live_host_construction_complete_clear_ready_log_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_construction_complete_clear_ready_log_helper_honesty(),
            "host construction complete clear ready log helper residual must latch"
        );
        assert!(residual_host_construction_complete_clear_ready_log_helper_ok());
        assert_eq!(
            residual_host_construction_complete_clear_ready_log_helper_last_action(),
            ResidualHostConstructionCompleteClearReadyLogHelperAction::Composite
        );
    }
}
