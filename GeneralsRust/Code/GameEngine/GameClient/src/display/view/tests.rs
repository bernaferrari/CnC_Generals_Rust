use super::*;

#[test]
fn test_view_creation() {
    crate::test_support::ensure_ambient_host();
    let view = View::new();
    assert!(view.id() > 0);
    assert_eq!(view.width(), 0);
    assert_eq!(view.height(), 0);
}

#[test]
fn test_view_initialization() {
    crate::test_support::ensure_ambient_host();
    let mut view = View::new();
    view.init();

    assert_eq!(view.width(), DEFAULT_VIEW_WIDTH);
    assert_eq!(view.height(), DEFAULT_VIEW_HEIGHT);
    assert_eq!(
        view.origin(),
        (DEFAULT_VIEW_ORIGIN_X, DEFAULT_VIEW_ORIGIN_Y)
    );
    assert!(view.is_zoom_limited());
}

#[test]
fn test_angle_and_pitch_limits() {
    crate::test_support::ensure_ambient_host();
    let mut view = View::new();

    // Test pitch limiting
    view.set_pitch(PI); // Try to set extreme pitch
    let limit = PI / 5.0;
    assert!((view.pitch() - limit).abs() < 0.001);

    view.set_pitch(-PI); // Try negative extreme
    assert!((view.pitch() - (-limit)).abs() < 0.001);

    // Angle should not be limited
    view.set_angle(2.0 * PI);
    assert!((view.angle() - 2.0 * PI).abs() < 0.001);
}

#[test]
fn test_zoom_limits() {
    crate::test_support::ensure_ambient_host();
    let mut view = View::new();
    view.init();
    view.min_zoom = 0.2;
    view.max_zoom = 1.3;
    view.min_height_above_ground = 100.0;
    view.max_height_above_ground = 500.0;

    // C++ W3DView.cpp1930-1949 clamps zoom independently of zoomLimited.
    for limited in [true, false] {
        view.set_zoom_limited(limited);
        view.set_zoom(10.0);
        assert_eq!(view.zoom(), 1.3);
        view.set_zoom(-1.0);
        assert_eq!(view.zoom(), 0.2);
        view.set_zoom(0.75);
        assert_eq!(view.zoom(), 0.75);
    }
    // Only desired HAG uses zoomLimited (W3DView.cpp1902-1916).
    view.set_zoom_limited(true);
    view.set_height_above_ground(700.0);
    assert_eq!(view.height_above_ground(), 500.0);
    view.set_height_above_ground(50.0);
    assert_eq!(view.height_above_ground(), 100.0);
    view.set_zoom_limited(false);
    view.set_height_above_ground(700.0);
    assert_eq!(view.height_above_ground(), 700.0);
    view.set_height_above_ground(50.0);
    assert_eq!(view.height_above_ground(), 50.0);
}

#[test]
fn test_position_and_look_at() {
    crate::test_support::ensure_ambient_host();
    let mut view = View::new();
    view.init();
    view.set_position(&Point3::new(0.0, 0.0, 12.0));

    let target = Point3::new(100.0, 200.0, 0.0);
    view.look_at(&target);

    // C++ W3DView::lookAt stores the look-at point and forces z=0.
    assert!((view.position().x - target.x).abs() < 0.001);
    assert!((view.position().y - target.y).abs() < 0.001);
    assert!((view.position().z - 0.0).abs() < 0.001);

    let before = *view.position();
    view.scroll_by(&Vector2::new(50.0, -25.0));
    assert_ne!(*view.position(), before);
}

#[test]
fn test_view_location_save_restore() {
    crate::test_support::ensure_ambient_host();
    let mut view = View::new();
    view.init();

    // Set specific view state
    let look_target = Point3::new(100.0, 200.0, 10.0);
    view.set_position(&Point3::new(0.0, 0.0, 7.0));
    view.look_at(&look_target);
    let saved_position = *view.position();
    view.set_angle(PI / 4.0);
    view.set_pitch(PI / 6.0);
    view.set_zoom(0.5);

    // Save location
    let location = view.get_location();
    assert!(location.is_valid());

    // Change view
    view.look_at(&Point3::origin());
    view.set_angle(0.0);
    view.set_pitch(0.0);
    view.set_zoom(1.0);

    // Restore location
    view.set_location(&location);

    assert_eq!(*view.position(), saved_position);
    assert!((view.angle() - PI / 4.0).abs() < 0.001);
    assert!((view.pitch() - PI / 6.0).abs() < 0.001);
    assert!((view.zoom() - 0.5).abs() < 0.001);
}

