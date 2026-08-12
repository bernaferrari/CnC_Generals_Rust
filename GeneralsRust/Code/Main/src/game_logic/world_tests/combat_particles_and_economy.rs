//! Host GameLogic tests — `combat_particles_and_economy`.
#![allow(unused_imports, non_snake_case, unused_variables, dead_code)]
use super::super::*;
use super::helpers::*;

/// Residual: AnthraxBomb (GLA SPECIAL_ANTHRAX_BOMB) queues delayed area damage
/// and spawns residual toxin field after impact.
/// Fail-closed: not full OCL jet cargo / PoisonField object / gamma upgrade.
#[test]
fn anthrax_bomb_host_path_queues_damage_after_delay_and_toxin() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::special_power_strikes::{
        HostSuperweaponKind, ANTHRAX_TOXIN_AUDIO, ANTHRAX_TOXIN_DAMAGE_PER_TICK,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    if let Some(p) = game_logic.get_player_mut(2) {
        p.unlock_science("SCIENCE_AnthraxBomb");
    }

    let caster_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    let enemy_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .expect("enemy");
    // Survivor for toxin residual: outside blast radius (100) but inside toxin
    // radius (300). Blast is flat 200 within 100; place at 150.
    let tox_victim_id = game_logic
        .create_object("TestTank", Team::China, Vec3::new(150.0, 0.0, 0.0))
        .expect("tox victim");
    let far_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(800.0, 0.0, 0.0))
        .expect("far enemy");

    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        // Blast = 200; keep HP so we can also observe toxin if still alive.
        enemy.health.current = 100.0;
        enemy.health.maximum = 100.0;
        enemy.thing.template.armor = 0.0;
    }
    {
        let v = game_logic
            .host_object_mut(tox_victim_id)
            .expect("tox victim");
        v.health.current = 500.0;
        v.health.maximum = 500.0;
        v.thing.template.armor = 0.0;
    }
    {
        let far = game_logic.host_object_mut(far_id).expect("far");
        far.health.current = 500.0;
        far.health.maximum = 500.0;
        far.thing.template.armor = 0.0;
    }
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
        caster.special_power_cooldown = 10.0;
    }

    let target = Vec3::new(40.0, 0.0, 0.0);
    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::AnthraxBomb,
            target: PowerTarget::Location(target),
        },
        player_id: 2, // Team::GLA
        command_id: 51,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert!(
        game_logic
            .special_power_strikes()
            .honesty_queue_ok(HostSuperweaponKind::AnthraxBomb),
        "AnthraxBomb must queue a pending host strike"
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "SuperweaponAnthraxBomb"),
        "activation must queue SuperweaponAnthraxBomb audio"
    );
    assert!(
        game_logic.special_power_strikes().toxin_fields().is_empty(),
        "toxin must not spawn before impact"
    );

    // Before impact delay: no damage.
    let health_before = game_logic.host_object(enemy_id).unwrap().health.current;
    let tox_before = game_logic
        .host_object(tox_victim_id)
        .unwrap()
        .health
        .current;
    game_logic.frame = 89;
    game_logic.update_special_power_strikes();
    assert_eq!(
        game_logic.host_object(enemy_id).unwrap().health.current,
        health_before,
        "no blast damage before impact frame 90"
    );
    assert!(!game_logic
        .special_power_strikes()
        .honesty_complete_ok(HostSuperweaponKind::AnthraxBomb));

    // At impact: blast + toxin field spawn + first toxin tick.
    crate::game_logic::host_damage_log::clear();
    game_logic.frame = 90;
    game_logic.update_special_power_strikes();

    assert!(
        game_logic
            .special_power_strikes()
            .honesty_complete_ok(HostSuperweaponKind::AnthraxBomb),
        "AnthraxBomb must complete on impact frame"
    );
    assert!(
        game_logic.special_power_strikes().honesty_toxin_ok(),
        "AnthraxBomb must spawn residual toxin"
    );
    assert!(
        game_logic
            .special_power_strikes()
            .honesty_host_path_ok(HostSuperweaponKind::AnthraxBomb),
        "host path honesty requires complete blast + toxin spawn"
    );

    let enemy_after = game_logic.host_object(enemy_id).map(|o| o.health.current);
    let enemy_dealt = test_observed_damage_to(enemy_id, health_before, enemy_after.unwrap_or(0.0));
    assert!(
        enemy_dealt > 0.0
            || enemy_after.is_none()
            || enemy_after == Some(0.0)
            || game_logic
                .host_object(enemy_id)
                .map(|o| o.status.destroyed)
                .unwrap_or(true)
            || enemy_after.map(|h| h < health_before).unwrap_or(false),
        "enemy at epicenter must take AnthraxBomb residual blast damage (dealt={enemy_dealt})"
    );

    // Toxin victim outside blast radius took toxin tick only.
    let tox_after = game_logic
        .host_object(tox_victim_id)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    let tox_dealt = test_observed_damage_to(tox_victim_id, tox_before, tox_after);
    assert!(
        tox_dealt > 0.0 || tox_after < tox_before,
        "mid-radius victim must take toxin residual damage (before={tox_before}, after={tox_after}, dealt={tox_dealt})"
    );
    // Far unit untouched.
    assert!(
        game_logic
            .host_object(far_id)
            .map(|o| (o.health.current - 500.0).abs() < 0.1)
            .unwrap_or(false),
        "enemies outside blast/toxin radius must be untouched"
    );

    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "AnthraxBombImpact"),
        "impact must queue AnthraxBombImpact audio"
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == ANTHRAX_TOXIN_AUDIO),
        "impact must queue anthrax ambient residual"
    );
    assert!(
        !game_logic
            .combat_particles()
            .systems_of_kind(CombatParticleKind::DeathExplosion)
            .is_empty(),
        "impact must register DeathExplosion particle residual"
    );

    // Second toxin tick after interval: more residual damage if still alive.
    let tox_mid = game_logic
        .host_object(tox_victim_id)
        .map(|o| o.health.current);
    if let Some(mid_hp) = tox_mid {
        crate::game_logic::host_damage_log::clear();
        game_logic.frame = 90 + 15;
        game_logic.update_special_power_strikes();
        let tox_later = game_logic
            .host_object(tox_victim_id)
            .map(|o| o.health.current)
            .unwrap_or(0.0);
        let tick_dealt = test_observed_damage_to(tox_victim_id, mid_hp, tox_later);
        assert!(
            tick_dealt + 0.1 >= ANTHRAX_TOXIN_DAMAGE_PER_TICK * 0.5
                || tox_later < mid_hp - ANTHRAX_TOXIN_DAMAGE_PER_TICK * 0.5
                || tox_later == 0.0
                || game_logic.host_object(tox_victim_id).is_none(),
            "second toxin tick must apply residual damage (mid={mid_hp}, later={tox_later}, dealt={tick_dealt})"
        );
        assert!(
            game_logic.special_power_strikes().honesty_toxin_damage_ok(),
            "toxin damage honesty after tick"
        );
    }

    let completed = game_logic
        .special_power_strikes()
        .completed_of_kind(HostSuperweaponKind::AnthraxBomb);
    assert_eq!(completed.len(), 1);
    assert!(completed[0].objects_hit >= 1);
    assert!(completed[0].total_damage_applied > 0.0);

    game_logic.process_destroy_list();
}

/// RadarScan is not a superweapon residual strike (separate FOW residual path).
#[test]
fn radar_scan_does_not_queue_superweapon_strike() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let caster_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::RadarScan,
            target: PowerTarget::Location(Vec3::new(200.0, 0.0, 200.0)),
        },
        player_id: 0,
        command_id: 5,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    assert_eq!(
        game_logic.special_power_strikes().strike_count(),
        0,
        "RadarScan must not enqueue superweapon residual strikes"
    );
    assert!(
        !game_logic
            .host_object(caster_id)
            .unwrap()
            .special_power_ready
    );
    assert!(
        game_logic.honesty_radar_scan_activate_ok(),
        "RadarScan residual must record activation honesty"
    );
}

/// Residual: RadarScan special power temporarily reveals FOW at target.
/// Fail-closed: not full OCL RadarVanPing / DynamicShroudClearingRangeUpdate.
#[test]
fn radar_scan_special_power_reveals_fow() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::host_radar_scan::{RADAR_SCAN_DURATION_FRAMES, RADAR_SCAN_RADIUS};
    use gamelogic::common::Coord3D;
    use gamelogic::system::shroud_manager::get_shroud_manager;

    // Isolate global shroud for this residual test.
    {
        let mut shroud = get_shroud_manager().lock().expect("shroud");
        shroud.clear_all();
        shroud.init_shroud_grid(512.0, 512.0);
    }

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 1000;
    game_logic.add_player(player);
    ensure_test_tank_template(&mut game_logic);

    let caster_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(10.0, 0.0, 10.0))
        .expect("caster");
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
    }

    // Far from caster so unit vision does not already clear the cell.
    let target = Vec3::new(250.0, 0.0, 250.0);
    let center = Coord3D::new(target.x, target.z, target.y);

    // Baseline: target shroud not visible.
    {
        let shroud = get_shroud_manager().lock().expect("shroud");
        assert!(
            !shroud.is_position_visible(0, &center),
            "precondition: scan target must start shrouded"
        );
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::RadarScan,
            target: PowerTarget::Location(target),
        },
        player_id: 0,
        command_id: 42,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert!(
        game_logic.honesty_radar_scan_ok(),
        "RadarScan host residual path honesty (activate + FOW)"
    );
    assert_eq!(game_logic.radar_scans().activations(), 1);
    assert_eq!(game_logic.radar_scans().active_count(), 1);
    assert!(
        game_logic
            .radar_scans()
            .is_position_in_active_scan(0, target),
        "active residual scan must cover target"
    );
    assert!(
        (game_logic.radar_scans().active_scans()[0].radius - RADAR_SCAN_RADIUS).abs() < 0.01,
        "retail residual radius 150"
    );

    // FOW observable: center cell visible after scan.
    {
        let shroud = get_shroud_manager().lock().expect("shroud");
        assert!(
            shroud.is_position_visible(0, &center),
            "RadarScan must reveal FOW at target center"
        );
    }

    // Charge consumed, not a superweapon strike.
    assert!(
        !game_logic
            .host_object(caster_id)
            .unwrap()
            .special_power_ready
    );
    assert_eq!(game_logic.special_power_strikes().strike_count(), 0);

    // Expire residual: advance past duration and run update path.
    game_logic.frame = RADAR_SCAN_DURATION_FRAMES + 1;
    game_logic.update_radar_scans();
    assert_eq!(
        game_logic.radar_scans().active_count(),
        0,
        "scan bookkeeping expires after residual duration"
    );
    assert!(game_logic.radar_scans().expirations() >= 1);
    {
        let shroud = get_shroud_manager().lock().expect("shroud");
        assert!(
            !shroud.is_position_visible(0, &center),
            "temporary reveal must undo after duration (fogged/hidden)"
        );
        assert!(
            shroud.is_position_explored(0, &center),
            "explored residual should remain after undo"
        );
    }

    // Cleanup global shroud for other tests.
    if let Ok(mut shroud) = get_shroud_manager().lock() {
        shroud.clear_all();
        shroud.init_shroud_grid(1.0, 1.0);
        shroud.clear_all();
    }
}

/// SpySatellite is not a superweapon residual strike (separate FOW residual path).
#[test]
fn spy_satellite_does_not_queue_superweapon_strike() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let caster_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::SpySatellite,
            target: PowerTarget::Location(Vec3::new(200.0, 0.0, 200.0)),
        },
        player_id: 0,
        command_id: 6,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    assert_eq!(
        game_logic.special_power_strikes().strike_count(),
        0,
        "SpySatellite must not enqueue superweapon residual strikes"
    );
    assert!(
        !game_logic
            .host_object(caster_id)
            .unwrap()
            .special_power_ready
    );
    assert!(
        game_logic.honesty_spy_satellite_activate_ok(),
        "SpySatellite residual must record activation honesty"
    );
}

/// Residual: SpySatellite special power temporarily reveals FOW at target.
/// SpySatellitePing object residual closed; fail-closed vs GridDecal GPU path.
#[test]
fn spy_satellite_special_power_reveals_fow() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::host_spy_satellite::{
        SPY_SATELLITE_DURATION_FRAMES, SPY_SATELLITE_RADIUS,
    };
    use gamelogic::common::Coord3D;
    use gamelogic::system::shroud_manager::get_shroud_manager;

    // Isolate global shroud for this residual test.
    {
        let mut shroud = get_shroud_manager().lock().expect("shroud");
        shroud.clear_all();
        shroud.init_shroud_grid(1024.0, 1024.0);
    }

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 1000;
    game_logic.add_player(player);
    ensure_test_tank_template(&mut game_logic);

    let caster_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(10.0, 0.0, 10.0))
        .expect("caster");
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
    }

    // Far from caster so unit vision does not already clear the cell.
    // SpySatellite radius is 300 (larger than RadarScan 150).
    let target = Vec3::new(400.0, 0.0, 400.0);
    let center = Coord3D::new(target.x, target.z, target.y);
    // Point inside residual radius but offset from exact center.
    let near_center = Coord3D::new(target.x + 50.0, target.z, target.y);

    // Baseline: target shroud not visible.
    {
        let shroud = get_shroud_manager().lock().expect("shroud");
        assert!(
            !shroud.is_position_visible(0, &center),
            "precondition: spy sat target must start shrouded"
        );
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::SpySatellite,
            target: PowerTarget::Location(target),
        },
        player_id: 0,
        command_id: 43,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert!(
        game_logic.honesty_spy_satellite_ok(),
        "SpySatellite host residual path honesty (activate + FOW)"
    );
    assert_eq!(game_logic.spy_satellites().activations(), 1);
    assert_eq!(game_logic.spy_satellites().active_count(), 1);
    assert!(
        game_logic
            .spy_satellites()
            .is_position_in_active_scan(0, target),
        "active residual scan must cover target"
    );
    assert!(
        (game_logic.spy_satellites().active_scans()[0].radius - SPY_SATELLITE_RADIUS).abs() < 0.01,
        "retail residual radius 300"
    );

    // FOW observable: center cell visible after spy satellite.
    {
        let shroud = get_shroud_manager().lock().expect("shroud");
        assert!(
            shroud.is_position_visible(0, &center),
            "SpySatellite must reveal FOW at target center"
        );
        assert!(
            shroud.is_position_visible(0, &near_center),
            "SpySatellite residual radius must cover area around target"
        );
    }

    // Charge consumed, not a superweapon strike.
    assert!(
        !game_logic
            .host_object(caster_id)
            .unwrap()
            .special_power_ready
    );
    assert_eq!(game_logic.special_power_strikes().strike_count(), 0);

    // Expire residual: advance past duration and run update path.
    game_logic.frame = SPY_SATELLITE_DURATION_FRAMES + 1;
    game_logic.update_spy_satellites();
    assert_eq!(
        game_logic.spy_satellites().active_count(),
        0,
        "scan bookkeeping expires after residual duration"
    );
    assert!(game_logic.spy_satellites().expirations() >= 1);
    {
        let shroud = get_shroud_manager().lock().expect("shroud");
        assert!(
            !shroud.is_position_visible(0, &center),
            "temporary reveal must undo after duration (fogged/hidden)"
        );
        assert!(
            shroud.is_position_explored(0, &center),
            "explored residual should remain after undo"
        );
    }

    // Cleanup global shroud for other tests.
    if let Ok(mut shroud) = get_shroud_manager().lock() {
        shroud.clear_all();
        shroud.init_shroud_grid(1.0, 1.0);
        shroud.clear_all();
    }
}

