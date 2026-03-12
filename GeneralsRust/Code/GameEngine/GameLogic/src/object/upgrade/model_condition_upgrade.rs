use std::sync::Arc;

use crate::common::{LegacyModuleData, ModelConditionFlags, ObjectID, UpgradeMaskType};
use crate::modules::UpgradeModuleInterface;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

/// Module data describing the model condition upgrade.
#[derive(Debug, Clone)]
pub struct ModelConditionUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    condition_flag: ModelConditionFlags,
}

impl Default for ModelConditionUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            condition_flag: ModelConditionFlags::empty(),
        }
    }
}

impl ModelConditionUpgradeModuleData {
    pub fn condition_flag(&self) -> ModelConditionFlags {
        self.condition_flag
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, MODEL_CONDITION_UPGRADE_FIELDS)
    }
}

crate::impl_legacy_module_data_with_key_field!(
    ModelConditionUpgradeModuleData,
    module_tag_name_key
);

impl Snapshotable for ModelConditionUpgradeModuleData {
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

/// Upgrade module that changes model conditions (visual state) on the owning object.
pub struct ModelConditionUpgrade {
    module_name_key: NameKeyType,
    data: Arc<ModelConditionUpgradeModuleData>,
    object_id: ObjectID,
    applied: bool,
}

impl ModelConditionUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<ModelConditionUpgradeModuleData>,
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

impl Module for ModelConditionUpgrade {
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

impl Snapshotable for ModelConditionUpgrade {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        let _ = _xfer.xfer_version(&mut version, 1);
        let mut applied = self.applied;
        let _ = _xfer.xfer_bool(&mut applied);
        self.applied = applied;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl UpgradeModuleInterface for ModelConditionUpgrade {
    fn can_upgrade(&self, _upgrade_mask: UpgradeMaskType) -> bool {
        !self.applied
    }

    fn apply_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) -> bool {
        if self.applied {
            return false;
        }
        use crate::object::registry::OBJECT_REGISTRY;

        let Some(object) = OBJECT_REGISTRY.get_object(self.object_id) else {
            log::warn!("ModelConditionUpgrade: Object {} not found", self.object_id);
            return false;
        };

        let mut object_guard = match object.write() {
            Ok(guard) => guard,
            Err(_) => {
                log::error!(
                    "ModelConditionUpgrade: Failed to lock object {}",
                    self.object_id
                );
                return false;
            }
        };

        let flag = self.data.condition_flag();
        if !flag.is_empty() {
            let _ = object_guard.set_model_condition_flags(flag);
        }

        self.applied = true;
        true
    }