#[test]
fn test_3d_camera_position_calculation() {
    crate::test_support::ensure_ambient_host();
    let mut view = View::new();
    view.init();

    // Set camera looking at origin with some height and angle
    view.look_at(&Point3::origin());
    view.set_height_above_ground(100.0);
    view.set_angle(PI / 4.0); // 45 degrees
    view.set_pitch(PI / 8.0); // Ensure camera has positive height component

    let camera_pos = view.get_3d_camera_position();

    // Camera should be offset from look-at point
    assert_ne!(camera_pos, *view.position());
    // Camera should be above ground
    assert!(camera_pos.z > 0.0);
}

#[test]
fn test_projection_mode_switching() {
    crate::test_support::ensure_ambient_host();
    let mut view = View::new();
    assert_eq!(view.projection_mode(), ProjectionMode::Perspective);

    view.set_projection_mode(ProjectionMode::Orthographic);
    assert_eq!(view.projection_mode(), ProjectionMode::Orthographic);
}

#[test]
fn test_move_camera_to_animated_ends_at_look_at_position() {
    crate::test_support::ensure_ambient_host();
    let mut view = View::new();
    view.init();

    let target = Point3::new(400.0, 300.0, 999.0);
    view.move_camera_to(&target, 1000, 0, false, 0.0, 0.0);

    for _ in 0..40 {
        view.update_view();
    }

    assert!(view.is_camera_movement_finished());
    assert!((view.position().x - target.x).abs() < 0.001);
    assert!((view.position().y - target.y).abs() < 0.001);
}

#[test]
fn test_move_camera_along_waypoint_path_reaches_last_waypoint() {
    crate::test_support::ensure_ambient_host();
    let mut view = View::new();
    view.init();

    let path = vec![
        Point3::new(100.0, 120.0, 0.0),
        Point3::new(260.0, 260.0, 0.0),
        Point3::new(520.0, 360.0, 0.0),
    ];
    view.move_camera_along_waypoint_path(&path, 1000, 0, true, 0.0, 0.0);

    for _ in 0..40 {
        view.update_view();
    }

    assert!(view.is_camera_movement_finished());
    let expected_x = path.last().unwrap().x;
    let expected_y = path.last().unwrap().y;
    assert!((view.position().x - expected_x).abs() < 0.001);
    assert!((view.position().y - expected_y).abs() < 0.001);
}

#[test]
fn test_camera_mod_final_time_multiplier_speeds_waypoint_path() {
    crate::test_support::ensure_ambient_host();
    let path = vec![
        Point3::new(100.0, 120.0, 0.0),
        Point3::new(260.0, 260.0, 0.0),
        Point3::new(520.0, 360.0, 0.0),
    ];
    let start_x = path[0].x;
    let start_y = path[0].y;

    // C++ `cameraModFinalTimeMultiplier` only retimes the *last* waypoint.
    // The first update still samples multiplier 1.0; after enough frames the
    // lerp into the final segment makes the accelerated path pull ahead.
    let mut normal = View::new();
    normal.init();
    normal.move_camera_along_waypoint_path(&path, 1000, 0, true, 0.0, 0.0);

    let mut accelerated = View::new();
    accelerated.init();
    accelerated.move_camera_along_waypoint_path(&path, 1000, 0, true, 0.0, 0.0);
    accelerated.camera_mod_final_time_multiplier(3);

    for _ in 0..20 {
        normal.update_view();
        accelerated.update_view();
    }
    let normal_dx = normal.position().x - start_x;
    let normal_dy = normal.position().y - start_y;
    let normal_distance = (normal_dx * normal_dx + normal_dy * normal_dy).sqrt();
    let fast_dx = accelerated.position().x - start_x;
    let fast_dy = accelerated.position().y - start_y;
    let fast_distance = (fast_dx * fast_dx + fast_dy * fast_dy).sqrt();

    assert!(
        fast_distance > normal_distance,
        "final-time multiplier must advance farther after several frames: fast={fast_distance} normal={normal_distance}"
    );
}

