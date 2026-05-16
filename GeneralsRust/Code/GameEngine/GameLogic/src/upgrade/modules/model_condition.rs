// ModelConditionUpgrade - Sets model condition flags for visual changes
use super::super::UpgradeMask;
use super::upgrade_mux::{UpgradeModuleInterface, UpgradeMux, UpgradeMuxData};
use crate::common::*;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ModelConditionUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    pub upgrade_mux_data: UpgradeMuxData,
    pub condition_flag: ModelConditionFlags,
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
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, MODEL_CONDITION_UPGRADE_FIELDS)
    }
}

impl ModuleData for ModelConditionUpgradeModuleData {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.module_tag_name_key = key;
    }
    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_tag_name_key
    }
}

impl Snapshotable for ModelConditionUpgradeModuleData {
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

pub struct ModelConditionUpgrade {
    module_name_key: NameKeyType,
    data: Arc<ModelConditionUpgradeModuleData>,
    object_id: ObjectID,
    mux: UpgradeMux,
}

impl ModelConditionUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<ModelConditionUpgradeModuleData>,
        object_id: ObjectID,
    ) -> Self {
        let mux = UpgradeMux::new(data.upgrade_mux_data.clone());
        Self {
            module_name_key,
            data,
            object_id,
            mux,
        }
    }

    fn upgrade_implementation(&mut self, object: &mut Object) {
        if self.data.condition_flag != ModelConditionFlags::Invalid {
            object.set_model_condition_state(self.data.condition_flag);
            log::info!(
                "ModelConditionUpgrade: Setting condition flag {:?} for object {}",
                self.data.condition_flag,
                self.object_id
            );
        }
    }
}

impl UpgradeModuleInterface for ModelConditionUpgrade {
    fn is_already_upgraded(&self) -> bool {
        self.mux.is_already_upgraded()
    }
    fn attempt_upgrade(&mut self, key_mask: UpgradeMask, object: &mut Object) -> bool {
        if self.mux.would_upgrade(key_mask) {
            self.mux.data.perform_upgrade_fx(object);
            self.upgrade_implementation(object);
            self.mux.set_upgrade_executed(true);
            true
        } else {
            false
        }
    }
    fn would_upgrade(&self, key_mask: UpgradeMask) -> bool {
        self.mux.would_upgrade(key_mask)
    }
    fn reset_upgrade(&mut self, key_mask: UpgradeMask) -> bool {
        self.mux.reset_upgrade(key_mask)
    }
    fn test_upgrade_conditions(&self, key_mask: UpgradeMask) -> bool {
        self.mux.test_upgrade_conditions(key_mask)
    }
    fn force_refresh_upgrade(&mut self, object: &mut Object) {
        if self.is_already_upgraded() {
            self.upgrade_implementation(object);
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
        self.data.get_module_tag_name_key()
    }
    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }
}

impl Snapshotable for ModelConditionUpgrade {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.mux.crc(xfer)
    }
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| e.to_string())?;
        self.mux.xfer(xfer)
    }
    fn load_post_process(&mut self) -> Result<(), String> {
        self.mux.load_post_process()
    }
}

fn parse_condition_flag(
    _ini: &mut INI,
    data: &mut ModelConditionUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .skip_while(|t| **t == "=")
        .next()
        .ok_or(INIError::InvalidData)?;
    data.condition_flag = parse_model_condition_flag(value).ok_or(INIError::InvalidData)?;
    Ok(())
}

fn parse_triggered_by(
    _ini: &mut INI,
    data: &mut ModelConditionUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    for token in tokens.iter().skip_while(|t| **t == "=") {
        if !token.is_empty() {
            data.upgrade_mux_data
                .activation_upgrade_names
                .push(AsciiString::from(*token));
        }
    }
    Ok(())
}

fn parse_conflicts_with(
    _ini: &mut INI,
    data: &mut ModelConditionUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    for token in tokens.iter().skip_while(|t| **t == "=") {
        if !token.is_empty() {
            data.upgrade_mux_data
                .conflicting_upgrade_names
                .push(AsciiString::from(*token));
        }
    }
    Ok(())
}

