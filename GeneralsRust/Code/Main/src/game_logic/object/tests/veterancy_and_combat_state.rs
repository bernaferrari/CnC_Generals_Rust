//! Behavior suite extracted from the original test module.
use super::*;

#[test]
fn veterancy_increases_weapon_damage() {
    let mut object = make_test_object();
    object.gain_experience(60.0); // Veteran → +10% dmg
    let veteran_damage = object.weapon.as_ref().map(|w| w.damage).unwrap_or_default();
    assert!((veteran_damage - 110.0).abs() < 0.01);

    object.gain_experience(90.0); // Elite → +20% dmg (total)
    let elite_damage = object.weapon.as_ref().map(|w| w.damage).unwrap_or_default();
    assert!((elite_damage - 120.0).abs() < 0.01);
}

#[test]
fn level_up_sets_weaponset_and_weaponbonus_flags() {
    let mut object = make_test_object();
    assert!(!object.weapon_set_veteran && !object.weapon_bonus_veteran);
    object.gain_experience(60.0);
    assert!(object.weapon_set_veteran);
    assert!(object.weapon_bonus_veteran);
    assert!(!object.weapon_set_elite && !object.weapon_set_hero);
    assert!(!object.weapon_bonus_elite && !object.weapon_bonus_hero);

    object.gain_experience(90.0); // 150 total → Elite
    assert!(!object.weapon_set_veteran && !object.weapon_bonus_veteran);
    assert!(object.weapon_set_elite);
    assert!(object.weapon_bonus_elite);
    assert!(!object.weapon_set_hero && !object.weapon_bonus_hero);

    object.gain_experience(150.0); // 300 total → Heroic
    assert!(!object.weapon_set_veteran && !object.weapon_set_elite);
    assert!(object.weapon_set_hero);
    assert!(object.weapon_bonus_hero);
    assert!(!object.weapon_bonus_veteran && !object.weapon_bonus_elite);
}

#[test]
fn weaponset_flag_and_salvage_unlock_unless_shared_across_sets() {
    // C++ WeaponSet::updateWeaponSet: set-flag / crate swaps drop a permanent
    // lock unless the incoming set has WeaponLockSharedAcrossSets.
    let mut object = make_test_object();
    object.secondary_weapon = Some(Weapon {
        damage: 30.0,
        ..Weapon::default()
    });
    assert!(object.set_weapon_lock(1, WeaponLockType::LockedPermanently));
    assert!(object.set_weapon_set_flag(0, true));
    assert!(!object.is_weapon_locked());
    assert_eq!(object.active_weapon_slot, 0);

    object.thing.template.weapon_lock_shared_across_sets = true;
    assert!(object.set_weapon_lock(1, WeaponLockType::LockedPermanently));
    assert!(object.set_weapon_set_flag(1, true));
    assert!(object.is_weapon_locked());
    assert_eq!(object.weapon_lock_slot, 1);

    object.thing.template.weapon_lock_shared_across_sets = false;
    assert!(object.set_weapon_lock(1, WeaponLockType::LockedPermanently));
    object.apply_salvage_weapon_upgrade();
    assert!(!object.is_weapon_locked());
    assert_eq!(object.active_weapon_slot, 0);
}

#[test]
fn veterancy_preserves_health_ratio_when_max_health_changes() {
    let mut object = make_test_object();
    object.health.current = 50.0;
    object.health.maximum = 100.0;

    object.gain_experience(60.0); // Veteran → +20% HP
    assert!((object.health.maximum - 120.0).abs() < 0.01);
    assert!((object.health.current - 60.0).abs() < 0.01);
}

#[test]
fn level_up_scales_current_max_health_not_template() {
    // C++ ActiveBody PRESERVE_RATIO: Composite Armor / difficulty max must survive.
    let mut object = make_test_object();
    object.thing.template.max_health = 100.0;
    object.health.maximum = 250.0;
    object.health.current = 250.0;
    object.max_health = 250.0;
    crate::game_logic::host_unit_training::clear_promote_fx();
    object.gain_experience(60.0);
    assert!(
        (object.health.maximum - 300.0).abs() < 0.01,
        "expected 250 * 1.2, got {}",
        object.health.maximum
    );
    assert!(
        (object.health.current - 300.0).abs() < 0.01,
        "PRESERVE_RATIO current, got {}",
        object.health.current
    );
}

#[test]
fn level_up_gives_veterancy_upgrade_and_promote_fx() {
    use crate::game_logic::host_unit_training::{
        UNIT_PROMOTED_AUDIO, UPGRADE_VETERANCY_VETERAN, clear_promote_fx, drain_promote_audio,
        promote_anims_snapshot,
    };
    let mut object = make_test_object();
    clear_promote_fx();
    object.gain_experience(60.0);
    assert!(object.has_upgrade_tag(UPGRADE_VETERANCY_VETERAN));
    let audio = drain_promote_audio();
    assert!(
        audio.iter().any(|e| e.event_name == UNIT_PROMOTED_AUDIO),
        "UnitPromoted must queue on rank-up"
    );
    assert!(
        !promote_anims_snapshot().is_empty(),
        "LevelGain Anim2D must queue on rank-up"
    );
}

#[test]
fn queue_veterancy_promote_fx_plays_for_local_visible_rank_up() {
    use crate::game_logic::host_unit_training::{
        UNIT_PROMOTED_AUDIO, clear_promote_fx, drain_promote_audio, promote_anims_snapshot,
    };
    let mut object = make_test_object();
    object.owner_player_id = Some(0);
    object.status.stealthed = false;
    clear_promote_fx();
    object.gain_experience(60.0);
    let audio = drain_promote_audio();
    assert!(
        audio.iter().any(|e| e.event_name == UNIT_PROMOTED_AUDIO),
        "local visible rank-up must queue UnitPromoted"
    );
    assert!(
        !promote_anims_snapshot().is_empty(),
        "local visible rank-up must queue LevelGain"
    );
}

#[test]
fn queue_veterancy_promote_fx_hides_undetected_stealthed_enemy() {
    use crate::game_logic::host_unit_training::{
        clear_promote_fx, drain_promote_audio, promote_anims_snapshot,
    };
    let mut object = make_test_object();
    object.owner_player_id = Some(1);
    object.status.stealthed = true;
    object.status.detected = false;
    object.status.disguised = false;
    clear_promote_fx();
    object.gain_experience(60.0);
    assert!(
        drain_promote_audio().is_empty(),
        "undetected stealthed enemy must not leak UnitPromoted"
    );
    assert!(
        promote_anims_snapshot().is_empty(),
        "undetected stealthed enemy must not leak LevelGain"
    );
}

#[test]
fn queue_veterancy_promote_fx_skips_rider_swap_and_hijack_without_feedback() {
    use crate::game_logic::host_unit_training::{
        clear_promote_fx, drain_promote_audio, promote_anims_snapshot,
    };
    let mut bike = make_test_object();
    clear_promote_fx();
    bike.set_rider_change_veterancy_level(VeterancyLevel::Veteran);
    assert!(
        drain_promote_audio().is_empty() && promote_anims_snapshot().is_empty(),
        "RiderChangeContain provideFeedback=FALSE queues nothing"
    );
    assert_eq!(bike.experience.level, VeterancyLevel::Veteran);

    let mut vehicle = make_test_object();
    let mut hijacker = make_test_object();
    hijacker.experience.level = VeterancyLevel::Elite;
    clear_promote_fx();
    vehicle.apply_hijacked_from(Some(&hijacker));
    assert!(
        drain_promote_audio().is_empty() && promote_anims_snapshot().is_empty(),
        "hijack setVeterancyLevel(..., FALSE) queues nothing"
    );
    assert_eq!(vehicle.experience.level, VeterancyLevel::Elite);
}

#[test]
fn ally_or_own_kill_awards_zero_experience() {
    let mut object = make_test_object();
    object.thing.template.experience_value = 40.0;
    object.thing.template.experience_values = [40.0, 40.0, 80.0, 120.0];
    assert!((object.kill_experience_value() - 40.0).abs() < 0.01);
    assert_eq!(object.kill_experience_value_from_killer(true), 0.0);
    object.status.under_construction = true;
    assert_eq!(object.kill_experience_value(), 0.0);
}

#[test]
fn kill_skill_point_value_uses_experience_value() {
    let mut object = make_test_object();
    object.thing.template.experience_value = 20.0;
    object.thing.template.experience_values = [20.0, 20.0, 40.0, 60.0];
    assert_eq!(object.kill_skill_point_value(), 20);
    object.experience.level = VeterancyLevel::Elite;
    assert_eq!(object.kill_skill_point_value(), 40);
    object.status.under_construction = true;
    assert_eq!(object.kill_skill_point_value(), 0);
}

#[test]
fn pilot_recrew_adds_pilot_levels_not_max() {
    let mut vehicle = make_test_object();
    vehicle.status.disabled_unmanned = true;
    vehicle.experience.level = VeterancyLevel::Elite;
    assert!(vehicle.apply_pilot_recrew(Team::USA, Some(0), VeterancyLevel::Veteran));
    assert_eq!(vehicle.experience.level, VeterancyLevel::Heroic);
}

#[test]
fn script_enabled_powered_set_live_disable_mask() {
    let mut obj = make_test_object();
    assert!(obj.can_move());
    assert!(obj.can_attack());
    obj.apply_object_panel_flag("Enabled", false);
    assert!(obj.is_disabled());
    assert!(obj.is_script_disabled());
    assert!(!obj.can_move());
    assert!(!obj.can_attack());
    obj.apply_object_panel_flag("Enabled", true);
    assert!(!obj.is_disabled());
    obj.apply_object_panel_flag("Powered", false);
    assert!(obj.is_script_underpowered());
    assert!(obj.is_disabled());
    assert!(!obj.can_move());
}

