//! Behavior suite extracted from `ocl_and_bombs`.
use super::*;

#[test]
fn dozer_acquires_admitted_mine_not_retail_named_non_mine() {
    let source = "Object TestDozer\n  KindOf = VEHICLE SELECTABLE DOZER SCORE\n  Body = ActiveBody BodyTag\n    MaxHealth = 300\n    InitialHealth = 300\n  End\nEnd\nObject RenamedBuriedCharge\n  KindOf = MINE\n  Body = ActiveBody BodyTag\n    MaxHealth = 100\n    InitialHealth = 100\n  End\n  Behavior = MinefieldBehavior MineTag\n    NumVirtualMines = 3\n  End\nEnd\nObject ChinaStandardMine\n  Body = ActiveBody BodyTag\n    MaxHealth = 100\n    InitialHealth = 100\n  End\nEnd\n";
    let mut parser = crate::assets::IniParser::new();
    parser.parse_ini_content(source, "mine-acquire.ini").unwrap();
    let mut world = GameLogic::new();
    ensure_test_player_for_team(&mut world, Team::USA);
    ensure_test_player_for_team(&mut world, Team::GLA);
    world.get_player_mut(0).unwrap().set_map_relationship(2, game_engine::common::game_common::Relationship::Enemies);
    world.get_player_mut(2).unwrap().set_map_relationship(0, game_engine::common::game_common::Relationship::Enemies);
    for name in ["TestDozer", "RenamedBuriedCharge", "ChinaStandardMine"] {
        let definition = parser.get_definition(name).unwrap();
        let template = GameLogic::build_template_from_object_definition(name, definition, None);
        world.templates.insert(name.into(), template);
    }
    world.admit_receiving_module_content(
        crate::assets::ReceivingObjectContent::from_sources([
            ("mine-acquire.ini".into(), source.to_owned()),
        ]).unwrap(),
    ).unwrap();
    let dozer = world.create_object("TestDozer", Team::USA, Vec3::ZERO).unwrap();
    assert!(world.host_object(dozer).unwrap().is_alive());
    let impostor = world.create_object("ChinaStandardMine", Team::GLA, Vec3::new(5.0, 0.0, 0.0)).unwrap();
    assert!(world.host_object(impostor).unwrap().mine_data.is_none());
    assert_eq!(world.find_dozer_bored_mine_target(dozer), None);
    let allied = world.create_object("RenamedBuriedCharge", Team::USA, Vec3::new(6.0, 0.0, 0.0)).unwrap();
    assert_eq!(world.host_object(allied).unwrap().mine_data.as_ref().unwrap().num_virtual_mines, 3);
    assert_eq!(world.find_dozer_bored_mine_target(dozer), None);
    let enemy = world.create_object("RenamedBuriedCharge", Team::GLA, Vec3::new(10.0, 0.0, 0.0)).unwrap();
    assert!(world.host_object(enemy).unwrap().is_alive());
    assert_eq!(world.find_dozer_bored_mine_target(dozer), Some(enemy));
}

#[test]
fn listening_outpost_residual_transport_load_unload() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::host_listening_outpost::LISTENING_OUTPOST_TRANSPORT_SLOTS;

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::China);
    let outpost_id = create_test_listening_outpost(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));

    // Evacuate InitialPayload residual TankHunters to free slots for transport test.
    {
        let occupants = game_logic
            .host_object(outpost_id)
            .map(|o| o.contained_units())
            .unwrap_or_default();
        if !occupants.is_empty() {
            game_logic.queue_command(GameCommand {
                command_type: CommandType::Evacuate,
                player_id: 1,
                command_id: 1,
                timestamp: std::time::SystemTime::now(),
                selected_units: vec![outpost_id],
                modifier_keys: crate::command_system::ModifierKeys::default(),
            });
            game_logic.process_commands();
            // C++ TransportContain files riders out over frames (door
            // reservation, TransportContain.cpp orderAllPassengersToExit);
            // the world tick drains pending_stream_exit each frame
            // (world_tick/movement.rs drain_pending_transport_exits). Mirror
            // the tick until the InitialPayload riders are all out.
            for _ in 0..30 {
                game_logic.frame = game_logic.frame.saturating_add(1);
                game_logic.drain_pending_transport_exits_for_test();
            }
        }
    }
    {
        let outpost = game_logic.host_object(outpost_id).expect("outpost");
        assert_eq!(
            outpost.transport_count(),
            0,
            "evacuate must free Listening Outpost residual slots"
        );
    }

    let unit_a = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(1.0, 0.0, 0.0))
        .expect("unit a");
    let unit_b = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(2.0, 0.0, 0.0))
        .expect("unit b");

    for unit_id in [unit_a, unit_b] {
        {
            let ai_entry_frame = game_logic.frame;
            let unit = game_logic.host_object_mut(unit_id).expect("unit mut");
            crate::fixture_content::arm_with_stats(
                unit,
                crate::game_logic::Weapon {
                    damage: 20.0,
                    range: 80.0,
                    reload_time: 0.5,
                    ..Default::default()
                },
            );
            unit.weapon.as_mut().unwrap().last_fire_time = -10.0;
            unit.target = Some(outpost_id);
            unit.set_ai_state(
                AIState::Entering,
                ai_entry_frame,
                &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
            );
        }
        game_logic
            .update_ai(&[unit_id, outpost_id], 1.0 / 30.0)
            .expect("receiving AI operation succeeds");
    }

    let outpost = game_logic.host_object(outpost_id).expect("outpost loaded");
    assert!(
        outpost.contained_units().contains(&unit_a) && outpost.contained_units().contains(&unit_b),
        "both infantry must load into Listening Outpost residual"
    );
    assert_eq!(outpost.transport_count(), LISTENING_OUTPOST_TRANSPORT_SLOTS);
    assert_eq!(game_logic.listening_outpost_residual_loads(), 2);
    assert!(
        outpost.test_weapon_condition(3),
        "armed riders must upgrade Listening Outpost weapon set"
    );
    assert!(
        outpost
            .weapon
            .as_ref()
            .map(|w| { crate::game_logic::host_combat_chinook::is_passenger_dummy_weapon(w) })
            .unwrap_or(false),
        "ListeningOutpostUpgradedDummyWeapon residual bind"
    );
    assert!(
        game_logic.honesty_listening_outpost_weapon_set_upgrade_ok(),
        "weapon-set upgrade residual honesty"
    );

    // Capacity full: third infantry rejected residual.
    let unit_c = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(3.0, 0.0, 0.0))
        .expect("unit c");
    {
        let ai_entry_frame = game_logic.frame;
        let unit = game_logic.host_object_mut(unit_c).unwrap();
        unit.target = Some(outpost_id);
        unit.set_ai_state(
            AIState::Entering,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }
    game_logic
        .update_ai(&[unit_c, outpost_id], 1.0 / 30.0)
        .expect("receiving AI operation succeeds");
    {
        let outpost = game_logic.host_object(outpost_id).expect("full");
        assert_eq!(outpost.transport_count(), LISTENING_OUTPOST_TRANSPORT_SLOTS);
        assert!(!outpost.contained_units().contains(&unit_c));
    }

    // Unload residual.
    game_logic.queue_command(GameCommand {
        command_type: CommandType::Evacuate,
        player_id: 1,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![outpost_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    // Door-reservation residual files the second rider out over frames
    // (drain_pending_transport_exits, world_tick/movement.rs); each exited
    // rider then walks out its exit-door order to completion like the world
    // tick's movement update before going Idle. C++ AIExitState hands the
    // rider to aiFollowPath (OpenContain.cpp:915-1020 exitObjectViaDoor) and
    // the walk-out completes into AI_IDLE only when the path finishes
    // (AIFollowPathState SUCCESS), so tick until both riders settle.
    for _ in 0..120 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.drain_pending_transport_exits_for_test();
        game_logic.update_movement_for_test(&[unit_a, unit_b], 1.0 / 30.0);
        game_logic
            .update_ai(&[unit_a, unit_b], 1.0 / 30.0)
            .expect("receiving AI operation succeeds");
        let both_idle = [unit_a, unit_b].iter().all(|&id| {
            game_logic
                .host_object(id)
                .map(|u| u.ai_state() == AIState::Idle)
                .unwrap_or(true)
        });
        if both_idle {
            break;
        }
    }

    for unit_id in [unit_a, unit_b] {
        let unit = game_logic.host_object(unit_id).expect("free unit");
        assert_eq!(unit.ai_state(), AIState::Idle);
        assert!(unit.contained_by.is_none());
        assert!(unit.can_move());
    }
    {
        let outpost = game_logic.host_object(outpost_id).expect("empty");
        assert_eq!(outpost.transport_count(), 0);
        assert!(
            !outpost.test_weapon_condition(3),
            "no armed riders → clear PLAYER_UPGRADE residual"
        );
    }
    assert!(
        game_logic.listening_outpost_residual_unloads() >= 2,
        "unload residual honesty counter"
    );
    assert!(
        game_logic.honesty_listening_outpost_load_unload_ok(),
        "load/unload residual honesty"
    );
    assert!(
        game_logic.honesty_listening_outpost_ok(),
        "listening outpost residual host path honesty"
    );
}

#[test]
fn listening_outpost_residual_rejects_vehicle_enter() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::China);
    let outpost_id = create_test_listening_outpost(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));

    // Free InitialPayload residual slots.
    {
        let occupants = game_logic
            .host_object(outpost_id)
            .map(|o| o.contained_units())
            .unwrap_or_default();
        if !occupants.is_empty() {
            use crate::command_system::{CommandType, GameCommand};
            game_logic.queue_command(GameCommand {
                command_type: CommandType::Evacuate,
                player_id: 1,
                command_id: 1,
                timestamp: std::time::SystemTime::now(),
                selected_units: vec![outpost_id],
                modifier_keys: crate::command_system::ModifierKeys::default(),
            });
            game_logic.process_commands();
        }
    }

    let loads_before = game_logic.listening_outpost_residual_loads();
    let tank_id = game_logic
        .create_object("TestTank", Team::China, Vec3::new(2.0, 0.0, 0.0))
        .expect("tank");
    {
        let ai_entry_frame = game_logic.frame;
        let unit = game_logic.host_object_mut(tank_id).unwrap();
        unit.target = Some(outpost_id);
        unit.set_ai_state(
            AIState::Entering,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }
    game_logic
        .update_ai(&[tank_id, outpost_id], 1.0 / 30.0)
        .expect("receiving AI operation succeeds");

    let outpost = game_logic.host_object(outpost_id).expect("outpost");
    assert!(
        !outpost.contained_units().contains(&tank_id),
        "vehicles must not enter Listening Outpost residual"
    );
    assert_eq!(
        game_logic.listening_outpost_residual_loads(),
        loads_before,
        "vehicle enter must not count as Listening Outpost load"
    );
}

#[test]
fn mine_residual_place_enemy_triggers_damage() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);

    let mine_id = game_logic
        .place_land_mine(Team::USA, Vec3::new(0.0, 0.0, 0.0), None)
        .expect("place mine");
    assert_eq!(game_logic.mine_residual_places(), 1);

    let mine = game_logic.host_object(mine_id).expect("mine object");
    assert!(
        mine.mine_data.is_some(),
        "placed mine must carry residual mine_data"
    );
    let trigger = mine.mine_data.as_ref().unwrap().trigger_range;
    assert!(trigger > 0.0);

    // Enemy infantry outside range: no detonation.
    let enemy_id = game_logic
        .create_object(
            "TestInfantry",
            Team::GLA,
            Vec3::new(trigger + 50.0, 0.0, 0.0),
        )
        .expect("enemy");
    let health_before = game_logic.host_object(enemy_id).unwrap().health.current;

    game_logic.update_mines_and_demo_traps().unwrap();
    assert_eq!(
        game_logic.mine_residual_proximity_detonations(),
        0,
        "out-of-range enemy must not trigger"
    );
    assert!(game_logic.host_object(mine_id).unwrap().is_alive());

    // Move enemy into trigger range.
    {
        let (enemy, fixture_partition_enemy) =
            game_logic.host_object_and_partition_mut(enemy_id).unwrap();
        enemy.set_position_with_partition_owner(
            Vec3::new(trigger * 0.25, 0.0, 0.0),
            0,
            &mut Default::default(),
            fixture_partition_enemy,
        );
    }
    game_logic.update_mines_and_demo_traps().unwrap();

    assert_eq!(
        game_logic.mine_residual_proximity_detonations(),
        1,
        "in-range enemy must proximity-detonate"
    );
    assert!(
        game_logic.honesty_mine_place_trigger_ok(),
        "place+trigger honesty"
    );

    let enemy = game_logic.host_object(enemy_id).expect("enemy after");
    assert!(
        enemy.health.current < health_before || !enemy.is_alive() || enemy.status.destroyed,
        "enemy must take residual mine damage"
    );

    // Mine marked detonated / destroyed residual.
    if let Some(mine) = game_logic.host_object(mine_id) {
        assert!(
            mine.mine_data.as_ref().map(|d| d.detonated).unwrap_or(true) || mine.status.destroyed
        );
    }
}

#[test]
fn mine_residual_ally_does_not_trigger_land_mine() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);

    let mine_id = game_logic
        .place_land_mine(Team::USA, Vec3::new(0.0, 0.0, 0.0), None)
        .expect("mine");
    let ally_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .expect("ally");

    game_logic.update_mines_and_demo_traps().unwrap();
    assert_eq!(game_logic.mine_residual_proximity_detonations(), 0);
    assert!(game_logic.host_object(mine_id).unwrap().is_alive());
    assert!(game_logic.host_object(ally_id).unwrap().is_alive());
}

#[test]
fn mine_same_faction_enemy_triggers_land_mine() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);

    let mut usa_a = Player::new(0, Team::USA, "USA-A", true);
    usa_a.alliance_team = 1;
    game_logic.add_player(usa_a);
    let mut usa_b = Player::new(1, Team::USA, "USA-B", false);
    usa_b.alliance_team = 2;
    game_logic.add_player(usa_b);

    let mine_id = game_logic
        .create_object_for_player("TestLandMine", 0, Vec3::new(0.0, 0.0, 0.0))
        .or_else(|| {
            game_logic.ensure_residual_mine_template(
                "TestLandMine",
                crate::game_logic::host_mines::HostMineKind::LandMine,
            );
            game_logic.create_object_for_player("TestLandMine", 0, Vec3::new(0.0, 0.0, 0.0))
        })
        .expect("mine");
    {
        let mine = game_logic.host_object_mut(mine_id).expect("mine mut");
        mine.mine_data = Some(crate::game_logic::host_mines::HostMineData::land_mine());
    }
    let enemy_id = game_logic
        .create_object_for_player("TestInfantry", 1, Vec3::new(1.0, 0.0, 0.0))
        .expect("same-faction enemy");

    assert_eq!(
        game_logic.player_relationship(0, 1),
        game_engine::common::game_common::Relationship::Enemies
    );

    game_logic.update_mines_and_demo_traps().unwrap();
    assert_eq!(
        game_logic.mine_residual_proximity_detonations(),
        1,
        "same-faction ENEMIES must proximity-detonate"
    );
    let _ = enemy_id;
    assert!(
        game_logic.host_object(mine_id).is_none_or(|mine| mine
            .mine_data
            .as_ref()
            .is_some_and(|d| d.detonated)
            || mine.status.destroyed),
        "same-faction ENEMIES must trip the mine"
    );
}

#[test]
fn demo_trap_residual_proximity_detonates_on_enemy() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);

    let trap_id = game_logic
        .place_demo_trap(Team::GLA, Vec3::new(0.0, 0.0, 0.0), None)
        .expect("trap");
    let trap = game_logic.host_object(trap_id).unwrap();
    assert_eq!(
        trap.mine_data.as_ref().unwrap().kind,
        crate::game_logic::host_mines::HostMineKind::DemoTrap
    );
    let range = trap.mine_data.as_ref().unwrap().trigger_range;
    assert!((range - 40.0).abs() < 0.01);

    let enemy_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .expect("enemy");
    let health_before = game_logic.host_object(enemy_id).unwrap().health.current;

    game_logic.update_mines_and_demo_traps().unwrap();
    assert_eq!(game_logic.mine_residual_proximity_detonations(), 0);
    {
        let trap = game_logic.host_object(trap_id).unwrap();
        assert!(
            trap.mine_data
                .as_ref()
                .is_some_and(|d| d.demo_trap_warning_armed()),
            "proximity must arm the 1s SlowDeath warning"
        );
        let enemy = game_logic.host_object(enemy_id).unwrap();
        assert!(
            (enemy.health.current - health_before).abs() < 0.01 && !enemy.status.destroyed,
            "warning fuse must not splash on the arm frame"
        );
    }
    game_logic.frame = game_logic
        .frame
        .saturating_add(crate::game_logic::host_mines::DEMO_TRAP_DESTRUCTION_DELAY_FRAMES);
    game_logic.update_mines_and_demo_traps().unwrap();
    assert_eq!(game_logic.mine_residual_proximity_detonations(), 1);
    let enemy = game_logic.host_object(enemy_id).unwrap();
    assert!(
        enemy.health.current < health_before || enemy.status.destroyed,
        "demo trap must damage enemy"
    );
}

#[test]
fn demo_trap_detonate_when_killed_fires() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);

    let trap_id = game_logic
        .place_demo_trap(Team::GLA, Vec3::new(0.0, 0.0, 0.0), None)
        .expect("trap");
    let enemy_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(80.0, 0.0, 0.0))
        .expect("enemy");
    let hp_before = game_logic.host_object(enemy_id).unwrap().health.current;

    {
        let trap = game_logic.host_object_mut(trap_id).unwrap();
        trap.health.current = 0.0;
        trap.status.destroyed = true;
        trap.status.effectively_dead = true;
    }

    game_logic.update_mines_and_demo_traps().unwrap();
    {
        let trap = game_logic.host_object(trap_id);
        let armed = trap
            .and_then(|t| t.mine_data.as_ref())
            .is_some_and(|m| m.demo_trap_warning_armed() && !m.detonated);
        assert!(armed, "DetonateWhenKilled must arm the 1s warning fuse");
    }
    game_logic.frame = game_logic
        .frame
        .saturating_add(crate::game_logic::host_mines::DEMO_TRAP_DESTRUCTION_DELAY_FRAMES);
    game_logic.update_mines_and_demo_traps().unwrap();
    let trap = game_logic.host_object(trap_id);
    let detonated = trap
        .and_then(|t| t.mine_data.as_ref())
        .map(|m| m.detonated)
        .unwrap_or(true);
    assert!(
        detonated,
        "DetonateWhenKilled must fire DemoTrapUpdate::detonate"
    );
    let enemy = game_logic.host_object(enemy_id).unwrap();
    assert!(
        enemy.health.current < hp_before || enemy.status.destroyed || detonated,
        "death-detonate splash or trap consumed"
    );
}

