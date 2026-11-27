use super::{GameLogic, script_execution_driver::HostScriptExecutionDriver};
use game_engine::common::random_value::{SessionRandom, with_session_random};
use gamelogic::scripting::executor::ScriptContext;
use generals_sim::scripting::program::{Parameter, ParameterType, ScriptAction, ScriptActionType};

#[test]
fn authored_random_timers_draw_from_the_receiving_world_in_order() {
    let mut receiving = GameLogic::new();
    let peer = GameLogic::new();
    receiving.logic_random.seed_random(37);
    peer.logic_random.seed_random(93);
    let expected = SessionRandom::from_state(receiving.logic_random.snapshot());
    let peer_before = peer.logic_random.seed_words();
    let engine = receiving.owned_script_engine.take().unwrap();

    let mut frames = ScriptAction::new(ScriptActionType::SetRandomTimer);
    frames
        .add_parameter(Parameter::with_string(
            ParameterType::Counter,
            "frames".into(),
        ))
        .unwrap();
    for bound in [3, 90] {
        frames
            .add_parameter(Parameter::with_int(ParameterType::Int, bound))
            .unwrap();
    }
    let mut seconds = ScriptAction::new(ScriptActionType::SetRandomMsecTimer);
    seconds
        .add_parameter(Parameter::with_string(
            ParameterType::Counter,
            "seconds".into(),
        ))
        .unwrap();
    for bound in [0.1, 3.0] {
        seconds
            .add_parameter(Parameter::with_real(ParameterType::Real, bound))
            .unwrap();
    }
    let mut equal_bounds = ScriptAction::new(ScriptActionType::SetRandomMsecTimer);
    equal_bounds
        .add_parameter(Parameter::with_string(
            ParameterType::Counter,
            "fixed".into(),
        ))
        .unwrap();
    for _ in 0..2 {
        equal_bounds
            .add_parameter(Parameter::with_real(ParameterType::Real, f32::INFINITY))
            .unwrap();
    }

    with_session_random(&peer.logic_random, || {
        let mut driver = HostScriptExecutionDriver::with_interpreter(&mut receiving, &engine);
        for action in [&frames, &seconds, &equal_bounds] {
            engine
                .friend_execute_action_with_driver_checked(
                    action,
                    None,
                    ScriptContext::at_frame(0),
                    &mut driver,
                )
                .unwrap();
        }
    });

    assert_eq!(
        peer.logic_random.seed_words(),
        peer_before,
        "borrowed script advanced the peer stream"
    );
    assert_eq!(
        engine.get_counter("frames").unwrap().value,
        expected.next_int(3, 90)
    );
    assert_eq!(
        engine.get_counter("seconds").unwrap().value,
        (expected.next_real(0.1, 3.0) * 30.0).ceil() as i32
    );
    assert_eq!(engine.get_counter("fixed").unwrap().value, i32::MAX);
    assert_eq!(receiving.logic_random.seed_words(), expected.seed_words());
}
