use super::*;

#[test]
fn healing_and_water_damage_residuals() {
    let mut health_events = crate::game_logic::HostHealthEvents::default();

    use crate::game_logic::combat::DamageType;
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut t = ThingTemplate::new("Ranger");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Infantry);
    let mut unit = Object::new(t, ObjectId(1), Team::USA);
    unit.health.current = 40.0;
    unit.health.maximum = 100.0;

    // Healing restores HP and never destroys.
    assert!(!unit.take_damage_from_typed(
        25.0,
        Some(ObjectId(99)),
        DamageType::Healing,
        0,
        &mut health_events
    ));
    assert!((unit.health.current - 65.0).abs() < 1e-3);
    assert!(unit.is_alive());
    // C++ ActiveBody.cpp:817 copies the complete HEALING DamageInfo,
    // including the medic source ID. Guard rejects its HEALING type instead
    // of requiring the source ID to be cleared (AIGuardRetaliate.cpp:750).
    assert_eq!(unit.last_damage_source, Some(ObjectId(99)));
    assert_eq!(unit.last_damage_info_type, Some(DamageType::Healing));
    assert!(unit.last_healing_timestamp.is_some());

    // Cap at maximum.
    assert!(!unit.take_damage_from_typed(1000.0, None, DamageType::Healing, 0, &mut health_events));
    assert!((unit.health.current - 100.0).abs() < 1e-3);

    // Water deals normal HP damage.
    unit.health.current = 100.0;
    let destroyed =
        unit.take_damage_from_typed(30.0, None, DamageType::Water, 0, &mut health_events);
    assert!(!destroyed);
    assert!((unit.health.current - 70.0).abs() < 1e-3);

    // Dead units do not heal.
    unit.health.current = 0.0;
    unit.status.destroyed = true;
    assert!(!unit.take_damage_from_typed(50.0, None, DamageType::Healing, 0, &mut health_events));
    assert!((unit.health.current - 0.0).abs() < 1e-3);
}

#[test]
fn deploy_hack_surrender_kill_garrisoned_damage_residuals() {
    let mut health_events = crate::game_logic::HostHealthEvents::default();

    use crate::game_logic::combat::DamageType;
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    // DEPLOY: no HP, sets pending assault signal.
    let mut tt = ThingTemplate::new("TroopCrawler");
    tt.set_health(100.0);
    let mut crawler = Object::new(tt, ObjectId(1), Team::China);
    crawler.health.current = 100.0;
    assert!(!crawler.take_damage_from_typed(50.0, None, DamageType::Deploy, 0, &mut health_events));
    assert!((crawler.health.current - 100.0).abs() < 1e-3);
    assert!(!crawler.status.destroyed);

    // HACK: no HP.
    let mut ht = ThingTemplate::new("Tank");
    ht.set_health(100.0);
    ht.add_kind_of(KindOf::Vehicle);
    let mut tank = Object::new(ht, ObjectId(2), Team::USA);
    tank.health.current = 100.0;
    assert!(!tank.take_damage_from_typed(40.0, None, DamageType::Hack, 0, &mut health_events));
    assert!((tank.health.current - 100.0).abs() < 1e-3);

    // SURRENDER: retail ALLOW_SURRENDER off — lethal hit is normal HP death
    // (ActiveBody.cpp:517-537 commented + compiled out).
    let mut it = ThingTemplate::new("Ranger");
    it.set_health(50.0);
    it.add_kind_of(KindOf::Infantry);
    let mut ranger = Object::new(it, ObjectId(3), Team::USA);
    ranger.health.current = 50.0;
    assert!(ranger.take_damage_from_typed(
        50.0,
        None,
        DamageType::Surrender,
        0,
        &mut health_events
    ));
    assert!(!ranger.is_surrendered);
    assert!(!ranger.is_alive());

    // KILL_GARRISONED: structure HP untouched; pending count = floor(amount).
    let mut st = ThingTemplate::new("Bunker");
    st.set_health(500.0);
    st.add_kind_of(KindOf::Structure);
    let mut bunker = Object::new(st, ObjectId(4), Team::GLA);
    bunker.health.current = 500.0;
    assert!(!bunker.take_damage_from_typed(
        3.7,
        None,
        DamageType::KillGarrisoned,
        0,
        &mut health_events
    ));
    assert!((bunker.health.current - 500.0).abs() < 1e-3);
    assert_eq!(bunker.take_pending_kill_garrisoned(), 3);

    // PENALTY: normal HP path.
    let mut pt = ThingTemplate::new("Tank");
    pt.set_health(100.0);
    let mut penalized = Object::new(pt, ObjectId(5), Team::USA);
    penalized.health.current = 100.0;
    let _ =
        penalized.take_damage_from_typed(25.0, None, DamageType::Penalty, 0, &mut health_events);
    assert!((penalized.health.current - 75.0).abs() < 1e-3);
}

