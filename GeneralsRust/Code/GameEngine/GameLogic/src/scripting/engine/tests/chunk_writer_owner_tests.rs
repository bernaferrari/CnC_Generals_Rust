//! Scripts.cpp544–567/1627/2387/2619: one driving catalog, unchanged chunk format.
use super::*;
use crate::common::Dict;
use crate::scripting::chunk_codec::ScriptTemplateLookup;
use crate::scripting::core::{ScriptListReadInfo, parse_player_scripts_list_chunk};
use crate::sides_list::SidesList;
use game_engine::common::system::{DataChunkInfo, DataChunkInput, DataChunkOutput};

fn private_engine() -> ScriptEngine {
    let mut engine = ScriptEngine::new().unwrap();
    engine.with_inner_mut(|inner| {
        inner.condition_templates[ConditionType::ConditionTrue as usize]
            .base
            .internal_name_key = NameKeyGenerator::name_to_key("OWNER41_PRIVATE_TRUE");
        for (kind, name) in [
            (ScriptActionType::Victory, "OWNER41_PRIVATE_VICTORY"),
            (ScriptActionType::Defeat, "OWNER41_PRIVATE_DEFEAT"),
        ] {
            inner.action_templates[kind as usize].base.internal_name_key =
                NameKeyGenerator::name_to_key(name);
        }
    });
    engine
}
fn write_conditions(
    node: Option<&Condition>,
    output: &mut DataChunkOutput,
    catalog: &ScriptTemplateLookup,
) {
    Condition::write_condition_data_chunk(node, output, catalog);
}
fn write_actions(
    node: Option<&ScriptAction>,
    output: &mut DataChunkOutput,
    label: &str,
    catalog: &ScriptTemplateLookup,
) {
    ScriptAction::write_action_data_chunk(node, output, label, catalog);
}
fn script(name: &str) -> Box<Script> {
    let mut script = Script::new();
    script.script_name = name.into();
    script.comment = "comment".into();
    script.condition_comment = "conditions".into();
    script.action_comment = "actions".into();
    script.is_one_shot = false;
    script.easy = false;
    script.is_subroutine = true;
    script.delay_evaluation_seconds = 7;
    let mut condition = Condition::new(ConditionType::ConditionTrue);
    condition.next_and_condition = Some(Box::new(Condition::new(ConditionType::ConditionFalse)));
    let mut or = OrCondition::new();
    or.first_and = Some(Box::new(condition));
    let mut next = OrCondition::new();
    next.first_and = Some(Box::new(Condition::new(ConditionType::ConditionTrue)));
    or.next_or = Some(Box::new(next));
    script.condition = Some(Box::new(or));
    for kind in [ScriptActionType::Victory, ScriptActionType::Defeat] {
        script.append_action(Box::new(ScriptAction::new(kind)));
    }
    for kind in [ScriptActionType::Defeat, ScriptActionType::Victory] {
        script.append_action_false(Box::new(ScriptAction::new(kind)));
    }
    Box::new(script)
}
fn graph() -> ScriptList {
    let mut list = ScriptList::new();
    let mut first = script("direct");
    first.next_script = Some(script("sibling"));
    list.first_script = Some(first);
    let mut first = ScriptGroup::new();
    first.group_name = "group-one".into();
    first.is_group_active = false;
    first.is_group_subroutine = true;
    first.first_script = Some(script("nested-one"));
    let mut second = ScriptGroup::new();
    second.group_name = "group-two".into();
    second.first_script = Some(script("nested-two"));
    first.next_group = Some(Box::new(second));
    list.first_group = Some(Box::new(first));
    list
}
fn encode(
    engine: &ScriptEngine,
    list: &ScriptList,
    catalog: &ScriptTemplateLookup,
    sides: bool,
) -> Vec<u8> {
    engine.with_active(|| {
        let mut output = DataChunkOutput::new();
        if sides {
            let mut sides = SidesList::new();
            sides.add_side(&Dict::new());
            sides
                .get_side_info_mut(0)
                .unwrap()
                .set_script_list(Some(Box::new(list.clone())));
            sides.write_sides_data_chunk(&mut output, catalog);
        } else {
            let empty = ScriptList::new();
            ScriptList::write_scripts_data_chunk(
                &mut output,
                &[Some(list), None, Some(&empty)],
                catalog,
            );
        }
        output.into_ckmp_bytes()
    })
}
fn sides_scripts(
    input: &mut DataChunkInput,
    info: &DataChunkInfo,
    state: &mut dyn std::any::Any,
) -> bool {
    assert_eq!(info.version, 3);
    assert_eq!(input.read_int(), 1);
    assert_eq!(input.read_dict().get_pair_count(), 0);
    assert_eq!(input.read_int(), 0); // No build-list entries.
    assert_eq!(input.read_int(), 0); // No team records before post-write validation.
    input.register_parser(
        "PlayerScriptsList",
        "SidesList",
        parse_player_scripts_list_chunk,
    );
    input.parse(state)
}
fn decode(bytes: Vec<u8>, catalog: ScriptTemplateLookup, sides: bool) -> Vec<Box<ScriptList>> {
    let mut input = DataChunkInput::new(bytes);
    assert!(input.is_valid_file_type());
    input.register_parser(
        if sides {
            "SidesList"
        } else {
            "PlayerScriptsList"
        },
        "",
        if sides {
            sides_scripts
        } else {
            parse_player_scripts_list_chunk
        },
    );
    let mut read = ScriptListReadInfo::with_templates(catalog);
    assert!(input.parse(&mut read));
    read.lists
}
fn check_script(script: &Script, name: &str) {
    assert_eq!(script.script_name, name);
    assert_eq!(
        (
            &script.comment[..],
            &script.condition_comment[..],
            &script.action_comment[..]
        ),
        ("comment", "conditions", "actions")
    );
    assert_eq!(script.delay_evaluation_seconds, 7);
    assert_eq!(
        (
            script.is_active,
            script.is_one_shot,
            script.easy,
            script.normal,
            script.hard,
            script.is_subroutine
        ),
        (true, false, false, true, true, true)
    );
    let or = script.condition.as_deref().unwrap();
    let condition = or.first_and.as_deref().unwrap();
    assert_eq!(condition.condition_type, ConditionType::ConditionTrue);
    assert_eq!(
        condition
            .next_and_condition
            .as_deref()
            .unwrap()
            .condition_type,
        ConditionType::ConditionFalse
    );
    assert_eq!(
        or.next_or
            .as_deref()
            .unwrap()
            .first_and
            .as_deref()
            .unwrap()
            .condition_type,
        ConditionType::ConditionTrue
    );
    for (first, a, b) in [
        (
            &script.action,
            ScriptActionType::Victory,
            ScriptActionType::Defeat,
        ),
        (
            &script.action_false,
            ScriptActionType::Defeat,
            ScriptActionType::Victory,
        ),
    ] {
        let first = first.as_deref().unwrap();
        assert_eq!(first.action_type, a);
        assert_eq!(first.next_action.as_deref().unwrap().action_type, b);
        assert!(first.next_action.as_deref().unwrap().next_action.is_none());
    }
}
fn check_graph(list: &ScriptList) {
    let script = list.first_script.as_deref().unwrap();
    check_script(script, "direct");
    check_script(script.next_script.as_deref().unwrap(), "sibling");
    assert!(script.next_script.as_deref().unwrap().next_script.is_none());
    let group = list.first_group.as_deref().unwrap();
    assert_eq!(group.group_name, "group-one");
    assert_eq!(
        (group.is_group_active, group.is_group_subroutine),
        (false, true)
    );
    check_script(group.first_script.as_deref().unwrap(), "nested-one");
    let next = group.next_group.as_deref().unwrap();
    assert_eq!(next.group_name, "group-two");
    check_script(next.first_script.as_deref().unwrap(), "nested-two");
    assert!(next.next_group.is_none());
}
#[test]
fn private_writer_catalog_survives_foreign_active_engine_in_all_branches() {
    let engine = private_engine();
    let catalog = ScriptTemplateLookup::from_engine(&engine);
    let foreign = ScriptEngine::new().unwrap();
    let list = graph();
    for sides in [false, true] {
        let bytes = encode(&foreign, &list, &catalog, sides);
        let decoded = decode(bytes, catalog.clone(), sides);
        assert_eq!(decoded.len(), if sides { 1 } else { 3 });
        check_graph(&decoded[0]);
        if !sides {
            for empty in &decoded[1..] {
                assert!(empty.first_script.is_none() && empty.first_group.is_none());
            }
        }
    }
}
#[test]
fn retained_catalog_outlives_engine_and_is_stable_across_nested_scopes() {
    let engine = private_engine();
    let catalog = ScriptTemplateLookup::from_engine(&engine);
    drop(engine);
    let first = ScriptEngine::new().unwrap();
    let second = ScriptEngine::new().unwrap();
    let list = graph();
    first.with_active(|| {
        for sides in [false, true] {
            let before = encode(&first, &list, &catalog, sides);
            let nested = second.with_active(|| encode(&second, &list, &catalog, sides));
            let after = encode(&first, &list, &catalog, sides);
            assert_eq!(before, nested);
            assert_eq!(before, after);
            check_graph(&decode(after, catalog.clone(), sides)[0]);
        }
    });
}
fn leaf_bytes(catalog: &ScriptTemplateLookup, action: bool) -> Vec<u8> {
    let mut output = DataChunkOutput::new();
    if action {
        write_actions(
            Some(&ScriptAction::new(ScriptActionType::Victory)),
            &mut output,
            "ScriptActionFalse",
            catalog,
        );
    } else {
        write_conditions(
            Some(&Condition::new(ConditionType::ConditionTrue)),
            &mut output,
            catalog,
        );
    }
    output.into_ckmp_bytes()
}
fn expected_leaf(key: u32, action: bool) -> Vec<u8> {
    let mut output = DataChunkOutput::new();
    output.open_data_chunk(
        if action {
            "ScriptActionFalse"
        } else {
            "Condition"
        },
        if action { 2 } else { 4 },
    );
    output.write_int(if action {
        ScriptActionType::Victory as i32
    } else {
        ConditionType::ConditionTrue as i32
    });
    output.write_name_key(key);
    output.write_int(0);
    output.close_data_chunk();
    output.into_ckmp_bytes()
}
#[test]
fn missing_template_writes_cpp_bogus_fallback() {
    let catalog = ScriptTemplateLookup::default();
    for action in [false, true] {
        assert_eq!(
            leaf_bytes(&catalog, action),
            expected_leaf(NameKeyGenerator::name_to_key("Bogus"), action)
        );
    }
}
#[test]
fn present_zero_key_is_not_a_missing_template() {
    let catalog = ScriptTemplateLookup::from_keys(
        vec![0; ConditionType::ConditionTrue as usize + 1],
        vec![0; ScriptActionType::Victory as usize + 1],
    );
    for action in [false, true] {
        assert_eq!(leaf_bytes(&catalog, action), expected_leaf(0, action));
    }
}
#[test]
fn valid_template_does_not_intern_cpp_bogus_fallback() {
    std::thread::spawn(|| {
        let engine = ScriptEngine::new().unwrap();
        let catalog = ScriptTemplateLookup::from_engine(&engine);
        let before = NameKeyGenerator::name_to_key("OWNER41_BEFORE_VALID_WRITE");
        for action in [false, true] {
            let _ = leaf_bytes(&catalog, action);
        }
        let after = NameKeyGenerator::name_to_key("OWNER41_AFTER_VALID_WRITE");
        assert_eq!(
            after,
            before + 1,
            "C++ interns Bogus only for a missing template"
        );
    })
    .join()
    .unwrap();
}
#[test]
fn retained_chunk_writer_wire_trace() {
    let engine = ScriptEngine::new().unwrap();
    let catalog = ScriptTemplateLookup::from_engine(&engine);
    for sides in [false, true] {
        let bytes = encode(&engine, &graph(), &catalog, sides);
        eprintln!(
            "owner41_wire_{sides}={}",
            bytes
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        );
        check_graph(&decode(bytes, catalog.clone(), sides)[0]);
    }
}

