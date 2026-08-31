use super::*;
use crate::game_logic::{GameLogic, Object, ObjectType};
use game_engine::common::global_data::with_global_data_restored;

#[test]
fn legacy_weapon_command_without_shot_field_uses_cxx_no_max_default() {
    let original = CommandType::DoWeapon {
        weapon_slot: WeaponSlot::Primary,
        max_shots_to_fire: i32::MAX,
        target: WeaponTarget::Location(Vec3::ZERO),
    };
    let mut encoded = serde_json::to_value(&original).expect("serialize weapon command");
    encoded["DoWeapon"]
        .as_object_mut()
        .expect("DoWeapon payload")
        .remove("max_shots_to_fire");

    let decoded: CommandType = serde_json::from_value(encoded).expect("deserialize legacy command");
    assert_eq!(decoded, original);
}

#[test]
fn test_command_creation() {
    let mut system = CommandSystem::new();
    let context = MouseCommandContext {
        world_position: Vec3::new(100.0, 0.0, 100.0),
        target_object: None,
        target_presentation: None,
        selected_presentation: Vec::new(),
        presentation_box_select_units: Vec::new(),
        presentation_select_similar_units: Vec::new(),
        screen_position: Vec2::new(400.0, 300.0),
        viewport_size: None,
        world_min: None,
        world_max: None,
        mouse_button: MouseButton::Right,
        modifier_keys: ModifierKeys::default(),
        is_drag: false,
        drag_start: None,
        drag_end: None,
        drag_start_world: None,
        drag_end_world: None,
    };

    let game_logic = GameLogic::new();
    let selected_units = vec![ObjectId(1)];

    if let Some(command) =
        system.process_mouse_input(&context, &selected_units, 0, Some(&game_logic))
    {
        match command.command_type {
            CommandType::MoveTo { destination, .. } => {
                assert_eq!(destination, Vec3::new(100.0, 0.0, 100.0));
            }
            _ => panic!("Expected MoveTo command"),
        }
    } else {
        panic!("Expected command to be created");
    }
}

#[test]
fn boot_gather_classification_requires_authored_supply_source_not_basename() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let system = CommandSystem::new();
    let mut game_logic = GameLogic::new();

    let mut harvester_template = ThingTemplate::new("RetailSupplyTruck");
    harvester_template
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Harvester)
        .set_health(100.0);
    game_logic.add_object(Object::new(harvester_template, ObjectId(1), Team::USA));

    let mut supply_template = ThingTemplate::new("ArbitraryRetailSupplyIdentity");
    supply_template
        .add_kind_of(KindOf::SupplySource)
        .add_kind_of(KindOf::Resource)
        .add_kind_of(KindOf::Harvestable)
        .set_health(1.0);
    game_logic.add_object(Object::new(supply_template, ObjectId(2), Team::Neutral));

    let mut lookalike_template = ThingTemplate::new("SupplyPileNamedButNotAuthored");
    lookalike_template
        .add_kind_of(KindOf::Structure)
        .set_health(1.0);
    game_logic.add_object(Object::new(lookalike_template, ObjectId(3), Team::Neutral));

    assert!(
        system.can_gather_from_target(&[ObjectId(1)], ObjectId(2), &game_logic),
        "a player-owned HARVESTER must be able to target an authored neutral supply source"
    );
    assert!(
        !system.can_gather_from_target(&[ObjectId(1)], ObjectId(3), &game_logic),
        "a supply-looking template name must not manufacture Gather authority"
    );
}

#[test]
fn test_command_execution() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let system = CommandSystem::new();
    let mut game_logic = GameLogic::new();

    // Create test object using a minimal thing template
    let mut template = ThingTemplate::new("TestUnit");
    template.add_kind_of(KindOf::Vehicle);
    template.set_health(100.0);

    let mut obj = Object::new(template, ObjectId(1), Team::USA);
    obj.position = Vec3::new(0.0, 0.0, 0.0);
    game_logic.add_object(obj);

    let command = GameCommand {
        command_type: CommandType::MoveTo {
            destination: Vec3::new(50.0, 0.0, 50.0),
            waypoints: Vec::new(),
        },
        player_id: 0,
        command_id: 1,
        timestamp: SystemTime::now(),
        selected_units: vec![ObjectId(1)],
        modifier_keys: ModifierKeys::default(),
    };

    let result = system.execute_command(&command, &mut game_logic);
    assert_eq!(result, CommandResult::Success);
}

#[test]
fn right_click_heal_pad_issues_get_healed() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut system = CommandSystem::new();
    let mut game_logic = GameLogic::new();

    let mut infantry_template = ThingTemplate::new("TestInfantry");
    infantry_template
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    let mut infantry = Object::new(infantry_template, ObjectId(1), Team::USA);
    // Damage authority freezes mid-frame HP on take_damage; set current directly
    // so is_damaged() is observable without a shadow writeback session.
    infantry.health.current = (infantry.health.maximum - 25.0).max(1.0);
    game_logic.add_object(infantry);

    let mut heal_pad_template = ThingTemplate::new("TestHealPad");
    heal_pad_template
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::HealPad)
        .add_kind_of(KindOf::Selectable)
        .set_health(900.0);
    let heal_pad = Object::new(heal_pad_template, ObjectId(2), Team::USA);
    game_logic.add_object(heal_pad);

    let context = MouseCommandContext {
        world_position: Vec3::new(0.0, 0.0, 0.0),
        target_object: Some(ObjectId(2)),
        target_presentation: None,
        selected_presentation: Vec::new(),
        presentation_box_select_units: Vec::new(),
        presentation_select_similar_units: Vec::new(),
        screen_position: Vec2::new(0.0, 0.0),
        viewport_size: None,
        world_min: None,
        world_max: None,
        mouse_button: MouseButton::Right,
        modifier_keys: ModifierKeys::default(),
        is_drag: false,
        drag_start: None,
        drag_end: None,
        drag_start_world: None,
        drag_end_world: None,
    };

    let command = system
        .process_mouse_input(&context, &[ObjectId(1)], 0, Some(&game_logic))
        .expect("right click should generate a command");
    assert!(
        matches!(
            command.command_type,
            CommandType::GetHealed {
                target_id: ObjectId(2)
            }
        ),
        "heal pad target should issue GetHealed"
    );
}

#[test]
fn right_click_repair_pad_issues_get_repaired() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut system = CommandSystem::new();
    let mut game_logic = GameLogic::new();

    let mut vehicle_template = ThingTemplate::new("TestTank");
    vehicle_template
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(250.0);
    let mut vehicle = Object::new(vehicle_template, ObjectId(10), Team::USA);
    // Damage authority freezes mid-frame HP on take_damage; set current directly
    // so is_damaged() is observable without a shadow writeback session.
    vehicle.health.current = (vehicle.health.maximum - 30.0).max(1.0);
    game_logic.add_object(vehicle);

    let mut repair_pad_template = ThingTemplate::new("TestRepairPad");
    repair_pad_template
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::RepairPad)
        .add_kind_of(KindOf::Selectable)
        .set_health(1000.0);
    let repair_pad = Object::new(repair_pad_template, ObjectId(11), Team::USA);
    game_logic.add_object(repair_pad);

    let context = MouseCommandContext {
        world_position: Vec3::new(0.0, 0.0, 0.0),
        target_object: Some(ObjectId(11)),
        target_presentation: None,
        selected_presentation: Vec::new(),
        presentation_box_select_units: Vec::new(),
        presentation_select_similar_units: Vec::new(),
        screen_position: Vec2::new(0.0, 0.0),
        viewport_size: None,
        world_min: None,
        world_max: None,
        mouse_button: MouseButton::Right,
        modifier_keys: ModifierKeys::default(),
        is_drag: false,
        drag_start: None,
        drag_end: None,
        drag_start_world: None,
        drag_end_world: None,
    };

    let command = system
        .process_mouse_input(&context, &[ObjectId(10)], 0, Some(&game_logic))
        .expect("right click should generate a command");
    assert!(
        matches!(
            command.command_type,
            CommandType::GetRepaired {
                target_id: ObjectId(11)
            }
        ),
        "repair pad target should issue GetRepaired"
    );
}

#[test]
fn right_click_service_uses_authored_kindof_not_service_shaped_names() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut system = CommandSystem::new();
    let mut game_logic = GameLogic::new();

    let mut vehicle_template = ThingTemplate::new("RetailGroundVehicle");
    vehicle_template
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(250.0);
    let mut vehicle = Object::new(vehicle_template, ObjectId(40), Team::USA);
    vehicle.health.current = 100.0;
    game_logic.add_object(vehicle);

    let mut infantry_template = ThingTemplate::new("RetailInfantry");
    infantry_template
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    let mut infantry = Object::new(infantry_template, ObjectId(41), Team::USA);
    infantry.health.current = 50.0;
    game_logic.add_object(infantry);

    // These names intentionally deny every legacy string heuristic.  The
    // parsed-equivalent KindOf is the only service authority.
    let mut repair_pad_template = ThingTemplate::new("SourceTaggedGroundService");
    repair_pad_template
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::RepairPad)
        .set_health(1_000.0);
    game_logic.add_object(Object::new(repair_pad_template, ObjectId(42), Team::USA));
    let mut heal_pad_template = ThingTemplate::new("SourceTaggedMedicalService");
    heal_pad_template
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::HealPad)
        .set_health(1_000.0);
    game_logic.add_object(Object::new(heal_pad_template, ObjectId(43), Team::USA));

    // These names would previously fabricate a BuildingType/provider even
    // though the retail KindOf source says nothing about a service role.
    let mut fake_repair_template = ThingTemplate::new("RepairHospitalAirfieldWithoutTag");
    fake_repair_template
        .add_kind_of(KindOf::Structure)
        .set_health(1_000.0);
    game_logic.add_object(Object::new(fake_repair_template, ObjectId(44), Team::USA));
    let mut fake_heal_template = ThingTemplate::new("HospitalMedicWithoutTag");
    fake_heal_template
        .add_kind_of(KindOf::Structure)
        .set_health(1_000.0);
    game_logic.add_object(Object::new(fake_heal_template, ObjectId(45), Team::USA));

    let context_for = |target_object| MouseCommandContext {
        world_position: Vec3::ZERO,
        target_object: Some(target_object),
        target_presentation: None,
        selected_presentation: Vec::new(),
        presentation_box_select_units: Vec::new(),
        presentation_select_similar_units: Vec::new(),
        screen_position: Vec2::ZERO,
        viewport_size: None,
        world_min: None,
        world_max: None,
        mouse_button: MouseButton::Right,
        modifier_keys: ModifierKeys::default(),
        is_drag: false,
        drag_start: None,
        drag_end: None,
        drag_start_world: None,
        drag_end_world: None,
    };

    let repair = system
        .process_mouse_input(
            &context_for(ObjectId(42)),
            &[ObjectId(40)],
            0,
            Some(&game_logic),
        )
        .expect("RMB command at authored repair pad");
    assert!(matches!(
        repair.command_type,
        CommandType::GetRepaired { .. }
    ));
    let reject_repair = system
        .process_mouse_input(
            &context_for(ObjectId(44)),
            &[ObjectId(40)],
            0,
            Some(&game_logic),
        )
        .expect("RMB command at untagged name-only target");
    assert!(matches!(
        reject_repair.command_type,
        CommandType::MoveTo { .. }
    ));

    let heal = system
        .process_mouse_input(
            &context_for(ObjectId(43)),
            &[ObjectId(41)],
            0,
            Some(&game_logic),
        )
        .expect("RMB command at authored heal pad");
    assert!(matches!(heal.command_type, CommandType::GetHealed { .. }));
    let reject_heal = system
        .process_mouse_input(
            &context_for(ObjectId(45)),
            &[ObjectId(41)],
            0,
            Some(&game_logic),
        )
        .expect("RMB command at untagged medical-looking target");
    assert!(matches!(
        reject_heal.command_type,
        CommandType::MoveTo { .. }
    ));
}

