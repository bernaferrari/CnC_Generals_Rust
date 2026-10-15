//! These tests execute production axis/HAG and codec boundaries without a GPU.
//! They do not construct a windowed engine or certify unowned Core camera FX.
use super::*;
use crate::cnc_game_engine::camera_height::{CameraHeight, HeightRules, HeightStep};
use crate::cnc_game_engine::camera_motion::{AxisKind, AxisMotion, MotionAdmission, step_axis};

fn fixture() -> CameraState {
    let mut height = CameraHeight::new(300.0, 37.5, 0.0);
    height.initialize_map(23.0, 300.0, 37.5, 13.0);
    height.set_adjustment_enabled(true);
    height.set_zoom_limited(false);
    height.set_height(470.0, 100.0, 500.0);
    CameraState {
        completed_eye: Vec3::new(0.0, 80.0, 90.0),
        completed_forward: Vec3::NEG_Z,
        completed_up: Vec3::Y,
        height: height.capture_continuation(),
        look_at: super::super::mouse::LookAtHostModes::default().capture_continuation(),
        display: NativeDisplayState::new().capture_continuation(0).unwrap(),
        slave: None,
        cursor: None,
        control_group_click: None,
        camera_position: Vec3::new(0.0, 80.0, 90.0),
        camera_target: Vec3::ZERO,
        scripted_camera_constraint_widen: None,
        camera_zoom: 1.0,
        camera_zoom_target: None,
        camera_zoom_start: 1.0,
        camera_zoom_duration: 0.0,
        camera_zoom_frame: 0,
        camera_zoom_ease_in: 0.0,
        camera_zoom_ease_out: 0.0,
        camera_pitch_radians: 0.0,
        camera_pitch_target: None,
        camera_pitch_start: 0.0,
        camera_pitch_duration: 0.0,
        camera_pitch_frame: 0,
        camera_pitch_ease_in: 0.0,
        camera_pitch_ease_out: 0.0,
        camera_fx_pitch: 1.0,
        camera_yaw_radians: 0.0,
        camera_yaw_target: None,
        camera_yaw_start: 0.0,
        camera_yaw_duration: 0.0,
        camera_yaw_frame: 0,
        camera_yaw_ease_in: 0.0,
        camera_yaw_ease_out: 0.0,
        camera_shake_offset: Vec3::ZERO,
        camera_shake_rotation: Vec3::ZERO,
        screen_shake_intensity: 0.0,
        screen_shake_angle_cos: 0.0,
        screen_shake_angle_sin: 0.0,
        script_camera_shakers: Vec::new(),
        script_fps_limit: None,
        camera_follow_factor: 0.0,
        host_match_camera_follow_active: None,
        host_match_camera_follow_position: None,
        host_match_camera_follow_id: None,
        host_match_script_camera_max_height: None,
        host_match_script_camera_pitch: None,
        keys_pressed: Default::default(),
        mouse_position: (0.0, 0.0),
        mouse_cursor_seen: false,
        mouse_world_position: Vec3::ZERO,
        sticky_waypoint_mode: false,
        sticky_auto_attack: false,
        use_alternate_mouse: false,
        is_dragging: false,
        selection_start: None,
        selection_start_screen: None,
        last_click_position: None,
        last_right_click_screen: None,
        left_click_release_behavior: Default::default(),
        displayed_max_selection_warning: false,
        lmb_context_started_physically: false,
        rmb_scroll_anchor: None,
        is_rmb_scrolling: false,
        move_rmb_scroll_anchor: false,
        draw_rmb_scroll_anchor: false,
        rmb_scroll_started_physically: false,
        rmb_deselect_down_screen: None,
        rmb_deselect_down_camera: None,
        is_mmb_rotating: false,
        mmb_anchor: None,
        selected_objects: Vec::new(),
        camera_view_bookmarks: [None; 8],
        camera_rotate_left_held: false,
        camera_rotate_right_held: false,
        camera_zoom_in_held: false,
        camera_zoom_out_held: false,
        camera_tracking_selection: false,
        replay_fast_forward: false,
        prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click: false,
        frame_counter: 0,
        script_fps_limit_last_tick_age: None,
        last_click_time_age: None,
        last_right_click_time_age: None,
        rmb_deselect_down_at_age: None,
    }
}
fn advance(state: &mut CameraState, full: bool, fast: bool, paused: bool) {
    let result = step_axis(
        AxisMotion {
            value: &mut state.camera_zoom,
            target: &mut state.camera_zoom_target,
            frame: &mut state.camera_zoom_frame,
            start: state.camera_zoom_start,
            duration_seconds: state.camera_zoom_duration,
            ease_in_seconds: state.camera_zoom_ease_in,
            ease_out_seconds: state.camera_zoom_ease_out,
        },
        AxisKind::ZoomPitch,
        MotionAdmission {
            update_views: full,
            fast_camera_only: fast,
            paused,
            debug_frozen: false,
            disable_camera_movement: false,
        },
    );
    let admission = MotionAdmission {
        update_views: full,
        fast_camera_only: fast,
        paused,
        debug_frozen: false,
        disable_camera_movement: false,
    };
    let pitch = step_axis(
        AxisMotion {
            value: &mut state.camera_fx_pitch,
            target: &mut state.camera_pitch_target,
            frame: &mut state.camera_pitch_frame,
            start: state.camera_pitch_start,
            duration_seconds: state.camera_pitch_duration,
            ease_in_seconds: state.camera_pitch_ease_in,
            ease_out_seconds: state.camera_pitch_ease_out,
        },
        AxisKind::ZoomPitch,
        admission,
    );
    let yaw = step_axis(
        AxisMotion {
            value: &mut state.camera_yaw_radians,
            target: &mut state.camera_yaw_target,
            frame: &mut state.camera_yaw_frame,
            start: state.camera_yaw_start,
            duration_seconds: state.camera_yaw_duration,
            ease_in_seconds: state.camera_yaw_ease_in,
            ease_out_seconds: state.camera_yaw_ease_out,
        },
        AxisKind::Yaw,
        admission,
    );
    if full {
        let mut height = CameraHeight::new(1.0, 37.5, 0.0);
        height.admit_continuation(state.height.clone());
        height.advance(
            &mut state.camera_zoom,
            HeightStep {
                terrain: 37.0,
                paused,
                scrolling: false,
                scroll_amount: 0.0,
                scripted: result.was_active || pitch.was_active || yaw.was_active,
                replay_camera: false,
                rules: HeightRules {
                    min: 100.0,
                    max: 500.0,
                    enforce_max: false,
                    adjust_speed: 0.2,
                    scroll_cutoff: 0.3,
                },
            },
        );
        state.height = height.capture_continuation();
    }
}
fn roundtrip(state: &CameraState) -> CameraState {
    let bytes = codec().serialize(state).unwrap();
    CnCGameEngine::decode_camera_continuation(&bytes).unwrap().0
}
#[test]
fn active_axis_height_future_survives_roundtrip_pause_fast_and_peer() {
    let mut uninterrupted = fixture();
    uninterrupted.camera_zoom_target = Some(1.8);
    uninterrupted.camera_zoom_duration = 0.66;
    uninterrupted.camera_zoom_ease_in = 0.09;
    uninterrupted.camera_zoom_ease_out = 0.12;
    uninterrupted.camera_pitch_start = 1.0;
    uninterrupted.camera_pitch_target = Some(1.3);
    uninterrupted.camera_pitch_duration = 0.99;
    uninterrupted.camera_yaw_target = Some(6.7);
    uninterrupted.camera_yaw_duration = 0.825;
    for _ in 0..7 {
        advance(&mut uninterrupted, true, false, false);
    }
    let mut restored = roundtrip(&uninterrupted);
    let mut peer = fixture();
    peer.camera_zoom = 4.0;
    peer.height = CameraHeight::new(990.0, 25.0, 0.0).capture_continuation();
    for (full, fast, paused) in [
        (true, false, true),
        (false, true, true),
        (false, false, false),
    ]
    .into_iter()
    .chain(std::iter::repeat_n((true, false, false), 40))
    {
        advance(&mut uninterrupted, full, fast, paused);
        advance(&mut peer, true, false, false);
        advance(&mut restored, full, fast, paused);
        assert_eq!(
            restored.camera_zoom.to_bits(),
            uninterrupted.camera_zoom.to_bits()
        );
        assert_eq!(restored.camera_zoom_frame, uninterrupted.camera_zoom_frame);
        assert_eq!(
            restored.camera_fx_pitch.to_bits(),
            uninterrupted.camera_fx_pitch.to_bits()
        );
        assert_eq!(
            restored.camera_yaw_radians.to_bits(),
            uninterrupted.camera_yaw_radians.to_bits()
        );
        assert_eq!(
            restored.camera_pitch_frame,
            uninterrupted.camera_pitch_frame
        );
        assert_eq!(restored.camera_yaw_frame, uninterrupted.camera_yaw_frame);
        assert_eq!(
            restored.camera_zoom_target,
            uninterrupted.camera_zoom_target
        );
        assert_eq!(
            codec().serialize(&restored.height).unwrap(),
            codec().serialize(&uninterrupted.height).unwrap()
        );
    }
    assert!(restored.camera_zoom_target.is_none());
}
#[test]
fn codec_rejects_truncation_trailing_nonfinite_and_unknown_cursor_before_admission() {
    let state = fixture();
    let bytes = codec().serialize(&state).unwrap();
    assert!(CnCGameEngine::decode_camera_continuation(&bytes[..bytes.len() - 1]).is_err());
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(CnCGameEngine::decode_camera_continuation(&trailing).is_err());
    assert!(CnCGameEngine::decode_camera_continuation(&vec![0; MAX_BYTES as usize + 1]).is_err());
    let mut bad = fixture();
    bad.camera_yaw_radians = f32::NAN;
    assert!(CnCGameEngine::decode_camera_continuation(&codec().serialize(&bad).unwrap()).is_err());
    let mut bad = fixture();
    bad.completed_forward = Vec3::splat(f32::MAX);
    assert!(CnCGameEngine::decode_camera_continuation(&codec().serialize(&bad).unwrap()).is_err());
    let mut bad = fixture();
    bad.cursor = Some("not-a-native-cursor".into());
    assert!(CnCGameEngine::decode_camera_continuation(&codec().serialize(&bad).unwrap()).is_err());
    assert_eq!(state.camera_zoom.to_bits(), 1.0_f32.to_bits());
    // Script targets are stored raw, unlike the current visible zoom. Preserve
    // any finite target accepted by the existing request/movement path.
    let mut raw_target = fixture();
    raw_target.camera_zoom_target = Some(0.0);
    assert!(
        CnCGameEngine::decode_camera_continuation(&codec().serialize(&raw_target).unwrap()).is_ok()
    );
}
#[test]
fn raw_pose_and_committed_view_remain_distinct_and_keys_survive() {
    let mut state = fixture();
    state.camera_yaw_radians = 7.25;
    state.camera_zoom_frame = 14;
    state.camera_zoom_target = Some(1.7);
    state.slave = Some(("CameraActor".into(), "CameraBone".into()));
    state.keys_pressed.insert(winit::keyboard::Key::Named(
        winit::keyboard::NamedKey::ArrowLeft,
    ));
    state
        .keys_pressed
        .insert(winit::keyboard::Key::Character("λ".into()));
    state.cursor = Some("Scroll".into());
    let restored = roundtrip(&state);
    assert_eq!(restored.keys_pressed, state.keys_pressed);
    assert_eq!(restored.completed_eye, state.completed_eye);
    assert_eq!(restored.completed_forward, state.completed_forward);
    assert_eq!(restored.camera_zoom_frame, 14);
    let pose = NativeCameraContinuation(restored).camera_persist();
    assert_eq!(pose.angle, 7.25);
    assert_eq!(pose.position, state.camera_position.to_array());
    assert_eq!(pose.target, state.camera_target.to_array());
}

