use super::*;
#[test]
fn sequence_capsule_without_team_calibrates() {
    let engine = ScriptEngine::new().unwrap();
    let records = vec![SequentialScriptSnapshot {
        script_name: "Sequence".into(),
        ..Default::default()
    }];
    engine.restore_sequential_scripts(&records);
    assert_eq!(engine.snapshot_sequential_scripts(), records);
}
#[test]
fn sequence_capsule_keeps_exact_reference_before_owner_binding() {
    let engine = ScriptEngine::new().unwrap();
    let records = vec![SequentialScriptSnapshot {
        team_id: 90,
        script_name: "Sequence".into(),
        ..Default::default()
    }];
    engine.restore_sequential_scripts(&records);
    assert_eq!(engine.snapshot_sequential_scripts(), records);
}
