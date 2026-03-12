//! StealthUpgrade Module - Complete Port from C++
//!
//! Matches C++ StealthUpgrade.cpp and StealthUpgrade.h exactly
//! Location: GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/StealthUpgrade.cpp
//!
//! Simple upgrade that grants OBJECT_STATUS_CAN_STEALTH status to enable stealth capability.
//! Used for upgrades like "Black Market" that grant stealth to units.

use crate::common::*;
use crate::modules::UpgradeModuleInterface;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::Object;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};
use log::debug;
use std::sync::Arc;

/// Stealth upgrade module data
/// Matches C++ StealthUpgrade (no custom data fields, uses base UpgradeModule)
#[derive(Debug, Clone)]
pub struct StealthUpgradeModuleData {
    module_tag_name_key: NameKeyType,
}

impl Default for StealthUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
        }
    }
}

impl ModuleData for StealthUpgradeModuleData {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.module_tag_name_key = key;
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_tag_name_key
    }
}

impl Snapshotable for StealthUpgradeModuleData {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Stealth upgrade module
/// Matches C++ StealthUpgrade class lines 20-35
pub struct StealthUpgrade {
    module_name_key: NameKeyType,
    data: Arc<StealthUpgradeModuleData>,
    object_id: ObjectID,
    applied: Bool,
}

impl StealthUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<StealthUpgradeModuleData>,
        object_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data,
            object_id,
            applied: false,
        }
    }

    /// Apply the upgrade
    /// Matches C++ upgradeImplementation lines 27-42
    pub fn upgrade_implementation(&mut self) -> Result<(), String> {
        let Some(obj) = OBJECT_REGISTRY.get_object(self.object_id) else {
            return Err("Object not found".to_string());
        };

        let mut guard = obj.write().map_err(|_| "Lock failed")?;

        // The logic that does the stealthupdate will notice this and start stealthing
        // C++ line 30: me->setStatus( MAKE_OBJECT_STATUS_MASK( OBJECT_STATUS_CAN_STEALTH ) );
        guard.set_status(ObjectStatusMaskType::CAN_STEALTH, true);

        // Grant stealth to spawns if applicable (C++ lines 33-41)
        if guard.is_kind_of(KindOf::SpawnsAreTheWeapons) {
            let _ = guard.with_spawn_behavior_full_interface(|spawn_behavior| {
                let _ = spawn_behavior.give_slaves_stealth_upgrade(true);
            });
        }

        self.applied = true;
        debug!("Stealth upgrade applied to object {}", self.object_id);

        Ok(())
    }

    pub fn is_applied(&self) -> bool {
        self.applied
    }
}

impl Module for StealthUpgrade {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }

    fn on_object_created(&mut self) {
        // Upgrade is typically triggered by player upgrade, not on creation
    }
}

impl UpgradeModuleInterface for StealthUpgrade {
    fn can_upgrade(&self, _upgrade_mask: UpgradeMaskType) -> bool {
        true
    }

    fn apply_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) -> bool {
        self.upgrade_implementation().is_ok()
    }

    fn remove_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) {
        // C++ does not remove stealth status once applied; keep parity.
    }
}

impl Snapshotable for StealthUpgrade {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        // Matches C++ crc lines 47-52
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        // Matches C++ xfer lines 60-70
        // Version 1 - no custom data to xfer
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        // Matches C++ loadPostProcess lines 76-82
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stealth_upgrade_creation() {
        let data = Arc::new(StealthUpgradeModuleData::default());
        let upgrade = StealthUpgrade::new(1, data, 100);
        assert!(!upgrade.is_applied());
    }

    #[test]
    fn test_stealth_upgrade_module_data() {
        let data = StealthUpgradeModuleData::default();
        assert_eq!(data.module_tag_name_key, 0);
    }
}
