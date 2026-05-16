//! Port of `GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/BridgeScaffoldBehavior.cpp`.
//!
//! BridgeScaffoldBehavior - Rust conversion of C++ BridgeScaffoldBehavior
//!
//! Bridge scaffold behavior for construction animation
//! Author: Colin Day, September 2002 (C++ version)
//! Rust conversion: 2025

use std::any::Any;
use std::sync::{Arc, Mutex, RwLock, Weak};

use crate::common::{
    AsciiString, BehaviorModuleData, Coord3D, ObjectID, Real, UnsignedInt, Xfer, XferExt,
    XferVersion,
};
use crate::helpers::TheGameLogic;
use crate::modules::{BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime};
use crate::object::{
    registry::OBJECT_REGISTRY, Object as GameObject, INVALID_ID as OBJECT_INVALID_ID,
};
use game_engine::common::ini::{INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::Snapshotable;
use game_engine::common::thing::module::{
    Module as EngineModule, ModuleData as EngineModuleData, NameKeyType, Thing as ModuleThing,
};
use game_engine::system::Xfer as EngineXfer;

use super::behavior_module::{
    xfer_update_module_base_state, BridgeScaffoldBehaviorInterface, ScaffoldTargetMotion,
};

/// BridgeScaffoldBehaviorModuleData - configuration container for scaffolds
#[derive(Debug, Clone)]
pub struct BridgeScaffoldBehaviorModuleData {
    pub base: BehaviorModuleData,
    pub default_lateral_speed: Real,
    pub default_vertical_speed: Real,
}

impl BridgeScaffoldBehaviorModuleData {
    pub fn new() -> Self {
        Self {
            base: BehaviorModuleData::new(),
            default_lateral_speed: 1.0,
            default_vertical_speed: 1.0,
        }
    }

    pub fn parse_from_ini(&mut self, _ini: &mut INI) -> Result<(), INIError> {
        // No custom INI fields are defined for bridge scaffolds; the behavior configures itself
        // at runtime using the owning bridge's parameters.
        Ok(())
    }
}

impl Default for BridgeScaffoldBehaviorModuleData {
    fn default() -> Self {
        Self::new()
    }
}

crate::impl_behavior_module_data_via_base!(BridgeScaffoldBehaviorModuleData, base);

/// BridgeScaffoldBehavior - controls scaffold motion during bridge construction
pub struct BridgeScaffoldBehavior {
    pub module_data: Arc<BridgeScaffoldBehaviorModuleData>,
    object_id: ObjectID,
    object_handle: Mutex<Option<Weak<RwLock<GameObject>>>>,
    next_call_frame_and_phase: UnsignedInt,

    // Motion state
    pub target_motion: ScaffoldTargetMotion,

    // Position data
    pub create_pos: Coord3D,
    pub rise_to_pos: Coord3D,
    pub build_pos: Coord3D,
    pub target_pos: Coord3D,

    // Speed settings
    pub lateral_speed: Real,
    pub vertical_speed: Real,
}

impl BridgeScaffoldBehavior {
    fn construct_with_object_id(
        object_id: ObjectID,
        module_data: Arc<BridgeScaffoldBehaviorModuleData>,
        initial_object: Option<Arc<RwLock<GameObject>>>,
    ) -> Self {
        let (initial_handle, initial_pos) = match initial_object {
            Some(object) => {
                let weak = Arc::downgrade(&object);
                let pos = object.read().ok().map(|guard| *guard.get_position());
                (Some(weak), pos)
            }
            None => {
                if object_id == OBJECT_INVALID_ID {
                    (None, None)
                } else if let Some(object) = OBJECT_REGISTRY.get_object(object_id) {
                    let pos = object.read().ok().map(|guard| *guard.get_position());
                    (Some(Arc::downgrade(&object)), pos)
                } else {
                    (None, None)
                }
            }
        };

        let mut behavior = Self {
            module_data,
            object_id,
            object_handle: Mutex::new(initial_handle),
            next_call_frame_and_phase: 0,
            target_motion: ScaffoldTargetMotion::Still,
            create_pos: Coord3D::new(0.0, 0.0, 0.0),
            rise_to_pos: Coord3D::new(0.0, 0.0, 0.0),
            build_pos: Coord3D::new(0.0, 0.0, 0.0),
            target_pos: Coord3D::new(0.0, 0.0, 0.0),
            lateral_speed: 1.0,
            vertical_speed: 1.0,
        };

        behavior.lateral_speed = behavior.module_data.default_lateral_speed;
        behavior.vertical_speed = behavior.module_data.default_vertical_speed;

        if let Some(pos) = initial_pos {
            behavior.create_pos = pos;
            behavior.rise_to_pos = behavior.create_pos.clone();
            behavior.build_pos = behavior.create_pos.clone();
            behavior.target_pos = behavior.create_pos.clone();
        }

        behavior
    }

    pub fn new_from_object_handle(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<BridgeScaffoldBehaviorModuleData>,
    ) -> Self {
        let object_id = object
            .read()
            .map(|guard| guard.get_id())
            .unwrap_or(OBJECT_INVALID_ID);

        Self::construct_with_object_id(object_id, module_data, Some(object.clone()))
    }

    pub fn from_module_thing(
        thing: Arc<dyn ModuleThing>,
        module_data: Arc<BridgeScaffoldBehaviorModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let module_object = thing
            .as_object()
            .ok_or_else(|| "BridgeScaffoldBehavior requires an owning object".to_string())?;

        let object_id = module_object.get_object_id();
        let object = OBJECT_REGISTRY.get_object(object_id).ok_or_else(|| {
            format!("BridgeScaffoldBehavior requires object {object_id} to exist")
        })?;

        Ok(Self::new_from_object_handle(object, module_data))
    }

    /// Get bridge scaffold behavior interface from object
    pub fn get_bridge_scaffold_behavior_interface_from_object(
        obj: Arc<RwLock<GameObject>>,
    ) -> Option<Arc<Mutex<dyn BridgeScaffoldBehaviorInterface>>> {
        let _ = obj;
        None
    }

    /// Update target position based on current motion state
    fn update_target_position(&mut self) {
        match self.target_motion {
            ScaffoldTargetMotion::Still => {
                if let Ok(me) = self.get_object() {
                    if let Ok(me_read) = me.read() {
                        self.target_pos = *me_read.get_position();
                    }
                }
            }
            ScaffoldTargetMotion::Rise => {
                self.target_pos = self.rise_to_pos.clone();
            }
            ScaffoldTargetMotion::BuildAcross => {
                self.target_pos = self.build_pos.clone();
            }
            ScaffoldTargetMotion::TearDownAcross => {
                self.target_pos = self.rise_to_pos.clone();
            }
            ScaffoldTargetMotion::Sink => {
                self.target_pos = self.create_pos.clone();
            }
        }
    }

    /// Get the object this behavior belongs to
    fn get_object(
        &self,
    ) -> Result<Arc<RwLock<GameObject>>, Box<dyn std::error::Error + Send + Sync>> {
        if self.object_id == OBJECT_INVALID_ID {
            return Err("BridgeScaffoldBehavior missing owning object id".into());
        }

        if let Ok(mut handle) = self.object_handle.lock() {
            if let Some(weak) = handle.as_ref() {
                if let Some(object) = weak.upgrade() {
                    return Ok(object);
                }
            }

            if let Some(object) = OBJECT_REGISTRY.get_object(self.object_id) {
                *handle = Some(Arc::downgrade(&object));
                return Ok(object);
            }
        }

        Err(format!(
            "BridgeScaffoldBehavior unable to upgrade handle for object {}",
            self.object_id
        )
        .into())
    }
}

// Implement BridgeScaffoldBehaviorInterface
impl BridgeScaffoldBehaviorInterface for BridgeScaffoldBehavior {
    fn set_positions(&mut self, create_pos: &Coord3D, rise_to_pos: &Coord3D, build_pos: &Coord3D) {
        self.create_pos = create_pos.clone();
        self.rise_to_pos = rise_to_pos.clone();
        self.build_pos = build_pos.clone();
        self.update_target_position();
    }

    fn set_motion(&mut self, target_motion: ScaffoldTargetMotion) {
        self.target_motion = target_motion;
        self.update_target_position();
    }

    fn get_current_motion(&self) -> ScaffoldTargetMotion {
        self.target_motion
    }

    fn reverse_motion(&mut self) {
        self.target_motion = match self.target_motion {
            ScaffoldTargetMotion::Rise => ScaffoldTargetMotion::Sink,
            ScaffoldTargetMotion::Sink => ScaffoldTargetMotion::Rise,
            ScaffoldTargetMotion::BuildAcross => ScaffoldTargetMotion::TearDownAcross,
            ScaffoldTargetMotion::TearDownAcross => ScaffoldTargetMotion::BuildAcross,
            ScaffoldTargetMotion::Still => ScaffoldTargetMotion::TearDownAcross,
        };
        self.update_target_position();
    }

    fn set_lateral_speed(&mut self, lateral_speed: Real) {
        self.lateral_speed = lateral_speed;
    }

    fn set_vertical_speed(&mut self, vertical_speed: Real) {
        self.vertical_speed = vertical_speed;
    }
}

// Implement UpdateModuleInterface
impl UpdateModuleInterface for BridgeScaffoldBehavior {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        if self.target_motion == ScaffoldTargetMotion::Still {
            return Ok(crate::modules::UPDATE_SLEEP_NONE);
        }

        let me = self.get_object()?;
        let mut me_write = me
            .write()
            .map_err(|e| format!("bridge scaffold lock poisoned: {}", e))?;
        let current_pos = *me_write.get_position();

        let dir = self.target_pos - current_pos;
        let dir_len = dir.length();
        if dir_len <= f32::EPSILON {
            me_write.set_position(&self.target_pos)?;
            return Ok(crate::modules::UPDATE_SLEEP_NONE);
        }

        let mut top_speed = 1.0;
        let (start, end) = match self.target_motion {
            ScaffoldTargetMotion::Rise => {
                top_speed = self.vertical_speed;
                (self.create_pos, self.rise_to_pos)
            }
            ScaffoldTargetMotion::Sink => {
                top_speed = self.vertical_speed;
                (self.rise_to_pos, self.create_pos)
            }
            ScaffoldTargetMotion::BuildAcross => {
                top_speed = self.lateral_speed;
                (self.rise_to_pos, self.build_pos)
            }
            ScaffoldTargetMotion::TearDownAcross => {
                top_speed = self.lateral_speed;
                (self.build_pos, self.rise_to_pos)
            }
            ScaffoldTargetMotion::Still => (current_pos, current_pos),
        };

        let total_distance = (end - start).length() * 0.25;
        let our_distance = (end - current_pos).length();
        let mut speed = if total_distance > f32::EPSILON {
            (our_distance / total_distance) * top_speed
        } else {
            top_speed
        };
        let min_speed = top_speed * 0.08;
        if speed < min_speed {
            speed = min_speed;
        }
        if speed > top_speed {
            speed = top_speed;
        }
        if speed < 0.001 {
            speed = 0.001;
        }

        let new_pos = current_pos + (dir / dir_len) * speed;
        let too_far = {
            let to_target_new = self.target_pos - new_pos;
            to_target_new.x * dir.x + to_target_new.y * dir.y + to_target_new.z * dir.z <= 0.0
        };

        if too_far {
            let final_pos = self.target_pos;
            me_write.set_position(&final_pos)?;
            drop(me_write);

            match self.target_motion {
                ScaffoldTargetMotion::Rise => self.set_motion(ScaffoldTargetMotion::BuildAcross),
                ScaffoldTargetMotion::BuildAcross => self.set_motion(ScaffoldTargetMotion::Still),
                ScaffoldTargetMotion::TearDownAcross => self.set_motion(ScaffoldTargetMotion::Sink),
                ScaffoldTargetMotion::Sink => {
                    if let Ok(me_read) = me.read() {
                        let _ = TheGameLogic::destroy_object(&*me_read);
                    }
                }
                ScaffoldTargetMotion::Still => {}
            }

            return Ok(crate::modules::UPDATE_SLEEP_NONE);
        }

        me_write.set_position(&new_pos)?;
        Ok(crate::modules::UPDATE_SLEEP_NONE)
    }
}

