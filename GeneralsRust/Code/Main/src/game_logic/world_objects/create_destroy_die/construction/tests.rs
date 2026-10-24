use super::*;

fn construction_world(cash: u32, boxes: u32) -> (GameLogic, ObjectId) {
    let mut logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = cash;
    player.did_preorder = true;
    logic.add_player(player);
    let mut builder = ThingTemplate::new("OwnedDozer");
    builder.add_kind_of(KindOf::Dozer).set_health(200.0);
    logic.templates.insert("OwnedDozer".into(), builder);
    let constructor = logic
        .create_object_for_player("OwnedDozer", 0, Vec3::ZERO)
        .unwrap();
    let mut template = ThingTemplate::new("OwnedScaffold");
    template
        .add_kind_of(KindOf::Structure)
        .set_health(900.0)
        .set_cost(300, -15);
    // A real onCreate effect: SupplyWarehouseCreate registers and seeds stores.
    template.has_supply_warehouse_create = true;
    template.has_preorder_create = true;
    template.has_supply_center_create = true;
    template.dock_kind = crate::game_logic::DockKind::SupplyWarehouse;
    template.dock_starting_boxes = Some(boxes);
    logic.templates.insert("OwnedScaffold".into(), template);
    (logic, constructor)
}

#[test]
fn ordinary_scaffold_factory_observes_cpp_birth_before_payment_and_placement() {
    let (mut logic, constructor) = construction_world(1000, 7);
    let before_power = logic.get_player(0).unwrap().power_available;
    let cost = Resources {
        supplies: 300,
        power: -15,
    };
    let position = Vec3::new(45.0, 0.0, 70.0);
    let mut witnessed = false;
    let id = logic
        .create_scaffold_with_factory_observer(
            "OwnedScaffold",
            Team::USA,
            Some(0),
            position,
            0.7,
            Some(constructor),
            Some(cost),
            |world, id| {
                witnessed = true;
                let object = world.host_object(id).unwrap();
                assert!(object.status.under_construction);
                assert!(!object.construction_power_admitted);
                assert_eq!(object.health.current, 900.0);
                assert_eq!(object.construction_percent, 1.0);
                assert_eq!(object.producer_id, None);
                assert_eq!(object.builder_id, None);
                assert_eq!(object.get_position(), Vec3::ZERO);
                assert_eq!(object.stored_resources.supplies, 7 * 75);
                assert!(
                    !crate::game_logic::host_preorder_create::has_preorder_model_bit(
                        object.model_condition_bits
                    )
                );
                assert_eq!(world.supply_create_warehouse_registers, 1);
                assert_eq!(world.get_player(0).unwrap().effective_supplies(), 1000);
            },
        )
        .unwrap();
    assert!(witnessed);
    let object = logic.host_object(id).unwrap();
    assert_eq!(object.producer_id, Some(constructor));
    assert_eq!(object.builder_id, Some(constructor));
    assert_eq!(object.health.current, 1.0);
    assert_eq!(object.construction_percent, 0.0);
    assert_eq!(object.get_position(), position);
    assert_eq!(object.get_orientation(), 0.7);
    assert_eq!(logic.get_player(0).unwrap().effective_supplies(), 700);
    assert_eq!(logic.get_player(0).unwrap().power_available, before_power);
    assert_eq!(
        logic.get_player(0).unwrap().statistics.structures_built(),
        0
    );
}

#[test]
fn same_id_construction_worlds_keep_factory_resources_and_builders_isolated() {
    let (mut first, first_builder) = construction_world(1000, 3);
    let (mut second, second_builder) = construction_world(600, 9);
    assert_eq!(first_builder, second_builder);
    let cost = Resources {
        supplies: 300,
        power: -15,
    };
    let a = first
        .create_dozer_scaffold(
            "OwnedScaffold",
            0,
            first_builder,
            Vec3::new(20.0, 0.0, 30.0),
            0.2,
            cost,
        )
        .unwrap();
    let b = second
        .create_dozer_scaffold(
            "OwnedScaffold",
            0,
            second_builder,
            Vec3::new(80.0, 0.0, 90.0),
            0.9,
            cost,
        )
        .unwrap();
    assert_eq!(a, b);
    second.destroy_object(b);
    assert_eq!(
        first.host_object(a).unwrap().stored_resources.supplies,
        3 * 75
    );
    assert_eq!(
        first.host_object(a).unwrap().get_position(),
        Vec3::new(20.0, 0.0, 30.0)
    );
    assert_eq!(
        first.host_object(a).unwrap().builder_id,
        Some(first_builder)
    );
    assert_eq!(first.get_player(0).unwrap().effective_supplies(), 700);
}

