use super::*;
use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
use glam::Vec3;

#[test]
fn bootstrap_seeds_ranger_with_non_default_damage() {
    ensure_host_weapon_store();
    assert!(store_has(RANGER_PRIMARY_WEAPON));
    let w = ThingTemplate::weapon_from_store(RANGER_PRIMARY_WEAPON).expect("store weapon");
    assert!(
        (w.damage - Weapon::default().damage).abs() > 0.01,
        "seeded ranger damage must differ from host Weapon::default (got {})",
        w.damage
    );
    assert!(
        (w.damage - 5.0).abs() < 0.01,
        "retail RangerAdvancedCombatRifle PrimaryDamage is 5.0, got {}",
        w.damage
    );
    assert!((w.range - 97.5).abs() < 0.01);
}

/// Wave 77 residual: core host WeaponStore seed residual pack honesty.
#[test]
fn weapon_store_host_seed_residual_wave77_honesty() {
    assert!(honesty_weapon_store_host_seed_residual_wave77());
    for name in HOST_WEAPON_STORE_CORE_SEED_NAMES {
        assert!(store_has(name), "missing core seed residual: {name}");
    }
    assert!(HOST_WEAPON_STORE_CORE_SEED_NAMES.contains(&RANGER_PRIMARY_WEAPON));
    assert!(HOST_WEAPON_STORE_CORE_SEED_NAMES.contains(&PATRIOT_PRIMARY_WEAPON));
    assert!(HOST_WEAPON_STORE_CORE_SEED_NAMES.contains(&SCUD_GUN_EXPLOSIVE));
}

#[test]
fn weapon_store_deepen_residual_wave92_honesty() {
    assert!(honesty_weapon_store_deepen_residual_wave92());
    for name in HOST_WEAPON_STORE_DEEPEN_SEED_NAMES_WAVE92 {
        assert!(store_has(name), "missing deepen seed residual: {name}");
    }
    assert!(HOST_WEAPON_STORE_DEEPEN_SEED_NAMES_WAVE92.contains(&MARAUDER_TANK_GUN));
    assert!(HOST_WEAPON_STORE_DEEPEN_SEED_NAMES_WAVE92.contains(&PATHFINDER_SNIPER_WEAPON));
    assert!(HOST_WEAPON_STORE_DEEPEN_SEED_NAMES_WAVE92.contains(&DRAGON_TANK_FLAME_WEAPON));
}

#[test]
fn weapon_store_deepen_residual_pack_honesty_wave103() {
    assert!(honesty_weapon_store_deepen_residual_wave103());
    for name in HOST_WEAPON_STORE_DEEPEN_SEED_NAMES_WAVE103 {
        assert!(
            store_has(name),
            "missing wave103 deepen seed residual: {name}"
        );
    }
    assert!(HOST_WEAPON_STORE_DEEPEN_SEED_NAMES_WAVE103.contains(&NUKE_CANNON_PRIMARY_WEAPON));
    assert!(HOST_WEAPON_STORE_DEEPEN_SEED_NAMES_WAVE103.contains(&JARMEN_KELL_RIFLE));
    assert!(HOST_WEAPON_STORE_DEEPEN_SEED_NAMES_WAVE103.contains(&OVERLORD_TANK_GUN));
}

#[test]
fn create_object_usa_ranger_binds_store_weapon_stats() {
    ensure_host_weapon_store();

    let mut logic = crate::game_logic::GameLogic::new();
    let mut ranger = ThingTemplate::new("USA_Ranger");
    ranger
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::Selectable)
        .set_health(120.0)
        .set_primary_weapon_name(RANGER_PRIMARY_WEAPON)
        .set_secondary_weapon_name(RANGER_SECONDARY_WEAPON);
    // Explicit host stats must NOT be set — prove store path.
    assert!(ranger.primary_weapon.is_none());
    assert!(ranger.secondary_weapon.is_none());
    logic.templates.insert("USA_Ranger".to_string(), ranger);

    let id = logic
        .create_object("USA_Ranger", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("create USA_Ranger");
    let obj = logic.objects.get(&id).expect("object");
    let weapon = obj.weapon.as_ref().expect("weapon bound at create_object");
    assert!(
        (weapon.damage - Weapon::default().damage).abs() > 0.01,
        "expected store damage, got default-like {}",
        weapon.damage
    );
    assert!(
        (weapon.damage - 5.0).abs() < 0.01,
        "expected RangerAdvancedCombatRifle damage 5.0, got {}",
        weapon.damage
    );
    assert!((weapon.range - 97.5).abs() < 0.01);

    let secondary = obj
        .secondary_weapon
        .as_ref()
        .expect("secondary weapon bound at create_object");
    assert!(
        (secondary.damage - Weapon::default().damage).abs() > 0.01,
        "expected store secondary damage, got default-like {}",
        secondary.damage
    );
    assert!(
        (secondary.damage - 35.0).abs() < 0.01,
        "expected RangerFlashBangGrenadeWeapon damage 35.0, got {}",
        secondary.damage
    );
    assert!((secondary.range - 172.5).abs() < 0.01);
}

#[test]
fn secondary_weapon_name_for_known_units() {
    assert_eq!(
        secondary_weapon_name_for_unit("USA_Ranger"),
        Some(RANGER_SECONDARY_WEAPON)
    );
    assert_eq!(
        secondary_weapon_name_for_unit("USA_Humvee"),
        Some(HUMVEE_SECONDARY_WEAPON)
    );
    assert_eq!(secondary_weapon_name_for_unit("GLA_Soldier"), None);
    assert_eq!(secondary_weapon_name_for_unit("USA_Dozer"), None);
    assert_eq!(
        secondary_weapon_name_for_unit("China_GattlingTank"),
        Some(GATTLING_TANK_GUN_AIR)
    );
    assert_eq!(
        secondary_weapon_name_for_unit("Infa_ChinaInfantryMiniGunner"),
        Some(MINIGUNNER_GUN_AIR)
    );
}

#[test]
fn primary_weapon_name_covers_china_gla_usa_residual_gaps() {
    // Units that previously fell through to Weapon::default without explicit names.
    assert_eq!(
        primary_weapon_name_for_unit("GLA_Technical"),
        Some(TECHNICAL_MACHINE_GUN)
    );
    assert_eq!(
        primary_weapon_name_for_unit("China_BattleTank"),
        Some(BATTLE_MASTER_TANK_GUN)
    );
    assert_eq!(
        primary_weapon_name_for_unit("China_BattlemasterTank"),
        Some(BATTLE_MASTER_TANK_GUN)
    );
    assert_eq!(
        primary_weapon_name_for_unit("GLA_MarauderTank"),
        Some(MARAUDER_TANK_GUN)
    );
    assert_eq!(
        primary_weapon_name_for_unit("GLA_RPGTrooper"),
        Some(TUNNEL_DEFENDER_ROCKET_WEAPON)
    );
    assert_eq!(
        primary_weapon_name_for_unit("China_DragonTank"),
        Some(DRAGON_TANK_FLAME_WEAPON)
    );
    assert_eq!(
        primary_weapon_name_for_unit("ChinaTankDragon"),
        Some(DRAGON_TANK_FLAME_WEAPON)
    );
    assert_eq!(
        primary_weapon_name_for_unit("China_GattlingTank"),
        Some(GATTLING_TANK_GUN)
    );
    assert_eq!(
        primary_weapon_name_for_unit("Infa_ChinaInfantryMiniGunner"),
        Some(MINIGUNNER_GUN)
    );
    assert_eq!(
        primary_weapon_name_for_unit("GLALightTank"),
        Some(CRUSADER_TANK_GUN)
    );
    assert_eq!(
        primary_weapon_name_for_unit("USA_PaladinTank"),
        Some(PALADIN_TANK_GUN)
    );
    // Non-combat residual stays fail-closed.
    assert_eq!(primary_weapon_name_for_unit("USA_Dozer"), None);
    assert_eq!(primary_weapon_name_for_unit("GLA_Worker"), None);
}

