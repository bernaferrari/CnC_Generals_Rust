//! Actual dispatcher transport: no standalone named tracker before Main.
use super::*;
use generals_sim::scripting::program::{Parameter, ParameterType, ScriptAction, ScriptActionType};

fn world() -> (GameLogic, ObjectId) {
    world_with_energy(None)
}

fn world_with_energy(energy: Option<i32>) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.add_player(Player::new(1, Team::USA, "ActionOwner", true));
    let mut template = ThingTemplate::new("NamedActionWarehouse");
    template.add_kind_of(KindOf::Structure).set_health(100.0);
    template.energy_production = energy;
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
            cash_bounty: None,
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
fn team_panel_flag_updates_energy_before_the_next_named_action() {
    let (mut owner, id) = world_with_energy(Some(20));
    let (peer, peer_id) = world_with_energy(Some(20));
    assert_eq!(id, peer_id);
    let team = owner.host_default_team_id(1).unwrap();

    let mut disable = ScriptAction::new(ScriptActionType::TeamAffectObjectPanelFlags);
    disable
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            generals_sim::scripting::program::THIS_TEAM.into(),
        ))
        .unwrap();
    disable
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "Powered".into(),
        ))
        .unwrap();
    disable
        .add_parameter(Parameter::with_int(ParameterType::Int, 0))
        .unwrap();
    disable.next_action = Some(Box::new(action(
        ScriptActionType::UnitAffectObjectPanelFlags,
        &["SourceName", "Powered"],
        Some(1),
    )));
    let mut engine = gamelogic::scripting::engine::ScriptEngine::new_in_published_world().unwrap();
    engine.set_external_eval_context(None, Some(team));
    let mut observations = Vec::new();
    {
        let mut observer = |world: &GameLogic| {
            observations.push((
                world.players[&1].power_produced,
                world.objects[&id].is_script_underpowered(),
            ));
        };
        let frame = owner.frame;
        let mut driver = HostScriptExecutionDriver::with_interpreter(&mut owner, &engine)
            .with_after_action_observer(&mut observer);
        engine.friend_execute_action_with_driver(
            &disable,
            None,
            gamelogic::scripting::executor::ScriptContext::at_frame(frame),
            &mut driver,
        );
    }
    assert_eq!(observations, [(0, true), (20, false)]);
    assert_eq!(peer.players[&1].power_produced, 20);
    assert!(!peer.objects[&id].is_script_underpowered());
}

#[test]
fn panel_flags_change_ai_recruitment_before_the_next_action_in_the_receiving_match() {
    use generals_sim::scripting::program::THIS_TEAM;

    let (mut receiving, id, other) = super::named_command_test_support::world();
    let (peer, peer_id, _) = super::named_command_test_support::world();
    assert_eq!(id, peer_id);
    assert!(receiving.host_object(id).unwrap().is_recruitable);
    let team = receiving.host_default_team_id(1).unwrap();

    let mut disable = action(
        ScriptActionType::TeamAffectObjectPanelFlags,
        &[THIS_TEAM, "AI Recruitable"],
        Some(0),
    );
    disable.next_action = Some(Box::new(action(
        ScriptActionType::UnitAffectObjectPanelFlags,
        &["NamedUnit", "AI Recruitable"],
        Some(1),
    )));
    let mut engine = gamelogic::scripting::engine::ScriptEngine::new_in_published_world().unwrap();
    engine.set_external_eval_context(None, Some(team));
    let mut observed = Vec::new();
    {
        let mut observer = |world: &GameLogic| {
            observed.push(world.host_object(id).unwrap().is_recruitable);
        };
        let mut driver = HostScriptExecutionDriver::with_interpreter(&mut receiving, &engine)
            .with_after_action_observer(&mut observer);
        engine.friend_execute_action_with_driver_checked(
            &disable,
            None,
            gamelogic::scripting::executor::ScriptContext::at_frame(0),
            &mut driver,
        ).unwrap();
    }
    assert_eq!(observed, [false, true], "each authored recruitment flag must take effect during its action");
    assert!(receiving.host_object(other).unwrap().is_recruitable);
    assert!(peer.host_object(peer_id).unwrap().is_recruitable);
}

