//! LaserUpdate leftover is C++ ClientUpdate, not invented DPS Behavior.
//!
//! C++ `LaserUpdate` is a drawable ClientUpdate (`LaserUpdate.cpp`). It never
//! deals damage. Damage comes from the weapon. This leftover module forwards
//! to the live ClientUpdate implementation and keeps leftover factory glue.

use crate::common::{AsciiString, ModuleData, ObjectID, Real};
use crate::helpers::TheGameLogic;
use crate::modules::{BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime};
use crate::object::Object as GameObject;
use crate::object::behavior::behavior_module::BehaviorModuleData;
use crate::object::drawable::DrawableArcExt;
use crate::prelude::Coord3D;
use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    ClientUpdateInterface, LaserUpdateInterface, Module, ModuleData as EngineModuleData,
    NameKeyType,
};
use std::sync::{Arc, RwLock};

#[derive(Clone, Debug)]
pub struct LaserUpdateModuleData {
    pub base: BehaviorModuleData,
    pub particle_system_name: String,
    pub target_particle_system_name: String,
    pub punch_through_scalar: Real,
}

impl Default for LaserUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            particle_system_name: String::new(),
            target_particle_system_name: String::new(),
            punch_through_scalar: 0.0,
        }
    }
}

crate::impl_behavior_module_data_via_base!(LaserUpdateModuleData, base);

impl LaserUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, LASER_UPDATE_FIELDS)
    }

    fn to_live(&self) -> crate::object::update::laser_update::LaserUpdateModuleData {
        crate::object::update::laser_update::LaserUpdateModuleData {
            module_tag_name_key: 0,
            particle_system_name: self.particle_system_name.clone(),
            target_particle_system_name: self.target_particle_system_name.clone(),
            punch_through_scalar: self.punch_through_scalar,
        }
    }
}

fn parse_muzzle_particle(
    _ini: &mut INI,
    data: &mut LaserUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    data.particle_system_name = INI::parse_ascii_string(value)?;
    Ok(())
}

fn parse_target_particle(
    _ini: &mut INI,
    data: &mut LaserUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    data.target_particle_system_name = INI::parse_ascii_string(value)?;
    Ok(())
}

fn parse_punch_through(
    _ini: &mut INI,
    data: &mut LaserUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    data.punch_through_scalar = INI::parse_real(value)?;
    Ok(())
}

const LASER_UPDATE_FIELDS: &[FieldParse<LaserUpdateModuleData>] = &[
    FieldParse {
        token: "MuzzleParticleSystem",
        parse: parse_muzzle_particle,
    },
    FieldParse {
        token: "TargetParticleSystem",
        parse: parse_target_particle,
    },
    FieldParse {
        token: "PunchThroughScalar",
        parse: parse_punch_through,
    },
];

pub struct LaserUpdate {
    object_id: ObjectID,
    module_data: Arc<LaserUpdateModuleData>,
    inner: crate::object::update::laser_update::LaserUpdate,
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

        let object_id = object
            .read()
            .ok()
            .map(|g| g.get_id())
            .unwrap_or(crate::common::INVALID_ID);
        let thing_id = object
            .read()
            .ok()
            .and_then(|g| g.get_drawable().map(|drawable| drawable.get_id()))
            .unwrap_or(object_id);

        Ok(Self {
            object_id,
            module_data: Arc::new(specific_data.clone()),
            inner: crate::object::update::laser_update::LaserUpdate::new(
                thing_id,
                specific_data.to_live(),
            ),
        })
    }
}

impl UpdateModuleInterface for LaserUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        // C++ LaserUpdate is ClientUpdate. It never ticks as object Behavior DPS.
        UpdateSleepTime::Forever
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

/// Invented leftover Behavior hook. C++ LaserUpdate has no object Behavior.
pub trait LaserBehaviorControlInterface {
    fn activate_laser(&mut self, target: ObjectID);
    fn configure_laser(&mut self, damage_per_frame: Real, duration: Real);
}

