//! C++ Object.cpp:1504-1518 and Weapon.cpp:2677-2690, 2820-2838.
//! These local objects exercise the supplied prefire frame and bonus boundary.
//! Stored status is observed directly and readiness uses `get_status_at`.

use std::sync::Arc;
use std::sync::RwLock;

use super::Object;
use crate::common::{ModelConditionFlags, ObjectStatusMaskType, ObjectStatusTypes};
use crate::object::drawable::{Drawable, DrawableType};
use crate::weapon::{
    Weapon, WeaponBonus, WeaponBonusField, WeaponPrefireType, WeaponSlotType, WeaponStatus,
    WeaponTemplate,
};

const VICTIM: u32 = 7002;

fn local_object() -> Object {
    let mut object = Object::new_test(7001, 100.0);
    // This object is never admitted to a world or the object registry.
    object.ambient_lifecycle_admitted = false;
    object.set_undetected_defector_flag(true);
    object
}

fn armed_object(template: WeaponTemplate, next_shot: u32) -> Object {
    let mut object = local_object();
    let mut weapon = Weapon::new_at(Arc::new(template), WeaponSlotType::Primary, 0);
    weapon.set_possible_next_shot_frame(next_shot);
    weapon.set_status(WeaponStatus::BetweenFiringShots);
    weapon.set_pre_attack_finished_frame(17);
    object
        .weapon_set
        .restore_weapon(WeaponSlotType::Primary, weapon);
    object
}

fn template(delay: i32, leech: bool) -> WeaponTemplate {
    let mut template = WeaponTemplate::new("OwnedPrefire".to_string());
    template.pre_attack_delay = delay;
    template.leech_range_weapon = leech;
    template
}

fn current_weapon(object: &Object) -> &Weapon {
    object
        .weapon_set
        .get_current_weapon()
        .expect("local weapon")
        .0
}

fn local_drawable(object: &mut Object) -> Arc<RwLock<Drawable>> {
    let drawable = Arc::new(RwLock::new(Drawable::new(
        7003,
        object.get_id(),
        "OwnedPrefirePresentation".to_string(),
        DrawableType::Static,
    )));
    object.drawable = Some(Arc::clone(&drawable));
    drawable
}

#[test]
fn no_weapon_leaves_defector_undetected() {
    let mut object = local_object();

    object
        .pre_fire_current_weapon_with_observations(100, VICTIM, &WeaponBonus::new())
        .unwrap();

    assert!(object.weapon_set.get_current_weapon().is_none());
    assert!(object.is_undetected_defector());
}

#[test]
fn next_shot_after_next_frame_preserves_prefire_state() {
    let mut object = armed_object(template(9, true), 102);

    object
        .pre_fire_current_weapon_with_observations(100, VICTIM, &WeaponBonus::new())
        .unwrap();

    let weapon = current_weapon(&object);
    assert_eq!(weapon.load_status(), WeaponStatus::BetweenFiringShots);
    assert_eq!(weapon.get_status_at(100), WeaponStatus::BetweenFiringShots);
    assert_eq!(weapon.get_status_at(101), WeaponStatus::BetweenFiringShots);
    assert_eq!(weapon.get_pre_attack_finished_frame(), 17);
    assert_eq!(weapon.get_possible_next_shot_frame(), 102);
    assert!(!weapon.has_leech_range());
    assert!(object.is_undetected_defector());
}

#[test]
fn next_frame_readiness_uses_supplied_bonus_and_starts_leech_delay() {
    let mut object = armed_object(template(9, true), 101);
    let mut bonus = WeaponBonus::new();
    bonus.set_field(WeaponBonusField::PreAttack, 0.5);

    object
        .pre_fire_current_weapon_with_observations(100, VICTIM, &bonus)
        .unwrap();

    let weapon = current_weapon(&object);
    // WeaponTemplate.cpp truncates the multiplied floating point delay to Int.
    assert_eq!(weapon.load_status(), WeaponStatus::PreAttack);
    assert_eq!(weapon.get_pre_attack_finished_frame(), 104);
    assert_eq!(weapon.get_possible_next_shot_frame(), 101);
    assert!(weapon.has_leech_range());
    assert!(!object.is_undetected_defector());
    assert_eq!(weapon.get_status_at(100), WeaponStatus::PreAttack);
    assert_eq!(weapon.get_status_at(103), WeaponStatus::PreAttack);
    // C++ getStatus tests now < preattack deadline before ammo/readiness.
    assert_eq!(weapon.get_status_at(104), WeaponStatus::OutOfAmmo);
}

#[test]
fn nonpositive_delay_preserves_weapon_state_but_reveals_defector() {
    for delay in [0, -3] {
        let mut object = armed_object(template(delay, true), 101);

        object
            .pre_fire_current_weapon_with_observations(100, VICTIM, &WeaponBonus::new())
            .unwrap();

        let weapon = current_weapon(&object);
        assert_eq!(weapon.load_status(), WeaponStatus::BetweenFiringShots);
        assert_eq!(weapon.get_pre_attack_finished_frame(), 17);
        assert!(!weapon.has_leech_range());
        // Object clears the flag after preFireWeapon even when delay is zero.
        assert!(!object.is_undetected_defector());
    }
}

