//! Behavior tests extracted from the original hero/weapons/airfields suite.
use super::*;

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
                t.stealth_upgrades = vec![crate::game_logic::UpgradeMuxTriggers {
                    triggered_by: vec!["Upgrade_GLACamoNetting".into()],
                    ..Default::default()
                }];
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
    admit_retail_production_energy_rules(&mut game_logic);
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
                .registered_values()
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
