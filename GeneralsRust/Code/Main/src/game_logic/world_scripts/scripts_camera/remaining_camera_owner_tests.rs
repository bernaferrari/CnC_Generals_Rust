//! Draft OLD/GREEN Main camera-owner regressions for hq-00m35.
//! Intended inclusion: game_logic/world_scripts/scripts_camera/remaining_camera_owner_tests.rs.
//! Test APIs use the existing Main fixture and current ScriptActionType surface;
//! they are expected to fail OLD because remaining callbacks use the foreign
//! ScriptEngine-retained MissionScriptActionHandler.
use super::*;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::{ScriptEngine, get_script_engine};
use gamelogic::system::map_loader::{MapData, MapWaypoint};
use gamelogic::terrain::TerrainLogic;

struct TerrainRestore(Option<TerrainLogic>);
impl Drop for TerrainRestore {
    fn drop(&mut self) {
        if let Some(previous) = self.0.take() {
            *gamelogic::terrain::get_terrain_logic().write().unwrap() = previous;
        }
    }
}

fn camera_terrain() -> TerrainLogic {
    let mut map = MapData::new();
    map.width = 64;
    map.height = 64;
    map.heightmap = vec![16; 64 * 64];
    for (id, name, x, y, z) in [
        (1, "CameraPoint", 310.0, 330.0, 0.0),
        (2, "LookPoint", 410.0, 430.0, 0.0),
    ] {
        map.waypoints.push(MapWaypoint {
            id,
            name: name.into(),
            location: gamelogic::common::Coord3D::new(x, y, z),
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

fn world(x: f32) -> (GameLogic, ObjectId, Vec3) {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "CameraOwner", true));
    let mut unit = ThingTemplate::new("CameraOwnerUnit");
    unit.add_kind_of(KindOf::Selectable).set_health(100.0);
    world.templates.insert("CameraOwnerUnit".into(), unit);
    let mut home = ThingTemplate::new("AmericaCommandCenter");
    home.add_kind_of(KindOf::Structure).set_health(100.0);
    world.templates.insert("AmericaCommandCenter".into(), home);
    let id = world
        .create_object_for_player("CameraOwnerUnit", 0, Vec3::new(x, 0.0, x + 20.0))
        .unwrap();
    world.host_object_mut(id).unwrap().name = "FocusScout".into();
    world.select_objects(0, vec![id]);
    let home = Vec3::new(x + 40.0, 0.0, x + 60.0);
    world
        .create_object_for_player("AmericaCommandCenter", 0, home)
        .unwrap();
    (world, id, home)
}

fn action(kind: ScriptActionType, params: Vec<Parameter>) -> Box<ScriptAction> {
    let mut action = ScriptAction::new(kind);
    for param in params {
        action.add_parameter(param).unwrap();
    }
    Box::new(action)
}
fn s(kind: ScriptActionType, value: &str, ty: ParameterType) -> Box<ScriptAction> {
    action(kind, vec![Parameter::with_string(ty, value.into())])
}
fn reals(kind: ScriptActionType, values: &[f32]) -> Box<ScriptAction> {
    action(
        kind,
        values
            .iter()
            .map(|value| Parameter::with_real(ParameterType::Real, *value))
            .collect(),
    )
}
fn ints(kind: ScriptActionType, values: &[i32]) -> Box<ScriptAction> {
    action(
        kind,
        values
            .iter()
            .map(|value| Parameter::with_int(ParameterType::Int, *value))
            .collect(),
    )
}
fn bools(kind: ScriptActionType, values: &[bool]) -> Box<ScriptAction> {
    action(
        kind,
        values
            .iter()
            .map(|value| Parameter::with_int(ParameterType::Boolean, i32::from(*value)))
            .collect(),
    )
}
fn chain(mut actions: Vec<Box<ScriptAction>>) -> Option<Box<ScriptAction>> {
    let mut next = None;
    while let Some(mut action) = actions.pop() {
        action.next_action = next;
        next = Some(action);
    }
    next
}
fn always_true() -> Option<Box<OrCondition>> {
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    Some(Box::new(condition))
}
fn script(name: &str, actions: Vec<Box<ScriptAction>>, one_shot: bool) -> Box<Script> {
    let mut script = Script::new();
    script.script_name = name.into();
    script.condition = always_true();
    script.action = chain(actions);
    script.is_one_shot = one_shot;
    Box::new(script)
}
fn engine(hooks: Arc<MissionScriptHooks>, list: ScriptList) -> ScriptEngine {
    let mut engine = ScriptEngine::new().unwrap();
    engine.set_action_handler(Some(Arc::new(MissionScriptActionHandler::new(hooks))));
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    engine
}
fn run(world: &mut GameLogic, engine: ScriptEngine) -> ScriptEngine {
    run_with_dt(world, engine, 0.0)
}
fn run_with_dt(world: &mut GameLogic, engine: ScriptEngine, dt: f32) -> ScriptEngine {
    *get_script_engine().write().unwrap() = Some(engine);
    world.scripts_loaded = true;
    world.evaluate_and_execute_scripts(dt);
    get_script_engine().write().unwrap().take().unwrap()
}
fn assert_camera_queues_empty(hooks: &MissionScriptHooks) {
    assert!(hooks.drain_camera_moves().is_empty());
    assert!(hooks.drain_camera_follows().is_empty());
    assert!(hooks.drain_camera_tethers().is_empty());
    assert!(hooks.drain_camera_path_moves().is_empty());
    assert!(hooks.drain_camera_move_to().is_empty());
    assert!(hooks.drain_camera_move_to_selection_requests().is_empty());
    assert!(hooks.drain_camera_move_home_requests().is_empty());
    assert!(hooks.drain_camera_zoom_requests().is_empty());
    assert!(hooks.drain_camera_pitch_requests().is_empty());
    assert!(hooks.drain_camera_rotate_requests().is_empty());
    assert!(hooks.drain_camera_mod_final_zoom_requests().is_empty());
    assert!(hooks.drain_camera_mod_final_pitch_requests().is_empty());
    assert!(hooks.drain_camera_mod_freeze_time_requests().is_empty());
    assert!(hooks.drain_camera_mod_freeze_angle_requests().is_empty());
    assert!(
        hooks
            .drain_camera_mod_final_speed_multiplier_requests()
            .is_empty()
    );
    assert!(hooks.drain_camera_mod_rolling_average_requests().is_empty());
    assert!(hooks.drain_camera_resets().is_empty());
    assert!(hooks.drain_camera_setup_requests().is_empty());
    assert!(hooks.drain_camera_look_toward_object_requests().is_empty());
    assert!(
        hooks
            .drain_camera_look_toward_waypoint_requests()
            .is_empty()
    );
    assert!(hooks.drain_camera_mod_look_toward_requests().is_empty());
    assert!(
        hooks
            .drain_camera_mod_final_look_toward_requests()
            .is_empty()
    );
    assert!(hooks.drain_camera_set_default_requests().is_empty());
    assert!(hooks.drain_camera_slave_mode_enable_requests().is_empty());
    assert!(hooks.drain_camera_slave_mode_disable_requests().is_empty());
    assert!(hooks.drain_letterbox_events().is_empty());
    assert!(hooks.drain_camera_add_shaker_requests().is_empty());
    assert!(hooks.drain_screen_shake_requests().is_empty());
    assert!(hooks.drain_view_guardband_requests().is_empty());
    assert!(hooks.drain_camera_bw_mode_requests().is_empty());
    assert!(hooks.drain_skybox_enabled_updates().is_empty());
    assert!(hooks.drain_camera_motion_blur_requests().is_empty());
}

fn remaining_camera_actions(x: f32) -> Vec<Box<ScriptAction>> {
    let move_to = action(
        ScriptActionType::MoveCameraTo,
        vec![
            Parameter::with_coord(
                ParameterType::Coord3D,
                gamelogic::common::Coord3D::new(x + 5.0, x + 25.0, 90.0),
            ),
            Parameter::with_real(ParameterType::Real, 0.0),
            Parameter::with_real(ParameterType::Real, 0.0),
            Parameter::with_real(ParameterType::Real, 0.0),
            Parameter::with_real(ParameterType::Real, 0.0),
        ],
    );
    vec![
        move_to,
        action(
            ScriptActionType::MoveCameraAlongWaypointPath,
            vec![
                Parameter::with_string(ParameterType::WaypointPath, "CameraPoint".into()),
                Parameter::with_real(ParameterType::Real, 2.0),
                Parameter::with_real(ParameterType::Real, 0.4),
                Parameter::with_real(ParameterType::Real, 0.3),
                Parameter::with_real(ParameterType::Real, 0.6),
            ],
        ),
        reals(ScriptActionType::RotateCamera, &[0.75, 3.0, 0.2, 0.4]),
        Box::new(ScriptAction::new(ScriptActionType::MoveCameraToSelection)),
        Box::new(ScriptAction::new(ScriptActionType::CameraMoveHome)),
        reals(ScriptActionType::ZoomCamera, &[0.8, 1.5, 0.3, 0.5]),
        reals(ScriptActionType::PitchCamera, &[0.7, 1.5, 0.3, 0.5]),
        Box::new(ScriptAction::new(ScriptActionType::CameraModFreezeTime)),
        Box::new(ScriptAction::new(ScriptActionType::CameraModFreezeAngle)),
        reals(ScriptActionType::CameraModSetFinalZoom, &[0.9, 0.2, 0.4]),
        reals(ScriptActionType::CameraModSetFinalPitch, &[0.6, 0.2, 0.4]),
        ints(ScriptActionType::CameraModSetFinalSpeedMultiplier, &[5]),
        ints(ScriptActionType::CameraModSetRollingAverage, &[9]),
        action(
            ScriptActionType::CameraFollowNamed,
            vec![
                Parameter::with_string(ParameterType::Unit, "FocusScout".into()),
                Parameter::with_int(ParameterType::Boolean, 1),
            ],
        ),
        Box::new(ScriptAction::new(ScriptActionType::CameraStopFollow)),
        action(
            ScriptActionType::CameraTetherNamed,
            vec![
                Parameter::with_string(ParameterType::Unit, "FocusScout".into()),
                Parameter::with_int(ParameterType::Boolean, 1),
                Parameter::with_real(ParameterType::Real, 1.75),
            ],
        ),
        Box::new(ScriptAction::new(ScriptActionType::CameraStopTetherNamed)),
        action(
            ScriptActionType::ResetCamera,
            vec![
                Parameter::with_string(ParameterType::Waypoint, "CameraPoint".into()),
                Parameter::with_real(ParameterType::Real, 2.5),
                Parameter::with_real(ParameterType::Real, 0.25),
                Parameter::with_real(ParameterType::Real, 0.75),
            ],
        ),
        action(
            ScriptActionType::SetupCamera,
            vec![
                Parameter::with_string(ParameterType::Waypoint, "CameraPoint".into()),
                Parameter::with_real(ParameterType::Real, 0.8),
                Parameter::with_real(ParameterType::Real, 0.6),
                Parameter::with_string(ParameterType::Waypoint, "LookPoint".into()),
            ],
        ),
        action(
            ScriptActionType::CameraLookTowardObject,
            vec![
                Parameter::with_string(ParameterType::Unit, "FocusScout".into()),
                Parameter::with_real(ParameterType::Real, 1.0),
                Parameter::with_real(ParameterType::Real, 2.0),
                Parameter::with_real(ParameterType::Real, 0.3),
                Parameter::with_real(ParameterType::Real, 0.4),
            ],
        ),
        action(
            ScriptActionType::CameraLookTowardWaypoint,
            vec![
                Parameter::with_string(ParameterType::Waypoint, "LookPoint".into()),
                Parameter::with_real(ParameterType::Real, 1.1),
                Parameter::with_real(ParameterType::Real, 0.2),
                Parameter::with_real(ParameterType::Real, 0.3),
                Parameter::with_int(ParameterType::Boolean, 1),
            ],
        ),
        s(
            ScriptActionType::CameraModLookToward,
            "LookPoint",
            ParameterType::Waypoint,
        ),
        s(
            ScriptActionType::CameraModFinalLookToward,
            "LookPoint",
            ParameterType::Waypoint,
        ),
        reals(ScriptActionType::CameraSetDefault, &[0.45, 1.25, 700.0]),
        action(
            ScriptActionType::CameraEnableSlaveMode,
            vec![
                Parameter::with_string(ParameterType::TextString, "CameraOwnerUnit".into()),
                Parameter::with_string(ParameterType::TextString, "Turret".into()),
            ],
        ),
        Box::new(ScriptAction::new(ScriptActionType::CameraDisableSlaveMode)),
        Box::new(ScriptAction::new(ScriptActionType::CameraLetterboxBegin)),
        Box::new(ScriptAction::new(ScriptActionType::CameraLetterboxEnd)),
        ints(ScriptActionType::ScreenShake, &[2]),
        action(
            ScriptActionType::CameraAddShakerAt,
            vec![
                Parameter::with_string(ParameterType::Waypoint, "CameraPoint".into()),
                Parameter::with_real(ParameterType::Real, 3.0),
                Parameter::with_real(ParameterType::Real, 1.5),
                Parameter::with_real(ParameterType::Real, 200.0),
            ],
        ),
        reals(ScriptActionType::ResizeViewGuardband, &[0.12, -0.08]),
        ints(ScriptActionType::CameraBwModeBegin, &[17]),
        ints(ScriptActionType::CameraBwModeEnd, &[19]),
        Box::new(ScriptAction::new(ScriptActionType::DrawSkyboxBegin)),
        Box::new(ScriptAction::new(ScriptActionType::DrawSkyboxEnd)),
        bools(ScriptActionType::CameraMotionBlur, &[true, false]),
        action(
            ScriptActionType::CameraMotionBlurJump,
            vec![
                Parameter::with_string(ParameterType::Waypoint, "LookPoint".into()),
                Parameter::with_int(ParameterType::Boolean, 1),
            ],
        ),
        ints(ScriptActionType::CameraMotionBlurFollow, &[23]),
        Box::new(ScriptAction::new(
            ScriptActionType::CameraMotionBlurEndFollow,
        )),
    ]
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn actual_main_all_remaining_camera_view_callbacks_ignore_foreign_retained_handlers() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "actual_main_all_remaining_camera_view_callbacks_ignore_foreign_retained_handlers",
        || {
            let terrain = camera_terrain();
            let _terrain = TerrainRestore(Some(std::mem::replace(
                &mut *gamelogic::terrain::get_terrain_logic().write().unwrap(),
                terrain,
            )));
            let (mut first, first_id, _) = world(110.0);
            let (mut second, second_id, _) = world(210.0);
            assert_eq!(first_id, second_id);
            let first_engine = engine(second.mission_scripts.clone(), {
                let mut list = ScriptList::new();
                list.append_script(script("SameCamera", remaining_camera_actions(100.0), true));
                list
            });
            let second_engine = engine(first.mission_scripts.clone(), {
                let mut list = ScriptList::new();
                list.append_script(script("SameCamera", remaining_camera_actions(200.0), true));
                list
            });

            run(&mut first, first_engine);
            run(&mut second, second_engine);

            // Every request must be translated into its driving world's effect state;
            // the deliberately foreign Arc handler must receive nothing.
            assert_camera_queues_empty(&first.mission_scripts);
            assert_camera_queues_empty(&second.mission_scripts);
            assert!(first.pending_view_guardband.is_some());
            assert!(second.pending_view_guardband.is_some());
            assert!(first.pending_camera_bw_mode.is_some());
            assert!(second.pending_camera_bw_mode.is_some());
            assert!(first.pending_camera_motion_blur.len() >= 4);
            assert!(second.pending_camera_motion_blur.len() >= 4);
            assert!(!first.cinematic_letterbox);
            assert!(!second.cinematic_letterbox);
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn actual_main_nested_tether_then_home_is_flushed_before_parent_continues() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "actual_main_nested_tether_then_home_is_flushed_before_parent_continues",
        || {
            let (mut first, id, home) = world(110.0);
            let (mut second, second_id, second_home) = world(210.0);
            assert_eq!(id, second_id);
            let mut list = ScriptList::new();
            let mut sub = script(
                "CameraSub",
                vec![
                    action(
                        ScriptActionType::CameraTetherNamed,
                        vec![
                            Parameter::with_string(ParameterType::Unit, "FocusScout".into()),
                            Parameter::with_int(ParameterType::Boolean, 1),
                            Parameter::with_real(ParameterType::Real, 12.0),
                        ],
                    ),
                    Box::new(ScriptAction::new(ScriptActionType::CameraMoveHome)),
                ],
                true,
            );
            sub.is_subroutine = true;
            let caller = script(
                "SameCamera",
                vec![
                    action(
                        ScriptActionType::CallSubroutine,
                        vec![Parameter::with_string(
                            ParameterType::Script,
                            "CameraSub".into(),
                        )],
                    ),
                    Box::new(ScriptAction::new(ScriptActionType::CameraStopFollow)),
                ],
                true,
            );
            list.append_script(caller);
            list.append_script(sub);
            let first_foreign = second.mission_scripts.clone();
            let second_foreign = first.mission_scripts.clone();
            run(&mut first, engine(first_foreign.clone(), list.clone()));
            run(&mut second, engine(second_foreign.clone(), list));

            assert_eq!(first.camera_follow_object_id(), None);
            assert_eq!(first.take_camera_focus_request(), Some(home));
            assert_eq!(second.camera_follow_object_id(), None);
            assert_eq!(second.take_camera_focus_request(), Some(second_home));
            assert!(first_foreign.drain_camera_tethers().is_empty());
            assert!(first_foreign.drain_camera_move_home_requests().is_empty());
            assert!(second_foreign.drain_camera_tethers().is_empty());
            assert!(second_foreign.drain_camera_move_home_requests().is_empty());
            assert!(first.mission_scripts.drain_camera_tethers().is_empty());
            assert!(second.mission_scripts.drain_camera_tethers().is_empty());
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn actual_main_camera_movement_finished_queries_the_driving_world() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "actual_main_camera_movement_finished_queries_the_driving_world",
        || {
            let (mut first, _, _) = world(110.0);
            let (mut second, _, _) = world(210.0);
            first.mission_scripts.set_camera_movement_finished(false);
            // The first world must not borrow this foreign true value.
            second.mission_scripts.set_camera_movement_finished(true);

            let make_engine = |foreign: Arc<MissionScriptHooks>| {
                let mover = script(
                    "SameCamera",
                    vec![action(
                        ScriptActionType::MoveCameraTo,
                        vec![
                            Parameter::with_coord(
                                ParameterType::Coord3D,
                                gamelogic::common::Coord3D::new(400.0, 0.0, 500.0),
                            ),
                            Parameter::with_real(ParameterType::Real, 3.0),
                            Parameter::with_real(ParameterType::Real, 0.0),
                            Parameter::with_real(ParameterType::Real, 0.0),
                            Parameter::with_real(ParameterType::Real, 0.0),
                        ],
                    )],
                    true,
                );
                let mut observer = script("WaitCamera", vec![message("finished")], true);
                let mut condition = OrCondition::new();
                condition.set_first_and_condition(Some(Box::new(Condition::new(
                    ConditionType::CameraMovementFinished,
                ))));
                observer.condition = Some(Box::new(condition));
                let mut list = ScriptList::new();
                list.append_script(mover);
                list.append_script(observer);
                engine(foreign, list)
            };
            let first_engine = make_engine(second.mission_scripts.clone());
            let second_engine = make_engine(first.mission_scripts.clone());

            let first_engine = run(&mut first, first_engine);
            run(&mut second, second_engine);

            assert!(first.new_script_messages.is_empty());
            assert!(second.new_script_messages.is_empty());
            assert!(!first.mission_scripts.is_camera_movement_finished());
            assert!(!second.mission_scripts.is_camera_movement_finished());
            first.frame = 105;
            let first_engine = run_with_dt(&mut first, first_engine, 3.5);
            assert!(first.new_script_messages.is_empty());
            first.frame = 106;
            let _ = run(&mut first, first_engine);
            assert_eq!(
                first.new_script_messages,
                vec!["Transmission: finished".to_string()]
            );
            assert!(second.new_script_messages.is_empty());
        },
    );
}

fn message(text: &str) -> Box<ScriptAction> {
    action(
        ScriptActionType::DisplayText,
        vec![Parameter::with_string(
            ParameterType::TextString,
            text.into(),
        )],
    )
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn standalone_camera_callback_keeps_retained_handler_fallback() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "standalone_camera_callback_keeps_retained_handler_fallback",
        || {
            let hooks = MissionScriptHooks::new();
            let mut list = ScriptList::new();
            list.append_script(script(
                "StandaloneCamera",
                vec![reals(
                    ScriptActionType::CameraSetDefault,
                    &[0.45, 1.25, 700.0],
                )],
                true,
            ));
            let mut standalone = engine(hooks.clone(), list);
            standalone.update().unwrap();
            let requests = hooks.drain_camera_set_default_requests();
            assert_eq!(requests.len(), 1);
            assert_eq!(requests[0].pitch, 0.45);
            assert_eq!(requests[0].angle, 1.25);
            assert_eq!(requests[0].max_height, 700.0);
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn standalone_motion_blur_jump_error_keeps_same_handler_move_fallback() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct FallbackProbe(AtomicUsize);
    impl gamelogic::scripting::engine::ScriptActionHandler for FallbackProbe {
        fn camera_motion_blur_jump(
            &self,
            _x: f32,
            _y: f32,
            _z: f32,
            _saturate: bool,
        ) -> gamelogic::GameLogicResult<()> {
            Err(gamelogic::GameLogicError::Configuration(
                "probe camera backend error".into(),
            ))
        }

        fn move_camera_to(
            &self,
            _x: f32,
            _y: f32,
            _z: f32,
            _seconds: f32,
            _stutter: f32,
            _ease_in: f32,
            _ease_out: f32,
        ) -> gamelogic::GameLogicResult<()> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
    }

    super::sequential_actor_tests::isolated(
        module_path!(),
        "standalone_motion_blur_jump_error_keeps_same_handler_move_fallback",
        || {
            let terrain = camera_terrain();
            let _terrain = TerrainRestore(Some(std::mem::replace(
                &mut *gamelogic::terrain::get_terrain_logic().write().unwrap(),
                terrain,
            )));
            let probe = Arc::new(FallbackProbe(AtomicUsize::new(0)));
            let mut list = ScriptList::new();
            list.append_script(script(
                "StandaloneBlurJump",
                vec![action(
                    ScriptActionType::CameraMotionBlurJump,
                    vec![
                        Parameter::with_string(ParameterType::Waypoint, "LookPoint".into()),
                        Parameter::with_int(ParameterType::Boolean, 1),
                    ],
                )],
                true,
            ));
            let mut standalone = ScriptEngine::new().unwrap();
            standalone.set_action_handler(Some(probe.clone()));
            standalone
                .set_script_list_for_player(0, Some(Box::new(list)))
                .unwrap();
            standalone.update().unwrap();
            assert_eq!(probe.0.load(Ordering::Relaxed), 1);
        },
    );
}

fn assert_remaining_effect(world: &GameLogic, kind: ScriptActionType, id: ObjectId) {
    use ScriptActionType::*;
    let camera_point = Vec3::new(310.0, 10.0, 330.0);
    let look_point = Vec3::new(410.0, 10.0, 430.0);
    match kind {
        CameraFollowNamed => {
            assert_eq!(world.camera_follow_target, Some(id));
            assert_eq!(
                world.peek_pending_camera_focus(),
                Some(world.objects.get(&id).unwrap().get_position())
            );
        }
        CameraTetherNamed => {
            assert_eq!(world.camera_follow_target, Some(id));
            assert_eq!(world.camera_tether_play, Some(1.75));
        }
        CameraStopFollow | CameraStopTetherNamed => {
            assert_eq!(world.camera_follow_target, None);
            assert_eq!(world.camera_tether_play, None);
        }
        ResetCamera => {
            let r = world.script_camera_move_to.as_ref().unwrap();
            assert_eq!((r.target, r.total_time_seconds), (camera_point, 2.5));
            assert_eq!(
                (
                    world.pending_camera_zoom_reset_duration,
                    world.pending_camera_zoom_reset_ease_in,
                    world.pending_camera_zoom_reset_ease_out
                ),
                (2.5, 0.25, 0.75)
            );
        }
        SetupCamera => {
            assert_eq!(world.peek_pending_camera_focus(), Some(camera_point));
            assert_eq!(
                world.pending_camera_zoom.as_ref().unwrap().zoom,
                0.8 * ((320.0 + 300.0) / 320.0)
            );
            assert_eq!(world.pending_camera_pitch.as_ref().unwrap().pitch, 0.6);
            assert_eq!(
                world.pending_camera_look_toward.as_ref().unwrap().position,
                look_point
            );
        }
        CameraLookTowardObject => {
            let r = world.pending_camera_look_toward.as_ref().unwrap();
            assert_eq!(r.position, world.objects.get(&id).unwrap().get_position());
            assert_eq!(
                (r.duration_seconds, r.ease_in_seconds, r.ease_out_seconds),
                (1.0, 0.3, 0.4)
            );
            assert_eq!(
                (
                    world.script_look_toward_object_id,
                    world.script_look_toward_hold_seconds
                ),
                (Some(id.0), 2.0)
            );
        }
        CameraLookTowardWaypoint => {
            let r = world.pending_camera_look_toward.as_ref().unwrap();
            assert_eq!(
                (
                    r.position,
                    r.duration_seconds,
                    r.ease_in_seconds,
                    r.ease_out_seconds,
                    r.reverse_rotation
                ),
                (look_point, 1.1, 0.2, 0.3, true)
            );
        }
        CameraModLookToward | CameraModFinalLookToward => assert_eq!(
            world.pending_camera_look_toward.as_ref().unwrap().position,
            look_point
        ),
        CameraSetDefault => assert_eq!(
            (
                world.script_default_camera_pitch,
                world.script_default_camera_angle,
                world.script_default_camera_max_height
            ),
            (0.45, 0.0, 700.0)
        ),
        CameraEnableSlaveMode => {
            let r = world.pending_camera_slave_mode_enable.as_ref().unwrap();
            assert_eq!(
                (r.thing_template_name.as_str(), r.bone_name.as_str()),
                ("CameraOwnerUnit", "Turret")
            );
            assert!(!world.pending_camera_slave_mode_disable);
        }
        CameraDisableSlaveMode => {
            assert!(world.pending_camera_slave_mode_enable.is_none());
            assert!(world.pending_camera_slave_mode_disable);
        }
        CameraLetterboxBegin => assert!(world.cinematic_letterbox),
        CameraLetterboxEnd => assert!(!world.cinematic_letterbox),
        ScreenShake => assert_eq!(world.pending_screen_shakes.last().unwrap().intensity, 2),
        CameraAddShakerAt => {
            let r = world.pending_camera_add_shakers.last().unwrap();
            assert_eq!(
                (r.position, r.amplitude, r.duration_seconds, r.radius),
                (camera_point, 3.0, 1.5, 200.0)
            );
        }
        ResizeViewGuardband => {
            let r = world.pending_view_guardband.as_ref().unwrap();
            assert_eq!((r.x_bias, r.y_bias), (0.12, -0.08));
        }
        CameraBwModeBegin | CameraBwModeEnd => {
            let r = world.pending_camera_bw_mode.as_ref().unwrap();
            assert_eq!(
                (r.enabled, r.frames),
                if kind == CameraBwModeBegin {
                    (true, 17)
                } else {
                    (false, 19)
                }
            );
        }
        DrawSkyboxBegin | DrawSkyboxEnd => {
            assert_eq!(world.script_skybox_enabled, kind == DrawSkyboxBegin)
        }
        CameraMotionBlur => assert!(matches!(
            world.pending_camera_motion_blur.last(),
            Some(CameraMotionBlurRequest::Basic {
                zoom_in: true,
                saturate: false
            })
        )),
        CameraMotionBlurJump => assert!(
            matches!(world.pending_camera_motion_blur.last(),Some(CameraMotionBlurRequest::Jump { position,saturate:true }) if *position==look_point)
        ),
        CameraMotionBlurFollow => assert!(matches!(
            world.pending_camera_motion_blur.last(),
            Some(CameraMotionBlurRequest::Follow { amount: 23 })
        )),
        CameraMotionBlurEndFollow => assert!(matches!(
            world.pending_camera_motion_blur.last(),
            Some(CameraMotionBlurRequest::EndFollow)
        )),
        _ => panic!("missing witness {kind:?}"),
    }
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn actual_main_each_remaining_camera_action_preserves_its_own_effect() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "actual_main_each_remaining_camera_action_preserves_its_own_effect",
        || {
            let _terrain = TerrainRestore(Some(std::mem::replace(
                &mut *gamelogic::terrain::get_terrain_logic().write().unwrap(),
                camera_terrain(),
            )));
            for action in remaining_camera_actions(100.0).into_iter().skip(13) {
                let kind = action.action_type;
                let (mut first, id, _) = world(110.0);
                let (mut second, other_id, _) = world(210.0);
                assert_eq!(id, other_id);
                let first_hooks = first.mission_scripts.clone();
                let second_hooks = second.mission_scripts.clone();
                for (active, foreign) in [(&mut first, second_hooks), (&mut second, first_hooks)] {
                    if matches!(
                        kind,
                        ScriptActionType::CameraStopFollow
                            | ScriptActionType::CameraStopTetherNamed
                    ) {
                        active.camera_follow_target = Some(id);
                        active.camera_tether_play = Some(2.0);
                    }
                    active.cinematic_letterbox = kind == ScriptActionType::CameraLetterboxEnd;
                    active.script_skybox_enabled = kind == ScriptActionType::DrawSkyboxEnd;
                    let mut actions = Vec::new();
                    if matches!(
                        kind,
                        ScriptActionType::CameraModLookToward
                            | ScriptActionType::CameraModFinalLookToward
                    ) {
                        actions.push(action_move_for_modifier());
                    }
                    actions.push(action.clone());
                    let mut list = ScriptList::new();
                    list.append_script(script("SameCamera", actions, true));
                    run(active, engine(foreign.clone(), list));
                    assert_camera_queues_empty(&foreign);
                    assert_remaining_effect(active, kind, id);
                }
            }
        },
    );
}
fn action_move_for_modifier() -> Box<ScriptAction> {
    action(
        ScriptActionType::MoveCameraTo,
        vec![
            Parameter::with_coord(
                ParameterType::Coord3D,
                gamelogic::common::Coord3D::new(800.0, 600.0, 0.0),
            ),
            Parameter::with_real(ParameterType::Real, 3.0),
        ],
    )
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn missing_camera_targets_are_no_ops_on_both_worlds() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "missing_camera_targets_are_no_ops_on_both_worlds",
        || {
            use ScriptActionType::*;
            let _terrain = TerrainRestore(Some(std::mem::replace(
                &mut *gamelogic::terrain::get_terrain_logic().write().unwrap(),
                camera_terrain(),
            )));
            let (mut first, id, _) = world(110.0);
            let (second, _, _) = world(210.0);
            let mut actions = remaining_camera_actions(100.0)
                .into_iter()
                .filter(|a| {
                    matches!(
                        a.action_type,
                        CameraFollowNamed
                            | CameraTetherNamed
                            | ResetCamera
                            | SetupCamera
                            | CameraLookTowardObject
                            | CameraLookTowardWaypoint
                            | CameraModLookToward
                            | CameraModFinalLookToward
                            | CameraAddShakerAt
                            | CameraMotionBlurJump
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(actions.len(), 10);
            for a in &mut actions {
                for parameter in a.parameters.iter_mut().flatten() {
                    if matches!(
                        parameter.param_type,
                        ParameterType::Unit | ParameterType::Waypoint
                    ) {
                        *parameter = Parameter::with_string(
                            parameter.param_type,
                            "AbsentCameraTarget".into(),
                        );
                    }
                }
            }
            first.camera_follow_target = Some(id);
            first.camera_tether_play = Some(4.0);
            let mut list = ScriptList::new();
            list.append_script(script("MissingTargets", actions, true));
            run(&mut first, engine(second.mission_scripts.clone(), list));
            assert_eq!(
                (first.camera_follow_target, first.camera_tether_play),
                (Some(id), Some(4.0))
            );
            assert!(first.script_camera_move_to.is_none());
            assert!(first.pending_camera_look_toward.is_none());
            assert!(first.pending_camera_zoom.is_none());
            assert!(first.pending_camera_pitch.is_none());
            assert!(first.pending_camera_add_shakers.is_empty());
            assert!(first.pending_camera_motion_blur.is_empty());
            assert_camera_queues_empty(&first.mission_scripts);
            assert_camera_queues_empty(&second.mission_scripts);
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn borrowed_blur_error_never_retries_a_foreign_handler() {
    use gamelogic::scripting::engine::{
        ScriptActionHandler, ScriptCameraRequest, ScriptExecutionDriver,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Foreign(AtomicUsize);
    impl ScriptActionHandler for Foreign {
        fn camera_motion_blur_jump(
            &self,
            _x: f32,
            _y: f32,
            _z: f32,
            _saturate: bool,
        ) -> gamelogic::GameLogicResult<()> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Err(gamelogic::GameLogicError::Configuration(
                "foreign filter failed".into(),
            ))
        }
        fn move_camera_to(
            &self,
            _x: f32,
            _y: f32,
            _z: f32,
            _seconds: f32,
            _stutter: f32,
            _ease_in: f32,
            _ease_out: f32,
        ) -> gamelogic::GameLogicResult<()> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
    }
    #[derive(Default)]
    struct Reject {
        calls: usize,
        flushes: usize,
    }
    impl ScriptExecutionDriver for Reject {
        fn camera(
            &mut self,
            request: ScriptCameraRequest<'_>,
        ) -> Option<gamelogic::GameLogicResult<()>> {
            assert_eq!(
                format!("{request:?}"),
                "MotionBlurJump { x: 410.0, y: 430.0, z: 10.0, saturate: true }"
            );
            self.calls += 1;
            Some(Err(gamelogic::GameLogicError::Configuration(
                "driving filter failed".into(),
            )))
        }
        fn after_action(&mut self) -> gamelogic::GameLogicResult<()> {
            self.flushes += 1;
            Ok(())
        }
    }
    super::sequential_actor_tests::isolated(
        module_path!(),
        "borrowed_blur_error_never_retries_a_foreign_handler",
        || {
            let _terrain = TerrainRestore(Some(std::mem::replace(
                &mut *gamelogic::terrain::get_terrain_logic().write().unwrap(),
                camera_terrain(),
            )));
            let foreign = Arc::new(Foreign(AtomicUsize::new(0)));
            let mut list = ScriptList::new();
            list.append_script(script(
                "RejectBlur",
                vec![action(
                    ScriptActionType::CameraMotionBlurJump,
                    vec![
                        Parameter::with_string(ParameterType::Waypoint, "LookPoint".into()),
                        Parameter::with_int(ParameterType::Boolean, 1),
                    ],
                )],
                true,
            ));
            let mut engine = ScriptEngine::new().unwrap();
            engine.set_action_handler(Some(foreign.clone()));
            engine
                .set_script_list_for_player(0, Some(Box::new(list)))
                .unwrap();
            let mut driver = Reject::default();
            engine
                .update_with_driver(
                    gamelogic::scripting::executor::ScriptContext::new(),
                    &mut driver,
                )
                .unwrap();
            assert_eq!((driver.calls, driver.flushes), (1, 1));
            assert_eq!(foreign.0.load(Ordering::Relaxed), 0);
        },
    );
}
