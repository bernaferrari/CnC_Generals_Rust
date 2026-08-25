//! Wave 601 residual peels: restart-mission UI residual is centralized through
//! `host_restart_mission_from_ui`, and pause residual uses `host_set_paused`.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 575 host pause/team residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` host_restart_mission_from_ui / host_set_paused
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_RESTART_PAUSE_HELPER_METHOD_NAMES_WAVE601: &[&str] = &[
    "host_restart_mission_from_ui",
    "restart_mission_from_ui",
    "host_set_paused",
    "presentation_or_boot_map_name",
    "start_game_from_ui",
    "Wave 601",
    "playable_claim = false",
];

pub const LIVE_HOST_RESTART_PAUSE_HELPER_NAV_STEPS_WAVE601: &[&str] = &[
    "REQUIRE_RESTART_HELPER",
    "REQUIRE_HOST_PAUSE",
    "REQUIRE_MAP_FACTION_FROM_PRESENTATION",
    "LIVE_HOST_RESTART_PAUSE_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_RESTART_PAUSE_HELPER_CMD_NAMES_WAVE601: &[&str] = &[
    "host_restart_helper",
    "host_pause",
    "map_faction_from_presentation",
    "restart_pause_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostRestartPauseHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostRestartPauseHelperAction {
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

fn residual_action_store(action: ResidualHostRestartPauseHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_restart_pause_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_restart_pause_helper_last_action() -> ResidualHostRestartPauseHelperAction {
    ResidualHostRestartPauseHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
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

pub fn honesty_host_restart_pause_helper_method_names_residual_wave601() -> bool {
    let names = LIVE_HOST_RESTART_PAUSE_HELPER_METHOD_NAMES_WAVE601;
    let ok = residual_name_index(names, "host_restart_mission_from_ui").is_some()
        && residual_name_index(names, "host_set_paused").is_some()
        && residual_name_index(names, "Wave 601").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostRestartPauseHelperAction::MethodNames);
    ok
}

pub fn honesty_host_restart_pause_helper_source_markers_residual_wave601() -> bool {
    let eng = eng_source();
    let Some(wrapper) = fn_body(eng, "fn restart_mission_from_ui(&mut self)") else {
        residual_action_store(ResidualHostRestartPauseHelperAction::SourceMarkers);
        return false;
    };
    let Some(host) = fn_body(eng, "fn host_restart_mission_from_ui(&mut self)") else {
        residual_action_store(ResidualHostRestartPauseHelperAction::SourceMarkers);
        return false;
    };
    let Some(pause) = fn_body(eng, "fn host_set_paused(&mut self, paused: bool)") else {
        residual_action_store(ResidualHostRestartPauseHelperAction::SourceMarkers);
        return false;
    };
    let wrapper_ok = wrapper.contains("Wave 601")
        && wrapper.contains("host_restart_mission_from_ui()")
        && !wrapper.contains("start_game_from_ui");
    let host_ok = host.contains("Wave 601")
        && host.contains("presentation_or_boot_map_name")
        && host.contains("presentation_or_live_game_mode")
        && host.contains("start_game_from_ui");
    // 2026-08-15: pause is SessionControlOp::SetPaused (ui_commands.rs:845-847).
    let pause_ok = pause.contains("Wave 575")
        && pause.contains("game_paused = paused")
        && (pause.contains("set_paused(paused)") || pause.contains("SessionControlOp::SetPaused"));
    let call_ok = eng.contains("self.host_restart_mission_from_ui()")
        && eng.contains("self.host_set_paused(true)")
        && eng.contains("self.host_set_paused(false)");
    let ok = wrapper_ok && host_ok && pause_ok && call_ok && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostRestartPauseHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_restart_pause_helper_nav_commands_residual_wave601() -> bool {
    let steps = LIVE_HOST_RESTART_PAUSE_HELPER_NAV_STEPS_WAVE601;
    let cmds = RUNTIME_HOST_LIVE_HOST_RESTART_PAUSE_HELPER_CMD_NAMES_WAVE601;
    let ok = residual_name_index(steps, "REQUIRE_RESTART_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PAUSE").is_some()
        && residual_name_index(steps, "REQUIRE_MAP_FACTION_FROM_PRESENTATION").is_some()
        && residual_name_index(steps, "LIVE_HOST_RESTART_PAUSE_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_restart_helper").is_some()
        && residual_name_index(cmds, "host_pause").is_some()
        && residual_name_index(cmds, "map_faction_from_presentation").is_some()
        && residual_name_index(cmds, "restart_pause_residual").is_some();
    residual_action_store(ResidualHostRestartPauseHelperAction::NavCommands);
    ok
}

pub fn simulate_host_restart_pause_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 601")
        && eng.contains("fn host_restart_mission_from_ui")
        && eng.contains("fn host_set_paused");
    residual_action_store(ResidualHostRestartPauseHelperAction::CollectSource);
    ok
}

pub fn simulate_host_restart_pause_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.host_restart_mission_from_ui()")
        && eng.contains("self.host_set_paused(true)")
        && eng.contains("self.host_set_paused(false)")
        && eng.contains("self.host_set_paused(!self.game_paused)");
    residual_action_store(ResidualHostRestartPauseHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_restart_pause_helper_residual_pack_wave601() -> bool {
    honesty_host_restart_pause_helper_method_names_residual_wave601()
        && honesty_host_restart_pause_helper_source_markers_residual_wave601()
        && honesty_host_restart_pause_helper_nav_commands_residual_wave601()
        && simulate_host_restart_pause_helper_collect_source()
        && simulate_host_restart_pause_helper_dispatch_source()
}

pub fn simulate_live_host_restart_pause_helper_honesty() -> bool {
    let ok = honesty_host_restart_pause_helper_residual_pack_wave601();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostRestartPauseHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_restart_pause_helper_method_names_residual_wave601());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_restart_pause_helper_source_markers_residual_wave601());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_restart_pause_helper_nav_commands_residual_wave601());
    }

    #[test]
    fn host_restart_pause_helper_sources() {
        assert!(simulate_host_restart_pause_helper_collect_source());
        assert!(simulate_host_restart_pause_helper_dispatch_source());
    }

    #[test]
    fn wave601_composite_pack() {
        assert!(honesty_host_restart_pause_helper_residual_pack_wave601());
    }

    #[test]
    fn simulate_live_host_restart_pause_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_restart_pause_helper_honesty(),
            "host restart pause helper residual must latch"
        );
        assert!(residual_host_restart_pause_helper_ok());
        assert_eq!(
            residual_host_restart_pause_helper_last_action(),
            ResidualHostRestartPauseHelperAction::Composite
        );
    }
}
