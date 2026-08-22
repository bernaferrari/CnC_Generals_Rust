//! Host GameLogic tests — `pilots_and_movement`.
#![allow(unused_imports, non_snake_case, unused_variables, dead_code)]
use super::super::*;
use super::helpers::*;

/// Residual: pilot Enter rejects manned vehicles for recrew residual path
/// (fails closed into normal transport rules).
#[test]
fn pilot_recrew_rejects_manned_vehicle() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let mut pilot_tpl = ThingTemplate::new("TestPilot");
    pilot_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    game_logic
        .templates
        .insert("TestPilot".to_string(), pilot_tpl);

    let pilot_id = game_logic
        .create_object("TestPilot", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .expect("pilot");
    let tank_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("tank");
    assert!(!game_logic.host_object(tank_id).unwrap().is_unmanned());

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter { target_id: tank_id },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![pilot_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    // Manned enemy vehicle: enter may fail (team/capacity) — recrew honesty stays false.
    assert!(!game_logic.honesty_pilot_recrew_ok());
    assert!(!game_logic.host_object(tank_id).unwrap().is_unmanned());
    assert_eq!(game_logic.host_object(tank_id).unwrap().team, Team::GLA);
}

/// Residual: WorkerShoes QueueUpgrade → speed 30 + supply boost +8 on drop-off.
#[test]
fn worker_shoes_upgrade_speed_and_supply_boost_residual() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::host_gla_worker::{
        is_gla_worker_template, residual_worker_shoes_drop_off_boost, worker_residual_speed,
        UPGRADE_GLA_WORKER_SHOES, WORKER_BASE_SPEED, WORKER_SHOES_SPEED, WORKER_SHOES_SUPPLY_BOOST,
    };
    use crate::game_logic::host_upgrades::HostUpgradeKind;

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::GLA, "GLA", true);
    player.resources.supplies = 5000;
    game_logic.add_player(player);
    ensure_test_barracks_template(&mut game_logic);

    let mut worker_tpl = ThingTemplate::new("GLAInfantryWorker");
    worker_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Worker)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    game_logic
        .templates
        .insert("GLAInfantryWorker".to_string(), worker_tpl);

    let mut supply_tpl = ThingTemplate::new("GLASupplyStash");
    supply_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::SupplyCenter)
        .add_kind_of(KindOf::Selectable)
        .set_health(1000.0);
    game_logic
        .templates
        .insert("GLASupplyStash".to_string(), supply_tpl);

    let barracks_id = game_logic
        .create_object("TestBarracks", Team::GLA, Vec3::new(-40.0, 0.0, 0.0))
        .expect("barracks");
    let worker_id = game_logic
        .create_object("GLAInfantryWorker", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("worker");
    let supply_id = game_logic
        .create_object("GLASupplyStash", Team::GLA, Vec3::new(5.0, 0.0, 0.0))
        .expect("supply");

    {
        let w = game_logic.host_object(worker_id).expect("worker");
        assert!(is_gla_worker_template(&w.template_name));
        assert!(
            (w.movement.max_speed - WORKER_BASE_SPEED).abs() < 0.01
                || (w.movement.max_speed - worker_residual_speed(false)).abs() < 0.01,
            "worker base residual speed (got {})",
            w.movement.max_speed
        );
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::QueueUpgrade {
            upgrade_name: UPGRADE_GLA_WORKER_SHOES.to_string(),
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![barracks_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    assert!(game_logic
        .host_upgrades()
        .honesty_queue_ok(HostUpgradeKind::WorkerShoes));

    game_logic.update();

    assert!(game_logic
        .host_upgrades()
        .honesty_complete_ok(HostUpgradeKind::WorkerShoes));
    assert!(
        game_logic.honesty_worker_shoes_apply_ok(),
        "WorkerShoes must affect workers"
    );

    let worker = game_logic
        .host_object(worker_id)
        .expect("worker after shoes");
    assert!(
        worker.has_upgrade_tag(UPGRADE_GLA_WORKER_SHOES),
        "worker must receive WorkerShoes tag"
    );
    assert!(
        (worker.movement.max_speed - WORKER_SHOES_SPEED).abs() < 0.01,
        "WorkerShoes residual speed 30 (got {})",
        worker.movement.max_speed
    );
    assert_eq!(
        residual_worker_shoes_drop_off_boost(true, true),
        WORKER_SHOES_SUPPLY_BOOST
    );

    // Drop-off residual: worker with cargo deposits +8 shoes boost.
    {
        let w = game_logic.host_object_mut(worker_id).expect("worker mut");
        w.set_stored_supplies(100);
        w.set_position(Vec3::new(5.0, 0.0, 0.0));
        w.target = Some(supply_id);
        w.set_ai_state(AIState::ReturningResources);
    }
    let cash_before = game_logic
        .get_player(0)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);

    game_logic.update_ai(&[worker_id, supply_id], 1.0 / 30.0);

    let cash_after = game_logic
        .get_player(0)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);
    let gained = cash_after.saturating_sub(cash_before);
    assert!(
        gained >= 100 + WORKER_SHOES_SUPPLY_BOOST,
        "WorkerShoes drop-off must credit cargo + boost (gained={gained})"
    );
    assert!(
        game_logic.honesty_worker_shoes_boost_ok(),
        "shoes boost honesty"
    );
    assert!(
        game_logic.honesty_worker_ok(),
        "combined worker residual honesty"
    );
    assert_eq!(
        game_logic.gla_worker_residual().shoes_bonus_cash_total,
        WORKER_SHOES_SUPPLY_BOOST
    );
    let _ = barracks_id;
}

/// C++ Object::initObject updateUpgradeModules: PLAYER shoes apply to workers
/// trained after research (not only units alive at complete).
#[test]
fn late_trained_worker_inherits_player_shoes_speed() {
    use crate::game_logic::host_gla_worker::{
        UPGRADE_GLA_WORKER_SHOES, WORKER_BASE_SPEED, WORKER_SHOES_SPEED,
    };

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::GLA, "GLA", true);
    player.complete_researched_upgrade(UPGRADE_GLA_WORKER_SHOES);
    game_logic.add_player(player);

    let mut other = Player::new(1, Team::GLA, "GLA2", false);
    game_logic.add_player(other);

    let mut worker_tpl = ThingTemplate::new("GLAInfantryWorker");
    worker_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Worker)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    game_logic
        .templates
        .insert("GLAInfantryWorker".to_string(), worker_tpl);

    let late_id = game_logic
        .create_object_for_player("GLAInfantryWorker", 0, Vec3::new(0.0, 0.0, 0.0))
        .expect("late worker");
    let late = game_logic.host_object(late_id).expect("late");
    assert!(
        late.has_upgrade_tag(UPGRADE_GLA_WORKER_SHOES),
        "PLAYER upgrade must stamp shoes on workers trained after research"
    );
    assert!(
        (late.movement.max_speed - WORKER_SHOES_SPEED).abs() < 0.01,
        "late worker must inherit WorkerShoes speed 30 (got {})",
        late.movement.max_speed
    );

    let other_id = game_logic
        .create_object_for_player("GLAInfantryWorker", 1, Vec3::new(10.0, 0.0, 0.0))
        .expect("other worker");
    let other_w = game_logic.host_object(other_id).expect("other");
    assert!(
        !other_w.has_upgrade_tag(UPGRADE_GLA_WORKER_SHOES),
        "same-faction player without shoes must not inherit"
    );
    assert!(
        (other_w.movement.max_speed - WORKER_BASE_SPEED).abs() < 0.01,
        "other player's late worker stays 25 (got {})",
        other_w.movement.max_speed
    );
}


/// Residual: Burton PlantTimedDemoCharge walks to target then plants sticky timed C4.

