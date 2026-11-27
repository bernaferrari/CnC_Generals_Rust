use game_engine::common::random_value::{SessionRandom, with_session_random};
use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
use gamelogic::scripting::XferSnapshot;
use gamelogic::scripting::engine::{ScriptEngine, ScriptExecutionDriver};
use gamelogic::scripting::executor::{
    ScriptActionDispatcher, ScriptConditionEvaluator, ScriptConditionResult, ScriptContext,
};
use gamelogic::system::engine_stores::{new_world_services, with_world_services};
use generals_sim::scripting::program::{
    ComparisonType, Condition, ConditionType, Parameter, ParameterType, ScriptAction,
    ScriptActionType, THIS_PLAYER,
};
use std::cell::RefCell;
use std::io::Cursor;

fn counter_name(name: &str) -> Parameter {
    Parameter::with_string(ParameterType::Counter, name.into())
}

fn run(engine: &ScriptEngine, kind: ScriptActionType, params: impl IntoIterator<Item = Parameter>) {
    let context = RefCell::new(ScriptContext::at_frame(0));
    let mut action = ScriptAction::new(kind);
    for param in params {
        action.add_parameter(param).unwrap();
    }
    ScriptActionDispatcher::new(engine, &context)
        .execute_action(&action)
        .unwrap();
}

fn evaluate(
    engine: &ScriptEngine,
    kind: ConditionType,
    params: impl IntoIterator<Item = Parameter>,
) -> ScriptConditionResult {
    let context = RefCell::new(ScriptContext::at_frame(0));
    let mut condition = Condition::new(kind);
    for param in params {
        condition.add_parameter(param).unwrap();
    }
    ScriptConditionEvaluator::new(engine, &context)
        .evaluate_condition(&mut condition)
        .unwrap()
}

struct Headless;

impl ScriptExecutionDriver<ScriptEngine> for Headless {
    fn after_action(&mut self) -> gamelogic::GameLogicResult<()> {
        Ok(())
    }

    fn initialize_named_cache(&mut self) -> Option<()> {
        Some(())
    }
}

#[test]
fn authored_integer_timer_uses_the_original_unsigned_random_span() {
    let services = new_world_services();
    let engine = ScriptEngine::new_with_services(&services).unwrap();
    let stream = SessionRandom::default();
    stream.seed_random(37);
    let reference = SessionRandom::from_state(stream.snapshot());
    let reversed = 15i32.wrapping_add((reference.next_u32() % 4_294_967_288) as i32);
    let equal = 7;
    reference.next_u32();
    let following = 20 + (reference.next_u32() % 11) as i32;

    let cases = [
        (15, 6, reversed),
        (7, 6, 6),
        (i32::MIN, i32::MAX, i32::MAX),
        (7, 7, equal),
        (20, 30, following),
    ];
    with_world_services(&services, || {
        with_session_random(&stream, || {
            for (low, high, expected) in cases {
                run(
                    &engine,
                    ScriptActionType::SetRandomTimer,
                    [
                        counter_name("result"),
                        Parameter::with_int(ParameterType::Int, low),
                        Parameter::with_int(ParameterType::Int, high),
                    ],
                );
                assert_eq!(
                    engine.get_counter("result").unwrap().value,
                    expected,
                    "{low}..={high}"
                );
            }
        })
    });
    assert_eq!(stream.seed_words(), reference.seed_words());
}

#[test]
fn authored_restart_only_resumes_a_positive_stopped_timer() {
    let engine = ScriptEngine::new_with_services(&new_world_services()).unwrap();
    for (name, value, active) in [
        ("zero", 0, false),
        ("expired", -1, true),
        ("next", 2, false),
    ] {
        engine.set_timer(name, value).unwrap();
        if !active {
            engine.stop_timer(name).unwrap();
        }
        run(
            &engine,
            ScriptActionType::RestartTimer,
            [counter_name(name)],
        );
        assert_eq!(
            engine.get_counter(name).unwrap().is_countdown_timer,
            active || value > 0,
            "{name}"
        );
    }
    engine
        .update_with_driver(ScriptContext::at_frame(1), &mut Headless)
        .unwrap();
    assert_eq!(engine.get_counter("zero").unwrap().value, 0);
    assert_eq!(engine.get_counter("expired").unwrap().value, -1);
    assert_eq!(engine.get_counter("next").unwrap().value, 1);
}

