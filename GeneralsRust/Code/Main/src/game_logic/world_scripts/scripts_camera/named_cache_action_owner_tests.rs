//! Actual dispatcher transport: no standalone named tracker before Main.
use super::*;
use gamelogic::scripting::core::{Parameter, ParameterType, ScriptAction, ScriptActionType};

fn world() -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.add_player(Player::new(1, Team::USA, "ActionOwner", true));
    let mut template = ThingTemplate::new("NamedActionWarehouse");
    template.add_kind_of(KindOf::Structure).set_health(100.0);
    template.dock_kind = crate::game_logic::DockKind::SupplyWarehouse;
    template
        .special_power_modules
        .push(crate::game_logic::SpecialPowerModuleMetadata {
            source_index: 0,
            module_tag: Some("OwnSpySatellite".into()),
            module_kind: crate::game_logic::SpecialPowerModuleKind::SpecialAbility,
            special_power_template: "SpecialPowerSpySatellite".into(),
            special_power_template_id: 17,
            command_power: Some(crate::command_system::SpecialPowerType::SpySatellite),
            reload_time_frames: 0,
            required_science: None,
            public_timer: false,
            shared_n_sync: false,
            shortcut_power: false,
            update_module_starts_attack: false,
            starts_paused: false,
            scripted_special_power_only: false,
        });
    world.templates.insert(template.name.clone(), template);
    let id = world
        .create_object_for_player("NamedActionWarehouse", 1, Vec3::ZERO)
        .unwrap();
    world.set_host_object_name(id, "SourceName");
    world.initialize_receiving_named_cache_for_fixture();
    // Cache pointers survive ordinary Object::setName; actions must use the
    // original cached key rather than scan the actor's current name.
    world.set_host_object_name(id, "FieldOnlyName");
    (world, id)
}

fn action(kind: ScriptActionType, strings: &[&str], number: Option<i32>) -> ScriptAction {
    let mut action = ScriptAction::new(kind);
    for text in strings {
        action
            .add_parameter(Parameter::with_string(ParameterType::Unit, (*text).into()))
            .unwrap();
    }
    if let Some(value) = number {
        action
            .add_parameter(Parameter::with_int(ParameterType::Int, value))
            .unwrap();
    }
    action
}

#[test]
fn actual_named_actions_use_cached_pointer_immediately_and_do_not_touch_peer_tracker() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "actual_named_actions_use_cached_pointer_immediately_and_do_not_touch_peer_tracker",
        || {
            let (mut owner, id) = world();
            let (peer, peer_id) = world();
            assert_eq!(id, peer_id);
            let tracker = gamelogic::scripting::engine::get_named_object_tracker();
            tracker
                .register_named_object("SourceName".into(), 701)
                .unwrap();
            let foreign = std::sync::Arc::new(std::sync::RwLock::new(
                gamelogic::object::Object::new_for_xfer_load(701, 100.0),
            ));
            gamelogic::object::registry::OBJECT_REGISTRY.register_object(701, &foreign);
            owner
                .with_owned_script_engine(|engine, receiving| {
                    assert!(engine.set_priority_default("ExactSet", 37));
                    let mut driver = HostScriptExecutionDriver::with_interpreter(receiving, engine);
                    let actions = [
                        action(
                            ScriptActionType::NamedApplyAttackPrioritySet,
                            &["SourceName", "ExactSet"],
                            None,
                        ),
                        action(
                            ScriptActionType::NamedSetEvacLeftOrRight,
                            &["SourceName"],
                            Some(1),
                        ),
                        action(
                            ScriptActionType::UnitAffectObjectPanelFlags,
                            &["SourceName", "indestructible"],
                            Some(1),
                        ),
                        action(
                            ScriptActionType::WarehouseSetValue,
                            &["SourceName"],
                            Some(600),
                        ),
                        action(
                            ScriptActionType::NamedSetSpecialPowerCountdown,
                            &["SourceName", "SpecialPowerSpySatellite"],
                            Some(7),
                        ),
                        action(
                            ScriptActionType::NamedAddSpecialPowerCountdown,
                            &["SourceName", "SpecialPowerSpySatellite"],
                            Some(3),
                        ),
                    ];
                    for action in &actions {
                        engine.friend_execute_action_with_driver(
                            action,
                            None,
                            gamelogic::scripting::executor::ScriptContext::new(),
                            &mut driver,
                        );
                    }
                    let object = driver.world.host_object(id).unwrap();
                    assert_eq!(object.attack_priority_set.as_deref(), Some("ExactSet"));
                    assert_eq!(object.garrison_evac_disposition(), 1);
                    assert!(object.is_indestructible());
                    assert_eq!(object.stored_resources.supplies, 600);
                    assert_eq!(
                        object.special_power_countdown_seconds(
                            &crate::command_system::SpecialPowerType::SpySatellite
                        ),
                        10.0
                    );
                    assert_eq!(
                        engine.get_object_attack_priority_set(id.0).as_deref(),
                        Some("ExactSet")
                    );
                    assert_eq!(tracker.get_object_id("SourceName").unwrap(), Some(701));
                })
                .unwrap();
            let peer = peer.host_object(peer_id).unwrap();
            assert_eq!(peer.attack_priority_set, None);
            assert!(!peer.is_indestructible());
            assert_eq!(peer.garrison_evac_disposition(), 3);
            assert_eq!(
                peer.special_power_countdown_seconds(
                    &crate::command_system::SpecialPowerType::SpySatellite
                ),
                0.0
            );
            assert_eq!(tracker.get_object_id("SourceName").unwrap(), Some(701));
            assert!(
                !foreign
                    .read()
                    .unwrap()
                    .get_body_module()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .is_indestructible()
            );
            gamelogic::object::registry::OBJECT_REGISTRY.unregister_object(701);
        },
    );
}