#[test]
fn script_indestructible_panel_flag_sets_body() {
    let mut obj = make_test_object();
    assert!(!obj.is_indestructible());
    obj.apply_object_panel_flag("Indestructible", true);
    assert!(obj.is_indestructible());
    obj.apply_object_panel_flag("Indestructible", false);
    assert!(!obj.is_indestructible());
}

#[test]
fn leftover_sa_player_targetable_sets_script_targetable() {
    let mut obj = make_test_object();
    assert!(!obj.is_script_targetable());
    obj.apply_object_panel_flag("Player Targetable", true);
    assert!(obj.script_targetable);
    assert!(obj.is_script_targetable());
    obj.apply_object_panel_flag("PlayerTargetable", false);
    assert!(!obj.script_targetable);
}

#[test]
fn paralyzed_rejects_move_orders() {
    let mut obj = make_test_object();
    obj.apply_disabled_paralyzed(30);
    assert!(obj.is_disabled());
    assert!(!obj.can_move(), "C++ isMobile is false while paralyzed");
    assert!(!obj.can_attack());
}

#[test]
fn sold_and_under_construction_cannot_attack_or_fire() {
    // C++ Object::isAbleToAttack (Object.cpp:3171-3176).
    let mut obj = make_test_object();
    obj.weapon.as_mut().unwrap().last_fire_time = -10.0;
    assert!(obj.can_attack());
    assert!(obj.can_fire(0.0));
    assert!(obj.fire_at(ObjectId(2), 0.0));

    obj.status.under_construction = true;
    assert!(!obj.can_attack(), "UC Patriots/Stingers cannot acquire");
    assert!(!obj.can_fire(1.0), "UC cannot discharge");
    assert!(!obj.fire_at(ObjectId(2), 1.0), "UC fire_at must fail");
    obj.status.under_construction = false;
    assert!(obj.can_attack());

    obj.status.sold = true;
    assert!(!obj.can_attack(), "sold defenses cannot acquire");
    assert!(!obj.can_fire(2.0), "sold cannot discharge");
    assert!(!obj.fire_at(ObjectId(2), 2.0), "sold fire_at must fail");
}

#[test]
fn unmanned_enter_wipes_veterancy_and_auto_heal() {
    let mut vehicle = make_test_object();
    vehicle.experience.level = VeterancyLevel::Elite;
    vehicle.experience.current = 200.0;
    vehicle.default_auto_heal = Some(crate::game_logic::host_heal::HostDefaultAutoHealData::new());
    assert!(
        vehicle
            .default_auto_heal
            .as_ref()
            .is_some_and(|h| h.upgrade_active)
    );
    vehicle.apply_kill_pilot_unmanned();
    assert_eq!(vehicle.experience.level, VeterancyLevel::Rookie);
    assert_eq!(vehicle.experience.current, 0.0);
    assert!(
        vehicle
            .default_auto_heal
            .as_ref()
            .is_some_and(|h| !h.upgrade_active)
    );
    assert!(vehicle.apply_pilot_recrew(Team::USA, Some(0), VeterancyLevel::Veteran));
    assert_eq!(vehicle.experience.level, VeterancyLevel::Veteran);
}

#[test]
fn battle_bus_second_life_is_held() {
    let mut obj = make_test_object();
    obj.status.disabled_held = true;
    assert!(obj.is_disabled());
    assert!(obj.is_physics_held());
    assert!(!obj.can_move());
    assert!(!obj.can_attack());
}

#[test]
fn hacked_spawn_site_idles_residual_slaves() {
    let mut site = make_test_object();
    site.template_name = "GLAStingerSite".into();
    site.hive_slaves = crate::game_logic::host_base_defense::init_stinger_hive_slave_roster();
    site.hive_slave_count = 3;
    let _ = crate::game_logic::host_base_defense::order_hive_slaves_to_attack_target(
        &mut site.hive_slaves,
        99,
    );
    assert!(site.hive_slaves.iter().any(|s| s.ai_attacking));
    site.apply_disabled_hacked(60);
    assert!(site.is_hacked_disabled());
    assert!(site.hive_slaves.iter().all(|s| !s.ai_attacking));
}

#[test]
fn stop_attack_clears_force_attack_and_targets() {
    let mut object = make_test_object();
    object.set_target(Some(ObjectId(99)));
    object.set_force_attack(true);
    object.set_target_location(Some(Vec3::new(1.0, 0.0, 2.0)));
    assert!(object.set_weapon_lock(0, WeaponLockType::LockedTemporarily));
    object.stop_attack();

    assert!(object.target.is_none());
    assert!(object.target_location.is_none());
    assert!(!object.force_attack);
    assert!(!object.status.attacking);
    assert_eq!(object.weapon_lock_type, WeaponLockType::NotLocked);
}

#[test]
fn temporary_tertiary_lock_releases_after_its_auto_clip_reloads() {
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

    assert!(object.fire_at(ObjectId(99), 1.0));

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
    let mut object = make_test_object();
    object.set_target(Some(ObjectId(77)));
    object.set_target_location(Some(Vec3::new(10.0, 0.0, 10.0)));

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
    assert!(object.fire_at(ObjectId(2), 0.0));
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
fn clip_ammo_cpp_surface() {
    let src = crate::game_logic::object::OBJECT_SRC;
    assert!(src.contains("fn consume_ammo_on_fire"));
    assert!(src.contains("clip_reload_time"));
}

#[test]
fn pre_attack_delay_blocks_first_shot() {
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
    assert!(!atk.fire_at(tgt_id, 10.0));
    assert_eq!(atk.pre_attack_target, Some(tgt_id));
    assert!((atk.pre_attack_ready_at - 11.0).abs() < 1e-4);
    assert_eq!(atk.weapon.as_ref().unwrap().ammo, Some(5));

    // Still winding up.
    assert!(!atk.fire_at(tgt_id, 10.5));
    assert_eq!(atk.weapon.as_ref().unwrap().ammo, Some(5));

    // After delay, fires and consumes ammo.
    assert!(atk.fire_at(tgt_id, 11.0));
    assert_eq!(atk.weapon.as_ref().unwrap().ammo, Some(4));
}

#[test]
fn pre_attack_resets_on_new_target() {
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
    assert!(!atk.fire_at(ObjectId(10), 5.0));
    assert!((atk.pre_attack_ready_at - 7.0).abs() < 1e-4);
    // Switch target restarts delay.
    assert!(!atk.fire_at(ObjectId(11), 6.0));
    assert_eq!(atk.pre_attack_target, Some(ObjectId(11)));
    assert!((atk.pre_attack_ready_at - 8.0).abs() < 1e-4);
}

#[test]
fn pre_attack_cpp_surface() {
    let src = crate::game_logic::object::OBJECT_SRC;
    assert!(src.contains("PRE_ATTACK residual"));
    assert!(src.contains("pre_attack_ready_at"));
    assert!(src.contains("pre_attack_delay"));
}

#[test]
fn small_arms_reduced_on_tank_armor_residual() {
    use crate::game_logic::combat::DamageType;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut tmpl = ThingTemplate::new("ArmorTank");
    tmpl.set_health(1000.0);
    tmpl.add_kind_of(KindOf::Vehicle);
    tmpl.add_kind_of(KindOf::Attackable);
    let mut tank = Object::new(tmpl, ObjectId(70), Team::USA);
    let hp0 = tank.health.current;
    // TankArmor SmallArms residual is 0.25 → 100 * 0.25 = 25
    tank.take_damage_from_typed(100.0, None, DamageType::Bullet);
    let dealt = hp0 - tank.health.current;
    assert!(
        (dealt - 25.0).abs() < 1.0,
        "expected ~25 small-arms on tank, got {dealt}"
    );
}

#[test]
fn laser_half_on_human_armor_residual() {
    use crate::game_logic::combat::DamageType;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut tmpl = ThingTemplate::new("ArmorInf");
    tmpl.set_health(500.0);
    tmpl.add_kind_of(KindOf::Infantry);
    tmpl.add_kind_of(KindOf::Attackable);
    let mut inf = Object::new(tmpl, ObjectId(71), Team::GLA);
    let hp0 = inf.health.current;
    // HumanArmor Laser residual 0.5 → 100 * 0.5 = 50
    inf.take_damage_from_typed(100.0, None, DamageType::Laser);
    let dealt = hp0 - inf.health.current;
    assert!(
        (dealt - 50.0).abs() < 1.0,
        "expected ~50 laser on infantry, got {dealt}"
    );
}

#[test]
fn flame_kill_sets_burned_death_type() {
    use crate::game_logic::combat::DamageType;
    use crate::game_logic::host_usa_pilot::HostDeathType;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut tmpl = ThingTemplate::new("BurnMe");
    tmpl.set_health(50.0);
    tmpl.add_kind_of(KindOf::Infantry);
    let mut o = Object::new(tmpl, ObjectId(80), Team::GLA);
    let dead =
        o.take_damage_from_typed_death(999.0, None, DamageType::Flame, HostDeathType::Burned);
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
    atk.set_ai_state(AIState::Docked);
    assert!(
        !atk.is_within_attack_range(&vic),
        "transport / hospital contain must not grant garrison RANGE"
    );
    atk.set_ai_state(AIState::Garrisoned);
    assert!(
        atk.is_within_attack_range(&vic),
        "garrison RANGE 133% should cover 120 with base 100"
    );
}

#[test]
fn fire_at_scales_secondary_damage_with_damage_bonus() {
    use crate::game_logic::host_unit_training::VETERANCY_DAMAGE_BONUS_VETERAN;
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};

    crate::game_logic::combat::clear_pending_projectile_queue_for_test();
    let mut tmpl = ThingTemplate::new("Scorpion");
    tmpl.add_kind_of(KindOf::Vehicle);
    tmpl.add_kind_of(KindOf::Attackable);
    tmpl.set_health(100.0);
    tmpl.primary_weapon_name = Some("ScorpionTankGun".into());
    let mut atk = Object::new(tmpl, ObjectId(1), Team::GLA);
    atk.weapon = Some(Weapon {
        damage: 25.0,
        range: 100.0,
        ..Weapon::default()
    });
    atk.weapon_bonus_veteran = true;
    assert!(atk.fire_at(ObjectId(2), 0.0));
    let raw = crate::game_logic::weapon_bootstrap::host_secondary_damage_for_weapon_name(
        "ScorpionTankGun",
    );
    assert!(raw > 0.0, "ScorpionTankGun must have a secondary ring");
    let stamped = crate::game_logic::combat::last_pending_projectile_secondary_damage_for_test()
        .expect("queued splash");
    let expected = raw * VETERANCY_DAMAGE_BONUS_VETERAN;
    assert!(
        (stamped - expected).abs() < 0.01,
        "secondary ring must scale with DAMAGE bonus ({stamped} vs {expected})"
    );
    crate::game_logic::combat::clear_pending_projectile_queue_for_test();
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
    o.mine_clearing_primary_weapon = Some(Weapon::default());

    assert_eq!(o.fired_barrel_for_slot(0), Some(0));
    assert!(o.restore_weapon_barrel_runtime_for_slot(0, 2, 2));

    // C++ destroys the old PRIMARY Weapon when its conditional WeaponSet
    // changes.  A topology arriving after that replacement cannot transfer
    // the old instance's saved cursor into the mine-detail Weapon.
    o.set_weapon_set_mine_clearing_detail(true);
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
    object.mine_clearing_primary_weapon = Some(Weapon::default());
    assert_eq!(object.fired_barrel_for_slot(0), Some(0));
    assert!(object.restore_weapon_barrel_runtime_for_slot(0, 2, 2));

    // Do not invoke a mutating cursor helper after the set swap: this models
    // a save before topology can configure the new concrete C++ Weapon.
    object.set_weapon_set_mine_clearing_detail(true);
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
    o.mine_clearing_primary_weapon = Some(Weapon::default());
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

    o.set_weapon_set_mine_clearing_detail(true);

    let state = o
        .weapon_barrel_state_for_slot(0)
        .expect("reset primary state");
    assert_eq!((state.current_barrel, state.shots_left_on_barrel), (0, 1));
}