#[test]
fn test_camera_mod_freeze_time_clears_after_scripted_movement_finishes() {
    crate::test_support::ensure_ambient_host();
    let mut view = View::new();
    view.init();
    view.camera_mod_freeze_time();
    assert!(view.is_time_frozen());

    let path = vec![
        Point3::new(100.0, 120.0, 0.0),
        Point3::new(260.0, 260.0, 0.0),
        Point3::new(520.0, 360.0, 0.0),
    ];
    view.move_camera_along_waypoint_path(&path, 1000, 0, true, 0.0, 0.0);
    assert!(view.freeze_time_for_camera_movement_active);

    for _ in 0..40 {
        view.update_view();
    }

    assert!(view.is_camera_movement_finished());
    assert!(!view.is_time_frozen());
}

#[test]
fn test_camera_mod_freeze_angle_stops_rotation_progress() {
    crate::test_support::ensure_ambient_host();
    let mut view = View::new();
    view.init();
    view.rotate_camera(0.5, 1000, 0.0, 0.0);
    view.update_view();

    let frozen_angle = view.angle();
    view.camera_mod_freeze_angle();

    for _ in 0..6 {
        view.update_view();
        assert!((view.angle() - frozen_angle).abs() < 0.001);
    }
}

#[test]
fn test_wireframe_mode_applies_with_frame_delay() {
    crate::test_support::ensure_ambient_host();
    let mut view = View::new();
    view.init();

    assert!(!view.is_3d_wireframe_mode());
    assert!(!view.pending_3d_wireframe_mode());

    view.set_3d_wireframe_mode(true);
    assert!(!view.is_3d_wireframe_mode());
    assert!(view.pending_3d_wireframe_mode());

    view.update_view();
    assert!(!view.is_3d_wireframe_mode());
    assert!(view.pending_3d_wireframe_mode());

    view.update_view();
    assert!(view.is_3d_wireframe_mode());
    assert!(view.pending_3d_wireframe_mode());

    view.set_3d_wireframe_mode(false);
    assert!(view.is_3d_wireframe_mode());
    assert!(!view.pending_3d_wireframe_mode());

    view.update_view();
    assert!(view.is_3d_wireframe_mode());
    assert!(!view.pending_3d_wireframe_mode());

    view.update_view();
    assert!(!view.is_3d_wireframe_mode());
    assert!(!view.pending_3d_wireframe_mode());
}

#[test]
fn test_camera_commands_zero_duration_apply_immediately() {
    crate::test_support::ensure_ambient_host();
    let mut view = View::new();
    view.init();

    view.set_position(&Point3::new(15.0, 25.0, 3.0));
    view.set_angle(0.25);
    view.set_pitch(0.2);
    view.set_zoom(0.8);

    let target = Point3::new(320.0, 240.0, 99.0);
    view.move_camera_to(&target, 0, 0, false, 0.0, 0.0);
    assert!(view.camera_move.is_none());
    assert!((view.position().x - target.x).abs() < 0.001);
    assert!((view.position().y - target.y).abs() < 0.001);
    assert!((view.position().z - 0.0).abs() < 0.001);

    let old_angle = view.angle();
    view.rotate_camera(0.5, 0, 0.0, 0.0);
    assert!(view.camera_rotate.is_none());
    assert!((view.angle() - (old_angle + PI)).abs() < 0.001);

    view.zoom_camera(0.33, 0, 0.0, 0.0);
    assert!(view.camera_zoom.is_none());
    assert!((view.zoom() - 0.33).abs() < 0.001);

    view.pitch_camera(10.0, 0, 0.0, 0.0);
    assert!(view.camera_pitch.is_none());
    assert!((view.fx_pitch() - 10.0).abs() < 0.001);

    view.reset_camera(&Point3::new(0.0, 0.0, 0.0), 0, 0.0, 0.0);
    assert!(view.is_camera_movement_finished());
    assert!((view.angle() - view.default_angle).abs() < 0.001);
    assert!((view.pitch() - view.default_pitch_angle).abs() < 0.001);
    assert!((view.zoom() - view.max_zoom()).abs() < 0.001);
}

#[test]
fn test_camera_transition_frame_thresholds_match_cpp_division() {
    crate::test_support::ensure_ambient_host();
    // C++ parity: frame count uses integer division by frame length with minimum 1.
    assert_eq!(ms_to_frames(1), 1);
    assert_eq!(ms_to_frames(33), 1);
    assert_eq!(ms_to_frames(34), 1);
    assert_eq!(ms_to_frames(66), 1);
    assert_eq!(ms_to_frames(67), 2);
}

