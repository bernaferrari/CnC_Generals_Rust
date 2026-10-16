//! C++ W3DView::lookAt2341–2348 and isCameraMovementFinished2789–2802.
//! Exercise authored actions and the actual receiving Main condition driver.
use super::*;

fn authored(
    world: &mut GameLogic,
    foreign: &Arc<MissionScriptHooks>,
    actions: Vec<Box<ScriptAction>>,
) {
    let mut list = ScriptList::new();
    list.append_script(script("OwnedCameraSetup", actions, true));
    world.frame += 1;
    let _ = run(world, engine(Arc::clone(foreign), list));
}

fn independent_axes(world: &mut GameLogic, foreign: &Arc<MissionScriptHooks>) {
    authored(
        world,
        foreign,
        vec![
            reals(ScriptActionType::RotateCamera, &[0.75, 8.0, 0.2, 0.4]),
            reals(ScriptActionType::ZoomCamera, &[0.8, 3.0, 0.3, 0.5]),
            reals(ScriptActionType::PitchCamera, &[0.7, 4.0, 0.3, 0.5]),
            Box::new(ScriptAction::new(ScriptActionType::CameraModFreezeTime)),
        ],
    );
    assert_eq!(remaining_frames(world, true), 90);
    assert_eq!(remaining_frames(world, false), 121);
    assert!(world.is_script_camera_time_frozen());
}

/// A fresh real observer avoids assumptions about the interpreter's retained
/// per-script cadence. Its condition is evaluated at dt=0 after an explicitly
/// completed camera phase; clock advancement is tested separately below.
fn observer(world: &mut GameLogic, foreign: &Arc<MissionScriptHooks>) -> bool {
    let mut waiting = script(
        "ObserveOwnedCamera",
        vec![message("owned axes finished")],
        true,
    );
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(
        ConditionType::CameraMovementFinished,
    ))));
    waiting.condition = Some(Box::new(condition));
    let mut list = ScriptList::new();
    list.append_script(waiting);
    let before = world.new_script_messages.len();
    world.frame += 1;
    let _ = run(world, engine(Arc::clone(foreign), list));
    world.new_script_messages[before..]
        .iter()
        .any(|message| message == "Transmission: owned axes finished")
}

fn offer_frames(world: &mut GameLogic, frames: u32) {
    for _ in 0..frames {
        world.headless_camera_view_offer(33);
    }
}
fn remaining_frames(world: &GameLogic, zoom: bool) -> u32 {
    let c = &world.session_view.camera;
    let (target, duration, frame) = if zoom {
        (
            c.camera_zoom_target,
            c.camera_zoom_duration,
            c.camera_zoom_frame,
        )
    } else {
        (
            c.camera_pitch_target,
            c.camera_pitch_duration,
            c.camera_pitch_frame,
        )
    };
    if target.is_none() {
        0
    } else {
        (((duration * 1000.) as i32).max(1) / 33).max(1) as u32 - frame
    }
}
fn assert_axes(world: &GameLogic, zoom: u32, pitch: u32) {
    assert_eq!(
        remaining_frames(world, true),
        zoom,
        "LookAt must preserve independent zoom clock"
    );
    assert_eq!(
        remaining_frames(world, false),
        pitch,
        "LookAt must preserve independent pitch clock"
    );
    assert!(!world.session_view.scripted.motion.active);
    assert!(world.session_view.camera.camera_yaw_target.is_none());
    assert!(world.pending_camera_rotate.is_none());
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn actual_look_at_preserves_independent_zoom_pitch_and_unfinished_condition() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "actual_look_at_preserves_independent_zoom_pitch_and_unfinished_condition",
        || {
            let (mut owner, id, _) = world(110.0);
            let (peer, peer_id, _) = world(210.0);
            assert_eq!(
                id, peer_id,
                "ownership witness uses equal numeric object IDs"
            );
            let peer_before = peer.capture_camera_continuation().unwrap();
            independent_axes(&mut owner, &peer.mission_scripts);
            owner.request_player_camera_look_at(Vec3::new(400.0, 0.0, 500.0));
            assert_axes(&owner, 90, 121);
            assert!(owner.pending_camera_zoom.is_some());
            assert!(owner.pending_camera_pitch.is_some());
            assert!(
                owner.session_view.scripted.script_camera_freeze_time,
                "LookAt must preserve the stored freeze flag"
            );
            assert!(owner.is_script_camera_time_frozen());
            assert!(!observer(&mut owner, &peer.mission_scripts));
            offer_frames(&mut owner, 90);
            assert_axes(&owner, 0, 31);
            assert!(
                owner.is_script_camera_time_frozen(),
                "pitch still blocks after zoom finishes"
            );
            assert!(!observer(&mut owner, &peer.mission_scripts));
            offer_frames(&mut owner, 31);
            assert_axes(&owner, 0, 0);
            assert!(!owner.is_script_camera_time_frozen());
            assert!(observer(&mut owner, &peer.mission_scripts));
            assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
            assert!(peer.new_script_messages.is_empty());
            assert_camera_queues_empty(&peer.mission_scripts);
        },
    );
}

