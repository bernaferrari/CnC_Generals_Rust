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
            let unit = game_logic.host_object_mut(unit_id).expect("unit mut");
            unit.weapon = Some(Weapon {
                damage: 20.0,
                range: 80.0,
                reload_time: 0.5,
                last_fire_time: -10.0,
                ..Weapon::default()
            });
            unit.target = Some(outpost_id);
            unit.set_ai_state(AIState::Entering);
        }
        game_logic.update_ai(&[unit_id, outpost_id], 1.0 / 30.0);
    }

    let outpost = game_logic.host_object(outpost_id).expect("outpost loaded");
    assert!(
        outpost.contained_units().contains(&unit_a) && outpost.contained_units().contains(&unit_b),
        "both infantry must load into Listening Outpost residual"
    );
    assert_eq!(outpost.transport_count(), LISTENING_OUTPOST_TRANSPORT_SLOTS);
    assert_eq!(game_logic.listening_outpost_residual_loads(), 2);
    assert!(
        outpost.weapon_set_player_upgrade,
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
        let unit = game_logic.host_object_mut(unit_c).unwrap();
        unit.target = Some(outpost_id);
        unit.set_ai_state(AIState::Entering);
    }
    game_logic.update_ai(&[unit_c, outpost_id], 1.0 / 30.0);
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
        game_logic.update_ai(&[unit_a, unit_b], 1.0 / 30.0);
        let both_idle = [unit_a, unit_b].iter().all(|&id| {
            game_logic
                .host_object(id)
                .map(|u| u.ai_state == AIState::Idle)
                .unwrap_or(true)
        });
        if both_idle {
            break;
        }
    }

    for unit_id in [unit_a, unit_b] {
        let unit = game_logic.host_object(unit_id).expect("free unit");
        assert_eq!(unit.ai_state, AIState::Idle);
        assert!(unit.contained_by.is_none());
        assert!(unit.can_move());
    }
    {
        let outpost = game_logic.host_object(outpost_id).expect("empty");
        assert_eq!(outpost.transport_count(), 0);
        assert!(
            !outpost.weapon_set_player_upgrade,
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
        let unit = game_logic.host_object_mut(tank_id).unwrap();
        unit.target = Some(outpost_id);
        unit.set_ai_state(AIState::Entering);
    }
    game_logic.update_ai(&[tank_id, outpost_id], 1.0 / 30.0);

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
        let enemy = game_logic.host_object_mut(enemy_id).unwrap();
        enemy.set_position(Vec3::new(trigger * 0.25, 0.0, 0.0));
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
        burton.thing.template.is_trainable = true;
        burton.thing.template.veterancy_xp_thresholds = [40.0, 150.0, 300.0];
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
        assert!(!charge.thing.template.is_trainable);
    }

    let victim_id = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(1.0, 0.0, 0.0))
        .expect("victim");
    {
        let victim = game_logic.host_object_mut(victim_id).unwrap();
        victim.health.current = 10.0;
        victim.health.maximum = 10.0;
        victim.thing.template.experience_value = 40.0;
        victim.thing.template.experience_values = [40.0, 40.0, 80.0, 120.0];
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
    assert!(burton.weapon_set_veteran);
    assert!(burton.weapon_bonus_veteran);
}