#[test]
fn disarm_damage_clears_mine_without_hp_on_tank() {
    let mut health_events = crate::game_logic::HostHealthEvents::default();

    use crate::game_logic::combat::DamageType;
    use crate::game_logic::host_mines::{HostMineData, HostMineKind};
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut mt = ThingTemplate::new("Mine");
    mt.set_health(10.0);
    let mut mine = Object::new(mt, ObjectId(1), Team::GLA);
    mine.mine_data = Some(HostMineData {
        kind: HostMineKind::LandMine,
        trigger_range: 10.0,
        detonation_damage: 100.0,
        detonation_radius: 20.0,
        secondary_damage: 0.0,
        secondary_radius: 0.0,
        demo_trap_profile: Default::default(),
        proximity_enabled: true,
        demo_trap_mode: crate::game_logic::host_mines::DemoTrapMode::Proximity,
        detonated: false,
        detonate_at_frame: None,
        attached_to: None,
        producer_id: None,
        ..HostMineData::land_mine()
    });
    mine.health.current = 10.0;
    assert!(mine.take_damage_from_typed(1.0, None, DamageType::Disarm, 0, &mut health_events));
    assert!(mine.status.destroyed);
    assert!(mine.mine_data.as_ref().unwrap().detonated);

    let mut tt = ThingTemplate::new("Tank");
    tt.set_health(100.0);
    tt.add_kind_of(KindOf::Vehicle);
    let mut tank = Object::new(tt, ObjectId(2), Team::USA);
    tank.health.current = 100.0;
    assert!(!tank.take_damage_from_typed(50.0, None, DamageType::Disarm, 0, &mut health_events));
    assert!((tank.health.current - 100.0).abs() < 1e-3);
}

#[test]
fn disarm_keeps_regenerating_china_pad() {
    let mut health_events = crate::game_logic::HostHealthEvents::default();

    use crate::game_logic::combat::DamageType;
    use crate::game_logic::host_mines::{HostMineData, MINE_MIN_HEALTH};
    use crate::game_logic::{Team, ThingTemplate};
    let mut mt = ThingTemplate::new("ChinaStandardMine");
    mt.set_health(100.0);
    let mut mine = Object::new(mt, ObjectId(3), Team::China);
    mine.mine_data = Some(HostMineData::land_mine_for_template("ChinaStandardMine"));
    mine.health.current = 100.0;
    assert!(!mine.take_damage_from_typed(1.0, None, DamageType::Disarm, 0, &mut health_events));
    assert!(!mine.status.destroyed);
    assert!((mine.health.current - MINE_MIN_HEALTH).abs() < 1e-3);
    assert_eq!(mine.mine_data.as_ref().unwrap().virtual_mines_remaining, 0);
    assert!(mine.mine_data.as_ref().unwrap().regenerates);
}

#[test]
fn disarm_damage_defuses_demo_trap_without_hp_splash_path() {
    let mut health_events = crate::game_logic::HostHealthEvents::default();

    use crate::game_logic::combat::DamageType;
    use crate::game_logic::host_mines::HostMineData;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut mt = ThingTemplate::new("GLADemoTrap");
    mt.set_health(100.0);
    mt.add_kind_of(KindOf::DemoTrap);
    let mut trap = Object::new(mt, ObjectId(9), Team::GLA);
    trap.mine_data = Some(HostMineData::demo_trap());
    trap.health.current = 100.0;
    assert!(trap.is_disarmable_mine());
    assert!(trap.take_damage_from_typed(1.0, None, DamageType::Disarm, 0, &mut health_events));
    assert!(trap.status.destroyed);
    assert!(trap.mine_data.as_ref().unwrap().detonated);
}

#[test]
fn kill_pilot_damage_unmans_vehicle_without_hp_loss() {
    let mut health_events = crate::game_logic::HostHealthEvents::default();

    use crate::game_logic::combat::DamageType;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut tmpl = ThingTemplate::new("Tank");
    tmpl.set_health(200.0);
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(tmpl, ObjectId(5), Team::China);
    o.health.current = 200.0;
    o.health.maximum = 200.0;
    assert!(!o.take_damage_from_typed(1.0, None, DamageType::KillPilot, 0, &mut health_events));
    assert!((o.health.current - 200.0).abs() < 1e-3);
    assert!(o.is_unmanned());
    assert_eq!(o.team, Team::Neutral);
}

