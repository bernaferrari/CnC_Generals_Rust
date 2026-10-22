//! Actual sale -> world snapshot -> retained SlowDeath continuation.
//! C++ SlowDeathBehavior.cpp:523-545 saves its sink/destruction frames.
use super::*;

fn continuation_roster() -> (GameLogic, ObjectId, [ObjectId; 3], ObjectId) {
    let (mut world, rack, slots, survivor) = ordered_roster();
    assert_eq!(
        world.reserve_airfield_parking_space(rack, survivor),
        Some(3)
    );
    let object = world.host_object_mut(survivor).unwrap();
    object.ground_height = 0.0;
    object.set_position(Vec3::new(100.0, 20.0, 100.0));
    assert!(object.is_above_terrain());
    (world, rack, slots, survivor)
}

#[test]
fn authored_airfield_sale_slow_death_world_snapshot_continues_to_original_retirement_frame() {
    let (mut source, rack, slots, survivor) = continuation_roster();
    let (mut restored, receiver_rack, receiver_slots, receiver_survivor) = continuation_roster();
    let (peer, peer_rack, peer_slots, peer_survivor) = continuation_roster();
    assert_eq!(
        (rack, slots, survivor),
        (receiver_rack, receiver_slots, receiver_survivor)
    );
    assert_eq!(
        (rack, slots, survivor),
        (peer_rack, peer_slots, peer_survivor)
    );
    let slow = slots[1];
    assert!(source.start_sell_object(rack));
    assert_instant_die(&source, slots[0]);
    assert_instant_die(&source, slots[2]);
    // Destruction requests are not a saved queue. Complete the actual removal
    // phase before capture, retaining the authored delayed body and survivor.
    source.process_destroy_list();
    assert!(queued(&source).is_empty());
    assert!(source.host_object(slots[0]).is_none());
    assert!(source.host_object(slots[2]).is_none());
    let completed = source
        .host_object(slow)
        .unwrap()
        .native_die_runtime
        .completed
        .clone();
    assert!(
        !completed.is_empty(),
        "actual SlowDeath slot already executed"
    );
    let builder = crate::save_load::SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&source).unwrap();
    let bytes = bincode_legacy::serialize(&saved).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    // The receiver was admitted through the same real player/default-team and
    // Object definitions before its births, rather than global CorePlayer setup.
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert_eq!(restored.frame, 40);
    assert!(restored.host_object(rack).unwrap().status.sold);
    assert_eq!(
        restored.snapshot_airfield_parking_spaces(),
        source.snapshot_airfield_parking_spaces()
    );
    assert!(queued(&restored).is_empty());
    for world in [&source, &restored] {
        let body = world.host_object(slow).unwrap();
        assert_eq!(body.health.current, 0.0);
        assert!(body.status.on_die_started);
        assert!(!body.status.destroyed);
        assert_eq!(body.native_die_runtime.completed, completed);
        let state = body.slow_death.as_ref().unwrap();
        assert_eq!(
            (
                state.begin_frame,
                state.sink_at_frame,
                state.destroy_at_frame
            ),
            (40, 46, 70)
        );
        assert!(!world.airfield_has_reserved_space(rack, slow));
        assert!(world.airfield_has_reserved_space(rack, survivor));
    }
    // This is the actual GameLogic AI completion phase, followed separately by
    // its real retirement phase. A direct Object timer call could not prove
    // that completion queues deletion instead of entering onDie a second time.
    for frame in 41..=70 {
        for world in [&mut source, &mut restored] {
            world.frame = frame;
            world.update_ai(&[slow], LOGIC_FRAME_TIMESTEP);
            let body = world
                .host_object(slow)
                .expect("AI queues before physical retirement");
            assert!(body.status.on_die_started);
            assert_eq!(
                body.native_die_runtime.completed, completed,
                "frame {frame}: no repeated Die"
            );
            let state = body.slow_death.as_ref().unwrap();
            assert_eq!(
                (
                    state.begin_frame,
                    state.sink_at_frame,
                    state.destroy_at_frame
                ),
                (40, 46, 70)
            );
            assert_eq!(body.status.destroyed, frame == 70);
            if frame < 70 {
                assert!(
                    queued(world).is_empty(),
                    "frame {frame}: not before deadline"
                );
            } else {
                assert_eq!(
                    queued(world),
                    vec![slow],
                    "one terminal removal at the saved deadline"
                );
            }
            world.process_destroy_list();
            assert_eq!(world.host_object(slow).is_none(), frame == 70);
            assert!(queued(world).is_empty());
            let live = world.host_object(survivor).unwrap();
            assert_eq!(live.health.current, 100.0);
            assert!(!live.status.on_die_started);
            assert_eq!(live.get_position(), Vec3::new(100.0, 20.0, 100.0));
            assert_eq!(live.producer_id, Some(rack));
            assert_eq!(live.airfield_parking_space_index, Some(3));
            assert!(world.airfield_has_reserved_space(rack, survivor));
        }
        assert_eq!(
            source.snapshot_airfield_parking_spaces(),
            restored.snapshot_airfield_parking_spaces()
        );
        if frame < 70 {
            assert_eq!(
                serde_json::to_value(&source.host_object(slow).unwrap().slow_death).unwrap(),
                serde_json::to_value(&restored.host_object(slow).unwrap().slow_death).unwrap(),
                "frame {frame}: retained future state agrees"
            );
        }
    }
    assert!(!peer.host_object(peer_rack).unwrap().status.sold);
    assert_eq!(peer.frame, 40);
    assert!(queued(&peer).is_empty());
    for id in peer_slots.into_iter().chain([peer_survivor]) {
        let object = peer.host_object(id).unwrap();
        assert_eq!(object.health.current, 100.0);
        assert!(!object.status.on_die_started);
        assert!(object.slow_death.is_none());
        assert!(peer.airfield_has_reserved_space(peer_rack, id));
    }
}
