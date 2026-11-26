#[cfg(test)]
use std::sync::Arc;

use crate::common::{ObjectID, UpgradeMaskType};
use crate::upgrade::modules::upgrade_mux::UpgradeMuxRuntime;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleDataHandle, ModuleDataView, NameKeyType};

pub use game_engine::common::system::upgrade::SubObjectsUpgradeModuleData;

/// Mutable execution state belongs to the installed module, not an object-ID registry.
pub struct SubObjectsUpgrade {
    module_name_key: NameKeyType,
    data: ModuleDataView<SubObjectsUpgradeModuleData>,
    applied: bool,
}

fn apply_subobject_visibility(
    object: &mut crate::object::Object,
    data: &SubObjectsUpgradeModuleData,
) {
    let Some(drawable) = object.get_drawable() else {
        return;
    };

    let mut update_sub_objects = false;
    let Ok(mut drawable_guard) = drawable.write() else {
        return;
    };
    for name in data.show_sub_object_names() {
        drawable_guard.show_sub_object(name.as_str(), true);
        update_sub_objects = true;
    }
    for name in data.hide_sub_object_names() {
        drawable_guard.show_sub_object(name.as_str(), false);
        update_sub_objects = true;
    }
    if update_sub_objects {
        drawable_guard.update_sub_objects();
    }
}

impl SubObjectsUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: ModuleDataHandle,
        _object_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data: ModuleDataView::new(data, |data| (data as &dyn std::any::Any).downcast_ref::<SubObjectsUpgradeModuleData>().expect("expected subobjects upgrade data")),
            applied: false,
        }
    }

    /// Eligibility is captured under the module borrow. The caller releases it
    /// before FX/removals, which can synchronously reset this very module.
    pub(crate) fn prepare_upgrade(
        &self,
        mask: UpgradeMaskType,
    ) -> Option<ModuleDataView<SubObjectsUpgradeModuleData>> {
        self.can_upgrade(mask).then(|| self.data.clone())
    }

    /// C++ giveSelfUpgrade commits execution even if implementation skips the
    /// drawable because a live object/player conflict remains after removals.
    pub(crate) fn finish_upgrade(&mut self, object: &mut crate::object::Object) {
        self.refresh_visibility(object);
        self.applied = true;
    }

    pub(crate) fn refresh_for_object(&self, object: &mut crate::object::Object) {
        if self.applied {
            self.refresh_visibility(object);
        }
    }

    fn refresh_visibility(&self, object: &mut crate::object::Object) {
        let data = self.data.read();
        let (_, conflicting) = data.upgrade_mux_data.clone().get_upgrade_activation_masks();
        let conflicting = UpgradeMaskType::from_bits_retain(conflicting.to_bits());
        if object.completed_upgrades().intersects(conflicting)
            || object.with_controlling_player(|player| {
                player.get_completed_upgrade_mask().intersects(conflicting)
            }) == Some(true)
        {
            return;
        }
        apply_subobject_visibility(object, &data);
    }
}

impl Module for SubObjectsUpgrade {
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

impl Snapshotable for SubObjectsUpgrade {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ base CRCs are empty; UpgradeMux CRC writes its version and flag.
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
            "SubObjectsUpgrade",
        )
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl SubObjectsUpgrade {
    pub(crate) fn can_upgrade(&self, upgrade_mask: UpgradeMaskType) -> bool {
        crate::object::upgrade::upgrade_module::mux_can_upgrade(
            &self.data.read().upgrade_mux_data,
            self.applied,
            upgrade_mask,
        )
    }

    pub(crate) fn remove_upgrade(&mut self, upgrade_mask: UpgradeMaskType) {
        // C++ resetUpgrade clears execution; it never undoes visibility.
        crate::object::upgrade::upgrade_module::mux_reset_upgrade(
            &self.data.read().upgrade_mux_data,
            &mut self.applied,
            upgrade_mask,
        );
    }
}

#[cfg(test)]
mod tests;
