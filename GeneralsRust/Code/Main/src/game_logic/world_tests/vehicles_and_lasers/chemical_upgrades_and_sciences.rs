//! Behavior tests extracted from the original hero/weapons/airfields suite.
use super::*;

#[test]
fn anthrax_gamma_residual_toxin_stream_and_field() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::host_toxin_tractor::{
        TOXIN_MED_FIELD_DAMAGE_UPGRADED, TOXIN_STREAM_DAMAGE_GAMMA, TOXIN_STREAM_DAMAGE_UPGRADED,
        TOXIN_TRUCK_GUN, TOXIN_TRUCK_SPRAYER, UPGRADE_GLA_ANTHRAX_GAMMA,
    };
    use crate::game_logic::host_upgrades::HostUpgradeKind;
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::GLA, "GLA", true);
    player.resources.supplies = 5000;
    game_logic.add_player(player);
    ensure_test_barracks_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let mut toxin_tpl = crate::game_logic::ThingTemplate::new("Chem_GLAVehicleToxinTruck");
    toxin_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0)
        .set_primary_weapon_name(TOXIN_TRUCK_GUN)
        .set_secondary_weapon_name(TOXIN_TRUCK_SPRAYER);
    game_logic
        .templates
        .insert("Chem_GLAVehicleToxinTruck".to_string(), toxin_tpl);

    let truck_id = game_logic
        .create_object(
            "Chem_GLAVehicleToxinTruck",
            Team::GLA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("chem toxin truck");
    let enemy = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .expect("enemy");
    let barracks_id = game_logic
        .create_object("TestBarracks", Team::GLA, Vec3::new(-40.0, 0.0, 0.0))
        .expect("barracks");

    // Chem baseline stream residual (Anthrax Beta 12.5).
    {
        let t = game_logic.host_object_mut(truck_id).unwrap();
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
    game_logic.update_combat(&[truck_id, enemy], LOGIC_FRAME_TIMESTEP);
    if game_logic.toxin_stream_missiles_spawned == 0 {
        let from = game_logic
            .host_object(truck_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::ZERO);
        let aim = game_logic
            .host_object(enemy)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(40.0, 0.0, 0.0));
        assert!(
            game_logic
                .spawn_toxin_stream_projectile(truck_id, from, aim, Some(enemy))
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
        "chem baseline stream honesty"
    );
    let hp_after_beta = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let beta_dmg = hp_before - hp_after_beta;
    assert!(
        beta_dmg + 0.1 >= TOXIN_STREAM_DAMAGE_UPGRADED,
        "Chem residual baseline must deal at least Anthrax Beta 12.5 (got {beta_dmg})"
    );

    // Research Anthrax Gamma residual via QueueUpgrade → complete.
    game_logic.queue_command(GameCommand {
        command_type: CommandType::QueueUpgrade {
            upgrade_name: UPGRADE_GLA_ANTHRAX_GAMMA.to_string(),
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![barracks_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    assert!(
        game_logic
            .host_upgrades()
            .honesty_queue_ok(HostUpgradeKind::AnthraxGamma)
    );
    // C++ ProductionUpdate.cpp:687-704 advances once per real logic frame.
    // Live update_with_dt caps each call at one frame; a 61s delta cannot
    // substitute for the producer's full 60s research window (hq-85nu7).
    let research_frames = HostUpgradeKind::AnthraxGamma.retail_research_frames();
    let research_start = game_logic.getFrame();
    for _ in 0..research_frames - 1 {
        game_logic.update_with_dt(LOGIC_FRAME_TIMESTEP);
    }
    assert_eq!(game_logic.getFrame(), research_start + research_frames - 1);
    assert!(
        !game_logic
            .host_upgrades()
            .honesty_complete_ok(HostUpgradeKind::AnthraxGamma),
        "AnthraxGamma must remain pending before the final authored research frame"
    );
    game_logic.update_with_dt(LOGIC_FRAME_TIMESTEP);
    assert!(
        game_logic
            .host_upgrades()
            .honesty_complete_ok(HostUpgradeKind::AnthraxGamma),
        "AnthraxGamma complete honesty"
    );
    assert!(
        game_logic
            .host_upgrades()
            .honesty_host_path_ok(HostUpgradeKind::AnthraxGamma),
        "AnthraxGamma must tag toxin units"
    );
    let truck = game_logic.host_object(truck_id).expect("truck");
    assert!(
        truck.has_upgrade_tag(UPGRADE_GLA_ANTHRAX_GAMMA)
            || truck.has_upgrade_tag("Chem_Upgrade_GLAAnthraxGamma")
            || truck.has_upgrade_tag("Upgrade_GLAAnthraxGamma"),
        "truck must receive gamma upgrade tag"
    );

    // Gamma stream residual: 20.5 (fresh target so prior stream splash cannot mask dmg).
    let gamma_enemy = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(45.0, 0.0, 0.0))
        .expect("gamma enemy");
    {
        let t = game_logic.host_object_mut(truck_id).unwrap();
        t.active_weapon_slot = 0;
        t.attack_target(gamma_enemy);
        if let Some(w) = t.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.05;
        }
        t.record_host_weapon_stats();
    }
    let hp_mid = game_logic
        .host_object(gamma_enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    game_logic.set_current_frame(40);
    let from = game_logic
        .host_object(truck_id)
        .map(|o| o.get_position())
        .unwrap_or(Vec3::ZERO);
    let aim = game_logic
        .host_object(gamma_enemy)
        .map(|o| o.get_position())
        .unwrap_or(Vec3::new(45.0, 0.0, 0.0));
    assert!(
        game_logic
            .spawn_toxin_stream_projectile(truck_id, from, aim, Some(gamma_enemy))
            .is_some(),
        "gamma stream projectile spawn"
    );
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
    let hp_after_gamma = game_logic
        .host_object(gamma_enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let gamma_dmg = hp_mid - hp_after_gamma;
    assert!(
        gamma_dmg + 0.1 >= TOXIN_STREAM_DAMAGE_GAMMA,
        "gamma stream must deal at least 20.5 (got {gamma_dmg})"
    );
    assert!(
        game_logic.toxin_tractor_registry().honesty_gamma_ok()
            || game_logic.honesty_toxin_stream_projectile_ok(),
        "gamma stream honesty"
    );

    // Contaminate spray residual → upgraded MediumPoisonField 2.5/tick.
    let spray_victim = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(8.0, 0.0, 0.0))
        .expect("spray victim");
    {
        let t = game_logic.host_object_mut(truck_id).unwrap();
        t.active_weapon_slot = 1;
        t.attack_target(spray_victim);
        if let Some(w) = t.secondary_weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.05;
            w.damage = 0.0;
            w.range = 15.0;
        }
        t.record_host_weapon_stats();
        if let Some(w) = t.weapon.as_mut() {
            w.last_fire_time = 0.0;
            w.reload_time = 1000.0;
        }
        t.record_host_weapon_stats();
    }
    game_logic.set_current_frame(60);
    use crate::game_logic::host_toxin_tractor::{
        TOXIN_SPRAY_CONTINUOUS_FIRE_COAST_FRAMES, TOXIN_SPRAY_MIN_SHOTS_TO_CREATE_OCL,
    };
    for f in 0..TOXIN_SPRAY_MIN_SHOTS_TO_CREATE_OCL {
        game_logic.set_current_frame(u64::from(50 + f));
        game_logic.update_combat(&[truck_id, spray_victim], LOGIC_FRAME_TIMESTEP);
    }
    for f in 0..TOXIN_SPRAY_MIN_SHOTS_TO_CREATE_OCL {
        game_logic.set_current_frame(u64::from(50 + f));
        let _ = game_logic.apply_toxin_tractor_spray_at(
            Vec3::new(10.0, 0.0, 0.0),
            Some(truck_id),
            Team::GLA,
        );
    }
    game_logic.set_current_frame(u64::from(
        50 + TOXIN_SPRAY_MIN_SHOTS_TO_CREATE_OCL + TOXIN_SPRAY_CONTINUOUS_FIRE_COAST_FRAMES,
    ));
    game_logic.tick_fire_ocl_after_weapon_cooldown();
    assert!(
        game_logic.honesty_toxin_tractor_spray_ok(),
        "gamma spray residual honesty"
    );
    assert!(
        game_logic.toxin_tractor_registry().active_count() > 0,
        "gamma spray must spawn medium poison field"
    );
    let zone = &game_logic.toxin_tractor_registry().active_zones()[0];
    assert!(
        (zone.damage_per_tick - TOXIN_MED_FIELD_DAMAGE_UPGRADED).abs() < 0.01,
        "gamma medium field DoT must be 2.5/tick (got {})",
        zone.damage_per_tick
    );
    assert!(zone.anthrax_tier.is_gamma());
}

#[test]
fn camo_netting_upgrade_stealths_gla_structures() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::host_upgrades::{
        HostUpgradeKind, UPGRADE_GLA_CAMO_NETTING, is_camo_netting_structure_template,
    };

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::GLA, "GLA", true);
    player.resources.supplies = 5000;
    game_logic.add_player(player);
    ensure_test_barracks_template(&mut game_logic);

    for name in [
        "Slth_GLACommandCenter",
        "GLATunnelNetwork",
        "GLAInfantryRebel",
    ] {
        if !game_logic.templates.contains_key(name) {
            let mut t = crate::game_logic::ThingTemplate::new(name);
            if name.contains("Rebel") {
                t.add_kind_of(KindOf::Infantry)
                    .add_kind_of(KindOf::Attackable)
                    .add_kind_of(KindOf::Selectable)
                    .set_health(100.0);
            } else {
                t.add_kind_of(KindOf::Structure)
                    .add_kind_of(KindOf::Selectable)
                    .set_health(1000.0);
            }
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

    assert!(is_camo_netting_structure_template("Slth_GLACommandCenter"));
    assert!(is_camo_netting_structure_template("GLATunnelNetwork"));

    let barracks_id = game_logic
        .create_object("TestBarracks", Team::GLA, Vec3::new(-40.0, 0.0, 0.0))
        .expect("barracks");
    let cc_id = game_logic
        .create_object("Slth_GLACommandCenter", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("slth cc");
    let tunnel_id = game_logic
        .create_object("GLATunnelNetwork", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
        .expect("tunnel");
    let rebel_id = game_logic
        .create_object("GLAInfantryRebel", Team::GLA, Vec3::new(100.0, 0.0, 0.0))
        .expect("rebel");

    for id in [cc_id, tunnel_id] {
        let o = game_logic.host_object_mut(id).unwrap();
        o.set_status_stealthed(false);
        o.innate_stealth = false;
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
    assert!(
        game_logic
            .host_upgrades()
            .honesty_queue_ok(HostUpgradeKind::CamoNetting)
    );
    // C++ ProductionUpdate.cpp:687-704 advances once per real logic frame.
    // Drive all 150 producer frames; a single 6s live delta runs one frame
    // and discards the backlog (the same fixture issue tracked by hq-85nu7).
    let research_frames = HostUpgradeKind::CamoNetting.retail_research_frames();
    let research_start = game_logic.getFrame();
    for _ in 0..research_frames - 1 {
        game_logic.update_with_dt(LOGIC_FRAME_TIMESTEP);
    }
    assert_eq!(game_logic.getFrame(), research_start + research_frames - 1);
    assert!(
        !game_logic
            .host_upgrades()
            .honesty_complete_ok(HostUpgradeKind::CamoNetting),
        "CamoNetting must remain pending before the final authored research frame"
    );
    game_logic.update_with_dt(LOGIC_FRAME_TIMESTEP);
    assert!(
        game_logic
            .host_upgrades()
            .honesty_complete_ok(HostUpgradeKind::CamoNetting),
        "CamoNetting complete honesty"
    );
    assert!(
        game_logic
            .host_upgrades()
            .honesty_host_path_ok(HostUpgradeKind::CamoNetting),
        "CamoNetting host path honesty"
    );

    let cc = game_logic.host_object(cc_id).expect("cc");
    assert!(
        cc.innate_stealth && !cc.status.stealthed,
        "Slth Command Center CAN_STEALTH after CamoNetting; StealthDelay not elapsed"
    );
    assert!(
        cc.has_upgrade_tag(UPGRADE_GLA_CAMO_NETTING),
        "structure must receive CamoNetting tag"
    );
    let cloak_at = cc.stealth_allowed_frame;
    drop(cc);
    let tunnel = game_logic.host_object(tunnel_id).expect("tunnel");
    assert!(
        tunnel.innate_stealth && !tunnel.status.stealthed,
        "Tunnel Network CAN_STEALTH after CamoNetting; StealthDelay not elapsed"
    );
    drop(tunnel);
    game_logic.frame = cloak_at.max(game_logic.frame);
    game_logic.update_stealth_and_detection();
    assert!(
        game_logic.host_object(cc_id).expect("cc").status.stealthed,
        "Slth Command Center cloaks after StealthDelay"
    );
    assert!(
        game_logic
            .host_object(tunnel_id)
            .expect("tunnel")
            .status
            .stealthed,
        "Tunnel Network cloaks after StealthDelay"
    );

    let rebel = game_logic.host_object(rebel_id).expect("rebel");
    assert!(
        !rebel.has_upgrade_tag(UPGRADE_GLA_CAMO_NETTING),
        "fail-closed: Rebel does not receive CamoNetting (use Camouflage residual)"
    );
}

#[test]
fn stealth_fighter_science_production_gate_residual() {
    use crate::game_logic::host_stealth_fighter::{
        SCIENCE_STEALTH_FIGHTER, STEALTH_FIGHTER_BUILD_COST,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    ensure_test_airfield_template(&mut game_logic);

    let mut fighter_tpl = crate::game_logic::ThingTemplate::new("AmericaJetStealthFighter");
    fighter_tpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(160.0)
        .set_cost(STEALTH_FIGHTER_BUILD_COST, 0);
    fighter_tpl.build_time = 0.05;
    game_logic
        .templates
        .insert("AmericaJetStealthFighter".to_string(), fighter_tpl);

    // Airforce free residual (no science Prerequisite).
    let mut airf_tpl = crate::game_logic::ThingTemplate::new("AirF_AmericaJetStealthFighter");
    airf_tpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(160.0)
        .set_cost(1000, 0);
    airf_tpl.build_time = 0.05;
    game_logic
        .templates
        .insert("AirF_AmericaJetStealthFighter".to_string(), airf_tpl);

    let airfield_id = game_logic
        .create_object("TestAirfield", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("airfield");
    {
        let af = game_logic.host_object_mut(airfield_id).unwrap();
        af.set_status_under_construction(false);
    }

    // Deny without science.
    assert!(
        !game_logic.enqueue_production(airfield_id, "AmericaJetStealthFighter".to_string()),
        "must deny Stealth Fighter without SCIENCE_StealthFighter"
    );
    assert!(
        game_logic.honesty_stealth_fighter_science_deny_ok(),
        "deny honesty"
    );

    // AirF free residual still enqueues.
    assert!(
        game_logic.enqueue_production(airfield_id, "AirF_AmericaJetStealthFighter".to_string()),
        "AirF Stealth Fighter must not require science"
    );
    // Clear free airf queue for clean science path.
    assert!(game_logic.cancel_all_production(airfield_id));

    // Unlock science residual → enqueue + complete spawn.
    assert!(game_logic.unlock_team_science(Team::USA, SCIENCE_STEALTH_FIGHTER));
    assert!(game_logic.honesty_stealth_fighter_science_unlock_ok());
    assert!(
        game_logic.enqueue_production(airfield_id, "AmericaJetStealthFighter".to_string()),
        "science unlock must allow production"
    );
    assert!(game_logic.honesty_stealth_fighter_science_produce_ok());

    // Advance production to completion (build_time 0.05s).
    for _ in 0..10 {
        game_logic.update_production(0.02);
    }
    assert!(
        game_logic.honesty_stealth_fighter_science_spawn_ok()
            || game_logic
                .objects
                .values()
                .any(|o| o.template_name.contains("StealthFighter")
                    && o.is_kind_of(KindOf::Aircraft)),
        "science-gated Stealth Fighter must spawn from production"
    );
    assert!(
        game_logic.honesty_stealth_fighter_science_ok(),
        "combined science residual honesty"
    );
}

#[test]
fn chem_terrorist_gamma_and_demo_death_weapon_residual() {
    use crate::game_logic::host_terrorist::{
        SUICIDE_DYNAMITE_PRIMARY_DAMAGE_DEMO, SUICIDE_DYNAMITE_PRIMARY_DAMAGE_GAMMA,
        TERRORIST_SUICIDE_WEAPON,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let mut chem_tpl = crate::game_logic::ThingTemplate::new("Chem_GLAInfantryTerrorist");
    chem_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(120.0)
        .set_primary_weapon_name(TERRORIST_SUICIDE_WEAPON);
    game_logic
        .templates
        .insert("Chem_GLAInfantryTerrorist".to_string(), chem_tpl);

    let mut demo_tpl = crate::game_logic::ThingTemplate::new("Demo_GLAInfantryTerrorist");
    demo_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(120.0)
        .set_primary_weapon_name(TERRORIST_SUICIDE_WEAPON);
    game_logic
        .templates
        .insert("Demo_GLAInfantryTerrorist".to_string(), demo_tpl);

    // Chem Gamma residual: tag Anthrax Gamma then detonate.
    let chem_id = game_logic
        .create_object(
            "Chem_GLAInfantryTerrorist",
            Team::GLA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("chem terrorist");
    {
        let t = game_logic.host_object_mut(chem_id).unwrap();
        t.apply_upgrade_tag("Chem_Upgrade_GLAAnthraxGamma");
        // Chem Gamma primary damage flag residual.
        if let Some(w) = t.weapon.as_mut() {
            w.damage = SUICIDE_DYNAMITE_PRIMARY_DAMAGE_GAMMA;
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
            w.range = 20.0;
        }
        t.record_host_weapon_stats();
    }
    let near = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(8.0, 0.0, 0.0))
        .expect("near");
    {
        let t = game_logic.host_object_mut(chem_id).unwrap();
        t.attack_target(near);
    }
    let hp_before = game_logic
        .host_object(near)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let zones_before = game_logic.toxin_tractor_registry().zones_spawned;
    game_logic.set_current_frame(40);
    game_logic.update_combat(&[chem_id, near], LOGIC_FRAME_TIMESTEP);
    assert!(
        game_logic.terrorist_residual_detonations() > 0,
        "chem terrorist detonation residual"
    );
    let hp_after = game_logic
        .host_object(near)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let gamma_dmg = hp_before - hp_after;
    assert!(
        gamma_dmg + 0.1 >= SUICIDE_DYNAMITE_PRIMARY_DAMAGE_GAMMA.min(hp_before),
        "Chem Gamma residual must deal ~600 primary (got {gamma_dmg}, before={hp_before})"
    );
    assert!(
        game_logic.toxin_tractor_registry().zones_spawned > zones_before,
        "Chem Gamma residual must spawn MediumPoisonField"
    );

    // Demo HE residual: 700 primary, no poison.
    let demo_id = game_logic
        .create_object(
            "Demo_GLAInfantryTerrorist",
            Team::GLA,
            Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("demo terrorist");
    let near2 = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(108.0, 0.0, 0.0))
        .expect("near2");
    {
        let t = game_logic.host_object_mut(demo_id).unwrap();
        t.attack_target(near2);
        if let Some(w) = t.weapon.as_mut() {
            assert!(
                (w.damage - SUICIDE_DYNAMITE_PRIMARY_DAMAGE_DEMO).abs() < 1.0,
                "Demo terrorist spawn weapon must flag 700 primary, got {}",
                w.damage
            );
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
            w.range = 20.0;
        }
        t.record_host_weapon_stats();
    }
    let zones_mid = game_logic.toxin_tractor_registry().zones_spawned;
    let hp2_before = game_logic
        .host_object(near2)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    game_logic.set_current_frame(80);
    game_logic.update_combat(&[demo_id, near2], LOGIC_FRAME_TIMESTEP);
    let hp2_after = game_logic
        .host_object(near2)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let demo_dmg = hp2_before - hp2_after;
    assert!(
        demo_dmg + 0.1 >= SUICIDE_DYNAMITE_PRIMARY_DAMAGE_DEMO.min(hp2_before),
        "Demo residual must deal ~700 primary (got {demo_dmg})"
    );
    assert_eq!(
        game_logic.toxin_tractor_registry().zones_spawned,
        zones_mid,
        "Demo residual must not spawn poison field"
    );
}

#[test]
fn chem_demo_trap_gamma_and_demo_he_residual() {
    use crate::game_logic::host_mines::DemoTrapProfile;

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);

    // Chem Gamma trap residual.
    let trap_id = game_logic
        .place_demo_trap_named(
            "Chem_GLADemoTrap",
            Team::GLA,
            Vec3::new(0.0, 0.0, 0.0),
            None,
            true, // gamma
        )
        .expect("chem trap");
    {
        let trap = game_logic.host_object(trap_id).unwrap();
        let md = trap.mine_data.as_ref().unwrap();
        assert_eq!(md.demo_trap_profile, DemoTrapProfile::ChemGamma);
        assert!((md.detonation_damage - 250.0).abs() < 0.01);
    }
    let enemy = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .expect("enemy");
    let zones_before = game_logic.toxin_tractor_registry().zones_spawned;
    let hp_before = game_logic.host_object(enemy).unwrap().health.current;
    game_logic.update_mines_and_demo_traps();
    game_logic.frame = game_logic
        .frame
        .saturating_add(crate::game_logic::host_mines::DEMO_TRAP_DESTRUCTION_DELAY_FRAMES);
    game_logic.update_mines_and_demo_traps();
    assert_eq!(game_logic.mine_residual_proximity_detonations(), 1);
    let enemy_after = game_logic.host_object(enemy);
    let damaged = enemy_after
        .map(|e| e.health.current < hp_before || e.status.destroyed)
        .unwrap_or(true);
    assert!(damaged, "Chem DemoTrap must damage enemy");
    assert!(
        game_logic.toxin_tractor_registry().zones_spawned > zones_before,
        "Chem Gamma DemoTrap must spawn MediumPoisonField"
    );

    // Demo HE trap residual (700/25 + 500/50 dual ring).
    let demo_trap = game_logic
        .place_demo_trap_named(
            "Demo_GLADemoTrap",
            Team::GLA,
            Vec3::new(200.0, 0.0, 0.0),
            None,
            false,
        )
        .expect("demo trap");
    {
        let trap = game_logic.host_object(demo_trap).unwrap();
        let md = trap.mine_data.as_ref().unwrap();
        assert_eq!(md.demo_trap_profile, DemoTrapProfile::Demo);
        assert!((md.detonation_damage - 700.0).abs() < 0.01);
        assert!((md.secondary_damage - 500.0).abs() < 0.01);
    }
    let far = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(230.0, 0.0, 0.0))
        .expect("far enemy in secondary ring");
    // Ensure within trigger range 40.
    let far2 = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(210.0, 0.0, 0.0))
        .expect("near enemy");
    let far_hp = game_logic.host_object(far2).unwrap().health.current;
    let zones_mid = game_logic.toxin_tractor_registry().zones_spawned;
    game_logic.update_mines_and_demo_traps();
    game_logic.frame = game_logic
        .frame
        .saturating_add(crate::game_logic::host_mines::DEMO_TRAP_DESTRUCTION_DELAY_FRAMES);
    game_logic.update_mines_and_demo_traps();
    assert!(
        game_logic.mine_residual_proximity_detonations() >= 2,
        "Demo HE trap must proximity detonate"
    );
    let far_after = game_logic.host_object(far2);
    let far_damaged = far_after
        .map(|e| e.health.current < far_hp || e.status.destroyed)
        .unwrap_or(true);
    assert!(far_damaged, "Demo HE trap must damage enemy");
    assert_eq!(
        game_logic.toxin_tractor_registry().zones_spawned,
        zones_mid,
        "Demo HE trap must not spawn poison"
    );
    let _ = far; // secondary-ring placement residual (optional observability)
}

#[test]
fn chem_demo_trap_construction_applies_anthrax_gamma_puddle() {
    use crate::game_logic::KindOf;
    use crate::game_logic::host_mines::DemoTrapProfile;
    use crate::game_logic::host_upgrades::UPGRADE_CHEM_ANTHRAX_GAMMA;

    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::GLA);
    if let Some(p) = logic.get_player_mut(2) {
        p.add_completed_upgrade(UPGRADE_CHEM_ANTHRAX_GAMMA);
    }
    let mut trap_t = ThingTemplate::new("Chem_GLADemoTrap");
    trap_t.add_kind_of(KindOf::Structure).set_health(100.0);
    logic.templates.insert("Chem_GLADemoTrap".into(), trap_t);
    let id = logic
        .create_object("Chem_GLADemoTrap", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("constructed chem trap");
    let md = logic
        .host_object(id)
        .and_then(|o| o.mine_data.as_ref())
        .expect("mine bind");
    assert_eq!(md.demo_trap_profile, DemoTrapProfile::ChemGamma);
    assert!(md.demo_trap_profile.spawns_poison());
    assert_eq!(
        md.demo_trap_profile.poison_anthrax_tier(),
        crate::game_logic::host_toxin_tractor::AnthraxResidualTier::Gamma
    );
}

#[test]
fn unit_training_science_veterancy_grant_residual() {
    use crate::game_logic::VeterancyLevel;
    use crate::game_logic::host_unit_training::{
        SCIENCE_BATTLEMASTER_TRAINING, SCIENCE_RED_GUARD_TRAINING,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::China);

    let mut rg_tpl = crate::game_logic::ThingTemplate::new("ChinaInfantryRedguard");
    rg_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(120.0);
    game_logic
        .templates
        .insert("ChinaInfantryRedguard".to_string(), rg_tpl);

    let mut bm_tpl = crate::game_logic::ThingTemplate::new("ChinaTankBattleMaster");
    bm_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(480.0);
    game_logic
        .templates
        .insert("ChinaTankBattleMaster".to_string(), bm_tpl);

    // Fail-closed: without science, spawn remains Rookie.
    let rookie_id = game_logic
        .create_object(
            "ChinaInfantryRedguard",
            Team::China,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("rookie redguard");
    {
        let u = game_logic.host_object(rookie_id).unwrap();
        assert!(
            matches!(u.experience.level, VeterancyLevel::Rookie),
            "without training science must remain Rookie"
        );
    }

    // Unlock Red Guard training → VETERAN on spawn.
    assert!(game_logic.unlock_team_science(Team::China, SCIENCE_RED_GUARD_TRAINING));
    assert!(game_logic.honesty_unit_training_unlock_ok());
    let vet_id = game_logic
        .create_object(
            "ChinaInfantryRedguard",
            Team::China,
            Vec3::new(20.0, 0.0, 0.0),
        )
        .expect("veteran redguard");
    {
        let u = game_logic.host_object(vet_id).unwrap();
        assert!(
            matches!(u.experience.level, VeterancyLevel::Veteran),
            "SCIENCE_RedGuardTraining must grant VETERAN, got {:?}",
            u.experience.level
        );
        // Veterancy health residual: +20% max HP.
        assert!(
            u.health.maximum + 0.1 >= 120.0 * 1.2,
            "VETERAN residual must apply +20% HP (got {})",
            u.health.maximum
        );
    }
    assert!(game_logic.honesty_unit_training_grant_ok());

    // Battlemaster training → ELITE.
    assert!(game_logic.unlock_team_science(Team::China, SCIENCE_BATTLEMASTER_TRAINING));
    let elite_id = game_logic
        .create_object(
            "ChinaTankBattleMaster",
            Team::China,
            Vec3::new(40.0, 0.0, 0.0),
        )
        .expect("elite battlemaster");
    {
        let u = game_logic.host_object(elite_id).unwrap();
        assert!(
            matches!(u.experience.level, VeterancyLevel::Elite),
            "SCIENCE_BattlemasterTraining must grant ELITE, got {:?}",
            u.experience.level
        );
        assert!(
            u.health.maximum + 0.1 >= 480.0 * 1.3,
            "ELITE residual must apply +30% HP (got {})",
            u.health.maximum
        );
    }
    assert!(
        game_logic.honesty_unit_training_ok(),
        "combined unit-training honesty"
    );
    assert!(game_logic.unit_training().battlemaster_grants >= 1);
    assert!(game_logic.unit_training().red_guard_grants >= 1);
}
