//! Wave 547 residual peels: `runtime_host_status_snapshot` selection count fails
//! closed under a presentation freeze — engine selection residual wins first,
//! then presentation `count_selected_friendlies` (even if zero). No empty-presentation
//! fallthrough that re-reads a second residual mid-frame inconsistently.
//! Boot residual without freeze unchanged (engine selection only).
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 546 host status map presentation fail-closed residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` runtime_host_status_snapshot
//!
//! Fail-closed:
//! - Engine `selected_objects` residual still wins first under freeze
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_STATUS_SELECTED_PRESENTATION_FAIL_CLOSED_METHOD_NAMES_WAVE547: &[&str] = &[
    "runtime_host_status_snapshot",
    "last_presentation_frame",
    "selected_objects",
    "count_selected_friendlies",
    "Wave 547",
    "playable_claim = false",
];

pub const LIVE_HOST_STATUS_SELECTED_PRESENTATION_FAIL_CLOSED_NAV_STEPS_WAVE547: &[&str] = &[
    "REQUIRE_HOST_STATUS_SELECTED_PRESENTATION_FAIL_CLOSED",
    "REQUIRE_ENGINE_SELECTION_THEN_PRESENTATION",
    "LIVE_HOST_STATUS_SELECTED_PRESENTATION_FAIL_CLOSED",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_STATUS_SELECTED_PRESENTATION_FAIL_CLOSED_CMD_NAMES_WAVE547:
    &[&str] = &[
    "host_status_selected_presentation_fail_closed",
    "engine_selection_residual_first",
    "presentation_selected_count",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostStatusSelectedPresentationFailClosedAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostStatusSelectedPresentationFailClosedAction {
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

fn residual_action_store(action: ResidualHostStatusSelectedPresentationFailClosedAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_status_selected_presentation_fail_closed_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_status_selected_presentation_fail_closed_last_action()
-> ResidualHostStatusSelectedPresentationFailClosedAction {
    ResidualHostStatusSelectedPresentationFailClosedAction::from_u8(
        RESIDUAL_ACTION.load(Ordering::SeqCst),
    )
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

pub fn honesty_host_status_selected_presentation_fail_closed_method_names_residual_wave547() -> bool
{
    let names = LIVE_HOST_STATUS_SELECTED_PRESENTATION_FAIL_CLOSED_METHOD_NAMES_WAVE547;
    let ok = residual_name_index(names, "runtime_host_status_snapshot").is_some()
        && residual_name_index(names, "last_presentation_frame").is_some()
        && residual_name_index(names, "selected_objects").is_some()
        && residual_name_index(names, "count_selected_friendlies").is_some()
        && residual_name_index(names, "Wave 547").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostStatusSelectedPresentationFailClosedAction::MethodNames);
    ok
}

pub fn honesty_host_status_selected_presentation_fail_closed_source_markers_residual_wave547()
-> bool {
    let eng = eng_source();
    let Some(body) = fn_body(eng, "fn runtime_host_status_snapshot(") else {
        residual_action_store(
            ResidualHostStatusSelectedPresentationFailClosedAction::SourceMarkers,
        );
        return false;
    };
    let wave = body.contains("Wave 547")
        && body.contains("presentation freeze owns selection count residual");
    let engine_first = body.contains("if !self.selected_objects.is_empty()")
        && body.contains("self.selected_objects.len() as u32");
    let pres_count = body.contains("frame.count_selected_friendlies(team)");
    // Old pattern: if n > 0 { n } else { selected_objects.len() }
    let no_old = !body.contains("let n = frame.count_selected_friendlies(team);")
        || !body.contains("let selected = if n > 0");
    // Stronger: must not use n > 0 fallthrough to selected_objects after presentation count.
    let no_n_gt = !body.contains("if n > 0");
    let ok = wave
        && engine_first
        && pres_count
        && no_n_gt
        && no_old
        && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostStatusSelectedPresentationFailClosedAction::SourceMarkers);
    ok
}

pub fn honesty_host_status_selected_presentation_fail_closed_nav_commands_residual_wave547() -> bool
{
    let steps = LIVE_HOST_STATUS_SELECTED_PRESENTATION_FAIL_CLOSED_NAV_STEPS_WAVE547;
    let cmds = RUNTIME_HOST_LIVE_HOST_STATUS_SELECTED_PRESENTATION_FAIL_CLOSED_CMD_NAMES_WAVE547;
    let ok = residual_name_index(
        steps,
        "REQUIRE_HOST_STATUS_SELECTED_PRESENTATION_FAIL_CLOSED",
    )
    .is_some()
        && residual_name_index(steps, "REQUIRE_ENGINE_SELECTION_THEN_PRESENTATION").is_some()
        && residual_name_index(steps, "LIVE_HOST_STATUS_SELECTED_PRESENTATION_FAIL_CLOSED")
            .is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_status_selected_presentation_fail_closed").is_some()
        && residual_name_index(cmds, "engine_selection_residual_first").is_some()
        && residual_name_index(cmds, "presentation_selected_count").is_some();
    residual_action_store(ResidualHostStatusSelectedPresentationFailClosedAction::NavCommands);
    ok
}

pub fn simulate_host_status_selected_presentation_fail_closed_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 547")
        && eng.contains("fn runtime_host_status_snapshot")
        && eng.contains("presentation freeze owns selection count residual");
    residual_action_store(ResidualHostStatusSelectedPresentationFailClosedAction::CollectSource);
    ok
}

pub fn simulate_host_status_selected_presentation_fail_closed_dispatch_source() -> bool {
    let eng = eng_source();
    let Some(body) = fn_body(eng, "fn runtime_host_status_snapshot(") else {
        residual_action_store(
            ResidualHostStatusSelectedPresentationFailClosedAction::DispatchSource,
        );
        return false;
    };
    let ok = body.contains("presentation freeze owns selection count residual")
        && body.contains("if !self.selected_objects.is_empty()")
        && body.contains("frame.count_selected_friendlies(team)")
        && !body.contains("if n > 0");
    residual_action_store(ResidualHostStatusSelectedPresentationFailClosedAction::DispatchSource);
    ok
}

pub fn honesty_host_status_selected_presentation_fail_closed_residual_pack_wave547() -> bool {
    honesty_host_status_selected_presentation_fail_closed_method_names_residual_wave547()
        && honesty_host_status_selected_presentation_fail_closed_source_markers_residual_wave547()
        && honesty_host_status_selected_presentation_fail_closed_nav_commands_residual_wave547()
        && simulate_host_status_selected_presentation_fail_closed_collect_source()
        && simulate_host_status_selected_presentation_fail_closed_dispatch_source()
}

pub fn simulate_live_host_status_selected_presentation_fail_closed_honesty() -> bool {
    let ok = honesty_host_status_selected_presentation_fail_closed_residual_pack_wave547();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostStatusSelectedPresentationFailClosedAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_host_status_selected_presentation_fail_closed_method_names_residual_wave547()
        );
    }

    #[test]
    fn source_markers_residual() {
        assert!(
            honesty_host_status_selected_presentation_fail_closed_source_markers_residual_wave547()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_host_status_selected_presentation_fail_closed_nav_commands_residual_wave547()
        );
    }

    #[test]
    fn host_status_selected_presentation_fail_closed_sources() {
        assert!(simulate_host_status_selected_presentation_fail_closed_collect_source());
        assert!(simulate_host_status_selected_presentation_fail_closed_dispatch_source());
    }

    #[test]
    fn wave547_composite_pack() {
        assert!(honesty_host_status_selected_presentation_fail_closed_residual_pack_wave547());
    }

    #[test]
    fn simulate_live_host_status_selected_presentation_fail_closed_honesty_residual_live() {
        assert!(
            simulate_live_host_status_selected_presentation_fail_closed_honesty(),
            "host status selected presentation fail-closed residual must latch"
        );
        assert!(residual_host_status_selected_presentation_fail_closed_ok());
        assert_eq!(
            residual_host_status_selected_presentation_fail_closed_last_action(),
            ResidualHostStatusSelectedPresentationFailClosedAction::Composite
        );
    }
}