#[test]
fn ordinary_scaffold_defers_create_module_completion_until_real_completion() {
    use crate::game_logic::host_preorder_create::has_preorder_model_bit;
    let (mut logic, constructor) = construction_world(1000, 7);
    let id = logic
        .create_dozer_scaffold(
            "OwnedScaffold",
            0,
            constructor,
            Vec3::new(45.0, 0.0, 70.0),
            0.7,
            Resources {
                supplies: 300,
                power: -15,
            },
        )
        .unwrap();
    assert!(!has_preorder_model_bit(
        logic.host_object(id).unwrap().model_condition_bits
    ));
    let object = logic.host_object_mut(id).unwrap();
    object.construction_percent = 1.0;
    object.set_status_under_construction(false);
    logic.notify_structure_construction_complete(id);
    assert!(has_preorder_model_bit(
        logic.host_object(id).unwrap().model_condition_bits
    ));
}

fn prime_ai_completion(world: &mut GameLogic, expected: ObjectId) {
    use crate::ai::{AIBuildingInfo, AIDifficulty};
    world
        .ai_manager
        .add_ai_player(0, Team::USA, AIDifficulty::Medium);
    let ai = world.ai_manager.ai_players.get_mut(&0).unwrap();
    ai.building_queue.clear();
    ai.next_team_queue_time = 500.0;
    ai.next_building_time = 700.0;
    let mut pad = AIBuildingInfo::new("OwnedScaffold".into(), Vec3::ZERO, 1);
    pad.object_id = Some(expected);
    ai.building_queue.push(pad);
    ai.building_queue
        .push(AIBuildingInfo::new("OwnedScaffold".into(), Vec3::ZERO, 1));
}

fn assert_owned_completion(world: &GameLogic, id: ObjectId) {
    let ai = &world.ai_manager.ai_players[&0];
    assert_eq!(ai.next_building_time, 0.0);
    assert_eq!(ai.next_team_queue_time, 0.0);
    assert!(ai.building_queue[0].is_built);
    assert!(!ai.building_queue[1].is_built);
    assert_eq!(world.script_object_count_changed_frame, 8);
    assert!(world.host_object(id).unwrap().construction_power_admitted);
}

#[test]
fn ordinary_player_completion_reaches_owned_ai_script_and_power_before_modules() {
    let (mut world, builder) = construction_world(1000, 7);
    let id = world
        .create_dozer_scaffold(
            "OwnedScaffold",
            0,
            builder,
            Vec3::ZERO,
            0.0,
            Resources {
                supplies: 300,
                power: 0,
            },
        )
        .unwrap();
    prime_ai_completion(&mut world, id);
    world.frame = 8;
    world.dozer_new_task_build(builder, id);
    world.host_object_mut(builder).unwrap().set_ai_state(
        AIState::Constructing,
        &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
    );
    let object = world.host_object_mut(id).unwrap();
    assert_eq!(object.health.maximum, 900.0);
    // CPP ActiveBody::internalChangeHealth updates canonical state before
    // construction completion observes this partially repaired scaffold.
    object.internal_change_health(450.0 - object.health.current);
    assert_eq!(object.health.current, 450.0);
    assert_eq!(
        object.body_damage_state,
        crate::game_logic::host_enum_table_residual::HostBodyDamageType::Damaged
    );
    let mut observed = false;
    world.notify_structure_construction_complete_with_prepared_observer(id, |world, id| {
        observed = true;
        let object = world.host_object(id).unwrap();
        assert!(!object.status.under_construction);
        assert_eq!(
            object.health.current, 450.0,
            "completion preserves scaffold damage"
        );
        assert_eq!(object.construction_percent, 1.0);
        assert_eq!(
            object.body_damage_state,
            crate::game_logic::host_enum_table_residual::HostBodyDamageType::Damaged
        );
        assert_eq!(object.builder_id, Some(builder));
        assert_eq!(
            world.host_object(builder).unwrap().dozer_task_build_target,
            Some(id)
        );
        assert!(!object.construction_power_admitted);
        assert_eq!(world.get_player(0).unwrap().statistics.resources_spent(), 0);
        assert_eq!(world.supply_create_center_registers, 0);
        assert_eq!(world.ai_manager.ai_players[&0].next_building_time, 700.0);
        assert!(
            !crate::game_logic::host_preorder_create::has_preorder_model_bit(
                object.model_condition_bits
            )
        );
    });
    assert!(observed);
    assert_owned_completion(&world, id);
    assert!(
        crate::game_logic::host_preorder_create::has_preorder_model_bit(
            world.host_object(id).unwrap().model_condition_bits
        )
    );
    assert_eq!(
        world.get_player(0).unwrap().statistics.resources_spent(),
        300
    );
    assert_eq!(world.supply_create_center_registers, 1);
    world.notify_structure_construction_complete(id);
    assert_eq!(
        world.get_player(0).unwrap().statistics.resources_spent(),
        300
    );
    assert_eq!(world.supply_create_center_registers, 1);
}