// Implement BehaviorModuleInterface
impl BehaviorModuleInterface for BridgeScaffoldBehavior {
    fn get_bridge_scaffold_behavior_interface(
        &mut self,
    ) -> Option<&mut dyn BridgeScaffoldBehaviorInterface> {
        Some(self)
    }
}

/// BridgeScaffoldBehaviorModule - integrates scaffold behavior with the module factory
pub struct BridgeScaffoldBehaviorModule {
    behavior: BridgeScaffoldBehavior,
    module_name_key: NameKeyType,
    module_data: Arc<BridgeScaffoldBehaviorModuleData>,
}

impl BridgeScaffoldBehaviorModule {
    pub fn new(
        behavior: BridgeScaffoldBehavior,
        module_name: &AsciiString,
        module_data: Arc<BridgeScaffoldBehaviorModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior(&self) -> &BridgeScaffoldBehavior {
        &self.behavior
    }

    pub fn behavior_mut(&mut self) -> &mut BridgeScaffoldBehavior {
        &mut self.behavior
    }
}

impl Snapshotable for BridgeScaffoldBehaviorModule {
    fn crc(&self, xfer: &mut dyn EngineXfer) -> Result<(), String> {
        self.behavior.crc(xfer).map_err(|err| err.to_string())
    }

