//! Wave 568 residual peels: script FPS residual is centralized through
//! `apply_ingame_script_fps_limit_residual` (freeze prefer + always drain) and
//! `apply_shell_script_fps_limit_residual` (shell freeze only when
//! `fow_shell_bypass`, else boot take). Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 567 boot movie helper residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` apply_ingame/shell_script_fps_limit_residual
//! - `presentation_frame.rs` script_fps_limit
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_SCRIPT_FPS_HELPER_METHOD_NAMES_WAVE568: &[&str] = &[
    "apply_ingame_script_fps_limit_residual",
    "apply_shell_script_fps_limit_residual",
    "script_fps_limit",
    "take_script_fps_limit_request",
    "Wave 568",
    "playable_claim = false",
];

pub const LIVE_SCRIPT_FPS_HELPER_NAV_STEPS_WAVE568: &[&str] = &[
    "REQUIRE_INGAME_SCRIPT_FPS_HELPER",
    "REQUIRE_SHELL_SCRIPT_FPS_HELPER",
    "LIVE_SCRIPT_FPS_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_SCRIPT_FPS_HELPER_CMD_NAMES_WAVE568: &[&str] = &[
    "ingame_script_fps_helper",
    "shell_script_fps_helper",
    "script_fps_limit_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualScriptFpsHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualScriptFpsHelperAction {
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

fn residual_action_store(action: ResidualScriptFpsHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_script_fps_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_script_fps_helper_last_action() -> ResidualScriptFpsHelperAction {
    ResidualScriptFpsHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
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
pub fn honesty_script_fps_helper_method_names_residual_wave568() -> bool {
    let names = LIVE_SCRIPT_FPS_HELPER_METHOD_NAMES_WAVE568;
    let ok = residual_name_index(names, "apply_ingame_script_fps_limit_residual").is_some()
        && residual_name_index(names, "apply_shell_script_fps_limit_residual").is_some()
        && residual_name_index(names, "script_fps_limit").is_some()
        && residual_name_index(names, "take_script_fps_limit_request").is_some()
        && residual_name_index(names, "Wave 568").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualScriptFpsHelperAction::MethodNames);
    ok
}

pub fn honesty_script_fps_helper_source_markers_residual_wave568() -> bool {
    let eng = eng_source();
    let pf = pf_source();
    let field_ok = pf.contains("pub script_fps_limit: Option<i32>");
    let Some(ingame) = fn_body(eng, "fn apply_ingame_script_fps_limit_residual(") else {
        residual_action_store(ResidualScriptFpsHelperAction::SourceMarkers);
        return false;
    };
    let Some(shell) = fn_body(eng, "fn apply_shell_script_fps_limit_residual(") else {
        residual_action_store(ResidualScriptFpsHelperAction::SourceMarkers);
        return false;
    };
    let ingame_ok = ingame.contains("Wave 568")
        && ingame.contains("p.script_fps_limit")
        && ingame.contains("take_script_fps_limit_request()");
    let shell_ok = shell.contains("Wave 568")
        && shell.contains("fow_shell_bypass")
        && shell.contains("p.script_fps_limit")
        && shell.contains("take_script_fps_limit_request()");
    let call_ok = eng.contains("self.apply_ingame_script_fps_limit_residual()")
        && eng.contains("self.apply_shell_script_fps_limit_residual()");
    let raw = eng
        .matches("script_fps_limit")
        .count();
    // only inside helpers
    let ok = field_ok
        && ingame_ok
        && shell_ok
        && call_ok
        && raw == 3
        && !eng.contains("playable_claim = true");
    residual_action_store(ResidualScriptFpsHelperAction::SourceMarkers);
    ok
}

pub fn honesty_script_fps_helper_nav_commands_residual_wave568() -> bool {
    let steps = LIVE_SCRIPT_FPS_HELPER_NAV_STEPS_WAVE568;
    let cmds = RUNTIME_HOST_LIVE_SCRIPT_FPS_HELPER_CMD_NAMES_WAVE568;
    let ok = residual_name_index(steps, "REQUIRE_INGAME_SCRIPT_FPS_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_SHELL_SCRIPT_FPS_HELPER").is_some()
        && residual_name_index(steps, "LIVE_SCRIPT_FPS_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "ingame_script_fps_helper").is_some()
        && residual_name_index(cmds, "shell_script_fps_helper").is_some()
        && residual_name_index(cmds, "script_fps_limit_residual").is_some();
    residual_action_store(ResidualScriptFpsHelperAction::NavCommands);
    ok
}

pub fn simulate_script_fps_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 568")
        && eng.contains("fn apply_ingame_script_fps_limit_residual")
        && eng.contains("fn apply_shell_script_fps_limit_residual");
    residual_action_store(ResidualScriptFpsHelperAction::CollectSource);
    ok
}

pub fn simulate_script_fps_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.apply_ingame_script_fps_limit_residual()")
        && eng.contains("self.apply_shell_script_fps_limit_residual()")
        && eng.contains("presentation_affirms_shell_or_boot");
    residual_action_store(ResidualScriptFpsHelperAction::DispatchSource);
    ok
}

pub fn honesty_script_fps_helper_residual_pack_wave568() -> bool {
    honesty_script_fps_helper_method_names_residual_wave568()
        && honesty_script_fps_helper_source_markers_residual_wave568()
        && honesty_script_fps_helper_nav_commands_residual_wave568()
        && simulate_script_fps_helper_collect_source()
        && simulate_script_fps_helper_dispatch_source()
}

pub fn simulate_live_script_fps_helper_honesty() -> bool {
    let ok = honesty_script_fps_helper_residual_pack_wave568();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualScriptFpsHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_script_fps_helper_method_names_residual_wave568());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_script_fps_helper_source_markers_residual_wave568());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_script_fps_helper_nav_commands_residual_wave568());
    }

    #[test]
    fn script_fps_helper_sources() {
        assert!(simulate_script_fps_helper_collect_source());
        assert!(simulate_script_fps_helper_dispatch_source());
    }

    #[test]
    fn wave568_composite_pack() {
        assert!(honesty_script_fps_helper_residual_pack_wave568());
    }

    #[test]
    fn simulate_live_script_fps_helper_honesty_residual_live() {
        assert!(
            simulate_live_script_fps_helper_honesty(),
            "script FPS helper residual must latch"
        );
        assert!(residual_script_fps_helper_ok());
        assert_eq!(
            residual_script_fps_helper_last_action(),
            ResidualScriptFpsHelperAction::Composite
        );
    }
}
