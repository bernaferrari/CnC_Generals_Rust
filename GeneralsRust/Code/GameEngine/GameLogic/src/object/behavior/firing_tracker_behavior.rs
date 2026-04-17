//! FiringTracker behavior module - Rust conversion of C++ FiringTracker (UpdateModule).

use std::any::Any;
use std::sync::Arc;

use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::rts::AsciiString;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData};

use crate::common::{DisabledMaskType, NameKeyType, ObjectID};
use crate::helpers::{FiringTracker, TheGameLogic};
use crate::modules::{
    BehaviorModuleInterface, SleepyUpdatePhase, UpdateModuleInterface, UpdateSleepTime,
};

#[derive(Debug, Clone, Default)]
pub struct FiringTrackerBehaviorModuleData {
    module_tag_name_key: NameKeyType,
}

crate::impl_legacy_module_data_with_key_field!(
    FiringTrackerBehaviorModuleData,
    module_tag_name_key
);

impl Snapshotable for FiringTrackerBehaviorModuleData {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Debug)]
pub struct FiringTrackerBehavior {
    object_id: ObjectID,
    tracker: FiringTracker,
}

impl FiringTrackerBehavior {
    pub fn new(object_id: ObjectID) -> Self {
        Self {
            object_id,
            tracker: FiringTracker::new(object_id),
        }
    }

    pub fn shot_fired(&mut self, weapon: &crate::weapon::Weapon, victim_id: ObjectID) {
        self.tracker.shot_fired(weapon, victim_id);
    }

    pub fn last_shot_frame(&self) -> u32 {
        self.tracker.get_last_shot_frame()
    }

    pub fn get_num_consecutive_shots_at_victim(&self, victim_id: ObjectID) -> i32 {
        self.tracker.get_num_consecutive_shots_at_victim(victim_id)
    }
}

impl UpdateModuleInterface for FiringTrackerBehavior {
    fn update_simple(&mut self) -> UpdateSleepTime {
        self.tracker.update()
    }

    fn get_disabled_types_to_process(&self) -> DisabledMaskType {
        DisabledMaskType::all()
    }

    fn get_update_phase(&self) -> SleepyUpdatePhase {
        SleepyUpdatePhase::Final
    }
}

impl BehaviorModuleInterface for FiringTrackerBehavior {
    fn get_module_name(&self) -> &'static str {
        "FiringTracker"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
}

impl Snapshotable for FiringTrackerBehavior {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("xfer version failed: {e:?}"))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

pub struct FiringTrackerBehaviorModule {
    behavior: FiringTrackerBehavior,
    module_name_key: NameKeyType,
    module_data: Arc<FiringTrackerBehaviorModuleData>,
}

impl FiringTrackerBehaviorModule {
    pub fn new(
        behavior: FiringTrackerBehavior,
        module_name: &AsciiString,
        module_data: Arc<FiringTrackerBehaviorModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior(&self) -> &FiringTrackerBehavior {
        &self.behavior
    }

    pub fn behavior_mut(&mut self) -> &mut FiringTrackerBehavior {
        &mut self.behavior
    }
}

impl Snapshotable for FiringTrackerBehaviorModule {
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

impl Module for FiringTrackerBehaviorModule {

    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.module_data.as_ref()
    }

    fn on_object_created(&mut self) {
        if self.object_id() != 0 {
            TheGameLogic::set_wake_frame(self.object_id(), UpdateSleepTime::Forever);
        }
    }
}

impl FiringTrackerBehaviorModule {
    fn object_id(&self) -> ObjectID {
        self.behavior.object_id
    }
}
