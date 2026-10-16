//! Actual authored waypoint camera actions resolve the receiving terrain.
//! Map admission grounds waypoints; no ambient terrain fixture is installed.
use super::*;

fn receiving_terrain(x: f32, height: u8) -> TerrainLogic {
    let mut map = MapData::new();
    map.width = 64;
    map.height = 64;
    map.heightmap = vec![height; 64 * 64];
    for (id, name, dx) in [(1, "CameraPoint", 0.), (2, "LookPoint", 40.)] {
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
    let mut terrain = TerrainLogic::new();
    terrain.load_map_data(map);
    terrain
}
fn execute(
    world: &mut GameLogic,
    foreign: &Arc<MissionScriptHooks>,
    actions: Vec<Box<ScriptAction>>,
) {
    let mut list = ScriptList::new();
    list.append_script(script("ReceivingWaypointAction", actions, true));
    world.frame += 1;
    let _ = run(world, engine(Arc::clone(foreign), list));
}
fn selected_action(kind: ScriptActionType) -> Box<ScriptAction> {
    if kind == ScriptActionType::MoveCameraTo {
        return action(
            kind,
            vec![
                Parameter::with_string(ParameterType::Waypoint, "CameraPoint".into()),
                Parameter::with_real(ParameterType::Real, 1.75),
                Parameter::with_real(ParameterType::Real, 0.15),
                Parameter::with_real(ParameterType::Real, 0.2),
                Parameter::with_real(ParameterType::Real, 0.3),
            ],
        );
    }
    remaining_camera_actions(100.)
        .into_iter()
        .find(|action| action.action_type == kind)
        .unwrap()
}
fn prepare_modifier(
    world: &mut GameLogic,
    foreign: &Arc<MissionScriptHooks>,
    kind: ScriptActionType,
) {
    if matches!(
        kind,
        ScriptActionType::CameraModLookToward | ScriptActionType::CameraModFinalLookToward
    ) {
        execute(world, foreign, vec![action_move_for_modifier()]);
        assert!(world.session_view.scripted.motion.active);
        assert!(world.pending_camera_look_toward.is_none());
    }
}
fn assert_effect(world: &GameLogic, kind: ScriptActionType, x: f32, ground: f32) {
    let camera = Vec3::new(x, ground, x + 20.);
    let look = Vec3::new(x + 40., ground, x + 60.);
    let motion = &world.session_view.scripted.motion;
    let native = &world.session_view.camera;
    match kind {
        ScriptActionType::MoveCameraTo | ScriptActionType::ResetCamera => {
            assert!(motion.active);
            assert_eq!(motion.points[motion.num_waypoints].world(), camera);
            assert_eq!(
                motion.total_time_ms,
                if kind == ScriptActionType::MoveCameraTo {
                    1750
                } else {
                    2500
                }
            );
            assert_eq!(
                world.pending_camera_zoom_reset,
                kind == ScriptActionType::ResetCamera
            );
            if kind == ScriptActionType::ResetCamera {
                assert_eq!(
                    (
                        world.pending_camera_zoom_reset_duration,
                        world.pending_camera_zoom_reset_ease_in,
                        world.pending_camera_zoom_reset_ease_out
                    ),
                    (2.5, 0.25, 0.75)
                );
                assert_eq!(motion.angles[2], 0.);
                assert_eq!(native.camera_pitch_target, Some(1.));
                assert!(native.camera_zoom_target.is_some());
                assert_eq!(world.session_view.scripted.camera_follow_target, None);
            }
        }
        ScriptActionType::SetupCamera => {
            assert!(motion.active);
            assert_eq!(
                motion.total_time_ms, 0,
                "setup1ms crossing defers actual focus"
            );
            assert_eq!(motion.points[2].world(), camera);
            assert_eq!(native.camera_pitch_target, Some(0.6));
            assert!(native.camera_zoom_target.is_some());
            assert!(world.peek_pending_camera_focus().is_none());
            assert!(world.pending_camera_look_toward.is_none());
            let dx = look.x - camera.x;
            let dy = look.z - camera.z;
            let expected = (f64::from(dx / (dx * dx + dy * dy).sqrt()).acos() as f32)
                - std::f32::consts::FRAC_PI_2;
            assert_eq!(motion.angles[2].to_bits(), expected.to_bits());
        }
        ScriptActionType::CameraLookTowardWaypoint => {
            let request = world.pending_camera_look_toward.as_ref().unwrap();
            assert_eq!(request.position, look);
            assert_eq!(
                (
                    native.camera_yaw_duration,
                    native.camera_yaw_ease_in,
                    native.camera_yaw_ease_out
                ),
                (1.1, 0.2, 0.3)
            );
            assert!(request.reverse_rotation);
            assert!(native.camera_yaw_target.is_some());
        }
        ScriptActionType::CameraModLookToward | ScriptActionType::CameraModFinalLookToward => {
            assert!(motion.active);
            assert!(
                world.pending_camera_look_toward.is_none(),
                "path writes its actual angle table"
            );
            let destination = motion.points[motion.num_waypoints];
            let dx = look.x - destination.x;
            let dy = look.z - destination.y;
            let length = f64::from(dx * dx + dy * dy).sqrt() as f32;
            let mut expected = f64::from(dx / length).acos() as f32;
            if dy < 0. {
                expected = -expected;
            }
            expected -= std::f32::consts::FRAC_PI_2;
            while expected < -std::f32::consts::PI {
                expected += std::f32::consts::TAU;
            }
            while expected > std::f32::consts::PI {
                expected -= std::f32::consts::TAU;
            }
            assert_eq!(
                motion.angles[motion.num_waypoints].to_bits(),
                expected.to_bits()
            );
        }
        ScriptActionType::CameraAddShakerAt => {
            let request = world.pending_camera_add_shakers.last().unwrap();
            assert_eq!(
                (
                    request.position,
                    request.amplitude,
                    request.duration_seconds,
                    request.radius
                ),
                (camera, 3., 1.5, 200.)
            );
        }
        _ => unreachable!(),
    }
}
fn assert_missing_effect(world: &GameLogic, kind: ScriptActionType) {
    let modifier = matches!(
        kind,
        ScriptActionType::CameraModLookToward | ScriptActionType::CameraModFinalLookToward
    );
    assert_eq!(world.session_view.scripted.motion.active, modifier);
    if modifier {
        let motion = &world.session_view.scripted.motion;
        assert_eq!(
            motion.points[motion.num_waypoints].world(),
            Vec3::new(800., 0., 600.)
        );
        assert_eq!(motion.total_time_ms, 3000);
        assert_eq!(motion.elapsed_ms, 0, "script walk never advances viewclock");
    }
    assert!(world.pending_camera_look_toward.is_none());
    assert!(world.pending_camera_add_shakers.is_empty());
    if matches!(
        kind,
        ScriptActionType::ResetCamera | ScriptActionType::MoveCameraTo
    ) {
        assert!(!world.pending_camera_zoom_reset);
    }
    if kind == ScriptActionType::SetupCamera {
        assert!(world.peek_pending_camera_focus().is_none());
        assert!(world.session_view.camera.camera_zoom_target.is_none());
        assert!(world.session_view.camera.camera_pitch_target.is_none());
    }
}
fn receiving_action(kind: ScriptActionType) {
    let (mut first, id, _) = world(110.);
    let (mut second, other, _) = world(210.);
    let (mut missing, missing_id, _) = world(310.);
    assert_eq!(id, other);
    assert_eq!(id, missing_id);
    *first.world_services.terrain().write().unwrap() = receiving_terrain(111., 60);
    *second.world_services.terrain().write().unwrap() = receiving_terrain(411., 140);
    *missing.world_services.terrain().write().unwrap() = TerrainLogic::new();
    prepare_modifier(&mut first, &second.mission_scripts, kind);
    prepare_modifier(&mut second, &first.mission_scripts, kind);
    prepare_modifier(&mut missing, &second.mission_scripts, kind);
    let peer_before = second.capture_camera_continuation().unwrap();
    execute(
        &mut first,
        &second.mission_scripts,
        vec![selected_action(kind)],
    );
    assert_effect(&first, kind, 111., 37.5);
    assert_eq!(second.capture_camera_continuation().unwrap(), peer_before);
    let first_before = first.capture_camera_continuation().unwrap();
    execute(
        &mut second,
        &first.mission_scripts,
        vec![selected_action(kind)],
    );
    assert_effect(&second, kind, 411., 87.5);
    assert_eq!(first.capture_camera_continuation().unwrap(), first_before);
    let second_before = second.capture_camera_continuation().unwrap();
    execute(
        &mut missing,
        &second.mission_scripts,
        vec![selected_action(kind)],
    );
    assert_missing_effect(&missing, kind);
    assert_eq!(first.capture_camera_continuation().unwrap(), first_before);
    assert_eq!(second.capture_camera_continuation().unwrap(), second_before);
    let (mut restored, _, _) = world(110.);
    restored
        .admit_camera_continuation(GameLogic::decode_camera_continuation(&first_before).unwrap())
        .unwrap();
    assert_eq!(
        restored.capture_camera_continuation().unwrap(),
        first_before,
        "encoded camera continuation retains the receiving authored result"
    );
    assert_effect(&restored, kind, 111., 37.5);
    assert_camera_queues_empty(&first.mission_scripts);
    assert_camera_queues_empty(&second.mission_scripts);
    assert_camera_queues_empty(&missing.mission_scripts);
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn authored_setup_camera_uses_receiving_waypoints_and_missing_is_authoritative() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "authored_setup_camera_uses_receiving_waypoints_and_missing_is_authoritative",
        || receiving_action(ScriptActionType::SetupCamera),
    );
}
#[test]
#[cfg(not(target_arch = "wasm32"))]
fn authored_look_toward_waypoint_uses_receiving_terrain_and_missing_is_authoritative() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "authored_look_toward_waypoint_uses_receiving_terrain_and_missing_is_authoritative",
        || receiving_action(ScriptActionType::CameraLookTowardWaypoint),
    );
}
#[test]
#[cfg(not(target_arch = "wasm32"))]
fn authored_mod_final_look_toward_uses_receiving_terrain_and_missing_is_authoritative() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "authored_mod_final_look_toward_uses_receiving_terrain_and_missing_is_authoritative",
        || receiving_action(ScriptActionType::CameraModFinalLookToward),
    );
}
#[test]
#[cfg(not(target_arch = "wasm32"))]
fn authored_mod_look_toward_uses_receiving_terrain_and_missing_is_authoritative() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "authored_mod_look_toward_uses_receiving_terrain_and_missing_is_authoritative",
        || receiving_action(ScriptActionType::CameraModLookToward),
    );
}
#[test]
#[cfg(not(target_arch = "wasm32"))]
fn authored_add_shaker_uses_receiving_waypoint_and_missing_is_authoritative() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "authored_add_shaker_uses_receiving_waypoint_and_missing_is_authoritative",
        || receiving_action(ScriptActionType::CameraAddShakerAt),
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn authored_reset_camera_uses_receiving_waypoint_and_missing_is_authoritative() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "authored_reset_camera_uses_receiving_waypoint_and_missing_is_authoritative",
        || receiving_action(ScriptActionType::ResetCamera),
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn authored_move_camera_to_waypoint_uses_receiving_terrain_and_missing_is_authoritative() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "authored_move_camera_to_waypoint_uses_receiving_terrain_and_missing_is_authoritative",
        || receiving_action(ScriptActionType::MoveCameraTo),
    );
}