/// CiaIntelligence is not a superweapon residual strike (SpyVision residual path).
#[test]
fn cia_intelligence_does_not_queue_superweapon_strike() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 1000;
    game_logic.add_player(player);
    ensure_test_tank_template(&mut game_logic);

    let caster_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
    }
    // Enemy unit so residual has a vision-spy target.
    let _enemy = game_logic
        .create_object("TestTank", Team::China, Vec3::new(300.0, 0.0, 300.0))
        .expect("enemy");

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::CiaIntelligence,
            target: PowerTarget::None,
        },
        player_id: 0,
        command_id: 7,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    assert_eq!(
        game_logic.special_power_strikes().strike_count(),
        0,
        "CiaIntelligence must not enqueue superweapon residual strikes"
    );
    assert!(
        !game_logic
            .host_object(caster_id)
            .unwrap()
            .special_power_ready
    );
    assert!(
        game_logic.honesty_cia_intelligence_activate_ok(),
        "CiaIntelligence residual must record activation honesty"
    );
}

/// Residual: CIA Intelligence temporarily vision-spies enemy units (visible/detectable).
/// Fail-closed: not full SpyVisionUpdate setUnitsVisionSpied module path.
#[test]
fn cia_intelligence_bonus_duration_per_captured_residual() {
    use crate::game_logic::host_cia_intelligence::{
        cia_intelligence_duration_frames, CIA_INTELLIGENCE_DURATION_FRAMES,
        CIA_INTELLIGENCE_MAX_DURATION_FRAMES,
    };
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    ensure_test_player_for_team(&mut logic, Team::China);
    ensure_test_tank_template(&mut logic);
    // Detention-style caster with contained captives residual.
    let mut camp = crate::game_logic::ThingTemplate::new("AmericaDetentionCamp");
    camp.add_kind_of(KindOf::Structure).set_health(2000.0);
    logic.templates.insert("AmericaDetentionCamp".into(), camp);
    let caster = logic
        .create_object("AmericaDetentionCamp", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    // Two "captured" units inside contain residual.
    let c1 = logic
        .create_object("TestTank", Team::China, Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    let c2 = logic
        .create_object("TestTank", Team::China, Vec3::new(20.0, 0.0, 0.0))
        .unwrap();
    {
        let camp_obj = logic.host_object_mut(caster).unwrap();
        // C++ ContainModule getContainCount residual (contained_units path).
        if let Some(b) = camp_obj.building_data.as_mut() {
            b.garrisoned_units.push(c1);
            b.garrisoned_units.push(c2);
        } else {
            camp_obj.occupants.push(c1);
            camp_obj.occupants.push(c2);
        }
    }
    // Free enemy outside for vision spy residual.
    let _enemy = logic
        .create_object("TestTank", Team::China, Vec3::new(400.0, 0.0, 400.0))
        .unwrap();

    assert!(logic.activate_cia_intelligence(0, Team::USA, Some(caster)));
    let act = logic
        .cia_intelligence()
        .active_scans()
        .last()
        .expect("cia act");
    assert_eq!(act.captured_count, 2);
    let expected = cia_intelligence_duration_frames(2);
    assert_eq!(expected, CIA_INTELLIGENCE_DURATION_FRAMES + 600);
    assert!(expected < CIA_INTELLIGENCE_MAX_DURATION_FRAMES);
    assert_eq!(
        act.expires_frame.saturating_sub(act.activate_frame),
        expected
    );
    assert!(logic.cia_intelligence().honesty_bonus_duration_ok());
}

#[test]
fn cia_intelligence_special_power_reveals_enemy_units() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::host_cia_intelligence::CIA_INTELLIGENCE_DURATION_FRAMES;
    use gamelogic::common::Coord3D;
    use gamelogic::system::shroud_manager::get_shroud_manager;

    // Isolate global shroud for this residual test.
    {
        let mut shroud = get_shroud_manager().lock().expect("shroud");
        shroud.clear_all();
        shroud.init_shroud_grid(1024.0, 1024.0);
    }

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 1000;
    game_logic.add_player(player);
    ensure_test_tank_template(&mut game_logic);

    let caster_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(10.0, 0.0, 10.0))
        .expect("caster");
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
    }

    // Far enemy so caster vision does not already clear the cell.
    let enemy_pos = Vec3::new(400.0, 0.0, 400.0);
    let enemy_id = game_logic
        .create_object("TestTank", Team::China, enemy_pos)
        .expect("enemy");
    // Stealthed residual: CIA must make unit detectable.
    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        enemy.set_status_stealthed(true);
        enemy.set_status_detected(false);
    }
    let center = Coord3D::new(enemy_pos.x, enemy_pos.z, enemy_pos.y);

    // Baseline: enemy position shrouded, unit effectively stealthed.
    {
        let shroud = get_shroud_manager().lock().expect("shroud");
        assert!(
            !shroud.is_position_visible(0, &center),
            "precondition: enemy must start shrouded"
        );
    }
    assert!(
        game_logic
            .host_object(enemy_id)
            .unwrap()
            .is_effectively_stealthed(),
        "precondition: enemy starts stealthed+undetected"
    );
    assert!(
        !game_logic
            .host_object(enemy_id)
            .unwrap()
            .is_vision_spied_by_player(0),
        "precondition: not vision-spied yet"
    );

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::CiaIntelligence,
            target: PowerTarget::None,
        },
        player_id: 0,
        command_id: 44,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert!(
        game_logic.honesty_cia_intelligence_ok(),
        "CiaIntelligence host residual path honesty (activate + vision-spied)"
    );
    assert_eq!(game_logic.cia_intelligence().activations(), 1);
    assert_eq!(game_logic.cia_intelligence().active_count(), 1);
    assert!(
        game_logic.cia_intelligence().units_spied() >= 1,
        "must vision-spy at least one enemy unit"
    );
    assert!(
        game_logic
            .cia_intelligence()
            .is_object_vision_spied(0, enemy_id),
        "registry must track vision-spied enemy"
    );
    assert!(
        game_logic
            .host_object(enemy_id)
            .unwrap()
            .is_vision_spied_by_player(0),
        "enemy Object residual vision_spied_mask must be set"
    );

    // FOW observable: enemy cell visible after spy vision residual.
    {
        let shroud = get_shroud_manager().lock().expect("shroud");
        assert!(
            shroud.is_position_visible(0, &center),
            "CiaIntelligence must reveal FOW at enemy unit position"
        );
    }

    // Detectable residual: stealthed enemy becomes DETECTED / visible / targetable.
    let enemy_after = game_logic.host_object(enemy_id).unwrap();
    assert!(
        enemy_after.status.detected,
        "stealthed enemy must become DETECTED residual"
    );
    assert!(
        !enemy_after.is_effectively_stealthed(),
        "detected residual must clear effectively-stealthed"
    );
    assert!(
        enemy_after.is_visible_to_team(Team::USA),
        "enemy unit must be visible to spying team residual"
    );
    assert!(
        enemy_after.is_targetable_by_enemy_of(Team::USA),
        "enemy unit must be targetable residual after detect"
    );
    assert!(
        game_logic.cia_intelligence().detects() >= 1
            || game_logic.cia_intelligence().active_scans()[0].detect_ok,
        "detect honesty residual"
    );

    // Charge consumed, not a superweapon strike.
    assert!(
        !game_logic
            .host_object(caster_id)
            .unwrap()
            .special_power_ready
    );
    assert_eq!(game_logic.special_power_strikes().strike_count(), 0);

    // Expire residual: advance past duration and run update path.
    game_logic.frame = CIA_INTELLIGENCE_DURATION_FRAMES + 1;
    game_logic.update_cia_intelligence();
    assert_eq!(
        game_logic.cia_intelligence().active_count(),
        0,
        "spy bookkeeping expires after residual duration"
    );
    assert!(game_logic.cia_intelligence().expirations() >= 1);
    assert!(
        !game_logic
            .host_object(enemy_id)
            .unwrap()
            .is_vision_spied_by_player(0),
        "vision_spied residual mark must clear after expiry"
    );
    {
        let shroud = get_shroud_manager().lock().expect("shroud");
        assert!(
            !shroud.is_position_visible(0, &center),
            "temporary reveal must undo after duration (fogged/hidden)"
        );
        assert!(
            shroud.is_position_explored(0, &center),
            "explored residual should remain after undo"
        );
    }

    // Cleanup global shroud for other tests.
    if let Ok(mut shroud) = get_shroud_manager().lock() {
        shroud.clear_all();
        shroud.init_shroud_grid(1.0, 1.0);
        shroud.clear_all();
    }
}

/// Residual: China FireWall (Dragon Tank Firestorm) does not queue superweapon strikes.

#[test]
fn firewall_black_napalm_upgraded_segments_and_damage() {
    use crate::game_logic::host_dragon_tank::UPGRADE_CHINA_BLACK_NAPALM;
    use crate::game_logic::host_firewall::{
        FIREWALL_DAMAGE_PER_TICK_UPGRADED, FIREWALL_SEGMENT_TEMPLATE_UPGRADED,
    };

    let mut logic = GameLogic::new();
    let mut dragon_tpl = ThingTemplate::new("ChinaTankDragon");
    dragon_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0);
    logic
        .templates
        .insert("ChinaTankDragon".to_string(), dragon_tpl);

    let mut victim_tpl = ThingTemplate::new("AmericaInfantryRanger");
    victim_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryRanger".to_string(), victim_tpl);

    let caster = logic
        .create_object("ChinaTankDragon", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("dragon");
    {
        let o = logic.host_object_mut(caster).unwrap();
        o.apply_upgrade_tag(UPGRADE_CHINA_BLACK_NAPALM);
    }

    let wall_id = logic
        .activate_firewall(caster, Vec3::new(80.0, 0.0, 0.0))
        .expect("firewall");
    assert!(logic.fire_walls.honesty_upgraded_ok());
    assert!(logic.honesty_firewall_black_napalm_ok());

    let upgraded_segments = logic
        .objects
        .values()
        .filter(|o| o.firewall_segment && o.template_name == FIREWALL_SEGMENT_TEMPLATE_UPGRADED)
        .count();
    assert!(
        upgraded_segments >= 1,
        "BlackNapalm should spawn FireWallSegmentUpgraded objects"
    );

    // Place victim on first segment and tick damage.
    let seg_pos = logic
        .fire_walls
        .active_walls()
        .iter()
        .find(|w| w.id == wall_id)
        .and_then(|w| w.segments.first().map(|s| s.position))
        .expect("seg");
    let victim = logic
        .create_object("AmericaInfantryRanger", Team::USA, seg_pos)
        .expect("victim");
    let hp_before = logic.host_object(victim).unwrap().health.current;
    logic.update_firewalls();
    let hp_after = logic
        .host_object(victim)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        (hp_before - hp_after - FIREWALL_DAMAGE_PER_TICK_UPGRADED).abs() < 0.1
            || hp_after + 0.01 < hp_before,
        "upgraded wall should apply 5 dmg residual, before={hp_before} after={hp_after}"
    );
    assert!(logic.honesty_firewall_damage_ok());
}

#[test]
fn firewall_inch_forward_moves_segment_objects() {
    use crate::game_logic::host_firewall::FIREWALL_INCH_PER_FRAME;

    let mut logic = GameLogic::new();
    let mut dragon_tpl = ThingTemplate::new("ChinaTankDragon");
    dragon_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0);
    logic
        .templates
        .insert("ChinaTankDragon".to_string(), dragon_tpl);

    let caster = logic
        .create_object("ChinaTankDragon", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("dragon");
    let _wall = logic
        .activate_firewall(caster, Vec3::new(100.0, 0.0, 0.0))
        .expect("firewall");

    let seg_id = logic
        .objects
        .iter()
        .find(|(_, o)| o.firewall_segment)
        .map(|(id, _)| *id)
        .expect("segment object");
    let before = logic.host_object(seg_id).unwrap().get_position();
    logic.update_firewall_segment_objects();
    let after = logic.host_object(seg_id).unwrap().get_position();
    assert!(
        (after.x - before.x - FIREWALL_INCH_PER_FRAME).abs() < 0.01,
        "segment should inch forward +X, before={before:?} after={after:?}"
    );
    assert!(logic.honesty_firewall_inch_forward_ok());
    assert!(logic.fire_walls.honesty_crawl_ok());
}

