//! Camera parsing, authoritative ownership and standalone synchronous re-entry.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Default)]
struct CameraDriver {
    requests: Vec<String>,
    flushed: Vec<usize>,
    reject: bool,
}
impl ScriptExecutionDriver for CameraDriver {
    fn camera(&mut self, request: ScriptCameraRequest<'_>) -> Option<GameLogicResult<()>> {
        self.requests.push(format!("{request:?}"));
        Some(if self.reject {
            Err(GameLogicError::Configuration("camera rejected".into()))
        } else {
            Ok(())
        })
    }
    fn after_action(&mut self) -> GameLogicResult<()> {
        self.flushed.push(self.requests.len());
        Ok(())
    }
}

struct RetainedCameraHandler {
    calls: Arc<AtomicUsize>,
    expected: Vec<String>,
}
impl RetainedCameraHandler {
    fn called(&self, request: ScriptCameraRequest<'_>) -> GameLogicResult<()> {
        let index = self.calls.fetch_add(1, Ordering::Relaxed);
        assert_eq!(format!("{request:?}"), self.expected[index]);
        // Immutable expected values need no recording lock during engine re-entry.
        assert!(matches!(
            with_script_engine_mut(|engine| engine.increment_counter("camera_reentry", 1)),
            Some(Ok(()))
        ));
        Err(GameLogicError::Configuration("camera rejected".into()))
    }
}
impl ScriptActionHandler for RetainedCameraHandler {
    fn move_camera_to(
        &self,
        x: f32,
        y: f32,
        z: f32,
        seconds: f32,
        camera_stutter_seconds: f32,
        ease_in_seconds: f32,
        ease_out_seconds: f32,
    ) -> GameLogicResult<()> {
        self.called(ScriptCameraRequest::MoveTo {
            x,
            y,
            z,
            seconds,
            camera_stutter_seconds,
            ease_in_seconds,
            ease_out_seconds,
        })
    }
    fn move_camera_along_waypoint_path(
        &self,
        waypoint_path: &str,
        seconds: f32,
        camera_stutter_seconds: f32,
        ease_in_seconds: f32,
        ease_out_seconds: f32,
    ) -> GameLogicResult<()> {
        self.called(ScriptCameraRequest::WaypointPath {
            waypoint_path,
            seconds,
            camera_stutter_seconds,
            ease_in_seconds,
            ease_out_seconds,
        })
    }
    fn rotate_camera(
        &self,
        rotations: f32,
        seconds: f32,
        ease_in_seconds: f32,
        ease_out_seconds: f32,
    ) -> GameLogicResult<()> {
        self.called(ScriptCameraRequest::Rotate {
            rotations,
            seconds,
            ease_in_seconds,
            ease_out_seconds,
        })
    }
    fn move_camera_to_selection(&self) -> GameLogicResult<()> {
        self.called(ScriptCameraRequest::MoveToSelection)
    }
    fn camera_move_home(&self) -> GameLogicResult<()> {
        self.called(ScriptCameraRequest::MoveHome)
    }
    fn zoom_camera(
        &self,
        zoom: f32,
        seconds: f32,
        ease_in_seconds: f32,
        ease_out_seconds: f32,
    ) -> GameLogicResult<()> {
        self.called(ScriptCameraRequest::Zoom {
            zoom,
            seconds,
            ease_in_seconds,
            ease_out_seconds,
        })
    }
    fn set_camera_pitch(
        &self,
        pitch: f32,
        seconds: f32,
        ease_in_seconds: f32,
        ease_out_seconds: f32,
    ) -> GameLogicResult<()> {
        self.called(ScriptCameraRequest::Pitch {
            pitch,
            seconds,
            ease_in_seconds,
            ease_out_seconds,
        })
    }
    fn camera_mod_freeze_time(&self) -> GameLogicResult<()> {
        self.called(ScriptCameraRequest::FreezeTime)
    }
    fn camera_mod_freeze_angle(&self) -> GameLogicResult<()> {
        self.called(ScriptCameraRequest::FreezeAngle)
    }
    fn camera_mod_set_final_zoom(
        &self,
        zoom: f32,
        ease_in: f32,
        ease_out: f32,
    ) -> GameLogicResult<()> {
        self.called(ScriptCameraRequest::FinalZoom {
            zoom,
            ease_in,
            ease_out,
        })
    }
    fn camera_mod_set_final_pitch(
        &self,
        pitch: f32,
        ease_in: f32,
        ease_out: f32,
    ) -> GameLogicResult<()> {
        self.called(ScriptCameraRequest::FinalPitch {
            pitch,
            ease_in,
            ease_out,
        })
    }
    fn camera_mod_set_final_speed_multiplier(&self, multiplier: i32) -> GameLogicResult<()> {
        self.called(ScriptCameraRequest::FinalSpeedMultiplier { multiplier })
    }
    fn camera_mod_set_rolling_average(&self, frames: i32) -> GameLogicResult<()> {
        self.called(ScriptCameraRequest::RollingAverage { frames })
    }
}

