//! C++ ScriptEngine::init: exact authored names, empty slots, and key order.
use super::*;

fn cpp_rows(kind: &str) -> impl Iterator<Item = (usize, &'static str)> + '_ {
    include_str!("authored_template_names.txt")
        .lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(move |line| {
            let mut parts = line.split('|');
            let row_kind = parts.next().unwrap();
            let ordinal = parts.next().unwrap().parse().unwrap();
            let name = parts.next().unwrap();
            (row_kind == kind).then_some((ordinal, name))
        })
}

fn assert_rows(engine: &ScriptEngine, kind: &str) {
    let mut count = 0;
    for (ordinal, name) in cpp_rows(kind) {
        let base = if kind == "action" {
            engine.get_action_template(ordinal).unwrap().base
        } else {
            engine.get_condition_template(ordinal).unwrap().base
        };
        assert_eq!(base.internal_name, name, "{kind} slot {ordinal}");
        assert_eq!(
            base.internal_name_key,
            NameKeyGenerator::name_to_key(name),
            "{kind} slot {ordinal} name/key consistency"
        );
        count += 1;
    }
    assert_eq!(count, if kind == "action" { 344 } else { 109 });
}

#[test]
fn all_authored_action_names_and_keys_match_cpp() {
    assert_rows(&ScriptEngine::new().unwrap(), "action");
}

#[test]
fn all_authored_condition_names_and_keys_match_cpp() {
    assert_rows(&ScriptEngine::new().unwrap(), "condition");
}

#[test]
fn unsupported_slots_keep_empty_names_and_share_the_empty_name_key() {
    let engine = ScriptEngine::new().unwrap();
    let empty_key = NameKeyGenerator::name_to_key("");
    let mut empty_slots = 0;
    for kind in ["condition", "action"] {
        for (ordinal, name) in cpp_rows(kind).filter(|(_, name)| name.is_empty()) {
            let base = if kind == "action" {
                engine.get_action_template(ordinal).unwrap().base
            } else {
                engine.get_condition_template(ordinal).unwrap().base
            };
            assert_eq!(
                base.internal_name, name,
                "unsupported {kind} slot {ordinal}"
            );
            assert_eq!(base.internal_name_key, empty_key);
            empty_slots += 1;
        }
    }
    assert_eq!(empty_slots, 7);
}

#[test]
fn keys_are_interned_in_cpp_condition_then_action_ordinal_order() {
    // This test thread owns its registry. No engine/global registry is published.
    NameKeyGenerator::reset();
    let engine = ScriptEngine::new().unwrap();
    let mut assigned = HashMap::new();
    let mut next_key = 1;
    for kind in ["condition", "action"] {
        for (ordinal, name) in cpp_rows(kind) {
            let expected = *assigned.entry(name).or_insert_with(|| {
                let key = next_key;
                next_key += 1;
                key
            });
            let base = if kind == "action" {
                engine.get_action_template(ordinal).unwrap().base
            } else {
                engine.get_condition_template(ordinal).unwrap().base
            };
            assert_eq!(base.internal_name_key, expected, "{kind} slot {ordinal}");
            assert_eq!(
                NameKeyGenerator::key_to_name(expected).as_deref(),
                Some(name)
            );
        }
    }
}

#[test]
fn reset_and_runtime_restore_preserve_authored_definitions() {
    let _serial = crate::test_sync::lock();
    let mut engine = ScriptEngine::new().unwrap();
    let before = engine.snapshot_xfer_tail();
    engine.set_counter("runtime-only", 42).unwrap();
    engine.reset();
    engine.restore_xfer_tail(&before);
    assert_rows(&engine, "condition");
    assert_rows(&engine, "action");
    // C++ does not transfer the template catalog as mutable runtime state.
    let restored = ScriptEngine::new().unwrap();
    restored.restore_xfer_tail(&before);
    assert_rows(&restored, "condition");
    assert_rows(&restored, "action");
}
