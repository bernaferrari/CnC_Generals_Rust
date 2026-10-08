//! ScriptEngine.cpp:6483-6639,8923-8941 table behavior through production callers.
use super::*;
use game_engine::common::system::xfer_load::XferLoad;
use game_engine::common::system::xfer_save::XferSave;
use std::io::Cursor;

fn priority(engine: &ScriptEngine, name: &str) -> i32 {
    engine.get_attack_info(name).unwrap().default_priority
}

fn script_default(engine: &ScriptEngine, name: &str, value: i32) {
    let mut action = ScriptAction::new(ScriptActionType::SetDefaultAttackPriority);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::AttackPrioritySet,
            name.into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, value))
        .unwrap();
    engine.with_active(|| {
        let dispatch_state = std::cell::RefCell::new(ScriptContext::new());
        let mut dispatch = ScriptActionDispatcher::new(&engine, &dispatch_state);
        assert_eq!(
            dispatch.execute_action(&action).unwrap(),
            ScriptActionResult::Success
        );
    });
}

#[test]
fn default_missing_and_named_lookup_keep_cpp_row_order() {
    let _serial = crate::test_sync::lock();
    let engine = ScriptEngine::new().unwrap();
    assert_eq!(priority(&engine, ""), 1);
    assert_eq!(engine.get_attack_info("Missing").unwrap().get_name(), "");
    assert!(engine.set_priority_default("Zulu", 7));
    assert!(engine.set_priority_default("Alpha", 9));
    assert_eq!(priority(&engine, "Zulu"), 7);
    assert_eq!(priority(&engine, "Missing"), 1);
    let rows = engine.snapshot_xfer_tail().attack_priorities;
    assert_eq!(
        rows.iter().map(|r| (r.0.as_str(), r.1)).collect::<Vec<_>>(),
        vec![("", 1), ("Zulu", 7), ("Alpha", 9)]
    );
}

#[test]
fn empty_named_row_is_distinct_from_canonical_default() {
    let _serial = crate::test_sync::lock();
    let engine = ScriptEngine::new().unwrap();
    // C++ findAttackInfo searches from1 even for an empty input name.
    assert!(engine.set_priority_default("", 77));
    assert_eq!(priority(&engine, ""), 77);
    assert_eq!(priority(&engine, "Missing"), 1);
    assert_eq!(
        engine
            .snapshot_xfer_tail()
            .attack_priorities
            .iter()
            .map(|r| r.1)
            .collect::<Vec<_>>(),
        vec![1, 77]
    );
}

#[test]
fn capacity_rejects_new_rows_but_updates_existing_rows() {
    let _serial = crate::test_sync::lock();
    let engine = ScriptEngine::new().unwrap();
    for i in 1..MAX_ATTACK_PRIORITIES {
        assert!(engine.set_priority_default(&format!("Row{i}"), i as i32));
    }
    assert!(!engine.set_priority_default("Overflow", 999));
    assert_eq!(priority(&engine, "Overflow"), 1);
    assert!(engine.set_priority_default("Row1", -5));
    assert_eq!(priority(&engine, "Row1"), -5);
    assert_eq!(
        engine.snapshot_xfer_tail().attack_priorities.len(),
        MAX_ATTACK_PRIORITIES
    );
}

#[test]
fn live_script_edits_update_existing_definition_without_new_row() {
    let _serial = crate::test_sync::lock();
    let engine = ScriptEngine::new().unwrap();
    script_default(&engine, "Live", 4);
    let old_snapshot = engine.get_attack_info("Live").unwrap();
    script_default(&engine, "Live", 13);
    assert_eq!(priority(&engine, "Live"), 13);
    assert_eq!(
        old_snapshot.default_priority, 4,
        "public getter still returns an owned snapshot"
    );
    assert_eq!(engine.snapshot_xfer_tail().attack_priorities.len(), 2);
}