impl LaserBehaviorControlInterface for LaserUpdate {
    fn activate_laser(&mut self, _target: ObjectID) {
        // C++ never deals laser DPS from LaserUpdate. Use ClientUpdate initLaser.
    }

    fn configure_laser(&mut self, _damage_per_frame: Real, _duration: Real) {}
}

impl ClientUpdateInterface for LaserUpdate {
    fn client_update(&mut self) -> bool {
        self.inner.client_update();
        true
    }
}

impl LaserUpdateInterface for LaserUpdate {
    fn is_dirty(&self) -> bool {
        self.inner.is_dirty()
    }

    fn set_dirty(&mut self, dirty: bool) {
        self.inner.set_dirty(dirty);
    }

    fn get_start_pos(&self) -> [f32; 3] {
        self.inner.get_start_pos().to_array()
    }

    fn get_end_pos(&self) -> [f32; 3] {
        self.inner.get_end_pos().to_array()
    }

    fn get_width_scale(&self) -> f32 {
        self.inner.get_width_scale()
    }

    fn init_laser(
        &mut self,
        parent_id: Option<ObjectID>,
        target_id: Option<ObjectID>,
        start_pos: Option<[f32; 3]>,
        end_pos: Option<[f32; 3]>,
        parent_bone_name: String,
        size_delta_frames: i32,
    ) {
        let parent_arc = parent_id.and_then(TheGameLogic::find_object_by_id);
        let target_arc = target_id.and_then(TheGameLogic::find_object_by_id);
        let parent_guard = parent_arc.as_ref().and_then(|arc| arc.read().ok());
        let target_guard = target_arc.as_ref().and_then(|arc| arc.read().ok());
        let start_pos = start_pos.map(Coord3D::from_array);
        let end_pos = end_pos.map(Coord3D::from_array);
        self.inner.init_laser(
            parent_guard.as_deref(),
            target_guard.as_deref(),
            start_pos.as_ref(),
            end_pos.as_ref(),
            parent_bone_name,
            size_delta_frames,
        );
    }

    fn set_decay_frames(&mut self, decay_frames: u32) {
        self.inner.set_decay_frames(decay_frames);
    }
}

impl Snapshotable for LaserUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("LaserUpdate xfer version failed: {:?}", e))?;
        let mut start = self.inner.get_start_pos();
        xfer.xfer_real(&mut start.x)
            .map_err(|e| format!("LaserUpdate xfer start failed: {:?}", e))?;
        xfer.xfer_real(&mut start.y)
            .map_err(|e| format!("LaserUpdate xfer start failed: {:?}", e))?;
        xfer.xfer_real(&mut start.z)
            .map_err(|e| format!("LaserUpdate xfer start failed: {:?}", e))?;
        let mut end = self.inner.get_end_pos();
        xfer.xfer_real(&mut end.x)
            .map_err(|e| format!("LaserUpdate xfer end failed: {:?}", e))?;
        xfer.xfer_real(&mut end.y)
            .map_err(|e| format!("LaserUpdate xfer end failed: {:?}", e))?;
        xfer.xfer_real(&mut end.z)
            .map_err(|e| format!("LaserUpdate xfer end failed: {:?}", e))?;
        let mut dirty = self.inner.is_dirty();
        xfer.xfer_bool(&mut dirty)
            .map_err(|e| format!("LaserUpdate xfer dirty failed: {:?}", e))?;
        self.inner.set_dirty(dirty);
        let _ = self.object_id;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Glue that exposes leftover LaserUpdate through the common Module trait.
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

    fn get_client_update_interface(&mut self) -> Option<&mut dyn ClientUpdateInterface> {
        Some(&mut self.behavior)
    }

    fn get_laser_update_interface(&mut self) -> Option<&mut dyn LaserUpdateInterface> {
        Some(&mut self.behavior)
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