fn cases(explicit: bool) -> Vec<(ScriptAction, ScriptCameraRequest<'static>)> {
    let mut cases = Vec::new();
    {
        let mut action = ScriptAction::new(ScriptActionType::MoveCameraTo);
        action
            .add_parameter(Parameter::with_coord(
                ParameterType::Coord3D,
                crate::common::Coord3D::new(-12.0, 34.0, 56.0),
            ))
            .unwrap();
        if explicit {
            action
                .add_parameter(Parameter::with_real(ParameterType::Real, 4.25))
                .unwrap();
        }
        if explicit {
            action
                .add_parameter(Parameter::with_real(ParameterType::Real, 5.25))
                .unwrap();
        }
        if explicit {
            action
                .add_parameter(Parameter::with_real(ParameterType::Real, 6.25))
                .unwrap();
        }
        if explicit {
            action
                .add_parameter(Parameter::with_real(ParameterType::Real, 7.25))
                .unwrap();
        }
        cases.push((
            action,
            ScriptCameraRequest::MoveTo {
                x: -12.0,
                y: 34.0,
                z: 56.0,
                seconds: if explicit { 4.25 } else { 0.0 },
                camera_stutter_seconds: if explicit { 5.25 } else { 0.0 },
                ease_in_seconds: if explicit { 6.25 } else { 0.0 },
                ease_out_seconds: if explicit { 7.25 } else { 0.0 },
            },
        ));
    }
    {
        let mut action = ScriptAction::new(ScriptActionType::MoveCameraAlongWaypointPath);
        action
            .add_parameter(Parameter::with_string(
                ParameterType::Waypoint,
                "Path A".into(),
            ))
            .unwrap();
        if explicit {
            action
                .add_parameter(Parameter::with_real(ParameterType::Real, 2.25))
                .unwrap();
        }
        if explicit {
            action
                .add_parameter(Parameter::with_real(ParameterType::Real, 3.25))
                .unwrap();
        }
        if explicit {
            action
                .add_parameter(Parameter::with_real(ParameterType::Real, 4.25))
                .unwrap();
        }
        if explicit {
            action
                .add_parameter(Parameter::with_real(ParameterType::Real, 5.25))
                .unwrap();
        }
        cases.push((
            action,
            ScriptCameraRequest::WaypointPath {
                waypoint_path: "Path A",
                seconds: if explicit { 2.25 } else { 0.0 },
                camera_stutter_seconds: if explicit { 3.25 } else { 0.0 },
                ease_in_seconds: if explicit { 4.25 } else { 0.0 },
                ease_out_seconds: if explicit { 5.25 } else { 0.0 },
            },
        ));
    }
    {
        let mut action = ScriptAction::new(ScriptActionType::RotateCamera);
        action
            .add_parameter(Parameter::with_real(ParameterType::Real, 1.25))
            .unwrap();
        if explicit {
            action
                .add_parameter(Parameter::with_real(ParameterType::Real, 2.25))
                .unwrap();
        }
        if explicit {
            action
                .add_parameter(Parameter::with_real(ParameterType::Real, 3.25))
                .unwrap();
        }
        if explicit {
            action
                .add_parameter(Parameter::with_real(ParameterType::Real, 4.25))
                .unwrap();
        }
        cases.push((
            action,
            ScriptCameraRequest::Rotate {
                rotations: 1.25,
                seconds: if explicit { 2.25 } else { 0.0 },
                ease_in_seconds: if explicit { 3.25 } else { 0.0 },
                ease_out_seconds: if explicit { 4.25 } else { 0.0 },
            },
        ));
    }
    {
        let mut action = ScriptAction::new(ScriptActionType::MoveCameraToSelection);
        cases.push((action, ScriptCameraRequest::MoveToSelection));
    }
    {
        let mut action = ScriptAction::new(ScriptActionType::CameraMoveHome);
        cases.push((action, ScriptCameraRequest::MoveHome));
    }
    {
        let mut action = ScriptAction::new(ScriptActionType::ZoomCamera);
        action
            .add_parameter(Parameter::with_real(ParameterType::Real, 1.25))
            .unwrap();
        if explicit {
            action
                .add_parameter(Parameter::with_real(ParameterType::Real, 2.25))
                .unwrap();
        }
        if explicit {
            action
                .add_parameter(Parameter::with_real(ParameterType::Real, 3.25))
                .unwrap();
        }
        if explicit {
            action
                .add_parameter(Parameter::with_real(ParameterType::Real, 4.25))
                .unwrap();
        }
        cases.push((
            action,
            ScriptCameraRequest::Zoom {
                zoom: 1.25,
                seconds: if explicit { 2.25 } else { 0.0 },
                ease_in_seconds: if explicit { 3.25 } else { 0.0 },
                ease_out_seconds: if explicit { 4.25 } else { 0.0 },
            },
        ));
    }
    {
        let mut action = ScriptAction::new(ScriptActionType::PitchCamera);
        action
            .add_parameter(Parameter::with_real(ParameterType::Real, 1.25))
            .unwrap();
        if explicit {
            action
                .add_parameter(Parameter::with_real(ParameterType::Real, 2.25))
                .unwrap();
        }
        if explicit {
            action
                .add_parameter(Parameter::with_real(ParameterType::Real, 3.25))
                .unwrap();
        }
        if explicit {
            action
                .add_parameter(Parameter::with_real(ParameterType::Real, 4.25))
                .unwrap();
        }
        cases.push((
            action,
            ScriptCameraRequest::Pitch {
                pitch: 1.25,
                seconds: if explicit { 2.25 } else { 0.0 },
                ease_in_seconds: if explicit { 3.25 } else { 0.0 },
                ease_out_seconds: if explicit { 4.25 } else { 0.0 },
            },
        ));
    }
    {
        let mut action = ScriptAction::new(ScriptActionType::CameraModFreezeTime);
        cases.push((action, ScriptCameraRequest::FreezeTime));
    }
    {
        let mut action = ScriptAction::new(ScriptActionType::CameraModFreezeAngle);
        cases.push((action, ScriptCameraRequest::FreezeAngle));
    }
    {
        let mut action = ScriptAction::new(ScriptActionType::CameraModSetFinalZoom);
        action
            .add_parameter(Parameter::with_real(ParameterType::Real, 1.25))
            .unwrap();
        if explicit {
            action
                .add_parameter(Parameter::with_real(ParameterType::Real, 2.25))
                .unwrap();
        }
        if explicit {
            action
                .add_parameter(Parameter::with_real(ParameterType::Real, 3.25))
                .unwrap();
        }
        cases.push((
            action,
            ScriptCameraRequest::FinalZoom {
                zoom: 1.25,
                ease_in: if explicit { 2.25 } else { 0.0 },
                ease_out: if explicit { 3.25 } else { 0.0 },
            },
        ));
    }
    {
        let mut action = ScriptAction::new(ScriptActionType::CameraModSetFinalPitch);
        action
            .add_parameter(Parameter::with_real(ParameterType::Real, 1.25))
            .unwrap();
        if explicit {
            action
                .add_parameter(Parameter::with_real(ParameterType::Real, 2.25))
                .unwrap();
        }
        if explicit {
            action
                .add_parameter(Parameter::with_real(ParameterType::Real, 3.25))
                .unwrap();
        }
        cases.push((
            action,
            ScriptCameraRequest::FinalPitch {
                pitch: 1.25,
                ease_in: if explicit { 2.25 } else { 0.0 },
                ease_out: if explicit { 3.25 } else { 0.0 },
            },
        ));
    }
    {
        let mut action = ScriptAction::new(ScriptActionType::CameraModSetFinalSpeedMultiplier);
        action
            .add_parameter(Parameter::with_int(ParameterType::Int, -7))
            .unwrap();
        cases.push((
            action,
            ScriptCameraRequest::FinalSpeedMultiplier { multiplier: -7 },
        ));
    }
    {
        let mut action = ScriptAction::new(ScriptActionType::CameraModSetRollingAverage);
        action
            .add_parameter(Parameter::with_int(ParameterType::Int, -7))
            .unwrap();
        cases.push((action, ScriptCameraRequest::RollingAverage { frames: -7 }));
    }
    cases
}

