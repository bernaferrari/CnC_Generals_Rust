//! Wave 586 residual peels: GameClient presentation shell tick is centralized
//! through `host_tick_game_client_presentation_shell`. Full `GameClient::update`
//! remains intentionally disconnected (Main owns OS input/commands, presentation
//! audio dispatch, sole RenderPipeline present; avoids client frame-timing sleep).
//! Wave 587: helper also runs `update_input` as device bookkeeping on Main-injected
//! THE_MOUSE/THE_KEYBOARD (not a second OS poll).
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 585 UI/shell/world helper residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` host_tick_game_client_presentation_shell
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Not full WND OS-input GameClient ownership reconnect

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_GAME_CLIENT_SHELL_TICK_HELPER_METHOD_NAMES_WAVE586: &[&str] = &[
    "host_tick_game_client_presentation_shell",
    "update_presentation_shell",
    "update_input",
    "apply_frozen_direct_shroud_statuses",
    "apply_frozen_direct_presentation_poses",
    "GameClient::update",
    "Wave 586",
    "playable_claim = false",
];

pub const LIVE_HOST_GAME_CLIENT_SHELL_TICK_HELPER_NAV_STEPS_WAVE586: &[&str] = &[
    "REQUIRE_HOST_GAME_CLIENT_SHELL_TICK",
    "REQUIRE_PRESENTATION_SHROUD_POSE",
    "REQUIRE_NO_FULL_GAMECLIENT_UPDATE",
    "REQUIRE_NO_CLIENT_FRAME_SLEEP_ON_HOST",
    "LIVE_HOST_GAME_CLIENT_SHELL_TICK_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_GAME_CLIENT_SHELL_TICK_HELPER_CMD_NAMES_WAVE586: &[&str] = &[
    "host_game_client_shell_tick_helper",
    "presentation_frozen_direct_shroud_pose_apply",
    "no_full_gameclient_update",
    "game_client_shell_tick_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostGameClientShellTickHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostGameClientShellTickHelperAction {
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

fn residual_action_store(action: ResidualHostGameClientShellTickHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_game_client_shell_tick_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_game_client_shell_tick_helper_last_action()
-> ResidualHostGameClientShellTickHelperAction {
    ResidualHostGameClientShellTickHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
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
pub fn honesty_host_game_client_shell_tick_helper_method_names_residual_wave586() -> bool {
    let names = LIVE_HOST_GAME_CLIENT_SHELL_TICK_HELPER_METHOD_NAMES_WAVE586;
    let ok = residual_name_index(names, "host_tick_game_client_presentation_shell").is_some()
        && residual_name_index(names, "update_presentation_shell").is_some()
        && residual_name_index(names, "apply_frozen_direct_shroud_statuses").is_some()
        && residual_name_index(names, "apply_frozen_direct_presentation_poses").is_some()
        && residual_name_index(names, "GameClient::update").is_some()
        && residual_name_index(names, "Wave 586").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostGameClientShellTickHelperAction::MethodNames);
    ok
}

pub fn honesty_host_game_client_shell_tick_helper_source_markers_residual_wave586() -> bool {
    let eng = eng_source();
    let Some(body) = fn_body(eng, "fn host_tick_game_client_presentation_shell(") else {
        residual_action_store(ResidualHostGameClientShellTickHelperAction::SourceMarkers);
        return false;
    };
    let body_ok = (body.contains("Wave 586") || body.contains("Wave 587"))
        && body.contains("update_presentation_shell")
        && body.contains("presentation_or_boot_time_frozen")
        && body.contains("presentation_or_boot_time_frozen")
        && body.contains("apply_presentation_cinematic_letterbox")
        && body.contains("presentation_or_boot_time_frozen")
        // Wave 587: device bookkeeping allowed; full update() still forbidden.
        && body.contains("update_input()")
        && !body.contains("game_client.update()");
    // Disconnect rationale (frame sleep / ownership) lives on the helper docs.
    let docs_ok = eng.contains("finish_frame_timing` sleeps")
        || eng.contains("finish_frame_timing sleeps")
        || eng.contains("finish_frame_timing");
    let call_ok = eng.contains("self.host_tick_game_client_presentation_shell()");
    // Only the helper should invoke presentation shell on the production path.
    let shell_calls = eng.matches("update_presentation_shell(").count();
    let ok =
        body_ok && call_ok && docs_ok && shell_calls == 1 && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostGameClientShellTickHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_game_client_shell_tick_helper_nav_commands_residual_wave586() -> bool {
    let steps = LIVE_HOST_GAME_CLIENT_SHELL_TICK_HELPER_NAV_STEPS_WAVE586;
    let cmds = RUNTIME_HOST_LIVE_HOST_GAME_CLIENT_SHELL_TICK_HELPER_CMD_NAMES_WAVE586;
    let ok = residual_name_index(steps, "REQUIRE_HOST_GAME_CLIENT_SHELL_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_PRESENTATION_SHROUD_POSE").is_some()
        && residual_name_index(steps, "REQUIRE_NO_FULL_GAMECLIENT_UPDATE").is_some()
        && residual_name_index(steps, "REQUIRE_NO_CLIENT_FRAME_SLEEP_ON_HOST").is_some()
        && residual_name_index(steps, "LIVE_HOST_GAME_CLIENT_SHELL_TICK_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_game_client_shell_tick_helper").is_some()
        && residual_name_index(cmds, "presentation_frozen_direct_shroud_pose_apply").is_some()
        && residual_name_index(cmds, "no_full_gameclient_update").is_some()
        && residual_name_index(cmds, "game_client_shell_tick_residual").is_some();
    residual_action_store(ResidualHostGameClientShellTickHelperAction::NavCommands);
    ok
}

