//! Upgrade Center - Global Registry
//!
//! Central registry for all upgrade templates in the game.
//! Matches C++ UpgradeCenter from Upgrade.h/.cpp
//!
//! Original C++ Author: Colin Day, March 2002

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use super::{
    UpgradeError, UpgradeResult, UpgradeTemplate, UpgradeType, prerequisites::get_tech_tree,
};
use crate::common::*;
use game_engine::common::ini::INI;

/// Central registry for upgrade templates
/// Matches C++ UpgradeCenter from Upgrade.h
#[derive(Clone)]
pub struct UpgradeCenter {
    /// Canonical catalog identity is the exact authored name. NameKey values
    /// belong to the generator namespace that constructed each template.
    upgrades: HashMap<String, Arc<UpgradeTemplate>>,
    /// Ordered list of upgrades (for iteration)
    upgrade_list: Vec<Arc<UpgradeTemplate>>,
    /// Default upgrade template for inheritance
    default_upgrade: Option<Arc<UpgradeTemplate>>,
    /// Next bit assigned by newUpgrade (C++ m_nextTemplateMaskBit)
    next_template_mask_bit: u32,
    /// C++ buttonImagesCached — one-shot during reset
    button_images_cached: bool,
}

impl UpgradeCenter {
    /// Create a new upgrade center
    /// Matches C++ UpgradeCenter::UpgradeCenter
    pub fn new() -> Self {
        Self {
            upgrades: HashMap::new(),
            upgrade_list: Vec::new(),
            default_upgrade: None,
            next_template_mask_bit: 0,
            button_images_cached: false,
        }
    }

    /// Initialize the upgrade center
    /// Matches C++ UpgradeCenter::init
    pub fn init(&mut self) {
        log::info!("Initializing UpgradeCenter");

        // C++ UpgradeCenter::init — create only if missing so a second
        // boot/load does not duplicate list entries.
        self.ensure_veterancy_upgrade("VETERAN");
        self.ensure_veterancy_upgrade("ELITE");
        self.ensure_veterancy_upgrade("HEROIC");

        log::info!(
            "UpgradeCenter initialized with {} upgrades",
            self.upgrades.len()
        );
    }

    /// Matches C++ UpgradeCenter::reset — cache button images once.
    pub fn reset(&mut self) {
        if self.button_images_cached {
            return;
        }
        if game_engine::common::ini::ini_mapped_image::get_mapped_image_collection().is_none() {
            return;
        }
        let list = std::mem::take(&mut self.upgrade_list);
        self.upgrade_list = list
            .into_iter()
            .map(|arc| {
                let mut template = (*arc).clone();
                template.cache_button_image();
                let cached = Arc::new(template);
                self.upgrades
                    .insert(cached.get_name().as_str().to_owned(), cached.clone());
                if cached.get_name().as_str() == "DefaultUpgrade" {
                    self.default_upgrade = Some(cached.clone());
                }
                cached
            })
            .collect();
        self.button_images_cached = true;
    }

    /// Matches C++ UpgradeCenter::newUpgrade
    pub fn new_upgrade(&mut self, name: AsciiString) -> Arc<UpgradeTemplate> {
        // Preserve construction-time key allocation; only catalog lookup is
        // independent of the calling thread's numeric namespace.
        let _name_key = NameKeyGenerator::name_to_key(&name);

        if let Some(existing) = self.upgrades.get(name.as_str()) {
            if !name.is_empty() {
                return existing.clone();
            }
        }

        let mut template = if let Some(default) = &self.default_upgrade {
            (**default).clone()
        } else {
            UpgradeTemplate::new(name.clone())
        };
        template.set_name(name.clone());

        let mut mask = super::UpgradeMask::none();
        mask.set_bit(self.next_template_mask_bit as usize);
        self.next_template_mask_bit = self.next_template_mask_bit.saturating_add(1);
        template.friend_set_upgrade_mask(mask);

        let template = Arc::new(template);
        self.upgrades
            .insert(template.get_name().as_str().to_owned(), template.clone());
        self.upgrade_list.insert(0, template.clone());

        if name.as_str() == "DefaultUpgrade" {
            self.default_upgrade = Some(template.clone());
        }

        template
    }

    fn ensure_veterancy_upgrade(&mut self, level: &str) {
        let name = format!("Upgrade_Veterancy_{level}");
        if self.find_upgrade(&name).is_some() {
            return;
        }
        self.create_veterancy_upgrade(level);
    }

