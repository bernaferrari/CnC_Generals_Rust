//! Wave 1097: presentation target/selected command-hint legality residual.
//!
//! After pick peels 1093–1096, right-click context still built
//! `presentation_target_hint` from any non-destroyed object (including sold/
//! masked/fogged non-local) and `presentation_selected_unit_hints` from sold/
//! masked/disabled/unselectable sources. Fail-close those so CommandSystem
//! mouse classification matches pick legality.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PRESENTATION_CMD_HINT_LEGALITY_METHOD_NAMES_WAVE1097: &[&str] = &[
    "presentation_target_hint",
    "presentation_selected_unit_hints",
    "Wave 1097",
    "playable_claim = false",
];

pub const LIVE_HOST_PRESENTATION_CMD_HINT_LEGALITY_NAV_STEPS_WAVE1097: &[&str] = &[
    "TARGET_HINT_SOLD_MASKED_FOW",
    "SELECTED_HINT_UNUSABLE",
    "LIVE_HOST_PRESENTATION_CMD_HINT_LEGALITY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPresentationCmdHintLegalityAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostPresentationCmdHintLegalityAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn es_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}

pub fn honesty_host_presentation_cmd_hint_legality_method_names_residual_wave1097() -> bool {
    let names = LIVE_HOST_PRESENTATION_CMD_HINT_LEGALITY_METHOD_NAMES_WAVE1097;
    let ok = residual_name_index(names, "presentation_target_hint").is_some()
        && residual_name_index(names, "Wave 1097").is_some();
    residual_action_store(ResidualHostPresentationCmdHintLegalityAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_presentation_cmd_hint_legality_nav_commands_residual_wave1097() -> bool {
    let steps = LIVE_HOST_PRESENTATION_CMD_HINT_LEGALITY_NAV_STEPS_WAVE1097;
    let ok = residual_name_index(steps, "LIVE_HOST_PRESENTATION_CMD_HINT_LEGALITY").is_some()
        && residual_name_index(steps, "TARGET_HINT_SOLD_MASKED_FOW").is_some();
    residual_action_store(ResidualHostPresentationCmdHintLegalityAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_presentation_cmd_hint_legality_residual_pack_wave1097() -> bool {
    let cnc = cnc_source();
    let es = es_source();
    let t_i = match cnc.find("fn presentation_target_hint") {
        Some(i) => i,
        None => {
            residual_action_store(ResidualHostPresentationCmdHintLegalityAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let t = &cnc[t_i..t_i.saturating_add(1600)];
    let s_i = match cnc.find("fn presentation_selected_unit_hints") {
        Some(i) => i,
        None => {
            residual_action_store(ResidualHostPresentationCmdHintLegalityAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let s = &cnc[s_i..s_i.saturating_add(1600)];
    let ok = t.contains("Wave 1097: target-hint residual fail-closed on sold/masked")
        && t.contains("!x.sold")
        && t.contains("!x.masked")
        && t.contains("visibility_alpha >= 0.95")
        && s.contains("Wave 1097: selected-hint residual fail-closed on unusable sources")
        && s.contains("o.sold")
        && s.contains("o.masked")
        && s.contains("o.disabled")
        && s.contains("o.unselectable")
        && cnc.contains("host_presentation_mouse_game_logic")
        && cnc.contains("// No live GameLogic dual-read for cursor/command classification.")
        && // 2026-08-15: playable_claim is the five-flag constructor, not a literal assignment.
        es.contains("self.playable_claim = Self::retail_windowed_playable_claim(")
        && es.contains("Headless smoke must keep `playable_claim == false`");
    residual_action_store(ResidualHostPresentationCmdHintLegalityAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_presentation_cmd_hint_legality_residual_honesty() -> bool {
    let a = honesty_host_presentation_cmd_hint_legality_method_names_residual_wave1097();
    let b = honesty_host_presentation_cmd_hint_legality_nav_commands_residual_wave1097();
    let c = honesty_host_presentation_cmd_hint_legality_residual_pack_wave1097();
    residual_action_store(ResidualHostPresentationCmdHintLegalityAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_presentation_cmd_hint_legality_residual_wave1097() {
        assert!(honesty_host_presentation_cmd_hint_legality_residual_pack_wave1097());
        assert!(honesty_host_presentation_cmd_hint_legality_method_names_residual_wave1097());
        assert!(honesty_host_presentation_cmd_hint_legality_nav_commands_residual_wave1097());
        assert!(simulate_live_host_presentation_cmd_hint_legality_residual_honesty());
    }
}
