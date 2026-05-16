use std::sync::Arc;

use crate::common::{KindOfMaskType, LegacyModuleData, ObjectID, Real, UpgradeMaskType};
use crate::modules::UpgradeModuleInterface;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

/// Module data describing the cost modifier upgrade.
#[derive(Debug, Clone)]
pub struct CostModifierUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    kind_of: KindOfMaskType,
    percentage: Real,
}

impl Default for CostModifierUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            kind_of: crate::common::KIND_OF_MASK_NONE,
            percentage: 0.0,
        }
    }
}

impl CostModifierUpgradeModuleData {
    pub fn kind_of(&self) -> KindOfMaskType {
        self.kind_of
    }

    pub fn percentage(&self) -> Real {
        self.percentage
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, COST_MODIFIER_UPGRADE_FIELDS)
    }
}

crate::impl_legacy_module_data_with_key_field!(CostModifierUpgradeModuleData, module_tag_name_key);

impl Snapshotable for CostModifierUpgradeModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Upgrade module that modifies production costs for buildings/factories.
pub struct CostModifierUpgrade {
    module_name_key: NameKeyType,
    data: Arc<CostModifierUpgradeModuleData>,
    object_id: ObjectID,
    applied: bool,
}

impl CostModifierUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<CostModifierUpgradeModuleData>,
        object_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data,
            object_id,
            applied: false,
        }
    }
}

impl Module for CostModifierUpgrade {
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

impl Snapshotable for CostModifierUpgrade {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1).map_err(|e| e.to_string())?;
        Ok(())
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

impl UpgradeModuleInterface for CostModifierUpgrade {
    fn can_upgrade(&self, _upgrade_mask: UpgradeMaskType) -> bool {
        !self.applied
    }

    fn apply_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) -> bool {
        if self.applied {
            return false;
        }
        use crate::object::registry::OBJECT_REGISTRY;

        let Some(object) = OBJECT_REGISTRY.get_object(self.object_id) else {
            log::warn!("CostModifierUpgrade: Object {} not found", self.object_id);
            return false;
        };

        let object_guard = match object.read() {
            Ok(guard) => guard,
            Err(_) => {
                log::error!(
                    "CostModifierUpgrade: Failed to lock object {}",
                    self.object_id
                );
                return false;
            }
        };

        if let Some(player) = object_guard.get_controlling_player() {
            if let Ok(mut player_guard) = player.write() {
                player_guard.add_kind_of_production_cost_change(
                    self.data.kind_of(),
                    self.data.percentage(),
                );
            }
        }

        self.applied = true;
        true
    }

    fn remove_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) {
        // C++ does not remove the cost change on upgrade removal; only on delete/capture.
    }

    fn on_delete(&mut self, object: &mut crate::object::Object) {
        if !self.applied {
            return;
        }

        if let Some(player) = object.get_controlling_player() {
            if let Ok(mut player_guard) = player.write() {
                player_guard.remove_kind_of_production_cost_change(
                    self.data.kind_of(),
                    self.data.percentage(),
                );
            }
        }

        self.applied = false;
    }

    fn on_capture(
        &mut self,
        _object: &mut crate::object::Object,
        old_owner: Option<&Arc<std::sync::RwLock<crate::player::Player>>>,
        new_owner: Option<&Arc<std::sync::RwLock<crate::player::Player>>>,
    ) {
        if !self.applied {
            return;
        }

        if let Some(old_owner) = old_owner {
            if let Ok(mut player_guard) = old_owner.write() {
                player_guard.remove_kind_of_production_cost_change(
                    self.data.kind_of(),
                    self.data.percentage(),
                );
                self.applied = false;
            }
        }

        if let Some(new_owner) = new_owner {
            if let Ok(mut player_guard) = new_owner.write() {
                player_guard.add_kind_of_production_cost_change(
                    self.data.kind_of(),
                    self.data.percentage(),
                );
                self.applied = true;
            }
        }
    }
}

fn parse_effect_kind_of_field(
    _ini: &mut INI,
    data: &mut CostModifierUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.kind_of = crate::object::behavior::auto_heal_behavior::parse_kind_of_mask(tokens);
    Ok(())
}

fn parse_percentage_field(
    _ini: &mut INI,
    data: &mut CostModifierUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.percentage = INI::parse_percent_to_real(tokens[0])?;
    Ok(())
}

const COST_MODIFIER_UPGRADE_FIELDS: &[FieldParse<CostModifierUpgradeModuleData>] = &[
    FieldParse {
        token: "EffectKindOf",
        parse: parse_effect_kind_of_field,
    },
    FieldParse {
        token: "Percentage",
        parse: parse_percentage_field,
    },
];
