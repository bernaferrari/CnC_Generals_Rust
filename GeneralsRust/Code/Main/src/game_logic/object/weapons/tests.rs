use super::*;

fn weapon(damage: f32) -> Weapon {
    Weapon {
        damage,
        range: 200.0,
        last_fire_time: -10.0,
        ..Weapon::default()
    }
}

#[test]
fn auto_chooser_never_promotes_a_tertiary_weapon() {
    let mut attacker = Object::new(
        ThingTemplate::new("ThreeSlotAttacker"),
        ObjectId(1),
        Team::USA,
    );
    attacker.weapon = Some(weapon(5.0));
    attacker.tertiary_weapon = Some(weapon(999.0));
    let mut target = Object::new(ThingTemplate::new("Target"), ObjectId(2), Team::GLA);
    target.set_position(glam::Vec3::new(50.0, 0.0, 0.0));

    assert_eq!(attacker.select_combat_weapon_slot(&target, 1.0), Some(0));

    attacker.weapon = None;
    assert_eq!(
        attacker.select_combat_weapon_slot(&target, 1.0),
        None,
        "a tertiary-only unit needs an explicit player slot selection"
    );

    attacker.set_active_weapon_slot(2);
    assert_eq!(attacker.select_combat_weapon_slot(&target, 1.0), Some(2));
}

#[test]
fn unknown_weapon_slot_fails_closed() {
    let mut object = Object::new(
        ThingTemplate::new("ThreeSlotAttacker"),
        ObjectId(1),
        Team::USA,
    );
    object.weapon = Some(weapon(5.0));
    assert!(object.weapon_slot(99).is_none());
    assert!(object.weapon_slot_mut(99).is_none());
    assert!(!object.set_weapon_lock(99, WeaponLockType::LockedPermanently));
}

#[test]
fn preferred_against_primary_infantry_beats_higher_damage_secondary() {
    // C++ WeaponSet.cpp:869-877 — Comanche cannon vs Hellfire: PRIMARY
    // PreferredAgainst INFANTRY wins even when secondary damage is larger.
    let mut attacker = Object::new(
        {
            let mut t = ThingTemplate::new("AmericaVehicleComanche");
            t.preferred_against[0] = vec![KindOf::Infantry];
            t
        },
        ObjectId(1),
        Team::USA,
    );
    attacker.weapon = Some(weapon(10.0));
    attacker.secondary_weapon = Some(weapon(100.0));
    let mut target = Object::new(
        {
            let mut t = ThingTemplate::new("Infantry");
            t.add_kind_of(KindOf::Infantry);
            t
        },
        ObjectId(2),
        Team::GLA,
    );
    target.set_position(glam::Vec3::new(50.0, 0.0, 0.0));
    assert_eq!(attacker.select_combat_weapon_slot(&target, 1.0), Some(0));
}

#[test]
fn preferred_against_secondary_infantry_beats_higher_damage_primary() {
    // C++ SCUD toxin: PreferredAgainst SECONDARY INFANTRY.
    let mut attacker = Object::new(
        {
            let mut t = ThingTemplate::new("GLAVehicleSCUDLauncher");
            t.preferred_against[1] = vec![KindOf::Infantry];
            t
        },
        ObjectId(1),
        Team::GLA,
    );
    attacker.weapon = Some(weapon(300.0));
    attacker.secondary_weapon = Some(weapon(50.0));
    let mut target = Object::new(
        {
            let mut t = ThingTemplate::new("Infantry");
            t.add_kind_of(KindOf::Infantry);
            t
        },
        ObjectId(2),
        Team::USA,
    );
    target.set_position(glam::Vec3::new(50.0, 0.0, 0.0));
    assert_eq!(attacker.select_combat_weapon_slot(&target, 1.0), Some(1));
}

#[test]
fn share_weapon_reload_time_syncs_sibling_last_fire() {
    // C++ Weapon.cpp:2655-2665 ShareWeaponReloadTime copies next-shot frame.
    let mut attacker = Object::new(
        {
            let mut t = ThingTemplate::new("SharedReload");
            t.share_weapon_reload_time = true;
            t
        },
        ObjectId(1),
        Team::USA,
    );
    attacker.weapon = Some(Weapon {
        last_fire_time: 4.0,
        ..weapon(10.0)
    });
    attacker.secondary_weapon = Some(Weapon {
        last_fire_time: 0.0,
        ..weapon(10.0)
    });
    attacker.sync_shared_weapon_reload();
    assert_eq!(attacker.weapon.as_ref().unwrap().last_fire_time, 4.0);
    assert_eq!(
        attacker.secondary_weapon.as_ref().unwrap().last_fire_time,
        4.0
    );
}

