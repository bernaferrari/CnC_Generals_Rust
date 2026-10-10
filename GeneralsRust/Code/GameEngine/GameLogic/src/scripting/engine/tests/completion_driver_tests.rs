//! ScriptEngine.cpp:7268-7330 keeps completion timers on the executing engine.
use super::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

struct RetainedCompletionHandler {
    calls: Arc<AtomicUsize>,
    answer: bool,
}

impl ScriptActionHandler for RetainedCompletionHandler {
    fn is_speech_complete(&self, _name: &str, _flush: bool) -> bool {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.answer
    }

    fn is_audio_complete(&self, _name: &str, _flush: bool) -> bool {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.answer
    }
}

fn completion_condition(kind: ConditionType, name: &str) -> Condition {
    let mut condition = Condition::new(kind);
    condition
        .add_parameter(Parameter::with_string(
            match kind {
                ConditionType::HasFinishedSpeech => ParameterType::Dialog,
                ConditionType::HasFinishedAudio => ParameterType::Sound,
                _ => panic!("not a completion condition"),
            },
            name.into(),
        ))
        .unwrap();
    condition
}

fn bool_result(result: ScriptConditionResult) -> bool {
    match result {
        ScriptConditionResult::True => true,
        ScriptConditionResult::False => false,
        ScriptConditionResult::Error(message) => panic!("condition failed: {message}"),
    }
}

#[test]
fn borrowed_engine_timers_are_authoritative_for_true_and_false() {
    for (kind, name) in [
        (ConditionType::HasFinishedSpeech, "Briefing"),
        (ConditionType::HasFinishedAudio, "Explosion"),
    ] {
        for answer in [false, true] {
            let calls = Arc::new(AtomicUsize::new(0));
            let mut own = ScriptEngine::new().unwrap();
            own.set_action_handler(Some(Arc::new(RetainedCompletionHandler {
                calls: calls.clone(),
                answer: !answer,
            })));
            let mut tail = own.snapshot_xfer_tail();
            let timers = if kind == ConditionType::HasFinishedSpeech {
                &mut tail.testing_speech
            } else {
                &mut tail.testing_audio
            };
            timers.push((name.into(), if answer { 10 } else { 11 }));
            own.restore_xfer_tail(&tail);
            let foreign = ScriptEngine::new().unwrap();
            let mut foreign_tail = foreign.snapshot_xfer_tail();
            foreign_tail.testing_speech = vec![(name.into(), 90)];
            foreign_tail.testing_audio = vec![(name.into(), 100)];
            foreign.restore_xfer_tail(&foreign_tail);
            let state = std::cell::RefCell::new(ScriptContext::new());
            state.borrow_mut().current_frame = 10;
            let mut evaluator = ScriptConditionEvaluator::new(&own, &state);
            let mut condition = completion_condition(kind, name);
            // An ambient foreign scope cannot redirect the explicit engine.
            let result = foreign
                .with_active_for_test(|| evaluator.evaluate_condition(&mut condition).unwrap());
            assert_eq!(bool_result(result), answer);
            assert_eq!(calls.load(Ordering::Relaxed), 0);
            let after = own.snapshot_xfer_tail();
            let timers = if kind == ConditionType::HasFinishedSpeech {
                after.testing_speech
            } else {
                after.testing_audio
            };
            if answer {
                assert!(timers.is_empty(), "completed rows flush");
            } else {
                assert_eq!(timers, [(name.into(), 11)], "pending rows remain");
            }
            assert_eq!(
                foreign.snapshot_xfer_tail().testing_speech,
                [(name.into(), 90)]
            );
            assert_eq!(
                foreign.snapshot_xfer_tail().testing_audio,
                [(name.into(), 100)]
            );
        }
    }
}

