//! Presentation command filters.

/// Lookup residual name index (exact match).

/// Cmd-filter + env presentation-only residual method names.

/// Ordered residual navigation steps.

/// Runtime-host command residual names.

/// Honesty: method names residual pack.

/// Honesty: nav steps + runtime-host cmd residual pack.

/// Wave 217 composite residual honesty pack.

/// Wave 217: sell identity from presentation freeze only (no live get_object).
pub fn presentation_selected_sellable_structure_ids(
    frame: Option<&crate::presentation_frame::PresentationFrame>,
    selected: &[crate::game_logic::ObjectId],
    team: crate::game_logic::Team,
) -> Vec<crate::game_logic::ObjectId> {
    use crate::game_logic::KindOf;
    use crate::presentation_frame::{PresentationBuildingType, PresentationObjectType};
    let Some(frame) = frame else {
        // Wave 217: presentation required for sell identity.
        return Vec::new();
    };
    selected
        .iter()
        .copied()
        .filter(|id| {
            frame.objects.iter().any(|o| {
                o.id == *id
                    && o.team == team
                    && !o.destroyed
                    && (crate::presentation_frame::PresentationFrame::object_has_kind(
                        o,
                        KindOf::Structure,
                    ) || o.object_type == PresentationObjectType::Building)
                    && !crate::presentation_frame::PresentationFrame::object_has_kind(
                        o,
                        KindOf::CommandCenter,
                    )
                    && o.building_type != Some(PresentationBuildingType::CommandCenter)
            })
        })
        .collect()
}

/// Source residual: sell/upgrade/formation/construct filters presentation-required.

/// Source residual: env hints presentation-only (Wave 455: no live GameLogic dual-read).

/// Live residual: source honesty pack latches.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presentation_selected_sellable_structure_ids_fail_closed_without_freeze() {
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};

        use crate::presentation_frame::PresentationFrame;
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut barracks = ThingTemplate::new("SellBarracks217");
        barracks.set_health(1000.0);
        barracks.add_kind_of(KindOf::Structure);
        barracks.add_kind_of(KindOf::Selectable);
        logic.templates.insert("SellBarracks217".into(), barracks);
        let mut cc = ThingTemplate::new("SellCC217");
        cc.set_health(2000.0);
        cc.add_kind_of(KindOf::Structure);
        cc.add_kind_of(KindOf::CommandCenter);
        cc.add_kind_of(KindOf::Selectable);
        logic.templates.insert("SellCC217".into(), cc);
        let mut ranger = ThingTemplate::new("SellRanger217");
        ranger.set_health(100.0);
        ranger.add_kind_of(KindOf::Infantry);
        ranger.add_kind_of(KindOf::Selectable);
        logic.templates.insert("SellRanger217".into(), ranger);

        let bid = logic
            .create_object("SellBarracks217", Team::USA, Vec3::new(10.0, 0.0, 0.0))
            .expect("barracks");
        let cid = logic
            .create_object("SellCC217", Team::USA, Vec3::new(20.0, 0.0, 0.0))
            .expect("cc");
        let rid = logic
            .create_object("SellRanger217", Team::USA, Vec3::new(30.0, 0.0, 0.0))
            .expect("ranger");

        assert!(
            presentation_selected_sellable_structure_ids(None, &[bid], Team::USA).is_empty(),
            "no presentation freeze → sell identity fail-closed"
        );

        let frame = PresentationFrame::build_from_logic(&mut logic, 0);
        let sold =
            presentation_selected_sellable_structure_ids(Some(&frame), &[bid, cid, rid], Team::USA);
        assert_eq!(sold, vec![bid], "only non-CC structure is sellable");
        assert!(
            presentation_selected_sellable_structure_ids(Some(&frame), &[rid], Team::USA)
                .is_empty()
        );
        assert!(
            presentation_selected_sellable_structure_ids(Some(&frame), &[cid], Team::USA)
                .is_empty(),
            "command center is not sellable"
        );
    }
}