#[test]
fn share_weapon_reload_time_copies_firing_slot_next_shot() {
    // C++ Weapon.cpp:2655-2665: siblings wait the firing slot delay,
    // not their own DelayBetweenShots / ClipReloadTime.
    let mut attacker = Object::new(
        {
            let mut t = ThingTemplate::new("AmericaVehicleComanche");
            t.share_weapon_reload_time = true;
            t
        },
        ObjectId(1),
        Team::USA,
    );
    attacker.weapon = Some(Weapon {
        last_fire_time: 4.0,
        reload_time: 0.2,
        ..weapon(10.0)
    });
    attacker.secondary_weapon = Some(Weapon {
        last_fire_time: 0.0,
        reload_time: 3.0,
        ..weapon(10.0)
    });
    attacker.sync_shared_weapon_reload();
    let gun = attacker.weapon.as_ref().unwrap();
    let pods = attacker.secondary_weapon.as_ref().unwrap();
    let gun_ready = gun.last_fire_time + 0.2;
    let pods_ready = pods.last_fire_time + 3.0;
    assert!(
        (gun_ready - pods_ready).abs() < 1e-4,
        "{gun_ready} vs {pods_ready}"
    );
    assert!((gun.last_fire_time - 4.0).abs() < 1e-4);
}

#[test]
fn auto_choose_none_secondary_is_not_picked_while_primary_reloads() {
    let mut attacker = Object::new(
        ThingTemplate::new("GLAInfantryJarmenKell"),
        ObjectId(1),
        Team::GLA,
    );
    assert!(
        !attacker.thing.template.slot_allows_auto_choose(1),
        "Jarmen secondary is AutoChooseSources NONE"
    );
    attacker.weapon = Some(Weapon {
        last_fire_time: 1.0,
        reload_time: 1.0,
        ..weapon(10.0)
    });
    attacker.secondary_weapon = Some(Weapon {
        last_fire_time: -10.0,
        reload_time: 0.0,
        damage: 100.0,
        range: 200.0,
        ..Weapon::default()
    });
    let mut target = Object::new(ThingTemplate::new("Tank"), ObjectId(2), Team::USA);
    target.set_position(glam::Vec3::new(50.0, 0.0, 0.0));
    assert_eq!(
        attacker.select_combat_weapon_slot(&target, 1.1),
        Some(0),
        "leftover backup keeps reloading PRIMARY; AutoChoose NONE must not snipe"
    );
}

#[test]
fn share_weapon_reload_marks_sibling_reloading_clip() {
    let mut attacker = Object::new(
        {
            let mut t = ThingTemplate::new("AmericaVehicleComanche");
            t.share_weapon_reload_time = true;
            t
        },
        ObjectId(1),
        Team::USA,
    );
    attacker.weapon = Some(Weapon {
        last_fire_time: 4.0,
        clip_size: 12,
        ammo: Some(0),
        clip_reload_time: 2.0,
        reloading_clip: true,
        ..weapon(10.0)
    });
    attacker.secondary_weapon = Some(Weapon {
        last_fire_time: 0.0,
        clip_size: 8,
        ammo: Some(8),
        clip_reload_time: 10.0,
        ..weapon(40.0)
    });
    attacker.sync_shared_weapon_reload();
    assert!(
        attacker.secondary_weapon.as_ref().unwrap().reloading_clip,
        "C++ setStatus(RELOADING_CLIP) on every sibling"
    );
    assert_eq!(attacker.secondary_weapon.as_ref().unwrap().ammo, Some(8));
    attacker.active_weapon_slot = 1;
    attacker.refresh_weapon_fire_status(4.5);
    assert!(
        attacker.secondary_weapon.as_ref().unwrap().reloading_clip,
        "sibling ReloadingClip must survive getStatus refresh"
    );
    assert_eq!(attacker.weapon_fire_status, WeaponFireStatus::ReloadingClip);
}

