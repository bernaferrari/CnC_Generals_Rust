//! Upgrade Center — GameLogic access to the canonical C++ `TheUpgradeCenter`.
//!
//! `UpgradeCenter` is the Common type (C++ Common/System/Upgrade.cpp). The
//! per-world instance is owned by `EngineStores`; [`get_upgrade_center`] is a
//! thin forward to it. Only `canAffordUpgrade` (needs GameLogic `Player`)
//! lives here.
//!
//! Original C++ Author: Colin Day, March 2002

use std::sync::{Arc, RwLock};

use super::UpgradeTemplate;
use super::template::UpgradeTemplatePlayerExt;
use crate::common::*;

pub use game_engine::common::system::upgrade::UpgradeCenter;

/// C++ `UpgradeCenter` members that take a `Player*`.
pub trait UpgradeCenterPlayerExt {
    /// C++ `UpgradeCenter::canAffordUpgrade` (Upgrade.cpp:395-420).
    fn can_afford_upgrade(
        &self,
        player: &Player,
        template: &UpgradeTemplate,
        display_reason: bool,
    ) -> bool;
}

impl UpgradeCenterPlayerExt for UpgradeCenter {
    fn can_afford_upgrade(
        &self,
        player: &Player,
        template: &UpgradeTemplate,
        display_reason: bool,
    ) -> bool {
        let cost = template.calc_cost_to_build(player);
        if player.get_money().get_money() < cost {
            if display_reason {
                crate::helpers::TheInGameUI::display_message("GUI:NotEnoughMoneyToUpgrade");
            }
            return false;
        }
        true
    }
}

/// C++ `TheUpgradeCenter`: the active world's center (EngineStores).
pub fn get_upgrade_center() -> Arc<RwLock<UpgradeCenter>> {
    crate::system::engine_stores::upgrade_center()
}

pub fn with_upgrade_center<F, R>(f: F) -> R
where
    F: FnOnce(&UpgradeCenter) -> R,
{
    let center = get_upgrade_center();
    let center = center.read().expect("UpgradeCenter lock poisoned");
    f(&center)
}

pub fn with_upgrade_center_mut<F, R>(f: F) -> R
where
    F: FnOnce(&mut UpgradeCenter) -> R,
{
    let center = get_upgrade_center();
    let mut center = center.write().expect("UpgradeCenter lock poisoned");
    f(&mut center)
}

#[cfg(test)]
mod tests {
    use super::super::UpgradeType;
    use super::*;
    use game_engine::common::ini::INI;

    fn setup_test_center() -> UpgradeCenter {
        let mut center = UpgradeCenter::new();
        center.init();
        center
    }

    fn parse(center: &mut UpgradeCenter, source: &str) {
        let mut ini = INI::new();
        ini.with_inline_source(source, |ini| {
            ini.read_line()?;
            center.parse_upgrade_definition(ini)
        })
        .expect("upgrade definition");
    }

    #[test]
    fn test_upgrade_center_creation() {
        let center = UpgradeCenter::new();
        assert_eq!(center.count(), 0);
    }

    #[test]
    fn test_upgrade_center_init() {
        let center = setup_test_center();
        assert_eq!(center.count(), 3);
    }

    #[test]
    fn global_center_boot_creates_veterancy_templates() {
        // This asserts engine boot, before authored upgrades mutate the shared
        // center. Suite order must not supply (or overwrite) the fixture.
        #[cfg(not(target_arch = "wasm32"))]
        if matches!(
            crate::test_process::run_bounded(
                "upgrade::center::tests::global_center_boot_creates_veterancy_templates",
                "GENERALS_UPGRADE_CENTER_BOOT_CHILD",
            ),
            crate::test_process::TestProcess::ParentVerified
        ) {
            return;
        }
        with_upgrade_center(|center| {
            let veteran = center
                .find_veterancy_upgrade("VETERAN")
                .expect("Upgrade_Veterancy_VETERAN");
            assert_eq!(veteran.get_upgrade_type(), UpgradeType::Object);
            assert_eq!(veteran.get_mask().bits(), 1);
            assert_eq!(
                center
                    .find_veterancy_upgrade("ELITE")
                    .unwrap()
                    .get_mask()
                    .bits(),
                2
            );
            assert_eq!(
                center
                    .find_veterancy_upgrade("HEROIC")
                    .unwrap()
                    .get_mask()
                    .bits(),
                4
            );
        });
    }

