//! Stealth Upgrade System
//!
//! Manages stealth capability upgrades for game objects, including:
//! - Stealth upgrade registration and configuration
//! - Per-unit upgrade tracking
//! - OBJECT_STATUS_CAN_STEALTH capability granting/revocation
//! - Spawned unit stealth inheritance
//! - Tech tree and black market integration
//! - KindOf filtering for upgrade applicability

use crate::common::ObjectID;
use log::{debug, trace, warn};
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::OnceLock;

/// Stealth upgrade configuration
#[derive(Debug, Clone)]
pub struct StealthUpgrade {
    /// Name identifier for the upgrade
    upgrade_name: String,

    /// KindOf mask for applicable unit types (bitmask)
    applies_to_kindof: u32,

    /// Whether this upgrade grants capability to spawned units
    grants_capability_to_spawned: bool,

    /// Whether upgrade requires black market access
    black_market_only: bool,

    /// Tech tree reference that grants this upgrade
    granted_by_tech: String,
}

impl StealthUpgrade {
    /// Create new stealth upgrade configuration
    pub fn new(
        upgrade_name: String,
        applies_to_kindof: u32,
        grants_capability_to_spawned: bool,
        black_market_only: bool,
        granted_by_tech: String,
    ) -> Self {
        Self {
            upgrade_name,
            applies_to_kindof,
            grants_capability_to_spawned,
            black_market_only,
            granted_by_tech,
        }
    }

    /// Get upgrade name
    pub fn name(&self) -> &str {
        &self.upgrade_name
    }

    /// Get KindOf mask
    pub fn kindof_mask(&self) -> u32 {
        self.applies_to_kindof
    }

    /// Check if upgrade grants capability to spawned units
    pub fn grants_spawned(&self) -> bool {
        self.grants_capability_to_spawned
    }

    /// Check if upgrade requires black market
    pub fn requires_black_market(&self) -> bool {
        self.black_market_only
    }

    /// Get tech tree reference
    pub fn tech_reference(&self) -> &str {
        &self.granted_by_tech
    }
}

/// Per-player black market availability state
#[derive(Debug, Clone, Copy)]
struct PlayerBlackMarketState {
    /// Whether player has black market access
    available: bool,
}

/// Stealth Upgrade Manager singleton
///
/// Manages stealth upgrade configuration and application to units.
/// Tracks which units have which upgrades and ensures capability
/// granting follows black market and tech tree requirements.
pub struct StealthUpgradeManager {
    /// Registered stealth upgrade configurations
    upgrades: HashMap<String, StealthUpgrade>,

    /// Per-unit: which upgrades have been applied
    unit_upgrades: HashMap<ObjectID, Vec<String>>,

    /// Per-unit: whether unit has OBJECT_STATUS_CAN_STEALTH capability
    has_capability: HashMap<ObjectID, bool>,

    /// Per-player black market availability
    black_market_state: [PlayerBlackMarketState; crate::common::MAX_PLAYER_COUNT],
}

impl StealthUpgradeManager {
    /// Create new StealthUpgradeManager
    pub fn new() -> Self {
        Self {
            upgrades: HashMap::new(),
            unit_upgrades: HashMap::new(),
            has_capability: HashMap::new(),
            black_market_state: [PlayerBlackMarketState { available: false };
                crate::common::MAX_PLAYER_COUNT],
        }
    }

    /// Register a new stealth upgrade configuration
    pub fn register_upgrade(&mut self, config: StealthUpgrade) -> Result<(), String> {
        let name = config.upgrade_name.clone();
        if self.upgrades.contains_key(&name) {
            return Err(format!("Upgrade '{}' already registered", name));
        }
        trace!("Registered stealth upgrade: {}", name);
        self.upgrades.insert(name, config);
        Ok(())
    }

    /// Check if upgrade is registered
    pub fn upgrade_exists(&self, upgrade_name: &str) -> bool {
        self.upgrades.contains_key(upgrade_name)
    }

