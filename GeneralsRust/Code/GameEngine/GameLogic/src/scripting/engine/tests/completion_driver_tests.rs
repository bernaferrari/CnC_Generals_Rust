//! Borrowed completion queries and standalone handler fallback.
use super::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Clone, PartialEq, Eq)]
enum RecordedCompletion {
    Speech(String, bool),
    Audio(String, bool),
}

#[derive(Default)]
struct CompletionDriver {
    responses: Vec<Option<bool>>,
    calls: Vec<RecordedCompletion>,
}

impl ScriptExecutionDriver for CompletionDriver {
    fn after_action(&mut self) -> GameLogicResult<()> {
        Ok(())
    }

    fn completion(&mut self, query: ScriptCompletionQuery<'_>) -> Option<bool> {
        self.calls.push(match query {
            ScriptCompletionQuery::Speech { name, flush } => {
                RecordedCompletion::Speech(name.to_string(), flush)
            }
            ScriptCompletionQuery::Audio { name, flush } => {
                RecordedCompletion::Audio(name.to_string(), flush)
            }
        });
        if self.responses.is_empty() {
            None
        } else {
            self.responses.remove(0)
        }
    }
}

struct RetainedCompletionHandler {
    speech_calls: Arc<AtomicUsize>,
    audio_calls: Arc<AtomicUsize>,
    speech_answer: bool,
    audio_answer: bool,
}

impl ScriptActionHandler for RetainedCompletionHandler {
    fn is_speech_complete(&self, name: &str, flush: bool) -> bool {
        assert_eq!(name, "Briefing");
        assert!(flush, "C++ HAS_FINISHED_SPEECH removes completed entries");
        self.speech_calls.fetch_add(1, Ordering::Relaxed);
        self.speech_answer
    }

    fn is_audio_complete(&self, name: &str, flush: bool) -> bool {
        assert_eq!(name, "Explosion");
        assert!(flush, "C++ HAS_FINISHED_AUDIO removes completed entries");
        self.audio_calls.fetch_add(1, Ordering::Relaxed);
        self.audio_answer
    }
}

fn completion_condition(kind: ConditionType, name: &str) -> Condition {
    let (parameter_type, name) = match kind {
        ConditionType::HasFinishedSpeech => (ParameterType::Dialog, name),
        ConditionType::HasFinishedAudio => (ParameterType::Sound, name),
        _ => panic!("not a completion condition"),
    };
    let mut condition = Condition::new(kind);
    condition
        .add_parameter(Parameter::with_string(parameter_type, name.to_string()))
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

fn retained_handler(
    speech_answer: bool,
    audio_answer: bool,
) -> (
    Arc<RetainedCompletionHandler>,
    Arc<AtomicUsize>,
    Arc<AtomicUsize>,
) {
    let speech_calls = Arc::new(AtomicUsize::new(0));
    let audio_calls = Arc::new(AtomicUsize::new(0));
    let handler = Arc::new(RetainedCompletionHandler {
        speech_calls: speech_calls.clone(),
        audio_calls: audio_calls.clone(),
        speech_answer,
        audio_answer,
    });
    (handler, speech_calls, audio_calls)
}

#[test]
fn borrowed_completion_answer_is_authoritative_for_true_and_false() {
    let _guard = crate::test_sync::lock();
    for (kind, name) in [
        (ConditionType::HasFinishedSpeech, "Briefing"),
        (ConditionType::HasFinishedAudio, "Explosion"),
    ] {
        for answer in [false, true] {
            let (handler, speech_calls, audio_calls) = retained_handler(!answer, !answer);
            let mut engine = ScriptEngine::new().unwrap();
            engine.set_action_handler(Some(handler));
            let mut evaluator =
                ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));
            let mut condition = completion_condition(kind, name);
            let mut driver = CompletionDriver {
                responses: vec![Some(answer)],
                ..Default::default()
            };

            let result = engine.with_active_for_test(|| {
                evaluator
                    .evaluate_condition_with_driver(&mut condition, &mut driver)
                    .unwrap()
            });

            assert_eq!(bool_result(result), answer);
            assert_eq!(
                driver.calls,
                [match kind {
                    ConditionType::HasFinishedSpeech => {
                        RecordedCompletion::Speech(name.to_string(), true)
                    }
                    ConditionType::HasFinishedAudio => {
                        RecordedCompletion::Audio(name.to_string(), true)
                    }
                    _ => unreachable!(),
                }]
            );
            assert_eq!(speech_calls.load(Ordering::Relaxed), 0);
            assert_eq!(audio_calls.load(Ordering::Relaxed), 0);
        }
    }
}

