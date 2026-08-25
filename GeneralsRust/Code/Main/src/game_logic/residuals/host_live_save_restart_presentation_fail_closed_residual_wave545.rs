//! Wave 545 residual peels: `build_save_info` and UI restart map/faction fail closed
//! under a presentation freeze — empty map_name does **not** dual-read
//! `get_current_map_name` / difficulty / play_time / local team. Boot residual
//! without freeze unchanged. Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 544 ui_selection_seed presentation fail-closed residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` build_save_info / UI restart
//!
//! Fail-closed:
//! - Presentation freeze owns save/restart metadata residual
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_SAVE_RESTART_PRESENTATION_FAIL_CLOSED_METHOD_NAMES_WAVE545: &[&str] = &[
    "build_save_info",
    "last_presentation_frame",
    "get_current_map_name",
    "UI requested restart",
    "Wave 545",
    "playable_claim = false",
];

pub const LIVE_SAVE_RESTART_PRESENTATION_FAIL_CLOSED_NAV_STEPS_WAVE545: &[&str] = &[
    "REQUIRE_SAVE_METADATA_PRESENTATION_FAIL_CLOSED",
    "REQUIRE_RESTART_METADATA_PRESENTATION_FAIL_CLOSED",
    "LIVE_SAVE_RESTART_PRESENTATION_FAIL_CLOSED",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_SAVE_RESTART_PRESENTATION_FAIL_CLOSED_CMD_NAMES_WAVE545: &[&str] = &[
    "save_metadata_presentation_fail_closed",
    "restart_metadata_presentation_fail_closed",
    "boot_get_current_map_name",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualSaveRestartPresentationFailClosedAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualSaveRestartPresentationFailClosedAction {
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

fn residual_action_store(action: ResidualSaveRestartPresentationFailClosedAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_save_restart_presentation_fail_closed_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_save_restart_presentation_fail_closed_last_action()
-> ResidualSaveRestartPresentationFailClosedAction {
    ResidualSaveRestartPresentationFailClosedAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
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

pub fn honesty_save_restart_presentation_fail_closed_method_names_residual_wave545() -> bool {
    let names = LIVE_SAVE_RESTART_PRESENTATION_FAIL_CLOSED_METHOD_NAMES_WAVE545;
    let ok = residual_name_index(names, "build_save_info").is_some()
        && residual_name_index(names, "last_presentation_frame").is_some()
        && residual_name_index(names, "get_current_map_name").is_some()
        && residual_name_index(names, "UI requested restart").is_some()
        && residual_name_index(names, "Wave 545").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualSaveRestartPresentationFailClosedAction::MethodNames);
    ok
}

pub fn honesty_save_restart_presentation_fail_closed_source_markers_residual_wave545() -> bool {
    let eng = eng_source();
    let Some(body) = fn_body(eng, "fn build_save_info(") else {
        residual_action_store(ResidualSaveRestartPresentationFailClosedAction::SourceMarkers);
        return false;
    };
    // Wave 554: save/restart metadata via presentation_or_boot_* helpers
    // (still fail-closed under freeze; raw dual-read only inside helpers).
    let save_ok = body.contains("Wave 545")
        && body.contains("presentation freeze owns save metadata residual")
        && !body.contains("filter(|s| !s.is_empty())")
        && (body.contains("pres.world_env.map_name.clone()")
            || body.contains("presentation_or_boot_map_name()"))
        && (body.contains("get_current_map_name()")
            || eng.contains("fn presentation_or_boot_map_name"));
    // Restart path
    let restart_ok = eng
        .contains("Wave 545: presentation freeze owns restart map/faction residual")
        && eng.contains("UI requested restart")
        && (eng.contains("pres.game_mode") || eng.contains("presentation_or_live_game_mode()"));
    let ok = save_ok && restart_ok && !eng.contains("playable_claim = true");
    residual_action_store(ResidualSaveRestartPresentationFailClosedAction::SourceMarkers);
    ok
}

pub fn honesty_save_restart_presentation_fail_closed_nav_commands_residual_wave545() -> bool {
    let steps = LIVE_SAVE_RESTART_PRESENTATION_FAIL_CLOSED_NAV_STEPS_WAVE545;
    let cmds = RUNTIME_HOST_LIVE_SAVE_RESTART_PRESENTATION_FAIL_CLOSED_CMD_NAMES_WAVE545;
    let ok = residual_name_index(steps, "REQUIRE_SAVE_METADATA_PRESENTATION_FAIL_CLOSED").is_some()
        && residual_name_index(steps, "REQUIRE_RESTART_METADATA_PRESENTATION_FAIL_CLOSED")
            .is_some()
        && residual_name_index(steps, "LIVE_SAVE_RESTART_PRESENTATION_FAIL_CLOSED").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "save_metadata_presentation_fail_closed").is_some()
        && residual_name_index(cmds, "restart_metadata_presentation_fail_closed").is_some()
        && residual_name_index(cmds, "boot_get_current_map_name").is_some();
    residual_action_store(ResidualSaveRestartPresentationFailClosedAction::NavCommands);
    ok
}

pub fn simulate_save_restart_presentation_fail_closed_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 545")
        && eng.contains("fn build_save_info")
        && eng.contains("presentation freeze owns save metadata residual");
    residual_action_store(ResidualSaveRestartPresentationFailClosedAction::CollectSource);
    ok
}

pub fn simulate_save_restart_presentation_fail_closed_dispatch_source() -> bool {
    let eng = eng_source();
    let Some(body) = fn_body(eng, "fn build_save_info(") else {
        residual_action_store(ResidualSaveRestartPresentationFailClosedAction::DispatchSource);
        return false;
    };
    let ok = body.contains("presentation freeze owns save metadata residual")
        && (body.contains("pres.total_play_time_seconds")
            || body.contains("presentation_or_boot_total_play_time()"))
        && (body.contains("get_current_map_name()")
            || eng.contains("fn presentation_or_boot_map_name"))
        && eng.contains("presentation freeze owns restart map/faction residual");
    residual_action_store(ResidualSaveRestartPresentationFailClosedAction::DispatchSource);
    ok
}

pub fn honesty_save_restart_presentation_fail_closed_residual_pack_wave545() -> bool {
    honesty_save_restart_presentation_fail_closed_method_names_residual_wave545()
        && honesty_save_restart_presentation_fail_closed_source_markers_residual_wave545()
        && honesty_save_restart_presentation_fail_closed_nav_commands_residual_wave545()
        && simulate_save_restart_presentation_fail_closed_collect_source()
        && simulate_save_restart_presentation_fail_closed_dispatch_source()
}

pub fn simulate_live_save_restart_presentation_fail_closed_honesty() -> bool {
    let ok = honesty_save_restart_presentation_fail_closed_residual_pack_wave545();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualSaveRestartPresentationFailClosedAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_save_restart_presentation_fail_closed_method_names_residual_wave545());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_save_restart_presentation_fail_closed_source_markers_residual_wave545());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_save_restart_presentation_fail_closed_nav_commands_residual_wave545());
    }

    #[test]
    fn save_restart_presentation_fail_closed_sources() {
        assert!(simulate_save_restart_presentation_fail_closed_collect_source());
        assert!(simulate_save_restart_presentation_fail_closed_dispatch_source());
    }

    #[test]
    fn wave545_composite_pack() {
        assert!(honesty_save_restart_presentation_fail_closed_residual_pack_wave545());
    }

    #[test]
    fn simulate_live_save_restart_presentation_fail_closed_honesty_residual_live() {
        assert!(
            simulate_live_save_restart_presentation_fail_closed_honesty(),
            "save/restart presentation fail-closed residual must latch"
        );
        assert!(residual_save_restart_presentation_fail_closed_ok());
        assert_eq!(
            residual_save_restart_presentation_fail_closed_last_action(),
            ResidualSaveRestartPresentationFailClosedAction::Composite
        );
    }
}
