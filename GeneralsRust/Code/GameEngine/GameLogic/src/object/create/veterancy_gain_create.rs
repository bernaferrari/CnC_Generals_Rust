//! VeterancyGainCreate module - Grants initial veterancy level on creation
//!
//! C++ Source: GameLogic/Object/Create/VeterancyGainCreate.cpp

use std::sync::Arc;

use crate::common::VeterancyLevel;
use crate::experience::ExperienceTracker;
use crate::helpers::TheGameLogic;
use crate::object::create::{CreateModule, CreateModuleData};
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::rts::{get_science_store, ScienceType, SCIENCE_INVALID};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{CreateInterface, ModuleData, Thing as ThingTrait};

/// Data structure for VeterancyGainCreate module
#[derive(Debug, Clone)]
pub struct VeterancyGainCreateModuleData {
    pub base: CreateModuleData,
    pub starting_level: VeterancyLevel,
    pub science_required: ScienceType,
}

impl Default for VeterancyGainCreateModuleData {
    fn default() -> Self {
        Self {
            base: CreateModuleData::new(),
            starting_level: VeterancyLevel::Regular,
            science_required: SCIENCE_INVALID,
        }
    }
}

impl VeterancyGainCreateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, VETERANCY_GAIN_CREATE_FIELDS)
    }
}

impl ModuleData for VeterancyGainCreateModuleData {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: game_engine::common::thing::module::NameKeyType) {
        ModuleData::set_module_tag_name_key(&mut self.base, key);
    }

    fn get_module_tag_name_key(&self) -> game_engine::common::thing::module::NameKeyType {
        ModuleData::get_module_tag_name_key(&self.base)
    }
}

impl Snapshotable for VeterancyGainCreateModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

/// VeterancyGainCreate module implementation
#[derive(Debug)]
pub struct VeterancyGainCreate {
    base: CreateModule,
    module_data: Arc<VeterancyGainCreateModuleData>,
}

impl VeterancyGainCreate {
    pub fn new(
        thing: Arc<dyn ThingTrait>,
        module_data: Arc<VeterancyGainCreateModuleData>,
    ) -> Self {
        Self {
            base: CreateModule::new(thing),
            module_data,
        }
    }
}

impl CreateInterface for VeterancyGainCreate {
    fn on_create(&self) {
        let object_id = self
            .base
            .get_thing()
            .as_object()
            .map(|obj| obj.get_object_id())
            .unwrap_or_default();
        if object_id == 0 {
            return;
        }

        let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return;
        };
        let Ok(object_guard) = object_arc.read() else {
            return;
        };
        let Some(player) = object_guard.get_controlling_player() else {
            return;
        };
        let Ok(player_guard) = player.read() else {
            return;
        };

        let science_required = self.module_data.science_required;
        if science_required != SCIENCE_INVALID && !player_guard.has_science(science_required) {
            return;
        }

        if let Some(exp_tracker) = object_guard.get_experience_tracker() {
            if let Ok(mut tracker_guard) = exp_tracker.lock() {
                if tracker_guard.is_trainable() {
                    tracker_guard.set_min_veterancy_level(
                        self.module_data.starting_level,
                        &ExperienceTracker::DEFAULT_EXPERIENCE_REQUIRED,
                    );
                }
            }
        }
    }

    fn on_build_complete(&self) {
        self.base.on_build_complete();
    }

    fn should_do_on_build_complete(&self) -> bool {
        self.base.should_do_on_build_complete()
    }
}

impl Snapshotable for VeterancyGainCreate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

fn parse_starting_level(
    _ini: &mut INI,
    data: &mut VeterancyGainCreateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    let level = match token.trim().to_ascii_uppercase().as_str() {
        "REGULAR" => VeterancyLevel::Regular,
        "VETERAN" => VeterancyLevel::Veteran,
        "ELITE" => VeterancyLevel::Elite,
        "HEROIC" => VeterancyLevel::Heroic,
        _ => return Err(INIError::InvalidData),
    };
    data.starting_level = level;
    Ok(())
}

fn parse_science_required(
    _ini: &mut INI,
    data: &mut VeterancyGainCreateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    let store = get_science_store().ok_or(INIError::InvalidData)?;
    let science = store.get_science_from_internal_name(token.trim());
    data.science_required = science;
    Ok(())
}

const VETERANCY_GAIN_CREATE_FIELDS: &[FieldParse<VeterancyGainCreateModuleData>] = &[
    FieldParse {
        token: "StartingLevel",
        parse: parse_starting_level,
    },
    FieldParse {
        token: "ScienceRequired",
        parse: parse_science_required,
    },
];
