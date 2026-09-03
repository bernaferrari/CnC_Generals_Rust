//! Behavior suite extracted from `network_and_scripts`.
use super::*;


#[test]
fn capturing_state_does_not_transfer_under_construction_building() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);

    let captor_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(3.0, 0.0, 0.0))
        .expect("captor should be created");
    let building_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("building should be created");
    {
        let building = game_logic
            .host_object_mut(building_id)
            .expect("building should exist");
        building.set_status_under_construction(true);
    }
    {
        let captor = game_logic
            .host_object_mut(captor_id)
            .expect("captor should exist");
        captor.target = Some(building_id);
        captor.set_ai_state(AIState::Capturing);
    }

    game_logic.update_ai(&[captor_id, building_id], 1.0 / 60.0);

    let building = game_logic
        .host_object(building_id)
        .expect("building should exist");
    assert_eq!(building.team, Team::GLA);

    let captor = game_logic
        .host_object(captor_id)
        .expect("captor should exist");
    assert_eq!(captor.ai_state, AIState::Idle);
    assert!(captor.target.is_none());
}

#[test]
fn capture_command_rejects_non_infantry_units() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);

    let tank_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(6.0, 0.0, 0.0))
        .expect("tank should be created");
    let building_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("building should be created");

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::CaptureBuilding {
            target_id: building_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![tank_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let tank = game_logic.host_object(tank_id).expect("tank should exist");
    assert_ne!(tank.ai_state, AIState::Capturing);
    assert_ne!(tank.target, Some(building_id));

    let building = game_logic
        .host_object(building_id)
        .expect("building should exist");
    assert_eq!(building.team, Team::GLA);
}

#[test]
fn repair_command_sets_all_selected_repairers_to_repairing() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_dozer_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);

    let repairer_a = game_logic
        .create_object("TestDozer", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("repairer A should be created");
    let repairer_b = game_logic
        .create_object("TestDozer", Team::USA, Vec3::new(4.0, 0.0, 0.0))
        .expect("repairer B should be created");
    let target_id = game_logic
        .create_object("TestBuilding", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .expect("repair target should be created");

    {
        let target = game_logic
            .host_object_mut(target_id)
            .expect("target should exist");
        let _ = target.take_damage(50.0);
    }

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::Repair { target_id },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![repairer_a, repairer_b],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let a = game_logic
        .host_object(repairer_a)
        .expect("repairer A should exist");
    let b = game_logic
        .host_object(repairer_b)
        .expect("repairer B should exist");

    assert_eq!(a.ai_state, AIState::Repairing);
    assert_eq!(b.ai_state, AIState::Repairing);
    assert_eq!(a.target, Some(target_id));
    assert_eq!(b.target, Some(target_id));
}

#[test]
fn repair_command_ignores_non_worker_units() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);

    let tank_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("tank should be created");
    let target_id = game_logic
        .create_object("TestBuilding", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .expect("repair target should be created");

    {
        let target = game_logic
            .host_object_mut(target_id)
            .expect("target should exist");
        let _ = target.take_damage(75.0);
    }

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::Repair { target_id },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![tank_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let tank = game_logic.host_object(tank_id).expect("tank should exist");
    assert_ne!(
        tank.ai_state,
        AIState::Repairing,
        "non-worker units should not enter repairing state from repair commands"
    );
}

#[test]
fn repair_command_allows_repairing_neutral_structures() {
    let mut game_logic = GameLogic::new();
    ensure_test_dozer_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);

    let repairer_id = game_logic
        .create_object("TestDozer", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("repairer should be created");
    let target_id = game_logic
        .create_object("TestBuilding", Team::Neutral, Vec3::new(6.0, 0.0, 0.0))
        .expect("neutral target should be created");

    {
        let target = game_logic
            .host_object_mut(target_id)
            .expect("target should exist");
        let _ = target.take_damage(60.0);
    }

    let before = game_logic
        .host_object(target_id)
        .expect("target should exist")
        .health
        .current;

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::Repair { target_id },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![repairer_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    // The real logic clock has advanced past frame 0 by the time a player can
    // issue Repair; C++ Object::attemptHealingFromSoleBenefactor (Object.cpp:1905)
    // refuses all healers while `now <= m_soleHealingBenefactorExpirationFrame`
    // (0 > 0 is false on a virgin frame-0 fixture), so stamp frame 1 to model
    // the live clock before the in-range heal tick.
    game_logic.frame = 1;
    game_logic.update_ai(&[repairer_id, target_id], 1.0 / 60.0);

    let repairer = game_logic
        .host_object(repairer_id)
        .expect("repairer should exist");
    assert_eq!(repairer.ai_state, AIState::Repairing);
    assert_eq!(repairer.target, Some(target_id));

    let after = game_logic
        .host_object(target_id)
        .expect("target should exist")
        .health
        .current;
    assert!(after > before);
}

#[test]
fn dozer_structure_repair_residual_recovers_hp_over_time() {
    let mut game_logic = GameLogic::new();
    ensure_test_dozer_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);

    // Place dozer in interact range so heal starts immediately.
    let dozer_id = game_logic
        .create_object("TestDozer", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .expect("dozer");
    // Explicit WarFactory-named structure so residual covers "repair WarFactory".
    let mut war_factory_tpl = crate::game_logic::ThingTemplate::new("USA_WarFactory");
    war_factory_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSWarFactory)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1500.0)
        .set_cost(2000, -2);
    game_logic
        .templates
        .insert("USA_WarFactory".to_string(), war_factory_tpl);

    let structure_id = game_logic
        .create_object("USA_WarFactory", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("war factory structure");

    {
        let structure = game_logic.host_object_mut(structure_id).expect("structure");
        let _ = structure.take_damage(400.0);
        assert!(
            structure.health.current + 0.01 < structure.health.maximum,
            "structure must be damaged before repair"
        );
    }
    let before = game_logic
        .host_object(structure_id)
        .expect("structure")
        .health
        .current;

    assert_eq!(game_logic.repair_residual_structure_commands(), 0);
    assert!(!game_logic.honesty_structure_repair_ok());

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::Repair {
            target_id: structure_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![dozer_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert_eq!(
        game_logic.repair_residual_structure_commands(),
        1,
        "successful Repair command must record honesty"
    );
    {
        let dozer = game_logic.host_object(dozer_id).expect("dozer");
        assert_eq!(dozer.ai_state, AIState::Repairing);
        assert_eq!(dozer.target, Some(structure_id));
    }
    // Frame-0 stamp models the live clock: the C++-faithful sole-benefactor
    // gate (Object.cpp:1905 `now > expirationFrame`) refuses the first in-range
    // heal while the fixture clock still sits on virgin frame 0.
    game_logic.frame = 1;

    // Several logic frames: HP must increase over time.
    for _ in 0..30 {
        game_logic.update_ai(&[dozer_id, structure_id], 1.0 / 30.0);
    }

    let after = game_logic
        .host_object(structure_id)
        .expect("structure")
        .health
        .current;
    assert!(
        after > before,
        "dozer Repair residual must restore structure HP over time (before={before}, after={after})"
    );
    assert!(
        game_logic.repair_residual_structure_heals() > 0,
        "must record structure heal honesty ticks"
    );
    assert!(
        game_logic.honesty_structure_repair_ok(),
        "structure repair residual honesty path"
    );
    assert!(game_logic.honesty_repair_ok(), "combined repair honesty");
}

#[test]
fn dozer_structure_repair_residual_walk_into_range_recovers_hp() {
    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    ensure_test_dozer_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);

    // Outside INTERACT_RANGE (14): must approach before healing.
    let dozer_id = game_logic
        .create_object("TestDozer", Team::USA, Vec3::new(55.0, 0.0, 0.0))
        .expect("dozer");
    let structure_id = game_logic
        .create_object("TestBuilding", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("structure");

    {
        let structure = game_logic.host_object_mut(structure_id).expect("structure");
        // This regression is about the selected dozer's Repair order.  Disable
        // the independent BaseRegenerateUpdate fixture module so autonomous
        // structure regeneration cannot masquerade as an in-range repair tick.
        structure.base_regenerate = None;
        let _ = structure.take_damage(300.0);
    }
    let before = game_logic
        .host_object(structure_id)
        .expect("structure")
        .health
        .current;

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::Repair {
            target_id: structure_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![dozer_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    {
        let dozer = game_logic.host_object(dozer_id).expect("dozer");
        assert_eq!(dozer.ai_state, AIState::Repairing);
        assert_eq!(dozer.target, Some(structure_id));
        assert!(
            !dozer.movement.path.is_empty(),
            "out-of-range repair must retain an A* approach path rather than bypass movement"
        );
    }
    // Must not heal while still out of range on first short step.
    game_logic.update();
    let mid = game_logic
        .host_object(structure_id)
        .expect("structure")
        .health
        .current;
    // May still be equal if not in range; allow equal on first frame.
    let _ = mid;

    let mut recovered = false;
    for _ in 0..900 {
        game_logic.update();
        if game_logic
            .host_object(structure_id)
            .map(|s| s.health.current > before + 0.5)
            .unwrap_or(false)
        {
            recovered = true;
            break;
        }
    }
    let dozer_after_walk = game_logic.host_object(dozer_id).expect("dozer after walk");
    assert!(
        recovered,
        "dozer must walk into repair range and restore structure HP; pos={:?}, state={:?}, target={:?}, moving={}, path_index={}, path_len={}, movement_target={:?}",
        dozer_after_walk.get_position(),
        dozer_after_walk.ai_state,
        dozer_after_walk.target,
        dozer_after_walk.status.moving,
        dozer_after_walk.movement.current_path_index,
        dozer_after_walk.movement.path.len(),
        dozer_after_walk.movement.target_position,
    );
    assert!(
        game_logic.honesty_structure_repair_ok(),
        "walk-in repair residual honesty (commands={}, heals={})",
        game_logic.repair_residual_structure_commands(),
        game_logic.repair_residual_structure_heals(),
    );

    // Repairing must not be clobbered to Idle mid-approach without finishing.
    let dozer = game_logic.host_object(dozer_id).expect("dozer");
    assert!(
        matches!(dozer.ai_state, AIState::Repairing | AIState::Idle),
        "dozer should still be repairing or finished idle, got {:?}",
        dozer.ai_state
    );
}

#[test]
fn war_factory_vehicle_repair_residual_recovers_hp() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let mut war_factory_tpl = crate::game_logic::ThingTemplate::new("China_WarFactory");
    war_factory_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSWarFactory)
        // C++ ActionManager::canGetRepairedAt (ActionManager.cpp:159-163):
        // a ground vehicle may only GetRepaired at KINDOF_REPAIR_PAD — the
        // FS_WARFACTORY token alone does not authorize the service dock.
        .add_kind_of(KindOf::RepairPad)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(2000.0)
        .set_cost(2000, -2);
    game_logic
        .templates
        .insert("China_WarFactory".to_string(), war_factory_tpl);

    let war_factory_id = game_logic
        .create_object("China_WarFactory", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("war factory");
    {
        let wf = game_logic.host_object(war_factory_id).expect("wf");
        assert_eq!(
            wf.building_data.as_ref().map(|b| b.building_type),
            Some(BuildingType::WarFactory)
        );
    }

    let vehicle_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(6.0, 0.0, 0.0))
        .expect("vehicle");
    {
        let vehicle = game_logic.host_object_mut(vehicle_id).expect("vehicle");
        let _ = vehicle.take_damage(120.0);
    }
    let before = game_logic
        .host_object(vehicle_id)
        .expect("vehicle")
        .health
        .current;

    assert!(!game_logic.honesty_vehicle_repair_ok());

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::GetRepaired {
            target_id: war_factory_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![vehicle_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    {
        let vehicle = game_logic.host_object(vehicle_id).expect("vehicle");
        assert_eq!(
            vehicle.ai_state,
            AIState::SeekingRepair,
            "WarFactory must accept GetRepaired for vehicles"
        );
        assert_eq!(vehicle.target, Some(war_factory_id));
    }
    // Live-clock stamp for the dock heal window (C++ sole-benefactor
    // expiration semantics refuse frame-0 healers).
    game_logic.frame = 1;

    for _ in 0..30 {
        game_logic.update_ai(&[war_factory_id, vehicle_id], 1.0 / 30.0);
    }

    let after = game_logic
        .host_object(vehicle_id)
        .expect("vehicle")
        .health
        .current;
    assert!(
        after > before,
        "WarFactory vehicle repair residual must restore HP (before={before}, after={after})"
    );
    assert!(
        game_logic.repair_residual_vehicle_heals() > 0,
        "must record vehicle heal honesty"
    );
    assert!(
        game_logic.honesty_vehicle_repair_ok(),
        "vehicle repair residual honesty"
    );
}

#[test]
fn ambulance_auto_heal_residual_recovers_infantry_hp() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);

    let mut ambulance_tpl = crate::game_logic::ThingTemplate::new("AmericaVehicleMedic");
    ambulance_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(240.0)
        .set_cost(600, 0);
    game_logic
        .templates
        .insert("AmericaVehicleMedic".to_string(), ambulance_tpl);

    let ambulance_id = game_logic
        .create_object("AmericaVehicleMedic", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("ambulance");
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .expect("infantry");

    {
        let infantry = game_logic.host_object_mut(infantry_id).expect("infantry");
        let _ = infantry.take_damage(40.0);
        assert!(
            infantry.health.current + 0.01 < infantry.health.maximum,
            "infantry must be damaged before ambulance heal"
        );
    }
    let before = game_logic
        .host_object(infantry_id)
        .expect("infantry")
        .health
        .current;

    assert_eq!(game_logic.heal_residual_ambulance_heals(), 0);
    assert!(!game_logic.honesty_ambulance_heal_ok());
    assert!(!game_logic.honesty_heal_ok());

    // Live-clock stamp: the C++-faithful sole-benefactor gate
    // (Object.cpp:1905 `now > m_soleHealingBenefactorExpirationFrame`)
    // refuses frame-0 healers, so the fixture clock must sit past frame 0.
    game_logic.frame = 1;
    // Several logic frames of residual AutoHeal (no command required — StartsActive).
    for _ in 0..30 {
        game_logic.update_ambulance_auto_heal(1.0 / 30.0);
    }

    let after = game_logic
        .host_object(infantry_id)
        .expect("infantry")
        .health
        .current;
    assert!(
        after > before,
        "ambulance AutoHeal residual must restore infantry HP (before={before}, after={after})"
    );
    assert!(
        game_logic.heal_residual_ambulance_heals() > 0,
        "must record ambulance heal honesty ticks"
    );
    assert!(
        game_logic.honesty_ambulance_heal_ok(),
        "ambulance heal residual honesty path"
    );
    assert!(game_logic.honesty_heal_ok(), "combined heal honesty");

    // Ambulance itself still present (not self-healed as infantry residual).
    assert!(
        game_logic
            .host_object(ambulance_id)
            .map(|a| a.is_alive())
            .unwrap_or(false),
        "ambulance must remain alive"
    );
}

#[test]
fn ambulance_auto_heal_residual_out_of_range_then_in_range() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);

    let mut ambulance_tpl = crate::game_logic::ThingTemplate::new("USA_Ambulance");
    ambulance_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(240.0)
        .set_cost(600, 0);
    game_logic
        .templates
        .insert("USA_Ambulance".to_string(), ambulance_tpl);

    let _ambulance_id = game_logic
        .create_object("USA_Ambulance", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("ambulance");
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(200.0, 0.0, 0.0))
        .expect("infantry");
    {
        let infantry = game_logic.host_object_mut(infantry_id).expect("infantry");
        let _ = infantry.take_damage(30.0);
    }
    let before = game_logic
        .host_object(infantry_id)
        .expect("infantry")
        .health
        .current;

    // Out of residual radius (100): no heal.
    for _ in 0..15 {
        game_logic.update_ambulance_auto_heal(1.0 / 30.0);
    }
    let mid = game_logic
        .host_object(infantry_id)
        .expect("infantry")
        .health
        .current;
    assert!(
        (mid - before).abs() < 0.01,
        "out-of-range infantry must not receive ambulance heal"
    );
    assert!(!game_logic.honesty_ambulance_heal_ok());

    // Move into radius.
    {
        let infantry = game_logic.host_object_mut(infantry_id).expect("infantry");
        infantry.set_position(Vec3::new(30.0, 0.0, 0.0));
    }
    // Live-clock stamp past frame 0 (C++ sole-benefactor expiration gate).
    game_logic.frame = 1;
    for _ in 0..30 {
        game_logic.update_ambulance_auto_heal(1.0 / 30.0);
    }
    let after = game_logic
        .host_object(infantry_id)
        .expect("infantry")
        .health
        .current;
    assert!(
        after > before,
        "in-range infantry must recover HP from ambulance residual"
    );
    assert!(game_logic.honesty_ambulance_heal_ok());
}

#[test]
fn ambulance_auto_heal_residual_skips_enemy_infantry() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);

    let mut ambulance_tpl = crate::game_logic::ThingTemplate::new("AmericaVehicleMedic");
    ambulance_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(240.0)
        .set_cost(600, 0);
    game_logic
        .templates
        .insert("AmericaVehicleMedic".to_string(), ambulance_tpl);

    let _ambulance_id = game_logic
        .create_object("AmericaVehicleMedic", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("ambulance");
    let enemy_id = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(10.0, 0.0, 0.0))
        .expect("enemy infantry");
    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        let _ = enemy.take_damage(40.0);
    }
    let before = game_logic
        .host_object(enemy_id)
        .expect("enemy")
        .health
        .current;

    for _ in 0..30 {
        game_logic.update_ambulance_auto_heal(1.0 / 30.0);
    }
    let after = game_logic
        .host_object(enemy_id)
        .expect("enemy")
        .health
        .current;
    assert!(
        (after - before).abs() < 0.01,
        "enemy infantry must not be healed by USA ambulance residual"
    );
    assert!(!game_logic.honesty_ambulance_heal_ok());
}

