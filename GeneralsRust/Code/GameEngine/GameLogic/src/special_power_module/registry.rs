//! Special Power Registry System
//!
//! Manages all special power instances and provides global access

use super::base_power::*;
use super::cooldown::CooldownManager;
use super::types::*;
use crate::common::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, RwLock};

/// Special power registry - manages all active special powers
pub struct SpecialPowerRegistry {
    /// All registered powers by ID
    powers: HashMap<SpecialPowerID, SharedSpecialPowerModule>,
    /// Powers by player ID
    player_powers: HashMap<ObjectID, Vec<SpecialPowerID>>,
    /// Powers by kind
    powers_by_kind: HashMap<SpecialPowerKind, Vec<SpecialPowerID>>,
    /// Cooldown manager
    cooldown_manager: CooldownManager,
    /// Next power ID
    next_power_id: SpecialPowerID,
}

impl SpecialPowerRegistry {
    pub fn new() -> Self {
        Self {
            powers: HashMap::new(),
            player_powers: HashMap::new(),
            powers_by_kind: HashMap::new(),
            cooldown_manager: CooldownManager::new(),
            next_power_id: 1,
        }
    }

    /// Register a new special power
    pub fn register_power(
        &mut self,
        mut power: Box<dyn SpecialPowerModuleInterface>,
        player_id: Option<ObjectID>,
    ) -> SpecialPowerID {
        let power_id = self.next_power_id;
        self.next_power_id += 1;

        // Set the power ID in the module data
        power.get_data_mut().power_id = power_id;

        // Get power properties before moving
        let power_kind = power.get_data().power_kind;
        let recharge_time = power.get_data().recharge_time;
        let init_charge_time = power.get_data().init_charge_time;
        let shared_group = power.get_data().shared_sync_group.clone();

        // Register in cooldown manager
        self.cooldown_manager.register_power(
            power_id,
            recharge_time,
            init_charge_time,
            shared_group,
        );

        // Store the power
        let shared_power: SharedSpecialPowerModule = Arc::new(Mutex::new(power));
        self.powers.insert(power_id, shared_power);

        // Track by player if specified
        if let Some(pid) = player_id {
            self.player_powers
                .entry(pid)
                .or_insert_with(Vec::new)
                .push(power_id);
        }

        // Track by kind
        self.powers_by_kind
            .entry(power_kind)
            .or_insert_with(Vec::new)
            .push(power_id);

        log::info!(
            "Registered special power ID {} ({:?})",
            power_id,
            power_kind
        );
        power_id
    }

    /// Get a power by ID
    pub fn get_power(&self, power_id: SpecialPowerID) -> Option<SharedSpecialPowerModule> {
        self.powers.get(&power_id).cloned()
    }