#[test]
fn microwave_is_hp_through_armor_not_emp_subdual() {
    let mut health_events = crate::game_logic::HostHealthEvents::default();

    // C++ Damage.h:63 DAMAGE_MICROWAVE is ordinary HP; IsSubdualDamage false.
    // TankArmor MICROWAVE 0% (Armor.cpp:43-55 via ActiveBody.cpp:351).
    use crate::game_logic::combat::DamageType;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut tmpl = ThingTemplate::new("Tank");
    tmpl.set_health(100.0);
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(tmpl, ObjectId(4), Team::USA);
    o.health.current = 100.0;
    o.health.maximum = 100.0;
    assert!(!o.take_damage_from_typed(40.0, None, DamageType::Microwave, 0, &mut health_events));
    assert!((o.health.current - 100.0).abs() < 1e-3);
    assert!(o.subdual_damage.abs() < 1e-3);
    assert!(!o.is_subdued());
    assert!(!o.take_damage_from_typed(70.0, None, DamageType::EMP, 0, &mut health_events));
    assert!((o.health.current - 100.0).abs() < 1e-3);
    assert!(o.subdual_damage.abs() < 1e-3);
    assert!(!o.is_subdued());
}

#[test]
fn status_damage_applies_faerie_without_hp_loss() {
    let mut health_events = crate::game_logic::HostHealthEvents::default();

    use crate::game_logic::combat::DamageType;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut tmpl = ThingTemplate::new("PaintMe");
    tmpl.set_health(100.0);
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(tmpl, ObjectId(9), Team::GLA);
    o.health.current = 100.0;
    o.health.maximum = 100.0;
    let context = crate::game_logic::object::DamageHitContext::with_status(
        gamelogic::common::ObjectStatusTypes::FaerieFire,
    );
    let dead = o.take_damage_with_context(
        200.0,
        None,
        DamageType::Status,
        crate::game_logic::host_usa_pilot::HostDeathType::Normal,
        None,
        0,
        &context,
        &mut health_events,
        None,
    );
    assert!(!dead);
    assert!((o.health.current - 100.0).abs() < 1e-3);
    assert!(o.is_faerie_fire());
    assert!(o.faerie_fire_until_frame > 0);
}

#[test]
fn status_damage_none_does_not_paint_faerie_fire() {
    let mut health_events = crate::game_logic::HostHealthEvents::default();

    use crate::game_logic::combat::DamageType;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut tmpl = ThingTemplate::new("NoPaint");
    tmpl.set_health(100.0);
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(tmpl, ObjectId(10), Team::GLA);
    o.health.current = 100.0;
    o.health.maximum = 100.0;
    // C++ default OBJECT_STATUS_NONE: no paint.
    let dead = o.take_damage_from_typed(200.0, None, DamageType::Status, 0, &mut health_events);
    assert!(!dead);
    assert!((o.health.current - 100.0).abs() < 1e-3);
    assert!(!o.is_faerie_fire());
}

#[test]
fn most_percent_ready_between_shots_progresses() {
    let mut _attack_output = crate::game_logic::host_attack_log::AttackEvents::default();

    let mut shot_combat = crate::game_logic::combat::CombatSystem::new();
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    let mut tmpl = ThingTemplate::new("PctReady");
    tmpl.set_health(100.0);
    tmpl.add_kind_of(KindOf::Infantry);
    let mut o = Object::new(tmpl.clone(), ObjectId(1), Team::USA);
    let tgt = Object::new(tmpl, ObjectId(2), Team::GLA);
    o.weapon = Some(Weapon {
        damage: 10.0,
        range: 100.0,
        reload_time: 1.0,
        last_fire_time: -100.0,
        ..Weapon::default()
    });
    assert_eq!(o.get_most_percent_ready_to_fire_any_weapon(0.0), 100);
    assert!(o.fire_at(
        tgt.id,
        1.0,
        0,
        &mut shot_combat,
        &mut _attack_output,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
    ));
    assert_eq!(o.weapon_fire_status, WeaponFireStatus::BetweenFiringShots);
    let mid = o.get_most_percent_ready_to_fire_any_weapon(1.5);
    assert!(mid > 0 && mid < 100, "mid={mid}");
    assert_eq!(o.get_most_percent_ready_to_fire_any_weapon(2.0), 100);
}

