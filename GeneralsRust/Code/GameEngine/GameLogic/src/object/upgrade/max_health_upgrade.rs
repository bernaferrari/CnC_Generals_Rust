#[cfg(test)]
use std::sync::Arc;

#[cfg(test)]
use crate::common::AsciiString;
use crate::common::{ObjectID, UpgradeMaskType};
use crate::object::Object;
use crate::object::upgrade::upgrade_module::{mux_can_upgrade, mux_reset_upgrade};
#[cfg(test)]
use game_engine::common::system::upgrade::MaxHealthChangeType;
pub use game_engine::common::system::upgrade::MaxHealthUpgradeModuleData;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleDataHandle, ModuleDataView, NameKeyType};

/// Installed mutable state belongs only to this module; definitions remain immutable.
pub struct MaxHealthUpgrade {
    module_name_key: NameKeyType,
    data: ModuleDataView<MaxHealthUpgradeModuleData>,
    applied: bool,
}

/// C++ MaxHealthUpgrade.cpp:56-68. The caller already borrows the actual owner.
pub(crate) fn apply_to_object(
    data: &MaxHealthUpgradeModuleData,
    owner: &mut Object,
    loading_map: Option<bool>,
) {
    owner
        .add_body_max_health_with_owner(data.add_max_health(), data.change_type(), loading_map)
        .expect("max health upgrade could not update body");
}

impl MaxHealthUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: ModuleDataHandle,
        _object_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data: ModuleDataView::new(data, |data| (data as &dyn std::any::Any).downcast_ref::<MaxHealthUpgradeModuleData>().expect("expected max health upgrade data")),
            applied: false,
        }
    }
    pub(crate) fn can_upgrade(&self, mask: UpgradeMaskType) -> bool {
        mux_can_upgrade(&self.data.read().upgrade_mux_data, self.applied, mask)
    }
    pub(crate) fn prepare_upgrade(
        &self,
        mask: UpgradeMaskType,
    ) -> Option<ModuleDataView<MaxHealthUpgradeModuleData>> {
        self.can_upgrade(mask).then(|| self.data.clone())
    }
    pub(crate) fn remove_upgrade(&mut self, mask: UpgradeMaskType) {
        // C++ resetUpgrade only resets execution; the health addition is not undone.
        mux_reset_upgrade(&self.data.read().upgrade_mux_data, &mut self.applied, mask);
    }
    pub(crate) fn finish_upgrade(&mut self) {
        // C++ commits execution even when the object has no BodyModule.
        self.applied = true;
    }
}

impl Module for MaxHealthUpgrade {
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

impl Snapshotable for MaxHealthUpgrade {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        crate::object::upgrade::upgrade_module::crc_upgrade_module_state(xfer, self.applied)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        crate::object::upgrade::upgrade_module::xfer_upgrade_module_with_version(
            xfer,
            &mut self.applied,
            "MaxHealthUpgrade",
        )
    }
    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod installed_tests;
