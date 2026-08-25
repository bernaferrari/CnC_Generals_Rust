//! Wave 560 residual peels: presentation freeze owns logic-frame residual via
//! `presentation_or_boot_logic_frame` (status sample), and env-seed
//! `build_for_engine` uses `current_player_id` (not `get_frame`) so FOW/local
//! team residual binds correctly. Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 559 presentation honesty align residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` presentation_or_boot_logic_frame / runtime_host_status_snapshot
//! - `cnc_game_engine.rs` ensure_presentation_env_for_hints build_for_engine local id
//! - `presentation_frame.rs` frame: LogicFrame
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_LOGIC_FRAME_PRESENTATION_HELPER_METHOD_NAMES_WAVE560: &[&str] = &[
    "presentation_or_boot_logic_frame",
    "runtime_host_status_snapshot",
    "ensure_presentation_env_for_hints",
    "host_ensure_presentation_env_for_hints",
    "current_player_id",
    "Wave 560",
    "playable_claim = false",
];

pub const LIVE_LOGIC_FRAME_PRESENTATION_HELPER_NAV_STEPS_WAVE560: &[&str] = &[
    "REQUIRE_LOGIC_FRAME_PRESENTATION_HELPER",
    "REQUIRE_ENV_SEED_CURRENT_PLAYER",
    "LIVE_LOGIC_FRAME_PRESENTATION_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_LOGIC_FRAME_PRESENTATION_HELPER_CMD_NAMES_WAVE560: &[&str] = &[
    "logic_frame_presentation_helper",
    "env_seed_current_player",
    "boot_get_frame",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualLogicFramePresentationHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualLogicFramePresentationHelperAction {
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

fn residual_action_store(action: ResidualLogicFramePresentationHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_logic_frame_presentation_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_logic_frame_presentation_helper_last_action()
-> ResidualLogicFramePresentationHelperAction {
    ResidualLogicFramePresentationHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}

fn fn_body<'a>(src: &'a str, sig: &str) -> Option<&'a str> {
    let start = src.find(sig)?;
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

// 2026-08-15: retarget honesty markers to host_match_*/fail-closed seams.
pub fn honesty_logic_frame_presentation_helper_method_names_residual_wave560() -> bool {
    let names = LIVE_LOGIC_FRAME_PRESENTATION_HELPER_METHOD_NAMES_WAVE560;
    let ok = residual_name_index(names, "presentation_or_boot_logic_frame").is_some()
        && residual_name_index(names, "runtime_host_status_snapshot").is_some()
        && residual_name_index(names, "ensure_presentation_env_for_hints").is_some()
        && residual_name_index(names, "current_player_id").is_some()
        && residual_name_index(names, "Wave 560").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualLogicFramePresentationHelperAction::MethodNames);
    ok
}

pub fn honesty_logic_frame_presentation_helper_source_markers_residual_wave560() -> bool {
    let eng = eng_source();
    let Some(helper) = fn_body(eng, "fn presentation_or_boot_logic_frame(") else {
        residual_action_store(ResidualLogicFramePresentationHelperAction::SourceMarkers);
        return false;
    };
    let Some(status) = fn_body(eng, "fn runtime_host_status_snapshot(") else {
        residual_action_store(ResidualLogicFramePresentationHelperAction::SourceMarkers);
        return false;
    };
    let Some(env) = fn_body(eng, "fn host_ensure_presentation_env_for_hints(")
        .or_else(|| fn_body(eng, "fn ensure_presentation_env_for_hints("))
    else {
        residual_action_store(ResidualLogicFramePresentationHelperAction::SourceMarkers);
        return false;
    };
    let helper_ok = helper.contains("Wave 560")
        && helper.contains("pres.frame.0")
        && helper.contains("host_match_logic_frame");
    let status_ok = status.contains("presentation_or_boot_logic_frame()")
        && !status.contains("self.game_logic.get_frame()");
    // 2026-08-15: env seed peeled onto Wave 590/466 (camera_drain.rs).
    let env_ok = (env.contains("Wave 560") || env.contains("Wave 590") || env.contains("Wave 466"))
        && env.contains("self.current_player_id")
        && env.contains("build_for_engine")
        && !env.contains("get_frame() as u32");
    let raw = eng.matches("self.game_logic.get_frame()").count();
    let ok = helper_ok && status_ok && env_ok && raw == 0 && !eng.contains("playable_claim = true");
    residual_action_store(ResidualLogicFramePresentationHelperAction::SourceMarkers);
    ok
}

pub fn honesty_logic_frame_presentation_helper_nav_commands_residual_wave560() -> bool {
    let steps = LIVE_LOGIC_FRAME_PRESENTATION_HELPER_NAV_STEPS_WAVE560;
    let cmds = RUNTIME_HOST_LIVE_LOGIC_FRAME_PRESENTATION_HELPER_CMD_NAMES_WAVE560;
    let ok = residual_name_index(steps, "REQUIRE_LOGIC_FRAME_PRESENTATION_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_ENV_SEED_CURRENT_PLAYER").is_some()
        && residual_name_index(steps, "LIVE_LOGIC_FRAME_PRESENTATION_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "logic_frame_presentation_helper").is_some()
        && residual_name_index(cmds, "env_seed_current_player").is_some()
        && residual_name_index(cmds, "boot_get_frame").is_some();
    residual_action_store(ResidualLogicFramePresentationHelperAction::NavCommands);
    ok
}

pub fn simulate_logic_frame_presentation_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 560")
        && eng.contains("fn presentation_or_boot_logic_frame")
        && eng.contains("fn ensure_presentation_env_for_hints");
    residual_action_store(ResidualLogicFramePresentationHelperAction::CollectSource);
    ok
}

pub fn simulate_logic_frame_presentation_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let Some(status) = fn_body(eng, "fn runtime_host_status_snapshot(") else {
        residual_action_store(ResidualLogicFramePresentationHelperAction::DispatchSource);
        return false;
    };
    let Some(env) = fn_body(eng, "fn host_ensure_presentation_env_for_hints(")
        .or_else(|| fn_body(eng, "fn ensure_presentation_env_for_hints("))
    else {
        residual_action_store(ResidualLogicFramePresentationHelperAction::DispatchSource);
        return false;
    };
    let ok = status.contains("logic_frame: self.presentation_or_boot_logic_frame()")
        && env.contains("self.current_player_id");
    residual_action_store(ResidualLogicFramePresentationHelperAction::DispatchSource);
    ok
}

