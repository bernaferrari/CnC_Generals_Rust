//! Actual registered INI -> owned engine boundary, from ScriptEngine.cpp:340-416.
use super::*;
use crate::scripting::core::{ConditionType, ScriptActionType};
use game_engine::common::ini::INIError;

fn action(engine: &ScriptEngine) -> Template {
    engine
        .get_action_template(ScriptActionType::SetFlag as usize)
        .unwrap()
        .base
}

fn source(ui: &str) -> String {
    format!("ScriptAction\nInternalName = SET_FLAG\nUIName = {ui}\nEnd\n")
}

fn assert_nonlabel_fields(before: &Template, after: &Template) {
    assert_eq!(before.internal_name, after.internal_name);
    assert_eq!(before.internal_name_key, after.internal_name_key);
    assert_eq!(before.num_parameters, after.num_parameters);
    assert_eq!(before.parameters, after.parameters);
    assert_eq!(before.num_ui_strings, after.num_ui_strings);
    assert_eq!(before.ui_strings, after.ui_strings);
}

#[test]
fn labels_apply_in_source_order_without_altering_authored_fields() {
    let mut engine = ScriptEngine::new().unwrap();
    let before = action(&engine);
    let text = format!(
        "{}ScriptAction\nInternalName = SET_FLAG\nUIName2 = Secondary\nHelpText = Help\nEnd\n",
        source("First")
    );
    engine.parse_template_labels(&text).unwrap();
    let after = action(&engine);
    assert_eq!(after.ui_name, "UNUSED/(placeholder)/placeholder");
    assert_eq!(after.ui_name2, "Secondary");
    assert_eq!(after.help_text, "Help");
    assert_nonlabel_fields(&before, &after);
    assert_eq!(before.internal_name, "SET_FLAG");
}

#[test]
fn exact_case_and_unknown_names_do_not_change_authored_templates() {
    let mut engine = ScriptEngine::new().unwrap();
    let before = action(&engine);
    engine.parse_template_labels("ScriptAction\nInternalName = set_flag\nUIName = Wrong\nEnd\nScriptAction\nInternalName = UNKNOWN_TEMPLATE\nUIName = Unknown\nEnd\n").unwrap();
    let after = action(&engine);
    assert_eq!(before.ui_name, after.ui_name);
    assert_eq!(before.ui_name2, after.ui_name2);
    assert_eq!(before.help_text, after.help_text);
    assert_nonlabel_fields(&before, &after);
}

#[test]
fn condition_labels_use_the_condition_catalog() {
    let mut engine = ScriptEngine::new().unwrap();
    let index = ConditionType::ConditionTrue as usize;
    let before = engine.get_condition_template(index).unwrap().base;
    let text = format!(
        "ScriptCondition\nInternalName = {}\nUIName = Always\nEnd\n",
        before.internal_name
    );
    engine.parse_template_labels(&text).unwrap();
    let after = engine.get_condition_template(index).unwrap().base;
    assert_eq!(after.ui_name, "Always");
    assert_nonlabel_fields(&before, &after);
}

#[test]
fn empty_name_changes_only_the_first_unused_condition_slot() {
    let mut engine = ScriptEngine::new().unwrap();
    let slots: Vec<_> = (0..ConditionType::NumItems as usize)
        .filter(|&i| {
            engine
                .get_condition_template(i)
                .unwrap()
                .base
                .internal_name
                .is_empty()
        })
        .collect();
    assert!(slots.len() > 1);
    let later = engine.get_condition_template(slots[1]).unwrap().base;
    engine
        .parse_template_labels("ScriptCondition\nUIName = EmptyName\nEnd\n")
        .unwrap();
    assert_eq!(
        engine
            .get_condition_template(slots[0])
            .unwrap()
            .base
            .ui_name,
        "EmptyName"
    );
    assert_eq!(
        engine
            .get_condition_template(slots[1])
            .unwrap()
            .base
            .ui_name,
        later.ui_name
    );
}

#[test]
fn interleaved_engines_constructor_reset_and_restore_keep_owner_labels() {
    let _serial = crate::test_sync::lock();
    let mut a = ScriptEngine::new().unwrap();
    a.parse_template_labels(&source("OwnerA")).unwrap();
    let mut b = ScriptEngine::new().unwrap();
    assert_eq!(action(&a).ui_name, "OwnerA");
    assert_ne!(action(&b).ui_name, "OwnerA");
    b.parse_template_labels(&source("OwnerB")).unwrap();
    assert_eq!(action(&a).ui_name, "OwnerA");
    assert_eq!(action(&b).ui_name, "OwnerB");
    let runtime = a.snapshot_xfer_tail();
    a.reset();
    a.restore_xfer_tail(&runtime);
    b.restore_xfer_tail(&runtime);
    assert_eq!(action(&a).ui_name, "OwnerA");
    assert_eq!(action(&b).ui_name, "OwnerB");
    assert_eq!(a.snapshot_xfer_tail(), b.snapshot_xfer_tail());
}

#[test]
fn later_parse_error_keeps_completed_block_but_not_incomplete_block() {
    let mut engine = ScriptEngine::new().unwrap();
    let text = format!(
        "{}ScriptAction\nInternalName = SET_FLAG\nUIName = Broken\nUnknown = 1\nEnd\n",
        source("Completed")
    );
    assert_eq!(
        engine.parse_template_labels(&text),
        Err(INIError::UnknownToken)
    );
    assert_eq!(action(&engine).ui_name, "Completed");
}
