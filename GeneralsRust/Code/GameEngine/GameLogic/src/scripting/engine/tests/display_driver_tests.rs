//! Borrowed display ownership, parameter policy and standalone re-entry.
use super::*;
use game_engine::common::global_data;
use std::sync::atomic::{AtomicUsize, Ordering};

struct CaptionPolicy(bool);

impl CaptionPolicy {
    fn set(disabled: bool) -> Self {
        let mut data = global_data::write_safe().unwrap();
        Self(std::mem::replace(
            &mut data.writable.disable_military_caption,
            disabled,
        ))
    }
}

impl Drop for CaptionPolicy {
    fn drop(&mut self) {
        global_data::write_safe()
            .unwrap()
            .writable
            .disable_military_caption = self.0;
    }
}

struct RetainedDisplayHandler(Arc<AtomicUsize>);

impl RetainedDisplayHandler {
    fn called(&self) -> GameLogicResult<()> {
        self.0.fetch_add(1, Ordering::Relaxed);
        // The dispatcher must release engine/list borrows before callbacks.
        assert!(matches!(
            with_script_engine_mut(|engine| engine.increment_counter("display_reentry", 1)),
            Some(Ok(()))
        ));
        Err(GameLogicError::Configuration("display rejected".into()))
    }
}

impl ScriptActionHandler for RetainedDisplayHandler {
    fn display_text(&self, text: &str) -> GameLogicResult<()> {
        assert_eq!(text, "text");
        self.called()
    }

    fn display_cinematic_text(&self, text: &str, font: &str, seconds: i32) -> GameLogicResult<()> {
        assert_eq!((text, font, seconds), ("cinematic", "Default", 0));
        self.called()
    }

    fn military_caption(&self, text: &str, duration: i32) -> GameLogicResult<()> {
        assert_eq!((text, duration), ("caption", -4000));
        self.called()
    }
}

fn action(kind: ScriptActionType, text: &str) -> Box<ScriptAction> {
    let mut action = ScriptAction::new(kind);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::TextString,
            text.into(),
        ))
        .unwrap();
    if kind == ScriptActionType::ShowMilitaryCaption {
        action
            .add_parameter(Parameter::with_int(ParameterType::Int, -4000))
            .unwrap();
    }
    Box::new(action)
}

fn engine() -> (ScriptEngine, Arc<AtomicUsize>) {
    let mut text = action(ScriptActionType::DisplayText, "text");
    let mut cinematic = action(ScriptActionType::DisplayCinematicText, "cinematic");
    cinematic.next_action = Some(action(ScriptActionType::ShowMilitaryCaption, "caption"));
    text.next_action = Some(cinematic);
    let mut script = Script::new();
    script.is_one_shot = true;
    script.condition = Some(always_true_condition());
    script.action = Some(text);
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let calls = Arc::new(AtomicUsize::new(0));
    let mut engine = ScriptEngine::new().unwrap();
    engine.set_action_handler(Some(Arc::new(RetainedDisplayHandler(calls.clone()))));
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    (engine, calls)
}

#[derive(Default)]
struct DisplayDriver {
    requests: Vec<RecordedDisplay>,
    flushed: Vec<usize>,
    reject: bool,
}

#[derive(Debug, PartialEq, Eq)]
enum RecordedDisplay {
    Text(String),
    Cinematic(String, String, i32),
    Caption(String, i32),
}

impl ScriptExecutionDriver for DisplayDriver {
    fn display(&mut self, request: ScriptDisplayRequest<'_>) -> Option<GameLogicResult<()>> {
        self.requests.push(match request {
            ScriptDisplayRequest::Text(text) => RecordedDisplay::Text(text.into()),
            ScriptDisplayRequest::Cinematic {
                text,
                font,
                duration_seconds,
            } => RecordedDisplay::Cinematic(text.into(), font.into(), duration_seconds),
            ScriptDisplayRequest::MilitaryCaption { text, duration_ms } => {
                RecordedDisplay::Caption(text.into(), duration_ms)
            }
        });
        Some(if self.reject {
            Err(GameLogicError::Configuration(
                "owner rejected display".into(),
            ))
        } else {
            Ok(())
        })
    }

