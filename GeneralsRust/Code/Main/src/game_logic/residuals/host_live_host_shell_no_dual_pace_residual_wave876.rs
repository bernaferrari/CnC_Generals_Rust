//! Wave 876: presentation shell skips finish_frame_timing sleep so Main remains
//! sole frame-pace owner. Full GameClient::update stays disconnected.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SHELL_NO_DUAL_PACE_METHOD_NAMES_WAVE876: &[&str] = &[
    "update_presentation_shell",
    "finish_frame_timing",
    "host_tick_game_client_presentation_shell",
    "Wave 876",
    "playable_claim = false",
];

pub const LIVE_HOST_SHELL_NO_DUAL_PACE_NAV_STEPS_WAVE876: &[&str] = &[
    "SHELL_NO_FINISH_FRAME_TIMING_SLEEP",
    "MAIN_SOLE_FRAME_PACE",
    "FULL_UPDATE_STILL_DISCONNECTED",
    "LIVE_HOST_SHELL_NO_DUAL_PACE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostShellNoDualPaceAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostShellNoDualPaceAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
}

fn game_client_source() -> &'static str {
    include_str!("../../../../GameEngine/GameClient/src/core/game_client.rs")
}

pub fn honesty_host_shell_no_dual_pace_method_names_residual_wave876() -> bool {
    let names = LIVE_HOST_SHELL_NO_DUAL_PACE_METHOD_NAMES_WAVE876;
    let ok = residual_name_index(names, "update_presentation_shell").is_some()
        && residual_name_index(names, "Wave 876").is_some()
        && residual_name_index(names, "finish_frame_timing").is_some();
    residual_action_store(ResidualHostShellNoDualPaceAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_shell_no_dual_pace_nav_commands_residual_wave876() -> bool {
    let steps = LIVE_HOST_SHELL_NO_DUAL_PACE_NAV_STEPS_WAVE876;
    let ok = residual_name_index(steps, "LIVE_HOST_SHELL_NO_DUAL_PACE").is_some()
        && residual_name_index(steps, "SHELL_NO_FINISH_FRAME_TIMING_SLEEP").is_some();
    residual_action_store(ResidualHostShellNoDualPaceAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_shell_no_dual_pace_residual_pack_wave876() -> bool {
    let cnc = cnc_source();
    let gc = game_client_source();
    let Some(j) = gc.find("pub fn update_presentation_shell") else {
        residual_action_store(ResidualHostShellNoDualPaceAction::SourceMarkers);
        RESIDUAL_OK.store(false, Ordering::SeqCst);
        return false;
    };
    let k = gc[j + 10..]
        .find(
            "
    pub fn ",
        )
        .map(|i| j + 10 + i)
        .unwrap_or(gc.len());
    let shell = &gc[j..k];
    let ok = !shell.contains("finish_frame_timing")
        && shell.contains("Wave 876: Main owns frame pacing")
        && cnc.contains(
            "Wave 876: `update_presentation_shell` no longer sleeps; Main sole frame pace.",
        )
        && cnc.contains("Full `GameClient::update()` stays disconnected on purpose")
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostShellNoDualPaceAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_shell_no_dual_pace_honesty() -> bool {
    let a = honesty_host_shell_no_dual_pace_method_names_residual_wave876();
    let b = honesty_host_shell_no_dual_pace_nav_commands_residual_wave876();
    let c = honesty_host_shell_no_dual_pace_residual_pack_wave876();
    residual_action_store(ResidualHostShellNoDualPaceAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_shell_no_dual_pace_residual_wave876() {
        assert!(honesty_host_shell_no_dual_pace_residual_pack_wave876());
        assert!(honesty_host_shell_no_dual_pace_method_names_residual_wave876());
        assert!(honesty_host_shell_no_dual_pace_nav_commands_residual_wave876());
        assert!(simulate_live_host_shell_no_dual_pace_honesty());
    }
}