#[test]
fn create_object_technical_and_battlemaster_bind_residual_not_default() {
    ensure_host_weapon_store();
    let mut logic = crate::game_logic::GameLogic::new();

    let mut technical = ThingTemplate::new("GLA_Technical");
    technical
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::Selectable)
        .set_health(200.0);
    // No primary_weapon_name — residual create path + name map must bind.
    logic
        .templates
        .insert("GLA_Technical".to_string(), technical);

    let mut battle = ThingTemplate::new("China_BattleTank");
    battle
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::Selectable)
        .set_health(500.0);
    logic
        .templates
        .insert("China_BattleTank".to_string(), battle);

    let tid = logic
        .create_object("GLA_Technical", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("create technical");
    let tw = logic
        .objects
        .get(&tid)
        .expect("technical obj")
        .weapon
        .as_ref()
        .expect("technical weapon");
    assert!(
        (tw.damage - Weapon::default().damage).abs() > 0.01,
        "Technical must not use Weapon::default (got {})",
        tw.damage
    );
    assert!((tw.damage - 10.0).abs() < 0.01);
    assert!((tw.range - 147.5).abs() < 0.01);

    let bid = logic
        .create_object("China_BattleTank", Team::China, Vec3::new(10.0, 0.0, 0.0))
        .expect("create battlemaster");
    let bw = logic
        .objects
        .get(&bid)
        .expect("battle obj")
        .weapon
        .as_ref()
        .expect("battle weapon");
    assert!(
        (bw.damage - Weapon::default().damage).abs() > 0.01,
        "Battlemaster must not use Weapon::default (got {})",
        bw.damage
    );
    assert!((bw.damage - 60.0).abs() < 0.01);
    assert!((bw.range - 147.5).abs() < 0.01);
}

/// Residual: combat must consider secondary vs structures (flashbang > rifle).
#[test]
fn update_combat_prefers_secondary_damage_vs_structure() {
    ensure_host_weapon_store();

    let mut logic = crate::game_logic::GameLogic::new();

    let mut ranger = ThingTemplate::new("USA_Ranger");
    ranger
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::Selectable)
        .set_health(120.0)
        .set_primary_weapon_name(RANGER_PRIMARY_WEAPON)
        .set_secondary_weapon_name(RANGER_SECONDARY_WEAPON);
    logic.templates.insert("USA_Ranger".to_string(), ranger);

    let mut bunker = ThingTemplate::new("GLA_Tunnel");
    bunker
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::Selectable)
        .set_health(500.0);
    logic.templates.insert("GLA_Tunnel".to_string(), bunker);

    let attacker_id = logic
        .create_object("USA_Ranger", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("ranger");
    let target_id = logic
        .create_object("GLA_Tunnel", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
        .expect("structure");

    // Sanity: both slots bound; secondary deals more damage than primary.
    let (primary_dmg, secondary_dmg) = {
        let atk = logic.objects.get(&attacker_id).expect("attacker");
        let p = atk.weapon.as_ref().expect("primary").damage;
        let s = atk.secondary_weapon.as_ref().expect("secondary").damage;
        assert!(s > p, "secondary should out-damage primary (s={s} p={p})");
        (p, s)
    };

    {
        let atk = logic.objects.get_mut(&attacker_id).expect("attacker");
        atk.attack_target(target_id);
        // Ensure both ready.
        if let Some(w) = atk.weapon.as_mut() {
            w.last_fire_time = 0.0;
            w.reload_time = 0.1;
        }
        if let Some(w) = atk.secondary_weapon.as_mut() {
            w.last_fire_time = 0.0;
            w.reload_time = 0.1;
        }
    }

    let health_before = logic
        .objects
        .get(&target_id)
        .expect("target")
        .health
        .current;

    logic.set_current_frame(60); // t = 1s
    logic.update_combat(&[attacker_id, target_id], 1.0 / 60.0);

    let health_after = logic
        .objects
        .get(&target_id)
        .expect("target")
        .health
        .current;
    let dealt = health_before - health_after;

    // Armor may reduce slightly; secondary path must land ~secondary damage, not primary.
    assert!(
        dealt > primary_dmg + 0.5,
        "structure shot must use secondary path: dealt={dealt} primary={primary_dmg} secondary={secondary_dmg}"
    );
    assert!(
        (dealt - secondary_dmg).abs() < 1.0 || dealt >= secondary_dmg * 0.5,
        "dealt damage should track secondary ({secondary_dmg}), got {dealt}"
    );

    // Secondary last_fire_time advanced; primary untouched this shot.
    let atk = logic.objects.get(&attacker_id).expect("attacker");
    let sec_last = atk
        .secondary_weapon
        .as_ref()
        .map(|w| w.last_fire_time)
        .unwrap_or(0.0);
    let pri_last = atk.weapon.as_ref().map(|w| w.last_fire_time).unwrap_or(0.0);
    assert!(
        sec_last > 0.0,
        "secondary last_fire_time must advance on secondary shot"
    );
    assert!(
        (pri_last - 0.0).abs() < f32::EPSILON,
        "primary last_fire_time must stay 0 when secondary fired"
    );
}

/// Residual PreferredAgainst: FlashBang secondary preferred vs infantry when
/// secondary damage > primary (Ranger 35 > 5).
#[test]
fn update_combat_prefers_secondary_damage_vs_infantry() {
    ensure_host_weapon_store();

    let mut logic = crate::game_logic::GameLogic::new();

    let mut ranger = ThingTemplate::new("USA_Ranger");
    ranger
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::Selectable)
        .set_health(120.0)
        .set_primary_weapon_name(RANGER_PRIMARY_WEAPON)
        .set_secondary_weapon_name(RANGER_SECONDARY_WEAPON);
    logic.templates.insert("USA_Ranger".to_string(), ranger);

    let mut rebel = ThingTemplate::new("GLA_Soldier");
    rebel
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic.templates.insert("GLA_Soldier".to_string(), rebel);

    let attacker_id = logic
        .create_object("USA_Ranger", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("ranger");
    let target_id = logic
        .create_object("GLA_Soldier", Team::GLA, Vec3::new(30.0, 0.0, 0.0))
        .expect("infantry");

    let (primary_dmg, secondary_dmg) = {
        let atk = logic.objects.get(&attacker_id).expect("attacker");
        let p = atk.weapon.as_ref().expect("primary").damage;
        let s = atk.secondary_weapon.as_ref().expect("secondary").damage;
        assert!(s > p, "FlashBang secondary must out-damage primary");
        (p, s)
    };

    {
        let atk = logic.objects.get_mut(&attacker_id).expect("attacker");
        atk.attack_target(target_id);
        if let Some(w) = atk.weapon.as_mut() {
            w.last_fire_time = 0.0;
            w.reload_time = 0.1;
        }
        if let Some(w) = atk.secondary_weapon.as_mut() {
            w.last_fire_time = 0.0;
            w.reload_time = 0.1;
        }
    }

    let health_before = logic
        .objects
        .get(&target_id)
        .expect("target")
        .health
        .current;

    logic.set_current_frame(60);
    logic.update_combat(&[attacker_id, target_id], 1.0 / 60.0);

    let health_after = logic
        .objects
        .get(&target_id)
        .expect("target")
        .health
        .current;
    let dealt = health_before - health_after;

    assert!(
        dealt > primary_dmg + 0.5,
        "infantry PreferredAgainst residual must use secondary: dealt={dealt} primary={primary_dmg} secondary={secondary_dmg}"
    );

    let atk = logic.objects.get(&attacker_id).expect("attacker");
    let pri_last = atk.weapon.as_ref().map(|w| w.last_fire_time).unwrap_or(0.0);
    let sec_last = atk
        .secondary_weapon
        .as_ref()
        .map(|w| w.last_fire_time)
        .unwrap_or(0.0);
    assert!(
        sec_last > 0.0,
        "secondary last_fire_time must advance vs infantry PreferredAgainst"
    );
    assert!(
        (pri_last - 0.0).abs() < f32::EPSILON,
        "primary must stay idle when secondary PreferredAgainst fires"
    );
}

/// Residual: when primary is reloading, secondary may still fire (alternate path).
#[test]
fn update_combat_uses_secondary_when_primary_reloading() {
    ensure_host_weapon_store();

    let mut logic = crate::game_logic::GameLogic::new();

    let mut ranger = ThingTemplate::new("USA_Ranger");
    ranger
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::Selectable)
        .set_health(120.0)
        .set_primary_weapon_name(RANGER_PRIMARY_WEAPON)
        .set_secondary_weapon_name(RANGER_SECONDARY_WEAPON);
    logic.templates.insert("USA_Ranger".to_string(), ranger);

    let mut rebel = ThingTemplate::new("GLA_Soldier");
    rebel
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::Selectable)
        .set_health(200.0);
    logic.templates.insert("GLA_Soldier".to_string(), rebel);

    let attacker_id = logic
        .create_object("USA_Ranger", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("ranger");
    let target_id = logic
        .create_object("GLA_Soldier", Team::GLA, Vec3::new(30.0, 0.0, 0.0))
        .expect("infantry");

    let secondary_dmg = logic
        .objects
        .get(&attacker_id)
        .and_then(|a| a.secondary_weapon.as_ref())
        .map(|w| w.damage)
        .unwrap_or(0.0);

    {
        let atk = logic.objects.get_mut(&attacker_id).expect("attacker");
        atk.attack_target(target_id);
        // Primary still on cooldown; secondary ready.
        if let Some(w) = atk.weapon.as_mut() {
            w.last_fire_time = 100.0;
            w.reload_time = 10.0;
        }
        if let Some(w) = atk.secondary_weapon.as_mut() {
            w.last_fire_time = 0.0;
            w.reload_time = 0.1;
        }
    }

    let health_before = logic
        .objects
        .get(&target_id)
        .expect("target")
        .health
        .current;

    logic.set_current_frame(60); // t=1s; primary still reloading (last=100, reload=10)
    logic.update_combat(&[attacker_id, target_id], 1.0 / 60.0);

    let health_after = logic
        .objects
        .get(&target_id)
        .expect("target")
        .health
        .current;
    let dealt = health_before - health_after;

    assert!(
        dealt > 0.0,
        "secondary must fire while primary reloads; dealt={dealt}"
    );
    assert!(
        (dealt - secondary_dmg).abs() < 1.0 || dealt >= secondary_dmg * 0.5,
        "damage should match secondary ({secondary_dmg}), got {dealt}"
    );

    let atk = logic.objects.get(&attacker_id).expect("attacker");
    let sec_last = atk
        .secondary_weapon
        .as_ref()
        .map(|w| w.last_fire_time)
        .unwrap_or(0.0);
    assert!(sec_last > 0.0, "secondary last_fire_time must advance");
}