    fn xfer(&mut self, xfer: &mut dyn EngineXfer) -> Result<(), String> {
        self.behavior.xfer(xfer).map_err(|err| err.to_string())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.behavior
            .load_post_process()
            .map_err(|err| err.to_string())
    }
}

impl EngineModule for BridgeScaffoldBehaviorModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn EngineModuleData {
        self.module_data.as_ref()
    }

    fn on_object_created(&mut self) {
        self.behavior.update_target_position();
    }

    fn on_delete(&mut self) {}
}

impl BridgeScaffoldBehavior {
    pub fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;
        let mut next_call_frame_and_phase = self.next_call_frame_and_phase;
        xfer_update_module_base_state(xfer, &mut next_call_frame_and_phase)?;

        let mut motion_value: UnsignedInt = self.target_motion as UnsignedInt;
        xfer.xfer_unsigned_int(&mut motion_value)?;

        let mut create_pos = self.create_pos;
        xfer.xfer_coord3d(&mut create_pos);
        let mut rise_to_pos = self.rise_to_pos;
        xfer.xfer_coord3d(&mut rise_to_pos);
        let mut build_pos = self.build_pos;
        xfer.xfer_coord3d(&mut build_pos);
        let mut lateral_speed = self.lateral_speed;
        xfer.xfer_real(&mut lateral_speed)?;
        let mut vertical_speed = self.vertical_speed;
        xfer.xfer_real(&mut vertical_speed)?;
        let mut target_pos = self.target_pos;
        xfer.xfer_coord3d(&mut target_pos);