#[test]
fn ammo_pip_and_waypoint_weapon_helpers() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    ensure_host_weapon_store();
    const PIPS: &str = "AmericaJetRaptorMissileWeapon";
    const WAYPT: &str = "HuntWaypointFollowWeapon";
    let _ = gamelogic::weapon::with_weapon_store_mut(|store| {
        let mut pips = gamelogic::weapon::WeaponTemplate::new(PIPS.to_string());
        pips.is_shows_ammo_pips = true;
        store.add_weapon_template(pips);
        let mut waypt = gamelogic::weapon::WeaponTemplate::new(WAYPT.to_string());
        waypt.capable_of_following_waypoint = true;
        store.add_weapon_template(waypt);
    });
    let mut tmpl = ThingTemplate::new("Raptor");
    tmpl.primary_weapon_name = Some(PIPS.into());
    tmpl.secondary_weapon_name = Some(WAYPT.into());
    tmpl.set_health(100.0);
    tmpl.add_kind_of(KindOf::Aircraft);
    let mut o = Object::new(tmpl, ObjectId(3), Team::USA);
    o.weapon = Some(Weapon {
        damage: 50.0,
        range: 200.0,
        reload_time: 1.0,
        clip_size: 4,
        ammo: Some(2),
        ..Weapon::default()
    });
    o.secondary_weapon = Some(Weapon {
        damage: 100.0,
        range: 500.0,
        reload_time: 5.0,
        ..Weapon::default()
    });
    assert_eq!(o.get_ammo_pip_showing_info(), Some((4, 2)));
    assert_eq!(o.find_waypoint_following_capable_weapon_slot(), Some(1));
}

#[test]
fn weapon_status_sets_between_firing_model_condition() {
    let mut _attack_output = crate::game_logic::host_attack_log::AttackEvents::default();

    let mut shot_combat = crate::game_logic::combat::CombatSystem::new();
    use crate::game_logic::host_enum_table_residual::{
        MC_BIT_BETWEEN_FIRING_SHOTS_A, MC_BIT_PREATTACK_A,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    let mut tmpl = ThingTemplate::new("McFire");
    tmpl.set_health(100.0);
    tmpl.add_kind_of(KindOf::Infantry);
    tmpl.add_kind_of(KindOf::Attackable);
    let mut atk = Object::new(tmpl.clone(), ObjectId(1), Team::USA);
    let tgt = Object::new(tmpl, ObjectId(2), Team::GLA);
    atk.weapon = Some(Weapon {
        damage: 10.0,
        range: 100.0,
        reload_time: 1.0,
        last_fire_time: -100.0,
        ..Weapon::default()
    });
    assert!(atk.fire_at(
        tgt.id,
        1.0,
        0,
        &mut shot_combat,
        &mut _attack_output,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
    ));
    assert_eq!(atk.weapon_fire_status, WeaponFireStatus::BetweenFiringShots);
    assert_ne!(
        atk.model_condition_bits & (1u128 << MC_BIT_BETWEEN_FIRING_SHOTS_A),
        0
    );
    atk.pre_attack_ready_at = 5.0;
    atk.refresh_weapon_fire_status(4.0);
    assert_eq!(atk.weapon_fire_status, WeaponFireStatus::PreAttack);
    assert_ne!(atk.model_condition_bits & (1u128 << MC_BIT_PREATTACK_A), 0);
}

#[test]
fn weapon_fire_status_between_shots_after_fire() {
    let mut _attack_output = crate::game_logic::host_attack_log::AttackEvents::default();

    let mut shot_combat = crate::game_logic::combat::CombatSystem::new();
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    let mut tmpl = ThingTemplate::new("StatusW");
    tmpl.set_health(100.0);
    tmpl.add_kind_of(KindOf::Infantry);
    tmpl.add_kind_of(KindOf::Attackable);
    let mut atk = Object::new(tmpl.clone(), ObjectId(1), Team::USA);
    let tgt = Object::new(tmpl, ObjectId(2), Team::GLA);
    atk.weapon = Some(Weapon {
        damage: 10.0,
        range: 100.0,
        reload_time: 1.0,
        last_fire_time: -100.0,
        ..Weapon::default()
    });
    assert_eq!(atk.weapon_fire_status, WeaponFireStatus::ReadyToFire);
    assert!(atk.fire_at(
        tgt.id,
        1.0,
        0,
        &mut shot_combat,
        &mut _attack_output,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
    ));
    assert_eq!(atk.weapon_fire_status, WeaponFireStatus::BetweenFiringShots);
    atk.refresh_weapon_fire_status(2.0);
    assert_eq!(atk.weapon_fire_status, WeaponFireStatus::ReadyToFire);
}

#[test]
fn can_fire_honors_weapon_bonus_rof() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    let mut tmpl = ThingTemplate::new("RofCan");
    tmpl.set_health(100.0);
    tmpl.add_kind_of(KindOf::Infantry);
    tmpl.add_kind_of(KindOf::Attackable);
    let mut o = Object::new(tmpl, ObjectId(1), Team::USA);
    o.weapon = Some(Weapon {
        damage: 10.0,
        range: 100.0,
        reload_time: 1.0,
        last_fire_time: 0.0,
        ..Weapon::default()
    });
    // Base: not ready at t=0.5
    assert!(!o.can_fire(0.5));
    // With 2x ROF, effective reload = 0.5 → ready at t=0.5
    o.weapon_bonus_enthusiastic = true;
    // Enthusiastic mult is typically >1; if not, force via horde path.
    let (_, _, rof, _, _) = o.weapon_bonus_fields();
    assert!(rof > 1.0, "expected ROF bonus mult, got {rof}");
    let need = 1.0 / rof;
    assert!(
        o.can_fire(need + 1e-4),
        "can_fire should honor ROF bonus at t={}",
        need
    );
    assert!(!o.can_fire(need - 0.05));
}

