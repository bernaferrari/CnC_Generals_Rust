//! SmartBombTargetHomingUpdate - Rust conversion of C++ SmartBombTargetHomingUpdate
//!
//! Update that nudges a falling object's position slightly toward its target.
//! Used for smart bombs and guided projectiles.
//! Author: Mark Lorenzen, July 2003 (C++ version)
//! Rust conversion: 2025

use crate::common::xfer::XferExt;
use crate::common::{
    AsciiString, Coord3D, ModuleData, ObjectID, Real, TheGameLogic, UnsignedInt, INVALID_ID,
};
use crate::modules::{
    BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime, UPDATE_SLEEP_NONE,
};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::Object as GameObject;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    Module, ModuleData as EngineModuleData, NameKeyType, Object as ModuleObject,
    PayloadTargetControlInterface, Thing as ModuleThing,
};
use log::warn;
use std::sync::{Arc, RwLock, Weak};

/// INI-configurable data for SmartBombTargetHomingUpdate
#[derive(Clone, Debug)]
pub struct SmartBombTargetHomingUpdateModuleData {
    pub base: BehaviorModuleData,
    /// Course correction scalar (0.0-1.0) - higher = more inertia, lower = homes faster
    pub course_correction_scalar: Real,
}

impl Default for SmartBombTargetHomingUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            course_correction_scalar: 0.99, // Match C++ default
        }
    }
}

crate::impl_behavior_module_data_via_base!(SmartBombTargetHomingUpdateModuleData, base);

impl SmartBombTargetHomingUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, SMART_BOMB_TARGET_HOMING_UPDATE_FIELDS)
    }
}

fn parse_course_correction_scalar(
    _ini: &mut INI,
    data: &mut SmartBombTargetHomingUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.course_correction_scalar = INI::parse_real(token)?;
    Ok(())
}

const SMART_BOMB_TARGET_HOMING_UPDATE_FIELDS: &[FieldParse<
    SmartBombTargetHomingUpdateModuleData,
>] = &[FieldParse {
    token: "CourseCorrectionScalar",
    parse: parse_course_correction_scalar,
}];

/// SmartBombTargetHomingUpdate - nudges falling objects toward targets
pub struct SmartBombTargetHomingUpdate {
    object_id: ObjectID,
    module_data: Arc<SmartBombTargetHomingUpdateModuleData>,
    next_call_frame_and_phase: UnsignedInt,

    /// Whether a target has been received
    target_received: bool,
    /// Target position to home towards
    target: Coord3D,
}

impl SmartBombTargetHomingUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let data = module_data
            .as_ref()
            .downcast_ref::<SmartBombTargetHomingUpdateModuleData>()
            .ok_or("Invalid module data")?;

        Ok(Self {
            object_id: object
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID),
            module_data: Arc::new(data.clone()),
            next_call_frame_and_phase: 0,
            target_received: false,
            target: Coord3D::default(),
        })
    }

    /// Set the target position for homing
    pub fn set_target_position(&mut self, target: &Coord3D) {
        // Ensure we have a valid target (non-zero position)
        if target.length() <= 0.0 {
            return;
        }

        self.target = *target;
        self.target_received = true;
    }
}

impl UpdateModuleInterface for SmartBombTargetHomingUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        // No target received yet
        if !self.target_received {
            return UPDATE_SLEEP_NONE;
        }

        // Get object reference
        let obj_arc = match (if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        }) {
            Some(arc) => arc,
            None => return UPDATE_SLEEP_NONE,
        };

        // Check if significantly above terrain
        {
            if let Ok(obj) = obj_arc.read() {
                if !obj.is_significantly_above_terrain() {
                    return UPDATE_SLEEP_NONE;
                }
            }
        }

        // Get current position and calculate new position
        let current_pos = {
            if let Ok(obj) = obj_arc.read() {
                *obj.get_position()
            } else {
                return UPDATE_SLEEP_NONE;
            }
        };

        // Calculate interpolation coefficients
        // status_coeff = how much to keep current position
        // target_coeff = how much to move toward target
        let status_coeff = self.module_data.course_correction_scalar.clamp(0.0, 1.0);
        let target_coeff = 1.0 - status_coeff;

        // Interpolate X and Y toward target, keep Z (altitude) unchanged
        let new_pos = Coord3D::new(
            self.target.x * target_coeff + current_pos.x * status_coeff,
            self.target.y * target_coeff + current_pos.y * status_coeff,
            current_pos.z, // Keep Z unchanged
        );

        // Apply new position
        if let Ok(mut obj) = obj_arc.write() {
            let _ = obj.set_position(&new_pos);
        }

        UPDATE_SLEEP_NONE
    }
}

impl BehaviorModuleInterface for SmartBombTargetHomingUpdate {
    fn get_module_name(&self) -> &'static str {
        "SmartBombTargetHomingUpdate"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
}

impl Snapshotable for SmartBombTargetHomingUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("SmartBombTargetHomingUpdate xfer version failed: {:?}", e))?;

        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Interface for SmartBombTargetHomingUpdate behavior
pub trait SmartBombTargetHomingUpdateInterface {
    fn set_target_position(&mut self, target: &Coord3D);
}

impl SmartBombTargetHomingUpdateInterface for SmartBombTargetHomingUpdate {
    fn set_target_position(&mut self, target: &Coord3D) {
        SmartBombTargetHomingUpdate::set_target_position(self, target);
    }
}

impl PayloadTargetControlInterface for SmartBombTargetHomingUpdate {
    fn set_payload_target_position(&mut self, target: [f32; 3]) {
        self.set_target_position(&Coord3D {
            x: target[0],
            y: target[1],
            z: target[2],
        });
    }
}

pub struct SmartBombTargetHomingUpdateFactory;
impl SmartBombTargetHomingUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(SmartBombTargetHomingUpdate::new(
            thing,
            module_data,
        )?))
    }
}

/// Glue that exposes SmartBombTargetHomingUpdate through the common Module trait.
pub struct SmartBombTargetHomingUpdateModule {
    behavior: SmartBombTargetHomingUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<SmartBombTargetHomingUpdateModuleData>,
}

impl SmartBombTargetHomingUpdateModule {
    pub fn new(
        behavior: SmartBombTargetHomingUpdate,
        module_name: &AsciiString,
        module_data: Arc<SmartBombTargetHomingUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut SmartBombTargetHomingUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for SmartBombTargetHomingUpdateModule {
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

impl Module for SmartBombTargetHomingUpdateModule {
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

    fn get_payload_target_control_interface(
        &mut self,
    ) -> Option<&mut dyn PayloadTargetControlInterface> {
        Some(&mut self.behavior)
    }
}

pub fn smart_bomb_target_homing_update_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn EngineModuleData> {
    let mut data = SmartBombTargetHomingUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse SmartBombTargetHomingUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

pub fn smart_bomb_target_homing_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn EngineModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_any()
        .downcast_ref::<SmartBombTargetHomingUpdateModuleData>()
        .expect("SmartBombTargetHomingUpdateModuleData expected");
    let module_data_arc = Arc::new(typed_data.clone());
    let owner_id = thing
        .as_object()
        .map(ModuleObject::get_object_id)
        .unwrap_or(INVALID_ID);
    let object = TheGameLogic::find_object_by_id(owner_id)
        .expect("SmartBombTargetHomingUpdate requires object");
    let behavior = SmartBombTargetHomingUpdate::new(object, module_data_arc.clone())
        .expect("SmartBombTargetHomingUpdate failed to initialize");
    let module_name = AsciiString::from("SmartBombTargetHomingUpdate");
    Box::new(SmartBombTargetHomingUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}
