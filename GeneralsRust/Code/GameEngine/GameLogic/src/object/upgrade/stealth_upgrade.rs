//! Installed StealthUpgrade behavior from StealthUpgrade.cpp.

use crate::common::{KindOf, ObjectID, ObjectStatusMaskType, UpgradeMaskType};
use crate::modules::UpgradeModuleInterface;
use crate::object::registry::OBJECT_REGISTRY;
pub use game_engine::common::system::upgrade::StealthUpgradeModuleData;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, NameKeyType};
use log::debug;

/// Wave 448: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    crate::object::registry::OBJECT_REGISTRY.is_empty()
}

/// Stealth upgrade module
/// Matches C++ StealthUpgrade class lines 20-35
pub struct StealthUpgrade {
    module_name_key: NameKeyType,
    data: game_engine::common::thing::module::ModuleDataHandle,
    object_id: ObjectID,
    applied: bool,
}

impl StealthUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: game_engine::common::thing::module::ModuleDataHandle,
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
        // C++ UpgradeMux::wouldUpgrade refuses if m_upgradeExecuted.
        if self.applied {
            return Ok(());
        }

        // Wave 448: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let Some(()) = OBJECT_REGISTRY.with_object_mut(self.object_id, |guard| {
            // The logic that does the stealthupdate will notice this and start stealthing
            // C++ line 30: me->setStatus( MAKE_OBJECT_STATUS_MASK( OBJECT_STATUS_CAN_STEALTH ) );
            guard.set_status(ObjectStatusMaskType::CAN_STEALTH, true);

            // Grant stealth to spawns if applicable (C++ lines 33-41)
            if guard.is_kind_of(KindOf::SpawnsAreTheWeapons) {
                let _ = guard.with_spawn_behavior_full_interface(|spawn_behavior| {
                    let _ = spawn_behavior.give_slaves_stealth_upgrade(true);
                });
            }
        }) else {
            return Err("Object not found".to_string());
        };

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
        self.data.read().get_module_tag_name_key()
    }

    fn get_module_data(&self) -> game_engine::common::thing::module::ModuleDataRead<'_> {
        game_engine::common::thing::module::ModuleDataRead::shared(&self.data)
    }

    fn on_object_created(&mut self) {
        // Upgrade is typically triggered by player upgrade, not on creation
    }
}

impl UpgradeModuleInterface for StealthUpgrade {
    fn can_upgrade(&mut self, upgrade_mask: UpgradeMaskType) -> bool {
        let data = self.data.read();
        let data = data.as_any().downcast_ref::<StealthUpgradeModuleData>().expect("StealthUpgradeModuleData");
        crate::object::upgrade::upgrade_module::mux_can_upgrade(&data.upgrade_mux_data, self.applied, upgrade_mask)
    }

    fn apply_upgrade(&mut self, upgrade_mask: UpgradeMaskType) -> bool {
        if !self.can_upgrade(upgrade_mask) {
            return false;
        }
        self.upgrade_implementation().is_ok()
    }

    fn remove_upgrade(&mut self, upgrade_mask: UpgradeMaskType) {
        // C++ resetUpgrade: clear executed so RemovesUpgrades can re-arm.
        // Does not remove stealth status once applied.
        let data = self.data.read();
        let data = data.as_any().downcast_ref::<StealthUpgradeModuleData>().expect("StealthUpgradeModuleData");
        crate::object::upgrade::upgrade_module::mux_reset_upgrade(&data.upgrade_mux_data, &mut self.applied, upgrade_mask);
    }
}

impl Snapshotable for StealthUpgrade {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        crate::object::upgrade::upgrade_module::crc_upgrade_module_state(xfer, self.applied)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: u8 = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;
        crate::object::upgrade::upgrade_module::xfer_upgrade_module_state(xfer, &mut self.applied)?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::AsciiString;
    use game_engine::common::thing::module::ModuleData;

    #[test]
    fn test_stealth_upgrade_creation() {
        let data = game_engine::common::thing::module::ModuleDataHandle::new(StealthUpgradeModuleData::default());
        let upgrade = StealthUpgrade::new(1, data, 100);
        assert!(!upgrade.is_applied());
    }

    #[test]
    fn test_stealth_upgrade_module_data() {
        let data = StealthUpgradeModuleData::default();
        assert_eq!(data.get_module_tag_name_key(), 0);
    }

    #[test]
    fn remove_upgrade_resets_executed_so_module_can_rearm() {
        let mut data = StealthUpgradeModuleData::default();
        data.upgrade_mux_data.definition
            .activation_upgrade_names
            .push(AsciiString::from("Upgrade_GLACamouflage"));
        let mut upgrade = StealthUpgrade::new(1, game_engine::common::thing::module::ModuleDataHandle::new(data), 100);
        upgrade.applied = true;
        let mask = crate::common::UpgradeMaskType::from_bits_retain(
            crate::upgrade::test_upgrade_mask("Upgrade_GLACamouflage").to_bits(),
        );
        upgrade.remove_upgrade(mask);
        assert!(!upgrade.is_applied());
        assert!(upgrade.can_upgrade(mask));
    }
}
