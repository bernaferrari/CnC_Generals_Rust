//! Behavior suite extracted from `ocl_and_bombs`.
use super::*;

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
            unit.weapon = Some(Weapon {
                damage: 20.0,
                range: 80.0,
                reload_time: 0.5,
                last_fire_time: -10.0,
                ..Weapon::default()
            });
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

    game_logic.update_mines_and_demo_traps();
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
    game_logic.update_mines_and_demo_traps();

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

    game_logic.update_mines_and_demo_traps();
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
        gamelogic::common::Relationship::Enemies
    );

    game_logic.update_mines_and_demo_traps();
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

    game_logic.update_mines_and_demo_traps();
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
    game_logic.update_mines_and_demo_traps();
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

    game_logic.update_mines_and_demo_traps();
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
    game_logic.update_mines_and_demo_traps();
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
fn timed_demo_charge_residual_detonates_after_delay() {
    let mut game_logic = GameLogic::new();
    ensure_test_structure_template(&mut game_logic);

    // Short delay for test observability (not full 10s retail lifetime).
    let charge_id = game_logic
        .place_timed_demo_charge(Team::USA, Vec3::new(0.0, 0.0, 0.0), None, None, Some(3))
        .expect("charge");
    assert_eq!(game_logic.mine_residual_places(), 1);

    let building_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(5.0, 0.0, 0.0))
        .expect("building");
    let health_before = game_logic.host_object(building_id).unwrap().health.current;

    // Before deadline: no detonation.
    game_logic.frame = 1;
    game_logic.update_mines_and_demo_traps();
    assert_eq!(game_logic.mine_residual_timed_detonations(), 0);
    assert!(game_logic.host_object(charge_id).unwrap().is_alive());

    // At deadline: detonate.
    game_logic.frame = 3;
    game_logic.update_mines_and_demo_traps();
    assert_eq!(game_logic.mine_residual_timed_detonations(), 1);
    assert!(
        game_logic.honesty_timed_demo_charge_ok(),
        "timed charge honesty"
    );

    let building = game_logic.host_object(building_id).unwrap();
    assert!(
        building.health.current < health_before || building.status.destroyed,
        "timed charge must damage nearby structure"
    );
}