#[test]
fn propaganda_tower_residual_recovers_hp_and_sets_enthusiastic() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);

    let mut tower_tpl = crate::game_logic::ThingTemplate::new("ChinaSpeakerTower");
    tower_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(300.0)
        .set_cost(500, 0);
    game_logic
        .templates
        .insert("ChinaSpeakerTower".to_string(), tower_tpl);

    let tower_id = game_logic
        .create_object("ChinaSpeakerTower", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("speaker tower");
    let unit_id = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(30.0, 0.0, 0.0))
        .expect("unit");

    {
        let unit = game_logic.host_object_mut(unit_id).expect("unit");
        let _ = unit.take_damage(40.0);
        assert!(
            unit.health.current + 0.01 < unit.health.maximum,
            "unit must be damaged before propaganda heal"
        );
        assert!(!unit.weapon_bonus_enthusiastic);
        assert!(!unit.weapon_bonus_subliminal);
    }
    let before = game_logic
        .host_object(unit_id)
        .expect("unit")
        .health
        .current;

    assert_eq!(game_logic.propaganda_residual_heals(), 0);
    assert_eq!(game_logic.propaganda_residual_buffs(), 0);
    assert!(!game_logic.honesty_propaganda_ok());

    // Several logic frames of residual pulse (no command — continuous AoE).
    for _ in 0..30 {
        game_logic.update_propaganda_tower_pulse(1.0 / 30.0);
    }

    let unit = game_logic.host_object(unit_id).expect("unit");
    assert!(
        unit.health.current > before,
        "propaganda residual must restore HP (before={before}, after={})",
        unit.health.current
    );
    assert!(
        unit.weapon_bonus_enthusiastic,
        "in-range unit must receive ENTHUSIASTIC residual buff"
    );
    assert!(
        !unit.weapon_bonus_subliminal,
        "base tower without upgrade must not grant SUBLIMINAL"
    );
    assert!(
        game_logic.propaganda_residual_heals() > 0,
        "must record propaganda heal honesty ticks"
    );
    assert!(
        game_logic.propaganda_residual_buffs() > 0,
        "must record propaganda buff honesty ticks"
    );
    assert!(game_logic.honesty_propaganda_heal_ok());
    assert!(game_logic.honesty_propaganda_buff_ok());
    assert!(game_logic.honesty_propaganda_ok());

    assert!(
        game_logic
            .host_object(tower_id)
            .map(|t| t.is_alive())
            .unwrap_or(false),
        "tower must remain alive"
    );
}

