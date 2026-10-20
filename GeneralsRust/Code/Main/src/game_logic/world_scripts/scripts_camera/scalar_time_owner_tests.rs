//! Actual authored script intake and receiving scalar time continuation.
use super::*;

#[test]
fn authored_scalar_rotation_final_speed_belongs_to_receiving_view() {
    let (mut first, first_id, _) = world(110.0);
    let (mut peer, peer_id, _) = world(210.0);
    assert_eq!(first_id, peer_id);
    let foreign_hooks = peer.mission_scripts.clone();
    let mut list = ScriptList::new();
    list.append_script(script(
        "ScalarSpeed",
        vec![
            reals(ScriptActionType::RotateCamera, &[0.25, 0.132, 0.0, 0.0]),
            ints(ScriptActionType::CameraModSetFinalSpeedMultiplier, &[5]),
        ],
        true,
    ));
    let interpreter = run(&mut first, engine(foreign_hooks, list));
    assert_eq!(
        first.visual_speed_multiplier, 1.0,
        "scalar rotation modifier changes its endpoint, not current view time"
    );
    assert_eq!(peer.visual_speed_multiplier, 1.0);
    for expected in [2.0, 3.0, 4.0, 5.0] {
        first
            .headless_camera_view_offer_with_script_engine(33, Some(&interpreter))
            .unwrap();
        assert_eq!(
            first.visual_speed_multiplier, expected,
            "rotation writes rounded eased time at the admitted scalar phase"
        );
        assert_eq!(
            peer.visual_speed_multiplier, 1.0,
            "same-ID peer is not the camera multiplier owner"
        );
    }
    assert!(!first.session_view.camera.rotation_registers.active());
}

#[test]
fn authored_zoom_only_final_speed_is_immediate_and_does_not_ramp() {
    let (mut world, _, _) = world(110.0);
    let hooks = world.mission_scripts.clone();
    let mut list = ScriptList::new();
    list.append_script(script(
        "ZoomSpeed",
        vec![
            reals(ScriptActionType::ZoomCamera, &[0.8, 0.132, 0.0, 0.0]),
            ints(ScriptActionType::CameraModSetFinalSpeedMultiplier, &[5]),
        ],
        true,
    ));
    let interpreter = run(&mut world, engine(hooks, list));
    assert_eq!(
        world.visual_speed_multiplier, 5.0,
        "C++ final idle else belongs to rotation/path, not zoom/pitch flags"
    );
    for _ in 0..4 {
        world
            .headless_camera_view_offer_with_script_engine(33, Some(&interpreter))
            .unwrap();
        assert_eq!(
            world.visual_speed_multiplier, 5.0,
            "zoomCameraOneFrame never consumes zcInfo time endpoints"
        );
    }
}

#[test]
fn authored_scalar_speed_save_continuation_and_peer_interleaving_are_independent() {
    let (mut source, source_id, _) = world(110.0);
    let (mut peer, peer_id, _) = world(210.0);
    assert_eq!(source_id, peer_id);
    let mut list = ScriptList::new();
    list.append_script(script(
        "SpeedSave",
        vec![
            reals(ScriptActionType::RotateCamera, &[0.25, 0.132, 0.0, 0.0]),
            ints(ScriptActionType::CameraModSetFinalSpeedMultiplier, &[5]),
        ],
        true,
    ));
    let interpreter = run(&mut source, engine(peer.mission_scripts.clone(), list));
    for _ in 0..2 {
        source
            .headless_camera_view_offer_with_script_engine(33, Some(&interpreter))
            .unwrap();
    }
    assert_eq!(source.visual_speed_multiplier, 3.0);
    let saved = source.capture_camera_continuation().unwrap();
    assert_eq!(&saved[4..8], &8u32.to_le_bytes());
    let (mut restored, restored_id, _) = world(110.0);
    assert_eq!(source_id, restored_id);
    restored
        .admit_camera_continuation(GameLogic::decode_camera_continuation(&saved).unwrap())
        .unwrap();
    // This is the camera companion continuation, not a full ScriptEngine save.
    // Each receiving world still supplies its own explicit interpreter loan.
    let restored_interpreter = engine(restored.mission_scripts.clone(), ScriptList::new());
    let mut peer_list = ScriptList::new();
    peer_list.append_script(script(
        "PeerOnly",
        vec![
            ints(ScriptActionType::CameraModSetFinalSpeedMultiplier, &[9]),
            reals(ScriptActionType::RotateCamera, &[1.0, 0.132, 0.0, 0.0]),
        ],
        true,
    ));
    let peer_interpreter = run(&mut peer, engine(source.mission_scripts.clone(), peer_list));
    for expected in [4.0, 5.0] {
        peer.headless_camera_view_offer_with_script_engine(33, Some(&peer_interpreter))
            .unwrap();
        source
            .headless_camera_view_offer_with_script_engine(33, Some(&interpreter))
            .unwrap();
        restored
            .headless_camera_view_offer_with_script_engine(33, Some(&restored_interpreter))
            .unwrap();
        assert_eq!(source.visual_speed_multiplier, expected);
        assert_eq!(restored.visual_speed_multiplier, expected);
        assert_eq!(
            source.session_view.camera.camera_yaw_radians.to_bits(),
            restored.session_view.camera.camera_yaw_radians.to_bits()
        );
    }
    drop(peer);
    assert_eq!(
        source.visual_speed_multiplier,
        restored.visual_speed_multiplier
    );
    assert_eq!(
        source
            .session_view
            .camera
            .rotation_registers
            .scalar_target(),
        restored
            .session_view
            .camera
            .rotation_registers
            .scalar_target()
    );
}