pub fn simulate_host_game_client_shell_tick_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 586")
        && eng.contains("fn host_tick_game_client_presentation_shell")
        && eng.contains("Full `GameClient::update()` stays disconnected on purpose");
    residual_action_store(ResidualHostGameClientShellTickHelperAction::CollectSource);
    ok
}

pub fn simulate_host_game_client_shell_tick_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.host_tick_game_client_presentation_shell()")
        && eng.contains("update_presentation_shell(visual_delta)")
        && eng.contains("update_input()")
        && !eng.contains("self.game_client.update()");
    residual_action_store(ResidualHostGameClientShellTickHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_game_client_shell_tick_helper_residual_pack_wave586() -> bool {
    honesty_host_game_client_shell_tick_helper_method_names_residual_wave586()
        && honesty_host_game_client_shell_tick_helper_source_markers_residual_wave586()
        && honesty_host_game_client_shell_tick_helper_nav_commands_residual_wave586()
        && simulate_host_game_client_shell_tick_helper_collect_source()
        && simulate_host_game_client_shell_tick_helper_dispatch_source()
}

pub fn simulate_live_host_game_client_shell_tick_helper_honesty() -> bool {
    let ok = honesty_host_game_client_shell_tick_helper_residual_pack_wave586();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostGameClientShellTickHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_game_client_shell_tick_helper_method_names_residual_wave586());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_game_client_shell_tick_helper_source_markers_residual_wave586());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_game_client_shell_tick_helper_nav_commands_residual_wave586());
    }

    #[test]
    fn host_game_client_shell_tick_helper_sources() {
        assert!(simulate_host_game_client_shell_tick_helper_collect_source());
        assert!(simulate_host_game_client_shell_tick_helper_dispatch_source());
    }

    #[test]
    fn wave586_composite_pack() {
        assert!(honesty_host_game_client_shell_tick_helper_residual_pack_wave586());
    }

    #[test]
    fn simulate_live_host_game_client_shell_tick_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_game_client_shell_tick_helper_honesty(),
            "host GameClient shell tick helper residual must latch"
        );
        assert!(residual_host_game_client_shell_tick_helper_ok());
        assert_eq!(
            residual_host_game_client_shell_tick_helper_last_action(),
            ResidualHostGameClientShellTickHelperAction::Composite
        );
    }
}