#[test]
#[cfg(all(not(target_arch = "wasm32"), feature = "game_client"))]
fn actual_motion_blur_completion_exception_does_not_erase_independent_axes() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "actual_motion_blur_completion_exception_does_not_erase_independent_axes",
        || {
            use game_client::display::view::{FilterMode, FilterType};
            let (mut owner, id, _) = world(110.0);
            *owner.world_services.terrain().write().unwrap() = camera_terrain();
            let (peer, peer_id, _) = world(210.0);
            assert_eq!(id, peer_id);
            let peer_before = peer.capture_camera_continuation().unwrap();
            independent_axes(&mut owner, &peer.mission_scripts);
            owner.request_player_camera_look_at(Vec3::new(400.0, 0.0, 500.0));
            let modes = [
                (
                    FilterMode::MBInAlpha,
                    bools(ScriptActionType::CameraMotionBlur, &[true, false]),
                ),
                (
                    FilterMode::MBOutAlpha,
                    bools(ScriptActionType::CameraMotionBlur, &[false, false]),
                ),
                (
                    FilterMode::MBInSaturate,
                    bools(ScriptActionType::CameraMotionBlur, &[true, true]),
                ),
                (
                    FilterMode::MBOutSaturate,
                    bools(ScriptActionType::CameraMotionBlur, &[false, true]),
                ),
                (
                    FilterMode::MBInAndOutAlpha,
                    action(
                        ScriptActionType::CameraMotionBlurJump,
                        vec![
                            Parameter::with_string(ParameterType::Waypoint, "LookPoint".into()),
                            Parameter::with_int(ParameterType::Boolean, 0),
                        ],
                    ),
                ),
                (
                    FilterMode::MBInAndOutSaturate,
                    action(
                        ScriptActionType::CameraMotionBlurJump,
                        vec![
                            Parameter::with_string(ParameterType::Waypoint, "LookPoint".into()),
                            Parameter::with_int(ParameterType::Boolean, 1),
                        ],
                    ),
                ),
            ];
            for (mode, blur) in modes {
                authored(&mut owner, &peer.mission_scripts, vec![blur]);
                assert_eq!(owner.session_view.filter.mode(), mode);
                assert_eq!(owner.session_view.filter.filter(), FilterType::MotionBlur);
                assert!(
                    observer(&mut owner, &peer.mission_scripts),
                    "original six-mode query exception: {mode:?}"
                );
                assert_axes(&owner, 90, 121);
                assert!(
                    owner.session_view.scripted.script_camera_freeze_time,
                    "query override must not erase stored freeze"
                );
                assert!(
                    !owner.is_script_camera_time_frozen(),
                    "effective camera freeze follows original query exception"
                );
                authored(
                    &mut owner,
                    &peer.mission_scripts,
                    vec![ints(ScriptActionType::CameraBwModeBegin, &[1])],
                );
                assert_eq!(
                    owner.session_view.filter.filter(),
                    FilterType::BlackAndWhite
                );
                assert!(
                    !observer(&mut owner, &peer.mission_scripts),
                    "BW must reveal the same unfinished axes after {mode:?}"
                );
                assert_axes(&owner, 90, 121);
                assert!(owner.is_script_camera_time_frozen());
                // There is no authored direct Null opcode. Admit the original BW
                // end command and one real filter fade tick instead of inventing one.
                authored(
                    &mut owner,
                    &peer.mission_scripts,
                    vec![ints(ScriptActionType::CameraBwModeEnd, &[1])],
                );
                owner.session_view.filter.tick_fade();
                assert_eq!(owner.session_view.filter.filter(), FilterType::Null);
                assert!(!observer(&mut owner, &peer.mission_scripts));
                assert_axes(&owner, 90, 121);
                assert!(owner.is_script_camera_time_frozen());
            }
            authored(
                &mut owner,
                &peer.mission_scripts,
                vec![ints(ScriptActionType::CameraMotionBlurFollow, &[23])],
            );
            assert_eq!(owner.session_view.filter.mode(), FilterMode::MBPanAlpha);
            assert!(
                !observer(&mut owner, &peer.mission_scripts),
                "pan is outside the original six exceptions"
            );
            assert!(owner.is_script_camera_time_frozen());
            assert_axes(&owner, 90, 121);
            assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
            assert!(peer.new_script_messages.is_empty());
            assert_camera_queues_empty(&peer.mission_scripts);
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn actual_look_at_encoded_camera_continuation_retains_axes_freeze_and_future_conditions() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "actual_look_at_encoded_camera_continuation_retains_axes_freeze_and_future_conditions",
        || {
            let (mut owner, _, _) = world(110.0);
            let (mut restored, _, _) = world(110.0);
            let (peer, _, _) = world(210.0);
            let peer_before = peer.capture_camera_continuation().unwrap();
            independent_axes(&mut owner, &peer.mission_scripts);
            owner.request_player_camera_look_at(Vec3::new(400., 0., 500.));
            offer_frames(&mut owner, 15);
            assert_axes(&owner, 75, 106);
            let encoded = owner.capture_camera_continuation().unwrap();
            restored.admit_camera_continuation(
                GameLogic::decode_camera_continuation(&encoded).unwrap(),
            );
            assert_eq!(restored.capture_camera_continuation().unwrap(), encoded);
            for (frames, zoom, pitch, finished) in
                [(0, 75, 106, false), (75, 0, 31, false), (31, 0, 0, true)]
            {
                offer_frames(&mut owner, frames);
                offer_frames(&mut restored, frames);
                assert_axes(&owner, zoom, pitch);
                assert_axes(&restored, zoom, pitch);
                assert_eq!(observer(&mut owner, &peer.mission_scripts), finished);
                assert_eq!(observer(&mut restored, &peer.mission_scripts), finished);
                assert_eq!(owner.is_script_camera_time_frozen(), !finished);
                assert_eq!(restored.is_script_camera_time_frozen(), !finished);
                assert_eq!(
                    owner.capture_camera_continuation().unwrap(),
                    restored.capture_camera_continuation().unwrap()
                );
            }
            assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
            assert_camera_queues_empty(&peer.mission_scripts);
        },
    );
}

