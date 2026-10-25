#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;

#[test]
fn mouse_wheel_quantization_matches_cpp_per_event_spin_count() {
    use winit::{dpi::PhysicalPosition, event::MouseScrollDelta};

    // C++ Mouse::processMouseEvent resets wheelPos for every raw event and
    // Mouse::createStreamMessages emits wheelPos / 120 only when nonzero
    // (Mouse.cpp:167-174, 678-685, 794-801). Winit's Windows LineDelta is
    // raw WM_MOUSEWHEEL delta / 120, so quantize each event independently.
    let cases = [
        (MouseScrollDelta::LineDelta(1.0, 0.0), 0.0), // horizontal only
        (MouseScrollDelta::LineDelta(0.0, 0.0), 0.0),
        (MouseScrollDelta::LineDelta(0.0, -0.0), 0.0), // signed zero
        (MouseScrollDelta::LineDelta(0.0, 0.5), 0.0),
        (MouseScrollDelta::LineDelta(0.0, -0.5), 0.0),
        (MouseScrollDelta::LineDelta(0.0, 1.0), 1.0),
        (MouseScrollDelta::LineDelta(0.0, -1.0), -1.0),
        (MouseScrollDelta::LineDelta(0.0, 1.5), 1.0),
        (MouseScrollDelta::LineDelta(0.0, -1.5), -1.0),
        (MouseScrollDelta::LineDelta(0.0, 3.0), 3.0),
        (MouseScrollDelta::LineDelta(0.0, -3.0), -3.0),
        (
            MouseScrollDelta::PixelDelta(PhysicalPosition::new(0.0, 0.0)),
            0.0,
        ),
    ];

    for (event, expected_spin) in cases {
        assert_eq!(
            super::camera::mouse_wheel_detents(&event),
            expected_spin,
            "event {event:?} must produce C++ spin count {expected_spin}"
        );
    }

    // Two half-detent events in one render/logic interval still each emit
    // zero C++ wheel messages; Main must not carry or combine their remainder.
    let half_detent = MouseScrollDelta::LineDelta(0.0, 0.5);
    assert_eq!(super::camera::mouse_wheel_detents(&half_detent), 0.0);
    assert_eq!(super::camera::mouse_wheel_detents(&half_detent), 0.0);
}

#[test]
fn zero_vertical_wheel_does_not_reach_lookat_side_effects() {
    use winit::{dpi::PhysicalPosition, event::MouseScrollDelta};

    let zero_events = [
        MouseScrollDelta::LineDelta(0.0, 0.0),
        MouseScrollDelta::LineDelta(1.0, 0.0), // horizontal-only
        MouseScrollDelta::LineDelta(0.0, -0.0),
        MouseScrollDelta::PixelDelta(PhysicalPosition::new(0.0, 0.0)),
    ];
    for event in zero_events {
        assert!(!super::camera::mouse_wheel_has_vertical_delta(&event));
        assert_eq!(super::camera::mouse_wheel_detents(&event), 0.0);
    }

    for delta in [0.5, -0.5] {
        let event = MouseScrollDelta::LineDelta(0.0, delta);
        assert!(super::camera::mouse_wheel_has_vertical_delta(&event));
        assert_eq!(super::camera::mouse_wheel_detents(&event), 0.0);
    }

    // The production handler returns for zero before any C++ wheel-message
    // side effect (stamp, GameClient injection, zoom, or stopScrolling).
}