#[test]
fn independent_engine_construction_does_not_publish_or_replace_definitions() {
    let _serial = crate::test_sync::lock();
    let first = ScriptEngine::new().unwrap();
    script_default(&first, "SharedName", 4);
    first.with_active(|| {
        let second = ScriptEngine::new().unwrap();
        assert_eq!(
            with_active_script_engine_ref(|e| priority(e, "SharedName")),
            Some(4)
        );
        script_default(&second, "SharedName", 19);
        assert_eq!(priority(&second, "SharedName"), 19);
        assert_eq!(
            with_active_script_engine_ref(|e| priority(e, "SharedName")),
            Some(4)
        );
    });
    assert_eq!(priority(&first, "SharedName"), 4);
}

#[test]
fn reset_recreates_default_and_removes_named_rows() {
    let _serial = crate::test_sync::lock();
    let mut engine = ScriptEngine::new().unwrap();
    assert!(engine.set_priority_default("BeforeReset", 23));
    let before = engine.get_attack_info("BeforeReset").unwrap();
    engine.reset();
    assert_eq!(priority(&engine, "BeforeReset"), 1);
    assert_eq!(engine.snapshot_xfer_tail().attack_priorities.len(), 1);
    assert_eq!(before.default_priority, 23);
    assert!(engine.set_priority_default("AfterReset", 31));
    assert_eq!(priority(&engine, "AfterReset"), 31);
}

#[test]
fn host_tail_preserves_row_order_overrides_and_empty_restore_default() {
    let _serial = crate::test_sync::lock();
    let engine = ScriptEngine::new().unwrap();
    let mut tail = engine.snapshot_xfer_tail();
    tail.attack_priorities = vec![
        ("".into(), 1, vec![]),
        (
            "Zulu".into(),
            7,
            vec![("Tank".into(), 100), ("Worker".into(), -4)],
        ),
        ("Alpha".into(), 9, vec![]),
    ];
    engine.restore_xfer_tail(&tail);
    assert_eq!(
        engine.get_attack_info("Zulu").unwrap().get_priority("Tank"),
        100
    );
    assert_eq!(
        engine
            .get_attack_info("Zulu")
            .unwrap()
            .get_priority("Worker"),
        -4
    );
    assert_eq!(
        engine
            .get_attack_info("Zulu")
            .unwrap()
            .get_priority("Other"),
        7
    );
    assert_eq!(
        engine.snapshot_xfer_tail().attack_priorities,
        tail.attack_priorities
    );
    tail.attack_priorities.clear();
    engine.restore_xfer_tail(&tail);
    assert_eq!(
        engine.snapshot_xfer_tail().attack_priorities,
        vec![(String::new(), 1, vec![])]
    );
}

#[test]
fn base_xfer_round_trip_keeps_priority_wire_and_rng() {
    let _serial = crate::test_sync::lock();
    get_named_object_tracker().clear().unwrap();
    let before = game_engine::common::random_value::get_game_logic_random_seed_state();
    let mut engine = ScriptEngine::new().unwrap();
    // C++ load intentionally starts a fade when saved fade is None; use the
    // actual new-map phase so full-wire resave remains a valid invariant.
    engine.new_map();
    assert!(engine.set_priority_default("Zulu", 7));
    assert!(engine.set_priority_default("Alpha", -9));
    let mut saved = Cursor::new(Vec::new());
    engine.xfer(&mut XferSave::new(&mut saved, 1)).unwrap();
    let saved = saved.into_inner();
    let mut restored = ScriptEngine::new().unwrap();
    restored
        .xfer(&mut XferLoad::new(Cursor::new(saved.clone()), 1))
        .unwrap();
    assert_eq!(priority(&restored, "Zulu"), 7);
    assert_eq!(priority(&restored, "Alpha"), -9);
    assert_eq!(
        restored.snapshot_xfer_tail().attack_priorities,
        engine.snapshot_xfer_tail().attack_priorities
    );
    let mut resaved = Cursor::new(Vec::new());
    restored.xfer(&mut XferSave::new(&mut resaved, 1)).unwrap();
    assert_eq!(resaved.into_inner(), saved);
    assert_eq!(
        game_engine::common::random_value::get_game_logic_random_seed_state(),
        before
    );
    println!("priority_table_wire={saved:02x?}");
}