#[test]
fn max_shots_to_fire_blocks_after_budget() {
    let mut _attack_output = crate::game_logic::host_attack_log::AttackEvents::default();

    let mut shot_combat = crate::game_logic::combat::CombatSystem::new();
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    use glam::Vec3;
    let mut tmpl = ThingTemplate::new("MaxShot");
    tmpl.set_health(100.0);
    tmpl.add_kind_of(KindOf::Infantry);
    tmpl.add_kind_of(KindOf::Attackable);
    let mut atk = Object::new(tmpl.clone(), ObjectId(1), Team::USA);
    let tgt = Object::new(tmpl, ObjectId(2), Team::GLA);
    atk.set_position(Vec3::ZERO);
    atk.weapon = Some(Weapon {
        damage: 10.0,
        range: 100.0,
        reload_time: 0.0,
        last_fire_time: -100.0,
        pre_attack_delay: 0.0,
        projectile_speed: 999_000.0,
        ..Weapon::default()
    });
    atk.set_max_shots_to_fire(2);
    assert!(atk.fire_at(
        tgt.id,
        1.0,
        0,
        &mut shot_combat,
        &mut _attack_output,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
    ));
    assert_eq!(atk.max_shots_to_fire, 1);
    assert!(atk.fire_at(
        tgt.id,
        2.0,
        0,
        &mut shot_combat,
        &mut _attack_output,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
    ));
    assert_eq!(atk.max_shots_to_fire, 0);
    assert!(!atk.fire_at(
        tgt.id,
        3.0,
        0,
        &mut shot_combat,
        &mut _attack_output,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
    ));
    atk.set_max_shots_to_fire(-1);
    assert!(atk.fire_at(
        tgt.id,
        4.0,
        0,
        &mut shot_combat,
        &mut _attack_output,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
    ));
    assert_eq!(atk.max_shots_to_fire, -1);
}

#[test]
fn leech_range_waives_max_in_is_within_attack_range() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut atk_t = ThingTemplate::new("LR_A");
    atk_t.add_kind_of(KindOf::Vehicle);
    atk_t.set_health(100.0);
    let mut vic_t = ThingTemplate::new("LR_V");
    vic_t.add_kind_of(KindOf::Infantry);
    vic_t.set_health(50.0);
    let mut atk = Object::new(atk_t, ObjectId(1), Team::USA);
    let mut vic = Object::new(vic_t, ObjectId(2), Team::GLA);
    atk.set_position(Vec3::ZERO);
    vic.set_position(Vec3::new(500.0, 0.0, 0.0)); // far beyond weapon range
    atk.weapon = Some(Weapon {
        damage: 10.0,
        range: 100.0,
        min_range: 0.0,
        ..Weapon::default()
    });
    // Force leech template name path: set flags directly (activate needs name peel).
    assert!(!atk.is_within_attack_range(&vic));
    atk.leech_range_active_primary = true;
    assert!(
        atk.is_within_attack_range(&vic),
        "leech must waive max range once active"
    );
    // Min range still blocks under leech.
    atk.weapon.as_mut().unwrap().min_range = 600.0;
    assert!(
        !atk.is_within_attack_range(&vic),
        "min range still enforced with leech"
    );
}

#[test]
fn force_reload_when_idle_refills_clip() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("AR_V");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(100.0);
    logic.templates.insert("AR_V".to_string(), tpl);
    let id = logic.create_object("AR_V", Team::USA, Vec3::ZERO).unwrap();
    {
        let a = logic.host_object_mut(id).unwrap();
        a.weapon = Some(Weapon {
            damage: 10.0,
            range: 100.0,
            reload_time: 0.5,
            clip_size: 4,
            ammo: Some(1), // partial
            ..Weapon::default()
        });
        a.auto_reload_when_idle_frames = 15;
        a.stamp_auto_reload_when_idle(100);
        assert_eq!(a.frame_to_force_reload, 115);
        a.tick_force_reload_when_idle(114);
        assert_eq!(a.weapon.as_ref().unwrap().ammo, Some(1));
        a.tick_force_reload_when_idle(115);
        assert_eq!(
            a.weapon.as_ref().unwrap().ammo,
            Some(4),
            "idle force reload refills clip"
        );
        assert_eq!(a.frame_to_force_reload, 0);
    }
}