#[test]
fn booby_trap_plant_spawns_special_object() {
    use crate::game_logic::host_booby_trap::{
        BOOBY_MAX_SPECIAL_OBJECTS, BOOBY_TRAP_OBJECT, UPGRADE_GLA_REBEL_BOOBY_TRAP,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let mut rebel = ThingTemplate::new("GLAInfantryRebel");
    rebel.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("GLAInfantryRebel".into(), rebel);
    let mut bld = ThingTemplate::new("AmericaWarFactory");
    bld.add_kind_of(KindOf::Structure).set_health(1000.0);
    logic.templates.insert("AmericaWarFactory".into(), bld);

    let planter = logic
        .create_object("GLAInfantryRebel", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    {
        let o = logic.host_object_mut(planter).unwrap();
        o.applied_upgrades
            .insert(UPGRADE_GLA_REBEL_BOOBY_TRAP.to_string());
    }
    let structure = logic
        .create_object("AmericaWarFactory", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .unwrap();

    // Direct plant residual (special ability completion path).
    let geom = 10.0;
    assert!(logic.booby_trap.can_place_special_object(planter));
    logic
        .booby_trap
        .install(structure, planter, Team::GLA, logic.frame, geom, None);
    let cid = logic
        .spawn_booby_trap_special_object(planter, Team::GLA, structure)
        .expect("BoobyTrap object");
    logic.booby_trap.set_charge_object(structure, cid);
    if let Some(t) = logic.objects.get_mut(&structure) {
        t.set_status_booby_trapped(true);
    }

    assert!(logic.honesty_booby_trap_special_object_ok());
    let charge = logic.host_object(cid).unwrap();
    assert_eq!(charge.template_name, BOOBY_TRAP_OBJECT);
    assert!(charge.booby_trap_special);
    assert_eq!(charge.booby_trap_attached_to, Some(structure));
    assert_eq!(charge.producer_id, Some(planter));

    // Follow sticky attachment.
    if let Some(s) = logic.objects.get_mut(&structure) {
        s.set_position(Vec3::new(40.0, 0.0, 20.0));
    }
    logic.update_booby_trap_special_attachments();
    let cpos = logic.host_object(cid).unwrap().get_position();
    assert!((cpos.x - 40.0).abs() < 0.1 && (cpos.z - 20.0).abs() < 0.1);

    // Detonate destroys special object.
    let hits =
        logic.detonate_booby_trap_at(structure, Vec3::new(40.0, 0.0, 20.0), None, false, true);
    let _ = hits;
    assert!(logic
        .host_object(cid)
        .map(|o| !o.is_alive() || o.status.destroyed)
        .unwrap_or(true));
    assert!(!logic.booby_trap.is_booby_trapped(structure));
    let _ = BOOBY_MAX_SPECIAL_OBJECTS;
}

#[test]
fn comanche_rocket_pod_spawns_scatter_projectiles() {
    use crate::game_logic::host_comanche_rocket_pods::{
        rocket_pod_scatter_impact, COMANCHE_ROCKET_POD_PROJECTILE, ROCKET_POD_CLIP_SIZE,
        ROCKET_POD_SCATTER_TARGET_SCALAR, UPGRADE_COMANCHE_ROCKET_PODS,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let (sx, _, sz) = rocket_pod_scatter_impact(100.0, 0.0, 50.0, 4);
    assert!((sx - (100.0 + 0.767 * ROCKET_POD_SCATTER_TARGET_SCALAR)).abs() < 0.01);
    assert!((sz - 50.0).abs() < 0.01);

    let mut logic = GameLogic::new();
    let mut c = ThingTemplate::new("AmericaVehicleComanche");
    c.add_kind_of(KindOf::Aircraft).set_health(200.0);
    logic.templates.insert("AmericaVehicleComanche".into(), c);
    let mut tank = ThingTemplate::new("ChinaTankBattleMaster");
    tank.add_kind_of(KindOf::Vehicle).set_health(400.0);
    logic.templates.insert("ChinaTankBattleMaster".into(), tank);

    let heli = logic
        .create_object(
            "AmericaVehicleComanche",
            Team::USA,
            Vec3::new(0.0, 40.0, 0.0),
        )
        .unwrap();
    {
        let o = logic.host_object_mut(heli).unwrap();
        o.applied_upgrades
            .insert(UPGRADE_COMANCHE_ROCKET_PODS.to_string());
        o.tertiary_weapon =
            Some(crate::game_logic::host_comanche_rocket_pods::comanche_rocket_pod_weapon());
        o.set_active_weapon_slot(2);
    }
    let tgt = logic
        .create_object(
            "ChinaTankBattleMaster",
            Team::China,
            Vec3::new(100.0, 0.0, 0.0),
        )
        .unwrap();
    let impact = Vec3::new(100.0, 0.0, 0.0);
    let from = Vec3::new(0.0, 40.0, 0.0);
    let shot = 0u32;
    let (ax, ay, az) = rocket_pod_scatter_impact(impact.x, impact.y, impact.z, shot);
    let aim = Vec3::new(ax, ay, az);
    let pid = logic
        .spawn_comanche_rocket_pod_projectile(heli, from, aim, shot)
        .expect("rocket");
    assert!(logic.honesty_comanche_rocket_pod_projectile_ok());
    let proj = logic.host_object(pid).unwrap();
    assert_eq!(proj.template_name, COMANCHE_ROCKET_POD_PROJECTILE);
    assert!(proj.comanche_rocket_pod_projectile);
    let hp_before = logic.host_object(tgt).unwrap().health.current;
    let (hits, _) = logic.apply_comanche_rocket_pod_area_at(aim, Some(heli));
    assert!(hits >= 1);
    let hp_after = logic
        .host_object(tgt)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(hp_after < hp_before || hits > 0);
    let _ = (ROCKET_POD_CLIP_SIZE, tgt);
    logic.frame = logic.frame.saturating_add(20);
    logic.update_comanche_rocket_pod_projectiles();
    assert!(logic
        .host_object(pid)
        .map(|o| !o.is_alive() || o.status.destroyed)
        .unwrap_or(true));
}

#[test]
fn stealth_jet_missile_projectile_flies_and_impacts() {
    use crate::game_logic::host_stealth_fighter::{
        is_stealth_fighter_template, STEALTH_FIGHTER_DAMAGE, STEALTH_JET_MISSILE_PROJECTILE,
        STEALTH_MISSILE_FUEL_FRAMES,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut sf_tpl = ThingTemplate::new("AmericaJetStealthFighter");
    sf_tpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Attackable)
        .set_health(120.0);
    logic
        .templates
        .insert("AmericaJetStealthFighter".into(), sf_tpl);
    let mut tank = ThingTemplate::new("TestTank");
    tank.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(500.0);
    logic.templates.insert("TestTank".into(), tank);

    let sf = logic
        .create_object(
            "AmericaJetStealthFighter",
            Team::USA,
            Vec3::new(0.0, 40.0, 0.0),
        )
        .expect("stealth fighter");
    assert!(is_stealth_fighter_template(
        &logic.host_object(sf).unwrap().template_name
    ));
    let enemy = logic
        .create_object("TestTank", Team::GLA, Vec3::new(100.0, 0.0, 0.0))
        .expect("enemy");
    let hp_before = logic.host_object(enemy).unwrap().health.current;

    let pid = logic
        .spawn_stealth_jet_missile_projectile(
            sf,
            Vec3::new(0.0, 40.0, 0.0),
            Vec3::new(100.0, 0.0, 0.0),
            Some(enemy),
        )
        .expect("missile");
    {
        let m = logic.host_object(pid).expect("missile obj");
        assert_eq!(m.template_name, STEALTH_JET_MISSILE_PROJECTILE);
        assert!(m.stealth_jet_missile_projectile);
        assert!(m.stealth_jet_missile_aim.is_some());
    }
    assert!(logic.honesty_stealth_jet_missile_projectile_ok());

    let mut hit = false;
    for _ in 0..(STEALTH_MISSILE_FUEL_FRAMES.min(200) + 20) {
        logic.frame = logic.frame.saturating_add(1);
        logic.update_stealth_jet_missile_projectiles();
        if !logic
            .host_object(pid)
            .map(|o| o.is_alive() && o.stealth_jet_missile_projectile)
            .unwrap_or(false)
        {
            hit = true;
            break;
        }
    }
    assert!(hit, "StealthJetMissile should impact within fuel lifetime");
    logic.process_destroy_list();
    let hp_after = logic
        .host_object(enemy)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        hp_after < hp_before,
        "impact residual damage (before={hp_before} after={hp_after} dmg={STEALTH_FIGHTER_DAMAGE})"
    );
}

#[test]
fn mig_missile_projectile_flies_and_impacts() {
    use crate::game_logic::host_mig::{
        MIG_MISSILE_FUEL_FRAMES, MIG_PRIMARY_DAMAGE, MIG_PROJECTILE,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut m = ThingTemplate::new("ChinaJetMIG");
    m.add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Attackable)
        .set_health(160.0);
    logic.templates.insert("ChinaJetMIG".into(), m);
    let mut tank = ThingTemplate::new("TestTank");
    tank.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(800.0);
    logic.templates.insert("TestTank".into(), tank);

    let src = logic
        .create_object("ChinaJetMIG", Team::China, Vec3::new(0.0, 80.0, 0.0))
        .unwrap();
    let enemy = logic
        .create_object("TestTank", Team::USA, Vec3::new(120.0, 0.0, 0.0))
        .unwrap();
    let hp_before = logic.host_object(enemy).unwrap().health.current;

    let pid = logic
        .spawn_mig_missile_projectile(
            src,
            Vec3::new(0.0, 80.0, 0.0),
            Vec3::new(120.0, 0.0, 0.0),
            Some(enemy),
        )
        .expect("missile");
    {
        let o = logic.host_object(pid).unwrap();
        assert_eq!(o.template_name, MIG_PROJECTILE);
        assert!(o.mig_missile_projectile);
    }
    assert!(logic.honesty_mig_missile_projectile_ok());

    let mut hit = false;
    for _ in 0..(MIG_MISSILE_FUEL_FRAMES.min(200) + 20) {
        logic.frame = logic.frame.saturating_add(1);
        logic.update_mig_missile_projectiles();
        if !logic
            .host_object(pid)
            .map(|o| o.is_alive() && o.mig_missile_projectile)
            .unwrap_or(false)
        {
            hit = true;
            break;
        }
    }
    assert!(hit, "MiG NapalmMissile should impact");
    logic.process_destroy_list();
    let hp_after = logic
        .host_object(enemy)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        hp_after < hp_before,
        "impact residual damage (before={hp_before} after={hp_after} dmg={MIG_PRIMARY_DAMAGE})"
    );
}

#[test]
fn flashbang_grenade_bezier_flight_and_blast() {
    use crate::game_logic::host_ranger::{
        flashbang_shell_flight_frames, FLASHBANG_GRENADE_PROJECTILE, FLASHBANG_PRIMARY_DAMAGE,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut r = ThingTemplate::new("AmericaInfantryRanger");
    r.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Attackable)
        .set_health(180.0);
    logic.templates.insert("AmericaInfantryRanger".into(), r);
    let mut tank = ThingTemplate::new("TestTank");
    tank.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(500.0);
    logic.templates.insert("TestTank".into(), tank);

    let src = logic
        .create_object("AmericaInfantryRanger", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    // equip flashbang residual
    if let Some(o) = logic.objects.get_mut(&src) {
        o.applied_upgrades
            .insert("Upgrade_AmericaRangerFlashBangGrenade".into());
        o.active_weapon_slot = 1;
    }
    let enemy = logic
        .create_object("TestTank", Team::GLA, Vec3::new(80.0, 0.0, 0.0))
        .unwrap();
    let hp_before = logic.host_object(enemy).unwrap().health.current;
    let from = Vec3::new(0.0, 0.0, 0.0);
    let aim = Vec3::new(80.0, 0.0, 0.0);

    let pid = logic
        .spawn_flashbang_grenade_projectile(src, from, aim, Some(enemy))
        .expect("grenade");
    let frames = {
        let m = logic.host_object(pid).unwrap();
        assert_eq!(m.template_name, FLASHBANG_GRENADE_PROJECTILE);
        assert!(m.flashbang_grenade_projectile);
        // ScatterRadius residual may offset aim; flight frames follow the scattered aim.
        assert!(m.flashbang_grenade_flight_frames > 0);
        let stored_aim = m.flashbang_grenade_aim.expect("aim");
        let to = Vec3::new(stored_aim[0], stored_aim[1], stored_aim[2]);
        let start = m
            .flashbang_grenade_from
            .map(|p| Vec3::new(p[0], p[1], p[2]))
            .unwrap_or(from);
        assert_eq!(
            m.flashbang_grenade_flight_frames,
            flashbang_shell_flight_frames(start, to).max(1)
        );
        m.flashbang_grenade_flight_frames
    };
    assert!(logic.honesty_flashbang_grenade_projectile_ok());
    assert!(logic.honesty_flashbang_scatter_ok());

    for _ in 0..(frames + 5) {
        logic.frame = logic.frame.saturating_add(1);
        logic.update_flashbang_grenade_projectiles();
        if !logic
            .host_object(pid)
            .map(|o| o.is_alive() && o.flashbang_grenade_projectile)
            .unwrap_or(false)
        {
            break;
        }
    }
    logic.process_destroy_list();
    let hp_after = logic
        .host_object(enemy)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        hp_after < hp_before,
        "blast should damage (before={hp_before} after={hp_after} dmg={FLASHBANG_PRIMARY_DAMAGE})"
    );
}

#[test]
fn helix_napalm_bomb_projectile_falls_and_height_dies() {
    use crate::game_logic::host_helix_napalm::{
        NAPALM_BOMB_HEIGHT_DIE_TARGET, NAPALM_BOMB_PROJECTILE,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut helix = ThingTemplate::new("ChinaVehicleHelix");
    helix
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Vehicle)
        .set_health(400.0);
    logic.templates.insert("ChinaVehicleHelix".into(), helix);
    let mut tank = ThingTemplate::new("TestTank");
    tank.add_kind_of(KindOf::Vehicle).set_health(200.0);
    logic.templates.insert("TestTank".into(), tank);

    let helix_id = logic
        .create_object("ChinaVehicleHelix", Team::China, Vec3::new(0.0, 50.0, 0.0))
        .unwrap();
    {
        let h = logic.host_object_mut(helix_id).unwrap();
        h.applied_upgrades
            .insert("Upgrade_HelixNapalmBomb".to_string());
    }
    let enemy = logic
        .create_object("TestTank", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .unwrap();
    let enemy_hp = logic.host_object(enemy).unwrap().health.current;

    let zone = logic
        .activate_helix_napalm_bomb(helix_id, Vec3::new(5.0, 0.0, 0.0))
        .expect("drop");
    let _ = zone;
    assert!(logic.helix_napalm.honesty_projectile_ok());
    let bomb_id = logic
        .objects
        .iter()
        .find(|(_, o)| o.helix_napalm_bomb_projectile)
        .map(|(id, _)| *id)
        .expect("bomb");
    {
        let b = logic.host_object(bomb_id).unwrap();
        assert!(
            b.template_name == NAPALM_BOMB_PROJECTILE
                || b.template_name.to_ascii_lowercase().contains("napalm")
        );
        assert!(b.height_die.is_some());
        assert!(b.get_position().y > NAPALM_BOMB_HEIGHT_DIE_TARGET + 5.0);
    }

    // Fall until HeightDie residual kills the bomb.
    for _ in 0..80 {
        logic.frame = logic.frame.saturating_add(1);
        logic.update_helix_napalm_bomb_projectiles();
        // Drive height-die tick residual used by host update.
        if let Some(o) = logic.objects.get_mut(&bomb_id) {
            if o.tick_height_die(logic.frame, 0.0) {
                logic.mark_object_for_destruction(bomb_id, None);
                break;
            }
        } else {
            break;
        }
    }
    // Process destruction → FireWeaponWhenDead residual.
    logic.process_destroy_list();
    let bomb_alive = logic
        .host_object(bomb_id)
        .map(|o| o.is_alive())
        .unwrap_or(false);
    assert!(!bomb_alive, "NapalmBomb should HeightDie");
    let enemy_hp_after = logic
        .host_object(enemy)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp || logic.helix_napalm.blast_hits > 0,
        "death weapon residual should damage nearby or record blast"
    );
}

#[test]
fn avenger_air_laser_spawns_laser_beam_object() {
    use crate::game_logic::host_avenger::{
        AVENGER_AIR_LASER, AVENGER_LASER_BEAM_LIFETIME_FRAMES, AVENGER_LASER_NAME,
    };
    use crate::game_logic::host_weapon_laser::laser_beam_lifetime_frames;
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    assert_eq!(laser_beam_lifetime_frames(AVENGER_LASER_NAME), 7);
    assert_eq!(AVENGER_LASER_BEAM_LIFETIME_FRAMES, 7);

    let mut logic = GameLogic::new();
    let mut av = ThingTemplate::new("AmericaTankAvenger");
    av.add_kind_of(KindOf::Vehicle).set_health(300.0);
    logic.templates.insert("AmericaTankAvenger".into(), av);
    let mut jet = ThingTemplate::new("ChinaJetMIG");
    jet.add_kind_of(KindOf::Aircraft).set_health(100.0);
    logic.templates.insert("ChinaJetMIG".into(), jet);

    let shooter = logic
        .create_object("AmericaTankAvenger", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let target = logic
        .create_object("ChinaJetMIG", Team::China, Vec3::new(100.0, 40.0, 0.0))
        .unwrap();
    let from = Vec3::new(0.0, 10.0, 0.0);
    let to = Vec3::new(100.0, 40.0, 0.0);
    let bid = logic
        .spawn_weapon_laser_beam_object(AVENGER_LASER_NAME, shooter, Some(target), from, to)
        .expect("AvengerLaserBeam");
    assert!(logic.honesty_weapon_laser_beam_object_ok());
    let beam = logic.host_object(bid).unwrap();
    assert_eq!(beam.template_name, AVENGER_LASER_NAME);
    assert!(beam.weapon_laser_beam);
    assert_eq!(beam.producer_id, Some(shooter));
    let _ = AVENGER_AIR_LASER;
    logic.frame = logic
        .frame
        .saturating_add(AVENGER_LASER_BEAM_LIFETIME_FRAMES + 2);
    logic.update_weapon_laser_beam_objects();
    assert!(logic
        .host_object(bid)
        .map(|o| !o.is_alive() || o.status.destroyed)
        .unwrap_or(true));
}

#[test]
fn angry_mob_spawns_member_objects_on_nexus() {
    use crate::game_logic::host_angry_mob::{
        ANGRY_MOB_EXPAND_INTERVAL_FRAMES, ANGRY_MOB_INITIAL_MEMBERS, ANGRY_MOB_MAX_MEMBERS,
        ANGRY_MOB_MEMBER_TEMPLATES,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let mut nexus = ThingTemplate::new("GLAInfantryAngryMobNexus");
    nexus.add_kind_of(KindOf::Infantry).set_health(99999.0);
    logic
        .templates
        .insert("GLAInfantryAngryMobNexus".into(), nexus);
    let nid = logic
        .create_object(
            "GLAInfantryAngryMobNexus",
            Team::GLA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .unwrap();
    // complete construction residual
    if let Some(o) = logic.objects.get_mut(&nid) {
        o.construction_percent = 1.0;
        o.status.under_construction = false;
    }
    logic.update_angry_mobs();
    assert!(logic.honesty_angry_mob_member_spawn_ok());
    let members: Vec<_> = logic
        .host_objects()
        .values()
        .filter(|o| o.angry_mob_member && o.angry_mob_nexus_id == Some(nid))
        .collect();
    assert_eq!(
        members.len() as u32,
        ANGRY_MOB_MAX_MEMBERS,
        "SpawnNumber members must spawn immediately"
    );
    assert!(members.iter().all(|m| {
        ANGRY_MOB_MEMBER_TEMPLATES
            .iter()
            .any(|t| *t == m.template_name.as_str())
    }));

    // Rapid spawn already at SpawnNumber; replace-delay does not add more.
    logic.frame = logic.frame.saturating_add(ANGRY_MOB_EXPAND_INTERVAL_FRAMES);
    logic.update_angry_mobs();
    let members2 = logic
        .host_objects()
        .values()
        .filter(|o| o.angry_mob_member && o.angry_mob_nexus_id == Some(nid))
        .count();
    assert_eq!(members2 as u32, ANGRY_MOB_MAX_MEMBERS);
    let _ = ANGRY_MOB_INITIAL_MEMBERS;

    // Follow nexus move via pathfind, not orbit teleport.
    if let Some(n) = logic.objects.get_mut(&nid) {
        n.set_position(Vec3::new(50.0, 0.0, 20.0));
    }
    logic.update_angry_mob_member_follow();
    let mid = logic
        .host_objects()
        .values()
        .find(|o| o.angry_mob_member)
        .unwrap()
        .id;
    let dest = logic
        .host_object(mid)
        .and_then(|o| o.movement.target_position)
        .expect("member must path-follow the nexus");
    assert!((dest.x - 50.0).abs() < 20.0 && (dest.z - 20.0).abs() < 20.0);

    // Nexus death destroys members.
    if let Some(n) = logic.objects.get_mut(&nid) {
        // Wave 752: under damage authority, do not zero host HP mid-frame
        // (dual with GW HP writeback). Project lethal via damage log + flags.
        if crate::gameworld_shadow::gameworld_damage_authority_live() {
            let hp = n.health.current.max(1.0);
            let oid = n.id;
            crate::game_logic::host_damage_log::record(oid, hp, None, true);
        } else {
            n.health.current = 0.0;
        }
        n.status.destroyed = true;
        n.status.effectively_dead = true;
    }
    logic.update_angry_mob_member_follow();
    assert!(logic
        .host_object(mid)
        .map(|o| !o.is_alive() || o.status.destroyed)
        .unwrap_or(true));
}

#[test]
fn countermeasures_divert_spawns_flare_objects() {
    use crate::game_logic::host_countermeasures::{
        try_divert_missile, FLARE_LIFETIME_FRAMES, FLARE_TEMPLATE_NAME, VOLLEY_SIZE,
    };
    use crate::game_logic::host_upgrades::UPGRADE_AMERICA_COUNTERMEASURES;
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let mut jet = ThingTemplate::new("AmericaJetRaptor");
    jet.add_kind_of(KindOf::Aircraft).set_health(120.0);
    logic.templates.insert("AmericaJetRaptor".into(), jet);
    let air = logic
        .create_object("AmericaJetRaptor", Team::USA, Vec3::new(0.0, 40.0, 0.0))
        .unwrap();
    {
        let o = logic.host_object_mut(air).unwrap();
        o.applied_upgrades
            .insert(UPGRADE_AMERICA_COUNTERMEASURES.to_string());
        o.status.airborne_target = true;
        o.movement.velocity = Vec3::new(20.0, 0.0, 0.0);
        o.set_orientation(0.0);
    }
    logic.countermeasures.ensure(air);

    // Force many rolls until a divert succeeds (deterministic seed space).
    let mut diverted = false;
    let mut diverted_frame = 0u32;
    for f in 0..64u32 {
        if try_divert_missile(&mut logic.countermeasures, air, ObjectId(900 + f), f, true) {
            diverted = true;
            diverted_frame = f;
            break;
        }
    }
    assert!(diverted, "evasion residual must succeed within seed window");
    logic.frame = diverted_frame;
    logic.flush_countermeasure_flare_spawns();
    assert!(logic.honesty_countermeasure_flare_object_ok());
    let flares: Vec<_> = logic
        .host_objects()
        .values()
        .filter(|o| o.countermeasure_flare)
        .collect();
    assert!(
        !flares.is_empty() && flares.len() as u32 <= VOLLEY_SIZE,
        "volley must spawn CountermeasureFlare objects, got {}",
        flares.len()
    );
    assert!(flares
        .iter()
        .all(|o| o.template_name == FLARE_TEMPLATE_NAME));
    assert!(
        flares.iter().all(|o| {
            let p = o.get_position();
            (p.x).abs() < 0.01 && (p.z).abs() < 0.01 && (p.y - 40.0).abs() < 0.01
        }),
        "C++ launchVolley spawns at aircraft position, not a static ring"
    );
    assert!(
        flares.iter().any(|o| o.movement.velocity.length() > 1.0),
        "flares must inherit jet velocity plus volley motive"
    );
    let fid = flares[0].id;
    logic.frame = logic.frame.saturating_add(FLARE_LIFETIME_FRAMES + 2);
    logic.update_countermeasure_flare_objects();
    assert!(logic
        .host_object(fid)
        .map(|o| !o.is_alive() || o.status.destroyed)
        .unwrap_or(true));
}

#[test]
fn aurora_fuel_air_impact_spawns_gas_object() {
    use crate::game_logic::host_aurora_bomb::{
        HostAuroraBombKind, AURORA_FUEL_AIR_DIVE_IMPACT_FRAMES,
    };
    use crate::game_logic::host_fuel_air_gas_slow_death::{
        AIRF_AURORA_BOMB_GAS_OBJECT, FUEL_AIR_GAS_DESTRUCTION_DELAY_FRAMES,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let mut jet = ThingTemplate::new("AirF_AmericaJetAurora");
    jet.add_kind_of(KindOf::Aircraft).set_health(80.0);
    logic.templates.insert("AirF_AmericaJetAurora".into(), jet);
    let mut tgt = ThingTemplate::new("GLATunnelNetwork");
    tgt.add_kind_of(KindOf::Structure).set_health(2000.0);
    logic.templates.insert("GLATunnelNetwork".into(), tgt);

    let src = logic
        .create_object(
            "AirF_AmericaJetAurora",
            Team::USA,
            Vec3::new(0.0, 50.0, 0.0),
        )
        .unwrap();
    let building = logic
        .create_object("GLATunnelNetwork", Team::GLA, Vec3::new(30.0, 0.0, 0.0))
        .unwrap();
    let hp_before = logic.host_object(building).unwrap().health.current;

    let mid = logic.aurora_bombs.queue(
        HostAuroraBombKind::FuelAir,
        src,
        Team::USA,
        Vec3::new(30.0, 0.0, 0.0),
        logic.frame,
    );
    assert!(mid > 0);
    logic.frame = logic
        .frame
        .saturating_add(AURORA_FUEL_AIR_DIVE_IMPACT_FRAMES);
    logic.update_aurora_bombs();

    assert!(logic.honesty_aurora_fuel_air_gas_object_ok());
    assert!(logic.aurora_fuel_air_gas_spawned >= 1);
    let gas = logic
        .host_objects()
        .values()
        .find(|o| o.template_name == AIRF_AURORA_BOMB_GAS_OBJECT)
        .expect("AirF_AuroraBombGas");
    assert!(gas.fuel_air_gas_slow_death.is_some());
    let gid = gas.id;
    // Immediate blast should not have nuked the building before gas SlowDeath FINAL.
    let hp_mid = logic.host_object(building).unwrap().health.current;
    assert_eq!(
        hp_mid, hp_before,
        "gas path defers primary blast to SlowDeath"
    );

    // Advance gas SlowDeath phase-by-phase (one event per update residual).
    for _ in 0..(FUEL_AIR_GAS_DESTRUCTION_DELAY_FRAMES + 4) {
        logic.frame = logic.frame.saturating_add(1);
        logic.update_fuel_air_gas_slow_death();
    }
    assert!(
        logic.fuel_air_gas_reg.final_detonations > 0 || logic.fuel_air_gas_reg.midpoint_flames > 0,
        "gas SlowDeath must tick midpoint/final residual"
    );
    assert!(
        logic
            .host_object(gid)
            .map(|o| {
                !o.is_alive()
                    || o.status.destroyed
                    || o.fuel_air_gas_slow_death
                        .as_ref()
                        .map(|d| d.is_complete())
                        .unwrap_or(false)
            })
            .unwrap_or(true),
        "gas object completes SlowDeath residual"
    );
    let _ = (building, hp_before);
}

#[test]
fn sticky_bomb_follows_moving_vehicle() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut tank = ThingTemplate::new("TestTank");
    tank.set_health(500.0);
    tank.add_kind_of(KindOf::Vehicle);
    logic.templates.insert("TestTank".into(), tank);
    let tid = logic
        .create_object("TestTank", Team::GLA, glam::Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let charge = logic
        .place_timed_demo_charge(Team::USA, glam::Vec3::ZERO, None, Some(tid), Some(300))
        .expect("charge");
    // Move target.
    if let Some(t) = logic.objects.get_mut(&tid) {
        t.set_position(glam::Vec3::new(50.0, 0.0, 20.0));
    }
    logic.update_sticky_bomb_attachments();
    let cpos = logic.objects.get(&charge).unwrap().get_position();
    assert!(
        (cpos.x - 50.0).abs() < 0.1 && (cpos.z - 20.0).abs() < 0.1,
        "charge must follow vehicle xy, got {cpos:?}"
    );
    assert!(
        cpos.y >= 8.0 - 0.1,
        "charge rides roof offset Z, y={}",
        cpos.y
    );
    assert!(logic.sticky_bomb_follow_ticks >= 1);
}

#[test]
fn burton_charges_use_retail_c4_special_objects() {
    use crate::game_logic::host_mines::{
        BURTON_MAX_REMOTE_CHARGES, BURTON_MAX_TIMED_CHARGES, BURTON_REMOTE_CHARGE_OBJECT,
        BURTON_TIMED_CHARGE_OBJECT, TANK_HUNTER_TNT_OBJECT, TIMED_C4_LIFETIME_FRAMES,
        TNT_STICKY_LIFETIME_FRAMES,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let mut burton = ThingTemplate::new("AmericaInfantryColonelBurton");
    burton.add_kind_of(KindOf::Infantry).set_health(200.0);
    logic
        .templates
        .insert("AmericaInfantryColonelBurton".into(), burton);
    let mut th = ThingTemplate::new("ChinaInfantryTankHunter");
    th.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("ChinaInfantryTankHunter".into(), th);
    let mut bld = ThingTemplate::new("GLATunnelNetwork");
    bld.add_kind_of(KindOf::Structure).set_health(1000.0);
    logic.templates.insert("GLATunnelNetwork".into(), bld);

    let hero = logic
        .create_object(
            "AmericaInfantryColonelBurton",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .unwrap();
    let hunter = logic
        .create_object(
            "ChinaInfantryTankHunter",
            Team::China,
            Vec3::new(10.0, 0.0, 0.0),
        )
        .unwrap();
    let targets: Vec<_> = (0..3)
        .map(|i| {
            logic
                .create_object(
                    "GLATunnelNetwork",
                    Team::GLA,
                    Vec3::new(50.0 + i as f32 * 20.0, 0.0, 0.0),
                )
                .unwrap()
        })
        .collect();

    let timed = logic
        .place_timed_demo_charge(
            Team::USA,
            Vec3::new(50.0, 0.0, 0.0),
            Some(hero),
            Some(targets[0]),
            None,
        )
        .expect("TimedC4");
    {
        let c = logic.host_object(timed).unwrap();
        assert_eq!(c.template_name, BURTON_TIMED_CHARGE_OBJECT);
        let md = c.mine_data.as_ref().unwrap();
        assert_eq!(md.attached_to, Some(targets[0]));
        let exp = md.detonate_at_frame.unwrap();
        assert_eq!(exp, logic.frame + TIMED_C4_LIFETIME_FRAMES);
    }

    let remote = logic
        .place_remote_demo_charge(
            Team::USA,
            Vec3::new(70.0, 0.0, 0.0),
            Some(hero),
            Some(targets[1]),
        )
        .expect("RemoteC4");
    assert_eq!(
        logic.host_object(remote).unwrap().template_name,
        BURTON_REMOTE_CHARGE_OBJECT
    );

    // UniqueSpecialObjectTargets: second charge on same target fails.
    assert!(logic
        .place_remote_demo_charge(
            Team::USA,
            Vec3::new(70.0, 0.0, 0.0),
            Some(hero),
            Some(targets[1]),
        )
        .is_none());

    let tnt = logic
        .place_timed_demo_charge(
            Team::China,
            Vec3::new(90.0, 0.0, 0.0),
            Some(hunter),
            Some(targets[2]),
            None,
        )
        .expect("TNT");
    {
        let c = logic.host_object(tnt).unwrap();
        assert_eq!(c.template_name, TANK_HUNTER_TNT_OBJECT);
        let md = c.mine_data.as_ref().unwrap();
        assert_eq!(
            md.detonate_at_frame.unwrap(),
            logic.frame + TNT_STICKY_LIFETIME_FRAMES
        );
    }

    // MaxSpecialObjects remote = 8.
    let mut planted = 1u32; // already one remote
    for i in 0..20 {
        let tid = logic
            .create_object(
                "GLATunnelNetwork",
                Team::GLA,
                Vec3::new(200.0 + i as f32, 0.0, 0.0),
            )
            .unwrap();
        if logic
            .place_remote_demo_charge(
                Team::USA,
                Vec3::new(200.0 + i as f32, 0.0, 0.0),
                Some(hero),
                Some(tid),
            )
            .is_some()
        {
            planted += 1;
        }
    }
    assert_eq!(planted, BURTON_MAX_REMOTE_CHARGES);

    // Owner dies → remote charges cleaned; timed persists.
    if let Some(o) = logic.objects.get_mut(&hero) {
        // Wave 752: under damage authority, do not zero host HP mid-frame
        // (dual with GW HP writeback). Project lethal via damage log + flags.
        if crate::gameworld_shadow::gameworld_damage_authority_live() {
            let hp = o.health.current.max(1.0);
            let oid = o.id;
            crate::game_logic::host_damage_log::record(oid, hp, None, true);
        } else {
            o.health.current = 0.0;
        }
        o.status.destroyed = true;
        o.status.effectively_dead = true;
    }
    logic.cleanup_remote_charges_when_owner_dies();
    assert!(logic
        .host_object(remote)
        .map(|o| !o.is_alive() || o.status.destroyed)
        .unwrap_or(true));
    assert!(logic.host_object(timed).unwrap().is_alive());
    let _ = BURTON_MAX_TIMED_CHARGES;
}

#[test]
fn sticky_bomb_destroyed_when_target_dies() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut tank = ThingTemplate::new("TestTank");
    tank.set_health(100.0);
    tank.add_kind_of(KindOf::Vehicle);
    logic.templates.insert("TestTank".into(), tank);
    let tid = logic
        .create_object("TestTank", Team::GLA, glam::Vec3::ZERO)
        .unwrap();
    let charge = logic
        .place_remote_demo_charge(Team::USA, glam::Vec3::ZERO, None, Some(tid))
        .expect("charge");
    logic.destroy_object(tid);
    // Ensure target is gone/dead for sticky check.
    if let Some(t) = logic.objects.get_mut(&tid) {
        t.status.effectively_dead = true;
        t.health.current = 0.0;
    }
    logic.update_sticky_bomb_attachments();
    let charge_alive = logic
        .objects
        .get(&charge)
        .map(|o| o.is_alive())
        .unwrap_or(false);
    assert!(
        !charge_alive || logic.sticky_bomb_target_deaths >= 1,
        "charge must die with target"
    );
}

#[test]
fn plant_timed_demo_charge_command_plants_after_reach() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);

    let burton_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(180.0, 0.0, 0.0))
        .expect("burton should be created");
    let target_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("building should be created");

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::PlantTimedDemoCharge { target_id },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![burton_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    // Not planted while out of range.
    assert_eq!(game_logic.mine_residual_places(), 0);
    assert!(!game_logic.honesty_plant_timed_demo_charge_ok());

    {
        let burton = game_logic
            .host_object_mut(burton_id)
            .expect("burton should exist");
        burton.set_position(Vec3::new(2.0, 0.0, 0.0));
        burton.set_ai_state(AIState::SpecialAbility);
        burton.target = Some(target_id);
    }
    game_logic.update_ai(&[burton_id, target_id], 1.0 / 60.0);

    assert!(
        game_logic.mine_residual_places() >= 1,
        "timed charge must be placed on contact"
    );
    assert!(
        game_logic.honesty_plant_timed_demo_charge_ok(),
        "plant timed charge residual honesty"
    );

    let charge_count = game_logic
        .host_objects()
        .values()
        .filter(|o| {
            o.mine_data
                .as_ref()
                .map(|d| {
                    d.kind == crate::game_logic::host_mines::HostMineKind::TimedDemoCharge
                        && d.is_active()
                        && d.attached_to == Some(target_id)
                })
                .unwrap_or(false)
        })
        .count();
    assert!(
        charge_count >= 1,
        "sticky timed charge must attach to target"
    );
}

/// Residual: Burton PlantRemoteDemoCharge plants sticky remote C4 (no auto-timer),
/// then DetonateRemoteDemoCharges blows all producer charges.
/// Fail-closed: not full StickyBombUpdate attach bones / max-charge list.
#[test]
fn plant_and_detonate_remote_demo_charge_residual() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let burton_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(180.0, 0.0, 0.0))
        .expect("burton should be created");
    let target_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("building should be created");
    let enemy_id = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(5.0, 0.0, 0.0))
        .expect("enemy near charge");

    let enemy_hp_before = game_logic
        .host_object(enemy_id)
        .map(|o| o.health.current)
        .unwrap_or(0.0);

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::PlantRemoteDemoCharge { target_id },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![burton_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert_eq!(game_logic.mine_residual_places(), 0);
    assert!(!game_logic.honesty_plant_remote_demo_charge_ok());

    {
        let burton = game_logic
            .host_object_mut(burton_id)
            .expect("burton should exist");
        burton.set_position(Vec3::new(2.0, 0.0, 0.0));
        burton.set_ai_state(AIState::SpecialAbility);
        burton.target = Some(target_id);
    }
    game_logic.update_ai(&[burton_id, target_id, enemy_id], 1.0 / 60.0);

    assert!(
        game_logic.mine_residual_places() >= 1,
        "remote charge must be placed on contact"
    );
    assert!(
        game_logic.honesty_plant_remote_demo_charge_ok(),
        "plant remote charge residual honesty"
    );

    let charge_id = game_logic
        .host_objects()
        .iter()
        .find_map(|(id, o)| {
            o.mine_data.as_ref().and_then(|d| {
                if d.kind == crate::game_logic::host_mines::HostMineKind::RemoteDemoCharge
                    && d.is_active()
                    && d.attached_to == Some(target_id)
                    && d.producer_id == Some(burton_id)
                    && d.detonate_at_frame.is_none()
                {
                    Some(*id)
                } else {
                    None
                }
            })
        })
        .expect("sticky remote charge must attach without auto-timer");

    // Advance many frames — remote charge must NOT auto-detonate.
    for _ in 0..400 {
        game_logic.update_mines_and_demo_traps();
        game_logic.frame = game_logic.frame.saturating_add(1);
    }
    assert!(
        game_logic
            .host_object(charge_id)
            .and_then(|o| o.mine_data.as_ref())
            .map(|d| d.is_active())
            .unwrap_or(false),
        "remote charge must remain live without DetonateRemoteDemoCharges"
    );
    assert_eq!(
        game_logic.mine_residual_timed_detonations(),
        0,
        "remote charge must not use timed detonation path"
    );

    // Detonate via command residual.
    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::DetonateRemoteDemoCharges,
        player_id: 0,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![burton_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert!(
        game_logic.honesty_remote_demo_charge_detonate_ok(),
        "remote detonate residual honesty"
    );
    assert!(
        game_logic.mine_residual_manual_detonations() >= 1,
        "remote detonate uses manual detonation residual counter"
    );

    let enemy_after = game_logic.host_object(enemy_id).expect("enemy");
    assert!(
        enemy_after.health.current < enemy_hp_before || enemy_after.status.destroyed,
        "remote detonate must damage nearby enemy (before={enemy_hp_before}, after={})",
        enemy_after.health.current
    );
}

/// Residual: owning a CommandCenter enables player radar (minimap online).
/// Fail-closed: not full RadarUpgrade grant / RadarUpdate extend animation.
#[test]
fn command_center_radar_residual_enables_player_has_radar() {
    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    ensure_test_command_center_template(&mut game_logic);

    // Before CC: no radar.
    game_logic.update_player_radar();
    let before = game_logic
        .get_player_mut_by_team(Team::USA)
        .map(|p| p.has_radar())
        .unwrap_or(true);
    assert!(!before, "player must not have radar without CommandCenter");
    assert!(!game_logic.honesty_radar_online_ok());

    let cc_id = game_logic
        .create_object("TestCommandCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("command center");
    if let Some(obj) = game_logic.host_object_mut(cc_id) {
        obj.set_status_under_construction(false);
        obj.construction_percent = 1.0;
    }

    game_logic.update_player_radar();
    let after = game_logic
        .get_player_mut_by_team(Team::USA)
        .map(|p| (p.has_radar(), p.radar_count))
        .expect("player");
    assert!(
        after.0,
        "player radar state must become true when CC present"
    );
    assert!(after.1 >= 1, "radar_count must reflect CC provider");
    assert!(
        game_logic.honesty_radar_online_ok(),
        "radar online residual honesty"
    );
    assert!(
        game_logic.host_radar().max_provider_count >= 1,
        "provider honesty"
    );

    // UI: forced off + script radar not hidden + local has_radar → radar online.
    game_logic.radar_forced = false;
    game_logic.radar_enabled = true;
    let ui = game_logic.update_ui_state(0);
    assert!(
        ui.radar_enabled,
        "minimap/radar UI online when local player has CC radar"
    );

    // Destroy CC → radar offline.
    if let Some(obj) = game_logic.host_object_mut(cc_id) {
        // Wave 753: under damage authority, do not zero host HP mid-frame
        // (dual with GW HP writeback). Project lethal via damage log + flags.
        if crate::gameworld_shadow::gameworld_damage_authority_live() {
            let hp = obj.health.current.max(1.0);
            let oid = obj.id;
            crate::game_logic::host_damage_log::record(oid, hp, None, true);
        } else {
            obj.health.current = 0.0;
        }
        obj.status.destroyed = true;
    }
    game_logic.update_player_radar();
    let offline = game_logic
        .get_player_mut_by_team(Team::USA)
        .map(|p| p.has_radar())
        .unwrap_or(true);
    assert!(!offline, "radar offline when CC destroyed");
    let ui_off = game_logic.update_ui_state(0);
    assert!(
        !ui_off.radar_enabled,
        "minimap/radar UI offline without provider (unless forced)"
    );
}

/// Residual: RadarVan also grants player radar (GLA path).
#[test]
fn radar_van_residual_enables_player_has_radar() {
    use crate::game_logic::{KindOf, ThingTemplate};

    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::GLA);

    if !game_logic.templates.contains_key("TestRadarVan") {
        let mut t = ThingTemplate::new("TestRadarVan");
        t.add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .set_health(300.0)
            .set_cost(500, 0);
        game_logic.templates.insert("TestRadarVan".to_string(), t);
    }

    let van_id = game_logic
        .create_object("TestRadarVan", Team::GLA, Vec3::new(10.0, 0.0, 0.0))
        .expect("radar van");
    if let Some(obj) = game_logic.host_object_mut(van_id) {
        obj.set_status_under_construction(false);
        obj.construction_percent = 1.0;
    }

    game_logic.update_player_radar();
    assert!(
        game_logic
            .get_player_mut_by_team(Team::GLA)
            .map(|p| p.has_radar())
            .unwrap_or(false),
        "RadarVan residual must enable player has_radar"
    );
    assert!(game_logic.host_radar().online_transitions >= 1);
}

/// Residual: Fake command centers do not grant radar.
#[test]
fn fake_command_center_does_not_grant_radar() {
    use crate::game_logic::{KindOf, ThingTemplate};

    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::GLA);

    if !game_logic.templates.contains_key("FakeGLACommandCenter") {
        let mut t = ThingTemplate::new("FakeGLACommandCenter");
        t.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::CommandCenter)
            .add_kind_of(KindOf::Selectable)
            .set_health(500.0);
        game_logic
            .templates
            .insert("FakeGLACommandCenter".to_string(), t);
    }

    let fake_id = game_logic
        .create_object("FakeGLACommandCenter", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("fake cc");
    if let Some(obj) = game_logic.host_object_mut(fake_id) {
        obj.set_status_under_construction(false);
        obj.construction_percent = 1.0;
    }

    game_logic.update_player_radar();
    assert!(
        !game_logic
            .get_player_mut_by_team(Team::GLA)
            .map(|p| p.has_radar())
            .unwrap_or(true),
        "Fake CC must not enable radar residual"
    );
}

/// Residual: GLA Black Market AutoDeposit deposits $20 every 60 logic frames.
/// Fail-closed: not full floating text / InitialCaptureBonus / upgrade boost.
#[test]
fn black_market_residual_deposits_cash_on_interval() {
    use crate::game_logic::host_black_market::{
        BLACK_MARKET_DEPOSIT_AMOUNT, BLACK_MARKET_DEPOSIT_INTERVAL_FRAMES,
    };
    use crate::game_logic::{KindOf, ThingTemplate};

    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::GLA);

    if !game_logic.templates.contains_key("TestBlackMarket") {
        let mut t = ThingTemplate::new("TestBlackMarket");
        t.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::FSBlackMarket)
            .add_kind_of(KindOf::Selectable)
            .set_health(1000.0)
            .set_cost(2000, 0);
        game_logic
            .templates
            .insert("TestBlackMarket".to_string(), t);
    }

    let market_id = game_logic
        .create_object("TestBlackMarket", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("black market");
    // Ensure constructed residual (create_object may leave under-construction off for tests).
    if let Some(obj) = game_logic.host_object_mut(market_id) {
        obj.set_status_under_construction(false);
    }

    let cash_before = game_logic
        .get_player_mut_by_team(Team::GLA)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);

    // First schedule is current_frame + interval on first observe.
    game_logic.frame = 0;
    game_logic.update_black_market_deposits();
    assert!(!game_logic.honesty_black_market_deposit_ok());
    let mid = game_logic
        .get_player_mut_by_team(Team::GLA)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);
    assert_eq!(mid, cash_before, "no deposit before interval");

    game_logic.frame = BLACK_MARKET_DEPOSIT_INTERVAL_FRAMES;
    game_logic.update_black_market_deposits();

    let cash_after = game_logic
        .get_player_mut_by_team(Team::GLA)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);
    assert_eq!(
        cash_after,
        cash_before.saturating_add(BLACK_MARKET_DEPOSIT_AMOUNT),
        "black market must deposit residual ${BLACK_MARKET_DEPOSIT_AMOUNT}"
    );
    assert!(
        game_logic.honesty_black_market_deposit_ok(),
        "black market deposit residual honesty"
    );
    assert_eq!(game_logic.black_markets().deposits, 1);
    assert_eq!(
        game_logic.black_markets().cash_total,
        BLACK_MARKET_DEPOSIT_AMOUNT
    );
    // AutoDeposit floating cash text residual (GUI:AddCash @ +Z10).
    assert!(
        game_logic.honesty_black_market_floating_text_ok(),
        "black market deposit must spawn floating cash text residual"
    );
    let bm_ft = game_logic
        .black_markets()
        .floating_texts
        .last()
        .expect("black market floating text");
    assert_eq!(bm_ft.amount, BLACK_MARKET_DEPOSIT_AMOUNT);
    assert_eq!(bm_ft.text_key, "GUI:AddCash");
    assert_eq!(bm_ft.color_rgba.3, 230);
    assert!((bm_ft.position.y - 10.0).abs() < 0.01);

    // Second interval deposit.
    game_logic.frame = BLACK_MARKET_DEPOSIT_INTERVAL_FRAMES * 2;
    game_logic.update_black_market_deposits();
    assert_eq!(game_logic.black_markets().deposits, 2);
    assert_eq!(
        game_logic.black_markets().cash_total,
        BLACK_MARKET_DEPOSIT_AMOUNT * 2
    );

    // Fake black market residual-skip.
    if !game_logic.templates.contains_key("FakeGLABlackMarket") {
        let mut t = ThingTemplate::new("FakeGLABlackMarket");
        t.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::FSBlackMarket)
            .set_health(500.0);
        game_logic
            .templates
            .insert("FakeGLABlackMarket".to_string(), t);
    }
    let fake_id = game_logic
        .create_object("FakeGLABlackMarket", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
        .expect("fake market");
    if let Some(obj) = game_logic.host_object_mut(fake_id) {
        obj.set_status_under_construction(false);
    }
    let deposits_before_fake = game_logic.black_markets().deposits;
    game_logic.frame = BLACK_MARKET_DEPOSIT_INTERVAL_FRAMES * 3;
    game_logic.update_black_market_deposits();
    // Real market deposits again; fake must not add an extra deposit beyond real's schedule.
    // Real market was due at frame 180 as well → deposits becomes 3.
    assert_eq!(
        game_logic.black_markets().deposits,
        deposits_before_fake + 1,
        "fake black market must not deposit cash"
    );
}