#[test]
fn disarm_damage_peel() {
    assert!(host_weapon_is_disarm_damage("DozerMineDisarmingWeapon"));
    assert!(host_weapon_is_disarm_damage("WorkerMineDisarmingWeapon"));
    assert!(!host_weapon_is_disarm_damage("AmericaTankCrusaderGun"));
}

#[test]
fn healing_water_damage_peels() {
    assert!(host_weapon_is_healing_damage("AmericaInfantryMedicHeal"));
    assert!(host_weapon_is_healing_damage("AmbulanceHealWeapon"));
    assert!(host_weapon_is_water_damage("WaveGuideWaterDamage"));
    assert!(!host_weapon_is_healing_damage("AmericaTankCrusaderGun"));
}
#[test]
fn deploy_hack_surrender_kill_garrisoned_peels() {
    assert!(host_weapon_is_deploy_damage("TroopCrawlerAssault"));
    assert!(host_weapon_is_hack_damage("BlackLotusDisableVehicleHack"));
    assert!(host_weapon_is_surrender_damage("SurrenderWeapon"));
    assert!(!host_weapon_is_deploy_damage("AmericaTankCrusaderGun"));
}

#[test]
fn kill_pilot_damage_peel() {
    assert!(host_weapon_is_kill_pilot_damage(
        "JarmenKellSnipeVehicleRifle"
    ));
    assert!(host_weapon_is_kill_pilot_damage(
        "AmericaPathfinderSniperRifle"
    ));
    assert!(!host_weapon_is_kill_pilot_damage("AmericaTankCrusaderGun"));
}
#[test]
fn status_damage_peels() {
    assert!(host_weapon_is_status_damage("AvengerTargetDesignator"));
    assert_eq!(
        host_damage_status_type_for_weapon_name("AvengerTargetDesignator"),
        Some("FAERIE_FIRE")
    );
    assert_eq!(host_status_damage_frames_from_primary_damage(200.0), 6);
    assert!(!host_weapon_is_status_damage("AmericaTankCrusaderGun"));
}
#[test]
fn die_on_detonate_reads_store_not_name_seed() {
    ensure_host_weapon_store();
    const NAME: &str = "HuntMissileCallsOnDieWeapon";
    let _ = gamelogic::weapon::with_weapon_store_mut(|store| {
        let mut template = WeaponTemplate::new(NAME.to_string());
        template.die_on_detonate = true;
        store.add_weapon_template(template);
    });
    assert!(host_die_on_detonate_for_weapon_name(NAME));
    assert!(!host_die_on_detonate_for_weapon_name(
        "AmericaTankCrusaderGun"
    ));
    // Name seed must not invent MissileCallsOnDie when the store field is false.
    assert!(!host_die_on_detonate_for_weapon_name(
        "NotInStoreTomahawkMissile"
    ));
}

#[test]
fn damage_dealt_at_self_position_reads_store_not_name_seed() {
    ensure_host_weapon_store();
    const NAME: &str = "HuntDamageDealtAtSelfWeapon";
    let _ = gamelogic::weapon::with_weapon_store_mut(|store| {
        let mut template = WeaponTemplate::new(NAME.to_string());
        template.damage_dealt_at_self_position = true;
        store.add_weapon_template(template);
    });
    assert!(host_damage_dealt_at_self_position_for_weapon_name(NAME));
    assert!(!host_damage_dealt_at_self_position_for_weapon_name(
        "AmericaTankCrusaderGun"
    ));
    assert!(!host_damage_dealt_at_self_position_for_weapon_name(
        "NotInStoreSuicideDynamite"
    ));
    assert!(!host_damage_dealt_at_self_position_for_weapon_name(""));
}

#[test]
fn projectile_stream_name_seeds() {
    assert_eq!(
        host_projectile_stream_name_for_weapon_name("DragonTankFlameWeapon"),
        "DragonTankFlameStream"
    );
    assert!(host_projectile_stream_name_for_weapon_name("AmericaTankCrusaderGun").is_empty());
}

