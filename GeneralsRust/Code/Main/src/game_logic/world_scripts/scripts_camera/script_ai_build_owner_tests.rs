//! ScriptActions.cpp:1082–1087,2284–2310 through real borrowed host admission.
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use gamelogic::scripting::core::{Parameter, ParameterType, ScriptAction, ScriptActionType};
use gamelogic::scripting::engine::ScriptEngine;

fn world() -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    let mut player = Player::new(1, Team::USA, "Owned builder display", false)
        .with_script_identity("OwnedBuilder");
    player.resources.supplies = 10_000;
    player.set_can_build_units(true);
    world.add_player(player);
    world.add_ai_opponent(1, Team::USA, crate::ai::AIDifficulty::Medium);
    // Exercise the authored admission, not the constructor's default layout pad.
    world
        .ai_manager
        .ai_players
        .get_mut(&1)
        .unwrap()
        .building_queue
        .clear();
    world.get_player_mut(1).unwrap().set_can_build_units(true);
    let mut ranger = ThingTemplate::new("AmericaInfantryRanger");
    ranger.set_cost(225, 0);
    world.templates.insert(ranger.name.clone(), ranger);
    let mut barracks = ThingTemplate::new("AmericaBarracks");
    barracks.set_cost(500, 0).add_kind_of(KindOf::Structure);
    world.templates.insert(barracks.name.clone(), barracks);
    world
        .create_object_for_player("AmericaBarracks", 1, Vec3::ZERO)
        .unwrap();
    let unit = world
        .create_object_for_player("AmericaInfantryRanger", 1, Vec3::new(30.0, 0.0, 0.0))
        .unwrap();
    world.host_object_mut(unit).unwrap().is_recruitable = true;
    let mut pile = ThingTemplate::new("SupplyWarehouse");
    pile.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Harvestable)
        .add_kind_of(KindOf::Resource)
        .add_kind_of(KindOf::SupplySource);
    pile.dock_kind = DockKind::SupplyWarehouse;
    world.templates.insert(pile.name.clone(), pile);
    let mut center = ThingTemplate::new("AmericaSupplyCenter");
    center
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::SupplyCenter);
    world.templates.insert(center.name.clone(), center);
    let warehouse = world
        .create_object("SupplyWarehouse", Team::Neutral, Vec3::new(200.0, 0.0, 0.0))
        .unwrap();
    world
        .host_object_mut(warehouse)
        .unwrap()
        .stored_resources
        .supplies = 5_000;
    {
        let mut factory = world.team_factory.lock().unwrap();
        factory
            .init_team_owned(
                "OwnedSquad".into(),
                "OwnedBuilder".into(),
                false,
                None,
                Some(1),
            )
            .unwrap();
        let mut prototype = (*factory.find_team_prototype("OwnedSquad").unwrap()).clone();
        prototype.set_units_info(
            0,
            gamelogic::team::CreateUnitsInfo {
                min_units: 1,
                max_units: 1,
                unit_thing_name: "AmericaInfantryRanger",
            },
        );
        prototype.set_production_priority(100);
        factory.replace_team_prototype(prototype);
        let admitted = factory.find_team_prototype("OwnedSquad").unwrap();
        assert_eq!(admitted.units_info().len(), 1);
        assert_eq!(
            admitted.units_info()[0].unit_thing_name,
            "AmericaInfantryRanger"
        );
        assert_eq!(admitted.units_info()[0].min_units, 1);
        assert_eq!(admitted.units_info()[0].max_units, 1);
        assert_eq!(admitted.get_owner_name().as_str(), "OwnedBuilder");
        assert_eq!(admitted.get_production_priority(), 100);
    }
    (world, unit, warehouse)
}

fn action(kind: ScriptActionType, token_type: ParameterType, token: &str) -> ScriptAction {
    let mut action = ScriptAction::new(kind);
    action
        .add_parameter(Parameter::with_string(token_type, token.into()))
        .unwrap();
    action
}

fn run(engine: &ScriptEngine, world: &mut GameLogic, action: &ScriptAction) {
    let frame = world.frame;
    engine.friend_execute_action_with_driver(
        action,
        None,
        gamelogic::scripting::executor::ScriptContext::at_frame(frame),
        &mut HostScriptExecutionDriver::new(world),
    );
}

