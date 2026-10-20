use super::*;
#[test]
fn constraint_v4_preserves_cache_while_v3_defaults_invalid_and_old_dtos_are_frozen() {
    let mut world = GameLogic::new();
    world
        .session_view
        .camera
        .constraints
        .recalculate(Vec3::ZERO, Vec3::new(1000., 0., 900.), 37.)
        .unwrap();
    world.session_view.camera.camera_follow_snap_immediate = true;
    let bytes = world.capture_camera_continuation().unwrap();
    assert_eq!(&bytes[4..8], &8u32.to_le_bytes());
    let current: EnvelopeV5 = codec()
        .deserialize::<EnvelopeV8>(&bytes[8..])
        .unwrap()
        .base
        .base
        .base;
    let saved = current.base;
    let mut v4 = MAGIC.to_vec();
    v4.extend_from_slice(&4u32.to_le_bytes());
    v4.extend(codec().serialize(&saved).unwrap());
    assert!(
        GameLogic::decode_camera_continuation(&v4)
            .unwrap()
            .owned_constraints()
            .is_valid()
    );
    let dto = codec().serialize(&saved.base.base.camera).unwrap();
    let legacy = saved.base.base.legacy_record.clone();
    let decoded = GameLogic::decode_camera_continuation(&bytes).unwrap();
    assert!(decoded.owned_constraints().is_valid());
    assert!(decoded.owned_follow_snap_immediate());
    let mut peer = GameLogic::new();
    let peer_before = peer.capture_camera_continuation().unwrap();
    let mut receiver = GameLogic::new();
    receiver.admit_camera_continuation(decoded).unwrap();
    assert_eq!(
        receiver.session_view.camera.constraints,
        world.session_view.camera.constraints
    );
    let mut v3 = MAGIC.to_vec();
    v3.extend_from_slice(&3u32.to_le_bytes());
    v3.extend(codec().serialize(&saved.base).unwrap());
    let old = GameLogic::decode_camera_continuation(&v3).unwrap();
    assert!(!old.owned_constraints().is_valid());
    assert!(old.owned_follow_snap_immediate());
    assert_eq!(
        codec()
            .serialize(old.owned_extras.camera.as_ref().unwrap())
            .unwrap(),
        dto
    );
    assert_eq!(codec().serialize(&old).unwrap(), legacy);
    let mut invalid = saved;
    invalid
        .constraints
        .recalculate(Vec3::splat(f32::INFINITY), Vec3::ONE, 0.)
        .unwrap_err();
    let mut nan = v4.clone();
    let len = nan.len();
    nan[len - 4..].copy_from_slice(&f32::NAN.to_le_bytes());
    assert!(GameLogic::decode_camera_continuation(&nan).is_err());
    // Invalid detached wire must reject before either receiver can change.
    let mut bad = bytes.clone();
    bad.truncate(bad.len() - 1);
    assert!(GameLogic::decode_camera_continuation(&bad).is_err());
    assert_eq!(peer_before, peer.capture_camera_continuation().unwrap());
}

#[test]
fn legacy_v4_finite_scale_overflow_is_rejected_by_receiving_admission_before_mutation() {
    let source = GameLogic::new();
    let bytes = source.capture_camera_continuation().unwrap();
    let mut current: EnvelopeV5 = codec()
        .deserialize::<EnvelopeV8>(&bytes[8..])
        .unwrap()
        .base
        .base
        .base;
    let mut legacy: LogicCameraContinuation = codec()
        .deserialize(&current.base.base.base.legacy_record)
        .unwrap();
    legacy.script_default_camera_max_height = f32::MAX;
    legacy.pending_camera_focus = Some(Vec3::new(400., 500., 600.));
    current.base.base.base.legacy_record = codec().serialize(&legacy).unwrap();
    let mut old = MAGIC.to_vec();
    old.extend_from_slice(&4u32.to_le_bytes());
    old.extend(codec().serialize(&current.base).unwrap());
    let detached = GameLogic::decode_camera_continuation(&old).unwrap();
    let mut receiver = GameLogic::new();
    let peer = GameLogic::new();
    receiver
        .admit_camera_application_rules(
            crate::presentation_frame::camera::settings::CameraContentRules {
                min_camera_height: 100.,
                max_camera_height: 300.,
                ..Default::default()
            },
        )
        .unwrap();
    let before = receiver.capture_camera_continuation().unwrap();
    let peer_before = peer.capture_camera_continuation().unwrap();
    assert!(
        receiver
            .validate_camera_continuation_admission(&detached)
            .is_err()
    );
    assert!(
        receiver
            .admit_camera_continuation(detached)
            .unwrap_err()
            .contains("nonfinite effective camera default maximum")
    );
    assert_eq!(before, receiver.capture_camera_continuation().unwrap());
    assert_eq!(peer_before, peer.capture_camera_continuation().unwrap());
}