    /// Apply upgrade to a unit
    pub fn apply_upgrade_to_unit(
        &mut self,
        unit_id: ObjectID,
        upgrade_name: &str,
        unit_kindof: u32,
    ) -> Result<(), String> {
        let _upgrade = self
            .upgrades
            .get(upgrade_name)
            .ok_or_else(|| format!("Upgrade '{}' not registered", upgrade_name))?;

        // Check if upgrade is applicable to this unit's KindOf
        if !self.is_upgrade_applicable(unit_kindof, upgrade_name)? {
            return Err(format!(
                "Upgrade '{}' not applicable to unit {} with KindOf mask 0x{:x}",
                upgrade_name, unit_id, unit_kindof
            ));
        }

        // Get or create upgrade list for unit
        let upgrades = self.unit_upgrades.entry(unit_id).or_insert_with(Vec::new);

        // Check if upgrade already applied
        if upgrades.contains(&upgrade_name.to_string()) {
            return Err(format!(
                "Upgrade '{}' already applied to unit {}",
                upgrade_name, unit_id
            ));
        }

        upgrades.push(upgrade_name.to_string());
        debug!("Applied upgrade '{}' to unit {}", upgrade_name, unit_id);

        // Grant stealth capability
        self.grant_capability(unit_id)?;

        Ok(())
    }

    /// Apply upgrade to spawned units from a parent unit
    pub fn apply_upgrade_to_spawned(
        &mut self,
        parent_id: ObjectID,
        upgrade_name: &str,
        spawned_ids: Vec<ObjectID>,
    ) -> Result<(), String> {
        let upgrade = self
            .upgrades
            .get(upgrade_name)
            .ok_or_else(|| format!("Upgrade '{}' not registered", upgrade_name))?;

        if !upgrade.grants_capability_to_spawned {
            return Err(format!(
                "Upgrade '{}' does not grant capability to spawned units",
                upgrade_name
            ));
        }

        for spawned_id in &spawned_ids {
            // Add upgrade to spawned unit's list
            let upgrades = self
                .unit_upgrades
                .entry(*spawned_id)
                .or_insert_with(Vec::new);
            if !upgrades.contains(&upgrade_name.to_string()) {
                upgrades.push(upgrade_name.to_string());
            }

            // Grant capability to spawned unit
            self.grant_capability(*spawned_id)?;
        }

        debug!(
            "Applied upgrade '{}' to {} spawned units from parent {}",
            upgrade_name,
            spawned_ids.len(),
            parent_id
        );

        Ok(())
    }

    /// Check if unit has stealth capability (OBJECT_STATUS_CAN_STEALTH)
    pub fn has_stealth_capability(&self, object_id: ObjectID) -> Result<bool, String> {
        Ok(self
            .has_capability
            .get(&object_id)
            .copied()
            .unwrap_or(false))
    }

    /// Grant stealth capability to unit
    pub fn grant_capability(&mut self, object_id: ObjectID) -> Result<(), String> {
        if self
            .has_capability
            .get(&object_id)
            .copied()
            .unwrap_or(false)
        {
            return Ok(());
        }
        self.has_capability.insert(object_id, true);
        trace!("Granted stealth capability to unit {}", object_id);
        Ok(())
    }

    /// Revoke stealth capability from unit
    pub fn revoke_capability(&mut self, object_id: ObjectID) -> Result<(), String> {
        if !self.has_capability.contains_key(&object_id) {
            return Err(format!(
                "Unit {} does not have stealth capability",
                object_id
            ));
        }
        self.has_capability.insert(object_id, false);
        debug!("Revoked stealth capability from unit {}", object_id);
        Ok(())
    }

