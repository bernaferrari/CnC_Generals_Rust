//! Transport destruction records immediate scatter pose before its path order.
use super::tests::setup_zero_damage_transport;

#[test]
fn transport_death_scatter_records_own_pose_before_path_order() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "transport_death_scatter_records_own_pose_before_path_order",
        || {
            // This regression runs in its isolated child; the coupled branch
            // needs both the session opt-in and a live shadow frame.
            crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
            for movement in [false, true] {
                let hull = glam::Vec3::new(10_000.0, 40.0, 10_000.0);
                let (mut world, transport, rider) = setup_zero_damage_transport(true, hull);
                let (mut foreign, foreign_transport, foreign_rider) =
                    setup_zero_damage_transport(true, hull + glam::Vec3::X * 200.0);
                assert_eq!(transport, foreign_transport);
                assert_eq!(rider, foreign_rider);
                world.frame = 42;
                world
                    .host_object_mut(transport)
                    .unwrap()
                    .is_technical_transport = true;
                let passenger = world.host_object_mut(rider).unwrap();
                passenger.next_mood_check_time = 9_999;
                passenger.randomly_offset_mood_check = false;
                let session = world.movement_events.session().clone();
                let mut authority =
                    crate::game_logic::game_logic::gameworld_authority::GameWorldAuthority::DEFAULT_OFF;
                authority.movement = movement;
                crate::gameworld_shadow::with_gameworld_authority(authority, || {
                    let _coupled = movement.then(crate::gameworld_shadow::CoupledTickGuard::enter);
                    assert_eq!(
                        crate::gameworld_shadow::gameworld_movement_authority_live(),
                        movement
                    );
                    world.mark_object_for_destruction(transport, None);
                    world.process_destroy_list();
                });

                let passenger = world.host_object(rider).unwrap();
                assert!(passenger.is_alive());
                assert!(passenger.contained_by.is_none());
                assert_eq!(passenger.get_position(), hull);
                assert_eq!(passenger.ai_state, crate::game_logic::AIState::Moving);
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
                let records = world.movement_events.snapshot_pending().to_vec();
                assert_eq!(records.len(), if movement { 2 } else { 1 });
                for (index, record) in records.iter().enumerate() {
                    assert_eq!(record.session, session);
                    assert_eq!(record.logic_frame, 42);
                    assert_eq!(record.sequence, index as u64 + 1);
                    assert_eq!(record.unit, rider);
                }
                if movement {
                    assert_eq!(records[0].destination, Some(hull.to_array()));
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
            }
        },
    );
}