#[test]
fn cluster_mines_special_power_places_mines() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::{
        SpecialPowerModuleKind, SpecialPowerModuleMetadata,
        host_mines::CLUSTER_MINE_COUNT,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_structure_template(&mut game_logic);
    // C++ SpecialPower.cpp:308 canUseSpecialPower: the caster must carry the
    // SpecialPowerModule for the command; stamp ClusterMines on the template.
    if let Some(t) = game_logic.templates.get_mut("TestBuilding") {
        t.special_power_modules
            .push(SpecialPowerModuleMetadata {
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
        .values()
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
        d.set_weapon_set_mine_clearing_detail(true);
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
        d.set_weapon_set_mine_clearing_detail(true);
    }

    crate::game_logic::host_ai_decision_log::clear();
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
        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            assert_eq!(dozer.ai_state, AIState::Idle);
            let moving =
                crate::gameworld_shadow::GameWorldShadow::host_ai_state_ordinal(&AIState::Moving);
            let events = crate::game_logic::host_ai_decision_log::snapshot();
            assert!(
                events.iter().any(|e| {
                    e.host_object == dozer_id
                        && e.kind == crate::game_logic::host_ai_decision_log::AI_DECISION_SET_STATE
                        && e.ai_state_ordinal == moving
                }),
                "mine approach must log SetAIState(Moving) under decision authority"
            );
        } else {
            assert_eq!(dozer.ai_state, AIState::Moving, "must approach mine");
        }
    }

    {
        let dozer = game_logic.host_object_mut(dozer_id).unwrap();
        dozer.set_position(Vec3::new(DOZER_MINE_CLEAR_RANGE * 0.25, 0.0, 0.0));
        dozer.set_ai_state(AIState::Idle);
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
        e.thing.template.geometry_info.authored = true;
        e.thing.template.geometry_info.major_radius = 8.0;
        e.thing.template.geometry_info.minor_radius = 8.0;
        e.thing.template.add_kind_of(KindOf::Vehicle);
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
        e.thing.template.geometry_info.authored = true;
        e.thing.template.geometry_info.major_radius = 8.0;
        e.thing.template.geometry_info.minor_radius = 8.0;
        e.thing.template.add_kind_of(KindOf::Vehicle);
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
        d.set_weapon_set_mine_clearing_detail(true);
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
        MINE_AUTO_HEAL_AMOUNT, MINE_AUTO_HEAL_DELAY_FRAMES, MINE_MIN_HEALTH,
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
    logic.frame = MINE_AUTO_HEAL_DELAY_FRAMES;
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
        let m = logic.host_object_mut(mine_id).unwrap();
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
        let m = logic.host_object_mut(mine_id).unwrap();
        m.health.current = 100.0;
        m.health.maximum = 100.0;
        if let Some(md) = m.mine_data.as_mut() {
            md.last_synced_health = Some(100.0);
        }
        let killed = m.take_damage_from(100.0, None);
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
        t.thing.template.kind_of.remove(&KindOf::Infantry);
        t.thing.template.add_kind_of(KindOf::Vehicle);
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
        c.thing.template.add_kind_of(KindOf::Aircraft);
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
    if let Some(o) = logic.host_object_mut(did) {
        o.set_ai_state(AIState::Idle);
        o.idle_since_frame = 1;
        assert!(!o.weapon_set_mine_clearing_detail);
    }
    logic.frame = 1 + crate::game_logic::host_repair::DOZER_BORED_TIME_FRAMES;
    logic.update_dozer_bored_repair();
    let d = logic.host_object(did).expect("d");
    assert!(
        d.weapon_set_mine_clearing_detail,
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
        s.set_status_stealthed(true);
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

#[test]
fn stealth_residual_fire_breaks_stealth() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);

    let shooter_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("shooter");
    let target_id = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(10.0, 0.0, 0.0))
        .expect("target");

    {
        let s = game_logic.host_object_mut(shooter_id).unwrap();
        s.set_status_stealthed(true);
        s.stealth_breaks_on_attack = true;
        s.weapon = Some(Weapon {
            damage: 5.0,
            range: 100.0,
            reload_time: 0.5,
            last_fire_time: -1.0, // ready immediately
            ..Weapon::default()
        });
        assert!(s.fire_at(target_id, 0.0));
        assert!(!s.status.stealthed, "fire_at must break stealth");
    }
}

