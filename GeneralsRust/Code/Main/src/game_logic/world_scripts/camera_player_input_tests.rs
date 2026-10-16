//! Real receiving player setters. Pure CPU witnesses; Engine event guard is supplemental.
use super::*;
fn owner() -> GameLogic {
    let mut world = GameLogic::new();
    let mut map = gamelogic::system::map_loader::MapData::new();
    map.width = 64;
    map.height = 64;
    map.heightmap = vec![0; 4096];
    world
        .world_services
        .terrain()
        .write()
        .unwrap()
        .load_map_data(map);
    world.session_view.target = Vec3::new(250., 0., 250.);
    let camera = &mut world.session_view.camera;
    camera.rebuild_projection();
    camera
        .constraints
        .recalculate(Vec3::ZERO, Vec3::new(640., 0., 640.), 0.)
        .unwrap();
    camera.native_camera.set_height(200., 100., 300.);
    camera
        .script_camera_shakers
        .push(crate::presentation_frame::camera::ScriptCameraShaker::new(
            Vec3::ZERO,
            10000.,
            30.,
            0.,
        ));
    world.session_view.commit_orbit_pose();
    world
}
#[test]
fn two_wheel_spins_perform_two_complete_setters_even_at_minimum_without_hag_clock() {
    for initial_goal in [100., 200.] {
        let mut world = owner();
        let peer = owner();
        let peer_saved = peer.capture_camera_continuation().unwrap();
        world
            .session_view
            .camera
            .native_camera
            .set_height(initial_goal, 100., 300.);
        let before = serde_json::to_value(
            world
                .session_view
                .camera
                .native_camera
                .capture_continuation(),
        )
        .unwrap();
        let zoom = world.session_view.camera.camera_zoom;
        world
            .admit_player_camera_wheel(2, &mut ScriptPresentationEffects::default())
            .unwrap();
        let camera = &world.session_view.camera;
        assert_eq!(
            camera.native_camera.desired(),
            (initial_goal - 20.).max(100.)
        );
        assert_eq!(
            camera.camera_zoom, zoom,
            "height request does not advance settling"
        );
        assert_eq!(
            camera.script_camera_shakers[0].elapsed_seconds,
            4. * (1. / 30.),
            "two invalid setters each build twice"
        );
        let after = serde_json::to_value(camera.native_camera.capture_continuation()).unwrap();
        for key in ["offset", "ground", "enabled", "limited"] {
            assert_eq!(after[key], before[key]);
        }
        assert_eq!(peer.capture_camera_continuation().unwrap(), peer_saved);
        let saved = world.capture_camera_continuation().unwrap();
        world
            .admit_player_camera_wheel(0, &mut ScriptPresentationEffects::default())
            .unwrap();
        assert_eq!(
            world.capture_camera_continuation().unwrap(),
            saved,
            "zero spin has no setter"
        );
    }
}
#[test]
fn delivered_equal_x_rotation_is_a_setter_but_opposite_held_keys_are_not() {
    let mut world = owner();
    let peer = owner();
    let peer_saved = peer.capture_camera_continuation().unwrap();
    world.session_view.camera.camera_yaw_target = Some(0.4);
    world.session_view.camera.camera_yaw_duration = 1.;
    world.session_view.camera.camera_zoom_target = Some(1.2);
    let pending = world.capture_camera_continuation().unwrap();
    world
        .admit_player_camera_held_keys(
            true,
            true,
            true,
            true,
            0.1,
            1.,
            &mut ScriptPresentationEffects::default(),
        )
        .unwrap();
    assert_eq!(
        world.capture_camera_continuation().unwrap(),
        pending,
        "opposite keys never enter setters or cancel motion"
    );
    let yaw = world.session_view.camera.camera_yaw_radians;
    let hag = serde_json::to_vec(
        &world
            .session_view
            .camera
            .native_camera
            .capture_continuation(),
    )
    .unwrap();
    world
        .admit_player_camera_rotation(0., &mut ScriptPresentationEffects::default())
        .unwrap();
    assert_eq!(world.session_view.camera.camera_yaw_radians, yaw);
    assert!(world.session_view.camera.camera_yaw_target.is_none());
    assert!(world.session_view.camera.camera_zoom_target.is_none());
    assert_eq!(
        world.session_view.camera.script_camera_shakers[0].elapsed_seconds,
        1. / 30.
    );
    assert_eq!(
        serde_json::to_vec(
            &world
                .session_view
                .camera
                .native_camera
                .capture_continuation()
        )
        .unwrap(),
        hag
    );
    assert_eq!(peer.capture_camera_continuation().unwrap(), peer_saved);
}
#[test]
fn delivered_cursor_boundaries_do_not_reintroduce_idle_rotation_polling() {
    let input = include_str!("../../cnc_game_engine/input.rs");
    for name in [
        "fn apply_cursor_position(",
        "fn inject_winit_equivalent_cursor_at(",
    ] {
        let body = input
            .split(name)
            .nth(1)
            .unwrap()
            .split(
                "
    }",
            )
            .next()
            .unwrap();
        assert!(body.contains("self.admit_delivered_camera_cursor()"));
    }
    let mouse = include_str!("../../cnc_game_engine/mouse/camera.rs");
    let refresh = mouse
        .split("fn update_camera(")
        .nth(1)
        .unwrap()
        .split("fn is_character_key_pressed")
        .next()
        .unwrap();
    assert!(!refresh.contains("self.is_mmb_rotating"));
    let begin = mouse
        .split("fn begin_mmb_lookat_rotate(")
        .nth(1)
        .unwrap()
        .split("fn receiving_camera_client_frame")
        .next()
        .unwrap();
    assert!(begin.contains("self.receiving_camera_client_frame()"));
    let end = mouse
        .split("fn end_mmb_lookat_rotate(")
        .nth(1)
        .unwrap()
        .split("fn reset_camera_pose_in_place")
        .next()
        .unwrap();
    assert!(end.contains("wrapping_sub(press_frame)"));
}

