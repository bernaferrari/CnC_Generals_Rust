//! Real GuardRetaliate RETURN callback receipts belong to the driving world.
use super::*;

fn guard_test_world() -> GameLogic {
    let mut world = GameLogic::new();
    world.set_ai_definition_base(game_engine::common::ini::AIData {
        guard_inner_modifier_human: 1.1,
        guard_outer_modifier_human: 1.333,
        guard_inner_modifier_ai: 1.8,
        guard_outer_modifier_ai: 2.2,
        alert_range_modifier: 1.1,
        aggressive_range_modifier: 1.5,
        ..Default::default()
    });
    world.add_player(Player::new(0, Team::USA, "GuardHuman", true));
    world
}

fn guard_test_object(mut template: ThingTemplate, id: ObjectId, team: Team) -> Object {
    template.set_authored_ai_update_interface(Some(true));
    let mut object = Object::new(template, id, team);
    object.owner_player_id = Some(0);
    object
}

#[test]
fn guard_return_movement_receipts_stay_with_same_id_owner() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "guard_return_movement_receipts_stay_with_same_id_owner",
        || {
            fn returning_guard(world: &mut GameLogic, anchor: glam::Vec3) -> ObjectId {
                let id = ObjectId(49_210);
                let mut template = ThingTemplate::new("OwnerReturningGuard");
                template.add_kind_of(KindOf::Infantry);
                let mut object = guard_test_object(template, id, Team::USA);
                object.set_position(anchor + glam::Vec3::new(200.0, 0.0, 0.0));
                object.guard_position = Some(anchor);
                object.set_ai_state(AIState::GuardingArea);
                object.begin_guard_retaliate(ObjectId(49_211), Some(anchor), None);
                world.objects.insert(id, object);
                id
            }

            let mut first = guard_test_world();
            let first_anchor = glam::Vec3::new(7.0, 0.0, 11.0);
            let id = returning_guard(&mut first, first_anchor);
            first.frame = 72;
            let first_session = first.movement_events.session().clone();
            first.tick_guard_retaliate_states();
            let first_receipts = first.movement_events.snapshot_pending().to_vec();
            assert_eq!(first_receipts.len(), 1);
            assert_eq!(first_receipts[0].session, first_session);
            assert_eq!(first_receipts[0].logic_frame, 72);
            assert_eq!(first_receipts[0].sequence, 1);
            assert_eq!(first_receipts[0].unit, id);
            assert_eq!(first_receipts[0].destination, Some(first_anchor.to_array()));
            let object = first.host_object(id).unwrap();
            assert_eq!(object.ai_state, AIState::GuardRetaliating);
            assert_eq!(object.movement.target_position, Some(first_anchor));
            assert!(object.status.moving);

            let mut foreign = guard_test_world();
            let foreign_anchor = glam::Vec3::new(301.0, 0.0, 401.0);
            assert_eq!(returning_guard(&mut foreign, foreign_anchor), id);
            foreign.frame = 91;
            foreign.tick_guard_retaliate_states();
            let foreign_receipts = foreign.movement_events.snapshot_pending();
            assert_eq!(foreign_receipts.len(), 1);
            assert_ne!(foreign_receipts[0].session, first_session);
            assert_eq!(foreign_receipts[0].logic_frame, 91);
            assert_eq!(foreign_receipts[0].unit, id);
            assert_eq!(
                foreign_receipts[0].destination,
                Some(foreign_anchor.to_array())
            );
            foreign.reset();
            assert!(foreign.movement_events.snapshot_pending().is_empty());
            drop(foreign);
            assert_eq!(first.movement_events.snapshot_pending(), first_receipts);

            first.frame = 73;
            first.tick_guard_retaliate_states();
            let receipts = first.movement_events.drain_for_host_boundary();
            assert_eq!(receipts.len(), 2);
            assert_eq!(receipts[0], first_receipts[0]);
            assert_eq!(receipts[1].session, first_session);
            assert_eq!(receipts[1].logic_frame, 73);
            assert_eq!(receipts[1].sequence, 2);
            assert_eq!(receipts[1].destination, Some(first_anchor.to_array()));
            assert!(first.movement_events.drain_for_host_boundary().is_empty());
            assert_eq!(first.movement_events.take_completed(), receipts);
            assert!(first.movement_events.take_completed().is_empty());
            assert_eq!(
                first.host_object(id).unwrap().ai_state,
                AIState::GuardRetaliating
            );
            first
                .host_object_mut(id)
                .unwrap()
                .set_position(first_anchor);
            first.tick_guard_retaliate_states();
            assert_eq!(
                first.host_object(id).unwrap().ai_state,
                AIState::GuardingArea
            );
            assert!(first.movement_events.snapshot_pending().is_empty());
        },
    );
}
