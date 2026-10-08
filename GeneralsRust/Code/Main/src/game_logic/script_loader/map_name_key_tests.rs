// C++ DataChunk.cpp:869 and Scripts.cpp:1663/2425: file name ids are not engine keys.
mod map_name_key_tests {
    use super::*;
    use game_engine::common::system::DataChunkOutput;
    use gamelogic::scripting::engine::{ScriptEngine, get_script_engine};

    struct RestoreEngine(Option<ScriptEngine>);
    impl RestoreEngine {
        fn install(engine: ScriptEngine) -> Self {
            let handle = get_script_engine();
            let mut slot = handle.write().unwrap();
            Self(std::mem::replace(&mut *slot, Some(engine)))
        }
    }
    impl Drop for RestoreEngine {
        fn drop(&mut self) {
            *get_script_engine().write().unwrap() = self.0.take();
        }
    }

    #[derive(Clone, Copy)]
    struct Record {
        condition: ConditionType,
        condition_key: u32,
        condition_version: u16,
        action: ScriptActionType,
        action_key: u32,
        action_version: u16,
    }
    impl Record {
        fn valid(engine: &ScriptEngine) -> Self {
            Self {
                condition: ConditionType::ConditionTrue,
                condition_key: engine
                    .get_condition_template(ConditionType::ConditionTrue as usize)
                    .unwrap()
                    .base
                    .internal_name_key,
                condition_version: 4,
                action: ScriptActionType::Victory,
                action_key: engine
                    .get_action_template(ScriptActionType::Victory as usize)
                    .unwrap()
                    .base
                    .internal_name_key,
                action_version: 2,
            }
        }
    }
    fn document(record: Record, sides: bool, padding: usize) -> ChunkyMap {
        let mut out = DataChunkOutput::new();
        // Extra labels move file-local name ids without changing engine keys.
        for i in 0..padding {
            out.open_data_chunk(&format!("Unused{i}"), 1);
            out.close_data_chunk();
        }
        if sides {
            out.open_data_chunk("SidesList", 2);
            out.write_int(0);
            out.write_int(0);
        }
        out.open_data_chunk("PlayerScriptsList", 1);
        out.open_data_chunk("ScriptList", 1);
        out.open_data_chunk("ScriptGroup", 2);
        out.write_ascii_string("key-group");
        out.write_byte(1);
        out.write_byte(0);
        out.open_data_chunk("Script", 2);
        for text in ["key-script", "comment", "condition", "actions"] {
            out.write_ascii_string(text);
        }
        for flag in [1, 0, 1, 1, 1, 0] {
            out.write_byte(flag);
        }
        out.write_int(7);
        out.open_data_chunk("OrCondition", 1);
        out.open_data_chunk("Condition", record.condition_version);
        out.write_int(record.condition as i32);
        if record.condition_version >= 4 {
            out.write_name_key(record.condition_key);
        }
        out.write_int(0);
        out.close_data_chunk();
        out.close_data_chunk();
        for label in ["ScriptAction", "ScriptActionFalse"] {
            out.open_data_chunk(label, record.action_version);
            out.write_int(record.action as i32);
            if record.action_version >= 2 {
                out.write_name_key(record.action_key);
            }
            out.write_int(0);
            out.close_data_chunk();
        }
        for _ in 0..4 {
            out.close_data_chunk();
        }
        if sides {
            out.close_data_chunk();
        }
        let bytes = out.into_ckmp_bytes();
        let (toc, body_offset) = parse_chunk_toc(&bytes).unwrap();
        ChunkyMap {
            source: PathBuf::from("cpp-name-key.map"),
            bytes,
            toc,
            body_offset,
        }
    }
    fn read(document: &ChunkyMap) -> (ConditionType, ScriptActionType, ScriptActionType) {
        // Actual public root, including its real engine/catalog adapter.
        let result = load_map_scripts_from_chunky(document).unwrap().unwrap();
        assert_eq!(result.total_scripts, 1);
        assert_eq!(result.script_lists.len(), 1);
        let group = result.script_lists[0].first_group.as_ref().unwrap();
        assert_eq!(group.group_name, "key-group");
        let script = group.first_script.as_ref().unwrap();
        assert_eq!(script.script_name, "key-script");
        assert_eq!(script.delay_evaluation_seconds, 7);
        let condition = script
            .condition
            .as_ref()
            .unwrap()
            .first_and
            .as_ref()
            .unwrap();
        let action = script.action.as_ref().unwrap();
        let otherwise = script.action_false.as_ref().unwrap();
        assert_eq!(
            (condition.num_parms, action.num_parms, otherwise.num_parms),
            (0, 0, 0)
        );
        (
            condition.condition_type,
            action.action_type,
            otherwise.action_type,
        )
    }
    #[test]
    fn direct_condition_matches_ordinal_by_decoded_name() {
        let engine = ScriptEngine::new().unwrap();
        let record = Record::valid(&engine);
        let _restore = RestoreEngine::install(engine);
        assert_eq!(
            read(&document(record, false, 0)).0,
            ConditionType::ConditionTrue
        );
    }
    #[test]
    fn direct_condition_rematches_stale_ordinal_by_decoded_name() {
        let engine = ScriptEngine::new().unwrap();
        let mut record = Record::valid(&engine);
        record.condition = ConditionType::ConditionFalse;
        let _restore = RestoreEngine::install(engine);
        assert_eq!(
            read(&document(record, false, 0)).0,
            ConditionType::ConditionTrue
        );
    }
    #[test]
    fn direct_action_matches_ordinal_by_decoded_name() {
        let engine = ScriptEngine::new().unwrap();
        let record = Record::valid(&engine);
        let _restore = RestoreEngine::install(engine);
        let (_, yes, no) = read(&document(record, false, 0));
        assert_eq!(yes, ScriptActionType::Victory);
        assert_eq!(no, ScriptActionType::Victory);
    }
    #[test]
    fn direct_action_rematches_stale_ordinal_in_both_branches() {
        let engine = ScriptEngine::new().unwrap();
        let mut record = Record::valid(&engine);
        record.action = ScriptActionType::NoOp;
        let _restore = RestoreEngine::install(engine);
        let (_, yes, no) = read(&document(record, false, 0));
        assert_eq!(yes, ScriptActionType::Victory);
        assert_eq!(no, ScriptActionType::Victory);
    }
    #[test]
    fn direct_and_sides_decode_identical_named_records_after_file_id_changes() {
        let engine = ScriptEngine::new().unwrap();
        let record = Record::valid(&engine);
        let _restore = RestoreEngine::install(engine);
        for padding in [0, 9] {
            assert_eq!(
                read(&document(record, false, padding)),
                read(&document(record, true, padding))
            );
        }
    }
    #[test]
    fn unknown_names_keep_false_noop_in_both_paths() {
        let engine = ScriptEngine::new().unwrap();
        let mut record = Record::valid(&engine);
        record.condition_key = NameKeyGenerator::name_to_key("UNKNOWN_MAP_CONDITION");
        record.action_key = NameKeyGenerator::name_to_key("UNKNOWN_MAP_ACTION");
        let _restore = RestoreEngine::install(engine);
        for sides in [false, true] {
            assert_eq!(
                read(&document(record, sides, 0)),
                (
                    ConditionType::ConditionFalse,
                    ScriptActionType::NoOp,
                    ScriptActionType::NoOp
                )
            );
        }
    }
    #[test]
    fn older_versions_do_not_consume_a_name_key_word() {
        let engine = ScriptEngine::new().unwrap();
        let mut record = Record::valid(&engine);
        record.condition_version = 3;
        record.action_version = 1;
        let _restore = RestoreEngine::install(engine);
        for sides in [false, true] {
            assert_eq!(
                read(&document(record, sides, 0)),
                (
                    ConditionType::ConditionTrue,
                    ScriptActionType::Victory,
                    ScriptActionType::Victory
                )
            );
        }
    }

