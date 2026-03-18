//! AnimationSteeringUpdate - Steers animation based on movement
//! Author: EA Pacific (C++ version) | Rust conversion: 2025

use crate::common::{ModelConditionFlags, ModuleData, Real, UnsignedInt};
use crate::helpers::TheGameLogic;
use crate::modules::{BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime};
use crate::object::behavior::behavior_module::BehaviorModuleData;
use crate::object::Object as GameObject;
use game_engine::common::system::{Snapshotable, Xfer};
use std::sync::{Arc, RwLock, Weak};

#[derive(Clone, Debug)]
pub struct AnimationSteeringUpdateModuleData {
    pub base: BehaviorModuleData,
    pub transition_frames: UnsignedInt,
}

impl Default for AnimationSteeringUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            transition_frames: 0,
        }
    }
}

crate::impl_behavior_module_data_via_base!(AnimationSteeringUpdateModuleData, base);

pub struct AnimationSteeringUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<AnimationSteeringUpdateModuleData>,
    last_direction: Real,
    current_turn_anim: ModelConditionFlags,
    next_transition_frame: UnsignedInt,
}

impl AnimationSteeringUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .as_any()
            .downcast_ref::<AnimationSteeringUpdateModuleData>()
            .ok_or("Invalid module data")?;

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            last_direction: 0.0,
            current_turn_anim: ModelConditionFlags::Invalid,
            next_transition_frame: 0,
        })
    }
}

impl UpdateModuleInterface for AnimationSteeringUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        let Some(object_arc) = self.object.upgrade() else {
            return UpdateSleepTime::Forever;
        };
        let Ok(object_guard) = object_arc.read() else {
            return UpdateSleepTime::Frames(1);
        };

        let Some(physics_arc) = object_guard.get_physics() else {
            return UpdateSleepTime::Frames(1);
        };
        let Some(drawable_arc) = object_guard.get_drawable() else {
            return UpdateSleepTime::Frames(1);
        };

        let now = TheGameLogic::get_frame();
        if now < self.next_transition_frame {
            return UpdateSleepTime::Frames(1);
        }

        let current_turn = physics_arc
            .lock()
            .map(|guard| guard.get_turning())
            .unwrap_or(0.0);

        let turn_state = if current_turn < 0.0 {
            ModelConditionFlags::CenterToRight
        } else if current_turn > 0.0 {
            ModelConditionFlags::CenterToLeft
        } else {
            ModelConditionFlags::Invalid
        };

        let mut drawable_guard = match drawable_arc.write() {
            Ok(guard) => guard,
            Err(_) => return UpdateSleepTime::Frames(1),
        };

        if self.current_turn_anim == ModelConditionFlags::Invalid {
            if turn_state == ModelConditionFlags::CenterToRight {
                drawable_guard.set_model_condition_state(ModelConditionFlags::CenterToRight);
                self.next_transition_frame = now + self.module_data.transition_frames;
                self.current_turn_anim = ModelConditionFlags::CenterToRight;
            } else if turn_state == ModelConditionFlags::CenterToLeft {
                drawable_guard.set_model_condition_state(ModelConditionFlags::CenterToLeft);
                self.next_transition_frame = now + self.module_data.transition_frames;
                self.current_turn_anim = ModelConditionFlags::CenterToLeft;
            }
        } else if self.current_turn_anim == ModelConditionFlags::CenterToRight {
            if turn_state != ModelConditionFlags::CenterToRight {
                drawable_guard.clear_and_set_model_condition_state(
                    ModelConditionFlags::CenterToRight,
                    ModelConditionFlags::RightToCenter,
                );
                self.next_transition_frame = now + self.module_data.transition_frames;
                self.current_turn_anim = ModelConditionFlags::RightToCenter;
            }
        } else if self.current_turn_anim == ModelConditionFlags::CenterToLeft {
            if turn_state != ModelConditionFlags::CenterToLeft {
                drawable_guard.clear_and_set_model_condition_state(
                    ModelConditionFlags::CenterToLeft,
                    ModelConditionFlags::LeftToCenter,
                );
                self.next_transition_frame = now + self.module_data.transition_frames;
                self.current_turn_anim = ModelConditionFlags::LeftToCenter;
            }
        } else if self.current_turn_anim == ModelConditionFlags::LeftToCenter
            || self.current_turn_anim == ModelConditionFlags::RightToCenter
        {
            if turn_state == ModelConditionFlags::Invalid {
                drawable_guard.clear_model_condition_flags(
                    ModelConditionFlags::LeftToCenter | ModelConditionFlags::RightToCenter,
                );
                self.next_transition_frame = now;
                self.current_turn_anim = ModelConditionFlags::Invalid;
            }
        }

        UpdateSleepTime::Frames(1)
    }
}

impl BehaviorModuleInterface for AnimationSteeringUpdate {
    fn get_module_name(&self) -> &'static str {
        "AnimationSteeringUpdate"
    }
    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
}

impl Snapshotable for AnimationSteeringUpdate {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        xfer.xfer_real(&mut self.last_direction)
            .map_err(|e| format!("AnimationSteeringUpdate xfer last_direction: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.next_transition_frame)
            .map_err(|e| format!("AnimationSteeringUpdate xfer next_transition_frame: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

pub struct AnimationSteeringUpdateFactory;
impl AnimationSteeringUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(AnimationSteeringUpdate::new(thing, module_data)?))
    }
}