#[test]
fn instant_build_player_completion_admits_power_and_owned_ai_synchronously() {
    let (mut world, builder) = construction_world(1000, 7);
    let expected = ObjectId(builder.0 + 1);
    prime_ai_completion(&mut world, expected);
    world.frame = 8;
    let id = world
        .create_instant_line_build_object_for_player(
            "OwnedScaffold",
            0,
            Vec3::new(40.0, 0.0, 50.0),
            0.4,
            builder,
        )
        .unwrap();
    assert_eq!(id, expected);
    assert_owned_completion(&world, id);
    assert!(world.host_object(id).unwrap().status.under_construction);
    assert!(
        crate::game_logic::host_preorder_create::has_preorder_model_bit(
            world.host_object(id).unwrap().model_condition_bits
        )
    );
    assert_eq!(world.get_player(0).unwrap().effective_supplies(), 1000);
    let admitted_power = world.get_player(0).unwrap().power_available;
    world.update_player_resources(0.0);
    assert_eq!(world.get_player(0).unwrap().power_available, admitted_power);
    let engine = gamelogic::scripting::engine::ScriptEngine::new().unwrap();
    world
        .update_local_script_engine_with_owner(&engine)
        .unwrap();
    assert_eq!(engine.get_frame_object_count_changed(), 8);
    let (mut foreign, _) = construction_world(400, 2);
    foreign
        .update_local_script_engine_with_owner(&engine)
        .unwrap();
    assert_eq!(engine.get_frame_object_count_changed(), 0);
    world
        .update_local_script_engine_with_owner(&engine)
        .unwrap();
    assert_eq!(engine.get_frame_object_count_changed(), 8);
}

#[test]
fn structure_player_completion_replaces_only_its_owned_footprint() {
    let (mut world, builder) = construction_world(1000, 7);
    let old_pos = Vec3::new(40.0, 0.0, 40.0);
    let new_pos = Vec3::new(150.0, 0.0, 150.0);
    let id = world
        .create_dozer_scaffold(
            "OwnedScaffold",
            0,
            builder,
            old_pos,
            0.0,
            Resources {
                supplies: 300,
                power: 0,
            },
        )
        .unwrap();
    let old_cell = world.pathfinding_system.grid.world_to_grid(old_pos);
    let new_cell = world.pathfinding_system.grid.world_to_grid(new_pos);
    let foreign_cell = crate::game_logic::pathfinding::GridPos::new(old_cell.x + 6, old_cell.y);
    world.pathfinding_system.grid.set_cell_obstacle_owned(
        foreign_cell,
        false,
        false,
        999,
        Some(0),
        Some(Team::USA),
    );
    assert!(world.pathfinding_system.grid.is_static_blocked(old_cell));
    let (object, fixture_partition_object) = world.host_object_and_partition_mut(id).unwrap();
    object.set_position_with_partition_owner(
        new_pos,
        0,
        &mut Default::default(),
        fixture_partition_object,
    );
    object.set_status_under_construction(false);
    object.construction_percent = 1.0;
    world.complete_owned_structure_player(id, 0, false);
    assert!(!world.pathfinding_system.grid.is_static_blocked(old_cell));
    assert!(world.pathfinding_system.grid.is_static_blocked(new_cell));
    assert!(
        world
            .pathfinding_system
            .grid
            .is_static_blocked(foreign_cell)
    );
}