    fn create_veterancy_upgrade(&mut self, level: &str) {
        let template = self.new_upgrade(AsciiString::from(""));
        let empty_name = template.get_name().as_str().to_owned();
        let mut owned = (*template).clone();
        owned.friend_make_veterancy_upgrade(level);
        let template = Arc::new(owned);
        self.upgrades.remove(&empty_name);
        self.upgrades
            .insert(template.get_name().as_str().to_owned(), template.clone());
        if let Some(slot) = self.upgrade_list.first_mut() {
            *slot = template;
        }
    }

    fn store_parsed_template(&mut self, _name_key: NameKeyType, template: Arc<UpgradeTemplate>) {
        self.upgrades
            .insert(template.get_name().as_str().to_owned(), template.clone());

        if let Some(existing) = self
            .upgrade_list
            .iter_mut()
            .find(|upgrade| upgrade.get_name() == template.get_name())
        {
            *existing = template;
        } else {
            self.upgrade_list.insert(0, template);
        }
    }

    /// Exact authored-name lookup in this catalog. C++ has one process key
    /// namespace; Rust catalogs may outlive or cross a TLS key namespace.
    /// Queries do not intern absent names; future numeric IDs may therefore
    /// differ from the C++ global generator's lookup-side allocation.
    pub fn find_upgrade(&self, name: &str) -> Option<Arc<UpgradeTemplate>> {
        self.upgrades.get(name).cloned()
    }

    /// Find upgrade by name key
    /// Matches C++ UpgradeCenter::findUpgradeByKey
    pub fn find_upgrade_by_key(&self, key: NameKeyType) -> Option<Arc<UpgradeTemplate>> {
        // C++ walks its linked list and returns the first matching stored key.
        // Numeric keys remain compatibility metadata, not portable identity.
        self.upgrade_list
            .iter()
            .find(|template| template.get_name_key() == key)
            .cloned()
    }

    /// Find veterancy upgrade by level
    /// Matches C++ UpgradeCenter::findVeterancyUpgrade
    pub fn find_veterancy_upgrade(&self, level: &str) -> Option<Arc<UpgradeTemplate>> {
        let name = format!("Upgrade_Veterancy_{}", level);
        self.find_upgrade(&name)
    }

    /// Get first upgrade template (for iteration)
    /// Matches C++ UpgradeCenter::firstUpgradeTemplate
    pub fn first_upgrade(&self) -> Option<Arc<UpgradeTemplate>> {
        self.upgrade_list.first().cloned()
    }

    /// Get all upgrade templates
    pub fn get_all_upgrades(&self) -> &[Arc<UpgradeTemplate>] {
        &self.upgrade_list
    }

    /// Get upgrade names (for WorldBuilder)
    /// Matches C++ UpgradeCenter::getUpgradeNames
    pub fn get_upgrade_names(&self) -> Vec<AsciiString> {
        self.upgrade_list
            .iter()
            .map(|t| t.get_name().clone())
            .collect()
    }

    /// Check if player can afford upgrade
    /// Matches C++ UpgradeCenter::canAffordUpgrade
    pub fn can_afford_upgrade(
        &self,
        player: &Player,
        template: &UpgradeTemplate,
        display_reason: bool,
    ) -> bool {
        let cost = template.calc_cost_to_build(player);
        let money = player.get_money();

        if money.get_money() < cost {
            if display_reason {
                crate::helpers::TheInGameUI::display_message("GUI:NotEnoughMoneyToUpgrade");
            }
            return false;
        }

        true
    }

    /// Parse upgrade definition from INI
    /// Matches C++ UpgradeCenter::parseUpgradeDefinition
    pub fn parse_upgrade_definition(&mut self, ini: &mut INI) -> Result<(), String> {
        // Read upgrade name
        let name_token = ini.get_next_token().map_err(|e| format!("{:?}", e))?;
        let name = AsciiString::from(name_token.as_str());

        log::debug!("Parsing upgrade definition: {}", name);

        // Find or create upgrade
        let name_key = NameKeyGenerator::name_to_key(&name);
        let mut template = if let Some(existing) = self.upgrades.get(name.as_str()) {
            // Clone existing to modify
            (**existing).clone()
        } else {
            // Create new
            UpgradeTemplate::new(name.clone())
        };

        // Parse INI fields
        template
            .parse_from_ini(ini)
            .map_err(|e| format!("Failed to parse upgrade '{}': {:?}", name, e))?;

        // Store updated template
        let template = Arc::new(template);
        self.store_parsed_template(name_key, template);

        Ok(())
    }