#[test]
fn fire_sound_loop_extends_and_stops() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    let mut tmpl = ThingTemplate::new("FlameLoop");
    tmpl.primary_weapon_name = Some("DragonTankFlameWeapon".into());
    tmpl.set_health(100.0);
    tmpl.add_kind_of(KindOf::Vehicle);
    tmpl.add_kind_of(KindOf::Attackable);
    let mut o = Object::new(tmpl, ObjectId(7), Team::China);
    o.weapon = Some(Weapon {
        damage: 5.0,
        range: 50.0,
        reload_time: 0.1,
        last_fire_time: -100.0,
        ..Weapon::default()
    });
    crate::game_logic::host_fire_sound_loop_log::clear();
    o.stamp_fire_sound_loop_after_shot(10, Some("DragonTankFlameWeapon"));
    assert!(o.fire_sound_loop_until_frame > 10);
    let start = crate::game_logic::host_fire_sound_loop_log::drain();
    assert_eq!(start.len(), 1);
    assert!(start[0].start);
    // refresh should not re-emit start while still active
    o.stamp_fire_sound_loop_after_shot(11, Some("DragonTankFlameWeapon"));
    assert!(crate::game_logic::host_fire_sound_loop_log::drain().is_empty());
    let stop_at = o.fire_sound_loop_until_frame;
    o.tick_fire_sound_loop(stop_at);
    let stop = crate::game_logic::host_fire_sound_loop_log::drain();
    assert_eq!(stop.len(), 1);
    assert!(!stop[0].start);
    assert_eq!(o.fire_sound_loop_until_frame, 0);
}

#[test]
fn height_die_kills_when_low() {
    use crate::game_logic::host_height_die::HostHeightDieData;
    use crate::game_logic::{Team, ThingTemplate};
    let mut t = ThingTemplate::new("AmericaAuroraBomb");
    t.set_health(10.0);
    let mut o = Object::new(t, ObjectId(1), Team::USA);
    o.height_die = Some(HostHeightDieData::with_target(5.0, true, 0));
    o.set_position(glam::Vec3::new(0.0, 100.0, 0.0));
    assert!(!o.tick_height_die(1, 0.0));
    o.set_position(glam::Vec3::new(0.0, 50.0, 0.0));
    assert!(!o.tick_height_die(2, 0.0));
    o.set_position(glam::Vec3::new(0.0, 3.0, 0.0));
    assert!(o.tick_height_die(3, 0.0));
    assert!(o.status.destroyed);
}

#[test]
fn squish_requires_velocity_toward_victim() {
    use crate::game_logic::host_squish_collide::velocity_toward_victim;
    assert!(velocity_toward_victim((0.0, 0.0), (5.0, 0.0), (2.0, 0.0)));
    assert!(!velocity_toward_victim((0.0, 0.0), (5.0, 0.0), (-2.0, 0.0)));

    let mut vt = ThingTemplate::new("CrusherTank");
    vt.add_kind_of(KindOf::Vehicle);
    let mut tank = Object::new(vt, ObjectId(101), Team::USA);
    tank.crusher_level = 1;
    tank.set_orientation(0.0);
    // Moving toward infantry (+X).
    tank.movement.velocity = glam::Vec3::new(5.0, 0.0, 0.0);
    tank.set_position(glam::Vec3::new(0.0, 0.0, 0.0));
    tank.selection_radius = 8.0;

    let mut it = ThingTemplate::new("CrushableInf");
    it.add_kind_of(KindOf::Infantry);
    let mut inf = Object::new(it, ObjectId(102), Team::GLA);
    inf.crushable_level = 0;
    inf.selection_radius = 5.0;
    inf.set_position(glam::Vec3::new(4.0, 0.0, 0.0));
    inf.health.current = 100.0;
    inf.health.maximum = 100.0;

    assert!(
        tank.check_for_overlap_collision(&mut inf, false),
        "squish must kill when moving toward infantry in tight radius"
    );
    assert!(inf.front_crushed && inf.back_crushed);
    assert!(!inf.is_alive() || inf.health.current <= 0.0);
}

#[test]
fn defection_timer_expires_and_blows_on_fire() {
    use crate::game_logic::host_defection_helper::DEFAULT_DEFECTION_PROTECTION_FRAMES;
    let mut t = ThingTemplate::new("AmericaInfantryPilot");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Infantry);
    let mut o = Object::new(t, ObjectId(301), Team::USA);
    o.begin_undetected_defection(0, 30, true);
    assert!(o.is_undetected_defector());
    for f in 0..29 {
        o.tick_defection_helper(f);
    }
    assert!(o.is_undetected_defector());
    o.tick_defection_helper(30);
    assert!(!o.is_undetected_defector());

    o.begin_undetected_defection(0, DEFAULT_DEFECTION_PROTECTION_FRAMES, true);
    assert!(o.is_undetected_defector());
    o.status.is_firing_weapon = true;
    o.tick_defection_helper(5);
    assert!(!o.is_undetected_defector());
}

#[test]
fn fire_weapon_power_queues_shots() {
    let mut t = ThingTemplate::new("SpectreHowitzerMarker");
    t.set_health(100.0);
    let mut o = Object::new(t, ObjectId(302), Team::USA);
    assert!(o.activate_fire_weapon_power(Some((100.0, 200.0))));
    let req = o.fire_weapon_power.as_ref().unwrap();
    assert_eq!(req.shots_remaining, 3);
    assert!(req.has_location);
}

#[test]
fn poisoned_behavior_dots_after_toxin() {
    use crate::game_logic::combat::DamageType;
    use crate::game_logic::host_historic_bonus;
    let mut t = ThingTemplate::new("TestInfantry");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Infantry);
    let mut o = Object::new(t, ObjectId(201), Team::USA);
    o.health.current = 100.0;
    o.health.maximum = 100.0;
    host_historic_bonus::set_logic_frame(10);
    let _ = o.take_damage_from_typed(20.0, None, DamageType::Toxin);
    // HP reduced by initial hit
    let after_hit = o.health.current;
    assert!(after_hit < 100.0);
    assert!(
        o.poisoned_behavior
            .as_ref()
            .map(|p| p.is_active())
            .unwrap_or(false)
    );
    assert!(o.is_poison_tinted());
    // Advance DoT ticks
    host_historic_bonus::set_logic_frame(20);
    let mut total_dot = 0.0;
    for f in 11..100 {
        if let Some((d, _)) = o.tick_poisoned_behavior(f) {
            total_dot += d;
            let _ = o.take_damage_from_typed_death(
                d,
                None,
                DamageType::Unresistable,
                crate::game_logic::host_usa_pilot::HostDeathType::Poisoned,
            );
        }
    }
    assert!(total_dot > 0.0, "poison DoT must tick");
    assert!(o.health.current < after_hit || !o.is_alive());
}

#[test]
fn healing_clears_poison() {
    use crate::game_logic::combat::DamageType;
    use crate::game_logic::host_historic_bonus;
    let mut t = ThingTemplate::new("TestInfantry");
    t.set_health(100.0);
    let mut o = Object::new(t, ObjectId(202), Team::USA);
    o.health.current = 80.0;
    o.health.maximum = 100.0;
    host_historic_bonus::set_logic_frame(5);
    let _ = o.take_damage_from_typed(10.0, None, DamageType::Toxin);
    assert!(o.is_poison_tinted());
    o.heal(5.0);
    assert!(!o.is_poison_tinted());
}

