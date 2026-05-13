//! LaserUpdate - Rust conversion of C++ LaserUpdate
//!
//! Continuous laser weapon behavior.
//! Author: EA Pacific (C++ version)
//! Rust conversion: 2025

use crate::common::xfer::XferExt;
use crate::common::{AsciiString, Bool, ModuleData, ObjectID, Real, LOGICFRAMES_PER_SECOND};
use crate::damage::{DamageInfo, DamageType, DeathType};
use crate::helpers::TheGameLogic;
use crate::modules::{BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime};
use crate::object::behavior::behavior_module::BehaviorModuleData;
use crate::object::{Object as GameObject, INVALID_ID as OBJECT_INVALID_ID};
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData as EngineModuleData, NameKeyType};
use std::sync::{Arc, RwLock, Weak};

#[derive(Clone, Debug)]
pub struct LaserUpdateModuleData {
    pub base: BehaviorModuleData,
    pub laser_bone_name: String,
    pub laser_duration: Real,
    pub laser_damage: Real,
}

impl Default for LaserUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            laser_bone_name: String::new(),
            laser_duration: 1.0,
            laser_damage: 10.0,
        }
    }
}

crate::impl_behavior_module_data_via_base!(LaserUpdateModuleData, base);

impl LaserUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, LASER_UPDATE_FIELDS)
    }
}

fn parse_laser_bone_name(
    _ini: &mut INI,
    data: &mut LaserUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.laser_bone_name = token.to_string();
    Ok(())
}

fn parse_laser_duration(
    _ini: &mut INI,
    data: &mut LaserUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.laser_duration = INI::parse_real(token)?;
    Ok(())
}

fn parse_laser_damage(
    _ini: &mut INI,
    data: &mut LaserUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.laser_damage = INI::parse_real(token)?;
    Ok(())
}

const LASER_UPDATE_FIELDS: &[FieldParse<LaserUpdateModuleData>] = &[
    FieldParse {
        token: "LaserBoneName",
        parse: parse_laser_bone_name,
    },
    FieldParse {
        token: "LaserDuration",
        parse: parse_laser_duration,
    },
    FieldParse {
        token: "LaserDamage",
        parse: parse_laser_damage,
    },
];

pub struct LaserUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<LaserUpdateModuleData>,
    current_target: ObjectID,
    laser_active: Bool,
    laser_end_frame: u32,
    laser_damage_override: Option<Real>,
    laser_duration_override: Option<Real>,
}

impl LaserUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .downcast_ref::<LaserUpdateModuleData>()
            .ok_or("Invalid module data")?;

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            current_target: OBJECT_INVALID_ID,
            laser_active: false,
            laser_end_frame: 0,
            laser_damage_override: None,
            laser_duration_override: None,
        })
    }

    pub fn activate_laser(&mut self, target: ObjectID) {
        self.current_target = target;
        self.laser_active = true;
        let duration = self
            .laser_duration_override
            .unwrap_or(self.module_data.laser_duration);
        let duration_frames = (duration * LOGICFRAMES_PER_SECOND as Real).max(0.0) as u32;
        self.laser_end_frame = TheGameLogic::get_frame().saturating_add(duration_frames);
    }

    pub fn deactivate_laser(&mut self) {
        self.laser_active = false;
        self.current_target = OBJECT_INVALID_ID;
        self.laser_end_frame = 0;
    }

    pub fn configure_laser(&mut self, damage_per_frame: Real, duration: Real) {
        self.laser_damage_override = Some(damage_per_frame * LOGICFRAMES_PER_SECOND as Real);
        self.laser_duration_override = Some(duration);
    }
}