#[test]
fn authored_waypoint_look_and_freeze_angle_keep_the_same_rotation_time_owner() {
    let (mut world, _, _) = world(110.0);
    *world.world_services.terrain().write().unwrap() = camera_terrain();
    let hooks = world.mission_scripts.clone();
    let mut list = ScriptList::new();
    list.append_script(script(
        "LookSpeed",
        vec![
            action(
                ScriptActionType::CameraLookTowardWaypoint,
                vec![
                    Parameter::with_string(ParameterType::Waypoint, "LookPoint".into()),
                    Parameter::with_real(ParameterType::Real, 0.132),
                    Parameter::with_real(ParameterType::Real, 0.0),
                    Parameter::with_real(ParameterType::Real, 0.0),
                    Parameter::with_int(ParameterType::Boolean, 0),
                ],
            ),
            Box::new(ScriptAction::new(ScriptActionType::CameraModFreezeAngle)),
            ints(ScriptActionType::CameraModSetFinalSpeedMultiplier, &[5]),
        ],
        true,
    ));
    let interpreter = run(&mut world, engine(hooks, list));
    assert_eq!(world.visual_speed_multiplier, 1.0);
    assert!(world.session_view.camera.rotation_registers.active());
    assert!(!world.session_view.camera.rotation_registers.is_tracking());
    let angle = world.session_view.camera.camera_yaw_radians.to_bits();
    for expected in [2.0, 3.0, 4.0, 5.0] {
        world
            .headless_camera_view_offer_with_script_engine(33, Some(&interpreter))
            .unwrap();
        assert_eq!(world.visual_speed_multiplier, expected);
        assert_eq!(
            world.session_view.camera.camera_yaw_radians.to_bits(),
            angle
        );
    }
}

#[test]
fn authored_scalar_rotation_replaces_tracking_identity_before_continuation_capture() {
    let (mut world, _, _) = world(110.0);
    let mut tracking = ScriptList::new();
    tracking.append_script(script(
        "TrackingFirst",
        vec![action(
            ScriptActionType::CameraLookTowardObject,
            vec![
                Parameter::with_string(ParameterType::Unit, "FocusScout".into()),
                Parameter::with_real(ParameterType::Real, 1.0),
                Parameter::with_real(ParameterType::Real, 2.0),
                Parameter::with_real(ParameterType::Real, 0.0),
                Parameter::with_real(ParameterType::Real, 0.0),
            ],
        )],
        true,
    ));
    let hooks = world.mission_scripts.clone();
    run(&mut world, engine(hooks, tracking));
    assert!(world.session_view.camera.rotation_registers.is_tracking());
    let mut rotate = ScriptList::new();
    rotate.append_script(script(
        "ScalarSecond",
        vec![
            reals(ScriptActionType::RotateCamera, &[0.25, 0.132, 0.0, 0.0]),
            ints(ScriptActionType::CameraModSetFinalSpeedMultiplier, &[5]),
        ],
        true,
    ));
    let hooks = world.mission_scripts.clone();
    let interpreter = run(&mut world, engine(hooks, rotate));
    assert!(!world.session_view.camera.rotation_registers.is_tracking());
    assert!(
        world
            .session_view
            .camera
            .rotation_registers
            .legacy_id()
            .is_none()
    );
    let saved = world.capture_camera_continuation().unwrap();
    GameLogic::decode_camera_continuation(&saved).unwrap();
    world
        .headless_camera_view_offer_with_script_engine(33, Some(&interpreter))
        .unwrap();
    assert_eq!(world.visual_speed_multiplier, 2.0);
}