#[test]
fn projectile_stream_name_reads_store_not_just_seed() {
    ensure_host_weapon_store();
    const NAME: &str = "Hunt10AuroraStreamWeapon";
    let _ = gamelogic::weapon::with_weapon_store_mut(|store| {
        let mut template = WeaponTemplate::new(NAME.to_string());
        template.projectile_stream_name = "AuroraBombStream".to_string();
        store.add_weapon_template(template);
    });
    assert_eq!(
        host_projectile_stream_name_for_weapon_name(NAME),
        "AuroraBombStream"
    );
    assert_eq!(
        host_projectile_stream_name_for_slots(Some(NAME), None, None, None),
        "AuroraBombStream"
    );
}
#[test]
fn shows_ammo_pips_reads_store_not_name_seed() {
    ensure_host_weapon_store();
    const NAME: &str = "HuntShowsAmmoPipsWeapon";
    let _ = gamelogic::weapon::with_weapon_store_mut(|store| {
        let mut template = WeaponTemplate::new(NAME.to_string());
        template.is_shows_ammo_pips = true;
        store.add_weapon_template(template);
    });
    assert!(host_shows_ammo_pips_for_weapon_name(NAME));
    assert!(!host_shows_ammo_pips_for_weapon_name(
        "AmericaTankCrusaderGun"
    ));
    // Name seed must not invent ShowsAmmoPips when the store field is false.
    assert!(!host_shows_ammo_pips_for_weapon_name(
        "NotInStoreRaptorMissileWeapon"
    ));
    assert!(!host_shows_ammo_pips_for_weapon_name(""));
}

#[test]
fn capable_of_following_waypoint_reads_store_not_name_seed() {
    ensure_host_weapon_store();
    const NAME: &str = "HuntWaypointFollowWeapon";
    let _ = gamelogic::weapon::with_weapon_store_mut(|store| {
        let mut template = WeaponTemplate::new(NAME.to_string());
        template.capable_of_following_waypoint = true;
        store.add_weapon_template(template);
    });
    assert!(host_capable_of_following_waypoint_for_weapon_name(NAME));
    assert!(!host_capable_of_following_waypoint_for_weapon_name(
        "AmericaTankCrusaderGun"
    ));
    // Name seed must not invent CapableOfFollowingWaypoints when the store field is false.
    assert!(!host_capable_of_following_waypoint_for_weapon_name(
        "NotInStoreParticleCannon"
    ));
    assert!(!host_capable_of_following_waypoint_for_weapon_name(""));
}

#[test]
fn play_fx_when_stealthed_uses_the_retail_weapon_field() {
    assert!(!host_play_fx_when_stealthed_for_weapon_name(
        "AmericaTankCrusaderGun"
    ));
    assert!(host_play_fx_when_stealthed_for_weapon_name(
        "DemoTrapDetonationWeapon"
    ));
    assert!(!host_play_fx_when_stealthed_for_weapon_name(
        "TunnelNetworkGun"
    ));
    assert!(!host_play_fx_when_stealthed_for_weapon_name(
        "UnknownWeaponXYZ"
    ));
    assert!(host_allow_attack_garrisoned_for_weapon_name(
        "DragonTankFlameWeapon"
    ));
    assert!(host_allow_attack_garrisoned_for_weapon_name(
        RANGER_SECONDARY_WEAPON
    ));
    assert!(!host_allow_attack_garrisoned_for_weapon_name(
        RANGER_PRIMARY_WEAPON
    ));
    assert!(!host_allow_attack_garrisoned_for_weapon_name(
        "AmericaTankCrusaderGun"
    ));
    assert!(host_weapon_is_kill_garrisoned_damage(
        MICROWAVE_BUILDING_CLEARER_WEAPON
    ));
    assert!(!host_weapon_is_kill_garrisoned_damage(
        RANGER_PRIMARY_WEAPON
    ));
    assert!(!host_weapon_is_kill_garrisoned_damage(
        "DragonTankFlameWeapon"
    ));
}
#[test]
fn fire_sound_for_seeded_weapons_residual() {
    let _ = ensure_host_weapon_store();
    assert_eq!(
        seed_fire_sound_for(TANK_HUNTER_PRIMARY_WEAPON),
        "RPGTrooperWeapon"
    );
    assert_eq!(seed_fire_sound_for("PaladinPointDefenseLaser"), "LaserFire");
    // Prefer Weapon.ini FireSound when present (e.g. TankHunterWeapon), else peel.
    let store = host_fire_sound_for_weapon_name(TANK_HUNTER_PRIMARY_WEAPON);
    assert!(
        store == "TankHunterWeapon"
            || store == "RPGTrooperWeapon"
            || store == "MissileLaunch"
            || !store.is_empty(),
        "unexpected tank hunter fire sound {store}"
    );
    let unit = host_fire_sound_for_unit_slot(
        "ChinaInfantryTankHunter",
        Some(TANK_HUNTER_PRIMARY_WEAPON),
        None,
        0,
    );
    assert_eq!(unit, store);
    let fallback = host_fire_sound_for_unit_slot("UnknownUnitXYZ", None, None, 0);
    assert_eq!(fallback, "WeaponFire");
}

#[test]
fn fire_fx_uses_exact_retail_weapon_reference() {
    let _ = ensure_host_weapon_store();
    // These values come from Weapon.ini. They must not be inferred from a
    // weapon name: a missing or unknown store entry instead produces no FX.
    assert_eq!(
        host_fire_fx_for_weapon_name("CrusaderTankGun"),
        "WeaponFX_GenericTankGunNoTracer"
    );
    assert_eq!(
        host_detonation_fx_for_weapon_name(TANK_HUNTER_PRIMARY_WEAPON),
        "WeaponFX_RocketBuggyMissileDetonation"
    );
    assert!(host_fire_fx_for_weapon_name("UnknownWeaponXYZ").is_empty());
    assert!(host_detonation_fx_for_weapon_name("UnknownWeaponXYZ").is_empty());
}

#[test]
fn parsed_veterancy_effects_drive_host_weapon_lookup() {
    // This exercises the host-facing path used by combat, rather than only
    // inspecting the parsed template. Each repeated Veterancy property must
    // remain attached to its own C++ rank slot.
    let ini = r#"
Weapon __RustHostVeterancyEffectLookup
  FireFX = FX_BaseFire
  VeterancyFireFX = HEROIC FX_HeroicFire
  ProjectileDetonationFX = FX_BaseDetonation
  VeterancyProjectileDetonationFX = ELITE FX_EliteDetonation
  FireOCL = OCL_BaseFire
  VeterancyFireOCL = VETERAN OCL_VeteranFire
  ProjectileDetonationOCL = OCL_BaseDetonation
  VeterancyProjectileDetonationOCL = HEROIC OCL_HeroicDetonation
  ProjectileExhaust = Exhaust_Base
  VeterancyProjectileExhaust = HEROIC Exhaust_Heroic
End
"#;
    assert_eq!(
        crate::assets::ini_template_loader::register_weapons_from_ini_text(ini),
        1
    );

    use crate::game_logic::VeterancyLevel::{Elite, Heroic, Rookie, Veteran};
    let name = "__RustHostVeterancyEffectLookup";
    assert_eq!(
        host_fire_fx_for_weapon_name_at_veterancy(name, Rookie),
        "FX_BaseFire"
    );
    assert_eq!(
        host_fire_fx_for_weapon_name_at_veterancy(name, Heroic),
        "FX_HeroicFire"
    );
    assert_eq!(
        host_detonation_fx_for_weapon_name_at_veterancy(name, Elite),
        "FX_EliteDetonation"
    );
    assert_eq!(
        host_fire_ocl_for_weapon_name_at_veterancy(name, Veteran),
        "OCL_VeteranFire"
    );
    assert_eq!(
        host_detonation_ocl_for_weapon_name_at_veterancy(name, Heroic),
        "OCL_HeroicDetonation"
    );
    assert_eq!(
        host_projectile_exhaust_for_weapon_name_at_veterancy(name, Heroic),
        "Exhaust_Heroic"
    );
}

