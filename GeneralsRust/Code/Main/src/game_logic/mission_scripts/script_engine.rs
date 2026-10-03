// Platform initialization adapter; canonical scheduling/state lives in GameLogic.
pub use gamelogic::scripting::mission_runtime::MissionScriptRuntime;

fn mission_runtime_with_host_trigger_world(
    pending_script_enabled_updates: Arc<Mutex<Vec<(String, bool)>>>,
    host_trigger_world: Arc<Mutex<gamelogic::scripting::HostTriggerWorld>>,
) -> GameLogicResult<MissionScriptRuntime> {
    let _ = initialize_script_engine();
    let evaluator =
        ScriptEvaluator::new_with_host_trigger_world(get_script_engine(), host_trigger_world);
    Ok(MissionScriptRuntime::new(
        evaluator,
        pending_script_enabled_updates,
    ))
}