#[test]
fn readiness_wraps_next_frame_before_comparing() {
    let mut not_ready = armed_object(template(5, true), 1);
    let mut ready = armed_object(template(5, true), 0);

    not_ready
        .pre_fire_current_weapon_with_observations(u32::MAX, VICTIM, &WeaponBonus::new())
        .unwrap();
    ready
        .pre_fire_current_weapon_with_observations(u32::MAX, VICTIM, &WeaponBonus::new())
        .unwrap();

    assert_eq!(
        current_weapon(&not_ready).load_status(),
        WeaponStatus::BetweenFiringShots
    );
    assert_eq!(
        current_weapon(&not_ready).get_pre_attack_finished_frame(),
        17
    );
    assert!(!current_weapon(&not_ready).has_leech_range());
    assert!(not_ready.is_undetected_defector());
    assert_eq!(
        current_weapon(&ready).load_status(),
        WeaponStatus::PreAttack
    );
    assert_eq!(current_weapon(&ready).get_pre_attack_finished_frame(), 4);
    assert!(current_weapon(&ready).has_leech_range());
    assert!(!ready.is_undetected_defector());
    // C++ uses ordinary unsigned comparisons after the wrapped write. The
    // prefire stored field and the observed status can therefore differ here.
    assert_eq!(
        current_weapon(&ready).get_status_at(u32::MAX),
        WeaponStatus::OutOfAmmo
    );
    assert_eq!(
        current_weapon(&ready).get_status_at(0),
        WeaponStatus::PreAttack
    );
}

#[test]
fn positive_delay_wraps_deadline_and_does_not_enable_nonleech_weapon() {
    let mut object = armed_object(template(5, false), u32::MAX - 1);

    object
        .pre_fire_current_weapon_with_observations(u32::MAX - 2, VICTIM, &WeaponBonus::new())
        .unwrap();

    let weapon = current_weapon(&object);
    assert_eq!(weapon.load_status(), WeaponStatus::PreAttack);
    assert_eq!(weapon.get_pre_attack_finished_frame(), 2);
    assert!(!weapon.has_leech_range());
    assert!(!object.is_undetected_defector());
}

#[test]
fn per_clip_prefire_delays_full_clip_only() {
    for (ammo, expected_status, expected_deadline, expected_leech) in [
        (3, WeaponStatus::PreAttack, 109, true),
        (2, WeaponStatus::BetweenFiringShots, 17, false),
    ] {
        let mut template = template(9, true);
        template.clip_size = 3;
        template.prefire_type = WeaponPrefireType::PrefirePerClip;
        let mut object = armed_object(template, 101);
        object
            .weapon_set
            .get_current_weapon_mut()
            .unwrap()
            .ammo_in_clip = ammo;

        object
            .pre_fire_current_weapon_with_observations(100, VICTIM, &WeaponBonus::new())
            .unwrap();

        let weapon = current_weapon(&object);
        assert_eq!(weapon.load_status(), expected_status);
        assert_eq!(weapon.get_pre_attack_finished_frame(), expected_deadline);
        assert_eq!(weapon.has_leech_range(), expected_leech);
        assert_eq!(weapon.ammo_in_clip, ammo);
        assert!(!object.is_undetected_defector());
    }
}

#[test]
fn per_attack_prefire_uses_actual_owned_tracker_for_same_victim_only() {
    for (victim, expected_count, expected_status, expected_deadline) in [
        (VICTIM, 3, WeaponStatus::BetweenFiringShots, 17),
        (VICTIM + 1, 0, WeaponStatus::PreAttack, 109),
        (crate::common::INVALID_ID, 0, WeaponStatus::PreAttack, 109),
    ] {
        let mut template = template(9, true);
        template.prefire_type = WeaponPrefireType::PrefirePerAttack;
        let mut object = armed_object(template, 101);
        let mut tracker = crate::helpers::FiringTracker::new(object.get_id());
        // A saved tracker continuation, using its existing local fixture seam.
        // No shot callback, world clock, registry, or audio lookup is needed.
        tracker.set_xfer_test_state(3, VICTIM, 200, 0, 0);
        object.firing_tracker = Some(Box::new(tracker));
        assert_eq!(
            object.get_num_consecutive_shots_fired_at_target(victim),
            expected_count
        );

        object
            .pre_fire_current_weapon_with_observations(100, victim, &WeaponBonus::new())
            .unwrap();

        let weapon = current_weapon(&object);
        assert_eq!(weapon.load_status(), expected_status);
        assert_eq!(weapon.get_pre_attack_finished_frame(), expected_deadline);
        assert_eq!(weapon.has_leech_range(), expected_count == 0);
        assert!(!object.is_undetected_defector());
        assert_eq!(object.get_num_consecutive_shots_fired_at_target(VICTIM), 3);
        assert_eq!(
            object.firing_tracker.as_ref().unwrap().xfer_test_state(),
            (3, VICTIM, 200, 0, 0)
        );
    }
}

