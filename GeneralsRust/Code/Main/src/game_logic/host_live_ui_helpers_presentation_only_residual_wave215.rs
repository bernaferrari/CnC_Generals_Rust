//! Wave 215 residual peels: InGame UI identity helpers are presentation-only
//! (`ui_object_can_produce` / under_construction / special_power_ready /
//! `ui_selected_ids` prefer freeze). No live GameLogic dual-read fallback when
//! a presentation frame is installed. Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 214 force-completed producer pick residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` ui_object_* / ui_selected_ids
//!
//! Fail-closed:
//! - Not full ControlBar WND command matrix
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// UI helpers presentation-only residual method names.
pub const LIVE_UI_HELPERS_PRESENTATION_ONLY_METHOD_NAMES_WAVE215: &[&str] = &[
    "ui_object_can_produce",
    "ui_object_under_construction",
    "ui_special_power_ready",
    "ui_selected_ids",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_UI_HELPERS_PRESENTATION_ONLY_NAV_STEPS_WAVE215: &[&str] = &[
    "REQUIRE_UI_HELPERS_PRESENTATION_ONLY",
    "REQUIRE_SELECTED_PREFERS_PRESENTATION",
    "LIVE_UI_HELPERS_PRESENTATION_ONLY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_UI_HELPERS_PRESENTATION_ONLY_CMD_NAMES_WAVE215: &[&str] = &[
    "click_live_ui_helpers_presentation_only_ok_prepare",
    "click_live_ui_helpers_presentation_only_ok_live",
    "click_live_ui_helpers_presentation_only_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_ui_helpers_presentation_only_method_names_residual_wave215() -> bool {
    LIVE_UI_HELPERS_PRESENTATION_ONLY_METHOD_NAMES_WAVE215.len() == 5
        && residual_name_index(
            LIVE_UI_HELPERS_PRESENTATION_ONLY_METHOD_NAMES_WAVE215,
            "ui_object_can_produce",
        ) == Some(0)
        && residual_name_index(
            LIVE_UI_HELPERS_PRESENTATION_ONLY_METHOD_NAMES_WAVE215,
            "ui_selected_ids",
        ) == Some(3)
        && residual_name_index(
            LIVE_UI_HELPERS_PRESENTATION_ONLY_METHOD_NAMES_WAVE215,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_ui_helpers_presentation_only_nav_commands_residual_wave215() -> bool {
    LIVE_UI_HELPERS_PRESENTATION_ONLY_NAV_STEPS_WAVE215.len() == 4
        && residual_name_index(
            LIVE_UI_HELPERS_PRESENTATION_ONLY_NAV_STEPS_WAVE215,
            "REQUIRE_UI_HELPERS_PRESENTATION_ONLY",
        ) == Some(0)
        && residual_name_index(
            LIVE_UI_HELPERS_PRESENTATION_ONLY_NAV_STEPS_WAVE215,
            "LIVE_UI_HELPERS_PRESENTATION_ONLY",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_UI_HELPERS_PRESENTATION_ONLY_CMD_NAMES_WAVE215.len() == 3
}

/// Wave 215 composite residual honesty pack.
pub fn honesty_live_ui_helpers_presentation_only_residual_pack_wave215() -> bool {
    honesty_live_ui_helpers_presentation_only_method_names_residual_wave215()
        && honesty_live_ui_helpers_presentation_only_nav_commands_residual_wave215()
}

fn eng_helper_body<'a>(eng: &'a str, sig: &str) -> Option<&'a str> {
    // Prefer production helpers (last occurrence of the signature).
    let i = eng.rfind(sig)?;
    let rest = &eng[i..];
    let end = rest.find("\n    fn ").unwrap_or(rest.len().min(900));
    Some(&rest[..end])
}

/// Source residual: can_produce / under_construction / special_power_ready are presentation-only.
pub fn honesty_ui_helpers_presentation_only_source() -> bool {
    let eng = include_str!("../cnc_game_engine.rs");
    for sig in [
        "fn ui_object_can_produce(&self, id: crate::game_logic::ObjectId) -> bool",
        "fn ui_object_under_construction(&self, id: crate::game_logic::ObjectId) -> bool",
        "fn ui_special_power_ready(&self, id: crate::game_logic::ObjectId) -> bool",
    ] {
        let Some(body) = eng_helper_body(eng, sig) else {
            return false;
        };
        if !body.contains("presentation_ro") {
            return false;
        }
        if body.contains("self.game_logic") {
            return false;
        }
        if !body.contains("Wave 215") {
            return false;
        }
    }
    true
}

/// Source residual: ui_selected_ids prefers presentation before host player list.
pub fn honesty_ui_selected_prefers_presentation_source() -> bool {
    let eng = include_str!("../cnc_game_engine.rs");
    let Some(body) = eng_helper_body(
        eng,
        "fn ui_selected_ids(&self, player_id: u32) -> Vec<crate::game_logic::ObjectId>",
    ) else {
        return false;
    };
    let pres = body.find("last_presentation_frame");
    let player = body.find("get_player(player_id)");
    matches!((pres, player), (Some(p), Some(g)) if p < g)
        && body.contains("Wave 215")
        && body.contains("selected_objects")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_ui_helpers_presentation_only_honesty() -> bool {
    honesty_live_ui_helpers_presentation_only_residual_pack_wave215()
        && honesty_ui_helpers_presentation_only_source()
        && honesty_ui_selected_prefers_presentation_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_ui_helpers_presentation_only_method_names_residual_wave215());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_ui_helpers_presentation_only_nav_commands_residual_wave215());
    }

    #[test]
    fn wave215_composite_pack() {
        assert!(honesty_live_ui_helpers_presentation_only_residual_pack_wave215());
    }

    #[test]
    fn ui_helpers_sources() {
        assert!(honesty_ui_helpers_presentation_only_source());
        assert!(honesty_ui_selected_prefers_presentation_source());
    }

    #[test]
    fn simulate_live_ui_helpers_presentation_only_honesty_residual_live() {
        assert!(
            simulate_live_ui_helpers_presentation_only_honesty(),
            "ui helpers presentation-only residual must latch"
        );
    }
}
