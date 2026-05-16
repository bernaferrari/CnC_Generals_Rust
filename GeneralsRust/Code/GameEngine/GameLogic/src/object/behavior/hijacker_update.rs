//! HijackerUpdate - Rust conversion of C++ HijackerUpdate
//!
//! Allows hijacker to stay with hijacked vehicle until it dies.
//! Author: Mark Lorenzen, July 2002 (C++ version)
//! Rust conversion: 2025

use crate::common::xfer::XferExt;
use crate::common::{Bool, Coord3D, ModuleData, ObjectID, UnsignedInt};
use crate::helpers::TheGameLogic;
use crate::modules::{BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::{Object as GameObject, INVALID_ID as OBJECT_INVALID_ID};
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::HijackerControlInterface;
use std::sync::{Arc, RwLock, Weak};

const UPDATE_SLEEP_FOREVER: UpdateSleepTime = UpdateSleepTime::Forever;

#[derive(Clone, Debug)]
pub struct HijackerUpdateModuleData {
    pub base: BehaviorModuleData,
    pub attach_to_bone: String,
    pub parachute_name: String,
}

impl Default for HijackerUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            attach_to_bone: String::new(),
            parachute_name: String::new(),
        }
    }
}

crate::impl_behavior_module_data_via_base!(HijackerUpdateModuleData, base);

impl HijackerUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, HIJACKER_UPDATE_FIELDS)
    }
}

fn parse_attach_to_target_bone(
    _ini: &mut INI,
    data: &mut HijackerUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)?;
    data.attach_to_bone = token.to_string();
    Ok(())
}

fn parse_parachute_name(
    _ini: &mut INI,
    data: &mut HijackerUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)?;
    data.parachute_name = token.to_string();
    Ok(())
}

const HIJACKER_UPDATE_FIELDS: &[FieldParse<HijackerUpdateModuleData>] = &[
    FieldParse {
        token: "AttachToTargetBone",
        parse: parse_attach_to_target_bone,
    },
    FieldParse {
        token: "ParachuteName",
        parse: parse_parachute_name,
    },
];

pub struct HijackerUpdate {
    object: Weak<RwLock<GameObject>>,
    #[allow(dead_code)]
    module_data: Arc<HijackerUpdateModuleData>,
    /// UpdateModule scheduler state serialized by the C++ base class.
    next_call_frame_and_phase: UnsignedInt,
    target_id: ObjectID,
    eject_pos: Coord3D,
    update: Bool,
    is_in_vehicle: Bool,
    was_target_airborne: Bool,
}

impl HijackerUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .downcast_ref::<HijackerUpdateModuleData>()
            .ok_or("Invalid module data")?;

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            next_call_frame_and_phase: 0,
            target_id: OBJECT_INVALID_ID,
            eject_pos: Coord3D {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            update: false,
            is_in_vehicle: false,
            was_target_airborne: false,
        })
    }

    pub fn set_target_object(&mut self, target_id: ObjectID) {
        self.target_id = target_id;
    }

    pub fn get_target_object(&self) -> ObjectID {
        self.target_id
    }

    pub fn set_update(&mut self, update: Bool) {
        self.update = update;
    }

    pub fn set_is_in_vehicle(&mut self, is_in_vehicle: Bool) {
        self.is_in_vehicle = is_in_vehicle;
    }
}

impl UpdateModuleInterface for HijackerUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        if !self.update {
            return UPDATE_SLEEP_FOREVER;
        }

        if self.target_id != OBJECT_INVALID_ID {
            let mut target_destroyed = true;
            if let Some(target_arc) = TheGameLogic::find_object_by_id(self.target_id) {
                if let Ok(target_guard) = target_arc.read() {
                    target_destroyed = target_guard.is_destroyed();
                    self.was_target_airborne = target_guard.is_airborne_target();
                }
            }

            if target_destroyed {
                if let Some(hijacker_arc) = self.object.upgrade() {
                    if let Ok(hijacker_guard) = hijacker_arc.read() {
                        if let Some(container_arc) = hijacker_guard.get_container() {
                            if let Ok(container_guard) = container_arc.read() {
                                if let Some(contain_arc) = container_guard.get_contain() {
                                    if let Ok(mut contain_guard) = contain_arc.lock() {
                                        let _ =
                                            contain_guard.release_object(hijacker_guard.get_id());
                                    }
                                }
                            }
                        }
                    }
                }

                self.target_id = OBJECT_INVALID_ID;
                self.is_in_vehicle = false;
                self.update = false;
                return UPDATE_SLEEP_FOREVER;
            }
        }

        if self.is_in_vehicle {
            return UpdateSleepTime::Frames(15); // Check periodically while in vehicle
        }

        UPDATE_SLEEP_FOREVER
    }
}

impl BehaviorModuleInterface for HijackerUpdate {
    fn get_module_name(&self) -> &'static str {
        "HijackerUpdate"
    }
    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_hijacker_control_interface(&mut self) -> Option<&mut dyn HijackerControlInterface> {
        Some(self)
    }
}

impl HijackerControlInterface for HijackerUpdate {
    fn configure_hijacked_vehicle(&mut self, target_id: ObjectID) {
        self.set_target_object(target_id);
        self.set_update(true);
        self.set_is_in_vehicle(true);
    }
}

impl Snapshotable for HijackerUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("HijackerUpdate xfer version: {:?}", e))?;
        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;
        xfer.xfer_object_id(&mut self.target_id)
            .map_err(|e| format!("HijackerUpdate xfer target_id: {:?}", e))?;
        xfer.xfer_coord3d(&mut self.eject_pos);
        xfer.xfer_bool(&mut self.update)
            .map_err(|e| format!("HijackerUpdate xfer update: {:?}", e))?;
        xfer.xfer_bool(&mut self.is_in_vehicle)
            .map_err(|e| format!("HijackerUpdate xfer is_in_vehicle: {:?}", e))?;
        xfer.xfer_bool(&mut self.was_target_airborne)
            .map_err(|e| format!("HijackerUpdate xfer was_target_airborne: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

pub struct HijackerUpdateFactory;
impl HijackerUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(HijackerUpdate::new(thing, module_data)?))
    }
}
