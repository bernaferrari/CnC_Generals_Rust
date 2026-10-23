//! Encoded waypoint ownership and receiver validation regressions.
use super::*;
fn world() -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.add_player(crate::game_logic::Player::new(
        1,
        crate::game_logic::Team::USA,
        "Wire",
        true,
    ));
    let template = crate::game_logic::ThingTemplate::new("WaypointWire");
    world.templates.insert(template.name.clone(), template);
    let id = world
        .create_object_for_player("WaypointWire", 1, Vec3::ZERO)
        .unwrap();
    let mut map = gamelogic::system::map_loader::MapData::new();
    map.width = 4;
    map.height = 4;
    map.heightmap = vec![0; 16];
    map.waypoints
        .push(gamelogic::system::map_loader::MapWaypoint {
            id: 0,
            name: String::new(),
            location: gamelogic::system::map_loader::Coord3D::new(0., 0., 0.),
            path_label1: String::new(),
            path_label2: String::new(),
            path_label3: String::new(),
            bi_directional: false,
        });
    world
        .world_services
        .terrain()
        .write()
        .unwrap()
        .load_map_data(map);
    (world, id)
}
#[test]
fn malformed_waypoint_identity_rejects_before_any_live_ai_order_mutation() {
    let (mut source, id) = world();
    source
        .host_object_mut(id)
        .unwrap()
        .stamp_pending_waypoint(0, [String::new(), String::new(), String::new()]);
    let mut bytes = Vec::new();
    append_to_lifecycle_tail(&mut bytes, &source);
    let suffix = find_tmai_suffix(&bytes).unwrap();
    let mut suffix = suffix;
    assert_eq!(take_u32(&mut suffix).unwrap(), 12);
    let length = take_u32(&mut suffix).unwrap() as usize;
    let mut envelope: AiTeamPersistPayloadV12 =
        bincode_legacy::deserialize(&suffix[..length]).unwrap();
    assert_eq!(
        envelope.waypoints[0].pending,
        Some(0),
        "ID zero is distinct from no pointer"
    );
    envelope.waypoints[0].receiver = 0;
    let encoded = bincode_legacy::serialize(&envelope).unwrap();
    let mut malformed = TMAI_MAGIC.to_vec();
    append_u32(&mut malformed, 12);
    append_u32(&mut malformed, encoded.len() as u32);
    malformed.extend(encoded);
    let (mut receiving, receiving_id) = world();
    assert_eq!(receiving_id, id);
    receiving
        .host_object_mut(id)
        .unwrap()
        .stamp_pending_waypoint(77, ["Keep".into(), "".into(), "".into()]);
    assert!(apply_from_lifecycle_tail(&malformed, &mut receiving).is_err());
    assert_eq!(
        receiving.host_object(id).unwrap().pending_waypoint_id,
        Some(77)
    );
    assert_eq!(
        receiving.host_object(id).unwrap().pending_waypoint_labels,
        ["Keep", "", ""]
    );
}
#[test]
fn historical_v11_known_labels_do_not_guess_a_receiving_waypoint_id() {
    let (mut source, id) = world();
    source
        .host_object_mut(id)
        .unwrap()
        .stamp_pending_waypoint_labels(["KnownLegacyPath"]);
    let payload = AiTeamPersistPayloadV11 {
        ai: capture(&source),
        movement_runtime: Vec::new(),
    };
    let encoded = bincode_legacy::serialize(&payload).unwrap();
    let mut bytes = TMAI_MAGIC.to_vec();
    append_u32(&mut bytes, 11);
    append_u32(&mut bytes, encoded.len() as u32);
    bytes.extend(encoded);
    let (mut restored, restored_id) = world();
    assert_eq!(id, restored_id);
    apply_from_lifecycle_tail(&bytes, &mut restored).unwrap();
    assert_eq!(
        restored.host_object(id).unwrap().pending_waypoint_labels,
        ["KnownLegacyPath"]
    );
    assert_eq!(restored.host_object(id).unwrap().pending_waypoint_id, None);
    assert_eq!(
        restored.host_object(id).unwrap().completed_waypoint_id,
        None
    );
    assert!(
        restored
            .host_object(id)
            .unwrap()
            .completed_waypoint_labels
            .is_empty()
    );
}