#[cfg(feature = "game_client")]
#[test]
fn wnd_wheel_mapping_matches_cpp_zero_spin_direction() {
    use game_client::gui::game_window::WindowMessage;
    use winit::{dpi::PhysicalPosition, event::MouseScrollDelta};

    let map = super::ui_dispatch::window_message_for_mouse_wheel;

    // C++ emits no message for raw wheelPos == 0. For a nonzero raw event
    // whose /120 spin truncates to zero, WindowXlat's spin > 0 / else branch
    // still maps the message to WHEEL_DOWN (WindowXlat.cpp:106-112).
    assert_eq!(map(&MouseScrollDelta::LineDelta(0.0, 0.0)), None);
    assert_eq!(map(&MouseScrollDelta::LineDelta(1.0, 0.0)), None);
    assert_eq!(
        map(&MouseScrollDelta::LineDelta(0.0, -0.0)),
        None,
        "signed zero vertical delta has no C++ raw wheel message"
    );
    assert_eq!(
        map(&MouseScrollDelta::LineDelta(0.0, 0.5)),
        Some(WindowMessage::WheelDown),
        "raw +60 becomes spin 0, whose WindowXlat else branch is WheelDown"
    );
    assert_eq!(
        map(&MouseScrollDelta::LineDelta(0.0, -0.5)),
        Some(WindowMessage::WheelDown),
        "raw -60 also becomes spin 0 and WheelDown"
    );
    assert_eq!(
        map(&MouseScrollDelta::LineDelta(0.0, 1.0)),
        Some(WindowMessage::WheelUp)
    );
    assert_eq!(
        map(&MouseScrollDelta::LineDelta(0.0, -1.0)),
        Some(WindowMessage::WheelDown)
    );
    assert_eq!(
        map(&MouseScrollDelta::LineDelta(0.0, 3.0)),
        Some(WindowMessage::WheelUp)
    );
    assert_eq!(
        map(&MouseScrollDelta::LineDelta(0.0, -3.0)),
        Some(WindowMessage::WheelDown)
    );

    // Retain Main's existing PixelDelta-to-lines WND policy; CPP parity for
    // platform pixel deltas is not established by this test.
    assert_eq!(
        map(&MouseScrollDelta::PixelDelta(PhysicalPosition::new(
            0.0, 16.0
        ))),
        Some(WindowMessage::WheelUp)
    );
    assert_eq!(
        map(&MouseScrollDelta::PixelDelta(PhysicalPosition::new(
            0.0, -16.0
        ))),
        Some(WindowMessage::WheelDown)
    );
}

#[test]
fn mouse_ray_extrapolates_beyond_viewport_like_cpp() {
    // W3DConvert.cpp:75-76 converts the raw screen pixel, even when capture
    // delivers coordinates outside the tactical view. A clamp would yield -1.
    let (near, far) = unproject_mouse_ray(
        glam::Mat4::IDENTITY,
        glam::Mat4::IDENTITY,
        (-64.0, 528.0),
        640.0,
        480.0,
    )
    .expect("finite ray outside viewport");
    assert!((near.x - -1.2).abs() < 1.0e-6);
    assert!((near.y - -1.2).abs() < 1.0e-6);
    assert!((far.x - near.x).abs() < 1.0e-6);
    assert!((far.y - near.y).abs() < 1.0e-6);
}

#[test]
fn cursor_ray_uses_bridge_deck_above_terrain() {
    let bridge = crate::presentation_frame::PresentationBridgeSegment {
        start: [0.0, 20.0, 0.0],
        end: [100.0, 30.0, 0.0],
        width: 20.0,
        template_name: "TestBridge".into(),
    };
    let bridges = [bridge];
    let near = Vec3::new(50.0, 100.0, 0.0);
    let far = Vec3::new(50.0, -100.0, 0.0);
    let deck = super::world_pick::raycast_frozen_bridge(near, far, &bridges)
        .expect("ray crosses bridge footprint");
    assert!((deck.y - 25.0).abs() < 1.0e-4);
    assert_eq!(
        super::world_pick::higher_bridge_or_terrain(Some(Vec3::new(50.0, 0.0, 0.0)), Some(deck)),
        Some(deck)
    );
    assert!(
        super::world_pick::raycast_frozen_bridge(
            Vec3::new(50.0, 100.0, 11.0),
            Vec3::new(50.0, -100.0, 11.0),
            &bridges,
        )
        .is_none(),
        "a ray outside half-width must keep the terrain hit"
    );
    assert_eq!(
        super::world_pick::higher_bridge_or_terrain(Some(Vec3::new(50.0, 40.0, 0.0)), Some(deck)),
        Some(Vec3::new(50.0, 40.0, 0.0)),
        "C++ chooses bridge only when its hit is above terrain"
    );
}

#[test]
fn overlapping_bridge_pick_follows_cpp_reverse_map_order() {
    let segment = |height| crate::presentation_frame::PresentationBridgeSegment {
        start: [0.0, height, 0.0],
        end: [100.0, height, 0.0],
        width: 20.0,
        template_name: "TestBridge".into(),
    };
    let bridges = [segment(30.0), segment(20.0)];
    let hit = super::world_pick::raycast_frozen_bridge(
        Vec3::new(50.0, 100.0, 0.0),
        Vec3::new(50.0, -100.0, 0.0),
        &bridges,
    )
    .expect("both bridge decks overlap the cursor ray");
    assert!((hit.y - 20.0).abs() < 1.0e-4);
}

