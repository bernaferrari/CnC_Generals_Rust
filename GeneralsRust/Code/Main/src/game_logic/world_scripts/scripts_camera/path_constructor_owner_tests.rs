//! Actual receiving camera path construction: terrain chain and native yaw.
//! No tactical View, global terrain fixture, GPU, or Engine mirror is used.
use super::*;

fn terrain_chain(name: &str, x: f32) -> TerrainLogic {
    let mut map = MapData::new();
    map.width = 64;
    map.height = 64;
    map.heightmap = vec![60; 64 * 64];
    for (id, name, dx) in [(1, name, 0.), (2, "OwnedCameraEnd", 40.)] {
        map.waypoints.push(MapWaypoint {
            id,
            name: name.into(),
            location: gamelogic::common::Coord3D::new(x + dx, x + 20. + dx, -123.),
            path_label1: String::new(),
            path_label2: String::new(),
            path_label3: String::new(),
            bi_directional: false,
        });
    }
    map.waypoint_links.push((1, 2));
    let mut terrain = TerrainLogic::new();
    terrain.load_map_data(map);
    terrain
}
fn path_action(name: &str) -> Box<ScriptAction> {
    action(
        ScriptActionType::MoveCameraAlongWaypointPath,
        vec![
            Parameter::with_string(ParameterType::WaypointPath, name.into()),
            Parameter::with_real(ParameterType::Real, 8.),
            Parameter::with_real(ParameterType::Real, 0.),
            Parameter::with_real(ParameterType::Real, 0.2),
            Parameter::with_real(ParameterType::Real, 0.3),
        ],
    )
}
fn authored(
    owner: &mut GameLogic,
    foreign: &Arc<MissionScriptHooks>,
    actions: Vec<Box<ScriptAction>>,
) {
    let mut list = ScriptList::new();
    list.append_script(script("OwnedCameraPath", actions, true));
    owner.frame += 1;
    let _ = run(owner, engine(Arc::clone(foreign), list));
}
fn assert_path(owner: &GameLogic, x: f32, yaw: f32, start: Vec3) {
    let path = &owner.session_view.scripted.motion;
    assert!(path.active, "authored camera path uses receiving terrain");
    assert_eq!(path.points[1].world(), start);
    assert_eq!(path.angles[0], yaw);
    assert!(
        path.angles[1..=path.num_waypoints]
            .iter()
            .all(|angle| *angle == yaw)
    );
    assert!(
        path.points
            .iter()
            .any(|point| point.world() == Vec3::new(x, 37.5, x + 20.))
    );
    assert!(
        path.points
            .iter()
            .any(|point| point.world() == Vec3::new(x + 40., 37.5, x + 60.))
    );
    assert_eq!(path.total_time_ms, 8000);
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn authored_camera_path_owns_receiving_chain_and_yaw_through_peer_reset_and_encoded_continuation() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "authored_camera_path_owns_receiving_chain_and_yaw_through_peer_reset_and_encoded_continuation",
        || {
            let (mut a, id, _) = world(110.);
            let (mut b, other, _) = world(210.);
            let (mut restored, _, _) = world(110.);
            assert_eq!(id, other);
            *a.world_services.terrain().write().unwrap() = terrain_chain("SameCameraPath", 111.);
            *b.world_services.terrain().write().unwrap() = terrain_chain("SameCameraPath", 411.);
            a.session_view.target = Vec3::new(10., 0., 20.);
            b.session_view.target = Vec3::new(70., 0., 80.);
            a.session_view.camera.camera_yaw_radians = 0.35;
            b.session_view.camera.camera_yaw_radians = -0.8;
            let b_before = b.capture_camera_continuation().unwrap();
            authored(
                &mut a,
                &b.mission_scripts,
                vec![
                    path_action("SameCameraPath"),
                    Box::new(ScriptAction::new(ScriptActionType::CameraModFreezeAngle)),
                ],
            );
            assert_path(&a, 111., 0.35, Vec3::new(10., 0., 20.));
            assert_eq!(b.capture_camera_continuation().unwrap(), b_before);
            assert_eq!(b.session_view.camera.camera_yaw_radians, -0.8);
            let a_before = a.capture_camera_continuation().unwrap();
            authored(
                &mut b,
                &a.mission_scripts,
                vec![
                    path_action("SameCameraPath"),
                    Box::new(ScriptAction::new(ScriptActionType::CameraModFreezeAngle)),
                ],
            );
            assert_path(&b, 411., -0.8, Vec3::new(70., 0., 80.));
            assert_eq!(a.capture_camera_continuation().unwrap(), a_before);
            b.reset();
            assert_eq!(a.capture_camera_continuation().unwrap(), a_before);
            assert_eq!(a.session_view.camera.camera_yaw_radians, 0.35);
            // LogicCameraContinuation does not contain the separately saved
            // persistent native target. Supply that paired owner value before
            // comparing future pending-focus origins.
            restored.session_view.target = a.session_view.target;
            restored.session_view.camera.camera_yaw_radians = 2.1;
            restored.admit_camera_continuation(
                GameLogic::decode_camera_continuation(&a_before).unwrap(),
            );
            assert_path(&restored, 111., 0.35, Vec3::new(10., 0., 20.));
            assert_eq!(
                restored.session_view.camera.camera_yaw_radians, 0.35,
                "owned continuation restores the same canonical native pose"
            );
            for _ in 0..114 {
                a.headless_camera_view_offer(33);
                restored.headless_camera_view_offer(33);
                assert_eq!(
                    a.capture_camera_continuation().unwrap(),
                    restored.capture_camera_continuation().unwrap()
                );
                assert_eq!(a.session_view.target, restored.session_view.target);
            }
            assert_camera_queues_empty(&a.mission_scripts);
            assert_camera_queues_empty(&b.mission_scripts);
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn authored_missing_camera_path_preserves_existing_move_path_or_rotate_like_no_op_phase() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "authored_missing_camera_path_preserves_existing_move_path_or_rotate_like_no_op_phase",
        || {
            let (mut peer, _, _) = world(210.);
            *peer.world_services.terrain().write().unwrap() =
                terrain_chain("MissingCameraPath", 411.);
            peer.session_view.camera.camera_yaw_radians = -0.8;
            let peer_before = peer.capture_camera_continuation().unwrap();
            for prior in 0..3 {
                let (mut owner, id, _) = world(110.);
                let (mut control, control_id, _) = world(110.);
                assert_eq!(id, control_id);
                *owner.world_services.terrain().write().unwrap() =
                    terrain_chain("PresentCameraPath", 111.);
                owner.session_view.camera.camera_yaw_radians = 0.35;
                let initial = match prior {
                    0 => action_move_for_modifier(),
                    1 => path_action("PresentCameraPath"),
                    _ => reals(ScriptActionType::RotateCamera, &[0.75, 8., 0.2, 0.3]),
                };
                authored(
                    &mut owner,
                    &peer.mission_scripts,
                    vec![
                        initial,
                        reals(ScriptActionType::ZoomCamera, &[0.8, 6., 0.1, 0.2]),
                        Box::new(ScriptAction::new(ScriptActionType::CameraModFreezeTime)),
                    ],
                );
                let baseline = owner.capture_camera_continuation().unwrap();
                control.admit_camera_continuation(
                    GameLogic::decode_camera_continuation(&baseline).unwrap(),
                );
                control.session_view.camera.camera_yaw_radians = 0.35;
                *control.world_services.terrain().write().unwrap() =
                    terrain_chain("PresentCameraPath", 111.);
                // Both actual walks perform the same ordinary dt=0 camera phase.
                // Comparing against NO_OP retains legitimate existing path focus,
                // shutter, look orientation and freeze instead of clearing them.
                authored(
                    &mut owner,
                    &peer.mission_scripts,
                    vec![path_action("MissingCameraPath")],
                );
                authored(
                    &mut control,
                    &peer.mission_scripts,
                    vec![Box::new(ScriptAction::new(ScriptActionType::NoOp))],
                );
                assert_eq!(
                    owner.capture_camera_continuation().unwrap(),
                    control.capture_camera_continuation().unwrap(),
                    "missing start must not call View/setup or cancel existing movement; prior={prior}"
                );
                assert_eq!(owner.session_view.camera.camera_yaw_radians, 0.35);
                assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
                assert_camera_queues_empty(&peer.mission_scripts);
            }
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn authored_missing_camera_path_keeps_idle_completion_and_armed_modifiers() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "authored_missing_camera_path_keeps_idle_completion_and_armed_modifiers",
        || {
            let (mut peer, _, _) = world(210.);
            let (mut owner, _, _) = world(110.);
            let (mut control, _, _) = world(110.);
            *peer.world_services.terrain().write().unwrap() =
                terrain_chain("MissingCameraPath", 411.);
            *owner.world_services.terrain().write().unwrap() = TerrainLogic::new();
            owner.session_view.scripted.script_camera_freeze_time = true;
            owner.apply_script_camera_mod_freeze_angle();
            owner.visual_speed_multiplier = 2.;
            let baseline = owner.capture_camera_continuation().unwrap();
            control.admit_camera_continuation(
                GameLogic::decode_camera_continuation(&baseline).unwrap(),
            );
            authored(
                &mut owner,
                &peer.mission_scripts,
                vec![path_action("MissingCameraPath")],
            );
            authored(
                &mut control,
                &peer.mission_scripts,
                vec![Box::new(ScriptAction::new(ScriptActionType::NoOp))],
            );
            assert!(owner.camera_movement_finished());
            assert!(owner.mission_scripts.is_camera_movement_finished());
            assert_eq!(
                owner.capture_camera_continuation().unwrap(),
                control.capture_camera_continuation().unwrap()
            );
            assert_camera_queues_empty(&peer.mission_scripts);
        },
    );
}
