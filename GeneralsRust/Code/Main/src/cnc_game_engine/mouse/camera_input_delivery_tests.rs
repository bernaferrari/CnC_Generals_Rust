//! CPU execution of the exact Engine-delivered operations over real receiving
//! Client frame loans. OS dispatch, UI injection and GPU construction are not run.
use super::*;
use game_client::core::game_client::GameClient;
fn owner() -> GameLogic {
    let mut world = GameLogic::new();
    let mut map = gamelogic::system::map_loader::MapData::new();
    map.width = 128;
    map.height = 128;
    map.heightmap = vec![0; 128 * 128];
    map.boundaries = vec![gamelogic::common::ICoord2D::new(128, 128)];
    world
        .world_services
        .terrain()
        .write()
        .unwrap()
        .load_map_data(map);
    world.session_view.target = Vec3::new(640., 0., 640.);
    world
        .session_view
        .camera
        .native_camera
        .initialize_map(0., 300., 37.5, 0.);
    world
        .session_view
        .camera
        .native_camera
        .set_height(200., 100., 300.);
    world
        .commit_native_camera_with_effects(&mut ScriptPresentationEffects::default())
        .unwrap();
    world.session_view.camera.script_camera_shakers.push(
        crate::presentation_frame::camera::ScriptCameraShaker::new(Vec3::ZERO, 10000., 30., 0.),
    );
    world
}
fn loan<'a>(
    client: &'a mut GameClient,
    modes: &'a mut LookAtHostModes,
    rotating: &'a mut bool,
    anchor: &'a mut Option<(f32, f32)>,
) -> CameraInputLoan<'a> {
    let (frame, _snow, _presence) = client.split_frame_snow_and_drawable_presence();
    CameraInputLoan::new(modes, rotating, anchor, frame)
}
#[test]
fn delivered_equal_x_cursor_is_a_setter_while_absent_event_and_idle_keys_are_not() {
    let mut client = GameClient::new().unwrap();
    let mut peer_client = GameClient::new().unwrap();
    client.set_frame(37);
    peer_client.set_frame(900);
    let mut world = owner();
    world.set_current_frame(777);
    let peer = owner();
    let peer_before = peer.capture_camera_continuation().unwrap();
    let (mut modes, mut rotating, mut anchor) = (LookAtHostModes::default(), false, None);
    loan(&mut client, &mut modes, &mut rotating, &mut anchor).begin_mmb((100., 100.), &world);
    assert_eq!(modes.mmb_press_frame, 37);
    assert_eq!(modes.last_mouse_move_frame, 777);
    assert!(rotating);
    world.session_view.camera.rotation_registers.begin_scalar(
        world
            .session_view
            .camera
            .rotation_registers
            .scalar_start()
            .unwrap_or(0.),
        0.5,
    );
    world.session_view.camera.rotation =
        crate::presentation_frame::camera::rotation::RotationTiming::historical(
            1.,
            world.session_view.camera.rotation.ease_in(),
            world.session_view.camera.rotation.ease_out(),
            world.session_view.camera.rotation.time,
        );
    let before = world.capture_camera_continuation().unwrap();
    let modes_before = serde_json::to_value(modes.capture_continuation()).unwrap();
    {
        let mut input = loan(&mut client, &mut modes, &mut rotating, &mut anchor);
        assert!(
            !input
                .delivered_cursor(
                    None,
                    true,
                    true,
                    &mut world,
                    &mut ScriptPresentationEffects::default()
                )
                .unwrap()
        );
        input
            .held_keys(
                false,
                false,
                false,
                false,
                0.1,
                1.,
                &mut world,
                &mut ScriptPresentationEffects::default(),
            )
            .unwrap();
    }
    assert_eq!(world.capture_camera_continuation().unwrap(), before);
    assert_eq!(
        serde_json::to_value(modes.capture_continuation()).unwrap(),
        modes_before
    );
    assert_eq!(anchor, Some((100., 100.)));
    let yaw = world.session_view.camera.camera_yaw_radians;
    assert!(
        loan(&mut client, &mut modes, &mut rotating, &mut anchor)
            .delivered_cursor(
                Some((100., 101.)),
                true,
                true,
                &mut world,
                &mut ScriptPresentationEffects::default()
            )
            .unwrap()
    );
    assert_eq!(world.session_view.camera.camera_yaw_radians, yaw);
    assert!(!world.session_view.camera.rotation_registers.active());
    assert_eq!(anchor, Some((100., 101.)));
    assert_eq!(
        world.session_view.camera.script_camera_shakers[0].elapsed_seconds,
        1. / 30.
    );
    assert_eq!(peer_before, peer.capture_camera_continuation().unwrap());
    assert_eq!(client.get_frame(), 37);
    assert_eq!(peer_client.get_frame(), 900);
}
#[test]
fn middle_release_reads_receiving_client_wrap_and_rejects_foreign_peer_and_engine_frames() {
    for (start, end, short) in [(u32::MAX - 1, 1, true), (37, 42, false)] {
        let mut client = GameClient::new().unwrap();
        let mut peer_client = GameClient::new().unwrap();
        client.set_frame(start);
        peer_client.set_frame(901);
        let mut world = owner();
        world.set_current_frame(777);
        let peer = owner();
        let peer_before = peer.capture_camera_continuation().unwrap();
        let (mut modes, mut rotating, mut anchor) = (LookAtHostModes::default(), false, None);
        loan(&mut client, &mut modes, &mut rotating, &mut anchor).begin_mmb((100., 100.), &world);
        assert_eq!(modes.mmb_press_frame, start);
        let before = world.capture_camera_continuation().unwrap();
        client.set_frame(end);
        world.set_current_frame(888);
        peer_client.set_frame(if short { 1000 } else { 38 });
        let admitted = loan(&mut client, &mut modes, &mut rotating, &mut anchor)
            .end_mmb(
                (100., 101.),
                &mut world,
                &mut ScriptPresentationEffects::default(),
            )
            .unwrap();
        assert_eq!(admitted, short);
        assert_eq!(modes.last_mouse_move_frame, 888);
        assert!(!rotating);
        assert_eq!(anchor, None);
        assert_eq!(modes.mmb_original_anchor, None);
        if short {
            assert_eq!(world.session_view.camera.native_camera.desired(), 300.);
            assert_eq!(
                world.session_view.camera.script_camera_shakers[0].elapsed_seconds,
                3. * (1. / 30.)
            );
        } else {
            assert_eq!(world.capture_camera_continuation().unwrap(), before);
        }
        assert_eq!(peer_before, peer.capture_camera_continuation().unwrap());
        assert_eq!(client.get_frame(), end);
    }
}
#[test]
fn actual_winit_wheel_delta_per_detent_and_opposite_keys_share_engine_loan_operations() {
    for (delta, expected_goal) in [(2.9, 180.), (-2.9, 220.)] {
        let mut client = GameClient::new().unwrap();
        client.set_frame(37);
        let mut peer_client = GameClient::new().unwrap();
        peer_client.set_frame(900);
        let mut world = owner();
        world.set_current_frame(777);
        let peer = owner();
        let peer_before = peer.capture_camera_continuation().unwrap();
        let (mut modes, mut rotating, mut anchor) = (LookAtHostModes::default(), false, None);
        let (admitted, result) = loan(&mut client, &mut modes, &mut rotating, &mut anchor)
            .delivered_wheel(
                &winit::event::MouseScrollDelta::LineDelta(0., delta),
                &mut world,
                &mut ScriptPresentationEffects::default(),
            );
        assert!(admitted);
        result.unwrap();
        assert_eq!(
            world.session_view.camera.native_camera.desired(),
            expected_goal
        );
        assert_eq!(
            world.session_view.camera.script_camera_shakers[0].elapsed_seconds,
            4. * (1. / 30.)
        );
        assert_eq!(
            modes.take_scripted_camera_player_cancel(),
            ScriptedCameraPlayerCancel::Set
        );
        world.session_view.camera.rotation_registers.begin_scalar(
            world
                .session_view
                .camera
                .rotation_registers
                .scalar_start()
                .unwrap_or(0.),
            0.5,
        );
        world.session_view.camera.rotation =
            crate::presentation_frame::camera::rotation::RotationTiming::historical(
                1.,
                world.session_view.camera.rotation.ease_in(),
                world.session_view.camera.rotation.ease_out(),
                world.session_view.camera.rotation.time,
            );
        let before = world.capture_camera_continuation().unwrap();
        let mode_before = serde_json::to_value(modes.capture_continuation()).unwrap();
        {
            let mut input = loan(&mut client, &mut modes, &mut rotating, &mut anchor);
            input
                .held_keys(
                    true,
                    true,
                    true,
                    true,
                    0.1,
                    1.,
                    &mut world,
                    &mut ScriptPresentationEffects::default(),
                )
                .unwrap();
            let (admitted, result) = input.delivered_wheel(
                &winit::event::MouseScrollDelta::LineDelta(5., 0.),
                &mut world,
                &mut ScriptPresentationEffects::default(),
            );
            assert!(!admitted);
            result.unwrap();
        }
        assert_eq!(world.capture_camera_continuation().unwrap(), before);
        assert_eq!(
            serde_json::to_value(modes.capture_continuation()).unwrap(),
            mode_before
        );
        assert_eq!(peer_before, peer.capture_camera_continuation().unwrap());
        assert_eq!(client.get_frame(), 37);
        assert_eq!(peer_client.get_frame(), 900);
    }
}
#[test]
fn delivered_cursor_gates_leave_actual_owner_and_anchor_unchanged() {
    let mut client = GameClient::new().unwrap();
    let mut world = owner();
    world.set_current_frame(777);
    let (mut modes, mut rotating, mut anchor) = (LookAtHostModes::default(), false, None);
    loan(&mut client, &mut modes, &mut rotating, &mut anchor).begin_mmb((100., 100.), &world);
    let before = world.capture_camera_continuation().unwrap();
    for (in_match, enabled) in [(false, true), (true, false)] {
        assert!(
            !loan(&mut client, &mut modes, &mut rotating, &mut anchor)
                .delivered_cursor(
                    Some((120., 120.)),
                    in_match,
                    enabled,
                    &mut world,
                    &mut ScriptPresentationEffects::default()
                )
                .unwrap()
        );
        assert_eq!(world.capture_camera_continuation().unwrap(), before);
        assert_eq!(anchor, Some((100., 100.)));
    }
}