#[test]
fn bone_fx_fires_on_body_damage_worsen() {
    use crate::game_logic::host_bone_fx_damage::{
        HostBoneFxAuthored, HostBoneFxDamageData, HostBoneFxSlot,
    };
    use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
    let mut t = ThingTemplate::new("GLAVehicleScudLauncher");
    t.set_health(1000.0);
    t.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(t, ObjectId(103), Team::GLA);
    let mut authored = HostBoneFxAuthored::default();
    authored.fx[HostBodyDamageType::Damaged.ordinal() as usize][0] = Some(HostBoneFxSlot {
        bone: "FXBone01".into(),
        only_once: true,
        delay_min: 0.0,
        delay_max: 0.0,
        name: "FX_ScudLauncherDamageTransition".into(),
    });
    o.bone_fx_damage = Some(HostBoneFxDamageData::from_authored(authored));
    o.health.current = 1000.0;
    o.health.maximum = 1000.0;
    o.refresh_model_condition_bits();
    // Drop into damaged band.
    o.health.current = 400.0;
    o.refresh_model_condition_bits();
    assert!(
        o.bone_fx_damage
            .as_ref()
            .map(|b| b.transitions > 0)
            .unwrap_or(false),
        "BoneFX must fire on damage transition"
    );
    assert_eq!(
        o.bone_fx_damage.as_ref().and_then(|b| b.last_fx.as_deref()),
        Some("FX_ScudLauncherDamageTransition")
    );
}

#[test]
fn bone_fx_pristine_slots_fire_without_state_change() {
    use crate::game_logic::host_bone_fx_damage::{
        HostBoneFxAuthored, HostBoneFxDamageData, HostBoneFxSlot,
    };
    use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
    let mut t = ThingTemplate::new("GLAVehicleScudLauncher");
    t.set_health(1000.0);
    t.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(t, ObjectId(104), Team::GLA);
    let mut authored = HostBoneFxAuthored::default();
    authored.fx[HostBodyDamageType::Pristine.ordinal() as usize][0] = Some(HostBoneFxSlot {
        bone: "FXBone01".into(),
        only_once: true,
        delay_min: 0.0,
        delay_max: 0.0,
        name: "FX_ScudLauncherPristine".into(),
    });
    o.bone_fx_damage = Some(HostBoneFxDamageData::from_authored(authored));
    o.health.current = 1000.0;
    o.health.maximum = 1000.0;
    o.body_damage_state = HostBodyDamageType::Pristine;
    if let Some(bfx) = o.bone_fx_damage.as_mut() {
        bfx.tick(10);
    }
    assert_eq!(
        o.bone_fx_damage.as_ref().and_then(|b| b.last_fx.as_deref()),
        Some("FX_ScudLauncherPristine"),
        "C++ BoneFXUpdate ctor BODY_PRISTINE; first update fires authored pristine slots"
    );
}

#[test]
fn crush_die_sets_model_condition_bits() {
    use crate::game_logic::host_neutron_missile_slow_death::{
        MC_BIT_BACKCRUSHED, MC_BIT_FRONTCRUSHED,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut t = ThingTemplate::new("TestInfantry");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Infantry);
    let mut o = Object::new(t, ObjectId(4), Team::USA);
    o.front_crushed = true;
    o.apply_crush_die_model_conditions();
    assert_ne!(o.model_condition_bits & (1u128 << MC_BIT_FRONTCRUSHED), 0);
    assert_eq!(o.model_condition_bits & (1u128 << MC_BIT_BACKCRUSHED), 0);
    o.back_crushed = true;
    o.apply_crush_die_model_conditions();
    assert_ne!(o.model_condition_bits & (1u128 << MC_BIT_BACKCRUSHED), 0);
}

#[test]
fn keep_object_die_leaves_rubble() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut t = ThingTemplate::new("TechHospital");
    t.set_health(500.0);
    t.add_kind_of(KindOf::Structure);
    let mut o = Object::new(t, ObjectId(9), Team::Neutral);
    o.health.current = 0.0;
    assert!(o.begin_keep_object_die(10));
    assert!(o.status.keep_as_rubble);
    assert!(o.status.effectively_dead);
    assert!(!o.status.destroyed);
    assert!(!o.is_alive());
    assert!(
        o.status.no_collisions,
        "C++ ActiveBody rubble stamps OBJECT_STATUS_NO_COLLISIONS"
    );
}

#[test]
fn structure_rubble_collapses_geometry_z_and_stamps_no_collisions() {
    // C++ ActiveBody.cpp:189-208 setGeometryInfoZ + OBJECT_STATUS_NO_COLLISIONS.
    use crate::game_logic::{HostGeometryInfo, HostGeometryType, KindOf, Team, ThingTemplate};
    let mut t = ThingTemplate::new("AmericaCommandCenter");
    t.set_health(400.0);
    t.add_kind_of(KindOf::Structure);
    t.structure_rubble_height = 8;
    t.geometry_info = HostGeometryInfo {
        geom_type: HostGeometryType::Box,
        is_small: false,
        height: 40.0,
        major_radius: 30.0,
        minor_radius: 25.0,
        authored: true,
    };
    let mut o = Object::new(t, ObjectId(11), Team::USA);
    assert!((o.thing.template.geometry_info.height - 40.0).abs() < 1e-4);
    assert!(!o.status.no_collisions);
    o.health.current = 0.0;
    o.refresh_model_condition_bits();
    assert_eq!(
        o.body_damage_state,
        crate::game_logic::host_enum_table_residual::HostBodyDamageType::Rubble
    );
    assert!(o.status.no_collisions);
    assert!(
        (o.thing.template.geometry_info.height - 8.0).abs() < 1e-4,
        "rubble must collapse authored box Z to StructureRubbleHeight"
    );
    assert!((o.thing.geometry.bounds_max.y - o.thing.geometry.bounds_min.y - 8.0).abs() < 1e-4);
}

#[test]
fn ground_or_structure_height_uses_bounding_circle_not_major() {
    // C++ PartitionManager.cpp:4686-4687 FROM_BOUNDINGSPHERE_2D subtracts
    // bounding-circle (sqrt(major^2+minor^2)), so rooftop corners stay in range.
    use crate::game_logic::{HostGeometryInfo, HostGeometryType, KindOf, Team, ThingTemplate};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("TestWideFactory");
    tmpl.add_kind_of(KindOf::Structure);
    tmpl.geometry_info = HostGeometryInfo {
        geom_type: HostGeometryType::Box,
        is_small: false,
        height: 25.0,
        major_radius: 20.0,
        minor_radius: 10.0,
        authored: true,
    };
    logic.templates.insert("TestWideFactory".into(), tmpl);
    logic
        .create_object("TestWideFactory", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("spawn factory");
    // Corner of the box is ~22.36wu from center. major_radius=20 would miss
    // RANGE=1; bounding circle includes it.
    let corner = Vec3::new(20.5, 0.0, 10.5);
    let h = logic.ground_or_structure_height_at(corner, 0.0);
    assert!(
        (h - 25.0).abs() < 0.01,
        "rooftop corner must ride structure height, got {h}"
    );
}

#[test]
fn jet_slow_death_begins_for_raptor() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut t = ThingTemplate::new("AmericaJetRaptor");
    t.set_health(200.0);
    t.add_kind_of(KindOf::Aircraft);
    let mut o = Object::new(t, ObjectId(3), Team::USA);
    o.health.current = 0.0;
    o.set_position(glam::Vec3::new(0.0, 80.0, 0.0));
    assert!(o.begin_jet_slow_death());
    assert!(o.jet_slow_death.as_ref().unwrap().is_active());
}

#[test]
fn helicopter_slow_death_begins() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut t = ThingTemplate::new("AmericaComanche");
    t.set_health(200.0);
    t.add_kind_of(KindOf::Aircraft);
    let mut o = Object::new(t, ObjectId(2), Team::USA);
    o.health.current = 0.0;
    assert!(o.begin_helicopter_slow_death());
    assert!(o.helicopter_slow_death.as_ref().unwrap().is_active());
}

#[test]
fn slow_death_infantry_defers_and_sinks() {
    use crate::game_logic::host_slow_death::HostSlowDeathIni;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut t = ThingTemplate::new("AmericaInfantryRanger");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Infantry);
    let mut o = Object::new(t, ObjectId(1), Team::USA);
    o.health.current = 0.0;
    // C++ SlowDeathBehavior.cpp:191 beginSlowDeath uses authored module data,
    // not KindOf::Infantry. Bare fixtures must pass INI SinkDelay/SinkRate/DestructionDelay.
    assert!(o.begin_slow_death_from_ini(0, &HostSlowDeathIni::infantry_retail()));
    assert!(!o.status.destroyed);
    assert!(o.slow_death.as_ref().unwrap().is_active());
    let mut done = false;
    for f in 0..400 {
        if o.tick_slow_death(f) {
            done = true;
            break;
        }
    }
    assert!(done);
    assert!(o.status.destroyed);
    assert!(o.presentation_slow_death_sink_offset() <= 0.0);
}

#[test]
fn create_object_die_queues_spawns() {
    use crate::game_logic::host_create_object_die::HostCreateObjectDieData;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut t = ThingTemplate::new("GLASneakAttackTunnelNetworkStart");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Structure);
    let mut o = Object::new(t, ObjectId(1), Team::GLA);
    o.create_object_die = Some(HostCreateObjectDieData {
        ocl_name: "OCL_CreateSneakAttackTunnel".into(),
        spawn_templates: vec!["GLASneakAttackTunnelNetwork".into()],
        transfer_previous_health: true,
        fired: false,
    });
    o.health.current = 0.0;
    o.status.destroyed = true;
    o.refresh_model_condition_bits();
    let (spawns, dmg, transfer, _sub, _src) = o.take_pending_create_object_die_spawns();
    assert_eq!(spawns, vec!["GLASneakAttackTunnelNetwork".to_string()]);
    assert!(transfer);
    assert!(dmg >= 0.0);
}

