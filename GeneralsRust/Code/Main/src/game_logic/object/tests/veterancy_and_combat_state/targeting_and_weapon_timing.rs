use super::*;

#[test]
fn stop_attack_clears_force_attack_and_targets() {
    let mut _attack_output = crate::game_logic::host_attack_log::AttackEvents::default();

    let mut object = make_test_object();
    object.set_target(
        Some(ObjectId(99)),
        &mut _attack_output,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    object.clear_target(&mut crate::game_logic::ObjectAudio::record(&mut Vec::new()));
    assert_eq!(object.ai_state(), AIState::Attacking);
    object.set_target(
        Some(ObjectId(99)),
        &mut _attack_output,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    object.set_force_attack(true);
    object.set_target_location(
        Some(Vec3::new(1.0, 0.0, 2.0)),
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    assert!(object.set_weapon_lock(0, WeaponLockType::LockedTemporarily));
    object.stop_attack(&mut crate::game_logic::ObjectAudio::record(&mut Vec::new()));

    assert_eq!(object.ai_state(), AIState::Attacking);
    assert!(object.target.is_none());
    assert!(object.target_location.is_none());
    assert!(!object.force_attack);
    assert!(!object.status.attacking);
    assert_eq!(object.weapon_lock_type, WeaponLockType::NotLocked);
}

#[test]
fn temporary_tertiary_lock_releases_after_its_auto_clip_reloads() {
    let mut _attack_output = crate::game_logic::host_attack_log::AttackEvents::default();

    let mut shot_combat = crate::game_logic::combat::CombatSystem::new();
    let mut object = make_test_object();
    object.tertiary_weapon = Some(Weapon {
        damage: 73.0,
        range: 100.0,
        reload_time: 0.1,
        clip_reload_time: 1.0,
        clip_size: 1,
        ammo: Some(1),
        last_fire_time: -10.0,
        ..Weapon::default()
    });
    assert!(object.set_weapon_lock(2, WeaponLockType::LockedTemporarily));

    assert!(object.fire_at(
        ObjectId(99),
        1.0,
        0,
        &mut shot_combat,
        &mut _attack_output,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
    ));

    assert_eq!(object.last_fire_slot, 2);
    assert_eq!(
        object
            .tertiary_weapon
            .as_ref()
            .and_then(|weapon| weapon.ammo),
        Some(0),
        "the host keeps an auto-reloading clip empty until its reload window finishes"
    );
    assert_eq!(object.weapon_lock_type, WeaponLockType::NotLocked);
    assert_eq!(
        object.active_weapon_slot, 0,
        "expired temporary tertiary selection must not turn into an automatic tertiary choice"
    );
}

#[test]
fn setting_target_location_clears_object_target() {
    let mut _attack_output = crate::game_logic::host_attack_log::AttackEvents::default();

    let mut object = make_test_object();
    object.set_target(
        Some(ObjectId(77)),
        &mut _attack_output,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    object.set_target_location(
        Some(Vec3::new(10.0, 0.0, 10.0)),
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );

    assert!(object.target.is_none());
    assert!(object.target_location.is_some());
    assert!(object.status.attacking);
}

#[test]
fn effectively_stealthed_blocks_enemy_visibility_and_targeting() {
    let mut stealthed = make_test_object();
    stealthed.team = Team::USA;
    stealthed.status.stealthed = true;
    stealthed.status.detected = false;
    stealthed.thing.template.add_kind_of(KindOf::Attackable);

    assert!(stealthed.is_effectively_stealthed());
    assert!(stealthed.is_visible_to_team(Team::USA));
    assert!(!stealthed.is_visible_to_team(Team::China));
    assert!(!stealthed.is_targetable_by_enemy_of(Team::China));

    stealthed.status.detected = true;
    assert!(!stealthed.is_effectively_stealthed());
    assert!(stealthed.is_visible_to_team(Team::China));
    assert!(stealthed.is_targetable_by_enemy_of(Team::China));
}

#[test]
fn targetable_by_enemy_honors_weaponset_unattackable_and_masked_overrides() {
    let mut object = make_test_object();
    object.team = Team::USA;
    object.thing.template.add_kind_of(KindOf::Attackable);

    object.thing.template.add_kind_of(KindOf::Unattackable);
    assert!(!object.is_targetable_by_enemy_of(Team::China));

    object.thing.template.kind_of.remove(&KindOf::Unattackable);
    object.status.masked = true;
    assert!(!object.is_targetable_by_enemy_of(Team::China));

    object.status.masked = false;
    assert!(object.is_targetable_by_enemy_of(Team::China));
}

#[test]
fn fire_at_breaks_stealth_when_forbidden_while_attacking() {
    let mut _attack_output = crate::game_logic::host_attack_log::AttackEvents::default();

    let mut shot_combat = crate::game_logic::combat::CombatSystem::new();
    let mut object = make_test_object();
    object.status.stealthed = true;
    object.stealth_breaks_on_attack = true;
    object.weapon = Some(Weapon {
        damage: 100.0,
        range: 100.0,
        reload_time: 0.5,
        last_fire_time: -1.0,
        ..Weapon::default()
    });
    assert!(object.fire_at(
        ObjectId(2),
        0.0,
        0,
        &mut shot_combat,
        &mut _attack_output,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
    ));
    assert!(!object.status.stealthed);
    assert!(!object.status.detected);
}

#[test]
fn can_target_rejects_undetected_stealthed_enemy() {
    let mut attacker = make_test_object();
    attacker.weapon = Some(Weapon {
        damage: 10.0,
        range: 100.0,
        ..Weapon::default()
    });

    let mut target = make_test_object();
    target.id = ObjectId(2);
    target.team = Team::China;
    target.status.stealthed = true;
    target.status.detected = false;
    target.set_position(Vec3::new(5.0, 0.0, 0.0));

    assert!(!attacker.can_target(&target));

    target.status.detected = true;
    assert!(attacker.can_target(&target));
}

#[test]
fn can_target_rejects_weaponset_unattackable_and_masked_overrides() {
    let mut attacker = make_test_object();
    attacker.weapon = Some(Weapon {
        damage: 10.0,
        range: 100.0,
        can_target_ground: true,
        ..Weapon::default()
    });

    let mut target = make_test_object();
    target.id = ObjectId(2);
    target.team = Team::China;
    target.set_position(Vec3::new(5.0, 0.0, 0.0));
    target.thing.template.add_kind_of(KindOf::Unattackable);
    assert!(!attacker.can_target(&target));

    target.thing.template.kind_of.remove(&KindOf::Unattackable);
    target.status.masked = true;
    assert!(!attacker.can_target(&target));

    target.status.masked = false;
    assert!(attacker.can_target(&target));
}

#[test]
fn clip_ammo_forces_clip_reload_gap() {
    use crate::game_logic::Weapon;
    let mut w = Weapon {
        damage: 10.0,
        range: 100.0,
        reload_time: 0.1, // between shots
        ammo: Some(2),
        clip_size: 2,
        clip_reload_time: 2.0, // long clip reload
        last_fire_time: -100.0,
        ..Weapon::default()
    };
    let t0 = 10.0;
    assert!(Object::weapon_ready(&w, t0));
    Object::consume_ammo_on_fire(&mut w, t0);
    assert_eq!(w.ammo, Some(1));
    // Between-shot: ready after 0.1
    assert!(!Object::weapon_ready(&w, t0 + 0.05));
    assert!(Object::weapon_ready(&w, t0 + 0.11));
    Object::consume_ammo_on_fire(&mut w, t0 + 0.11);
    assert_eq!(w.ammo, Some(0));
    // Clip empty: not ready until clip_reload (~2.0 from last fire adjusted)
    assert!(!Object::weapon_ready(&w, t0 + 0.11 + 0.5));
    assert!(
        Object::weapon_ready(&w, t0 + 0.11 + 2.0),
        "clip reload must elapse before next ready"
    );
    Object::consume_ammo_on_fire(&mut w, t0 + 0.11 + 2.0);
    assert_eq!(w.ammo, Some(1), "refill then spend one");
}

#[test]
fn pre_attack_delay_blocks_first_shot() {
    let mut _attack_output = crate::game_logic::host_attack_log::AttackEvents::default();

    let mut shot_combat = crate::game_logic::combat::CombatSystem::new();
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    use glam::Vec3;
    let mut tmpl = ThingTemplate::new("PreAtk");
    tmpl.set_health(100.0);
    tmpl.add_kind_of(KindOf::Infantry);
    tmpl.add_kind_of(KindOf::Attackable);
    let mut atk = Object::new(tmpl, ObjectId(1), Team::USA);
    atk.set_position(Vec3::ZERO);
    atk.weapon = Some(Weapon {
        damage: 25.0,
        range: 100.0,
        reload_time: 0.0,
        last_fire_time: -100.0,
        pre_attack_delay: 1.0,
        ammo: Some(5),
        clip_size: 5,
        ..Weapon::default()
    });
    let tgt_id = ObjectId(2);

    // First call starts wind-up, must not fire (ammo unchanged).
    assert!(!atk.fire_at(
        tgt_id,
        10.0,
        0,
        &mut shot_combat,
        &mut _attack_output,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
    ));
    assert_eq!(atk.pre_attack_target, Some(tgt_id));
    assert!((atk.pre_attack_ready_at - 11.0).abs() < 1e-4);
    assert_eq!(atk.weapon.as_ref().unwrap().ammo, Some(5));

    // Still winding up.
    assert!(!atk.fire_at(
        tgt_id,
        10.5,
        0,
        &mut shot_combat,
        &mut _attack_output,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
    ));
    assert_eq!(atk.weapon.as_ref().unwrap().ammo, Some(5));

    // After delay, fires and consumes ammo.
    assert!(atk.fire_at(
        tgt_id,
        11.0,
        0,
        &mut shot_combat,
        &mut _attack_output,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
    ));
    assert_eq!(atk.weapon.as_ref().unwrap().ammo, Some(4));
}

#[test]
fn pre_attack_resets_on_new_target() {
    let mut _attack_output = crate::game_logic::host_attack_log::AttackEvents::default();

    let mut shot_combat = crate::game_logic::combat::CombatSystem::new();
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    let mut tmpl = ThingTemplate::new("PreAtk2");
    tmpl.add_kind_of(KindOf::Infantry);
    tmpl.add_kind_of(KindOf::Attackable);
    let mut atk = Object::new(tmpl, ObjectId(3), Team::USA);
    atk.weapon = Some(Weapon {
        damage: 10.0,
        range: 50.0,
        reload_time: 0.0,
        last_fire_time: -100.0,
        pre_attack_delay: 2.0,
        ..Weapon::default()
    });
    assert!(!atk.fire_at(
        ObjectId(10),
        5.0,
        0,
        &mut shot_combat,
        &mut _attack_output,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
    ));
    assert!((atk.pre_attack_ready_at - 7.0).abs() < 1e-4);
    // Switch target restarts delay.
    assert!(!atk.fire_at(
        ObjectId(11),
        6.0,
        0,
        &mut shot_combat,
        &mut _attack_output,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
    ));
    assert_eq!(atk.pre_attack_target, Some(ObjectId(11)));
    assert!((atk.pre_attack_ready_at - 8.0).abs() < 1e-4);
}

#[test]
fn small_arms_reduced_on_tank_armor_residual() {
    let mut health_events = crate::game_logic::HostHealthEvents::default();

    use crate::game_logic::combat::DamageType;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut tmpl = ThingTemplate::new("ArmorTank");
    tmpl.set_health(1000.0);
    tmpl.add_kind_of(KindOf::Vehicle);
    tmpl.add_kind_of(KindOf::Attackable);
    let mut tank = Object::new(tmpl, ObjectId(70), Team::USA);
    let hp0 = tank.health.current;
    // TankArmor SmallArms residual is 0.25 → 100 * 0.25 = 25
    tank.take_damage_from_typed(100.0, None, DamageType::Bullet, 0, &mut health_events);
    let dealt = hp0 - tank.health.current;
    assert!(
        (dealt - 25.0).abs() < 1.0,
        "expected ~25 small-arms on tank, got {dealt}"
    );
}

#[test]
fn laser_half_on_human_armor_residual() {
    let mut health_events = crate::game_logic::HostHealthEvents::default();

    use crate::game_logic::combat::DamageType;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut tmpl = ThingTemplate::new("ArmorInf");
    tmpl.set_health(500.0);
    tmpl.add_kind_of(KindOf::Infantry);
    tmpl.add_kind_of(KindOf::Attackable);
    let mut inf = Object::new(tmpl, ObjectId(71), Team::GLA);
    let hp0 = inf.health.current;
    // HumanArmor Laser residual 0.5 → 100 * 0.5 = 50
    inf.take_damage_from_typed(100.0, None, DamageType::Laser, 0, &mut health_events);
    let dealt = hp0 - inf.health.current;
    assert!(
        (dealt - 50.0).abs() < 1.0,
        "expected ~50 laser on infantry, got {dealt}"
    );
}

#[test]
fn flame_kill_sets_burned_death_type() {
    let mut health_events = crate::game_logic::HostHealthEvents::default();

    use crate::game_logic::combat::DamageType;
    use crate::game_logic::host_usa_pilot::HostDeathType;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut tmpl = ThingTemplate::new("BurnMe");
    tmpl.set_health(50.0);
    tmpl.add_kind_of(KindOf::Infantry);
    let mut o = Object::new(tmpl, ObjectId(80), Team::GLA);
    let dead = o.take_damage_from_typed_death(
        999.0,
        None,
        DamageType::Flame,
        HostDeathType::Burned,
        0,
        &mut health_events,
    );
    assert!(dead);
    assert_eq!(o.status.death_type, HostDeathType::Burned);
}

#[test]
fn resolve_death_type_from_damage_class() {
    use crate::game_logic::combat::DamageType;
    use crate::game_logic::host_armor_residual::resolve_host_death_type;
    use crate::game_logic::host_usa_pilot::HostDeathType;
    assert_eq!(
        resolve_host_death_type(None, DamageType::Explosive),
        HostDeathType::Exploded
    );
    assert_eq!(
        resolve_host_death_type(None, DamageType::Laser),
        HostDeathType::Lasered
    );
    assert_eq!(
        resolve_host_death_type(None, DamageType::Toxin),
        HostDeathType::Poisoned
    );
}

#[test]
fn garrison_range_bonus_extends_is_within_attack_range() {
    use crate::game_logic::{AIState, KindOf, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut atk_t = ThingTemplate::new("GR_A");
    atk_t.add_kind_of(KindOf::Infantry);
    atk_t.set_health(100.0);
    let mut vic_t = ThingTemplate::new("GR_V");
    vic_t.add_kind_of(KindOf::Vehicle);
    vic_t.set_health(100.0);
    let mut atk = Object::new(atk_t, ObjectId(1), Team::USA);
    let mut vic = Object::new(vic_t, ObjectId(2), Team::GLA);
    atk.set_position(Vec3::ZERO);
    // 120 units away; weapon range 100 — out without garrison, in with 133%.
    vic.set_position(Vec3::new(120.0, 0.0, 0.0));
    atk.weapon = Some(Weapon {
        damage: 10.0,
        range: 100.0,
        ..Weapon::default()
    });
    assert!(!atk.is_within_attack_range(&vic));
    atk.contained_by = Some(ObjectId(99));
    atk.set_ai_state(
        AIState::Docked,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    assert!(
        !atk.is_within_attack_range(&vic),
        "transport / hospital contain must not grant garrison RANGE"
    );
    atk.set_ai_state(
        AIState::Garrisoned,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
    );
    assert!(
        atk.is_within_attack_range(&vic),
        "garrison RANGE 133% should cover 120 with base 100"
    );
}

#[test]
fn fire_at_scales_secondary_damage_with_damage_bonus() {
    use crate::assets::ini_template_loader::{
        WeaponDefinitionLoadType, parse_weapon_definitions_ini_into,
    };
    use crate::game_logic::host_unit_training::VETERANCY_DAMAGE_BONUS_VETERAN;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut definitions = std::collections::HashMap::new();
    // This is an authored generic secondary-ring contract, not retail
    // ScorpionTankGun (Weapon.ini128876 has no SecondaryDamage field).
    assert_eq!(
        parse_weapon_definitions_ini_into(
            &mut definitions,
            "Weapon OwnedSecondaryDamageBonus\nPrimaryDamage = 25\nPrimaryDamageRadius = 5\nSecondaryDamage = 25\nSecondaryDamageRadius = 10\nAttackRange = 100\nWeaponSpeed = 30000000\nProjectileObject = NONE\nDamageType = ARMOR_PIERCING\nDeathType = NORMAL\nDelayBetweenShots = 1000\nClipSize = 0\nClipReloadTime = 0\nEnd\n",
            WeaponDefinitionLoadType::Normal
        ),
        1
    );
    let native = definitions.remove("ownedsecondarydamagebonus").unwrap();
    let mut template = ThingTemplate::new("OwnedSecondaryBonusSource");
    template.add_kind_of(KindOf::Vehicle);
    template.add_kind_of(KindOf::Attackable);
    template.set_health(100.0);
    template.set_primary_weapon_name(&native.name);
    template.bind_native_weapon_row([Some(&native), None, None]);
    let mut attacker = Object::new_with_logic_frame(template, ObjectId(1), Team::GLA, 0);
    attacker.weapon = ThingTemplate::weapon_from_template_at(&native, 0);
    attacker.weapon_bonus_veteran = true;
    let mut shot_combat = crate::game_logic::combat::CombatSystem::new();
    let mut attack_output = crate::game_logic::host_attack_log::AttackEvents::default();
    // Wait through the actual authored one-second reload window, instead of
    // fabricating a mutable cached weapon's old last-fire timestamp.
    assert!(attacker.fire_at(
        ObjectId(2),
        1.0,
        30,
        &mut shot_combat,
        &mut attack_output,
        &mut crate::game_logic::ObjectAudio::record(&mut Vec::new())
    ));
    assert_eq!(
        crate::game_logic::combat::pending_projectile_queue_len_for_test(&shot_combat),
        1
    );
    let stamped =
        crate::game_logic::combat::last_pending_projectile_secondary_damage_for_test(&shot_combat)
            .expect("queued splash");
    let expected = native.secondary_damage * VETERANCY_DAMAGE_BONUS_VETERAN;
    assert_eq!(expected, 27.5);
    assert!(
        (stamped - expected).abs() < 0.01,
        "secondary ring must scale with DAMAGE bonus ({stamped} vs {expected})"
    );
}

#[test]
fn barrel_advances_after_shots_per_barrel() {
    let vt = ThingTemplate::new("QuadCannon");
    let mut o = Object::new(vt, ObjectId(1), Team::USA);
    o.weapon = Some(Weapon::default());
    o.weapon_barrel_states[0] = WeaponBarrelState::new(2, 4, None);
    o.advance_weapon_barrel_after_shot(0);
    let primary = o
        .weapon_barrel_state_for_slot(0)
        .expect("primary barrel state");
    assert_eq!(primary.current_barrel, 0);
    assert_eq!(primary.shots_left_on_barrel, 1);
    o.advance_weapon_barrel_after_shot(0);
    let primary = o
        .weapon_barrel_state_for_slot(0)
        .expect("primary barrel state");
    assert_eq!(primary.current_barrel, 1);
    assert_eq!(primary.shots_left_on_barrel, 2);
    for _ in 0..6 {
        o.advance_weapon_barrel_after_shot(0);
    }
    // C++ retains the end-of-cycle cursor until the *next* discharge compares
    // it against the current Drawable count. Eight shots therefore leave the
    // transient value 4, not a modulo-wrapped zero.
    assert_eq!(
        o.weapon_barrel_state_for_slot(0)
            .expect("primary barrel state")
            .current_barrel,
        4
    );
    assert_eq!(
        o.fired_barrel_for_slot(0),
        Some(0),
        "the next C++ pre-fire >= barrelCount check resets the transient cursor"
    );
}

#[test]
fn barrel_cursor_resets_on_next_fire_after_current_draw_topology_shrinks() {
    let template = ThingTemplate::new("TopologyShrinkTank");
    let mut object = Object::new(template, ObjectId(1), Team::USA);
    object.weapon = Some(Weapon::default());
    object.weapon_barrel_states[0] = WeaponBarrelState::new(1, 5, None);
    object.weapon_barrel_states[0].current_barrel = 4;

    // A new ModelCondition state has fewer valid WeaponBarrelInfo entries.
    // C++ does not modulo the old cursor when that state is selected.
    assert!(object.set_weapon_barrel_count_for_slot(0, 3));
    assert_eq!(
        object
            .weapon_barrel_state_for_slot(0)
            .expect("configured primary")
            .current_barrel,
        4
    );
    assert_eq!(object.fired_barrel_for_slot(0), Some(0));
}

#[test]
fn barrel_cursors_are_independent_per_weapon_set_slot() {
    let vt = ThingTemplate::new("IndependentBarrels");
    let mut o = Object::new(vt, ObjectId(1), Team::USA);
    o.weapon = Some(Weapon::default());
    o.secondary_weapon = Some(Weapon::default());
    o.weapon_barrel_states[0] = WeaponBarrelState::new(2, 3, None);
    o.weapon_barrel_states[1] = WeaponBarrelState::new(3, 2, None);

    o.advance_weapon_barrel_after_shot(0);
    o.advance_weapon_barrel_after_shot(0);

    let primary = o.weapon_barrel_state_for_slot(0).expect("primary state");
    assert_eq!(
        (primary.current_barrel, primary.shots_left_on_barrel),
        (1, 2)
    );
    let secondary = o.weapon_barrel_state_for_slot(1).expect("secondary state");
    assert_eq!(
        (secondary.current_barrel, secondary.shots_left_on_barrel),
        (0, 3),
        "PRIMARY must not rotate SECONDARY's independent Weapon cursor"
    );

    for _ in 0..3 {
        o.advance_weapon_barrel_after_shot(1);
    }
    let primary = o.weapon_barrel_state_for_slot(0).expect("primary state");
    assert_eq!(
        (primary.current_barrel, primary.shots_left_on_barrel),
        (1, 2)
    );
    let secondary = o.weapon_barrel_state_for_slot(1).expect("secondary state");
    assert_eq!(
        (secondary.current_barrel, secondary.shots_left_on_barrel),
        (1, 3)
    );
}

#[test]
fn direct_weapon_set_replacement_resets_only_its_own_barrel_cursor() {
    let vt = ThingTemplate::new("ExactSlotReplacement");
    let mut o = Object::new(vt, ObjectId(1), Team::USA);
    o.weapon = Some(Weapon::default());
    o.secondary_weapon = Some(Weapon::default());
    o.tertiary_weapon = Some(Weapon::default());
    o.weapon_barrel_states[0] = WeaponBarrelState::new(2, 4, None);
    o.weapon_barrel_states[1] = WeaponBarrelState::new(3, 3, None);
    o.weapon_barrel_states[2] = WeaponBarrelState::new(4, 2, None);
    o.weapon_barrel_states[0].current_barrel = 3;
    o.weapon_barrel_states[1].current_barrel = 2;
    o.weapon_barrel_states[2].current_barrel = 1;

    assert!(o.replace_weapon_set_slot(1, Some(Weapon::default())));
    assert_eq!(o.weapon_barrel_states[0].current_barrel, 3);
    assert_eq!(o.weapon_barrel_states[1].current_barrel, 0);
    assert_eq!(o.weapon_barrel_states[2].current_barrel, 1);
    assert!(!o.replace_weapon_set_slot(3, Some(Weapon::default())));
}

#[test]
fn authored_shots_per_barrel_configures_the_exact_active_slot() {
    let ini_content = r#"
Weapon __RustObjectSlotBarrelAuthored
  AttackRange = 100.0
  PrimaryDamage = 25.0
  ShotsPerBarrel = 3
End
"#;
    assert_eq!(
        crate::assets::ini_template_loader::register_weapons_from_ini_text(ini_content),
        1
    );

    let mut vt = ThingTemplate::new("AuthoredSlotBarrelObject");
    vt.set_primary_weapon_name("__RustObjectSlotBarrelAuthored");
    let mut o = Object::new(vt, ObjectId(1), Team::USA);
    o.weapon = Some(Weapon::default());

    assert_eq!(o.fired_barrel_for_slot(0), Some(0));
    let state = o.weapon_barrel_state_for_slot(0).expect("configured state");
    assert_eq!((state.shots_per_barrel, state.shots_left_on_barrel), (3, 3));

    o.advance_weapon_barrel_after_shot(0);
    assert_eq!(
        o.weapon_barrel_state_for_slot(0)
            .expect("configured state")
            .shots_left_on_barrel,
        2
    );
}

#[test]
fn restored_multi_barrel_cursor_waits_for_validated_topology() {
    let vt = ThingTemplate::new("DeferredBarrelTopology");
    let mut o = Object::new(vt, ObjectId(1), Team::USA);
    o.weapon_barrel_states[0] = WeaponBarrelState::new(3, 1, None);

    assert!(o.restore_weapon_barrel_runtime_for_slot(0, 2, 2));
    let before_topology = o.weapon_barrel_state_for_slot(0).expect("barrel state");
    assert_eq!(
        (
            before_topology.current_barrel,
            before_topology.shots_left_on_barrel
        ),
        (0, 3),
        "a one-barrel default must not truncate a saved multi-barrel cursor"
    );

    assert!(o.set_weapon_barrel_count_for_slot(0, 4));
    let after_topology = o.weapon_barrel_state_for_slot(0).expect("barrel state");
    assert_eq!(
        (
            after_topology.current_barrel,
            after_topology.shots_left_on_barrel
        ),
        (2, 2),
        "validated topology consumes the pending saved cursor exactly once"
    );
}

#[test]
fn live_fire_before_topology_never_replays_a_stale_restored_cursor() {
    let vt = ThingTemplate::new("DeferredBarrelTopologyLiveFire");
    let mut o = Object::new(vt, ObjectId(1), Team::USA);
    o.weapon = Some(Weapon::default());
    o.weapon_barrel_states[0] = WeaponBarrelState::new(3, 1, None);

    assert!(o.restore_weapon_barrel_runtime_for_slot(0, 2, 2));
    o.advance_weapon_barrel_after_shot(0);
    assert!(o.set_weapon_barrel_count_for_slot(0, 4));

    let state = o.weapon_barrel_state_for_slot(0).expect("barrel state");
    assert_eq!(
        (state.current_barrel, state.shots_left_on_barrel),
        (0, 2),
        "late topology must retain the real post-load shot instead of replaying the stale save cursor"
    );
}

#[test]
fn deferred_restored_cursor_never_crosses_to_a_replaced_weapon_set_slot() {
    let ini_content = r#"
Weapon __RustPendingOriginalBarrelWeapon
  AttackRange = 100.0
  PrimaryDamage = 25.0
  ShotsPerBarrel = 3
End

Weapon __RustPendingReplacementBarrelWeapon
  AttackRange = 100.0
  PrimaryDamage = 25.0
  ShotsPerBarrel = 5
End
"#;
    assert_eq!(
        crate::assets::ini_template_loader::register_weapons_from_ini_text(ini_content),
        2
    );

    let mut vt = ThingTemplate::new("DeferredCursorWeaponSetSwap");
    vt.set_primary_weapon_name("__RustPendingOriginalBarrelWeapon");
    vt.set_mine_clearing_primary_weapon_name("__RustPendingReplacementBarrelWeapon");
    let mut o = Object::new(vt, ObjectId(1), Team::USA);
    o.weapon = Some(Weapon::default());

    assert_eq!(o.fired_barrel_for_slot(0), Some(0));
    assert!(o.restore_weapon_barrel_runtime_for_slot(0, 2, 2));

    // C++ destroys the old PRIMARY Weapon when its conditional WeaponSet
    // changes.  A topology arriving after that replacement cannot transfer
    // the old instance's saved cursor into the mine-detail Weapon.
    o.set_weapon_set_mine_clearing_detail_at_frame(true, 0);
    assert!(o.set_weapon_barrel_count_for_slot(0, 4));

    let state = o
        .weapon_barrel_state_for_slot(0)
        .expect("replacement state");
    assert_eq!(
        (
            state.current_barrel,
            state.shots_per_barrel,
            state.shots_left_on_barrel
        ),
        (0, 5, 5)
    );
}

#[test]
fn snapshot_cursor_projection_rejects_pending_cursor_after_weapon_set_swap() {
    let ini_content = r#"
Weapon __RustSnapshotPendingOriginalWeapon
  AttackRange = 100.0
  PrimaryDamage = 25.0
  ShotsPerBarrel = 3
End

Weapon __RustSnapshotPendingReplacementWeapon
  AttackRange = 100.0
  PrimaryDamage = 25.0
  ShotsPerBarrel = 5
End
"#;
    assert_eq!(
        crate::assets::ini_template_loader::register_weapons_from_ini_text(ini_content),
        2
    );

    let mut template = ThingTemplate::new("SnapshotPendingWeaponSetSwap");
    template.set_primary_weapon_name("__RustSnapshotPendingOriginalWeapon");
    template.set_mine_clearing_primary_weapon_name("__RustSnapshotPendingReplacementWeapon");
    let mut object = Object::new(template, ObjectId(1), Team::USA);
    object.weapon = Some(Weapon::default());

    assert_eq!(object.fired_barrel_for_slot(0), Some(0));
    assert!(object.restore_weapon_barrel_runtime_for_slot(0, 2, 2));

    // Do not invoke a mutating cursor helper after the set swap: this models
    // a save before topology can configure the new concrete C++ Weapon.
    object.set_weapon_set_mine_clearing_detail_at_frame(true, 0);
    assert_eq!(
        object.weapon_barrel_cursor_for_snapshot(0),
        Some((0, 0)),
        "a staged cursor from a destroyed old Weapon must not cross a save boundary"
    );
}

#[test]
fn mine_clearing_weapon_set_swap_resets_primary_barrel_cursor() {
    let mut vt = ThingTemplate::new("MineDetailBarrelSwap");
    vt.set_primary_weapon_name("OrdinaryMineDetailTestWeapon");
    vt.set_mine_clearing_primary_weapon_name("DetailMineDetailTestWeapon");
    let mut o = Object::new(vt, ObjectId(1), Team::USA);
    o.weapon = Some(Weapon::default());

    // Model a partially consumed old PRIMARY cursor.  The next conditional
    // WeaponSet bind must replace it even before the first detail shot.
    o.weapon_barrel_states[0] =
        WeaponBarrelState::new(2, 4, Some("OrdinaryMineDetailTestWeapon".to_string()));
    o.weapon_barrel_states[0].current_barrel = 1;
    o.weapon_barrel_states[0].shots_left_on_barrel = 1;
    assert_eq!(
        o.weapon_barrel_state_for_slot(0)
            .expect("primary state")
            .current_barrel,
        1
    );

    o.set_weapon_set_mine_clearing_detail_at_frame(true, 0);

    let state = o
        .weapon_barrel_state_for_slot(0)
        .expect("reset primary state");
    assert_eq!((state.current_barrel, state.shots_left_on_barrel), (0, 1));
}