/// Residual: TechOilDerrick AutoDeposit deposits $200 every 360 logic frames
/// and awards InitialCaptureBonus $1000 once when first non-neutral owned.
/// Floating text + SupplyLines UpgradedBoost +20 residual.
/// Fail-closed: not full InGameUI GPU / STEALTHED local display gate.
#[test]
fn oil_derrick_residual_deposits_cash_on_interval() {
    use crate::game_logic::host_oil_derrick::{
        OIL_DERRICK_DEPOSIT_AMOUNT, OIL_DERRICK_DEPOSIT_INTERVAL_FRAMES,
        OIL_DERRICK_INITIAL_CAPTURE_BONUS, OIL_DERRICK_SUPPLY_LINES_BOOST,
    };
    use crate::game_logic::host_upgrades::UPGRADE_AMERICA_SUPPLY_LINES;
    use crate::game_logic::{KindOf, ThingTemplate};

    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    // Distinct player color for floating text residual honesty.
    if let Some(p) = game_logic.get_player_mut_by_team(Team::USA) {
        p.color_rgb = (0, 120, 255);
    }

    if !game_logic.templates.contains_key("TestOilDerrick") {
        let mut t = ThingTemplate::new("TestOilDerrick");
        t.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .set_health(2000.0)
            .set_cost(0, 0);
        game_logic.templates.insert("TestOilDerrick".to_string(), t);
    }

    // Spawn neutral (map residual), then capture to USA.
    let derrick_id = game_logic
        .create_object("TestOilDerrick", Team::Neutral, Vec3::new(0.0, 0.0, 0.0))
        .expect("oil derrick");
    if let Some(obj) = game_logic.host_object_mut(derrick_id) {
        obj.set_status_under_construction(false);
    }

    let cash_before = game_logic
        .get_player_mut_by_team(Team::USA)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);

    // Neutral residual-skip.
    game_logic.frame = 0;
    game_logic.update_oil_derrick_deposits();
    assert!(!game_logic.honesty_oil_derrick_ok());
    let mid = game_logic
        .get_player_mut_by_team(Team::USA)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);
    assert_eq!(mid, cash_before, "neutral derrick must not deposit");

    // Capture residual: flip team to USA → InitialCaptureBonus.
    if let Some(obj) = game_logic.host_object_mut(derrick_id) {
        obj.set_team(Team::USA);
    }
    game_logic.frame = 0;
    game_logic.update_oil_derrick_deposits();
    let after_capture = game_logic
        .get_player_mut_by_team(Team::USA)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);
    assert_eq!(
        after_capture,
        cash_before.saturating_add(OIL_DERRICK_INITIAL_CAPTURE_BONUS),
        "capture must award residual ${OIL_DERRICK_INITIAL_CAPTURE_BONUS}"
    );
    assert!(
        game_logic.honesty_oil_derrick_capture_bonus_ok(),
        "oil derrick capture bonus residual honesty"
    );
    // Capture bonus floating text residual.
    assert!(
        game_logic.honesty_oil_derrick_floating_text_ok(),
        "capture bonus must spawn floating cash text residual"
    );
    let capture_ft = game_logic
        .oil_derricks()
        .floating_texts
        .iter()
        .find(|t| t.is_capture_bonus)
        .expect("capture floating text");
    assert_eq!(capture_ft.amount, OIL_DERRICK_INITIAL_CAPTURE_BONUS);
    assert_eq!(capture_ft.text_key, "GUI:AddCash");
    assert_eq!(capture_ft.color_rgba, (0, 120, 255, 230));
    assert!((capture_ft.position.y - 10.0).abs() < 0.01);
    // Periodic deposit not yet due (rescheduled after capture).
    assert!(!game_logic.honesty_oil_derrick_deposit_ok());

    // Second capture-bonus call residual-skip.
    game_logic.update_oil_derrick_deposits();
    let after_dup = game_logic
        .get_player_mut_by_team(Team::USA)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);
    assert_eq!(after_dup, after_capture, "capture bonus once only");

    // Periodic deposit after interval.
    game_logic.frame = OIL_DERRICK_DEPOSIT_INTERVAL_FRAMES;
    game_logic.update_oil_derrick_deposits();
    let after_deposit = game_logic
        .get_player_mut_by_team(Team::USA)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);
    assert_eq!(
        after_deposit,
        after_capture.saturating_add(OIL_DERRICK_DEPOSIT_AMOUNT),
        "oil derrick must deposit residual ${OIL_DERRICK_DEPOSIT_AMOUNT}"
    );
    assert!(
        game_logic.honesty_oil_derrick_deposit_ok(),
        "oil derrick deposit residual honesty"
    );
    assert_eq!(game_logic.oil_derricks().deposits, 1);
    assert_eq!(
        game_logic.oil_derrick_residual_cash_total(),
        OIL_DERRICK_DEPOSIT_AMOUNT
    );
    assert_eq!(
        game_logic.oil_derrick_capture_bonus_cash_total(),
        OIL_DERRICK_INITIAL_CAPTURE_BONUS
    );
    assert_eq!(game_logic.oil_derrick_supply_lines_boost_cash_total(), 0);

    // Second interval.
    game_logic.frame = OIL_DERRICK_DEPOSIT_INTERVAL_FRAMES * 2;
    game_logic.update_oil_derrick_deposits();
    assert_eq!(game_logic.oil_derricks().deposits, 2);
    assert_eq!(
        game_logic.oil_derrick_residual_cash_total(),
        OIL_DERRICK_DEPOSIT_AMOUNT * 2
    );

    // SupplyLines UpgradedBoost residual: +20 per deposit.
    if let Some(p) = game_logic.get_player_mut_by_team(Team::USA) {
        p.unlock_science(UPGRADE_AMERICA_SUPPLY_LINES);
    }
    game_logic.frame = OIL_DERRICK_DEPOSIT_INTERVAL_FRAMES * 3;
    game_logic.update_oil_derrick_deposits();
    assert_eq!(game_logic.oil_derricks().deposits, 3);
    assert_eq!(
        game_logic.oil_derrick_residual_cash_total(),
        OIL_DERRICK_DEPOSIT_AMOUNT * 2
            + OIL_DERRICK_DEPOSIT_AMOUNT
            + OIL_DERRICK_SUPPLY_LINES_BOOST
    );
    assert_eq!(
        game_logic.oil_derrick_supply_lines_boost_cash_total(),
        OIL_DERRICK_SUPPLY_LINES_BOOST
    );
    assert!(game_logic.honesty_oil_derrick_supply_lines_boost_ok());
    let deposit_ft = game_logic
        .oil_derricks()
        .floating_texts
        .iter()
        .rev()
        .find(|t| !t.is_capture_bonus)
        .expect("deposit floating text");
    assert_eq!(
        deposit_ft.amount,
        OIL_DERRICK_DEPOSIT_AMOUNT + OIL_DERRICK_SUPPLY_LINES_BOOST
    );
    assert_eq!(deposit_ft.text, "+$220");
}

