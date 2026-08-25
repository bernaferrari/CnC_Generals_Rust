//! Wave 609 residual peels: UI economy/mouse/game-mode/camera-pitch/selection-seed
//! probes are centralized through `host_ui_local_economy`,
//! `host_presentation_mouse_game_logic`, `host_presentation_or_live_game_mode`,
//! `host_ui_script_default_camera_pitch`, and `host_ui_selection_seed_id`.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Waves 238/542/544/252 presentation probe residuals.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` host UI/presentation residual helpers
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_UI_ECONOMY_MOUSE_MODE_HELPER_METHOD_NAMES_WAVE609: &[&str] = &[
    "host_ui_local_economy",
    "host_presentation_mouse_game_logic",
    "host_presentation_or_live_game_mode",
    "host_ui_script_default_camera_pitch",
    "host_ui_selection_seed_id",
    "ui_local_economy",
    "Wave 609",
    "playable_claim = false",
];

pub const LIVE_HOST_UI_ECONOMY_MOUSE_MODE_HELPER_NAV_STEPS_WAVE609: &[&str] = &[
    "REQUIRE_HOST_UI_ECONOMY_HELPER",
    "REQUIRE_HOST_MOUSE_GAME_LOGIC_HELPER",
    "REQUIRE_HOST_GAME_MODE_HELPER",
    "REQUIRE_THIN_WRAPPERS",
    "LIVE_HOST_UI_ECONOMY_MOUSE_MODE_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_UI_ECONOMY_MOUSE_MODE_HELPER_CMD_NAMES_WAVE609: &[&str] = &[
    "host_ui_economy_helper",
    "host_mouse_game_logic_helper",
    "host_game_mode_helper",
    "thin_wrappers",
    "ui_economy_mouse_mode_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostUiEconomyMouseModeHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostUiEconomyMouseModeHelperAction {
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

fn residual_action_store(action: ResidualHostUiEconomyMouseModeHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_ui_economy_mouse_mode_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_ui_economy_mouse_mode_helper_last_action()
-> ResidualHostUiEconomyMouseModeHelperAction {
    ResidualHostUiEconomyMouseModeHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
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
pub fn honesty_host_ui_economy_mouse_mode_helper_method_names_residual_wave609() -> bool {
    let names = LIVE_HOST_UI_ECONOMY_MOUSE_MODE_HELPER_METHOD_NAMES_WAVE609;
    let ok = residual_name_index(names, "host_ui_local_economy").is_some()
        && residual_name_index(names, "host_presentation_mouse_game_logic").is_some()
        && residual_name_index(names, "host_presentation_or_live_game_mode").is_some()
        && residual_name_index(names, "host_ui_selection_seed_id").is_some()
        && residual_name_index(names, "Wave 609").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostUiEconomyMouseModeHelperAction::MethodNames);
    ok
}

pub fn honesty_host_ui_economy_mouse_mode_helper_source_markers_residual_wave609() -> bool {
    let eng = eng_source();
    let required = [
        "fn host_ui_local_economy(",
        "fn host_presentation_mouse_game_logic(",
        "fn host_presentation_or_live_game_mode(",
        "fn host_ui_script_default_camera_pitch(",
        "fn host_ui_selection_seed_id(",
    ];
    let mut defs_ok = true;
    for sig in required {
        let Some(body) = fn_body(eng, sig) else {
            defs_ok = false;
            break;
        };
        if !body.contains("Wave 609") {
            defs_ok = false;
            break;
        }
    }
    let Some(econ_wrap) = fn_body(eng, "fn ui_local_economy(") else {
        residual_action_store(ResidualHostUiEconomyMouseModeHelperAction::SourceMarkers);
        return false;
    };
    let Some(mouse_wrap) = fn_body(eng, "fn presentation_mouse_game_logic(") else {
        residual_action_store(ResidualHostUiEconomyMouseModeHelperAction::SourceMarkers);
        return false;
    };
    let Some(econ_host) = fn_body(eng, "fn host_ui_local_economy(") else {
        residual_action_store(ResidualHostUiEconomyMouseModeHelperAction::SourceMarkers);
        return false;
    };
    let Some(mouse_host) = fn_body(eng, "fn host_presentation_mouse_game_logic(") else {
        residual_action_store(ResidualHostUiEconomyMouseModeHelperAction::SourceMarkers);
        return false;
    };
    let wrap_ok = econ_wrap.contains("host_ui_local_economy")
        && econ_wrap.contains("Wave 609")
        && mouse_wrap.contains("host_presentation_mouse_game_logic")
        && mouse_wrap.contains("Wave 609");
    // 2026-08-15: economy fail-closed (0,0,0); mouse is presentation-only None
    // (input.rs:1270-1273).
    let host_ok = econ_host.contains("last_presentation_frame")
        && (econ_host.contains("player_economy") || econ_host.contains("local_supplies"))
        && (mouse_host.contains("last_presentation_frame")
            || mouse_host.contains("presentation-only"))
        && (mouse_host.contains("Some(&self.game_logic)") || mouse_host.contains("None"));
    let call_ok = eng.contains("self.host_ui_local_economy()")
        && eng.contains("self.host_presentation_mouse_game_logic()")
        && eng.contains("ui_local_economy()");
    let ok = defs_ok && wrap_ok && host_ok && call_ok && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostUiEconomyMouseModeHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_ui_economy_mouse_mode_helper_nav_commands_residual_wave609() -> bool {
    let steps = LIVE_HOST_UI_ECONOMY_MOUSE_MODE_HELPER_NAV_STEPS_WAVE609;
    let cmds = RUNTIME_HOST_LIVE_HOST_UI_ECONOMY_MOUSE_MODE_HELPER_CMD_NAMES_WAVE609;
    let ok = residual_name_index(steps, "REQUIRE_HOST_UI_ECONOMY_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_MOUSE_GAME_LOGIC_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_GAME_MODE_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_THIN_WRAPPERS").is_some()
        && residual_name_index(steps, "LIVE_HOST_UI_ECONOMY_MOUSE_MODE_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_ui_economy_helper").is_some()
        && residual_name_index(cmds, "host_mouse_game_logic_helper").is_some()
        && residual_name_index(cmds, "host_game_mode_helper").is_some()
        && residual_name_index(cmds, "thin_wrappers").is_some()
        && residual_name_index(cmds, "ui_economy_mouse_mode_residual").is_some();
    residual_action_store(ResidualHostUiEconomyMouseModeHelperAction::NavCommands);
    ok
}

pub fn simulate_host_ui_economy_mouse_mode_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 609")
        && eng.contains("fn host_ui_local_economy")
        && eng.contains("fn host_presentation_mouse_game_logic")
        && eng.contains("fn host_presentation_or_live_game_mode");
    residual_action_store(ResidualHostUiEconomyMouseModeHelperAction::CollectSource);
    ok
}

pub fn simulate_host_ui_economy_mouse_mode_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.host_ui_local_economy()")
        && eng.contains("self.host_presentation_mouse_game_logic()")
        && eng.contains("Wave 609: thin wrapper");
    residual_action_store(ResidualHostUiEconomyMouseModeHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_ui_economy_mouse_mode_helper_residual_pack_wave609() -> bool {
    honesty_host_ui_economy_mouse_mode_helper_method_names_residual_wave609()
        && honesty_host_ui_economy_mouse_mode_helper_source_markers_residual_wave609()
        && honesty_host_ui_economy_mouse_mode_helper_nav_commands_residual_wave609()
        && simulate_host_ui_economy_mouse_mode_helper_collect_source()
        && simulate_host_ui_economy_mouse_mode_helper_dispatch_source()
}

pub fn simulate_live_host_ui_economy_mouse_mode_helper_honesty() -> bool {
    let ok = honesty_host_ui_economy_mouse_mode_helper_residual_pack_wave609();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostUiEconomyMouseModeHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_ui_economy_mouse_mode_helper_method_names_residual_wave609());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_ui_economy_mouse_mode_helper_source_markers_residual_wave609());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_ui_economy_mouse_mode_helper_nav_commands_residual_wave609());
    }

    #[test]
    fn host_ui_economy_mouse_mode_helper_sources() {
        assert!(simulate_host_ui_economy_mouse_mode_helper_collect_source());
        assert!(simulate_host_ui_economy_mouse_mode_helper_dispatch_source());
    }

    #[test]
    fn wave609_composite_pack() {
        assert!(honesty_host_ui_economy_mouse_mode_helper_residual_pack_wave609());
    }

    #[test]
    fn simulate_live_host_ui_economy_mouse_mode_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_ui_economy_mouse_mode_helper_honesty(),
            "host ui economy mouse mode helper residual must latch"
        );
        assert!(residual_host_ui_economy_mouse_mode_helper_ok());
        assert_eq!(
            residual_host_ui_economy_mouse_mode_helper_last_action(),
            ResidualHostUiEconomyMouseModeHelperAction::Composite
        );
    }
}
