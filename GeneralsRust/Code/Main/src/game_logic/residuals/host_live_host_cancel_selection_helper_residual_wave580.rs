//! Wave 580 residual peels: host cancel-production residual is centralized
//! through `host_cancel_production_and_sync_hud`, and remaining selection
//! dual-writes route through `host_set_selection` (Wave 579). Never flips shell
//! `playable_claim`.
//!
//! Orthogonal to Wave 579 host selection/map helper residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` host_cancel_production_and_sync_hud / host_set_selection
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_CANCEL_SELECTION_HELPER_METHOD_NAMES_WAVE580: &[&str] = &[
    "host_cancel_production_and_sync_hud",
    "host_set_selection",
    "cancel_production",
    "select_objects",
    "Wave 580",
    "playable_claim = false",
];

pub const LIVE_HOST_CANCEL_SELECTION_HELPER_NAV_STEPS_WAVE580: &[&str] = &[
    "REQUIRE_HOST_CANCEL_PRODUCTION_HELPER",
    "REQUIRE_HOST_SET_SELECTION_COVERAGE",
    "LIVE_HOST_CANCEL_SELECTION_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_CANCEL_SELECTION_HELPER_CMD_NAMES_WAVE580: &[&str] = &[
    "host_cancel_production_helper",
    "host_set_selection_helper",
    "cancel_selection_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostCancelSelectionHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostCancelSelectionHelperAction {
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

fn residual_action_store(action: ResidualHostCancelSelectionHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_cancel_selection_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_cancel_selection_helper_last_action() -> ResidualHostCancelSelectionHelperAction
{
    ResidualHostCancelSelectionHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
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
pub fn honesty_host_cancel_selection_helper_method_names_residual_wave580() -> bool {
    let names = LIVE_HOST_CANCEL_SELECTION_HELPER_METHOD_NAMES_WAVE580;
    let ok = residual_name_index(names, "host_cancel_production_and_sync_hud").is_some()
        && residual_name_index(names, "host_set_selection").is_some()
        && residual_name_index(names, "cancel_production").is_some()
        && residual_name_index(names, "select_objects").is_some()
        && residual_name_index(names, "Wave 580").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostCancelSelectionHelperAction::MethodNames);
    ok
}

pub fn honesty_host_cancel_selection_helper_source_markers_residual_wave580() -> bool {
    let eng = eng_source();
    let Some(cancel) = fn_body(eng, "fn host_cancel_production_and_sync_hud(") else {
        residual_action_store(ResidualHostCancelSelectionHelperAction::SourceMarkers);
        return false;
    };
    let Some(sel) = fn_body(eng, "fn host_set_selection(") else {
        residual_action_store(ResidualHostCancelSelectionHelperAction::SourceMarkers);
        return false;
    };
    let cancel_ok =
        cancel.contains("Wave 580") && cancel.contains("ObjectLifecycleOp::CancelProduction");
    let sel_ok = sel.contains("Wave 579")
        && sel.contains("SessionControlOp::SelectObjects")
        && sel.contains("self.selected_objects = ids");
    let call_ok = eng.contains("self.host_cancel_production_and_sync_hud(")
        && eng.contains("self.host_set_selection(");
    let raw_cancel = eng.matches("self.game_logic.cancel_production").count();
    let raw_sel = eng.matches("self.game_logic.select_objects").count();
    let ok = cancel_ok
        && sel_ok
        && call_ok
        && raw_cancel == 0
        && raw_sel == 0
        && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostCancelSelectionHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_cancel_selection_helper_nav_commands_residual_wave580() -> bool {
    let steps = LIVE_HOST_CANCEL_SELECTION_HELPER_NAV_STEPS_WAVE580;
    let cmds = RUNTIME_HOST_LIVE_HOST_CANCEL_SELECTION_HELPER_CMD_NAMES_WAVE580;
    let ok = residual_name_index(steps, "REQUIRE_HOST_CANCEL_PRODUCTION_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_SET_SELECTION_COVERAGE").is_some()
        && residual_name_index(steps, "LIVE_HOST_CANCEL_SELECTION_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_cancel_production_helper").is_some()
        && residual_name_index(cmds, "host_set_selection_helper").is_some()
        && residual_name_index(cmds, "cancel_selection_residual").is_some();
    residual_action_store(ResidualHostCancelSelectionHelperAction::NavCommands);
    ok
}

pub fn simulate_host_cancel_selection_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 580")
        && eng.contains("fn host_cancel_production_and_sync_hud")
        && eng.contains("fn host_set_selection");
    residual_action_store(ResidualHostCancelSelectionHelperAction::CollectSource);
    ok
}

pub fn simulate_host_cancel_selection_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.host_cancel_production_and_sync_hud(")
        && eng.contains("cancel_selected_production_queue_head")
        && eng.contains("cancel_all_selected_production")
        && eng.contains("self.host_set_selection(self.current_player_id, mobile_sel.clone())");
    residual_action_store(ResidualHostCancelSelectionHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_cancel_selection_helper_residual_pack_wave580() -> bool {
    honesty_host_cancel_selection_helper_method_names_residual_wave580()
        && honesty_host_cancel_selection_helper_source_markers_residual_wave580()
        && honesty_host_cancel_selection_helper_nav_commands_residual_wave580()
        && simulate_host_cancel_selection_helper_collect_source()
        && simulate_host_cancel_selection_helper_dispatch_source()
}

pub fn simulate_live_host_cancel_selection_helper_honesty() -> bool {
    let ok = honesty_host_cancel_selection_helper_residual_pack_wave580();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostCancelSelectionHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_cancel_selection_helper_method_names_residual_wave580());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_cancel_selection_helper_source_markers_residual_wave580());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_cancel_selection_helper_nav_commands_residual_wave580());
    }

    #[test]
    fn host_cancel_selection_helper_sources() {
        assert!(simulate_host_cancel_selection_helper_collect_source());
        assert!(simulate_host_cancel_selection_helper_dispatch_source());
    }

    #[test]
    fn wave580_composite_pack() {
        assert!(honesty_host_cancel_selection_helper_residual_pack_wave580());
    }

    #[test]
    fn simulate_live_host_cancel_selection_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_cancel_selection_helper_honesty(),
            "host cancel/selection helper residual must latch"
        );
        assert!(residual_host_cancel_selection_helper_ok());
        assert_eq!(
            residual_host_cancel_selection_helper_last_action(),
            ResidualHostCancelSelectionHelperAction::Composite
        );
    }
}