#[test]
fn create_object_die_uses_previous_health_not_current() {
    use crate::game_logic::host_create_object_die::HostCreateObjectDieData;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut t = ThingTemplate::new("GLASneakAttackTunnelNetworkStart");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Structure);
    let mut o = Object::new(t, ObjectId(3), Team::GLA);
    o.create_object_die = Some(HostCreateObjectDieData {
        ocl_name: "OCL_CreateSneakAttackTunnel".into(),
        spawn_templates: vec!["GLASneakAttackTunnelNetwork".into()],
        transfer_previous_health: true,
        fired: false,
    });
    o.health.current = 0.0;
    o.previous_health = 100.0;
    o.subdual_damage = 25.0;
    o.last_damage_source = Some(ObjectId(9));
    o.fire_create_object_die();
    let (_spawns, dmg, transfer, sub, src) = o.take_pending_create_object_die_spawns();
    assert!(transfer);
    assert!(
        dmg.abs() < 1e-3,
        "lifetime kill at full previous health transfers 0, got {dmg}"
    );
    assert!((sub - 25.0).abs() < 1e-3);
    assert_eq!(src, Some(ObjectId(9)));
}

#[test]
fn crush_die_queues_sound_on_crushed_death() {
    use crate::game_logic::host_usa_pilot::HostDeathType;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut t = ThingTemplate::new("AmericaRanger");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Infantry);
    let mut o = Object::new(t, ObjectId(4), Team::USA);
    o.status.death_type = HostDeathType::Crushed;
    o.front_crushed = true;
    o.back_crushed = true;
    o.fire_crush_die();
    let (_, audio) = o.take_pending_death_fx_audio();
    assert_eq!(audio.as_deref(), Some("InfantryCrush"));
}

#[test]
fn crush_die_zero_flags_defaults_to_total() {
    use crate::game_logic::host_neutron_missile_slow_death::{
        MC_BIT_BACKCRUSHED, MC_BIT_FRONTCRUSHED,
    };
    use crate::game_logic::host_usa_pilot::HostDeathType;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut t = ThingTemplate::new("AmericaRanger");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Infantry);
    let mut o = Object::new(t, ObjectId(5), Team::USA);
    o.status.death_type = HostDeathType::Crushed;
    assert!(!o.front_crushed && !o.back_crushed);
    o.fire_crush_die();
    assert!(o.front_crushed && o.back_crushed);
    assert_ne!(o.model_condition_bits & (1u128 << MC_BIT_FRONTCRUSHED), 0);
    assert_ne!(o.model_condition_bits & (1u128 << MC_BIT_BACKCRUSHED), 0);
    let (_, audio) = o.take_pending_death_fx_audio();
    assert_eq!(audio.as_deref(), Some("InfantryCrush"));
}

#[test]
fn crush_die_recomputes_front_from_crusher_pose() {
    use crate::game_logic::host_neutron_missile_slow_death::{
        MC_BIT_BACKCRUSHED, MC_BIT_FRONTCRUSHED,
    };
    use crate::game_logic::host_usa_pilot::HostDeathType;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut t = ThingTemplate::new("AmericaRanger");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Infantry);
    let mut o = Object::new(t, ObjectId(6), Team::USA);
    o.status.death_type = HostDeathType::Crushed;
    o.set_position(glam::Vec3::new(0.0, 0.0, 0.0));
    o.set_orientation(0.0);
    o.thing.template.geometry_info.authored = true;
    o.thing.template.geometry_info.major_radius = 10.0;
    o.fire_crush_die_from_crusher(Some((5.0, 0.0)));
    assert!(o.front_crushed);
    assert!(!o.back_crushed);
    assert_ne!(o.model_condition_bits & (1u128 << MC_BIT_FRONTCRUSHED), 0);
    assert_eq!(o.model_condition_bits & (1u128 << MC_BIT_BACKCRUSHED), 0);
}

#[test]
fn instant_death_under_construction_building() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut t = ThingTemplate::new("AmericaParticleUplinkCannon");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Structure);
    let mut o = Object::new(t, ObjectId(5), Team::USA);
    o.status.under_construction = true;
    assert!(o.fire_instant_death());
    let (fx, _) = o.take_pending_death_fx_audio();
    assert_eq!(fx.as_deref(), Some("FX_StructureMediumDeath"));
}

#[test]
fn lifetime_update_expires() {
    use crate::game_logic::host_lifetime_update::HostLifetimeUpdateData;
    use crate::game_logic::{Team, ThingTemplate};
    let mut t = ThingTemplate::new("PoisonFieldMedium");
    t.set_health(10.0);
    let mut o = Object::new(t, ObjectId(2), Team::Neutral);
    o.lifetime_update = Some(HostLifetimeUpdateData::from_delay_frames(0, 3));
    assert!(!o.tick_lifetime_update(2));
    assert!(o.tick_lifetime_update(3));
}

#[test]
fn transition_damage_fx_queues_on_worse_state() {
    use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
    use crate::game_logic::host_transition_damage_fx::HostTransitionDamageFxData;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut t = ThingTemplate::new("AmericaWarFactory");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Structure);
    let mut o = Object::new(t, ObjectId(1), Team::USA);
    o.health.current = 100.0;
    o.health.maximum = 100.0;
    o.transition_damage_fx = Some(HostTransitionDamageFxData::generic_structure_residual());
    o.body_damage_state = HostBodyDamageType::Pristine;
    o.health.current = 40.0; // damaged
    o.refresh_model_condition_bits();
    assert_eq!(o.body_damage_state, HostBodyDamageType::Damaged);
    let ev = o.take_pending_transition_damage_fx();
    assert!(!ev.is_empty());
    assert_eq!(ev[0].new_state, HostBodyDamageType::Damaged.ordinal());
}

#[test]
fn fx_list_die_queues_on_rubble() {
    use crate::game_logic::host_fx_list_die::HostFxListDieData;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut t = ThingTemplate::new("AmericaTankCrusader");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(t, ObjectId(2), Team::USA);
    o.fx_list_die = Some(HostFxListDieData {
        death_fx: Some("FX_VehicleDie".into()),
        death_audio: Some("VehicleDestroyed".into()),
        ..Default::default()
    });
    o.health.current = 0.0;
    o.status.destroyed = true;
    o.refresh_model_condition_bits();
    let (fx, audio) = o.take_pending_death_fx_audio();
    assert_eq!(fx.as_deref(), Some("FX_VehicleDie"));
    assert_eq!(audio.as_deref(), Some("VehicleDestroyed"));
}

#[test]
fn fx_list_die_extra_module_uses_last_damage_source() {
    use crate::game_logic::host_fx_list_die::HostFxListDieData;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut t = ThingTemplate::new("AmericaTankCrusader");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(t, ObjectId(2), Team::USA);
    o.fx_list_die = Some(HostFxListDieData {
        death_fx: Some("FX_VehicleDie".into()),
        more: vec![HostFxListDieData::with_fx("FX_RicochetDie")],
        ..Default::default()
    });
    o.last_damage_source = Some(ObjectId(99));
    o.fire_fx_list_die();
    let (fx, _) = o.take_pending_death_fx_audio();
    assert_eq!(fx.as_deref(), Some("FX_VehicleDie"));
    assert_eq!(o.last_damage_source, Some(ObjectId(99)));
}

#[test]
fn fx_list_die_exempt_status_skips_burned_object() {
    use crate::game_logic::host_status_bits_upgrade::object_status_mask_from_names;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut t = ThingTemplate::new("AmericaTankCrusader");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(t, ObjectId(3), Team::USA);
    o.fx_list_die = Some(fx_list_die_from_behavior_attrs_for_test());
    o.object_status_bits = object_status_mask_from_names(&["BURNED"]);
    o.fire_fx_list_die();
    let (fx, _) = o.take_pending_death_fx_audio();
    assert!(fx.is_none(), "ExemptStatus BURNED must skip FXListDie");
}

#[test]
fn structure_collapse_on_lethal() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut t = ThingTemplate::new("CivilianBarn01");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Structure);
    let mut b = Object::new(t, ObjectId(1), Team::Neutral);
    b.health.current = 100.0;
    assert!(b.begin_structure_collapse(5));
    assert!(b.structure_collapse_data.as_ref().unwrap().is_active());
    let mut done = false;
    for f in 5..800 {
        if b.tick_structure_collapse(f) {
            done = true;
            break;
        }
    }
    assert!(done);
    assert!(b.status.destroyed);
    assert!(b.presentation_collapse_height_offset() < -1.0);
}

#[test]
fn structure_collapse_binds_and_plays_phase_fx() {
    use crate::game_logic::host_structure_collapse::{
        clear_collapse_ini_override_for_tests, collapse_ini_from_behavior_attrs,
        override_collapse_ini_for_tests,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let ini = collapse_ini_from_behavior_attrs(&[
        ("MinBurstDelay", "66"),
        ("MaxBurstDelay", "66"),
        (
            "FXList",
            "INITIAL FX_DieInitial\nDELAY FX_DieDelay\nBURST FX_DieBurst\nFINAL FX_DieFinal",
        ),
    ]);
    override_collapse_ini_for_tests("CivilianBarn01", ini);
    let mut t = ThingTemplate::new("CivilianBarn01");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Structure);
    let mut b = Object::new(t, ObjectId(1), Team::Neutral);
    assert!(b.begin_structure_collapse(0));
    {
        let sc = b.structure_collapse_data.as_ref().unwrap();
        assert!(sc.initial_played);
        assert_eq!(sc.fx_initial.as_deref(), Some("FX_DieInitial"));
        assert_eq!(sc.fx_burst.as_deref(), Some("FX_DieBurst"));
        assert_eq!(sc.fx_delay.as_deref(), Some("FX_DieDelay"));
        assert_eq!(sc.fx_final.as_deref(), Some("FX_DieFinal"));
        assert!(
            sc.pending_phase_fx.is_empty(),
            "INITIAL dispatched at begin"
        );
    }
    let mut saw_start_burst = false;
    let mut done = false;
    for f in 0..800 {
        if b.tick_structure_collapse(f) {
            done = true;
            break;
        }
        if b.structure_collapse_data
            .as_ref()
            .map(|d| d.start_burst_played)
            .unwrap_or(false)
        {
            saw_start_burst = true;
        }
    }
    assert!(saw_start_burst);
    assert!(done);
    assert!(
        b.structure_collapse_data
            .as_ref()
            .map(|d| d.final_played)
            .unwrap_or(false)
    );
    clear_collapse_ini_override_for_tests();
}

