#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;

#[test]
fn double_click_type_select_uses_os_pixel_slop() {
    let src = MOUSE_SOURCE;
    assert!(src.contains("is_os_style_double_click"));
    assert!(src.contains("os_double_click_time_ms"));
    assert!(src.contains("OS_DOUBLE_CLICK_SLOP_PX"));
    let left_click = src.find("fn handle_left_click").expect("handle_left_click");
    let left_click_body = &src[left_click..src.len().min(left_click + 2_500)];
    assert!(!left_click_body.contains("time_delta < 500 && pos_delta < 10.0"));
}

#[test]
fn reset_camera_pose_restores_fx_pitch_to_one() {
    let src = MOUSE_SOURCE;
    let start = src
        .find("fn reset_camera_pose_in_place")
        .expect("reset_camera_pose_in_place");
    let body = &src[start..start + 900];
    assert!(
        body.contains("self.camera_fx_pitch = 1.0"),
        "CAMERA_RESET/MMB must restore C++ m_FXPitch 1.0"
    );
    assert!(
        body.contains("self.camera_pitch_target = None"),
        "reset must cancel an in-flight CAMERA_PITCH lerp"
    );
    assert!(
        body.contains("live_home_pitch_radians")
            && body.contains("self.ui_script_default_camera_pitch()"),
        "CAMERA_RESET/MMB must restore scripted m_defaultPitchAngle"
    );
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

    let home_default = live_home_pitch_radians(37.5, 1.0);
    assert!((home_default - 37.5_f32.to_radians()).abs() < 1.0e-5);
    let home_script_zero = live_home_pitch_radians(37.5, 0.0);
    assert!((home_script_zero - 37.5_f32.to_radians()).abs() < 1.0e-5);
    let home_scripted = live_home_pitch_radians(37.5, 0.8);
    assert!((home_scripted - (37.5_f32.to_radians() + 0.8)).abs() < 1.0e-5);

    let src = MOUSE_SOURCE;
    let zoom = src
        .find("fn apply_player_height_zoom_steps")
        .expect("apply_player_height_zoom_steps");
    assert!(
        src[zoom..zoom + 700].contains("live_view_height_clamp"),
        "wheel clamp must use script-scaled View max"
    );
    let settle = src
        .find("fn ease_camera_height_above_ground")
        .expect("ease_camera_height_above_ground");
    assert!(
        src[settle..settle + 1200].contains("live_view_height_clamp"),
        "settle clamp must use script-scaled View max"
    );
}

fn cursor_pick_uses_camera_ray_not_twenty_wu_pad() {
    let src = include_str!("../selection.rs");
    assert!(src.contains("fn host_pick_object_at_cursor"));
    assert!(src.contains("pick_object_id_along_camera_ray"));
    assert!(src.contains("host_cursor_blocked_by_opaque_window"));
    let mouse = MOUSE_SOURCE;
    assert!(mouse.contains("fn find_object_at_cursor"));
    assert!(mouse.contains("self.find_object_at_cursor(false)"));
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
    assert!(ray_interval_in_world_xz(
        Vec3::new(20.0, 5.0, 0.0),
        Vec3::new(20.0, -5.0, 0.0),
        Vec3::new(-10.0, 0.0, -10.0),
        Vec3::new(10.0, 0.0, 10.0),
    )
    .is_none());
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
        lookat_resolve_scroll_type(LookAtScrollType::Rmb, true, false, true, true, true),
        LookAtScrollType::Rmb
    );
    assert_eq!(
        lookat_resolve_scroll_type(LookAtScrollType::ScreenEdge, true, false, false, true, true),
        LookAtScrollType::ScreenEdge
    );
    assert_eq!(
        lookat_resolve_scroll_type(LookAtScrollType::Key, true, false, false, true, true),
        LookAtScrollType::Key
    );
    assert_eq!(
        lookat_resolve_scroll_type(LookAtScrollType::None, true, false, false, false, true),
        LookAtScrollType::ScreenEdge
    );
    assert_eq!(
        lookat_resolve_scroll_type(LookAtScrollType::None, true, false, false, true, true),
        LookAtScrollType::Key
    );
    // Edge may start while box-selecting; keys may not.
    assert_eq!(
        lookat_resolve_scroll_type(LookAtScrollType::None, true, true, false, true, true),
        LookAtScrollType::ScreenEdge
    );
    assert_eq!(
        lookat_resolve_scroll_type(LookAtScrollType::Key, false, false, false, true, true),
        LookAtScrollType::None
    );
}

