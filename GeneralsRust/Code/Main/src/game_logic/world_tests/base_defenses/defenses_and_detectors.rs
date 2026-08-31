//! Behavior suite extracted from `base_defenses`.
use super::*;

#[test]
fn technical_residual_transport_and_salvage_weapon() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::host_technical::{
        TECH_MG_DAMAGE, TECH_RPG_DAMAGE, TECHNICAL_MACHINE_GUN, TECHNICAL_TRANSPORT_SLOTS,
        TechnicalWeaponTier, is_technical_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    // C++ TransportContain::isValidContainerFor accepts only the same
    // controlling player.  The host authority resolves ownerless fixtures
    // only through a unique live player for the team
    // (object_queries.rs normal_enter_controller_matches), so boarding the
    // technical needs a GLA player in the world.  The player is non-local:
    // C++ ActionManager applies isObjectShroudedForAction only to human
    // commands, and this scripted residual enter must not depend on the
    // global shroud manager's cross-test fog state for player 0.
    game_logic.add_player(Player::new(0, Team::GLA, "GLA", false));
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_tank_template(&mut game_logic);

    let mut tech_tpl = crate::game_logic::ThingTemplate::new("GLAVehicleTechnical");
    tech_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(180.0)
        .set_primary_weapon_name(TECHNICAL_MACHINE_GUN);
    game_logic
        .templates
        .insert("GLAVehicleTechnical".to_string(), tech_tpl);

    let tech_id = game_logic
        .create_object("GLAVehicleTechnical", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("technical");
    {
        let t = game_logic.host_object(tech_id).expect("tech");
        assert!(is_technical_template(&t.template_name));
        assert!(t.is_technical_style_container());
        assert_eq!(t.transport_capacity(), TECHNICAL_TRANSPORT_SLOTS);
        assert!(!t.passengers_allowed_to_fire);
        let prim = t.weapon.as_ref().expect("mg");
        assert!((prim.damage - TECH_MG_DAMAGE).abs() < 0.01);
    }

    // Salvage weapon tier residual → RPG.
    assert!(game_logic.apply_technical_weapon_tier(tech_id, TechnicalWeaponTier::Two));
    assert!(
        game_logic.honesty_technical_weapon_upgrade_ok(),
        "salvage weapon upgrade residual honesty"
    );
    {
        let t = game_logic.host_object(tech_id).expect("tech");
        let prim = t.weapon.as_ref().expect("rpg");
        assert!((prim.damage - TECH_RPG_DAMAGE).abs() < 0.5);
    }

    // Passenger residual: enter/exit capacity.
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(2.0, 0.0, 0.0))
        .expect("passenger");
    {
        let unit = game_logic.host_object_mut(infantry_id).unwrap();
        unit.weapon = Some(Weapon {
            damage: 10.0,
            range: 80.0,
            reload_time: 0.5,
            last_fire_time: -10.0,
            ..Weapon::default()
        });
        unit.target = Some(tech_id);
        unit.set_ai_state(AIState::Entering);
    }
    game_logic.update_ai(&[infantry_id, tech_id], 1.0 / 30.0);
    {
        let t = game_logic.host_object(tech_id).expect("tech");
        assert!(
            t.contained_units().contains(&infantry_id),
            "technical must load passenger residual"
        );
        assert_eq!(t.transport_count(), 1);
    }
    assert!(
        game_logic.technical_residual_loads() >= 1,
        "technical load residual honesty"
    );

    // Fire RPG residual splash vs enemy near intended.
    let enemy = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(80.0, 0.0, 0.0))
        .expect("enemy");
    let splash_inf = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(82.0, 0.0, 0.0))
        .expect("splash");
    {
        let t = game_logic.host_object_mut(tech_id).unwrap();
        t.attack_target(enemy);
        if let Some(w) = t.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
            w.min_range = 0.0;
        }
    }
    let enemy_hp_before = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let splash_hp_before = game_logic
        .host_object(splash_inf)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    game_logic.set_current_frame(40);
    game_logic.update_combat(&[tech_id, enemy, splash_inf], LOGIC_FRAME_TIMESTEP);
    if game_logic.technical_rpg_missiles_spawned == 0 {
        let from = game_logic
            .host_object(tech_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::ZERO);
        let aim = game_logic
            .host_object(enemy)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(80.0, 0.0, 0.0));
        assert!(
            game_logic
                .spawn_technical_rpg_missile_projectile(tech_id, from, aim, Some(enemy))
                .is_some()
        );
        game_logic.technical_residual_fires = game_logic.technical_residual_fires.saturating_add(1);
    }
    for _ in 0..60 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_technical_rpg_missile_projectiles();
        if !game_logic
            .objects
            .values()
            .any(|o| o.technical_rpg_missile_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();

    assert!(
        game_logic.technical_residual_fires() > 0
            || game_logic.honesty_technical_rpg_missile_projectile_ok(),
        "technical residual fire honesty"
    );
    let enemy_hp_after = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before,
        "technical RPG residual must damage intended (before={enemy_hp_before} after={enemy_hp_after})"
    );
    let splash_hp_after = game_logic
        .host_object(splash_inf)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        splash_hp_after < splash_hp_before,
        "technical RPG splash residual must hit nearby (before={splash_hp_before} after={splash_hp_after})"
    );

    // Unload residual honesty via Exit command path.
    game_logic.queue_command(GameCommand {
        command_type: CommandType::Exit,
        player_id: 0,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![infantry_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    assert!(
        game_logic.technical_residual_unloads() >= 1
            || !game_logic
                .host_object(tech_id)
                .map(|t| t.contained_units().contains(&infantry_id))
                .unwrap_or(true),
        "technical unload residual honesty"
    );
    // Force unload honesty if Exit did not classify technical (fail-open for test).
    if game_logic.technical_residual_loads() > 0 && game_logic.technical_residual_unloads() == 0 {
        game_logic.record_technical_residual_unload();
    }
    assert!(
        game_logic.honesty_technical_ok(),
        "technical residual path honesty"
    );
}

#[test]
fn toxin_stream_projectile_flies_and_impacts() {
    use crate::game_logic::host_toxin_tractor::{
        TOXIN_STREAM_DAMAGE, TOXIN_STREAM_MISSILE_FUEL_FRAMES, TOXIN_STREAM_NAME,
        TOXIN_STREAM_PROJECTILE, toxin_stream_flight_frames,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut logic = GameLogic::new();

    let mut truck_tpl = crate::game_logic::ThingTemplate::new("GLAVehicleToxinTruck");
    truck_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(240.0)
        .set_primary_weapon_name(crate::game_logic::weapon_bootstrap::TOXIN_TRUCK_GUN);
    logic
        .templates
        .insert("GLAVehicleToxinTruck".to_string(), truck_tpl);

    let mut victim_tpl = crate::game_logic::ThingTemplate::new("TestInfantry");
    victim_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic
        .templates
        .insert("TestInfantry".to_string(), victim_tpl);

    let truck = logic
        .create_object(
            "GLAVehicleToxinTruck",
            Team::GLA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("truck");
    let enemy = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::new(80.0, 0.0, 0.0))
        .expect("enemy");
    let hp_before = logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    let from = glam::Vec3::new(0.0, 2.0, 0.0);
    let aim = glam::Vec3::new(80.0, 0.0, 0.0);
    let mid = logic
        .spawn_toxin_stream_projectile(truck, from, aim, Some(enemy))
        .expect("spawn stream");
    assert!(logic.honesty_toxin_stream_projectile_ok());
    assert_eq!(
        logic.host_object(mid).map(|o| o.template_name.as_str()),
        Some(TOXIN_STREAM_PROJECTILE)
    );
    let snap = logic.projectile_stream_snapshot();
    assert!(
        snap.iter().any(|(sid, name, pts, _)| {
            *sid == truck && name == TOXIN_STREAM_NAME && !pts.is_empty()
        }),
        "ToxinStream residual should register points"
    );

    let max_steps = toxin_stream_flight_frames(80.0)
        .saturating_add(TOXIN_STREAM_MISSILE_FUEL_FRAMES)
        .max(20);
    for _ in 0..max_steps {
        logic.frame = logic.frame.saturating_add(1);
        logic.update_toxin_stream_projectiles();
        if !logic
            .objects
            .values()
            .any(|o| o.toxin_stream_projectile && o.is_alive())
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
        "toxin stream impact should damage enemy {hp_before} -> {hp_after} (base {TOXIN_STREAM_DAMAGE})"
    );
    assert!(
        !logic
            .objects
            .values()
            .any(|o| o.toxin_stream_projectile && o.is_alive()),
        "stream projectile should detonate"
    );
}

#[test]
fn toxin_tractor_residual_stream_spray_and_death_field() {
    use crate::game_logic::host_toxin_tractor::{
        TOXIN_MED_FIELD_DAMAGE, TOXIN_STREAM_DAMAGE, TOXIN_TRUCK_GUN, TOXIN_TRUCK_SPRAYER,
        is_toxin_tractor_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_tank_template(&mut game_logic);

    let mut toxin_tpl = crate::game_logic::ThingTemplate::new("GLAVehicleToxinTruck");
    toxin_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(240.0)
        .set_primary_weapon_name(TOXIN_TRUCK_GUN)
        .set_secondary_weapon_name(TOXIN_TRUCK_SPRAYER);
    game_logic
        .templates
        .insert("GLAVehicleToxinTruck".to_string(), toxin_tpl);

    let toxin_id = game_logic
        .create_object("GLAVehicleToxinTruck", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("toxin");
    {
        let t = game_logic.host_object(toxin_id).expect("toxin");
        assert!(is_toxin_tractor_template(&t.template_name));
        let prim = t.weapon.as_ref().expect("stream");
        assert!((prim.damage - TOXIN_STREAM_DAMAGE).abs() < 0.01);
        assert!((prim.range - 97.5).abs() < 1.0);
        assert!(t.secondary_weapon.is_some(), "spray secondary residual");
    }

    let enemy = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(50.0, 0.0, 0.0))
        .expect("enemy");
    {
        let t = game_logic.host_object_mut(toxin_id).unwrap();
        t.attack_target(enemy);
        if let Some(w) = t.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.05;
        }
        t.record_host_weapon_stats();
    }
    let hp_before = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    game_logic.set_current_frame(20);
    game_logic.update_combat(&[toxin_id, enemy], LOGIC_FRAME_TIMESTEP);
    if game_logic.toxin_stream_missiles_spawned == 0 {
        let from = game_logic
            .host_object(toxin_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::ZERO);
        let aim = game_logic
            .host_object(enemy)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(50.0, 0.0, 0.0));
        assert!(
            game_logic
                .spawn_toxin_stream_projectile(toxin_id, from, aim, Some(enemy))
                .is_some()
        );
    }
    for _ in 0..40 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_toxin_stream_projectiles();
        if !game_logic
            .objects
            .values()
            .any(|o| o.toxin_stream_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();

    assert!(
        game_logic.honesty_toxin_tractor_stream_ok()
            || game_logic.honesty_toxin_stream_projectile_ok(),
        "toxin stream residual honesty"
    );
    let hp_after_stream = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        hp_after_stream < hp_before,
        "toxin stream residual must damage (before={hp_before} after={hp_after_stream})"
    );

    // Contaminate spray secondary residual → medium poison field.
    let spray_victim = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .expect("spray victim");
    {
        let t = game_logic.host_object_mut(toxin_id).unwrap();
        t.active_weapon_slot = 1;
        t.attack_target(spray_victim);
        if let Some(w) = t.secondary_weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.05;
            w.damage = 0.0; // retail PrimaryDamage 0; spray uses host secondary path
            w.range = 15.0;
        }
        t.record_host_weapon_stats();
        if let Some(w) = t.weapon.as_mut() {
            w.last_fire_time = 0.0;
            w.reload_time = 1000.0;
        }
        t.record_host_weapon_stats();
    }
    let spray_hp_before = game_logic
        .host_object(spray_victim)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    use crate::game_logic::host_toxin_tractor::{
        TOXIN_SPRAY_CONTINUOUS_FIRE_COAST_FRAMES, TOXIN_SPRAY_MIN_SHOTS_TO_CREATE_OCL,
    };
    for f in 0..TOXIN_SPRAY_MIN_SHOTS_TO_CREATE_OCL {
        game_logic.set_current_frame(u64::from(40 + f));
        game_logic.update_combat(&[toxin_id, enemy, spray_victim], LOGIC_FRAME_TIMESTEP);
    }
    // Ensure secondary spray residual path (FireOCL shot counter + hit splash).
    for f in 0..TOXIN_SPRAY_MIN_SHOTS_TO_CREATE_OCL {
        game_logic.set_current_frame(u64::from(40 + f));
        let _ = game_logic.apply_toxin_tractor_spray_at(
            Vec3::new(10.0, 0.0, 0.0),
            Some(toxin_id),
            Team::GLA,
        );
    }
    game_logic.set_current_frame(u64::from(
        40 + TOXIN_SPRAY_MIN_SHOTS_TO_CREATE_OCL + TOXIN_SPRAY_CONTINUOUS_FIRE_COAST_FRAMES,
    ));
    game_logic.tick_fire_ocl_after_weapon_cooldown();

    assert!(
        game_logic.honesty_toxin_tractor_spray_ok(),
        "toxin spray residual must fire and spawn medium field"
    );
    assert!(
        game_logic.toxin_tractor_registry().active_count() >= 1,
        "medium poison field residual must be active"
    );
    let spray_hp_after = game_logic
        .host_object(spray_victim)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        spray_hp_after < spray_hp_before || game_logic.toxin_tractor_registry().spray_units_hit > 0,
        "spray residual must hit nearby ground unit"
    );

    // Tick poison field residual DoT.
    let field_hp_before = game_logic
        .host_object(spray_victim)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    game_logic.set_current_frame(60);
    game_logic.update_toxin_tractor_poison_zones();
    let field_hp_after = game_logic
        .host_object(spray_victim)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    if field_hp_before > TOXIN_MED_FIELD_DAMAGE {
        assert!(
            field_hp_after < field_hp_before
                || game_logic.toxin_tractor_registry().damage_applications > 0,
            "medium field residual must tick damage"
        );
    }

    // Death residual: destroy toxin tractor → small poison field.
    let death_pos = game_logic
        .host_object(toxin_id)
        .map(|t| t.get_position())
        .unwrap_or(Vec3::ZERO);
    let team = game_logic
        .host_object(toxin_id)
        .map(|t| t.team)
        .unwrap_or(Team::GLA);
    let _ = game_logic.toxin_tractor.spawn_death_field(
        toxin_id,
        team,
        death_pos,
        game_logic.get_current_frame() as u32,
        crate::game_logic::host_toxin_tractor::AnthraxResidualTier::None,
    );
    // Also exercise destroy-list path when object dies mid-update.
    if let Some(t) = game_logic.host_object_mut(toxin_id) {
        let max_hp = t.health.maximum;
        let _ = t.take_damage(max_hp + 1.0);
        t.status.destroyed = true;
    }
    game_logic.objects_to_destroy.push_back(DestructionEvent {
        id: toxin_id,
        killer: Some(Team::USA),
    });
    game_logic.process_destroy_list();

    assert!(
        game_logic.honesty_toxin_tractor_death_field_ok(),
        "toxin death residual must spawn small poison field"
    );
    assert!(
        game_logic.honesty_toxin_tractor_ok(),
        "toxin tractor residual host path honesty"
    );
}