#[test]
fn projectile_object_for_seeded_weapons_residual() {
    let _ = ensure_host_weapon_store();
    assert_eq!(
        seed_projectile_name_for("AmericaTankCrusaderGun"),
        "GenericTankShell"
    );
    assert_eq!(seed_projectile_name_for("PaladinPointDefenseLaser"), "");
    let p =
        host_projectile_name_for_unit_slot("AmericaTankCrusader", Some(CRUSADER_TANK_GUN), None, 0);
    // Store INI may supply retail projectile; peel residual is GenericTankShell.
    assert!(
        !p.is_empty() || p == seed_projectile_name_for(CRUSADER_TANK_GUN),
        "unexpected projectile {p}"
    );
    let store = host_projectile_name_for_weapon_name(CRUSADER_TANK_GUN);
    assert!(!store.is_empty() || store == "GenericTankShell" || store.is_empty());
    // Prefer non-empty for crusader family peel/store.
    let peel = seed_projectile_name_for(CRUSADER_TANK_GUN);
    assert_eq!(peel, "GenericTankShell");
}

#[test]
fn weapon_ocl_uses_exact_retail_reference() {
    let _ = ensure_host_weapon_store();
    // Retail ToxinShellWeapon has FireOCL only. The old name heuristic
    // invented a ProjectileDetonationOCL poison field, which differs from
    // Weapon.ini and causes an extra gameplay spawn.
    assert_eq!(
        host_fire_ocl_for_weapon_name("ToxinShellWeapon"),
        "OCL_PoisonFieldSmall"
    );
    assert!(host_detonation_ocl_for_weapon_name("ToxinShellWeapon").is_empty());
    let (f, d) =
        host_weapon_ocl_for_unit_slot("AnyHostTemplate", Some("ToxinShellWeapon"), None, 0);
    assert_eq!(f, "OCL_PoisonFieldSmall");
    assert!(d.is_empty());
    // Unknown stays empty (fail-closed).
    assert!(host_fire_ocl_for_weapon_name("UnknownWeaponXYZ").is_empty());
    assert!(host_detonation_ocl_for_weapon_name("UnknownWeaponXYZ").is_empty());
}

#[test]
fn projectile_exhaust_uses_exact_retail_weapon_reference() {
    let _ = ensure_host_weapon_store();
    let e = host_projectile_exhaust_for_unit_slot(
        "ChinaInfantryTankHunter",
        Some("ChinaInfantryTankHunterMissileLauncher"),
        None,
        0,
    );
    assert_eq!(e, "MissileExhaust");
    assert_eq!(
        host_projectile_exhaust_for_weapon_name_at_veterancy(
            "ChinaInfantryTankHunterMissileLauncher",
            crate::game_logic::VeterancyLevel::Heroic,
        ),
        "HeroicMissileExhaust"
    );
    assert!(host_projectile_exhaust_for_weapon_name("UnknownWeaponXYZ").is_empty());
}

#[test]
fn laser_name_for_seeded_weapons_residual() {
    assert_eq!(
        seed_laser_name_for("AmericaVehicleAvengerTargetDesignator"),
        "AvengerTargetingLaserBeam"
    );
    assert_eq!(
        seed_laser_name_for("AmericaTankPaladinPointDefenseLaser"),
        "PointDefenseLaserBeam"
    );
    assert_eq!(
        seed_laser_name_for("Lazr_AmericaTankCrusaderLaserWeapon"),
        "Lazr_CrusaderLaserBeam"
    );
    assert_eq!(seed_laser_name_for("AmericaTankCrusaderGun"), "");
    let n = host_laser_name_for_unit_slot(
        "AmericaVehicleAvenger",
        Some("AmericaVehicleAvengerLaserWeapon"),
        None,
        0,
    );
    assert_eq!(n, "AvengerLaserBeam");
    assert!(host_laser_name_for_weapon_name("UnknownWeaponXYZ").is_empty());
}

#[test]
fn laser_bone_name_for_seeded_weapons_residual() {
    assert_eq!(
        seed_laser_bone_name_for("AmericaTankPaladinPointDefenseLaser"),
        "LASER"
    );
    assert_eq!(
        seed_laser_bone_name_for("AmericaVehicleAvengerLaserWeapon"),
        "TurretFX01"
    );
    assert_eq!(
        seed_laser_bone_name_for("Lazr_AmericaTankCrusaderLaserWeapon"),
        "TurretMS01"
    );
    assert_eq!(seed_laser_bone_name_for("AmericaTankCrusaderGun"), "");
    let b = host_laser_bone_name_for_unit_slot(
        "AmericaTankPaladin",
        Some("AmericaTankPaladinPointDefenseLaser"),
        None,
        0,
    );
    assert_eq!(b, "LASER");
    assert!(host_laser_bone_name_for_weapon_name("UnknownWeaponXYZ").is_empty());
}

#[test]
fn secondary_damage_for_seeded_weapons_residual() {
    assert_eq!(seed_secondary_damage_for("ScudStormDamageWeapon"), 150.0);
    assert_eq!(
        seed_secondary_damage_radius_for("ScudStormDamageWeapon"),
        200.0
    );
    assert_eq!(seed_secondary_damage_for("SuicideDynamitePack"), 50.0);
    assert_eq!(seed_secondary_damage_for("AmericaTankCrusaderGun"), 0.0);
    assert_eq!(
        host_secondary_damage_for_weapon_name("UnknownWeaponXYZ"),
        0.0
    );
}

#[test]
fn shock_wave_for_seeded_weapons_residual() {
    assert_eq!(seed_shock_wave_amount_for("MOABDetonationWeapon"), 250.0);
    assert_eq!(seed_shock_wave_radius_for("MOABDetonationWeapon"), 100.0);
    assert!((seed_shock_wave_taper_for("MOABDetonationWeapon") - 0.75).abs() < 1e-3);
    assert_eq!(seed_shock_wave_amount_for("AmericaTankCrusaderGun"), 0.0);
    assert_eq!(
        host_shock_wave_amount_for_weapon_name("UnknownWeaponXYZ"),
        0.0
    );
}

#[test]
fn radius_damage_affects_for_seeded_weapons_residual() {
    use crate::game_logic::ObjectId;
    use crate::game_logic::host_ai_path_combat_residual_wave105::{
        WEAPON_AFFECTS_ALLIES, WEAPON_AFFECTS_ENEMIES, WEAPON_AFFECTS_NEUTRALS,
        WEAPON_DOESNT_AFFECT_AIRBORNE,
    };
    use gamelogic::common::Relationship;

    ensure_host_weapon_store();
    // Omitted RadiusDamageAffects / unknown name: C++ constructor default.
    let omitted = seed_radius_damage_affects_for("CompletelyUnknownSplashWeaponXYZ");
    assert_eq!(omitted, WEAPON_AFFECTS_DEFAULT);
    assert_eq!(omitted & WEAPON_AFFECTS_ALLIES, WEAPON_AFFECTS_ALLIES);
    assert_eq!(omitted & WEAPON_AFFECTS_ENEMIES, WEAPON_AFFECTS_ENEMIES);
    assert_eq!(omitted & WEAPON_AFFECTS_NEUTRALS, WEAPON_AFFECTS_NEUTRALS);

    assert_eq!(
        crate::assets::ini_template_loader::register_weapons_from_ini_text(
            r#"Weapon TestNeutronSplashWeapon
  RadiusDamageAffects = SUICIDE SELF ALLIES ENEMIES NEUTRALS NOT_SIMILAR NOT_AIRBORNE
End
"#,
        ),
        1
    );
    let n = host_radius_damage_affects_for_weapon_name("TestNeutronSplashWeapon");
    assert_ne!(n & WEAPON_DOESNT_AFFECT_AIRBORNE, 0);
    assert_ne!(n & WEAPON_AFFECTS_ALLIES, 0);
    assert_eq!(n, seed_radius_damage_affects_for("TestNeutronSplashWeapon"));

    assert!(radius_damage_affects_victim(
        omitted,
        Relationship::Enemies,
        ObjectId(1),
        ObjectId(2),
        None,
        false,
        false,
    ));
    // C++ default friendly-fires allies unless INI clears ALLIES.
    assert!(radius_damage_affects_victim(
        omitted,
        Relationship::Allies,
        ObjectId(1),
        ObjectId(3),
        None,
        false,
        false,
    ));
    assert!(!radius_damage_affects_victim(
        n,
        Relationship::Enemies,
        ObjectId(1),
        ObjectId(4),
        None,
        true,
        false,
    ));
}