#[test]
fn authored_build_recruit_and_supply_actions_mutate_only_the_borrowed_world() {
    let (mut owner, unit, warehouse) = world();
    let (peer, peer_unit, peer_warehouse) = world();
    assert_eq!(unit, peer_unit);
    assert_eq!(warehouse, peer_warehouse);
    let peer_team_before = peer.host_object(peer_unit).unwrap().team_instance_id;
    let peer_before =
        serde_json::to_vec(&peer.ai_manager.ai_players[&1].capture_queue_persist()).unwrap();
    let engine = ScriptEngine::new().unwrap();
    run(
        &engine,
        &mut owner,
        &action(
            ScriptActionType::BuildTeam,
            ParameterType::Team,
            "OwnedSquad",
        ),
    );
    let queued = owner.ai_manager.ai_players[&1]
        .team_queue
        .front()
        .expect("actual AI build queue");
    assert_eq!(queued.name, "OwnedSquad");
    assert!(queued.priority_build);
    assert_eq!(queued.work_orders[0].num_required, 1);
    let mut recruit = action(
        ScriptActionType::RecruitTeam,
        ParameterType::Team,
        "OwnedSquad",
    );
    recruit
        .add_parameter(Parameter::with_real(ParameterType::Real, 250.0))
        .unwrap();
    run(&engine, &mut owner, &recruit);
    let ready = owner.ai_manager.ai_players[&1]
        .team_ready_queue
        .front()
        .expect("actual recruited team");
    // CPP AIPlayer.cpp2593-2601 prepends the ready team with NULL workOrders.
    // Its canonical roster, not a duplicate observed list, owns membership.
    assert!(ready.work_orders.is_empty());
    assert_eq!(ready.frame_started, owner.frame);
    let recruited_id = ready.team_id.expect("exact recruited instance");
    assert_eq!(
        owner.host_object(unit).unwrap().team_instance_id,
        Some(recruited_id)
    );
    assert_eq!(
        owner.host_object(unit).unwrap().team_instance_name,
        "OwnedSquad"
    );
    assert_eq!(owner.host_object(unit).unwrap().owner_player_id, Some(1));
    let recruited = owner
        .team_factory
        .lock()
        .unwrap()
        .find_team_by_id(recruited_id)
        .unwrap();
    assert_eq!(recruited.read().unwrap().get_members(), &[unit.0]);
    assert_eq!(
        peer.host_object(peer_unit).unwrap().team_instance_id,
        peer_team_before
    );
    let mut supply = action(
        ScriptActionType::AiPlayerBuildSupplyCenter,
        ParameterType::Side,
        "OwnedBuilder",
    );
    supply
        .add_parameter(Parameter::with_string(
            ParameterType::ObjectType,
            "AmericaSupplyCenter".into(),
        ))
        .unwrap();
    supply
        .add_parameter(Parameter::with_int(ParameterType::Int, 100))
        .unwrap();
    run(&engine, &mut owner, &supply);
    let pad = owner.ai_manager.ai_players[&1]
        .building_queue
        .iter()
        .find(|pad| pad.template_name == "AmericaSupplyCenter")
        .expect("actual supply construction pad");
    assert!(pad.is_priority);
    assert!((pad.position - Vec3::new(200.0, 0.0, 0.0)).length() < 80.0);
    assert_eq!(
        serde_json::to_vec(&peer.ai_manager.ai_players[&1].capture_queue_persist()).unwrap(),
        peer_before
    );
    assert!(
        peer.host_object(peer_unit)
            .unwrap()
            .requested_destination
            .is_none()
    );
    let before = owner.ai_manager.ai_players[&1].team_queue.len();
    run(
        &engine,
        &mut owner,
        &action(
            ScriptActionType::BuildTeam,
            ParameterType::Team,
            "MissingPrototype",
        ),
    );
    assert_eq!(owner.ai_manager.ai_players[&1].team_queue.len(), before);
}