#[test]
fn cluster_mines_special_power_places_mines() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::{
        SpecialPowerModuleKind, SpecialPowerModuleMetadata, host_mines::CLUSTER_MINE_COUNT,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_structure_template(&mut game_logic);
    // C++ SpecialPower.cpp:308 canUseSpecialPower: the caster must carry the
    // SpecialPowerModule for the command; stamp ClusterMines on the template.
    if let Some(t) = game_logic.templates.get_mut("TestBuilding") {
        t.special_power_modules.push(SpecialPowerModuleMetadata {
            source_index: 0,
            module_tag: Some("ModuleTag_SpecialPowerClusterMines".into()),
            module_kind: SpecialPowerModuleKind::OclSpecialPower,
            special_power_template: "SpecialPowerClusterMines".into(),
            special_power_template_id: 1,
            command_power: Some(SpecialPowerType::ClusterMines),
            reload_time_frames: 0,
            required_science: Some("SCIENCE_ClusterMines".into()),
            public_timer: false,
            shared_n_sync: false,
            shortcut_power: false,
            update_module_starts_attack: false,
            starts_paused: false,
            scripted_special_power_only: false,
            cash_bounty: None,
        });
    }

    // Ensure controlling player + science residual (SCIENCE_ClusterMines gate).
    if game_logic.get_player(0).is_none() {
        game_logic.add_player(Player::new(0, Team::USA, "USA", true));
    }
    if let Some(p) = game_logic.get_player_mut(0) {
        p.unlock_science("SCIENCE_ClusterMines");
    }

    // Caster that can fire special powers (player_id 0 → Team::USA ownership).
    let caster_id = game_logic
        .create_object("TestBuilding", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    {
        let caster = game_logic.host_object_mut(caster_id).unwrap();
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
    }

    // Mirror cluster_mines_flight_places_field geometry: the cargo bomb impact
    // must land far enough from the caster that no SmartBorder spot is
    // structure-skipped (mine_spot_blocked under-structure residual).
    let target = Vec3::new(180.0, 0.0, 0.0);
    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::ClusterMines,
            target: PowerTarget::Location(target),
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    // DeliverPayload residual: cargo plane drops bomb then mines place on impact.
    assert!(
        game_logic.cluster_mines_flight_reg.transports_spawned >= 1
            || game_logic.mine_residual_places() as usize >= 1,
        "ClusterMines must spawn cargo residual or place mines"
    );
    for f in 0..400 {
        game_logic.frame = f;
        game_logic.update_cluster_mines_flights();
        if game_logic.cluster_mines_flight_reg.minefields_placed >= 1
            || game_logic.mine_residual_places() as usize >= CLUSTER_MINE_COUNT
        {
            break;
        }
    }

    assert!(
        game_logic.mine_residual_places() as usize >= CLUSTER_MINE_COUNT,
        "ClusterMines must place residual mine ring (got {})",
        game_logic.mine_residual_places()
    );

    let mine_count = game_logic
        .host_objects()
        .registered_values()
        .filter(|o| {
            o.mine_data
                .as_ref()
                .map(|d| {
                    d.kind == crate::game_logic::host_mines::HostMineKind::LandMine && d.is_active()
                })
                .unwrap_or(false)
        })
        .count();
    assert!(
        mine_count >= CLUSTER_MINE_COUNT,
        "expected live residual mines, got {mine_count}"
    );
}

#[test]
fn demo_trap_manual_detonate_residual() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);

    let trap_id = game_logic
        .place_demo_trap(Team::GLA, Vec3::new(0.0, 0.0, 0.0), None)
        .expect("trap");
    let enemy_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .expect("enemy");
    // Disable proximity so only manual path fires.
    {
        let trap = game_logic.host_object_mut(trap_id).unwrap();
        if let Some(md) = trap.mine_data.as_mut() {
            md.proximity_enabled = false;
        }
    }

    // C++ DemoTrapUpdate::detonate (DemoTrapUpdate.cpp:240-251): the command
    // path fires the detonation weapon and kills the trap the SAME frame.
    // The retail SlowDeath DestructionDelay = 1000 (FX_GLADemoTrapWarning)
    // is the visual death sequence only — it never defers damage or the
    // detonation count.
    assert!(game_logic.manual_detonate_mine(trap_id).unwrap());
    assert_eq!(game_logic.mine_residual_manual_detonations(), 1);
    let enemy = game_logic.host_object(enemy_id).unwrap();
    assert!(
        enemy.health.current < enemy.max_health || enemy.status.destroyed,
        "manual detonate must damage nearby enemy"
    );
}

/// Authored `DemoTrapUpdate` fixture: a `Chem_`-shaped trap name is bound to a
/// captured Object definition, so only the module's own `DetonationWeapon` /
/// death modules own the blast — never the name-profile synthetic splash,
/// warning arm, or poison zone (C++ DemoTrapUpdate.cpp:240-251).
fn authored_demo_trap_world(
    trap_modules: &str,
    extra_objects: &str,
    weapons_ini: &str,
    ocl_ini: &str,
) -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    // Authored proximity scans query the partition manager (C++
    // iterateObjectsInRange position overload); a flat map + grid + one
    // partition phase publishes created objects before the first update.
    let mut map = game_engine::map_data::MapData::new();
    map.width = 32;
    map.height = 32;
    map.heightmap = vec![0; 1024];
    map.boundaries = vec![glam::IVec2::new(32, 32)];
    world
        .world_services
        .terrain()
        .write()
        .unwrap()
        .try_load_map_geometry(map)
        .unwrap();
    world.initialize_receiving_partition_grid().unwrap();
    ensure_test_player_for_team(&mut world, Team::GLA);
    ensure_test_player_for_team(&mut world, Team::USA);
    world
        .get_player_mut(0)
        .unwrap()
        .set_map_relationship(2, game_engine::common::game_common::Relationship::Enemies);
    world
        .get_player_mut(2)
        .unwrap()
        .set_map_relationship(0, game_engine::common::game_common::Relationship::Enemies);
    if !ocl_ini.trim().is_empty() {
        world
            .admit_object_creation_list_sources([ocl_ini.to_owned()])
            .unwrap();
    }
    world
        .admit_object_ini_sources([(
            "authored-traps.ini".to_owned(),
            format!(
                "Object Chem_DemoTrapAuthored\n  KindOf = STRUCTURE IMMOBILE DEMOTRAP\n  Body = ActiveBody BodyTag\n    MaxHealth = 100\n    InitialHealth = 100\n  End\n  {trap_modules}\nEnd\nObject TrapBlastTarget\n  KindOf = VEHICLE\n  Body = ActiveBody BodyTag\n    MaxHealth = 1000\n    InitialHealth = 1000\n  End\nEnd\n{extra_objects}"
            ),
        )])
        .unwrap();
    let mut weapons = std::collections::HashMap::new();
    assert!(
        crate::assets::ini_template_loader::parse_weapon_definitions_ini_into(
            &mut weapons,
            weapons_ini,
            crate::assets::ini_template_loader::WeaponDefinitionLoadType::Normal,
        ) >= 1,
        "fixture weapons must parse"
    );
    world
        .admit_weapon_content_snapshot(
            weapons
                .into_values()
                .map(|rule| (rule.name.clone(), rule))
                .collect(),
            &[],
        )
        .unwrap();
    let trap = world
        .create_object("Chem_DemoTrapAuthored", Team::GLA, Vec3::ZERO)
        .expect("authored trap");
    let enemy = world
        .create_object("TrapBlastTarget", Team::USA, Vec3::new(8.0, 0.0, 0.0))
        .expect("enemy");
    world
        .update_owned_partition_with_effects(&mut Default::default())
        .unwrap();
    (world, trap, enemy)
}

/// 31-damage no-projectile blast used by the authored DemoTrap tests.
const AUTHORED_TRAP_BLAST_INI: &str = "Weapon AuthoredTrapBlast\n PrimaryDamage = 31\n PrimaryDamageRadius = 20\n WeaponSpeed = 999999\n DamageType = UNRESISTABLE\n RadiusDamageAffects = ENEMIES\n ProjectileObject = NONE\nEnd\n";

fn authored_trap_children(world: &GameLogic, name: &str) -> Vec<ObjectId> {
    world
        .host_objects()
        .registered()
        .filter(|(_, object)| object.template_name == name)
        .map(|(id, _)| *id)
        .collect()
}

#[test]
fn authored_demo_trap_manual_detonate_fires_bound_weapon_once() {
    let (mut world, trap, enemy) = authored_demo_trap_world(
        "Behavior = DemoTrapUpdate TrapModule\n    DetonationWeapon = AuthoredTrapBlast\n    DetonateWhenKilled = No\n    DefaultProximityMode = Yes\n  End",
        "Object TrapBlastChild\n  KindOf = IMMOBILE INERT NO_COLLIDE\n  Body = ActiveBody BodyTag\n    MaxHealth = 10\n    InitialHealth = 10\n  End\nEnd\n",
        "Weapon AuthoredTrapBlast\n PrimaryDamage = 31\n PrimaryDamageRadius = 20\n WeaponSpeed = 999999\n DamageType = UNRESISTABLE\n RadiusDamageAffects = ENEMIES\n ProjectileObject = NONE\n FireOCL = OCL_TrapBlastSpawn\nEnd\n",
        "ObjectCreationList OCL_TrapBlastSpawn\n  CreateObject\n    ObjectNames = TrapBlastChild\n    Count = 1\n  End\nEnd\n",
    );
    assert_eq!(
        world.host_object(trap).unwrap().mine_data.as_ref().unwrap().kind,
        crate::game_logic::host_mines::HostMineKind::DemoTrap,
        "authored DemoTrapUpdate must bind mine runtime"
    );
    let zones_before = world.toxin_tractor_registry().zones_spawned;

    assert!(world.manual_detonate_mine(trap).unwrap());
    assert_eq!(world.mine_residual_manual_detonations(), 1);
    assert_eq!(
        world.host_object(enemy).unwrap().health.current,
        969.0,
        "authored DetonationWeapon applies exactly its PrimaryDamage at range 8"
    );
    let children = authored_trap_children(&world, "TrapBlastChild");
    assert_eq!(children.len(), 1, "weapon FireOCL creates exactly one child");
    assert_eq!(
        world.host_object(children[0]).unwrap().producer_id,
        Some(trap),
        "OCL child producer is the detonating trap"
    );
    {
        let trap_obj = world.host_object(trap).unwrap();
        assert!(
            !trap_obj.is_alive() || trap_obj.status.destroyed,
            "detonate() runs Object::kill the same frame"
        );
        assert!(trap_obj.mine_data.as_ref().unwrap().detonated);
    }
    assert_eq!(
        world.toxin_tractor_registry().zones_spawned,
        zones_before,
        "authored trap never spawns a synthetic poison zone"
    );

    // detonate() is once-only: m_detonated latches.
    assert!(!world.manual_detonate_mine(trap).unwrap());
    assert_eq!(world.mine_residual_manual_detonations(), 1);
    assert_eq!(authored_trap_children(&world, "TrapBlastChild").len(), 1);
}

#[test]
fn authored_demo_trap_detonate_none_delegates_to_death_modules() {
    let (mut world, trap, enemy) = authored_demo_trap_world(
        "Behavior = DemoTrapUpdate TrapModule\n    DetonationWeapon = None\n    DetonateWhenKilled = No\n    DefaultProximityMode = Yes\n  End\n  Behavior = FireWeaponWhenDeadBehavior DeathTag\n    StartsActive = Yes\n    DeathWeapon = AuthoredTrapDeathBlast\n  End",
        "Object TrapPoisonChild\n  KindOf = IMMOBILE INERT NO_COLLIDE\n  Body = ActiveBody BodyTag\n    MaxHealth = 10\n    InitialHealth = 10\n  End\nEnd\n",
        "Weapon AuthoredTrapDeathBlast\n PrimaryDamage = 31\n PrimaryDamageRadius = 20\n WeaponSpeed = 999999\n DamageType = UNRESISTABLE\n RadiusDamageAffects = ENEMIES\n ProjectileObject = NONE\n FireOCL = OCL_TrapDeathSpawn\nEnd\n",
        "ObjectCreationList OCL_TrapDeathSpawn\n  CreateObject\n    ObjectNames = TrapPoisonChild\n    Count = 1\n  End\nEnd\n",
    );
    let zones_before = world.toxin_tractor_registry().zones_spawned;

    assert!(world.manual_detonate_mine(trap).unwrap());
    assert_eq!(world.mine_residual_manual_detonations(), 1);
    assert_eq!(
        world.host_object(enemy).unwrap().health.current,
        969.0,
        "no direct blast: the death module's authored DeathWeapon owns the damage"
    );
    let children = authored_trap_children(&world, "TrapPoisonChild");
    assert_eq!(
        children.len(),
        1,
        "death weapon FireOCL emits exactly the authored child"
    );
    assert_eq!(
        world.host_object(children[0]).unwrap().producer_id,
        Some(trap)
    );
    assert_eq!(
        world.toxin_tractor_registry().zones_spawned,
        zones_before,
        "Chem_-shaped name must not add the synthetic MediumPoisonField"
    );
}

#[test]
fn authored_demo_trap_without_weapon_or_detonate_when_killed_stays_quiet() {
    // Neither the trap update nor an on-death module supplies a blast.
    let (mut world, trap, enemy) = authored_demo_trap_world(
        "Behavior = DemoTrapUpdate TrapModule\n    DetonateWhenKilled = No\n    DefaultProximityMode = Yes\n  End",
        "",
        "Weapon AuthoredTrapUnused\n PrimaryDamage = 31\n DamageType = UNRESISTABLE\n ProjectileObject = NONE\nEnd\n",
        "",
    );
    let zones_before = world.toxin_tractor_registry().zones_spawned;

    assert!(world.manual_detonate_mine(trap).unwrap());
    assert_eq!(world.mine_residual_manual_detonations(), 1);
    assert_eq!(
        world.host_object(enemy).unwrap().health.current,
        1000.0,
        "authored module with no blast does no synthetic splash"
    );
    assert_eq!(world.toxin_tractor_registry().zones_spawned, zones_before);

    // DetonateWhenKilled = No: a normal (non-detonate) death must not queue
    // the killed scan either (DemoTrapUpdate.cpp:124-130).
    let (mut killed_world, killed_trap, killed_enemy) = authored_demo_trap_world(
        "Behavior = DemoTrapUpdate TrapModule\n    DetonationWeapon = AuthoredTrapBlast\n    DetonateWhenKilled = No\n    DefaultProximityMode = Yes\n  End",
        "Object TrapBlastChild\n  KindOf = IMMOBILE INERT NO_COLLIDE\n  Body = ActiveBody BodyTag\n    MaxHealth = 10\n    InitialHealth = 10\n  End\nEnd\n",
        "Weapon AuthoredTrapBlast\n PrimaryDamage = 31\n PrimaryDamageRadius = 20\n WeaponSpeed = 999999\n DamageType = UNRESISTABLE\n RadiusDamageAffects = ENEMIES\n ProjectileObject = NONE\n FireOCL = OCL_TrapBlastSpawn\nEnd\n",
        "ObjectCreationList OCL_TrapBlastSpawn\n  CreateObject\n    ObjectNames = TrapBlastChild\n    Count = 1\n  End\nEnd\n",
    );
    {
        let trap_obj = killed_world.host_object_mut(killed_trap).unwrap();
        trap_obj.health.current = 0.0;
        trap_obj.status.destroyed = true;
        trap_obj.status.effectively_dead = true;
    }
    killed_world
        .update_demo_trap_detonate_when_killed_with_effects(&mut Default::default())
        .unwrap();
    assert_eq!(
        killed_world.mine_residual_manual_detonations(),
        0,
        "DetonateWhenKilled = No must not queue the killed scan"
    );
    assert_eq!(
        killed_world.host_object(killed_enemy).unwrap().health.current,
        1000.0
    );
    assert_eq!(
        authored_trap_children(&killed_world, "TrapBlastChild").len(),
        0
    );
}

#[test]
fn authored_demo_trap_defaults_to_manual_and_zero_range() {
    // DemoTrapUpdateModuleData ctor: m_defaultsToProximityMode=false,
    // m_triggerDetonationRange=0.0f — only authored fields enable the scan.
    let blast_ini = AUTHORED_TRAP_BLAST_INI;
    let cases: [(&str, bool); 4] = [
        ("    TriggerDetonationRange = 20\n", false),
        (
            "    TriggerDetonationRange = 20\n    DefaultProximityMode = No\n",
            false,
        ),
        ("    DefaultProximityMode = Yes\n", false),
        (
            "    TriggerDetonationRange = 20\n    DefaultProximityMode = Yes\n",
            true,
        ),
    ];
    for (module_fields, expect_auto) in cases {
        let (mut world, trap, enemy) = authored_demo_trap_world(
            &format!(
                "Behavior = DemoTrapUpdate TrapModule\n    DetonationWeapon = AuthoredTrapBlast\n    DetonateWhenKilled = No\n    AutoDetonationWithFriendsInvolved = Yes\n{module_fields}  End"
            ),
            "",
            blast_ini,
            "",
        );
        world.update_mines_and_demo_traps().unwrap();
        let enemy_hp_after = world.host_object(enemy).unwrap().health.current;
        if expect_auto {
            assert_eq!(
                enemy_hp_after, 969.0,
                "a proximity trip fires DetonationWeapon on the same update"
            );
            let trap_obj = world.host_object(trap).unwrap();
            assert!(
                !trap_obj.is_alive() || trap_obj.status.destroyed,
                "detonate() runs Object::kill the same frame"
            );
            assert!(trap_obj.mine_data.as_ref().unwrap().detonated);
        } else {
            assert_eq!(enemy_hp_after, 1000.0);
            let trap_obj = world.host_object(trap).unwrap();
            let md = trap_obj.mine_data.as_ref().unwrap();
            assert!(!md.detonated && !md.demo_trap_warning_armed());
            assert!(trap_obj.is_alive());
            // The fixture setup itself still detonates on the manual order.
            assert!(world.manual_detonate_mine(trap).unwrap());
            assert_eq!(
                world.host_object(enemy).unwrap().health.current,
                969.0,
                "manual detonate fires exactly the authored PrimaryDamage"
            );
        }
    }
}

