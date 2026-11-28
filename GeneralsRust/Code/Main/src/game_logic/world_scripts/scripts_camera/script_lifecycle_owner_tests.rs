//! Original script lifecycle effects use the supplied world/interpreter.
use super::named_command_test_support::world;
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use generals_sim::scripting::program::{Parameter, ParameterType, ScriptAction, ScriptActionType};

fn action(kind: ScriptActionType, name: &str) -> ScriptAction {
    let mut action = ScriptAction::new(kind);
    action
        .add_parameter(Parameter::with_string(
            if matches!(
                kind,
                ScriptActionType::TeamDelete
                    | ScriptActionType::TeamDeleteLiving
                    | ScriptActionType::TeamKill
            ) {
                ParameterType::Team
            } else if kind == ScriptActionType::PlayerKill {
                ParameterType::Side
            } else {
                ParameterType::Unit
            },
            name.into(),
        ))
        .unwrap();
    action
}
fn execute(
    world: &mut GameLogic,
    engine: &gamelogic::scripting::engine::ScriptEngine,
    kind: ScriptActionType,
    name: &str,
) {
    let context = generals_sim::scripting::context::ScriptContext::at_frame(11);
    let mut driver = HostScriptExecutionDriver::with_interpreter(world, engine);
    engine.friend_execute_action_with_driver(&action(kind, name), None, context, &mut driver);
}
fn roster(world: &mut GameLogic, name: &str, members: &[ObjectId]) {
    let team = {
        let mut factory = world.team_factory.lock().unwrap();
        factory
            .init_team_owned(name.into(), "NamedOwner".into(), false, None, Some(1))
            .unwrap();
        factory.create_team(name).unwrap()
    };
    for id in members {
        world.host_object_mut(*id).unwrap().team_instance_name = name.into();
        team.write().unwrap().add_member(id.0);
    }
    team.write().unwrap().set_active();
}

#[test]
fn topple_action_updates_the_interpreter_that_runs_it() {
    use gamelogic::scripting::engine::ScriptEngine;
    use generals_sim::scripting::context::ScriptContext;

    let (mut receiving, _, _) = world();
    receiving
        .with_owned_script_engine(|engine, _| {
            engine.set_topple_direction("FutureStructure", Some(Vec3::X));
        })
        .unwrap();
    let supplied = ScriptEngine::new_in_published_world().unwrap();
    let mut topple = action(ScriptActionType::NamedSetToppleDirection, "FutureStructure");
    topple
        .add_parameter(Parameter::with_coord(ParameterType::Coord3D, Vec3::Y))
        .unwrap();
    supplied
        .friend_execute_action_with_driver_checked(
            &topple,
            None,
            ScriptContext::at_frame(11),
            &mut HostScriptExecutionDriver::new(&mut receiving),
        )
        .unwrap();
    assert_eq!(
        supplied.topple_direction_for_name("FutureStructure"),
        Some(Vec3::Y),
        "the executing interpreter owns the authored direction"
    );
    assert_eq!(
        receiving
            .with_owned_script_engine(|engine, _| engine.topple_direction_for_name("FutureStructure"))
            .flatten(),
        Some(Vec3::X)
    );
}

