// WeaponSet.cpp:197-267 and Weapon.cpp Xfer: immutable definitions belong to
// the receiving session, even when another session uses the same names.
use super::*;
use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
use std::io::Cursor;

fn authored(template: Arc<WeaponTemplate>) -> WeaponSet {
    let mut set = WeaponSet::new();
    set.weapon_template_sets
        .push(test_template_set(template, None, false));
    set.remember_thing_template_name("OwnedSoldier");
    set.update_weapon_set_with_services(
        7,
        &WeaponSetFlags::new(),
        crate::common::types::WeaponBonusConditionFlags::empty(),
        40,
        &crate::weapon::WeaponBonusSet::default(),
    )
    .unwrap();
    set
}

fn save(set: &mut WeaponSet) -> Vec<u8> {
    let mut bytes = Vec::new();
    set.xfer_with_definitions(
        &mut XferSave::new(Cursor::new(&mut bytes), 1),
        40,
        &mut |_| panic!("save must not resolve a thing"),
        &mut |_| panic!("save must not resolve a weapon"),
    )
    .unwrap();
    bytes
}

#[test]
fn same_name_load_uses_exact_receiving_definitions_and_preserves_wire() {
    let source_template = test_weapon_template("OwnedGun", 4);
    let mut source = authored(source_template);
    let bytes = save(&mut source);
    let peer_template = test_weapon_template("OwnedGun", 19);
    let mut peer = authored(peer_template.clone());
    let receiver_template = test_weapon_template("OwnedGun", 8);
    let mut receiver = authored(receiver_template.clone());
    let mut names = Vec::new();
    receiver
        .xfer_with_definitions(
            &mut XferLoad::new(Cursor::new(&bytes), 1),
            40,
            &mut |name| name == "OwnedSoldier",
            &mut |name| {
                names.push(name.to_owned());
                Ok((name == "OwnedGun").then(|| receiver_template.clone()))
            },
        )
        .unwrap();
    assert_eq!(names, ["OwnedGun"]);
    let weapon = receiver.get_current_weapon().unwrap().0;
    assert!(Arc::ptr_eq(&weapon.template, &receiver_template));
    assert_eq!(weapon.template.clip_size, 8);
    assert!(Arc::ptr_eq(
        &peer.get_current_weapon().unwrap().0.template,
        &peer_template
    ));
    assert_eq!(
        save(&mut receiver),
        bytes,
        "definition identity is not serialized"
    );
    assert_eq!(peer.get_current_weapon().unwrap().0.template.clip_size, 19);
    assert_ne!(
        save(&mut peer),
        bytes,
        "independent authored clip state remains distinct"
    );
}

#[test]
fn missing_owned_thing_or_weapon_is_reported_without_peer_mutation() {
    let mut source = authored(test_weapon_template("OwnedGun", 4));
    let bytes = save(&mut source);
    let mut peer = authored(test_weapon_template("OwnedGun", 19));
    let before = save(&mut peer);
    let mut receiver = authored(test_weapon_template("OwnedGun", 8));
    let error = receiver
        .xfer_with_definitions(
            &mut XferLoad::new(Cursor::new(&bytes), 1),
            40,
            &mut |_| false,
            &mut |_| panic!("weapon lookup follows thing validation"),
        )
        .unwrap_err();
    assert!(error.contains("missing thing template OwnedSoldier"));
    let error = receiver
        .xfer_with_definitions(
            &mut XferLoad::new(Cursor::new(&bytes), 1),
            40,
            &mut |_| true,
            &mut |_| Ok(None),
        )
        .unwrap_err();
    assert!(error.contains("missing template OwnedGun"));
    assert_eq!(save(&mut peer), before);
}