    /// Check if upgrade is applicable to unit with given KindOf
    pub fn is_upgrade_applicable(
        &self,
        unit_kindof: u32,
        upgrade_name: &str,
    ) -> Result<bool, String> {
        let upgrade = self
            .upgrades
            .get(upgrade_name)
            .ok_or_else(|| format!("Upgrade '{}' not registered", upgrade_name))?;

        // Check if unit's KindOf matches upgrade's mask
        Ok((unit_kindof & upgrade.kindof_mask()) != 0)
    }

    /// Get list of upgrades applied to unit
    pub fn get_unit_upgrades(&self, object_id: ObjectID) -> Result<Vec<String>, String> {
        Ok(self
            .unit_upgrades
            .get(&object_id)
            .cloned()
            .unwrap_or_default())
    }

    /// Mark upgrade as requiring black market access
    pub fn require_black_market(&mut self, upgrade_name: &str) -> Result<(), String> {
        let upgrade = self
            .upgrades
            .get_mut(upgrade_name)
            .ok_or_else(|| format!("Upgrade '{}' not registered", upgrade_name))?;

        // Modify the upgrade to require black market
        // Since StealthUpgrade is immutable by design, we need to update it
        let updated = upgrade.clone();
        // Create new upgrade with black market requirement
        let new_upgrade = StealthUpgrade {
            black_market_only: true,
            ..updated
        };
        self.upgrades.insert(upgrade_name.to_string(), new_upgrade);
        debug!("Marked upgrade '{}' as black market only", upgrade_name);
        Ok(())
    }

    /// Check if player has black market access
    pub fn check_black_market_available(&self, player_id: usize) -> Result<bool, String> {
        if player_id >= crate::common::MAX_PLAYER_COUNT {
            return Err(format!("Invalid player_id: {}", player_id));
        }
        Ok(self.black_market_state[player_id].available)
    }

    /// Set black market availability for player
    pub fn set_black_market_available(
        &mut self,
        player_id: usize,
        available: bool,
    ) -> Result<(), String> {
        if player_id >= crate::common::MAX_PLAYER_COUNT {
            return Err(format!("Invalid player_id: {}", player_id));
        }
        self.black_market_state[player_id].available = available;
        if available {
            debug!("Black market enabled for player {}", player_id);
        } else {
            debug!("Black market disabled for player {}", player_id);
        }
        Ok(())
    }

    /// Clear all upgrades for unit (unregistration)
    pub fn clear_unit_upgrades(&mut self, object_id: ObjectID) -> Result<(), String> {
        if self.unit_upgrades.remove(&object_id).is_some() {
            self.has_capability.remove(&object_id);
            trace!("Cleared all upgrades for unit {}", object_id);
            Ok(())
        } else {
            Err(format!("Unit {} has no registered upgrades", object_id))
        }
    }

    /// Get upgrade configuration
    pub fn get_upgrade_config(&self, upgrade_name: &str) -> Result<StealthUpgrade, String> {
        self.upgrades
            .get(upgrade_name)
            .cloned()
            .ok_or_else(|| format!("Upgrade '{}' not found", upgrade_name))
    }

    /// Get all registered upgrades
    pub fn get_all_upgrades(&self) -> Vec<String> {
        self.upgrades.keys().cloned().collect()
    }

    /// Unregister unit completely
    pub fn unregister_unit(&mut self, object_id: ObjectID) -> Result<(), String> {
        self.unit_upgrades.remove(&object_id);
        self.has_capability.remove(&object_id);
        trace!(
            "Unregistered unit {} from stealth upgrade system",
            object_id
        );
        Ok(())
    }
}

impl Default for StealthUpgradeManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global singleton accessor for StealthUpgradeManager
static STEALTH_UPGRADE_MANAGER: OnceLock<Mutex<StealthUpgradeManager>> = OnceLock::new();

/// Get the global StealthUpgradeManager singleton
pub fn get_stealth_upgrade_manager() -> &'static Mutex<StealthUpgradeManager> {
    STEALTH_UPGRADE_MANAGER.get_or_init(|| Mutex::new(StealthUpgradeManager::new()))
}

