//! Host GameLogic tests — named weapon skip residuals.
#![allow(unused_imports, non_snake_case, unused_variables, dead_code)]
use super::*;

/// Spawn `template` as Infantry authoring HumveeGun PRIMARY and let it fire at
/// a TestTank for a few frames (object_hit residual gate fixtures).
fn fire_named_template_with_humvee_gun(template: &str, team: Team) -> GameLogic {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let mut tpl = crate::game_logic::ThingTemplate::new(template);
    tpl.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic.templates.insert(template.to_string(), tpl);
    let id = game_logic
        .create_object(template, team, Vec3::new(0.0, 0.0, 0.0))
        .expect("attacker");
    let enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
        .expect("enemy");
    {
        let (t, attack_events) = game_logic.host_object_and_attack_events_mut(id).unwrap();
        t.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = t.weapon.as_mut() {
            w.last_fire_time = -20.0;
            w.reload_time = 0.1;
        }
    }
    for f in 50..60 {
        game_logic.set_current_frame(f);
        game_logic.update_combat(&[id, enemy], LOGIC_FRAME_TIMESTEP);
    }
    game_logic
}

/// hq-2qedt.182: Jarmen Kell-named template authoring a non-sniper PRIMARY
/// must not route through the Jarmen Kell sniper residual on object hit.
#[test]
fn jarmen_kell_named_non_sniper_weapon_skips_jarmen_object_hit_residual() {
    let gl = fire_named_template_with_humvee_gun("GLAInfantryJarmenKell", Team::USA);
    assert_eq!(
        gl.jarmen_kell_residual_fires(),
        0,
        "Jarmen residual chosen by template-name peel"
    );
}

/// hq-2qedt.183: MiniGunner-named template authoring a non-minigun PRIMARY
/// must not route through the MiniGunner residual on object hit.
#[test]
fn minigunner_named_non_minigun_weapon_skips_minigunner_object_hit_residual() {
    let gl = fire_named_template_with_humvee_gun("ChinaInfantryMiniGunner", Team::USA);
    assert_eq!(
        gl.minigunner_residual_ground_fires() + gl.minigunner_residual_aa_fires(),
        0,
        "MiniGunner residual chosen by template-name peel"
    );
}

/// hq-2qedt.184: Tank Hunter-named template authoring a non-missile PRIMARY
/// must not route through the Tank Hunter missile residual on object hit.
#[test]
fn tank_hunter_named_non_missile_weapon_skips_tank_hunter_object_hit_residual() {
    let gl = fire_named_template_with_humvee_gun("ChinaInfantryTankHunter", Team::USA);
    assert_eq!(
        gl.tank_hunter_residual_fires(),
        0,
        "Tank Hunter residual chosen by template-name peel"
    );
}

/// hq-2qedt.185: RPG Trooper-named template authoring a non-rocket PRIMARY
/// must not route through the RPG Trooper rocket residual on object hit.
#[test]
fn rpg_trooper_named_non_rocket_weapon_skips_rpg_object_hit_residual() {
    let gl = fire_named_template_with_humvee_gun("GLAInfantryTunnelDefender", Team::USA);
    assert_eq!(
        gl.rpg_trooper_residual_fires(),
        0,
        "RPG Trooper residual chosen by template-name peel"
    );
}

/// hq-2qedt.187: Red Guard-named template authoring a non-Redguard PRIMARY
/// must not route through the Red Guard gun/bayonet residual on object hit.
#[test]
fn red_guard_named_non_redguard_weapon_skips_red_guard_object_hit_residual() {
    let gl = fire_named_template_with_humvee_gun("ChinaInfantryRedguard", Team::USA);
    assert_eq!(
        gl.red_guard_residual_fires(),
        0,
        "Red Guard residual chosen by template-name peel"
    );
}

/// hq-2qedt.188: Colonel Burton-named template authoring a non-Burton PRIMARY
/// must not route through the Burton sniper/knife residual on object hit.
#[test]
fn burton_named_non_burton_weapon_skips_burton_object_hit_residual() {
    let gl = fire_named_template_with_humvee_gun("AmericaInfantryColonelBurton", Team::USA);
    assert_eq!(
        gl.burton_residual_sniper_fires() + gl.burton_residual_knife_kills(),
        0,
        "Burton residual chosen by template-name peel"
    );
}

/// hq-2qedt.189: Helix-named template authoring a non-minigun PRIMARY
/// must not route through the Helix minigun residual on object hit.
#[test]
fn helix_named_non_minigun_weapon_skips_helix_object_hit_residual() {
    let gl = fire_named_template_with_humvee_gun("ChinaVehicleHelix", Team::USA);
    assert_eq!(
        gl.helix_minigun_residual_fires(),
        0,
        "Helix residual chosen by template-name peel"
    );
}