fn attached_completion_engine() -> gamelogic::scripting::engine::ScriptEngine {
    use gamelogic::scripting::core::{
        Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
        ScriptActionType, ScriptList,
    };
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut outer = Script::new();
    outer.script_name = "AuthoredCompletion".into();
    outer.is_subroutine = true;
    outer.is_one_shot = false;
    outer.condition = Some(Box::new(condition.clone()));
    let mut call = ScriptAction::new(ScriptActionType::CallSubroutine);
    call.add_parameter(Parameter::with_string(
        ParameterType::Script,
        "NestedCompletion".into(),
    ))
    .unwrap();
    outer.action = Some(Box::new(call));
    let mut nested = Script::new();
    nested.script_name = "NestedCompletion".into();
    nested.is_subroutine = true;
    nested.is_one_shot = false;
    nested.condition = Some(Box::new(condition));
    let mut text = ScriptAction::new(ScriptActionType::DisplayText);
    text.add_parameter(Parameter::with_string(
        ParameterType::TextString,
        "attached:done".into(),
    ))
    .unwrap();
    nested.action = Some(Box::new(text));
    let mut list = ScriptList::new();
    list.append_script(Box::new(outer));
    list.append_script(Box::new(nested));
    let engine = gamelogic::scripting::engine::ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    engine
}

#[test]
fn authored_ai_completion_applies_metadata_and_nested_script_to_its_owner() {
    let (mut first, builder) = construction_world(1000, 7);
    first.templates.get_mut("OwnedScaffold").unwrap().dock_kind =
        crate::game_logic::DockKind::SupplyCenter;
    let (mut foreign, foreign_builder) = construction_world(2000, 11);
    let make = |world: &mut GameLogic, builder| {
        world
            .create_dozer_scaffold(
                "OwnedScaffold",
                0,
                builder,
                Vec3::ZERO,
                0.0,
                Resources {
                    supplies: 300,
                    power: 0,
                },
            )
            .unwrap()
    };
    let id = make(&mut first, builder);
    let foreign_id = make(&mut foreign, foreign_builder);
    assert_eq!(id, foreign_id);
    prime_ai_completion(&mut first, id);
    prime_ai_completion(&mut foreign, foreign_id);
    first.install_owned_script_engine(attached_completion_engine());
    foreign.install_owned_script_engine(attached_completion_engine());
    first
        .ai_manager
        .ai_players
        .get_mut(&0)
        .unwrap()
        .building_queue[0]
        .completion = Some(crate::ai::AuthoredStructureCompletion {
        name: "AuthoredStructure".into(),
        script: "AuthoredCompletion".into(),
        health_percent: 25,
        unsellable: true,
    });
    first.frame = 9000;
    first.notify_structure_construction_complete(id);
    let object = first.host_object(id).unwrap();
    assert_eq!(object.name, "AuthoredStructure");
    assert_eq!(object.health.current, 225.0);
    assert!(object.is_script_unsellable());
    assert!(!object.status.under_construction);
    assert_eq!(first.new_script_messages, ["Transmission: attached:done"]);
    assert!(
        first.owned_script_engine.is_some(),
        "interpreter remains callback visible"
    );
    assert!(
        crate::game_logic::host_preorder_create::has_preorder_model_bit(
            object.model_condition_bits
        )
    );
    let pad = &first.ai_manager.ai_players[&0].building_queue[0];
    assert!(pad.supply_building);
    assert_eq!(pad.current_gatherers, -1);
    assert!(pad.desired_gatherers >= 1);
    assert!(
        !foreign
            .host_object(foreign_id)
            .unwrap()
            .is_script_unsellable()
    );
    assert_eq!(foreign.host_object(foreign_id).unwrap().health.current, 1.0);
    assert!(foreign.new_script_messages.is_empty());
    assert_eq!(foreign.ai_manager.ai_players[&0].next_building_time, 700.0);
    first.notify_structure_construction_complete(id);
    assert_eq!(
        first.new_script_messages.len(),
        1,
        "completion must not replay attachment"
    );
}

