//! Rider Change Contain Module
//!
//! Specialized container that can change the type of riders it contains

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, Weak};

use super::{ContainerIniParse, ContainerInterface};
use crate::common::{
    AsciiString, GameResult, LocomotorSetType, ModelConditionFlags, ObjectID, ObjectStatusMaskType,
    ObjectStatusTypes, PlayerMaskType,
};
use crate::damage::{DamageInfo, DamageType, DeathType};
use crate::helpers::{TheGameLogic, TheThingFactory};
use crate::modules::{ContainModuleInterface, ContainWant, UpdateSleepTime};
use crate::object::contain::TransportContain;
use crate::object::{Object, ObjectId};
use crate::weapon::WeaponSetType;
use game_engine::common::ini::{FieldParse, INIError, INI};

/// Configuration data for RiderChangeContain module
#[derive(Debug, Clone)]
pub struct RiderChangeContainModuleData {
    /// Configuration from parent TransportContain
    pub base: super::TransportContainModuleData,
    /// Rider info entries
    pub riders: [RiderInfo; 8],
    /// Scuttle delay in frames
    pub scuttle_frames: u32,
    /// Scuttle status (model condition)
    pub scuttle_state: ModelConditionFlags,
}

#[derive(Debug, Clone)]
pub struct RiderInfo {
    pub template_name: String,
    pub model_condition_flag: ModelConditionFlags,
    pub weapon_set_flag: WeaponSetType,
    pub object_status: ObjectStatusMaskType,
    pub command_set: AsciiString,
    pub locomotor_set: LocomotorSetType,
}

impl Default for RiderInfo {
    fn default() -> Self {
        Self {
            template_name: String::new(),
            model_condition_flag: ModelConditionFlags::empty(),
            weapon_set_flag: WeaponSetType::Veteran,
            object_status: ObjectStatusMaskType::empty(),
            command_set: AsciiString::new(),
            locomotor_set: LocomotorSetType::Invalid,
        }
    }
}

impl Default for RiderChangeContainModuleData {
    fn default() -> Self {
        Self {
            base: Default::default(),
            riders: std::array::from_fn(|_| RiderInfo::default()),
            scuttle_frames: 0,
            scuttle_state: ModelConditionFlags::TOPPLED,
        }
    }
}

impl RiderChangeContainModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        self.base.parse_from_ini(ini)?;
        ini.init_from_ini_with_fields_allow_unknown(self, RIDER_CHANGE_FIELDS)
    }

    pub fn parse_from_config(&mut self, config: &str) -> Result<(), INIError> {
        self.base.parse_from_config(config)?;
        super::parse_with_fields_allow_unknown(config, self, RIDER_CHANGE_FIELDS)
    }
}

impl ContainerIniParse for RiderChangeContainModuleData {
    fn parse_from_config(&mut self, config: &str) -> Result<(), INIError> {
        RiderChangeContainModuleData::parse_from_config(self, config)
    }
}

fn parse_rider_info_at(
    index: usize,
    _ini: &mut INI,
    data: &mut RiderChangeContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.len() < 6 {
        return Err(INIError::InvalidData);
    }

    data.riders[index] = RiderInfo {
        template_name: tokens[0].to_string(),
        model_condition_flag: parse_model_condition_flag(tokens[1])?,
        weapon_set_flag: parse_weapon_set_type(tokens[2])?,
        object_status: parse_object_status(tokens[3])?,
        command_set: AsciiString::from(tokens[4]),
        locomotor_set: parse_locomotor_set_type(tokens[5])?,
    };
    Ok(())
}

fn parse_rider1(
    ini: &mut INI,
    data: &mut RiderChangeContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_rider_info_at(0, ini, data, tokens)
}

fn parse_rider2(
    ini: &mut INI,
    data: &mut RiderChangeContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_rider_info_at(1, ini, data, tokens)
}

fn parse_rider3(
    ini: &mut INI,
    data: &mut RiderChangeContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_rider_info_at(2, ini, data, tokens)
}