#[test]
fn authored_demo_trap_proximity_scan_obeys_friends_veto_and_ignore_kinds() {
    let blast_ini = AUTHORED_TRAP_BLAST_INI;
    let ally_ini =
        "Object TrapBlastAlly\n  KindOf = VEHICLE\n  Body = ActiveBody BodyTag\n    MaxHealth = 500\n    InitialHealth = 500\n  End\nEnd\n";
    // C++ DemoTrapUpdate.cpp:196-205 — a non-enemy inside the trigger range
    // vetoes the whole scan unless AutoDetonationWithFriendsInvolved = Yes.
    for friends in ["No", "Yes"] {
        let (mut world, trap, enemy) = authored_demo_trap_world(
            &format!(
                "Behavior = DemoTrapUpdate TrapModule\n    DetonationWeapon = AuthoredTrapBlast\n    DetonateWhenKilled = No\n    DefaultProximityMode = Yes\n    TriggerDetonationRange = 20\n    IgnoreTargetTypes = DEMOTRAP UNATTACKABLE\n    AutoDetonationWithFriendsInvolved = {friends}\n  End"
            ),
            ally_ini,
            blast_ini,
            "",
        );
        let ally = world
            .create_object("TrapBlastAlly", Team::GLA, Vec3::new(4.0, 0.0, 0.0))
            .expect("ally");
        world
            .update_owned_partition_with_effects(&mut Default::default())
            .unwrap();
        world.update_mines_and_demo_traps().unwrap();
        let trap_obj = world.host_object(trap).unwrap();
        if friends == "No" {
            assert_eq!(
                world.host_object(enemy).unwrap().health.current,
                1000.0,
                "a friend in range vetoes the entire proximity scan"
            );
            assert!(trap_obj.is_alive() && !trap_obj.mine_data.as_ref().unwrap().detonated);
        } else {
            assert_eq!(
                world.host_object(enemy).unwrap().health.current,
                969.0,
                "friends-involved mode stops at the first grounded enemy"
            );
            assert!(trap_obj.mine_data.as_ref().unwrap().detonated);
            assert_eq!(
                world.host_object(ally).unwrap().health.current,
                500.0,
                "ENEMIES-only blast spares the nearby friend"
            );
        }
    }

    // IgnoreTargetTypes: the matching enemy is skipped; a valid enemy entering
    // later still trips the trap.
    let (mut world, trap, enemy) = authored_demo_trap_world(
        "Behavior = DemoTrapUpdate TrapModule\n    DetonationWeapon = AuthoredTrapBlast\n    DetonateWhenKilled = No\n    DefaultProximityMode = Yes\n    TriggerDetonationRange = 20\n    IgnoreTargetTypes = DEMOTRAP UNATTACKABLE\n    AutoDetonationWithFriendsInvolved = No\n  End",
        "Object TrapBlastIgnored\n  KindOf = VEHICLE UNATTACKABLE\n  Body = ActiveBody BodyTag\n    MaxHealth = 1000\n    InitialHealth = 1000\n  End\nEnd\n",
        blast_ini,
        "",
    );
    let _ignored = world
        .create_object("TrapBlastIgnored", Team::USA, Vec3::new(8.0, 0.0, 0.0))
        .expect("ignored enemy");
    assert!(world.set_registered_object_position(enemy, Vec3::new(200.0, 0.0, 0.0)));
    world
        .update_owned_partition_with_effects(&mut Default::default())
        .unwrap();
    world.update_mines_and_demo_traps().unwrap();
    assert_eq!(
        world.host_object(enemy).unwrap().health.current,
        1000.0,
        "an UNATTACKABLE enemy in range is ignored, not a veto"
    );
    let trap_obj = world.host_object(trap).unwrap();
    assert!(trap_obj.is_alive() && !trap_obj.mine_data.as_ref().unwrap().detonated);

    assert!(world.set_registered_object_position(enemy, Vec3::new(8.0, 0.0, 0.0)));
    world
        .update_owned_partition_with_effects(&mut Default::default())
        .unwrap();
    world.update_mines_and_demo_traps().unwrap();
    assert_eq!(
        world.host_object(enemy).unwrap().health.current,
        969.0,
        "a valid enemy entering range trips the same trap"
    );
}

#[test]
fn authored_demo_trap_scan_rate_counts_down_before_next_scan() {
    let blast_ini = AUTHORED_TRAP_BLAST_INI;
    let modules = "Behavior = DemoTrapUpdate TrapModule\n    DetonationWeapon = AuthoredTrapBlast\n    DetonateWhenKilled = No\n    DefaultProximityMode = Yes\n    TriggerDetonationRange = 20\n    AutoDetonationWithFriendsInvolved = Yes\n    ScanRate = 100\n  End";
    // ScanRate 100ms -> 3 frames: scan on the countdown-expired update, then
    // 3 countdown updates before the next scan (DemoTrapUpdate.cpp:140-150).
    let (mut world, _trap, enemy) = authored_demo_trap_world(modules, "", blast_ini, "");
    // No enemy in range for the initial scan at frame 1.
    assert!(world.set_registered_object_position(enemy, Vec3::new(200.0, 0.0, 0.0)));
    world
        .update_owned_partition_with_effects(&mut Default::default())
        .unwrap();
    world.frame = 1;
    world.update_mines_and_demo_traps().unwrap();
    assert_eq!(world.host_object(enemy).unwrap().health.current, 1000.0);
    // Enemy enters between scans; the countdown still has to finish.
    assert!(world.set_registered_object_position(enemy, Vec3::new(8.0, 0.0, 0.0)));
    world
        .update_owned_partition_with_effects(&mut Default::default())
        .unwrap();
    for frame in 2..=3 {
        world.frame = frame;
        world.update_mines_and_demo_traps().unwrap();
        assert_eq!(
            world.host_object(enemy).unwrap().health.current,
            1000.0,
            "frame {frame}: m_nextScanFrames still counting down"
        );
    }

    // Snapshot mid-countdown (frame 3: remaining 1) and restore into an
    // identical receiving world — the countdown resumes, not resets.
    use crate::save_load::{SnapshotBuilder, snapshot::decode_bincode_world_snapshot};
    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&world).unwrap();
    let encoded = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = decode_bincode_world_snapshot(&encoded).unwrap();
    let (mut restored, restored_trap, restored_enemy) =
        authored_demo_trap_world(modules, "", blast_ini, "");
    restored.clear_all_players();
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    let restored_scan = restored
        .host_object(restored_trap)
        .unwrap()
        .mine_data
        .as_ref()
        .unwrap()
        .authored_demo_trap
        .expect("authored scan state survives snapshot");
    assert_eq!(restored_scan.remaining, 1, "frame-3 snapshot keeps the countdown");
    for frame in 4..=5 {
        let expected = if frame == 5 { 969.0 } else { 1000.0 };
        world.frame = frame;
        world.update_mines_and_demo_traps().unwrap();
        assert_eq!(
            world.host_object(enemy).unwrap().health.current,
            expected,
            "live world frame {frame}"
        );
        restored.frame = frame;
        restored.update_mines_and_demo_traps().unwrap();
        assert_eq!(
            restored.host_object(restored_enemy).unwrap().health.current,
            expected,
            "restored world frame {frame}: same countdown, same blast"
        );
    }
}

#[test]
fn authored_demo_trap_named_placement_keeps_admitted_scan() {
    let blast_ini = AUTHORED_TRAP_BLAST_INI;
    let (mut world, trap, enemy) = authored_demo_trap_world(
        "Behavior = DemoTrapUpdate TrapModule\n    DetonationWeapon = AuthoredTrapBlast\n    DetonateWhenKilled = No\n    DefaultProximityMode = Yes\n    TriggerDetonationRange = 20\n    AutoDetonationWithFriendsInvolved = Yes\n    ScanRate = 100\n  End",
        "",
        blast_ini,
        "",
    );
    let placed = world
        .place_demo_trap_named(
            "Chem_DemoTrapAuthored",
            Team::GLA,
            Vec3::new(50.0, 0.0, 0.0),
            Some(trap),
            false,
        )
        .expect("named placement");
    {
        let placed_obj = world.host_object(placed).unwrap();
        let md = placed_obj.mine_data.as_ref().unwrap();
        let scan = md
            .authored_demo_trap
            .expect("authored scan survives named placement");
        assert_eq!(scan.interval, 3, "ScanRate 100ms -> 3 frames");
        assert_eq!(scan.remaining, 0);
        assert!(matches!(
            md.demo_trap_mode,
            crate::game_logic::host_mines::DemoTrapMode::Proximity
        ));
        assert!(md.proximity_enabled && md.trigger_range == 20.0);
        assert_eq!(md.producer_id, Some(trap));
        assert_eq!(placed_obj.producer_id, Some(trap));
        assert!(
            !placed_obj.innate_stealth && !placed_obj.status.stealthed,
            "authored placement adds no innate stealth"
        );
    }
    // Live proof: the placed trap runs its own authored scan and detonates.
    assert!(world.set_registered_object_position(enemy, Vec3::new(55.0, 0.0, 0.0)));
    world
        .update_owned_partition_with_effects(&mut Default::default())
        .unwrap();
    world.update_mines_and_demo_traps().unwrap();
    assert_eq!(
        world.host_object(enemy).unwrap().health.current,
        969.0,
        "named placement births a working authored proximity trap"
    );
    assert!(world.host_object(placed).unwrap().mine_data.as_ref().unwrap().detonated);
    assert!(
        world.host_object(trap).unwrap().is_alive(),
        "the out-of-range fixture trap stays quiet"
    );
}

#[test]
fn authored_demo_trap_observes_selected_weapon_slot_on_update() {
    use crate::game_logic::WeaponLockType;
    // DemoTrapUpdate::onObjectCreated arms WEAPONSET_VETERAN then locks the
    // authored mode slot; the unconditional base set lets the row identity
    // actually switch, and every slot binds a dummy so selection is real.
    let weapon_set = "WeaponSet\n  Conditions = None\n  Weapon = PRIMARY TrapModeDummy\n  Weapon = SECONDARY TrapModeDummy\n  Weapon = TERTIARY TrapModeDummy\nEnd\nWeaponSet\n  Conditions = VETERAN\n  Weapon = PRIMARY TrapModeDummy\n  Weapon = SECONDARY TrapModeDummy\n  Weapon = TERTIARY TrapModeDummy\nEnd\n";
    let weapons = format!(
        "{AUTHORED_TRAP_BLAST_INI}Weapon TrapModeDummy\n PrimaryDamage = 0\n DamageType = UNRESISTABLE\n ProjectileObject = NONE\nEnd\n"
    );
    // Permuted slots: manual PRIMARY, proximity TERTIARY, detonation SECONDARY.
    let modules = |mode: &str| {
        format!(
            "{weapon_set}Behavior = DemoTrapUpdate TrapModule\n    DetonationWeapon = AuthoredTrapBlast\n    DetonateWhenKilled = No\n    {mode}\n    TriggerDetonationRange = 20\n    AutoDetonationWithFriendsInvolved = Yes\n    ScanRate = 100\n    ManualModeWeaponSlot = PRIMARY\n    ProximityModeWeaponSlot = TERTIARY\n    DetonationWeaponSlot = SECONDARY\n  End"
        )
    };

    let (mut world, trap, enemy) =
        authored_demo_trap_world(&modules("DefaultProximityMode = No"), "", &weapons, "");
    {
        let obj = world.host_object(trap).unwrap();
        assert!(
            obj.test_weapon_condition(0),
            "VETERAN weapon set armed at birth"
        );
        assert_eq!(
            obj.selected_weapon_slot(),
            Some(0),
            "manual mode locks the authored PRIMARY slot"
        );
    }
    // The enemy sits inside trigger range but the manual slot never scans.
    world.frame = 1;
    world.update_mines_and_demo_traps().unwrap();
    assert_eq!(world.host_object(enemy).unwrap().health.current, 1000.0);

    // Locking TERTIARY — the authored proximity slot — only selects the
    // weapon; the module observes it on its next update, no inline damage.
    assert!(
        world
            .unit_command_set_weapon_lock(trap, 2, WeaponLockType::LockedPermanently)
            .unwrap()
    );
    assert_eq!(world.host_object(enemy).unwrap().health.current, 1000.0);

    // A real proximity scan with the enemy out of range starts the countdown.
    assert!(world.set_registered_object_position(enemy, Vec3::new(200.0, 0.0, 0.0)));
    world
        .update_owned_partition_with_effects(&mut Default::default())
        .unwrap();
    world.frame = 2;
    world.update_mines_and_demo_traps().unwrap();
    {
        let md = world.host_object(trap).unwrap().mine_data.as_ref().unwrap();
        assert_eq!(md.authored_demo_trap.unwrap().remaining, 3);
        assert!(!md.detonated);
    }

    // Bring the enemy back and select SECONDARY — the authored detonation
    // slot fires before the countdown is touched (DemoTrapUpdate.cpp:134-138).
    assert!(world.set_registered_object_position(enemy, Vec3::new(8.0, 0.0, 0.0)));
    world
        .update_owned_partition_with_effects(&mut Default::default())
        .unwrap();
    assert!(
        world
            .unit_command_set_weapon_lock(trap, 1, WeaponLockType::LockedPermanently)
            .unwrap()
    );
    assert_eq!(
        world.host_object(enemy).unwrap().health.current,
        1000.0,
        "the lock command itself has no inline damage"
    );
    assert!(world.host_object(trap).unwrap().is_alive());
    world.frame = 3;
    world.update_mines_and_demo_traps().unwrap();
    assert_eq!(
        world.host_object(enemy).unwrap().health.current,
        969.0,
        "detonation slot bypasses the remaining countdown"
    );
    assert!(world.host_object(trap).unwrap().mine_data.as_ref().unwrap().detonated);

    // Fresh proximity-default fixture: the authored TERTIARY slot is selected
    // at birth and auto-triggers despite the permutation.
    let (mut world, trap, enemy) =
        authored_demo_trap_world(&modules("DefaultProximityMode = Yes"), "", &weapons, "");
    assert_eq!(
        world.host_object(trap).unwrap().selected_weapon_slot(),
        Some(2)
    );
    world.update_mines_and_demo_traps().unwrap();
    assert_eq!(world.host_object(enemy).unwrap().health.current, 969.0);
}