#[test]
fn test_rotate_camera_34ms_finishes_in_one_update() {
    crate::test_support::ensure_ambient_host();
    let mut view = View::new();
    view.init();
    view.set_angle(0.0);

    view.rotate_camera(0.5, 34, 0.0, 0.0);
    assert!(view.camera_rotate.is_some());

    view.update_view();

    assert!(view.camera_rotate.is_none());
    assert!((view.angle() - PI).abs() < 0.001);
}

#[test]
fn test_set_default_view_does_not_mutate_current_camera_immediately() {
    crate::test_support::ensure_ambient_host();
    let mut view = View::new();
    view.init();
    view.set_angle(0.35);
    view.set_pitch(0.2);

    let current_angle = view.angle();
    let current_pitch = view.pitch();
    let global_max = get_global_data()
        .map(|global| global.read().max_camera_height)
        .unwrap_or(view.max_height_above_ground);
    view.set_default_view(0.6, 1.5, 0.75);

    assert!((view.angle() - current_angle).abs() < 0.001);
    assert!((view.pitch() - current_pitch).abs() < 0.001);
    assert!(
        (view.max_height_above_ground - (global_max * 0.75).max(view.min_height_above_ground))
            .abs()
            < 0.001
    );
}

#[test]
fn test_shake_overflow_clamps_to_cpp_constant() {
    crate::test_support::ensure_ambient_host();
    let mut view = View::new();
    view.init();
    view.set_position(&Point3::origin());
    let epicenter = Point3::origin();
    let mut rules = game_engine::common::global_data::GlobalData::default();
    rules.shake_subtle_intensity = 2.0;
    rules.max_shake_intensity = 3.0;
    rules.max_shake_range = 100.0;
    let mut draws = 0;
    for _ in 0..8 {
        view.shake_with_services(&epicenter, CameraShakeType::Subtle, &rules, &mut |_, _| {
            draws += 1;
            0.0
        });
    }
    assert_eq!(draws, 8);
    assert_eq!(view.shake_intensity, 3.0);
}

#[test]
fn tick_impulse_shake_seeds_and_decays_offset() {
    crate::test_support::ensure_ambient_host();
    let mut view = View::new();
    view.init();
    view.set_position(&Point3::origin());
    view.shake(&Point3::origin(), CameraShakeType::Severe);
    let pending = view.impulse_shake_offset();
    assert_eq!((pending.x, pending.y), (0.0, 0.0));
    let before = view.camera_shake_intensity();
    view.tick_impulse_shake();
    let committed = view.impulse_shake_offset();
    assert!(committed.x.abs() > 0.0 || committed.y.abs() > 0.0);
    assert!(view.camera_shake_intensity() < before);
}

#[test]
fn test_camera_mod_final_zoom_uses_remaining_movement_time() {
    crate::test_support::ensure_ambient_host();
    let mut view = View::new();
    view.init();

    view.move_camera_to(&Point3::new(300.0, 200.0, 0.0), 1000, 0, false, 0.0, 0.0);
    assert!(view.camera_move.is_some());
    assert!(view.camera_zoom.is_none());

    // Advance once so camera-mod computes a non-full remaining duration.
    view.update_view();
    view.camera_mod_final_zoom(0.45, 0.0, 0.0);
    assert!(view.camera_zoom.is_some());

    for _ in 0..40 {
        view.update_view();
    }
    assert!((view.zoom() - 0.45).abs() < 0.001);
}

#[test]
fn test_camera_mod_final_pitch_uses_remaining_movement_time() {
    crate::test_support::ensure_ambient_host();
    let mut view = View::new();
    view.init();

    view.move_camera_to(&Point3::new(300.0, 200.0, 0.0), 1000, 0, false, 0.0, 0.0);
    assert!(view.camera_move.is_some());
    assert!(view.camera_pitch.is_none());

    view.update_view();
    view.camera_mod_final_pitch(0.4, 0.0, 0.0);
    assert!(view.camera_pitch.is_some());

    for _ in 0..40 {
        view.update_view();
    }
    assert!((view.fx_pitch() - 0.4).abs() < 0.001);
}