#[test]
#[cfg(feature = "game_client")]
fn native_camera_continuation_actual_archives_use_one_canonical_pose() {
    use crate::save_load::snapshot::{CameraPersist, ClientDrawableWorldSnapshot, ClientSaveState};
    use crate::save_load::{GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo};
    let directory = tempfile::tempdir().unwrap();
    let mut saves = SaveFileManager::with_save_directory(directory.path());
    let info = SaveGameInfo {
        filename: "camera".into(),
        display_name: "camera".into(),
        description: "camera".into(),
        map_name: "camera".into(),
        pristine_map_name: None,
        campaign_side: None,
        mission_number: None,
        save_date: std::time::UNIX_EPOCH,
        game_version: "test".into(),
        play_time: Duration::ZERO,
        difficulty: GameDifficulty::Medium,
        save_type: SaveFileType::Normal,
    };
    let world = crate::game_logic::GameLogic::new();
    let mut a = fixture();
    a.camera_zoom_target = Some(0.7);
    a.camera_zoom_duration = 3.0;
    a.camera_zoom_frame = 7;
    let mut b = fixture();
    b.camera_target = Vec3::splat(90.0);
    b.camera_yaw_radians = 1.5;
    for (name, state) in [("native_a", &a), ("native_b", &b)] {
        let mut client = game_client::core::game_client::GameClient::new().unwrap();
        saves
            .save_game_with_client_save_state(
                name,
                &world,
                ClientDrawableWorldSnapshot::default(),
                &mut client,
                &info,
                ClientSaveState {
                    // A second caller-supplied pose cannot override the full owner's pose.
                    camera: Some(CameraPersist {
                        angle: 9.0,
                        position: [999.0; 3],
                        target: [888.0; 3],
                        zoom: 1.2,
                    }),
                    camera_continuation: Some(codec().serialize(state).unwrap()),
                    control_groups: None,
                },
            )
            .unwrap();
    }
    let (peer, _) = saves.load_game_snapshot("native_b").unwrap();
    let (saved, _) = saves.load_game_snapshot("native_a").unwrap();
    let saved_native =
        crate::save_load::snapshot::read_native_camera_continuation(&saved.lifecycle_tail)
            .unwrap()
            .unwrap();
    let peer_native =
        crate::save_load::snapshot::read_native_camera_continuation(&peer.lifecycle_tail)
            .unwrap()
            .unwrap();
    assert_eq!(
        saved.persist_v18.camera().unwrap(),
        saved_native.camera_persist()
    );
    assert_eq!(
        peer.persist_v18.camera().unwrap(),
        peer_native.camera_persist()
    );
    assert_eq!(saved_native.0.camera_zoom_target, Some(0.7));
    assert_eq!(saved_native.0.camera_zoom_frame, 7);
    assert_eq!(saved_native.0.camera_zoom_duration, 3.0);
    assert_eq!(peer_native.0.camera_target, Vec3::splat(90.0));
    assert_eq!(
        codec().serialize(&saved_native.0.height).unwrap(),
        codec().serialize(&a.height).unwrap()
    );
    assert_eq!(saved_native.0.completed_eye, a.completed_eye);
}