#[test]
fn named_special_power_uses_receiving_targets_and_stored_waypoint_height() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::{SpecialPowerModuleKind, SpecialPowerModuleMetadata};
    use game_engine::map_data::{MapData, MapWaypoint};

    const POWER: &str = "SuperweaponParticleUplinkCannon";
    let fixture = |target_height| {
        let mut world = GameLogic::new();
        let mut map = MapData::new();
        map.width = 32;
        map.height = 32;
        map.heightmap = vec![10; 32 * 32];
        map.waypoints.push(MapWaypoint {
            id: 7,
            name: "PowerWaypoint".into(),
            location: Vec3::new(100.0, 110.0, 0.0),
            path_label1: String::new(),
            path_label2: String::new(),
            path_label3: String::new(),
            bi_directional: false,
        });
        let terrain = world.world_services.terrain();
        {
            let mut terrain = terrain.write().unwrap();
            terrain.load_map_data(map);
            // TerrainLogic::newMap samples waypoint height before gameplay.
            // Lower the ground after admission, as a crater would.
            for x in 9..=11 {
                for y in 10..=12 {
                    terrain.set_raw_map_height_with_effects(x, y, 0, &mut |_| {});
                }
            }
        }
        assert_eq!(
            world.terrain_height_at(Vec3::new(100.0, 0.0, 110.0)),
            Some(0.0)
        );
        assert_eq!(
            terrain
                .read()
                .unwrap()
                .waypoints()
                .by_name("PowerWaypoint")
                .unwrap()
                .get_location()
                .z,
            6.25,
        );
        world.add_player(Player::new(1, Team::USA, "Caster", true));
        let mut cannon = ThingTemplate::new("NamedPowerCannon");
        cannon
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::FSSuperweapon)
            .set_health(4000.0);
        cannon.special_power_modules.push(SpecialPowerModuleMetadata {
            source_index: 0,
            module_tag: Some("ScriptPower".into()),
            module_kind: SpecialPowerModuleKind::OclSpecialPower,
            special_power_template: POWER.into(),
            special_power_template_id: 1,
            command_power: Some(SpecialPowerType::ParticleCannon),
            reload_time_frames: 30,
            required_science: None,
            public_timer: true,
            shared_n_sync: false,
            shortcut_power: false,
            update_module_starts_attack: false,
            starts_paused: false,
            scripted_special_power_only: false,
            cash_bounty: None,
        });
        world.templates.insert(cannon.name.clone(), cannon);
        let source = world
            .create_object_for_player("NamedPowerCannon", 1, Vec3::new(20.0, 6.25, 30.0))
            .unwrap();
        let target = world
            .create_object_for_player(
                "NamedPowerCannon",
                1,
                Vec3::new(130.0, target_height, 140.0),
            )
            .unwrap();
        assert!(world.set_host_object_name(source, "PowerSource"));
        assert!(world.set_host_object_name(target, "PowerTarget"));
        world.initialize_receiving_named_cache_for_fixture();
        (world, source, target)
    };
    let (mut owner, source, target) = fixture(45.0);
    let (mut peer, peer_source, peer_target) = fixture(95.0);
    assert_eq!((source, target), (peer_source, peer_target));

    let fire = |world: &mut GameLogic, kind, target_type, name: &str| {
        let mut action = action(kind, "PowerSource");
        action
            .add_parameter(Parameter::with_string(
                ParameterType::SpecialPower,
                POWER.into(),
            ))
            .unwrap();
        action
            .add_parameter(Parameter::with_string(target_type, name.into()))
            .unwrap();
        world.execute_owned_script_action_for_test(&action);
    };
    fire(
        &mut owner,
        ScriptActionType::NamedFireSpecialPowerAtWaypoint,
        ParameterType::Waypoint,
        "PowerWaypoint",
    );
    assert!(peer.special_power_strikes.strikes_snapshot().is_empty());
    let strikes = owner.special_power_strikes.strikes_snapshot();
    assert_eq!(strikes.len(), 1, "the authored action applies immediately");
    assert_eq!(strikes[0].source_object, source);
    assert_eq!(
        strikes[0].target_position,
        Vec3::new(100.0, 6.25, 110.0),
        "the script uses the stored waypoint height after terrain changes"
    );
    fire(
        &mut peer,
        ScriptActionType::NamedFireSpecialPowerAtNamed,
        ParameterType::Unit,
        "PowerTarget",
    );
    let strikes = peer.special_power_strikes.strikes_snapshot();
    assert_eq!(strikes.len(), 1);
    assert_eq!(strikes[0].source_object, peer_source);
    assert_eq!(
        strikes[0].target_position,
        peer.host_object(peer_target).unwrap().get_position()
    );
    assert_eq!(owner.special_power_strikes.strikes_snapshot().len(), 1);
}

#[test]
fn named_delete_and_missing_kill_leave_same_id_peer_unchanged() {
    let (mut owner, id, target) = world();
    let (peer, peer_id, _) = world();
    assert_eq!(id, peer_id);
    let engine = gamelogic::scripting::engine::ScriptEngine::new_in_published_world().unwrap();
    execute(&mut owner, &engine, ScriptActionType::NamedKill, "Missing");
    assert!(owner.host_object(id).unwrap().is_alive());
    execute(
        &mut owner,
        &engine,
        ScriptActionType::NamedDelete,
        "NamedUnit",
    );
    assert!(owner.host_object(id).unwrap().status.destroyed);
    assert!(
        !owner.host_object(id).unwrap().status.on_die_started,
        "direct deletion does not run onDie"
    );
    assert!(owner.host_object(target).unwrap().is_alive());
    assert!(peer.host_object(peer_id).unwrap().is_alive());
}

#[test]
fn team_delete_living_uses_effectively_dead_state_instead_of_health() {
    let (mut owner, id, target) = world();
    roster(&mut owner, "OwnedLifecycle", &[id, target]);
    owner.host_object_mut(id).unwrap().health.current = 0.0;
    owner
        .host_object_mut(target)
        .unwrap()
        .status
        .effectively_dead = true;
    let engine = gamelogic::scripting::engine::ScriptEngine::new_in_published_world().unwrap();
    execute(
        &mut owner,
        &engine,
        ScriptActionType::TeamDeleteLiving,
        "OwnedLifecycle",
    );
    assert!(
        owner.host_object(id).unwrap().status.destroyed,
        "zero HP alone is not effectively dead"
    );
    assert!(!owner.host_object(target).unwrap().status.destroyed);
}