fn parse_removes_upgrades(
    _ini: &mut INI,
    data: &mut ModelConditionUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    for token in tokens.iter().skip_while(|t| **t == "=") {
        if !token.is_empty() {
            data.upgrade_mux_data
                .removal_upgrade_names
                .push(AsciiString::from(*token));
        }
    }
    Ok(())
}

fn parse_requires_all_triggers(
    _ini: &mut INI,
    data: &mut ModelConditionUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .skip_while(|t| **t == "=")
        .next()
        .ok_or(INIError::InvalidData)?;
    data.upgrade_mux_data.requires_all_triggers = INI::parse_bool(value)?;
    Ok(())
}

const MODEL_CONDITION_UPGRADE_FIELDS: &[FieldParse<ModelConditionUpgradeModuleData>] = &[
    FieldParse {
        token: "TriggeredBy",
        parse: parse_triggered_by,
    },
    FieldParse {
        token: "ConflictsWith",
        parse: parse_conflicts_with,
    },
    FieldParse {
        token: "RemovesUpgrades",
        parse: parse_removes_upgrades,
    },
    FieldParse {
        token: "RequiresAllTriggers",
        parse: parse_requires_all_triggers,
    },
    FieldParse {
        token: "ConditionFlag",
        parse: parse_condition_flag,
    },
];