    fn remove_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) {
        // C++ does not clear model condition flags for this upgrade; keep parity.
    }
}

fn parse_model_condition_flag(token: &str) -> Option<ModelConditionFlags> {
    let upper = token.trim().to_ascii_uppercase();
    match upper.as_str() {
        "PRISTINE" => Some(ModelConditionFlags::PRISTINE),
        "DAMAGED" => Some(ModelConditionFlags::DAMAGED),
        "REALLY_DAMAGED" => Some(ModelConditionFlags::REALLY_DAMAGED),
        "REALLYDAMAGED" => Some(ModelConditionFlags::REALLYDAMAGED),
        "RUBBLE" => Some(ModelConditionFlags::RUBBLE),
        "MOVING" => Some(ModelConditionFlags::MOVING),
        "FIRING_PRIMARY" | "FIRING_A" => Some(ModelConditionFlags::FIRING_PRIMARY),
        "FIRING_SECONDARY" | "FIRING_B" => Some(ModelConditionFlags::FIRING_SECONDARY),
        "FIRING_TERTIARY" | "FIRING_C" => Some(ModelConditionFlags::FIRING_TERTIARY),
        "SELECTED" => Some(ModelConditionFlags::SELECTED),
        "POWER_PLANT_UPGRADING" | "POWERPLANTUPGRADING" => {
            Some(ModelConditionFlags::POWER_PLANT_UPGRADING)
        }
        "POWER_PLANT_UPGRADED" | "POWERPLANTUPGRADED" => {
            Some(ModelConditionFlags::POWER_PLANT_UPGRADED)
        }
        "ACTIVELY_BEING_CONSTRUCTED" => Some(ModelConditionFlags::ACTIVELY_BEING_CONSTRUCTED),
        "PARTIALLY_CONSTRUCTED" => Some(ModelConditionFlags::PARTIALLY_CONSTRUCTED),
        "AWAITING_CONSTRUCTION" => Some(ModelConditionFlags::AWAITING_CONSTRUCTION),
        "CONSTRUCTION_COMPLETE" => Some(ModelConditionFlags::CONSTRUCTION_COMPLETE),
        "NIGHT" => Some(ModelConditionFlags::NIGHT),
        "SNOW" => Some(ModelConditionFlags::SNOW),
        "WEAPON_UPGRADED" => Some(ModelConditionFlags::WEAPON_UPGRADED),
        "ARMOR_UPGRADED" => Some(ModelConditionFlags::ARMOR_UPGRADED),
        "DOOR_1_OPENING" | "DOOR1OPENING" => Some(ModelConditionFlags::DOOR_1_OPENING),
        "DOOR_1_WAITING_OPEN" | "DOOR1WAITINGOPEN" => {
            Some(ModelConditionFlags::DOOR_1_WAITING_OPEN)
        }
        "DOOR_1_CLOSING" | "DOOR1CLOSING" => Some(ModelConditionFlags::DOOR_1_CLOSING),
        "DOOR_1_WAITING_TO_CLOSE" | "DOOR1WAITINGTOCLOSE" => {
            Some(ModelConditionFlags::Door1WaitingToClose)
        }
        "DOOR_2_OPENING" | "DOOR2OPENING" => Some(ModelConditionFlags::DOOR_2_OPENING),
        "DOOR_2_WAITING_OPEN" | "DOOR2WAITINGOPEN" => {
            Some(ModelConditionFlags::DOOR_2_WAITING_OPEN)
        }
        "DOOR_2_CLOSING" | "DOOR2CLOSING" => Some(ModelConditionFlags::DOOR_2_CLOSING),
        "DOOR_2_WAITING_TO_CLOSE" | "DOOR2WAITINGTOCLOSE" => {
            Some(ModelConditionFlags::Door2WaitingToClose)
        }
        "DOOR_3_OPENING" | "DOOR3OPENING" => Some(ModelConditionFlags::DOOR_3_OPENING),
        "DOOR_3_WAITING_OPEN" | "DOOR3WAITINGOPEN" => {
            Some(ModelConditionFlags::DOOR_3_WAITING_OPEN)
        }
        "DOOR_3_CLOSING" | "DOOR3CLOSING" => Some(ModelConditionFlags::DOOR_3_CLOSING),
        "DOOR_3_WAITING_TO_CLOSE" | "DOOR3WAITINGTOCLOSE" => {
            Some(ModelConditionFlags::Door3WaitingToClose)
        }
        "DOOR_4_OPENING" | "DOOR4OPENING" => Some(ModelConditionFlags::DOOR_4_OPENING),
        "DOOR_4_WAITING_OPEN" | "DOOR4WAITINGOPEN" => {
            Some(ModelConditionFlags::DOOR_4_WAITING_OPEN)
        }
        "DOOR_4_CLOSING" | "DOOR4CLOSING" => Some(ModelConditionFlags::DOOR_4_CLOSING),
        "PARACHUTING" => Some(ModelConditionFlags::PARACHUTING),
        "EXPLODED_FLAILING" => Some(ModelConditionFlags::EXPLODED_FLAILING),
        "EXPLODED_BOUNCING" => Some(ModelConditionFlags::EXPLODED_BOUNCING),
        "SPLATTED" => Some(ModelConditionFlags::SPLATTED),
        "CAPTURED" => Some(ModelConditionFlags::CAPTURED),
        "CENTER_TO_RIGHT" | "CENTERTORIGHT" => Some(ModelConditionFlags::CenterToRight),
        "CENTER_TO_LEFT" | "CENTERTOLEFT" => Some(ModelConditionFlags::CenterToLeft),
        "RIGHT_TO_CENTER" | "RIGHTTOCENTER" => Some(ModelConditionFlags::RightToCenter),
        "LEFT_TO_CENTER" | "LEFTTOCENTER" => Some(ModelConditionFlags::LeftToCenter),
        "PACKING" => Some(ModelConditionFlags::Packing),
        "UNPACKING" => Some(ModelConditionFlags::Unpacking),
        "BETWEEN_FIRING_SHOTS_B" => Some(ModelConditionFlags::BetweenFiringShotsB),
        "BETWEEN_FIRING_SHOTS_C" => Some(ModelConditionFlags::BetweenFiringShotsC),
        "RELOADING_B" => Some(ModelConditionFlags::ReloadingB),
        "RELOADING_C" => Some(ModelConditionFlags::ReloadingC),
        "ACTIVELY_CONSTRUCTING" => Some(ModelConditionFlags::ActivelyConstructing),
        "RADAR_EXTENDING" => Some(ModelConditionFlags::RadarExtending),
        "RADAR_UPGRADED" => Some(ModelConditionFlags::RadarUpgraded),
        "AFLAME" => Some(ModelConditionFlags::AFLAME),
        "SMOLDERING" => Some(ModelConditionFlags::SMOLDERING),
        "BURNED" => Some(ModelConditionFlags::BURNED),
        "LOADED" => Some(ModelConditionFlags::Loaded),
        "ARMORSET_CRATEUPGRADE_ONE" => Some(ModelConditionFlags::ArmorsetCrateUpgradeOne),
        "ARMORSET_CRATEUPGRADE_TWO" => Some(ModelConditionFlags::ArmorsetCrateUpgradeTwo),
        "DISGUISED" => Some(ModelConditionFlags::DISGUISED),
        "TOPPLED" | "FRONTCRUSHED" => Some(ModelConditionFlags::TOPPLED),
        "FLOODED" | "BACKCRUSHED" => Some(ModelConditionFlags::FLOODED),
        "POST_COLLAPSE" => Some(ModelConditionFlags::POST_COLLAPSE),
        "JETAFTERBURNER" => Some(ModelConditionFlags::JETAFTERBURNER),
        "JETEXHAUST" => Some(ModelConditionFlags::JETEXHAUST),
        "ENEMYNEAR" => Some(ModelConditionFlags::ENEMYNEAR),
        "STUNNED_FLAILING" => Some(ModelConditionFlags::STUNNED_FLAILING),
        "STUNNED" => Some(ModelConditionFlags::STUNNED),
        "FREEFALL" => Some(ModelConditionFlags::FREEFALL),
        "PRONE" => Some(ModelConditionFlags::PRONE),
        "SPECIAL_CHEERING" => Some(ModelConditionFlags::SPECIAL_CHEERING),
        "SPECIAL_DAMAGED" => Some(ModelConditionFlags::SPECIAL_DAMAGED),
        "ATTACKING" => Some(ModelConditionFlags::ATTACKING),
        "DYING" => Some(ModelConditionFlags::DYING),
        "CARRYING" => Some(ModelConditionFlags::CARRYING),
        "DEPLOYED" => Some(ModelConditionFlags::DEPLOYED),
        "OVER_WATER" => Some(ModelConditionFlags::OVER_WATER),
        "SOLD" => Some(ModelConditionFlags::SOLD),
        "ARMED" => Some(ModelConditionFlags::ARMED),
        "SECOND_LIFE" => Some(ModelConditionFlags::SECOND_LIFE),
        "JAMMED" => Some(ModelConditionFlags::JAMMED),
        "WEAPONSET_VETERAN" => Some(ModelConditionFlags::WEAPONSET_VETERAN),
        "WEAPONSET_ELITE" => Some(ModelConditionFlags::WEAPONSET_ELITE),
        "WEAPONSET_HERO" => Some(ModelConditionFlags::WEAPONSET_HERO),
        "WEAPONSET_CRATEUPGRADE_ONE" => Some(ModelConditionFlags::WEAPONSET_CRATEUPGRADE_ONE),
        "WEAPONSET_CRATEUPGRADE_TWO" => Some(ModelConditionFlags::WEAPONSET_CRATEUPGRADE_TWO),
        "WEAPONSET_PLAYER_UPGRADE" => Some(ModelConditionFlags::WEAPONSET_PLAYER_UPGRADE),
        "PANICKING" => Some(ModelConditionFlags::PANICKING),
        "GARRISONED" => Some(ModelConditionFlags::GARRISONED),
        "USER_1" => Some(ModelConditionFlags::USER_1),
        "USER_2" => Some(ModelConditionFlags::USER_2),
        _ => None,
    }
}

fn parse_condition_flag_field(
    _ini: &mut INI,
    data: &mut ModelConditionUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    let Some(flag) = parse_model_condition_flag(tokens[0]) else {
        return Err(INIError::InvalidData);
    };
    data.condition_flag = flag;
    Ok(())
}

const MODEL_CONDITION_UPGRADE_FIELDS: &[FieldParse<ModelConditionUpgradeModuleData>] =
    &[FieldParse {
        token: "ConditionFlag",
        parse: parse_condition_flag_field,
    }];