#[test]
fn construct_and_structure_repair_require_authored_dozer_kindof() {
    use crate::command_system::CommandableObject;
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut name_only = ThingTemplate::new("DozerWorkerConstructionNamedOnly");
    name_only.add_kind_of(KindOf::Vehicle).set_health(100.0);
    let name_only = Object::new(name_only, ObjectId(50), Team::USA);
    assert!(
        !CommandableObject::can_construct(&name_only) && !CommandableObject::can_repair(&name_only),
        "name-only worker/dozer spelling must not grant C++ DOZER authority"
    );

    let mut authored = ThingTemplate::new("SourceTaggedBuilder");
    authored
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Dozer)
        .set_health(100.0);
    let authored = Object::new(authored, ObjectId(51), Team::USA);
    assert!(
        CommandableObject::can_construct(&authored) && CommandableObject::can_repair(&authored),
        "an arbitrary basename with authored DOZER must retain C++ authority"
    );
}

#[test]
fn drag_selection_prefers_world_drag_bounds_when_provided() {
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

    let mut system = CommandSystem::new();
    let mut game_logic = GameLogic::new();
    game_logic.add_player(Player::new(0, Team::USA, "TestPlayer", true));

    let mut template = ThingTemplate::new("TestUnit");
    template
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);

    let mut near = Object::new(template.clone(), ObjectId(31), Team::USA);
    near.set_position(Vec3::new(10.0, 0.0, 10.0));
    // C++ drag select is owner-scoped (InGameUI picks the local player's
    // objects); the exact-owner bounds probe needs the fixture stamped.
    near.owner_player_id = Some(0);
    game_logic.add_object(near);

    let mut far = Object::new(template, ObjectId(32), Team::USA);
    far.set_position(Vec3::new(120.0, 0.0, 120.0));
    far.owner_player_id = Some(0);
    game_logic.add_object(far);

    let context = MouseCommandContext {
        world_position: Vec3::new(0.0, 0.0, 0.0),
        target_object: None,
        target_presentation: None,
        selected_presentation: Vec::new(),
        presentation_box_select_units: Vec::new(),
        presentation_select_similar_units: Vec::new(),
        screen_position: Vec2::new(0.0, 0.0),
        viewport_size: Some(Vec2::new(1024.0, 768.0)),
        world_min: Some(Vec3::new(-256.0, 0.0, -256.0)),
        world_max: Some(Vec3::new(256.0, 0.0, 256.0)),
        mouse_button: MouseButton::Left,
        modifier_keys: ModifierKeys::default(),
        is_drag: true,
        drag_start: Some(Vec2::new(999.0, 999.0)),
        drag_end: Some(Vec2::new(1000.0, 1000.0)),
        drag_start_world: Some(Vec3::new(0.0, 0.0, 0.0)),
        drag_end_world: Some(Vec3::new(50.0, 0.0, 50.0)),
    };

    let command = system
        .process_mouse_input(&context, &[], 0, Some(&game_logic))
        .expect("drag selection should produce command");

    match command.command_type {
        CommandType::CreateSelectedGroup { units, .. } => {
            assert!(units.contains(&ObjectId(31)));
            assert!(!units.contains(&ObjectId(32)));
        }
        other => panic!("expected drag CreateSelectedGroup command, got {other:?}"),
    }
}

#[test]
fn queue_upgrade_deducts_once_per_team_and_prevents_duplicate_queue() {
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

    let system = CommandSystem::new();
    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 5000;
    game_logic.add_player(player);

    let mut template = ThingTemplate::new("AmericaSupplyCenter");
    template
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);

    let producer_a = Object::new(template.clone(), ObjectId(201), Team::USA);
    let producer_b = Object::new(template, ObjectId(202), Team::USA);
    game_logic.add_object(producer_a);
    game_logic.add_object(producer_b);

    let queue_command = GameCommand {
        command_type: CommandType::QueueUpgrade {
            upgrade_name: "Upgrade_AmericaSupplyLines".to_string(),
        },
        player_id: 0,
        command_id: 1,
        timestamp: SystemTime::now(),
        selected_units: vec![ObjectId(201), ObjectId(202)],
        modifier_keys: ModifierKeys::default(),
    };

    let first_result = system.execute_command(&queue_command, &mut game_logic);
    assert_eq!(first_result, CommandResult::Success);

    let player_after_first = game_logic.get_player(0).expect("player should exist");
    assert_eq!(
        player_after_first.effective_supplies(),
        4200,
        "upgrade cost should be charged once per team, not per selected unit (retail SupplyLines=800)"
    );
    assert!(
        player_after_first
            .queued_upgrades
            .contains("Upgrade_AmericaSupplyLines")
    );

    let second_result = system.execute_command(&queue_command, &mut game_logic);
    assert_eq!(second_result, CommandResult::InvalidCommand);
}

#[test]
fn queue_upgrade_identity_matches_ini_name_variants() {
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

    let system = CommandSystem::new();
    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 5000;
    game_logic.add_player(player);

    let mut template = ThingTemplate::new("AmericaSupplyCenter");
    template
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    game_logic.add_object(Object::new(template, ObjectId(251), Team::USA));

    let queue_command = GameCommand {
        command_type: CommandType::QueueUpgrade {
            upgrade_name: "Upgrade_AmericaSupplyLines".to_string(),
        },
        player_id: 0,
        command_id: 30,
        timestamp: SystemTime::now(),
        selected_units: vec![ObjectId(251)],
        modifier_keys: ModifierKeys::default(),
    };
    assert_eq!(
        system.execute_command(&queue_command, &mut game_logic),
        CommandResult::Success
    );

    let variant_command = GameCommand {
        command_type: CommandType::QueueUpgrade {
            upgrade_name: "upgradeamericasupplylines".to_string(),
        },
        player_id: 0,
        command_id: 31,
        timestamp: SystemTime::now(),
        selected_units: vec![ObjectId(251)],
        modifier_keys: ModifierKeys::default(),
    };
    assert_eq!(
        system.execute_command(&variant_command, &mut game_logic),
        CommandResult::InvalidCommand,
        "same upgrade should not be charged twice when naming style differs"
    );

    let cancel_variant = GameCommand {
        command_type: CommandType::CancelUpgrade {
            upgrade_name: "UPGRADE_AMERICA_SUPPLY_LINES".to_string(),
        },
        player_id: 0,
        command_id: 32,
        timestamp: SystemTime::now(),
        selected_units: vec![ObjectId(251)],
        modifier_keys: ModifierKeys::default(),
    };
    assert_eq!(
        system.execute_command(&cancel_variant, &mut game_logic),
        CommandResult::Success,
        "cancel should find the queued upgrade by normalized INI identity"
    );

    let player = game_logic.get_player(0).expect("player should exist");
    assert_eq!(player.effective_supplies(), 5000);
    assert!(player.queued_upgrades.is_empty());
}

#[test]
fn purchase_science_identity_matches_command_name_variants() {
    use crate::game_logic::{Player, Team};

    let system = CommandSystem::new();
    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 3000;
    // C++ residual: science purchase points, not supplies.
    player.science_purchase_points = 2;
    player.unlocked_sciences.insert("SCIENCE_AMERICA".into());
    player.unlocked_sciences.insert("SCIENCE_Rank1".into());
    game_logic.add_player(player);

    let purchase_command = GameCommand {
        command_type: CommandType::PurchaseScience {
            science_name: "PaladinTank".to_string(),
        },
        player_id: 0,
        command_id: 40,
        timestamp: SystemTime::now(),
        selected_units: Vec::new(),
        modifier_keys: ModifierKeys::default(),
    };
    assert_eq!(
        system.execute_command(&purchase_command, &mut game_logic),
        CommandResult::Success
    );

    let variant_command = GameCommand {
        command_type: CommandType::PurchaseScience {
            science_name: "paladin_tank".to_string(),
        },
        player_id: 0,
        command_id: 41,
        timestamp: SystemTime::now(),
        selected_units: Vec::new(),
        modifier_keys: ModifierKeys::default(),
    };
    assert_eq!(
        system.execute_command(&variant_command, &mut game_logic),
        CommandResult::InvalidCommand,
        "same science should not be charged twice when naming style differs"
    );

    let player = game_logic.get_player(0).expect("player should exist");
    assert_eq!(
        player.effective_supplies(),
        3000,
        "science purchase must not spend supplies residual"
    );
    assert_eq!(
        player.science_purchase_points, 1,
        "one point spent residual"
    );
    assert!(
        player.has_unlocked_science("SCIENCE_PaladinTank"),
        "canonical Paladin science residual (Science.ini Rank1)"
    );
}

#[test]
fn sell_refunds_queued_production() {
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

    with_global_data_restored(|| {
        game_engine::common::global_data::write().sell_percentage = 0.5;

        let system = CommandSystem::new();
        let mut game_logic = GameLogic::new();
        let mut player = Player::new(0, Team::USA, "USA", true);
        player.resources.supplies = 1_000;
        game_logic.add_player(player);

        let mut barracks = ThingTemplate::new("TestBarracks");
        barracks
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .set_health(1_000.0)
            .set_cost(1_000, -1);
        game_logic
            .templates
            .insert("TestBarracks".to_string(), barracks);

        let mut infantry = ThingTemplate::new("TestInfantry");
        infantry
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0)
            .set_cost(100, 0);
        game_logic
            .templates
            .insert("TestInfantry".to_string(), infantry);

        let barracks_id = game_logic
            .create_object("TestBarracks", Team::USA, Vec3::ZERO)
            .expect("barracks should be created");

        let queue_command = GameCommand {
            command_type: CommandType::QueueUnitCreate {
                template_name: "TestInfantry".to_string(),
                quantity: 1,
            },
            player_id: 0,
            command_id: 50,
            timestamp: SystemTime::now(),
            selected_units: vec![barracks_id],
            modifier_keys: ModifierKeys::default(),
        };
        assert_eq!(
            system.execute_command(&queue_command, &mut game_logic),
            CommandResult::Success
        );
        assert_eq!(
            game_logic.get_player(0).unwrap().effective_supplies(),
            900,
            "queued unit should charge before selling"
        );

        let sell_command = GameCommand {
            command_type: CommandType::Sell {
                object_id: barracks_id,
            },
            player_id: 0,
            command_id: 51,
            timestamp: SystemTime::now(),
            selected_units: vec![barracks_id],
            modifier_keys: ModifierKeys::default(),
        };
        assert_eq!(
            system.execute_command(&sell_command, &mut game_logic),
            CommandResult::Success
        );

        // C++ BuildAssistant::sellObject cancels production at sell start;
        // structure refund deposits when sell finishes (~90 frames).
        assert_eq!(
            game_logic.get_player(0).unwrap().effective_supplies(),
            1_000,
            "sell start should refund queued production immediately"
        );
        assert!(
            game_logic
                .host_object(barracks_id)
                .map(|object| object.status.sold)
                .unwrap_or(false),
            "sell start should mark structure sold residual"
        );
        assert!(
            game_logic
                .host_object(barracks_id)
                .and_then(|object| object.building_data.as_ref())
                .map(|building| building.production_queue.is_empty())
                .unwrap_or(true),
            "sell should drain queued production at sell start"
        );

        // Advance multi-frame sell residual to completion.
        for step in 1..=200u64 {
            game_logic.set_current_frame(step);
            game_logic.update_sell_list();
            game_logic.process_destroy_list();
            if game_logic.host_object(barracks_id).is_none() {
                break;
            }
        }
        assert!(
            game_logic.host_object(barracks_id).is_none(),
            "sell finish should destroy structure"
        );
        assert_eq!(
            game_logic.get_player(0).unwrap().effective_supplies(),
            1_500,
            "selling should refund both the structure sell value and queued production"
        );
    });
}

