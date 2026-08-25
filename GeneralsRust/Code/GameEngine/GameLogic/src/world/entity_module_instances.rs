//! Real crate helper instances for flag-ON entity module install.
//!
//! Helpers are the same types `Object::install_ctor_helpers` constructs
//! (Object.cpp:299-384). Template tags are recorded as live handles without
//! `TheGameLogic` update registration (no ticking).

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
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub enum EntityLiveModule {
    Smc(Arc<Mutex<ObjectSMCHelper>>),
    Status(Arc<Mutex<StatusDamageHelper>>),
    Subdual(Arc<Mutex<SubdualDamageHelper>>),
    Repulsor(Arc<Mutex<ObjectRepulsorHelper>>),
    Defection(Arc<Mutex<ObjectDefectionHelper>>),
    WeaponStatus(Arc<Mutex<ObjectWeaponStatusHelper>>),
    FiringTracker(Arc<Mutex<FiringTracker>>),
    TempWeaponBonus(Arc<Mutex<TempWeaponBonusHelper>>),
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
    out.push(EntityLiveModule::Smc(Arc::new(Mutex::new(
        ObjectSMCHelper::new(ObjectSMCHelperModuleData::new()),
    ))));
    if !spec.inactive_body {
        out.push(EntityLiveModule::Status(Arc::new(Mutex::new(
            StatusDamageHelper::new(0, StatusDamageHelperModuleData::new()),
        ))));
        out.push(EntityLiveModule::Subdual(Arc::new(Mutex::new(
            SubdualDamageHelper::new(0, SubdualDamageHelperModuleData::new()),
        ))));
    }
    if spec.can_be_repulsed {
        out.push(EntityLiveModule::Repulsor(Arc::new(Mutex::new(
            ObjectRepulsorHelper::new(ObjectRepulsorHelperModuleData::new()),
        ))));
    }
    if !spec.shrubbery {
        out.push(EntityLiveModule::Defection(Arc::new(Mutex::new(
            ObjectDefectionHelper::new(ObjectDefectionHelperModuleData::new()),
        ))));
    }
    if spec.has_weapons {
        out.push(EntityLiveModule::WeaponStatus(Arc::new(Mutex::new(
            ObjectWeaponStatusHelper::new(ObjectWeaponStatusHelperModuleData::new(), true),
        ))));
        out.push(EntityLiveModule::FiringTracker(Arc::new(Mutex::new(
            FiringTracker::new(0),
        ))));
        out.push(EntityLiveModule::TempWeaponBonus(Arc::new(Mutex::new(
            TempWeaponBonusHelper::new(0, TempWeaponBonusHelperModuleData::new()),
        ))));
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