#[test]
fn panel_flags_change_pickability_and_deselect_in_the_receiving_match() {
    use crate::presentation_frame::PresentationFrame;
    use crate::unit_control::UnitControlSystem;
    use generals_sim::scripting::program::THIS_TEAM;

    let (mut receiving, id) = world();
    let (mut peer, peer_id) = world();
    assert_eq!(id, peer_id);
    assert!(receiving.select_local_player(Some(1)));
    let team = receiving.host_default_team_id(1).unwrap();
    let mut engine = gamelogic::scripting::engine::ScriptEngine::new_in_published_world().unwrap();
    engine.set_external_eval_context(None, Some(team));
    let execute = |world: &mut GameLogic, kind, unit, value, frame| {
        world.frame = frame;
        let mut driver = HostScriptExecutionDriver::with_interpreter(world, &engine);
        engine
            .friend_execute_action_with_driver_checked(
                &action(kind, &[unit, "Selectable"], Some(value)),
                None,
                gamelogic::scripting::executor::ScriptContext::at_frame(frame),
                &mut driver,
            )
            .unwrap();
    };
    let assert_pickability = |world: &mut GameLogic, expected| {
        assert_eq!(
            world.host_object(id).unwrap().is_selectable(),
            expected,
            "an authored panel flag must change live object selectability"
        );
        let frame = PresentationFrame::try_build_from_logic(world, 1).unwrap();
        let drawable = frame.objects.iter().find(|object| object.id == id).unwrap();
        assert_eq!(
            UnitControlSystem::presentation_is_selectable(drawable),
            expected
        );
        assert_eq!(
            PresentationFrame::presentation_is_squad_live(drawable),
            expected
        );
        let pick = world.translator_pick_object_view();
        assert_eq!(pick.get(id.0).unwrap().selectable, expected);
    };

    assert_pickability(&mut receiving, false);
    execute(
        &mut receiving,
        ScriptActionType::UnitAffectObjectPanelFlags,
        "SourceName",
        1,
        17,
    );
    assert_pickability(&mut receiving, true);
    assert_pickability(&mut peer, false);
    receiving.select_objects_no_sound(1, vec![id]);
    assert_eq!(receiving.selected_objects, [id]);
    assert_eq!(receiving.players[&1].selected_objects, [id]);

    execute(
        &mut receiving,
        ScriptActionType::TeamAffectObjectPanelFlags,
        THIS_TEAM,
        0,
        18,
    );
    assert_pickability(&mut receiving, false);
    assert_pickability(&mut peer, false);
    let object = receiving.host_object(id).unwrap();
    assert!(!object.selected);
    assert!(!object.status.selected);
    assert!(!object.status.unselectable);
    assert!(!object.is_kind_of(KindOf::Selectable));
    assert!(receiving.selected_objects.is_empty());
    assert!(receiving.players[&1].selected_objects.is_empty());
    assert_eq!(receiving.selection_changed_frame, 18);
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
                    assert_eq!(object.warehouse_boxes(), 6);
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
                    glam::Vec3::new(30.0, 40.0, 0.0),
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
            owner.destroy_object(old).unwrap();
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
                glam::Vec3::new(30.0, 40.0, 0.0),
            ))
            .unwrap();
        spawn
            .add_parameter(Parameter::with_real(ParameterType::Real, 0.0))
            .unwrap();
        spawn.next_action = Some(Box::new(action(
            ScriptActionType::UnitAffectObjectPanelFlags,
            &[name, "indestructible"],
            Some(1),
        )));
        spawn
    };
    let engine = gamelogic::scripting::engine::ScriptEngine::new_in_published_world().unwrap();
    let mut driver = HostScriptExecutionDriver::new(&mut world);
    engine
        .friend_execute_action_with_driver_checked(
            &spawn("CallingSpawn", "SpawnSiblings"),
            Some(older),
            gamelogic::scripting::executor::ScriptContext::new(),
            &mut driver,
        )
        .unwrap();
    let id = driver
        .world
        .host_object_id_by_script_name("CallingSpawn")
        .unwrap();
    assert_eq!(
        driver.world.host_object(id).unwrap().team_instance_id,
        Some(older)
    );
    assert!(driver.world.host_object(id).unwrap().is_indestructible());
    assert_ne!(older, newer);
    let count = driver.world.host_objects().registered().count();
    engine
        .friend_execute_action_with_driver_checked(
            &spawn(
                "MissingThisSpawn",
                generals_sim::scripting::program::THIS_TEAM,
            ),
            Some(u32::MAX),
            gamelogic::scripting::executor::ScriptContext::new(),
            &mut driver,
        )
        .unwrap();
    assert_eq!(driver.world.host_objects().registered().count(), count);
    assert!(
        driver
            .world
            .host_object_id_by_script_name("MissingThisSpawn")
            .is_none()
    );
}