#[test]
fn actual_named_spawn_checks_cached_live_pointer_then_reuses_original_tombstone() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "actual_named_spawn_checks_cached_live_pointer_then_reuses_original_tombstone",
        || {
            let (mut owner, old) = world();
            let team = owner.host_object(old).unwrap().team_instance_name.clone();
            let mut spawn = action(
                ScriptActionType::UnitSpawnNamedLocationOrientation,
                &["SourceName", "NamedActionWarehouse", &team],
                None,
            );
            spawn
                .add_parameter(Parameter::with_coord(
                    ParameterType::Coord3D,
                    gamelogic::common::Coord3D::new(30.0, 40.0, 0.0),
                ))
                .unwrap();
            spawn
                .add_parameter(Parameter::with_real(ParameterType::Real, 0.5))
                .unwrap();
            owner
                .with_owned_script_engine(|engine, receiving| {
                    let mut driver = HostScriptExecutionDriver::with_interpreter(receiving, engine);
                    engine.friend_execute_action_with_driver(
                        &spawn,
                        None,
                        gamelogic::scripting::executor::ScriptContext::new(),
                        &mut driver,
                    );
                    assert_eq!(
                        driver.world.host_object_id_by_script_name("SourceName"),
                        Some(old)
                    );
                    assert_eq!(driver.world.host_objects().registered().count(), 1);
                })
                .unwrap();
            owner.destroy_object(old);
            owner.process_destroy_list();
            assert!(owner.named_script_cache.did_exist("SourceName"));
            owner
                .with_owned_script_engine(|engine, receiving| {
                    let mut driver = HostScriptExecutionDriver::with_interpreter(receiving, engine);
                    engine.friend_execute_action_with_driver(
                        &spawn,
                        None,
                        gamelogic::scripting::executor::ScriptContext::new(),
                        &mut driver,
                    );
                    let replacement = driver
                        .world
                        .host_object_id_by_script_name("SourceName")
                        .unwrap();
                    assert_ne!(replacement, old);
                    assert_eq!(
                        driver.world.host_object(replacement).unwrap().name,
                        "SourceName"
                    );
                    assert!(!driver.world.named_script_cache.did_exist("SourceName"));
                    assert_eq!(
                        driver.world.named_script_cache.wire(),
                        [("SourceName".into(), replacement.0)]
                    );
                })
                .unwrap();
        },
    );
}

#[test]
fn actual_named_spawn_preserves_calling_same_name_team_and_missing_this_is_not_created() {
    let (mut world, _) = world();
    let (older, newer) = {
        let mut factory = world.team_factory.lock().unwrap();
        factory
            .init_team("SpawnSiblings".into(), "".into(), false, None)
            .unwrap();
        let older = factory.create_inactive_team("SpawnSiblings").unwrap();
        let newer = factory.create_inactive_team("SpawnSiblings").unwrap();
        let mut older = older.write().unwrap();
        older.set_controlling_player_id(Some(1));
        let mut newer = newer.write().unwrap();
        newer.set_controlling_player_id(Some(1));
        (older.get_id(), newer.get_id())
    };
    let spawn = |name: &str, team: &str| {
        let mut spawn = action(
            ScriptActionType::UnitSpawnNamedLocationOrientation,
            &[name, "NamedActionWarehouse", team],
            None,
        );
        spawn
            .add_parameter(Parameter::with_coord(
                ParameterType::Coord3D,
                gamelogic::common::Coord3D::new(30.0, 40.0, 0.0),
            ))
            .unwrap();
        spawn
            .add_parameter(Parameter::with_real(ParameterType::Real, 0.0))
            .unwrap();
        spawn
    };
    world
        .with_owned_script_engine(|engine, receiving| {
            let mut driver = HostScriptExecutionDriver::with_interpreter(receiving, engine);
            engine.friend_execute_action_with_driver(
                &spawn("CallingSpawn", "SpawnSiblings"),
                Some(older),
                gamelogic::scripting::executor::ScriptContext::new(),
                &mut driver,
            );
            let id = driver
                .world
                .host_object_id_by_script_name("CallingSpawn")
                .unwrap();
            assert_eq!(
                driver.world.host_object(id).unwrap().team_instance_id,
                Some(older)
            );
            assert_ne!(older, newer);
            let count = driver.world.host_objects().registered().count();
            engine.friend_execute_action_with_driver(
                &spawn("MissingThisSpawn", gamelogic::scripting::core::THIS_TEAM),
                Some(u32::MAX),
                gamelogic::scripting::executor::ScriptContext::new(),
                &mut driver,
            );
            assert_eq!(driver.world.host_objects().registered().count(), count);
            assert!(
                driver
                    .world
                    .host_object_id_by_script_name("MissingThisSpawn")
                    .is_none()
            );
        })
        .unwrap();
}
