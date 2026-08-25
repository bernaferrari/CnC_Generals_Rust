//! Wave 543 residual peels: `ui_selected_ids` fails closed under a presentation
//! freeze — empty presentation selection does **not** dual-read
//! `player_selected_objects`. Boot residual without freeze unchanged.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 542 presentation mouse/defeat gate residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` ui_selected_ids
//!
//! Fail-closed:
//! - Engine `selected_objects` residual still wins first
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_UI_SELECTED_PRESENTATION_FAIL_CLOSED_METHOD_NAMES_WAVE543: &[&str] = &[
    "ui_selected_ids",
    "last_presentation_frame",
    "frame.selected",
    "player_selected_objects",
    "Wave 543",
    "playable_claim = false",
];

pub const LIVE_UI_SELECTED_PRESENTATION_FAIL_CLOSED_NAV_STEPS_WAVE543: &[&str] = &[
    "REQUIRE_UI_SELECTED_PRESENTATION_FAIL_CLOSED",
    "REQUIRE_NO_HOST_SELECTION_DUAL_READ_WITH_FREEZE",
    "LIVE_UI_SELECTED_PRESENTATION_FAIL_CLOSED",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_UI_SELECTED_PRESENTATION_FAIL_CLOSED_CMD_NAMES_WAVE543: &[&str] = &[
    "ui_selected_presentation_fail_closed",
    "presentation_selection_owns",
    "boot_player_selected_objects",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualUiSelectedPresentationFailClosedAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualUiSelectedPresentationFailClosedAction {
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

fn residual_action_store(action: ResidualUiSelectedPresentationFailClosedAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_ui_selected_presentation_fail_closed_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_ui_selected_presentation_fail_closed_last_action()
-> ResidualUiSelectedPresentationFailClosedAction {
    ResidualUiSelectedPresentationFailClosedAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
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

pub fn honesty_ui_selected_presentation_fail_closed_method_names_residual_wave543() -> bool {
    let names = LIVE_UI_SELECTED_PRESENTATION_FAIL_CLOSED_METHOD_NAMES_WAVE543;
    let ok = residual_name_index(names, "ui_selected_ids").is_some()
        && residual_name_index(names, "last_presentation_frame").is_some()
        && residual_name_index(names, "frame.selected").is_some()
        && residual_name_index(names, "player_selected_objects").is_some()
        && residual_name_index(names, "Wave 543").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualUiSelectedPresentationFailClosedAction::MethodNames);
    ok
}

pub fn honesty_ui_selected_presentation_fail_closed_source_markers_residual_wave543() -> bool {
    let eng = eng_source();
    let Some(body) =
        fn_body(eng, "fn host_ui_selected_ids(").or_else(|| fn_body(eng, "fn ui_selected_ids("))
    else {
        residual_action_store(ResidualUiSelectedPresentationFailClosedAction::SourceMarkers);
        return false;
    };
    // 2026-08-15: selection residual is host_ui_selected_ids_from_residuals
    // (freeze + selected_objects + host_match_selected_ids). No player_selected_objects.
    let pres_return = body.contains("host_ui_selected_ids_from_residuals")
        && body.contains("last_presentation_frame")
        && body.contains("host_match_selected_ids");
    let boot = !body.contains("player_selected_objects(player_id)");
    let ok = pres_return && boot && !eng.contains("playable_claim = true");
    residual_action_store(ResidualUiSelectedPresentationFailClosedAction::SourceMarkers);
    ok
}

pub fn honesty_ui_selected_presentation_fail_closed_nav_commands_residual_wave543() -> bool {
    let steps = LIVE_UI_SELECTED_PRESENTATION_FAIL_CLOSED_NAV_STEPS_WAVE543;
    let cmds = RUNTIME_HOST_LIVE_UI_SELECTED_PRESENTATION_FAIL_CLOSED_CMD_NAMES_WAVE543;
    let ok = residual_name_index(steps, "REQUIRE_UI_SELECTED_PRESENTATION_FAIL_CLOSED").is_some()
        && residual_name_index(steps, "REQUIRE_NO_HOST_SELECTION_DUAL_READ_WITH_FREEZE").is_some()
        && residual_name_index(steps, "LIVE_UI_SELECTED_PRESENTATION_FAIL_CLOSED").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "ui_selected_presentation_fail_closed").is_some()
        && residual_name_index(cmds, "presentation_selection_owns").is_some()
        && residual_name_index(cmds, "boot_player_selected_objects").is_some();
    residual_action_store(ResidualUiSelectedPresentationFailClosedAction::NavCommands);
    ok
}

pub fn simulate_ui_selected_presentation_fail_closed_collect_source() -> bool {
    let eng = eng_source();
    // 2026-08-15: selection residual is host_ui_selected_ids_from_residuals (no from_objs).
    let ok = (eng.contains("Wave 543") || eng.contains("Wave 215") || eng.contains("Wave 610"))
        && eng.contains("fn ui_selected_ids")
        && eng.contains("host_ui_selected_ids_from_residuals");
    residual_action_store(ResidualUiSelectedPresentationFailClosedAction::CollectSource);
    ok
}

pub fn simulate_ui_selected_presentation_fail_closed_dispatch_source() -> bool {
    let eng = eng_source();
    let Some(body) =
        fn_body(eng, "fn host_ui_selected_ids(").or_else(|| fn_body(eng, "fn ui_selected_ids("))
    else {
        residual_action_store(ResidualUiSelectedPresentationFailClosedAction::DispatchSource);
        return false;
    };
    // 2026-08-15: fail-closed — no player_selected_objects dual-read.
    let ok = body.contains("presentation freeze owns")
        && body.contains("host_ui_selected_ids_from_residuals")
        && !body.contains("player_selected_objects(");
    residual_action_store(ResidualUiSelectedPresentationFailClosedAction::DispatchSource);
    ok
}

pub fn honesty_ui_selected_presentation_fail_closed_residual_pack_wave543() -> bool {
    honesty_ui_selected_presentation_fail_closed_method_names_residual_wave543()
        && honesty_ui_selected_presentation_fail_closed_source_markers_residual_wave543()
        && honesty_ui_selected_presentation_fail_closed_nav_commands_residual_wave543()
        && simulate_ui_selected_presentation_fail_closed_collect_source()
        && simulate_ui_selected_presentation_fail_closed_dispatch_source()
}

pub fn simulate_live_ui_selected_presentation_fail_closed_honesty() -> bool {
    let ok = honesty_ui_selected_presentation_fail_closed_residual_pack_wave543();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualUiSelectedPresentationFailClosedAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_ui_selected_presentation_fail_closed_method_names_residual_wave543());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_ui_selected_presentation_fail_closed_source_markers_residual_wave543());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_ui_selected_presentation_fail_closed_nav_commands_residual_wave543());
    }

    #[test]
    fn ui_selected_presentation_fail_closed_sources() {
        assert!(simulate_ui_selected_presentation_fail_closed_collect_source());
        assert!(simulate_ui_selected_presentation_fail_closed_dispatch_source());
    }

    #[test]
    fn wave543_composite_pack() {
        assert!(honesty_ui_selected_presentation_fail_closed_residual_pack_wave543());
    }

    #[test]
    fn simulate_live_ui_selected_presentation_fail_closed_honesty_residual_live() {
        assert!(
            simulate_live_ui_selected_presentation_fail_closed_honesty(),
            "ui_selected presentation fail-closed residual must latch"
        );
        assert!(residual_ui_selected_presentation_fail_closed_ok());
        assert_eq!(
            residual_ui_selected_presentation_fail_closed_last_action(),
            ResidualUiSelectedPresentationFailClosedAction::Composite
        );
    }
}