#[test]
fn authored_tracking_freeze_then_near_zero_look_retains_common_curve_and_union_projection() {
    let (mut owner, id, _) = world(110.);
    let (peer, peer_id, _) = world(210.);
    assert_eq!(id, peer_id);
    *owner.world_services.terrain().write().unwrap() = camera_terrain();
    owner.session_view.target = Vec3::new(410., 0., 430.);
    let mut tracking = ScriptList::new();
    tracking.append_script(script(
        "BeforeNearZero",
        vec![
            action(
                ScriptActionType::CameraLookTowardObject,
                vec![
                    Parameter::with_string(ParameterType::Unit, "FocusScout".into()),
                    Parameter::with_real(ParameterType::Real, 0.132),
                    Parameter::with_real(ParameterType::Real, 0.132),
                    Parameter::with_real(ParameterType::Real, 0.033),
                    Parameter::with_real(ParameterType::Real, 0.033),
                ],
            ),
            ints(ScriptActionType::CameraModSetFinalSpeedMultiplier, &[31]),
        ],
        true,
    ));
    let interpreter = run(&mut owner, engine(peer.mission_scripts.clone(), tracking));
    // Exercise an actual tracking offer before interpreting its target union.
    owner
        .headless_camera_view_offer_with_script_engine(33, Some(&interpreter))
        .unwrap();
    let cached = owner
        .session_view
        .camera
        .rotation_registers
        .cached_target()
        .unwrap();
    let timing = owner.session_view.camera.rotation;
    let frame = owner.session_view.camera.camera_yaw_frame;
    let mut near_zero = ScriptList::new();
    near_zero.append_script(script(
        "NearZeroAfterFreeze",
        vec![
            Box::new(ScriptAction::new(ScriptActionType::CameraModFreezeAngle)),
            action(
                ScriptActionType::CameraLookTowardWaypoint,
                vec![
                    Parameter::with_string(ParameterType::Waypoint, "LookPoint".into()),
                    Parameter::with_real(ParameterType::Real, 0.99),
                    Parameter::with_real(ParameterType::Real, 0.),
                    Parameter::with_real(ParameterType::Real, 0.),
                    Parameter::with_int(ParameterType::Boolean, 0),
                ],
            ),
        ],
        true,
    ));
    let foreign_before = peer.capture_camera_continuation().unwrap();
    let owner_interpreter = run(&mut owner, engine(peer.mission_scripts.clone(), near_zero));
    assert_eq!(peer.capture_camera_continuation().unwrap(), foreign_before);
    assert!(!owner.session_view.camera.rotation_registers.is_tracking());
    assert!(
        owner
            .session_view
            .camera
            .rotation_registers
            .legacy_id()
            .is_none()
    );
    assert_eq!(
        owner
            .session_view
            .camera
            .rotation_registers
            .scalar_start()
            .unwrap_or(0.)
            .to_bits(),
        0,
        "freeze cleared the source ID, not the tracking variant"
    );
    assert_eq!(
        owner
            .session_view
            .camera
            .rotation_registers
            .scalar_target()
            .map(f32::to_bits),
        Some(cached.x.to_bits())
    );
    assert_eq!(owner.session_view.camera.camera_yaw_frame, frame);
    assert_eq!(owner.session_view.camera.rotation.turn_frames, 30);
    assert_eq!(owner.session_view.camera.rotation.time, timing.time);
    assert_eq!(
        owner
            .session_view
            .camera
            .rotation
            .duration_seconds()
            .to_bits(),
        timing.duration_seconds().to_bits()
    );
    let saved = owner.capture_camera_continuation().unwrap();
    let (mut receiving, _, _) = world(110.);
    receiving
        .admit_camera_continuation(GameLogic::decode_camera_continuation(&saved).unwrap())
        .unwrap();
    // This camera companion does not contain ScriptEngine state. Keep the
    // actual post-action interpreter for the source and give the restored
    // receiving world its own explicit interpreter, as required by the normal
    // checked View admission. No ambient interpreter or debug bypass is used.
    let receiving_interpreter = engine(receiving.mission_scripts.clone(), ScriptList::new());
    for _ in 0..3 {
        owner
            .headless_camera_view_offer_with_script_engine(33, Some(&owner_interpreter))
            .unwrap();
        receiving
            .headless_camera_view_offer_with_script_engine(33, Some(&receiving_interpreter))
            .unwrap();
        assert_eq!(
            owner.capture_camera_continuation().unwrap(),
            receiving.capture_camera_continuation().unwrap()
        );
    }
}