pub(crate) fn parse_model_condition_flag(value: &str) -> Option<ModelConditionFlags> {
    let key = value.trim();
    if key.is_empty() {
        return None;
    }
    let mut upper = key.to_uppercase();
    if let Some(stripped) = upper.strip_prefix("MODELCONDITION_") {
        upper = stripped.to_string();
    }
    let normalized = upper.replace('_', "");
    match normalized.as_str() {
        "INVALID" => Some(ModelConditionFlags::Invalid),
        "PRISTINE" => Some(ModelConditionFlags::PRISTINE),
        "DAMAGED" => Some(ModelConditionFlags::DAMAGED),
        "REALLYDAMAGED" => Some(ModelConditionFlags::REALLY_DAMAGED),
        "RUBBLE" => Some(ModelConditionFlags::RUBBLE),
        "MOVING" => Some(ModelConditionFlags::MOVING),
        "FIRINGPRIMARY" => Some(ModelConditionFlags::FIRING_PRIMARY),
        "FIRINGSECONDARY" => Some(ModelConditionFlags::FIRING_SECONDARY),
        "FIRINGTERTIARY" => Some(ModelConditionFlags::FIRING_TERTIARY),
        "SELECTED" => Some(ModelConditionFlags::SELECTED),
        "POWERPLANTUPGRADING" => Some(ModelConditionFlags::POWER_PLANT_UPGRADING),
        "POWERPLANTUPGRADED" => Some(ModelConditionFlags::POWER_PLANT_UPGRADED),
        "ACTIVELYBEINGCONSTRUCTED" => Some(ModelConditionFlags::ACTIVELY_BEING_CONSTRUCTED),
        "PARTIALLYCONSTRUCTED" => Some(ModelConditionFlags::PARTIALLY_CONSTRUCTED),
        "AWAITINGCONSTRUCTION" => Some(ModelConditionFlags::AWAITING_CONSTRUCTION),
        "CONSTRUCTIONCOMPLETE" => Some(ModelConditionFlags::CONSTRUCTION_COMPLETE),
        "NIGHT" => Some(ModelConditionFlags::NIGHT),
        "SNOW" => Some(ModelConditionFlags::SNOW),
        "WEAPONUPGRADED" => Some(ModelConditionFlags::WEAPON_UPGRADED),
        "ARMORUPGRADED" => Some(ModelConditionFlags::ARMOR_UPGRADED),
        "DOOR1OPENING" => Some(ModelConditionFlags::DOOR_1_OPENING),
        "DOOR1WAITINGOPEN" => Some(ModelConditionFlags::DOOR_1_WAITING_OPEN),
        "DOOR1CLOSING" => Some(ModelConditionFlags::DOOR_1_CLOSING),
        "DOOR2OPENING" => Some(ModelConditionFlags::DOOR_2_OPENING),
        "DOOR2WAITINGOPEN" => Some(ModelConditionFlags::DOOR_2_WAITING_OPEN),
        "DOOR2CLOSING" => Some(ModelConditionFlags::DOOR_2_CLOSING),
        "DOOR3OPENING" => Some(ModelConditionFlags::DOOR_3_OPENING),
        "DOOR3WAITINGOPEN" => Some(ModelConditionFlags::DOOR_3_WAITING_OPEN),
        "DOOR3CLOSING" => Some(ModelConditionFlags::DOOR_3_CLOSING),
        "DOOR4OPENING" => Some(ModelConditionFlags::DOOR_4_OPENING),
        "DOOR4WAITINGOPEN" => Some(ModelConditionFlags::DOOR_4_WAITING_OPEN),
        "DOOR4CLOSING" => Some(ModelConditionFlags::DOOR_4_CLOSING),
        "PARACHUTING" => Some(ModelConditionFlags::PARACHUTING),
        "EXPLODEDFLAILING" => Some(ModelConditionFlags::EXPLODED_FLAILING),
        "EXPLODEDBOUNCING" => Some(ModelConditionFlags::EXPLODED_BOUNCING),
        "SPLATTED" => Some(ModelConditionFlags::SPLATTED),
        "CAPTURED" => Some(ModelConditionFlags::CAPTURED),
        "CENTERTORIGHT" => Some(ModelConditionFlags::CenterToRight),
        "CENTERTOLEFT" => Some(ModelConditionFlags::CenterToLeft),
        "RIGHTTOCENTER" => Some(ModelConditionFlags::RightToCenter),
        "LEFTTOCENTER" => Some(ModelConditionFlags::LeftToCenter),
        "PACKING" => Some(ModelConditionFlags::Packing),
        "UNPACKING" => Some(ModelConditionFlags::Unpacking),
        "FIRINGB" => Some(ModelConditionFlags::FiringB),
        "FIRINGC" => Some(ModelConditionFlags::FiringC),
        "BETWEENFIRINGSHOTSB" => Some(ModelConditionFlags::BetweenFiringShotsB),
        "BETWEENFIRINGSHOTSC" => Some(ModelConditionFlags::BetweenFiringShotsC),
        "RELOADINGB" => Some(ModelConditionFlags::ReloadingB),
        "RELOADINGC" => Some(ModelConditionFlags::ReloadingC),
        "ACTIVELYCONSTRUCTING" => Some(ModelConditionFlags::ActivelyConstructing),
        "RADAREXTENDING" => Some(ModelConditionFlags::RadarExtending),
        "RADARUPGRADED" => Some(ModelConditionFlags::RadarUpgraded),
        "AFLAME" => Some(ModelConditionFlags::Aflame),
        "SMOLDERING" => Some(ModelConditionFlags::Smoldering),
        "BURNED" => Some(ModelConditionFlags::Burned),
        "DOOR1WAITINGTOCLOSE" => Some(ModelConditionFlags::Door1WaitingToClose),
        "DOOR2WAITINGTOCLOSE" => Some(ModelConditionFlags::Door2WaitingToClose),
        "DOOR3WAITINGTOCLOSE" => Some(ModelConditionFlags::Door3WaitingToClose),
        "LOADED" => Some(ModelConditionFlags::Loaded),
        "ARMORSETCRATEUPGRADEONE" => Some(ModelConditionFlags::ArmorsetCrateUpgradeOne),
        "ARMORSETCRATEUPGRADETWO" => Some(ModelConditionFlags::ArmorsetCrateUpgradeTwo),
        "DISGUISED" => Some(ModelConditionFlags::DISGUISED),
        "TOPPLED" => Some(ModelConditionFlags::TOPPLED),
        "FLOODED" => Some(ModelConditionFlags::FLOODED),
        "POSTCOLLAPSE" => Some(ModelConditionFlags::POST_COLLAPSE),
        "JETAFTERBURNER" => Some(ModelConditionFlags::JETAFTERBURNER),
        "JETEXHAUST" => Some(ModelConditionFlags::JETEXHAUST),
        "PREORDER" => Some(ModelConditionFlags::PREORDER),
        "ENEMYNEAR" => Some(ModelConditionFlags::ENEMYNEAR),
        "STUNNEDFLAILING" => Some(ModelConditionFlags::STUNNED_FLAILING),
        "STUNNED" => Some(ModelConditionFlags::STUNNED),
        "FREEFALL" => Some(ModelConditionFlags::FREEFALL),
        "PRONE" => Some(ModelConditionFlags::PRONE),
        "PANICKING" => Some(ModelConditionFlags::PANICKING),
        "GARRISONED" => Some(ModelConditionFlags::GARRISONED),
        "USER1" => Some(ModelConditionFlags::USER_1),
        "USER2" => Some(ModelConditionFlags::USER_2),
        "BETWEENFIRINGSHOTSA" => Some(ModelConditionFlags::BetweenFiringShotsA),
        "RELOADINGA" => Some(ModelConditionFlags::ReloadingA),
        "PREATTACKA" => Some(ModelConditionFlags::PreAttackA),
        "USINGWEAPONA" => Some(ModelConditionFlags::UsingWeaponA),
        "PREATTACKB" => Some(ModelConditionFlags::PreAttackB),
        "USINGWEAPONB" => Some(ModelConditionFlags::UsingWeaponB),
        "PREATTACKC" => Some(ModelConditionFlags::PreAttackC),
        "USINGWEAPONC" => Some(ModelConditionFlags::UsingWeaponC),
        "DOCKING" => Some(ModelConditionFlags::DOCKING),
        "DOCKINGBEGINNING" => Some(ModelConditionFlags::DOCKING_BEGINNING),
        "DOCKINGACTIVE" => Some(ModelConditionFlags::DOCKING_ACTIVE),
        "DOCKINGENDING" => Some(ModelConditionFlags::DOCKING_ENDING),
        "CLIMBING" => Some(ModelConditionFlags::CLIMBING),
        "RAPPELLING" => Some(ModelConditionFlags::RAPPELLING),
        "RIDER1" => Some(ModelConditionFlags::RIDER1),
        "RIDER2" => Some(ModelConditionFlags::RIDER2),
        "RIDER3" => Some(ModelConditionFlags::RIDER3),
        "RIDER4" => Some(ModelConditionFlags::RIDER4),
        "RIDER5" => Some(ModelConditionFlags::RIDER5),
        "RIDER6" => Some(ModelConditionFlags::RIDER6),
        "RIDER7" => Some(ModelConditionFlags::RIDER7),
        "RIDER8" => Some(ModelConditionFlags::RIDER8),
        "SPECIALCHEERING" => Some(ModelConditionFlags::SPECIAL_CHEERING),
        "SPECIALDAMAGED" => Some(ModelConditionFlags::SPECIAL_DAMAGED),
        "ATTACKING" => Some(ModelConditionFlags::ATTACKING),
        "DYING" => Some(ModelConditionFlags::DYING),
        "CARRYING" => Some(ModelConditionFlags::CARRYING),
        "DEPLOYED" => Some(ModelConditionFlags::DEPLOYED),
        "OVERWATER" => Some(ModelConditionFlags::OVER_WATER),
        "SOLD" => Some(ModelConditionFlags::SOLD),
        "ARMED" => Some(ModelConditionFlags::ARMED),
        "SECONDLIFE" => Some(ModelConditionFlags::SECOND_LIFE),
        "JAMMED" => Some(ModelConditionFlags::JAMMED),
        "WEAPONSETVETERAN" => Some(ModelConditionFlags::WEAPONSET_VETERAN),
        "WEAPONSETELITE" => Some(ModelConditionFlags::WEAPONSET_ELITE),
        "WEAPONSETHERO" => Some(ModelConditionFlags::WEAPONSET_HERO),
        "WEAPONSETCRATEUPGRADEONE" => Some(ModelConditionFlags::WEAPONSET_CRATEUPGRADE_ONE),
        "WEAPONSETCRATEUPGRADETWO" => Some(ModelConditionFlags::WEAPONSET_CRATEUPGRADE_TWO),
        "WEAPONSETPLAYERUPGRADE" => Some(ModelConditionFlags::WEAPONSET_PLAYER_UPGRADE),
        _ => None,
    }
}
