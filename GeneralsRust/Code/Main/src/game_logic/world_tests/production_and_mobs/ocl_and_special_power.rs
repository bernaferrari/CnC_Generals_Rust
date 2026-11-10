//! Behavior suite extracted from `production_and_mobs`.
use super::*;

#[test]
fn combat_chinook_residual_load_two_unload_both_free() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    // C++ TransportContain.cpp:105 "only OUR OWN units can be transported":
    // the exact-controller gate needs a registered controlling player.
    game_logic.players.insert(
        0,
        crate::game_logic::Player::new(0, crate::game_logic::Team::USA, "USA", true),
    );
    ensure_test_infantry_template(&mut game_logic);
    let chinook_id = create_test_combat_chinook(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));

    let unit_a = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .expect("unit a");
    let unit_b = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(2.0, 0.0, 0.0))
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
            unit.target = Some(chinook_id);
            unit.set_ai_state(
                AIState::Entering,
                ai_entry_frame,
                &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
            );
        }
        game_logic
            .update_ai(&[unit_id, chinook_id], 1.0 / 30.0)
            .expect("receiving AI operation succeeds");
    }

    let chinook = game_logic.host_object(chinook_id).expect("chinook loaded");
    assert!(
        chinook.contained_units().contains(&unit_a) && chinook.contained_units().contains(&unit_b),
        "both infantry must be loaded into Combat Chinook residual"
    );
    assert_eq!(chinook.transport_count(), 2);
    assert_eq!(game_logic.combat_chinook_residual_loads(), 2);
    assert!(chinook.test_weapon_condition(3));

    for unit_id in [unit_a, unit_b] {
        let unit = game_logic.host_object(unit_id).expect("loaded unit");
        assert_eq!(unit.ai_state(), AIState::Docked);
        assert_eq!(unit.contained_by, Some(chinook_id));
        assert!(!unit.can_move());
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Evacuate,
        player_id: 0,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![chinook_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    // C++ TransportContain does not dump riders synchronously: the Evacuate
    // command calls orderAllPassengersToExit → aiExit per rider
    // (OpenContain.cpp:1353-1371) and each exit is paced by the transport
    // exit door (TransportContain ExitDelay; host COMBAT_CHINOOK_EXIT_DELAY_FRAMES
    // = 3, host_combat_chinook.rs:65). Advance frames so the exit door cycles
    // both riders out and their exit walks settle, matching the C++ stream.
    for _ in 0..60 {
        game_logic.frame += 1;
        game_logic.update_movement_for_test(&[chinook_id, unit_a, unit_b], 1.0 / 30.0);
        game_logic
            .update_ai(&[chinook_id, unit_a, unit_b], 1.0 / 30.0)
            .expect("receiving AI operation succeeds");
    }

    let chinook = game_logic.host_object(chinook_id).expect("chinook empty");
    assert!(
        chinook.contained_units().is_empty(),
        "evacuate must clear all Combat Chinook residual occupants"
    );
    assert_eq!(chinook.transport_count(), 0);
    assert!(
        !chinook.test_weapon_condition(3),
        "weapon set upgrade must clear when empty"
    );

    for unit_id in [unit_a, unit_b] {
        let unit = game_logic.host_object(unit_id).expect("freed unit");
        // C++ OpenContain::exitObjectViaDoor places the rider at ExitStart and
        // issues aiFollowPath to ExitEnd (OpenContain.cpp:915-1020); the freed
        // rider walks its exit path instead of Idling in place.
        assert_eq!(
            unit.ai_state(),
            AIState::Moving,
            "unloaded unit must walk its exit path (C++ aiFollowPath)"
        );
        assert!(unit.contained_by.is_none(), "contained_by must clear");
        assert!(unit.can_move(), "unloaded unit must be free to move");
    }

    assert_eq!(game_logic.combat_chinook_residual_unloads(), 2);
    assert!(
        game_logic.honesty_combat_chinook_load_unload_ok(),
        "load+unload residual honesty"
    );
    assert_eq!(
        game_logic.transport_residual_unloads(),
        0,
        "Combat Chinook unload must not count as generic transport unload"
    );
    assert_eq!(
        game_logic.battle_bus_residual_unloads(),
        0,
        "Combat Chinook unload must not count as Battle Bus unload"
    );
    assert_eq!(
        game_logic.garrison_residual_exits(),
        0,
        "Combat Chinook unload must not count as garrison exit"
    );
}

#[test]
fn combat_chinook_residual_passenger_fire_damages_nearby_enemy() {
    let mut game_logic = GameLogic::new();
    // Ownership pair so USA passenger vs GLA enemy resolve to ENEMIES.
    game_logic.players.insert(
        0,
        crate::game_logic::Player::new(0, crate::game_logic::Team::USA, "USA", true),
    );
    game_logic.players.insert(
        1,
        crate::game_logic::Player::new(1, crate::game_logic::Team::GLA, "GLA", false),
    );
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_tank_template(&mut game_logic);

    let chinook_id = create_test_combat_chinook(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .expect("infantry");
    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(30.0, 0.0, 0.0))
        .expect("enemy");

    {
        let ai_entry_frame = game_logic.frame;
        let (unit, fixture_partition_unit) = game_logic
            .host_object_and_partition_mut(infantry_id)
            .unwrap();
        crate::fixture_content::arm_with_stats(
            unit,
            crate::game_logic::Weapon {
                damage: 40.0,
                range: 100.0,
                reload_time: 0.1,
                ..Default::default()
            },
        );
        unit.weapon.as_mut().unwrap().last_fire_time = -10.0;
        unit.target = Some(chinook_id);
        unit.set_contained_by_with_partition_owner(Some(chinook_id), true, fixture_partition_unit);
        unit.set_ai_state(
            AIState::Docked,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        unit.set_position_with_partition_owner(
            Vec3::new(0.0, 0.0, 0.0),
            0,
            &mut Default::default(),
            fixture_partition_unit,
        );
    }
    {
        let chinook = game_logic.host_object_mut(chinook_id).unwrap();
        assert!(chinook.add_occupant(infantry_id));
    }
    game_logic.refresh_battle_bus_armed_riders_weapon_set(chinook_id);

    let enemy_hp_before = game_logic
        .host_object(enemy_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    game_logic.update_combat(&[infantry_id, chinook_id, enemy_id], 1.0 / 30.0);

    let enemy_hp_after = game_logic
        .host_object(enemy_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before,
        "Combat Chinook passenger residual fire must damage nearby enemy (before={enemy_hp_before}, after={enemy_hp_after})"
    );
    assert!(
        game_logic.honesty_combat_chinook_passenger_fire_ok(),
        "passenger fire residual honesty"
    );
    assert_eq!(
        game_logic.host_object(infantry_id).unwrap().ai_state(),
        AIState::Docked,
        "firing must not eject Combat Chinook passenger"
    );
    assert_eq!(
        game_logic.host_object(infantry_id).unwrap().contained_by,
        Some(chinook_id)
    );
}

#[test]
fn combat_chinook_residual_capacity_full_rejects_enter() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    // C++ TransportContain.cpp:105 "only OUR OWN units can be transported":
    // the exact-controller gate needs a registered controlling player.
    game_logic.players.insert(
        0,
        crate::game_logic::Player::new(0, crate::game_logic::Team::USA, "USA", true),
    );
    ensure_test_infantry_template(&mut game_logic);
    let chinook_id = create_test_combat_chinook(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));

    for i in 0..crate::game_logic::host_combat_chinook::COMBAT_CHINOOK_TRANSPORT_SLOTS {
        let id = game_logic
            .create_object(
                "TestInfantry",
                Team::USA,
                Vec3::new(1.0 + i as f32 * 0.1, 0.0, 0.0),
            )
            .expect("infantry");
        {
            let ai_entry_frame = game_logic.frame;
            let unit = game_logic.host_object_mut(id).unwrap();
            unit.target = Some(chinook_id);
            unit.set_ai_state(
                AIState::Entering,
                ai_entry_frame,
                &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
            );
        }
        game_logic
            .update_ai(&[id, chinook_id], 1.0 / 30.0)
            .expect("receiving AI operation succeeds");
    }
    assert_eq!(
        game_logic
            .host_object(chinook_id)
            .unwrap()
            .transport_count(),
        crate::game_logic::host_combat_chinook::COMBAT_CHINOOK_TRANSPORT_SLOTS
    );

    let extra_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(4.0, 0.0, 0.0))
        .expect("extra");
    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter {
            target_id: chinook_id,
        },
        player_id: 0,
        command_id: 9,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![extra_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let extra = game_logic.host_object(extra_id).expect("extra after");
    assert_ne!(
        extra.ai_state(),
        AIState::Entering,
        "full Combat Chinook residual must reject Enter"
    );
    assert_eq!(
        game_logic
            .host_object(chinook_id)
            .unwrap()
            .transport_count(),
        crate::game_logic::host_combat_chinook::COMBAT_CHINOOK_TRANSPORT_SLOTS
    );
}

#[test]
fn combat_chinook_residual_allows_vehicle_enter() {
    let mut game_logic = GameLogic::new();
    // C++ TransportContain.cpp:105 "only OUR OWN units can be transported":
    // the exact-controller gate needs a registered controlling player.
    game_logic.players.insert(
        0,
        crate::game_logic::Player::new(0, crate::game_logic::Team::USA, "USA", true),
    );
    ensure_test_tank_template(&mut game_logic);
    let chinook_id = create_test_combat_chinook(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    let tank_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .expect("tank");

    {
        let ai_entry_frame = game_logic.frame;
        let unit = game_logic.host_object_mut(tank_id).unwrap();
        unit.target = Some(chinook_id);
        unit.set_ai_state(
            AIState::Entering,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }
    game_logic
        .update_ai(&[tank_id, chinook_id], 1.0 / 30.0)
        .expect("receiving AI operation succeeds");

    let chinook = game_logic.host_object(chinook_id).expect("chinook");
    assert!(
        chinook.contained_units().contains(&tank_id),
        "vehicles may enter Combat Chinook residual"
    );
    assert_eq!(game_logic.combat_chinook_residual_loads(), 1);
    let tank = game_logic.host_object(tank_id).expect("tank");
    assert_eq!(tank.ai_state(), AIState::Docked);
}

#[test]
fn combat_chinook_vehicle_rider_does_not_arm_weapon_set() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let chinook_id = create_test_combat_chinook(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    let tank_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .expect("tank");
    {
        let ai_entry_frame = game_logic.frame;
        let (unit, fixture_partition_unit) =
            game_logic.host_object_and_partition_mut(tank_id).unwrap();
        crate::fixture_content::arm_with_stats(
            unit,
            crate::game_logic::Weapon {
                damage: 20.0,
                range: 80.0,
                ..Default::default()
            },
        );
        unit.set_contained_by_with_partition_owner(Some(chinook_id), true, fixture_partition_unit);
        unit.set_ai_state(
            AIState::Docked,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }
    {
        let chinook = game_logic.host_object_mut(chinook_id).unwrap();
        assert!(
            chinook.add_occupant(tank_id),
            "Combat Chinook admits vehicles"
        );
    }
    game_logic.refresh_battle_bus_armed_riders_weapon_set(chinook_id);
    let chinook = game_logic.host_object(chinook_id).expect("chinook");
    assert!(
        !chinook.test_weapon_condition(3),
        "vehicle-only load must not set WEAPONSET_PLAYER_UPGRADE"
    );
}

#[test]
fn listening_outpost_residual_capacity_detect_and_payload() {
    use crate::game_logic::host_listening_outpost::{
        LISTENING_OUTPOST_DETECTION_RANGE, LISTENING_OUTPOST_INITIAL_PAYLOAD_COUNT,
        LISTENING_OUTPOST_STEALTH_DELAY_FRAMES, LISTENING_OUTPOST_TRANSPORT_SLOTS,
        is_listening_outpost_template,
    };

    let mut game_logic = GameLogic::new();
    let outpost_id = create_test_listening_outpost(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));
    let outpost = game_logic.host_object(outpost_id).expect("outpost");
    assert!(is_listening_outpost_template(&outpost.template_name));
    assert!(outpost.is_listening_outpost_style_container());
    assert!(outpost.can_contain());
    assert_eq!(
        outpost.transport_capacity(),
        LISTENING_OUTPOST_TRANSPORT_SLOTS
    );
    assert!(outpost.passengers_allowed_to_fire);
    assert!(outpost.armed_riders_upgrade_weapon_set);
    assert!(outpost.is_detector, "listening outpost detector residual");
    assert!(
        (outpost.detection_range - LISTENING_OUTPOST_DETECTION_RANGE).abs() < 0.1,
        "detection range residual 300, got {}",
        outpost.detection_range
    );
    assert!(
        !outpost.status.stealthed,
        "C++ ctor sets CAN_STEALTH only; STEALTHED waits StealthDelay"
    );
    assert!(outpost.innate_stealth);
    assert_eq!(
        outpost.stealth_allowed_frame,
        LISTENING_OUTPOST_STEALTH_DELAY_FRAMES
    );
    // InitialPayload TankHunter × 2 residual docks when payload template available.
    assert!(
        outpost.transport_count() == LISTENING_OUTPOST_INITIAL_PAYLOAD_COUNT
            || game_logic.listening_outpost_residual_initial_payload_docks()
                == LISTENING_OUTPOST_INITIAL_PAYLOAD_COUNT as u32,
        "InitialPayload residual docks 2 TankHunters (count={}, docks={})",
        outpost.transport_count(),
        game_logic.listening_outpost_residual_initial_payload_docks()
    );
    assert!(
        game_logic.honesty_listening_outpost_initial_payload_ok()
            || outpost.transport_count() == LISTENING_OUTPOST_INITIAL_PAYLOAD_COUNT,
        "InitialPayload residual honesty"
    );
    // Armed riders from payload upgrade weapon set residual.
    if outpost.transport_count() > 0 {
        assert!(
            outpost.test_weapon_condition(3),
            "payload armed riders must upgrade Listening Outpost weapon set"
        );
        assert!(
            outpost.weapon.is_some(),
            "PLAYER_UPGRADE residual binds ListeningOutpost dummy weapon"
        );
        if let Some(dummy) = outpost.weapon.as_ref() {
            assert!(
                crate::game_logic::host_combat_chinook::is_passenger_dummy_weapon(dummy),
                "Listening Outpost dummy residual (damage ~0.1)"
            );
        }
    }
}

#[test]
fn listening_outpost_residual_detect_stealth_in_range() {
    use crate::game_logic::host_listening_outpost::LISTENING_OUTPOST_DETECTION_RANGE;

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    game_logic.templates.get_mut("TestInfantry").unwrap().stealth_update =
        Some(crate::game_logic::object::StealthUpdateData {
            innate_stealth: false,
            granted_by_special_power: true,
            ..Default::default()
        });
    let outpost_id = create_test_listening_outpost(&mut game_logic, Vec3::new(0.0, 0.0, 0.0));

    // Stealthed enemy within 300 residual detect range.
    let stealth_id = game_logic
        .create_object(
            "TestInfantry",
            Team::GLA,
            Vec3::new(LISTENING_OUTPOST_DETECTION_RANGE * 0.5, 0.0, 0.0),
        )
        .expect("stealthed enemy");
    {
        let frame = game_logic.frame;
        let e = game_logic.host_object_mut(stealth_id).unwrap();
        e.apply_grant_stealth(frame);
        e.set_status_detected(false);
    }

    game_logic.frame = 1;
    // C++ StealthDetectorUpdate ctor sleeps the first scan by
    // GameLogicRandomValue(1, DetectionRate) (StealthDetectorUpdate.cpp:70);
    // the outpost's 900ms/27f stagger wakes at some frame <= 27, so tick
    // until the scan actually fires instead of assuming a frame-1 scan.
    for _ in 0..40 {
        game_logic.update_stealth_and_detection();
        if game_logic
            .host_object(stealth_id)
            .map(|e| e.status.detected)
            .unwrap_or(false)
        {
            break;
        }
        game_logic.frame += 1;
    }
    {
        let e = game_logic.host_object(stealth_id).expect("enemy");
        assert!(
            e.status.detected,
            "listening outpost detector residual must reveal stealthed enemy in range"
        );
    }

    // Fail-closed: enemy beyond 300 is not detected by this residual.
    let far_id = game_logic
        .create_object(
            "TestInfantry",
            Team::GLA,
            Vec3::new(LISTENING_OUTPOST_DETECTION_RANGE + 50.0, 0.0, 0.0),
        )
        .expect("far stealthed");
    {
        let frame = game_logic.frame;
        let e = game_logic.host_object_mut(far_id).unwrap();
        e.apply_grant_stealth(frame);
        e.set_status_detected(false);
    }
    // Already-detected near enemy stays marked; far must remain undetected.
    // Advance a full DetectionRate cycle (27f) so the staggered scan really
    // re-evaluates with the far enemy in world — fail-closed under a real scan.
    for _ in 0..40 {
        game_logic.frame += 1;
        game_logic.update_stealth_and_detection();
    }
    {
        let e = game_logic.host_object(far_id).expect("far");
        assert!(e.status.stealthed);
        assert!(
            !e.status.detected,
            "fail-closed: enemy outside DetectionRange 300 must not be residual-detected"
        );
    }

    // Move residual: uncloak while moving, re-cloak when idle.
    // C++ StealthForbiddenConditions=MOVING is velocity-based:
    // physics->getVelocityMagnitude() > m_stealthSpeed
    // (StealthUpdate.cpp:390-392), not the AI moving flag.
    {
        let ai_entry_frame = game_logic.frame;
        let o = game_logic.host_object_mut(outpost_id).unwrap();
        o.set_ai_state(
            AIState::Moving,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        o.movement.velocity = Vec3::new(10.0, 0.0, 0.0);
        // Direct field writes bypass the C++ PhysicsBehavior velocity cache.
        o.invalidate_velocity_magnitude();
        o.set_status_stealthed(true);
    }
    game_logic.update_stealth_and_detection();
    {
        let o = game_logic.host_object(outpost_id).expect("outpost");
        assert!(
            !o.status.stealthed,
            "moving listening outpost must uncloak residual"
        );
    }
    {
        let ai_entry_frame = game_logic.frame;
        let o = game_logic.host_object_mut(outpost_id).unwrap();
        o.set_ai_state(
            AIState::Idle,
            ai_entry_frame,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        o.movement.velocity = Vec3::ZERO;
        o.invalidate_velocity_magnitude();
        o.set_status_moving(false);
    }
    {
        let o = game_logic.host_object(outpost_id).expect("outpost");
        assert!(
            !o.status.stealthed,
            "idle listening outpost must not re-cloak instantly after stopping"
        );
    }
    let allowed = game_logic
        .host_object(outpost_id)
        .unwrap()
        .stealth_allowed_frame;
    game_logic.frame = allowed;
    game_logic.update_stealth_and_detection();
    {
        let o = game_logic.host_object(outpost_id).expect("outpost");
        assert!(
            o.status.stealthed,
            "idle listening outpost re-cloaks after StealthDelay"
        );
    }
}

#[test]
fn tunnel_network_oneshot_spawns_two_rpg_world_objects() {
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, crate::game_logic::Player::new(0, Team::GLA, "GLA", true));
    logic
        .admit_object_ini_sources([(
            "TunnelSpawn.ini".into(),
            "Object GLATunnelNetwork\n KindOf = STRUCTURE SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 1000\n InitialHealth = 1000\n End\n Behavior = TunnelContain ContainTag\n End\n Behavior = SpawnBehavior SpawnTag\n OneShot = Yes\n SpawnNumber = 2\n SpawnTemplateName = GLAInfantryTunnelDefender\n End\n Behavior = DefaultProductionExitUpdate ExitTag\n UnitCreatePoint = X:0 Y:0 Z:0\n NaturalRallyPoint = X:0 Y:0 Z:0\n End\nEnd\nObject GLAInfantryTunnelDefender\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n".into(),
        )])
        .expect("tunnel spawn definitions");

    let tunnel_id = logic
        .create_object("GLATunnelNetwork", Team::GLA, Vec3::ZERO)
        .expect("tunnel");
    for _ in 0..32 {
        logic.update();
    }
    let troopers: Vec<_> = logic
        .objects
        .registered_values()
        .filter(|o| {
            o.producer_id == Some(tunnel_id)
                && o.template_name
                    .eq_ignore_ascii_case("GLAInfantryTunnelDefender")
        })
        .map(|o| o.id)
        .collect();
    assert_eq!(
        troopers.len(),
        2,
        "C++ SpawnBehavior OneShot must create two GLAInfantryTunnelDefender world objects"
    );
    for id in &troopers {
        let trooper = logic.host_object(*id).expect("trooper");
        assert!(
            !trooper.status.disabled_held,
            "an ordinary production exit does not hold its spawned child"
        );
        let pos = trooper.get_position();
        assert!(
            (pos.x - 8.0).abs() > 0.5,
            "must not invent forward*8 + lateral*6 tunnel line, got {pos:?}"
        );
    }

    // OneShot must not fire again after children die.
    for id in troopers {
        logic.mark_object_for_destruction(id, None);
    }
    for _ in 0..32 {
        logic.update();
    }
    let again = logic
        .objects
        .registered_values()
        .filter(|o| {
            o.producer_id == Some(tunnel_id)
                && o.template_name
                    .eq_ignore_ascii_case("GLAInfantryTunnelDefender")
                && o.is_alive()
        })
        .count();
    assert_eq!(again, 0, "OneShot must not replace dead free RPG troopers");
}

#[test]
fn stinger_without_a_receiving_bone_provider_has_no_available_exit() {
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, crate::game_logic::Player::new(0, Team::GLA, "GLA", true));
    logic.admit_object_ini_sources([(
        "StingerSpawn.ini".into(),
        "Object GLAStingerSite
 KindOf = STRUCTURE SELECTABLE FS_BASE_DEFENSE SPAWNS_ARE_THE_WEAPONS
 Draw = W3DModelDraw SiteDraw
  DefaultConditionState
   Model = UBStingerS
  End
 End
 Body = ActiveBody SiteBody
  MaxHealth = 1000
  InitialHealth = 1000
 End
 Behavior = SpawnBehavior SiteSpawn
  OneShot = No
  SpawnNumber = 3
  SpawnReplaceDelay = 30000
  SpawnTemplateName = GLAInfantryStingerSoldier
 End
 Behavior = SpawnPointProductionExitUpdate SiteExit
  SpawnPointBoneName = SpawnPoint
 End
End
Object GLAInfantryStingerSoldier
 KindOf = INFANTRY SELECTABLE
 Body = ActiveBody SoldierBody
  MaxHealth = 100
  InitialHealth = 100
 End
End
".into(),
    )]).expect("stinger spawn definitions");

    let site_id = logic
        .create_object("GLAStingerSite", Team::GLA, Vec3::ZERO)
        .expect("stinger");

    for _ in 0..32 {
        logic.update();
    }
    let soldiers: Vec<_> = logic
        .objects
        .registered_values()
        .filter(|o| {
            o.producer_id == Some(site_id)
                && o.template_name
                    .eq_ignore_ascii_case("GLAInfantryStingerSoldier")
        })
        .map(|o| o.id)
        .collect();
    assert!(soldiers.is_empty(), "a missing SpawnPoint Drawable cannot reserve a door");
    assert!(!logic.host_object(site_id).unwrap().spawn_behavior_replacement_times().is_empty());
}

#[cfg(feature = "game_client")]
#[test]
fn spawn_point_exit_uses_pristine_bones_and_refuses_full_slots() {
    use std::f32::consts::FRAC_PI_2;
    fn bones(offset: f32, with_spawn_points: bool) -> crate::assets::AssetManager {
        use crate::assets::{W3dHierarchy, W3dHlod, W3dHlodLod, W3dPivot};
        let mut model = crate::assets::W3DModel::new("UBStingerSTest".into());
        let pivot = |name: &str, parent_idx, translation, rotation| W3dPivot {
            name: name.into(), parent_idx, translation, euler_angles: [0.; 3], rotation,
        };
        let q = glam::Quat::from_rotation_z(FRAC_PI_2).to_array();
        model.hierarchy = Some(W3dHierarchy {
            name: "SpawnHierarchy".into(),
            pivots: vec![
                pivot("ROOT", u32::MAX, [0., 0., 0.], [0., 0., 0., 1.]),
                pivot("SpawnPoint01", 0, [4. + offset, 0., 0.], q),
                pivot("SpawnPoint02", 0, [offset, 5., 0.], [0., 0., 0., 1.]),
                pivot("SpawnPoint03", 0, [-3. + offset, -2., 0.], [0., 0., 0., 1.]),
            ],
            pivot_fixups: Vec::new(),
        });
        if !with_spawn_points {
            model.hierarchy.as_mut().unwrap().pivots.truncate(1);
        }
        model.hlods = vec![W3dHlod {
            version: 0, name: "UBStingerSTest".into(), hierarchy_name: "SpawnHierarchy".into(),
            lods: vec![W3dHlodLod { max_screen_size: 1., subobjects: Vec::new() }],
            aggregates: None, proxies: None, has_unrendered_aggregates: false,
            has_invalid_trailing_records: false,
        }];
        let mut assets = crate::assets::AssetManager::new().unwrap();
        assets.admit_test_w3d_source("UBStingerSTest", model);
        assets
    }

    fn authored_world() -> GameLogic {
        let mut world = GameLogic::new();
        let mut weapon = gamelogic::weapon::WeaponTemplate::new("StingerMissileWeapon".into());
        weapon.primary_damage = 20.0;
        weapon.attack_range = 225.0;
        weapon.projectile_name = "NONE".into();
        world.admit_receiving_weapon_snapshot(vec![("StingerMissileWeapon".into(), weapon)]).unwrap();
        world.admit_object_ini_sources([(
            "spawn-bones.ini".into(),
            "Object GLAStingerSite
 KindOf = STRUCTURE SELECTABLE SPAWNS_ARE_THE_WEAPONS
 Draw = W3DModelDraw TestDraw
  DefaultConditionState
   Model = UBStingerSTest
  End
 End
 Body = ActiveBody SiteBody
  MaxHealth = 1000
  InitialHealth = 1000
 End
 Behavior = SpawnBehavior SiteSpawn
  OneShot = No
  SpawnNumber = 3
  SpawnReplaceDelay = 30000
  SpawnTemplateName = GLAInfantryStingerSoldier
 End
 Behavior = SpawnPointProductionExitUpdate SiteExit
  SpawnPointBoneName = SpawnPoint
 End
End
Object GLAInfantryStingerSoldier
 KindOf = INFANTRY SELECTABLE CAN_ATTACK
 VisionRange = 400
 WeaponSet
  Conditions = None
  Weapon = PRIMARY StingerMissileWeapon
 End
 Body = ActiveBody SoldierBody
  MaxHealth = 100
  InitialHealth = 100
 End
 Behavior = AIUpdateInterface SoldierAI
  AutoAcquireEnemiesWhenIdle = Yes Stealthed ATTACK_BUILDINGS
 End
End
Object SpawnEnemy
 KindOf = VEHICLE SELECTABLE
 Body = ActiveBody EnemyBody
  MaxHealth = 1000
  InitialHealth = 1000
 End
End
Object SpawnWorker
 KindOf = INFANTRY DOZER HARVESTER SELECTABLE
 Body = ActiveBody WorkerBody
  MaxHealth = 1000
  InitialHealth = 1000
 End
End
Object SpawnStructure
 KindOf = STRUCTURE SELECTABLE
 Body = ActiveBody StructureBody
  MaxHealth = 1000
  InitialHealth = 1000
 End
End
".into(),
        )]).unwrap();
        world.set_ai_definition_base(game_engine::common::ini::AIData {
            guard_inner_modifier_ai: 1.1,
            guard_outer_modifier_ai: 1.333,
            aggressive_range_modifier: 1.5,
            ..Default::default()
        });
        let mut owner = crate::game_logic::Player::new(0, Team::GLA, "GLA", false);
        owner.alliance_team = 1;
        world.add_player(owner);
        let mut enemy = crate::game_logic::Player::new(1, Team::USA, "Enemy", true);
        enemy.alliance_team = 2;
        world.add_player(enemy);
        assert!(world.select_local_player(Some(1)));
        world.setup_skirmish_ai(1);
        assert!(world.is_host_ai_active(0));
        world
    }
    let mut logic = authored_world();
    let mut peer = authored_world();
    let peer_site = peer.create_object("GLAStingerSite", Team::GLA, Vec3::new(10.0, 2.0, 20.0)).unwrap();

    let site_id = logic
        .create_object("GLAStingerSite", Team::GLA, Vec3::new(10.0, 2.0, 20.0))
        .expect("stinger");

    logic.frame = 4;
    logic.update_spawn_behavior_with_effects(
        &mut crate::game_logic::ScriptPresentationEffects {
            pristine_bones: Some(&bones(0.0, true)),
            ..Default::default()
        },
    ).unwrap();
    peer.frame = 4;
    peer.update_spawn_behavior_with_effects(
        &mut crate::game_logic::ScriptPresentationEffects {
            pristine_bones: Some(&bones(100.0, true)),
            ..Default::default()
        },
    ).unwrap();
    let mut missing = authored_world();
    let missing_site = missing.create_object(
        "GLAStingerSite", Team::GLA, Vec3::new(10.0, 2.0, 20.0),
    ).unwrap();
    missing.frame = 4;
    missing.update_spawn_behavior_with_effects(
        &mut crate::game_logic::ScriptPresentationEffects {
            pristine_bones: Some(&bones(0.0, false)),
            ..Default::default()
        },
    ).unwrap();
    assert!(missing.host_object(missing_site).unwrap().spawn_point_world_poses.as_ref().unwrap().is_empty());
    assert!(missing.host_object(missing_site).unwrap().spawn_point_occupants.is_empty());
    assert!(!missing.host_object(missing_site).unwrap().spawn_behavior_replacement_times().is_empty());
    // C++ composes the complete Object transform with each pristine matrix.
    // A +90° host Y rotation sends local +X toward world -Z; adding +90°
    // host X rotation also sends local +Z toward world -Y.
    for case in 0..3 {
        let mut transformed = authored_world();
        let origin = Vec3::new(10.0, 2.0, 20.0);
        let owner = transformed.create_object("GLAStingerSite", Team::GLA, origin).unwrap();
        let rotation = glam::Mat4::from_rotation_y(FRAC_PI_2)
            * if case > 0 { glam::Mat4::from_rotation_x(FRAC_PI_2) } else { glam::Mat4::IDENTITY }
            * if case > 1 { glam::Mat4::from_rotation_z(FRAC_PI_2) } else { glam::Mat4::IDENTITY };
        assert!(transformed.set_registered_object_transform(
            owner, glam::Mat4::from_translation(origin) * rotation,
        ));
        let assets = bones(0.0, true);
        let parent = transformed.host_object(owner).unwrap();
        let (name, expected, expected_angle) = match case {
            0 => ("SpawnPoint01", Vec3::new(10.0, 2.0, 16.0), 0.0),
            1 => ("SpawnPoint02", Vec3::new(10.0, -3.0, 20.0), -FRAC_PI_2),
            _ => ("SpawnPoint02", Vec3::new(10.0, -3.0, 20.0), 0.0),
        };
        let poses = transformed.query_pristine_world_bone_poses(
            parent, Some(&assets), name, 0, 1, 0, 0,
        ).unwrap();
        assert_eq!(poses.len(), 1);
        assert!(poses[0].0.distance(expected) < 0.001);
        assert!((poses[0].1 - expected_angle).abs() < 0.001);
        assert!(transformed.query_pristine_world_bone_poses(
            parent, Some(&assets), "NoSuchBone", 0, 1, 0, 0,
        ).unwrap().is_empty());
        assert!(transformed.query_pristine_world_bone_poses(
            parent, None, name, 0, 1, 0, 0,
        ).is_err());
        transformed.frame = 4;
        transformed.update_spawn_behavior_with_effects(
            &mut crate::game_logic::ScriptPresentationEffects {
                pristine_bones: Some(&assets),
                ..Default::default()
            },
        ).unwrap();
        let slot = if case > 0 { 1 } else { 0 };
        let (child_id, _) = transformed.host_object(owner).unwrap().spawn_point_occupants[slot].unwrap();
        let child = transformed.host_object(child_id).unwrap();
        assert!((child.get_position().x - expected.x).abs() < 0.001);
        assert!((child.get_position().z - expected.z).abs() < 0.001);
        assert!((child.get_orientation() - expected_angle).abs() < 0.001);
    }
    let peer_positions: Vec<_> = peer.objects.registered_values()
        .filter(|object| object.producer_id == Some(peer_site))
        .map(|object| object.get_position()).collect();
    assert_eq!(peer_positions.len(), 3);
    assert!(peer_positions.iter().all(|position| position.x > 100.0));
    let mut soldiers: Vec<(ObjectId, Vec3, f32)> = logic
        .objects
        .registered_values()
        .filter(|o| {
            o.producer_id == Some(site_id)
                && o.template_name
                    .eq_ignore_ascii_case("GLAInfantryStingerSoldier")
        })
        .map(|o| (o.id, o.get_position(), o.get_orientation()))
        .collect();
    soldiers.sort_by(|a, b| a.1.x.partial_cmp(&b.1.x).unwrap());
    assert_eq!(soldiers.len(), 3, "three bone slots");

    // C++ convertBonePosToWorldPos: parent (10,2,20) + local (x,z,y).
    // SpawnPoint03 (-3, -2, 0) → host (-3, 0, -2) → world (7, 2, 18)
    // SpawnPoint01 (4, 0, 0) rot Z 90° → host (4, 0, 0) → world (14, 2, 20)
    // SpawnPoint02 (0, 5, 0) → host (0, 0, 5) → world (10, 2, 25)
    assert!((soldiers[0].1.x - 7.0).abs() < 0.05 && (soldiers[0].1.z - 18.0).abs() < 0.05);
    assert!((soldiers[1].1.x - 10.0).abs() < 0.05 && (soldiers[1].1.z - 25.0).abs() < 0.05);
    assert!((soldiers[2].1.x - 14.0).abs() < 0.05 && (soldiers[2].1.z - 20.0).abs() < 0.05);
    assert!(
        (soldiers[2].2 - FRAC_PI_2).abs() < 0.05,
        "bone 01 world yaw is parent + Get_Z_Rotation"
    );
    for (id, _, _) in &soldiers {
        let obj = logic.host_object(*id).expect("held");
        assert!(obj.status.disabled_held);
        assert!(!obj.can_move());
        assert_eq!(obj.vision_range, 400.0, "child uses the admitted VisionRange");
        assert_eq!(obj.get_template().authored_ai_update_interface(), Some(true),
            "child uses the admitted AIUpdateInterface");
        assert!(!logic.skirmish_ai_auto_engage_paused(*id), "the authored AI owner is active");
    }

    let site_bones = bones(0.0, true);
    for _ in 0..15 {
        logic.frame += 1;
        logic.update_spawn_behavior_with_effects(
            &mut crate::game_logic::ScriptPresentationEffects {
                pristine_bones: Some(&site_bones),
                ..Default::default()
            },
        ).unwrap();
    }
    let again = logic
        .objects
        .registered_values()
        .filter(|o| {
            o.producer_id == Some(site_id)
                && o.template_name
                    .eq_ignore_ascii_case("GLAInfantryStingerSoldier")
                && o.is_alive()
        })
        .count();
    assert_eq!(
        again, 3,
        "reserveDoorForExit refuses when every bone is occupied"
    );
    let directory = tempfile::TempDir::new().unwrap();
    let mut saves = crate::save_load::SaveFileManager::with_save_directory(directory.path());
    saves.init().unwrap();
    let info = crate::save_load::SaveGameInfo {
        pristine_map_name: None,
        filename: "spawn_pose".into(),
        display_name: "spawn_pose".into(),
        description: String::new(),
        map_name: "SpawnPoseMap".into(),
        campaign_side: None,
        mission_number: None,
        save_date: std::time::SystemTime::now(),
        game_version: env!("CARGO_PKG_VERSION").into(),
        play_time: std::time::Duration::ZERO,
        difficulty: crate::save_load::GameDifficulty::Medium,
        save_type: crate::save_load::SaveFileType::Normal,
    };
    saves.save_game("spawn_pose", &logic, &info).unwrap();
    let mut loaded = authored_world();
    saves.load_game("spawn_pose", &mut loaded).unwrap();
    let restored_site = loaded.host_object(site_id).unwrap();
    assert_eq!(restored_site.spawn_behavior_frames_to_wait, 15);
    assert!(restored_site.spawn_point_world_poses.is_none());
    assert_eq!(restored_site.spawn_point_occupants.iter().flatten().count(), 3);
    for (id, generation) in restored_site.spawn_point_occupants.iter().flatten() {
        assert_ne!(*generation, 0);
        assert_eq!(*generation, loaded.host_object(*id).unwrap().visual_object_generation,
            "saved door identity is rebound to the receiving object's admission");
    }
    for (id, position, _) in &soldiers {
        assert_eq!(loaded.host_object(*id).unwrap().get_position(), *position);
    }
    for remaining in (1..15).rev() {
        loaded.frame += 1;
        loaded.update_spawn_behavior_with_effects(&mut Default::default()).unwrap();
        assert_eq!(loaded.host_object(site_id).unwrap().spawn_behavior_frames_to_wait, remaining);
    }
    loaded.frame += 1;
    loaded.update_spawn_behavior_with_effects(&mut Default::default()).unwrap();
    assert_eq!(loaded.host_object(site_id).unwrap().spawn_behavior_frames_to_wait, 15);
    let loaded_enemy = loaded.create_object_for_player(
        "SpawnEnemy", 1, Vec3::new(50.0, 2.0, 20.0),
    ).unwrap();
    let loaded_hp = loaded.host_object(loaded_enemy).unwrap().health.current;
    for _ in 0..240 {
        loaded.update_with_dt(1.0 / 30.0);
        if loaded.host_object(loaded_enemy).unwrap().health.current < loaded_hp { break; }
    }
    assert!(loaded.host_object(loaded_enemy).unwrap().health.current < loaded_hp,
        "restored physical children must autonomously acquire and damage an enemy");
    // The site itself cannot fire: only the physical held children can
    // acquire this authored enemy and discharge the authored hitscan weapon.
    logic.host_object_mut(site_id).unwrap().status.disabled_emp = true;
    let blocked = soldiers[0].0;
    logic.host_object_mut(blocked).unwrap().status.disabled_emp = true;
    let enemy = logic.create_object_for_player(
        "SpawnEnemy", 1, Vec3::new(50.0, 2.0, 20.0),
    ).unwrap();
    let peer_enemy = peer.create_object_for_player(
        "SpawnEnemy", 1, Vec3::new(50.0, 2.0, 20.0),
    ).unwrap();
    assert_eq!(enemy, peer_enemy);
    let peer_hp = peer.host_object(peer_enemy).unwrap().health.current;
    let initial_hp = logic.host_object(enemy).unwrap().health.current;
    let mut saw_target = false;
    for _ in 0..240 {
        logic.update_with_dt(1.0 / 30.0);
        saw_target |= soldiers.iter().skip(1).any(|(id, _, _)|
            logic.host_object(*id).is_some_and(|child| child.target == Some(enemy)));
        assert!(logic.host_object(blocked).unwrap().target.is_none());
        if logic.host_object(enemy).unwrap().health.current < initial_hp {
            break;
        }
    }
    assert!(saw_target, "an authored held child must autonomously acquire the enemy");
    assert!(logic.host_object(enemy).unwrap().health.current < initial_hp,
        "an authored held child must inflict real weapon damage");
    assert_eq!(peer.host_object(peer_enemy).unwrap().health.current, peer_hp);
    for victim_template in ["SpawnWorker", "SpawnStructure"] {
        let mut world = authored_world();
        let mut untouched = authored_world();
        let pos = Vec3::new(10.0, 2.0, 20.0);
        let site = world.create_object("GLAStingerSite", Team::GLA, pos).unwrap();
        let other_site = untouched.create_object("GLAStingerSite", Team::GLA, pos).unwrap();
        assert_eq!(site, other_site);
        let content = bones(0.0, true);
        for owner in [&mut world, &mut untouched] {
            owner.frame = 4;
            owner.update_spawn_behavior_with_effects(
                &mut crate::game_logic::ScriptPresentationEffects {
                    pristine_bones: Some(&content),
                    ..Default::default()
                },
            ).unwrap();
        }
        let child_ids: Vec<_> = world.objects.registered_values()
            .filter(|child| child.producer_id == Some(site))
            .map(|child| child.id)
            .collect();
        assert_eq!(child_ids.len(), 3);
        assert!(child_ids.iter().all(|id| world.host_object(*id).unwrap().status.disabled_held));
        world.host_object_mut(site).unwrap().status.disabled_emp = true;
        world.host_object_mut(child_ids[0]).unwrap().status.disabled_emp = true;
        let target_pos = Vec3::new(50.0, 2.0, 20.0);
        let victim = world.create_object_for_player(victim_template, 1, target_pos).unwrap();
        let other_victim = untouched.create_object_for_player(victim_template, 1, target_pos).unwrap();
        assert_eq!(victim, other_victim);
        let hp = world.host_object(victim).unwrap().health.current;
        let mut acquired = false;
        for _ in 0..240 {
            world.update_with_dt(1.0 / 30.0);
            acquired |= child_ids.iter().skip(1).any(|id|
                world.host_object(*id).is_some_and(|child| child.target == Some(victim)));
            assert!(world.host_object(child_ids[0]).unwrap().target.is_none());
            if world.host_object(victim).unwrap().health.current < hp {
                break;
            }
        }
        assert!(acquired, "held child must acquire unarmed {victim_template}");
        assert!(world.host_object(victim).unwrap().health.current < hp,
            "held child must damage unarmed {victim_template}");
        assert_eq!(untouched.host_object(other_victim).unwrap().health.current, hp);
    }

}

#[test]
fn retail_stinger_w3d_spawns_held_children_from_mounted_archives() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../windows_game");
    let base = root.join("Command & Conquer Generals/W3D.big");
    let expansion = root.join("Command & Conquer Generals Zero Hour/Data/W3DZH.big");
    if !base.exists() || !expansion.exists() {
        eprintln!("retail W3D archives unavailable");
        return;
    }
    let mut manager = crate::assets::AssetManager::new().unwrap();
    manager.load_user_mods("", base.to_str().unwrap()).unwrap();
    manager.load_user_mods("", expansion.to_str().unwrap()).unwrap();
    let assets = std::sync::Mutex::new(manager);
    let mut world = GameLogic::new();
    world.admit_object_ini_sources([(
        "retail-stinger-w3d.ini".into(),
        "Object GLAStingerSite
 KindOf = STRUCTURE SELECTABLE SPAWNS_ARE_THE_WEAPONS
 Draw = W3DModelDraw DrawTag
  DefaultConditionState
   Model = UBStingerS
  End
 End
 Body = ActiveBody SiteBody
  MaxHealth = 1000
  InitialHealth = 1000
 End
 Behavior = SpawnBehavior SiteSpawn
  SpawnNumber = 3
  SpawnReplaceDelay = 30000
  SpawnTemplateName = GLAInfantryStingerSoldier
 End
 Behavior = SpawnPointProductionExitUpdate SiteExit
  SpawnPointBoneName = SpawnPoint
 End
End
Object GLAInfantryStingerSoldier
 KindOf = INFANTRY SELECTABLE
 Body = ActiveBody SoldierBody
  MaxHealth = 100
  InitialHealth = 100
 End
End
".into(),
    )]).unwrap();
    world.add_player(crate::game_logic::Player::new(0, Team::GLA, "GLA", true));
    let site = world.create_object("GLAStingerSite", Team::GLA, Vec3::ZERO).unwrap();
    world.frame = 4;
    world.update_spawn_behavior_with_effects(
        &mut crate::game_logic::ScriptPresentationEffects {
            pristine_bones: Some(&assets),
            ..Default::default()
        },
    ).unwrap();
    let children: Vec<_> = world.objects.registered_values()
        .filter(|object| object.producer_id == Some(site)).collect();
    assert_eq!(children.len(), 3, "retail W3D must expose three available SpawnPoints");
    assert!(children.iter().all(|child| child.status.disabled_held));
    assert!(children.iter().any(|child| child.get_position().x != 0.0 || child.get_position().z != 0.0));
}

#[test]
fn special_power_create_expresses_shared_n_sync_ready_now() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::{Player, SpecialPowerModuleKind, SpecialPowerModuleMetadata};

    let mut logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.reset_shared_special_power_timer(&SpecialPowerType::SpyDrone, 60.0);
    assert!(
        player
            .shared_special_power_cooldowns
            .get(&SpecialPowerType::SpyDrone)
            .copied()
            .unwrap_or(0.0)
            > 0.0
    );
    logic.players.insert(0, player);

    let mut tpl = ThingTemplate::new("AmericaCommandCenter");
    tpl.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(2000.0);
    tpl.has_special_power_create = true;
    tpl.special_power_modules.push(SpecialPowerModuleMetadata {
        source_index: 0,
        module_tag: Some("ModuleTag_SpyDrone".into()),
        module_kind: SpecialPowerModuleKind::SpecialAbility,
        special_power_template: "SpecialPowerSpyDrone".into(),
        special_power_template_id: 1,
        command_power: Some(SpecialPowerType::SpyDrone),
        reload_time_frames: 300,
        required_science: Some("SCIENCE_SpyDrone".into()),
        public_timer: false,
        shared_n_sync: true,
        shortcut_power: false,
        update_module_starts_attack: false,
        starts_paused: false,
        scripted_special_power_only: false,
        cash_bounty: None,
    });
    tpl.special_power_modules.push(SpecialPowerModuleMetadata {
        source_index: 1,
        module_tag: Some("ModuleTag_Scripted".into()),
        module_kind: SpecialPowerModuleKind::SpecialAbility,
        special_power_template: "SpecialPowerScriptedOnly".into(),
        special_power_template_id: 2,
        command_power: Some(SpecialPowerType::Paradrop),
        reload_time_frames: 150,
        required_science: None,
        public_timer: false,
        shared_n_sync: false,
        shortcut_power: false,
        update_module_starts_attack: false,
        starts_paused: false,
        scripted_special_power_only: true,
        cash_bounty: None,
    });
    logic.templates.insert("AmericaCommandCenter".into(), tpl);

    let id = logic
        .create_object_for_player("AmericaCommandCenter", 0, Vec3::ZERO)
        .expect("cc");
    let player = logic.players.get(&0).expect("player");
    assert!(
        !player
            .shared_special_power_cooldowns
            .contains_key(&SpecialPowerType::SpyDrone),
        "SharedNSync must express ready-now on Command Center build complete"
    );
    let obj = logic.host_object(id).expect("cc obj");
    let remaining = obj
        .special_power_cooldowns
        .get(&SpecialPowerType::Paradrop)
        .copied()
        .unwrap_or(0.0);
    assert!(
        remaining > 0.0,
        "scripted SpecialPowerCreate modules must start ReloadTime recharge"
    );
}

#[test]
fn spawned_require_spawner_kills_children_through_their_own_die_modules() {
    // C++ SpawnBehavior.cpp:150-171: after clearing producers, each still-live
    // spawn gets Object::kill() (Object.cpp:1930-1944), i.e. a normal damage
    // request routed through the child's Body/Die modules. A SlowDeath child
    // must therefore begin its own slow death and stay registered rather than
    // being zeroed and force-removed.
    let mut logic = GameLogic::new();
    logic
        .admit_object_ini_sources([(
            "RequiredSpawnLib.ini".into(),
            "Object Nest\n KindOf = STRUCTURE SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 500\n InitialHealth = 500\n End\n Behavior = SpawnBehavior Spawn\n OneShot = Yes\n SpawnNumber = 2\n SpawnTemplateName = Child\n SpawnedRequireSpawner = Yes\n End\n Behavior = DefaultProductionExitUpdate ExitTag\n UnitCreatePoint = X:0 Y:0 Z:0\n NaturalRallyPoint = X:0 Y:0 Z:0\n End\nEnd\nObject Child\n KindOf = INFANTRY SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = SlowDeathBehavior Death\n DestructionDelay = 3000\n End\nEnd\n"
                .into(),
        )])
        .expect("ini");
    logic
        .players
        .insert(1, crate::game_logic::Player::new(1, Team::GLA, "Owner", true));
    let nest = logic
        .create_object_for_player("Nest", 1, Vec3::ZERO)
        .expect("nest");
    for _ in 0..32 {
        logic.update();
    }
    let children: Vec<_> = logic
        .objects
        .registered_values()
        .filter(|o| o.producer_id == Some(nest))
        .map(|o| o.id)
        .collect();
    assert_eq!(children.len(), 2);

    logic.apply_host_damage(nest, 1000.0);

    for id in children {
        let child = logic
            .host_object(id)
            .expect("SlowDeath retains the killed child");
        assert!(child.status.effectively_dead, "child was killed");
        assert!(child.slow_death.is_some(), "child's own SlowDeath ran");
        assert!(child.producer_id.is_none(), "producer cleared first");
    }
}

#[test]
fn authored_upgrade_die_removes_each_modules_upgrade_from_producer_like_cpp() {
    // C++ UpgradeDie::onDie (UpgradeDie.cpp): each module removes its own
    // authored UpgradeToRemove from the producer. No template-name inference;
    // two UpgradeDie modules on one Object each run once.
    let mut logic = GameLogic::new();
    for upgrade in ["Upgrade_FixtureA", "Upgrade_FixtureB"] {
        let mut ini = game_engine::common::ini::INI::new();
        ini.with_inline_source(&format!("Upgrade {upgrade}\nType = OBJECT\nEnd\n"), |ini| {
            ini.read_line()?;
            logic.upgrade_definitions.parse_upgrade_definition(ini)
        })
        .expect("upgrade def");
    }
    logic
        .admit_object_ini_sources([(
            "AuthoredUpgradeDie.ini".into(),
            "Object FixtureProducer\n KindOf = STRUCTURE SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 500\n InitialHealth = 500\n End\nEnd\nObject FixtureGenericChild\n KindOf = VEHICLE SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = UpgradeDie DieA\n DeathTypes = ALL\n UpgradeToRemove = Upgrade_FixtureA\n End\n Behavior = UpgradeDie DieB\n DeathTypes = ALL\n UpgradeToRemove = Upgrade_FixtureB\n End\nEnd\n"
                .into(),
        )])
        .expect("ini");
    logic.players.insert(
        1,
        crate::game_logic::Player::new(1, Team::USA, "Owner", true),
    );
    let producer = logic
        .create_object_for_player("FixtureProducer", 1, Vec3::ZERO)
        .expect("producer");
    logic.apply_upgrade_to_object(producer, "Upgrade_FixtureA");
    logic.apply_upgrade_to_object(producer, "Upgrade_FixtureB");
    let child = logic
        .create_object_for_player("FixtureGenericChild", 1, Vec3::new(20.0, 0.0, 0.0))
        .expect("child");
    logic.host_object_mut(child).unwrap().producer_id = Some(producer);
    assert!(
        logic
            .host_object(producer)
            .unwrap()
            .has_object_upgrade_complete("Upgrade_FixtureA")
    );

    logic.apply_host_damage(child, 1000.0);

    let p = logic.host_object(producer).unwrap();
    assert!(!p.has_object_upgrade_complete("Upgrade_FixtureA"), "DieA removes its upgrade");
    assert!(!p.has_object_upgrade_complete("Upgrade_FixtureB"), "DieB removes its upgrade");
}

#[test]
fn create_installs_upgrade_die_from_authored_module_without_template_name_sniff() {
    // C++ ModuleFactory binds UpgradeDie from the Object's authored module
    // data (UpgradeToRemove); the template name plays no part.
    let mut logic = GameLogic::new();
    logic
        .admit_object_ini_sources([(
            "AuthoredUpgradeDieCreate.ini".into(),
            "Object FixtureGenericDroneChild\n KindOf = VEHICLE SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\n Behavior = UpgradeDie DieA\n DeathTypes = ALL\n UpgradeToRemove = Upgrade_FixtureGeneric\n End\nEnd\nObject AmericaVehicleScoutDrone\n KindOf = VEHICLE SELECTABLE\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n"
                .into(),
        )])
        .expect("ini");
    logic.players.insert(
        1,
        crate::game_logic::Player::new(1, Team::USA, "Owner", true),
    );
    let generic = logic
        .create_object_for_player("FixtureGenericDroneChild", 1, Vec3::ZERO)
        .expect("generic");
    assert_eq!(
        logic
            .host_object(generic)
            .unwrap()
            .upgrade_die
            .as_ref()
            .map(|ud| ud.upgrade_to_remove.as_str()),
        Some("Upgrade_FixtureGeneric"),
        "authored UpgradeToRemove binds at create"
    );
    let named = logic
        .create_object_for_player("AmericaVehicleScoutDrone", 1, Vec3::new(30.0, 0.0, 0.0))
        .expect("named");
    assert!(
        logic.host_object(named).unwrap().upgrade_die.is_none(),
        "retail drone name without authored UpgradeDie installs nothing"
    );
}