#[test]
fn upgrade_and_nearest_team_actions_apply_immediately_to_actual_owner() {
    use super::condition_cursor_control_tests::admit;
    let (mut owner, unit, _) = world();
    let (peer, peer_unit, _) = world();
    assert_eq!(unit, peer_unit);
    admit(&owner, "NearestOwnedTeam", 77, Some(unit));
    admit(&peer, "NearestOwnedTeam", 77, Some(peer_unit));
    let before = peer.ai_manager.ai_players[&1].building_queue.len();
    let engine = ScriptEngine::new().unwrap();
    let mut nearest = action(
        ScriptActionType::AiPlayerBuildTypeNearestTeam,
        ParameterType::Side,
        "OwnedBuilder",
    );
    nearest
        .add_parameter(Parameter::with_string(
            ParameterType::ObjectType,
            "AmericaPatriotBattery".into(),
        ))
        .unwrap();
    nearest
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "NearestOwnedTeam".into(),
        ))
        .unwrap();
    run(&engine, &mut owner, &nearest);
    let pad = owner.ai_manager.ai_players[&1]
        .building_queue
        .last()
        .unwrap();
    assert_eq!(pad.template_name, "AmericaPatriotBattery");
    assert_eq!(
        pad.position,
        owner.host_object(unit).unwrap().get_position()
    );
    assert_eq!(peer.ai_manager.ai_players[&1].building_queue.len(), before);
    let mut upgrade = action(
        ScriptActionType::AiPlayerBuildUpgrade,
        ParameterType::Side,
        "OwnedBuilder",
    );
    upgrade
        .add_parameter(Parameter::with_string(
            ParameterType::Upgrade,
            "Upgrade_AmericaRangerCaptureBuilding".into(),
        ))
        .unwrap();
    run(&engine, &mut owner, &upgrade);
    assert!(
        owner
            .get_player(1)
            .unwrap()
            .has_queued_upgrade("Upgrade_AmericaRangerCaptureBuilding")
    );
    assert!(
        !peer
            .get_player(1)
            .unwrap()
            .has_queued_upgrade("Upgrade_AmericaRangerCaptureBuilding")
    );
    assert_eq!(owner.ai_manager.ai_players[&1].activity_count, 1);
    assert_eq!(owner.ai_manager.ai_players.len(), 1);
    let money = owner.get_player(1).unwrap().resources.supplies;
    run(&engine, &mut owner, &upgrade);
    assert_eq!(owner.get_player(1).unwrap().resources.supplies, money); // already queued admission rejected
}

#[test]
fn approach_move_uses_owned_team_and_owned_enemy_numbered_waypoint() {
    use super::condition_cursor_control_tests::admit;
    let (mut owner, unit, _) = super::named_command_test_support::world();
    let (mut peer, peer_unit, _) = super::named_command_test_support::world();
    assert_eq!(unit, peer_unit);
    for (logic, x) in [(&mut owner, 200.0), (&mut peer, 800.0)] {
        admit(logic, "ActualApproach", 77, Some(unit));
        logic.get_player_mut(2).unwrap().start_position = 1;
        let mut map = gamelogic::system::map_loader::MapData::new();
        map.width = 100;
        map.height = 100;
        map.heightmap = vec![0; 10000];
        map.waypoints
            .push(gamelogic::system::map_loader::MapWaypoint {
                id: 71,
                name: "OwnedApproachPoint".into(),
                location: gamelogic::system::map_loader::Coord3D::new(x, 60.0, 0.0),
                path_label1: "ActualPath2".into(),
                path_label2: String::new(),
                path_label3: String::new(),
                bi_directional: false,
            });
        logic
            .world_services
            .terrain()
            .write()
            .unwrap()
            .load_map_data(map);
    }
    let mut action = action(
        ScriptActionType::SkirmishMoveToApproachPath,
        ParameterType::Team,
        "ActualApproach",
    );
    action
        .add_parameter(Parameter::with_string(
            ParameterType::WaypointPath,
            "ActualPath".into(),
        ))
        .unwrap();
    let engine = ScriptEngine::new().unwrap();
    run(&engine, &mut owner, &action);
    assert_eq!(
        owner.host_object(unit).unwrap().requested_destination,
        Some(Vec3::new(200.0, 0.0, 60.0))
    );
    assert_eq!(
        peer.host_object(peer_unit).unwrap().requested_destination,
        None
    );
}

