//! C++ Weapon.cpp:1880-1894,2736-2752: clip completion preserves its
//! next-shot deadline; WeaponSet.cpp:809-843 tests each actual slot.
use super::*;

fn waiting_clip(clip: f32, between: f32) -> Object {
    let mut attacker = Object::new(
        ThingTemplate::new("ClipDeadlineControl"),
        ObjectId(1),
        Team::USA,
    );
    attacker.last_fire_frame = 77;
    attacker.weapon = Some(Weapon {
        damage: 10.0,
        range: 200.0,
        last_fire_time: 1.0,
        clip_size: 2,
        ammo: Some(0),
        clip_reload_time: clip,
        reloading_clip: true,
        reload_time: between,
        ..Weapon::default()
    });
    attacker
}

#[test]
fn shorter_clip_deadline_stays_ready_across_refresh_and_actual_object_acceptance() {
    let mut attacker = waiting_clip(0.25, 1.0);
    attacker.refresh_weapon_fire_status(1.2);
    assert_eq!(attacker.weapon_fire_status, WeaponFireStatus::ReloadingClip);
    assert_eq!(attacker.weapon.as_ref().unwrap().ammo, Some(0));
    assert!(!attacker.can_fire_slot(0, 1.2));

    attacker.refresh_weapon_fire_status(1.25);
    assert_eq!(attacker.weapon.as_ref().unwrap().ammo, Some(2));
    assert!(!attacker.weapon.as_ref().unwrap().reloading_clip);
    assert_eq!(attacker.weapon_fire_status, WeaponFireStatus::ReadyToFire);
    assert!(attacker.can_fire_slot(0, 1.25));
    assert_eq!(
        attacker.last_fire_frame, 77,
        "refresh is not an accepted shot"
    );
    attacker.refresh_weapon_fire_status(1.25);
    attacker.refresh_weapon_fire_status(1.3);
    assert_eq!(attacker.weapon_fire_status, WeaponFireStatus::ReadyToFire);
    assert!(attacker.can_fire_slot(0, 1.3));

    // Actual Object acceptance owns its real queue. This primitive control
    // does not claim a roster tick, victim materialization, or world save.
    let mut combat = crate::game_logic::combat::CombatSystem::new();
    assert!(attacker.fire_at(ObjectId(2), 1.3, 88, &mut combat, false));
    assert_eq!(attacker.last_fire_frame, 88);
    assert_eq!(attacker.weapon.as_ref().unwrap().ammo, Some(1));
    assert_eq!(attacker.weapon.as_ref().unwrap().last_fire_time, 1.3);
    attacker.refresh_weapon_fire_status(1.4);
    assert_eq!(
        attacker.weapon_fire_status,
        WeaponFireStatus::BetweenFiringShots
    );
    assert!(!attacker.can_fire_slot(0, 1.4));
    attacker.refresh_weapon_fire_status(2.3);
    assert_eq!(attacker.weapon_fire_status, WeaponFireStatus::ReadyToFire);
    assert_eq!(attacker.weapon.as_ref().unwrap().ammo, Some(1));
}

#[test]
fn delayed_clip_poll_retains_original_deadline_in_existing_weapon_payload() {
    let mut attacker = waiting_clip(1.0, 2.0);
    attacker.refresh_weapon_fire_status(4.0);
    assert_eq!(attacker.weapon_fire_status, WeaponFireStatus::ReadyToFire);
    assert_eq!(attacker.weapon.as_ref().unwrap().ammo, Some(2));
    assert!(!attacker.weapon.as_ref().unwrap().reloading_clip);
    assert_eq!(attacker.last_fire_frame, 77);
    assert!(
        attacker.can_fire_slot(0, 2.0),
        "the deadline was 2.0, not the later polling time"
    );

    // Exercise the existing Weapon serde payload actually used by host saves.
    // This is not a SaveFileManager framing or whole-world restore claim.
    let bytes = bincode_legacy::serialize(attacker.weapon.as_ref().unwrap()).unwrap();
    let restored: Weapon = bincode_legacy::deserialize(&bytes).unwrap();
    assert_eq!(bincode_legacy::serialize(&restored).unwrap(), bytes);
    attacker.weapon = Some(restored);
    attacker.refresh_weapon_fire_status(4.0);
    assert_eq!(attacker.weapon_fire_status, WeaponFireStatus::ReadyToFire);
    assert!(attacker.can_fire_slot(0, 2.0));
    assert_eq!(attacker.weapon.as_ref().unwrap().ammo, Some(2));
}

#[test]
fn actual_secondary_pitch_definition_is_independent_of_limited_primary() {
    const PRIMARY: &str = "__RustPitchOwnedPrimary";
    const SECONDARY: &str = "__RustPitchOwnedSecondary";
    gamelogic::initialize_weapon_store().expect("actual pitch catalog initialization");
    gamelogic::weapon::with_weapon_store_mut(|store| {
        let mut primary = gamelogic::weapon::WeaponTemplate::new(PRIMARY.to_owned());
        primary.primary_damage = 100.0;
        primary.attack_range = 200.0;
        primary.min_target_pitch = (-15f32).to_radians();
        primary.max_target_pitch = 15f32.to_radians();
        store.add_weapon_template(primary);
        let mut secondary = gamelogic::weapon::WeaponTemplate::new(SECONDARY.to_owned());
        secondary.primary_damage = 10.0;
        secondary.attack_range = 200.0;
        secondary.min_target_pitch = (-90f32).to_radians();
        secondary.max_target_pitch = 90f32.to_radians();
        store.add_weapon_template(secondary);
    })
    .expect("register both actual slot definitions");
    let mut rules = ThingTemplate::new("ExactPitchControl");
    rules.set_primary_weapon_name(PRIMARY);
    rules.secondary_weapon_name = Some(SECONDARY.to_owned());
    let mut attacker = Object::new(rules, ObjectId(1), Team::USA);
    attacker.weapon = Some(weapon(100.0));
    attacker.secondary_weapon = Some(weapon(10.0));
    let mut target = Object::new(
        ThingTemplate::new("HighPitchTarget"),
        ObjectId(2),
        Team::GLA,
    );
    target.set_position(glam::Vec3::new(10.0, 50.0, 0.0));
    assert!(!attacker.is_slot_within_target_pitch(0, &target));
    assert!(attacker.is_slot_within_target_pitch(1, &target));
    assert_eq!(attacker.select_combat_weapon_slot(&target, 1.0), Some(1));
    assert_eq!(
        attacker.active_weapon_slot, 0,
        "selection query does not switch slot"
    );
}
