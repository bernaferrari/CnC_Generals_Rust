//! Explicit decoder definitions remain independent of a foreign active engine.
use super::*;
use crate::scripting::core::{ScriptListReadInfo, parse_player_scripts_list_chunk};
use game_engine::common::system::{DataChunkInput, DataChunkOutput};

struct RestoreGlobal(Option<ScriptEngine>);
impl Drop for RestoreGlobal {
    fn drop(&mut self) {
        *get_script_engine().write().unwrap() = self.0.take();
    }
}

fn decode(engine: &mut ScriptEngine, condition: bool) -> ScriptList {
    let key = NameKeyGenerator::name_to_key(if condition {
        "PRIVATE_CODEC_TRUE"
    } else {
        "PRIVATE_CODEC_VICTORY"
    });
    engine.with_inner_mut(|inner| {
        if condition {
            inner.condition_templates[ConditionType::ConditionTrue as usize]
                .base
                .internal_name_key = key;
        } else {
            inner.action_templates[ScriptActionType::Victory as usize]
                .base
                .internal_name_key = key;
        }
    });
    let mut output = DataChunkOutput::new();
    output.open_data_chunk("PlayerScriptsList", 1);
    output.open_data_chunk("ScriptList", 1);
    output.open_data_chunk("Script", 2);
    for name in ["private", "", "", ""] {
        output.write_ascii_string(name);
    }
    for value in [1, 0, 1, 1, 1, 0] {
        output.write_byte(value);
    }
    output.write_int(0);
    if condition {
        output.open_data_chunk("OrCondition", 1);
    }
    output.open_data_chunk(
        if condition {
            "Condition"
        } else {
            "ScriptAction"
        },
        if condition { 4 } else { 2 },
    );
    // Deliberately stale numeric ordinal: C++ rematches the name key.
    output.write_int(if condition {
        ConditionType::ConditionFalse as i32
    } else {
        ScriptActionType::NoOp as i32
    });
    output.write_name_key(key);
    output.write_int(0);
    output.close_data_chunk();
    if condition {
        output.close_data_chunk();
    }
    output.close_data_chunk();
    output.close_data_chunk();
    output.close_data_chunk();
    let mut input = DataChunkInput::new(output.into_ckmp_bytes());
    assert!(input.is_valid_file_type());
    input.register_parser("PlayerScriptsList", "", parse_player_scripts_list_chunk);
    let templates = engine.script_template_lookup();
    let foreign = ScriptEngine::new().unwrap();
    foreign.with_active(|| {
        let mut read = ScriptListReadInfo::with_templates(templates);
        assert!(input.parse(&mut read));
        assert_eq!(read.lists.len(), 1);
        *read.lists.pop().unwrap()
    })
}

#[test]
fn explicit_condition_decoder_ignores_foreign_active_engine() {
    let _serial = crate::test_sync::lock();
    let _restore = RestoreGlobal(get_script_engine().write().unwrap().take());
    let mut engine = ScriptEngine::new().unwrap();
    let list = decode(&mut engine, true);
    let condition = list
        .first_script
        .unwrap()
        .condition
        .unwrap()
        .first_and
        .unwrap();
    assert_eq!(condition.condition_type, ConditionType::ConditionTrue);
    assert_eq!(condition.num_parms, 0);
    assert!(get_script_engine().read().unwrap().is_none());
}

#[test]
fn explicit_action_decoder_ignores_foreign_active_engine() {
    let _serial = crate::test_sync::lock();
    let _restore = RestoreGlobal(get_script_engine().write().unwrap().take());
    let mut engine = ScriptEngine::new().unwrap();
    let list = decode(&mut engine, false);
    let action = list.first_script.unwrap().action.unwrap();
    assert_eq!(action.action_type, ScriptActionType::Victory);
    assert_eq!(action.num_parms, 0);
    assert!(get_script_engine().read().unwrap().is_none());
}
