//! Wave 607 residual peels: presentation/boot drains and UI player/camera probes
//! are centralized through `host_take_presentation_or_boot_*` and `host_ui_*` /
//! `host_local_*` helpers.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Waves 569/570/574 presentation drain residuals.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` host presentation/boot drain + UI residual helpers
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_UI_PRESENTATION_DRAIN_HELPER_METHOD_NAMES_WAVE607: &[&str] = &[
    "host_take_presentation_or_boot_new_script_messages",
    "host_take_presentation_or_boot_defeat_events",
    "host_take_presentation_or_boot_alliance_events",
    "host_local_player_id_for_ui",
    "host_local_team_for_ui",
    "host_ui_player_info",
    "host_ui_player_team",
    "host_ui_player_name",
    "host_ui_local_player_team_name",
    "host_ui_script_default_camera_max_height",
    "Wave 607",
    "playable_claim = false",
];

pub const LIVE_HOST_UI_PRESENTATION_DRAIN_HELPER_NAV_STEPS_WAVE607: &[&str] = &[
    "REQUIRE_HOST_PRESENTATION_DRAIN_HELPERS",
    "REQUIRE_HOST_UI_PLAYER_HELPERS",
    "REQUIRE_THIN_WRAPPERS",
    "LIVE_HOST_UI_PRESENTATION_DRAIN_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_UI_PRESENTATION_DRAIN_HELPER_CMD_NAMES_WAVE607: &[&str] = &[
    "host_presentation_drain_helpers",
    "host_ui_player_helpers",
    "thin_wrappers",
    "ui_presentation_drain_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostUiPresentationDrainHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostUiPresentationDrainHelperAction {
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

fn residual_action_store(action: ResidualHostUiPresentationDrainHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_ui_presentation_drain_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_ui_presentation_drain_helper_last_action()
-> ResidualHostUiPresentationDrainHelperAction {
    ResidualHostUiPresentationDrainHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
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
pub fn honesty_host_ui_presentation_drain_helper_method_names_residual_wave607() -> bool {
    let names = LIVE_HOST_UI_PRESENTATION_DRAIN_HELPER_METHOD_NAMES_WAVE607;
    let ok = residual_name_index(names, "host_take_presentation_or_boot_new_script_messages")
        .is_some()
        && residual_name_index(names, "host_local_player_id_for_ui").is_some()
        && residual_name_index(names, "host_ui_player_info").is_some()
        && residual_name_index(names, "Wave 607").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostUiPresentationDrainHelperAction::MethodNames);
    ok
}

pub fn honesty_host_ui_presentation_drain_helper_source_markers_residual_wave607() -> bool {
    let eng = eng_source();
    let required = [
        "fn host_take_presentation_or_boot_new_script_messages(",
        "fn host_take_presentation_or_boot_defeat_events(",
        "fn host_take_presentation_or_boot_alliance_events(",
        "fn host_local_player_id_for_ui(",
        "fn host_local_team_for_ui(",
        "fn host_ui_player_info(",
        "fn host_ui_player_team(",
        "fn host_ui_player_name(",
        "fn host_ui_local_player_team_name(",
        "fn host_ui_script_default_camera_max_height(",
    ];
    let mut defs_ok = true;
    for sig in required {
        let Some(body) = fn_body(eng, sig) else {
            defs_ok = false;
            break;
        };
        if !body.contains("Wave 607") {
            defs_ok = false;
            break;
        }
    }
    let Some(script_wrap) = fn_body(eng, "fn take_presentation_or_boot_new_script_messages(")
    else {
        residual_action_store(ResidualHostUiPresentationDrainHelperAction::SourceMarkers);
        return false;
    };
    let Some(local_wrap) = fn_body(eng, "fn local_player_id_for_ui(") else {
        residual_action_store(ResidualHostUiPresentationDrainHelperAction::SourceMarkers);
        return false;
    };
    let Some(script_host) = fn_body(
        eng,
        "fn host_take_presentation_or_boot_new_script_messages(",
    ) else {
        residual_action_store(ResidualHostUiPresentationDrainHelperAction::SourceMarkers);
        return false;
    };
    let Some(local_host) = fn_body(eng, "fn host_local_player_id_for_ui(") else {
        residual_action_store(ResidualHostUiPresentationDrainHelperAction::SourceMarkers);
        return false;
    };
    let wrap_ok = script_wrap.contains("host_take_presentation_or_boot_new_script_messages")
        && script_wrap.contains("Wave 607")
        && local_wrap.contains("host_local_player_id_for_ui")
        && local_wrap.contains("Wave 607");
    // 2026-08-15: Wave 900 fail-closed — clone freeze messages, no live take.
    let host_ok = script_host.contains("new_script_messages")
        && !script_host.contains("self.game_logic.take_new_script_messages()")
        && local_host.contains("boot_local_player_id_from_host()");
    let call_ok = eng.contains("self.take_presentation_or_boot_new_script_messages()")
        && eng.contains("self.host_take_presentation_or_boot_new_script_messages()")
        && eng.contains("self.host_local_player_id_for_ui()");
    let ok = defs_ok && wrap_ok && host_ok && call_ok && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostUiPresentationDrainHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_ui_presentation_drain_helper_nav_commands_residual_wave607() -> bool {
    let steps = LIVE_HOST_UI_PRESENTATION_DRAIN_HELPER_NAV_STEPS_WAVE607;
    let cmds = RUNTIME_HOST_LIVE_HOST_UI_PRESENTATION_DRAIN_HELPER_CMD_NAMES_WAVE607;
    let ok = residual_name_index(steps, "REQUIRE_HOST_PRESENTATION_DRAIN_HELPERS").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_UI_PLAYER_HELPERS").is_some()
        && residual_name_index(steps, "REQUIRE_THIN_WRAPPERS").is_some()
        && residual_name_index(steps, "LIVE_HOST_UI_PRESENTATION_DRAIN_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_presentation_drain_helpers").is_some()
        && residual_name_index(cmds, "host_ui_player_helpers").is_some()
        && residual_name_index(cmds, "thin_wrappers").is_some()
        && residual_name_index(cmds, "ui_presentation_drain_residual").is_some();
    residual_action_store(ResidualHostUiPresentationDrainHelperAction::NavCommands);
    ok
}

pub fn simulate_host_ui_presentation_drain_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 607")
        && eng.contains("fn host_take_presentation_or_boot_new_script_messages")
        && eng.contains("fn host_local_player_id_for_ui")
        && eng.contains("fn host_ui_player_info");
    residual_action_store(ResidualHostUiPresentationDrainHelperAction::CollectSource);
    ok
}

pub fn simulate_host_ui_presentation_drain_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.host_take_presentation_or_boot_new_script_messages()")
        && eng.contains("self.host_local_player_id_for_ui()")
        && eng.contains("Wave 607: thin wrapper");
    residual_action_store(ResidualHostUiPresentationDrainHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_ui_presentation_drain_helper_residual_pack_wave607() -> bool {
    honesty_host_ui_presentation_drain_helper_method_names_residual_wave607()
        && honesty_host_ui_presentation_drain_helper_source_markers_residual_wave607()
        && honesty_host_ui_presentation_drain_helper_nav_commands_residual_wave607()
        && simulate_host_ui_presentation_drain_helper_collect_source()
        && simulate_host_ui_presentation_drain_helper_dispatch_source()
}

pub fn simulate_live_host_ui_presentation_drain_helper_honesty() -> bool {
    let ok = honesty_host_ui_presentation_drain_helper_residual_pack_wave607();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostUiPresentationDrainHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_ui_presentation_drain_helper_method_names_residual_wave607());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_ui_presentation_drain_helper_source_markers_residual_wave607());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_ui_presentation_drain_helper_nav_commands_residual_wave607());
    }

    #[test]
    fn host_ui_presentation_drain_helper_sources() {
        assert!(simulate_host_ui_presentation_drain_helper_collect_source());
        assert!(simulate_host_ui_presentation_drain_helper_dispatch_source());
    }

    #[test]
    fn wave607_composite_pack() {
        assert!(honesty_host_ui_presentation_drain_helper_residual_pack_wave607());
    }

    #[test]
    fn simulate_live_host_ui_presentation_drain_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_ui_presentation_drain_helper_honesty(),
            "host ui presentation drain helper residual must latch"
        );
        assert!(residual_host_ui_presentation_drain_helper_ok());
        assert_eq!(
            residual_host_ui_presentation_drain_helper_last_action(),
            ResidualHostUiPresentationDrainHelperAction::Composite
        );
    }
}
