//! C++ ScriptConditions.cpp1421 and ScriptEngine.cpp7253: consume one exact
//! completion from the receiving interpreter; playback state is not a signal.
use super::*;

struct NoMediaCallbacks;
impl ScriptActionHandler for NoMediaCallbacks {}

fn video(name: &str) -> Condition {
    let mut condition = Condition::new(ConditionType::HasFinishedVideo);
    condition
        .add_parameter(Parameter::with_string(ParameterType::Movie, name.into()))
        .unwrap();
    condition
}

fn completed(engine: &ScriptEngine, name: &str) -> bool {
    let state = RefCell::new(ScriptContext::at_frame(0));
    ScriptConditionEvaluator::new(engine, &state)
        .evaluate_condition(&mut video(name))
        .unwrap()
        == ScriptConditionResult::True
}

#[test]
fn video_condition_consumes_only_its_receiving_list_even_with_handler_and_foreign_scope() {
    let mut owner = ScriptEngine::new().unwrap();
    owner.set_action_handler(Some(Arc::new(NoMediaCallbacks)));
    let peer = ScriptEngine::new().unwrap();
    peer.notify_of_completed_video("Briefing");
    assert!(!completed(&owner, "Briefing"));
    owner.notify_of_completed_video("Briefing");
    owner.notify_of_completed_video("Briefing");
    peer.with_active(|| {
        assert!(!completed(&owner, "briefing"), "C++ compares exact names");
        assert!(completed(&owner, "Briefing"));
        assert!(
            completed(&owner, "Briefing"),
            "one duplicate remains after first erase"
        );
        assert!(!completed(&owner, "Briefing"));
        assert!(
            peer.is_video_complete("Briefing", false),
            "foreign entry was never consumed"
        );
    });
    drop(peer);
    assert!(!completed(&owner, "Briefing"));
}

#[test]
fn video_completion_tail_and_reset_preserve_each_receiving_future() {
    let mut owner = ScriptEngine::new().unwrap();
    owner.notify_of_completed_video("Briefing");
    owner.notify_of_completed_video("Briefing");
    let tail = owner.snapshot_xfer_tail_with_named_objects(Vec::new());
    let restored = ScriptEngine::new().unwrap();
    restored.restore_receiving_xfer_tail(&tail);
    assert!(completed(&owner, "Briefing"));
    owner.reset();
    assert!(!completed(&owner, "Briefing"));
    assert!(completed(&restored, "Briefing"));
    assert!(completed(&restored, "Briefing"));
    assert!(!completed(&restored, "Briefing"));
    owner.notify_of_completed_video("Restarted");
    assert!(completed(&owner, "Restarted"));
    assert!(!completed(&restored, "Restarted"));
}