#[test]
fn sell_refund_uses_global_sell_percentage() {
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

    with_global_data_restored(|| {
        game_engine::common::global_data::write().sell_percentage = 0.25;

        let system = CommandSystem::new();
        let mut game_logic = GameLogic::new();
        let mut player = Player::new(0, Team::USA, "USA", true);
        player.resources.supplies = 0;
        game_logic.add_player(player);

        let mut barracks = ThingTemplate::new("TestBarracks");
        barracks
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .set_health(1_000.0)
            .set_cost(1_000, -1);
        game_logic
            .templates
            .insert("TestBarracks".to_string(), barracks);

        let barracks_id = game_logic
            .create_object("TestBarracks", Team::USA, Vec3::ZERO)
            .expect("barracks should be created");

        // Re-assert sell percentage immediately before sell so the production
        // path is proven to consume the live GlobalData value under isolation.
        assert!(
            (game_engine::common::global_data::read().sell_percentage - 0.25).abs() < f32::EPSILON,
            "test isolation must preserve configured SellPercentage"
        );

        let sell_command = GameCommand {
            command_type: CommandType::Sell {
                object_id: barracks_id,
            },
            player_id: 0,
            command_id: 52,
            timestamp: SystemTime::now(),
            selected_units: vec![barracks_id],
            modifier_keys: ModifierKeys::default(),
        };
        assert_eq!(
            system.execute_command(&sell_command, &mut game_logic),
            CommandResult::Success
        );

        // Structure refund deposits at sell finish (C++ BuildAssistant::update).
        for step in 1..=200u64 {
            game_logic.set_current_frame(step);
            game_logic.update_sell_list();
            game_logic.process_destroy_list();
            if game_logic.host_object(barracks_id).is_none() {
                break;
            }
        }
        assert!(
            game_logic.host_object(barracks_id).is_none(),
            "sell finish should destroy structure"
        );
        assert_eq!(
            game_logic.get_player(0).unwrap().effective_supplies(),
            250,
            "sell refund should use GlobalData SellPercentage (effective under economy auth)"
        );
    });
}

#[test]
fn cancel_construction_refunds_full_build_cost() {
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

    let system = CommandSystem::new();
    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 0;
    game_logic.add_player(player);

    let mut barracks = ThingTemplate::new("TestBarracks");
    barracks
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1_000.0)
        .set_cost(1_000, -1);
    game_logic
        .templates
        .insert("TestBarracks".to_string(), barracks);

    let barracks_id = game_logic
        .create_object_under_construction("TestBarracks", Team::USA, Vec3::ZERO)
        .expect("under-construction barracks should be created");

    let cancel_command = GameCommand {
        command_type: CommandType::DozerCancelConstruct {
            object_id: barracks_id,
        },
        player_id: 0,
        command_id: 60,
        timestamp: SystemTime::now(),
        selected_units: vec![],
        modifier_keys: ModifierKeys::default(),
    };

    assert_eq!(
        system.execute_command(&cancel_command, &mut game_logic),
        CommandResult::Success
    );
    game_logic.update();

    assert!(
        game_logic.host_object(barracks_id).is_none(),
        "cancelled construction should be destroyed"
    );
    assert_eq!(
        game_logic.get_player(0).unwrap().effective_supplies(),
        1_000,
        "C++ dozer cancel refunds the full build cost"
    );
}

#[test]
fn cancel_construction_rejects_enemy_structure() {
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

    let system = CommandSystem::new();
    let mut game_logic = GameLogic::new();
    let mut usa = Player::new(0, Team::USA, "USA", true);
    usa.resources.supplies = 0;
    game_logic.add_player(usa);
    let mut gla = Player::new(2, Team::GLA, "GLA", false);
    gla.resources.supplies = 0;
    game_logic.add_player(gla);

    let mut barracks = ThingTemplate::new("TestBarracks");
    barracks
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1_000.0)
        .set_cost(1_000, -1);
    game_logic
        .templates
        .insert("TestBarracks".to_string(), barracks);

    let barracks_id = game_logic
        .create_object_under_construction("TestBarracks", Team::USA, Vec3::ZERO)
        .expect("under-construction barracks should be created");

    let cancel_command = GameCommand {
        command_type: CommandType::DozerCancelConstruct {
            object_id: barracks_id,
        },
        player_id: 2,
        command_id: 61,
        timestamp: SystemTime::now(),
        selected_units: vec![],
        modifier_keys: ModifierKeys::default(),
    };

    assert_eq!(
        system.execute_command(&cancel_command, &mut game_logic),
        CommandResult::InvalidTarget
    );
    game_logic.update();

    assert!(
        game_logic.host_object(barracks_id).is_some(),
        "enemy cancel command must not destroy the target"
    );
    assert_eq!(
        game_logic.get_player(2).unwrap().effective_supplies(),
        0,
        "enemy cancel command must not refund the issuing player"
    );
}

#[test]
fn right_click_ctrl_force_attacks_object_residual() {
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA", true));

    let mut ranger_t = ThingTemplate::new("AmericaInfantryRanger");
    ranger_t
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger_t);
    let mut rebel_t = ThingTemplate::new("GLAInfantryRebel");
    rebel_t
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic.templates.insert("GLAInfantryRebel".into(), rebel_t);

    let attacker = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("attacker");
    let target = logic
        .create_object(
            "GLAInfantryRebel",
            Team::GLA,
            glam::Vec3::new(50.0, 0.0, 0.0),
        )
        .expect("target");

    let ctx = MouseCommandContext {
        world_position: glam::Vec3::new(50.0, 0.0, 0.0),
        target_object: Some(target),
        target_presentation: None,
        selected_presentation: Vec::new(),
        presentation_box_select_units: Vec::new(),
        presentation_select_similar_units: Vec::new(),
        screen_position: glam::Vec2::ZERO,
        viewport_size: None,
        world_min: None,
        world_max: None,
        mouse_button: MouseButton::Right,
        modifier_keys: ModifierKeys {
            ctrl: true,
            shift: false,
            alt: false,
        },
        is_drag: false,
        drag_start: None,
        drag_end: None,
        drag_start_world: None,
        drag_end_world: None,
    };
    let mut sys = CommandSystem::new();
    let cmd = sys
        .process_mouse_input(&ctx, &[attacker], 0, Some(&logic))
        .expect("ctrl RMB should produce command");
    match cmd.command_type {
        CommandType::ForceAttackObject { target_id } => assert_eq!(target_id, target),
        other => panic!("expected ForceAttackObject, got {other:?}"),
    }
}

#[test]
fn right_click_ctrl_force_attacks_ground_residual() {
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA", true));

    let mut ranger_t = ThingTemplate::new("AmericaInfantryRanger");
    ranger_t
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger_t);

    let attacker = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("attacker");

    let loc = glam::Vec3::new(80.0, 0.0, 40.0);
    let ctx = MouseCommandContext {
        world_position: loc,
        target_object: None,
        target_presentation: None,
        selected_presentation: Vec::new(),
        presentation_box_select_units: Vec::new(),
        presentation_select_similar_units: Vec::new(),
        screen_position: glam::Vec2::ZERO,
        viewport_size: None,
        world_min: None,
        world_max: None,
        mouse_button: MouseButton::Right,
        modifier_keys: ModifierKeys {
            ctrl: true,
            shift: false,
            alt: false,
        },
        is_drag: false,
        drag_start: None,
        drag_end: None,
        drag_start_world: None,
        drag_end_world: None,
    };
    let mut sys = CommandSystem::new();
    let cmd = sys
        .process_mouse_input(&ctx, &[attacker], 0, Some(&logic))
        .expect("ctrl RMB ground should produce command");
    match cmd.command_type {
        CommandType::ForceAttackGround { location } => {
            assert!((location - loc).length() < 0.1);
        }
        other => panic!("expected ForceAttackGround, got {other:?}"),
    }
}

#[test]
fn right_click_waypoint_mode_outranks_ctrl_force_attack() {
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA", true));

    let mut ranger_t = ThingTemplate::new("AmericaInfantryRanger");
    ranger_t
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger_t);
    let mut rebel_t = ThingTemplate::new("GLAInfantryRebel");
    rebel_t
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic.templates.insert("GLAInfantryRebel".into(), rebel_t);

    let attacker = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("attacker");
    let target = logic
        .create_object(
            "GLAInfantryRebel",
            Team::GLA,
            glam::Vec3::new(50.0, 0.0, 0.0),
        )
        .expect("target");

    let loc = glam::Vec3::new(50.0, 0.0, 0.0);
    let ctx = MouseCommandContext {
        world_position: loc,
        target_object: Some(target),
        target_presentation: None,
        selected_presentation: Vec::new(),
        presentation_box_select_units: Vec::new(),
        presentation_select_similar_units: Vec::new(),
        screen_position: glam::Vec2::ZERO,
        viewport_size: None,
        world_min: None,
        world_max: None,
        mouse_button: MouseButton::Right,
        modifier_keys: ModifierKeys {
            ctrl: true,
            shift: false,
            alt: true,
        },
        is_drag: false,
        drag_start: None,
        drag_end: None,
        drag_start_world: None,
        drag_end_world: None,
    };
    let mut sys = CommandSystem::new();
    let cmd = sys
        .process_mouse_input(&ctx, &[attacker], 0, Some(&logic))
        .expect("waypoint+ctrl RMB should produce command");
    match cmd.command_type {
        CommandType::AddWaypoint { destination } => {
            assert!((destination - loc).length() < 0.1);
        }
        other => panic!("expected AddWaypoint over ForceAttack, got {other:?}"),
    }

    let ctx_sticky = MouseCommandContext {
        modifier_keys: ModifierKeys {
            ctrl: true,
            shift: false,
            alt: false,
        },
        ..ctx
    };
    sys.set_waypoint_mode_for_player(0, true);
    let cmd = sys
        .process_mouse_input(&ctx_sticky, &[attacker], 0, Some(&logic))
        .expect("sticky waypoint+ctrl should produce command");
    match cmd.command_type {
        CommandType::AddWaypoint { .. } => {}
        other => panic!("sticky waypoint must outrank Ctrl, got {other:?}"),
    }
}