#[cfg(test)]
mod stealth_upgrade_tests {
    use super::*;

    #[test]
    fn test_stealth_upgrade_basic() {
        let mut manager = StealthUpgradeManager::new();

        let upgrade = StealthUpgrade::new(
            "GLA_Stealth".to_string(),
            0x0001,
            false,
            false,
            "GLA_Stealth_Tech".to_string(),
        );

        assert!(manager.register_upgrade(upgrade).is_ok());
        assert!(manager.upgrade_exists("GLA_Stealth"));
        assert!(!manager.upgrade_exists("NonExistent"));
    }

    #[test]
    fn test_register_duplicate_upgrade() {
        let mut manager = StealthUpgradeManager::new();

        let upgrade = StealthUpgrade::new(
            "Stealth_V1".to_string(),
            0x0001,
            false,
            false,
            "Tech_Stealth".to_string(),
        );

        assert!(manager.register_upgrade(upgrade.clone()).is_ok());
        assert!(manager.register_upgrade(upgrade).is_err());
    }

    #[test]
    fn test_apply_upgrade_to_unit() {
        let mut manager = StealthUpgradeManager::new();

        let upgrade = StealthUpgrade::new(
            "Unit_Stealth".to_string(),
            0x0001, // applies to KindOf mask 0x0001
            false,
            false,
            "Tech_Unit".to_string(),
        );

        manager.register_upgrade(upgrade).unwrap();
        assert!(manager
            .apply_upgrade_to_unit(1, "Unit_Stealth", 0x0001)
            .is_ok());

        let upgrades = manager.get_unit_upgrades(1).unwrap();
        assert_eq!(upgrades.len(), 1);
        assert_eq!(upgrades[0], "Unit_Stealth");
    }

    #[test]
    fn test_apply_upgrade_to_spawned() {
        let mut manager = StealthUpgradeManager::new();

        let upgrade = StealthUpgrade::new(
            "Spawned_Stealth".to_string(),
            0x0002,
            true, // grants to spawned
            false,
            "Tech_Spawned".to_string(),
        );

        manager.register_upgrade(upgrade).unwrap();
        manager
            .apply_upgrade_to_spawned(1, "Spawned_Stealth", vec![2, 3, 4])
            .unwrap();

        assert_eq!(manager.get_unit_upgrades(2).unwrap().len(), 1);
        assert_eq!(manager.get_unit_upgrades(3).unwrap().len(), 1);
        assert_eq!(manager.get_unit_upgrades(4).unwrap().len(), 1);
    }

    #[test]
    fn test_stealth_capability_granting() {
        let mut manager = StealthUpgradeManager::new();

        let upgrade = StealthUpgrade::new(
            "Capability_Test".to_string(),
            0x0001,
            false,
            false,
            "Tech_Cap".to_string(),
        );

        manager.register_upgrade(upgrade).unwrap();
        manager
            .apply_upgrade_to_unit(5, "Capability_Test", 0x0001)
            .unwrap();

        assert!(manager.has_stealth_capability(5).unwrap());
        assert!(!manager.has_stealth_capability(6).unwrap());
    }

    #[test]
    fn test_stealth_capability_revocation() {
        let mut manager = StealthUpgradeManager::new();

        let upgrade = StealthUpgrade::new(
            "Revoke_Test".to_string(),
            0x0001,
            false,
            false,
            "Tech_Revoke".to_string(),
        );

        manager.register_upgrade(upgrade).unwrap();
        manager
            .apply_upgrade_to_unit(7, "Revoke_Test", 0x0001)
            .unwrap();

        assert!(manager.has_stealth_capability(7).unwrap());
        assert!(manager.revoke_capability(7).is_ok());
        assert!(!manager.has_stealth_capability(7).unwrap());
    }

