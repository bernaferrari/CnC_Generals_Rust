//! Parity tests for the canonical upgrade store (C++ Upgrade.cpp).

use super::*;
use crate::common::ascii_string::AsciiString;
use crate::common::ini::INI;
use crate::common::system::Xfer;
use crate::common::system::xfer_load::XferLoad;
use crate::common::system::xfer_save::XferSave;
use std::io::Cursor;
use std::sync::{Arc, RwLock};

const DEFAULT_UPGRADE_INI: &str = "\
Upgrade DefaultUpgrade
  BuildTime = 7.0
  BuildCost = 100
End

Upgrade Upgrade_ParityA
  BuildCost = 400
  ButtonImage = SSParityA
End
";

const UPGRADE_INI: &str = "\
Upgrade Upgrade_ParityB
  Type = OBJECT
  BuildTime = 12.5
End

Upgrade Upgrade_ParityA
  BuildCost = 900
End

Upgrade DefaultUpgrade
  BuildCost = 250
End

Upgrade Upgrade_ParityC
  DisplayName = UPGRADE:ParityC
End
";

fn engine_center() -> Arc<RwLock<UpgradeCenter>> {
    let mut center = UpgradeCenter::new();
    center.init();
    Arc::new(RwLock::new(center))
}

fn load(
    center: &Arc<RwLock<UpgradeCenter>>,
    text: &str,
) -> Result<(), crate::common::ini::INIError> {
    let mut ini = INI::new();
    ini.set_upgrade_center_target(Arc::clone(center));
    ini.with_inline_source(text, |ini| ini.parse_current_file())
}

fn bit_of(center: &UpgradeCenter, name: &str) -> u32 {
    let mask = center.find_upgrade(name).expect(name).get_mask().bits();
    assert_eq!(mask.count_ones(), 1, "{name} owns exactly one bit");
    mask.trailing_zeros()
}

#[test]
fn mask_bits_follow_cpp_allocation_order_and_overrides_overlay() {
    let center = engine_center();
    load(&center, DEFAULT_UPGRADE_INI).expect("Default/Upgrade.ini");
    load(&center, UPGRADE_INI).expect("Upgrade.ini");
    let center = center.read().unwrap();

    // C++ UpgradeCenter::init: veterancy first, bits 0..2.
    assert_eq!(bit_of(&center, "Upgrade_Veterancy_VETERAN"), 0);
    assert_eq!(bit_of(&center, "Upgrade_Veterancy_ELITE"), 1);
    assert_eq!(bit_of(&center, "Upgrade_Veterancy_HEROIC"), 2);
    // Then file order: Default/Upgrade.ini, then Upgrade.ini new names only.
    assert_eq!(bit_of(&center, "DefaultUpgrade"), 3);
    assert_eq!(bit_of(&center, "Upgrade_ParityA"), 4);
    assert_eq!(bit_of(&center, "Upgrade_ParityB"), 5);
    assert_eq!(bit_of(&center, "Upgrade_ParityC"), 6);
    assert_eq!(center.count(), 7, "override blocks allocate no template");

    // Override overlays: BuildCost replaced, earlier fields kept.
    let a = center.find_upgrade("Upgrade_ParityA").unwrap();
    assert_eq!(a.get_cost(), 900);
    assert_eq!(
        a.get_build_time(),
        7.0,
        "inherited DefaultUpgrade field kept"
    );
    assert_eq!(a.get_button_image_name().as_str(), "SSParityA");

    // C++ head-insert list order.
    let names: Vec<String> = center
        .get_upgrade_names()
        .into_iter()
        .map(|name| name.to_string())
        .collect();
    assert_eq!(
        names,
        [
            "Upgrade_ParityC",
            "Upgrade_ParityB",
            "Upgrade_ParityA",
            "DefaultUpgrade",
            "Upgrade_Veterancy_HEROIC",
            "Upgrade_Veterancy_ELITE",
            "Upgrade_Veterancy_VETERAN",
        ]
    );
    let veteran = center.find_veterancy_upgrade("VETERAN").unwrap();
    assert_eq!(veteran.get_upgrade_type(), UpgradeType::Object);
}

#[test]
fn new_upgrade_copies_current_default_upgrade_contents() {
    let center = engine_center();
    load(&center, DEFAULT_UPGRADE_INI).unwrap();
    load(&center, UPGRADE_INI).unwrap();
    let center = center.read().unwrap();

    // B was created before DefaultUpgrade's override: cost 100.
    let b = center.find_upgrade("Upgrade_ParityB").unwrap();
    assert_eq!(b.get_cost(), 100);
    assert_eq!(b.get_build_time(), 12.5);
    assert_eq!(b.get_upgrade_type(), UpgradeType::Object);
    // C was created after the override: C++ copies *current* DefaultUpgrade.
    let c = center.find_upgrade("Upgrade_ParityC").unwrap();
    assert_eq!(c.get_cost(), 250);
    assert_eq!(c.get_build_time(), 7.0);
    assert_eq!(c.get_display_name().as_str(), "UPGRADE:ParityC");
    assert_eq!(c.get_name().as_str(), "Upgrade_ParityC");
    assert_ne!(
        c.get_mask(),
        center.find_upgrade("DefaultUpgrade").unwrap().get_mask()
    );
}