fn camera_engine(explicit: bool) -> (ScriptEngine, Arc<AtomicUsize>, Vec<String>) {
    let mut inputs = cases(explicit);
    let expected: Vec<String> = inputs
        .iter()
        .map(|(_, request)| format!("{request:?}"))
        .collect();
    let mut next = None;
    while let Some((mut action, _)) = inputs.pop() {
        action.next_action = next;
        next = Some(Box::new(action));
    }
    let mut script = Script::new();
    script.is_one_shot = true;
    script.condition = Some(always_true_condition());
    script.action = next;
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let mut engine = ScriptEngine::new_in_published_world().unwrap();
    let retained = Arc::new(AtomicUsize::new(0));
    engine.set_action_handler(Some(Arc::new(RetainedCameraHandler {
        calls: retained.clone(),
        expected: expected.clone(),
    })));
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    (engine, retained, expected)
}

#[test]
fn borrowed_camera_driver_preserves_parameters_and_error_continuation() {
    let _guard = crate::test_sync::lock();
    for explicit in [false, true] {
        for reject in [false, true] {
            let (engine, retained, expected) = camera_engine(explicit);
            let mut driver = CameraDriver {
                reject,
                ..Default::default()
            };
            engine
                .update_with_driver(ScriptContext::new(), &mut driver)
                .unwrap();
            assert_eq!(driver.requests, expected);
            assert_eq!(driver.flushed, (1..=13).collect::<Vec<_>>());
            assert!(
                retained.load(Ordering::Relaxed) == 0,
                "authoritative error must not retry the retained owner"
            );
        }
    }
}

