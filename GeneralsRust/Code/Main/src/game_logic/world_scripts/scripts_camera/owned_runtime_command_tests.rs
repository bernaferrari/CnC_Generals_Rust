//! Actual ScriptActions effects against distinct driving owners with colliding IDs.
use super::named_command_test_support::{execute, world};
use super::*;
use gamelogic::scripting::core::{
    Parameter, ParameterType, ScriptAction, ScriptActionType, THIS_TEAM,
};
use gamelogic::scripting::engine::ScriptEngine;

fn roster(owner: &GameLogic, name: &str, unit: ObjectId, player: u32) -> u32 {
    let mut factory = owner.team_factory.lock().unwrap();
    factory
        .init_team(name.into(), "NamedOwner".into(), false, None)
        .unwrap();
    let team = factory.create_team(name).unwrap();
    let mut team = team.write().unwrap();
    team.set_controlling_player_id_for_owned_members(Some(player));
    team.add_member(unit.0);
    team.get_id()
}

#[test]
fn team_attack_team_keeps_group_and_locomotor_and_uses_owned_victim_roster() {
    let (mut owner, unit, target) = world();
    let (peer, peer_unit, peer_target) = world();
    assert_eq!((unit, target), (peer_unit, peer_target));
    roster(&owner, "Attackers", unit, 1);
    roster(&owner, "Victims", target, 2);
    roster(&peer, "Attackers", peer_unit, 1);
    roster(&peer, "Victims", peer_unit, 1);
    assert_eq!(
        execute(
            &mut owner,
            &[(ScriptActionType::TeamAttackTeam, "Attackers", "Victims")]
        ),
        1
    );
    let object = owner.host_object(unit).unwrap();
    assert_eq!(object.target, Some(target));
    assert_eq!(object.formation_id, 19);
    assert_eq!(
        object.jet_ai.cur_locomotor_set.as_deref(),
        Some("SET_PANIC")
    );
    assert_eq!(object.max_shots_to_fire, -1);
    assert_eq!(
        object.last_command_source,
        crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT
    );
    assert_eq!(peer.host_object(peer_unit).unwrap().target, None);
}