#[test]
fn clip_reload_elapse_refills_and_clears_reloading() {
    let mut attacker = Object::new(ThingTemplate::new("ClipWait"), ObjectId(1), Team::USA);
    attacker.weapon = Some(Weapon {
        last_fire_time: 1.0,
        clip_size: 4,
        ammo: Some(0),
        clip_reload_time: 1.0,
        reloading_clip: true,
        reload_time: 0.2,
        ..weapon(10.0)
    });
    attacker.refresh_weapon_fire_status(1.5);
    assert_eq!(attacker.weapon_fire_status, WeaponFireStatus::ReloadingClip);
    assert_eq!(attacker.weapon.as_ref().unwrap().ammo, Some(0));
    attacker.refresh_weapon_fire_status(2.05);
    assert_eq!(attacker.weapon.as_ref().unwrap().ammo, Some(4));
    assert!(!attacker.weapon.as_ref().unwrap().reloading_clip);
    assert_eq!(attacker.weapon_fire_status, WeaponFireStatus::ReadyToFire);
}

#[test]
fn clip_reload_time_zero_is_ready_same_frame() {
    let mut attacker = Object::new(ThingTemplate::new("InstantClip"), ObjectId(1), Team::USA);
    attacker.weapon = Some(Weapon {
        last_fire_time: 1.0,
        clip_size: 2,
        ammo: Some(0),
        clip_reload_time: 0.0,
        reloading_clip: true,
        reload_time: 1.0,
        ..weapon(10.0)
    });
    attacker.refresh_weapon_fire_status(1.0);
    assert_eq!(attacker.weapon.as_ref().unwrap().ammo, Some(2));
    assert!(!attacker.weapon.as_ref().unwrap().reloading_clip);
    assert_eq!(attacker.weapon_fire_status, WeaponFireStatus::ReadyToFire);
}

#[test]
fn rof_bonus_change_restarts_in_progress_reload() {
    let mut attacker = Object::new(ThingTemplate::new("RofWait"), ObjectId(1), Team::USA);
    attacker.weapon = Some(Weapon {
        last_fire_time: 1.0,
        reload_time: 1.0,
        last_bonus_rof: 1.0,
        ..weapon(10.0)
    });
    attacker.weapon_bonus_veteran = true;
    attacker.refresh_weapon_fire_status(1.5);
    let last = attacker.weapon.as_ref().unwrap().last_fire_time;
    assert!(
        (last - 1.5).abs() < 1e-4,
        "C++ onWeaponBonusChange restarts the wait from now, last={last}"
    );
}

#[test]
fn weaponset_change_unlocks_unless_shared_across_sets() {
    let mut attacker = Object::new(
        ThingTemplate::new("AmericaVehicleHumvee"),
        ObjectId(1),
        Team::USA,
    );
    attacker.weapon = Some(weapon(10.0));
    attacker.secondary_weapon = Some(weapon(30.0));
    assert!(attacker.set_weapon_lock(1, WeaponLockType::LockedPermanently));
    attacker.apply_veterancy_bonuses(
        crate::game_logic::VeterancyLevel::Rookie,
        crate::game_logic::VeterancyLevel::Veteran,
    );
    assert!(!attacker.is_weapon_locked());
    assert_eq!(attacker.active_weapon_slot, 0);

    attacker.thing.template.weapon_lock_shared_across_sets = true;
    assert!(attacker.set_weapon_lock(1, WeaponLockType::LockedPermanently));
    attacker.apply_veterancy_bonuses(
        crate::game_logic::VeterancyLevel::Veteran,
        crate::game_logic::VeterancyLevel::Elite,
    );
    assert!(attacker.is_weapon_locked());
    assert_eq!(attacker.weapon_lock_slot, 1);
}

#[test]
fn retarget_inside_continuous_fire_coast_keeps_consecutive_shots() {
    let mut attacker = Object::new(
        ThingTemplate::new("ChinaGattlingTank"),
        ObjectId(1),
        Team::China,
    );
    attacker.consecutive_shot_target = Some(ObjectId(2));
    attacker.consecutive_shots_at_target = 5;
    attacker.continuous_fire_coast_until_frame = u32::MAX;
    attacker.record_shot_at_target(ObjectId(3));
    assert_eq!(attacker.consecutive_shots_at_target, 6);
    assert_eq!(attacker.consecutive_shot_target, Some(ObjectId(3)));

    attacker.continuous_fire_coast_until_frame = 0;
    attacker.record_shot_at_target(ObjectId(4));
    // C++ FiringTracker::shotFired (:111-131) + coolDown (:319-321):
    // block 1 crossed '> ContinuousFireOne' into MEAN; this retargeted
    // shot restarts at 1 < ContinuousFireOne, so it coolDown()s — the
    // demoting shot leaves no count and no victim.
    assert_eq!(attacker.consecutive_shots_at_target, 0);
    assert_eq!(attacker.consecutive_shot_target, None);
}

