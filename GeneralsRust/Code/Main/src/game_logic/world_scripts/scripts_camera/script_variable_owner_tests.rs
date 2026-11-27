use super::{GameLogic, script_execution_driver::HostScriptExecutionDriver};
use game_engine::common::random_value::{SessionRandom, with_session_random};
use gamelogic::scripting::executor::ScriptContext;
use generals_sim::scripting::program::{Parameter, ParameterType, ScriptAction, ScriptActionType};

fn random_timer(name: &str, low: i32, high: i32) -> ScriptAction {
    let mut action = ScriptAction::new(ScriptActionType::SetRandomTimer);
    action
        .add_parameter(Parameter::with_string(ParameterType::Counter, name.into()))
        .unwrap();
    for bound in [low, high] {
        action
            .add_parameter(Parameter::with_int(ParameterType::Int, bound))
            .unwrap();
    }
    action
}

#[test]
fn authored_random_timers_draw_from_the_receiving_world_in_order() {
    let mut receiving = GameLogic::new();
    let peer = GameLogic::new();
    receiving.logic_random.seed_random(37);
    peer.logic_random.seed_random(93);
    let expected = SessionRandom::from_state(receiving.logic_random.snapshot());
    let peer_before = peer.logic_random.seed_words();
    let engine = receiving.owned_script_engine.take().unwrap();

    let frames = random_timer("frames", 3, 90);
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
    let random_seconds = expected.next_real(0.1, 3.0);
    assert_eq!(
        engine.get_counter("seconds").unwrap().value,
        game_engine::base_type::real_to_int_ceil((random_seconds * 1000.0) * 0.03)
    );
    assert_eq!(engine.get_counter("fixed").unwrap().value, i32::MIN);
    assert_eq!(receiving.logic_random.seed_words(), expected.seed_words());
}

#[test]
fn reversed_timer_draw_continues_after_world_snapshot_restore() {
    use crate::save_load::SnapshotBuilder;

    let mut source = GameLogic::new();
    let peer = GameLogic::new();
    source.set_logic_random_seed(37);
    peer.logic_random.seed_random(93);
    let expected = SessionRandom::from_state(source.logic_random.snapshot());
    let peer_before = peer.logic_random.seed_words();
    let run = |world: &mut GameLogic, action: &ScriptAction| {
        world
            .with_owned_script_engine(|engine, receiving| {
                let mut driver = HostScriptExecutionDriver::with_interpreter(receiving, engine);
                with_session_random(&peer.logic_random, || {
                    engine
                        .friend_execute_action_with_driver_checked(
                            action,
                            None,
                            ScriptContext::at_frame(0),
                            &mut driver,
                        )
                        .unwrap();
                });
            })
            .unwrap();
    };
    let timer = |world: &GameLogic, name: &str| {
        world
            .owned_script_engine
            .as_ref()
            .unwrap()
            .get_counter(name)
            .unwrap()
            .value
    };

    let mut first = random_timer("reversed", 15, 6);
    first.next_action = Some(Box::new(random_timer("zero span", 7, 6)));
    run(&mut source, &first);
    let reversed = 15i32.wrapping_add((expected.next_u32() % 4_294_967_288) as i32);
    assert_eq!(timer(&source, "reversed"), reversed);
    assert_eq!(timer(&source, "zero span"), 6);
    assert_eq!(source.logic_random.seed_words(), expected.seed_words());

    let builder = SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&source).unwrap();
    let encoded = bincode_legacy::serialize(&saved).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&encoded).unwrap();
    let mut restored = GameLogic::new();
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert_eq!(timer(&restored, "reversed"), reversed);
    assert_eq!(timer(&restored, "zero span"), 6);
    assert_eq!(restored.logic_random.seed_words(), expected.seed_words());

    let following = 20 + (expected.next_u32() % 11) as i32;
    let next = random_timer("following", 20, 30);
    for world in [&mut source, &mut restored] {
        run(world, &next);
        assert_eq!(timer(world, "following"), following);
        assert_eq!(world.logic_random.seed_words(), expected.seed_words());
    }
    assert_eq!(peer.logic_random.seed_words(), peer_before);
}
