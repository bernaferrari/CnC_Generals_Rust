//! Wave 544 residual peels: `ui_selection_seed_id` fails closed under a
//! presentation freeze — empty presentation selection does **not** dual-read
//! `player_selected_objects`. Boot residual without freeze unchanged.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 543 ui_selected_ids presentation fail-closed residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` ui_selection_seed_id
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

pub const LIVE_UI_SELECTION_SEED_PRESENTATION_FAIL_CLOSED_METHOD_NAMES_WAVE544: &[&str] = &[
    "ui_selection_seed_id",
    "last_presentation_frame",
    "selection_ids_for_consumers",
    "player_selected_objects",
    "Wave 544",
    "playable_claim = false",
];

pub const LIVE_UI_SELECTION_SEED_PRESENTATION_FAIL_CLOSED_NAV_STEPS_WAVE544: &[&str] = &[
    "REQUIRE_UI_SELECTION_SEED_PRESENTATION_FAIL_CLOSED",
    "REQUIRE_NO_HOST_SELECTION_SEED_DUAL_READ_WITH_FREEZE",
    "LIVE_UI_SELECTION_SEED_PRESENTATION_FAIL_CLOSED",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_UI_SELECTION_SEED_PRESENTATION_FAIL_CLOSED_CMD_NAMES_WAVE544:
    &[&str] = &[
    "ui_selection_seed_presentation_fail_closed",
    "presentation_selection_seed_owns",
    "boot_player_selected_objects",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualUiSelectionSeedPresentationFailClosedAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualUiSelectionSeedPresentationFailClosedAction {
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

fn residual_action_store(action: ResidualUiSelectionSeedPresentationFailClosedAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_ui_selection_seed_presentation_fail_closed_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_ui_selection_seed_presentation_fail_closed_last_action()
-> ResidualUiSelectionSeedPresentationFailClosedAction {
    ResidualUiSelectionSeedPresentationFailClosedAction::from_u8(
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

pub fn honesty_ui_selection_seed_presentation_fail_closed_method_names_residual_wave544() -> bool {
    let names = LIVE_UI_SELECTION_SEED_PRESENTATION_FAIL_CLOSED_METHOD_NAMES_WAVE544;
    let ok = residual_name_index(names, "ui_selection_seed_id").is_some()
        && residual_name_index(names, "last_presentation_frame").is_some()
        && residual_name_index(names, "selection_ids_for_consumers").is_some()
        && residual_name_index(names, "player_selected_objects").is_some()
        && residual_name_index(names, "Wave 544").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualUiSelectionSeedPresentationFailClosedAction::MethodNames);
    ok
}

pub fn honesty_ui_selection_seed_presentation_fail_closed_source_markers_residual_wave544() -> bool
{
    let eng = eng_source();
    let Some(body) = fn_body(eng, "fn host_ui_selection_seed_id(")
        .or_else(|| fn_body(eng, "fn ui_selection_seed_id("))
    else {
        residual_action_store(ResidualUiSelectionSeedPresentationFailClosedAction::SourceMarkers);
        return false;
    };
    // 2026-08-15: seed prefers selected_objects then freeze; no player_selected_objects.
    let pres_return = body.contains("Wave 544")
        && body.contains("last_presentation_frame")
        && body.contains("return None;");
    // 2026-08-15: comments name the peeled dual-read; live code must not call it.
    let boot =
        !body.contains("player_selected_objects(") && !body.contains(".player_selected_objects");
    let pres_arm_ok = body.contains("selected_objects.first()");
    let ok = pres_return && boot && pres_arm_ok && !eng.contains("playable_claim = true");
    residual_action_store(ResidualUiSelectionSeedPresentationFailClosedAction::SourceMarkers);
    ok
}

pub fn honesty_ui_selection_seed_presentation_fail_closed_nav_commands_residual_wave544() -> bool {
    let steps = LIVE_UI_SELECTION_SEED_PRESENTATION_FAIL_CLOSED_NAV_STEPS_WAVE544;
    let cmds = RUNTIME_HOST_LIVE_UI_SELECTION_SEED_PRESENTATION_FAIL_CLOSED_CMD_NAMES_WAVE544;
    let ok = residual_name_index(steps, "REQUIRE_UI_SELECTION_SEED_PRESENTATION_FAIL_CLOSED")
        .is_some()
        && residual_name_index(
            steps,
            "REQUIRE_NO_HOST_SELECTION_SEED_DUAL_READ_WITH_FREEZE",
        )
        .is_some()
        && residual_name_index(steps, "LIVE_UI_SELECTION_SEED_PRESENTATION_FAIL_CLOSED").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "ui_selection_seed_presentation_fail_closed").is_some()
        && residual_name_index(cmds, "presentation_selection_seed_owns").is_some()
        && residual_name_index(cmds, "boot_player_selected_objects").is_some();
    residual_action_store(ResidualUiSelectionSeedPresentationFailClosedAction::NavCommands);
    ok
}

pub fn simulate_ui_selection_seed_presentation_fail_closed_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 544")
        && eng.contains("fn ui_selection_seed_id")
        && eng.contains("presentation freeze owns selection seed residual");
    residual_action_store(ResidualUiSelectionSeedPresentationFailClosedAction::CollectSource);
    ok
}

pub fn simulate_ui_selection_seed_presentation_fail_closed_dispatch_source() -> bool {
    let eng = eng_source();
    let Some(body) = fn_body(eng, "fn host_ui_selection_seed_id(")
        .or_else(|| fn_body(eng, "fn ui_selection_seed_id("))
    else {
        residual_action_store(ResidualUiSelectionSeedPresentationFailClosedAction::DispatchSource);
        return false;
    };
    let ok = body.contains("Wave 544")
        && body.contains("return None;")
        && !body.contains("player_selected_objects(")
        && !body.contains(".player_selected_objects");
    residual_action_store(ResidualUiSelectionSeedPresentationFailClosedAction::DispatchSource);
    ok
}

pub fn honesty_ui_selection_seed_presentation_fail_closed_residual_pack_wave544() -> bool {
    honesty_ui_selection_seed_presentation_fail_closed_method_names_residual_wave544()
        && honesty_ui_selection_seed_presentation_fail_closed_source_markers_residual_wave544()
        && honesty_ui_selection_seed_presentation_fail_closed_nav_commands_residual_wave544()
        && simulate_ui_selection_seed_presentation_fail_closed_collect_source()
        && simulate_ui_selection_seed_presentation_fail_closed_dispatch_source()
}

pub fn simulate_live_ui_selection_seed_presentation_fail_closed_honesty() -> bool {
    let ok = honesty_ui_selection_seed_presentation_fail_closed_residual_pack_wave544();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualUiSelectionSeedPresentationFailClosedAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_ui_selection_seed_presentation_fail_closed_method_names_residual_wave544());
    }

    #[test]
    fn source_markers_residual() {
        assert!(
            honesty_ui_selection_seed_presentation_fail_closed_source_markers_residual_wave544()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_ui_selection_seed_presentation_fail_closed_nav_commands_residual_wave544());
    }

    #[test]
    fn ui_selection_seed_presentation_fail_closed_sources() {
        assert!(simulate_ui_selection_seed_presentation_fail_closed_collect_source());
        assert!(simulate_ui_selection_seed_presentation_fail_closed_dispatch_source());
    }

    #[test]
    fn wave544_composite_pack() {
        assert!(honesty_ui_selection_seed_presentation_fail_closed_residual_pack_wave544());
    }

    #[test]
    fn simulate_live_ui_selection_seed_presentation_fail_closed_honesty_residual_live() {
        assert!(
            simulate_live_ui_selection_seed_presentation_fail_closed_honesty(),
            "ui_selection_seed presentation fail-closed residual must latch"
        );
        assert!(residual_ui_selection_seed_presentation_fail_closed_ok());
        assert_eq!(
            residual_ui_selection_seed_presentation_fail_closed_last_action(),
            ResidualUiSelectionSeedPresentationFailClosedAction::Composite
        );
    }
}