impl UpdateModuleInterface for LaserUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        if !self.laser_active {
            return UpdateSleepTime::Forever;
        }

        let now = TheGameLogic::get_frame();
        if self.laser_end_frame != 0 && now >= self.laser_end_frame {
            self.deactivate_laser();
            return UpdateSleepTime::Forever;
        }

        let owner_id = self
            .object
            .upgrade()
            .and_then(|arc| arc.read().ok().map(|guard| guard.get_id()))
            .unwrap_or(OBJECT_INVALID_ID);

        let Some(target_arc) = TheGameLogic::find_object_by_id(self.current_target) else {
            self.deactivate_laser();
            return UpdateSleepTime::Forever;
        };

        if target_arc
            .read()
            .ok()
            .map(|guard| guard.is_destroyed())
            .unwrap_or(true)
        {
            self.deactivate_laser();
            return UpdateSleepTime::Forever;
        }

        let laser_damage = self
            .laser_damage_override
            .unwrap_or(self.module_data.laser_damage);
        let damage_per_frame = laser_damage / LOGICFRAMES_PER_SECOND as Real;
        let mut info = DamageInfo::with_simple(
            damage_per_frame,
            owner_id,
            DamageType::Laser,
            DeathType::Normal,
        );
        info.sync_from_input();

        if let Ok(mut target_guard) = target_arc.write() {
            let _ = target_guard.attempt_damage(&mut info);
        }

        UpdateSleepTime::Frames(1) // Update every frame while active
    }
}

impl BehaviorModuleInterface for LaserUpdate {
    fn get_module_name(&self) -> &'static str {
        "LaserUpdate"
    }
    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
    fn get_laser_behavior_control_interface(
        &mut self,
    ) -> Option<&mut dyn LaserBehaviorControlInterface> {
        Some(self)
    }
}

pub trait LaserBehaviorControlInterface {
    fn activate_laser(&mut self, target: ObjectID);
    fn configure_laser(&mut self, damage_per_frame: Real, duration: Real);
}

impl LaserBehaviorControlInterface for LaserUpdate {
    fn activate_laser(&mut self, target: ObjectID) {
        LaserUpdate::activate_laser(self, target);
    }

    fn configure_laser(&mut self, damage_per_frame: Real, duration: Real) {
        LaserUpdate::configure_laser(self, damage_per_frame, duration);
    }
}

impl Snapshotable for LaserUpdate {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("LaserUpdate xfer version failed: {:?}", e))?;

        xfer.xfer_object_id(&mut self.current_target)
            .map_err(|e| format!("LaserUpdate xfer current_target failed: {:?}", e))?;
        xfer.xfer_bool(&mut self.laser_active)
            .map_err(|e| format!("LaserUpdate xfer active failed: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.laser_end_frame)
            .map_err(|e| format!("LaserUpdate xfer end frame failed: {:?}", e))?;

        let mut has_damage_override = self.laser_damage_override.is_some();
        xfer.xfer_bool(&mut has_damage_override)
            .map_err(|e| format!("LaserUpdate xfer damage override flag failed: {:?}", e))?;
        if has_damage_override {
            let mut value = self.laser_damage_override.unwrap_or(0.0);
            xfer.xfer_real(&mut value)
                .map_err(|e| format!("LaserUpdate xfer damage override value failed: {:?}", e))?;
            self.laser_damage_override = Some(value);
        } else {
            self.laser_damage_override = None;
        }

        let mut has_duration_override = self.laser_duration_override.is_some();
        xfer.xfer_bool(&mut has_duration_override)
            .map_err(|e| format!("LaserUpdate xfer duration override flag failed: {:?}", e))?;
        if has_duration_override {
            let mut value = self.laser_duration_override.unwrap_or(0.0);
            xfer.xfer_real(&mut value)
                .map_err(|e| format!("LaserUpdate xfer duration override value failed: {:?}", e))?;
            self.laser_duration_override = Some(value);
        } else {
            self.laser_duration_override = None;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Glue that exposes LaserUpdate through the common Module trait.
pub struct LaserUpdateModule {
    behavior: LaserUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<LaserUpdateModuleData>,
}

impl LaserUpdateModule {
    pub fn new(
        behavior: LaserUpdate,
        module_name: &AsciiString,
        module_data: Arc<LaserUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut LaserUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for LaserUpdateModule {
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

impl Module for LaserUpdateModule {
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

pub struct LaserUpdateFactory;
impl LaserUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(LaserUpdate::new(thing, module_data)?))
    }
}