fn receiving_jump_terrain(position: gamelogic::common::Coord3D, height: u8) -> TerrainLogic {
    let mut map = MapData::new();
    map.width = 64;
    map.height = 64;
    // TerrainLogic::load_map_data admits waypoints at the map ground height.
    // The action must preserve this admitted location, with no second sampler
    // from another world or invented authored-Z override.
    map.heightmap = vec![height; 64 * 64];
    map.waypoints.push(MapWaypoint {
        id: 2,
        name: "LookPoint".into(),
        location: position,
        path_label1: String::new(),
        path_label2: String::new(),
        path_label3: String::new(),
        bi_directional: false,
    });
    let mut terrain = TerrainLogic::new();
    terrain.load_map_data(map);
    terrain
}

#[test]
#[cfg(all(not(target_arch = "wasm32"), feature = "game_client"))]
fn actual_authored_blur_jump_uses_receiving_waypoint_height_and_missing_does_not_borrow_peer() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "actual_authored_blur_jump_uses_receiving_waypoint_height_and_missing_does_not_borrow_peer",
        || {
            use game_client::display::view::{FilterMode, FilterType, Point3, Vector2};
            let (mut owner, id, _) = world(110.0);
            let (mut peer, peer_id, _) = world(210.0);
            let (mut missing, missing_id, _) = world(310.0);
            assert_eq!(id, peer_id);
            assert_eq!(id, missing_id);
            *owner.world_services.terrain().write().unwrap() =
                receiving_jump_terrain(gamelogic::common::Coord3D::new(111., 222., 37.), 60);
            *peer.world_services.terrain().write().unwrap() =
                receiving_jump_terrain(gamelogic::common::Coord3D::new(511., 622., 87.), 140);
            *missing.world_services.terrain().write().unwrap() = TerrainLogic::new();
            let jump = || {
                action(
                    ScriptActionType::CameraMotionBlurJump,
                    vec![
                        Parameter::with_string(ParameterType::Waypoint, "LookPoint".into()),
                        Parameter::with_int(ParameterType::Boolean, 1),
                    ],
                )
            };
            let peer_before = peer.capture_camera_continuation().unwrap();
            authored(&mut owner, &peer.mission_scripts, vec![jump()]);
            let composite = owner.session_view.filter.composite(Vector2::zero());
            assert_eq!(composite.filter, FilterType::MotionBlur);
            assert_eq!(composite.mode, FilterMode::MBInAndOutSaturate);
            assert_eq!(
                composite.zoom_to,
                Some(Point3::new(111., 222., 37.5)),
                "Jump must use the receiving map-admitted waypoint height (60 * 0.625), not another world's terrain"
            );
            assert!(matches!(owner.pending_camera_motion_blur.last(),
            Some(CameraMotionBlurRequest::Jump { position, saturate: true })
            if *position == Vec3::new(111., 37.5, 222.)));
            assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
            let owner_before = owner.capture_camera_continuation().unwrap();
            authored(&mut peer, &owner.mission_scripts, vec![jump()]);
            assert_eq!(
                peer.session_view.filter.composite(Vector2::zero()).zoom_to,
                Some(Point3::new(511., 622., 87.5))
            );
            assert_eq!(owner.capture_camera_continuation().unwrap(), owner_before);
            let peer_admitted = peer.capture_camera_continuation().unwrap();
            authored(&mut missing, &peer.mission_scripts, vec![jump()]);
            assert_eq!(
                missing.session_view.filter.filter(),
                FilterType::Null,
                "authoritative missing waypoint must not fall back to another session or Core terrain"
            );
            assert!(missing.pending_camera_motion_blur.is_empty());
            assert_eq!(owner.capture_camera_continuation().unwrap(), owner_before);
            assert_eq!(peer.capture_camera_continuation().unwrap(), peer_admitted);
            // These are retained action-handler hooks, deliberately foreign to
            // each authored walk. The receiving driver must consume the action.
            assert_camera_queues_empty(&owner.mission_scripts);
            assert_camera_queues_empty(&peer.mission_scripts);
            assert_camera_queues_empty(&missing.mission_scripts);
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn actual_independent_axis_completion_retains_freeze_for_next_authored_motion_and_encoded_restore()
{
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "actual_independent_axis_completion_retains_freeze_for_next_authored_motion_and_encoded_restore",
        || {
            let (mut owner, _, _) = world(110.0);
            let (mut restored, _, _) = world(110.0);
            let (peer, _, _) = world(210.0);
            let peer_before = peer.capture_camera_continuation().unwrap();
            authored(
                &mut owner,
                &peer.mission_scripts,
                vec![
                    reals(ScriptActionType::ZoomCamera, &[0.8, 3., 0.3, 0.5]),
                    reals(ScriptActionType::PitchCamera, &[0.7, 4., 0.3, 0.5]),
                    Box::new(ScriptAction::new(ScriptActionType::CameraModFreezeTime)),
                ],
            );
            assert!(owner.is_script_camera_time_frozen());
            offer_frames(&mut owner, 121);
            assert_eq!(remaining_frames(&owner, true), 0);
            assert_eq!(remaining_frames(&owner, false), 0);
            assert!(observer(&mut owner, &peer.mission_scripts));
            assert!(
                !owner.is_script_camera_time_frozen(),
                "finished independent axes release effective freeze"
            );
            assert!(
                owner.session_view.scripted.script_camera_freeze_time,
                "CPP zoom/pitch completion does not clear m_freezeTimeForCameraMovement"
            );
            let encoded = owner.capture_camera_continuation().unwrap();
            restored.admit_camera_continuation(
                GameLogic::decode_camera_continuation(&encoded).unwrap(),
            );
            assert!(restored.session_view.scripted.script_camera_freeze_time);
            assert!(!restored.is_script_camera_time_frozen());
            for world in [&mut owner, &mut restored] {
                authored(
                    world,
                    &peer.mission_scripts,
                    vec![reals(ScriptActionType::ZoomCamera, &[0.6, 2., 0., 0.])],
                );
                assert_eq!(remaining_frames(world, true), 60);
                assert!(world.session_view.scripted.script_camera_freeze_time);
                assert!(
                    world.is_script_camera_time_frozen(),
                    "new zoom reuses stored freeze without another freeze opcode"
                );
                assert!(!observer(world, &peer.mission_scripts));
            }
            assert_eq!(
                owner.capture_camera_continuation().unwrap(),
                restored.capture_camera_continuation().unwrap()
            );
            assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
            assert_camera_queues_empty(&peer.mission_scripts);
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn actual_rotate_completion_clears_stored_freeze_while_independent_axes_remain_unfinished() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "actual_rotate_completion_clears_stored_freeze_while_independent_axes_remain_unfinished",
        || {
            let (mut owner, _, _) = world(110.0);
            let (peer, _, _) = world(210.0);
            let peer_before = peer.capture_camera_continuation().unwrap();
            authored(
                &mut owner,
                &peer.mission_scripts,
                vec![
                    reals(ScriptActionType::RotateCamera, &[0.75, 1., 0., 0.]),
                    reals(ScriptActionType::ZoomCamera, &[0.8, 3., 0., 0.]),
                    reals(ScriptActionType::PitchCamera, &[0.7, 4., 0., 0.]),
                    Box::new(ScriptAction::new(ScriptActionType::CameraModFreezeTime)),
                ],
            );
            assert!(owner.is_script_camera_time_frozen());
            offer_frames(&mut owner, 30);
            assert_eq!(owner.session_view.camera.camera_yaw_target, None);
            assert_eq!(remaining_frames(&owner, true), 60);
            assert_eq!(remaining_frames(&owner, false), 91);
            assert!(
                !owner.session_view.scripted.script_camera_freeze_time,
                "CPP rotate terminal branch clears the stored freeze even with independent axes"
            );
            assert!(!owner.is_script_camera_time_frozen());
            assert!(
                !observer(&mut owner, &peer.mission_scripts),
                "completion query must still see unfinished independent axes after freeze clears"
            );
            assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
            assert_camera_queues_empty(&peer.mission_scripts);
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn actual_simple_move_completion_clears_stored_freeze_while_independent_axes_remain_unfinished() {
    super::super::sequential_actor_tests::isolated(
        module_path!(),
        "actual_simple_move_completion_clears_stored_freeze_while_independent_axes_remain_unfinished",
        || {
            let (mut owner, _, _) = world(110.0);
            let (peer, _, _) = world(210.0);
            let peer_before = peer.capture_camera_continuation().unwrap();
            authored(
                &mut owner,
                &peer.mission_scripts,
                vec![
                    action(
                        ScriptActionType::MoveCameraTo,
                        vec![
                            Parameter::with_coord(
                                ParameterType::Coord3D,
                                gamelogic::common::Coord3D::new(400., 500., 20.),
                            ),
                            Parameter::with_real(ParameterType::Real, 1.),
                            Parameter::with_real(ParameterType::Real, 0.),
                            Parameter::with_real(ParameterType::Real, 0.),
                            Parameter::with_real(ParameterType::Real, 0.),
                        ],
                    ),
                    reals(ScriptActionType::ZoomCamera, &[0.8, 3., 0., 0.]),
                    reals(ScriptActionType::PitchCamera, &[0.7, 4., 0., 0.]),
                    Box::new(ScriptAction::new(ScriptActionType::CameraModFreezeTime)),
                ],
            );
            assert!(owner.session_view.scripted.motion.active);
            assert!(owner.is_script_camera_time_frozen());
            offer_frames(&mut owner, 30);
            // The next33ms source offer crosses the strict1000ms deadline;
            // independent scalar axes receive that same admitted offer.
            offer_frames(&mut owner, 1);
            assert!(!owner.session_view.scripted.motion.active);
            assert_eq!(remaining_frames(&owner, true), 59);
            assert_eq!(remaining_frames(&owner, false), 90);
            assert!(
                !owner.session_view.scripted.script_camera_freeze_time,
                "CPP path terminal branch clears stored freeze independently of zoom/pitch"
            );
            assert!(!owner.is_script_camera_time_frozen());
            assert!(!observer(&mut owner, &peer.mission_scripts));
            assert_eq!(peer.capture_camera_continuation().unwrap(), peer_before);
            assert_camera_queues_empty(&peer.mission_scripts);
        },
    );
}