#[test]
fn mouse_activity_uses_paused_logic_while_middle_click_duration_uses_client_frames() {
    // CPP LookAtXlat119–124/226/239: activity is measured in Logic frames,
    // while the separate press timestamp and click duration use Client frames.
    let mut client = GameClient::new().unwrap();
    let mut peer_client = GameClient::new().unwrap();
    client.set_frame(37);
    peer_client.set_frame(900);
    let mut world = owner();
    world.set_current_frame(713);
    let mut peer = owner();
    peer.set_current_frame(50);
    let peer_before = peer.capture_camera_continuation().unwrap();
    let (mut modes, mut rotating, mut anchor) = (LookAtHostModes::default(), false, None);
    loan(&mut client, &mut modes, &mut rotating, &mut anchor).begin_mmb((100., 100.), &world);
    assert_eq!(modes.last_mouse_move_frame, 713);
    assert_eq!(modes.mmb_press_frame, 37);
    let before = world.capture_camera_continuation().unwrap();

    // A paused Logic owner can remain at 713 as Client presentation advances.
    client.set_frame(42);
    peer_client.set_frame(38);
    assert!(modes.lookat_has_mouse_moved_recently(world.get_current_frame() as u32));
    assert!(
        !loan(&mut client, &mut modes, &mut rotating, &mut anchor)
            .end_mmb(
                (100., 101.),
                &mut world,
                &mut ScriptPresentationEffects::default(),
            )
            .unwrap(),
        "five receiving Client frames are not a short click despite paused Logic"
    );
    assert_eq!(modes.last_mouse_move_frame, 713);
    assert_eq!(world.capture_camera_continuation().unwrap(), before);
    assert_eq!(world.get_current_frame(), 713);
    assert_eq!(peer.get_current_frame(), 50);
    assert_eq!(peer_before, peer.capture_camera_continuation().unwrap());

    // The recent-motion interval is the original inclusive 30 Logic frames.
    world.set_current_frame(743);
    assert!(modes.lookat_has_mouse_moved_recently(world.get_current_frame() as u32));
    world.set_current_frame(744);
    assert!(!modes.lookat_has_mouse_moved_recently(world.get_current_frame() as u32));

    // Outer Engine calls must use that same owner domain for cursor/scroll.
}