#[test]
fn reinforcement_uses_the_authored_prototype_name_in_the_receiving_world() {
    use gamelogic::team::{CreateUnitsInfo, TeamPrototype};
    use generals_sim::scripting::program::THIS_TEAM;

    let (mut owner, existing) = world();
    let (peer, peer_existing) = world();
    assert_eq!(existing, peer_existing);
    let prototype = "ReinforcementOwner";
    let calling = {
        let mut factory = owner.team_factory.lock().unwrap();
        let mut definition = TeamPrototype::new(prototype.into());
        definition.set_units_info(
            0,
            CreateUnitsInfo {
                unit_thing_name: "NamedActionWarehouse".into(),
                min_units: 1,
                max_units: 1,
            },
        );
        factory.replace_team_prototype(definition);
        assert!(factory.restore_prototype_controller(prototype, Some(1)));
        let team = factory.create_inactive_team(prototype).unwrap();
        let id = team.read().unwrap().get_id();
        id
    };
    admit_spawn_waypoint(&owner);

    let engine = gamelogic::scripting::engine::ScriptEngine::new_in_published_world().unwrap();
    let run = |owner: &mut GameLogic, name| {
        engine
            .friend_execute_action_with_driver_checked(
                &action(
                    ScriptActionType::CreateReinforcementTeam,
                    &[name, "SpawnHere"],
                    None,
                ),
                Some(calling),
                gamelogic::scripting::executor::ScriptContext::new(),
                &mut HostScriptExecutionDriver::with_interpreter(owner, &engine),
            )
            .unwrap();
    };
    run(&mut owner, THIS_TEAM);
    assert_eq!(
        owner.host_objects().registered().count(),
        1,
        "reinforcement looks up the literal prototype name before creating a team"
    );
    run(&mut owner, prototype);
    let reinforcement = owner
        .host_objects()
        .registered_values()
        .find(|unit| unit.id != existing)
        .unwrap();
    assert_eq!(reinforcement.get_position(), Vec3::new(50.0, 0.0, 70.0));
    assert_eq!(reinforcement.team_instance_name, prototype);
    assert_ne!(reinforcement.team_instance_id, Some(calling));
    assert_eq!(peer.host_objects().registered().count(), 1);
}

fn admit_spawn_waypoint(owner: &GameLogic) {
    use game_engine::map_data::{MapData, MapWaypoint};

    let mut map = MapData::new();
    map.width = 64;
    map.height = 64;
    map.heightmap = vec![0; 64 * 64];
    map.waypoints.push(MapWaypoint {
        id: 71,
        name: "SpawnHere".into(),
        location: Vec3::new(50.0, 70.0, 0.0),
        path_label1: String::new(),
        path_label2: String::new(),
        path_label3: String::new(),
        bi_directional: false,
    });
    owner
        .world_services
        .terrain()
        .write()
        .unwrap()
        .load_map_data(map);
}

#[test]
fn waypoint_creation_dispatches_named_and_unnamed_units_in_order() {
    use generals_sim::scripting::program::THIS_TEAM;

    let (mut owner, existing) = world();
    admit_spawn_waypoint(&owner);
    let team = owner.host_default_team_id(1).unwrap();
    let mut unnamed = action(
        ScriptActionType::CreateUnnamedOnTeamAtWaypoint,
        &["NamedActionWarehouse", THIS_TEAM, "SpawnHere"],
        None,
    );
    unnamed.next_action = Some(Box::new(action(
        ScriptActionType::UnitAffectObjectPanelFlags,
        &["WaypointUnit", "indestructible"],
        Some(1),
    )));
    let mut named = action(
        ScriptActionType::CreateNamedOnTeamAtWaypoint,
        &[
            "WaypointUnit",
            "NamedActionWarehouse",
            THIS_TEAM,
            "SpawnHere",
        ],
        None,
    );
    named.next_action = Some(Box::new(unnamed));
    let engine = gamelogic::scripting::engine::ScriptEngine::new_in_published_world().unwrap();
    engine
        .friend_execute_action_with_driver_checked(
            &named,
            Some(team),
            gamelogic::scripting::executor::ScriptContext::new(),
            &mut HostScriptExecutionDriver::with_interpreter(&mut owner, &engine),
        )
        .unwrap();

    let id = owner.host_object_id_by_script_name("WaypointUnit").unwrap();
    let named = owner.host_object(id).unwrap();
    let unnamed = owner
        .host_objects()
        .registered_values()
        .find(|unit| unit.id != id && unit.id != existing)
        .unwrap();
    assert_eq!(owner.host_objects().registered().count(), 3);
    assert_eq!(named.name, "WaypointUnit");
    assert!(named.is_indestructible());
    for unit in [named, unnamed] {
        assert_eq!(unit.team_instance_id, Some(team));
        assert_eq!(unit.get_position(), Vec3::new(50.0, 0.0, 70.0));
    }
}