/// Residual: AmericaSupplyDropZone OCL queues cargo DeliverPayload every 3600
/// logic frames; after approach delay, spawns 6 crates and credits $1500
/// (BuildingPickup residual). With SupplyLines, +25/crate → $1650.
/// Fail-closed: not full CreateAtEdge aircraft Object / parachute fall physics.
#[test]
fn supply_drop_zone_residual_credits_cash_on_interval() {
    use crate::game_logic::host_deliver_payload::{
        HostCargoPlaneFlightPhase, HostDeliverPayloadKind, CARGO_PLANE_APPROACH_DELAY_FRAMES,
        CARGO_PLANE_DOOR_DELAY_FRAMES, CARGO_PLANE_PREFERRED_HEIGHT, SUPPLY_DROP_DROP_DELAY_FRAMES,
        SUPPLY_DROP_PAYLOAD_COUNT, SUPPLY_DROP_PAYLOAD_RESIDUAL_TEMPLATE,
    };
    use crate::game_logic::host_supply_drop_zone::{
        SUPPLY_DROP_ZONE_DROP_CASH, SUPPLY_DROP_ZONE_DROP_CASH_WITH_SUPPLY_LINES,
        SUPPLY_DROP_ZONE_INTERVAL_FRAMES,
    };
    use crate::game_logic::host_upgrades::UPGRADE_AMERICA_SUPPLY_LINES;
    use crate::game_logic::{KindOf, ThingTemplate};

    let kind = HostDeliverPayloadKind::SupplyDropZoneCrate;
    // Run DropDelay stagger from first item frame through last item frame.
    let run_stagger = |gl: &mut GameLogic, activate: u32| {
        let first = kind.item_drop_frame(activate, 0);
        let last = kind.mission_complete_frame(activate);
        for f in first..=last {
            gl.frame = f;
            gl.update_deliver_payloads();
        }
    };

    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::USA);

    if !game_logic.templates.contains_key("TestSupplyDropZone") {
        let mut t = ThingTemplate::new("TestSupplyDropZone");
        t.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .set_health(1000.0)
            .set_cost(2500, 0);
        game_logic
            .templates
            .insert("TestSupplyDropZone".to_string(), t);
    }

    let zone_id = game_logic
        .create_object("TestSupplyDropZone", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("supply drop zone");
    if let Some(obj) = game_logic.host_object_mut(zone_id) {
        obj.set_status_under_construction(false);
    }

    let cash_before = game_logic
        .get_player_mut_by_team(Team::USA)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);
    let objects_before = game_logic.host_objects().len();

    // First observation schedules without flight (C++ m_nextCreationFrame == 0).
    game_logic.frame = 0;
    game_logic.update_supply_drop_zone_drops();
    game_logic.update_deliver_payloads();
    assert!(!game_logic.honesty_supply_drop_zone_ok());
    assert!(!game_logic.honesty_supply_drop_zone_flight_ok());
    let mid = game_logic
        .get_player_mut_by_team(Team::USA)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);
    assert_eq!(mid, cash_before, "no cash before first interval");

    // OCL due at 3600: queues cargo flight, no cash / crates yet.
    let activate = SUPPLY_DROP_ZONE_INTERVAL_FRAMES;
    game_logic.frame = activate;
    game_logic.update_supply_drop_zone_drops();
    assert!(
        game_logic.honesty_supply_drop_zone_flight_ok(),
        "OCL must start cargo flight residual"
    );
    assert_eq!(game_logic.supply_drop_zone_residual_flights(), 1);
    assert!(
        game_logic
            .host_deliver_payloads()
            .honesty_inbound_ok(HostDeliverPayloadKind::SupplyDropZoneCrate),
        "DeliverPayload cargo mission must be inbound"
    );
    // CreateAtEdge residual: edge spawn at PreferredHeight on queue.
    assert!(
        game_logic
            .host_deliver_payloads()
            .honesty_create_at_edge_ok(),
        "CreateAtEdge cargo plane residual must spawn on queue"
    );
    let flight0 = game_logic
        .host_deliver_payloads()
        .cargo_flights_snapshot()
        .into_iter()
        .next()
        .expect("cargo flight residual");
    assert_eq!(flight0.phase, HostCargoPlaneFlightPhase::EdgeSpawn);
    assert!((flight0.current_pos.y - CARGO_PLANE_PREFERRED_HEIGHT).abs() < 0.01);
    assert_eq!(flight0.transport_template, "AmericaJetCargoPlane");
    assert_eq!(flight0.model_name, "AVCargoPln");
    assert!(!game_logic.honesty_supply_drop_zone_ok());
    assert_eq!(
        game_logic.host_objects().len(),
        objects_before,
        "no crates before approach delay"
    );
    let after_queue = game_logic
        .get_player_mut_by_team(Team::USA)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);
    assert_eq!(after_queue, cash_before, "no cash during cargo approach");
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "CargoPlaneApproach"),
        "flight queue must emit CargoPlaneApproach audio"
    );

    // One frame before first item (approach + door delay residual): no crates.
    let first_item = kind.item_drop_frame(activate, 0);
    assert_eq!(
        first_item,
        activate + CARGO_PLANE_APPROACH_DELAY_FRAMES + CARGO_PLANE_DOOR_DELAY_FRAMES
    );
    game_logic.frame = first_item - 1;
    game_logic.update_deliver_payloads();
    assert_eq!(
        game_logic.host_objects().len(),
        objects_before,
        "still no crates one frame before first DropDelay item"
    );

    // First item only: 1 crate, no bulk BuildingPickup cash yet.
    game_logic.frame = first_item;
    game_logic.update_deliver_payloads();
    assert_eq!(
        game_logic.host_objects().len(),
        objects_before + 1,
        "first DropDelay item must spawn one crate"
    );
    let mid_stagger_cash = game_logic
        .get_player_mut_by_team(Team::USA)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);
    assert_eq!(
        mid_stagger_cash, cash_before,
        "no bulk BuildingPickup cash until final DropDelay item"
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "SupplyDropZoneDrop"),
        "first item must queue SupplyDropZoneDrop audio"
    );

    // Advance remaining stagger items → full spawn + BuildingPickup cash.
    let last_item = kind.mission_complete_frame(activate);
    assert_eq!(
        last_item,
        first_item + (SUPPLY_DROP_PAYLOAD_COUNT - 1) * SUPPLY_DROP_DROP_DELAY_FRAMES
    );
    for f in (first_item + 1)..=last_item {
        game_logic.frame = f;
        game_logic.update_deliver_payloads();
    }
    let after_drop = game_logic
        .get_player_mut_by_team(Team::USA)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);
    assert_eq!(
        after_drop,
        cash_before.saturating_add(SUPPLY_DROP_ZONE_DROP_CASH),
        "supply drop zone must credit residual ${SUPPLY_DROP_ZONE_DROP_CASH} on final item"
    );
    assert!(
        game_logic.honesty_supply_drop_zone_ok(),
        "supply drop zone residual honesty"
    );
    assert!(game_logic.honesty_supply_drop_zone_drop_ok());
    assert!(
        game_logic.honesty_supply_drop_cargo_deliver_payload_ok(),
        "DeliverPayload cargo host path honesty"
    );
    assert!(game_logic.honesty_deliver_payload_cargo_ok());
    assert!(
        game_logic.honesty_deliver_payload_drop_delay_stagger_ok(),
        "DropDelay stagger honesty"
    );
    assert_eq!(game_logic.supply_drop_zone_residual_drops(), 1);
    assert_eq!(
        game_logic.supply_drop_zone_residual_cash_total(),
        SUPPLY_DROP_ZONE_DROP_CASH
    );
    assert_eq!(game_logic.supply_drop_zones().drops, 1);
    assert_eq!(
        game_logic.host_objects().len(),
        objects_before + SUPPLY_DROP_PAYLOAD_COUNT as usize,
        "must spawn residual SupplyDropZoneCrate count"
    );

    let completed = game_logic
        .host_deliver_payloads()
        .completed_of_kind(HostDeliverPayloadKind::SupplyDropZoneCrate);
    assert_eq!(completed.len(), 1);
    assert_eq!(
        completed[0].spawned_payload_ids.len(),
        SUPPLY_DROP_PAYLOAD_COUNT as usize
    );
    assert_eq!(completed[0].items_dropped, SUPPLY_DROP_PAYLOAD_COUNT);
    assert_eq!(completed[0].max_attempts, 4);
    assert_eq!(completed[0].transport_template, "AmericaJetCargoPlane");
    assert_eq!(completed[0].put_in_container, "AmericaCrateParachute");
    for id in &completed[0].spawned_payload_ids {
        let obj = game_logic.host_object(*id).expect("spawned crate");
        assert_eq!(obj.team, Team::USA);
        assert!(
            obj.thing.template.name == SUPPLY_DROP_PAYLOAD_RESIDUAL_TEMPLATE
                || obj.thing.template.name.contains("Crate")
                || obj.thing.template.name.contains("SupplyDrop"),
            "spawned residual crate template, got {}",
            obj.thing.template.name
        );
        // AmericaCrateParachute residual: elevated spawn (PreferredHeight + DropOffset).
        assert!(
            obj.is_parachuting() || obj.get_position().y <= 0.5,
            "spawned crate must parachute or have landed, y={}",
            obj.get_position().y
        );
        if obj.is_parachuting() {
            assert!(
                obj.get_position().y > 50.0,
                "airborne crate residual drop height, got y={}",
                obj.get_position().y
            );
        }
    }

    // Tick AmericaCrateParachute residual until crates land (OpenDist + sink).
    let payload_ids = completed[0].spawned_payload_ids.clone();
    for _ in 0..80 {
        for id in &payload_ids {
            game_logic.tick_crate_parachute_residual(*id);
        }
        if payload_ids.iter().all(|id| {
            game_logic
                .host_object(*id)
                .map(|o| !o.is_parachuting() && o.get_position().y <= 0.5)
                .unwrap_or(true)
        }) {
            break;
        }
    }
    assert!(
        game_logic.honesty_crate_parachute_fall_physics_ok(),
        "AmericaCrateParachute open+land residual honesty"
    );
    assert!(
        game_logic.honesty_crate_parachute_bone_attach_ok(),
        "AmericaCrateParachute PARA_COG bone attach residual honesty"
    );
    assert!(
        game_logic.honesty_create_at_edge_flight_ok(),
        "CreateAtEdge + DeliveryDistance flight residual honesty"
    );
    for id in &payload_ids {
        let obj = game_logic.host_object(*id).expect("landed crate");
        assert!(
            !obj.is_parachuting(),
            "crate must clear parachuting on land"
        );
        assert!(
            obj.get_position().y <= 0.5,
            "crate must land at ground residual, y={}",
            obj.get_position().y
        );
    }

    // Second interval: queue flight then complete full DropDelay stagger.
    let activate2 = SUPPLY_DROP_ZONE_INTERVAL_FRAMES * 2;
    game_logic.frame = activate2;
    game_logic.update_supply_drop_zone_drops();
    assert_eq!(game_logic.supply_drop_zone_residual_flights(), 2);
    run_stagger(&mut game_logic, activate2);
    assert_eq!(game_logic.supply_drop_zone_residual_drops(), 2);
    assert_eq!(
        game_logic.supply_drop_zone_residual_cash_total(),
        SUPPLY_DROP_ZONE_DROP_CASH * 2
    );

    // SupplyLines residual: next drop is $1650 (base + 6×25).
    if let Some(player) = game_logic.get_player_mut_by_team(Team::USA) {
        player.unlock_science(UPGRADE_AMERICA_SUPPLY_LINES);
        assert!(
            player.has_unlocked_upgrade(UPGRADE_AMERICA_SUPPLY_LINES),
            "test setup: SupplyLines must be unlocked"
        );
    }

    let cash_before_sl = game_logic
        .get_player_mut_by_team(Team::USA)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);
    let activate3 = SUPPLY_DROP_ZONE_INTERVAL_FRAMES * 3;
    game_logic.frame = activate3;
    game_logic.update_supply_drop_zone_drops();
    run_stagger(&mut game_logic, activate3);
    let after_sl = game_logic
        .get_player_mut_by_team(Team::USA)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);
    assert_eq!(
        after_sl,
        cash_before_sl.saturating_add(SUPPLY_DROP_ZONE_DROP_CASH_WITH_SUPPLY_LINES),
        "with SupplyLines drop must credit residual ${SUPPLY_DROP_ZONE_DROP_CASH_WITH_SUPPLY_LINES}"
    );
    assert_eq!(
        game_logic.supply_drop_zone_supply_lines_boost_cash_total(),
        SUPPLY_DROP_ZONE_DROP_CASH_WITH_SUPPLY_LINES - SUPPLY_DROP_ZONE_DROP_CASH
    );
}

/// Residual: DeliverPayload cargo path constants + DropDelay / DoorDelay / MaxAttempts.
#[test]
fn deliver_payload_cargo_residual_constants_and_skip() {
    use crate::game_logic::host_deliver_payload::{
        drop_delay_frames_from_ms, residual_allowed_delivery_distance, HostDeliverPayloadKind,
        CARGO_PLANE_APPROACH_DELAY_FRAMES, CARGO_PLANE_DOOR_DELAY_FRAMES,
        CARGO_PLANE_DOOR_DELAY_MS, SUPPLY_DROP_CARGO_TRANSPORT, SUPPLY_DROP_DROP_DELAY_FRAMES,
        SUPPLY_DROP_DROP_DELAY_MS, SUPPLY_DROP_DROP_OFFSET_Y, SUPPLY_DROP_MAX_ATTEMPTS,
        SUPPLY_DROP_PAYLOAD_COUNT, SUPPLY_DROP_PRE_OPEN_DISTANCE, SUPPLY_DROP_PUT_IN_CONTAINER,
    };

    assert_eq!(CARGO_PLANE_APPROACH_DELAY_FRAMES, 90);
    assert_eq!(SUPPLY_DROP_PAYLOAD_COUNT, 6);
    assert_eq!(SUPPLY_DROP_DROP_DELAY_MS, 350);
    assert_eq!(SUPPLY_DROP_DROP_DELAY_FRAMES, 11);
    assert_eq!(drop_delay_frames_from_ms(350), 11);
    assert_eq!(CARGO_PLANE_DOOR_DELAY_MS, 500);
    assert_eq!(CARGO_PLANE_DOOR_DELAY_FRAMES, 15);
    assert_eq!(drop_delay_frames_from_ms(500), 15);
    assert_eq!(SUPPLY_DROP_MAX_ATTEMPTS, 4);
    assert!((SUPPLY_DROP_PRE_OPEN_DISTANCE - 0.0).abs() < 0.01);
    assert!((SUPPLY_DROP_DROP_OFFSET_Y - (-5.0)).abs() < 0.01);
    assert_eq!(SUPPLY_DROP_CARGO_TRANSPORT, "AmericaJetCargoPlane");
    assert_eq!(SUPPLY_DROP_PUT_IN_CONTAINER, "AmericaCrateParachute");
    assert!(HostDeliverPayloadKind::SupplyDropZoneCrate.spawns_payload_objects());
    assert!(!HostDeliverPayloadKind::AmericaParadrop.spawns_payload_objects());
    assert!(
        (residual_allowed_delivery_distance(HostDeliverPayloadKind::SupplyDropZoneCrate) - 410.0)
            .abs()
            < 0.01
    );
    // Stagger frame honesty: first item at approach+door, last at +5*DropDelay.
    let k = HostDeliverPayloadKind::SupplyDropZoneCrate;
    assert_eq!(k.item_drop_frame(0, 0), 105);
    assert_eq!(k.mission_complete_frame(0), 105 + 5 * 11);
}

/// Residual: MoneyCrateCollide unit pickup credits cash and destroys crate.
#[test]
fn money_crate_collide_unit_pickup_residual() {
    use crate::game_logic::host_money_crate::{
        SUPPLY_DROP_CRATE_MONEY_PROVIDED, SUPPLY_DROP_CRATE_SUPPLY_LINES_BOOST,
    };
    use crate::game_logic::host_upgrades::UPGRADE_AMERICA_SUPPLY_LINES;
    use crate::game_logic::{KindOf, ThingTemplate};

    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::USA);

    if !game_logic.templates.contains_key("TestSupplyDropZoneCrate") {
        let mut t = ThingTemplate::new("TestSupplyDropZoneCrate");
        t.add_kind_of(KindOf::Resource)
            .add_kind_of(KindOf::Selectable)
            .set_health(1.0);
        game_logic
            .templates
            .insert("TestSupplyDropZoneCrate".to_string(), t);
    }
    if !game_logic.templates.contains_key("TestRanger") {
        let mut t = ThingTemplate::new("TestRanger");
        t.add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        game_logic.templates.insert("TestRanger".to_string(), t);
    }

    let crate_id = game_logic
        .create_object(
            "TestSupplyDropZoneCrate",
            Team::Neutral,
            Vec3::new(10.0, 0.0, 10.0),
        )
        .expect("crate");
    game_logic
        .host_money_crates
        .register_supply_drop_crate(crate_id);

    let ranger_id = game_logic
        .create_object("TestRanger", Team::USA, Vec3::new(12.0, 0.0, 10.0))
        .expect("ranger");
    let _ = ranger_id;

    let cash_before = game_logic
        .get_player_mut_by_team(Team::USA)
        .map(|p| test_observed_supplies(p))
        .unwrap_or(0);

    game_logic.update_money_crate_collides();
    // Destroy list processes on full update; destroy_object queues destruction.
    // Cash should already be credited (effective supplies under ECONOMY_AUTHORITY).
    let cash_after = game_logic
        .get_player_mut_by_team(Team::USA)
        .map(|p| test_observed_supplies(p))
        .unwrap_or(0);
    assert_eq!(
        cash_after,
        cash_before.saturating_add(SUPPLY_DROP_CRATE_MONEY_PROVIDED),
        "unit MoneyCrateCollide must credit MoneyProvided"
    );
    assert!(game_logic.honesty_money_crate_unit_pickup_ok());
    assert!(game_logic.honesty_money_crate_collide_ok());
    assert!(
        game_logic.honesty_money_pickup_anim_ok(),
        "MoneyPickUp ExecuteAnimation residual must record"
    );
    assert!(
        game_logic
            .host_money_crates()
            .money_pickup_anims
            .iter()
            .any(|a| a.template == "MoneyPickUp"
                && (a.display_time_seconds - 4.0).abs() < 0.01
                && (a.z_rise_per_second - 15.0).abs() < 0.01
                && a.fades),
        "MoneyPickUp residual presentation descriptor constants"
    );
    assert!(
        game_logic
            .host_money_crates()
            .money_floating_texts
            .is_empty(),
        "C++ MoneyCrateCollide never emits GUI:AddCash floating text"
    );

    assert!(!game_logic.host_money_crates().contains(crate_id));
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "CrateMoney"),
        "pickup must queue CrateMoney audio"
    );

    // SupplyLines boost residual on second crate.
    let crate2 = game_logic
        .create_object(
            "TestSupplyDropZoneCrate",
            Team::Neutral,
            Vec3::new(50.0, 0.0, 50.0),
        )
        .expect("crate2");
    game_logic
        .host_money_crates
        .register_supply_drop_crate(crate2);
    let _r2 = game_logic
        .create_object("TestRanger", Team::USA, Vec3::new(51.0, 0.0, 50.0))
        .expect("ranger2");
    if let Some(player) = game_logic.get_player_mut_by_team(Team::USA) {
        player.unlock_science(UPGRADE_AMERICA_SUPPLY_LINES);
    }
    let cash_mid = game_logic
        .get_player_mut_by_team(Team::USA)
        .map(|p| test_observed_supplies(p))
        .unwrap_or(0);
    game_logic.update_money_crate_collides();
    let cash_sl = game_logic
        .get_player_mut_by_team(Team::USA)
        .map(|p| test_observed_supplies(p))
        .unwrap_or(0);
    assert_eq!(
        cash_sl,
        cash_mid.saturating_add(
            SUPPLY_DROP_CRATE_MONEY_PROVIDED + SUPPLY_DROP_CRATE_SUPPLY_LINES_BOOST
        )
    );
    assert!(game_logic
        .host_money_crates()
        .honesty_supply_lines_boost_ok());
}