#[test]
fn named_waypoint_orders_read_owned_map_clear_group_and_use_script_source() {
    let (mut owner, unit, _) = world();
    let (peer, peer_unit, _) = world();
    let mut map = gamelogic::system::map_loader::MapData::new();
    map.width = 64;
    map.height = 64;
    map.heightmap = vec![0; 64 * 64];
    map.boundaries = vec![gamelogic::common::ICoord2D::new(64, 64)];
    map.waypoints
        .push(gamelogic::system::map_loader::MapWaypoint {
            id: 9101,
            name: "OwnedWaypoint".into(),
            location: gamelogic::common::Coord3D::new(80.0, 90.0, 0.0),
            path_label1: "OwnedPath".into(),
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
    owner
        .host_object_mut(unit)
        .unwrap()
        .stamp_pending_waypoint_labels(["OldPath".to_owned()]);
    assert_eq!(
        execute(
            &mut owner,
            &[(
                ScriptActionType::MoveNamedUnitTo,
                "NamedUnit",
                "OwnedWaypoint"
            )]
        ),
        1
    );
    let object = owner.host_object(unit).unwrap();
    assert_eq!(object.formation_id, 0);
    assert_eq!(
        object.jet_ai.cur_locomotor_set.as_deref(),
        Some("SET_NORMAL")
    );
    assert_eq!(
        object.last_command_source,
        crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT
    );
    assert_eq!(
        object.requested_destination,
        Some(Vec3::new(80.0, 0.0, 90.0))
    );
    assert_eq!(
        execute(
            &mut owner,
            &[(
                ScriptActionType::NamedFollowWaypoints,
                "NamedUnit",
                "OwnedPath"
            )]
        ),
        1
    );
    assert_eq!(
        owner.host_object(unit).unwrap().last_command_source,
        crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT
    );
    assert_eq!(peer.host_object(peer_unit).unwrap().formation_id, 19);
    assert_eq!(
        peer.host_object(peer_unit).unwrap().requested_destination,
        None
    );
}

#[test]
fn team_move_uses_the_receiving_world_for_colliding_object_ids() {
    let (mut owner, unit, _) = world();
    let (peer, peer_unit, _) = world();
    assert_eq!(unit, peer_unit);
    roster(&owner, "MoveTeam", unit, 1);
    roster(&peer, "MoveTeam", peer_unit, 1);

    let mut map = gamelogic::system::map_loader::MapData::new();
    map.width = 64;
    map.height = 64;
    map.heightmap = vec![0; 64 * 64];
    map.boundaries = vec![gamelogic::common::ICoord2D::new(64, 64)];
    map.waypoints
        .push(gamelogic::system::map_loader::MapWaypoint {
            id: 9201,
            name: "MoveOwnerWaypoint".into(),
            location: gamelogic::common::Coord3D::new(180.0, 210.0, 0.0),
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

    assert_eq!(
        execute(
            &mut owner,
            &[(
                ScriptActionType::MoveTeamTo,
                "MoveTeam",
                "MoveOwnerWaypoint"
            )]
        ),
        1
    );
    assert_eq!(
        owner.host_object(unit).unwrap().requested_destination,
        Some(Vec3::new(180.0, 0.0, 210.0))
    );
    assert_eq!(
        peer.host_object(peer_unit).unwrap().requested_destination,
        None
    );
}

#[test]
fn team_move_this_team_uses_calling_team_identity() {
    let (mut owner, calling_unit, _) = world();
    let condition_unit = owner
        .create_object_for_player("OwnedNamedInfantry", 2, Vec3::new(110.0, 0.0, 110.0))
        .unwrap();
    let mut factory = owner.team_factory.lock().unwrap();
    factory
        .init_team("SameNameMoveTeam".into(), "NamedOwner".into(), false, None)
        .unwrap();
    let calling = factory.create_inactive_team("SameNameMoveTeam").unwrap();
    calling.write().unwrap().add_member(calling_unit.0);
    let calling_id = calling.read().unwrap().get_id();
    let condition = factory.create_inactive_team("SameNameMoveTeam").unwrap();
    condition.write().unwrap().add_member(condition_unit.0);
    let condition_id = condition.read().unwrap().get_id();
    drop(factory);

    let mut map = gamelogic::system::map_loader::MapData::new();
    map.waypoints
        .push(gamelogic::system::map_loader::MapWaypoint {
            id: 9202,
            name: "ContextMoveWaypoint".into(),
            location: gamelogic::common::Coord3D::new(210.0, 230.0, 0.0),
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

    let mut action = ScriptAction::new(ScriptActionType::MoveTeamTo);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            THIS_TEAM.into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Waypoint,
            "ContextMoveWaypoint".into(),
        ))
        .unwrap();
    let mut engine = ScriptEngine::new().unwrap();
    engine.set_external_eval_context(None, Some(condition_id));
    engine.friend_execute_action_with_driver(
        &action,
        Some(calling_id),
        gamelogic::scripting::executor::ScriptContext::at_frame(owner.frame),
        &mut super::script_execution_driver::HostScriptExecutionDriver::new(&mut owner),
    );

    assert_eq!(
        owner
            .host_object(calling_unit)
            .unwrap()
            .requested_destination,
        Some(Vec3::new(210.0, 0.0, 230.0))
    );
    assert_eq!(
        owner
            .host_object(condition_unit)
            .unwrap()
            .requested_destination,
        None
    );
}

#[test]
fn team_move_this_team_prefers_calling_team_identity_over_condition_team() {
    let (mut owner, calling_unit, _) = world();
    let condition_unit = owner
        .create_object_for_player("OwnedNamedInfantry", 2, Vec3::new(100.0, 0.0, 100.0))
        .unwrap();
    let mut factory = owner.team_factory.lock().unwrap();
    factory
        .init_team("SameNameMoveTeam".into(), "NamedOwner".into(), false, None)
        .unwrap();
    let calling = factory.create_inactive_team("SameNameMoveTeam").unwrap();
    calling.write().unwrap().add_member(calling_unit.0);
    let calling_id = calling.read().unwrap().get_id();
    let condition = factory.create_inactive_team("SameNameMoveTeam").unwrap();
    condition.write().unwrap().add_member(condition_unit.0);
    let condition_id = condition.read().unwrap().get_id();
    drop(factory);

    let mut map = gamelogic::system::map_loader::MapData::new();
    map.waypoints
        .push(gamelogic::system::map_loader::MapWaypoint {
            id: 9202,
            name: "ContextMoveWaypoint".into(),
            location: gamelogic::common::Coord3D::new(210.0, 230.0, 0.0),
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

    let mut action = ScriptAction::new(ScriptActionType::MoveTeamTo);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            THIS_TEAM.into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Waypoint,
            "ContextMoveWaypoint".into(),
        ))
        .unwrap();
    let mut engine = ScriptEngine::new().unwrap();
    engine.set_external_eval_context(None, Some(condition_id));
    engine.friend_execute_action_with_driver(
        &action,
        Some(calling_id),
        gamelogic::scripting::executor::ScriptContext::at_frame(owner.frame),
        &mut super::script_execution_driver::HostScriptExecutionDriver::new(&mut owner),
    );

    assert_eq!(
        owner
            .host_object(calling_unit)
            .unwrap()
            .requested_destination,
        Some(Vec3::new(210.0, 0.0, 230.0))
    );
    assert_eq!(
        owner
            .host_object(condition_unit)
            .unwrap()
            .requested_destination,
        None
    );
}

#[test]
fn team_transfer_updates_sibling_owner_views_and_preserves_selected_team_scope() {
    let (mut owner, unit, _) = world();
    let (peer, peer_unit, _) = world();
    let team_id = roster(&owner, "TransferTeam", unit, 1);
    assert_eq!(team_id, roster(&peer, "TransferTeam", peer_unit, 1));
    {
        let object = owner.host_object_mut(unit).unwrap();
        object.team_instance_id = Some(team_id);
        object.team_instance_name = "TransferTeam".into();
    }
    let sibling_unit = owner
        .create_object_for_player("OwnedNamedInfantry", 1, Vec3::new(135.0, 0.0, 145.0))
        .expect("create same-world sibling object");
    let sibling_team_id = {
        let mut factory = owner.team_factory.lock().unwrap();
        let sibling = factory
            .create_inactive_team("TransferTeam")
            .expect("create sibling Team instance");
        sibling.write().unwrap().add_member(sibling_unit.0);
        sibling.read().unwrap().get_id()
    };
    {
        let sibling = owner.host_object_mut(sibling_unit).unwrap();
        sibling.team_instance_id = Some(sibling_team_id);
        sibling.team_instance_name = "TransferTeam".into();
    }
    let selected_team_id = owner
        .team_factory
        .with_read_without_flushing(|factory| {
            factory
                .find_team_instances("TransferTeam")
                .first()
                .and_then(|team| team.read().ok().map(|team| team.get_id()))
        })
        .unwrap()
        .expect("factory has a selected TransferTeam instance");
    assert_eq!(selected_team_id, sibling_team_id);
    assert_eq!(
        execute(
            &mut owner,
            &[(
                ScriptActionType::TeamTransferToPlayer,
                "TransferTeam",
                "NamedVictim"
            )]
        ),
        1
    );
    assert_eq!(owner.host_object(unit).unwrap().owner_player_id, Some(2));
    assert_eq!(
        owner.host_object(sibling_unit).unwrap().owner_player_id,
        Some(2)
    );
    assert_eq!(owner.host_object(unit).unwrap().team, Team::China);
    assert_eq!(owner.host_object(sibling_unit).unwrap().team, Team::China);
    assert_eq!(owner.host_object(unit).unwrap().formation_id, 19);
    let factory = owner.team_factory.lock().unwrap();
    let team = factory.find_team_by_id(team_id).unwrap();
    assert_eq!(team.read().unwrap().get_controlling_player_id(), Some(2));
    assert_eq!(team.read().unwrap().get_members(), &[unit.0]);
    let sibling = factory.find_team_by_id(sibling_team_id).unwrap();
    assert_eq!(sibling.read().unwrap().get_controlling_player_id(), Some(2));
    assert_eq!(sibling.read().unwrap().get_members(), &[sibling_unit.0]);
    assert_ne!(team_id, sibling_team_id);
    assert_eq!(
        peer.host_object(peer_unit).unwrap().owner_player_id,
        Some(1)
    );
}

#[test]
fn sell_everything_uses_owned_faction_structure_filter_and_real_sell_lifecycle() {
    let (mut owner, unit, _) = world();
    let (peer, peer_unit, _) = world();
    let mut template = ThingTemplate::new("OwnedSellFactory");
    template
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSWarFactory)
        .set_health(100.0);
    owner
        .templates
        .insert(template.name.clone(), template.clone());
    let factory = owner
        .create_object_for_player("OwnedSellFactory", 1, Vec3::new(100.0, 0.0, 100.0))
        .unwrap();
    let foreign = owner
        .create_object_for_player("OwnedSellFactory", 2, Vec3::new(130.0, 0.0, 130.0))
        .unwrap();
    assert_eq!(
        execute(
            &mut owner,
            &[(ScriptActionType::PlayerSellEverything, "NamedOwner", "")]
        ),
        1
    );
    assert!(owner.host_object(factory).unwrap().status.sold);
    assert!(owner.host_object(factory).unwrap().status.unselectable);
    assert!(!owner.host_object(unit).unwrap().status.sold);
    assert!(!owner.host_object(foreign).unwrap().status.sold);
    assert!(!peer.host_object(peer_unit).unwrap().status.sold);
}

#[test]
fn command_button_hunt_validates_authored_button_then_arms_owned_real_update() {
    // Immutable authored content is admitted once, without world publication,
    // global resets, or registration of mutable match objects.
    static DEFINITION: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    const BUTTON: &str = "Command_OwnedRuntimeFireWeapon";
    DEFINITION.get_or_init(|| {
        let mut ini = game_engine::common::ini::INI::new();
        ini.with_inline_source(
            &format!("CommandButton {BUTTON}\n Command = FIRE_WEAPON\nEnd\n"),
            |ini| ini.parse_current_file(),
        )
        .unwrap();
    });
    let (mut owner, unit, _) = world();
    let (peer, peer_unit, _) = world();
    roster(&owner, "HuntTeam", unit, 1);
    roster(&peer, "HuntTeam", peer_unit, 1);
    // The actual native classifier needs a hunt-capable authored unit.
    owner.host_object_mut(unit).unwrap().template_name = "OwnedRuntimeRanger".into();
    assert_eq!(
        execute(
            &mut owner,
            &[(
                ScriptActionType::TeamHuntWithCommandButton,
                "HuntTeam",
                "MissingButton"
            )]
        ),
        1
    );
    assert!(
        owner
            .host_object(unit)
            .unwrap()
            .command_button_hunt
            .is_none()
    );
    assert_eq!(
        execute(
            &mut owner,
            &[(
                ScriptActionType::TeamHuntWithCommandButton,
                "HuntTeam",
                BUTTON
            )]
        ),
        1
    );
    let hunt = owner
        .host_object(unit)
        .unwrap()
        .command_button_hunt
        .as_ref()
        .unwrap();
    assert!(hunt.active);
    assert_eq!(hunt.button_name.as_str(), BUTTON);
    assert!(
        peer.host_object(peer_unit)
            .unwrap()
            .command_button_hunt
            .is_none()
    );
}

#[test]
fn guard_object_rejects_missing_owner_target_and_preserves_group_and_locomotor() {
    let (mut owner, unit, target) = world();
    let (peer, peer_unit, _) = world();
    roster(&owner, "GuardTeam", unit, 1);
    roster(&peer, "GuardTeam", peer_unit, 1);
    assert_eq!(
        execute(
            &mut owner,
            &[(
                ScriptActionType::TeamGuardObject,
                "GuardTeam",
                "MissingTarget"
            )]
        ),
        1
    );
    assert_eq!(owner.host_object(unit).unwrap().active_guard_target(), None);
    assert_eq!(
        execute(
            &mut owner,
            &[(
                ScriptActionType::TeamGuardObject,
                "GuardTeam",
                "NamedTarget"
            )]
        ),
        1
    );
    let object = owner.host_object(unit).unwrap();
    assert_eq!(object.active_guard_target(), Some(target));
    assert_eq!(object.formation_id, 19);
    assert_eq!(
        object.jet_ai.cur_locomotor_set.as_deref(),
        Some("SET_PANIC")
    );
    assert_eq!(
        object.last_command_source,
        crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT
    );
    assert_eq!(
        peer.host_object(peer_unit).unwrap().active_guard_target(),
        None
    );
}

#[test]
fn team_damage_runs_immediate_owned_unresistable_health_and_negative_kill() {
    use super::script_execution_driver::HostScriptExecutionDriver;
    use gamelogic::scripting::core::{Parameter, ParameterType, ScriptAction};
    let (mut owner, unit, _) = world();
    let (peer, peer_unit, _) = world();
    roster(&owner, "DamageTeam", unit, 1);
    roster(&peer, "DamageTeam", peer_unit, 1);
    let engine = gamelogic::scripting::engine::ScriptEngine::new().unwrap();
    for amount in [25.0, -1.0] {
        let mut action = ScriptAction::new(ScriptActionType::DamageMembersOfTeam);
        action
            .add_parameter(Parameter::with_string(
                ParameterType::Team,
                "DamageTeam".into(),
            ))
            .unwrap();
        action
            .add_parameter(Parameter::with_real(ParameterType::Real, amount))
            .unwrap();
        let frame = owner.frame;
        let mut driver = HostScriptExecutionDriver::with_interpreter(&mut owner, &engine);
        engine.friend_execute_action_with_driver(
            &action,
            None,
            gamelogic::scripting::executor::ScriptContext::at_frame(frame),
            &mut driver,
        );
        if amount > 0.0 {
            assert_eq!(owner.host_object(unit).unwrap().health.current, 75.0);
        } else {
            assert!(!owner.host_object(unit).unwrap().is_alive());
        }
        assert_eq!(peer.host_object(peer_unit).unwrap().health.current, 100.0);
    }
}

#[test]
fn world_effects_deliver_to_the_driving_interpreter_hooks_before_next_action() {
    use super::script_execution_driver::HostScriptExecutionDriver;
    use gamelogic::scripting::core::{Parameter, ParameterType, ScriptAction};
    let (mut owner, _, _) = world();
    let (peer, _, _) = world();
    let mut engine = gamelogic::scripting::engine::ScriptEngine::new().unwrap();
    engine.set_action_handler(Some(Arc::new(
        crate::game_logic::mission_scripts::MissionScriptActionHandler::new(Arc::clone(
            &owner.mission_scripts,
        )),
    )));
    for kind in [
        ScriptActionType::MoviePlayFullscreen,
        ScriptActionType::RadarForceEnable,
        ScriptActionType::FreezeTime,
    ] {
        let mut action = ScriptAction::new(kind);
        if kind == ScriptActionType::MoviePlayFullscreen {
            action
                .add_parameter(Parameter::with_string(
                    ParameterType::Movie,
                    "OwnedMovie.bik".into(),
                ))
                .unwrap();
        }
        let frame = owner.frame;
        engine.friend_execute_action_with_driver(
            &action,
            None,
            gamelogic::scripting::executor::ScriptContext::at_frame(frame),
            &mut HostScriptExecutionDriver::with_interpreter(&mut owner, &engine),
        );
    }
    assert!(owner.radar_forced());
    assert!(!peer.radar_forced());
    assert!(owner.is_script_time_frozen());
    assert!(!peer.is_script_time_frozen());
    // after_action delivers the hook to the canonical host before proceeding.
    assert_eq!(owner.pending_movie.as_deref(), Some("OwnedMovie.bik"));
    assert!(peer.pending_movie.is_none());
    assert!(owner.mission_scripts.drain_movie_requests().is_empty());
    assert!(peer.mission_scripts.drain_movie_requests().is_empty());
}

#[test]
fn named_presentation_and_sequential_use_owned_identity_then_topple_consumes_owned_cache() {
    use super::script_execution_driver::HostScriptExecutionDriver;
    use gamelogic::scripting::core::{Parameter, ParameterType, Script, ScriptAction, ScriptList};
    let (mut owner, unit, _) = world();
    let (peer, peer_unit, _) = world();
    let mut template = ThingTemplate::new("OwnedTopplingStructure");
    template.add_kind_of(KindOf::Structure).set_health(100.0);
    template.authored_structure_topple = true;
    owner.templates.insert(template.name.clone(), template);
    let building = owner
        .create_object_for_player("OwnedTopplingStructure", 1, Vec3::new(200.0, 0.0, 200.0))
        .unwrap();
    owner.host_object_mut(building).unwrap().name = "OwnedFallingHall".into();
    assert!(owner.add_receiving_named_script_object(building));
    let engine = gamelogic::scripting::engine::ScriptEngine::new().unwrap();
    let mut script = Script::new();
    script.script_name = "OwnedUnitSequence".into();
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    for kind in [
        ScriptActionType::NamedHideSpecialPowerDisplay,
        ScriptActionType::NamedShowSpecialPowerDisplay,
        ScriptActionType::UnitExecuteSequentialScript,
        ScriptActionType::UnitStopSequentialScript,
        ScriptActionType::NamedSetToppleDirection,
    ] {
        let mut action = ScriptAction::new(kind);
        let name = if kind == ScriptActionType::NamedSetToppleDirection {
            "OwnedFallingHall"
        } else {
            "NamedUnit"
        };
        action
            .add_parameter(Parameter::with_string(ParameterType::Unit, name.into()))
            .unwrap();
        if kind == ScriptActionType::UnitExecuteSequentialScript {
            action
                .add_parameter(Parameter::with_string(
                    ParameterType::Script,
                    "OwnedUnitSequence".into(),
                ))
                .unwrap();
        }
        if kind == ScriptActionType::NamedSetToppleDirection {
            action
                .add_parameter(Parameter::with_coord(
                    ParameterType::Coord3D,
                    gamelogic::scripting::core::Coord3D::new(0.0, 1.0, 0.0),
                ))
                .unwrap();
        }
        let frame = owner.frame;
        engine.friend_execute_action_with_driver(
            &action,
            None,
            gamelogic::scripting::executor::ScriptContext::at_frame(frame),
            &mut HostScriptExecutionDriver::with_interpreter(&mut owner, &engine),
        );
        match kind {
            ScriptActionType::NamedHideSpecialPowerDisplay => {
                assert!(owner.script_superweapon_hidden_objects.contains(&unit))
            }
            ScriptActionType::NamedShowSpecialPowerDisplay => {
                assert!(!owner.script_superweapon_hidden_objects.contains(&unit))
            }
            ScriptActionType::UnitExecuteSequentialScript => {
                assert!(engine.has_active_sequential_script_for_object(unit.0))
            }
            ScriptActionType::UnitStopSequentialScript => {
                assert!(!engine.has_active_sequential_script_for_object(unit.0))
            }
            ScriptActionType::NamedSetToppleDirection => assert!(
                owner
                    .host_object(building)
                    .unwrap()
                    .structure_topple_data
                    .is_none()
            ),
            _ => unreachable!(),
        }
    }
    assert_eq!(peer.host_object(peer_unit).unwrap().formation_id, 19);
    assert!(!peer.script_superweapon_hidden_objects.contains(&peer_unit));
    owner.owned_script_engine = Some(engine);
    assert!(owner.try_begin_structure_topple_instead_of_destroy(building));
    let data = owner
        .host_object(building)
        .unwrap()
        .structure_topple_data
        .as_ref()
        .unwrap();
    assert!(data.dir_x.abs() < 0.05 && (data.dir_y - 1.0).abs() < 0.05);
}

#[test]
fn input_actions_and_victory_admit_only_the_driving_world() {
    let (mut owner, _, _) = world();
    let (peer, _, _) = world();
    assert!(owner.script_input_enabled());
    assert!(peer.script_input_enabled());
    assert_eq!(
        execute(&mut owner, &[(ScriptActionType::DisableInput, "", "")]),
        1
    );
    assert!(!owner.script_input_enabled());
    assert!(peer.script_input_enabled());
    assert_eq!(
        execute(&mut owner, &[(ScriptActionType::EnableInput, "", "")]),
        1
    );
    assert!(owner.script_input_enabled());
    assert_eq!(
        execute(&mut owner, &[(ScriptActionType::Victory, "", "")]),
        1
    );
    assert!(!owner.script_input_enabled());
    assert!(peer.script_input_enabled());
}

#[test]
fn borrowed_script_topple_override_reaches_immediate_named_and_negative_team_death() {
    use super::script_execution_driver::HostScriptExecutionDriver;
    use gamelogic::scripting::core::{Parameter, ParameterType, ScriptAction};
    for team_kill in [false, true] {
        let (mut owner, _, _) = world();
        let (mut peer, _, _) = world();
        let mut template = ThingTemplate::new("BorrowedScriptToppleBuilding");
        template.add_kind_of(KindOf::Structure).set_health(100.0);
        template.authored_structure_topple = true;
        owner
            .templates
            .insert(template.name.clone(), template.clone());
        peer.templates.insert(template.name.clone(), template);
        let building = owner
            .create_object_for_player(
                "BorrowedScriptToppleBuilding",
                1,
                Vec3::new(200.0, 0.0, 200.0),
            )
            .unwrap();
        let foreign = peer
            .create_object_for_player(
                "BorrowedScriptToppleBuilding",
                1,
                Vec3::new(200.0, 0.0, 200.0),
            )
            .unwrap();
        assert_eq!(building, foreign);
        owner.host_object_mut(building).unwrap().name = "BorrowedFallingHall".into();
        peer.host_object_mut(foreign).unwrap().name = "BorrowedFallingHall".into();
        assert!(owner.add_receiving_named_script_object(building));
        assert!(peer.add_receiving_named_script_object(foreign));
        roster(&owner, "BorrowedKillTeam", building, 1);
        roster(&peer, "BorrowedKillTeam", foreign, 1);
        // Exercise the actual detached-interpreter phase, as a script walk does.
        let engine = owner.owned_script_engine.take().unwrap();
        let foreign_engine = peer.owned_script_engine.take().unwrap();
        foreign_engine.set_topple_direction(
            "BorrowedFallingHall",
            Some(gamelogic::common::Coord3D::new(-1.0, 0.0, 0.0)),
        );
        peer.owned_script_engine = Some(foreign_engine);
        assert!(owner.owned_script_engine.is_none());
        let mut topple = ScriptAction::new(ScriptActionType::NamedSetToppleDirection);
        topple
            .add_parameter(Parameter::with_string(
                ParameterType::Unit,
                "BorrowedFallingHall".into(),
            ))
            .unwrap();
        topple
            .add_parameter(Parameter::with_coord(
                ParameterType::Coord3D,
                gamelogic::scripting::core::Coord3D::new(0.0, 1.0, 0.0),
            ))
            .unwrap();
        engine.friend_execute_action_with_driver(
            &topple,
            None,
            gamelogic::scripting::executor::ScriptContext::at_frame(0),
            &mut HostScriptExecutionDriver::with_interpreter(&mut owner, &engine),
        );
        assert!(
            owner
                .host_object(building)
                .unwrap()
                .structure_topple_data
                .is_none()
        );
        let mut damage = ScriptAction::new(if team_kill {
            ScriptActionType::DamageMembersOfTeam
        } else {
            ScriptActionType::NamedDamage
        });
        damage
            .add_parameter(Parameter::with_string(
                ParameterType::Unit,
                if team_kill {
                    "BorrowedKillTeam"
                } else {
                    "BorrowedFallingHall"
                }
                .into(),
            ))
            .unwrap();
        damage
            .add_parameter(if team_kill {
                Parameter::with_real(ParameterType::Real, -1.0)
            } else {
                Parameter::with_int(ParameterType::Int, 100)
            })
            .unwrap();
        engine.friend_execute_action_with_driver(
            &damage,
            None,
            gamelogic::scripting::executor::ScriptContext::at_frame(0),
            &mut HostScriptExecutionDriver::with_interpreter(&mut owner, &engine),
        );
        let object = owner.host_object(building).unwrap();
        assert!(object.status.on_die_started);
        let data = object.structure_topple_data.as_ref().unwrap();
        assert!(data.dir_x.abs() < 0.05 && (data.dir_y - 1.0).abs() < 0.05);
        assert_eq!(peer.host_object(foreign).unwrap().health.current, 100.0);
        assert!(
            peer.host_object(foreign)
                .unwrap()
                .structure_topple_data
                .is_none()
        );
    }
}