#[test]
fn front_defense_script_admits_actual_layout_row_in_supplied_ai() {
    let (mut owner, _, _) = world();
    let (peer, _, _) = world();
    let mut defense = ThingTemplate::new("OwnedDefense");
    defense
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBaseDefense)
        .set_health(1000.0);
    owner.templates.insert(defense.name.clone(), defense);
    owner.world_min = Vec3::splat(-1000.0);
    owner.world_max = Vec3::splat(1000.0);
    owner.ai_manager.ai_players.get_mut(&1).unwrap().base_center = Vec3::ZERO;
    let before = owner.ai_manager.ai_players[&1].building_queue.len();
    let peer_before = peer.ai_manager.ai_players[&1].building_queue.len();
    let engine = ScriptEngine::new().unwrap();
    super::condition_cursor_control_tests::admit(&owner, "DefenseCaller", 77, None);
    engine.friend_execute_action_with_driver(
        &action(
            ScriptActionType::SkirmishBuildStructureFront,
            ParameterType::ObjectType,
            "OwnedDefense",
        ),
        Some(77),
        gamelogic::scripting::executor::ScriptContext::at_frame(0),
        &mut HostScriptExecutionDriver::new(&mut owner),
    );
    assert_eq!(
        owner.ai_manager.ai_players[&1].building_queue.len(),
        before + 1
    );
    assert_eq!(
        owner.ai_manager.ai_players[&1]
            .building_queue
            .last()
            .unwrap()
            .template_name,
        "OwnedDefense"
    );
    assert_eq!(
        peer.ai_manager.ai_players[&1].building_queue.len(),
        peer_before
    );
}

#[test]
fn panic_action_uses_actual_team_member_and_own_closest_waypoint() {
    use super::condition_cursor_control_tests::admit;
    let (mut owner, unit, foreign) = super::named_command_test_support::world();
    let (mut peer, peer_unit, _) = super::named_command_test_support::world();
    for logic in [&mut owner, &mut peer] {
        admit(logic, "OwnedPanic", 88, Some(unit));
        let mut map = gamelogic::system::map_loader::MapData::new();
        map.width = 100;
        map.height = 100;
        map.heightmap = vec![0; 10000];
        for (id, x) in [(81, 60.0), (82, 500.0)] {
            map.waypoints
                .push(gamelogic::system::map_loader::MapWaypoint {
                    id,
                    name: format!("Panic{id}"),
                    location: gamelogic::system::map_loader::Coord3D::new(x, 50.0, 0.0),
                    path_label1: "ActualPanicPath".into(),
                    path_label2: String::new(),
                    path_label3: String::new(),
                    bi_directional: false,
                });
        }
        logic
            .world_services
            .terrain()
            .write()
            .unwrap()
            .load_map_data(map);
    }
    let mut panic = action(
        ScriptActionType::TeamPanic,
        ParameterType::Team,
        "OwnedPanic",
    );
    panic
        .add_parameter(Parameter::with_string(
            ParameterType::WaypointPath,
            "MissingPath".into(),
        ))
        .unwrap();
    let engine = ScriptEngine::new().unwrap();
    run(&engine, &mut owner, &panic);
    assert!(owner.host_object(unit).unwrap().panic_runtime.is_none());
    panic.parameters[1].as_mut().unwrap().string_value = "ActualPanicPath".into();
    run(&engine, &mut owner, &panic);
    assert_eq!(
        owner
            .host_object(unit)
            .unwrap()
            .panic_runtime
            .as_ref()
            .unwrap()
            .current_waypoint_id,
        81
    );
    assert_eq!(owner.host_object(unit).unwrap().ai_state, AIState::Panic);
    assert!(owner.host_object(foreign).unwrap().panic_runtime.is_none());
    assert!(peer.host_object(peer_unit).unwrap().panic_runtime.is_none());
}