#[test]
fn leftover_choose_best_skips_zero_damage_unless_unresistable() {
    let mut attacker = Object::new(ThingTemplate::new("ZeroDmgChooser"), ObjectId(1), Team::USA);
    attacker.weapon = Some(weapon(0.0));
    attacker.secondary_weapon = Some(weapon(10.0));
    let mut target = Object::new(ThingTemplate::new("Target"), ObjectId(2), Team::GLA);
    target.set_position(glam::Vec3::new(50.0, 0.0, 0.0));
    assert_eq!(
        attacker.select_combat_weapon_slot(&target, 1.0),
        Some(1),
        "zero-damage PRIMARY is eliminated so SECONDARY wins"
    );

    const UNRES: &str = "__RustChooseBestZeroUnresistable";
    let _ = gamelogic::weapon::with_weapon_store_mut(|store| {
        let mut template = gamelogic::weapon::WeaponTemplate::new(UNRES.to_string());
        template.primary_damage = 0.0;
        template.damage_type = gamelogic::damage::DamageType::Unresistable;
        template.attack_range = 200.0;
        store.add_weapon_template(template);
    });
    attacker.thing.template.set_primary_weapon_name(UNRES);
    attacker.weapon = Some(weapon(0.0));
    assert_eq!(
        attacker.select_combat_weapon_slot(&target, 1.0),
        Some(0),
        "DAMAGE_UNRESISTABLE may keep a zero-damage slot"
    );
}

#[test]
fn leftover_choose_best_turret_aim_demotes_ready_slot() {
    let mut attacker = Object::new(ThingTemplate::new("TurretChooser"), ObjectId(1), Team::USA);
    attacker.weapon = Some(weapon(100.0));
    attacker.secondary_weapon = Some(weapon(10.0));
    attacker.turret_enabled = true;
    attacker.turret_substate = TurretSubState::Aim;
    attacker.turret_target_id = Some(ObjectId(2));
    let mut target = Object::new(ThingTemplate::new("Target"), ObjectId(2), Team::GLA);
    target.set_position(glam::Vec3::new(50.0, 0.0, 0.0));
    assert!(attacker.is_weapon_slot_on_turret_and_aiming_at_target(0, &target));
    assert!(!attacker.is_weapon_slot_on_turret_and_aiming_at_target(1, &target));
    assert_eq!(
        attacker.select_combat_weapon_slot(&target, 1.0),
        Some(1),
        "aiming turret PRIMARY is demoted so hull SECONDARY can fire"
    );
}

#[test]
fn leftover_choose_best_skips_slot_outside_target_pitch() {
    const LOFT: &str = "__RustChooseBestLoftLimited";
    gamelogic::initialize_weapon_store().expect("actual pitch catalog initialization");
    gamelogic::weapon::with_weapon_store_mut(|store| {
        let mut template = gamelogic::weapon::WeaponTemplate::new(LOFT.to_string());
        template.primary_damage = 100.0;
        template.attack_range = 200.0;
        template.min_target_pitch = (-15f32).to_radians();
        template.max_target_pitch = 15f32.to_radians();
        store.add_weapon_template(template);
    })
    .expect("register actual authored PRIMARY pitch limits");
    let mut t = ThingTemplate::new("PitchChooser");
    t.set_primary_weapon_name(LOFT);
    let mut attacker = Object::new(t, ObjectId(1), Team::USA);
    attacker.weapon = Some(weapon(100.0));
    attacker.secondary_weapon = Some(weapon(10.0));
    let mut target = Object::new(ThingTemplate::new("HighTarget"), ObjectId(2), Team::GLA);
    target.set_position(glam::Vec3::new(10.0, 50.0, 0.0));
    assert_eq!(attacker.authored_weapon_name_for_slot(0), Some(LOFT));
    assert_eq!(attacker.authored_weapon_name_for_slot(1), None);
    assert!(!attacker.is_slot_within_target_pitch(0, &target));
    assert_eq!(
        attacker.select_combat_weapon_slot(&target, 1.0),
        Some(1),
        "loft-failing PRIMARY is eliminated so SECONDARY wins"
    );
}

#[test]
fn leftover_choose_best_backup_picks_unready_valid_slot() {
    let mut attacker = Object::new(ThingTemplate::new("BackupChooser"), ObjectId(1), Team::USA);
    attacker.weapon = Some(Weapon {
        last_fire_time: 1.0,
        reload_time: 1.0,
        ..weapon(10.0)
    });
    attacker.secondary_weapon = Some(Weapon {
        last_fire_time: 1.0,
        reload_time: 1.0,
        ..weapon(40.0)
    });
    let mut target = Object::new(ThingTemplate::new("Target"), ObjectId(2), Team::GLA);
    target.set_position(glam::Vec3::new(50.0, 0.0, 0.0));
    assert_eq!(
        attacker.select_combat_weapon_slot(&target, 1.1),
        Some(1),
        "when every auto-choose slot is mid-reload leftover keeps the best backup"
    );
}