#[test]
fn later_ini_upgrade_gets_fresh_unique_bit() {
    // map.ini-style late block: a new name gets the next bit, never bit 0.
    let center = engine_center();
    load(&center, DEFAULT_UPGRADE_INI).unwrap();
    load(
        &center,
        "Upgrade MapOnlyParityUpgrade\n  BuildCost = 5\nEnd\n",
    )
    .unwrap();
    let center = center.read().unwrap();
    let bit = bit_of(&center, "MapOnlyParityUpgrade");
    assert_eq!(bit, 5);
    let others = center
        .get_all_upgrades()
        .iter()
        .filter(|t| t.get_name().as_str() != "MapOnlyParityUpgrade")
        .fold(0u128, |acc, t| acc | t.get_mask().bits());
    assert_eq!(others & (1u128 << bit), 0);
}

#[test]
fn unknown_field_still_allocates_bit_like_cpp_new_upgrade() {
    let center = engine_center();
    let result = load(
        &center,
        "Upgrade BadFieldUpgrade\n  BuildCost = 5\n  Bogus = 1\nEnd\n",
    );
    assert!(result.is_err(), "C++ initFromINI rejects unknown tokens");
    let center = center.read().unwrap();
    let bad = center
        .find_upgrade("BadFieldUpgrade")
        .expect("newUpgrade ran first");
    assert_eq!(bad.get_mask().bits(), 1 << 3);
    assert_eq!(bad.get_cost(), 5, "fields before the error stay applied");
}

#[test]
fn xfer_upgrade_mask_round_trips_by_name() {
    initialize_upgrade_center();
    let mask = {
        let center = get_upgrade_center();
        let mut center = center.write().unwrap();
        let a = center.new_upgrade(AsciiString::from("XferRoundTripParityA"));
        center.new_upgrade(AsciiString::from("XferRoundTripParityB"));
        let c = center.new_upgrade(AsciiString::from("XferRoundTripParityC"));
        let heroic = center.find_veterancy_upgrade("HEROIC").unwrap();
        a.get_mask().bits() | c.get_mask().bits() | heroic.get_mask().bits()
    };

    let mut saved = Vec::new();
    {
        let mut xfer = XferSave::new(Cursor::new(&mut saved), 0);
        let mut bits = mask;
        xfer.xfer_upgrade_mask(&mut bits).unwrap();
    }
    // version + count + names
    assert_eq!(saved[0], 1);
    assert_eq!(u16::from_le_bytes([saved[1], saved[2]]), 3);

    let mut loaded = 0u128;
    let mut xfer = XferLoad::new(Cursor::new(saved), 0);
    xfer.xfer_upgrade_mask(&mut loaded).unwrap();
    assert_eq!(loaded, mask);
}

#[test]
fn xfer_upgrade_mask_rejects_unknown_name() {
    initialize_upgrade_center();
    let mut bytes = vec![1u8, 1, 0];
    let name = b"XferNeverDefinedParityUpgrade";
    bytes.push(name.len() as u8);
    bytes.extend_from_slice(name);
    let mut loaded = 0u128;
    let mut xfer = XferLoad::new(Cursor::new(bytes), 0);
    assert!(xfer.xfer_upgrade_mask(&mut loaded).is_err());
}

#[test]
fn cloned_center_lookups_survive_foreign_thread_key_namespace() {
    use crate::common::name_key_generator::NameKeyGenerator;

    // Engine-lifetime center built on one thread (its keys: VETERAN = 1, ...).
    let built = std::thread::spawn(|| {
        let center = engine_center();
        load(
            &center,
            "Upgrade Upgrade_WorkerCompletionParity\n  BuildCost = 75\nEnd\n",
        )
        .unwrap();
        let center = center.read().unwrap().clone();
        center
    })
    .join()
    .expect("producer thread");

    std::thread::spawn(move || {
        // A world clone used on a thread whose generator allocated other
        // names first: numeric key 1 now means something else.
        let world = built.clone();
        let unrelated = NameKeyGenerator::name_to_key("UnrelatedThreadLocalName");
        let worker_key = NameKeyGenerator::name_to_key("Upgrade_WorkerCompletionParity");
        assert_eq!(unrelated, 1);

        assert!(world.find_upgrade_by_key(unrelated).is_none());
        let by_key = world.find_upgrade_by_key(worker_key).expect("by key");
        assert_eq!(by_key.get_name().as_str(), "Upgrade_WorkerCompletionParity");
        assert_eq!(by_key.get_name_key(), worker_key);
        let by_name = world
            .find_upgrade("Upgrade_WorkerCompletionParity")
            .unwrap();
        assert_eq!(by_name.get_cost(), 75);
        assert_eq!(by_name.get_mask().bits(), 1 << 3, "mask order unchanged");

        let veteran = world.find_veterancy_upgrade("VETERAN").unwrap();
        assert_eq!(
            world
                .find_upgrade_by_key(veteran.get_name_key())
                .unwrap()
                .get_name(),
            veteran.get_name()
        );
        assert!(
            world
                .find_upgrade("upgrade_workercompletionparity")
                .is_none()
        );
    })
    .join()
    .expect("consumer thread");
}

#[test]
fn mask_lookup_returns_none_for_unknown_names() {
    initialize_upgrade_center();
    assert!(upgrade_mask_for_name("NeverDefinedParityUpgrade").is_none());
    assert_eq!(
        upgrade_mask_for_name("Upgrade_Veterancy_VETERAN").map(|mask| mask.bits()),
        Some(1)
    );
}