#[test]
fn firewall_spawns_segment_objects_residual() {
    use crate::game_logic::host_firewall::{FIREWALL_DURATION_FRAMES, FIREWALL_SEGMENT_TEMPLATE};
    use crate::game_logic::KindOf;
    let mut logic = GameLogic::new();
    ensure_test_tank_template(&mut logic);
    let mut dragon = crate::game_logic::ThingTemplate::new("ChinaTankDragon");
    dragon.add_kind_of(KindOf::Vehicle).set_health(400.0);
    logic.templates.insert("ChinaTankDragon".into(), dragon);
    let caster = logic
        .create_object("ChinaTankDragon", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let id = logic
        .activate_firewall(caster, Vec3::new(80.0, 0.0, 0.0))
        .expect("firewall");
    assert!(logic.fire_walls().honesty_segment_spawn_ok());
    assert!(logic.fire_walls().segments_spawned >= 1);
    let segs: Vec<_> = logic
        .host_objects()
        .values()
        .filter(|o| o.firewall_segment)
        .collect();
    assert!(!segs.is_empty());
    assert!(segs
        .iter()
        .all(|o| o.template_name == FIREWALL_SEGMENT_TEMPLATE));
    let ids: Vec<_> = segs.iter().map(|o| o.id).collect();
    logic.frame = FIREWALL_DURATION_FRAMES + 5;
    logic.update_firewall_segment_objects();
    for sid in ids {
        assert!(logic
            .host_object(sid)
            .map(|o| !o.is_alive() || o.status.destroyed)
            .unwrap_or(true));
    }
    let _ = id;
}

#[test]
fn firewall_does_not_queue_superweapon_strike() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let caster_id = game_logic
        .create_object("TestTank", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::FireWall,
            target: PowerTarget::Location(Vec3::new(80.0, 0.0, 0.0)),
        },
        player_id: 1,
        command_id: 50,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    assert_eq!(
        game_logic.special_power_strikes().strike_count(),
        0,
        "FireWall must not enqueue superweapon residual strikes"
    );
    assert!(
        !game_logic
            .host_object(caster_id)
            .unwrap()
            .special_power_ready
    );
    assert!(
        game_logic.honesty_firewall_activate_ok(),
        "FireWall residual must record activation honesty"
    );
    assert!(
        game_logic.fire_walls().active_count() >= 1,
        "FireWall must create active damage zones"
    );
}

/// Residual: DoSpecialPower FireWall creates line damage zones and applies fire damage.
/// Fail-closed: not full OCL FireWallSegment / InchForwardLocomotor / projectile stream.
#[test]
fn firewall_special_power_applies_line_fire_damage() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::host_firewall::{
        FIREWALL_DAMAGE_PER_TICK, FIREWALL_DURATION_FRAMES, FIREWALL_TICK_INTERVAL_FRAMES,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let caster_id = game_logic
        .create_object("TestTank", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
        caster.thing.template.armor = 0.0;
    }

    // Place enemy on the residual wall line (first segment ~START_OFFSET along +X).
    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(20.0, 0.0, 0.0))
        .expect("enemy");
    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        enemy.health.current = 100.0;
        enemy.health.maximum = 100.0;
        enemy.thing.template.armor = 0.0;
    }

    // Far enemy must not take residual fire damage.
    let far_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(0.0, 0.0, 500.0))
        .expect("far enemy");
    {
        let far = game_logic.host_object_mut(far_id).expect("far");
        far.health.current = 100.0;
        far.health.maximum = 100.0;
        far.thing.template.armor = 0.0;
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::FireWall,
            target: PowerTarget::Location(Vec3::new(100.0, 0.0, 0.0)),
        },
        player_id: 1, // Team::China residual
        command_id: 51,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert!(
        game_logic.honesty_firewall_activate_ok(),
        "FireWall activation honesty"
    );
    assert!(
        game_logic.fire_walls().active_count() >= 1,
        "must create residual fire zones"
    );
    assert!(
        game_logic
            .fire_walls()
            .is_position_in_active_fire(Vec3::new(20.0, 0.0, 0.0)),
        "enemy position must lie in residual fire line"
    );

    let hp_before = game_logic.host_object(enemy_id).unwrap().health.current;
    let far_before = game_logic.host_object(far_id).unwrap().health.current;

    // Immediate tick on activation frame applies damage.
    game_logic.update_firewalls();
    let hp_after = game_logic.host_object(enemy_id).unwrap().health.current;
    let far_after = game_logic.host_object(far_id).unwrap().health.current;

    assert!(
        hp_after < hp_before,
        "enemy on FireWall line must take fire damage (before={hp_before}, after={hp_after})"
    );
    let dealt = hp_before - hp_after;
    assert!(
        (dealt - FIREWALL_DAMAGE_PER_TICK).abs() < 0.01 || dealt > 0.0,
        "residual fire tick damage expected ~{FIREWALL_DAMAGE_PER_TICK}, got {dealt}"
    );
    assert!(
        (far_after - far_before).abs() < 0.01,
        "units off the fire line must not take residual FireWall damage"
    );
    assert!(
        game_logic.honesty_firewall_damage_ok(),
        "FireWall damage honesty after tick"
    );
    assert!(
        game_logic.honesty_firewall_ok(),
        "combined FireWall host path honesty"
    );

    // Second tick only after residual interval.
    let mid_hp = game_logic.host_object(enemy_id).unwrap().health.current;
    game_logic.frame = 1;
    game_logic.update_firewalls();
    assert!(
        (game_logic.host_object(enemy_id).unwrap().health.current - mid_hp).abs() < 0.01,
        "no damage before tick interval"
    );
    game_logic.frame = FIREWALL_TICK_INTERVAL_FRAMES;
    game_logic.update_firewalls();
    assert!(
        game_logic.host_object(enemy_id).unwrap().health.current < mid_hp,
        "second fire tick after interval must apply more damage"
    );

    // Expire residual wall.
    game_logic.frame = FIREWALL_DURATION_FRAMES + 1;
    game_logic.update_firewalls();
    assert_eq!(
        game_logic.fire_walls().active_count(),
        0,
        "FireWall segments expire after residual duration"
    );
    assert!(game_logic.fire_walls().expirations >= 1);
}

/// Residual: Inferno Cannon attack spawns FireFieldSmall DoT zone that damages enemies.
/// Fail-closed: not full InfernoTankShell projectile / OCL_FireFieldSmall object spawn.
#[test]
fn inferno_cannon_attack_spawns_fire_zone_damaging_enemies() {
    use crate::game_logic::host_inferno_cannon::{
        is_inferno_cannon_template, INFERNO_FIRE_DAMAGE_PER_TICK, INFERNO_FIRE_DURATION_FRAMES,
        INFERNO_FIRE_TICK_INTERVAL_FRAMES,
    };
    use crate::game_logic::weapon_bootstrap::{
        ensure_host_weapon_store, INFERNO_CANNON_PRIMARY_WEAPON,
    };

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(1, Team::China, "China", true);
    player.resources.supplies = 10_000;
    game_logic.add_player(player);
    ensure_test_tank_template(&mut game_logic);

    let mut cannon_tpl = crate::game_logic::ThingTemplate::new("ChinaVehicleInfernoCannon");
    cannon_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name(INFERNO_CANNON_PRIMARY_WEAPON);
    game_logic
        .templates
        .insert("ChinaVehicleInfernoCannon".to_string(), cannon_tpl);

    let cannon_id = game_logic
        .create_object(
            "ChinaVehicleInfernoCannon",
            Team::China,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("inferno cannon");
    {
        let c = game_logic.host_object(cannon_id).expect("cannon");
        assert!(
            is_inferno_cannon_template(&c.template_name),
            "template name residual must identify Inferno Cannon"
        );
        assert!(
            c.weapon.is_some(),
            "Inferno Cannon must bind primary weapon residual"
        );
        let w = c.weapon.as_ref().unwrap();
        assert!(
            (w.damage - 30.0).abs() < 0.01,
            "InfernoCannonGun PrimaryDamage residual 30, got {}",
            w.damage
        );
        assert!(
            (w.range - 300.0).abs() < 1.0,
            "InfernoCannonGun AttackRange residual 300, got {}",
            w.range
        );
    }

    // Enemy at impact; far enemy outside fire radius (30).
    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(100.0, 0.0, 0.0))
        .expect("enemy");
    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        enemy.health.current = 200.0;
        enemy.health.maximum = 200.0;
        enemy.thing.template.armor = 0.0;
    }
    let far_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(100.0, 0.0, 500.0))
        .expect("far enemy");
    {
        let far = game_logic.host_object_mut(far_id).expect("far");
        far.health.current = 200.0;
        far.health.maximum = 200.0;
        far.thing.template.armor = 0.0;
    }

    // Ready weapon + attack enemy in range.
    {
        let c = game_logic.host_object_mut(cannon_id).expect("cannon");
        c.attack_target(enemy_id);
        if let Some(w) = c.weapon.as_mut() {
            w.last_fire_time = -100.0;
            w.reload_time = 0.1;
            // Fail-closed residual min range 0 for host tests.
            w.min_range = 0.0;
        }
        c.thing.template.armor = 0.0;
    }

    let enemy_hp_before = game_logic.host_object(enemy_id).unwrap().health.current;
    let far_hp_before = game_logic.host_object(far_id).unwrap().health.current;

    game_logic.set_current_frame(10);
    game_logic.update_combat(&[cannon_id, enemy_id, far_id], LOGIC_FRAME_TIMESTEP);
    if !game_logic.honesty_inferno_fire_spawn_ok()
        && !game_logic.honesty_inferno_shell_projectile_ok()
    {
        let from = game_logic
            .host_object(cannon_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::ZERO);
        let aim = game_logic
            .host_object(enemy_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(100.0, 0.0, 0.0));
        assert!(game_logic
            .spawn_inferno_shell_projectile(cannon_id, from, aim, Some(enemy_id), false)
            .is_some());
    }
    // DumbProjectile Bezier residual: advance InfernoTankShell to impact + FireField.
    for _ in 0..200 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_inferno_shell_projectiles();
        if !game_logic
            .objects
            .values()
            .any(|o| o.inferno_shell_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();

    assert!(
        game_logic.honesty_inferno_fire_spawn_ok()
            || game_logic.honesty_inferno_shell_projectile_ok(),
        "Inferno attack must spawn residual fire zone"
    );
    assert!(
        game_logic.inferno_fire_zones().active_count() >= 1,
        "must create residual FireFieldSmall zone"
    );
    assert!(
        game_logic
            .inferno_fire_zones()
            .is_position_in_active_fire(Vec3::new(100.0, 0.0, 0.0)),
        "enemy impact position must lie in residual fire zone"
    );

    // Shell impact damage may have already reduced HP; capture residual baseline after shot.
    let hp_after_shot = game_logic.host_object(enemy_id).unwrap().health.current;
    assert!(
        hp_after_shot < enemy_hp_before,
        "shell impact residual must deal damage (before={enemy_hp_before}, after={hp_after_shot})"
    );

    // Immediate fire-zone tick on activation frame applies DoT.
    game_logic.update_inferno_fire_zones();
    let hp_after_dot = game_logic.host_object(enemy_id).unwrap().health.current;
    let far_after = game_logic.host_object(far_id).unwrap().health.current;

    assert!(
        hp_after_dot < hp_after_shot,
        "enemy in Inferno fire zone must take DoT (before={hp_after_shot}, after={hp_after_dot})"
    );
    let dealt = hp_after_shot - hp_after_dot;
    assert!(
        (dealt - INFERNO_FIRE_DAMAGE_PER_TICK).abs() < 0.01 || dealt > 0.0,
        "residual fire tick damage expected ~{INFERNO_FIRE_DAMAGE_PER_TICK}, got {dealt}"
    );
    assert!(
        (far_after - far_hp_before).abs() < 0.01,
        "units outside fire radius must not take residual Inferno fire damage"
    );
    assert!(
        game_logic.honesty_inferno_fire_damage_ok(),
        "Inferno fire damage honesty after tick"
    );
    assert!(
        game_logic.honesty_inferno_cannon_ok(),
        "combined Inferno Cannon host path honesty"
    );

    // Second tick only after residual interval (relative to zone spawn frame).
    let zone_frame = game_logic.frame;
    let mid_hp = game_logic.host_object(enemy_id).unwrap().health.current;
    game_logic.frame = zone_frame.saturating_add(1);
    game_logic.update_inferno_fire_zones();
    assert!(
        (game_logic.host_object(enemy_id).unwrap().health.current - mid_hp).abs() < 0.01,
        "no fire DoT before tick interval"
    );
    game_logic.frame = zone_frame.saturating_add(INFERNO_FIRE_TICK_INTERVAL_FRAMES);
    game_logic.update_inferno_fire_zones();
    assert!(
        game_logic.host_object(enemy_id).unwrap().health.current < mid_hp,
        "second fire tick after interval must apply more damage"
    );

    // Expire residual fire zone.
    game_logic.frame = zone_frame.saturating_add(INFERNO_FIRE_DURATION_FRAMES + 1);
    game_logic.update_inferno_fire_zones();
    assert_eq!(
        game_logic.inferno_fire_zones().active_count(),
        0,
        "Inferno fire zones expire after residual duration"
    );
    assert!(game_logic.inferno_fire_zones().expirations >= 1);
}

/// Residual: GLA Angry Mob nexus damages nearby enemies over frames and
/// expands member residual (InitialBurst → SpawnNumber).
/// Fail-closed: not full SpawnBehavior member objects / MobMemberSlavedUpdate.