#[test]
fn populated_parameters_and_siblings_keep_cpp_chunk_bytes() {
    let engine = private_engine();
    let catalog = ScriptTemplateLookup::from_engine(&engine);
    let mut first = Condition::new(ConditionType::ConditionTrue);
    first
        .add_parameter(Parameter::with_int(ParameterType::Int, -73))
        .unwrap();
    let mut second = Condition::new(ConditionType::ConditionFalse);
    second
        .add_parameter(Parameter::with_real(ParameterType::Real, 2.5))
        .unwrap();
    first.next_and_condition = Some(Box::new(second));
    let mut actual = DataChunkOutput::new();
    write_conditions(Some(&first), &mut actual, &catalog);
    let mut expected = DataChunkOutput::new();
    let mut cursor = Some(&first);
    while let Some(node) = cursor {
        expected.open_data_chunk("Condition", 4);
        expected.write_int(node.condition_type as i32);
        expected.write_name_key(
            engine
                .get_condition_template(node.condition_type as usize)
                .unwrap()
                .base
                .internal_name_key,
        );
        expected.write_int(node.num_parms as i32);
        for param in node.parameters.iter().take(node.num_parms).flatten() {
            param.clone().write_parameter(&mut expected);
        }
        expected.close_data_chunk();
        cursor = node.next_and_condition.as_deref();
    }
    assert_eq!(actual.into_ckmp_bytes(), expected.into_ckmp_bytes());
    let mut first = ScriptAction::new(ScriptActionType::Victory);
    first
        .add_parameter(Parameter::with_int(ParameterType::Int, 41))
        .unwrap();
    let mut second = ScriptAction::new(ScriptActionType::Defeat);
    second
        .add_parameter(Parameter::with_string(
            ParameterType::TextString,
            "parameter".into(),
        ))
        .unwrap();
    first.next_action = Some(Box::new(second));
    for label in ["ScriptAction", "ScriptActionFalse"] {
        let mut actual = DataChunkOutput::new();
        write_actions(Some(&first), &mut actual, label, &catalog);
        let mut expected = DataChunkOutput::new();
        let mut cursor = Some(&first);
        while let Some(node) = cursor {
            expected.open_data_chunk(label, 2);
            expected.write_int(node.action_type as i32);
            expected.write_name_key(
                engine
                    .get_action_template(node.action_type as usize)
                    .unwrap()
                    .base
                    .internal_name_key,
            );
            expected.write_int(node.num_parms as i32);
            for param in node.parameters.iter().take(node.num_parms).flatten() {
                param.clone().write_parameter(&mut expected);
            }
            expected.close_data_chunk();
            cursor = node.next_action.as_deref();
        }
        assert_eq!(actual.into_ckmp_bytes(), expected.into_ckmp_bytes());
    }
}