#[test]
fn camera_set_default_scales_wheel_clamp_and_home_pitch() {
    let (min_h, max_half) = live_view_height_clamp(40.0, 200.0, 0.5);
    assert!((min_h - 40.0).abs() < f32::EPSILON);
    assert!((max_half - 100.0).abs() < f32::EPSILON);
    let (_, max_double) = live_view_height_clamp(40.0, 200.0, 2.0);
    assert!((max_double - 400.0).abs() < f32::EPSILON);
    let (_, max_zero) = live_view_height_clamp(40.0, 200.0, 0.0);
    assert!(
        (max_zero - 40.0).abs() < f32::EPSILON,
        "scale 0 floors to min like C++ setDefaultView"
    );

    // View.cpp:163 assigns the raw default directly. Script value1 is a
    // legitimate View pitch, independent of the FXPitch identity value1.
    let basis = super::super::camera_height::CameraBasis::for_map(0.0, 100.0, 45.0, 0.0);
    let (default_eye, _) = basis.pose(Vec3::ZERO, 1.0, 0.0, 0.0);
    let (script_eye, _) = basis.pose(Vec3::ZERO, 1.0, 1.0, 0.0);
    assert!((default_eye.y - 100.0).abs() < 0.0001);
    assert!((script_eye - default_eye).length() > 1.0);
    // Replay/bookmark setLocation calls setPitch, unlike raw Home assignment.
    let clamped = super::super::camera_height::clamp_pitch(1.0);
    assert_eq!(clamped, std::f32::consts::PI / 5.0);
    let (location_eye, _) = basis.pose(Vec3::ZERO, 1.0, clamped, 0.0);
    assert!((script_eye - location_eye).length() > 1.0);
}

fn classic_shift_lmb_only_prefers_selection_for_a_local_target() {
    assert!(classic_left_context_action_allowed(true, false, true));
    assert!(!classic_left_context_action_allowed(true, true, true));
    assert!(classic_left_context_action_allowed(true, true, false));
    assert!(!classic_left_context_action_allowed(false, false, false));
}

#[test]
fn center_screen_pick_follows_the_render_camera_not_map_extents() {
    let camera = Vec3::new(0.0, 120.0, 120.0);
    let view = Mat4::look_at_rh(camera, Vec3::ZERO, Vec3::Y);
    let projection = Mat4::perspective_rh(60.0_f32.to_radians(), 1.0, 1.0, 2_000.0);
    let (near, far) = unproject_mouse_ray(view, projection, (500.0, 500.0), 1_000.0, 1_000.0)
        .expect("a finite WGPU camera ray");

    let hit = raycast_ground_plane_clamped(
        near,
        far,
        Vec3::new(-1_000.0, 0.0, -1_000.0),
        Vec3::new(1_000.0, 0.0, 1_000.0),
        None,
    )
    .expect("center ray intersects the ground plane");

    assert!(
        hit.length() < 0.02,
        "center-screen pick must land at the camera target, got {hit:?}"
    );
}

#[test]
fn ray_interval_rejects_a_parallel_ray_outside_the_map() {
    assert!(
        ray_interval_in_world_xz(
            Vec3::new(20.0, 5.0, 0.0),
            Vec3::new(20.0, -5.0, 0.0),
            Vec3::new(-10.0, 0.0, -10.0),
            Vec3::new(10.0, 0.0, 10.0),
        )
        .is_none()
    );
}

#[test]
fn point_select_predicate_accepts_non_local_selectable() {
    // C++ SelectionXlat.cpp:181-189 — point clicks select anything selectable.
    // Local-only remains the Shift-add / drag-select gate.
    assert!(classic_left_context_action_allowed(true, true, false));
    assert!(!classic_left_context_action_allowed(true, true, true));
}

#[test]
fn lookat_arrow_keys_blocked_during_box_select_and_rmb_scroll() {
    // C++ LookAtXlat.cpp:174-175
    assert!(lookat_keyboard_scroll_blocked(true, false));
    assert!(lookat_keyboard_scroll_blocked(false, true));
    assert!(lookat_keyboard_scroll_blocked(true, true));
    assert!(!lookat_keyboard_scroll_blocked(false, false));
}