#[test]
fn continuous_fire_coasts_down_after_idle() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("CFC_V");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(100.0);
    logic.templates.insert("CFC_V".to_string(), tpl);
    let id = logic.create_object("CFC_V", Team::USA, Vec3::ZERO).unwrap();
    let tgt = ObjectId(7);
    let mut loop_events =
        crate::game_logic::host_fire_sound_loop_log::FireSoundLoopEvents::default();
    {
        let (a, health_events) = logic.host_object_and_health_events_mut(id).unwrap();
        a.continuous_fire_one_shots = 1;
        a.continuous_fire_two_shots = 4;
        a.continuous_fire_coast_frames = 10;
        a.record_shot_at_target(Some(tgt), 100, 100);
        a.record_shot_at_target(Some(tgt), 100, 100);
        assert_eq!(a.continuous_fire_level, 1);
        a.stamp_continuous_fire_coast(100);
        assert_eq!(a.continuous_fire_coast_until_frame, 110);
        a.tick_continuous_fire_coast(
            109,
            &mut loop_events,
            health_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        assert_eq!(a.continuous_fire_level, 1);
        // C++ FiringTracker.cpp:189: strict `now > m_frameToStartCooldown` —
        // the deadline frame itself is still cool-free; cool one frame later.
        a.tick_continuous_fire_coast(
            110,
            &mut loop_events,
            health_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        assert_eq!(a.continuous_fire_level, 1);
        a.tick_continuous_fire_coast(
            111,
            &mut loop_events,
            health_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        assert_eq!(a.continuous_fire_level, 0);
        assert_eq!(a.consecutive_shots_at_target, 0);
        let (_, _, rof, _, _) = a.weapon_bonus_fields();
        assert!((rof - 1.0).abs() < 0.01);
    }
}

#[test]
fn continuous_fire_mean_rof_after_threshold() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("CF_V");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(100.0);
    logic.templates.insert("CF_V".to_string(), tpl);
    let id = logic.create_object("CF_V", Team::USA, Vec3::ZERO).unwrap();
    let tgt = ObjectId(42);
    {
        let a = logic.host_object_mut(id).unwrap();
        a.continuous_fire_one_shots = 2;
        a.continuous_fire_two_shots = 5;
        assert_eq!(a.continuous_fire_level, 0);
        a.record_shot_at_target(Some(tgt), 100, 100);
        a.record_shot_at_target(Some(tgt), 100, 100);
        assert_eq!(a.continuous_fire_level, 0); // need consecutive > 2
        a.record_shot_at_target(Some(tgt), 100, 100);
        assert_eq!(a.continuous_fire_level, 1);
        let (_, _, rof, _, _) = a.weapon_bonus_fields();
        assert!((rof - 2.0).abs() < 0.01, "MEAN ROF 200% got {rof}");
        for _ in 0..3 {
            a.record_shot_at_target(Some(tgt), 100, 100);
        }
        assert_eq!(a.continuous_fire_level, 2);
        let (_, _, rof2, _, _) = a.weapon_bonus_fields();
        assert!((rof2 - 3.0).abs() < 0.01, "FAST ROF 300% got {rof2}");
    }
}

#[test]
fn continuous_fire_demotion_resets_consecutive_and_victim() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("CFD_V");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(100.0);
    logic.templates.insert("CFD_V".to_string(), tpl);
    let id = logic.create_object("CFD_V", Team::USA, Vec3::ZERO).unwrap();
    let tgt = ObjectId(9);
    {
        let a = logic.host_object_mut(id).unwrap();
        a.continuous_fire_one_shots = 2;
        a.continuous_fire_two_shots = 5;
        // Stale MEAN state from an earlier engagement.
        a.continuous_fire_level = 1;
        a.consecutive_shots_at_target = 4;
        a.consecutive_shot_target = Some(ObjectId(55));
        // Fresh engagement: count restarts at 1 < one → C++ coolDown demotes.
        a.record_shot_at_target(Some(tgt), 100, 100);
        assert_eq!(a.continuous_fire_level, 0);
        // FiringTracker.cpp:319-321: count and victim restart together.
        assert_eq!(a.consecutive_shots_at_target, 0);
        assert_eq!(a.consecutive_shot_target, None);
        assert_eq!(a.consecutive_shots_at_target, 0);
        assert_eq!(a.consecutive_shot_target.map_or(0, |id| id.0), 0);
        // Promotion keeps the count (C++ speedUp does not reset).
        a.record_shot_at_target(Some(tgt), 100, 100);
        a.record_shot_at_target(Some(tgt), 100, 100);
        a.record_shot_at_target(Some(tgt), 100, 100);
        assert_eq!(a.continuous_fire_level, 1);
        assert_eq!(a.consecutive_shots_at_target, 3);
        assert_eq!(a.consecutive_shot_target, Some(tgt));
        assert_eq!(a.consecutive_shots_at_target, 3);
        assert_eq!(a.consecutive_shot_target.map_or(0, |id| id.0), tgt.0);
    }
}