#[test]
fn timed_c4_kill_sinks_xp_to_planter() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    // C++ Object::scoreTheKill awards XP only between ENEMIES; fresh test
    // slots default Neutral (Player::new alliance_team=-1) — declare hostility.
    game_logic
        .get_player_mut(0)
        .unwrap()
        .set_map_relationship(2, gamelogic::common::Relationship::Enemies);
    game_logic
        .get_player_mut(2)
        .unwrap()
        .set_map_relationship(0, gamelogic::common::Relationship::Enemies);
    let burton_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(-40.0, 0.0, 0.0))
        .expect("burton");
    {
        let burton = game_logic.host_object_mut(burton_id).unwrap();
        burton.template_mut().is_trainable = true;
        burton.template_mut().veterancy_xp_thresholds = [40.0, 150.0, 300.0];
    }

    let charge_id = game_logic
        .place_timed_demo_charge(
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
            Some(burton_id),
            None,
            Some(1),
        )
        .expect("charge");
    {
        let charge = game_logic.host_object(charge_id).unwrap();
        assert_eq!(
            charge.experience_sink,
            Some(burton_id),
            "C4 must sink XP to the planter"
        );
        assert!(!charge.thing().template.is_trainable);
    }

    let victim_id = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(1.0, 0.0, 0.0))
        .expect("victim");
    {
        let victim = game_logic.host_object_mut(victim_id).unwrap();
        victim.health.current = 10.0;
        victim.health.maximum = 10.0;
        victim.template_mut().experience_value = 40.0;
        victim.template_mut().experience_values = [40.0, 40.0, 80.0, 120.0];
    }

    game_logic.frame = 1;
    game_logic.update_mines_and_demo_traps();

    let burton = game_logic.host_object(burton_id).unwrap();
    assert!(
        burton.experience.current + f32::EPSILON >= 40.0,
        "C4 kill XP must sink to planter, got {}",
        burton.experience.current
    );
    assert_eq!(burton.experience.level, VeterancyLevel::Veteran);
    assert!(burton.test_weapon_condition(0));
    assert!(burton.weapon_bonus_veteran);
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
    assert!(game_logic.manual_detonate_mine(trap_id));
    assert_eq!(game_logic.mine_residual_manual_detonations(), 1);
    let enemy = game_logic.host_object(enemy_id).unwrap();
    assert!(
        enemy.health.current < enemy.max_health || enemy.status.destroyed,
        "manual detonate must damage nearby enemy"
    );
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
    game_logic.update_mines_and_demo_traps();
    assert_eq!(
        game_logic.mine_residual_clears(),
        0,
        "PreAttackDelay must hold the first frame"
    );

    game_logic.frame = 1 + DOZER_MINE_CLEAR_PRE_ATTACK_FRAMES;
    game_logic.update_mines_and_demo_traps();

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

    game_logic.update_mines_and_demo_traps();
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
    game_logic.update_mines_and_demo_traps();
    assert_eq!(
        game_logic.mine_residual_clears(),
        0,
        "PreAttackDelay must hold after entering range"
    );

    game_logic.frame = 10 + DOZER_MINE_CLEAR_PRE_ATTACK_FRAMES;
    game_logic.update_mines_and_demo_traps();

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

    game_logic.update_mines_and_demo_traps();
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

    game_logic.update_mines_and_demo_traps();
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

    game_logic.update_mines_and_demo_traps();
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

    game_logic.update_mines_and_demo_traps();
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
        game_logic.clear_mine_internal(trap_id, dozer_id),
        "DISARM must defuse demo traps"
    );
    assert_eq!(game_logic.mine_residual_clears(), 1);
    assert_eq!(
        game_logic.mine_residual_proximity_detonations(),
        0,
        "defuse must not splash"
    );

    game_logic.frame = 1;
    game_logic.update_mines_and_demo_traps();
    game_logic.frame = 1 + DOZER_MINE_CLEAR_PRE_ATTACK_FRAMES;
    game_logic.update_mines_and_demo_traps();

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
    assert!(logic.clear_mine_internal(mine_id, ObjectId(999)));
    let pad = logic.host_object(mine_id).expect("regen pad kept");
    assert!(pad.is_alive(), "China pad must survive disarm");
    assert!(!pad.status.destroyed);
    assert!((pad.health.current - MINE_MIN_HEALTH).abs() < 1e-3);
    assert_eq!(pad.mine_data.as_ref().unwrap().virtual_mines_remaining, 0);

    logic.frame = 0;
    logic.update_mines_and_demo_traps();
    logic.frame = MINE_AUTO_HEAL_START_DELAY_FRAMES;
    logic.update_mines_and_demo_traps();
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
    logic.update_mines_and_demo_traps();
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
    logic.update_mines_and_demo_traps();
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
    logic.update_mines_and_demo_traps();
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
    logic.update_mines_and_demo_traps();
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
    logic.update_mines_and_demo_traps();
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
    logic.update_mines_and_demo_traps();
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
    logic.update_mines_and_demo_traps();
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
    logic
        .players
        .insert(0, crate::game_logic::Player::new(0, Team::USA, "USA", true));
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
        a.weapon = Some(Weapon {
            damage: 10.0,
            range: 150.0,
            ..Weapon::default()
        });
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

    game_logic.update_stealth_and_detection();
    game_logic.update_stealth_and_detection();

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
        s.weapon = Some(Weapon {
            damage: 5.0,
            range: 100.0,
            reload_time: 0.5,
            last_fire_time: -1.0, // ready immediately
            ..Weapon::default()
        });
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
    game_logic.update_stealth_and_detection();
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
    game_logic.update_stealth_and_detection();
    assert!(
        game_logic.host_object(stealth_id).unwrap().status.detected,
        "must remain detected before expiry frame"
    );

    game_logic.frame = 5;
    game_logic.update_stealth_and_detection();
    let stealth = game_logic.host_object(stealth_id).unwrap();
    assert!(
        !stealth.status.detected && stealth.status.stealthed,
        "detected clears at expiry; stealthed may remain"
    );
    assert!(stealth.is_effectively_stealthed());
}