    fn replace_packed_words(document: &mut ChunkyMap, condition_word: u32, action_word: u32) {
        for (label, version, word) in [
            ("Condition", 4u16, condition_word),
            ("ScriptAction", 2, action_word),
            ("ScriptActionFalse", 2, action_word),
        ] {
            let id = *document
                .toc
                .iter()
                .find(|(_, name)| name.as_str() == label)
                .unwrap()
                .0;
            let mut header = id.to_le_bytes().to_vec();
            header.extend_from_slice(&version.to_le_bytes());
            header.extend_from_slice(&12u32.to_le_bytes());
            let body = &document.bytes[document.body_offset..];
            let matches: Vec<_> = body
                .windows(header.len())
                .enumerate()
                .filter_map(|(i, bytes)| (bytes == header).then_some(i))
                .collect();
            assert_eq!(matches.len(), 1, "one fixture chunk for {label}");
            let key_offset = document.body_offset + matches[0] + header.len() + 4;
            document.bytes[key_offset..key_offset + 4].copy_from_slice(&word.to_le_bytes());
        }
    }
    fn rebuild_file_toc(document: &mut ChunkyMap) {
        let body = document.bytes[document.body_offset..].to_vec();
        let mut bytes = b"CkMp".to_vec();
        bytes.extend_from_slice(&(document.toc.len() as i32).to_le_bytes());
        let mut rows: Vec<_> = document.toc.iter().collect();
        rows.sort_by_key(|(id, _)| **id);
        for (id, name) in rows {
            bytes.push(name.len() as u8);
            bytes.extend_from_slice(name.as_bytes());
            bytes.extend_from_slice(&id.to_le_bytes());
        }
        document.body_offset = bytes.len();
        bytes.extend_from_slice(&body);
        document.bytes = bytes;
        let (toc, offset) = parse_chunk_toc(&document.bytes).unwrap();
        assert_eq!(toc, document.toc);
        assert_eq!(offset, document.body_offset);
    }
    #[test]
    fn absent_file_name_ids_keep_false_noop_in_both_paths() {
        let engine = ScriptEngine::new().unwrap();
        let record = Record::valid(&engine);
        let _restore = RestoreEngine::install(engine);
        for sides in [false, true] {
            let mut document = document(record, sides, 0);
            replace_packed_words(&mut document, 0x7fff_ee03, 0x7fff_ed03);
            assert_eq!(
                read(&document),
                (
                    ConditionType::ConditionFalse,
                    ScriptActionType::NoOp,
                    ScriptActionType::NoOp
                )
            );
        }
    }
    #[test]
    fn signed_packed_ids_do_not_alias_positive_toc_ids() {
        let engine = ScriptEngine::new().unwrap();
        let record = Record::valid(&engine);
        let _restore = RestoreEngine::install(engine);
        for sides in [false, true] {
            let mut document = document(record, sides, 0);
            // C++ Int >>8 gives a negative id; logical u32 >>8 would match these
            // positive aliases and incorrectly turn invalid records into live ones.
            document.toc.insert(
                0x0080_0010,
                NameKeyGenerator::key_to_name(record.condition_key).unwrap(),
            );
            document.toc.insert(
                0x0080_0020,
                NameKeyGenerator::key_to_name(record.action_key).unwrap(),
            );
            rebuild_file_toc(&mut document);
            replace_packed_words(&mut document, 0x8000_1003, 0x8000_2003);
            assert_eq!(
                read(&document),
                (
                    ConditionType::ConditionFalse,
                    ScriptActionType::NoOp,
                    ScriptActionType::NoOp
                )
            );
        }
    }
}