fn right_click_damaged_vehicle_get_repaired_context_residual() {
    use crate::game_logic::{
        KindOf, Player, Team, ThingTemplate,
        buildings::{BuildingData, BuildingType},
    };

    let mut logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    logic.add_player(player);

    let mut tank_t = ThingTemplate::new("AmericaTankCrusader");
    tank_t
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(500.0);
    logic.templates.insert("AmericaTankCrusader".into(), tank_t);
    let mut wf_t = ThingTemplate::new("AmericaWarFactory");
    wf_t.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(2000.0);
    logic.templates.insert("AmericaWarFactory".into(), wf_t);

    let tank = logic
        .create_object(
            "AmericaTankCrusader",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("tank");
    let wf = logic
        .create_object(
            "AmericaWarFactory",
            Team::USA,
            glam::Vec3::new(40.0, 0.0, 0.0),
        )
        .expect("wf");
    if let Some(o) = logic./* Wave 950 */ host_object_mut(tank) {
        o.health.current = 100.0; // damaged residual
    }
    if let Some(o) = logic.host_object_mut(wf) {
        o.building_data = Some(BuildingData::new(BuildingType::WarFactory));
    }

    let ctx = MouseCommandContext {
        world_position: glam::Vec3::new(40.0, 0.0, 0.0),
        target_object: Some(wf),
        target_presentation: None,
        selected_presentation: Vec::new(),
        presentation_box_select_units: Vec::new(),
        presentation_select_similar_units: Vec::new(),
        screen_position: glam::Vec2::ZERO,
        viewport_size: None,
        world_min: None,
        world_max: None,
        mouse_button: MouseButton::Right,
        modifier_keys: ModifierKeys::default(),
        is_drag: false,
        drag_start: None,
        drag_end: None,
        drag_start_world: None,
        drag_end_world: None,
    };
    let mut sys = CommandSystem::new();
    let cmd = sys
        .process_mouse_input(&ctx, &[tank], 0, Some(&logic))
        .expect("context command");
    match cmd.command_type {
        CommandType::GetRepaired { target_id } => assert_eq!(target_id, wf),
        other => panic!("expected GetRepaired, got {other:?}"),
    }
}

#[test]
fn command_type_from_button_name_view_and_formation_residual() {
    use crate::command_system::{CommandType, command_type_from_button_name};
    assert!(matches!(
        command_type_from_button_name("Command_CreateFormation"),
        Some(CommandType::CreateFormation)
    ));
    assert!(matches!(
        command_type_from_button_name("Command_ViewCommandCenter"),
        Some(CommandType::ViewCommandCenter)
    ));
    assert!(matches!(
        command_type_from_button_name("Command_ViewLastRadarEvent"),
        Some(CommandType::ViewLastRadarEvent)
    ));
    assert!(matches!(
        command_type_from_button_name("Command_PlaceBeacon"),
        Some(CommandType::PlaceBeacon { .. })
    ));
    assert!(matches!(
        command_type_from_button_name("Command_RemoveBeacon"),
        Some(CommandType::RemoveBeacon)
    ));
    assert!(matches!(
        command_type_from_button_name("Command_Cheer"),
        Some(CommandType::Cheer)
    ));
    assert!(matches!(
        command_type_from_button_name("Command_Deploy"),
        Some(CommandType::Deploy)
    ));
}

#[test]
fn crate_click_issues_do_salvage_for_salvager_selection() {
    // C++ CommandXlat.cpp:116-149 / 1921-1937 — canSelectionSalvage issues
    // MSG_DO_SALVAGE at the crate position when a KINDOF_SALVAGER is selected.
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::GLA, "GLA", true));

    let mut scav = ThingTemplate::new("GLAVehicleTechnical");
    scav.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Salvager)
        .set_health(200.0);
    logic.templates.insert("GLAVehicleTechnical".into(), scav);
    let mut crate_t = ThingTemplate::new("SalvageCrate");
    crate_t
        .add_kind_of(KindOf::Crate)
        .add_kind_of(KindOf::Selectable)
        .set_health(1.0);
    logic.templates.insert("SalvageCrate".into(), crate_t);

    let salvager = logic
        .create_object_for_player("GLAVehicleTechnical", 0, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("salvager");
    let crate_id = logic
        .create_object(
            "SalvageCrate",
            Team::Neutral,
            glam::Vec3::new(30.0, 0.0, 0.0),
        )
        .expect("crate");
    logic.host_money_crates.register_salvage_crate(crate_id, 50);

    let dest = glam::Vec3::new(30.0, 0.0, 0.0);
    let ctx = MouseCommandContext {
        world_position: dest,
        target_object: Some(crate_id),
        target_presentation: Some(PresentationTargetHint {
            id: crate_id,
            is_alive: true,
            is_structure: false,
            is_resource: false,
            under_construction: false,
            sold: false,
            team: Team::Neutral,
            is_enemy_of_local: false,
            is_neutral: true,
            template_name: "SalvageCrate".into(),
            can_be_entered: false,
            enter_available_capacity: 0,
            enter_uses_transport_slots: false,
            enter_requires_infantry: false,
            enter_forbids_aircraft: false,
            enter_disabled_subdued: false,
            enter_is_rider_change: false,
            rider_change_allowed_templates: Vec::new(),
            is_damaged: false,
            is_friendly_of_local: false,
            provides_vehicle_repair: false,
            provides_aircraft_repair: false,
            provides_heal: false,
            can_provide_service: true,
            dock_kind: crate::game_logic::DockKind::None,
            dock_controller_is_local: false,
            stored_supplies: 0,
            capturable: false,
            immune_to_capture: false,
            capture_garrisonable: false,
            capture_nonstealthed_garrison_count: 0,
            capture_friendly_garrison_count: 0,
            capture_target_effectively_stealthed: false,
            is_crate: true,
            is_salvage_crate: true,
            is_vehicle: false,
            is_aircraft: false,
            is_drone: false,
            is_carbomb: false,
            is_unmanned: false,
            is_mine: false,
        }),
        selected_presentation: vec![PresentationSelectedUnitHint {
            id: salvager,
            is_alive: true,
            is_resource_collector: false,
            is_worker: false,
            can_attack: true,
            can_move: true,
            can_request_service: true,
            can_capture: false,
            template_name: "GLAVehicleTechnical".into(),
            can_repair: false,
            is_damaged: false,
            is_vehicle: true,
            is_aircraft: false,
            is_above_terrain: false,
            is_infantry: false,
            transport_slot_count: 0,
            stored_supplies: 0,
            is_controlled_by_local: true,
            capture_power: crate::game_logic::CapturePowerKind::None,
            capture_power_ready: false,
            is_salvager: true,
            can_override_special_power_destination: false,
        }],
        presentation_box_select_units: Vec::new(),
        presentation_select_similar_units: Vec::new(),
        screen_position: glam::Vec2::ZERO,
        viewport_size: None,
        world_min: None,
        world_max: None,
        mouse_button: MouseButton::Right,
        modifier_keys: ModifierKeys::default(),
        is_drag: false,
        drag_start: None,
        drag_end: None,
        drag_start_world: None,
        drag_end_world: None,
    };
    let mut sys = CommandSystem::new();
    let cmd = sys
        .process_mouse_input(&ctx, &[salvager], 0, Some(&logic))
        .expect("crate salvage command");
    match cmd.command_type {
        CommandType::DoSalvage { destination } => {
            assert!((destination - dest).length() < 0.1);
        }
        other => panic!("expected DoSalvage, got {other:?}"),
    }
}

#[test]
fn ordinary_crate_click_issues_move_to_crate() {
    // C++ crate pickup (non-salvage) is a move-to-crate, not a hard no-op.
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA", true));

    let mut ranger = ThingTemplate::new("AmericaInfantryRanger");
    ranger
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger);
    let mut crate_t = ThingTemplate::new("HealCrate");
    crate_t
        .add_kind_of(KindOf::Crate)
        .add_kind_of(KindOf::Selectable)
        .set_health(1.0);
    logic.templates.insert("HealCrate".into(), crate_t);

    let unit = logic
        .create_object_for_player("AmericaInfantryRanger", 0, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("unit");
    let crate_id = logic
        .create_object("HealCrate", Team::Neutral, glam::Vec3::new(18.0, 0.0, 4.0))
        .expect("crate");
    logic.host_money_crates.register_heal_crate(crate_id);

    let dest = glam::Vec3::new(18.0, 0.0, 4.0);
    let ctx = MouseCommandContext {
        world_position: dest,
        target_object: Some(crate_id),
        target_presentation: Some(PresentationTargetHint {
            id: crate_id,
            is_alive: true,
            is_structure: false,
            is_resource: false,
            under_construction: false,
            sold: false,
            team: Team::Neutral,
            is_enemy_of_local: false,
            is_neutral: true,
            template_name: "HealCrate".into(),
            can_be_entered: false,
            enter_available_capacity: 0,
            enter_uses_transport_slots: false,
            enter_requires_infantry: false,
            enter_forbids_aircraft: false,
            enter_disabled_subdued: false,
            enter_is_rider_change: false,
            rider_change_allowed_templates: Vec::new(),
            is_damaged: false,
            is_friendly_of_local: false,
            provides_vehicle_repair: false,
            provides_aircraft_repair: false,
            provides_heal: false,
            can_provide_service: true,
            dock_kind: crate::game_logic::DockKind::None,
            dock_controller_is_local: false,
            stored_supplies: 0,
            capturable: false,
            immune_to_capture: false,
            capture_garrisonable: false,
            capture_nonstealthed_garrison_count: 0,
            capture_friendly_garrison_count: 0,
            capture_target_effectively_stealthed: false,
            is_crate: true,
            is_salvage_crate: false,
            is_vehicle: false,
            is_aircraft: false,
            is_drone: false,
            is_carbomb: false,
            is_unmanned: false,
            is_mine: false,
        }),
        selected_presentation: vec![PresentationSelectedUnitHint {
            id: unit,
            is_alive: true,
            is_resource_collector: false,
            is_worker: false,
            can_attack: true,
            can_move: true,
            can_request_service: true,
            can_capture: false,
            template_name: "AmericaInfantryRanger".into(),
            can_repair: false,
            is_damaged: false,
            is_vehicle: false,
            is_aircraft: false,
            is_above_terrain: false,
            is_infantry: true,
            transport_slot_count: 1,
            stored_supplies: 0,
            is_controlled_by_local: true,
            capture_power: crate::game_logic::CapturePowerKind::None,
            capture_power_ready: false,
            is_salvager: false,
            can_override_special_power_destination: false,
        }],
        presentation_box_select_units: Vec::new(),
        presentation_select_similar_units: Vec::new(),
        screen_position: glam::Vec2::ZERO,
        viewport_size: None,
        world_min: None,
        world_max: None,
        mouse_button: MouseButton::Right,
        modifier_keys: ModifierKeys::default(),
        is_drag: false,
        drag_start: None,
        drag_end: None,
        drag_start_world: None,
        drag_end_world: None,
    };
    let mut sys = CommandSystem::new();
    let cmd = sys
        .process_mouse_input(&ctx, &[unit], 0, Some(&logic))
        .expect("ordinary crate pickup");
    match cmd.command_type {
        CommandType::MoveTo { destination, .. } => {
            assert!((destination - dest).length() < 0.1);
        }
        other => panic!("expected MoveTo crate, got {other:?}"),
    }
}

fn special_power_button_maps_and_structure_resolves_puc_residual() {
    use crate::command_system::{CommandType, SpecialPowerType, command_type_from_button_name};
    use crate::game_logic::host_superweapon_kindof::special_power_for_superweapon_structure;
    assert!(matches!(
        command_type_from_button_name("Command_SpecialPower"),
        Some(CommandType::DoSpecialPower { .. })
    ));
    assert_eq!(
        special_power_for_superweapon_structure("AmericaParticleCannonUplink"),
        Some(SpecialPowerType::ParticleCannon)
    );
    assert_eq!(
        special_power_for_superweapon_structure("GLAScudStorm"),
        Some(SpecialPowerType::ScudStorm)
    );
    assert_eq!(
        special_power_for_superweapon_structure("ChinaNuclearMissile"),
        Some(SpecialPowerType::NuclearMissile)
    );
}

