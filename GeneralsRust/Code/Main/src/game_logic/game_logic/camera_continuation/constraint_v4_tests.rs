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
    assert_eq!(&bytes[4..8], &4u32.to_le_bytes());
    let saved: EnvelopeV4 = codec().deserialize(&bytes[8..]).unwrap();
    let dto = codec().serialize(&saved.base.base.camera).unwrap();
    let legacy = saved.base.base.legacy_record.clone();
    let decoded = GameLogic::decode_camera_continuation(&bytes).unwrap();
    assert!(decoded.owned_constraints().is_valid());
    assert!(decoded.owned_follow_snap_immediate());
    let mut peer = GameLogic::new();
    let peer_before = peer.capture_camera_continuation().unwrap();
    let mut receiver = GameLogic::new();
    receiver.admit_camera_continuation(decoded);
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
    let mut nan = bytes.clone();
    let len = nan.len();
    nan[len - 4..].copy_from_slice(&f32::NAN.to_le_bytes());
    assert!(GameLogic::decode_camera_continuation(&nan).is_err());
    // Invalid detached wire must reject before either receiver can change.
    let mut bad = bytes.clone();
    bad.truncate(bad.len() - 1);
    assert!(GameLogic::decode_camera_continuation(&bad).is_err());
    assert_eq!(peer_before, peer.capture_camera_continuation().unwrap());
}
