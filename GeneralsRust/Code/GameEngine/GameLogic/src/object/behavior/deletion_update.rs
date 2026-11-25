//! DeletionUpdate - Auto-deletion of objects after conditions
//! Author: EA Pacific (C++ version) | Rust conversion: 2025

use crate::common::UnsignedInt;
use crate::modules::{
    BehaviorModuleInterface, SleepyUpdatePhase, UpdateModuleInterface, UpdateSleepTime,
};
use crate::object::Object as GameObject;
use crate::object::behavior::behavior_module::{BehaviorModuleData, UpdateModuleClock};
use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::DeletionLifetimeInterface;
use game_engine::common::thing::update_module::UpdateScheduleContext;
use std::sync::{Arc, RwLock};

#[derive(Clone, Debug)]
pub struct DeletionUpdateModuleData {
    pub base: BehaviorModuleData,
    pub min_lifetime: UnsignedInt,
    pub max_lifetime: UnsignedInt,
}

impl Default for DeletionUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            min_lifetime: 0,
            max_lifetime: 0,
        }
    }
}

crate::impl_behavior_module_data_via_base!(DeletionUpdateModuleData, base);

impl DeletionUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, DELETION_UPDATE_FIELDS)
    }
}

fn parse_min_lifetime(
    _ini: &mut INI,
    data: &mut DeletionUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)?;
    data.min_lifetime = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

fn parse_max_lifetime(
    _ini: &mut INI,
    data: &mut DeletionUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)?;
    data.max_lifetime = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

const DELETION_UPDATE_FIELDS: &[FieldParse<DeletionUpdateModuleData>] = &[
    FieldParse {
        token: "MinLifetime",
        parse: parse_min_lifetime,
    },
    FieldParse {
        token: "MaxLifetime",
        parse: parse_max_lifetime,
    },
];

pub struct DeletionUpdate {
    clock: UpdateModuleClock,
    delete_frame: UnsignedInt,
}

impl DeletionUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: game_engine::common::thing::module::ModuleDataHandle,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::new_with_module_data(object, module_data)
    }

    pub(crate) fn new_with_module_data(
        _object: Arc<RwLock<GameObject>>,
        module_data: game_engine::common::thing::module::ModuleDataHandle,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let guard = module_data.read();
        let specific_data = guard.downcast_ref::<DeletionUpdateModuleData>().ok_or("Invalid module data")?;

        // Get current frame from game logic - matches C++ DeletionUpdate.cpp
        let current_frame = crate::helpers::TheGameLogic::get_frame();
        let lifetime =
            Self::calc_sleep_delay_static(specific_data.min_lifetime, specific_data.max_lifetime);
        drop(guard);

        let mut update = Self {
            clock: UpdateModuleClock::default(),
            delete_frame: current_frame.wrapping_add(lifetime),
        };
        update
            .clock
            .set_wake(update.delete_frame, SleepyUpdatePhase::Normal);
        Ok(update)
    }

    pub fn set_lifetime_range(
        &mut self,
        min_lifetime: UnsignedInt,
        max_lifetime: UnsignedInt,
    ) -> UnsignedInt {
        let current_frame = crate::helpers::TheGameLogic::get_frame();
        let delay = Self::calc_sleep_delay_static(min_lifetime, max_lifetime);
        self.delete_frame = current_frame.wrapping_add(delay);
        self.delete_frame
    }

    pub fn initial_wake_frame(&self) -> UnsignedInt {
        self.clock.wake_frame()
    }

    /// Calculate random sleep delay between min and max frames.
    /// C++ Reference: DeletionUpdate.cpp - uses GameLogicRandomValue and clamps to at least 1.
    fn calc_sleep_delay_static(
        min_lifetime: UnsignedInt,
        max_lifetime: UnsignedInt,
    ) -> UnsignedInt {
        let mut delay = crate::GameLogicRandomValue!(min_lifetime, max_lifetime) as UnsignedInt;
        if delay < 1 {
            delay = 1;
        }
        delay
    }
}

