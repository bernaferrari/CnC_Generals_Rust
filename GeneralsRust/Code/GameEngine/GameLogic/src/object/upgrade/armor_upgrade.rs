#[cfg(test)]
use std::sync::Arc;

#[cfg(test)]
use crate::common::AsciiString;
use crate::common::{ObjectID, UpgradeMaskType};
use crate::object::Object;
use crate::object::body::body_module::ArmorSetType;
use crate::object::draw::draw_module::TerrainDecalType;
use crate::object::drawable::DrawableArcExt;
use crate::object::upgrade::upgrade_module::{mux_can_upgrade, mux_reset_upgrade};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleDataHandle, ModuleDataView, NameKeyType};

pub use game_engine::common::system::upgrade::ArmorUpgradeModuleData;

/// Installed execution state; authored rules are shared and immutable.
pub struct ArmorUpgrade {
    module_name_key: NameKeyType,
    data: ModuleDataView<ArmorUpgradeModuleData>,
    applied: bool,
}

impl ArmorUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: ModuleDataHandle,
        _object_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data: ModuleDataView::new(data, |data| (data as &dyn std::any::Any).downcast_ref::<ArmorUpgradeModuleData>().expect("expected armor upgrade data")),
            applied: false,
        }
    }

    pub(crate) fn can_upgrade(&self, mask: UpgradeMaskType) -> bool {
        mux_can_upgrade(&self.data.read().upgrade_mux_data, self.applied, mask)
    }

    pub(crate) fn prepare_upgrade(
        &self,
        mask: UpgradeMaskType,
    ) -> Option<ModuleDataView<ArmorUpgradeModuleData>> {
        self.can_upgrade(mask).then(|| self.data.clone())
    }

    pub(crate) fn remove_upgrade(&mut self, mask: UpgradeMaskType) {
        // C++ resetUpgrade clears execution; armor and decal are not undone.
        mux_reset_upgrade(&self.data.read().upgrade_mux_data, &mut self.applied, mask);
    }

    pub(crate) fn finish_upgrade(&mut self) {
        self.applied = true;
    }
}

/// C++ ArmorUpgrade::upgradeImplementation, after FX and removals.
/// Called with the actual owner and no installed upgrade module guard.
pub(crate) fn apply_to_object(data: &ArmorUpgradeModuleData, owner: &mut Object) {
    if let Some(body) = owner.get_body_module() {
        let mut body = body.lock().expect("ArmorUpgrade body poisoned");
        body.set_armor_set_flag(ArmorSetType::PlayerUpgrade)
            .expect("ArmorUpgrade body armor setter failed");
    }
    if data
        .upgrade_mux_data.definition
        .is_triggered_by("Upgrade_AmericaChemicalSuits")
    {
        if let Some(drawable) = owner.get_drawable() {
            drawable.set_terrain_decal_for_object(TerrainDecalType::ChemSuit, owner);
        }
    }
}

impl Module for ArmorUpgrade {
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

impl Snapshotable for ArmorUpgrade {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        crate::object::upgrade::upgrade_module::crc_upgrade_module_state(xfer, self.applied)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        crate::object::upgrade::upgrade_module::xfer_upgrade_module_with_version(
            xfer,
            &mut self.applied,
            "ArmorUpgrade",
        )
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
