#[cfg(test)]
use std::sync::Arc;

use crate::common::{ObjectID, UpgradeMaskType, WeaponBonusConditionType};
use crate::object::upgrade::upgrade_module::{mux_can_upgrade, mux_reset_upgrade};
pub use game_engine::common::system::upgrade::WeaponBonusUpgradeModuleData;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleDataHandle, ModuleDataView, NameKeyType};

/// Upgrade module that increases weapon damage on the owning object.
pub struct WeaponBonusUpgrade {
    module_name_key: NameKeyType,
    data: ModuleDataView<WeaponBonusUpgradeModuleData>,
    applied: bool,
}

impl WeaponBonusUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: ModuleDataHandle,
        _object_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data: ModuleDataView::new(data, |data| (data as &dyn std::any::Any).downcast_ref::<WeaponBonusUpgradeModuleData>().expect("expected weapon bonus upgrade data")),
            applied: false,
        }
    }
}

impl Module for WeaponBonusUpgrade {
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
        self.data.handle().read().get_module_tag_name_key()
    }

    fn get_module_data(&self) -> game_engine::common::thing::module::ModuleDataRead<'_> {
        game_engine::common::thing::module::ModuleDataRead::shared(self.data.handle())
    }
}

impl Snapshotable for WeaponBonusUpgrade {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        crate::object::upgrade::upgrade_module::crc_upgrade_module_state(xfer, self.applied)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        crate::object::upgrade::upgrade_module::xfer_upgrade_module_with_version(
            xfer,
            &mut self.applied,
            std::any::type_name::<Self>(),
        )
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl WeaponBonusUpgrade {
    pub(crate) fn can_upgrade(&self, mask: UpgradeMaskType) -> bool {
        mux_can_upgrade(&self.data.read().upgrade_mux_data, self.applied, mask)
    }

    /// Retain immutable rules while releasing the installed module before callbacks.
    pub(crate) fn prepare_upgrade(
        &self,
        mask: UpgradeMaskType,
    ) -> Option<ModuleDataView<WeaponBonusUpgradeModuleData>> {
        self.can_upgrade(mask).then(|| self.data.clone())
    }

    pub(crate) fn remove_upgrade(&mut self, mask: UpgradeMaskType) {
        // C++ resetUpgrade clears execution without undoing the owner bonus flag.
        mux_reset_upgrade(&self.data.read().upgrade_mux_data, &mut self.applied, mask);
    }

    pub(crate) fn finish_upgrade(&mut self) {
        // UpgradeMux commits only after FX, removals, and upgradeImplementation.
        self.applied = true;
    }
}

pub(crate) fn apply_to_object(owner: &mut crate::object::Object) {
    // WeaponBonusUpgrade.cpp:62–69 acts on its exact Object. The setter
    // preserves Object.cpp:4650–4659's change-only weapon notifications.
    owner.set_weapon_bonus_condition(WeaponBonusConditionType::PlayerUpgrade);
}

#[cfg(test)]
#[path = "weapon_bonus_upgrade/tests.rs"]
mod ownership_tests;
