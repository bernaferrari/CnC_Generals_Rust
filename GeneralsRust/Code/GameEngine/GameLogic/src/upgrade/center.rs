//! Upgrade Center - Global Registry
//!
//! Central registry for all upgrade templates in the game.
//! Matches C++ UpgradeCenter from Upgrade.h/.cpp
//!
//! Original C++ Author: Colin Day, March 2002

use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use super::{
    prerequisites::get_tech_tree, UpgradeError, UpgradeResult, UpgradeTemplate, UpgradeType,
};
use crate::common::*;
use game_engine::common::ini::INI;

/// Global upgrade center singleton
/// Matches C++ TheUpgradeCenter
pub static THE_UPGRADE_CENTER: Lazy<Arc<RwLock<UpgradeCenter>>> =
    Lazy::new(|| Arc::new(RwLock::new(UpgradeCenter::new())));

/// Central registry for upgrade templates
/// Matches C++ UpgradeCenter from Upgrade.h
pub struct UpgradeCenter {
    /// All upgrade templates indexed by name key
    upgrades: HashMap<NameKeyType, Arc<UpgradeTemplate>>,
    /// Ordered list of upgrades (for iteration)
    upgrade_list: Vec<Arc<UpgradeTemplate>>,
    /// Default upgrade template for inheritance
    default_upgrade: Option<Arc<UpgradeTemplate>>,
}

impl UpgradeCenter {
    /// Create a new upgrade center
    /// Matches C++ UpgradeCenter::UpgradeCenter
    pub fn new() -> Self {
        Self {
            upgrades: HashMap::new(),
            upgrade_list: Vec::new(),
            default_upgrade: None,
        }
    }

    /// Initialize the upgrade center
    /// Matches C++ UpgradeCenter::init
    pub fn init(&mut self) {
        log::info!("Initializing UpgradeCenter");

        // Create veterancy upgrades
        // Matches C++ UpgradeCenter::init creating veterancy templates
        self.create_veterancy_upgrade("VETERAN");
        self.create_veterancy_upgrade("ELITE");
        self.create_veterancy_upgrade("HEROIC");

        log::info!(
            "UpgradeCenter initialized with {} upgrades",
            self.upgrades.len()
        );
    }

    /// Reset the upgrade center
    /// Matches C++ UpgradeCenter::reset
    pub fn reset(&mut self) {
        log::info!("Resetting UpgradeCenter");
        // Keep templates, just reset runtime state if needed
    }

    /// Create a new upgrade template
    /// Matches C++ UpgradeCenter::newUpgrade
    pub fn new_upgrade(&mut self, name: AsciiString) -> Arc<UpgradeTemplate> {
        let name_key = NameKeyGenerator::name_to_key(&name);

        // Check if already exists
        if let Some(existing) = self.upgrades.get(&name_key) {
            log::warn!("Upgrade '{}' already exists, returning existing", name);
            return existing.clone();
        }

        let mut template = UpgradeTemplate::new(name.clone());

        // Copy defaults if available
        if let Some(default) = &self.default_upgrade {
            template.set_upgrade_type(default.get_upgrade_type());
            template.set_build_time(default.get_build_time());
            template.set_cost(default.get_cost());
        }

        let template = Arc::new(template);

        // Store in registry
        self.upgrades.insert(name_key, template.clone());
        self.upgrade_list.push(template.clone());

        // Check if this is the default upgrade
        if name.as_str() == "DefaultUpgrade" {
            self.default_upgrade = Some(template.clone());
        }

        log::debug!("Created upgrade template: {}", name);
        template
    }

    /// Create a veterancy upgrade
    fn create_veterancy_upgrade(&mut self, level: &str) {
        let template = UpgradeTemplate::make_veterancy_upgrade(level);
        let name_key = template.get_name_key();
        let template = Arc::new(template);

        self.upgrades.insert(name_key, template.clone());
        self.upgrade_list.push(template);

        log::debug!("Created veterancy upgrade: {}", level);
    }

    /// Find upgrade by name
    /// Matches C++ UpgradeCenter::findUpgrade
    pub fn find_upgrade(&self, name: &str) -> Option<Arc<UpgradeTemplate>> {
        let key = NameKeyGenerator::name_to_key(name);
        self.find_upgrade_by_key(key)
    }

    /// Find upgrade by name key
    /// Matches C++ UpgradeCenter::findUpgradeByKey
    pub fn find_upgrade_by_key(&self, key: NameKeyType) -> Option<Arc<UpgradeTemplate>> {
        self.upgrades.get(&key).cloned()
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
                let message = format!(
                    "Cannot afford upgrade '{}': need {} but have {}",
                    template.get_name(),
                    cost,
                    money.get_money()
                );
                log::info!("{}", message);

                // Show UI message via TheInGameUI
                // Matches C++ UpgradeCenter displaying affordability messages
                crate::helpers::TheInGameUI::display_message(&message);
            }
            return false;
        }

        if let Some(tree_guard) = get_tech_tree() {
            if !tree_guard.can_research(template.get_name_key(), player) {
                if display_reason {
                    crate::helpers::TheInGameUI::display_message("GUI:UpgradePrereqNotMet");
                }
                return false;
            }
        }

        true
    }

    /// Parse upgrade definition from INI
    /// Matches C++ UpgradeCenter::parseUpgradeDefinition
    pub fn parse_upgrade_definition(&mut self, ini: &mut INI) -> Result<(), String> {
        // Read upgrade name
        let name_token = ini.get_next_token().ok_or("Missing upgrade name")?;
        let name = AsciiString::from(name_token.as_str());

        log::debug!("Parsing upgrade definition: {}", name);

        // Find or create upgrade
        let name_key = NameKeyGenerator::name_to_key(&name);
        let mut template = if let Some(existing) = self.upgrades.get(&name_key) {
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
        self.upgrades.insert(name_key, template.clone());

        // Add to list if new
        if !self
            .upgrade_list
            .iter()
            .any(|t| t.get_name_key() == name_key)
        {
            self.upgrade_list.push(template);
        }

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
    THE_UPGRADE_CENTER.clone()
}

pub fn with_upgrade_center<F, R>(f: F) -> R
where
    F: FnOnce(&UpgradeCenter) -> R,
{
    let center = THE_UPGRADE_CENTER
        .read()
        .expect("UpgradeCenter lock poisoned");
    f(&center)
}

pub fn with_upgrade_center_mut<F, R>(f: F) -> R
where
    F: FnOnce(&mut UpgradeCenter) -> R,
{
    let mut center = THE_UPGRADE_CENTER
        .write()
        .expect("UpgradeCenter lock poisoned");
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
