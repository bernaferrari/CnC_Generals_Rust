//! Supply-center / warehouse lookups routed through the controlling player's
//! owned [`ResourceGatheringManager`].
//!
//! # Source
//! `GeneralsMD/Code/GameEngine/Source/Common/RTS/ResourceGatheringManager.cpp`
//! and `Include/Common/ResourceGatheringManager.h`, including the
//! `TheActionManager->canTransferSuppliesAt` and dock-clearance gates inside
//! `computeRelativeCost`.
//!
//! # Ownership
//! C++ keeps one manager per `Player`. Rust does the same: these free functions
//! resolve the object's controlling player and mutate that player's manager.
//! There is no process-wide resource service.

use std::collections::HashMap;

use crate::action_manager::ActionManager;
use crate::common::{Coord3D, Coord3DExt, ObjectID, Real};
use crate::object::registry::OBJECT_REGISTRY;
use crate::player;
use crate::supply_system::{DEFAULT_WAREHOUSE_MAX_DISTANCE_SQUARED, ResourceGatheringManager};

type ObjectId = u32;

fn with_owner_resource_manager<R>(
    object_id: ObjectId,
    f: impl FnOnce(&mut ResourceGatheringManager, ObjectID, Coord3D) -> R,
) -> Option<R> {
    let object = OBJECT_REGISTRY.get_object(object_id)?;
    let (player_id, position) = {
        let guard = object.read().ok()?;
        let player_id = guard.get_controlling_player_id()?;
        let position = *guard.get_position();
        (player_id, position)
    };
    player::with_player_mut(player_id as i32, |player| {
        let manager = player.get_resource_manager_mut()?;
        Some(f(manager, object_id, position))
    })?
}

/// C++ `computeRelativeCost`: transfer + clear-to-approach + distance².
fn relative_cost(
    query_id: ObjectID,
    dest_id: ObjectID,
) -> Option<(Coord3D, Coord3D, Real)> {
    let query = OBJECT_REGISTRY.get_object(query_id)?;
    let dest = OBJECT_REGISTRY.get_object(dest_id)?;
    let query_guard = query.read().ok()?;
    let dest_guard = dest.read().ok()?;
    if dest_guard.is_destroyed() {
        return None;
    }
    if !ActionManager::can_transfer_supplies_at(&query_guard, &dest_guard) {
        return None;
    }
    let is_clear = dest_guard
        .with_dock_update_interface(|dock| dock.is_clear_to_approach(query_id).unwrap_or(true))
        .unwrap_or(true);
    if !is_clear {
        return None;
    }
    let truck_position = *query_guard.get_position();
    let dest_position = *dest_guard.get_position();
    let distance_squared = truck_position.distance_squared_to(&dest_position);
    Some((truck_position, dest_position, distance_squared))
}

fn has_update_module(object_id: ObjectID, module_name: &str) -> bool {
    let Some(object) = OBJECT_REGISTRY.get_object(object_id) else {
        return false;
    };
    let Ok(guard) = object.read() else {
        return false;
    };
    guard.find_update_module(module_name).is_some()
}

fn dock_maps(
    query_id: ObjectID,
    ids: &[ObjectID],
) -> (HashMap<ObjectID, Coord3D>, HashMap<ObjectID, bool>) {
    let mut positions = HashMap::new();
    let mut available = HashMap::new();
    for &id in ids {
        match relative_cost(query_id, id) {
            Some((_, dest_position, _)) => {
                positions.insert(id, dest_position);
                available.insert(id, true);
            }
            None => {
                available.insert(id, false);
            }
        }
    }
    (positions, available)
}

/// Preferred dock + warehouse scan distance from the truck AI.
///
/// C++ seeds `maxDistanceSquared = 100000` and only replaces it when a
/// `SupplyTruckAIInterface` is present.
fn preferred_dock_and_scan(query_id: ObjectId) -> (Option<ObjectID>, Real) {
    let Some(object) = OBJECT_REGISTRY.get_object(query_id) else {
        return (None, DEFAULT_WAREHOUSE_MAX_DISTANCE_SQUARED);
    };
    let Ok(guard) = object.read() else {
        return (None, DEFAULT_WAREHOUSE_MAX_DISTANCE_SQUARED);
    };
    let Some(ai) = guard.get_ai_update_interface() else {
        return (None, DEFAULT_WAREHOUSE_MAX_DISTANCE_SQUARED);
    };
    let Ok(ai_guard) = ai.lock() else {
        return (None, DEFAULT_WAREHOUSE_MAX_DISTANCE_SQUARED);
    };
    let Some(truck) = ai_guard.get_supply_truck_ai_interface() else {
        return (None, DEFAULT_WAREHOUSE_MAX_DISTANCE_SQUARED);
    };
    let preferred = truck.get_preferred_dock_id();
    let is_ai_player = guard
        .get_controlling_player_id()
        .and_then(|player_id| {
            player::with_player(player_id as i32, |player| player.is_skirmish_ai())
        })
        .unwrap_or(false);
    let scan = truck
        .get_warehouse_scan_distance(is_ai_player)
        .unwrap_or_else(|| DEFAULT_WAREHOUSE_MAX_DISTANCE_SQUARED.sqrt());
    (preferred, scan * scan)
}

