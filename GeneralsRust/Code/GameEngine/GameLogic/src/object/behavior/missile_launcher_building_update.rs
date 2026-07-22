//! MissileLauncherBuildingUpdate - Rust conversion of C++ MissileLauncherBuildingUpdate
//!
//! Update module that manages building animations/states (doors) based on
//! special power readiness (e.g. Scud Storm, Nuclear Missile).
//! Author: Matthew D. Campbell, April 2002 (C++ version)

use crate::common::audio::AudioEventRts;
use crate::common::types::{
    ModelConditionFlags, MODELCONDITION_DOOR_1_CLOSING, MODELCONDITION_DOOR_1_OPENING,
    MODELCONDITION_DOOR_1_WAITING_OPEN, MODELCONDITION_DOOR_1_WAITING_TO_CLOSE,
    OBJECT_STATUS_UNDER_CONSTRUCTION,
};
/// Rust conversion: 2025
use crate::common::{AsciiString, Coord3D, ModuleData, ObjectID, Real, UnsignedInt};
use crate::helpers::TheFXList;
use crate::helpers::{TheAudio, TheGameLogic};
use crate::modules::{
    BehaviorModuleInterface, SpecialPowerModuleInterface, SpecialPowerUpdateInterface,
    UpdateModuleInterface, UpdateSleepTime, UPDATE_SLEEP_NONE,
};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::Object as GameObject;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData as EngineModuleData, NameKeyType};
use std::sync::{Arc, RwLock, Weak};

/// Door states for building animations
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoorStateType {
    Closed,
    Opening,
    Open,
    WaitingToClose,
    Closing,
}

/// INI-configurable data for MissileLauncherBuildingUpdate
#[derive(Clone, Debug)]
pub struct MissileLauncherBuildingUpdateModuleData {
    pub base: BehaviorModuleData,
    pub special_power_template_name: String,
    pub door_open_time: u32,
    pub door_wait_open_time: u32,
    pub door_closing_time: u32,

    pub opening_fx: Option<String>,
    pub open_fx: Option<String>,
    pub waiting_to_close_fx: Option<String>,
    pub closing_fx: Option<String>,
    pub closed_fx: Option<String>,
    pub open_idle_audio: Option<String>,
}

impl Default for MissileLauncherBuildingUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            special_power_template_name: String::new(),
            door_open_time: 0,
            door_wait_open_time: 0,
            door_closing_time: 0,
            opening_fx: None,
            open_fx: None,
            waiting_to_close_fx: None,
            closing_fx: None,
            closed_fx: None,
            open_idle_audio: None,
        }
    }
}

crate::impl_behavior_module_data_via_base!(MissileLauncherBuildingUpdateModuleData, base);

impl MissileLauncherBuildingUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, MISSILE_LAUNCHER_BUILDING_UPDATE_FIELDS)
    }
}