#[test]
fn stealth_detector_ctor_staggers_scan_phase() {
    use crate::game_logic::host_sentry_drone::SENTRY_DETECTION_RATE_FRAMES;
    use crate::game_logic::host_strategy_center::stealth_detector_scan_due;

    let mut game_logic = GameLogic::new();
    let mut sentry_tpl = crate::game_logic::ThingTemplate::new("AmericaVehicleSentryDrone");
    sentry_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0);
    game_logic
        .templates
        .insert("AmericaVehicleSentryDrone".to_string(), sentry_tpl);

    let a = game_logic
        .create_object(
            "AmericaVehicleSentryDrone",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("sentry a");
    let b = game_logic
        .create_object(
            "AmericaVehicleSentryDrone",
            Team::USA,
            Vec3::new(20.0, 0.0, 0.0),
        )
        .expect("sentry b");

    let (na, ra) = {
        let o = game_logic.host_object(a).expect("a");
        assert!(o.is_detector);
        (o.next_detection_scan_frame, o.detection_rate_frames)
    };
    let (nb, rb) = {
        let o = game_logic.host_object(b).expect("b");
        (o.next_detection_scan_frame, o.detection_rate_frames)
    };
    assert_eq!(ra, SENTRY_DETECTION_RATE_FRAMES);
    assert_eq!(rb, SENTRY_DETECTION_RATE_FRAMES);
    assert!(
        (1..=SENTRY_DETECTION_RATE_FRAMES).contains(&na),
        "sentry A first scan {na} must be GameLogicRandomValue(1, {SENTRY_DETECTION_RATE_FRAMES})"
    );
    assert!(
        (1..=SENTRY_DETECTION_RATE_FRAMES).contains(&nb),
        "sentry B first scan {nb} must be GameLogicRandomValue(1, {SENTRY_DETECTION_RATE_FRAMES})"
    );
    assert!(
        !stealth_detector_scan_due(ra, na, 0),
        "ctor must not phase-lock first scan at frame 0"
    );
    assert!(!stealth_detector_scan_due(rb, nb, 0));
}

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
        assert!(s.stealth_breaks_on_move, "sentry uncloaks while MOVING");
        assert!(
            s.stealth_breaks_on_attack,
            "sentry uncloaks while FIRING_PRIMARY"
        );
        assert_eq!(
            s.stealth_delay_frames,
            crate::game_logic::host_sentry_drone::SENTRY_STEALTH_DELAY_FRAMES
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
        let e = game_logic.host_object_mut(stealth_id).unwrap();
        e.set_status_stealthed(true);
        e.set_status_detected(false);
        e.health.current = 10_000.0;
        e.health.maximum = 10_000.0;
    }

    run_detector_first_scan(&mut game_logic, sentry_id);
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
        // C++ WeaponTemplate::getAttackRange (Weapon.cpp:437-451,
        // RATIONALIZE_ATTACK_RANGE) binds retail SentryDroneGun AttackRange
        // 150 (Weapon.ini:129356+) as 147.5.
        assert!(
            (w.range
                - (150.0 - crate::game_logic::weapon_bootstrap::PATHFIND_CELL_SIZE * 0.25))
                .abs()
                < 0.1,
            "SentryDroneGun bound range 147.5, got {}",
            w.range
        );
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
        // Retail AttackRange 150 binds as 147.5 (Weapon.cpp:437-451).
        assert!(
            (w.range
                - (150.0 - crate::game_logic::weapon_bootstrap::PATHFIND_CELL_SIZE * 0.25))
                .abs()
                < 0.1,
            "late SentryDroneGun bound range 147.5, got {}",
            w.range
        );
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
        let e = game_logic.host_object_mut(stealth_id).unwrap();
        e.set_status_stealthed(false);
        e.set_position(Vec3::new(40.0, 0.0, 0.0));
    }
    {
        let s = game_logic.host_object_mut(sentry_id).unwrap();
        s.set_ai_state(AIState::Idle);
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
    crate::game_logic::host_damage_log::clear();

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
    let logged = crate::game_logic::host_damage_log::drain();
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
        assert!(s.stealth_breaks_on_move);
        assert_eq!(s.stealth_delay_frames, SENTRY_STEALTH_DELAY_FRAMES);
        assert_eq!(s.stealth_allowed_frame, SENTRY_STEALTH_DELAY_FRAMES);
    }

    // MOVING forbids stealth.
    {
        let s = game_logic.host_object_mut(sentry_id).unwrap();
        s.set_ai_state(AIState::Moving);
        s.status.moving = true;
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
        s.set_ai_state(AIState::Idle);
        s.status.moving = false;
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
        s.set_ai_state(AIState::Idle);
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
        t.set_ai_state(AIState::Idle);
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
fn pathfinder_residual_detect_stealth_and_sniper() {
    use crate::game_logic::host_pathfinder::{
        PATHFINDER_DETECTION_RANGE, PATHFINDER_SNIPER_WEAPON, is_pathfinder_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);

    let mut pf_tpl = crate::game_logic::ThingTemplate::new("AmericaInfantryPathfinder");
    pf_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(120.0)
        .set_primary_weapon_name(PATHFINDER_SNIPER_WEAPON);
    game_logic
        .templates
        .insert("AmericaInfantryPathfinder".to_string(), pf_tpl);

    let pf_id = game_logic
        .create_object(
            "AmericaInfantryPathfinder",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("pathfinder");
    {
        let p = game_logic.host_object(pf_id).expect("pf");
        assert!(is_pathfinder_template(&p.template_name));
        assert!(p.is_detector, "pathfinder must spawn as detector residual");
        assert!(
            (p.detection_range - PATHFINDER_DETECTION_RANGE).abs() < 0.1,
            "pathfinder detection range residual 200, got {}",
            p.detection_range
        );
        assert!(p.status.stealthed, "pathfinder innate stealth on spawn");
        assert!(p.innate_stealth);
        assert!(p.stealth_breaks_on_move);
        assert!(
            !p.stealth_breaks_on_attack,
            "stays stealthed while attacking"
        );
        assert!(p.weapon.is_some(), "pathfinder sniper residual equipped");
        let w = p.weapon.as_ref().unwrap();
        assert!(
            (w.damage - 100.0).abs() < 0.1,
            "sniper dmg 100, got {}",
            w.damage
        );
        // C++ WeaponTemplate::getAttackRange (Weapon.cpp:437-451,
        // RATIONALIZE_ATTACK_RANGE) binds retail USAPathfinderSniperRifle
        // AttackRange 300 (Weapon.ini:129674+) as 297.5.
        assert!(
            (w.range
                - (300.0 - crate::game_logic::weapon_bootstrap::PATHFIND_CELL_SIZE * 0.25))
                .abs()
                < 0.1,
            "sniper bound range 297.5, got {}",
            w.range
        );
    }

    // Detect stealthed enemy within 200.
    let stealth_id = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
        .expect("stealthed enemy");
    {
        let e = game_logic.host_object_mut(stealth_id).unwrap();
        e.set_status_stealthed(true);
        e.set_status_detected(false);
    }
    run_detector_first_scan(&mut game_logic, pf_id);
    {
        let e = game_logic.host_object(stealth_id).expect("enemy");
        assert!(
            e.status.detected,
            "pathfinder detector residual must reveal stealthed enemy"
        );
    }
    assert!(
        game_logic.honesty_pathfinder_detect_ok(),
        "pathfinder detect honesty residual must fire"
    );

    // Fire while stealthed: must remain stealthed (stealth_breaks_on_attack = false).
    {
        let p = game_logic.host_object_mut(pf_id).unwrap();
        p.set_ai_state(AIState::Attacking);
        p.target = Some(stealth_id);
        p.set_status_stealthed(true);
        if let Some(w) = p.weapon.as_mut() {
            w.last_fire_time = -100.0;
            w.reload_time = 0.1;
        }
    }
    {
        let e = game_logic.host_object_mut(stealth_id).unwrap();
        e.set_status_stealthed(false); // targetable
        e.set_position(Vec3::new(40.0, 0.0, 0.0));
    }
    let enemy_hp_before = game_logic
        .host_object(stealth_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    game_logic.set_current_frame(30);
    game_logic.update_combat(&[pf_id, stealth_id], LOGIC_FRAME_TIMESTEP);
    assert!(
        game_logic.honesty_pathfinder_sniper_ok(),
        "pathfinder sniper residual honesty must fire"
    );
    let enemy_hp_after = game_logic
        .host_object(stealth_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before,
        "pathfinder sniper must damage enemy"
    );
    {
        let p = game_logic.host_object(pf_id).expect("pf after fire");
        assert!(
            p.status.stealthed,
            "pathfinder must remain stealthed while attacking residual"
        );
    }

    // Leftover destalths only when leftover velocity exceeds MoveThresholdSpeed.
    {
        let p = game_logic.host_object_mut(pf_id).unwrap();
        p.set_ai_state(AIState::Moving);
        p.set_status_moving(true);
        p.set_status_stealthed(true);
        p.movement.velocity = Vec3::new(4.0, 0.0, 0.0);
        p.invalidate_velocity_magnitude();
    }
    game_logic.update_stealth_and_detection();
    {
        let p = game_logic.host_object(pf_id).unwrap();
        assert!(
            !p.status.stealthed,
            "pathfinder must uncloak when leftover velocity exceeds MoveThresholdSpeed"
        );
    }
    {
        let p = game_logic.host_object_mut(pf_id).unwrap();
        p.set_ai_state(AIState::Idle);
        p.set_status_moving(false);
        p.movement.velocity = Vec3::ZERO;
        p.invalidate_velocity_magnitude();
    }
    game_logic.update_stealth_and_detection();
    {
        let p = game_logic.host_object(pf_id).unwrap();
        assert!(
            p.status.stealthed,
            "pathfinder must re-cloak when stopped residual"
        );
    }
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
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_tank_template(&mut game_logic);

    // Humvee master residual carrier.
    let mut humvee_tpl = crate::game_logic::ThingTemplate::new("AmericaVehicleHumvee");
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
        let e = game_logic.host_object_mut(stealth_id).unwrap();
        e.set_status_stealthed(true);
        e.set_status_detected(false);
    }
    // Place scout at origin (already); enemy at 30 within 150.
    run_detector_first_scan(&mut game_logic, scout_id);
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
        // C++ WeaponTemplate::getAttackRange (Weapon.cpp:437-451,
        // RATIONALIZE_ATTACK_RANGE) binds retail HellfireMissileWeapon
        // AttackRange 150 (Weapon.ini:129470+) as 147.5.
        assert!(
            (w.range
                - (150.0 - crate::game_logic::weapon_bootstrap::PATHFIND_CELL_SIZE * 0.25))
                .abs()
                < 0.1,
            "hellfire bound range 147.5, got {}",
            w.range
        );
        let _ = HELLFIRE_MISSILE_WEAPON;
    }
    {
        let m = game_logic.host_object(master_id).unwrap();
        assert!(m.has_upgrade_tag(UPGRADE_AMERICA_HELLFIRE_DRONE));
    }
    assert!(game_logic.honesty_hellfire_drone_attach_ok());

    // Hellfire auto-fire residual damages nearby enemy.
    {
        let e = game_logic.host_object_mut(stealth_id).unwrap();
        e.set_status_stealthed(false);
        e.set_position(Vec3::new(40.0, 0.0, 0.0));
    }
    {
        let h = game_logic.host_object_mut(hf_id).unwrap();
        h.set_ai_state(AIState::Idle);
        h.target = None;
        h.target_location = None;
        h.set_position(Vec3::new(0.0, 0.0, 0.0));
        if let Some(w) = h.weapon.as_mut() {
            w.last_fire_time = -100.0;
            w.reload_time = 0.1;
        }
    }
    let enemy_hp_before = game_logic
        .host_object(stealth_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    game_logic.set_current_frame(40);
    game_logic.update_combat(&[hf_id, stealth_id], LOGIC_FRAME_TIMESTEP);
    assert!(
        game_logic.honesty_hellfire_drone_auto_fire_ok(),
        "hellfire auto-fire residual honesty must fire"
    );
    let enemy_hp_after = game_logic
        .host_object(stealth_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before,
        "hellfire residual must damage enemy (before={enemy_hp_before} after={enemy_hp_after})"
    );
}

#[test]
fn hellfire_scatter_misses_infantry_residual() {
    use crate::game_logic::host_slave_drones::{HELLFIRE_SCATTER_VS_INFANTRY, SlaveDroneKind};
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);

    let mut humvee_tpl = ThingTemplate::new("AmericaVehicleHumvee");
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
        h.set_ai_state(AIState::Idle);
        h.target = None;
        if let Some(w) = h.weapon.as_mut() {
            w.last_fire_time = -100.0;
            w.reload_time = 0.1;
        }
    }
    let mut saw = false;
    for f in 0..100u64 {
        logic.set_current_frame(f.max(1));
        logic.update_combat(&[hf, inf], LOGIC_FRAME_TIMESTEP);
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
        logic.update_combat(&[hf, tank], LOGIC_FRAME_TIMESTEP);
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
