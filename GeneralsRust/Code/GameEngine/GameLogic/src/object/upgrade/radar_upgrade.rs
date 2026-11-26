use std::sync::Arc;

use crate::common::{Bool, ObjectID, UpgradeMaskType};
use crate::object::upgrade::upgrade_module::mux_can_upgrade;
pub use game_engine::common::system::upgrade::RadarUpgradeModuleData;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, NameKeyType};

/// Upgrade module that grants radar to the owning player's UI.
pub struct RadarUpgrade {
    module_name_key: NameKeyType,
    data: game_engine::common::thing::module::ModuleDataView<RadarUpgradeModuleData>,
    applied: bool,
}

impl RadarUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: game_engine::common::thing::module::ModuleDataHandle,
        _object_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data: game_engine::common::thing::module::ModuleDataView::new(data, |data| (data as &dyn std::any::Any).downcast_ref::<RadarUpgradeModuleData>().expect("RadarUpgradeModuleData")),
            applied: false,
        }
    }

    pub fn is_applied(&self) -> bool {
        self.applied
    }

    pub fn is_disable_proof(&self) -> Bool {
        self.data.read().is_disable_proof()
    }

    pub(crate) fn prepare_upgrade(
        &self,
        mask: UpgradeMaskType,
    ) -> Option<game_engine::common::thing::module::ModuleDataView<RadarUpgradeModuleData>> {
        mux_can_upgrade(&self.data.read().upgrade_mux_data, self.applied, mask)
            .then(|| self.data.clone())
    }

    pub(crate) fn finish_upgrade(&mut self) {
        self.applied = true;
    }

    pub(crate) fn enabled_applied_radar_proof(&self, disabled: bool) -> Option<bool> {
        (self.applied && !disabled).then(|| self.data.read().is_disable_proof())
    }

    pub(crate) fn set_applied_owned(&mut self, applied: bool) {
        self.applied = applied;
    }

    // Removing the upgrade does nothing; cleanup happens on delete/capture.
}

impl Module for RadarUpgrade {
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

impl Snapshotable for RadarUpgrade {
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

impl RadarUpgrade {
    pub(crate) fn can_upgrade(&mut self, upgrade_mask: UpgradeMaskType) -> bool {
        mux_can_upgrade(&self.data.read().upgrade_mux_data, self.applied, upgrade_mask)
    }

    pub(crate) fn remove_upgrade(&mut self, upgrade_mask: UpgradeMaskType) {
        // C++ doesn't remove radar; resetUpgrade only clears executed.
        let _ = crate::object::upgrade::upgrade_module::mux_reset_upgrade(
            &self.data.read().upgrade_mux_data,
            &mut self.applied,
            upgrade_mask,
        );
    }

    pub(crate) fn on_delete(&mut self, object: &mut crate::object::Object) {
        if !self.applied || object.is_disabled() {
            return;
        }

        object.with_controlling_player_mut(|player_guard| {
            player_guard.remove_radar(self.data.read().is_disable_proof());
        });

        self.applied = false;
    }

    pub(crate) fn on_capture(
        &mut self,
        object: &mut crate::object::Object,
        old_owner: Option<&Arc<std::sync::RwLock<crate::player::Player>>>,
        new_owner: Option<&Arc<std::sync::RwLock<crate::player::Player>>>,
    ) {
        if !self.applied || object.is_disabled() {
            return;
        }

        if let Some(old_owner) = old_owner {
            if let Ok(mut player_guard) = old_owner.write() {
                player_guard.remove_radar(self.data.read().is_disable_proof());
                self.applied = false;
            }
        }

        if let Some(new_owner) = new_owner {
            if let Ok(mut player_guard) = new_owner.write() {
                player_guard.add_radar(self.data.read().is_disable_proof());
                self.applied = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::common::ini::INI;
    use game_engine::common::thing::module::ModuleData;

    #[test]
    fn disable_proof_uses_ini_bool_parser() {
        let mut data = RadarUpgradeModuleData::default();
        let mut ini = INI::new();

        ini.with_inline_source("DisableProof = yes\nEnd\n", |ini| data.parse_from_ini(ini))
            .expect("bool disable proof");

        assert!(data.is_disable_proof());
    }

    #[test]
    fn radar_upgrade_builds_from_typed_config() {
        let mut data = RadarUpgradeModuleData::default();
        data.set_module_tag_name_key(0xCAFE);
        INI::new()
            .with_inline_source("DisableProof = Yes\nEnd\n", |ini| data.parse_from_ini(ini))
            .unwrap();

        let module = RadarUpgrade::new(0xBEEF, game_engine::common::thing::module::ModuleDataHandle::new(data), 42);

        assert_eq!(module.module_name_key, 0xBEEF);
        assert!(module.is_disable_proof());
        assert_eq!(module.get_module_tag_name_key(), 0xCAFE);
    }
}
