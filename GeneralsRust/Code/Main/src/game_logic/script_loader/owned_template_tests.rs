// Exercise the actual SidesList fallback through the public map decoder.
fn owned_template_sides_document(engine: &gamelogic::scripting::engine::ScriptEngine) -> ChunkyMap {
    use game_engine::common::system::DataChunkOutput;
    let mut output = DataChunkOutput::new();
    output.open_data_chunk("SidesList", 2);
    output.write_int(0); // sides
    output.write_int(0); // team dictionaries
    output.open_data_chunk("PlayerScriptsList", 1);
    output.open_data_chunk("ScriptList", 1);
    output.open_data_chunk("ScriptGroup", 2);
    output.write_ascii_string("group");
    output.write_byte(1);
    output.write_byte(0);
    output.open_data_chunk("Script", 2);
    for name in ["owned", "", "", ""] {
        output.write_ascii_string(name);
    }
    for value in [1, 0, 1, 1, 1, 0] {
        output.write_byte(value);
    }
    output.write_int(0);
    output.open_data_chunk("OrCondition", 1);
    output.open_data_chunk("Condition", 4);
    output.write_int(ConditionType::ConditionFalse as i32);
    output.write_name_key(
        engine
            .get_condition_template(ConditionType::ConditionTrue as usize)
            .unwrap()
            .base
            .internal_name_key,
    );
    output.write_int(0);
    output.close_data_chunk();
    output.close_data_chunk();
    for label in ["ScriptAction", "ScriptActionFalse"] {
        output.open_data_chunk(label, 2);
        output.write_int(ScriptActionType::NoOp as i32);
        output.write_name_key(
            engine
                .get_action_template(ScriptActionType::Victory as usize)
                .unwrap()
                .base
                .internal_name_key,
        );
        output.write_int(0);
        output.close_data_chunk();
    }
    for _ in 0..5 {
        output.close_data_chunk();
    }
    let bytes = output.into_ckmp_bytes();
    let (toc, body_offset) = parse_chunk_toc(&bytes).unwrap();
    ChunkyMap {
        source: PathBuf::from("owned-template.map"),
        bytes,
        toc,
        body_offset,
    }
}

#[test]
fn sides_fallback_uses_supplied_catalog_for_nested_conditions_and_both_actions() {
    let engine = gamelogic::scripting::engine::ScriptEngine::new().unwrap();
    let document = owned_template_sides_document(&engine);
    for (catalog, condition_kind, action_kind) in [
        (
            ScriptTemplateLookup::default(),
            ConditionType::ConditionFalse,
            ScriptActionType::NoOp,
        ),
        (
            ScriptTemplateLookup::from_engine(&engine),
            ConditionType::ConditionTrue,
            ScriptActionType::Victory,
        ),
        (
            ScriptTemplateLookup::default(),
            ConditionType::ConditionFalse,
            ScriptActionType::NoOp,
        ),
    ] {
        let result = load_map_scripts_from_chunky_with_templates(&document, &catalog)
            .unwrap()
            .unwrap();
        assert_eq!(result.total_scripts, 1);
        let group = result.script_lists[0].first_group.as_ref().unwrap();
        assert_eq!(group.group_name, "group");
        let script = group.first_script.as_ref().unwrap();
        assert_eq!(
            script
                .condition
                .as_ref()
                .unwrap()
                .first_and
                .as_ref()
                .unwrap()
                .condition_type,
            condition_kind
        );
        assert_eq!(script.action.as_ref().unwrap().action_type, action_kind);
        assert_eq!(
            script.action_false.as_ref().unwrap().action_type,
            action_kind
        );
    }
}
