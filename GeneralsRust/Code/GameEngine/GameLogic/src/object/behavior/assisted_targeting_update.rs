//! AssistedTargetingUpdate - Rust conversion of C++ AssistedTargetingUpdate
//!
//! Handles AI-assisted attacks, laser feedback, and targeting outside normal range.
//! Author: Graham Smallwood, September 2002 (C++ version)
//! Rust conversion: 2025

use crate::ai::{AiCommandParams, AiCommandType, CommandSourceType};
use crate::common::{
    AsciiString, ModuleData, ObjectID, ThingTemplate, UnsignedInt, LOGICFRAMES_PER_SECOND,
};
use crate::helpers::{TheGameClient, TheGameLogic, TheThingFactory};
use crate::modules::{
    AssistedTargetingUpdateInterface, BehaviorModuleInterface, UpdateModuleInterface,
    UpdateSleepTime,
};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::{Object as GameObject, OBJECT_REGISTRY};
use crate::weapon::{WeaponLockType, WeaponSlotType, WeaponStatus};
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{Module, ModuleData as EngineModuleData, NameKeyType};
use std::sync::{Arc, RwLock, Weak};
const FEEDBACK_LASER_LIFE_FRAMES: UnsignedInt = LOGICFRAMES_PER_SECOND / 2;

#[derive(Clone, Debug)]
pub struct AssistedTargetingUpdateModuleData {
    pub base: BehaviorModuleData,
    pub clip_size: i32,
    pub weapon_slot: WeaponSlotType,
    pub laser_from_assisted_name: AsciiString,
    pub laser_to_target_name: AsciiString,
}

impl Default for AssistedTargetingUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            clip_size: 1,
            weapon_slot: WeaponSlotType::Primary,
            laser_from_assisted_name: AsciiString::new(),
            laser_to_target_name: AsciiString::new(),
        }
    }
}

crate::impl_behavior_module_data_via_base!(AssistedTargetingUpdateModuleData, base);

impl AssistedTargetingUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, ASSISTED_TARGETING_UPDATE_FIELDS)
    }
}

pub struct AssistedTargetingUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<AssistedTargetingUpdateModuleData>,
    /// UpdateModule scheduler state serialized by the C++ base class.
    next_call_frame_and_phase: UnsignedInt,
    laser_from_assisted: Option<Arc<dyn ThingTemplate>>,
    laser_to_target: Option<Arc<dyn ThingTemplate>>,
    feedback_beams: Vec<(u32, UnsignedInt)>,
}

impl AssistedTargetingUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .downcast_ref::<AssistedTargetingUpdateModuleData>()
            .ok_or("Invalid module data")?;

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            next_call_frame_and_phase: 0,
            laser_from_assisted: None,
            laser_to_target: None,
            feedback_beams: Vec::new(),
        })
    }

    fn refresh_laser_templates(&mut self) {
        self.laser_from_assisted = if self.module_data.laser_from_assisted_name.is_empty() {
            None
        } else {
            TheThingFactory::find_template(self.module_data.laser_from_assisted_name.as_str())
        };
        self.laser_to_target = if self.module_data.laser_from_assisted_name.is_empty() {
            None
        } else {
            TheThingFactory::find_template(self.module_data.laser_from_assisted_name.as_str())
        };
    }

    fn make_feedback_laser(
        &mut self,
        laser_template: &Arc<dyn ThingTemplate>,
        from_id: ObjectID,
        to_id: ObjectID,
    ) {
        let from_pos = if let Some(from_arc) = OBJECT_REGISTRY.get_object(from_id) {
            *from_arc.read().unwrap().get_position()
        } else {
            return;
        };
        let to_pos = if let Some(to_arc) = OBJECT_REGISTRY.get_object(to_id) {
            *to_arc.read().unwrap().get_position()
        } else {
            return;
        };

        let team = if let Some(me_arc) = self.object.upgrade() {
            me_arc
                .read()
                .ok()
                .and_then(|me| me.get_controlling_player())
                .and_then(|player| player.read().ok().and_then(|p| p.get_default_team()))
        } else {
            None
        };

        let Some(team_arc) = team else {
            return;
        };
        let Ok(team_guard) = team_arc.read() else {
            return;
        };

        if let Ok(factory) = TheThingFactory::get() {
            if let Ok(laser) = factory.new_object(Arc::clone(laser_template), &team_guard) {
                let laser_id = laser.read().ok().map(|guard| guard.get_id()).unwrap_or(0);
                if let Ok(mut laser_guard) = laser.write() {
                    let _ = laser_guard.set_position(&from_pos);
                }
                if let Some(client) = TheGameClient::get() {
                    let draw_id = client.create_drawable(laser_template.as_ref());
                    client.set_drawable_beam(draw_id, &from_pos, &to_pos);
                    if laser_id != 0 {
                        client.set_drawable_shroud_status_object_id(draw_id, laser_id);
                    }
                    let end_frame =
                        TheGameLogic::get_frame().saturating_add(FEEDBACK_LASER_LIFE_FRAMES);
                    self.feedback_beams.push((draw_id, end_frame));
                }
            }
        }
    }
}