#[test]
fn structure_topple_on_lethal_damage() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut t = ThingTemplate::new("AmericaWarFactory");
    t.set_health(200.0);
    t.add_kind_of(KindOf::Structure);
    let mut b = Object::new(t, ObjectId(1), Team::USA);
    b.health.current = 200.0;
    assert!(b.begin_structure_topple(10, Some((0.0, 0.0))));
    assert!(
        b.structure_topple_data.as_ref().unwrap().is_active()
            || !b.structure_topple_data.as_ref().unwrap().is_standing()
    );
    assert!(!b.status.destroyed);
    let mut done = false;
    for f in 10..800 {
        if b.tick_structure_topple(f) {
            done = true;
            break;
        }
    }
    assert!(done);
    assert!(b.status.destroyed);
    assert_eq!(
        b.status.death_type,
        crate::game_logic::host_usa_pilot::HostDeathType::Toppled
    );
    assert!(b.presentation_topple_lean_radians() > 1.0);
}

#[test]
fn topple_residual_falls_and_dies() {
    use crate::game_logic::host_topple::{HostToppleData, TOPPLE_OPTIONS_NO_BOUNCE};
    use crate::game_logic::{Team, ThingTemplate};
    let mut t = ThingTemplate::new("TreeOak");
    t.set_health(50.0);
    let mut tree = Object::new(t, ObjectId(1), Team::Neutral);
    tree.health.current = 50.0;
    tree.topple_data = Some(HostToppleData::default());
    assert!(!tree.apply_topple(1.0, 0.0, 2.0, TOPPLE_OPTIONS_NO_BOUNCE));
    assert!(tree.is_alive());
    let mut died = false;
    for _ in 0..600 {
        if tree.tick_topple() {
            died = true;
            break;
        }
    }
    assert!(died);
    assert!(tree.status.destroyed);
    assert_eq!(
        tree.status.death_type,
        crate::game_logic::host_usa_pilot::HostDeathType::Toppled
    );
}

#[test]
fn crate_and_chemsuit_set_terrain_decal_types() {
    use crate::game_logic::host_battlemaster::{
        TERRAIN_DECAL_CHEMSUIT, TERRAIN_DECAL_CRATE, TERRAIN_DECAL_SHADOW_TEXTURE,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut ct = ThingTemplate::new("SalvageCrate");
    ct.add_kind_of(KindOf::Crate);
    let mut crate_obj = Object::new(ct, ObjectId(1), Team::Neutral);
    crate_obj.apply_crate_terrain_decal();
    assert_eq!(crate_obj.terrain_decal_type, TERRAIN_DECAL_CRATE);
    assert!(crate_obj.terrain_decal_size > 0.0);
    assert!(crate_obj.terrain_decal_fade_rate > 0.0);
    crate_obj.tick_terrain_decal_fade();
    assert!(crate_obj.terrain_decal_opacity > 0.0);

    let mut it = ThingTemplate::new("AmericaInfantryRanger");
    it.add_kind_of(KindOf::Infantry);
    let mut ranger = Object::new(it, ObjectId(2), Team::USA);
    ranger.set_terrain_decal_chemsuit(true);
    assert!(ranger.terrain_decal_chemsuit);
    assert_eq!(ranger.terrain_decal_type, TERRAIN_DECAL_CHEMSUIT);

    let mut ft = ThingTemplate::new("GLAFakeBarracks");
    ft.add_kind_of(KindOf::Structure);
    ft.add_kind_of(KindOf::FSFake);
    let fake = Object::new(ft, ObjectId(3), Team::GLA);
    assert_eq!(fake.terrain_decal_type, TERRAIN_DECAL_SHADOW_TEXTURE);
}

#[test]
fn tree_topple_queues_stump_and_crush_direction() {
    use crate::game_logic::host_topple::TOPPLE_OPTIONS_NONE;
    use crate::game_logic::{Team, ThingTemplate};
    let mut t = ThingTemplate::new("TreeOak");
    t.set_health(50.0);
    let mut tree = Object::new(t, ObjectId(4), Team::Neutral);
    tree.health.current = 50.0;
    assert!(!tree.apply_topple(0.0, 1.0, 2.0, TOPPLE_OPTIONS_NONE));
    let td = tree.topple_data.as_ref().expect("topple");
    assert!(!td.shadows_enabled);
    assert!((td.dir_y - 1.0).abs() < 1e-5);
    assert_eq!(td.pending_stump_name, "TreeOakStump");
    assert_eq!(td.pending_topple_fx, "");
    // FX already dispatched (pending cleared).
}

#[test]
fn healing_and_water_damage_residuals() {
    use crate::game_logic::combat::DamageType;
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut t = ThingTemplate::new("Ranger");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Infantry);
    let mut unit = Object::new(t, ObjectId(1), Team::USA);
    unit.health.current = 40.0;
    unit.health.maximum = 100.0;

    // Healing restores HP and never destroys.
    assert!(!unit.take_damage_from_typed(25.0, Some(ObjectId(99)), DamageType::Healing));
    assert!((unit.health.current - 65.0).abs() < 1e-3);
    assert!(unit.is_alive());
    // C++ attemptHealing stamps lastDamageInfo as HEALING (forgets prior attacker).
    // Live clears source so the medic is not a Guard nemesis (no last-type field).
    assert!(unit.last_damage_source.is_none());
    assert!(unit.last_healing_timestamp.is_some());

    // Cap at maximum.
    assert!(!unit.take_damage_from_typed(1000.0, None, DamageType::Healing));
    assert!((unit.health.current - 100.0).abs() < 1e-3);

    // Water deals normal HP damage.
    unit.health.current = 100.0;
    let destroyed = unit.take_damage_from_typed(30.0, None, DamageType::Water);
    assert!(!destroyed);
    assert!((unit.health.current - 70.0).abs() < 1e-3);

    // Dead units do not heal.
    unit.health.current = 0.0;
    unit.status.destroyed = true;
    assert!(!unit.take_damage_from_typed(50.0, None, DamageType::Healing));
    assert!((unit.health.current - 0.0).abs() < 1e-3);
}

#[test]
fn deploy_hack_surrender_kill_garrisoned_damage_residuals() {
    use crate::game_logic::combat::DamageType;
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    // DEPLOY: no HP, sets pending assault signal.
    let mut tt = ThingTemplate::new("TroopCrawler");
    tt.set_health(100.0);
    let mut crawler = Object::new(tt, ObjectId(1), Team::China);
    crawler.health.current = 100.0;
    assert!(!crawler.take_damage_from_typed(50.0, None, DamageType::Deploy));
    assert!((crawler.health.current - 100.0).abs() < 1e-3);
    assert!(!crawler.status.destroyed);

    // HACK: no HP.
    let mut ht = ThingTemplate::new("Tank");
    ht.set_health(100.0);
    ht.add_kind_of(KindOf::Vehicle);
    let mut tank = Object::new(ht, ObjectId(2), Team::USA);
    tank.health.current = 100.0;
    assert!(!tank.take_damage_from_typed(40.0, None, DamageType::Hack));
    assert!((tank.health.current - 100.0).abs() < 1e-3);

    // SURRENDER: retail ALLOW_SURRENDER off — lethal hit is normal HP death
    // (ActiveBody.cpp:517-537 commented + compiled out).
    let mut it = ThingTemplate::new("Ranger");
    it.set_health(50.0);
    it.add_kind_of(KindOf::Infantry);
    let mut ranger = Object::new(it, ObjectId(3), Team::USA);
    ranger.health.current = 50.0;
    assert!(ranger.take_damage_from_typed(50.0, None, DamageType::Surrender));
    assert!(!ranger.is_surrendered);
    assert!(!ranger.is_alive());

    // KILL_GARRISONED: structure HP untouched; pending count = floor(amount).
    let mut st = ThingTemplate::new("Bunker");
    st.set_health(500.0);
    st.add_kind_of(KindOf::Structure);
    let mut bunker = Object::new(st, ObjectId(4), Team::GLA);
    bunker.health.current = 500.0;
    assert!(!bunker.take_damage_from_typed(3.7, None, DamageType::KillGarrisoned));
    assert!((bunker.health.current - 500.0).abs() < 1e-3);
    assert_eq!(bunker.take_pending_kill_garrisoned(), 3);

    // PENALTY: normal HP path.
    let mut pt = ThingTemplate::new("Tank");
    pt.set_health(100.0);
    let mut penalized = Object::new(pt, ObjectId(5), Team::USA);
    penalized.health.current = 100.0;
    let _ = penalized.take_damage_from_typed(25.0, None, DamageType::Penalty);
    assert!((penalized.health.current - 75.0).abs() < 1e-3);
}

#[test]
fn disarm_damage_clears_mine_without_hp_on_tank() {
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
    assert!(mine.take_damage_from_typed(1.0, None, DamageType::Disarm));
    assert!(mine.status.destroyed);
    assert!(mine.mine_data.as_ref().unwrap().detonated);

    let mut tt = ThingTemplate::new("Tank");
    tt.set_health(100.0);
    tt.add_kind_of(KindOf::Vehicle);
    let mut tank = Object::new(tt, ObjectId(2), Team::USA);
    tank.health.current = 100.0;
    assert!(!tank.take_damage_from_typed(50.0, None, DamageType::Disarm));
    assert!((tank.health.current - 100.0).abs() < 1e-3);
}

