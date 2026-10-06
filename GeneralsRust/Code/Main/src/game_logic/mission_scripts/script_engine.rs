// Passive host adapter; explicit map startup installs the canonical ScriptEngine.
pub use gamelogic::scripting::mission_runtime::MissionScriptRuntime;

fn mission_runtime_with_host_trigger_world(
    pending_script_enabled_updates: Arc<Mutex<Vec<(String, bool)>>>,
    host_trigger_world: Arc<Mutex<gamelogic::scripting::HostTriggerWorld>>,
) -> MissionScriptRuntime {
    // C++ GameLogic.cpp:199-246 constructs state; init():375-379 creates the
    // ScriptEngine later. Capturing the existing handle must not populate it.
    let evaluator =
        ScriptEvaluator::new_with_host_trigger_world(get_script_engine(), host_trigger_world);
    MissionScriptRuntime::new(evaluator, pending_script_enabled_updates)
}
