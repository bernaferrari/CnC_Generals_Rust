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

#[test]
fn peek_status_is_pure_and_observe_status_is_the_mutable_owner_write() {
    // hq-2qedt.15: C++ Weapon::getStatus (Weapon.cpp:2736) writes the
    // `mutable m_status`. Rust splits that into a pure `peek_status_at(&self)`
    // and a mutable-owner `observe_status_at(&mut self)`.
    let mut template = WeaponTemplate::new("ObserveStatusGun".into());
    template.clip_size = 1;
    let mut weapon = Weapon::new_at(Arc::new(template), WeaponSlotType::Primary, 0);
    weapon.ammo_in_clip = 1;
    weapon.when_pre_attack_finished = 0;
    weapon.when_we_can_fire_again = 10;
    weapon.store_status(WeaponStatus::ReloadingClip);

    assert_eq!(weapon.peek_status_at(5), WeaponStatus::ReloadingClip);
    assert_eq!(weapon.peek_status_at(10), WeaponStatus::ReadyToFire);
    assert_eq!(
        weapon.load_status(),
        WeaponStatus::ReloadingClip,
        "peek must not write"
    );

    assert_eq!(weapon.observe_status_at(10), WeaponStatus::ReadyToFire);
    assert_eq!(
        weapon.load_status(),
        WeaponStatus::ReadyToFire,
        "observe writes like C++"
    );
}

#[test]
fn weapon_selection_observes_status_through_mut_owner_and_peeks_in_selector() {
    // hq-2qedt.80: C++ WeaponSet::chooseBestWeaponForTarget calls
    // Weapon::getStatus (writes `mutable m_status`) on every slot that passes
    // the auto-choose mask. Rust: the &mut owner observes, the selector peeks.
    let mut template = WeaponTemplate::new("SelectObserveGun".into());
    template.clip_size = 1;
    let mut weapon = Weapon::new_at(Arc::new(template), WeaponSlotType::Primary, 0);
    weapon.ammo_in_clip = 1;
    weapon.when_pre_attack_finished = 0;
    weapon.when_we_can_fire_again = 10;
    weapon.store_status(WeaponStatus::ReloadingClip);
    let mut set = WeaponSet::new();
    set.restore_weapon(WeaponSlotType::Primary, weapon);

    set.observe_selection_statuses_at(10, crate::common::CommandSourceType::FromAi);
    assert_eq!(
        set.get_weapon_in_slot(WeaponSlotType::Primary)
            .unwrap()
            .load_status(),
        WeaponStatus::ReadyToFire,
        "&mut selection observe writes stored status like C++ getStatus"
    );

    set.get_weapon_in_slot_mut(WeaponSlotType::Primary)
        .unwrap()
        .store_status(WeaponStatus::ReloadingClip);
    assert_eq!(
        set.peek_selection_status_at(WeaponSlotType::Primary, 10),
        Some(WeaponStatus::ReadyToFire)
    );
    assert_eq!(
        set.get_weapon_in_slot(WeaponSlotType::Primary)
            .unwrap()
            .load_status(),
        WeaponStatus::ReloadingClip,
        "&self selector status read is a pure peek"
    );
}

#[test]
fn shared_borrow_status_reads_are_pure_and_crc_reads_observed_status() {
    // hq-2qedt.78: every `&self` status read (get_status_at, estimate,
    // percent-ready, remaining-ammo, WeaponSet out-of-ammo) is pure. Only the
    // `&mut` owner (`observe_status_at`) refreshes stored `m_status`, which is
    // what the CRC/xfer path (crc_snapshot.rs load_status) serialises.
    let mut template = WeaponTemplate::new("PureSharedReadGun".into());
    template.clip_size = 1;
    let mut weapon = Weapon::new_at(Arc::new(template), WeaponSlotType::Primary, 0);
    weapon.ammo_in_clip = 1;
    weapon.when_pre_attack_finished = 0;
    weapon.when_we_can_fire_again = 10;
    weapon.when_last_reload_started = 0;
    weapon.store_status(WeaponStatus::ReloadingClip);

    assert_eq!(weapon.get_status_at(10), WeaponStatus::ReadyToFire);
    let _ = weapon.get_remaining_ammo_at(10);
    let mut set = WeaponSet::new();
    set.restore_weapon(WeaponSlotType::Primary, weapon);
    assert!(!set.is_out_of_ammo_at(10));
    let weapon = set.get_weapon_in_slot(WeaponSlotType::Primary).unwrap();
    assert_eq!(
        weapon.load_status(),
        WeaponStatus::ReloadingClip,
        "&self status reads must not write stored status"
    );

    let weapon = set.get_weapon_in_slot_mut(WeaponSlotType::Primary).unwrap();
    assert_eq!(weapon.observe_status_at(10), WeaponStatus::ReadyToFire);
    assert_eq!(weapon.load_status(), WeaponStatus::ReadyToFire, "CRC sees observed status");
}