#[test]
fn propaganda_tower_residual_out_of_range_then_in_range() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);

    let mut tower_tpl = crate::game_logic::ThingTemplate::new("ChinaSpeakerTower");
    tower_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(300.0)
        .set_cost(500, 0);
    game_logic
        .templates
        .insert("ChinaSpeakerTower".to_string(), tower_tpl);

    let _tower_id = game_logic
        .create_object("ChinaSpeakerTower", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("speaker tower");
    let unit_id = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(250.0, 0.0, 0.0))
        .expect("unit");
    {
        let unit = game_logic.host_object_mut(unit_id).expect("unit");
        let _ = unit.take_damage(30.0);
    }
    let before = game_logic
        .host_object(unit_id)
        .expect("unit")
        .health
        .current;

    // Out of residual radius (150): no heal / no buff.
    for _ in 0..15 {
        game_logic.update_propaganda_tower_pulse(1.0 / 30.0);
    }
    {
        let unit = game_logic.host_object(unit_id).expect("unit");
        assert!(
            (unit.health.current - before).abs() < 0.01,
            "out-of-range unit must not receive propaganda heal"
        );
        assert!(!unit.weapon_bonus_enthusiastic);
    }
    assert!(!game_logic.honesty_propaganda_ok());

    // Move into radius — membership waits for the next 2s scan.
    {
        let unit = game_logic.host_object_mut(unit_id).expect("unit");
        unit.set_position(Vec3::new(40.0, 0.0, 0.0));
    }
    game_logic.update_propaganda_tower_pulse(1.0 / 30.0);
    {
        let unit = game_logic.host_object(unit_id).expect("unit");
        assert!(
            !unit.weapon_bonus_enthusiastic,
            "enter waits for next scan (C++ m_scanDelayInFrames)"
        );
    }
    game_logic.frame = game_logic.frame.saturating_add(
        crate::game_logic::host_propaganda::HOST_PROPAGANDA_DELAY_BETWEEN_UPDATES_FRAMES,
    );
    for _ in 0..30 {
        game_logic.update_propaganda_tower_pulse(1.0 / 30.0);
    }
    {
        let unit = game_logic.host_object(unit_id).expect("unit");
        assert!(
            unit.health.current > before,
            "in-range unit must recover HP from propaganda residual"
        );
        assert!(unit.weapon_bonus_enthusiastic);
    }
    assert!(game_logic.honesty_propaganda_ok());

    // Leave radius: buff sticks until the next scan.
    {
        let unit = game_logic.host_object_mut(unit_id).expect("unit");
        unit.set_position(Vec3::new(300.0, 0.0, 0.0));
    }
    game_logic.update_propaganda_tower_pulse(1.0 / 30.0);
    {
        let unit = game_logic.host_object(unit_id).expect("unit");
        assert!(
            unit.weapon_bonus_enthusiastic,
            "leave keeps ENTHUSIASTIC until next scan"
        );
    }
    game_logic.frame = game_logic.frame.saturating_add(
        crate::game_logic::host_propaganda::HOST_PROPAGANDA_DELAY_BETWEEN_UPDATES_FRAMES,
    );
    game_logic.update_propaganda_tower_pulse(1.0 / 30.0);
    {
        let unit = game_logic.host_object(unit_id).expect("unit");
        assert!(
            !unit.weapon_bonus_enthusiastic,
            "next scan after leave must clear ENTHUSIASTIC"
        );
        assert!(!unit.weapon_bonus_subliminal);
    }
}

#[test]
fn propaganda_tower_residual_skips_enemy_units() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);

    let mut tower_tpl = crate::game_logic::ThingTemplate::new("ChinaSpeakerTower");
    tower_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(300.0)
        .set_cost(500, 0);
    game_logic
        .templates
        .insert("ChinaSpeakerTower".to_string(), tower_tpl);

    let _tower_id = game_logic
        .create_object("ChinaSpeakerTower", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("speaker tower");
    let enemy_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .expect("enemy");
    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        let _ = enemy.take_damage(40.0);
    }
    let before = game_logic
        .host_object(enemy_id)
        .expect("enemy")
        .health
        .current;

    for _ in 0..30 {
        game_logic.update_propaganda_tower_pulse(1.0 / 30.0);
    }
    let enemy = game_logic.host_object(enemy_id).expect("enemy");
    assert!(
        (enemy.health.current - before).abs() < 0.01,
        "enemy unit must not be healed by China speaker tower residual"
    );
    assert!(!enemy.weapon_bonus_enthusiastic);
    assert!(!game_logic.honesty_propaganda_ok());
}

#[test]
fn propaganda_tower_residual_subliminal_upgrade_buff_and_faster_heal() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);

    let mut tower_tpl = crate::game_logic::ThingTemplate::new("ChinaSpeakerTower");
    tower_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(300.0)
        .set_cost(500, 0);
    game_logic
        .templates
        .insert("ChinaSpeakerTower".to_string(), tower_tpl);

    // C++ PropagandaTowerBehavior::effectLogic:275 reads
    // getControllingPlayer()->hasUpgradeComplete(m_upgradeRequired): the
    // subliminal upgrade lives on the tower's controlling PLAYER (owner + team
    // provenance), never on the tower object, so the fixture registers the
    // China player, completes the upgrade there, and owner-stamps the tower.
    ensure_test_player_for_team(&mut game_logic, Team::China);
    {
        let player = game_logic.get_player_mut(1).expect("china player");
        player.completed_upgrades.insert(
            crate::game_logic::host_propaganda::UPGRADE_CHINA_SUBLIMINAL_MESSAGING.to_string(),
        );
    }
    let tower_id = game_logic
        .create_object_for_player("ChinaSpeakerTower", 1, Vec3::new(0.0, 0.0, 0.0))
        .expect("speaker tower");

    let unit_id = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(20.0, 0.0, 0.0))
        .expect("unit");
    {
        let unit = game_logic.host_object_mut(unit_id).expect("unit");
        let _ = unit.take_damage(40.0);
    }
    let before = game_logic
        .host_object(unit_id)
        .expect("unit")
        .health
        .current;

    // C++ PropagandaTowerBehavior m_scanDelayInFrames: the first in-range scan
    // only registers membership; the buff lands on a later scan. Advance the
    // clock past the scan delay exactly like the sibling propaganda fixtures.
    game_logic.frame = game_logic.frame.saturating_add(
        crate::game_logic::host_propaganda::HOST_PROPAGANDA_DELAY_BETWEEN_UPDATES_FRAMES,
    );
    for _ in 0..30 {
        game_logic.update_propaganda_tower_pulse(1.0 / 30.0);
    }

    let unit = game_logic.host_object(unit_id).expect("unit");
    assert!(unit.weapon_bonus_enthusiastic);
    assert!(
        unit.weapon_bonus_subliminal,
        "upgraded tower must grant SUBLIMINAL residual buff"
    );
    // 4% of max (80) per second * 1s = ~3.2 HP; base would be ~1.6.
    assert!(
        unit.health.current > before + 2.5,
        "upgraded heal rate residual should exceed base (before={before}, after={})",
        unit.health.current
    );
    assert!(game_logic.honesty_propaganda_ok());
}

#[test]
fn propaganda_tower_name_residual_helix_propaganda_heals() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);

    let mut tower_tpl = crate::game_logic::ThingTemplate::new("ChinaHelixPropagandaTower");
    tower_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(200.0)
        .set_cost(0, 0);
    game_logic
        .templates
        .insert("ChinaHelixPropagandaTower".to_string(), tower_tpl);

    let _tower_id = game_logic
        .create_object(
            "ChinaHelixPropagandaTower",
            Team::China,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("helix prop tower");
    let unit_id = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(25.0, 0.0, 0.0))
        .expect("unit");
    {
        let unit = game_logic.host_object_mut(unit_id).expect("unit");
        let _ = unit.take_damage(30.0);
    }
    let before = game_logic
        .host_object(unit_id)
        .expect("unit")
        .health
        .current;

    for _ in 0..30 {
        game_logic.update_propaganda_tower_pulse(1.0 / 30.0);
    }
    let unit = game_logic.host_object(unit_id).expect("unit");
    assert!(unit.health.current > before);
    assert!(unit.weapon_bonus_enthusiastic);
    assert!(game_logic.honesty_propaganda_ok());
}