    /// Get number of registered upgrades
    pub fn count(&self) -> usize {
        self.upgrades.len()
    }
}

impl Default for UpgradeCenter {
    fn default() -> Self {
        Self::new()
    }
}

/// Global accessor functions
/// Matches C++ TheUpgradeCenter usage

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
    use super::*;

    fn setup_test_center() -> UpgradeCenter {
        let mut center = UpgradeCenter::new();
        center.init();
        center
    }

    #[test]
    fn test_upgrade_center_creation() {
        let center = UpgradeCenter::new();
        assert_eq!(center.count(), 0);
    }

    #[test]
    fn test_upgrade_center_init() {
        let center = setup_test_center();
        assert!(center.count() >= 3); // At least 3 veterancy upgrades
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
            assert!(center.find_veterancy_upgrade("ELITE").is_some());
            assert!(center.find_veterancy_upgrade("HEROIC").is_some());
        });
    }

    #[test]
    fn test_create_upgrade() {
        let mut center = UpgradeCenter::new();
        let template = center.new_upgrade(AsciiString::from("TestUpgrade"));

        assert_eq!(template.get_name().as_str(), "TestUpgrade");
        assert_eq!(center.count(), 1);
    }

    #[test]
    fn test_find_upgrade() {
        let mut center = UpgradeCenter::new();
        center.new_upgrade(AsciiString::from("TestUpgrade"));

        let found = center.find_upgrade("TestUpgrade");
        assert!(found.is_some());
        assert_eq!(found.unwrap().get_name().as_str(), "TestUpgrade");
    }

    #[test]
    fn test_find_veterancy_upgrade() {
        let center = setup_test_center();

        let veteran = center.find_veterancy_upgrade("VETERAN");
        assert!(veteran.is_some());
        assert_eq!(veteran.unwrap().get_upgrade_type(), UpgradeType::Object);
    }

    #[test]
    fn test_get_all_upgrades() {
        let center = setup_test_center();
        let upgrades = center.get_all_upgrades();
        assert!(upgrades.len() >= 3);
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
            &names[..3],
            &[
                "Upgrade_Veterancy_HEROIC".to_string(),
                "Upgrade_Veterancy_ELITE".to_string(),
                "Upgrade_Veterancy_VETERAN".to_string(),
            ]
        );
    }

    #[test]
    fn parsed_existing_upgrade_refreshes_iteration_entry() {
        let mut center = UpgradeCenter::new();
        center.new_upgrade(AsciiString::from("ExistingUpgrade"));
        center.new_upgrade(AsciiString::from("OtherUpgrade"));

        let mut reparsed = UpgradeTemplate::new(AsciiString::from("ExistingUpgrade"));
        reparsed.set_cost(777);
        reparsed.set_build_time(12.5);
        let name_key = reparsed.get_name_key();
        center.store_parsed_template(name_key, Arc::new(reparsed));

        assert_eq!(
            center.find_upgrade("ExistingUpgrade").unwrap().get_cost(),
            777
        );

        let listed = center
            .get_all_upgrades()
            .iter()
            .find(|upgrade| upgrade.get_name().as_str() == "ExistingUpgrade")
            .unwrap();
        assert_eq!(listed.get_cost(), 777);
        assert_eq!(listed.get_build_time(), 12.5);

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
        // Assuming player starts with enough money
        assert!(center.can_afford_upgrade(&player, &template, false));
    }

    #[test]
    fn test_default_upgrade_inheritance() {
        let mut center = UpgradeCenter::new();

        // Seed defaults directly (templates are immutable once registered).
        let mut default_template = UpgradeTemplate::new(AsciiString::from("DefaultUpgrade"));
        default_template.set_build_time(5.0);
        default_template.set_cost(500);
        center.default_upgrade = Some(Arc::new(default_template));

        // Create another upgrade - should inherit defaults
        let other = center.new_upgrade(AsciiString::from("OtherUpgrade"));
        assert!(center.default_upgrade.is_some());
        assert_eq!(other.get_build_time(), 5.0);
        assert_eq!(other.get_cost(), 500);
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "center/namespace_tests.rs"]
mod namespace_tests;