fn required_value<'a>(tokens: &'a [&'a str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

const MISSILE_LAUNCHER_BUILDING_UPDATE_FIELDS: &[FieldParse<
    MissileLauncherBuildingUpdateModuleData,
>] = &[
    FieldParse {
        token: "SpecialPowerTemplate",
        parse: |_, data, tokens| {
            data.special_power_template_name = required_value(tokens)?.to_string();
            Ok(())
        },
    },
    FieldParse {
        token: "DoorOpenTime",
        parse: |_, data, tokens| {
            data.door_open_time = INI::parse_duration_unsigned_int(required_value(tokens)?)?;
            Ok(())
        },
    },
    FieldParse {
        token: "DoorWaitOpenTime",
        parse: |_, data, tokens| {
            data.door_wait_open_time = INI::parse_duration_unsigned_int(required_value(tokens)?)?;
            Ok(())
        },
    },
    FieldParse {
        token: "DoorCloseTime",
        parse: |_, data, tokens| {
            data.door_closing_time = INI::parse_duration_unsigned_int(required_value(tokens)?)?;
            Ok(())
        },
    },
    FieldParse {
        token: "DoorOpeningFX",
        parse: |_, data, tokens| {
            data.opening_fx = Some(required_value(tokens)?.to_string());
            Ok(())
        },
    },
    FieldParse {
        token: "DoorOpenFX",
        parse: |_, data, tokens| {
            data.open_fx = Some(required_value(tokens)?.to_string());
            Ok(())
        },
    },
    FieldParse {
        token: "DoorWaitingToCloseFX",
        parse: |_, data, tokens| {
            data.waiting_to_close_fx = Some(required_value(tokens)?.to_string());
            Ok(())
        },
    },
    FieldParse {
        token: "DoorClosingFX",
        parse: |_, data, tokens| {
            data.closing_fx = Some(required_value(tokens)?.to_string());
            Ok(())
        },
    },
    FieldParse {
        token: "DoorClosedFX",
        parse: |_, data, tokens| {
            data.closed_fx = Some(required_value(tokens)?.to_string());
            Ok(())
        },
    },
    FieldParse {
        token: "DoorOpenIdleAudio",
        parse: |_, data, tokens| {
            data.open_idle_audio = Some(required_value(tokens)?.to_string());
            Ok(())
        },
    },
];

/// MissileLauncherBuildingUpdate - manages building doors for special powers
pub struct MissileLauncherBuildingUpdate {
    object_id: ObjectID,
    module_data: Arc<MissileLauncherBuildingUpdateModuleData>,

    next_call_frame_and_phase: UnsignedInt,
    door_state: DoorStateType,
    timeout_state: DoorStateType,
    timeout_frame: u32,
    open_idle_audio: AudioEventRts,
}

impl MissileLauncherBuildingUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let data = module_data
            .as_ref()
            .downcast_ref::<MissileLauncherBuildingUpdateModuleData>()
            .ok_or("Invalid module data")?;

        let mut audio = AudioEventRts::default();
        if let Some(audio_name) = &data.open_idle_audio {
            audio.set_event_name(audio_name.clone());
        }

        Ok(Self {
            object_id: object
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID),
            module_data: Arc::new(data.clone()),
            next_call_frame_and_phase: 0,
            door_state: DoorStateType::Closed,
            timeout_state: DoorStateType::Closed,
            timeout_frame: 0,
            open_idle_audio: audio,
        })
    }

    fn switch_to_state(&mut self, dst: DoorStateType, obj: &mut GameObject) {
        if self.door_state == dst {
            return;
        }

        let mut clr = ModelConditionFlags::empty();
        let mut set = ModelConditionFlags::empty();
        let now = TheGameLogic::get_frame();

        match dst {
            DoorStateType::Closed => {
                clr.insert(MODELCONDITION_DOOR_1_WAITING_TO_CLOSE);
                clr.insert(MODELCONDITION_DOOR_1_CLOSING);
                clr.insert(MODELCONDITION_DOOR_1_OPENING);
                clr.insert(MODELCONDITION_DOOR_1_WAITING_OPEN);
                self.timeout_frame = 0;
                self.timeout_state = DoorStateType::Closed;
                if let Some(fx_name) = &self.module_data.closed_fx {
                    if let Some(fx) = TheFXList::get() {
                        fx.do_fx_at_position(fx_name, obj.get_position());
                    }
                }
                self.stop_idle_audio();
            }
            DoorStateType::Opening => {
                clr.insert(MODELCONDITION_DOOR_1_WAITING_TO_CLOSE);
                clr.insert(MODELCONDITION_DOOR_1_CLOSING);
                clr.insert(MODELCONDITION_DOOR_1_WAITING_OPEN);
                set.insert(MODELCONDITION_DOOR_1_OPENING);

                // End it one frame before ready
                let ready_frame = self.get_power_ready_frame();
                self.timeout_frame = if ready_frame > 0 {
                    ready_frame - 1
                } else {
                    now
                };
                self.timeout_state = DoorStateType::Open;

                if let Some(fx_name) = &self.module_data.opening_fx {
                    if let Some(fx) = TheFXList::get() {
                        fx.do_fx_at_position(fx_name, obj.get_position());
                    }
                }
                self.stop_idle_audio();
            }
            DoorStateType::Open => {
                clr.insert(MODELCONDITION_DOOR_1_WAITING_TO_CLOSE);
                clr.insert(MODELCONDITION_DOOR_1_CLOSING);
                clr.insert(MODELCONDITION_DOOR_1_OPENING);
                set.insert(MODELCONDITION_DOOR_1_WAITING_OPEN);
                self.timeout_frame = 0;
                self.timeout_state = DoorStateType::Open;

                if let Some(fx_name) = &self.module_data.open_fx {
                    if let Some(fx) = TheFXList::get() {
                        fx.do_fx_at_position(fx_name, obj.get_position());
                    }
                }
                self.play_idle_audio(obj.get_id());
            }
            DoorStateType::WaitingToClose => {
                clr.insert(MODELCONDITION_DOOR_1_CLOSING);
                clr.insert(MODELCONDITION_DOOR_1_OPENING);
                clr.insert(MODELCONDITION_DOOR_1_WAITING_OPEN);
                set.insert(MODELCONDITION_DOOR_1_WAITING_TO_CLOSE);
                self.timeout_frame = now + self.module_data.door_wait_open_time;
                self.timeout_state = DoorStateType::Closing;

                if let Some(fx_name) = &self.module_data.waiting_to_close_fx {
                    if let Some(fx) = TheFXList::get() {
                        fx.do_fx_at_position(fx_name, obj.get_position());
                    }
                }
                self.stop_idle_audio();
            }
            DoorStateType::Closing => {
                clr.insert(MODELCONDITION_DOOR_1_WAITING_TO_CLOSE);
                clr.insert(MODELCONDITION_DOOR_1_WAITING_OPEN);
                clr.insert(MODELCONDITION_DOOR_1_OPENING);
                set.insert(MODELCONDITION_DOOR_1_CLOSING);
                self.timeout_frame = now + self.module_data.door_closing_time;

                // Adjust timeout if power recharges faster
                let ready_frame = self.get_power_ready_frame();
                let delta = if ready_frame > now {
                    ready_frame - now
                } else {
                    0
                };
                if self.timeout_frame > now + delta / 2 {
                    self.timeout_frame = now + delta / 2;
                }

                self.timeout_state = DoorStateType::Closed;
                if let Some(fx_name) = &self.module_data.closing_fx {
                    if let Some(fx) = TheFXList::get() {
                        fx.do_fx_at_position(fx_name, obj.get_position());
                    }
                }
                self.stop_idle_audio();
            }
        }

        self.door_state = dst;
        if let Err(err) = obj.clear_and_set_model_condition_flags(clr, set) {
            log::warn!(
                "MissileLauncherBuildingUpdate: failed to update model conditions for object {}: {}",
                obj.get_id(),
                err
            );
        }

        if self.timeout_frame > now {
            if let Some(drawable) = obj.get_drawable() {
                if let Ok(mut guard) = drawable.write() {
                    guard.set_animation_loop_duration(self.timeout_frame - now);
                }
            }
        }
    }

    fn get_power_ready_frame(&self) -> u32 {
        if let Some(obj_arc) = (if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        }) {
            if let Ok(obj) = obj_arc.read() {
                if let Some(frame) = obj.with_special_power_module_interface_by_name(
                    &self.module_data.special_power_template_name,
                    |spm| spm.get_ready_frame(),
                ) {
                    return frame;
                }
            }
        }
        0
    }

    fn is_power_ready(&self) -> bool {
        if let Some(obj_arc) = (if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        }) {
            if let Ok(obj) = obj_arc.read() {
                if let Some(is_ready) = obj.with_special_power_module_interface_by_name(
                    &self.module_data.special_power_template_name,
                    |spm| spm.is_ready(),
                ) {
                    return is_ready;
                }
            }
        }
        false
    }

    fn play_idle_audio(&mut self, object_id: ObjectID) {
        if !self.open_idle_audio.is_currently_playing()
            && !self.open_idle_audio.get_event_name().is_empty()
        {
            self.open_idle_audio.set_object_id(object_id);
            if let Some(audio) = TheAudio::get() {
                let handle = audio.add_audio_event(&self.open_idle_audio);
                self.open_idle_audio.set_playing_handle(handle);
            }
        }
    }

    fn stop_idle_audio(&mut self) {
        if self.open_idle_audio.is_currently_playing() {
            if let Some(audio) = TheAudio::get() {
                audio.remove_audio_event(self.open_idle_audio.get_playing_handle());
                self.open_idle_audio.set_playing_handle(0);
            }
        }
    }
}