#[test]
fn heal_pad_seeking_healing_residual_recovers_infantry_hp() {
    let _env_guard = HOST_STATE_RESIDUAL_TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    // Host-state residual honesty without shadow writeback: authority
    // channels default off on the fresh GameLogic context.

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_heal_pad_template(&mut game_logic);

    let heal_pad_id = game_logic
        .create_object("TestHealPad", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("heal pad");
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .expect("infantry");
    {
        let infantry = game_logic.host_object_mut(infantry_id).expect("infantry");
        let _ = infantry.take_damage(40.0);
    }
    let before = game_logic
        .host_object(infantry_id)
        .expect("infantry")
        .health
        .current;

    assert!(!game_logic.honesty_heal_pad_ok());

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::GetHealed {
            target_id: heal_pad_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![infantry_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    {
        let infantry = game_logic.host_object(infantry_id).expect("infantry");
        assert_eq!(infantry.ai_state, AIState::SeekingHealing);
        assert_eq!(infantry.target, Some(heal_pad_id));
    }

    for _ in 0..30 {
        game_logic.update_ai(&[heal_pad_id, infantry_id], 1.0 / 30.0);
    }

    let after = game_logic
        .host_object(infantry_id)
        .expect("infantry")
        .health
        .current;
    assert!(
        after > before,
        "HealPad SeekingHealing residual must restore infantry HP (before={before}, after={after})"
    );
    assert!(
        game_logic.heal_residual_heal_pad_heals() > 0,
        "must record heal-pad honesty ticks"
    );
    assert!(game_logic.honesty_heal_pad_ok());
    assert!(game_logic.honesty_heal_ok());

}

#[test]
fn physical_service_commands_use_player_relationship_and_revalidate_owner_changes() {
    use crate::command_system::{
        CommandSystem, ModifierKeys, MouseButton, MouseCommandContext,
        PresentationSelectedUnitHint, PresentationTargetHint,
    };

    let mut game_logic = GameLogic::new();
    let mut local = Player::new(0, Team::USA, "USA local", true);
    local.alliance_team = 7;
    let mut same_faction_enemy = Player::new(1, Team::USA, "USA enemy", false);
    same_faction_enemy.alliance_team = 9;
    let mut cross_faction_ally = Player::new(2, Team::China, "China ally", false);
    cross_faction_ally.alliance_team = 7;
    game_logic.add_player(local);
    game_logic.add_player(same_faction_enemy);
    game_logic.add_player(cross_faction_ally);

    ensure_test_tank_template(&mut game_logic);
    let mut service_pad = ThingTemplate::new("OwnerRelationServicePad");
    service_pad
        .add_kind_of(KindOf::Structure)
        // The active service authority is C++ KINDOF_REPAIR_PAD, not the
        // legacy BuildingType presentation fixture below.
        .add_kind_of(KindOf::RepairPad)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1_000.0)
        .set_cost(500, -1);
    game_logic
        .templates
        .insert("OwnerRelationServicePad".to_string(), service_pad);

    let tank_id = game_logic
        .create_object_for_player("TestTank", 0, Vec3::new(8.0, 0.0, 0.0))
        .expect("local tank");
    let same_faction_enemy_pad = game_logic
        .create_object_for_player("OwnerRelationServicePad", 1, Vec3::ZERO)
        .expect("same-faction enemy pad");
    let cross_faction_ally_pad = game_logic
        .create_object_for_player("OwnerRelationServicePad", 2, Vec3::ZERO)
        .expect("cross-faction allied pad");
    for pad_id in [same_faction_enemy_pad, cross_faction_ally_pad] {
        game_logic
            .host_object_mut(pad_id)
            .expect("service pad")
            .building_data = Some(BuildingData::new(BuildingType::RepairPad));
    }
    {
        // Keep the service command test independent of a coupled GameWorld
        // damage writeback: command authority observes this host HP directly.
        let tank = game_logic.host_object_mut(tank_id).expect("tank");
        tank.health.current = (tank.health.maximum - 80.0).max(1.0);
    }

    let selected_hint = || PresentationSelectedUnitHint {
        id: tank_id,
        is_alive: true,
        is_resource_collector: false,
        is_worker: false,
        can_attack: false,
        can_move: true,
        can_request_service: true,
        can_capture: false,
        template_name: "TestTank".to_string(),
        can_repair: false,
        is_damaged: true,
        is_vehicle: true,
        is_aircraft: false,
        is_above_terrain: false,
        is_infantry: false,
        transport_slot_count: 3,
        stored_supplies: 0,
        is_controlled_by_local: true,
        capture_power: CapturePowerKind::None,
        capture_power_ready: false,
        is_salvager: false,
        can_override_special_power_destination: false,
    };
    let service_context =
        |target_id, team, is_enemy_of_local, is_friendly_of_local| MouseCommandContext {
            world_position: Vec3::ZERO,
            target_object: Some(target_id),
            target_presentation: Some(PresentationTargetHint {
                id: target_id,
                is_alive: true,
                is_structure: true,
                is_resource: false,
                under_construction: false,
                sold: false,
                team,
                is_enemy_of_local,
                is_neutral: false,
                template_name: "OwnerRelationServicePad".to_string(),
                can_be_entered: false,
                enter_available_capacity: 0,
                enter_uses_transport_slots: false,
                enter_requires_infantry: false,
                enter_forbids_aircraft: false,
                enter_disabled_subdued: false,
                enter_is_rider_change: false,
                rider_change_allowed_templates: Vec::new(),
                is_damaged: false,
                is_friendly_of_local,
                provides_vehicle_repair: true,
                provides_aircraft_repair: false,
                provides_heal: false,
                can_provide_service: true,
                dock_kind: DockKind::None,
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
            }),
            selected_presentation: vec![selected_hint()],
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

    let mut command_system = CommandSystem::new();
    let enemy_click = command_system
        .process_mouse_input(
            &service_context(same_faction_enemy_pad, Team::USA, true, false),
            &[tank_id],
            0,
            Some(&game_logic),
        )
        .expect("physical right click command");
    assert!(
        matches!(
            enemy_click.command_type,
            crate::command_system::CommandType::MoveTo { .. }
        ),
        "same-faction enemy service pad must not classify as a friendly repair command"
    );

    // A stale/malicious service command cannot bypass the frozen RMB result.
    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::GetRepaired {
            target_id: same_faction_enemy_pad,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![tank_id],
        modifier_keys: ModifierKeys::default(),
    });
    game_logic.process_commands();
    let tank = game_logic
        .host_object(tank_id)
        .expect("tank after rejected command");
    assert_ne!(tank.ai_state, AIState::SeekingRepair);
    assert_ne!(tank.target, Some(same_faction_enemy_pad));

    let ally_click = command_system
        .process_mouse_input(
            &service_context(cross_faction_ally_pad, Team::China, false, true),
            &[tank_id],
            0,
            Some(&game_logic),
        )
        .expect("physical allied right click command");
    assert!(
        matches!(
            ally_click.command_type,
            crate::command_system::CommandType::GetRepaired { target_id }
                if target_id == cross_faction_ally_pad
        ),
        "cross-faction allied repair pad must issue GetRepaired"
    );
    game_logic.queue_command(ally_click);
    game_logic.process_commands();
    let tank = game_logic
        .host_object(tank_id)
        .expect("tank after allied command");
    assert_eq!(tank.ai_state, AIState::SeekingRepair);
    assert_eq!(tank.target, Some(cross_faction_ally_pad));

    // Revalidate while moving/docked: a captured or reassigned repair pad may
    // no longer service this tank even if its original RMB was legal.
    assert!(game_logic.transfer_object_to_player(cross_faction_ally_pad, 1));
    game_logic.update_ai(&[tank_id, cross_faction_ally_pad], 1.0 / 30.0);
    let tank = game_logic
        .host_object(tank_id)
        .expect("tank after owner change");
    assert_ne!(tank.target, Some(cross_faction_ally_pad));
}

#[test]
fn get_repaired_command_targets_only_damaged_vehicles() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_repair_pad_template(&mut game_logic);

    let repair_bay_id = game_logic
        .create_object("TestRepairPad", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("repair bay should be created");
    let vehicle_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(8.0, 0.0, 0.0))
        .expect("vehicle should be created");
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(9.0, 0.0, 0.0))
        .expect("infantry should be created");

    {
        let vehicle = game_logic
            .host_object_mut(vehicle_id)
            .expect("vehicle should exist");
        let _ = vehicle.take_damage(80.0);
    }
    {
        let infantry = game_logic
            .host_object_mut(infantry_id)
            .expect("infantry should exist");
        let _ = infantry.take_damage(20.0);
    }

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::GetRepaired {
            target_id: repair_bay_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![vehicle_id, infantry_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let vehicle = game_logic
        .host_object(vehicle_id)
        .expect("vehicle should exist");
    let infantry = game_logic
        .host_object(infantry_id)
        .expect("infantry should exist");
    assert_eq!(vehicle.ai_state, AIState::SeekingRepair);
    assert_ne!(infantry.ai_state, AIState::SeekingRepair);
}

#[test]
fn get_repaired_command_requires_repair_destination_type() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);

    let non_repair_structure = game_logic
        .create_object("TestBuilding", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("support structure should be created");
    let vehicle_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(8.0, 0.0, 0.0))
        .expect("vehicle should be created");
    {
        let vehicle = game_logic
            .host_object_mut(vehicle_id)
            .expect("vehicle should exist");
        let _ = vehicle.take_damage(80.0);
    }

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::GetRepaired {
            target_id: non_repair_structure,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![vehicle_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let vehicle = game_logic
        .host_object(vehicle_id)
        .expect("vehicle should exist");
    assert_ne!(vehicle.ai_state, AIState::SeekingRepair);
    assert_ne!(vehicle.target, Some(non_repair_structure));
}

#[test]
fn get_repaired_command_rejects_under_construction_destination() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_repair_pad_template(&mut game_logic);

    let repair_pad_id = game_logic
        .create_object("TestRepairPad", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("repair pad should be created");
    let vehicle_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(8.0, 0.0, 0.0))
        .expect("vehicle should be created");
    {
        let vehicle = game_logic
            .host_object_mut(vehicle_id)
            .expect("vehicle should exist");
        let _ = vehicle.take_damage(80.0);
    }
    {
        let repair_pad = game_logic
            .host_object_mut(repair_pad_id)
            .expect("repair pad should exist");
        repair_pad.set_status_under_construction(true);
    }

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::GetRepaired {
            target_id: repair_pad_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![vehicle_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let vehicle = game_logic
        .host_object(vehicle_id)
        .expect("vehicle should exist");
    assert_ne!(vehicle.ai_state, AIState::SeekingRepair);
    assert_ne!(vehicle.target, Some(repair_pad_id));
}

#[test]
fn get_repaired_command_aircraft_requires_airfield() {
    let mut game_logic = GameLogic::new();
    ensure_test_aircraft_template(&mut game_logic);
    ensure_test_repair_pad_template(&mut game_logic);
    ensure_test_airfield_template(&mut game_logic);

    let repair_pad_id = game_logic
        .create_object("TestRepairPad", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("repair pad should be created");
    let airfield_id = game_logic
        .create_object("TestAirfield", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .expect("airfield should be created");
    let aircraft_id = game_logic
        .create_object("TestAircraft", Team::USA, Vec3::new(8.0, 0.0, 0.0))
        .expect("aircraft should be created");
    {
        let aircraft = game_logic
            .host_object_mut(aircraft_id)
            .expect("aircraft should exist");
        let _ = aircraft.take_damage(100.0);
        // C++ canGetRepairedAt (ActionManager.cpp:164-171) requires the
        // aircraft to be above terrain (`isAboveTerrain`) before the airfield
        // branch; the host proof-of-altitude channel is the airborne status.
        aircraft.status.airborne_target = true;
    }

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::GetRepaired {
            target_id: repair_pad_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![aircraft_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    let aircraft = game_logic
        .host_object(aircraft_id)
        .expect("aircraft should exist");
    assert_ne!(aircraft.ai_state, AIState::SeekingRepair);

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::GetRepaired {
            target_id: airfield_id,
        },
        player_id: 0,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![aircraft_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    let aircraft = game_logic
        .host_object(aircraft_id)
        .expect("aircraft should exist");
    // C++ MSG_GET_REPAIRED for an airframe resolves through
    // JetAIUpdate::doLandingCommand (JetAIUpdate.cpp:2277) — the jet flies the
    // landing approach instead of entering the generic ground-vehicle
    // SeekingRepair dock state, and the jet path binds landing/RTB fields
    // rather than the generic order target. The contract under test is that
    // the airfield ACCEPTS the above-terrain aircraft (command not refused).
    assert_ne!(
        aircraft.ai_state,
        AIState::Idle,
        "airfield must accept GetRepaired for an above-terrain aircraft"
    );
}

#[test]
fn get_healed_command_targets_only_injured_infantry() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_heal_pad_template(&mut game_logic);

    let heal_pad_id = game_logic
        .create_object("TestHealPad", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("heal pad should be created");
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(8.0, 0.0, 0.0))
        .expect("infantry should be created");
    let vehicle_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(9.0, 0.0, 0.0))
        .expect("vehicle should be created");

    {
        let infantry = game_logic
            .host_object_mut(infantry_id)
            .expect("infantry should exist");
        let _ = infantry.take_damage(20.0);
    }
    {
        let vehicle = game_logic
            .host_object_mut(vehicle_id)
            .expect("vehicle should exist");
        let _ = vehicle.take_damage(80.0);
    }

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::GetHealed {
            target_id: heal_pad_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![infantry_id, vehicle_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let infantry = game_logic
        .host_object(infantry_id)
        .expect("infantry should exist");
    let vehicle = game_logic
        .host_object(vehicle_id)
        .expect("vehicle should exist");
    assert_eq!(infantry.ai_state, AIState::SeekingHealing);
    assert_ne!(vehicle.ai_state, AIState::SeekingHealing);
}

#[test]
fn get_healed_command_requires_heal_destination_type() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);

    let non_heal_structure = game_logic
        .create_object("TestBuilding", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("non-heal destination should be created");
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(8.0, 0.0, 0.0))
        .expect("infantry should be created");
    {
        let infantry = game_logic
            .host_object_mut(infantry_id)
            .expect("infantry should exist");
        let _ = infantry.take_damage(20.0);
    }

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::GetHealed {
            target_id: non_heal_structure,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![infantry_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let infantry = game_logic
        .host_object(infantry_id)
        .expect("infantry should exist");
    assert_ne!(infantry.ai_state, AIState::SeekingHealing);
    assert_ne!(infantry.target, Some(non_heal_structure));
}

#[test]
fn get_healed_command_rejects_under_construction_destination() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_heal_pad_template(&mut game_logic);

    let heal_pad_id = game_logic
        .create_object("TestHealPad", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("heal pad should be created");
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(8.0, 0.0, 0.0))
        .expect("infantry should be created");
    {
        let infantry = game_logic
            .host_object_mut(infantry_id)
            .expect("infantry should exist");
        let _ = infantry.take_damage(20.0);
    }
    {
        let heal_pad = game_logic
            .host_object_mut(heal_pad_id)
            .expect("heal pad should exist");
        heal_pad.set_status_under_construction(true);
    }

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::GetHealed {
            target_id: heal_pad_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![infantry_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let infantry = game_logic
        .host_object(infantry_id)
        .expect("infantry should exist");
    assert_ne!(infantry.ai_state, AIState::SeekingHealing);
    assert_ne!(infantry.target, Some(heal_pad_id));
}

#[test]
fn special_ability_state_without_pending_order_resets_to_idle() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let actor_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("actor should be created");
    let target_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(3.0, 0.0, 0.0))
        .expect("target should be created");

    {
        let actor = game_logic
            .host_object_mut(actor_id)
            .expect("actor should exist");
        actor.target = Some(target_id);
        actor.set_ai_state(AIState::SpecialAbility);
    }

    game_logic.update_ai(&[actor_id, target_id], 1.0 / 60.0);

    let actor = game_logic
        .host_object(actor_id)
        .expect("actor should exist");
    assert_eq!(actor.ai_state, AIState::Idle);
    assert!(actor.target.is_none());
}

#[test]
fn build_command_rejects_non_worker_constructor() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::USA);

    let tank_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("tank should be created");

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::DozerConstruct {
            template_name: "TestBuilding".to_string(),
            location: Vec3::new(20.0, 0.0, 20.0),
            orientation: 0.0,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![tank_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let created_structures = game_logic
        .host_objects()
        .values()
        .filter(|o| o.template_name == "TestBuilding")
        .count();
    assert_eq!(created_structures, 0);

    let tank = game_logic.host_object(tank_id).expect("tank should exist");
    assert_ne!(tank.ai_state, AIState::Constructing);
}

#[test]
fn dozer_line_assigns_each_worker_a_segment() {
    let mut game_logic = GameLogic::new();
    ensure_test_dozer_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);
    // C++ BuildAssistant::buildObjectLineNow tiles the line at
    // majorRadius*2 (BuildAssistant.cpp:441) and pre-checks each tile with
    // isLocationLegalToBuild during tiling (BuildAssistant.cpp:1173-1181);
    // buildObjectNow then places each tile with NO legality re-check. The
    // spacing/overlap contract under test is therefore geometry-driven, and
    // every retail structure template authors Geometry rows — author the
    // 10wu footprint so tiles sit at majorRadius*2 = 20wu, exactly touching
    // (dist 20 is NOT < place_r 10 + neighbour r 10), C++ isLocationClearOfObjects.
    use crate::game_logic::{HostGeometryInfo, HostGeometryType};
    game_logic
        .templates
        .get_mut("TestBuilding")
        .expect("TestBuilding template")
        .geometry_info = HostGeometryInfo {
        geom_type: HostGeometryType::Cylinder,
        is_small: false,
        height: 20.0,
        major_radius: 10.0,
        minor_radius: 10.0,
        authored: true,
    };
    ensure_test_player_for_team(&mut game_logic, Team::USA);

    // place_line_build_segment enforces the C++ exact-controller ownership
    // (builder.owner == issuing player), so the fixtures must spawn with the
    // player binding instead of faction-only ownerless spawns.
    let dozer_a = game_logic
        .create_object_for_player("TestDozer", 0, Vec3::new(0.0, 0.0, 0.0))
        .expect("dozer A should be created");
    let dozer_b = game_logic
        .create_object_for_player("TestDozer", 0, Vec3::new(2.0, 0.0, 0.0))
        .expect("dozer B should be created");

    let start = Vec3::new(10.0, 0.0, 10.0);
    let end = Vec3::new(30.0, 0.0, 10.0);
    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::DozerConstructLine {
            template_name: "TestBuilding".to_string(),
            start,
            end,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![dozer_a, dozer_b],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let dozer_a_state = game_logic
        .host_object(dozer_a)
        .expect("dozer A should exist");
    let dozer_b_state = game_logic
        .host_object(dozer_b)
        .expect("dozer B should exist");
    // C++ line build (BuildAssistant::buildObjectLineNow) places the line as
    // under-construction scaffolds first; DozerAIUpdate then WALKS to each
    // segment (AI_MOVE) and only flips to AI_CONSTRUCT/Constructing on
    // arrival, so the honest post-command state is Moving.
    assert_eq!(dozer_a_state.ai_state, AIState::Moving);
    assert_eq!(dozer_b_state.ai_state, AIState::Moving);

    let a_dest = dozer_a_state
        .movement
        .target_position
        .expect("dozer A should receive a line segment destination");
    let b_dest = dozer_b_state
        .movement
        .target_position
        .expect("dozer B should receive a line segment destination");
    // The walk target is a legal approach point beside the scaffold footprint
    // (A* cannot end inside the building's own static footprint), not the raw
    // segment centre. The contract under test is segment OWNERSHIP: worker A
    // is bound near the line start, worker B near the line end.
    assert!(
        a_dest.distance(start) < a_dest.distance(end),
        "dozer A must own the start segment (dest={a_dest:?})"
    );
    assert!(
        b_dest.distance(end) < b_dest.distance(start),
        "dozer B must own the end segment (dest={b_dest:?})"
    );

    let created_structures = game_logic
        .host_objects()
        .values()
        .filter(|o| o.template_name == "TestBuilding")
        .count();
    assert_eq!(created_structures, 2);
}

#[test]
fn hijack_transfers_vehicle_and_updates_team_color() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    // C++ hijack authority keys on the GLAInfantryHijacker/HijackerUpdate
    // module owner, not an arbitrary vehicle: the executor and the pending
    // ability drain both require the hijacker basename (abilities.rs
    // execute_hijack / update.rs drain gate), so the fixture must author one.
    let mut hijacker_tpl = crate::game_logic::ThingTemplate::new("TestHijacker");
    hijacker_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    game_logic
        .templates
        .insert("TestHijacker".to_string(), hijacker_tpl);

    let hijacker_id = game_logic
        .create_object("TestHijacker", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("hijacker should be created");
    let target_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(4.0, 0.0, 0.0))
        .expect("target should be created");

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::Hijack { target_id },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![hijacker_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    game_logic.update_ai(&[hijacker_id, target_id], 1.0 / 60.0);

    let target = game_logic
        .host_object(target_id)
        .expect("target should exist");
    assert_eq!(target.team, Team::USA);
    assert_eq!(target.team_color, Team::USA.get_color());
    assert!(
        target.status.hijacked,
        "hijack residual must set OBJECT_STATUS_HIJACKED"
    );
    assert!(target.is_hijacked(), "hijack residual is_hijacked helper");
    assert!(game_logic.honesty_hijack_ok(), "hijack residual honesty");
    assert_eq!(
        game_logic.car_bomb_residual().hijacks,
        1,
        "hijack honesty counter"
    );

    let hijacker = game_logic
        .host_object(hijacker_id)
        .expect("hijacker should exist");
    assert!(
        hijacker.status.destroyed,
        "hijacker infantry consumed after steal"
    );
}

#[test]
fn hijack_rejects_already_hijacked_vehicle() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let hijacker_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("hijacker should be created");
    let target_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(4.0, 0.0, 0.0))
        .expect("target should be created");
    {
        let target = game_logic
            .host_object_mut(target_id)
            .expect("target should exist");
        target.apply_hijacked();
    }

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::Hijack { target_id },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![hijacker_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    game_logic.update_ai(&[hijacker_id, target_id], 1.0 / 60.0);

    let target = game_logic
        .host_object(target_id)
        .expect("target should exist");
    assert_eq!(
        target.team,
        Team::GLA,
        "already-hijacked vehicle must not re-transfer"
    );
    assert!(!game_logic.honesty_hijack_ok());
    let hijacker = game_logic
        .host_object(hijacker_id)
        .expect("hijacker should exist");
    assert!(
        !hijacker.status.destroyed,
        "failed re-hijack must not consume infantry"
    );
}

#[test]
fn hijack_command_applies_only_after_unit_reaches_target() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    // Same hijacker-basename residual as the transfers test: the executor
    // refuses an arbitrary vehicle as the hijacking unit.
    let mut hijacker_tpl = crate::game_logic::ThingTemplate::new("TestHijacker");
    hijacker_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    game_logic
        .templates
        .insert("TestHijacker".to_string(), hijacker_tpl);

    let hijacker_id = game_logic
        .create_object("TestHijacker", Team::USA, Vec3::new(150.0, 0.0, 0.0))
        .expect("hijacker should be created");
    let target_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("target should be created");

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::Hijack { target_id },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![hijacker_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let target_after_command = game_logic
        .host_object(target_id)
        .expect("target should exist");
    assert_eq!(
        target_after_command.team,
        Team::GLA,
        "hijack should not transfer target immediately on command issue"
    );

    game_logic.update_ai(&[hijacker_id, target_id], 1.0 / 60.0);
    let target_after_far_update = game_logic
        .host_object(target_id)
        .expect("target should exist");
    assert_eq!(
        target_after_far_update.team,
        Team::GLA,
        "hijack should stay pending while hijacker is out of range"
    );

    {
        let hijacker = game_logic
            .host_object_mut(hijacker_id)
            .expect("hijacker should exist");
        hijacker.set_position(Vec3::new(2.0, 0.0, 0.0));
        hijacker.set_ai_state(AIState::SpecialAbility);
        hijacker.target = Some(target_id);
    }
    game_logic.update_ai(&[hijacker_id, target_id], 1.0 / 60.0);

    let target_after_contact = game_logic
        .host_object(target_id)
        .expect("target should exist");
    assert_eq!(target_after_contact.team, Team::USA);

    let hijacker = game_logic
        .host_object(hijacker_id)
        .expect("hijacker should exist");
    assert!(hijacker.status.destroyed);
}

#[test]
fn sabotage_command_applies_only_after_unit_reaches_target() {
    let mut game_logic = GameLogic::new();
    ensure_test_saboteur_template(&mut game_logic);
    ensure_test_power_plant_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    ensure_test_player_for_team(&mut game_logic, Team::USA);

    let saboteur_id = game_logic
        .create_object("GLAInfantrySaboteur", Team::GLA, Vec3::new(150.0, 0.0, 0.0))
        .expect("saboteur should be created");
    let target_id = game_logic
        .create_object("AmericaPowerPlant", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("target should be created");

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::Sabotage { target_id },
        player_id: 2,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![saboteur_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    // Not yet: out of range.
    assert!(!game_logic.honesty_saboteur_power_ok());
    assert_eq!(
        game_logic
            .get_player_mut_by_team(Team::USA)
            .map(|p| p.power_sabotaged_till_frame)
            .unwrap_or(0),
        0
    );

    game_logic.update_ai(&[saboteur_id, target_id], 1.0 / 60.0);
    assert!(!game_logic.honesty_saboteur_power_ok());

    {
        let saboteur = game_logic
            .host_object_mut(saboteur_id)
            .expect("saboteur should exist");
        saboteur.set_position(Vec3::new(2.0, 0.0, 0.0));
        saboteur.set_ai_state(AIState::SpecialAbility);
        saboteur.target = Some(target_id);
    }
    game_logic.frame = 30;
    game_logic.update_ai(&[saboteur_id, target_id], 1.0 / 60.0);

    assert!(
        game_logic.honesty_saboteur_power_ok(),
        "power sabotage residual must apply on reach"
    );
    let until = game_logic
        .get_player_mut_by_team(Team::USA)
        .map(|p| p.power_sabotaged_till_frame)
        .unwrap_or(0);
    assert!(
        until > 30,
        "power_sabotaged_till_frame must be set (until={until})"
    );
    // Saboteur consumed residual.
    let sab_alive = game_logic
        .host_object(saboteur_id)
        .map(|s| s.is_alive() && !s.status.destroyed)
        .unwrap_or(false);
    assert!(!sab_alive, "saboteur must be consumed on success");
}

#[test]
fn sabotage_command_rejects_non_structure_targets() {
    let mut game_logic = GameLogic::new();
    ensure_test_saboteur_template(&mut game_logic);
    ensure_test_tank_template(&mut game_logic);

    let saboteur_id = game_logic
        .create_object("GLAInfantrySaboteur", Team::GLA, Vec3::new(10.0, 0.0, 0.0))
        .expect("saboteur should be created");
    let target_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("target should be created");

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::Sabotage { target_id },
        player_id: 2,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![saboteur_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let saboteur = game_logic
        .host_object(saboteur_id)
        .expect("saboteur should exist");
    assert_ne!(saboteur.ai_state, AIState::SpecialAbility);
    assert_ne!(saboteur.target, Some(target_id));
}

#[test]
fn saboteur_military_factory_residual_disables_production() {
    let mut game_logic = GameLogic::new();
    ensure_test_saboteur_template(&mut game_logic);
    ensure_test_war_factory_template(&mut game_logic);

    let saboteur_id = game_logic
        .create_object("GLAInfantrySaboteur", Team::GLA, Vec3::new(2.0, 0.0, 0.0))
        .expect("saboteur");
    let target_id = game_logic
        .create_object("AmericaWarFactory", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("factory");

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::Sabotage { target_id },
        player_id: 2,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![saboteur_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    {
        let s = game_logic.host_object_mut(saboteur_id).unwrap();
        s.set_position(Vec3::new(1.0, 0.0, 0.0));
        s.set_ai_state(AIState::SpecialAbility);
        s.target = Some(target_id);
    }
    game_logic.frame = 10;
    game_logic.update_ai(&[saboteur_id, target_id], 1.0 / 60.0);

    assert!(
        game_logic.honesty_saboteur_military_ok(),
        "military factory sabotage residual honesty"
    );
    let factory = game_logic.host_object(target_id).expect("factory");
    assert!(
        factory.is_hacked_disabled() || factory.status.disabled_hacked,
        "factory must be DISABLED_HACKED residual"
    );
    assert!(
        factory.status.disabled_hacked_until_frame > 10,
        "disable timer residual"
    );
}

#[test]
fn sabotage_command_rejects_non_saboteur_units() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_power_plant_template(&mut game_logic);

    let tank_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(10.0, 0.0, 0.0))
        .expect("tank");
    let target_id = game_logic
        .create_object("AmericaPowerPlant", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("plant");

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::Sabotage { target_id },
        player_id: 2,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![tank_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    let tank = game_logic.host_object(tank_id).expect("tank");
    assert_ne!(tank.ai_state, AIState::SpecialAbility);
    assert!(!game_logic.honesty_saboteur_ok());
}

#[test]
fn snipe_vehicle_command_applies_only_after_unit_reaches_target() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let sniper_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(300.0, 0.0, 0.0))
        .expect("sniper should be created");
    let target_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("target should be created");

    let initial_health = game_logic
        .host_object(target_id)
        .expect("target should exist")
        .health
        .current;

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::SnipeVehicle { target_id },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![sniper_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let target_after_command = game_logic
        .host_object(target_id)
        .expect("target should exist");
    assert_eq!(
        target_after_command.health.current, initial_health,
        "snipe should not apply immediately on command issue"
    );
    assert!(
        !target_after_command.is_unmanned(),
        "vehicle must remain manned until sniper resolves"
    );

    game_logic.update_ai(&[sniper_id, target_id], 1.0 / 60.0);
    let target_after_far_update = game_logic
        .host_object(target_id)
        .expect("target should exist");
    assert_eq!(
        target_after_far_update.health.current, initial_health,
        "snipe should be pending while sniper is out of KILLPILOT range"
    );
    assert!(!target_after_far_update.is_unmanned());

    {
        let sniper = game_logic
            .host_object_mut(sniper_id)
            .expect("sniper should exist");
        // C++ MSG_DO_WEAPON_AT_OBJECT uses GLAJarmenKellVehiclePilotSniperRifle
        // AttackRange 225, not contact radii+4.
        sniper.set_position(Vec3::new(200.0, 0.0, 0.0));
        sniper.set_ai_state(AIState::SpecialAbility);
        sniper.target = Some(target_id);
    }
    game_logic.update_ai(&[sniper_id, target_id], 1.0 / 60.0);

    let target_after_contact = game_logic
        .host_object(target_id)
        .expect("target should exist");
    // C++ DAMAGE_KILLPILOT residual: no HP damage; vehicle unmanned + Neutral.
    assert_eq!(
        target_after_contact.health.current, initial_health,
        "kill-pilot residual must not damage vehicle HP"
    );
    assert!(
        target_after_contact.is_unmanned(),
        "snipe must leave vehicle unmanned"
    );
    assert_eq!(
        target_after_contact.team,
        Team::Neutral,
        "sniped vehicle becomes Neutral (gray/unowned)"
    );
    assert!(
        !target_after_contact.can_move(),
        "unmanned vehicle cannot move"
    );
    assert!(
        game_logic.honesty_snipe_vehicle_ok(),
        "snipe residual honesty"
    );
}

#[test]
fn retail_pilot_metadata_drives_starting_veteran_and_same_owner_recrew() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::VeterancyLevel;
    use std::path::Path;

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    game_logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA 0", true));
    game_logic
        .players
        .insert(1, Player::new(1, Team::USA, "USA 1", true));

    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("Main crate must remain three levels below repository root");
    let source = std::fs::read_to_string(
        repo_root
            .join("windows_game/extracted_big_files_v2/INIZH/Data/INI/Object/AmericaInfantry.ini"),
    )
    .expect("retail AmericaInfantry.ini");
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(&source, "AmericaInfantry.ini")
        .expect("parse retail America infantry");
    let pilot_tpl = GameLogic::build_template_from_object_definition(
        "AmericaInfantryPilot",
        parser
            .get_definition("AmericaInfantryPilot")
            .expect("retail pilot definition"),
        None,
    );
    let metadata = pilot_tpl
        .veterancy_crate_collide
        .expect("retail IsPilot metadata");
    assert!(metadata.supports_pilot_recrew());
    assert_eq!(
        metadata.pilot_starting_level(),
        Some(VeterancyLevel::Veteran)
    );
    game_logic
        .templates
        .insert("AmericaInfantryPilot".to_string(), pilot_tpl);

    let pilot_id = game_logic
        .create_object_for_player("AmericaInfantryPilot", 0, Vec3::new(2.0, 0.0, 0.0))
        .expect("pilot");
    {
        let p = game_logic.host_object_mut(pilot_id).expect("pilot obj");
        assert_eq!(p.experience.level, VeterancyLevel::Veteran);
        assert_eq!(p.owner_player_id, Some(0));
    }

    // A pilot-named template with no parsed behavior remains ordinary
    // infantry: it starts Rookie and cannot create an Enter order.
    let mut name_only = ThingTemplate::new("AmericaInfantryPilotNameOnly");
    name_only
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    game_logic
        .templates
        .insert("AmericaInfantryPilotNameOnly".to_string(), name_only);
    let name_only_id = game_logic
        .create_object_for_player("AmericaInfantryPilotNameOnly", 0, Vec3::new(2.0, 0.0, 2.0))
        .expect("name-only pilot");
    assert_eq!(
        game_logic
            .host_object(name_only_id)
            .expect("name-only object")
            .experience
            .level,
        VeterancyLevel::Rookie,
        "missing IsPilot metadata must fail closed for starting veterancy"
    );
    // A same-faction but different controlling player refuses the PILOT
    // RECREW flavor: C++ VeterancyCrateCollide::isValidToExecute requires
    // other->getControllingPlayer() == getObject()->getControllingPlayer()
    // for m_isPilot.  KillPilot neutralizes the target while preserving
    // owner #1 in ObjectStatus.  The generic Enter order itself is a
    // separate authority: C++ canEnterObject lets ANY non-REJECT_UNMANNED
    // infantry order-enter ANY DISABLED_UNMANNED husk with no controller
    // check (ActionManager.cpp:549-557), so the retail contract is asserted
    // on the two query gates without installing an order.
    let foreign_tank_id = game_logic
        .create_object_for_player("TestTank", 1, Vec3::new(0.0, 0.0, 0.0))
        .expect("foreign tank");
    {
        let t = game_logic
            .host_object_mut(foreign_tank_id)
            .expect("foreign tank object");
        t.apply_kill_pilot_unmanned();
        t.set_team(Team::Neutral);
        t.experience.level = VeterancyLevel::Rookie;
        assert_eq!(t.status.unmanned_owner_player_id, Some(1));
    }
    assert!(
        !game_logic.can_execute_pilot_recrew(pilot_id, foreign_tank_id),
        "pilot recrew flavor must refuse a foreign controlling player"
    );
    assert!(
        game_logic.can_execute_infantry_unmanned_recrew(pilot_id, foreign_tank_id),
        "generic infantry husk takeover stays available (retail any-infantry steal)"
    );
    assert!(
        game_logic.host_object(foreign_tank_id).unwrap().is_unmanned(),
        "precondition: unmanned vehicle, no order installed"
    );


    // The name-only impostor has no parsed IsPilot authority, so the PILOT
    // RECREW flavor refuses it even for a same-controller target
    // (VeterancyCrateCollide.cpp:56-61: no module data → no levelsToGain →
    // invalid).  The generic husk-takeover order itself stays available —
    // C++ canEnterObject authorizes ANY non-REJECT_UNMANNED infantry into a
    // DISABLED_UNMANNED husk (ActionManager.cpp:549-557) — so the retail
    // split is asserted on the two query gates without installing an order.
    let tank_id = game_logic
        .create_object_for_player("TestTank", 0, Vec3::new(0.0, 0.0, 0.0))
        .expect("own tank");
    {
        let t = game_logic
            .host_object_mut(tank_id)
            .expect("own tank object");
        t.apply_kill_pilot_unmanned();
        t.set_team(Team::Neutral);
        t.experience.level = VeterancyLevel::Rookie;
        assert_eq!(t.status.unmanned_owner_player_id, Some(0));
    }
    assert!(
        !game_logic.can_execute_pilot_recrew(name_only_id, tank_id),
        "a template name is not IsPilot authority"
    );
    assert!(
        game_logic.can_execute_infantry_unmanned_recrew(name_only_id, tank_id),
        "generic infantry husk takeover stays available (retail any-infantry steal)"
    );
    assert!(
        game_logic.host_object(tank_id).unwrap().is_unmanned(),
        "precondition: unmanned vehicle, no order installed"
    );

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter { target_id: tank_id },
        player_id: 0,
        command_id: 3,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![pilot_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    {
        let p = game_logic.host_object(pilot_id).expect("pilot after cmd");
        assert_eq!(p.ai_state, AIState::Entering);
        assert_eq!(p.target, Some(tank_id));
    }

    game_logic.update_ai(&[pilot_id, tank_id], 1.0 / 30.0);

    let tank = game_logic.host_object(tank_id).expect("tank after recrew");
    assert!(!tank.is_unmanned(), "recrew must clear DISABLED_UNMANNED");
    assert_eq!(tank.team, Team::USA, "recrew transfers pilot team");
    assert_eq!(
        tank.owner_player_id,
        Some(0),
        "recrew restores the exact controlling player, not only its faction"
    );
    assert_eq!(
        tank.experience.level,
        VeterancyLevel::Veteran,
        "pilot veterancy must transfer onto vehicle"
    );
    assert!(game_logic.honesty_pilot_recrew_ok(), "pilot recrew honesty");
    assert!(
        game_logic.honesty_pilot_veterancy_transfer_ok(),
        "veterancy transfer honesty"
    );
    assert_eq!(game_logic.usa_pilot_residual().recrews, 1);

    let pilot = game_logic
        .host_object(pilot_id)
        .expect("pilot after recrew");
    // `SlowDeathBehavior` can keep the corpse in the object map for its
    // authored delay, but it is no longer a live/controllable pilot.
    assert!(
        !pilot.is_alive(),
        "pilot infantry must be consumed even when its authored SlowDeath defers removal"
    );
}

#[test]
fn live_host_injects_completed_waypoint_labels_and_shroud_discovered_by() {
    use gamelogic::common::ObjectShroudStatus;
    use gamelogic::object::registry::OBJECT_REGISTRY;
    use gamelogic::scripting::{
        clear_host_script_query_snapshot, host_script_query_object, host_script_query_object_by_id,
    };
    use gamelogic::system::shroud_manager::get_shroud_manager;

    OBJECT_REGISTRY.clear();
    clear_host_script_query_snapshot();

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(1, Team::USA, "PlyrAmerica", true));
    logic.add_player(Player::new(2, Team::China, "PlyrChina", false));
    let mut t = ThingTemplate::new("NamedScout");
    t.set_health(100.0);
    logic.templates.insert("NamedScout".into(), t);
    let id = logic
        .create_object_for_player("NamedScout", 2, Vec3::new(10.0, 0.0, 20.0))
        .expect("unit");
    if let Some(o) = logic.host_object_mut(id) {
        o.name = "MapNamedScout".into();
        o.completed_waypoint_labels = vec!["HeroPath".into()];
    }
    {
        let shroud_manager = get_shroud_manager();
        let mut shroud = shroud_manager.lock().expect("shroud");
        shroud.set_host_object_shroud_status(1, id.0, ObjectShroudStatus::Shrouded);
        shroud.set_host_object_shroud_status(2, id.0, ObjectShroudStatus::Clear);
    }
    logic.inject_host_script_query_snapshot();
    let obj = host_script_query_object("MapNamedScout").expect("snapshot");
    assert_eq!(obj.waypoint_labels, vec!["HeroPath".to_string()]);
    assert!(
        obj.discovered_by
            .iter()
            .any(|n| n.eq_ignore_ascii_case("PlyrChina")),
        "owner is CLEAR"
    );
    assert!(
        !obj.discovered_by
            .iter()
            .any(|n| n.eq_ignore_ascii_case("PlyrAmerica")),
        "shrouded player must not discover"
    );
    assert_eq!(
        host_script_query_object_by_id(id.0).map(|o| o.id),
        Some(id.0)
    );
    assert!(OBJECT_REGISTRY.is_empty());
    clear_host_script_query_snapshot();
}

#[test]
fn live_host_from_named_and_skirmish_conditions_use_inject() {
    use gamelogic::object::registry::OBJECT_REGISTRY;
    use gamelogic::player::player_list;
    use gamelogic::scripting::clear_host_script_query_snapshot;
    use gamelogic::scripting::core::{Condition, ConditionType, Parameter, ParameterType};
    use gamelogic::scripting::engine::{
        get_named_object_tracker, initialize_script_engine, with_script_engine_mut,
    };
    use gamelogic::scripting::executor::{
        ScriptConditionEvaluator, ScriptConditionResult, ScriptContext,
    };
    use std::sync::{Arc, RwLock};

    OBJECT_REGISTRY.clear();
    clear_host_script_query_snapshot();
    initialize_script_engine().expect("script engine");
    player_list().write().unwrap().clear();
    let leftover = Arc::new(RwLock::new(gamelogic::player::Player::new(1)));
    leftover.write().unwrap().set_display_name("PlyrAmerica");
    player_list().write().unwrap().add_player(leftover);

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(1, Team::USA, "PlyrAmerica", true));
    logic.add_player(Player::new(2, Team::China, "PlyrChina", false));
    let mut t = ThingTemplate::new("NamedCannon");
    t.set_health(1000.0);
    logic.templates.insert("NamedCannon".into(), t);
    let id = logic
        .create_object_for_player("NamedCannon", 1, Vec3::new(5.0, 0.0, 5.0))
        .expect("cannon");
    if let Some(o) = logic.host_object_mut(id) {
        o.name = "ParticleCannon".into();
        o.team_instance_name = "USA_Superweapons".into();
        o.completed_waypoint_labels = vec!["HeroPath".into()];
    }
    logic.inject_host_named_unit_map_into_crate_tracker();

    let completed = with_script_engine_mut(|engine| {
        engine.notify_of_triggered_special_power(1, "SuperweaponParticleUplinkCannon", id.0);
        let mut from_named = Condition::new(ConditionType::PlayerTriggeredSpecialPowerFromNamed);
        from_named
            .add_parameter(Parameter::with_string(
                ParameterType::Side,
                "PlyrAmerica".into(),
            ))
            .unwrap();
        from_named
            .add_parameter(Parameter::with_string(
                ParameterType::SpecialPower,
                "SuperweaponParticleUplinkCannon".into(),
            ))
            .unwrap();
        from_named
            .add_parameter(Parameter::with_string(
                ParameterType::Unit,
                "ParticleCannon".into(),
            ))
            .unwrap();
        let mut evaluator =
            ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));
        assert_eq!(
            evaluator.evaluate_condition(&mut from_named).unwrap(),
            ScriptConditionResult::True
        );

        let mut reached = Condition::new(ConditionType::NamedReachedWaypointsEnd);
        reached
            .add_parameter(Parameter::with_string(
                ParameterType::Unit,
                "ParticleCannon".into(),
            ))
            .unwrap();
        reached
            .add_parameter(Parameter::with_string(
                ParameterType::WaypointPath,
                "HeroPath".into(),
            ))
            .unwrap();
        assert_eq!(
            evaluator.evaluate_condition(&mut reached).unwrap(),
            ScriptConditionResult::True
        );

        let mut team_reached = Condition::new(ConditionType::TeamReachedWaypointsEnd);
        team_reached
            .add_parameter(Parameter::with_string(
                ParameterType::Team,
                "USA_Superweapons".into(),
            ))
            .unwrap();
        team_reached
            .add_parameter(Parameter::with_string(
                ParameterType::WaypointPath,
                "HeroPath".into(),
            ))
            .unwrap();
        assert_eq!(
            evaluator.evaluate_condition(&mut team_reached).unwrap(),
            ScriptConditionResult::True
        );
    });

    player_list().write().unwrap().clear();
    clear_host_script_query_snapshot();
    get_named_object_tracker().clear().ok();
    assert_eq!(completed, Some(()));
    assert!(OBJECT_REGISTRY.is_empty());
}

#[test]
fn live_named_set_attitude_drains_to_host_ai() {
    use crate::game_logic::host_strategy_center::HostAiAttitude;
    use gamelogic::object::registry::OBJECT_REGISTRY;
    use gamelogic::scripting::{
        HostScriptNamedAttitudeRequest, request_host_script_named_attitude,
    };

    OBJECT_REGISTRY.clear();
    drain_script_act_b_queues();

    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("Hero");
    t.set_health(100.0);
    logic.templates.insert("Hero".into(), t);
    let id = logic
        .create_object("Hero", Team::USA, Vec3::new(8.0, 0.0, 4.0))
        .expect("hero");
    if let Some(o) = logic.host_object_mut(id) {
        o.name = "Hero".into();
        o.set_ai_attitude_i8(0);
    }

    request_host_script_named_attitude(HostScriptNamedAttitudeRequest {
        unit: "Hero".into(),
        mood: 2,
    });
    logic.scripts_loaded = true;
    logic.evaluate_and_execute_scripts(0.0);

    assert_eq!(
        logic.host_object(id).expect("hero").ai_attitude(),
        HostAiAttitude::Aggressive,
        "NAMED_SET_ATTITUDE must call host setAttitude (Aggressive=2)"
    );
    assert!(OBJECT_REGISTRY.is_empty());
}

#[test]
fn live_set_stopping_distance_drains_and_arrives_at_scripted_band() {
    use gamelogic::object::registry::OBJECT_REGISTRY;
    use gamelogic::scripting::{
        HostScriptStoppingDistanceRequest, request_host_script_stopping_distance,
    };

    OBJECT_REGISTRY.clear();
    drain_script_act_b_queues();

    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("Scout");
    t.set_health(100.0);
    logic.templates.insert("Scout".into(), t);
    let id = logic
        .create_object("Scout", Team::USA, Vec3::ZERO)
        .expect("scout");
    if let Some(o) = logic.host_object_mut(id) {
        o.name = "Scout".into();
        o.team_instance_name = "TeamA".into();
        o.request_path(Vec3::new(10.0, 0.0, 0.0), None);
    }

    request_host_script_stopping_distance(HostScriptStoppingDistanceRequest::Team {
        team: "TeamA".into(),
        distance: 25.0,
    });
    logic.scripts_loaded = true;
    logic.evaluate_and_execute_scripts(0.0);

    {
        let obj = logic.host_object(id).expect("scout");
        assert_eq!(
            obj.close_enough_dist,
            Some(25.0),
            "SET_STOPPING_DISTANCE must stamp Locomotor::setCloseEnoughDist"
        );
    }

    if let Some(o) = logic.host_object_mut(id) {
        o.update_movement(1.0 / 30.0);
        assert!(
            o.movement.target_position.is_none(),
            "scripted closeEnoughDist 25 must stop a 10wu goal"
        );
    }

    request_host_script_stopping_distance(HostScriptStoppingDistanceRequest::Team {
        team: "TeamA".into(),
        distance: 0.25,
    });
    logic.evaluate_and_execute_scripts(0.0);
    assert_eq!(
        logic.host_object(id).expect("scout").close_enough_dist,
        Some(25.0),
        "C++ ignores stoppingDistance < 0.5"
    );
    assert!(OBJECT_REGISTRY.is_empty());
}

#[test]
fn live_set_stopping_distance_aborts_at_first_member_without_locomotor() {
    use gamelogic::object::registry::OBJECT_REGISTRY;
    use gamelogic::scripting::{
        HostScriptStoppingDistanceRequest, request_host_script_stopping_distance,
    };

    OBJECT_REGISTRY.clear();
    drain_script_act_b_queues();

    let mut logic = GameLogic::new();
    let mut bunker_t = ThingTemplate::new("StopDistBunker");
    bunker_t.add_kind_of(KindOf::Structure);
    bunker_t.set_health(400.0);
    logic.templates.insert("StopDistBunker".into(), bunker_t);
    let mut ranger_t = ThingTemplate::new("StopDistRanger");
    ranger_t.add_kind_of(KindOf::Infantry);
    ranger_t.set_health(100.0);
    logic.templates.insert("StopDistRanger".into(), ranger_t);

    // Lower ObjectId first: C++ team-list order / live sort-by-id.
    let bunker = logic
        .create_object("StopDistBunker", Team::USA, Vec3::ZERO)
        .expect("bunker");
    let ranger = logic
        .create_object("StopDistRanger", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .expect("ranger");
    if let Some(o) = logic.host_object_mut(bunker) {
        o.team_instance_name = "TeamA".into();
    }
    if let Some(o) = logic.host_object_mut(ranger) {
        o.team_instance_name = "TeamA".into();
        o.close_enough_dist = Some(1.0);
    }

    request_host_script_stopping_distance(HostScriptStoppingDistanceRequest::Team {
        team: "TeamA".into(),
        distance: 25.0,
    });
    logic.scripts_loaded = true;
    logic.evaluate_and_execute_scripts(0.0);

    assert_eq!(
        logic.host_object(bunker).expect("bunker").close_enough_dist,
        None,
        "C++ returns at first member without locomotor"
    );
    assert_eq!(
        logic.host_object(ranger).expect("ranger").close_enough_dist,
        Some(1.0),
        "members after the first structure keep the old closeEnoughDist"
    );
    assert!(OBJECT_REGISTRY.is_empty());
}

#[test]
fn live_named_enter_and_exit_all_drain_ai_enter_evacuate() {
    use crate::game_logic::AIState;
    use gamelogic::object::registry::OBJECT_REGISTRY;
    use gamelogic::scripting::{
        HostScriptGarrisonEnterExitRequest, request_host_script_garrison_enter,
        take_host_script_garrison_enter_requests,
    };

    OBJECT_REGISTRY.clear();
    let _ = take_host_script_garrison_enter_requests();

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(1, Team::USA, "PlyrAmerica", true));
    let mut ranger = ThingTemplate::new("GarrEnterRanger");
    ranger.add_kind_of(KindOf::Infantry);
    ranger.set_health(100.0);
    logic.templates.insert("GarrEnterRanger".into(), ranger);
    let mut humvee = ThingTemplate::new("GarrEnterHumvee");
    humvee.add_kind_of(KindOf::Vehicle);
    humvee.add_kind_of(KindOf::Transport);
    humvee.set_health(400.0);
    logic.templates.insert("GarrEnterHumvee".into(), humvee);

    let infantry = logic
        .create_object("GarrEnterRanger", Team::USA, Vec3::new(50.0, 0.0, 50.0))
        .expect("infantry");
    if let Some(obj) = logic.host_object_mut(infantry) {
        obj.name = "NamedRanger".into();
        obj.owner_player_id = Some(1);
    }
    let transport = logic
        .create_object("GarrEnterHumvee", Team::USA, Vec3::new(80.0, 0.0, 50.0))
        .expect("transport");
    if let Some(obj) = logic.host_object_mut(transport) {
        obj.name = "NamedHumvee".into();
        obj.owner_player_id = Some(1);
        obj.max_transport = 8;
    }

    request_host_script_garrison_enter(HostScriptGarrisonEnterExitRequest::NamedEnter {
        unit: "NamedRanger".into(),
        dest: "NamedHumvee".into(),
    });
    logic.apply_host_garrison_enter_exit_script_requests();

    let infantry_after = logic.host_object(infantry).expect("infantry after enter");
    assert_eq!(
        infantry_after.target,
        Some(transport),
        "NAMED_ENTER_NAMED must leftover-drain aiEnter"
    );
    assert_eq!(infantry_after.ai_state, AIState::Entering);

    if let Some(obj) = logic.host_object_mut(transport) {
        obj.occupants.push(infantry);
    }
    if let Some(obj) = logic.host_object_mut(infantry) {
        obj.set_contained_by(Some(transport));
        obj.set_ai_state(AIState::Idle);
        obj.target = None;
    }

    request_host_script_garrison_enter(HostScriptGarrisonEnterExitRequest::NamedExitAll {
        unit: "NamedHumvee".into(),
    });
    logic.apply_host_garrison_enter_exit_script_requests();

    let infantry_after_exit = logic.host_object(infantry).expect("infantry after exit");
    assert!(
        infantry_after_exit.contained_by.is_none(),
        "NAMED_EXIT_ALL must leftover-drain aiEvacuate"
    );
    assert!(OBJECT_REGISTRY.is_empty());
}

#[test]
fn live_team_garrison_specific_building_drains_ai_enter() {
    use crate::game_logic::AIState;
    use crate::game_logic::{BuildingData, BuildingType};
    use gamelogic::object::registry::OBJECT_REGISTRY;
    use gamelogic::scripting::{
        HostScriptGarrisonEnterExitRequest, request_host_script_garrison_enter,
        take_host_script_garrison_enter_requests,
    };

    OBJECT_REGISTRY.clear();
    let _ = take_host_script_garrison_enter_requests();

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(1, Team::USA, "PlyrAmerica", true));
    let mut ranger = ThingTemplate::new("GarrBunkRanger");
    ranger.add_kind_of(KindOf::Infantry);
    ranger.set_health(100.0);
    logic.templates.insert("GarrBunkRanger".into(), ranger);
    let mut bunker_t = ThingTemplate::new("GarrBunk");
    bunker_t.add_kind_of(KindOf::Structure);
    bunker_t.set_health(800.0);
    logic.templates.insert("GarrBunk".into(), bunker_t);

    let infantry = logic
        .create_object("GarrBunkRanger", Team::USA, Vec3::new(20.0, 0.0, 20.0))
        .expect("infantry");
    if let Some(obj) = logic.host_object_mut(infantry) {
        obj.owner_player_id = Some(1);
        obj.team_instance_name = "USA_Infantry".into();
    }
    let bunker = logic
        .create_object("GarrBunk", Team::USA, Vec3::new(60.0, 0.0, 20.0))
        .expect("bunker");
    if let Some(obj) = logic.host_object_mut(bunker) {
        obj.name = "NamedBunker".into();
        obj.owner_player_id = Some(1);
        let mut bd = BuildingData::new(BuildingType::Bunker);
        bd.max_garrison = 5;
        obj.building_data = Some(bd);
    }

    request_host_script_garrison_enter(HostScriptGarrisonEnterExitRequest::TeamGarrisonSpecific {
        team: "USA_Infantry".into(),
        building: "NamedBunker".into(),
    });
    logic.apply_host_garrison_enter_exit_script_requests();

    let infantry_after = logic.host_object(infantry).expect("infantry after");
    assert_eq!(
        infantry_after.target,
        Some(bunker),
        "TEAM_GARRISON_SPECIFIC_BUILDING must leftover-drain aiEnter"
    );
    assert_eq!(infantry_after.ai_state, AIState::Entering);
    assert!(OBJECT_REGISTRY.is_empty());
}