pub fn add_supply_center(center_id: ObjectId) {
    let _ = with_owner_resource_manager(center_id, |manager, id, _| {
        manager.add_supply_center(id);
    });
}

pub fn remove_supply_center(center_id: ObjectId) {
    let _ = with_owner_resource_manager(center_id, |manager, id, _| {
        manager.remove_supply_center(id);
    });
}

pub fn add_supply_warehouse(warehouse_id: ObjectId) {
    let _ = with_owner_resource_manager(warehouse_id, |manager, id, _| {
        manager.add_supply_warehouse(id);
    });
}

pub fn remove_supply_warehouse(warehouse_id: ObjectId) {
    let _ = with_owner_resource_manager(warehouse_id, |manager, id, _| {
        manager.remove_supply_warehouse(id);
    });
}

pub fn find_best_supply_warehouse(query_id: ObjectId) -> Option<ObjectId> {
    let (preferred, max_distance_squared) = preferred_dock_and_scan(query_id);
    let preferred_ok = preferred.is_some_and(|dock| {
        has_update_module(dock, "SupplyWarehouseDockUpdate")
            && relative_cost(query_id, dock).is_some()
    });
    with_owner_resource_manager(query_id, |manager, _, truck_position| {
        manager.retain_existing_warehouses(|id| OBJECT_REGISTRY.get_object(id).is_some());
        let warehouses = manager.get_supply_warehouses().to_vec();
        let (positions, available) = dock_maps(query_id, &warehouses);
        manager.find_best_supply_warehouse(
            &truck_position,
            preferred,
            preferred_ok,
            max_distance_squared,
            &positions,
            &available,
        )
    })
    .flatten()
}

pub fn find_best_supply_center(query_id: ObjectId) -> Option<ObjectId> {
    let (preferred, _) = preferred_dock_and_scan(query_id);
    let preferred_ok = preferred.is_some_and(|dock| {
        has_update_module(dock, "SupplyCenterDockUpdate")
            && relative_cost(query_id, dock).is_some()
    });
    with_owner_resource_manager(query_id, |manager, _, truck_position| {
        manager.retain_existing_centers(|id| OBJECT_REGISTRY.get_object(id).is_some());
        let centers = manager.get_supply_centers().to_vec();
        let (positions, available) = dock_maps(query_id, &centers);
        manager.find_best_supply_center(
            &truck_position,
            preferred,
            preferred_ok,
            &positions,
            &available,
        )
    })
    .flatten()
}

pub fn reset() {
    // Per-player managers are cleared with their owning Player/session reset.
    // Kept as a no-op so leftover call sites compile without a process store.
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::Player;
    use std::sync::{Arc, Barrier, Mutex, RwLock};
    use std::thread;

    #[test]
    fn parallel_players_keep_independent_supply_lists() {
        let barrier = Arc::new(Barrier::new(2));
        let results = Arc::new(Mutex::new(Vec::new()));
        let mut handles = Vec::new();
        for (index, center_id) in [(0i32, 100u32), (1, 101)] {
            let barrier = Arc::clone(&barrier);
            let results = Arc::clone(&results);
            handles.push(thread::spawn(move || {
                let player = Arc::new(RwLock::new(Player::new(index)));
                player.write().unwrap().init_from_dict_defaults();
                barrier.wait();
                {
                    let mut guard = player.write().unwrap();
                    let mgr = guard.get_resource_manager_mut().expect("manager");
                    mgr.add_supply_center(center_id);
                    mgr.add_supply_warehouse(center_id + 100);
                }
                barrier.wait();
                let centers = player
                    .read()
                    .unwrap()
                    .get_resource_manager()
                    .unwrap()
                    .get_supply_centers()
                    .to_vec();
                results.lock().unwrap().push((index, centers));
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        let mut got = results.lock().unwrap();
        got.sort_by_key(|(i, _)| *i);
        assert_eq!(got[0].1, vec![100]);
        assert_eq!(got[1].1, vec![101]);
        assert_ne!(got[0].1, got[1].1);
    }
}