#[test]
fn base_defense_residual_patriot_auto_fires_without_attack_object() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let _ = super::weapon_bootstrap::ensure_host_weapon_store();

    // GameLogic::new does not load faction buildings; register residual template.
    let mut patriot_tpl = crate::game_logic::ThingTemplate::new("USA_Patriot");
    patriot_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::FSBaseDefense)
        .set_health(600.0)
        .set_primary_weapon_name(super::weapon_bootstrap::PATRIOT_PRIMARY_WEAPON)
        .set_secondary_weapon_name(super::weapon_bootstrap::PATRIOT_SECONDARY_WEAPON);
    game_logic
        .templates
        .insert("USA_Patriot".to_string(), patriot_tpl);

    let patriot_id = game_logic
        .create_object("USA_Patriot", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("patriot");
    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
        .expect("enemy");

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

    // Several combat ticks (reload residual) without any AttackObject command.
    for f in 0..80 {
        game_logic.frame = f;
        game_logic.update_combat(&[patriot_id, enemy_id], LOGIC_FRAME_TIMESTEP);
    }

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
fn residual_auto_fire_commits_canonical_target_before_observation() {
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
        o.weapon = Some(crate::game_logic::Weapon {
            damage: 10.0,
            range: 200.0,
            min_range: 0.0,
            reload_time: 0.1,
            last_fire_time: -10.0,
            last_fire_frame: 0,
            ammo: None,
            clip_size: 0,
            clip_reload_time: 0.0,
            can_target_air: false,
            can_target_ground: true,
            projectile_speed: 0.0,
            pre_attack_delay: 0.0,
            splash_radius: 0.0,
            suspend_fx_frame: 0,
            reloading_clip: false,
            last_bonus_rof: 0.0,
        });
    }
    assert!(logic.host_object(attacker).unwrap().target.is_none());
    let weapon = logic.host_object(attacker).and_then(|o| o.weapon.clone());
    let pos = logic.host_object(attacker).unwrap().get_position();
    let _ = logic.residual_auto_fire_apply_damage(attacker, victim, 10.0, pos, weapon.as_ref(), 0);
    assert_eq!(logic.host_object(attacker).unwrap().target, Some(victim));
    let mut shadow = crate::gameworld_shadow::GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let entity = shadow.entity_for_host(attacker).unwrap();
    assert_eq!(
        shadow.world().entity(entity).unwrap().attack_target,
        shadow.entity_for_host(victim)
    );
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
    ensure_test_tank_template(&mut game_logic);

    let mut stinger_tpl = crate::game_logic::ThingTemplate::new("GLA_StingerSite");
    stinger_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::FSBaseDefense)
        .set_health(400.0)
        .set_primary_weapon_name(STINGER_PRIMARY_WEAPON)
        .set_secondary_weapon_name(STINGER_SECONDARY_WEAPON);
    game_logic
        .templates
        .insert("GLA_StingerSite".to_string(), stinger_tpl);

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
        game_logic.update_combat(&[stinger_id, enemy_id], LOGIC_FRAME_TIMESTEP);
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
        game_logic.update_combat(&[stinger_id, air_id], LOGIC_FRAME_TIMESTEP);
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
    game_logic.update_stealth_and_detection();
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
    let _ = game_logic.apply_host_damage(tunnel_id, 10.0);
    game_logic.update_stealth_and_detection();
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
    game_logic.update_stealth_and_detection();
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
    game_logic.update_stealth_and_detection();
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
    game_logic.update_stealth_and_detection();
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
        e.weapon = Some(Weapon {
            damage: 10.0,
            range: 200.0,
            reload_time: 1.0,
            last_fire_time: -10.0,
            ..Weapon::default()
        });
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
        PATRIOT_AIR_DAMAGE, PATRIOT_AIR_RANGE, PATRIOT_GROUND_DAMAGE, PATRIOT_PRIMARY_WEAPON,
        PATRIOT_SECONDARY_WEAPON, is_patriot_battery_structure,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();

    let mut patriot_tpl = crate::game_logic::ThingTemplate::new("USA_Patriot");
    patriot_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::FSBaseDefense)
        .set_health(600.0)
        .set_primary_weapon_name(PATRIOT_PRIMARY_WEAPON)
        .set_secondary_weapon_name(crate::game_logic::host_base_defense::PATRIOT_ASSIST_WEAPON)
        .set_tertiary_weapon_name(PATRIOT_SECONDARY_WEAPON);
    game_logic
        .templates
        .insert("USA_Patriot".to_string(), patriot_tpl);

    let mut aircraft_tpl = crate::game_logic::ThingTemplate::new("TestAircraft");
    aircraft_tpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::Selectable)
        .set_health(200.0);
    game_logic
        .templates
        .insert("TestAircraft".to_string(), aircraft_tpl);

    let patriot_id = game_logic
        .create_object("USA_Patriot", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("patriot");
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

    let air_id = game_logic
        .create_object("TestAircraft", Team::GLA, Vec3::new(250.0, 0.0, 0.0))
        .expect("aircraft");
    {
        let a = game_logic.host_object_mut(air_id).unwrap();
        a.status.airborne_target = true;
    }
    {
        let p = game_logic.host_object_mut(patriot_id).unwrap();
        if let Some(w) = p.secondary_weapon.as_mut() {
            w.last_fire_time = -10.0;
        }
    }

    let air_hp_before = game_logic
        .host_object(air_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    game_logic.health_events.clear_damage();
    for f in 0..80 {
        game_logic.frame = f;
        game_logic.update_combat(&[patriot_id, air_id], LOGIC_FRAME_TIMESTEP);
    }
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
        PATRIOT_ASSIST_DAMAGE, PATRIOT_ASSISTING_CLIP_SIZE, PATRIOT_PRIMARY_WEAPON,
        PATRIOT_REQUEST_ASSIST_RANGE, PATRIOT_SECONDARY_WEAPON,
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

    // Requester at origin; assistant within RequestAssistRange 200.
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
    // Far patriot outside RequestAssistRange — must not assist.
    let far_id = game_logic
        .create_object(
            "USA_Patriot",
            Team::USA,
            Vec3::new(PATRIOT_REQUEST_ASSIST_RANGE + 50.0, 0.0, 0.0),
        )
        .expect("far");
    // Enemy in primary range of requester (50 < 225).
    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
        .expect("enemy");
    // Buff enemy HP so assist clip can land multiple shots.
    {
        let e = game_logic.host_object_mut(enemy_id).unwrap();
        e.health.current = 500.0;
        e.health.maximum = 500.0;
        e.max_health = 500.0;
    }
    {
        let p = game_logic.host_object_mut(requester_id).unwrap();
        if let Some(w) = p.weapon.as_mut() {
            w.last_fire_time = -10.0;
        }
    }
    {
        let p = game_logic.host_object_mut(assistant_id).unwrap();
        if let Some(w) = p.weapon.as_mut() {
            w.last_fire_time = -10.0;
        }
    }
    {
        let p = game_logic.host_object_mut(far_id).unwrap();
        if let Some(w) = p.weapon.as_mut() {
            w.last_fire_time = -10.0;
        }
    }

    let enemy_hp_before = game_logic
        .host_object(enemy_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    game_logic.health_events.clear_damage();

    // One combat tick: requester primary fire → assist request → first assist shot.
    game_logic.frame = 1;
    game_logic.update_combat(
        &[requester_id, assistant_id, far_id, enemy_id],
        LOGIC_FRAME_TIMESTEP,
    );

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

    // Advance AssistingClipSize shots (DelayBetweenShots 8 frames).
    for f in 2..(2 + PATRIOT_ASSISTING_CLIP_SIZE * 10) {
        game_logic.frame = f;
        // Keep weapons cold so only assist residual continues (no re-request spam).
        if let Some(p) = game_logic.host_object_mut(requester_id) {
            if let Some(w) = p.weapon.as_mut() {
                w.last_fire_time = f as f32 * LOGIC_FRAME_TIMESTEP;
            }
        }
        game_logic.update_pending_patriot_assists();
    }

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
        // One primary hit kills.
        let e = game_logic.host_object_mut(enemy_id).unwrap();
        e.health.current = 1.0;
        e.health.maximum = 1.0;
        e.max_health = 1.0;
    }
    for id in [requester_id, assistant_id] {
        if let Some(w) = game_logic.host_object_mut(id).unwrap().weapon.as_mut() {
            w.last_fire_time = -10.0;
        }
    }
    game_logic.frame = 1;
    game_logic.update_combat(&[requester_id, assistant_id, enemy_id], LOGIC_FRAME_TIMESTEP);

    assert!(
        !game_logic
            .host_object(enemy_id)
            .map(|e| e.is_alive())
            .unwrap_or(false),
        "requester shot must kill the victim"
    );
    assert!(
        game_logic.patriot_assist_residual_requests() > 0,
        "assist must be requested before the killing shot"
    );
    assert!(
        game_logic.patriot_assist_residual_accepts() >= 1,
        "free assistant must accept the pre-fire request"
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