#[test]
fn actual_receiving_setter_counts_replay_extracted_cpp_input_branches() {
    for row in include_str!("../../cnc_game_engine/mouse/fixtures/input_admission_cpp.csv").lines()
    {
        let fields: Vec<_> = row.split(',').collect();
        if fields[0] == "click" {
            continue;
        }
        let values: Vec<i32> = fields[1..].iter().map(|x| x.parse().unwrap()).collect();
        let mut world = owner();
        let mut effects = ScriptPresentationEffects::default();
        let expected_builds = match fields[0] {
            "wheel" => {
                world
                    .admit_player_camera_wheel(values[0], &mut effects)
                    .unwrap();
                2 * values[4]
            }
            "cursor" => {
                world
                    .admit_player_camera_rotation(values[0] as f32 * 0.01, &mut effects)
                    .unwrap();
                values[4]
            }
            "keys" => {
                world
                    .admit_player_camera_held_keys(
                        values[0] != 0,
                        values[1] != 0,
                        values[2] != 0,
                        values[3] != 0,
                        0.1,
                        1.,
                        &mut effects,
                    )
                    .unwrap();
                let rotations = i32::from((values[0] != 0) != (values[1] != 0));
                let heights = i32::from((values[2] != 0) != (values[3] != 0));
                assert_eq!(rotations + heights, values[4], "{row}");
                rotations + 2 * heights
            }
            other => panic!("unexpected oracle category {other}"),
        };
        let expected_elapsed = (0..expected_builds).fold(0., |elapsed, _| elapsed + 1. / 30.);
        assert_eq!(
            world.session_view.camera.script_camera_shakers[0].elapsed_seconds, expected_elapsed,
            "{row}"
        );
    }
}