fn parse_rider4(
    ini: &mut INI,
    data: &mut RiderChangeContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_rider_info_at(3, ini, data, tokens)
}

fn parse_rider5(
    ini: &mut INI,
    data: &mut RiderChangeContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_rider_info_at(4, ini, data, tokens)
}

fn parse_rider6(
    ini: &mut INI,
    data: &mut RiderChangeContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_rider_info_at(5, ini, data, tokens)
}

fn parse_rider7(
    ini: &mut INI,
    data: &mut RiderChangeContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_rider_info_at(6, ini, data, tokens)
}

fn parse_rider8(
    ini: &mut INI,
    data: &mut RiderChangeContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_rider_info_at(7, ini, data, tokens)
}

fn parse_scuttle_delay(
    _ini: &mut INI,
    data: &mut RiderChangeContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.scuttle_frames = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

fn parse_scuttle_status(
    _ini: &mut INI,
    data: &mut RiderChangeContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.scuttle_state = parse_model_condition_flag(token)?;
    Ok(())
}

const RIDER_CHANGE_FIELDS: &[FieldParse<RiderChangeContainModuleData>] = &[
    FieldParse {
        token: "Rider1",
        parse: parse_rider1,
    },
    FieldParse {
        token: "Rider2",
        parse: parse_rider2,
    },
    FieldParse {
        token: "Rider3",
        parse: parse_rider3,
    },
    FieldParse {
        token: "Rider4",
        parse: parse_rider4,
    },
    FieldParse {
        token: "Rider5",
        parse: parse_rider5,
    },
    FieldParse {
        token: "Rider6",
        parse: parse_rider6,
    },
    FieldParse {
        token: "Rider7",
        parse: parse_rider7,
    },
    FieldParse {
        token: "Rider8",
        parse: parse_rider8,
    },
    FieldParse {
        token: "ScuttleDelay",
        parse: parse_scuttle_delay,
    },
    FieldParse {
        token: "ScuttleStatus",
        parse: parse_scuttle_status,
    },
];

fn parse_locomotor_set_type(token: &str) -> Result<LocomotorSetType, INIError> {
    let mut name = token.trim().to_ascii_uppercase();
    if let Some(stripped) = name.strip_prefix("SET_") {
        name = stripped.to_string();
    }

    match name.as_str() {
        "NORMAL" => Ok(LocomotorSetType::Normal),
        "NORMAL_UPGRADED" => Ok(LocomotorSetType::NormalUpgraded),
        "FREEFALL" => Ok(LocomotorSetType::Freefall),
        "WANDER" => Ok(LocomotorSetType::Wander),
        "PANIC" => Ok(LocomotorSetType::Panic),
        "TAXIING" => Ok(LocomotorSetType::Taxiing),
        "SUPERSONIC" => Ok(LocomotorSetType::Supersonic),
        "SLUGGISH" => Ok(LocomotorSetType::Sluggish),
        _ => Err(INIError::InvalidData),
    }
}

fn parse_weapon_set_type(token: &str) -> Result<WeaponSetType, INIError> {
    let mut name = token.trim().to_ascii_uppercase();
    if let Some(stripped) = name.strip_prefix("WEAPONSET_") {
        name = stripped.to_string();
    }

    match name.as_str() {
        "VETERAN" => Ok(WeaponSetType::Veteran),
        "ELITE" => Ok(WeaponSetType::Elite),
        "HERO" => Ok(WeaponSetType::Hero),
        "PLAYER_UPGRADE" => Ok(WeaponSetType::PlayerUpgrade),
        "CRATEUPGRADE_ONE" | "CRATE_UPGRADE_ONE" => Ok(WeaponSetType::CrateUpgradeOne),
        "CRATEUPGRADE_TWO" | "CRATE_UPGRADE_TWO" => Ok(WeaponSetType::CrateUpgradeTwo),
        "VEHICLE_HIJACK" => Ok(WeaponSetType::VehicleHijack),
        "CARBOMB" | "CAR_BOMB" => Ok(WeaponSetType::CarBomb),
        "MINE_CLEARING_DETAIL" => Ok(WeaponSetType::MineClearingDetail),
        "RIDER1" => Ok(WeaponSetType::WeaponRider1),
        "RIDER2" => Ok(WeaponSetType::WeaponRider2),
        "RIDER3" => Ok(WeaponSetType::WeaponRider3),
        "RIDER4" => Ok(WeaponSetType::WeaponRider4),
        "RIDER5" => Ok(WeaponSetType::WeaponRider5),
        "RIDER6" => Ok(WeaponSetType::WeaponRider6),
        "RIDER7" => Ok(WeaponSetType::WeaponRider7),
        "RIDER8" => Ok(WeaponSetType::WeaponRider8),
        _ => Err(INIError::InvalidData),
    }
}

fn parse_object_status(token: &str) -> Result<ObjectStatusMaskType, INIError> {
    let mut name = token.trim();
    if let Some(stripped) = name.strip_prefix("OBJECT_STATUS_") {
        name = stripped;
    } else if let Some(stripped) = name.strip_prefix("STATUS_") {
        name = stripped;
    }

    ObjectStatusMaskType::from_case_insensitive_name(name).ok_or(INIError::InvalidData)
}

fn parse_model_condition_flag(token: &str) -> Result<ModelConditionFlags, INIError> {
    let mut name = token.trim().to_ascii_uppercase();
    if let Some(stripped) = name.strip_prefix("MODELCONDITION_") {
        name = stripped.to_string();
    }

    match name.as_str() {
        "TOPPLED" => Ok(ModelConditionFlags::TOPPLED),
        "RIDER1" => Ok(ModelConditionFlags::RIDER1),
        "RIDER2" => Ok(ModelConditionFlags::RIDER2),
        "RIDER3" => Ok(ModelConditionFlags::RIDER3),
        "RIDER4" => Ok(ModelConditionFlags::RIDER4),
        "RIDER5" => Ok(ModelConditionFlags::RIDER5),
        "RIDER6" => Ok(ModelConditionFlags::RIDER6),
        "RIDER7" => Ok(ModelConditionFlags::RIDER7),
        "RIDER8" => Ok(ModelConditionFlags::RIDER8),
        _ => Err(INIError::InvalidData),
    }
}

/// Rider change contain module - can transform contained units
#[derive(Debug)]
pub struct RiderChangeContain {
    /// Base functionality from TransportContain
    pub base: TransportContain,
    /// Reference to the owning object
    object: Weak<RwLock<Object>>,
    /// Module configuration
    module_data: RiderChangeContainModuleData,
    /// Frame when scuttling started
    scuttled_on_frame: u32,
    /// Whether we are currently replacing a rider
    containing: bool,
}

impl RiderChangeContain {
    /// Create a new RiderChangeContain module
    pub fn new(
        object: Weak<RwLock<Object>>,
        module_data: &RiderChangeContainModuleData,
    ) -> GameResult<Self> {
        let base = TransportContain::new(object.clone(), &module_data.base)?;

        Ok(Self {
            base,
            object,
            module_data: module_data.clone(),
            scuttled_on_frame: 0,
            containing: false,
        })
    }

    /// Check if this is a rider change container
    pub fn is_rider_change_contain(&self) -> bool {
        true
    }

    /// Transform riders to new type
    pub fn change_riders(&mut self, new_template: &str) -> GameResult<()> {
        // Implementation would transform contained units
        let _ = new_template;
        Ok(())
    }

    pub fn is_valid_container_for(&self, rider: &Object, check_capacity: bool) -> bool {
        if !self.base.is_valid_container_for(rider, false) {
            return false;
        }

        if self.scuttled_on_frame != 0 {
            return false;
        }

        for rider_info in &self.module_data.riders {
            if rider_info.template_name.is_empty() {
                continue;
            }

            let Some(template) = TheThingFactory::find_template(rider_info.template_name.as_str())
            else {
                continue;
            };

            if template.is_equivalent_to(rider.get_template().as_ref()) {
                return !check_capacity || self.base.is_valid_container_for(rider, true);
            }
        }

        false
    }

    pub fn add_to_contain(
        &mut self,
        rider: Arc<RwLock<Object>>,
        was_selected: bool,
    ) -> GameResult<()> {
        let rider_guard = rider.read().map_err(|_| "Rider lock poisoned")?;
        if !self.is_valid_container_for(&*rider_guard, true) {
            return Err("Object not valid for this rider change container".into());
        }
        drop(rider_guard);

        self.base.add_to_contain_list(rider.clone())?;
        self.on_containing(rider, was_selected)?;
        Ok(())
    }

    pub fn remove_from_contain(
        &mut self,
        rider: Arc<RwLock<Object>>,
        expose_stealth_units: bool,
    ) -> GameResult<()> {
        let rider_id = rider.read().map_err(|_| "Rider lock poisoned")?.get_id();

        if let Some(pos) = self
            .base
            .base
            .get_contained_items_list()?
            .iter()
            .position(|obj| Arc::ptr_eq(obj, &rider))
        {
            let _ = pos;
            self.base.base.remove_from_contain_list(rider_id);
            self.on_removing(rider)?;
        }

        let _ = expose_stealth_units;
        Ok(())
    }

    pub fn on_containing(
        &mut self,
        rider: Arc<RwLock<Object>>,
        was_selected: bool,
    ) -> GameResult<()> {
        self.containing = true;

        let contained_items = self.base.base.get_contained_items_list()?;
        for existing in contained_items {
            if Arc::ptr_eq(&existing, &rider) {
                continue;
            }
            let _ = self.base.remove_from_contain(existing, true);
        }

        let rider_template = rider
            .read()
            .map_err(|_| "Rider lock poisoned")?
            .get_template()
            .clone();

        if let Some(owner) = self.object.upgrade() {
            if let Ok(mut owner_guard) = owner.write() {
                for rider_info in &self.module_data.riders {
                    if rider_info.template_name.is_empty() {
                        continue;
                    }

                    let Some(template) =
                        TheThingFactory::find_template(rider_info.template_name.as_str())
                    else {
                        continue;
                    };

                    if template.is_equivalent_to(rider_template.as_ref()) {
                        owner_guard.set_model_condition_state(rider_info.model_condition_flag);
                        owner_guard.set_weapon_set_flag(rider_info.weapon_set_flag);
                        owner_guard.set_status(rider_info.object_status, true);
                        owner_guard.set_command_set_string_override(&rider_info.command_set);

                        if let Some(ai) = owner_guard.get_ai() {
                            let _ = ai
                                .lock()
                                .map_err(|_| "AI lock poisoned")?
                                .choose_locomotor_set(rider_info.locomotor_set);
                        }

                        if owner_guard.test_status(ObjectStatusTypes::Stealthed) {
                            if let Some(stealth) = owner_guard.get_stealth() {
                                if let Ok(mut stealth_guard) = stealth.lock() {
                                    stealth_guard.mark_as_detected();
                                }
                            }
                        }

                        if let (Some(rider_tracker), Some(owner_tracker)) = (
                            rider_guard_experience(&rider),
                            owner_guard.get_experience_tracker(),
                        ) {
                            if let (Ok(mut rider_tracker), Ok(mut owner_tracker)) =
                                (rider_tracker.lock(), owner_tracker.lock())
                            {
                                owner_tracker
                                    .set_veterancy_level(rider_tracker.get_veterancy_level());
                                let _ = rider_tracker.set_experience_and_level(
                                    0,
                                    &crate::experience::ExperienceTracker::DEFAULT_EXPERIENCE_REQUIRED,
                                );
                            }
                        }

                        break;
                    }
                }
            }
        }

        self.base.on_containing(rider, was_selected)?;
        self.containing = false;
        Ok(())
    }

    pub fn on_removing(&mut self, rider: Arc<RwLock<Object>>) -> GameResult<()> {
        if let Some(owner) = self.object.upgrade() {
            if let Ok(owner_guard) = owner.read() {
                if owner_guard.is_effectively_dead() {
                    let rider_guard = rider.read().map_err(|_| "Rider lock poisoned")?;
                    let _ = TheGameLogic::destroy_object(&*rider_guard);
                    return Ok(());
                }
            }
        }

        self.base.on_removing(rider.clone())?;

        let rider_template = rider
            .read()
            .map_err(|_| "Rider lock poisoned")?
            .get_template()
            .clone();

        if let Some(owner) = self.object.upgrade() {
            if let Ok(mut owner_guard) = owner.write() {
                for rider_info in &self.module_data.riders {
                    if rider_info.template_name.is_empty() {
                        continue;
                    }

                    let Some(template) =
                        TheThingFactory::find_template(rider_info.template_name.as_str())
                    else {
                        continue;
                    };

                    if template.is_equivalent_to(rider_template.as_ref()) {
                        let _ = owner_guard.clear_model_condition_flags(
                            rider_info.model_condition_flag | ModelConditionFlags::DOOR_1_CLOSING,
                        );
                        owner_guard.clear_weapon_set_flag(rider_info.weapon_set_flag);
                        owner_guard.set_status(rider_info.object_status, false);

                        if let (Some(rider_tracker), Some(owner_tracker)) = (
                            rider_guard_experience(&rider),
                            owner_guard.get_experience_tracker(),
                        ) {
                            if let (Ok(mut rider_tracker), Ok(mut owner_tracker)) =
                                (rider_tracker.lock(), owner_tracker.lock())
                            {
                                rider_tracker
                                    .set_veterancy_level(owner_tracker.get_veterancy_level());
                                let _ = owner_tracker.set_experience_and_level(
                                    0,
                                    &crate::experience::ExperienceTracker::DEFAULT_EXPERIENCE_REQUIRED,
                                );
                            }
                        }

                        break;
                    }
                }

                if !self.containing {
                    self.scuttled_on_frame = TheGameLogic::get_frame();
                    owner_guard.set_status(
                        ObjectStatusMaskType::from_status(ObjectStatusTypes::Unselectable),
                        true,
                    );
                    owner_guard.set_model_condition_state(self.module_data.scuttle_state);

                    if let Some(ai) = owner_guard.get_ai() {
                        if let Ok(ai_guard) = ai.lock() {
                            if !ai_guard.is_moving() {
                                owner_guard.set_status(
                                    ObjectStatusMaskType::from_status(ObjectStatusTypes::Immobile),
                                    true,
                                );
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub fn update(&mut self) -> GameResult<()> {
        if self.scuttled_on_frame != 0 {
            let now = TheGameLogic::get_frame();
            if self.scuttled_on_frame + self.module_data.scuttle_frames <= now {
                if let Some(owner) = self.object.upgrade() {
                    if let Ok(mut owner_guard) = owner.write() {
                        owner_guard.kill(Some(DamageType::Unresistable), Some(DeathType::Toppled));
                    }
                }
            }
        }

        let _ = self.base.update();
        Ok(())
    }

    /// Handle capture event (inherits TransportContain capture behavior).
    /// Matches C++ RiderChangeContain::onCapture.
    pub fn on_capture(
        &mut self,
        owner: &Object,
        old_owner: Option<&Arc<RwLock<crate::player::Player>>>,
        new_owner: Option<&Arc<RwLock<crate::player::Player>>>,
    ) -> GameResult<()> {
        self.base.on_capture(owner, old_owner, new_owner)
    }

    /// Serialize state for save/load
    pub fn save_state(&self) -> GameResult<HashMap<String, Vec<u8>>> {
        self.base.save_state()
    }

    /// Deserialize state for save/load
    pub fn load_state(&mut self, state: &HashMap<String, Vec<u8>>) -> GameResult<()> {
        self.base.load_state(state)
    }
}

impl ContainModuleInterface for RiderChangeContain {
    fn can_contain(&self, object_id: ObjectID) -> bool {
        if let Some(obj) = TheGameLogic::find_object_by_id(object_id) {
            if let Ok(obj_guard) = obj.read() {
                return self.is_valid_container_for(&*obj_guard, true);
            }
        }
        false
    }

    fn contain_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        let obj = TheGameLogic::find_object_by_id(object_id)
            .ok_or_else(|| format!("Contain object {} not found", object_id))?;
        self.add_to_contain(obj, false).map_err(|e| e.to_string())
    }

    fn release_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        let obj = match TheGameLogic::find_object_by_id(object_id) {
            Some(obj) => obj,
            None => return Ok(()),
        };
        self.remove_from_contain(obj, false)
            .map_err(|e| e.to_string())
    }

    fn get_contained_objects(&self) -> &[ObjectID] {
        ContainModuleInterface::get_contained_objects(&self.base)
    }

    fn get_contained_count(&self) -> usize {
        ContainModuleInterface::get_contained_count(&self.base)
    }

    fn get_player_who_entered(&self) -> PlayerMaskType {
        self.base.get_player_who_entered()
    }

    fn get_max_capacity(&self) -> usize {
        let max = self.base.get_contain_max();
        if max < 0 {
            usize::MAX
        } else {
            max as usize
        }
    }

    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        RiderChangeContain::update(self)?;
        Ok(UpdateSleepTime::Frames(1))
    }

    fn on_damage(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_damage(damage_info).map_err(|e| e.into())
    }

    fn on_die(
        &mut self,
        damage_info: Option<&DamageInfo>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_die(damage_info).map_err(|e| e.into())
    }

    fn is_valid_container_for(&self, obj: &Object, check_capacity: bool) -> bool {
        self.is_valid_container_for(obj, check_capacity)
    }

    fn add_to_contain(
        &mut self,
        obj: &Object,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.contain_object(obj.get_id()).map_err(|e| e.into())
    }

    fn enable_load_sounds(
        &mut self,
        enabled: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.base.enable_load_sounds(enabled);
        Ok(())
    }

    fn on_object_wants_to_enter_or_exit(
        &mut self,
        obj: &Object,
        want: ContainWant,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base
            .on_object_wants_to_enter_or_exit(obj, want)
            .map_err(|e| e.into())
    }

    fn on_capture(
        &mut self,
        owner: &Object,
        old_owner: Option<&Arc<RwLock<crate::player::Player>>>,
        new_owner: Option<&Arc<RwLock<crate::player::Player>>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        RiderChangeContain::on_capture(self, owner, old_owner, new_owner).map_err(|e| e.into())
    }

    fn set_passenger_allowed_to_fire(&mut self, allowed: bool) {
        self.base.set_passenger_allowed_to_fire(allowed);
    }

    fn harm_and_force_exit_all_contained(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base
            .harm_and_force_exit_all_contained(damage_info)
            .map_err(|e| e.into())
    }

    fn kill_all_contained(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.kill_all_contained().map_err(|e| e.into())
    }
}

impl ContainerInterface for RiderChangeContain {
    fn can_contain(&self, obj: &Object) -> bool {
        ContainerInterface::can_contain(&self.base, obj)
    }

    fn add_object(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()> {
        self.add_to_contain(obj, false)
    }

    fn remove_object(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()> {
        self.remove_from_contain(obj, false)
    }

    fn get_usage(&self) -> (u32, u32) {
        self.base.get_usage()
    }
}

fn rider_guard_experience(
    rider: &Arc<RwLock<Object>>,
) -> Option<Arc<Mutex<crate::common::ExperienceTracker>>> {
    let guard = rider.read().ok()?;
    guard.get_experience_tracker()
}
