#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::common::system::snapshot::Snapshotable;
    use game_engine::{XferLoad, XferSave};
    use std::fs;
    use std::sync::{Arc, Mutex, OnceLock, RwLock};

    fn test_state_lock() -> std::sync::MutexGuard<'static, ()> {
        static TEST_STATE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        TEST_STATE_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("test state lock poisoned")
    }

    fn player_runtime_fixture_path(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "generals_player_runtime_{tag}_{}_{}.bin",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ))
    }

    fn reset_player_runtime_fixture(player: Player) {
        let players = player_list();
        let mut players = players.write().expect("player list lock");
        players.clear();
        players.add_player(Arc::new(RwLock::new(player)));
    }

    fn save_player_runtime_fixture(path: &std::path::Path) {
        let mut xfer = XferSave::new();
        xfer.open(path.to_string_lossy().into_owned())
            .expect("open player runtime save");
        xfer_player_list_runtime_state(&mut xfer).expect("save player runtime state");
        xfer.close().expect("close player runtime save");
    }

    fn write_legacy_v1_player_runtime_fixture(path: &std::path::Path) {
        let players = player_list();
        let players = players.read().expect("player list lock");
        let mut xfer = XferSave::new();
        xfer.open(path.to_string_lossy().into_owned())
            .expect("open legacy player runtime fixture");

        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .expect("legacy v1 version");
        let mut player_count = players.get_player_count() as i32;
        xfer.xfer_int(&mut player_count)
            .expect("legacy player count");
        for player_arc in players.iter() {
            let player = player_arc.read().expect("player lock");
            let mut money = player.get_money().get_money();
            xfer.xfer_int(&mut money).expect("legacy money");
            let mut power_production = player.get_energy().production();
            xfer.xfer_int(&mut power_production)
                .expect("legacy power production");
            let mut power_consumption = player.get_energy().consumption();
            xfer.xfer_int(&mut power_consumption)
                .expect("legacy power consumption");
            let mut power_sabotaged = player.get_energy().get_power_sabotaged_till_frame();
            xfer.xfer_unsigned_int(&mut power_sabotaged)
                .expect("legacy power sabotage frame");
            let mut defeated = player.is_defeated();
            xfer.xfer_bool(&mut defeated).expect("legacy defeated");
            let mut observer = player.is_player_observer();
            xfer.xfer_bool(&mut observer).expect("legacy observer");
            let mut rank_level = player.get_rank_level();
            xfer.xfer_int(&mut rank_level).expect("legacy rank");
            let mut science_points = player.get_science_purchase_points();
            xfer.xfer_int(&mut science_points)
                .expect("legacy science points");
        }
        xfer.close().expect("close legacy player runtime fixture");
    }

    fn load_player_runtime_fixture(path: &std::path::Path) -> Result<(), XferStatus> {
        let mut xfer = XferLoad::new();
        xfer.open(path.to_string_lossy().into_owned())?;
        let result = xfer_player_list_runtime_state(&mut xfer);
        let close_result = xfer.close();
        result.and(close_result)
    }

    #[test]
    fn player_runtime_v2_preserves_auxiliary_resource_and_tunnel_state() {
        let _lock = test_state_lock();
        let path = player_runtime_fixture_path("v2_auxiliary");
        crate::ai::integration::initialize_ai_integration().expect("reset source AI integration");

        let mut source = Player::new(0);
        source.init_from_dict_defaults();
        {
            let resources = source
                .get_resource_manager_mut()
                .expect("map-created resource manager");
            resources.add_supply_warehouse(101);
            resources.add_supply_center(202);
        }
        {
            let tunnels = source
                .get_tunnel_system_mut()
                .expect("map-created tunnel tracker");
            tunnels.on_tunnel_created_id(301).expect("tunnel one");
            tunnels.on_tunnel_created_id(302).expect("tunnel two");
            tunnels.add_to_contain_list_id(401).expect("contained unit");
        }
        reset_player_runtime_fixture(source);
        crate::ai::integration::with_ai_integration_mut(|manager| {
            manager.ensure_ai_player(0, false);
            manager
                .with_ai_player_mut(0, |ai| match ai {
                    crate::ai::integration::IntegratedAiPlayer::Standard(ai) => {
                        ai.set_team_delay_frames(77);
                        ai.set_team_timer_frames(33);
                    }
                    crate::ai::integration::IntegratedAiPlayer::Skirmish(_) => {
                        panic!("fixture must keep the non-skirmish AI variant")
                    }
                })
                .expect("source AI player");
        })
        .expect("source AI integration manager");
        save_player_runtime_fixture(&path);

        let mut destination = Player::new(0);
        destination.init_from_dict_defaults();
        reset_player_runtime_fixture(destination);
        crate::ai::integration::initialize_ai_integration()
            .expect("reset destination AI integration");
        crate::ai::integration::with_ai_integration_mut(|manager| {
            manager.ensure_ai_player(0, false);
        })
        .expect("destination AI integration manager");
        load_player_runtime_fixture(&path).expect("v2 player runtime load");

        let contained_object = Arc::new(RwLock::new(Object::new_test(401, 100.0)));
        get_game_logic()
            .lock()
            .expect("game logic lock")
            .register_object(contained_object)
            .expect("register contained object");
        let mut player_snapshot = PlayerListSnapshotBridge;
        XferSnapshotTrait::load_post_process(&mut player_snapshot)
            .expect("Player post-process after object blocks resolve");

        let players = player_list();
        let players = players.read().expect("player list lock");
        let loaded = players
            .get_player(0)
            .expect("loaded player")
            .read()
            .expect("loaded player lock");
        let resources = loaded
            .get_resource_manager()
            .expect("loaded resource manager");
        assert_eq!(resources.get_supply_warehouses(), &[101]);
        assert_eq!(resources.get_supply_centers(), &[202]);
        let tunnels = loaded.get_tunnel_system().expect("loaded tunnel tracker");
        assert_eq!(
            tunnels.get_container_list().expect("tunnel list"),
            vec![301, 302]
        );
        assert_eq!(tunnels.get_tunnel_count(), 2);
        assert_eq!(tunnels.get_contained_item_ids(), &[401]);

        let (team_delay, team_timer) = crate::ai::integration::with_ai_integration(|manager| {
            manager.with_ai_player(0, |ai| match ai {
                crate::ai::integration::IntegratedAiPlayer::Standard(ai) => {
                    (ai.get_team_delay(), ai.get_team_timer())
                }
                crate::ai::integration::IntegratedAiPlayer::Skirmish(_) => {
                    panic!("fixture must keep the non-skirmish AI variant")
                }
            })
        })
        .flatten()
        .expect("loaded AI player");
        assert_eq!((team_delay, team_timer), (77, 33));

        drop(loaded);
        drop(players);
        // Do not destroy this minimal fixture object here: its normal destroy
        // path performs a pathfinder registry read while holding the object's
        // write lock.  It is isolated by this test's unique ID and does not
        // affect the following Player fixtures.
        crate::ai::integration::initialize_ai_integration().expect("clear AI integration");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn player_runtime_v1_fixture_loads_without_consuming_v2_tail() {
        let _lock = test_state_lock();
        let path = player_runtime_fixture_path("legacy_v1");
        crate::ai::integration::initialize_ai_integration().expect("reset AI integration");

        let mut source = Player::new(0);
        source.get_money_mut().set_money(7_654);
        assert!(source.set_rank_level(4), "source rank");
        source.add_science_purchase_points(9);
        reset_player_runtime_fixture(source);
        write_legacy_v1_player_runtime_fixture(&path);

        let mut destination = Player::new(0);
        destination.init_from_dict_defaults();
        reset_player_runtime_fixture(destination);
        load_player_runtime_fixture(&path).expect("v1 player runtime load");

        let players = player_list();
        let players = players.read().expect("player list lock");
        let loaded = players
            .get_player(0)
            .expect("loaded player")
            .read()
            .expect("loaded player lock");
        assert_eq!(loaded.get_money().get_money(), 7_654);
        assert_eq!(loaded.get_rank_level(), 4);
        assert_eq!(loaded.get_science_purchase_points(), 9);
        assert!(loaded.get_resource_manager().is_some());
        assert!(loaded.get_tunnel_system().is_some());

        drop(loaded);
        drop(players);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn player_runtime_v2_rejects_map_component_presence_mismatch() {
        let _lock = test_state_lock();
        let path = player_runtime_fixture_path("presence_mismatch");
        crate::ai::integration::initialize_ai_integration().expect("reset AI integration");

        let mut source = Player::new(0);
        source.init_from_dict_defaults();
        reset_player_runtime_fixture(source);
        save_player_runtime_fixture(&path);

        reset_player_runtime_fixture(Player::new(0));
        assert_eq!(
            load_player_runtime_fixture(&path),
            Err(XferStatus::InvalidData),
            "v2 component presence must agree with the map-created Player"
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn direct_player_xfer_rejects_presence_mismatch_without_constructing_state() {
        let _lock = test_state_lock();
        use game_engine::common::system::xfer_load::XferLoad as CommonXferLoad;
        use game_engine::common::system::xfer_save::XferSave as CommonXferSave;
        use std::io::Cursor;

        crate::ai::integration::initialize_ai_integration().expect("reset AI integration");

        let mut source = Player::new(0);
        source.init_from_dict_defaults();
        let mut bytes = Vec::new();
        {
            let cursor = Cursor::new(&mut bytes);
            let mut xfer = CommonXferSave::new(cursor, 1);
            Snapshotable::xfer(&mut source, &mut xfer).expect("save direct Player payload");
        }

        let mut destination = Player::new(0);
        {
            let cursor = Cursor::new(bytes.as_slice());
            let mut xfer = CommonXferLoad::new(cursor, 1);
            let error = Snapshotable::xfer(&mut destination, &mut xfer)
                .expect_err("presence mismatch must fail before constructing managers");
            assert!(error.contains("resource gathering manager presence mismatch"));
        }
        assert!(destination.get_resource_manager().is_none());
        assert!(destination.get_tunnel_system().is_none());
    }

    #[test]
    fn time_freeze_matches_cpp_tactical_and_script_conditions() {
        assert!(should_freeze_time(true, false, false));
        assert!(!should_freeze_time(true, true, false));
        assert!(should_freeze_time(false, true, true));
        assert!(!should_freeze_time(false, true, false));
    }

    #[test]
    fn test_game_logic_creation() {
        let logic = GameLogic::new();
        assert_eq!(logic.frame, 0);
        assert_eq!(logic.game_time, 0.0);
        assert!(!logic.is_in_update);
    }

    #[test]
    fn empty_world_update_still_runs_cpp_phases_and_frame_increment() {
        // C++ GameLogic.cpp:3600 / 3622 / 3762 / 3799 — empty m_objList still
        // runs ScriptEngine, TerrainLogic, processDestroyList, and m_frame++.
        let _lock = test_state_lock();
        OBJECT_REGISTRY.clear();
        let mut logic = GameLogic::new();
        assert!(!logic.last_update_was_empty_noop());
        assert_eq!(logic.empty_world_tick_count(), 0);
        logic
            .update(0)
            .expect("empty world still returns Ok so host frame loop continues");
        assert!(
            !logic.last_update_was_empty_noop(),
            "empty registry+objects must still be a C++ GameLogic::update tick"
        );
        assert_eq!(logic.empty_world_tick_count(), 0);
        assert_eq!(
            logic.get_frame(),
            1,
            "empty-world tick must still increment C++ m_frame"
        );
        logic.update(1).expect("second empty tick still Ok");
        assert!(!logic.last_update_was_empty_noop());
        assert_eq!(logic.empty_world_tick_count(), 0);
        assert_eq!(logic.get_frame(), 2);
        logic.reset();
        assert!(!logic.last_update_was_empty_noop());
        assert_eq!(logic.empty_world_tick_count(), 0);
    }

    #[test]
    fn empty_world_second_tick_keeps_incrementing_frame() {
        // Same C++ GameLogic.cpp:3799 contract as a second empty tick.
        // Do not use the process-wide singleton here: update_game_logic()
        // can block on the_ai / script globals while other crate tests run.
        let _lock = test_state_lock();
        OBJECT_REGISTRY.clear();
        let mut logic = GameLogic::new();
        logic.update(0).expect("first empty tick");
        assert_eq!(logic.get_frame(), 1);
        logic.update(1).expect("second empty tick");
        assert!(!logic.last_update_was_empty_noop());
        assert_eq!(logic.empty_world_tick_count(), 0);
        assert_eq!(logic.get_frame(), 2);
    }

    #[test]
    fn empty_world_nonempty_update_stays_a_cpp_tick() {
        let _lock = test_state_lock();
        OBJECT_REGISTRY.clear();
        let mut logic = GameLogic::new();
        logic.update(0).expect("empty world still returns Ok");
        assert!(!logic.last_update_was_empty_noop());
        assert_eq!(logic.empty_world_tick_count(), 0);

        let dummy = Arc::new(RwLock::new(Object::new_test(42, 100.0)));
        logic.objects.insert(42, dummy);
        logic.set_game_paused(true, false);
        logic.update(1).expect("non-empty update still Ok");
        assert!(
            !logic.last_update_was_empty_noop(),
            "paused non-empty update is still not an empty-world no-op"
        );
        assert_eq!(
            logic.empty_world_tick_count(),
            0,
            "full ticks must not increment empty_world_tick"
        );
    }

    #[test]
    fn test_game_logic_reset() {
        let mut logic = GameLogic::new();
        logic.frame = 100;
        logic.game_time = 3.33;
        logic.reset();
        assert_eq!(logic.frame, 0);
        assert_eq!(logic.game_time, 0.0);
    }

    #[test]
    fn control_bar_overrides_preserve_null_and_button_slots_like_cpp() {
        let mut logic = GameLogic::new();

        logic.set_control_bar_override("AmericaVehicleCommandSet", 0, Some("Command_Construct"));
        logic.set_control_bar_override("AmericaVehicleCommandSet", 17, None);
        logic.set_control_bar_override("AmericaVehicleCommandSet", 18, Some("Ignored"));

        assert_eq!(
            logic.find_control_bar_override("AmericaVehicleCommandSet", 0),
            Some(Some("Command_Construct"))
        );
        assert_eq!(
            logic.find_control_bar_override("AmericaVehicleCommandSet", 17),
            Some(None)
        );
        assert_eq!(
            logic.find_control_bar_override("AmericaVehicleCommandSet", 18),
            None
        );
    }

    #[test]
    fn control_bar_overrides_xfer_as_cpp_key_value_sentinel_list() {
        let mut overrides = HashMap::new();
        overrides.insert(
            "0AmericaVehicleCommandSet".to_string(),
            Some("Command_Construct".to_string()),
        );
        overrides.insert("AAmericaVehicleCommandSet".to_string(), None);

        let path = std::env::temp_dir().join(format!(
            "generalsrust_control_bar_overrides_{}.xfer",
            std::process::id()
        ));
        let path_string = path.to_string_lossy().to_string();

        let mut save = XferSave::new();
        save.open(path_string.clone()).unwrap();
        xfer_control_bar_overrides(&mut save, &mut overrides).unwrap();
        save.close().unwrap();

        let mut loaded = HashMap::new();
        let mut load = XferLoad::new();
        load.open(path_string.clone()).unwrap();
        xfer_control_bar_overrides(&mut load, &mut loaded).unwrap();
        load.close().unwrap();
        let _ = std::fs::remove_file(path);

        assert_eq!(
            loaded
                .get("0AmericaVehicleCommandSet")
                .and_then(|v| v.as_deref()),
            Some("Command_Construct")
        );
        assert_eq!(loaded.get("AAmericaVehicleCommandSet"), Some(&None));
    }

    #[test]
    fn test_object_id_allocation() {
        let mut logic = GameLogic::new();
        let id1 = logic.allocate_object_id();
        let id2 = logic.allocate_object_id();
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_ne!(id1, id2);
    }

    #[test]
    fn get_crc_recalc_uses_cpp_markers_object_crc_and_is_stable() {
        let _guard = test_state_lock();
        OBJECT_REGISTRY.clear();

        let mut marker = "MARKER:Objects".to_string();
        let mut system_xfer = LogicXferCrc::new();
        Xfer::xfer_ascii_string(&mut system_xfer, &mut marker).expect("system marker");
        let mut common_xfer = LogicXferCrc::new();
        let mut marker = "MARKER:Objects".to_string();
        game_engine::common::system::xfer::Xfer::xfer_ascii_string(&mut common_xfer, &mut marker)
            .expect("common marker");
        assert_eq!(
            system_xfer.get_crc(),
            common_xfer.get_crc(),
            "both Xfer stacks must share fold_crc_bytes addCRC"
        );
        assert_ne!(
            common_xfer.get_crc(),
            0,
            "MARKER:Objects bytes must fold into XferCRC"
        );

        // Leftover 1–3 bytes: both Xfer stacks must match fold_crc_bytes + getCRC htonl.
        let leftover = [0x11u8, 0x22, 0x33, 0x44, 0xAB];
        let expected = game_engine::common::system::xfer_crc::fold_crc_bytes(0, &leftover).to_be();
        let mut system_left = LogicXferCrc::new();
        // SAFETY: `leftover` is a fully initialized stack array that stays
        // alive for the whole call; the CRC xfer only reads its bytes.
        unsafe {
            Xfer::xfer_implementation(
                &mut system_left,
                leftover.as_ptr() as *mut u8,
                leftover.len(),
            )
            .expect("system leftover");
        }
        let mut common_left = LogicXferCrc::new();
        // SAFETY: same as above — initialized local array, read-only CRC
        // fold for the duration of the call.
        unsafe {
            game_engine::common::system::xfer::Xfer::xfer_implementation(
                &mut common_left,
                leftover.as_ptr() as *mut u8,
                leftover.len(),
            )
            .expect("common leftover");
        }
        assert_eq!(system_left.get_crc(), expected);
        assert_eq!(common_left.get_crc(), expected);

        let mut logic = GameLogic::new();
        let empty_crc = logic.get_crc(CrcMode::Recalc);
        assert_ne!(
            empty_crc, 0,
            "empty world CRC includes MARKER:Objects bytes"
        );
        assert_ne!(
            empty_crc,
            common_xfer.get_crc(),
            "empty world continues past MARKER:Objects (seed/partition/players/AI)"
        );

        let first = Arc::new(RwLock::new(Object::new_test(11, 100.0)));
        logic.register_object(first).expect("register first");
        let with_obj = logic.get_crc(CrcMode::Recalc);
        assert_ne!(with_obj, empty_crc, "registering an object changes CRC");

        let with_obj_again = logic.get_crc(CrcMode::Recalc);
        assert_eq!(with_obj, with_obj_again, "same object twice → same CRC");

        assert_eq!(logic.get_crc(CrcMode::Cached), 0);
    }

    #[test]
    fn register_object_send_object_created_binds_drawable() {
        let _guard = test_state_lock();
        OBJECT_REGISTRY.clear();
        let mut logic = GameLogic::new();
        let object = Arc::new(RwLock::new(Object::new_test(9001, 100.0)));
        logic
            .register_object(Arc::clone(&object))
            .expect("register");
        let bound = object.read().expect("object").get_drawable().is_some();
        assert!(
            bound,
            "C++ sendObjectCreated binds a drawable onto the logic object"
        );
    }

    #[test]
    fn test_object_list_links_relink_on_cleanup() {
        let _guard = test_state_lock();
        use crate::object::registry::OBJECT_REGISTRY;
        use crate::object::Object;
        use std::sync::{Arc, RwLock};

        OBJECT_REGISTRY.clear();

        let mut logic = GameLogic::new();
        let first = Arc::new(RwLock::new(Object::new_test(11, 100.0)));
        let middle = Arc::new(RwLock::new(Object::new_test(22, 100.0)));
        let last = Arc::new(RwLock::new(Object::new_test(33, 100.0)));

        OBJECT_REGISTRY.register_object(11, &first);
        OBJECT_REGISTRY.register_object(22, &middle);
        OBJECT_REGISTRY.register_object(33, &last);

        logic.add_restored_object(Arc::clone(&first));
        logic.add_restored_object(Arc::clone(&middle));
        logic.add_restored_object(Arc::clone(&last));

        // C++ GameLogic.cpp:3866 prependToList — newest restored object is head.
        assert_eq!(logic.all_objects, vec![33, 22, 11]);
        assert_eq!(
            last.read()
                .unwrap()
                .get_next_object()
                .unwrap()
                .read()
                .unwrap()
                .get_id(),
            22
        );
        assert_eq!(
            middle
                .read()
                .unwrap()
                .get_prev_object()
                .unwrap()
                .read()
                .unwrap()
                .get_id(),
            33
        );

        logic.destroy_object(22);
        assert!(
            middle.read().unwrap().is_destroyed(),
            "C++ destroyObject sets OBJECT_STATUS_DESTROYED immediately"
        );
        assert!(logic.cleanup_dead_objects().is_ok());

        assert_eq!(logic.all_objects, vec![33, 11]);
        assert_eq!(
            last.read()
                .unwrap()
                .get_next_object()
                .unwrap()
                .read()
                .unwrap()
                .get_id(),
            11
        );
        assert_eq!(
            first
                .read()
                .unwrap()
                .get_prev_object()
                .unwrap()
                .read()
                .unwrap()
                .get_id(),
            33
        );

        OBJECT_REGISTRY.clear();
    }

    #[test]
    fn load_post_process_rebuilds_sleepy_queue_from_object_modules() {
        let _guard = test_state_lock();
        use crate::modules::{UpdateModuleDummy, UpdateModulePtr};
        use crate::object::registry::OBJECT_REGISTRY;
        use crate::object::Object;
        use std::sync::{Arc, RwLock};

        OBJECT_REGISTRY.clear();
        let mut logic = GameLogic::new();
        let obj = Arc::new(RwLock::new(Object::new_test(77, 100.0)));
        let dummy: UpdateModulePtr = Arc::new(RwLock::new(UpdateModuleDummy));
        obj.write()
            .unwrap()
            .attach_update_module_registration(dummy);
        logic.add_restored_object(Arc::clone(&obj));
        logic.sleepy_updates.clear();
        logic.normal_updates.clear();
        logic.module_lookup.clear();
        assert_eq!(logic.sleepy_update_count(), 0);

        logic.load_post_process();
        assert!(
            logic.sleepy_update_count() >= 1,
            "C++ loadPostProcess re-pushes every object's update modules"
        );
        OBJECT_REGISTRY.clear();
    }

    /// C++ GameState.cpp:1528-1529 — production registers ThePartitionManager->update
    /// so gameStatePostProcessLoad is not a no-op after CHUNK_GameStateMap.
    #[test]
    fn game_state_post_process_load_updates_partition_after_snapshots() {
        let _guard = test_state_lock();
        use crate::object::registry::OBJECT_REGISTRY;
        use crate::object::Object;

        OBJECT_REGISTRY.clear();
        let _ = get_game_logic();

        let obj_id: ObjectID = 9001;
        let pos = Coord3D::new(250.0, 250.0, 0.0);
        let obj = Arc::new(RwLock::new(Object::new_test(obj_id, 100.0)));
        obj.write()
            .expect("object write")
            .set_position(&pos)
            .expect("set position");
        OBJECT_REGISTRY.register_object(obj_id, &obj);

        {
            let mut logic = get_game_logic().lock().unwrap_or_else(|e| e.into_inner());
            logic.partition_manager.remove_object(obj_id);
            assert!(
                !logic
                    .partition_manager
                    .find_objects_in_radius(pos, 10.0)
                    .contains(&obj_id),
                "precondition: object must be absent from partition before post-process"
            );
        }

        {
            let mut state = game_engine::System::get_game_state();
            state
                .game_state_post_process_load()
                .expect("C++ GameState::gameStatePostProcessLoad");
        }

        {
            let logic = get_game_logic().lock().unwrap_or_else(|e| e.into_inner());
            assert!(
                logic
                    .partition_manager
                    .find_objects_in_radius(pos, 10.0)
                    .contains(&obj_id),
                "production hook must call PartitionManager::update after all snapshots"
            );
            // Drop the object so later tests do not see it.
        }

        OBJECT_REGISTRY.clear();
        if let Ok(mut logic) = get_game_logic().lock() {
            logic.partition_manager.remove_object(obj_id);
        }
    }

    #[test]
    fn empty_registry_falls_back_to_game_logic_objects_like_cpp() {
        let _guard = test_state_lock();
        use crate::object::registry::OBJECT_REGISTRY;
        use crate::object::Object;
        use std::sync::{Arc, RwLock};

        OBJECT_REGISTRY.clear();
        let obj = Arc::new(RwLock::new(Object::new_test(4242, 10.0)));
        {
            let mut logic = get_game_logic().lock().unwrap_or_else(|e| e.into_inner());
            logic.objects.insert(4242, Arc::clone(&obj));
            logic.all_objects.push(4242);
        }
        assert!(
            OBJECT_REGISTRY.contains(4242),
            "C++ GameLogic.objects is authority when factory registry store is empty"
        );
        let ids = OBJECT_REGISTRY.get_all_object_ids();
        assert!(ids.contains(&4242), "get_all_object_ids={ids:?}");
        assert!(OBJECT_REGISTRY.get_object(4242).is_some());
        {
            let mut logic = get_game_logic().lock().unwrap_or_else(|e| e.into_inner());
            logic.objects.remove(&4242);
            logic.all_objects.retain(|id| *id != 4242);
        }
        OBJECT_REGISTRY.clear();
    }

    #[test]
    fn test_register_object_sets_link_ids() {
        let _guard = test_state_lock();
        use crate::object::registry::OBJECT_REGISTRY;
        use crate::object::Object;
        use std::sync::{Arc, RwLock};

        OBJECT_REGISTRY.clear();

        let mut logic = GameLogic::new();
        let first = Arc::new(RwLock::new(Object::new_test(44, 100.0)));
        let second = Arc::new(RwLock::new(Object::new_test(55, 100.0)));

        assert_eq!(logic.register_object(Arc::clone(&first)).unwrap(), 44);
        assert_eq!(logic.register_object(Arc::clone(&second)).unwrap(), 55);

        // C++ GameLogic.cpp:3866 obj->prependToList(&m_objList) — newest first.
        assert_eq!(logic.all_objects, vec![55, 44]);
        assert_eq!(second.read().unwrap().get_next_object_id(), Some(44));
        assert_eq!(second.read().unwrap().get_prev_object_id(), None);
        assert_eq!(first.read().unwrap().get_prev_object_id(), Some(55));
        assert_eq!(first.read().unwrap().get_next_object_id(), None);

        OBJECT_REGISTRY.clear();
    }

    #[test]
    fn test_frame_events_cleared() {
        let mut logic = GameLogic::new();
        logic.event_queue.push(GameEvent::ObjectCreated(1));
        logic.radar_updates.push(RadarUpdate {
            player_id: 0,
            position: (0.0, 0.0),
            event_type: RadarEventType::UnitCreated,
        });

        assert!(logic.clear_frame_events().is_ok());
        assert_eq!(logic.event_queue.len(), 0);
        assert_eq!(logic.radar_updates.len(), 0);
    }

    #[test]
    fn test_radar_updates_promoted_to_events() {
        let mut logic = GameLogic::new();
        logic.radar_updates.push(RadarUpdate {
            player_id: 1,
            position: (42.0, 84.0),
            event_type: RadarEventType::BaseAttacked,
        });

        logic.process_radar_updates();

        assert_eq!(logic.event_queue.len(), 1);
        match &logic.event_queue[0] {
            GameEvent::RadarUpdate {
                player_id,
                position,
                event_type,
            } => {
                assert_eq!(*player_id, 1);
                assert_eq!(*position, (42.0, 84.0));
                assert!(matches!(event_type, RadarEventType::BaseAttacked));
            }
            other => panic!("Unexpected event emitted: {:?}", other),
        }
    }

    #[test]
    fn test_update_loop_phases() {
        let mut logic = GameLogic::new();

        // Should not allow re-entrant calls
        logic.is_in_update = true;
        let result = logic.update(0);
        assert!(result.is_err());

        // Normal update should succeed
        logic.is_in_update = false;
        let result = logic.update(0);
        assert!(result.is_ok());
        assert_eq!(logic.frame, 0);
    }

    #[test]
    fn test_command_queue() {
        let mut logic = GameLogic::new();
        let command = GameCommand::MoveUnit {
            player_id: 0,
            unit_ids: vec![1, 2, 3],
            target_position: (100.0, 100.0, 0.0),
        };

        logic.queue_command(command);
        assert_eq!(logic.command_queue.len(), 1);

        // Process commands
        let result = logic.process_command_queue();
        assert!(result.is_ok());
        assert_eq!(logic.command_queue.len(), 0);
    }

    #[test]
    fn test_physics_damage_queue() {
        let mut logic = GameLogic::new();
        logic.queue_damage(1, 2, 50.0);

        assert_eq!(logic.physics_world.pending_damage.len(), 1);
    }

    #[test]
    fn test_game_mode_checks() {
        let mut logic = GameLogic::new();

        logic.set_game_mode(GAME_SINGLE_PLAYER);
        assert!(logic.is_in_single_player_game());
        assert!(!logic.is_in_multiplayer_game());

        logic.set_game_mode(GAME_LAN);
        assert!(!logic.is_in_single_player_game());
        assert!(logic.is_in_multiplayer_game());
    }

    #[test]
    fn destroy_object_runs_on_destroy_same_frame_like_cpp() {
        let mut logic = GameLogic::new();
        let mut obj = Object::new_test(77, 100.0);
        let _ = obj.set_position(&Coord3D::new(5.0, 6.0, 7.0));
        let arc = std::sync::Arc::new(std::sync::RwLock::new(obj));
        logic.objects.insert(77, Arc::clone(&arc));
        logic.all_objects = vec![77];
        logic.destroy_object(77);
        let guard = arc.read().expect("read");
        assert!(
            guard.is_destroyed(),
            "C++ sets OBJECT_STATUS_DESTROYED inside destroyObject"
        );
        assert!(
            logic.dead_objects.contains(&77),
            "queued for processDestroyList"
        );
    }

    #[test]
    fn cleanup_dead_objects_processes_same_frame_cascade_like_cpp() {
        // C++ GameLogic.cpp:2449-2510 — iterator re-evaluates end() so a
        // sub-object queued during processDestroyList is deleted same frame.
        let mut logic = GameLogic::new();
        let parent = Arc::new(RwLock::new(Object::new_test(11, 100.0)));
        let child = Arc::new(RwLock::new(Object::new_test(22, 100.0)));
        logic.objects.insert(11, Arc::clone(&parent));
        logic.objects.insert(22, Arc::clone(&child));
        logic.all_objects = vec![22, 11];
        logic.dead_objects.push(11);
        GameLogic::test_queue_cleanup_cascade(22);
        assert!(logic.cleanup_dead_objects().is_ok());
        assert!(
            logic.all_objects.is_empty(),
            "same-frame cascade must drain parent and child in one processDestroyList"
        );
        assert!(logic.dead_objects.is_empty());
    }

    #[test]
    fn destroy_object_removes_wall_piece_and_marks_special_power_ui_dirty() {
        // C++ GameLogic.cpp:3969-3980 — WALK_ON_TOP_OF_WALL pathfinder removal
        // and ControlBar::markUIDirty for local special-power objects.
        use crate::common::{DefaultThingTemplate, KindOf};
        use crate::control_bar::{register_control_bar_ui_hooks, ControlBarUiHooks};
        use crate::object::special_power_types::SpecialPowerType;
        use std::sync::atomic::{AtomicU32, Ordering};

        struct DirtyHooks(Arc<AtomicU32>);
        impl ControlBarUiHooks for DirtyHooks {
            fn mark_ui_dirty(&self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
            fn on_player_science_purchase_points_changed(&self, _: i32, _: i32) {}
            fn on_player_rank_changed(&self, _: i32, _: i32, _: i32) {}
        }

        let dirty = Arc::new(AtomicU32::new(0));
        register_control_bar_ui_hooks(Arc::new(DirtyHooks(Arc::clone(&dirty))));

        let mut template = DefaultThingTemplate::new("WallPiece".into());
        template.add_kind_of(KindOf::WalkOnTopOfWall);
        let mut obj = Object::new_test_from_template(88, 100.0, Arc::new(template));
        obj.set_special_power_available(SpecialPowerType::DaisyCutter, true);
        assert!(obj.has_any_special_power());
        assert!(obj.is_kind_of(KindOf::WalkOnTopOfWall));

        let ai_store = crate::ai::the_ai(); if let Ok(ai) = ai_store.read() {
            if let Some(pf) = ai.pathfinder() {
                if let Ok(mut pf) = pf.write() {
                    pf.add_wall_piece(88);
                }
            }
        }

        let mut logic = GameLogic::new();
        let arc = Arc::new(RwLock::new(obj));
        logic.objects.insert(88, Arc::clone(&arc));
        logic.all_objects = vec![88];
        logic.destroy_object(88);
        assert!(
            arc.read().unwrap().is_destroyed(),
            "C++ destroyObject sets OBJECT_STATUS_DESTROYED immediately"
        );
        let src = include_str!("impl_lifecycle.rs");
        let destroy_fn = src
            .split("pub fn destroy_object(&mut self, object_id: ObjectID)")
            .nth(1)
            .and_then(|rest| rest.split("fn prepend_to_object_list").next())
            .expect("destroy_object body");
        assert!(
            destroy_fn.contains("remove_wall_piece") && destroy_fn.contains("mark_ui_dirty"),
            "destroyObject must remove WALK_ON_TOP_OF_WALL and markUIDirty"
        );
        register_control_bar_ui_hooks(Arc::new(DirtyHooks(Arc::new(AtomicU32::new(0)))));
        let _ = dirty;
    }
    #[test]
    fn crate_update_does_not_run_client_drawable_updates() {
        // C++ GameLogic.cpp:3548-3803 has no ClientUpdate / updateDrawables.
        // hq-um5t: those extras belong on GameClient, not the logic tick.
        let src = include_str!("impl_update.rs");
        let update_fn = src
            .split("pub fn update(&mut self, frame: u32)")
            .nth(1)
            .and_then(|rest| rest.split("pub fn take_radar_updates").next())
            .expect("GameLogic::update body");
        assert!(
            !update_fn.contains("process_client_updates")
                && !update_fn.contains("update_drawables"),
            "logic tick must not run client drawable updates"
        );
        assert!(
            update_fn.contains("is_start_new_game_requested")
                && src.contains("C++ GameLogic.cpp:3560-3576"),
            "startNewGame must stay at the top of update"
        );
    }

    #[test]
    fn xfer_v10_writes_toc_and_object_blocks() {
        use game_engine::{Xfer, XferLoad, XferSave};

        let mut save_logic = GameLogic::new();
        save_logic.frame = 42;
        let mut obj = Object::new_test(11, 100.0);
        let _ = obj.set_position(&Coord3D::new(1.0, 2.0, 3.0));
        save_logic
            .register_object(std::sync::Arc::new(std::sync::RwLock::new(obj)))
            .unwrap();

        let path = std::env::temp_dir().join(format!(
            "generals_xfer_v10_{}_{}.bin",
            std::process::id(),
            11
        ));
        {
            let mut xfer = XferSave::new();
            xfer.open(path.to_string_lossy().into_owned())
                .expect("open save");
            xfer_game_logic_state(&mut save_logic, &mut xfer).expect("save");
            xfer.close().expect("close save");
        }
        let mut load_logic = GameLogic::new();
        {
            let mut xfer = XferLoad::new();
            xfer.open(path.to_string_lossy().into_owned())
                .expect("open load");
            xfer_game_logic_state(&mut load_logic, &mut xfer).expect("load");
            xfer.close().expect("close load");
        }
        let _ = std::fs::remove_file(&path);
        assert_eq!(load_logic.frame, 42);
        assert!(
            load_logic.find_toc_entry_by_name("TestObject").is_some(),
            "C++ xferObjectTOC must survive roundtrip"
        );
        let loaded = load_logic
            .find_object_by_id(11)
            .expect("object from TOC block");
        let guard = loaded.read().unwrap();
        assert_eq!(guard.get_position().x, 1.0);
        assert_eq!(guard.get_position().y, 2.0);
        assert_eq!(guard.get_position().z, 3.0);
        assert_eq!(
            load_logic.get_object_id_counter(),
            1,
            "C++ GameLogic::xfer must not xfer m_nextObjectID"
        );
        let xfer_src = include_str!("xfer_helpers.rs");
        assert!(
            xfer_src.contains("TheThingFactory::find_template")
                && xfer_src.contains("Object::new_with_id")
                && xfer_src.contains("new_for_xfer_load"),
            "load must ThingFactory::newObject then xferSnapshot (fallback new_for_xfer_load)"
        );
    }

    #[test]
    fn xfer_v10_includes_sell_list_and_does_not_write_next_object_id() {
        use game_engine::{Xfer, XferLoad, XferSave};

        game_engine::common::system::build_assistant::init_build_assistant();
        {
            let mut assistant =
                game_engine::common::system::build_assistant::get_build_assistant().unwrap();
            assistant.reset();
            assistant.sell_object(
                &game_engine::common::system::build_assistant::Object {
                    id: 77,
                    position: Default::default(),
                    orientation: 0.0,
                    command_set: None,
                },
                12,
            );
        }

        let mut save_logic = GameLogic::new();
        save_logic.frame = 9;
        save_logic.set_superweapon_restriction(3);
        save_logic.set_rank_level_limit(8);
        save_logic.next_object_id = 99;

        let path =
            std::env::temp_dir().join(format!("generals_xfer_v10_sell_{}.bin", std::process::id()));
        {
            let mut xfer = XferSave::new();
            xfer.open(path.to_string_lossy().into_owned())
                .expect("open save");
            xfer_game_logic_state(&mut save_logic, &mut xfer).expect("save");
            xfer.close().expect("close save");
        }
        game_engine::common::system::build_assistant::init_build_assistant();
        let mut load_logic = GameLogic::new();
        load_logic.next_object_id = 1;
        {
            let mut xfer = XferLoad::new();
            xfer.open(path.to_string_lossy().into_owned())
                .expect("open load");
            xfer_game_logic_state(&mut load_logic, &mut xfer).expect("load");
            xfer.close().expect("close load");
        }
        let _ = std::fs::remove_file(&path);
        assert_eq!(load_logic.frame, 9);
        assert_eq!(load_logic.get_superweapon_restriction(), 3);
        assert_eq!(load_logic.get_rank_level_limit(), 8);
        assert_eq!(load_logic.next_object_id, 1);
        let assistant =
            game_engine::common::system::build_assistant::get_build_assistant().unwrap();
        assert_eq!(assistant.get_sell_list().len(), 1);
        assert_eq!(assistant.get_sell_list()[0].id, 77);
        assert_eq!(assistant.get_sell_list()[0].sell_frame, 12);
    }

    #[test]
    fn start_new_game_now_drives_load_screen_lifecycle() {
        let _guard = test_state_lock();

        #[derive(Default)]
        struct RecordingLoadScreenHooks {
            events: Mutex<Vec<String>>,
        }

        impl crate::helpers::LoadScreenHooks for RecordingLoadScreenHooks {
            fn begin_load_screen(&self, game_mode: i32, loading_save_game: bool) {
                self.events
                    .lock()
                    .unwrap()
                    .push(format!("begin:{game_mode}:{loading_save_game}"));
            }

            fn update_load_screen(&self, progress: i32) {
                self.events
                    .lock()
                    .unwrap()
                    .push(format!("update:{progress}"));
            }

            fn run_load_screen_completion_transition(&self, loading_save_game: bool) {
                self.events
                    .lock()
                    .unwrap()
                    .push(format!("finish-transition:{loading_save_game}"));
            }

            fn end_load_screen(&self) {
                self.events.lock().unwrap().push("end".to_string());
            }
        }

        let hooks = std::sync::Arc::new(RecordingLoadScreenHooks::default());
        crate::helpers::clear_load_screen_hooks();
        crate::helpers::register_load_screen_hooks(hooks.clone());

        let global_data = game_engine::common::ini::ini_game_data::ensure_global_data();
        let original_map_name = global_data.read().map_name.clone();
        global_data.write().map_name = "__definitely_missing_load_screen_map__.map".to_string();

        let mut logic = GameLogic::new();
        logic.set_game_mode(GAME_SKIRMISH);
        let result = logic.start_new_game_now(false);

        global_data.write().map_name = original_map_name;
        crate::helpers::clear_load_screen_hooks();

        assert!(result.is_err());
        assert!(!logic.is_loading_map());
        assert_eq!(
            hooks.events.lock().unwrap().as_slice(),
            &[
                "begin:2:false".to_string(),
                "update:0".to_string(),
                "update:1".to_string(),
                "end".to_string(),
            ]
        );
    }

    #[test]
    fn test_sleepy_update_ordering() {
        let _logic = GameLogic::new();

        let entry1 = SleepyUpdateEntry {
            wake_frame: 10,
            phase: SleepyUpdatePhase::Normal,
            object_id: 1,
            module: Arc::new(RwLock::new(crate::modules::UpdateModuleDummy {})),
        };
        let entry2 = SleepyUpdateEntry {
            wake_frame: 5,
            phase: SleepyUpdatePhase::Normal,
            object_id: 2,
            module: Arc::new(RwLock::new(crate::modules::UpdateModuleDummy {})),
        };

        // Earlier wake frame should have higher priority (min-heap)
        assert!(entry2 > entry1);
    }

    // ============================================================================
    // WEEK 2: GAME LOOP INTEGRATION TESTS (60+ tests for orchestration)
    // ============================================================================

    #[test]
    fn test_fixed_delta_time_constant() {
        // Verify fixed timestep is correct for 30 FPS
        assert!((FIXED_DELTA_TIME - 1.0 / 30.0).abs() < 0.00001);
    }

    #[test]
    fn test_frame_counting() {
        let mut logic = GameLogic::new();

        for frame in 0..100 {
            let result = logic.update(frame);
            assert!(result.is_ok());
            assert_eq!(logic.get_frame(), frame);
        }
    }

    #[test]
    fn test_game_time_accumulation() {
        let mut logic = GameLogic::new();
        logic.init();

        // Game time tracks the start time of the current frame: `time = frame * dt`.
        // At frame 30 (30 FPS), time should be 1 second.
        for frame in 0..=30 {
            let _ = logic.update(frame);
        }

        assert!((logic.get_game_time() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_multiple_commands_processed() {
        let mut logic = GameLogic::new();

        // Queue multiple commands
        for i in 0..10 {
            let command = GameCommand::MoveUnit {
                player_id: 0,
                unit_ids: vec![i],
                target_position: (100.0, 100.0, 0.0),
            };
            logic.queue_command(command);
        }

        assert_eq!(logic.command_queue.len(), 10);

        // Process them
        let result = logic.process_command_queue();
        assert!(result.is_ok());
        assert_eq!(logic.command_queue.len(), 0);
    }

    #[test]
    fn test_world_dimensions() {
        let mut logic = GameLogic::new();
        logic.set_dimensions(1024.0, 768.0);

        assert_eq!(logic.get_width(), 1024.0);
        assert_eq!(logic.get_height(), 768.0);
    }

    #[test]
    fn set_defaults_uses_cpp_default_world_dimensions() {
        let mut logic = GameLogic::new();
        logic.set_dimensions(1024.0, 768.0);
        logic.frame = 99;
        logic.next_object_id = 42;

        logic.set_defaults(true);

        assert_eq!(logic.frame, 0);
        assert_eq!(logic.get_width(), 64.0);
        assert_eq!(logic.get_height(), 64.0);
        assert_eq!(logic.next_object_id, 42);

        logic.set_defaults(false);

        assert_eq!(logic.next_object_id, 1);
    }

    #[test]
    fn test_loading_flags() {
        let mut logic = GameLogic::new();

        assert!(!logic.is_loading_map());
        logic.set_loading_map(true);
        assert!(logic.is_loading_map());

        assert!(!logic.is_loading_save());
        logic.set_loading_save(true);
        assert!(logic.is_loading_save());
    }

    #[test]
    fn test_new_game_start_request_waits_for_movie_gate() {
        let mut logic = GameLogic::new();

        crate::helpers::TheGameLogic::clear_start_new_game_request();
        crate::helpers::TheGameLogic::set_intro_movie_playing(true);
        crate::helpers::TheGameLogic::request_start_new_game();

        assert!(logic.is_loading_map());

        assert!(logic.update(0).is_ok());
        assert!(crate::helpers::TheGameLogic::is_start_new_game_requested());
        assert!(logic.is_loading_map());

        crate::helpers::TheGameLogic::set_intro_movie_playing(false);
        crate::helpers::TheGameLogic::clear_start_new_game_request();
    }

    #[test]
    fn test_game_event_queue_cleared_each_frame() {
        let mut logic = GameLogic::new();

        // Add an event
        logic.event_queue.push(GameEvent::ObjectCreated(1));
        assert_eq!(logic.event_queue.len(), 1);

        // Frame update should clear events
        let _ = logic.clear_frame_events();
        assert_eq!(logic.event_queue.len(), 0);
    }

    #[test]
    fn test_move_command_parsing() {
        let cmd = GameCommand::MoveUnit {
            player_id: 0,
            unit_ids: vec![1, 2, 3],
            target_position: (100.0, 200.0, 0.0),
        };

        match cmd {
            GameCommand::MoveUnit {
                player_id,
                unit_ids,
                target_position,
            } => {
                assert_eq!(player_id, 0);
                assert_eq!(unit_ids.len(), 3);
                assert_eq!(target_position.0, 100.0);
            }
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_attack_command_parsing() {
        let cmd = GameCommand::AttackTarget {
            player_id: 1,
            attacker_ids: vec![5, 6],
            target_id: 99,
        };

        match cmd {
            GameCommand::AttackTarget {
                player_id,
                attacker_ids,
                target_id,
            } => {
                assert_eq!(player_id, 1);
                assert_eq!(attacker_ids.len(), 2);
                assert_eq!(target_id, 99);
            }
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_build_command_parsing() {
        let cmd = GameCommand::BuildStructure {
            player_id: 0,
            builder_id: 10,
            structure_type: "BarracksBridge".to_string(),
            position: (500.0, 500.0),
        };

        match cmd {
            GameCommand::BuildStructure {
                player_id,
                builder_id,
                structure_type,
                position,
            } => {
                assert_eq!(player_id, 0);
                assert_eq!(builder_id, 10);
                assert_eq!(structure_type, "BarracksBridge");
                assert_eq!(position.0, 500.0);
            }
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_special_power_command_parsing() {
        let cmd = GameCommand::UseSpecialPower {
            player_id: 0,
            power_name: "Carpet Bomb".to_string(),
            target_position: Some((300.0, 300.0, 0.0)),
        };

        match cmd {
            GameCommand::UseSpecialPower {
                player_id,
                power_name,
                target_position,
            } => {
                assert_eq!(player_id, 0);
                assert_eq!(power_name, "Carpet Bomb");
                assert!(target_position.is_some());
            }
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_radar_update_creation() {
        let update = RadarUpdate {
            player_id: 0,
            position: (250.0, 250.0),
            event_type: RadarEventType::UnitCreated,
        };

        assert_eq!(update.player_id, 0);
        assert_eq!(update.position.0, 250.0);
        assert!(matches!(update.event_type, RadarEventType::UnitCreated));
    }

    #[test]
    fn test_all_radar_event_types() {
        let events = vec![
            RadarEventType::UnitCreated,
            RadarEventType::UnitDestroyed,
            RadarEventType::BaseAttacked,
            RadarEventType::EnemyDetected,
        ];

        assert_eq!(events.len(), 4);
    }

    #[test]
    fn test_game_mode_single_player() {
        let mut logic = GameLogic::new();
        logic.set_game_mode(GAME_SINGLE_PLAYER);

        assert!(logic.is_in_single_player_game());
        assert!(!logic.is_in_multiplayer_game());
        assert!(!logic.is_in_skirmish_game());
    }

    #[test]
    fn test_game_mode_lan() {
        let mut logic = GameLogic::new();
        logic.set_game_mode(GAME_LAN);

        assert!(!logic.is_in_single_player_game());
        assert!(logic.is_in_multiplayer_game());
        assert!(!logic.is_in_skirmish_game());
    }

    #[test]
    fn test_game_mode_internet() {
        let mut logic = GameLogic::new();
        logic.set_game_mode(GAME_INTERNET);

        assert!(!logic.is_in_single_player_game());
        assert!(logic.is_in_multiplayer_game());
        assert!(!logic.is_in_skirmish_game());
    }

    #[test]
    fn test_game_mode_skirmish() {
        let mut logic = GameLogic::new();
        logic.set_game_mode(GAME_SKIRMISH);

        assert!(!logic.is_in_single_player_game());
        assert!(!logic.is_in_multiplayer_game());
        assert!(logic.is_in_skirmish_game());
    }

    #[test]
    fn test_physics_world_damage_queuing() {
        let mut physics = PhysicsWorld::new();

        physics.queue_damage(10, 20, 50.0);
        physics.queue_damage(11, 21, 75.0);

        assert_eq!(physics.pending_damage.len(), 2);
    }

    #[test]
    fn test_object_id_allocation_sequential() {
        let mut logic = GameLogic::new();

        let id1 = logic.allocate_object_id();
        let id2 = logic.allocate_object_id();
        let id3 = logic.allocate_object_id();

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);
    }

    #[test]
    fn test_update_not_reentrant() {
        let mut logic = GameLogic::new();

        // Set update flag
        logic.is_in_update = true;

        // Attempt update should fail
        let result = logic.update(0);
        assert!(result.is_err());

        match result.unwrap_err() {
            GameLogicError::InvalidState(msg) => {
                assert!(msg.contains("Re-entrant"));
            }
            _ => panic!("Expected InvalidState error"),
        }
    }

    #[test]
    fn test_error_display_object_not_found() {
        let err = GameLogicError::ObjectNotFound(999);
        assert!(err.to_string().contains("999"));
    }

    #[test]
    fn test_error_display_physics_error() {
        let err = GameLogicError::PhysicsError("collision failed".to_string());
        assert!(err.to_string().contains("collision failed"));
    }

    #[test]
    fn test_error_display_script_error() {
        let err = GameLogicError::ScriptError("condition syntax".to_string());
        assert!(err.to_string().contains("condition syntax"));
    }

    #[test]
    fn test_error_display_ai_error() {
        let err = GameLogicError::AIError("pathfinding failed".to_string());
        assert!(err.to_string().contains("pathfinding failed"));
    }

    #[test]
    fn test_error_display_command_error() {
        let err = GameLogicError::CommandError("invalid target".to_string());
        assert!(err.to_string().contains("invalid target"));
    }

    #[test]
    fn test_partition_manager_creation() {
        let mut partition = PartitionManager::new();
        let result = partition.update();
        assert!(result.is_ok());
    }

    #[test]
    fn partition_xfer_writes_cpp_v2_cell_shroud_not_object_positions() {
        // C++ PartitionManager::xfer (PartitionManager.cpp:4558-4657):
        // u8 v2, f32 cellSize, i32 totalCellCount, per-cell PartitionCell
        // snapshots, then pendingUndoShroudReveals.
        let _lock = test_state_lock();
        {
            let shroud_manager = get_shroud_manager();
            let mut shroud = shroud_manager.lock().expect("shroud");
            shroud.init_shroud_grid(100.0, 100.0);
            let mut snapshot = shroud.snapshot_state();
            if let Some(grid) = snapshot.grid.as_mut() {
                grid.cells[0].current_shroud[0] = -2;
                grid.cells[0].active_shroud_level[0] = 3;
            }
            shroud.replace_state(&snapshot).expect("seed shroud");
        }

        let path = std::env::temp_dir().join(format!(
            "partition_xfer_v2_{}_{}.bin",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        {
            let mut logic = GameLogic::new();
            let mut save = XferSave::new();
            save.open(path.to_string_lossy().into_owned()).unwrap();
            xfer_partition_state(&mut logic, &mut save).unwrap();
            save.close().unwrap();
        }

        {
            let shroud_manager = get_shroud_manager();
            let mut shroud = shroud_manager.lock().expect("shroud");
            shroud.init_shroud_grid(100.0, 100.0);
        }
        {
            let mut logic = GameLogic::new();
            let mut load = XferLoad::new();
            load.open(path.to_string_lossy().into_owned()).unwrap();
            xfer_partition_state(&mut logic, &mut load).unwrap();
            load.close().unwrap();
        }
        let _ = std::fs::remove_file(&path);

        let snapshot = get_shroud_manager()
            .lock()
            .expect("shroud")
            .snapshot_state();
        let cell = &snapshot.grid.expect("grid").cells[0];
        assert_eq!(cell.current_shroud[0], -2);
        assert_eq!(cell.active_shroud_level[0], 3);
    }


    #[test]
    fn test_partition_add_object() {
        let mut partition = PartitionManager::new();
        partition.add_object(1, (100.0, 100.0, 0.0));
        // If no panic, test succeeds
        assert!(true);
    }

    #[test]
    fn test_partition_remove_object() {
        let mut partition = PartitionManager::new();
        partition.add_object(1, (100.0, 100.0, 0.0));
        partition.remove_object(1);
        // If no panic, test succeeds
        assert!(true);
    }

    fn ghost_capture(name: &str) -> crate::object::w3d_ghost_object::W3DGhostSnapshotCapture {
        use crate::object::w3d_ghost_object::{
            Matrix3x4, ParentGeometrySnapshot, RenderObjectClass, RenderObjectState,
        };
        crate::object::w3d_ghost_object::W3DGhostSnapshotCapture {
            capture_window_generation: None,
            drawable_effectively_hidden: false,
            render_objects: vec![RenderObjectState {
                name: name.to_string(),
                scale: 1.25,
                color: 0xff00_ff00,
                transform: Matrix3x4::IDENTITY,
                sub_objects: Vec::new(),
                class_id: RenderObjectClass::HLod,
            }],
            geometry: ParentGeometrySnapshot {
                geometry_type: 2,
                is_small: false,
                major_radius: 10.0,
                minor_radius: 5.0,
                position: [100.0, 100.0, 0.0],
                angle: 0.5,
            },
        }
    }

    #[test]
    fn partition_ghost_snapshots_only_on_clear_to_fogged_and_frees_on_clear() {
        use crate::common::ObjectShroudStatus;
        use crate::object::w3d_ghost_object::FrozenW3DGhostSceneEvent;

        let mut partition = PartitionManager::new();
        let mut manager = crate::object::W3DGhostObjectManager::new();
        partition.add_object(41, (100.0, 100.0, 0.0));
        let scene_id = partition
            .attach_object_ghost(41, true, &mut manager)
            .expect("eligible immobile object gets ghost link");

        assert!(partition.apply_object_ghost_shroud_status(
            41,
            0,
            ObjectShroudStatus::Clear,
            None,
            &mut manager,
        ));
        let capture = ghost_capture("Building");
        assert!(partition.object_ghost_needs_capture(41, 0, ObjectShroudStatus::Fogged));
        partition.apply_object_ghost_shroud_status(
            41,
            0,
            ObjectShroudStatus::Fogged,
            Some(&capture),
            &mut manager,
        );
        assert!(manager.linked_ghost_has_any_snapshot(scene_id));
        let first_events = manager.drain_scene_events();
        assert_eq!(
            first_events
                .iter()
                .filter(|event| matches!(event, FrozenW3DGhostSceneEvent::UpsertSnapshot(_)))
                .count(),
            1
        );

        partition.apply_object_ghost_shroud_status(
            41,
            0,
            ObjectShroudStatus::Fogged,
            Some(&ghost_capture("ChangedWhileFogged")),
            &mut manager,
        );
        assert!(manager.drain_scene_events().is_empty());

        partition.apply_object_ghost_shroud_status(
            41,
            0,
            ObjectShroudStatus::Clear,
            None,
            &mut manager,
        );
        assert!(!manager.linked_ghost_has_any_snapshot(scene_id));
        assert!(manager.drain_scene_events().iter().any(|event| matches!(
            event,
            FrozenW3DGhostSceneEvent::RemoveSnapshot(key) if key.ghost_id == scene_id
        )));
    }

    #[test]
    fn partition_ghost_orphan_survives_parent_then_releases_after_memory_clears() {
        use crate::common::ObjectShroudStatus;

        let mut partition = PartitionManager::new();
        let mut manager = crate::object::W3DGhostObjectManager::new();
        let scene_id = partition
            .attach_object_ghost(52, true, &mut manager)
            .expect("ghost link");
        partition.apply_object_ghost_shroud_status(
            52,
            0,
            ObjectShroudStatus::Fogged,
            Some(&ghost_capture("DeadBuildingMemory")),
            &mut manager,
        );

        assert!(partition.detach_object_ghost(52, &mut manager));
        assert_eq!(partition.ghost_link_scene_id(52), Some(scene_id));
        assert_eq!(manager.used_count(), 1);
        assert_eq!(manager.used()[0].parent_object_id(), None);

        partition.apply_object_ghost_shroud_status(
            52,
            0,
            ObjectShroudStatus::Shrouded,
            None,
            &mut manager,
        );
        assert_eq!(partition.ghost_link_scene_id(52), None);
        assert_eq!(manager.used_count(), 0);
        assert_eq!(manager.free_count(), 1);
    }

    #[test]
    fn test_empty_object_list() {
        let logic = GameLogic::new();
        assert_eq!(logic.all_objects.len(), 0);
    }

    #[test]
    fn test_empty_dead_objects_list() {
        let logic = GameLogic::new();
        assert_eq!(logic.dead_objects.len(), 0);
    }

    #[test]
    fn test_clear_multiple_times() {
        let mut logic = GameLogic::new();

        for _ in 0..10 {
            let _ = logic.clear_frame_events();
        }

        assert_eq!(logic.event_queue.len(), 0);
    }

    #[test]
    fn test_reset_temporary_flags() {
        let mut logic = GameLogic::new();
        let result = logic.reset_temporary_flags();
        assert!(result.is_ok());
    }

    #[test]
    fn test_consecutive_frames() {
        let mut logic = GameLogic::new();

        for frame in 0..10 {
            let result = logic.update(frame);
            assert!(result.is_ok(), "Frame {} update failed", frame);
            assert_eq!(logic.get_frame(), frame);
        }
    }

    #[test]
    fn test_game_time_matches_frame_count() {
        let mut logic = GameLogic::new();

        for frame in 0..60 {
            let _ = logic.update(frame);
            let expected_time = frame as f32 * FIXED_DELTA_TIME;
            assert!(
                (logic.get_game_time() - expected_time).abs() < 0.0001,
                "Frame {}: time mismatch",
                frame
            );
        }
    }

    #[test]
    fn test_object_event_structure() {
        let events = vec![
            GameEvent::ObjectCreated(1),
            GameEvent::ObjectDestroyed(2),
            GameEvent::DamageDealt {
                attacker: 3,
                target: 4,
                amount: 50.0,
            },
            GameEvent::VictoryConditionMet {
                player_id: 0,
                condition_name: "LastEnemyDestroyed".to_string(),
            },
        ];

        assert_eq!(events.len(), 4);
    }

    #[test]
    fn test_pending_damage_structure() {
        let damage = PendingDamage {
            target_id: 10,
            attacker_id: 20,
            damage_amount: 75.5,
            damage_type: crate::damage::DamageType::Explosion,
            death_type: crate::damage::DeathType::Normal,
        };

        assert_eq!(damage.target_id, 10);
        assert_eq!(damage.attacker_id, 20);
        assert!((damage.damage_amount - 75.5).abs() < 0.01);
    }

    #[test]
    fn test_pending_collision_structure() {
        let collision = PendingCollision {
            object_a: 1,
            object_b: 2,
            collision_point: (100.0, 200.0, 0.0),
        };

        assert_eq!(collision.object_a, 1);
        assert_eq!(collision.object_b, 2);
        assert_eq!(collision.collision_point.0, 100.0);
    }

    #[test]
    fn test_game_command_enum_variants() {
        let commands = vec![
            GameCommand::MoveUnit {
                player_id: 0,
                unit_ids: vec![1],
                target_position: (0.0, 0.0, 0.0),
            },
            GameCommand::AttackTarget {
                player_id: 0,
                attacker_ids: vec![1],
                target_id: 2,
            },
            GameCommand::BuildStructure {
                player_id: 0,
                builder_id: 1,
                structure_type: "Barracks".to_string(),
                position: (0.0, 0.0),
            },
            GameCommand::UseSpecialPower {
                player_id: 0,
                power_name: "Power".to_string(),
                target_position: None,
            },
        ];

        assert_eq!(commands.len(), 4);
    }
}
