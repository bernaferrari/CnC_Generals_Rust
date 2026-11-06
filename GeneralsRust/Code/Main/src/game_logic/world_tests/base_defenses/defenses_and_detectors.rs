//! Behavior suite extracted from `base_defenses`.
use super::*;

#[test]
fn sentry_drone_residual_detect_and_auto_fire() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::host_sentry_drone::{
        SENTRY_DETECTION_RANGE, SENTRY_DRONE_GUN_WEAPON, SENTRY_PACK_TIME_FRAMES,
        SENTRY_TURRETS_MUST_CENTER_BEFORE_PACK, SENTRY_TURRETS_ONLY_WHEN_DEPLOYED,
        SENTRY_UNPACK_TIME_FRAMES, UPGRADE_AMERICA_SENTRY_DRONE_GUN, is_sentry_drone_template,
    };
    use crate::game_logic::host_upgrades::HostUpgradeKind;
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 10_000;
    game_logic.add_player(player);
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_barracks_template(&mut game_logic);

    let mut sentry_tpl = crate::game_logic::ThingTemplate::new("AmericaVehicleSentryDrone");
    sentry_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0);
    // Retail AmericaVehicleSentryDrone's exact DeployStyleAIUpdate module.
    // The Sentry auto-acquire residual is permitted only through this authored
    // metadata, never by the template basename alone.
    sentry_tpl.deploy_style_metadata = Some(crate::game_logic::DeployStyleMetadata {
        pack_time_frames: SENTRY_PACK_TIME_FRAMES,
        unpack_time_frames: SENTRY_UNPACK_TIME_FRAMES,
        turrets_function_only_when_deployed: SENTRY_TURRETS_ONLY_WHEN_DEPLOYED,
        turrets_must_center_before_packing: SENTRY_TURRETS_MUST_CENTER_BEFORE_PACK,
        ..Default::default()
    });
    // No primary weapon until gun upgrade residual.
    game_logic
        .templates
        .insert("AmericaVehicleSentryDrone".to_string(), sentry_tpl);

    let producer_id = game_logic
        .create_object("TestBarracks", Team::USA, Vec3::new(-40.0, 0.0, 0.0))
        .expect("producer");
    // Keep this DeployStyle fire fixture independent of the low-power
    // production-speed residual: its lone producer has no power draw, so the
    // authored 30-second Upgrade.ini duration maps to 900 logic frames.
    game_logic
        .host_object_mut(producer_id)
        .expect("producer object")
        .power_consumed = 0;

    let sentry_id = game_logic
        .create_object(
            "AmericaVehicleSentryDrone",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("sentry");
    {
        let s = game_logic.host_object(sentry_id).expect("sentry");
        assert!(is_sentry_drone_template(&s.template_name));
        assert!(
            s.is_detector,
            "sentry must spawn as stealth detector residual"
        );
        assert!(
            (s.detection_range - SENTRY_DETECTION_RANGE).abs() < 0.1,
            "sentry detection range residual 225, got {}",
            s.detection_range
        );
        assert!(
            !s.status.stealthed,
            "sentry ctor waits StealthDelay before first cloak"
        );
        assert_eq!(
            s.stealth_allowed_frame,
            crate::game_logic::host_sentry_drone::SENTRY_STEALTH_DELAY_FRAMES
        );

        assert!(s.innate_stealth, "sentry InnateStealth Yes");
        assert_eq!(
            s.stealth_update_data().map(|d| d.stealth_delay_frames),
            Some(crate::game_logic::host_sentry_drone::SENTRY_STEALTH_DELAY_FRAMES)
        );
        assert!(
            s.weapon.is_none(),
            "pre-upgrade sentry must lack gun residual"
        );
    }

    // Place stealthed enemy within detection range.
    let mut stealth_id = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
        .expect("stealthed enemy");
    {
        let frame = game_logic.frame;
        let e = game_logic.host_object_mut(stealth_id).unwrap();
        e.apply_grant_stealth(frame);
        e.set_status_detected(false);
        e.health.current = 10_000.0;
        e.health.maximum = 10_000.0;
    }

    run_detector_first_scan(&mut game_logic, sentry_id);
    game_logic.update_stealth_and_detection();
    {
        let e = game_logic.host_object(stealth_id).expect("enemy");
        assert!(
            e.status.detected,
            "sentry detector residual must reveal stealthed enemy in range"
        );
    }
    assert!(
        game_logic.honesty_sentry_drone_detect_ok(),
        "sentry detect honesty residual must fire"
    );

    // Without gun: no auto-fire residual.
    game_logic.frame = 2;
    game_logic.update_combat(&[sentry_id, stealth_id], LOGIC_FRAME_TIMESTEP);
    assert!(
        !game_logic.honesty_sentry_drone_auto_fire_ok(),
        "fail-closed: unarmed sentry must not auto-fire"
    );

    // Research gun upgrade residual.
    game_logic.queue_command(GameCommand {
        command_type: CommandType::QueueUpgrade {
            upgrade_name: UPGRADE_AMERICA_SENTRY_DRONE_GUN.to_string(),
        },
        player_id: 0,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![producer_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    assert!(
        game_logic
            .host_upgrades()
            .honesty_queue_ok(HostUpgradeKind::SentryDroneGun),
        "sentry gun upgrade must queue residual"
    );
    // The actual producer owns its parsed Upgrade.ini BuildTime. One logic
    // frame must not make the Sentry gun appear. Advance the remaining exact
    // fixed frames individually, avoiding an incidental large-delta catch-up
    // path while preserving the real 900-frame research duration.
    game_logic.update();
    assert!(
        !game_logic
            .get_player(0)
            .is_some_and(|player| player.has_unlocked_upgrade(UPGRADE_AMERICA_SENTRY_DRONE_GUN)),
        "Sentry gun must remain queued after one logic frame"
    );
    for _ in 1..HostUpgradeKind::SentryDroneGun.retail_research_frames() {
        game_logic.update_with_dt(LOGIC_FRAME_TIMESTEP);
    }
    let queued_progress = game_logic
        .host_object(producer_id)
        .and_then(|producer| producer.building_data.as_ref())
        .and_then(|building| building.production_queue.first())
        .map(|item| (item.progress, item.total_time));
    assert!(
        game_logic
            .host_upgrades()
            .honesty_complete_ok(HostUpgradeKind::SentryDroneGun),
        "sentry gun upgrade must complete residual (producer={producer_id:?} owner={:?} frame={} power={:?} queue_progress={queued_progress:?} host_entries={:?})",
        game_logic
            .host_object(producer_id)
            .and_then(|producer| producer.owner_player_id),
        game_logic.frame,
        game_logic
            .get_player(0)
            .map(|player| (player.power_produced, player.power_consumed)),
        game_logic.host_upgrades().entries_snapshot(),
    );
    {
        let s = game_logic.host_object(sentry_id).expect("sentry");
        assert!(
            s.has_upgrade_tag(UPGRADE_AMERICA_SENTRY_DRONE_GUN),
            "sentry must receive gun upgrade tag"
        );
        assert!(
            s.weapon.is_some(),
            "sentry must equip SentryDroneGun after upgrade"
        );
        let w = s.weapon.as_ref().unwrap();
        assert!(
            (w.damage - 8.0).abs() < 0.1,
            "SentryDroneGun damage residual 8, got {}",
            w.damage
        );
        assert!(
            (w.range - 150.0).abs() < 0.1,
            "SentryDroneGun authored range150, got {}",
            w.range
        );
        assert!(s.is_within_attack_range_at_distance(0, 147.5));
        assert!(!s.is_within_attack_range_at_distance(0, 147.51));
        let _ = SENTRY_DRONE_GUN_WEAPON;
    }
    // C++ initObject updateUpgradeModules: drones built after research spawn armed.
    let late_sentry_id = game_logic
        .create_object(
            "AmericaVehicleSentryDrone",
            Team::USA,
            Vec3::new(20.0, 0.0, 0.0),
        )
        .expect("late sentry");
    {
        let s = game_logic.host_object(late_sentry_id).expect("late sentry");
        assert!(
            s.has_upgrade_tag(UPGRADE_AMERICA_SENTRY_DRONE_GUN),
            "late sentry must inherit completed PLAYER_UPGRADE tag"
        );
        assert!(
            s.weapon.is_some(),
            "sentry built after research must spawn with SentryDroneGun"
        );
        let w = s.weapon.as_ref().unwrap();
        assert!(
            (w.damage - 8.0).abs() < 0.1,
            "late SentryDroneGun damage residual 8, got {}",
            w.damage
        );
        assert!(
            (w.range - 150.0).abs() < 0.1,
            "late SentryDroneGun authored range150, got {}",
            w.range
        );
        assert!(s.is_within_attack_range_at_distance(0, 147.5));
        assert!(!s.is_within_attack_range_at_distance(0, 147.51));
    }
    // Detected enemy becomes targetable; place in gun range and idle for auto-fire.
    if game_logic.host_object(stealth_id).is_none() {
        stealth_id = game_logic
            .create_object("TestInfantry", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
            .expect("stealthed enemy respawn");
        let e = game_logic.host_object_mut(stealth_id).unwrap();
        e.set_status_stealthed(false);
        e.set_status_detected(true);
        e.health.current = 10_000.0;
        e.health.maximum = 10_000.0;
    } else {
        let (e, fixture_partition_e) = game_logic
            .host_object_and_partition_mut(stealth_id)
            .unwrap();
        e.set_status_stealthed(false);
        e.set_position_with_partition_owner(
            Vec3::new(40.0, 0.0, 0.0),
            0,
            &mut Default::default(),
            fixture_partition_e,
        );
    }
    {
        let s = game_logic.host_object_mut(sentry_id).unwrap();
        s.set_ai_state(
            AIState::Idle,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        s.target = None;
        s.target_location = None;
        if let Some(w) = s.weapon.as_mut() {
            w.last_fire_time = -100.0;
            w.reload_time = 0.1;
        }
    }

    let enemy_hp_before = game_logic
        .host_object(stealth_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    game_logic.health_events.clear_damage();

    game_logic.set_current_frame(30);
    game_logic.update_combat(&[sentry_id, stealth_id], LOGIC_FRAME_TIMESTEP);

    assert!(
        game_logic
            .host_object(sentry_id)
            .is_some_and(|s| s.target == Some(stealth_id)),
        "in-range auto acquisition must retain its pending target while unpacking"
    );
    assert!(
        matches!(
            game_logic
                .host_object(sentry_id)
                .and_then(|s| s.deploy_style.as_ref())
                .map(|deploy| deploy.state),
            Some(crate::game_logic::host_deploy_style::HostDeployStyleState::Deploying)
        ),
        "packed sentry must start the authored unpack timer before firing"
    );
    let enemy_hp_packed = game_logic
        .host_object(stealth_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        (enemy_hp_packed - enemy_hp_before).abs() < f32::EPSILON,
        "packed sentry must not damage before the 30-frame unpack completes"
    );

    // One frame before the C++ timer boundary remains packed.
    game_logic.set_current_frame(59);
    game_logic.tick_deploy_style_updates();
    game_logic.update_combat(&[sentry_id, stealth_id], LOGIC_FRAME_TIMESTEP);
    assert_eq!(
        game_logic.host_object(stealth_id).unwrap().health.current,
        enemy_hp_before,
        "no auto shot before the final unpack frame"
    );

    // At the exact 30-frame boundary, normal combat consumes the pending auto
    // target; the special residual never directly bypasses DeployStyle.
    game_logic.set_current_frame(60);
    game_logic.tick_deploy_style_updates();
    game_logic.update_combat(&[sentry_id, stealth_id], LOGIC_FRAME_TIMESTEP);
    let enemy_hp_after = game_logic
        .host_object(stealth_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let logged = game_logic.health_events.drain_damage();
    let log_hit = logged
        .iter()
        .any(|e| e.target == stealth_id && e.amount > 0.0);
    assert!(
        enemy_hp_after < enemy_hp_before || log_hit,
        "deployed sentry must damage its pending auto target via HP or damage-authority log (before={enemy_hp_before} after={enemy_hp_after}, log_hit={log_hit})"
    );
}

#[test]
fn sentry_drone_uncloaks_while_moving_and_recloaks_after_fire_delay() {
    use crate::game_logic::host_sentry_drone::SENTRY_STEALTH_DELAY_FRAMES;

    let mut game_logic = GameLogic::new();
    let mut sentry_tpl = crate::game_logic::ThingTemplate::new("AmericaVehicleSentryDrone");
    sentry_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0);
    // Authored `Behavior = StealthUpdate`: InnateStealth, MOVING FIRING_PRIMARY.
    sentry_tpl.stealth_update = Some(crate::game_logic::object::StealthUpdateData {
        stealth_delay_frames: SENTRY_STEALTH_DELAY_FRAMES,
        forbidden_conditions: crate::game_logic::object::StealthForbiddenConditions::MOVING
            .union(crate::game_logic::object::StealthForbiddenConditions::FIRING_PRIMARY),
        ..Default::default()
    });
    game_logic
        .templates
        .insert("AmericaVehicleSentryDrone".to_string(), sentry_tpl);

    let sentry_id = game_logic
        .create_object(
            "AmericaVehicleSentryDrone",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("sentry");
    {
        let s = game_logic.host_object(sentry_id).expect("sentry");
        assert!(
            !s.status.stealthed,
            "sentry ctor is visible until StealthDelay"
        );
        assert!(s.innate_stealth);
        assert_eq!(
            s.stealth_update_data().map(|d| d.stealth_delay_frames),
            Some(SENTRY_STEALTH_DELAY_FRAMES)
        );
        assert_eq!(s.stealth_allowed_frame, SENTRY_STEALTH_DELAY_FRAMES);
    }

    // MOVING forbids stealth.
    {
        let s = game_logic.host_object_mut(sentry_id).unwrap();
        s.set_ai_state(
            AIState::Moving,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        s.status.moving = true;
        // C++ MOVING is PhysicsBehavior velocity (StealthUpdate.cpp:390-392).
        s.movement.velocity = Vec3::new(10.0, 0.0, 0.0);
    }
    game_logic.frame = 1;
    game_logic.update_stealth_and_detection();
    assert!(
        !game_logic.host_object(sentry_id).unwrap().status.stealthed,
        "sentry must uncloak while moving"
    );

    // Stop: stay visible until StealthDelay elapses.
    {
        let s = game_logic.host_object_mut(sentry_id).unwrap();
        s.set_ai_state(
            AIState::Idle,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        s.status.moving = false;
        s.movement.velocity = Vec3::ZERO;
        s.set_status_attacking(false);
    }
    game_logic.frame = 2;
    game_logic.update_stealth_and_detection();
    assert!(
        !game_logic.host_object(sentry_id).unwrap().status.stealthed,
        "sentry must not re-cloak instantly after stopping"
    );
    let allowed = game_logic
        .host_object(sentry_id)
        .unwrap()
        .stealth_allowed_frame;
    assert!(allowed > 2, "StealthDelay must schedule re-cloak");

    game_logic.frame = allowed;
    game_logic.update_stealth_and_detection();
    assert!(
        game_logic.host_object(sentry_id).unwrap().status.stealthed,
        "sentry re-cloaks after StealthDelay"
    );

    // FIRING_PRIMARY forbids stealth; after fire, wait delay again.
    let fire_frame = game_logic.frame.saturating_add(1);
    {
        let s = game_logic.host_object_mut(sentry_id).unwrap();
        s.set_status_firing_weapon(true);
        s.last_fire_slot = 0;
        s.last_fire_frame = fire_frame;
    }

    game_logic.frame = allowed.saturating_add(1);
    game_logic.update_stealth_and_detection();
    assert!(
        !game_logic.host_object(sentry_id).unwrap().status.stealthed,
        "sentry must uncloak while firing"
    );
    {
        let s = game_logic.host_object_mut(sentry_id).unwrap();
        s.set_ai_state(
            AIState::Idle,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        s.set_status_attacking(false);
        s.set_status_firing_weapon(false);
    }

    let after_fire = game_logic.frame.saturating_add(1);
    game_logic.frame = after_fire;
    game_logic.update_stealth_and_detection();
    assert!(
        !game_logic.host_object(sentry_id).unwrap().status.stealthed,
        "sentry stays visible after first shot until StealthDelay"
    );
    let allowed_after_fire = game_logic
        .host_object(sentry_id)
        .unwrap()
        .stealth_allowed_frame;
    game_logic.frame = allowed_after_fire;
    game_logic.update_stealth_and_detection();
    assert!(
        game_logic.host_object(sentry_id).unwrap().status.stealthed,
        "sentry re-cloaks after fire StealthDelay"
    );
}

#[test]
fn sentry_drone_residual_skips_non_sentry_units() {
    use crate::game_logic::host_sentry_drone::is_sentry_drone_template;

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let tank_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("tank");
    {
        let t = game_logic.host_object_mut(tank_id).unwrap();
        t.is_detector = true;
        t.detection_range = 225.0;
        if let Some(w) = t.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
            w.range = 150.0;
        }
        t.set_ai_state(
            AIState::Idle,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        t.target = None;
    }
    let enemy_id = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
        .expect("enemy");

    {
        let t = game_logic.host_object(tank_id).unwrap();
        assert!(!is_sentry_drone_template(&t.template_name));
    }

    game_logic.set_current_frame(10);
    game_logic.update_combat(&[tank_id, enemy_id], LOGIC_FRAME_TIMESTEP);
    assert!(
        !game_logic.honesty_sentry_drone_auto_fire_ok(),
        "fail-closed: ordinary tank must not use Sentry auto-fire residual"
    );
}

#[test]
fn scout_and_hellfire_drone_residual_attach_detect_and_fire() {
    use crate::game_logic::host_slave_drones::{
        HELLFIRE_MISSILE_WEAPON, SCOUT_DETECTION_RANGE, SlaveDroneKind,
        UPGRADE_AMERICA_HELLFIRE_DRONE, UPGRADE_AMERICA_SCOUT_DRONE, is_hellfire_drone_template,
        is_scout_drone_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    // CPP TAiData initializes guard modifiers to zero until content loads.
    // Supply this receiving match's authored AIData rather than depending on
    // another fixture's engine catalog to permit ordinary mood acquisition.
    game_logic.set_ai_definition_base(game_engine::common::ini::AIData {
        guard_inner_modifier_ai: 1.1,
        guard_outer_modifier_ai: 1.333,
        aggressive_range_modifier: 1.5,
        ..Default::default()
    });

    let mut owner = Player::new(0, Team::USA, "slave owner", false);
    owner.alliance_team = 1;
    game_logic.add_player(owner);
    let mut opponent = Player::new(1, Team::GLA, "slave enemy", false);
    opponent.alliance_team = 2;
    game_logic.add_player(opponent);

    super::super::super::world_combat::slaved_update_tests::install_slave_fixture(
        &mut game_logic,
        SlaveDroneKind::Scout,
    );
    super::super::super::world_combat::slaved_update_tests::install_slave_fixture(
        &mut game_logic,
        SlaveDroneKind::Hellfire,
    );
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_tank_template(&mut game_logic);

    // Humvee master residual carrier.
    let mut humvee_tpl = crate::game_logic::ThingTemplate::new("AmericaVehicleHumvee");
    humvee_tpl.set_primary_weapon_name("HumveeGun");
    humvee_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
    game_logic
        .templates
        .insert("AmericaVehicleHumvee".to_string(), humvee_tpl);

    let master_id = game_logic
        .create_object("AmericaVehicleHumvee", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("humvee");

    // Attach Scout residual.
    let scout_id = game_logic
        .residual_attach_slave_drone(master_id, SlaveDroneKind::Scout)
        .expect("scout attach");
    {
        let s = game_logic.host_object(scout_id).expect("scout");
        assert!(is_scout_drone_template(&s.template_name));
        assert!(s.is_detector, "scout must spawn as detector residual");
        assert!(
            (s.detection_range - SCOUT_DETECTION_RANGE).abs() < 0.1,
            "scout detection range residual 150, got {}",
            s.detection_range
        );
        assert!(s.weapon.is_none(), "scout sensor drone has no gun residual");
    }
    {
        let m = game_logic.host_object(master_id).unwrap();
        assert!(
            m.has_upgrade_tag(UPGRADE_AMERICA_SCOUT_DRONE),
            "master must receive scout upgrade tag residual"
        );
    }
    assert!(game_logic.honesty_scout_drone_attach_ok());

    // Scout detects stealthed enemy.
    let stealth_id = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(30.0, 0.0, 0.0))
        .expect("stealthed");
    {
        let frame = game_logic.frame;
        let e = game_logic.host_object_mut(stealth_id).unwrap();
        e.apply_grant_stealth(frame);
        e.set_status_detected(false);
    }
    // Place scout at origin (already); enemy at 30 within 150.
    run_detector_first_scan(&mut game_logic, scout_id);
    game_logic.update_stealth_and_detection();
    {
        let e = game_logic.host_object(stealth_id).unwrap();
        assert!(
            e.status.detected,
            "scout detector residual must reveal stealthed enemy"
        );
    }
    assert!(game_logic.honesty_scout_drone_detect_ok());

    // Attach Hellfire residual to same master (fail-closed: no ConflictsWith skip).
    let hf_id = game_logic
        .residual_attach_slave_drone(master_id, SlaveDroneKind::Hellfire)
        .expect("hellfire attach");
    {
        let h = game_logic.host_object(hf_id).expect("hellfire");
        assert!(is_hellfire_drone_template(&h.template_name));
        assert!(h.weapon.is_some(), "hellfire must equip missile residual");
        let w = h.weapon.as_ref().unwrap();
        assert!(
            (w.damage - 40.0).abs() < 0.1,
            "hellfire dmg 40, got {}",
            w.damage
        );
        assert!(
            (w.range - 150.0).abs() < 0.1,
            "hellfire authored range150, got {}",
            w.range
        );
        assert!(h.is_within_attack_range_at_distance(0, 147.5));
        assert!(!h.is_within_attack_range_at_distance(0, 147.51));
        let _ = HELLFIRE_MISSILE_WEAPON;
    }
    {
        let m = game_logic.host_object(master_id).unwrap();
        assert!(m.has_upgrade_tag(UPGRADE_AMERICA_HELLFIRE_DRONE));
    }
    assert!(game_logic.honesty_hellfire_drone_attach_ok());

    // Complete the Scout detection phase without leaving its Infantry
    // victim in Hellfire's acquisition range. C++ Weapon.cpp:954-989 and
    // HellfireMissileWeapon ScatterRadiusVsInfantry=10 permit a miss;
    // accepted fire at Infantry is not a guaranteed single-shot HP change.
    game_logic.set_fixture_object_position(stealth_id, Vec3::new(1000.0, 0.0, 0.0));
    let damage_target_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
        .expect("non-infantry Hellfire damage target");
    assert!(
        game_logic
            .host_object(damage_target_id)
            .unwrap()
            .is_kind_of(KindOf::Vehicle)
    );
    assert!(
        !game_logic
            .host_object(damage_target_id)
            .unwrap()
            .is_kind_of(KindOf::Infantry)
    );

    // Hellfire auto-fire residual damages the nearby Vehicle. The separate
    // hellfire_scatter_misses_infantry_residual case retains scatter coverage.
    {
        let (h, fixture_partition_h) = game_logic.host_object_and_partition_mut(hf_id).unwrap();
        h.set_ai_state(
            AIState::Idle,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        h.target = None;
        h.target_location = None;
        h.set_position_with_partition_owner(
            Vec3::new(0.0, 0.0, 0.0),
            0,
            &mut Default::default(),
            fixture_partition_h,
        );
        if let Some(w) = h.weapon.as_mut() {
            w.last_fire_time = -100.0;
            w.reload_time = 0.1;
        }
    }
    let enemy_hp_before = game_logic
        .host_object(damage_target_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    game_logic.set_current_frame(40);
    assert_eq!(game_logic.host_object(hf_id).unwrap().vision_range, 100.0);
    assert!(
        game_logic.adjusted_vision_range_for_mood(hf_id) > 100.0,
        "actual supplied AIData must enable idle search radius"
    );
    assert!(
        game_logic
            .host_object(hf_id)
            .unwrap()
            .auto_acquire_when_idle()
    );
    let _ = game_logic.try_mood_auto_acquire(hf_id);
    assert_eq!(
        game_logic.host_object(hf_id).unwrap().target,
        Some(damage_target_id),
        "actual mood scan must select the Vehicle"
    );
    game_logic.update_combat(&[hf_id, damage_target_id], LOGIC_FRAME_TIMESTEP);
    game_logic.tick_nested_attack_machines(&[hf_id], 40.0 / 30.0, 40);
    assert_eq!(
        game_logic.hellfire_drone_residual_auto_fires, 0,
        "AIM transition must not fire its next state's body early"
    );
    game_logic.frame = 41;
    game_logic.update_combat(&[hf_id, damage_target_id], LOGIC_FRAME_TIMESTEP);
    game_logic.tick_nested_attack_machines(&[hf_id], 41.0 / 30.0, 41);
    crate::game_logic::combat::drain_pending_projectiles(
        &mut game_logic.combat_system,
        game_logic.objects.read(),
        41,
        &crate::game_logic::projectile_definitions::fixture_projectile_definitions(),
    );
    assert!(
        game_logic.honesty_hellfire_drone_auto_fire_ok(),
        "hellfire auto-fire residual honesty must fire"
    );
    assert_eq!(
        game_logic
            .host_object(damage_target_id)
            .unwrap()
            .health
            .current,
        enemy_hp_before,
        "named Hellfire missile launch must not immediately hit the Vehicle"
    );
    assert!(
        game_logic
            .combat_system
            .projectiles_snapshot()
            .iter()
            .any(|projectile| {
                projectile.shooter_id == hf_id && !projectile.projectile_object_name.is_empty()
            }),
        "accepted authored Hellfire creates its actual physical projectile"
    );
    let launched = game_logic
        .combat_system
        .projectiles_snapshot()
        .into_iter()
        .find(|p| p.shooter_id == hf_id)
        .unwrap();
    assert_eq!(launched.projectile_object_name, "MissileDefenderMissile");
    assert_eq!(launched.explosion_radius, 5.0);
    assert_eq!(
        launched.speed,
        (600.0_f32 * (1.0_f32 / 30.0_f32)) * 30.0_f32
    );
    assert!(
        matches!(
            launched.flight,
            Some(crate::game_logic::weapon_bootstrap::HostProjectileFlight::Missile(_))
        ),
        "receiving parsed missile profile must bind before launch"
    );
    assert_eq!(
        launched.target_id,
        Some(damage_target_id),
        "launch must retain actual selected Vehicle identity"
    );
    let mut flight_trace = Vec::new();
    flight_trace.push((
        game_logic.frame,
        vec![(
            launched.position,
            launched.target_position,
            launched.target_id,
            launched.lifetime,
            launched.explosion_radius,
            format!("{:?}", launched.flight),
            format!("{:?}", launched.flight_runtime),
        )],
    ));
    // Prevent another accepted shot; the already launched projectile continues
    // under the real receiving simulation/flight owner, not a fixture HP write.
    game_logic
        .host_object_mut(hf_id)
        .unwrap()
        .status
        .weapons_jammed = true;
    for step in 0..120 {
        game_logic.update_with_dt(LOGIC_FRAME_TIMESTEP);
        if step < 8 || step == 119 {
            flight_trace.push((
                game_logic.frame,
                game_logic
                    .combat_system
                    .projectiles_snapshot()
                    .into_iter()
                    .filter(|p| p.shooter_id == hf_id)
                    .map(|p| {
                        (
                            p.position,
                            p.target_position,
                            p.target_id,
                            p.lifetime,
                            p.explosion_radius,
                            format!("{:?}", p.flight),
                            format!("{:?}", p.flight_runtime),
                        )
                    })
                    .collect(),
            ));
        }
        if game_logic
            .host_object(damage_target_id)
            .unwrap()
            .health
            .current
            < enemy_hp_before
        {
            break;
        }
    }
    let enemy_hp_after = game_logic
        .host_object(damage_target_id)
        .unwrap()
        .health
        .current;
    assert!(
        enemy_hp_after < enemy_hp_before,
        "actual named Hellfire flight must damage Vehicle at impact (before={enemy_hp_before} after={enemy_hp_after}, target={:?}, trace={flight_trace:?})",
        game_logic.host_object(damage_target_id).map(|o| (
            o.get_position(),
            o.is_alive(),
            o.status.effectively_dead
        ))
    );
}

#[test]
fn hellfire_scatter_misses_infantry_residual() {
    use crate::game_logic::host_slave_drones::{HELLFIRE_SCATTER_VS_INFANTRY, SlaveDroneKind};
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut logic = GameLogic::new();
    // CPP TAiData initializes guard modifiers to zero until content loads.
    // Supply this receiving match's authored AIData rather than depending on
    // another fixture's engine catalog to permit ordinary mood acquisition.
    logic.set_ai_definition_base(game_engine::common::ini::AIData {
        guard_inner_modifier_ai: 1.1,
        guard_outer_modifier_ai: 1.333,
        aggressive_range_modifier: 1.5,
        ..Default::default()
    });

    let mut owner = Player::new(0, Team::USA, "slave owner", false);
    owner.alliance_team = 1;
    logic.add_player(owner);
    let mut opponent = Player::new(1, Team::GLA, "slave enemy", false);
    opponent.alliance_team = 2;
    logic.add_player(opponent);

    super::super::super::world_combat::slaved_update_tests::install_slave_fixture(
        &mut logic,
        SlaveDroneKind::Hellfire,
    );
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);

    let mut humvee_tpl = ThingTemplate::new("AmericaVehicleHumvee");
    humvee_tpl.set_primary_weapon_name("HumveeGun");
    humvee_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
    logic
        .templates
        .insert("AmericaVehicleHumvee".to_string(), humvee_tpl);

    let master = logic
        .create_object("AmericaVehicleHumvee", Team::USA, glam::Vec3::ZERO)
        .expect("humvee");
    let hf = logic
        .residual_attach_slave_drone(master, SlaveDroneKind::Hellfire)
        .expect("hellfire");
    let inf = logic
        .create_object("TestInfantry", Team::GLA, glam::Vec3::new(40.0, 0.0, 0.0))
        .expect("inf");
    if let Some(o) = logic.objects.get_mut(&inf) {
        o.set_selection_radius(0.5);
    }
    if let Some(h) = logic.objects.get_mut(&hf) {
        h.set_ai_state(
            AIState::Idle,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        h.target = None;
        if let Some(w) = h.weapon.as_mut() {
            w.last_fire_time = -100.0;
            w.reload_time = 0.1;
        }
    }
    let mut saw = false;
    for f in 0..100u64 {
        logic.set_current_frame(f.max(1));
        if f == 0 {
            assert!(
                logic.adjusted_vision_range_for_mood(hf) > 100.0,
                "actual supplied AIData must enable Infantry search"
            );
            assert!(logic.host_object(hf).unwrap().auto_acquire_when_idle());
        }
        let _ = logic.try_mood_auto_acquire(hf);
        if f == 0 {
            assert_eq!(
                logic.host_object(hf).unwrap().target,
                Some(inf),
                "actual idle mood scan must select Infantry"
            );
        }
        logic.update_combat(&[hf, inf], LOGIC_FRAME_TIMESTEP);
        logic.tick_nested_attack_machines(&[hf], logic.frame as f32 / 30.0, logic.frame);
        crate::game_logic::combat::drain_pending_projectiles(
            &mut logic.combat_system,
            logic.objects.read(),
            logic.frame,
            &crate::game_logic::projectile_definitions::fixture_projectile_definitions(),
        );
        if logic.hellfire_scatter_applied > 0 {
            saw = true;
        }
        if logic.hellfire_scatter_misses > 0 {
            break;
        }
    }
    assert!(saw || logic.hellfire_drone_residual_auto_fires > 0);
    assert!((HELLFIRE_SCATTER_VS_INFANTRY - 10.0).abs() < 0.01);
    // Vehicle path still damages (no infantry scatter gate).
    let tank = logic
        .create_object("TestTank", Team::GLA, glam::Vec3::new(35.0, 0.0, 0.0))
        .expect("tank");
    if let Some(h) = logic.objects.get_mut(&hf) {
        if let Some(w) = h.weapon.as_mut() {
            w.last_fire_time = -100.0;
        }
    }
    // Remove infantry so acquire prefers tank.
    logic.mark_object_for_destruction(inf, None);
    logic.process_destroy_list();
    let hp_before = logic.host_object(tank).unwrap().health.current;
    for f in 100..160u64 {
        logic.set_current_frame(f);
        let _ = logic.try_mood_auto_acquire(hf);
        logic.update_combat(&[hf, tank], LOGIC_FRAME_TIMESTEP);
        logic.tick_nested_attack_machines(&[hf], logic.frame as f32 / 30.0, logic.frame);
        crate::game_logic::combat::drain_pending_projectiles(
            &mut logic.combat_system,
            logic.objects.read(),
            logic.frame,
            &crate::game_logic::projectile_definitions::fixture_projectile_definitions(),
        );
    }
    let hp_after = logic
        .host_object(tank)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        hp_after < hp_before || logic.hellfire_drone_residual_auto_fires > 0,
        "hellfire must still engage vehicles (hp {}->{})",
        hp_before,
        hp_after
    );
    assert!(logic.honesty_hellfire_scatter_ok() || logic.hellfire_drone_residual_auto_fires > 0);
}