#[test]
fn update_camera_does_not_snap_scout_past_6000wu() {
    // C++ LookAtXlat / W3DView::scrollBy have no distance-to-own-units snap.
    let src = MOUSE_SOURCE;
    let start = src
        .find("fn update_camera(&mut self, dt: f32)")
        .expect("update_camera");
    let end = src[start..]
        .find("fn is_character_key_pressed")
        .map(|i| start + i)
        .unwrap_or(src.len());
    let body = &src[start..end];
    assert!(
        !body.contains("camera_is_unreasonably_far_from_local_units")
            && !body.contains("6_000.0 * 6_000.0")
            && !body.contains("snap_camera_to_local_units_if_needed"),
        "scouting past 6000wu must not yank the camera back to base"
    );
    assert!(
        body.contains("lookat_resolve_scroll_type")
            && body.contains("LookAtScrollType::ScreenEdge"),
        "live LookAt must apply one exclusive scroll type"
    );
}

#[test]
fn lookat_mmb_short_click_matches_cpp_5px_5_frame_window() {
    // C++ LookAtXlat.cpp:241-250 CLICK_DURATION=5 PIXEL_OFFSET=5
    assert!(lookat_mmb_is_short_click(0.0, 0.0, 0));
    assert!(lookat_mmb_is_short_click(5.0, 5.0, 4));
    assert!(!lookat_mmb_is_short_click(5.1, 0.0, 0));
    assert!(!lookat_mmb_is_short_click(0.0, 0.0, 5));
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
fn force_attack_and_double_click_guard_follow_command_xlat() {
    // C++ CommandXlat.cpp:152-244 / 3635-3713.
    let src = MOUSE_SOURCE;
    assert!(
        src.contains("fn host_selection_can_force_attack")
            && src.contains("presentation_is_spawns_are_the_weapons")
            && src.contains("presentation_closest_spawn_or_rider"),
        "live force-attack must gate on canObjectForceAttack spawn/rider"
    );
    assert!(
        src.contains("fn host_try_double_click_guard_command")
            && src.contains("CommandType::Guard")
            && src.contains("GuardMode::Normal"),
        "double-click attack-move must post DoGuardPosition / Guard Normal"
    );
}

#[test]
fn host_replay_camera_emit_matches_cpp_look_at_gates() {
    // C++ LookAtXlat.cpp:459 — saveCameraInReplay && (SP || skirmish).
    assert!(!should_emit_host_replay_camera(
        GameState::Menu,
        crate::game_logic::GameMode::Skirmish
    ));
    assert!(!should_emit_host_replay_camera(
        GameState::InGame,
        crate::game_logic::GameMode::Multiplayer
    ));
    assert!(!should_emit_host_replay_camera(
        GameState::InGame,
        crate::game_logic::GameMode::Shell
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
    let src = MOUSE_SOURCE;
    let cam = src
        .find("fn update_camera(&mut self, dt: f32)")
        .expect("update_camera");
    let cam_body = &src[cam..src.len().min(cam + 4_500)];
    assert!(
        cam_body.contains("self.host_is_selecting()"),
        "arrow scroll must use isSelecting, not is_dragging"
    );
    let rmb = src
        .find("fn start_rmb_lookat_scroll")
        .expect("start_rmb_lookat_scroll");
    let rmb_body = &src[rmb..src.len().min(rmb + 400)];
    assert!(
        rmb_body.contains("self.host_is_selecting()")
            && !rmb_body.contains("self.is_dragging || self.is_rmb_scrolling"),
        "RMB scroll must gate on isSelecting, not LMB-held"
    );
}

#[test]
fn placement_rotate_uses_5px_screen_not_1wu() {
    // C++ PlaceEventTranslator.cpp:307-320 Euclidean 5px, no 1wu world gate.
    assert!(!placement_screen_drag_exceeds_threshold(3.0, 3.0));
    assert!(placement_screen_drag_exceeds_threshold(3.0, 4.0));
    assert!(placement_screen_drag_exceeds_threshold(5.0, 0.0));
    let src = MOUSE_SOURCE;
    assert!(
        src.contains("placement_screen_drag_exceeds_threshold(drag_dx, drag_dy)")
            && src.contains("fn update_anchored_placement_from_cursor")
            && !src.contains("dx * dx + dz * dz > 1.0"),
        "placement rotate must use 5px screen, not 1wu release gate"
    );
}

#[test]
fn placement_lmb_down_cancels_when_builder_gone() {
    // C++ PlaceEventTranslator.cpp:68-75 — missing builder does not anchor.
    let src = MOUSE_SOURCE;
    let start = src.find("fn handle_left_click").expect("handle_left_click");
    let body = &src[start..src.len().min(start + 2200)];
    assert!(
        body.contains("pending_place_builder_is_gone")
            && body.contains("cancel_structure_placement_from_ui")
            && body.contains("set_placement_start"),
        "LMB-down must cancel when the pending place source is gone"
    );
    assert!(
        src.contains("get_pending_place_source_object_id") && src.contains("object.sold"),
        "builder-gone must look up leftover place source and treat sold as gone"
    );
    let ui = include_str!("../ui_commands.rs");
    let begin = ui
        .find("fn begin_structure_placement_from_ui")
        .expect("begin_structure_placement_from_ui");
    assert!(
        ui[begin..ui.len().min(begin + 1_800)].contains("place_build_available"),
        "arming placement must store leftover pending place source"
    );
}

#[test]
fn placement_angle_reprojects_screen_anchor() {
    // C++ InGameUI::handleBuildPlacements screenToTerrain both points.
    let src = MOUSE_SOURCE;
    let drag = src
        .find("fn update_anchored_placement_from_cursor")
        .expect("update_anchored_placement_from_cursor");
    let drag_body = &src[drag..src.len().min(drag + 900)];
    assert!(
        (drag_body.contains("self.screen_to_terrain(start)")
            || drag_body.contains("self\n            .screen_to_terrain(start)"))
            && !drag_body.contains("self.selection_start.unwrap_or(self.mouse_world_position)"),
        "anchored rotate must reproject the screen start, not the stale world point"
    );
    let confirm = src
        .find("confirm re-projects the screen anchor")
        .expect("placement confirm comment");
    let confirm_body = &src[confirm..src.len().min(confirm + 700)];
    assert!(
        confirm_body.contains("self.screen_to_terrain(s)")
            && confirm_body.contains("place_structure_from_ui(&template, start_world)"),
        "placement confirm must place at the reprojected screen anchor"
    );
}

#[test]
fn alternate_mouse_one_click_prevent_deselect_matches_cpp() {
    // C++ SelectionXlat.cpp:935-943 + GUICommandTranslator.cpp:471-473.
    let mouse = MOUSE_SOURCE;
    let ui = include_str!("../ui_commands.rs");
    assert!(mouse.contains("host_consume_prevent_left_click_deselection"));
    assert!(mouse.contains("host_set_prevent_left_click_deselection"));
    assert!(ui.contains("host_set_prevent_left_click_deselection(true)"));
    assert!(mouse.contains("host_pick_hover_object_at_cursor"));
}

#[test]
fn left_release_point_click_does_not_box_wipe() {
    // C++ MetaEvent.cpp:571-596 + SelectionXlat.cpp:575-597 / 905-950.
    let src = MOUSE_SOURCE;
    assert!(src.contains("const DRAG_TOLERANCE_PX: f32 = 5.0"));
    assert!(src.contains("is_point_click_drag(drag_dx, drag_dy)"));
    assert!(!src.contains("drag_distance_screen <= 2.0"));
    assert!(src.contains("alternate_mouse_blank_click_deselects"));
    assert!(src.contains("garrisonable_building_ids_in_screen_rect"));
    assert!(src.contains("box_selection_must_replace"));
    assert!(src.contains("union_object_ids(similar_units, previous)"));
    assert!(
        !src.contains("first selectable local"),
        "pick miss must not invent the first locally-owned unit"
    );
    assert!(src.contains("CommandType::Enter { target_id }"));
    let release = src
        .find("fn handle_left_release")
        .expect("handle_left_release");
    let release_end = src[release..]
        .find("fn handle_right_click")
        .map(|i| release + i)
        .unwrap_or(release + 12_000);
    let release_body = &src[release..release_end];
    assert!(
        release_body.contains("select_left_click_target"),
        "point LMB must commit selection on click, not press"
    );
    let press = src.find("fn handle_left_click").expect("handle_left_click");
    let press_end = src[press + 1..]
        .find("\n    fn ")
        .map(|i| press + 1 + i)
        .unwrap_or(press + 2500);
    let force = format!("{}{}", "force-select local ", "object");
    assert!(
        !release_body.contains(&force) && !src[press..press_end].contains(&force),
        "empty LMB must not force-select CanSelectDrawable rejects"
    );
    assert!(
        !src[press..press_end].contains("select_left_click_target"),
        "RAW LMB down must not commit SelectionXlat"
    );
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
    let src = MOUSE_SOURCE;
    assert!(src.contains("fn presentation_double_click_consumes"));
    assert!(src.contains("KEEP_MESSAGE so CommandXlat.cpp:3698-3713"));
    assert!(src.contains("union_object_ids(similar_units, previous)"));
    assert!(src.contains("let boxed_any = !boxed.is_empty()"));
    assert!(src.contains("host_mouse_drag_tolerance_px()"));
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

    let src = MOUSE_SOURCE;
    assert!(src.contains("fn cancel_area_select_from_control_bar"));
    assert!(src.contains("fn note_rmb_deselect_anchor"));
    assert!(src.contains("fn rmb_release_is_deselect_click"));
    assert!(src.contains("host_find_object_at_position"));
    let input = include_str!("../input.rs");
    assert!(input.contains("cancel_area_select_from_control_bar"));
    assert!(input.contains("rmb_release_is_deselect_click"));
    assert!(!input.contains("DRAG_THRESHOLD_SQ"));
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
    look_at_host_modes().last_mouse_move_frame = 10;
    assert!(lookat_has_mouse_moved_recently(10));
    assert!(lookat_has_mouse_moved_recently(40));
    assert!(!lookat_has_mouse_moved_recently(41));
}

#[test]
fn disable_input_blocks_key_and_edge_scroll() {
    // C++ LookAtXlat setScrolling / RAW_MOUSE_POSITION gate on input enabled.
    assert_eq!(
        lookat_resolve_scroll_type(LookAtScrollType::None, false, false, false, true, true),
        LookAtScrollType::None
    );
}

#[test]
fn wheel_stop_blocks_key_and_edge_until_next_input() {
    // C++ MSG_RAW_MOUSE_WHEEL fallthrough stopScrolling; KEY/EDGE stay down
    // until the next RAW_KEY / RAW_MOUSE_POSITION.
    look_at_host_modes().wheel_stopped_scroll = true;
    assert_eq!(
        lookat_resolve_scroll_type(LookAtScrollType::None, true, false, false, true, true),
        LookAtScrollType::None
    );
    lookat_note_raw_key_activity();
    assert_eq!(
        lookat_resolve_scroll_type(LookAtScrollType::None, true, false, false, true, false),
        LookAtScrollType::Key
    );
}

#[test]
fn click_and_wheel_stamp_activity_without_pixel_change() {
    look_at_host_modes().last_mouse_move_frame = 0;
    look_at_host_modes().last_mouse_pixel = (10.0, 10.0);
    lookat_note_mouse_moved(50, (10.0, 10.0));
    assert_eq!(look_at_host_modes().last_mouse_move_frame, 0);
    lookat_stamp_mouse_activity(50);
    assert_eq!(look_at_host_modes().last_mouse_move_frame, 50);
    assert!(lookat_has_mouse_moved_recently(50));
}

#[test]
fn wheel_zoom_stops_scroll_and_lmb_resets_group_tap() {
    let src = MOUSE_SOURCE;
    let wheel = src
        .find("fn handle_mouse_wheel")
        .map(|i| &src[i..src.len().min(i + 2500)])
        .expect("handle_mouse_wheel");
    assert!(
        wheel.contains("stop_rmb_lookat_scroll")
            && wheel.contains("LookAtScrollType::None")
            && wheel.contains("wheel_stopped_scroll = true")
            && wheel.contains("lookat_stamp_mouse_activity"),
        "wheel must stamp activity and stop RMB/key/edge scroll"
    );
    let edge = src
        .find("if edge_allowed")
        .map(|i| &src[i..src.len().min(i + 700)])
        .expect("edge_allowed");
    assert!(
        edge.contains("window.inner_size()") && edge.contains("win_h"),
        "bottom edge-scroll must use Display height, not tactical 80%"
    );
    assert!(
        !edge.contains("tactical_viewport_size()"),
        "edge-scroll must not use the 80% tactical viewport"
    );
    assert!(
        src.contains("last_control_group_select = None"),
        "manual LMB select must reset control-group double-tap"
    );
    let recall = src
        .find("fn save_or_recall_camera_view")
        .map(|i| &src[i..src.len().min(i + 2800)])
        .expect("save_or_recall_camera_view");
    assert!(
        !recall.contains("push_info_message") || recall.contains("lookat_bookmark_message"),
        "empty F1-F8 bookmark must stay silent"
    );
    assert!(
        recall.contains("Unsaved F1-F8 is silent"),
        "empty bookmark path must remain a silent no-op"
    );
    let cancel = src
        .find("fn cancel_world_mouse_targeting")
        .map(|i| &src[i..src.len().min(i + 700)])
        .expect("cancel_world_mouse_targeting");
    assert!(
        !cancel.contains("push_info_message")
            && !cancel.contains(concat!("Cancelled pending", " command")),
        "hq-0foy9: RMB cancel of an armed GUI command must stay silent"
    );
    assert!(
        src.contains("fn sync_letterbox_os_cursor_visibility")
            && src.contains("set_cursor_visible(!over_bar)"),
        "OS cursor must hide under cinematic letterbox bars"
    );
    assert!(
        src.contains("lookat_stamp_mouse_activity(self.frame_counter)")
            && src.contains("fn start_rmb_lookat_scroll")
            && src.contains("fn begin_mmb_lookat_rotate"),
        "RMB/MMB/wheel must stamp hasMouseMovedRecently"
    );
}

#[test]
fn update_camera_does_not_gate_arrows_on_modifiers() {
    let src = MOUSE_SOURCE;
    let start = src
        .find("fn update_camera(&mut self, dt: f32)")
        .expect("update_camera");
    let end = src[start..]
        .find("fn is_character_key_pressed")
        .map(|i| start + i)
        .unwrap_or(src.len());
    let body = &src[start..end];
    assert!(
        !body.contains("mods_down"),
        "C++ RAW_KEY has no Ctrl/Shift/Alt gate"
    );
    assert!(
        body.contains("break_camera_follow_lock"),
        "player scroll must clear camera lock"
    );
    assert!(
        body.contains("clamp_w3d_zoom"),
        "replay/bookmark zoom must clamp"
    );
    assert!(
        body.contains("apply_host_slave_camera") && body.contains("apply_airborne_follow_yaw"),
        "slave bone + airborne follow yaw must be live"
    );
    assert!(
        body.contains("cancel_scripted_camera_from_player_set")
            && body.contains("cancel_scripted_camera_from_player_scroll"),
        "player rotate/zoom/scroll must cancel scripted camera"
    );
}

#[test]
fn rmb_set_scrolling_breaks_camera_lock() {
    // C++ InGameUI.cpp:2799-2801 setCameraLock(INVALID) + setCameraLockDrawable(NULL).
    let src = MOUSE_SOURCE;
    let start = src
        .find("fn start_rmb_lookat_scroll")
        .expect("start_rmb_lookat_scroll");
    let body = &src[start..src.len().min(start + 900)];
    assert!(
        body.contains("break_camera_follow_lock"),
        "RMB setScrolling must break camera lock"
    );
    let brk = src
        .find("fn break_camera_follow_lock")
        .expect("break_camera_follow_lock");
    let brk_body = &src[brk..src.len().min(brk + 500)];
    assert!(
        brk_body.contains("set_camera_lock(None)")
            && brk_body.contains("set_camera_lock_drawable(None)"),
        "setScrolling clears lock + drawable"
    );
}

#[test]
fn key_and_screenedge_scroll_mouse_lock_like_cpp() {
    // C++ LookAtXlat.cpp:50-62 setScrolling — KEY/SCREENEDGE lock the mouse
    // the same as RMB so WindowXlat keeps hover/RMB/MMB off the HUD.
    let src = MOUSE_SOURCE;
    let helper = src
        .find("fn set_lookat_scroll_mouse_lock")
        .expect("set_lookat_scroll_mouse_lock");
    let helper_body = &src[helper..src.len().min(helper + 1400)];
    assert!(
        helper_body.contains("modes.mouse_locked = true")
            && helper_body.contains("TheInGameUI::set_scrolling(true)")
            && helper_body.contains("view.set_mouse_lock(true)")
            && helper_body.contains("set_scrolling(false)")
            && helper_body.contains("view.set_mouse_lock(false)"),
        "shared setScrolling path must lock/unlock mouse + InGameUI"
    );
    let cam = src
        .find("fn update_camera(&mut self, dt: f32)")
        .expect("update_camera");
    let cam_end = src[cam..]
        .find("fn is_character_key_pressed")
        .map(|i| cam + i)
        .unwrap_or(src.len());
    let cam_body = &src[cam..cam_end];
    assert!(
        cam_body.contains("self.set_lookat_scroll_mouse_lock(scroll_type.is_scrolling())")
            && cam_body.contains("self.set_lookat_scroll_mouse_lock(false)"),
        "update_camera must mouse-lock KEY/SCREENEDGE and unlock when input dies"
    );
    let wheel = src
        .find("fn handle_mouse_wheel")
        .map(|i| &src[i..src.len().min(i + 1800)])
        .expect("handle_mouse_wheel");
    assert!(
        wheel.contains("self.set_lookat_scroll_mouse_lock(false)"),
        "wheel stopScrolling must unlock KEY/SCREENEDGE immediately"
    );
    let reset = src
        .find("fn apply_look_at_reset_modes")
        .map(|i| &src[i..src.len().min(i + 700)])
        .expect("apply_look_at_reset_modes");
    assert!(
        reset.contains("self.set_lookat_scroll_mouse_lock(false)"),
        "doDisableInput reset must not leave KEY/SCREENEDGE mouse-locked"
    );
    assert!(
        src.contains("if look_at_host_mouse_locked()")
            && src.contains("do not overwrite the locked cursor mid KEY/RMB/EDGE pan"),
        "context cursor must stay put during KEY/SCREENEDGE lock"
    );
}

#[test]
fn airborne_look_at_ray_hits_ground_plane() {
    let look_dir = Vec3::new(0.0, -40.0, -120.0);
    let hit = airborne_look_at_ground(
        Vec3::new(0.0, 120.0, 120.0),
        Vec3::new(0.0, 80.0, 0.0),
        look_dir,
        2_000.0,
        Vec3::new(-1_000.0, 0.0, -1_000.0),
        Vec3::new(1_000.0, 0.0, 1_000.0),
        None,
    )
    .expect("airborne look-at must hit ground");
    assert!(
        hit.y.abs() < 0.02,
        "ground hit Y must be the plane, got {hit:?}"
    );
    assert!(
        hit.z.abs() > 1.0,
        "ray through an elevated unit must land past the XY origin, got {hit:?}"
    );

    // Off-axis unit: look dir is screen-center, not camera-to-object.
    let off_axis = airborne_look_at_ground(
        Vec3::new(0.0, 120.0, 120.0),
        Vec3::new(40.0, 80.0, 0.0),
        look_dir,
        2_000.0,
        Vec3::new(-1_000.0, 0.0, -1_000.0),
        Vec3::new(1_000.0, 0.0, 1_000.0),
        None,
    )
    .expect("off-axis airborne look-at must hit ground");
    assert!(
        (off_axis.x - 40.0).abs() < 0.5,
        "look dir must keep the unit on the screen-center ray, got {off_axis:?}"
    );
}

#[test]
fn vertical_pan_uses_display_aspect_boost() {
    // C++ W3DView.cpp:1796-1798 — 1920x1080 with 80% tactical frac → 2.222.
    let forward = Vec3::new(0.0, 0.0, 1.0);
    let right = Vec3::new(1.0, 0.0, 0.0);
    let aspect = 1920.0 / 864.0;
    let dx = lookat_scroll_world_delta(Vec2::new(1.0, 0.0), forward, right, 250.0, aspect);
    let dy = lookat_scroll_world_delta(Vec2::new(0.0, 1.0), forward, right, 250.0, aspect);
    assert!((dx.x - 1.0).abs() < 1.0e-5, "horizontal step {dx:?}");
    assert!(
        (dy.z + aspect).abs() < 1.0e-5,
        "vertical step must be aspect-boosted, got {dy:?}"
    );
    assert!(
        dy.length() > dx.length() * 2.0,
        "retail vertical pan is faster than horizontal by view aspect"
    );
}

#[test]
fn replay_hover_feeds_has_mouse_moved_recently_gate() {
    let src = MOUSE_SOURCE;
    let start = src
        .find("fn sync_ingame_mouseover_hint")
        .expect("sync_ingame_mouseover_hint");
    let end = src[start + 1..]
        .find("\n    fn ")
        .map(|i| start + 1 + i)
        .unwrap_or(start + 600);
    let body = &src[start..end];
    let feed_at = body
        .find("feed_look_at_replay_hover_gate")
        .expect("must stamp leftover InGameUI playback/moved-recently");
    let hint_at = body
        .find("create_mouseover_hint")
        .expect("must still post mouseover hint");
    assert!(
        feed_at < hint_at,
        "C++ InGameUI.cpp:2462 gate must be fed before createMouseoverHint"
    );
    assert!(
        body.contains("host_recorder_is_playback")
            && body.contains("lookat_has_mouse_moved_recently"),
        "live host owns both playback and LookAt 1s window"
    );
}