#[test]
fn model_condition_at_keeps_ready_pose_only_for_selected_weapon_slot() {
    // Object.cpp:4697-4735 never sets a firing pose for an unselected slot.
    let mut object = armed_object(template(0, false), 0);
    let primary = object.weapon_set.get_current_weapon_mut().unwrap();
    primary.ammo_in_clip = 1;
    primary.last_fire_frame = 100;
    let mut secondary = Weapon::new_at(Arc::new(template(0, false)), WeaponSlotType::Secondary, 0);
    secondary.ammo_in_clip = 1;
    object
        .weapon_set
        .restore_weapon(WeaponSlotType::Secondary, secondary);
    assert!(
        object
            .weapon_set
            .apply_weapon_selection((WeaponSlotType::Secondary, true))
    );
    object.set_status(
        ObjectStatusMaskType::from_status(ObjectStatusTypes::IsAttacking)
            | ObjectStatusMaskType::from_status(ObjectStatusTypes::IsAimingWeapon),
        true,
    );
    let drawable = local_drawable(&mut object);
    drawable.write().unwrap().set_model_condition_state(
        ModelConditionFlags::FiringA | ModelConditionFlags::UsingWeaponA,
    );
    // Match the prior selected-slot pose and its C++ m_lastWeaponCondition
    // cache: adjustModelConditionForWeaponStatus changes flags only when the
    // requested condition differs from that cache (Object.cpp:4746-4750).
    object.last_weapon_condition[0] = crate::weapon::WeaponSetConditionType::Firing as u8;

    object.adjust_model_condition_for_weapon_status_at(100);

    let flags = drawable.read().unwrap().get_model_conditions();
    assert!(!flags.intersects(Object::all_weapon_fire_flags(WeaponSlotType::Primary)));
    assert!(
        flags
            .contains(ModelConditionFlags::BetweenFiringShotsB | ModelConditionFlags::UsingWeaponB)
    );
    assert_eq!(
        object.last_weapon_condition[0],
        crate::weapon::WeaponSetConditionType::None as u8
    );
    assert_eq!(
        object.last_weapon_condition[1],
        crate::weapon::WeaponSetConditionType::Between as u8
    );
}

#[test]
fn model_condition_at_observes_supplied_preattack_deadline_and_last_shot_precedence() {
    let mut object = armed_object(template(9, false), 101);
    let drawable = local_drawable(&mut object);
    object.set_status(
        ObjectStatusMaskType::from_status(ObjectStatusTypes::IsAttacking),
        true,
    );
    object
        .pre_fire_current_weapon_with_observations(100, VICTIM, &WeaponBonus::new())
        .unwrap();

    object.adjust_model_condition_for_weapon_status_at(104);
    assert!(
        drawable
            .read()
            .unwrap()
            .get_model_conditions()
            .contains(ModelConditionFlags::PreAttackA | ModelConditionFlags::UsingWeaponA,)
    );
    assert_eq!(
        object.last_weapon_condition[0],
        crate::weapon::WeaponSetConditionType::PreAttack as u8
    );
    object.adjust_model_condition_for_weapon_status_at(109);
    assert!(
        !drawable
            .read()
            .unwrap()
            .get_model_conditions()
            .intersects(Object::all_weapon_fire_flags(WeaponSlotType::Primary))
    );

    object
        .weapon_set
        .get_current_weapon_mut()
        .unwrap()
        .last_fire_frame = 110;
    object.set_status(
        ObjectStatusMaskType::from_status(ObjectStatusTypes::IsAttacking),
        false,
    );
    object.adjust_model_condition_for_weapon_status_at(110);
    // C++ tests the current-frame shot before IS_ATTACKING or weapon status.
    assert!(
        drawable
            .read()
            .unwrap()
            .get_model_conditions()
            .contains(ModelConditionFlags::FiringA | ModelConditionFlags::UsingWeaponA,)
    );
    assert_eq!(
        object.last_weapon_condition[0],
        crate::weapon::WeaponSetConditionType::Firing as u8
    );
}

#[test]
fn missing_weapon_records_none_and_preserves_existing_drawable_flags() {
    let mut object = local_object();
    object.last_weapon_condition = [crate::weapon::WeaponSetConditionType::Firing as u8; 3];
    let drawable = local_drawable(&mut object);
    let prior_flags = ModelConditionFlags::FiringA | ModelConditionFlags::UsingWeaponA;
    drawable
        .write()
        .unwrap()
        .set_model_condition_state(prior_flags);

    object.adjust_model_condition_for_weapon_status_at(100);

    assert_eq!(
        object.last_weapon_condition,
        [crate::weapon::WeaponSetConditionType::None as u8; 3]
    );
    assert_eq!(drawable.read().unwrap().get_model_conditions(), prior_flags);

    object.drawable = None;
    object.last_weapon_condition = [crate::weapon::WeaponSetConditionType::Firing as u8; 3];
    object.adjust_model_condition_for_weapon_status_at(101);
    assert_eq!(
        object.last_weapon_condition,
        [crate::weapon::WeaponSetConditionType::None as u8; 3]
    );
}