#[test]
fn test_camera_mod_look_toward_only_applies_while_path_active() {
    crate::test_support::ensure_ambient_host();
    let mut idle = View::new();
    idle.init();
    idle.set_angle(0.33);
    idle.camera_mod_look_toward(&Point3::new(500.0, 100.0, 0.0));
    assert!((idle.angle() - 0.33).abs() < 0.001);

    let path = vec![
        Point3::new(100.0, 120.0, 0.0),
        Point3::new(260.0, 260.0, 0.0),
        Point3::new(520.0, 360.0, 0.0),
    ];

    let mut baseline = View::new();
    baseline.init();
    baseline.move_camera_along_waypoint_path(&path, 1000, 0, true, 0.0, 0.0);
    baseline.update_view();
    let baseline_angle = baseline.angle();

    let mut modified = View::new();
    modified.init();
    modified.move_camera_along_waypoint_path(&path, 1000, 0, true, 0.0, 0.0);
    modified.camera_mod_look_toward(&Point3::new(900.0, 100.0, 0.0));
    modified.update_view();
    let modified_angle = modified.angle();

    assert!((baseline_angle - modified_angle).abs() > 0.001);
}

#[test]
fn test_camera_mod_final_move_to_retargets_path_endpoint() {
    crate::test_support::ensure_ambient_host();
    let path = vec![
        Point3::new(100.0, 120.0, 0.0),
        Point3::new(260.0, 260.0, 0.0),
        Point3::new(520.0, 360.0, 0.0),
    ];

    let mut baseline = View::new();
    baseline.init();
    baseline.move_camera_along_waypoint_path(&path, 1000, 0, true, 0.0, 0.0);
    for _ in 0..40 {
        baseline.update_view();
    }

    let mut modified = View::new();
    modified.init();
    modified.move_camera_along_waypoint_path(&path, 1000, 0, true, 0.0, 0.0);
    let retarget = Point3::new(780.0, 510.0, 0.0);
    modified.camera_mod_final_move_to(&retarget);
    for _ in 0..40 {
        modified.update_view();
    }

    let expected_x = retarget.x;
    let expected_y = retarget.y;
    assert!((modified.position().x - expected_x).abs() < 0.001);
    assert!((modified.position().y - expected_y).abs() < 0.001);
    assert!(
        (baseline.position().x - modified.position().x).abs() > 0.001
            || (baseline.position().y - modified.position().y).abs() > 0.001
    );
}

#[test]
#[test]
fn authored_look_toward_retarget_resets_cur_frame_and_start() {
    crate::test_support::ensure_ambient_host();
    let mut view = View::new();
    view.init();
    view.set_position(&Point3::new(100.0, 100.0, 0.0));
    view.set_angle(0.0);
    view.rotate_camera_toward_position(&Point3::new(200.0, 100.0, 0.0), 1000, 0.0, 0.0, false);
    let first = view.rotate_camera_toward.clone().expect("toward active");
    assert_eq!(first.cur_frame, 0);
    // Advance mid-transition.
    for _ in 0..5 {
        view.update_view();
    }
    let mid = view.rotate_camera_toward.clone().expect("still active");
    assert!(mid.cur_frame > 0, "transition must advance before retarget");
    let mid_start = mid.start_angle;
    // Authored retarget must restart like C++ lookTowardPosition, not preserve frame.
    view.rotate_camera_toward_position(&Point3::new(100.0, 300.0, 0.0), 1000, 0.0, 0.0, false);
    let retarget = view.rotate_camera_toward.clone().expect("retarget active");
    assert_eq!(retarget.cur_frame, 0, "authored retarget resets curFrame");
    assert_ne!(
        retarget.end_angle, mid.end_angle,
        "retarget aims at the new position"
    );
    assert_eq!(
        retarget.start_angle, view.angle,
        "retarget start is the live camera angle at admission"
    );
    let _ = mid_start;
}

fn test_camera_mod_look_toward_applies_to_active_move_transition() {
    crate::test_support::ensure_ambient_host();
    let mut view = View::new();
    view.init();
    view.set_angle(0.0);

    view.move_camera_to(&Point3::new(400.0, 320.0, 0.0), 1000, 0, false, 0.0, 0.0);
    view.camera_mod_look_toward(&Point3::new(900.0, 120.0, 0.0));
    assert!(view.camera_rotate.is_some());

    let start_angle = view.angle();
    view.update_view();
    assert!((view.angle() - start_angle).abs() > 0.0001);
}

#[test]
fn test_reset_camera_animated_targets_w3d_pitch_endpoint() {
    crate::test_support::ensure_ambient_host();
    let mut view = View::new();
    view.init();
    view.set_fx_pitch(0.25);

    view.reset_camera(&Point3::new(300.0, 300.0, 0.0), 1000, 0.0, 0.0);
    assert!(view.camera_pitch.is_some());

    for _ in 0..40 {
        view.update_view();
    }

    // C++ resetCamera / pitchCamera(1.0f) restores FXPitch, not orbit pitch.
    assert!((view.fx_pitch() - 1.0).abs() < 0.001);
}