#[test]
fn unavailable_driver_and_public_wrapper_keep_retained_handler_behavior() {
    let _guard = crate::test_sync::lock();
    let (handler, speech_calls, audio_calls) = retained_handler(true, false);
    let mut engine = ScriptEngine::new().unwrap();
    engine.set_action_handler(Some(handler));
    let mut evaluator = ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));
    let mut driver = CompletionDriver::default();

    engine.with_active_for_test(|| {
        for (kind, name, expected) in [
            (ConditionType::HasFinishedSpeech, "Briefing", true),
            (ConditionType::HasFinishedAudio, "Explosion", false),
        ] {
            let mut condition = completion_condition(kind, name);
            let result = evaluator
                .evaluate_condition_with_driver(&mut condition, &mut driver)
                .unwrap();
            assert_eq!(bool_result(result), expected);

            let mut standalone = completion_condition(kind, name);
            let result = evaluator.evaluate_condition(&mut standalone).unwrap();
            assert_eq!(bool_result(result), expected);
        }
    });

    assert_eq!(
        driver.calls,
        [
            RecordedCompletion::Speech("Briefing".into(), true),
            RecordedCompletion::Audio("Explosion".into(), true),
        ]
    );
    assert_eq!(speech_calls.load(Ordering::Relaxed), 2);
    assert_eq!(audio_calls.load(Ordering::Relaxed), 2);
}

#[test]
fn malformed_completion_conditions_do_not_call_owner_or_fallback() {
    let _guard = crate::test_sync::lock();
    let (handler, speech_calls, audio_calls) = retained_handler(true, true);
    let mut engine = ScriptEngine::new().unwrap();
    engine.set_action_handler(Some(handler));
    let mut evaluator = ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));
    let mut driver = CompletionDriver {
        responses: vec![Some(true)],
        ..Default::default()
    };

    engine.with_active_for_test(|| {
        for kind in [
            ConditionType::HasFinishedSpeech,
            ConditionType::HasFinishedAudio,
        ] {
            let mut malformed = Condition::new(kind);
            assert!(
                evaluator
                    .evaluate_condition_with_driver(&mut malformed, &mut driver)
                    .is_err()
            );
        }
    });

    assert!(driver.calls.is_empty());
    assert_eq!(speech_calls.load(Ordering::Relaxed), 0);
    assert_eq!(audio_calls.load(Ordering::Relaxed), 0);
}

fn and_chain(first: Condition, second: Condition) -> Box<Condition> {
    let mut first = first;
    first.set_next_condition(Some(Box::new(second)));
    Box::new(first)
}

#[test]
fn completion_queries_preserve_and_or_short_circuit_order() {
    let _guard = crate::test_sync::lock();
    let mut evaluator = ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));

    // A true AND branch evaluates both members, then skips later OR branches.
    let mut first_or = OrCondition::new();
    first_or.set_first_and_condition(Some(and_chain(
        completion_condition(ConditionType::HasFinishedSpeech, "Briefing"),
        completion_condition(ConditionType::HasFinishedAudio, "Explosion"),
    )));
    let mut second_or = OrCondition::new();
    second_or.set_first_and_condition(Some(Box::new(completion_condition(
        ConditionType::HasFinishedAudio,
        "Explosion",
    ))));
    first_or.set_next_or_condition(Some(Box::new(second_or)));
    let mut driver = CompletionDriver {
        responses: vec![Some(true), Some(true)],
        ..Default::default()
    };
    assert!(
        evaluator
            .evaluate_or_condition_with_driver(&mut first_or, &mut driver)
            .unwrap()
    );
    assert_eq!(
        driver.calls,
        [
            RecordedCompletion::Speech("Briefing".into(), true),
            RecordedCompletion::Audio("Explosion".into(), true),
        ]
    );

    // A false first AND condition skips its sibling, then OR evaluates branch two.
    let mut first_or = OrCondition::new();
    first_or.set_first_and_condition(Some(and_chain(
        completion_condition(ConditionType::HasFinishedSpeech, "Briefing"),
        completion_condition(ConditionType::HasFinishedAudio, "Explosion"),
    )));
    let mut second_or = OrCondition::new();
    second_or.set_first_and_condition(Some(Box::new(completion_condition(
        ConditionType::HasFinishedAudio,
        "Explosion",
    ))));
    first_or.set_next_or_condition(Some(Box::new(second_or)));
    let mut driver = CompletionDriver {
        responses: vec![Some(false), Some(true)],
        ..Default::default()
    };
    assert!(
        evaluator
            .evaluate_or_condition_with_driver(&mut first_or, &mut driver)
            .unwrap()
    );
    assert_eq!(
        driver.calls,
        [
            RecordedCompletion::Speech("Briefing".into(), true),
            RecordedCompletion::Audio("Explosion".into(), true),
        ]
    );
}