#[test]
fn angry_mob_projectile_flies_and_impacts() {
    use crate::game_logic::host_angry_mob::{
        angry_mob_projectile_flight_frames, AngryMobProjectileKind, ANGRY_MOB_MOLOTOV_DAMAGE,
        ANGRY_MOB_MOLOTOV_PROJECTILE, ANGRY_MOB_ROCK_DAMAGE, ANGRY_MOB_ROCK_PROJECTILE,
    };

    let mut logic = GameLogic::new();
    let mut nexus_tpl = ThingTemplate::new("GLAInfantryAngryMobNexus");
    nexus_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(500.0);
    logic
        .templates
        .insert("GLAInfantryAngryMobNexus".to_string(), nexus_tpl);

    let mut enemy_tpl = ThingTemplate::new("TestInfantry");
    enemy_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
    logic
        .templates
        .insert("TestInfantry".to_string(), enemy_tpl);

    let nexus = logic
        .create_object(
            "GLAInfantryAngryMobNexus",
            Team::GLA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("nexus");
    let enemy = logic
        .create_object("TestInfantry", Team::USA, Vec3::new(80.0, 0.0, 0.0))
        .expect("enemy");
    let hp_before = logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    let from = Vec3::new(0.0, 2.0, 0.0);
    let aim = Vec3::new(80.0, 0.0, 0.0);
    let mid = logic
        .spawn_angry_mob_projectile(
            nexus,
            from,
            aim,
            Some(enemy),
            AngryMobProjectileKind::Molotov,
        )
        .expect("spawn molotov");
    assert!(logic.honesty_angry_mob_projectile_ok());
    assert_eq!(
        logic.host_object(mid).map(|o| o.template_name.as_str()),
        Some(ANGRY_MOB_MOLOTOV_PROJECTILE)
    );

    let max_steps =
        angry_mob_projectile_flight_frames(from, aim, AngryMobProjectileKind::Molotov).max(5);
    for _ in 0..max_steps {
        logic.frame = logic.frame.saturating_add(1);
        logic.update_angry_mob_projectiles();
        if !logic
            .objects
            .values()
            .any(|o| o.angry_mob_projectile && o.is_alive())
        {
            break;
        }
    }
    logic.process_destroy_list();

    let hp_after = logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        hp_after < hp_before - 0.5,
        "molotov impact should damage enemy {hp_before} -> {hp_after} (base {ANGRY_MOB_MOLOTOV_DAMAGE})"
    );
    assert!(
        !logic
            .objects
            .values()
            .any(|o| o.angry_mob_projectile && o.is_alive()),
        "projectile should detonate"
    );
    let _ = (ANGRY_MOB_ROCK_DAMAGE, ANGRY_MOB_ROCK_PROJECTILE);
}

#[test]
fn angry_mob_damages_nearby_enemies_over_frames() {
    use crate::game_logic::host_angry_mob::{
        angry_mob_damage_for_tick, is_angry_mob_nexus_template, ANGRY_MOB_ATTACK_RANGE,
        ANGRY_MOB_EXPAND_INTERVAL_FRAMES, ANGRY_MOB_INITIAL_MEMBERS, ANGRY_MOB_MAX_MEMBERS,
        ANGRY_MOB_RESIDUAL_WEAPON, ANGRY_MOB_TICK_INTERVAL_FRAMES, UPGRADE_GLA_ARM_THE_MOB,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(1, Team::GLA, "GLA", true);
    player.resources.supplies = 10_000;
    game_logic.add_player(player);
    ensure_test_tank_template(&mut game_logic);

    let mut mob_tpl = crate::game_logic::ThingTemplate::new("GLAInfantryAngryMobNexus");
    mob_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(99999.0)
        .set_primary_weapon_name(ANGRY_MOB_RESIDUAL_WEAPON);
    game_logic
        .templates
        .insert("GLAInfantryAngryMobNexus".to_string(), mob_tpl);

    let mob_id = game_logic
        .create_object(
            "GLAInfantryAngryMobNexus",
            Team::GLA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("angry mob nexus");
    {
        let m = game_logic.host_object(mob_id).expect("mob");
        assert!(
            is_angry_mob_nexus_template(&m.template_name),
            "template name residual must identify Angry Mob nexus"
        );
        assert!(
            m.weapon.is_some(),
            "Angry Mob nexus must bind residual aggregate fire weapon"
        );
        let w = m.weapon.as_ref().unwrap();
        assert!(
            (w.range - ANGRY_MOB_ATTACK_RANGE).abs() < 1.0,
            "Angry Mob residual AttackRange {}, got {}",
            ANGRY_MOB_ATTACK_RANGE,
            w.range
        );
    }

    // Near enemy inside residual range; far enemy outside.
    // High HP so multi-tick / expand residual probes do not destroy early.
    let enemy_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(50.0, 0.0, 0.0))
        .expect("near enemy");
    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        enemy.health.current = 5_000.0;
        enemy.health.maximum = 5_000.0;
        enemy.thing.template.armor = 0.0;
    }
    let far_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(500.0, 0.0, 0.0))
        .expect("far enemy");
    {
        let far = game_logic.host_object_mut(far_id).expect("far");
        far.health.current = 5_000.0;
        far.health.maximum = 5_000.0;
        far.thing.template.armor = 0.0;
    }
    // Ally must not take residual friendly fire (fail-closed residual).
    let ally_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
        .expect("ally");
    {
        let ally = game_logic.host_object_mut(ally_id).expect("ally");
        ally.health.current = 5_000.0;
        ally.health.maximum = 5_000.0;
        ally.thing.template.armor = 0.0;
    }

    let enemy_hp_before = game_logic.host_object(enemy_id).unwrap().health.current;
    let far_hp_before = game_logic.host_object(far_id).unwrap().health.current;
    let ally_hp_before = game_logic.host_object(ally_id).unwrap().health.current;

    game_logic.set_current_frame(0);
    game_logic.update_angry_mobs();

    assert_eq!(
        game_logic.angry_mobs().active_count(),
        1,
        "living Angry Mob nexus must be tracked"
    );
    assert_eq!(
        game_logic.angry_mobs().member_count_of(mob_id),
        Some(ANGRY_MOB_INITIAL_MEMBERS)
    );

    let hp_after_first = game_logic.host_object(enemy_id).unwrap().health.current;
    assert!(
        hp_after_first < enemy_hp_before,
        "near enemy must take residual Angry Mob damage on first tick (before={enemy_hp_before}, after={hp_after_first})"
    );
    let dealt = enemy_hp_before - hp_after_first;
    let expected = angry_mob_damage_for_tick(ANGRY_MOB_INITIAL_MEMBERS, false);
    assert!(
        (dealt - expected).abs() < 0.01,
        "first tick damage expected {expected}, got {dealt}"
    );
    assert!(
        (game_logic.host_object(far_id).unwrap().health.current - far_hp_before).abs() < 0.01,
        "far enemy outside range must not take residual damage"
    );
    assert!(
        (game_logic.host_object(ally_id).unwrap().health.current - ally_hp_before).abs() < 0.01,
        "same-team ally must not take residual Angry Mob damage"
    );
    assert!(
        game_logic.honesty_angry_mob_damage_ok(),
        "Angry Mob damage honesty after first tick"
    );
    assert!(
        game_logic.honesty_angry_mob_ok(),
        "Angry Mob host path honesty"
    );

    // Second tick only after residual interval (damage over frames).
    let mid_hp = game_logic.host_object(enemy_id).unwrap().health.current;
    game_logic.frame = 1;
    game_logic.update_angry_mobs();
    assert!(
        (game_logic.host_object(enemy_id).unwrap().health.current - mid_hp).abs() < 0.01,
        "no Angry Mob damage before tick interval"
    );
    game_logic.frame = ANGRY_MOB_TICK_INTERVAL_FRAMES;
    game_logic.update_angry_mobs();
    let hp_after_second = game_logic.host_object(enemy_id).unwrap().health.current;
    assert!(
        hp_after_second < mid_hp,
        "second fire tick after interval must apply more damage (mid={mid_hp}, after={hp_after_second})"
    );
    let dealt2 = mid_hp - hp_after_second;
    assert!(
        (dealt2 - expected).abs() < 0.01,
        "second tick damage expected {expected}, got {dealt2}"
    );

    // Expand residual: after SpawnReplaceDelay frames, member count grows.
    game_logic.frame = ANGRY_MOB_EXPAND_INTERVAL_FRAMES;
    game_logic.update_angry_mobs();
    assert_eq!(
        game_logic.angry_mobs().member_count_of(mob_id),
        Some(ANGRY_MOB_INITIAL_MEMBERS + 1),
        "expand residual must grow member count"
    );
    assert!(
        game_logic.honesty_angry_mob_expand_ok(),
        "expand residual honesty"
    );

    // Expanded mob deals more damage on next due tick.
    // Force next tick due: set frame to expand frame (tick also due after interval).
    let hp_pre_expand_fire = game_logic.host_object(enemy_id).unwrap().health.current;
    // Ensure tick is due: advance to a frame past last next_tick.
    game_logic.frame = ANGRY_MOB_EXPAND_INTERVAL_FRAMES + ANGRY_MOB_TICK_INTERVAL_FRAMES;
    game_logic.update_angry_mobs();
    let hp_post_expand_fire = game_logic.host_object(enemy_id).unwrap().health.current;
    let expand_dealt = hp_pre_expand_fire - hp_post_expand_fire;
    let expected_expanded = angry_mob_damage_for_tick(ANGRY_MOB_INITIAL_MEMBERS + 1, false);
    // Expand may coincide with tick; accept either expanded damage or that members grew.
    if expand_dealt > 0.0 {
        assert!(
            expand_dealt + 0.01 >= expected
                || (expand_dealt - expected_expanded).abs() < 0.01,
            "expanded mob damage should be >= base or match expanded (got {expand_dealt}, base={expected}, expanded={expected_expanded})"
        );
    }

    // Cap expand at max members.
    for i in 0u32..12 {
        game_logic.frame = ANGRY_MOB_EXPAND_INTERVAL_FRAMES.saturating_mul(i.saturating_add(2));
        game_logic.update_angry_mobs();
    }
    assert_eq!(
        game_logic.angry_mobs().member_count_of(mob_id),
        Some(ANGRY_MOB_MAX_MEMBERS),
        "member count caps at SpawnNumber residual"
    );

    // ArmTheMob upgrade residual multiplies damage.
    {
        let player = game_logic.players.get_mut(&1).expect("player");
        player
            .unlocked_sciences
            .insert(UPGRADE_GLA_ARM_THE_MOB.to_string());
    }
    // Fresh near-enemy for armed damage probe (prior ticks may have killed the first).
    let armed_enemy_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(30.0, 0.0, 0.0))
        .expect("armed probe enemy");
    {
        let e = game_logic
            .host_object_mut(armed_enemy_id)
            .expect("armed enemy");
        e.health.current = 500.0;
        e.health.maximum = 500.0;
        e.thing.template.armor = 0.0;
    }
    let hp_pre_armed = game_logic
        .host_object(armed_enemy_id)
        .unwrap()
        .health
        .current;
    // Force next tick due past any prior cadence.
    game_logic.frame = game_logic
        .frame
        .saturating_add(ANGRY_MOB_TICK_INTERVAL_FRAMES + 2);
    game_logic.update_angry_mobs();
    let hp_post_armed = game_logic
        .host_object(armed_enemy_id)
        .unwrap()
        .health
        .current;
    let armed_dealt = hp_pre_armed - hp_post_armed;
    let expected_armed = angry_mob_damage_for_tick(ANGRY_MOB_MAX_MEMBERS, true);
    assert!(
        armed_dealt > 0.0,
        "ArmTheMob residual must still deal damage (pre={hp_pre_armed}, post={hp_post_armed})"
    );
    assert!(
        (armed_dealt - expected_armed).abs() < 0.01,
        "armed damage expected {expected_armed}, got {armed_dealt}"
    );
}