#[test]
fn partial_button_filters_actual_valid_members_before_percentage_without_readiness_gate() {
    use crate::command_system::SpecialPowerType;
    use game_engine::common::ini::ini_command_button::CommandButton;
    let (mut owner, first, invalid) = super::named_command_test_support::world();
    let (peer, _, _) = super::named_command_test_support::world();
    let second = owner
        .create_object_for_player("OwnedNamedInfantry", 1, Vec3::new(80.0, 0.0, 50.0))
        .unwrap();
    for id in [first, second] {
        owner
            .host_object_mut(id)
            .unwrap()
            .template_mut()
            .special_power_modules
            .push(SpecialPowerModuleMetadata {
                source_index: 0,
                module_tag: None,
                module_kind: SpecialPowerModuleKind::SpecialAbility,
                special_power_template: "OwnerCia".into(),
                special_power_template_id: 99,
                command_power: Some(SpecialPowerType::CiaIntelligence),
                reload_time_frames: 1800,
                required_science: None,
                public_timer: false,
                shared_n_sync: false,
                shortcut_power: false,
                update_module_starts_attack: false,
                starts_paused: false,
                scripted_special_power_only: false,
            });
        owner
            .host_object_mut(id)
            .unwrap()
            .set_special_power_ready_seconds(&SpecialPowerType::CiaIntelligence, 999.0);
    }
    let mut button = CommandButton::new("OwnerCiaButton".into());
    button.special_power_template = Some("OwnerCia".into());
    let roster = [invalid.0, first.0, second.0];
    assert_eq!(
        owner.owned_partial_button_members(&roster, &button, 50.0),
        vec![first]
    );
    assert_eq!(
        owner.owned_partial_button_members(&roster, &button, 100.0),
        vec![first, second]
    );
    assert!(
        peer.owned_partial_button_members(&roster, &button, 100.0)
            .is_empty()
    );
    assert!(
        owner
            .owned_partial_button_members(
                &roster,
                &CommandButton::new("Command_Stop".into()),
                100.0
            )
            .is_empty()
    );
    button.options_bits =
        gamelogic::object::update::special_power_update::SpecialPowerCommandOption::NEED_TARGET_POS
            .bits();
    assert!(
        owner
            .owned_partial_button_members(&roster, &button, 100.0)
            .is_empty()
    );
}

#[test]
fn partial_authored_button_executes_actual_power_for_first_valid_owned_team_member() {
    use super::condition_cursor_control_tests::admit;
    use crate::command_system::SpecialPowerType;
    use game_engine::common::ini::ini_command_button::CommandButton;
    let (mut owner, first, _) = super::named_command_test_support::world();
    let (peer, peer_first, _) = super::named_command_test_support::world();
    assert_eq!(first, peer_first);
    let second = owner
        .create_object_for_player("OwnedNamedInfantry", 1, Vec3::new(80.0, 0.0, 50.0))
        .unwrap();
    let invalid = owner
        .create_object_for_player("OwnedNamedInfantry", 1, Vec3::new(90.0, 0.0, 50.0))
        .unwrap();
    for id in [first, second] {
        owner
            .host_object_mut(id)
            .unwrap()
            .template_mut()
            .special_power_modules
            .push(SpecialPowerModuleMetadata {
                source_index: 0,
                module_tag: None,
                module_kind: SpecialPowerModuleKind::SpyVisionSpecialPower,
                special_power_template: "OwnerCia".into(),
                special_power_template_id: 99,
                command_power: Some(SpecialPowerType::CiaIntelligence),
                reload_time_frames: 1800,
                required_science: None,
                public_timer: false,
                shared_n_sync: false,
                shortcut_power: false,
                update_module_starts_attack: false,
                starts_paused: false,
                scripted_special_power_only: false,
            });
        owner
            .host_object_mut(id)
            .unwrap()
            .set_special_power_ready_seconds(&SpecialPowerType::CiaIntelligence, 0.0);
    }
    admit(&owner, "ActualPartial", 91, Some(first));
    owner
        .team_factory
        .lock()
        .unwrap()
        .find_team_by_id(91)
        .unwrap()
        .write()
        .unwrap()
        .restore_owned_members(&[invalid.0, first.0, second.0]);
    let mut button = CommandButton::new("ArbitraryOwnedCiaAbility".into());
    button.command = "SPECIAL_POWER".into();
    button.special_power_template = Some("OwnerCia".into());
    owner
        .apply_owned_team_partial_button_definition(
            gamelogic::scripting::core::THIS_TEAM,
            &button,
            50.0,
            Some(91),
            None,
        )
        .unwrap();
    assert_eq!(owner.cia_intelligence.activations(), 1);
    assert_eq!(
        owner.cia_intelligence.active_scans()[0].caster_id,
        Some(first)
    );
    assert_eq!(peer.cia_intelligence.activations(), 0);
    assert!(
        owner.host_object(first).unwrap().special_power_cooldowns
            [&SpecialPowerType::CiaIntelligence]
            > 0.0
    );
    assert_eq!(
        owner
            .host_object(second)
            .unwrap()
            .special_power_cooldowns
            .get(&SpecialPowerType::CiaIntelligence)
            .copied()
            .unwrap_or(0.0),
        0.0
    );
}