    /// Get all powers for a player
    pub fn get_player_powers(&self, player_id: ObjectID) -> Vec<SharedSpecialPowerModule> {
        self.player_powers
            .get(&player_id)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.powers.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get all powers of a specific kind
    pub fn get_powers_by_kind(&self, kind: SpecialPowerKind) -> Vec<SharedSpecialPowerModule> {
        self.powers_by_kind
            .get(&kind)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.powers.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Unregister a power
    pub fn unregister_power(&mut self, power_id: SpecialPowerID) -> bool {
        if let Some(_power) = self.powers.remove(&power_id) {
            // Remove from player powers
            for powers in self.player_powers.values_mut() {
                powers.retain(|&id| id != power_id);
            }

            // Remove from kind powers
            for powers in self.powers_by_kind.values_mut() {
                powers.retain(|&id| id != power_id);
            }

            log::info!("Unregistered special power ID {}", power_id);
            true
        } else {
            false
        }
    }

    /// Update all powers
    pub fn update(&mut self) {
        let delta_time = SECONDS_PER_LOGICFRAME_REAL;

        // Update cooldowns
        self.cooldown_manager.update(delta_time);

        // Update each power
        for power in self.powers.values() {
            if let Ok(mut p) = power.lock() {
                p.update(delta_time);
            }
        }
    }

    /// Reset all powers
    pub fn reset_all(&mut self) {
        self.cooldown_manager.reset_all();

        for power in self.powers.values() {
            if let Ok(mut p) = power.lock() {
                p.reset();
            }
        }
    }

    /// Clear all powers (for cleanup)
    pub fn clear(&mut self) {
        self.powers.clear();
        self.player_powers.clear();
        self.powers_by_kind.clear();
        self.cooldown_manager = CooldownManager::new();
        self.next_power_id = 1;
    }

    /// Get total number of registered powers
    pub fn power_count(&self) -> usize {
        self.powers.len()
    }

    /// Get statistics for all powers
    pub fn get_all_stats(&self) -> HashMap<SpecialPowerID, SpecialPowerStats> {
        let mut stats = HashMap::new();

        for (&id, power) in &self.powers {
            if let Ok(p) = power.lock() {
                stats.insert(id, p.get_stats().clone());
            }
        }

        stats
    }

    /// Get all registered powers.
    pub fn get_all_powers(&self) -> Vec<SharedSpecialPowerModule> {
        self.powers.values().cloned().collect()
    }
}

impl Default for SpecialPowerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Global registry instance
static POWER_REGISTRY: OnceLock<RwLock<SpecialPowerRegistry>> = OnceLock::new();

/// Initialize the global power registry
pub fn initialize_power_registry() {
    POWER_REGISTRY.get_or_init(|| RwLock::new(SpecialPowerRegistry::new()));
    log::info!("Special power registry initialized");
}

/// Get a reference to the global power registry
pub fn get_power_registry() -> Option<&'static RwLock<SpecialPowerRegistry>> {
    POWER_REGISTRY.get()
}

/// Register a power in the global registry
pub fn register_power(
    power: Box<dyn SpecialPowerModuleInterface>,
    player_id: Option<ObjectID>,
) -> Result<SpecialPowerID, String> {
    let registry = get_power_registry().ok_or("Power registry not initialized")?;

    let mut reg = registry
        .write()
        .map_err(|_| "Failed to acquire registry lock")?;

    Ok(reg.register_power(power, player_id))
}

/// Get a power from the global registry
pub fn get_power(power_id: SpecialPowerID) -> Option<SharedSpecialPowerModule> {
    let registry = get_power_registry()?;
    let reg = registry.read().ok()?;
    reg.get_power(power_id)
}

/// Get all powers for a player
pub fn get_player_powers(player_id: ObjectID) -> Vec<SharedSpecialPowerModule> {
    let registry = match get_power_registry() {
        Some(r) => r,
        None => return Vec::new(),
    };

    let reg = match registry.read() {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    reg.get_player_powers(player_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::special_power_module::base_power::{SpecialPowerModule, SpecialPowerModuleData};

    #[test]
    fn test_registry_creation() {
        let registry = SpecialPowerRegistry::new();
        assert_eq!(registry.power_count(), 0);
    }

    #[test]
    fn test_power_registration() {
        let mut registry = SpecialPowerRegistry::new();

        let data = SpecialPowerModuleData::new("TestPower".into(), SpecialPowerKind::OCL);
        let power = Box::new(SpecialPowerModule::new(data));

        let power_id = registry.register_power(power, Some(1));
        assert_eq!(power_id, 1);
        assert_eq!(registry.power_count(), 1);

        // Verify we can retrieve it
        let retrieved = registry.get_power(power_id);
        assert!(retrieved.is_some());
    }

    #[test]
    fn test_player_powers() {
        let mut registry = SpecialPowerRegistry::new();

        // Register multiple powers for player 1
        for i in 0..3 {
            let data =
                SpecialPowerModuleData::new(format!("Power{}", i).into(), SpecialPowerKind::OCL);
            let power = Box::new(SpecialPowerModule::new(data));
            registry.register_power(power, Some(1));
        }

        let player_powers = registry.get_player_powers(1);
        assert_eq!(player_powers.len(), 3);
    }

    #[test]
    fn test_powers_by_kind() {
        let mut registry = SpecialPowerRegistry::new();

        // Register powers of different kinds
        let data1 = SpecialPowerModuleData::new("Power1".into(), SpecialPowerKind::OCL);
        registry.register_power(Box::new(SpecialPowerModule::new(data1)), None);

        let data2 = SpecialPowerModuleData::new("Power2".into(), SpecialPowerKind::OCL);
        registry.register_power(Box::new(SpecialPowerModule::new(data2)), None);

        let data3 = SpecialPowerModuleData::new("Power3".into(), SpecialPowerKind::FireWeapon);
        registry.register_power(Box::new(SpecialPowerModule::new(data3)), None);

        let ocl_powers = registry.get_powers_by_kind(SpecialPowerKind::OCL);
        assert_eq!(ocl_powers.len(), 2);

        let fire_powers = registry.get_powers_by_kind(SpecialPowerKind::FireWeapon);
        assert_eq!(fire_powers.len(), 1);
    }

    #[test]
    fn test_power_unregistration() {
        let mut registry = SpecialPowerRegistry::new();

        let data = SpecialPowerModuleData::new("TestPower".into(), SpecialPowerKind::OCL);
        let power = Box::new(SpecialPowerModule::new(data));
        let power_id = registry.register_power(power, None);

        assert_eq!(registry.power_count(), 1);

        let result = registry.unregister_power(power_id);
        assert!(result);
        assert_eq!(registry.power_count(), 0);
    }
}