        Ok(())
    }

    pub fn xfer(
        &mut self,
        xfer: &mut dyn Xfer,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;
        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;

        let mut motion_value: UnsignedInt = self.target_motion as UnsignedInt;
        xfer.xfer_unsigned_int(&mut motion_value)?;
        self.target_motion = match motion_value {
            0 => ScaffoldTargetMotion::Still,
            1 => ScaffoldTargetMotion::Rise,
            2 => ScaffoldTargetMotion::BuildAcross,
            3 => ScaffoldTargetMotion::TearDownAcross,
            4 => ScaffoldTargetMotion::Sink,
            _ => ScaffoldTargetMotion::Still,
        };

        xfer.xfer_coord3d(&mut self.create_pos);
        xfer.xfer_coord3d(&mut self.rise_to_pos);
        xfer.xfer_coord3d(&mut self.build_pos);
        xfer.xfer_real(&mut self.lateral_speed)?;
        xfer.xfer_real(&mut self.vertical_speed)?;
        xfer.xfer_coord3d(&mut self.target_pos);

        Ok(())
    }

    pub fn load_post_process(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

unsafe impl Send for BridgeScaffoldBehavior {}
unsafe impl Sync for BridgeScaffoldBehavior {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn bridge_scaffold_behavior_defaults_to_still_motion() {
        let data = Arc::new(BridgeScaffoldBehaviorModuleData::default());
        let behavior = BridgeScaffoldBehavior::construct_with_object_id(
            super::OBJECT_INVALID_ID,
            Arc::clone(&data),
            None,
        );
        assert_eq!(behavior.target_motion, ScaffoldTargetMotion::Still);
    }
}