#[test]
fn dozer_mine_clear_residual_disarms_enemy_mine_safely() {
    use crate::game_logic::host_mines::{
        DOZER_MINE_CLEAR_PRE_ATTACK_FRAMES, DOZER_MINE_CLEAR_RANGE,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_dozer_template(&mut game_logic);

    let mine_id = game_logic
        .place_land_mine(Team::GLA, Vec3::new(0.0, 0.0, 0.0), None)
        .expect("enemy mine");
    assert_eq!(game_logic.mine_residual_places(), 1);

    let dozer_id = game_logic
        .create_object(
            "TestDozer",
            Team::USA,
            Vec3::new(DOZER_MINE_CLEAR_RANGE * 0.5, 0.0, 0.0),
        )
        .expect("dozer");
    if let Some(d) = game_logic.host_object_mut(dozer_id) {
        d.set_weapon_condition_at_frame(8, true, 0);
    }
    let health_before = game_logic.host_object(dozer_id).unwrap().health.current;
    assert!(
        game_logic
            .host_object(dozer_id)
            .unwrap()
            .is_kind_of(KindOf::Worker),
        "TestDozer must be Worker residual for mine clear"
    );

    game_logic.frame = 1;
    game_logic.update_mines_and_demo_traps().unwrap();
    assert_eq!(
        game_logic.mine_residual_clears(),
        0,
        "PreAttackDelay must hold the first frame"
    );

    game_logic.frame = 1 + DOZER_MINE_CLEAR_PRE_ATTACK_FRAMES;
    game_logic.update_mines_and_demo_traps().unwrap();

    assert_eq!(
        game_logic.mine_residual_clears(),
        1,
        "dozer must clear enemy mine after PreAttackDelay"
    );
    assert_eq!(
        game_logic.mine_residual_proximity_detonations(),
        0,
        "clear must not detonate"
    );
    assert!(
        game_logic.honesty_mine_clear_ok(),
        "place+clear honesty path"
    );

    if let Some(mine) = game_logic.host_object(mine_id) {
        assert!(
            mine.mine_data
                .as_ref()
                .map(|d| d.detonated || !d.is_active())
                .unwrap_or(true)
                || mine.status.destroyed,
            "cleared mine must be inactive"
        );
    }

    let dozer = game_logic.host_object(dozer_id).expect("dozer after clear");
    assert!(dozer.is_alive(), "dozer must survive clear");
    assert!(
        !dozer.status.destroyed,
        "dozer must not be marked destroyed"
    );
    assert_eq!(
        dozer.health.current, health_before,
        "dozer must take no damage from safe clear"
    );
}

#[test]
fn dozer_mine_clear_residual_approaches_then_clears() {
    use crate::game_logic::host_mines::{
        DOZER_MINE_CLEAR_PRE_ATTACK_FRAMES, DOZER_MINE_CLEAR_RANGE, DOZER_MINE_CLEAR_SCAN_RANGE,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_dozer_template(&mut game_logic);

    let mine_id = game_logic
        .place_land_mine(Team::GLA, Vec3::new(0.0, 0.0, 0.0), None)
        .expect("mine");

    let approach_dist = (DOZER_MINE_CLEAR_RANGE + DOZER_MINE_CLEAR_SCAN_RANGE) * 0.5;
    assert!(approach_dist > DOZER_MINE_CLEAR_RANGE);
    assert!(approach_dist < DOZER_MINE_CLEAR_SCAN_RANGE);

    let dozer_id = game_logic
        .create_object("TestDozer", Team::USA, Vec3::new(approach_dist, 0.0, 0.0))
        .expect("dozer");
    if let Some(d) = game_logic.host_object_mut(dozer_id) {
        d.set_weapon_condition_at_frame(8, true, 0);
    }

    game_logic.update_mines_and_demo_traps().unwrap();
    assert_eq!(
        game_logic.mine_residual_clears(),
        0,
        "not in clear range yet"
    );
    {
        let dozer = game_logic.host_object(dozer_id).unwrap();
        assert!(
            dozer.movement.target_position.is_some(),
            "must have move target toward mine"
        );
        {
            assert_eq!(dozer.ai_state(), AIState::Moving, "must approach mine");
        }
    }

    {
        let ai_entry_frame = game_logic.frame;
        let (dozer, fixture_partition_dozer) =
            game_logic.host_object_and_partition_mut(dozer_id).unwrap();
        dozer.set_position_with_partition_owner(
            Vec3::new(DOZER_MINE_CLEAR_RANGE * 0.25, 0.0, 0.0),
            0,
            &mut Default::default(),
            fixture_partition_dozer,
        );
        dozer.set_ai_state(
            AIState::Idle,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }
    game_logic.frame = 10;
    game_logic.update_mines_and_demo_traps().unwrap();
    assert_eq!(
        game_logic.mine_residual_clears(),
        0,
        "PreAttackDelay must hold after entering range"
    );

    game_logic.frame = 10 + DOZER_MINE_CLEAR_PRE_ATTACK_FRAMES;
    game_logic.update_mines_and_demo_traps().unwrap();

    assert_eq!(game_logic.mine_residual_clears(), 1);
    assert_eq!(game_logic.mine_residual_proximity_detonations(), 0);
    assert!(game_logic.host_object(dozer_id).unwrap().is_alive());
    if let Some(mine) = game_logic.host_object(mine_id) {
        assert!(mine.mine_data.as_ref().map(|d| d.detonated).unwrap_or(true));
    }
}

#[test]
fn dozer_mine_clear_residual_skips_ally_mine() {
    use crate::game_logic::host_mines::DOZER_MINE_CLEAR_RANGE;

    let mut game_logic = GameLogic::new();
    ensure_test_dozer_template(&mut game_logic);

    let mine_id = game_logic
        .place_land_mine(Team::USA, Vec3::new(0.0, 0.0, 0.0), None)
        .expect("ally mine");
    let _dozer_id = game_logic
        .create_object(
            "TestDozer",
            Team::USA,
            Vec3::new(DOZER_MINE_CLEAR_RANGE * 0.5, 0.0, 0.0),
        )
        .expect("dozer");

    game_logic.update_mines_and_demo_traps().unwrap();
    assert_eq!(game_logic.mine_residual_clears(), 0);
    assert_eq!(game_logic.mine_residual_proximity_detonations(), 0);
    let mine = game_logic.host_object(mine_id).unwrap();
    assert!(
        mine.mine_data
            .as_ref()
            .map(|d| d.is_active())
            .unwrap_or(false),
        "ally mine must remain active"
    );
}

#[test]
fn dozer_mine_clear_residual_infantry_still_triggers() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);

    let mine_id = game_logic
        .place_land_mine(Team::GLA, Vec3::new(0.0, 0.0, 0.0), None)
        .expect("mine");
    let trigger = game_logic
        .host_object(mine_id)
        .unwrap()
        .mine_data
        .as_ref()
        .unwrap()
        .trigger_range;
    let _enemy = game_logic
        .create_object(
            "TestInfantry",
            Team::USA,
            Vec3::new(trigger * 0.25, 0.0, 0.0),
        )
        .expect("infantry");

    game_logic.update_mines_and_demo_traps().unwrap();
    assert_eq!(game_logic.mine_residual_clears(), 0);
    assert_eq!(game_logic.mine_residual_proximity_detonations(), 1);
}

#[test]
fn land_mine_trips_on_vehicle_geometry_contact() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);

    let mine_id = game_logic
        .place_land_mine(Team::USA, Vec3::new(0.0, 0.0, 0.0), None)
        .expect("mine");
    let trigger = game_logic
        .host_object(mine_id)
        .unwrap()
        .mine_data
        .as_ref()
        .unwrap()
        .trigger_range;
    assert!((trigger - 30.0).abs() < 0.01);

    // Center 20 wu away: retail 30wu pad + victim 8 overlaps.
    let enemy_id = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(20.0, 0.0, 0.0))
        .expect("vehicle");
    {
        let e = game_logic.host_object_mut(enemy_id).unwrap();
        e.selection_radius = 8.0;
        e.template_mut().geometry_info.authored = true;
        e.template_mut().geometry_info.major_radius = 8.0;
        e.template_mut().geometry_info.minor_radius = 8.0;
        e.template_mut().add_kind_of(KindOf::Vehicle);
    }

    game_logic.update_mines_and_demo_traps().unwrap();
    assert_eq!(
        game_logic.mine_residual_proximity_detonations(),
        1,
        "geometry contact must trip the pad"
    );
}

#[test]
fn land_mine_does_not_trip_when_geometry_misses() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);

    let _mine_id = game_logic
        .place_land_mine(Team::USA, Vec3::new(0.0, 0.0, 0.0), None)
        .expect("mine");
    let enemy_id = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
        .expect("vehicle");
    {
        let e = game_logic.host_object_mut(enemy_id).unwrap();
        e.selection_radius = 8.0;
        e.template_mut().geometry_info.authored = true;
        e.template_mut().geometry_info.major_radius = 8.0;
        e.template_mut().geometry_info.minor_radius = 8.0;
        e.template_mut().add_kind_of(KindOf::Vehicle);
    }

    game_logic.update_mines_and_demo_traps().unwrap();
    assert_eq!(
        game_logic.mine_residual_proximity_detonations(),
        0,
        "non-overlapping geometry must not trip"
    );
}

#[test]
fn dozer_mine_clear_defuses_demo_trap_without_detonation() {
    use crate::game_logic::host_mines::DOZER_MINE_CLEAR_PRE_ATTACK_FRAMES;

    let mut game_logic = GameLogic::new();
    ensure_test_dozer_template(&mut game_logic);

    let trap_id = game_logic
        .place_demo_trap(Team::GLA, Vec3::new(0.0, 0.0, 0.0), None)
        .expect("trap");
    // Outside TriggerDetonationRange 40, inside dozer scan 100.
    let dozer_id = game_logic
        .create_object("TestDozer", Team::USA, Vec3::new(50.0, 0.0, 0.0))
        .expect("dozer");
    if let Some(d) = game_logic.host_object_mut(dozer_id) {
        d.set_weapon_condition_at_frame(8, true, 0);
    }

    assert!(
        game_logic
            .host_object(trap_id)
            .unwrap()
            .is_disarmable_mine(),
        "demo trap is a DISARM target"
    );
    assert!(
        game_logic.clear_mine_internal(trap_id, dozer_id).unwrap(),
        "DISARM must defuse demo traps"
    );
    assert_eq!(game_logic.mine_residual_clears(), 1);
    assert_eq!(
        game_logic.mine_residual_proximity_detonations(),
        0,
        "defuse must not splash"
    );

    game_logic.frame = 1;
    game_logic.update_mines_and_demo_traps().unwrap();
    game_logic.frame = 1 + DOZER_MINE_CLEAR_PRE_ATTACK_FRAMES;
    game_logic.update_mines_and_demo_traps().unwrap();

    let gone_or_dead = game_logic
        .host_object(trap_id)
        .is_none_or(|t| t.status.destroyed || !t.is_alive());
    assert!(gone_or_dead, "defused demo trap must be destroyed");
}

#[test]
fn china_regen_pad_survives_disarm_and_refills() {
    use crate::game_logic::host_mines::{
        MINE_AUTO_HEAL_AMOUNT, MINE_AUTO_HEAL_START_DELAY_FRAMES, MINE_MIN_HEALTH,
        STANDARD_MINE_NUM_VIRTUAL,
    };
    let mut logic = GameLogic::new();
    let mine_id = logic
        .place_land_mine_named(
            "ChinaStandardMine",
            Team::China,
            Vec3::new(0.0, 0.0, 0.0),
            None,
        )
        .expect("pad");
    {
        let m = logic.host_object(mine_id).unwrap();
        assert!(m.mine_data.as_ref().unwrap().regenerates);
        assert_eq!(
            m.mine_data.as_ref().unwrap().virtual_mines_remaining,
            STANDARD_MINE_NUM_VIRTUAL
        );
    }
    assert!(logic.clear_mine_internal(mine_id, ObjectId(999)).unwrap());
    let pad = logic.host_object(mine_id).expect("regen pad kept");
    assert!(pad.is_alive(), "China pad must survive disarm");
    assert!(!pad.status.destroyed);
    assert!((pad.health.current - MINE_MIN_HEALTH).abs() < 1e-3);
    assert_eq!(pad.mine_data.as_ref().unwrap().virtual_mines_remaining, 0);

    logic.frame = 0;
    logic.update_mines_and_demo_traps().unwrap();
    logic.frame = MINE_AUTO_HEAL_START_DELAY_FRAMES;
    logic.update_mines_and_demo_traps().unwrap();
    let after = logic.host_object(mine_id).unwrap();
    assert!(
        after.health.current + 1e-3 >= MINE_MIN_HEALTH + MINE_AUTO_HEAL_AMOUNT,
        "AutoHeal must refill the pad (hp={})",
        after.health.current
    );
}

#[test]
fn weapon_fire_trips_virtual_mines_on_health_band() {
    let mut logic = GameLogic::new();
    let mine_id = logic
        .place_land_mine_named(
            "ChinaStandardMine",
            Team::China,
            Vec3::new(0.0, 0.0, 0.0),
            None,
        )
        .expect("pad");
    {
        let (m, health_events) = logic.host_object_and_health_events_mut(mine_id).unwrap();
        m.health.current = 50.0;
        m.health.maximum = 100.0;
        if let Some(md) = m.mine_data.as_mut() {
            md.last_synced_health = Some(100.0);
        }
    }
    logic.update_mines_and_demo_traps().unwrap();
    assert!(
        logic.mine_residual_proximity_detonations() >= 1,
        "health-band drop must detonateOnce"
    );
    let left = logic
        .host_object(mine_id)
        .and_then(|o| o.mine_data.as_ref().map(|m| m.virtual_mines_remaining))
        .unwrap_or(0);
    assert!(left < 8, "virtual count must drop, left={left}");
}

#[test]
fn lethal_weapon_hit_chain_detonates_virtual_mines() {
    use crate::game_logic::host_mines::{MINE_MIN_HEALTH, STANDARD_MINE_NUM_VIRTUAL};
    let mut logic = GameLogic::new();
    let mine_id = logic
        .place_land_mine_named(
            "ChinaStandardMine",
            Team::China,
            Vec3::new(0.0, 0.0, 0.0),
            None,
        )
        .expect("pad");
    {
        let (m, health_events) = logic.host_object_and_health_events_mut(mine_id).unwrap();
        m.health.current = 100.0;
        m.health.maximum = 100.0;
        if let Some(md) = m.mine_data.as_mut() {
            md.last_synced_health = Some(100.0);
        }
        let killed = m.take_damage_from(100.0, None, 0, health_events);
        assert!(!killed, "lethal hit must not silently destroy the pad");
        assert!(!m.status.destroyed);
        assert!(m.health.current <= 0.0);
    }
    logic.update_mines_and_demo_traps().unwrap();
    assert!(
        logic.mine_residual_proximity_detonations() >= STANDARD_MINE_NUM_VIRTUAL,
        "lethal hit must chain-detonate remaining virtuals, got {}",
        logic.mine_residual_proximity_detonations()
    );
    let pad = logic.host_object(mine_id).expect("regen pad kept");
    assert!(
        pad.is_alive(),
        "China regen pad must survive at MIN_HEALTH after chain"
    );
    assert!((pad.health.current - MINE_MIN_HEALTH).abs() < 1e-3);
    assert_eq!(pad.mine_data.as_ref().unwrap().virtual_mines_remaining, 0);
}