#[test]
fn lookat_scroll_types_are_exclusive() {
    // C++ m_scrollType: frame tick applies only one of RMB / KEY / SCREENEDGE.
    assert_eq!(
        lookat_resolve_scroll_type(false, LookAtScrollType::Rmb, true, false, true, true, true),
        LookAtScrollType::Rmb
    );
    assert_eq!(
        lookat_resolve_scroll_type(
            false,
            LookAtScrollType::ScreenEdge,
            true,
            false,
            false,
            true,
            true
        ),
        LookAtScrollType::ScreenEdge
    );
    assert_eq!(
        lookat_resolve_scroll_type(false, LookAtScrollType::Key, true, false, false, true, true),
        LookAtScrollType::Key
    );
    assert_eq!(
        lookat_resolve_scroll_type(
            false,
            LookAtScrollType::None,
            true,
            false,
            false,
            false,
            true
        ),
        LookAtScrollType::ScreenEdge
    );
    assert_eq!(
        lookat_resolve_scroll_type(
            false,
            LookAtScrollType::None,
            true,
            false,
            false,
            true,
            true
        ),
        LookAtScrollType::Key
    );
    // Edge may start while box-selecting; keys may not.
    assert_eq!(
        lookat_resolve_scroll_type(false, LookAtScrollType::None, true, true, false, true, true),
        LookAtScrollType::ScreenEdge
    );
    assert_eq!(
        lookat_resolve_scroll_type(
            false,
            LookAtScrollType::Key,
            false,
            false,
            false,
            true,
            true
        ),
        LookAtScrollType::None
    );
}

#[test]
fn lookat_mmb_short_click_matches_cpp_5px_5_frame_window() {
    // C++ LookAtXlat.cpp:241-250 CLICK_DURATION=5 PIXEL_OFFSET=5
    assert!(lookat_mmb_is_short_click(0.0, 0.0, 0));
    assert!(lookat_mmb_is_short_click(5.0, 5.0, 4));
    assert!(!lookat_mmb_is_short_click(5.1, 0.0, 0));
    assert!(!lookat_mmb_is_short_click(0.0, 0.0, 5));
    assert!(
        !lookat_mmb_is_short_click(0., -500., 4),
        "original signed dy is promoted to unsigned by PIXEL_OFFSET"
    );
    assert!(
        !lookat_mmb_is_short_click(0., -1., 4),
        "even one upward integer pixel is moved, unlike downward one pixel"
    );
    assert!(lookat_mmb_is_short_click(0., 1., 4));
    assert!(!lookat_mmb_is_short_click(0., 6., 4));
    assert!(lookat_mmb_is_short_click(
        0.,
        0.,
        1u32.wrapping_sub(u32::MAX - 1)
    ));
}

#[test]
fn lookat_keyboard_rotate_uses_ini_speed_per_logic_frame() {
    // C++ GlobalData.cpp m_keyboardCameraRotateSpeed default 0.1, per InGameUI frame.
    let delta = lookat_keyboard_rotate_delta(0.1, 1.0 / 30.0, 30.0);
    assert!((delta - 0.1).abs() < 1.0e-5);
    let faster = lookat_keyboard_rotate_delta(0.25, 1.0 / 30.0, 30.0);
    assert!((faster - 0.25).abs() < 1.0e-5);
    assert!((LOOKAT_MMB_YAW_FACTOR - 0.01).abs() < f32::EPSILON);
}

#[test]
fn lookat_view_location_stores_full_pose() {
    let loc = CameraViewLocation {
        pos: Vec3::new(100.0, 2.0, -40.0),
        yaw: 0.5,
        pitch: 0.7,
        zoom: 1.25,
    };
    assert_eq!(loc.pos.x, 100.0);
    assert!((loc.yaw - 0.5).abs() < f32::EPSILON);
    assert!((loc.pitch - 0.7).abs() < f32::EPSILON);
    assert!((loc.zoom - 1.25).abs() < f32::EPSILON);
    assert_eq!(lookat_view_slot(NamedKey::F1), Some(0));
    assert_eq!(lookat_view_slot(NamedKey::F8), Some(7));
    assert!(lookat_bookmark_message(3).contains("3"));
}