impl UpdateModuleInterface for MissileLauncherBuildingUpdate {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        let obj_arc = match (if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        }) {
            Some(arc) => arc,
            None => return Ok(UPDATE_SLEEP_NONE),
        };

        let now = TheGameLogic::get_frame();

        {
            let obj = obj_arc.read().unwrap();
            if obj.test_status(OBJECT_STATUS_UNDER_CONSTRUCTION) {
                return Ok(UPDATE_SLEEP_NONE);
            }
        }

        let has_special_power = if let Ok(obj) = obj_arc.read() {
            obj.with_special_power_module_interface_by_name(
                &self.module_data.special_power_template_name,
                |_| true,
            )
            .unwrap_or(false)
        } else {
            false
        };

        if has_special_power {
            let ready_frame = self.get_power_ready_frame();
            let when_to_start_opening = if ready_frame >= self.module_data.door_open_time {
                ready_frame - self.module_data.door_open_time
            } else {
                0
            };

            let mut obj = obj_arc.write().unwrap();

            // Check timeouts
            if self.timeout_frame != 0 && now > self.timeout_frame {
                let next_state = self.timeout_state;
                self.switch_to_state(next_state, &mut obj);
            }

            // State changes based on readiness
            if self.door_state != DoorStateType::Open && self.is_power_ready() {
                self.switch_to_state(DoorStateType::Open, &mut obj);
            } else if self.door_state == DoorStateType::Closed && now >= when_to_start_opening {
                self.switch_to_state(DoorStateType::Opening, &mut obj);
            }
        }

        Ok(UPDATE_SLEEP_NONE)
    }
}

