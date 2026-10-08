// Actual map decoder routes using ordinary, unmodified ScriptEngine definitions.
mod authored_definition_tests {
    use super::*;
    use game_engine::common::system::DataChunkOutput;
    use gamelogic::scripting::engine::ScriptEngine;

    fn document(
        condition: ConditionType,
        condition_name: &str,
        parameters: &[Parameter],
        action: ScriptActionType,
        action_name: &str,
        sides: bool,
    ) -> ChunkyMap {
        let mut out = DataChunkOutput::new();
        if sides {
            out.open_data_chunk("SidesList", 2);
            out.write_int(0);
            out.write_int(0);
        }
        out.open_data_chunk("PlayerScriptsList", 1);
        out.open_data_chunk("ScriptList", 1);
        out.open_data_chunk("Script", 2);
        for text in ["authored-keys", "", "", ""] {
            out.write_ascii_string(text);
        }
        for flag in [1, 0, 1, 1, 1, 0] {
            out.write_byte(flag);
        }
        out.write_int(0);
        out.open_data_chunk("OrCondition", 1);
        out.open_data_chunk("Condition", 4);
        out.write_int(condition as i32);
        out.write_name_key(NameKeyGenerator::name_to_key(condition_name));
        out.write_int(parameters.len() as i32);
        for parameter in parameters {
            out.write_int(parameter.param_type as i32);
            out.write_int(parameter.int_value);
            out.write_real(parameter.real_value);
            out.write_ascii_string(&parameter.string_value);
        }
        out.close_data_chunk();
        out.close_data_chunk();
        for label in ["ScriptAction", "ScriptActionFalse"] {
            out.open_data_chunk(label, 2);
            out.write_int(action as i32);
            out.write_name_key(NameKeyGenerator::name_to_key(action_name));
            out.write_int(0);
            out.close_data_chunk();
        }
        for _ in 0..3 {
            out.close_data_chunk();
        }
        if sides {
            out.close_data_chunk();
        }
        let bytes = out.into_ckmp_bytes();
        let (toc, body_offset) = parse_chunk_toc(&bytes).unwrap();
        ChunkyMap {
            source: PathBuf::from("cpp-authored-keys.map"),
            bytes,
            toc,
            body_offset,
        }
    }

    fn read(engine: &ScriptEngine, document: &ChunkyMap) -> Box<Script> {
        // The ordinary Main initializer calls this bound production entry point.
        // No test/global slot lock or scoped engine publication is needed.
        let catalog = ScriptTemplateLookup::from_engine(engine);
        let mut result = load_map_scripts_from_chunky_with_templates(document, &catalog)
            .unwrap()
            .unwrap();
        assert_eq!(result.total_scripts, 1);
        assert_eq!(result.script_lists.len(), 1);
        result.script_lists.remove(0).first_script.take().unwrap()
    }

    fn check_condition(name: &str, expected: ConditionType, opponent: bool) {
        let mut engine = ScriptEngine::new().unwrap();
        // Ordinary reset keeps the authored definitions; it must not invent names.
        engine.reset();
        let mut parameters = vec![
            Parameter::with_string(ParameterType::Side, "PlyrAmerica".into()),
            Parameter::with_int(ParameterType::Int, 9),
        ];
        if opponent {
            parameters.push(Parameter::with_string(
                ParameterType::Side,
                "PlyrChina".into(),
            ));
        }
        for sides in [false, true] {
            let script = read(
                &engine,
                &document(
                    ConditionType::ConditionFalse,
                    name,
                    &parameters,
                    ScriptActionType::NoOp,
                    "VICTORY",
                    sides,
                ),
            );
            let condition = script.condition.unwrap().first_and.unwrap();
            assert_eq!(condition.condition_type, expected, "SidesList={sides}");
            assert_eq!(condition.num_parms, parameters.len());
            for (actual, expected) in condition.parameters.iter().flatten().zip(&parameters) {
                assert_eq!(actual.param_type, expected.param_type);
                assert_eq!(actual.int_value, expected.int_value);
                assert_eq!(actual.string_value, expected.string_value);
            }
            assert_eq!(
                script.action.unwrap().action_type,
                ScriptActionType::Victory
            );
            assert_eq!(
                script.action_false.unwrap().action_type,
                ScriptActionType::Victory
            );
        }
    }

    #[test]
    fn both_routes_resolve_cpp_n_or_fewer_buildings() {
        check_condition(
            "PLAYER_HAS_N_OR_FEWER_BUILDINGS",
            ConditionType::PlayerHasNOrFewerBuildings,
            false,
        );
    }
    #[test]
    fn both_routes_resolve_cpp_n_or_fewer_faction_buildings() {
        check_condition(
            "PLAYER_HAS_N_OR_FEWER_FACTION_BUILDINGS",
            ConditionType::PlayerHasNOrFewerFactionBuildings,
            false,
        );
    }
    #[test]
    fn both_routes_resolve_cpp_destroyed_n_buildings() {
        check_condition(
            "PLAYER_DESTROYED_N_BUILDINGS_PLAYER",
            ConditionType::PlayerDestroyedNBuildingsPlayer,
            true,
        );
    }
    #[test]
    fn both_routes_reject_a_name_cpp_never_authored() {
        let engine = ScriptEngine::new().unwrap();
        for sides in [false, true] {
            let script = read(
                &engine,
                &document(
                    ConditionType::ConditionTrue,
                    "CONDITION_TRUE",
                    &[],
                    ScriptActionType::NamedReceiveUpgrade,
                    "NAMED_RECEIVE_UPGRADE",
                    sides,
                ),
            );
            assert_eq!(script.action.unwrap().action_type, ScriptActionType::NoOp);
            assert_eq!(
                script.action_false.unwrap().action_type,
                ScriptActionType::NoOp
            );
        }
    }
    #[test]
    fn empty_names_preserve_stored_slots_then_rematch_first_empty_slot() {
        let engine = ScriptEngine::new().unwrap();
        for sides in [false, true] {
            for (stored, expected) in [
                (
                    ConditionType::ObsoleteScript2,
                    ConditionType::ObsoleteScript2,
                ),
                (
                    ConditionType::ConditionFalse,
                    ConditionType::ObsoleteScript1,
                ),
            ] {
                let script = read(
                    &engine,
                    &document(
                        stored,
                        "",
                        &[],
                        ScriptActionType::NamedReceiveUpgrade,
                        "",
                        sides,
                    ),
                );
                assert_eq!(
                    script.condition.unwrap().first_and.unwrap().condition_type,
                    expected
                );
                assert_eq!(
                    script.action.unwrap().action_type,
                    ScriptActionType::NamedReceiveUpgrade
                );
                assert_eq!(
                    script.action_false.unwrap().action_type,
                    ScriptActionType::NamedReceiveUpgrade
                );
            }
        }
    }
}