#[test]
fn test_camera_locking() {
    crate::test_support::ensure_ambient_host();
    let mut view = View::new();

    assert_eq!(view.camera_lock_id(), None);

    view.set_camera_lock(Some(42));
    assert_eq!(view.camera_lock_id(), Some(42));

    view.set_snap_mode(CameraLockType::Tether, 100.0);
    // Additional testing would require object system integration
}

#[test]
fn test_field_of_view_limits() {
    crate::test_support::ensure_ambient_host();
    let mut view = View::new();

    // Test FOV limiting
    view.set_field_of_view(0.0); // Too small
    assert!(view.field_of_view() > 0.0);

    view.set_field_of_view(PI); // Too large
    assert!(view.field_of_view() < PI);

    // Test normal FOV
    view.set_field_of_view(PI / 3.0); // 60 degrees
    assert!((view.field_of_view() - PI / 3.0).abs() < 0.001);
}

#[test]
fn test_basic_vector_math() {
    crate::test_support::ensure_ambient_host();
    let v1 = Vector3::new(1.0, 0.0, 0.0);
    let v2 = Vector3::new(0.0, 1.0, 0.0);

    let cross = v1.cross(&v2);
    assert_eq!(cross, Vector3::new(0.0, 0.0, 1.0));

    let dot = v1.dot(&v2);
    assert_eq!(dot, 0.0); // Perpendicular vectors

    assert!((v1.magnitude() - 1.0).abs() < 0.001);
}

#[test]
fn test_point_vector_operations() {
    crate::test_support::ensure_ambient_host();
    let p1 = Point3::new(1.0, 2.0, 3.0);
    let p2 = Point3::new(4.0, 5.0, 6.0);
    let v = Vector3::new(1.0, 1.0, 1.0);

    let diff = Vector3::new(p2.x - p1.x, p2.y - p1.y, p2.z - p1.z);
    assert_eq!(diff, Vector3::new(3.0, 3.0, 3.0));

    let moved = Point3::new(p1.x + v.x, p1.y + v.y, p1.z + v.z);
    assert_eq!(moved, Point3::new(2.0, 3.0, 4.0));
}

#[test]
fn init_height_for_map_rebuilds_camera_offset_from_ground() {
    crate::test_support::ensure_ambient_host();
    let mut view = View::new();
    view.init();
    view.set_position(&Point3::new(10.0, 20.0, 0.0));
    view.init_height_for_map();
    assert!(view.ground_level() <= 120.0);
    assert!(view.camera_offset().z > view.ground_level());
}

#[test]
fn real_zoom_mode_changes_field_of_view() {
    crate::test_support::ensure_ambient_host();
    let mut view = View::new();
    view.init();
    view.set_zoom_limited(false);
    view.set_zoom(0.6);
    let default_fov = view.field_of_view();
    view.camera_enable_real_zoom_mode();
    view.update_view();
    assert!(view.is_real_zoom_cam());
    assert!((view.field_of_view() - default_fov).abs() > 0.001);
    view.camera_disable_real_zoom_mode();
    assert!(!view.is_real_zoom_cam());
    assert!((view.field_of_view() - DEFAULT_FOV_RADIANS).abs() < 0.001);
}

#[test]
fn slave_mode_attaches_until_named_object_missing() {
    crate::test_support::ensure_ambient_host();
    let mut view = View::new();
    view.init();
    view.camera_enable_slave_mode("MissingCameraBone", "CAMERABONE");
    assert!(!view.is_camera_slaved());
    view.update_view();
    assert!(!view.is_camera_slaved());
}

#[test]
fn screen_to_terrain_returns_finite_point() {
    crate::test_support::ensure_ambient_host();
    let mut view = View::new();
    view.init();
    view.look_at(&Point3::new(100.0, 200.0, 0.0));
    let hit = view
        .screen_to_terrain(&IPoint2::new(320, 240))
        .expect("pick");
    assert!(hit.x.is_finite() && hit.y.is_finite() && hit.z.is_finite());
}

#[test]
fn pick_drawable_skips_when_view_uninitialized() {
    crate::test_support::ensure_ambient_host();
    let view = View::new();
    assert!(
        view.pick_drawable(
            &IPoint2::new(10, 10),
            false,
            PickType::Selectable,
            &mut |_, _| false
        )
        .is_none()
    );
}