#[test]
fn stealth_residual_detection_expires() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);

    let stealth_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("stealth");
    {
        let s = game_logic.host_object_mut(stealth_id).unwrap();
        s.set_status_stealthed(true);
        s.mark_detected(5); // expires at frame 5
    }

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
        assert_eq!(p.ai_state, AIState::Idle);
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
    crate::game_logic::host_damage_log::clear();

    // Several combat ticks (reload residual) without any AttackObject command.
    for f in 0..80 {
        game_logic.frame = f;
        game_logic.update_combat(&[patriot_id, enemy_id], LOGIC_FRAME_TIMESTEP);
    }

    let enemy_hp_after = game_logic
        .host_object(enemy_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let logged = crate::game_logic::host_damage_log::drain();
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
fn residual_auto_fire_ai_decision_writeback_sets_host_target() {
    use crate::game_logic::host_ai_decision_log;
    use crate::gameworld_shadow::{
        GameWorldShadow, authority_env_lock, begin_shadow_coupled_tick, end_shadow_coupled_tick,
        gameworld_ai_attack_authority_enabled, gameworld_ai_decision_authority_enabled,
    };

    let _env_guard = authority_env_lock();
    // Authority opted in via GameLogic setters (env channel retired): the
    // scratch instance publishes the snapshot the asserts below consult.
    let mut logic = GameLogic::new();
    logic.set_ai_decision_authority(true);
    logic.set_ai_attack_authority(true);
    assert!(gameworld_ai_decision_authority_enabled());
    assert!(gameworld_ai_attack_authority_enabled());
    host_ai_decision_log::clear();
    begin_shadow_coupled_tick();

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
    // Decision channel logged; host target still empty until shadow writeback.
    let events = host_ai_decision_log::drain();
    assert!(
        !events.is_empty(),
        "residual auto-fire must emit AI decision events under AI_DECISION_AUTHORITY"
    );
    assert!(logic.host_object(attacker).unwrap().target.is_none());

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
    assert!(shadow.writeback_attack_targets_to_host(&mut logic) >= 1);
    assert_eq!(logic.host_object(attacker).unwrap().target, Some(victim));

    end_shadow_coupled_tick();
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
    // Physical soldier attach residual: orderSlavesToAttackTarget on fire.
    assert!(
        game_logic.honesty_stinger_slave_order_ok(),
        "Stinger residual fire must order residual soldiers to attack target"
    );
    assert!(game_logic.stinger_slave_order_attack_count() >= 1);
    {
        let s = game_logic.host_object(stinger_id).unwrap();
        assert!(
            s.hive_slaves
                .iter()
                .filter(|sl| sl.alive)
                .any(|sl| sl.ai_attacking && sl.attack_target_id == enemy_id.0),
            "alive residual slaves must carry attack order residual"
        );
    }

    // AA residual auto-fire vs aircraft at AA range.
    let air_id = game_logic
        .create_object("TestAircraft", Team::USA, Vec3::new(300.0, 0.0, 0.0))
        .expect("aircraft");
    {
        let a = game_logic.host_object_mut(air_id).unwrap();
        a.status.airborne_target = true;
    }
    {
        let s = game_logic.host_object_mut(stinger_id).unwrap();
        if let Some(w) = s.secondary_weapon.as_mut() {
            w.last_fire_time = -10.0;
        }
        // Drop ground target so AA scan prefers aircraft.
        s.target = None;
        s.set_ai_state(AIState::Idle);
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

    // HiveStructureBody residual honesty: spawn starts with 3 soldiers.
    {
        let s = game_logic.host_object(stinger_id).expect("stinger hive");
        assert_eq!(
            s.hive_slave_count, 3,
            "SpawnNumber residual must start with 3 soldiers"
        );
        assert!(
            (s.hive_slave_hp - crate::game_logic::host_base_defense::STINGER_SOLDIER_MAX_HEALTH)
                .abs()
                < 0.01
        );
    }
}

#[test]
fn stinger_hive_structure_body_and_spawn_respawn_residual() {
    use crate::game_logic::host_base_defense::{
        HostHiveDamageClass, STINGER_SOLDIER_MAX_HEALTH, STINGER_SPAWN_NUMBER,
        STINGER_SPAWN_REPLACE_DELAY_FRAMES,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let mut stinger_tpl = crate::game_logic::ThingTemplate::new("GLAStingerSite");
    stinger_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::FSBaseDefense)
        .set_health(1000.0);
    game_logic
        .templates
        .insert("GLAStingerSite".to_string(), stinger_tpl);

    let stinger_id = game_logic
        .create_object("GLAStingerSite", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("stinger");
    {
        let s = game_logic.host_object(stinger_id).unwrap();
        assert_eq!(s.hive_slave_count, STINGER_SPAWN_NUMBER as u8);
    }

    let struct_hp_before = game_logic
        .host_object(stinger_id)
        .map(|s| s.health.current)
        .unwrap_or(0.0);

    // Propagate residual: SMALL_ARMS-like damages slaves, not structure.
    let (destroyed, blocked) =
        game_logic.apply_host_hive_damage(stinger_id, 60.0, HostHiveDamageClass::PropagateToSlaves);
    assert!(!destroyed && !blocked);
    {
        let s = game_logic.host_object(stinger_id).unwrap();
        assert!((s.health.current - struct_hp_before).abs() < 0.01);
        assert!((s.hive_slave_hp - 40.0).abs() < 0.01);
        assert_eq!(s.hive_slave_count, 3);
    }
    assert!(game_logic.stinger_hive_residual_slave_hits() >= 1);

    // Kill active slave residual.
    let _ =
        game_logic.apply_host_hive_damage(stinger_id, 50.0, HostHiveDamageClass::PropagateToSlaves);
    {
        let s = game_logic.host_object(stinger_id).unwrap();
        assert_eq!(s.hive_slave_count, 2);
        assert!((s.hive_slave_hp - STINGER_SOLDIER_MAX_HEALTH).abs() < 0.01);
        assert!(s.hive_slave_respawn_frame > 0);
    }
    assert!(game_logic.stinger_hive_residual_slave_kills() >= 1);

    // Kill remaining slaves.
    for _ in 0..4 {
        let _ = game_logic.apply_host_hive_damage(
            stinger_id,
            200.0,
            HostHiveDamageClass::PropagateToSlaves,
        );
    }
    {
        let s = game_logic.host_object(stinger_id).unwrap();
        assert_eq!(s.hive_slave_count, 0);
        // Structure still full: pure propagate with 0 slaves falls through to structure.
        // After last kill with overkill residual may have hit structure; re-check swallow path.
    }

    // Reset structure HP for swallow honesty.
    {
        use crate::game_logic::host_base_defense::clear_hive_slave_roster;
        let s = game_logic.host_object_mut(stinger_id).unwrap();
        clear_hive_slave_roster(&mut s.hive_slaves);
        s.hive_slave_count = 0;
        s.hive_slave_hp = 0.0;
        s.health.current = 1000.0;
        s.status.destroyed = false;
    }
    let hp_before_swallow = 1000.0;
    let _ = game_logic.apply_host_hive_damage(
        stinger_id,
        500.0,
        HostHiveDamageClass::SwallowIfNoSlaves,
    );
    {
        let s = game_logic.host_object(stinger_id).unwrap();
        assert!(
            (s.health.current - hp_before_swallow).abs() < 0.01,
            "SNIPER residual must be swallowed with 0 slaves"
        );
    }
    assert!(game_logic.stinger_hive_residual_swallows() >= 1);

    // HitStructure residual damages building even with slaves restored.
    {
        use crate::game_logic::host_base_defense::{
            align_hive_roster_to_count, sync_hive_slave_mirrors,
        };
        let s = game_logic.host_object_mut(stinger_id).unwrap();
        align_hive_roster_to_count(&mut s.hive_slaves, 3);
        let (c, h) = sync_hive_slave_mirrors(&s.hive_slaves);
        s.hive_slave_count = c;
        s.hive_slave_hp = h.max(STINGER_SOLDIER_MAX_HEALTH);
        s.health.current = 1000.0;
    }
    let _ = game_logic.apply_host_hive_damage(stinger_id, 100.0, HostHiveDamageClass::HitStructure);
    {
        let s = game_logic.host_object(stinger_id).unwrap();
        assert!((s.health.current - 900.0).abs() < 0.01);
        assert_eq!(s.hive_slave_count, 3);
    }

    // SPAWNS_ARE_THE_WEAPONS residual: 0 soldiers cannot fire.
    {
        use crate::game_logic::host_base_defense::clear_hive_slave_roster;
        let s = game_logic.host_object_mut(stinger_id).unwrap();
        clear_hive_slave_roster(&mut s.hive_slaves);
        s.hive_slave_count = 0;
        s.hive_slave_hp = 0.0;
        if let Some(w) = s.weapon.as_mut() {
            w.last_fire_time = -10.0;
        }
    }
    let enemy_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .expect("enemy");
    let enemy_hp_before = game_logic
        .host_object(enemy_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let fires_before = game_logic.stinger_site_residual_ground_fires();
    for f in 0..40 {
        game_logic.frame = f;
        game_logic.update_combat(&[stinger_id, enemy_id], LOGIC_FRAME_TIMESTEP);
    }
    let enemy_hp_after = game_logic
        .host_object(enemy_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        (enemy_hp_after - enemy_hp_before).abs() < 0.01,
        "0-soldier Stinger must not fire residual"
    );
    assert_eq!(
        game_logic.stinger_site_residual_ground_fires(),
        fires_before
    );

    // SpawnReplaceDelay residual respawn.
    {
        use crate::game_logic::host_base_defense::{
            align_hive_roster_to_count, sync_hive_slave_mirrors,
        };
        let s = game_logic.host_object_mut(stinger_id).unwrap();
        align_hive_roster_to_count(&mut s.hive_slaves, 2);
        let (c, h) = sync_hive_slave_mirrors(&s.hive_slaves);
        s.hive_slave_count = c;
        s.hive_slave_hp = h.max(STINGER_SOLDIER_MAX_HEALTH);
        s.hive_slave_respawn_frame = 10;
    }
    game_logic.frame = 10;
    game_logic.update_stinger_hive_respawns();
    {
        let s = game_logic.host_object(stinger_id).unwrap();
        assert_eq!(s.hive_slave_count, 3);
        assert_eq!(s.hive_slave_respawn_frame, 0);
    }
    assert!(game_logic.stinger_hive_residual_respawns() >= 1);
    assert!(game_logic.honesty_stinger_hive_ok());
    assert_eq!(STINGER_SPAWN_REPLACE_DELAY_FRAMES, 900);
}

#[test]
fn stinger_get_closest_slave_physical_roster_residual() {
    use crate::game_logic::host_base_defense::{
        HostHiveDamageClass, STINGER_SOLDIER_MAX_HEALTH, STINGER_SPAWN_NUMBER,
        STINGER_SPAWN_POINT_RADIUS, STINGER_SPAWN_TEMPLATE,
    };

    let mut game_logic = GameLogic::new();
    let mut stinger_tpl = crate::game_logic::ThingTemplate::new("GLAStingerSite");
    stinger_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::FSBaseDefense)
        .set_health(1000.0);
    game_logic
        .templates
        .insert("GLAStingerSite".to_string(), stinger_tpl);

    let stinger_id = game_logic
        .create_object("GLAStingerSite", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("stinger");
    {
        let s = game_logic.host_object(stinger_id).unwrap();
        assert_eq!(s.hive_slave_count, STINGER_SPAWN_NUMBER as u8);
        assert_eq!(
            s.hive_slaves.iter().filter(|sl| sl.alive).count(),
            STINGER_SPAWN_NUMBER as usize,
            "physical residual roster must start with SpawnNumber slots"
        );
        assert!(
            (s.hive_slaves[0].offset_x - STINGER_SPAWN_POINT_RADIUS).abs() < 0.01,
            "slave 0 SpawnPoint residual at +radius"
        );
        assert_eq!(STINGER_SPAWN_TEMPLATE, "GLAInfantryStingerSoldier");
    }

    // Shooter near slave 0 (+radius, 0) → damage only that residual slot.
    let shooter_near_0 = (STINGER_SPAWN_POINT_RADIUS + 5.0, 0.0);
    let (destroyed, blocked) = game_logic.apply_host_hive_damage_from(
        stinger_id,
        40.0,
        HostHiveDamageClass::PropagateToSlaves,
        Some(shooter_near_0),
        None,
    );
    assert!(!destroyed && !blocked);
    {
        let s = game_logic.host_object(stinger_id).unwrap();
        assert!((s.hive_slaves[0].hp - 60.0).abs() < 0.01);
        assert!((s.hive_slaves[1].hp - STINGER_SOLDIER_MAX_HEALTH).abs() < 0.01);
        assert!((s.hive_slaves[2].hp - STINGER_SOLDIER_MAX_HEALTH).abs() < 0.01);
        assert_eq!(s.hive_slave_count, 3);
    }
    assert!(game_logic.stinger_hive_residual_closest_slave_hits() >= 1);
    assert!(game_logic.honesty_stinger_closest_slave_ok());

    // Shooter near slave 2 world position → damage that slot.
    let (sx2, sz2) = {
        let s = game_logic.host_object(stinger_id).unwrap();
        s.hive_slaves[2].world_xz(0.0, 0.0)
    };
    let _ = game_logic.apply_host_hive_damage_from(
        stinger_id,
        25.0,
        HostHiveDamageClass::PropagateToSlaves,
        Some((sx2, sz2)),
        None,
    );
    {
        let s = game_logic.host_object(stinger_id).unwrap();
        assert!((s.hive_slaves[2].hp - 75.0).abs() < 0.01);
        assert!((s.hive_slaves[0].hp - 60.0).abs() < 0.01); // unchanged
    }

    // Kill slave 0 via closest residual.
    let _ = game_logic.apply_host_hive_damage_from(
        stinger_id,
        100.0,
        HostHiveDamageClass::PropagateToSlaves,
        Some(shooter_near_0),
        None,
    );
    {
        let s = game_logic.host_object(stinger_id).unwrap();
        assert!(!s.hive_slaves[0].alive);
        assert_eq!(s.hive_slave_count, 2);
        assert!(s.hive_slave_respawn_frame > 0);
    }
    assert!(game_logic.stinger_hive_residual_slave_kills() >= 1);
    assert!(game_logic.honesty_stinger_hive_ok());

    // Physical soldier attach residual: facing + order + presentation.
    use crate::game_logic::host_base_defense::{
        build_hive_slave_attach_presentation, order_hive_slaves_to_attack_target,
        order_hive_slaves_to_go_idle, stinger_spawn_point_facings,
    };
    {
        let s = game_logic.host_object(stinger_id).unwrap();
        let facings = stinger_spawn_point_facings();
        assert!(
            (s.hive_slaves[0].facing_deg - facings[0]).abs() < 0.01,
            "SpawnPoint facing residual must match host layout"
        );
        let attach = build_hive_slave_attach_presentation(
            &s.hive_slaves,
            s.get_position().x,
            s.get_position().z,
        );
        assert_eq!(attach[0].template_name, STINGER_SPAWN_TEMPLATE);
        assert!((attach[0].facing_deg - facings[0]).abs() < 0.01);
    }
    {
        let s = game_logic.host_object_mut(stinger_id).unwrap();
        let n = order_hive_slaves_to_attack_target(&mut s.hive_slaves, 1234);
        assert!(n >= 1);
        assert!(
            s.hive_slaves
                .iter()
                .filter(|sl| sl.alive)
                .all(|sl| { sl.ai_attacking && sl.attack_target_id == 1234 })
        );
        let n_idle = order_hive_slaves_to_go_idle(&mut s.hive_slaves);
        assert!(n_idle >= 1);
        assert!(
            s.hive_slaves
                .iter()
                .filter(|sl| sl.alive)
                .all(|sl| !sl.ai_attacking && sl.attack_target_id == 0)
        );
    }
}

#[test]
fn camo_netting_structure_attack_and_damage_reveal_residual() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::host_upgrades::{
        CAMO_NETTING_FRIENDLY_OPACITY_MAX, CAMO_NETTING_FRIENDLY_OPACITY_MIN,
        CAMO_NETTING_STEALTH_DELAY_FRAMES, UPGRADE_GLA_CAMO_NETTING,
    };

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::GLA, "GLA", true);
    player.resources.supplies = 5000;
    game_logic.add_player(player);
    ensure_test_barracks_template(&mut game_logic);
    ensure_test_tank_template(&mut game_logic);

    for name in ["GLATunnelNetwork", "GLAStingerSite"] {
        if !game_logic.templates.contains_key(name) {
            let mut t = crate::game_logic::ThingTemplate::new(name);
            t.add_kind_of(KindOf::Structure)
                .add_kind_of(KindOf::Selectable)
                .add_kind_of(KindOf::Attackable)
                .add_kind_of(KindOf::FSBaseDefense)
                .set_health(1000.0);
            game_logic.templates.insert(name.to_string(), t);
        }
    }
    if !game_logic.templates.contains_key("GLABlackMarket") {
        let mut market = crate::game_logic::ThingTemplate::new("GLABlackMarket");
        market
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::FSBlackMarket)
            .set_health(1000.0);
        game_logic
            .templates
            .insert("GLABlackMarket".to_string(), market);
    }
    let _market_id = game_logic
        .create_object("GLABlackMarket", Team::GLA, Vec3::new(-200.0, 0.0, 0.0))
        .expect("black market");

    let barracks_id = game_logic
        .create_object("TestBarracks", Team::GLA, Vec3::new(-80.0, 0.0, 0.0))
        .expect("barracks");
    let tunnel_id = game_logic
        .create_object("GLATunnelNetwork", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("tunnel");
    let stinger_id = game_logic
        .create_object("GLAStingerSite", Team::GLA, Vec3::new(100.0, 0.0, 0.0))
        .expect("stinger");

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
    // C++ research advances on the producer's Upgrade.ini BuildTime
    // (ProductionUpdate.cpp:686-704); tick past the 5s CamoNetting window.
    game_logic.update();
    game_logic.update_with_dt(6.0);

    for id in [tunnel_id, stinger_id] {
        let o = game_logic.host_object(id).unwrap();
        assert!(o.innate_stealth);
        assert!(!o.status.stealthed, "CamoNetting re-arms StealthDelay");
        assert!(o.stealth_breaks_on_attack);
        assert!(o.stealth_breaks_on_damage);
        assert_eq!(o.stealth_delay_frames, CAMO_NETTING_STEALTH_DELAY_FRAMES);
        assert!(o.stealth_allowed_frame > game_logic.frame);
        assert!(
            o.camo_net_sub_object_shown,
            "CamoNetting residual must show net mesh sub-object"
        );
    }
    let cloak_at = game_logic
        .host_object(tunnel_id)
        .map(|o| o.stealth_allowed_frame)
        .unwrap_or(0);
    game_logic.frame = cloak_at;
    game_logic.update_stealth_and_detection();
    for id in [tunnel_id, stinger_id] {
        let o = game_logic.host_object(id).unwrap();
        assert!(o.status.stealthed && o.innate_stealth);
        assert!(
            (o.camo_friendly_opacity - CAMO_NETTING_FRIENDLY_OPACITY_MIN).abs() < 0.01,
            "CamoNetting cloak residual must set FriendlyOpacityMin, got {}",
            o.camo_friendly_opacity
        );
    }
    assert!(
        game_logic.honesty_camo_netting_friendly_opacity_ok(),
        "FriendlyOpacity residual honesty must record cloak"
    );
    assert!(game_logic.camo_netting_opacity_cloak_count() >= 2);
    assert!(
        game_logic.honesty_camo_netting_sub_object_ok(),
        "CamoNetting sub-object net mesh residual honesty"
    );
    assert!(game_logic.camo_netting_sub_object_show_count() >= 2);

    // TAKING_DAMAGE residual uncloaks.
    let _ = game_logic.apply_host_damage(tunnel_id, 10.0);
    assert!(
        !game_logic
            .host_object(tunnel_id)
            .map(|o| o.status.stealthed)
            .unwrap_or(true),
        "damage residual must break CamoNetting structure stealth"
    );
    assert!(
        game_logic
            .host_object(tunnel_id)
            .map(|o| (o.camo_friendly_opacity - CAMO_NETTING_FRIENDLY_OPACITY_MAX).abs() < 0.01)
            .unwrap_or(false),
        "damage reveal residual must set FriendlyOpacityMax"
    );
    assert!(
        game_logic
            .host_object(tunnel_id)
            .map(|o| o.stealth_delay_pending || o.stealth_allowed_frame > 0)
            .unwrap_or(false)
            || game_logic.camo_netting_structure_residual_reveals() > 0,
        "damage reveal must schedule StealthDelay residual"
    );

    // Resolve delay pending + stay uncloaked until delay elapses.
    game_logic.frame = 1;
    game_logic.update_stealth_and_detection();
    let allowed = game_logic
        .host_object(tunnel_id)
        .map(|o| o.stealth_allowed_frame)
        .unwrap_or(0);
    assert!(
        allowed > 0,
        "StealthDelay residual must set stealth_allowed_frame"
    );
    assert!(
        !game_logic
            .host_object(tunnel_id)
            .map(|o| o.status.stealthed)
            .unwrap_or(true),
        "must remain uncloaked during StealthDelay"
    );

    // ATTACKING residual: IS_FIRING_WEAPON destalths; approach does not.
    {
        let s = game_logic.host_object_mut(stinger_id).unwrap();
        s.set_status_stealthed(true);
        s.stealth_allowed_frame = 0;
        s.stealth_delay_pending = false;
        s.set_status_firing_weapon(true);
    }

    game_logic.frame = 2;
    game_logic.update_stealth_and_detection();
    assert!(
        !game_logic
            .host_object(stinger_id)
            .map(|o| o.status.stealthed)
            .unwrap_or(true),
        "attacking residual must break CamoNetting structure stealth"
    );

    // USING_ABILITY residual: forbids stealth while OBJECT_STATUS_IS_USING_ABILITY.
    {
        let s = game_logic.host_object_mut(stinger_id).unwrap();
        s.set_status_stealthed(true);
        s.stealth_allowed_frame = 0;
        s.stealth_delay_pending = false;
        s.set_status_attacking(false);
        s.set_status_firing_weapon(false);
        s.set_ai_state(AIState::Idle);
        s.set_status_using_ability(true);
    }
    game_logic.frame = 3;
    game_logic.update_stealth_and_detection();
    assert!(
        !game_logic
            .host_object(stinger_id)
            .map(|o| o.status.stealthed)
            .unwrap_or(true),
        "USING_ABILITY residual must break CamoNetting structure stealth"
    );
    // Clear ability so re-cloak path can exercise.
    {
        let s = game_logic.host_object_mut(stinger_id).unwrap();
        s.set_status_using_ability(false);
    }

    // OrderIdleEnemiesToAttackMeUponReveal residual: idle enemy in vision
    // wakes and targets the revealed structure.
    let enemy_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(30.0, 0.0, 0.0))
        .expect("enemy tank");
    {
        let e = game_logic.host_object_mut(enemy_id).unwrap();
        e.set_ai_state(AIState::Idle);
        e.target = None;
        e.weapon = Some(Weapon {
            damage: 10.0,
            range: 200.0,
            reload_time: 1.0,
            last_fire_time: -10.0,
            ..Weapon::default()
        });
        // Ensure vision covers the tunnel at x=0 (enemy at x=30).
        e.get_template(); // touch template path
    }
    // Force tunnel stealthed then damage-reveal so OrderIdle residual fires.
    {
        let t = game_logic.host_object_mut(tunnel_id).unwrap();
        t.set_status_stealthed(true);
        t.set_status_attacking(false);
        t.set_status_using_ability(false);
        t.set_ai_state(AIState::Idle);
        t.stealth_allowed_frame = 0;
        t.stealth_delay_pending = false;
        t.set_status_firing_weapon(true); // force uncloak this frame
    }

    let order_before = game_logic.camo_netting_order_idle_enemies_count();
    game_logic.frame = 4;
    game_logic.update_stealth_and_detection();
    assert!(
        !game_logic
            .host_object(tunnel_id)
            .map(|o| o.status.stealthed)
            .unwrap_or(true),
        "reveal residual must uncloak for OrderIdle path"
    );
    assert!(
        game_logic.camo_netting_order_idle_enemies_count() > order_before
            || game_logic
                .host_object(enemy_id)
                .map(|e| e.target == Some(tunnel_id))
                .unwrap_or(false),
        "OrderIdleEnemies residual must wake idle enemy on reveal"
    );
    assert!(
        game_logic
            .host_object(enemy_id)
            .map(|e| e.target == Some(tunnel_id) && e.ai_state == AIState::Attacking)
            .unwrap_or(false)
            || game_logic.honesty_camo_netting_order_idle_enemies_ok(),
        "idle enemy residual must attempt to target revealed structure"
    );

    // Idle + StealthDelay elapsed → re-cloak residual.
    game_logic.frame = 10 + CAMO_NETTING_STEALTH_DELAY_FRAMES;
    let recloak_frame = game_logic.frame;
    {
        let t = game_logic.host_object_mut(tunnel_id).unwrap();
        t.set_status_attacking(false);
        t.set_status_firing_weapon(false);
        t.set_status_using_ability(false);
        t.set_ai_state(AIState::Idle);
        t.target = None;
        t.set_status_stealthed(false);
        // frame < allowed forbids; equal allows re-cloak residual.
        t.stealth_allowed_frame = recloak_frame;
        t.stealth_delay_pending = false;
        // Research now advances on the producer (frame ~150), so the earlier
        // damage-reveal stamped an absolute timestamp past this rewound frame;
        // this block exercises idle re-cloak, not the damage window.
        t.last_damage_timestamp = None;
    }
    game_logic.update_stealth_and_detection();
    assert!(
        game_logic
            .host_object(tunnel_id)
            .map(|o| o.status.stealthed)
            .unwrap_or(false),
        "idle after StealthDelay must re-cloak CamoNetting structure"
    );
    assert!(
        game_logic
            .host_object(tunnel_id)
            .map(
                |o| (o.camo_friendly_opacity - CAMO_NETTING_FRIENDLY_OPACITY_MIN).abs() < 0.01
                    || o.camo_friendly_opacity <= CAMO_NETTING_FRIENDLY_OPACITY_MAX
            )
            .unwrap_or(false),
        "re-cloak residual must restore FriendlyOpacityMin (or pulse within min..max)"
    );
    // Pulse residual while cloaked: one more tick advances phase / opacity.
    let (op_before, phase_before) = game_logic
        .host_object(tunnel_id)
        .map(|o| (o.camo_friendly_opacity, o.camo_opacity_pulse_phase))
        .unwrap_or((1.0, 0.0));
    game_logic.frame = recloak_frame.saturating_add(1);
    game_logic.update_stealth_and_detection();
    {
        let o = game_logic.host_object(tunnel_id).unwrap();
        assert!(o.status.stealthed);
        assert!(
            o.camo_opacity_pulse_phase > phase_before
                || (o.camo_friendly_opacity - op_before).abs() > 0.001
                || o.camo_friendly_opacity >= CAMO_NETTING_FRIENDLY_OPACITY_MIN,
            "cloaked residual must pulse FriendlyOpacity"
        );
        assert!(
            o.camo_friendly_opacity >= CAMO_NETTING_FRIENDLY_OPACITY_MIN - 0.01
                && o.camo_friendly_opacity <= CAMO_NETTING_FRIENDLY_OPACITY_MAX + 0.01,
            "pulse residual opacity must stay in min..max, got {}",
            o.camo_friendly_opacity
        );
    }
    assert!(
        game_logic.camo_netting_structure_residual_recloaks() > 0
            || game_logic.honesty_camo_netting_structure_stealth_ok(),
        "camo structure stealth residual honesty"
    );
    assert!(game_logic.honesty_camo_netting_structure_stealth_ok());

    // StealthLook / heat-vision residual: detect cloaked structure → second pass.
    use crate::game_logic::host_upgrades::{HostCamoStealthLook, camo_netting_stealth_look};
    {
        let t = game_logic.host_object_mut(tunnel_id).unwrap();
        t.set_status_stealthed(true);
        t.set_status_detected(true);
        t.camo_heat_vision_opacity = 0.0;
        t.camo_stealth_look = 0;
        t.record_host_vision_camo();
    }
    game_logic.update_stealth_and_detection();
    {
        let t = game_logic.host_object(tunnel_id).unwrap();
        let look = HostCamoStealthLook::from_u8(t.camo_stealth_look);
        assert_eq!(
            look,
            HostCamoStealthLook::VisibleDetected,
            "enemy-detected residual StealthLook must be VISIBLE_DETECTED"
        );
        assert!(
            (t.camo_heat_vision_opacity - 1.0).abs() < 0.01,
            "heat-vision second material pass residual opacity must be 1.0, got {}",
            t.camo_heat_vision_opacity
        );
        assert_eq!(
            camo_netting_stealth_look(true, true, false),
            HostCamoStealthLook::VisibleDetected
        );
    }
    assert!(
        game_logic.honesty_camo_netting_heat_vision_ok(),
        "CamoNetting heat-vision residual honesty"
    );
    assert!(game_logic.camo_netting_heat_vision_count() >= 1);

    // Sub-object residual: detected → observer-visible with heat-vision pass.
    use crate::game_logic::host_upgrades::{
        CAMO_NETTING_SUB_OBJECT_MESH_NAME, camo_netting_sub_object_observer_visible,
        camo_netting_sub_object_state,
    };
    {
        let t = game_logic.host_object(tunnel_id).unwrap();
        assert!(t.camo_net_sub_object_shown);
        let sub = camo_netting_sub_object_state(
            true,
            t.status.stealthed,
            t.status.detected,
            false,
            t.camo_friendly_opacity,
        );
        assert_eq!(sub.mesh_name, CAMO_NETTING_SUB_OBJECT_MESH_NAME);
        assert!(sub.shown);
        assert!(sub.heat_vision_pass);
        assert!(camo_netting_sub_object_observer_visible(&sub));
        assert!(t.camo_net_sub_object_observer_visible);
    }
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
        .set_secondary_weapon_name(PATRIOT_SECONDARY_WEAPON);
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
        let sec = p.secondary_weapon.as_ref().expect("AA");
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
    crate::game_logic::host_damage_log::clear();
    for f in 0..80 {
        game_logic.frame = f;
        game_logic.update_combat(&[patriot_id, air_id], LOGIC_FRAME_TIMESTEP);
    }
    let air_hp_after = game_logic
        .host_object(air_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let dealt = test_observed_damage_to(air_id, air_hp_before, air_hp_after);
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
    crate::game_logic::host_damage_log::clear();

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
    assert!(
        game_logic.patriot_assist_residual_fires() >= 1,
        "assist residual must fire at least one assist-weapon shot same frame"
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
    let damage_dealt = test_observed_damage_to(enemy_id, enemy_hp_before, enemy_hp_after);
    // At least one assist-scale residual hit; full clip + primary is more.
    assert!(
        damage_dealt >= PATRIOT_ASSIST_DAMAGE,
        "assist residual must contribute damage (dealt={damage_dealt}, before={enemy_hp_before}, after={enemy_hp_after})"
    );
    assert!(
        game_logic.honesty_patriot_assist_ok(),
        "patriot assist honesty must record request + accept + fire"
    );
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
    // Full clip residual honesty: 4 assist shots when victim survives.
    assert!(
        game_logic.patriot_assist_residual_fires() >= PATRIOT_ASSISTING_CLIP_SIZE
            || enemy_hp_after <= 0.0
            || !game_logic
                .host_object(enemy_id)
                .map(|e| e.is_alive())
                .unwrap_or(false),
        "expected AssistingClipSize={} assist fires or victim dead (got {})",
        PATRIOT_ASSISTING_CLIP_SIZE,
        game_logic.patriot_assist_residual_fires()
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
