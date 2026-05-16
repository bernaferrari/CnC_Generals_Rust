//! FloatUpdate - Rust conversion of C++ FloatUpdate
//!
//! Snap objects to the top of water and adds a slight rocking motion.
//! Author: Colin Day, May 2002 (C++ version)
//! Rust conversion: 2025

use crate::common::xfer::XferExt;
use crate::common::{AsciiString, Bool, ModuleData, Real, UnsignedInt, XferVersion};
use crate::helpers::{TheGameLogic, TheTerrainLogic};
use crate::modules::{
    BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime, UPDATE_SLEEP_NONE,
};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::drawable::DrawableArcExt;
use crate::object::Object as GameObject;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData as EngineModuleData, NameKeyType};
use glam::{EulerRot, Mat4};
use std::sync::{Arc, RwLock, Weak};

#[derive(Clone, Debug)]
pub struct FloatUpdateModuleData {
    pub base: BehaviorModuleData,
    pub enabled: Bool,
}

impl Default for FloatUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            enabled: false,
        }
    }
}

crate::impl_behavior_module_data_via_base!(FloatUpdateModuleData, base);

impl FloatUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, FLOAT_UPDATE_FIELDS)
    }
}

fn parse_enabled(
    _ini: &mut INI,
    data: &mut FloatUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.enabled = INI::parse_bool(token)?;
    Ok(())
}

const FLOAT_UPDATE_FIELDS: &[FieldParse<FloatUpdateModuleData>] = &[FieldParse {
    token: "Enabled",
    parse: parse_enabled,
}];

pub struct FloatUpdate {
    object: Weak<RwLock<GameObject>>,
    #[allow(dead_code)]
    module_data: Arc<FloatUpdateModuleData>,
    next_call_frame_and_phase: UnsignedInt,
    enabled: Bool,
}

impl FloatUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .downcast_ref::<FloatUpdateModuleData>()
            .ok_or("Invalid module data")?;

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            next_call_frame_and_phase: 0,
            enabled: specific_data.enabled,
        })
    }
}

impl UpdateModuleInterface for FloatUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        let me_arc = match self.object.upgrade() {
            Some(arc) => arc,
            None => return UPDATE_SLEEP_NONE,
        };

        if self.enabled {
            let mut me = match me_arc.write() {
                Ok(guard) => guard,
                Err(_) => return UPDATE_SLEEP_NONE,
            };
            let mut pos = *me.get_position();

            let mut water_z = 0.0;
            if let Some(terrain) = TheTerrainLogic::get() {
                // Determine if we're underwater and get surface height
                terrain.is_underwater(pos.x, pos.y, Some(&mut water_z), None);
            }

            // Snap to the water surface
            pos.z = water_z;
            let _ = me.set_position(&pos);
        }

        // Apply rocking motion to the drawable
        let me = match me_arc.read() {
            Ok(guard) => guard,
            Err(_) => return UPDATE_SLEEP_NONE,
        };
        if let Some(draw) = me.get_drawable() {
            let frame = TheGameLogic::get_frame() as f32;
            let yaw_rocking = (frame * 0.0291).sin() * 0.05;
            let pitch_rocking = (frame * 0.0515).sin() * 0.05;

            let transform = draw.get_transform();
            let (_, rotation, _) = transform.to_scale_rotation_translation();
            let (_, _, z_rot) = rotation.to_euler(EulerRot::XYZ);

            // C++: Rotate_Z(zRot); Rotate_Y(yaw); Rotate_X(pitch);
            let mut mx = Mat4::from_rotation_z(z_rot);
            mx *= Mat4::from_rotation_y(yaw_rocking);
            mx *= Mat4::from_rotation_x(pitch_rocking);

            draw.set_instance_matrix(Some(&mx));
        }

        UPDATE_SLEEP_NONE
    }
}

impl BehaviorModuleInterface for FloatUpdate {
    fn get_module_name(&self) -> &'static str {
        "FloatUpdate"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
}

impl Snapshotable for FloatUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;
        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;
        xfer.xfer_bool(&mut self.enabled)
            .map_err(|e| format!("Failed to xfer enabled: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Glue that exposes FloatUpdate through the common Module trait.
pub struct FloatUpdateModule {
    behavior: FloatUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<FloatUpdateModuleData>,
}

impl FloatUpdateModule {
    pub fn new(
        behavior: FloatUpdate,
        module_name: &AsciiString,
        module_data: Arc<FloatUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut FloatUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for FloatUpdateModule {
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

impl Module for FloatUpdateModule {
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

pub struct FloatUpdateFactory;
impl FloatUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(FloatUpdate::new(thing, module_data)?))
    }
}