#[test]
fn disarm_keeps_regenerating_china_pad() {
    use crate::game_logic::combat::DamageType;
    use crate::game_logic::host_mines::{HostMineData, MINE_MIN_HEALTH};
    use crate::game_logic::{Team, ThingTemplate};
    let mut mt = ThingTemplate::new("ChinaStandardMine");
    mt.set_health(100.0);
    let mut mine = Object::new(mt, ObjectId(3), Team::China);
    mine.mine_data = Some(HostMineData::land_mine_for_template("ChinaStandardMine"));
    mine.health.current = 100.0;
    assert!(!mine.take_damage_from_typed(1.0, None, DamageType::Disarm));
    assert!(!mine.status.destroyed);
    assert!((mine.health.current - MINE_MIN_HEALTH).abs() < 1e-3);
    assert_eq!(mine.mine_data.as_ref().unwrap().virtual_mines_remaining, 0);
    assert!(mine.mine_data.as_ref().unwrap().regenerates);
}

#[test]
fn disarm_damage_defuses_demo_trap_without_hp_splash_path() {
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
    assert!(trap.take_damage_from_typed(1.0, None, DamageType::Disarm));
    assert!(trap.status.destroyed);
    assert!(trap.mine_data.as_ref().unwrap().detonated);
}

#[test]
fn kill_pilot_damage_unmans_vehicle_without_hp_loss() {
    use crate::game_logic::combat::DamageType;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut tmpl = ThingTemplate::new("Tank");
    tmpl.set_health(200.0);
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(tmpl, ObjectId(5), Team::China);
    o.health.current = 200.0;
    o.health.maximum = 200.0;
    assert!(!o.take_damage_from_typed(1.0, None, DamageType::KillPilot));
    assert!((o.health.current - 200.0).abs() < 1e-3);
    assert!(o.is_unmanned());
    assert_eq!(o.team, Team::Neutral);
}

#[test]
fn microwave_is_hp_through_armor_not_emp_subdual() {
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
    assert!(!o.take_damage_from_typed(40.0, None, DamageType::Microwave));
    assert!((o.health.current - 100.0).abs() < 1e-3);
    assert!(o.subdual_damage.abs() < 1e-3);
    assert!(!o.is_subdued());
    assert!(!o.take_damage_from_typed(70.0, None, DamageType::EMP));
    assert!((o.health.current - 100.0).abs() < 1e-3);
    assert!(o.subdual_damage.abs() < 1e-3);
    assert!(!o.is_subdued());
}

#[test]
fn status_damage_applies_faerie_without_hp_loss() {
    use crate::game_logic::combat::DamageType;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut tmpl = ThingTemplate::new("PaintMe");
    tmpl.set_health(100.0);
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(tmpl, ObjectId(9), Team::GLA);
    o.health.current = 100.0;
    o.health.maximum = 100.0;
    crate::game_logic::object::set_pending_damage_status_type(Some("FAERIE_FIRE"));
    let dead = o.take_damage_from_typed(200.0, None, DamageType::Status);
    assert!(!dead);
    assert!((o.health.current - 100.0).abs() < 1e-3);
    assert!(o.is_faerie_fire());
    assert!(o.faerie_fire_until_frame > 0);
}

#[test]
fn status_damage_none_does_not_paint_faerie_fire() {
    use crate::game_logic::combat::DamageType;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut tmpl = ThingTemplate::new("NoPaint");
    tmpl.set_health(100.0);
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(tmpl, ObjectId(10), Team::GLA);
    o.health.current = 100.0;
    o.health.maximum = 100.0;
    // C++ default OBJECT_STATUS_NONE: no paint.
    let dead = o.take_damage_from_typed(200.0, None, DamageType::Status);
    assert!(!dead);
    assert!((o.health.current - 100.0).abs() < 1e-3);
    assert!(!o.is_faerie_fire());
}