    #[test]
    fn gamelogic_and_common_resolve_the_same_center() {
        // Process-global resolution: isolate from tests that install worlds.
        #[cfg(not(target_arch = "wasm32"))]
        if matches!(
            crate::test_process::run_bounded(
                "upgrade::center::tests::gamelogic_and_common_resolve_the_same_center",
                "GENERALS_UPGRADE_CENTER_SAME_STORE_CHILD",
            ),
            crate::test_process::TestProcess::ParentVerified
        ) {
            return;
        }
        let gamelogic = get_upgrade_center();
        let common = game_engine::common::system::upgrade::get_upgrade_center();
        assert!(Arc::ptr_eq(&gamelogic, &common));
    }

    #[test]
    fn test_create_and_find_upgrade() {
        let mut center = UpgradeCenter::new();
        let template = center.new_upgrade(AsciiString::from("TestUpgrade"));
        assert_eq!(template.get_name().as_str(), "TestUpgrade");
        assert_eq!(center.count(), 1);
        let found = center.find_upgrade("TestUpgrade").unwrap();
        assert_eq!(found.get_mask(), template.get_mask());
    }

    #[test]
    fn upgrade_center_uses_cpp_head_insertion_order() {
        let mut center = UpgradeCenter::new();
        center.new_upgrade(AsciiString::from("FirstUpgrade"));
        center.new_upgrade(AsciiString::from("SecondUpgrade"));
        center.new_upgrade(AsciiString::from("ThirdUpgrade"));

        let names: Vec<_> = center
            .get_upgrade_names()
            .into_iter()
            .map(|name| name.to_string())
            .collect();
        assert_eq!(names, vec!["ThirdUpgrade", "SecondUpgrade", "FirstUpgrade"]);
        assert_eq!(
            center.first_upgrade().unwrap().get_name().as_str(),
            "ThirdUpgrade"
        );
    }

    #[test]
    fn init_veterancy_upgrades_match_cpp_list_order() {
        let center = setup_test_center();
        let names: Vec<_> = center
            .get_upgrade_names()
            .into_iter()
            .map(|name| name.to_string())
            .collect();
        assert_eq!(
            names,
            [
                "Upgrade_Veterancy_HEROIC",
                "Upgrade_Veterancy_ELITE",
                "Upgrade_Veterancy_VETERAN",
            ]
        );
    }

    #[test]
    fn parsed_existing_upgrade_overlays_in_place() {
        let mut center = UpgradeCenter::new();
        let existing = center.new_upgrade(AsciiString::from("ExistingUpgrade"));
        center.new_upgrade(AsciiString::from("OtherUpgrade"));

        parse(
            &mut center,
            "Upgrade ExistingUpgrade\nBuildCost = 777\nBuildTime = 12.5\nEnd\n",
        );
        parse(
            &mut center,
            "Upgrade ExistingUpgrade\nBuildCost = 778\nEnd\n",
        );

        let listed = center
            .get_all_upgrades()
            .iter()
            .find(|upgrade| upgrade.get_name().as_str() == "ExistingUpgrade")
            .unwrap();
        assert_eq!(listed.get_cost(), 778);
        assert_eq!(listed.get_build_time(), 12.5);
        assert_eq!(listed.get_mask(), existing.get_mask());

        let names: Vec<_> = center
            .get_upgrade_names()
            .into_iter()
            .map(|name| name.to_string())
            .collect();
        assert_eq!(names, vec!["OtherUpgrade", "ExistingUpgrade"]);
    }

    #[test]
    fn test_can_afford_upgrade() {
        let mut center = UpgradeCenter::new();
        let template = center.new_upgrade(AsciiString::from("TestUpgrade"));
        let player = Player::default();
        assert!(center.can_afford_upgrade(&player, &template, false));
    }

    #[test]
    fn test_default_upgrade_inheritance() {
        let mut center = UpgradeCenter::new();
        parse(
            &mut center,
            "Upgrade DefaultUpgrade\nBuildTime = 5.0\nBuildCost = 500\nEnd\n",
        );
        let other = center.new_upgrade(AsciiString::from("OtherUpgrade"));
        assert_eq!(other.get_build_time(), 5.0);
        assert_eq!(other.get_cost(), 500);
        assert_eq!(other.get_name().as_str(), "OtherUpgrade");
        assert_eq!(other.get_mask().bits(), 2);
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "center/namespace_tests.rs"]
mod namespace_tests;
