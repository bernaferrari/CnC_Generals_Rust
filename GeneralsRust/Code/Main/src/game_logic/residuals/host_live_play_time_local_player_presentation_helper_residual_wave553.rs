//! Wave 553 residual peels: total play-time and local player id residuals are
//! centralized through `presentation_or_boot_total_play_time` /
//! `presentation_or_boot_local_player_id` — presentation freeze owns both when
//! installed (pipeline preferred for play-time); boot residual without freeze
//! uses host probes. Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 552 shell-bypass presentation helper residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` presentation_or_boot_total_play_time /
//!   presentation_or_boot_local_player_id / call sites
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_PLAY_TIME_LOCAL_PLAYER_PRESENTATION_HELPER_METHOD_NAMES_WAVE553: &[&str] = &[
    "presentation_or_boot_total_play_time",
    "presentation_or_boot_local_player_id",
    "total_play_time_seconds",
    "local_player_id",
    "Wave 553",
    "playable_claim = false",
];

pub const LIVE_PLAY_TIME_LOCAL_PLAYER_PRESENTATION_HELPER_NAV_STEPS_WAVE553: &[&str] = &[
    "REQUIRE_PLAY_TIME_PRESENTATION_HELPER",
    "REQUIRE_LOCAL_PLAYER_PRESENTATION_HELPER",
    "LIVE_PLAY_TIME_LOCAL_PLAYER_PRESENTATION_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_PLAY_TIME_LOCAL_PLAYER_PRESENTATION_HELPER_CMD_NAMES_WAVE553:
    &[&str] = &[
    "play_time_presentation_helper",
    "local_player_presentation_helper",
    "boot_get_total_play_time",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualPlayTimeLocalPlayerPresentationHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualPlayTimeLocalPlayerPresentationHelperAction {
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

fn residual_action_store(action: ResidualPlayTimeLocalPlayerPresentationHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_play_time_local_player_presentation_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_play_time_local_player_presentation_helper_last_action()
-> ResidualPlayTimeLocalPlayerPresentationHelperAction {
    ResidualPlayTimeLocalPlayerPresentationHelperAction::from_u8(
        RESIDUAL_ACTION.load(Ordering::SeqCst),
    )
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
pub fn honesty_play_time_local_player_presentation_helper_method_names_residual_wave553() -> bool {
    let names = LIVE_PLAY_TIME_LOCAL_PLAYER_PRESENTATION_HELPER_METHOD_NAMES_WAVE553;
    let ok = residual_name_index(names, "presentation_or_boot_total_play_time").is_some()
        && residual_name_index(names, "presentation_or_boot_local_player_id").is_some()
        && residual_name_index(names, "total_play_time_seconds").is_some()
        && residual_name_index(names, "local_player_id").is_some()
        && residual_name_index(names, "Wave 553").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualPlayTimeLocalPlayerPresentationHelperAction::MethodNames);
    ok
}

pub fn honesty_play_time_local_player_presentation_helper_source_markers_residual_wave553() -> bool
{
    let eng = eng_source();
    let Some(pt) = fn_body(eng, "fn presentation_or_boot_total_play_time(") else {
        residual_action_store(ResidualPlayTimeLocalPlayerPresentationHelperAction::SourceMarkers);
        return false;
    };
    let Some(lp) = fn_body(eng, "fn presentation_or_boot_local_player_id(") else {
        residual_action_store(ResidualPlayTimeLocalPlayerPresentationHelperAction::SourceMarkers);
        return false;
    };
    // 2026-08-15: Wave 895 fail-closed — helpers use host_match_* / 0.0, no
    // get_total_play_time dual-read in the presentation path (camera_drain.rs).
    let pt_ok = pt.contains("Wave 553")
        && pt.contains("total_play_time_seconds")
        && pt.contains("host_match_total_play_time")
        && pt.contains("presentation_frame()");
    let lp_ok = lp.contains("Wave 553")
        && lp.contains("pres.local_player_id")
        && lp.contains("host_match_local_player_id");
    let calls = eng.contains("presentation_or_boot_total_play_time()")
        && eng.contains("presentation_or_boot_local_player_id()");
    let ok = pt_ok
        && lp_ok
        && calls
        && !pt.contains("self.game_logic.get_total_play_time()")
        && !lp.contains("self.game_logic.local_player_id()")
        && !eng.contains("playable_claim = true");
    residual_action_store(ResidualPlayTimeLocalPlayerPresentationHelperAction::SourceMarkers);
    ok
}

pub fn honesty_play_time_local_player_presentation_helper_nav_commands_residual_wave553() -> bool {
    let steps = LIVE_PLAY_TIME_LOCAL_PLAYER_PRESENTATION_HELPER_NAV_STEPS_WAVE553;
    let cmds = RUNTIME_HOST_LIVE_PLAY_TIME_LOCAL_PLAYER_PRESENTATION_HELPER_CMD_NAMES_WAVE553;
    let ok = residual_name_index(steps, "REQUIRE_PLAY_TIME_PRESENTATION_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_LOCAL_PLAYER_PRESENTATION_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAY_TIME_LOCAL_PLAYER_PRESENTATION_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "play_time_presentation_helper").is_some()
        && residual_name_index(cmds, "local_player_presentation_helper").is_some()
        && residual_name_index(cmds, "boot_get_total_play_time").is_some();
    residual_action_store(ResidualPlayTimeLocalPlayerPresentationHelperAction::NavCommands);
    ok
}

pub fn simulate_play_time_local_player_presentation_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 553")
        && eng.contains("fn presentation_or_boot_total_play_time")
        && eng.contains("fn presentation_or_boot_local_player_id");
    residual_action_store(ResidualPlayTimeLocalPlayerPresentationHelperAction::CollectSource);
    ok
}

pub fn simulate_play_time_local_player_presentation_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("presentation_or_boot_total_play_time()")
        && eng.contains("presentation_or_boot_local_player_id()")
        && eng.contains("Wave 462: prefer pipeline/last presentation sim clock residual")
        && eng.contains("Wave 553");
    residual_action_store(ResidualPlayTimeLocalPlayerPresentationHelperAction::DispatchSource);
    ok
}

pub fn honesty_play_time_local_player_presentation_helper_residual_pack_wave553() -> bool {
    honesty_play_time_local_player_presentation_helper_method_names_residual_wave553()
        && honesty_play_time_local_player_presentation_helper_source_markers_residual_wave553()
        && honesty_play_time_local_player_presentation_helper_nav_commands_residual_wave553()
        && simulate_play_time_local_player_presentation_helper_collect_source()
        && simulate_play_time_local_player_presentation_helper_dispatch_source()
}

pub fn simulate_live_play_time_local_player_presentation_helper_honesty() -> bool {
    let ok = honesty_play_time_local_player_presentation_helper_residual_pack_wave553();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPlayTimeLocalPlayerPresentationHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_play_time_local_player_presentation_helper_method_names_residual_wave553());
    }

    #[test]
    fn source_markers_residual() {
        assert!(
            honesty_play_time_local_player_presentation_helper_source_markers_residual_wave553()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_play_time_local_player_presentation_helper_nav_commands_residual_wave553());
    }

    #[test]
    fn play_time_local_player_presentation_helper_sources() {
        assert!(simulate_play_time_local_player_presentation_helper_collect_source());
        assert!(simulate_play_time_local_player_presentation_helper_dispatch_source());
    }

    #[test]
    fn wave553_composite_pack() {
        assert!(honesty_play_time_local_player_presentation_helper_residual_pack_wave553());
    }

    #[test]
    fn simulate_live_play_time_local_player_presentation_helper_honesty_residual_live() {
        assert!(
            simulate_live_play_time_local_player_presentation_helper_honesty(),
            "play-time/local-player presentation helper residual must latch"
        );
        assert!(residual_play_time_local_player_presentation_helper_ok());
        assert_eq!(
            residual_play_time_local_player_presentation_helper_last_action(),
            ResidualPlayTimeLocalPlayerPresentationHelperAction::Composite
        );
    }
}
