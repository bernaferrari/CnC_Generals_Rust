//! Actual receiving WeaponStore histories; no registry, frame publication or test mutex.
use super::*;
use crate::common::Coord3D;
use std::sync::Arc;

#[test]
fn post_fire_auto_reload_uses_the_supplied_operation_frame() {
    // Weapon.cpp:2629 calls reloadWithBonus within the same firing operation.
    let mut template = WeaponTemplate::new("OwnedReloadGun".into());
    template.clip_size = 1;
    template.clip_reload_time = 17;
    template.reload_type = WeaponReloadType::AutoReload;
    let mut weapon = Weapon::new(Arc::new(template), WeaponSlotType::Primary);
    weapon.ammo_in_clip = 1;
    assert!(weapon.apply_post_fire_state(7, 1234, &WeaponBonus::new()));
    assert_eq!(weapon.load_status(), WeaponStatus::ReloadingClip);
    assert_eq!(weapon.when_last_reload_started, 1234);
    assert_eq!(weapon.when_we_can_fire_again, 1251);
    assert_eq!(weapon.ammo_in_clip, 1);
}
fn definition() -> WeaponTemplate {
    let mut template = WeaponTemplate::new("OwnedHistoryGun".into());
    template.historic_bonus_count = 3;
    template.historic_bonus_time = 90;
    template.historic_bonus_radius = 20.0;
    template
}
#[test]
fn same_definition_records_independently_in_two_stores_and_reset_is_local() {
    let definition = Arc::new(definition());
    let mut first = WeaponStore::new();
    let mut peer = WeaponStore::new();
    let pos = Coord3D::ZERO;
    assert!(!first.observe_historic_damage_at(&definition, &pos, 100, 90, false));
    assert!(!first.observe_historic_damage_at(&definition, &pos, 100, 90, false));
    assert_eq!(first.historic_damage_len(&definition), 2);
    assert_eq!(peer.historic_damage_len(&definition), 0);
    assert!(!peer.observe_historic_damage_at(&definition, &pos, 100, 90, false));
    assert!(first.observe_historic_damage_at(&definition, &pos, 100, 90, false));
    assert_eq!(
        first.historic_damage_len(&definition),
        2,
        "CPP retains history during nested bonus delivery"
    );
    first.clear_historic_damage(&definition);
    assert_eq!(first.historic_damage_len(&definition), 0);
    assert_eq!(peer.historic_damage_len(&definition), 1);
    first.reset().unwrap();
    assert_eq!(peer.historic_damage_len(&definition), 1);
}
#[test]
fn projectile_definition_clone_shares_history_identity_but_override_does_not() {
    let definition = Arc::new(definition());
    let cloned = (*definition).clone();
    let mut store = WeaponStore::new();
    assert!(!store.observe_historic_damage_at(&definition, &Coord3D::ZERO, 100, 90, false));
    assert_eq!(store.historic_damage_len(&cloned), 1);
    let override_definition = store
        .create_weapon_override(&definition, "OwnedHistoryOverride".into())
        .unwrap();
    assert_eq!(store.historic_damage_len(&override_definition), 0);
    assert!(!store.observe_historic_damage_at(
        &override_definition,
        &Coord3D::ZERO,
        100,
        90,
        false
    ));
    assert_eq!(store.historic_damage_len(&definition), 1);
}
#[test]
fn same_named_separate_definitions_do_not_merge_histories() {
    let first = definition();
    let peer = definition();
    let mut store = WeaponStore::new();
    assert!(!store.observe_historic_damage_at(&first, &Coord3D::ZERO, 100, 90, false));
    assert_eq!(store.historic_damage_len(&peer), 0);
}
#[test]
fn original_window_expiration_and_inclusive_radius_use_supplied_frame() {
    let definition = definition();
    let mut store = WeaponStore::new();
    assert!(!store.observe_historic_damage_at(&definition, &Coord3D::ZERO, 100, 90, false));
    assert!(!store.observe_historic_damage_at(
        &definition,
        &Coord3D::new(20.0, 0.0, 0.0),
        100,
        90,
        false
    ));
    assert!(store.observe_historic_damage_at(&definition, &Coord3D::ZERO, 100, 90, false));
    store.trim_historic_damage_at(&definition, 190, 90);
    assert_eq!(
        store.historic_damage_len(&definition),
        0,
        "CPP removes frame <= current-limit"
    );
}