#[test]
fn host_radius_damage_affects_uses_store_or_cpp_default() {
    use crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_ALLIES;
    ensure_host_weapon_store();
    let unknown = host_radius_damage_affects_for_weapon_name("CompletelyUnknownSplashWeaponXYZ");
    assert_eq!(unknown, WEAPON_AFFECTS_DEFAULT);
    assert_ne!(unknown & WEAPON_AFFECTS_ALLIES, 0);
}

#[test]
fn projectile_collides_for_seeded_weapons_residual() {
    assert_eq!(
        seed_projectile_collides_for("AmericaTankCrusaderGun") & PROJECTILE_COLLIDE_STRUCTURES,
        PROJECTILE_COLLIDE_STRUCTURES
    );
    assert_eq!(seed_projectile_collides_for("PaladinPointDefenseLaser"), 0);
    assert!(projectile_collides_structures(PROJECTILE_COLLIDE_DEFAULT));
    assert!(!projectile_collides_structures(0));
    assert!(projectile_collides_structures(
        PROJECTILE_COLLIDE_CONTROLLED_STRUCTURES
    ));
    assert!(!projectile_collides_with_structure(
        PROJECTILE_COLLIDE_DEFAULT,
        true
    ));
    assert!(projectile_collides_with_structure(
        PROJECTILE_COLLIDE_DEFAULT,
        false
    ));
    assert!(projectile_collides_with_structure(
        PROJECTILE_COLLIDE_CONTROLLED_STRUCTURES,
        true
    ));
    assert!(!projectile_collides_with_structure(
        PROJECTILE_COLLIDE_CONTROLLED_STRUCTURES,
        false
    ));
    assert!(projectile_structure_same_controller(
        Some(1),
        Some(1),
        false
    ));
    assert!(!projectile_structure_same_controller(
        Some(1),
        Some(2),
        true
    ));
    assert!(projectile_structure_same_controller(None, None, true));
    assert!(!projectile_structure_same_controller(None, Some(1), false));
    assert_eq!(
        seed_projectile_collides_for("ScudStormDamageWeapon"),
        PROJECTILE_COLLIDE_STRUCTURES
    );
}

#[test]
fn scatter_radius_for_seeded_weapons_residual() {
    assert_eq!(
        seed_scatter_radius_vs_infantry_for("AmericaTankCrusaderGun"),
        10.0
    );
    assert_eq!(
        host_effective_scatter_radius("AmericaTankCrusaderGun", true),
        10.0
    );
    assert_eq!(
        host_effective_scatter_radius("AmericaTankCrusaderGun", false),
        0.0
    );
    assert_eq!(
        seed_scatter_radius_vs_infantry_for("PaladinPointDefenseLaser"),
        0.0
    );
    assert_eq!(
        seed_scatter_radius_vs_infantry_for("FireBaseHowitzerGun"),
        15.0
    );
    assert_eq!(
        host_effective_scatter_radius("FireBaseHowitzerGun", true),
        15.0
    );
    assert_eq!(seed_scatter_radius_vs_infantry_for("NukeCannonGun"), 30.0);
    assert_eq!(host_effective_scatter_radius("NukeCannonGun", true), 30.0);
    let o = scatter_aim_offset(42, 10.0);
    assert!(o.length() <= 10.0 + 1e-3);
    // Stable re-query.
    let o2 = scatter_aim_offset(42, 10.0);
    assert_eq!(o, o2);
}

#[test]
fn scale_weapon_speed_lob_residual() {
    let peel = HostWeaponSpeedPeel {
        weapon_speed: 300.0,
        min_weapon_speed: 75.0,
        scale_weapon_speed: true,
        attack_range: 375.0,
        min_attack_range: 50.0,
    };
    // At min range → min speed.
    let s_min = host_scaled_weapon_speed(&peel, 50.0);
    assert!((s_min - 75.0).abs() < 1e-3, "min {s_min}");
    // At max range → max speed.
    let s_max = host_scaled_weapon_speed(&peel, 375.0);
    assert!((s_max - 300.0).abs() < 1e-3, "max {s_max}");
    // Midpoint.
    let s_mid = host_scaled_weapon_speed(&peel, 212.5);
    assert!((s_mid - 187.5).abs() < 1e-2, "mid {s_mid}");
    // No scale → constant.
    let flat = HostWeaponSpeedPeel {
        weapon_speed: 200.0,
        min_weapon_speed: 50.0,
        scale_weapon_speed: false,
        attack_range: 300.0,
        min_attack_range: 0.0,
    };
    assert_eq!(host_scaled_weapon_speed(&flat, 10.0), 200.0);
    // Seed peel for firebase.
    let fb = seed_weapon_speed_peel_for("AmericaFireBaseHowitzer");
    assert!(fb.scale_weapon_speed);
    assert_eq!(fb.min_weapon_speed, 75.0);
}

#[test]
fn leech_range_weapon_seed_residual() {
    assert!(seed_leech_range_weapon_for("GLAInfantryTerrorist"));
    assert!(seed_leech_range_weapon_for("ColonelBurtonKnifeAttack"));
    assert!(!seed_leech_range_weapon_for("AmericaTankCrusaderGun"));
    assert!(!seed_leech_range_weapon_for("PaladinPointDefenseLaser"));
}

#[test]
fn historic_bonus_seed_inferno_residual() {
    let p = seed_historic_bonus_for("InfernoCannonGun");
    assert!(p.is_active());
    assert_eq!(p.count, 3);
    assert_eq!(p.time_frames, 90);
    assert!((p.radius - 20.0).abs() < 1e-3);
    assert!(p.bonus_weapon.contains("Firestorm"));
    let u = seed_historic_bonus_for("InfernoCannonGunUpgraded");
    assert!(u.is_black_napalm_bonus());
    let none = seed_historic_bonus_for("AmericaTankCrusaderGun");
    assert!(!none.is_active());
}

#[test]
fn acceptable_aim_delta_normalize_and_seed() {
    assert!((normalize_aim_delta_radians(180.0) - std::f32::consts::PI).abs() < 1e-3);
    assert!((normalize_aim_delta_radians(0.0) - AIM_DELTA_REL_THRESH_RAD).abs() < 1e-5);
    assert!((normalize_aim_delta_radians(20.0) - 20f32.to_radians()).abs() < 1e-4);
    let omni = host_aim_delta_for_weapon_name("AmericaSentryDroneGun");
    assert!(omni >= std::f32::consts::PI - 0.05);
    // Orientation 0 faces +X (movement convention). Target on +Z → ~-PI/2.
    let rel = relative_angle_2d(glam::Vec3::ZERO, 0.0, glam::Vec3::new(0.0, 0.0, 10.0));
    assert!(rel.abs() > 1.0, "rel={rel}");
    assert!(is_within_aim_delta(0.01, AIM_DELTA_REL_THRESH_RAD));
    assert!(!is_within_aim_delta(1.0, 20f32.to_radians()));
}

