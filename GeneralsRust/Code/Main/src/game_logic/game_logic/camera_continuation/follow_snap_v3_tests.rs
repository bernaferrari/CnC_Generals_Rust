use super::*;
#[test]
fn pending_snap_v3_roundtrips_without_extending_frozen_v2_or_logic40() {
    let mut world = GameLogic::new();
    world
        .session_view
        .set_camera_follow(Some(crate::game_logic::ObjectId(42)));
    let before =
        GameLogic::decode_camera_continuation(&world.capture_camera_continuation().unwrap())
            .unwrap();
    world.session_view.camera.camera_follow_snap_immediate = true;
    let bytes = world.capture_camera_continuation().unwrap();
    assert_eq!(&bytes[4..8], &6_u32.to_le_bytes());
    let pending = decode(&bytes).unwrap();
    assert!(pending.owned_follow_snap_immediate());
    assert_eq!(
        codec().serialize(&before).unwrap(),
        codec().serialize(&pending).unwrap()
    );
    assert_eq!(
        codec().serialize(&before.owned_extras.camera).unwrap(),
        codec().serialize(&pending.owned_extras.camera).unwrap()
    );
    let v5: EnvelopeV5 = codec().deserialize::<EnvelopeV6>(&bytes[8..]).unwrap().base;
    let v4 = v5.base;
    let v3 = v4.base;
    let mut old3 = MAGIC.to_vec();
    old3.extend_from_slice(&3_u32.to_le_bytes());
    old3.extend(codec().serialize(&v3).unwrap());
    assert!(decode(&old3).unwrap().owned_follow_snap_immediate());
    let mut old = MAGIC.to_vec();
    old.extend_from_slice(&2_u32.to_le_bytes());
    old.extend(codec().serialize(&v3.base).unwrap());
    assert!(!decode(&old).unwrap().owned_follow_snap_immediate());
    let mut receiving = GameLogic::new();
    let peer = GameLogic::new();
    let peer_before = peer.capture_camera_continuation().unwrap();
    receiving.admit_camera_continuation(pending).unwrap();
    assert!(receiving.session_view.camera.camera_follow_snap_immediate);
    let facts = crate::presentation_frame::follow::FollowTargetFacts {
        id: crate::game_logic::ObjectId(42),
        position: Vec3::new(80., 37., 90.),
        airborne_orientation: None,
    };
    receiving
        .session_view
        .admit_camera_follow(
            Some(crate::game_logic::ObjectId(42)),
            Some(facts),
            10.,
            false,
        )
        .unwrap();
    assert_eq!(receiving.session_view.target, Vec3::new(80., 0., 90.));
    assert!(!receiving.session_view.camera.camera_follow_snap_immediate);
    assert_eq!(peer_before, peer.capture_camera_continuation().unwrap());
    let unchanged = receiving.capture_camera_continuation().unwrap();
    // V3 is a frozen V2 prefix followed by this bool. Current V6 retains
    // that exact nested prefix but ends in clip/shake rule floats, not snap.
    let snap_offset = 8 + codec().serialize(&v3.base).unwrap().len();
    for mut invalid in [old3, bytes] {
        assert_eq!(invalid[snap_offset], 1, "verified pending-snap bool");
        invalid[snap_offset] = 2;
        assert!(GameLogic::decode_camera_continuation(&invalid).is_err());
    }
    assert_eq!(unchanged, receiving.capture_camera_continuation().unwrap());
}
