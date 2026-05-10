//! LifetimeUpdate - Rust conversion of C++ LifetimeUpdate
//!
//! Object destruction after a lifetime expires.
//! Author: Colin Day, December 2001 (C++ version)
//! Rust conversion: 2025

use crate::common::{AsciiString, ModuleData, TheGameLogic, UnsignedInt, XferVersion, INVALID_ID};
use crate::modules::{BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::Object as GameObject;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    Module, ModuleData as EngineModuleData, NameKeyType, Object as ModuleObject,
    Thing as ModuleThing,
};
use log::warn;
use std::sync::{Arc, RwLock, Weak};

const UPDATE_SLEEP_FOREVER: UpdateSleepTime = UpdateSleepTime::Forever;

#[derive(Clone, Debug)]
pub struct LifetimeUpdateModuleData {
    pub base: BehaviorModuleData,
    pub min_frames: UnsignedInt,
    pub max_frames: UnsignedInt,
}

impl Default for LifetimeUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            min_frames: 0,
            max_frames: 0,
        }
    }
}

impl LifetimeUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, LIFETIME_UPDATE_FIELDS)
    }
}

crate::impl_behavior_module_data_via_base!(LifetimeUpdateModuleData, base);

fn parse_min_lifetime(
    _ini: &mut INI,
    data: &mut LifetimeUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.min_frames = INI::parse_duration_unsigned_int(tokens[0])?;
    Ok(())
}

fn parse_max_lifetime(
    _ini: &mut INI,
    data: &mut LifetimeUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.max_frames = INI::parse_duration_unsigned_int(tokens[0])?;
    Ok(())
}

const LIFETIME_UPDATE_FIELDS: &[FieldParse<LifetimeUpdateModuleData>] = &[
    FieldParse {
        token: "MinLifetime",
        parse: parse_min_lifetime,
    },
    FieldParse {
        token: "MaxLifetime",
        parse: parse_max_lifetime,
    },
];

pub struct LifetimeUpdate {
    object: Weak<RwLock<GameObject>>,
    #[allow(dead_code)]
    module_data: Arc<LifetimeUpdateModuleData>,
    next_call_frame_and_phase: UnsignedInt,
    die_frame: UnsignedInt,
}

impl LifetimeUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .downcast_ref::<LifetimeUpdateModuleData>()
            .ok_or("Invalid module data")?;

        // Get current frame from game logic - matches C++ LifetimeUpdate.cpp
        let current_frame = crate::helpers::TheGameLogic::get_frame();
        let die_frame = current_frame
            + Self::calc_sleep_delay_static(specific_data.min_frames, specific_data.max_frames);

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            next_call_frame_and_phase: 0,
            die_frame,
        })
    }

    pub fn set_lifetime_range(&mut self, min_frames: UnsignedInt, max_frames: UnsignedInt) {
        // Get current frame from game logic - matches C++ LifetimeUpdate.cpp
        let current_frame = crate::helpers::TheGameLogic::get_frame();
        self.die_frame = current_frame + Self::calc_sleep_delay_static(min_frames, max_frames);
    }

    pub fn get_die_frame(&self) -> UnsignedInt {
        self.die_frame
    }

    /// Calculate random sleep delay between min and max frames
    /// C++ Reference: LifetimeUpdate.cpp - uses GameLogicRandomValue
    fn calc_sleep_delay_static(min_frames: UnsignedInt, max_frames: UnsignedInt) -> UnsignedInt {
        let mut delay = crate::GameLogicRandomValue!(min_frames, max_frames) as UnsignedInt;
        if delay < 1 {
            delay = 1;
        }
        delay
    }

    /// Get remaining frames until death
    pub fn get_remaining_frames(&self) -> UnsignedInt {
        let current_frame = crate::helpers::TheGameLogic::get_frame();
        self.die_frame.saturating_sub(current_frame)
    }

    /// Check if the object has expired
    pub fn is_expired(&self) -> bool {
        let current_frame = crate::helpers::TheGameLogic::get_frame();
        current_frame >= self.die_frame
    }
}

impl UpdateModuleInterface for LifetimeUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        // Get current frame from game logic - matches C++ LifetimeUpdate.cpp
        let current_frame = crate::helpers::TheGameLogic::get_frame();

        if current_frame >= self.die_frame {
            // Destroy object through game logic
            if let Some(object) = self.object.upgrade() {
                if let Ok(mut guard) = object.write() {
                    guard.kill(None, None);
                }
            }
            return UPDATE_SLEEP_FOREVER;
        }

        // Sleep until die frame
        UpdateSleepTime::from_u32(self.die_frame - current_frame)
    }
}

impl BehaviorModuleInterface for LifetimeUpdate {
    fn get_module_name(&self) -> &'static str {
        "LifetimeUpdate"
    }
    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
}

impl Snapshotable for LifetimeUpdate {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // version -- C++ LifetimeUpdate.cpp line 96: currentVersion = 1
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("LifetimeUpdate version xfer failed: {:?}", e))?;

        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;

        // die frame -- C++ LifetimeUpdate.cpp line 104
        xfer.xfer_unsigned_int(&mut self.die_frame)
            .map_err(|e| format!("LifetimeUpdate die_frame xfer failed: {:?}", e))?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Glue that exposes LifetimeUpdate through the common Module trait.
pub struct LifetimeUpdateModule {
    behavior: LifetimeUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<LifetimeUpdateModuleData>,
}

impl LifetimeUpdateModule {
    pub fn new(
        behavior: LifetimeUpdate,
        module_name: &AsciiString,
        module_data: Arc<LifetimeUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut LifetimeUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for LifetimeUpdateModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.behavior.load_post_process()
    }
}

impl Module for LifetimeUpdateModule {
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
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn EngineModuleData {
        self.module_data.as_ref()
    }
}

pub struct LifetimeUpdateFactory;
impl LifetimeUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(LifetimeUpdate::new(thing, module_data)?))
    }
}

pub fn lifetime_update_data_factory(ini: Option<&mut INI>) -> Box<dyn EngineModuleData> {
    let mut data = LifetimeUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse LifetimeUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

pub fn lifetime_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn EngineModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_any()
        .downcast_ref::<LifetimeUpdateModuleData>()
        .expect("LifetimeUpdateModuleData expected");
    let module_data_arc = Arc::new(typed_data.clone());
    let owner_id = thing
        .as_object()
        .map(ModuleObject::get_object_id)
        .unwrap_or(INVALID_ID);
    let object =
        TheGameLogic::find_object_by_id(owner_id).expect("LifetimeUpdate requires a valid object");
    let behavior = LifetimeUpdate::new(object, module_data_arc.clone())
        .expect("Failed to create LifetimeUpdate");
    let module_name = AsciiString::from("LifetimeUpdate");
    Box::new(LifetimeUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}
