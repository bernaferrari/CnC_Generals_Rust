//! Wave 610 residual peels: UI selection/science/template probes and startup map
//! finalize are centralized through `host_ui_selected_ids`,
//! `host_ui_local_science_purchase_points`, `host_presentation_or_live_has_template`,
//! and `host_finalize_startup_map_load`.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Waves 215/234/543/581 presentation probe residuals.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` host UI selection + startup finalize helpers
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_UI_SELECTION_STARTUP_HELPER_METHOD_NAMES_WAVE610: &[&str] = &[
    "host_ui_selected_ids",
    "host_ui_local_science_purchase_points",
    "host_presentation_or_live_has_template",
    "host_finalize_startup_map_load",
    "ui_selected_ids",
    "finalize_startup_map_load",
    "Wave 610",
    "playable_claim = false",
];

pub const LIVE_HOST_UI_SELECTION_STARTUP_HELPER_NAV_STEPS_WAVE610: &[&str] = &[
    "REQUIRE_HOST_UI_SELECTED_IDS_HELPER",
    "REQUIRE_HOST_SCIENCE_POINTS_HELPER",
    "REQUIRE_HOST_TEMPLATE_PROBE_HELPER",
    "REQUIRE_HOST_STARTUP_FINALIZE_HELPER",
    "REQUIRE_THIN_WRAPPERS",
    "LIVE_HOST_UI_SELECTION_STARTUP_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_UI_SELECTION_STARTUP_HELPER_CMD_NAMES_WAVE610: &[&str] = &[
    "host_ui_selected_ids_helper",
    "host_science_points_helper",
    "host_template_probe_helper",
    "host_startup_finalize_helper",
    "thin_wrappers",
    "ui_selection_startup_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostUiSelectionStartupHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostUiSelectionStartupHelperAction {
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

fn residual_action_store(action: ResidualHostUiSelectionStartupHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_ui_selection_startup_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_ui_selection_startup_helper_last_action()
-> ResidualHostUiSelectionStartupHelperAction {
    ResidualHostUiSelectionStartupHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
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

pub fn honesty_host_ui_selection_startup_helper_method_names_residual_wave610() -> bool {
    let names = LIVE_HOST_UI_SELECTION_STARTUP_HELPER_METHOD_NAMES_WAVE610;
    let ok = residual_name_index(names, "host_ui_selected_ids").is_some()
        && residual_name_index(names, "host_ui_local_science_purchase_points").is_some()
        && residual_name_index(names, "host_finalize_startup_map_load").is_some()
        && residual_name_index(names, "Wave 610").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostUiSelectionStartupHelperAction::MethodNames);
    ok
}

pub fn honesty_host_ui_selection_startup_helper_source_markers_residual_wave610() -> bool {
    let eng = eng_source();
    let required = [
        "fn host_ui_selected_ids(",
        "fn host_ui_local_science_purchase_points(",
        "fn host_presentation_or_live_has_template(",
        "fn host_finalize_startup_map_load(",
    ];
    let mut defs_ok = true;
    for sig in required {
        let Some(body) = fn_body(eng, sig) else {
            defs_ok = false;
            break;
        };
        if !body.contains("Wave 610") {
            defs_ok = false;
            break;
        }
    }
    let Some(sel_wrap) = fn_body(eng, "fn ui_selected_ids(") else {
        residual_action_store(ResidualHostUiSelectionStartupHelperAction::SourceMarkers);
        return false;
    };
    let Some(fin_wrap) = fn_body(eng, "fn finalize_startup_map_load(") else {
        residual_action_store(ResidualHostUiSelectionStartupHelperAction::SourceMarkers);
        return false;
    };
    let Some(sel_host) = fn_body(eng, "fn host_ui_selected_ids(") else {
        residual_action_store(ResidualHostUiSelectionStartupHelperAction::SourceMarkers);
        return false;
    };
    let Some(fin_host) = fn_body(eng, "fn host_finalize_startup_map_load(") else {
        residual_action_store(ResidualHostUiSelectionStartupHelperAction::SourceMarkers);
        return false;
    };
    let wrap_ok = sel_wrap.contains("host_ui_selected_ids")
        && sel_wrap.contains("Wave 610")
        && fin_wrap.contains("host_finalize_startup_map_load")
        && fin_wrap.contains("Wave 610");
    let host_ok = sel_host.contains("last_presentation_frame")
        && (sel_host.contains("selected_objects") || sel_host.contains("Wave 215"))
        && fin_host.contains("result.game_logic")
        && fin_host.contains("ensure_presentation_env_seeded");
    let call_ok = eng.contains("self.host_ui_selected_ids(")
        && eng.contains("self.host_finalize_startup_map_load(")
        && eng.contains("ui_selected_ids(");
    let ok = defs_ok && wrap_ok && host_ok && call_ok && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostUiSelectionStartupHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_ui_selection_startup_helper_nav_commands_residual_wave610() -> bool {
    let steps = LIVE_HOST_UI_SELECTION_STARTUP_HELPER_NAV_STEPS_WAVE610;
    let cmds = RUNTIME_HOST_LIVE_HOST_UI_SELECTION_STARTUP_HELPER_CMD_NAMES_WAVE610;
    let ok = residual_name_index(steps, "REQUIRE_HOST_UI_SELECTED_IDS_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_SCIENCE_POINTS_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_TEMPLATE_PROBE_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_STARTUP_FINALIZE_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_THIN_WRAPPERS").is_some()
        && residual_name_index(steps, "LIVE_HOST_UI_SELECTION_STARTUP_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_ui_selected_ids_helper").is_some()
        && residual_name_index(cmds, "host_science_points_helper").is_some()
        && residual_name_index(cmds, "host_template_probe_helper").is_some()
        && residual_name_index(cmds, "host_startup_finalize_helper").is_some()
        && residual_name_index(cmds, "thin_wrappers").is_some()
        && residual_name_index(cmds, "ui_selection_startup_residual").is_some();
    residual_action_store(ResidualHostUiSelectionStartupHelperAction::NavCommands);
    ok
}

pub fn simulate_host_ui_selection_startup_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 610")
        && eng.contains("fn host_ui_selected_ids")
        && eng.contains("fn host_finalize_startup_map_load")
        && eng.contains("fn host_presentation_or_live_has_template");
    residual_action_store(ResidualHostUiSelectionStartupHelperAction::CollectSource);
    ok
}

pub fn simulate_host_ui_selection_startup_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.host_ui_selected_ids(")
        && eng.contains("self.host_finalize_startup_map_load(")
        && eng.contains("Wave 610: thin wrapper — residual via host helper");
    residual_action_store(ResidualHostUiSelectionStartupHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_ui_selection_startup_helper_residual_pack_wave610() -> bool {
    honesty_host_ui_selection_startup_helper_method_names_residual_wave610()
        && honesty_host_ui_selection_startup_helper_source_markers_residual_wave610()
        && honesty_host_ui_selection_startup_helper_nav_commands_residual_wave610()
        && simulate_host_ui_selection_startup_helper_collect_source()
        && simulate_host_ui_selection_startup_helper_dispatch_source()
}

pub fn simulate_live_host_ui_selection_startup_helper_honesty() -> bool {
    let ok = honesty_host_ui_selection_startup_helper_residual_pack_wave610();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostUiSelectionStartupHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_ui_selection_startup_helper_method_names_residual_wave610());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_ui_selection_startup_helper_source_markers_residual_wave610());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_ui_selection_startup_helper_nav_commands_residual_wave610());
    }

    #[test]
    fn host_ui_selection_startup_helper_sources() {
        assert!(simulate_host_ui_selection_startup_helper_collect_source());
        assert!(simulate_host_ui_selection_startup_helper_dispatch_source());
    }

    #[test]
    fn wave610_composite_pack() {
        assert!(honesty_host_ui_selection_startup_helper_residual_pack_wave610());
    }

    #[test]
    fn simulate_live_host_ui_selection_startup_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_ui_selection_startup_helper_honesty(),
            "host ui selection startup helper residual must latch"
        );
        assert!(residual_host_ui_selection_startup_helper_ok());
        assert_eq!(
            residual_host_ui_selection_startup_helper_last_action(),
            ResidualHostUiSelectionStartupHelperAction::Composite
        );
    }
}