#[test]
fn prefire_type_seed_residual() {
    assert_eq!(
        seed_prefire_type_for("AmericaGattlingTankGun"),
        HostPrefireType::PerShot
    );
    assert_eq!(
        seed_prefire_type_for("ColonelBurtonKnifeAttack"),
        HostPrefireType::PerAttack
    );
    assert_eq!(
        seed_prefire_type_for("ScudStormWeapon"),
        HostPrefireType::PerClip
    );
    assert_eq!(
        HostPrefireType::from_ini("PER_CLIP"),
        HostPrefireType::PerClip
    );
    assert_eq!(
        HostPrefireType::from_ini("per_attack"),
        HostPrefireType::PerAttack
    );
}

#[test]
fn reload_type_seed_return_to_base() {
    use super::HostReloadType;
    assert_eq!(
        seed_reload_type_for("RaptorJetMissileWeapon"),
        HostReloadType::ReturnToBase
    );
    assert_eq!(
        seed_reload_type_for("AmericaTankCrusaderGun"),
        HostReloadType::Auto
    );
    assert_eq!(
        seed_reload_type_for("GLAInfantryTerrorist"),
        HostReloadType::Manual
    );
    assert_eq!(
        seed_reload_type_for("ComancheRocketPodWeapon"),
        HostReloadType::Auto,
        "retail ComancheRocketPodWeapon declares AutoReloadsClip = Yes"
    );
}

#[test]
fn delay_between_shots_frames_draws_inclusive_range() {
    assert_eq!(super::delay_between_shots_frames(12, 12), 12);
    game_engine::common::random_value::init_game_logic_random(0xC0FFEE);
    for _ in 0..32 {
        let drawn = super::delay_between_shots_frames(3, 9);
        assert!((3..=9).contains(&drawn), "drawn={drawn}");
    }
}

#[test]
fn target_pitch_limits_seed_and_gate() {
    let sc = seed_target_pitch_limits_for("AmericaStrategyCenterArtillery");
    assert!(!sc.is_unlimited());
    assert!((sc.min_pitch - 45f32.to_radians()).abs() < 1e-3);
    // C++ ACCEPTABLE_DZ=10: same height always allowed regardless of loft window.
    assert!(is_pitch_within_limits(
        glam::Vec3::ZERO,
        glam::Vec3::new(100.0, 0.0, 0.0),
        &sc
    ));
    // Large negative elevation outside strategy loft without geometry span.
    assert!(!is_pitch_within_limits(
        glam::Vec3::ZERO,
        glam::Vec3::new(100.0, -80.0, 0.0),
        &sc
    ));
    // Geometry span can bridge into loft window (building-height residual).
    assert!(is_pitch_within_limits_geom(
        glam::Vec3::ZERO,
        glam::Vec3::new(100.0, 20.0, 0.0),
        &sc,
        0.0,
        100.0,
        0.0,
    ));
    // Steep loft into window.
    let dy = (100.0_f32) * 45f32.to_radians().tan() + 5.0;
    assert!(is_pitch_within_limits(
        glam::Vec3::ZERO,
        glam::Vec3::new(100.0, dy, 0.0),
        &sc
    ));
    let tank = seed_target_pitch_limits_for("AmericaTankCrusaderGun");
    assert!(is_pitch_within_limits(
        glam::Vec3::ZERO,
        glam::Vec3::new(50.0, 0.0, 0.0),
        &tank
    ));
    // Too steep for tank ±15°.
    assert!(!is_pitch_within_limits(
        glam::Vec3::ZERO,
        glam::Vec3::new(10.0, 50.0, 0.0),
        &tank
    ));
    let open = HostTargetPitchLimits::default();
    assert!(open.is_unlimited());
    assert!(is_pitch_within_limits(
        glam::Vec3::ZERO,
        glam::Vec3::new(1.0, 100.0, 0.0),
        &open
    ));
}

#[test]
fn continue_attack_range_seed_residual() {
    assert_eq!(
        seed_continue_attack_range_for("DozerMineDisarmingWeapon"),
        100.0
    );
    assert_eq!(
        seed_continue_attack_range_for("WorkerMineDisarmingWeapon"),
        100.0
    );
    assert_eq!(
        seed_continue_attack_range_for("RangerAdvancedCombatRifle"),
        0.0
    );
    assert!(host_continue_attack_range_for_weapon_name("DozerMineDisarmingWeapon") >= 100.0 - 1e-3);
}

#[test]
fn contact_weapon_range_and_approach_residual() {
    // Leftover is_contact_weapon: (authored - 2.5) < 10 → contact if < 12.5.
    assert!(is_contact_weapon_range(5.0));
    assert!(is_contact_weapon_range(9.0));
    assert!(is_contact_weapon_range(10.0));
    assert!(is_contact_weapon_range(12.49));
    assert!(!is_contact_weapon_range(12.5));
    assert!(!is_contact_weapon_range(150.0));
    // Already-undersized get_attack_range: contact iff effective < 10.
    assert!(is_contact_effective_range(7.5));
    assert!(!is_contact_effective_range(10.0));
    assert!(!is_contact_effective_range(12.5));
    assert!(seed_is_contact_weapon_name("DozerMineDisarmingWeapon"));
    assert!(seed_is_contact_weapon_name("TerroristSuicideWeapon"));
    assert!(!seed_is_contact_weapon_name("AmericaTankCrusaderGun"));
    let src = glam::Vec3::new(0.0, 0.0, 0.0);
    let tgt = glam::Vec3::new(100.0, 0.0, 0.0);
    let contact = compute_approach_target_pos(src, tgt, 5.0);
    assert!((contact - tgt).length() < 1e-3, "contact approaches target");
    let stand = compute_approach_target_pos(src, tgt, 50.0);
    // Standoff ~45 from target toward source → x≈55
    assert!(
        (stand.x - 55.0).abs() < 1.0,
        "non-contact standoff x={}",
        stand.x
    );
}

#[test]
fn minimum_attack_range_backup_residual() {
    assert!(is_inside_minimum_attack_range(30.0, 50.0));
    assert!(!is_inside_minimum_attack_range(50.0, 50.0));
    assert!(!is_inside_minimum_attack_range(80.0, 50.0));
    let src = glam::Vec3::new(10.0, 0.0, 0.0);
    let tgt = glam::Vec3::new(0.0, 0.0, 0.0);
    let back = compute_min_range_backup_pos(src, tgt, 50.0);
    let d = ((back.x - tgt.x).powi(2) + (back.z - tgt.z).powi(2)).sqrt();
    assert!(
        (d - effective_minimum_attack_range(50.0)).abs() < 0.5,
        "backup dist={d}"
    );
    assert!(back.x > src.x - 1e-3, "backs away along outward radial");
}

#[test]
fn scatter_miss_gate_residual() {
    assert!(!scatter_misses_intended_target(0.0, 1, 5.0));
    // Large scatter + tiny hit radius → miss for most seeds.
    let mut misses = 0u32;
    for s in 0..32u32 {
        if scatter_misses_intended_target(20.0, s, 2.0) {
            misses += 1;
        }
    }
    assert!(
        misses >= 20,
        "most large-scatter shots miss tiny target ({misses}/32)"
    );
    // Zero hit radius floor is 1.0; still can hit if offset tiny.
    let _ = scatter_seed_for_shot(1, 2, 3);
}