#[test]
fn saved_projectile_stream_fixup_uses_only_the_receiving_object_store() {
    let template = test_weapon_template("OwnedGun", 4);
    let mut source = Weapon::new_at(template.clone(), WeaponSlotType::Primary, 40);
    source.projectile_stream_id = 77;
    let mut bytes = Vec::new();
    source
        .xfer_with_templates(&mut XferSave::new(Cursor::new(&mut bytes), 1), &mut |_| {
            panic!("saving does not resolve definitions")
        })
        .unwrap();
    let mut receiver = Weapon::new_at(template.clone(), WeaponSlotType::Primary, 40);
    receiver
        .xfer_with_templates(&mut XferLoad::new(Cursor::new(&bytes), 1), &mut |_| {
            Ok(Some(template.clone()))
        })
        .unwrap();
    receiver.load_post_process_with_objects(&mut |id| {
        assert_eq!(id, 77);
        true
    });
    assert_eq!(receiver.projectile_stream_id, 77);
    let mut peer = Weapon::new_at(template, WeaponSlotType::Primary, 40);
    peer.projectile_stream_id = 77;
    peer.load_post_process_with_objects(&mut |id| {
        assert_eq!(id, 77);
        false
    });
    assert_eq!(peer.projectile_stream_id, crate::common::INVALID_ID);
    assert_eq!(receiver.projectile_stream_id, 77);
    assert_eq!(source.projectile_stream_id, 77);
}

#[test]
fn owned_load_constructs_missing_slot_without_observing_an_ambient_clock() {
    let mut source = authored(test_weapon_template("OwnedGun", 4));
    let bytes = save(&mut source);
    let template = test_weapon_template("OwnedGun", 8);
    let mut receiver = authored(template.clone());
    receiver.weapons[0] = None;
    receiver
        .xfer_with_definitions(
            &mut XferLoad::new(Cursor::new(&bytes), 1),
            900,
            &mut |name| name == "OwnedSoldier",
            &mut |name| Ok((name == "OwnedGun").then(|| template.clone())),
        )
        .unwrap();
    assert!(Arc::ptr_eq(
        &receiver.weapons[0].as_ref().unwrap().template,
        &template
    ));
    assert_eq!(
        save(&mut receiver),
        bytes,
        "loaded timer fields replace construction defaults"
    );
}

#[test]
fn unknown_and_negative_weapon_set_flag_records_are_errors() {
    for count in [-1_i32, 1] {
        let mut bytes = Vec::new();
        let mut writer = XferSave::new(Cursor::new(&mut bytes), 1);
        writer.xfer_version(&mut 1, 1).unwrap();
        let mut encoded_count = count;
        writer.xfer_int(&mut encoded_count).unwrap();
        if count > 0 {
            writer
                .xfer_ascii_string(&mut "NOT_A_WEAPON_SET".to_owned())
                .unwrap();
        }
        let mut flags = 0;
        let error = xfer_named_bits(
            &mut XferLoad::new(Cursor::new(bytes), 1),
            &mut flags,
            WEAPON_SET_XFER_NAMES,
        )
        .unwrap_err();
        assert!(error.contains(if count < 0 {
            "negative"
        } else {
            "NOT_A_WEAPON_SET"
        }));
    }
}

#[test]
fn sparse_match_load_uses_receiving_definition_order_and_rejects_foreign_set_owner() {
    let source_template = test_weapon_template("OwnedGun", 4);
    let mut source = authored(source_template.clone());
    let mut upgraded = WeaponTemplateSet::new();
    upgraded.conditions.set(WeaponSetType::Veteran);
    upgraded.weapon_templates[0] = Some(source_template);
    source.current_weapon_template_set = Some(Arc::new(upgraded));
    let bytes = save(&mut source);
    let template = test_weapon_template("OwnedGun", 8);
    let mut receiver = authored(template.clone());
    let default_set = receiver.weapon_template_sets[0].clone();
    receiver
        .xfer_with_definitions(
            &mut XferLoad::new(Cursor::new(&bytes), 1),
            40,
            &mut |name| name == "OwnedSoldier",
            &mut |_| Ok(Some(template.clone())),
        )
        .unwrap();
    // CPP SparseMatchFinder permits a default set when the requested Veteran
    // record has no exact receiving set; the original authored order wins ties.
    assert!(Arc::ptr_eq(
        receiver.current_weapon_template_set.as_ref().unwrap(),
        &default_set
    ));
    receiver.remember_thing_template_name("ForeignSoldier");
    let error = receiver
        .xfer_with_definitions(
            &mut XferLoad::new(Cursor::new(bytes), 1),
            40,
            &mut |_| true,
            &mut |_| Ok(Some(template.clone())),
        )
        .unwrap_err();
    assert!(error.contains("receiving definitions belong to ForeignSoldier"));
}