    #[test]
    fn test_upgrade_applicability() {
        let mut manager = StealthUpgradeManager::new();

        let upgrade = StealthUpgrade::new(
            "Kindof_Test".to_string(),
            0x00FF, // applies to masks 0x00-0xFF
            false,
            false,
            "Tech_Kindof".to_string(),
        );

        manager.register_upgrade(upgrade).unwrap();

        // Should be applicable
        assert!(manager
            .is_upgrade_applicable(0x0001, "Kindof_Test")
            .unwrap());
        assert!(manager
            .is_upgrade_applicable(0x0080, "Kindof_Test")
            .unwrap());

        // Should not be applicable
        assert!(!manager
            .is_upgrade_applicable(0x0100, "Kindof_Test")
            .unwrap());
        assert!(!manager
            .is_upgrade_applicable(0x1000, "Kindof_Test")
            .unwrap());
    }

    #[test]
    fn test_black_market_requirement() {
        let mut manager = StealthUpgradeManager::new();

        let upgrade = StealthUpgrade::new(
            "Black_Market".to_string(),
            0x0001,
            false,
            true,
            "Tech_Black".to_string(),
        );

        manager.register_upgrade(upgrade).unwrap();
        assert!(!manager.check_black_market_available(0).unwrap());

        manager.set_black_market_available(0, true).unwrap();
        assert!(manager.check_black_market_available(0).unwrap());

        manager.set_black_market_available(0, false).unwrap();
        assert!(!manager.check_black_market_available(0).unwrap());
    }

    #[test]
    fn test_kindof_filtering() {
        let mut manager = StealthUpgradeManager::new();

        let upgrade = StealthUpgrade::new(
            "Tank_Stealth".to_string(),
            0x0010, // only applies to KindOf 0x0010
            false,
            false,
            "Tech_Tank".to_string(),
        );

        manager.register_upgrade(upgrade).unwrap();

        // Should work with matching KindOf
        assert!(manager
            .apply_upgrade_to_unit(10, "Tank_Stealth", 0x0010)
            .is_ok());

        // Should fail with non-matching KindOf
        assert!(manager
            .apply_upgrade_to_unit(11, "Tank_Stealth", 0x0020)
            .is_err());
    }

    #[test]
    fn test_multiple_upgrades() {
        let mut manager = StealthUpgradeManager::new();

        let upgrade1 = StealthUpgrade::new(
            "Upgrade_1".to_string(),
            0x0001,
            false,
            false,
            "Tech_1".to_string(),
        );

        let upgrade2 = StealthUpgrade::new(
            "Upgrade_2".to_string(),
            0x0001,
            false,
            false,
            "Tech_2".to_string(),
        );

        manager.register_upgrade(upgrade1).unwrap();
        manager.register_upgrade(upgrade2).unwrap();

        manager
            .apply_upgrade_to_unit(15, "Upgrade_1", 0x0001)
            .unwrap();
        manager
            .apply_upgrade_to_unit(15, "Upgrade_2", 0x0001)
            .unwrap();

        let upgrades = manager.get_unit_upgrades(15).unwrap();
        assert_eq!(upgrades.len(), 2);
        assert!(upgrades.contains(&"Upgrade_1".to_string()));
        assert!(upgrades.contains(&"Upgrade_2".to_string()));
    }

    #[test]
    fn test_spawned_unit_inheritance() {
        let mut manager = StealthUpgradeManager::new();

        let upgrade = StealthUpgrade::new(
            "Spawn_Inherit".to_string(),
            0x0003,
            true,
            false,
            "Tech_Spawn".to_string(),
        );

        manager.register_upgrade(upgrade).unwrap();

        // Parent gets upgrade
        manager
            .apply_upgrade_to_unit(20, "Spawn_Inherit", 0x0001)
            .unwrap();

        // Spawned get upgrade
        let spawned = vec![21, 22, 23];
        manager
            .apply_upgrade_to_spawned(20, "Spawn_Inherit", spawned.clone())
            .unwrap();

        // All should have capability
        assert!(manager.has_stealth_capability(20).unwrap());
        for spawned_id in spawned {
            assert!(manager.has_stealth_capability(spawned_id).unwrap());
        }
    }

