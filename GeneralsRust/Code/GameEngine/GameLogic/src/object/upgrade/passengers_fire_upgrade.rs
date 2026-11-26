#[cfg(test)]
use std::sync::Arc;

use crate::common::{ObjectID, UpgradeMaskType};
use crate::object::Object;
use crate::object::upgrade::upgrade_module::{mux_can_upgrade, mux_reset_upgrade};
pub use game_engine::common::system::upgrade::PassengersFireUpgradeModuleData;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleDataHandle, ModuleDataView, NameKeyType};

/// Installed upgrade state. Definitions are immutable; execution belongs to this module.
pub struct PassengersFireUpgrade {
    module_name_key: NameKeyType,
    data: ModuleDataView<PassengersFireUpgradeModuleData>,
    applied: bool,
}

impl PassengersFireUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: ModuleDataHandle,
        _object_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data: ModuleDataView::new(data, |data| (data as &dyn std::any::Any).downcast_ref::<PassengersFireUpgradeModuleData>().expect("expected passengers fire upgrade data")),
            applied: false,
        }
    }

    pub(crate) fn can_upgrade(&self, mask: UpgradeMaskType) -> bool {
        mux_can_upgrade(&self.data.read().upgrade_mux_data, self.applied, mask)
    }

    /// Release the installed module before synchronous FX, removals, and owner callbacks.
    pub(crate) fn prepare_upgrade(
        &self,
        mask: UpgradeMaskType,
    ) -> Option<ModuleDataView<PassengersFireUpgradeModuleData>> {
        self.can_upgrade(mask).then(|| self.data.clone())
    }

    pub(crate) fn remove_upgrade(&mut self, mask: UpgradeMaskType) {
        // C++ resetUpgrade only clears execution; it does not undo the effect.
        mux_reset_upgrade(&self.data.read().upgrade_mux_data, &mut self.applied, mask);
    }

    pub(crate) fn finish_upgrade(&mut self, owner: &mut Object) {
        // C++ commits execution even when this object has no containment module.
        if let Some(contain) = owner.get_contain() {
            contain
                .lock()
                .expect("passengers fire contain module poisoned")
                .set_passenger_allowed_to_fire(true);
        }
        self.applied = true;
    }
}

impl Module for PassengersFireUpgrade {
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

impl Snapshotable for PassengersFireUpgrade {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ base CRCs are empty; UpgradeMux writes its version and executed flag.
        let mut version = 1u8;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        let mut executed = self.applied;
        xfer.xfer_bool(&mut executed).map_err(|e| e.to_string())
    }
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        crate::object::upgrade::upgrade_module::xfer_upgrade_module_with_version(
            xfer,
            &mut self.applied,
            "PassengersFireUpgrade",
        )
    }
    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
#[path = "passengers_fire_upgrade/tests.rs"]
mod installed_tests;
