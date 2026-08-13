// GameClient unit tests.
// Split from `core/game_client.rs` dump. Included by `game_client_impl/mod.rs`
// so this stays one logical `game_client` module (public API identical).

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drawable::Drawable;
    use crate::message_stream::game_message::GameMessageType;
    use crate::network::is_network_command_message;
    use crate::system::{Coord3D, GameMessageResult};
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    use game_engine::common::thing::{
        get_thing_factory, init_thing_factory, ThingFactory as CommonThingFactory,
    };
    use game_engine::common::{
        global_data as runtime_global_data,
        ini::{get_global_data, ini_game_data::init_global_data},
        recorder::Recorder,
    };
    use gamelogic::common::types::{ObjectShroudStatus, ObjectStatusMaskType};
    use gamelogic::thing_template::DefaultThingTemplate as LogicDefaultThingTemplate;
    use std::io::Cursor;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    struct StubCommandTranslator;

    impl CommandTranslator for StubCommandTranslator {
        fn evaluate_context_command(
            &self,
            _drawable: &dyn Drawable,
            _position: &Coord3D,
            _cmd_type: CommandEvaluateType,
        ) -> GameMessageResult<GameMessageType> {
            Ok(GameMessageType::ValidGUICommandHint)
        }
    }

    fn serialize_client(client: &mut GameClient) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let cursor = Cursor::new(&mut bytes);
            let mut xfer = XferSave::new(cursor, 3);
            client
                .xfer(&mut xfer)
                .expect("game client serialization should succeed");
        }
        bytes
    }

    fn deserialize_client(bytes: &[u8]) -> GameClient {
        let mut loaded = GameClient::new().expect("game client creation should succeed");
        let cursor = Cursor::new(bytes.to_vec());
        let mut xfer = XferLoad::new(cursor, 3);
        loaded
            .xfer(&mut xfer)
            .expect("game client deserialization should succeed");
        loaded
    }

    #[test]
    fn position_shroud_status_uses_shroud_grid_like_cpp_partition_manager() {
        let shroud_manager = gamelogic::system::shroud_manager::get_shroud_manager();
        {
            let mut shroud = shroud_manager.lock().expect("shroud manager lock");
            *shroud = gamelogic::system::shroud_manager::ShroudManager::new();
            shroud.init_shroud_grid(500.0, 500.0);
        }

        let client = GameClient::new().expect("GameClient::new should succeed");
        let pos = Coord3D::new(100.0, 100.0, 0.0);

        assert_eq!(
            client.get_shroud_status_for_player(-1, &pos),
            ShroudStatus::Shrouded
        );
        assert_eq!(
            client.get_shroud_status_for_player(0, &pos),
            ShroudStatus::Shrouded
        );

        {
            let mut shroud = shroud_manager.lock().expect("shroud manager lock");
            let world = gamelogic::common::Coord3D {
                x: pos.x,
                y: pos.y,
                z: pos.z,
            };
            shroud.do_shroud_reveal(&world, 75.0, 1);
        }
        assert_eq!(
            client.get_shroud_status_for_player(0, &pos),
            ShroudStatus::Clear
        );

        {
            let mut shroud = shroud_manager.lock().expect("shroud manager lock");
            let world = gamelogic::common::Coord3D {
                x: pos.x,
                y: pos.y,
                z: pos.z,
            };
            shroud.undo_shroud_reveal(&world, 75.0, 1);
        }
        assert_eq!(
            client.get_shroud_status_for_player(0, &pos),
            ShroudStatus::Fogged
        );

        *shroud_manager.lock().expect("shroud manager lock") =
            gamelogic::system::shroud_manager::ShroudManager::new();
    }

    fn read_utf16_z_end(bytes: &[u8], mut offset: usize) -> usize {
        loop {
            assert!(
                offset + 1 < bytes.len(),
                "Malformed replay header while reading UTF-16 string"
            );
            let code_unit = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
            offset += 2;
            if code_unit == 0 {
                return offset;
            }
        }
    }

    fn replay_version_offsets(bytes: &[u8]) -> (usize, usize, usize, usize, usize, usize) {
        // Magic + fixed replay stats block
        let mut offset = 6 + 8 + 8 + 4 + 1 + 1 + 8;
        // Replay name
        offset = read_utf16_z_end(bytes, offset);
        // Timestamp
        offset += 8;
        let version_string_start = offset;
        let version_string_end = read_utf16_z_end(bytes, offset);
        let version_time_start = version_string_end;
        let version_time_end = read_utf16_z_end(bytes, version_time_start);
        let version_number_offset = version_time_end;
        let exe_crc_offset = version_number_offset + 4;
        let ini_crc_offset = version_number_offset + 8;
        (
            version_string_start,
            version_string_end,
            version_time_start,
            version_number_offset,
            exe_crc_offset,
            ini_crc_offset,
        )
    }

    fn mutate_utf16_first_code_unit(bytes: &mut [u8], start: usize, end: usize, field_name: &str) {
        assert!(
            end >= start + 4,
            "Replay {field_name} field is unexpectedly empty"
        );
        let current = u16::from_le_bytes([bytes[start], bytes[start + 1]]);
        let next = current.wrapping_add(1).max(1);
        bytes[start..start + 2].copy_from_slice(&next.to_le_bytes());
    }

    fn write_variant(
        base_path: &Path,
        replays_dir: &Path,
        variant_name: &str,
        mutate: impl FnOnce(&mut Vec<u8>),
    ) -> PathBuf {
        let mut bytes = std::fs::read(base_path).expect("base replay should be readable");
        mutate(&mut bytes);
        let variant_path = replays_dir.join(variant_name);
        std::fs::write(&variant_path, bytes).expect("variant replay should be writable");
        variant_path
    }

    fn ensure_templates_registered(names: &[&str]) {
        let _ = init_thing_factory();
        let mut guard = get_thing_factory().expect("thing factory lock should be available");
        let factory = guard
            .as_mut()
            .expect("thing factory should be initialized for save/load tests");
        for &name in names {
            if factory.find_template(name, false).is_none() {
                factory.new_template(name);
            }
        }
    }

    fn insert_basic_drawable_for_test(
        client: &mut GameClient,
        id: u32,
        template_name: &str,
        position: Vector3,
    ) {
        let drawable_id = DrawableId(id);
        let mut drawable = BasicDrawable::new(drawable_id);
        drawable.set_id(drawable_id);
        drawable.set_template_name(Some(template_name.to_string()));
        drawable.set_position(position);
        client.drawable_map.insert(drawable_id, Box::new(drawable));
    }

    #[test]
    fn test_create_drawable_from_template_attaches_w3d_snapshot_modules() {
        use game_engine::common::rts::AsciiString;
        use game_engine::common::thing::module::{ModuleData, ModuleInterfaceType};

        let mut template = ThingTemplate::new();
        template.set_template_name(AsciiString::from("SnapshotTemplate"));
        template.add_draw_module_info(
            AsciiString::from("W3DTreeDraw"),
            AsciiString::from("TreeDrawTag"),
            Arc::new(W3DTreeDrawModuleData::new()) as Arc<dyn ModuleData>,
            ModuleInterfaceType::DRAW,
        );

        let mut client = GameClient::new().expect("GameClient::new should succeed");
        let drawable_id = client
            .create_drawable_from_template(&template)
            .expect("template drawable should be created");
        let drawable = client
            .find_drawable_by_id(drawable_id)
            .expect("created drawable should be registered");
        let basic = drawable
            .as_any()
            .downcast_ref::<BasicDrawable>()
            .expect("template drawable should be BasicDrawable");

        assert_eq!(basic.get_draw_modules().len(), 1);
        assert_eq!(
            basic.get_draw_modules()[0].snapshot_module_identifier(),
            Some("TreeDrawTag")
        );
        assert_eq!(
            basic.get_draw_modules()[0].drawable_module_type_index(),
            LogicDrawModuleSnapshotAdapter::DRAW_MODULE_TYPE_INDEX
        );
    }

    #[test]
    fn test_create_drawable_from_template_attaches_client_update_snapshot_modules() {
        use game_engine::common::rts::AsciiString;
        use game_engine::common::thing::module::{BaseModuleData, ModuleData, ModuleInterfaceType};

        let mut template = ThingTemplate::new();
        template.set_template_name(AsciiString::from("ClientUpdateSnapshotTemplate"));
        template.add_client_update_module_info(
            AsciiString::from("SwayClientUpdate"),
            AsciiString::from("SwayTag"),
            Arc::new(BaseModuleData::new()) as Arc<dyn ModuleData>,
            ModuleInterfaceType::CLIENT_UPDATE,
        );

        let mut client = GameClient::new().expect("GameClient::new should succeed");
        let drawable_id = client
            .create_drawable_from_template(&template)
            .expect("template drawable should be created");
        let drawable = client
            .find_drawable_by_id(drawable_id)
            .expect("created drawable should be registered");
        let basic = drawable
            .as_any()
            .downcast_ref::<BasicDrawable>()
            .expect("template drawable should be BasicDrawable");

        assert_eq!(basic.get_draw_modules().len(), 1);
        assert_eq!(
            basic.get_draw_modules()[0].snapshot_module_identifier(),
            Some("SwayTag")
        );
        assert_eq!(
            basic.get_draw_modules()[0].drawable_module_type_index(),
            LogicDrawModuleSnapshotAdapter::CLIENT_UPDATE_MODULE_TYPE_INDEX
        );
    }

    #[test]
    fn test_context_command_uses_stored_translator() {
        let mut client = GameClient::new().expect("GameClient::new should succeed");
        client.command_translator = Some(Arc::new(StubCommandTranslator));

        insert_basic_drawable_for_test(
            &mut client,
            42,
            "ContextProbe",
            Vector3::new(3.0, 4.0, 0.0),
        );
        let drawable = client
            .drawable_map
            .get(&DrawableId(42))
            .expect("drawable should exist");

        let result = client
            .evaluate_context_command(
                drawable.as_ref(),
                &Coord3D::new(3.0, 4.0, 0.0),
                CommandEvaluateType::Context,
            )
            .expect("context evaluation should succeed");

        assert_eq!(result, GameMessageType::ValidGUICommandHint);
    }

    #[test]
    fn test_no_draw_skip_condition_matches_cpp_guard() {
        let global_data = game_engine::common::ini::ini_game_data::ensure_global_data();
        let saved_no_draw = global_data.read().no_draw;
        let saved_logic_frame = gamelogic::system::game_logic::get_game_logic()
            .lock()
            .map(|logic| logic.get_current_frame())
            .unwrap_or(0);

        {
            global_data.write().no_draw = 10;
            if let Ok(mut logic) = gamelogic::system::game_logic::get_game_logic().lock() {
                logic.set_current_frame(1);
            }
            let client = GameClient::new().expect("GameClient::new should succeed");
            assert!(client.should_skip_visual_updates_for_no_draw());
        }

        {
            global_data.write().no_draw = 1;
            if let Ok(mut logic) = gamelogic::system::game_logic::get_game_logic().lock() {
                logic.set_current_frame(1);
            }
            let client = GameClient::new().expect("GameClient::new should succeed");
            assert!(!client.should_skip_visual_updates_for_no_draw());
        }

        {
            global_data.write().no_draw = u32::MAX;
            if let Ok(mut logic) = gamelogic::system::game_logic::get_game_logic().lock() {
                logic.set_current_frame(0);
            }
            let client = GameClient::new().expect("GameClient::new should succeed");
            assert!(!client.should_skip_visual_updates_for_no_draw());
        }

        global_data.write().no_draw = saved_no_draw;
        if let Ok(mut logic) = gamelogic::system::game_logic::get_game_logic().lock() {
            logic.set_current_frame(saved_logic_frame);
        }
    }

    #[test]
    fn test_freeze_visual_time_same_frame_guard_uses_logic_frame() {
        let saved_logic_frame = gamelogic::system::game_logic::get_game_logic()
            .lock()
            .map(|logic| logic.get_current_frame())
            .unwrap_or(0);
        let saved_paused = TheGameLogic::is_game_paused();
        let (saved_script_frozen, saved_debug_frozen) =
            if let Ok(engine_guard) = gamelogic::get_script_engine().read() {
                if let Some(engine) = engine_guard.as_ref() {
                    (
                        engine.is_time_frozen_script(),
                        engine.is_time_frozen_debug(),
                    )
                } else {
                    (false, false)
                }
            } else {
                (false, false)
            };

        TheGameLogic::set_game_paused(false, false);
        if let Ok(mut engine_guard) = gamelogic::get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                engine.do_unfreeze_time();
                engine.set_time_frozen_debug(false);
            }
        }

        if let Ok(mut logic) = gamelogic::system::game_logic::get_game_logic().lock() {
            logic.set_current_frame(100);
        }

        let mut client = GameClient::new().expect("GameClient::new should succeed");
        client.frame = 1;
        assert!(
            !client.should_freeze_visual_time(),
            "first pass at a logic frame should not freeze when no freeze flags are set"
        );
        assert!(
            client.should_freeze_visual_time(),
            "second pass in the same logic frame should freeze (C++ lastFrame == m_frame guard)"
        );

        // Changing client frame alone should not bypass same-frame freeze; logic frame drives this.
        client.frame = client.frame.wrapping_add(1);
        assert!(
            client.should_freeze_visual_time(),
            "same logic frame must remain frozen even if client update counter changes"
        );

        if let Ok(mut logic) = gamelogic::system::game_logic::get_game_logic().lock() {
            logic.set_current_frame(101);
        }
        assert!(
            !client.should_freeze_visual_time(),
            "advancing simulation frame should clear same-frame freeze guard"
        );

        TheGameLogic::set_game_paused(saved_paused, false);
        if let Ok(mut engine_guard) = gamelogic::get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                if saved_script_frozen {
                    engine.do_freeze_time();
                } else {
                    engine.do_unfreeze_time();
                }
                engine.set_time_frozen_debug(saved_debug_frozen);
            }
        }
        if let Ok(mut logic) = gamelogic::system::game_logic::get_game_logic().lock() {
            logic.set_current_frame(saved_logic_frame);
        }
    }

    #[test]
    fn test_drawable_id_creation() {
        let id = DrawableId(42);
        assert!(id.is_valid());
        assert_eq!(id.0, 42);

        let invalid = DrawableId::INVALID;
        assert!(!invalid.is_valid());
    }

    #[test]
    fn test_game_client_creation() {
        let client = GameClient::new();
        assert!(client.is_ok());

        let client = client.unwrap();
        assert_eq!(client.get_frame(), 0);
        assert!(!client.initialized);
    }

    #[test]
    fn test_startup_movie_action_prefers_logo_before_after_intro() {
        assert_eq!(
            startup_movie_action(true, true, true, true, false),
            Some(StartupMovieAction::PlayLogo("EALogoMovie"))
        );
    }

    #[test]
    fn test_startup_movie_action_uses_low_res_variants() {
        assert_eq!(
            startup_movie_action(true, false, false, false, true),
            Some(StartupMovieAction::PlayLogo("EALogoMovie640"))
        );
        assert_eq!(
            startup_movie_action(false, true, true, true, true),
            Some(StartupMovieAction::PlaySizzle("Sizzle640"))
        );
    }

    #[test]
    fn test_startup_movie_action_only_plays_sizzle_when_pending() {
        assert_eq!(
            startup_movie_action(false, true, true, true, false),
            Some(StartupMovieAction::PlaySizzle("Sizzle"))
        );
        assert_eq!(
            startup_movie_action(false, true, true, false, false),
            Some(StartupMovieAction::FinalizeStartup)
        );
    }

    #[test]
    fn test_startup_movie_action_ignores_sizzle_when_after_intro_is_clear() {
        assert_eq!(startup_movie_action(false, false, true, true, false), None);
    }

    #[test]
    fn update_startup_movies_without_display_finishes_movie_state() {
        init_global_data();
        let global_data = get_global_data().expect("global data should be initialized");
        {
            let mut global = global_data.write();
            global.initial_file = "Maps\\TestMap\\TestMap.map".to_string();
            global.play_intro = true;
            global.after_intro = false;
            global.play_sizzle = true;
            global.break_the_movie = false;
            global.allow_exit_out_of_movies = false;
        }

        let mut client = GameClient::new().expect("GameClient::new should succeed");
        client.startup_sizzle_pending = true;
        assert!(client.subsystem_manager.display.is_none());

        client
            .update_startup_movies()
            .expect("movie fallback should finish without display");

        let global = global_data.read();
        assert!(!global.play_intro);
        assert!(!global.after_intro);
        assert!(global.break_the_movie);
        assert!(global.allow_exit_out_of_movies);
        assert!(!client.startup_sizzle_pending);
        assert!(!client.startup_movies_active());
    }

    #[test]
    fn ensure_shell_visible_does_not_requeue_active_shell_map() {
        init_global_data();
        let global_data = get_global_data().expect("global data should be initialized");
        {
            let mut global = global_data.write();
            global.initial_file.clear();
            global.shell_map_on = false;
            global.shell_map_name = "Maps\\ShellMap\\ShellMap.map".to_string();
            global.pending_file.clear();
        }
        if let Ok(mut logic) = gamelogic::system::game_logic::get_game_logic().lock() {
            logic.set_game_mode(gamelogic::system::game_logic::GAME_NONE);
        }

        {
            let mut shell = get_shell();
            let _ = shell.init();
            let _ = shell.reset();
            shell.show_shell_map(false);
        }
        {
            let mut stream = THE_MESSAGE_STREAM
                .write()
                .unwrap_or_else(|e| e.into_inner());
            stream.clear_messages();
        }
        {
            let mut global = global_data.write();
            global.shell_map_on = true;
        }

        let client = GameClient::new().expect("GameClient::new should succeed");
        client
            .ensure_shell_visible()
            .expect("first shell activation should succeed");
        client
            .ensure_shell_visible()
            .expect("second shell activation should succeed");

        let stream = THE_MESSAGE_STREAM.read().unwrap_or_else(|e| e.into_inner());
        let new_game_count = stream
            .get_messages()
            .iter()
            .filter(|msg| matches!(msg.get_type(), GameMessageType::NewGame))
            .count();
        assert_eq!(new_game_count, 1);
        assert_eq!(
            global_data.read().pending_file,
            "Maps\\ShellMap\\ShellMap.map"
        );
        assert!(get_shell().is_shell_map_on());
    }

    #[test]
    fn test_drawable_id_allocation() {
        let mut client = GameClient::new().unwrap();

        let id1 = client.alloc_drawable_id();
        let id2 = client.alloc_drawable_id();

        assert_ne!(id1, id2);
        assert_eq!(id1.0 + 1, id2.0);
    }

    #[test]
    fn test_apply_lod_texture_reduction_clamps_and_updates_renderer_state() {
        init_global_data();
        let global_data = get_global_data().expect("global data should be available for LOD test");
        global_data.write().texture_reduction_factor = 0;
        if let Ok(mut runtime_global) = runtime_global_data::write_safe() {
            runtime_global.texture_reduction_factor = 0;
        }
        ww3d_renderer_3d::rendering::texture_quality::set_texture_reduction(
            0,
            WW3D_TEXTURE_REDUCTION_MIN_DIMENSION,
        );

        assert_eq!(apply_lod_texture_reduction(9), Some(4));
        assert_eq!(global_data.read().texture_reduction_factor, 4);
        assert_eq!(runtime_global_data::read().texture_reduction_factor, 4);
        assert_eq!(
            ww3d_renderer_3d::rendering::texture_quality::texture_reduction(),
            4
        );
    }

    #[test]
    fn test_adjust_lod_texture_reduction_respects_cpp_delta_clamp() {
        init_global_data();
        let global_data = get_global_data().expect("global data should be available for LOD test");
        global_data.write().texture_reduction_factor = 0;
        ww3d_renderer_3d::rendering::texture_quality::set_texture_reduction(
            0,
            WW3D_TEXTURE_REDUCTION_MIN_DIMENSION,
        );

        assert_eq!(adjust_lod_texture_reduction(-1), Some(0));
        assert_eq!(adjust_lod_texture_reduction(5), Some(4));
        assert_eq!(adjust_lod_texture_reduction(-20), Some(0));
        assert_eq!(global_data.read().texture_reduction_factor, 0);
    }

    #[test]
    fn test_register_drawable_replaces_object_lookup_owner() {
        let mut client = GameClient::new().unwrap();

        let mut first = BasicDrawable::new(DrawableId::INVALID);
        first.set_object_id(Some(77));
        let first_id = client.register_drawable(Box::new(first)).unwrap();

        let mut second = BasicDrawable::new(DrawableId::INVALID);
        second.set_object_id(Some(77));
        let second_id = client.register_drawable(Box::new(second)).unwrap();

        assert_eq!(client.get_drawable_for_object(77), Some(second_id));
        assert_eq!(
            client
                .find_drawable_by_id(first_id)
                .and_then(|d| d.get_object_id()),
            None
        );
    }

    #[test]
    fn test_bind_drawable_to_object_rebinds_and_destroy_keeps_new_owner() {
        let mut client = GameClient::new().unwrap();

        let first_id = client
            .register_drawable(Box::new(BasicDrawable::new(DrawableId::INVALID)))
            .unwrap();
        let second_id = client
            .register_drawable(Box::new(BasicDrawable::new(DrawableId::INVALID)))
            .unwrap();

        client.bind_drawable_to_object(first_id, 99).unwrap();
        client.bind_drawable_to_object(second_id, 99).unwrap();

        assert_eq!(client.get_drawable_for_object(99), Some(second_id));
        assert_eq!(
            client
                .find_drawable_by_id(first_id)
                .and_then(|d| d.get_object_id()),
            None
        );

        client.destroy_drawable(first_id).unwrap();
        assert_eq!(client.get_drawable_for_object(99), Some(second_id));
    }

    fn presentation_drawable_sync_for_test(
        object_id: u32,
        host_epoch: u64,
        visual_template_name: &str,
        resident: bool,
        destroyed: bool,
        position: [f32; 3],
        orientation: f32,
    ) -> PresentationDrawableSync {
        PresentationDrawableSync {
            object_id,
            host_epoch,
            resident,
            visual_template_name: visual_template_name.to_string(),
            template_name: "UnderlyingTemplate".to_string(),
            position,
            orientation,
            destroyed,
            model_condition_bits: 0,
            body_damage_state: 0,
            kind_names: Vec::new(),
            team_color: [1.0, 1.0, 1.0, 1.0],
            effectively_stealthed: false,
            scene_hidden_by_stealth: false,
            health_current: 0.0,
            health_max: 0.0,
            selected: false,
            veterancy_level: 0,
            under_construction: false,
            construction_percent: 0.0,
            sold: false,
            ammo_pip_total: 0,
            ammo_pip_full: 0,
            occupant_count: 0,
            max_garrison: 0,
            disabled: false,
            is_carbomb: false,
            weapon_bonus_enthusiastic: false,
            show_healing: false,
            healing_icon_type: 0,
            garrisoned_ids: Vec::new(),
            emoticon_name: String::new(),
            emoticon_frames_left: 0,
            formation_id: 0,
            caption: String::new(),
        }
    }

    #[test]
    fn direct_visual_binding_uses_residency_and_replaces_on_visual_identity_change() {
        let mut client = GameClient::new().unwrap();
        let object_id = 101;
        let host_epoch = 7;
        let first = presentation_drawable_sync_for_test(
            object_id,
            host_epoch,
            "VisualA",
            true,
            true,
            [2.0, 3.0, 4.0],
            0.2,
        );

        // `destroyed` is gameplay state, not direct visual residency.  An
        // active slow-death/rubble visual remains bound while resident.
        assert_eq!(
            client.sync_presentation_drawables([first.clone()]),
            (1, 0, 0)
        );
        let initial = client
            .presentation_direct_drawable_state(host_epoch, object_id)
            .expect("resident direct visual should receive a runtime key");
        let initial_id = initial.binding_key.drawable_id;
        assert_eq!(
            client.apply_frozen_direct_shroud_statuses(
                40,
                [FrozenDirectShroudStatus {
                    binding_key: initial.binding_key,
                    raw_status: ObjectShroudStatus::Fogged,
                    effectively_dead: true,
                }],
            ),
            1
        );
        assert!(
            client
                .presentation_direct_drawable_state(host_epoch, object_id)
                .expect("same binding remains queryable")
                .fully_obscured
        );

        // Exact same resident visual keeps its full binding key and therefore
        // retains C++ volatile direct shroud history.
        assert_eq!(
            client.sync_presentation_drawables([first.clone()]),
            (0, 1, 0)
        );
        assert_eq!(
            client
                .presentation_direct_drawable_state(host_epoch, object_id)
                .expect("same visual stays bound")
                .binding_key,
            initial.binding_key
        );

        // C++ `friend_bindToObject` preserves volatile Drawable shroud state
        // for an ordinary rebind to the same Object.  The runtime key must
        // remain valid too, otherwise the next presentation sync would
        // recreate the Drawable and discard the direct visibility history.
        client
            .bind_drawable_to_object(initial.binding_key.drawable_id, object_id)
            .expect("same-owner rebind");
        let rebound = client
            .presentation_direct_drawable_state(host_epoch, object_id)
            .expect("same-owner rebind keeps the direct binding");
        assert_eq!(rebound.binding_key, initial.binding_key);
        assert!(
            rebound.fully_obscured,
            "same-owner rebind must retain volatile shroud state"
        );
        assert_eq!(
            client.sync_presentation_drawables([first.clone()]),
            (0, 1, 0)
        );

        let replacement = presentation_drawable_sync_for_test(
            object_id,
            host_epoch,
            "VisualB",
            true,
            false,
            [5.0, 6.0, 7.0],
            0.4,
        );
        assert_eq!(
            client.sync_presentation_drawables([replacement]),
            (1, 0, 0),
            "a visual disguise/template replacement recreates the Drawable"
        );
        let replaced = client
            .presentation_direct_drawable_state(host_epoch, object_id)
            .expect("replacement visual should receive a runtime key");
        assert_ne!(replaced.binding_key, initial.binding_key);
        assert_ne!(replaced.binding_key.drawable_id, initial_id);
        assert!(replaced.binding_key.binding_generation > initial.binding_key.binding_generation);
        assert!(
            !replaced.fully_obscured,
            "replacement resets volatile shroud state"
        );

        // A frozen status/pose for the replaced visual cannot mutate the new
        // binding, even though host epoch and ObjectID still match.
        assert_eq!(
            client.apply_frozen_direct_shroud_statuses(
                41,
                [FrozenDirectShroudStatus {
                    binding_key: initial.binding_key,
                    raw_status: ObjectShroudStatus::Clear,
                    effectively_dead: false,
                }],
            ),
            0
        );
        assert_eq!(
            client.apply_frozen_direct_presentation_poses([FrozenDirectPresentationPose {
                binding_key: initial.binding_key,
                position: [99.0, 99.0, 99.0],
                orientation: 1.0,
            }]),
            0
        );
    }

    #[test]
    fn direct_scene_candidate_refreshes_clear_history_only_for_the_current_binding() {
        let mut client = GameClient::new().unwrap();
        let object_id = 212;
        let host_epoch = 8;
        let entry = presentation_drawable_sync_for_test(
            object_id,
            host_epoch,
            "VisualA",
            true,
            false,
            [0.0, 0.0, 0.0],
            0.0,
        );
        assert_eq!(client.sync_presentation_drawables([entry]), (1, 0, 0));
        let initial = client
            .presentation_direct_drawable_state(host_epoch, object_id)
            .expect("resident direct visual gets a binding key");

        let decision = client.evaluate_frozen_direct_scene_shroud_candidates(
            10,
            [FrozenDirectSceneShroudCandidate {
                binding_key: initial.binding_key,
                raw_status: ObjectShroudStatus::Clear,
                effectively_dead: false,
            }],
        );
        assert_eq!(
            decision,
            [FrozenDirectSceneShroudDecision {
                binding_key: initial.binding_key,
                decision: crate::drawable::SceneShroudDecision::RenderDrawable {
                    final_status: ObjectShroudStatus::Clear,
                    pushes_projected_shroud_pass: false,
                },
            }],
            "only an accepted direct scene candidate refreshes clear history"
        );

        assert_eq!(
            client.apply_frozen_direct_shroud_statuses(
                69,
                [FrozenDirectShroudStatus {
                    binding_key: initial.binding_key,
                    raw_status: ObjectShroudStatus::Fogged,
                    effectively_dead: false,
                }],
            ),
            1
        );
        assert!(
            !client
                .presentation_direct_drawable_state(host_epoch, object_id)
                .expect("current binding remains queryable")
                .fully_obscured,
            "the 2-second scene clear history keeps the next client update visible"
        );

        assert_eq!(
            client.apply_frozen_direct_shroud_statuses(
                70,
                [FrozenDirectShroudStatus {
                    binding_key: initial.binding_key,
                    raw_status: ObjectShroudStatus::Fogged,
                    effectively_dead: false,
                }],
            ),
            1
        );
        assert!(
            client
                .presentation_direct_drawable_state(host_epoch, object_id)
                .expect("same binding remains queryable")
                .fully_obscured,
            "the source limit is strict: frame 70 is outside a clear at frame 10"
        );

        let stale = PresentationDirectDrawableBindingKey {
            binding_generation: initial.binding_key.binding_generation.wrapping_add(1),
            ..initial.binding_key
        };
        assert!(
            client
                .evaluate_frozen_direct_scene_shroud_candidates(
                    71,
                    [FrozenDirectSceneShroudCandidate {
                        binding_key: stale,
                        raw_status: ObjectShroudStatus::Clear,
                        effectively_dead: false,
                    }],
                )
                .is_empty(),
            "a stale ledger must not refresh the current direct Drawable"
        );
    }

    #[test]
    fn hidden_direct_scene_candidate_does_not_refresh_clear_history() {
        let mut client = GameClient::new().unwrap();
        let object_id = 213;
        let host_epoch = 8;
        let entry = presentation_drawable_sync_for_test(
            object_id,
            host_epoch,
            "VisualA",
            true,
            false,
            [0.0, 0.0, 0.0],
            0.0,
        );
        assert_eq!(client.sync_presentation_drawables([entry]), (1, 0, 0));
        let binding = client
            .presentation_direct_drawable_state(host_epoch, object_id)
            .expect("resident direct visual gets a binding key")
            .binding_key;
        client
            .find_drawable_by_id_mut(binding.drawable_id)
            .expect("binding owns a live drawable")
            .as_any_mut()
            .downcast_mut::<crate::drawable::BasicDrawable>()
            .expect("presentation direct binding uses BasicDrawable")
            .set_drawable_hidden(true);
        assert!(
            client
                .presentation_direct_drawable_state(host_epoch, object_id)
                .expect("current binding exports exact scene-hidden state")
                .scene_effectively_hidden,
            "Main must cull the same C++ hidden predicate before model load"
        );

        assert_eq!(
            client.evaluate_frozen_direct_scene_shroud_candidates(
                10,
                [FrozenDirectSceneShroudCandidate {
                    binding_key: binding,
                    raw_status: ObjectShroudStatus::Clear,
                    effectively_dead: false,
                }],
            ),
            [FrozenDirectSceneShroudDecision {
                binding_key: binding,
                decision: crate::drawable::SceneShroudDecision::HiddenDirectDrawable,
            }],
            "the BasicDrawable hidden predicate remains authoritative at scene dispatch"
        );
        assert_eq!(
            client.apply_frozen_direct_shroud_statuses(
                11,
                [FrozenDirectShroudStatus {
                    binding_key: binding,
                    raw_status: ObjectShroudStatus::Fogged,
                    effectively_dead: false,
                }],
            ),
            1
        );
        assert!(
            client
                .presentation_direct_drawable_state(host_epoch, object_id)
                .expect("current binding remains queryable")
                .fully_obscured,
            "a hidden clear candidate cannot establish later C++ clear grace"
        );
    }

    #[test]
    fn presentation_visible_flag_does_not_suppress_cxx_direct_scene_clear_history() {
        let mut client = GameClient::new().unwrap();
        let object_id = 214;
        let host_epoch = 8;
        let entry = presentation_drawable_sync_for_test(
            object_id,
            host_epoch,
            "VisualA",
            true,
            false,
            [0.0, 0.0, 0.0],
            0.0,
        );
        assert_eq!(client.sync_presentation_drawables([entry]), (1, 0, 0));
        let binding = client
            .presentation_direct_drawable_state(host_epoch, object_id)
            .expect("resident direct visual gets a binding key")
            .binding_key;
        client
            .find_drawable_by_id_mut(binding.drawable_id)
            .expect("binding owns a live drawable")
            .set_visible(false);

        assert_eq!(
            client.evaluate_frozen_direct_scene_shroud_candidates(
                10,
                [FrozenDirectSceneShroudCandidate {
                    binding_key: binding,
                    raw_status: ObjectShroudStatus::Clear,
                    effectively_dead: false,
                }],
            ),
            [FrozenDirectSceneShroudDecision {
                binding_key: binding,
                decision: crate::drawable::SceneShroudDecision::RenderDrawable {
                    final_status: ObjectShroudStatus::Clear,
                    pushes_projected_shroud_pass: false,
                },
            }],
            "C++ scene visibility excludes Rust's broader presentation visible flag"
        );
        assert_eq!(
            client.apply_frozen_direct_shroud_statuses(
                69,
                [FrozenDirectShroudStatus {
                    binding_key: binding,
                    raw_status: ObjectShroudStatus::Fogged,
                    effectively_dead: false,
                }],
            ),
            1
        );
        assert!(
            !client
                .presentation_direct_drawable_state(host_epoch, object_id)
                .expect("same binding remains queryable")
                .fully_obscured,
            "the accepted clear candidate establishes the source two-second grace"
        );
    }

    #[test]
    fn direct_scene_uses_viewer_relative_stealth_not_generic_effective_stealth() {
        let mut client = GameClient::new().unwrap();
        let object_id = 215;
        let host_epoch = 8;
        let mut entry = presentation_drawable_sync_for_test(
            object_id,
            host_epoch,
            "VisualA",
            true,
            false,
            [0.0, 0.0, 0.0],
            0.0,
        );
        // The host has generic stealth for overlay/UI purposes, but C++
        // `m_hiddenByStealth` is a viewer-relative look. A friendly viewer
        // must still dispatch this Drawable (the WGPU frame supplies its
        // translucent presentation alpha separately).
        entry.effectively_stealthed = true;
        entry.scene_hidden_by_stealth = false;
        assert_eq!(
            client.sync_presentation_drawables([entry.clone()]),
            (1, 0, 0)
        );
        let binding = client
            .presentation_direct_drawable_state(host_epoch, object_id)
            .expect("resident direct visual gets a binding key");
        assert!(
            !binding.scene_effectively_hidden,
            "generic effective stealth must not become C++ hiddenByStealth"
        );
        assert!(matches!(
            client
                .evaluate_frozen_direct_scene_shroud_candidates(
                    10,
                    [FrozenDirectSceneShroudCandidate {
                        binding_key: binding.binding_key,
                        raw_status: ObjectShroudStatus::Clear,
                        effectively_dead: false,
                    }],
                )
                .as_slice(),
            [FrozenDirectSceneShroudDecision {
                decision: crate::drawable::SceneShroudDecision::RenderDrawable { .. },
                ..
            }]
        ));

        // An undetected non-allied viewer gets C++ `StealthLook::Invisible`.
        entry.scene_hidden_by_stealth = true;
        assert_eq!(client.sync_presentation_drawables([entry]), (0, 1, 0));
        assert!(
            client
                .presentation_direct_drawable_state(host_epoch, object_id)
                .expect("same direct binding remains queryable")
                .scene_effectively_hidden
        );
        assert_eq!(
            client.evaluate_frozen_direct_scene_shroud_candidates(
                11,
                [FrozenDirectSceneShroudCandidate {
                    binding_key: binding.binding_key,
                    raw_status: ObjectShroudStatus::Clear,
                    effectively_dead: false,
                }],
            ),
            [FrozenDirectSceneShroudDecision {
                binding_key: binding.binding_key,
                decision: crate::drawable::SceneShroudDecision::HiddenDirectDrawable,
            }],
            "only the frozen viewer-relative look hides the direct scene candidate"
        );
    }

    #[test]
    fn direct_visual_binding_prunes_nonresident_and_world_invalidation_mints_a_new_generation() {
        let mut client = GameClient::new().unwrap();
        let object_id = 313;
        let first_epoch = 17;
        let resident = presentation_drawable_sync_for_test(
            object_id,
            first_epoch,
            "VisualA",
            true,
            false,
            [0.0, 0.0, 0.0],
            0.0,
        );
        assert_eq!(client.sync_presentation_drawables([resident]), (1, 0, 0));
        let initial = client
            .presentation_direct_drawable_state(first_epoch, object_id)
            .expect("initial binding");

        // Nonresident, rather than `destroyed`, tears down the direct visual.
        let nonresident = presentation_drawable_sync_for_test(
            object_id,
            first_epoch,
            "VisualA",
            false,
            false,
            [0.0, 0.0, 0.0],
            0.0,
        );
        assert_eq!(client.sync_presentation_drawables([nonresident]), (0, 0, 1));
        assert_eq!(
            client.presentation_direct_drawable_state(first_epoch, object_id),
            None
        );

        let second_epoch = 18;
        let replacement = presentation_drawable_sync_for_test(
            object_id,
            second_epoch,
            "VisualA",
            true,
            false,
            [0.0, 0.0, 0.0],
            0.0,
        );
        assert_eq!(client.sync_presentation_drawables([replacement]), (1, 0, 0));
        let replacement = client
            .presentation_direct_drawable_state(second_epoch, object_id)
            .expect("new host epoch binding");
        assert!(
            replacement.binding_key.binding_generation > initial.binding_key.binding_generation
        );

        client.invalidate_presentation_drawable_world();
        assert!(client.get_drawable_ids().is_empty());
        assert_eq!(
            client.presentation_direct_drawable_state(second_epoch, object_id),
            None
        );

        let third_epoch = 19;
        let after_reset = presentation_drawable_sync_for_test(
            object_id,
            third_epoch,
            "VisualA",
            true,
            false,
            [0.0, 0.0, 0.0],
            0.0,
        );
        assert_eq!(client.sync_presentation_drawables([after_reset]), (1, 0, 0));
        let after_reset = client
            .presentation_direct_drawable_state(third_epoch, object_id)
            .expect("world replacement must reconstruct a fresh visual binding");
        assert!(
            after_reset.binding_key.binding_generation > replacement.binding_key.binding_generation
        );
        // The newly reconstructed direct binding is immediately usable by the
        // first Main render's frozen sidecar. With no prior scene Clear
        // dispatch, Fogged must cull it rather than inheriting a predecessor
        // world's clear-grace timer.
        assert_eq!(
            client.apply_frozen_direct_shroud_statuses(
                100,
                [FrozenDirectShroudStatus {
                    binding_key: after_reset.binding_key,
                    raw_status: ObjectShroudStatus::Fogged,
                    effectively_dead: false,
                }],
            ),
            1
        );
        assert!(
            client
                .presentation_direct_drawable_state(third_epoch, object_id)
                .expect("reconstructed binding remains current")
                .fully_obscured,
            "first render after world replacement must receive current direct shroud cull state"
        );
    }

    #[test]
    fn presentation_pose_keeps_world_translation_out_of_instance_transform() {
        // C++ Drawable::draw begins from the attached Object/Thing world
        // transform, then post-multiplies the local instance matrix exactly
        // once.  The host presentation sync must therefore put its frozen
        // position in BasicDrawable::position and only its yaw in the local
        // instance matrix.
        let mut client = GameClient::new().unwrap();
        let object_id = 91_337;
        let first_position = [37.0, -11.5, 8.25];
        let first_yaw = 0.61;

        assert_eq!(
            client.ensure_presentation_drawables([(
                object_id,
                "PresentationPoseParityTank".to_string(),
                first_position,
                first_yaw,
            )]),
            1
        );
        // The legacy convenience helper deliberately uses epoch zero and may
        // not leak a usable direct-host binding into Main's keyed pipeline.
        assert_eq!(
            client.presentation_direct_drawable_state(0, object_id),
            None
        );

        let drawable_id = client
            .get_drawable_for_object(object_id)
            .expect("presentation sync should bind the new drawable");
        let drawable = client
            .find_drawable_by_id(drawable_id)
            .expect("presentation drawable should remain registered");
        let expected_first = Matrix4::translation(Vector3::new(
            first_position[0],
            first_position[1],
            first_position[2],
        ))
        .mul(&Matrix4::rotation_y(first_yaw));
        assert_eq!(drawable.get_transform(), expected_first);

        let second_position = [-4.0, 19.0, 2.5];
        let second_yaw = -0.28;
        assert_eq!(
            client
                .apply_presentation_pose_to_drawables([(object_id, second_position, second_yaw,)]),
            1
        );
        let drawable = client
            .find_drawable_by_id(drawable_id)
            .expect("presentation drawable should remain registered after pose update");
        let expected_second = Matrix4::translation(Vector3::new(
            second_position[0],
            second_position[1],
            second_position[2],
        ))
        .mul(&Matrix4::rotation_y(second_yaw));
        assert_eq!(drawable.get_transform(), expected_second);
    }

    #[test]
    fn post_draw_pass_populates_drawable_icon_ui() {
        let mut client = GameClient::new().unwrap();
        let mut drawable = BasicDrawable::new(DrawableId::INVALID);
        drawable.set_caption_text("Beacon Alpha");
        drawable.overlay_data.health_region =
            Some(IRegion2D::new(ICoord2D::new(0, 0), ICoord2D::new(64, 12)));

        let drawable_id = client.register_drawable(Box::new(drawable)).unwrap();
        client.draw_drawable_icon_ui();

        let drawable = client
            .find_drawable_by_id(drawable_id)
            .expect("drawable should remain registered");
        let basic = drawable
            .as_any()
            .downcast_ref::<BasicDrawable>()
            .expect("registered drawable should be BasicDrawable");
        assert_eq!(basic.overlay_data.caption.as_deref(), Some("Beacon Alpha"));
        assert!(basic.overlay_data.visible);
    }

    #[test]
    fn test_snapshot_serialization_is_deterministic_for_same_state() {
        let mut client = GameClient::new().unwrap();

        let mut first = BasicDrawable::new(DrawableId::INVALID);
        first.set_template_name(Some("Tank".to_string()));
        first.set_position(Vector3::new(10.0, 20.0, 0.0));
        client.register_drawable(Box::new(first)).unwrap();

        let mut second = BasicDrawable::new(DrawableId::INVALID);
        second.set_template_name(Some("Jeep".to_string()));
        second.set_position(Vector3::new(-5.0, 4.0, 0.0));
        client.register_drawable(Box::new(second)).unwrap();

        let mut skipped = BasicDrawable::new(DrawableId::INVALID);
        skipped.set_template_name(Some("ShouldSkip".to_string()));
        let mut status = skipped.get_status();
        status.set(DrawableStatus::NO_SAVE);
        skipped.set_status(status);
        client.register_drawable(Box::new(skipped)).unwrap();

        let first_save = serialize_client(&mut client);
        let second_save = serialize_client(&mut client);
        assert_eq!(first_save, second_save);
    }

    #[test]
    fn test_snapshot_serialization_is_stable_across_drawable_hashmap_insertion_order() {
        let mut client_a = GameClient::new().unwrap();
        insert_basic_drawable_for_test(&mut client_a, 100, "Tank", Vector3::new(10.0, 20.0, 0.0));
        insert_basic_drawable_for_test(&mut client_a, 10, "Jeep", Vector3::new(-2.0, 3.0, 0.0));
        insert_basic_drawable_for_test(&mut client_a, 55, "Humvee", Vector3::new(1.0, 9.0, 0.0));

        let mut client_b = GameClient::new().unwrap();
        insert_basic_drawable_for_test(&mut client_b, 55, "Humvee", Vector3::new(1.0, 9.0, 0.0));
        insert_basic_drawable_for_test(&mut client_b, 100, "Tank", Vector3::new(10.0, 20.0, 0.0));
        insert_basic_drawable_for_test(&mut client_b, 10, "Jeep", Vector3::new(-2.0, 3.0, 0.0));

        let bytes_a = serialize_client(&mut client_a);
        let bytes_b = serialize_client(&mut client_b);
        assert_eq!(bytes_a, bytes_b);
    }

    #[test]
    fn test_snapshot_round_trip_serialization_is_stable() {
        ensure_templates_registered(&["RoundTripAlpha", "RoundTripBeta", "RoundTripGamma"]);

        let mut original = GameClient::new().unwrap();
        insert_basic_drawable_for_test(
            &mut original,
            30,
            "RoundTripAlpha",
            Vector3::new(1.0, 2.0, 0.0),
        );
        insert_basic_drawable_for_test(
            &mut original,
            5,
            "RoundTripBeta",
            Vector3::new(-4.0, 7.5, 0.0),
        );
        insert_basic_drawable_for_test(
            &mut original,
            77,
            "RoundTripGamma",
            Vector3::new(9.0, -3.0, 0.0),
        );

        let first_save = serialize_client(&mut original);
        let mut loaded = deserialize_client(&first_save);
        let second_save = serialize_client(&mut loaded);

        assert_eq!(first_save, second_save);
    }

    #[test]
    fn test_snapshot_serialization_is_stable_across_many_insertion_permutations() {
        let fixtures: Vec<(u32, &str, Vector3)> = vec![
            (41, "Alpha", Vector3::new(1.0, 2.0, 0.0)),
            (7, "Beta", Vector3::new(3.0, -2.0, 0.0)),
            (18, "Alpha", Vector3::new(-4.0, 5.0, 0.0)),
            (99, "Gamma", Vector3::new(6.0, 1.0, 0.0)),
            (3, "Delta", Vector3::new(-7.0, -8.0, 0.0)),
            (64, "Gamma", Vector3::new(2.5, 9.5, 0.0)),
            (12, "Epsilon", Vector3::new(0.0, 0.0, 0.0)),
            (55, "Beta", Vector3::new(8.0, -3.0, 0.0)),
        ];

        let mut baseline_client = GameClient::new().unwrap();
        for (id, name, pos) in &fixtures {
            insert_basic_drawable_for_test(&mut baseline_client, *id, name, *pos);
        }
        let baseline = serialize_client(&mut baseline_client);

        for seed in 0_u64..32_u64 {
            let mut indices: Vec<usize> = (0..fixtures.len()).collect();
            let mut state = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
            for i in (1..indices.len()).rev() {
                state = state
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let j = (state as usize) % (i + 1);
                indices.swap(i, j);
            }

            let mut client = GameClient::new().unwrap();
            for idx in indices {
                let (id, name, pos) = fixtures[idx];
                insert_basic_drawable_for_test(&mut client, id, name, pos);
            }

            let bytes = serialize_client(&mut client);
            assert_eq!(
                bytes, baseline,
                "serialization drift for permutation seed {}",
                seed
            );
        }
    }

    #[test]
    fn test_collect_saveable_drawables_sorted_orders_by_drawable_id_and_skips_nonsave() {
        let mut client = GameClient::new().unwrap();
        insert_basic_drawable_for_test(&mut client, 7, "Seven", Vector3::new(0.0, 0.0, 0.0));
        insert_basic_drawable_for_test(&mut client, 2, "Two", Vector3::new(0.0, 0.0, 0.0));
        insert_basic_drawable_for_test(&mut client, 5, "Five", Vector3::new(0.0, 0.0, 0.0));

        let mut skipped = BasicDrawable::new(DrawableId(4));
        skipped.set_id(DrawableId(4));
        skipped.set_template_name(Some("SkipMe".to_string()));
        let mut skipped_status = skipped.get_status();
        skipped_status.set(DrawableStatus::NO_SAVE);
        skipped.set_status(skipped_status);
        client.drawable_map.insert(DrawableId(4), Box::new(skipped));

        let saveable = client.collect_saveable_drawables_sorted().unwrap();
        let ids: Vec<u32> = saveable.iter().map(|(id, _)| id.0).collect();
        let names: Vec<&str> = saveable.iter().map(|(_, name)| name.as_str()).collect();
        assert_eq!(ids, vec![2, 5, 7]);
        assert_eq!(names, vec!["Two", "Five", "Seven"]);
    }

    #[test]
    fn test_save_uses_object_template_when_drawable_template_missing() {
        let mut client = GameClient::new().unwrap();
        let object_id: ObjectID = 990_001;

        let template: Arc<dyn gamelogic::thing_template::ThingTemplate> = Arc::new(
            LogicDefaultThingTemplate::new("FallbackTemplate".to_string()),
        );
        let object = Arc::new(RwLock::new(GameLogicObject::new_raw(
            template,
            object_id,
            ObjectStatusMaskType::none(),
            None,
        )));
        OBJECT_REGISTRY.register_object(object_id, &object);

        let mut drawable = BasicDrawable::new(DrawableId::INVALID);
        drawable.set_object_id(Some(object_id));
        let drawable_id = client.register_drawable(Box::new(drawable)).unwrap();

        let bytes = serialize_client(&mut client);
        assert!(!bytes.is_empty());
        assert_eq!(
            client
                .find_drawable_by_id(drawable_id)
                .and_then(|d| d.get_template_name()),
            Some("FallbackTemplate")
        );
        assert!(client
            .drawable_toc
            .iter()
            .any(|entry| entry.name == "FallbackTemplate"));

        OBJECT_REGISTRY.unregister_object(object_id);
    }

    #[test]
    fn test_snapshot_round_trip_mixed_no_save_drawables_matches_cpp_rules() {
        ensure_templates_registered(&[
            "FallbackPersistTemplate",
            "SkippedTemplate",
            "PersistedTemplate",
        ]);

        let mut client = GameClient::new().unwrap();
        let object_id: ObjectID = 990_010;

        let template: Arc<dyn gamelogic::thing_template::ThingTemplate> = Arc::new(
            LogicDefaultThingTemplate::new("FallbackPersistTemplate".to_string()),
        );
        let object = Arc::new(RwLock::new(GameLogicObject::new_raw(
            template,
            object_id,
            ObjectStatusMaskType::none(),
            None,
        )));
        OBJECT_REGISTRY.register_object(object_id, &object);

        let mut bound_no_save = BasicDrawable::new(DrawableId::INVALID);
        bound_no_save.set_object_id(Some(object_id));
        let mut bound_status = bound_no_save.get_status();
        bound_status.set(DrawableStatus::NO_SAVE);
        bound_no_save.set_status(bound_status);
        client.register_drawable(Box::new(bound_no_save)).unwrap();

        let mut skipped_no_save = BasicDrawable::new(DrawableId::INVALID);
        skipped_no_save.set_template_name(Some("SkippedTemplate".to_string()));
        let mut skipped_status = skipped_no_save.get_status();
        skipped_status.set(DrawableStatus::NO_SAVE);
        skipped_no_save.set_status(skipped_status);
        client.register_drawable(Box::new(skipped_no_save)).unwrap();

        let mut persisted = BasicDrawable::new(DrawableId::INVALID);
        persisted.set_template_name(Some("PersistedTemplate".to_string()));
        persisted.set_position(Vector3::new(2.0, 3.0, 0.0));
        client.register_drawable(Box::new(persisted)).unwrap();

        let first_save = serialize_client(&mut client);
        let mut loaded = deserialize_client(&first_save);
        let second_save = serialize_client(&mut loaded);

        assert_eq!(first_save, second_save);

        let loaded_bound_id = loaded
            .get_drawable_for_object(object_id)
            .expect("object-bound drawable should persist even with NO_SAVE");
        assert_eq!(
            loaded
                .find_drawable_by_id(loaded_bound_id)
                .and_then(|d| d.get_template_name()),
            Some("FallbackPersistTemplate")
        );

        assert_eq!(loaded.drawable_map.len(), 2);
        assert!(!loaded
            .drawable_map
            .values()
            .any(|drawable| { drawable.get_template_name() == Some("SkippedTemplate") }));

        OBJECT_REGISTRY.unregister_object(object_id);
    }

    #[test]
    fn test_register_drawable_preserves_explicit_template_name_over_object_fallback() {
        let mut client = GameClient::new().unwrap();
        let object_id: ObjectID = 990_002;

        let template: Arc<dyn gamelogic::thing_template::ThingTemplate> = Arc::new(
            LogicDefaultThingTemplate::new("FallbackTemplate".to_string()),
        );
        let object = Arc::new(RwLock::new(GameLogicObject::new_raw(
            template,
            object_id,
            ObjectStatusMaskType::none(),
            None,
        )));
        OBJECT_REGISTRY.register_object(object_id, &object);

        let mut drawable = BasicDrawable::new(DrawableId::INVALID);
        drawable.set_object_id(Some(object_id));
        drawable.set_template_name(Some("ExplicitTemplate".to_string()));
        let drawable_id = client.register_drawable(Box::new(drawable)).unwrap();

        let bytes = serialize_client(&mut client);
        assert!(!bytes.is_empty());
        assert_eq!(
            client
                .find_drawable_by_id(drawable_id)
                .and_then(|d| d.get_template_name()),
            Some("ExplicitTemplate")
        );
        assert!(client
            .drawable_toc
            .iter()
            .any(|entry| entry.name == "ExplicitTemplate"));
        assert!(!client
            .drawable_toc
            .iter()
            .any(|entry| entry.name == "FallbackTemplate"));

        OBJECT_REGISTRY.unregister_object(object_id);
    }

    #[test]
    fn test_drawable_template_equivalence_uses_final_override() {
        let mut factory = CommonThingFactory::new();
        let base_a = factory.new_template("TemplateA");
        let base_b = factory.new_template("TemplateB");
        let shared_final = factory.new_template("SharedFinal");
        base_a.set_next_override(Some(shared_final.clone()));
        base_b.set_next_override(Some(shared_final));

        let mut drawable = BasicDrawable::new(DrawableId::INVALID);
        drawable.set_template_name(Some("TemplateA".to_string()));

        assert!(GameClient::drawable_matches_saved_template(
            &drawable, &base_b, &factory
        ));

        let different = factory.new_template("DifferentFinal");
        assert!(!GameClient::drawable_matches_saved_template(
            &drawable, &different, &factory
        ));
    }

    #[test]
    fn test_message_dispatcher() {
        let dispatcher = GameClientMessageDispatcher::new();
        assert_eq!(dispatcher.message_filters.len(), 0);

        let move_cmd = GameMessage::new(GameMessageType::DoMoveTo(Coord3D::default()));
        assert_eq!(
            dispatcher.translate_game_message(&move_cmd),
            GameMessageDisposition::KeepMessage
        );

        let crc_cmd = GameMessage::new(GameMessageType::LogicCRC(0xABCD1234));
        assert_eq!(
            dispatcher.translate_game_message(&crc_cmd),
            GameMessageDisposition::KeepMessage
        );

        let new_game = GameMessage::new(GameMessageType::NewGame);
        assert_eq!(
            dispatcher.translate_game_message(&new_game),
            GameMessageDisposition::KeepMessage
        );

        let meta_toggle = GameMessage::new(GameMessageType::MetaToggleControlBar);
        assert_eq!(
            dispatcher.translate_game_message(&meta_toggle),
            GameMessageDisposition::DestroyMessage
        );
    }

    #[test]
    fn test_replay_update_culls_local_network_commands_but_keeps_crc() {
        let temp = tempfile::tempdir().expect("temp dir should be created");
        if let Some(global) = get_global_data() {
            let mut data = global.write();
            data.set_path_user_data(temp.path().to_string_lossy().to_string());
            data.map_name = "Maps/ReplayCullParity.map".to_string();
            data.pending_file.clear();
        }

        let mut writer = Recorder::new();
        writer
            .start_recording(1, 2, 3, 60)
            .expect("recording should start");
        writer.set_current_frame(5);
        writer
            .write_to_file(&GameMessage::new(GameMessageType::LogicCRC(0x1234ABCD)))
            .expect("recorded replay message should be written");
        writer.stop_recording();

        let replay_name = format!(
            "{}{}",
            writer.last_replay_filename(),
            writer.replay_extension()
        );

        let command_list_arc = get_command_list();
        {
            let mut command_list = command_list_arc
                .write()
                .expect("command list lock should be writable");
            command_list.clear_all_commands();
            command_list.append_message(GameMessage::new(GameMessageType::DoMoveTo(
                Coord3D::default(),
            )));
            command_list.append_message(GameMessage::new(GameMessageType::LogicCRC(0xDEADBEEF)));
            command_list.append_message(GameMessage::new(GameMessageType::NewGame));
        }

        let mut reader = Recorder::new();
        let command_cull: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {
            let command_list_arc = get_command_list();
            if let Ok(mut command_list) = command_list_arc.write() {
                command_list.retain_messages(|msg| {
                    let msg_type = msg.get_type().clone();
                    !(is_network_command_message(&msg_type)
                        && !matches!(msg_type, GameMessageType::LogicCRC(_)))
                });
            };
        });
        reader.set_command_cull(Some(command_cull));

        assert!(reader
            .playback_file(replay_name)
            .expect("replay playback should start"));
        reader.set_current_frame(0);
        reader.update();

        let messages = command_list_arc
            .read()
            .expect("command list lock should be readable")
            .snapshot_messages();

        assert!(messages
            .iter()
            .all(|msg| !matches!(msg.get_type(), GameMessageType::DoMoveTo(_))));
        assert!(messages
            .iter()
            .any(|msg| matches!(msg.get_type(), GameMessageType::LogicCRC(_))));
        assert!(messages
            .iter()
            .any(|msg| matches!(msg.get_type(), GameMessageType::NewGame)));

        reader.stop_playback();
        command_list_arc
            .write()
            .expect("command list lock should be writable")
            .clear_all_commands();
    }

    #[test]
    fn test_recorder_update_records_network_commands_from_source() {
        let temp = tempfile::tempdir().expect("temp dir should be created");
        if let Some(global) = get_global_data() {
            let mut data = global.write();
            data.set_path_user_data(temp.path().to_string_lossy().to_string());
            data.map_name = "Maps/ReplayRecordSourceParity.map".to_string();
            data.pending_file.clear();
        }

        let mut writer = Recorder::new();
        writer
            .start_recording(1, 2, 3, 60)
            .expect("recording should start");

        let source_state = std::sync::Arc::new(std::sync::Mutex::new(true));
        let source_state_clone = source_state.clone();
        let command_source: Arc<dyn Fn() -> Vec<GameMessage> + Send + Sync> = Arc::new(move || {
            let mut emit = source_state_clone
                .lock()
                .expect("command source mutex should not be poisoned");
            if !*emit {
                return Vec::new();
            }
            *emit = false;
            vec![
                GameMessage::new(GameMessageType::DoMoveTo(Coord3D {
                    x: 11.0,
                    y: 22.0,
                    z: 0.0,
                })),
                GameMessage::new(GameMessageType::MetaToggleControlBar),
            ]
        });
        writer.set_command_source(Some(command_source));
        writer.set_current_frame(9);
        writer.update();
        writer.stop_recording();

        let replay_name = format!(
            "{}{}",
            writer.last_replay_filename(),
            writer.replay_extension()
        );
        let replay_path = writer.replay_dir().join(&replay_name);
        assert!(replay_path.exists());

        let mut reader = Recorder::new();
        assert!(reader
            .playback_file(replay_name)
            .expect("recorded replay should be playable"));
        reader.set_current_frame(9);
        reader.update();

        let pending = reader.drain_pending_commands();
        assert!(pending.iter().any(|msg| {
            matches!(
                msg.get_type(),
                GameMessageType::DoMoveTo(coord)
                if (coord.x - 11.0).abs() <= f32::EPSILON
                    && (coord.y - 22.0).abs() <= f32::EPSILON
            )
        }));
        assert!(!pending
            .iter()
            .any(|msg| matches!(msg.get_type(), GameMessageType::MetaToggleControlBar)));
    }

    #[test]
    fn test_playback_file_clears_stale_pending_commands_when_sink_absent() {
        let temp = tempfile::tempdir().expect("temp dir should be created");
        if let Some(global) = get_global_data() {
            let mut data = global.write();
            data.set_path_user_data(temp.path().to_string_lossy().to_string());
            data.map_name = "Maps/ReplayPendingQueueParity.map".to_string();
            data.pending_file.clear();
        }

        let mut writer = Recorder::new();
        writer
            .start_recording(1, 2, 3, 60)
            .expect("recording should start");
        writer.set_current_frame(6);
        writer
            .write_to_file(&GameMessage::new(GameMessageType::DoMoveTo(
                Coord3D::default(),
            )))
            .expect("recorded replay command should be written");
        writer.stop_recording();

        let replay_name = format!(
            "{}{}",
            writer.last_replay_filename(),
            writer.replay_extension()
        );

        let mut reader = Recorder::new();
        assert!(reader
            .playback_file(replay_name.clone())
            .expect("first playback should start"));
        reader.set_current_frame(6);
        reader.update();
        reader.stop_playback();

        assert!(reader
            .playback_file(replay_name)
            .expect("second playback should start"));
        reader.set_current_frame(6);
        reader.update();
        let pending = reader.drain_pending_commands();

        let new_game_count = pending
            .iter()
            .filter(|msg| matches!(msg.get_type(), GameMessageType::NewGame))
            .count();
        let move_count = pending
            .iter()
            .filter(|msg| matches!(msg.get_type(), GameMessageType::DoMoveTo(_)))
            .count();

        assert_eq!(pending.len(), 2);
        assert_eq!(new_game_count, 1);
        assert_eq!(move_count, 1);
    }

    #[test]
    fn test_replay_version_playback_detects_combined_header_mismatches() {
        let temp = tempfile::tempdir().expect("temp dir should be created");
        if let Some(global) = get_global_data() {
            let mut data = global.write();
            data.set_path_user_data(temp.path().to_string_lossy().to_string());
            data.map_name = "Maps/ReplayVersionCombined.map".to_string();
            data.pending_file.clear();
            data.exe_crc = 0x0102_0304;
            data.ini_crc = 0x0506_0708;
        }

        let mut writer = Recorder::new();
        writer
            .start_recording(1, 2, 3, 60)
            .expect("recording should start");
        writer.set_current_frame(4);
        writer
            .write_to_file(&GameMessage::new(GameMessageType::LogicCRC(0x0A0B0C0D)))
            .expect("recorded replay message should be written");
        writer.stop_recording();

        let base_name = format!(
            "{}{}",
            writer.last_replay_filename(),
            writer.replay_extension()
        );
        let replays_dir = writer.replay_dir();
        let base_path = replays_dir.join(&base_name);
        assert!(base_path.exists());

        let (
            version_string_start,
            version_string_end,
            _version_time_start,
            version_number_offset,
            exe_crc_offset,
            ini_crc_offset,
        ) = replay_version_offsets(
            &std::fs::read(&base_path).expect("base replay should be readable for offset parsing"),
        );

        // Baseline: exact match must report no mismatch.
        assert!(!Recorder::new()
            .test_version_playback(base_name.clone())
            .expect("baseline replay should be readable"));

        let ext = writer.replay_extension();

        let version_and_exe_crc = format!("combined_version_exe_crc{ext}");
        write_variant(&base_path, &replays_dir, &version_and_exe_crc, |bytes| {
            mutate_utf16_first_code_unit(
                bytes,
                version_string_start,
                version_string_end,
                "version string",
            );

            let current = u32::from_le_bytes(
                bytes[exe_crc_offset..exe_crc_offset + 4]
                    .try_into()
                    .expect("exe CRC slice should be 4 bytes"),
            );
            bytes[exe_crc_offset..exe_crc_offset + 4]
                .copy_from_slice(&current.wrapping_add(1).to_le_bytes());
        });
        assert!(Recorder::new()
            .test_version_playback(version_and_exe_crc)
            .expect("combined mismatch replay should be readable"));

        let version_number_and_ini_crc = format!("combined_version_number_ini_crc{ext}");
        write_variant(
            &base_path,
            &replays_dir,
            &version_number_and_ini_crc,
            |bytes| {
                let version_number = u32::from_le_bytes(
                    bytes[version_number_offset..version_number_offset + 4]
                        .try_into()
                        .expect("version number slice should be 4 bytes"),
                );
                bytes[version_number_offset..version_number_offset + 4]
                    .copy_from_slice(&version_number.wrapping_add(1).to_le_bytes());

                let ini_crc = u32::from_le_bytes(
                    bytes[ini_crc_offset..ini_crc_offset + 4]
                        .try_into()
                        .expect("ini CRC slice should be 4 bytes"),
                );
                bytes[ini_crc_offset..ini_crc_offset + 4]
                    .copy_from_slice(&ini_crc.wrapping_add(1).to_le_bytes());
            },
        );
        assert!(Recorder::new()
            .test_version_playback(version_number_and_ini_crc)
            .expect("combined mismatch replay should be readable"));
    }

    #[test]
    fn test_region_3d_containment() {
        let region = Region3D {
            lo: Coord3D {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            hi: Coord3D {
                x: 10.0,
                y: 10.0,
                z: 10.0,
            },
        };

        let point_inside = Coord3D {
            x: 5.0,
            y: 5.0,
            z: 5.0,
        };
        let point_outside = Coord3D {
            x: 15.0,
            y: 5.0,
            z: 5.0,
        };

        // Test containment logic
        let inside = point_inside.x >= region.lo.x
            && point_inside.x <= region.hi.x
            && point_inside.y >= region.lo.y
            && point_inside.y <= region.hi.y
            && point_inside.z >= region.lo.z
            && point_inside.z <= region.hi.z;

        let outside = point_outside.x >= region.lo.x
            && point_outside.x <= region.hi.x
            && point_outside.y >= region.lo.y
            && point_outside.y <= region.hi.y
            && point_outside.z >= region.lo.z
            && point_outside.z <= region.hi.z;

        assert!(inside);
        assert!(!outside);
    }
}
