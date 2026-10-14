//! Exact immutable receiving rows, including authoritative false/zero values.
use super::*;
use gamelogic::weapon::{WeaponAntiMask, WeaponReloadType, WeaponTemplate};
fn owner(rule: &WeaponTemplate) -> Object {
    let mut template = ThingTemplate::new("ReceivingSelectedRuleOwner");
    template.add_kind_of(KindOf::Infantry);
    template.bind_native_weapon_row([Some(rule), None, None]);
    let mut object = Object::new(template, ObjectId(99001), Team::USA);
    object.weapon = Some(ThingTemplate::weapon_from_template_at(rule, 0).unwrap());
    object
}
fn row() -> WeaponTemplate {
    let mut rule = WeaponTemplate::new("SameReceivingWeapon".into());
    rule.attack_range = 100.0;
    rule.clip_size = 3;
    rule.min_delay_between_shots = 0;
    rule.max_delay_between_shots = 0;
    rule
}
#[test]
fn same_name_receiving_rows_keep_authoritative_zero_false_and_opposing_eligibility() {
    let mut first = row();
    first.primary_damage = 0.0;
    first.anti_mask = WeaponAntiMask::new(0);
    first.aim_delta = 0.0;
    first.is_shows_ammo_pips = false;
    first.capable_of_following_waypoint = false;
    let mut second = first.clone();
    second.primary_damage = 25.0;
    second.anti_mask = WeaponAntiMask::new(WeaponAntiMask::GROUND);
    second.aim_delta = 1.0;
    second.is_shows_ammo_pips = true;
    second.capable_of_following_waypoint = true;
    let a = owner(&first);
    let b = owner(&second);
    let mut target_template = ThingTemplate::new("ReceivingSelectedRuleTarget");
    target_template.add_kind_of(KindOf::Infantry);
    let target = Object::new(target_template, ObjectId(99002), Team::China);
    assert!(!a.weapon_allows_target_anti_mask(
        a.weapon.as_ref().unwrap(),
        Some(0),
        WeaponAntiMask::GROUND
    ));
    assert!(b.weapon_allows_target_anti_mask(
        b.weapon.as_ref().unwrap(),
        Some(0),
        WeaponAntiMask::GROUND
    ));
    assert_eq!(a.aim_delta_for_slot(0), 0.0);
    assert_eq!(b.aim_delta_for_slot(0), 1.0);
    assert_eq!(a.get_ammo_pip_showing_info(), None);
    assert_eq!(b.get_ammo_pip_showing_info(), Some((3, 3)));
    assert_eq!(a.find_waypoint_following_capable_weapon_slot(), None);
    assert_eq!(b.find_waypoint_following_capable_weapon_slot(), Some(0));
    assert_eq!(a.estimated_slot_damage_vs(0, &target), 0.0);
    assert_eq!(b.estimated_slot_damage_vs(0, &target), 25.0);
    // Mutating the caller's draft after admission cannot change either world.
    first.primary_damage = 500.0;
    first.aim_delta = 2.0;
    assert_eq!(a.estimated_slot_damage_vs(0, &target), 0.0);
    assert_eq!(b.aim_delta_for_slot(0), 1.0);
}
#[test]
fn receiving_reload_policy_and_idle_deadline_do_not_adopt_same_name_peer() {
    let mut manual = row();
    manual.reload_type = WeaponReloadType::NoReload;
    manual.auto_reload_when_idle_frames = 0;
    let mut auto = manual.clone();
    auto.reload_type = WeaponReloadType::AutoReload;
    auto.clip_reload_time = 9;
    auto.auto_reload_when_idle_frames = 7;
    let mut a = owner(&manual);
    let mut b = owner(&auto);
    for object in [&mut a, &mut b] {
        let weapon = object.weapon.as_mut().unwrap();
        weapon.ammo = Some(0);
        weapon.last_fire_time = 0.0;
        weapon.reloading_clip = true;
    }
    assert!(!a.selected_slot_has_ammo_for_shot(0, a.weapon.as_ref().unwrap()));
    assert!(b.selected_slot_has_ammo_for_shot(0, b.weapon.as_ref().unwrap()));
    assert!(!a.complete_elapsed_auto_reload_clips(1.0));
    assert_eq!(a.weapon.as_ref().unwrap().ammo, Some(0));
    assert!(b.complete_elapsed_auto_reload_clips(1.0));
    assert_eq!(b.weapon.as_ref().unwrap().ammo, Some(3));
    a.stamp_auto_reload_when_idle_from_slot(0, 100);
    b.weapon.as_mut().unwrap().ammo = Some(2);
    b.stamp_auto_reload_when_idle_from_slot(0, 100);
    assert_eq!(a.frame_to_force_reload, 0);
    assert_eq!(b.frame_to_force_reload, 107);
}
#[test]
fn selected_pitch_limits_override_same_name_contact_inference() {
    let mut constrained = row();
    constrained.name = "RangerMachineGun".into();
    constrained.min_target_pitch = 0.0;
    constrained.max_target_pitch = 0.0;
    let mut open = constrained.clone();
    open.min_target_pitch = -std::f32::consts::FRAC_PI_2;
    open.max_target_pitch = std::f32::consts::FRAC_PI_2;
    let a = owner(&constrained);
    let b = owner(&open);
    let mut target = Object::new(
        ThingTemplate::new("PitchOwnerTarget"),
        ObjectId(99002),
        Team::China,
    );
    target.set_position(Vec3::new(10.0, 100.0, 0.0));
    assert!(!a.is_slot_within_target_pitch(0, &target));
    assert!(b.is_slot_within_target_pitch(0, &target));
}