impl SpecialPowerUpdateInterface for MissileLauncherBuildingUpdate {
    fn update_special_power(
        &mut self,
        _frame_time: f32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    fn is_power_ready(&self) -> bool {
        self.is_power_ready()
    }

    fn initiate_intent_to_do_special_power(
        &mut self,
        _special_power_template: &crate::object::SpecialPowerTemplate,
        _target_obj: Option<ObjectID>,
        _target_pos: Option<&Coord3D>,
        _waypoint: Option<&crate::object::special_power_module::Waypoint>,
        _command_options: crate::modules::SpecialPowerCommandOptions,
    ) -> bool {
        // C++ version asserts template matches, but we'll assume it for now
        let obj_arc = match (if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        }) {
            Some(arc) => arc,
            None => return false,
        };

        if let Ok(mut obj) = obj_arc.write() {
            self.switch_to_state(DoorStateType::WaitingToClose, &mut obj);
        }
        true
    }

    fn is_special_ability(&self) -> bool {
        false
    }

    fn is_special_power(&self) -> bool {
        true
    }

    fn is_active(&self) -> bool {
        self.door_state != DoorStateType::Closed
    }

    fn get_command_option(&self) -> crate::modules::SpecialPowerCommandOptions {
        crate::modules::SpecialPowerCommandOptions::NONE
    }

    fn does_special_power_have_overridable_destination_active(&self) -> bool {
        false
    }

    fn does_special_power_have_overridable_destination(&self) -> bool {
        false
    }

    fn set_special_power_overridable_destination(&mut self, _location: &Coord3D) {}

    fn is_power_currently_in_use(
        &self,
        _command: Option<&crate::command_button::CommandButton>,
    ) -> bool {
        self.door_state != DoorStateType::Closed
    }
}

impl BehaviorModuleInterface for MissileLauncherBuildingUpdate {
    fn get_module_name(&self) -> &'static str {
        "MissileLauncherBuildingUpdate"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_special_power_update_interface(
        &mut self,
    ) -> Option<&mut dyn SpecialPowerUpdateInterface> {
        Some(self)
    }
}

pub struct MissileLauncherBuildingUpdateFactory;
impl MissileLauncherBuildingUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(MissileLauncherBuildingUpdate::new(
            thing,
            module_data,
        )?))
    }
}

pub struct MissileLauncherBuildingUpdateModule {
    behavior: MissileLauncherBuildingUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<MissileLauncherBuildingUpdateModuleData>,
}

impl MissileLauncherBuildingUpdateModule {
    pub fn new(
        behavior: MissileLauncherBuildingUpdate,
        module_name: &AsciiString,
        module_data: Arc<MissileLauncherBuildingUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut MissileLauncherBuildingUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for MissileLauncherBuildingUpdateModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: u8 = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;

        let b = &mut self.behavior;

        xfer_update_module_base_state(xfer, &mut b.next_call_frame_and_phase)?;

        let mut door_state = b.door_state as i32;
        xfer.xfer_int(&mut door_state).map_err(|e| e.to_string())?;
        if xfer.is_reading() {
            b.door_state = match door_state {
                0 => DoorStateType::Closed,
                1 => DoorStateType::Opening,
                2 => DoorStateType::Open,
                3 => DoorStateType::WaitingToClose,
                4 => DoorStateType::Closing,
                _ => DoorStateType::Closed,
            };
        }

        let mut timeout_state = b.timeout_state as i32;
        xfer.xfer_int(&mut timeout_state)
            .map_err(|e| e.to_string())?;
        if xfer.is_reading() {
            b.timeout_state = match timeout_state {
                0 => DoorStateType::Closed,
                1 => DoorStateType::Opening,
                2 => DoorStateType::Open,
                3 => DoorStateType::WaitingToClose,
                4 => DoorStateType::Closing,
                _ => DoorStateType::Closed,
            };
        }

        xfer.xfer_unsigned_int(&mut b.timeout_frame)
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Module for MissileLauncherBuildingUpdateModule {
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