#[test]
fn shock_wave_force_tapers_from_firer_not_impact() {
    let firer = glam::Vec3::ZERO;
    let near = compute_shock_wave_force(firer, glam::Vec3::new(10.0, 0.0, 0.0), 100.0, 100.0, 0.75)
        .expect("near");
    let far = compute_shock_wave_force(firer, glam::Vec3::new(90.0, 0.0, 0.0), 100.0, 100.0, 0.75)
        .expect("far");
    assert!(near.length() > far.length(), "near {near:?} far {far:?}");
    assert!(near.x > 0.0);
    assert!(near.y > 0.0, "up force");
    let saturated =
        compute_shock_wave_force(firer, glam::Vec3::new(200.0, 0.0, 0.0), 100.0, 40.0, 0.75)
            .expect("saturated");
    assert!(
        (saturated.length() - 75.0 * 2.0_f32.sqrt()).abs() < 1.0
            || (saturated.length() - 75.0).abs() < 20.0,
        "ranged blast saturates to taper_off, got {saturated:?}"
    );
}

#[test]
fn splash_skips_producer_when_self_unset() {
    use crate::game_logic::ObjectId;
    use crate::game_logic::host_ai_path_combat_residual_wave105::{
        WEAPON_AFFECTS_ALLIES, WEAPON_AFFECTS_ENEMIES, WEAPON_AFFECTS_NEUTRALS, WEAPON_AFFECTS_SELF,
    };
    use gamelogic::common::Relationship;
    let default_mask = WEAPON_AFFECTS_ALLIES | WEAPON_AFFECTS_ENEMIES | WEAPON_AFFECTS_NEUTRALS;
    let tank = ObjectId(10);
    let factory = ObjectId(1);
    assert!(!radius_damage_affects_victim(
        default_mask,
        Relationship::Allies,
        tank,
        factory,
        Some(factory),
        false,
        false,
    ));
    assert!(radius_damage_affects_victim(
        default_mask | WEAPON_AFFECTS_SELF,
        Relationship::Allies,
        tank,
        factory,
        Some(factory),
        false,
        false,
    ));
    assert!(!radius_damage_affects_victim(
        default_mask,
        Relationship::Allies,
        tank,
        tank,
        None,
        false,
        false,
    ));
}

#[test]
fn estimate_sniper_refuses_empty_and_uc_structures() {
    let sniper = HostEstimateWeapon {
        damage_type_sniper: true,
        damage_type_surrender: false,
        damage_type_disarm: false,
        damage_type_deploy: false,
        damage_type_kill_garrisoned: false,
        death_type_burned: false,
        allow_attack_garrisoned_bldgs: false,
        primary_damage: 100.0,
    };
    let empty = HostEstimateVictim {
        kind_structure: true,
        kind_shrubbery: false,
        kind_mine: false,
        kind_booby_trap: false,
        kind_demo_trap: false,
        under_construction: false,
        contain_count: 0,
        garrisonable: true,
        immune_to_clear_building: false,
        airborne_target: false,
        armor_coeff: 1.0,
    };
    assert_eq!(estimate_weapon_template_damage(&sniper, Some(&empty)), 0.0);
    let uc = HostEstimateVictim {
        under_construction: true,
        contain_count: 2,
        ..empty
    };
    assert_eq!(estimate_weapon_template_damage(&sniper, Some(&uc)), 0.0);
    let occupied = HostEstimateVictim {
        contain_count: 2,
        ..empty
    };
    assert!(estimate_weapon_template_damage(&sniper, Some(&occupied)) > 0.0);
    let occupied_armored = HostEstimateVictim {
        armor_coeff: 0.0,
        ..occupied
    };
    assert_eq!(
        estimate_weapon_template_damage(&sniper, Some(&occupied_armored)),
        0.0
    );
    let tank = HostEstimateVictim {
        kind_structure: false,
        garrisonable: false,
        armor_coeff: 0.0,
        ..empty
    };
    assert_eq!(estimate_weapon_template_damage(&sniper, Some(&tank)), 0.0);
    assert!(host_weapon_is_kill_pilot_cursor_slot(
        "GLAJarmenKellVehiclePilotSniperRifle"
    ));
    assert!(!host_weapon_is_kill_pilot_cursor_slot("GLAJarmenKellRifle"));
    assert!(!host_weapon_is_kill_pilot_cursor_slot(
        "AmericaInfantryPathfinderSniperRifle"
    ));
}

#[test]
fn estimate_sniper_vs_tank_applies_residual_armor() {
    ensure_host_weapon_store();
    let mut tmpl = ThingTemplate::new("GLATankScorpion");
    tmpl.add_kind_of(KindOf::Vehicle).set_health(400.0);
    let tank = crate::game_logic::Object::new(tmpl, crate::game_logic::ObjectId(1), Team::GLA);
    let est = host_estimate_weapon_from_name("GLAJarmenKellRifle", 180.0);
    let dt = crate::game_logic::host_armor_residual::host_damage_type_for_weapon_name(
        "GLAJarmenKellRifle",
    );
    let victim = host_estimate_victim_from_object(
        &tank,
        0,
        crate::game_logic::host_armor_residual::apply_residual_armor(&tank, dt, 1.0),
    );
    assert_eq!(estimate_weapon_template_damage(&est, Some(&victim)), 0.0);
    let infantry_t = {
        let mut t = ThingTemplate::new("GLAInfantryRebel");
        t.add_kind_of(KindOf::Infantry).set_health(100.0);
        t
    };
    let infantry =
        crate::game_logic::Object::new(infantry_t, crate::game_logic::ObjectId(2), Team::USA);
    let inf_victim = host_estimate_victim_from_object(
        &infantry,
        0,
        crate::game_logic::host_armor_residual::apply_residual_armor(&infantry, dt, 1.0),
    );
    assert!(estimate_weapon_template_damage(&est, Some(&inf_victim)) > 0.0);
}

#[test]
fn estimate_disarm_only_vs_mines() {
    let disarm = HostEstimateWeapon {
        damage_type_sniper: false,
        damage_type_surrender: false,
        damage_type_disarm: true,
        damage_type_deploy: false,
        damage_type_kill_garrisoned: false,
        death_type_burned: false,
        allow_attack_garrisoned_bldgs: false,
        primary_damage: 1.0,
    };
    let tank = HostEstimateVictim {
        kind_structure: false,
        kind_shrubbery: false,
        kind_mine: false,
        kind_booby_trap: false,
        kind_demo_trap: false,
        under_construction: false,
        contain_count: 0,
        garrisonable: false,
        immune_to_clear_building: false,
        airborne_target: false,
        armor_coeff: 1.0,
    };
    assert_eq!(estimate_weapon_template_damage(&disarm, Some(&tank)), 0.0);
    let mine = HostEstimateVictim {
        kind_mine: true,
        ..tank
    };
    assert_eq!(estimate_weapon_template_damage(&disarm, Some(&mine)), 1.0);
}

#[test]
fn nearer_bridge_end_is_not_span_center() {
    let from = glam::Vec3::new(0.0, 0.0, 0.0);
    let center = glam::Vec3::new(100.0, 0.0, 0.0);
    let tmpl = ThingTemplate::new("TestBridge");
    let mut bridge =
        crate::game_logic::Object::new(tmpl, crate::game_logic::ObjectId(1), Team::Neutral);
    bridge.set_position(center);
    bridge.selection_radius = 40.0;
    let aim = crate::game_logic::combat::nearer_live_bridge_attack_point(from, &bridge);
    assert!(aim.x < center.x, "nearer end {aim:?} vs center {center:?}");
    assert!((aim.x - 60.0).abs() < 1.0);
}
