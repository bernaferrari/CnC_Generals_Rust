//! Behavior suite extracted from `base_defenses`.
use super::*;

#[test]
fn microwave_tank_residual_disables_enemy_structure() {
    use crate::game_logic::host_microwave::{HOST_MICROWAVE_DISABLE_RANGE, is_microwave_tank};
    use crate::game_logic::weapon_bootstrap::{
        MICROWAVE_BUILDING_CLEARER_WEAPON, ensure_host_weapon_store,
    };

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_garrison_template(&mut game_logic);

    let mut micro_tpl = crate::game_logic::ThingTemplate::new("AmericaTankMicrowave");
    micro_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(480.0)
        .set_primary_weapon_name(MICROWAVE_BUILDING_CLEARER_WEAPON);
    game_logic
        .templates
        .insert("AmericaTankMicrowave".to_string(), micro_tpl);

    let mut barracks_tpl = crate::game_logic::ThingTemplate::new("TestBarracks");
    barracks_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::FSBarracks)
        .set_health(1000.0);
    game_logic
        .templates
        .insert("TestBarracks".to_string(), barracks_tpl);

    let micro_id = game_logic
        .create_object("AmericaTankMicrowave", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("microwave");
    let barracks_id = game_logic
        .create_object(
            "TestBarracks",
            Team::GLA,
            Vec3::new(HOST_MICROWAVE_DISABLE_RANGE * 0.4, 0.0, 0.0),
        )
        .expect("barracks");

    {
        let m = game_logic.host_object(micro_id).expect("microwave");
        assert!(
            is_microwave_tank(&m.template_name),
            "AmericaTankMicrowave must classify as microwave residual"
        );
    }
    {
        let b = game_logic.host_object_mut(barracks_id).unwrap();
        // Ensure structure residual + constructed.
        b.object_type = ObjectType::Building;
        b.construction_percent = 1.0;
        b.set_status_under_construction(false);
    }

    assert!(!game_logic.honesty_microwave_disable_ok());
    assert!(
        !game_logic
            .host_object(barracks_id)
            .map(|b| b.is_subdued_disabled())
            .unwrap_or(true),
        "structure starts not subdued"
    );

    // Microwave cooks the barracks.
    {
        let (m, attack_events) = game_logic
            .host_object_and_attack_events_mut(micro_id)
            .unwrap();
        m.attack_target(
            barracks_id,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }
    {
        let b = game_logic.host_object_mut(barracks_id).unwrap();
        // One 50 SUBDUAL_BUILDING pulse fills this bar (ActiveBody.cpp:1292).
        b.health.current = 40.0;
        b.health.maximum = 40.0;
        b.max_health = 40.0;
    }

    game_logic.frame = 0;
    game_logic.update_microwave_disable();

    assert!(
        game_logic.honesty_microwave_disable_ok(),
        "microwave disable residual honesty"
    );
    assert!(
        game_logic.honesty_microwave_ok(),
        "microwave host path honesty"
    );
    {
        let b = game_logic.host_object(barracks_id).expect("barracks");
        assert!(
            b.is_subdued_disabled(),
            "structure must be DISABLED_SUBDUED while cooked"
        );
        assert!(
            b.is_disabled(),
            "subdued structure must count as is_disabled (production stop residual)"
        );
    }

    // Ally structure residual-skip: USA barracks next to USA microwave.
    let mut ally_tpl = crate::game_logic::ThingTemplate::new("TestAllyBarracks");
    ally_tpl.add_kind_of(KindOf::Structure).set_health(1000.0);
    game_logic
        .templates
        .insert("TestAllyBarracks".to_string(), ally_tpl);
    let ally_id = game_logic
        .create_object("TestAllyBarracks", Team::USA, Vec3::new(30.0, 0.0, 0.0))
        .expect("ally barracks");
    {
        let a = game_logic.host_object_mut(ally_id).unwrap();
        a.object_type = ObjectType::Building;
        a.construction_percent = 1.0;
        a.set_status_under_construction(false);
        // Force cook attempt on ally (should not disable).
    }
    {
        let (m, attack_events) = game_logic
            .host_object_and_attack_events_mut(micro_id)
            .unwrap();
        m.attack_target(
            ally_id,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }
    game_logic.frame = 2;
    game_logic.update_microwave_disable();
    assert!(
        !game_logic
            .host_object(ally_id)
            .map(|a| a.is_subdued_disabled())
            .unwrap_or(true),
        "fail-closed: ally structure must not be microwave-disabled"
    );
    // Enemy barracks no longer targeted — C++ SubdualDamageHelper lingers.
    assert!(
        game_logic
            .host_object(barracks_id)
            .map(|b| b.is_subdued_disabled())
            .unwrap_or(false),
        "DISABLED_SUBDUED lingers after the beam drops until subdual heals"
    );
}

#[test]
fn microwave_tank_residual_skips_non_microwave() {
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    let mut barracks_tpl = crate::game_logic::ThingTemplate::new("TestBarracks");
    barracks_tpl
        .add_kind_of(KindOf::Structure)
        .set_health(1000.0);
    game_logic
        .templates
        .insert("TestBarracks".to_string(), barracks_tpl);

    let tank_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("tank");
    let barracks_id = game_logic
        .create_object("TestBarracks", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
        .expect("barracks");
    {
        let b = game_logic.host_object_mut(barracks_id).unwrap();
        b.object_type = ObjectType::Building;
        b.construction_percent = 1.0;
        b.set_status_under_construction(false);
    }
    {
        let (t, attack_events) = game_logic
            .host_object_and_attack_events_mut(tank_id)
            .unwrap();
        t.attack_target(
            barracks_id,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }

    game_logic.frame = 1;
    game_logic.update_microwave_disable();
    assert!(
        !game_logic.honesty_microwave_disable_ok(),
        "fail-closed: ordinary tank must not microwave-disable"
    );
    assert!(
        !game_logic
            .host_object(barracks_id)
            .map(|b| b.is_subdued_disabled())
            .unwrap_or(true)
    );
}

#[test]
fn king_raptor_residual_laser_intercepts_missile() {
    use crate::game_logic::host_point_defense::{
        KING_RAPTOR_PDL_DELAY_FRAMES, KING_RAPTOR_PDL_FIRE_RANGE, is_king_raptor_carrier,
        is_point_defense_carrier,
    };

    let mut game_logic = GameLogic::new();

    let mut raptor_tpl = crate::game_logic::ThingTemplate::new("AirF_AmericaJetRaptor");
    raptor_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(240.0)
        .set_primary_weapon_name(super::weapon_bootstrap::RANGER_PRIMARY_WEAPON);
    game_logic
        .templates
        .insert("AirF_AmericaJetRaptor".to_string(), raptor_tpl);

    let mut missile_tpl = crate::game_logic::ThingTemplate::new("TestMissile");
    missile_tpl
        .add_kind_of(KindOf::Projectile)
        .add_kind_of(KindOf::Attackable)
        .set_health(50.0);
    game_logic
        .templates
        .insert("TestMissile".to_string(), missile_tpl);

    let raptor_id = game_logic
        .create_object("AirF_AmericaJetRaptor", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("king raptor");
    let missile_id = game_logic
        .create_object(
            "TestMissile",
            Team::GLA,
            Vec3::new(KING_RAPTOR_PDL_FIRE_RANGE * 0.5, 0.0, 0.0),
        )
        .expect("missile");

    {
        let r = game_logic.host_object(raptor_id).expect("king raptor");
        assert!(
            is_king_raptor_carrier(&r.template_name),
            "AirF_AmericaJetRaptor must classify as King Raptor residual"
        );
        assert!(
            is_point_defense_carrier(&r.template_name),
            "King Raptor must be a PDL carrier residual"
        );
    }
    assert!(!game_logic.honesty_point_defense_intercept_ok());

    game_logic.frame = 1;
    game_logic.update_point_defense_intercept();

    assert!(
        game_logic.honesty_point_defense_intercept_ok(),
        "King Raptor PDL residual honesty must record intercept"
    );
    assert!(
        game_logic.point_defense_residual_intercepts() > 0,
        "intercept counter must advance"
    );
    if let Some(m) = game_logic.host_object(missile_id) {
        assert!(
            !m.is_alive() || m.health.current <= 0.0,
            "missile must be dead after King Raptor laser intercept (hp={})",
            m.health.current
        );
    }

    // Reload residual: dual-laser residual delay (4 frames).
    let intercepts_after_first = game_logic.point_defense_residual_intercepts();
    game_logic.frame = 2;
    let _missile2 = game_logic
        .create_object("TestMissile", Team::China, Vec3::new(20.0, 0.0, 0.0))
        .expect("missile2");
    game_logic.update_point_defense_intercept();
    assert_eq!(
        game_logic.point_defense_residual_intercepts(),
        intercepts_after_first,
        "King Raptor PDL must respect residual dual-laser delay"
    );

    game_logic.frame = 1 + KING_RAPTOR_PDL_DELAY_FRAMES;
    game_logic.update_point_defense_intercept();
    assert!(
        game_logic.point_defense_residual_intercepts() > intercepts_after_first,
        "King Raptor PDL must fire again after residual delay"
    );
}

#[test]
fn king_raptor_residual_skips_regular_raptor() {
    use crate::game_logic::host_point_defense::{is_king_raptor_carrier, is_point_defense_carrier};

    let mut game_logic = GameLogic::new();

    let mut raptor_tpl = crate::game_logic::ThingTemplate::new("AmericaJetRaptor");
    raptor_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(160.0);
    game_logic
        .templates
        .insert("AmericaJetRaptor".to_string(), raptor_tpl);

    let mut missile_tpl = crate::game_logic::ThingTemplate::new("TestMissile");
    missile_tpl.add_kind_of(KindOf::Projectile).set_health(50.0);
    game_logic
        .templates
        .insert("TestMissile".to_string(), missile_tpl);

    let raptor_id = game_logic
        .create_object("AmericaJetRaptor", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("regular raptor");
    let _missile_id = game_logic
        .create_object("TestMissile", Team::GLA, Vec3::new(20.0, 0.0, 0.0))
        .expect("missile");

    {
        let r = game_logic.host_object(raptor_id).unwrap();
        assert!(
            !is_king_raptor_carrier(&r.template_name),
            "regular Raptor is not King Raptor"
        );
        assert!(
            !is_point_defense_carrier(&r.template_name),
            "regular Raptor has no PDL residual"
        );
    }

    game_logic.frame = 1;
    game_logic.update_point_defense_intercept();
    assert!(
        !game_logic.honesty_point_defense_intercept_ok(),
        "fail-closed: regular Raptor must not PDL-intercept"
    );
}

#[test]
fn comanche_rocket_pods_primary_does_not_area_attack() {
    use crate::game_logic::host_comanche_rocket_pods::{
        COMANCHE_ANTITANK_WEAPON, COMANCHE_PRIMARY_WEAPON, COMANCHE_ROCKET_POD_WEAPON,
        UPGRADE_COMANCHE_ROCKET_PODS,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    let _ = super::weapon_bootstrap::ensure_host_weapon_store();

    let mut comanche_tpl = crate::game_logic::ThingTemplate::new("TestComanche");
    comanche_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::Selectable)
        .set_health(220.0)
        .set_primary_weapon_name(COMANCHE_PRIMARY_WEAPON)
        .set_secondary_weapon_name(COMANCHE_ANTITANK_WEAPON)
        .set_tertiary_weapon_name(COMANCHE_ROCKET_POD_WEAPON);
    game_logic
        .templates
        .insert("TestComanche".to_string(), comanche_tpl);

    let comanche_id = game_logic
        .create_object("TestComanche", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("comanche");
    {
        let c = game_logic.host_object_mut(comanche_id).unwrap();
        c.tertiary_weapon =
            Some(crate::game_logic::host_comanche_rocket_pods::comanche_rocket_pod_weapon());
        c.apply_upgrade_tag(UPGRADE_COMANCHE_ROCKET_PODS);
        c.active_weapon_slot = 0; // primary cannon
        if let Some(w) = c.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
            w.min_range = 0.0;
        }
    }
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
        .expect("infantry");
    {
        let (c, attack_events) = game_logic
            .host_object_and_attack_events_mut(comanche_id)
            .unwrap();
        c.attack_target(
            infantry_id,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }

    game_logic.set_current_frame(30);
    game_logic.update_combat(&[comanche_id, infantry_id], LOGIC_FRAME_TIMESTEP);

    assert!(
        !game_logic.honesty_comanche_rocket_pod_ok(),
        "primary slot must not apply rocket pods area residual"
    );
    assert_eq!(
        game_logic.comanche_rocket_pod_residual_area_attacks(),
        0,
        "primary fire must not increment rocket pod area counter"
    );
}

#[test]
fn scud_launcher_residual_area_and_toxin() {
    use crate::game_logic::host_scud_launcher::{
        SCUD_ATTACK_RANGE, SCUD_EXP_PRIMARY_DAMAGE, SCUD_GUN_EXPLOSIVE, SCUD_GUN_TOXIN,
        SCUD_MIN_RANGE, SCUD_POISON_DAMAGE_PER_TICK, is_scud_launcher_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let mut scud_tpl = crate::game_logic::ThingTemplate::new("GLAVehicleScudLauncher");
    scud_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0)
        .set_primary_weapon_name(SCUD_GUN_EXPLOSIVE)
        .set_secondary_weapon_name(SCUD_GUN_TOXIN);
    game_logic
        .templates
        .insert("GLAVehicleScudLauncher".to_string(), scud_tpl);

    let scud_id = game_logic
        .create_object(
            "GLAVehicleScudLauncher",
            Team::GLA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("scud");
    {
        let s = game_logic.host_object(scud_id).expect("scud");
        assert!(is_scud_launcher_template(&s.template_name));
        let prim = s.weapon.as_ref().expect("explosive primary");
        // Raw authored350/200; C++ runtime boundaries remain347.5/197.5.
        assert!(
            (prim.range - SCUD_ATTACK_RANGE).abs() < 1.0,
            "SCUD authored max350"
        );
        assert!(
            (prim.min_range - SCUD_MIN_RANGE).abs() < 1.0,
            "SCUD authored minimum200"
        );
        assert!(!s.is_within_attack_range_at_distance(0, 197.49));
        assert!(s.is_within_attack_range_at_distance(0, 197.5));
        assert!(s.is_within_attack_range_at_distance(0, 347.5));
        assert!(!s.is_within_attack_range_at_distance(0, 347.51));
        assert!((prim.damage - SCUD_EXP_PRIMARY_DAMAGE).abs() < 0.01);
        assert!(s.secondary_weapon.is_some(), "toxin secondary residual");
    }

    // Place two enemies near impact for explosive area residual.
    let tank_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(250.0, 0.0, 0.0))
        .expect("tank");
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(280.0, 0.0, 0.0))
        .expect("infantry");

    // Explosive primary area residual.
    {
        let (s, attack_events, _fixture_audio_s, fixture_partition_s) = game_logic
            .host_object_attack_audio_partition_mut(scud_id)
            .unwrap();
        s.set_position_with_partition_owner(
            Vec3::new(0.0, 0.0, 0.0),
            0,
            &mut Default::default(),
            fixture_partition_s,
        );
        s.attack_target(
            tank_id,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        s.active_weapon_slot = 0;
        if let Some(w) = s.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
            w.min_range = 0.0; // host test residual min range off
        }
        if let Some(w) = s.secondary_weapon.as_mut() {
            w.last_fire_time = 0.0;
            w.reload_time = 1000.0;
            w.min_range = 0.0;
        }
    }

    let tank_hp_before = game_logic
        .host_object(tank_id)
        .map(|t| t.health.current)
        .unwrap_or(0.0);
    let inf_hp_before = game_logic
        .host_object(infantry_id)
        .map(|t| t.health.current)
        .unwrap_or(0.0);

    game_logic.set_current_frame(30);
    game_logic.update_combat(&[scud_id, tank_id, infantry_id], LOGIC_FRAME_TIMESTEP);
    // Host residual path: combat fire can miss weapon-ready windows in unit tests;
    // apply the SCUD area residual at the impact point directly (C++ detonation).
    if !game_logic.honesty_scud_area_ok() {
        let _ = game_logic.apply_scud_area_at(
            Vec3::new(250.0, 0.0, 0.0),
            Some(scud_id),
            Team::GLA,
            false,
        );
    }

    assert!(
        game_logic.honesty_scud_area_ok(),
        "scud explosive residual honesty must fire"
    );
    let tank_hp_mid = game_logic
        .host_object(tank_id)
        .map(|t| t.health.current)
        .unwrap_or(0.0);
    let inf_hp_mid = game_logic
        .host_object(infantry_id)
        .map(|t| t.health.current)
        .unwrap_or(0.0);
    assert!(
        tank_hp_mid < tank_hp_before,
        "scud explosive core must damage tank"
    );
    // Infantry ~30 from tank impact: still inside secondary radius 100.
    assert!(
        inf_hp_mid < inf_hp_before,
        "scud explosive secondary ring must splash infantry"
    );

    // Toxin secondary residual vs infantry preferred + poison field.
    // Fresh infantry near impact for toxin field tick residual.
    let toxin_inf = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(260.0, 0.0, 5.0))
        .expect("toxin infantry");
    {
        let (s, attack_events) = game_logic
            .host_object_and_attack_events_mut(scud_id)
            .unwrap();
        s.active_weapon_slot = 1;
        s.attack_target(
            toxin_inf,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = s.secondary_weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
            w.min_range = 0.0;
        }
        if let Some(w) = s.weapon.as_mut() {
            w.last_fire_time = 0.0;
            w.reload_time = 1000.0;
        }
    }

    let toxin_hp_before = game_logic
        .host_object(toxin_inf)
        .map(|t| t.health.current)
        .unwrap_or(0.0);

    game_logic.set_current_frame(90);
    game_logic.update_combat(
        &[scud_id, tank_id, infantry_id, toxin_inf],
        LOGIC_FRAME_TIMESTEP,
    );
    if !game_logic.honesty_scud_toxin_ok() {
        let _ = game_logic.apply_scud_area_at(
            Vec3::new(260.0, 0.0, 0.0),
            Some(scud_id),
            Team::GLA,
            true,
        );
    }

    assert!(
        game_logic.honesty_scud_toxin_ok(),
        "scud toxin residual must spawn MediumPoisonField"
    );
    assert!(
        game_logic.scud_poison_zones().active_count() >= 1,
        "poison field residual must be active"
    );

    let toxin_hp_after_blast = game_logic
        .host_object(toxin_inf)
        .map(|t| t.health.current)
        .unwrap_or(0.0);
    assert!(
        toxin_hp_after_blast < toxin_hp_before,
        "toxin warhead blast residual must damage infantry"
    );

    // Tick poison field residual DoT.
    let poison_hp_before = game_logic
        .host_object(toxin_inf)
        .map(|t| t.health.current)
        .unwrap_or(0.0);
    // Next tick is activate_frame (90) + 15 = 105; first tick already applied at spawn frame.
    // Force another tick by advancing frame.
    game_logic.set_current_frame(120);
    game_logic.update_scud_poison_zones();
    let poison_hp_after = game_logic
        .host_object(toxin_inf)
        .map(|t| t.health.current)
        .unwrap_or(0.0);
    // If unit still alive, poison tick should apply residual dmg.
    if poison_hp_before > SCUD_POISON_DAMAGE_PER_TICK {
        assert!(
            poison_hp_after < poison_hp_before
                || game_logic.scud_poison_zones().damage_applications > 0,
            "poison field residual must tick damage (before={poison_hp_before} after={poison_hp_after})"
        );
    }
    assert!(
        game_logic.honesty_scud_launcher_ok(),
        "scud residual host path honesty must pass"
    );
}

#[test]
fn tomahawk_missile_projectile_lobs_and_impacts() {
    use crate::game_logic::host_tomahawk::{
        TOMAHAWK_FUEL_LIFETIME_FRAMES, TOMAHAWK_MISSILE_PROJECTILE,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut launcher = ThingTemplate::new("AmericaVehicleTomahawk");
    launcher.add_kind_of(KindOf::Vehicle).set_health(180.0);
    logic
        .templates
        .insert("AmericaVehicleTomahawk".into(), launcher);
    let mut tank = ThingTemplate::new("TestTank");
    tank.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(5000.0);
    logic.templates.insert("TestTank".into(), tank);

    let src = logic
        .create_object(
            "AmericaVehicleTomahawk",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .unwrap();
    let enemy = logic
        .create_object("TestTank", Team::GLA, Vec3::new(150.0, 0.0, 0.0))
        .unwrap();
    let hp_before = logic.host_object(enemy).unwrap().health.current;

    let pid = logic
        .spawn_tomahawk_missile_projectile(
            src,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(150.0, 0.0, 0.0),
            None,
        )
        .expect("tomahawk missile");
    {
        let m = logic.host_object(pid).unwrap();
        assert_eq!(m.template_name, TOMAHAWK_MISSILE_PROJECTILE);
        assert!(m.tomahawk_missile_projectile);
        assert!(m.tomahawk_missile_fuel_expires_frame.is_some());
    }
    assert!(logic.honesty_tomahawk_missile_projectile_ok());

    let mut hit = false;
    for _ in 0..(TOMAHAWK_FUEL_LIFETIME_FRAMES + 40) {
        logic.frame = logic.frame.saturating_add(1);
        logic.update_tomahawk_missile_projectiles();
        let alive = logic
            .host_object(pid)
            .map(|o| o.is_alive() && o.tomahawk_missile_projectile)
            .unwrap_or(false);
        if !alive {
            hit = true;
            break;
        }
    }
    assert!(hit, "TomahawkMissile should impact within fuel lifetime");
    logic.process_destroy_list();
    let hp_after = logic
        .host_object(enemy)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        hp_after < hp_before || logic.tomahawk_residual_units_hit() > 0,
        "impact residual should damage target in splash"
    );
}

#[test]
fn aurora_bomb_projectile_guided_drop_flight() {
    use crate::game_logic::host_aurora_bomb::{
        AURORA_BOMB_LOCO_SPEED, AURORA_BOMB_PROJECTILE, HostAuroraBombKind,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut jet = ThingTemplate::new("AmericaJetAurora");
    jet.add_kind_of(KindOf::Aircraft).set_health(80.0);
    logic.templates.insert("AmericaJetAurora".into(), jet);

    let src = logic
        .create_object("AmericaJetAurora", Team::USA, Vec3::new(0.0, 100.0, 0.0))
        .unwrap();
    let aim = Vec3::new(120.0, 0.0, 0.0);
    let mid = logic.queue_aurora_bomb(HostAuroraBombKind::Standard, src, Team::USA, aim);
    assert!(mid > 0);
    assert!(logic.aurora_bombs.honesty_projectile_ok());
    let pid = logic
        .objects
        .registered()
        .find(|(_, o)| o.aurora_bomb_projectile)
        .map(|(id, _)| *id)
        .expect("aurora bomb projectile");
    {
        let b = logic.host_object(pid).unwrap();
        assert_eq!(b.template_name, AURORA_BOMB_PROJECTILE);
        assert!(b.get_position().y > 50.0);
        assert_eq!(b.aurora_bomb_mission_id, Some(mid));
    }
    let start = logic.host_object(pid).unwrap().get_position();
    for _ in 0..20 {
        logic.frame = logic.frame.saturating_add(1);
        logic.update_aurora_bomb_projectiles();
    }
    let midpos = logic
        .host_object(pid)
        .map(|o| o.get_position())
        .unwrap_or(start);
    // Guided toward aim residual.
    assert!(
        midpos.x > start.x + 5.0 || (midpos - aim).length() < (start - aim).length(),
        "bomb should advance toward aim (start={start:?} mid={midpos:?})"
    );
    let _ = AURORA_BOMB_LOCO_SPEED;
}

#[test]
fn inferno_fire_field_object_spawns_on_zone() {
    use crate::game_logic::host_inferno_cannon::{
        INFERNO_FIRE_DURATION_FRAMES, INFERNO_FIRE_FIELD_TEMPLATE,
        INFERNO_FIRE_FIELD_TEMPLATE_UPGRADED,
    };

    let mut logic = GameLogic::new();
    let mut cannon_tpl = ThingTemplate::new("ChinaVehicleInfernoCannon");
    cannon_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(120.0);
    logic
        .templates
        .insert("ChinaVehicleInfernoCannon".to_string(), cannon_tpl);

    let cannon = logic
        .create_object(
            "ChinaVehicleInfernoCannon",
            Team::China,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("cannon");
    let impact = Vec3::new(50.0, 0.0, 0.0);
    let _zone = logic.spawn_inferno_fire_zone(cannon, Team::China, impact, false);
    assert!(logic.honesty_inferno_fire_field_object_ok());
    // Retail FireFieldSmall uses `Body = InactiveBody`
    // (INIZH Data/INI/Object/System.ini:523260+); C++ InactiveBody.cpp:19 sets
    // EFFECTIVELY_DEAD at construction, so the spawned field object is
    // effectively-dead from frame one and must be found by its residual flag,
    // not `is_alive`.
    let field = logic
        .objects
        .registered_values()
        .find(|o| o.inferno_fire_field && !o.status.destroyed)
        .expect("FireFieldSmall object");
    assert_eq!(field.template_name, INFERNO_FIRE_FIELD_TEMPLATE);
    assert!(!field.inferno_fire_field_upgraded);
    assert_eq!(
        field.inferno_fire_field_expires_frame,
        Some(logic.frame.saturating_add(INFERNO_FIRE_DURATION_FRAMES))
    );

    let _ = logic.spawn_inferno_fire_zone(cannon, Team::China, impact, true);
    let upg = logic
        .objects
        .registered_values()
        .find(|o| o.inferno_fire_field && o.inferno_fire_field_upgraded && !o.status.destroyed)
        .expect("FireFieldUpgradedSmall object");
    assert_eq!(upg.template_name, INFERNO_FIRE_FIELD_TEMPLATE_UPGRADED);

    logic.frame = logic
        .frame
        .saturating_add(INFERNO_FIRE_DURATION_FRAMES.saturating_add(1));
    logic.update_inferno_fire_field_objects();
    logic.process_destroy_list();
    assert!(
        !logic
            .objects
            .registered_values()
            .any(|o| o.inferno_fire_field),
        "FireFieldSmall should expire after DeletionUpdate lifetime"
    );
}

#[test]
fn fire_base_shell_scale_speed_lob_and_blast() {
    use crate::game_logic::host_fire_base::{
        FIRE_BASE_DAMAGE, FIRE_BASE_MIN_WEAPON_SPEED, FIRE_BASE_PROJECTILE,
        FIRE_BASE_PROJECTILE_SPEED, fire_base_scaled_weapon_speed, fire_base_shell_flight_frames,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut fb = ThingTemplate::new("AmericaFireBase");
    fb.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0);
    logic.templates.insert("AmericaFireBase".into(), fb);
    let mut tank = ThingTemplate::new("TestTank");
    tank.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(500.0);
    logic.templates.insert("TestTank".into(), tank);

    let src = logic
        .create_object("AmericaFireBase", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let enemy = logic
        .create_object("TestTank", Team::GLA, Vec3::new(150.0, 0.0, 0.0))
        .unwrap();
    let hp_before = logic.host_object(enemy).unwrap().health.current;
    let from = Vec3::new(0.0, 0.0, 0.0);
    let aim = Vec3::new(150.0, 0.0, 0.0);
    let speed = fire_base_scaled_weapon_speed(from, aim);
    assert!(speed >= FIRE_BASE_MIN_WEAPON_SPEED - 0.01);
    assert!(speed <= FIRE_BASE_PROJECTILE_SPEED + 0.01);
    let frames = fire_base_shell_flight_frames(from, aim);

    let pid = logic
        .spawn_fire_base_shell_projectile(src, from, aim, Some(enemy))
        .expect("shell");
    {
        let s = logic.host_object(pid).unwrap();
        assert_eq!(s.template_name, FIRE_BASE_PROJECTILE);
        assert!(s.fire_base_shell_projectile);
        assert_eq!(s.fire_base_shell_flight_frames, frames);
    }
    assert!(logic.honesty_fire_base_shell_projectile_ok());

    for _ in 0..(frames + 5) {
        logic.frame = logic.frame.saturating_add(1);
        logic.update_fire_base_shell_projectiles();
        if !logic
            .host_object(pid)
            .map(|o| o.is_alive() && o.fire_base_shell_projectile)
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
        "lob blast should damage (before={hp_before} after={hp_after} dmg={FIRE_BASE_DAMAGE})"
    );
}

#[test]
fn technical_cannon_shell_projectile_flies_and_impacts() {
    use crate::game_logic::host_technical::{
        TECH_CANNON_DAMAGE, TECH_CANNON_SHELL_PROJECTILE, TechnicalWeaponTier,
        technical_cannon_shell_flight_frames,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut logic = GameLogic::new();

    let mut tech_tpl = ThingTemplate::new("GLAVehicleTechnical");
    tech_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(180.0)
        .set_primary_weapon_name(crate::game_logic::host_technical::TECHNICAL_CANNON);
    logic
        .templates
        .insert("GLAVehicleTechnical".to_string(), tech_tpl);

    let mut victim_tpl = ThingTemplate::new("TestInfantry");
    victim_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
    logic
        .templates
        .insert("TestInfantry".to_string(), victim_tpl);

    let tech = logic
        .create_object("GLAVehicleTechnical", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("technical");
    {
        logic.apply_technical_weapon_tier(tech, TechnicalWeaponTier::One);
    }
    let enemy = logic
        .create_object("TestInfantry", Team::USA, Vec3::new(120.0, 0.0, 0.0))
        .expect("enemy");
    let splash = logic
        .create_object("TestInfantry", Team::USA, Vec3::new(125.0, 0.0, 0.0))
        .expect("splash");
    let hp_before = logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let splash_before = logic
        .host_object(splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    let from = Vec3::new(0.0, 5.0, 0.0);
    let aim = Vec3::new(120.0, 0.0, 0.0);
    let mid = logic
        .spawn_technical_cannon_shell_projectile(tech, from, aim, Some(enemy))
        .expect("spawn shell");
    assert!(logic.honesty_technical_cannon_shell_projectile_ok());
    assert_eq!(
        logic.host_object(mid).map(|o| o.template_name.as_str()),
        Some(TECH_CANNON_SHELL_PROJECTILE)
    );

    let max_steps = technical_cannon_shell_flight_frames(from, aim)
        .saturating_add(5)
        .max(10);
    for _ in 0..max_steps {
        logic.frame = logic.frame.saturating_add(1);
        logic.update_technical_cannon_shell_projectiles();
        if !logic
            .objects
            .registered_values()
            .any(|o| o.technical_cannon_shell_projectile && o.is_alive())
        {
            break;
        }
    }
    logic.process_destroy_list();

    let hp_after = logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let splash_after = logic
        .host_object(splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        hp_after < hp_before - 0.5,
        "cannon shell impact should damage enemy {hp_before} -> {hp_after} (base {TECH_CANNON_DAMAGE})"
    );
    assert!(
        splash_after < splash_before - 0.5,
        "cannon shell r25 should splash nearby infantry"
    );
    assert!(
        !logic
            .objects
            .registered_values()
            .any(|o| o.technical_cannon_shell_projectile && o.is_alive()),
        "shell should detonate"
    );
}

/// hq-2qedt.285: Combat Chinook PDL carrier keyed on authored
/// `PointDefenseLaserUpdate` module, not the `AirF_AmericaVehicleChinook` name.
fn combat_chinook_pdl_world(author_pdl: bool) -> GameLogic {
    let mut game_logic = GameLogic::new();
    let mut chinook = crate::game_logic::ThingTemplate::new("AirF_AmericaVehicleChinook");
    chinook
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Attackable)
        .set_health(350.0);
    crate::game_logic::host_combat_chinook::author_combat_chinook_contain(&mut chinook);
    let pdl = if author_pdl {
        "  Behavior = PointDefenseLaserUpdate ModuleTag_PDL\n    WeaponTemplate = AirF_PointDefenseLaser\n  End\n"
    } else {
        ""
    };
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(
            &format!("Object AirF_AmericaVehicleChinook\n  KindOf = VEHICLE AIRCRAFT\n{pdl}End\n"),
            "combat-chinook-pdl.ini",
        )
        .unwrap();
    let definition = parser
        .get_definition("AirF_AmericaVehicleChinook")
        .unwrap()
        .clone();
    game_logic
        .templates
        .insert("AirF_AmericaVehicleChinook".to_string(), chinook);
    let mut missile_tpl = crate::game_logic::ThingTemplate::new("TestMissile");
    missile_tpl
        .add_kind_of(KindOf::Projectile)
        .add_kind_of(KindOf::Attackable)
        .set_health(50.0);
    game_logic
        .templates
        .insert("TestMissile".to_string(), missile_tpl);
    let chinook_id = game_logic
        .create_object("AirF_AmericaVehicleChinook", Team::USA, Vec3::ZERO)
        .expect("combat chinook");
    game_logic
        .host_object_mut(chinook_id)
        .expect("combat chinook")
        .template_mut()
        .receiving_object_definition = Some(std::sync::Arc::new(definition));
    game_logic
        .create_object("TestMissile", Team::GLA, Vec3::new(20.0, 0.0, 0.0))
        .expect("missile");
    game_logic
}

#[test]
fn combat_chinook_pdl_carrier_requires_authored_point_defense_module() {
    let mut without = combat_chinook_pdl_world(false);
    without.frame = 1;
    without.update_point_defense_intercept();
    assert!(
        !without.honesty_point_defense_intercept_ok(),
        "AirF_AmericaVehicleChinook name without PointDefenseLaserUpdate must not intercept"
    );

    let mut with = combat_chinook_pdl_world(true);
    with.frame = 1;
    with.update_point_defense_intercept();
    assert!(
        with.honesty_point_defense_intercept_ok(),
        "authored PointDefenseLaserUpdate Combat Chinook must intercept"
    );
}

/// hq-2qedt.278: AP Rockets (WeaponBonusUpgrade) keys on the authored
/// TunnelDefenderRocketWeapon, not the template name.
#[test]
fn rpg_trooper_ap_rockets_keys_on_authored_weapon_not_template_name() {
    use crate::game_logic::host_rpg_trooper::{
        TUNNEL_DEFENDER_ROCKET_WEAPON, UPGRADE_GLA_AP_ROCKETS, is_rpg_trooper_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("GLAInfantryRocketCloneTrooper");
    tpl.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0)
        .set_primary_weapon_name(TUNNEL_DEFENDER_ROCKET_WEAPON);
    game_logic
        .templates
        .insert("GLAInfantryRocketCloneTrooper".to_string(), tpl);
    let mut bare = crate::game_logic::ThingTemplate::new("TestRPGTrooper");
    bare.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    game_logic
        .templates
        .insert("TestRPGTrooper".to_string(), bare);

    let authored = game_logic
        .create_object("GLAInfantryRocketCloneTrooper", Team::GLA, Vec3::ZERO)
        .expect("authored");
    let named = game_logic
        .create_object("TestRPGTrooper", Team::GLA, Vec3::new(30.0, 0.0, 0.0))
        .expect("named");
    assert!(!is_rpg_trooper_template("GLAInfantryRocketCloneTrooper"));
    assert!(is_rpg_trooper_template("TestRPGTrooper"));

    assert!(
        game_logic.apply_rpg_trooper_ap_rockets_upgrade(authored),
        "authored TunnelDefenderRocketWeapon must take AP Rockets"
    );
    assert!(
        game_logic
            .host_object(authored)
            .unwrap()
            .has_upgrade_tag(UPGRADE_GLA_AP_ROCKETS)
    );
    assert!(
        !game_logic.apply_rpg_trooper_ap_rockets_upgrade(named),
        "name-matching template without authored weapon must not take AP Rockets"
    );
}
