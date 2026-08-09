//! Wave 575 residual peels: host pause dual-write is centralized through
//! `host_set_paused`, and `ui_local_player_team_name` routes through
//! `presentation_or_boot_local_team`. Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 574 boot local player helper residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` host_set_paused / ui_local_player_team_name
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PAUSE_TEAM_HELPER_METHOD_NAMES_WAVE575: &[&str] = &[
    "host_set_paused",
    "ui_local_player_team_name",
    "presentation_or_boot_local_team",
    "set_paused",
    "Wave 575",
    "playable_claim = false",
];

pub const LIVE_HOST_PAUSE_TEAM_HELPER_NAV_STEPS_WAVE575: &[&str] = &[
    "REQUIRE_HOST_PAUSE_HELPER",
    "REQUIRE_TEAM_NAME_USES_LOCAL_TEAM_HELPER",
    "LIVE_HOST_PAUSE_TEAM_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_PAUSE_TEAM_HELPER_CMD_NAMES_WAVE575: &[&str] = &[
    "host_pause_helper",
    "team_name_helper",
    "pause_team_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPauseTeamHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostPauseTeamHelperAction {
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

fn residual_action_store(action: ResidualHostPauseTeamHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_pause_team_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_pause_team_helper_last_action() -> ResidualHostPauseTeamHelperAction {
    ResidualHostPauseTeamHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
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

pub fn honesty_host_pause_team_helper_method_names_residual_wave575() -> bool {
    let names = LIVE_HOST_PAUSE_TEAM_HELPER_METHOD_NAMES_WAVE575;
    let ok = residual_name_index(names, "host_set_paused").is_some()
        && residual_name_index(names, "ui_local_player_team_name").is_some()
        && residual_name_index(names, "presentation_or_boot_local_team").is_some()
        && residual_name_index(names, "set_paused").is_some()
        && residual_name_index(names, "Wave 575").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostPauseTeamHelperAction::MethodNames);
    ok
}

pub fn honesty_host_pause_team_helper_source_markers_residual_wave575() -> bool {
    let eng = eng_source();
    let Some(pause) = fn_body(eng, "fn host_set_paused(") else {
        residual_action_store(ResidualHostPauseTeamHelperAction::SourceMarkers);
        return false;
    };
    let Some(name) = fn_body(eng, "fn host_ui_local_player_team_name(")
        .or_else(|| fn_body(eng, "fn ui_local_player_team_name("))
    else {
        residual_action_store(ResidualHostPauseTeamHelperAction::SourceMarkers);
        return false;
    };
    let pause_ok = pause.contains("Wave 575")
        && pause.contains("self.game_paused = paused")
        && pause.contains("self.game_logic.set_paused(paused)");
    let name_ok = name.contains("Wave 575")
        && name.contains("presentation_or_boot_local_team()")
        && !name.contains("player_team(");
    let raw_set = eng.matches("self.game_logic.set_paused").count();
    let call_ok = eng.contains("self.host_set_paused(true)")
        && eng.contains("self.host_set_paused(false)")
        && eng.contains("self.host_set_paused(!self.game_paused)");
    // only inside host_set_paused
    let ok =
        pause_ok && name_ok && call_ok && raw_set == 1 && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostPauseTeamHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_pause_team_helper_nav_commands_residual_wave575() -> bool {
    let steps = LIVE_HOST_PAUSE_TEAM_HELPER_NAV_STEPS_WAVE575;
    let cmds = RUNTIME_HOST_LIVE_HOST_PAUSE_TEAM_HELPER_CMD_NAMES_WAVE575;
    let ok = residual_name_index(steps, "REQUIRE_HOST_PAUSE_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_TEAM_NAME_USES_LOCAL_TEAM_HELPER").is_some()
        && residual_name_index(steps, "LIVE_HOST_PAUSE_TEAM_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_pause_helper").is_some()
        && residual_name_index(cmds, "team_name_helper").is_some()
        && residual_name_index(cmds, "pause_team_residual").is_some();
    residual_action_store(ResidualHostPauseTeamHelperAction::NavCommands);
    ok
}

pub fn simulate_host_pause_team_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 575")
        && eng.contains("fn host_set_paused")
        && eng.contains("fn ui_local_player_team_name");
    residual_action_store(ResidualHostPauseTeamHelperAction::CollectSource);
    ok
}

pub fn simulate_host_pause_team_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.host_set_paused(")
        && eng.contains("presentation_or_boot_local_team()")
        && eng.contains("apply_presentation_popup_music_residual");
    residual_action_store(ResidualHostPauseTeamHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_pause_team_helper_residual_pack_wave575() -> bool {
    honesty_host_pause_team_helper_method_names_residual_wave575()
        && honesty_host_pause_team_helper_source_markers_residual_wave575()
        && honesty_host_pause_team_helper_nav_commands_residual_wave575()
        && simulate_host_pause_team_helper_collect_source()
        && simulate_host_pause_team_helper_dispatch_source()
}

pub fn simulate_live_host_pause_team_helper_honesty() -> bool {
    let ok = honesty_host_pause_team_helper_residual_pack_wave575();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostPauseTeamHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_pause_team_helper_method_names_residual_wave575());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_pause_team_helper_source_markers_residual_wave575());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_pause_team_helper_nav_commands_residual_wave575());
    }

    #[test]
    fn host_pause_team_helper_sources() {
        assert!(simulate_host_pause_team_helper_collect_source());
        assert!(simulate_host_pause_team_helper_dispatch_source());
    }

    #[test]
    fn wave575_composite_pack() {
        assert!(honesty_host_pause_team_helper_residual_pack_wave575());
    }

    #[test]
    fn simulate_live_host_pause_team_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_pause_team_helper_honesty(),
            "host pause/team helper residual must latch"
        );
        assert!(residual_host_pause_team_helper_ok());
        assert_eq!(
            residual_host_pause_team_helper_last_action(),
            ResidualHostPauseTeamHelperAction::Composite
        );
    }
}
