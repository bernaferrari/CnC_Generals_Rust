//! Real crate helper instances for flag-ON entity module install.
//!
//! Helpers are the same types `Object::install_ctor_helpers` constructs
//! (Object.cpp:299-384). Each graph owns its concrete mutable helpers.
//! Template tags are recorded without `TheGameLogic` update registration
//! (no ticking); no helper handle escapes the owning graph.

use super::entity_modules::{
    EntityModuleInstallSpec, HELPER_TAG_DEFECTION, HELPER_TAG_FIRING_TRACKER, HELPER_TAG_REPULSOR,
    HELPER_TAG_SMC, HELPER_TAG_STATUS, HELPER_TAG_SUBDUAL, HELPER_TAG_TEMP_WEAPON_BONUS,
    HELPER_TAG_WEAPON_STATUS, helper_handle_name,
};
use crate::object::firing_tracker::FiringTracker;
use crate::object::helper::{
    ObjectDefectionHelper, ObjectDefectionHelperModuleData, ObjectRepulsorHelper,
    ObjectRepulsorHelperModuleData, ObjectSMCHelper, ObjectSMCHelperModuleData,
    ObjectWeaponStatusHelper, ObjectWeaponStatusHelperModuleData, StatusDamageHelper,
    StatusDamageHelperModuleData, SubdualDamageHelper, SubdualDamageHelperModuleData,
    TempWeaponBonusHelper, TempWeaponBonusHelperModuleData,
};

#[derive(Debug)]
pub enum EntityLiveModule {
    Smc(ObjectSMCHelper),
    Status(StatusDamageHelper),
    Subdual(SubdualDamageHelper),
    Repulsor(ObjectRepulsorHelper),
    Defection(ObjectDefectionHelper),
    WeaponStatus(ObjectWeaponStatusHelper),
    FiringTracker(FiringTracker),
    TempWeaponBonus(TempWeaponBonusHelper),
    Template { tag: String },
}

impl EntityLiveModule {
    pub fn tag(&self) -> &str {
        match self {
            Self::Smc(_) => HELPER_TAG_SMC,
            Self::Status(_) => HELPER_TAG_STATUS,
            Self::Subdual(_) => HELPER_TAG_SUBDUAL,
            Self::Repulsor(_) => HELPER_TAG_REPULSOR,
            Self::Defection(_) => HELPER_TAG_DEFECTION,
            Self::WeaponStatus(_) => HELPER_TAG_WEAPON_STATUS,
            Self::FiringTracker(_) => HELPER_TAG_FIRING_TRACKER,
            Self::TempWeaponBonus(_) => HELPER_TAG_TEMP_WEAPON_BONUS,
            Self::Template { tag } => tag.as_str(),
        }
    }

    pub fn handle(&self) -> &str {
        helper_handle_name(self.tag())
    }
}

pub fn live_modules_from_spec(spec: &EntityModuleInstallSpec) -> Vec<EntityLiveModule> {
    let mut out = Vec::new();
    out.push(EntityLiveModule::Smc(ObjectSMCHelper::new(
        ObjectSMCHelperModuleData::new(),
    )));
    if !spec.inactive_body {
        out.push(EntityLiveModule::Status(StatusDamageHelper::new(
            0,
            StatusDamageHelperModuleData::new(),
        )));
        out.push(EntityLiveModule::Subdual(SubdualDamageHelper::new(
            0,
            SubdualDamageHelperModuleData::new(),
        )));
    }
    if spec.can_be_repulsed {
        out.push(EntityLiveModule::Repulsor(ObjectRepulsorHelper::new(
            ObjectRepulsorHelperModuleData::new(),
        )));
    }
    if !spec.shrubbery {
        out.push(EntityLiveModule::Defection(ObjectDefectionHelper::new(
            ObjectDefectionHelperModuleData::new(),
        )));
    }
    if spec.has_weapons {
        out.push(EntityLiveModule::WeaponStatus(
            ObjectWeaponStatusHelper::new(ObjectWeaponStatusHelperModuleData::new(), true),
        ));
        out.push(EntityLiveModule::FiringTracker(FiringTracker::new(0)));
        out.push(EntityLiveModule::TempWeaponBonus(
            TempWeaponBonusHelper::new(0, TempWeaponBonusHelperModuleData::new()),
        ));
    }
    for tag in &spec.template_module_tags {
        out.push(EntityLiveModule::Template { tag: tag.clone() });
    }
    out
}

pub fn live_modules_from_tags(tags: &[String]) -> Vec<EntityLiveModule> {
    let mut spec = EntityModuleInstallSpec::default();
    spec.inactive_body = true;
    spec.shrubbery = true;
    spec.can_be_repulsed = false;
    spec.has_weapons = false;
    let mut template_tags = Vec::new();
    for tag in tags {
        match tag.as_str() {
            HELPER_TAG_SMC => {}
            HELPER_TAG_STATUS | HELPER_TAG_SUBDUAL => spec.inactive_body = false,
            HELPER_TAG_REPULSOR => spec.can_be_repulsed = true,
            HELPER_TAG_DEFECTION => spec.shrubbery = false,
            HELPER_TAG_WEAPON_STATUS | HELPER_TAG_FIRING_TRACKER | HELPER_TAG_TEMP_WEAPON_BONUS => {
                spec.has_weapons = true;
            }
            other => template_tags.push(other.to_string()),
        }
    }
    spec.template_module_tags = template_tags;
    live_modules_from_spec(&spec)
}
