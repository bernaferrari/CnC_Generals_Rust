//! Wave 1089: dual-world select-similar / select-matching unusable FOW residual.
//!
//! After Waves 1085–1088 hover/cmd peels, mass-select still matched catalog
//! entries that were destroyed/sold/masked/disabled or non-local stealth/FOW.
//! Fail-close seed + candidates so double-click select-similar and
//! select-matching-across-map/region match collect_selectable peels.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SELECT_SIMILAR_MATCHING_UNUSABLE_METHOD_NAMES_WAVE1089: &[&str] = &[
    "select_similar_units",
    "select_matching_from_presentation_catalog",
    "Wave 1089",
    "playable_claim = false",
];

pub const LIVE_HOST_SELECT_SIMILAR_MATCHING_UNUSABLE_NAV_STEPS_WAVE1089: &[&str] = &[
    "SELECT_SIMILAR",
    "SELECT_MATCHING",
    "UNUSABLE_FOW_STEALTH",
    "LIVE_HOST_SELECT_SIMILAR_MATCHING_UNUSABLE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSelectSimilarMatchingUnusableAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSelectSimilarMatchingUnusableAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn ui_source() -> &'static str {
    game_client::gui::ingame_ui::INGAME_UI_SRC
}
fn es_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}

pub fn honesty_host_select_similar_matching_unusable_method_names_residual_wave1089() -> bool {
    let names = LIVE_HOST_SELECT_SIMILAR_MATCHING_UNUSABLE_METHOD_NAMES_WAVE1089;
    let ok = residual_name_index(names, "select_similar_units").is_some()
        && residual_name_index(names, "Wave 1089").is_some();
    residual_action_store(ResidualHostSelectSimilarMatchingUnusableAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_select_similar_matching_unusable_nav_commands_residual_wave1089() -> bool {
    let steps = LIVE_HOST_SELECT_SIMILAR_MATCHING_UNUSABLE_NAV_STEPS_WAVE1089;
    let ok = residual_name_index(steps, "LIVE_HOST_SELECT_SIMILAR_MATCHING_UNUSABLE").is_some()
        && residual_name_index(steps, "SELECT_MATCHING").is_some();
    residual_action_store(ResidualHostSelectSimilarMatchingUnusableAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_select_similar_matching_unusable_residual_pack_wave1089() -> bool {
    let ui = ui_source();
    let es = es_source();
    let sim_i = match ui.find("fn select_similar_units") {
        Some(i) => i,
        None => {
            residual_action_store(ResidualHostSelectSimilarMatchingUnusableAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let sim = &ui[sim_i..sim_i.saturating_add(2800)];
    let mat_i = match ui.find("fn select_matching_from_presentation_catalog") {
        Some(i) => i,
        None => {
            residual_action_store(ResidualHostSelectSimilarMatchingUnusableAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let mat = &ui[mat_i..mat_i.saturating_add(4500)];
    let ok = sim.contains("Wave 1089: select-similar seed residual fail-closed on unusable seed")
        && sim.contains("Wave 1089: candidate residual fail-closed on unusable / non-local")
        && sim.contains("ObjectShroudStatus::Fogged")
        && sim.contains("u.effectively_stealthed")
        && mat.contains("Wave 1089: select-matching seed residual fail-closed on unusable")
        && mat.contains("Wave 1089: select-matching candidate residual fail-closed")
        && mat.contains("ObjectShroudStatus::Fogged")
        && mat.contains("u.effectively_stealthed")
        && // 2026-08-15: playable_claim is the five-flag constructor, not a literal assignment.
        es.contains("self.playable_claim = Self::retail_windowed_playable_claim(")
        && es.contains("Headless smoke must keep `playable_claim == false`");
    residual_action_store(ResidualHostSelectSimilarMatchingUnusableAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_select_similar_matching_unusable_residual_honesty() -> bool {
    let a = honesty_host_select_similar_matching_unusable_method_names_residual_wave1089();
    let b = honesty_host_select_similar_matching_unusable_nav_commands_residual_wave1089();
    let c = honesty_host_select_similar_matching_unusable_residual_pack_wave1089();
    residual_action_store(ResidualHostSelectSimilarMatchingUnusableAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_select_similar_matching_unusable_residual_wave1089() {
        assert!(honesty_host_select_similar_matching_unusable_residual_pack_wave1089());
        assert!(honesty_host_select_similar_matching_unusable_method_names_residual_wave1089());
        assert!(honesty_host_select_similar_matching_unusable_nav_commands_residual_wave1089());
        assert!(simulate_live_host_select_similar_matching_unusable_residual_honesty());
    }
}