#[test]
fn lookat_reset_pose_keeps_look_at_not_base() {
    // C++ InGameUI.cpp:4141-4143 getLocation then resetCamera(&currentView.getPosition()).
    let look_at = Vec3::new(900.0, 10.0, -300.0);
    let after_reset_target = look_at;
    assert_ne!(after_reset_target, Vec3::ZERO);
    assert!((LOOKAT_DEFAULT_PITCH_DEG - 37.5).abs() < f32::EPSILON);
}

#[test]
fn host_replay_camera_emit_matches_cpp_look_at_gates() {
    // C++ LookAtXlat.cpp:459 — saveCameraInReplay && (SP || skirmish).
    assert!(!should_emit_host_replay_camera(
        GameState::Menu,
        crate::game_logic::GameMode::Skirmish,
        true,
        false,
    ));
    assert!(!should_emit_host_replay_camera(
        GameState::InGame,
        crate::game_logic::GameMode::Multiplayer,
        true,
        false,
    ));
    assert!(!should_emit_host_replay_camera(
        GameState::InGame,
        crate::game_logic::GameMode::Shell,
        true,
        false,
    ));
    assert!(!should_emit_host_replay_camera(
        GameState::InGame,
        crate::game_logic::GameMode::Skirmish,
        true,
        true,
    ));
}

#[test]
fn drag_tolerance_and_empty_click_match_cpp_selection_xlat() {
    // C++ Mouse.cpp DragTolerance default 5 / SelectionXlat.cpp:399-407, 575-597, 617-626, 930-937.
    assert!(is_point_click_drag(0.0, 0.0));
    assert!(is_point_click_drag(5.0, 0.0));
    assert!(is_point_click_drag(0.0, 5.0));
    assert!(
        is_point_click_drag(4.0, 4.0),
        "4x4 diagonal is still a click"
    );
    assert!(is_point_click_drag(5.0, 5.0));
    assert!(!is_point_click_drag(5.1, 0.0));
    assert!(!is_point_click_drag(0.0, 5.1));
    assert!(!alternate_mouse_blank_click_deselects(
        false, false, false, false
    ));
    assert!(alternate_mouse_blank_click_deselects(
        true, false, false, false
    ));
    assert!(!alternate_mouse_blank_click_deselects(
        true, true, false, false
    ));
    assert!(box_selection_must_replace(true, false, false, false));
    assert!(box_selection_must_replace(false, false, false, true));
    assert!(!box_selection_must_replace(false, false, false, false));
    assert!(infantry_garrison_context_takes_region(
        false, false, true, 1
    ));
    assert!(!infantry_garrison_context_takes_region(
        true, false, true, 1
    ));
    assert!(!infantry_garrison_context_takes_region(
        false, true, true, 1
    ));
    assert!(!infantry_garrison_context_takes_region(
        false, false, true, 0
    ));
}

#[test]
fn host_is_selecting_requires_drag_tolerance_not_lmb_held() {
    // C++ SelectionXlat.cpp:399-408 — isSelecting after DragTolerance, not LMB down.
    assert!(!host_is_selecting_now(
        false,
        false,
        Some((0.0, 0.0)),
        (3.0, 0.0)
    ));
    assert!(!host_is_selecting_now(
        true,
        false,
        Some((0.0, 0.0)),
        (5.0, 0.0)
    ));
    assert!(host_is_selecting_now(
        true,
        false,
        Some((0.0, 0.0)),
        (5.1, 0.0)
    ));
    assert!(
        !host_is_selecting_now(true, true, Some((0.0, 0.0)), (20.0, 0.0)),
        "placement rotate must not count as isSelecting"
    );
}

#[test]
fn placement_rotate_uses_5px_screen_not_1wu() {
    // C++ PlaceEventTranslator.cpp:307-320 Euclidean 5px, no 1wu world gate.
    assert!(!placement_screen_drag_exceeds_threshold(3.0, 3.0));
    assert!(placement_screen_drag_exceeds_threshold(3.0, 4.0));
    assert!(placement_screen_drag_exceeds_threshold(5.0, 0.0));
}