#[test]
fn settling_hag_goal_survives_native_record_instead_of_freezing_visible_height() {
    let mut uninterrupted = fixture();
    for _ in 0..7 {
        advance(&mut uninterrupted, true, false, false);
    }
    let mut restored = roundtrip(&uninterrupted);
    let mut legacy_static_pose = roundtrip(&uninterrupted);
    let mut height = CameraHeight::new(1.0, 37.5, 0.0);
    height.admit_continuation(uninterrupted.height.clone());
    assert_eq!(height.desired(), 470.0);
    // The old pose-only admission froze the current visible height as its goal.
    // At this save point the camera is still settling toward a different goal.
    height.admit_restored_pose_height(legacy_static_pose.camera_zoom, 37.0);
    assert_ne!(height.desired(), 470.0);
    legacy_static_pose.height = height.capture_continuation();
    for _ in 0..40 {
        advance(&mut uninterrupted, true, false, false);
        advance(&mut restored, true, false, false);
        advance(&mut legacy_static_pose, true, false, false);
        assert_eq!(
            restored.camera_zoom.to_bits(),
            uninterrupted.camera_zoom.to_bits()
        );
    }
    assert_ne!(
        legacy_static_pose.camera_zoom.to_bits(),
        uninterrupted.camera_zoom.to_bits(),
        "the regression must detect replacing the in-flight HAG goal with the visible pose"
    );
}