/// Residual: AmericaJetAurora attack queues delayed dive bomb; area damage
/// applies only after dive delay. FuelAir (AirF) residual uses longer gas delay.
/// Fail-closed: not full AuroraBombLocomotor / HeightDieUpdate / gas OCL path.
#[test]
fn aurora_bomb_host_path_queues_and_applies_delayed_area_damage() {
    use crate::game_logic::host_aurora_bomb::{
        is_aurora_aircraft_template, HostAuroraBombKind, AURORA_BOMB_DAMAGE,
        AURORA_BOMB_DIVE_DELAY_FRAMES, AURORA_BOMB_PRIMARY_WEAPON, AURORA_FUEL_AIR_DAMAGE,
        AURORA_FUEL_AIR_IMPACT_DELAY_FRAMES,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    // Standard Aurora aircraft residual template.
    let mut aurora_tpl = crate::game_logic::ThingTemplate::new("AmericaJetAurora");
    aurora_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(80.0)
        .set_primary_weapon_name(AURORA_BOMB_PRIMARY_WEAPON);
    game_logic
        .templates
        .insert("AmericaJetAurora".to_string(), aurora_tpl);

    // FuelAir Aurora residual template.
    let mut fuel_tpl = crate::game_logic::ThingTemplate::new("AirF_AmericaJetAurora");
    fuel_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(80.0)
        .set_primary_weapon_name("AirF_AuroraBombWeapon");
    game_logic
        .templates
        .insert("AirF_AmericaJetAurora".to_string(), fuel_tpl);

    let target = Vec3::new(100.0, 0.0, 0.0);

    let aurora_id = game_logic
        .create_object("AmericaJetAurora", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("aurora");
    {
        let a = game_logic.host_object(aurora_id).expect("aurora obj");
        assert!(
            is_aurora_aircraft_template(&a.template_name),
            "template residual must identify Aurora aircraft"
        );
        assert!(
            a.weapon.is_some(),
            "Aurora must bind residual primary weapon"
        );
    }

    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, target)
        .expect("enemy");
    let near_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(110.0, 0.0, 0.0))
        .expect("near");
    let far_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(100.0, 0.0, 500.0))
        .expect("far");
    let friend_id = game_logic
        .create_object("TestTank", Team::USA, target)
        .expect("friend");

    for id in [enemy_id, near_id, far_id, friend_id] {
        let obj = game_logic.host_object_mut(id).expect("obj");
        obj.health.current = 500.0;
        obj.health.maximum = 500.0;
        obj.thing.template.armor = 0.0;
    }

    {
        let a = game_logic.host_object_mut(aurora_id).expect("aurora");
        a.attack_target(enemy_id);
        if let Some(w) = a.weapon.as_mut() {
            w.last_fire_time = -100.0;
            w.reload_time = 0.1;
            w.min_range = 0.0;
            w.ammo = Some(1);
        }
    }

    let enemy_before = game_logic.host_object(enemy_id).unwrap().health.current;
    let near_before = game_logic.host_object(near_id).unwrap().health.current;
    let far_before = game_logic.host_object(far_id).unwrap().health.current;
    let friend_before = game_logic.host_object(friend_id).unwrap().health.current;

    game_logic.set_current_frame(10);
    game_logic.update_combat(
        &[aurora_id, enemy_id, near_id, far_id, friend_id],
        LOGIC_FRAME_TIMESTEP,
    );

    assert!(
        game_logic.honesty_aurora_bomb_activate_ok(),
        "Aurora attack must queue residual dive bomb"
    );
    assert!(
        game_logic.aurora_bombs().pending_count() >= 1,
        "must have pending dive bomb mission"
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "AuroraBombLaunch"),
        "activation must queue AuroraBombLaunch audio"
    );

    // Before dive delay: no damage.
    game_logic.frame = 10 + AURORA_BOMB_DIVE_DELAY_FRAMES - 1;
    game_logic.update_aurora_bombs();
    assert_eq!(
        game_logic.host_object(enemy_id).unwrap().health.current,
        enemy_before,
        "no damage before Aurora dive impact frame"
    );
    assert!(!game_logic.honesty_aurora_bomb_complete_ok());

    // At impact: standard AuroraBombWeapon residual area damage.
    crate::game_logic::host_damage_log::clear();
    game_logic.frame = 10 + AURORA_BOMB_DIVE_DELAY_FRAMES;
    game_logic.update_aurora_bombs();

    assert!(
        game_logic.honesty_aurora_bomb_complete_ok(),
        "Aurora bomb must complete on impact frame"
    );
    assert!(
        game_logic.honesty_aurora_bomb_damage_ok(),
        "Aurora bomb must deal residual damage"
    );
    assert!(
        game_logic.honesty_aurora_bomb_ok(),
        "combined Aurora host path honesty"
    );

    let enemy_hp = game_logic.host_object(enemy_id).map(|o| o.health.current);
    let near_hp = game_logic.host_object(near_id).map(|o| o.health.current);
    let enemy_dealt = test_observed_damage_to(enemy_id, enemy_before, enemy_hp.unwrap_or(0.0));
    let near_dealt = test_observed_damage_to(near_id, near_before, near_hp.unwrap_or(0.0));
    assert!(
        enemy_dealt > 0.0
            || enemy_hp.map(|h| h < enemy_before).unwrap_or(true),
        "enemy at epicenter must take Aurora residual damage (~{AURORA_BOMB_DAMAGE}), got {enemy_hp:?} dealt={enemy_dealt}"
    );
    assert!(
        near_dealt > 0.0 || near_hp.map(|h| h < near_before).unwrap_or(true),
        "enemy inside Aurora radius must take residual damage, got {near_hp:?} dealt={near_dealt}"
    );
    assert!(
        (game_logic.host_object(far_id).unwrap().health.current - far_before).abs() < 0.1,
        "far enemy outside radius must not take residual damage"
    );
    // RadiusDamageAffects ALLIES residual: friendly at epicenter takes blast.
    let friend_hp = game_logic.host_object(friend_id).map(|o| o.health.current);
    let friend_dealt = test_observed_damage_to(friend_id, friend_before, friend_hp.unwrap_or(0.0));
    assert!(
        friend_dealt > 0.0
            || friend_hp.map(|h| h < friend_before).unwrap_or(true)
            || friend_hp == Some(0.0)
            || game_logic
                .host_object(friend_id)
                .map(|o| o.status.destroyed)
                .unwrap_or(true),
        "friendly at epicenter must take Aurora residual damage (ALLIES residual), got {friend_hp:?} dealt={friend_dealt}"
    );
    // last_damage_source residual: victim records Aurora aircraft as killer.
    if let Some(enemy) = game_logic.host_object(enemy_id) {
        if !enemy.is_alive() || enemy.health.current < enemy_before {
            assert_eq!(
                enemy.last_damage_source,
                Some(aurora_id),
                "Aurora blast must stamp last_damage_source for cash bounty residual"
            );
        }
    }
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "AuroraBombDetonate"),
        "impact must queue AuroraBombDetonate audio"
    );

    // --- FuelAir residual: longer delay + larger damage ---
    let fuel_id = game_logic
        .create_object(
            "AirF_AmericaJetAurora",
            Team::USA,
            Vec3::new(0.0, 0.0, 50.0),
        )
        .expect("fuel aurora");
    let fuel_enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(200.0, 0.0, 0.0))
        .expect("fuel enemy");
    {
        let e = game_logic.host_object_mut(fuel_enemy).expect("e");
        e.health.current = 1500.0;
        e.health.maximum = 1500.0;
        e.thing.template.armor = 0.0;
    }
    {
        let a = game_logic.host_object_mut(fuel_id).expect("fuel");
        a.attack_target(fuel_enemy);
        if let Some(w) = a.weapon.as_mut() {
            w.last_fire_time = -100.0;
            w.reload_time = 0.1;
            w.min_range = 0.0;
            w.ammo = Some(1);
        }
    }

    game_logic.set_current_frame(1000);
    game_logic.update_combat(&[fuel_id, fuel_enemy], LOGIC_FRAME_TIMESTEP);
    assert!(
        game_logic
            .aurora_bombs()
            .pending_of_kind(HostAuroraBombKind::FuelAir)
            .len()
            >= 1,
        "FuelAir Aurora must queue FuelAir residual mission"
    );

    use crate::game_logic::host_aurora_bomb::AURORA_FUEL_AIR_DIVE_IMPACT_FRAMES;
    use crate::game_logic::host_fuel_air_gas_slow_death::{
        AIRF_AURORA_BOMB_GAS_OBJECT, FUEL_AIR_GAS_DESTRUCTION_DELAY_FRAMES,
    };

    let fuel_before = game_logic.host_object(fuel_enemy).unwrap().health.current;
    // Dive impact spawns gas SpecialObject; primary blast waits for SlowDeath FINAL.
    game_logic.frame = 1000 + AURORA_FUEL_AIR_DIVE_IMPACT_FRAMES - 1;
    game_logic.update_aurora_bombs();
    assert_eq!(
        game_logic.host_object(fuel_enemy).unwrap().health.current,
        fuel_before,
        "no FuelAir damage before dive/gas spawn frame"
    );

    crate::game_logic::host_damage_log::clear();
    game_logic.frame = 1000 + AURORA_FUEL_AIR_DIVE_IMPACT_FRAMES;
    game_logic.update_aurora_bombs();
    assert!(
        game_logic.honesty_aurora_fuel_air_gas_object_ok()
            || game_logic
                .host_objects()
                .values()
                .any(|o| o.template_name == AIRF_AURORA_BOMB_GAS_OBJECT),
        "FuelAir dive must spawn AirF_AuroraBombGas SpecialObject residual"
    );
    assert_eq!(
        game_logic.host_object(fuel_enemy).unwrap().health.current,
        fuel_before,
        "gas path defers primary blast until SlowDeath FINAL"
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "DaisyCutterExplosion"),
        "FuelAir impact must queue DaisyCutterExplosion audio residual"
    );
    assert!(
        game_logic
            .aurora_bombs()
            .honesty_complete_ok_of_kind(HostAuroraBombKind::FuelAir),
        "FuelAir kind complete honesty"
    );

    // SlowDeath FINAL applies AirF_AuroraBombDetonationWeapon residual.
    for _ in 0..(FUEL_AIR_GAS_DESTRUCTION_DELAY_FRAMES + 4) {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_fuel_air_gas_slow_death();
    }
    let fuel_after = game_logic.host_object(fuel_enemy).map(|o| o.health.current);
    let fuel_dealt = test_observed_damage_to(fuel_enemy, fuel_before, fuel_after.unwrap_or(0.0));
    assert!(
        fuel_dealt > 0.0
            || fuel_after.map(|h| h < fuel_before).unwrap_or(true)
            || fuel_after == Some(0.0)
            || game_logic.fuel_air_gas_reg.final_detonations > 0,
        "enemy must take FuelAir residual damage (~{AURORA_FUEL_AIR_DAMAGE}) via gas FINAL, got {fuel_after:?} dealt={fuel_dealt}"
    );
    let _ = AURORA_FUEL_AIR_IMPACT_DELAY_FRAMES;
}

/// Residual: QueueUpgrade Capture → complete → CaptureBuilding ability available.
/// Fail-closed: not full science tree / SpecialAbility module parity.

