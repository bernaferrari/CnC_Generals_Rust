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
    let eng = crate::cnc_game_engine::ENGINE_SRC;
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

/// Wave 215: when a presentation freeze is installed, it owns selection residual
/// (even if empty). Engine `selected_objects` / host-match ids are boot fallbacks
/// only. Never dual-reads GameLogic `get_player` / `player_selected_objects`.
pub fn host_ui_selected_ids_from_residuals(
    presentation: Option<&crate::presentation_frame::PresentationFrame>,
    engine_selected: &[crate::game_logic::ObjectId],
    host_match_selected: Option<&[crate::game_logic::ObjectId]>,
) -> Vec<crate::game_logic::ObjectId> {
    if let Some(frame) = presentation {
        if !frame.selected.is_empty() {
            return frame.selected.clone();
        }
        return frame
            .objects
            .iter()
            .filter(|o| o.selected && !o.destroyed && o.health_current > 0.0)
            .map(|o| o.id)
            .collect();
    }
    if !engine_selected.is_empty() {
        return engine_selected.to_vec();
    }
    host_match_selected
        .map(|ids| ids.to_vec())
        .unwrap_or_default()
}

/// Source residual: ui_selected_ids prefers presentation freeze; no GameLogic dual-read.
pub fn honesty_ui_selected_prefers_presentation_source() -> bool {
    let helper = include_str!("host_live_ui_helpers_presentation_only_residual_wave215.rs");
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    let Some(i) = helper.find("pub fn host_ui_selected_ids_from_residuals(") else {
        return false;
    };
    let body = &helper[i..helper.len().min(i + 1200)];
    let pres = body.find("presentation");
    let engine_sel = body.find("engine_selected");
    matches!((pres, engine_sel), (Some(p), Some(e)) if p < e)
        && body.contains("frame.selected")
        && !body.contains("get_player(player_id)")
        && !body.contains("player_selected_objects")
        && eng.contains("host_ui_selected_ids_from_residuals")
        && eng.contains("Wave 215")
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

    #[test]
    fn host_ui_selected_ids_prefers_presentation_freeze_over_engine_selection() {
        use crate::game_logic::{GameLogic, KindOf, ObjectId, Team, ThingTemplate};
        use crate::gameworld_shadow::ensure_gate_damage_authority;
        use crate::presentation_frame::PresentationFrame;
        use glam::Vec3;

        ensure_gate_damage_authority();
        let mut logic = GameLogic::new();
        let mut t = ThingTemplate::new("UiSelRanger215");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("UiSelRanger215".into(), t);
        let id = logic
            .create_object("UiSelRanger215", Team::USA, Vec3::new(3.0, 0.0, 4.0))
            .expect("create");

        let mut frame = PresentationFrame::build_from_logic(&logic, 0);
        frame.selected = vec![id];
        if let Some(o) = frame.objects.iter_mut().find(|o| o.id == id) {
            o.selected = true;
        }
        let engine_only = [ObjectId(99_999)];
        let from_freeze = host_ui_selected_ids_from_residuals(Some(&frame), &engine_only, None);
        assert_eq!(
            from_freeze,
            vec![id],
            "installed presentation freeze owns selection residual"
        );

        frame.selected.clear();
        if let Some(o) = frame.objects.iter_mut().find(|o| o.id == id) {
            o.selected = false;
        }
        let empty_freeze = host_ui_selected_ids_from_residuals(Some(&frame), &engine_only, None);
        assert!(
            empty_freeze.is_empty(),
            "empty presentation freeze must fail-closed (no engine dual-read)"
        );

        let boot = host_ui_selected_ids_from_residuals(None, &engine_only, None);
        assert_eq!(boot, engine_only, "no freeze → engine selected residual");
    }
}