impl UpdateModuleInterface for DeletionUpdate {
    fn record_scheduled_wake(&mut self, frame: u32, phase: SleepyUpdatePhase) {
        self.clock.set_wake(frame, phase);
    }

    fn update_scheduled(
        &mut self,
        context: &mut dyn UpdateScheduleContext,
    ) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        // DeletionUpdate.cpp:58–82 calls destroyObject synchronously. Its
        // callbacks run on the driving GameLogic before the next update.
        context.stage_owner_destruction();
        Ok(UpdateSleepTime::Forever)
    }

    fn update_simple(&mut self) -> UpdateSleepTime {
        UpdateSleepTime::Forever
    }
}

impl BehaviorModuleInterface for DeletionUpdate {
    fn get_module_name(&self) -> &'static str {
        "DeletionUpdate"
    }
    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
    /// Sleepy-update table participant (see `ActiveBehaviorModule`).
    fn get_sleepy_update_interface(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
    /// Wake frame consumed by `initial_update_wake_frame` through
    /// `ActiveBehaviorModule<DeletionUpdate>`.
    fn behavior_initial_wake_frame(&mut self) -> Option<u32> {
        Some(DeletionUpdate::initial_wake_frame(self))
    }

    fn get_deletion_lifetime_interface(&mut self) -> Option<&mut dyn DeletionLifetimeInterface> {
        Some(self)
    }
}

impl DeletionLifetimeInterface for DeletionUpdate {
    fn set_lifetime_range(
        &mut self,
        min_lifetime: UnsignedInt,
        max_lifetime: UnsignedInt,
    ) -> UnsignedInt {
        DeletionUpdate::set_lifetime_range(self, min_lifetime, max_lifetime)
    }
}

impl Snapshotable for DeletionUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let _ = xfer;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("DeletionUpdate xfer version: {:?}", e))?;

        self.clock.xfer(xfer)?;

        xfer.xfer_unsigned_int(&mut self.delete_frame)
            .map_err(|e| format!("DeletionUpdate xfer delete_frame: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

pub struct DeletionUpdateFactory;
impl DeletionUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: game_engine::common::thing::module::ModuleDataHandle,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(DeletionUpdate::new(thing, module_data)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_update() -> DeletionUpdate {
        DeletionUpdate {
            clock: UpdateModuleClock::default(),
            delete_frame: 0,
        }
    }

    #[test]
    fn deletion_update_exposes_typed_lifetime_interface() {
        let mut update = test_update();
        let current_frame = crate::helpers::TheGameLogic::get_frame();

        let deletion = update
            .get_deletion_lifetime_interface()
            .expect("DeletionUpdate should expose lifetime control");
        assert_eq!(deletion.set_lifetime_range(5, 5), current_frame + 5);
        assert_eq!(update.delete_frame, current_frame + 5);
    }

    #[test]
    fn scheduled_update_requests_destruction_even_before_delete_frame() {
        struct Schedule {
            destroy_owner: bool,
        }
        impl UpdateScheduleContext for Schedule {
            fn frame(&self) -> u32 {
                0
            }
            fn stage_owner_destruction(&mut self) {
                self.destroy_owner = true;
            }
            fn awaken(&mut self, _: &crate::object::UpdateModulePtr, _: u32) {}
            fn unregister(&mut self, _: &crate::object::UpdateModulePtr) {}
        }
        let object = Arc::new(RwLock::new(GameObject::new_test(9701, 100.0)));
        let module_data = game_engine::common::thing::module::ModuleDataHandle::new(DeletionUpdateModuleData {
            min_lifetime: 30,
            max_lifetime: 30,
            ..DeletionUpdateModuleData::default()
        });
        let mut update = DeletionUpdate::new(Arc::clone(&object), module_data).unwrap();

        let mut schedule = Schedule {
            destroy_owner: false,
        };
        assert!(update.delete_frame > schedule.frame());
        assert_eq!(
            update.update_scheduled(&mut schedule).unwrap(),
            UpdateSleepTime::Forever
        );
        assert!(schedule.destroy_owner);
    }
}
