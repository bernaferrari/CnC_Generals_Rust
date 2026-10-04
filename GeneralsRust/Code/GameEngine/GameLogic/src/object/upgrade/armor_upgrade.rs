use std::sync::Arc;

use crate::common::{AsciiString, LegacyModuleData, ObjectID, UpgradeMaskType};
use crate::object::Object;
use crate::object::body::body_module::ArmorSetType;
use crate::object::draw::draw_module::TerrainDecalType;
use crate::object::drawable::DrawableArcExt;
use crate::object::upgrade::upgrade_module::{UpgradeMuxData, mux_can_upgrade, mux_reset_upgrade};
use game_engine::common::ini::{INI, INIError};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

/// Module data describing the armor upgrade to apply.
#[derive(Debug, Clone)]
pub struct ArmorUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    pub upgrade_mux_data: UpgradeMuxData,
}

impl Default for ArmorUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            upgrade_mux_data: UpgradeMuxData::default(),
        }
    }
}

impl ArmorUpgradeModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        self.upgrade_mux_data.parse_from_ini(ini)
    }
}

crate::impl_legacy_module_data_with_key_field!(ArmorUpgradeModuleData, module_tag_name_key);

impl Snapshotable for ArmorUpgradeModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Installed execution state; authored rules are shared and immutable.
pub struct ArmorUpgrade {
    module_name_key: NameKeyType,
    data: Arc<ArmorUpgradeModuleData>,
    applied: bool,
}

impl ArmorUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<ArmorUpgradeModuleData>,
        _object_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data,
            applied: false,
        }
    }

    pub(crate) fn can_upgrade(&self, mask: UpgradeMaskType) -> bool {
        mux_can_upgrade(&self.data.upgrade_mux_data, self.applied, mask)
    }

    pub(crate) fn prepare_upgrade(
        &self,
        mask: UpgradeMaskType,
    ) -> Option<Arc<ArmorUpgradeModuleData>> {
        self.can_upgrade(mask).then(|| Arc::clone(&self.data))
    }

    pub(crate) fn remove_upgrade(&mut self, mask: UpgradeMaskType) {
        // C++ resetUpgrade clears execution; armor and decal are not undone.
        mux_reset_upgrade(&self.data.upgrade_mux_data, &mut self.applied, mask);
    }

    pub(crate) fn finish_upgrade(&mut self) {
        self.applied = true;
    }
}

impl ArmorUpgradeModuleData {
    /// C++ ArmorUpgrade::upgradeImplementation, after FX and removals.
    /// Called with the actual owner and no installed upgrade module guard.
    pub(crate) fn apply_to_object(&self, owner: &mut Object) {
        if let Some(body) = owner.get_body_module() {
            let mut body = body.lock().expect("ArmorUpgrade body poisoned");
            body.set_armor_set_flag(ArmorSetType::PlayerUpgrade)
                .expect("ArmorUpgrade body armor setter failed");
        }
        if self
            .upgrade_mux_data
            .is_triggered_by("Upgrade_AmericaChemicalSuits")
        {
            if let Some(drawable) = owner.get_drawable() {
                drawable.set_terrain_decal_for_object(TerrainDecalType::ChemSuit, owner);
            }
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
        LegacyModuleData::get_module_tag_name_key(self.data.as_ref())
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
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
