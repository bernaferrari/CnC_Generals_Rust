use std::sync::Mutex;

use game_engine::rts::resource_gathering_manager::{ObjectId, ResourceGatheringManager};
use once_cell::sync::Lazy;

use crate::resource_world::LiveResourceWorld;

static RESOURCE_SERVICE: Lazy<Mutex<ResourceService>> =
    Lazy::new(|| Mutex::new(ResourceService::new()));

pub struct ResourceService {
    manager: ResourceGatheringManager,
}

impl ResourceService {
    fn new() -> Self {
        Self {
            manager: ResourceGatheringManager::new(),
        }
    }

    fn with_manager<F, R>(f: F) -> R
    where
        F: FnOnce(&mut ResourceGatheringManager, LiveResourceWorld) -> R,
    {
        // Safe mutex access with panic recovery
        let mut guard = match RESOURCE_SERVICE.lock() {
            Ok(g) => g,
            Err(poisoned) => {
                eprintln!("WARN: ResourceService lock poisoned, recovering...");
                poisoned.into_inner()
            }
        };
        let world = LiveResourceWorld::new();
        f(&mut guard.manager, world)
    }

    fn reset(&mut self) {
        self.manager.clear();
    }
}

pub fn add_supply_center(center_id: ObjectId) {
    // Safe mutex access with panic recovery
    let mut guard = match RESOURCE_SERVICE.lock() {
        Ok(g) => g,
        Err(poisoned) => {
            eprintln!("WARN: ResourceService lock poisoned, recovering...");
            poisoned.into_inner()
        }
    };
    guard.manager.add_supply_center(center_id);
}

pub fn remove_supply_center(center_id: ObjectId) {
    // Safe mutex access with panic recovery
    let mut guard = match RESOURCE_SERVICE.lock() {
        Ok(g) => g,
        Err(poisoned) => {
            eprintln!("WARN: ResourceService lock poisoned, recovering...");
            poisoned.into_inner()
        }
    };
    guard.manager.remove_supply_center(center_id);
}

pub fn add_supply_warehouse(warehouse_id: ObjectId) {
    // Safe mutex access with panic recovery
    let mut guard = match RESOURCE_SERVICE.lock() {
        Ok(g) => g,
        Err(poisoned) => {
            eprintln!("WARN: ResourceService lock poisoned, recovering...");
            poisoned.into_inner()
        }
    };
    guard.manager.add_supply_warehouse(warehouse_id);
}

pub fn remove_supply_warehouse(warehouse_id: ObjectId) {
    // Safe mutex access with panic recovery
    let mut guard = match RESOURCE_SERVICE.lock() {
        Ok(g) => g,
        Err(poisoned) => {
            eprintln!("WARN: ResourceService lock poisoned, recovering...");
            poisoned.into_inner()
        }
    };
    guard.manager.remove_supply_warehouse(warehouse_id);
}

pub fn find_best_supply_warehouse(query_id: ObjectId) -> Option<ObjectId> {
    ResourceService::with_manager(|manager, world| {
        manager.find_best_supply_warehouse(query_id, &world)
    })
}

pub fn find_best_supply_center(query_id: ObjectId) -> Option<ObjectId> {
    ResourceService::with_manager(|manager, world| {
        manager.find_best_supply_center(query_id, &world)
    })
}

pub fn reset() {
    // Safe mutex access with panic recovery
    let mut guard = match RESOURCE_SERVICE.lock() {
        Ok(g) => g,
        Err(poisoned) => {
            eprintln!("WARN: ResourceService lock poisoned, recovering...");
            poisoned.into_inner()
        }
    };
    guard.reset();
}