#[test]
fn authored_tracking_negative_multiplier_uses_original_nearest_rounding_on_live_target() {
    let (mut owner, owner_id, _) = world(110.);
    let (mut peer, peer_id, _) = world(210.);
    assert_eq!(owner_id, peer_id);
    owner.visual_speed_multiplier = 5.;
    owner.session_view.target = Vec3::ZERO;
    let peer_before = peer.capture_camera_continuation().unwrap();
    let mut list = ScriptList::new();
    list.append_script(script(
        "TrackingNegativeMultiplier",
        vec![
            action(
                ScriptActionType::CameraLookTowardObject,
                vec![
                    Parameter::with_string(ParameterType::Unit, "FocusScout".into()),
                    Parameter::with_real(ParameterType::Real, 0.066),
                    Parameter::with_real(ParameterType::Real, 0.),
                    Parameter::with_real(ParameterType::Real, 0.),
                    Parameter::with_real(ParameterType::Real, 0.),
                ],
            ),
            ints(ScriptActionType::CameraModSetFinalSpeedMultiplier, &[2]),
        ],
        true,
    ));
    let interpreter = run(&mut owner, engine(peer.mission_scripts.clone(), list));
    assert_eq!(
        owner.visual_speed_multiplier, 5.,
        "authored modifier changes the tracking endpoint, not current time"
    );
    assert_eq!(owner.session_view.camera.rotation.turn_frames, 2);
    assert_eq!(owner.session_view.camera.rotation.time.start, 5);
    assert_eq!(owner.session_view.camera.rotation.time.end, 2);
    assert_eq!(
        owner.session_view.camera.rotation_registers.legacy_id(),
        Some(owner_id.0)
    );
    assert_eq!(owner.session_view.camera.camera_yaw_frame, 0);
    let angle_before = owner.session_view.camera.camera_yaw_radians;
    let first_target = Vec3::new(100., 15., 0.);
    owner
        .host_object_mut(owner_id)
        .unwrap()
        .set_position(first_target);
    peer.host_object_mut(peer_id)
        .unwrap()
        .set_position(Vec3::new(0., 25., 100.));
    owner
        .headless_camera_view_offer_with_script_engine(33, Some(&interpreter))
        .unwrap();
    assert_eq!(owner.session_view.camera.camera_yaw_frame, 1);
    assert_eq!(
        owner
            .session_view
            .camera
            .rotation_registers
            .cached_target()
            .unwrap(),
        first_target
    );
    assert_ne!(
        owner.session_view.camera.camera_yaw_radians.to_bits(),
        angle_before.to_bits()
    );
    assert!(owner.session_view.camera.rotation_registers.active());
    // Original BaseType fast_float_floor under GameLogic's RC_NEAR:
    // delta=-3 * 0.5=-1.5; float(0.5+delta)=-1; subtract almost1 rounds
    // to-2, then truncation gives-2, so the first view multiplier is5-2=3.
    // The removed f64 mathematical floor path incorrectly yielded4 here.
    assert_eq!(owner.visual_speed_multiplier, 3.);
    assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
    let second_target = Vec3::new(0., 15., 100.);
    owner
        .host_object_mut(owner_id)
        .unwrap()
        .set_position(second_target);
    owner
        .headless_camera_view_offer_with_script_engine(33, Some(&interpreter))
        .unwrap();
    assert_eq!(owner.session_view.camera.camera_yaw_frame, 2);
    assert_eq!(
        owner
            .session_view
            .camera
            .rotation_registers
            .cached_target()
            .unwrap(),
        second_target
    );
    assert_eq!(owner.visual_speed_multiplier, 2.);
    assert!(!owner.session_view.camera.rotation_registers.active());
    assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
}