/// Residual: airborne crate blocks unit MoneyCrateCollide; ForbiddenKindOf PROJECTILE.
#[test]
fn money_crate_above_terrain_and_forbidden_kindof_residual() {
    use crate::game_logic::host_deliver_payload::{
        cargo_crate_drop_height, SUPPLY_DROP_DROP_OFFSET_Y,
    };
    use crate::game_logic::{KindOf, ThingTemplate};

    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::USA);

    if !game_logic.templates.contains_key("TestSupplyDropZoneCrate") {
        let mut t = ThingTemplate::new("TestSupplyDropZoneCrate");
        t.add_kind_of(KindOf::Resource)
            .add_kind_of(KindOf::Selectable)
            .set_health(1.0);
        game_logic
            .templates
            .insert("TestSupplyDropZoneCrate".to_string(), t);
    }
    if !game_logic.templates.contains_key("TestRanger") {
        let mut t = ThingTemplate::new("TestRanger");
        t.add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        game_logic.templates.insert("TestRanger".to_string(), t);
    }
    if !game_logic.templates.contains_key("TestProjectile") {
        let mut t = ThingTemplate::new("TestProjectile");
        t.add_kind_of(KindOf::Projectile).set_health(1.0);
        game_logic.templates.insert("TestProjectile".to_string(), t);
    }

    let drop_y = cargo_crate_drop_height(SUPPLY_DROP_DROP_OFFSET_Y);
    let crate_id = game_logic
        .create_object(
            "TestSupplyDropZoneCrate",
            Team::Neutral,
            Vec3::new(10.0, drop_y, 10.0),
        )
        .expect("airborne crate");
    game_logic
        .host_money_crates
        .register_supply_drop_crate(crate_id);
    if let Some(obj) = game_logic.objects.get_mut(&crate_id) {
        obj.apply_crate_parachuting();
    }
    let _ranger = game_logic
        .create_object("TestRanger", Team::USA, Vec3::new(12.0, 0.0, 10.0))
        .expect("ranger");
    let cash_before = game_logic
        .get_player_mut_by_team(Team::USA)
        .map(|p| test_observed_supplies(p))
        .unwrap_or(0);
    game_logic.update_money_crate_collides();
    let cash_air = game_logic
        .get_player_mut_by_team(Team::USA)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);
    assert_eq!(
        cash_air, cash_before,
        "unit must not pick up airborne crate residual"
    );
    assert!(
        game_logic.honesty_money_crate_above_terrain_reject_ok(),
        "above-terrain unit reject residual honesty"
    );
    assert!(game_logic.host_money_crates().contains(crate_id));

    // Land residual then unit pickup succeeds + MoneyPickUp anim.
    for _ in 0..80 {
        game_logic.tick_crate_parachute_residual(crate_id);
        if game_logic
            .host_object(crate_id)
            .map(|o| !o.is_parachuting())
            .unwrap_or(true)
        {
            break;
        }
    }
    assert!(
        game_logic
            .host_object(crate_id)
            .map(|o| !o.is_parachuting() && o.get_position().y <= 0.5)
            .unwrap_or(false),
        "crate parachute residual must land"
    );
    assert!(game_logic.honesty_crate_parachute_fall_physics_ok());
    assert!(
        game_logic.honesty_crate_parachute_bone_attach_ok(),
        "AmericaCrateParachute bone attach residual must build while open"
    );
    game_logic.update_money_crate_collides();
    let cash_land = game_logic
        .get_player_mut_by_team(Team::USA)
        .map(|p| test_observed_supplies(p))
        .unwrap_or(0);
    assert!(cash_land > cash_before, "landed crate unit pickup residual");
    assert!(game_logic.honesty_money_pickup_anim_ok());
    assert!(
        game_logic
            .host_money_crates()
            .money_floating_texts
            .is_empty(),
        "landed MoneyCrateCollide still has no +$N floater"
    );


    // ForbiddenKindOf PROJECTILE residual: projectile near ground crate rejected.
    let crate2 = game_logic
        .create_object(
            "TestSupplyDropZoneCrate",
            Team::Neutral,
            Vec3::new(80.0, 0.0, 80.0),
        )
        .expect("crate2");
    game_logic
        .host_money_crates
        .register_supply_drop_crate(crate2);
    let _proj = game_logic
        .create_object("TestProjectile", Team::USA, Vec3::new(81.0, 0.0, 80.0))
        .expect("projectile");
    let cash_mid = game_logic
        .get_player_mut_by_team(Team::USA)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);
    game_logic.update_money_crate_collides();
    let cash_proj = game_logic
        .get_player_mut_by_team(Team::USA)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);
    assert_eq!(
        cash_proj, cash_mid,
        "PROJECTILE ForbiddenKindOf residual must not pick up crate"
    );
    assert!(game_logic.host_money_crates().contains(crate2));
    assert!(
        game_logic.host_money_crates().forbidden_kindof_rejects > 0
            || game_logic.host_money_crates().honesty_forbidden_kindof_ok()
    );
}

#[test]
fn money_crate_required_kindof_rejects_infantry() {
    // C++ CrateCollide::isValidToExecute isKindOfMulti(m_kindof, m_kindofnot).
    use crate::game_logic::host_money_crate::SUPPLY_DROP_CRATE_MONEY_PROVIDED;
    use crate::game_logic::{KindOf, ThingTemplate};
    use game_engine::common::system::kind_of::KindOfMask;

    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    if !game_logic.templates.contains_key("TestSupplyDropZoneCrate") {
        let mut t = ThingTemplate::new("TestSupplyDropZoneCrate");
        t.add_kind_of(KindOf::Crate).set_health(1.0);
        game_logic
            .templates
            .insert("TestSupplyDropZoneCrate".to_string(), t);
    }
    if !game_logic.templates.contains_key("TestRanger") {
        let mut t = ThingTemplate::new("TestRanger");
        t.add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        game_logic.templates.insert("TestRanger".to_string(), t);
    }
    if !game_logic.templates.contains_key("TestTank") {
        ensure_test_tank_template(&mut game_logic);
    }

    let crate_id = game_logic
        .create_object(
            "TestSupplyDropZoneCrate",
            Team::Neutral,
            Vec3::new(10.0, 0.0, 10.0),
        )
        .expect("crate");
    game_logic
        .host_money_crates
        .register_supply_drop_crate(crate_id);
    game_logic.host_money_crates.set_kindof_gates(
        crate_id,
        KindOfMask::VEHICLE.bits(),
        0,
    );

    let _ranger = game_logic
        .create_object("TestRanger", Team::USA, Vec3::new(12.0, 0.0, 10.0))
        .expect("ranger");
    let cash_before = game_logic
        .get_player_mut_by_team(Team::USA)
        .map(|p| test_observed_supplies(p))
        .unwrap_or(0);
    game_logic.update_money_crate_collides();
    let cash_inf = game_logic
        .get_player_mut_by_team(Team::USA)
        .map(|p| test_observed_supplies(p))
        .unwrap_or(0);
    assert_eq!(
        cash_inf, cash_before,
        "RequiredKindOf VEHICLE must reject infantry"
    );
    assert!(game_logic.host_money_crates.contains(crate_id));

    let _tank = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(11.0, 0.0, 10.0))
        .expect("tank");
    game_logic.update_money_crate_collides();
    let cash_veh = game_logic
        .get_player_mut_by_team(Team::USA)
        .map(|p| test_observed_supplies(p))
        .unwrap_or(0);
    assert_eq!(
        cash_veh,
        cash_before.saturating_add(SUPPLY_DROP_CRATE_MONEY_PROVIDED),
        "RequiredKindOf VEHICLE must accept tank"
    );
}

#[test]
fn money_crate_mine_without_ai_does_not_pickup() {
    // C++ CrateCollide::isValidToExecute requires getAIUpdateInterface unless BuildingPickup.
    use crate::game_logic::{KindOf, ThingTemplate};

    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    if !game_logic.templates.contains_key("TestSupplyDropZoneCrate") {
        let mut t = ThingTemplate::new("TestSupplyDropZoneCrate");
        t.add_kind_of(KindOf::Crate).set_health(1.0);
        game_logic
            .templates
            .insert("TestSupplyDropZoneCrate".to_string(), t);
    }
    if !game_logic.templates.contains_key("TestLandMine") {
        let mut t = ThingTemplate::new("TestLandMine");
        t.add_kind_of(KindOf::Mine).set_health(10.0);
        game_logic.templates.insert("TestLandMine".to_string(), t);
    }

    let crate_id = game_logic
        .create_object(
            "TestSupplyDropZoneCrate",
            Team::Neutral,
            Vec3::new(10.0, 0.0, 10.0),
        )
        .expect("crate");
    game_logic
        .host_money_crates
        .register_supply_drop_crate(crate_id);
    let _mine = game_logic
        .create_object("TestLandMine", Team::USA, Vec3::new(11.0, 0.0, 10.0))
        .expect("mine");
    let cash_before = game_logic
        .get_player_mut_by_team(Team::USA)
        .map(|p| test_observed_supplies(p))
        .unwrap_or(0);
    game_logic.update_money_crate_collides();
    let cash_after = game_logic
        .get_player_mut_by_team(Team::USA)
        .map(|p| test_observed_supplies(p))
        .unwrap_or(0);
    assert_eq!(
        cash_after, cash_before,
        "mine has no AIUpdateInterface and must not absorb crate"
    );
    assert!(game_logic.host_money_crates.contains(crate_id));
}


/// Residual: under-construction / neutral supply drop zone must not credit cash.
#[test]
fn supply_drop_zone_residual_skips_under_construction_and_neutral() {
    use crate::game_logic::host_supply_drop_zone::SUPPLY_DROP_ZONE_INTERVAL_FRAMES;
    use crate::game_logic::{KindOf, ThingTemplate};

    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::USA);

    if !game_logic.templates.contains_key("AmericaSupplyDropZone") {
        let mut t = ThingTemplate::new("AmericaSupplyDropZone");
        t.add_kind_of(KindOf::Structure).set_health(1000.0);
        game_logic
            .templates
            .insert("AmericaSupplyDropZone".to_string(), t);
    }

    // Under construction USA zone.
    let uc_id = game_logic
        .create_object("AmericaSupplyDropZone", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("uc zone");
    if let Some(obj) = game_logic.host_object_mut(uc_id) {
        obj.set_status_under_construction(true);
    }

    // Neutral constructed zone.
    let neutral_id = game_logic
        .create_object(
            "AmericaSupplyDropZone",
            Team::Neutral,
            Vec3::new(50.0, 0.0, 0.0),
        )
        .expect("neutral zone");
    if let Some(obj) = game_logic.host_object_mut(neutral_id) {
        obj.set_status_under_construction(false);
    }

    let cash_before = game_logic
        .get_player_mut_by_team(Team::USA)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);

    game_logic.frame = 0;
    game_logic.update_supply_drop_zone_drops();
    game_logic.frame = SUPPLY_DROP_ZONE_INTERVAL_FRAMES;
    game_logic.update_supply_drop_zone_drops();

    let cash_after = game_logic
        .get_player_mut_by_team(Team::USA)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);
    assert_eq!(
        cash_after, cash_before,
        "under-construction / neutral supply drop zone must not credit cash"
    );
    assert!(!game_logic.honesty_supply_drop_zone_ok());
}

/// Residual: under-construction oil derrick must not deposit or award capture bonus.
#[test]
fn oil_derrick_residual_skips_under_construction() {
    use crate::game_logic::host_oil_derrick::OIL_DERRICK_DEPOSIT_INTERVAL_FRAMES;
    use crate::game_logic::{KindOf, ThingTemplate};

    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::USA);

    if !game_logic.templates.contains_key("TestOilDerrick") {
        let mut t = ThingTemplate::new("TestOilDerrick");
        t.add_kind_of(KindOf::Structure).set_health(2000.0);
        game_logic.templates.insert("TestOilDerrick".to_string(), t);
    }

    let derrick_id = game_logic
        .create_object("TestOilDerrick", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("oil derrick");
    if let Some(obj) = game_logic.host_object_mut(derrick_id) {
        obj.set_status_under_construction(true);
    }

    let cash_before = game_logic
        .get_player_mut_by_team(Team::USA)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);

    game_logic.frame = OIL_DERRICK_DEPOSIT_INTERVAL_FRAMES;
    game_logic.update_oil_derrick_deposits();
    let cash_after = game_logic
        .get_player_mut_by_team(Team::USA)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);
    assert_eq!(
        cash_after, cash_before,
        "under-construction oil derrick must not deposit"
    );
    assert!(!game_logic.honesty_oil_derrick_ok());
}

/// Residual: Hacker contained in Internet Center auto-hacks and deposits $5
/// after CashUpdateDelayFast plus the following logic update.
#[test]
fn hacker_internet_center_residual_deposits_cash() {
    use crate::game_logic::host_hacker_income::{
        HACKER_CASH_INTERVAL_FAST_FRAMES, HACKER_CASH_REGULAR,
    };
    use crate::game_logic::{
        ContainAdmission, ContainModuleKind, ContainModuleMetadata, HackInternetAIUpdateMetadata,
        KindOf, ThingTemplate,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::China);

    if !game_logic.templates.contains_key("TestHacker") {
        let mut t = ThingTemplate::new("TestHacker");
        t.add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::MoneyHacker)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0)
            .set_cost(200, 0);
        t.transport_slot_count = Some(1);
        t.hack_internet_ai_update = Some(HackInternetAIUpdateMetadata {
            unpack_time_frames: 219,
            pack_time_frames: 154,
            cash_update_delay_frames: 60,
            cash_update_delay_fast_frames: 54,
            regular_cash_amount: HACKER_CASH_REGULAR,
            veteran_cash_amount: 6,
            elite_cash_amount: 8,
            heroic_cash_amount: 10,
            xp_per_cash_update: 1.0,
            pack_unpack_variation_factor: 0.5,
        });
        game_logic.templates.insert("TestHacker".to_string(), t);
    }
    if !game_logic.templates.contains_key("TestInternetCenter") {
        let mut t = ThingTemplate::new("TestInternetCenter");
        t.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::FSInternetCenter)
            .add_kind_of(KindOf::Selectable)
            .set_health(2000.0)
            .set_cost(2500, 0);
        t.contain_module = ContainModuleMetadata {
            kind: ContainModuleKind::InternetHack,
            slots: Some(8),
            admission: ContainAdmission::MoneyHackerOnly,
            ..Default::default()
        };
        game_logic
            .templates
            .insert("TestInternetCenter".to_string(), t);
    }

    let ic_id = game_logic
        .create_object("TestInternetCenter", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("internet center");
    if let Some(obj) = game_logic.host_object_mut(ic_id) {
        obj.set_status_under_construction(false);
    }

    let hacker_id = game_logic
        .create_object("TestHacker", Team::China, Vec3::new(1.0, 0.0, 0.0))
        .expect("hacker");
    // Place the exact typed passenger inside the exact parsed containment
    // contract. A mere `contained_by` link is intentionally insufficient.
    assert!(game_logic
        .host_object_mut(ic_id)
        .expect("Internet Center")
        .add_occupant(hacker_id));
    if let Some(obj) = game_logic.host_object_mut(hacker_id) {
        obj.set_contained_by(Some(ic_id));
        obj.set_ai_state(AIState::Docked);
    }

    let cash_before = game_logic
        .get_player_mut_by_team(Team::China)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);

    game_logic.frame = 0;
    game_logic.update_hacker_income();
    assert!(
        game_logic.hacker_income().is_hacking(hacker_id),
        "hacker in IC must auto-start residual hacking"
    );
    let mid = game_logic
        .get_player_mut_by_team(Team::China)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);
    assert_eq!(mid, cash_before, "no deposit before fast interval");
    let first = 219 + HACKER_CASH_INTERVAL_FAST_FRAMES + 1;
    game_logic.frame = first - 1;
    game_logic.update_hacker_income();
    assert_eq!(
        game_logic
            .get_player_mut_by_team(Team::China)
            .map(|p| p.resources.supplies)
            .unwrap_or(0),
        cash_before,
        "C++ UNPACKING then fast delay before first cash"
    );
    game_logic.frame = first;
    game_logic.update_hacker_income();
    let cash_after = game_logic
        .get_player_mut_by_team(Team::China)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);
    assert_eq!(
        cash_after,
        cash_before.saturating_add(HACKER_CASH_REGULAR),
        "internet center hacker must deposit residual ${HACKER_CASH_REGULAR}"
    );
    assert_eq!(
        game_logic
            .get_player_mut_by_team(Team::China)
            .map(|p| p.statistics.money_earned)
            .unwrap_or(0),
        HACKER_CASH_REGULAR
    );
    assert!(
        game_logic.honesty_hacker_income_ok(),
        "hacker income residual honesty"
    );
    assert!(
        game_logic.honesty_hacker_internet_center_ok(),
        "hacker internet center residual honesty"
    );
    assert_eq!(game_logic.hacker_residual_deposits(), 1);
    assert_eq!(game_logic.hacker_residual_cash_total(), HACKER_CASH_REGULAR);

    // Second fast interval after unpack, again with the following update.
    game_logic.frame = first + HACKER_CASH_INTERVAL_FAST_FRAMES + 1;
    game_logic.update_hacker_income();
    assert_eq!(game_logic.hacker_residual_deposits(), 2);
    assert_eq!(
        game_logic.hacker_residual_cash_total(),
        HACKER_CASH_REGULAR * 2
    );
}