#[test]
fn public_wrapper_and_driver_walk_query_the_same_canonical_timer() {
    let own = ScriptEngine::new().unwrap();
    let mut tail = own.snapshot_xfer_tail();
    tail.testing_speech = vec![("Briefing".into(), 20)];
    tail.testing_audio = vec![("Explosion".into(), 20)];
    own.restore_xfer_tail(&tail);
    let state = std::cell::RefCell::new(ScriptContext::new());
    state.borrow_mut().current_frame = 19;
    let mut evaluator = ScriptConditionEvaluator::new(&own, &state);
    let mut driver = CanonicalScriptExecutionDriver;
    for (kind, name) in [
        (ConditionType::HasFinishedSpeech, "Briefing"),
        (ConditionType::HasFinishedAudio, "Explosion"),
    ] {
        let mut condition = completion_condition(kind, name);
        assert!(!bool_result(
            evaluator
                .evaluate_condition_with_driver(&mut condition, &mut driver)
                .unwrap()
        ));
        assert!(!bool_result(
            evaluator.evaluate_condition(&mut condition).unwrap()
        ));
        state.borrow_mut().current_frame = 20;
        assert!(bool_result(
            evaluator.evaluate_condition(&mut condition).unwrap()
        ));
        state.borrow_mut().current_frame = 19;
    }
    assert!(own.snapshot_xfer_tail().testing_speech.is_empty());
    assert!(own.snapshot_xfer_tail().testing_audio.is_empty());
}

#[test]
fn malformed_completion_conditions_leave_canonical_timers_untouched() {
    let own = ScriptEngine::new().unwrap();
    let mut tail = own.snapshot_xfer_tail();
    tail.testing_speech = vec![("Briefing".into(), 20)];
    tail.testing_audio = vec![("Explosion".into(), 30)];
    own.restore_xfer_tail(&tail);
    let state = std::cell::RefCell::new(ScriptContext::new());
    let mut evaluator = ScriptConditionEvaluator::new(&own, &state);
    for kind in [
        ConditionType::HasFinishedSpeech,
        ConditionType::HasFinishedAudio,
    ] {
        assert!(
            evaluator
                .evaluate_condition(&mut Condition::new(kind))
                .is_err()
        );
    }
    assert_eq!(own.snapshot_xfer_tail().testing_speech, tail.testing_speech);
    assert_eq!(own.snapshot_xfer_tail().testing_audio, tail.testing_audio);
}

fn and_chain(first: Condition, second: Condition) -> Box<Condition> {
    let mut first = first;
    first.set_next_condition(Some(Box::new(second)));
    Box::new(first)
}

#[test]
fn completion_queries_preserve_and_or_short_circuit_order() {
    let own = ScriptEngine::new().unwrap();
    let state = std::cell::RefCell::new(ScriptContext::new());
    state.borrow_mut().current_frame = 10;
    let mut evaluator = ScriptConditionEvaluator::new(&own, &state);
    for speech_done in [true, false] {
        let mut tail = own.snapshot_xfer_tail();
        tail.testing_speech = vec![("Briefing".into(), if speech_done { 10 } else { 11 })];
        tail.testing_audio = vec![("Sibling".into(), 10), ("OtherBranch".into(), 10)];
        own.restore_xfer_tail(&tail);
        let mut first_or = OrCondition::new();
        first_or.set_first_and_condition(Some(and_chain(
            completion_condition(ConditionType::HasFinishedSpeech, "Briefing"),
            completion_condition(ConditionType::HasFinishedAudio, "Sibling"),
        )));
        let mut second_or = OrCondition::new();
        second_or.set_first_and_condition(Some(Box::new(completion_condition(
            ConditionType::HasFinishedAudio,
            "OtherBranch",
        ))));
        first_or.set_next_or_condition(Some(Box::new(second_or)));
        assert!(evaluator.evaluate_or_condition(&mut first_or).unwrap());
        if speech_done {
            assert!(own.snapshot_xfer_tail().testing_speech.is_empty());
            assert_eq!(
                own.snapshot_xfer_tail().testing_audio,
                [("OtherBranch".into(), 10)]
            );
        } else {
            assert_eq!(
                own.snapshot_xfer_tail().testing_speech,
                [("Briefing".into(), 11)]
            );
            assert_eq!(
                own.snapshot_xfer_tail().testing_audio,
                [("Sibling".into(), 10)]
            );
        }
    }
}

#[test]
fn speech_frames_from_length_ms_truncates_like_cpp() {
    for (length, frames) in [(0.0, 0), (5000.0, 150), (33.3, 0), (33.34, 1), (1000.0, 30)] {
        assert_eq!(
            ScriptEngine::timed_audio_frames_from_length_ms(length),
            frames
        );
    }
}