#[test]
fn specialized_lanes_keep_generic_bookkeeping_without_continuous_fire() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("CFL_V");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(100.0);
    logic.templates.insert("CFL_V".to_string(), tpl);
    let id = logic.create_object("CFL_V", Team::USA, Vec3::ZERO).unwrap();
    let tgt = ObjectId(11);
    {
        let a = logic.host_object_mut(id).unwrap();
        a.continuous_fire_one_shots = 2;
        a.continuous_fire_two_shots = 5;
        a.continuous_fire_coast_frames = 10;
        // Specialized-lane-owned state: coast armed while at base level —
        // the generic stamp zeroes exactly this.
        a.continuous_fire_coast_until_frame = 200;
        a.continuous_fire_level = 0;
        a.pre_attack_ready_at = 5.0;
        // The lane has already applied exactly one accepted tracker shot.
        a.consecutive_shot_target = Some(tgt);
        a.consecutive_shots_at_target = 1;
        a.finish_shot_after_specialized_tracker();
        // Generic derivation must not touch lane-owned state...
        assert_eq!(a.continuous_fire_coast_until_frame, 200);
        assert_eq!(a.continuous_fire_level, 0);
        assert_eq!(a.consecutive_shots_at_target, 1);
        assert_eq!(a.consecutive_shot_target, Some(tgt));
        // ...and post-shot rearm occurs without counting the shot again.
        assert_eq!(a.consecutive_shot_target, Some(tgt));
        assert_eq!(a.consecutive_shots_at_target, 1);
        assert_eq!(a.pre_attack_ready_at, 0.0);
    }
}

#[test]
fn construct_binds_continuous_fire_and_auto_reload_from_store() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    // C++ FiringTracker.cpp:112-136 / 101-104 — thresholds come from the
    // fired WeaponTemplate, bound onto the host Object at construct.
    crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
    const NAME: &str = "Hunt4GattlingGun";
    let _ = gamelogic::weapon::with_weapon_store_mut(|store| {
        let mut template = gamelogic::weapon::WeaponTemplate::new(NAME.to_string());
        template.primary_damage = 5.0;
        template.attack_range = 150.0;
        template.continuous_fire_one_shots_needed = 2;
        template.continuous_fire_two_shots_needed = 6;
        template.continuous_fire_coast_frames = 30;
        template.auto_reload_when_idle_frames = 183;
        store.add_weapon_template(template);
    });
    let mut tpl = ThingTemplate::new("Hunt4Gattling");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.set_primary_weapon_name(NAME);
    let object = Object::new(tpl, ObjectId(1), Team::USA);
    assert_eq!(object.continuous_fire_one_shots, 2);
    assert_eq!(object.continuous_fire_two_shots, 6);
    assert_eq!(object.continuous_fire_coast_frames, 30);
    assert_eq!(object.auto_reload_when_idle_frames, 183);
}

#[test]
fn take_scatter_table_offset_uses_store_pattern() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    // C++ Weapon.cpp:2584-2609 — fire picks an unused ScatterTarget.
    crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
    const NAME: &str = "Hunt4ScatterFireWeapon";
    let _ = gamelogic::weapon::with_weapon_store_mut(|store| {
        let mut template = gamelogic::weapon::WeaponTemplate::new(NAME.to_string());
        template.primary_damage = 10.0;
        template.attack_range = 100.0;
        template.scatter_target_scalar = 10.0;
        template.scatter_targets = vec![
            gamelogic::weapon::Coord2D { x: 1.0, y: 0.0 },
            gamelogic::weapon::Coord2D { x: 0.0, y: 1.0 },
        ];
        store.add_weapon_template(template);
    });
    let mut tpl = ThingTemplate::new("Hunt4ScatterUnit");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.set_primary_weapon_name(NAME);
    let mut object = Object::new(tpl, ObjectId(9), Team::USA);
    object.weapon = ThingTemplate::weapon_from_store(NAME);
    let offset = object
        .take_scatter_table_offset(0, Some(NAME))
        .expect("scatter table must offset fire");
    let mag = (offset.x * offset.x + offset.y * offset.y).sqrt();
    assert!(
        (mag - 10.0).abs() < 0.01,
        "scatter table * scalar must move aim off center, got {offset:?}"
    );
}