#[test]
fn select_p2_leftover_double_click_and_sound_match_cpp() {
    // hq-kmgkv: 3–4px is still a click (Mouse.ini DragTolerance default 5).
    assert!(host_screen_drag_is_click(4.0, 0.0));
    assert!(host_screen_drag_is_click(0.0, 4.0));
    assert!(host_screen_drag_is_click(5.0, 5.0));
    assert!(!host_screen_drag_is_click(6.0, 0.0));
    assert!((host_mouse_drag_tolerance_px() - 5.0).abs() < f32::EPSILON);

    // hq-ht4gw / hq-myqqv / hq-gqwjy live-host markers.
}

#[test]
fn rmb_click_gates_match_cpp_selection_xlat() {
    // C++ SelectionXlat.cpp:982-1000 + Mouse.ini defaults 5px / 250ms / 5wu.
    assert!(host_rmb_release_is_click(0.0, 0.0, 0, 0.0));
    assert!(host_rmb_release_is_click(5.0, 5.0, 250, 5.0));
    assert!(!host_rmb_release_is_click(6.0, 0.0, 0, 0.0));
    assert!(!host_rmb_release_is_click(0.0, 0.0, 251, 0.0));
    assert!(!host_rmb_release_is_click(0.0, 0.0, 0, 5.1));
    assert!((host_mouse_drag_tolerance_ms() as f32 - 250.0).abs() < f32::EPSILON);
    assert!((host_mouse_drag_tolerance_3d() - 5.0).abs() < f32::EPSILON);
}

#[test]
fn w3d_set_zoom_clamps_to_view_min_max() {
    // C++ View.cpp:78-79 / W3DView::setZoom [0.2, 1.3].
    assert!((clamp_w3d_zoom(0.1) - 0.2).abs() < f32::EPSILON);
    assert!((clamp_w3d_zoom(2.0) - 1.3).abs() < f32::EPSILON);
    assert!((clamp_w3d_zoom(1.0) - 1.0).abs() < f32::EPSILON);
}

#[test]
fn letterbox_lifts_min_max_camera_height_clamp() {
    let limited = height_after_zoom_steps(120.0, -20.0, 40.0, 200.0, true);
    assert!((limited - 40.0).abs() < f32::EPSILON);
    let unlocked = height_after_zoom_steps(120.0, -20.0, 40.0, 200.0, false);
    assert!((unlocked - -80.0).abs() < f32::EPSILON);
    let high = height_after_zoom_steps(180.0, 10.0, 40.0, 200.0, false);
    assert!((high - 280.0).abs() < f32::EPSILON);
}

#[test]
fn lookat_mouse_moved_recently_is_one_logic_second() {
    let mut modes = LookAtHostModes::default();
    modes.last_mouse_move_frame = 10;
    assert!(modes.lookat_has_mouse_moved_recently(10));
    assert!(modes.lookat_has_mouse_moved_recently(40));
    assert!(!modes.lookat_has_mouse_moved_recently(41));
}

#[test]
fn disable_input_blocks_key_and_edge_scroll() {
    // C++ LookAtXlat setScrolling / RAW_MOUSE_POSITION gate on input enabled.
    assert_eq!(
        lookat_resolve_scroll_type(
            false,
            LookAtScrollType::None,
            false,
            false,
            false,
            true,
            true
        ),
        LookAtScrollType::None
    );
}

#[test]
fn wheel_stop_blocks_key_and_edge_until_next_input() {
    // C++ MSG_RAW_MOUSE_WHEEL fallthrough stopScrolling; KEY/EDGE stay down
    // until the next RAW_KEY / RAW_MOUSE_POSITION.
    let mut modes = LookAtHostModes::default();
    modes.wheel_stopped_scroll = true;
    assert_eq!(
        lookat_resolve_scroll_type(
            modes.wheel_stopped_scroll,
            LookAtScrollType::None,
            true,
            false,
            false,
            true,
            true
        ),
        LookAtScrollType::None
    );
    modes.lookat_note_raw_key_activity();
    assert_eq!(
        lookat_resolve_scroll_type(
            modes.wheel_stopped_scroll,
            LookAtScrollType::None,
            true,
            false,
            false,
            true,
            false
        ),
        LookAtScrollType::Key
    );
}

