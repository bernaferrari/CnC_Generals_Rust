//! Whole-suffix admission after the driving owner's lifecycle/spawn phase.

use super::GameWorldShadow;
use crate::game_logic::GameLogic;
use crate::game_logic::movement_events::MovementBatch;

/// Queue diagnostic movement without advancing its execution phase.
///
/// Already-applied observations stay in the receipt prefix. Only physically
/// absent owner objects are discarded; retained but unmapped subjects keep the
/// entire pending suffix on their owner until a later mapping boundary.
pub(super) fn queue_owned_movement_batch(
    shadow: &mut GameWorldShadow,
    logic: &mut GameLogic,
    mut move_batch: MovementBatch,
) -> usize {
    move_batch.retain_pending_subjects(|id| logic.host_object(id).is_some());
    let mapped = move_batch.pending_shadow_events().iter().all(|event| {
        shadow
            .entity_for_host(event.unit)
            .and_then(|eid| shadow.world().entity(eid))
            .is_some()
    });
    if !mapped {
        logic.movement_events.set_early_batch(move_batch);
        return 0;
    }
    let queued = move_batch.pending_shadow_events().len();
    for event in move_batch.pending_shadow_events() {
        // No callbacks or mapping mutations occur between preflight and queue.
        assert!(shadow.queue_set_move_target_for_host(event.unit, event.destination));
    }
    move_batch.mark_shadow_applied();
    queued
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};
    use crate::gameworld_shadow::{ShadowCoupleGuard, eager_apply_host_move_attack_after_logic};
    use crate::presentation_frame::{PresentationEvent, PresentationFrame};
    use glam::Vec3;

    fn delayed_mapping_keeps_applied_prefix_and_late_suffix_case() {
        crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
        let mut logic = GameLogic::new();
        logic.add_player(Player::new(0, Team::USA, "MovementOwner", true));
        let mut template = ThingTemplate::new("MovementAdmissionUnit");
        template.add_kind_of(KindOf::Infantry).set_health(100.0);
        logic.templates.insert(template.name.clone(), template);
        let a = logic
            .create_object("MovementAdmissionUnit", Team::USA, Vec3::ZERO)
            .unwrap();
        let mut shadow = GameWorldShadow::new(64);
        shadow.sync_from_host(&logic);
        let a_entity = shadow.entity_for_host(a).unwrap();
        logic.movement_events.clear();
        let (object, mut writer) = logic.host_object_and_movement_events_mut(a).unwrap();
        object.set_destination(Vec3::new(20.0, 0.0, 0.0), &mut writer);
        let _couple = ShadowCoupleGuard::enter();
        assert_eq!(
            eager_apply_host_move_attack_after_logic(&mut shadow, &mut logic).1,
            1
        );
        // A current path-follow target is newer than the accepted eager goal.
        // Retrying the applied prefix must not overwrite that observation.
        logic.host_object_mut(a).unwrap().movement.target_position =
            Some(Vec3::new(99.0, 0.0, 0.0));
        shadow
            .world_mut()
            .world_mut()
            .entity_mut(a_entity)
            .unwrap()
            .move_target = Some([99.0, 0.0, 0.0]);
        let b = logic
            .create_object(
                "MovementAdmissionUnit",
                Team::USA,
                Vec3::new(10.0, 0.0, 0.0),
            )
            .unwrap();
        let (object, mut writer) = logic.host_object_and_movement_events_mut(b).unwrap();
        object.set_destination(Vec3::new(30.0, 0.0, 0.0), &mut writer);
        assert!(shadow.entity_for_host(b).is_none());
        let first = logic.movement_events.drain_for_shadow_boundary();
        assert_eq!(
            queue_owned_movement_batch(&mut shadow, &mut logic, first),
            0
        );
        assert!(logic.movement_events.has_pending(b));
        assert!(!logic.movement_events.has_pending(a));
        let second = logic.movement_events.drain_for_shadow_boundary();
        assert_eq!(
            queue_owned_movement_batch(&mut shadow, &mut logic, second),
            0
        );
        assert_eq!(logic.movement_events.snapshot_completed().len(), 2);
        let (object, mut writer) = logic.host_object_and_movement_events_mut(a).unwrap();
        object.set_destination(Vec3::new(40.0, 0.0, 0.0), &mut writer);
        let late = logic.movement_events.drain_for_shadow_boundary();
        assert_eq!(queue_owned_movement_batch(&mut shadow, &mut logic, late), 0);
        let _ = shadow.apply_pending();
        assert_eq!(
            shadow.world().entity(a_entity).unwrap().move_target,
            Some([99.0, 0.0, 0.0])
        );
        assert!(logic.movement_events.has_pending(a));
        assert!(logic.movement_events.has_pending(b));

        // The real mapping operation admits B; the exact late-session helper
        // then queues the whole suffix. This is a component test of admission,
        // not a claim that an ordinary full session leaves its map incomplete.
        shadow.sync_from_host(&logic);
        let b_entity = shadow.entity_for_host(b).unwrap();
        let mapped = logic.movement_events.drain_for_shadow_boundary();
        assert_eq!(
            queue_owned_movement_batch(&mut shadow, &mut logic, mapped),
            2
        );
        let _ = shadow.apply_pending();
        assert_eq!(
            shadow.world().entity(a_entity).unwrap().move_target,
            Some([40.0, 0.0, 0.0])
        );
        assert_eq!(
            shadow.world().entity(b_entity).unwrap().move_target,
            Some([30.0, 0.0, 0.0])
        );
        assert!(!logic.movement_events.has_pending(a));
        assert!(!logic.movement_events.has_pending(b));
        let receipts = logic.movement_events.snapshot_completed();
        assert_eq!(receipts.len(), 3);
        assert!(
            receipts
                .windows(2)
                .all(|pair| pair[0].sequence < pair[1].sequence)
        );
        assert_eq!(
            receipts
                .iter()
                .map(|event| (event.unit, event.destination))
                .collect::<Vec<_>>(),
            [
                (a, Some([20.0, 0.0, 0.0])),
                (b, Some([30.0, 0.0, 0.0])),
                (a, Some([40.0, 0.0, 0.0]))
            ]
        );
        let repeated = logic.movement_events.drain_for_shadow_boundary();
        assert_eq!(
            queue_owned_movement_batch(&mut shadow, &mut logic, repeated),
            0
        );
        assert_eq!(logic.movement_events.snapshot_completed().len(), 3);
        let frame = PresentationFrame::build_from_logic(&mut logic, 0);
        assert_eq!(
            frame
                .events
                .iter()
                .filter(|event| matches!(event, PresentationEvent::MoveOrdered { .. }))
                .count(),
            3
        );
        assert!(logic.movement_events.take_completed().is_empty());
    }

    #[test]
    fn delayed_mapping_keeps_applied_prefix_and_late_suffix() {
        crate::game_logic::game_logic::pose_owner_tests::isolated_at(
            module_path!(),
            "delayed_mapping_keeps_applied_prefix_and_late_suffix",
            delayed_mapping_keeps_applied_prefix_and_late_suffix_case,
        );
    }
}
