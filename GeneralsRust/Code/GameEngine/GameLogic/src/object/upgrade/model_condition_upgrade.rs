use std::sync::Arc;

use crate::common::{LegacyModuleData, ModelConditionFlags, ObjectID, UpgradeMaskType};
use crate::modules::UpgradeModuleInterface;
use crate::object::upgrade::upgrade_module::{
    UpgradeMuxData, mux_can_upgrade, mux_give_self_upgrade_for_object,
};
use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

/// Wave 448: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    crate::object::registry::OBJECT_REGISTRY.is_empty()
}

/// Module data describing the model condition upgrade.
#[derive(Debug, Clone)]
pub struct ModelConditionUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    pub upgrade_mux_data: UpgradeMuxData,
    condition_flag: ModelConditionFlags,
}

impl Default for ModelConditionUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            upgrade_mux_data: UpgradeMuxData::default(),
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
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|err| format!("{:?} crc version: {err:?}", std::any::type_name::<Self>()))?;
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

impl UpgradeModuleInterface for ModelConditionUpgrade {
    fn can_upgrade(&self, upgrade_mask: UpgradeMaskType) -> bool {
        mux_can_upgrade(&self.data.upgrade_mux_data, self.applied, upgrade_mask)
    }

    fn apply_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) -> bool {
        // Wave 448: empty dual-world → false.
        if dual_world_registry_unavailable() {
            return false;
        }

        if self.applied {
            return false;
        }
        mux_give_self_upgrade_for_object(&self.data.upgrade_mux_data, self.object_id);
        use crate::object::registry::OBJECT_REGISTRY;

        let flag = self.data.condition_flag();
        let Some(()) = OBJECT_REGISTRY.with_object_mut(self.object_id, |object_guard| {
            if !flag.is_empty() {
                let _ = object_guard.set_model_condition_flags(flag);
            }
        }) else {
            log::warn!("ModelConditionUpgrade: Object {} not found", self.object_id);
            return false;
        };

        self.applied = true;
        true
    }

    fn remove_upgrade(&mut self, upgrade_mask: UpgradeMaskType) {
        // C++ does not clear model condition flags; resetUpgrade only clears executed.
        let _ = crate::object::upgrade::upgrade_module::mux_reset_upgrade(
            &self.data.upgrade_mux_data,
            &mut self.applied,
            upgrade_mask,
        );
    }
}

fn parse_model_condition_flag(token: &str) -> Option<ModelConditionFlags> {
    let upper = token.trim().to_ascii_uppercase();
    // Compact / alias spellings used in some INI blocks. C++ parseSingleBitFromINI
    // uses BitFlags.cpp s_bitNameList; aliases are extra.
    let canonical = match upper.as_str() {
        "FIRING_PRIMARY" => "FIRING_A",
        "FIRING_SECONDARY" => "FIRING_B",
        "FIRING_TERTIARY" => "FIRING_C",
        "REALLY_DAMAGED" => "REALLYDAMAGED",
        "POWERPLANTUPGRADING" => "POWER_PLANT_UPGRADING",
        "POWERPLANTUPGRADED" => "POWER_PLANT_UPGRADED",
        "DOOR1OPENING" => "DOOR_1_OPENING",
        "DOOR1WAITINGOPEN" => "DOOR_1_WAITING_OPEN",
        "DOOR1CLOSING" => "DOOR_1_CLOSING",
        "DOOR1WAITINGTOCLOSE" => "DOOR_1_WAITING_TO_CLOSE",
        "DOOR2OPENING" => "DOOR_2_OPENING",
        "DOOR2WAITINGOPEN" => "DOOR_2_WAITING_OPEN",
        "DOOR2CLOSING" => "DOOR_2_CLOSING",
        "DOOR2WAITINGTOCLOSE" => "DOOR_2_WAITING_TO_CLOSE",
        "DOOR3OPENING" => "DOOR_3_OPENING",
        "DOOR3WAITINGOPEN" => "DOOR_3_WAITING_OPEN",
        "DOOR3CLOSING" => "DOOR_3_CLOSING",
        "DOOR3WAITINGTOCLOSE" => "DOOR_3_WAITING_TO_CLOSE",
        "DOOR4OPENING" => "DOOR_4_OPENING",
        "DOOR4WAITINGOPEN" => "DOOR_4_WAITING_OPEN",
        "DOOR4CLOSING" => "DOOR_4_CLOSING",
        "DOOR4WAITINGTOCLOSE" => "DOOR_4_WAITING_TO_CLOSE",
        "CENTERTORIGHT" => "CENTER_TO_RIGHT",
        "CENTERTOLEFT" => "CENTER_TO_LEFT",
        "RIGHTTOCENTER" => "RIGHT_TO_CENTER",
        "LEFTTOCENTER" => "LEFT_TO_CENTER",
        other => other,
    };

    if let Some(bit) = game_engine::common::bit_flags::ModelConditionFlags::BIT_NAMES
        .iter()
        .position(|name| name.eq_ignore_ascii_case(canonical))
    {
        return ModelConditionFlags::from_bits(1u128 << bit);
    }

    // Extra GameLogic-only names (not in C++ ModelConditionType).
    match canonical {
        "PRISTINE" => Some(ModelConditionFlags::PRISTINE),
        "SELECTED" => Some(ModelConditionFlags::SELECTED),
        "WEAPON_UPGRADED" => Some(ModelConditionFlags::WEAPON_UPGRADED),
        "ARMOR_UPGRADED" => Some(ModelConditionFlags::ARMOR_UPGRADED),
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

crate::impl_upgrade_mux_field_parsers!(ModelConditionUpgradeModuleData);

const MODEL_CONDITION_UPGRADE_FIELDS: &[FieldParse<ModelConditionUpgradeModuleData>] =
    crate::upgrade_mux_field_table!(FieldParse {
        token: "ConditionFlag",
        parse: parse_condition_flag_field,
    },);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crush_flags_are_distinct_from_toppled_and_flooded() {
        let front = parse_model_condition_flag("FRONTCRUSHED").expect("FRONTCRUSHED");
        let back = parse_model_condition_flag("BACKCRUSHED").expect("BACKCRUSHED");
        let toppled = parse_model_condition_flag("TOPPLED").expect("TOPPLED");
        let flooded = parse_model_condition_flag("FLOODED").expect("FLOODED");
        assert_eq!(front, ModelConditionFlags::FRONTCRUSHED);
        assert_eq!(back, ModelConditionFlags::BACKCRUSHED);
        assert_ne!(front, toppled);
        assert_ne!(back, flooded);
    }

    #[test]
    fn missing_cpp_condition_flags_parse() {
        assert_eq!(
            parse_model_condition_flag("PREATTACK_A"),
            Some(ModelConditionFlags::PREATTACK_A)
        );
        assert_eq!(
            parse_model_condition_flag("USING_WEAPON_B"),
            Some(ModelConditionFlags::USING_WEAPON_B)
        );
        assert_eq!(
            parse_model_condition_flag("DOOR_4_WAITING_TO_CLOSE"),
            Some(ModelConditionFlags::DOOR_4_WAITING_TO_CLOSE)
        );
        assert_eq!(
            parse_model_condition_flag("RIDER3"),
            Some(ModelConditionFlags::RIDER3)
        );
        assert_eq!(
            parse_model_condition_flag("TURRET_ROTATE"),
            Some(ModelConditionFlags::TURRET_ROTATE)
        );
        assert_eq!(
            parse_model_condition_flag("CONTINUOUS_FIRE_FAST"),
            Some(ModelConditionFlags::CONTINUOUS_FIRE_FAST)
        );
    }
}