/// hq-2qedt.190: Humvee-named template with TOW upgrade firing a non-TOW
/// authored SECONDARY must not route through the Humvee TOW residual.
#[test]
fn humvee_named_non_tow_secondary_skips_humvee_tow_object_hit_residual() {
    use crate::game_logic::host_upgrades::UPGRADE_AMERICA_TOW;
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let mut tpl = crate::game_logic::ThingTemplate::new("AmericaVehicleHumvee");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0)
        .set_primary_weapon_name("HumveeGun")
        .set_secondary_weapon_name("BattleMasterTankGun");
    game_logic
        .templates
        .insert("AmericaVehicleHumvee".to_string(), tpl);
    let id = game_logic
        .create_object("AmericaVehicleHumvee", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("humvee");
    let enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
        .expect("enemy");
    {
        let (h, attack_events) = game_logic.host_object_and_attack_events_mut(id).unwrap();
        h.apply_upgrade_tag(UPGRADE_AMERICA_TOW);
        h.active_weapon_slot = 1;
        h.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = h.secondary_weapon.as_mut() {
            w.last_fire_time = -100.0;
            w.reload_time = 0.1;
            w.min_range = 0.0;
        }
    }
    for f in 50..60 {
        game_logic.set_current_frame(f);
        game_logic.update_combat(&[id, enemy], LOGIC_FRAME_TIMESTEP);
    }
    assert_eq!(
        game_logic.humvee_tow_residual_fires(),
        0,
        "Humvee TOW residual chosen by template-name peel"
    );
}

/// hq-2qedt.191: Troop Crawler-named transport authoring a non-assault PRIMARY
/// must not route through the TroopCrawlerAssault DEPLOY residual on object hit.
#[test]
fn troop_crawler_named_non_assault_weapon_skips_assault_deploy_object_hit_residual() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let mut tpl = crate::game_logic::ThingTemplate::new("ChinaVehicleTroopCrawler");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("ChinaVehicleTroopCrawler".to_string(), tpl);
    let id = game_logic
        .create_object(
            "ChinaVehicleTroopCrawler",
            Team::China,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("crawler");
    let enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
        .expect("enemy");
    {
        let (c, attack_events) = game_logic.host_object_and_attack_events_mut(id).unwrap();
        c.is_troop_crawler_transport = true;
        c.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = c.weapon.as_mut() {
            w.last_fire_time = -20.0;
            w.reload_time = 0.1;
        }
    }
    for f in 50..60 {
        game_logic.set_current_frame(f);
        game_logic.update_combat(&[id, enemy], LOGIC_FRAME_TIMESTEP);
    }
    assert_eq!(
        game_logic.troop_crawler_residual_assault_deploys(),
        0,
        "Troop Crawler DEPLOY residual chosen by template-name peel"
    );
}

/// hq-2qedt.215: Raptor-named jet authoring a non-jet-missile PRIMARY must not
/// route through the Raptor jet-missile residual on object hit.
#[test]
fn raptor_named_non_jet_missile_weapon_skips_raptor_object_hit_residual() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let mut tpl = crate::game_logic::ThingTemplate::new("AmericaJetRaptor");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("AmericaJetRaptor".to_string(), tpl);
    let id = game_logic
        .create_object("AmericaJetRaptor", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("raptor");
    let enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(200.0, 0.0, 0.0))
        .expect("enemy");
    // Outside RaptorJetMissileWeapon MinRange (100) so a create-peeled jet weapon still fires.
    {
        let (c, attack_events) = game_logic.host_object_and_attack_events_mut(id).unwrap();
        c.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = c.weapon.as_mut() {
            w.last_fire_time = -20.0;
            w.reload_time = 0.1;
        }
    }
    for f in 50..60 {
        game_logic.set_current_frame(f);
        game_logic.update_combat(&[id, enemy], LOGIC_FRAME_TIMESTEP);
    }
    assert_eq!(
        game_logic.raptor_residual_fires(),
        0,
        "Raptor jet-missile residual chosen by template-name peel"
    );
}

/// hq-2qedt.216: Terrorist-named infantry authoring a non-suicide PRIMARY must
/// not route through the SuicideDynamitePack residual on object hit.
#[test]
fn terrorist_named_non_suicide_weapon_skips_terrorist_object_hit_residual() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let mut tpl = crate::game_logic::ThingTemplate::new("GLAInfantryTerrorist");
    tpl.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("GLAInfantryTerrorist".to_string(), tpl);
    let id = game_logic
        .create_object("GLAInfantryTerrorist", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("terrorist");
    // Inside any create-peeled suicide trigger range so either weapon fires.
    let enemy = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(3.0, 0.0, 0.0))
        .expect("enemy");
    {
        let (c, attack_events) = game_logic.host_object_and_attack_events_mut(id).unwrap();
        c.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = c.weapon.as_mut() {
            w.last_fire_time = -20.0;
            w.reload_time = 0.1;
        }
    }
    for f in 50..60 {
        game_logic.set_current_frame(f);
        game_logic.update_combat(&[id, enemy], LOGIC_FRAME_TIMESTEP);
    }
    assert_eq!(
        game_logic.terrorist_residual_detonations(),
        0,
        "Terrorist suicide residual chosen by template-name peel"
    );
}