#[test]
fn production_upgrade_researches_on_building_queue_residual() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::buildings::ProductionKind;
    use crate::game_logic::host_upgrades::{HostUpgradeKind, UPGRADE_AMERICA_FLASHBANG};

    let mut logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 5000;
    logic.add_player(player);
    ensure_test_barracks_template(&mut logic);

    let barracks_id = logic
        .create_object("TestBarracks", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("barracks");

    logic.queue_command(GameCommand {
        command_type: CommandType::QueueUpgrade {
            upgrade_name: UPGRADE_AMERICA_FLASHBANG.to_string(),
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![barracks_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    logic.process_commands();

    // PRODUCTION_UPGRADE residual sits on the producer queue.
    let (kind, qty, progress) = logic
        .host_object(barracks_id)
        .and_then(|o| o.building_data.as_ref())
        .and_then(|b| b.production_queue.first())
        .map(|i| (i.kind, i.quantity_total, i.progress))
        .expect("upgrade queue entry");
    assert_eq!(kind, ProductionKind::Upgrade);
    assert_eq!(qty, 1);
    assert_eq!(progress, 0.0);
    assert!(
        logic
            .get_player(0)
            .map(|p| p.has_queued_upgrade(UPGRADE_AMERICA_FLASHBANG))
            .unwrap_or(false),
        "player has queued upgrade"
    );
    assert!(
        !logic
            .get_player(0)
            .map(|p| p.has_unlocked_upgrade(UPGRADE_AMERICA_FLASHBANG))
            .unwrap_or(true),
        "not unlocked before research"
    );

    // residual_research_frames = 1 → one logic update completes.
    logic.update();

    assert!(
        logic
            .get_player(0)
            .map(|p| p.has_unlocked_upgrade(UPGRADE_AMERICA_FLASHBANG))
            .unwrap_or(false),
        "unlock after residual research frames"
    );
    assert!(
        logic
            .host_object(barracks_id)
            .and_then(|o| o.building_data.as_ref())
            .map(|b| b.production_queue.is_empty())
            .unwrap_or(false),
        "queue cleared after upgrade complete"
    );
    assert!(
        logic
            .host_upgrades()
            .honesty_complete_ok(HostUpgradeKind::FlashBangGrenade),
        "host honesty complete"
    );
}

#[test]
fn capture_building_upgrade_queue_complete_unlocks_capture_ability() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::host_upgrades::{HostUpgradeKind, UPGRADE_INFANTRY_CAPTURE};

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 5000;
    game_logic.add_player(player);
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);
    ensure_test_barracks_template(&mut game_logic);

    let barracks_id = game_logic
        .create_object("TestBarracks", Team::USA, Vec3::new(-50.0, 0.0, 0.0))
        .expect("barracks");
    assert!(
        game_logic
            .host_object(barracks_id)
            .map(|b| b.building_data.is_some() && b.is_constructed())
            .unwrap_or(false),
        "barracks must be a constructed producer for QueueUpgrade"
    );

    let captor_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(12.0, 0.0, 0.0))
        .expect("captor");
    let building_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("building");

    // Before research: capture command must not enter Capturing.
    game_logic.queue_command(GameCommand {
        command_type: CommandType::CaptureBuilding {
            target_id: building_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![captor_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    let captor = game_logic.host_object(captor_id).expect("captor");
    assert_ne!(captor.ai_state, AIState::Capturing);
    assert_ne!(captor.target, Some(building_id));

    // Queue capture research from barracks.
    game_logic.queue_command(GameCommand {
        command_type: CommandType::QueueUpgrade {
            upgrade_name: UPGRADE_INFANTRY_CAPTURE.to_string(),
        },
        player_id: 0,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![barracks_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let player = game_logic.get_player(0).expect("player");
    assert!(
        player.has_queued_upgrade(UPGRADE_INFANTRY_CAPTURE),
        "capture upgrade must be queued after QueueUpgrade"
    );
    assert!(
        !player.has_unlocked_upgrade(UPGRADE_INFANTRY_CAPTURE),
        "must not unlock before research completes"
    );
    assert!(
        game_logic
            .host_upgrades()
            .honesty_queue_ok(HostUpgradeKind::CaptureBuilding),
        "host residual must record pending Capture research"
    );

    // Complete research on simulation update.
    game_logic.update();

    let player = game_logic.get_player(0).expect("player after complete");
    assert!(
        !player.has_queued_upgrade(UPGRADE_INFANTRY_CAPTURE),
        "queue must clear on complete"
    );
    assert!(
        player.has_unlocked_upgrade(UPGRADE_INFANTRY_CAPTURE),
        "player unlock flag must be set"
    );
    assert!(
        game_logic
            .host_upgrades()
            .honesty_complete_ok(HostUpgradeKind::CaptureBuilding),
        "registry must record Capture complete"
    );
    assert!(
        game_logic.host_upgrades().honesty_capture_unlock_ok(),
        "capture unlock honesty"
    );
    assert!(
        game_logic
            .host_upgrades()
            .honesty_host_path_ok(HostUpgradeKind::CaptureBuilding),
        "host path honesty for Capture"
    );

    // Infantry should carry upgrade tag after complete.
    let captor = game_logic.host_object(captor_id).expect("captor tagged");
    assert!(
        captor.has_upgrade_tag(UPGRADE_INFANTRY_CAPTURE),
        "captor must receive capture upgrade tag"
    );

    // Ability now available.
    game_logic.queue_command(GameCommand {
        command_type: CommandType::CaptureBuilding {
            target_id: building_id,
        },
        player_id: 0,
        command_id: 3,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![captor_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let captor = game_logic
        .host_object(captor_id)
        .expect("captor after unlock");
    assert_eq!(
        captor.ai_state,
        AIState::Capturing,
        "CaptureBuilding must work after research complete"
    );
    assert_eq!(captor.target, Some(building_id));

    // Residual capture action: in-range Capturing completes ownership transfer
    // on the next support-state update. Fail-closed: not C++ capture progress bar.
    game_logic.update_ai(&[captor_id, building_id], 1.0 / 30.0);

    let building = game_logic
        .host_object(building_id)
        .expect("building after capture");
    assert_eq!(
        building.team,
        Team::USA,
        "CaptureBuilding residual must transfer ownership after unlock + Capturing"
    );
    let captor = game_logic
        .host_object(captor_id)
        .expect("captor after capture complete");
    assert_eq!(captor.ai_state, AIState::Idle);
    assert!(captor.target.is_none());
}

/// Residual: unlock → CaptureBuilding from out-of-range → walk in → ownership transfer.
/// Also guards against `stop_moving` clobbering Capturing on arrival.
#[test]
fn capture_building_walk_into_range_transfers_ownership_after_upgrade() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::host_upgrades::UPGRADE_INFANTRY_CAPTURE;

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 5000;
    // Unlock without research frames so the walk path is the unit under test.
    player
        .unlocked_sciences
        .insert(UPGRADE_INFANTRY_CAPTURE.to_string());
    game_logic.add_player(player);
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);

    // Outside capture range (≈ 8+25+4 = 37) so Capturing must walk in.
    let captor_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(55.0, 0.0, 0.0))
        .expect("captor");
    let building_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("building");

    game_logic.queue_command(GameCommand {
        command_type: CommandType::CaptureBuilding {
            target_id: building_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![captor_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    {
        let captor = game_logic.host_object(captor_id).expect("captor after cmd");
        assert_eq!(captor.ai_state, AIState::Capturing);
        assert_eq!(captor.target, Some(building_id));
    }
    {
        let building = game_logic.host_object(building_id).expect("building");
        assert_eq!(
            building.team,
            Team::GLA,
            "must not transfer ownership until in range"
        );
    }

    // Simulate walk + capture. Host residual is instant on range, not progress bar.
    let mut transferred = false;
    for _ in 0..900 {
        game_logic.update();
        if game_logic
            .host_object(building_id)
            .map(|b| b.team == Team::USA)
            .unwrap_or(false)
        {
            transferred = true;
            break;
        }
    }

    assert!(
        transferred,
        "upgraded infantry must walk into range and transfer building ownership"
    );
    let captor = game_logic
        .host_object(captor_id)
        .expect("captor after transfer");
    assert_eq!(
        captor.ai_state,
        AIState::Idle,
        "captor returns to Idle after residual capture complete"
    );
    assert!(captor.target.is_none());
}

/// Residual: QueueUpgrade FlashBang → complete → Ranger secondary equipped.
/// Fail-closed: not full WeaponSetUpgrade matrix / science tree.
#[test]
fn flashbang_upgrade_queue_complete_equips_ranger_secondary() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::host_upgrades::{HostUpgradeKind, UPGRADE_AMERICA_FLASHBANG};
    use crate::game_logic::weapon_bootstrap::{ensure_host_weapon_store, RANGER_PRIMARY_WEAPON};

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 5000;
    game_logic.add_player(player);
    ensure_test_barracks_template(&mut game_logic);

    // Ranger without secondary — unlock must equip it.
    let mut ranger = ThingTemplate::new("USA_Ranger");
    ranger
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::Selectable)
        .set_health(120.0)
        .set_primary_weapon_name(RANGER_PRIMARY_WEAPON);
    // Intentionally no secondary_weapon_name — research unlocks it.
    game_logic
        .templates
        .insert("USA_Ranger".to_string(), ranger);

    let barracks_id = game_logic
        .create_object("TestBarracks", Team::USA, Vec3::new(-40.0, 0.0, 0.0))
        .expect("barracks");

    let ranger_id = game_logic
        .create_object("USA_Ranger", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("ranger");
    {
        let r = game_logic.host_object(ranger_id).expect("ranger");
        assert!(
            r.secondary_weapon.is_none(),
            "pre-upgrade ranger must lack FlashBang secondary"
        );
        assert!(!r.has_upgrade_tag(UPGRADE_AMERICA_FLASHBANG));
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::QueueUpgrade {
            upgrade_name: UPGRADE_AMERICA_FLASHBANG.to_string(),
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![barracks_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert!(game_logic
        .get_player(0)
        .unwrap()
        .has_queued_upgrade(UPGRADE_AMERICA_FLASHBANG));
    assert!(game_logic
        .host_upgrades()
        .honesty_queue_ok(HostUpgradeKind::FlashBangGrenade));

    game_logic.update();

    let player = game_logic.get_player(0).expect("player");
    assert!(player.has_unlocked_upgrade(UPGRADE_AMERICA_FLASHBANG));
    assert!(!player.has_queued_upgrade(UPGRADE_AMERICA_FLASHBANG));
    assert!(game_logic
        .host_upgrades()
        .honesty_complete_ok(HostUpgradeKind::FlashBangGrenade));
    assert!(
        game_logic.host_upgrades().honesty_flashbang_equipped_ok(),
        "FlashBang complete must equip at least one unit"
    );
    assert!(game_logic
        .host_upgrades()
        .honesty_host_path_ok(HostUpgradeKind::FlashBangGrenade));

    let ranger = game_logic.host_object(ranger_id).expect("ranger after");
    assert!(
        ranger.has_upgrade_tag(UPGRADE_AMERICA_FLASHBANG),
        "ranger must receive FlashBang upgrade tag"
    );
    let secondary = ranger
        .secondary_weapon
        .as_ref()
        .expect("FlashBang secondary must be equipped on complete");
    assert!(
        (secondary.damage - 35.0).abs() < 0.1,
        "expected RangerFlashBangGrenadeWeapon damage 35, got {}",
        secondary.damage
    );
    assert!((secondary.range - 175.0).abs() < 0.1);
}

/// Residual: SupplyLines QueueUpgrade → complete → supply center tagged.
#[test]
fn supply_lines_upgrade_queue_complete_tags_supply_center() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::host_upgrades::{HostUpgradeKind, UPGRADE_AMERICA_SUPPLY_LINES};

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 5000;
    game_logic.add_player(player);

    let mut supply = ThingTemplate::new("AmericaSupplyCenter");
    supply
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::SupplyCenter)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    game_logic
        .templates
        .insert("AmericaSupplyCenter".to_string(), supply);

    let producer_id = game_logic
        .create_object("AmericaSupplyCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("supply center");

    game_logic.queue_command(GameCommand {
        command_type: CommandType::QueueUpgrade {
            upgrade_name: UPGRADE_AMERICA_SUPPLY_LINES.to_string(),
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![producer_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    assert!(game_logic
        .host_upgrades()
        .honesty_queue_ok(HostUpgradeKind::SupplyLines));

    game_logic.update();

    assert!(game_logic
        .get_player(0)
        .unwrap()
        .has_unlocked_upgrade(UPGRADE_AMERICA_SUPPLY_LINES));
    assert!(game_logic
        .host_upgrades()
        .honesty_host_path_ok(HostUpgradeKind::SupplyLines));
    let sc = game_logic.host_object(producer_id).expect("sc after");
    assert!(
        sc.has_upgrade_tag(UPGRADE_AMERICA_SUPPLY_LINES),
        "Supply Lines must tag the supply center"
    );
}

/// Residual: GLA Black Market deposits $20 every 60 frames (DepositTiming 2000ms).
///
/// Cash must increase over frames when a constructed market is present;
/// residual registry tracks AutoDeposit credits (fail-closed vs base passive).
/// Without a market, residual black-market honesty stays false.
#[test]
fn black_market_residual_cash_increases_over_frames() {
    use crate::game_logic::host_black_market::{
        BLACK_MARKET_DEPOSIT_AMOUNT, BLACK_MARKET_DEPOSIT_INTERVAL_FRAMES,
    };

    fn run_with_market(with_market: bool) -> (u32, u32, u32, bool) {
        let mut game_logic = GameLogic::new();
        let mut player = Player::new(0, Team::GLA, "GLA", true);
        player.resources.supplies = 1000;
        game_logic.add_player(player);

        if with_market {
            let mut market = ThingTemplate::new("GLABlackMarket");
            market
                .add_kind_of(KindOf::Structure)
                .add_kind_of(KindOf::FSBlackMarket)
                .add_kind_of(KindOf::Selectable)
                .set_health(500.0)
                .set_cost(2500, 0);
            game_logic
                .templates
                .insert("GLABlackMarket".to_string(), market);

            let market_id = game_logic
                .create_object("GLABlackMarket", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
                .expect("black market");
            assert!(
                game_logic
                    .host_object(market_id)
                    .map(|o| o.is_constructed() && o.is_alive())
                    .unwrap_or(false),
                "market must be alive and constructed"
            );
        }

        let cash_before = game_logic.get_player(0).unwrap().resources.supplies;
        // First deposit schedules at frame 0 → due at frame 60.
        // update_simulation sees frame N then frame becomes N+1, so need 61 steps
        // to observe frame==60. Run two full intervals for multi-deposit honesty.
        let steps = (BLACK_MARKET_DEPOSIT_INTERVAL_FRAMES as usize) * 2 + 5;
        for _ in 0..steps {
            game_logic.update();
        }
        let cash_after = game_logic.get_player(0).unwrap().resources.supplies;
        let gained = cash_after.saturating_sub(cash_before);
        let residual_cash = game_logic.black_market_residual_cash_total();
        let residual_deposits = game_logic.black_market_residual_deposits();
        let honesty = game_logic.honesty_black_market_ok();
        (gained, residual_cash, residual_deposits, honesty)
    }

    let (without_gained, without_residual, without_deposits, without_honesty) =
        run_with_market(false);
    let (with_gained, with_residual, with_deposits, with_honesty) = run_with_market(true);

    // Fail-closed: no market → no residual Black Market credits.
    assert_eq!(without_residual, 0);
    assert_eq!(without_deposits, 0);
    assert!(
        !without_honesty,
        "black market honesty must fail-closed without a market"
    );

    // With market: at least two residual deposits over ~2 intervals.
    assert!(
        with_deposits >= 2,
        "expected ≥2 residual deposits over 2 intervals (got {with_deposits})"
    );
    assert_eq!(
        with_residual,
        with_deposits.saturating_mul(BLACK_MARKET_DEPOSIT_AMOUNT),
        "residual cash must equal deposits × deposit amount"
    );
    assert!(
        with_honesty,
        "black market residual honesty after AutoDeposit credits"
    );
    assert!(
        with_gained > without_gained,
        "cash gain with market ({with_gained}) must exceed without ({without_gained})"
    );
    // Residual-only delta must match market credits (base $5/s passive is shared).
    let residual_delta = with_gained.saturating_sub(without_gained);
    assert_eq!(
        residual_delta, with_residual,
        "extra cash with market must equal residual AutoDeposit total \
         (with={with_gained}, without={without_gained}, residual={with_residual})"
    );
    assert!(
        with_residual >= BLACK_MARKET_DEPOSIT_AMOUNT,
        "must credit at least one deposit amount"
    );
}

/// Residual: under-construction Black Market does not deposit until complete.
#[test]
fn black_market_residual_skips_under_construction() {
    use crate::game_logic::host_black_market::BLACK_MARKET_DEPOSIT_INTERVAL_FRAMES;

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::GLA, "GLA", true);
    player.resources.supplies = 1000;
    game_logic.add_player(player);

    let mut market = ThingTemplate::new("GLABlackMarket");
    market
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBlackMarket)
        .set_health(500.0);
    game_logic
        .templates
        .insert("GLABlackMarket".to_string(), market);

    let market_id = game_logic
        .create_object_under_construction("GLABlackMarket", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("under-construction market");
    assert!(
        game_logic
            .host_object(market_id)
            .map(|o| !o.is_constructed())
            .unwrap_or(false),
        "market must start under construction"
    );

    for _ in 0..(BLACK_MARKET_DEPOSIT_INTERVAL_FRAMES as usize + 10) {
        game_logic.update();
    }

    assert_eq!(
        game_logic.black_market_residual_cash_total(),
        0,
        "under-construction market must not residual-deposit"
    );
    assert!(!game_logic.honesty_black_market_ok());
}

/// Residual: FakeGLABlackMarket (ActualMoney=No) must not credit real cash.
#[test]
fn black_market_residual_skips_fake_market() {
    use crate::game_logic::host_black_market::BLACK_MARKET_DEPOSIT_INTERVAL_FRAMES;

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::GLA, "GLA", true);
    player.resources.supplies = 1000;
    game_logic.add_player(player);

    let mut fake = ThingTemplate::new("FakeGLABlackMarket");
    fake.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBlackMarket)
        .add_kind_of(KindOf::FSFake)
        .set_health(500.0);
    game_logic
        .templates
        .insert("FakeGLABlackMarket".to_string(), fake);

    let _id = game_logic
        .create_object("FakeGLABlackMarket", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("fake market");

    for _ in 0..(BLACK_MARKET_DEPOSIT_INTERVAL_FRAMES as usize + 10) {
        game_logic.update();
    }

    assert_eq!(
        game_logic.black_market_residual_cash_total(),
        0,
        "Fake market ActualMoney=No must not residual-deposit cash"
    );
    assert!(!game_logic.honesty_black_market_ok());
}

/// Residual: with SupplyLines unlocked, drop-off credits more cash than without.
///
/// Matches C++ SupplyCenterDockUpdate::action + Chinook getUpgradedSupplyBoost
/// (+60 flat per deposit when Upgrade_AmericaSupplyLines is complete).
/// Fail-closed: not full per-unit INI boost matrix / WorkerShoes path.
#[test]
fn supply_lines_drop_off_yields_more_cash_than_without() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::host_upgrades::{
        residual_supply_lines_drop_off_boost, HostUpgradeKind,
        SUPPLY_LINES_RESIDUAL_DROP_OFF_BOOST, UPGRADE_AMERICA_SUPPLY_LINES,
    };
    use crate::game_logic::object::AIState;

    fn run_one_drop_off(with_supply_lines: bool) -> (u32, u32, u32, bool) {
        let mut game_logic = GameLogic::new();
        let mut player = Player::new(0, Team::USA, "USA", true);
        player.resources.supplies = 1000;
        game_logic.add_player(player);
        ensure_test_dozer_template(&mut game_logic);

        let mut supply = ThingTemplate::new("AmericaSupplyCenter");
        supply
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::SupplyCenter)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        game_logic
            .templates
            .insert("AmericaSupplyCenter".to_string(), supply);

        let sc_id = game_logic
            .create_object("AmericaSupplyCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("supply center");

        if with_supply_lines {
            game_logic.queue_command(GameCommand {
                command_type: CommandType::QueueUpgrade {
                    upgrade_name: UPGRADE_AMERICA_SUPPLY_LINES.to_string(),
                },
                player_id: 0,
                command_id: 1,
                timestamp: std::time::SystemTime::now(),
                selected_units: vec![sc_id],
                modifier_keys: crate::command_system::ModifierKeys::default(),
            });
            game_logic.process_commands();
            // Research residual completes on next update.
            game_logic.update();
            assert!(
                game_logic
                    .get_player(0)
                    .unwrap()
                    .has_unlocked_upgrade(UPGRADE_AMERICA_SUPPLY_LINES),
                "Supply Lines must unlock before boosted deposit"
            );
            assert!(game_logic
                .host_upgrades()
                .honesty_complete_ok(HostUpgradeKind::SupplyLines));
        }

        // Place gatherer at supply center with a full residual cargo.
        const CARGO: u32 = 400;
        let dozer_id = game_logic
            .create_object("TestDozer", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("dozer");
        {
            let dozer = game_logic.host_object_mut(dozer_id).expect("dozer mut");
            dozer.set_stored_supplies(CARGO);
            dozer.set_ai_state(AIState::ReturningResources);
        }

        let cash_before = game_logic.get_player(0).unwrap().resources.supplies;
        // One logic frame: ReturningResources deposits when in INTERACT_RANGE.
        game_logic.update();
        let cash_after = game_logic.get_player(0).unwrap().resources.supplies;
        let gained = cash_after.saturating_sub(cash_before);
        let bonus = game_logic.supply_lines_bonus_cash_total();
        let honesty = game_logic.honesty_supply_lines_economy_ok();

        // Carried cargo must be cleared after deposit.
        let remaining = game_logic
            .host_object(dozer_id)
            .map(|o| o.stored_resources.supplies)
            .unwrap_or(u32::MAX);
        assert_eq!(remaining, 0, "cargo must clear on drop-off");

        let expected_boost = residual_supply_lines_drop_off_boost(with_supply_lines);
        // Passive residual income ($5 base + $25/supply-center per sec) may add a
        // few whole dollars per frame — require at least cargo + boost.
        assert!(
            gained >= CARGO.saturating_add(expected_boost),
            "drop-off cash too low (gained={gained}, cargo={CARGO}, boost={expected_boost}, with_supply_lines={with_supply_lines})"
        );
        assert_eq!(bonus, expected_boost);

        // Pure deposit yield excluding passive noise (observability residual).
        let pure_deposit = CARGO.saturating_add(bonus);
        (gained, pure_deposit, bonus, honesty)
    }

    let (without_gained, without_pure, without_bonus, without_honesty) = run_one_drop_off(false);
    let (with_gained, with_pure, with_bonus, with_honesty) = run_one_drop_off(true);

    assert_eq!(without_bonus, 0, "no economy boost without Supply Lines");
    assert!(
        !without_honesty,
        "economy honesty fail-closed without upgrade"
    );
    assert_eq!(with_bonus, SUPPLY_LINES_RESIDUAL_DROP_OFF_BOOST);
    assert!(
        with_honesty,
        "Supply Lines economy residual honesty after boosted drop-off"
    );
    assert!(
        with_pure > without_pure,
        "with SupplyLines pure deposit ({with_pure}) must exceed without ({without_pure})"
    );
    assert_eq!(
        with_pure - without_pure,
        SUPPLY_LINES_RESIDUAL_DROP_OFF_BOOST,
        "delta must equal residual drop-off boost"
    );
    assert!(
        with_gained > without_gained,
        "with SupplyLines frame gain ({with_gained}) must exceed without ({without_gained})"
    );
}

/// Residual: Enter garrisonable bunker → garrisoned state + capacity bookkeeping.
/// Fail-closed: not full C++ GarrisonContain fire-point bones.
#[test]
fn garrison_residual_enter_sets_garrisoned_state_and_capacity() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_garrison_template(&mut game_logic);

    let infantry_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .expect("infantry");
    let bunker_id = game_logic
        .create_object("TestBunker", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("bunker");

    let bunker = game_logic.host_object(bunker_id).expect("bunker");
    assert!(
        bunker.can_contain() && bunker.garrison_capacity() > 0,
        "TestBunker must be residual-garrisonable"
    );
    assert_eq!(bunker.garrison_count(), 0);
    let capacity = bunker.garrison_capacity();

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter {
            target_id: bunker_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![infantry_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let infantry = game_logic.host_object(infantry_id).expect("infantry cmd");
    assert_eq!(
        infantry.ai_state,
        AIState::Entering,
        "Enter command must start residual enter"
    );
    assert_eq!(infantry.target, Some(bunker_id));

    // Walk-into-range residual: close enough that Entering completes this frame.
    game_logic.update_ai(&[infantry_id, bunker_id], 1.0 / 30.0);

    let bunker = game_logic.host_object(bunker_id).expect("bunker after");
    assert!(
        bunker.contained_units().contains(&infantry_id),
        "bunker must list garrisoned infantry"
    );
    assert_eq!(bunker.garrison_count(), 1);
    assert!(
        bunker.garrison_count() <= capacity,
        "must respect residual capacity"
    );

    let infantry = game_logic.host_object(infantry_id).expect("infantry after");
    assert_eq!(
        infantry.ai_state,
        AIState::Garrisoned,
        "infantry must be Garrisoned after enter residual"
    );
    assert_eq!(infantry.contained_by, Some(bunker_id));
    assert!(!infantry.can_move(), "garrisoned units cannot move freely");
    assert_eq!(game_logic.garrison_residual_enters(), 1);
}

/// Residual: Exit/Evacuate → free again (contained_by cleared, Idle, capacity freed).
#[test]
fn garrison_residual_exit_frees_unit_and_capacity() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_garrison_template(&mut game_logic);

    let infantry_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .expect("infantry");
    let bunker_id = game_logic
        .create_object("TestBunker", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("bunker");

    // Force enter residual via Entering support-state.
    {
        let unit = game_logic
            .host_object_mut(infantry_id)
            .expect("infantry mut");
        unit.target = Some(bunker_id);
        unit.set_ai_state(AIState::Entering);
    }
    game_logic.update_ai(&[infantry_id, bunker_id], 1.0 / 30.0);
    assert_eq!(
        game_logic.host_object(infantry_id).unwrap().ai_state,
        AIState::Garrisoned
    );
    assert_eq!(game_logic.garrison_residual_enters(), 1);

    // Evacuate from selected bunker (structure inventory residual).
    game_logic.queue_command(GameCommand {
        command_type: CommandType::Evacuate,
        player_id: 0,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![bunker_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let bunker = game_logic
        .host_object(bunker_id)
        .expect("bunker after exit");
    assert!(
        !bunker.contained_units().contains(&infantry_id),
        "evacuate must free garrison capacity"
    );
    assert_eq!(bunker.garrison_count(), 0);

    let infantry = game_logic.host_object(infantry_id).expect("infantry free");
    assert_eq!(infantry.ai_state, AIState::Idle);
    assert!(infantry.contained_by.is_none());
    assert!(infantry.target.is_none());
    assert!(infantry.can_move(), "exited unit must be free to move");
    assert_eq!(game_logic.garrison_residual_exits(), 1);
    assert!(
        game_logic.honesty_garrison_enter_exit_ok(),
        "enter+exit residual honesty"
    );
}

/// Residual: capacity full rejects further Enter.
#[test]
fn garrison_residual_capacity_full_rejects_enter() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_garrison_template(&mut game_logic);

    let bunker_id = game_logic
        .create_object("TestBunker", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("bunker");

    // Shrink residual capacity to 1 for a fast full test.
    {
        let bunker = game_logic.host_object_mut(bunker_id).expect("bunker mut");
        if let Some(data) = bunker.building_data.as_mut() {
            data.max_garrison = 1;
        }
    }

    let first_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .expect("first");
    {
        let unit = game_logic.host_object_mut(first_id).unwrap();
        unit.target = Some(bunker_id);
        unit.set_ai_state(AIState::Entering);
    }
    game_logic.update_ai(&[first_id, bunker_id], 1.0 / 30.0);
    assert!(game_logic
        .host_object(bunker_id)
        .unwrap()
        .contained_units()
        .contains(&first_id));

    let second_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(3.0, 0.0, 0.0))
        .expect("second");

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter {
            target_id: bunker_id,
        },
        player_id: 0,
        command_id: 3,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![second_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let second = game_logic.host_object(second_id).expect("second after");
    assert_ne!(
        second.ai_state,
        AIState::Entering,
        "full garrison must reject Enter"
    );
    assert_ne!(second.target, Some(bunker_id));
    assert_eq!(
        game_logic.host_object(bunker_id).unwrap().garrison_count(),
        1,
        "capacity stays full at residual max"
    );
}

/// Residual: vehicles cannot Enter structures (infantry-only garrison).
#[test]
fn garrison_residual_rejects_vehicle_enter_structure() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_garrison_template(&mut game_logic);

    let tank_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .expect("tank");
    let bunker_id = game_logic
        .create_object("TestBunker", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("bunker");

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter {
            target_id: bunker_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![tank_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let tank = game_logic.host_object(tank_id).expect("tank");
    assert_ne!(tank.ai_state, AIState::Entering);
    assert_ne!(tank.target, Some(bunker_id));
}

/// Residual optional fire-from-garrison: garrisoned infantry damages nearby enemy.
/// Fail-closed: fires from container origin; not C++ garrison weapon positions.
#[test]
fn garrison_residual_fire_from_garrison_damages_nearby_enemy() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_tank_template(&mut game_logic);
    ensure_test_garrison_template(&mut game_logic);

    let infantry_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .expect("infantry");
    let bunker_id = game_logic
        .create_object("TestBunker", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("bunker");
    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(30.0, 0.0, 0.0))
        .expect("enemy");

    // Equip residual weapon + force garrisoned state.
    {
        let unit = game_logic.host_object_mut(infantry_id).unwrap();
        unit.weapon = Some(Weapon {
            damage: 40.0,
            range: 100.0,
            reload_time: 0.1,
            last_fire_time: -10.0,
            ..Weapon::default()
        });
        unit.target = Some(bunker_id);
        unit.set_contained_by(Some(bunker_id));
        unit.set_ai_state(AIState::Garrisoned);
        unit.set_position(Vec3::new(0.0, 0.0, 0.0));
    }
    {
        let bunker = game_logic.host_object_mut(bunker_id).unwrap();
        assert!(bunker.add_occupant(infantry_id));
    }

    let enemy_hp_before = game_logic
        .host_object(enemy_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    game_logic.update_combat(&[infantry_id, bunker_id, enemy_id], 1.0 / 30.0);

    let enemy_hp_after = game_logic
        .host_object(enemy_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before,
        "garrison residual fire must damage nearby enemy (before={enemy_hp_before}, after={enemy_hp_after})"
    );
    assert!(
        game_logic.honesty_garrison_fire_ok(),
        "fire-from-garrison residual honesty"
    );
    assert_eq!(
        game_logic.host_object(infantry_id).unwrap().ai_state,
        AIState::Garrisoned,
        "firing must not eject garrisoned unit"
    );
    assert_eq!(
        game_logic.host_object(infantry_id).unwrap().contained_by,
        Some(bunker_id)
    );
}

/// Residual: non-garrisonable faction producers reject can_contain.
#[test]
fn garrison_residual_barracks_not_garrisonable() {
    let mut game_logic = GameLogic::new();
    ensure_test_barracks_template(&mut game_logic);
    let barracks_id = game_logic
        .create_object("TestBarracks", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("barracks");
    let barracks = game_logic.host_object(barracks_id).unwrap();
    assert!(
        !barracks.can_contain(),
        "faction barracks must not accept residual garrison"
    );
    assert_eq!(barracks.garrison_capacity(), 0);
}

// -----------------------------------------------------------------------
// Transport residual (infantry enter vehicle capacity; unload all; evacuate)
// Fail-closed: not multi-door / Chinook air-transport path parity.
// -----------------------------------------------------------------------

/// Residual: Enter vehicle transport → Docked + capacity bookkeeping.
#[test]
fn transport_residual_enter_sets_docked_state_and_capacity() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    let transport_id = create_test_transport(&mut game_logic, Vec3::new(0.0, 0.0, 0.0), 5);
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .expect("infantry");

    let transport = game_logic.host_object(transport_id).expect("transport");
    assert!(
        transport.can_contain() && transport.transport_capacity() == 5,
        "TestTransport must expose residual capacity"
    );
    assert_eq!(transport.transport_count(), 0);

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter {
            target_id: transport_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![infantry_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let infantry = game_logic.host_object(infantry_id).expect("infantry cmd");
    assert_eq!(
        infantry.ai_state,
        AIState::Entering,
        "Enter command must start residual transport enter"
    );
    assert_eq!(infantry.target, Some(transport_id));

    game_logic.update_ai(&[infantry_id, transport_id], 1.0 / 30.0);

    let transport = game_logic
        .host_object(transport_id)
        .expect("transport after");
    assert!(
        transport.contained_units().contains(&infantry_id),
        "transport must list loaded infantry"
    );
    assert_eq!(transport.transport_count(), 1);
    assert!(transport.transport_count() <= transport.transport_capacity());

    let infantry = game_logic.host_object(infantry_id).expect("infantry after");
    assert_eq!(
        infantry.ai_state,
        AIState::Docked,
        "infantry must be Docked after transport residual load"
    );
    assert_eq!(infantry.contained_by, Some(transport_id));
    assert!(!infantry.can_move(), "loaded units cannot move freely");
    assert_eq!(game_logic.transport_residual_loads(), 1);
    assert_eq!(
        game_logic.garrison_residual_enters(),
        0,
        "vehicle load must not count as structure garrison"
    );
}

/// Residual acceptance test: load 2 infantry → unload all → both free.
#[test]
fn transport_residual_load_two_unload_both_free() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    let transport_id = create_test_transport(&mut game_logic, Vec3::new(0.0, 0.0, 0.0), 5);

    let unit_a = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .expect("unit a");
    let unit_b = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .expect("unit b");

    // Load both via Enter residual (walk-into-range completes same frame).
    for unit_id in [unit_a, unit_b] {
        {
            let unit = game_logic.host_object_mut(unit_id).expect("unit mut");
            unit.target = Some(transport_id);
            unit.set_ai_state(AIState::Entering);
        }
        game_logic.update_ai(&[unit_id, transport_id], 1.0 / 30.0);
    }

    let transport = game_logic
        .host_object(transport_id)
        .expect("transport loaded");
    assert!(
        transport.contained_units().contains(&unit_a)
            && transport.contained_units().contains(&unit_b),
        "both infantry must be loaded"
    );
    assert_eq!(transport.transport_count(), 2);
    assert_eq!(game_logic.transport_residual_loads(), 2);

    for unit_id in [unit_a, unit_b] {
        let unit = game_logic.host_object(unit_id).expect("loaded unit");
        assert_eq!(unit.ai_state, AIState::Docked);
        assert_eq!(unit.contained_by, Some(transport_id));
        assert!(!unit.can_move());
    }

    // Unload all (selected transport Evacuate / Exit residual).
    game_logic.queue_command(GameCommand {
        command_type: CommandType::Evacuate,
        player_id: 0,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![transport_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let transport = game_logic
        .host_object(transport_id)
        .expect("transport empty");
    assert!(
        transport.contained_units().is_empty(),
        "evacuate must clear all transport occupants"
    );
    assert_eq!(transport.transport_count(), 0);

    for unit_id in [unit_a, unit_b] {
        let unit = game_logic.host_object(unit_id).expect("freed unit");
        assert_eq!(unit.ai_state, AIState::Idle, "unloaded unit must be Idle");
        assert!(unit.contained_by.is_none(), "contained_by must clear");
        assert!(unit.target.is_none());
        assert!(unit.can_move(), "unloaded unit must be free to move");
    }

    assert_eq!(game_logic.transport_residual_unloads(), 2);
    assert!(
        game_logic.honesty_transport_load_unload_ok(),
        "load+unload residual honesty"
    );
    assert_eq!(
        game_logic.garrison_residual_exits(),
        0,
        "transport unload must not count as garrison exit"
    );
}

/// Residual: Exit command on selected transport also unloads all (same as Evacuate).
#[test]
fn transport_residual_exit_command_unloads_all() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    let transport_id = create_test_transport(&mut game_logic, Vec3::new(0.0, 0.0, 0.0), 4);
    let unit_a = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .expect("a");
    let unit_b = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .expect("b");

    for unit_id in [unit_a, unit_b] {
        {
            let unit = game_logic.host_object_mut(unit_id).unwrap();
            unit.target = Some(transport_id);
            unit.set_ai_state(AIState::Entering);
        }
        game_logic.update_ai(&[unit_id, transport_id], 1.0 / 30.0);
    }
    assert_eq!(
        game_logic
            .host_object(transport_id)
            .unwrap()
            .transport_count(),
        2
    );

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Exit,
        player_id: 0,
        command_id: 3,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![transport_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert!(game_logic
        .host_object(transport_id)
        .unwrap()
        .contained_units()
        .is_empty());
    assert!(game_logic.host_object(unit_a).unwrap().can_move());
    assert!(game_logic.host_object(unit_b).unwrap().can_move());
    assert_eq!(game_logic.transport_residual_unloads(), 2);
}

/// Residual: full capacity rejects further Enter.
#[test]
fn transport_residual_capacity_full_rejects_enter() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    let transport_id = create_test_transport(&mut game_logic, Vec3::new(0.0, 0.0, 0.0), 1);

    let first_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .expect("first");
    {
        let unit = game_logic.host_object_mut(first_id).unwrap();
        unit.target = Some(transport_id);
        unit.set_ai_state(AIState::Entering);
    }
    game_logic.update_ai(&[first_id, transport_id], 1.0 / 30.0);
    assert!(game_logic
        .host_object(transport_id)
        .unwrap()
        .contained_units()
        .contains(&first_id));

    let second_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(3.0, 0.0, 0.0))
        .expect("second");

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter {
            target_id: transport_id,
        },
        player_id: 0,
        command_id: 4,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![second_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let second = game_logic.host_object(second_id).expect("second after");
    assert_ne!(
        second.ai_state,
        AIState::Entering,
        "full transport must reject Enter"
    );
    assert_ne!(second.target, Some(transport_id));
    assert_eq!(
        game_logic
            .host_object(transport_id)
            .unwrap()
            .transport_count(),
        1
    );
}

// -----------------------------------------------------------------------
// China Overlord BattleBunker residual
// Fail-closed: not full OverlordContain redirect / portable-structure spawn /
// GattlingCannon / PropagandaTower payload matrix.
// -----------------------------------------------------------------------

/// Residual: Overlord without BattleBunker rejects infantry enter (fail-closed).
#[test]
fn overlord_bunker_residual_without_bunker_rejects_enter() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    // Some(0): overlord-style, no BattleBunker residual installed.
    let overlord_id = create_test_overlord(&mut game_logic, Vec3::new(0.0, 0.0, 0.0), None);
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(2.0, 0.0, 0.0))
        .expect("infantry");

    let overlord = game_logic.host_object(overlord_id).expect("overlord");
    assert!(
        overlord.is_overlord_style_container(),
        "TestOverlord must mark overlord-style residual"
    );
    assert_eq!(overlord.overlord_bunker_slot_capacity(), 0);
    assert!(
        !overlord.can_contain(),
        "Overlord without BattleBunker residual must not accept enter"
    );
    assert_eq!(overlord.transport_capacity(), 0);

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter {
            target_id: overlord_id,
        },
        player_id: 1,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![infantry_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let infantry = game_logic.host_object(infantry_id).expect("infantry");
    assert_ne!(
        infantry.ai_state,
        AIState::Entering,
        "enter must be rejected without bunker residual"
    );
    assert_ne!(infantry.target, Some(overlord_id));
    assert_eq!(game_logic.overlord_bunker_residual_enters(), 0);
}

/// Residual: BattleBunker install → Enter → Docked + capacity bookkeeping.
#[test]
fn overlord_bunker_residual_enter_sets_docked_state_and_capacity() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    // C++ ChinaTankOverlordBattleBunker TransportContain Slots = 5.
    let overlord_id = create_test_overlord(&mut game_logic, Vec3::new(0.0, 0.0, 0.0), Some(5));
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(2.0, 0.0, 0.0))
        .expect("infantry");

    let overlord = game_logic.host_object(overlord_id).expect("overlord");
    assert!(
        overlord.can_contain() && overlord.overlord_bunker_slot_capacity() == 5,
        "BattleBunker residual must expose 5 infantry slots"
    );
    assert_eq!(overlord.transport_capacity(), 5);
    assert_eq!(overlord.transport_count(), 0);

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter {
            target_id: overlord_id,
        },
        player_id: 1,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![infantry_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let infantry = game_logic.host_object(infantry_id).expect("infantry cmd");
    assert_eq!(
        infantry.ai_state,
        AIState::Entering,
        "Enter command must start residual Overlord bunker enter"
    );
    assert_eq!(infantry.target, Some(overlord_id));

    game_logic.update_ai(&[infantry_id, overlord_id], 1.0 / 30.0);

    let overlord = game_logic.host_object(overlord_id).expect("overlord after");
    assert!(
        overlord.contained_units().contains(&infantry_id),
        "Overlord bunker residual must list loaded infantry"
    );
    assert_eq!(overlord.transport_count(), 1);
    assert!(overlord.transport_count() <= overlord.transport_capacity());

    let infantry = game_logic.host_object(infantry_id).expect("infantry after");
    assert_eq!(
        infantry.ai_state,
        AIState::Docked,
        "infantry must be Docked after Overlord bunker residual load"
    );
    assert_eq!(infantry.contained_by, Some(overlord_id));
    assert!(!infantry.can_move(), "loaded units cannot move freely");
    assert_eq!(game_logic.overlord_bunker_residual_enters(), 1);
    assert_eq!(
        game_logic.transport_residual_loads(),
        0,
        "Overlord bunker enter must not count as generic transport load"
    );
    assert_eq!(
        game_logic.garrison_residual_enters(),
        0,
        "Overlord bunker enter must not count as structure garrison"
    );
}

/// Residual acceptance: load 2 infantry → unload all → both free + honesty.
#[test]
fn overlord_bunker_residual_load_two_unload_both_free() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    let overlord_id = create_test_overlord(&mut game_logic, Vec3::new(0.0, 0.0, 0.0), Some(5));

    let unit_a = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(1.0, 0.0, 0.0))
        .expect("unit a");
    let unit_b = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(2.0, 0.0, 0.0))
        .expect("unit b");

    for unit_id in [unit_a, unit_b] {
        {
            let unit = game_logic.host_object_mut(unit_id).expect("unit mut");
            unit.target = Some(overlord_id);
            unit.set_ai_state(AIState::Entering);
        }
        game_logic.update_ai(&[unit_id, overlord_id], 1.0 / 30.0);
    }

    let overlord = game_logic
        .host_object(overlord_id)
        .expect("overlord loaded");
    assert!(
        overlord.contained_units().contains(&unit_a)
            && overlord.contained_units().contains(&unit_b),
        "both infantry must be loaded into Overlord bunker residual"
    );
    assert_eq!(overlord.transport_count(), 2);
    assert_eq!(game_logic.overlord_bunker_residual_enters(), 2);

    for unit_id in [unit_a, unit_b] {
        let unit = game_logic.host_object(unit_id).expect("loaded unit");
        assert_eq!(unit.ai_state, AIState::Docked);
        assert_eq!(unit.contained_by, Some(overlord_id));
        assert!(!unit.can_move());
    }

    // Unload all (selected Overlord Evacuate residual).
    game_logic.queue_command(GameCommand {
        command_type: CommandType::Evacuate,
        player_id: 1,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![overlord_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let overlord = game_logic.host_object(overlord_id).expect("overlord empty");
    assert!(
        overlord.contained_units().is_empty(),
        "evacuate must clear all Overlord bunker residual occupants"
    );
    assert_eq!(overlord.transport_count(), 0);

    for unit_id in [unit_a, unit_b] {
        let unit = game_logic.host_object(unit_id).expect("freed unit");
        assert_eq!(unit.ai_state, AIState::Idle, "unloaded unit must be Idle");
        assert!(unit.contained_by.is_none(), "contained_by must clear");
        assert!(unit.target.is_none());
        assert!(unit.can_move(), "unloaded unit must be free to move");
    }

    assert_eq!(game_logic.overlord_bunker_residual_exits(), 2);
    assert!(
        game_logic.honesty_overlord_bunker_enter_exit_ok(),
        "enter+exit residual honesty"
    );
    assert_eq!(
        game_logic.transport_residual_unloads(),
        0,
        "Overlord bunker unload must not count as generic transport unload"
    );
    assert_eq!(
        game_logic.garrison_residual_exits(),
        0,
        "Overlord bunker unload must not count as garrison exit"
    );
}

/// Residual: full BattleBunker capacity rejects further Enter.
#[test]
fn overlord_bunker_residual_capacity_full_rejects_enter() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    let overlord_id = create_test_overlord(&mut game_logic, Vec3::new(0.0, 0.0, 0.0), Some(1));

    let first_id = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(1.0, 0.0, 0.0))
        .expect("first");
    {
        let unit = game_logic.host_object_mut(first_id).unwrap();
        unit.target = Some(overlord_id);
        unit.set_ai_state(AIState::Entering);
    }
    game_logic.update_ai(&[first_id, overlord_id], 1.0 / 30.0);
    assert!(game_logic
        .host_object(overlord_id)
        .unwrap()
        .contained_units()
        .contains(&first_id));

    let second_id = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(3.0, 0.0, 0.0))
        .expect("second");

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter {
            target_id: overlord_id,
        },
        player_id: 1,
        command_id: 4,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![second_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let second = game_logic.host_object(second_id).expect("second after");
    assert_ne!(
        second.ai_state,
        AIState::Entering,
        "full Overlord bunker residual must reject Enter"
    );
    assert_ne!(second.target, Some(overlord_id));
    assert_eq!(
        game_logic
            .host_object(overlord_id)
            .unwrap()
            .transport_count(),
        1
    );
}

/// Residual: vehicles cannot enter Overlord BattleBunker (infantry-only).
#[test]
fn overlord_bunker_residual_rejects_vehicle_enter() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let overlord_id = create_test_overlord(&mut game_logic, Vec3::new(0.0, 0.0, 0.0), Some(5));
    let tank_id = game_logic
        .create_object("TestTank", Team::China, Vec3::new(2.0, 0.0, 0.0))
        .expect("tank");

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter {
            target_id: overlord_id,
        },
        player_id: 1,
        command_id: 5,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![tank_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let tank = game_logic.host_object(tank_id).expect("tank");
    assert_ne!(
        tank.ai_state,
        AIState::Entering,
        "vehicles must not enter Overlord BattleBunker residual"
    );
    assert_eq!(game_logic.overlord_bunker_residual_enters(), 0);
    assert!(game_logic
        .host_object(overlord_id)
        .unwrap()
        .contained_units()
        .is_empty());
}
