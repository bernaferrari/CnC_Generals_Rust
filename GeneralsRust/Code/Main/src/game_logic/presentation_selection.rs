//! Presentation selection.

/// Lookup residual name index (exact match).

/// UI helpers presentation-only residual method names.

/// Ordered residual navigation steps.

/// Runtime-host command residual names.

/// Honesty: method names residual pack.

/// Honesty: nav steps + runtime-host cmd residual pack.

/// Wave 215 composite residual honesty pack.

/// Source residual: can_produce / under_construction / special_power_ready are presentation-only.

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
        let flagged: Vec<_> = frame
            .objects
            .iter()
            .filter(|o| o.selected && !o.destroyed && o.health_current > 0.0)
            .map(|o| o.id)
            .collect();
        if !flagged.is_empty() {
            return flagged;
        }
        // An installed freeze with nothing selected is a deselect. Do not
        // order the leftover engine or match list.
        return Vec::new();
    }
    if !engine_selected.is_empty() {
        return engine_selected.to_vec();
    }
    host_match_selected
        .map(|ids| ids.to_vec())
        .unwrap_or_default()
}

/// Source residual: ui_selected_ids prefers presentation freeze; no GameLogic dual-read.

/// Live residual: source honesty pack latches.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_ui_selected_ids_prefers_presentation_freeze_over_engine_selection() {
        use crate::game_logic::{GameLogic, KindOf, ObjectId, Team, ThingTemplate};

        use crate::presentation_frame::PresentationFrame;
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut t = ThingTemplate::new("UiSelRanger215");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("UiSelRanger215".into(), t);
        let id = logic
            .create_object("UiSelRanger215", Team::USA, Vec3::new(3.0, 0.0, 4.0))
            .expect("create");

        let mut frame = PresentationFrame::build_from_logic(&mut logic, 0);
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
