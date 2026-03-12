//! PilotFindVehicleUpdate - Ejected pilot finds and enters vehicle
//! Author: EA Pacific (C++ version) | Rust conversion: 2025

use crate::common::{KindOf, ModuleData, ObjectID, Real, UnsignedInt};
use crate::helpers::{TheGameLogic, ThePartitionManager};
use crate::modules::{BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime};
use crate::object::behavior::behavior_module::BehaviorModuleData;
use crate::object::contain::open_contain::ObjectRelationship;
use crate::object::{Object as GameObject, INVALID_ID as OBJECT_INVALID_ID};
use std::sync::{Arc, RwLock, Weak};

#[derive(Clone, Debug)]
pub struct PilotFindVehicleUpdateModuleData {
    pub base: BehaviorModuleData,
    pub scan_rate: UnsignedInt,
    pub scan_range: Real,
}

impl Default for PilotFindVehicleUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            scan_rate: 15,
            scan_range: 150.0,
        }
    }
}

crate::impl_behavior_module_data_via_base!(PilotFindVehicleUpdateModuleData, base);

pub struct PilotFindVehicleUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<PilotFindVehicleUpdateModuleData>,
    target_vehicle: ObjectID,
}

impl PilotFindVehicleUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .as_any()
            .downcast_ref::<PilotFindVehicleUpdateModuleData>()
            .ok_or("Invalid module data")?;

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            target_vehicle: OBJECT_INVALID_ID,
        })
    }
}

impl UpdateModuleInterface for PilotFindVehicleUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        let Some(owner_arc) = self.object.upgrade() else {
            return UpdateSleepTime::Forever;
        };
        let Ok(owner_guard) = owner_arc.read() else {
            return UpdateSleepTime::Forever;
        };

        if owner_guard.is_destroyed() || owner_guard.get_container().is_some() {
            return UpdateSleepTime::Forever;
        }

        let owner_id = owner_guard.get_id();
        let owner_pos = *owner_guard.get_position();

        let object_ids = ThePartitionManager::get()
            .map(|mgr| mgr.get_objects_in_range(&owner_pos, self.module_data.scan_range))
            .unwrap_or_default();

        let mut best_target = None;
        let mut best_dist_sqr = Real::MAX;

        for obj_id in object_ids {
            if obj_id == owner_id {
                continue;
            }

            let Some(obj_arc) = TheGameLogic::find_object_by_id(obj_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };

            if obj_guard.is_destroyed() || !obj_guard.is_kind_of(KindOf::Vehicle) {
                continue;
            }

            if owner_guard.get_relationship_to(&obj_guard) != ObjectRelationship::Ally {
                continue;
            }

            let Some(contain_arc) = obj_guard.get_contain() else {
                continue;
            };
            let Ok(contain_guard) = contain_arc.lock() else {
                continue;
            };
            if contain_guard.get_contained_count() >= contain_guard.get_max_capacity() {
                continue;
            }

            let pos = obj_guard.get_position();
            let dx = pos.x - owner_pos.x;
            let dy = pos.y - owner_pos.y;
            let dist_sqr = dx * dx + dy * dy;
            if dist_sqr < best_dist_sqr {
                best_dist_sqr = dist_sqr;
                best_target = Some(obj_id);
            }
        }

        if let Some(target_id) = best_target {
            if let Some(target_arc) = TheGameLogic::find_object_by_id(target_id) {
                if let Ok(target_guard) = target_arc.read() {
                    if let Some(contain_arc) = target_guard.get_contain() {
                        if let Ok(mut contain_guard) = contain_arc.lock() {
                            let _ = contain_guard.contain_object(owner_id);
                        }
                    }
                }
            }
        }

        UpdateSleepTime::from_u32(self.module_data.scan_rate)
    }
}

impl BehaviorModuleInterface for PilotFindVehicleUpdate {
    fn get_module_name(&self) -> &'static str {
        "PilotFindVehicleUpdate"
    }
    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
}

pub struct PilotFindVehicleUpdateFactory;
impl PilotFindVehicleUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(PilotFindVehicleUpdate::new(thing, module_data)?))
    }
}