impl UpdateModuleInterface for AssistedTargetingUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        self.refresh_laser_templates();
        if self.feedback_beams.is_empty() {
            return UpdateSleepTime::Forever;
        }

        let now = TheGameLogic::get_frame();
        if let Some(client) = TheGameClient::get() {
            self.feedback_beams.retain(|(id, end_frame)| {
                if now >= *end_frame {
                    client.destroy_drawable(*id);
                    false
                } else {
                    true
                }
            });
        } else {
            self.feedback_beams.clear();
        }

        if self.feedback_beams.is_empty() {
            UpdateSleepTime::Forever
        } else {
            UpdateSleepTime::Frames(1)
        }
    }
}

impl BehaviorModuleInterface for AssistedTargetingUpdate {
    fn get_module_name(&self) -> &'static str {
        "AssistedTargetingUpdate"
    }
    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
    fn get_assisted_targeting_update_interface(
        &mut self,
    ) -> Option<&mut dyn AssistedTargetingUpdateInterface> {
        Some(self)
    }
}

impl AssistedTargetingUpdateInterface for AssistedTargetingUpdate {
    fn is_free_to_assist(&self) -> bool {
        let Some(me_arc) = self.object.upgrade() else {
            return false;
        };
        let me = me_arc.read().unwrap();

        if !me.is_able_to_attack() {
            return false;
        }

        // Logic frames check or weapon status check
        if let Some(weapon) = me.get_weapon_in_slot(self.module_data.weapon_slot) {
            return weapon.get_status() == WeaponStatus::ReadyToFire;
        }
        false
    }

    fn assist_attack(&mut self, requesting_object_id: ObjectID, victim_object_id: ObjectID) {
        let Some(me_arc) = self.object.upgrade() else {
            return;
        };
        let mut me = me_arc.write().unwrap();

        if let Some(ai_arc) = me.get_ai() {
            me.set_weapon_lock(
                self.module_data.weapon_slot,
                WeaponLockType::LockedTemporarily,
            );
            let mut params =
                AiCommandParams::new(AiCommandType::AttackObject, CommandSourceType::FromAi);
            params.obj = Some(victim_object_id);
            params.int_value = self.module_data.clip_size;
            let _ = ai_arc.lock().unwrap().execute_command(&params);
        }

        let me_id = me.get_id();
        drop(me);

        let laser_from_assisted = self.laser_from_assisted.clone();
        let laser_to_target = self.laser_to_target.clone();
        if let Some(template) = laser_from_assisted.as_ref() {
            self.make_feedback_laser(template, requesting_object_id, me_id);
        }
        if let Some(template) = laser_to_target.as_ref() {
            self.make_feedback_laser(template, me_id, victim_object_id);
        }
    }
}

impl Snapshotable for AssistedTargetingUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("AssistedTargetingUpdate xfer version failed: {:?}", e))?;
        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.refresh_laser_templates();
        Ok(())
    }
}

/// Glue that exposes AssistedTargetingUpdate through the common Module trait.
pub struct AssistedTargetingUpdateModule {
    behavior: AssistedTargetingUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<AssistedTargetingUpdateModuleData>,
}

impl AssistedTargetingUpdateModule {
    pub fn new(
        behavior: AssistedTargetingUpdate,
        module_name: &AsciiString,
        module_data: Arc<AssistedTargetingUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut AssistedTargetingUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for AssistedTargetingUpdateModule {
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

impl Module for AssistedTargetingUpdateModule {
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

pub struct AssistedTargetingUpdateFactory;
impl AssistedTargetingUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(AssistedTargetingUpdate::new(thing, module_data)?))
    }
}

fn parse_clip_size(
    _ini: &mut INI,
    data: &mut AssistedTargetingUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.clip_size = tokens[0].parse().map_err(|_| INIError::InvalidData)?;
    Ok(())
}

fn parse_weapon_slot(
    _ini: &mut INI,
    data: &mut AssistedTargetingUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    let slot = match tokens[0].to_ascii_uppercase().as_str() {
        "PRIMARY" | "PRIMARY_WEAPON" => WeaponSlotType::Primary,
        "SECONDARY" | "SECONDARY_WEAPON" => WeaponSlotType::Secondary,
        "TERTIARY" | "TERTIARY_WEAPON" => WeaponSlotType::Tertiary,
        _ => return Err(INIError::InvalidData),
    };
    data.weapon_slot = slot;
    Ok(())
}

fn parse_laser_from_assisted(
    _ini: &mut INI,
    data: &mut AssistedTargetingUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.laser_from_assisted_name = AsciiString::from(tokens[0]);
    Ok(())
}

fn parse_laser_to_target(
    _ini: &mut INI,
    data: &mut AssistedTargetingUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.laser_to_target_name = AsciiString::from(tokens[0]);
    Ok(())
}

const ASSISTED_TARGETING_UPDATE_FIELDS: &[FieldParse<AssistedTargetingUpdateModuleData>] = &[
    FieldParse {
        token: "AssistingClipSize",
        parse: parse_clip_size,
    },
    FieldParse {
        token: "AssistingWeaponSlot",
        parse: parse_weapon_slot,
    },
    FieldParse {
        token: "LaserFromAssisted",
        parse: parse_laser_from_assisted,
    },
    FieldParse {
        token: "LaserToTarget",
        parse: parse_laser_to_target,
    },
];