#[test]
fn snapshot_builder_legacy_overflow_preflight_keeps_entire_persisted_owner_and_peer() {
    let source = GameLogic::new();
    let mut receiver = GameLogic::new();
    let peer = GameLogic::new();
    let rules = crate::presentation_frame::camera::settings::CameraContentRules {
        min_camera_height: 100.,
        max_camera_height: 300.,
        ..Default::default()
    };
    receiver.admit_camera_application_rules(rules).unwrap();
    receiver.set_current_frame(123); // Distinct real state exposes a late-only preflight.

    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let mut incoming = builder.create_world_snapshot(&source).unwrap();
    let raw = source.capture_camera_continuation().unwrap();
    let mut current: EnvelopeV5 = codec()
        .deserialize::<EnvelopeV8>(&raw[8..])
        .unwrap()
        .base
        .base
        .base;
    let mut legacy: LogicCameraContinuation = codec()
        .deserialize(&current.base.base.base.legacy_record)
        .unwrap();
    legacy.script_default_camera_max_height = f32::MAX;
    current.base.base.base.legacy_record = codec().serialize(&legacy).unwrap();
    let mut old = MAGIC.to_vec();
    old.extend_from_slice(&4u32.to_le_bytes());
    old.extend(codec().serialize(&current.base).unwrap());
    // Independent historical CAMC fixture framing, preserving all other real capsules.
    let mut payload = Vec::new();
    payload.extend_from_slice(&(old.len() as u32).to_le_bytes());
    payload.extend(old);
    payload.extend_from_slice(&0u32.to_le_bytes());
    let mut record = b"CAMC".to_vec();
    record.extend_from_slice(&1u32.to_le_bytes());
    record.extend_from_slice(&((payload.len() * 2) as u32).to_le_bytes());
    for value in payload {
        record.extend(format!("{value:02x}").as_bytes());
    }
    let at = incoming
        .lifecycle_tail
        .windows(4)
        .position(|word| word == b"CAMC")
        .unwrap();
    let len =
        u32::from_le_bytes(incoming.lifecycle_tail[at + 8..at + 12].try_into().unwrap()) as usize;
    incoming.lifecycle_tail.splice(at..at + 12 + len, record);
    let mut before = builder.create_world_snapshot(&receiver).unwrap();
    let mut peer_before = builder.create_world_snapshot(&peer).unwrap();
    assert!(
        builder
            .restore_from_snapshot(&incoming, &mut receiver)
            .unwrap_err()
            .to_string()
            .contains("nonfinite effective camera default maximum")
    );
    let mut after = builder.create_world_snapshot(&receiver).unwrap();
    after.timestamp = before.timestamp;
    let mut peer_after = builder.create_world_snapshot(&peer).unwrap();
    peer_after.timestamp = peer_before.timestamp;
    // Each capture allocates a new timer HashMap. Compare every key/value bit
    // before removing its arbitrary iteration order from the full byte check.
    for (before, after) in [
        (&mut before, &mut after),
        (&mut peer_before, &mut peer_after),
    ] {
        let timer_bits = |timers: &std::collections::HashMap<String, f32>| {
            timers
                .iter()
                .map(|(key, value)| (key.clone(), value.to_bits()))
                .collect::<std::collections::BTreeMap<_, _>>()
        };
        assert_eq!(
            timer_bits(&before.global_ai_state.global_timers),
            timer_bits(&after.global_ai_state.global_timers)
        );
        before.global_ai_state.global_timers.clear();
        after.global_ai_state.global_timers.clear();
    }
    assert_eq!(
        codec().serialize(&after).unwrap(),
        codec().serialize(&before).unwrap()
    );
    assert_eq!(
        codec().serialize(&peer_after).unwrap(),
        codec().serialize(&peer_before).unwrap()
    );
}
