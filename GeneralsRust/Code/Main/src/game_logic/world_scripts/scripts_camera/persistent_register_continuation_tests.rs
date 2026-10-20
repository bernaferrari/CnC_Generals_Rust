//! Complete register persistence at inactive and actual reset checkpoints.
use super::*;

#[test]
fn completed_scalar_end_survives_v8_save_before_missing_replacement_tracking() {
    let (mut source, _, source_b) = fixture();
    let (mut restored, _, restored_b) = fixture();
    let (peer, _, _) = fixture();
    let peer_before = peer.capture_camera_continuation().unwrap();
    track(&mut source, &peer.mission_scripts, "FocusScout", 0.132, 0.);
    offer(&mut source);
    actions(
        &mut source,
        &peer.mission_scripts,
        vec![reals(
            ScriptActionType::RotateCamera,
            &[0.25, 0.033, 0., 0.],
        )],
    );
    offer(&mut source);
    assert!(!source.session_view.camera.rotation_registers.active());
    let words = source
        .session_view
        .camera
        .rotation_registers
        .initialized_source_words()
        .unwrap();
    assert_eq!([words[2], words[3]], [50f32.to_bits(), 15f32.to_bits()]);
    assert_eq!(
        words[1],
        source.session_view.camera.camera_yaw_radians.to_bits()
    );
    let saved = source.capture_camera_continuation().unwrap();
    restored
        .admit_camera_continuation(GameLogic::decode_camera_continuation(&saved).unwrap())
        .unwrap();
    restored
        .install_owned_script_engine(engine(restored.mission_scripts.clone(), ScriptList::new()));
    assert_eq!(restored.capture_camera_continuation().unwrap(), saved);
    for owner in [&mut source, &mut restored] {
        track(
            owner,
            &peer.mission_scripts,
            "ReplacementScout",
            0.132,
            0.066,
        );
        let replacement = owner
            .session_view
            .camera
            .rotation_registers
            .initialized_source_words()
            .unwrap();
        assert_eq!(
            &replacement[1..],
            &words[1..],
            "inactive scalar end and trailing source cache are persistent words"
        );
    }
    remove(&mut source, source_b);
    remove(&mut restored, restored_b);
    for _ in 0..6 {
        offer(&mut source);
        offer(&mut restored);
        assert_eq!(
            source.capture_camera_continuation().unwrap(),
            restored.capture_camera_continuation().unwrap()
        );
    }
    assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
}

#[test]
fn actual_same_instance_reset_retains_initialized_payload_and_v8_scalar_future() {
    let (mut source, _, _) = fixture();
    let (peer, _, _) = fixture();
    track(&mut source, &peer.mission_scripts, "FocusScout", 0.132, 0.);
    offer(&mut source);
    let words = source
        .session_view
        .camera
        .rotation_registers
        .initialized_source_words()
        .unwrap();
    source.reset();
    assert!(
        source.objects.is_empty(),
        "exercise actual world reset, not only camera cancellation"
    );
    assert!(!source.session_view.camera.rotation_registers.active());
    assert_eq!(
        source
            .session_view
            .camera
            .rotation_registers
            .initialized_source_words(),
        Some(words)
    );
    let saved = source.capture_camera_continuation().unwrap();
    let mut restored = GameLogic::new();
    restored
        .admit_camera_continuation(GameLogic::decode_camera_continuation(&saved).unwrap())
        .unwrap();
    assert_eq!(restored.capture_camera_continuation().unwrap(), saved);
    for owner in [&mut source, &mut restored] {
        owner
            .session_view
            .install_rotate(0.25, 0.132, 0., 0., owner.visual_speed_multiplier);
    }
    for _ in 0..4 {
        offer(&mut source);
        offer(&mut restored);
        assert_eq!(
            source.capture_camera_continuation().unwrap(),
            restored.capture_camera_continuation().unwrap()
        );
        let retained = source
            .session_view
            .camera
            .rotation_registers
            .initialized_source_words()
            .unwrap();
        assert_eq!(&retained[2..], &words[2..]);
    }
}