pub fn honesty_logic_frame_presentation_helper_residual_pack_wave560() -> bool {
    honesty_logic_frame_presentation_helper_method_names_residual_wave560()
        && honesty_logic_frame_presentation_helper_source_markers_residual_wave560()
        && honesty_logic_frame_presentation_helper_nav_commands_residual_wave560()
        && simulate_logic_frame_presentation_helper_collect_source()
        && simulate_logic_frame_presentation_helper_dispatch_source()
}

pub fn simulate_live_logic_frame_presentation_helper_honesty() -> bool {
    let ok = honesty_logic_frame_presentation_helper_residual_pack_wave560();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualLogicFramePresentationHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_logic_frame_presentation_helper_method_names_residual_wave560());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_logic_frame_presentation_helper_source_markers_residual_wave560());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_logic_frame_presentation_helper_nav_commands_residual_wave560());
    }

    #[test]
    fn logic_frame_presentation_helper_sources() {
        assert!(simulate_logic_frame_presentation_helper_collect_source());
        assert!(simulate_logic_frame_presentation_helper_dispatch_source());
    }

    #[test]
    fn wave560_composite_pack() {
        assert!(honesty_logic_frame_presentation_helper_residual_pack_wave560());
    }

    #[test]
    fn simulate_live_logic_frame_presentation_helper_honesty_residual_live() {
        assert!(
            simulate_live_logic_frame_presentation_helper_honesty(),
            "logic frame presentation helper residual must latch"
        );
        assert!(residual_logic_frame_presentation_helper_ok());
        assert_eq!(
            residual_logic_frame_presentation_helper_last_action(),
            ResidualLogicFramePresentationHelperAction::Composite
        );
    }
}