#[test]
fn ordinary_completion_records_canonical_academy_exactly_once() {
    let (mut world, builder) = construction_world(1000, 7);
    world
        .templates
        .get_mut("OwnedScaffold")
        .unwrap()
        .add_kind_of(KindOf::FSSupplyCenter)
        .add_kind_of(KindOf::FSBarracks);
    let id = world
        .create_dozer_scaffold(
            "OwnedScaffold",
            0,
            builder,
            Vec3::ZERO,
            0.0,
            Resources {
                supplies: 300,
                power: 0,
            },
        )
        .unwrap();
    world.frame = 9000;
    assert!(
        !world
            .get_player(0)
            .unwrap()
            .academy_stats
            .had_a_supply_center()
    );
    world.notify_structure_construction_complete(id);
    assert!(
        world
            .get_player(0)
            .unwrap()
            .academy_stats
            .had_a_supply_center()
    );
    let expected = world.get_player(0).unwrap().academy_stats.serialize();
    world.notify_structure_construction_complete(id);
    assert_eq!(
        world.get_player(0).unwrap().academy_stats.serialize(),
        expected
    );
}

#[test]
fn completion_callback_executes_attachment_with_already_borrowed_interpreter() {
    let (mut world, builder) = construction_world(1000, 7);
    let id = world
        .create_dozer_scaffold(
            "OwnedScaffold",
            0,
            builder,
            Vec3::ZERO,
            0.0,
            Resources {
                supplies: 300,
                power: 0,
            },
        )
        .unwrap();
    prime_ai_completion(&mut world, id);
    world.install_owned_script_engine(attached_completion_engine());
    world
        .ai_manager
        .ai_players
        .get_mut(&0)
        .unwrap()
        .building_queue[0]
        .completion = Some(crate::ai::AuthoredStructureCompletion {
        name: "BorrowedCompletion".into(),
        script: "AuthoredCompletion".into(),
        health_percent: 75,
        unsellable: true,
    });
    world
        .with_owned_script_engine(|engine, world| {
            assert!(world.owned_script_engine.is_none());
            world.notify_structure_construction_complete_with_interpreter(id, engine);
            assert_eq!(world.new_script_messages, ["Transmission: attached:done"]);
            assert_eq!(world.host_object(id).unwrap().health.current, 675.0);
        })
        .unwrap();
    assert!(world.owned_script_engine.is_some());
    world.notify_structure_construction_complete(id);
    assert_eq!(world.new_script_messages.len(), 1);
}

#[test]
fn rebuilt_structure_callback_rebinds_only_exact_owned_hole_successor() {
    let (mut world, builder) = construction_world(1000, 7);
    let structure = world
        .create_dozer_scaffold(
            "OwnedScaffold",
            0,
            builder,
            Vec3::ZERO,
            0.0,
            Resources {
                supplies: 300,
                power: 0,
            },
        )
        .unwrap();
    let hole = world
        .create_object_for_player("OwnedDozer", 0, Vec3::ZERO)
        .unwrap();
    let unrelated_hole = world
        .create_object_for_player("OwnedDozer", 0, Vec3::ZERO)
        .unwrap();
    world.host_object_mut(hole).unwrap().is_rebuild_hole = true;
    world
        .host_object_mut(hole)
        .unwrap()
        .rebuild_reconstructing_id = Some(structure);
    world
        .host_object_mut(unrelated_hole)
        .unwrap()
        .is_rebuild_hole = true;
    prime_ai_completion(&mut world, structure);
    let ai = world.ai_manager.ai_players.get_mut(&0).unwrap();
    let mut unrelated = ai.building_queue[0].clone();
    unrelated.object_id = Some(unrelated_hole);
    ai.building_queue[0].object_id = Some(hole);
    ai.building_queue[0].completion = Some(crate::ai::AuthoredStructureCompletion {
        name: "MustNotApply".into(),
        script: "AuthoredCompletion".into(),
        health_percent: 25,
        unsellable: true,
    });
    ai.building_queue.insert(0, unrelated);
    world.install_owned_script_engine(attached_completion_engine());
    world.notify_owned_ai_structure_produced(structure);
    let ai = &world.ai_manager.ai_players[&0];
    assert_eq!(ai.building_queue[0].object_id, Some(unrelated_hole));
    assert_eq!(ai.building_queue[1].object_id, Some(structure));
    assert_eq!(ai.next_building_time, 0.0);
    assert_eq!(ai.next_team_queue_time, 0.0);
    assert_ne!(world.host_object(structure).unwrap().name, "MustNotApply");
    assert_eq!(world.host_object(structure).unwrap().health.current, 1.0);
    assert!(
        world
            .host_object(structure)
            .unwrap()
            .status
            .under_construction
    );
    assert!(world.new_script_messages.is_empty());
}