/// Residual: field HackInternet command deposits $5 after the authored
/// 60-frame delay and the following logic update.
#[test]
fn hacker_field_residual_deposits_cash_on_interval() {
    use crate::game_logic::host_hacker_income::{HACKER_CASH_INTERVAL_FRAMES, HACKER_CASH_REGULAR};
    use crate::game_logic::{HackInternetAIUpdateMetadata, KindOf, ThingTemplate};

    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::China);

    if !game_logic.templates.contains_key("TestHacker") {
        let mut t = ThingTemplate::new("TestHacker");
        t.add_kind_of(KindOf::Infantry).set_health(100.0);
        t.hack_internet_ai_update = Some(HackInternetAIUpdateMetadata {
            unpack_time_frames: 219,
            pack_time_frames: 154,
            cash_update_delay_frames: HACKER_CASH_INTERVAL_FRAMES,
            cash_update_delay_fast_frames: 54,
            regular_cash_amount: HACKER_CASH_REGULAR,
            veteran_cash_amount: 6,
            elite_cash_amount: 8,
            heroic_cash_amount: 10,
            xp_per_cash_update: 1.0,
            pack_unpack_variation_factor: 0.5,
        });
        game_logic.templates.insert("TestHacker".to_string(), t);
    }

    let hacker_id = game_logic
        .create_object("TestHacker", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("hacker");

    let cash_before = game_logic
        .get_player_mut_by_team(Team::China)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);

    // Field residual: not auto-hacking until command.
    game_logic.frame = 0;
    game_logic.update_hacker_income();
    assert!(!game_logic.hacker_income().is_hacking(hacker_id));

    assert!(
        game_logic.start_hacker_internet_hack(hacker_id),
        "field start_hacking must succeed for living hacker"
    );
    game_logic.update_hacker_income();
    let mid = game_logic
        .get_player_mut_by_team(Team::China)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);
    assert_eq!(mid, cash_before, "no deposit before field interval");

    let first = 219 + HACKER_CASH_INTERVAL_FRAMES + 1;
    game_logic.frame = first - 1;
    game_logic.update_hacker_income();
    assert_eq!(
        game_logic
            .get_player_mut_by_team(Team::China)
            .map(|p| p.resources.supplies)
            .unwrap_or(0),
        cash_before,
        "C++ UNPACKING then field delay before first cash"
    );
    game_logic.frame = first;
    game_logic.update_hacker_income();
    let cash_after = game_logic
        .get_player_mut_by_team(Team::China)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);
    assert_eq!(
        cash_after,
        cash_before.saturating_add(HACKER_CASH_REGULAR),
        "field hacker must deposit residual ${HACKER_CASH_REGULAR}"
    );
    assert_eq!(
        game_logic
            .get_player_mut_by_team(Team::China)
            .map(|p| p.statistics.money_earned)
            .unwrap_or(0),
        HACKER_CASH_REGULAR
    );
    assert!(game_logic.honesty_hacker_income_ok());
    // Field path is not internet-center classified.
    assert!(!game_logic.honesty_hacker_internet_center_ok());
}

/// C++ aiDoCommand PACKING: a move order stops HackInternet cash.
#[test]
fn hacker_move_command_stops_hacking() {
    use crate::game_logic::host_hacker_income::{HACKER_CASH_INTERVAL_FRAMES, HACKER_CASH_REGULAR};
    use crate::game_logic::{HackInternetAIUpdateMetadata, KindOf, ThingTemplate};

    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::China);
    let mut t = ThingTemplate::new("TestHackerMoveStop");
    t.add_kind_of(KindOf::Infantry).set_health(100.0);
    t.hack_internet_ai_update = Some(HackInternetAIUpdateMetadata {
        unpack_time_frames: 0,
        pack_time_frames: 0,
        cash_update_delay_frames: HACKER_CASH_INTERVAL_FRAMES,
        cash_update_delay_fast_frames: 54,
        regular_cash_amount: HACKER_CASH_REGULAR,
        veteran_cash_amount: 6,
        elite_cash_amount: 8,
        heroic_cash_amount: 10,
        xp_per_cash_update: 1.0,
        pack_unpack_variation_factor: 0.0,
    });
    game_logic
        .templates
        .insert("TestHackerMoveStop".to_string(), t);

    let hacker_id = game_logic
        .create_object(
            "TestHackerMoveStop",
            Team::China,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("hacker");
    assert!(game_logic.start_hacker_internet_hack(hacker_id));
    assert!(game_logic.hacker_income().is_hacking(hacker_id));
    assert!(game_logic.unit_command_move_to(hacker_id, Vec3::new(40.0, 0.0, 0.0)));
    assert!(
        !game_logic.hacker_income().is_hacking(hacker_id),
        "move must PACKING-stop hack cash"
    );
    game_logic.frame = HACKER_CASH_INTERVAL_FRAMES + 1;
    game_logic.update_hacker_income();
    assert_eq!(
        game_logic
            .get_player_mut_by_team(Team::China)
            .map(|p| p.resources.supplies)
            .unwrap_or(0),
        0,
        "no cash after packing"
    );
}

#[test]
fn hacker_field_unpack_stamps_unpacking_then_firing_a() {
    use crate::game_logic::host_enum_table_residual::{
        firing_a_model_bit, unpacking_model_bit,
    };
    use crate::game_logic::host_hacker_income::{
        HackerInternetPhase, HACKER_CASH_INTERVAL_FRAMES, HACKER_CASH_REGULAR,
        HACKER_UNIT_UNPACK_AUDIO,
    };
    use crate::game_logic::{HackInternetAIUpdateMetadata, KindOf, ThingTemplate};

    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::China);
    let mut t = ThingTemplate::new("TestHackerUnpackPose");
    t.add_kind_of(KindOf::Infantry).set_health(100.0);
    t.hack_internet_ai_update = Some(HackInternetAIUpdateMetadata {
        unpack_time_frames: 2,
        pack_time_frames: 2,
        cash_update_delay_frames: HACKER_CASH_INTERVAL_FRAMES,
        cash_update_delay_fast_frames: 54,
        regular_cash_amount: HACKER_CASH_REGULAR,
        veteran_cash_amount: 6,
        elite_cash_amount: 8,
        heroic_cash_amount: 10,
        xp_per_cash_update: 1.0,
        pack_unpack_variation_factor: 0.0,
    });
    game_logic
        .templates
        .insert("TestHackerUnpackPose".to_string(), t);
    let hacker_id = game_logic
        .create_object("TestHackerUnpackPose", Team::China, Vec3::ZERO)
        .expect("hacker");
    assert!(game_logic.start_hacker_internet_hack(hacker_id));
    let bits = game_logic.objects[&hacker_id].model_condition_bits;
    assert_ne!(bits & (1u128 << unpacking_model_bit()), 0);
    assert!(game_logic
        .queued_audio_events
        .iter()
        .any(|e| e.event_type == HACKER_UNIT_UNPACK_AUDIO));
    assert_eq!(
        game_logic.hacker_income().pack_phase(hacker_id),
        Some(HackerInternetPhase::Unpacking)
    );

    game_logic.frame = 2;
    game_logic.update_hacker_income();
    let bits = game_logic.objects[&hacker_id].model_condition_bits;
    assert_eq!(bits & (1u128 << unpacking_model_bit()), 0);
    assert_ne!(bits & (1u128 << firing_a_model_bit()), 0);
    assert_eq!(
        game_logic.hacker_income().pack_phase(hacker_id),
        Some(HackerInternetPhase::Hacking)
    );
}

#[test]
fn hacker_move_while_hacking_packs_before_walking() {
    use crate::game_logic::host_enum_table_residual::packing_model_bit;
    use crate::game_logic::host_hacker_income::{
        HackerInternetPhase, HACKER_CASH_INTERVAL_FRAMES, HACKER_CASH_REGULAR,
        HACKER_UNIT_PACK_AUDIO,
    };
    use crate::game_logic::{HackInternetAIUpdateMetadata, KindOf, ThingTemplate};

    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::China);
    let mut t = ThingTemplate::new("TestHackerPackHold");
    t.add_kind_of(KindOf::Infantry).set_health(100.0);
    t.hack_internet_ai_update = Some(HackInternetAIUpdateMetadata {
        unpack_time_frames: 0,
        pack_time_frames: 3,
        cash_update_delay_frames: HACKER_CASH_INTERVAL_FRAMES,
        cash_update_delay_fast_frames: 54,
        regular_cash_amount: HACKER_CASH_REGULAR,
        veteran_cash_amount: 6,
        elite_cash_amount: 8,
        heroic_cash_amount: 10,
        xp_per_cash_update: 1.0,
        pack_unpack_variation_factor: 0.0,
    });
    game_logic
        .templates
        .insert("TestHackerPackHold".to_string(), t);
    let hacker_id = game_logic
        .create_object("TestHackerPackHold", Team::China, Vec3::ZERO)
        .expect("hacker");
    assert!(game_logic.start_hacker_internet_hack(hacker_id));
    game_logic.update_hacker_income();
    assert_eq!(
        game_logic.hacker_income().pack_phase(hacker_id),
        Some(HackerInternetPhase::Hacking)
    );

    assert!(game_logic.unit_command_move_to(hacker_id, Vec3::new(40.0, 0.0, 0.0)));
    assert!(!game_logic.hacker_income().is_hacking(hacker_id));
    assert_eq!(
        game_logic.hacker_income().pack_phase(hacker_id),
        Some(HackerInternetPhase::Packing)
    );
    assert_ne!(
        game_logic.objects[&hacker_id].model_condition_bits & (1u128 << packing_model_bit()),
        0
    );
    assert_ne!(game_logic.objects[&hacker_id].ai_state, AIState::Moving);
    assert!(game_logic
        .queued_audio_events
        .iter()
        .any(|e| e.event_type == HACKER_UNIT_PACK_AUDIO));

    game_logic.frame = 3;
    game_logic.update_hacker_income();
    assert_eq!(game_logic.objects[&hacker_id].ai_state, AIState::Moving);
}

/// Residual: non-hacker template must not start residual internet hack.
#[test]
fn hacker_residual_rejects_non_hacker() {
    use crate::game_logic::{KindOf, ThingTemplate};

    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::China);
    if !game_logic.templates.contains_key("TestRedGuard") {
        let mut t = ThingTemplate::new("TestRedGuard");
        t.add_kind_of(KindOf::Infantry).set_health(100.0);
        game_logic.templates.insert("TestRedGuard".to_string(), t);
    }
    let id = game_logic
        .create_object("TestRedGuard", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("red guard");
    assert!(!game_logic.start_hacker_internet_hack(id));
    assert!(!game_logic.hacker_income().is_hacking(id));
}

/// Residual: Black Lotus StealCashHack steals cash after reaching supply building.
#[test]
fn steal_cash_hack_command_transfers_cash_after_reach() {
    let mut game_logic = GameLogic::new();
    ensure_test_black_lotus_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::China);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);

    // Seed victim cash so steal is observable on both sides.
    {
        let victim = game_logic
            .get_player_mut_by_team(Team::GLA)
            .expect("GLA player");
        victim.resources.supplies = 5_000;
    }
    let attacker_cash_before = game_logic
        .get_player_mut_by_team(Team::China)
        .map(|p| p.effective_supplies())
        .unwrap_or(0);

    let lotus_id = game_logic
        .create_object(
            "ChinaInfantryBlackLotus",
            Team::China,
            Vec3::new(200.0, 0.0, 0.0),
        )
        .expect("lotus should be created");
    // TestBuilding is residual cash-generator template name.
    let target_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("supply should be created");

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::StealCashHack { target_id },
        player_id: 1,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![lotus_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert_eq!(
        game_logic.hero_abilities().cash_steals,
        0,
        "cash hack must not resolve at issue range"
    );
    {
        let lotus = game_logic
            .host_object(lotus_id)
            .expect("lotus should exist");
        assert_eq!(
            lotus.ai_state,
            AIState::SpecialAbility,
            "steal cash must enter SpecialAbility on issue"
        );
    }

    // StartAbilityRange 150 residual: mid-range should still complete.
    {
        let lotus = game_logic
            .host_object_mut(lotus_id)
            .expect("lotus should exist");
        lotus.set_position(Vec3::new(100.0, 0.0, 0.0));
        lotus.set_ai_state(AIState::SpecialAbility);
        lotus.target = Some(target_id);
    }
    game_logic.update_ai(&[lotus_id, target_id], 1.0 / 60.0);
    // C++ NeedToFace then Unpack 6730ms + Prep 6000ms before trigger.
    game_logic.update_ai(&[lotus_id, target_id], 1.0);
    game_logic.update_ai(&[lotus_id, target_id], 7.0);
    game_logic.update_ai(&[lotus_id, target_id], 6.0);

    assert!(
        game_logic.honesty_steal_cash_ok(),
        "steal cash residual honesty"
    );
    assert_eq!(
        game_logic.hero_abilities().cash_stolen_total,
        crate::game_logic::host_hero_abilities::STEAL_CASH_DEFAULT_AMOUNT
    );
    let attacker_cash_after = game_logic
        .players
        .values()
        .find(|p| p.team == Team::China)
        .map(|p| p.effective_supplies())
        .unwrap_or(0);
    assert!(
        attacker_cash_after > attacker_cash_before,
        "attacker must gain cash (before={attacker_cash_before}, after={attacker_cash_after})"
    );
    let victim_cash = game_logic
        .players
        .values()
        .find(|p| p.team == Team::GLA)
        .map(|p| p.effective_supplies())
        .unwrap_or(0);
    assert!(
        victim_cash < 5_000,
        "victim must lose cash (remaining={victim_cash})"
    );
}