#[test]
fn authored_millisecond_timers_round_the_signed_duration_in_original_frame_units() {
    let engine = ScriptEngine::new_with_services(&new_world_services()).unwrap();
    let real = |seconds| Parameter::with_real(ParameterType::Real, seconds);
    for (kind, seconds, expected) in [
        (ScriptActionType::SubFromMsecTimer, 0.12, -1),
        (ScriptActionType::AddToMsecTimer, -0.12, -1),
        (ScriptActionType::SubFromMsecTimer, -0.12, 6),
        (ScriptActionType::AddToMsecTimer, 1.0, 33),
    ] {
        engine.set_counter("paused", 2).unwrap();
        run(&engine, kind, [real(seconds), counter_name("paused")]);
        let counter = engine.get_counter("paused").unwrap();
        assert_eq!(counter.value, expected, "{kind:?} {seconds}");
        assert!(!counter.is_countdown_timer);
    }
    run(
        &engine,
        ScriptActionType::SetMillisecondTimer,
        [counter_name("signed"), real(-0.12)],
    );
    assert_eq!(engine.get_counter("signed").unwrap().value, -3);
    assert!(engine.get_counter("signed").unwrap().is_countdown_timer);
}

#[test]
fn authored_conditions_admit_literal_names_in_evaluation_order_and_xfer_them() {
    let services = new_world_services();
    let mut source = ScriptEngine::new_with_services(&services).unwrap();
    let peer = ScriptEngine::new_with_services(&new_world_services()).unwrap();
    source.set_external_eval_context(Some("StealthGeneral".into()), None);
    source.set_flag("StealthGeneral", true).unwrap();
    peer.set_counter("Score", 19).unwrap();

    let score = || {
        [
            counter_name("Score"),
            Parameter::with_int(ParameterType::Comparison, ComparisonType::Equal as i32),
            Parameter::with_int(ParameterType::Int, 0),
        ]
    };
    assert_eq!(
        evaluate(
            &source,
            ConditionType::TimerExpired,
            [counter_name("Deadline")]
        ),
        ScriptConditionResult::False
    );
    assert_eq!(
        evaluate(&source, ConditionType::Counter, score()),
        ScriptConditionResult::True
    );
    assert_eq!(
        evaluate(&source, ConditionType::Counter, score()),
        ScriptConditionResult::True
    );
    assert_eq!(
        evaluate(&peer, ConditionType::Counter, score()),
        ScriptConditionResult::False
    );
    assert_eq!(
        source.snapshot_named_trackers().0,
        [("Deadline".into(), 0, false), ("Score".into(), 0, false)]
    );

    let flag = |name: &str, expected| {
        [
            Parameter::with_string(ParameterType::Flag, name.into()),
            Parameter::with_int(ParameterType::Boolean, expected),
        ]
    };
    assert_eq!(
        evaluate(&source, ConditionType::Flag, flag(THIS_PLAYER, 0)),
        ScriptConditionResult::True
    );
    source.signal_ui_interact("Pressed");
    assert_eq!(
        evaluate(&source, ConditionType::Flag, flag("Pressed", 1)),
        ScriptConditionResult::True
    );
    assert_eq!(
        source.snapshot_named_trackers().1,
        [
            ("StealthGeneral".into(), true),
            (THIS_PLAYER.into(), false),
            ("Pressed".into(), false)
        ]
    );
    assert_eq!(
        peer.snapshot_named_trackers(),
        (vec![("Score".into(), 19, false)], vec![])
    );

    let snapshot = source.snapshot_named_trackers();
    let mut bytes = Cursor::new(Vec::new());
    with_world_services(&services, || source.xfer(&mut XferSave::new(&mut bytes, 1))).unwrap();
    let restore_services = new_world_services();
    let mut restored = ScriptEngine::new_with_services(&restore_services).unwrap();
    with_world_services(&restore_services, || {
        restored.xfer(&mut XferLoad::new(Cursor::new(bytes.get_ref()), 1))
    })
    .unwrap();
    assert_eq!(restored.snapshot_named_trackers(), snapshot);
    assert_eq!(restored.allocate_counter("Next").unwrap(), 3);
    assert_eq!(restored.allocate_flag("NextFlag").unwrap(), 4);
}