#[test]
fn receiving_graph_missing_or_changed_waypoint_rejects_before_mutating_live_ai() {
    let (mut source, id) = world();
    source
        .host_object_mut(id)
        .unwrap()
        .stamp_pending_waypoint(0, [String::new(), String::new(), String::new()]);
    let mut bytes = Vec::new();
    append_to_lifecycle_tail(&mut bytes, &source);
    let mut suffix = framing::find(&bytes).unwrap().unwrap();
    take_u32(&mut suffix).unwrap();
    let length = take_u32(&mut suffix).unwrap() as usize;
    let envelope: AiTeamPersistPayloadV12 = bincode_legacy::deserialize(&suffix[..length]).unwrap();
    for changed_slots in [false, true] {
        let mut invalid = AiTeamPersistPayloadV12 {
            previous: envelope.previous.clone(),
            waypoints: envelope.waypoints.clone(),
        };
        if changed_slots {
            invalid.previous.ai.orders[0].pending_waypoint_labels[0] = "Forged".into();
        } else {
            invalid.waypoints[0].pending = Some(99);
        }
        let encoded = bincode_legacy::serialize(&invalid).unwrap();
        let mut tail = TMAI_MAGIC.to_vec();
        append_u32(&mut tail, 12);
        append_u32(&mut tail, encoded.len() as u32);
        tail.extend(encoded);
        let (mut receiving, peer_id) = world();
        assert_eq!(id, peer_id);
        receiving
            .host_object_mut(id)
            .unwrap()
            .stamp_pending_waypoint_labels(["Keep"]);
        assert!(apply_from_lifecycle_tail(&tail, &mut receiving).is_err());
        assert_eq!(
            receiving.host_object(id).unwrap().pending_waypoint_labels,
            ["Keep"]
        );
        assert_eq!(
            receiving.host_object(id).unwrap().completed_waypoint_id,
            None
        );
    }
}

#[test]
fn spcd_v1_maximum_optional_pause_count_preserves_following_tmai_owner() {
    let (mut source, id) = world();
    source
        .host_object_mut(id)
        .unwrap()
        .stamp_pending_waypoint(0, [String::new(), String::new(), String::new()]);
    let mut bytes = b"SPCD".to_vec();
    for value in [1, 0, 0, 65536] {
        append_u32(&mut bytes, value);
    }
    for id in 0..65536 {
        append_u32(&mut bytes, id);
        append_u32(&mut bytes, 0);
    }
    append_to_lifecycle_tail(&mut bytes, &source);
    let (mut receiving, receiving_id) = world();
    apply_from_lifecycle_tail(&bytes, &mut receiving).unwrap();
    assert_eq!(receiving_id, id);
    assert_eq!(
        receiving.host_object(id).unwrap().pending_waypoint_id,
        Some(0)
    );
}

#[test]
fn full_snapshot_waypoint_preflight_uses_incoming_ids_and_preserves_live_receiver_on_error() {
    for changed_slots in [false, true] {
        assert_full_snapshot_waypoint_preflight_preserves_receiver(|payload| {
            if changed_slots {
                payload.previous.ai.orders[0].pending_waypoint_labels[0] = "Forged".into();
            } else {
                payload.waypoints[0].pending = Some(99);
            }
        });
    }
}

#[test]
fn full_snapshot_rejects_duplicate_zero_and_unknown_v12_orders_before_retirement() {
    for corruption in 0..3 {
        assert_full_snapshot_waypoint_preflight_preserves_receiver(|payload| {
            if corruption == 0 {
                let mut duplicate = payload.previous.ai.orders[0].clone();
                duplicate.pending_waypoint_labels[0] = "ForgedByLaterDuplicate".into();
                payload.previous.ai.orders.push(duplicate);
            } else {
                // Validate every saved order even without a waypoint row.
                payload.waypoints.clear();
                payload.previous.ai.orders[0].object_id = if corruption == 1 { 0 } else { 999 };
            }
        });
    }
}