    #[test]
    fn test_upgrade_config_retrieval() {
        let mut manager = StealthUpgradeManager::new();

        let upgrade = StealthUpgrade::new(
            "Config_Test".to_string(),
            0x00AA,
            true,
            true,
            "Tech_Config".to_string(),
        );

        manager.register_upgrade(upgrade).unwrap();

        let config = manager.get_upgrade_config("Config_Test").unwrap();
        assert_eq!(config.name(), "Config_Test");
        assert_eq!(config.kindof_mask(), 0x00AA);
        assert!(config.grants_spawned());
        assert!(config.requires_black_market());
        assert_eq!(config.tech_reference(), "Tech_Config");
    }

    #[test]
    fn test_clear_unit_upgrades() {
        let mut manager = StealthUpgradeManager::new();

        let upgrade = StealthUpgrade::new(
            "Clear_Test".to_string(),
            0x0001,
            false,
            false,
            "Tech_Clear".to_string(),
        );

        manager.register_upgrade(upgrade).unwrap();
        manager
            .apply_upgrade_to_unit(25, "Clear_Test", 0x0001)
            .unwrap();

        assert!(manager.get_unit_upgrades(25).unwrap().len() > 0);
        assert!(manager.clear_unit_upgrades(25).is_ok());
        assert_eq!(manager.get_unit_upgrades(25).unwrap().len(), 0);
        assert!(!manager.has_stealth_capability(25).unwrap());
    }

    #[test]
    fn test_get_all_upgrades() {
        let mut manager = StealthUpgradeManager::new();

        let upgrade1 = StealthUpgrade::new(
            "All_Upgrades_1".to_string(),
            0x0001,
            false,
            false,
            "Tech".to_string(),
        );

        let upgrade2 = StealthUpgrade::new(
            "All_Upgrades_2".to_string(),
            0x0002,
            false,
            false,
            "Tech".to_string(),
        );

        manager.register_upgrade(upgrade1).unwrap();
        manager.register_upgrade(upgrade2).unwrap();

        let all = manager.get_all_upgrades();
        assert_eq!(all.len(), 2);
        assert!(all.contains(&"All_Upgrades_1".to_string()));
        assert!(all.contains(&"All_Upgrades_2".to_string()));
    }

    #[test]
    fn test_invalid_player_id() {
        let mut manager = StealthUpgradeManager::new();

        assert!(manager.check_black_market_available(8).is_err());
        assert!(manager.set_black_market_available(255, true).is_err());
    }

    #[test]
    fn test_spawned_without_capability() {
        let mut manager = StealthUpgradeManager::new();

        let upgrade = StealthUpgrade::new(
            "No_Spawn".to_string(),
            0x0001,
            false, // does NOT grant to spawned
            false,
            "Tech_No".to_string(),
        );

        manager.register_upgrade(upgrade).unwrap();

        assert!(manager
            .apply_upgrade_to_spawned(30, "No_Spawn", vec![31])
            .is_err());
    }

    #[test]
    fn test_duplicate_upgrade_on_unit() {
        let mut manager = StealthUpgradeManager::new();

        let upgrade = StealthUpgrade::new(
            "Dup_Test".to_string(),
            0x0001,
            false,
            false,
            "Tech_Dup".to_string(),
        );

        manager.register_upgrade(upgrade).unwrap();
        manager
            .apply_upgrade_to_unit(35, "Dup_Test", 0x0001)
            .unwrap();

        assert!(manager
            .apply_upgrade_to_unit(35, "Dup_Test", 0x0001)
            .is_err());
    }
}
