//! DeletionUpdate - Auto-deletion of objects after conditions
//! Author: EA Pacific (C++ version) | Rust conversion: 2025

use crate::common::{Bool, ModuleData, TheGameLogic, UnsignedInt};
use crate::modules::{BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::Object as GameObject;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::DeletionLifetimeInterface;
use std::sync::{Arc, RwLock, Weak};

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

#[allow(dead_code)]
pub struct DeletionUpdate {
    #[allow(dead_code)]
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<DeletionUpdateModuleData>,
    next_call_frame_and_phase: UnsignedInt,
    delete_frame: UnsignedInt,
}

impl DeletionUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .downcast_ref::<DeletionUpdateModuleData>()
            .ok_or("Invalid module data")?;

        // Get current frame from game logic - matches C++ DeletionUpdate.cpp
        let current_frame = crate::helpers::TheGameLogic::get_frame();
        let lifetime =
            Self::calc_sleep_delay_static(specific_data.min_lifetime, specific_data.max_lifetime);

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            next_call_frame_and_phase: current_frame + lifetime,
            delete_frame: current_frame + lifetime,
        })
    }

    pub fn set_lifetime_range(&mut self, min_lifetime: UnsignedInt, max_lifetime: UnsignedInt) {
        let current_frame = crate::helpers::TheGameLogic::get_frame();
        let delay = Self::calc_sleep_delay_static(min_lifetime, max_lifetime);
        self.delete_frame = current_frame + delay;
        self.next_call_frame_and_phase = self.delete_frame;
        if let Some(object) = self.object.upgrade() {
            if let Ok(object) = object.read() {
                crate::helpers::TheGameLogic::set_wake_frame(
                    object.get_id(),
                    UpdateSleepTime::from_u32(delay),
                );
            }
        }
    }

    pub fn initial_wake_frame(&self) -> UnsignedInt {
        self.next_call_frame_and_phase
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
    fn update_simple(&mut self) -> UpdateSleepTime {
        // C++ destroys whenever the scheduled update is invoked; timing is owned by the scheduler.
        if let Some(object) = self.object.upgrade() {
            if let Ok(guard) = object.read() {
                let _ = TheGameLogic::destroy_object(&guard);
            }
        }
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

    fn get_deletion_lifetime_interface(&mut self) -> Option<&mut dyn DeletionLifetimeInterface> {
        Some(self)
    }
}

impl DeletionLifetimeInterface for DeletionUpdate {
    fn set_lifetime_range(&mut self, min_lifetime: UnsignedInt, max_lifetime: UnsignedInt) {
        DeletionUpdate::set_lifetime_range(self, min_lifetime, max_lifetime);
    }
}

impl Snapshotable for DeletionUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("DeletionUpdate xfer version: {:?}", e))?;

        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;

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
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(DeletionUpdate::new(thing, module_data)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_update() -> DeletionUpdate {
        DeletionUpdate {
            object: Weak::new(),
            module_data: Arc::new(DeletionUpdateModuleData::default()),
            next_call_frame_and_phase: 0,
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
        deletion.set_lifetime_range(5, 5);

        assert_eq!(update.delete_frame, current_frame + 5);
    }

    #[test]
    fn update_destroys_even_before_delete_frame_when_invoked_like_cpp() {
        let object = Arc::new(RwLock::new(GameObject::new_test(9701, 100.0)));
        let module_data: Arc<dyn ModuleData> = Arc::new(DeletionUpdateModuleData {
            min_lifetime: 30,
            max_lifetime: 30,
            ..DeletionUpdateModuleData::default()
        });
        let mut update = DeletionUpdate::new(Arc::clone(&object), module_data).unwrap();

        assert!(update.delete_frame > crate::helpers::TheGameLogic::get_frame());
        assert_eq!(update.update_simple(), UpdateSleepTime::Forever);
    }
}
