//! Actual immutable upgrade definitions created on one thread and used on
//! another. Numeric NameKey metadata is a compatibility namespace, not catalog
//! identity. These witnesses use production creation/parsing/query operations.
use super::*;

fn parse(center: &mut UpgradeCenter, name: &str, kind: &str, cost: u32, seconds: u32) {
    if center.find_upgrade(name).is_none() {
        center.new_upgrade(AsciiString::from(name));
    }
    let source = format!("{name}\nType = {kind}\nBuildCost = {cost}\nBuildTime = {seconds}\nEnd\n");
    let mut ini = INI::new();
    ini.with_inline_source(&source, |ini| {
        ini.read_line()?;
        center
            .parse_upgrade_definition(ini)
            .map_err(|_| game_engine::common::ini::INIError::InvalidData)
    })
    .expect("actual native upgrade parser");
}

fn producer_catalog() -> UpgradeCenter {
    std::thread::spawn(|| {
        let mut center = UpgradeCenter::new();
        parse(&mut center, "OwnedCatalogPlayer", "PLAYER", 321, 7);
        parse(&mut center, "OwnedCatalogObject", "OBJECT", 654, 19);
        let player = center.find_upgrade("OwnedCatalogPlayer").unwrap();
        assert_eq!(player.get_name_key(), 1, "fresh producer namespace");
        assert_eq!(player.get_upgrade_type(), UpgradeType::Player);
        assert_eq!(player.get_mask().to_bits(), 1);
        assert_eq!(
            center
                .find_upgrade("OwnedCatalogObject")
                .unwrap()
                .get_mask()
                .to_bits(),
            2
        );
        center
    })
    .join()
    .expect("producer thread")
}

#[test]
fn named_catalog_lookup_ignores_foreign_sequential_key_namespace() {
    let center = producer_catalog();
    std::thread::spawn(move || {
        assert_eq!(
            NameKeyGenerator::name_to_key("AbsentCatalogRule"),
            1,
            "different consumer name genuinely collides with producer's key1"
        );
        assert!(
            center.find_upgrade("AbsentCatalogRule").is_none(),
            "missing rule must not resolve another template with the same numeric key"
        );
        let player = center
            .find_upgrade("OwnedCatalogPlayer")
            .expect("owned real player rules");
        assert_eq!(player.get_name().as_str(), "OwnedCatalogPlayer");
        assert_eq!(player.get_upgrade_type(), UpgradeType::Player);
        assert_eq!(player.get_cost(), 321);
        assert_eq!(player.get_build_time(), 7.0);
        assert_eq!(
            player.get_name_key(),
            1,
            "original definition metadata remains intact"
        );
        let object = center.find_upgrade("OwnedCatalogObject").unwrap();
        assert_eq!(object.get_upgrade_type(), UpgradeType::Object);
        assert_eq!(object.get_cost(), 654);
        assert_eq!(object.get_build_time(), 19.0);
        assert!(
            center.find_upgrade("ownedcatalogplayer").is_none(),
            "CPP names remain exact-case"
        );
        assert_eq!(center.count(), 2);
    })
    .join()
    .expect("consumer thread");
}

#[test]
fn named_catalog_registration_and_reparse_keep_other_templates_and_masks() {
    let mut center = producer_catalog();
    std::thread::spawn(move || {
        assert_eq!(NameKeyGenerator::name_to_key("OwnedCatalogThird"), 1);
        let added = center.new_upgrade(AsciiString::from("OwnedCatalogThird"));
        assert_eq!(
            added.get_name().as_str(),
            "OwnedCatalogThird",
            "new named rule must not return a foreign-key alias"
        );
        assert_eq!(
            added.get_mask().to_bits(),
            4,
            "next mask allocation remains ordered"
        );
        assert_eq!(
            added.get_name_key(),
            1,
            "numeric metadata retains this namespace's actual allocation"
        );
        assert_eq!(center.count(), 3);
        assert_eq!(NameKeyGenerator::name_to_key("ConsumerPadding"), 2);
        assert_eq!(NameKeyGenerator::name_to_key("OwnedCatalogPlayer"), 3);
        parse(&mut center, "OwnedCatalogPlayer", "PLAYER", 777, 23);
        let player = center.find_upgrade("OwnedCatalogPlayer").unwrap();
        assert_eq!(player.get_cost(), 777);
        assert_eq!(player.get_build_time(), 23.0);
        assert_eq!(
            player.get_mask().to_bits(),
            1,
            "reparse preserves original mask"
        );
        assert_eq!(
            player.get_name_key(),
            1,
            "reparse preserves original template key metadata"
        );
        let object = center.find_upgrade("OwnedCatalogObject").unwrap();
        assert_eq!(object.get_upgrade_type(), UpgradeType::Object);
        assert_eq!(object.get_cost(), 654);
        assert_eq!(object.get_mask().to_bits(), 2);
        assert_eq!(
            center
                .find_upgrade("OwnedCatalogThird")
                .unwrap()
                .get_mask()
                .to_bits(),
            4
        );
        assert_eq!(
            center.count(),
            3,
            "reparse replaces only its exact-name list slot"
        );
        assert_eq!(
            center
                .get_all_upgrades()
                .iter()
                .filter(|t| t.get_name().as_str() == "OwnedCatalogPlayer")
                .count(),
            1
        );
        // Explicit numeric lookup remains C++ first matching linked-list order.
        // It is deliberately not a cross-namespace identity API.
        assert_eq!(
            center.find_upgrade_by_key(1).unwrap().get_name().as_str(),
            "OwnedCatalogThird"
        );
        assert_eq!(
            center.first_upgrade().unwrap().get_name().as_str(),
            "OwnedCatalogThird"
        );
    })
    .join()
    .expect("consumer registration thread");
}