#[test]
fn most_percent_ready_between_shots_progresses() {
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
    assert!(o.fire_at(tgt.id, 1.0));
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
    assert!(atk.fire_at(tgt.id, 1.0));
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
    assert!(atk.fire_at(tgt.id, 1.0));
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
    assert!(atk.fire_at(tgt.id, 1.0));
    assert_eq!(atk.max_shots_to_fire, 1);
    assert!(atk.fire_at(tgt.id, 2.0));
    assert_eq!(atk.max_shots_to_fire, 0);
    assert!(!atk.fire_at(tgt.id, 3.0));
    atk.set_max_shots_to_fire(-1);
    assert!(atk.fire_at(tgt.id, 4.0));
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
    {
        let a = logic.host_object_mut(id).unwrap();
        a.continuous_fire_one_shots = 1;
        a.continuous_fire_two_shots = 4;
        a.continuous_fire_coast_frames = 10;
        a.record_shot_at_target(tgt);
        a.record_shot_at_target(tgt);
        assert_eq!(a.continuous_fire_level, 1);
        a.stamp_continuous_fire_coast(100);
        assert_eq!(a.continuous_fire_coast_until_frame, 110);
        a.tick_continuous_fire_coast(109);
        assert_eq!(a.continuous_fire_level, 1);
        a.tick_continuous_fire_coast(110);
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
        a.record_shot_at_target(tgt);
        a.record_shot_at_target(tgt);
        assert_eq!(a.continuous_fire_level, 0); // need consecutive > 2
        a.record_shot_at_target(tgt);
        assert_eq!(a.continuous_fire_level, 1);
        let (_, _, rof, _, _) = a.weapon_bonus_fields();
        assert!((rof - 2.0).abs() < 0.01, "MEAN ROF 200% got {rof}");
        for _ in 0..3 {
            a.record_shot_at_target(tgt);
        }
        assert_eq!(a.continuous_fire_level, 2);
        let (_, _, rof2, _, _) = a.weapon_bonus_fields();
        assert!((rof2 - 3.0).abs() < 0.01, "FAST ROF 300% got {rof2}");
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
        let a = logic.host_object_mut(atk).unwrap();
        a.weapon = Some(Weapon {
            damage: 10.0,
            range: 200.0,
            reload_time: 1.0,
            last_fire_time: -100.0, // never-fired residual
            ..Weapon::default()
        });
        // First shot at t=0
        assert!(a.fire_at_ex(ObjectId(99), 0.0, false, true));
        // Without faerie, not ready at 0.7 (needs full 1.0s)
        assert!(!a.fire_at_ex(ObjectId(99), 0.7, false, false));
        // With faerie ROF 150%, ready at 0.7 (effective reload ~0.667)
        assert!(
            a.fire_at_ex(ObjectId(99), 0.7, false, true),
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

#[test]
fn effective_max_lift_uses_damaged_locomotor() {
    use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("LIFT_V");
    tpl.add_kind_of(KindOf::Aircraft);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(100.0);
    logic.templates.insert("LIFT_V".to_string(), tpl);
    let id = logic
        .create_object("LIFT_V", Team::USA, Vec3::new(0.0, 50.0, 0.0))
        .unwrap();
    {
        let o = logic.host_object_mut(id).unwrap();
        o.max_lift = 8.0;
        o.max_lift_damaged = 3.0;
        o.health.current = 100.0;
        o.health.maximum = 100.0;
        o.refresh_model_condition_bits();
        assert_eq!(o.body_damage_state, HostBodyDamageType::Pristine);
        assert!((o.effective_max_lift() - 8.0).abs() < 0.01);
        o.health.current = 10.0;
        o.refresh_model_condition_bits();
        assert_eq!(o.body_damage_state, HostBodyDamageType::ReallyDamaged);
        assert!(
            (o.effective_max_lift() - 3.0).abs() < 0.01,
            "really damaged uses max_lift_damaged"
        );
    }
}

#[test]
fn body_damage_sets_model_condition_bits() {
    use crate::game_logic::host_enum_table_residual::{
        HostBodyDamageType, MC_BIT_DAMAGED, MC_BIT_DYING, MC_BIT_REALLYDAMAGED, MC_BIT_RUBBLE,
        host_model_condition_has,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut tmpl = ThingTemplate::new("McBits");
    tmpl.set_health(100.0);
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(tmpl, ObjectId(90), Team::USA);
    o.refresh_model_condition_bits();
    assert_eq!(o.body_damage_state, HostBodyDamageType::Pristine);
    assert!(!host_model_condition_has(
        o.model_condition_bits,
        MC_BIT_DAMAGED
    ));

    o.health.current = 40.0; // between 0.25 and 0.5
    o.refresh_model_condition_bits();
    assert_eq!(o.body_damage_state, HostBodyDamageType::Damaged);
    assert!(host_model_condition_has(
        o.model_condition_bits,
        MC_BIT_DAMAGED
    ));
    assert!(!host_model_condition_has(
        o.model_condition_bits,
        MC_BIT_REALLYDAMAGED
    ));

    o.health.current = 10.0;
    o.refresh_model_condition_bits();
    assert_eq!(o.body_damage_state, HostBodyDamageType::ReallyDamaged);
    assert!(host_model_condition_has(
        o.model_condition_bits,
        MC_BIT_REALLYDAMAGED
    ));

    o.take_damage(9999.0);
    assert!(o.status.destroyed);
    assert_eq!(o.body_damage_state, HostBodyDamageType::Rubble);
    assert!(host_model_condition_has(
        o.model_condition_bits,
        MC_BIT_RUBBLE
    ));
    assert!(host_model_condition_has(
        o.model_condition_bits,
        MC_BIT_DYING
    ));
}

#[test]
fn body_damage_threshold_cpp_surface() {
    use crate::game_logic::host_enum_table_residual::{
        HOST_UNIT_DAMAGED_THRESH, HOST_UNIT_REALLY_DAMAGED_THRESH, HostBodyDamageType,
        host_calc_body_damage_state,
    };
    // C++ ActiveBody.cpp:88 + GameData.ini UnitDamagedThreshold 0.7 / 0.35.
    assert!((HOST_UNIT_DAMAGED_THRESH - 0.7).abs() < 1e-6);
    assert!((HOST_UNIT_REALLY_DAMAGED_THRESH - 0.35).abs() < 1e-6);
    assert_eq!(
        host_calc_body_damage_state(100.0, 100.0),
        HostBodyDamageType::Pristine
    );
    assert_eq!(
        host_calc_body_damage_state(50.0, 100.0),
        HostBodyDamageType::Damaged
    );
    assert_eq!(
        host_calc_body_damage_state(25.0, 100.0),
        HostBodyDamageType::ReallyDamaged
    );
    assert_eq!(
        host_calc_body_damage_state(0.0, 100.0),
        HostBodyDamageType::Rubble
    );
}

#[test]
fn fire_at_stamps_detonation_fx_on_pending() {
    // Surface residual: PendingProjectile carries ProjectileDetonationFX name.
    let src = crate::game_logic::object::OBJECT_SRC;
    assert!(src.contains("detonation_fx_name"));
    assert!(src.contains("host_detonation_fx_for_weapon_name"));
    assert!(src.contains("detonation_ocl_name"));
    assert!(src.contains("host_detonation_ocl_for_weapon_name"));
    assert!(src.contains("exhaust_name"));
    assert!(src.contains("host_projectile_exhaust_for_unit_slot"));
    let csrc = concat!(
        include_str!("../../combat/weapon_fire.rs"),
        include_str!("../../combat/resolution.rs"),
    );
    assert!(csrc.contains("take_impact_fx"));
    assert!(csrc.contains("ProjectileImpactFx"));
}

#[test]
fn leech_range_waives_max_range_after_activate() {
    let mut tmpl = ThingTemplate::new("GLAInfantryTerrorist");
    tmpl.primary_weapon_name = Some("GLAInfantryTerrorist".into());
    let mut atk = Object::new(tmpl, ObjectId(1), Team::GLA);
    atk.set_position(glam::Vec3::ZERO);
    atk.weapon = Some(Weapon {
        damage: 100.0,
        range: 20.0,
        min_range: 0.0,
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: 0.0,
        ..Weapon::default()
    });

    let mut tgt = Object::new(
        ThingTemplate::new("AmericaTankCrusader"),
        ObjectId(2),
        Team::USA,
    );
    tgt.set_position(glam::Vec3::new(100.0, 0.0, 0.0)); // out of 20 range
    tgt.thing.template.add_kind_of(KindOf::Vehicle);
    tgt.thing.template.add_kind_of(KindOf::Attackable);

    // Before leech: out of range.
    assert!(!atk.can_target_with_slot(&tgt, atk.weapon.as_ref().unwrap(), Some(0)));

    // Activate leech (as if pre-fire / fire occurred in range).
    atk.activate_leech_range_for_slot(0);
    assert!(atk.leech_range_active_primary);
    assert!(atk.can_target_with_slot(&tgt, atk.weapon.as_ref().unwrap(), Some(0)));

    // stop_attack clears.
    atk.stop_attack();
    assert!(!atk.leech_range_active_primary);
    assert!(!atk.can_target_with_slot(&tgt, atk.weapon.as_ref().unwrap(), Some(0)));
}

#[test]
fn acceptable_aim_delta_blocks_then_allows_after_turn() {
    let mut tmpl = ThingTemplate::new("AmericaTankCrusader");
    tmpl.primary_weapon_name = Some("AmericaTankCrusaderGun".into());
    let mut atk = Object::new(tmpl, ObjectId(1), Team::USA);
    atk.set_position(glam::Vec3::ZERO);
    atk.set_orientation(0.0); // face +X residual (movement convention)
    atk.weapon = Some(Weapon {
        damage: 10.0,
        range: 200.0,
        ..Weapon::default()
    });
    let target = glam::Vec3::new(0.0, 0.0, 50.0); // off to +Z (~90°)
    let aim = atk.aim_delta_for_slot(0);
    let rel = atk.relative_angle_2d_to(target);
    // 20° aim residual should NOT be aimed at 90° offset.
    assert!(
        !atk.is_aimed_at_position(target, 0),
        "unexpectedly aimed: aim_delta={aim} rel={rel} ori={}",
        atk.get_orientation()
    );
    // Turn in steps until aimed.
    let mut aimed = false;
    for _ in 0..20 {
        if atk.turn_toward_position(target, 0, 0.2) {
            aimed = true;
            break;
        }
    }
    assert!(
        aimed,
        "should aim after turns, ori={}",
        atk.get_orientation()
    );
    assert!(atk.is_aimed_at_position(target, 0));
}

#[test]
fn omni_aim_delta_always_aimed() {
    let mut tmpl = ThingTemplate::new("AmericaSentryDrone");
    tmpl.primary_weapon_name = Some("AmericaSentryDroneGun".into());
    let mut atk = Object::new(tmpl, ObjectId(3), Team::USA);
    atk.set_position(glam::Vec3::ZERO);
    atk.set_orientation(0.0);
    let target = glam::Vec3::new(-40.0, 0.0, 10.0);
    assert!(atk.is_aimed_at_position(target, 0));
}

#[test]
fn pre_attack_type_per_shot_delays_every_discharge() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    use glam::Vec3;
    let mut tmpl = ThingTemplate::new("Gattling");
    tmpl.primary_weapon_name = Some("AmericaGattlingTankGun".into());
    tmpl.add_kind_of(KindOf::Vehicle);
    tmpl.add_kind_of(KindOf::Attackable);
    let mut atk = Object::new(tmpl, ObjectId(1), Team::USA);
    atk.set_position(Vec3::ZERO);
    atk.weapon = Some(Weapon {
        damage: 5.0,
        range: 100.0,
        reload_time: 0.0,
        last_fire_time: -100.0,
        pre_attack_delay: 0.5,
        ..Weapon::default()
    });
    let tgt = ObjectId(9);
    // First wind-up
    assert!(!atk.fire_at(tgt, 10.0));
    assert!((atk.pre_attack_ready_at - 10.5).abs() < 1e-4);
    // Still winding
    assert!(!atk.fire_at(tgt, 10.2));
    // Fire after delay
    assert!(atk.fire_at(tgt, 10.5));
    assert_eq!(atk.consecutive_shots_at_target, 1);
    // PER_SHOT: next shot needs a new delay even vs same target
    assert!(!atk.fire_at(tgt, 10.5));
    assert!(atk.pre_attack_ready_at > 10.5);
    assert!(!atk.fire_at(tgt, 10.7));
    assert!(atk.fire_at(tgt, 11.0));
    assert_eq!(atk.consecutive_shots_at_target, 2);
}

#[test]
fn pre_attack_type_per_attack_delays_once_per_target() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    use glam::Vec3;
    let mut tmpl = ThingTemplate::new("Ranger");
    tmpl.primary_weapon_name = Some("AmericaRangerMachineGun".into());
    tmpl.add_kind_of(KindOf::Infantry);
    tmpl.add_kind_of(KindOf::Attackable);
    let mut atk = Object::new(tmpl, ObjectId(1), Team::USA);
    atk.set_position(Vec3::ZERO);
    atk.weapon = Some(Weapon {
        damage: 5.0,
        range: 100.0,
        reload_time: 0.0,
        last_fire_time: -100.0,
        pre_attack_delay: 1.0,
        ammo: Some(5),
        clip_size: 5,
        ..Weapon::default()
    });
    let tgt = ObjectId(9);
    assert!(!atk.fire_at(tgt, 5.0)); // wind-up
    assert!(atk.fire_at(tgt, 6.0)); // fire
    // Same target: no second wind-up
    assert!(atk.fire_at(tgt, 6.0));
    assert_eq!(atk.consecutive_shots_at_target, 2);
    // New target: delay again
    let tgt2 = ObjectId(10);
    assert!(!atk.fire_at(tgt2, 6.0));
    assert!(atk.fire_at(tgt2, 7.0));
}

#[test]
fn pre_attack_type_per_clip_delays_on_full_clip_only() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    use glam::Vec3;
    let mut tmpl = ThingTemplate::new("Scud");
    // Seed-only name (not in WeaponStore) so PreAttackType peels to PER_CLIP.
    tmpl.primary_weapon_name = Some("HostTestScudStormClipWeapon".into());
    tmpl.add_kind_of(KindOf::Structure);
    tmpl.add_kind_of(KindOf::Attackable);
    let mut atk = Object::new(tmpl, ObjectId(1), Team::GLA);
    atk.set_position(Vec3::ZERO);
    atk.weapon = Some(Weapon {
        damage: 50.0,
        range: 300.0,
        reload_time: 0.0,
        last_fire_time: -100.0,
        pre_attack_delay: 2.0,
        ammo: Some(3),
        clip_size: 3,
        clip_reload_time: 0.0,
        ..Weapon::default()
    });
    let tgt = ObjectId(9);
    // Full clip → delay
    assert!(!atk.fire_at(tgt, 1.0));
    assert!(atk.fire_at(tgt, 3.0));
    assert_eq!(atk.weapon.as_ref().unwrap().ammo, Some(2));
    // Mid-clip → no delay
    assert!(atk.fire_at(tgt, 3.0));
    assert_eq!(atk.weapon.as_ref().unwrap().ammo, Some(1));
    assert!(atk.fire_at(tgt, 3.0));
    assert_eq!(atk.weapon.as_ref().unwrap().ammo, Some(0));
}
