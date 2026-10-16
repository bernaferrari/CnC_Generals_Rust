//! Authored named camera actions resolve through the driving host owner.
use super::*;

fn named_camera_action(kind: ScriptActionType, name: &str) -> Box<ScriptAction> {
    let mut parameters = vec![Parameter::with_string(ParameterType::Unit, name.into())];
    if kind == ScriptActionType::CameraLookTowardObject {
        parameters.extend(
            [1., 2., 0.3, 0.4]
                .into_iter()
                .map(|v| Parameter::with_real(ParameterType::Real, v)),
        );
    } else {
        parameters.push(Parameter::with_int(ParameterType::Boolean, 0));
        if kind == ScriptActionType::CameraTetherNamed {
            parameters.push(Parameter::with_real(ParameterType::Real, 1.75));
        }
    }
    action(kind, parameters)
}
fn authored_named(owner: &mut GameLogic, peer: &GameLogic, kind: ScriptActionType, name: &str) {
    let mut list = ScriptList::new();
    list.append_script(script(
        "ReceivingNamedCamera",
        vec![named_camera_action(kind, name)],
        true,
    ));
    let _ = run(owner, engine(peer.mission_scripts.clone(), list));
}

#[test]
fn named_camera_actions_use_receiving_id_and_exact_name() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "named_camera_actions_use_receiving_id_and_exact_name",
        || {
            for kind in [
                ScriptActionType::CameraFollowNamed,
                ScriptActionType::CameraTetherNamed,
                ScriptActionType::CameraLookTowardObject,
            ] {
                let (mut owner, original, _) = world(110.);
                let (peer, peer_id, _) = world(210.);
                owner.host_object_mut(original).unwrap().name = "OtherScout".into();
                let position = Vec3::new(350., 0., 470.);
                let id = owner
                    .create_object_for_player("CameraOwnerUnit", 0, position)
                    .unwrap();
                owner.host_object_mut(id).unwrap().name = "FocusScout".into();
                assert_ne!(id, peer_id);
                authored_named(&mut owner, &peer, kind, "FocusScout");
                if kind == ScriptActionType::CameraLookTowardObject {
                    assert_eq!(
                        owner.session_view.scripted.script_look_toward_object_id,
                        Some(id.0)
                    );
                    let tracking = owner
                        .session_view
                        .scripted
                        .object_tracking
                        .as_ref()
                        .unwrap();
                    assert_eq!(tracking.last_position, position);
                    assert_eq!(tracking.hold_frames, 60);
                } else {
                    assert_eq!(owner.camera_follow_object_id(), Some(id));
                    assert_eq!(
                        owner.session_view.scripted.camera_tether_play,
                        (kind == ScriptActionType::CameraTetherNamed).then_some(1.75)
                    );
                }
                assert_eq!(peer.camera_follow_object_id(), None);
                assert_eq!(
                    peer.session_view.scripted.script_look_toward_object_id,
                    None
                );
                assert_camera_queues_empty(&peer.mission_scripts);
            }
        },
    );
}

#[test]
fn named_camera_missing_and_case_mismatch_preserve_receiving_camera() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "named_camera_missing_and_case_mismatch_preserve_receiving_camera",
        || {
            for kind in [
                ScriptActionType::CameraFollowNamed,
                ScriptActionType::CameraTetherNamed,
                ScriptActionType::CameraLookTowardObject,
            ] {
                for name in ["PeerOnly", "focusscout"] {
                    let (mut owner, id, _) = world(110.);
                    let (mut peer, peer_id, _) = world(210.);
                    peer.host_object_mut(peer_id).unwrap().name = "PeerOnly".into();
                    owner.session_view.scripted.camera_follow_target = Some(id);
                    owner.session_view.scripted.camera_tether_play = Some(7.);
                    owner.session_view.camera.install_zoom(2., 3., 0., 0.);
                    let count = owner.host_objects().len();
                    authored_named(&mut owner, &peer, kind, name);
                    assert_eq!(owner.camera_follow_object_id(), Some(id));
                    assert_eq!(owner.session_view.scripted.camera_tether_play, Some(7.));
                    assert_eq!(
                        owner.session_view.scripted.script_look_toward_object_id,
                        None
                    );
                    assert!(owner.session_view.scripted.object_tracking.is_none());
                    assert_eq!(owner.session_view.camera.camera_zoom_target, Some(2.));
                    assert_eq!(
                        owner.host_objects().len(),
                        count,
                        "missing camera target must not be created"
                    );
                    assert_camera_queues_empty(&peer.mission_scripts);
                }
            }
        },
    );
}