fn command_type_from_button_name_upgrade_and_cancel_residual() {
    let q = command_type_from_button_name("Command_UpgradeAmericaRangerFlashBangGrenade")
        .expect("upgrade");
    match q {
        CommandType::QueueUpgrade { upgrade_name } => {
            assert_eq!(upgrade_name, "Upgrade_AmericaRangerFlashBangGrenade");
        }
        other => panic!("expected QueueUpgrade, got {other:?}"),
    }
    let c = command_type_from_button_name("Command_CancelUpgrade").expect("cancel");
    assert!(matches!(
        c,
        CommandType::CancelUpgrade { upgrade_name } if upgrade_name.is_empty()
    ));
    assert!(matches!(
        command_type_from_button_name("Command_Stop"),
        Some(CommandType::Stop)
    ));
    assert!(matches!(
        command_type_from_button_name("Command_AttackMove"),
        Some(CommandType::AttackMoveTo { .. })
    ));
    assert!(matches!(
        command_type_from_button_name("Command_SetRallyPoint"),
        Some(CommandType::SetRallyPoint { .. })
    ));
    assert!(matches!(
        command_type_from_button_name("Command_Evacuate"),
        Some(CommandType::Evacuate)
    ));
    assert!(matches!(
        command_type_from_button_name("Command_Sell"),
        Some(CommandType::Sell { .. })
    ));
    assert!(matches!(
        command_type_from_button_name("Command_SpecialPower"),
        Some(CommandType::DoSpecialPower { .. })
    ));
}

#[test]
fn queue_upgrade_refuses_when_production_queue_full_residual() {
    use crate::game_logic::buildings::{
        BuildingData, BuildingType, DEFAULT_PRODUCTION_QUEUE_LIMIT, ProductionItem, ProductionKind,
    };
    use crate::game_logic::host_upgrades::UPGRADE_AMERICA_FLASHBANG;
    use crate::game_logic::{KindOf, Player, Resources, Team, ThingTemplate};

    let mut logic = crate::game_logic::GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 50_000;
    logic.add_player(player);
    let mut bar = ThingTemplate::new("TestBarracks");
    bar.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBarracks)
        .set_health(1000.0);
    logic.templates.insert("TestBarracks".into(), bar);
    let bid = logic
        .create_object("TestBarracks", Team::USA, glam::Vec3::ZERO)
        .expect("barracks");
    if let Some(o) = logic.host_object_mut(bid) {
        let mut bd = BuildingData::new(BuildingType::Barracks);
        for i in 0..DEFAULT_PRODUCTION_QUEUE_LIMIT {
            bd.production_queue.push(ProductionItem {
                template_name: format!("Filler{i}"),
                progress: 0.0,
                total_time: 10.0,
                construction_frames: 0,
                cost: Resources {
                    supplies: 0,
                    power: 0,
                },
                quantity_total: 1,
                quantity_produced: 0,
                kind: ProductionKind::Unit,
            });
        }
        o.building_data = Some(bd);
    }
    let money_before = logic
        .get_player(0)
        .map(|p| p.effective_supplies())
        .unwrap_or(0);
    logic.queue_command(GameCommand {
        command_type: CommandType::QueueUpgrade {
            upgrade_name: UPGRADE_AMERICA_FLASHBANG.to_string(),
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![bid],
        modifier_keys: ModifierKeys::default(),
    });
    logic.process_commands();
    let money_after = logic
        .get_player(0)
        .map(|p| p.effective_supplies())
        .unwrap_or(0);
    assert_eq!(
        money_before, money_after,
        "queue-full upgrade must not charge residual"
    );
    assert!(
        !logic
            .get_player(0)
            .map(|p| p.has_queued_upgrade(UPGRADE_AMERICA_FLASHBANG))
            .unwrap_or(true),
        "must not queue upgrade when production queue full"
    );
}