#[test]
fn leftover_choose_best_skips_empty_no_auto_reload_slot() {
    // C++ WeaponSet.cpp:834-836: OUT_OF_AMMO && !getAutoReloadsClip()
    // continues past the slot; leftover weapon_set.rs:732-737 ports it.
    // An empty no-auto-reload PRIMARY must not win leftover backup.
    const EMPTY: &str = "__RustChooseBestEmptyNoReload";
    let _ = gamelogic::weapon::with_weapon_store_mut(|store| {
        let mut template = gamelogic::weapon::WeaponTemplate::new(EMPTY.to_string());
        template.primary_damage = 100.0;
        template.attack_range = 200.0;
        template.reload_type = gamelogic::weapon::WeaponReloadType::NoReload;
        store.add_weapon_template(template);
    });
    let mut t = ThingTemplate::new("EmptyNoReloadChooser");
    t.set_primary_weapon_name(EMPTY);
    let mut attacker = Object::new(t, ObjectId(1), Team::USA);
    attacker.weapon = Some(Weapon {
        ammo: Some(0),
        clip_size: 1,
        last_fire_time: 1.0,
        reload_time: 1.0,
        ..weapon(100.0)
    });
    attacker.secondary_weapon = Some(Weapon {
        last_fire_time: 1.0,
        reload_time: 1.0,
        ..weapon(10.0)
    });
    let mut target = Object::new(ThingTemplate::new("Target"), ObjectId(2), Team::GLA);
    target.set_position(glam::Vec3::new(50.0, 0.0, 0.0));
    assert_eq!(
        attacker.select_combat_weapon_slot(&target, 1.1),
        Some(1),
        "empty no-auto-reload PRIMARY is skipped so leftover backup keeps SECONDARY"
    );
}

#[test]
fn leftover_choose_best_temp_lock_keeps_unready_slot() {
    // C++ WeaponSet.cpp:782-783: isCurWeaponLocked → keep current slot.
    // Reloading FireWeapon/flashbang/snipe must not fall through to PRIMARY.
    let mut attacker = Object::new(
        ThingTemplate::new("AmericaInfantryRanger"),
        ObjectId(1),
        Team::USA,
    );
    attacker.weapon = Some(Weapon {
        last_fire_time: -10.0,
        reload_time: 0.0,
        ..weapon(5.0)
    });
    attacker.secondary_weapon = Some(Weapon {
        last_fire_time: 1.0,
        reload_time: 1.0,
        ammo: Some(0),
        clip_size: 1,
        ..weapon(35.0)
    });
    assert!(attacker.set_weapon_lock(1, WeaponLockType::LockedTemporarily));
    let mut target = Object::new(
        {
            let mut t = ThingTemplate::new("Infantry");
            t.add_kind_of(KindOf::Infantry);
            t
        },
        ObjectId(2),
        Team::GLA,
    );
    target.set_position(glam::Vec3::new(50.0, 0.0, 0.0));
    assert_eq!(
        attacker.select_combat_weapon_slot(&target, 1.1),
        Some(1),
        "temp-locked reloading SECONDARY must wait, not auto-choose PRIMARY"
    );
}

#[test]
fn leftover_choose_best_ground_resets_primary_unless_locked() {
    let mut attacker = Object::new(
        ThingTemplate::new("AmericaVehicleHumvee"),
        ObjectId(1),
        Team::USA,
    );
    attacker.weapon = Some(weapon(10.0));
    attacker.secondary_weapon = Some(weapon(30.0));
    attacker.set_active_weapon_slot(1);
    assert_eq!(attacker.leftover_choose_best_ground_slot(), 0);
    attacker.leftover_choose_best_reset_primary_for_ground();
    assert_eq!(attacker.active_weapon_slot, 0);

    assert!(attacker.set_weapon_lock(1, WeaponLockType::LockedTemporarily));
    assert_eq!(attacker.leftover_choose_best_ground_slot(), 1);
    attacker.leftover_choose_best_reset_primary_for_ground();
    assert_eq!(attacker.active_weapon_slot, 1);
    assert_eq!(attacker.weapon_lock_slot, 1);
}

#[path = "clip_pitch_contract_tests.rs"]
mod clip_pitch_contract_tests;