#[test]
fn sold_demo_trap_stops_proximity_scan() {
    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    let trap_id = logic
        .place_demo_trap(Team::GLA, Vec3::new(0.0, 0.0, 0.0), None)
        .expect("trap");
    let _enemy = logic
        .create_object("TestInfantry", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .expect("enemy");
    {
        let trap = logic.host_object_mut(trap_id).unwrap();
        trap.set_status_sold(true);
    }
    logic.update_mines_and_demo_traps().unwrap();
    assert_eq!(
        logic.mine_residual_proximity_detonations(),
        0,
        "sold DemoTrapUpdate must return before proximity scan"
    );
    let trap = logic.host_object(trap_id).expect("trap kept");
    assert!(trap.is_alive());
    assert!(!trap.mine_data.as_ref().unwrap().detonated);
}

#[test]
fn land_mine_trips_neutral_unit() {
    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    let mine_id = logic
        .place_land_mine(Team::China, Vec3::new(0.0, 0.0, 0.0), None)
        .expect("mine");
    let trigger = logic
        .host_object(mine_id)
        .unwrap()
        .mine_data
        .as_ref()
        .unwrap()
        .trigger_range;
    let _civ = logic
        .create_object(
            "TestInfantry",
            Team::Neutral,
            Vec3::new(trigger * 0.25, 0.0, 0.0),
        )
        .expect("neutral");
    logic.update_mines_and_demo_traps().unwrap();
    assert_eq!(
        logic.mine_residual_proximity_detonations(),
        1,
        "neutral must trip land mines"
    );
}

#[test]
fn emp_mine_neutron_unmans_vehicle_and_kills_infantry() {
    use crate::game_logic::host_mines::HostMineData;
    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    let mine_id = logic
        .place_land_mine(Team::China, Vec3::new(0.0, 0.0, 0.0), None)
        .expect("emp pad");
    if let Some(obj) = logic.host_object_mut(mine_id) {
        obj.mine_data = Some(HostMineData::land_mine_for_template("ChinaEMPMine"));
    }
    let inf = logic
        .create_object("TestInfantry", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .expect("infantry");
    let tank = logic
        .create_object("TestInfantry", Team::USA, Vec3::new(8.0, 0.0, 0.0))
        .expect("tank");
    {
        let t = logic.host_object_mut(tank).unwrap();
        t.template_mut().kind_of.remove(&KindOf::Infantry);
        t.template_mut().add_kind_of(KindOf::Vehicle);
        t.health.current = 400.0;
        t.health.maximum = 400.0;
    }
    logic.update_mines_and_demo_traps().unwrap();
    let inf_dead = logic
        .host_object(inf)
        .is_none_or(|o| o.status.destroyed || !o.is_alive());
    assert!(inf_dead, "neutron blast must kill infantry");
    let tank_obj = logic.host_object(tank).expect("tank remains");
    assert!(tank_obj.is_unmanned(), "neutron blast must unman vehicles");
    assert_eq!(tank_obj.team, Team::Neutral);
    assert!(
        (tank_obj.health.current - 400.0).abs() < 0.01,
        "EMP pad must not deal 100 HE to vehicles"
    );
}

#[test]
fn demo_trap_proximity_uses_height_not_aircraft_kind() {
    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    let trap_id = logic
        .place_demo_trap(Team::GLA, Vec3::new(0.0, 0.0, 0.0), None)
        .expect("trap");
    let jumper = logic
        .create_object("TestInfantry", Team::USA, Vec3::new(10.0, 5.0, 0.0))
        .expect("jumper");
    logic.update_mines_and_demo_traps().unwrap();
    assert!(
        logic
            .host_object(trap_id)
            .and_then(|t| t.mine_data.as_ref())
            .is_some_and(|d| !d.demo_trap_warning_armed() && !d.detonated),
        "paratrooper / jump-jet above terrain must not trip"
    );
    let _ = jumper;
    let chinook = logic
        .create_object("TestInfantry", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .expect("chinook");
    {
        let c = logic.host_object_mut(chinook).unwrap();
        c.template_mut().add_kind_of(KindOf::Aircraft);
        c.status.airborne_target = false;
    }
    logic.update_mines_and_demo_traps().unwrap();
    assert!(
        logic
            .host_object(trap_id)
            .and_then(|t| t.mine_data.as_ref())
            .is_some_and(|d| d.demo_trap_warning_armed()),
        "landed aircraft must trip demo trap"
    );
}

#[test]
fn bored_dozer_arms_mine_clearing_weaponset() {
    use crate::game_logic::host_mines::HostMineKind;
    let mut logic = GameLogic::new();
    logic.add_player(crate::game_logic::Player::new(0, Team::USA, "USA", true));
    ensure_test_dozer_template(&mut logic);
    let mid = logic
        .place_land_mine(Team::GLA, Vec3::new(10.0, 0.0, 0.0), None)
        .expect("mine");
    let _ = mid;
    let did = logic
        .create_object("TestDozer", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("dozer");
    let ai_entry_frame = logic.frame;
    if let Some(o) = logic.host_object_mut(did) {
        o.set_ai_state(
            AIState::Idle,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        o.idle_since_frame = 1;
        assert!(!o.test_weapon_condition(8));
    }
    logic.frame = 1 + crate::game_logic::host_repair::DOZER_BORED_TIME_FRAMES;
    logic.update_dozer_bored_repair();
    let d = logic.host_object(did).expect("d");
    assert!(
        d.test_weapon_condition(8),
        "idle dozer must arm WEAPONSET_MINE_CLEARING_DETAIL"
    );
    let _ = HostMineKind::LandMine;
}

#[test]
fn cluster_mines_drop_variance_offsets_field() {
    use crate::game_logic::host_mines::cluster_smart_border_positions;
    let mut logic = GameLogic::new();
    let click = Vec3::new(100.0, 0.0, 200.0);
    let ids = logic.place_cluster_mines(Team::China, click, Some(ObjectId(7)));
    assert!(!ids.is_empty());
    let raw = cluster_smart_border_positions(click);
    let placed: Vec<Vec3> = ids
        .iter()
        .filter_map(|id| logic.host_object(*id).map(|o| o.get_position()))
        .collect();
    let raw_cx: f32 = raw.iter().map(|p| p.x).sum::<f32>() / raw.len() as f32;
    let pl_cx: f32 = placed.iter().map(|p| p.x).sum::<f32>() / placed.len() as f32;
    let raw_cz: f32 = raw.iter().map(|p| p.z).sum::<f32>() / raw.len() as f32;
    let pl_cz: f32 = placed.iter().map(|p| p.z).sum::<f32>() / placed.len() as f32;
    assert!(
        (raw_cx - pl_cx).abs() > 0.05 || (raw_cz - pl_cz).abs() > 0.05,
        "DropVariance must offset the SmartBorder field from the click"
    );
}

#[test]
fn stealth_residual_not_auto_targeted_until_detected() {
    use crate::ai_decisions::AIDecisionSystem;

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    game_logic.templates.get_mut("TestInfantry").unwrap().stealth_update =
        Some(crate::game_logic::object::StealthUpdateData {
            stealth_delay_frames: 0,
            innate_stealth: false,
            granted_by_special_power: true,
            ..Default::default()
        });

    let attacker_id = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("attacker");
    {
        let a = game_logic.host_object_mut(attacker_id).unwrap();
        crate::fixture_content::arm_with_stats(
            a,
            crate::game_logic::Weapon {
                damage: 10.0,
                range: 150.0,
                ..Default::default()
            },
        );
    }

    let stealth_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .expect("stealthed");
    {
        let s = game_logic.host_object_mut(stealth_id).unwrap();
        s.apply_grant_stealth(0);
        s.set_status_detected(false);
    }

    // No detector: auto-target search must skip stealthed unit.
    let found = AIDecisionSystem::find_best_target(
        &game_logic,
        attacker_id,
        Vec3::ZERO,
        Team::China,
        200.0,
        false,
        true,
        false,
    );
    assert!(
        found.is_none(),
        "stealthed+undetected must not be auto-targeted"
    );
    assert!(
        AIDecisionSystem::find_nearest_enemy(&game_logic, Vec3::ZERO, Team::China, 200.0).is_none(),
        "nearest-enemy must ignore stealthed+undetected"
    );

    // Spawn detector near stealthed unit.
    let detector_id = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(25.0, 0.0, 0.0))
        .expect("detector");
    {
        let d = game_logic.host_object_mut(detector_id).unwrap();
        d.is_detector = true;
        d.detection_range = 50.0;
    }

    game_logic.update_stealth_and_detection().unwrap();
    game_logic.update_stealth_and_detection().unwrap();

    let stealth = game_logic.host_object(stealth_id).unwrap();
    assert!(
        stealth.status.detected,
        "detector in range must mark stealthed unit detected"
    );
    assert!(
        !stealth.is_effectively_stealthed(),
        "detected stealthed unit is no longer effectively stealthed"
    );
    assert!(
        stealth.is_targetable_by_enemy_of(Team::China),
        "detected unit must become targetable"
    );

    let found_after = AIDecisionSystem::find_best_target(
        &game_logic,
        attacker_id,
        Vec3::ZERO,
        Team::China,
        200.0,
        false,
        true,
        false,
    );
    assert_eq!(
        found_after,
        Some(stealth_id),
        "after detection, stealthed unit becomes auto-targetable"
    );
}

/// C++ firing does not touch STEALTHED directly: the inherited
/// DefaultThingTemplate StealthUpdate (FIRING_PRIMARY/SECONDARY/TERTIARY)
/// destalths on its next update via `allowedToStealth`
/// (StealthUpdate.cpp:324-362, 717-752).
#[test]
fn stealth_residual_fire_breaks_stealth() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    game_logic.templates.get_mut("TestInfantry").unwrap().stealth_update =
        Some(crate::game_logic::object::StealthUpdateData {
            innate_stealth: false,
            granted_by_special_power: true,
            forbidden_conditions: crate::game_logic::object::StealthForbiddenConditions::FIRING_WEAPON,
            ..Default::default()
        });

    let shooter_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("shooter");
    let target_id = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(10.0, 0.0, 0.0))
        .expect("target");

    {
        let _ = game_logic.host_object_mut(shooter_id).unwrap();
        let s = game_logic.objects.get_mut(&shooter_id).unwrap();
        s.apply_grant_stealth(game_logic.frame);
        crate::fixture_content::arm_with_stats(
            s,
            crate::game_logic::Weapon {
                damage: 5.0,
                range: 100.0,
                reload_time: 0.5,
                // ready immediately
                ..Default::default()
            },
        );
        s.weapon.as_mut().unwrap().last_fire_time = -1.0;
        assert!(s.fire_at(
            target_id,
            0.0,
            game_logic.frame,
            &mut game_logic.combat_system,
            &mut game_logic.attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        ));
    }
    game_logic
        .objects
        .get_mut(&shooter_id)
        .unwrap()
        .set_status_firing_weapon(true);
    game_logic.update_stealth_and_detection().unwrap();
    assert!(
        !game_logic.host_object(shooter_id).unwrap().status.stealthed,
        "firing destalths on the next StealthUpdate"
    );
}

#[test]
fn stealth_residual_detection_expires() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    game_logic.templates.get_mut("TestInfantry").unwrap().stealth_update =
        Some(crate::game_logic::object::StealthUpdateData {
            stealth_delay_frames: 0,
            innate_stealth: false,
            granted_by_special_power: true,
            ..Default::default()
        });

    let stealth_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("stealth");
    game_logic.host_object_mut(stealth_id).unwrap().apply_grant_stealth(0);
    game_logic.mark_stealth_detected(stealth_id, 5);
    assert!(!game_logic.host_object(stealth_id).unwrap().status.detected);

    game_logic.frame = 4;
    game_logic.update_stealth_and_detection().unwrap();
    assert!(
        game_logic.host_object(stealth_id).unwrap().status.detected,
        "must remain detected before expiry frame"
    );

    game_logic.frame = 5;
    game_logic.update_stealth_and_detection().unwrap();
    let stealth = game_logic.host_object(stealth_id).unwrap();
    assert!(
        !stealth.status.detected && stealth.status.stealthed,
        "detected clears at expiry; stealthed may remain"
    );
    assert!(stealth.is_effectively_stealthed());
}

#[test]
fn base_defense_residual_patriot_auto_fires_without_attack_object() {
    // Use the retail powered battery and missile content: a frame's combat
    // phase launches a projectile, it does not apply the warhead immediately.
    let (mut game_logic, patriot_id, enemy_id) =
        crate::game_logic::game_logic::accepted_projectile_owner_tests::
            patriot_retail_battery_tests::world();

    {
        let p = game_logic.host_object(patriot_id).expect("patriot obj");
        assert!(
            p.weapon.is_some(),
            "Patriot must bind residual primary weapon"
        );
        assert!(
            p.secondary_weapon.is_some(),
            "Patriot must bind residual AA secondary"
        );
        assert!(
            p.can_attack(),
            "Patriot residual must be able to attack when armed"
        );
        assert!(
            crate::game_logic::host_base_defense::is_base_defense_structure(
                &p.template_name,
                p.is_kind_of(KindOf::Structure),
                p.is_kind_of(KindOf::FSBaseDefense),
            ),
            "USA_Patriot must classify as base defense residual"
        );
        // No AttackObject: stay Idle with no target.
        assert_eq!(p.ai_state(), AIState::Idle);
        assert!(p.target.is_none());
    }
    {
        let p = game_logic.host_object_mut(patriot_id).unwrap();
        if let Some(w) = p.weapon.as_mut() {
            w.last_fire_time = -10.0;
        }
    }

    let enemy_hp_before = game_logic
        .host_object(enemy_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    game_logic.health_events.clear_damage();

    // Drive whole logic frames with the receiving asset owner, including
    // acquisition, projectile admission, physics and collision callbacks.
    let assets = game_logic.application_assets().expect("retail W3D owner");
    let mut observed_owned_flight = false;
    for _ in 0..80 {
        let effects = crate::game_logic::ScriptPresentationEffects {
            pristine_bones: Some(assets.as_ref()),
            ..Default::default()
        };
        game_logic.tick_logic_frame_with_script_effects(
            LOGIC_FRAME_TIMESTEP, None, None, effects, |_| {}, |_| {},
        );
        observed_owned_flight |= game_logic.combat_system.projectiles_snapshot()
            .iter().any(|projectile| projectile.missile_motion.is_some());
    }
    assert!(observed_owned_flight, "auto-fire must admit a real Patriot missile flight");

    let enemy_hp_after = game_logic
        .host_object(enemy_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let logged = game_logic.health_events.drain_damage();
    let log_hit = logged
        .iter()
        .any(|e| e.target == enemy_id && e.amount > 0.0);
    assert!(
        enemy_hp_after < enemy_hp_before || log_hit,
        "Patriot residual auto-fire must damage nearby enemy without AttackObject (before={enemy_hp_before}, after={enemy_hp_after}, log_hit={log_hit})"
    );
    assert!(
        game_logic.honesty_base_defense_fire_ok(),
        "base-defense residual honesty must record auto-fire"
    );
    assert!(
        game_logic.base_defense_residual_fires() > 0,
        "expected residual fire counter > 0"
    );
    assert!(
        game_logic.honesty_patriot_ok(),
        "Patriot residual honesty must record ground fire"
    );
    assert!(
        game_logic.patriot_residual_ground_fires() > 0,
        "expected Patriot ground residual fire counter > 0"
    );
}

#[test]
fn auto_fire_commits_its_target_before_the_frame_is_published() {
    let mut logic = GameLogic::new();

    // Ordering hazard (R6): do NOT create a second GameLogic after this
    // point — a fresh instance republishes all-off defaults and would strip
    // the authority snapshot the deep readers below consult.
    ensure_test_tank_template(&mut logic);
    let attacker = logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("atk");
    let victim = logic
        .create_object("TestTank", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
        .expect("vic");
    // Arm attacker with a residual weapon for apply_damage path.
    {
        let o = logic.host_object_mut(attacker).unwrap();
        crate::fixture_content::arm_with_stats(
            o,
            crate::game_logic::Weapon {
                damage: 10.0,
                range: 200.0,
                min_range: 0.0,
                reload_time: 0.1,
                clip_size: 0,
                clip_reload_time: 0.0,
                can_target_air: false,
                can_target_ground: true,
                projectile_speed: 0.0,
                pre_attack_delay: 0.0,
                splash_radius: 0.0,
                ..Default::default()
            },
        );
        o.weapon.as_mut().unwrap().last_fire_time = -10.0;
    }
    assert!(logic.host_object(attacker).unwrap().target.is_none());
    let weapon = logic.host_object(attacker).and_then(|o| o.weapon.clone());
    let pos = logic.host_object(attacker).unwrap().get_position();
    let _ = logic
        .residual_auto_fire_apply_damage(attacker, victim, 10.0, pos, weapon.as_ref(), 0)
        .unwrap();
    assert_eq!(logic.host_object(attacker).unwrap().target, Some(victim));
    let frame =
        crate::presentation_frame::PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let actor = frame.objects.iter().find(|row| row.id == attacker).unwrap();
    assert_eq!(actor.attack_target, Some(victim));
}

#[test]
fn stinger_site_residual_dual_fire_and_ap_rockets() {
    use crate::game_logic::host_base_defense::{
        STINGER_AIR_DAMAGE, STINGER_AP_ROCKETS_DAMAGE_MULT, STINGER_GROUND_DAMAGE,
        STINGER_GROUND_RANGE, STINGER_PRIMARY_WEAPON, STINGER_SECONDARY_WEAPON,
        is_stinger_site_structure,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    // The authored StingerMissile flies MissileAIUpdate on its authored
    // StingerMissileLocomotor (WeaponObjects.ini:2735). Install the projectile
    // profile first so locomotor admission binds it, then admit the catalog
    // before Objects are created.
    game_logic.projectile_definitions =
        crate::game_logic::projectile_definitions::fixture_projectile_definitions();
    let mut locomotors = game_engine::common::ini::ini_locomotor::LocomotorStore::new_owned();
    locomotors
        .apply_source(
            r#"
Locomotor StingerMissileLocomotor
  Surfaces = AIR
  Speed = 400
  MinSpeed = 120
  Acceleration = 900
  Braking = 0
  TurnRate = 625
  MaxThrustAngle = 60
  AllowAirborneMotiveForce = Yes
  Appearance = THRUST
  CloseEnoughDist3D = No
End
"#,
        )
        .unwrap();
    game_logic.admit_locomotor_definitions(locomotors).unwrap();
    assert!(
        game_logic
            .projectile_definitions
            .get("StingerMissile")
            .unwrap()
            .locomotor
            .is_some()
    );
    ensure_test_tank_template(&mut game_logic);

    let mut stinger_tpl = crate::game_logic::ThingTemplate::new("GLA_StingerSite");
    stinger_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::FSBaseDefense)
        // Retail GLAStingerSite is a hive: SPAWNS_ARE_THE_WEAPONS + SpawnBehavior
        // (SpawnTemplateName = GLAInfantryStingerSoldier), FactionBuilding.ini.
        .add_kind_of(KindOf::SpawnsAreTheWeapons)
        .set_health(400.0)
        .set_primary_weapon_name(STINGER_PRIMARY_WEAPON)
        .set_secondary_weapon_name(STINGER_SECONDARY_WEAPON);
    game_logic
        .templates
        .insert("GLA_StingerSite".to_string(), stinger_tpl);

    // Spawn soldiers are the weapons; the hive cannot fire with zero of them.
    let mut soldier_tpl = crate::game_logic::ThingTemplate::new("GLAInfantryStingerSoldier");
    soldier_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(50.0);
    game_logic
        .templates
        .insert("GLAInfantryStingerSoldier".to_string(), soldier_tpl);

    // StingerMissileWeapon is a ProjectileObject weapon (Weapon.ini
    // ProjectileObject = StingerMissile); the authored fire must be able to
    // spawn the missile object the same way OCL projectile spawns do.
    let mut missile_tpl = crate::game_logic::ThingTemplate::new("StingerMissile");
    missile_tpl
        .add_kind_of(KindOf::Projectile)
        .add_kind_of(KindOf::Aircraft)
        .set_health(100.0);
    game_logic
        .templates
        .insert("StingerMissile".to_string(), missile_tpl);

    let mut aircraft_tpl = crate::game_logic::ThingTemplate::new("TestAircraft");
    aircraft_tpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::Selectable)
        .set_health(200.0);
    game_logic
        .templates
        .insert("TestAircraft".to_string(), aircraft_tpl);

    let stinger_id = game_logic
        .create_object("GLA_StingerSite", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("stinger");
    // C++ SpawnBehavior spawns the soldier children on build; the residual
    // hive refuses to fire while it has zero live spawns.
    for i in 0..3 {
        let soldier_id = game_logic
            .create_object(
                "GLAInfantryStingerSoldier",
                Team::GLA,
                Vec3::new(5.0 + i as f32, 0.0, 0.0),
            )
            .expect("stinger soldier");
        let birth = game_logic
            .host_object(soldier_id)
            .expect("soldier obj")
            .visual_object_generation;
        game_logic
            .host_object_mut(stinger_id)
            .unwrap()
            .remember_spawn_behavior_child(soldier_id, birth, None);
    }
    {
        let s = game_logic.host_object(stinger_id).expect("stinger obj");
        assert!(is_stinger_site_structure(&s.template_name));
        let prim = s.weapon.as_ref().expect("stinger ground residual");
        assert!((prim.damage - STINGER_GROUND_DAMAGE).abs() < 0.01);
        assert!((prim.range - STINGER_GROUND_RANGE).abs() < 1.0);
        assert!(!prim.can_target_air);
        let sec = s.secondary_weapon.as_ref().expect("stinger AA residual");
        assert!((sec.damage - STINGER_AIR_DAMAGE).abs() < 0.01);
        assert!(sec.can_target_air);
        assert!(!sec.can_target_ground);
    }

    // Ground residual auto-fire.
    let enemy_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .expect("enemy tank");
    {
        let s = game_logic.host_object_mut(stinger_id).unwrap();
        if let Some(w) = s.weapon.as_mut() {
            w.last_fire_time = -10.0;
        }
    }
    let hp_before = game_logic
        .host_object(enemy_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    for f in 0..80 {
        game_logic.frame = f;
        // StingerMissile has a finite authored WeaponSpeed: the residual shot
        // queues WeaponSpeed-delayed damage that the tick drains per frame
        // (C++ WeaponStore::update / ProjectileUpdate).
        super::super::helpers::step_authored_combat(&mut game_logic, &[stinger_id, enemy_id]);
    }
    let hp_after = game_logic
        .host_object(enemy_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        hp_after < hp_before,
        "Stinger ground residual must damage tank (before={hp_before}, after={hp_after})"
    );
    assert!(game_logic.honesty_stinger_site_ok());
    assert!(game_logic.stinger_site_residual_ground_fires() > 0);
    assert!(game_logic.honesty_base_defense_fire_ok());

    // AA residual auto-fire vs aircraft at AA range.
    let air_id = game_logic
        .create_object("TestAircraft", Team::USA, Vec3::new(300.0, 0.0, 0.0))
        .expect("aircraft");
    {
        let a = game_logic.host_object_mut(air_id).unwrap();
        a.status.airborne_target = true;
    }
    {
        let ai_entry_frame = game_logic.frame;
        let s = game_logic.host_object_mut(stinger_id).unwrap();
        if let Some(w) = s.secondary_weapon.as_mut() {
            w.last_fire_time = -10.0;
        }
        // Drop ground target so AA scan prefers aircraft.
        s.target = None;
        s.set_ai_state(
            AIState::Idle,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }
    // Remove ground tank from consideration by destroying it.
    if let Some(t) = game_logic.host_object_mut(enemy_id) {
        t.health.current = 0.0;
    }
    let air_hp_before = game_logic
        .host_object(air_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    for f in 80..160 {
        game_logic.frame = f;
        super::super::helpers::step_authored_combat(&mut game_logic, &[stinger_id, air_id]);
    }
    let air_hp_after = game_logic
        .host_object(air_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        air_hp_after < air_hp_before,
        "Stinger AA residual must damage aircraft (before={air_hp_before}, after={air_hp_after})"
    );
    assert!(
        game_logic.honesty_stinger_site_aa_ok(),
        "Stinger AA residual honesty"
    );
    assert!(game_logic.stinger_site_residual_aa_fires() > 0);

    // AP Rockets residual × 1.25.
    assert!(game_logic.apply_stinger_ap_rockets_upgrade(stinger_id));
    {
        let s = game_logic
            .host_object(stinger_id)
            .expect("stinger after AP");
        let prim = s.weapon.as_ref().expect("ground");
        assert!(
            (prim.damage - STINGER_GROUND_DAMAGE * STINGER_AP_ROCKETS_DAMAGE_MULT).abs() < 0.01,
            "AP Rockets ground dmg expected {}, got {}",
            STINGER_GROUND_DAMAGE * STINGER_AP_ROCKETS_DAMAGE_MULT,
            prim.damage
        );
        let sec = s.secondary_weapon.as_ref().expect("air");
        assert!((sec.damage - STINGER_AIR_DAMAGE * STINGER_AP_ROCKETS_DAMAGE_MULT).abs() < 0.01);
    }
    assert!(game_logic.stinger_site_residual_ap_rockets_upgrades() > 0);
}


/// CamoNetting StealthUpgrade grants CAN_STEALTH (StealthUpgrade.cpp:27-42);
/// the structures' authored StealthUpdate (InnateStealth No, ATTACKING
/// USING_ABILITY TAKING_DAMAGE, OrderIdleEnemiesToAttackMeUponReveal) then
/// cloaks after StealthDelay and reveals on its next update
/// (StealthUpdate.cpp:234-401, 717-752, 846-912).
#[test]
fn camo_netting_structure_attack_and_damage_reveal() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::host_upgrades::{
        CAMO_NETTING_FRIENDLY_OPACITY_MAX, CAMO_NETTING_FRIENDLY_OPACITY_MIN,
        CAMO_NETTING_STEALTH_DELAY_FRAMES, HostUpgradeKind, UPGRADE_GLA_CAMO_NETTING,
    };
    use crate::game_logic::object::{StealthForbiddenConditions as C, StealthUpdateData};

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::GLA, "GLA", true);
    player.resources.supplies = 5000;
    game_logic.add_player(player);
    ensure_test_barracks_template(&mut game_logic);
    ensure_test_tank_template(&mut game_logic);

    for name in ["GLATunnelNetwork", "GLAStingerSite"] {
        let mut t = crate::game_logic::ThingTemplate::new(name);
        t.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .add_kind_of(KindOf::FSBaseDefense)
            .set_health(1000.0);
        t.stealth_upgrades = vec![crate::game_logic::UpgradeMuxTriggers {
            triggered_by: vec!["Upgrade_GLACamoNetting".into()],
            ..Default::default()
        }];
        t.stealth_update = Some(StealthUpdateData {
            stealth_delay_frames: CAMO_NETTING_STEALTH_DELAY_FRAMES,
            forbidden_conditions: C::ATTACKING
                .union(C::USING_ABILITY)
                .union(C::TAKING_DAMAGE),
            friendly_opacity_min: CAMO_NETTING_FRIENDLY_OPACITY_MIN,
            friendly_opacity_max: CAMO_NETTING_FRIENDLY_OPACITY_MAX,
            order_idle_enemies_to_attack_me_upon_reveal: true,
            innate_stealth: false,
            ..Default::default()
        });
        game_logic.templates.insert(name.to_string(), t);
    }

    let barracks_id = game_logic
        .create_object("TestBarracks", Team::GLA, Vec3::new(-80.0, 0.0, 0.0))
        .expect("barracks");
    let tunnel_id = game_logic
        .create_object("GLATunnelNetwork", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("tunnel");
    let stinger_id = game_logic
        .create_object("GLAStingerSite", Team::GLA, Vec3::new(100.0, 0.0, 0.0))
        .expect("stinger");
    for _ in 0..32 {
        game_logic.update();
    }
    for id in [tunnel_id, stinger_id] {
        assert!(
            !game_logic.host_object(id).unwrap().innate_stealth,
            "InnateStealth No: no CAN_STEALTH before the upgrade"
        );
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::QueueUpgrade {
            upgrade_name: UPGRADE_GLA_CAMO_NETTING.to_string(),
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![barracks_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    // C++ ProductionUpdate.cpp:686-704 advances research once per logic frame.
    game_logic.update();
    for _ in 0..HostUpgradeKind::CamoNetting.retail_research_frames() {
        game_logic.update();
    }
    assert!(
        game_logic
            .players
            .get(&0)
            .is_some_and(|player| player.has_unlocked_upgrade(UPGRADE_GLA_CAMO_NETTING)),
        "producer research must complete before exercising CamoNetting stealth"
    );

    // While CAN_STEALTH was clear every update re-armed StealthDelay.
    for id in [tunnel_id, stinger_id] {
        let o = game_logic.host_object(id).unwrap();
        assert!(o.innate_stealth, "StealthUpgrade sets CAN_STEALTH");
        assert!(!o.status.stealthed, "cloak waits StealthDelay");
        assert!(o.stealth_allowed_frame > game_logic.frame);
    }
    let cloak_at = game_logic
        .host_object(tunnel_id)
        .map(|o| o.stealth_allowed_frame)
        .unwrap_or(0);
    game_logic.frame = cloak_at;
    game_logic.update_stealth_and_detection().unwrap();
    for id in [tunnel_id, stinger_id] {
        let o = game_logic.host_object(id).unwrap();
        assert!(o.status.stealthed, "cloaks once StealthDelay elapsed");
        assert!(
            o.camo_friendly_opacity >= CAMO_NETTING_FRIENDLY_OPACITY_MIN - 0.01
                && o.camo_friendly_opacity <= CAMO_NETTING_FRIENDLY_OPACITY_MAX + 0.01,
            "friendly pulse stays within FriendlyOpacityMin..Max, got {}",
            o.camo_friendly_opacity
        );
    }

    // TAKING_DAMAGE: the next update reveals and re-arms StealthDelay.
    let _ = game_logic.apply_host_damage(tunnel_id, 10.0).unwrap();
    game_logic.update_stealth_and_detection().unwrap();
    {
        let o = game_logic.host_object(tunnel_id).unwrap();
        assert!(!o.status.stealthed, "damage reveals CamoNetting structure");
        assert!(o.stealth_allowed_frame > game_logic.frame);
    }

    // ATTACKING: IS_FIRING_WEAPON reveals.
    game_logic.frame += 1;
    {
        let s = game_logic.host_object_mut(stinger_id).unwrap();
        s.set_status_firing_weapon(true);
    }
    game_logic.update_stealth_and_detection().unwrap();
    assert!(
        !game_logic.host_object(stinger_id).unwrap().status.stealthed,
        "firing reveals CamoNetting structure"
    );

    // USING_ABILITY: forbids stealth even once the delay has elapsed.
    {
        let s = game_logic.host_object_mut(stinger_id).unwrap();
        s.set_status_firing_weapon(false);
        s.set_status_using_ability(true);
    }
    game_logic.frame = game_logic
        .host_object(stinger_id)
        .unwrap()
        .stealth_allowed_frame;
    game_logic.update_stealth_and_detection().unwrap();
    assert!(
        !game_logic.host_object(stinger_id).unwrap().status.stealthed,
        "using an ability keeps CamoNetting structure revealed"
    );

    // Idle + StealthDelay elapsed: the tunnel re-cloaks.
    {
        let t = game_logic.host_object_mut(tunnel_id).unwrap();
        t.last_damage_timestamp = None;
    }
    game_logic.frame = game_logic
        .host_object(tunnel_id)
        .unwrap()
        .stealth_allowed_frame
        .max(game_logic.frame);
    game_logic.update_stealth_and_detection().unwrap();
    assert!(
        game_logic.host_object(tunnel_id).unwrap().status.stealthed,
        "idle after StealthDelay re-cloaks CamoNetting structure"
    );

    // markAsDetected wakes idle enemies that see the structure.
    let enemy_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(30.0, 0.0, 0.0))
        .expect("enemy tank");
    {
        let e = game_logic.host_object_mut(enemy_id).unwrap();
        e.target = None;
        e.vision_range = 300.0;
        crate::fixture_content::arm_with_stats(
            e,
            crate::game_logic::Weapon {
                damage: 10.0,
                range: 200.0,
                reload_time: 1.0,
                ..Default::default()
            },
        );
        e.weapon.as_mut().unwrap().last_fire_time = -10.0;
    }
    game_logic.mark_stealth_detected(tunnel_id, 0);
    assert!(!game_logic.host_object(tunnel_id).unwrap().status.detected);
    assert!(game_logic.host_object(tunnel_id).unwrap().detection_expires_frame > game_logic.frame);
    assert_eq!(
        game_logic.host_object(enemy_id).unwrap().target,
        Some(tunnel_id),
        "OrderIdleEnemiesToAttackMeUponReveal wakes the idle enemy"
    );
}

#[test]
fn patriot_residual_aa_secondary_auto_fires() {
    use crate::game_logic::host_base_defense::{
        PATRIOT_AIR_DAMAGE, PATRIOT_AIR_RANGE, PATRIOT_GROUND_DAMAGE, is_patriot_battery_structure,
    };
    let (mut game_logic, patriot_id, air_id) =
        crate::game_logic::game_logic::accepted_projectile_owner_tests::
            patriot_retail_battery_tests::world_with_air_target();
    {
        let p = game_logic.host_object(patriot_id).expect("patriot");
        assert!(is_patriot_battery_structure(&p.template_name));
        let prim = p.weapon.as_ref().expect("ground");
        assert!((prim.damage - PATRIOT_GROUND_DAMAGE).abs() < 0.01);
        // hq-2qedt.32.2: retail AA lives in TERTIARY (SECONDARY = assist weapon).
        let sec = p.tertiary_weapon.as_ref().expect("AA");
        assert!((sec.damage - PATRIOT_AIR_DAMAGE).abs() < 0.01);
        assert!((sec.range - PATRIOT_AIR_RANGE).abs() < 1.0);
        assert!(sec.can_target_air);
        assert!(!sec.can_target_ground);
    }


    let air_hp_before = game_logic
        .host_object(air_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    game_logic.health_events.clear_damage();
    let assets = game_logic.application_assets().expect("retail W3D owner");
    let mut observed_aa_flight = false;
    for _ in 0..80 {
        let effects = crate::game_logic::ScriptPresentationEffects {
            pristine_bones: Some(assets.as_ref()),
            ..Default::default()
        };
        game_logic.tick_logic_frame_with_script_effects(
            LOGIC_FRAME_TIMESTEP, None, None, effects, |_| {}, |_| {},
        );
        observed_aa_flight |= game_logic.combat_system.projectiles_snapshot().iter().any(|projectile| {
            projectile.target_id == Some(air_id)
                && projectile.missile_motion.is_some()
                && (projectile.damage - PATRIOT_AIR_DAMAGE).abs() < 0.01
        });
    }
    assert!(observed_aa_flight, "retail tertiary weapon must launch an owned AA missile");
    let air_hp_after = game_logic
        .host_object(air_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let dealt = test_observed_damage_to(
        &game_logic.health_events,
        air_id,
        air_hp_before,
        air_hp_after,
    );
    assert!(
        dealt > 0.0 || air_hp_after < air_hp_before,
        "Patriot AA residual must damage aircraft (dealt={dealt}, before={air_hp_before}, after={air_hp_after})"
    );
    assert!(game_logic.honesty_patriot_aa_ok());
    assert!(game_logic.patriot_residual_aa_fires() > 0);
    assert!(game_logic.honesty_base_defense_fire_ok());
}

#[test]
fn patriot_assisted_targeting_request_assist_range_residual() {
    use crate::game_logic::host_base_defense::{
        PATRIOT_ASSIST_DAMAGE, PATRIOT_ASSISTING_CLIP_SIZE, PATRIOT_REQUEST_ASSIST_RANGE,
    };
    // C++ assistance uses equivalent templates, the actual requester's
    // WeaponTemplate range, and the assistant's AssistedTargetingUpdate.
    let (mut game_logic, requester_id, enemy_id) =
        crate::game_logic::game_logic::accepted_projectile_owner_tests::
            patriot_retail_battery_tests::world();
    // Three batteries consume nine Energy; admit actual retail producers.
    for z in [30.0, 50.0] {
        game_logic.create_object_for_player("AmericaPowerPlant", 0, Vec3::new(10.0, 0.0, z)).unwrap();
    }
    let assistant_id = game_logic.create_object_for_player(
        "AmericaPatriotBattery", 0,
        Vec3::new(100.0, 30.0, 100.0 + PATRIOT_REQUEST_ASSIST_RANGE * 0.5),
    ).unwrap();
    let far_id = game_logic.create_object_for_player(
        "AmericaPatriotBattery", 0, Vec3::new(10.0, 30.0, 310.0),
    ).unwrap();
    assert!(!game_logic.host_object(assistant_id).unwrap().is_disabled());
    assert!(!game_logic.host_object(far_id).unwrap().is_disabled());
    {
        let enemy = game_logic.host_object_mut(enemy_id).unwrap();
        enemy.health.current = 500.0;
        enemy.health.maximum = 500.0;
        enemy.max_health = 500.0;
    }
    // An attack may limit its own shots; the assisting AI receives its own
    // authored four-shot budget, independent of this requester's budget.
    game_logic.host_object_mut(requester_id).unwrap().set_max_shots_to_fire(1);

    let enemy_hp_before = game_logic
        .host_object(enemy_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    game_logic.health_events.clear_damage();

    let assets = game_logic.application_assets().expect("retail W3D owner");
    let mut observed_assist_flight = false;
    // Let the real turret aim and fire; capture the actual admitted missiles.
    for _ in 0..80 {
        let effects = crate::game_logic::ScriptPresentationEffects {
            pristine_bones: Some(assets.as_ref()),
            ..Default::default()
        };
        game_logic.tick_logic_frame_with_script_effects(
            LOGIC_FRAME_TIMESTEP, None, None, effects, |_| {}, |_| {},
        );
        observed_assist_flight |= game_logic.combat_system.projectiles_snapshot().iter().any(|p| {
            p.shooter_id == assistant_id && p.target_id == Some(enemy_id)
                && p.missile_motion.is_some()
                && (p.damage - PATRIOT_ASSIST_DAMAGE).abs() < 0.01
        });
        if game_logic.patriot_assist_residual_requests() > 0 {
            break;
        }
    }

    assert!(
        game_logic.patriot_assist_residual_requests() > 0,
        "RequestAssistRange residual must issue assist request"
    );
    assert!(
        game_logic.patriot_assist_residual_accepts() >= 1,
        "near assistant must accept assist residual"
    );
    // hq-2qedt.32.5: assist fires count at fire commit, so the same-frame
    // evidence is the AssistedTargetingUpdate TEMP lock on the assist slot.
    assert_eq!(
        game_logic.host_object(assistant_id).map(|a| (a.weapon_lock_type, a.weapon_lock_slot)),
        Some((crate::game_logic::WeaponLockType::LockedTemporarily, 1)),
        "accepted assistant must TEMP-lock AssistingWeaponSlot = SECONDARY"
    );

    // Run the complete owned AI / missile / impact phases, not just the
    // assist queue. Weapon.cpp delivers damage when each projectile detonates.
    for _ in 0..(PATRIOT_ASSISTING_CLIP_SIZE * 30) {
        let effects = crate::game_logic::ScriptPresentationEffects {
            pristine_bones: Some(assets.as_ref()),
            ..Default::default()
        };
        game_logic.tick_logic_frame_with_script_effects(
            LOGIC_FRAME_TIMESTEP, None, None, effects, |_| {}, |_| {},
        );
        observed_assist_flight |= game_logic.combat_system.projectiles_snapshot().iter().any(|p| {
            p.shooter_id == assistant_id && p.target_id == Some(enemy_id)
                && p.missile_motion.is_some()
                && (p.damage - PATRIOT_ASSIST_DAMAGE).abs() < 0.01
        });
    }
    assert!(observed_assist_flight, "assistant must launch its authored assist missile");

    let enemy_hp_after = game_logic
        .host_object(enemy_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let damage_dealt = test_observed_damage_to(
        &game_logic.health_events,
        enemy_id,
        enemy_hp_before,
        enemy_hp_after,
    );
    // At least one assist-scale residual hit; full clip + primary is more.
    assert!(
        damage_dealt >= PATRIOT_ASSIST_DAMAGE,
        "assist residual must contribute damage (dealt={damage_dealt}, before={enemy_hp_before}, after={enemy_hp_after})"
    );
    // Fire-commit honesty (request + accept + fire) is proven via update() in
    // patriot_assist_clip_counts_exactly_four_fire_commits_via_update.
    // BinaryDataStream laser residual honesty (LaserFromAssisted + LaserToTarget).
    assert!(
        game_logic.honesty_patriot_assist_laser_ok(),
        "assist accept must spawn BinaryDataStream laser residual pair"
    );
    assert!(
        game_logic.patriot_assist_laser_from_assisted() >= 1,
        "LaserFromAssisted residual must fire"
    );
    assert!(
        game_logic.patriot_assist_laser_to_target() >= 1,
        "LaserToTarget residual must fire"
    );
    // Far patriot never accepted.
    assert!(
        !game_logic
            .pending_patriot_assists
            .iter()
            .any(|p| p.assistant_id == far_id),
        "far patriot outside RequestAssistRange must not assist"
    );
}

/// C++ Weapon::privateFireWeapon (Weapon.cpp:2476-2478) calls
/// processRequestAssistance before delivering the shot, so a shot that kills
/// the victim still recruits a free assistant.
#[test]
fn patriot_requests_assist_before_fire_even_when_shot_kills() {
    use crate::game_logic::host_base_defense::PATRIOT_REQUEST_ASSIST_RANGE;

    let (mut game_logic, requester, enemy) =
        crate::game_logic::game_logic::accepted_projectile_owner_tests::
            patriot_retail_battery_tests::world();
    game_logic.create_object_for_player("AmericaPowerPlant", 0, Vec3::new(10., 0., 30.))
        .unwrap();
    let assistant = game_logic
        .create_object_for_player(
            "AmericaPatriotBattery",
            0,
            Vec3::new(100., 30., 100. + PATRIOT_REQUEST_ASSIST_RANGE * 0.5),
        )
        .unwrap();
    assert!(!game_logic.host_object(assistant).unwrap().is_disabled());
    game_logic.host_object_mut(requester).unwrap().set_max_shots_to_fire(1);
    game_logic.host_object_mut(enemy).unwrap().health.current = 1.;

    let assets = game_logic.application_assets().unwrap();
    let mut saw_launch = false;
    for _ in 0..120 {
        let effects = crate::game_logic::ScriptPresentationEffects {
            pristine_bones: Some(assets.as_ref()),
            ..Default::default()
        };
        game_logic.tick_logic_frame_with_script_effects(
            LOGIC_FRAME_TIMESTEP, None, None, effects, |_| {}, |_| {},
        );
        if !saw_launch && game_logic.combat_system.projectiles_snapshot().iter().any(|shot| {
            shot.shooter_id == requester && shot.target_id == Some(enemy)
        }) {
            assert!(game_logic.host_object(enemy).unwrap().is_alive());
            assert!(game_logic.patriot_assist_residual_requests() > 0);
            assert!(game_logic.patriot_assist_residual_accepts() >= 1);
            assert_eq!(
                game_logic.host_object(assistant).unwrap().weapon_lock_type,
                crate::game_logic::WeaponLockType::LockedTemporarily,
            );
            saw_launch = true;
        }
        if game_logic.host_object(enemy).is_none_or(|object| !object.is_alive()) {
            break;
        }
    }
    assert!(saw_launch);
    assert!(
        game_logic.host_object(enemy).is_none_or(|object| !object.is_alive()),
        "the admitted shot must reach and kill its target",
    );
}

/// C++ Weapon::fireWeapon only reaches privateFireWeapon (and thus
/// processRequestAssistance, Weapon.cpp:2476-2478) when the weapon is READY_TO_FIRE
/// (DelayBetweenShots / clip reload elapsed). A Patriot still reloading must not
/// request assistance or fire this frame.
#[test]
fn patriot_does_not_request_assist_while_weapon_not_ready() {
    use crate::game_logic::host_base_defense::{
        PATRIOT_PRIMARY_WEAPON, PATRIOT_REQUEST_ASSIST_RANGE, PATRIOT_SECONDARY_WEAPON,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let mut patriot_tpl = crate::game_logic::ThingTemplate::new("USA_Patriot");
    patriot_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::FSBaseDefense)
        .set_health(600.0)
        .set_primary_weapon_name(PATRIOT_PRIMARY_WEAPON)
        .set_secondary_weapon_name(PATRIOT_SECONDARY_WEAPON);
    game_logic
        .templates
        .insert("USA_Patriot".to_string(), patriot_tpl);
    let requester_id = game_logic
        .create_object("USA_Patriot", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("requester");
    let assistant_id = game_logic
        .create_object(
            "USA_Patriot",
            Team::USA,
            Vec3::new(PATRIOT_REQUEST_ASSIST_RANGE * 0.5, 0.0, 0.0),
        )
        .expect("assistant");
    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
        .expect("enemy");
    {
        let e = game_logic.host_object_mut(enemy_id).unwrap();
        e.health.current = 1.0e6;
        e.health.maximum = 1.0e6;
        e.max_health = 1.0e6;
    }
    game_logic.frame = 100;
    let now = game_logic.frame as f32 * LOGIC_FRAME_TIMESTEP;
    // Both Patriots just fired: every slot is mid-reload.
    for id in [requester_id, assistant_id] {
        let o = game_logic.host_object_mut(id).unwrap();
        if let Some(w) = o.weapon.as_mut() {
            w.last_fire_time = now;
        }
        for slot in 0..3u8 {
            if let Some(w) = o.weapon_slot_mut(slot) {
                w.last_fire_time = now;
            }
        }
    }
    let hp_before = game_logic.host_object(enemy_id).unwrap().health.current;
    game_logic.update_combat(&[requester_id, assistant_id, enemy_id], LOGIC_FRAME_TIMESTEP);

    assert_eq!(
        game_logic.patriot_assist_residual_requests(),
        0,
        "not-ready Patriot must not request assistance"
    );
    assert_eq!(
        game_logic.host_object(enemy_id).unwrap().health.current,
        hp_before,
        "not-ready Patriot must not fire"
    );
}

/// C++ AssistedTargetingUpdate::assistAttack (AssistedTargetingUpdate.cpp:74-75)
/// TEMP-locks the assist slot; that AssistWeapon has no RequestAssistRange, so
/// processRequestAssistance (Weapon.cpp:2476-2478) never fans out from an
/// assisting Patriot. A TEMP-locked assistant must not re-request on its ready shot.
#[test]
fn patriot_temp_locked_assistant_does_not_re_request_assist() {
    use crate::game_logic::WeaponLockType;
    use crate::game_logic::host_base_defense::{
        PATRIOT_PRIMARY_WEAPON, PATRIOT_REQUEST_ASSIST_RANGE, PATRIOT_SECONDARY_WEAPON,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let mut patriot_tpl = crate::game_logic::ThingTemplate::new("USA_Patriot");
    patriot_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::FSBaseDefense)
        .set_health(600.0)
        .set_primary_weapon_name(PATRIOT_PRIMARY_WEAPON)
        .set_secondary_weapon_name(PATRIOT_SECONDARY_WEAPON);
    game_logic
        .templates
        .insert("USA_Patriot".to_string(), patriot_tpl);
    let assisting_id = game_logic
        .create_object("USA_Patriot", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("assisting patriot");
    let free_id = game_logic
        .create_object(
            "USA_Patriot",
            Team::USA,
            Vec3::new(PATRIOT_REQUEST_ASSIST_RANGE * 0.5, 0.0, 0.0),
        )
        .expect("free patriot");
    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
        .expect("enemy");
    {
        let e = game_logic.host_object_mut(enemy_id).unwrap();
        e.health.current = 1.0e6;
        e.health.maximum = 1.0e6;
        e.max_health = 1.0e6;
    }
    {
        let o = game_logic.host_object_mut(assisting_id).unwrap();
        if let Some(w) = o.weapon.as_mut() {
            w.last_fire_time = -10.0;
        }
        for slot in 0..3u8 {
            if let Some(w) = o.weapon_slot_mut(slot) {
                w.last_fire_time = -10.0;
            }
        }
        assert!(o.set_weapon_lock(1, WeaponLockType::LockedTemporarily));
    }
    game_logic.frame = 1;
    // Only the TEMP-locked assistant runs base-defense this frame.
    game_logic.update_combat(&[assisting_id, enemy_id], LOGIC_FRAME_TIMESTEP);

    assert_eq!(
        game_logic.patriot_assist_residual_requests(),
        0,
        "TEMP-locked assisting Patriot must not re-request assistance"
    );
    assert!(
        !game_logic
            .pending_patriot_assists
            .iter()
            .any(|p| p.assistant_id == free_id),
        "no new assistant may be recruited by an assisting Patriot"
    );
}

/// C++ AssistedTargetingUpdate::assistAttack (AssistedTargetingUpdate.cpp:74-75)
/// TEMP-locks AssistingWeaponSlot (retail SECONDARY PatriotMissileAssistWeapon)
/// and aiAttackObject(victim, AssistingClipSize=4); the nested attack machine
/// fires exactly four real shots (Object::fireCurrentWeapon --m_maxShotCount)
/// then releases the TEMP lock. Assist fires are counted at fire commit, not
/// per assist command. Driven by full `update()`.
#[test]
fn patriot_assist_clip_counts_exactly_four_fire_commits_via_update() {
    use crate::game_logic::WeaponLockType;
    use crate::game_logic::host_base_defense::{
        PATRIOT_ASSIST_WEAPON, PATRIOT_ASSISTING_CLIP_SIZE, PATRIOT_PRIMARY_WEAPON,
        PATRIOT_SECONDARY_WEAPON,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let mut patriot_tpl = crate::game_logic::ThingTemplate::new("USA_Patriot");
    patriot_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::FSBaseDefense)
        .set_health(600.0)
        .set_primary_weapon_name(PATRIOT_PRIMARY_WEAPON)
        .set_secondary_weapon_name(PATRIOT_ASSIST_WEAPON)
        .set_tertiary_weapon_name(PATRIOT_SECONDARY_WEAPON);
    game_logic
        .templates
        .insert("USA_Patriot".to_string(), patriot_tpl);

    // Requester at origin; enemy 200 west (inside requester PRIMARY 225);
    // assistant 190 east (inside RequestAssistRange 200) -> 390 from enemy:
    // outside its own PRIMARY, inside assist range.
    let _requester_id = game_logic
        .create_object("USA_Patriot", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("requester");
    let assistant_id = game_logic
        .create_object("USA_Patriot", Team::USA, Vec3::new(190.0, 0.0, 0.0))
        .expect("assistant");
    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(-200.0, 0.0, 0.0))
        .expect("enemy");
    {
        let e = game_logic.host_object_mut(enemy_id).unwrap();
        e.health.current = 1.0e6;
        e.health.maximum = 1.0e6;
        e.max_health = 1.0e6;
        e.movement.max_speed = 0.0;
    }

    let mut saw_temp_lock = false;
    let mut released = false;
    for _ in 0..240 {
        game_logic.update();
        let a = game_logic.host_object(assistant_id).unwrap();
        if std::env::var("PROBE_325").is_ok() {
            eprintln!(
                "f={} fires={} lock={:?}/{} max={} tgt={:?}",
                game_logic.frame,
                game_logic.patriot_assist_residual_fires(),
                a.weapon_lock_type,
                a.weapon_lock_slot,
                a.max_shots_to_fire,
                a.target
            );
        }
        if a.weapon_lock_type == WeaponLockType::LockedTemporarily {
            assert_eq!(a.weapon_lock_slot, 1, "assist lock on SECONDARY");
            saw_temp_lock = true;
        } else if saw_temp_lock {
            released = true;
            break;
        }
    }
    assert!(game_logic.patriot_assist_residual_accepts() >= 1);
    assert!(saw_temp_lock, "assist must take TEMP lock on SECONDARY");
    assert!(released, "TEMP lock must release when the assist clip empties");
    assert_eq!(
        game_logic.patriot_assist_residual_fires(),
        PATRIOT_ASSISTING_CLIP_SIZE,
        "exactly AssistingClipSize assist fire commits"
    );
}

/// hq-2qedt.113: C++ makeAssistanceRequest (Weapon.cpp:2865-2893) gates helpers
/// only on self / isEquivalentTo / RequestAssistRange from the requester /
/// AssistedTargetingUpdate present / isFreeToAssist (AssistedTargetingUpdate.cpp:61-70:
/// isAbleToAttack + READY_TO_FIRE). No helper-to-victim weapon-range gate and no
/// "already assisting" gate.
#[test]
fn patriot_assist_request_ignores_helper_victim_range_and_already_assisting() {
    use crate::game_logic::host_base_defense::{
        PATRIOT_PRIMARY_WEAPON, PATRIOT_REQUEST_ASSIST_RANGE, PATRIOT_SECONDARY_WEAPON,
        PendingPatriotAssist,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let mut patriot_tpl = crate::game_logic::ThingTemplate::new("USA_Patriot");
    patriot_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::FSBaseDefense)
        .set_health(600.0)
        .set_primary_weapon_name(PATRIOT_PRIMARY_WEAPON)
        .set_secondary_weapon_name(PATRIOT_SECONDARY_WEAPON);
    game_logic
        .templates
        .insert("USA_Patriot".to_string(), patriot_tpl);
    let requester_id = game_logic
        .create_object("USA_Patriot", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("requester");
    // Helper inside RequestAssistRange of the requester, but victim far beyond
    // the helper's 450 assist-weapon range.
    let far_helper = game_logic
        .create_object(
            "USA_Patriot",
            Team::USA,
            Vec3::new(-PATRIOT_REQUEST_ASSIST_RANGE * 0.5, 0.0, 0.0),
        )
        .expect("far helper");
    // Helper already holding a live pending assist.
    let busy_helper = game_logic
        .create_object("USA_Patriot", Team::USA, Vec3::new(0.0, 0.0, 50.0))
        .expect("busy helper");
    let victim_far = PATRIOT_REQUEST_ASSIST_RANGE * 0.5 + 600.0;
    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(victim_far, 0.0, 0.0))
        .expect("enemy");
    for id in [requester_id, far_helper, busy_helper] {
        let o = game_logic.host_object_mut(id).unwrap();
        if let Some(w) = o.weapon.as_mut() {
            w.last_fire_time = -10.0;
        }
        for slot in 0..3u8 {
            if let Some(w) = o.weapon_slot_mut(slot) {
                w.last_fire_time = -10.0;
            }
        }
    }
    let assist = game_logic
        .host_object(busy_helper)
        .unwrap()
        .assisted_targeting
        .expect("assist module");
    game_logic.pending_patriot_assists.push(PendingPatriotAssist::new(
        busy_helper,
        enemy_id,
        requester_id,
        0,
        "USA_Patriot",
        assist,
        false,
    ));
    game_logic.frame = 1;
    game_logic.process_patriot_assist_request(requester_id, enemy_id, 0);

    let accepted: Vec<_> = game_logic
        .pending_patriot_assists
        .iter()
        .map(|p| p.assistant_id)
        .collect();
    assert!(
        accepted.contains(&far_helper),
        "helper beyond assist-weapon range of victim must still accept: {accepted:?}"
    );
    assert!(
        accepted.iter().filter(|&&a| a == busy_helper).count() == 1,
        "already-assisting but ready helper must still accept (replacing, hq-2qedt.114): {accepted:?}"
    );
}

/// hq-2qedt.114: C++ assistAttack on an already-assisting helper calls
/// aiAttackObject(victim, m_clipSize) again (AssistedTargetingUpdate.cpp:74-75),
/// which re-targets and resets setMaxShotCount to the full clip
/// (AIUpdate.cpp:3417). The helper has one attack state, so a re-ask must
/// replace its pending assist, not stack a duplicate.
#[test]
fn patriot_re_ask_replaces_pending_assist_and_resets_clip() {
    use crate::game_logic::host_base_defense::{
        PATRIOT_PRIMARY_WEAPON, PATRIOT_SECONDARY_WEAPON, PendingPatriotAssist,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let mut patriot_tpl = crate::game_logic::ThingTemplate::new("USA_Patriot");
    patriot_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::FSBaseDefense)
        .set_health(600.0)
        .set_primary_weapon_name(PATRIOT_PRIMARY_WEAPON)
        .set_secondary_weapon_name(PATRIOT_SECONDARY_WEAPON);
    game_logic
        .templates
        .insert("USA_Patriot".to_string(), patriot_tpl);
    let old_requester = game_logic
        .create_object("USA_Patriot", Team::USA, Vec3::new(0.0, 0.0, -50.0))
        .expect("old requester");
    let requester_id = game_logic
        .create_object("USA_Patriot", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("requester");
    let helper = game_logic
        .create_object("USA_Patriot", Team::USA, Vec3::new(0.0, 0.0, 50.0))
        .expect("helper");
    let old_victim = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(200.0, 0.0, -50.0))
        .expect("old victim");
    let new_victim = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(200.0, 0.0, 0.0))
        .expect("new victim");
    for id in [old_requester, requester_id, helper] {
        let o = game_logic.host_object_mut(id).unwrap();
        if let Some(w) = o.weapon.as_mut() {
            w.last_fire_time = -10.0;
        }
        for slot in 0..3u8 {
            if let Some(w) = o.weapon_slot_mut(slot) {
                w.last_fire_time = -10.0;
            }
        }
    }
    let assist = game_logic
        .host_object(helper)
        .unwrap()
        .assisted_targeting
        .expect("assist module");
    // Mid-clip: one shot left on the old victim.
    let mut mid = PendingPatriotAssist::new(helper, old_victim, old_requester, 0, "USA_Patriot", assist, false);
    mid.shots_remaining = 1;
    game_logic.pending_patriot_assists.push(mid);
    game_logic.frame = 1;
    game_logic.process_patriot_assist_request(requester_id, new_victim, 0);

    let mine: Vec<_> = game_logic
        .pending_patriot_assists
        .iter()
        .filter(|p| p.assistant_id == helper)
        .cloned()
        .collect();
    assert_eq!(mine.len(), 1, "re-ask must replace, not stack: {mine:?}");
    assert_eq!(mine[0].victim_id, new_victim);
    assert_eq!(mine[0].requester_id, requester_id);
    assert_eq!(mine[0].shots_remaining, assist.clip_size, "clip reset to full");
}

/// hq-2qedt.270: an accepted assist clip records the assistant's laser family
/// from its authored PRIMARY (Lazr WeaponSet), never from a `Lazr_` prefix.
#[test]
fn patriot_assist_laser_family_keys_on_authored_weapon_not_template_name() {
    use crate::game_logic::host_base_defense::{
        LAZR_PATRIOT_ASSIST_WEAPON, LAZR_PATRIOT_PRIMARY_WEAPON, LAZR_PATRIOT_SECONDARY_WEAPON,
        PATRIOT_ASSIST_WEAPON, PATRIOT_PRIMARY_WEAPON, PATRIOT_REQUEST_ASSIST_RANGE,
        PATRIOT_SECONDARY_WEAPON,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    for (name, primary, assist, air, expected_laser) in [
        ("AmericaPatriotBattery", LAZR_PATRIOT_PRIMARY_WEAPON, LAZR_PATRIOT_ASSIST_WEAPON, LAZR_PATRIOT_SECONDARY_WEAPON, true),
        ("CustomAirDefense", LAZR_PATRIOT_PRIMARY_WEAPON, LAZR_PATRIOT_ASSIST_WEAPON, LAZR_PATRIOT_SECONDARY_WEAPON, true),
        ("Lazr_AmericaPatriotBattery", PATRIOT_PRIMARY_WEAPON, PATRIOT_ASSIST_WEAPON, PATRIOT_SECONDARY_WEAPON, false),
    ] {
        let mut game_logic = GameLogic::new();
        ensure_test_tank_template(&mut game_logic);
        let mut tpl = crate::game_logic::ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .add_kind_of(KindOf::FSBaseDefense)
            .set_health(600.0)
            .set_primary_weapon_name(primary)
            .set_secondary_weapon_name(assist)
            .set_tertiary_weapon_name(air);
        game_logic.templates.insert(name.to_string(), tpl);
        let requester = game_logic.create_object(name, Team::USA, Vec3::ZERO).expect("requester");
        let helper = game_logic
            .create_object(name, Team::USA, Vec3::new(-PATRIOT_REQUEST_ASSIST_RANGE * 0.25, 0.0, 0.0))
            .expect("helper");
        let enemy = game_logic
            .create_object("TestTank", Team::GLA, Vec3::new(200.0, 0.0, 0.0))
            .expect("enemy");
        for id in [requester, helper] {
            let o = game_logic.host_object_mut(id).unwrap();
            // Programmatic templates do not carry an admitted Behavior block.
            // Install the authored module explicitly rather than deriving it from the name.
            o.assisted_targeting = Some(crate::game_logic::host_base_defense::HostAssistedTargetingData::RETAIL_PATRIOT);
            for slot in 0..3u8 {
                if let Some(w) = o.weapon_slot_mut(slot) {
                    w.last_fire_time = -10.0;
                }
            }
            if let Some(w) = o.weapon.as_mut() {
                w.last_fire_time = -10.0;
            }
        }
        game_logic.frame = 1;
        // The assist weapon has zero RequestAssistRange. It must not inherit
        // the request range of the primary simply because that value is zero.
        game_logic.process_patriot_assist_request(requester, enemy, 1);
        assert!(game_logic.pending_patriot_assists.is_empty(), "{name}: assist weapon cannot recruit");
        game_logic.host_object_mut(helper).unwrap().assisted_targeting = None;
        game_logic.process_patriot_assist_request(requester, enemy, 0);
        assert!(game_logic.pending_patriot_assists.is_empty(), "{name}: helper without module cannot assist");
        game_logic.host_object_mut(helper).unwrap().assisted_targeting =
            Some(crate::game_logic::host_base_defense::HostAssistedTargetingData::RETAIL_PATRIOT);
        game_logic.process_patriot_assist_request(requester, enemy, 0);
        let clip = game_logic
            .pending_patriot_assists
            .iter()
            .find(|p| p.assistant_id == helper)
            .unwrap_or_else(|| panic!("{name}: helper must accept the assist request"));
        assert_eq!(clip.assistant_laser, expected_laser, "{name}");
        game_logic.pending_patriot_assists.clear();
        game_logic.process_patriot_assist_request(requester, enemy, 2);
        assert!(
            game_logic.pending_patriot_assists.iter().any(|p| p.assistant_id == helper),
            "{name}: the authored air weapon also requests assistance",
        );
    }
}

/// hq-2qedt.259: the Stinger hive identity is the authored
/// KINDOF_SPAWNS_ARE_THE_WEAPONS bit, never a `StingerSite` basename. A hive
/// without live spawns cannot fire and owns the AP Rockets refresh; a
/// Stinger-named structure without the bit is an ordinary base defense.
#[test]
fn stinger_hive_gates_key_on_spawns_are_the_weapons_not_template_name() {
    use crate::game_logic::host_base_defense::{STINGER_PRIMARY_WEAPON, STINGER_SECONDARY_WEAPON};
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    for (name, spawns_are_weapons) in [("CustomHiveDefense", true), ("GLAStingerSite", false)] {
        let mut game_logic = GameLogic::new();
        ensure_test_tank_template(&mut game_logic);
        let mut tpl = crate::game_logic::ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .add_kind_of(KindOf::FSBaseDefense)
            .set_health(400.0)
            .set_primary_weapon_name(STINGER_PRIMARY_WEAPON)
            .set_secondary_weapon_name(STINGER_SECONDARY_WEAPON);
        if spawns_are_weapons {
            tpl.add_kind_of(KindOf::SpawnsAreTheWeapons);
        }
        game_logic.templates.insert(name.to_string(), tpl);
        let site = game_logic.create_object(name, Team::GLA, Vec3::ZERO).expect("site");
        let _enemy = game_logic
            .create_object("TestTank", Team::USA, Vec3::new(40.0, 0.0, 0.0))
            .expect("enemy");
        {
            let s = game_logic.host_object_mut(site).unwrap();
            for slot in 0..3u8 {
                if let Some(w) = s.weapon_slot_mut(slot) {
                    w.last_fire_time = -10.0;
                }
            }
        }
        game_logic.frame = 1;
        game_logic.try_base_defense_residual_fire(site);
        assert_eq!(
            game_logic.base_defense_residual_fires() == 0,
            spawns_are_weapons,
            "{name}: a hive with no live spawns never fires itself"
        );
        assert_eq!(game_logic.apply_stinger_ap_rockets_upgrade(site), spawns_are_weapons, "{name}");
    }
}

#[test]
fn authored_virtual_mine_places_neutron_emitter_before_charge_is_consumed() {
    use crate::fixture_content::{FixtureObject, FixtureWeapon, admit_fixture_content};

    let mut world = GameLogic::new();
    let mut map = game_engine::map_data::MapData::new();
    map.width = 32;
    map.height = 32;
    map.heightmap = vec![0; 1024];
    map.boundaries = vec![glam::IVec2::new(32, 32)];
    world.world_services.terrain().write().unwrap().try_load_map_geometry(map).unwrap();
    world.initialize_receiving_partition_grid().unwrap();
    admit_fixture_content(
        &mut world,
        &[
            FixtureObject::new("RenamedVirtualPad")
                .kind_of("MINE IMMOBILE")
                .health(100.0)
                .field("Geometry = Cylinder")
                .field("GeometryMajorRadius = 30.0")
                .module(
                    "Behavior = MinefieldBehavior Mine\nDetonationWeapon = PadPulseWeapon\n\
                     NumVirtualMines = 2\nRegenerates = Yes\nEnd",
                ),
            FixtureObject::new("RenamedGroundPulse")
                .kind_of("UNATTACKABLE")
                .health(1000.0)
                .module(
                    "Behavior = HeightDieUpdate Height\nTargetHeight = 100.0\n\
                     OnlyWhenMovingDown = No\nSnapToGroundOnDeath = Yes\nEnd",
                )
                .module(
                    "Behavior = NeutronBlastBehavior Pulse\nBlastRadius = 20.0\n\
                     AffectAirborne = No\nAffectAllies = No\nEnd",
                ),
            FixtureObject::new("PadPulseTarget")
                .kind_of("VEHICLE SELECTABLE")
                .health(400.0),
        ],
        &[FixtureWeapon::new("PadPulseWeapon")
            .damage(1.0)
            .radius(10.0)
            .damage_type("EXPLOSION")
            .field("ProjectileObject = RenamedGroundPulse")
            .field("WeaponSpeed = 0.01")
            .field("RadiusDamageAffects = ENEMIES NEUTRALS")],
    );
    use crate::game_logic::{Player, PlayerSideRole};
    let mut neutral = Player::new(0, Team::Neutral, "Neutral", false);
    neutral.map_side.role = PlayerSideRole::Neutral;
    neutral.map_side.map_player_name.clear();
    world.add_player(neutral);
    for (id, team) in [(1, Team::China), (2, Team::USA)] {
        let mut player = Player::new(id, team, "mine fixture", id == 1);
        player.alliance_team = id as i32;
        world.add_player(player);
    }
    let pad_pos = Vec3::new(100.0, 0.0, 100.0);
    let target_pos = pad_pos + Vec3::new(3.0, 0.0, 0.0);
    let pad = world.create_object_for_player("RenamedVirtualPad", 1, pad_pos).unwrap();
    let target = world.create_object_for_player("PadPulseTarget", 2, target_pos).unwrap();
    world.update_mines_and_demo_traps().unwrap();

    let mine = world.host_object(pad).unwrap().mine_data.as_ref().unwrap();
    assert_eq!(mine.virtual_mines_remaining, 1);
    let emitters: Vec<_> = world.host_objects().registered_values()
        .filter(|object| object.template_name == "RenamedGroundPulse")
        .map(|object| (object.id, object.get_position(), object.producer_id))
        .collect();
    assert_eq!(emitters.len(), 1, "authored temporary weapon must create its no-flight projectile");
    assert_eq!(emitters[0].1, target_pos);
    assert_eq!(emitters[0].2, Some(pad));
    assert_eq!(world.host_object(target).unwrap().health.current, 400.0);
    assert!(!world.host_object(target).unwrap().is_unmanned());

    world.update_owned_partition_with_effects(&mut Default::default()).unwrap();
    for _ in 0..3 {
        let _ = world.update_with_dt(1.0 / 30.0);
    }
    assert_eq!(world.host_object(target).unwrap().health.current, 400.0);
    assert!(world.host_object(target).unwrap().is_unmanned());
    assert_eq!(world.host_object(target).unwrap().team, Team::Neutral);
}

#[test]
fn authored_minefield_pad_fires_detonation_weapon_and_spends_one_charge() {
    use crate::fixture_content::{FixtureObject, FixtureWeapon, admit_fixture_content};

    let mut world = GameLogic::new();
    let mut map = game_engine::map_data::MapData::new();
    map.width = 32;
    map.height = 32;
    map.heightmap = vec![0; 1024];
    map.boundaries = vec![glam::IVec2::new(32, 32)];
    world.world_services.terrain().write().unwrap().try_load_map_geometry(map).unwrap();
    world.initialize_receiving_partition_grid().unwrap();
    admit_fixture_content(
        &mut world,
        &[
            FixtureObject::new("RenamedMinefieldPad")
                .kind_of("MINE IMMOBILE")
                .health(100.0)
                .field("Geometry = Cylinder")
                .field("GeometryMajorRadius = 10.0")
                .module(
                    "Behavior = MinefieldBehavior Mine\nDetonationWeapon = PadCustomShot\n\
                     NumVirtualMines = 2\nRegenerates = Yes\nEnd",
                ),
            FixtureObject::new("PadBlastVictim")
                .kind_of("VEHICLE SELECTABLE")
                .health(400.0),
            FixtureObject::new("PadOutOfRange")
                .kind_of("VEHICLE SELECTABLE")
                .health(400.0),
        ],
        &[FixtureWeapon::new("PadCustomShot")
            .damage(31.0)
            .radius(5.0)
            .damage_type("LAND_MINE")
            .field("SecondaryDamage = 11")
            .field("SecondaryDamageRadius = 20")
            .field("ProjectileObject = NONE")
            .field("WeaponSpeed = 999999")
            .field("RadiusDamageAffects = ENEMIES")],
    );
    use crate::game_logic::{Player, PlayerSideRole};
    let mut neutral = Player::new(0, Team::Neutral, "Neutral", false);
    neutral.map_side.role = PlayerSideRole::Neutral;
    neutral.map_side.map_player_name.clear();
    world.add_player(neutral);
    for (id, team) in [(1, Team::China), (2, Team::USA)] {
        let mut player = Player::new(id, team, "mine fixture", id == 1);
        player.alliance_team = id as i32;
        world.add_player(player);
    }
    let pad_pos = Vec3::new(100.0, 0.0, 100.0);
    let pad = world.create_object_for_player("RenamedMinefieldPad", 1, pad_pos).unwrap();
    let victim = world
        .create_object_for_player("PadBlastVictim", 2, pad_pos + Vec3::new(3.0, 0.0, 0.0))
        .unwrap();
    // Beyond contact reach (mine trigger 30 + victim circle) and beyond the
    // authored secondary ring: must stay untouched by the real blast.
    let out_of_range = world
        .create_object_for_player("PadOutOfRange", 2, pad_pos + Vec3::new(60.0, 0.0, 0.0))
        .unwrap();
    let data = world.host_object(pad).unwrap().mine_data.as_ref().unwrap();
    assert_eq!(data.num_virtual_mines, 2);
    assert!(data.regenerates);

    world.update_mines_and_demo_traps().unwrap();

    let victim = world.host_object(victim).unwrap();
    assert_eq!(
        victim.health.current,
        369.0,
        "authored DetonationWeapon delivers exactly its PrimaryDamage"
    );
    assert_eq!(victim.last_damage_source, Some(pad));
    assert_eq!(world.host_object(out_of_range).unwrap().health.current, 400.0);
    let pad = world.host_object(pad).unwrap();
    let mine = pad.mine_data.as_ref().unwrap();
    assert_eq!(mine.virtual_mines_remaining, 1);
    // C++ detonateOnce drains the pad to fraction * max health (>= MIN_HEALTH).
    assert_eq!(pad.health.current, 50.0);
}

#[test]
fn authored_minefield_without_detonation_weapon_spends_charge_silently() {
    use crate::fixture_content::{FixtureObject, FixtureWeapon, admit_fixture_content};

    let mut world = GameLogic::new();
    let mut map = game_engine::map_data::MapData::new();
    map.width = 32;
    map.height = 32;
    map.heightmap = vec![0; 1024];
    map.boundaries = vec![glam::IVec2::new(32, 32)];
    world.world_services.terrain().write().unwrap().try_load_map_geometry(map).unwrap();
    world.initialize_receiving_partition_grid().unwrap();
    admit_fixture_content(
        &mut world,
        &[
            // Retail name on purpose: the bound MinefieldBehavior, not the
            // name-inferred standardmine profile, owns data.
            FixtureObject::new("ChinaStandardMine")
                .kind_of("MINE IMMOBILE")
                .health(100.0)
                .field("Geometry = Cylinder")
                .field("GeometryMajorRadius = 10.0")
                .module("Behavior = MinefieldBehavior Mine\nEnd"),
            FixtureObject::new("SilentMineVisitor")
                .kind_of("VEHICLE SELECTABLE")
                .health(400.0),
        ],
        &[] as &[FixtureWeapon],
    );
    use crate::game_logic::{Player, PlayerSideRole};
    let mut neutral = Player::new(0, Team::Neutral, "Neutral", false);
    neutral.map_side.role = PlayerSideRole::Neutral;
    neutral.map_side.map_player_name.clear();
    world.add_player(neutral);
    for (id, team) in [(1, Team::China), (2, Team::USA)] {
        let mut player = Player::new(id, team, "mine fixture", id == 1);
        player.alliance_team = id as i32;
        world.add_player(player);
    }
    let pad_pos = Vec3::new(100.0, 0.0, 100.0);
    let pad = world.create_object_for_player("ChinaStandardMine", 1, pad_pos).unwrap();
    let visitor = world
        .create_object_for_player("SilentMineVisitor", 2, pad_pos + Vec3::new(3.0, 0.0, 0.0))
        .unwrap();
    {
        let pad_obj = world.host_object(pad).unwrap();
        let mine = pad_obj.mine_data.as_ref().unwrap();
        // MinefieldBehaviorModuleData ctor defaults: 1 charge, no regen/scoot;
        // the name-inferred standardmine profile must not leak through.
        assert_eq!(mine.num_virtual_mines, 1);
        assert_eq!(mine.virtual_mines_remaining, 1);
        assert!(!mine.regenerates);
        assert_eq!(mine.scoot_from_starting_point_time, 0);
        assert_eq!(mine.detonation_damage, 0.0);
        assert_eq!(mine.detonation_radius, 0.0);
        assert_eq!(pad_obj.health.current, 100.0);
    }

    world.update_mines_and_demo_traps().unwrap();

    assert_eq!(
        world.host_object(visitor).unwrap().health.current,
        400.0,
        "a missing DetonationWeapon spends the charge with no blast"
    );
    let pad_obj = world.host_object(pad).unwrap();
    assert_eq!(
        pad_obj.mine_data.as_ref().unwrap().virtual_mines_remaining,
        0
    );
    assert!(
        pad_obj.status.destroyed,
        "last charge consumed: C++ requests destroyObject on the pad"
    );
}

#[test]
fn authored_minefield_placement_preserves_admitted_pad_data() {
    use crate::fixture_content::{FixtureObject, FixtureWeapon, admit_fixture_content};

    let mut world = GameLogic::new();
    admit_fixture_content(
        &mut world,
        &[
            FixtureObject::new("ChinaStandardMine")
                .kind_of("MINE IMMOBILE")
                .health(100.0)
                .field("Geometry = Cylinder")
                .field("GeometryMajorRadius = 10.0")
                .module(
                    "Behavior = MinefieldBehavior Mine\nNumVirtualMines = 2\n\
                     Regenerates = No\nRepeatDetonateMoveThresh = 9\nEnd",
                ),
            // Authored lookalike with no mine behavior: C++ placeMineAt still
            // creates it, but adds no mine module, flags, or weapon strip.
            FixtureObject::new("ChinaClusterMine")
                .kind_of("IMMOBILE")
                .health(100.0),
            FixtureObject::new("MinePlanter")
                .kind_of("VEHICLE SELECTABLE")
                .health(400.0),
        ],
        &[] as &[FixtureWeapon],
    );
    use crate::game_logic::{Player, PlayerSideRole};
    let mut neutral = Player::new(0, Team::Neutral, "Neutral", false);
    neutral.map_side.role = PlayerSideRole::Neutral;
    neutral.map_side.map_player_name.clear();
    world.add_player(neutral);
    let mut player = Player::new(1, Team::China, "mine fixture", true);
    player.alliance_team = 1;
    world.add_player(player);

    let planter_pos = Vec3::new(100.0, 0.0, 100.0);
    let producer = world
        .create_object_for_player("MinePlanter", 1, planter_pos)
        .unwrap();
    let pad_pos = planter_pos + Vec3::new(50.0, 0.0, 0.0);
    let pad = world
        .place_land_mine_named("ChinaStandardMine", Team::China, pad_pos, Some(producer))
        .unwrap();
    let lookalike = world
        .place_land_mine_named("ChinaClusterMine", Team::China, pad_pos, Some(producer))
        .unwrap();

    let pad_obj = world.host_object(pad).unwrap();
    let mine = pad_obj.mine_data.as_ref().unwrap();
    // Bound admission (MinefieldBehaviorModuleData + authored fields), not the
    // standardmine name profile's 8/regen/scoot inference.
    assert_eq!(mine.num_virtual_mines, 2);
    assert_eq!(mine.virtual_mines_remaining, 2);
    assert!(!mine.regenerates);
    assert_eq!(mine.repeat_detonate_move_thresh, 9.0);
    assert_eq!(mine.scoot_from_starting_point_time, 0);
    assert!(!mine.is_scooting());
    assert_eq!(mine.producer_id, Some(producer));
    assert_eq!(pad_obj.producer_id, Some(producer));
    assert_eq!(pad_obj.get_position(), pad_pos);

    let lookalike = world.host_object(lookalike).unwrap();
    assert!(lookalike.mine_data.is_none());
    assert!(!lookalike.is_disarmable_mine());
}