fn assert_full_snapshot_waypoint_preflight_preserves_receiver(
    corrupt: impl FnOnce(&mut AiTeamPersistPayloadV12),
) {
    let (mut source, incoming_id) = world();
    source
        .host_object_mut(incoming_id)
        .unwrap()
        .stamp_pending_waypoint(0, [String::new(), String::new(), String::new()]);
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let snapshot_bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let mut malformed =
        crate::save_load::snapshot::decode_bincode_world_snapshot(&snapshot_bytes).unwrap();
    let suffix = framing::find(&malformed.lifecycle_tail).unwrap().unwrap();
    // The checked owner scanner gives the exact record, including opaque-data collisions.
    let start = suffix.as_ptr() as usize - malformed.lifecycle_tail.as_ptr() as usize - 4;
    let end = start + 4 + suffix.len();
    let mut rest = suffix;
    assert_eq!(take_u32(&mut rest).unwrap(), 12);
    let size = take_u32(&mut rest).unwrap() as usize;
    let mut payload: AiTeamPersistPayloadV12 = bincode_legacy::deserialize(&rest[..size]).unwrap();
    corrupt(&mut payload);
    let encoded = bincode_legacy::serialize(&payload).unwrap();
    let mut owner = TMAI_MAGIC.to_vec();
    append_u32(&mut owner, 12);
    append_u32(&mut owner, encoded.len() as u32);
    owner.extend(encoded);
    malformed.lifecycle_tail.splice(start..end, owner);
    let bytes = bincode_legacy::serialize(&malformed).unwrap();
    let malformed = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let (mut receiving, old_id) = world();
    receiving.destroy_object(old_id);
    receiving.process_destroy_list();
    receiving.set_next_object_id_for_restore(ObjectId(77));
    let outgoing = receiving
        .create_object_for_player("WaypointWire", 1, Vec3::ZERO)
        .unwrap();
    assert_eq!(outgoing, ObjectId(77));
    receiving.host_object_mut(outgoing).unwrap().health.current = 43.;
    receiving.get_player_mut(1).unwrap().resources.supplies = 1337;
    receiving.set_current_frame(412);
    receiving.set_receiving_time_of_day(game_engine::common::ini::TimeOfDay::Night);
    let token = receiving
        .host_object(outgoing)
        .unwrap()
        .visual_draw_identity_token
        .clone();
    let error = builder
        .restore_from_snapshot(&malformed, &mut receiving)
        .unwrap_err();
    assert!(error.to_string().contains("TMAI"), "{error}");
    assert_eq!(receiving.get_current_frame(), 412);
    assert_eq!(receiving.get_player(1).unwrap().resources.supplies, 1337);
    assert_eq!(
        receiving.lighting_rules().time_of_day,
        game_engine::common::ini::TimeOfDay::Night
    );
    assert_eq!(
        receiving
            .host_objects()
            .registered_keys()
            .copied()
            .collect::<Vec<_>>(),
        [outgoing]
    );
    let actor = receiving.host_object(outgoing).unwrap();
    assert_eq!(actor.health.current, 43.);
    assert!(std::sync::Arc::ptr_eq(
        &token,
        &actor.visual_draw_identity_token
    ));
    assert!(receiving.host_object(incoming_id).is_none());
    assert_eq!(
        receiving
            .world_services
            .terrain()
            .read()
            .unwrap()
            .get_waypoint_by_id(0)
            .unwrap()
            .get_path_label1(),
        ""
    );
    // Same incoming graph, different outgoing IDs: valid admission must use snapshot IDs.
    builder
        .restore_from_snapshot(&snapshot, &mut receiving)
        .unwrap();
    assert!(receiving.host_object(outgoing).is_none());
    assert_eq!(
        receiving
            .host_object(incoming_id)
            .unwrap()
            .pending_waypoint_id,
        Some(0)
    );
}