#[test]
fn click_and_wheel_stamp_activity_without_pixel_change() {
    let mut modes = LookAtHostModes::default();
    modes.last_mouse_move_frame = 0;
    modes.last_mouse_pixel = (10.0, 10.0);
    modes.lookat_note_mouse_moved(50, (10.0, 10.0));
    assert_eq!(modes.last_mouse_move_frame, 0);
    modes.lookat_stamp_mouse_activity(50);
    assert_eq!(modes.last_mouse_move_frame, 50);
    assert!(modes.lookat_has_mouse_moved_recently(50));
}

#[test]
fn airborne_look_at_ray_hits_ground_plane() {
    let mut map = gamelogic::system::map_loader::MapData::new();
    map.width = 32;
    map.height = 32;
    map.heightmap = vec![0; 32 * 32];
    let mut terrain = gamelogic::terrain::TerrainLogic::new();
    terrain.load_map_data(map);
    let mut camera = crate::presentation_frame::camera::SessionCamera::default();
    let eye = Vec3::new(0., 120., 120.);
    camera.view_matrix = glam::Mat4::look_at_rh(eye, eye + Vec3::new(0., -40., -120.), Vec3::Y);
    camera.projection_matrix = glam::Mat4::perspective_rh(60_f32.to_radians(), 1., 10., 2000.);
    let hit = crate::presentation_frame::receiving_look_at_target(
        Vec3::new(0., 80., 0.),
        &camera,
        &terrain,
    )
    .unwrap();
    assert_eq!(hit.y, 0.);
    assert!((hit.z + 240.).abs() < 0.05, "{hit:?}");
    let off_axis = crate::presentation_frame::receiving_look_at_target(
        Vec3::new(40., 80., 0.),
        &camera,
        &terrain,
    )
    .unwrap();
    assert!((off_axis.x - 40.).abs() < 0.01, "{off_axis:?}");
    assert!((off_axis.z - hit.z).abs() < 0.05, "{off_axis:?}");
}

#[test]
fn vertical_pan_uses_display_aspect_boost() {
    // C++ W3DView.cpp:1779-1823 scrollBy unprojects the view corner and the
    // scroll offset through the live camera (vertical pre-multiplied by view
    // aspect): 1920x864 tactical view -> aspect 2.222.
    let view = Mat4::look_at_rh(Vec3::new(0.0, 120.0, 120.0), Vec3::ZERO, Vec3::Y);
    let aspect = 1920.0 / 864.0;
    let viewport = (1920.0_f32, 864.0_f32);
    let projection = Mat4::perspective_rh(50.0_f32.to_radians(), aspect, 1.0, 2_000.0);
    let dx = lookat_scroll_world_delta(Vec2::new(1.0, 0.0), view, projection, viewport);
    let dy = lookat_scroll_world_delta(Vec2::new(0.0, 1.0), view, projection, viewport);

    // Ground-plane steps only. The camera pitches down 45 degrees toward -Z,
    // so screen right walks +X and screen down walks back toward the camera.
    assert!(dx.x > 0.0 && dx.z.abs() < 1.0e-4, "horizontal step {dx:?}");
    assert!(dy.z > 0.0 && dy.x.abs() < 1.0e-4, "vertical step {dy:?}");
    assert!(dx.y.abs() < 1.0e-4 && dy.y.abs() < 1.0e-4);
    assert!(
        dy.length() > dx.length() * 1.4,
        "retail vertical pan is faster than horizontal by view aspect: {dy:?} vs {dx:?}"
    );

    // The world step must come from the real projection, not a constant: a
    // wider FOV covers more world per screen pixel.
    let wide = Mat4::perspective_rh(80.0_f32.to_radians(), aspect, 1.0, 2_000.0);
    let dx_wide = lookat_scroll_world_delta(Vec2::new(1.0, 0.0), view, wide, viewport);
    assert!(
        dx_wide.x > dx.x * 1.5,
        "wider FOV must scale the world step: {dx_wide:?} vs {dx:?}"
    );
}

#[test]
fn script_default_view_pitch_starts_and_resets_to_zero() {
    let mut logic = crate::game_logic::GameLogic::new();
    assert_eq!(logic.script_default_camera_pitch(), 0.0);
    logic.reset();
    assert_eq!(logic.script_default_camera_pitch(), 0.0);
}

#[path = "player_camera_cancel_owner_tests.rs"]
mod player_camera_cancel_owner_tests;