#[test]
fn named_camera_this_object_uses_driving_interpreter_and_restores_cursor() {
    use super::super::script_execution_driver::HostScriptExecutionDriver;
    use gamelogic::scripting::executor::ScriptContext;
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "named_camera_this_object_uses_driving_interpreter_and_restores_cursor",
        || {
            for kind in [
                ScriptActionType::CameraFollowNamed,
                ScriptActionType::CameraTetherNamed,
                ScriptActionType::CameraLookTowardObject,
            ] {
                let (mut owner, id, _) = world(110.);
                let (peer, _, _) = world(210.);
                let mut list = ScriptList::new();
                let mut sub = script(
                    "CameraCallingObject",
                    vec![named_camera_action(
                        kind,
                        gamelogic::scripting::core::THIS_OBJECT,
                    )],
                    true,
                );
                sub.is_subroutine = true;
                list.append_script(sub);
                let interpreter = engine(peer.mission_scripts.clone(), list);
                let frame = owner.frame;
                assert!(
                    interpreter
                        .run_object_script_with_driver(
                            "CameraCallingObject",
                            id.0,
                            ScriptContext::at_frame(frame),
                            &mut HostScriptExecutionDriver::with_interpreter(
                                &mut owner,
                                &interpreter
                            )
                        )
                        .unwrap()
                );
                if kind == ScriptActionType::CameraLookTowardObject {
                    assert_eq!(
                        owner.session_view.scripted.script_look_toward_object_id,
                        Some(id.0)
                    );
                } else {
                    assert_eq!(owner.camera_follow_object_id(), Some(id));
                }
                // The cursor must restore after the subroutine; a plain action
                // using THIS_OBJECT outside an object call is an owned miss.
                owner.session_view.scripted.camera_follow_target = None;
                owner.session_view.scripted.script_look_toward_object_id = None;
                owner.session_view.scripted.object_tracking = None;
                interpreter.friend_execute_action_with_driver(
                    &named_camera_action(kind, gamelogic::scripting::core::THIS_OBJECT),
                    None,
                    ScriptContext::at_frame(frame),
                    &mut HostScriptExecutionDriver::with_interpreter(&mut owner, &interpreter),
                );
                assert_eq!(owner.camera_follow_object_id(), None);
                assert_eq!(
                    owner.session_view.scripted.script_look_toward_object_id,
                    None
                );
                assert!(owner.session_view.scripted.object_tracking.is_none());
                assert_camera_queues_empty(&peer.mission_scripts);
            }
        },
    );
}

#[test]
fn authored_named_tracking_encoded_continuation_keeps_receiving_identity() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "authored_named_tracking_encoded_continuation_keeps_receiving_identity",
        || {
            let (mut owner, id, _) = world(110.);
            let (mut restored, restored_id, _) = world(110.);
            let (mut peer, _, _) = world(210.);
            assert_eq!(id, restored_id);
            authored_named(
                &mut owner,
                &peer,
                ScriptActionType::CameraLookTowardObject,
                "FocusScout",
            );
            let encoded = owner.capture_camera_continuation().unwrap();
            peer.session_view.scripted.object_tracking = None;
            peer.session_view.target = Vec3::new(-500., 0., -600.);
            restored.admit_camera_continuation(
                GameLogic::decode_camera_continuation(&encoded).unwrap(),
            );
            assert_eq!(restored.capture_camera_continuation().unwrap(), encoded);
            assert_eq!(
                restored.session_view.scripted.script_look_toward_object_id,
                Some(id.0)
            );
            assert_eq!(
                restored
                    .session_view
                    .scripted
                    .object_tracking
                    .as_ref()
                    .unwrap()
                    .last_position,
                owner.host_object(id).unwrap().get_position()
            );
            assert_eq!(peer.session_view.target, Vec3::new(-500., 0., -600.));
        },
    );
}