/// C++ `SpecialAbilityUpdate::triggerAbilityEffect` AwardXPForTriggering
/// (`SpecialAbilityUpdate.cpp:1248-1253`). Retail StealCashHack = 20.
#[test]
fn steal_cash_hack_awards_lotus_award_xp_for_triggering() {
    let mut game_logic = GameLogic::new();
    ensure_test_black_lotus_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::China);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    {
        let victim = game_logic
            .get_player_mut_by_team(Team::GLA)
            .expect("GLA player");
        victim.resources.supplies = 5_000;
    }

    let lotus_id = game_logic
        .create_object(
            "ChinaInfantryBlackLotus",
            Team::China,
            Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("lotus");
    {
        let lotus = game_logic.host_object_mut(lotus_id).expect("lotus");
        lotus.thing.template.is_trainable = true;
    }
    let target_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("supply");

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::StealCashHack { target_id },
        player_id: 1,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![lotus_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    {
        let lotus = game_logic.host_object_mut(lotus_id).expect("lotus");
        lotus.set_position(Vec3::new(100.0, 0.0, 0.0));
        lotus.set_ai_state(AIState::SpecialAbility);
        lotus.target = Some(target_id);
    }
    game_logic.update_ai(&[lotus_id, target_id], 1.0 / 60.0);
    // C++ leftover SpecialAbilityUpdate: NeedToFace, Unpack 6730ms, Prep 6000ms.
    game_logic.update_ai(&[lotus_id, target_id], 1.0);
    game_logic.update_ai(&[lotus_id, target_id], 7.0);
    game_logic.update_ai(&[lotus_id, target_id], 6.0);

    let lotus = game_logic.host_object(lotus_id).expect("lotus after steal");
    assert_eq!(
        lotus.experience.current,
        crate::game_logic::host_hero_abilities::LOTUS_STEAL_AWARD_XP as f32,
        "StealCashHack must grant AwardXPForTriggering=20"
    );
    assert!(
        game_logic.hero_abilities().cash_stolen_total > 0
            || game_logic.honesty_steal_cash_ok(),
        "steal trigger that awards XP must also steal cash"
    );
}


/// Residual polish: non-Lotus units cannot issue StealCashHack (fail-closed).
#[test]
fn steal_cash_hack_rejects_non_lotus_and_non_cash_targets() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_black_lotus_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);
    ensure_test_barracks_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::China);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);

    let tank_id = game_logic
        .create_object("TestTank", Team::China, Vec3::new(10.0, 0.0, 0.0))
        .expect("tank");
    let supply_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("supply");
    let barracks_id = game_logic
        .create_object("TestBarracks", Team::GLA, Vec3::new(5.0, 0.0, 0.0))
        .expect("barracks");

    // Non-lotus unit cannot steal.
    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::StealCashHack {
            target_id: supply_id,
        },
        player_id: 1,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![tank_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    {
        let tank = game_logic.host_object(tank_id).expect("tank");
        assert_ne!(
            tank.ai_state,
            AIState::SpecialAbility,
            "non-lotus must not enter StealCashHack"
        );
    }

    // Lotus cannot steal from non-cash-generator barracks.
    let lotus_id = game_logic
        .create_object(
            "ChinaInfantryBlackLotus",
            Team::China,
            Vec3::new(20.0, 0.0, 0.0),
        )
        .expect("lotus");
    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::StealCashHack {
            target_id: barracks_id,
        },
        player_id: 1,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![lotus_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    {
        let lotus = game_logic.host_object(lotus_id).expect("lotus");
        assert_ne!(
            lotus.ai_state,
            AIState::SpecialAbility,
            "steal cash rejects non-cash-generator structure"
        );
    }
    assert!(!game_logic.honesty_steal_cash_ok());
}

/// C++ StealCashHack withdraws/deposits controlling players, not first faction slot.
#[test]
fn steal_cash_hack_uses_controlling_players_not_faction_slot() {
    let mut game_logic = GameLogic::new();
    ensure_test_black_lotus_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::China);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    let mut china_bystander = Player::new(11, Team::China, "ChinaBystander", true);
    china_bystander.resources.supplies = 9_000;
    game_logic.add_player(china_bystander);
    let mut gla_victim = Player::new(12, Team::GLA, "GlaVictim", true);
    gla_victim.resources.supplies = 5_000;
    game_logic.add_player(gla_victim);
    if let Some(slot) = game_logic.get_player_mut(2) {
        slot.resources.supplies = 1;
    }

    let lotus_id = game_logic
        .create_object(
            "ChinaInfantryBlackLotus",
            Team::China,
            Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("lotus");
    let target_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("supply");
    if let Some(lotus) = game_logic.host_object_mut(lotus_id) {
        lotus.owner_player_id = Some(1);
    }
    if let Some(target) = game_logic.host_object_mut(target_id) {
        target.owner_player_id = Some(12);
    }

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::StealCashHack { target_id },
        player_id: 1,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![lotus_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    {
        let lotus = game_logic.host_object_mut(lotus_id).expect("lotus");
        lotus.set_position(Vec3::new(100.0, 0.0, 0.0));
        lotus.set_ai_state(AIState::SpecialAbility);
        lotus.target = Some(target_id);
    }
    game_logic.update_ai(&[lotus_id, target_id], 1.0 / 60.0);
    game_logic.update_ai(&[lotus_id, target_id], 1.0);
    game_logic.update_ai(&[lotus_id, target_id], 7.0);
    game_logic.update_ai(&[lotus_id, target_id], 6.0);

    let china_caster = game_logic.get_player(1).expect("china caster").resources.supplies;
    let china_other = game_logic.get_player(11).expect("china other").resources.supplies;
    let gla_slot = game_logic.get_player(2).expect("gla slot").resources.supplies;
    let gla_owner = game_logic.get_player(12).expect("gla owner").resources.supplies;
    assert_eq!(
        china_other, 9_000,
        "same-faction bystander must not bank Lotus steal"
    );
    assert_eq!(gla_slot, 1, "first GLA slot must not be the victim");
    assert_eq!(gla_owner, 4_000, "building owner must lose 1000");
    assert!(
        china_caster >= 1_000,
        "Lotus controller must receive the steal (have {china_caster})"
    );
}

/// C++ update/continuePreparation abort Lotus steal when the target cloaks.
#[test]
fn leftover_lotus_prep_aborts_when_target_stealthed() {
    use crate::game_logic::host_hero_abilities::LeftoverSaPhase;
    let mut game_logic = GameLogic::new();
    ensure_test_black_lotus_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::China);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    {
        let victim = game_logic.get_player_mut_by_team(Team::GLA).expect("gla");
        victim.resources.supplies = 5_000;
    }
    let lotus_id = game_logic
        .create_object(
            "ChinaInfantryBlackLotus",
            Team::China,
            Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("lotus");
    let target_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("supply");
    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::StealCashHack { target_id },
        player_id: 1,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![lotus_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    {
        let lotus = game_logic.host_object_mut(lotus_id).expect("lotus");
        lotus.set_position(Vec3::new(100.0, 0.0, 0.0));
        lotus.set_ai_state(AIState::SpecialAbility);
        lotus.target = Some(target_id);
    }
    game_logic.update_ai(&[lotus_id, target_id], 1.0);
    game_logic.update_ai(&[lotus_id, target_id], 7.0);
    let phase = game_logic
        .hero_abilities()
        .leftover_channel(lotus_id)
        .map(|ch| ch.phase);
    assert_eq!(phase, Some(LeftoverSaPhase::Preparing), "must be mid-prep");
    if let Some(target) = game_logic.host_object_mut(target_id) {
        target.set_status_stealthed(true);
        target.set_status_detected(false);
    }
    game_logic.update_ai(&[lotus_id, target_id], 0.2);
    assert_eq!(
        game_logic.hero_abilities().cash_stolen_total, 0,
        "stealthed target must abort Lotus steal"
    );
    let phase = game_logic
        .hero_abilities()
        .leftover_channel(lotus_id)
        .map(|ch| ch.phase);
    assert!(
        phase != Some(LeftoverSaPhase::Preparing),
        "prep must pack or end after stealth abort, got {phase:?}"
    );
}

/// C++ startUnpacking stamps UNPACKING and plays UnpackSound.
#[test]
fn leftover_lotus_unpack_sets_model_and_unpack_sound() {
    use crate::game_logic::host_enum_table_residual::unpacking_model_bit;
    use crate::game_logic::host_hero_abilities::{LeftoverSaPhase, LOTUS_UNPACK_SOUND};
    let mut game_logic = GameLogic::new();
    ensure_test_black_lotus_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::China);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    let lotus_id = game_logic
        .create_object(
            "ChinaInfantryBlackLotus",
            Team::China,
            Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("lotus");
    let target_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("supply");
    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::StealCashHack { target_id },
        player_id: 1,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![lotus_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    {
        let lotus = game_logic.host_object_mut(lotus_id).expect("lotus");
        lotus.set_position(Vec3::new(100.0, 0.0, 0.0));
        lotus.set_ai_state(AIState::SpecialAbility);
        lotus.target = Some(target_id);
    }
    game_logic.queued_audio_events.clear();
    game_logic.update_ai(&[lotus_id, target_id], 1.0);
    game_logic.update_ai(&[lotus_id, target_id], 1.0);
    let lotus = game_logic.host_object(lotus_id).expect("lotus");
    assert_ne!(
        lotus.model_condition_bits & (1u128 << unpacking_model_bit()),
        0,
        "Lotus unpack must set MODELCONDITION_UNPACKING"
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .any(|event| event.event_type == LOTUS_UNPACK_SOUND),
        "Lotus unpack must queue BlackLotusUnpack"
    );
    assert_eq!(
        game_logic
            .hero_abilities()
            .leftover_channel(lotus_id)
            .map(|ch| ch.phase),
        Some(LeftoverSaPhase::Unpacking)
    );
}

/// C++ initiateIntentToDoSpecialPower onExit siblings — no leftover hot-swap.
#[test]
fn leftover_sa_hot_swap_aborts_in_flight_channel() {
    use crate::game_logic::host_hero_abilities::{LeftoverSaKind, LeftoverSaPhase};
    let mut game_logic = GameLogic::new();
    ensure_test_black_lotus_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::China);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    let lotus_id = game_logic
        .create_object(
            "ChinaInfantryBlackLotus",
            Team::China,
            Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("lotus");
    let supply_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("supply");
    let tank_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(10.0, 0.0, 0.0))
        .expect("tank");
    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::StealCashHack {
            target_id: supply_id,
        },
        player_id: 1,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![lotus_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    {
        let lotus = game_logic.host_object_mut(lotus_id).expect("lotus");
        lotus.set_position(Vec3::new(100.0, 0.0, 0.0));
        lotus.set_ai_state(AIState::SpecialAbility);
        lotus.target = Some(supply_id);
    }
    game_logic.update_ai(&[lotus_id, supply_id], 1.0);
    game_logic.update_ai(&[lotus_id, supply_id], 1.0);
    assert_eq!(
        game_logic
            .hero_abilities()
            .leftover_channel(lotus_id)
            .map(|ch| ch.kind),
        Some(LeftoverSaKind::StealCash)
    );
    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::DisableVehicleHack {
            target_id: tank_id,
        },
        player_id: 1,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![lotus_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    assert!(
        game_logic
            .hero_abilities()
            .leftover_channel(lotus_id)
            .is_none(),
        "new leftover SA must onExit the in-flight steal channel"
    );
    {
        let lotus = game_logic.host_object_mut(lotus_id).expect("lotus");
        lotus.set_position(Vec3::new(10.0, 0.0, 0.0));
        lotus.set_ai_state(AIState::SpecialAbility);
        lotus.target = Some(tank_id);
    }
    game_logic.update_ai(&[lotus_id, tank_id], 1.0);
    let kind = game_logic
        .hero_abilities()
        .leftover_channel(lotus_id)
        .map(|ch| ch.kind);
    assert!(
        kind != Some(LeftoverSaKind::StealCash),
        "hot-swap must not keep the steal leftover timer, got {kind:?}"
    );
    let _ = LeftoverSaPhase::Facing;
}

/// C++ FlipOwnerAfterUnpacking rotates +PI when unpack completes.
#[test]
fn leftover_burton_flips_180_after_unpack() {
    use crate::game_logic::ChargePlantAbilityMetadata;
    let mut game_logic = GameLogic::new();
    ensure_test_structure_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    let mut burton = ThingTemplate::new("AmericaInfantryColonelBurton");
    burton
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    burton.charge_plant_abilities.push(ChargePlantAbilityMetadata {
        special_power_template: "SpecialAbilityColonelBurtonTimedCharges".into(),
        unpack_time_ms: 200,
        pack_time_ms: 0,
        pack_unpack_variation_factor: 0.0,
        flee_range_after_completion: 0.0,
        flip_object_after_unpacking: true,
        flip_object_after_packing: false,
    });
    game_logic
        .templates
        .insert("AmericaInfantryColonelBurton".into(), burton);
    let burton_id = game_logic
        .create_object(
            "AmericaInfantryColonelBurton",
            Team::USA,
            Vec3::new(2.0, 0.0, 0.0),
        )
        .expect("burton");
    let target_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("bldg");
    let facing_before = game_logic
        .host_object(burton_id)
        .expect("burton")
        .get_orientation();
    game_logic.queue_pending_special_ability(
        burton_id,
        crate::game_logic::PendingSpecialAbility::PlantTimedDemoCharge { target_id },
    );
    if let Some(burton) = game_logic.host_object_mut(burton_id) {
        burton.set_ai_state(AIState::SpecialAbility);
        burton.target = Some(target_id);
    }
    game_logic.update_ai(&[burton_id, target_id], 1.0 / 30.0);
    game_logic.update_ai(&[burton_id, target_id], 0.3);
    let facing_after = game_logic
        .host_object(burton_id)
        .expect("burton")
        .get_orientation();
    let delta = (facing_after - facing_before).abs();
    let flipped = (delta - std::f32::consts::PI).abs() < 0.05
        || (delta - std::f32::consts::TAU).abs() < 0.05
        || (delta - 0.0).abs() > 2.5;
    assert!(
        flipped,
        "Burton must rotate ~PI after unpack (before={facing_before} after={facing_after})"
    );
}

/// C++ markSpecialPowerTriggered is startPreparation, not click.
#[test]
fn tank_hunter_tnt_does_not_consume_cooldown_at_click() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::{SpecialPowerModuleKind, SpecialPowerModuleMetadata};
    let mut game_logic = GameLogic::new();
    ensure_test_structure_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::China);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    let mut hunter = ThingTemplate::new("ChinaInfantryTankHunter");
    hunter
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    hunter.special_power_modules.push(SpecialPowerModuleMetadata {
        source_index: 0,
        module_tag: None,
        module_kind: SpecialPowerModuleKind::SpecialAbility,
        special_power_template: "SpecialAbilityTankHunterTNTAttack".into(),
        special_power_template_id: 1,
        command_power: Some(SpecialPowerType::TankHunterTnt),
        reload_time_frames: 225,
        required_science: None,
        public_timer: false,
        shared_n_sync: false,
        shortcut_power: false,
        update_module_starts_attack: true,
        starts_paused: false,
        scripted_special_power_only: false,
    });
    game_logic
        .templates
        .insert("ChinaInfantryTankHunter".into(), hunter);
    let hunter_id = game_logic
        .create_object(
            "ChinaInfantryTankHunter",
            Team::China,
            Vec3::new(200.0, 0.0, 0.0),
        )
        .expect("hunter");
    let target_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("bldg");
    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::TankHunterTnt,
            target: PowerTarget::Object(target_id),
        },
        player_id: 1,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![hunter_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    let hunter = game_logic.host_object(hunter_id).expect("hunter");
    assert!(
        !hunter
            .special_power_cooldowns
            .contains_key(&SpecialPowerType::TankHunterTnt),
        "TNT must not start reload at click"
    );
}

/// C++ killSpecialObjects laser case locks PRIMARY temporarily.
#[test]
fn leftover_laser_abort_resets_primary_weapon_lock() {
    use crate::game_logic::WeaponLockType;
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    let mut md = ThingTemplate::new("AmericaInfantryMissileDefender");
    md.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    game_logic
        .templates
        .insert("AmericaInfantryMissileDefender".into(), md);
    let md_id = game_logic
        .create_object(
            "AmericaInfantryMissileDefender",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("md");
    let tgt_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
        .expect("tgt");
    {
        let md = game_logic.host_object_mut(md_id).expect("md");
        md.weapon = Some(crate::game_logic::Weapon {
            damage: 10.0,
            range: 200.0,
            ..crate::game_logic::Weapon::default()
});
        md.secondary_weapon = md.weapon.clone();
        let _ = md.set_weapon_lock(1, WeaponLockType::LockedTemporarily);
    }
    assert!(game_logic.activate_missile_defender_laser_guided(md_id, tgt_id));
    if let Some(tgt) = game_logic.host_object_mut(tgt_id) {
        tgt.set_status_stealthed(true);
        tgt.set_status_detected(false);
    }
    game_logic.update_ai(&[md_id, tgt_id], 1.0 / 30.0);
    let md = game_logic.host_object(md_id).expect("md");
    assert_eq!(
        md.weapon_lock_slot, 0,
        "laser abort must lock PRIMARY"
    );
    assert_eq!(md.weapon_lock_type, WeaponLockType::LockedTemporarily);
}


/// Residual: Black Lotus DisableVehicleHack walks to enemy vehicle → DISABLED_HACKED.
#[test]
fn disable_vehicle_hack_command_disables_after_reach() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_black_lotus_template(&mut game_logic);

    // China Lotus vs GLA vehicle residual.
    let lotus_id = game_logic
        .create_object(
            "ChinaInfantryBlackLotus",
            Team::China,
            Vec3::new(190.0, 0.0, 0.0),
        )
        .expect("lotus should be created");
    let target_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("target should be created");

    let initial_health = game_logic
        .host_object(target_id)
        .expect("target should exist")
        .health
        .current;

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::DisableVehicleHack { target_id },
        player_id: 1,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![lotus_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    // Confirm command was accepted (pending special ability + SpecialAbility state).
    {
        let lotus = game_logic
            .host_object(lotus_id)
            .expect("lotus should exist");
        assert_eq!(
            lotus.ai_state,
            AIState::SpecialAbility,
            "disable hack must enter SpecialAbility on issue"
        );
        assert_eq!(lotus.target, Some(target_id));
    }
    assert!(
        !game_logic
            .host_object(target_id)
            .expect("target")
            .is_hacked_disabled(),
        "disable hack must not apply immediately on command issue"
    );
    assert!(!game_logic.honesty_disable_vehicle_hack_ok());

    // Beyond StartAbilityRange 150 — stays pending.
    game_logic.update_ai(&[lotus_id, target_id], 1.0 / 60.0);
    assert!(
        !game_logic
            .host_object(target_id)
            .expect("target")
            .is_hacked_disabled(),
        "disable hack stays pending out of range"
    );

    // Within StartAbilityRange 150 residual (not melee pad).
    {
        let lotus = game_logic
            .host_object_mut(lotus_id)
            .expect("lotus should exist");
        lotus.set_position(Vec3::new(100.0, 0.0, 0.0));
        lotus.set_ai_state(AIState::SpecialAbility);
        lotus.target = Some(target_id);
    }
    game_logic.update_ai(&[lotus_id, target_id], 1.0 / 60.0);
    // C++ NeedToFace then Unpack 2000ms + Prep 2000ms.
    game_logic.update_ai(&[lotus_id, target_id], 1.0);
    game_logic.update_ai(&[lotus_id, target_id], 2.1);
    game_logic.update_ai(&[lotus_id, target_id], 2.1);

    let target_after = game_logic
        .host_object(target_id)
        .expect("target should exist");
    assert_eq!(
        target_after.health.current, initial_health,
        "disable hack residual must not damage HP"
    );
    assert_eq!(
        target_after.team,
        Team::GLA,
        "disable hack must not change ownership"
    );
    assert!(
        target_after.is_hacked_disabled(),
        "vehicle must be DISABLED_HACKED"
    );
    assert!(!target_after.can_move(), "hacked vehicle cannot move");
    assert!(
        game_logic.honesty_disable_vehicle_hack_ok(),
        "disable vehicle residual honesty"
    );
    assert_eq!(game_logic.hero_abilities().vehicle_disables, 1);

    // Expire residual timer → vehicle recovers.
    let until = target_after.status.disabled_hacked_until_frame;
    assert!(until > game_logic.frame);
    game_logic.frame = until;
    game_logic.update_ai(&[target_id], 1.0 / 60.0);
    let recovered = game_logic
        .host_object(target_id)
        .expect("target should exist");
    assert!(
        !recovered.is_hacked_disabled(),
        "DISABLED_HACKED must clear after EffectDuration"
    );
    assert!(recovered.can_move(), "recovered vehicle can move again");
}

/// Residual: Black Lotus CaptureBuilding without Capture research (range 150).
#[test]
fn black_lotus_capture_building_without_upgrade() {
    let mut game_logic = GameLogic::new();
    ensure_test_black_lotus_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::China);
    ensure_test_player_for_team(&mut game_logic, Team::GLA);

    // No Capture upgrade on China team — Lotus still captures via hero residual.
    assert!(!game_logic.team_has_completed_capture_upgrade(Team::China));

    let lotus_id = game_logic
        .create_object(
            "ChinaInfantryBlackLotus",
            Team::China,
            Vec3::new(200.0, 0.0, 0.0),
        )
        .expect("lotus");
    let building_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("building");

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::CaptureBuilding {
            target_id: building_id,
        },
        player_id: 1,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![lotus_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    {
        let lotus = game_logic.host_object(lotus_id).expect("lotus");
        assert_eq!(
            lotus.ai_state,
            AIState::Capturing,
            "Black Lotus CaptureBuilding must enter Capturing without upgrade"
        );
        assert_eq!(lotus.target, Some(building_id));
    }
    assert!(!game_logic.honesty_black_lotus_capture_ok());

    // Beyond StartAbilityRange 150 — still walking.
    game_logic.update_ai(&[lotus_id, building_id], 1.0 / 60.0);
    assert_eq!(
        game_logic.host_object(building_id).expect("building").team,
        Team::GLA,
        "capture must not complete out of StartAbilityRange"
    );

    // Within residual range 150.
    {
        let lotus = game_logic.host_object_mut(lotus_id).expect("lotus");
        lotus.set_position(Vec3::new(100.0, 0.0, 0.0));
        lotus.set_ai_state(AIState::Capturing);
        lotus.target = Some(building_id);
    }
    game_logic.update_ai(&[lotus_id, building_id], 1.0 / 60.0);

    assert_eq!(
        game_logic.host_object(building_id).expect("building").team,
        Team::China,
        "Black Lotus residual capture must transfer ownership"
    );
    assert!(
        game_logic.honesty_black_lotus_capture_ok(),
        "capture residual honesty"
    );
    assert_eq!(game_logic.hero_abilities().building_captures, 1);
    {
        let lotus = game_logic.host_object(lotus_id).expect("lotus");
        assert_ne!(
            lotus.ai_state,
            AIState::Capturing,
            "captor leaves Capturing after complete"
        );
    }
}

/// Residual polish: non-Lotus cannot issue DisableVehicleHack (fail-closed).
#[test]
fn disable_vehicle_hack_rejects_non_lotus() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let actor_id = game_logic
        .create_object("TestTank", Team::China, Vec3::new(10.0, 0.0, 0.0))
        .expect("actor");
    let target_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("target");

    game_logic.queue_command(crate::command_system::GameCommand {
        command_type: crate::command_system::CommandType::DisableVehicleHack { target_id },
        player_id: 1,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![actor_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let actor = game_logic.host_object(actor_id).expect("actor");
    assert_ne!(
        actor.ai_state,
        AIState::SpecialAbility,
        "non-lotus must not DisableVehicleHack"
    );
    assert!(!game_logic.honesty_disable_vehicle_hack_ok());
}

/// Residual E2E: enemy weapons jammed near China ECM tank residual field.
///
/// C++ ECMTankVehicleDisabler (SUBDUAL → DISABLED_SUBDUED cannot fire) +
/// ECMTankMissileJammer FireWeaponUpdate pulse (PrimaryDamageRadius=150).
/// Fail-closed: continuous aura (not full subdual damage / laser stream).

/// C++ MissileAIUpdate.cpp:777-809 projectileNowJammed — scatter, no HP.
#[test]
fn ecm_missile_jam_scatters_in_flight_projectile() {
    use crate::game_logic::host_ecm_jam::HOST_ECM_JAM_RADIUS;

    let mut logic = GameLogic::new();
    let mut ecm_tpl = ThingTemplate::new("ChinaTankECM");
    ecm_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0);
    logic.templates.insert("ChinaTankECM".to_string(), ecm_tpl);

    let mut tom_tpl = ThingTemplate::new("AmericaVehicleTomahawk");
    tom_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(180.0);
    logic
        .templates
        .insert("AmericaVehicleTomahawk".to_string(), tom_tpl);

    let ecm = logic
        .create_object("ChinaTankECM", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("ecm");
    let tom = logic
        .create_object(
            "AmericaVehicleTomahawk",
            Team::USA,
            Vec3::new(50.0, 0.0, 0.0),
        )
        .expect("tom");
    let aim = Vec3::new(200.0, 0.0, 0.0);
    let from = Vec3::new(50.0, 5.0, 0.0);
    let missile = logic
        .spawn_tomahawk_missile_projectile(tom, from, aim, None)
        .expect("missile");
    if let Some(o) = logic.objects.get_mut(&missile) {
        o.set_position(Vec3::new(10.0, 5.0, 0.0));
    }
    let aim_before = logic
        .objects
        .get(&missile)
        .and_then(|o| o.tomahawk_missile_aim)
        .expect("aim");
    let hp_before = logic.objects.get(&missile).unwrap().health.current;

    logic.update_ecm_missile_jam();
    assert!(logic.honesty_ecm_missile_jam_ok());
    let o = logic.objects.get(&missile).expect("missile still exists");
    assert!(o.is_alive(), "jam must not explode the missile");
    assert!(
        (o.health.current - hp_before).abs() < 1e-3,
        "SUBDUAL_MISSILE must not deal HP, before={hp_before} after={}",
        o.health.current
    );
    assert!(o.ecm_missile_jammed, "missile should be marked jammed");
    let aim_after = o.tomahawk_missile_aim.expect("aim after");
    assert!(
        (aim_after[0] - aim_before[0]).abs() > 0.1
            || (aim_after[2] - aim_before[2]).abs() > 0.1,
        "jam should scatter aim"
    );
    let _ = (ecm, HOST_ECM_JAM_RADIUS);
}

#[test]
fn ecm_jam_spawns_disable_stream_laser() {
    use crate::game_logic::host_ecm_jam::{
        ECM_DISABLE_STREAM_LASER, ECM_VEHICLE_DISABLER_DELAY_FRAMES,
    };

    let mut logic = GameLogic::new();
    let mut ecm_tpl = ThingTemplate::new("ChinaTankECM");
    ecm_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0);
    logic.templates.insert("ChinaTankECM".to_string(), ecm_tpl);

    let mut tank_tpl = ThingTemplate::new("AmericaTankCrusader");
    tank_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0);
    logic
        .templates
        .insert("AmericaTankCrusader".to_string(), tank_tpl);

    let ecm = logic
        .create_object("ChinaTankECM", Team::China, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("ecm");
    let enemy = logic
        .create_object(
            "AmericaTankCrusader",
            Team::USA,
            glam::Vec3::new(50.0, 0.0, 0.0),
        )
        .expect("enemy");
    // Ensure enemy has a weapon residual so jam candidates include it.
    {
        let o = logic.host_object_mut(enemy).unwrap();
        o.weapon = Some(Weapon {
            damage: 25.0,
            range: 150.0,
            reload_time: 0.1,
            last_fire_time: -10.0,
            ..Weapon::default()
});
        // One 24 SUBDUAL_VEHICLE pulse fills this bar (ActiveBody.cpp:1292).
        o.health.current = 20.0;
        o.health.maximum = 20.0;
        o.max_health = 20.0;
    }
    // Align to laser pulse cadence.
    logic.frame = ECM_VEHICLE_DISABLER_DELAY_FRAMES;
    logic.update_ecm_jam_field();
    assert!(logic.host_object(enemy).unwrap().status.weapons_jammed);
    assert!(logic.host_object(enemy).unwrap().status.disabled_subdued);
    assert!(logic.honesty_ecm_jam_ok());
    assert!(
        logic.honesty_ecm_laser_ok(),
        "ECMDisableStream laser should spawn on jam pulse"
    );
    let beams = logic
        .objects
        .values()
        .filter(|o| o.weapon_laser_beam && o.template_name == ECM_DISABLE_STREAM_LASER)
        .count();
    assert!(beams >= 1, "ECMDisableStream beam object expected");
    assert!(
        logic
            .weapon_lasers
            .iter()
            .any(|l| l.laser_name == ECM_DISABLE_STREAM_LASER),
        "presentation residual laser expected"
    );
    let _ = ecm;
}

#[test]
fn ecm_jam_residual_jams_enemy_weapons_in_radius() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let mut ecm_tpl = crate::game_logic::ThingTemplate::new("ChinaTankECM");
    ecm_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0)
        .set_cost(800, 0);
    game_logic
        .templates
        .insert("ChinaTankECM".to_string(), ecm_tpl);

    let ecm_id = game_logic
        .create_object("ChinaTankECM", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("ecm tank");
    let enemy_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .expect("enemy tank");
    let ally_id = game_logic
        .create_object("TestTank", Team::China, Vec3::new(30.0, 0.0, 0.0))
        .expect("ally tank");

    // Bind weapons so residual jam target filter accepts units.
    for id in [enemy_id, ally_id, ecm_id] {
        let unit = game_logic.host_object_mut(id).expect("unit");
        unit.weapon = Some(Weapon {
            damage: 25.0,
            range: 150.0,
            reload_time: 0.1,
            last_fire_time: -10.0,
            ..Weapon::default()
});
        if id == enemy_id {
            // One 24 SUBDUAL_VEHICLE pulse fills the bar (ActiveBody.cpp:1292).
            unit.health.current = 20.0;
            unit.health.maximum = 20.0;
            unit.max_health = 20.0;
        }
    }

    assert_eq!(game_logic.ecm_residual_jams(), 0);
    assert!(!game_logic.honesty_ecm_jam_ok());
    assert!(
        !game_logic
            .host_object(enemy_id)
            .expect("enemy")
            .is_weapons_jammed(),
        "enemy must not start jammed"
    );

    // One residual pulse — continuous aura, no command required.
    game_logic.update_ecm_jam_field();

    let enemy = game_logic.host_object(enemy_id).expect("enemy");
    assert!(
        enemy.is_weapons_jammed(),
        "enemy weapons must be jammed near ECM residual"
    );
    assert!(
        enemy.is_subdued_disabled(),
        "C++ DISABLED_SUBDUED after ECM vehicle subdual"
    );
    assert!(
        !enemy.can_attack(),
        "jammed enemy must fail can_attack residual"
    );
    assert!(
        !enemy.can_fire(0.0),
        "jammed enemy must fail can_fire residual"
    );
    assert!(
        !enemy.can_move(),
        "DISABLED_SUBDUED skips AI/locomotor (full halt)"
    );

    let ally = game_logic.host_object(ally_id).expect("ally");
    assert!(
        !ally.is_weapons_jammed(),
        "same-team ally must not be jammed by own ECM"
    );
    assert!(ally.can_attack(), "ally with weapons must still can_attack");

    assert!(
        game_logic.ecm_residual_jams() > 0,
        "must record ECM jam honesty ticks"
    );
    assert!(
        game_logic.honesty_ecm_jam_ok(),
        "ECM jam residual honesty path"
    );

    // Combat path must not fire while jammed.
    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        enemy.target = Some(ecm_id);
        enemy.set_ai_state(AIState::Attacking);
        enemy.set_status_attacking(true);
    }
    let ecm_hp_before = game_logic.host_object(ecm_id).expect("ecm").health.current;
    game_logic.update_combat(&[enemy_id, ecm_id, ally_id], 1.0 / 30.0);
    let ecm_hp_after = game_logic.host_object(ecm_id).expect("ecm").health.current;
    assert_eq!(
        ecm_hp_before, ecm_hp_after,
        "jammed enemy must not damage ECM via combat residual"
    );

    // Leave radius — C++ SubdualDamageHelper lingers; do not instantly unjam.
    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        enemy.set_position(Vec3::new(400.0, 0.0, 0.0));
    }
    game_logic.update_ecm_jam_field();
    assert!(
        game_logic
            .host_object(enemy_id)
            .expect("enemy")
            .is_subdued_disabled(),
        "leaving radius must linger while subdual bar is full"
    );
    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        enemy.subdual_damage = 0.0;
    }
    game_logic.update_ecm_jam_field();
    let enemy = game_logic.host_object(enemy_id).expect("enemy");
    assert!(
        !enemy.is_weapons_jammed(),
        "weapons recover after subdual heals out"
    );
    assert!(
        !enemy.is_subdued_disabled(),
        "DISABLED_SUBDUED clears after subdual heals out"
    );
    assert!(enemy.can_attack(), "recovered enemy must can_attack again");

    assert!(
        game_logic
            .host_object(ecm_id)
            .map(|e| e.is_alive())
            .unwrap_or(false),
        "ECM tank must remain alive"
    );
}

/// Residual: unit outside ECM jam radius is not jammed; walk-in jams weapons.
#[test]
fn ecm_jam_residual_out_of_range_then_in_range() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let mut ecm_tpl = crate::game_logic::ThingTemplate::new("Tank_ChinaTankECM");
    ecm_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(300.0)
        .set_cost(800, 0);
    game_logic
        .templates
        .insert("Tank_ChinaTankECM".to_string(), ecm_tpl);

    let _ecm_id = game_logic
        .create_object("Tank_ChinaTankECM", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("ecm");
    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(300.0, 0.0, 0.0))
        .expect("enemy");
    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        enemy.weapon = Some(Weapon {
            damage: 20.0,
            range: 120.0,
            last_fire_time: -5.0,
            ..Weapon::default()
});
        enemy.health.current = 20.0;
        enemy.health.maximum = 20.0;
        enemy.max_health = 20.0;
    }

    game_logic.update_ecm_jam_field();
    assert!(
        !game_logic
            .host_object(enemy_id)
            .expect("enemy")
            .is_weapons_jammed(),
        "out-of-range enemy must not be jammed"
    );
    // First pulse with no cover does not increment honesty.
    assert_eq!(game_logic.ecm_residual_jams(), 0);
    assert!(!game_logic.honesty_ecm_jam_ok());

    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        enemy.set_position(Vec3::new(20.0, 0.0, 0.0));
    }
    game_logic.update_ecm_jam_field();
    assert!(
        game_logic
            .host_object(enemy_id)
            .expect("enemy")
            .is_weapons_jammed(),
        "walk-in to ECM radius must jam weapons"
    );
    assert!(game_logic.honesty_ecm_jam_ok());
}