    fn after_action(&mut self) -> GameLogicResult<()> {
        self.flushed.push(self.requests.len());
        Ok(())
    }
}

#[test]
fn borrowed_display_owner_is_authoritative_even_on_callback_error() {
    let _guard = crate::test_sync::lock();
    let _caption_policy = CaptionPolicy::set(false);
    for reject in [false, true] {
        let (engine, retained) = engine();
        let mut driver = DisplayDriver {
            reject,
            ..Default::default()
        };
        engine
            .update_with_driver(ScriptContext::new(), &mut driver)
            .unwrap();
        assert_eq!(
            retained.load(Ordering::Relaxed),
            0,
            "no fallback or duplicate delivery"
        );
        assert_eq!(
            driver.flushed,
            [1, 2, 3],
            "errors preserve action continuation and flushes"
        );
        assert_eq!(
            driver.requests,
            [
                RecordedDisplay::Text("text".into()),
                RecordedDisplay::Cinematic("cinematic".into(), "Default".into(), 0),
                RecordedDisplay::Caption("caption".into(), -4000),
            ]
        );
    }
}

#[test]
fn standalone_display_adapter_preserves_defaults_errors_and_reentry() {
    let _guard = crate::test_sync::lock();
    let _caption_policy = CaptionPolicy::set(false);
    let (engine, retained) = engine();
    engine.update().unwrap();
    assert_eq!(retained.load(Ordering::Relaxed), 3);
    assert_eq!(engine.get_counter("display_reentry").unwrap().value, 3);
}

#[test]
fn invalid_display_parameters_do_not_reach_the_owner() {
    let dispatch_engine = crate::scripting::engine::ScriptEngine::new().expect("script engine");

    let _guard = crate::test_sync::lock();
    let dispatcher_state = std::cell::RefCell::new(ScriptContext::new());
    let mut dispatcher = ScriptActionDispatcher::new(&dispatch_engine, &dispatcher_state);
    let mut driver = DisplayDriver::default();
    for kind in [
        ScriptActionType::DisplayText,
        ScriptActionType::DisplayCinematicText,
        ScriptActionType::ShowMilitaryCaption,
    ] {
        assert!(
            dispatcher
                .execute_action_with_driver(&ScriptAction::new(kind), &mut driver)
                .is_err()
        );
    }
    let missing_duration = action(ScriptActionType::DisplayText, "caption");
    let mut missing_duration = *missing_duration;
    missing_duration.action_type = ScriptActionType::ShowMilitaryCaption;
    assert!(
        dispatcher
            .execute_action_with_driver(&missing_duration, &mut driver)
            .is_err()
    );
    assert!(driver.requests.is_empty());
    assert!(driver.flushed.is_empty());
}

#[test]
fn display_parameters_preserve_explicit_cinematic_values_and_caption_policy() {
    let dispatch_engine = crate::scripting::engine::ScriptEngine::new().expect("script engine");

    let _guard = crate::test_sync::lock();
    let dispatcher_state = std::cell::RefCell::new(ScriptContext::new());
    let mut dispatcher = ScriptActionDispatcher::new(&dispatch_engine, &dispatcher_state);
    let mut driver = DisplayDriver::default();
    let mut cinematic = action(ScriptActionType::DisplayCinematicText, "explicit");
    cinematic
        .add_parameter(Parameter::with_string(
            ParameterType::TextString,
            "Font - Size: 18 [Bold]".into(),
        ))
        .unwrap();
    cinematic
        .add_parameter(Parameter::with_int(ParameterType::Int, -3))
        .unwrap();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&cinematic, &mut driver)
            .unwrap(),
        ScriptActionResult::Success
    );
    for disabled in [false, true] {
        let _policy = CaptionPolicy::set(disabled);
        assert_eq!(
            dispatcher
                .execute_action_with_driver(
                    &action(ScriptActionType::ShowMilitaryCaption, "caption"),
                    &mut driver
                )
                .unwrap(),
            ScriptActionResult::Success
        );
    }
    assert_eq!(
        driver.requests,
        [
            RecordedDisplay::Cinematic("explicit".into(), "Font - Size: 18 [Bold]".into(), -3),
            RecordedDisplay::Caption("caption".into(), -4000),
            RecordedDisplay::Caption("caption".into(), 1),
        ]
    );
}
