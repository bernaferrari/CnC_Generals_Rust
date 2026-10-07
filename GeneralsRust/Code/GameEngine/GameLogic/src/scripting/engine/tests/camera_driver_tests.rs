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
    let mut engine = ScriptEngine::new().unwrap();
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
fn invalid_camera_parameters_do_not_reach_the_owner() {
    let _guard = crate::test_sync::lock();
    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
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
