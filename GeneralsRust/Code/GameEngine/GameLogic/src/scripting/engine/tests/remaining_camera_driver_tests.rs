//! Additional camera/view actions must select one driving owner, even on error.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Default)]
struct ViewDriver {
    requests: Vec<String>,
    flushes: Vec<usize>,
    reject: bool,
}
impl ScriptExecutionDriver for ViewDriver {
    fn camera(&mut self, request: ScriptCameraRequest<'_>) -> Option<GameLogicResult<()>> {
        self.requests.push(format!("{request:?}"));
        Some(if self.reject {
            Err(GameLogicError::Configuration("view rejected".into()))
        } else {
            Ok(())
        })
    }
    fn after_action(&mut self) -> GameLogicResult<()> {
        self.flushes.push(self.requests.len());
        Ok(())
    }
}
struct ViewHandler(Arc<AtomicUsize>);
impl ViewHandler {
    fn called(&self) -> GameLogicResult<()> {
        self.0.fetch_add(1, Ordering::Relaxed);
        assert!(matches!(
            with_script_engine_mut(|engine| engine.increment_counter("view_reentry", 1)),
            Some(Ok(()))
        ));
        Err(GameLogicError::Configuration(
            "standalone view rejected".into(),
        ))
    }
}
impl ScriptActionHandler for ViewHandler {
    fn stop_camera_follow(&self) -> GameLogicResult<()> {
        self.called()
    }
    fn camera_letterbox_begin(&self) -> GameLogicResult<()> {
        self.called()
    }
    fn camera_letterbox_end(&self) -> GameLogicResult<()> {
        self.called()
    }
    fn camera_set_default(&self, pitch: f32, angle: f32, max_height: f32) -> GameLogicResult<()> {
        assert_eq!((pitch, angle, max_height), (0.2, 0.4, 120.0));
        self.called()
    }
    fn camera_enable_slave_mode(&self, template: &str, bone: &str) -> GameLogicResult<()> {
        assert_eq!((template, bone), ("SameTemplate", "CameraBone"));
        self.called()
    }
    fn camera_disable_slave_mode(&self) -> GameLogicResult<()> {
        self.called()
    }
    fn screen_shake(&self, intensity: i32) -> GameLogicResult<()> {
        assert_eq!(intensity, 2);
        self.called()
    }
    fn resize_view_guardband(&self, x: f32, y: f32) -> GameLogicResult<()> {
        assert_eq!((x, y), (0.3, 0.6));
        self.called()
    }
    fn set_camera_bw_mode(&self, enabled: bool, frames: i32) -> GameLogicResult<()> {
        assert_eq!(frames, if enabled { 17 } else { 0 });
        self.called()
    }
    fn set_skybox_enabled(&self, _: bool) -> GameLogicResult<()> {
        self.called()
    }
    fn camera_motion_blur(&self, zoom: bool, saturate: bool) -> GameLogicResult<()> {
        assert_eq!((zoom, saturate), (true, false));
        self.called()
    }
    fn camera_motion_blur_follow(&self, amount: i32) -> GameLogicResult<()> {
        assert_eq!(amount, 8);
        self.called()
    }
    fn camera_motion_blur_end_follow(&self) -> GameLogicResult<()> {
        self.called()
    }
}
fn view_cases() -> Vec<(ScriptAction, &'static str)> {
    use ScriptActionType::*;
    let real = |v| Parameter::with_real(ParameterType::Real, v);
    let integer = |v| Parameter::with_int(ParameterType::Int, v);
    let text = |v: &str| Parameter::with_string(ParameterType::TextString, v.into());
    let inputs = vec![
        (CameraStopFollow, vec![], "StopFollow"),
        (CameraStopTetherNamed, vec![], "StopFollow"),
        (CameraLetterboxBegin, vec![], "LetterboxBegin"),
        (CameraLetterboxEnd, vec![], "LetterboxEnd"),
        (
            CameraSetDefault,
            vec![real(0.2), real(0.4), real(120.0)],
            "SetDefault { pitch: 0.2, angle: 0.4, max_height: 120.0 }",
        ),
        (
            CameraEnableSlaveMode,
            vec![text("SameTemplate"), text("CameraBone")],
            "EnableSlaveMode { thing_template_name: \"SameTemplate\", bone_name: \"CameraBone\" }",
        ),
        (CameraDisableSlaveMode, vec![], "DisableSlaveMode"),
        (
            ScreenShake,
            vec![integer(2)],
            "ScreenShake { intensity: 2 }",
        ),
        (
            ResizeViewGuardband,
            vec![real(0.3), real(0.6)],
            "ViewGuardband { gbx: 0.3, gby: 0.6 }",
        ),
        (
            CameraBwModeBegin,
            vec![integer(17)],
            "BwMode { enabled: true, frames: 17 }",
        ),
        (
            CameraBwModeEnd,
            vec![],
            "BwMode { enabled: false, frames: 0 }",
        ),
        (DrawSkyboxBegin, vec![], "SkyboxEnabled { enabled: true }"),
        (DrawSkyboxEnd, vec![], "SkyboxEnabled { enabled: false }"),
        (
            CameraMotionBlur,
            vec![Parameter::with_int(ParameterType::Boolean, 1)],
            "MotionBlur { zoom_in: true, saturate: false }",
        ),
        (
            CameraMotionBlurFollow,
            vec![integer(8)],
            "MotionBlurFollow { amount: 8 }",
        ),
        (CameraMotionBlurEndFollow, vec![], "MotionBlurEndFollow"),
    ];
    inputs
        .into_iter()
        .map(|(kind, params, expected)| {
            let mut action = ScriptAction::new(kind);
            for p in params {
                action.add_parameter(p).unwrap();
            }
            (action, expected)
        })
        .collect()
}
fn view_engine() -> (ScriptEngine, Arc<AtomicUsize>, Vec<String>) {
    let mut cases = view_cases();
    let expected = cases.iter().map(|(_, e)| e.to_string()).collect();
    let mut next = None;
    while let Some((mut action, _)) = cases.pop() {
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
    let calls = Arc::new(AtomicUsize::new(0));
    engine.set_action_handler(Some(Arc::new(ViewHandler(calls.clone()))));
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    (engine, calls, expected)
}
#[test]
fn remaining_view_driver_is_authoritative_and_flushes_every_action_on_error() {
    let _guard = crate::test_sync::lock();
    for reject in [false, true] {
        let (engine, calls, expected) = view_engine();
        let mut driver = ViewDriver {
            reject,
            ..Default::default()
        };
        engine
            .update_with_driver(ScriptContext::new(), &mut driver)
            .unwrap();
        assert_eq!(driver.requests, expected);
        assert_eq!(driver.flushes, (1..=16).collect::<Vec<_>>());
        assert_eq!(
            calls.load(Ordering::Relaxed),
            0,
            "never retry a foreign handler after Some(Err)"
        );
    }
}
#[test]
fn standalone_remaining_view_callbacks_keep_defaults_errors_and_engine_reentry() {
    let _guard = crate::test_sync::lock();
    let (engine, calls, expected) = view_engine();
    engine.update().unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), expected.len());
    assert_eq!(engine.get_counter("view_reentry").unwrap().value, 16);
}