#[test]
fn cancel_upgrade_empty_name_cancels_production_head_residual() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::host_upgrades::UPGRADE_AMERICA_FLASHBANG;
    use crate::game_logic::{
        KindOf, Player, Team, ThingTemplate,
        buildings::{BuildingData, BuildingType},
    };

    let mut logic = crate::game_logic::GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 5000;
    logic.add_player(player);
    let mut bar = ThingTemplate::new("TestBarracks");
    bar.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBarracks)
        .set_health(1000.0);
    logic.templates.insert("TestBarracks".into(), bar);
    let bid = logic
        .create_object("TestBarracks", Team::USA, glam::Vec3::ZERO)
        .expect("barracks");
    if let Some(o) = logic.host_object_mut(bid) {
        o.building_data = Some(BuildingData::new(BuildingType::Barracks));
    }

    // Queue via command path so player + building both hold residual.
    logic.queue_command(GameCommand {
        command_type: CommandType::QueueUpgrade {
            upgrade_name: UPGRADE_AMERICA_FLASHBANG.to_string(),
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![bid],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    logic.process_commands();
    assert!(
        logic
            .get_player(0)
            .map(|p| p.has_queued_upgrade(UPGRADE_AMERICA_FLASHBANG))
            .unwrap_or(false)
    );
    let money_after_queue = logic
        .get_player(0)
        .map(|p| p.effective_supplies())
        .unwrap_or(0);

    // Empty name CancelUpgrade → head residual.
    logic.queue_command(GameCommand {
        command_type: CommandType::CancelUpgrade {
            upgrade_name: String::new(),
        },
        player_id: 0,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![bid],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    logic.process_commands();

    assert!(
        !logic
            .get_player(0)
            .map(|p| p.has_queued_upgrade(UPGRADE_AMERICA_FLASHBANG))
            .unwrap_or(true),
        "queued upgrade cleared"
    );
    let q_empty = logic
        .host_object(bid)
        .and_then(|o| o.building_data.as_ref())
        .map(|b| b.production_queue.is_empty())
        .unwrap_or(false);
    assert!(q_empty, "building PRODUCTION_UPGRADE head removed");
    let money_after = logic
        .get_player(0)
        .map(|p| p.effective_supplies())
        .unwrap_or(0);
    assert!(
        money_after > money_after_queue,
        "cancel refunds residual cost: before={money_after_queue} after={money_after}"
    );
}

#[test]
fn cancel_upgrade_refunds_only_when_upgrade_is_queued() {
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

    let system = CommandSystem::new();
    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 3000;
    game_logic.add_player(player);

    let mut template = ThingTemplate::new("AmericaSupplyCenter");
    template
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    let producer = Object::new(template, ObjectId(301), Team::USA);
    game_logic.add_object(producer);

    let queue_command = GameCommand {
        command_type: CommandType::QueueUpgrade {
            upgrade_name: "Upgrade_AmericaSupplyLines".to_string(),
        },
        player_id: 0,
        command_id: 10,
        timestamp: SystemTime::now(),
        selected_units: vec![ObjectId(301)],
        modifier_keys: ModifierKeys::default(),
    };
    assert_eq!(
        system.execute_command(&queue_command, &mut game_logic),
        CommandResult::Success
    );

    let cancel_command = GameCommand {
        command_type: CommandType::CancelUpgrade {
            upgrade_name: "Upgrade_AmericaSupplyLines".to_string(),
        },
        player_id: 0,
        command_id: 11,
        timestamp: SystemTime::now(),
        selected_units: vec![ObjectId(301)],
        modifier_keys: ModifierKeys::default(),
    };
    assert_eq!(
        system.execute_command(&cancel_command, &mut game_logic),
        CommandResult::Success
    );

    let player_after_cancel = game_logic.get_player(0).expect("player should exist");
    assert_eq!(
        player_after_cancel.effective_supplies(),
        3000,
        "cancel should refund the queued upgrade cost"
    );
    assert!(
        !player_after_cancel
            .queued_upgrades
            .contains("Upgrade_AmericaSupplyLines")
    );

    assert_eq!(
        system.execute_command(&cancel_command, &mut game_logic),
        CommandResult::InvalidCommand,
        "cancelling a non-queued upgrade should not issue another refund"
    );
}

#[test]
fn queue_upgrade_requires_constructed_building_source() {
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

    let system = CommandSystem::new();
    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 3000;
    game_logic.add_player(player);

    let mut unit_template = ThingTemplate::new("TestUnit");
    unit_template
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    game_logic.add_object(Object::new(unit_template, ObjectId(351), Team::USA));

    let command = GameCommand {
        command_type: CommandType::QueueUpgrade {
            upgrade_name: "Upgrade_AmericaSupplyLines".to_string(),
        },
        player_id: 0,
        command_id: 12,
        timestamp: SystemTime::now(),
        selected_units: vec![ObjectId(351)],
        modifier_keys: ModifierKeys::default(),
    };

    assert_eq!(
        system.execute_command(&command, &mut game_logic),
        CommandResult::InvalidCommand
    );
    let player_after = game_logic.get_player(0).expect("player should exist");
    assert_eq!(
        player_after.effective_supplies(),
        3000,
        "non-producing units must not charge upgrade resources"
    );
    assert!(player_after.queued_upgrades.is_empty());
}

#[test]
fn queue_upgrade_refuses_command_set_without_upgrade_button() {
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

    let system = CommandSystem::new();
    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 5000;
    game_logic.add_player(player);

    let mut template = ThingTemplate::new("AmericaBarracks");
    template
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    game_logic.add_object(Object::new(template, ObjectId(501), Team::USA));

    let command = GameCommand {
        command_type: CommandType::QueueUpgrade {
            upgrade_name: "Upgrade_AmericaSupplyLines".to_string(),
        },
        player_id: 0,
        command_id: 40,
        timestamp: SystemTime::now(),
        selected_units: vec![ObjectId(501)],
        modifier_keys: ModifierKeys::default(),
    };
    assert_eq!(
        system.execute_command(&command, &mut game_logic),
        CommandResult::InvalidCommand,
        "Barracks CommandSet has no SupplyLines button"
    );
    let player_after = game_logic.get_player(0).expect("player");
    assert_eq!(player_after.effective_supplies(), 5000);
    assert!(!player_after.has_queued_upgrade("Upgrade_AmericaSupplyLines"));
}

#[test]
fn queued_upgrade_completes_during_simulation_update() {
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

    let system = CommandSystem::new();
    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 3000;
    game_logic.add_player(player);

    let mut template = ThingTemplate::new("AmericaSupplyCenter");
    template
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    let mut producer = Object::new(template, ObjectId(401), Team::USA);
    // C++ ProductionUpdate advances upgrade research on producers that carry
    // a ProductionUpdate module; the host models that via building_data
    // (retail AmericaSupplyCenter authors it).  Without the module data the
    // production tick skips the producer and the queued upgrade never
    // completes during the simulation update.
    producer.construction_percent = 1.0;
    producer.building_data = Some(crate::game_logic::buildings::BuildingData::new(
        crate::game_logic::buildings::BuildingType::SupplyCenter,
    ));
    game_logic.add_object(producer);

    let command = GameCommand {
        command_type: CommandType::QueueUpgrade {
            upgrade_name: "Upgrade_AmericaSupplyLines".to_string(),
        },
        player_id: 0,
        command_id: 20,
        timestamp: SystemTime::now(),
        selected_units: vec![ObjectId(401)],
        modifier_keys: ModifierKeys::default(),
    };
    assert_eq!(
        system.execute_command(&command, &mut game_logic),
        CommandResult::Success
    );

    let player_after_queue = game_logic.get_player(0).expect("player should exist");
    assert!(
        player_after_queue
            .queued_upgrades
            .contains("Upgrade_AmericaSupplyLines")
    );
    assert!(
        !player_after_queue
            .unlocked_sciences
            .contains("Upgrade_AmericaSupplyLines")
    );

    // C++ research advances on the upgrade's Upgrade.ini BuildTime
    // (ProductionUpdate.cpp:874-879); one 1/30s frame must not instant-
    // complete it.  Advance 30s like the capture upgrade fixtures do.
    game_logic.update();
    game_logic.update_with_dt(30.0);
    let player_after_update = game_logic
        .get_player(0)
        .expect("player should exist after update");
    assert!(
        !player_after_update
            .queued_upgrades
            .contains("Upgrade_AmericaSupplyLines")
    );
    assert!(
        player_after_update
            .unlocked_sciences
            .contains("Upgrade_AmericaSupplyLines")
    );
    assert_eq!(
        system.execute_command(&command, &mut game_logic),
        CommandResult::InvalidCommand,
        "completed upgrades should not be queued or charged again"
    );
}

#[test]
fn command_system_residual_locomotion_pathfinds() {
    let src = crate::command_system::COMMAND_SYSTEM_SRC;
    // Wave 230/231 renamed the executor entry points to the GameLogic
    // unit_command_* authority APIs (GameLogicDispatch.cpp doMoveTo /
    // AIGroup::groupAttackMoveToPosition / groupScatter analogs); each still
    // routes through assign_unit_path (unit_commands.rs:206,603,705).
    let move_i = src.find("fn execute_move_command").expect("move");
    let w = &src[move_i..move_i + 800];
    assert!(
        w.contains("CommandExecutor")
            || w.contains("assign_unit_path")
            || w.contains("unit_command_move_to"),
        "residual move must pathfind via executor or pathfinding authority"
    );
    let am_i = src.find("fn execute_attack_move_command").expect("am");
    let w = &src[am_i..am_i + 800];
    assert!(
        w.contains("CommandExecutor")
            || w.contains("assign_unit_path")
            || w.contains("unit_command_attack_move_to"),
        "residual attack-move must pathfind"
    );
    let sc_i = src.find("fn execute_scatter_command").expect("sc");
    let w = &src[sc_i..sc_i + 1600];
    assert!(
        w.contains("assign_unit_path") || w.contains("unit_command_move_to_moving"),
        "residual scatter must assign_unit_path"
    );
}

#[test]
fn resume_construction_context_residual() {
    let src = crate::command_system::COMMAND_SYSTEM_SRC;
    assert!(
        src.contains("fn can_resume_construction")
            && src.contains("CommandType::ResumeConstruction"),
        "context path must offer ResumeConstruction for unfinished structures"
    );
    let start = src
        .find("fn determine_context_command")
        .expect("determine_context_command");
    let body = &src[start..start + 2200];
    assert!(
        body.contains("can_resume_construction"),
        "determine_context_command must call can_resume_construction"
    );
}

#[test]
fn capture_building_context_residual() {
    let src = crate::command_system::COMMAND_SYSTEM_SRC;
    assert!(
        src.contains("fn can_capture_building") && src.contains("CommandType::CaptureBuilding"),
        "context path must offer CaptureBuilding residual"
    );
    let start = src
        .find("fn determine_context_command")
        .expect("determine_context_command");
    // Window spans the Wave-228 presentation freeze and the C++
    // CommandXlat.cpp dock/gather precedence blocks ahead of the capture
    // probe (can_capture_building call sits ~3400 chars into the body).
    let body = &src[start..start + 3600];
    assert!(
        body.contains("can_capture_building"),
        "determine_context_command must call can_capture_building"
    );
}

#[test]
fn unit_ability_button_name_map_residual() {
    use crate::command_system::{CommandType, command_type_from_button_name};
    assert!(matches!(
        command_type_from_button_name("Command_Hijack"),
        Some(CommandType::Hijack { .. })
    ));
    assert!(matches!(
        command_type_from_button_name("Command_SnipeVehicle"),
        Some(CommandType::SnipeVehicle { .. })
    ));
    assert!(matches!(
        command_type_from_button_name("Command_CaptureBuilding"),
        Some(CommandType::CaptureBuilding { .. })
    ));
    assert!(matches!(
        command_type_from_button_name("Command_PlantTimedDemoCharge"),
        Some(CommandType::PlantTimedDemoCharge { .. })
    ));
    assert!(matches!(
        command_type_from_button_name("Command_BlackLotusStealCash")
            .or_else(|| command_type_from_button_name("Command_StealCashHack")),
        Some(CommandType::StealCashHack { .. })
    ));
}

#[test]
fn retail_special_power_names_map_without_fuzzy_asset_or_id_fallbacks() {
    use crate::command_system::{SpecialPowerType as Power, special_power_type_from_template_name};

    let cases = [
        (
            "AirF_SuperweaponA10ThunderboltMissileStrike",
            Power::AirForceAirstrike,
        ),
        ("AirF_SuperweaponCarpetBomb", Power::AirForceCarpetBomb),
        (
            "AirF_SuperweaponSpectreGunship",
            Power::AirForceSpectreGunship,
        ),
        (
            "Demo_SpecialAbilityDemoKellTimedCharges",
            Power::DemoKellTimedCharges,
        ),
        (
            "Demo_SpecialAbilityDemoRebelTimedCharges",
            Power::DemoRebelTimedCharges,
        ),
        (
            "Demo_SpecialAbilityKellRemoteCharges",
            Power::DemoKellRemoteCharges,
        ),
        (
            "Early_SuperweaponChinaCarpetBomb",
            Power::EarlyChinaCarpetBomb,
        ),
        (
            "Early_SuperweaponEmergencyRepair",
            Power::EarlyEmergencyRepair,
        ),
        ("Early_SuperweaponFrenzy", Power::EarlyFrenzy),
        ("Early_SuperweaponLeafletDrop", Power::EarlyLeafletDrop),
        ("Infa_SuperweaponInfantryParadrop", Power::InfantryParadrop),
        ("Lazr_LaserCannon", Power::LaserCannon),
        (
            "Lazr_SpecialAbilityLaserGuidedHowitzer",
            Power::LaserGuidedHowitzer,
        ),
        ("Nuke_SpecialAbilityHelixNukeBomb", Power::HelixNukeBomb),
        (
            "Nuke_SuperweaponChinaCarpetBomb",
            Power::NukeChinaCarpetBomb,
        ),
        ("Nuke_SuperweaponNeutronMissile", Power::NukeNeutronMissile),
        ("Nuke_SuperweaponNukeDrop", Power::NukeDrop),
        ("Slth_SuperweaponGPSScrambler", Power::StealthGpsScrambler),
        ("SpecialAbilityAmbulanceCleanupArea", Power::CleanupArea),
        (
            "SpecialAbilityBlackLotusCaptureBuilding",
            Power::BlackLotusCaptureBuilding,
        ),
        (
            "SpecialAbilityBlackLotusDisableVehicleHack",
            Power::BlackLotusDisableVehicle,
        ),
        (
            "SpecialAbilityBlackLotusStealCashHack",
            Power::BlackLotusStealCash,
        ),
        (
            "SpecialAbilityColonelBurtonRemoteCharges",
            Power::BurtonRemoteCharges,
        ),
        (
            "SpecialAbilityColonelBurtonTimedCharges",
            Power::BurtonTimedCharges,
        ),
        (
            "SpecialAbilityDisguiseAsVehicle",
            Power::DisguiseAsVehiclePower,
        ),
        (
            "SpecialAbilityHackerDisableBuilding",
            Power::HackerDisableBuilding,
        ),
        ("SpecialAbilityHelixNapalmBomb", Power::HelixNapalmBomb),
        (
            "SpecialAbilityMicrowaveDisableBuilding",
            Power::MicrowaveDisableBuilding,
        ),
        (
            "SpecialAbilityMissileDefenderLaserGuidedMissiles",
            Power::MissileDefenderLaserGuided,
        ),
        (
            "SpecialAbilityRangerCaptureBuilding",
            Power::RangerCaptureBuilding,
        ),
        (
            "SpecialAbilityRebelCaptureBuilding",
            Power::RebelCaptureBuilding,
        ),
        (
            "SpecialAbilityRedGuardCaptureBuilding",
            Power::RedGuardCaptureBuilding,
        ),
        ("SpecialAbilityTankHunterTNTAttack", Power::TankHunterTnt),
        (
            "SpecialPowerBattleshipBombardment",
            Power::BattleshipBombardment,
        ),
        (
            "SpecialPowerCommunicationsDownload",
            Power::CommunicationsDownload,
        ),
        ("SpecialPowerRadarVanScan", Power::RadarScan),
        ("SpecialPowerSpyDrone", Power::SpyDrone),
        ("SpecialPowerSpySatellite", Power::SpySatellite),
        ("SuperweaponA10ThunderboltMissileStrike", Power::Airstrike),
        ("SuperweaponAnthraxBomb", Power::AnthraxBomb),
        ("SuperweaponArtilleryBarrage", Power::Artillery),
        ("SuperweaponCarpetBomb", Power::CarpetBomb),
        ("SuperweaponCashHack", Power::CashHack),
        ("SuperweaponCIAIntelligence", Power::CiaIntelligence),
        ("SuperweaponClusterMines", Power::ClusterMines),
        ("SuperweaponCrateDrop", Power::CrateDrop),
        ("SuperweaponDaisyCutter", Power::DaisyCutter),
        ("SuperweaponEmergencyRepair", Power::EmergencyRepair),
        ("SuperweaponEMPPulse", Power::EmpPulse),
        ("SuperweaponFrenzy", Power::Frenzy),
        ("SuperweaponGPSScrambler", Power::GpsScrambler),
        ("SuperweaponLaunchBaikonurRocket", Power::BaikonurRocket),
        ("SuperweaponLeafletDrop", Power::LeafletDrop),
        ("SuperweaponNapalmStrike", Power::NapalmStrike),
        ("SuperweaponNeutronMissile", Power::NuclearMissile),
        ("SuperweaponParadropAmerica", Power::Paradrop),
        ("SuperweaponParticleUplinkCannon", Power::ParticleCannon),
        ("SuperweaponRebelAmbush", Power::Ambush),
        ("SuperweaponScudStorm", Power::ScudStorm),
        ("SuperweaponSneakAttack", Power::SneakAttack),
        ("SuperweaponSpectreGunship", Power::SpectreGunship),
        ("SuperweaponTerrorCell", Power::TerrorCell),
        ("SupW_CruiseMissile", Power::CruiseMissile),
        (
            "SupW_SuperweaponParticleUplinkCannon",
            Power::SuperweaponParticleCannon,
        ),
        (
            "SupW_SuperweaponNeutronMissile",
            Power::SuperweaponNeutronMissile,
        ),
        ("Tank_SuperweaponTankParadrop", Power::TankParadrop),
    ];

    for (name, expected) in cases {
        assert_eq!(
            special_power_type_from_template_name(name),
            Some(expected),
            "retail CommandButton special power {name} must retain its exact identity"
        );
    }

    assert_eq!(
        special_power_type_from_template_name("SuperweaponCarpetBomb_FA"),
        None,
        "condition/faction suffixes must not silently resolve to a different retail power"
    );
    assert_eq!(
        special_power_type_from_template_name("SpecialAbilityBoobyTrap"),
        None,
        "Booby Trap is an object-target ability, not a nearby superweapon"
    );
}

#[test]
fn hijacker_context_click_issues_hijack_before_attack() {
    // C++ CommandXlat.cpp:1856-1962 — hijack before enter/attack.
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::GLA, "GLA", true));

    let mut hijacker_t = ThingTemplate::new("GLAInfantryHijacker");
    hijacker_t
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic
        .templates
        .insert("GLAInfantryHijacker".into(), hijacker_t);
    let mut tank_t = ThingTemplate::new("AmericaTankCrusader");
    tank_t
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(400.0);
    logic.templates.insert("AmericaTankCrusader".into(), tank_t);

    let hijacker = logic
        .create_object_for_player("GLAInfantryHijacker", 0, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("hijacker");
    let tank = logic
        .create_object(
            "AmericaTankCrusader",
            Team::USA,
            glam::Vec3::new(20.0, 0.0, 0.0),
        )
        .expect("tank");

    let ctx = MouseCommandContext {
        world_position: glam::Vec3::new(20.0, 0.0, 0.0),
        target_object: Some(tank),
        target_presentation: Some(PresentationTargetHint {
            id: tank,
            is_alive: true,
            is_structure: false,
            is_resource: false,
            under_construction: false,
            sold: false,
            team: Team::USA,
            is_enemy_of_local: true,
            is_neutral: false,
            template_name: "AmericaTankCrusader".into(),
            can_be_entered: false,
            enter_available_capacity: 0,
            enter_uses_transport_slots: false,
            enter_requires_infantry: false,
            enter_forbids_aircraft: false,
            enter_disabled_subdued: false,
            enter_is_rider_change: false,
            rider_change_allowed_templates: Vec::new(),
            is_damaged: false,
            is_friendly_of_local: false,
            provides_vehicle_repair: false,
            provides_aircraft_repair: false,
            provides_heal: false,
            can_provide_service: true,
            dock_kind: crate::game_logic::DockKind::None,
            dock_controller_is_local: false,
            stored_supplies: 0,
            capturable: false,
            immune_to_capture: false,
            capture_garrisonable: false,
            capture_nonstealthed_garrison_count: 0,
            capture_friendly_garrison_count: 0,
            capture_target_effectively_stealthed: false,
            is_crate: false,
            is_salvage_crate: false,
            is_vehicle: true,
            is_aircraft: false,
            is_drone: false,
            is_carbomb: false,
            is_unmanned: false,
            is_mine: false,
        }),
        selected_presentation: vec![PresentationSelectedUnitHint {
            id: hijacker,
            is_alive: true,
            is_resource_collector: false,
            is_worker: false,
            can_attack: true,
            can_move: true,
            can_request_service: true,
            can_capture: false,
            template_name: "GLAInfantryHijacker".into(),
            can_repair: false,
            is_damaged: false,
            is_vehicle: false,
            is_aircraft: false,
            is_above_terrain: false,
            is_infantry: true,
            transport_slot_count: 1,
            stored_supplies: 0,
            is_controlled_by_local: true,
            capture_power: crate::game_logic::CapturePowerKind::None,
            capture_power_ready: false,
            is_salvager: false,
            can_override_special_power_destination: false,
        }],
        presentation_box_select_units: Vec::new(),
        presentation_select_similar_units: Vec::new(),
        screen_position: glam::Vec2::ZERO,
        viewport_size: None,
        world_min: None,
        world_max: None,
        mouse_button: MouseButton::Right,
        modifier_keys: ModifierKeys::default(),
        is_drag: false,
        drag_start: None,
        drag_end: None,
        drag_start_world: None,
        drag_end_world: None,
    };
    let mut sys = CommandSystem::new();
    let cmd = sys
        .process_mouse_input(&ctx, &[hijacker], 0, None)
        .expect("hijack context command");
    match cmd.command_type {
        CommandType::Hijack { target_id } => assert_eq!(target_id, tank),
        other => panic!("expected Hijack before Attack, got {other:?}"),
    }
}

#[test]
fn lotus_context_click_auto_hacks_enemy_vehicle() {
    // C++ CommandXlat.cpp:2050-2084 ACTIONTYPE_DISABLE_VEHICLE_VIA_HACKING.
    let lotus = crate::game_logic::ObjectId(1);
    let tank = crate::game_logic::ObjectId(2);
    let ctx = MouseCommandContext {
        world_position: glam::Vec3::new(20.0, 0.0, 0.0),
        target_object: Some(tank),
        target_presentation: Some(PresentationTargetHint {
            id: tank,
            is_alive: true,
            is_structure: false,
            is_resource: false,
            under_construction: false,
            sold: false,
            team: crate::game_logic::Team::USA,
            is_enemy_of_local: true,
            is_neutral: false,
            template_name: "AmericaTankCrusader".into(),
            can_be_entered: false,
            enter_available_capacity: 0,
            enter_uses_transport_slots: false,
            enter_requires_infantry: false,
            enter_forbids_aircraft: false,
            enter_disabled_subdued: false,
            enter_is_rider_change: false,
            rider_change_allowed_templates: Vec::new(),
            is_damaged: false,
            is_friendly_of_local: false,
            provides_vehicle_repair: false,
            provides_aircraft_repair: false,
            provides_heal: false,
            can_provide_service: true,
            dock_kind: crate::game_logic::DockKind::None,
            dock_controller_is_local: false,
            stored_supplies: 0,
            capturable: false,
            immune_to_capture: false,
            capture_garrisonable: false,
            capture_nonstealthed_garrison_count: 0,
            capture_friendly_garrison_count: 0,
            capture_target_effectively_stealthed: false,
            is_crate: false,
            is_salvage_crate: false,
            is_vehicle: true,
            is_aircraft: false,
            is_drone: false,
            is_carbomb: false,
            is_unmanned: false,
            is_mine: false,
        }),
        selected_presentation: vec![PresentationSelectedUnitHint {
            id: lotus,
            is_alive: true,
            is_resource_collector: false,
            is_worker: false,
            can_attack: false,
            can_move: true,
            can_request_service: true,
            can_capture: false,
            template_name: "ChinaInfantryBlackLotus".into(),
            can_repair: false,
            is_damaged: false,
            is_vehicle: false,
            is_aircraft: false,
            is_above_terrain: false,
            is_infantry: true,
            transport_slot_count: 1,
            stored_supplies: 0,
            is_controlled_by_local: true,
            capture_power: crate::game_logic::CapturePowerKind::None,
            capture_power_ready: false,
            is_salvager: false,
            can_override_special_power_destination: false,
        }],
        presentation_box_select_units: Vec::new(),
        presentation_select_similar_units: Vec::new(),
        screen_position: glam::Vec2::ZERO,
        viewport_size: None,
        world_min: None,
        world_max: None,
        mouse_button: MouseButton::Right,
        modifier_keys: ModifierKeys::default(),
        is_drag: false,
        drag_start: None,
        drag_end: None,
        drag_start_world: None,
        drag_end_world: None,
    };
    let mut sys = CommandSystem::new();
    let cmd = sys
        .process_mouse_input(&ctx, &[lotus], 0, None)
        .expect("lotus hack context command");
    match cmd.command_type {
        CommandType::DisableVehicleHack { target_id } => assert_eq!(target_id, tank),
        other => panic!("expected DisableVehicleHack, got {other:?}"),
    }
}

#[test]
fn active_special_power_destination_steers_world_click() {
    // C++ CommandXlat.cpp:1659-1684 override dest before resume/dock/move.
    let puc = crate::game_logic::ObjectId(9);
    let dest = glam::Vec3::new(80.0, 0.0, 40.0);
    let ctx = MouseCommandContext {
        world_position: dest,
        target_object: None,
        target_presentation: None,
        selected_presentation: vec![PresentationSelectedUnitHint {
            id: puc,
            is_alive: true,
            is_resource_collector: false,
            is_worker: false,
            can_attack: false,
            can_move: false,
            can_request_service: true,
            can_capture: false,
            template_name: "AmericaParticleCannonUplink".into(),
            can_repair: false,
            is_damaged: false,
            is_vehicle: false,
            is_aircraft: false,
            is_above_terrain: false,
            is_infantry: false,
            transport_slot_count: 0,
            stored_supplies: 0,
            is_controlled_by_local: true,
            capture_power: crate::game_logic::CapturePowerKind::None,
            capture_power_ready: false,
            is_salvager: false,
            can_override_special_power_destination: true,
        }],
        presentation_box_select_units: Vec::new(),
        presentation_select_similar_units: Vec::new(),
        screen_position: glam::Vec2::ZERO,
        viewport_size: None,
        world_min: None,
        world_max: None,
        mouse_button: MouseButton::Right,
        modifier_keys: ModifierKeys::default(),
        is_drag: false,
        drag_start: None,
        drag_end: None,
        drag_start_world: None,
        drag_end_world: None,
    };
    let mut sys = CommandSystem::new();
    let cmd = sys
        .process_mouse_input(&ctx, &[puc], 0, None)
        .expect("override dest command");
    match cmd.command_type {
        CommandType::OverrideSpecialPowerDestination { location } => {
            assert!((location.x - dest.x).abs() < f32::EPSILON);
            assert!((location.z - dest.z).abs() < f32::EPSILON);
        }
        other => panic!("expected OverrideSpecialPowerDestination, got {other:?}"),
    }
}

fn hint_target(id: ObjectId) -> PresentationTargetHint {
    PresentationTargetHint {
        id,
        is_alive: true,
        is_structure: false,
        is_resource: false,
        under_construction: false,
        sold: false,
        team: crate::game_logic::Team::Neutral,
        is_enemy_of_local: false,
        is_neutral: true,
        template_name: "HintTarget".into(),
        can_be_entered: false,
        enter_available_capacity: 0,
        enter_uses_transport_slots: false,
        enter_requires_infantry: false,
        enter_forbids_aircraft: false,
        enter_disabled_subdued: false,
        enter_is_rider_change: false,
        rider_change_allowed_templates: Vec::new(),
        is_damaged: false,
        is_friendly_of_local: false,
        provides_vehicle_repair: false,
        provides_aircraft_repair: false,
        provides_heal: false,
        can_provide_service: true,
        dock_kind: crate::game_logic::DockKind::None,
        dock_controller_is_local: false,
        stored_supplies: 0,
        capturable: false,
        immune_to_capture: false,
        capture_garrisonable: false,
        capture_nonstealthed_garrison_count: 0,
        capture_friendly_garrison_count: 0,
        capture_target_effectively_stealthed: false,
        is_crate: false,
        is_salvage_crate: false,
        is_vehicle: false,
        is_aircraft: false,
        is_drone: false,
        is_carbomb: false,
        is_unmanned: false,
        is_mine: false,
    }
}

fn hint_selected(id: ObjectId, template: &str) -> PresentationSelectedUnitHint {
    PresentationSelectedUnitHint {
        id,
        is_alive: true,
        is_resource_collector: false,
        is_worker: false,
        can_attack: false,
        can_move: true,
        can_request_service: true,
        can_capture: false,
        template_name: template.into(),
        can_repair: false,
        is_damaged: false,
        is_vehicle: false,
        is_aircraft: false,
        is_above_terrain: false,
        is_infantry: false,
        transport_slot_count: 1,
        stored_supplies: 0,
        is_controlled_by_local: true,
        capture_power: crate::game_logic::CapturePowerKind::None,
        capture_power_ready: false,
        is_salvager: false,
        can_override_special_power_destination: false,
    }
}

fn rmb_ctx(
    target: PresentationTargetHint,
    selected: Vec<PresentationSelectedUnitHint>,
) -> MouseCommandContext {
    MouseCommandContext {
        world_position: glam::Vec3::new(20.0, 0.0, 0.0),
        target_object: Some(target.id),
        target_presentation: Some(target),
        selected_presentation: selected,
        presentation_box_select_units: Vec::new(),
        presentation_select_similar_units: Vec::new(),
        screen_position: glam::Vec2::ZERO,
        viewport_size: None,
        world_min: None,
        world_max: None,
        mouse_button: MouseButton::Right,
        modifier_keys: ModifierKeys::default(),
        is_drag: false,
        drag_start: None,
        drag_end: None,
        drag_start_world: None,
        drag_end_world: None,
    }
}

#[test]
fn tank_rmb_attacks_neutral_tech_oil() {
    // hq-5fhuz: C++ getCanAttackObject is weapon-legal; neutrals/tech are attacked.
    let tank = ObjectId(1);
    let oil = ObjectId(2);
    let mut selected = hint_selected(tank, "AmericaTankCrusader");
    selected.can_attack = true;
    selected.is_vehicle = true;
    let mut target = hint_target(oil);
    target.is_structure = true;
    target.capturable = true;
    target.template_name = "TechOilDerrick".into();
    let mut sys = CommandSystem::new();
    let cmd = sys
        .process_mouse_input(&rmb_ctx(target, vec![selected]), &[tank], 0, None)
        .expect("tank RMB on oil");
    match cmd.command_type {
        CommandType::AttackObject { target_id } => assert_eq!(target_id, oil),
        other => panic!("expected AttackObject on neutral oil, got {other:?}"),
    }
}

#[test]
fn dozer_rmb_does_not_attack_enemy_tank() {
    // hq-5fhuz: dozer DISARM vs non-mine is NOT_POSSIBLE.
    let dozer = ObjectId(1);
    let tank = ObjectId(2);
    let mut selected = hint_selected(dozer, "AmericaVehicleDozer");
    selected.can_attack = true;
    selected.is_worker = true;
    selected.can_repair = true;
    let mut target = hint_target(tank);
    target.is_neutral = false;
    target.is_enemy_of_local = true;
    target.is_vehicle = true;
    target.team = crate::game_logic::Team::GLA;
    target.template_name = "AmericaTankCrusader".into();
    let mut sys = CommandSystem::new();
    let cmd = sys.process_mouse_input(&rmb_ctx(target, vec![selected]), &[dozer], 0, None);
    assert!(
        !matches!(
            cmd.as_ref().map(|c| &c.command_type),
            Some(CommandType::AttackObject { .. })
        ),
        "dozer must not Attack a non-mine, got {cmd:?}"
    );
}

#[test]
fn infantry_rmb_enters_unmanned_husk() {
    // hq-7rr63: C++ canEnterObject unmanned before capacity.
    let ranger = ObjectId(1);
    let husk = ObjectId(2);
    let mut selected = hint_selected(ranger, "AmericaInfantryRanger");
    selected.is_infantry = true;
    let mut target = hint_target(husk);
    target.is_unmanned = true;
    target.is_vehicle = true;
    target.can_be_entered = false;
    target.enter_available_capacity = 0;
    target.template_name = "AmericaTankCrusader".into();
    let mut sys = CommandSystem::new();
    let cmd = sys
        .process_mouse_input(&rmb_ctx(target, vec![selected]), &[ranger], 0, None)
        .expect("ranger RMB on husk");
    match cmd.command_type {
        CommandType::Enter { target_id } => assert_eq!(target_id, husk),
        other => panic!("expected Enter on unmanned husk, got {other:?}"),
    }
}

#[test]
fn mixed_dozer_group_repairs_before_enter() {
    // hq-wzk8m: C++ Repair is before Enter; mixed dozer+ranger must Repair.
    let dozer = ObjectId(1);
    let ranger = ObjectId(2);
    let building = ObjectId(3);
    let mut dozer_h = hint_selected(dozer, "AmericaVehicleDozer");
    dozer_h.is_worker = true;
    dozer_h.can_repair = true;
    let mut ranger_h = hint_selected(ranger, "AmericaInfantryRanger");
    ranger_h.is_infantry = true;
    ranger_h.can_attack = true;
    let mut target = hint_target(building);
    target.is_structure = true;
    target.is_damaged = true;
    target.is_neutral = false;
    target.is_friendly_of_local = true;
    target.is_enemy_of_local = false;
    target.team = crate::game_logic::Team::USA;
    target.can_be_entered = true;
    target.enter_available_capacity = 5;
    target.enter_requires_infantry = true;
    target.template_name = "AmericaBarracks".into();
    let mut sys = CommandSystem::new();
    let cmd = sys
        .process_mouse_input(
            &rmb_ctx(target, vec![dozer_h, ranger_h]),
            &[dozer, ranger],
            0,
            None,
        )
        .expect("mixed dozer RMB");
    match cmd.command_type {
        CommandType::Repair { target_id } => assert_eq!(target_id, building),
        other => panic!("expected Repair before Enter, got {other:?}"),
    }
}

#[test]
fn mixed_dozer_colonel_repairs_damaged_civilian_before_capture() {
    // hq-wzk8m: dozer + colonel on damaged civilian/tech → Repair, not Capture.
    let dozer = ObjectId(1);
    let colonel = ObjectId(2);
    let civic = ObjectId(3);
    let mut dozer_h = hint_selected(dozer, "AmericaVehicleDozer");
    dozer_h.is_worker = true;
    dozer_h.can_repair = true;
    let mut colonel_h = hint_selected(colonel, "AmericaInfantryColonelBurton");
    colonel_h.is_infantry = true;
    colonel_h.can_attack = true;
    colonel_h.can_capture = true;
    colonel_h.capture_power = crate::game_logic::CapturePowerKind::Ranger;
    colonel_h.capture_power_ready = true;
    let mut target = hint_target(civic);
    target.is_structure = true;
    target.is_damaged = true;
    target.capturable = true;
    target.template_name = "TechOilDerrick".into();
    let mut sys = CommandSystem::new();
    let cmd = sys
        .process_mouse_input(
            &rmb_ctx(target, vec![dozer_h, colonel_h]),
            &[dozer, colonel],
            0,
            None,
        )
        .expect("dozer+colonel RMB");
    match cmd.command_type {
        CommandType::Repair { target_id } => assert_eq!(target_id, civic),
        other => panic!("expected Repair before Capture, got {other:?}"),
    }
}

#[test]
fn location_power_unit_click_issues_at_location_not_object() {
    let mut system = CommandSystem::new();
    system.current_mode = CommandMode::SpecialPower {
        power_type: SpecialPowerType::Airstrike,
    };
    let click = Vec3::new(100.0, 0.0, 50.0);
    let context = MouseCommandContext {
        world_position: click,
        target_object: Some(ObjectId(99)),
        target_presentation: None,
        selected_presentation: Vec::new(),
        presentation_box_select_units: Vec::new(),
        presentation_select_similar_units: Vec::new(),
        screen_position: Vec2::new(0.0, 0.0),
        viewport_size: None,
        world_min: None,
        world_max: None,
        mouse_button: MouseButton::Left,
        modifier_keys: ModifierKeys::default(),
        is_drag: false,
        drag_start: None,
        drag_end: None,
        drag_start_world: None,
        drag_end_world: None,
    };
    let command = system
        .process_mouse_input(&context, &[ObjectId(1)], 0, None)
        .expect("location-power click must issue a command");
    match command.command_type {
        CommandType::DoSpecialPower { power_type, target } => {
            assert_eq!(power_type, SpecialPowerType::Airstrike);
            assert_eq!(
                target,
                PowerTarget::Location(click),
                "C++ NEED_TARGET_POS unit click is AT_LOCATION"
            );
        }
        other => panic!("expected DoSpecialPower, got {other:?}"),
    }

    system.current_mode = CommandMode::SpecialPower {
        power_type: SpecialPowerType::MissileDefenderLaserGuided,
    };
    let command = system
        .process_mouse_input(&context, &[ObjectId(1)], 0, None)
        .expect("object-power click must issue a command");
    match command.command_type {
        CommandType::DoSpecialPower { power_type, target } => {
            assert_eq!(power_type, SpecialPowerType::MissileDefenderLaserGuided);
            assert_eq!(
                target,
                PowerTarget::Object(ObjectId(99)),
                "NEED_OBJECT_TARGET unit click stays AT_OBJECT"
            );
        }
        other => panic!("expected DoSpecialPower, got {other:?}"),
    }
}

#[test]
fn leftover_location_target_only_covers_need_target_pos_powers() {
    use SpecialPowerType as P;
    let location_only = [
        P::Airstrike,
        P::DaisyCutter,
        P::FuelAirBomb,
        P::Frenzy,
        P::EarlyFrenzy,
        P::Paradrop,
        P::InfantryParadrop,
        P::TankParadrop,
        P::CrateDrop,
        P::ParticleCannon,
        P::SuperweaponParticleCannon,
        P::LaserCannon,
        P::SpectreGunship,
        P::AirForceSpectreGunship,
        P::Artillery,
        P::NuclearMissile,
        P::NukeNeutronMissile,
        P::Ambush,
        P::LeafletDrop,
        P::GpsScrambler,
        P::StealthGpsScrambler,
        P::EmergencyRepair,
        P::SneakAttack,
        P::CleanupArea,
        P::CarpetBomb,
        P::ScudStorm,
        P::AnthraxBomb,
        P::EmpPulse,
        P::ClusterMines,
        P::SpySatellite,
        P::SpyDrone,
        P::RadarScan,
    ];
    for power in location_only {
        assert!(
            leftover_special_power_is_location_target_only(&power),
            "{power:?} must be leftover NEED_TARGET_POS"
        );
    }
    assert!(!leftover_special_power_is_location_target_only(
        &P::BattleshipBombardment
    ));
    assert!(!leftover_special_power_is_location_target_only(
        &P::MissileDefenderLaserGuided
    ));
    assert!(!leftover_special_power_is_location_target_only(
        &P::LaserGuidedHowitzer
    ));
    assert!(!leftover_special_power_is_location_target_only(
        &P::BaikonurRocket
    ));
    assert!(!leftover_special_power_is_location_target_only(
        &P::RangerCaptureBuilding
    ));
}

#[test]
fn leftover_no_target_covers_can_do_special_power_types() {
    use SpecialPowerType as P;
    let no_target = [
        P::CiaIntelligence,
        P::CommunicationsDownload,
        P::DetonateDirtyNuke,
        P::DemoKellRemoteCharges,
        P::BurtonRemoteCharges,
        P::BattlePlanBombardment,
        P::BattlePlanHoldTheLine,
        P::BattlePlanSearchAndDestroy,
        P::BaikonurRocket,
    ];
    for power in no_target {
        assert!(
            leftover_special_power_is_no_target(&power),
            "{power:?} must leftover-call can_do_special_power"
        );
        assert!(
            !leftover_special_power_is_location_target_only(&power),
            "{power:?} is not leftover NEED_TARGET_POS"
        );
    }
    assert!(!leftover_special_power_is_no_target(&P::SpySatellite));
    assert!(!leftover_special_power_is_no_target(&P::ParticleCannon));
    assert!(!leftover_special_power_is_no_target(&P::CashHack));
}
