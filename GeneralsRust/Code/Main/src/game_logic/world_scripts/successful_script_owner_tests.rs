use super::*;
use gamelogic::scripting::core::ScriptActionType;

fn map(directory: &std::path::Path) -> std::path::PathBuf {
    let mut out = game_engine::common::system::DataChunkOutput::new();
    out.open_data_chunk("HeightMapData", 3);
    for value in [8, 8, 0, 64] {
        out.write_int(value);
    }
    for _ in 0..64 {
        out.write_byte(40);
    }
    out.close_data_chunk();
    out.open_data_chunk("PlayerScriptsList", 1);
    out.open_data_chunk("ScriptList", 1);
    out.open_data_chunk("Script", 2);
    for text in ["Retained authored script", "", "", ""] {
        out.write_ascii_string(text);
    }
    for value in [0, 0, 1, 1, 1, 0] {
        out.write_byte(value);
    }
    out.write_int(0);
    for _ in 0..3 {
        out.close_data_chunk();
    }
    let path = directory.join("ReceivingScriptOwner.map");
    std::fs::write(&path, out.into_ckmp_bytes()).unwrap();
    path
}

#[test]
fn successful_chunky_map_retains_receiving_debug_templates_and_authored_script_lists() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "successful_chunky_map_retains_receiving_debug_templates_and_authored_script_lists",
        || {
            let directory = tempfile::tempdir().unwrap();
            let path = map(directory.path());
            let mut owner = GameLogic::new();
            let mut peer = GameLogic::new();
            {
                let engine = owner.owned_script_engine.as_mut().unwrap();
                engine.set_time_frozen_debug(true);
                engine.set_global_difficulty(gamelogic::player::GameDifficulty::Hard);
                engine.parse_template_labels("ScriptAction\nInternalName = SET_FLAG\nHelpText = \"Receiving custom label\"\nEnd\n").unwrap();
            }
            let before = owner
                .owned_script_engine
                .as_ref()
                .unwrap()
                .get_action_template(ScriptActionType::SetFlag as usize)
                .unwrap();
            assert_eq!(before.base.help_text, "Receiving custom label");
            assert!(owner.load_map(path.to_str().unwrap()));
            assert!(owner.scripts_loaded);
            assert!(owner.script_debug_frozen(None).unwrap());
            assert_eq!(
                owner
                    .owned_script_engine
                    .as_ref()
                    .unwrap()
                    .get_global_difficulty(),
                gamelogic::player::GameDifficulty::Hard
            );
            let after = owner
                .owned_script_engine
                .as_ref()
                .unwrap()
                .get_action_template(ScriptActionType::SetFlag as usize)
                .unwrap();
            assert_eq!(after.base.help_text, before.base.help_text);
            assert_eq!(after.base.internal_name_key, before.base.internal_name_key);
            assert!(owner.loaded_script_lists.iter().any(|list| {
                list.get_script()
                    .is_some_and(|script| script.get_name() == "Retained authored script")
            }));
            assert!(peer.load_map(path.to_str().unwrap()));
            assert!(!peer.script_debug_frozen(None).unwrap());
            assert!(owner.script_debug_frozen(None).unwrap());
            owner.session_view.camera.camera_zoom_start = 1.0;
            owner.session_view.camera.camera_zoom_target = Some(2.0);
            owner.session_view.camera.camera_zoom_duration = 1.0;
            owner.session_view.camera.camera_zoom_frame = 0;
            let before_frame = owner.session_view.camera.camera_zoom_frame;
            owner
                .advance_receiving_camera_view(true, false, false, false, 33, None)
                .unwrap();
            assert_eq!(owner.session_view.camera.camera_zoom_frame, before_frame);
        },
    );
}

#[test]
fn map_script_definition_admission_rejects_an_outstanding_actual_interpreter_loan() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "map_script_definition_admission_rejects_an_outstanding_actual_interpreter_loan",
        || {
            let mut owner = GameLogic::new();
            let before_lists = owner.loaded_script_lists.len();
            owner
                .owned_script_engine
                .as_mut()
                .unwrap()
                .set_time_frozen_debug(true);
            owner
                .with_owned_script_engine(|engine, world| {
                    assert!(!world.admit_map_script_definitions_with_difficulty(
                        "unused.map",
                        None,
                        None
                    ));
                    assert!(world.owned_script_engine.is_none());
                    assert_eq!(world.loaded_script_lists.len(), before_lists);
                    assert!(engine.is_time_frozen_debug());
                })
                .unwrap();
            assert!(owner.script_debug_frozen(None).unwrap());
            // Direct definition admission is after reset, not another reset. Publish
            // the actual current policy exactly as the old install boundary did.
            let directory = tempfile::tempdir().unwrap();
            let path = map(directory.path());
            owner
                .owned_script_engine
                .as_ref()
                .unwrap()
                .set_objects_should_receive_difficulty_bonus(false);
            owner.objects_receive_difficulty_bonus = true;
            assert!(owner.admit_map_script_definitions_with_difficulty(
                path.to_str().unwrap(),
                None,
                None
            ));
            assert!(
                !owner
                    .owned_script_engine
                    .as_ref()
                    .unwrap()
                    .get_objects_should_receive_difficulty_bonus()
            );
            assert!(!owner.objects_receive_difficulty_bonus);
            assert!(owner.script_debug_frozen(None).unwrap());
        },
    );
}