#[test]
fn fire_at_ex_faerie_fire_speeds_reload() {
    use crate::game_logic::host_avenger::FAERIE_FIRE_ROF_MULTIPLIER;
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("FF_ATK");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(100.0);
    logic.templates.insert("FF_ATK".to_string(), tpl);
    let atk = logic
        .create_object("FF_ATK", Team::USA, Vec3::ZERO)
        .unwrap();
    {
        let _ = logic.host_object_mut(atk).unwrap();
        let a = logic.objects.get_mut(&atk).unwrap();
        a.weapon = Some(Weapon {
            damage: 10.0,
            range: 200.0,
            reload_time: 1.0,
            last_fire_time: -100.0, // never-fired residual
            ..Weapon::default()
        });
        // First shot at t=0
        assert!(a.fire_at_ex(
            ObjectId(99),
            0.0,
            false,
            true,
            logic.frame,
            &mut logic.combat_system,
            &mut logic.attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        ));
        // Without faerie, not ready at 0.7 (needs full 1.0s)
        assert!(!a.fire_at_ex(
            ObjectId(99),
            0.7,
            false,
            false,
            logic.frame,
            &mut logic.combat_system,
            &mut logic.attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        ));
        // With faerie ROF 150%, ready at 0.7 (effective reload ~0.667)
        assert!(
            a.fire_at_ex(
                ObjectId(99),
                0.7,
                false,
                true,
                logic.frame,
                &mut logic.combat_system,
                &mut logic.attack_events,
                &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
            ),
            "TARGET_FAERIE_FIRE should ready at ~0.667s reload"
        );
        assert!((FAERIE_FIRE_ROF_MULTIPLIER - 1.5).abs() < 0.001);
    }
}

#[test]
fn weapon_bonus_fields_stack_rof_and_damage() {
    use crate::game_logic::host_propaganda::ENTHUSIASTIC_RATE_OF_FIRE_MULT;
    use crate::game_logic::host_red_guard::INFANTRY_HORDE_ROF_MULT;
    use crate::game_logic::host_strategy_center::BOMBARDMENT_DAMAGE_MULT;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("WB_V");
    tpl.add_kind_of(KindOf::Infantry);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(100.0);
    logic.templates.insert("WB_V".to_string(), tpl);
    let id = logic.create_object("WB_V", Team::USA, Vec3::ZERO).unwrap();
    {
        let o = logic.host_object_mut(id).unwrap();
        o.weapon_bonus_enthusiastic = true;
        o.weapon_bonus_horde = true;
        o.weapon_bonus_battle_plan_bombardment = true;
        let (dmg, _range, rof, _, _) = o.weapon_bonus_fields();
        // C++ appendBonuses: 1.0 + (1.25-1) + (1.5-1) = 1.75, not 1.25*1.5.
        assert!(
            (rof - (ENTHUSIASTIC_RATE_OF_FIRE_MULT + INFANTRY_HORDE_ROF_MULT - 1.0)).abs() < 0.001,
            "ROF stacks propaganda+horde additively got {rof}"
        );
        assert!(
            (dmg - BOMBARDMENT_DAMAGE_MULT).abs() < 0.001,
            "damage includes bombardment got {dmg}"
        );
        assert!((o.effective_weapon_reload(2.0) - 2.0 / rof).abs() < 0.001);
        assert!((o.effective_weapon_damage(10.0) - 10.0 * dmg).abs() < 0.001);
    }
}

#[test]
fn weapon_bonus_fields_apply_player_upgrade_and_fanaticism() {
    use crate::game_logic::host_red_guard::{
        INFANTRY_FANATICISM_ROF_MULT, INFANTRY_NATIONALISM_ROF_MULT,
    };

    let mut obj = make_test_object();
    let (dmg0, _, rof0, _, _) = obj.weapon_bonus_fields();
    assert!((dmg0 - 1.0).abs() < 0.001);
    assert!((rof0 - 1.0).abs() < 0.001);

    obj.weapon_bonus_player_upgrade = true;
    let (dmg, _, _, _, _) = obj.weapon_bonus_fields();
    assert!(
        (dmg - 1.25).abs() < 0.001,
        "PLAYER_UPGRADE DAMAGE 125% got {dmg}"
    );
    assert!((obj.effective_weapon_damage(80.0) - 100.0).abs() < 0.001);

    obj.weapon_bonus_nationalism = true;
    obj.weapon_bonus_fanaticism = true;
    let (_, _, rof, _, _) = obj.weapon_bonus_fields();
    // C++ appendBonuses: 1.0 + (1.25-1) + (1.25-1) = 1.50, not 1.25*1.25.
    let expected = INFANTRY_NATIONALISM_ROF_MULT + INFANTRY_FANATICISM_ROF_MULT - 1.0;
    assert!(
        (rof - expected).abs() < 0.001,
        "FANATICISM stacks on NATIONALISM additively got {rof} expected {expected}"
    );
}
