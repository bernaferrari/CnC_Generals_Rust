//! Real authored object tracking through the receiving admitted view owner.
use super::*;

fn authored_tracking(world: &mut GameLogic, foreign: &Arc<MissionScriptHooks>) {
    let mut list = ScriptList::new();
    list.append_script(script(
        "ReceivingTracking",
        vec![
            action(
                ScriptActionType::CameraLookTowardObject,
                vec![
                    Parameter::with_string(ParameterType::Unit, "FocusScout".into()),
                    Parameter::with_real(ParameterType::Real, 0.099),
                    Parameter::with_real(ParameterType::Real, 0.132),
                    Parameter::with_real(ParameterType::Real, 0.),
                    Parameter::with_real(ParameterType::Real, 0.),
                ],
            ),
            Box::new(ScriptAction::new(ScriptActionType::CameraModFreezeTime)),
        ],
        true,
    ));
    world.frame += 1;
    let _ = run(world, engine(Arc::clone(foreign), list));
    let tracking = world
        .session_view
        .scripted
        .object_tracking
        .as_ref()
        .unwrap();
    assert_eq!((tracking.turn_frames, tracking.hold_frames), (3, 4));
    assert_eq!(world.session_view.camera.camera_yaw_frame, 0);
    assert!(world.session_view.camera.camera_yaw_target.is_some());
}
fn position(world: &mut GameLogic, id: ObjectId, position: Vec3) {
    world.host_object_mut(id).unwrap().set_position(position);
}
fn cached(world: &GameLogic) -> Vec3 {
    world
        .session_view
        .scripted
        .object_tracking
        .as_ref()
        .unwrap()
        .last_position
}
fn angle_to(position: Vec3) -> f32 {
    let length = f64::from(position.x * position.x + position.z * position.z).sqrt() as f32;
    let mut angle = f64::from(position.x / length).acos() as f32;
    if position.z < 0. {
        angle = -angle;
    }
    angle - std::f32::consts::FRAC_PI_2
}
#[test]
#[cfg(not(target_arch = "wasm32"))]
fn authored_tracking_moves_only_receiving_same_id_and_retains_removed_target_through_save() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "authored_tracking_moves_only_receiving_same_id_and_retains_removed_target_through_save",
        || {
            let (mut owner, id, _) = world(110.);
            let (mut peer, peer_id, _) = world(210.);
            assert_eq!(id, peer_id);
            let peer_before = peer.capture_camera_continuation().unwrap();
            authored_tracking(&mut owner, &peer.mission_scripts);
            assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
            let owner_before = owner.capture_camera_continuation().unwrap();
            authored_tracking(&mut peer, &owner.mission_scripts);
            assert_eq!(owner.capture_camera_continuation().unwrap(), owner_before);
            let first = Vec3::new(100., 15., 0.);
            let foreign = Vec3::new(0., 25., 100.);
            position(&mut owner, id, first);
            position(&mut peer, peer_id, foreign);
            owner.headless_camera_view_offer(33);
            assert_eq!(cached(&owner), first);
            assert!(owner.session_view.camera.camera_yaw_radians < 0.);
            peer.headless_camera_view_offer(33);
            assert_eq!(cached(&peer), foreign);
            assert_eq!(
                peer.session_view.camera.camera_yaw_radians.to_bits(),
                0.0f32.to_bits()
            );
            let peer_camera = peer.capture_camera_continuation().unwrap();
            let initial_yaw = owner.session_view.camera.camera_yaw_radians;
            position(&mut owner, id, Vec3::new(0., 35., 100.));
            owner.headless_camera_view_offer(33);
            assert_eq!(cached(&owner), Vec3::new(0., 35., 100.));
            assert!(
                owner.session_view.camera.camera_yaw_radians > initial_yaw,
                "turn phase re-eases from the current angle to the moving target"
            );
            let turn_end = Vec3::new(100., 45., 100.);
            position(&mut owner, id, turn_end);
            let prior = owner.session_view.camera.camera_yaw_radians;
            let delta = angle_to(turn_end) - prior;
            let expected_turn_end = prior + delta;
            owner.headless_camera_view_offer(33);
            assert_eq!(
                owner.session_view.camera.camera_yaw_radians.to_bits(),
                expected_turn_end.to_bits(),
                "turn terminal retains original current-angle addition ordering"
            );
            let hold_target = Vec3::new(-100., 55., 0.);
            position(&mut owner, id, hold_target);
            owner.headless_camera_view_offer(33);
            assert_eq!(owner.session_view.camera.camera_yaw_frame, 4);
            assert_eq!(cached(&owner), hold_target);
            assert_eq!(
                owner.session_view.camera.camera_yaw_radians.to_bits(),
                angle_to(hold_target).to_bits(),
                "hold phase follows directly instead of restarting interpolation"
            );
            owner.destroy_object(id);
            assert!(
                owner.objects.contains_key(&id),
                "destroy is deferred until the real cleanup phase"
            );
            owner.process_destroy_list();
            assert!(!owner.objects.contains_key(&id));
            let encoded = owner.capture_camera_continuation().unwrap();
            let (mut restored, restored_id, _) = world(110.);
            assert_eq!(id, restored_id);
            restored.destroy_object(restored_id);
            restored.process_destroy_list();
            assert!(!restored.objects.contains_key(&restored_id));
            restored.admit_camera_continuation(
                GameLogic::decode_camera_continuation(&encoded).unwrap(),
            );
            assert_eq!(restored.capture_camera_continuation().unwrap(), encoded);
            for frame in 5..=7 {
                owner.headless_camera_view_offer(33);
                restored.headless_camera_view_offer(33);
                assert_eq!(
                    cached(&owner),
                    hold_target,
                    "removed target retains the last observed receiving position"
                );
                assert_eq!(owner.session_view.camera.camera_yaw_frame, frame);
                assert_eq!(
                    owner.capture_camera_continuation().unwrap(),
                    restored.capture_camera_continuation().unwrap()
                );
                assert_eq!(
                    owner.session_view.camera.camera_yaw_target.is_some(),
                    frame < 7
                );
                assert_eq!(owner.is_script_camera_time_frozen(), frame < 7);
            }
            assert_eq!(peer.capture_camera_continuation().unwrap(), peer_camera);
            assert_camera_queues_empty(&peer.mission_scripts);
            assert_camera_queues_empty(&owner.mission_scripts);
        },
    );
}
#[test]
#[cfg(not(target_arch = "wasm32"))]
fn authored_tracking_freeze_angle_invalidates_id_but_keeps_last_target_and_original_clock() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "authored_tracking_freeze_angle_invalidates_id_but_keeps_last_target_and_original_clock",
        || {
            let (mut owner, id, _) = world(110.);
            let (mut peer, peer_id, _) = world(210.);
            assert_eq!(id, peer_id);
            authored_tracking(&mut owner, &peer.mission_scripts);
            let target = Vec3::new(100., 15., 0.);
            position(&mut owner, id, target);
            owner.headless_camera_view_offer(33);
            let first_yaw = owner.session_view.camera.camera_yaw_radians;
            let mut list = ScriptList::new();
            list.append_script(script(
                "FreezeTrackedId",
                vec![Box::new(ScriptAction::new(
                    ScriptActionType::CameraModFreezeAngle,
                ))],
                true,
            ));
            owner.frame += 1;
            let _ = run(&mut owner, engine(Arc::clone(&peer.mission_scripts), list));
            assert_eq!(
                owner.session_view.scripted.script_look_toward_object_id,
                None
            );
            assert_eq!(owner.session_view.camera.camera_yaw_frame, 1);
            position(&mut owner, id, Vec3::new(0., 25., 100.));
            peer.reset();
            owner.headless_camera_view_offer(33);
            assert_eq!(cached(&owner), target);
            assert!(
                owner.session_view.camera.camera_yaw_radians < first_yaw,
                "source invalidates the ID, not the cached target or remaining turn"
            );
            for _ in 0..5 {
                owner.headless_camera_view_offer(33);
            }
            assert_eq!(owner.session_view.camera.camera_yaw_frame, 7);
            assert!(owner.session_view.camera.camera_yaw_target.is_none());
            assert_eq!(cached(&owner), target);
            assert_eq!(
                owner.session_view.camera.camera_yaw_radians.to_bits(),
                angle_to(target).to_bits()
            );
            assert!(!owner.is_script_camera_time_frozen());
            assert_camera_queues_empty(&peer.mission_scripts);
        },
    );
}
