    use super::*;
    use crate::game_logic::{GameLogic, Object, ObjectType};
    use game_engine::common::global_data::with_global_data_restored;

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
        game_logic.add_object(near);

        let mut far = Object::new(template, ObjectId(32), Team::USA);
        far.set_position(Vec3::new(120.0, 0.0, 120.0));
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
            player_after_first.effective_supplies(), 4200,
            "upgrade cost should be charged once per team, not per selected unit (retail SupplyLines=800)"
        );
        assert!(player_after_first
            .queued_upgrades
            .contains("Upgrade_AmericaSupplyLines"));

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
                science_name: "A10Strike1".to_string(),
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
                science_name: "a10_strike_1".to_string(),
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
            player.has_unlocked_science("SCIENCE_A10ThunderboltMissileStrike1"),
            "canonical A10 science residual"
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
                (game_engine::common::global_data::read().sell_percentage - 0.25).abs()
                    < f32::EPSILON,
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

    fn right_click_damaged_vehicle_get_repaired_context_residual() {
        use crate::game_logic::{
            buildings::{BuildingData, BuildingType},
            KindOf, Player, Team, ThingTemplate,
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
        use crate::command_system::{command_type_from_button_name, CommandType};
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

    fn special_power_button_maps_and_structure_resolves_puc_residual() {
        use crate::command_system::{command_type_from_button_name, CommandType, SpecialPowerType};
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
            BuildingData, BuildingType, ProductionItem, ProductionKind,
            DEFAULT_PRODUCTION_QUEUE_LIMIT,
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
            buildings::{BuildingData, BuildingType},
            KindOf, Player, Team, ThingTemplate,
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
        assert!(logic
            .get_player(0)
            .map(|p| p.has_queued_upgrade(UPGRADE_AMERICA_FLASHBANG))
            .unwrap_or(false));
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
        assert!(!player_after_cancel
            .queued_upgrades
            .contains("Upgrade_AmericaSupplyLines"));

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
        let producer = Object::new(template, ObjectId(401), Team::USA);
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
        assert!(player_after_queue
            .queued_upgrades
            .contains("Upgrade_AmericaSupplyLines"));
        assert!(!player_after_queue
            .unlocked_sciences
            .contains("Upgrade_AmericaSupplyLines"));

        game_logic.update();

        let player_after_update = game_logic
            .get_player(0)
            .expect("player should exist after update");
        assert!(!player_after_update
            .queued_upgrades
            .contains("Upgrade_AmericaSupplyLines"));
        assert!(player_after_update
            .unlocked_sciences
            .contains("Upgrade_AmericaSupplyLines"));
        assert_eq!(
            system.execute_command(&command, &mut game_logic),
            CommandResult::InvalidCommand,
            "completed upgrades should not be queued or charged again"
        );
    }

    #[test]
    fn command_system_residual_locomotion_pathfinds() {
        let src = include_str!("../command_system.rs");
        let move_i = src.find("fn execute_move_command").expect("move");
        let w = &src[move_i..move_i + 800];
        assert!(
            w.contains("CommandExecutor") || w.contains("assign_unit_path"),
            "residual move must pathfind via executor or assign_unit_path"
        );
        let am_i = src.find("fn execute_attack_move_command").expect("am");
        let w = &src[am_i..am_i + 800];
        assert!(
            w.contains("CommandExecutor") || w.contains("assign_unit_path"),
            "residual attack-move must pathfind"
        );
        let sc_i = src.find("fn execute_scatter_command").expect("sc");
        let w = &src[sc_i..sc_i + 1600];
        assert!(
            w.contains("assign_unit_path"),
            "residual scatter must assign_unit_path"
        );
    }

    #[test]
    fn resume_construction_context_residual() {
        let src = include_str!("../command_system.rs");
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
        let src = include_str!("../command_system.rs");
        assert!(
            src.contains("fn can_capture_building") && src.contains("CommandType::CaptureBuilding"),
            "context path must offer CaptureBuilding residual"
        );
        let start = src
            .find("fn determine_context_command")
            .expect("determine_context_command");
        let body = &src[start..start + 2800];
        assert!(
            body.contains("can_capture_building"),
            "determine_context_command must call can_capture_building"
        );
    }

    #[test]
    fn unit_ability_button_name_map_residual() {
        use crate::command_system::{command_type_from_button_name, CommandType};
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
