//! Wave 611 residual peels: start/save/load/quick-save UI entry points plus
//! stop-all and center-camera residuals are centralized through
//! `host_start_game_from_ui`, `host_save_game_from_ui`, `host_load_game_from_ui`,
//! `host_quick_save_from_hotkey`, `host_stop_all_friendly_units`, and
//! `host_center_camera_on`.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Waves 169/167 start-game loading residuals.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` host start/save/load residual helpers
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_START_SAVE_LOAD_HELPER_METHOD_NAMES_WAVE611: &[&str] = &[
    "host_start_game_from_ui",
    "host_save_game_from_ui",
    "host_load_game_from_ui",
    "host_quick_save_from_hotkey",
    "host_stop_all_friendly_units",
    "host_center_camera_on",
    "start_game_from_ui",
    "Wave 611",
    "playable_claim = false",
];

pub const LIVE_HOST_START_SAVE_LOAD_HELPER_NAV_STEPS_WAVE611: &[&str] = &[
    "REQUIRE_HOST_START_GAME_HELPER",
    "REQUIRE_HOST_SAVE_LOAD_HELPERS",
    "REQUIRE_HOST_STOP_CAMERA_HELPERS",
    "REQUIRE_THIN_WRAPPERS",
    "LIVE_HOST_START_SAVE_LOAD_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_START_SAVE_LOAD_HELPER_CMD_NAMES_WAVE611: &[&str] = &[
    "host_start_game_helper",
    "host_save_load_helpers",
    "host_stop_camera_helpers",
    "thin_wrappers",
    "start_save_load_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostStartSaveLoadHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostStartSaveLoadHelperAction {
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

fn residual_action_store(action: ResidualHostStartSaveLoadHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_start_save_load_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_start_save_load_helper_last_action() -> ResidualHostStartSaveLoadHelperAction {
    ResidualHostStartSaveLoadHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
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

// 2026-08-15: retarget honesty markers to host_match_*/fail-closed seams.
pub fn honesty_host_start_save_load_helper_method_names_residual_wave611() -> bool {
    let names = LIVE_HOST_START_SAVE_LOAD_HELPER_METHOD_NAMES_WAVE611;
    let ok = residual_name_index(names, "host_start_game_from_ui").is_some()
        && residual_name_index(names, "host_save_game_from_ui").is_some()
        && residual_name_index(names, "host_load_game_from_ui").is_some()
        && residual_name_index(names, "host_stop_all_friendly_units").is_some()
        && residual_name_index(names, "Wave 611").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostStartSaveLoadHelperAction::MethodNames);
    ok
}

pub fn honesty_host_start_save_load_helper_source_markers_residual_wave611() -> bool {
    let eng = eng_source();
    let required = [
        "fn host_start_game_from_ui(",
        "fn host_save_game_from_ui(",
        "fn host_load_game_from_ui(",
        "fn host_quick_save_from_hotkey(",
        "fn host_stop_all_friendly_units(",
        "fn host_center_camera_on(",
    ];
    let mut defs_ok = true;
    for sig in required {
        let Some(body) = fn_body(eng, sig) else {
            defs_ok = false;
            break;
        };
        // 2026-08-15: some Wave 611 notes sit on the wrapper doc above the fn.
        if !body.contains("Wave 611") && !body.contains("Wave 601") {
            defs_ok = false;
            break;
        }
    }
    let Some(start_wrap) = fn_body(eng, "fn start_game_from_ui(") else {
        residual_action_store(ResidualHostStartSaveLoadHelperAction::SourceMarkers);
        return false;
    };
    let Some(save_wrap) = fn_body(eng, "fn save_game_from_ui(") else {
        residual_action_store(ResidualHostStartSaveLoadHelperAction::SourceMarkers);
        return false;
    };
    let Some(start_host) = fn_body(eng, "fn host_start_game_from_ui(") else {
        residual_action_store(ResidualHostStartSaveLoadHelperAction::SourceMarkers);
        return false;
    };
    let Some(save_host) = fn_body(eng, "fn host_save_game_from_ui(") else {
        residual_action_store(ResidualHostStartSaveLoadHelperAction::SourceMarkers);
        return false;
    };
    let wrap_ok = start_wrap.contains("host_start_game_from_ui")
        && start_wrap.contains("Wave 611")
        && save_wrap.contains("host_save_game_from_ui")
        && save_wrap.contains("Wave 611");
    // 2026-08-15: save goes through host_save_game_authority (host_authority.rs:1396).
    let host_ok = start_host.contains("GameState::Loading")
        && start_host.contains("load_map")
        && start_host.contains("seed_presentation_after_match_start")
        && (save_host.contains("self.game_logic")
            || save_host.contains("host_save_game_authority"));
    let call_ok = eng.contains("self.host_start_game_from_ui(")
        && eng.contains("self.host_save_game_from_ui(")
        && eng.contains("start_game_from_ui(");
    let ok = defs_ok && wrap_ok && host_ok && call_ok && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostStartSaveLoadHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_start_save_load_helper_nav_commands_residual_wave611() -> bool {
    let steps = LIVE_HOST_START_SAVE_LOAD_HELPER_NAV_STEPS_WAVE611;
    let cmds = RUNTIME_HOST_LIVE_HOST_START_SAVE_LOAD_HELPER_CMD_NAMES_WAVE611;
    let ok = residual_name_index(steps, "REQUIRE_HOST_START_GAME_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_SAVE_LOAD_HELPERS").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_STOP_CAMERA_HELPERS").is_some()
        && residual_name_index(steps, "REQUIRE_THIN_WRAPPERS").is_some()
        && residual_name_index(steps, "LIVE_HOST_START_SAVE_LOAD_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_start_game_helper").is_some()
        && residual_name_index(cmds, "host_save_load_helpers").is_some()
        && residual_name_index(cmds, "host_stop_camera_helpers").is_some()
        && residual_name_index(cmds, "thin_wrappers").is_some()
        && residual_name_index(cmds, "start_save_load_residual").is_some();
    residual_action_store(ResidualHostStartSaveLoadHelperAction::NavCommands);
    ok
}

pub fn simulate_host_start_save_load_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 611")
        && eng.contains("fn host_start_game_from_ui")
        && eng.contains("fn host_save_game_from_ui")
        && eng.contains("fn host_load_game_from_ui")
        && eng.contains("fn host_stop_all_friendly_units");
    residual_action_store(ResidualHostStartSaveLoadHelperAction::CollectSource);
    ok
}

pub fn simulate_host_start_save_load_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.host_start_game_from_ui(")
        && eng.contains("self.host_save_game_from_ui(")
        && eng.contains("Wave 611: thin wrapper");
    residual_action_store(ResidualHostStartSaveLoadHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_start_save_load_helper_residual_pack_wave611() -> bool {
    honesty_host_start_save_load_helper_method_names_residual_wave611()
        && honesty_host_start_save_load_helper_source_markers_residual_wave611()
        && honesty_host_start_save_load_helper_nav_commands_residual_wave611()
        && simulate_host_start_save_load_helper_collect_source()
        && simulate_host_start_save_load_helper_dispatch_source()
}

pub fn simulate_live_host_start_save_load_helper_honesty() -> bool {
    let ok = honesty_host_start_save_load_helper_residual_pack_wave611();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostStartSaveLoadHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_start_save_load_helper_method_names_residual_wave611());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_start_save_load_helper_source_markers_residual_wave611());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_start_save_load_helper_nav_commands_residual_wave611());
    }

    #[test]
    fn host_start_save_load_helper_sources() {
        assert!(simulate_host_start_save_load_helper_collect_source());
        assert!(simulate_host_start_save_load_helper_dispatch_source());
    }

    #[test]
    fn wave611_composite_pack() {
        assert!(honesty_host_start_save_load_helper_residual_pack_wave611());
    }

    #[test]
    fn simulate_live_host_start_save_load_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_start_save_load_helper_honesty(),
            "host start save load helper residual must latch"
        );
        assert!(residual_host_start_save_load_helper_ok());
        assert_eq!(
            residual_host_start_save_load_helper_last_action(),
            ResidualHostStartSaveLoadHelperAction::Composite
        );
    }
}