#[test]
fn refresh_radar_script_action_targets_only_the_receiving_world() {
    let (mut owner, _, _) = world();
    let (mut peer, _, _) = world();
    assert!(owner.admit_radar_terrain_colors(
        vec![Some([0.1, 0.2, 0.8]); 128 * 128],
        [0.2, 0.3, 0.9],
    ));
    assert!(peer.admit_radar_terrain_colors(
        vec![Some([0.8, 0.2, 0.1]); 128 * 128],
        [0.9, 0.3, 0.2],
    ));
    let owner_generation = owner.radar().terrain_generation();
    let peer_generation = peer.radar().terrain_generation();
    let peer_texture = peer.radar().get_terrain_texture().to_vec();
    let engine = gamelogic::scripting::engine::ScriptEngine::new_in_published_world().unwrap();
    let context = generals_sim::scripting::context::ScriptContext::at_frame(11);
    let mut driver = HostScriptExecutionDriver::with_interpreter(&mut owner, &engine);
    engine.friend_execute_action_with_driver(
        &ScriptAction::new(ScriptActionType::RefreshRadar),
        None,
        context,
        &mut driver,
    );

    assert!(owner.radar().terrain_generation() > owner_generation);
    assert_eq!(peer.radar().terrain_generation(), peer_generation);
    assert_eq!(peer.radar().get_terrain_texture(), peer_texture);
}

#[test]
fn named_kill_topple_callback_reads_only_the_supplied_interpreter() {
    let source = "Object AuthoredToppleTarget\n KindOf = STRUCTURE\n Body = ActiveBody ModuleTag_Body\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = StructureToppleUpdate ModuleTag_Fall\n End\nEnd\n";
    let mut owner = GameLogic::new();
    owner
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "owned-lifecycle.ini".to_owned(),
                source.to_owned(),
            )])
            .unwrap(),
        )
        .unwrap();
    let id = owner
        .create_object("AuthoredToppleTarget", Team::Neutral, Vec3::ZERO)
        .unwrap();
    owner.host_object_mut(id).unwrap().name = "ToppleTarget".into();
    owner.initialize_receiving_named_cache_for_fixture();
    let supplied = gamelogic::scripting::engine::ScriptEngine::new_in_published_world().unwrap();
    let foreign = gamelogic::scripting::engine::ScriptEngine::new_in_published_world().unwrap();
    supplied.set_topple_direction(
        "ToppleTarget",
        Some(glam::Vec3::new(0.0, 1.0, 0.0)),
    );
    foreign.set_topple_direction(
        "ToppleTarget",
        Some(glam::Vec3::new(1.0, 0.0, 0.0)),
    );
    struct NestedKill<'a> {
        owner: &'a mut GameLogic,
        supplied: &'a gamelogic::scripting::engine::ScriptEngine,
    }
    impl gamelogic::scripting::engine::ScriptExecutionDriver for NestedKill<'_> {
        fn after_action(&mut self) -> generals_sim::GameLogicResult<()> {
            Ok(())
        }

        fn object_lifecycle(
            &mut self,
            _: &generals_sim::scripting::lifecycle_actions::LifecycleAction,
            _: Option<gamelogic::common::ObjectID>,
            _: Option<gamelogic::team::TeamID>,
            _: Option<gamelogic::team::TeamID>,
        ) -> Option<generals_sim::GameLogicResult<()>> {
            // Public execution ingress installs the foreign interpreter while
            // the actual nested kill still receives the driving interpreter.
            execute(
                self.owner,
                self.supplied,
                ScriptActionType::NamedKill,
                "ToppleTarget",
            );
            Some(Ok(()))
        }
    }
    let mut nested = NestedKill {
        owner: &mut owner,
        supplied: &supplied,
    };
    foreign.friend_execute_action_with_driver(
        &action(ScriptActionType::NamedKill, "ToppleTarget"),
        None,
        generals_sim::scripting::context::ScriptContext::at_frame(11),
        &mut nested,
    );
    let topple = owner
        .host_object(id)
        .unwrap()
        .structure_topple_data
        .as_ref()
        .unwrap();
    assert_eq!(topple.dir_x, 0.0);
    assert_eq!(topple.dir_y, 1.0);
}

#[test]
fn player_kill_targets_exact_same_faction_owner_and_missing_is_inert() {
    let (mut owner, id, _) = world();
    owner.add_player(Player::new(3, Team::USA, "SameFactionPeer", true));
    let other = owner
        .create_object_for_player("OwnedNamedInfantry", 3, Vec3::ZERO)
        .unwrap();
    let engine = gamelogic::scripting::engine::ScriptEngine::new_in_published_world().unwrap();
    execute(&mut owner, &engine, ScriptActionType::PlayerKill, "Missing");
    assert!(owner.host_object(id).unwrap().is_alive());
    execute(
        &mut owner,
        &engine,
        ScriptActionType::PlayerKill,
        "NamedOwner",
    );
    assert!(!owner.host_object(id).unwrap().is_alive());
    assert!(owner.host_object(other).unwrap().is_alive());
    assert!(owner.players[&3].is_alive);
}
