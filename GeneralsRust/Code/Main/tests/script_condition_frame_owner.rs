use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script,
};
use gamelogic::scripting::engine::{ScriptEngine, ScriptExecutionDriver};
use gamelogic::scripting::executor::ScriptContext;
use generals_main::game_logic::GameLogic;

struct NoopDriver;
impl ScriptExecutionDriver for NoopDriver {
    fn after_action(&mut self) -> gamelogic::GameLogicResult<()> {
        Ok(())
    }
}

fn supply_source_cached_result(world: &GameLogic, cache_frame: u32) -> bool {
    let mut condition = Condition::new(ConditionType::SupplySourceSafe);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "AbsentReceivingPlayer".into(),
        ))
        .expect("player parameter");
    condition
        .add_parameter(Parameter::with_int(ParameterType::Int, 0))
        .expect("minimum supply parameter");
    // A cached positive answer avoids consulting player/AI state. The only
    // question on this path is whether this world is inside the cache window.
    condition.custom_data = 1;
    condition.custom_frame = cache_frame;

    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(condition)));
    let mut script = Script::new();
    script.condition = Some(Box::new(branch));
    let engine = ScriptEngine::new().expect("script engine");
    let mut driver = NoopDriver;
    engine.evaluate_conditions_with_driver(
        &mut script,
        None,
        None,
        ScriptContext::at_frame(world.get_current_frame() as u32),
        &mut driver,
    )
}

#[test]
fn same_thread_world_frames_drive_cached_nested_condition_queries() {
    let mut owner = GameLogic::new();
    owner.set_current_frame(100);
    let mut peer = GameLogic::new();
    peer.set_current_frame(101);

    // The two same-thread owners observe adjacent frames around the exact
    // cached-through boundary. The owner still sees its cached true value;
    // the peer must re-evaluate after its own frame advances past the bound.
    assert!(supply_source_cached_result(&owner, 100));
    assert!(!supply_source_cached_result(&peer, 100));
}
