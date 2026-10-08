//! Parsed labels belong to the driving INI operation, not the process.
use super::*;

fn actions(ini: &mut INI) -> Vec<ScriptTemplate> {
    ini.take_script_template_definitions()
        .into_iter()
        .filter_map(|row| match row {
            ScriptTemplateDefinition::Action(template) => Some(template),
            ScriptTemplateDefinition::Condition(_) => None,
        })
        .collect()
}

fn parse(ini: &mut INI, ui: &str) {
    let source = format!("ScriptAction\nInternalName = OWNER_SHARED_ACTION\nUIName = {ui}\nEnd\n");
    ini.with_inline_source(&source, |ini| ini.parse_current_file())
        .unwrap();
}

#[test]
fn interleaved_parsers_keep_their_own_labels_for_the_same_name() {
    let mut a = INI::new();
    let mut b = INI::new();
    parse(&mut a, "OwnerA");
    parse(&mut b, "OwnerB");
    let find = |rows: Vec<ScriptTemplate>| {
        rows.into_iter()
            .find(|row| row.internal_name == "OWNER_SHARED_ACTION")
            .unwrap()
            .ui_name
    };
    assert_eq!(find(actions(&mut a)), "OwnerA");
    assert_eq!(find(actions(&mut b)), "OwnerB");
}

#[test]
fn draining_one_operation_does_not_replay_it_into_a_later_load() {
    let mut ini = INI::new();
    parse(&mut ini, "First");
    assert_eq!(
        actions(&mut ini)
            .into_iter()
            .filter(|row| row.internal_name == "OWNER_SHARED_ACTION")
            .count(),
        1
    );
    assert!(actions(&mut ini).is_empty());
    parse(&mut ini, "Second");
    let rows = actions(&mut ini);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].ui_name, "Second");
}