#[test]
fn standalone_camera_adapter_preserves_defaults_errors_and_reentry() {
    let _guard = crate::test_sync::lock();
    for explicit in [false, true] {
        let (engine, retained, expected) = camera_engine(explicit);
        engine.update().unwrap();
        assert_eq!(retained.load(Ordering::Relaxed), expected.len());
        assert_eq!(engine.get_counter("camera_reentry").unwrap().value, 13);
    }
}

#[test]
fn standalone_camera_targets_belong_to_the_engine_under_a_peer_world_scope() {
    use crate::system::engine_stores::{new_world_services, with_world_services};
    use game_engine::map_data::{MapData, MapWaypoint};

    #[derive(Debug, PartialEq)]
    enum CameraTarget {
        Waypoint(Coord3D),
        Object(u32),
    }

    #[derive(Default)]
    struct CameraTargets(Mutex<Vec<CameraTarget>>);

    impl ScriptActionHandler for CameraTargets {
        fn move_camera_to(
            &self,
            x: f32,
            y: f32,
            z: f32,
            _: f32,
            _: f32,
            _: f32,
            _: f32,
        ) -> GameLogicResult<()> {
            self.0.lock().unwrap().push(CameraTarget::Waypoint(Coord3D::new(x, y, z)));
            Ok(())
        }

        fn camera_follow_object(&self, id: u32, _: bool) -> GameLogicResult<()> {
            self.0.lock().unwrap().push(CameraTarget::Object(id));
            Ok(())
        }
    }

    let receiving = new_world_services();
    let peer = new_world_services();
    let position = Coord3D::new(10.0, 20.0, 0.0);
    for (world, position, id) in [
        (&receiving, position, 17),
        (&peer, Coord3D::new(80.0, 90.0, 0.0), 24),
    ] {
        let mut map = MapData::new();
        map.width = 16;
        map.height = 16;
        map.heightmap = vec![0; 16 * 16];
        map.waypoints.push(MapWaypoint {
            id: 1,
            name: "SharedWaypoint".into(),
            location: position,
            path_label1: String::new(),
            path_label2: String::new(),
            path_label3: String::new(),
            bi_directional: false,
        });
        world.terrain().write().unwrap().load_map_data(map);
        world
            .scripts()
            .named_objects()
            .register_named_object("SharedUnit".into(), id)
            .unwrap();
    }
    let targets = Arc::new(CameraTargets::default());
    let mut engine = ScriptEngine::new_with_services(&receiving).unwrap();
    engine.set_action_handler(Some(targets.clone()));
    let state = std::cell::RefCell::new(ScriptContext::new());
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &state);

    with_world_services(&peer, || {
        for (opcode, kind, name) in [
            (ScriptActionType::MoveCameraTo, ParameterType::Waypoint, "SharedWaypoint"),
            (ScriptActionType::CameraFollowNamed, ParameterType::Unit, "SharedUnit"),
        ] {
            let mut action = ScriptAction::new(opcode);
            action.add_parameter(Parameter::with_string(kind, name.into())).unwrap();
            assert_eq!(dispatcher.execute_action(&action).unwrap(), ScriptActionResult::Success);
        }
    });

    assert_eq!(
        &*targets.0.lock().unwrap(),
        &[CameraTarget::Waypoint(position), CameraTarget::Object(17)],
    );
}

#[test]
fn invalid_camera_parameters_do_not_reach_the_owner() {
    let dispatch_engine = crate::scripting::engine::ScriptEngine::new_in_published_world().expect("script engine");

    let _guard = crate::test_sync::lock();
    let dispatcher_state = std::cell::RefCell::new(ScriptContext::new());
    let mut dispatcher = ScriptActionDispatcher::new(&dispatch_engine, &dispatcher_state);
    let mut driver = CameraDriver::default();
    for kind in [
        ScriptActionType::MoveCameraTo,
        ScriptActionType::MoveCameraAlongWaypointPath,
        ScriptActionType::RotateCamera,
        ScriptActionType::ZoomCamera,
        ScriptActionType::PitchCamera,
        ScriptActionType::CameraModSetFinalZoom,
        ScriptActionType::CameraModSetFinalPitch,
        ScriptActionType::CameraModSetFinalSpeedMultiplier,
        ScriptActionType::CameraModSetRollingAverage,
    ] {
        assert!(
            dispatcher
                .execute_action_with_driver(&ScriptAction::new(kind), &mut driver)
                .is_err()
        );
    }
    assert!(driver.requests.is_empty());
}
