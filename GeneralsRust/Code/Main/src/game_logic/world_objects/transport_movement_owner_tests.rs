//! Transport destruction records immediate scatter pose before its path order.
use super::tests::setup_zero_damage_transport;

#[test]
fn transport_death_scatter_records_own_pose_before_path_order() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "transport_death_scatter_records_own_pose_before_path_order",
        || {
            // Native contain audio still uses its compatibility catalog.
            // The position and path are observations of the receiving owner.
            let hull = glam::Vec3::new(10_000.0, 40.0, 10_000.0);
            let (mut world, transport, rider) = setup_zero_damage_transport(true, hull);
            let (mut foreign, foreign_transport, foreign_rider) =
                setup_zero_damage_transport(true, hull + glam::Vec3::X * 200.0);
            assert_eq!(transport, foreign_transport);
            assert_eq!(rider, foreign_rider);
            world.frame = 42;
            // A running match queues pathfinding. Its request captures the
            // installed rider position synchronously at admission.
            world.map_loaded = true;
            world
                .host_object_mut(transport)
                .unwrap()
                .is_technical_transport = true;
            let passenger = world.host_object_mut(rider).unwrap();
            passenger.next_mood_check_time = 9_999;
            passenger.randomly_offset_mood_check = false;
            let session = world.movement_events.session().clone();

            {
                world.mark_object_for_destruction(transport, None);
                world.process_destroy_list();
            };

            let passenger = world.host_object(rider).unwrap();
            assert!(passenger.is_alive());
            assert!(passenger.contained_by.is_none());
            assert_eq!(passenger.get_position(), hull);
            // TransportContain::onRemoving first admits Aggressive attitude,
            // then scatterToNearbyPosition; the owned movement command
            // correctly carries that attitude as an attack-move order.
            assert_eq!(
                passenger.ai_state(),
                crate::game_logic::AIState::AttackMoving
            );
            assert_eq!(passenger.ignored_obstacle_id, Some(transport));
            assert_eq!(passenger.next_mood_check_time, 9_999);
            assert!(!passenger.randomly_offset_mood_check);
            let destination = passenger
                .movement
                .path
                .last()
                .copied()
                .or_else(|| {
                    world
                        .pathfinding_system
                        .pending_paths()
                        .find(|request| request.unit_id == rider)
                        .map(|request| request.destination)
                })
                .expect("scatter admitted the real path request");
            let path_origin = world
                .pathfinding_system
                .pending_paths()
                .find(|request| request.unit_id == rider)
                .map(|request| request.start)
                .expect("scatter queues a real match path request");
            assert_eq!(
                path_origin, hull,
                "path admission must observe the wreck pose, not the prior passenger pose"
            );
            let records = world.movement_events.snapshot_pending().to_vec();
            assert_eq!(
                records.len(),
                1,
                "only the actual path order emits a receipt"
            );
            for (index, record) in records.iter().enumerate() {
                assert_eq!(record.session, session);
                assert_eq!(record.logic_frame, 42);
                assert_eq!(record.sequence, index as u64 + 1);
                assert_eq!(record.unit, rider);
            }
            assert_eq!(
                records.last().unwrap().destination,
                Some(destination.to_array())
            );
            assert!(foreign.movement_events.snapshot_pending().is_empty());
            foreign.reset();
            drop(foreign);
            assert_eq!(world.movement_events.snapshot_pending(), records);
            assert_eq!(world.movement_events.drain_for_host_boundary(), records);
            assert_eq!(world.movement_events.take_completed(), records);
            assert!(world.movement_events.take_completed().is_empty());
        },
    );
}