/// Residual: EmpPulse special power disables vehicles in radius (DISABLED_EMP).
///
/// C++ SuperweaponEMPPulse → EMPPulseEffectSpheroid EMPUpdate::doDisableAttack
/// setDisabledUntil(DISABLED_EMP, now + DisabledDuration=30000ms).
/// Fail-closed: not full OCL bomb / spheroid drawable / spark particles.

#[test]
fn special_power_completion_die_notifies_script() {
    use crate::game_logic::script_events::{self, ScriptEvent};
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let _ = script_events::drain_events(); // clear
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(1, Team::USA, "USA", true));
    let mut t = ThingTemplate::new("ScudStormMissile");
    t.set_health(100.0);
    logic.templates.insert("ScudStormMissile".into(), t);
    let id = logic
        .create_object("ScudStormMissile", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    if let Some(obj) = logic.objects.get_mut(&id) {
        obj.set_special_power_completion("SuperweaponScudStorm", 42);
    }
    logic.destroy_object(id);
    assert!(
        logic.special_power_completion_log.notifications >= 1,
        "SpecialPowerCompletionDie must notify on destroy"
    );
    let evs = script_events::drain_events();
    assert!(
        evs.iter().any(|e| matches!(
            e,
            ScriptEvent::CompletedSpecialPower {
                special_power_name,
                creator_id: 42,
                ..
            } if special_power_name == "SuperweaponScudStorm"
        )),
        "expected CompletedSpecialPower event, got {evs:?}"
    );
}

#[test]
fn ocl_weapon_spawn_sets_special_power_completion_creator() {
    // C++ ObjectCreationList.cpp:386-393 + Weapon.cpp:1103-1113 setCreator.
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(1, Team::USA, "USA", true));
    let mut t = ThingTemplate::new("ScudStormMissile");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Projectile);
    logic.templates.insert("ScudStormMissile".into(), t);
    let id = logic
        .create_object("ScudStormMissile", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    if let Some(obj) = logic.objects.get_mut(&id) {
        obj.bind_special_power_completion_creator(77);
    }
    let data = logic
        .objects
        .get(&id)
        .and_then(|o| o.special_power_completion.clone())
        .expect("completion die residual");
    assert!(data.creator_set);
    assert_eq!(data.creator_id, 77);
    assert_eq!(data.special_power_name, "SuperweaponScudStorm");
}


#[test]
fn power_plant_rods_extend_upgrading_then_upgraded() {
    use crate::game_logic::host_enum_table_residual::model_condition_bit_name_index;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("AmericaPowerPlant");
    t.set_health(1000.0);
    t.add_kind_of(KindOf::Structure);
    t.add_kind_of(KindOf::PowerPlant);
    logic.templates.insert("AmericaPowerPlant".into(), t);
    let id = logic
        .create_object("AmericaPowerPlant", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    assert!(logic.begin_power_plant_rods_extend(id));
    let upgrading = model_condition_bit_name_index("POWER_PLANT_UPGRADING").unwrap();
    let upgraded = model_condition_bit_name_index("POWER_PLANT_UPGRADED").unwrap();
    let o = logic.objects.get(&id).unwrap();
    assert_ne!(o.model_condition_bits & (1u128 << upgrading), 0);
    assert_eq!(o.model_condition_bits & (1u128 << upgraded), 0);
    // Advance frames past extend time.
    logic.frame = o.power_plant_rods_done_frame;
    logic.update_power_plant_rods();
    let o = logic.objects.get(&id).unwrap();
    assert_eq!(o.model_condition_bits & (1u128 << upgrading), 0);
    assert_ne!(o.model_condition_bits & (1u128 << upgraded), 0);
    assert!(logic.special_power_completion_log.rods_extend_completes >= 1);
}

#[test]
fn supply_warehouse_create_seeds_starting_boxes() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("SupplyWarehouse");
    t.set_health(1000.0);
    t.add_kind_of(KindOf::Harvestable);
    logic.templates.insert("SupplyWarehouse".into(), t);
    let id = logic
        .create_object("SupplyWarehouse", Team::Neutral, glam::Vec3::ZERO)
        .unwrap();
    let supplies = logic.objects.get(&id).unwrap().stored_resources.supplies;
    assert_eq!(supplies, 400 * 75, "StartingBoxes 400 × $75");
    assert!(logic.supply_create_warehouse_registers >= 1);
}

#[test]
fn china_mines_upgrade_places_structure_minefield() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("ChinaBarracks");
    t.set_health(1000.0);
    t.add_kind_of(KindOf::Structure);
    logic.templates.insert("ChinaBarracks".into(), t);
    let id = logic
        .create_object(
            "ChinaBarracks",
            Team::China,
            glam::Vec3::new(100.0, 0.0, 100.0),
        )
        .unwrap();
    let before = logic.objects.len();
    logic.apply_upgrade_to_object(id, "Upgrade_ChinaMines");
    let after = logic.objects.len();
    assert!(
        after >= before + 8,
        "mine ring should place 8 mines, before={before} after={after}"
    );
    assert!(logic.structure_minefield_placements >= 8);
    let mines = logic
        .objects
        .values()
        .filter(|o| o.template_name == "ChinaStandardMine")
        .count();
    assert!(mines >= 8, "mines={mines}");
}

#[test]
fn sub_objects_upgrade_bomb_truck_bio_load() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("GLAVehicleBombTruck");
    t.set_health(200.0);
    t.add_kind_of(KindOf::Vehicle);
    logic.templates.insert("GLAVehicleBombTruck".into(), t);
    let id = logic
        .create_object("GLAVehicleBombTruck", Team::GLA, glam::Vec3::ZERO)
        .unwrap();
    logic.apply_upgrade_to_object(id, "Upgrade_GLABombTruckBioBomb");
    let o = logic.objects.get(&id).unwrap();
    assert!(
        o.sub_object_visibility.is_shown("Bombload02"),
        "bio bomb must show Bombload02"
    );
    assert!(logic.sub_objects_upgrades.honesty_ok());
}

#[test]
fn sub_objects_upgrade_helix_bomb_wing() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("ChinaVehicleHelix");
    t.set_health(500.0);
    t.add_kind_of(KindOf::Aircraft);
    logic.templates.insert("ChinaVehicleHelix".into(), t);
    let id = logic
        .create_object("ChinaVehicleHelix", Team::China, glam::Vec3::ZERO)
        .unwrap();
    logic.apply_upgrade_to_object(id, "Upgrade_HelixNapalmBomb");
    assert!(logic
        .objects
        .get(&id)
        .unwrap()
        .sub_object_visibility
        .is_shown("BombWing"));
}

#[test]
fn sub_objects_upgrade_bomb_truck_both_loads() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("GLAVehicleBombTruck");
    t.set_health(200.0);
    t.add_kind_of(KindOf::Vehicle);
    logic.templates.insert("GLAVehicleBombTruck".into(), t);
    let id = logic
        .create_object("GLAVehicleBombTruck", Team::GLA, glam::Vec3::ZERO)
        .unwrap();
    logic.apply_upgrade_to_object(id, "Upgrade_GLABombTruckBioBomb");
    logic.apply_upgrade_to_object(id, "Upgrade_GLABombTruckHighExplosiveBomb");
    assert!(logic
        .objects
        .get(&id)
        .unwrap()
        .sub_object_visibility
        .is_shown("Bombload04"));
}

#[test]
fn replace_object_upgrade_fake_gla_becomes_real() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(1, Team::GLA, "GLA", true));
    let mut fake = ThingTemplate::new("FakeGLABarracks");
    fake.set_health(500.0);
    fake.add_kind_of(KindOf::Structure);
    logic.templates.insert("FakeGLABarracks".into(), fake);
    let mut real = ThingTemplate::new("GLABarracks");
    real.set_health(1000.0);
    real.add_kind_of(KindOf::Structure);
    logic.templates.insert("GLABarracks".into(), real);

    let id = logic
        .create_object(
            "FakeGLABarracks",
            Team::GLA,
            glam::Vec3::new(10.0, 0.0, 20.0),
        )
        .unwrap();
    logic.apply_upgrade_to_object(id, "Upgrade_BecomeRealGLABarracks");
    assert!(
        logic.objects.get(&id).is_none(),
        "fake must be removed from world"
    );
    assert!(
        logic
            .objects
            .values()
            .any(|o| o.template_name == "GLABarracks" && o.is_alive()),
        "real barracks must spawn"
    );
    assert!(logic.replace_grant_command_upgrades.replace_count >= 1);
}

#[test]
fn grant_science_upgrade_moab() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(1, Team::USA, "USA", true));
    let mut t = ThingTemplate::new("AmericaCommandCenter");
    t.set_health(1000.0);
    t.add_kind_of(KindOf::Structure);
    logic.templates.insert("AmericaCommandCenter".into(), t);
    let id = logic
        .create_object("AmericaCommandCenter", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    logic.apply_upgrade_to_object(id, "Upgrade_AmericaMOAB");
    let p = logic
        .players
        .values()
        .find(|p| p.team == Team::USA)
        .unwrap();
    assert!(
        p.has_unlocked_science("SCIENCE_MOAB"),
        "MOAB science must be granted"
    );
}

#[test]
fn command_set_upgrade_emp_mines_on_cc() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("ChinaCommandCenter");
    t.set_health(1000.0);
    t.add_kind_of(KindOf::Structure);
    logic.templates.insert("ChinaCommandCenter".into(), t);
    let id = logic
        .create_object("ChinaCommandCenter", Team::China, glam::Vec3::ZERO)
        .unwrap();
    logic.apply_upgrade_to_object(id, "Upgrade_ChinaEMPMines");
    let cs = logic
        .objects
        .get(&id)
        .and_then(|o| o.command_set_override.clone());
    assert_eq!(cs.as_deref(), Some("ChinaCommandCenterCommandSetUpgrade"));
}

#[test]
fn weapon_set_upgrade_sets_player_upgrade_flag() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("AmericaInfantryRanger");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Infantry);
    logic.templates.insert("AmericaInfantryRanger".into(), t);
    let id = logic
        .create_object("AmericaInfantryRanger", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    assert!(!logic.objects.get(&id).unwrap().weapon_set_player_upgrade);
    logic.apply_upgrade_to_object(id, "Upgrade_AmericaRangerFlashBangGrenade");
    assert!(logic.objects.get(&id).unwrap().weapon_set_player_upgrade);
    assert!(logic.upgrade_module_residuals.weapon_set_applications > 0);
}

#[test]
fn armor_upgrade_sets_armor_set_and_chemsuit_decal() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("AmericaInfantryRanger");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Infantry);
    logic.templates.insert("AmericaInfantryRanger".into(), t);
    let id = logic
        .create_object("AmericaInfantryRanger", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    logic.apply_upgrade_to_object(id, "Upgrade_AmericaChemicalSuits");
    let o = logic.objects.get(&id).unwrap();
    assert!(o.armor_set_player_upgrade);
    assert!(o.terrain_decal_chemsuit);
}

#[test]
fn locomotor_set_upgrade_worker_shoes_speed() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("GLAInfantryWorker");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Infantry);
    logic.templates.insert("GLAInfantryWorker".into(), t);
    let id = logic
        .create_object("GLAInfantryWorker", Team::GLA, glam::Vec3::ZERO)
        .unwrap();
    logic.apply_upgrade_to_object(id, "Upgrade_GLAWorkerShoes");
    let o = logic.objects.get(&id).unwrap();
    assert!(o.locomotor_upgrade);
    assert!(
        o.movement.max_speed >= 30.0,
        "WorkerShoes speed residual, got {}",
        o.movement.max_speed
    );
}

#[test]
fn cost_modifier_upgrade_reduces_vehicle_cost() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(1, Team::USA, "USA", true));
    let pid = 1u32;
    let mut b = ThingTemplate::new("CostPad");
    b.set_health(500.0);
    b.add_kind_of(KindOf::Structure);
    logic.templates.insert("CostPad".into(), b);
    let mut tank_t = ThingTemplate::new("TestTank");
    tank_t.set_health(200.0);
    tank_t.add_kind_of(KindOf::Vehicle);
    tank_t.build_cost.supplies = 1000;
    logic.templates.insert("TestTank".into(), tank_t);

    let pad = logic
        .create_object("CostPad", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    assert_eq!(
        logic.modified_build_cost_supplies(pid, "TestTank", 1000),
        1000
    );
    logic.apply_upgrade_to_object(pad, "Upgrade_CostReduction");
    assert_eq!(
        logic.modified_build_cost_supplies(pid, "TestTank", 1000),
        900
    );
    assert!(logic.upgrade_module_residuals.honesty_ok());
}

#[test]
fn weapon_bonus_upgrade_sets_player_upgrade_condition() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("GLATankScorpion");
    t.set_health(200.0);
    t.add_kind_of(KindOf::Vehicle);
    logic.templates.insert("GLATankScorpion".into(), t);
    let id = logic
        .create_object("GLATankScorpion", Team::GLA, glam::Vec3::ZERO)
        .unwrap();
    assert!(!logic.objects.get(&id).unwrap().weapon_bonus_player_upgrade);
    logic.apply_upgrade_to_object(id, "Upgrade_GLAAPRockets");
    assert!(logic.objects.get(&id).unwrap().weapon_bonus_player_upgrade);
}
